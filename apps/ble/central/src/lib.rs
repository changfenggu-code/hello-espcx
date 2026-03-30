//! BLE Central 会话库 / BLE Central session library
//!
//! 提供 BLE Central 端的核心功能：扫描、连接、GATT 操作。
//! Provides core BLE Central functionality: scanning, connecting, and GATT operations.
//!
//! ## 架构概览 / Architecture Overview
//!
//! ```text
//! discover_product_candidates()  →  扫描并筛选目标设备
//!         ↓                        Scan and filter target devices
//! ProductCandidate               →  候选设备（含厂商身份信息）
//!         ↓                        Candidate device (with manufacturer identity)
//! candidate.connect()            →  建立 BLE 连接
//!         ↓                        Establish BLE connection
//! BleSession                     →  已连接会话，可进行 GATT 读写/通知
//!                                  Connected session for GATT read/write/notify
//! ```

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

/// 默认扫描超时 / Default scan timeout (30 seconds).
const SCAN_TIMEOUT: Duration = Duration::from_secs(30);

// ============================================================================
// 数据类型 / Data Types
// ============================================================================

/// 设备信息 / Device information read from the peripheral's Device Info Service.
///
/// 从外设的 Device Information Service 一次性读取的静态字符串。
/// Static strings read once from the peripheral's Device Information Service.
#[derive(Debug)]
pub struct DeviceInfo {
    /// 制造商名称 / Manufacturer name.
    pub manufacturer: String,
    /// 型号 / Model number.
    pub model: String,
    /// 固件版本 / Firmware revision.
    pub firmware: String,
    /// 软件版本 / Software revision.
    pub software: String,
}

/// 扫描到的候选设备 / A scanned peripheral plus the decoded manufacturer identity.
///
/// 包含 BLE 外设和从广播数据中解析出的厂商身份摘要。
/// Contains the BLE peripheral and the manufacturer identity decoded from advertisement data.
#[derive(Debug, Clone)]
pub struct ProductCandidate {
    /// 底层 BLE 外设 / Underlying BLE peripheral.
    peripheral: Peripheral,
    /// 从 manufacturer_data 解码的身份摘要 / Decoded identity from manufacturer_data.
    identity: advertisement_identity::ManufacturerPayload,
}

/// BLE 已连接会话 / An active BLE session with a connected peripheral.
///
/// 持有 GATT 客户端和所有已知特征的 UUID，提供高层读写接口。
/// Holds a GATT client and all known characteristic UUIDs, providing high-level read/write APIs.
///
/// UUID 分属 5 个服务 / UUIDs belong to 5 services:
/// - **Battery** (标准): `battery_uuid`
/// - **Device Info** (标准): `manufacturer_uuid`, `model_uuid`, `firmware_uuid`, `software_uuid`
/// - **Echo** (自定义): `echo_uuid`
/// - **Status** (自定义): `status_uuid`
/// - **Bulk** (自定义): `bulk_control_uuid`, `bulk_data_uuid`, `bulk_stats_uuid`
pub struct BleSession {
    /// GATT 客户端，用于对已连接外设进行属性操作 / GATT client for attribute operations.
    gatt: Client,
    /// 电量特征 UUID (0x2A19) / Battery Level characteristic UUID.
    battery_uuid: Uuid,
    /// 制造商名称特征 UUID (0x2A29) / Manufacturer Name characteristic UUID.
    manufacturer_uuid: Uuid,
    /// 型号特征 UUID (0x2A24) / Model Number characteristic UUID.
    model_uuid: Uuid,
    /// 固件版本特征 UUID (0x2A26) / Firmware Revision characteristic UUID.
    firmware_uuid: Uuid,
    /// 软件版本特征 UUID (0x2A28) / Software Revision characteristic UUID.
    software_uuid: Uuid,
    /// Echo 特征 UUID (128-bit 自定义) / Echo characteristic UUID (128-bit custom).
    echo_uuid: Uuid,
    /// Status 特征 UUID (128-bit 自定义) / Status characteristic UUID (128-bit custom).
    status_uuid: Uuid,
    /// Bulk 控制特征 UUID / Bulk control characteristic UUID (write commands).
    bulk_control_uuid: Uuid,
    /// Bulk 数据特征 UUID / Bulk data characteristic UUID (stream transfer).
    bulk_data_uuid: Uuid,
    /// Bulk 统计特征 UUID / Bulk stats characteristic UUID (rx/tx counters).
    bulk_stats_uuid: Uuid,
}

// ============================================================================
// 扫描与发现 / Scanning & Discovery
// ============================================================================

