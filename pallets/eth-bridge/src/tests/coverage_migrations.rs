use super::*;
use crate::requests::*;
use crate::tests::mock::{EthBridge, ExtBuilder, Runtime, System};
use common::mock::{alice, bob};
use sp_core::{H160, H256};

fn outgoing() -> OutgoingRequest<Runtime> {
    OutgoingRequest::AddAsset(OutgoingAddAsset {
        author: alice(),
        asset_id: common::VAL,
        nonce: 7,
        network_id: 0,
        timepoint: Default::default(),
    })
}

#[test]
fn legacy_conversion_preserves_wire_bytes_and_identity_for_unchanged_request_variants() {
    let hash = H256::repeat_byte(17);
    let incoming: Vec<OldIncomingRequest<Runtime>> = vec![
        OldIncomingRequest::Transfer(IncomingTransfer {
            from: H160::repeat_byte(1),
            to: bob(),
            asset_id: common::VAL,
            asset_kind: AssetKind::Thischain,
            amount: 123,
            author: alice(),
            tx_hash: hash,
            at_height: 41,
            timepoint: Default::default(),
            network_id: 0,
            should_take_fee: false,
        }),
        OldIncomingRequest::AddToken(IncomingAddToken {
            token_address: H160::repeat_byte(2),
            asset_id: common::VAL,
            precision: 18,
            symbol: common::AssetSymbol(b"TEST".to_vec()),
            name: common::AssetName(b"Test token".to_vec()),
            author: alice(),
            tx_hash: hash,
            at_height: 41,
            timepoint: Default::default(),
            network_id: 0,
        }),
        OldIncomingRequest::CancelOutgoingRequest(IncomingCancelOutgoingRequest {
            outgoing_request: outgoing(),
            outgoing_request_hash: hash,
            initial_request_hash: H256::repeat_byte(18),
            tx_input: vec![1, 2, 3, 4],
            author: alice(),
            tx_hash: hash,
            at_height: 41,
            timepoint: Default::default(),
            network_id: 0,
        }),
        OldIncomingRequest::MarkAsDone(IncomingMarkAsDoneRequest {
            outgoing_request_hash: hash,
            initial_request_hash: H256::repeat_byte(18),
            author: alice(),
            at_height: 41,
            timepoint: Default::default(),
            network_id: 0,
        }),
        OldIncomingRequest::PrepareForMigration(IncomingPrepareForMigration {
            author: alice(),
            tx_hash: hash,
            at_height: 41,
            timepoint: Default::default(),
            network_id: 0,
        }),
        OldIncomingRequest::Migrate(IncomingMigrate {
            new_contract_address: H160::repeat_byte(3),
            author: alice(),
            tx_hash: hash,
            at_height: 41,
            timepoint: Default::default(),
            network_id: 0,
        }),
        OldIncomingRequest::ChangePeersCompat(IncomingChangePeersCompat {
            peer_account_id: bob(),
            peer_address: H160::repeat_byte(4),
            added: true,
            contract: ChangePeersContract::VAL,
            author: alice(),
            tx_hash: hash,
            at_height: 41,
            timepoint: Default::default(),
            network_id: 0,
        }),
    ];
    for old in incoming {
        let old = OldOffchainRequest::Incoming(old, hash);
        let bytes = old.encode();
        let converted: OffchainRequest<Runtime> = old.into();
        assert_eq!(converted.encode(), bytes);
        assert_eq!(converted.hash(), hash);
        assert_eq!(converted.author(), &alice());
        assert_eq!(converted.network_id(), 0);
        assert_eq!(
            decode_offchain_request_compat::<Runtime>(&bytes)
                .ok()
                .unwrap()
                .into_request(),
            converted
        );
    }
    let load = LoadIncomingRequest::Transaction(LoadIncomingTransactionRequest {
        author: alice(),
        hash,
        kind: IncomingTransactionRequestKind::Transfer,
        timepoint: Default::default(),
        network_id: 0,
    });
    for old in [
        OldOffchainRequest::Outgoing(outgoing(), hash),
        OldOffchainRequest::LoadIncoming(load),
    ] {
        let bytes = old.encode();
        let converted: OffchainRequest<Runtime> = old.into();
        assert_eq!(converted.encode(), bytes);
        assert_eq!(converted.hash(), hash);
    }
    assert!(decode_exact::<u8>(&[1, 2]).is_err());
    assert!(decode_offchain_request_compat::<Runtime>(&[255]).is_err());
}

