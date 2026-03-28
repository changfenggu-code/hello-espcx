//! GAP adapter entry point.
//! GAP 适配器入口。
//!
//! # Public API / 公开 API
//!
//! | Method | Description |
//! |--------|-------------|
//! | [`Adapter::default`] | Open the system default Bluetooth adapter. 打开系统默认蓝牙适配器。 |
//! | [`Adapter::find`] | Scan and find a peripheral matching the filter. 扫描并查找匹配过滤条件的外设。 |
//! | [`Adapter::find_ref`] | Scan and find a peripheral (accepts `&ScanFilter`). 同上，但接收 `&ScanFilter` 引用。 |
//! | [`Adapter::discover`] | Scan and collect all peripherals matching the filter. 扫描并收集所有匹配过滤器的外设。 |
//! | [`Adapter::connect_with_filter`] | One-step: scan, find, and connect. 一步完成扫描 + 查找 + 连接。 |

use bluest::Adapter as BluestAdapter;
use futures_util::StreamExt;
use std::collections::HashSet;
use std::time::Duration;

use crate::error::BtleplusError;

use super::{Connection, Peripheral, PeripheralProperties, ScanFilter};

/// Wrapper around the system Bluetooth adapter.
/// 系统蓝牙适配器的包装器。
#[derive(Debug, Clone)]
pub struct Adapter {
    inner: BluestAdapter,
}

impl Adapter {
    /// Open the system default Bluetooth adapter.
    /// 打开系统默认蓝牙适配器。
    pub async fn default() -> Result<Self, BtleplusError> {
        let inner = BluestAdapter::default()
            .await
            .ok_or_else(|| BtleplusError::Bluetooth("No Bluetooth adapter found".to_string()))?;
        Ok(Self { inner })
    }

    /// Return a reference to the underlying bluest adapter.
    /// 返回底层 bluest 适配器的引用。
    pub(crate) fn inner(&self) -> &BluestAdapter {
        &self.inner
    }

    /// Scan and collect all peripherals matching the supplied scan filter.
    /// 扫描并收集所有匹配过滤器的外设。
    pub async fn discover(
        &self,
        filter: ScanFilter,
        timeout: Duration,
    ) -> Result<Vec<Peripheral>, BtleplusError> {
        scan_for_targets(self, &filter, timeout).await
    }

    /// Find a peripheral matching the supplied scan filter.
    /// 查找与提供的扫描过滤器匹配的外设。
    ///
    /// Ownership of `filter` is transferred into this call.
    /// After the call, the caller can no longer use `filter`.
    pub async fn find(
        &self,
        filter: ScanFilter,
        timeout: Duration,
    ) -> Result<Peripheral, BtleplusError> {
        self.find_ref(&filter, timeout).await
    }

    /// Find a peripheral matching the supplied scan filter.
    /// 查找与提供的扫描过滤器匹配的外设。
    ///
    /// `filter` is borrowed by this call.
    /// The caller retains ownership and can reuse `filter` afterwards.
    pub async fn find_ref(
        &self,
        filter: &ScanFilter,
        timeout: Duration,
    ) -> Result<Peripheral, BtleplusError> {
        scan_for_target(self, filter, timeout).await
    }

    /// Scan, find, and connect in a single step.
    /// 一步完成扫描、查找和连接。
    pub async fn connect_with_filter(
        &self,
        filter: ScanFilter,
        timeout: Duration,
    ) -> Result<Connection, BtleplusError> {
        self.find(filter, timeout).await?.connect().await
    }
}

