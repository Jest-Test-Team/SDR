#![no_main]

mod espnow_setup;
mod mac;

use esp_idf_svc::espnow::EspNow;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::delay::FreeRtos;
use esp_idf_svc::hal::gpio::{PinDriver, Pull};
use esp_idf_svc::log::EspLogger;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::sys;
use esp_idf_svc::wifi::{ClientConfiguration, Configuration, EspWifi};
use protocol::encode_espnow;
use protocol::frame::{Payload, TelemetryFrame};

use crate::espnow_setup::{
    ESPNOW_CHANNEL, add_gateway_peer, disable_wifi_power_save, lock_wifi_channel,
    set_max_tx_power_dbm,
};
use crate::mac::parse_mac;

const GATEWAY_MAC: &str = env!("GATEWAY_MAC");
const NODE_ID: u8 = {
    const ID_STR: &str = env!("NODE_ID");
    parse_node_id(ID_STR)
};
const DEBOUNCE_MS: u32 = 50;
const HEARTBEAT_MS: u64 = 2_000;
const TX_POWER_DBM: Option<&str> = option_env!("TX_POWER_DBM");

static SEQ: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
static BOOT_PAYLOAD_BYTE: core::sync::atomic::AtomicU8 = core::sync::atomic::AtomicU8::new(0xB2);

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

fn send_payload(esp_now: &EspNow<'_>, gateway_mac: [u8; 6], payload: Payload) {
    let seq = SEQ.fetch_add(1, core::sync::atomic::Ordering::Relaxed) + 1;
    let frame = TelemetryFrame {
        seq,
        timestamp_ms: now_ms(),
        node_id: NODE_ID,
        payload,
    };

    let Ok(packet) = encode_espnow(&frame) else {
        log::error!("encode_espnow failed");
        return;
    };

    match esp_now.send(gateway_mac, &packet) {
        Ok(()) => log::info!(
            "ESP-NOW sent node={} seq={} payload={:?}",
            NODE_ID,
            seq,
            frame.payload
        ),
        Err(e) => log::error!("ESP-NOW send failed: {:?}", e),
    }
}

fn send_bool(esp_now: &EspNow<'_>, gateway_mac: [u8; 6], value: bool) {
    send_payload(esp_now, gateway_mac, Payload::BoolCmd(value));
}

fn send_boot_payload(esp_now: &EspNow<'_>, gateway_mac: [u8; 6]) {
    let value = BOOT_PAYLOAD_BYTE.load(core::sync::atomic::Ordering::Relaxed);
    send_payload(esp_now, gateway_mac, Payload::ByteCmd(value));
}

fn apply_configured_tx_power() {
    let Some(raw) = TX_POWER_DBM else {
        log::info!("TX_POWER_DBM not set; using ESP-IDF default Wi-Fi TX power");
        return;
    };
    match raw.parse::<i8>() {
        Ok(dbm) => set_max_tx_power_dbm(dbm),
        Err(_) => log::warn!("invalid TX_POWER_DBM='{}'; using ESP-IDF default", raw),
    }
}

fn parse_i8(bytes: &[u8]) -> Option<i8> {
    core::str::from_utf8(bytes).ok()?.parse().ok()
}

fn parse_u8(bytes: &[u8]) -> Option<u8> {
    core::str::from_utf8(bytes).ok()?.parse().ok()
}

fn apply_runtime_control(data: &[u8]) {
    if !data.starts_with(b"SDRCTL,") {
        return;
    }
    let mut fields = data.split(|b| *b == b',');
    let _tag = fields.next();
    let Some(node) = fields.next().and_then(parse_u8) else {
        log::warn!("invalid firmware control node field");
        return;
    };
    if node != 0 && node != NODE_ID {
        return;
    }
    if let Some(tx_power) = fields.next().and_then(parse_i8) {
        if tx_power != i8::MIN {
            set_max_tx_power_dbm(tx_power);
        }
    }
    if let Some(boot_byte) = fields.next().and_then(parse_u8) {
        BOOT_PAYLOAD_BYTE.store(boot_byte, core::sync::atomic::Ordering::Relaxed);
        log::info!("BOOT payload byte set to 0x{:02X}", boot_byte);
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
    lock_wifi_channel(ESPNOW_CHANNEL);
    disable_wifi_power_save();
    apply_configured_tx_power();

    let esp_now = EspNow::take().unwrap();
    lock_wifi_channel(ESPNOW_CHANNEL);
    let gateway_mac = parse_mac(GATEWAY_MAC);
    match add_gateway_peer(&esp_now, gateway_mac) {
        Ok(()) => log::info!(
            "ESP-NOW ready, gateway={:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X} ch={}",
            gateway_mac[0],
            gateway_mac[1],
            gateway_mac[2],
            gateway_mac[3],
            gateway_mac[4],
            gateway_mac[5],
            ESPNOW_CHANNEL
        ),
        Err(e) => log::error!("add_peer failed: {:?}", e),
    }
    esp_now
        .register_recv_cb(|_info, data| {
            apply_runtime_control(data);
        })
        .unwrap();

    // BOOT button on GPIO0. Do not open UartDriver on UART0 — that port is the console.
    let button = PinDriver::input(peripherals.pins.gpio0, Pull::Up).unwrap();
    log::info!("main loop started (BOOT=GPIO0 trigger)");

    let mut last_button_ms = 0u64;
    let mut button_down = false;
    let mut trigger_sent = false;
    let mut last_heartbeat_ms = now_ms();

    loop {
        let pressed = button.is_low();
        if pressed {
            if !button_down {
                button_down = true;
                last_button_ms = now_ms();
            } else if !trigger_sent && now_ms().saturating_sub(last_button_ms) >= DEBOUNCE_MS as u64
            {
                trigger_sent = true;
                log::info!("BOOT pressed, sending ESP-NOW");
                send_boot_payload(&esp_now, gateway_mac);
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