/// 用默认超时扫描目标设备候选列表 / Scan for target product candidates with default timeout.
pub async fn discover_product_candidates() -> Result<Vec<ProductCandidate>, Error> {
    discover_product_candidates_with_timeout(SCAN_TIMEOUT).await
}

/// 用自定义超时扫描目标设备候选列表 / Scan for target product candidates with custom timeout.
///
/// 流程 / Flow:
/// 1. 构建扫描过滤器（设备名 + 服务 UUID + 厂商数据）
///    Build scan filter (device name + service UUID + manufacturer data)
/// 2. 通过系统蓝牙适配器执行扫描
///    Scan via system Bluetooth adapter
/// 3. 对扫描结果按信号强度和可连接性排序
///    Rank results by signal strength and connectability
/// 4. 将每个外设解析为 `ProductCandidate`
///    Parse each peripheral into a `ProductCandidate`
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

// ============================================================================
// 连接 / Connection
// ============================================================================

/// 用默认超时连接外设 / Connect to peripheral with default timeout.
pub async fn connect_session() -> Result<BleSession, Error> {
    connect_session_with_timeout(SCAN_TIMEOUT).await
}

/// 用自定义超时连接外设 / Connect to peripheral with custom timeout.
///
/// 扫描 → 取信号最强的候选 → 连接 → 构建 BleSession。
/// Scan → pick strongest candidate → connect → build BleSession.
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

// ============================================================================
// ProductCandidate — 候选设备的访问方法
// ProductCandidate — Accessor methods for candidate devices
// ============================================================================

impl ProductCandidate {
    /// 设备唯一标识 / Device unique identifier (platform-assigned).
    pub fn id(&self) -> &str {
        self.peripheral.id()
    }

    /// 设备广播名称 / Advertised local name.
    pub fn local_name(&self) -> Option<&str> {
        self.peripheral.local_name()
    }

    /// 信号强度 (dBm) / Signal strength in dBm.
    pub fn rssi(&self) -> Option<i16> {
        self.peripheral.properties().rssi
    }

    /// 是否可连接 / Whether the device accepts connections.
    pub fn is_connectable(&self) -> bool {
        self.peripheral.properties().is_connectable
    }

    /// 厂商身份摘要 / Decoded manufacturer identity payload.
    pub fn identity(&self) -> &advertisement_identity::ManufacturerPayload {
        &self.identity
    }

    /// 连接外设并建立 GATT 会话 / Connect to the peripheral and establish a GATT session.
    ///
    /// BLE 连接 → GATT 服务发现 → 返回 `BleSession`。
    /// BLE connect → GATT service discovery → return `BleSession`.
    pub async fn connect(self) -> Result<BleSession, Error> {
        let connection = self
            .peripheral
            .connect()
            .await
            .map_err(|e| anyhow!("Failed to connect: {e}"))?;

        build_session(connection.into_gatt().await?)
    }
}

// ============================================================================
// BleSession — GATT 操作方法
// BleSession — GATT operation methods
// ============================================================================

impl BleSession {
    // ---- Battery Service (标准 BLE) ----

    /// 读取电池电量 / Read battery level (0–100%).
    ///
    /// 返回单字节百分比值 / Returns a single-byte percentage value.
    pub async fn battery_level(&self) -> Result<u8, Error> {
        let bytes = self.gatt.read(self.battery_uuid).await?;
        if bytes.len() != 1 {
            return Err(anyhow!("Expected 1 byte, got {}", bytes.len()));
        }
        Ok(bytes[0])
    }

    // ---- Device Information Service (标准 BLE) ----

    /// 一次性读取全部设备信息 / Read all device info strings at once.
    ///
    /// 包含厂商名、型号、固件版本、软件版本，全部为只读静态字符串。
    /// Includes manufacturer name, model, firmware revision, and software revision.
    /// All are read-only static strings.
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

    // ---- Status Service (自定义, postcard 序列化) ----

    /// 读取状态值 / Read status value (postcard-deserialized bool).
    pub async fn status(&self) -> Result<bool, Error> {
        self.gatt
            .read_to(self.status_uuid)
            .await
            .map_err(|e| anyhow!("{e}"))
    }

    /// 写入状态值 / Write status value (postcard-serialized bool).
    ///
    /// `value` 为 true/false，写入后外设值变化时会触发通知。
    /// Writes a bool; the peripheral will notify on value change.
    pub async fn set_status(&self, value: bool) -> Result<(), Error> {
        self.gatt
            .write_from(self.status_uuid, &value, true)
            .await
            .map_err(|e| anyhow!("{e}"))
    }

    // ---- Echo Service (自定义) ----

    /// Echo：写入数据，等待外设通过 notify 回传相同数据 / Echo: write data, peripheral notifies it back.
    ///
    /// 用于验证 BLE 链路的数据完整性。数据长度不得超过 MTU 限制。
    /// Used to verify BLE link data integrity. Data length must not exceed MTU limit.
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

