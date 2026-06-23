#![no_std]

extern crate alloc;

use alloc::collections::BTreeMap;

/// Tracks last accepted sequence number per node to reject replayed frames.
#[derive(Debug, Default)]
pub struct ReplayGuard {
    last_seq: BTreeMap<u8, u32>,
}

impl ReplayGuard {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true if `seq` is strictly greater than the last seen seq for `node_id`.
    pub fn accept(&mut self, node_id: u8, seq: u32) -> bool {
        match self.last_seq.get(&node_id) {
            Some(&last) if seq <= last => false,
            _ => {
                self.last_seq.insert(node_id, seq);
                true
            }
        }
    }

    pub fn last_seq(&self, node_id: u8) -> Option<u32> {
        self.last_seq.get(&node_id).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_increasing_seq() {
        let mut guard = ReplayGuard::new();
        assert!(guard.accept(1, 1));
        assert!(guard.accept(1, 2));
        assert_eq!(guard.last_seq(1), Some(2));
    }

    #[test]
    fn rejects_replay() {
        let mut guard = ReplayGuard::new();
        assert!(guard.accept(1, 2));
        assert!(!guard.accept(1, 2));
        assert!(!guard.accept(1, 1));
    }

    #[test]
    fn independent_per_node() {
        let mut guard = ReplayGuard::new();
        assert!(guard.accept(1, 5));
        assert!(guard.accept(2, 1));
        assert!(!guard.accept(1, 5));
        assert!(guard.accept(2, 2));
    }
}
