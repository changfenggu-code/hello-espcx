//! easyble — 基于 `trouble-host` 的分阶段 BLE 外设辅助库
//! easyble — stage-oriented BLE peripheral helpers built on `trouble-host`.
//!
//! ## 架构 / Architecture
//!
//! easyble 将 BLE 外设生命周期拆分为三个阶段，各阶段由 app 层按需组合：
//! easyble splits the BLE peripheral lifecycle into three stages, each wired by the app layer:
//!
//! ```text
//! ┌─ Init（初始化）─────────────┐
//! │  easyble::gap::init()      │  构建 BLE 协议栈，返回 peripheral + runner
//! │  build BLE stack, return peripheral + runner
//! └────────────────────────────┘
//!           ↓
//! ┌─ GAP（广播）────────────────┐
//! │  easyble::gap::advertising() │  单次广播→接受连接，返回 Connection
//! │  single advertise→accept, return Connection
//! └────────────────────────────┘
//!           ↓
//! ┌─ GATT（连接会话）────────────┐
//! │  easyble::gatt::connected()   │  绑定 AttributeServer
//! │  bind AttributeServer
//! │  easyble::gatt::session()    │  驱动 GATT 事件循环
//! │  drive GATT event loop
//! └────────────────────────────┘
//! ```
//!
//! ## 设计原则 / Design Principles
//!
//! - **保持小巧**：只封装 stack setup、advertising、server binding 和事件循环，
//!   不引入额外抽象层。
//! - **Stay small**: only wraps stack setup, advertising, server binding, and event loop.
//!
//! - **App 主导生命周期**：app 层在 `main.rs` 中手动组装各阶段，
//!   easyble 不控制外层循环。
//! - **App owns the lifecycle**: the app layer manually assembles each stage in `main.rs`,
//!   easyble does not control the outer loop.
//!
//! - **'static 资源泄漏**：通过 `Box::leak` 将 `HostResources` 和 `Stack`
//!   泄漏为 `'static`，使 `AttributeServer` 的 `'static` 生命周期合法。
//! - **'static resource leaking**: `Box::leak` on `HostResources` and `Stack`
//!   gives them `'static` lifetime, making `AttributeServer`'s `'static` bound valid.

#![no_std]
extern crate alloc;

pub mod gap;
pub mod gatt;
