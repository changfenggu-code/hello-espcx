//! GATT 会话事件循环 / GATT session event loop.
//!
//! 驱动单次连接的 GATT 事件循环，通过回调将事件分发给调用方。
//! Drives the GATT event loop for a single connection, dispatching events via callback.
//!
//! 此模块完全通用，不包含任何产品特定逻辑。
//! This module is fully generic and contains no product-specific logic.

use rtt_target::rprintln;
use trouble_host::prelude::*;

/// 驱动单次连接的 GATT 事件循环 / Drive the connection-scoped GATT event loop for one session.
///
/// 持续从连接中读取 GATT 事件，对每个事件：
/// Continuously reads GATT events from the connection, for each event:
///
/// 1. 调用 `on_gatt_event` 回调让调用方处理事件
///    Calls `on_gatt_event` callback for the caller to handle the event
/// 2. 自动发送 GATT 响应（accept + send）
///    Automatically sends the GATT response (accept + send)
///
/// 连接断开时返回 `Ok(())`。
/// Returns `Ok(())` when the connection disconnects.
///
/// ## 参数 / Parameters
///
/// - `conn`: 已绑定 attribute server 的 GATT 连接 / GATT connection with attribute server bound
/// - `on_gatt_event`: 事件处理回调（每次 GATT 读/写事件触发）/ Event handler callback (fires on each read/write)
pub(crate) async fn run_session<P, F>(
    conn: &GattConnection<'_, '_, P>,
    mut on_gatt_event: F,
) -> Result<(), Error>
where
    P: PacketPool,
    F: for<'stack, 'server> FnMut(&GattEvent<'stack, 'server, P>),
{
    let reason = loop {
        match conn.next().await {
            // 连接断开，退出事件循环 / Connection lost, exit event loop
            GattConnectionEvent::Disconnected { reason } => break reason,
            // 收到 GATT 事件（读/写请求） / GATT event received (read/write request)
            GattConnectionEvent::Gatt { event } => {
                // 交给调用方处理 / Dispatch to caller
                on_gatt_event(&event);

                // 自动回复 GATT 响应 / Auto-reply GATT response
                match event.accept() {
                    Ok(reply) => reply.send().await,
                    Err(e) => rprintln!("[gatt] error sending response: {:?}", e),
                };
            }
            _ => {}
        }
    };
    rprintln!("[gatt] disconnected: {:?}", reason);
    Ok(())
}
