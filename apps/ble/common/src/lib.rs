#![no_std]

//! BLE Common Types for hello-espcx
//!
//! This crate defines shared constants and types for both the BLE peripheral
//! and the BLE central.
//!
//! ## 文件的作用
//!
//! 两端通信的前提是：**双方对 UUID、容量、数据格式达成一致**。
//! `common/` 就是这份“协议合同”。
//!
//! 这里放的是：
//! - 设备基础标识
//! - 传输相关基础常量
//! - 广播身份摘要结构
//! - 各个服务及特征的 UUID 命名空间
//! - 双端共用的小型辅助函数
//!
//! peripheral 和 central 都引用它，确保两边说的是同一套语言。
//!
//! ## BLE 基本概念速查
//!
//! - **Service（服务）**：一组相关特征的集合，例如 Battery Service
//! - **Characteristic（特征）**：最小的数据单元，例如 Battery Level
//! - **UUID**：服务或特征的唯一标识；标准 BLE 多用 16 位 UUID，自定义服务/特征多用 128 位 UUID
//! - **MTU**：Maximum Transmission Unit，BLE 单次传输的最大字节数
//! - **ATT Payload**：ATT 协议层实际载荷 = MTU - 3

use serde::{Deserialize, Serialize};

// ============================================================================
// 基础信息：设备标识 + 传输常量
// ============================================================================

/// Peripheral 的广播名称。Central 按这个名字扫描设备。
pub const PERIPHERAL_NAME: &str = "hello-espcx";

/// Peripheral 的固定随机蓝牙地址。
///
/// ESP32 每次上电保持同一个地址，方便 Central 直接连接。
pub const PERIPHERAL_ADDRESS: [u8; 6] = [0xff, 0x8f, 0x1a, 0x05, 0xe4, 0xff];

/// BLE 连接的最大传输单元（MTU）。
pub const BLE_MTU: usize = 255;

/// ATT 层的最大载荷 = MTU - 3（ATT 头占 3 字节）。
pub const ATT_PAYLOAD_MAX: usize = BLE_MTU - 3; // 252

// ============================================================================
// 广播身份摘要：scan-time 设备识别信息
// ============================================================================

pub mod advertisement_identity {
    use super::{Deserialize, Serialize};

    /// Manufacturer payload format version (current).
    pub const VERSION: u8 = 1;

    /// Temporary development company identifier used only for local testing.
    ///
    /// Replace this with a real Bluetooth SIG company identifier before
    /// production use.
    pub const DEVELOPMENT_COMPANY_ID: u16 = 0xFFFF;

    /// Product identifier for the current hello-espcx product family.
    pub const PRODUCT_ID_HELLO_ESPCX: u8 = 1;

    /// Byte length of the current payload layout.
    pub const PAYLOAD_LEN: usize = 7;

    /// Reserved flag bit: device has completed configuration.
    pub const FLAG_CONFIGURED: u8 = 1 << 0;
    /// Reserved flag bit: device is bound/claimed.
    pub const FLAG_BOUND: u8 = 1 << 1;
    /// Reserved flag bit: device is in test mode.
    pub const FLAG_TEST_MODE: u8 = 1 << 2;
    /// Reserved flag bit: device reports low battery.
    pub const FLAG_LOW_BATTERY: u8 = 1 << 3;

    /// Compact manufacturer payload intended for scan-time device selection.
    ///
    /// Layout (V1):
    /// - version: u8
    /// - product_id: u8
    /// - unit_id: u32
    /// - flags: u8
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct ManufacturerPayload {
        pub version: u8,
        pub product_id: u8,
        pub unit_id: u32,
        pub flags: u8,
    }

    impl ManufacturerPayload {
        /// Create a payload with the current protocol version baked in.
        pub const fn new(version: u8, product_id: u8, unit_id: u32, flags: u8) -> Self {
            Self {
                version,
                product_id,
                unit_id,
                flags,
            }
        }

        pub const fn has_flag(&self, flag: u8) -> bool {
            self.flags & flag != 0
        }

        pub const fn to_bytes(self) -> [u8; PAYLOAD_LEN] {
            let unit_id = self.unit_id.to_le_bytes();
            [
                self.version,
                self.product_id,
                unit_id[0],
                unit_id[1],
                unit_id[2],
                unit_id[3],
                self.flags,
            ]
        }
    }

    /// Derive a stable 32-bit unit identifier from a 6-byte BLE address.
    pub const fn unit_id_from_address(address: [u8; 6]) -> u32 {
        u32::from_le_bytes([address[0], address[1], address[2], address[3]])
    }
}

// ============================================================================
// 标准 BLE 服务
// ============================================================================

// ----------------------------------------------------------------------------
// battery
// 用途：外设定期通知当前电池电量（0-100%）。
// Central 可以主动读取，也可以订阅通知被动接收。
// ----------------------------------------------------------------------------

/// Battery Service - 标准 BLE 规范定义，所有 BLE 设备通用。
pub mod battery {
    /// Service UUID 常量。
    pub mod service {
        /// Battery Service 的 UUID（16 位，BLE 标准分配）。
        pub const UUID16: u16 = 0x180F;
    }

    /// Characteristic UUID 常量。
    pub mod characteristic {
        /// Battery Level 特征的 UUID（16 位，BLE 标准分配）。
        pub const LEVEL_UUID16: u16 = 0x2A19;
    }
}

// ----------------------------------------------------------------------------
// device_info
// 用途：返回静态设备信息（厂商、型号、固件版本等）。
// 全部只读，Central 读取一次即可。
// ----------------------------------------------------------------------------

