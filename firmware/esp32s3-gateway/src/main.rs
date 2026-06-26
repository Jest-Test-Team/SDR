#![no_main]

//! ESP32-S3 **software-sim node** role (Secure Telemetry Gateway topology).
//!
//! ```text
//! Mac (control-plane) --USB(this board)-- ESP32-S3 [software-sim sender/receiver]
//!                                              ^ ESP-NOW
//!                                          ESP32 [gateway]
//! ```
//!
//! The S3 is the server-facing endpoint. Over native USB it accepts text
//! commands from the Mac and turns them into ESP-NOW traffic to the ESP32
//! gateway, then prints the gateway's replies back over USB:
//!
//! - `GW,HEALTH`              -> gateway free heap / uptime / Wi-Fi mode
//! - `GW,TOGGLE`              -> toggle downstream AP on the gateway
//! - `GW,DEAUTH`              -> kick downstream stations
//! - `GW,STALIST`             -> connected station count
//! - `GW,SNMP_SET,<oid>,<val>`-> write a simulated OID
//! - `GW,SNMP_GET,<oid>`      -> read a simulated OID
//! - `SIM,SEND,<0|1>`         -> send a TelemetryFrame (gateway echoes it back)
//!
//! Gateway replies arrive over ESP-NOW and are printed as `GWRESP ...` /
//! `SIMRECV ...` lines for the host CLI (`scripts/sim_node.py`).

use std::cell::RefCell;
use std::sync::Mutex;

use esp_idf_svc::espnow::{EspNow, PeerInfo, ReceiveInfo};
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::delay::FreeRtos;
use esp_idf_svc::hal::io::Write;
use esp_idf_svc::hal::usb_serial::{UsbSerialConfig, UsbSerialDriver};
use esp_idf_svc::log::EspLogger;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::sys::{
    esp_wifi_set_channel, wifi_second_chan_t_WIFI_SECOND_CHAN_NONE,
};
use esp_idf_svc::wifi::{ClientConfiguration, Configuration, EspWifi};
use heapless::Deque;
use protocol::frame::{Payload, TelemetryFrame};
use protocol::gwlink::{GwMsg, decode_gw_espnow, encode_gw_espnow};
use protocol::{decode_espnow, encode_espnow};

const GATEWAY_MAC: &str = env!("GATEWAY_MAC");
const ESPNOW_CHANNEL: u8 = 1;
const MAX_PENDING: usize = 8;

static SEQ: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

type PktBuf = heapless::Vec<u8, { protocol::MAX_ESP_NOW_PAYLOAD }>;

/// Inbound ESP-NOW payloads (gateway replies / telemetry echoes), drained in
/// the main loop where the USB serial driver lives.
static RX_INBOX: Mutex<RefCell<Deque<PktBuf, MAX_PENDING>>> =
    Mutex::new(RefCell::new(Deque::new()));

fn parse_mac(s: &str) -> [u8; 6] {
    let mut mac = [0u8; 6];
    let mut idx = 0usize;
    let mut hi: Option<u8> = None;
    for byte in s.bytes() {
        let v = match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            b'A'..=b'F' => byte - b'A' + 10,
            _ => continue,
        };
        if let Some(high) = hi {
            if idx < 6 {
                mac[idx] = (high << 4) | v;
                idx += 1;
            }
            hi = None;
        } else {
            hi = Some(v);
        }
    }
    mac
}

fn lock_wifi_channel(channel: u8) {
    esp_idf_svc::sys::esp!(unsafe {
        esp_wifi_set_channel(channel, wifi_second_chan_t_WIFI_SECOND_CHAN_NONE)
    })
    .expect("esp_wifi_set_channel");
}

fn enqueue_rx(data: &[u8]) {
    let Ok(cell) = RX_INBOX.lock() else {
        return;
    };
    let mut buf = PktBuf::new();
    if buf.extend_from_slice(data).is_err() {
        return;
    }
    let _ = cell.borrow_mut().push_back(buf);
}

