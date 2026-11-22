// This file is part of the SORA network and Polkaswap app.
// SPDX-License-Identifier: BSD-4-Clause

//! An OCW executes against the block that spawned it, even after another block
//! has been imported. Serialize HASHI key use and local submission with imports
//! and consult the node's current best state at the point of each operation.

use super::halt_at;
use crate::bridge_priority_scan::{is_priority_storage_key, PriorityLease, PriorityScanPlan};
use crate::service::{FullBackend, FullClient};
use codec::{DecodeAll, Encode};
use common::bridge_halt::{workers_may_run, GuardianHalt};
use framenode_runtime::{eth_bridge, opaque::Block, RuntimeCall, UncheckedExtrinsic};
use sc_client_api::{Backend, StorageProvider};
use sc_transaction_pool_api::{
    error::Error as PoolError, LocalTransactionPool, TransactionPool, TransactionSource,
};
use sp_blockchain::HeaderBackend;
use sp_core::{
    crypto::KeyTypeId, ecdsa, ed25519, offchain::OffchainStorage, sr25519, storage::StorageKey,
    H256,
};
use sp_keystore::{Error, Keystore, KeystorePtr};
use sp_runtime::{offchain::STORAGE_PREFIX, OpaqueExtrinsic};
use std::{
    future::{poll_fn, Future},
    sync::{Arc, Mutex, Weak},
    task::Poll,
};

#[derive(Clone)]
pub(crate) struct WorkerGuard {
    pub client: Arc<FullClient>,
    pub backend: Arc<FullBackend>,
    /// The Wasm context using this key. None is used only by the shared pool.
    pub execution_block: Option<H256>,
    pub priority_plan_armed: bool,
    pub guardian_monitor: Arc<super::observed::Monitor>,
}

pub(crate) trait BridgeGuard: Send + Sync {
    fn while_open<R>(&self, action: impl FnOnce() -> R) -> Option<R>;

    fn for_submission_at(&self, _at: H256) -> Self
    where
        Self: Sized + Clone,
    {
        self.clone()
    }
}

fn priority_domain_allows(
    armed: bool,
    genesis: H256,
    code: Option<H256>,
    current_contracts_match: impl FnOnce() -> bool,
) -> bool {
    !armed
        || (crate::bridge_local_quarantine::reviewed_priority_code(genesis, code)
            && current_contracts_match())
}

impl BridgeGuard for WorkerGuard {
    fn for_submission_at(&self, at: H256) -> Self {
        let mut guard = self.clone();
        if guard.priority_plan_armed {
            guard.execution_block = Some(at);
        }
        guard
    }

    fn while_open<R>(&self, action: impl FnOnce() -> R) -> Option<R> {
        // The OCW already occupies a Wasm instance. Yield to an active or queued
        // importer instead of waiting for the lock while holding that instance.
        let _import = self.backend.get_import_lock().try_read()?;
        let best = self.client.info().best_hash;
        self.guardian_monitor.while_open(best, || {
            // The local request guard relies on reviewed Wasm signing/retry paths.
            // Recheck both the worker's historical context and live best code under
            // the import lock. An upgrade, unknown chain or missing state closes
            // HASHI signing/submission until its behavior is reviewed.
            let code_key = StorageKey(b":code".to_vec());
            let genesis = self.client.info().genesis_hash;
            let best_code = self.client.storage_hash(best, &code_key).ok()?;
            let worker_code = match self.execution_block {
                Some(block) if block != best => self.client.storage_hash(block, &code_key).ok()?,
                _ => best_code,
            };
            if !crate::bridge_local_quarantine::reviewed_worker_code(genesis, best_code, worker_code) {
                log::error!("SORA Nexus local request quarantine: HASHI worker requires the same reviewed runtime as the live chain");
                return None;
            }
            // Pending/failed imports can outlive the invocation that discovered them.
            // Enforce the pinned domain on every HASHI sign and pool poll while this
            // plan is installed, including chronological retries of cached bytes.
            if !priority_domain_allows(self.priority_plan_armed, genesis, worker_code, || {
                let key = StorageKey(crate::bridge_scan_cursor::contract_storage_key(0));
                let matches = |at| self.client.storage(at, &key).ok().flatten().is_some_and(|value| {
                    value.0.as_slice() == crate::bridge_priority_manifest::MAINNET_DEPOSIT_CONTRACT
                });
                matches(best) && self.execution_block.is_none_or(|at| at == best || matches(at))
            }) { return None; }
            let latest = halt_at(&self.client, best).ok()?;
            let storage = self.backend.offchain_storage()?;
            let confirmed = storage
                .get(STORAGE_PREFIX, eth_bridge::STORAGE_RESUMED_AFTER_HALT_KEY)
                .and_then(|bytes| GuardianHalt::decode_all(&mut &bytes[..]).ok());
            if !workers_may_run(&Ok(latest), confirmed) {
                return None;
            }
            let completed = crate::recovery_acknowledgement::completion_at(&self.client, best);
            let execution = self
                .execution_block
                .map(|hash| crate::recovery_acknowledgement::completion_at(&self.client, hash));
            let acknowledgement = storage.get(
                STORAGE_PREFIX,
                bridge_types::recovery::RECOVERY_ACK_STORAGE_KEY,
            );
            if !crate::recovery_acknowledgement::workers_may_run(
                genesis,
                &completed,
                execution.as_ref(),
                acknowledgement.as_deref(),
            ) {
                return None;
            }
            Some(action())
        })
    }
}