#[test]
fn pending_and_frozen_legacy_statuses_migrate_without_changing_request_ids() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        for (byte, old) in [
            (1, OldRequestStatus::Pending),
            (2, OldRequestStatus::Frozen),
        ] {
            let key = RequestStatuses::<Runtime>::hashed_key_for(0, H256::repeat_byte(byte));
            sp_io::storage::set(&key, &old.encode());
        }
        migrate::<Runtime>();
        assert_eq!(
            RequestStatuses::<Runtime>::get(0, H256::repeat_byte(1)),
            Some(RequestStatus::Pending)
        );
        assert_eq!(
            RequestStatuses::<Runtime>::get(0, H256::repeat_byte(2)),
            Some(RequestStatus::Frozen)
        );
        assert_eq!(
            EthBridge::on_chain_storage_version(),
            StorageVersion::new(3)
        );
    });
}

#[test]
fn account_migration_rejects_oversized_cursor_and_trailing_value_bytes_without_writes() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_eq!(
            AccountRequestsV3ToV4::<Runtime>::bounded_cursor(vec![0; 129]),
            Err(SteppedMigrationError::InvalidCursor)
        );
        StorageVersion::new(3).put::<EthBridge>();
        let key = AccountRequests::<Runtime>::hashed_key_for(alice());
        let mut bytes = vec![(0u32, H256::repeat_byte(1))].encode();
        bytes.push(0xff);
        sp_io::storage::set(&key, &bytes);
        let mut meter = WeightMeter::with_limit(Weight::MAX);
        assert_eq!(
            AccountRequestsV3ToV4::<Runtime>::step(None, &mut meter),
            Err(SteppedMigrationError::Failed)
        );
        assert_eq!(
            sp_io::storage::get(&key).unwrap().as_ref(),
            bytes.as_slice()
        );
        assert_eq!(
            EthBridge::on_chain_storage_version(),
            StorageVersion::new(3)
        );
    });
}

#[test]
fn decommission_failure_rolls_back_quarantine_and_missing_timestamp_is_backfilled() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        System::set_block_number(19);
        BridgeAccount::<Runtime>::remove(0);
        let block_weights: frame_system::limits::BlockWeights =
            <Runtime as frame_system::Config>::BlockWeights::get();
        assert_eq!(
            decommission_legacy_ethereum_xor::<Runtime>(),
            block_weights.max_block
        );
        assert!(!DeprecatedSidechainTokens::<Runtime>::contains_key(
            0,
            LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS
        ));
        assert!(!is_legacy_ethereum_xor_decommissioned::<Runtime>());
        assert_eq!(legacy_ethereum_xor_decommissioned_at::<Runtime>(), None);
        LegacyEthereumXorDecommissioned::<Runtime>::put(true);
        decommission_legacy_ethereum_xor::<Runtime>();
        assert_eq!(legacy_ethereum_xor_decommissioned_at::<Runtime>(), Some(19));
        assert!(is_legacy_ethereum_xor_decommissioned::<Runtime>());
    });
}

#[test]
fn malformed_historical_requests_are_retained_but_not_treated_as_valid_blockers() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let hash = H256::repeat_byte(9);
        let key = Requests::<Runtime>::hashed_key_for(0, hash);
        sp_io::storage::set(&key, &[255]);
        assert_eq!(legacy_ethereum_xor_decommission_blockers::<Runtime>(), 0);
        let (requests, reads) = legacy_ethereum_xor_requests::<Runtime>(0);
        assert!(requests.is_empty());
        assert_eq!(reads, 2);
        assert_eq!(sp_io::storage::get(&key).unwrap().as_ref(), &[255]);
        let request = OffchainRequest::outgoing(outgoing());
        assert!(!is_legacy_xor_request::<Runtime>(1, &request));
    });
}

