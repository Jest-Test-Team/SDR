use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use protocol::ReplayGuard;
use tokio::sync::{RwLock, broadcast};

use crate::gwbackend::{GatewayBackend, GatewayStatus};
use crate::sim::{
    GatewayCommand, GatewayResponse, Kpis, PipelineSnapshot, SimConfig, TelemetryEvent,
};

#[derive(Clone, Debug)]
pub struct SecureIngestConfig {
    pub url: String,
    pub client_cert: String,
    pub client_key: String,
    pub server_ca: String,
}

pub struct AppState {
    pub config: RwLock<SimConfig>,
    pub kpis: RwLock<Kpis>,
    pub events: RwLock<Vec<TelemetryEvent>>,
    pub replay: RwLock<ReplayGuard>,
    pub seq: AtomicU32,
    pub tx: broadcast::Sender<PipelineSnapshot>,
    pub zmq_endpoint: String,
    pub secure_ingest: Option<SecureIngestConfig>,
    pub gateway: GatewayBackend,
}

pub type SharedState = Arc<AppState>;

impl AppState {
    pub fn new(zmq_endpoint: String, secure_ingest: Option<SecureIngestConfig>) -> SharedState {
        Self::with_gateway(zmq_endpoint, secure_ingest, GatewayBackend::simulation())
    }

    pub fn with_gateway(
        zmq_endpoint: String,
        secure_ingest: Option<SecureIngestConfig>,
        gateway: GatewayBackend,
    ) -> SharedState {
        let (tx, _) = broadcast::channel(32);
        Arc::new(Self {
            config: RwLock::new(SimConfig::default()),
            kpis: RwLock::new(Kpis::default()),
            events: RwLock::new(Vec::new()),
            replay: RwLock::new(ReplayGuard::new()),
            seq: AtomicU32::new(0),
            tx,
            zmq_endpoint,
            secure_ingest,
            gateway,
        })
    }

    pub async fn gateway_snapshot(&self) -> crate::sim::GatewaySnapshot {
        self.gateway.snapshot().await
    }

    pub async fn apply_gateway_command(&self, command: &GatewayCommand) -> GatewayResponse {
        self.gateway.apply(command).await
    }

    pub fn gateway_status(&self) -> GatewayStatus {
        self.gateway.status()
    }

    pub fn sidecar_transport(&self) -> &'static str {
        if self.secure_ingest.is_some() {
            "tls13_mtls"
        } else {
            "zmq"
        }
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
                let published = if let Some(config) = &self.secure_ingest {
                    crate::sim::publish_secure_ingest(config, frame)
                        .await
                        .unwrap_or(false)
                } else {
                    crate::sim::publish_zmq(&self.zmq_endpoint, frame).unwrap_or(false)
                };
                if published {
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
