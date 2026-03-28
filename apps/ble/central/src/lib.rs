use anyhow::{Error, anyhow};
use futures_util::StreamExt;
use hello_ble_common::{PERIPHERAL_NAME, battery, bulk, device_info, echo, status};
use std::time::Duration;
use tokio::time::sleep;
use btleplus::{Adapter, BluetoothUuidExt, Client, Result as BtleplusResult, ScanFilter, Uuid};

const SCAN_TIMEOUT: Duration = Duration::from_secs(30);

/// Device information from peripheral
#[derive(Debug)]
pub struct DeviceInfo {
    pub manufacturer: String,
    pub model: String,
    pub firmware: String,
    pub software: String,
}

pub struct BleSession {
    /// GATT client for attribute operations on the connected peripheral.
    gatt: Client,
    battery_uuid: Uuid,
    // Device Info UUIDs
    manufacturer_uuid: Uuid,
    model_uuid: Uuid,
    firmware_uuid: Uuid,
    software_uuid: Uuid,
    // Custom service UUIDs
    echo_uuid: Uuid,
    status_uuid: Uuid,
    bulk_control_uuid: Uuid,
    bulk_data_uuid: Uuid,
    bulk_stats_uuid: Uuid,
}

impl BleSession {
    /// Read battery level (0-100%)
    pub async fn battery_level(&self) -> Result<u8, Error> {
        let bytes = self.gatt.read(self.battery_uuid).await?;
        if bytes.len() != 1 {
            return Err(anyhow!("Expected 1 byte, got {}", bytes.len()));
        }
        Ok(bytes[0])
    }

    /// Read device info strings
    pub async fn device_info(&self) -> Result<DeviceInfo, Error> {
        let manufacturer = self.gatt.read_string(self.manufacturer_uuid).await?;
        let model = self.gatt.read_string(self.model_uuid).await?;
        let firmware = self.gatt.read_string(self.firmware_uuid).await?;
        let software = self.gatt.read_string(self.software_uuid).await?;
        Ok(DeviceInfo {
            manufacturer,
            model,
            firmware,
            software,
        })
    }

    /// Read status value (uses postcard)
    pub async fn status(&self) -> Result<bool, Error> {
        self.gatt
            .read_typed(self.status_uuid)
            .await
            .map_err(|e| anyhow!("{}", e))
    }

    /// Write status value (uses postcard)
    pub async fn set_status(&self, value: bool) -> Result<(), Error> {
        self.gatt
            .write_typed(self.status_uuid, &value, true)
            .await
            .map_err(|e| anyhow!("{}", e))
    }

    /// Echo: write data and wait for notification
    pub async fn echo(&self, data: &[u8]) -> Result<(), Error> {
        if data.len() > echo::CAPACITY {
            return Err(anyhow!(
                "Echo data too large: {} > {}",
                data.len(),
                echo::CAPACITY
            ));
        }
        self.gatt.write(self.echo_uuid, data, true).await?;
        Ok(())
    }

    /// Read bulk stats (uses postcard)
    pub async fn read_bulk_stats(&self) -> Result<bulk::BulkStats, Error> {
        self.gatt
            .read_typed(self.bulk_stats_uuid)
            .await
            .map_err(|e| anyhow!("{}", e))
    }

    /// Reset bulk transfer stats (uses postcard)
    pub async fn reset_bulk_stats(&self) -> Result<(), Error> {
        self.gatt
            .write_typed(
                self.bulk_control_uuid,
                &bulk::BulkControlCommand::ResetStats,
                true,
            )
            .await
            .map_err(|e| anyhow!("{}", e))?;

        // Wait for stats to be reset
        for _ in 0..30 {
            sleep(Duration::from_millis(100)).await;
            let stats = self.read_bulk_stats().await?;
            if stats == bulk::BulkStats::default() {
                return Ok(());
            }
        }
        Err(anyhow!("Timeout waiting for stats reset"))
    }

    /// Start bulk notify stream on peripheral (uses postcard)
    pub async fn start_bulk_stream(&self, total_bytes: u32) -> Result<(), Error> {
        self.gatt
            .write_typed(
                self.bulk_control_uuid,
                &bulk::BulkControlCommand::StartStream { total_bytes },
                true,
            )
            .await
            .map_err(|e| anyhow!("{}", e))
    }

    /// Upload data via bulk_data characteristic (write without response)
    pub async fn upload_bulk_data(&self, data: &[u8]) -> Result<(), Error> {
        if data.len() > bulk::CHUNK_SIZE {
            return Err(anyhow!(
                "Data too large: {} > {}",
                data.len(),
                bulk::CHUNK_SIZE
            ));
        }
        self.gatt.write(self.bulk_data_uuid, data, false).await?;
        Ok(())
    }

