use super::coverage_paths::{deposit_log, header, outgoing};
use super::*;
use crate::requests::{OutgoingRemovePeer, RequestStatus};
use crate::tests::{mock::*, Error};
use crate::types::{self, Log, Transaction};
use common::mock::alice;
use ethabi::Token;
use frame_support::assert_ok;
use sp_core::{crypto::KeyTypeId, ecdsa, ed25519, sr25519, H160};
use sp_keystore::{testing::MemoryKeystore, Error as KeystoreError, Keystore, KeystoreExt};
use std::sync::atomic::{AtomicUsize, Ordering};

/// A real memory keystore with one failure at the host boundary: the key may be
/// removed between enumeration and signing, or its signer may be unavailable.
struct UnavailableKeystore {
    inner: MemoryKeystore,
    reads: AtomicUsize,
    visible_reads: usize,
    fail_prehash: bool,
}

macro_rules! forward_keystore {
    ($(fn $name:ident($($arg:ident: $ty:ty),*) -> $result:ty;)*) => {
        $(fn $name(&self, $($arg: $ty),*) -> $result {
            self.inner.$name($($arg),*)
        })*
    };
}

impl Keystore for UnavailableKeystore {
    forward_keystore! {
        fn sr25519_public_keys(key_type: KeyTypeId) -> Vec<sr25519::Public>;
        fn sr25519_generate_new(key_type: KeyTypeId, seed: Option<&str>) -> Result<sr25519::Public, KeystoreError>;
        fn sr25519_sign(key_type: KeyTypeId, public: &sr25519::Public, msg: &[u8]) -> Result<Option<sr25519::Signature>, KeystoreError>;
        fn sr25519_vrf_sign(key_type: KeyTypeId, public: &sr25519::Public, data: &sr25519::vrf::VrfSignData) -> Result<Option<sr25519::vrf::VrfSignature>, KeystoreError>;
        fn sr25519_vrf_pre_output(key_type: KeyTypeId, public: &sr25519::Public, input: &sr25519::vrf::VrfInput) -> Result<Option<sr25519::vrf::VrfPreOutput>, KeystoreError>;
        fn ed25519_public_keys(key_type: KeyTypeId) -> Vec<ed25519::Public>;
        fn ed25519_generate_new(key_type: KeyTypeId, seed: Option<&str>) -> Result<ed25519::Public, KeystoreError>;
        fn ed25519_sign(key_type: KeyTypeId, public: &ed25519::Public, msg: &[u8]) -> Result<Option<ed25519::Signature>, KeystoreError>;
        fn ecdsa_generate_new(key_type: KeyTypeId, seed: Option<&str>) -> Result<ecdsa::Public, KeystoreError>;
        fn ecdsa_sign(key_type: KeyTypeId, public: &ecdsa::Public, msg: &[u8]) -> Result<Option<ecdsa::Signature>, KeystoreError>;
        fn insert(key_type: KeyTypeId, suri: &str, public: &[u8]) -> Result<(), ()>;
        fn keys(key_type: KeyTypeId) -> Result<Vec<Vec<u8>>, KeystoreError>;
        fn has_keys(public_keys: &[(Vec<u8>, KeyTypeId)]) -> bool;
    }

    fn ecdsa_public_keys(&self, key_type: KeyTypeId) -> Vec<ecdsa::Public> {
        if self.reads.fetch_add(1, Ordering::SeqCst) < self.visible_reads {
            self.inner.ecdsa_public_keys(key_type)
        } else {
            vec![]
        }
    }

    fn ecdsa_sign_prehashed(
        &self,
        key_type: KeyTypeId,
        public: &ecdsa::Public,
        msg: &[u8; 32],
    ) -> Result<Option<ecdsa::Signature>, KeystoreError> {
        if self.fail_prehash {
            Err(KeystoreError::Unavailable)
        } else {
            self.inner.ecdsa_sign_prehashed(key_type, public, msg)
        }
    }
}