/// Shared by every invocation, unlike the token assigned to each OCW extension.
/// The weak owner expires even if Wasm traps and never drops its StorageLockGuard.
#[derive(Clone, Default)]
pub(crate) struct WorkerPasses {
    slot: Arc<Mutex<Option<PassLock>>>,
    plan: Option<Arc<PriorityScanPlan>>,
}

struct PassLock {
    owner: Weak<()>,
    deadline: Vec<u8>,
    priority: Option<PriorityAttempt>,
}

impl PassLock {
    fn owned_by(&self, invocation: &Arc<()>) -> bool {
        self.owner.ptr_eq(&Arc::downgrade(invocation))
    }
}

struct PriorityAttempt {
    lease: PriorityLease,
    write: CursorWrite,
}

enum CursorWrite {
    Pending,
    Advanced(u64),
    Unexpected,
}

// These three network-0 keys alone receive a temporary view. Retry maps,
// other networks, Ethereum height and signing APIs keep their normal behavior.
const ETH_CURSOR: &[u8] = b"eth-bridge-ocw::eth-to-handle-from-height-0";
const ETH_RANGE_SIZE: &[u8] = b"eth-bridge-ocw::eth-log-range-size-0";
const ETH_RANGE_LIMIT: &[u8] = b"eth-bridge-ocw::eth-log-range-limit-v1-0";

fn scan_key(key: &[u8]) -> bool {
    matches!(key, ETH_CURSOR | ETH_RANGE_SIZE | ETH_RANGE_LIMIT)
}

impl PriorityAttempt {
    fn read(&self, key: &[u8]) -> Option<Vec<u8>> {
        match key {
            ETH_CURSOR => Some(match self.write {
                CursorWrite::Advanced(next) => next.encode(),
                _ => self.lease.height().encode(),
            }),
            ETH_RANGE_SIZE => Some(1u64.encode()),
            // Suppress the normal scanner's adaptive range probe for this pass.
            ETH_RANGE_LIMIT => None,
            _ => unreachable!("only network-0 scan keys are scoped"),
        }
    }

    fn record_write(&mut self, key: &[u8], bytes: Option<&[u8]>) {
        if key == ETH_CURSOR {
            let height = bytes.and_then(|mut b| u64::decode_all(&mut b).ok());
            self.write = match height {
                Some(n)
                    if Some(n) == self.lease.height().checked_add(1)
                        && !matches!(self.write, CursorWrite::Unexpected) =>
                {
                    CursorWrite::Advanced(n)
                }
                _ => CursorWrite::Unexpected,
            };
        }
    }
}

impl WorkerPasses {
    pub(crate) fn new(plan: Option<PriorityScanPlan>) -> Self {
        Self {
            plan: plan.map(Arc::new),
            ..Self::default()
        }
    }

    fn storage<S>(&self, inner: S) -> WorkerStorage<S> {
        WorkerStorage {
            inner,
            passes: self.clone(),
            invocation: Arc::new(()),
            guard: None,
        }
    }

    pub(crate) fn extension<S: OffchainStorage + 'static>(
        &self,
        inner: S,
        guard: WorkerGuard,
    ) -> Box<dyn sp_externalities::Extension> {
        self.extension_inner(inner, Some(guard))
    }

    #[cfg(test)]
    pub(crate) fn test_extension<S: OffchainStorage + 'static>(
        &self,
        inner: S,
    ) -> Box<dyn sp_externalities::Extension> {
        self.extension_inner(inner, None)
    }

    fn extension_inner<S: OffchainStorage + 'static>(
        &self,
        inner: S,
        guard: Option<WorkerGuard>,
    ) -> Box<dyn sp_externalities::Extension> {
        use sp_core::offchain::{Capabilities, LimitedExternalities, OffchainDbExt};
        let mut storage = self.storage(inner);
        storage.guard = guard;
        Box::new(OffchainDbExt::new(LimitedExternalities::new(
            Capabilities::all(),
            sc_offchain::OffchainDb::new(storage),
        )))
    }
}

#[derive(Clone)]
struct WorkerStorage<S> {
    inner: S,
    passes: WorkerPasses,
    invocation: Arc<()>,
    guard: Option<WorkerGuard>,
}

fn guard_capability(key: &[u8]) -> bool {
    key == eth_bridge::STORAGE_LIVE_SIGNING_GUARD_KEY
        || key == eth_bridge::STORAGE_SERIAL_SIGNING_GUARD_KEY
}

fn worker_write_protected(key: &[u8]) -> bool {
    guard_capability(key)
        || is_priority_storage_key(key)
        || key == bridge_types::recovery::RECOVERY_ACK_STORAGE_KEY
        || key == super::observed::STORAGE_KEY
        || key == eth_bridge::STORAGE_RESUMED_AFTER_HALT_KEY
}

fn guarded_scan<R>(guard: Option<&WorkerGuard>, action: impl FnOnce() -> R) -> Option<R> {
    match guard {
        Some(guard) => guard.while_open(action),
        None if cfg!(test) => Some(action()),
        None => None,
    }
}

