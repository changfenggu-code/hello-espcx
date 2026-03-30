//! GATT 层 — AttributeServer 绑定和连接会话事件驱动
//! GATT layer — attribute server binding and connected session event driving.
//!
//! ## 职责 / Responsibilities
//!
//! GATT 层负责连接建立后的会话处理：
//! The GATT layer handles post-connection session processing:
//!
//! - `connected`: 将 `Connection` 与 `AttributeServer` 绑定，得到 `GattConnection`
//!   Bind `Connection` with `AttributeServer` to get `GattConnection`
//! - `session`: 驱动 GATT 事件循环，通过回调分派每个读写事件
//!   Drive GATT event loop, dispatching each read/write via callback
//!
//! ## 生命周期 / Lifecycle
//!
//! ```text
//! Connection
//!   └─ connected(server)  →  GattConnection
//!       └─ session(|event| ...)  →  直到连接断开
//! ```

pub mod connected;
pub mod session;

// Re-export for convenience / 导出常用函数，方便调用方使用
pub use connected::connected;
pub use session::session;