fn install_unavailable_keystore(
    ext: &mut sp_io::TestExternalities,
    state: &State,
    visible_reads: usize,
    fail_prehash: bool,
) {
    let key = &state.networks[&0].ocw_keypairs[0];
    let inner = MemoryKeystore::new();
    inner
        .insert(
            crate::KEY_TYPE,
            &format!("0x{}", key.2.to_hex::<String>()),
            key.0.as_ref(),
        )
        .unwrap();
    ext.register_extension(KeystoreExt::new(UnavailableKeystore {
        inner,
        reads: AtomicUsize::new(0),
        visible_reads,
        fail_prehash,
    }));
}

#[test]
fn outgoing_signing_failure_and_key_removal_are_distinct_from_submission_failure() {
    for (visible_reads, fail_prehash) in [(usize::MAX, true), (2, false)] {
        let (mut ext, state) = ExtBuilder::default().build();
        install_unavailable_keystore(&mut ext, &state, visible_reads, fail_prehash);
        ext.execute_with(|| {
            let request = outgoing();
            let hash = OffchainRequest::outgoing(request.clone()).hash();
            assert_eq!(
                EthBridge::handle_outgoing_request(request, hash),
                Err(Error::FailedToSignMessage)
            );
            let metric = EthBridge::per_network_reason_metric_key(
                STORAGE_OUTGOING_APPROVAL_FAILURES_KEY,
                0,
                OUTGOING_APPROVAL_FAILURE_FAILED_SIGN,
            );
            assert_eq!(
                StorageValueRef::persistent(&metric).get::<u64>().unwrap(),
                Some(1)
            );
            assert!(state.pending_txs().is_empty());
            assert!(state.pool_state.read().transactions.is_empty());
        });
    }
}

#[test]
fn failed_signed_transaction_construction_never_creates_or_replaces_retry_records() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let call = frame_system::Call::<Runtime>::remark { remark: vec![1] };
        set_signed_transaction_creation_results(&[false]);
        assert_eq!(
            EthBridge::send_transaction(call.clone()),
            Err(Error::NoLocalAccountForSigning)
        );
        assert!(state.pending_txs().is_empty());
        let signer = EthBridge::get_signer().unwrap();
        let account = Signer::<Runtime, crate::offchain::crypto::TestAuthId>::keystore_accounts()
            .next()
            .unwrap();
        let mut tx =
            SignedTransactionData::<Runtime>::new(H256::repeat_byte(1), None, call.clone());
        let original = tx.encode();
        set_signed_transaction_creation_results(&[true, false]);
        assert!(!tx.resend(&signer));
        assert_eq!(tx.encode(), original);
        set_signed_transaction_creation_results(&[false]);
        EthBridge::add_pending_extrinsic(call, &account, true);
        assert!(state.pending_txs().is_empty());
        assert_eq!(state.pool_state.read().transactions.len(), 1);
        set_signed_transaction_creation_results(&[false]);
        let request = outgoing();
        let hash = OffchainRequest::outgoing(request.clone()).hash();
        assert_eq!(
            EthBridge::handle_outgoing_request(request, hash),
            Err(Error::FailedToSendSignedTransaction)
        );
        assert!(state.pending_txs().is_empty());
    });
}

#[test]
fn http_start_deadline_and_body_errors_are_retryable_and_do_not_advance_scan() {
    for fault in [HttpFault::Start, HttpFault::Deadline, HttpFault::ReadBody] {
        let (mut ext, _) = ExtBuilder::default().build();
        ext.execute_with(|| {
            set_http_fault(Some(fault));
            if fault != HttpFault::Start {
                push_global_json_rpc_response(Vec::<Log>::new());
            }
            let mut cursor = 5;
            assert_eq!(
                EthBridge::handle_logs(5, 7, &mut cursor, 0),
                Err(Error::HttpFetchingError)
            );
            assert_eq!(cursor, 5);
            set_http_fault(None);
            push_global_json_rpc_response(Vec::<Log>::new());
            assert_ok!(EthBridge::handle_logs(5, 7, &mut cursor, 0));
            assert_eq!(cursor, 8);
        });
    }
}

