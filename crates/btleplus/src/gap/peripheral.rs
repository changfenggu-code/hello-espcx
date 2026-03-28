//! Peripheral discovered during GAP scanning.
//! GAP 扫描期间发现的外设。
//!
//! # Public API / 公开 API
//!
//! | Type / Method | Description |
//! |---------------|-------------|
//! | [`Peripheral`] | Discovered device before connection. 扫描发现但尚未连接的外设。 |
//! | [`Peripheral::connect`] | Connect and obtain a GAP connection. 连接并获取 GAP 连接。 |
//! | [`Peripheral::properties`] | Get scan-time properties. 获取扫描时的属性。 |
//! | [`Peripheral::local_name`] | Get advertised local name. 获取广播的本地名称。 |
//! | [`Peripheral::id`] | Get device identifier. 获取设备标识符。 |
//! | [`PeripheralProperties`] | Scan-time metadata snapshot. 扫描时捕获的元数据快照。 |

use bluest::{AdvertisingDevice, Device, Uuid};

use crate::error::BtleplusError;

use super::{Adapter, Connection};

/// Snapshot of peripheral properties captured during scanning.
/// 扫描期间捕获的外设属性快照。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeripheralProperties {
    /// Stable adapter-specific device identifier.
    /// 稳定的适配器特定设备标识符。
    pub id: String,
    /// Advertised local name, if present.
    /// 广播的本地名称（如果有）。
    pub local_name: Option<String>,
    /// Services advertised in scan data.
    /// 扫描数据中广播的服务。
    pub advertised_services: Vec<Uuid>,
    /// Received signal strength indicator in dBm.
    /// 接收信号强度指示器，单位 dBm。
    pub rssi: Option<i16>,
    /// Whether the device reported itself as connectable.
    /// 设备是否报告自身为可连接的。
    pub is_connectable: bool,
}

impl PeripheralProperties {
    pub(crate) fn from_advertising_device(device: &AdvertisingDevice) -> Self {
        Self {
            id: device.device.id().to_string(),
            local_name: device.adv_data.local_name.clone(),
            advertised_services: device.adv_data.services.clone(),
            rssi: device.rssi,
            is_connectable: device.adv_data.is_connectable,
        }
    }
}

/// Peripheral discovered via scanning, but not yet connected.
/// 通过扫描发现但尚未连接的外设。
#[derive(Debug, Clone)]
pub struct Peripheral {
    adapter: Adapter,
    device: Device,
    properties: PeripheralProperties,
}

impl Peripheral {
    pub(crate) fn new(adapter: Adapter, device: Device, properties: PeripheralProperties) -> Self {
        Self {
            adapter,
            device,
            properties,
        }
    }

    /// Connect to this peripheral and obtain a GAP connection.
    /// 连接到此外设并获取 GAP 连接。
    pub async fn connect(self) -> Result<Connection, BtleplusError> {
        self.adapter.inner().connect_device(&self.device).await?;
        Ok(Connection::new(self.adapter, self.device, self.properties))
    }

    /// Peripheral properties captured during scanning.
    /// 扫描期间捕获的外设属性。
    pub fn properties(&self) -> &PeripheralProperties {
        &self.properties
    }

    /// Advertised local name, if present.
    /// 广播的本地名称（如果有）。
    pub fn local_name(&self) -> Option<&str> {
        self.properties.local_name.as_deref()
    }

    /// Device identifier string.
    /// 设备标识符字符串。
    pub fn id(&self) -> &str {
        &self.properties.id
    }
}
