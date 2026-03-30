//! easyble — 简洁的 BLE 外设运行时 / Ergonomic BLE peripheral runtime
//!
//! 基于 `trouble-host` 构建，提供简洁的外设生命周期管理。
//!
//! ## 架构 / Architecture
//!
//! ```text
//! App (impl AppHooks)
//!   ├─ build_advertisement()  →  广播数据
//!   ├─ build_server()        →  构建 GATT server（返回 server）
//!   └─ on_session()          →  连接会话（使用 self 中的 server）
//!
//! easyble (生命周期循环)
//!   └─ advertising → accept → on_session → (断开) → advertising → ...
//! ```

#![no_std]
extern crate alloc;

pub mod gap;

use embassy_futures::join::join;
use gap::advertising::advertise;
use rtt_target::rprintln;
use trouble_host::prelude::*;

// Re-export for convenience
pub use gap::advertising::AdvertisementView;
pub use trouble_host::prelude::{Connection, Error, GattEvent};

// 显式导入 alloc::boxed::Box（no_std 中 Box 不在 prelude 中）
// Explicitly import Box (not in prelude in no_std context)
use alloc::boxed::Box;

/// 广播数据 / Advertisement data.
pub struct AdvertisementData {
    /// 广播数据缓冲区 / Advertisement data buffer.
    pub adv_data: [u8; 31],
    /// 广播数据有效长度 / Valid length of advertisement data.
    pub adv_len: usize,
    /// 扫描响应数据缓冲区 / Scan response data buffer.
    pub scan_data: [u8; 31],
    /// 扫描响应有效长度 / Valid length of scan response data.
    pub scan_len: usize,
}

impl AdvertisementData {
    /// 转换为视图引用 / Convert to view reference.
    pub fn as_view(&self) -> AdvertisementView<'_> {
        AdvertisementView {
            adv_data: &self.adv_data[..self.adv_len],
            scan_data: &self.scan_data[..self.scan_len],
        }
    }
}

/// BLE 外设运行时配置 / BLE peripheral runtime configuration.
pub struct Config<'a> {
    /// 蓝牙地址（6 字节）/ Bluetooth address (6 bytes).
    pub address: [u8; 6],
    /// 致命错误时的处理策略，默认死循环等待看门狗。
    /// Policy on fatal errors. Default: spin loop waiting for watchdog.
    pub on_fatal: &'a dyn Fn(&str),
}

impl Default for Config<'_> {
    fn default() -> Self {
        Self {
            address: [0u8; 6],
            on_fatal: &|_msg| loop {},
        }
    }
}

/// App 层必须实现的行为接口 / Behavior interface that the app layer must implement.
///
/// 三个方法对应三个生命周期阶段：
/// - `build_advertisement`: Init 阶段，构建广播数据
/// - `build_server`: Init 阶段，构建 GATT AttributeServer
/// - `on_session`: 每个连接的会话阶段
pub trait AppHooks {
    /// 构建广播数据 / Build advertisement data.
    fn build_advertisement(&mut self) -> Result<AdvertisementData, Error>;

    /// 构建 GATT AttributeServer 并返回。
    ///
    /// 在 `Init` 阶段调用一次。被 `on_session` 通过 `self` 访问。
    /// Called once during `Init`. Accessed by `on_session` through `self`.
    ///
    /// ## 实现提示 / Implementation note
    ///
    /// `AttributeServer` 引用 `HostResources` 的内部数据。
    /// 需要用 `core::mem::forget(resources)` 将 `HostResources` 泄漏为 `'static`，
    /// 使 `AttributeServer` 的 `'static` 生命周期合法。
    fn build_server(&mut self) -> Result<(), Error>;

    /// 连接建立后的会话回调 / Called after a connection is established.
    ///
    /// 收到 `conn` 后自行完成绑定 GATT server 和会话运行。
    /// Receives `conn` and handles GATT server binding and session running internally.
    #[allow(async_fn_in_trait)]
    async fn on_session(&mut self, conn: Connection<'_, DefaultPacketPool>);
}

/// BLE 外设运行时入口 / BLE peripheral runtime entry point.
///
/// 构建 BLE stack，运行生命周期循环，永不返回。
pub async fn run<C, H, const CONN: usize, const L2CAP: usize>(
    controller: C,
    config: Config<'_>,
    hooks: &mut H,
) where
    C: Controller,
    H: AppHooks,
{
    // === Init: 构建 stack ===
    let address = Address::random(config.address);
    rprintln!("[easyble] address = {:?}", address);

    // 将 HostResources 分配在堆上并泄漏，使 AttributeServer 的 'static 生命周期合法
    // Allocate HostResources on the heap and leak it, so AttributeServer can have 'static lifetime
    let resources = Box::leak(Box::new(HostResources::<
        DefaultPacketPool,
        CONN,
        L2CAP,
        1,
    >::new()));
    let stack = trouble_host::new(controller, resources).set_random_address(address);
    let Host {
        mut peripheral, runner, ..
    } = stack.build();

    // Init: 构建 app 数据
    match hooks.build_advertisement() {
        Ok(_) => {}
        Err(_e) => {
            rprintln!("[easyble] build_advertisement failed");
            (config.on_fatal)("build_advertisement failed");
        }
    }

    match hooks.build_server() {
        Ok(_) => {}
        Err(_e) => {
            rprintln!("[easyble] build_server failed");
            (config.on_fatal)("build_server failed");
        }
    }

    let adv_data = hooks.build_advertisement().unwrap_or(AdvertisementData {
        adv_data: [0; 31],
        adv_len: 0,
        scan_data: [0; 31],
        scan_len: 0,
    });
    let adv_view = adv_data.as_view();

    // === 并行运行：协议栈驱动 + 外设生命周期 ===
    join(ble_task(runner), main_loop(&mut peripheral, &adv_view, hooks)).await;
}

/// BLE stack 驱动循环 / BLE stack driver loop.
async fn ble_task<C: Controller, P: PacketPool>(mut runner: Runner<'_, C, P>) {
    let _ = runner.run().await;
}

/// 外设生命周期主循环 / Peripheral lifecycle main loop.
async fn main_loop<'a, C: Controller, H: AppHooks>(
    peripheral: &mut Peripheral<'a, C, DefaultPacketPool>,
    adv_view: &AdvertisementView<'_>,
    hooks: &mut H,
) where
    C: Controller,
{
    loop {
        match advertise(peripheral, adv_view).await {
            Ok(conn) => {
                hooks.on_session(conn).await;
            }
            Err(_e) => {
                rprintln!("[easyble] advertise error");
            }
        }
    }
}
