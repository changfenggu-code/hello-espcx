//! Error types for btleplus.
//! btleplus 错误类型。
//!
//! # Error variants / 错误变体
//!
//! | Variant | Description |
//! |---------|-------------|
//! | [`BtleplusError::Bluetooth`] | Bluetooth subsystem error. 蓝牙子系统错误。 |
//! | [`BtleplusError::DeviceNotFound`] | Device not found during scan. 扫描期间未找到设备。 |
//! | [`BtleplusError::ConnectionFailed`] | Connection attempt failed. 连接尝试失败。 |
//! | [`BtleplusError::Io`] | IO error (file, network, etc). IO 错误（文件、网络等）。 |
//! | [`BtleplusError::Timeout`] | Operation timed out. 操作超时。 |
//! | [`BtleplusError::NotConnected`] | Operation requires connection but device is not connected. 操作需要连接但设备未连接。 |
//! | [`BtleplusError::InvalidOperation`] | Invalid operation (e.g., characteristic not found). 无效操作（如找不到特征值）。 |
//! | [`BtleplusError::SelectionFailed`] | Device selection failed before connection. 连接前的设备选择失败。 |
//! | [`BtleplusError::Deserialize`] | Deserialize error. 反序列化错误。 |
//! | [`BtleplusError::Serialize`] | Serialize error. 序列化错误。 |

use thiserror::Error;

/// Errors that can occur during BLE operations.
/// BLE 操作中可能出现的错误。
#[derive(Error, Debug)]
pub enum BtleplusError {
    /// Bluetooth subsystem error.
    /// 蓝牙子系统错误。
    #[error("Bluetooth error: {0}")]
    Bluetooth(String),

    /// Device was not found during scan.
    /// 扫描期间未找到设备。
    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    /// Failed to connect to a device.
    /// 连接设备失败。
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    /// IO error (file, network, etc).
    /// IO 错误（文件、网络等）。
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Operation timed out.
    /// 操作超时。
    #[error("Timeout")]
    Timeout,

    /// Operation requires a connection but device is not connected.
    /// 操作需要连接但设备未连接。
    #[error("Not connected")]
    NotConnected,

    /// Invalid operation (e.g., characteristic not found).
    /// 无效操作（如找不到特征值）。
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    /// Device selection failed before connection.
    /// 连接前的设备选择失败。
    #[error("Selection failed: {0}")]
    SelectionFailed(String),

    /// Deserialize error.
    /// 反序列化错误。
    #[error("Deserialize error: {0}")]
    Deserialize(String),

    /// Serialize error.
    /// 序列化错误。
    #[error("Serialize error: {0}")]
    Serialize(String),
}

impl From<bluest::Error> for BtleplusError {
    fn from(e: bluest::Error) -> Self {
        BtleplusError::Bluetooth(e.to_string())
    }
}
