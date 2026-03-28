#![no_std]

//! BLE Common Types for hello-espcx
//!
//! This crate defines shared constants and types for the BLE peripheral and central.
//!
//! ## 文件的作用
//!
//! 两端通信的前提是**双方对 UUID、容量、数据格式达成一致**。
//! `common/` 就是这个"协议合同"——所有常量和数据类型都定义在这里，
//! peripheral 和 central 各自引用它，确保两边说的是同一套语言。
//!
//! ## BLE 基本概念速查
//!
//! - **Service（服务）**：一组相关特征的集合，例如 Battery Service 包含电量值
//! - **Characteristic（特征）**：最小的数据单元，例如 Battery Level 特征存电量值
//! - **UUID**：服务的唯一标识。标准 BLE 用 16 位 UUID（0x1800~0x180F），
//!   自定义服务用 128 位 UUID 避免冲突
//! - **MTU**：Maximum Transmission Unit，BLE 单次传输的最大字节数，这里是 255
//! - **ATT Payload**：ATT 协议层实际载荷 = MTU - 3 = 252 字节

use serde::{Deserialize, Serialize};

// ============================================================================
// 设备标识
// ============================================================================

/// Peripheral 的广播名称。Central 按这个名字扫描设备。
pub const PERIPHERAL_NAME: &str = "hello-espcx";

/// Peripheral 的固定随机蓝牙地址。
/// ESP32 每次上电保持同一个地址，方便 Central 直接连接。
pub const PERIPHERAL_ADDRESS: [u8; 6] = [0xff, 0x8f, 0x1a, 0x05, 0xe4, 0xff];

// ============================================================================
// BLE 基础常量
// ============================================================================

/// BLE 连接的最大传输单元（MTU）。影响单次读写能传多少字节。
pub const BLE_MTU: usize = 255;

/// ATT 层的最大载荷 = MTU - 3（ATT 头占 3 字节）。
/// 这是单次 GATT 操作实际能传的最大字节数。
pub const ATT_PAYLOAD_MAX: usize = BLE_MTU - 3; // 252

// ============================================================================
// 服务一：battery（标准 BLE）
//
// 用途：外设定期通知当前电池电量（0-100%）。
// Central 可以主动读取，也可以订阅通知被动接收。
// =============================================================================

/// Battery Service — 标准 BLE 规范定义，所有 BLE 设备通用。
pub mod battery {
    /// Battery Service 的 UUID（16 位，BLE 标准分配）。
    pub const SERVICE_UUID16: u16 = 0x180F;

    /// Battery Level 特征的 UUID（16 位，BLE 标准分配）。
    pub const LEVEL_UUID16: u16 = 0x2A19;
}

