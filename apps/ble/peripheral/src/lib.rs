//! Peripheral 产品层 — BLE 外设固件 / Peripheral product layer — BLE peripheral firmware.
//!
//! 本文件是 crate root（`lib.rs`），包含所有服务定义和 `easyble::AppHooks` 实现。
//! This file is the crate root (`lib.rs`), containing all service definitions and `easyble::AppHooks` implementation.
//!
//! ## 生命周期 / Lifecycle
//!
//! ```text
//! easyble::run()
//!   ├─ build_advertisement()  →  构建广播数据
//!   ├─ build_server()        →  构建 GATT server（泄漏为 'static）
//!   └─ on_session(conn)       →  每次连接会话
//! ```

#![no_std]
extern crate alloc;

// ============================================================================
// Services — 5 GATT 服务合并于此 / 5 GATT services merged here
// ============================================================================
//
// | 服务 / Service | UUID | 说明 / Description |
// |---|---|---|
// | Battery | 0x180F | 标准 BLE 电量 / Standard BLE battery |
// | Device Info | 0x180A | 标准 BLE 设备信息 / Standard BLE device info |
// | Echo | 自定义 | 双向数据完整性验证 / Bidirectional integrity check |
// | Status | 自定义 | read/write/notify 演示 / Read/write/notify demo |
// | Bulk | 自定义 | 大批量数据传输 / Bulk data transfer |

use heapless::Vec;
use postcard::to_slice;
use rtt_target::rprintln;
use trouble_host::prelude::*;

use hello_ble_common::{bulk, echo, status};

/// Battery Service / 电池服务（标准 BLE）。
///
/// UUID: `0x180F`（服务）、`0x2A19`（电量特征）。
#[gatt_service(uuid = service::BATTERY)]
pub struct BatteryService {
    #[characteristic(uuid = characteristic::BATTERY_LEVEL, read, notify, value = 50)]
    pub level: u8,
}

/// Device Information Service / 设备信息服务（标准 BLE）。
///
/// UUID: `0x180A`。
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

/// Echo Service / 回声服务（自定义）。
///
/// UUID: `echo::SERVICE_UUID`。Central 写入数据，Peripheral notify 回传。
#[gatt_service(uuid = echo::SERVICE_UUID)]
pub struct EchoService {
    #[characteristic(uuid = echo::UUID, write, notify, value = Vec::new())]
    pub echo: Vec<u8, { echo::CAPACITY }>,
}

/// Status Service / 状态服务（自定义）。
///
/// UUID: `status::SERVICE_UUID`。演示 read/write/notify。
#[gatt_service(uuid = status::SERVICE_UUID)]
pub struct StatusService {
    #[characteristic(uuid = status::UUID, read, write, notify, value = initial_status_value())]
    pub status: Vec<u8, { status::CAPACITY }>,
}

/// 生成 Status 初始值（`false`）/ Generate initial value for Status (`false`).
fn initial_status_value() -> Vec<u8, { status::CAPACITY }> {
    let mut buf = [0u8; status::CAPACITY];
    let used = to_slice(&false, &mut buf).unwrap();
    Vec::from_slice(used).unwrap()
}

