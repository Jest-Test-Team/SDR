#![no_std]

extern crate alloc;

use alloc::{vec, vec::Vec};
use core::fmt;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TelemetryFrame {
    pub seq: u32,
    pub timestamp_ms: u64,
    pub node_id: u8,
    pub payload: Payload,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum Payload {
    BoolCmd(bool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    Cobs,
    TooShort,
    CrcMismatch,
    Postcard,
    VendorMismatch,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecodeError::Cobs => write!(f, "COBS decode error"),
            DecodeError::TooShort => write!(f, "Frame too short for CRC"),
            DecodeError::CrcMismatch => write!(f, "CRC mismatch"),
            DecodeError::Postcard => write!(f, "Postcard deserialization error"),
            DecodeError::VendorMismatch => write!(f, "ESP-NOW vendor ID mismatch"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{encode_frame, decode_frame};

    #[test]
    fn test_roundtrip() {
        let frame = TelemetryFrame {
            seq: 42,
            timestamp_ms: 1234567890,
            node_id: 0xAB,
            payload: Payload::BoolCmd(true),
        };

        let encoded = encode_frame(&frame).unwrap();
        let decoded = decode_frame(&encoded).unwrap();
        assert_eq!(frame, decoded);
    }

    #[test]
    fn test_false_roundtrip() {
        let frame = TelemetryFrame {
            seq: 1,
            timestamp_ms: 0,
            node_id: 0x01,
            payload: Payload::BoolCmd(false),
        };

        let encoded = encode_frame(&frame).unwrap();
        let decoded = decode_frame(&encoded).unwrap();
        assert_eq!(frame, decoded);
    }

    #[test]
    fn test_crc_corruption() {
        let frame = TelemetryFrame {
            seq: 1,
            timestamp_ms: 0,
            node_id: 0x01,
            payload: Payload::BoolCmd(true),
        };

        let mut encoded = encode_frame(&frame).unwrap();
        // Corrupt a byte
        if !encoded.is_empty() {
            encoded[0] ^= 0xFF;
        }
        assert!(decode_frame(&encoded).is_err());
    }
}