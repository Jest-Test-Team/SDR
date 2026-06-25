//! Gateway control link between the ESP32-S3 software-sim node and the ESP32
//! gateway, carried over ESP-NOW.
//!
//! Topology:
//! ```text
//! Mac (control-plane) --USB-- ESP32-S3 [software-sim sender/receiver]
//!                                  ^ ESP-NOW (this protocol)
//!                              ESP32 [gateway, AP-STA routing]
//! ```
//!
//! These messages use a distinct vendor id from telemetry frames so the two
//! never alias on the wire: a receiver can tell a `GwMsg` apart from a
//! [`crate::frame::TelemetryFrame`] by the first byte alone.

use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::frame::DecodeError;

/// First byte of an ESP-NOW gateway-link packet (telemetry uses
/// [`crate::ESP_NOW_VENDOR_ID`] = 0x1A).
pub const GW_LINK_VENDOR_ID: u8 = 0x1B;

/// Wi-Fi mode reported by the gateway. `ApSta` keeps the downstream AP up;
/// `Sta` has dropped it (downstream severed).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum WifiMode {
    Sta,
    ApSta,
}

/// Request/response messages exchanged over the gateway control link.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum GwMsg {
    /// CMD_SYS_HEALTH
    HealthReq,
    HealthResp {
        free_heap: u32,
        uptime_ms: u64,
        rx_count: u32,
        wifi_mode: WifiMode,
    },
    /// CMD_NET_TOGGLE_DOWNSTREAM
    ToggleReq,
    ToggleResp {
        downstream_online: bool,
        wifi_mode: WifiMode,
    },
    /// CMD_DEAUTH_STA (aid 0 = all stations)
    DeauthReq,
    DeauthResp {
        kicked: u32,
    },
    /// CMD_STALIST
    StaListReq,
    StaListResp {
        count: u32,
    },
    /// CMD_SNMP_SET
    SnmpSetReq {
        oid: String,
        value: String,
    },
    /// CMD_SNMP_GET
    SnmpGetReq {
        oid: String,
    },
    SnmpResp {
        oid: String,
        value: Option<String>,
        ok: bool,
    },
}

/// ESP-NOW framing: `[GW_LINK_VENDOR_ID][postcard || crc16_le]`.
pub fn encode_gw_espnow(msg: &GwMsg) -> Result<Vec<u8>, postcard::Error> {
    let mut payload = postcard::to_allocvec(msg)?;
    let crc = crate::crc16_xmodem(&payload);
    payload.extend_from_slice(&crc.to_le_bytes());

    let mut buf = Vec::with_capacity(1 + payload.len());
    buf.push(GW_LINK_VENDOR_ID);
    buf.extend_from_slice(&payload);
    Ok(buf)
}

/// Decode an ESP-NOW gateway-link packet. Returns [`DecodeError::VendorMismatch`]
/// for non-gateway-link packets (e.g. telemetry frames), so a single recv
/// callback can demux both.
pub fn decode_gw_espnow(data: &[u8]) -> Result<GwMsg, DecodeError> {
    if data.is_empty() {
        return Err(DecodeError::TooShort);
    }
    if data[0] != GW_LINK_VENDOR_ID {
        return Err(DecodeError::VendorMismatch);
    }
    let body = &data[1..];
    if body.len() < 2 {
        return Err(DecodeError::TooShort);
    }
    let (payload, crc_bytes) = body.split_at(body.len() - 2);
    let expected = u16::from_le_bytes([crc_bytes[0], crc_bytes[1]]);
    if crate::crc16_xmodem(payload) != expected {
        return Err(DecodeError::CrcMismatch);
    }
    postcard::from_bytes(payload).map_err(|_| DecodeError::Postcard)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    fn roundtrip(msg: GwMsg) {
        let wire = encode_gw_espnow(&msg).unwrap();
        assert_eq!(wire[0], GW_LINK_VENDOR_ID);
        assert_eq!(decode_gw_espnow(&wire).unwrap(), msg);
    }

    #[test]
    fn health_roundtrip() {
        roundtrip(GwMsg::HealthReq);
        roundtrip(GwMsg::HealthResp {
            free_heap: 180_224,
            uptime_ms: 42_000,
            rx_count: 7,
            wifi_mode: WifiMode::ApSta,
        });
    }

    #[test]
    fn toggle_and_deauth_roundtrip() {
        roundtrip(GwMsg::ToggleReq);
        roundtrip(GwMsg::ToggleResp {
            downstream_online: false,
            wifi_mode: WifiMode::Sta,
        });
        roundtrip(GwMsg::DeauthReq);
        roundtrip(GwMsg::DeauthResp { kicked: 2 });
    }

    #[test]
    fn snmp_roundtrip() {
        roundtrip(GwMsg::SnmpSetReq {
            oid: "1.3.6.1.4.1.custom.isolate".to_string(),
            value: "true".to_string(),
        });
        roundtrip(GwMsg::SnmpGetReq {
            oid: "1.3.6.1.4.1.custom.relay".to_string(),
        });
        roundtrip(GwMsg::SnmpResp {
            oid: "1.3.6.1.4.1.custom.relay".to_string(),
            value: Some("on".to_string()),
            ok: true,
        });
    }

    #[test]
    fn rejects_telemetry_vendor_id() {
        let wire = crate::encode_espnow(&crate::frame::TelemetryFrame {
            seq: 1,
            timestamp_ms: 0,
            node_id: 1,
            payload: crate::frame::Payload::BoolCmd(true),
        })
        .unwrap();
        assert_eq!(decode_gw_espnow(&wire), Err(DecodeError::VendorMismatch));
    }

    #[test]
    fn rejects_crc_corruption() {
        let mut wire = encode_gw_espnow(&GwMsg::HealthReq).unwrap();
        let last = wire.len() - 1;
        wire[last] ^= 0xFF;
        assert_eq!(decode_gw_espnow(&wire), Err(DecodeError::CrcMismatch));
    }
}
