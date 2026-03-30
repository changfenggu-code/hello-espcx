//! Status Service — 演示 read + write + notify 三种操作 / Demonstrates read + write + notify GATT operations.
//!
//! 自定义服务，Central 可以读写一个布尔值，外设在值变更时可推送通知。
//! Custom service where Central can read/write a boolean value, and the peripheral
//! can push notifications on value changes.
//!
//! 数据用 postcard 序列化，初始值为 `false`。
//! Data is serialized with postcard, initial value is `false`.

use heapless::Vec;
use postcard::to_slice;
use trouble_host::prelude::*;

use hello_ble_common::status;

/// Status Service / 状态服务（自定义）。
///
/// 演示 GATT 的三种基本操作：read、write、notify。
/// Demonstrates three fundamental GATT operations: read, write, and notify.
#[gatt_service(uuid = status::SERVICE_UUID)]
pub struct StatusService {
    /// 状态特征，postcard 序列化的 bool 值 / Status characteristic, postcard-serialized bool.
    #[characteristic(uuid = status::UUID, read, write, notify, value = initial_status_value())]
    pub status: Vec<u8, { status::CAPACITY }>,
}

/// 生成 Status 特征的初始值（`false`） / Generate initial value for Status characteristic (`false`).
///
/// 将 `false` 通过 postcard 序列化为 `Vec<u8>`，供 `#[gatt_service]` 宏的 `value` 属性使用。
/// Serializes `false` via postcard into a `Vec<u8>` for the `#[gatt_service]` macro's `value` attribute.
pub fn initial_status_value() -> Vec<u8, { status::CAPACITY }> {
    let mut buf = [0u8; status::CAPACITY];
    let used = to_slice(&false, &mut buf).unwrap();
    Vec::from_slice(used).unwrap()
}
