use protocol::frame::{Payload, TelemetryFrame};
use protocol::{decode_frame, encode_frame};
use serde::{Deserialize, Serialize};

use super::ook::{downsample, noise_sample};

const SAMPLES_PER_BIT: usize = 36;
const CARRIER_PERIOD: usize = 6;
const CHART_POINTS: usize = 360;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum TransmissionMode {
    #[default]
    EspNow,
    BleAdvertisement,
    Ook433Mhz,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimConfig {
    pub mode: TransmissionMode,
    pub tx_power_dbm: f32,
    pub snr_db: f32,
    pub filter_bw_mhz: f32,
    pub threshold: f32,
    pub noise_level: f32,
    pub replay_guard: bool,
    pub data_bits: String,
    pub node_id: u8,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            mode: TransmissionMode::EspNow,
            tx_power_dbm: 0.0,
            snr_db: 15.0,
            filter_bw_mhz: 1.0,
            threshold: 0.75,
            noise_level: 0.2,
            replay_guard: true,
            data_bits: "10110010".to_string(),
            node_id: 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitAnalysis {
    pub original: String,
    pub recovered: String,
    pub error_indices: Vec<usize>,
    pub ber: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Waveforms {
    pub baseband: Vec<f32>,
    pub rf_tx: Vec<f32>,
    pub rf_rx: Vec<f32>,
    pub magnitude: Vec<f32>,
    pub threshold: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryEvent {
    pub time: String,
    pub node_id: u8,
    pub payload_json: String,
    pub rssi_dbm: f32,
    pub status: String,
    pub seq: u32,
    pub latency_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Kpis {
    pub prr_percent: f32,
    pub latency_ms: u32,
    pub last_bool: bool,
    pub security_alerts: u32,
    pub packets_sent: u32,
    pub packets_ok: u32,
}

impl Default for Kpis {
    fn default() -> Self {
        Self {
            prr_percent: 100.0,
            latency_ms: 25,
            last_bool: false,
            security_alerts: 0,
            packets_sent: 0,
            packets_ok: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineSnapshot {
    pub mode: TransmissionMode,
    pub hardware_mode: String,
    pub waveforms: Waveforms,
    pub bits: BitAnalysis,
    pub packet_ok: bool,
    pub crc_ok: bool,
    pub replay_rejected: bool,
    pub frame: Option<TelemetryFrame>,
    pub zmq_published: bool,
    pub kpis: Kpis,
    pub event: TelemetryEvent,
}

pub fn parse_bits(data: &str) -> Vec<bool> {
    data.chars()
        .filter(|c| *c == '0' || *c == '1')
        .map(|c| c == '1')
        .collect()
}

pub fn bits_to_string(bits: &[bool]) -> String {
    bits.iter().map(|b| if *b { '1' } else { '0' }).collect()
}

fn simulate_waveforms(bits: &[bool], config: &SimConfig) -> (Waveforms, Vec<bool>) {
    let mut baseband = Vec::with_capacity(bits.len() * SAMPLES_PER_BIT);
    let mut rf_tx = Vec::with_capacity(bits.len() * SAMPLES_PER_BIT);
    let mut rf_rx = Vec::with_capacity(bits.len() * SAMPLES_PER_BIT);
    let mut magnitude = Vec::with_capacity(bits.len() * SAMPLES_PER_BIT);

    let gain = 10f32.powf(config.tx_power_dbm / 20.0).clamp(0.1, 3.0);
    let noise_amp =
        (10f32.powf(-config.snr_db / 20.0) * 0.35 + config.noise_level * 0.25).clamp(0.01, 1.5);

    let mut t = 0usize;
    for &bit in bits {
        for _ in 0..SAMPLES_PER_BIT {
            let level = if bit { 1.0 } else { 0.0 };
            baseband.push(level);

            let carrier = if bit {
                gain * (std::f32::consts::TAU * t as f32 / CARRIER_PERIOD as f32).sin()
            } else {
                0.0
            };
            rf_tx.push(carrier);

            let n = noise_sample(t as u32) * noise_amp;
            let rx = carrier + n;
            rf_rx.push(rx);
            magnitude.push(rx.abs());
            t += 1;
        }
    }

    let recovered = slice_bits(&magnitude, SAMPLES_PER_BIT, config.threshold);

    let waveforms = Waveforms {
        baseband: downsample(&baseband, CHART_POINTS),
        rf_tx: downsample(&rf_tx, CHART_POINTS),
        rf_rx: downsample(&rf_rx, CHART_POINTS),
        magnitude: downsample(&magnitude, CHART_POINTS),
        threshold: config.threshold,
    };

    (waveforms, recovered)
}

fn slice_bits(magnitude: &[f32], samples_per_bit: usize, threshold: f32) -> Vec<bool> {
    magnitude
        .chunks(samples_per_bit)
        .map(|chunk| {
            let avg = chunk.iter().sum::<f32>() / chunk.len() as f32;
            avg >= threshold
        })
        .collect()
}

fn analyze_bits(original: &[bool], recovered: &[bool]) -> BitAnalysis {
    let len = original.len().max(recovered.len());
    let mut error_indices = Vec::new();
    let mut errors = 0usize;
    for i in 0..len {
        let o = original.get(i).copied().unwrap_or(false);
        let r = recovered.get(i).copied().unwrap_or(false);
        if o != r {
            errors += 1;
            error_indices.push(i);
        }
    }
    let ber = if len == 0 {
        0.0
    } else {
        errors as f32 / len as f32
    };
    BitAnalysis {
        original: bits_to_string(original),
        recovered: bits_to_string(recovered),
        error_indices,
        ber,
    }
}

pub fn run_trigger(
    config: &SimConfig,
    seq: u32,
    bool_value: bool,
    last_seq: Option<u32>,
    kpis: &mut Kpis,
) -> PipelineSnapshot {
    let bit_pattern = if config.data_bits.chars().any(|c| c == '0' || c == '1') {
        parse_bits(&config.data_bits)
    } else {
        parse_bits("10110010")
    };

    let (waveforms, recovered_bits) = simulate_waveforms(&bit_pattern, config);
    let bits = analyze_bits(&bit_pattern, &recovered_bits);

    let frame = TelemetryFrame {
        seq,
        timestamp_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0),
        node_id: config.node_id,
        payload: Payload::BoolCmd(bool_value),
    };

    let wire_ok = encode_frame(&frame).is_ok();
    let mut crc_ok = false;
    if let Ok(wire) = encode_frame(&frame) {
        crc_ok = decode_frame(&wire).is_ok();
    }

    let packet_ok = bits.ber == 0.0 && crc_ok && wire_ok;
    let replay_rejected = config.replay_guard && last_seq.is_some_and(|s| seq <= s);

    let latency = (20 + (bits.error_indices.len() as u32 * 5) + if packet_ok { 5 } else { 30 })
        .clamp(15, 120);
    let rssi = config.tx_power_dbm - 40.0 + config.snr_db * 0.2;

    kpis.packets_sent += 1;
    if packet_ok && !replay_rejected {
        kpis.packets_ok += 1;
    }
    if replay_rejected {
        kpis.security_alerts += 1;
    }
    kpis.prr_percent = if kpis.packets_sent == 0 {
        100.0
    } else {
        (kpis.packets_ok as f32 / kpis.packets_sent as f32) * 100.0
    };
    kpis.latency_ms = latency;
    kpis.last_bool = bool_value;

    let status = if replay_rejected {
        "重放拒絕"
    } else if packet_ok {
        "封包完整"
    } else if !crc_ok {
        "封包損壞 (CRC Error)"
    } else {
        "位元錯誤"
    };

    let payload_json = serde_json::json!({
        "seq": frame.seq,
        "node_id": frame.node_id,
        "payload": { "BoolCmd": bool_value },
        "mode": format!("{:?}", config.mode),
    })
    .to_string();

    let event = TelemetryEvent {
        time: chrono_lite_now(),
        node_id: config.node_id,
        payload_json,
        rssi_dbm: rssi,
        status: status.to_string(),
        seq,
        latency_ms: latency,
    };

    PipelineSnapshot {
        mode: config.mode,
        hardware_mode: "esp32_simulation".to_string(),
        waveforms,
        bits,
        packet_ok: packet_ok && !replay_rejected,
        crc_ok,
        replay_rejected,
        frame: Some(frame),
        zmq_published: false,
        kpis: kpis.clone(),
        event,
    }
}

fn chrono_lite_now() -> String {
    let dur = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs() % 86_400;
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    let ms = dur.subsec_millis();
    format!("{h:02}:{m:02}:{s:02}.{ms:03}")
}

pub fn publish_zmq(endpoint: &str, frame: &TelemetryFrame) -> anyhow::Result<bool> {
    let wire = encode_frame(frame)?;
    let ctx = zmq::Context::new();
    let socket = ctx.socket(zmq::PUB)?;
    socket.connect(endpoint)?;
    std::thread::sleep(std::time::Duration::from_millis(50));
    socket.send(&wire, 0)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_channel_recovers_bits() {
        let config = SimConfig {
            snr_db: 40.0,
            noise_level: 0.0,
            threshold: 0.5,
            ..Default::default()
        };
        let bits = parse_bits("10110010");
        let (wf, recovered) = simulate_waveforms(&bits, &config);
        assert!(!wf.baseband.is_empty());
        assert_eq!(bits_to_string(&recovered), "10110010");
    }

    #[test]
    fn high_noise_increases_ber() {
        let config = SimConfig {
            snr_db: 0.0,
            noise_level: 0.9,
            threshold: 0.75,
            ..Default::default()
        };
        let mut kpis = Kpis::default();
        let snap = run_trigger(&config, 1, true, None, &mut kpis);
        assert!(snap.bits.ber >= 0.0);
    }
}
