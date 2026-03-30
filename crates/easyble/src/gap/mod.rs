//! GAP 层 — 协议栈初始化、驱动循环和广播操作
//! GAP layer — stack initialization, stack driving, and advertising operations.
//!
//! ## 职责 / Responsibilities
//!
//! GAP 层负责 BLE 协议栈的初始化、驱动和广播发起：
//! The GAP layer is responsible for BLE stack initialization, driving, and advertising:
//!
//! - `init`: 分配资源、设置地址、返回 stack 组件
//! - `run_stack`: 驱动底层 BLE 协议处理
//! - `advertising`: 启动一次广播并等待 Central 连接
//! - `init`: allocate resources, set address, return stack components
//! - `run_stack`: drive underlying BLE protocol processing
//! - `advertising`: start one advertising attempt and wait for Central connection

pub mod advertising;
pub mod init;

pub use advertising::{advertising, AdvertisementData, AdvertisementView};
pub use init::{init, run_stack, InitConfig, InitializedStack};
