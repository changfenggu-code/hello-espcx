//! 产品广播数据构建 / Product advertisement data building.
//!
//! 构建符合本产品协议的 BLE 广播包和扫描响应包。
//! Builds BLE advertisement and scan response payloads conforming to this product's protocol.
//!
//! 广播包包含 / Advertisement contains:
//! - Flags: LE General Discoverable + BR/EDR Not Supported
//! - Service UUIDs: Battery Service (0x180F)
//! - Complete Local Name: `hello-espcx`
//! - Manufacturer Specific Data: 产品身份摘要（version + product_id + unit_id + flags）

use hello_ble_common::{
    PERIPHERAL_ADDRESS, PERIPHERAL_NAME, advertisement_identity, battery,
};
use trouble_host::prelude::*;

/// 预构建的广播数据 / Pre-built advertisement data.
///
/// BLE 广播包最大 31 字节，扫描响应包最大 31 字节。
/// BLE advertisement max 31 bytes, scan response max 31 bytes.
pub(crate) struct AdvertisementData {
    /// 广播数据缓冲区 / Advertisement data buffer.
    pub(crate) adv_data: [u8; 31],
    /// 广播数据有效长度 / Valid length of advertisement data.
    pub(crate) adv_len: usize,
    /// 扫描响应数据缓冲区 / Scan response data buffer.
    pub(crate) scan_data: [u8; 31],
    /// 扫描响应有效长度 / Valid length of scan response data.
    pub(crate) scan_len: usize,
}

/// 构建产品的广播数据 / Build the product's advertisement payload.
///
/// 将设备名、服务 UUID、厂商数据编码为 BLE AD Structure 序列。
/// Encodes device name, service UUIDs, and manufacturer data into BLE AD Structure sequence.
///
/// 当前未使用扫描响应（`scan_len = 0`）。
/// Currently scan response is unused (`scan_len = 0`).
pub(crate) fn build_product_advertisement() -> Result<AdvertisementData, Error> {
    let mut adv_data = [0; 31];
    let scan_data = [0; 31];

    // 构建厂商身份摘要（version + product_id + unit_id + flags）
    // Build manufacturer identity payload (version + product_id + unit_id + flags)
    let manufacturer_payload = advertisement_identity::ManufacturerPayload::new(
        advertisement_identity::PRODUCT_ID_HELLO_ESPCX,
        advertisement_identity::unit_id_from_address(PERIPHERAL_ADDRESS),
        0, // flags: 无特殊标志 / no special flags
    )
    .to_bytes();

    // 编码 AD Structure 序列 / Encode AD Structure sequence
    let adv_len = AdStructure::encode_slice(
        &[
            // BLE 发现模式标志 / BLE discoverability flags
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
            // 包含的服务 UUID 列表 / Included service UUID list
            AdStructure::ServiceUuids16(&[battery::SERVICE_UUID16.to_le_bytes()]),
            // 完整设备名 / Complete local device name
            AdStructure::CompleteLocalName(PERIPHERAL_NAME.as_bytes()),
            // 厂商自定义数据 / Manufacturer-specific data
            AdStructure::ManufacturerSpecificData {
                company_identifier: advertisement_identity::DEVELOPMENT_COMPANY_ID,
                payload: &manufacturer_payload,
            },
        ],
        &mut adv_data[..],
    )?;

    Ok(AdvertisementData {
        adv_data,
        adv_len,
        scan_data,
        scan_len: 0,
    })
}
