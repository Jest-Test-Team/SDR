//! Gateway backend: routes dashboard gateway commands to either the real ESP32
//! boards (over the S3 USB serial link) or the in-memory [`GatewaySim`].
//!
//! Hardware mode is selected at startup when a serial port to the ESP32-S3
//! software-sim node can be opened; otherwise the dashboard transparently falls
//! back to simulation. The dashboard reflects which is active via
//! [`GatewayStatus::mode`].

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::Serialize;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{Mutex, Notify, RwLock, broadcast};
use tokio::time::timeout;

use crate::sim::gateway::{
    DeviceIdentity, GatewayCommand, GatewayResponse, GatewaySim, GatewaySnapshot, OidEntry,
    ProvisioningState, SnmpResponse, WifiMode,
};

const HEAP_TOTAL_BYTES: u32 = 327_680;
const CMD_TIMEOUT: Duration = Duration::from_millis(1_500);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GatewayMode {
    Hardware,
    Simulation,
}

#[derive(Debug, Clone, Serialize)]
pub struct GatewayStatus {
    pub mode: GatewayMode,
    pub connected: bool,
    pub port: Option<String>,
}

/// Backend used by the API layer.
pub enum GatewayBackend {
    Simulation(RwLock<GatewaySim>),
    Hardware(HwGateway),
}

impl GatewayBackend {
    pub fn simulation() -> Self {
        GatewayBackend::Simulation(RwLock::new(GatewaySim::new()))
    }

    /// Try to open the serial link to the S3 node. Returns a simulation backend
    /// if the port cannot be opened.
    pub fn connect(port: &str, baud: u32) -> Self {
        match HwGateway::open(port, baud) {
            Ok(hw) => {
                tracing::info!("gateway hardware backend connected on {port}");
                GatewayBackend::Hardware(hw)
            }
            Err(e) => {
                tracing::warn!("gateway serial {port} unavailable ({e}); using simulation");
                GatewayBackend::simulation()
            }
        }
    }

    pub fn status(&self) -> GatewayStatus {
        match self {
            GatewayBackend::Simulation(_) => GatewayStatus {
                mode: GatewayMode::Simulation,
                connected: false,
                port: None,
            },
            GatewayBackend::Hardware(hw) => GatewayStatus {
                mode: GatewayMode::Hardware,
                connected: hw.connected.load(Ordering::Relaxed),
                port: Some(hw.port.clone()),
            },
        }
    }

    pub async fn snapshot(&self) -> GatewaySnapshot {
        match self {
            GatewayBackend::Simulation(sim) => sim.read().await.snapshot(),
            GatewayBackend::Hardware(hw) => hw.snapshot.read().await.clone(),
        }
    }

    pub async fn apply(&self, command: &GatewayCommand) -> GatewayResponse {
        match self {
            GatewayBackend::Simulation(sim) => sim.write().await.apply(command),
            GatewayBackend::Hardware(hw) => hw.apply(command).await,
        }
    }

    /// Subscribe to the live monitor stream (raw lines from the boards, or sim
    /// command echoes).
    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        match self {
            GatewayBackend::Simulation(_) => {
                // A short-lived channel; simulation has no async line source, so
                // callers just get the initial snapshot over the socket.
                let (tx, rx) = broadcast::channel(8);
                drop(tx);
                rx
            }
            GatewayBackend::Hardware(hw) => hw.monitor_tx.subscribe(),
        }
    }
}

pub struct HwGateway {
    port: String,
    writer: Arc<Mutex<tokio::io::WriteHalf<tokio_serial::SerialStream>>>,
    snapshot: Arc<RwLock<GatewaySnapshot>>,
    last_line: Arc<RwLock<String>>,
    monitor_tx: broadcast::Sender<String>,
    notify: Arc<Notify>,
    connected: Arc<AtomicBool>,
}

