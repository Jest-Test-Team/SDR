use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use protocol::ReplayGuard;
use tokio::sync::{RwLock, broadcast};

use crate::sim::{Kpis, PipelineSnapshot, SimConfig, TelemetryEvent};

pub struct AppState {
    pub config: RwLock<SimConfig>,
    pub kpis: RwLock<Kpis>,
    pub events: RwLock<Vec<TelemetryEvent>>,
    pub replay: RwLock<ReplayGuard>,
    pub seq: AtomicU32,
    pub tx: broadcast::Sender<PipelineSnapshot>,
    pub zmq_endpoint: String,
}

pub type SharedState = Arc<AppState>;

impl AppState {
    pub fn new(zmq_endpoint: String) -> SharedState {
        let (tx, _) = broadcast::channel(32);
        Arc::new(Self {
            config: RwLock::new(SimConfig::default()),
            kpis: RwLock::new(Kpis::default()),
            events: RwLock::new(Vec::new()),
            replay: RwLock::new(ReplayGuard::new()),
            seq: AtomicU32::new(0),
            tx,
            zmq_endpoint,
        })
    }

    pub async fn trigger(self: &Arc<Self>, value: bool) -> PipelineSnapshot {
        let config = self.config.read().await.clone();
        let seq = self.seq.fetch_add(1, Ordering::Relaxed) + 1;

        let last_seq = self.replay.read().await.last_seq(config.node_id);
        let mut kpis = self.kpis.write().await;
        let mut snap = crate::sim::run_trigger(&config, seq, value, last_seq, &mut kpis);

        if config.replay_guard {
            let mut guard = self.replay.write().await;
            if !guard.accept(config.node_id, seq) {
                snap.replay_rejected = true;
                snap.packet_ok = false;
                snap.event.status = "重放拒絕".to_string();
                kpis.security_alerts += 1;
                snap.kpis = kpis.clone();
            }
        }

        if snap.packet_ok {
            if let Some(ref frame) = snap.frame {
                if crate::sim::publish_zmq(&self.zmq_endpoint, frame).unwrap_or(false) {
                    snap.zmq_published = true;
                }
            }
        }

        {
            let mut events = self.events.write().await;
            events.insert(0, snap.event.clone());
            events.truncate(50);
        }

        let _ = self.tx.send(snap.clone());
        snap
    }
}
