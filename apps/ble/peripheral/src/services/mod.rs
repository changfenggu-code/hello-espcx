//! BLE GATT services — one module per service.

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
