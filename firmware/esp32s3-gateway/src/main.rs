#![no_main]

use core::cell::RefCell;
use std::sync::Mutex;

use esp_idf_svc::espnow::{EspNow, PeerInfo, ReceiveInfo};
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::delay::FreeRtos;
use esp_idf_svc::hal::io::{Read, Write};
use esp_idf_svc::hal::usb_serial::{UsbSerialConfig, UsbSerialDriver};
use esp_idf_svc::log::EspLogger;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::{ClientConfiguration, Configuration, EspWifi};
use heapless::Deque;
use protocol::{ESP_NOW_VENDOR_ID, decode_espnow, encode_frame};

const MAX_PENDING: usize = 8;
const MAX_FRAME: usize = 256;
const ESPNOW_CHANNEL: u8 = 1;
const BROADCAST_MAC: [u8; 6] = [0xFF; 6];

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

fn add_broadcast_peer(esp_now: &EspNow<'_>) -> Result<(), esp_idf_svc::sys::EspError> {
    if esp_now.peer_exists(BROADCAST_MAC).unwrap_or(false) {
        esp_now.del_peer(BROADCAST_MAC)?;
    }
    let mut peer = PeerInfo::default();
    peer.peer_addr = BROADCAST_MAC;
    peer.channel = ESPNOW_CHANNEL;
    peer.encrypt = false;
    esp_now.add_peer(peer)
}

fn drain_control_from_usb(
    serial: &mut UsbSerialDriver<'_>,
    esp_now: &EspNow<'_>,
    rx_buf: &mut [u8; 128],
    line: &mut heapless::Vec<u8, 128>,
) {
    match serial.read(rx_buf) {
        Ok(0) => {}
        Ok(n) => {
            for &byte in &rx_buf[..n] {
                if byte == b'\n' {
                    if line.starts_with(b"SDRCTL,") {
                        match esp_now.send(BROADCAST_MAC, line.as_slice()) {
                            Ok(()) => log::info!(
                                "forwarded firmware control over ESP-NOW: {}",
                                core::str::from_utf8(line.as_slice()).unwrap_or("<binary>")
                            ),
                            Err(e) => log::warn!("firmware control ESP-NOW send failed: {:?}", e),
                        }
                    }
                    line.clear();
                } else if line.push(byte).is_err() {
                    log::warn!("firmware control line too long, dropping");
                    line.clear();
                }
            }
        }
        Err(e) => {
            log::warn!("USB serial read failed: {:?}", e);
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
    match add_broadcast_peer(&esp_now) {
        Ok(()) => log::info!("ESP-NOW broadcast control peer ready"),
        Err(e) => log::warn!("add broadcast peer failed: {:?}", e),
    }
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
    let mut control_rx_buf = [0u8; 128];
    let mut control_line = heapless::Vec::<u8, 128>::new();

    loop {
        drain_to_usb(&mut usb_serial);
        drain_control_from_usb(
            &mut usb_serial,
            &esp_now,
            &mut control_rx_buf,
            &mut control_line,
        );
        FreeRtos::delay_ms(5);
    }
}
