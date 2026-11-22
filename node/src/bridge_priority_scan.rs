// SPDX-License-Identifier: BSD-4-Clause

//! Select individual Ethereum blocks for the existing network-zero deposit scanner.

use codec::{Decode, DecodeAll, Encode};
use sp_core::{offchain::OffchainStorage, ConstU32, H256};
use sp_runtime::{offchain::STORAGE_PREFIX, BoundedVec};

const MAX_BLOCKS: u32 = 256;
const KEY_PREFIX: &[u8] = b"eth-bridge-node::priority-scan-v1/";
// A successful scan may have submitted a proposal that still needs other peers.
// Recheck it occasionally until the operator retires the schedule after finality.
const REVISIT_AFTER_PASSES: u64 = 600;
const MIN_RETRY_PASSES: u64 = 60;

pub(crate) fn is_priority_storage_key(key: &[u8]) -> bool {
    key.starts_with(KEY_PREFIX)
}

#[derive(Debug)]
pub(crate) struct PriorityScanPlan {
    heights: Vec<u64>,
    storage_key: Vec<u8>,
}

#[derive(Debug)]
pub(crate) struct PriorityLease {
    height: u64,
    index: usize,
    storage_key: Vec<u8>,
    reservation: Vec<u8>,
}

impl PriorityLease {
    pub(crate) fn height(&self) -> u64 {
        self.height
    }
}

#[derive(Encode, Decode)]
struct Progress {
    pass: u64,
    next_index: u16,
    history_due: bool,
    blocks: BoundedVec<BlockProgress, ConstU32<MAX_BLOCKS>>,
}

#[derive(Clone, Copy, Default, Encode, Decode)]
struct BlockProgress {
    last_attempt: u64,
    // Zero means the runtime has never advanced the cursor past this height.
    last_scanned: u64,
}

