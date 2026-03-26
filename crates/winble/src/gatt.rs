//! Windows BLE GATT operations via bluest
//!
//! ## Quick Start
//!
//! ```ignore
//! use winble::{Session, ScanFilter, Uuid};
//! use std::time::Duration;
//!
//! // Connect by name
//! let session = Session::connect("device-name", Duration::from_secs(10)).await?;
//!
//! // Read characteristic
//! let data = session.read_by_uuid(Uuid::from_u16(0x2A19)).await?;
//!
//! // Write with response
//! session.write(Uuid::from_u16(0x2A19), &[1, 2, 3], true).await?;
//!
//! // Subscribe to notifications
//! let stream = session.notifications(Uuid::from_u16(0x2A19)).await?;
//! ```
//!
//! ## Scan Filter
//!
//! ```ignore
//! let filter = ScanFilter::default()
//!     .with_name_pattern("my-device")
//!     .with_name_patterns(["device1", "device2"])
//!     .with_addr_pattern("001122334455")
//!     .with_service_uuid(Uuid::from_u16(0x180F))
//!     .with_scan_interval_secs(3);
//!
//! let session = Session::connect_with_filter(filter, Duration::from_secs(10)).await?;
//! ```

use crate::error::WinbleError;
use bluest::{Adapter, Characteristic, Device, Uuid};
use futures_core::Stream;
use futures_util::StreamExt;
use std::time::Duration;

/// Result type alias
pub type Result<T> = std::result::Result<T, WinbleError>;

/// Scan filter for discovering peripherals.
///
/// Filters can be combined (all conditions are OR'd within each category):
/// - Empty `name_patterns`/`addr_patterns` matches all
/// - Non-empty filters match if device matches any pattern (prefix supported)
///
/// Service UUIDs are used for OS-level filtering during scan.
#[derive(Default, Clone)]
pub struct ScanFilter {
    /// Filter by peripheral name patterns (OR'd, prefix matching supported)
    pub name_patterns: Vec<String>,
    /// Filter by address patterns in format "XXXXXXXXXXXX" (OR'd, prefix supported)
    pub addr_patterns: Vec<String>,
    /// Filter by service UUIDs (OS-level scan filter)
    pub service_uuids: Vec<Uuid>,
    /// Scan interval between iterations in seconds (default: 2)
    pub scan_interval_secs: u64,
}

impl ScanFilter {
    /// Add a name pattern filter (supports prefix matching).
    pub fn with_name_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.name_patterns.push(pattern.into());
        self
    }

    /// Add multiple name pattern filters.
    pub fn with_name_patterns(mut self, patterns: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.name_patterns.extend(patterns.into_iter().map(|n| n.into()));
        self
    }

    /// Add an address pattern filter (supports prefix matching).
    pub fn with_addr_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.addr_patterns.push(pattern.into());
        self
    }

    /// Add multiple address pattern filters.
    pub fn with_addr_patterns(mut self, patterns: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.addr_patterns.extend(patterns.into_iter().map(|a| a.into()));
        self
    }

    /// Add a service UUID filter (used in OS scan).
    pub fn with_service_uuid(mut self, uuid: Uuid) -> Self {
        self.service_uuids.push(uuid);
        self
    }

    /// Add multiple service UUID filters.
    pub fn with_service_uuids(mut self, uuids: impl IntoIterator<Item = Uuid>) -> Self {
        self.service_uuids.extend(uuids);
        self
    }

    /// Set scan interval between iterations.
    pub fn with_scan_interval_secs(mut self, secs: u64) -> Self {
        self.scan_interval_secs = secs;
        self
    }

    /// Check if a device matches this filter by name or address.
    /// Uses OR logic: matches if name matches OR address matches.
    /// Pattern matching supports prefix matching (e.g., "SmartBulb-" matches "SmartBulb-A1B2C3").
    fn matches(&self, name: &str, address: &str) -> bool {
        // Name matches: pattern match OR empty (match all)
        let name_matches = self.name_patterns.is_empty()
            || self.name_patterns.iter().any(|p| {
                p.is_empty() || name.starts_with(p) || *p == name
            });

        // Address matches: pattern match OR empty (match all)
        let addr_matches = self.addr_patterns.is_empty()
            || self.addr_patterns.iter().any(|p| {
                p.is_empty() || address.starts_with(p) || *p == address
            });

        name_matches || addr_matches
    }
}

