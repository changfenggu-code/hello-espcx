//! Bulk Service — 大批量数据传输 / Bulk Service — large data transfer.
//!
//! 自定义服务，支持双向大批量数据传输，分为三个特征：
//! Custom service supporting bidirectional bulk data transfer, with three characteristics:
//!
//! - **control**: 控制命令（Idle / ResetStats / StartStream）
//! - **data**: 实际数据传输（write = 上传，notify = 下发）
//! - **stats**: 传输统计（rx/tx 字节数）
//!
//! ```text
//! 上行 / Upload:   Central ──write──▶ Peripheral（逐块写入 / chunk-by-chunk write）
//! 下行 / Download: Peripheral ──notify──▶ Central（流式推送 / stream via notify）
//! ```

use heapless::Vec;
use postcard::to_slice;
use trouble_host::prelude::*;

use hello_ble_common::bulk;

/// Bulk Service / 批量传输服务（自定义）。
///
/// 支持两个方向 / Supports two directions:
/// - **上传（Upload）**: Central 通过 write 写入数据块 / Central writes data chunks
/// - **下发（Download）**: Peripheral 收到 StartStream 后通过 notify 逐块推送 / Peripheral notifies chunks after StartStream
#[gatt_service(uuid = bulk::SERVICE_UUID)]
pub struct BulkService {
    /// 控制特征：接收命令（Idle / ResetStats / StartStream）/ Control: receives commands.
    #[characteristic(uuid = bulk::CONTROL_UUID, write, read, value = initial_bulk_control_value())]
    pub control: Vec<u8, { bulk::CONTROL_CAPACITY }>,
    /// 数据特征：双向传输（write = 上传，notify = 下发）/ Data: bidirectional transfer.
    #[characteristic(uuid = bulk::CHUNK_UUID, write, write_without_response, notify, value = Vec::new())]
    pub data: Vec<u8, { bulk::CHUNK_SIZE }>,
    /// 统计特征：只读，反映 rx/tx 字节计数 / Stats: read-only, reflects rx/tx byte counters.
    #[characteristic(uuid = bulk::STATS_UUID, read, value = initial_bulk_stats_value())]
    pub stats: Vec<u8, { bulk::STATS_CAPACITY }>,
}

/// 生成 Bulk 控制特征的初始值（`Idle` 命令） / Generate initial value for bulk control (`Idle` command).
///
/// 将 `BulkControlCommand::Idle` 通过 postcard 序列化。
/// Serializes `BulkControlCommand::Idle` via postcard.
pub fn initial_bulk_control_value() -> Vec<u8, { bulk::CONTROL_CAPACITY }> {
    let mut buf = [0u8; bulk::CONTROL_CAPACITY];
    let used = to_slice(&bulk::BulkControlCommand::Idle, &mut buf).unwrap();
    Vec::from_slice(used).unwrap()
}

/// 生成 Bulk 统计特征的初始值（rx=0, tx=0） / Generate initial value for bulk stats (rx=0, tx=0).
///
/// 将 `BulkStats::default()` 通过 postcard 序列化。
/// Serializes `BulkStats::default()` via postcard.
pub fn initial_bulk_stats_value() -> Vec<u8, { bulk::STATS_CAPACITY }> {
    let mut buf = [0u8; bulk::STATS_CAPACITY];
    let used = to_slice(&bulk::BulkStats::default(), &mut buf).unwrap();
    Vec::from_slice(used).unwrap()
}