impl<S: OffchainStorage> WorkerStorage<S> {
    fn acquire(&mut self, old: Option<&[u8]>, new: &[u8]) -> bool {
        let mut slot = self.passes.slot.lock().expect("HASHI worker pass lock");
        let active = slot.as_mut().filter(|pass| pass.owner.strong_count() != 0);
        if active.as_ref().map(|pass| pass.deadline.as_slice()) != old {
            return false;
        }
        if let Some(pass) = active {
            if !pass.owned_by(&self.invocation) {
                return false;
            }
            // A renewal retains its lane and consumes no scheduling opportunity.
            pass.deadline = new.to_vec();
            return true;
        }
        // Bootstrap reads before the lock and competing invocations see real
        // history. Only a new, exclusive worker can select a priority block.
        let priority = self.passes.plan.as_ref().and_then(|plan| {
            let bytes = self.inner.get(STORAGE_PREFIX, ETH_CURSOR)?;
            let history = u64::decode_all(&mut &bytes[..]).ok()?;
            let result =
                guarded_scan(self.guard.as_ref(), || plan.begin(&mut self.inner, history))?;
            match result {
                Ok(lease) => lease.map(|lease| PriorityAttempt {
                    lease,
                    write: CursorWrite::Pending,
                }),
                Err(e) => {
                    log::warn!(
                        "Bridge priority scan unavailable, continuing chronological replay: {e}"
                    );
                    None
                }
            }
        });
        *slot = Some(PassLock {
            owner: Arc::downgrade(&self.invocation),
            deadline: new.to_vec(),
            priority,
        });
        true
    }

    fn release(&mut self) {
        let mut slot = self.passes.slot.lock().expect("HASHI worker pass lock");
        if !slot
            .as_ref()
            .is_some_and(|pass| pass.owned_by(&self.invocation))
        {
            return;
        }
        let priority = slot.take().and_then(|pass| pass.priority);
        // A trapped invocation never completes a target. A clean release needs
        // the reviewed scanner's exact one-block cursor advancement as well.
        if let (Some(plan), Some(attempt)) = (&self.passes.plan, priority) {
            if let CursorWrite::Advanced(next) = attempt.write {
                let result = guarded_scan(self.guard.as_ref(), || {
                    plan.complete(&mut self.inner, &attempt.lease, next)
                });
                if let Some(Err(e)) = result {
                    log::warn!("Bridge priority scan progress was not recorded: {e}");
                }
            }
        }
    }

    fn edit_scan_key<R>(
        &mut self,
        prefix: &[u8],
        key: &[u8],
        denied: R,
        scoped: impl FnOnce(&mut PriorityAttempt) -> R,
        ordinary: impl FnOnce(&mut S) -> R,
    ) -> R {
        if prefix == STORAGE_PREFIX && self.passes.plan.is_some() && scan_key(key) {
            let mut slot = self.passes.slot.lock().expect("HASHI worker pass lock");
            let Some(pass) = slot.as_mut().filter(|p| p.owned_by(&self.invocation)) else {
                return denied;
            };
            return match &mut pass.priority {
                Some(attempt) => scoped(attempt),
                None => ordinary(&mut self.inner),
            };
        }
        ordinary(&mut self.inner)
    }
}

impl<S: OffchainStorage> OffchainStorage for WorkerStorage<S> {
    fn get(&self, prefix: &[u8], key: &[u8]) -> Option<Vec<u8>> {
        if prefix == STORAGE_PREFIX && guard_capability(key) {
            return self
                .guard
                .as_ref()
                .map_or(true, |g| g.guardian_monitor.is_open_now())
                .then(|| vec![1]);
        }
        if prefix == STORAGE_PREFIX && key == eth_bridge::STORAGE_WORKER_LOCK_KEY {
            return self
                .passes
                .slot
                .lock()
                .expect("HASHI worker pass lock")
                .as_ref()
                .filter(|pass| pass.owner.strong_count() != 0)
                .map(|pass| pass.deadline.clone());
        }
        if prefix == STORAGE_PREFIX && self.passes.plan.is_some() && scan_key(key) {
            let slot = self.passes.slot.lock().expect("HASHI worker pass lock");
            if let Some(attempt) = slot
                .as_ref()
                .filter(|p| p.owned_by(&self.invocation))
                .and_then(|p| p.priority.as_ref())
            {
                return attempt.read(key);
            }
        }
        self.inner.get(prefix, key)
    }

    fn set(&mut self, prefix: &[u8], key: &[u8], value: &[u8]) {
        if prefix != STORAGE_PREFIX
            || (!worker_write_protected(key) && key != eth_bridge::STORAGE_WORKER_LOCK_KEY)
        {
            self.edit_scan_key(
                prefix,
                key,
                (),
                |attempt| attempt.record_write(key, Some(value)),
                |db| db.set(prefix, key, value),
            );
        }
    }

    fn remove(&mut self, prefix: &[u8], key: &[u8]) {
        if prefix == STORAGE_PREFIX && key == eth_bridge::STORAGE_WORKER_LOCK_KEY {
            self.release();
        } else if prefix != STORAGE_PREFIX || !worker_write_protected(key) {
            self.edit_scan_key(
                prefix,
                key,
                (),
                |attempt| attempt.record_write(key, None),
                |db| db.remove(prefix, key),
            );
        }
    }

