//! BLE Peripheral with standard and custom test services
#![allow(clippy::needless_borrows_for_generic_args)]
#![allow(dead_code)] // device_info_service provided for standard BLE compliance

use core::sync::atomic::{AtomicU32, Ordering};
use embassy_futures::join::join;
use embassy_futures::select::select;
use embassy_time::Timer;
use esp_hal::system::software_reset;
use heapless::Vec;
use postcard::{from_bytes, to_slice};
use hello_ble_common::{
    BulkControlCommand, BulkStats, SERVICE_BATTERY_UUID16, BULK_CONTROL_CAPACITY,
    BULK_CONTROL_UUID, BULK_CHUNK_UUID, BULK_CHUNK_SIZE, SERVICE_BULK_UUID,
    BULK_STATS_CAPACITY, BULK_STATS_UUID, ECHO_CAPACITY, ECHO_UUID,
    SERVICE_ECHO_UUID, PERIPHERAL_ADDRESS, PERIPHERAL_NAME,
    SERVICE_STATUS_UUID, STATUS_CAPACITY, STATUS_UUID,
};
use rtt_target::rprintln;
use trouble_host::prelude::*;

/// Max number of connections
const CONNECTIONS_MAX: usize = 1;
/// Max number of L2CAP channels
const L2CAP_CHANNELS_MAX: usize = 2;

/// Fill buffer with test pattern for bulk transfer verification
fn fill_test_pattern(start_offset: usize, buffer: &mut [u8]) {
    for (index, byte) in buffer.iter_mut().enumerate() {
        *byte = ((((start_offset + index) * 17) + 29) % 256) as u8;
    }
}

// GATT Server with all services
#[gatt_server]
struct Server {
    battery_service: BatteryService,
    device_info_service: DeviceInfoService,
    echo_service: EchoService,
    status_service: StatusService,
    bulk_service: BulkService,
}

// Battery Service (standard BLE): read + notify
#[gatt_service(uuid = service::BATTERY)]
struct BatteryService {
    #[characteristic(uuid = characteristic::BATTERY_LEVEL, read, notify, value = 50)]
    level: u8,
}

// Device Information Service (standard BLE): read only
#[gatt_service(uuid = service::DEVICE_INFORMATION)]
struct DeviceInfoService {
    #[characteristic(uuid = characteristic::MANUFACTURER_NAME_STRING, read, value = "ESP")]
    manufacturer: &'static str,
    #[characteristic(uuid = characteristic::MODEL_NUMBER_STRING, read, value = "ESP32-C6")]
    model: &'static str,
    #[characteristic(uuid = characteristic::FIRMWARE_REVISION_STRING, read, value = "1.0.0")]
    firmware: &'static str,
    #[characteristic(uuid = characteristic::SOFTWARE_REVISION_STRING, read, value = env!("CARGO_PKG_VERSION"))]
    software: &'static str,
}

// Echo Service: write -> notify
#[gatt_service(uuid = SERVICE_ECHO_UUID)]
struct EchoService {
    #[characteristic(uuid = ECHO_UUID, write, notify, value = Vec::new())]
    echo: Vec<u8, ECHO_CAPACITY>,
}

// Status Service: read + write + notify
#[gatt_service(uuid = SERVICE_STATUS_UUID)]
struct StatusService {
    #[characteristic(uuid = STATUS_UUID, read, write, notify, value = initial_status_value())]
    status: Vec<u8, STATUS_CAPACITY>,
}

// Bulk Service: control + data transfer + stats
#[gatt_service(uuid = SERVICE_BULK_UUID)]
struct BulkService {
    #[characteristic(uuid = BULK_CONTROL_UUID, write, read, value = initial_bulk_control_value())]
    control: Vec<u8, BULK_CONTROL_CAPACITY>,
    #[characteristic(uuid = BULK_CHUNK_UUID, write, write_without_response, notify, value = Vec::new())]
    data: Vec<u8, BULK_CHUNK_SIZE>,
    #[characteristic(uuid = BULK_STATS_UUID, read, value = initial_bulk_stats_value())]
    stats: Vec<u8, BULK_STATS_CAPACITY>,
}

// Bulk transfer stats
static RX_BYTES: AtomicU32 = AtomicU32::new(0);
static TX_BYTES: AtomicU32 = AtomicU32::new(0);

/// Run the BLE stack as a peripheral
pub async fn run<C>(controller: C)
where
    C: Controller,
{
    let address: Address = Address::random(PERIPHERAL_ADDRESS);
    rprintln!("Our address = {:?}", address);

    let mut resources: HostResources<DefaultPacketPool, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX> = HostResources::new();
    let stack = trouble_host::new(controller, &mut resources).set_random_address(address);
    let Host {
        mut peripheral, runner, ..
    } = stack.build();

    let server = Server::new_with_config(GapConfig::Peripheral(PeripheralConfig {
        name: PERIPHERAL_NAME,
        appearance: &appearance::power_device::GENERIC_POWER_DEVICE,
    }))
    .unwrap();

    rprintln!("Starting advertising with 4 services");

    let _ = join(ble_task(runner), async {
        loop {
            match advertise(&mut peripheral, &server).await {
                Ok(conn) => {
                    let a = gatt_events_task(&server, &conn);
                    let b = custom_task::<C, DefaultPacketPool>(&server, &conn);
                    select(a, b).await;
                }
                Err(e) => {
                    log_error_and_reset("adv", &e).await;
                }
            }
        }
    })
    .await;
}

