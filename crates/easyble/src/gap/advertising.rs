//! GAP 广播操作 — 单次广播→接受连接
//! GAP advertising operations — single advertise→accept-connection flow.
//!
//! ## 广播阶段 / Advertising Stage
//!
//! `advertising()` 执行一次完整的广播→连接接受流程：
//! `advertising()` performs one complete advertise→accept-connection flow:
//!
//! ```text
//! advertise(ConnectableScannableUndirected)
//!   └─ accept()  ← 等待 Central 连接请求
//!       └─ return Connection
//! ```
//!
//! 返回底层 BLE 连接；后续是否绑定 GATT server 由调用方决定。
//! Returns the raw BLE connection; whether and how to bind a GATT server is up to the caller.

use rtt_target::rprintln;
use trouble_host::prelude::*;

/// App 层拥有的广播数据（所有权）/ Advertisement payload owned by the app layer.
///
/// BLE 广播包和扫描响应包各最大 31 字节。
/// BLE advertisement and scan response each max 31 bytes.
pub struct AdvertisementData {
    /// 广播数据缓冲区 / Advertisement data buffer.
    pub adv_data: [u8; 31],
    /// 广播数据有效长度 / Valid length of advertisement data.
    pub adv_len: usize,
    /// 扫描响应数据缓冲区 / Scan response data buffer.
    pub scan_data: [u8; 31],
    /// 扫描响应有效长度 / Valid length of scan response data.
    pub scan_len: usize,
}

impl AdvertisementData {
    /// 转换为视图引用（供 advertising 函数使用）
    /// Convert to borrowed view (consumed by advertising function).
    pub fn as_view(&self) -> AdvertisementView<'_> {
        AdvertisementView {
            adv_data: &self.adv_data[..self.adv_len],
            scan_data: &self.scan_data[..self.scan_len],
        }
    }
}

/// 广播数据视图（借用）/ Borrowed advertisement data view.
///
/// 描述"要发哪些字节"，不关心这些字节如何由产品层构建。
/// Describes the bytes to advertise, without knowing how the product layer built them.
pub struct AdvertisementView<'a> {
    /// 广播数据 / Advertisement data.
    pub adv_data: &'a [u8],
    /// 扫描响应数据 / Scan response data.
    pub scan_data: &'a [u8],
}

/// 启动一次广播并等待 Central 连接
/// Start one advertising attempt and wait for a Central connection.
///
/// 以可连接+可扫描模式开始广播，等待 Central 发起连接请求后返回 `Connection`。
/// Starts advertising in connectable-scannable mode and returns a `Connection`
/// once a Central initiates a connection.
///
/// ## 返回值 / Return Value
///
/// 返回已建立的底层 BLE 连接；后续是否绑定 GATT server 由更高层决定。
/// Returns the established BLE connection; higher layers decide whether and how
/// to bind a GATT server.
pub async fn advertising<'stack, C: Controller>(
    peripheral: &mut Peripheral<'stack, C, DefaultPacketPool>,
    data: AdvertisementView<'_>,
) -> Result<Connection<'stack, DefaultPacketPool>, BleHostError<C::Error>> {
    let advertiser = peripheral
        .advertise(
            &Default::default(),
            Advertisement::ConnectableScannableUndirected {
                adv_data: data.adv_data,
                scan_data: data.scan_data,
            },
        )
        .await?;

    rprintln!("[easyble] advertising");
    let conn = advertiser.accept().await?;
    rprintln!("[easyble] connection established");
    Ok(conn)
}