    // ---- Bulk Service (自定义) ----

    /// 重置批量传输统计 / Reset bulk transfer stats (rx/tx counters to zero).
    ///
    /// 写入 ResetStats 命令后轮询等待外设确认（最多 3 秒）。
    /// Writes ResetStats command, then polls until peripheral confirms (up to 3s).
    pub async fn reset_bulk_stats(&self) -> Result<(), Error> {
        self.gatt
            .write_from(
                self.bulk_control_uuid,
                &bulk::BulkControlCommand::ResetStats,
                true,
            )
            .await
            .map_err(|e| anyhow!("{e}"))?;

        // 轮询等待统计归零 / Poll until stats read as zeroed
        for _ in 0..30 {
            sleep(Duration::from_millis(100)).await;
            let stats = self.read_bulk_stats().await?;
            if stats == bulk::BulkStats::default() {
                return Ok(());
            }
        }

        Err(anyhow!("Timeout waiting for stats reset"))
    }

    /// 读取批量传输统计 / Read bulk transfer stats (rx/tx byte counts).
    pub async fn read_bulk_stats(&self) -> Result<bulk::BulkStats, Error> {
        self.gatt
            .read_to(self.bulk_stats_uuid)
            .await
            .map_err(|e| anyhow!("{e}"))
    }

    /// 启动外设→Central 的批量数据流 / Start peripheral→Central bulk notify stream.
    ///
    /// 外设收到命令后通过 notify 逐块推送数据，直到达到 `total_bytes`。
    /// The peripheral pushes data chunks via notify until `total_bytes` is reached.
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

    /// 向外设上传一块数据 / Upload one chunk to peripheral (write without response).
    ///
    /// 单块最大 `bulk::CHUNK_SIZE` (252) 字节。不等待外设确认。
    /// Max chunk size is `bulk::CHUNK_SIZE` (252) bytes. No response awaited.
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

    /// 上传确定性测试数据模式 / Upload a deterministic test pattern in chunks.
    ///
    /// 用 `fill_test_pattern` 逐块生成数据并上传，用于验证批量上行完整性。
    /// Generates data with `fill_test_pattern` and uploads chunk by chunk to verify bulk upload integrity.
    pub async fn upload_test_pattern(&self, total_bytes: usize) -> Result<(), Error> {
        let mut chunk = [0u8; bulk::CHUNK_SIZE];
        for offset in (0..total_bytes).step_by(bulk::CHUNK_SIZE) {
            let len = (total_bytes - offset).min(bulk::CHUNK_SIZE);
            hello_ble_common::fill_test_pattern(offset, &mut chunk[..len]);
            self.upload_bulk_data(&chunk[..len]).await?;
        }
        Ok(())
    }

    /// 接收批量数据流并验证完整性 / Receive bulk notify stream and verify data integrity.
    ///
    /// 订阅 `bulk_data` 通知，逐块与 `fill_test_pattern` 生成的期望值比对。
    /// Subscribes to `bulk_data` notifications and compares each chunk against
    /// the expected pattern generated by `fill_test_pattern`.
    ///
    /// 超时或数据不匹配时返回错误。
    /// Returns error on timeout or data mismatch.
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

            // 检查块大小 / Verify chunk size
            if chunk_len != expected_len {
                return Err(anyhow!(
                    "Unexpected chunk size: {}, expected {}",
                    chunk_len,
                    expected_len
                ));
            }

            // 比对数据内容 / Compare data content
            hello_ble_common::fill_test_pattern(received, &mut expected[..chunk_len]);
            if next.as_slice() != &expected[..chunk_len] {
                return Err(anyhow!("Bulk data mismatch at offset {}", received));
            }

            received += chunk_len;
        }

        Ok(())
    }

    // ---- 通用 / General-purpose ----

    /// 订阅指定特征的通知流 / Subscribe to notifications for a specific characteristic.
    pub async fn notifications(
        &self,
        uuid: Uuid,
    ) -> Result<impl StreamExt<Item = Result<Vec<u8>, Error>> + Unpin + '_, Error> {
        let stream = self.gatt.notifications(uuid).await?;
        Ok(stream.map(|result| result.map_err(|e| anyhow!("{e}"))))
    }

    /// 断开 BLE 连接 / Disconnect from the peripheral.
    pub async fn disconnect(&self) -> Result<(), Error> {
        self.gatt.connection().disconnect().await?;
        Ok(())
    }

    /// 检查连接是否仍活跃 / Check if the connection is still active.
    pub async fn is_connected(&self) -> bool {
        self.gatt.connection().is_connected().await
    }

    /// 调试：列出所有已发现的特征 UUID / Debug: list all discovered characteristic UUIDs.
    pub async fn list_characteristics(&self) -> BtleplusResult<Vec<String>> {
        use futures_util::TryStreamExt;

        let chars = self.gatt.discovered_characteristics().await?;
        chars
            .map_ok(|characteristic| characteristic.uuid().to_string())
            .try_collect()
            .await
    }

    // ---- UUID 访问器 / UUID accessors (for test/notification subscription) ----

    /// 电量特征 UUID / Battery Level characteristic UUID.
    pub fn battery_uuid(&self) -> Uuid {
        self.battery_uuid
    }

    /// Echo 特征 UUID / Echo characteristic UUID.
    pub fn echo_uuid(&self) -> Uuid {
        self.echo_uuid
    }
}

