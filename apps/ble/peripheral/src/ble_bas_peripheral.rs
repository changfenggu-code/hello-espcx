//! BLE Peripheral — ESP32-C6 BLE 外设实现
//!
//! ## 整体架构
//!
//! Peripheral 固件有三个并行的异步任务：
//!
//! ```text
//! ┌──────────────────────────────────────────────────┐
//! │                  run() 主入口                     │
//! │  ┌──────────────┐   ┌────────────────────────┐   │
//! │  │  ble_task()  │ + │  advertise() → 等待连接 │   │
//! │  └──────┬───────┘   └───────────┬────────────┘   │
//! │         │                       │                │
//! │         │            连接建立后并发执行：          │
//! │         │         ┌─────────────┴─────────┐      │
//! │         │         │  select(gatt_events,  │      │
//! │         │         │          custom_task) │      │
//! │         │         └───────────────────────┘      │
//! └─────────┴────────────────────────────────────────┘
//! ```
//!
//! - **ble_task**：运行 BLE 协议栈底层（Host），保证 BLE 控制器始终工作
//! - **advertise**：等待 Central 连接，连接后返回 GattConnection
//! - **gatt_events_task**：处理所有 GATT 请求（读、写、通知订阅）
//! - **custom_task**：主动发起的任务（定时通知、bulk 流、echo 回传）

#![allow(dead_code)] // device_info_service provided for standard BLE compliance

use crate::services::{
    BatteryService, BulkService, DeviceInfoService, EchoService, StatusService,
};

/// GATT 属性服务器，聚合所有服务。
///
/// `#[gatt_server]` proc-macro 读取这些 struct 定义，生成完整的 GATT 属性表
///（Service → Characteristic → Descriptor），并生成 Server 实例化代码。
/// Central 连接后会发现这些属性并发起操作。
#[allow(clippy::needless_borrows_for_generic_args)]
#[allow(dead_code)] // device_info_service provided for standard BLE compliance
#[gatt_server]
struct Server {
    battery_service: BatteryService,
    device_info_service: DeviceInfoService,
    echo_service: EchoService,
    status_service: StatusService,
    bulk_service: BulkService,
}

use core::sync::atomic::{AtomicU32, Ordering};
use embassy_futures::join::join;
use embassy_futures::select::select;
use embassy_time::Timer;
use esp_hal::system::software_reset;
use heapless::Vec;
use postcard::from_bytes;
use rtt_target::rprintln;
use crate::services::initial_bulk_control_value;
use trouble_host::prelude::*;

use hello_ble_common::{
    advertisement_identity, battery, bulk, fill_test_pattern, PERIPHERAL_ADDRESS, PERIPHERAL_NAME,
};

// ============================================================================
// 连接配置
// ============================================================================

const CONNECTIONS_MAX: usize = 1;
const L2CAP_CHANNELS_MAX: usize = 2;

// ============================================================================
// 全局状态（用于 Bulk 传输统计）
// ============================================================================

/// Peripheral 收到的字节数（Central → Peripheral 上传方向）。
static RX_BYTES: AtomicU32 = AtomicU32::new(0);
/// Peripheral 发出的字节数（Peripheral → Central 下发方向）。
static TX_BYTES: AtomicU32 = AtomicU32::new(0);

// ============================================================================
// 入口函数
// ============================================================================

/// 启动 BLE 外设。
///
/// 创建 BLE Host，配置随机地址，组装 GATT Server，然后进入主循环：
/// 广播 → 等待连接 → 处理请求 → 断开 → 重新广播。
pub async fn run<C>(controller: C)
where
    C: Controller,
{
    // 1. 设置随机地址
    let address = Address::random(PERIPHERAL_ADDRESS);
    rprintln!("Our address = {:?}", address);

    // 2. 创建 BLE Host 资源（内存池、 最大连接数、最大L2CAP通道数）
    let mut resources: HostResources<DefaultPacketPool, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX> =
        HostResources::new();
    let stack = trouble_host::new(controller, &mut resources).set_random_address(address);
    let Host {
        mut peripheral, runner, ..
    } = stack.build();

    // 3. 创建 GATT Server 实例（关联所有服务）
    let server = Server::new_with_config(GapConfig::Peripheral(PeripheralConfig {
        name: PERIPHERAL_NAME,
        appearance: &appearance::power_device::GENERIC_POWER_DEVICE,
    }))
    .unwrap();

    rprintln!("Starting advertising with 4 services");

    // 4. 启动 BLE Host + 主循环
    //    ble_task 独占一个 branch，advertise/handle 连接独占另一个 branch
    let _ = join(ble_task(runner), async {
        loop {
            match advertise(&mut peripheral, &server).await {
                Ok(conn) => {
                    // 连接建立后并发运行两个任务：
                    // - gatt_events_task：处理 GATT 请求（Central 发起的读/写）
                    // - custom_task：主动任务（定时通知、echo 回传、bulk 流）
                    let a = gatt_events_task(&server, &conn);
                    let b = custom_task::<C, DefaultPacketPool>(&server, &conn);
                    select(a, b).await;
                }
                Err(e) => {
                    log_error_and_reset("adv", &e).await;
                }
            }
        }
    })
    .await;
}

