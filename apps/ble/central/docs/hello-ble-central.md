# hello-ble-central — Windows BLE Central 应用

## 概述

`hello-ble-central` 是运行在 Windows 上的 BLE Central（主机）应用，负责扫描、连接 ESP32-C6 Peripheral，并对其 GATT 服务进行读写和订阅操作。

```
┌──────────────────────────────────────────────────────────┐
│  hello-ble-central                                       │
├──────────────────────────────────────────────────────────┤
│  main.rs           程序入口，重连循环 + 事件监听           │
│  lib.rs            BleSession 业务封装 + 连接函数          │
│  tests/hil_real.rs 硬件在环测试（需真实 ESP32）           │
└──────────────────────────────────────────────────────────┘
         │
         ▼
┌────────────────────┐     ┌─────────────────────┐
│  hello-ble-common  │     │  winble (crates/)   │
│  共享常量 + 类型    │     │  BLE 操作封装        │
└────────────────────┘     └─────────────────────┘
                                   │
                                   ▼
                          Windows Bluetooth API
```

**设计原则**：`lib.rs` 只包含业务逻辑；所有测试工具（数据生成、校验）放在 `tests/hil_real.rs` 内，不污染业务代码。

## 依赖

```toml
[dependencies]
winble              = { path = "../../../crates/winble" }  # BLE 操作封装
hello-ble-common   = { path = "../common" }             # 共享常量
tokio              = { version = "1", features = ["rt", "macros", "sync", "time"] }
tracing            = "0.1"
tracing-subscriber = "0.3"
futures-util       = "0.3"
anyhow             = "1"
postcard           = { version = "1", default-features = false }
serde              = { version = "1", default-features = false }
```

| 依赖 | 作用 |
|---|---|
| `winble` | Windows BLE GATT 操作（底层 bluest/WinRT） |
| `hello-ble-common` | 共享 BLE 常量（UUID、地址、名称） |
| `tokio` | async runtime |
| `tracing` | 结构化日志 |
| `postcard` | 结构体序列化/反序列化 |
| `anyhow` | 简化错误传播 |

## 架构分层

```
main.rs (tokio main, 重连循环)
    │
    ▼
BleSession（lib.rs）— 业务 API
    ├── 连接管理：disconnect(), is_connected()
    ├── 读：battery_level(), device_info(), status(), read_bulk_stats()
    ├── 写：set_status(), echo(), start_bulk_stream(), upload_bulk_data(), reset_bulk_stats()
    ├── 订阅：notifications()
    └── UUID 获取：battery_uuid(), echo_uuid(), bulk_data_uuid(), ...
            │
            ▼
     winble::Session（扫描/连接/读/写/订阅）
            │
            ▼
     Windows Bluetooth API
```

`BleSession` 是对 `winble::Session` 的业务封装，把裸的 UUID 操作映射为有语义的方法名，并处理序列化细节。

---

## lib.rs — 业务代码

### DeviceInfo

```rust
#[derive(Debug)]
pub struct DeviceInfo {
    pub manufacturer: String,  // 制造商
    pub model: String,       // 型号
    pub firmware: String,    // 固件版本
    pub software: String,    // 软件版本
}
```

值来源于 ESP32 Peripheral 的 Device Information Service。

### BleSession

```rust
pub struct BleSession {
    session: Session,           // winble 底层连接
    battery_uuid: Uuid,        // 0x2A19
    manufacturer_uuid: Uuid,    // 0x2A29
    model_uuid: Uuid,          // 0x2A24
    firmware_uuid: Uuid,       // 0x2A26
    software_uuid: Uuid,       // 0x2A28
    echo_uuid: Uuid,          // 自定义 UUID128
    status_uuid: Uuid,         // 自定义 UUID128
    bulk_control_uuid: Uuid,   // 自定义 UUID128
    bulk_data_uuid: Uuid,      // 自定义 UUID128
    bulk_stats_uuid: Uuid,     // 自定义 UUID128
}
```

### BleSession 方法速查

#### 连接管理

| 方法 | 说明 |
|---|---|
| `disconnect()` | 主动断开连接 |
| `is_connected()` | 查询当前连接状态 |

#### 读操作

| 方法 | 说明 | 返回类型 |
|---|---|---|
| `battery_level()` | 读电量（0-100%） | `u8` |
| `device_info()` | 读制造商/型号/固件/软件 | `DeviceInfo` |
| `status()` | 读状态（postcard 反序列化） | `bool` |
| `read_bulk_stats()` | 读 bulk 传输统计 | `BulkStats` |

#### 写操作

| 方法 | 说明 | 可靠性 |
|---|---|---|
| `set_status(value)` | 写状态（有响应） | 可靠 |
| `echo(data)` | 发送 Echo 数据（有响应） | 可靠 |
| `reset_bulk_stats()` | 重置 bulk 统计（有响应 + 轮询确认） | 可靠 |
| `start_bulk_stream(total_bytes)` | 触发 peripheral 推送数据流（有响应） | 可靠 |
| `upload_bulk_data(data)` | 上传一块数据（无响应） | 不可靠 |

#### 通知订阅

| 方法 | 说明 |
|---|---|
| `notifications(uuid)` | 通用通知订阅，按 UUID 订阅 |

#### UUID 获取

| 方法 | 说明 |
|---|---|
| `battery_uuid()` | 获取电量特征 UUID |
| `echo_uuid()` | 获取 Echo 特征 UUID |
| `bulk_data_uuid()` | 获取 bulk 数据特征 UUID |

#### 调试

| 方法 | 说明 |
|---|---|
| `list_characteristics()` | 列出所有发现特征的 UUID |

