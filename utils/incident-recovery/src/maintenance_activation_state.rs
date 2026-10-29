//! Bounded, read-only decoding of the runtime's actual post-recovery staking state.
use super::{replay, required, storage_key, BlockWitness, Result};
use codec::{Decode, Encode};
use serde_json::{json, Value};
use sha2::{Digest as _, Sha256};
use sp_core::H256;
use std::collections::{BTreeMap, BTreeSet};

const MAX_VALIDATORS: usize = 1200;
const MAX_RECENCY: usize = 100_000;
pub(super) const MAX_ROWS: usize = 10_000;
pub(super) const MAX_REPORT_BYTES: usize = 128 * 1024 * 1024;
type Account = [u8; 32];
// Runtime SessionKeys: BABE, GRANDPA, im-online and compressed ECDSA BEEFY.
type SessionKey = ([u8; 32], [u8; 32], [u8; 32], [u8; 33]);

fn need(ok: bool, message: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(message.into())
    }
}

#[derive(Clone)]
pub(super) struct Core {
    pub slot: u64,
    pub epoch: u64,
    pub genesis: u64,
    pub session: u32,
    pub current: u32,
    pub active: u32,
    pub start: Option<u64>,
}
impl Core {
    pub fn read(ext: &mut sp_io::TestExternalities) -> Result<Self> {
        let (active, start): (u32, Option<u64>) = required(ext, b"Staking", b"ActiveEra")?;
        let state = Self {
            slot: required(ext, b"Babe", b"CurrentSlot")?,
            epoch: required(ext, b"Babe", b"EpochIndex")?,
            genesis: required(ext, b"Babe", b"GenesisSlot")?,
            session: required(ext, b"Session", b"CurrentIndex")?,
            current: required(ext, b"Staking", b"CurrentEra")?,
            active,
            start,
        };
        need(
            state.slot >= state.genesis
                && (state.current == active || active.checked_add(1) == Some(state.current)),
            "activation invalid slot or active/planned era relationship",
        )?;
        Ok(state)
    }
}

pub(crate) struct Maintenance {
    checkpoint: Core,
    previous: Core,
    first: Option<Core>,
    blocks: usize,
    maximum_gap: u64,
    last_hash: H256,
}
impl Maintenance {
    pub fn checkpoint(ext: &mut sp_io::TestExternalities) -> Result<Self> {
        let root = replay::root(ext);
        let state = Core::read(ext)?;
        need(
            replay::root(ext) == root,
            "activation checkpoint observer changed state",
        )?;
        Ok(Self {
            checkpoint: state.clone(),
            previous: state,
            first: None,
            blocks: 0,
            maximum_gap: 0,
            last_hash: H256::from(common::recovery::CHECKPOINT_HASH),
        })
    }
    pub fn accepted(
        &mut self,
        ext: &mut sp_io::TestExternalities,
        witness: &BlockWitness,
        slot: u64,
    ) -> Result<()> {
        let root = replay::root(ext);
        let state = Core::read(ext)?;
        let expected_number = common::recovery::CHECKPOINT_NUMBER
            .checked_add(self.blocks as u32 + 1)
            .ok_or("activation maintenance number overflow")?;
        need(
            witness.number == expected_number
                && witness.maintenance_session_frozen
                && witness.independent_import_root_matches
                && witness.disk_restart_root_matches
                && witness.state_root == format!("{root:#x}")
                && state.slot == slot
                && state.slot > self.previous.slot
                && state.epoch >= self.previous.epoch
                && state.genesis == self.checkpoint.genesis
                && state.session == self.checkpoint.session
                && state.current == self.checkpoint.current
                && state.active == self.checkpoint.active
                && state.start == self.checkpoint.start,
            "activation maintenance clock/session/era witness differs",
        )?;
        self.maximum_gap = self.maximum_gap.max(state.slot - self.previous.slot);
        self.first.get_or_insert_with(|| state.clone());
        self.previous = state;
        self.blocks += 1;
        self.last_hash = witness
            .hash
            .parse()
            .map_err(|_| "activation maintenance hash encoding")?;
        need(
            replay::root(ext) == root,
            "activation maintenance observer changed state",
        )
    }
    pub(super) fn parent(&self, parent: H256, state: &Core) -> Result<()> {
        need(
            self.blocks > 0
                && self.last_hash == parent
                && state.slot == self.previous.slot
                && state.epoch == self.previous.epoch
                && state.genesis == self.previous.genesis
                && state.session == self.previous.session
                && state.current == self.previous.current
                && state.active == self.previous.active
                && state.start == self.previous.start,
            "activation parent is not the observed completed maintenance state",
        )
    }
    pub fn report(&self) -> Result<Value> {
        let first = self
            .first
            .as_ref()
            .ok_or("activation maintenance observation absent")?;
        Ok(
            json!({"checkpointSlot":self.checkpoint.slot,"checkpointEpoch":self.checkpoint.epoch,
            "checkpointSession":self.checkpoint.session,"checkpointActiveEra":self.checkpoint.active,
            "checkpointCurrentEra":self.checkpoint.current,"firstRecoverySlot":first.slot,
            "firstRecoveryEpoch":first.epoch,"lastSlot":self.previous.slot,"lastEpoch":self.previous.epoch,
            "blocks":self.blocks,"maximumSlotGap":self.maximum_gap}),
        )
    }
}

