//! Cross-platform BLE library via [bluest]
//! 通过 bluest 实现跨平台 BLE 库
//!
//! ## Architecture
//! ## 架构
//!
//! The library is split into two primary layers:
//! 该库分为两个主要层次：
//!
//! - [`gap`] — Generic Access Profile: scanning, filtering, connection management
//! - [`gap`] — 通用访问层：扫描、过滤、连接管理
//! - [`gatt`] — Generic Attribute Profile: service discovery, read, write, notifications
//! - [`gatt`] — 通用属性层：服务发现、读、写、通知
//!
//! Recommended flow:
//! 推荐流程：
//!
//! `gap::Adapter -> gap::Peripheral -> gap::Connection -> gatt::Client`
//!
//! ## Quick Start
//! ## 快速开始
//!
//! ```ignore
//! use btleplus::{Connection, ScanFilter, Uuid};
//! use std::time::Duration;
//!
//! // Connect by name
//! // 按名称连接
//! let connection = Connection::connect("device-name", Duration::from_secs(10)).await?;
//! let gatt = connection.into_gatt().await?;
//!
//! // Read characteristic
//! // 读取特征值
//! let data = gatt.read(Uuid::from_u16(0x2A19)).await?;
//!
//! // Write with response
//! // 带响应的写操作
//! gatt.write(Uuid::from_u16(0x2A19), &[1, 2, 3], true).await?;
//!
//! // Subscribe to notifications
//! // 订阅通知
//! let stream = gatt.notifications(Uuid::from_u16(0x2A19)).await?;
//! ```
//!
//! ## Scan Filter
//! ## 扫描过滤器
//!
//! ```ignore
//! let filter = ScanFilter::default()
//!     .with_name_pattern("my-device")
//!     .with_name_patterns(["device1", "device2"])
//!     .with_addr_pattern("001122334455")
//!     .with_service_uuid(Uuid::from_u16(0x180F))
//!     .with_scan_interval_secs(3);
//!
//! let connection = Connection::connect_with_filter(filter, Duration::from_secs(10)).await?;
//! let gatt = connection.into_gatt().await?;
//! ```

#![cfg(windows)]

mod error;
pub mod gap;
pub mod gatt;

// Re-exports / 重新导出
pub use bluest::Uuid;
pub use bluest::btuuid::BluetoothUuidExt;
pub use error::BtleplusError;
pub use gap::{
    Adapter, Connection, ManufacturerData, Peripheral, PeripheralDisplayExt, PeripheralDisplayList,
    PeripheralProperties, PeripheralSelectionExt, ScanFilter, Selector,
};
pub use gatt::{Client, GattDatabase, Result};