#[test]
fn single_block_provider_limits_do_not_advance_cursor_or_treat_quota_as_size_limit() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        StorageValueRef::persistent(b"eth-bridge-ocw::eth-to-handle-from-height-0").set(&5u64);
        StorageValueRef::persistent(b"eth-bridge-ocw::eth-log-range-size-0").set(&1u64);
        push_global_json_rpc_response(types::U64::from(CONFIRMATION_INTERVAL + 5));
        push_global_raw_response(br#"{"jsonrpc":"2.0","error":{"code":-32005,"message":"query is limited to a block range"},"id":0}"#);
        assert_eq!(EthBridge::handle_ethereum(0, false), Ok((CONFIRMATION_INTERVAL + 5, true)));
        assert_eq!(StorageValueRef::persistent(b"eth-bridge-ocw::eth-to-handle-from-height-0").get::<u64>().unwrap(), Some(5));
        for (message, expected) in [
            ("query is limited to a log count", Error::HttpResponseTooLarge),
            ("account is limited to a daily quota", Error::JsonDeserializationError),
        ] {
            push_global_raw_response(serde_json::json!({"jsonrpc":"2.0","error":{"code":-32005,"message":message},"id":0}).to_string().as_bytes());
            assert_eq!(EthBridge::load_transfers_logs(0, 5, 5), Err(expected));
        }
    });
}

#[test]
fn queued_quarantined_and_dependency_blocked_requests_are_retained_without_signing() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let outgoing = OffchainRequest::outgoing(outgoing());
        let outgoing_hash = outgoing.hash();
        sp_io::storage::set(&bridge_types::recovery::hashi_key(0u32, outgoing_hash), &[]);
        let tx_hash = H256::repeat_byte(9);
        let incoming = OffchainRequest::LoadIncoming(LoadIncomingRequest::Transaction(
            LoadIncomingTransactionRequest::new(
                alice(),
                tx_hash,
                Default::default(),
                IncomingTransactionRequestKind::Transfer,
                0,
            ),
        ));
        sp_io::storage::set(
            &bridge_types::recovery::message_key(
                bridge_types::GenericNetworkId::EVMLegacy(0),
                tx_hash,
            ),
            &[],
        );
        let compat_hash = H256::repeat_byte(10);
        RequestStatuses::<Runtime>::insert(0, compat_hash, RequestStatus::Pending);
        let blocked =
            OffchainRequest::outgoing(OutgoingRequest::RemovePeer(OutgoingRemovePeer::<Runtime> {
                author: alice(),
                peer_account_id: common::mock::bob(),
                peer_address: H160::repeat_byte(3),
                nonce: 0,
                network_id: 0,
                timepoint: Default::default(),
                compat_hash: Some(compat_hash),
            }));
        let requests = [outgoing, incoming, blocked];
        let hashes = requests
            .iter()
            .map(OffchainRequest::hash)
            .collect::<Vec<_>>();
        for request in requests {
            Requests::<Runtime>::insert(0, request.hash(), request);
        }
        RequestsQueue::<Runtime>::insert(0, &hashes);
        push_global_json_rpc_response(types::U64::from(0));
        EthBridge::handle_network(0, 1);
        assert_eq!(RequestsQueue::<Runtime>::get(0), hashes);
        assert!(state.pending_txs().is_empty());
        assert!(state.pool_state.read().transactions.is_empty());
    });
}