/// Device Information Service - 标准 BLE 规范定义。
pub mod device_info {
    /// Service UUID 常量。
    pub mod service {
        /// Device Information Service 的 UUID（16 位，BLE 标准分配）。
        pub const UUID16: u16 = 0x180A;
    }

    /// Characteristic UUID 常量。
    pub mod characteristic {
        /// 制造商名称特征 UUID。
        pub const MANUFACTURER_NAME_UUID16: u16 = 0x2A29;
        /// 型号特征 UUID。
        pub const MODEL_NUMBER_UUID16: u16 = 0x2A24;
        /// 固件版本特征 UUID。
        pub const FIRMWARE_REVISION_UUID16: u16 = 0x2A26;
        /// 软件版本特征 UUID。
        pub const SOFTWARE_REVISION_UUID16: u16 = 0x2A28;
    }

    /// Device Info 特征字符串的最大长度限制。
    pub const STRING_CAPACITY: usize = 30;
}

// ============================================================================
// 自定义服务
// ============================================================================

// ----------------------------------------------------------------------------
// echo
// 用途：验证双向数据完整性。
// Central 写入任意数据，Peripheral 立刻把同样的数据通过 notify 发回来。
// ----------------------------------------------------------------------------

/// Echo Service - 自定义服务，用于双向数据完整性验证。
pub mod echo {
    use super::ATT_PAYLOAD_MAX;

    /// Service UUID 常量。
    pub mod service {
        /// Echo Service 的 UUID（128 位自定义）。
        pub const UUID128: u128 = 0x4088_13df_5dd4_1f87_ec11_cdb0_0110_1001;
    }

    /// Characteristic UUID 常量。
    pub mod characteristic {
        /// Echo 特征的 UUID。
        pub const ECHO_UUID128: u128 = 0x4088_13df_5dd4_1f87_ec11_cdb0_0110_1002;
    }

    /// Echo payload 最大长度（MTU - 3 = 252 字节）。
    pub const CAPACITY: usize = ATT_PAYLOAD_MAX;
}

// ----------------------------------------------------------------------------
// status
// 用途：演示 GATT 的 read + write + notify 三种操作组合。
// ----------------------------------------------------------------------------

/// Status Service - 自定义服务，演示 read/write/notify 三种操作。
pub mod status {
    /// Service UUID 常量。
    pub mod service {
        /// Status Service 的 UUID（128 位自定义）。
        pub const UUID128: u128 = 0x4088_13df_5dd4_1f87_ec11_cdb0_0110_2001;
    }

    /// Characteristic UUID 常量。
    pub mod characteristic {
        /// Status 特征的 UUID。
        pub const STATUS_UUID128: u128 = 0x4088_13df_5dd4_1f87_ec11_cdb0_0110_2002;
    }

    /// Status 值序列化后的最大字节数。
    pub const CAPACITY: usize = 4;
}

// ----------------------------------------------------------------------------
// bulk
// 用途：大批量数据传输，支持控制命令、流式下发和传输统计。
// ----------------------------------------------------------------------------

/// Bulk Service - 自定义服务，大批量数据传输。
pub mod bulk {
    use super::{ATT_PAYLOAD_MAX, Deserialize, Serialize};

    /// Service UUID 常量。
    pub mod service {
        /// Bulk Service 的 UUID（128 位自定义）。
        pub const UUID128: u128 = 0x4088_13df_5dd4_1f87_ec11_cdb0_0110_3001;
    }

    /// Characteristic UUID 常量。
    pub mod characteristic {
        /// 控制特征的 UUID（写命令：Idle / ResetStats / StartStream）。
        pub const CONTROL_UUID128: u128 = 0x4088_13df_5dd4_1f87_ec11_cdb0_0110_3002;
        /// 数据传输特征的 UUID（双向：写 = 上传，notify = 下发）。
        pub const DATA_UUID128: u128 = 0x4088_13df_5dd4_1f87_ec11_cdb0_0110_3003;
        /// 统计特征的 UUID（读：返回 rx/tx 字节数）。
        pub const STATS_UUID128: u128 = 0x4088_13df_5dd4_1f87_ec11_cdb0_0110_3004;
    }

    /// 控制特征值序列化后的最大字节数。
    pub const CONTROL_CAPACITY: usize = 8;
    /// 单块数据的最大字节数（受 MTU 限制）。
    pub const CHUNK_SIZE: usize = ATT_PAYLOAD_MAX;
    /// 统计特征值序列化后的最大字节数。
    pub const STATS_CAPACITY: usize = 16;

    /// Bulk Service 的控制命令。
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub enum BulkControlCommand {
        /// 空闲状态（Peripheral 重启后的默认状态）。
        Idle,
        /// 重置 rx/tx 计数器到 0。
        ResetStats,
        /// 让 Peripheral 开始通过 notify 向 Central 推送数据流。
        StartStream { total_bytes: u32 },
    }

    /// Bulk Service 的传输统计。
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
    pub struct BulkStats {
        /// Peripheral 收到的字节数（Central 上传方向）。
        pub rx_bytes: u32,
        /// Peripheral 发出的字节数（Peripheral 下发方向）。
        pub tx_bytes: u32,
    }
}

// ============================================================================
// 辅助工具：测试数据生成
// ============================================================================

// 用于 Bulk 大批量传输的数据完整性验证。
// Peripheral 和 Central 使用完全相同的公式生成数据：
//   byte = ((offset + index) * 17 + 29) % 256

/// 用确定性公式填满 `buffer`。
pub fn fill_test_pattern(start_offset: usize, buffer: &mut [u8]) {
    for (index, byte) in buffer.iter_mut().enumerate() {
        *byte = ((((start_offset + index) * 17) + 29) % 256) as u8;
    }
}
