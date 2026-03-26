use anyhow::anyhow;
use futures_util::StreamExt;
use hello_ble_central::connect_session;
use std::time::Duration;
use tokio::time::sleep;

const RECONNECT_DELAY: Duration = Duration::from_secs(2);
const PERIODIC_READ_INTERVAL: Duration = Duration::from_secs(10);

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    // Filter out noisy bluest adapter warnings
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_target(false) // Hide target names like "bluest::windows::adapter"
        .init();

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

        if let Err(error) = monitor_session(&mut session).await {
            tracing::error!("{error}");
        }

        tracing::info!("retrying in {} seconds...", RECONNECT_DELAY.as_secs());
        sleep(RECONNECT_DELAY).await;
    }
}

async fn monitor_session(session: &mut hello_ble_central::BleSession) -> anyhow::Result<()> {
    // Debug: list discovered characteristics
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

    // === 1. Device Info (read) ===
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

    // === 2. Battery (read + notify) ===
    let level = session.battery_level().await?;
    tracing::info!("Battery level: {}%", level);

    // === 3. Status (read + write + notify) ===
    let status = session.status().await?;
    tracing::info!("Status: {}", status);

    // === 4. Echo (write -> notify) ===
    let test_data = b"Hello, BLE!";
    session.echo(test_data).await?;
    tracing::info!("Echo sent: {:?}", String::from_utf8_lossy(test_data));

    // Subscribe to battery notifications
    let mut battery_stream = session.notifications(session.battery_uuid()).await?;

    // Periodic monitoring
    loop {
        tokio::select! {
            // Battery notification
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