// ============================================================================
// 广播
// ============================================================================

/// 开始广播，等待 Central 连接。
///
/// 在广告包里声明：
/// - Flags: LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED
/// - Service UUID: 0x180F (Battery Service) — 用于 Central 做 OS 层扫描过滤
/// - Local Name: "hello-espcx"
///
/// Central 连接后，`accept()` 返回 GattConnection，
/// 再用 `with_attribute_server()` 把 GATT Server 附加到连接上。
async fn advertise<'values, 'server, C: Controller>(
    peripheral: &mut Peripheral<'values, C, DefaultPacketPool>,
    server: &'server Server<'values>,
) -> Result<GattConnection<'values, 'server, DefaultPacketPool>, BleHostError<C::Error>> {
    let mut advertiser_data = [0; 31];
    let manufacturer_payload = advertisement_identity::ManufacturerPayload::new(
        advertisement_identity::PRODUCT_ID_HELLO_ESPCX,
        advertisement_identity::unit_id_from_address(PERIPHERAL_ADDRESS),
        0,
    )
    .to_bytes();
    let len = AdStructure::encode_slice(
        &[
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
            AdStructure::ServiceUuids16(&[battery::SERVICE_UUID16.to_le_bytes()]),
            AdStructure::CompleteLocalName(PERIPHERAL_NAME.as_bytes()),
            AdStructure::ManufacturerSpecificData {
                company_identifier: advertisement_identity::DEVELOPMENT_COMPANY_ID,
                payload: &manufacturer_payload,
            },
        ],
        &mut advertiser_data[..],
    )?;

    let advertiser = peripheral
        .advertise(
            &Default::default(),
            Advertisement::ConnectableScannableUndirected {
                adv_data: &advertiser_data[..len],
                scan_data: &[],
            },
        )
        .await?;

    rprintln!("[adv] advertising");
    let conn = advertiser.accept().await?.with_attribute_server(server)?;
    rprintln!("[adv] connection established");
    Ok(conn)
}

// ============================================================================
// BLE Host 运行循环
// ============================================================================

/// 运行 BLE 协议栈底层。
///
/// `Runner` 不断调用 BLE 控制器的 `run()`，处理所有底层事件
///（连接事件、数据包、超时等）。如果出错就重启芯片。
async fn ble_task<C: Controller, P: PacketPool>(mut runner: Runner<'_, C, P>) {
    loop {
        if let Err(e) = runner.run().await {
            log_error_and_reset("ble_task", &e).await;
        }
    }
}

// ============================================================================
// GATT 事件处理（响应 Central 的请求）
// ============================================================================

