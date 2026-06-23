use anyhow::{Context, Result};
use protocol::frame::TelemetryFrame;
use std::path::Path;

pub struct TelemetryStore {
    db: sled::Db,
}

impl TelemetryStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let db = sled::open(path).context("open sled database")?;
        Ok(Self { db })
    }

    pub fn insert(&self, frame: &TelemetryFrame) -> Result<()> {
        let key = format!("{}:{}", frame.node_id, frame.seq);
        let value = postcard::to_allocvec(frame).context("serialize frame")?;
        self.db.insert(key.as_bytes(), value).context("sled insert")?;
        self.db.flush().ok();
        Ok(())
    }

    pub fn contains_action(&self, node_id: u8, seq: u32) -> bool {
        let key = format!("{node_id}:{seq}");
        self.db.contains_key(key.as_bytes()).unwrap_or(false)
    }
}
