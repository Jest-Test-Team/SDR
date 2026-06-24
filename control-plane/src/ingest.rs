use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
};
use protocol::{ReplayGuard, frame::TelemetryFrame};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

use crate::{live::LiveBus, store::TelemetryStore};

#[derive(Clone)]
pub struct IngestState {
    pub replay: Arc<Mutex<ReplayGuard>>,
    pub store: Arc<TelemetryStore>,
    pub live: LiveBus,
}

#[derive(Debug, Deserialize)]
pub struct IngestFrameRequest {
    pub source: Option<String>,
    pub frame: TelemetryFrame,
}

#[derive(Debug, Serialize)]
struct IngestFrameResponse {
    ok: bool,
    source: String,
    seq: u32,
    node_id: u8,
}

pub fn router(state: IngestState) -> Router {
    Router::new()
        .route("/api/v1/ingest/frame", post(ingest_frame))
        .with_state(state)
}

async fn ingest_frame(
    State(state): State<IngestState>,
    Json(request): Json<IngestFrameRequest>,
) -> Result<Json<IngestFrameResponse>, IngestError> {
    let source = request.source.unwrap_or_else(|| "software-sim".to_string());
    if source.trim() != "software-sim" {
        return Err(IngestError::BadRequest(
            "unsupported ingest source".to_string(),
        ));
    }

    let seq = request.frame.seq;
    let node_id = request.frame.node_id;
    {
        let mut replay = state
            .replay
            .lock()
            .map_err(|_| IngestError::Internal("replay mutex poisoned".to_string()))?;
        crate::subscriber::process_frame(
            request.frame,
            &mut replay,
            &state.store,
            Some(&state.live),
        )
        .map_err(|e| IngestError::Internal(e.to_string()))?;
    }
    crate::metrics::FRAMES_DECODED.inc();

    Ok(Json(IngestFrameResponse {
        ok: true,
        source: "software-sim".to_string(),
        seq,
        node_id,
    }))
}

enum IngestError {
    BadRequest(String),
    Internal(String),
}

impl IntoResponse for IngestError {
    fn into_response(self) -> Response {
        match self {
            Self::BadRequest(message) => (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "ok": false, "error": message })),
            )
                .into_response(),
            Self::Internal(message) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "ok": false, "error": message })),
            )
                .into_response(),
        }
    }
}
