//! GATT discovery cache.
//! GATT 发现缓存。
//!
//! # Public API / 公开 API
//!
//! | Method | Description |
//! |--------|-------------|
//! | [`GattDatabase::discover`] | Discover services and characteristics from a connected device. 从已连接设备发现服务和特征值。 |
//! | [`GattDatabase::num_services`] | Number of discovered services. 已发现的服务数量。 |
//! | [`GattDatabase::num_characteristics`] | Number of discovered characteristics. 已发现的特征值数量。 |
//! | [`GattDatabase::discovered_characteristics`] | Stream all discovered characteristics. 流式返回所有已发现的特征值。 |

use bluest::{Characteristic, Device, Service, Uuid};
use futures_core::Stream;

use crate::error::BtleplusError;

use super::Result;

/// Cached GATT database discovered from a connected device.
/// 从已连接设备发现的缓存 GATT 数据库。
///
/// Stores all discovered services and their characteristics so that
/// UUID-based lookups are fast and do not require repeated BLE round-trips.
/// 存储所有已发现的服务及其特征值，使基于 UUID 的查找无需重复 BLE 往返。
#[derive(Debug, Clone)]
pub struct GattDatabase {
    services: Vec<Service>,
    characteristics: Vec<Characteristic>,
}

impl GattDatabase {
    /// Discover services and characteristics from a connected device.
    /// 从已连接设备发现服务和特征值。
    pub async fn discover(device: &Device) -> Result<Self> {
        let services = device.discover_services().await?;
        let mut characteristics = Vec::new();
        for service in &services {
            if let Ok(chars) = service.characteristics().await {
                characteristics.extend(chars);
            }
        }

        Ok(Self {
            services,
            characteristics,
        })
    }

    /// Number of discovered services.
    /// 已发现的服务数量。
    pub fn num_services(&self) -> usize {
        self.services.len()
    }

    /// Number of discovered characteristics.
    /// 已发现的特征值数量。
    pub fn num_characteristics(&self) -> usize {
        self.characteristics.len()
    }

    /// Stream all discovered characteristics.
    /// 流式返回所有已发现的特征值。
    pub async fn discovered_characteristics(
        &self,
    ) -> Result<impl Stream<Item = Result<Characteristic>>> {
        let stream = futures_util::stream::iter(
            self.characteristics
                .clone()
                .into_iter()
                .map(Ok::<Characteristic, BtleplusError>),
        );
        Ok(stream)
    }

    // Internal: find a characteristic by UUID in the cached database.
    // 内部方法：在缓存数据库中按 UUID 查找特征值。
    pub(crate) fn find_characteristic(&self, uuid: Uuid) -> Result<&Characteristic> {
        self.characteristics
            .iter()
            .find(|characteristic| characteristic.uuid() == uuid)
            .ok_or_else(|| {
                BtleplusError::InvalidOperation(format!("Characteristic {} not found", uuid))
            })
    }
}
