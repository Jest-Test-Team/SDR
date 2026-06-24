#![no_main]

mod espnow_setup;
mod mac;

use core::sync::atomic::{AtomicBool, Ordering};

use esp_idf_svc::espnow::{EspNow, BROADCAST};
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

use crate::espnow_setup::{add_gateway_peer, disable_wifi_power_save, ESPNOW_CHANNEL};
use crate::mac::parse_mac;

const GATEWAY_MAC: &str = env!("GATEWAY_MAC");
const NODE_ID: u8 = {
    const ID_STR: &str = env!("NODE_ID");
    parse_node_id(ID_STR)
};
const DEBOUNCE_MS: u32 = 50;
const HEARTBEAT_MS: u64 = 5_000;

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

    for dest in [gateway_mac, BROADCAST] {
        match esp_now.send(dest, &packet) {
            Ok(()) => log::info!(
                "ESP-NOW sent node={} seq={} value={} dst={:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                NODE_ID,
                seq,
                value,
                dest[0],
                dest[1],
                dest[2],
                dest[3],
                dest[4],
                dest[5]
            ),
            Err(e) => log::error!("ESP-NOW send failed: {:?}", e),
        }
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
        channel: Some(ESPNOW_CHANNEL),
        ..Default::default()
    }))
    .unwrap();
    wifi.start().unwrap();
    disable_wifi_power_save();

    let esp_now = EspNow::take().unwrap();
    let gateway_mac = parse_mac(GATEWAY_MAC);
    log::info!("GATEWAY_MAC={} ch={}", GATEWAY_MAC, ESPNOW_CHANNEL);
    add_gateway_peer(&esp_now, gateway_mac);
    log::info!(
        "ESP-NOW ready, gateway={:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        gateway_mac[0],
        gateway_mac[1],
        gateway_mac[2],
        gateway_mac[3],
        gateway_mac[4],
        gateway_mac[5]
    );

    let button = PinDriver::input(peripherals.pins.gpio0, Pull::Up).unwrap();
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
    let mut button_down = false;
    let mut trigger_sent = false;
    let mut last_heartbeat_ms = now_ms();

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
        if pressed {
            if !button_down {
                button_down = true;
                last_button_ms = now_ms();
            } else if !trigger_sent && now_ms().saturating_sub(last_button_ms) >= DEBOUNCE_MS as u64 {
                trigger_sent = true;
                log::info!("BOOT pressed, sending ESP-NOW");
                send_bool(&esp_now, gateway_mac, true);
            }
        } else {
            button_down = false;
            trigger_sent = false;
        }

        let now = now_ms();
        if now.saturating_sub(last_heartbeat_ms) >= HEARTBEAT_MS {
            last_heartbeat_ms = now;
            log::info!("heartbeat");
            send_bool(&esp_now, gateway_mac, false);
        }

        FreeRtos::delay_ms(10);
    }
}
