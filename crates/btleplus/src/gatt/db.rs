//! GATT discovery cache.
//! GATT 发现缓存。
//!
//! # Public API / 公开 API
//!
//! | Method | Description |
//! |--------|-------------|
//! | [`GattDatabase::discover`] | Discover services and characteristics from a device. 从设备发现服务和特征值。 |
//! | [`GattDatabase::num_services`] | Number of discovered services. 已发现服务的数量。 |
//! | [`GattDatabase::num_characteristics`] | Number of discovered characteristics. 已发现特征值的数量。 |
//! | [`GattDatabase::discovered_characteristics`] | Stream all discovered characteristics. 流式获取所有已发现的特征值。 |

use bluest::{Characteristic, Device, Service, Uuid};
use futures_core::Stream;

use crate::error::BtleplusError;

use super::Result;

/// Cached GATT database discovered from a connected device.
/// 从已连接设备发现的缓存 GATT 数据库。
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

    pub(crate) fn find_characteristic(&self, uuid: Uuid) -> Result<&Characteristic> {
        self.characteristics
            .iter()
            .find(|characteristic| characteristic.uuid() == uuid)
            .ok_or_else(|| {
                BtleplusError::InvalidOperation(format!("Characteristic {} not found", uuid))
            })
    }

    /// Number of discovered services.
    /// 已发现服务的数量。
    pub fn num_services(&self) -> usize {
        self.services.len()
    }

    /// Number of discovered characteristics.
    /// 已发现特征值的数量。
    pub fn num_characteristics(&self) -> usize {
        self.characteristics.len()
    }

    /// Stream all discovered characteristics.
    /// 流式获取所有已发现的特征值。
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
}
