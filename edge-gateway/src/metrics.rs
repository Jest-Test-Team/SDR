use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use prometheus::{Encoder, IntCounter, TextEncoder};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use tokio::sync::mpsc;

pub static DECODE_ERRORS: LazyLock<IntCounter> =
    LazyLock::new(|| IntCounter::new("decode_errors_total", "UART decode errors").unwrap());

pub static FRAMES_PUBLISHED: LazyLock<IntCounter> =
    LazyLock::new(|| IntCounter::new("frames_published_total", "Frames published to ZMQ").unwrap());

#[derive(Clone)]
pub struct ControlState {
    pub tx: mpsc::Sender<String>,
}

#[derive(Debug, Deserialize)]
pub struct FirmwareConfigRequest {
    pub node_id: Option<u8>,
    pub tx_power_dbm: Option<i8>,
    pub data_bits: Option<String>,
    pub mode: Option<String>,
    pub snr_db: Option<f32>,
    pub noise_level: Option<f32>,
    pub filter_bw_mhz: Option<f32>,
    pub threshold: Option<f32>,
    pub replay_guard: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct FirmwareConfigResponse {
    pub ok: bool,
    pub applied: Vec<&'static str>,
    pub unsupported: Vec<&'static str>,
    pub command: String,
}

pub fn router(control: ControlState) -> Router {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/metrics", get(metrics_handler))
        .route("/api/v1/firmware/config", post(apply_firmware_config))
        .with_state(control)
}

async fn metrics_handler() -> String {
    let mut buffer = Vec::new();
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap_or_default()
}

async fn apply_firmware_config(
    State(control): State<ControlState>,
    Json(req): Json<FirmwareConfigRequest>,
) -> Result<Json<FirmwareConfigResponse>, StatusCode> {
    let node_id = req.node_id.unwrap_or(0);
    let tx_power = req.tx_power_dbm.unwrap_or(i8::MIN);
    let boot_byte = req
        .data_bits
        .as_deref()
        .and_then(parse_data_bits)
        .unwrap_or(0xB2);
    let command = format!("SDRCTL,{node_id},{tx_power},{boot_byte}\n");

    control
        .tx
        .send(command.clone())
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let mut applied = Vec::new();
    if req.node_id.is_some() {
        applied.push("node_id");
    }
    if req.tx_power_dbm.is_some() {
        applied.push("tx_power_dbm");
    }
    if req.data_bits.is_some() {
        applied.push("data_bits");
    }

    let mut unsupported = Vec::new();
    if req.mode.as_deref().is_some_and(|mode| mode != "EspNow") {
        unsupported.push("mode_non_esp_now");
    }
    if req.snr_db.is_some() {
        unsupported.push("snr_db");
    }
    if req.noise_level.is_some() {
        unsupported.push("noise_level");
    }
    if req.filter_bw_mhz.is_some() {
        unsupported.push("filter_bw_mhz");
    }
    if req.threshold.is_some() {
        unsupported.push("threshold");
    }
    if req.replay_guard.is_some() {
        unsupported.push("replay_guard");
    }

    Ok(Json(FirmwareConfigResponse {
        ok: true,
        applied,
        unsupported,
        command: command.trim().to_string(),
    }))
}

fn parse_data_bits(bits: &str) -> Option<u8> {
    if bits.len() != 8 || !bits.bytes().all(|b| matches!(b, b'0' | b'1')) {
        return None;
    }
    u8::from_str_radix(bits, 2).ok()
}
