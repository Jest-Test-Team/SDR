//! Gateway sidecar surface for the HIL simulator.
//!
//! Re-exports the hardware-free Secure Telemetry Gateway model from
//! `firmware-software-sim` so the dashboard can drive the same command set that
//! the on-device ESP32-S3 firmware implements (downstream toggle, simulated
//! SNMP, deauth, system health).

pub use firmware_software_sim::gateway::{
    CMD_DEAUTH_STA, CMD_NET_TOGGLE_DOWNSTREAM, CMD_REGISTER_NODE, CMD_SNMP_GET, CMD_SNMP_SET,
    CMD_SYS_HEALTH, GatewayCommand, GatewayResponse, GatewaySim, GatewaySnapshot, NodeInfo,
    OidEntry, SnmpResponse, WifiMode,
};