#[derive(Clone, Debug, Default, Decode, Encode)]
struct AsyncState {
    pending: Option<(Vec<Account>, u32, Option<u32>)>,
    queued_era: Option<(u32, u32)>,
    election_deadline: Option<u32>,
    forced: bool,
    timestamp_era: Option<u32>,
}

fn validators(accounts: &[Account]) -> Result<()> {
    need(
        !accounts.is_empty()
            && accounts.len() <= MAX_VALIDATORS
            && accounts.iter().collect::<BTreeSet<_>>().len() == accounts.len(),
        "activation validator set is empty, duplicated or oversized",
    )
}

fn recency(
    ext: &mut sp_io::TestExternalities,
    current: u32,
) -> Result<(BTreeMap<Account, u32>, String)> {
    let prefix = storage_key(b"Staking", b"LastValidatorEra");
    ext.execute_with(|| {
        need(
            sp_io::storage::get(&prefix).is_none(),
            "activation malformed recency prefix value",
        )?;
        let mut previous = prefix.clone();
        let mut map = BTreeMap::new();
        let mut hash = Sha256::new();
        while let Some(key) = sp_io::storage::next_key(&previous) {
            if !key.starts_with(&prefix) {
                break;
            }
            need(
                key > previous && key.len() == prefix.len() + 40 && map.len() < MAX_RECENCY,
                "activation LastValidatorEra key/order/count differs",
            )?;
            let account: Account = key[prefix.len() + 8..]
                .try_into()
                .map_err(|_| "activation recency owner width")?;
            need(
                key[prefix.len()..prefix.len() + 8] == sp_io::hashing::twox_64(&account),
                "activation LastValidatorEra owner hash differs",
            )?;
            let raw = sp_io::storage::get(&key).ok_or("activation recency row disappeared")?;
            need(raw.len() == 4, "activation recency era width")?;
            let era = u32::from_le_bytes(raw[..].try_into().map_err(|_| "activation recency era")?);
            need(
                era <= current && map.insert(account, era).is_none(),
                "activation future or duplicate recency",
            )?;
            hash.update((key.len() as u64).to_le_bytes());
            hash.update(&key);
            hash.update((raw.len() as u64).to_le_bytes());
            hash.update(&raw);
            previous = key;
        }
        Ok((map, hex::encode(hash.finalize())))
    })
}

