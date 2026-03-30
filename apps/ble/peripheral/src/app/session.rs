//! 产品 GATT 会话处理 / Product GATT session handling.
//!
//! 处理本产品特有的 GATT 读写事件（电量、echo、status、bulk）。
//! Handles GATT read/write events specific to this product (battery, echo, status, bulk).
//!
//! 将通用的 `GattEvent` 分派到对应的产品逻辑，与通用 GATT 会话驱动（`gatt::session`）解耦。
//! Dispatches generic `GattEvent`s to product-specific logic, decoupled from the
//! generic GATT session driver (`gatt::session`).

use crate::app::server::Server;
use crate::app::tasks::{record_rx, reset_stats, sync_bulk_stats};
use hello_ble_common::bulk;
use postcard::from_bytes;
use rtt_target::rprintln;
use trouble_host::prelude::*;

/// 产品特征的句柄缓存 / Cached handles for product characteristics.
///
/// 避免每次事件都从 server 查找特征。在会话开始时一次性克隆所有 handle。
/// Caches characteristic handles to avoid per-event lookups. Cloned once at session start.
struct ProductHandles {
    /// 电量特征 / Battery level handle.
    level: Characteristic<u8>,
    /// Echo 特征 / Echo handle.
    echo: Characteristic<heapless::Vec<u8, { hello_ble_common::echo::CAPACITY }>>,
    /// Status 特征 / Status handle.
    status_char: Characteristic<heapless::Vec<u8, { hello_ble_common::status::CAPACITY }>>,
    /// Bulk 控制特征 / Bulk control handle.
    bulk_control: Characteristic<heapless::Vec<u8, { bulk::CONTROL_CAPACITY }>>,
    /// Bulk 数据特征 / Bulk data handle.
    bulk_data: Characteristic<heapless::Vec<u8, { bulk::CHUNK_SIZE }>>,
    /// Bulk 统计特征 / Bulk stats handle.
    bulk_stats: Characteristic<heapless::Vec<u8, { bulk::STATS_CAPACITY }>>,
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

/// 分派单个 GATT 事件到产品处理逻辑 / Dispatch a single GATT event to product handlers.
///
/// 按 handle 匹配事件，执行对应产品的日志记录和副作用。
/// Matches events by handle and performs product-specific logging and side effects.
fn handle_gatt_event<P: PacketPool>(
    server: &Server<'_>,
    handles: &ProductHandles,
    event: &GattEvent<'_, '_, P>,
) {
    match event {
        // 电量读取 / Battery level read
        GattEvent::Read(event) if event.handle() == handles.level.handle => {
            rprintln!("[battery] read");
        }

        // Echo 写入 / Echo write
        GattEvent::Write(event) if event.handle() == handles.echo.handle => {
            rprintln!("[echo] write {} bytes", event.data().len());
        }

        // Status 读取 / Status read
        GattEvent::Read(event) if event.handle() == handles.status_char.handle => {
            match server.get(&handles.status_char) {
                Ok(raw) => {
                    let val: Result<bool, _> = from_bytes(&raw);
                    rprintln!("[status] read: {:?}", val);
                }
                Err(e) => rprintln!("[status] read error: {:?}", e),
            }
        }

        // Status 写入 / Status write
        GattEvent::Write(event) if event.handle() == handles.status_char.handle => {
            match from_bytes::<bool>(event.data()) {
                Ok(val) => rprintln!("[status] write: {}", val),
                Err(e) => rprintln!("[status] write error: {:?}", e),
            }
        }

        // Bulk 控制命令 / Bulk control command
        GattEvent::Write(event) if event.handle() == handles.bulk_control.handle => {
            match from_bytes::<bulk::BulkControlCommand>(event.data()) {
                Ok(cmd) => {
                    rprintln!("[bulk] control: {:?}", cmd);
                    if cmd == bulk::BulkControlCommand::ResetStats {
                        reset_stats();
                        sync_bulk_stats(server, &handles.bulk_stats);
                    }
                }
                Err(e) => rprintln!("[bulk] control error: {:?}", e),
            }
        }

        // Bulk 数据写入（上行）/ Bulk data write (upload direction)
        GattEvent::Write(event) if event.handle() == handles.bulk_data.handle => {
            rprintln!("[bulk] data write {} bytes", event.data().len());
            record_rx(event.data());
            sync_bulk_stats(server, &handles.bulk_stats);
        }

        // Bulk 统计读取（无需特殊处理）/ Bulk stats read (no special handling)
        GattEvent::Read(event) if event.handle() == handles.bulk_stats.handle => {}

        _ => {}
    }
}

/// 运行产品 GATT 会话 / Run the product GATT session.
///
/// 创建特征 handle 缓存，将事件通过 `handle_gatt_event` 分派，
/// 并委托给通用的 `gatt::session::run_session`。
/// Creates characteristic handle cache, dispatches events via `handle_gatt_event`,
/// and delegates to the generic `gatt::session::run_session`.
pub(crate) async fn run_product_session<P: PacketPool>(
    server: &Server<'_>,
    conn: &GattConnection<'_, '_, P>,
) -> Result<(), Error> {
    let handles = ProductHandles::new(server);
    crate::gatt::session::run_session(conn, |event| handle_gatt_event(server, &handles, event))
        .await
}
