# btleplus

Cross-platform BLE central library built on top of `bluest`.

## Overview

`btleplus` exposes BLE concepts in two layers:

- `gap`: discovery, filtering, connection lifecycle
- `gatt`: service discovery cache, read, write, notifications

Recommended flow:

```rust
use std::time::Duration;
use btleplus::{Adapter, ScanFilter, Uuid, BluetoothUuidExt};

let adapter = Adapter::default().await?;

let filter = ScanFilter::default()
    .with_name_pattern("hello-espcx")
    .with_service_uuid(Uuid::from_u16(0x180F));

let peripheral = adapter.find(filter, Duration::from_secs(30)).await?;
let connection = peripheral.connect().await?;
let gatt = connection.into_gatt().await?;
```

To collect all matching peripherals during a scan window:

```rust
let peripherals = adapter.discover(filter, Duration::from_secs(30)).await?;
```

## Call Path

When application code builds a connection with:

```rust
let adapter = Adapter::default().await?;
let peripheral = adapter.find(filter, timeout).await?;
let connection = peripheral.connect().await?;
let gatt = connection.into_gatt().await?;
```

the internal flow is:

1. `Adapter::default()`
   Opens the system default Bluetooth adapter.

2. `Adapter::find(filter, timeout)`
   Delegates to `find_ref`, then to the internal `scan_for_target(...)`.

`Adapter::discover(filter, timeout)` follows the same filtering rules, but keeps
collecting matching peripherals until the scan window ends.

3. `scan_for_target(...)`
   Starts scanning with `filter.service_uuids` as the OS-level filter.

4. `ScanFilter::matches(name, address)`
   Applies app-level filtering for `name_patterns` and `addr_patterns`.

5. `Peripheral::new(...)`
   Wraps the discovered `bluest` device plus scan-time properties into a
   `Peripheral`.

6. `Peripheral::connect()`
   Calls the underlying adapter to connect and returns a `Connection`.

7. `Connection::into_gatt()`
   Hands the live link to the GATT layer and builds a `Client`.

8. `GattDatabase::discover(device)`
   Discovers services and characteristics and caches them for later UUID lookup.

9. `Client`
   Handles read, write, typed read/write, notifications, and characteristic
   enumeration.

In short:

```text
Adapter::default
  -> Adapter::find
  -> scan_for_target
  -> ScanFilter::matches
  -> Peripheral
  -> Peripheral::connect
  -> Connection
  -> Connection::into_gatt
  -> GattDatabase::discover
  -> Client
```

## One-line Memory Map

- `filter.rs`: filter who
- `adapter.rs`: go find who
- `peripheral.rs`: found who
- `connection.rs`: connected to who
- `db.rs`: remember which GATT items it has
- `client.rs`: actually read and write it
- `error.rs`: describe failures
- `lib.rs`: expose the public surface

## Public API

Root exports:

```rust
pub use bluest::Uuid;
pub use bluest::btuuid::BluetoothUuidExt;
pub use error::BtleplusError;
pub use gap::{Adapter, Connection, Peripheral, PeripheralProperties, ScanFilter};
pub use gatt::{Client, GattDatabase, Result};
```

## GAP

### `ScanFilter`

Application-level scan filter with OS-level service UUID filtering.

```rust
let filter = ScanFilter::default()
    .with_name_pattern("hello")
    .with_name_patterns(["device-a", "device-b"])
    .with_addr_pattern("ff8f1a")
    .with_addr_patterns(["001122", "aabbcc"])
    .with_service_uuid(Uuid::from_u16(0x180F))
    .with_service_uuids([Uuid::from_u16(0x180F), Uuid::from_u16(0x180D)])
    .with_scan_interval_secs(3);
```

Matching rules:

- `name_patterns`: prefix or exact match, empty means match all
- `addr_patterns`: prefix or exact match, empty means match all
- name/address use OR logic
- `service_uuids`: passed down to OS scanning

### `Adapter`

System Bluetooth adapter wrapper.

Key methods:

```rust
pub async fn default() -> Result<Adapter, BtleplusError>
pub async fn discover(&self, filter: ScanFilter, timeout: Duration) -> Result<Vec<Peripheral>, BtleplusError>
pub async fn find(&self, filter: ScanFilter, timeout: Duration) -> Result<Peripheral, BtleplusError>
pub async fn connect_with_filter(&self, filter: ScanFilter, timeout: Duration) -> Result<Connection, BtleplusError>
```

