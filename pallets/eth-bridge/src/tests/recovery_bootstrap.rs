//! Recovery bootstrap may prepare a peer locally without opening the bridge halt.

use crate::tests::mock::{BlockNumber, EthBridge, ExtBuilder, Runtime};
use crate::{STORAGE_BOOTSTRAP_READY_KEY, STORAGE_NETWORK_IDS_KEY, STORAGE_PEER_MARKER_KEY};
use frame_support::traits::Hooks;
use sp_runtime::offchain::storage::StorageValueRef;
use std::collections::BTreeSet;

#[test]
fn initialized_peer_stays_idle_while_recovery_halt_exists_without_worker_lock() {
    let (mut ext, mut state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        // The fixture supplies actual local signing keys, peer membership, network IDs
        // and RPC configuration. Keep only the public marker used by node bootstrap.
        StorageValueRef::persistent(crate::STORAGE_PEER_SECRET_KEY).clear();
        assert!(EthBridge::get_signer().is_ok());
        let marker = StorageValueRef::persistent(STORAGE_PEER_MARKER_KEY)
            .get::<Vec<u8>>()
            .unwrap()
            .expect("initialized peer marker");
        let networks = StorageValueRef::persistent(STORAGE_NETWORK_IDS_KEY)
            .get::<BTreeSet<u32>>()
            .unwrap()
            .expect("initialized peer networks");
        assert!(!marker.is_empty());
        assert!(!networks.is_empty());
        let local_accounts = EthBridge::get_keystore_accounts();
        assert!(networks.iter().all(|network| {
            crate::Peers::<Runtime>::get(network)
                .iter()
                .any(|peer| local_accounts.contains(peer))
        }));
        StorageValueRef::persistent(STORAGE_BOOTSTRAP_READY_KEY).set(&1u64);

        // No worker lock is held. Both values must halt by key existence, so
        // nothing may depend on an absent marker, missing key or boolean decoding.
        for value in [1u8, 0u8] {
            sp_io::storage::set(crate::recovery::HALT_KEY, &[value]);
            <EthBridge as Hooks<BlockNumber>>::offchain_worker(1);
            EthBridge::offchain();

            assert!(state.http_requests().is_empty());
            assert!(state.pool_state.read().transactions.is_empty());
            assert_eq!(
                sp_io::storage::get(crate::recovery::HALT_KEY).as_deref(),
                Some(&[value][..])
            );
            assert_eq!(
                StorageValueRef::persistent(STORAGE_PEER_MARKER_KEY)
                    .get::<Vec<u8>>()
                    .unwrap(),
                Some(marker.clone())
            );
            assert_eq!(
                StorageValueRef::persistent(STORAGE_NETWORK_IDS_KEY)
                    .get::<BTreeSet<u32>>()
                    .unwrap(),
                Some(networks.clone())
            );
            assert_eq!(
                StorageValueRef::persistent(STORAGE_BOOTSTRAP_READY_KEY)
                    .get::<u64>()
                    .unwrap(),
                Some(1)
            );
        }

        // Remove the latch only inside this test's in-memory chain state. The
        // helper queues deterministic local mock responses, never external I/O.
        // The same prepared marker and network IDs must now drive the real hook.
        sp_io::storage::clear(crate::recovery::HALT_KEY);
        state.run_next_offchain_worker_with_params(0, 0, false);
        let methods: Vec<_> = state
            .http_requests()
            .into_iter()
            .map(|request| {
                request["method"]
                    .as_str()
                    .expect("mock JSON-RPC method")
                    .to_owned()
            })
            .collect();
        assert_eq!(
            methods,
            [
                "chain_getFinalizedHead",
                "chain_getHeader",
                "chain_getBlockHash",
                "chain_getBlock",
                "eth_blockNumber",
            ]
        );
        assert!(state.pool_state.read().transactions.is_empty());
        assert!(!sp_io::storage::exists(crate::recovery::HALT_KEY));
    });
}