async fn advertise<'values, 'server, C: Controller>(
    peripheral: &mut Peripheral<'values, C, DefaultPacketPool>,
    server: &'server Server<'values>,
) -> Result<GattConnection<'values, 'server, DefaultPacketPool>, BleHostError<C::Error>> {
    let mut advertiser_data = [0; 31];
    let len = AdStructure::encode_slice(
        &[
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
            AdStructure::ServiceUuids16(&[SERVICE_BATTERY_UUID16.to_le_bytes()]),
            AdStructure::CompleteLocalName(PERIPHERAL_NAME.as_bytes()),
        ],
        &mut advertiser_data[..],
    )?;

    let advertiser = peripheral
        .advertise(
            &Default::default(),
            Advertisement::ConnectableScannableUndirected {
                adv_data: &advertiser_data[..len],
                scan_data: &[],
            },
        )
        .await?;

    rprintln!("[adv] advertising");
    let conn = advertiser.accept().await?.with_attribute_server(server)?;
    rprintln!("[adv] connection established");
    Ok(conn)
}

async fn ble_task<C: Controller, P: PacketPool>(mut runner: Runner<'_, C, P>) {
    loop {
        if let Err(e) = runner.run().await {
            log_error_and_reset("ble_task", &e).await;
        }
    }
}

async fn gatt_events_task<P: PacketPool>(
    server: &Server<'_>,
    conn: &GattConnection<'_, '_, P>,
) -> Result<(), Error> {
    let level = server.battery_service.level;
    let echo = server.echo_service.echo.clone();
    let status = server.status_service.status.clone();
    let bulk_control = server.bulk_service.control.clone();
    let bulk_data = server.bulk_service.data.clone();
    let bulk_stats = server.bulk_service.stats.clone();

    let reason = loop {
        match conn.next().await {
            GattConnectionEvent::Disconnected { reason } => break reason,
            GattConnectionEvent::Gatt { event } => {
                match &event {
                    // Battery: read
                    GattEvent::Read(event) if event.handle() == level.handle => {
                        rprintln!("[battery] read");
                    }
                    // Echo: write -> will echo back in custom_task
                    GattEvent::Write(event) if event.handle() == echo.handle => {
                        rprintln!("[echo] write {} bytes", event.data().len());
                    }
                    // Status: read/write
                    GattEvent::Read(event) if event.handle() == status.handle => {
                        match server.get(&status) {
                            Ok(raw) => {
                                let val: Result<bool, _> = from_bytes(&raw);
                                rprintln!("[status] read: {:?}", val);
                            }
                            Err(e) => rprintln!("[status] read error: {:?}", e),
                        }
                    }
                    GattEvent::Write(event) if event.handle() == status.handle => {
                        match from_bytes::<bool>(event.data()) {
                            Ok(val) => rprintln!("[status] write: {}", val),
                            Err(e) => rprintln!("[status] write error: {:?}", e),
                        }
                    }
                    // Bulk: control write
                    GattEvent::Write(event) if event.handle() == bulk_control.handle => {
                        match from_bytes::<BulkControlCommand>(event.data()) {
                            Ok(cmd) => {
                                rprintln!("[bulk] control: {:?}", cmd);
                                if cmd == BulkControlCommand::ResetStats {
                                    reset_stats();
                                    sync_bulk_stats(server, &bulk_stats);
                                }
                            }
                            Err(e) => rprintln!("[bulk] control error: {:?}", e),
                        }
                    }
                    // Bulk: data write
                    GattEvent::Write(event) if event.handle() == bulk_data.handle => {
                        rprintln!("[bulk] data write {} bytes", event.data().len());
                        record_rx(event.data());
                        sync_bulk_stats(server, &bulk_stats);
                    }
                    // Bulk: stats read
                    GattEvent::Read(event) if event.handle() == bulk_stats.handle => {
                        // Stats are synced automatically
                    }
                    _ => {}
                };
                match event.accept() {
                    Ok(reply) => reply.send().await,
                    Err(e) => rprintln!("[gatt] error sending response: {:?}", e),
                };
            }
            _ => {}
        }
    };
    rprintln!("[gatt] disconnected: {:?}", reason);
    Ok(())
}

