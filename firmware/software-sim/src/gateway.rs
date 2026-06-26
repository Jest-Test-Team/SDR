//! Secure Telemetry Gateway simulation.
//!
//! Models the ESP32-S3 gateway (AP-STA mode) described in the implementation
//! guide: it routes commands from the Mac orchestrator down to a downstream
//! ESP32 endpoint, can sever/restore the isolated downstream Wi-Fi network,
//! speaks a simulated SNMP protocol (JSON instead of ASN.1/BER), exposes gateway
//! system health, and can deauthenticate (kick) connected stations by MAC.
//!
//! The logic is hardware-free so it can be reused by the `hil-simulator`
//! sidecar and exercised by unit / Robot Framework tests, while the on-device
//! firmware (`esp32s3-gateway`) maps the same commands onto `esp-idf-svc` calls.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Wire command names shared with the on-device firmware.
pub const CMD_NET_TOGGLE_DOWNSTREAM: &str = "CMD_NET_TOGGLE_DOWNSTREAM";
pub const CMD_SNMP_SET: &str = "CMD_SNMP_SET";
pub const CMD_SNMP_GET: &str = "CMD_SNMP_GET";
pub const CMD_DEAUTH_STA: &str = "CMD_DEAUTH_STA";
pub const CMD_SYS_HEALTH: &str = "CMD_SYS_HEALTH";
pub const CMD_STA_LIST: &str = "CMD_STA_LIST";
pub const CMD_REGISTER_NODE: &str = "CMD_REGISTER_NODE";
pub const CMD_ENROLL_DEVICE: &str = "CMD_ENROLL_DEVICE";
pub const CMD_CLAIM_DEVICE: &str = "CMD_CLAIM_DEVICE";
pub const CMD_ROTATE_CREDENTIAL: &str = "CMD_ROTATE_CREDENTIAL";
pub const CMD_REVOKE_DEVICE: &str = "CMD_REVOKE_DEVICE";

/// Simulated SNMP protocol label (matches the JSON payload in the guide).
pub const SIM_SNMP_PROTOCOL: &str = "sim_snmp_v3";

const HEAP_TOTAL_BYTES: u32 = 327_680;
const HEAP_FLOOR_BYTES: u32 = 40_960;
const HEAP_PER_COMMAND: u32 = 1_280;
const MAX_LOG: usize = 20;

/// Wi-Fi mode of the gateway. `ApSta` keeps both the upstream link to the Mac
/// and the downstream isolated AP; `Sta` drops the AP (downstream severed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WifiMode {
    ApSta,
    Sta,
}

/// Lifecycle state of a provisioned device identity.
///
/// `Pending`  -> enrolled, credential issued, not yet activated.
/// `Active`   -> claimed and onboarded as a downstream node.
/// `Revoked`  -> terminal; credential no longer trusted, node offboarded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvisioningState {
    Pending,
    Active,
    Revoked,
}

/// A provisioned device identity managed by the gateway's provisioning registry.
///
/// The `credential_fingerprint` is a deterministic stand-in for a real
/// key/certificate fingerprint (no real crypto is performed); it changes every
/// time the credential is issued or rotated, and `credential_version` counts how
/// many times a credential has been issued for this device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceIdentity {
    pub device_id: String,
    pub mac: String,
    pub state: ProvisioningState,
    pub credential_fingerprint: String,
    pub credential_version: u32,
}

/// A station connected to the gateway's isolated downstream network.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeInfo {
    pub mac: String,
    pub ip: String,
    pub free_heap_bytes: u32,
    pub online: bool,
}

/// Simulated SNMP request (JSON over a lightweight transport).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnmpRequest {
    pub protocol: String,
    pub operation: String,
    pub oid: String,
    #[serde(default)]
    pub value: Option<String>,
}

/// Simulated SNMP response forwarded back to the Mac.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnmpResponse {
    pub protocol: String,
    pub operation: String,
    pub oid: String,
    pub value: Option<String>,
    pub ok: bool,
    pub message: String,
}

