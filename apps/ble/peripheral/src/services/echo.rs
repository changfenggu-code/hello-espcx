//! Echo Service — write → notify the same payload back.

use heapless::Vec;
use trouble_host::prelude::*;

use hello_ble_common::echo;

/// Echo Service: Central writes data, Peripheral immediately notifies the same data back.
///
/// Used for bidirectional data integrity verification.
#[gatt_service(uuid = echo::SERVICE_UUID)]
pub struct EchoService {
    #[characteristic(uuid = echo::UUID, write, notify, value = Vec::new())]
    pub echo: Vec<u8, { echo::CAPACITY }>,
}