    fn compare_and_set(
        &mut self,
        prefix: &[u8],
        key: &[u8],
        old: Option<&[u8]>,
        new: &[u8],
    ) -> bool {
        if prefix == STORAGE_PREFIX && key == eth_bridge::STORAGE_WORKER_LOCK_KEY {
            return self.acquire(old, new);
        }
        (prefix != STORAGE_PREFIX || !worker_write_protected(key))
            && self.edit_scan_key(
                prefix,
                key,
                false,
                |attempt| {
                    // The reviewed runtime advances the cursor with set, never CAS.
                    attempt.record_write(key, None);
                    false
                },
                |db| db.compare_and_set(prefix, key, old, new),
            )
    }
}

pub(crate) struct WorkerKeystore<G> {
    pub inner: KeystorePtr,
    pub guard: G,
}

// All other OCW keys and keystore operations keep their normal behavior.
macro_rules! delegate {
    ($(fn $name:ident($($arg:ident : $ty:ty),*) -> $ret:ty;)+) => {
        $(fn $name(&self, $($arg: $ty),*) -> $ret {
            self.inner.$name($($arg),*)
        })+
    };
}

impl<G: BridgeGuard> Keystore for WorkerKeystore<G> {
    delegate! {
        fn sr25519_public_keys(id: KeyTypeId) -> Vec<sr25519::Public>;
        fn sr25519_generate_new(id: KeyTypeId, seed: Option<&str>) -> Result<sr25519::Public, Error>;
        fn sr25519_sign(id: KeyTypeId, public: &sr25519::Public, msg: &[u8]) -> Result<Option<sr25519::Signature>, Error>;
        fn sr25519_vrf_sign(id: KeyTypeId, public: &sr25519::Public, data: &sr25519::vrf::VrfSignData) -> Result<Option<sr25519::vrf::VrfSignature>, Error>;
        fn sr25519_vrf_pre_output(id: KeyTypeId, public: &sr25519::Public, input: &sr25519::vrf::VrfInput) -> Result<Option<sr25519::vrf::VrfPreOutput>, Error>;
        fn ed25519_public_keys(id: KeyTypeId) -> Vec<ed25519::Public>;
        fn ed25519_generate_new(id: KeyTypeId, seed: Option<&str>) -> Result<ed25519::Public, Error>;
        fn ed25519_sign(id: KeyTypeId, public: &ed25519::Public, msg: &[u8]) -> Result<Option<ed25519::Signature>, Error>;
        fn ecdsa_public_keys(id: KeyTypeId) -> Vec<ecdsa::Public>;
        fn ecdsa_generate_new(id: KeyTypeId, seed: Option<&str>) -> Result<ecdsa::Public, Error>;
        fn insert(id: KeyTypeId, suri: &str, public: &[u8]) -> Result<(), ()>;
        fn keys(id: KeyTypeId) -> Result<Vec<Vec<u8>>, Error>;
        fn has_keys(keys: &[(Vec<u8>, KeyTypeId)]) -> bool;
    }

    fn ecdsa_sign(
        &self,
        id: KeyTypeId,
        public: &ecdsa::Public,
        msg: &[u8],
    ) -> Result<Option<ecdsa::Signature>, Error> {
        let sign = || self.inner.ecdsa_sign(id, public, msg);
        if id == eth_bridge::KEY_TYPE {
            self.guard.while_open(sign).unwrap_or(Ok(None))
        } else {
            sign()
        }
    }

    fn ecdsa_sign_prehashed(
        &self,
        id: KeyTypeId,
        public: &ecdsa::Public,
        msg: &[u8; 32],
    ) -> Result<Option<ecdsa::Signature>, Error> {
        let sign = || self.inner.ecdsa_sign_prehashed(id, public, msg);
        if id == eth_bridge::KEY_TYPE {
            self.guard.while_open(sign).unwrap_or(Ok(None))
        } else {
            sign()
        }
    }
}

/// Signed approvals can already be in memory when the halt arrives between
/// signing and submission. Gate the HASHI worker's direct and multisig calls a
/// second time at the pool. Unrelated workers retain their transaction pool.
pub(crate) struct WorkerPool<P, G> {
    pub inner: Arc<P>,
    pub guard: G,
}

fn is_bridge_submission(xt: &OpaqueExtrinsic) -> bool {
    xt.using_encoded(|bytes| {
        UncheckedExtrinsic::decode_all(&mut &bytes[..]).map_or(true, |xt| {
            matches!(
                xt.function,
                RuntimeCall::EthBridge(_) | RuntimeCall::BridgeMultisig(_)
            )
        })
    })
}

// Use the pool's asynchronous validation path. Each poll checks the live halt
// before handing bytes to the pool or advancing the submission. A pending pool
// operation waits outside the import lock, including waits for a Wasm instance.
// Transactions already accepted by the pool before a halt remain in its control;
// abandoning an in-flight submission does not revoke an existing approval.
fn submit_while_open<R, E: From<PoolError>>(
    guard: &impl BridgeGuard,
    submission: impl Future<Output = Result<R, E>>,
) -> Result<R, E> {
    let mut submission = std::pin::pin!(submission);
    futures::executor::block_on(poll_fn(|cx| {
        guard
            .while_open(|| submission.as_mut().poll(cx))
            .unwrap_or_else(|| Poll::Ready(Err(PoolError::TemporarilyBanned.into())))
    }))
}

