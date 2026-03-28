//! Connected GAP link to a peripheral.
//! 到外设的已连接 GAP 链路。
//!
//! # Public API / 公开 API
//!
//! | Method | Description |
//! |--------|-------------|
//! | [`Connection::into_gatt`] | Convert to GATT client. 转换为 GATT 客户端。 |
//! | [`Connection::is_connected`] | Check if still connected. 检查是否仍连接。 |
//! | [`Connection::disconnect`] | Disconnect. 断开连接。 |
//! | [`Connection::reconnect`] | Reconnect to the same device. 重连到同一设备。 |
//! | [`Connection::peripheral`] | Get scan-time properties (use for id/name/etc.). 获取扫描时的属性（用于 id/name 等）。 |

use bluest::Device;

use crate::{error::BtleplusError, gatt::Client};

use super::{Adapter, PeripheralProperties};

/// Connected peripheral link.
/// 已连接的外设链路。
///
/// This type owns connection lifecycle operations such as disconnect, reconnect,
/// 此类型拥有连接生命周期操作，如断开连接、重连和连接状态检查。
/// and connection-state checks. Use [`Connection::into_gatt`] to hand the live
/// 请使用 [`Connection::into_gatt`] 将实时链路移交给 GATT 层进行属性操作。
/// link to the GATT layer for attribute operations.
#[derive(Debug, Clone)]
pub struct Connection {
    pub(crate) adapter: Adapter,
    pub(crate) device: Device,
    peripheral: PeripheralProperties,
}

impl Connection {
    /// Create a new Connection from its components.
    /// 从组件创建新的 Connection。
    pub(crate) fn new(adapter: Adapter, device: Device, peripheral: PeripheralProperties) -> Self {
        Self {
            adapter,
            device,
            peripheral,
        }
    }

    /// Convert this live connection into a GATT client.
    /// 将此活动连接转换为 GATT 客户端。
    pub async fn into_gatt(self) -> Result<Client, BtleplusError> {
        Client::from_connection(self).await
    }

    /// Check if the device is still connected.
    /// 检查设备是否仍然连接。
    pub async fn is_connected(&self) -> bool {
        self.device.is_connected().await
    }

    /// Disconnect from the device if connected.
    /// 如果已连接则断开与设备的连接。
    pub async fn disconnect(&self) -> Result<(), BtleplusError> {
        if self.device.is_connected().await {
            self.adapter.inner().disconnect_device(&self.device).await?;
        }
        Ok(())
    }

    /// Reconnect to the same device.
    /// 重新连接到同一设备。
    pub async fn reconnect(&self) -> Result<(), BtleplusError> {
        if self.device.is_connected().await {
            self.adapter.inner().disconnect_device(&self.device).await?;
        }
        self.adapter.inner().connect_device(&self.device).await?;
        Ok(())
    }

    /// Metadata captured from the scan result that led to this connection.
    /// 从导致此连接的扫描结果中捕获的元数据。
    pub fn peripheral(&self) -> &PeripheralProperties {
        &self.peripheral
    }

    /// Access the underlying platform device handle.
    /// 访问底层平台设备句柄。
    pub(crate) fn device(&self) -> &Device {
        &self.device
    }

    /// Connected device identifier string.
    /// 已连接设备的标识符字符串。
    pub fn id(&self) -> &str {
        &self.peripheral.id
    }

    /// Connected device local name, if known from the scan result.
    /// 已连接设备的本地名称（如果从扫描结果中已知）。
    pub fn local_name(&self) -> Option<&str> {
        self.peripheral.local_name.as_deref()
    }
}
