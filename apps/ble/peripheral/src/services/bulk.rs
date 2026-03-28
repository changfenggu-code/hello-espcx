//! Bulk Service — control + data transfer + stats.

use heapless::Vec;
use postcard::to_slice;
use trouble_host::prelude::*;

use hello_ble_common::bulk;

/// Bulk Service: large data transfer with control commands and statistics.
///
/// Supports two directions:
/// - Upload (Central → Peripheral): write data chunks via bulk_data
/// - Download (Peripheral → Central): start stream, receive notify chunks
#[gatt_service(uuid = bulk::SERVICE_UUID)]
pub struct BulkService {
    /// Control characteristic: write commands (Idle / ResetStats / StartStream).
    #[characteristic(uuid = bulk::CONTROL_UUID, write, read, value = initial_bulk_control_value())]
    pub control: Vec<u8, { bulk::CONTROL_CAPACITY }>,
    /// Data characteristic: bidirectional (write = upload, notify = download).
    #[characteristic(uuid = bulk::CHUNK_UUID, write, write_without_response, notify, value = Vec::new())]
    pub data: Vec<u8, { bulk::CHUNK_SIZE }>,
    /// Stats characteristic: read-only, reflects rx/tx byte counters.
    #[characteristic(uuid = bulk::STATS_UUID, read, value = initial_bulk_stats_value())]
    pub stats: Vec<u8, { bulk::STATS_CAPACITY }>,
}

/// Initial value for bulk control (Idle).
pub fn initial_bulk_control_value() -> Vec<u8, { bulk::CONTROL_CAPACITY }> {
    let mut buf = [0u8; bulk::CONTROL_CAPACITY];
    let used = to_slice(&bulk::BulkControlCommand::Idle, &mut buf).unwrap();
    Vec::from_slice(used).unwrap()
}

/// Initial value for bulk stats (rx=0, tx=0).
pub fn initial_bulk_stats_value() -> Vec<u8, { bulk::STATS_CAPACITY }> {
    let mut buf = [0u8; bulk::STATS_CAPACITY];
    let used = to_slice(&bulk::BulkStats::default(), &mut buf).unwrap();
    Vec::from_slice(used).unwrap()
}
