#![no_std]

extern crate alloc;

use alloc::{vec, vec::Vec};

pub use cobs;
pub use crc16;
pub use postcard;

pub mod frame;
pub mod replay;

pub use replay::ReplayGuard;

pub const ESP_NOW_VENDOR_ID: u8 = 0x1A;
pub const MAX_ESP_NOW_PAYLOAD: usize = 250;

/// UART framing: COBS(postcard || crc16_le)
pub fn encode_frame(frame: &frame::TelemetryFrame) -> Result<Vec<u8>, postcard::Error> {
    let payload = encode_payload(frame)?;
    Ok(cobs::encode_vec(&payload))
}

/// UART framing: COBS(postcard || crc16_le)
pub fn decode_frame(data: &[u8]) -> Result<frame::TelemetryFrame, frame::DecodeError> {
    let decoded = cobs::decode_vec(data).map_err(|_| frame::DecodeError::Cobs)?;
    decode_payload(&decoded)
}

/// ESP-NOW framing: [vendor_id][postcard || crc16_le]
pub fn encode_espnow(frame: &frame::TelemetryFrame) -> Result<Vec<u8>, postcard::Error> {
    let mut buf = Vec::with_capacity(1 + 64);
    buf.push(ESP_NOW_VENDOR_ID);
    buf.extend_from_slice(&encode_payload(frame)?);
    Ok(buf)
}

/// ESP-NOW framing: [vendor_id][postcard || crc16_le]
pub fn decode_espnow(data: &[u8]) -> Result<frame::TelemetryFrame, frame::DecodeError> {
    if data.is_empty() {
        return Err(frame::DecodeError::TooShort);
    }
    if data[0] != ESP_NOW_VENDOR_ID {
        return Err(frame::DecodeError::VendorMismatch);
    }
    decode_payload(&data[1..])
}

fn encode_payload(frame: &frame::TelemetryFrame) -> Result<Vec<u8>, postcard::Error> {
    let mut buf = postcard::to_allocvec(frame)?;
    let crc = crc16::State::<crc16::XMODEM>::calculate(&buf);
    buf.extend_from_slice(&crc.to_le_bytes());
    Ok(buf)
}

fn decode_payload(data: &[u8]) -> Result<frame::TelemetryFrame, frame::DecodeError> {
    if data.len() < 2 {
        return Err(frame::DecodeError::TooShort);
    }
    let (payload, crc_bytes) = data.split_at(data.len() - 2);
    let expected_crc = u16::from_le_bytes([crc_bytes[0], crc_bytes[1]]);
    let actual_crc = crc16::State::<crc16::XMODEM>::calculate(payload);
    if actual_crc != expected_crc {
        return Err(frame::DecodeError::CrcMismatch);
    }
    postcard::from_bytes(payload).map_err(|_| frame::DecodeError::Postcard)
}

#[cfg(test)]
mod tests {
    use super::*;
    use frame::{Payload, TelemetryFrame};

    fn sample_frame(seq: u32) -> TelemetryFrame {
        TelemetryFrame {
            seq,
            timestamp_ms: 1_234,
            node_id: 0x01,
            payload: Payload::BoolCmd(true),
        }
    }

    #[test]
    fn uart_roundtrip() {
        let frame = sample_frame(42);
        let encoded = encode_frame(&frame).unwrap();
        let decoded = decode_frame(&encoded).unwrap();
        assert_eq!(frame, decoded);
    }

    #[test]
    fn espnow_roundtrip() {
        let frame = sample_frame(7);
        let encoded = encode_espnow(&frame).unwrap();
        assert_eq!(encoded[0], ESP_NOW_VENDOR_ID);
        let decoded = decode_espnow(&encoded).unwrap();
        assert_eq!(frame, decoded);
    }

    #[test]
    fn espnow_rejects_wrong_vendor() {
        let frame = sample_frame(1);
        let mut encoded = encode_espnow(&frame).unwrap();
        encoded[0] = 0xFF;
        assert!(matches!(
            decode_espnow(&encoded),
            Err(frame::DecodeError::VendorMismatch)
        ));
    }

    #[test]
    fn uart_and_espnow_share_payload() {
        let frame = sample_frame(3);
        let uart = encode_frame(&frame).unwrap();
        let esp = encode_espnow(&frame).unwrap();
        let uart_payload = cobs::decode_vec(&uart).unwrap();
        assert_eq!(&uart_payload, &esp[1..]);
    }
}
