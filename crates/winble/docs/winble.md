# winble — Windows BLE Central Library

## 概述

`winble` 是运行在 Windows 上的 BLE Central（主机）封装库，底层基于 `bluest`（WinRT API），提供 async/await 风格的 GATT 操作接口。

**平台限制**：仅支持 Windows（通过 `#![cfg(windows)]` 条件编译）。

```
┌────────────────────────────────────────────────────────┐
│  Your Application Code (hello-ble-central)             │
├────────────────────────────────────────────────────────┤
│  winble (this crate)                                   │
│  ├── Session       连接 + GATT 读/写/通知               │
│  ├── ScanFilter    扫描过滤器                           │
│  ├── WinbleError   错误类型                             │
│  └── bluest        WinRT BLE API 封装                  │
├────────────────────────────────────────────────────────┤
│  Windows Bluetooth Driver Stack                        │
│  (WinRT → Bluetooth API → 蓝牙适配器)                   │
└────────────────────────────────────────────────────────┘
```

## 依赖

```toml
[dependencies]
winble = { path = "crates/winble" }
tokio = { version = "1", features = ["rt", "rt-multi-thread", "sync", "time"] }
serde = { version = "1", features = ["derive"] }
postcard = { version = "1", default-features = false, features = ["alloc"] }
futures-util = "0.3"
```

## 快速开始

```rust
use winble::{Session, ScanFilter, Uuid, BluetoothUuidExt};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. 创建扫描过滤器
    let filter = ScanFilter::default()
        .with_name_pattern("hello-espcx")
        .with_service_uuid(Uuid::from_u16(0x180F)); // Battery Service

    // 2. 扫描并连接（超时 30 秒）
    let session = Session::connect_with_filter(filter, Duration::from_secs(30)).await?;

    // 3. 读取 Battery Level 特征
    let battery_uuid = Uuid::from_u16(0x2A19);
    let data = session.read(battery_uuid).await?;
    println!("Battery: {}%", data[0]);

    // 4. 订阅通知
    let mut stream = session.notifications(battery_uuid).await?;
    while let Some(result) = stream.next().await {
        println!("Battery update: {}%", result?[0]);
    }

    Ok(())
}
```

## API 参考

### 模块导出

```rust
pub use bluest::Uuid;                    // UUID 类型
pub use bluest::btuuid::BluetoothUuidExt; // UUID 构造扩展（from_u16, from_u128 等）
pub use error::WinbleError;              // 错误类型
pub use gatt::{ScanFilter, Session, Result}; // 核心 API
```

---

### `ScanFilter` — 扫描过滤器

扫描时对发现的设备进行过滤。支持链式调用。

```rust
let filter = ScanFilter::default()
    .with_name_pattern("hello")          // 精确或前缀匹配
    .with_name_patterns(["device1", "device2"]) // 多个 name，OR 逻辑
    .with_addr_pattern("ff8f1a")        // 地址前缀匹配
    .with_addr_patterns(["001122", "aabbcc"])
    .with_service_uuid(Uuid::from_u16(0x180F))  // OS 级别过滤
    .with_service_uuids([Uuid::from_u16(0x180F), Uuid::from_u16(0x180D)])
    .with_scan_interval_secs(3);          // 扫描间隔，默认 2 秒
```

**过滤逻辑**：

| 字段 | 匹配规则 |
|---|---|
| `name_patterns` | 前缀匹配或精确匹配；空 = 匹配所有 |
| `addr_patterns` | 前缀匹配或精确匹配；空 = 匹配所有 |
| `service_uuids` | OS 级别过滤，扫描时直接丢弃不包含该 Service 的广播包 |
| `scan_interval_secs` | 每次扫描迭代之间的等待间隔 |

`name` 和 `addr` 之间是 **OR 逻辑**：name 匹配 OR addr 匹配即为匹配。

---

### `Session` — GATT 连接会话

#### 连接

