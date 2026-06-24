use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    Json, Router,
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
    routing::get,
};
use futures::stream::Stream;
use protocol::frame::{Payload, TelemetryFrame};
use serde::Serialize;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;

use crate::rules::RuleOutcome;

#[derive(Clone, Debug, Serialize)]
pub struct LiveEvent {
    pub ts_ms: u64,
    pub level: &'static str,
    pub source: &'static str,
    pub message: String,
    pub node_id: Option<u8>,
    pub seq: Option<u32>,
    pub payload: Option<String>,
}

#[derive(Clone)]
pub struct LiveBus {
    inner: Arc<LiveBusInner>,
}

impl Default for LiveBus {
    fn default() -> Self {
        Self::new(200)
    }
}

struct LiveBusInner {
    events: Mutex<VecDeque<LiveEvent>>,
    tx: tokio::sync::broadcast::Sender<LiveEvent>,
    last_action_seq: Mutex<Option<(u8, u32)>>,
}

impl LiveBus {
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = tokio::sync::broadcast::channel(capacity.max(16));
        Self {
            inner: Arc::new(LiveBusInner {
                events: Mutex::new(VecDeque::with_capacity(capacity)),
                tx,
                last_action_seq: Mutex::new(None),
            }),
        }
    }

    pub fn push(&self, event: LiveEvent) {
        if event.level == "action" {
            if let (Some(node_id), Some(seq)) = (event.node_id, event.seq) {
                *self.inner.last_action_seq.lock().expect("live mutex") = Some((node_id, seq));
            }
        }
        let _ = self.inner.tx.send(event.clone());
        let mut guard = self.inner.events.lock().expect("live mutex");
        guard.push_front(event);
        while guard.len() > 200 {
            guard.pop_back();
        }
    }

    pub fn recent(&self) -> Vec<LiveEvent> {
        self.inner
            .events
            .lock()
            .expect("live mutex")
            .iter()
            .cloned()
            .collect()
    }

    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<LiveEvent> {
        self.inner.tx.subscribe()
    }

    pub fn last_action(&self) -> Option<(u8, u32)> {
        *self.inner.last_action_seq.lock().expect("live mutex")
    }

    pub fn record_frame(
        &self,
        frame: &TelemetryFrame,
        outcome: RuleOutcome,
        replay_rejected: bool,
    ) {
        if replay_rejected {
            self.push(LiveEvent {
                ts_ms: now_ms(),
                level: "warn",
                source: "control_plane",
                message: format!(
                    "rejected replayed frame node_id={} seq={}",
                    frame.node_id, frame.seq
                ),
                node_id: Some(frame.node_id),
                seq: Some(frame.seq),
                payload: Some(payload_label(&frame.payload)),
            });
            return;
        }

        match outcome {
            RuleOutcome::ActionTriggered => {
                self.push(LiveEvent {
                    ts_ms: now_ms(),
                    level: "action",
                    source: "control_plane",
                    message: format!(
                        "ACTION_TRIGGERED: {} node={} seq={}",
                        payload_label(&frame.payload),
                        frame.node_id,
                        frame.seq
                    ),
                    node_id: Some(frame.node_id),
                    seq: Some(frame.seq),
                    payload: Some(payload_label(&frame.payload)),
                });
            }
            RuleOutcome::Logged => {
                self.push(LiveEvent {
                    ts_ms: now_ms(),
                    level: "info",
                    source: "control_plane",
                    message: format!(
                        "telemetry node={} seq={} payload={}",
                        frame.node_id,
                        frame.seq,
                        payload_label(&frame.payload)
                    ),
                    node_id: Some(frame.node_id),
                    seq: Some(frame.seq),
                    payload: Some(payload_label(&frame.payload)),
                });
            }
        }
    }
}

#[derive(Serialize)]
struct LiveStatus {
    frames_decoded: u64,
    events_buffered: usize,
    last_action: Option<ActionRef>,
}

#[derive(Serialize)]
struct ActionRef {
    node_id: u8,
    seq: u32,
}

pub fn router(bus: LiveBus) -> Router {
    Router::new()
        .route("/api/v1/live/events", get(events_handler))
        .route("/api/v1/live/stream", get(stream_handler))
        .route("/api/v1/live/status", get(status_handler))
        .with_state(bus)
}

async fn events_handler(State(bus): State<LiveBus>) -> Json<Vec<LiveEvent>> {
    Json(bus.recent())
}

async fn status_handler(State(bus): State<LiveBus>) -> Json<LiveStatus> {
    let last_action = bus
        .last_action()
        .map(|(node_id, seq)| ActionRef { node_id, seq });
    Json(LiveStatus {
        frames_decoded: crate::metrics::FRAMES_DECODED.get(),
        events_buffered: bus.recent().len(),
        last_action,
    })
}

async fn stream_handler(
    State(bus): State<LiveBus>,
) -> Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>> {
    let initial = bus.recent();
    let rx = bus.subscribe();
    let history = tokio_stream::iter(initial.into_iter().map(|ev| {
        Ok(Event::default()
            .json_data(ev)
            .unwrap_or_else(|_| Event::default()))
    }));
    let live = BroadcastStream::new(rx).map(|msg| {
        let ev = msg.unwrap_or_else(|_| LiveEvent {
            ts_ms: now_ms(),
            level: "info",
            source: "control_plane",
            message: "stream reconnect".into(),
            node_id: None,
            seq: None,
            payload: None,
        });
        Ok(Event::default()
            .json_data(ev)
            .unwrap_or_else(|_| Event::default()))
    });
    Sse::new(history.chain(live)).keep_alive(KeepAlive::default())
}

fn payload_label(payload: &Payload) -> String {
    match payload {
        Payload::BoolCmd(true) => "BoolCmd(true)".into(),
        Payload::BoolCmd(false) => "BoolCmd(false)".into(),
        Payload::ByteCmd(value) => format!("ByteCmd(0x{value:02X})"),
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol::frame::Payload;

    #[test]
    fn records_action_event() {
        let bus = LiveBus::new(16);
        let frame = TelemetryFrame {
            seq: 11,
            timestamp_ms: 0,
            node_id: 1,
            payload: Payload::BoolCmd(true),
        };
        bus.record_frame(&frame, RuleOutcome::ActionTriggered, false);
        let events = bus.recent();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].level, "action");
        assert!(events[0].message.contains("ACTION_TRIGGERED"));
    }
}
