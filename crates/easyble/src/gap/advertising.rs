//! GAP 广播操作 / GAP advertising operations.
//!
//! 提供单次广播→接受连接的异步流程。
//! Provides a single advertise→accept-connection async flow.

use rtt_target::rprintln;
use trouble_host::prelude::*;

/// 广播数据视图 / Advertisement data view.
pub struct AdvertisementView<'a> {
    /// 广播数据（最大 31 字节）/ Advertisement data (max 31 bytes).
    pub adv_data: &'a [u8],
    /// 扫描响应数据（最大 31 字节）/ Scan response data (max 31 bytes).
    pub scan_data: &'a [u8],
}

/// 启动一次广播并等待 Central 连接 / Start one advertising attempt and wait for a connection.
///
/// 返回已建立的 BLE 连接；后续是否绑定 GATT server 由更高层决定。
/// Returns the established BLE connection; higher layers decide whether and how
/// to bind a GATT server.
pub async fn advertise<'values, C: Controller>(
    peripheral: &mut Peripheral<'values, C, DefaultPacketPool>,
    data: &AdvertisementView<'_>,
) -> Result<Connection<'values, DefaultPacketPool>, BleHostError<C::Error>> {
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