/// Bulk Service / 批量传输服务（自定义）。
///
/// UUID: `bulk::SERVICE_UUID`。
#[gatt_service(uuid = bulk::SERVICE_UUID)]
pub struct BulkService {
    #[characteristic(uuid = bulk::CONTROL_UUID, write, read, value = initial_bulk_control_value())]
    pub control: Vec<u8, { bulk::CONTROL_CAPACITY }>,
    #[characteristic(uuid = bulk::CHUNK_UUID, write, write_without_response, notify, value = Vec::new())]
    pub data: Vec<u8, { bulk::CHUNK_SIZE }>,
    #[characteristic(uuid = bulk::STATS_UUID, read, value = initial_bulk_stats_value())]
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

/// 产品 GATT server（5 个服务）。
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
// AppHooks — easyble 集成 / easyble integration
// ============================================================================

/// 累计接收字节数 / Cumulative received bytes.
static RX_BYTES: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
/// 累计发送字节数 / Cumulative sent bytes.
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

/// 产品 App 状态 — 实现 `easyble::AppHooks`。
pub struct AppState {
    /// 泄漏的 GATT server 原始指针 / Leaked GATT server raw pointer.
    server_ptr: Option<*mut Server<'static>>,
    /// 广播数据 / Advertisement data.
    adv: easyble::AdvertisementData,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            server_ptr: None,
            adv: easyble::AdvertisementData {
                adv_data: [0; 31],
                adv_len: 0,
                scan_data: [0; 31],
                scan_len: 0,
            },
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl easyble::AppHooks for AppState {
    fn build_advertisement(&mut self) -> Result<easyble::AdvertisementData, Error> {
        use hello_ble_common::advertisement_identity;

        let mut adv_data = [0u8; 31];
        let manufacturer_payload =
            advertisement_identity::ManufacturerPayload::new(
                advertisement_identity::PRODUCT_ID_HELLO_ESPCX,
                advertisement_identity::unit_id_from_address(
                    hello_ble_common::PERIPHERAL_ADDRESS,
                ),
                0,
            )
            .to_bytes();

        let adv_len = AdStructure::encode_slice(
            &[
                AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
                AdStructure::ServiceUuids16(&[hello_ble_common::battery::SERVICE_UUID16
                    .to_le_bytes()]),
                AdStructure::CompleteLocalName(hello_ble_common::PERIPHERAL_NAME.as_bytes()),
                AdStructure::ManufacturerSpecificData {
                    company_identifier: advertisement_identity::DEVELOPMENT_COMPANY_ID,
                    payload: &manufacturer_payload,
                },
            ],
            &mut adv_data[..],
        )?;

        self.adv = easyble::AdvertisementData {
            adv_data,
            adv_len,
            scan_data: [0; 31],
            scan_len: 0,
        };
        Ok(easyble::AdvertisementData {
            adv_data: self.adv.adv_data,
            adv_len: self.adv.adv_len,
            scan_data: self.adv.scan_data,
            scan_len: self.adv.scan_len,
        })
    }

    fn build_server(&mut self) -> Result<(), Error> {
        let server = Server::new_with_config(GapConfig::Peripheral(PeripheralConfig {
            name: hello_ble_common::PERIPHERAL_NAME,
            appearance: &appearance::power_device::GENERIC_POWER_DEVICE,
        }))
        .unwrap();

        // 泄漏为 'static，绕过 Deref coercion 问题
        self.server_ptr =
            Some(alloc::boxed::Box::into_raw(alloc::boxed::Box::new(server)));
        Ok(())
    }

    async fn on_session(&mut self, conn: Connection<'_, DefaultPacketPool>) {
        let server = unsafe { &mut *self.server_ptr.unwrap() };
        let Ok(gatt_conn) = conn.with_attribute_server(server) else {
            rprintln!("[app] failed to bind GATT server");
            return;
        };

        let session_fut = run_product_session(&gatt_conn, server);
        let task_fut = custom_task(&gatt_conn, server);
        embassy_futures::join::join(session_fut, task_fut).await;
    }
}

// ============================================================================
// 会话处理 / Session handling
// ============================================================================

struct ProductHandles {
    level: Characteristic<u8>,
    echo: Characteristic<Vec<u8, { echo::CAPACITY }>>,
    status_char: Characteristic<Vec<u8, { status::CAPACITY }>>,
    bulk_control: Characteristic<Vec<u8, { bulk::CONTROL_CAPACITY }>>,
    bulk_data: Characteristic<Vec<u8, { bulk::CHUNK_SIZE }>>,
    bulk_stats: Characteristic<Vec<u8, { bulk::STATS_CAPACITY }>>,
}

impl ProductHandles {
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

async fn gatt_session(
    conn: &GattConnection<'_, '_, DefaultPacketPool>,
    mut on_event: impl for<'a, 's> FnMut(&GattEvent<'a, 's, DefaultPacketPool>),
) -> Result<(), Error> {
    let reason = loop {
        match conn.next().await {
            GattConnectionEvent::Disconnected { reason } => break reason,
            GattConnectionEvent::Gatt { event } => {
                on_event(&event);
                if let Ok(reply) = event.accept() {
                    let _ = reply.send().await;
                }
            }
            _ => {}
        }
    };
    rprintln!("[gatt] disconnected: {:?}", reason);
    Ok(())
}

async fn run_product_session(
    conn: &GattConnection<'_, '_, DefaultPacketPool>,
    server: &Server<'_>,
) {
    let handles = ProductHandles::new(server);
    let _ = gatt_session(conn, |e| handle_gatt_event(server, &handles, e)).await;
}

async fn custom_task(
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
        // 1. 检查 bulk StartStream 命令
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

        // 2. Echo 回传
        if let Ok(data) = server.get(&echo) {
            if !data.is_empty() {
                rprintln!("[echo] notifying {} bytes", data.len());
                if echo.notify(conn, &data).await.is_err() {
                    rprintln!("[echo] notify failed");
                }
                let _ = echo.set(server, &Vec::new());
            }
        }

        // 3. 电量通知
        battery_tick = battery_tick.wrapping_add(1);
        if level.notify(conn, &battery_tick).await.is_err() {
            break;
        }

        // 4. 等待 2 秒
        embassy_time::Timer::after_secs(2).await;
    }
}

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