    /// Upload a test pattern to peripheral in chunks, verifying with stats.
    ///
    /// Uses `fill_test_pattern` to generate verifiable data, then sends via
    /// `upload_bulk_data`. After upload, call `read_bulk_stats` to verify
    /// `rx_bytes` matches `total_bytes`.
    pub async fn upload_test_pattern(&self, total_bytes: usize) -> Result<(), Error> {
        let mut chunk = [0u8; bulk::CHUNK_SIZE];
        for offset in (0..total_bytes).step_by(bulk::CHUNK_SIZE) {
            let len = (total_bytes - offset).min(bulk::CHUNK_SIZE);
            hello_ble_common::fill_test_pattern(offset, &mut chunk[..len]);
            self.upload_bulk_data(&chunk[..len]).await?;
        }
        Ok(())
    }

    /// Receive a bulk notify stream and verify data integrity.
    ///
    /// Subscribes to bulk_data notifications, receives `total_bytes` of data,
    /// and verifies each chunk matches the expected test pattern.
    pub async fn receive_bulk_stream(
        &self,
        total_bytes: usize,
        timeout: Duration,
    ) -> Result<(), Error> {
        let mut stream = self.notifications(self.bulk_data_uuid).await?;
        let mut received = 0usize;
        let mut expected = [0u8; bulk::CHUNK_SIZE];

        while received < total_bytes {
            let next = tokio::time::timeout(timeout, stream.next())
                .await
                .map_err(|_| anyhow!("Timeout waiting for bulk data"))?;
            let next = next.ok_or_else(|| anyhow!("Stream ended"))??;

            let chunk_len = next.len();
            let expected_len = (total_bytes - received).min(bulk::CHUNK_SIZE);

            if chunk_len != expected_len {
                return Err(anyhow!(
                    "Unexpected chunk size: {}, expected {}",
                    chunk_len,
                    expected_len
                ));
            }

            hello_ble_common::fill_test_pattern(received, &mut expected[..chunk_len]);
            if next.as_slice() != &expected[..chunk_len] {
                return Err(anyhow!("Bulk data mismatch at offset {}", received));
            }
            received += chunk_len;
        }
        Ok(())
    }

    /// Disconnect from peripheral
    pub async fn disconnect(&self) -> Result<(), Error> {
        self.gatt.connection().disconnect().await?;
        Ok(())
    }

    pub async fn is_connected(&self) -> bool {
        self.gatt.connection().is_connected().await
    }

    /// Get notifications stream for a specific characteristic
    pub async fn notifications(
        &self,
        uuid: Uuid,
    ) -> Result<impl StreamExt<Item = Result<Vec<u8>, Error>> + Unpin + '_, Error> {
        use futures_util::StreamExt;
        let stream = self.gatt.notifications(uuid).await?;
        Ok(stream.map(|r| r.map_err(|e| anyhow!("{}", e))))
    }

    pub fn battery_uuid(&self) -> Uuid {
        self.battery_uuid
    }

    pub fn echo_uuid(&self) -> Uuid {
        self.echo_uuid
    }

    pub fn bulk_data_uuid(&self) -> Uuid {
        self.bulk_data_uuid
    }

    /// Debug: list all discovered characteristic UUIDs
    pub async fn list_characteristics(&self) -> BtleplusResult<Vec<String>> {
        use futures_util::TryStreamExt;
        let chars = self.gatt.discovered_characteristics().await?;
        let uuids: Vec<String> = chars.map_ok(|c| c.uuid().to_string()).try_collect().await?;
        Ok(uuids)
    }
}

/// Connect to peripheral with default timeout (30s).
pub async fn connect_session() -> Result<BleSession, Error> {
    connect_session_with_timeout(SCAN_TIMEOUT).await
}

/// Connect to peripheral with custom timeout.
pub async fn connect_session_with_timeout(timeout: Duration) -> Result<BleSession, Error> {
    let filter = ScanFilter::default()
        .with_name_pattern(PERIPHERAL_NAME)
        .with_service_uuid(Uuid::from_u16(battery::SERVICE_UUID16));
    let adapter = Adapter::default()
        .await
        .map_err(|e| anyhow!("Failed to connect: {}", e))?;
    let peripheral = adapter
        .find(filter, timeout)
        .await
        .map_err(|e| anyhow!("Failed to find peripheral: {}", e))?;
    let connection = peripheral
        .connect()
        .await
        .map_err(|e| anyhow!("Failed to connect: {}", e))?;

    tracing::info!("Connected to {}", PERIPHERAL_NAME);

    let gatt = connection.into_gatt().await?;

    Ok(BleSession {
        gatt,
        battery_uuid: Uuid::from_u16(battery::LEVEL_UUID16),
        // Device Info
        manufacturer_uuid: Uuid::from_u16(device_info::MANUFACTURER_NAME_UUID16),
        model_uuid: Uuid::from_u16(device_info::MODEL_NUMBER_UUID16),
        firmware_uuid: Uuid::from_u16(device_info::FIRMWARE_REVISION_UUID16),
        software_uuid: Uuid::from_u16(device_info::SOFTWARE_REVISION_UUID16),
        // Custom services
        echo_uuid: Uuid::from_u128(echo::UUID),
        status_uuid: Uuid::from_u128(status::UUID),
        bulk_control_uuid: Uuid::from_u128(bulk::CONTROL_UUID),
        bulk_data_uuid: Uuid::from_u128(bulk::CHUNK_UUID),
        bulk_stats_uuid: Uuid::from_u128(bulk::STATS_UUID),
    })
}
