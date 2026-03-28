//! BLE Peripheral — runs on ESP32C6

#![no_std]
#![no_main]

mod ble_bas_peripheral;
mod services;

use embassy_executor::Spawner;
use esp_alloc as _;
use esp_hal::{
    clock::CpuClock,
    interrupt::software::SoftwareInterruptControl,
    timer::timg::TimerGroup,
};
use hello_ble_common::PERIPHERAL_NAME;
use esp_radio::ble::controller::BleConnector;
use rtt_target::{rprintln, rtt_init_print};
use trouble_host::prelude::ExternalController;

#[panic_handler]
fn panic(panic_info: &core::panic::PanicInfo) -> ! {
    rprintln!("{}", panic_info);
    loop {}
}

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(_s: Spawner) {
    rtt_init_print!();
    // === 核心逻辑 ===
    // 1. 初始化 ESP32 外设
    rprintln!("[init] {} peripheral starting...", PERIPHERAL_NAME);
    let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));

    // 2. 配置 RTOS 调度器（embassy 需要 timer + software interrupt）
    rprintln!("[init] configuring timer and software interrupt...");
    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_int = SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_int.software_interrupt0);

    // 3. 创建 BLE controller（芯片级 BLE 硬件）
    rprintln!("[init] initializing BLE controller...");
    let bluetooth = peripherals.BT;
    let connector = BleConnector::new(bluetooth, Default::default()).unwrap();
    let controller: ExternalController<_, 1> = ExternalController::new(connector);

    // 4. 启动 BLE 外设（广告 + GATT 服务，进入主循环）
    rprintln!("[init] starting BLE peripheral...");
    ble_bas_peripheral::run(controller).await;
}