// ============================================================================
// 内部构造函数 / Internal constructors
// ============================================================================

/// 从 GATT 客户端构建 BleSession，绑定所有已知特征 UUID。
/// Build a BleSession from a GATT client, binding all known characteristic UUIDs.
fn build_session(gatt: Client) -> Result<BleSession, Error> {
    Ok(BleSession {
        gatt,
        // 标准 BLE 16-bit UUID / Standard BLE 16-bit UUIDs
        battery_uuid: Uuid::from_u16(battery::LEVEL_UUID16),
        manufacturer_uuid: Uuid::from_u16(device_info::MANUFACTURER_NAME_UUID16),
        model_uuid: Uuid::from_u16(device_info::MODEL_NUMBER_UUID16),
        firmware_uuid: Uuid::from_u16(device_info::FIRMWARE_REVISION_UUID16),
        software_uuid: Uuid::from_u16(device_info::SOFTWARE_REVISION_UUID16),
        // 自定义 128-bit UUID / Custom 128-bit UUIDs
        echo_uuid: Uuid::from_u128(echo::UUID),
        status_uuid: Uuid::from_u128(status::UUID),
        bulk_control_uuid: Uuid::from_u128(bulk::CONTROL_UUID),
        bulk_data_uuid: Uuid::from_u128(bulk::CHUNK_UUID),
        bulk_stats_uuid: Uuid::from_u128(bulk::STATS_UUID),
    })
}

// ============================================================================
// 扫描过滤与排序 / Scan filtering & ranking
// ============================================================================

/// 构建产品扫描过滤器 / Build the product scan filter.
///
/// 按三个条件过滤外设 / Filters peripherals by three criteria:
/// 1. 广播名匹配 `PERIPHERAL_NAME` / Name matches `PERIPHERAL_NAME`
/// 2. 包含 Battery Service UUID / Contains Battery Service UUID
/// 3. 厂商数据匹配产品身份 / Manufacturer data matches product identity
fn build_product_scan_filter() -> ScanFilter {
    ScanFilter::default()
        .with_name_pattern(PERIPHERAL_NAME)
        .with_service_uuid(Uuid::from_u16(battery::SERVICE_UUID16))
        .with_manufacturer_company_id(advertisement_identity::DEVELOPMENT_COMPANY_ID)
        .with_manufacturer_data(matches_product_identity)
}

/// 构建设备选择器 / Build the peripheral selector.
///
/// 优先选择可连接且信号最强的设备 / Prefers connectable devices with strongest signal.
fn build_product_selector() -> Selector {
    Selector::default()
        .prefer_connectable()
        .prefer_strongest_signal()
}

// ============================================================================
// 厂商数据解析 / Manufacturer data decoding
// ============================================================================

/// 从扫描结果提取候选设备 / Extract a ProductCandidate from a scanned peripheral.
///
/// 解析外设的 manufacturer_data 为 `ManufacturerPayload`。
/// Parses the peripheral's manufacturer_data into a `ManufacturerPayload`.
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

/// 检查厂商数据是否属于本产品 / Check if manufacturer data belongs to this product.
///
/// 验证版本号和产品 ID 是否匹配 / Verifies version and product ID match.
fn matches_product_identity(data: &ManufacturerData) -> bool {
    decode_manufacturer_payload(data).is_some_and(|payload| {
        payload.version == advertisement_identity::VERSION
            && payload.product_id == advertisement_identity::PRODUCT_ID_HELLO_ESPCX
    })
}

/// 解码厂商数据为 ManufacturerPayload / Decode raw manufacturer data into ManufacturerPayload.
///
/// 布局（V1）/ Layout (V1):
/// - `payload[0]`: version (u8)
/// - `payload[1]`: product_id (u8)
/// - `payload[2..6]`: unit_id (u32 LE)
/// - `payload[6]`: flags (u8)
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
