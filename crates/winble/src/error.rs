//! Error types for winble

use thiserror::Error;

/// Errors that can occur during BLE operations.
#[derive(Error, Debug)]
pub enum WinbleError {
    /// Bluetooth subsystem error.
    #[error("Bluetooth error: {0}")]
    Bluetooth(String),

    /// Device was not found during scan.
    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    /// Failed to connect to a device.
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    /// IO error (file, network, etc).
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Operation timed out.
    #[error("Timeout")]
    Timeout,

    /// Operation requires a connection but device is not connected.
    #[error("Not connected")]
    NotConnected,

    /// Invalid operation (e.g., characteristic not found).
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    /// Deserialize error.
    #[error("Deserialize error: {0}")]
    Deserialize(String),

    /// Serialize error.
    #[error("Serialize error: {0}")]
    Serialize(String),
}

impl From<bluest::Error> for WinbleError {
    fn from(e: bluest::Error) -> Self {
        WinbleError::Bluetooth(e.to_string())
    }
}
