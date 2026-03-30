//! BLE 外设入口 — 运行在 ESP32-C6
//! BLE Peripheral entry point running on ESP32-C6.
//!
//! ## 生命周期 / Lifecycle
//!
//! ```text
//! init
//!   ├─ 初始化 ESP32 外设 / Init ESP32 peripherals
//!   ├─ 配置 RTOS 调度器 / Configure RTOS scheduler
//!   ├─ 创建 BLE controller / Create BLE controller
//!   └─ 进入外设生命周期循环 / Enter peripheral lifecycle loop
//! ```
//!
//! ## 生命周期循环 / Lifecycle Loop
//!
//! ```text
//! advertising
//!   └─ easyble::gap::advertising()
//!       └─ accept() 等待连接 / wait for connection
//!           └─ connected → session → disconnected → advertising ...
//! ```

#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_futures::join::join;
use esp_alloc as _; // 堆内存分配器，必须链接 / Heap allocator, must be linked
use esp_hal::{
    clock::CpuClock,
    interrupt::software::SoftwareInterruptControl,
    timer::timg::TimerGroup,
};
use esp_radio::ble::controller::BleConnector;
use hello_ble_common::{PERIPHERAL_ADDRESS, PERIPHERAL_NAME};
use hello_ble_peripheral::{build_advertisement, build_server, custom_task, run_product_session};
use rtt_target::{rprintln, rtt_init_print};
use trouble_host::prelude::ExternalController;

/// 最大并发连接数 / Maximum concurrent BLE connections.
const CONNECTIONS_MAX: usize = 1;
/// L2CAP 通道数（Signal + ATT）/ L2CAP channel count (Signal + ATT).
const L2CAP_CHANNELS_MAX: usize = 2;

/// 全局 panic 处理 — 通过 RTT 输出后死循环。
#[panic_handler]
fn panic(panic_info: &core::panic::PanicInfo) -> ! {
    rprintln!("{}", panic_info);
    loop {}
}

// 向 ESP-IDF bootloader 注册应用描述信息 / Register app description with ESP-IDF bootloader.
esp_bootloader_esp_idf::esp_app_desc!();

/// 程序入口 / Application entry point.
///
/// 由 `esp_rtos` 调度，负责初始化硬件并启动 BLE 外设循环。
/// Scheduled by `esp_rtos`, initializes hardware and starts the BLE peripheral loop.
#[esp_rtos::main]
async fn main(_s: Spawner) {
    rtt_init_print!();

    // === 1. 初始化 ESP32 外设 / Init ESP32 peripherals ===
    rprintln!("[init] {} peripheral starting...", PERIPHERAL_NAME);
    let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));

    // === 2. 配置 RTOS 调度器 / Configure RTOS scheduler ===
    // embassy 异步运行时需要硬件 timer + software interrupt 驱动
    rprintln!("[init] configuring timer and software interrupt...");
    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_int = SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_int.software_interrupt0);

    // === 3. 创建 BLE controller / Create BLE controller ===
    // 将 ESP32-C6 蓝牙硬件包装为 trouble-host 可用的 ExternalController
    rprintln!("[init] initializing BLE controller...");
    let bluetooth = peripherals.BT;
    let connector = BleConnector::new(bluetooth, Default::default()).unwrap();
    let controller: ExternalController<_, 1> = ExternalController::new(connector);

    // === 4. 构建产品数据 / Build product data ===
    let advertisement = match build_advertisement() {
        Ok(data) => data,
        Err(e) => fatal("build_advertisement", &e),
    };
    let server = match build_server() {
        Ok(server) => server,
        Err(e) => fatal("build_server", &e),
    };

    // === 5. 初始化 BLE 协议栈 / Init BLE protocol stack ===
    let easyble::InitializedStack {
        mut peripheral,
        runner,
    } = easyble::gap::init::<_, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX>(
        controller,
        easyble::InitConfig {
            address: PERIPHERAL_ADDRESS,
        },
    );

    // === 6. 并行运行：协议栈驱动 + 外设生命周期循环 ===
    // === Run in parallel: stack driver + peripheral lifecycle loop ===
    rprintln!("[init] starting BLE peripheral lifecycle...");
    join(
        async {
            // BLE 协议栈驱动循环 / BLE stack driver loop
            if let Err(e) = easyble::gap::run_stack(runner).await {
                fatal("ble_task", &e);
            }
        },
        async {
            // 外设生命周期循环 / Peripheral lifecycle loop
            loop {
                // 广播阶段 / Advertising phase
                let conn = match easyble::gap::advertising(&mut peripheral, advertisement.as_view()).await {
                    Ok(conn) => conn,
                    Err(e) => fatal("advertising", &e),
                };

                // GATT 连接阶段 / GATT connected phase
                let gatt_conn = match easyble::gatt::connected(conn, server) {
                    Ok(conn) => conn,
                    Err(e) => fatal("connected", &e),
                };

                // 会话阶段 / Session phase
                rprintln!("[session] started");
                join(
                    run_product_session(&gatt_conn, server),
                    custom_task(&gatt_conn, server),
                )
                .await;
                rprintln!("[session] ended -> advertising");
            }
        },
    )
    .await;
}

/// 记录致命错误并执行软件复位 / Log fatal error and perform software reset.
///
/// 打印错误信息后通过软件复位恢复。此函数永不返回。
/// Logs error and performs a software reset. Never returns.
fn fatal<E: core::fmt::Debug>(context: &str, error: &E) -> ! {
    rprintln!("[fatal:{}] {:?}", context, error);
    rprintln!("[fatal:{}] resetting...", context);
    esp_hal::system::software_reset()
}
