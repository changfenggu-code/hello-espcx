//! Battery Service — standard BLE service for battery level monitoring.

use trouble_host::prelude::*;

/// Battery Service (standard BLE): read + notify
#[gatt_service(uuid = service::BATTERY)]
pub struct BatteryService {
    #[characteristic(uuid = characteristic::BATTERY_LEVEL, read, notify, value = 50)]
    pub level: u8,
}
