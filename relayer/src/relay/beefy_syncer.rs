use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

#[derive(Clone)]
pub struct BeefySyncer {
    latest_requested: Arc<AtomicU64>,
    latest_sent: Arc<AtomicU64>,
}

impl BeefySyncer {
    pub fn new() -> Self {
        Self {
            latest_requested: Default::default(),
            latest_sent: Default::default(),
        }
    }

    pub fn request(&self, block: u64) {
        self.latest_requested
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
                if v < block {
                    debug!("Requesting new BEEFY block {}", block);
                    Some(block)
                } else {
                    None
                }
            })
            .ok();
    }

    pub fn latest_requested(&self) -> u64 {
        self.latest_requested.load(Ordering::Relaxed)
    }

    pub fn latest_sent(&self) -> u64 {
        self.latest_sent.load(Ordering::Relaxed)
    }

    pub fn update_latest_sent(&self, block: u64) {
        self.latest_sent
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
                if v < block {
                    debug!("Updating latest sent BEEFY block to {}", block);
                    Some(block)
                } else {
                    None
                }
            })
            .ok();
    }
}