impl HwGateway {
    fn open(port: &str, baud: u32) -> Result<Self, std::io::Error> {
        let stream = tokio_serial::SerialStream::open(&tokio_serial::new(port, baud))?;
        let (read_half, write_half) = tokio::io::split(stream);

        let snapshot = Arc::new(RwLock::new(default_hw_snapshot()));
        let last_line = Arc::new(RwLock::new(String::new()));
        let (monitor_tx, _) = broadcast::channel(128);
        let notify = Arc::new(Notify::new());
        let connected = Arc::new(AtomicBool::new(true));

        // Background reader: parse lines into the snapshot and broadcast them.
        let r_snap = snapshot.clone();
        let r_last = last_line.clone();
        let r_tx = monitor_tx.clone();
        let r_notify = notify.clone();
        let r_connected = connected.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(read_half).lines();
            loop {
                match lines.next_line().await {
                    Ok(Some(line)) => {
                        let trimmed = line.trim().to_string();
                        if trimmed.is_empty() {
                            continue;
                        }
                        apply_line(&r_snap, &trimmed).await;
                        *r_last.write().await = trimmed.clone();
                        let _ = r_tx.send(trimmed);
                        r_notify.notify_waiters();
                    }
                    Ok(None) | Err(_) => {
                        r_connected.store(false, Ordering::Relaxed);
                        tracing::warn!("gateway serial reader ended");
                        break;
                    }
                }
            }
        });

        Ok(Self {
            port: port.to_string(),
            writer: Arc::new(Mutex::new(write_half)),
            snapshot,
            last_line,
            monitor_tx,
            notify,
            connected,
        })
    }

    async fn send_line(&self, line: &str) {
        let mut w = self.writer.lock().await;
        let payload = format!("{line}\n");
        if let Err(e) = w.write_all(payload.as_bytes()).await {
            tracing::warn!("gateway serial write failed: {e}");
            self.connected.store(false, Ordering::Relaxed);
        } else {
            let _ = w.flush().await;
        }
    }

    async fn apply(&self, command: &GatewayCommand) -> GatewayResponse {
        let name = command.name().to_string();
        let line = command_to_line(command);

        let (ok, message) = match line {
            Some(line) => {
                let notified = self.notify.notified();
                self.send_line(&line).await;
                // Wait for the reader to ingest a reply.
                let _ = timeout(CMD_TIMEOUT, notified).await;
                let last = self.last_line.read().await.clone();
                let ok = !last.starts_with("ERR");
                (ok, last)
            }
            None => (
                false,
                format!("{name} is simulation-only; not supported on hardware"),
            ),
        };

        let snapshot = self.snapshot.read().await.clone();
        let snmp = extract_snmp(command, &snapshot);
        GatewayResponse {
            ok,
            command: name,
            message,
            snmp,
            snapshot,
        }
    }
}

fn default_hw_snapshot() -> GatewaySnapshot {
    GatewaySnapshot {
        wifi_mode: WifiMode::ApSta,
        downstream_online: true,
        free_heap_bytes: 0,
        heap_total_bytes: HEAP_TOTAL_BYTES,
        command_count: 0,
        oids: Vec::new(),
        nodes: Vec::new(),
        devices: Vec::new(),
        command_log: Vec::new(),
    }
}

/// Map a dashboard command to the S3 node's serial command line.
fn command_to_line(command: &GatewayCommand) -> Option<String> {
    match command {
        GatewayCommand::NetToggleDownstream => Some("GW,TOGGLE".to_string()),
        GatewayCommand::SnmpSet { oid, value } => Some(format!("GW,SNMP_SET,{oid},{value}")),
        GatewayCommand::SnmpGet { oid } => Some(format!("GW,SNMP_GET,{oid}")),
        GatewayCommand::DeauthSta { .. } => Some("GW,DEAUTH".to_string()),
        GatewayCommand::SysHealth => Some("GW,HEALTH".to_string()),
        // Provisioning is a real on-device operation: the ESP32 gateway keeps the
        // device registry and replies with `GWRESP PROVISION ...`.
        GatewayCommand::EnrollDevice { device_id, mac } => {
            Some(format!("GW,ENROLL,{device_id},{mac}"))
        }
        GatewayCommand::ClaimDevice { device_id } => Some(format!("GW,CLAIM,{device_id}")),
        GatewayCommand::RotateCredential { device_id } => Some(format!("GW,ROTATE,{device_id}")),
        GatewayCommand::RevokeDevice { device_id } => Some(format!("GW,REVOKE,{device_id}")),
        // No on-device equivalent: registration is a pure simulation concept.
        GatewayCommand::RegisterNode { .. } => None,
    }
}

