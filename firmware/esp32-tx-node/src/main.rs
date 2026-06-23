#![no_main]

use core::sync::atomic::{AtomicBool, Ordering};

use esp_idf_svc::espnow::{EspNow, PeerInfo};
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::delay::FreeRtos;
use esp_idf_svc::hal::gpio::{PinDriver, Pull};
use esp_idf_svc::hal::io::Read;
use esp_idf_svc::hal::uart::UartDriver;
use esp_idf_svc::log::EspLogger;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::sys;
use esp_idf_svc::wifi::{ClientConfiguration, Configuration, EspWifi};
use heapless::String;
use protocol::frame::{Payload, TelemetryFrame};
use protocol::encode_espnow;

const GATEWAY_MAC: &str = env!("GATEWAY_MAC", "FF:FF:FF:FF:FF:FF");
const NODE_ID: u8 = {
    match option_env!("NODE_ID") {
        Some(s) => parse_node_id(s),
        None => 1,
    }
};
const DEBOUNCE_MS: u32 = 50;

static SEQ: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
static UART_TRIGGER: AtomicBool = AtomicBool::new(false);
static UART_RELEASE: AtomicBool = AtomicBool::new(false);

const fn parse_node_id(s: &str) -> u8 {
    let bytes = s.as_bytes();
    if bytes.is_empty() {
        return 1;
    }
    let mut value = 0u8;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b < b'0' || b > b'9' {
            break;
        }
        value = value.saturating_mul(10).saturating_add(b - b'0');
        i += 1;
    }
    value
}

fn parse_mac(s: &str) -> [u8; 6] {
    let mut mac = [0xFFu8; 6];
    let mut idx = 0usize;
    let mut part = 0u8;
    let mut nibble = 0u8;
    let mut has_nibble = false;

    for byte in s.bytes() {
        let v = match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            b'A'..=b'F' => byte - b'A' + 10,
            b':' | b'-' if has_nibble => {
                part = (part << 4) | nibble;
                has_nibble = false;
                if idx < 6 {
                    mac[idx] = part;
                    idx += 1;
                    part = 0;
                }
                continue;
            }
            _ => continue,
        };
        nibble = v;
        has_nibble = true;
    }

    if has_nibble && idx < 6 {
        part = (part << 4) | nibble;
        mac[idx] = part;
    }

    mac
}

fn now_ms() -> u64 {
    unsafe { (sys::esp_timer_get_time() / 1_000) as u64 }
}

fn send_bool(esp_now: &EspNow<'_>, gateway_mac: [u8; 6], value: bool) {
    let seq = SEQ.fetch_add(1, Ordering::Relaxed) + 1;
    let frame = TelemetryFrame {
        seq,
        timestamp_ms: now_ms(),
        node_id: NODE_ID,
        payload: Payload::BoolCmd(value),
    };

    let Ok(packet) = encode_espnow(&frame) else {
        log::error!("encode_espnow failed");
        return;
    };

    match esp_now.send(gateway_mac, &packet) {
        Ok(()) => log::info!("ESP-NOW sent node={} seq={} value={}", NODE_ID, seq, value),
        Err(e) => log::error!("ESP-NOW send failed: {:?}", e),
    }
}

fn poll_uart(uart: &mut UartDriver<'_>, line: &mut String<64>) {
    let mut byte = [0u8; 1];
    while uart.read(&mut byte).unwrap_or(0) > 0 {
        let ch = byte[0];
        if ch == b'\n' || ch == b'\r' {
            if line.as_str().trim() == "TRIGGER" {
                UART_TRIGGER.store(true, Ordering::Relaxed);
            } else if line.as_str().trim() == "RELEASE" {
                UART_RELEASE.store(true, Ordering::Relaxed);
            }
            line.clear();
        } else if line.push(ch as char).is_err() {
            line.clear();
        }
    }
}

#[unsafe(no_mangle)]
fn main() -> ! {
    EspLogger::initialize_default();
    log::info!("ESP32 TX Node starting (node_id={})", NODE_ID);

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
    let gateway_mac = parse_mac(GATEWAY_MAC);

    let mut peer = PeerInfo::default();
    peer.peer_addr = gateway_mac;
    peer.encrypt = false;
    esp_now.add_peer(peer).unwrap();
    log::info!(
        "ESP-NOW ready, gateway={:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        gateway_mac[0],
        gateway_mac[1],
        gateway_mac[2],
        gateway_mac[3],
        gateway_mac[4],
        gateway_mac[5]
    );

    let mut button = PinDriver::input(peripherals.pins.gpio0, Pull::Up).unwrap();
    let mut uart = UartDriver::new(
        peripherals.uart0,
        peripherals.pins.gpio1,
        peripherals.pins.gpio3,
        Option::<esp_idf_svc::hal::gpio::AnyIOPin>::None,
        Option::<esp_idf_svc::hal::gpio::AnyIOPin>::None,
        &esp_idf_svc::hal::uart::config::Config::default(),
    )
    .unwrap();

    let mut line = String::<64>::new();
    let mut last_button_ms = 0u64;
    let mut last_level = true;

    loop {
        poll_uart(&mut uart, &mut line);

        if UART_TRIGGER.swap(false, Ordering::Relaxed) {
            log::info!("UART TRIGGER received");
            send_bool(&esp_now, gateway_mac, true);
        }
        if UART_RELEASE.swap(false, Ordering::Relaxed) {
            send_bool(&esp_now, gateway_mac, false);
        }

        let pressed = button.is_low();
        if pressed && !last_level {
            let now = now_ms();
            if now.saturating_sub(last_button_ms) >= DEBOUNCE_MS as u64 {
                last_button_ms = now;
                send_bool(&esp_now, gateway_mac, true);
            }
        }
        last_level = pressed;
        FreeRtos::delay_ms(10);
    }
}
