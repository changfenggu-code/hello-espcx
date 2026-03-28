use std::time::Duration;

use anyhow::{Error, anyhow};
use btleplus::{
    Adapter, BluetoothUuidExt, Client, ManufacturerData, Peripheral, PeripheralSelectionExt,
    Result as BtleplusResult, ScanFilter, Selector, Uuid,
};
use futures_util::StreamExt;
use hello_ble_common::{
    PERIPHERAL_NAME, advertisement_identity, battery, bulk, device_info, echo, status,
};
use tokio::time::sleep;

const SCAN_TIMEOUT: Duration = Duration::from_secs(30);

/// Device information from the connected peripheral.
#[derive(Debug)]
pub struct DeviceInfo {
    pub manufacturer: String,
    pub model: String,
    pub firmware: String,
    pub software: String,
}

/// A scanned peripheral plus the decoded manufacturer identity we care about.
#[derive(Debug, Clone)]
pub struct ProductCandidate {
    peripheral: Peripheral,
    identity: advertisement_identity::ManufacturerPayload,
}

pub struct BleSession {
    /// GATT client for attribute operations on the connected peripheral.
    gatt: Client,
    battery_uuid: Uuid,
    manufacturer_uuid: Uuid,
    model_uuid: Uuid,
    firmware_uuid: Uuid,
    software_uuid: Uuid,
    echo_uuid: Uuid,
    status_uuid: Uuid,
    bulk_control_uuid: Uuid,
    bulk_data_uuid: Uuid,
    bulk_stats_uuid: Uuid,
}

pub async fn discover_product_candidates() -> Result<Vec<ProductCandidate>, Error> {
    discover_product_candidates_with_timeout(SCAN_TIMEOUT).await
}

pub async fn discover_product_candidates_with_timeout(
    timeout: Duration,
) -> Result<Vec<ProductCandidate>, Error> {
    let filter = build_product_scan_filter();
    let adapter = Adapter::default()
        .await
        .map_err(|e| anyhow!("Failed to open adapter: {e}"))?;
    let peripherals = adapter
        .discover(filter, timeout)
        .await
        .map_err(|e| anyhow!("Failed to discover peripherals: {e}"))?;
    let selector = build_product_selector();
    let ranked = peripherals
        .rank_with(&selector)
        .map_err(|e| anyhow!("Failed to rank peripherals: {e}"))?;

    ranked
        .into_iter()
        .map(product_candidate_from_peripheral)
        .collect()
}

/// Connect to peripheral with default timeout.
pub async fn connect_session() -> Result<BleSession, Error> {
    connect_session_with_timeout(SCAN_TIMEOUT).await
}

/// Connect to peripheral with custom timeout.
pub async fn connect_session_with_timeout(timeout: Duration) -> Result<BleSession, Error> {
    let candidate = discover_product_candidates_with_timeout(timeout)
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("No matching product candidates found"))?;

    tracing::info!(
        "Connected to {} (unit_id={})",
        candidate.local_name().unwrap_or(PERIPHERAL_NAME),
        candidate.identity().unit_id
    );

    candidate.connect().await
}

impl ProductCandidate {
    pub fn id(&self) -> &str {
        self.peripheral.id()
    }

    pub fn local_name(&self) -> Option<&str> {
        self.peripheral.local_name()
    }

    pub fn rssi(&self) -> Option<i16> {
        self.peripheral.properties().rssi
    }

    pub fn is_connectable(&self) -> bool {
        self.peripheral.properties().is_connectable
    }

    pub fn identity(&self) -> &advertisement_identity::ManufacturerPayload {
        &self.identity
    }

    pub async fn connect(self) -> Result<BleSession, Error> {
        let connection = self
            .peripheral
            .connect()
            .await
            .map_err(|e| anyhow!("Failed to connect: {e}"))?;

        build_session(connection.into_gatt().await?)
    }
}

impl BleSession {
    /// Read battery level (0-100%).
    pub async fn battery_level(&self) -> Result<u8, Error> {
        let bytes = self.gatt.read(self.battery_uuid).await?;
        if bytes.len() != 1 {
            return Err(anyhow!("Expected 1 byte, got {}", bytes.len()));
        }
        Ok(bytes[0])
    }

    /// Read device info strings.
    pub async fn device_info(&self) -> Result<DeviceInfo, Error> {
        let manufacturer = self.gatt.read_to_string(self.manufacturer_uuid).await?;
        let model = self.gatt.read_to_string(self.model_uuid).await?;
        let firmware = self.gatt.read_to_string(self.firmware_uuid).await?;
        let software = self.gatt.read_to_string(self.software_uuid).await?;

        Ok(DeviceInfo {
            manufacturer,
            model,
            firmware,
            software,
        })
    }

    /// Read status value (uses postcard).
    pub async fn status(&self) -> Result<bool, Error> {
        self.gatt
            .read_to(self.status_uuid)
            .await
            .map_err(|e| anyhow!("{e}"))
    }

    /// Read bulk stats (uses postcard).
    pub async fn read_bulk_stats(&self) -> Result<bulk::BulkStats, Error> {
        self.gatt
            .read_to(self.bulk_stats_uuid)
            .await
            .map_err(|e| anyhow!("{e}"))
    }