/// Commands the Mac orchestrator can issue to the gateway.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum GatewayCommand {
    /// CMD_NET_TOGGLE_DOWNSTREAM: sever/restore the downstream AP.
    NetToggleDownstream,
    /// CMD_SNMP_SET: write a simulated OID on the downstream endpoint.
    SnmpSet { oid: String, value: String },
    /// CMD_SNMP_GET: read a simulated OID from the downstream endpoint.
    SnmpGet { oid: String },
    /// CMD_DEAUTH_STA: forcefully kick a connected station by MAC.
    DeauthSta { mac: String },
    /// CMD_SYS_HEALTH: report gateway free heap / uptime / link state.
    SysHealth,
    /// CMD_STA_LIST: report the number of connected downstream stations.
    StaList,
    /// CMD_REGISTER_NODE: simulate a downstream endpoint joining the AP.
    RegisterNode { mac: String, ip: String },
    /// CMD_ENROLL_DEVICE: create a device identity and issue its first credential.
    EnrollDevice { device_id: String, mac: String },
    /// CMD_CLAIM_DEVICE: activate an enrolled device and onboard it as a node.
    ClaimDevice { device_id: String },
    /// CMD_ROTATE_CREDENTIAL: reissue the credential for an active device.
    RotateCredential { device_id: String },
    /// CMD_REVOKE_DEVICE: revoke a device identity and offboard its node.
    RevokeDevice { device_id: String },
}

impl GatewayCommand {
    pub fn name(&self) -> &'static str {
        match self {
            GatewayCommand::NetToggleDownstream => CMD_NET_TOGGLE_DOWNSTREAM,
            GatewayCommand::SnmpSet { .. } => CMD_SNMP_SET,
            GatewayCommand::SnmpGet { .. } => CMD_SNMP_GET,
            GatewayCommand::DeauthSta { .. } => CMD_DEAUTH_STA,
            GatewayCommand::SysHealth => CMD_SYS_HEALTH,
            GatewayCommand::StaList => CMD_STA_LIST,
            GatewayCommand::RegisterNode { .. } => CMD_REGISTER_NODE,
            GatewayCommand::EnrollDevice { .. } => CMD_ENROLL_DEVICE,
            GatewayCommand::ClaimDevice { .. } => CMD_CLAIM_DEVICE,
            GatewayCommand::RotateCredential { .. } => CMD_ROTATE_CREDENTIAL,
            GatewayCommand::RevokeDevice { .. } => CMD_REVOKE_DEVICE,
        }
    }
}

/// Result of applying a command, suitable for returning to the dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayResponse {
    pub ok: bool,
    pub command: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snmp: Option<SnmpResponse>,
    pub snapshot: GatewaySnapshot,
}

/// Point-in-time view of the gateway, published to the dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewaySnapshot {
    pub wifi_mode: WifiMode,
    pub downstream_online: bool,
    pub free_heap_bytes: u32,
    pub heap_total_bytes: u32,
    pub station_count: u32,
    pub command_count: u32,
    pub oids: Vec<OidEntry>,
    pub nodes: Vec<NodeInfo>,
    pub devices: Vec<DeviceIdentity>,
    pub command_log: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OidEntry {
    pub oid: String,
    pub value: String,
}

/// Hardware-free model of the ESP32-S3 gateway.
#[derive(Debug, Clone)]
pub struct GatewaySim {
    downstream_online: bool,
    free_heap_bytes: u32,
    command_count: u32,
    oids: BTreeMap<String, String>,
    nodes: Vec<NodeInfo>,
    devices: Vec<DeviceIdentity>,
    credential_counter: u32,
    log: Vec<String>,
}

impl Default for GatewaySim {
    fn default() -> Self {
        let mut oids = BTreeMap::new();
        // Default simulated MIB matching the guide's example OID.
        oids.insert("1.3.6.1.4.1.custom.isolate".to_string(), "false".to_string());
        oids.insert("1.3.6.1.4.1.custom.relay".to_string(), "off".to_string());
        Self {
            downstream_online: true,
            free_heap_bytes: HEAP_TOTAL_BYTES - HEAP_PER_COMMAND,
            command_count: 0,
            oids,
            nodes: vec![NodeInfo {
                mac: "24:6F:28:00:00:01".to_string(),
                ip: "192.168.4.2".to_string(),
                free_heap_bytes: 180_224,
                online: true,
            }],
            devices: Vec::new(),
            credential_counter: 0,
            log: Vec::new(),
        }
    }
}

