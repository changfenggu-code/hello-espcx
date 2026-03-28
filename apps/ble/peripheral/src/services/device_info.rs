//! Device Information Service — standard BLE service for static device info.

#![allow(dead_code)] // provided for standard BLE compliance

use trouble_host::prelude::*;

/// Device Information Service (standard BLE): read only
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
