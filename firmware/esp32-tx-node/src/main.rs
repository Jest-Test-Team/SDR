#![no_main]

//! ESP32 **gateway** role (Secure Telemetry Gateway topology).
//!
//! ```text
//! Mac --USB-- ESP32-S3 [software-sim node] --ESP-NOW(this link)-- ESP32 [gateway]
//! ```
//!
//! The gateway listens for [`GwMsg`] control requests over ESP-NOW from the S3
//! node and executes them on real hardware:
//! - `HealthReq`  -> real `esp_get_free_heap_size()`, uptime, RX count, Wi-Fi mode
//! - `ToggleReq`  -> bring the downstream AP up/down (`Configuration::Mixed` <-> `Client`)
//! - `DeauthReq`  -> `esp_wifi_deauth_sta(0)` (kick all downstream stations)
//! - `StaListReq` -> count of connected stations
//! - `SnmpSetReq`/`SnmpGetReq` -> a small simulated MIB held in RAM
//!
//! It also echoes any telemetry [`TelemetryFrame`] back to the sender so the S3
//! "receiver" role can observe a round trip.
//!
//! NOTE: This board was previously the TX node. Reconfiguring Wi-Fi while
//! ESP-NOW is active is the riskiest part; the channel is re-locked after every
//! reconfiguration to keep the ESP-NOW control link alive.

mod espnow_setup;
mod mac;

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Mutex;

use esp_idf_svc::espnow::{EspNow, PeerInfo, ReceiveInfo};
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::delay::FreeRtos;
use esp_idf_svc::log::EspLogger;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::sys;
use esp_idf_svc::wifi::{
    AccessPointConfiguration, AuthMethod, ClientConfiguration, Configuration, EspWifi,
};
use heapless::Deque;
use protocol::frame::{Payload, TelemetryFrame};
use protocol::gwlink::{GwMsg, WifiMode, decode_gw_espnow, encode_gw_espnow};
use protocol::{decode_espnow, encode_espnow};

use crate::espnow_setup::{ESPNOW_CHANNEL, disable_wifi_power_save, lock_wifi_channel};

const AP_SSID: &str = "GATEWAY_ISOLATED_NET";
const AP_PASSWORD: &str = "sdrgateway";
const MAX_PENDING: usize = 8;

static RX_COUNT: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

type PktBuf = heapless::Vec<u8, { protocol::MAX_ESP_NOW_PAYLOAD }>;

/// Inbox of (source MAC, raw ESP-NOW payload) filled by the recv callback and
/// drained in the main loop (which owns `EspNow` + `EspWifi`).
static INBOX: Mutex<RefCell<Deque<([u8; 6], PktBuf), MAX_PENDING>>> =
    Mutex::new(RefCell::new(Deque::new()));

fn now_ms() -> u64 {
    unsafe { (sys::esp_timer_get_time() / 1_000) as u64 }
}

fn free_heap() -> u32 {
    unsafe { sys::esp_get_free_heap_size() }
}

fn enqueue(src: [u8; 6], data: &[u8]) {
    let Ok(cell) = INBOX.lock() else {
        return;
    };
    let mut buf = PktBuf::new();
    if buf.extend_from_slice(data).is_err() {
        log::warn!("inbox packet too large, dropping");
        return;
    }
    let _ = cell.borrow_mut().push_back((src, buf));
}

fn ensure_peer(esp_now: &EspNow<'_>, mac: [u8; 6]) {
    if esp_now.peer_exists(mac).unwrap_or(false) {
        return;
    }
    let mut peer = PeerInfo::default();
    peer.peer_addr = mac;
    peer.channel = ESPNOW_CHANNEL;
    peer.encrypt = false;
    if let Err(e) = esp_now.add_peer(peer) {
        log::warn!("add_peer failed: {:?}", e);
    }
}

fn reply(esp_now: &EspNow<'_>, dst: [u8; 6], msg: &GwMsg) {
    match encode_gw_espnow(msg) {
        Ok(bytes) => {
            ensure_peer(esp_now, dst);
            if let Err(e) = esp_now.send(dst, &bytes) {
                log::warn!("reply send failed: {:?}", e);
            }
        }
        Err(e) => log::error!("encode_gw_espnow failed: {:?}", e),
    }
}

/// On-device provisioning record for a downstream device identity.
struct DeviceRec {
    #[allow(dead_code)]
    mac: String,
    state: &'static str,
    fingerprint: String,
    version: u32,
}

struct Gateway {
    downstream_online: bool,
    oids: HashMap<String, String>,
    devices: HashMap<String, DeviceRec>,
    credential_counter: u32,
}

