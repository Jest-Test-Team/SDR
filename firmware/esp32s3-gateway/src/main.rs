#![no_main]

use core::cell::RefCell;
use std::sync::Mutex;

use esp_idf_svc::espnow::{EspNow, ReceiveInfo};
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::delay::FreeRtos;
use esp_idf_svc::hal::io::Write;
use esp_idf_svc::hal::usb_serial::{UsbSerialConfig, UsbSerialDriver};
use esp_idf_svc::log::EspLogger;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::{ClientConfiguration, Configuration, EspWifi};
use heapless::Deque;
use protocol::{ESP_NOW_VENDOR_ID, decode_espnow, encode_frame};

const MAX_PENDING: usize = 8;
const MAX_FRAME: usize = 256;
const ESPNOW_CHANNEL: u8 = 1;

fn lock_wifi_channel(channel: u8) {
    use esp_idf_svc::sys::{esp_wifi_set_channel, wifi_second_chan_t_WIFI_SECOND_CHAN_NONE};
    esp_idf_svc::sys::esp!(unsafe {
        esp_wifi_set_channel(channel, wifi_second_chan_t_WIFI_SECOND_CHAN_NONE)
    })
    .expect("esp_wifi_set_channel");
}

static USB_TX_COUNT: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

type FrameBuf = heapless::Vec<u8, MAX_FRAME>;

static PENDING: Mutex<RefCell<Deque<FrameBuf, MAX_PENDING>>> =
    Mutex::new(RefCell::new(Deque::new()));

fn enqueue_uart_frame(data: &[u8]) {
    let Ok(cell) = PENDING.lock() else {
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

fn drain_to_usb(serial: &mut UsbSerialDriver<'_>) {
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
        if let Err(e) = serial.write_all(&uart_frame) {
            log::error!("USB serial write failed: {:?}", e);
        } else {
            let n = USB_TX_COUNT.fetch_add(1, core::sync::atomic::Ordering::Relaxed) + 1;
            log::info!("USB TX frame #{} ({} bytes)", n, uart_frame.len());
            let _ = serial.flush();
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
        channel: Some(ESPNOW_CHANNEL),
        ..Default::default()
    }))
    .unwrap();
    wifi.start().unwrap();
    lock_wifi_channel(ESPNOW_CHANNEL);
    log::info!("WiFi locked to channel {}", ESPNOW_CHANNEL);

    let esp_now = EspNow::take().unwrap();
    lock_wifi_channel(ESPNOW_CHANNEL);
    esp_now
        .register_recv_cb(|info: &ReceiveInfo, data: &[u8]| {
            log::info!(
                "ESP-NOW raw {} bytes from {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                data.len(),
                info.src_addr[0],
                info.src_addr[1],
                info.src_addr[2],
                info.src_addr[3],
                info.src_addr[4],
                info.src_addr[5]
            );
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

    // Telemetry to PC via native USB (shows as /dev/cu.usbmodem* on macOS).
    let usb_config = UsbSerialConfig::new()
        .tx_buffer_size(1024)
        .rx_buffer_size(256);
    let mut usb_serial = UsbSerialDriver::new(
        peripherals.usb_serial,
        peripherals.pins.gpio19,
        peripherals.pins.gpio20,
        &usb_config,
    )
    .unwrap();

    log::info!("Gateway ready (USB serial bridge to PC)");

    loop {
        drain_to_usb(&mut usb_serial);
        FreeRtos::delay_ms(5);
    }
}