### 连接函数

#### `connect_session()`

```rust
pub async fn connect_session() -> Result<BleSession, Error>
```

默认 30 秒扫描超时，调用 `connect_session_with_timeout(SCAN_TIMEOUT)`。

#### `connect_session_with_timeout(timeout)`

```rust
pub async fn connect_session_with_timeout(timeout: Duration) -> Result<BleSession, Error>
```

完整连接流程：

```
1. 构造 ScanFilter
   - name_pattern = "hello-espcx"
   - service_uuid = 0x180F (Battery Service)
   │
   ▼
2. winble::Session::connect_with_filter()
   - 扫描设备
   - 连接 GATT
   - 发现服务和特征
   │
   ▼
3. 构造 BleSession
   - 填充所有 UUID（来自 hello-ble-common）
   │
   ▼
4. 返回 BleSession
```

---

## main.rs — 程序入口

```rust
#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()>
```

使用 `current_thread` runtime，单线程处理 BLE 事件。

### 主循环

```
启动 tracing 日志
    │
    ▼
loop {
    扫描并连接 peripheral（默认 30s 超时）
        │
        ├── 成功 → monitor_session()（阻塞直到断开或出错）
        └── 失败 → 等待 2 秒后重试
}
```

### 监控函数 `monitor_session`

连接建立后依次执行：

1. **打印所有发现特征**（调试用）
2. **读取设备信息**（4 个字符串）
3. **读取电量**
4. **读写状态**（bool）
5. **发送 Echo**
6. **订阅电量通知**

使用 `tokio::select!` 并发监听两个事件源：

```rust
tokio::select! {
    // 电量通知到达
    notification = battery_stream.next() => { ... }

    // 10 秒定时器（保活 + 定期读电量）
    _ = sleep(PERIODIC_READ_INTERVAL) => {
        if !session.is_connected().await {
            return Err(anyhow!("Disconnected"));
        }
        let level = session.battery_level().await?;
        tracing::info!("[periodic] Battery: {}%", level);
    }
}
```

---

## tests/hil_real.rs — 硬件在环测试

### 测试前提

需要：
- ESP32-C6 已烧录 `hello-ble-peripheral` 固件并运行
- Windows 蓝牙能发现并连接到 ESP32

运行方式：

```bash
cargo test --test hil_real -- --ignored
```

### 测试工具（自包含）

以下函数定义在 `hil_real.rs` 内部，不在 `lib.rs` 中：

```rust
/// 用伪随机公式填测试数据，用于 bulk 传输校验
fn fill_test_pattern(start_offset: usize, buffer: &mut [u8])
// 公式: *byte = ((((start_offset + index) * 17) + 29) % 256)

/// 循环调用 fill_test_pattern + upload_bulk_data，上传完整测试数据
async fn upload_test_pattern(session: &BleSession, total_bytes: usize) -> anyhow::Result<()>

/// 接收 peripheral 通知推送，逐块校验数据完整性
async fn receive_bulk_stream(session: &BleSession, total_bytes: usize, timeout: Duration) -> anyhow::Result<()>
```

### 测试用例

#### `esp32c6_end_to_end_hil`

完整 E2E 流程测试：

```
连接
    │
    ▼
读电量 (0-100)
    │
    ▼
写 status=true → 读 status == true
    │
    ▼
写 status=false → 读 status == false
    │
    ▼
发送 Echo 数据 → 等待回复通知 → 校验内容
    │
    ▼
重置 bulk 统计
    │
    ▼
上传 10 KiB 测试数据 → 读取统计验证
    │
    ▼
重置 bulk 统计
    │
    ▼
触发 peripheral 推送 10 KiB → 接收并校验
    │
    ▼
断开连接
```

#### `esp32c6_bulk_stress_hil`

批量传输压力测试，执行 3 轮：

```
每轮：
    重置统计
        │
        ├── 上传 10 KiB → 打印吞吐量 (KiB/s)
        │
        └── 接收 10 KiB 推送 → 打印吞吐量 (KiB/s)
```

吞吐量打印格式：

```
[hil] upload: 10.0 KiB in 1.23s -> 8.1 KiB/s
```

#### `print_throughput`

```rust
fn print_throughput(label: &str, total_bytes: usize, elapsed: Duration)
```

计算并打印 `KiB/s` 吞吐量。

---

## 数据流总览

### Peripheral → Central（通知）

```
ESP32 每 2s  →  电量通知（u8）
ESP32 按需   →  Echo 通知（回显数据）
ESP32 按需   →  Status 通知（状态变化）
ESP32 按需   →  Bulk Data 通知（批量数据流）
```

### Central → Peripheral（写/读）

```
读  Battery Level  →  u8 (0-100)
读  Device Info    →  4 个字符串
读  Status         →  bool (postcard)
读  Bulk Stats     →  BulkStats { rx, tx }

写  Status         ←  bool (postcard, 有响应)
写  Echo           ←  数据 bytes (有响应)
写  Bulk Control   ←  BulkControlCommand (有响应)
写  Bulk Data      ←  数据块 bytes (无响应)
```

### Bulk 传输协议

```
Central → Peripheral（上传）：
    for each chunk:
        upload_bulk_data(chunk)   // write without response

Central → Peripheral（触发推送）：
    start_bulk_stream(total_bytes)   // write StartStream command
    then peripheral starts streaming:
        for each chunk:
            notify(BULK_CHUNK_UUID, chunk)  ← Central receives

Central 读统计：
    read_bulk_stats() → BulkStats { rx_bytes, tx_bytes }

Central 重置：
    reset_bulk_stats()   // write ResetStats + poll until {0, 0}
```
