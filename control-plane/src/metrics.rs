use axum::{routing::get, Router};
use prometheus::{Encoder, IntCounter, TextEncoder};
use std::sync::LazyLock;

pub static FRAMES_DECODED: LazyLock<IntCounter> = LazyLock::new(|| {
    IntCounter::new("frames_decoded_total", "Successfully decoded telemetry frames").unwrap()
});

pub fn router() -> Router {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/metrics", get(metrics_handler))
}

async fn metrics_handler() -> String {
    let mut buffer = Vec::new();
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap_or_default()
}