#[test]
fn event_validation_rejects_unknown_assets_contracts_and_malformed_migrations() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let mut log = deposit_log(H256::repeat_byte(1));
        log.address = types::H160(EthBridge::val_master_contract_address().0);
        log.data = ethabi::encode(&[
            Token::FixedBytes(alice().encode()),
            Token::Uint(1.into()),
            Token::Address(types::H160::repeat_byte(99)),
            Token::FixedBytes(vec![0; 32]),
        ])
        .into();
        assert_eq!(
            EthBridge::parse_deposit_event_from_known_contract(0, &log),
            Err(Error::UnsupportedAssetId)
        );
        assert_eq!(
            EthBridge::get_asset_by_raw_asset_id(H256::repeat_byte(99), &H160::zero(), 0),
            Err(Error::UnknownAssetId)
        );
        log.address = types::H160::repeat_byte(99);
        assert_eq!(
            EthBridge::parse_main_event(
                0,
                &[log.clone()],
                IncomingTransactionRequestKind::Transfer
            ),
            Err(Error::UnknownEvent)
        );
        log.address = types::H160(EthBridge::bridge_contract_address(0).0);
        log.topics = vec![types::H256(hex_literal::hex!(
            "a2e7361c23d7820040603b83c0cd3f494d377bac69736377d75bb56c651a5098"
        ))];
        log.data = vec![1].into();
        assert_eq!(
            EthBridge::parse_main_event(0, &[log], IncomingTransactionRequestKind::Migrate),
            Err(Error::EthAbiDecodingError)
        );
        crate::BridgeContractAddress::<Runtime>::insert(1, H160::repeat_byte(2));
        assert_ok!(EthBridge::ensure_known_contract(H160::repeat_byte(2), 1));
        assert_eq!(
            EthBridge::ensure_known_contract(H160::repeat_byte(3), 1),
            Err(Error::UnknownContractAddress)
        );
        push_global_http_failure();
        EthBridge::offchain();
        assert!(state.pending_txs().is_empty());
    });
}

#[test]
fn compatibility_peer_method_rejects_an_unrelated_outgoing_request() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let request = OffchainRequest::outgoing(outgoing());
        let hash = request.hash();
        Requests::<Runtime>::insert(0, hash, request);
        crate::contract::ADD_PEER_BY_PEER_ID.get_or_init(crate::contract::init_add_peer_by_peer_fn);
        let input = crate::contract::ADD_PEER_BY_PEER_FN
            .get()
            .unwrap()
            .encode_input(&[
                Token::Address(types::H160::repeat_byte(2)),
                Token::FixedBytes(hash.0.to_vec()),
                Token::Array(vec![]),
                Token::Array(vec![]),
                Token::Array(vec![]),
            ])
            .unwrap();
        let load = LoadIncomingTransactionRequest::new(
            alice(),
            H256::repeat_byte(2),
            Default::default(),
            IncomingTransactionRequestKind::AddPeerCompat,
            0,
        );
        assert_eq!(
            EthBridge::parse_old_incoming_request_method_call(
                load,
                Transaction {
                    input: input.into(),
                    ..Default::default()
                }
            ),
            Err(Error::InvalidFunctionInput)
        );
    });
}

#[test]
fn pending_multisig_selector_skips_ineligible_entries_before_retrying_eligible_one() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let id = EthBridge::bridge_account(0).unwrap();
        for hash in [[1; 32], [2; 32], [3; 32]] {
            bridge_multisig::Multisigs::<Runtime>::insert(
                &id,
                hash,
                bridge_multisig::Multisig {
                    when: Default::default(),
                    deposit: 0,
                    depositor: id.clone(),
                    approvals: vec![id.clone()],
                },
            );
        }
        // Use actual trie order: both ineligible entries must precede the selected
        // candidate in each pass through the network's multisig queue.
        let order = bridge_multisig::Multisigs::<Runtime>::iter_prefix(&id)
            .map(|(hash, _)| hash)
            .collect::<Vec<_>>();
        for (hash, height) in order.iter().zip([
            MultiChainHeight::Thischain(1),
            MultiChainHeight::Sidechain(999),
            MultiChainHeight::Sidechain(0),
        ]) {
            bridge_multisig::Multisigs::<Runtime>::mutate(&id, hash, |value| {
                value.as_mut().unwrap().when.height = height
            });
        }
        push_global_http_failure();
        assert!(EthBridge::handle_pending_multisig_calls(0, 1000));
        assert_eq!(state.http_requests().len(), 1);
        let handled = EthBridge::pending_multisig_rehandle_key(
            "eth-bridge-ocw::eth-re-handle-handled",
            0,
            &order[2],
            0,
            0,
        );
        assert_eq!(
            StorageValueRef::persistent(&handled).get::<bool>().unwrap(),
            None
        );
        push_global_json_rpc_response(Vec::<Log>::new());
        assert!(EthBridge::handle_pending_multisig_calls(0, 1000));
        assert_eq!(
            StorageValueRef::persistent(&handled).get::<bool>().unwrap(),
            Some(true)
        );
        assert!(!EthBridge::handle_pending_multisig_calls(0, 1000));
        assert_eq!(state.http_requests().len(), 2);
    });
}