impl Gateway {
    fn new() -> Self {
        let mut oids = HashMap::new();
        oids.insert("1.3.6.1.4.1.custom.isolate".to_string(), "false".to_string());
        oids.insert("1.3.6.1.4.1.custom.relay".to_string(), "off".to_string());
        Self {
            downstream_online: true,
            oids,
            devices: HashMap::new(),
            credential_counter: 0,
        }
    }

    /// Issue a deterministic credential fingerprint (FNV-1a over device id +
    /// issuance counter). Stand-in for real key/cert material; matches the
    /// host-side `GatewaySim` scheme so on-device and sim outputs are comparable.
    fn issue_credential(&mut self, device_id: &str) -> String {
        self.credential_counter += 1;
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in device_id.bytes().chain(self.credential_counter.to_le_bytes()) {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
        format!("cred-{hash:016x}")
    }

    /// Build a `ProvisionResp` for `device_id` from current registry state.
    fn provision_resp(&self, device_id: &str, ok: bool) -> GwMsg {
        match self.devices.get(device_id) {
            Some(rec) => GwMsg::ProvisionResp {
                device_id: device_id.to_string(),
                state: rec.state.to_string(),
                fingerprint: rec.fingerprint.clone(),
                version: rec.version,
                ok,
            },
            None => GwMsg::ProvisionResp {
                device_id: device_id.to_string(),
                state: "unknown".to_string(),
                fingerprint: String::new(),
                version: 0,
                ok: false,
            },
        }
    }

    fn wifi_mode(&self) -> WifiMode {
        if self.downstream_online {
            WifiMode::ApSta
        } else {
            WifiMode::Sta
        }
    }

    /// Apply the current downstream state to the Wi-Fi driver, then re-lock the
    /// ESP-NOW channel so the control link survives the reconfiguration.
    fn apply_wifi(&self, wifi: &mut EspWifi<'_>) {
        let sta = ClientConfiguration {
            ssid: "".try_into().unwrap(),
            channel: Some(ESPNOW_CHANNEL),
            ..Default::default()
        };
        let result = if self.downstream_online {
            let ap = AccessPointConfiguration {
                ssid: AP_SSID.try_into().unwrap(),
                password: AP_PASSWORD.try_into().unwrap(),
                auth_method: AuthMethod::WPA2Personal,
                channel: ESPNOW_CHANNEL,
                ..Default::default()
            };
            wifi.set_configuration(&Configuration::Mixed(sta, ap))
        } else {
            wifi.set_configuration(&Configuration::Client(sta))
        };
        if let Err(e) = result {
            log::error!("set_configuration failed: {:?}", e);
        }
        lock_wifi_channel(ESPNOW_CHANNEL);
        log::info!("wifi mode -> {:?}", self.wifi_mode());
    }

    fn handle(&mut self, esp_now: &EspNow<'_>, wifi: &mut EspWifi<'_>, src: [u8; 6], data: &[u8]) {
        // Telemetry frame? echo it back so the S3 "receiver" sees a round trip.
        if let Ok(frame) = decode_espnow(data) {
            log::info!("telemetry node={} seq={} -> echo", frame.node_id, frame.seq);
            let echo = TelemetryFrame {
                seq: frame.seq,
                timestamp_ms: now_ms(),
                node_id: frame.node_id,
                payload: Payload::ByteCmd(0xAC),
            };
            if let Ok(bytes) = encode_espnow(&echo) {
                ensure_peer(esp_now, src);
                let _ = esp_now.send(src, &bytes);
            }
            return;
        }

        let msg = match decode_gw_espnow(data) {
            Ok(m) => m,
            Err(e) => {
                log::warn!("undecodable ESP-NOW packet: {}", e);
                return;
            }
        };

        let response = match msg {
            GwMsg::HealthReq => GwMsg::HealthResp {
                free_heap: free_heap(),
                uptime_ms: now_ms(),
                rx_count: RX_COUNT.load(core::sync::atomic::Ordering::Relaxed),
                wifi_mode: self.wifi_mode(),
            },
            GwMsg::ToggleReq => {
                self.downstream_online = !self.downstream_online;
                self.apply_wifi(wifi);
                GwMsg::ToggleResp {
                    downstream_online: self.downstream_online,
                    wifi_mode: self.wifi_mode(),
                }
            }
            GwMsg::DeauthReq => {
                let rc = unsafe { sys::esp_wifi_deauth_sta(0) };
                log::info!("esp_wifi_deauth_sta(0) -> {}", rc);
                GwMsg::DeauthResp {
                    kicked: if rc == 0 { 1 } else { 0 },
                }
            }
            GwMsg::StaListReq => GwMsg::StaListResp {
                count: self.sta_count(),
            },
            GwMsg::SnmpSetReq { oid, value } => {
                if self.downstream_online {
                    self.oids.insert(oid.clone(), value.clone());
                    GwMsg::SnmpResp {
                        oid,
                        value: Some(value),
                        ok: true,
                    }
                } else {
                    GwMsg::SnmpResp {
                        oid,
                        value: None,
                        ok: false,
                    }
                }
            }
            GwMsg::SnmpGetReq { oid } => {
                let value = if self.downstream_online {
                    self.oids.get(&oid).cloned()
                } else {
                    None
                };
                let ok = value.is_some();
                GwMsg::SnmpResp { oid, value, ok }
            }
            GwMsg::EnrollReq { device_id, mac } => {
                if self.devices.contains_key(&device_id) {
                    self.provision_resp(&device_id, false)
                } else {
                    let fingerprint = self.issue_credential(&device_id);
                    self.devices.insert(
                        device_id.clone(),
                        DeviceRec {
                            mac,
                            state: "pending",
                            fingerprint,
                            version: 1,
                        },
                    );
                    self.provision_resp(&device_id, true)
                }
            }
            GwMsg::ClaimReq { device_id } => {
                let ok = match self.devices.get_mut(&device_id) {
                    Some(rec) if rec.state == "pending" => {
                        rec.state = "active";
                        true
                    }
                    _ => false,
                };
                self.provision_resp(&device_id, ok)
            }
            GwMsg::RotateReq { device_id } => {
                let allowed = matches!(
                    self.devices.get(&device_id),
                    Some(rec) if rec.state != "revoked"
                );
                let ok = if allowed {
                    let fingerprint = self.issue_credential(&device_id);
                    if let Some(rec) = self.devices.get_mut(&device_id) {
                        rec.fingerprint = fingerprint;
                        rec.version += 1;
                    }
                    true
                } else {
                    false
                };
                self.provision_resp(&device_id, ok)
            }
            GwMsg::RevokeReq { device_id } => {
                let ok = match self.devices.get_mut(&device_id) {
                    Some(rec) if rec.state != "revoked" => {
                        rec.state = "revoked";
                        true
                    }
                    _ => false,
                };
                self.provision_resp(&device_id, ok)
            }
            // Responses are not expected inbound on the gateway.
            other => {
                log::warn!("unexpected inbound message: {:?}", other);
                return;
            }
        };

        reply(esp_now, src, &response);
    }

    fn sta_count(&self) -> u32 {
        if !self.downstream_online {
            return 0;
        }
        let mut list: sys::wifi_sta_list_t = unsafe { core::mem::zeroed() };
        let rc = unsafe { sys::esp_wifi_ap_get_sta_list(&mut list) };
        if rc == 0 {
            list.num as u32
        } else {
            0
        }
    }
}

#[unsafe(no_mangle)]
fn main() -> ! {
    EspLogger::initialize_default();
    log::info!("ESP32 Gateway starting (Secure Telemetry Gateway role)");

    let peripherals = esp_idf_svc::hal::peripherals::Peripherals::take().unwrap();
    let sys_loop = EspSystemEventLoop::take().unwrap();
    let nvs = EspDefaultNvsPartition::take().unwrap();

    let mut wifi = EspWifi::new(peripherals.modem, sys_loop, Some(nvs)).unwrap();
    let mut gateway = Gateway::new();
    // Start in AP-STA so the downstream network is up by default.
    wifi.set_configuration(&Configuration::Mixed(
        ClientConfiguration {
            ssid: "".try_into().unwrap(),
            channel: Some(ESPNOW_CHANNEL),
            ..Default::default()
        },
        AccessPointConfiguration {
            ssid: AP_SSID.try_into().unwrap(),
            password: AP_PASSWORD.try_into().unwrap(),
            auth_method: AuthMethod::WPA2Personal,
            channel: ESPNOW_CHANNEL,
            ..Default::default()
        },
    ))
    .unwrap();
    wifi.start().unwrap();
    lock_wifi_channel(ESPNOW_CHANNEL);
    disable_wifi_power_save();
    log::info!("AP '{}' up on channel {}", AP_SSID, ESPNOW_CHANNEL);

    let esp_now = EspNow::take().unwrap();
    lock_wifi_channel(ESPNOW_CHANNEL);
    esp_now
        .register_recv_cb(|info: &ReceiveInfo, data: &[u8]| {
            RX_COUNT.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
            enqueue(*info.src_addr, data);
        })
        .unwrap();

    log::info!("Gateway ready, awaiting ESP-NOW control requests from S3 node");

    loop {
        let next = {
            let Ok(cell) = INBOX.lock() else {
                FreeRtos::delay_ms(10);
                continue;
            };
            let item = cell.borrow_mut().pop_front();
            item
        };
        if let Some((src, buf)) = next {
            gateway.handle(&esp_now, &mut wifi, src, &buf);
        }
        FreeRtos::delay_ms(5);
    }
}
