# hello-ble-central

Windows BLE central app for the ESP32-C6 peripheral in this repository.

## Overview

`hello-ble-central` scans for the peripheral, connects to it, discovers GATT,
then exposes a small business API around battery, device-info, echo, status,
and bulk-transfer flows.

Current layering:

```text
main.rs
  -> connect_session()
  -> BleSession
       -> btleplus::Adapter
       -> btleplus::Peripheral
       -> btleplus::Connection
       -> btleplus::Client
```

`BleSession` is intentionally not a generic BLE wrapper. It is the app-specific
facade for this project’s UUIDs and payload formats.

## Files

- `src/main.rs`: program entry, reconnect loop, runtime behavior
- `src/lib.rs`: `BleSession` and connection helpers
- `tests/hil_real.rs`: real-hardware integration tests

## Connection Flow

`connect_session_with_timeout()` does:

1. Build a `ScanFilter` for the peripheral name and battery service.
2. Open `btleplus::Adapter`.
3. Find a matching `btleplus::Peripheral`.
4. Connect and get a `btleplus::Connection`.
5. Convert that into a `btleplus::Client`.
6. Build `BleSession` with the fixed UUID set from `hello-ble-common`.

In code terms:

```rust
let adapter = Adapter::default().await?;
let peripheral = adapter.find(filter, timeout).await?;
let connection = peripheral.connect().await?;
let gatt = connection.into_gatt().await?;
```

## Core Type

```rust
pub struct BleSession {
    gatt: Client,
    battery_uuid: Uuid,
    manufacturer_uuid: Uuid,
    model_uuid: Uuid,
    firmware_uuid: Uuid,
    software_uuid: Uuid,
    echo_uuid: Uuid,
    status_uuid: Uuid,
    bulk_control_uuid: Uuid,
    bulk_data_uuid: Uuid,
    bulk_stats_uuid: Uuid,
}
```

Important boundary:

- GATT reads/writes/notifications go through `gatt`
- connection lifecycle goes through `gatt.connection()`

So `BleSession::disconnect()` and `BleSession::is_connected()` now delegate to
the underlying `Connection`, not to the GATT client itself.

## BleSession API

Read operations:

- `battery_level() -> Result<u8, Error>`
- `device_info() -> Result<DeviceInfo, Error>`
- `status() -> Result<bool, Error>`
- `read_bulk_stats() -> Result<bulk::BulkStats, Error>`

Write/control operations:

- `set_status(value)`
- `echo(data)`
- `reset_bulk_stats()`
- `start_bulk_stream(total_bytes)`
- `upload_bulk_data(data)`
- `upload_test_pattern(total_bytes)`

Notification/stream operations:

- `notifications(uuid)`
- `receive_bulk_stream(total_bytes, timeout)`

Connection/debug operations:

- `disconnect()`
- `is_connected()`
- `list_characteristics()`

UUID helpers:

- `battery_uuid()`
- `echo_uuid()`
- `bulk_data_uuid()`

## DeviceInfo

```rust
pub struct DeviceInfo {
    pub manufacturer: String,
    pub model: String,
    pub firmware: String,
    pub software: String,
}
```

These values come from the peripheral’s Device Information Service.

## Runtime Behavior

`main.rs` keeps a reconnect loop alive:

1. connect
2. log discovered characteristics
3. read device info
4. read battery
5. read/write status
6. send echo payload
7. subscribe to battery notifications
8. periodically re-read battery and watch for disconnect

The monitor loop uses `tokio::select!` to wait on:

- battery notifications
- periodic timer ticks

## Bulk Transfer Notes

Two directions exist:

- Central -> Peripheral upload:
  `upload_bulk_data()` or `upload_test_pattern()`
- Peripheral -> Central notify stream:
  `start_bulk_stream()` then `receive_bulk_stream()`

Stats are validated with `read_bulk_stats()` and reset with
`reset_bulk_stats()`.

## Tests

`tests/hil_real.rs` contains real-hardware tests for:

- end-to-end battery/status/echo validation
- bulk upload verification
- bulk notify-stream verification
- throughput reporting

These tests require a real ESP32-C6 peripheral running the matching firmware.
