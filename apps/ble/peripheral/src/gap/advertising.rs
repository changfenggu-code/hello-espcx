//! GAP 广播操作 / GAP advertising operations.
//!
//! 提供单次广播→接受连接的异步流程。
//! Provides a single advertise→accept-connection async flow.

use rtt_target::rprintln;
use trouble_host::prelude::*;

/// 通用广播数据视图 / Generic advertisement data view.
///
/// 只描述“要发哪些字节”，不关心这些字节如何由产品层构建。
/// Describes only the bytes to advertise, without knowing how product code built them.
pub(crate) struct AdvertisementView<'a> {
    pub(crate) adv_data: &'a [u8],
    pub(crate) scan_data: &'a [u8],
}

/// 启动一次广播并等待 Central 连接 / Start one advertising attempt and wait for a connection.
///
/// 流程 / Flow:
/// 1. 以可连接+可扫描模式开始广播
///    Start advertising in connectable-scannable mode
/// 2. 等待 Central 发起连接
///    Wait for a Central to connect
/// 返回已建立的底层 BLE 连接；后续是否绑定 GATT server 由更高层决定。
/// Returns the established BLE connection; higher layers decide whether and how
/// to bind a GATT server next.
pub(crate) async fn advertise<'values, C: Controller>(
    peripheral: &mut Peripheral<'values, C, DefaultPacketPool>,
    data: AdvertisementView<'_>,
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

    rprintln!("[adv] advertising");
    let conn = advertiser.accept().await?;
    rprintln!("[adv] connection established");
    Ok(conn)
}
