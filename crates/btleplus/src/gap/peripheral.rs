//! Peripheral discovered during GAP scanning.
//! GAP 扫描中发现的外设。
//!
//! # Public types / 公开类型
//!
//! | Type | Description |
//! |------|-------------|
//! | [`Peripheral`] | A discovered device not yet connected. 扫描到但尚未连接的设备。 |
//! | [`PeripheralProperties`] | Snapshot of advertisement properties. 广播属性的快照。 |
//! | [`ManufacturerData`] | Manufacturer-specific advertisement payload. 厂商特定的广播载荷。 |
//!
//! # ManufacturerData methods / ManufacturerData 方法
//!
//! | Method | Description |
//! |--------|-------------|
//! | [`ManufacturerData::payload`] | Borrow the manufacturer payload bytes. 借用厂商载荷字节。 |
//! | [`ManufacturerData::is_company_id`] | Check company identifier match. 检查公司标识符是否匹配。 |
//!
//! # Peripheral methods / Peripheral 方法
//!
//! | Method | Description |
//! |--------|-------------|
//! | [`Peripheral::connect`] | Connect and obtain a GAP connection. 连接并获取 GAP 连接。 |
//! | [`Peripheral::properties`] | Access scan-time properties. 访问扫描时的属性。 |
//! | [`Peripheral::local_name`] | Get advertised local name. 获取广播的本地名称。 |
//! | [`Peripheral::id`] | Get device identifier string. 获取设备标识符字符串。 |

use bluest::{AdvertisingDevice, Device, Uuid};
use std::collections::BTreeMap;

use crate::error::BtleplusError;

use super::{Adapter, Connection};

/// Stable snapshot of manufacturer-specific advertisement data.
/// 厂商特定广播数据的稳定快照。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ManufacturerData {
    /// Company identifier assigned by the Bluetooth SIG.
    /// Bluetooth SIG 分配的公司标识符。
    pub company_id: u16,
    /// Manufacturer-specific payload bytes.
    /// 厂商特定的载荷字节。
    pub data: Vec<u8>,
}

impl ManufacturerData {
    /// Borrow the manufacturer payload bytes.
    /// 借用厂商载荷字节。
    pub fn payload(&self) -> &[u8] {
        &self.data
    }

    /// Check whether the company identifier matches.
    /// 检查公司标识符是否匹配。
    pub fn is_company_id(&self, company_id: u16) -> bool {
        self.company_id == company_id
    }
}

/// Snapshot of peripheral properties captured during scanning.
/// 扫描时捕获的外设属性快照。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeripheralProperties {
    /// Stable adapter-specific device identifier.
    /// 适配器级别的稳定设备标识符。
    pub id: String,
    /// Advertised local name, if present.
    /// 广播的本地名称（如存在）。
    pub local_name: Option<String>,
    /// Services advertised in scan data.
    /// 广播中声明的服务 UUID 列表。
    pub advertised_services: Vec<Uuid>,
    /// Manufacturer-specific advertisement payload, if present.
    /// 厂商特定的广播载荷（如存在）。
    pub manufacturer_data: Option<ManufacturerData>,
    /// Service data sections from the advertisement, keyed by service UUID.
    /// 广播中的服务数据，按服务 UUID 索引。
    pub service_data: BTreeMap<Uuid, Vec<u8>>,
    /// Received signal strength indicator in dBm.
    /// 接收信号强度指示（dBm）。
    pub rssi: Option<i16>,
    /// Whether the device reported itself as connectable.
    /// 设备是否声明自身可连接。
    pub is_connectable: bool,
}

impl PeripheralProperties {
    /// Convert a raw bluest advertising device into a peripheral properties snapshot.
    /// 将 bluest 原始广播设备转换为外设属性快照。
    pub(crate) fn from_advertising_device(device: &AdvertisingDevice) -> Self {
        Self {
            id: device.device.id().to_string(),
            local_name: device.adv_data.local_name.clone(),
            advertised_services: device.adv_data.services.clone(),
            manufacturer_data: device.adv_data.manufacturer_data.clone().map(|data| {
                ManufacturerData {
                    company_id: data.company_id,
                    data: data.data,
                }
            }),
            service_data: device.adv_data.service_data.clone().into_iter().collect(),
            rssi: device.rssi,
            is_connectable: device.adv_data.is_connectable,
        }
    }
}

/// Peripheral discovered via scanning, but not yet connected.
/// 通过扫描发现的外设，尚未连接。
///
/// Call [`connect`](Peripheral::connect) to establish a BLE connection
/// and obtain a [`Connection`].
/// 调用 [`connect`](Peripheral::connect) 建立 BLE 连接，获取 [`Connection`]。
#[derive(Debug, Clone)]
pub struct Peripheral {
    adapter: Adapter,
    device: Device,
    properties: PeripheralProperties,
}

impl Peripheral {
    /// Create a new `Peripheral` from its components.
    /// 从组件创建新的 Peripheral。
    pub(crate) fn new(adapter: Adapter, device: Device, properties: PeripheralProperties) -> Self {
        Self {
            adapter,
            device,
            properties,
        }
    }

    /// Connect to this peripheral and obtain a GAP connection.
    /// 连接到此设备，获取 GAP 连接。
    pub async fn connect(self) -> Result<Connection, BtleplusError> {
        self.adapter.inner().connect_device(&self.device).await?;
        Ok(Connection::new(self.adapter, self.device, self.properties))
    }

    /// Peripheral properties captured during scanning.
    /// 扫描时捕获的外设属性。
    pub fn properties(&self) -> &PeripheralProperties {
        &self.properties
    }

    /// Advertised local name, if present.
    /// 广播的本地名称（如存在）。
    pub fn local_name(&self) -> Option<&str> {
        self.properties.local_name.as_deref()
    }

    /// Device identifier string.
    /// 设备标识符字符串。
    pub fn id(&self) -> &str {
        &self.properties.id
    }
}