    /// Write status value (uses postcard).
    pub async fn set_status(&self, value: bool) -> Result<(), Error> {
        self.gatt
            .write_from(self.status_uuid, &value, true)
            .await
            .map_err(|e| anyhow!("{e}"))
    }

    /// Echo: write data and wait for notification.
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

    /// Reset bulk transfer stats (uses postcard).
    pub async fn reset_bulk_stats(&self) -> Result<(), Error> {
        self.gatt
            .write_from(
                self.bulk_control_uuid,
                &bulk::BulkControlCommand::ResetStats,
                true,
            )
            .await
            .map_err(|e| anyhow!("{e}"))?;

        for _ in 0..30 {
            sleep(Duration::from_millis(100)).await;
            let stats = self.read_bulk_stats().await?;
            if stats == bulk::BulkStats::default() {
                return Ok(());
            }
        }

        Err(anyhow!("Timeout waiting for stats reset"))
    }

    /// Start bulk notify stream on peripheral (uses postcard).
    pub async fn start_bulk_stream(&self, total_bytes: u32) -> Result<(), Error> {
        self.gatt
            .write_from(
                self.bulk_control_uuid,
                &bulk::BulkControlCommand::StartStream { total_bytes },
                true,
            )
            .await
            .map_err(|e| anyhow!("{e}"))
    }

    /// Upload data via bulk_data characteristic (write without response).
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

    /// Upload a test pattern to peripheral in chunks.
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

    /// Get notifications stream for a specific characteristic.
    pub async fn notifications(
        &self,
        uuid: Uuid,
    ) -> Result<impl StreamExt<Item = Result<Vec<u8>, Error>> + Unpin + '_, Error> {
        let stream = self.gatt.notifications(uuid).await?;
        Ok(stream.map(|result| result.map_err(|e| anyhow!("{e}"))))
    }

    /// Disconnect from the peripheral.
    pub async fn disconnect(&self) -> Result<(), Error> {
        self.gatt.connection().disconnect().await?;
        Ok(())
    }

    pub async fn is_connected(&self) -> bool {
        self.gatt.connection().is_connected().await
    }

    /// Debug: list all discovered characteristic UUIDs.
    pub async fn list_characteristics(&self) -> BtleplusResult<Vec<String>> {
        use futures_util::TryStreamExt;

        let chars = self.gatt.discovered_characteristics().await?;
        chars
            .map_ok(|characteristic| characteristic.uuid().to_string())
            .try_collect()
            .await
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
}

fn build_session(gatt: Client) -> Result<BleSession, Error> {
    Ok(BleSession {
        gatt,
        battery_uuid: Uuid::from_u16(battery::LEVEL_UUID16),
        manufacturer_uuid: Uuid::from_u16(device_info::MANUFACTURER_NAME_UUID16),
        model_uuid: Uuid::from_u16(device_info::MODEL_NUMBER_UUID16),
        firmware_uuid: Uuid::from_u16(device_info::FIRMWARE_REVISION_UUID16),
        software_uuid: Uuid::from_u16(device_info::SOFTWARE_REVISION_UUID16),
        echo_uuid: Uuid::from_u128(echo::UUID),
        status_uuid: Uuid::from_u128(status::UUID),
        bulk_control_uuid: Uuid::from_u128(bulk::CONTROL_UUID),
        bulk_data_uuid: Uuid::from_u128(bulk::CHUNK_UUID),
        bulk_stats_uuid: Uuid::from_u128(bulk::STATS_UUID),
    })
}

fn build_product_scan_filter() -> ScanFilter {
    ScanFilter::default()
        .with_name_pattern(PERIPHERAL_NAME)
        .with_service_uuid(Uuid::from_u16(battery::SERVICE_UUID16))
        .with_manufacturer_company_id(advertisement_identity::DEVELOPMENT_COMPANY_ID)
        .with_manufacturer_data(matches_product_identity)
}

fn build_product_selector() -> Selector {
    Selector::default()
        .prefer_connectable()
        .prefer_strongest_signal()
}

fn product_candidate_from_peripheral(peripheral: Peripheral) -> Result<ProductCandidate, Error> {
    let Some(data) = peripheral.properties().manufacturer_data.as_ref() else {
        return Err(anyhow!("Matched peripheral is missing manufacturer data"));
    };
    let Some(identity) = decode_manufacturer_payload(data) else {
        return Err(anyhow!(
            "Matched peripheral has invalid manufacturer payload"
        ));
    };

    Ok(ProductCandidate {
        peripheral,
        identity,
    })
}

fn matches_product_identity(data: &ManufacturerData) -> bool {
    decode_manufacturer_payload(data).is_some_and(|payload| {
        payload.version == advertisement_identity::VERSION
            && payload.product_id == advertisement_identity::PRODUCT_ID_HELLO_ESPCX
    })
}

fn decode_manufacturer_payload(
    data: &ManufacturerData,
) -> Option<advertisement_identity::ManufacturerPayload> {
    if !data.is_company_id(advertisement_identity::DEVELOPMENT_COMPANY_ID) {
        return None;
    }

    let payload = data.payload();
    if payload.len() != advertisement_identity::PAYLOAD_LEN {
        return None;
    }

    Some(advertisement_identity::ManufacturerPayload {
        version: payload[0],
        product_id: payload[1],
        unit_id: u32::from_le_bytes([payload[2], payload[3], payload[4], payload[5]]),
        flags: payload[6],
    })
}

#[cfg(test)]
mod tests;
