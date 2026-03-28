//! GAP — Generic Access Profile.
//! GAP — 通用访问层。
//!
//! Handles scanning, filtering, selection, and connection management.
//! 处理扫描、过滤、选择和连接管理。
//!
//! # Modules / 模块
//!
//! | Module | Description |
//! |--------|-------------|
//! | [`adapter`] | System Bluetooth adapter entry point. 系统蓝牙适配器入口。 |
//! | [`filter`] | Scan-time hard filter (`ScanFilter`). 扫描期硬性过滤器。 |
//! | [`selection`] | Post-scan ranking and selection (`Selector`). 扫描后排序和选择。 |
//! | [`display`] | Formatting helpers for peripheral output. 外设输出的格式化辅助。 |
//! | [`peripheral`] | Discovered device type and properties. 发现的设备类型和属性。 |
//! | [`connection`] | Established BLE link. 已建立的 BLE 链路。 |

mod adapter;
mod connection;
mod display;
pub mod filter;
mod peripheral;
mod selection;

pub use adapter::Adapter;
pub use connection::Connection;
pub use display::{PeripheralDisplayExt, PeripheralDisplayList};
pub use filter::ScanFilter;
pub use peripheral::{ManufacturerData, Peripheral, PeripheralProperties};
pub use selection::{PeripheralSelectionExt, Selector};
