//! BLE 协议栈初始化和驱动循环
//! BLE stack initialization and driver loop.
//!
//! ## 初始化阶段 / Init Stage
//!
//! `init()` 完成以下工作：
//! `init()` performs the following:
//!
//! 1. 根据 `config.address` 生成随机蓝牙地址
//!    Generate random Bluetooth address from `config.address`
//! 2. 分配 `HostResources<CONN, L2CAP>` 并泄漏为 `'static`
//!    Allocate `HostResources<CONN, L2CAP>` and leak it to `'static`
//! 3. 构建 `Stack` 并泄漏为 `'static`
//!    Build `Stack` and leak it to `'static`
//! 4. 返回 `InitializedStack { peripheral, runner }`
//!    Return `InitializedStack { peripheral, runner }`
//!
//! 泄漏 `HostResources` 和 `Stack` 是为了让 `AttributeServer` 能够拥有 `'static` 生命周期。
//! Leaking `HostResources` and `Stack` enables `AttributeServer` to have `'static` lifetime.

use alloc::boxed::Box;
use rtt_target::rprintln;
use trouble_host::prelude::*;

/// Init 阶段的主机配置 / Host configuration for the init stage.
pub struct InitConfig {
    /// 蓝牙地址（6 字节）/ Bluetooth address (6 bytes).
    pub address: [u8; 6],
}

impl Default for InitConfig {
    fn default() -> Self {
        Self { address: [0; 6] }
    }
}

/// 初始化后的 BLE 协议栈组件 / Initialized BLE stack pieces.
///
/// 由 `init()` 返回，包含 `Peripheral`（广播/连接控制）和 `Runner`（协议栈驱动）。
/// Returned by `init()`, contains `Peripheral` (advertising/connection control)
/// and `Runner` (stack driver).
///
/// ## 使用方式 / Usage
///
/// ```text
/// let stack = easyble::gap::init(controller, config);
/// join(
///     async { easyble::gap::run_stack(stack.runner).await; },
///     async { /* advertising loop using stack.peripheral */ },
/// ).await;
/// ```
pub struct InitializedStack<C: Controller + 'static> {
    /// Peripheral — 控制广播和连接接受
    /// Peripheral — controls advertising and connection acceptance.
    pub peripheral: Peripheral<'static, C, DefaultPacketPool>,
    /// Runner — 驱动底层 BLE 协议栈（需要在后台持续运行）
    /// Runner — drives the underlying BLE stack (must run continuously in background).
    pub runner: Runner<'static, C, DefaultPacketPool>,
}

/// 运行 Init 阶段：分配协议栈资源，设置外设地址，返回后续阶段所需的组件
/// Run the init stage: allocate stack resources, set peripheral address,
/// and return components needed by later stages.
///
/// ## 参数 / Parameters
///
/// - `controller`: BLE 控制器（硬件抽象）/ BLE controller (hardware abstraction)
/// - `config`: 初始化配置（含地址）/ Init config (includes address)
/// - `CONN`: 最大并发连接数 / Maximum concurrent connections
/// - `L2CAP`: L2CAP 通道数（Signal + ATT）/ L2CAP channel count (Signal + ATT)
pub fn init<C, const CONN: usize, const L2CAP: usize>(
    controller: C,
    config: InitConfig,
) -> InitializedStack<C>
where
    C: Controller + 'static,
{
    let address = Address::random(config.address);
    rprintln!("[easyble] address = {:?}", address);

    // 分配 HostResources 并泄漏为 'static，使 AttributeServer 生命周期合法
    // Allocate HostResources and leak to 'static for AttributeServer 'static lifetime
    let resources = Box::leak(Box::new(HostResources::<DefaultPacketPool, CONN, L2CAP, 1>::new()));
    // 分配 Stack 并泄漏为 'static
    // Allocate Stack and leak to 'static
    let stack = Box::leak(Box::new(
        trouble_host::new(controller, resources).set_random_address(address),
    ));
    let Host {
        peripheral, runner, ..
    } = stack.build();

    InitializedStack { peripheral, runner }
}

/// 驱动底层 BLE 协议栈直到返回或失败
/// Drive the underlying BLE stack until it returns or fails.
///
/// 需要在后台并发运行，`runner.run()` 驱动所有 BLE 协议处理。
/// Must run concurrently in the background; `runner.run()` drives all BLE protocol processing.
pub async fn run_stack<C: Controller + 'static>(
    mut runner: Runner<'static, C, DefaultPacketPool>,
) -> Result<(), BleHostError<C::Error>> {
    runner.run().await
}
