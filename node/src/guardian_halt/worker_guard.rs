// This file is part of the SORA network and Polkaswap app.
// SPDX-License-Identifier: BSD-4-Clause

//! An OCW executes against the block that spawned it, even after another block
//! has been imported. Serialize HASHI key use and local submission with imports
//! and consult the node's current best state at the point of each operation.

use super::halt_at;
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
}

pub(crate) trait BridgeGuard: Send + Sync {
    fn while_open<R>(&self, action: impl FnOnce() -> R) -> Option<R>;
}

impl BridgeGuard for WorkerGuard {
    fn while_open<R>(&self, action: impl FnOnce() -> R) -> Option<R> {
        // The OCW already occupies a Wasm instance. Yield to an active or queued
        // importer instead of waiting for the lock while holding that instance.
        let _import = self.backend.get_import_lock().try_read()?;
        let best = self.client.info().best_hash;
        let latest = halt_at(&self.client, best).ok()?;
        let storage = self.backend.offchain_storage()?;
        let confirmed = storage
            .get(STORAGE_PREFIX, eth_bridge::STORAGE_RESUMED_AFTER_HALT_KEY)
            .and_then(|bytes| GuardianHalt::decode_all(&mut &bytes[..]).ok());
        if !workers_may_run(&Ok(latest), confirmed) {
            return None;
        }
        let root_latch = StorageKey(common::recovery::BRIDGE_HALT_STORAGE_KEY.to_vec());
        if self.client.storage(best, &root_latch).ok()?.is_some() {
            return None;
        }
        Some(action())
    }
}

/// Shared by every invocation, unlike the token assigned to each OCW extension.
/// The weak owner expires even if Wasm traps and never drops its StorageLockGuard.
#[derive(Clone, Default)]
pub(crate) struct WorkerPasses(Arc<Mutex<Option<PassLock>>>);

struct PassLock {
    owner: Weak<()>,
    deadline: Vec<u8>,
}

impl WorkerPasses {
    fn storage<S>(&self, inner: S) -> WorkerStorage<S> {
        WorkerStorage {
            inner,
            passes: self.clone(),
            invocation: Arc::new(()),
        }
    }

    pub(crate) fn extension<S: OffchainStorage + 'static>(
        &self,
        inner: S,
    ) -> Box<dyn sp_externalities::Extension> {
        use sp_core::offchain::{Capabilities, LimitedExternalities, OffchainDbExt};
        Box::new(OffchainDbExt::new(LimitedExternalities::new(
            Capabilities::all(),
            sc_offchain::OffchainDb::new(self.storage(inner)),
        )))
    }
}

#[derive(Clone)]
struct WorkerStorage<S> {
    inner: S,
    passes: WorkerPasses,
    invocation: Arc<()>,
}

fn guard_capability(key: &[u8]) -> bool {
    key == eth_bridge::STORAGE_LIVE_SIGNING_GUARD_KEY
        || key == eth_bridge::STORAGE_SERIAL_SIGNING_GUARD_KEY
}

impl<S: OffchainStorage> WorkerStorage<S> {
    fn acquire(&self, old: Option<&[u8]>, new: &[u8]) -> bool {
        let mut slot = self.passes.0.lock().expect("HASHI worker pass lock");
        let active = slot.as_ref().filter(|pass| pass.owner.strong_count() != 0);
        if active.map(|pass| pass.deadline.as_slice()) != old
            || active.is_some_and(|pass| !pass.owner.ptr_eq(&Arc::downgrade(&self.invocation)))
        {
            return false;
        }
        *slot = Some(PassLock {
            owner: Arc::downgrade(&self.invocation),
            deadline: new.to_vec(),
        });
        true
    }
}

impl<S: OffchainStorage> OffchainStorage for WorkerStorage<S> {
    fn get(&self, prefix: &[u8], key: &[u8]) -> Option<Vec<u8>> {
        if prefix == STORAGE_PREFIX && guard_capability(key) {
            return Some(vec![1]);
        }
        if prefix == STORAGE_PREFIX && key == eth_bridge::STORAGE_WORKER_LOCK_KEY {
            return self
                .passes
                .0
                .lock()
                .expect("HASHI worker pass lock")
                .as_ref()
                .filter(|pass| pass.owner.strong_count() != 0)
                .map(|pass| pass.deadline.clone());
        }
        self.inner.get(prefix, key)
    }

    fn set(&mut self, prefix: &[u8], key: &[u8], value: &[u8]) {
        if prefix != STORAGE_PREFIX
            || (!guard_capability(key) && key != eth_bridge::STORAGE_WORKER_LOCK_KEY)
        {
            self.inner.set(prefix, key, value);
        }
    }

    fn remove(&mut self, prefix: &[u8], key: &[u8]) {
        if prefix == STORAGE_PREFIX && key == eth_bridge::STORAGE_WORKER_LOCK_KEY {
            let mut slot = self.passes.0.lock().expect("HASHI worker pass lock");
            if slot
                .as_ref()
                .is_some_and(|pass| pass.owner.ptr_eq(&Arc::downgrade(&self.invocation)))
            {
                *slot = None;
            }
        } else if prefix != STORAGE_PREFIX || !guard_capability(key) {
            self.inner.remove(prefix, key);
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
        (prefix != STORAGE_PREFIX || !guard_capability(key))
            && self.inner.compare_and_set(prefix, key, old, new)
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
    G: BridgeGuard,
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
            &self.guard,
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
    fn an_expired_lease_cannot_lose_pending_or_failed_entries_to_an_overlapping_pass() {
        use sp_core::offchain::storage::InMemOffchainStorage;
        let db = InMemOffchainStorage::default();
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
            db.get(STORAGE_PREFIX, lock).is_none(),
            "a node restart cannot retain the lease"
        );
    }

    #[test]
    fn per_invocation_sdk_database_extension_releases_an_abandoned_pass() {
        use sp_core::offchain::{storage::InMemOffchainStorage, StorageKind::PERSISTENT};
        let passes = WorkerPasses::default();
        let db = InMemOffchainStorage::default();
        let mut older = sp_io::TestExternalities::default();
        older.register_extension(passes.extension(db.clone()));
        let mut newer = sp_io::TestExternalities::default();
        newer.register_extension(passes.extension(db));
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
}