/// 处理 Central 发来的所有 GATT 请求。
///
/// `conn.next().await` 是一个无限流，等待下一个 GATT 事件。
/// 循环直到收到 `Disconnected` 事件。
///
/// ### 事件分发逻辑
///
/// 对于每个收到的 GATT 事件，根据 handle 判断是哪个特征的操作：
///
/// | handle | 事件 | 处理 |
/// |--------|------|------|
/// | level.handle | Read | 记录日志（值由 GATT Server 自动返回） |
/// | echo.handle | Write | 记录日志，实际 echo 在 custom_task 里做 |
/// | status_char.handle | Read | 反序列化并记录 |
/// | status_char.handle | Write | 反序列化并记录 |
/// | bulk_control.handle | Write | 解析命令，如果是 ResetStats 则重置计数器 |
/// | bulk_data.handle | Write | 累加 rx 计数，同步到 stats |
/// | bulk_stats.handle | Read | stats 已自动同步，无需额外处理 |
async fn gatt_events_task<P: PacketPool>(
    server: &Server<'_>,
    conn: &GattConnection<'_, '_, P>,
) -> Result<(), Error> {
    // 提前提取所有特征的 handle，避免每次事件都查找
    let level = server.battery_service.level;
    let echo = server.echo_service.echo.clone();
    let status_char = server.status_service.status.clone();
    let bulk_control = server.bulk_service.control.clone();
    let bulk_data = server.bulk_service.data.clone();
    let bulk_stats = server.bulk_service.stats.clone();

    let reason = loop {
        match conn.next().await {
            GattConnectionEvent::Disconnected { reason } => break reason,
            GattConnectionEvent::Gatt { event } => {
                match &event {
                    // Battery: Read 请求
                    // GATT Server 会自动返回特征值（初始值 50），这里只记录日志
                    GattEvent::Read(event) if event.handle() == level.handle => {
                        rprintln!("[battery] read");
                    }

                    // Echo: Write 请求
                    // 数据会被存入 echo 特征，custom_task 会检测到并回复
                    GattEvent::Write(event) if event.handle() == echo.handle => {
                        rprintln!("[echo] write {} bytes", event.data().len());
                    }

                    // Status: Read 请求
                    GattEvent::Read(event) if event.handle() == status_char.handle => {
                        match server.get(&status_char) {
                            Ok(raw) => {
                                let val: Result<bool, _> = from_bytes(&raw);
                                rprintln!("[status] read: {:?}", val);
                            }
                            Err(e) => rprintln!("[status] read error: {:?}", e),
                        }
                    }

                    // Status: Write 请求
                    GattEvent::Write(event) if event.handle() == status_char.handle => {
                        match from_bytes::<bool>(event.data()) {
                            Ok(val) => rprintln!("[status] write: {}", val),
                            Err(e) => rprintln!("[status] write error: {:?}", e),
                        }
                    }

                    // Bulk Control: Write 命令
                    GattEvent::Write(event) if event.handle() == bulk_control.handle => {
                        match from_bytes::<bulk::BulkControlCommand>(event.data()) {
                            Ok(cmd) => {
                                rprintln!("[bulk] control: {:?}", cmd);
                                if cmd == bulk::BulkControlCommand::ResetStats {
                                    reset_stats();
                                    sync_bulk_stats(server, &bulk_stats);
                                }
                                // StartStream 命令在 custom_task 里处理（因为是异步的）
                            }
                            Err(e) => rprintln!("[bulk] control error: {:?}", e),
                        }
                    }

                    // Bulk Data: Write（Central → Peripheral 上传数据）
                    GattEvent::Write(event) if event.handle() == bulk_data.handle => {
                        rprintln!("[bulk] data write {} bytes", event.data().len());
                        record_rx(event.data());
                        sync_bulk_stats(server, &bulk_stats);
                    }

                    // Bulk Stats: Read（值已自动同步，直接返回）
                    GattEvent::Read(event) if event.handle() == bulk_stats.handle => {}

                    // 其他事件忽略
                    _ => {}
                };

                // 发送 GATT 响应（ACK）
                // 对于 write-with-response，这会等待 Central 的确认
                match event.accept() {
                    Ok(reply) => reply.send().await,
                    Err(e) => rprintln!("[gatt] error sending response: {:?}", e),
                };
            }
            _ => {}
        }
    };
    rprintln!("[gatt] disconnected: {:?}", reason);
    Ok(())
}

// ============================================================================
// 主动任务（定时通知、echo 回传、bulk 流）
// ============================================================================

/// Peripheral 主动发起的任务，和 gatt_events_task 并发运行。
///
/// 轮询检查 GATT 属性状态，检测到需要主动发起的操作后执行。
/// 每轮结束后等待 2 秒，然后继续检查。
///
/// ### 轮询检查顺序
///
/// 1. **Bulk Stream**：检查 bulk_control 是否收到 StartStream 命令
///    → 如果是，执行 `run_bulk_stream` 逐块推送数据
/// 2. **Echo**：检查 echo 特征是否有新数据
///    → 如果有，通过 notify 把同样数据发回 Central
/// 3. **Battery**：每轮都发送电量通知（递增的 tick 值）
async fn custom_task<C: Controller, P: PacketPool>(
    server: &Server<'_>,
    conn: &GattConnection<'_, '_, P>,
) {
    let level = server.battery_service.level;
    let echo = server.echo_service.echo.clone();
    let bulk_control = server.bulk_service.control.clone();
    let bulk_data = server.bulk_service.data.clone();
    let bulk_stats = server.bulk_service.stats.clone();

    let mut battery_tick: u8 = 0;

    loop {
        // 1. 检查 Bulk Stream 命令
        //    StartStream 是在 gatt_events_task 里收到但无法同步处理的，
        //    所以在这里轮询 detect 到后启动
        if let Ok(raw) = server.get(&bulk_control) {
            if let Ok(bulk::BulkControlCommand::StartStream { total_bytes }) =
                from_bytes::<bulk::BulkControlCommand>(&raw)
            {
                rprintln!("[bulk] starting stream: {} bytes", total_bytes);
                run_bulk_stream(
                    server,
                    conn,
                    &bulk_stats,
                    &bulk_data,
                    &bulk_control,
                    total_bytes,
                )
                .await;
                continue; // bulk 流结束后继续循环
            }
        }

        // 2. 检查 Echo 数据
        if let Ok(data) = server.get(&echo) {
            if !data.is_empty() {
                rprintln!("[echo] notifying {} bytes", data.len());
                if echo.notify(conn, &data).await.is_err() {
                    rprintln!("[echo] notify failed");
                }
                // 清空 echo 缓冲区，避免重复发送
                let _ = echo.set(server, &Vec::new());
            }
        }

        // 3. 定时发送电量通知（每 2 秒一次）
        battery_tick = battery_tick.wrapping_add(1);
        if level.notify(conn, &battery_tick).await.is_err() {
            break; // 连接断了，退出循环
        }

        Timer::after_secs(2).await;
    }
}