impl GatewaySim {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn wifi_mode(&self) -> WifiMode {
        if self.downstream_online {
            WifiMode::ApSta
        } else {
            WifiMode::Sta
        }
    }

    fn record(&mut self, line: String) {
        self.command_count += 1;
        // Each handled command consumes a little heap, floored to stay realistic.
        self.free_heap_bytes = self
            .free_heap_bytes
            .saturating_sub(HEAP_PER_COMMAND)
            .max(HEAP_FLOOR_BYTES);
        self.log.insert(0, line);
        self.log.truncate(MAX_LOG);
    }

    pub fn snapshot(&self) -> GatewaySnapshot {
        GatewaySnapshot {
            wifi_mode: self.wifi_mode(),
            downstream_online: self.downstream_online,
            free_heap_bytes: self.free_heap_bytes,
            heap_total_bytes: HEAP_TOTAL_BYTES,
            station_count: self.online_station_count(),
            command_count: self.command_count,
            oids: self
                .oids
                .iter()
                .map(|(oid, value)| OidEntry {
                    oid: oid.clone(),
                    value: value.clone(),
                })
                .collect(),
            nodes: self.nodes.clone(),
            devices: self.devices.clone(),
            command_log: self.log.clone(),
        }
    }

    /// Issue a fresh deterministic credential fingerprint. This is a stand-in for
    /// real key/certificate material: it is reproducible from the device id and an
    /// issuance counter, and changes on every issuance/rotation.
    fn issue_credential(&mut self, device_id: &str) -> String {
        self.credential_counter += 1;
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325; // FNV-1a offset basis
        for byte in device_id.bytes().chain(self.credential_counter.to_le_bytes()) {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
        format!("cred-{hash:016x}")
    }

    fn device_index(&self, device_id: &str) -> Option<usize> {
        self.devices.iter().position(|d| d.device_id == device_id)
    }

    /// Number of connected downstream stations (mirrors `GW,STALIST` on hardware).
    fn online_station_count(&self) -> u32 {
        if !self.downstream_online {
            return 0;
        }
        self.nodes.iter().filter(|n| n.online).count() as u32
    }

    /// Apply a command and return the response plus updated snapshot.
    pub fn apply(&mut self, command: &GatewayCommand) -> GatewayResponse {
        let name = command.name().to_string();
        let (ok, message, snmp) = match command {
            GatewayCommand::NetToggleDownstream => {
                self.downstream_online = !self.downstream_online;
                for node in &mut self.nodes {
                    node.online = self.downstream_online;
                }
                let msg = if self.downstream_online {
                    "downstream AP restored (WifiMode::ApSta)".to_string()
                } else {
                    "downstream AP severed (WifiMode::Sta)".to_string()
                };
                (true, msg, None)
            }
            GatewayCommand::SnmpSet { oid, value } => {
                if !self.downstream_online {
                    let resp = self.snmp_error(CMD_SNMP_SET, oid, "downstream offline");
                    (false, "downstream offline".to_string(), Some(resp))
                } else {
                    self.oids.insert(oid.clone(), value.clone());
                    let resp = SnmpResponse {
                        protocol: SIM_SNMP_PROTOCOL.to_string(),
                        operation: "set".to_string(),
                        oid: oid.clone(),
                        value: Some(value.clone()),
                        ok: true,
                        message: "ack".to_string(),
                    };
                    (true, format!("set {oid} = {value}"), Some(resp))
                }
            }
            GatewayCommand::SnmpGet { oid } => {
                if !self.downstream_online {
                    let resp = self.snmp_error(CMD_SNMP_GET, oid, "downstream offline");
                    (false, "downstream offline".to_string(), Some(resp))
                } else {
                    match self.oids.get(oid) {
                        Some(value) => {
                            let resp = SnmpResponse {
                                protocol: SIM_SNMP_PROTOCOL.to_string(),
                                operation: "get".to_string(),
                                oid: oid.clone(),
                                value: Some(value.clone()),
                                ok: true,
                                message: "ok".to_string(),
                            };
                            (true, format!("get {oid} = {value}"), Some(resp))
                        }
                        None => {
                            let resp = self.snmp_error(CMD_SNMP_GET, oid, "no such OID");
                            (false, format!("no such OID {oid}"), Some(resp))
                        }
                    }
                }
            }
            GatewayCommand::DeauthSta { mac } => {
                let mac_norm = mac.to_ascii_uppercase();
                let mut found = false;
                self.nodes.retain(|n| {
                    if n.mac.to_ascii_uppercase() == mac_norm {
                        found = true;
                        false
                    } else {
                        true
                    }
                });
                if found {
                    (true, format!("deauthenticated {mac}"), None)
                } else {
                    (false, format!("no station {mac}"), None)
                }
            }
            GatewayCommand::SysHealth => (
                true,
                format!(
                    "free_heap={} bytes, downstream={}, stations={}",
                    self.free_heap_bytes,
                    self.downstream_online,
                    self.nodes.len()
                ),
                None,
            ),
            GatewayCommand::StaList => (
                true,
                format!("stations={}", self.online_station_count()),
                None,
            ),
            GatewayCommand::RegisterNode { mac, ip } => {
                let mac_norm = mac.to_ascii_uppercase();
                if self.nodes.iter().any(|n| n.mac.to_ascii_uppercase() == mac_norm) {
                    (false, format!("{mac} already joined"), None)
                } else {
                    self.nodes.push(NodeInfo {
                        mac: mac.clone(),
                        ip: ip.clone(),
                        free_heap_bytes: 180_224,
                        online: self.downstream_online,
                    });
                    (true, format!("{mac} joined at {ip}"), None)
                }
            }
            GatewayCommand::EnrollDevice { device_id, mac } => {
                if self.device_index(device_id).is_some() {
                    (false, format!("device {device_id} already enrolled"), None)
                } else {
                    let fingerprint = self.issue_credential(device_id);
                    self.devices.push(DeviceIdentity {
                        device_id: device_id.clone(),
                        mac: mac.clone(),
                        state: ProvisioningState::Pending,
                        credential_fingerprint: fingerprint.clone(),
                        credential_version: 1,
                    });
                    (
                        true,
                        format!("enrolled {device_id} (pending), credential {fingerprint}"),
                        None,
                    )
                }
            }
            GatewayCommand::ClaimDevice { device_id } => match self.device_index(device_id) {
                None => (false, format!("unknown device {device_id}"), None),
                Some(idx) => {
                    if self.devices[idx].state != ProvisioningState::Pending {
                        (
                            false,
                            format!(
                                "device {device_id} is {:?}, only pending devices can be claimed",
                                self.devices[idx].state
                            ),
                            None,
                        )
                    } else {
                        self.devices[idx].state = ProvisioningState::Active;
                        let mac = self.devices[idx].mac.clone();
                        if !self.nodes.iter().any(|n| n.mac.eq_ignore_ascii_case(&mac)) {
                            self.nodes.push(NodeInfo {
                                mac,
                                ip: format!("192.168.4.{}", 10 + self.nodes.len()),
                                free_heap_bytes: 180_224,
                                online: self.downstream_online,
                            });
                        }
                        (true, format!("claimed {device_id} (active)"), None)
                    }
                }
            },
            GatewayCommand::RotateCredential { device_id } => {
                match self.device_index(device_id) {
                    None => (false, format!("unknown device {device_id}"), None),
                    Some(idx) if self.devices[idx].state == ProvisioningState::Revoked => (
                        false,
                        format!("device {device_id} is revoked; cannot rotate"),
                        None,
                    ),
                    Some(idx) => {
                        let fingerprint = self.issue_credential(device_id);
                        self.devices[idx].credential_fingerprint = fingerprint.clone();
                        self.devices[idx].credential_version += 1;
                        (
                            true,
                            format!(
                                "rotated {device_id} credential to {fingerprint} (v{})",
                                self.devices[idx].credential_version
                            ),
                            None,
                        )
                    }
                }
            }
            GatewayCommand::RevokeDevice { device_id } => match self.device_index(device_id) {
                None => (false, format!("unknown device {device_id}"), None),
                Some(idx) => {
                    if self.devices[idx].state == ProvisioningState::Revoked {
                        (false, format!("device {device_id} already revoked"), None)
                    } else {
                        self.devices[idx].state = ProvisioningState::Revoked;
                        let mac = self.devices[idx].mac.clone();
                        self.nodes.retain(|n| !n.mac.eq_ignore_ascii_case(&mac));
                        (true, format!("revoked {device_id}"), None)
                    }
                }
            },
        };

        self.record(format!("{name}: {message}"));
        GatewayResponse {
            ok,
            command: name,
            message,
            snmp,
            snapshot: self.snapshot(),
        }
    }

    fn snmp_error(&self, op: &str, oid: &str, message: &str) -> SnmpResponse {
        SnmpResponse {
            protocol: SIM_SNMP_PROTOCOL.to_string(),
            operation: if op == CMD_SNMP_SET { "set" } else { "get" }.to_string(),
            oid: oid.to_string(),
            value: None,
            ok: false,
            message: message.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_downstream_switches_wifi_mode() {
        let mut gw = GatewaySim::new();
        assert_eq!(gw.wifi_mode(), WifiMode::ApSta);
        let resp = gw.apply(&GatewayCommand::NetToggleDownstream);
        assert!(resp.ok);
        assert_eq!(gw.wifi_mode(), WifiMode::Sta);
        assert!(!resp.snapshot.downstream_online);
        gw.apply(&GatewayCommand::NetToggleDownstream);
        assert_eq!(gw.wifi_mode(), WifiMode::ApSta);
    }

    #[test]
    fn snmp_set_then_get_roundtrips() {
        let mut gw = GatewaySim::new();
        let set = gw.apply(&GatewayCommand::SnmpSet {
            oid: "1.3.6.1.4.1.custom.isolate".to_string(),
            value: "true".to_string(),
        });
        assert!(set.ok);
        let get = gw.apply(&GatewayCommand::SnmpGet {
            oid: "1.3.6.1.4.1.custom.isolate".to_string(),
        });
        let snmp = get.snmp.unwrap();
        assert!(snmp.ok);
        assert_eq!(snmp.value.as_deref(), Some("true"));
    }

    #[test]
    fn snmp_fails_when_downstream_offline() {
        let mut gw = GatewaySim::new();
        gw.apply(&GatewayCommand::NetToggleDownstream); // sever
        let get = gw.apply(&GatewayCommand::SnmpGet {
            oid: "1.3.6.1.4.1.custom.relay".to_string(),
        });
        assert!(!get.ok);
        assert!(!get.snmp.unwrap().ok);
    }

    #[test]
    fn deauth_removes_station() {
        let mut gw = GatewaySim::new();
        let before = gw.snapshot().nodes.len();
        let resp = gw.apply(&GatewayCommand::DeauthSta {
            mac: "24:6f:28:00:00:01".to_string(),
        });
        assert!(resp.ok);
        assert_eq!(gw.snapshot().nodes.len(), before - 1);
    }

    #[test]
    fn register_node_is_idempotent() {
        let mut gw = GatewaySim::new();
        let first = gw.apply(&GatewayCommand::RegisterNode {
            mac: "AA:BB:CC:DD:EE:FF".to_string(),
            ip: "192.168.4.3".to_string(),
        });
        assert!(first.ok);
        let second = gw.apply(&GatewayCommand::RegisterNode {
            mac: "aa:bb:cc:dd:ee:ff".to_string(),
            ip: "192.168.4.3".to_string(),
        });
        assert!(!second.ok);
    }

    #[test]
    fn sys_health_reports_decreasing_heap() {
        let mut gw = GatewaySim::new();
        let start = gw.snapshot().free_heap_bytes;
        gw.apply(&GatewayCommand::SysHealth);
        assert!(gw.snapshot().free_heap_bytes < start);
    }

    fn enroll(gw: &mut GatewaySim, id: &str, mac: &str) -> GatewayResponse {
        gw.apply(&GatewayCommand::EnrollDevice {
            device_id: id.to_string(),
            mac: mac.to_string(),
        })
    }

    #[test]
    fn provisioning_full_lifecycle() {
        let mut gw = GatewaySim::new();
        let e = enroll(&mut gw, "dev-1", "AA:BB:CC:00:00:01");
        assert!(e.ok);
        let dev = &e.snapshot.devices[0];
        assert_eq!(dev.state, ProvisioningState::Pending);
        assert_eq!(dev.credential_version, 1);
        assert!(dev.credential_fingerprint.starts_with("cred-"));

        let nodes_before = gw.snapshot().nodes.len();
        let claim = gw.apply(&GatewayCommand::ClaimDevice {
            device_id: "dev-1".to_string(),
        });
        assert!(claim.ok);
        assert_eq!(claim.snapshot.devices[0].state, ProvisioningState::Active);
        assert_eq!(claim.snapshot.nodes.len(), nodes_before + 1);

        let prev = claim.snapshot.devices[0].credential_fingerprint.clone();
        let rot = gw.apply(&GatewayCommand::RotateCredential {
            device_id: "dev-1".to_string(),
        });
        assert!(rot.ok);
        assert_eq!(rot.snapshot.devices[0].credential_version, 2);
        assert_ne!(rot.snapshot.devices[0].credential_fingerprint, prev);

        let rev = gw.apply(&GatewayCommand::RevokeDevice {
            device_id: "dev-1".to_string(),
        });
        assert!(rev.ok);
        assert_eq!(rev.snapshot.devices[0].state, ProvisioningState::Revoked);
        assert!(!rev.snapshot.nodes.iter().any(|n| n.mac == "AA:BB:CC:00:00:01"));
    }

    #[test]
    fn duplicate_enroll_rejected() {
        let mut gw = GatewaySim::new();
        assert!(enroll(&mut gw, "dev-1", "AA:BB:CC:00:00:01").ok);
        assert!(!enroll(&mut gw, "dev-1", "AA:BB:CC:00:00:02").ok);
        assert_eq!(gw.snapshot().devices.len(), 1);
    }

    #[test]
    fn claim_unknown_or_non_pending_rejected() {
        let mut gw = GatewaySim::new();
        assert!(!gw
            .apply(&GatewayCommand::ClaimDevice {
                device_id: "ghost".to_string(),
            })
            .ok);
        enroll(&mut gw, "dev-1", "AA:BB:CC:00:00:01");
        assert!(gw.apply(&GatewayCommand::ClaimDevice {
            device_id: "dev-1".to_string(),
        }).ok);
        // Second claim on an active device is rejected.
        assert!(!gw.apply(&GatewayCommand::ClaimDevice {
            device_id: "dev-1".to_string(),
        }).ok);
    }

    #[test]
    fn revoked_device_cannot_rotate() {
        let mut gw = GatewaySim::new();
        enroll(&mut gw, "dev-1", "AA:BB:CC:00:00:01");
        gw.apply(&GatewayCommand::RevokeDevice {
            device_id: "dev-1".to_string(),
        });
        assert!(!gw.apply(&GatewayCommand::RotateCredential {
            device_id: "dev-1".to_string(),
        }).ok);
    }

    #[test]
    fn sta_list_reports_online_stations() {
        let mut gw = GatewaySim::new();
        let resp = gw.apply(&GatewayCommand::StaList);
        assert!(resp.ok);
        assert_eq!(resp.snapshot.station_count, 1);
        assert!(resp.message.contains("stations=1"));
        // Severing the downstream AP drops the station count to zero.
        gw.apply(&GatewayCommand::NetToggleDownstream);
        assert_eq!(gw.snapshot().station_count, 0);
    }

    #[test]
    fn provisioning_command_json_roundtrips() {
        let cmd = GatewayCommand::EnrollDevice {
            device_id: "dev-1".to_string(),
            mac: "AA:BB:CC:00:00:01".to_string(),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let back: GatewayCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(cmd, back);
    }

    #[test]
    fn command_json_roundtrips() {
        let cmd = GatewayCommand::SnmpSet {
            oid: "1.3.6.1.4.1.custom.relay".to_string(),
            value: "on".to_string(),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let back: GatewayCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(cmd, back);
    }
}
