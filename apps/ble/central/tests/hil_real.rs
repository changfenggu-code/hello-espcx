use anyhow::anyhow;
use futures_util::StreamExt;
use hello_ble_central::connect_session_with_timeout;
use std::time::{Duration, Instant};

const BULK_TRANSFER_TOTAL_BYTES: usize = 10 * 1024;
const BULK_TIMEOUT: Duration = Duration::from_secs(30);
const ECHO_TIMEOUT: Duration = Duration::from_secs(5);
const STRESS_ROUNDS: usize = 3;

// === Test cases ===

const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);

#[tokio::test]
#[ignore = "requires a connected ESP32-C6 peripheral running hello-espcx firmware"]
async fn esp32c6_end_to_end_hil() -> anyhow::Result<()> {
    let session = connect_session_with_timeout(CONNECT_TIMEOUT).await?;

    // Battery: read
    println!("[hil] read battery");
    let battery = session.battery_level().await?;
    assert!(battery <= 100);

    // Status: read + write
    println!("[hil] write status = true");
    session.set_status(true).await?;
    println!("[hil] read status");
    assert!(session.status().await?);

    println!("[hil] write status = false");
    session.set_status(false).await?;
    println!("[hil] read status");
    assert!(!session.status().await?);

    // Echo: write -> notify
    println!("[hil] echo test");
    let test_data = b"Hello from central!";
    session.echo(test_data).await?;

    // Subscribe to echo notifications
    let mut echo_stream = session.notifications(session.echo_uuid()).await?;

    // Wait for echo reply with timeout
    let reply = tokio::time::timeout(ECHO_TIMEOUT, echo_stream.next())
        .await
        .map_err(|_| anyhow!("Echo timeout"))?
        .ok_or_else(|| anyhow!("Echo stream ended"))??;

    assert_eq!(reply.as_slice(), test_data.as_slice());
    println!("[hil] echo verified: {:?}", String::from_utf8_lossy(&reply));
    drop(echo_stream); // Release borrow before bulk operations

    // Bulk: reset stats
    println!("[hil] reset bulk stats");
    session.reset_bulk_stats().await?;

    // Bulk: upload test pattern
    println!("[hil] upload {} bytes", BULK_TRANSFER_TOTAL_BYTES);
    session
        .upload_test_pattern(BULK_TRANSFER_TOTAL_BYTES)
        .await?;
    let stats = session.read_bulk_stats().await?;
    println!("[hil] stats after upload: {:?}", stats);

    // Bulk: receive stream
    println!("[hil] reset bulk stats");
    session.reset_bulk_stats().await?;
    println!("[hil] start bulk stream");
    session
        .start_bulk_stream(BULK_TRANSFER_TOTAL_BYTES as u32)
        .await?;
    println!("[hil] receive bulk stream");

    let stats = {
        session
            .receive_bulk_stream(BULK_TRANSFER_TOTAL_BYTES, BULK_TIMEOUT)
            .await?;
        session.read_bulk_stats().await?
    };
    println!("[hil] stats after stream: {:?}", stats);

    session.disconnect().await?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires a connected ESP32-C6 peripheral running hello-espcx firmware"]
async fn esp32c6_bulk_stress_hil() -> anyhow::Result<()> {
    let session = connect_session_with_timeout(CONNECT_TIMEOUT).await?;

    for round in 1..=STRESS_ROUNDS {
        println!(
            "[hil] round {round}/{STRESS_ROUNDS}: upload {} bytes",
            BULK_TRANSFER_TOTAL_BYTES
        );
        session.reset_bulk_stats().await?;
        let upload_started = Instant::now();
        session
            .upload_test_pattern(BULK_TRANSFER_TOTAL_BYTES)
            .await?;
        print_throughput(
            "upload",
            BULK_TRANSFER_TOTAL_BYTES,
            upload_started.elapsed(),
        );

        println!(
            "[hil] round {round}/{STRESS_ROUNDS}: receive {} bytes",
            BULK_TRANSFER_TOTAL_BYTES
        );
        session.reset_bulk_stats().await?;
        let notify_started = Instant::now();
        session
            .start_bulk_stream(BULK_TRANSFER_TOTAL_BYTES as u32)
            .await?;
        session
            .receive_bulk_stream(BULK_TRANSFER_TOTAL_BYTES, BULK_TIMEOUT)
            .await?;
        print_throughput(
            "notify",
            BULK_TRANSFER_TOTAL_BYTES,
            notify_started.elapsed(),
        );
    }

    session.disconnect().await?;
    Ok(())
}

fn print_throughput(label: &str, total_bytes: usize, elapsed: Duration) {
    let kib = total_bytes as f64 / 1024.0;
    let seconds = elapsed.as_secs_f64();
    let kib_per_sec = if seconds > 0.0 { kib / seconds } else { 0.0 };
    println!(
        "[hil] {label}: {:.1} KiB in {:.2}s -> {:.1} KiB/s",
        kib, seconds, kib_per_sec
    );
}
