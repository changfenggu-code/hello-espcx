//! Peripheral 产品层 — BLE 外设固件产品逻辑
//! Peripheral product layer / BLE peripheral firmware product logic.
//!
//! 本文件是 crate root（`lib.rs`），包含所有产品特定的代码：
//! The app keeps product-owned pieces here:
//! - GATT 服务定义 / service definitions
//! - 广播载荷构建 / advertisement payload building
//! - 产品特定的 GATT 事件处理 / product-specific GATT event handling
//! - 产品特定的主动推送任务 / product-specific active tasks
//!
//! 生命周期循环由二进制 target（`main.rs`）手动组装，lib 只提供构建块。
//! The lifecycle loop itself is now assembled manually by the binary target.

#![no_std]
extern crate alloc;

use alloc::boxed::Box;
use heapless::Vec;
use postcard::to_slice;
use rtt_target::rprintln;
use trouble_host::prelude::*;

use hello_ble_common::{advertisement_identity, battery, bulk, echo, status};

// ============================================================================
// Services — 5 个 GATT 服务定义 / 5 GATT service definitions
// ============================================================================

/// Battery Service / 标准 BLE 电量监测服务.
///
/// UUID: `0x180F`（服务）、`0x2A19`（电量特征）。支持 read + notify。
#[gatt_service(uuid = service::BATTERY)]
pub struct BatteryService {
    /// 电量百分比，支持 read + notify / Battery level percentage, read + notify.
    #[characteristic(uuid = characteristic::BATTERY_LEVEL, read, notify, value = 50)]
    pub level: u8,
}

/// Device Information Service / 标准 BLE 设备信息服务.
///
/// UUID: `0x180A`。全部只读，连接后读取一次即可。
#[gatt_service(uuid = service::DEVICE_INFORMATION)]
pub struct DeviceInfoService {
    #[characteristic(uuid = characteristic::MANUFACTURER_NAME_STRING, read, value = "ESP")]
    pub manufacturer: &'static str,
    #[characteristic(uuid = characteristic::MODEL_NUMBER_STRING, read, value = "ESP32-C6")]
    pub model: &'static str,
    #[characteristic(uuid = characteristic::FIRMWARE_REVISION_STRING, read, value = "1.0.0")]
    pub firmware: &'static str,
    #[characteristic(uuid = characteristic::SOFTWARE_REVISION_STRING, read, value = env!("CARGO_PKG_VERSION"))]
    pub software: &'static str,
}

/// Echo Service / 自定义回声服务.
///
/// UUID: `echo::service::UUID128`（服务）、
/// `echo::characteristic::ECHO_UUID128`（特征）。Central 写入数据，Peripheral notify 回传。
#[gatt_service(uuid = echo::service::UUID128)]
pub struct EchoService {
    /// Echo 特征，支持 write + notify / Echo characteristic, write + notify.
    #[characteristic(uuid = echo::characteristic::ECHO_UUID128, write, notify, value = Vec::new())]
    pub echo: Vec<u8, { echo::CAPACITY }>,
}

/// Status Service / 自定义状态服务.
///
/// UUID: `status::service::UUID128`（服务）、
/// `status::characteristic::STATUS_UUID128`（特征）。演示 read + write + notify。
#[gatt_service(uuid = status::service::UUID128)]
pub struct StatusService {
    /// 状态特征，postcard 序列化的 bool / Status characteristic, postcard-serialized bool.
    #[characteristic(uuid = status::characteristic::STATUS_UUID128, read, write, notify, value = initial_status_value())]
    pub status: Vec<u8, { status::CAPACITY }>,
}

/// 生成 Status 初始值（`false`）/ Generate initial value for Status characteristic (`false`).
fn initial_status_value() -> Vec<u8, { status::CAPACITY }> {
    let mut buf = [0u8; status::CAPACITY];
    let used = to_slice(&false, &mut buf).unwrap();
    Vec::from_slice(used).unwrap()
}

