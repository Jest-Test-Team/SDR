#![no_std]

extern crate alloc;

use alloc::{vec, vec::Vec};
use core::fmt;

pub use postcard;
pub use crc16;
pub use cobs;

pub mod frame;

pub const ESP_NOW_VENDOR_ID: u8 = 0x1A;
pub const MAX_ESP_NOW_PAYLOAD: usize = 250;

pub fn encode_frame(frame: &frame::TelemetryFrame) -> Result<Vec<u8>, postcard::Error> {
    let mut buf = postcard::to_allocvec(frame)?;
    let crc = crc16::State::<crc16::XMODEM>::calculate(&buf);
    buf.extend_from_slice(&crc.to_le_bytes());
    Ok(cobs::encode_vec(&buf))
}

pub fn decode_frame(data: &[u8]) -> Result<frame::TelemetryFrame, frame::DecodeError> {
    let decoded = cobs::decode_vec(data).map_err(|_| frame::DecodeError::Cobs)?;
    if decoded.len() < 2 {
        return Err(frame::DecodeError::TooShort);
    }
    let (payload, crc_bytes) = decoded.split_at(decoded.len() - 2);
    let expected_crc = u16::from_le_bytes([crc_bytes[0], crc_bytes[1]]);
    let actual_crc = crc16::State::<crc16::XMODEM>::calculate(payload);
    if actual_crc != expected_crc {
        return Err(frame::DecodeError::CrcMismatch);
    }
    postcard::from_bytes(payload).map_err(|_| frame::DecodeError::Postcard)
}