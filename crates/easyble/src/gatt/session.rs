//! GATT 会话阶段 — 被动 GATT 事件循环
//! GATT session stage — passive GATT event loop.
//!
//! ## 会话阶段 / Session Stage
//!
//! `session()` 驱动单次连接的 GATT 事件循环，通过回调将每个 GATT 事件分派给调用方：
//! `session()` drives the GATT event loop for a single connection, dispatching each
//! GATT event to the caller via a callback:
//!
//! ```text
//! loop {
//!     conn.next().await
//!       ├─ GattConnectionEvent::Disconnected  →  break, return Ok(())
//!       ├─ GattConnectionEvent::Gatt { event } →  on_event(&event); event.accept().send()
//!       └─ _                                  →  ignore
//! }
//! ```
//!
//! ## 事件处理约定 / Event Handling Convention
//!
//! `session()` 负责：
//! `session()` is responsible for:
//! 1. 读取下一个 GATT 事件
//!    Read the next GATT event
//! 2. 调用 `on_event` 回调让 app 层处理事件
//!    Call `on_event` callback to let the app handle the event
//! 3. 自动调用 `event.accept()` 并 `send()` 回复客户端
//!    Automatically call `event.accept()` and `send()` to reply to client
//! 4. 连接断开时自动退出循环
//!    Exit loop automatically on disconnect

use rtt_target::rprintln;
use trouble_host::prelude::*;

/// 驱动单次连接的被动 GATT 事件循环
/// Drive the passive GATT event loop for one connected session.
///
/// 持续从连接中读取 GATT 事件，对每个事件调用 `on_event` 回调，
/// 并自动回复 GATT 响应。连接断开时返回 `Ok(())`。
/// Continuously reads GATT events from the connection, calls `on_event` callback
/// for each event, and auto-replies GATT responses. Returns `Ok(())` on disconnect.
///
/// ## 参数 / Parameters
///
/// - `conn`: 已绑定 AttributeServer 的 GATT 连接
///            GATT connection with AttributeServer bound
/// - `on_event`: 事件处理回调（每次 GATT 读/写请求触发）
///                Event handler callback (fires on each GATT read/write request)
pub async fn session<P, F>(
    conn: &GattConnection<'_, '_, P>,
    mut on_event: F,
) -> Result<(), Error>
where
    P: PacketPool,
    // F: 回调签名 / Callback signature — `FnMut(&GattEvent<'stack, 'server, P>)`
    F: for<'stack, 'server> FnMut(&GattEvent<'stack, 'server, P>),
{
    let reason = loop {
        match conn.next().await {
            // 连接断开，退出事件循环 / Connection lost, exit event loop
            GattConnectionEvent::Disconnected { reason } => break reason,
            // 收到 GATT 事件（读/写请求）/ GATT event received (read/write request)
            GattConnectionEvent::Gatt { event } => {
                // 交给 app 层处理 / Dispatch to app layer
                on_event(&event);

                // 自动回复 GATT 响应 / Auto-reply GATT response
                match event.accept() {
                    Ok(reply) => {
                        let _ = reply.send().await;
                    }
                    Err(e) => rprintln!("[easyble:gatt] accept error: {:?}", e),
                }
            }
            // 其他事件忽略 / Ignore other events
            _ => {}
        }
    };

    rprintln!("[easyble:gatt] disconnected: {:?}", reason);
    Ok(())
}
