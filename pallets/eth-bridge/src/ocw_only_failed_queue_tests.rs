//! Exercise the actual secondary retry queue, not only SignedTransactionData::resend.

use super::*;
fn activate() {
    common::recovery::bridge_admission_fixtures::complete([1; 32]);
    sp_io::storage::set(common::recovery::BRIDGE_HALT_STORAGE_KEY, &[1]);
    let completion = bridge_types::recovery::completion().unwrap().unwrap();
    let genesis: [u8; 32] = System::block_hash(0).into();
    let ack = bridge_types::recovery::RecoveryAcknowledgement::new(genesis, &completion);
    sp_io::offchain::local_storage_set(
        sp_core::offchain::StorageKind::PERSISTENT,
        bridge_types::recovery::RECOVERY_ACK_STORAGE_KEY,
        &ack.encode(),
    );
}
use crate::tests::mock::*;
use frame_support::dispatch::GetDispatchInfo;
use sp_core::{crypto::KeyTypeId, ecdsa, ed25519, sr25519};
use sp_keystore::{Error as KeystoreError, Keystore, KeystoreExt};

/// Every operation fails the test, including key enumeration. Rejected cache
/// entries must be classified before they access any signing capability.
struct NoKeyAccess;

macro_rules! deny_key_access {
    ($(fn $name:ident($($arg:ident: $ty:ty),*) -> $result:ty;)*) => {
        $(fn $name(&self, $($arg: $ty),*) -> $result {
            $(let _ = $arg;)*
            panic!("rejected retry accessed keystore: {}", stringify!($name));
        })*
    };
}

impl Keystore for NoKeyAccess {
    deny_key_access! {
        fn sr25519_public_keys(key_type: KeyTypeId) -> Vec<sr25519::Public>;
        fn sr25519_generate_new(key_type: KeyTypeId, seed: Option<&str>) -> Result<sr25519::Public, KeystoreError>;
        fn sr25519_sign(key_type: KeyTypeId, public: &sr25519::Public, msg: &[u8]) -> Result<Option<sr25519::Signature>, KeystoreError>;
        fn sr25519_vrf_sign(key_type: KeyTypeId, public: &sr25519::Public, data: &sr25519::vrf::VrfSignData) -> Result<Option<sr25519::vrf::VrfSignature>, KeystoreError>;
        fn sr25519_vrf_pre_output(key_type: KeyTypeId, public: &sr25519::Public, input: &sr25519::vrf::VrfInput) -> Result<Option<sr25519::vrf::VrfPreOutput>, KeystoreError>;
        fn ed25519_public_keys(key_type: KeyTypeId) -> Vec<ed25519::Public>;
        fn ed25519_generate_new(key_type: KeyTypeId, seed: Option<&str>) -> Result<ed25519::Public, KeystoreError>;
        fn ed25519_sign(key_type: KeyTypeId, public: &ed25519::Public, msg: &[u8]) -> Result<Option<ed25519::Signature>, KeystoreError>;
        fn ecdsa_public_keys(key_type: KeyTypeId) -> Vec<ecdsa::Public>;
        fn ecdsa_generate_new(key_type: KeyTypeId, seed: Option<&str>) -> Result<ecdsa::Public, KeystoreError>;
        fn ecdsa_sign(key_type: KeyTypeId, public: &ecdsa::Public, msg: &[u8]) -> Result<Option<ecdsa::Signature>, KeystoreError>;
        fn ecdsa_sign_prehashed(key_type: KeyTypeId, public: &ecdsa::Public, msg: &[u8; 32]) -> Result<Option<ecdsa::Signature>, KeystoreError>;
        fn insert(key_type: KeyTypeId, suri: &str, public: &[u8]) -> Result<(), ()>;
        fn keys(key_type: KeyTypeId) -> Result<Vec<Vec<u8>>, KeystoreError>;
        fn has_keys(public_keys: &[(Vec<u8>, KeyTypeId)]) -> bool;
    }
}

fn import(network: u32) -> RuntimeCall {
    let timepoint = bridge_multisig::Pallet::<Runtime>::sidechain_timepoint(100, 1);
    crate::Call::<Runtime>::import_incoming_request {
        load_incoming_request: LoadIncomingRequest::Transaction(
            LoadIncomingTransactionRequest::new(
                get_account_id_from_seed::<sr25519::Public>("Alice"),
                H256::repeat_byte(61),
                timepoint,
                IncomingTransactionRequestKind::Transfer,
                network,
            ),
        ),
        incoming_request_result: Err(crate::Error::<Runtime>::EthTransactionIsFailed.into()),
    }
    .into()
}

fn wrap(bytes: Vec<u8>) -> RuntimeCall {
    bridge_multisig::Call::<Runtime>::as_multi {
        id: crate::BridgeAccount::<Runtime>::get(0).unwrap(),
        maybe_timepoint: Some(bridge_multisig::Pallet::<Runtime>::sidechain_timepoint(
            100, 1,
        )),
        call: bytes,
        store_call: true,
        max_weight: import(0).get_dispatch_info().total_weight(),
    }
    .into()
}

