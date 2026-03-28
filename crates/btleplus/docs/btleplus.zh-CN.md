# btleplus

基于 `bluest` 构建的跨平台 BLE 中心库（Central）。

## 概览

`btleplus` 将 BLE 概念分成两层：

- `gap`：发现、过滤、选择、连接生命周期管理
- `gatt`：服务发现缓存、读、写、通知

单设备快速路径：

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

多设备产品路径：

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

## 调用路径

单设备便利路径：

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

多设备选择路径：

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

## 一句话记忆

- `filter.rs`：过滤谁
- `adapter.rs`：去找谁
- `peripheral.rs`：找到了谁
- `selection.rs`：选谁
- `connection.rs`：连上了谁
- `db.rs`：记住它有哪些 GATT 项
- `client.rs`：真正读写它
- `error.rs`：出错怎么说
- `lib.rs`：对外暴露哪些公共接口

## 公开 API

根级导出：

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

## GAP 层

### `ScanFilter`

应用层扫描过滤器，叠加了 OS 级别的服务 UUID 过滤。

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

匹配规则：

- `name_patterns`：前缀或精确匹配，空表示匹配全部
- `addr_patterns`：前缀或精确匹配，空表示匹配全部
- `name` 和 `addr` 之间是 OR 逻辑
- `service_uuids`：透传给底层 OS 扫描过滤
- `manufacturer_company_ids`：扫描阶段的硬过滤
- `with_manufacturer_data(...)`：扫描阶段的硬过滤
- `filter(...)`：针对完整 `PeripheralProperties` 的高级硬过滤

### `Adapter`

系统蓝牙适配器包装器。

关键方法：

```rust
pub async fn default() -> Result<Adapter, BtleplusError>
pub async fn discover(&self, filter: ScanFilter, timeout: Duration) -> Result<Vec<Peripheral>, BtleplusError>
pub async fn find(&self, filter: ScanFilter, timeout: Duration) -> Result<Peripheral, BtleplusError>
pub async fn connect_with_filter(&self, filter: ScanFilter, timeout: Duration) -> Result<Connection, BtleplusError>
```

### `Peripheral`

表示扫描到但尚未连接的设备。

关键方法：

```rust
pub async fn connect(self) -> Result<Connection, BtleplusError>
pub fn properties(&self) -> &PeripheralProperties
pub fn local_name(&self) -> Option<&str>
pub fn id(&self) -> &str
```

`PeripheralProperties` 包含扫描时可见的元数据：

- `id`
- `local_name`
- `advertised_services`
- `manufacturer_data`
- `service_data`
- `rssi`
- `is_connectable`

### `Selector`

当一次扫描返回多台候选设备，而调用方需要在连接前先选一台时，
推荐使用 `Selector`。

常用 builder 方法：

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

规则语义：

- `filter(...)`：发现之后的硬过滤
- `prefer*`：软偏好，只影响排序，且越早链式调用优先级越高
- `select(...)`：返回剩余候选里排名最高的那台
- `rank(...)`：返回完整的排序列表

### `Connection`

表示已连接的 GAP 链路。连接生命周期管理留在这一层。

关键方法：

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

## GATT 层

### `Client`

基于活跃 `Connection` 构建的 GATT 客户端。

关键方法：

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

重要边界：

- connect/disconnect/reconnect/is_connected 属于 `Connection`
- read/write/notify/discovery cache 属于 `Client`

### `GattDatabase`

服务和特征发现结果的本地缓存。

关键方法：

```rust
pub async fn discover(device: &bluest::Device) -> Result<GattDatabase>
pub fn num_services(&self) -> usize
pub fn num_characteristics(&self) -> usize
pub async fn discovered_characteristics(&self) -> Result<impl Stream<Item = Result<bluest::Characteristic>>>
```

## 错误类型

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

## 与 ESP 外设配合的示例

```rust
use std::time::Duration;
use btleplus::{Adapter, ScanFilter, Selector, Uuid, BluetoothUuidExt};

let adapter = Adapter::default().await?;

let filter = ScanFilter::default()
    .with_name_pattern("hello-espcx")
    .with_service_uuid(Uuid::from_u16(0x180F))
    .with_manufacturer_company_id(0xFFFF);

// 先扫描出所有通过扫描期过滤的候选设备
let peripherals = adapter.discover(filter, Duration::from_secs(30)).await?;

// 先定义选择器，链式顺序就是偏好优先级
let selector = Selector::default()
    .prefer_connectable()
    .prefer_strongest_signal();

// 再把选择器作用到候选设备上
let ranked = peripherals.rank_with(&selector)?;

// 如果你要做 CLI / UI，可直接打印完整排序列表
println!("{}", ranked.display_lines());

// 自动取第一名
let peripheral = peripherals.select_with(&selector)?;

let connection = peripheral.connect().await?;
let gatt = connection.into_gatt().await?;

let battery_uuid = Uuid::from_u16(0x2A19);
let level = gatt.read(battery_uuid).await?;
println!("battery={}", level[0]);

let mut stream = gatt.notifications(battery_uuid).await?;
```

## 注意事项

1. 目前仅支持 Windows。crate 受 `#![cfg(windows)]` 保护。
2. `ScanFilter` 负责最强的一层扫描期过滤，包括厂商数据过滤。
3. 当一次扫描返回多台候选设备时，推荐使用 `Selector` 对“已经相关”的候选集做排序和选择。
4. `Client` 会缓存已发现的特征值。如果远端 GATT 布局可能已更改，请调用 `rediscover()`。
5. 自定义结构的类型化读写默认使用 `postcard`，对嵌入式对端更友好。
6. 标准 BLE 特征值通常应直接按规范解析，不要为了统一而额外包一层 `postcard`。
