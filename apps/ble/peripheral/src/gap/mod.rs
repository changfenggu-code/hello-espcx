//! GAP 层 — 外设发现与连接管理 / GAP layer — peripheral discovery and connection management.
//!
//! 负责广播构建、连接接受和断线重连循环。
//! Responsible for advertisement building, connection acceptance, and reconnect loop.

pub mod advertising;
pub mod peripheral_loop;