fn recency_rows(
    map: &BTreeMap<Account, u32>,
    accounts: &[Account],
    min: u32,
    max: u32,
) -> Result<Vec<Value>> {
    accounts
        .iter()
        .map(|account| {
            let era = *map
                .get(account)
                .ok_or("activation active/queued validator recency missing")?;
            need(
                era >= min && era <= max,
                "activation active/queued validator recency differs",
            )?;
            Ok(json!({"account":format!("0x{}",hex::encode(account)),"era":era}))
        })
        .collect()
}

pub(super) struct State {
    pub core: Core,
    pub timestamp: u64,
    pub async_report: Value,
    pub recency_report: Value,
}
impl State {
    pub fn read(ext: &mut sp_io::TestExternalities, finalized: bool) -> Result<Self> {
        let core = Core::read(ext)?;
        let timestamp: u64 = required(ext, b"Timestamp", b"Now")?;
        let state: AsyncState = required(ext, b"StakingAsyncBridge", b"State")?;
        let active: Vec<Account> = required(ext, b"Session", b"Validators")?;
        let queued: Vec<(Account, SessionKey)> = required(ext, b"Session", b"QueuedKeys")?;
        let queued: Vec<_> = queued.into_iter().map(|(owner, _)| owner).collect();
        validators(&active)?;
        validators(&queued)?;
        need(
            !(state.pending.is_some() && state.queued_era.is_some())
                && state
                    .timestamp_era
                    .is_none_or(|era| !finalized && era == core.active)
                && (!finalized || core.start.is_some_and(|start| start <= timestamp)),
            "activation async pending/queued/timestamp state differs",
        )?;
        let next = core.active.checked_add(1);
        if let Some((accounts, era, _)) = &state.pending {
            validators(accounts)?;
            need(
                next == Some(*era) && *era == core.current,
                "activation pending election era differs",
            )?;
        }
        if let Some((session, era)) = state.queued_era {
            need(
                core.session.checked_add(1) == Some(session)
                    && next == Some(era)
                    && core.current == era
                    && state.election_deadline.is_none(),
                "activation queued era/session/deadline differs",
            )?;
        }
        // election_deadline may be expired while an on_poll retry is pending,
        // and may remain set until new_session consumes pending validators.
        let (map, digest) = recency(ext, core.current)?;
        let active_rows = recency_rows(&map, &active, core.active, core.current)?;
        let queued_rows = if let Some((_, era)) = state.queued_era {
            recency_rows(&map, &queued, era, era)?
        } else {
            Vec::new()
        };
        let pending_rows = if let Some((accounts, era, _)) = &state.pending {
            recency_rows(&map, accounts, *era, *era)?
        } else {
            Vec::new()
        };
        Ok(Self {
            core,
            timestamp,
            async_report: json!({"pendingValidators":state.pending.as_ref().map_or(0,|(v,_,_)|v.len()),
                "pendingEra":state.pending.as_ref().map(|(_,era,_)|*era),
                "pruneUpTo":state.pending.as_ref().and_then(|(_,_,prune)|*prune),
                "queuedSession":state.queued_era.map(|(session,_)|session),"queuedEra":state.queued_era.map(|(_,era)|era),
                "electionDeadline":state.election_deadline,"forced":state.forced,"timestampEra":state.timestamp_era}),
            recency_report: json!({"count":map.len(),"sha256":digest,"active":active_rows,
                "queued":queued_rows,"pending":pending_rows}),
        })
    }
}

pub(super) fn transition(before: &Core, after: &Core) -> Result<(bool, bool)> {
    let epoch_changed = after.epoch > before.epoch;
    let session_changed = after.session > before.session;
    let era_changed = after.active > before.active;
    need(
        after.genesis == before.genesis
            && after.slot > before.slot
            && after.epoch >= before.epoch
            && after.session >= before.session
            && (after.session == before.session
                || before.session.checked_add(1) == Some(after.session))
            && epoch_changed == session_changed
            && after.active >= before.active
            && (after.active == before.active
                || before.active.checked_add(1) == Some(after.active))
            && (!era_changed || session_changed)
            && after.current >= before.current
            && (after.current == before.current
                || before.current.checked_add(1) == Some(after.current)),
        "activation post-block epoch/session/era transition differs",
    )?;
    Ok((session_changed, era_changed))
}

