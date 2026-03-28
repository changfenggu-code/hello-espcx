//! Connected GAP link to a peripheral.
//! 到外设的已连接 GAP 链路。
//!
//! # Public API / 公开 API
//!
//! | Method | Description |
//! |--------|-------------|
//! | [`Connection::connect`] | Connect by device name. 按设备名连接。 |
//! | [`Connection::connect_by_address`] | Connect by address. 按地址连接。 |
//! | [`Connection::connect_by_service`] | Connect by advertised service UUID. 按广播的服务 UUID 连接。 |
//! | [`Connection::connect_with_filter`] | Connect with a custom scan filter. 使用自定义扫描过滤器连接。 |
//! | [`Connection::into_gatt`] | Convert to GATT client. 转换为 GATT 客户端。 |
//! | [`Connection::is_connected`] | Check if still connected. 检查是否仍连接。 |
//! | [`Connection::disconnect`] | Disconnect. 断开连接。 |
//! | [`Connection::reconnect`] | Reconnect to the same device. 重连到同一设备。 |
//! | [`Connection::peripheral`] | Get scan-time properties. 获取扫描时的属性。 |
//! | [`Connection::id`] | Get device identifier. 获取设备标识符。 |
//! | [`Connection::local_name`] | Get advertised local name. 获取广播的本地名称。 |

use bluest::{Device, Uuid};
use std::time::Duration;

use crate::{error::BtleplusError, gatt::Client};

use super::{Adapter, PeripheralProperties, ScanFilter};

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
    pub(crate) fn new(adapter: Adapter, device: Device, peripheral: PeripheralProperties) -> Self {
        Self {
            adapter,
            device,
            peripheral,
        }
    }

    /// Connect to a peripheral by name.
    /// 按名称连接到外设。
    pub async fn connect(name: &str, timeout: Duration) -> Result<Self, BtleplusError> {
        let filter = ScanFilter::default().with_name_pattern(name);
        Self::connect_with_filter(filter, timeout).await
    }

    /// Connect to a peripheral by address.
    /// 按地址连接到外设。
    pub async fn connect_by_address(
        address: &str,
        timeout: Duration,
    ) -> Result<Self, BtleplusError> {
        let filter = ScanFilter::default().with_addr_pattern(address);
        Self::connect_with_filter(filter, timeout).await
    }

    /// Connect to a peripheral advertising a service UUID.
    /// 连接到广播特定服务 UUID 的外设。
    pub async fn connect_by_service(uuid: Uuid, timeout: Duration) -> Result<Self, BtleplusError> {
        let filter = ScanFilter::default().with_service_uuid(uuid);
        Self::connect_with_filter(filter, timeout).await
    }

    /// Scan and connect using a custom filter.
    /// 使用自定义过滤器扫描并连接。
    pub async fn connect_with_filter(
        filter: ScanFilter,
        timeout: Duration,
    ) -> Result<Self, BtleplusError> {
        let adapter = Adapter::default().await?;
        adapter.connect_with_filter(filter, timeout).await
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
