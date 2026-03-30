//! BLE 外设入口 — 运行在 ESP32-C6 / BLE Peripheral entry point — runs on ESP32-C6
//!
//! ## 启动流程 / Boot Sequence
//!
//! ```text
//! main()
//!   ├─ 初始化堆内存分配器 / Init heap allocator
//!   ├─ 配置 CPU 最大频率 / Configure CPU to max clock
//!   ├─ 配置 RTOS 调度器（timer + software interrupt）/ Configure RTOS scheduler
//!   ├─ 创建 BLE controller（芯片级硬件）/ Create BLE controller
//!   └─ easyble::run(controller, config, &mut app)  → 进入外设主循环 / Enter peripheral main loop
//! ```

#![no_std]
#![no_main]

mod lib;

use crate::lib::AppState;
use embassy_executor::Spawner;
use esp_alloc as _; // 堆内存分配器，必须链接 / Heap allocator, must be linked
use esp_hal::{
    clock::CpuClock,
    interrupt::software::SoftwareInterruptControl,
    timer::timg::TimerGroup,
};
use hello_ble_common::{PERIPHERAL_ADDRESS, PERIPHERAL_NAME};
use esp_radio::ble::controller::BleConnector;
use rtt_target::{rprintln, rtt_init_print};
use trouble_host::prelude::ExternalController;

/// 最大并发连接数 / Maximum concurrent BLE connections.
const CONNECTIONS_MAX: usize = 1;
/// L2CAP 通道数 / L2CAP channel count.
const L2CAP_CHANNELS_MAX: usize = 2;

/// 全局 panic 处理 / Global panic handler — 通过 RTT 输出后死循环。
#[panic_handler]
fn panic(panic_info: &core::panic::PanicInfo) -> ! {
    rprintln!("{}", panic_info);
    loop {}
}

// 向 ESP-IDF bootloader 注册应用描述信息 / Register app description with ESP-IDF bootloader.
esp_bootloader_esp_idf::esp_app_desc!();

/// 程序入口 / Application entry point.
///
/// 由 `esp_rtos` 调度，负责初始化硬件并启动 BLE 外设循环。
/// Scheduled by `esp_rtos`, initializes hardware and starts the BLE peripheral loop.
#[esp_rtos::main]
async fn main(_s: Spawner) {
    rtt_init_print!();

    // === 1. 初始化 ESP32 外设 / Init ESP32 peripherals ===
    rprintln!("[init] {} peripheral starting...", PERIPHERAL_NAME);
    let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));

    // === 2. 配置 RTOS 调度器 / Configure RTOS scheduler ===
    rprintln!("[init] configuring timer and software interrupt...");
    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_int = SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_int.software_interrupt0);

    // === 3. 创建 BLE controller / Create BLE controller ===
    rprintln!("[init] initializing BLE controller...");
    let bluetooth = peripherals.BT;
    let connector = BleConnector::new(bluetooth, Default::default()).unwrap();
    let controller: ExternalController<_, 1> = ExternalController::new(connector);

    // === 4. 启动 BLE 外设运行时 / Start BLE peripheral runtime ===
    rprintln!("[init] starting BLE peripheral...");
    let config = easyble::Config {
        address: PERIPHERAL_ADDRESS,
        on_fatal: &|_msg| {
            // 致命错误时通过软件复位恢复
            // Recover from fatal errors via software reset
            esp_hal::system::software_reset();
        },
    };
    let mut app = AppState::new();
    easyble::run::<_, AppState, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX>(controller, config, &mut app).await;
}