#[cfg(test)]
pub(super) fn fixture(ext: &mut sp_io::TestExternalities, core: &Core) {
    let set = |ext: &mut sp_io::TestExternalities, pallet, item, raw: Vec<u8>| {
        ext.execute_with(|| sp_io::storage::set(&storage_key(pallet, item), &raw));
    };
    set(ext, b"Babe", b"CurrentSlot", core.slot.encode());
    set(ext, b"Babe", b"EpochIndex", core.epoch.encode());
    set(ext, b"Babe", b"GenesisSlot", core.genesis.encode());
    set(ext, b"Session", b"CurrentIndex", core.session.encode());
    set(ext, b"Staking", b"CurrentEra", core.current.encode());
    set(
        ext,
        b"Staking",
        b"ActiveEra",
        (core.active, core.start).encode(),
    );
    set(
        ext,
        b"Timestamp",
        b"Now",
        core.start.unwrap_or(100).encode(),
    );
    set(
        ext,
        b"StakingAsyncBridge",
        b"State",
        AsyncState::default().encode(),
    );
    set(ext, b"Session", b"Validators", vec![[4u8; 32]].encode());
    set(
        ext,
        b"Session",
        b"QueuedKeys",
        vec![([4u8; 32], ([1u8; 32], [2u8; 32], [3u8; 32], [4u8; 33]))].encode(),
    );
    let key = [
        storage_key(b"Staking", b"LastValidatorEra"),
        sp_io::hashing::twox_64(&[4; 32]).to_vec(),
        vec![4; 32],
    ]
    .concat();
    ext.execute_with(|| sp_io::storage::set(&key, &core.current.encode()));
}

