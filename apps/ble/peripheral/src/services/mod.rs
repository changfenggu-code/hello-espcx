//! BLE GATT 服务定义 / BLE GATT service definitions — one module per service.
//!
//! 每个服务文件用 `#[gatt_service]` 宏声明特征结构体。
//! Each service file declares a characteristic struct via the `#[gatt_service]` macro.
//!
//! ## 服务清单 / Service List
//!
//! | 模块 / Module | 服务 / Service | 类型 / Type | 说明 / Description |
//! |---|---|---|---|
//! | `battery` | Battery Service | 标准 BLE | 电量监测，read + notify / Battery level, read + notify |
//! | `device_info` | Device Information | 标准 BLE | 静态设备信息，read only / Static device info, read only |
//! | `echo` | Echo Service | 自定义 | 双向数据完整性验证 / Bidirectional integrity check |
//! | `status` | Status Service | 自定义 | read + write + notify 演示 / Read/write/notify demo |
//! | `bulk` | Bulk Service | 自定义 | 大批量数据传输 / Bulk data transfer |

mod battery;
mod bulk;
mod device_info;
mod echo;
mod status;

pub use battery::BatteryService;
pub use bulk::{BulkService, initial_bulk_control_value};
pub use device_info::DeviceInfoService;
pub use echo::EchoService;
pub use status::StatusService;