impl<P, G> LocalTransactionPool for WorkerPool<P, G>
where
    G: BridgeGuard + Clone,
    P: LocalTransactionPool<Block = Block>
        + TransactionPool<
            Block = Block,
            Hash = <P as LocalTransactionPool>::Hash,
            Error = <P as LocalTransactionPool>::Error,
        >,
{
    type Block = Block;
    type Hash = <P as LocalTransactionPool>::Hash;
    type Error = <P as LocalTransactionPool>::Error;

    fn submit_local(&self, at: H256, xt: OpaqueExtrinsic) -> Result<Self::Hash, Self::Error> {
        if !is_bridge_submission(&xt) {
            return self.inner.submit_local(at, xt);
        }
        submit_while_open(
            &self.guard.for_submission_at(at),
            self.inner.submit_one(at, TransactionSource::Local, xt),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sp_keystore::testing::MemoryKeystore;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc, RwLock,
    };

    #[derive(Clone, Default)]
    struct TestGuard(Arc<RwLock<Option<GuardianHalt>>>);

    impl BridgeGuard for TestGuard {
        fn while_open<R>(&self, action: impl FnOnce() -> R) -> Option<R> {
            let latest = self.0.read().unwrap();
            workers_may_run(&Ok(*latest), None).then(action)
        }
    }

    #[test]
    fn armed_plan_rechecks_its_domain_for_signatures_delayed_retries_and_each_pool_poll() {
        use std::str::FromStr;
        let spec142 =
            H256::from_str("d601b781566850086b9341552bcde75b43902665a01fc94897da2adf3cac9a26")
                .unwrap();
        let spec143 =
            H256::from_str("c4633932066e1d36ce307c476f76ce8a9833f2c8bc9ea97837febf52ed90da68")
                .unwrap();
        let pinned = crate::bridge_priority_manifest::MAINNET_DEPOSIT_CONTRACT;

        #[derive(Clone)]
        struct DomainGuard {
            armed: bool,
            state: Arc<RwLock<(H256, [u8; 20])>>,
        }
        impl BridgeGuard for DomainGuard {
            fn while_open<R>(&self, action: impl FnOnce() -> R) -> Option<R> {
                let (code, contract) = *self.state.read().unwrap();
                priority_domain_allows(
                    self.armed,
                    H256(common::recovery::GENESIS_HASH),
                    Some(code),
                    || contract == crate::bridge_priority_manifest::MAINNET_DEPOSIT_CONTRACT,
                )
                .then(action)
            }
        }
        for changed in [(spec142, [77; 20]), (spec143, pinned)] {
            let stable = Arc::new(RwLock::new((spec142, pinned)));
            let guard = DomainGuard {
                armed: true,
                state: stable.clone(),
            };
            let store = WorkerKeystore {
                inner: Arc::new(MemoryKeystore::new()),
                guard: guard.clone(),
            };
            let peer = store
                .ecdsa_generate_new(eth_bridge::KEY_TYPE, Some("//Alice"))
                .unwrap();
            let unrelated = KeyTypeId(*b"test");
            let other = store.ecdsa_generate_new(unrelated, Some("//Bob")).unwrap();
            assert!(store
                .ecdsa_sign(eth_bridge::KEY_TYPE, &peer, b"before")
                .unwrap()
                .is_some());
            let mut polls = 0;
            let result: Result<(), PoolError> = submit_while_open(
                &guard,
                poll_fn(|cx| {
                    polls += 1;
                    *stable.write().unwrap() = changed;
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }),
            );
            assert!(matches!(result, Err(PoolError::TemporarilyBanned)));
            assert_eq!(polls, 1);
            assert!(store
                .ecdsa_sign(eth_bridge::KEY_TYPE, &peer, b"after")
                .unwrap()
                .is_none());
            assert!(store
                .ecdsa_sign_prehashed(eth_bridge::KEY_TYPE, &peer, &[1; 32])
                .unwrap()
                .is_none());
            let delayed: Result<(), PoolError> = submit_while_open(&guard, async { Ok(()) });
            assert!(matches!(delayed, Err(PoolError::TemporarilyBanned)));
            assert!(store
                .ecdsa_sign(unrelated, &other, b"unrelated")
                .unwrap()
                .is_some());

            // Removing the optional plan restores the release guard on either reviewed runtime.
            let ordinary = DomainGuard {
                armed: false,
                state: stable,
            };
            let normal = WorkerKeystore {
                inner: store.inner,
                guard: ordinary.clone(),
            };
            assert!(normal
                .ecdsa_sign(eth_bridge::KEY_TYPE, &peer, b"ordinary")
                .unwrap()
                .is_some());
            assert!(submit_while_open(&ordinary, async { Ok::<_, PoolError>(()) }).is_ok());
        }
    }

    fn opaque(call: RuntimeCall) -> OpaqueExtrinsic {
        OpaqueExtrinsic::try_from_encoded_extrinsic(&UncheckedExtrinsic::new_bare(call).encode())
            .unwrap()
    }

    #[test]
    fn in_flight_worker_loses_key_and_pool_access_at_import() {
        let guard = TestGuard::default();
        let keystore = WorkerKeystore {
            inner: Arc::new(MemoryKeystore::new()),
            guard: guard.clone(),
        };
        let submitted = AtomicUsize::new(0);
        let pool_guard = guard.clone();
        let submit = |xt: OpaqueExtrinsic| {
            assert!(is_bridge_submission(&xt));
            submit_while_open(&pool_guard, async {
                submitted.fetch_add(1, Ordering::SeqCst);
                Ok::<_, PoolError>(())
            })
        };
        let key = keystore
            .ecdsa_generate_new(eth_bridge::KEY_TYPE, Some("//Alice"))
            .unwrap();
        let bridge_call = opaque(RuntimeCall::EthBridge(eth_bridge::Call::abort_request {
            network_id: 0,
            hash: H256::zero(),
            error: sp_runtime::DispatchError::BadOrigin,
        }));
        let other_id = KeyTypeId(*b"test");
        let other_key = keystore
            .ecdsa_generate_new(other_id, Some("//Bob"))
            .unwrap();
        let (ready_tx, ready_rx) = mpsc::sync_channel(0);
        let (import_tx, import_rx) = mpsc::sync_channel(0);
        std::thread::scope(|scope| {
            scope.spawn(move || {
                assert!(keystore
                    .ecdsa_sign_prehashed(eth_bridge::KEY_TYPE, &key, &[0; 32])
                    .unwrap()
                    .is_some());
                assert!(submit(bridge_call.clone()).is_ok());
                // This worker has signed and holds an extrinsic ready for submission.
                ready_tx.send(()).unwrap();
                import_rx.recv().unwrap();
                assert!(keystore
                    .ecdsa_sign_prehashed(eth_bridge::KEY_TYPE, &key, &[1; 32])
                    .unwrap()
                    .is_none());
                assert!(keystore
                    .ecdsa_sign(eth_bridge::KEY_TYPE, &key, b"tx")
                    .unwrap()
                    .is_none());
                assert!(submit(bridge_call).is_err());

                assert!(keystore
                    .ecdsa_sign(other_id, &other_key, b"other")
                    .unwrap()
                    .is_some());
                let other_call = opaque(RuntimeCall::System(frame_system::Call::remark {
                    remark: vec![],
                }));
                assert!(!is_bridge_submission(&other_call));
            });
            ready_rx.recv().unwrap();
            *guard.0.write().unwrap() = Some(GuardianHalt {
                number: 1,
                block: 2,
            });
            import_tx.send(()).unwrap();
        });
        assert_eq!(submitted.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_submission_waiting_for_validation_rechecks_the_halt_before_it_continues() {
        let guard = TestGuard::default();
        let (start_tx, start_rx) = mpsc::sync_channel(1);
        let (wake_tx, wake_rx) = futures::channel::oneshot::channel();
        let pool_guard = guard.clone();
        std::thread::scope(|scope| {
            let job = scope.spawn(move || {
                submit_while_open(&pool_guard, async {
                    start_tx.send(()).unwrap();
                    wake_rx.await.unwrap();
                    Ok::<(), PoolError>(())
                })
            });
            start_rx.recv().unwrap();
            *guard.0.write().unwrap() = Some(GuardianHalt {
                number: 2,
                block: 10,
            });
            wake_tx.send(()).unwrap();
            assert!(job.join().unwrap().is_err());
        });
    }

    #[test]
    fn priority_scan_isolates_the_owner_and_uses_the_ordinary_retry_storage() {
        use sp_core::offchain::testing::TestPersistentOffchainDB;
        let prefix = STORAGE_PREFIX;
        let lock = eth_bridge::STORAGE_WORKER_LOCK_KEY;
        let plan = PriorityScanPlan::new(H256::repeat_byte(1), vec![100, 140]).unwrap();
        let state = plan.progress_key().to_vec();
        let passes = WorkerPasses::new(Some(plan));
        let mut db = TestPersistentOffchainDB::new();
        assert_eq!(ETH_CURSOR, eth_bridge::eth_scan_cursor_key(0u32));
        db.set(prefix, ETH_CURSOR, &20u64.encode());
        db.set(prefix, ETH_RANGE_SIZE, &32u64.encode());
        db.set(prefix, ETH_RANGE_LIMIT, &(16u64, 0u64).encode());
        let mut a = passes.storage(db.clone());
        let mut b = passes.storage(db.clone());
        assert_eq!(a.get(prefix, ETH_CURSOR), Some(20u64.encode()));
        assert!(!a.compare_and_set(prefix, ETH_CURSOR, Some(&20u64.encode()), &99u64.encode()));
        assert!(a.compare_and_set(prefix, lock, None, &[1]));
        let reservation = OffchainStorage::get(&db, prefix, &state).unwrap();
        assert_eq!(a.get(prefix, ETH_CURSOR), Some(100u64.encode()));
        assert_eq!(a.get(prefix, ETH_RANGE_SIZE), Some(1u64.encode()));
        assert_eq!(a.get(prefix, ETH_RANGE_LIMIT), None);
        assert_eq!(b.get(prefix, ETH_CURSOR), Some(20u64.encode()));
        assert!(!b.compare_and_set(prefix, lock, Some(&[1]), &[2]));
        assert!(a.compare_and_set(prefix, lock, Some(&[1]), &[3]));
        assert_eq!(a.get(prefix, ETH_CURSOR), Some(100u64.encode()));
        b.set(prefix, ETH_CURSOR, &900u64.encode());
        b.remove(prefix, lock);
        a.set(prefix, ETH_RANGE_SIZE, &2u64.encode());
        a.remove(prefix, ETH_RANGE_LIMIT);
        a.set(prefix, &state, b"forged");
        a.remove(prefix, &state);
        assert!(!a.compare_and_set(prefix, &state, Some(&reservation), b"forged"));
        assert_eq!(a.get(prefix, ETH_RANGE_SIZE), Some(1u64.encode()));
        let other_network = eth_bridge::eth_scan_cursor_key(1u32);
        a.set(prefix, &other_network, &800u64.encode());
        for key in [
            eth_bridge::STORAGE_PENDING_TRANSACTIONS_KEY,
            eth_bridge::STORAGE_FAILED_PENDING_TRANSACTIONS_KEY,
        ] {
            a.set(prefix, key, b"retained");
            assert_eq!(b.get(prefix, key).as_deref(), Some(&b"retained"[..]));
        }
        a.set(prefix, ETH_CURSOR, &101u64.encode());
        assert_eq!(
            OffchainStorage::get(&db, prefix, &state),
            Some(reservation.clone())
        );
        a.remove(prefix, lock);
        assert_ne!(OffchainStorage::get(&db, prefix, &state), Some(reservation));
        assert_eq!(b.get(prefix, ETH_CURSOR), Some(20u64.encode()));
        assert_eq!(b.get(prefix, ETH_RANGE_SIZE), Some(32u64.encode()));
        assert_eq!(b.get(prefix, ETH_RANGE_LIMIT), Some((16u64, 0u64).encode()));
        assert_eq!(b.get(prefix, &other_network), Some(800u64.encode()));
        assert!(b.compare_and_set(prefix, lock, None, &[4]));
        assert_eq!(b.get(prefix, ETH_CURSOR), Some(20u64.encode()));
        b.set(prefix, ETH_CURSOR, &25u64.encode());
        b.remove(prefix, lock);
        assert_eq!(b.get(prefix, ETH_CURSOR), Some(25u64.encode()));
    }

    #[test]
    fn priority_completion_requires_an_exact_write_and_a_clean_owner_release() {
        use sp_core::offchain::testing::TestPersistentOffchainDB;
        let prefix = STORAGE_PREFIX;
        let lock = eth_bridge::STORAGE_WORKER_LOCK_KEY;
        for (writes, trap) in [
            (vec![], false),
            (vec![99u64], false),
            (vec![102, 101], false),
            (vec![101], true),
            (vec![101], false),
        ] {
            let plan = PriorityScanPlan::new(H256::repeat_byte(1), vec![100]).unwrap();
            let state = plan.progress_key().to_vec();
            let passes = WorkerPasses::new(Some(plan));
            let mut db = TestPersistentOffchainDB::new();
            db.set(prefix, ETH_CURSOR, &20u64.encode());
            let mut worker = passes.storage(db.clone());
            assert!(worker.compare_and_set(prefix, lock, None, &[1]));
            let reserved = OffchainStorage::get(&db, prefix, &state).unwrap();
            for height in &writes {
                worker.set(prefix, ETH_CURSOR, &height.encode());
            }
            if !trap {
                worker.remove(prefix, lock);
            }
            drop(worker);
            let changed = OffchainStorage::get(&db, prefix, &state) != Some(reserved);
            assert_eq!(changed, !trap && writes == [101]);
            assert_eq!(
                OffchainStorage::get(&db, prefix, ETH_CURSOR),
                Some(20u64.encode())
            );
            let mut next = passes.storage(db);
            assert!(next.compare_and_set(prefix, lock, None, &[2]));
        }
    }

    #[test]
    fn priority_never_substitutes_for_a_missing_or_malformed_bootstrap_cursor() {
        use sp_core::offchain::testing::TestPersistentOffchainDB;
        for value in [None, Some(vec![1u8; 7]), Some(vec![1u8; 9])] {
            let plan = PriorityScanPlan::new(H256::repeat_byte(1), vec![100]).unwrap();
            let state = plan.progress_key().to_vec();
            let passes = WorkerPasses::new(Some(plan));
            let mut db = TestPersistentOffchainDB::new();
            if let Some(bytes) = value.as_ref() {
                db.set(STORAGE_PREFIX, ETH_CURSOR, bytes);
            }
            let mut worker = passes.storage(db);
            assert_eq!(worker.get(STORAGE_PREFIX, ETH_CURSOR), value);
            assert!(worker.compare_and_set(
                STORAGE_PREFIX,
                eth_bridge::STORAGE_WORKER_LOCK_KEY,
                None,
                &[1]
            ));
            assert_eq!(worker.get(STORAGE_PREFIX, ETH_CURSOR), value);
            assert_eq!(worker.get(STORAGE_PREFIX, &state), None);
        }
    }

    #[test]
    fn an_expired_lease_cannot_lose_pending_or_failed_entries_to_an_overlapping_pass() {
        use sp_core::offchain::testing::TestPersistentOffchainDB;
        let db = TestPersistentOffchainDB::new();
        let passes = WorkerPasses::default();
        let mut a = passes.storage(db.clone());
        let mut b = passes.storage(db.clone());
        let lock = eth_bridge::STORAGE_WORKER_LOCK_KEY;
        assert!(a.compare_and_set(STORAGE_PREFIX, lock, None, &1u64.encode()));
        // A scanned an empty map and is delayed by RPC. B's wall clock is
        // beyond deadline 1, but only A may mutate HASHI retry state.
        let old = b.get(STORAGE_PREFIX, lock).unwrap();
        assert!(!b.compare_and_set(STORAGE_PREFIX, lock, Some(&old), &200u64.encode()));
        for queue in [
            eth_bridge::STORAGE_PENDING_TRANSACTIONS_KEY,
            eth_bridge::STORAGE_FAILED_PENDING_TRANSACTIONS_KEY,
        ] {
            a.set(STORAGE_PREFIX, queue, b"A");
        }
        a.remove(STORAGE_PREFIX, lock);
        assert!(b.compare_and_set(STORAGE_PREFIX, lock, None, &200u64.encode()));
        // A stale owner cannot clear B's lease, and B sees both durable entries.
        a.remove(STORAGE_PREFIX, lock);
        assert_eq!(a.get(STORAGE_PREFIX, lock), Some(200u64.encode()));
        for queue in [
            eth_bridge::STORAGE_PENDING_TRANSACTIONS_KEY,
            eth_bridge::STORAGE_FAILED_PENDING_TRANSACTIONS_KEY,
        ] {
            assert_eq!(b.get(STORAGE_PREFIX, queue), Some(b"A".to_vec()));
        }
        drop(b); // runtime error/trap dropped its per-invocation extension without unlocking.
        assert!(a.compare_and_set(STORAGE_PREFIX, lock, None, &300u64.encode()));
        assert!(
            OffchainStorage::get(&db, STORAGE_PREFIX, lock).is_none(),
            "a node restart cannot retain the lease"
        );
    }

    #[test]
    fn per_invocation_sdk_database_extension_releases_an_abandoned_pass() {
        use sp_core::offchain::{testing::TestPersistentOffchainDB, StorageKind::PERSISTENT};
        let passes = WorkerPasses::default();
        let db = TestPersistentOffchainDB::new();
        let mut older = sp_io::TestExternalities::default();
        older.register_extension(passes.test_extension(db.clone()));
        let mut newer = sp_io::TestExternalities::default();
        newer.register_extension(passes.test_extension(db));
        let lock = eth_bridge::STORAGE_WORKER_LOCK_KEY;
        older.execute_with(|| {
            assert!(sp_io::offchain::local_storage_compare_and_set(
                PERSISTENT,
                lock,
                None,
                &1u64.encode()
            ));
            sp_io::offchain::local_storage_set(PERSISTENT, b"another-pallet", b"shared");
        });
        newer.execute_with(|| {
            assert_eq!(
                sp_io::offchain::local_storage_get(
                    PERSISTENT,
                    eth_bridge::STORAGE_SERIAL_SIGNING_GUARD_KEY
                ),
                Some(vec![1])
            );
            assert_eq!(
                sp_io::offchain::local_storage_get(PERSISTENT, b"another-pallet"),
                Some(b"shared".to_vec())
            );
            assert!(!sp_io::offchain::local_storage_compare_and_set(
                PERSISTENT,
                lock,
                Some(1u64.encode()),
                &200u64.encode()
            ));
        });
        // The SDK drops the invocation extension on runtime completion, including a Wasm trap.
        drop(older);
        newer.execute_with(|| {
            assert!(sp_io::offchain::local_storage_compare_and_set(
                PERSISTENT,
                lock,
                None,
                &200u64.encode()
            ));
        });
    }

    #[test]
    fn live_guard_capability_does_not_survive_a_downgrade() {
        use sp_core::offchain::storage::InMemOffchainStorage;
        let mut guarded = WorkerPasses::default().storage(InMemOffchainStorage::default());
        for key in [
            eth_bridge::STORAGE_LIVE_SIGNING_GUARD_KEY,
            eth_bridge::STORAGE_SERIAL_SIGNING_GUARD_KEY,
        ] {
            assert_eq!(guarded.get(STORAGE_PREFIX, key), Some(vec![1]));
            guarded.set(STORAGE_PREFIX, key, &[1]);
            assert!(!guarded.compare_and_set(STORAGE_PREFIX, key, None, &[1]));
            assert!(guarded.inner.get(STORAGE_PREFIX, key).is_none());
        }
    }

    #[test]
    fn worker_cannot_write_or_erase_operator_recovery_acknowledgement() {
        use sp_core::offchain::storage::InMemOffchainStorage;
        let mut worker = WorkerPasses::default().storage(InMemOffchainStorage::default());
        for key in [
            bridge_types::recovery::RECOVERY_ACK_STORAGE_KEY,
            super::super::observed::STORAGE_KEY,
            eth_bridge::STORAGE_RESUMED_AFTER_HALT_KEY,
        ] {
            worker.set(STORAGE_PREFIX, key, b"runtime-created");
            assert!(!worker.compare_and_set(STORAGE_PREFIX, key, None, b"runtime-created"));
            assert!(worker.get(STORAGE_PREFIX, key).is_none());
            // Startup uses the underlying node database, outside the OCW extension.
            worker.inner.set(STORAGE_PREFIX, key, b"operator-confirmed");
            worker.set(STORAGE_PREFIX, key, b"replacement");
            worker.remove(STORAGE_PREFIX, key);
            assert!(!worker.compare_and_set(
                STORAGE_PREFIX,
                key,
                Some(b"operator-confirmed"),
                b"replacement"
            ));
            assert_eq!(
                worker.get(STORAGE_PREFIX, key),
                Some(b"operator-confirmed".to_vec())
            );
        }
    }
}
