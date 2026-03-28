//! GATT — Generic Attribute Profile
//! GATT — 通用属性层
//!
//! Handles service/characteristic discovery, reading, writing, and notifications.
//! 处理服务/特征值发现、读取、写入和通知。
//!
//! # Submodules / 子模块
//!
//! | Module | Description |
//! |--------|-------------|
//! | [`Client`] | GATT client for a connected peripheral. 已连接外设的 GATT 客户端。 |
//! | [`GattDatabase`] | Cached GATT database from discovery. 发现结果的缓存 GATT 数据库。 |

//! # Types / 类型
//!
//! | Type | Description |
//! |------|-------------|
//! | [`Result`] | Alias for `Result<T, BtleplusError>`. `Result<T, BtleplusError>` 的别名。 |

use crate::error::BtleplusError;

mod client;
mod db;

pub use client::Client;
pub use db::GattDatabase;

/// Result type alias
/// 结果类型别名
pub type Result<T> = std::result::Result<T, BtleplusError>;
