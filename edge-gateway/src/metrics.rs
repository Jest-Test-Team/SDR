use axum::{routing::get, Router};
use prometheus::{Encoder, IntCounter, TextEncoder};
use std::sync::LazyLock;

pub static DECODE_ERRORS: LazyLock<IntCounter> =
    LazyLock::new(|| IntCounter::new("decode_errors_total", "UART decode errors").unwrap());

pub static FRAMES_PUBLISHED: LazyLock<IntCounter> = LazyLock::new(|| {
    IntCounter::new("frames_published_total", "Frames published to ZMQ").unwrap()
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