#[test]
fn retry_batch_limit_preserves_deferred_transactions_for_the_next_block() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        System::set_block_number(20);
        let pending = (1..=MAX_SUCCESSFUL_SENT_SIGNED_TX_PER_ONCE + 1)
            .map(|index| {
                let hash = H256::repeat_byte(index);
                (
                    hash,
                    SignedTransactionData::<Runtime>::new(
                        hash,
                        None,
                        frame_system::Call::<Runtime>::remark {
                            remark: vec![index],
                        },
                    ),
                )
            })
            .collect::<BTreeMap<_, _>>();
        StorageValueRef::persistent(STORAGE_PENDING_TRANSACTIONS_KEY).set(&pending);
        assert_ok!(EthBridge::handle_substrate_block(
            SubstrateBlockLimited {
                header: header(20),
                extrinsics: vec![]
            },
            20
        ));
        let after = state.pending_txs();
        assert_eq!(
            after.len(),
            usize::from(MAX_SUCCESSFUL_SENT_SIGNED_TX_PER_ONCE + 1)
        );
        assert_eq!(
            after
                .values()
                .filter(|tx| tx.submitted_at == Some(20))
                .count(),
            usize::from(MAX_SUCCESSFUL_SENT_SIGNED_TX_PER_ONCE)
        );
        let deferred = H256::repeat_byte(MAX_SUCCESSFUL_SENT_SIGNED_TX_PER_ONCE + 1);
        assert_eq!(after[&deferred].encode(), pending[&deferred].encode());
        assert_eq!(
            state.pool_state.read().transactions.len(),
            usize::from(MAX_SUCCESSFUL_SENT_SIGNED_TX_PER_ONCE)
        );
        assert_ok!(EthBridge::handle_substrate_block(
            SubstrateBlockLimited {
                header: header(21),
                extrinsics: vec![]
            },
            21
        ));
        assert!(state
            .pending_txs()
            .values()
            .all(|tx| tx.submitted_at.is_some()));
        assert_eq!(
            state.pool_state.read().transactions.len(),
            usize::from(MAX_SUCCESSFUL_SENT_SIGNED_TX_PER_ONCE + 1)
        );
    });
}

#[test]
fn failed_queue_preserves_non_multisig_calls_without_attempting_import_recovery() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let hash = H256::repeat_byte(1);
        let record = SignedTransactionData::<Runtime>::new(
            hash,
            None,
            frame_system::Call::<Runtime>::remark { remark: vec![255] },
        );
        assert!(bridge_multisig::Call::<Runtime>::decode(&mut &record.call.encode()[1..]).is_err());
        let failed = BTreeMap::from([(hash, record)]);
        StorageValueRef::persistent(STORAGE_FAILED_PENDING_TRANSACTIONS_KEY).set(&failed);
        EthBridge::handle_failed_transactions_queue();
        assert_eq!(state.failed_pending_txs().encode(), failed.encode());
        assert!(state.pending_txs().is_empty());
        assert!(state.pool_state.read().transactions.is_empty());
    });
}

#[test]
fn hook_bootstraps_and_releases_worker_lock_after_rpc_failure() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        for block in [1, 2] {
            push_global_http_failure();
            <EthBridge as frame_support::traits::Hooks<BlockNumber>>::offchain_worker(block);
            assert_eq!(state.http_requests().len(), block as usize);
            assert_eq!(
                StorageValueRef::persistent(crate::STORAGE_BOOTSTRAP_READY_KEY)
                    .get::<u64>()
                    .unwrap(),
                Some(1)
            );
            assert_eq!(
                StorageValueRef::persistent(crate::STORAGE_LOCAL_SIGNING_KEY_READY_KEY)
                    .get::<u64>()
                    .unwrap(),
                Some(1)
            );
        }
        assert!(state.pending_txs().is_empty());
        assert!(state.pool_state.read().transactions.is_empty());
    });
}
