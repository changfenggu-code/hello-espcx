//! GATT 连接阶段 — 将 AttributeServer 绑定到 BLE 连接
//! GATT connected stage — bind an AttributeServer to a BLE connection.
//!
//! ## 连接阶段 / Connected Stage
//!
//! `connected()` 将底层 BLE `Connection` 与 `AttributeServer` 绑定，
//! 返回可用于 GATT 读写的 `GattConnection`。
//! `connected()` binds the raw BLE `Connection` with an `AttributeServer`,
//! returning a `GattConnection` that can be used for GATT read/write operations.
//!
//! ## 生命周期要求 / Lifetime Requirements
//!
//! `AttributeServer` 必须具有 `'static` 生命周期。
//! 这通常通过 `Box::leak` 泄漏 `HostResources` 和 server 来实现（参见 `easyble::gap::init`）。
//! The `AttributeServer` must have `'static` lifetime.
//! This is typically achieved by leaking `HostResources` and the server via `Box::leak`
//! (see `easyble::gap::init`).

use embassy_sync::blocking_mutex::raw::RawMutex;
use trouble_host::prelude::*;

/// 将 AttributeServer 绑定到底层 BLE 连接
/// Bind an AttributeServer to the raw BLE connection.
///
/// 返回一个 `GattConnection`，可用于驱动 GATT 事件循环（`easyble::gatt::session`）。
/// Returns a `GattConnection` that can be used to drive the GATT event loop
/// (`easyble::gatt::session`).
///
/// ## 泛型参数 / Generic Parameters
///
/// - `ATT_MAX`: 属性表中最大属性数 / Maximum attributes in attribute table
/// - `CCCD_MAX`: CCCD 表格最大条目数 / Maximum CCCD table entries
/// - `CONN_MAX`: 连接槽最大数量 / Maximum connection slots
///
/// ## 参数说明 / Parameter Notes
///
/// `server` 参数必须具有 `'static` 生命周期。参见上文的"生命周期要求"。
/// The `server` parameter must have `'static` lifetime. See "Lifetime Requirements" above.
pub fn connected<
    'stack,
    'server,
    'values,
    M: RawMutex,
    const ATT_MAX: usize,
    const CCCD_MAX: usize,
    const CONN_MAX: usize,
>(
    conn: Connection<'stack, DefaultPacketPool>,
    // server 必须为 'static 生命周期（通过 Box::leak 实现）
    // server must have 'static lifetime (achieved via Box::leak)
    server: &'server AttributeServer<'values, M, DefaultPacketPool, ATT_MAX, CCCD_MAX, CONN_MAX>,
) -> Result<GattConnection<'stack, 'server, DefaultPacketPool>, Error> {
    conn.with_attribute_server(server)
}
