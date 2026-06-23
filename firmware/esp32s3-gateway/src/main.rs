#![no_main]

use core::cell::RefCell;
use std::sync::Mutex;

use esp_idf_svc::espnow::{EspNow, ReceiveInfo};
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::delay::FreeRtos;
use esp_idf_svc::hal::io::Write;
use esp_idf_svc::hal::units::Hertz;
use esp_idf_svc::hal::uart::UartDriver;
use esp_idf_svc::log::EspLogger;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::{ClientConfiguration, Configuration, EspWifi};
use heapless::Deque;
use protocol::{decode_espnow, encode_frame, ESP_NOW_VENDOR_ID};

const UART_BAUD: u32 = 921_600;
const MAX_PENDING: usize = 8;
const MAX_FRAME: usize = 256;

type FrameBuf = heapless::Vec<u8, MAX_FRAME>;

static PENDING: Mutex<RefCell<Deque<FrameBuf, MAX_PENDING>>> =
    Mutex::new(RefCell::new(Deque::new()));

fn enqueue_uart_frame(data: &[u8]) {
    let Ok(mut cell) = PENDING.lock() else {
        return;
    };
    let mut frame = FrameBuf::new();
    if frame.extend_from_slice(data).is_err() {
        log::warn!("UART frame too large, dropping");
        return;
    }
    if frame.push(0).is_err() {
        log::warn!("UART delimiter failed, dropping");
        return;
    }
    let _ = cell.borrow_mut().push_back(frame);
}

fn drain_to_uart(uart: &mut UartDriver<'_>) {
    loop {
        let pending: Option<FrameBuf> = {
            let Ok(cell) = PENDING.lock() else {
                return;
            };
            cell.borrow_mut().pop_front()
        };
        let Some(uart_frame) = pending else {
            break;
        };
        if let Err(e) = uart.write_all(&uart_frame) {
            log::error!("UART write failed: {:?}", e);
        }
    }
}

#[unsafe(no_mangle)]
fn main() -> ! {
    EspLogger::initialize_default();
    log::info!("ESP32-S3 Gateway starting");

    let peripherals = esp_idf_svc::hal::peripherals::Peripherals::take().unwrap();
    let sys_loop = EspSystemEventLoop::take().unwrap();
    let nvs = EspDefaultNvsPartition::take().unwrap();

    let mut wifi = EspWifi::new(peripherals.modem, sys_loop, Some(nvs)).unwrap();
    wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: "".try_into().unwrap(),
        ..Default::default()
    }))
    .unwrap();
    wifi.start().unwrap();

    let esp_now = EspNow::take().unwrap();
    esp_now
        .register_recv_cb(|info: &ReceiveInfo, data: &[u8]| {
            if data.first() != Some(&ESP_NOW_VENDOR_ID) {
                return;
            }
            match decode_espnow(data) {
                Ok(frame) => {
                    log::info!(
                        "ESP-NOW RX from {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X} node={} seq={}",
                        info.src_addr[0],
                        info.src_addr[1],
                        info.src_addr[2],
                        info.src_addr[3],
                        info.src_addr[4],
                        info.src_addr[5],
                        frame.node_id,
                        frame.seq
                    );
                    match encode_frame(&frame) {
                        Ok(uart_frame) => enqueue_uart_frame(&uart_frame),
                        Err(_) => log::error!("encode_frame failed"),
                    }
                }
                Err(e) => log::warn!("decode_espnow failed: {}", e),
            }
        })
        .unwrap();

    let uart_config = esp_idf_svc::hal::uart::config::Config::default().baudrate(Hertz(
        UART_BAUD,
    ));
    let mut uart = UartDriver::new(
        peripherals.uart0,
        peripherals.pins.gpio43,
        peripherals.pins.gpio44,
        Option::<esp_idf_svc::hal::gpio::AnyIOPin>::None,
        Option::<esp_idf_svc::hal::gpio::AnyIOPin>::None,
        &uart_config,
    )
    .unwrap();

    log::info!("Gateway ready (UART {} baud)", UART_BAUD);

    loop {
        drain_to_uart(&mut uart);
        FreeRtos::delay_ms(5);
    }
}