fn cache(call: RuntimeCall) {
    let hash = H256::repeat_byte(62);
    StorageValueRef::persistent(STORAGE_FAILED_PENDING_TRANSACTIONS_KEY).set(&BTreeMap::from([(
        hash,
        SignedTransactionData::<Runtime>::new(hash, None, call),
    )]));
}

#[test]
fn ocw_only_failed_queue_preserves_malformed_and_wrong_calls_without_key_access() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.register_extension(KeystoreExt::new(NoKeyAccess));
    ext.execute_with(|| {
        activate();
        let encoded = import(0).encode();
        let mut trailing = encoded.clone();
        trailing.push(0);
        let remark: RuntimeCall = frame_system::Call::remark { remark: vec![1] }.into();
        let mut mismatched_import = import(0);
        let RuntimeCall::EthBridge(crate::Call::import_incoming_request {
            incoming_request_result,
            ..
        }) = &mut mismatched_import
        else {
            panic!("import fixture");
        };
        *incoming_request_result = Ok(crate::IncomingRequest::PrepareForMigration(
            crate::IncomingPrepareForMigration {
                author: get_account_id_from_seed::<sr25519::Public>("Alice"),
                tx_hash: H256::repeat_byte(61),
                at_height: 100,
                timepoint: Default::default(),
                network_id: 1,
            },
        ));
        let calls = vec![
            wrap(mismatched_import.encode()),
            wrap(vec![]),
            wrap(encoded[..1].to_vec()),
            wrap(encoded[..encoded.len() - 1].to_vec()),
            wrap(vec![255]),
            wrap(trailing),
            wrap(remark.encode()),
            wrap(import(1).encode()),
            bridge_multisig::Call::<Runtime>::as_multi_threshold_1 {
                id: crate::BridgeAccount::<Runtime>::get(0).unwrap(),
                call: Box::new(remark.clone()),
                timepoint: Default::default(),
            }
            .into(),
            remark,
        ];
        for call in calls {
            cache(call);
            let before = state.failed_pending_txs().encode();
            let chain = sp_io::storage::root(sp_runtime::StateVersion::V1);
            EthBridge::handle_failed_transactions_queue();
            assert_eq!(state.failed_pending_txs().encode(), before);
            assert!(state.pending_txs().is_empty());
            assert!(state.pool_state.read().transactions.is_empty());
            assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), chain);
        }
    });
}

#[test]
fn ocw_only_failed_queue_still_retries_valid_network_zero_import() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        activate();
        let call = wrap(import(0).encode());
        cache(call.clone());
        let before = state.failed_pending_txs().encode();
        EthBridge::handle_failed_transactions_queue();
        assert_eq!(state.failed_pending_txs().encode(), before);
        assert_eq!(state.pending_txs().len(), 1);
        let transactions = state.pool_state.read().transactions.clone();
        assert_eq!(transactions.len(), 1);
        let submitted = TestExtrinsic::decode(&mut &transactions[0][..]).unwrap();
        assert_eq!(submitted.function, call);
        let signer = submitted.preamble.to_signed().unwrap().0;
        assert!(crate::Peers::<Runtime>::get(0).contains(&signer));
    });
}

#[test]
fn ocw_only_failed_queue_network_mismatch_rejects_before_any_key_access() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.register_extension(KeystoreExt::new(NoKeyAccess));
    ext.execute_with(|| {
        activate();
        let request = crate::requests::OffchainRequest::incoming(
            crate::IncomingRequest::PrepareForMigration(crate::IncomingPrepareForMigration {
                author: get_account_id_from_seed::<sr25519::Public>("Alice"),
                tx_hash: H256::repeat_byte(63),
                at_height: 1,
                timepoint: Default::default(),
                network_id: 1,
            }),
        );
        let hash = request.hash();
        crate::Requests::<Runtime>::insert(0, hash, request);
        let calls = vec![
            crate::Call::<Runtime>::approve_request {
                ocw_public: ecdsa::Public::from_raw([2; 33]),
                hash,
                signature_params: Default::default(),
                network_id: 0,
            },
            crate::Call::<Runtime>::abort_request {
                hash,
                error: crate::Error::<Runtime>::EthTransactionIsFailed.into(),
                network_id: 0,
            },
            crate::Call::<Runtime>::finalize_incoming_request {
                hash,
                network_id: 0,
            },
        ];
        for call in calls {
            let before = sp_io::storage::root(sp_runtime::StateVersion::V1);
            assert!(matches!(
                EthBridge::send_transaction(call),
                Err(crate::Error::<Runtime>::UnknownNetwork)
            ));
            assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), before);
            assert!(state.pool_state.read().transactions.is_empty());
            assert!(state.pending_txs().is_empty());
        }
    });
}