/// GATT session for communicating with a BLE peripheral.
///
/// Created via [`Session::connect`], [`Session::connect_by_address`],
/// [`Session::connect_by_service`], or [`Session::connect_with_filter`].
pub struct Session {
    adapter: Adapter,
    device: Device,
    services: Vec<bluest::Service>,
    characteristics: Vec<Characteristic>,
}

impl Session {
    /// Connect to a peripheral by name (supports pattern matching).
    pub async fn connect(name: &str, timeout: Duration) -> Result<Self> {
        let filter = ScanFilter::default().with_name_pattern(name);
        Self::connect_with_filter(filter, timeout).await
    }

    /// Connect to a peripheral by address (supports pattern matching).
    pub async fn connect_by_address(address: &str, timeout: Duration) -> Result<Self> {
        let filter = ScanFilter::default().with_addr_pattern(address);
        Self::connect_with_filter(filter, timeout).await
    }

    /// Connect to a peripheral advertising a service UUID.
    pub async fn connect_by_service(uuid: Uuid, timeout: Duration) -> Result<Self> {
        let filter = ScanFilter::default().with_service_uuid(uuid);
        Self::connect_with_filter(filter, timeout).await
    }

    /// Scan and connect using a custom filter.
    pub async fn connect_with_filter(filter: ScanFilter, timeout: Duration) -> Result<Self> {
        let adapter = Adapter::default()
            .await
            .ok_or_else(|| WinbleError::Bluetooth("No Bluetooth adapter found".to_string()))?;
        let device = Self::scan_for_target(&adapter, &filter, timeout).await?;

        adapter.connect_device(&device).await?;

        let (services, characteristics) = Self::discover_gatt(&device).await?;

        Ok(Self {
            adapter,
            device,
            services,
            characteristics,
        })
    }

    /// Reconnect to the same device after disconnection.
    pub async fn reconnect(&mut self) -> Result<()> {
        if self.device.is_connected().await {
            self.adapter.disconnect_device(&self.device).await?;
        }

        self.adapter.connect_device(&self.device).await?;

        let (services, characteristics) = Self::discover_gatt(&self.device).await?;
        self.services = services;
        self.characteristics = characteristics;

        Ok(())
    }

    /// Discover services and characteristics from a connected device.
    async fn discover_gatt(device: &Device) -> Result<(Vec<bluest::Service>, Vec<Characteristic>)> {
        let services = device.discover_services().await?;
        let mut characteristics = Vec::new();
        for service in &services {
            if let Ok(chars) = service.characteristics().await {
                characteristics.extend(chars);
            }
        }
        Ok((services, characteristics))
    }

    /// Check if currently connected to the peripheral.
    pub async fn is_connected(&self) -> bool {
        self.device.is_connected().await
    }

    /// Find a characteristic by UUID.
    fn find_char(&self, uuid: Uuid) -> Result<&Characteristic> {
        self.characteristics
            .iter()
            .find(|c| c.uuid() == uuid)
            .ok_or_else(|| WinbleError::InvalidOperation(format!("Characteristic {} not found", uuid)))
    }

    /// Read a characteristic value by UUID.
    pub async fn read(&self, uuid: Uuid) -> Result<Vec<u8>> {
        let char = self.find_char(uuid)?;
        Ok(char.read().await?)
    }