```rust
// 按设备名连接（支持前缀匹配）
pub async fn connect(name: &str, timeout: Duration) -> Result<Self>

// 按蓝牙地址连接（支持前缀匹配）
pub async fn connect_by_address(address: &str, timeout: Duration) -> Result<Self>

// 按广播的服务 UUID 连接
pub async fn connect_by_service(uuid: Uuid, timeout: Duration) -> Result<Self>

// 自定义过滤器连接
pub async fn connect_with_filter(filter: ScanFilter, timeout: Duration) -> Result<Self>

// 断开后重连（使用之前保存的 device）
pub async fn reconnect(&mut self) -> Result<()>
```

**完整连接流程**：

```
Adapter::default()          获取蓝牙适配器
        │
        ▼
scan()                      启动 BLE 扫描
        │
        ▼
matches()                   name/addr 应用级别过滤
        │
        ▼
open_device()               打开设备对象
        │
        ▼
connect_device()            建立 GATT 连接
        │
        ▼
discover_services()         发现所有服务和特征
        │
        ▼
Session { adapter, device, services, characteristics }
```

**超时行为**：超时后返回 `WinbleError::Timeout`。

#### 读

```rust
// 读取原始字节
pub async fn read(&self, uuid: Uuid) -> Result<Vec<u8>>

// 读取并转为 UTF-8 字符串
pub async fn read_string(&self, uuid: Uuid) -> Result<String>

// 读取并用 postcard 反序列化为 Rust 类型
pub async fn read_typed<T: serde::de::DeserializeOwned>(&self, uuid: Uuid) -> Result<T>
```

示例：

```rust
// 读原始字节
let bytes = session.read(battery_uuid).await?;
let level = bytes[0];

// 读字符串
let manufacturer = session.read_string(manufacturer_uuid).await?;

// 读自定义类型（需要 serde）
#[derive(Serialize, Deserialize, Debug)]
struct BulkStats {
    pub rx_bytes: u32,
    pub tx_bytes: u32,
}
let stats = session.read_typed::<BulkStats>(bulk_stats_uuid).await?;
```

#### 写

```rust
// 写入原始字节
pub async fn write(&self, uuid: Uuid, data: &[u8], with_response: bool) -> Result<()>

// 序列化后写入
pub async fn write_typed<T: serde::Serialize>(
    &self, uuid: Uuid, value: &T, with_response: bool
) -> Result<()>
```

参数 `with_response`：

| 值 | GATT 操作 | 行为 |
|---|---|---|
| `true` | Write Request | 等服务器 ACK，可靠但慢 |
| `false` | Write Command | 发送后立即返回，不可靠但快 |

示例：

```rust
// 写原始字节（无响应）
session.write(echo_uuid, b"Hello", false).await?;

// 写原始字节（有响应）
session.write(echo_uuid, b"Hello", true).await?;

// 写序列化数据
session.write_typed(bulk_control_uuid, &command, true).await?;
```

#### 通知订阅

```rust
// 订阅特征的通知，返回 Stream
pub async fn notifications(&self, uuid: Uuid) -> Result<impl Stream<Item = Result<Vec<u8>>>>
```

使用方式：

```rust
use futures_util::StreamExt;

let mut stream = session.notifications(battery_uuid).await?;
while let Some(result) = stream.next().await {
    let data = result?;           // Result<Vec<u8>>
    println!("{:?}", data);
}
```

> 底层自动处理 CCCD（Client Characteristic Configuration Descriptor）写入。

#### 其他

```rust
// 检查连接状态
pub async fn is_connected(&self) -> bool

// 断开连接
pub async fn disconnect(&self) -> Result<()>

// 获取所有发现特征的 UUID（调试用）
pub async fn discovered_characteristics(&self) -> Result<impl Stream<Item = Result<Characteristic>>>
```

---

### `WinbleError` — 错误类型

