use anyhow::{Context, Result};
use protocol::{decode_frame, ReplayGuard};
use std::sync::{Arc, Mutex};
use tracing::{info, warn};

use crate::rules::RuleOutcome;
use crate::store::TelemetryStore;

pub fn process_frame(
    frame: protocol::frame::TelemetryFrame,
    replay: &mut ReplayGuard,
    store: &TelemetryStore,
) -> Result<()> {
    if !replay.accept(frame.node_id, frame.seq) {
        warn!(
            node_id = frame.node_id,
            seq = frame.seq,
            "rejected replayed frame"
        );
        return Ok(());
    }

    match crate::rules::evaluate(&frame) {
        RuleOutcome::ActionTriggered => {
            info!(
                "ACTION_TRIGGERED: BoolCmd(true) node={} seq={}",
                frame.node_id, frame.seq
            );
            store.insert(&frame).context("persist telemetry frame")?;
        }
        RuleOutcome::Logged => {
            info!(
                "telemetry node={} seq={} payload={:?}",
                frame.node_id, frame.seq, frame.payload
            );
        }
    }
    Ok(())
}

pub fn run_subscriber_blocking(
    endpoint: String,
    replay: Arc<Mutex<ReplayGuard>>,
    store: Arc<TelemetryStore>,
    max_frames: Option<usize>,
) -> Result<()> {
    let ctx = zmq::Context::new();
    let socket = ctx.socket(zmq::SUB).context("create ZMQ SUB socket")?;
    socket
        .connect(&endpoint)
        .with_context(|| format!("connect {endpoint}"))?;
    socket.set_subscribe(b"").context("subscribe all topics")?;
    info!("ZMQ SUB connected to {endpoint}");

    let mut received = 0usize;
    loop {
        let msg = socket.recv_bytes(0).context("ZMQ recv")?;
        match decode_frame(&msg) {
            Ok(frame) => {
                crate::metrics::FRAMES_DECODED.inc();
                let mut guard = replay.lock().expect("replay mutex poisoned");
                process_frame(frame, &mut guard, &store)?;
            }
            Err(e) => warn!("frame decode error: {}", e),
        }
        received += 1;
        if max_frames.is_some_and(|max| received >= max) {
            break;
        }
    }
    Ok(())
}

pub async fn run_subscriber(
    endpoint: String,
    replay: Arc<Mutex<ReplayGuard>>,
    store: Arc<TelemetryStore>,
) -> Result<()> {
    let endpoint = endpoint.clone();
    tokio::task::spawn_blocking(move || {
        run_subscriber_blocking(endpoint, replay, store, None)
    })
    .await?
}