    /// Read a characteristic value as a UTF-8 string.
    pub async fn read_string(&self, uuid: Uuid) -> Result<String> {
        let bytes = self.read(uuid).await?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// Read and deserialize a characteristic value using postcard.
    pub async fn read_typed<T: serde::de::DeserializeOwned>(&self, uuid: Uuid) -> Result<T> {
        let bytes = self.read(uuid).await?;
        postcard::from_bytes(&bytes)
            .map_err(|_| WinbleError::Deserialize("postcard deserialize failed".into()))
    }

    /// Serialize and write a value to a characteristic using postcard.
    pub async fn write_typed<T: serde::Serialize>(
        &self,
        uuid: Uuid,
        value: &T,
        with_response: bool,
    ) -> Result<()> {
        let mut buf = [0u8; 256];
        let used = postcard::to_slice(value, &mut buf)
            .map_err(|_| WinbleError::Serialize("postcard serialize failed".into()))?;
        self.write(uuid, used, with_response).await
    }

    /// Write to a characteristic.
    ///
    /// Set `with_response` to `true` for write-with-response (waits for ACK).
    /// Set to `false` for write-without-response (fire-and-forget).
    pub async fn write(&self, uuid: Uuid, data: &[u8], with_response: bool) -> Result<()> {
        let char = self.find_char(uuid)?;

        if with_response {
            Ok(char.write(data).await?)
        } else {
            Ok(char.write_without_response(data).await?)
        }
    }

    /// Get a stream of notifications from a characteristic.
    ///
    /// This enables notifications and returns a stream of incoming values.
    /// Use with `futures_util::StreamExt`:
    ///
    /// ```ignore
    /// use futures_util::StreamExt;
    ///
    /// let mut stream = session.notifications(uuid).await?;
    /// while let Some(result) = stream.next().await {
    ///     let data = result?;
    ///     println!("{:?}", data);
    /// }
    /// ```
    pub async fn notifications(&self, uuid: Uuid) -> Result<impl Stream<Item = Result<Vec<u8>>> + '_> {
        let char = self.find_char(uuid)?;
        let stream = char.notify().await?;
        Ok(stream.map(|v| v.map_err(WinbleError::from)))
    }

    /// Disconnect from the peripheral.
    pub async fn disconnect(&self) -> Result<()> {
        if self.device.is_connected().await {
            self.adapter.disconnect_device(&self.device).await?;
        }
        Ok(())
    }

    /// Get a stream of all discovered characteristic UUIDs (for debugging).
    pub async fn discovered_characteristics(&self) -> Result<impl Stream<Item = Result<Characteristic>>> {
        let stream = futures_util::stream::iter(
            self.characteristics.clone()
                .into_iter()
                .map(Ok::<Characteristic, WinbleError>)
        );
        Ok(stream)
    }

    async fn scan_for_target(
        adapter: &Adapter,
        filter: &ScanFilter,
        timeout: Duration,
    ) -> Result<Device> {
        let scan_interval = Duration::from_secs(filter.scan_interval_secs.max(1));

        let result = tokio::time::timeout(timeout, async {
            // Scan with OS-level service UUID filter
            let mut scan_stream = adapter.scan(&filter.service_uuids).await.ok()?;

            loop {
                tokio::time::sleep(scan_interval).await;

                if let Some(adv_device) = scan_stream.next().await {
                    let address = adv_device.device.id().to_string();
                    let name = adv_device.adv_data.local_name.clone().unwrap_or_default();

                    if filter.matches(&name, &address) {
                        let device = adapter.open_device(&adv_device.device.id()).await.ok();
                        if let Some(device) = device {
                            return Some(device);
                        }
                    }
                }
            }
        })
        .await;

        match result {
            Ok(Some(device)) => Ok(device),
            Ok(None) => Err(WinbleError::DeviceNotFound("Device not found".to_string())),
            Err(_) => Err(WinbleError::Timeout),
        }
    }
}
