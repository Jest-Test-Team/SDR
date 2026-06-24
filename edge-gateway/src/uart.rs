use anyhow::{Context, Result};
use protocol::{decode_frame, encode_frame};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::sleep;
use tracing::{info, warn};

pub fn split_cobs_frames(buffer: &mut Vec<u8>) -> Vec<Vec<u8>> {
    let mut frames = Vec::new();
    let mut start = 0usize;
    for (idx, byte) in buffer.iter().enumerate() {
        if *byte == 0 {
            if idx > start {
                frames.push(buffer[start..idx].to_vec());
            }
            start = idx + 1;
        }
    }
    if start < buffer.len() {
        buffer.drain(..start);
    } else {
        buffer.clear();
    }
    frames
}

pub async fn run_uart_reader(port: String, baud: u32, tx: mpsc::Sender<Vec<u8>>) -> Result<()> {
    use tokio::io::AsyncReadExt;
    use tokio_serial::SerialPortBuilderExt;

    let mut buffer = Vec::with_capacity(4096);
    let mut read_buf = [0u8; 256];

    loop {
        let mut serial = match tokio_serial::new(&port, baud).open_native_async() {
            Ok(p) => {
                info!("serial port open: {port}");
                p
            }
            Err(e) => {
                warn!("open serial port failed: {e}, retry in 1s");
                sleep(Duration::from_secs(1)).await;
                continue;
            }
        };

        loop {
            match serial.read(&mut read_buf).await {
                Ok(0) => {
                    sleep(Duration::from_millis(10)).await;
                }
                Ok(n) => {
                    buffer.extend_from_slice(&read_buf[..n]);
                    for frame in split_cobs_frames(&mut buffer) {
                        match decode_frame(&frame) {
                            Ok(decoded) => {
                                info!(
                                    seq = decoded.seq,
                                    node_id = decoded.node_id,
                                    "UART frame received"
                                );
                                let wire = encode_frame(&decoded).context("re-encode frame")?;
                                if tx.send(wire).await.is_err() {
                                    return Ok(());
                                }
                            }
                            Err(e) => {
                                crate::metrics::DECODE_ERRORS.inc();
                                warn!("frame decode error: {}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("serial read error: {e}, reconnecting in 1s");
                    break;
                }
            }
        }
        buffer.clear();
        sleep(Duration::from_secs(1)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol::frame::{Payload, TelemetryFrame};

    #[test]
    fn splits_delimited_frames() {
        let frame = TelemetryFrame {
            seq: 1,
            timestamp_ms: 0,
            node_id: 1,
            payload: Payload::BoolCmd(true),
        };
        let mut wire = encode_frame(&frame).unwrap();
        wire.push(0);
        wire.extend_from_slice(&encode_frame(&frame).unwrap());
        wire.push(0);

        let mut buffer = wire;
        let frames = split_cobs_frames(&mut buffer);
        assert_eq!(frames.len(), 2);
        assert!(decode_frame(&frames[0]).is_ok());
    }
}
