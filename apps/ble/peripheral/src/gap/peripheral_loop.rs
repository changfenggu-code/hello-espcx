//! GAP 外设主循环 / GAP peripheral main loop.
//!
//! 构建 BLE 协议栈，运行"广播→连接→会话→断开→重连"的无限循环。
//! Builds the BLE stack and runs the "advertise→connect→session→disconnect→reconnect" infinite loop.
//!
//! ```text
//! run(controller)
//!   ├─ 配置随机地址 / Set random address
//!   ├─ 创建 HostResources + Stack / Create host resources and stack
//!   ├─ 构建 app 运行时对象（server + 广播数据）/ Build app runtime bundle (server + advertising data)
//!   ├─ 并行启动 / Run in parallel:
//!   │   ├─ ble_task: 协议栈驱动循环 / Stack driver loop
//!   │   └─ 广播-连接-会话循环 / Advertise-connect-session loop
//!   │       ├─ advertise() → accept() → Connection
//!   │       ├─ app.run_connected_session()
//!   │       │   ├─ with_attribute_server(server)
//!   │       │   ├─ run_product_session (GATT 事件处理) / GATT event handling
//!   │       │   └─ custom_task (主动推送任务) / Active push tasks
//!   │       └─ 会话结束后重新广播 / Re-advertise after session ends
//!   └─ 致命错误 → 软件复位 / Fatal error → software reset
//! ```

use crate::app::runtime::build_runtime;
use crate::gap::advertising::advertise;
use embassy_futures::join::join;
use embassy_time::Timer;
use esp_hal::system::software_reset;
use hello_ble_common::PERIPHERAL_ADDRESS;
use rtt_target::rprintln;
use trouble_host::prelude::*;

/// 最大并发连接数 / Maximum concurrent BLE connections.
const CONNECTIONS_MAX: usize = 1;

/// L2CAP 通道数（Signal + ATT，不含 CoC）/ L2CAP channels (Signal + ATT, no CoC).
const L2CAP_CHANNELS_MAX: usize = 2;

/// 构建 BLE 协议栈并运行外设生命周期循环 / Build the BLE host stack and run the peripheral lifecycle loop.
///
/// 这是外设侧的顶级入口，由 `main.rs` 调用。此函数永远不会正常返回。
/// Top-level entry for the peripheral side, called from `main.rs`. Never returns normally.
///
/// 致命错误时执行软件复位恢复。
/// On fatal errors, performs a software reset to recover.
pub async fn run<C>(controller: C)
where
    C: Controller,
{
    // 设置固定随机蓝牙地址 / Set fixed random BLE address
    let address = Address::random(PERIPHERAL_ADDRESS);
    rprintln!("Our address = {:?}", address);

    // 分配协议栈资源（连接池、L2CAP 通道池）/ Allocate stack resources
    let mut resources: HostResources<DefaultPacketPool, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX> =
        HostResources::new();
    let stack = trouble_host::new(controller, &mut resources).set_random_address(address);
    let Host {
        mut peripheral, runner, ..
    } = stack.build();

    // 构建 app 运行时对象（server + 广播数据）/ Build app runtime bundle (server + advertising data)
    let app = match build_runtime() {
        Ok(runtime) => runtime,
        Err(e) => log_error_and_reset("adv-build", &e).await,
    };

    rprintln!("Starting advertising with 4 services");

    // 并行运行：协议栈驱动 + 外设主循环 / Run in parallel: stack driver + peripheral loop
    let _ = join(ble_task(runner), async {
        loop {
            match advertise(&mut peripheral, app.advertising_view()).await {
                Ok(conn) => {
                    // 连接成功，运行 app 层定义的连接会话 / Connected, run the app-defined connected session
                    if let Err(e) = app.run_connected_session(conn).await {
                        log_error_and_reset("session", &e).await;
                    }
                    // 会话结束（断开或任务退出）→ 重新广播 / Session ended (disconnect or task exit) → re-advertise
                }
                Err(e) => {
                    log_error_and_reset("adv", &e).await;
                }
            }
        }
    })
    .await;
}

/// trouble-host 协议栈驱动循环 / trouble-host stack driver loop.
///
/// 持续调用 `runner.run()` 驱动底层 BLE 协议处理。出错则复位。
/// Continuously calls `runner.run()` to drive the BLE stack. Resets on error.
async fn ble_task<C: Controller, P: PacketPool>(mut runner: Runner<'_, C, P>) {
    loop {
        if let Err(e) = runner.run().await {
            log_error_and_reset("ble_task", &e).await;
        }
    }
}

/// 记录致命错误并执行软件复位 / Log fatal error and perform software reset.
///
/// 等待 100ms 让 RTT 缓冲区刷新后再复位。此函数永不返回（`-> !`）。
/// Waits 100ms for RTT buffer to flush before resetting. Never returns (`-> !`).
async fn log_error_and_reset<E: core::fmt::Debug>(context: &str, error: &E) -> ! {
    rprintln!("[fatal:{}] {:?}", context, error);
    rprintln!("[fatal:{}] resetting...", context);
    Timer::after_millis(100).await;
    software_reset()
}