#[cfg(test)]
mod tests {
    use super::*;
    fn core() -> Core {
        Core {
            slot: 6001,
            epoch: 10,
            genesis: 1,
            session: 10,
            current: 7,
            active: 6,
            start: Some(100),
        }
    }
    fn put_async(ext: &mut sp_io::TestExternalities, state: &AsyncState) {
        ext.execute_with(|| {
            sp_io::storage::set(
                &storage_key(b"StakingAsyncBridge", b"State"),
                &state.encode(),
            )
        });
    }
    #[test]
    fn valid_poll_pending_and_queued_states_preserve_recency_and_timestamp_contracts() {
        let mut ext = sp_io::TestExternalities::default();
        fixture(&mut ext, &core());
        let mut state = AsyncState {
            pending: Some((vec![[4; 32]], 7, None)),
            election_deadline: Some(1),
            ..Default::default()
        };
        put_async(&mut ext, &state);
        let observed = State::read(&mut ext, true).unwrap();
        assert_eq!(observed.async_report["pendingValidators"], 1);
        assert_eq!(observed.recency_report["pending"][0]["era"], 7);
        state.pending = None;
        state.queued_era = Some((11, 7));
        state.election_deadline = None;
        put_async(&mut ext, &state);
        State::read(&mut ext, true).unwrap();
        state.timestamp_era = Some(6);
        put_async(&mut ext, &state);
        State::read(&mut ext, false).unwrap();
        assert!(State::read(&mut ext, true).is_err());
    }
    #[test]
    fn invalid_queued_session_conflicting_pending_or_missing_recency_reject() {
        let mut ext = sp_io::TestExternalities::default();
        fixture(&mut ext, &core());
        let mut state = AsyncState {
            queued_era: Some((10, 7)),
            ..Default::default()
        };
        put_async(&mut ext, &state);
        assert!(State::read(&mut ext, true).is_err());
        state.queued_era = Some((11, 7));
        state.pending = Some((vec![[4; 32]], 7, None));
        put_async(&mut ext, &state);
        assert!(State::read(&mut ext, true).is_err());
        state.pending = None;
        state.election_deadline = Some(5);
        put_async(&mut ext, &state);
        assert!(State::read(&mut ext, true).is_err());
        state.election_deadline = None;
        put_async(&mut ext, &state);
        let key = [
            storage_key(b"Staking", b"LastValidatorEra"),
            sp_io::hashing::twox_64(&[4; 32]).to_vec(),
            vec![4; 32],
        ]
        .concat();
        ext.execute_with(|| sp_io::storage::set(&key, &6u32.encode()));
        assert!(State::read(&mut ext, true).is_err());
        ext.execute_with(|| sp_io::storage::clear(&key));
        assert!(State::read(&mut ext, true).is_err());
    }
    #[test]
    fn malformed_recency_hash_and_future_era_reject() {
        let mut ext = sp_io::TestExternalities::default();
        fixture(&mut ext, &core());
        let mut key = [
            storage_key(b"Staking", b"LastValidatorEra"),
            sp_io::hashing::twox_64(&[4; 32]).to_vec(),
            vec![4; 32],
        ]
        .concat();
        ext.execute_with(|| sp_io::storage::set(&key, &8u32.encode()));
        assert!(State::read(&mut ext, true).is_err());
        ext.execute_with(|| sp_io::storage::clear(&key));
        key[32] ^= 1;
        ext.execute_with(|| sp_io::storage::set(&key, &7u32.encode()));
        assert!(State::read(&mut ext, true).is_err());
    }
    #[test]
    fn session_and_era_cannot_skip_or_advance_without_actual_epoch() {
        let before = core();
        let mut after = before.clone();
        after.slot += 1;
        assert_eq!(transition(&before, &after).unwrap(), (false, false));
        after.epoch += 100;
        after.session += 1;
        after.active += 1;
        assert_eq!(transition(&before, &after).unwrap(), (true, true));
        after.session += 1;
        assert!(transition(&before, &after).is_err());
        after.session = before.session + 1;
        after.active += 1;
        assert!(transition(&before, &after).is_err());
        after = before.clone();
        after.slot += 1;
        after.session += 1;
        assert!(transition(&before, &after).is_err());
    }
    #[test]
    fn maintenance_gap_uses_actual_state_and_keeps_economic_clock_frozen() {
        let mut ext = sp_io::TestExternalities::default();
        fixture(&mut ext, &core());
        let mut track = Maintenance::checkpoint(&mut ext).unwrap();
        let mut after = core();
        after.slot += 6000;
        after.epoch += 10;
        fixture(&mut ext, &after);
        let mut witness = BlockWitness {
            number: common::recovery::CHECKPOINT_NUMBER + 1,
            hash: format!("{:#x}", H256::repeat_byte(1)),
            state_root: format!("{:#x}", replay::root(&mut ext)),
            executing_code_blake2: "01".repeat(32),
            resulting_spec: 134,
            phase: "Applying".into(),
            cursor: 0,
            operation_counter: 0,
            block_sha256: "02".repeat(32),
            disk_snapshot_manifest_sha256: "03".repeat(32),
            forbidden_extrinsics_rejected: 0,
            maintenance_session_frozen: true,
            independent_import_root_matches: true,
            disk_restart_root_matches: true,
            generated_authorization_sha256: None,
        };
        assert!(track.accepted(&mut ext, &witness, after.slot + 1).is_err());
        track.accepted(&mut ext, &witness, after.slot).unwrap();
        let report = track.report().unwrap();
        assert_eq!(report["maximumSlotGap"], 6000);
        assert_eq!(report["firstRecoveryEpoch"], 20);
        assert_eq!(report["checkpointSession"], 10);
        track.parent(H256::repeat_byte(1), &after).unwrap();
        assert!(track.parent(H256::repeat_byte(2), &after).is_err());
        after.slot += 1;
        after.active += 1;
        fixture(&mut ext, &after);
        witness.number += 1;
        witness.state_root = format!("{:#x}", replay::root(&mut ext));
        assert!(track.accepted(&mut ext, &witness, after.slot).is_err());
    }
}
