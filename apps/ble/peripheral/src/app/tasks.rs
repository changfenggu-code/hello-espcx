//! 产品主动任务 / Product active tasks.
//!
//! 运行连接期间的主动推送行为：电量定时通知、echo 回传、bulk 下发流。
//! Runs active push behaviors during a connection: periodic battery notifications,
//! echo replay, and bulk download stream.
//!
//! ## 主循环逻辑 / Main Loop Logic
//!
//! ```text
//! loop {
//!     1. 检查 bulk_control → 如果是 StartStream，执行下发流 / Check bulk control → run stream
//!     2. 检查 echo → 如果有写入数据，notify 回传 / Check echo → notify back if written
//!     3. 递增电量计数器并 notify / Increment battery counter and notify
//!     4. 等待 2 秒 / Wait 2 seconds
//! }
//! ```

use crate::app::server::Server;
use crate::services::initial_bulk_control_value;
use core::sync::atomic::{AtomicU32, Ordering};
use embassy_time::Timer;
use heapless::Vec;
use hello_ble_common::{bulk, fill_test_pattern};
use postcard::from_bytes;
use rtt_target::rprintln;
use trouble_host::prelude::*;

/// 累计接收字节数（Central → Peripheral 上行方向）/ Cumulative received bytes (Central → Peripheral upload).
static RX_BYTES: AtomicU32 = AtomicU32::new(0);

/// 累计发送字节数（Peripheral → Central 下发方向）/ Cumulative sent bytes (Peripheral → Central download).
static TX_BYTES: AtomicU32 = AtomicU32::new(0);

/// 运行连接期间的主动推送任务 / Run product-specific active tasks during a connected session.
///
/// 在后台持续运行，直到 notify 失败（连接断开）时退出。
/// Runs continuously in the background until a notify fails (connection lost).
pub(crate) async fn custom_task<P: PacketPool>(
    server: &Server<'_>,
    conn: &GattConnection<'_, '_, P>,
) {
    // 克隆特征 handle 供异步循环使用 / Clone characteristic handles for use in async loop
    let level = server.battery_service.level;
    let echo = server.echo_service.echo.clone();
    let bulk_control = server.bulk_service.control.clone();
    let bulk_data = server.bulk_service.data.clone();
    let bulk_stats = server.bulk_service.stats.clone();

    let mut battery_tick: u8 = 0;

    loop {
        // 1. 检查是否收到 StartStream 命令 / Check for StartStream command
        if let Ok(raw) = server.get(&bulk_control) {
            if let Ok(bulk::BulkControlCommand::StartStream { total_bytes }) =
                from_bytes::<bulk::BulkControlCommand>(&raw)
            {
                rprintln!("[bulk] starting stream: {} bytes", total_bytes);
                run_bulk_stream(
                    server,
                    conn,
                    &bulk_stats,
                    &bulk_data,
                    &bulk_control,
                    total_bytes,
                )
                .await;
                continue; // 流结束后跳过本轮剩余步骤 / Skip rest of loop after stream
            }
        }

        // 2. 检查 echo 写入并回传 / Check echo write and notify back
        if let Ok(data) = server.get(&echo) {
            if !data.is_empty() {
                rprintln!("[echo] notifying {} bytes", data.len());
                if echo.notify(conn, &data).await.is_err() {
                    rprintln!("[echo] notify failed");
                }
                let _ = echo.set(server, &Vec::new()); // 清空，防止重复回传 / Clear to prevent re-notify
            }
        }

        // 3. 电量计数器递增并通知 / Increment battery counter and notify
        battery_tick = battery_tick.wrapping_add(1);
        if level.notify(conn, &battery_tick).await.is_err() {
            break; // notify 失败 → 连接已断开 / Notify failed → connection lost
        }

        // 4. 等待 2 秒 / Wait 2 seconds
        Timer::after_secs(2).await;
    }
}

/// 执行一次 bulk 下发流 / Execute one bulk download stream.
///
/// 用确定性测试模式填满每个 chunk，通过 notify 逐块发送直到达到 `total_bytes`。
/// Fills each chunk with a deterministic test pattern and sends via notify until `total_bytes`.
///
/// 每块发送后同步统计到 GATT 特征，供 Central 读取验证。
/// After each chunk, syncs stats to the GATT characteristic for Central to verify.
async fn run_bulk_stream<P: PacketPool>(
    server: &Server<'_>,
    conn: &GattConnection<'_, '_, P>,
    bulk_stats: &Characteristic<Vec<u8, { bulk::STATS_CAPACITY }>>,
    bulk_data: &Characteristic<Vec<u8, { bulk::CHUNK_SIZE }>>,
    bulk_control: &Characteristic<Vec<u8, { bulk::CONTROL_CAPACITY }>>,
    total_bytes: u32,
) {
    let mut chunk = [0u8; bulk::CHUNK_SIZE];

    // 重置发送计数 / Reset tx counter
    TX_BYTES.store(0, Ordering::Relaxed);
    sync_bulk_stats(server, bulk_stats);

    let total = total_bytes as usize;
    for offset in (0..total).step_by(bulk::CHUNK_SIZE) {
        let len = (total - offset).min(bulk::CHUNK_SIZE);

        // 生成确定性测试数据 / Generate deterministic test pattern
        fill_test_pattern(offset, &mut chunk[..len]);
        let payload = match Vec::from_slice(&chunk[..len]) {
            Ok(v) => v,
            Err(_) => break,
        };

        // 发送并记录 / Send and record
        if bulk_data.notify(conn, &payload).await.is_err() {
            rprintln!("[bulk] notify error");
            break;
        }
        record_tx(&payload);
        sync_bulk_stats(server, bulk_stats);
    }

    // 流结束，将控制命令重置为 Idle / Stream done, reset control to Idle
    let _ = bulk_control.set(server, &initial_bulk_control_value());
    rprintln!("[bulk] stream complete: {} bytes", total_bytes);
}

// ============================================================================
// 统计辅助函数 / Stats helpers
// ============================================================================

/// 重置所有传输统计 / Reset all transfer stats (rx and tx to zero).
pub(crate) fn reset_stats() {
    RX_BYTES.store(0, Ordering::Relaxed);
    TX_BYTES.store(0, Ordering::Relaxed);
}

/// 记录接收到的数据量（上行方向）/ Record received data length (upload direction).
pub(crate) fn record_rx(data: &[u8]) {
    RX_BYTES.fetch_add(data.len() as u32, Ordering::Relaxed);
}

/// 记录发送的数据量（下发方向）/ Record sent data length (download direction).
fn record_tx(data: &[u8]) {
    TX_BYTES.fetch_add(data.len() as u32, Ordering::Relaxed);
}

/// 将当前统计同步到 GATT bulk_stats 特征 / Sync current stats to the GATT bulk_stats characteristic.
///
/// Central 可以通过读取此特征来验证传输完整性。
/// Central can read this characteristic to verify transfer integrity.
pub(crate) fn sync_bulk_stats(
    server: &Server<'_>,
    bulk_stats: &Characteristic<Vec<u8, { bulk::STATS_CAPACITY }>>,
) {
    let stats = bulk::BulkStats {
        rx_bytes: RX_BYTES.load(Ordering::Relaxed),
        tx_bytes: TX_BYTES.load(Ordering::Relaxed),
    };
    let mut buf = [0u8; bulk::STATS_CAPACITY];
    if let Ok(used) = postcard::to_slice(&stats, &mut buf) {
        if let Ok(vec) = Vec::from_slice(used) {
            let _ = bulk_stats.set(server, &vec);
        }
    }
}
