//! BLE Central 桌面程序入口 / BLE Central desktop application entry point.
//!
//! 连接外设后持续监控电量通知，断开后自动重连。
//! After connecting to a peripheral, continuously monitors battery notifications.
//! Automatically reconnects on disconnection.
//!
//! ## 运行流程 / Runtime Flow
//!
//! ```text
//! main loop
//!   ├─ connect_session()          扫描并连接外设 / scan & connect
//!   │     ├─ 成功 → monitor_session()
//!   │     └─ 失败 → 等待重连 / wait & retry
//!   └─ monitor_session()
//!         ├─ 读设备信息 / read device info
//!         ├─ 读电量 / read battery
//!         ├─ 读状态 / read status
//!         ├─ Echo 测试 / echo test
//!         ├─ 订阅电量通知 / subscribe battery notifications
//!         └─ select! 循环：通知 vs 定时轮询 / loop: notifications vs periodic polling
//!               ├─ 收到通知 → 打印电量 / notification received → print level
//!               ├─ 定时到期 → 主动读电量 / timer expired → poll battery
//!               └─ 连接丢失 → 返回主循环重连 / connection lost → return to main loop
//! ```

use anyhow::anyhow;
use futures_util::StreamExt;
use hello_ble_central::connect_session;
use std::time::Duration;
use tokio::time::sleep;

/// 重连间隔 / Delay before reconnecting after disconnection or scan failure.
const RECONNECT_DELAY: Duration = Duration::from_secs(2);

/// 定时轮询间隔 / Interval for periodic battery level polling.
const PERIODIC_READ_INTERVAL: Duration = Duration::from_secs(10);

/// 程序入口 / Application entry point.
///
/// 使用单线程 tokio 运行时（BLE 操作无需多线程）。
/// Uses single-threaded tokio runtime (BLE operations don't need multi-threading).
#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    // 初始化日志 / Initialize tracing subscriber
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_target(false) // 隐藏模块路径如 "bluest::windows::adapter" / Hide module paths
        .init();

    // 主循环：扫描→连接→监控→检测到断开→重连 / Main loop: scan→connect→monitor→detect disconnect→reconnect
    loop {
        tracing::info!("scanning for BLE peripheral...");
        let mut session = match connect_session().await {
            Ok(session) => session,
            Err(error) => {
                tracing::error!("{error}");
                tracing::info!("retrying in {} seconds...", RECONNECT_DELAY.as_secs());
                sleep(RECONNECT_DELAY).await;
                continue;
            }
        };

        // 监控外设，直到断开 / Monitor peripheral until disconnection
        if let Err(error) = monitor_session(&mut session).await {
            tracing::error!("{error}");
        }

        tracing::info!("retrying in {} seconds...", RECONNECT_DELAY.as_secs());
        sleep(RECONNECT_DELAY).await;
    }
}

/// 连接后的监控循环 / Post-connection monitoring loop.
///
/// 依次执行一次性操作（读设备信息、电量、状态、Echo），然后进入通知监听循环。
/// Performs one-shot operations (device info, battery, status, echo), then enters
/// the notification listening loop.
async fn monitor_session(session: &mut hello_ble_central::BleSession) -> anyhow::Result<()> {
    // 调试：列出所有已发现的特征 / Debug: list all discovered characteristics
    match session.list_characteristics().await {
        Ok(chars) => {
            tracing::info!("Discovered {} characteristics", chars.len());
            for uuid in &chars {
                tracing::debug!("  - {}", uuid);
            }
        }
        Err(e) => {
            tracing::warn!("Could not list characteristics: {}", e);
        }
    }

    // === 1. Device Info（一次性读取）/ One-shot read ===
    match session.device_info().await {
        Ok(info) => {
            tracing::info!("Device Info:");
            tracing::info!("  Manufacturer: {}", info.manufacturer);
            tracing::info!("  Model: {}", info.model);
            tracing::info!("  Firmware: {}", info.firmware);
            tracing::info!("  Software: {}", info.software);
        }
        Err(e) => tracing::warn!("Could not read device info: {}", e),
    }

    // === 2. Battery（读取当前值）/ Read current value ===
    let level = session.battery_level().await?;
    tracing::info!("Battery level: {}%", level);

    // === 3. Status（读取当前值）/ Read current value ===
    let status = session.status().await?;
    tracing::info!("Status: {}", status);

    // === 4. Echo（写入并等待回传）/ Write and wait for echo back ===
    let test_data = b"Hello, BLE!";
    session.echo(test_data).await?;
    tracing::info!("Echo sent: {:?}", String::from_utf8_lossy(test_data));

    // 订阅电量通知 / Subscribe to battery level notifications
    let mut battery_stream = session.notifications(session.battery_uuid()).await?;

    // 持续监控循环 / Continuous monitoring loop
    //
    // select! 同时等待两个事件 / select! waits for two concurrent events:
    // - 电池通知到达 / battery notification arrives
    // - 定时器到期，主动轮询 / timer expires, poll actively
    loop {
        tokio::select! {
            // 分支 1：收到电量通知 / Branch 1: battery notification received
            notification = battery_stream.next() => {
                match notification {
                    Some(Ok(n)) if n.len() == 1 => {
                        tracing::info!("[notify] Battery: {}%", n[0]);
                    }
                    Some(Ok(n)) => {
                        tracing::info!("[notify] {} bytes", n.len());
                    }
                    Some(Err(e)) => return Err(anyhow!("Notification error: {}", e)),
                    None => return Err(anyhow!("Stream ended")),
                }
            },
            // 分支 2：定时轮询连接状态和电量 / Branch 2: periodic connection check & battery poll
            _ = sleep(PERIODIC_READ_INTERVAL) => {
                if !session.is_connected().await {
                    return Err(anyhow!("Disconnected"));
                }
                let level = session.battery_level().await?;
                tracing::info!("[periodic] Battery: {}%", level);
            }
        }
    }
}