/// Bulk Service / 自定义批量传输服务.
///
/// UUID: `bulk::service::UUID128`（服务）、
/// `bulk::characteristic::CONTROL_UUID128` / `DATA_UUID128` / `STATS_UUID128`（特征）。
/// 支持双向大批量数据传输。
#[gatt_service(uuid = bulk::service::UUID128)]
pub struct BulkService {
    /// 控制特征：Idle / ResetStats / StartStream 命令 / Control: Idle/ResetStats/StartStream commands.
    #[characteristic(uuid = bulk::characteristic::CONTROL_UUID128, write, read, value = initial_bulk_control_value())]
    pub control: Vec<u8, { bulk::CONTROL_CAPACITY }>,
    /// 数据特征：双向传输（write = 上传，notify = 下发）/ Data: bidirectional (write=upload, notify=download).
    #[characteristic(uuid = bulk::characteristic::DATA_UUID128, write, write_without_response, notify, value = Vec::new())]
    pub data: Vec<u8, { bulk::CHUNK_SIZE }>,
    /// 统计特征：只读，反映 rx/tx 字节计数 / Stats: read-only, reflects rx/tx byte counters.
    #[characteristic(uuid = bulk::characteristic::STATS_UUID128, read, value = initial_bulk_stats_value())]
    pub stats: Vec<u8, { bulk::STATS_CAPACITY }>,
}

/// 生成 Bulk 控制特征初始值（`Idle`）/ Generate initial bulk control value (`Idle`).
pub fn initial_bulk_control_value() -> Vec<u8, { bulk::CONTROL_CAPACITY }> {
    let mut buf = [0u8; bulk::CONTROL_CAPACITY];
    let used = to_slice(&bulk::BulkControlCommand::Idle, &mut buf).unwrap();
    Vec::from_slice(used).unwrap()
}

/// 生成 Bulk 统计特征初始值 / Generate initial bulk stats value.
fn initial_bulk_stats_value() -> Vec<u8, { bulk::STATS_CAPACITY }> {
    let mut buf = [0u8; bulk::STATS_CAPACITY];
    let used = to_slice(&bulk::BulkStats::default(), &mut buf).unwrap();
    Vec::from_slice(used).unwrap()
}

// ============================================================================
// Server — GATT 属性服务器 / GATT attribute server
// ============================================================================

/// 产品 GATT server（由 5 个服务组成）。
#[allow(clippy::needless_borrows_for_generic_args)]
#[gatt_server]
pub struct Server {
    pub battery_service: BatteryService,
    pub device_info_service: DeviceInfoService,
    pub echo_service: EchoService,
    pub status_service: StatusService,
    pub bulk_service: BulkService,
}

// ============================================================================
// 产品入口函数 / Product entry functions
// ============================================================================

/// 构建产品广播载荷 / Build the product advertisement payload.
///
/// 包含 Flags、Service UUIDs、设备名和厂商数据。
/// Contains Flags, Service UUIDs, device name, and manufacturer data.
pub fn build_advertisement() -> Result<easyble::gap::AdvertisementData, Error> {
    let mut adv_data = [0u8; 31];
    let manufacturer_payload = advertisement_identity::ManufacturerPayload::new(
        advertisement_identity::VERSION,
        advertisement_identity::PRODUCT_ID_HELLO_ESPCX,
        advertisement_identity::unit_id_from_address(hello_ble_common::PERIPHERAL_ADDRESS),
        0,
    )
    .to_bytes();

    let adv_len = AdStructure::encode_slice(
        &[
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
            AdStructure::ServiceUuids16(&[battery::service::UUID16.to_le_bytes()]),
            AdStructure::CompleteLocalName(hello_ble_common::PERIPHERAL_NAME.as_bytes()),
            AdStructure::ManufacturerSpecificData {
                company_identifier: advertisement_identity::DEVELOPMENT_COMPANY_ID,
                payload: &manufacturer_payload,
            },
        ],
        &mut adv_data[..],
    )?;

    Ok(easyble::gap::AdvertisementData {
        adv_data,
        adv_len,
        scan_data: [0; 31],
        scan_len: 0,
    })
}

/// 构建并泄漏产品 server，使生命周期循环可跨会话复用
/// Build and leak the product server so the app lifecycle loop can reuse it
/// across advertising sessions.
pub fn build_server() -> Result<&'static Server<'static>, Error> {
    let server = Server::new_with_config(GapConfig::Peripheral(PeripheralConfig {
        name: hello_ble_common::PERIPHERAL_NAME,
        appearance: &appearance::power_device::GENERIC_POWER_DEVICE,
    }))
    .unwrap();

    Ok(Box::leak(Box::new(server)))
}

// ============================================================================
// 传输统计 / Transfer stats
// ============================================================================

