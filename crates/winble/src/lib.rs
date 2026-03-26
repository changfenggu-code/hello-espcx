//! Windows BLE GATT operations via [bluest]
//!
//! [bluest]: https://docs.rs/bluest

#![cfg(windows)]

mod error;
mod gatt;

// Re-exports
pub use bluest::Uuid;
pub use bluest::btuuid::BluetoothUuidExt;
pub use error::WinbleError;
pub use gatt::{ScanFilter, Session, Result};