// ============================================================================
// Bulk Stream 实现
// ============================================================================

/// 执行 bulk 数据流下发（Peripheral → Central）。
///
/// 收到 StartStream 命令后调用此函数：
/// 1. 重置 tx 计数器
/// 2. 逐块生成测试数据并通过 notify 发送
/// 3. 每块发送后同步 stats，让 Central 可以实时读取进度
/// 4. 发完后将 control 设为 Idle
async fn run_bulk_stream<P: PacketPool>(
    server: &Server<'_>,
    conn: &GattConnection<'_, '_, P>,
    bulk_stats: &Characteristic<Vec<u8, { bulk::STATS_CAPACITY }>>,
    bulk_data: &Characteristic<Vec<u8, { bulk::CHUNK_SIZE }>>,
    bulk_control: &Characteristic<Vec<u8, { bulk::CONTROL_CAPACITY }>>,
    total_bytes: u32,
) {
    let mut chunk = [0u8; bulk::CHUNK_SIZE];

    // 重置计数器并同步到 GATT 属性
    TX_BYTES.store(0, Ordering::Relaxed);
    sync_bulk_stats(server, bulk_stats);

    let total = total_bytes as usize;
    for offset in (0..total).step_by(bulk::CHUNK_SIZE) {
        let len = (total - offset).min(bulk::CHUNK_SIZE);

        // 用确定性公式生成测试数据
        fill_test_pattern(offset, &mut chunk[..len]);
        let payload = match Vec::from_slice(&chunk[..len]) {
            Ok(v) => v,
            Err(_) => break, // 内存不足，终止
        };

        // 通过 notify 发送，fire-and-forget
        if bulk_data.notify(conn, &payload).await.is_err() {
            rprintln!("[bulk] notify error");
            break;
        }
        record_tx(&payload);
        sync_bulk_stats(server, bulk_stats);
    }

    // 发完后将 control 设为 Idle
    let _ = bulk_control.set(server, &initial_bulk_control_value());
    rprintln!("[bulk] stream complete: {} bytes", total_bytes);
}

// ============================================================================
// Bulk 统计辅助函数
// ============================================================================

/// 重置 rx/tx 计数器。
fn reset_stats() {
    RX_BYTES.store(0, Ordering::Relaxed);
    TX_BYTES.store(0, Ordering::Relaxed);
}

/// 累加收到的字节数。
fn record_rx(data: &[u8]) {
    RX_BYTES.fetch_add(data.len() as u32, Ordering::Relaxed);
}

/// 累加发送的字节数。
fn record_tx(data: &[u8]) {
    TX_BYTES.fetch_add(data.len() as u32, Ordering::Relaxed);
}

/// 将原子计数器同步到 GATT BulkStats 特征。
///
/// 每当 rx/tx 变化时就同步一次，这样 Central 任何时候读 stats
/// 都能拿到最新的计数值。
fn sync_bulk_stats(
    server: &Server<'_>,
    bulk_stats: &Characteristic<Vec<u8, { bulk::STATS_CAPACITY }>>,
) {
    let stats = bulk::BulkStats {
        rx_bytes: RX_BYTES.load(Ordering::Relaxed),
        tx_bytes: TX_BYTES.load(Ordering::Relaxed),
    };
    let mut buf = [0u8; bulk::STATS_CAPACITY];
    if let Ok(used) = postcard::to_slice(&stats, &mut buf) {
        if let Ok(vec) = Vec::from_slice(used) {
            let _ = bulk_stats.set(server, &vec);
        }
    }
}

// ============================================================================
// 错误处理
// ============================================================================

/// 打印致命错误并重启 ESP32。
///
/// BLE Host 出错通常是底层控制器问题，无法恢复，重启是最安全的做法。
async fn log_error_and_reset<E: core::fmt::Debug>(context: &str, error: &E) -> ! {
    rprintln!("[fatal:{}] {:?}", context, error);
    rprintln!("[fatal:{}] resetting...", context);
    Timer::after_millis(100).await;
    software_reset()
}
