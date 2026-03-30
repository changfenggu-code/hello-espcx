//! App 层 — 产品特定逻辑 / App layer — product-specific logic.
//!
//! 包含本产品的 GATT server 组装、广播数据构建、GATT 事件处理和主动任务。
//! Contains this product's GATT server assembly, advertisement data building,
//! GATT event handling, and active tasks.
//!
//! ## 职责划分 / Responsibility Split
//!
//! | 模块 / Module | 职责 / Responsibility |
//! |---|---|
//! | `advertising` | 构建广播和扫描响应数据 / Build advertisement & scan response payloads |
//! | `server` | 组装产品的 GATT 服务集合 / Assemble the product's GATT service set |
//! | `session` | 处理 GATT 读写事件 / Handle GATT read/write events |
//! | `tasks` | 运行主动推送任务（电量、echo、bulk 流）/ Run active push tasks |

pub mod advertising;
pub mod runtime;
pub mod server;
pub mod session;
pub mod tasks;