// ============================================================================
// 广播身份摘要（manufacturer_data 规划）
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
        /// version 由 `VERSION` 常量决定，调用方无需关心。
        pub const fn new(product_id: u8, unit_id: u32, flags: u8) -> Self {
            Self {
                version: VERSION,
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
// 服务二：device_info（标准 BLE）
//
// 用途：返回静态设备信息（厂商、型号、固件版本等）。
// 全部只读，Central 读取一次即可。
// =============================================================================

/// Device Information Service — 标准 BLE 规范定义。
pub mod device_info {
    /// Device Information Service 的 UUID（16 位，BLE 标准分配）。
    pub const SERVICE_UUID16: u16 = 0x180A;

    /// 制造商名称特征 UUID。
    pub const MANUFACTURER_NAME_UUID16: u16 = 0x2A29;
    /// 型号特征 UUID。
    pub const MODEL_NUMBER_UUID16: u16 = 0x2A24;
    /// 固件版本特征 UUID。
    pub const FIRMWARE_REVISION_UUID16: u16 = 0x2A26;
    /// 软件版本特征 UUID。
    pub const SOFTWARE_REVISION_UUID16: u16 = 0x2A28;

    /// Device Info 特征的字符串最大长度限制。
    pub const STRING_CAPACITY: usize = 30;
}

// ============================================================================
// 服务三：echo（自定义）
//
// 用途：验证双向数据完整性。
// Central 写入任意数据，Peripheral 立刻把同样的数据通过 notify 发回来。
// 如果回来的数据和发出去的一致，说明 BLE 链路没有丢包或篡改。
//
// 数据流：
//   Central ───write──▶ Peripheral（收到数据）
//   Central ◀──notify── Peripheral（立刻回传同样数据）
//   Central 验证两边数据一致
// =============================================================================

/// Echo Service — 自定义服务，用于双向数据完整性验证。
pub mod echo {
    // 引用父级的 ATT_PAYLOAD_MAX，确保数据不超 MTU 限制
    use super::ATT_PAYLOAD_MAX;

    /// Echo Service 的 UUID（128 位自定义）。
    pub const SERVICE_UUID: u128 = 0x4088_13df_5dd4_1f87_ec11_cdb0_0110_1001;
    /// Echo 特征的 UUID。
    pub const UUID: u128 = 0x4088_13df_5dd4_1f87_ec11_cdb0_0110_1002;
    /// Echo payload 最大长度（MTU - 3 = 252 字节）。
    pub const CAPACITY: usize = ATT_PAYLOAD_MAX;
}

// ============================================================================
// 服务四：status（自定义）
//
// 用途：演示 GATT 的 read + write + notify 三种操作组合。
// Central 可以读写一个布尔值，Peripheral 在值变更时主动通知。
//
// 数据流：
//   Central ──write(true)──▶ Peripheral（设置状态）
//   Central ───read()──────▶ Peripheral（读取当前状态）
//   Peripheral ◀─notify──  当值变化时主动推送
// =============================================================================

/// Status Service — 自定义服务，演示 read/write/notify 三种操作。
pub mod status {
    /// Status Service 的 UUID（128 位自定义）。
    pub const SERVICE_UUID: u128 = 0x4088_13df_5dd4_1f87_ec11_cdb0_0110_2001;
    /// Status 特征的 UUID。
    pub const UUID: u128 = 0x4088_13df_5dd4_1f87_ec11_cdb0_0110_2002;
    /// Status 值序列化后的最大字节数。
    pub const CAPACITY: usize = 4;
}

// ============================================================================
// 服务五：bulk（自定义）
//
// 用途：大批量数据传输，支持控制命令、流式下发和传输统计。
// 是这个项目里最复杂的服务，分为三个特征：
//   - control：控制命令（Idle / ResetStats / StartStream）
//   - data：实际传输的数据
//   - stats：传输统计（rx/tx 字节数）
//
// 数据流（Peripheral → Central 下发）：
//   Central ──write(StartStream{10000})──▶ Peripheral（发起流）
//   Central ◀──notify(data, chunk)────── Peripheral（逐块发送数据）
//   Central ◀──notify(data, chunk)────── （每块 252 字节）
//   ... 直到发完
//   Central ───read(STATS)──────────────▶ 读取传输统计验证完整性
//
// 数据流（Central → Peripheral 上传）：
//   Central ──write(chunk)──────────────▶ Peripheral（逐块发送）
//   ... 重复直到发完
//   Central ───read(STATS)──────────────▶ 验证 Peripheral 收到了多少
// =============================================================================

/// Bulk Service — 自定义服务，大批量数据传输。
pub mod bulk {
    use super::{ATT_PAYLOAD_MAX, Deserialize, Serialize};

    /// Bulk Service 的 UUID（128 位自定义）。
    pub const SERVICE_UUID: u128 = 0x4088_13df_5dd4_1f87_ec11_cdb0_0110_3001;
    /// 控制特征的 UUID（写命令：Idle / ResetStats / StartStream）。
    pub const CONTROL_UUID: u128 = 0x4088_13df_5dd4_1f87_ec11_cdb0_0110_3002;
    /// 数据传输特征的 UUID（双向：写 = 上传，notify = 下发）。
    pub const CHUNK_UUID: u128 = 0x4088_13df_5dd4_1f87_ec11_cdb0_0110_3003;
    /// 统计特征的 UUID（读：返回 rx/tx 字节数）。
    pub const STATS_UUID: u128 = 0x4088_13df_5dd4_1f87_ec11_cdb0_0110_3004;

    /// 控制特征值序列化的最大字节数。
    pub const CONTROL_CAPACITY: usize = 8;
    /// 单块数据的最大字节数（MTU 限制）。
    pub const CHUNK_SIZE: usize = ATT_PAYLOAD_MAX;
    /// 统计特征值序列化的最大字节数。
    pub const STATS_CAPACITY: usize = 16;

    /// Bulk Service 的控制命令。
    ///
    /// 数据用 [postcard] 序列化后通过 GATT write 操作发送。
    /// Central 向 control 特征写入命令，Peripheral 解析并执行。
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub enum BulkControlCommand {
        /// 空闲状态（Peripheral 重启后的默认状态）。
        Idle,
        /// 重置 rx/tx 计数器到 0。
        ResetStats,
        /// 让 Peripheral 开始通过 notify 向 Central 推送数据流。
        /// `total_bytes` 指定总共要发送多少字节。
        StartStream { total_bytes: u32 },
    }

    /// Bulk Service 的传输统计。
    ///
    /// 用于验证大批量传输的数据完整性。
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
    pub struct BulkStats {
        /// Peripheral 收到的字节数（Central 上传方向）。
        pub rx_bytes: u32,
        /// Peripheral 发出的字节数（Peripheral 下发方向）。
        pub tx_bytes: u32,
    }
}

// ============================================================================
// 向后兼容别名
//
// 当前正在从扁平的命名空间迁移到子模块结构（如 battery::LEVEL_UUID16）。
// 这些 `as` 别名让新旧两套名字共存，方便逐步迁移。
// 迁移完成后这些别名将被删除。
// ============================================================================

pub use battery::{LEVEL_UUID16 as BATTERY_LEVEL_UUID16, SERVICE_UUID16 as SERVICE_BATTERY_UUID16};
pub use bulk::{
    BulkControlCommand, BulkStats, CHUNK_SIZE as BULK_CHUNK_SIZE, CHUNK_UUID as BULK_CHUNK_UUID,
    CONTROL_CAPACITY as BULK_CONTROL_CAPACITY, CONTROL_UUID as BULK_CONTROL_UUID,
    SERVICE_UUID as SERVICE_BULK_UUID, STATS_CAPACITY as BULK_STATS_CAPACITY,
    STATS_UUID as BULK_STATS_UUID,
};
pub use device_info::{
    FIRMWARE_REVISION_UUID16 as DEVICE_INFO_FIRMWARE_REVISION_UUID16,
    MANUFACTURER_NAME_UUID16 as DEVICE_INFO_MANUFACTURER_NAME_UUID16,
    MODEL_NUMBER_UUID16 as DEVICE_INFO_MODEL_NUMBER_UUID16,
    SERVICE_UUID16 as SERVICE_DEVICE_INFO_UUID16,
    SOFTWARE_REVISION_UUID16 as DEVICE_INFO_SOFTWARE_REVISION_UUID16,
    STRING_CAPACITY as DEVICE_INFO_STRING_CAPACITY,
};
pub use echo::{CAPACITY as ECHO_CAPACITY, SERVICE_UUID as SERVICE_ECHO_UUID, UUID as ECHO_UUID};
pub use status::{
    CAPACITY as STATUS_CAPACITY, SERVICE_UUID as SERVICE_STATUS_UUID, UUID as STATUS_UUID,
};

// ============================================================================
// 测试数据生成
//
// 用于 Bulk 大批量传输的数据完整性验证。
// Peripheral 和 Central 使用完全相同的公式生成数据：
//   byte = ((offset + index) * 17 + 29) % 256
//
// 这样 Central 收到数据后用同一公式重新计算期望值，即可验证完整性，
// 不需要事先约定一大段"期望数据"的内容。
// ============================================================================

/// 用确定性公式填满 buffer。
///
/// 两端（Peripheral 发送方、Central 接收方）用完全相同的算法，
/// 所以 Central 知道"期望收到什么数据"，可以实时校验每一块是否正确。
///
/// 公式选得比较随意（乘 17 加 29），优点是计算快、不需要随机数种子。
pub fn fill_test_pattern(start_offset: usize, buffer: &mut [u8]) {
    for (index, byte) in buffer.iter_mut().enumerate() {
        *byte = ((((start_offset + index) * 17) + 29) % 256) as u8;
    }
}