async fn custom_task<C: Controller, P: PacketPool>(
    server: &Server<'_>,
    conn: &GattConnection<'_, '_, P>,
) {
    let level = server.battery_service.level;
    let echo = server.echo_service.echo.clone();
    let bulk_control = server.bulk_service.control.clone();
    let bulk_data = server.bulk_service.data.clone();
    let bulk_stats = server.bulk_service.stats.clone();

    let mut battery_tick: u8 = 0;

    loop {
        // Check for bulk stream start
        if let Ok(raw) = server.get(&bulk_control) {
            if let Ok(BulkControlCommand::StartStream { total_bytes }) = from_bytes::<BulkControlCommand>(&raw) {
                rprintln!("[bulk] starting stream: {} bytes", total_bytes);
                run_bulk_stream(server, conn, &bulk_stats, &bulk_data, &bulk_control, total_bytes).await;
                continue;
            }
        }

        // Check for echo data
        if let Ok(data) = server.get(&echo) {
            if !data.is_empty() {
                rprintln!("[echo] notifying {} bytes", data.len());
                if echo.notify(conn, &data).await.is_err() {
                    rprintln!("[echo] notify failed");
                }
                // Clear echo
                let _ = echo.set(server, &Vec::new());
            }
        }

        // Battery notification (every 2 seconds)
        battery_tick = battery_tick.wrapping_add(1);
        if level.notify(conn, &battery_tick).await.is_err() {
            break;
        }

        Timer::after_secs(2).await;
    }
}

async fn run_bulk_stream<P: PacketPool>(
    server: &Server<'_>,
    conn: &GattConnection<'_, '_, P>,
    bulk_stats: &Characteristic<Vec<u8, BULK_STATS_CAPACITY>>,
    bulk_data: &Characteristic<Vec<u8, BULK_CHUNK_SIZE>>,
    bulk_control: &Characteristic<Vec<u8, BULK_CONTROL_CAPACITY>>,
    total_bytes: u32,
) {
    let mut chunk = [0u8; BULK_CHUNK_SIZE];

    TX_BYTES.store(0, Ordering::Relaxed);
    sync_bulk_stats(server, bulk_stats);

    let total = total_bytes as usize;
    for offset in (0..total).step_by(BULK_CHUNK_SIZE) {
        let len = (total - offset).min(BULK_CHUNK_SIZE);
        fill_test_pattern(offset, &mut chunk[..len]);
        let payload = match Vec::from_slice(&chunk[..len]) {
            Ok(v) => v,
            Err(_) => break,
        };

        if bulk_data.notify(conn, &payload).await.is_err() {
            rprintln!("[bulk] notify error");
            break;
        }
        record_tx(&payload);
        sync_bulk_stats(server, bulk_stats);
    }

    // Reset control to Idle
    let _ = bulk_control.set(server, &initial_bulk_control_value());
    rprintln!("[bulk] stream complete: {} bytes", total_bytes);
}

fn reset_stats() {
    RX_BYTES.store(0, Ordering::Relaxed);
    TX_BYTES.store(0, Ordering::Relaxed);
}

fn record_rx(data: &[u8]) {
    RX_BYTES.fetch_add(data.len() as u32, Ordering::Relaxed);
}

fn record_tx(data: &[u8]) {
    TX_BYTES.fetch_add(data.len() as u32, Ordering::Relaxed);
}

fn sync_bulk_stats(server: &Server<'_>, bulk_stats: &Characteristic<Vec<u8, BULK_STATS_CAPACITY>>) {
    let stats = BulkStats {
        rx_bytes: RX_BYTES.load(Ordering::Relaxed),
        tx_bytes: TX_BYTES.load(Ordering::Relaxed),
    };
    let mut buf = [0u8; BULK_STATS_CAPACITY];
    if let Ok(used) = to_slice(&stats, &mut buf) {
        if let Ok(vec) = Vec::from_slice(used) {
            let _ = bulk_stats.set(server, &vec);
        }
    }
}

fn initial_status_value() -> Vec<u8, STATUS_CAPACITY> {
    let mut buf = [0u8; STATUS_CAPACITY];
    let used = to_slice(&false, &mut buf).unwrap();
    Vec::from_slice(used).unwrap()
}

fn initial_bulk_control_value() -> Vec<u8, BULK_CONTROL_CAPACITY> {
    let mut buf = [0u8; BULK_CONTROL_CAPACITY];
    let used = to_slice(&BulkControlCommand::Idle, &mut buf).unwrap();
    Vec::from_slice(used).unwrap()
}

fn initial_bulk_stats_value() -> Vec<u8, BULK_STATS_CAPACITY> {
    let mut buf = [0u8; BULK_STATS_CAPACITY];
    let used = to_slice(&BulkStats::default(), &mut buf).unwrap();
    Vec::from_slice(used).unwrap()
}

async fn log_error_and_reset<E: core::fmt::Debug>(context: &str, error: &E) -> ! {
    rprintln!("[fatal:{}] {:?}", context, error);
    rprintln!("[fatal:{}] resetting...", context);
    Timer::after_millis(100).await;
    software_reset()
}
