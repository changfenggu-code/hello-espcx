//! GATT client operations on top of a live GAP connection.
//! 在活动 GAP 连接之上的 GATT 客户端操作。
//!
//! # Public API / 公开 API
//!
//! | Method | Description |
//! |--------|-------------|
//! | [`Client::connection`] | Borrow the underlying GAP connection. 借用底层 GAP 连接。 |
//! | [`Client::into_connection`] | Consume and return the GAP connection. 消费并返回 GAP 连接。 |
//! | [`Client::database`] | Borrow the discovered GATT database cache. 借用已发现的 GATT 数据库缓存。 |
//! | [`Client::rediscover`] | Re-run GATT discovery. 重新执行 GATT 发现。 |
//! | [`Client::read`] | Read characteristic bytes by UUID. 按 UUID 读取特征值字节。 |
//! | [`Client::read_string`] | Read characteristic as UTF-8 string. 读取特征值为 UTF-8 字符串。 |
//! | [`Client::read_typed`] | Read and deserialize with postcard. 读取并用 postcard 反序列化。 |
//! | [`Client::write_typed`] | Serialize with postcard and write. 用 postcard 序列化并写入。 |
//! | [`Client::write`] | Write bytes to characteristic. 写入字节到特征值。 |
//! | [`Client::notifications`] | Subscribe to notifications stream. 订阅通知流。 |
//! | [`Client::discovered_characteristics`] | Stream all discovered characteristics. 流式获取所有已发现的特征值。 |
//! | [`Client::num_services`] | Number of discovered services. 已发现服务的数量。 |
//! | [`Client::num_characteristics`] | Number of discovered characteristics. 已发现特征值的数量。 |

use bluest::Uuid;
use futures_core::Stream;
use futures_util::StreamExt;

use crate::{error::BtleplusError, gap::Connection};

use super::{GattDatabase, Result};

/// GATT client for a connected peripheral.
/// 已连接外设的 GATT 客户端。
pub struct Client {
    connection: Connection,
    db: GattDatabase,
}

impl Client {
    pub(crate) async fn from_connection(connection: Connection) -> Result<Self> {
        let db = GattDatabase::discover(connection.device()).await?;
        Ok(Self { connection, db })
    }

    /// Borrow the underlying GAP connection.
    /// 借用底层 GAP 连接。
    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    /// Consume the client and return the underlying GAP connection.
    /// 消费客户端并返回底层 GAP 连接。
    pub fn into_connection(self) -> Connection {
        self.connection
    }

    /// Borrow the discovered GATT database cache.
    /// 借用已发现的 GATT 数据库缓存。
    pub fn database(&self) -> &GattDatabase {
        &self.db
    }

    /// Re-run GATT discovery on the current connection.
    /// 在当前连接上重新运行 GATT 发现。
    pub async fn rediscover(&mut self) -> Result<()> {
        self.db = GattDatabase::discover(self.connection.device()).await?;
        Ok(())
    }

    /// Read a characteristic value by UUID.
    /// 按 UUID 读取特征值。
    pub async fn read(&self, uuid: Uuid) -> Result<Vec<u8>> {
        let characteristic = self.db.find_characteristic(uuid)?;
        Ok(characteristic.read().await?)
    }

    /// Read a characteristic value as a UTF-8 string.
    /// 将特征值读取为 UTF-8 字符串。
    pub async fn read_string(&self, uuid: Uuid) -> Result<String> {
        let bytes = self.read(uuid).await?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// Read and deserialize a characteristic value using postcard.
    /// 使用 postcard 读取并反序列化特征值。
    pub async fn read_typed<T: serde::de::DeserializeOwned>(&self, uuid: Uuid) -> Result<T> {
        let bytes = self.read(uuid).await?;
        postcard::from_bytes(&bytes)
            .map_err(|_| BtleplusError::Deserialize("postcard deserialize failed".into()))
    }

    /// Serialize and write a value to a characteristic using postcard.
    /// 使用 postcard 序列化并写入值到特征值。
    pub async fn write_typed<T: serde::Serialize>(
        &self,
        uuid: Uuid,
        value: &T,
        with_response: bool,
    ) -> Result<()> {
        let mut buf = [0u8; 256];
        let used = postcard::to_slice(value, &mut buf)
            .map_err(|_| BtleplusError::Serialize("postcard serialize failed".into()))?;
        self.write(uuid, used, with_response).await
    }

    /// Write to a characteristic.
    /// 写入特征值。
    pub async fn write(&self, uuid: Uuid, data: &[u8], with_response: bool) -> Result<()> {
        let characteristic = self.db.find_characteristic(uuid)?;

        if with_response {
            Ok(characteristic.write(data).await?)
        } else {
            Ok(characteristic.write_without_response(data).await?)
        }
    }

    /// Get a stream of notifications from a characteristic.
    /// 获取特征值的通知流。
    pub async fn notifications(
        &self,
        uuid: Uuid,
    ) -> Result<impl Stream<Item = Result<Vec<u8>>> + '_> {
        let characteristic = self.db.find_characteristic(uuid)?;
        let stream = characteristic.notify().await?;
        Ok(stream.map(|value| value.map_err(BtleplusError::from)))
    }

    /// Get a stream of all discovered characteristics.
    /// 获取所有已发现特征值的流。
    pub async fn discovered_characteristics(
        &self,
    ) -> Result<impl Stream<Item = Result<bluest::Characteristic>>> {
        self.db.discovered_characteristics().await
    }

    /// Number of discovered services.
    /// 已发现服务的数量。
    pub fn num_services(&self) -> usize {
        self.db.num_services()
    }

    /// Number of discovered characteristics.
    /// 已发现特征值的数量。
    pub fn num_characteristics(&self) -> usize {
        self.db.num_characteristics()
    }
}