impl PriorityScanPlan {
    pub(crate) fn new(plan_digest: H256, heights: Vec<u64>) -> Result<Self, String> {
        if plan_digest.is_zero()
            || heights.is_empty()
            || heights.len() > MAX_BLOCKS as usize
            || heights.first() == Some(&0)
            || heights.last() == Some(&u64::MAX)
            || heights.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err("Priority scan needs a pinned digest and 1–256 strictly increasing, scannable heights".into());
        }
        let schedule = sp_io::hashing::sha2_256(&(plan_digest, &heights).encode());
        let storage_key = [KEY_PREFIX, &schedule].concat();
        Ok(Self {
            heights,
            storage_key,
        })
    }

    fn progress(&self, raw: Option<&[u8]>) -> Result<Progress, String> {
        match raw {
            Some(mut bytes) => {
                let progress = Progress::decode_all(&mut bytes)
                    .map_err(|_| "Invalid priority scan progress encoding")?;
                if progress.pass == 0
                    || usize::from(progress.next_index) >= self.heights.len()
                    || progress.blocks.len() != self.heights.len()
                    || progress.blocks.iter().any(|block| {
                        block.last_scanned > block.last_attempt
                            || block.last_attempt > progress.pass
                    })
                {
                    return Err("Invalid priority scan progress for this schedule".into());
                }
                Ok(progress)
            }
            None => Ok(Progress {
                pass: 0,
                next_index: 0,
                history_due: false,
                blocks: vec![BlockProgress::default(); self.heights.len()]
                    .try_into()
                    .map_err(|_| "Too many priority heights")?,
            }),
        }
    }

    #[cfg(test)]
    pub(crate) fn progress_key(&self) -> &[u8] {
        &self.storage_key
    }

    /// Called once per acquired worker lock. Reserve the next turn before exposing
    /// a virtual cursor so a crash leaves a turn for history and later targets.
    pub(crate) fn begin(
        &self,
        db: &mut impl OffchainStorage,
        history_cursor: u64,
    ) -> Result<Option<PriorityLease>, String> {
        let raw = db.get(STORAGE_PREFIX, &self.storage_key);
        let mut progress = self.progress(raw.as_deref())?;
        if !progress.history_due && self.heights.as_slice() == [history_cursor] {
            return Ok(None);
        }
        progress.pass = progress
            .pass
            .checked_add(1)
            .ok_or("Priority scan pass counter exhausted")?;

        // Each block gets an initial turn. Even if every block keeps failing, revisits
        // can use at most a quarter of passes over a full retry window thereafter.
        let retry_after = MIN_RETRY_PASSES.max(self.heights.len() as u64 * 4);
        let selected = if progress.history_due {
            None
        } else {
            let start = usize::from(progress.next_index);
            (start..self.heights.len()).chain(0..start).find(|&index| {
                let block = &progress.blocks[index];
                let delay = if block.last_scanned == block.last_attempt {
                    REVISIT_AFTER_PASSES.max(retry_after)
                } else {
                    retry_after
                };
                // The ordinary scanner already starts at the next chronological block.
                // Explicitly listed older blocks may still have uncredited deposits.
                self.heights[index] != history_cursor
                    && (block.last_attempt == 0 || progress.pass - block.last_attempt >= delay)
            })
        };
        progress.history_due = selected.is_some();
        if let Some(index) = selected {
            progress.next_index = ((index + 1) % self.heights.len()) as u16;
            progress.blocks[index].last_attempt = progress.pass;
        }
        let reservation = progress.encode();
        if !db.compare_and_set(
            STORAGE_PREFIX,
            &self.storage_key,
            raw.as_deref(),
            &reservation,
        ) {
            return Err("Priority scan reservation changed before acquiring it".into());
        }
        Ok(selected.map(|index| PriorityLease {
            height: self.heights[index],
            index,
            storage_key: self.storage_key.clone(),
            reservation,
        }))
    }

    /// The caller observed this runtime cursor write and normal worker lock release.
    /// The recorded height is evidence of a scan, independent of quorum or finality.
    pub(crate) fn complete(
        &self,
        db: &mut impl OffchainStorage,
        lease: &PriorityLease,
        next_height: u64,
    ) -> Result<(), String> {
        if self.storage_key != lease.storage_key
            || self.heights.get(lease.index) != Some(&lease.height)
            || next_height != lease.height + 1
        {
            return Err("Priority scan did not advance exactly one reserved block".into());
        }
        let mut progress = self.progress(Some(&lease.reservation))?;
        let first_scan = progress.blocks[lease.index].last_scanned == 0;
        progress.blocks[lease.index].last_scanned = progress.pass;
        let completed = progress.encode();
        if db.compare_and_set(
            STORAGE_PREFIX,
            &self.storage_key,
            Some(&lease.reservation),
            &completed,
        ) {
            let count = progress
                .blocks
                .iter()
                .filter(|block| block.last_scanned != 0)
                .count();
            log::info!(
                "Ethereum priority scan committed height={} completed={}/{}",
                lease.height,
                count,
                self.heights.len()
            );
            if first_scan && count == self.heights.len() {
                log::info!("Deposit scanner has reviewed all {} priority blocks at least once; confirmation and quorum are tracked on chain", self.heights.len());
            }
            Ok(())
        } else if db.get(STORAGE_PREFIX, &self.storage_key).as_deref() == Some(completed.as_slice())
        {
            Ok(())
        } else {
            Err("Priority scan completion belongs to a superseded reservation".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sp_core::offchain::testing::TestPersistentOffchainDB;

    fn plan(heights: &[u64]) -> PriorityScanPlan {
        PriorityScanPlan::new(H256::repeat_byte(1), heights.to_vec()).unwrap()
    }

    #[test]
    fn validates_bounds_and_binds_state_to_document_and_heights() {
        for heights in [
            vec![],
            vec![0],
            vec![u64::MAX],
            vec![3, 3],
            vec![7, 5],
            (1..=257).collect(),
        ] {
            assert!(PriorityScanPlan::new(H256::repeat_byte(9), heights).is_err());
        }
        assert!(PriorityScanPlan::new(H256::zero(), vec![3]).is_err());
        assert!(PriorityScanPlan::new(H256::repeat_byte(9), (1..=256).collect()).is_ok());
        let first = plan(&[10]);
        assert!(is_priority_storage_key(&first.storage_key));
        assert!(!is_priority_storage_key(
            b"eth-bridge-ocw::eth-to-handle-from-height-0"
        ));
        let mut db = TestPersistentOffchainDB::new();
        let lease = first.begin(&mut db, 2).unwrap().unwrap();
        for other in [
            plan(&[10, 20]),
            PriorityScanPlan::new(H256::repeat_byte(2), vec![10]).unwrap(),
        ] {
            assert_ne!(first.storage_key, other.storage_key);
            assert!(other.complete(&mut db, &lease, 11).is_err());
            assert_eq!(other.begin(&mut db, 2).unwrap().unwrap().height(), 10);
        }
    }

    #[test]
    fn failed_or_trapped_block_retries_after_restart_without_starving_history_or_later_blocks() {
        let heights = [100, 140];
        let mut db = TestPersistentOffchainDB::new();
        let cursor_key = framenode_runtime::eth_bridge::eth_scan_cursor_key(0);
        db.set(STORAGE_PREFIX, &cursor_key, &17u64.encode());
        let first = plan(&heights);
        let lease = first.begin(&mut db, 17).unwrap().unwrap();
        assert_eq!(lease.height(), 100);
        for invalid in [0, 100, 102, u64::MAX] {
            assert!(first.complete(&mut db, &lease, invalid).is_err());
        }
        let restarted = plan(&heights);
        let mut persistent = db.clone();
        assert!(restarted.begin(&mut persistent, 17).unwrap().is_none());
        let second = restarted.begin(&mut persistent, 17).unwrap().unwrap();
        assert_eq!(second.height(), 140);
        assert!(first.complete(&mut db, &lease, 101).is_err());
        restarted.complete(&mut persistent, &second, 141).unwrap();
        restarted.complete(&mut persistent, &second, 141).unwrap();
        for _ in 3..MIN_RETRY_PASSES {
            assert!(restarted.begin(&mut persistent, 17).unwrap().is_none());
        }
        let retry = restarted.begin(&mut persistent, 17).unwrap().unwrap();
        assert_eq!(retry.height(), 100);
        assert_eq!(
            OffchainStorage::get(&db, STORAGE_PREFIX, &cursor_key),
            Some(17u64.encode())
        );
    }

    #[test]
    fn revisits_an_older_target_without_moving_history_and_leaves_its_next_block_to_history() {
        let mut db = TestPersistentOffchainDB::new();
        let cursor_key = framenode_runtime::eth_bridge::eth_scan_cursor_key(0);
        db.set(STORAGE_PREFIX, &cursor_key, &120u64.encode());
        let plan = plan(&[90, 120, 140]);
        let historical = plan.begin(&mut db, 120).unwrap().unwrap();
        assert_eq!(historical.height(), 90);
        plan.complete(&mut db, &historical, 91).unwrap();
        assert!(plan.begin(&mut db, 120).unwrap().is_none());
        let next = plan.begin(&mut db, 120).unwrap().unwrap();
        assert_eq!(next.height(), 140);
        assert!(plan.complete(&mut db, &historical, 91).is_err());
        assert_eq!(
            OffchainStorage::get(&db, STORAGE_PREFIX, &cursor_key),
            Some(120u64.encode())
        );
    }

    #[test]
    fn plan_containing_only_the_current_chronological_height_needs_no_reservation() {
        let mut db = TestPersistentOffchainDB::new();
        let plan = plan(&[120]);
        assert!(plan.begin(&mut db, 120).unwrap().is_none());
        assert!(OffchainStorage::get(&db, STORAGE_PREFIX, &plan.storage_key).is_none());
        assert_eq!(plan.begin(&mut db, 170).unwrap().unwrap().height(), 120);
    }

    #[test]
    fn rechecks_previously_scanned_proposals_while_quorum_may_be_outstanding() {
        let plan = plan(&[100]);
        let mut db = TestPersistentOffchainDB::new();
        let first = plan.begin(&mut db, 1).unwrap().unwrap();
        plan.complete(&mut db, &first, 101).unwrap();
        for _ in 1..REVISIT_AFTER_PASSES {
            assert!(plan.begin(&mut db, 1).unwrap().is_none());
        }
        let revisit = plan.begin(&mut db, 1).unwrap().unwrap();
        assert_eq!(revisit.height(), 100);
        assert!(plan.complete(&mut db, &first, 101).is_err());
    }

    #[test]
    fn rejects_corrupt_or_trailing_state_and_preserves_it() {
        let mut db = TestPersistentOffchainDB::new();
        let plan = plan(&[100]);
        let mut valid = plan.begin(&mut db, 1).unwrap().unwrap().reservation;
        valid.push(1);
        for raw in [vec![255], valid] {
            db.set(STORAGE_PREFIX, &plan.storage_key, &raw);
            assert!(plan.begin(&mut db, 1).is_err());
            assert_eq!(
                OffchainStorage::get(&db, STORAGE_PREFIX, &plan.storage_key),
                Some(raw)
            );
        }
    }
}