/// 累计接收字节数（Central → Peripheral）/ Cumulative received bytes (Central → Peripheral).
static RX_BYTES: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
/// 累计发送字节数（Peripheral → Central）/ Cumulative sent bytes (Peripheral → Central).
static TX_BYTES: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

/// 重置传输统计 / Reset transfer stats.
pub fn reset_stats() {
    RX_BYTES.store(0, core::sync::atomic::Ordering::Relaxed);
    TX_BYTES.store(0, core::sync::atomic::Ordering::Relaxed);
}

/// 记录接收数据量 / Record received data length.
pub fn record_rx(data: &[u8]) {
    RX_BYTES.fetch_add(data.len() as u32, core::sync::atomic::Ordering::Relaxed);
}

/// 同步 bulk 统计到 GATT 特征 / Sync bulk stats to GATT characteristic.
fn sync_bulk_stats(
    server: &Server<'_>,
    bulk_stats: &Characteristic<Vec<u8, { bulk::STATS_CAPACITY }>>,
) {
    let stats = bulk::BulkStats {
        rx_bytes: RX_BYTES.load(core::sync::atomic::Ordering::Relaxed),
        tx_bytes: TX_BYTES.load(core::sync::atomic::Ordering::Relaxed),
    };
    let mut buf = [0u8; bulk::STATS_CAPACITY];
    if let Ok(used) = postcard::to_slice(&stats, &mut buf) {
        if let Ok(vec) = Vec::from_slice(used) {
            let _ = bulk_stats.set(server, &vec);
        }
    }
}

// ============================================================================
// GATT 事件处理 / GATT event handling
// ============================================================================

/// 产品特征的句柄缓存 / Cached handles for product characteristics.
///
/// 避免每次事件都从 server 查找特征。在会话开始时一次性克隆所有 handle。
struct ProductHandles {
    level: Characteristic<u8>,
    echo: Characteristic<Vec<u8, { echo::CAPACITY }>>,
    status_char: Characteristic<Vec<u8, { status::CAPACITY }>>,
    bulk_control: Characteristic<Vec<u8, { bulk::CONTROL_CAPACITY }>>,
    bulk_data: Characteristic<Vec<u8, { bulk::CHUNK_SIZE }>>,
    bulk_stats: Characteristic<Vec<u8, { bulk::STATS_CAPACITY }>>,
}

impl ProductHandles {
    /// 从 server 中提取所有产品特征的 handle / Extract all product characteristic handles from server.
    fn new(server: &Server<'_>) -> Self {
        Self {
            level: server.battery_service.level,
            echo: server.echo_service.echo.clone(),
            status_char: server.status_service.status.clone(),
            bulk_control: server.bulk_service.control.clone(),
            bulk_data: server.bulk_service.data.clone(),
            bulk_stats: server.bulk_service.stats.clone(),
        }
    }
}

/// 处理 GATT 事件分派 / Handle GATT event dispatch.
///
/// 按 handle 匹配事件，执行对应产品的日志记录和副作用。
fn handle_gatt_event(
    server: &Server<'_>,
    handles: &ProductHandles,
    event: &GattEvent<'_, '_, DefaultPacketPool>,
) {
    use postcard::from_bytes as pb_from_bytes;

    match event {
        GattEvent::Read(e) if e.handle() == handles.level.handle => {
            rprintln!("[battery] read");
        }
        GattEvent::Write(e) if e.handle() == handles.echo.handle => {
            rprintln!("[echo] write {} bytes", e.data().len());
        }
        GattEvent::Read(e) if e.handle() == handles.status_char.handle => {
            if let Ok(raw) = server.get(&handles.status_char) {
                let val: Result<bool, _> = pb_from_bytes(&raw);
                rprintln!("[status] read: {:?}", val);
            }
        }
        GattEvent::Write(e) if e.handle() == handles.status_char.handle => {
            if let Ok(val) = pb_from_bytes::<bool>(e.data()) {
                rprintln!("[status] write: {}", val);
            }
        }
        GattEvent::Write(e) if e.handle() == handles.bulk_control.handle => {
            if let Ok(cmd) = pb_from_bytes::<bulk::BulkControlCommand>(e.data()) {
                rprintln!("[bulk] control: {:?}", cmd);
                if cmd == bulk::BulkControlCommand::ResetStats {
                    reset_stats();
                    sync_bulk_stats(server, &handles.bulk_stats);
                }
            }
        }
        GattEvent::Write(e) if e.handle() == handles.bulk_data.handle => {
            rprintln!("[bulk] data write {} bytes", e.data().len());
            record_rx(e.data());
            sync_bulk_stats(server, &handles.bulk_stats);
        }
        _ => {}
    }
}

