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
#[cfg_attr(test, derive(Clone))]
pub(crate) struct PriorityScanPlan {
    heights: Vec<u64>,
    storage_key: Vec<u8>,
    follow: Option<ContinuousScan>,
}

#[derive(Debug)]
pub(crate) struct PriorityLease {
    height: u64,
    index: usize,
    storage_key: Vec<u8>,
    reservation: Vec<u8>,
    // Zero denotes an individual block in the finite schedule.
    continuous_span: u64,
    range_limit: Option<(u64, u64)>,
}

impl PriorityLease {
    pub(crate) fn height(&self) -> u64 {
        self.height
    }

    pub(crate) fn span(&self) -> u64 {
        self.continuous_span.max(1)
    }

    pub(crate) fn accepts(&self, next: u64) -> bool {
        // The pinned Wasm may issue a doubled probe once its scoped provider
        // cooldown expires. The normal read still sees the original range size.
        let max_span = self.range_limit.map_or(self.span(), |(ceiling, _)| {
            self.span().max(self.span().min(ceiling) * 2)
        });
        next > self.height && next - self.height <= max_span
    }

    pub(crate) fn range_limit(&self) -> Option<(u64, u64)> {
        self.range_limit
    }

    pub(crate) fn continuous(&self) -> bool {
        self.continuous_span != 0
    }
}

const CONTINUOUS_RANGE: u64 = 16;

// Spec 142 halves the next range after a provider rejection, but learns
// queried_span - 1 as its ceiling. An expired ceiling of 15 can therefore probe 30.
pub(crate) fn valid_continuous_ceiling(ceiling: u64) -> bool {
    (1..CONTINUOUS_RANGE).contains(&ceiling)
}

#[derive(Debug)]
#[cfg_attr(test, derive(Clone))]
struct ContinuousScan {
    start: u64,
    key: Vec<u8>,
}

#[derive(Encode, Decode)]
struct ContinuousProgress {
    turn: u64,
    next: u64,
    range: u64,
    limit: Option<(u64, u64)>,
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
    #[cfg(test)]
    pub(crate) fn new(plan_digest: H256, heights: Vec<u64>) -> Result<Self, String> {
        Self::from_manifest(plan_digest, heights, None)
    }

