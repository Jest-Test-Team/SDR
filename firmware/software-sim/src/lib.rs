pub mod gateway;

use protocol::encode_frame;
use protocol::frame::{Payload, TelemetryFrame};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardRole {
    Esp32TxNode,
    Esp32S3Gateway,
}

pub fn tx_bool_frame(seq: u32, node_id: u8, timestamp_ms: u64, value: bool) -> TelemetryFrame {
    TelemetryFrame {
        seq,
        timestamp_ms,
        node_id,
        payload: Payload::BoolCmd(value),
    }
}

pub fn tx_boot_frame(seq: u32, node_id: u8, timestamp_ms: u64, value: u8) -> TelemetryFrame {
    TelemetryFrame {
        seq,
        timestamp_ms,
        node_id,
        payload: Payload::ByteCmd(value),
    }
}

pub fn esp32_tx_bytes(frame: &TelemetryFrame) -> anyhow::Result<Vec<u8>> {
    Ok(protocol::encode_espnow(frame)?)
}

pub fn esp32s3_gateway_bytes(frame: &TelemetryFrame) -> anyhow::Result<Vec<u8>> {
    Ok(encode_frame(frame)?)
}

pub fn gateway_control_line(node_id: u8, tx_power_dbm: i8, boot_byte: u8) -> String {
    format!("SDRCTL,{node_id},{tx_power_dbm},{boot_byte}\n")
}

pub fn write_frame_bytes(frame: &TelemetryFrame, role: BoardRole) -> anyhow::Result<Vec<u8>> {
    match role {
        BoardRole::Esp32TxNode => esp32_tx_bytes(frame),
        BoardRole::Esp32S3Gateway => esp32s3_gateway_bytes(frame),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol::{decode_espnow, decode_frame};

    #[test]
    fn tx_node_bytes_roundtrip() {
        let frame = tx_bool_frame(7, 1, 1234, true);
        let bytes = esp32_tx_bytes(&frame).unwrap();
        let decoded = decode_espnow(&bytes).unwrap();
        assert_eq!(frame, decoded);
    }

    #[test]
    fn gateway_uart_bytes_roundtrip() {
        let frame = tx_boot_frame(11, 1, 99, 0xB2);
        let bytes = esp32s3_gateway_bytes(&frame).unwrap();
        let decoded = decode_frame(&bytes).unwrap();
        assert_eq!(frame, decoded);
    }

    #[test]
    fn control_line_matches_gateway_protocol() {
        assert_eq!(gateway_control_line(1, 10, 0xB2), "SDRCTL,1,10,178\n");
    }
}