// Internal: scan for a device matching the filter and return an opened Peripheral.
// 内部函数：扫描匹配过滤条件的外设，返回已打开的 Peripheral。
async fn scan_for_target(
    adapter: &Adapter,
    filter: &ScanFilter,
    timeout: Duration,
) -> Result<Peripheral, BtleplusError> {
    // Convert scan interval from seconds to Duration, minimum 1 second.
    // 将扫描间隔从秒转换为 Duration，最小 1 秒。
    let scan_interval = Duration::from_secs(filter.scan_interval_secs.max(1));

    // Wrap the scan in a timeout so we don't wait forever.
    // 将扫描封装在 timeout 内，避免无限等待。
    let result = tokio::time::timeout(timeout, async {
        // Start scanning with OS-level service UUID filtering.
        // 启动扫描，使用 OS 级别的服务 UUID 过滤。
        let mut scan_stream = adapter.inner.scan(&filter.service_uuids).await.ok()?;

        // Continuously poll for advertising devices until timeout.
        // 持续轮询广播设备直到超时。
        loop {
            // Wait for the next scan interval before checking.
            // 等待下一个扫描间隔后再检查。
            tokio::time::sleep(scan_interval).await;

            if let Some(adv_device) = scan_stream.next().await {
                // Extract properties (name, address, RSSI, etc.) from the advertising packet.
                // 从广播包中提取属性（名称、地址、RSSI 等）。
                let properties = PeripheralProperties::from_advertising_device(&adv_device);
                let local_name = properties.local_name.as_deref().unwrap_or_default();

                // Apply app-level name/address filter on top of OS UUID filter.
                // 在 OS UUID 过滤之上叠加应用层的名称/地址过滤。
                if filter.matches(local_name, &properties.id) {
                    // Open the device handle for later connection.
                    // 打开设备句柄以供后续连接。
                    let device = adapter.inner.open_device(&adv_device.device.id()).await.ok()?;
                    return Some(Peripheral::new(adapter.clone(), device, properties));
                }
            }
        }
    })
    .await;

    // Interpret timeout results: device found, not found, or timed out.
    // 解释 timeout 结果：找到了设备、未找到、或超时。
    match result {
        Ok(Some(device)) => Ok(device),
        Ok(None) => Err(BtleplusError::DeviceNotFound("Device not found".to_string())),
        Err(_) => Err(BtleplusError::Timeout),
    }
}

// Internal: scan for all devices matching the filter and return opened Peripherals.
// 内部函数：扫描所有匹配过滤条件的外设，返回已打开的 Peripheral 列表。
async fn scan_for_targets(
    adapter: &Adapter,
    filter: &ScanFilter,
    timeout: Duration,
) -> Result<Vec<Peripheral>, BtleplusError> {
    // Convert scan interval from seconds to Duration, minimum 1 second.
    // 将扫描间隔从秒转换为 Duration，最小 1 秒。
    let scan_interval = Duration::from_secs(filter.scan_interval_secs.max(1));

    // Start scanning with OS-level service UUID filtering.
    // 启动扫描，使用 OS 级别的服务 UUID 过滤。
    let mut scan_stream = adapter
        .inner
        .scan(&filter.service_uuids)
        .await
        .map_err(BtleplusError::from)?;

    // Compute the absolute deadline from the requested timeout.
    // 根据请求的超时时间计算绝对截止时刻。
    let deadline = tokio::time::Instant::now() + timeout;

    // Track device IDs we have already seen to avoid duplicates.
    // 跟踪已见过的设备 ID，避免重复收集。
    let mut seen_ids = HashSet::new();
    let mut peripherals = Vec::new();

    loop {
        // Check if the deadline has been reached before each iteration.
        // 每次迭代前检查是否已到达截止时刻。
        let now = tokio::time::Instant::now();
        if now >= deadline {
            break;
        }

        // Sleep for the shorter of the remaining time and the scan interval.
        // 休眠时间取剩余时间和扫描间隔中的较小值。
        let pause = deadline.saturating_duration_since(now).min(scan_interval);
        tokio::time::sleep(pause).await;

        // Re-check deadline after sleeping.
        // 休眠后再次检查截止时刻。
        let now = tokio::time::Instant::now();
        if now >= deadline {
            break;
        }

        // Wait for the next advertising device, bounded by the remaining time.
        // 等待下一个广播设备，受剩余时间限制。
        let remaining = deadline.saturating_duration_since(now);
        let next = match tokio::time::timeout(remaining, scan_stream.next()).await {
            Ok(item) => item,
            Err(_) => break,
        };

        let Some(adv_device) = next else {
            break;
        };

        // Extract properties and apply app-level name/address filter.
        // 提取属性并应用应用层的名称/地址过滤。
        let properties = PeripheralProperties::from_advertising_device(&adv_device);
        let local_name = properties.local_name.as_deref().unwrap_or_default();
        if !filter.matches(local_name, &properties.id) {
            continue;
        }

        // Skip devices we have already collected.
        // 跳过已经收集过的设备。
        if !seen_ids.insert(properties.id.clone()) {
            continue;
        }

        // Open the device handle and add to the result list.
        // 打开设备句柄并加入结果列表。
        if let Ok(device) = adapter.inner.open_device(&adv_device.device.id()).await {
            peripherals.push(Peripheral::new(adapter.clone(), device, properties));
        }
    }

    Ok(peripherals)
}
