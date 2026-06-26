//! Gateway sidecar surface for the HIL simulator.
//!
//! Re-exports the hardware-free Secure Telemetry Gateway model from
//! `firmware-software-sim` so the dashboard can drive the same command set that
//! the on-device ESP32-S3 firmware implements (downstream toggle, simulated
//! SNMP, deauth, system health).

pub use firmware_software_sim::gateway::{
    CMD_CLAIM_DEVICE, CMD_DEAUTH_STA, CMD_ENROLL_DEVICE, CMD_NET_TOGGLE_DOWNSTREAM,
    CMD_REGISTER_NODE, CMD_REVOKE_DEVICE, CMD_ROTATE_CREDENTIAL, CMD_SNMP_GET, CMD_SNMP_SET,
    CMD_SYS_HEALTH, DeviceIdentity, GatewayCommand, GatewayResponse, GatewaySim, GatewaySnapshot,
    NodeInfo, OidEntry, ProvisioningState, SnmpResponse, WifiMode,
};