```rust
pub enum WinbleError {
    Bluetooth(String),        // 蓝牙子系统错误
    DeviceNotFound(String),   // 扫描超时找不到设备
    ConnectionFailed(String),  // 连接失败
    Io(#[from] std::io::Error), // IO 错误
    Timeout,                 // 操作超时
    NotConnected,            // 未连接
    InvalidOperation(String), // 特征未找到等
    Deserialize(String),     // postcard 反序列化失败
    Serialize(String),        // postcard 序列化失败
}
```

---

## 实际使用示例

完整示例来自 `apps/ble/central/src/main.rs`：

```rust
use hello_ble_central::{connect_session, BleSession};
use futures_util::StreamExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 自动扫描 "hello-espcx" 并连接
    let mut session = connect_session().await?;

    // 读取设备信息
    let info = session.device_info().await?;
    println!("Manufacturer: {}", info.manufacturer);

    // 读取电量
    let level = session.battery_level().await?;
    println!("Battery: {}%", level);

    // 读/写状态
    let status = session.status().await?;
    session.set_status(true).await?;

    // Echo 测试
    session.echo(b"Hello, BLE!").await?;

    // 订阅电量通知
    let mut battery_stream = session.notifications(session.battery_uuid()).await?;

    loop {
        tokio::select! {
            notif = battery_stream.next() => {
                if let Some(Ok(n)) = notif {
                    println!("Battery: {}%", n[0]);
                }
            }
        }
    }
}
```

---

## 与 ESP32 Peripheral 配合

假设 ESP32 Peripheral 暴露以下服务：

| Service | Characteristic | 操作 |
|---|---|---|
| Battery Service (0x180F) | Battery Level (0x2A19) | read, notify |
| Heart Rate Service (0x180D) | Heart Rate Measurement (0x2A37) | notify |
| Custom Echo Service | Echo (UUID-128) | write → notify |
| Custom Status Service | Status (UUID-128) | read, write, notify |
| Custom Bulk Service | Control, Data, Stats | write, notify, read |

Central 端调用：

```rust
use winble::{Session, ScanFilter, Uuid, BluetoothUuidExt};

// 连接
let session = Session::connect("hello-espcx", Duration::from_secs(30)).await?;

// 读电量
let level = session.read(Uuid::from_u16(0x2A19)).await?;

// 写状态（序列化）
let cmd = BulkControlCommand::StartStream { total_bytes: 10240 };
session.write_typed(bulk_control_uuid, &cmd, true).await?;

// 订阅 bulk 数据流
let mut stream = session.notifications(bulk_data_uuid).await?;
while let Some(result) = stream.next().await {
    let chunk = result?;
    // 处理数据...
}
```

---

## UUID 构造方式

使用 `bluest::btuuid::BluetoothUuidExt` 扩展：

```rust
use winble::{Uuid, BluetoothUuidExt};

// 标准 BLE UUID16（16 位）
let battery_level = Uuid::from_u16(0x2A19);    // Battery Level Characteristic
let battery_service = Uuid::from_u16(0x180F);  // Battery Service

// 自定义 UUID128（128 位）
let echo_uuid = Uuid::from_u128(0x4088_13df_5dd4_1f87_ec11_cdb0_0110_1002);

// UUID16 转数组
let uuid16_bytes = battery_level.to_u16().to_le_bytes();
```

---

## 注意事项

1. **仅 Windows**：`winble` 只能在 Windows 上编译和运行（`#![cfg(windows)]`）
2. **需要 WinRT**：Windows 10 1809+ 支持 WinRT BLE API
3. **无需管理员权限**：WinRT API 不需要管理员权限，比原始 HCI socket 更友好
4. **异步 runtime**：所有操作都是 `async`，需要 `tokio` 或其他 async runtime
5. **特征查找是 O(n)**：`find_char` 线性遍历所有特征，连接后特征数量通常不多，不是性能瓶颈
6. **postcard 序列化**：`read_typed` / `write_typed` 使用 `postcard`（`no_std` 友好），注意序列化类型需要实现 `Serialize` / `Deserialize`
