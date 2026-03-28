//! Error types for btleplus
//! btleplus 的错误类型

use thiserror::Error;

/// Errors that can occur during BLE operations.
/// BLE 操作过程中可能发生的错误。
#[derive(Error, Debug)]
pub enum BtleplusError {
    /// Bluetooth subsystem error.
    /// 蓝牙子系统错误。
    #[error("Bluetooth error: {0}")]
    Bluetooth(String),

    /// Device was not found during scan.
    /// 扫描时未找到设备。
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
    /// 无效操作（例如找不到特征值）。
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

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
