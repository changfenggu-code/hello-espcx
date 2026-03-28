# btleplus

基于 `bluest` 构建的跨平台 BLE 中心库（Central）。

## 概述

`btleplus` 将 BLE 概念分为两层：

- `gap`：发现设备、过滤、连接生命周期管理
- `gatt`：服务发现缓存、读、写、通知

推荐流程：

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

如果你想在一个扫描时间窗内收集所有匹配设备，可以使用：

```rust
let peripherals = adapter.discover(filter, Duration::from_secs(30)).await?;
```

## 调用路径

应用代码构建连接时：

```rust
let adapter = Adapter::default().await?;
let peripheral = adapter.find(filter, timeout).await?;
let connection = peripheral.connect().await?;
let gatt = connection.into_gatt().await?;
```

内部流程如下：

1. `Adapter::default()`
   打开系统默认蓝牙适配器。

2. `Adapter::find(filter, timeout)`
   委托给 `find_ref`，再委托给内部 `scan_for_target(...)`。

`Adapter::discover(filter, timeout)` 使用同样的过滤规则，但会在整个扫描时间窗内持续收集所有匹配到的外设。

3. `scan_for_target(...)`
   以 `filter.service_uuids` 作为 OS 级别过滤器开始扫描。

4. `ScanFilter::matches(name, address)`
   对 `name_patterns` 和 `addr_patterns` 执行应用层过滤。

5. `Peripheral::new(...)`
   将发现的 `bluest` device 和扫描时的属性封装成 `Peripheral`。

6. `Peripheral::connect()`
   调用底层适配器连接，返回 `Connection`。

7. `Connection::into_gatt()`
   将活动链路移交给 GATT 层，构建 `Client`。

8. `GattDatabase::discover(device)`
   发现服务和特征值，并缓存供后续 UUID 查询。

9. `Client`
   处理读、写、类型化读写、通知以及特征值枚举。

简图：

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

## 一句话记忆

- `filter.rs`：过滤谁
- `adapter.rs`：去找谁
- `peripheral.rs`：找到了谁
- `connection.rs`：连上了谁
- `db.rs`：记住它有哪些 GATT 项
- `client.rs`：真正读写它
- `error.rs`：出错怎么说
- `lib.rs`：对外暴露的公共接口

## 公开 API

根级导出：

```rust
pub use bluest::Uuid;
pub use bluest::btuuid::BluetoothUuidExt;
pub use error::BtleplusError;
pub use gap::{Adapter, Connection, Peripheral, PeripheralProperties, ScanFilter};
pub use gatt::{Client, GattDatabase, Result};
```

## GAP 层

### `ScanFilter`

应用层扫描过滤器，带 OS 级别的服务 UUID 过滤。

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

匹配规则：

- `name_patterns`：前缀匹配或精确匹配，空表示匹配所有
- `addr_patterns`：前缀匹配或精确匹配，空表示匹配所有
- name/address 之间是 OR 逻辑
- `service_uuids`：透传给操作系统扫描

### `Adapter`

系统蓝牙适配器的包装器。

关键方法：

```rust
pub async fn default() -> Result<Adapter, BtleplusError>
pub async fn discover(&self, filter: ScanFilter, timeout: Duration) -> Result<Vec<Peripheral>, BtleplusError>
pub async fn find(&self, filter: ScanFilter, timeout: Duration) -> Result<Peripheral, BtleplusError>
pub async fn connect_with_filter(&self, filter: ScanFilter, timeout: Duration) -> Result<Connection, BtleplusError>
```

### `Peripheral`

表示连接前已发现的设备。

关键方法：

```rust
pub async fn connect(self) -> Result<Connection, BtleplusError>
pub fn properties(&self) -> &PeripheralProperties
pub fn local_name(&self) -> Option<&str>
pub fn id(&self) -> &str
```

`PeripheralProperties` 包含扫描时的元数据：

- `id`：设备标识符
- `local_name`：广播的本地名称
- `advertised_services`：广播的服务列表
- `rssi`：信号强度（dBm）
- `is_connectable`：设备是否报告为可连接

### `Connection`

表示已连接的 GAP 链路。连接生命周期管理在此层。

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

基于活动 `Connection` 构建的 GATT 客户端。

关键方法：

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

重要边界：

- connect/disconnect/reconnect/is_connected 属于 `Connection`
- read/write/notify/discovery cache 属于 `Client`

### `GattDatabase`

服务和特征值发现的缓存结果。

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
    Bluetooth(String),          // 蓝牙子系统错误
    DeviceNotFound(String),      // 扫描未找到设备
    ConnectionFailed(String),    // 连接设备失败
    Io(std::io::Error),         // IO 错误
    Timeout,                     // 操作超时
    NotConnected,                // 需要连接但未连接
    InvalidOperation(String),    // 无效操作（如找不到特征值）
    Deserialize(String),         // 反序列化错误
    Serialize(String),           // 序列化错误
}
```

## 与 ESP 外设配合的示例

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

## 注意事项

1. 目前仅支持 Windows。crate 以 `#![cfg(windows)]` 保护。
2. `ScanFilter` 将 OS 级别的服务 UUID 过滤与应用层的名称/地址匹配混合使用。
3. `Client` 会缓存已发现的特征值。如果远程 GATT 布局可能已更改，请调用 `rediscover()`。
4. 类型化读写使用 `postcard`，使 API 对嵌入式对端保持友好。
