use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, put},
    Json, Router,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};

use crate::sim::{Kpis, SimConfig, TransmissionMode};
use crate::state::AppState;

#[derive(Serialize)]
struct StatusResponse {
    hardware_mode: &'static str,
    kpis: Kpis,
    config: SimConfig,
}

#[derive(Deserialize)]
struct TriggerRequest {
    value: Option<bool>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/status", get(get_status))
        .route("/api/v1/config", get(get_config).put(update_config))
        .route("/api/v1/trigger", post(trigger))
        .route("/api/v1/events", get(get_events))
        .route("/ws/live", get(ws_handler))
        .with_state(state)
}

async fn get_status(State(state): State<AppState>) -> Json<StatusResponse> {
    Json(StatusResponse {
        hardware_mode: "esp32_simulation",
        kpis: state.kpis.read().await.clone(),
        config: state.config.read().await.clone(),
    })
}

async fn get_config(State(state): State<AppState>) -> Json<SimConfig> {
    Json(state.config.read().await.clone())
}

async fn update_config(
    State(state): State<AppState>,
    Json(mut config): Json<SimConfig>,
) -> Result<Json<SimConfig>, StatusCode> {
    if config.data_bits.chars().all(|c| c != '0' && c != '1') {
        config.data_bits = "10110010".to_string();
    }
    config.threshold = config.threshold.clamp(0.1, 1.2);
    config.noise_level = config.noise_level.clamp(0.0, 1.0);
    config.snr_db = config.snr_db.clamp(-5.0, 40.0);
    *state.config.write().await = config.clone();
    Ok(Json(config))
}

async fn trigger(
    State(state): State<AppState>,
    body: Option<Json<TriggerRequest>>,
) -> Json<serde_json::Value> {
    let value = body.and_then(|b| b.value).unwrap_or(true);
    let snap = state.trigger(value).await;
    Json(serde_json::json!({ "ok": true, "snapshot": snap }))
}

async fn get_events(State(state): State<AppState>) -> Json<Vec<crate::sim::TelemetryEvent>> {
    Json(state.events.read().await.clone())
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = state.tx.subscribe();

    let init = serde_json::json!({
        "type": "hello",
        "hardware_mode": "esp32_simulation",
        "config": state.config.read().await.clone(),
        "kpis": state.kpis.read().await.clone(),
        "events": state.events.read().await.clone(),
    });

    if sender
        .send(Message::Text(init.to_string()))
        .await
        .is_err()
    {
        return;
    }

    let mut send_task = tokio::spawn(async move {
        while let Ok(snap) = rx.recv().await {
            let msg = serde_json::json!({ "type": "snapshot", "data": snap }).to_string();
            if sender.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
    });

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let Message::Text(text) = msg {
                if text.contains("ping") {
                    continue;
                }
            }
        }
    });

    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    }
}

pub fn mode_label(mode: TransmissionMode) -> &'static str {
    match mode {
        TransmissionMode::EspNow => "ESP-NOW",
        TransmissionMode::BleAdvertisement => "BLE Advertisement",
        TransmissionMode::Ook433Mhz => "433MHz OOK",
    }
}