fn send_gw(esp_now: &EspNow<'_>, gateway_mac: [u8; 6], msg: &GwMsg) {
    match encode_gw_espnow(msg) {
        Ok(bytes) => {
            if let Err(e) = esp_now.send(gateway_mac, &bytes) {
                log::warn!("ESP-NOW gw send failed: {:?}", e);
            }
        }
        Err(e) => log::error!("encode_gw_espnow failed: {:?}", e),
    }
}

fn send_sim(esp_now: &EspNow<'_>, gateway_mac: [u8; 6], value: bool) {
    let seq = SEQ.fetch_add(1, core::sync::atomic::Ordering::Relaxed) + 1;
    let frame = TelemetryFrame {
        seq,
        timestamp_ms: 0,
        node_id: 1,
        payload: Payload::BoolCmd(value),
    };
    if let Ok(bytes) = encode_espnow(&frame) {
        if let Err(e) = esp_now.send(gateway_mac, &bytes) {
            log::warn!("ESP-NOW sim send failed: {:?}", e);
        }
    }
}

fn write_line(serial: &mut UsbSerialDriver<'_>, line: &str) {
    let _ = serial.write_all(line.as_bytes());
    let _ = serial.write_all(b"\n");
    let _ = serial.flush();
}

/// Translate one command line from the Mac into ESP-NOW traffic.
fn handle_command(
    serial: &mut UsbSerialDriver<'_>,
    esp_now: &EspNow<'_>,
    gateway_mac: [u8; 6],
    line: &str,
) {
    let mut fields = line.trim().split(',');
    match (fields.next(), fields.next()) {
        (Some("GW"), Some("HEALTH")) => send_gw(esp_now, gateway_mac, &GwMsg::HealthReq),
        (Some("GW"), Some("TOGGLE")) => send_gw(esp_now, gateway_mac, &GwMsg::ToggleReq),
        (Some("GW"), Some("DEAUTH")) => send_gw(esp_now, gateway_mac, &GwMsg::DeauthReq),
        (Some("GW"), Some("STALIST")) => send_gw(esp_now, gateway_mac, &GwMsg::StaListReq),
        (Some("GW"), Some("SNMP_SET")) => {
            let oid = fields.next().unwrap_or("");
            let value = fields.next().unwrap_or("");
            send_gw(
                esp_now,
                gateway_mac,
                &GwMsg::SnmpSetReq {
                    oid: oid.to_string(),
                    value: value.to_string(),
                },
            );
        }
        (Some("GW"), Some("SNMP_GET")) => {
            let oid = fields.next().unwrap_or("");
            send_gw(
                esp_now,
                gateway_mac,
                &GwMsg::SnmpGetReq {
                    oid: oid.to_string(),
                },
            );
        }
        (Some("GW"), Some("ENROLL")) => {
            let device_id = fields.next().unwrap_or("").to_string();
            let mac = fields.next().unwrap_or("").to_string();
            send_gw(esp_now, gateway_mac, &GwMsg::EnrollReq { device_id, mac });
        }
        (Some("GW"), Some("CLAIM")) => {
            let device_id = fields.next().unwrap_or("").to_string();
            send_gw(esp_now, gateway_mac, &GwMsg::ClaimReq { device_id });
        }
        (Some("GW"), Some("ROTATE")) => {
            let device_id = fields.next().unwrap_or("").to_string();
            send_gw(esp_now, gateway_mac, &GwMsg::RotateReq { device_id });
        }
        (Some("GW"), Some("REVOKE")) => {
            let device_id = fields.next().unwrap_or("").to_string();
            send_gw(esp_now, gateway_mac, &GwMsg::RevokeReq { device_id });
        }
        (Some("SIM"), Some("SEND")) => {
            let value = matches!(fields.next(), Some("1") | Some("true"));
            send_sim(esp_now, gateway_mac, value);
            write_line(serial, "SIMSENT ok");
        }
        _ => write_line(serial, &format!("ERR unknown command: {}", line.trim())),
    }
}