    pub(crate) fn from_manifest(
        plan_digest: H256,
        heights: Vec<u64>,
        reconciled: Option<u64>,
    ) -> Result<Self, String> {
        if plan_digest.is_zero()
            || (heights.is_empty() && reconciled.is_none())
            || heights.len() > MAX_BLOCKS as usize
            || heights.first() == Some(&0)
            || heights.last() == Some(&u64::MAX)
            || heights.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err("Priority scan needs a pinned digest and up to 256 strictly increasing, scannable heights; an empty list requires a continuation".into());
        }
        let schedule = sp_io::hashing::sha2_256(&(plan_digest, &heights).encode());
        let storage_key = [KEY_PREFIX, &schedule].concat();
        let plan = Self {
            heights,
            storage_key,
            follow: None,
        };
        match reconciled {
            Some(height) => plan.follow_after(height),
            None => Ok(plan),
        }
    }

    #[cfg(test)]
    pub(crate) fn continuation_height(&self, db: &impl OffchainStorage) -> Option<u64> {
        let follow = self.follow.as_ref()?;
        let raw = db.get(STORAGE_PREFIX, &follow.key)?;
        Some(ContinuousProgress::decode_all(&mut &raw[..]).ok()?.next)
    }

    pub(crate) fn follow_after(mut self, reconciled: u64) -> Result<Self, String> {
        let start = reconciled
            .checked_add(1)
            .filter(|next| *next < u64::MAX)
            .ok_or("Invalid continuous deposit scan boundary")?;
        let key = [
            &self.storage_key[..],
            b"/continuous-v1/",
            &start.to_le_bytes(),
        ]
        .concat();
        self.follow = Some(ContinuousScan { start, key });
        Ok(self)
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
        if let Some(follow) = &self.follow {
            let raw = db.get(STORAGE_PREFIX, &follow.key);
            let mut progress = match &raw {
                Some(bytes) => ContinuousProgress::decode_all(&mut bytes.as_slice())
                    .map_err(|_| "Invalid continuous deposit progress encoding")?,
                None => ContinuousProgress {
                    turn: 0,
                    next: follow.start,
                    range: CONTINUOUS_RANGE,
                    limit: None,
                },
            };
            if (raw.is_some() && progress.turn == 0)
                || progress.next < follow.start
                || !(1..=CONTINUOUS_RANGE).contains(&progress.range)
                || progress
                    .limit
                    .is_some_and(|(cap, _)| !valid_continuous_ceiling(cap))
            {
                return Err("Invalid continuous deposit cursor or range".into());
            }
            progress.turn = progress
                .turn
                .checked_add(1)
                .ok_or("Continuous scan turns exhausted")?;
            let reservation = progress.encode();
            if !db.compare_and_set(STORAGE_PREFIX, &follow.key, raw.as_deref(), &reservation) {
                return Err("Continuous deposit reservation changed".into());
            }
            // Always give the original scanner every other turn, including after
            // crashes. The finite schedule and continuous scanner receive one
            // guaranteed opportunity each in every four successful acquisitions.
            if progress.turn % 2 == 0 {
                return Ok(None);
            }
            if progress.turn % 4 == 1 && !self.heights.is_empty() {
                if let Some(point) = self.begin_points(db, history_cursor)? {
                    return Ok(Some(point));
                }
            }
            // A primary cursor that reaches this position will handle it normally.
            // Retain the independent cursor so overlapping coverage can still be replayed.
            let span = progress.range.min(u64::MAX - progress.next);
            if span == 0 || progress.next == history_cursor {
                return Ok(None);
            }
            return Ok(Some(PriorityLease {
                height: progress.next,
                index: 0,
                storage_key: follow.key.clone(),
                reservation,
                continuous_span: span,
                range_limit: progress.limit,
            }));
        }
        self.begin_points(db, history_cursor)
    }

    fn begin_points(
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
        // The continuous scheduler already reserved every other pass for history.
        progress.history_due = self.follow.is_none() && selected.is_some();
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
            continuous_span: 0,
            range_limit: None,
        }))
    }

    pub(crate) fn finish_continuous(
        &self,
        db: &mut impl OffchainStorage,
        lease: &PriorityLease,
        next: Option<u64>,
        range: Option<u64>,
        limit: Option<Option<(u64, u64)>>,
    ) -> Result<(), String> {
        let follow = self
            .follow
            .as_ref()
            .ok_or("No continuous scan configured")?;
        if lease.storage_key != follow.key
            || !lease.continuous()
            || next.is_some_and(|n| !lease.accepts(n))
            || limit
                .flatten()
                .is_some_and(|(cap, _)| !valid_continuous_ceiling(cap))
        {
            return Err("Invalid continuous scan completion".into());
        }
        let mut progress = ContinuousProgress::decode_all(&mut lease.reservation.as_slice())
            .map_err(|_| "Invalid continuous scan reservation")?;
        if progress.next != lease.height {
            return Err("Continuous scan lease starts at a different cursor".into());
        }
        if let Some(n) = next {
            progress.next = n;
        }
        if let Some(r) = range {
            progress.range = r.clamp(1, CONTINUOUS_RANGE);
        }
        if let Some(new_limit) = limit {
            progress.limit = new_limit;
        }
        let updated = progress.encode();
        if !db.compare_and_set(
            STORAGE_PREFIX,
            &follow.key,
            Some(&lease.reservation),
            &updated,
        ) {
            return Err("Continuous scan completion belongs to a superseded reservation".into());
        }
        Ok(())
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
    #[test]
    fn continuous_resumes_without_gaps_after_crash_or_partial_scan_and_keeps_history_fair() {
        let plan = || plan(&[9]).follow_after(100).unwrap();
        let mut db = TestPersistentOffchainDB::new();
        let old = plan();
        // First finite priority slot, chronological slot, uninterrupted continuation slot.
        let point = old.begin(&mut db, 1).unwrap().unwrap();
        assert!(!point.continuous());
        assert!(old.begin(&mut db, 1).unwrap().is_none());
        let crashed = old.begin(&mut db, 1).unwrap().unwrap();
        assert_eq!((crashed.height(), crashed.span()), (101, CONTINUOUS_RANGE));
        drop(old);
        let restarted = plan();
        assert!(restarted.begin(&mut db, 1).unwrap().is_none());
        let retry = restarted.begin(&mut db, 1).unwrap().unwrap();
        assert_eq!(retry.height(), 101);
        assert!(restarted
            .finish_continuous(&mut db, &crashed, Some(103), None, None)
            .is_err());
        assert!(restarted
            .finish_continuous(&mut db, &retry, Some(118), None, None)
            .is_err());
        assert!(restarted
            .finish_continuous(
                &mut db,
                &retry,
                None,
                None,
                Some(Some((CONTINUOUS_RANGE, u64::MAX)))
            )
            .is_err());
        restarted
            .finish_continuous(&mut db, &retry, Some(104), None, None)
            .unwrap();
        assert!(restarted.begin(&mut db, 1).unwrap().is_none());
        let next = restarted.begin(&mut db, 1).unwrap().unwrap();
        assert_eq!(next.height(), 104);
        restarted
            .finish_continuous(&mut db, &next, None, Some(1), None)
            .unwrap();
        assert!(restarted.begin(&mut db, 1).unwrap().is_none());
        let smaller = restarted.begin(&mut db, 1).unwrap().unwrap();
        assert_eq!((smaller.height(), smaller.span()), (104, 1));
    }

    #[test]
    fn continuation_capacity_exceeds_tip_growth_with_history_and_every_point_slot_busy() {
        let points = (1..=256).collect::<Vec<_>>();
        let plan = plan(&points).follow_after(1000).unwrap();
        let mut db = TestPersistentOffchainDB::new();
        let (mut history, mut point, mut continuation) = (0, 0, 0);
        let mut next = 1001;
        let mut tip = 1101;
        for turn in 1..=200 {
            tip += 1; // stress: one new confirmed Ethereum block per native pass
            match plan.begin(&mut db, 500).unwrap() {
                None => history += 1,
                Some(lease) if lease.continuous() => {
                    continuation += 1;
                    assert_eq!(lease.height(), next);
                    next = (next + lease.span()).min(tip + 1);
                    plan.finish_continuous(&mut db, &lease, Some(next), None, None)
                        .unwrap();
                    if turn > 50 {
                        assert_eq!(next, tip + 1);
                    }
                }
                Some(_) => point += 1,
            }
        }
        assert_eq!((history, point, continuation), (100, 50, 50));
        assert!(tip + 1 - next <= 3);
    }
}
