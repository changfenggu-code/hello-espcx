//! GAP adapter entry point.

use bluest::Adapter as BluestAdapter;
use futures_util::StreamExt;
use std::collections::HashSet;
use std::time::Duration;

use crate::error::BtleplusError;

use super::{Connection, Peripheral, PeripheralProperties, ScanFilter};

/// Wrapper around the system Bluetooth adapter.
#[derive(Debug, Clone)]
pub struct Adapter {
    inner: BluestAdapter,
}

impl Adapter {
    /// Open the system default Bluetooth adapter.
    pub async fn default() -> Result<Self, BtleplusError> {
        let inner = BluestAdapter::default()
            .await
            .ok_or_else(|| BtleplusError::Bluetooth("No Bluetooth adapter found".to_string()))?;
        Ok(Self { inner })
    }

    /// Return a reference to the underlying bluest adapter.
    pub(crate) fn inner(&self) -> &BluestAdapter {
        &self.inner
    }

    /// Scan and collect all peripherals matching the supplied scan filter.
    pub async fn discover(
        &self,
        filter: ScanFilter,
        timeout: Duration,
    ) -> Result<Vec<Peripheral>, BtleplusError> {
        scan_for_targets(self, &filter, timeout).await
    }

    /// Find the first peripheral matching the supplied scan filter.
    pub async fn find(
        &self,
        filter: ScanFilter,
        timeout: Duration,
    ) -> Result<Peripheral, BtleplusError> {
        self.find_ref(&filter, timeout).await
    }

    /// Borrowing variant of [`Adapter::find`].
    pub async fn find_ref(
        &self,
        filter: &ScanFilter,
        timeout: Duration,
    ) -> Result<Peripheral, BtleplusError> {
        scan_for_target(self, filter, timeout).await
    }

    /// Scan, find, and connect in a single step.
    pub async fn connect_with_filter(
        &self,
        filter: ScanFilter,
        timeout: Duration,
    ) -> Result<Connection, BtleplusError> {
        self.find(filter, timeout).await?.connect().await
    }
}

fn should_collect_discovered_properties(
    properties: &PeripheralProperties,
    filter: &ScanFilter,
    seen_ids: &mut HashSet<String>,
) -> bool {
    if !filter.matches_properties(properties) {
        return false;
    }

    seen_ids.insert(properties.id.clone())
}

async fn scan_for_target(
    adapter: &Adapter,
    filter: &ScanFilter,
    timeout: Duration,
) -> Result<Peripheral, BtleplusError> {
    let scan_interval = Duration::from_secs(filter.scan_interval_secs.max(1));

    let result = tokio::time::timeout(timeout, async {
        let mut scan_stream = adapter.inner.scan(&filter.service_uuids).await.ok()?;

        loop {
            tokio::time::sleep(scan_interval).await;

            if let Some(adv_device) = scan_stream.next().await {
                let properties = PeripheralProperties::from_advertising_device(&adv_device);
                if filter.matches_properties(&properties) {
                    let device = adapter
                        .inner
                        .open_device(&adv_device.device.id())
                        .await
                        .ok()?;
                    return Some(Peripheral::new(adapter.clone(), device, properties));
                }
            }
        }
    })
    .await;

    match result {
        Ok(Some(device)) => Ok(device),
        Ok(None) => Err(BtleplusError::DeviceNotFound(
            "Device not found".to_string(),
        )),
        Err(_) => Err(BtleplusError::Timeout),
    }
}

async fn scan_for_targets(
    adapter: &Adapter,
    filter: &ScanFilter,
    timeout: Duration,
) -> Result<Vec<Peripheral>, BtleplusError> {
    let scan_interval = Duration::from_secs(filter.scan_interval_secs.max(1));
    let mut scan_stream = adapter
        .inner
        .scan(&filter.service_uuids)
        .await
        .map_err(BtleplusError::from)?;

    let deadline = tokio::time::Instant::now() + timeout;
    let mut seen_ids = HashSet::new();
    let mut peripherals = Vec::new();

    loop {
        let now = tokio::time::Instant::now();
        if now >= deadline {
            break;
        }

        let pause = deadline.saturating_duration_since(now).min(scan_interval);
        tokio::time::sleep(pause).await;

        let now = tokio::time::Instant::now();
        if now >= deadline {
            break;
        }

        let remaining = deadline.saturating_duration_since(now);
        let next = match tokio::time::timeout(remaining, scan_stream.next()).await {
            Ok(item) => item,
            Err(_) => break,
        };

        let Some(adv_device) = next else {
            break;
        };

        let properties = PeripheralProperties::from_advertising_device(&adv_device);
        if !should_collect_discovered_properties(&properties, filter, &mut seen_ids) {
            continue;
        }

        if let Ok(device) = adapter.inner.open_device(&adv_device.device.id()).await {
            peripherals.push(Peripheral::new(adapter.clone(), device, properties));
        }
    }

    Ok(peripherals)
}

#[cfg(test)]
mod tests;