#[test]
fn migration_queue_bypass_retains_request_kind_duplicate_and_signing_guards() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let load = OffchainRequest::LoadIncoming(LoadIncomingRequest::Transaction(
            LoadIncomingTransactionRequest {
                author: alice(),
                hash: H256::repeat_byte(1),
                kind: IncomingTransactionRequestKind::Transfer,
                timepoint: Default::default(),
                network_id: 0,
            },
        ));
        assert_eq!(
            add_request_unchecked_capacity::<Runtime>(&load),
            Err(Error::<Runtime>::ExpectedOutgoingRequest.into())
        );
        let token =
            OffchainRequest::outgoing(OutgoingRequest::AddToken(OutgoingAddToken::<Runtime> {
                author: alice(),
                token_address: H160::repeat_byte(7),
                symbol: "TEST".into(),
                name: "Test token".into(),
                decimals: 18,
                nonce: 0,
                network_id: 0,
                timepoint: Default::default(),
            }));
        assert_eq!(add_request_unchecked_capacity::<Runtime>(&token), Ok(()));
        assert_eq!(
            add_request_unchecked_capacity::<Runtime>(&token),
            Err(Error::<Runtime>::SidechainAssetIsAlreadyRegistered.into())
        );
        RequestsQueue::<Runtime>::remove(0);
        assert_eq!(
            add_request_unchecked_capacity::<Runtime>(&token),
            Err(Error::<Runtime>::DuplicatedRequest.into())
        );
        RequestStatuses::<Runtime>::insert(
            0,
            token.hash(),
            RequestStatus::Failed(Error::<Runtime>::UnknownRequest.into()),
        );
        assert_eq!(add_request_unchecked_capacity::<Runtime>(&token), Ok(()));
        assert_eq!(RequestsQueue::<Runtime>::get(0), vec![token.hash()]);
        assert_eq!(
            RequestStatuses::<Runtime>::get(0, token.hash()),
            Some(RequestStatus::Pending)
        );
        let weak =
            OffchainRequest::outgoing(OutgoingRequest::AddPeerCompat(OutgoingAddPeerCompat::<
                Runtime,
            > {
                author: alice(),
                peer_address: H160::repeat_byte(8),
                peer_account_id: bob(),
                nonce: 0,
                network_id: 1,
                timepoint: Default::default(),
            }));
        BridgeStatuses::<Runtime>::insert(1, BridgeStatus::Initialized);
        assert!(!is_legacy_xor_request::<Runtime>(0, &weak));
        assert_eq!(
            add_request_unchecked_capacity::<Runtime>(&weak),
            Err(Error::<Runtime>::WeakLegacySigningDisabled.into())
        );
        assert_eq!(RequestsQueue::<Runtime>::get(1), vec![]);
    });
}

#[test]
fn account_migration_resumes_after_a_value_exceeds_remaining_step_budget() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        StorageVersion::new(3).put::<EthBridge>();
        let mut keys = [
            AccountRequests::<Runtime>::hashed_key_for(alice()),
            AccountRequests::<Runtime>::hashed_key_for(bob()),
        ];
        keys.sort();
        let first = vec![(0u32, H256::repeat_byte(1))].encode();
        let second = vec![(0u32, H256::repeat_byte(2)); 10].encode();
        sp_io::storage::set(&keys[0], &first);
        sp_io::storage::set(&keys[1], &second);
        let limit = AccountRequestsV3ToV4::<Runtime>::base_weight()
            .saturating_add(AccountRequestsV3ToV4::<Runtime>::scan_weight().saturating_mul(2))
            .saturating_add(AccountRequestsV3ToV4::<Runtime>::encoded_value_weight(
                first.len() as u32,
                false,
            ))
            .saturating_add(AccountRequestsV3ToV4::<Runtime>::finish_weight());
        let cursor =
            AccountRequestsV3ToV4::<Runtime>::step(None, &mut WeightMeter::with_limit(limit))
                .unwrap()
                .unwrap();
        assert_eq!(cursor.as_slice(), keys[0]);
        assert_eq!(sp_io::storage::get(&keys[1]).unwrap().as_ref(), second);
        assert_eq!(
            EthBridge::on_chain_storage_version(),
            StorageVersion::new(3)
        );
        assert_eq!(
            AccountRequestsV3ToV4::<Runtime>::step(
                Some(cursor),
                &mut WeightMeter::with_limit(Weight::MAX)
            ),
            Ok(None)
        );
        assert_eq!(
            EthBridge::on_chain_storage_version(),
            StorageVersion::new(4)
        );
        let retained =
            Vec::<(u32, H256)>::decode(&mut sp_io::storage::get(&keys[1]).unwrap().as_ref())
                .unwrap();
        assert_eq!(retained, vec![(0u32, H256::repeat_byte(2)); 3]);
    });
}

#[test]
fn account_migration_removes_all_history_when_indexing_is_disabled() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        crate::tests::mock::MaxRequestsPerAccountConst::set(0);
        StorageVersion::new(3).put::<EthBridge>();
        AccountRequests::<Runtime>::insert(alice(), vec![(0u32, H256::repeat_byte(1))]);
        AccountRequests::<Runtime>::insert(bob(), Vec::<(u32, H256)>::new());
        assert_eq!(
            AccountRequestsV3ToV4::<Runtime>::step(None, &mut WeightMeter::with_limit(Weight::MAX)),
            Ok(None)
        );
        assert!(!AccountRequests::<Runtime>::contains_key(alice()));
        assert!(!AccountRequests::<Runtime>::contains_key(bob()));
        assert_eq!(
            EthBridge::on_chain_storage_version(),
            StorageVersion::new(4)
        );
    });
}