// ============================================================================
// 连接会话任务 / Connected session tasks
// ============================================================================

/// 运行产品 GATT 会话（被动事件处理）/ Run product GATT session (passive event handling).
///
/// 持续从连接中读取 GATT 事件并分派给 `handle_gatt_event`，直到连接断开。
pub async fn run_product_session(
    conn: &GattConnection<'_, '_, DefaultPacketPool>,
    server: &Server<'_>,
) {
    let handles = ProductHandles::new(server);
    let _ = easyble::gatt::session(conn, |event| handle_gatt_event(server, &handles, event)).await;
}

/// 运行主动推送任务 / Run active push tasks.
///
/// 在后台持续运行，直到 notify 失败（连接断开）时退出。
/// 运行：bulk 流、echo 回传、电量通知，循环间隔 2 秒。
pub async fn custom_task(
    conn: &GattConnection<'_, '_, DefaultPacketPool>,
    server: &Server<'_>,
) {
    use postcard::from_bytes as pb_from_bytes;

    let level = server.battery_service.level;
    let echo = server.echo_service.echo.clone();
    let bulk_control = server.bulk_service.control.clone();
    let bulk_data = server.bulk_service.data.clone();
    let bulk_stats = server.bulk_service.stats.clone();

    let mut battery_tick: u8 = 0;

    loop {
        // 1. 检查 bulk StartStream 命令 / Check bulk StartStream command
        if let Ok(raw) = server.get(&bulk_control) {
            if let Ok(bulk::BulkControlCommand::StartStream { total_bytes }) =
                pb_from_bytes::<bulk::BulkControlCommand>(&raw)
            {
                rprintln!("[bulk] starting stream: {} bytes", total_bytes);
                run_bulk_stream(server, conn, &bulk_stats, &bulk_data, &bulk_control, total_bytes)
                    .await;
                continue;
            }
        }

        // 2. Echo 回传 / Echo notify
        if let Ok(data) = server.get(&echo) {
            if !data.is_empty() {
                rprintln!("[echo] notifying {} bytes", data.len());
                if echo.notify(conn, &data).await.is_err() {
                    rprintln!("[echo] notify failed");
                }
                let _ = echo.set(server, &Vec::new());
            }
        }

        // 3. 电量通知 / Battery notify
        battery_tick = battery_tick.wrapping_add(1);
        if level.notify(conn, &battery_tick).await.is_err() {
            break;
        }

        // 4. 等待 2 秒 / Wait 2 seconds
        embassy_time::Timer::after_secs(2).await;
    }
}

/// 执行 bulk 下发流 / Execute bulk download stream.
///
/// 用确定性测试模式填满每个 chunk，通过 notify 逐块发送直到达到 `total_bytes`。
async fn run_bulk_stream(
    server: &Server<'_>,
    conn: &GattConnection<'_, '_, DefaultPacketPool>,
    bulk_stats: &Characteristic<Vec<u8, { bulk::STATS_CAPACITY }>>,
    bulk_data: &Characteristic<Vec<u8, { bulk::CHUNK_SIZE }>>,
    bulk_control: &Characteristic<Vec<u8, { bulk::CONTROL_CAPACITY }>>,
    total_bytes: u32,
) {
    let mut chunk = [0u8; bulk::CHUNK_SIZE];

    TX_BYTES.store(0, core::sync::atomic::Ordering::Relaxed);
    sync_bulk_stats(server, bulk_stats);

    let total = total_bytes as usize;
    for offset in (0..total).step_by(bulk::CHUNK_SIZE) {
        let len = (total - offset).min(bulk::CHUNK_SIZE);
        hello_ble_common::fill_test_pattern(offset, &mut chunk[..len]);
        let payload = match Vec::from_slice(&chunk[..len]) {
            Ok(v) => v,
            Err(_) => break,
        };

        if bulk_data.notify(conn, &payload).await.is_err() {
            rprintln!("[bulk] notify error");
            break;
        }
        TX_BYTES.fetch_add(payload.len() as u32, core::sync::atomic::Ordering::Relaxed);
        sync_bulk_stats(server, bulk_stats);
    }

    let _ = bulk_control.set(server, &initial_bulk_control_value());
    rprintln!("[bulk] stream complete: {} bytes", total_bytes);
}