/// Format an inbound ESP-NOW payload as a USB line for the host.
fn format_inbound(data: &[u8]) -> String {
    if let Ok(frame) = decode_espnow(data) {
        return format!(
            "SIMRECV node={} seq={} payload={:?}",
            frame.node_id, frame.seq, frame.payload
        );
    }
    match decode_gw_espnow(data) {
        Ok(GwMsg::HealthResp {
            free_heap,
            uptime_ms,
            rx_count,
            wifi_mode,
        }) => format!(
            "GWRESP HEALTH free_heap={} uptime_ms={} rx_count={} wifi_mode={:?}",
            free_heap, uptime_ms, rx_count, wifi_mode
        ),
        Ok(GwMsg::ToggleResp {
            downstream_online,
            wifi_mode,
        }) => format!(
            "GWRESP TOGGLE downstream_online={} wifi_mode={:?}",
            downstream_online, wifi_mode
        ),
        Ok(GwMsg::DeauthResp { kicked }) => format!("GWRESP DEAUTH kicked={}", kicked),
        Ok(GwMsg::StaListResp { count }) => format!("GWRESP STALIST count={}", count),
        Ok(GwMsg::SnmpResp { oid, value, ok }) => format!(
            "GWRESP SNMP oid={} value={} ok={}",
            oid,
            value.as_deref().unwrap_or("<none>"),
            ok
        ),
        Ok(GwMsg::ProvisionResp {
            device_id,
            state,
            fingerprint,
            version,
            ok,
        }) => format!(
            "GWRESP PROVISION device_id={} state={} fingerprint={} version={} ok={}",
            device_id,
            state,
            if fingerprint.is_empty() { "<none>" } else { &fingerprint },
            version,
            ok
        ),
        Ok(other) => format!("GWRESP other={:?}", other),
        Err(e) => format!("ERR undecodable inbound: {}", e),
    }
}

fn read_commands(
    serial: &mut UsbSerialDriver<'_>,
    esp_now: &EspNow<'_>,
    gateway_mac: [u8; 6],
    rx_buf: &mut [u8; 128],
    line: &mut heapless::Vec<u8, 160>,
) {
    match serial.read(rx_buf, 0) {
        Ok(0) | Err(_) => {}
        Ok(n) => {
            for &byte in &rx_buf[..n] {
                if byte == b'\n' || byte == b'\r' {
                    if !line.is_empty() {
                        if let Ok(text) = core::str::from_utf8(line.as_slice()) {
                            let owned = text.to_string();
                            handle_command(serial, esp_now, gateway_mac, &owned);
                        }
                        line.clear();
                    }
                } else if line.push(byte).is_err() {
                    line.clear();
                }
            }
        }
    }
}

#[unsafe(no_mangle)]
fn main() -> ! {
    EspLogger::initialize_default();
    log::info!("ESP32-S3 software-sim node starting");

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

    let esp_now = EspNow::take().unwrap();
    lock_wifi_channel(ESPNOW_CHANNEL);
    let gateway_mac = parse_mac(GATEWAY_MAC);
    let mut peer = PeerInfo::default();
    peer.peer_addr = gateway_mac;
    peer.channel = ESPNOW_CHANNEL;
    peer.encrypt = false;
    match esp_now.add_peer(peer) {
        Ok(()) => log::info!(
            "ESP-NOW ready, gateway={:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            gateway_mac[0],
            gateway_mac[1],
            gateway_mac[2],
            gateway_mac[3],
            gateway_mac[4],
            gateway_mac[5]
        ),
        Err(e) => log::error!("add_peer failed: {:?}", e),
    }
    esp_now
        .register_recv_cb(|_info: &ReceiveInfo, data: &[u8]| {
            enqueue_rx(data);
        })
        .unwrap();

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

    write_line(&mut usb_serial, "READY software-sim node");
    log::info!("software-sim node ready (USB command interface)");

    let mut rx_buf = [0u8; 128];
    let mut line = heapless::Vec::<u8, 160>::new();

    loop {
        read_commands(&mut usb_serial, &esp_now, gateway_mac, &mut rx_buf, &mut line);

        // Drain inbound ESP-NOW replies to USB.
        loop {
            let next = {
                let Ok(cell) = RX_INBOX.lock() else {
                    break;
                };
                let item = cell.borrow_mut().pop_front();
                item
            };
            let Some(buf) = next else {
                break;
            };
            let out = format_inbound(&buf);
            write_line(&mut usb_serial, &out);
        }

        FreeRtos::delay_ms(5);
    }
}
