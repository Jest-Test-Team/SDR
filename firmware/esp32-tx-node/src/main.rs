#![no_std]
#![no_main]

use esp_idf_svc::hal::prelude::*;
use esp_idf_svc::log::EspLogger;

#[no_mangle]
fn main() -> ! {
    EspLogger::initialize_default();
    log::info!("ESP32 TX Node starting...");

    let peripherals = esp_idf_svc::hal::peripherals::Peripherals::take().unwrap();
    let _timer = esp_idf_svc::hal::timer::TimerDriver::new(peripherals.timer00).unwrap();

    log::info!("Initialization complete. Entering main loop.");
    loop {
        esp_idf_svc::hal::delay::FreeRtos::delay_ms(1000);
    }
}