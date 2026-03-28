//! GATT client operations on top of a live GAP connection.
//! 基于 GAP 连接的 GATT 客户端操作。
//!
//! # Public API / 公开 API
//!
//! | Method | Description |
//! |--------|-------------|
//! | [`Client::connection`] | Borrow the underlying GAP connection. 借用底层 GAP 连接。 |
//! | [`Client::into_connection`] | Consume client, return GAP connection. 消费客户端，返回 GAP 连接。 |
//! | [`Client::database`] | Borrow the discovered GATT database cache. 借用已发现的 GATT 数据库缓存。 |
//! | [`Client::rediscover`] | Re-run GATT discovery on current connection. 在当前连接上重新执行 GATT 发现。 |
//! | [`Client::read`] | Read a characteristic value by UUID. 按 UUID 读取特征值。 |
//! | [`Client::read_to_string`] | Read a characteristic as UTF-8 string. 读取特征值并转为 UTF-8 字符串。 |
//! | [`Client::read_to`] | Read and deserialize via postcard. 读取并通过 postcard 反序列化。 |
//! | [`Client::write`] | Write bytes to a characteristic. 向特征值写入字节。 |
//! | [`Client::write_from`] | Serialize via postcard and write. 通过 postcard 序列化后写入。 |
//! | [`Client::notifications`] | Get a notification stream from a characteristic. 获取特征值的通知流。 |
//! | [`Client::discovered_characteristics`] | Stream all discovered characteristics. 流式返回所有已发现的特征值。 |
//! | [`Client::num_services`] | Number of discovered services. 已发现的服务数量。 |
//! | [`Client::num_characteristics`] | Number of discovered characteristics. 已发现的特征值数量。 |

use bluest::Uuid;
use futures_core::Stream;
use futures_util::StreamExt;

use crate::{error::BtleplusError, gap::Connection};

use super::{GattDatabase, Result};

/// GATT client for a connected peripheral.
/// 已连接外设的 GATT 客户端。
///
/// Wraps a [`Connection`] together with a discovered [`GattDatabase`],
/// providing read/write/notify operations on characteristic values.
/// 将 [`Connection`] 和已发现的 [`GattDatabase`] 封装在一起，
/// 提供对特征值的读/写/通知操作。
pub struct Client {
    connection: Connection,
    db: GattDatabase,
}

impl Client {
    /// Borrow the underlying GAP connection.
    /// 借用底层 GAP 连接。
    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    /// Consume the client and return the underlying GAP connection.
    /// 消费客户端，返回底层 GAP 连接。
    pub fn into_connection(self) -> Connection {
        self.connection
    }

    /// Borrow the discovered GATT database cache.
    /// 借用已发现的 GATT 数据库缓存。
    pub fn database(&self) -> &GattDatabase {
        &self.db
    }

    /// Re-run GATT discovery on the current connection.
    /// 在当前连接上重新执行 GATT 发现。
    pub async fn rediscover(&mut self) -> Result<()> {
        self.db = GattDatabase::discover(self.connection.device()).await?;
        Ok(())
    }

    /// Read a characteristic value by UUID.
    /// 按 UUID 读取特征值的原始字节。
    pub async fn read(&self, uuid: Uuid) -> Result<Vec<u8>> {
        let characteristic = self.db.find_characteristic(uuid)?;
        Ok(characteristic.read().await?)
    }

    /// Read a characteristic value as a UTF-8 string.
    /// 读取特征值并转为 UTF-8 字符串（无效字节被替换为 U+FFFD）。
    pub async fn read_to_string(&self, uuid: Uuid) -> Result<String> {
        let bytes = self.read(uuid).await?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// Read and deserialize a characteristic value using postcard.
    /// 读取特征值并通过 postcard 反序列化为指定类型。
    pub async fn read_to<T: serde::de::DeserializeOwned>(&self, uuid: Uuid) -> Result<T> {
        let bytes = self.read(uuid).await?;
        postcard::from_bytes(&bytes)
            .map_err(|_| BtleplusError::Deserialize("postcard deserialize failed".into()))
    }

    /// Serialize and write a value to a characteristic using postcard.
    /// 通过 postcard 序列化后写入特征值。
    pub async fn write_from<T: serde::Serialize>(
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

    /// Write bytes to a characteristic.
    /// 向特征值写入原始字节。
    ///
    /// `with_response` selects write-with-response vs write-without-response.
    /// `with_response` 选择有响应写入或无响应写入。
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
    /// 流式返回所有已发现的特征值。
    pub async fn discovered_characteristics(
        &self,
    ) -> Result<impl Stream<Item = Result<bluest::Characteristic>>> {
        self.db.discovered_characteristics().await
    }

    /// Number of discovered services.
    /// 已发现的服务数量。
    pub fn num_services(&self) -> usize {
        self.db.num_services()
    }

    /// Number of discovered characteristics.
    /// 已发现的特征值数量。
    pub fn num_characteristics(&self) -> usize {
        self.db.num_characteristics()
    }

    // Internal: create a Client by discovering GATT services on the connection.
    // 内部方法：通过在连接上发现 GATT 服务来创建 Client。
    pub(crate) async fn from_connection(connection: Connection) -> Result<Self> {
        let db = GattDatabase::discover(connection.device()).await?;
        Ok(Self { connection, db })
    }
}
