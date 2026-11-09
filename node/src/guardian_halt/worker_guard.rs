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
    sync::Arc,
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

/// The runtime can distinguish a guarded node from an older host using the same
/// Wasm imports. This key is synthesized only for the OCW, outside the real DB.
#[derive(Clone)]
pub(crate) struct WorkerStorage<S>(pub S);

impl<S: OffchainStorage> OffchainStorage for WorkerStorage<S> {
    fn get(&self, prefix: &[u8], key: &[u8]) -> Option<Vec<u8>> {
        if prefix == STORAGE_PREFIX && key == eth_bridge::STORAGE_LIVE_SIGNING_GUARD_KEY {
            Some(vec![1])
        } else {
            self.0.get(prefix, key)
        }
    }

    fn set(&mut self, prefix: &[u8], key: &[u8], value: &[u8]) {
        if key != eth_bridge::STORAGE_LIVE_SIGNING_GUARD_KEY {
            self.0.set(prefix, key, value);
        }
    }

    fn remove(&mut self, prefix: &[u8], key: &[u8]) {
        self.0.remove(prefix, key)
    }

    fn compare_and_set(
        &mut self,
        prefix: &[u8],
        key: &[u8],
        old: Option<&[u8]>,
        new: &[u8],
    ) -> bool {
        key != eth_bridge::STORAGE_LIVE_SIGNING_GUARD_KEY
            && self.0.compare_and_set(prefix, key, old, new)
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
    fn live_guard_capability_does_not_survive_a_downgrade() {
        use sp_core::offchain::storage::InMemOffchainStorage;
        let mut guarded = WorkerStorage(InMemOffchainStorage::default());
        let key = eth_bridge::STORAGE_LIVE_SIGNING_GUARD_KEY;
        assert_eq!(guarded.get(STORAGE_PREFIX, key), Some(vec![1]));
        guarded.set(STORAGE_PREFIX, key, &[1]);
        assert!(!guarded.compare_and_set(STORAGE_PREFIX, key, None, &[1]));
        assert!(guarded.0.get(STORAGE_PREFIX, key).is_none());
    }
}
