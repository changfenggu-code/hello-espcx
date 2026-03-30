//! Echo Service — 双向数据完整性验证 / Echo Service — bidirectional data integrity verification.
//!
//! 自定义服务，用于验证 BLE 链路的数据完整性：
//! Custom service for verifying BLE link data integrity:
//!
//! ```text
//! Central ───write──▶ Peripheral（收到数据 / receive data）
//! Central ◀──notify── Peripheral（回传相同数据 / echo same data back）
//! ```

use heapless::Vec;
use trouble_host::prelude::*;

use hello_ble_common::echo;

/// Echo Service / 回声服务（自定义）。
///
/// Central 写入任意数据，Peripheral 立刻通过 notify 回传相同内容。
/// Central writes arbitrary data, Peripheral immediately notifies the same data back.
///
/// payload 最大长度由 `echo::CAPACITY`（252 字节，即 ATT_PAYLOAD_MAX）限制。
/// Max payload length is `echo::CAPACITY` (252 bytes, i.e. ATT_PAYLOAD_MAX).
#[gatt_service(uuid = echo::SERVICE_UUID)]
pub struct EchoService {
    /// Echo 特征，支持 write + notify / Echo characteristic, write + notify.
    #[characteristic(uuid = echo::UUID, write, notify, value = Vec::new())]
    pub echo: Vec<u8, { echo::CAPACITY }>,
}
