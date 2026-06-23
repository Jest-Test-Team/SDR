use anyhow::{Context, Result};
use std::sync::{
    atomic::{AtomicU64, Ordering},
};
use tokio::sync::mpsc;
use tracing::info;

static FRAMES_PUBLISHED: AtomicU64 = AtomicU64::new(0);

pub fn frames_published() -> u64 {
    FRAMES_PUBLISHED.load(Ordering::Relaxed)
}

pub async fn run_publisher(endpoint: String, mut rx: mpsc::Receiver<Vec<u8>>) -> Result<()> {
    let endpoint = endpoint.clone();
    tokio::task::spawn_blocking(move || -> Result<()> {
        let ctx = zmq::Context::new();
        let socket = ctx.socket(zmq::PUB).context("create ZMQ PUB socket")?;
        socket.bind(&endpoint).with_context(|| format!("bind {endpoint}"))?;
        info!("ZMQ PUB bound to {endpoint}");

        while let Some(frame) = rx.blocking_recv() {
            socket.send(&frame, 0).context("ZMQ send")?;
            FRAMES_PUBLISHED.fetch_add(1, Ordering::Relaxed);
        }
        Ok(())
    })
    .await??;
    Ok(())
}

pub fn spawn_publisher(
    endpoint: String,
) -> (mpsc::Sender<Vec<u8>>, tokio::task::JoinHandle<Result<()>>) {
    let (tx, rx) = mpsc::channel(64);
    let handle = tokio::spawn(async move { run_publisher(endpoint, rx).await });
    (tx, handle)
}
