//! Durable, node-local emergency stop. Chain reorgs cannot release it.

use super::halt_at;
use crate::service::{FullBackend, FullClient};
use codec::{Decode, DecodeAll, Encode};
use common::bridge_halt::GuardianHalt;
use framenode_runtime::opaque::Block;
use futures::{FutureExt, StreamExt};
use sc_client_api::{Backend, BlockchainEvents, ImportNotifications};
use sp_blockchain::HeaderBackend;
use sp_core::{offchain::OffchainStorage, H256};
use sp_runtime::offchain::STORAGE_PREFIX;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

pub(crate) const STORAGE_KEY: &[u8] = b"eth-bridge-ocw::observed-halt:v1";

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
enum Observation {
    Absent,
    Halt(GuardianHalt),
    Unreadable,
}

impl Observation {
    fn from_result(value: Result<Option<GuardianHalt>, String>) -> Self {
        match value {
            Ok(None) => Self::Absent,
            Ok(Some(halt)) if halt.number > 0 => Self::Halt(halt),
            Ok(Some(_)) => Self::Unreadable,
            Err(_) => Self::Unreadable,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
struct Record {
    version: u8,
    genesis: H256,
    epoch: u64,
    observation: Observation,
    released: bool,
}

impl Record {
    fn decode_checked(bytes: &[u8], genesis: H256) -> Result<Self, String> {
        if bytes.len() > 96 {
            return Err("observed-halt record exceeds its bound".into());
        }
        let (value, checksum) = <(Self, [u8; 32])>::decode_all(&mut &bytes[..])
            .map_err(|_| "observed-halt record is corrupt")?;
        if sp_io::hashing::blake2_256(&value.encode()) != checksum {
            return Err("observed-halt record checksum mismatch".into());
        }
        if value.version != 1 || value.genesis != genesis {
            return Err("observed-halt record has wrong version or genesis".into());
        }
        if value.epoch == 0 && (value.observation != Observation::Absent || !value.released) {
            return Err("observed-halt initial epoch is invalid".into());
        }
        if matches!(value.observation, Observation::Halt(halt) if halt.number == 0 || halt.block == 0)
        {
            return Err("observed-halt signal is invalid".into());
        }
        if value.released && value.observation == Observation::Unreadable {
            return Err("unreadable halt cannot be released".into());
        }
        Ok(value)
    }

    fn persisted_bytes(&self) -> Vec<u8> {
        (self, sp_io::hashing::blake2_256(&self.encode())).encode()
    }

    fn digest(&self) -> H256 {
        H256(sp_io::hashing::blake2_256(
            &(
                b"sora-nexus/observed-halt-release/v1".as_slice(),
                self.version,
                self.genesis,
                self.epoch,
                &self.observation,
            )
                .encode(),
        ))
    }
}

fn observe(
    storage: &mut impl OffchainStorage,
    genesis: H256,
    observation: Observation,
) -> Result<Record, String> {
    let previous = storage.get(STORAGE_PREFIX, STORAGE_KEY);
    let mut record = match previous.as_deref() {
        Some(bytes) => Record::decode_checked(bytes, genesis)?,
        None => Record {
            version: 1,
            genesis,
            epoch: 0,
            observation: Observation::Absent,
            released: true,
        },
    };
    if record.observation != observation {
        record.epoch = record.epoch.checked_add(1).ok_or("halt epoch exhausted")?;
        record.observation = observation;
        record.released = false;
    }
    let encoded = record.persisted_bytes();
    if previous.as_deref() != Some(encoded.as_slice()) {
        write_checked(storage, STORAGE_KEY, &encoded)?;
    }
    Ok(record)
}

fn write_checked(
    storage: &mut impl OffchainStorage,
    key: &[u8],
    bytes: &[u8],
) -> Result<(), String> {
    storage.set(STORAGE_PREFIX, key, bytes);
    if storage.get(STORAGE_PREFIX, key).as_deref() != Some(bytes) {
        return Err("local guardian state write could not be verified".into());
    }
    Ok(())
}

fn acknowledge(
    storage: &mut impl OffchainStorage,
    genesis: H256,
    requested: H256,
    best: &Observation,
    finalized: &Observation,
) -> Result<Record, String> {
    let bytes = storage
        .get(STORAGE_PREFIX, STORAGE_KEY)
        .ok_or("no observed-halt record")?;
    let mut record = Record::decode_checked(&bytes, genesis)?;
    if best == &Observation::Unreadable || best != finalized || best != &record.observation {
        return Err("best, finalized and observed guardian states must agree".into());
    }
    if record.digest() != requested {
        return Err(format!(
            "observed-halt context changed; exact current digest is {:#x}",
            record.digest()
        ));
    }
    // Preserve the runtime's existing finalized guardian acknowledgement. Only
    // this explicit digest path can install or remove it after migration.
    let legacy = framenode_runtime::eth_bridge::STORAGE_RESUMED_AFTER_HALT_KEY;
    match best {
        Observation::Halt(halt) => write_checked(storage, legacy, &halt.encode())?,
        Observation::Absent => {
            storage.remove(STORAGE_PREFIX, legacy);
            if storage.get(STORAGE_PREFIX, legacy).is_some() {
                return Err("local guardian acknowledgement removal could not be verified".into());
            }
        }
        Observation::Unreadable => unreachable!(),
    }
    // Write release last. Partial failure can leave a conservative stop, never
    // release through an unchanged, stale numeric service flag.
    record.released = true;
    write_checked(storage, STORAGE_KEY, &record.persisted_bytes())?;
    Ok(record)
}

fn numeric_is_idempotent(record: &Record, requested: u32) -> bool {
    record.released
        && matches!(record.observation, Observation::Halt(halt) if halt.number == requested)
}

fn gate_open(
    storage: &impl OffchainStorage,
    genesis: H256,
    best: H256,
    caught_up: Option<H256>,
    faulted: bool,
) -> bool {
    !faulted
        && caught_up == Some(best)
        && storage
            .get(STORAGE_PREFIX, STORAGE_KEY)
            .and_then(|bytes| Record::decode_checked(&bytes, genesis).ok())
            .is_some_and(|record| record.released)
}

/// Observer updates and actual signing/pool polls share this lock. The import
/// lock is held by WorkerGuard while it checks the current-best barrier. A
/// delayed import notification cannot be skipped merely because a later reorg
/// already removed its halt: signing stays closed until queued observations
/// reach the current best hash.
struct ObserverState {
    caught_up: Option<H256>,
    imports: ImportNotifications<Block>,
    drained: bool,
}

pub(crate) struct Monitor {
    client: Arc<FullClient>,
    backend: Arc<FullBackend>,
    state: Mutex<ObserverState>,
    failed: AtomicBool,
}

impl Monitor {
    fn observation(&self, hash: H256) -> Observation {
        Observation::from_result(halt_at(&self.client, hash))
    }

    fn observe_locked(&self, hash: H256, state: &mut ObserverState) {
        if self.failed.load(Ordering::Acquire) {
            state.caught_up = None;
            return;
        }
        let startup = state.caught_up.is_none();
        state.caught_up = None;
        let Some(mut storage) = self.backend.offchain_storage() else {
            self.failed.store(true, Ordering::Release);
            return;
        };
        let before = storage.get(STORAGE_PREFIX, STORAGE_KEY);
        match observe(
            &mut storage,
            self.client.info().genesis_hash,
            self.observation(hash),
        ) {
            Ok(record) => {
                state.caught_up = Some(hash);
                if !record.released
                    && (startup || before.as_deref() != Some(record.persisted_bytes().as_slice()))
                {
                    log::warn!(
                        "SORA Nexus observed guardian halt remains stopped locally (epoch {}). After review and finality, restart with --bridge-resume-after-observed-halt {:#x}",
                        record.epoch, record.digest()
                    );
                }
            }
            Err(error) => {
                self.failed.store(true, Ordering::Release);
                log::error!("SORA Nexus guardian stop remains closed: {error}");
            }
        }
    }

    /// The SDK enqueues every import notification before releasing its import
    /// write lock. Under WorkerGuard's read lock, reaching Pending here proves
    /// there is no unprocessed halt hidden behind a best-hash ABA transition.
    fn drain_locked(&self, state: &mut ObserverState) {
        state.drained = false;
        if self.failed.load(Ordering::Acquire) {
            return;
        }
        // Bound each pass during bulk sync. Exhausting this bound keeps the gate
        // closed; the background observer continues draining on its next pass.
        for _ in 0..1024 {
            match state.imports.next().now_or_never() {
                Some(Some(notification)) => {
                    if notification.is_new_best {
                        self.observe_locked(notification.hash, state);
                    }
                    if self.failed.load(Ordering::Acquire) {
                        return;
                    }
                }
                Some(None) => {
                    state.caught_up = None;
                    self.failed.store(true, Ordering::Release);
                    log::error!("SORA Nexus guardian notification stream ended; HASHI access remains closed");
                    return;
                }
                None => {
                    state.drained = true;
                    return;
                }
            }
        }
    }

    pub(crate) fn refresh(&self) {
        let mut state = self.state.lock().expect("guardian observer lock");
        self.drain_locked(&mut state);
    }

    pub(crate) fn while_open<R>(
        &self,
        best: H256,
        action: impl FnOnce() -> Option<R>,
    ) -> Option<R> {
        let mut state = self.state.lock().expect("guardian observer lock");
        self.drain_locked(&mut state);
        if !state.drained || self.failed.load(Ordering::Acquire) {
            return None;
        }
        let storage = self.backend.offchain_storage()?;
        if !gate_open(
            &storage,
            self.client.info().genesis_hash,
            best,
            state.caught_up,
            self.failed.load(Ordering::Acquire),
        ) {
            return None;
        }
        action()
    }

    pub(crate) fn is_open_now(&self) -> bool {
        self.while_open(self.client.info().best_hash, || Some(true))
            .unwrap_or(false)
    }
}

/// Subscribe before observing startup state so no intervening best-chain change
/// can be lost. This task remains active even for an OCW-disabled syncing node.
pub(crate) fn start(
    client: Arc<FullClient>,
    backend: Arc<FullBackend>,
    numeric: Option<u32>,
    requested: Option<H256>,
) -> (
    Arc<Monitor>,
    impl std::future::Future<Output = ()> + Send + 'static,
) {
    let imports = client.every_import_notification_stream();
    let monitor = Arc::new(Monitor {
        client,
        backend,
        state: Mutex::new(ObserverState {
            caught_up: None,
            imports,
            drained: false,
        }),
        failed: AtomicBool::new(false),
    });
    let info = monitor.client.info();
    {
        let mut state = monitor.state.lock().expect("guardian observer lock");
        monitor.observe_locked(info.best_hash, &mut state);
        monitor.drain_locked(&mut state);
        if let Some(mut storage) = monitor.backend.offchain_storage() {
            if let Some(digest) = requested {
                match acknowledge(&mut storage, info.genesis_hash, digest,
                    &monitor.observation(info.best_hash),
                    &monitor.observation(info.finalized_hash)) {
                    Ok(record) => log::warn!(
                        "SORA Nexus observed halt epoch {} acknowledged locally; worker mode and recovery acknowledgement remain independent",
                        record.epoch
                    ),
                    Err(error) => {
                        monitor.failed.store(true, Ordering::Release);
                        log::error!("SORA Nexus observed halt acknowledgement refused: {error}");
                    }
                }
            }
            if let Some(number) = numeric {
                let compatible = storage
                    .get(STORAGE_PREFIX, STORAGE_KEY)
                    .and_then(|bytes| Record::decode_checked(&bytes, info.genesis_hash).ok())
                    .is_some_and(|record| numeric_is_idempotent(&record, number));
                if !compatible {
                    log::error!("SORA Nexus numeric --bridge-resume-after-halt cannot release a pending or replacement halt; use the exact observed-halt digest printed by this node");
                }
            }
        }
    }
    let runner = monitor.clone();
    let task = async move {
        loop {
            runner.refresh();
            if runner.failed.load(Ordering::Acquire) {
                return;
            }
            futures_timer::Delay::new(std::time::Duration::from_millis(100)).await;
        }
    };
    (monitor, task)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sp_core::offchain::storage::InMemOffchainStorage;

    const FIRST: GuardianHalt = GuardianHalt {
        number: 1,
        block: 10,
    };
    const SECOND: GuardianHalt = GuardianHalt {
        number: 2,
        block: 20,
    };

    #[test]
    fn observed_halt_stays_stopped_through_reorg_disappearance_and_ordinary_restart() {
        let genesis = H256::repeat_byte(7);
        let mut storage = InMemOffchainStorage::default();
        assert!(
            observe(&mut storage, genesis, Observation::Absent)
                .unwrap()
                .released
        );
        let stopped = observe(&mut storage, genesis, Observation::Halt(FIRST)).unwrap();
        assert!(!stopped.released);
        let removed = observe(&mut storage, genesis, Observation::Absent).unwrap();
        assert!(!removed.released);
        assert_ne!(removed.digest(), stopped.digest());
        let restarted = observe(&mut storage, genesis, Observation::Absent).unwrap();
        assert_eq!(restarted, removed);
        assert!(acknowledge(
            &mut storage,
            genesis,
            stopped.digest(),
            &Observation::Absent,
            &Observation::Absent
        )
        .is_err());
        let released = acknowledge(
            &mut storage,
            genesis,
            removed.digest(),
            &Observation::Absent,
            &Observation::Absent,
        )
        .unwrap();
        assert!(released.released);
    }

    #[test]
    fn observed_halt_exact_release_cannot_cover_next_epoch_or_numeric_replacement() {
        let genesis = H256::repeat_byte(7);
        let mut storage = InMemOffchainStorage::default();
        let first = Observation::Halt(FIRST);
        let pending = observe(&mut storage, genesis, first.clone()).unwrap();
        assert!(!numeric_is_idempotent(&pending, FIRST.number));
        assert!(acknowledge(
            &mut storage,
            genesis,
            pending.digest(),
            &first,
            &Observation::Absent
        )
        .is_err());
        let released =
            acknowledge(&mut storage, genesis, pending.digest(), &first, &first).unwrap();
        assert!(numeric_is_idempotent(&released, FIRST.number));
        assert!(
            observe(&mut storage, genesis, first.clone())
                .unwrap()
                .released
        );
        let next = observe(&mut storage, genesis, Observation::Halt(SECOND)).unwrap();
        assert!(!next.released);
        let rewind = observe(&mut storage, genesis, first.clone()).unwrap();
        assert!(!rewind.released);
        assert!(!numeric_is_idempotent(&rewind, FIRST.number));
        assert!(acknowledge(&mut storage, genesis, pending.digest(), &first, &first).is_err());
        let replacement = Observation::Halt(GuardianHalt {
            number: 1,
            block: 11,
        });
        let replaced = observe(&mut storage, genesis, replacement.clone()).unwrap();
        assert!(!numeric_is_idempotent(&replaced, FIRST.number));
        assert!(
            acknowledge(
                &mut storage,
                genesis,
                replaced.digest(),
                &replacement,
                &replacement
            )
            .unwrap()
            .released
        );
    }

    #[test]
    fn observed_halt_corruption_read_failures_and_legacy_ack_never_release() {
        let genesis = H256::repeat_byte(7);
        let mut storage = InMemOffchainStorage::default();
        storage.set(
            STORAGE_PREFIX,
            framenode_runtime::eth_bridge::STORAGE_RESUMED_AFTER_HALT_KEY,
            &FIRST.encode(),
        );
        let pending = observe(&mut storage, genesis, Observation::Halt(FIRST)).unwrap();
        assert!(!pending.released); // migration never treats old numeric ack as authority
        let unreadable = observe(&mut storage, genesis, Observation::Unreadable).unwrap();
        assert!(!unreadable.released);
        assert!(acknowledge(
            &mut storage,
            genesis,
            unreadable.digest(),
            &Observation::Unreadable,
            &Observation::Unreadable
        )
        .is_err());
        let recovered = observe(&mut storage, genesis, Observation::Halt(FIRST)).unwrap();
        assert!(!recovered.released);
        for malformed in [
            vec![],
            vec![1],
            [recovered.persisted_bytes(), vec![0]].concat(),
        ] {
            storage.set(STORAGE_PREFIX, STORAGE_KEY, &malformed);
            assert!(observe(&mut storage, genesis, Observation::Absent).is_err());
            assert!(acknowledge(
                &mut storage,
                genesis,
                recovered.digest(),
                &Observation::Halt(FIRST),
                &Observation::Halt(FIRST)
            )
            .is_err());
            assert_eq!(storage.get(STORAGE_PREFIX, STORAGE_KEY), Some(malformed));
        }
        let mut flipped = recovered.persisted_bytes();
        let release_index = recovered.encode().len() - 1;
        flipped[release_index] ^= 1;
        storage.set(STORAGE_PREFIX, STORAGE_KEY, &flipped);
        assert!(observe(&mut storage, genesis, Observation::Halt(FIRST)).is_err());
        storage.set(STORAGE_PREFIX, STORAGE_KEY, &recovered.persisted_bytes());
        assert!(observe(&mut storage, H256::repeat_byte(8), Observation::Absent).is_err());
    }

    #[test]
    fn observed_halt_import_barrier_closes_until_every_best_observation_is_processed() {
        let genesis = H256::repeat_byte(7);
        let old = H256::repeat_byte(8);
        let halted = H256::repeat_byte(9);
        let reorg = H256::repeat_byte(10);
        let mut storage = InMemOffchainStorage::default();
        observe(&mut storage, genesis, Observation::Absent).unwrap();
        assert!(gate_open(&storage, genesis, old, Some(old), false));
        // A later reorg tip cannot sign while its preceding halt notification
        // is still waiting in the observer's ordered queue.
        assert!(!gate_open(&storage, genesis, reorg, Some(old), false));
        observe(&mut storage, genesis, Observation::Halt(FIRST)).unwrap();
        assert!(!gate_open(&storage, genesis, reorg, Some(halted), false));
        let pending = observe(&mut storage, genesis, Observation::Absent).unwrap();
        assert!(!gate_open(&storage, genesis, reorg, Some(reorg), false));
        acknowledge(
            &mut storage,
            genesis,
            pending.digest(),
            &Observation::Absent,
            &Observation::Absent,
        )
        .unwrap();
        assert!(gate_open(&storage, genesis, reorg, Some(reorg), false));
        assert!(!gate_open(&storage, genesis, reorg, None, false));
        storage.set(STORAGE_PREFIX, STORAGE_KEY, b"corrupt");
        assert!(!gate_open(&storage, genesis, reorg, Some(reorg), false));
    }

    #[derive(Clone, Default)]
    struct RefuseWrites(InMemOffchainStorage);

    impl OffchainStorage for RefuseWrites {
        fn get(&self, prefix: &[u8], key: &[u8]) -> Option<Vec<u8>> {
            self.0.get(prefix, key)
        }
        fn set(&mut self, _: &[u8], _: &[u8], _: &[u8]) {}
        fn remove(&mut self, _: &[u8], _: &[u8]) {}
        fn compare_and_set(&mut self, _: &[u8], _: &[u8], _: Option<&[u8]>, _: &[u8]) -> bool {
            false
        }
    }

    #[test]
    fn observed_halt_unverified_storage_writes_cannot_release_key_or_pool_gate() {
        let genesis = H256::repeat_byte(7);
        let best = H256::repeat_byte(8);
        let mut storage = RefuseWrites::default();
        assert!(observe(&mut storage, genesis, Observation::Absent).is_err());
        assert!(!gate_open(&storage, genesis, best, Some(best), false));
        observe(&mut storage.0, genesis, Observation::Absent).unwrap();
        let faulted = observe(&mut storage, genesis, Observation::Halt(FIRST)).is_err();
        assert!(faulted);
        // A dropped halt write leaves an old released record. The monitor's
        // sticky fault, not a later successful Absent read, keeps access closed.
        assert!(!gate_open(&storage, genesis, best, Some(best), faulted));
        observe(&mut storage.0, genesis, Observation::Absent).unwrap();
        assert!(!gate_open(&storage, genesis, best, Some(best), faulted));
        let pending = observe(&mut storage.0, genesis, Observation::Halt(FIRST)).unwrap();
        assert!(acknowledge(
            &mut storage,
            genesis,
            pending.digest(),
            &Observation::Halt(FIRST),
            &Observation::Halt(FIRST)
        )
        .is_err());
        assert!(!gate_open(&storage, genesis, best, Some(best), false));
        assert_eq!(
            storage.get(STORAGE_PREFIX, STORAGE_KEY),
            Some(pending.persisted_bytes())
        );
    }

    #[derive(Clone, Default)]
    struct CorruptWrites(InMemOffchainStorage);

    impl OffchainStorage for CorruptWrites {
        fn get(&self, prefix: &[u8], key: &[u8]) -> Option<Vec<u8>> {
            self.0.get(prefix, key)
        }
        fn set(&mut self, prefix: &[u8], key: &[u8], _: &[u8]) {
            self.0.set(prefix, key, b"corrupt")
        }
        fn remove(&mut self, prefix: &[u8], key: &[u8]) {
            self.0.remove(prefix, key)
        }
        fn compare_and_set(&mut self, _: &[u8], _: &[u8], _: Option<&[u8]>, _: &[u8]) -> bool {
            false
        }
    }

    #[test]
    fn observed_halt_corrupting_writes_fail_readback_and_leave_access_closed() {
        let genesis = H256::repeat_byte(7);
        let best = H256::repeat_byte(8);
        let mut storage = CorruptWrites::default();
        assert!(observe(&mut storage, genesis, Observation::Halt(FIRST)).is_err());
        assert!(!gate_open(&storage, genesis, best, Some(best), false));
        let pending = observe(&mut storage.0, genesis, Observation::Absent);
        assert!(pending.is_err()); // Corrupt state is preserved, never reset to absence.
        storage.0.remove(STORAGE_PREFIX, STORAGE_KEY);
        let pending = observe(&mut storage.0, genesis, Observation::Halt(FIRST)).unwrap();
        assert!(acknowledge(
            &mut storage,
            genesis,
            pending.digest(),
            &Observation::Halt(FIRST),
            &Observation::Halt(FIRST)
        )
        .is_err());
        assert!(!gate_open(&storage, genesis, best, Some(best), false));
    }
}
