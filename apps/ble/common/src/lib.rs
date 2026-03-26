#![no_std]

//! BLE Common Types for hello-espcx
//!
//! This crate defines shared constants and types for the BLE peripheral and central.
//!
//! ## Standard Services (UUID16)
//!
//! | Service | Purpose | Operations |
//! |---------|---------|------------|
//! | Battery | Battery level monitoring | `read` + `notify` |
//! | Device Info | Device information | `read` (manufacturer, model, firmware, software) |
//! | Heart Rate | Heart rate monitoring | `read` + `notify` + `write` |
//!
//! ## Custom Services (UUID128)
//!
//! | Service | Purpose | Operations |
//! |---------|---------|------------|
//! | Echo | Ping/pong test | `write` → `notify` |
//! | Status | State synchronization | `read` + `write` + `notify` |
//! | Bulk | Large data transfer | `write` + streaming `notify` + stats |

use serde::{Deserialize, Serialize};

pub const PERIPHERAL_NAME: &str = "hello-espcx";
pub const PERIPHERAL_ADDRESS: [u8; 6] = [0xff, 0x8f, 0x1a, 0x05, 0xe4, 0xff];

// === BLE Constants ===
pub const BLE_MTU: usize = 255;
pub const ATT_PAYLOAD_MAX: usize = BLE_MTU - 3; // 252 bytes per ATT packet

// === Battery Service (Standard BLE) ===
// Purpose: Demonstrate basic `read` + `notify`
// - Central reads current battery level (0-100%)
// - Peripheral periodically notifies battery updates
pub const SERVICE_BATTERY_UUID16: u16 = 0x180F;
pub const BATTERY_LEVEL_UUID16: u16 = 0x2A19;

// === Device Information Service (Standard BLE) ===
// Purpose: Demonstrate `read` for static device information
// - Central reads manufacturer name, model, firmware version, etc.
pub const SERVICE_DEVICE_INFO_UUID16: u16 = 0x180A;
pub const DEVICE_INFO_MANUFACTURER_NAME_UUID16: u16 = 0x2A29;
pub const DEVICE_INFO_MODEL_NUMBER_UUID16: u16 = 0x2A24;
pub const DEVICE_INFO_FIRMWARE_REVISION_UUID16: u16 = 0x2A26;
pub const DEVICE_INFO_SOFTWARE_REVISION_UUID16: u16 = 0x2A28;
/// Max string length for Device Info characteristics
pub const DEVICE_INFO_STRING_CAPACITY: usize = 30;

// === Heart Rate Service (Standard BLE) ===
// Purpose: Demonstrate `notify` + `write` control
// - Heart Rate Measurement: peripheral notifies heart rate value
// - Heart Rate Control Point: central writes to control peripheral behavior
pub const SERVICE_HEART_RATE_UUID16: u16 = 0x180D;
pub const HEART_RATE_MEASUREMENT_UUID16: u16 = 0x2A37;
pub const HEART_RATE_CONTROL_UUID16: u16 = 0x2A39;
/// Max size for Heart Rate Measurement characteristic (flags + heart rate value)
pub const HEART_RATE_MEASUREMENT_CAPACITY: usize = 3;
/// Max size for Heart Rate Control Point (1 byte command)
pub const HEART_RATE_CONTROL_CAPACITY: usize = 1;

// === Echo Service (Custom) ===
// Purpose: Test `write` → `notify` round-trip
// - Central writes data to echo characteristic
// - Peripheral immediately notifies the same data back
// - Verifies bidirectional data integrity
pub const SERVICE_ECHO_UUID: u128 = 0x4088_13df_5dd4_1f87_ec11_cdb0_0110_1001;
pub const ECHO_UUID: u128 = 0x4088_13df_5dd4_1f87_ec11_cdb0_0110_1002;
pub const ECHO_CAPACITY: usize = ATT_PAYLOAD_MAX;

// === Status Service (Custom) ===
// Purpose: Test `read` + `write` + `notify` for state sync
// - Central reads current status
// - Central writes new status
// - Peripheral notifies status changes to all subscribers
pub const SERVICE_STATUS_UUID: u128 = 0x4088_13df_5dd4_1f87_ec11_cdb0_0110_2001;
pub const STATUS_UUID: u128 = 0x4088_13df_5dd4_1f87_ec11_cdb0_0110_2002;
pub const STATUS_CAPACITY: usize = 4;

// === Bulk Service (Custom) ===
// Purpose: Test large data transfer with `write` + streaming `notify` + statistics
// - Central sends control commands (StartStream, ResetStats)
// - Peripheral streams large data via notifications
// - Central reads transfer statistics for verification
pub const SERVICE_BULK_UUID: u128 = 0x4088_13df_5dd4_1f87_ec11_cdb0_0110_3001;
pub const BULK_CONTROL_UUID: u128 = 0x4088_13df_5dd4_1f87_ec11_cdb0_0110_3002;
pub const BULK_CHUNK_UUID: u128 = 0x4088_13df_5dd4_1f87_ec11_cdb0_0110_3003;
pub const BULK_STATS_UUID: u128 = 0x4088_13df_5dd4_1f87_ec11_cdb0_0110_3004;
pub const BULK_CONTROL_CAPACITY: usize = 8;
pub const BULK_CHUNK_SIZE: usize = ATT_PAYLOAD_MAX;
pub const BULK_STATS_CAPACITY: usize = 16;

// Bulk Service types
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BulkControlCommand {
    Idle,
    ResetStats,
    StartStream { total_bytes: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BulkStats {
    /// Bytes received from Central (upload direction)
    pub rx_bytes: u32,
    /// Bytes sent to Central (notify direction)
    pub tx_bytes: u32,
}

