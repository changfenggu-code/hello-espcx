//! 产品 GATT server 组装 / Product GATT server assembly.
//!
//! 用 `#[gatt_server]` 宏将所有服务组合成一个 Server 类型。
//! Uses the `#[gatt_server]` macro to compose all services into one Server type.
//!
//! 这个文件是产品特定的——服务列表属于产品决策，不属于通用运行时。
//! This file is product-specific — the service list is a product decision, not generic runtime.

#![allow(dead_code)]

use crate::services::{
    BatteryService, BulkService, DeviceInfoService, EchoService, StatusService,
};
use hello_ble_common::PERIPHERAL_NAME;
use trouble_host::prelude::*;

/// 产品的 GATT 属性服务器 / The product's GATT attribute server.
///
/// 由 5 个服务组合而成 / Composed of 5 services:
/// - `battery_service` — 电量 / Battery level
/// - `device_info_service` — 设备信息 / Device information
/// - `echo_service` — 回声 / Echo
/// - `status_service` — 状态 / Status
/// - `bulk_service` — 批量传输 / Bulk transfer
#[allow(clippy::needless_borrows_for_generic_args)]
#[allow(dead_code)] // device_info_service 提供标准 BLE 合规性 / provided for standard BLE compliance
#[gatt_server]
pub(crate) struct Server {
    pub(crate) battery_service: BatteryService,
    pub(crate) device_info_service: DeviceInfoService,
    pub(crate) echo_service: EchoService,
    pub(crate) status_service: StatusService,
    pub(crate) bulk_service: BulkService,
}

/// 构建并初始化产品 GATT server / Build and initialize the product GATT server.
///
/// 配置 GAP 外设角色（设备名 + 外观）并创建所有服务实例。
/// Configures GAP peripheral role (device name + appearance) and creates all service instances.
pub(crate) fn build_server<'values>() -> Server<'values> {
    Server::new_with_config(GapConfig::Peripheral(PeripheralConfig {
        name: PERIPHERAL_NAME,
        appearance: &appearance::power_device::GENERIC_POWER_DEVICE,
    }))
    .unwrap()
}
