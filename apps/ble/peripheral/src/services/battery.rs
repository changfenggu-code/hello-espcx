//! Battery Service — 标准 BLE 电量监测服务 / Standard BLE battery level monitoring service.
//!
//! UUID: `0x180F`（服务）、`0x2A19`（电量特征），均为 BLE SIG 标准分配。
//! UUIDs: `0x180F` (service), `0x2A19` (level characteristic), both BLE SIG standard-assigned.
//!
//! 支持 read（Central 主动读取）和 notify（外设定期推送）。
//! Supports read (Central polls) and notify (Peripheral pushes periodically).

use trouble_host::prelude::*;

/// Battery Service / 电池服务（标准 BLE）。
///
/// 包含一个 `level` 特征（u8，0–100%），初始值为 50。
/// Contains one `level` characteristic (u8, 0–100%), initial value 50.
#[gatt_service(uuid = service::BATTERY)]
pub struct BatteryService {
    /// 电量百分比，支持 read + notify / Battery level percentage, read + notify.
    #[characteristic(uuid = characteristic::BATTERY_LEVEL, read, notify, value = 50)]
    pub level: u8,
}