### `Peripheral`

Represents a discovered device before connection.

Key methods:

```rust
pub async fn connect(self) -> Result<Connection, BtleplusError>
pub fn properties(&self) -> &PeripheralProperties
pub fn local_name(&self) -> Option<&str>
pub fn id(&self) -> &str
```

`PeripheralProperties` contains scan-time metadata:

- `id`
- `local_name`
- `advertised_services`
- `rssi`
- `is_connectable`

### `Connection`

Represents the connected GAP link. Connection lifecycle stays here.

Key methods:

```rust
pub async fn connect(name: &str, timeout: Duration) -> Result<Connection, BtleplusError>
pub async fn connect_by_address(address: &str, timeout: Duration) -> Result<Connection, BtleplusError>
pub async fn connect_by_service(uuid: Uuid, timeout: Duration) -> Result<Connection, BtleplusError>
pub async fn connect_with_filter(filter: ScanFilter, timeout: Duration) -> Result<Connection, BtleplusError>
pub async fn into_gatt(self) -> Result<Client, BtleplusError>
pub async fn disconnect(&self) -> Result<(), BtleplusError>
pub async fn reconnect(&self) -> Result<(), BtleplusError>
pub async fn is_connected(&self) -> bool
pub fn peripheral(&self) -> &PeripheralProperties
```

## GATT

### `Client`

GATT client built from a live `Connection`.

Key methods:

```rust
pub fn connection(&self) -> &Connection
pub fn into_connection(self) -> Connection
pub fn database(&self) -> &GattDatabase
pub async fn rediscover(&mut self) -> Result<()>
pub async fn read(&self, uuid: Uuid) -> Result<Vec<u8>>
pub async fn read_string(&self, uuid: Uuid) -> Result<String>
pub async fn read_typed<T>(&self, uuid: Uuid) -> Result<T>
pub async fn write(&self, uuid: Uuid, data: &[u8], with_response: bool) -> Result<()>
pub async fn write_typed<T>(&self, uuid: Uuid, value: &T, with_response: bool) -> Result<()>
pub async fn notifications(&self, uuid: Uuid) -> Result<impl Stream<Item = Result<Vec<u8>>> + '_>
pub async fn discovered_characteristics(&self) -> Result<impl Stream<Item = Result<bluest::Characteristic>>>
pub fn num_services(&self) -> usize
pub fn num_characteristics(&self) -> usize
```

Important boundary:

- connect/disconnect/reconnect/is_connected belong to `Connection`
- read/write/notify/discovery cache belong to `Client`

### `GattDatabase`

Cached result of service/characteristic discovery.

Key methods:

```rust
pub async fn discover(device: &bluest::Device) -> Result<GattDatabase>
pub fn num_services(&self) -> usize
pub fn num_characteristics(&self) -> usize
pub async fn discovered_characteristics(&self) -> Result<impl Stream<Item = Result<bluest::Characteristic>>>
```

## Error Type

```rust
pub enum BtleplusError {
    Bluetooth(String),
    DeviceNotFound(String),
    ConnectionFailed(String),
    Io(std::io::Error),
    Timeout,
    NotConnected,
    InvalidOperation(String),
    Deserialize(String),
    Serialize(String),
}
```

## Example With ESP Peripheral

```rust
use std::time::Duration;
use btleplus::{Adapter, ScanFilter, Uuid, BluetoothUuidExt};

let adapter = Adapter::default().await?;

let filter = ScanFilter::default()
    .with_name_pattern("hello-espcx")
    .with_service_uuid(Uuid::from_u16(0x180F));

let peripheral = adapter.find(filter, Duration::from_secs(30)).await?;
let connection = peripheral.connect().await?;
let gatt = connection.into_gatt().await?;

let battery_uuid = Uuid::from_u16(0x2A19);
let level = gatt.read(battery_uuid).await?;

let mut stream = gatt.notifications(battery_uuid).await?;
```

## Notes

1. Windows only. The crate is guarded by `#![cfg(windows)]`.
2. `ScanFilter` mixes OS-level service UUID filtering with app-level name/address matching.
3. `Client` caches discovered characteristics. Call `rediscover()` if the remote GATT layout may have changed.
4. Typed reads/writes use `postcard`, which keeps the API friendly for embedded peers.
