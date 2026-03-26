use anyhow::{anyhow, Error};
use futures_util::StreamExt;
use hello_ble_common::{
    BulkControlCommand, BulkStats, BATTERY_LEVEL_UUID16, BULK_CONTROL_UUID,
    BULK_CHUNK_UUID, BULK_CHUNK_SIZE, BULK_STATS_UUID, ECHO_CAPACITY, ECHO_UUID,
    HEART_RATE_MEASUREMENT_UUID16, PERIPHERAL_NAME, SERVICE_BATTERY_UUID16, STATUS_UUID,
    DEVICE_INFO_MANUFACTURER_NAME_UUID16, DEVICE_INFO_MODEL_NUMBER_UUID16,
    DEVICE_INFO_FIRMWARE_REVISION_UUID16, DEVICE_INFO_SOFTWARE_REVISION_UUID16,
};
use std::time::Duration;
use tokio::time::sleep;
use winble::{ScanFilter, Session, Uuid, BluetoothUuidExt, Result as WinbleResult};

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
    session: Session,
    battery_uuid: Uuid,
    // Device Info UUIDs
    manufacturer_uuid: Uuid,
    model_uuid: Uuid,
    firmware_uuid: Uuid,
    software_uuid: Uuid,
    // Heart Rate UUIDs
    heart_rate_uuid: Uuid,
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
        let bytes = self.session.read(self.battery_uuid).await?;
        if bytes.len() != 1 {
            return Err(anyhow!("Expected 1 byte, got {}", bytes.len()));
        }
        Ok(bytes[0])
    }

    /// Read device info strings
    pub async fn device_info(&self) -> Result<DeviceInfo, Error> {
        let manufacturer = self.session.read_string(self.manufacturer_uuid).await?;
        let model = self.session.read_string(self.model_uuid).await?;
        let firmware = self.session.read_string(self.firmware_uuid).await?;
        let software = self.session.read_string(self.software_uuid).await?;
        Ok(DeviceInfo { manufacturer, model, firmware, software })
    }

    /// Get heart rate notifications stream
    pub async fn heart_rate_stream(
        &self,
    ) -> anyhow::Result<impl StreamExt<Item = anyhow::Result<u8>> + Unpin> {
        let stream = self.notifications(self.heart_rate_uuid).await?;
        Ok(stream.filter_map(|r| async move {
            match r {
                Ok(bytes) if !bytes.is_empty() => Some(Ok(bytes[0])),
                Ok(_) => None,
                Err(e) => Some(Err(anyhow!("{}", e))),
            }
        }).boxed())
    }

    /// Read status value (uses postcard)
    pub async fn status(&self) -> Result<bool, Error> {
        self.session.read_typed(self.status_uuid).await.map_err(|e| anyhow!("{}", e))
    }

    /// Write status value (uses postcard)
    pub async fn set_status(&self, value: bool) -> Result<(), Error> {
        self.session.write_typed(self.status_uuid, &value, true)
            .await
            .map_err(|e| anyhow!("{}", e))
    }

    /// Echo: write data and wait for notification
    pub async fn echo(&self, data: &[u8]) -> Result<(), Error> {
        if data.len() > ECHO_CAPACITY {
            return Err(anyhow!("Echo data too large: {} > {}", data.len(), ECHO_CAPACITY));
        }
        self.session.write(self.echo_uuid, data, true).await?;
        Ok(())
    }

    /// Read bulk stats (uses postcard)
    pub async fn read_bulk_stats(&self) -> Result<BulkStats, Error> {
        self.session.read_typed(self.bulk_stats_uuid).await.map_err(|e| anyhow!("{}", e))
    }

    /// Reset bulk transfer stats (uses postcard)
    pub async fn reset_bulk_stats(&self) -> Result<(), Error> {
        self.session.write_typed(self.bulk_control_uuid, &BulkControlCommand::ResetStats, true)
            .await
            .map_err(|e| anyhow!("{}", e))?;

        // Wait for stats to be reset
        for _ in 0..30 {
            sleep(Duration::from_millis(100)).await;
            let stats = self.read_bulk_stats().await?;
            if stats == BulkStats::default() {
                return Ok(());
            }
        }
        Err(anyhow!("Timeout waiting for stats reset"))
    }

    /// Start bulk notify stream on peripheral (uses postcard)
    pub async fn start_bulk_stream(&self, total_bytes: u32) -> Result<(), Error> {
        self.session
            .write_typed(
                self.bulk_control_uuid,
                &BulkControlCommand::StartStream { total_bytes },
                true,
            )
            .await
            .map_err(|e| anyhow!("{}", e))
    }

    /// Upload data via bulk_data characteristic (write without response)
    pub async fn upload_bulk_data(&self, data: &[u8]) -> Result<(), Error> {
        if data.len() > BULK_CHUNK_SIZE {
            return Err(anyhow!("Data too large: {} > {}", data.len(), BULK_CHUNK_SIZE));
        }
        self.session
            .write(self.bulk_data_uuid, data, false)
            .await?;
        Ok(())
    }

    /// Disconnect from peripheral
    pub async fn disconnect(&self) -> Result<(), Error> {
        self.session.disconnect().await?;
        Ok(())
    }

    pub async fn is_connected(&self) -> bool {
        self.session.is_connected().await
    }

    /// Get notifications stream for a specific characteristic
    pub async fn notifications(
        &self,
        uuid: Uuid,
    ) -> Result<impl StreamExt<Item = Result<Vec<u8>, Error>> + Unpin + '_, Error> {
        use futures_util::StreamExt;
        let stream = self.session.notifications(uuid).await?;
        Ok(stream.map(|r| r.map_err(|e| anyhow!("{}", e))))
    }

    pub fn battery_uuid(&self) -> Uuid {
        self.battery_uuid
    }

    pub fn heart_rate_uuid(&self) -> Uuid {
        self.heart_rate_uuid
    }

    pub fn echo_uuid(&self) -> Uuid {
        self.echo_uuid
    }

    pub fn bulk_data_uuid(&self) -> Uuid {
        self.bulk_data_uuid
    }

    /// Debug: list all discovered characteristic UUIDs
    pub async fn list_characteristics(&self) -> WinbleResult<Vec<String>> {
        use futures_util::TryStreamExt;
        let chars = self.session.discovered_characteristics().await?;
        let uuids: Vec<String> = chars
            .map_ok(|c| c.uuid().to_string())
            .try_collect()
            .await?;
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
        .with_service_uuid(Uuid::from_u16(SERVICE_BATTERY_UUID16));
    let session = Session::connect_with_filter(filter, timeout)
        .await
        .map_err(|e| anyhow!("Failed to connect: {}", e))?;

    tracing::info!("Connected to {}", PERIPHERAL_NAME);

    Ok(BleSession {
        session,
        battery_uuid: Uuid::from_u16(BATTERY_LEVEL_UUID16),
        // Device Info
        manufacturer_uuid: Uuid::from_u16(DEVICE_INFO_MANUFACTURER_NAME_UUID16),
        model_uuid: Uuid::from_u16(DEVICE_INFO_MODEL_NUMBER_UUID16),
        firmware_uuid: Uuid::from_u16(DEVICE_INFO_FIRMWARE_REVISION_UUID16),
        software_uuid: Uuid::from_u16(DEVICE_INFO_SOFTWARE_REVISION_UUID16),
        // Heart Rate
        heart_rate_uuid: Uuid::from_u16(HEART_RATE_MEASUREMENT_UUID16),
        // Custom services
        echo_uuid: Uuid::from_u128(ECHO_UUID),
        status_uuid: Uuid::from_u128(STATUS_UUID),
        bulk_control_uuid: Uuid::from_u128(BULK_CONTROL_UUID),
        bulk_data_uuid: Uuid::from_u128(BULK_CHUNK_UUID),
        bulk_stats_uuid: Uuid::from_u128(BULK_STATS_UUID),
    })
}