fn extract_snmp(command: &GatewayCommand, snap: &GatewaySnapshot) -> Option<SnmpResponse> {
    let oid = match command {
        GatewayCommand::SnmpSet { oid, .. } | GatewayCommand::SnmpGet { oid } => oid,
        _ => return None,
    };
    let value = snap.oids.iter().find(|e| &e.oid == oid).map(|e| e.value.clone());
    Some(SnmpResponse {
        protocol: "sim_snmp_v3".to_string(),
        operation: match command {
            GatewayCommand::SnmpSet { .. } => "set",
            _ => "get",
        }
        .to_string(),
        oid: oid.clone(),
        ok: value.is_some(),
        value,
        message: "hardware".to_string(),
    })
}

/// Parse a serial line from the boards into the live snapshot.
async fn apply_line(snap: &Arc<RwLock<GatewaySnapshot>>, line: &str) {
    let mut s = snap.write().await;
    s.command_count = s.command_count.saturating_add(1);
    s.command_log.insert(0, line.to_string());
    s.command_log.truncate(20);

    let mut tokens = line.split_whitespace();
    let tag = tokens.next().unwrap_or("");
    if tag == "GWRESP" {
        let sub = tokens.next().unwrap_or("");
        let kv = parse_kv(tokens);
        match sub {
            "HEALTH" => {
                if let Some(v) = kv_u32(&kv, "free_heap") {
                    s.free_heap_bytes = v;
                }
                if let Some(mode) = kv.get("wifi_mode") {
                    set_wifi_mode(&mut s, mode);
                }
            }
            "TOGGLE" => {
                if let Some(b) = kv.get("downstream_online") {
                    s.downstream_online = b == "true";
                }
                if let Some(mode) = kv.get("wifi_mode") {
                    set_wifi_mode(&mut s, mode);
                }
            }
            "SNMP" => {
                if let (Some(oid), Some(value)) = (kv.get("oid"), kv.get("value")) {
                    if value != "<none>" {
                        upsert_oid(&mut s, oid, value);
                    }
                }
            }
            "PROVISION" => {
                if let Some(device_id) = kv.get("device_id") {
                    upsert_device(&mut s, device_id, &kv);
                }
            }
            _ => {}
        }
    }
}

fn set_wifi_mode(s: &mut GatewaySnapshot, mode: &str) {
    match mode {
        "ApSta" | "ap_sta" => {
            s.wifi_mode = WifiMode::ApSta;
            s.downstream_online = true;
        }
        "Sta" | "sta" => {
            s.wifi_mode = WifiMode::Sta;
            s.downstream_online = false;
        }
        _ => {}
    }
}

fn upsert_oid(s: &mut GatewaySnapshot, oid: &str, value: &str) {
    if let Some(entry) = s.oids.iter_mut().find(|e| e.oid == oid) {
        entry.value = value.to_string();
    } else {
        s.oids.push(OidEntry {
            oid: oid.to_string(),
            value: value.to_string(),
        });
    }
}

fn upsert_device(
    s: &mut GatewaySnapshot,
    device_id: &str,
    kv: &std::collections::HashMap<String, String>,
) {
    let state = match kv.get("state").map(String::as_str) {
        Some("active") => ProvisioningState::Active,
        Some("revoked") => ProvisioningState::Revoked,
        Some("pending") => ProvisioningState::Pending,
        // "unknown" (or anything else) is not a real identity — don't record it,
        // so a rejected op (e.g. claim of a non-existent device) leaves the table
        // unchanged.
        _ => return,
    };
    let fingerprint = kv
        .get("fingerprint")
        .filter(|f| f.as_str() != "<none>")
        .cloned()
        .unwrap_or_default();
    let version = kv_u32(kv, "version").unwrap_or(0);
    if let Some(dev) = s.devices.iter_mut().find(|d| d.device_id == device_id) {
        dev.state = state;
        if !fingerprint.is_empty() {
            dev.credential_fingerprint = fingerprint;
        }
        dev.credential_version = version;
    } else {
        s.devices.push(DeviceIdentity {
            device_id: device_id.to_string(),
            mac: kv.get("mac").cloned().unwrap_or_default(),
            state,
            credential_fingerprint: fingerprint,
            credential_version: version,
        });
    }
}

fn parse_kv<'a>(
    tokens: impl Iterator<Item = &'a str>,
) -> std::collections::HashMap<String, String> {
    tokens
        .filter_map(|t| t.split_once('='))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn kv_u32(kv: &std::collections::HashMap<String, String>, key: &str) -> Option<u32> {
    kv.get(key).and_then(|v| v.parse().ok())
}
