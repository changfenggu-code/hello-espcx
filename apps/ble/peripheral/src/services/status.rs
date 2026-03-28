//! Status Service — read + write + notify.

use heapless::Vec;
use postcard::to_slice;
use trouble_host::prelude::*;

use hello_ble_common::status;

/// Status Service: Central can read/write a boolean value.
///
/// Demonstrates all three GATT operations: read, write, and notify.
#[gatt_service(uuid = status::SERVICE_UUID)]
pub struct StatusService {
    #[characteristic(uuid = status::UUID, read, write, notify, value = initial_status_value())]
    pub status: Vec<u8, { status::CAPACITY }>,
}

/// Initial value for Status characteristic (false).
pub fn initial_status_value() -> Vec<u8, { status::CAPACITY }> {
    let mut buf = [0u8; status::CAPACITY];
    let used = to_slice(&false, &mut buf).unwrap();
    Vec::from_slice(used).unwrap()
}
