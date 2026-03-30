//! Device Information Service — 标准 BLE 设备信息服务 / Standard BLE device information service.
//!
//! UUID: `0x180A`（服务），各特征均为 BLE SIG 标准分配的 16-bit UUID。
//! UUID: `0x180A` (service), all characteristics use BLE SIG standard 16-bit UUIDs.
//!
//! 全部只读，Central 连接后读取一次即可。
//! All read-only. Central reads once after connection.

#![allow(dead_code)] // 提供标准 BLE 合规性，并非所有字段都直接使用 / Provided for standard BLE compliance

use trouble_host::prelude::*;

/// Device Information Service / 设备信息服务（标准 BLE）。
///
/// 包含制造商、型号、固件版本、软件版本四个只读字符串。
/// Contains manufacturer, model, firmware revision, and software revision as read-only strings.
///
/// 注意：字段值为硬编码产品信息。通用化时应改为构造参数。
/// Note: field values are hardcoded product info. For reusability, make them constructor parameters.
#[gatt_service(uuid = service::DEVICE_INFORMATION)]
pub struct DeviceInfoService {
    /// 制造商名称 / Manufacturer name.
    #[characteristic(uuid = characteristic::MANUFACTURER_NAME_STRING, read, value = "ESP")]
    pub manufacturer: &'static str,
    /// 型号 / Model number.
    #[characteristic(uuid = characteristic::MODEL_NUMBER_STRING, read, value = "ESP32-C6")]
    pub model: &'static str,
    /// 固件版本 / Firmware revision.
    #[characteristic(uuid = characteristic::FIRMWARE_REVISION_STRING, read, value = "1.0.0")]
    pub firmware: &'static str,
    /// 软件版本（编译时从 Cargo.toml 注入）/ Software revision (injected from Cargo.toml at build time).
    #[characteristic(uuid = characteristic::SOFTWARE_REVISION_STRING, read, value = env!("CARGO_PKG_VERSION"))]
    pub software: &'static str,
}
