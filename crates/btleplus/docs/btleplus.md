# btleplus

Cross-platform BLE central library built on top of `bluest`.

## Overview

`btleplus` exposes BLE concepts in two layers:

- `gap`: discovery, filtering, selection, connection lifecycle
- `gatt`: service discovery cache, read, write, notifications

Single-device quick path:

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

Multi-device product path:

```rust
use std::time::Duration;
use btleplus::{
    Adapter, PeripheralSelectionExt, ScanFilter, Selector, Uuid, BluetoothUuidExt,
};

let adapter = Adapter::default().await?;

let filter = ScanFilter::default()
    .with_name_pattern("hello-espcx")
    .with_service_uuid(Uuid::from_u16(0x180F));

let peripherals = adapter.discover(filter, Duration::from_secs(30)).await?;
let selector = Selector::default()
    .prefer_connectable()
    .prefer_strongest_signal();
let peripheral = peripherals.select_with(&selector)?;

let connection = peripheral.connect().await?;
let gatt = connection.into_gatt().await?;
```

## Call Path

Single-device convenience flow:

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

Multi-device selection flow:

```text
Adapter::default
  -> Adapter::discover
  -> scan_for_targets
  -> ScanFilter::matches
  -> Peripheral[]
  -> Selector::select
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
- `selection.rs`: choose who
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
pub use gap::{
    Adapter,
    Connection,
    ManufacturerData,
    Peripheral,
    PeripheralProperties,
    ScanFilter,
    Selector,
};
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
    .with_manufacturer_company_id(0xFFFF)
    .with_scan_interval_secs(3);
```

Matching rules:

- `name_patterns`: prefix or exact match, empty means match all
- `addr_patterns`: prefix or exact match, empty means match all
- name/address use OR logic
- `service_uuids`: passed down to OS scanning
- `manufacturer_company_ids`: hard filter during scanning
- `with_manufacturer_data(...)`: hard filter during scanning
- `filter(...)`: advanced hard filter over the full `PeripheralProperties` snapshot

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
- `manufacturer_data`
- `service_data`
- `rssi`
- `is_connectable`

### `Selector`

Used when one scan window may return multiple matching peripherals and the
caller must choose one before connecting.

Common builder methods:

```rust
pub fn prefer_connectable(self) -> Self
pub fn prefer_strongest_signal(self) -> Self
pub fn prefer_id(self, id: impl Into<String>) -> Self
pub fn prefer_local_name(self, name: impl Into<String>) -> Self
pub fn prefer_manufacturer_company_id(self, company_id: u16) -> Self
pub fn prefer_manufacturer_data<F>(self, predicate: F) -> Self
pub fn filter<F>(self, predicate: F) -> Self
pub fn select(&self, peripherals: &[Peripheral]) -> Result<Peripheral, BtleplusError>
pub fn rank(&self, peripherals: &[Peripheral]) -> Result<Vec<Peripheral>, BtleplusError>
```

Rule semantics:

- `filter(...)`: post-discovery hard elimination
- `prefer*`: soft ranking, and earlier chained calls have higher priority
- `select(...)`: returns the highest-ranked surviving peripheral
- `rank(...)`: returns the full ranked list

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
pub async fn read_to_string(&self, uuid: Uuid) -> Result<String>
pub async fn read_to<T>(&self, uuid: Uuid) -> Result<T>
pub async fn write(&self, uuid: Uuid, data: &[u8], with_response: bool) -> Result<()>
pub async fn write_from<T>(&self, uuid: Uuid, value: &T, with_response: bool) -> Result<()>
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
    SelectionFailed(String),
    Deserialize(String),
    Serialize(String),
}
```

## Example With ESP Peripheral

```rust
use std::time::Duration;
use btleplus::{Adapter, ScanFilter, Selector, Uuid, BluetoothUuidExt};

let adapter = Adapter::default().await?;

let filter = ScanFilter::default()
    .with_name_pattern("hello-espcx")
    .with_service_uuid(Uuid::from_u16(0x180F))
    .with_manufacturer_company_id(0xFFFF);

// Discover all candidate devices that pass the scan-time filters.
let peripherals = adapter.discover(filter, Duration::from_secs(30)).await?;

// Build a selector, then apply it to the discovered peripherals.
let selector = Selector::default()
    .prefer_connectable()
    .prefer_strongest_signal();

// Rank the surviving candidates.
let ranked = peripherals.rank_with(&selector)?;

// Inspect the full ranked device list if you want to drive a CLI or UI.
println!("{}", ranked.display_lines());

// Auto-select the first-ranked candidate.
let peripheral = peripherals.select_with(&selector)?;

let connection = peripheral.connect().await?;
let gatt = connection.into_gatt().await?;

let battery_uuid = Uuid::from_u16(0x2A19);
let level = gatt.read(battery_uuid).await?;
println!("battery={}", level[0]);

let mut stream = gatt.notifications(battery_uuid).await?;
```

## Notes

1. Windows only. The crate is guarded by `#![cfg(windows)]`.
2. `ScanFilter` owns the strongest scan-time filters, including manufacturer-data-based elimination.
3. `Selector` is the recommended way to rank and choose among already-relevant candidates.
4. `Client` caches discovered characteristics. Call `rediscover()` if the remote GATT layout may have changed.
5. Typed reads/writes use `postcard` for custom payload structs.
6. Standard BLE characteristic values should usually be parsed directly instead of being wrapped in `postcard`.
