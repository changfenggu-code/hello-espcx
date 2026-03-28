//! GAP — Generic Access Profile
//! GAP — 通用访问层
//!
//! Handles advertising, scanning, filtering, and connection management.
//! 处理广播、扫描、过滤和连接管理。

mod adapter;
mod connection;
pub mod filter;
mod peripheral;

pub use adapter::Adapter;
pub use connection::Connection;
pub use filter::ScanFilter;
pub use peripheral::{Peripheral, PeripheralProperties};
