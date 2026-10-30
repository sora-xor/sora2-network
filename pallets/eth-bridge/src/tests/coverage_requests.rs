// SPDX-License-Identifier: BSD-4-Clause

use super::mock::{ExtBuilder, Runtime};
use super::Error;
use crate::requests::encode_packed::{encode_packed, TokenWrapper};
use crate::requests::*;
use crate::{BridgeSignatureVersion, BridgeStatus, EthAddress};
use bridge_multisig::{BridgeTimepoint, MultiChainHeight};
use codec::{Decode, Encode};
use ethabi::Token;
use frame_support::{assert_err, assert_ok};
use sp_core::H256;

fn timepoint() -> crate::Timepoint<Runtime> {
    BridgeTimepoint {
        height: MultiChainHeight::Thischain(7),
        index: 3,
    }
}

#[test]
fn packed_encoding_preserves_widths_and_collection_layout() {
    let tokens = vec![
        Token::Bytes(vec![0xab, 0xcd]).into(),
        Token::String("é".into()).into(),
        Token::Bool(true).into(),
        Token::Bool(false).into(),
        TokenWrapper::UintSized(0x1234u64.into(), 16),
        TokenWrapper::IntSized(ethabi::Uint::MAX, 16),
        Token::FixedArray(vec![Token::Bool(true), Token::Bytes(vec![0xff])]).into(),
        Token::Tuple(vec![Token::Bool(false), Token::String("x".into())]).into(),
    ];
    assert_eq!(
        encode_packed(&tokens),
        vec![0xab, 0xcd, 0xc3, 0xa9, 1, 0, 0x12, 0x34, 0xff, 0xff, 1, 0xff, 0, b'x']
    );
    assert_eq!(
        encode_packed(&[Token::Int(ethabi::Uint::MAX).into()]),
        vec![0xff; 32]
    );
    assert_eq!(encode_packed(&[]), Vec::<u8>::new());
    let addresses = vec![
        Token::Address([0x12; 20].into()),
        Token::Address([0x34; 20].into()),
    ];
    let encoded = encode_packed(&[Token::Array(addresses).into()]);
    let mut expected = vec![0; 12];
    expected.extend([0x12; 20]);
    expected.extend([0; 12]);
    expected.extend([0x34; 20]);
    assert_eq!(encoded, expected);
}

#[test]
fn contract_argument_helpers_keep_signature_columns_in_order() {
    let mut encoder = Encoder::new();
    encoder.write_address(&EthAddress::repeat_byte(0x19));
    encoder.write_string("token".into());
    encoder.write_u8(18);
    assert_eq!(
        encoder.into_inner(),
        vec![
            Token::Address([0x19; 20].into()),
            Token::String("token".into()),
            Token::Uint(18u64.into())
        ]
    );
    let signatures = vec![
        crate::offchain::SignatureParams {
            r: [1; 32],
            s: [2; 32],
            v: 27,
        },
        crate::offchain::SignatureParams {
            r: [3; 32],
            s: [4; 32],
            v: 28,
        },
    ];
    assert_eq!(
        signature_params_to_tokens(signatures),
        vec![
            Token::Array(vec![Token::Uint(27u64.into()), Token::Uint(28u64.into())]),
            Token::Array(vec![
                Token::FixedBytes(vec![1; 32]),
                Token::FixedBytes(vec![3; 32])
            ]),
            Token::Array(vec![
                Token::FixedBytes(vec![2; 32]),
                Token::FixedBytes(vec![4; 32])
            ]),
        ]
    );
    assert_eq!(
        signature_params_to_tokens(vec![]),
        vec![Token::Array(vec![]); 3]
    );
    let hash = H256::repeat_byte(0x42);
    assert_eq!(
        parse_hash_from_call::<Runtime>(
            vec![Token::Bool(true), Token::FixedBytes(hash.0.to_vec())],
            1
        ),
        Ok(hash)
    );
    assert_err!(
        parse_hash_from_call::<Runtime>(vec![], 0),
        Error::FailedToParseTxHashInCall
    );
    assert_err!(
        parse_hash_from_call::<Runtime>(vec![Token::Bool(true)], 0),
        Error::FailedToParseTxHashInCall
    );
    assert_err!(
        parse_hash_from_call::<Runtime>(vec![Token::FixedBytes(vec![0; 31])], 0),
        Error::FailedToParseTxHashInCall
    );
}

#[test]
fn load_requests_preserve_sidechain_hashes_and_validate_completion_state() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let hash = H256::repeat_byte(0x43);
        let author: super::mock::AccountId = [5; 32].into();
        let mut transaction =
            LoadIncomingRequest::<Runtime>::Transaction(LoadIncomingTransactionRequest::new(
                author.clone(),
                hash,
                timepoint(),
                IncomingTransactionRequestKind::Transfer,
                0,
            ));
        transaction.set_hash(H256::repeat_byte(0x99));
        assert_eq!(transaction.hash(), hash);
        assert_eq!(transaction.author(), &author);
        assert_eq!(transaction.timepoint(), timepoint());
        assert_eq!(transaction.network_id(), 0);
        assert_ok!(transaction.validate());
        assert_ok!(transaction.prepare());
        assert_ok!(transaction.cancel());
        assert_ok!(transaction.finalize());

        let meta = LoadIncomingMetaRequest::<Runtime>::new(
            author.clone(),
            hash,
            timepoint(),
            IncomingMetaRequestKind::MarkAsDone,
            0,
        );
        let load = OffchainRequest::load_incoming_meta(meta);
        assert!(load.is_load_incoming());
        assert!(!load.is_incoming());
        assert!(!load.should_be_skipped());
        assert!(load.as_outgoing().is_none());
        assert!(load.clone().into_outgoing().is_none());
        assert!(load.as_incoming().is_none());
        assert!(load.clone().into_incoming().is_none());
        assert_ne!(load.hash(), hash);
        assert_eq!(load.timepoint(), timepoint());
        assert_eq!(load.author(), &author);
        assert_eq!(
            OffchainRequest::<Runtime>::decode(&mut &load.encode()[..]).unwrap(),
            load
        );
        assert_err!(load.validate(), Error::UnknownRequest);
        crate::RequestStatuses::<Runtime>::insert(0, hash, RequestStatus::Pending);
        assert_err!(load.validate(), Error::RequestIsNotReady);
        crate::RequestStatuses::<Runtime>::insert(0, hash, RequestStatus::ApprovalsReady);
        assert_ok!(load.validate());
        assert_ok!(load.prepare());
        assert_ok!(load.cancel());
        assert_ok!(load.finalize());
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(0, hash),
            Some(RequestStatus::ApprovalsReady)
        );
    });
}

#[test]
fn contract_migration_requires_prepare_and_pending_signature_version() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let author: super::mock::AccountId = [5; 32].into();
        let hash = H256::repeat_byte(0x51);
        let new_contract = EthAddress::repeat_byte(0x52);
        let prepare = IncomingPrepareForMigration::<Runtime> {
            author,
            tx_hash: hash,
            at_height: 9,
            timepoint: timepoint(),
            network_id: 0,
        };
        let mut unknown = prepare.clone();
        unknown.network_id = 99;
        assert_err!(unknown.prepare(), Error::UnknownNetwork);
        assert_eq!(prepare.author(), &prepare.author);
        assert_eq!(prepare.timepoint(), timepoint());
        assert_ok!(prepare.prepare());
        assert_ok!(prepare.cancel());
        assert_eq!(
            crate::BridgeStatuses::<Runtime>::get(0),
            Some(BridgeStatus::Initialized)
        );
        assert_eq!(prepare.finalize(), Ok(hash));
        assert_eq!(
            crate::BridgeStatuses::<Runtime>::get(0),
            Some(BridgeStatus::Migrating)
        );
        assert_err!(prepare.prepare(), Error::ContractIsAlreadyInMigrationStage);

        let incoming = IncomingMigrate::<Runtime> {
            author: prepare.author.clone(),
            new_contract_address: new_contract,
            tx_hash: hash,
            at_height: 10,
            timepoint: timepoint(),
            network_id: 0,
        };
        assert_err!(incoming.prepare(), Error::ContractIsNotInMigrationStage);
        let outgoing = OutgoingMigrate::<Runtime> {
            author: prepare.author,
            new_contract_address: new_contract,
            erc20_native_tokens: vec![],
            nonce: 0,
            network_id: 0,
            timepoint: timepoint(),
            new_signature_version: BridgeSignatureVersion::V3,
        };
        assert_ok!(outgoing.validate());
        assert_ok!(outgoing.prepare(()));
        assert_ok!(outgoing.cancel());
        assert_ok!(outgoing.finalize());
        assert_eq!(
            crate::PendingBridgeSignatureVersions::<Runtime>::get(0),
            Some(BridgeSignatureVersion::V3)
        );
        assert_eq!(incoming.author(), &incoming.author);
        assert_eq!(incoming.timepoint(), timepoint());
        assert_ok!(incoming.prepare());
        assert_ok!(incoming.cancel());
        assert_ne!(
            crate::BridgeContractAddress::<Runtime>::get(0),
            new_contract
        );
        assert_eq!(incoming.finalize(), Ok(hash));
        assert_eq!(
            crate::BridgeContractAddress::<Runtime>::get(0),
            new_contract
        );
        assert_eq!(
            crate::BridgeStatuses::<Runtime>::get(0),
            Some(BridgeStatus::Initialized)
        );
        assert_eq!(
            crate::BridgeSignatureVersions::<Runtime>::get(0),
            BridgeSignatureVersion::V3
        );
        assert_eq!(
            crate::PendingBridgeSignatureVersions::<Runtime>::get(0),
            None
        );
        assert_err!(incoming.prepare(), Error::ContractIsNotInMigrationStage);
        assert_err!(outgoing.validate(), Error::ContractIsNotInMigrationStage);
        let mut unknown = incoming;
        unknown.network_id = 99;
        assert_err!(unknown.prepare(), Error::UnknownNetwork);
        let mut unknown = outgoing;
        unknown.network_id = 99;
        assert_err!(unknown.validate(), Error::UnknownNetwork);
    });
}

#[test]
fn prepared_and_migrated_events_preserve_network_metadata() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let load = LoadIncomingTransactionRequest::<Runtime>::new(
            [6; 32].into(),
            H256::repeat_byte(0x65),
            timepoint(),
            IncomingTransactionRequestKind::PrepareForMigration,
            0,
        );
        for event in [
            crate::contract::ContractEvent::PreparedForMigration,
            crate::contract::ContractEvent::Migrated(EthAddress::repeat_byte(0x66)),
        ] {
            let incoming =
                IncomingRequest::try_from_contract_event(event, load.clone(), 84).unwrap();
            assert_eq!(incoming.hash(), load.hash);
            assert_eq!(incoming.author(), &load.author);
            assert_eq!(incoming.at_height(), 84);
            assert_eq!(incoming.timepoint(), timepoint());
            assert_eq!(incoming.network_id(), 0);
            assert_ok!(incoming.validate());
            assert_ok!(incoming.cancel());
            let offchain = OffchainRequest::incoming(incoming.clone());
            assert!(offchain.is_incoming());
            assert!(!offchain.is_load_incoming());
            assert!(!offchain.should_be_skipped());
            assert!(offchain.as_outgoing().is_none());
            assert!(offchain.clone().into_outgoing().is_none());
            assert_eq!(offchain.as_incoming(), Some((&incoming, offchain.hash())));
            assert_eq!(
                offchain.clone().into_incoming(),
                Some((incoming, offchain.hash()))
            );
            assert_eq!(
                OffchainRequest::<Runtime>::decode(&mut &offchain.encode()[..]).unwrap(),
                offchain
            );
        }
    });
}

#[test]
fn migration_extrinsics_complete_handshake_and_preserve_nonce_on_rejection() {
    use super::mock::{EthBridge, RuntimeOrigin, System};
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let authority = state.authority_account_id.clone();
        let contract = EthAddress::repeat_byte(0x83);
        let nonce = System::account_nonce(&authority);
        assert_err!(
            EthBridge::prepare_for_migration(RuntimeOrigin::signed(authority.clone()), 0),
            sp_runtime::DispatchError::BadOrigin
        );
        assert_err!(
            EthBridge::migrate(
                RuntimeOrigin::signed(authority.clone()),
                contract,
                vec![],
                0,
                BridgeSignatureVersion::V3
            ),
            sp_runtime::DispatchError::BadOrigin
        );
        assert_err!(
            EthBridge::migrate(
                RuntimeOrigin::root(),
                contract,
                vec![],
                0,
                BridgeSignatureVersion::V3
            ),
            Error::ContractIsNotInMigrationStage
        );
        assert_eq!(System::account_nonce(&authority), nonce);
        assert_ok!(EthBridge::prepare_for_migration(RuntimeOrigin::root(), 0));
        assert_eq!(System::account_nonce(&authority), nonce + 1);
        let (request, request_hash) = super::last_outgoing_request(0).unwrap();
        assert!(matches!(request, OutgoingRequest::PrepareForMigration(_)));
        assert!(!request.is_allowed_during_migration());
        assert_eq!(request.author(), &authority);
        super::approve_request(&state, request, request_hash).unwrap();
        assert_eq!(
            crate::BridgeStatuses::<Runtime>::get(0),
            Some(BridgeStatus::Initialized)
        );

        let hash = H256::repeat_byte(0x84);
        super::request_incoming(
            authority.clone(),
            hash,
            IncomingTransactionRequestKind::PrepareForMigration.into(),
            0,
        )
        .unwrap();
        let incoming = IncomingPrepareForMigration::<Runtime> {
            author: authority.clone(),
            tx_hash: hash,
            at_height: 21,
            timepoint: timepoint(),
            network_id: 0,
        };
        super::assert_incoming_request_done(&state, incoming.into()).unwrap();
        assert_eq!(
            crate::BridgeStatuses::<Runtime>::get(0),
            Some(BridgeStatus::Migrating)
        );
        assert_ok!(EthBridge::migrate(
            RuntimeOrigin::root(),
            contract,
            vec![EthAddress::repeat_byte(0x85)],
            0,
            BridgeSignatureVersion::V3
        ));
        let (request, request_hash) = super::last_outgoing_request(0).unwrap();
        assert!(request.is_allowed_during_migration());
        super::approve_request(&state, request, request_hash).unwrap();
        assert_eq!(
            crate::PendingBridgeSignatureVersions::<Runtime>::get(0),
            Some(BridgeSignatureVersion::V3)
        );

        let hash = H256::repeat_byte(0x86);
        super::request_incoming(
            authority.clone(),
            hash,
            IncomingTransactionRequestKind::Migrate.into(),
            0,
        )
        .unwrap();
        let incoming = IncomingMigrate::<Runtime> {
            new_contract_address: contract,
            author: authority,
            tx_hash: hash,
            at_height: 22,
            timepoint: timepoint(),
            network_id: 0,
        };
        super::assert_incoming_request_done(&state, incoming.into()).unwrap();
        assert_eq!(
            crate::BridgeStatuses::<Runtime>::get(0),
            Some(BridgeStatus::Initialized)
        );
        assert_eq!(crate::BridgeContractAddress::<Runtime>::get(0), contract);
        assert_eq!(
            crate::PendingBridgeSignatureVersions::<Runtime>::get(0),
            None
        );
    });
}

#[test]
fn migration_extrinsics_reject_missing_authority_and_unsafe_legacy_peer_count() {
    use super::mock::{EthBridge, RuntimeOrigin};
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        crate::AuthorityAccount::<Runtime>::kill();
        assert_err!(
            EthBridge::prepare_for_migration(RuntimeOrigin::root(), 0),
            Error::AuthorityAccountNotSet
        );
        assert_err!(
            EthBridge::migrate(
                RuntimeOrigin::root(),
                EthAddress::zero(),
                vec![],
                0,
                BridgeSignatureVersion::V3
            ),
            Error::AuthorityAccountNotSet
        );
        crate::BridgeSignatureVersions::<Runtime>::insert(0, BridgeSignatureVersion::V1);
        crate::Peers::<Runtime>::remove(0);
        assert_err!(
            EthBridge::prepare_for_migration(RuntimeOrigin::root(), 0),
            Error::UnsafeMigration
        );
        assert!(crate::RequestsQueue::<Runtime>::get(0).is_empty());
    });
}

#[test]
fn peer_protocol_admission_authenticates_request_identity_and_lifecycle() {
    use super::mock::EthBridge;
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let account = state.networks[&0].config.bridge_account_id.clone();
        let outsider: super::mock::AccountId = [0x71; 32].into();
        let tx_hash = H256::repeat_byte(0x72);
        let incoming: IncomingRequest<Runtime> = IncomingPrepareForMigration {
            author: outsider.clone(),
            tx_hash,
            at_height: 42,
            timepoint: timepoint(),
            network_id: 0,
        }
        .into();
        let call = crate::Call::<Runtime>::register_incoming_request {
            incoming_request: incoming.clone(),
        };
        assert_eq!(EthBridge::peer_protocol_network(&call), Some(0));
        assert_eq!(
            EthBridge::validate_peer_protocol_call(&outsider, &call),
            Ok(false)
        );
        assert_eq!(
            EthBridge::validate_peer_protocol_call(&account, &call),
            Ok(true)
        );
        let offchain = OffchainRequest::incoming(incoming.clone());
        let hash = offchain.hash();
        assert_eq!(crate::RequestStatuses::<Runtime>::get(0, hash), None);
        assert_eq!(crate::Requests::<Runtime>::get(0, hash), None);

        crate::RequestStatuses::<Runtime>::insert(
            0,
            hash,
            RequestStatus::Failed(sp_runtime::DispatchError::Other("old attempt")),
        );
        assert_err!(
            EthBridge::validate_peer_protocol_call(&account, &call),
            Error::RequestIsAlreadyRegistered
        );
        crate::RequestStatuses::<Runtime>::remove(0, hash);
        crate::LoadToIncomingRequestHash::<Runtime>::insert(0, tx_hash, hash);
        assert_err!(
            EthBridge::validate_peer_protocol_call(&account, &call),
            Error::RequestIsAlreadyRegistered
        );
        crate::LoadToIncomingRequestHash::<Runtime>::remove(0, tx_hash);
        crate::Requests::<Runtime>::insert(0, hash, &offchain);
        assert_err!(
            EthBridge::validate_peer_protocol_call(&account, &call),
            Error::RequestIsAlreadyRegistered
        );

        let finalize = crate::Call::<Runtime>::finalize_incoming_request {
            hash,
            network_id: 0,
        };
        assert_err!(
            EthBridge::validate_peer_protocol_call(&account, &finalize),
            Error::ExpectedPendingRequest
        );
        crate::RequestStatuses::<Runtime>::insert(0, hash, RequestStatus::Pending);
        assert_eq!(
            EthBridge::validate_peer_protocol_call(&account, &finalize),
            Ok(true)
        );
        let abort = crate::Call::<Runtime>::abort_request {
            hash,
            error: sp_runtime::DispatchError::Other("abort"),
            network_id: 0,
        };
        assert_eq!(
            EthBridge::validate_peer_protocol_call(&account, &abort),
            Ok(true)
        );
        crate::RequestStatuses::<Runtime>::insert(0, hash, RequestStatus::Done);
        assert_err!(
            EthBridge::validate_peer_protocol_call(&account, &finalize),
            Error::ExpectedPendingRequest
        );
        assert_err!(
            EthBridge::validate_peer_protocol_call(&account, &abort),
            Error::ExpectedPendingRequest
        );
        crate::Requests::<Runtime>::remove(0, hash);
        assert_err!(
            EthBridge::validate_peer_protocol_call(&account, &finalize),
            Error::UnknownRequest
        );
        assert_err!(
            EthBridge::validate_peer_protocol_call(&account, &abort),
            Error::UnknownRequest
        );

        let user_call = crate::Call::<Runtime>::prepare_for_migration { network_id: 0 };
        assert_eq!(EthBridge::peer_protocol_network(&user_call), None);
        assert_eq!(
            EthBridge::validate_peer_protocol_call(&account, &user_call),
            Ok(false)
        );
        crate::BridgeStatuses::<Runtime>::remove(0);
        assert_err!(
            EthBridge::validate_peer_protocol_call(&account, &call),
            Error::UnknownNetwork
        );
    });
}

#[test]
fn imported_request_admission_rejects_mismatched_and_duplicate_replay_keys() {
    use super::mock::EthBridge;
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let account = state.networks[&0].config.bridge_account_id.clone();
        let tx_hash = H256::repeat_byte(0x73);
        let load =
            LoadIncomingRequest::Transaction(LoadIncomingTransactionRequest::<Runtime>::new(
                account.clone(),
                tx_hash,
                timepoint(),
                IncomingTransactionRequestKind::PrepareForMigration,
                0,
            ));
        let incoming = IncomingPrepareForMigration::<Runtime> {
            author: account.clone(),
            tx_hash,
            at_height: 42,
            timepoint: timepoint(),
            network_id: 0,
        };
        let call = |incoming_request_result| crate::Call::<Runtime>::import_incoming_request {
            load_incoming_request: load.clone(),
            incoming_request_result,
        };
        assert_eq!(
            EthBridge::validate_peer_protocol_call(&account, &call(Ok(incoming.clone().into()))),
            Ok(true)
        );
        assert_eq!(
            EthBridge::validate_peer_protocol_call(
                &account,
                &call(Err(sp_runtime::DispatchError::Other("RPC unavailable")))
            ),
            Ok(true)
        );
        let mut wrong_network = incoming.clone();
        wrong_network.network_id = 1;
        assert_err!(
            EthBridge::validate_peer_protocol_call(&account, &call(Ok(wrong_network.into()))),
            Error::UnknownNetwork
        );
        let mut wrong_hash = incoming.clone();
        wrong_hash.tx_hash = H256::repeat_byte(0x74);
        assert_err!(
            EthBridge::validate_peer_protocol_call(&account, &call(Ok(wrong_hash.into()))),
            Error::IncomingRequestHashMismatch
        );
        crate::LoadToIncomingRequestHash::<Runtime>::insert(0, tx_hash, H256::repeat_byte(0x75));
        assert_err!(
            EthBridge::validate_peer_protocol_call(&account, &call(Ok(incoming.clone().into()))),
            Error::RequestIsAlreadyRegistered
        );
        crate::LoadToIncomingRequestHash::<Runtime>::remove(0, tx_hash);
        crate::Requests::<Runtime>::insert(0, tx_hash, OffchainRequest::LoadIncoming(load.clone()));
        assert_err!(
            EthBridge::validate_peer_protocol_call(&account, &call(Ok(incoming.into()))),
            Error::DuplicatedRequest
        );

        let finalize = crate::Call::<Runtime>::finalize_incoming_request {
            hash: tx_hash,
            network_id: 0,
        };
        assert_err!(
            EthBridge::validate_peer_protocol_call(&account, &finalize),
            Error::ExpectedIncomingRequest
        );
    });
}

#[test]
fn incoming_token_registration_creates_mapping_and_rejects_duplicate_finalization() {
    use common::{AssetInfoProvider, AssetName, AssetSymbol};
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let token = EthAddress::repeat_byte(0x90);
        let incoming: IncomingRequest<Runtime> = IncomingAddToken {
            token_address: token,
            asset_id: common::DAI,
            precision: 6,
            symbol: AssetSymbol(b"USDC".to_vec()),
            name: AssetName(b"USD Coin".to_vec()),
            author: state.authority_account_id.clone(),
            tx_hash: H256::repeat_byte(0x91),
            at_height: 5,
            timepoint: timepoint(),
            network_id: 0,
        }
        .into();
        assert_eq!(incoming.author(), &state.authority_account_id);
        assert_eq!(incoming.timepoint(), timepoint());
        assert_eq!(incoming.at_height(), 5);
        assert_ok!(incoming.validate());
        assert_ok!(incoming.prepare());
        assert_ok!(incoming.cancel());
        assert!(!crate::RegisteredSidechainAsset::<Runtime>::contains_key(
            0, token
        ));
        assert_eq!(incoming.finalize(), Ok(H256::repeat_byte(0x91)));
        let asset = crate::RegisteredSidechainAsset::<Runtime>::get(0, token).unwrap();
        assert_eq!(
            crate::RegisteredSidechainToken::<Runtime>::get(0, asset),
            Some(token)
        );
        assert_eq!(
            crate::RegisteredAsset::<Runtime>::get(0, asset),
            Some(AssetKind::Sidechain)
        );
        assert_eq!(crate::SidechainAssetPrecision::<Runtime>::get(0, asset), 6);
        let (symbol, name, _, ..) = assets::Pallet::<Runtime>::get_asset_info(&asset);
        assert_eq!(symbol, AssetSymbol(b"USDC".to_vec()));
        assert_eq!(name, AssetName(b"USD Coin".to_vec()));
        assert_err!(incoming.finalize(), Error::TokenIsAlreadyAdded);
    });
}

#[test]
fn migration_contract_arguments_append_signature_arrays_without_changing_payload() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let tx_hash = H256::repeat_byte(0x92);
        let prepare: OutgoingRequest<Runtime> = OutgoingPrepareForMigration {
            author: [3; 32].into(),
            nonce: 2,
            network_id: 0,
            timepoint: timepoint(),
        }
        .into();
        let migrate: OutgoingRequest<Runtime> = OutgoingMigrate {
            author: [3; 32].into(),
            new_contract_address: EthAddress::repeat_byte(0x93),
            erc20_native_tokens: vec![],
            nonce: 3,
            network_id: 0,
            timepoint: timepoint(),
            new_signature_version: BridgeSignatureVersion::V3,
        }
        .into();
        let prepare = prepare.to_eth_abi(tx_hash).unwrap();
        let expected = vec![
            Token::Address(crate::BridgeContractAddress::<Runtime>::get(0).0.into()),
            Token::FixedBytes(tx_hash.0.to_vec()),
        ];
        assert_eq!(prepare.input_tokens(None), expected);
        let mut expected_with_signatures = expected;
        expected_with_signatures.extend(vec![Token::Array(vec![]); 3]);
        assert_eq!(prepare.input_tokens(Some(vec![])), expected_with_signatures);
        let migrate = migrate.to_eth_abi(tx_hash).unwrap();
        let expected = vec![Token::FixedBytes(tx_hash.0.to_vec())];
        assert_eq!(migrate.input_tokens(None), expected);
        let mut expected_with_signatures = expected;
        expected_with_signatures.extend(vec![Token::Array(vec![]); 3]);
        assert_eq!(migrate.input_tokens(Some(vec![])), expected_with_signatures);
    });
}

#[test]
fn recovery_asset_registration_bypasses_queue_capacity_but_preserves_admission_checks() {
    use super::mock::{EthBridge, System};
    use crate::migration::queue_ethereum_xor_thischain_add_asset_unchecked_capacity as queue;
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        crate::migration::decommission_legacy_ethereum_xor::<Runtime>();
        let authority = state.authority_account_id.clone();
        let nonce = System::account_nonce(&authority);
        crate::BridgeStatuses::<Runtime>::insert(0, BridgeStatus::Migrating);
        assert_err!(queue::<Runtime>(), Error::ContractIsInMigrationStage);
        crate::BridgeStatuses::<Runtime>::insert(0, BridgeStatus::Initialized);
        crate::BridgeSignatureVersions::<Runtime>::insert(0, BridgeSignatureVersion::V1);
        assert_err!(queue::<Runtime>(), Error::WeakLegacySigningDisabled);
        crate::BridgeSignatureVersions::<Runtime>::insert(0, BridgeSignatureVersion::V3);
        let full_queue = vec![H256::repeat_byte(0xa1); 64];
        crate::RequestsQueue::<Runtime>::insert(0, full_queue.clone());
        assert_ok!(queue::<Runtime>());
        assert_eq!(System::account_nonce(&authority), nonce + 1);
        let (request, hash) = super::last_outgoing_request(0).unwrap();
        assert!(matches!(request, OutgoingRequest::AddAsset(OutgoingAddAsset { asset_id, .. }) if asset_id == common::XOR));
        assert_eq!(crate::RequestsQueue::<Runtime>::get(0)[..64], full_queue);
        assert_eq!(crate::RequestStatuses::<Runtime>::get(0, hash), Some(RequestStatus::Pending));
        assert!(crate::AccountRequests::<Runtime>::get(&authority).contains(&(0, hash)));
        assert_eq!(crate::RequestSubmissionHeight::<Runtime>::get(0, hash), System::block_number());
        assert_err!(queue::<Runtime>(), Error::TokenIsAlreadyAdded);
        assert_eq!(System::account_nonce(&authority), nonce + 1);
        assert!(EthBridge::is_add_asset_request_pending(0, common::XOR));
        crate::BridgeStatuses::<Runtime>::remove(0);
        assert_err!(queue::<Runtime>(), Error::UnknownNetwork);
        crate::AuthorityAccount::<Runtime>::kill();
        assert_err!(queue::<Runtime>(), Error::AuthorityAccountNotSet);
    });
}

#[test]
fn stepped_account_migration_rejects_bad_versions_budgets_cursors_and_encodings() {
    use crate::migration::AccountRequestsV3ToV4 as Migration;
    use frame_support::migrations::{SteppedMigration, SteppedMigrationError};
    use frame_support::storage::StoragePrefixedMap;
    use frame_support::traits::{GetStorageVersion, StorageVersion};
    use frame_support::weights::{Weight, WeightMeter};
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_eq!(Migration::<Runtime>::id().version_from, 3);
        assert_eq!(Migration::<Runtime>::id().version_to, 4);
        assert_eq!(Migration::<Runtime>::max_steps(), Some(32));
        assert_eq!(
            Migration::<Runtime>::migrating_prefixes()
                .unwrap()
                .into_iter()
                .collect::<Vec<_>>(),
            vec![crate::AccountRequests::<Runtime>::final_prefix().to_vec()]
        );
        let mut meter = WeightMeter::with_limit(Weight::zero());
        assert!(matches!(
            Migration::<Runtime>::step(None, &mut meter),
            Err(SteppedMigrationError::InsufficientWeight { .. })
        ));
        StorageVersion::new(2).put::<crate::Pallet<Runtime>>();
        assert_eq!(
            Migration::<Runtime>::step(None, &mut WeightMeter::with_limit(Weight::MAX)),
            Err(SteppedMigrationError::Failed)
        );
        StorageVersion::new(3).put::<crate::Pallet<Runtime>>();
        let account: super::mock::AccountId = [0xa2; 32].into();
        let key = crate::AccountRequests::<Runtime>::hashed_key_for(&account);
        let mut malformed_key = key.clone();
        malformed_key[32] ^= 1;
        assert_eq!(
            Migration::<Runtime>::step(
                Some(malformed_key.try_into().unwrap()),
                &mut WeightMeter::with_limit(Weight::MAX)
            ),
            Err(SteppedMigrationError::InvalidCursor)
        );
        let mut extra_key = key.clone();
        extra_key.push(0);
        assert_eq!(
            Migration::<Runtime>::step(
                Some(extra_key.try_into().unwrap()),
                &mut WeightMeter::with_limit(Weight::MAX)
            ),
            Err(SteppedMigrationError::InvalidCursor)
        );
        let mut truncated_key = key.clone();
        truncated_key.truncate(49);
        assert_eq!(
            Migration::<Runtime>::step(
                Some(truncated_key.try_into().unwrap()),
                &mut WeightMeter::with_limit(Weight::MAX)
            ),
            Err(SteppedMigrationError::InvalidCursor)
        );
        let history = vec![(0u32, H256::repeat_byte(0xa3))];
        crate::AccountRequests::<Runtime>::insert(&account, history.clone());
        for limit in [1_000_000, 6_000_000] {
            let mut meter = WeightMeter::with_limit(Weight::from_parts(limit, u64::MAX));
            assert!(matches!(
                Migration::<Runtime>::step(None, &mut meter),
                Err(SteppedMigrationError::InsufficientWeight { .. })
            ));
            assert_eq!(crate::AccountRequests::<Runtime>::get(&account), history);
        }
        let mut trailing = history.encode();
        trailing.push(0);
        for malformed in [vec![], codec::Compact(65_537u32).encode(), trailing] {
            sp_io::storage::set(&key, &malformed);
            assert_eq!(
                Migration::<Runtime>::step(None, &mut WeightMeter::with_limit(Weight::MAX)),
                Err(SteppedMigrationError::Failed)
            );
            assert_eq!(
                sp_io::storage::get(&key).map(|bytes| bytes.to_vec()),
                Some(malformed)
            );
            assert_eq!(
                crate::Pallet::<Runtime>::on_chain_storage_version(),
                StorageVersion::new(3)
            );
        }
    });
}

#[test]
fn bridge_asset_metadata_normalization_and_network_registration_are_stable() {
    use super::mock::{EthBridge, RuntimeOrigin};
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let asset = crate::BridgeAssetData::<Runtime>::new(
            "USD.Coin.Token",
            "USDC",
            6,
            EthAddress::repeat_byte(0xa4),
        );
        let same = crate::BridgeAssetData::<Runtime>::new(
            "USD Coin Token",
            "USDC",
            6,
            EthAddress::repeat_byte(0xa4),
        );
        assert_eq!(asset.name.0, b"USD Coin Token");
        assert_eq!(asset.asset_id, same.asset_id);
        let distinct = crate::BridgeAssetData::<Runtime>::new(
            "USD Coin Token",
            "USDC",
            6,
            EthAddress::repeat_byte(0xa5),
        );
        assert_ne!(asset.asset_id, distinct.asset_id);
        let config = crate::AssetConfig::sidechain(asset.asset_id, asset.sidechain_asset_id, 6);
        assert_eq!(config.asset_id(), &asset.asset_id);
        assert_eq!(config.kind(), AssetKind::Sidechain);
        assert_eq!(BridgeStatus::default(), BridgeStatus::Initialized);
        let net = crate::NextNetworkId::<Runtime>::get();
        let contract = EthAddress::repeat_byte(0xa6);
        let peers: Vec<super::mock::AccountId> = vec![[0xa7; 32].into(), [0xa8; 32].into()];
        assert_ok!(EthBridge::register_bridge(
            RuntimeOrigin::root(),
            contract,
            peers.clone(),
            BridgeSignatureVersion::V3
        ));
        assert_eq!(crate::BridgeContractAddress::<Runtime>::get(net), contract);
        assert_eq!(
            crate::BridgeStatuses::<Runtime>::get(net),
            Some(BridgeStatus::Initialized)
        );
        assert_eq!(
            crate::BridgeSignatureVersions::<Runtime>::get(net),
            BridgeSignatureVersion::V3
        );
        assert_eq!(
            crate::Peers::<Runtime>::get(net),
            peers.into_iter().collect()
        );
        assert!(crate::BridgeAccount::<Runtime>::contains_key(net));
        assert_eq!(crate::NextNetworkId::<Runtime>::get(), net + 1);
    });
}

#[test]
fn asset_token_and_peer_call_arguments_keep_signature_tail_and_metadata() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let author: super::mock::AccountId = [0xb1; 32].into();
        let address = EthAddress::repeat_byte(0xb2);
        let hash = H256::repeat_byte(0xb3);
        let sidechain_asset: H256 = common::PSWAP.into();
        let cases: Vec<(OutgoingRequest<Runtime>, Vec<Token>)> = vec![
            (
                OutgoingAddAsset {
                    author: author.clone(),
                    asset_id: common::PSWAP,
                    nonce: 1,
                    network_id: 0,
                    timepoint: timepoint(),
                }
                .into(),
                vec![
                    Token::String("T".into()),
                    Token::String("T".into()),
                    Token::Uint(18u64.into()),
                    Token::FixedBytes(sidechain_asset.0.to_vec()),
                ],
            ),
            (
                OutgoingAddToken {
                    author: author.clone(),
                    token_address: address,
                    symbol: "USD".into(),
                    name: "Dollar".into(),
                    decimals: 6,
                    nonce: 2,
                    network_id: 0,
                    timepoint: timepoint(),
                }
                .into(),
                vec![
                    Token::Address(address.0.into()),
                    Token::String("USD".into()),
                    Token::String("Dollar".into()),
                    Token::Uint(6u64.into()),
                ],
            ),
            (
                OutgoingAddPeer {
                    author: author.clone(),
                    peer_account_id: [0xb4; 32].into(),
                    peer_address: address,
                    nonce: 3,
                    network_id: 0,
                    timepoint: timepoint(),
                }
                .into(),
                vec![
                    Token::Address(address.0.into()),
                    Token::FixedBytes(hash.0.to_vec()),
                ],
            ),
            (
                OutgoingRemovePeer {
                    author: author.clone(),
                    peer_account_id: [0xb4; 32].into(),
                    peer_address: address,
                    nonce: 4,
                    network_id: 0,
                    timepoint: timepoint(),
                    compat_hash: None,
                }
                .into(),
                vec![
                    Token::Address(address.0.into()),
                    Token::FixedBytes(hash.0.to_vec()),
                ],
            ),
        ];
        for (request, expected) in cases {
            let offchain = OffchainRequest::outgoing(request.clone());
            assert_eq!(offchain.timepoint(), timepoint());
            assert_eq!(offchain.author(), &author);
            let encoded = request.to_eth_abi(hash).unwrap();
            assert_eq!(encoded.input_tokens(None), expected);
            let mut signed = expected;
            signed.extend(vec![Token::Array(vec![]); 3]);
            assert_eq!(encoded.input_tokens(Some(vec![])), signed);
        }
    });
}

#[test]
fn legacy_dispatch_errors_preserve_their_semantics_during_storage_upgrade() {
    use frame_support::sp_runtime::legacy::byte_sized_error::DispatchError as Old;
    use sp_runtime::{ArithmeticError, DispatchError as New, TokenError};
    let cases = vec![
        (Old::Other("legacy"), New::Other("legacy")),
        (Old::CannotLookup, New::CannotLookup),
        (Old::BadOrigin, New::BadOrigin),
        (Old::ConsumerRemaining, New::ConsumerRemaining),
        (Old::NoProviders, New::NoProviders),
        (Old::TooManyConsumers, New::TooManyConsumers),
        (
            Old::Token(TokenError::FundsUnavailable),
            New::Token(TokenError::FundsUnavailable),
        ),
        (
            Old::Arithmetic(ArithmeticError::Overflow),
            New::Arithmetic(ArithmeticError::Overflow),
        ),
    ];
    for (old, expected) in cases {
        assert_eq!(crate::migration::migrate_error(old), expected);
    }
}

#[test]
fn current_peers_cancel_an_orphaned_multisig_only_after_quorum() {
    use super::mock::{EthBridge, RuntimeOrigin};
    use frame_support::weights::Weight;
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let network = &state.networks[&0];
        let bridge = network.config.bridge_account_id.clone();
        let peers = network
            .ocw_keypairs
            .iter()
            .map(|(_, account, _)| account.clone())
            .collect::<Vec<_>>();
        let hash = [0xc1; 32];
        assert_ok!(bridge_multisig::Pallet::<Runtime>::approve_as_multi(
            RuntimeOrigin::signed(peers[0].clone()),
            bridge.clone(),
            None,
            hash,
            Weight::zero()
        ));
        let when = bridge_multisig::Pallet::<Runtime>::thischain_timepoint();
        let call = crate::Call::<Runtime>::cancel_pending_multisig {
            network_id: 0,
            call_hash: hash,
            timepoint: when,
        };
        let outsider: super::mock::AccountId = [0xc2; 32].into();
        assert_eq!(
            EthBridge::validate_peer_protocol_call(&outsider, &call),
            Ok(false)
        );
        assert_err!(
            EthBridge::cancel_pending_multisig(RuntimeOrigin::signed(outsider), 0, hash, when),
            Error::Forbidden
        );
        assert_eq!(
            EthBridge::validate_peer_protocol_call(&peers[0], &call),
            Ok(true)
        );
        assert!(!bridge_multisig::CancellationApprovals::<Runtime>::contains_key(&bridge, hash));
        for peer in &peers[..2] {
            assert_ok!(EthBridge::cancel_pending_multisig(
                RuntimeOrigin::signed(peer.clone()),
                0,
                hash,
                when
            ));
            assert!(bridge_multisig::Multisigs::<Runtime>::contains_key(
                &bridge, hash
            ));
        }
        assert_err!(
            EthBridge::validate_peer_protocol_call(&peers[0], &call),
            bridge_multisig::Error::<Runtime>::AlreadyApproved
        );
        assert_ok!(EthBridge::cancel_pending_multisig(
            RuntimeOrigin::signed(peers[2].clone()),
            0,
            hash,
            when
        ));
        assert!(!bridge_multisig::Multisigs::<Runtime>::contains_key(
            &bridge, hash
        ));
        assert!(!bridge_multisig::CancellationApprovals::<Runtime>::contains_key(&bridge, hash));
        assert_err!(
            EthBridge::validate_peer_protocol_call(&peers[2], &call),
            bridge_multisig::Error::<Runtime>::NotFound
        );
        crate::BridgeAccount::<Runtime>::remove(0);
        assert_err!(
            EthBridge::validate_peer_protocol_call(&peers[0], &call),
            Error::UnknownNetwork
        );
        assert_err!(
            EthBridge::cancel_pending_multisig(
                RuntimeOrigin::signed(peers[0].clone()),
                0,
                hash,
                when
            ),
            Error::UnknownNetwork
        );
    });
}

#[test]
fn outgoing_protocol_admission_rejects_bad_signatures_duplicates_and_approval_overflow() {
    use super::mock::{EthBridge, RuntimeOrigin};
    use crate::offchain::SignatureParams;
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_ok!(EthBridge::prepare_for_migration(RuntimeOrigin::root(), 0));
        let (request, hash) = super::last_outgoing_request(0).unwrap();
        let (_, peer, seed) = &state.networks[&0].ocw_keypairs[0];
        let secret = secp256k1::SecretKey::parse_slice(seed).unwrap();
        let public = sp_core::ecdsa::Public::from_raw(
            secp256k1::PublicKey::from_secret_key(&secret).serialize_compressed(),
        );
        let message = common::eth::prepare_message(request.to_eth_abi(hash).unwrap().as_raw());
        let signature = super::get_signature_params(&secp256k1::sign(&message, &secret));
        let call = |signature_params| crate::Call::<Runtime>::approve_request {
            ocw_public: public.clone(),
            hash,
            signature_params,
            network_id: 0,
        };
        assert_eq!(
            EthBridge::validate_peer_protocol_call(
                &state.authority_account_id,
                &call(signature.clone())
            ),
            Ok(false)
        );
        let mut invalid = signature.clone();
        invalid.r = [0xff; 32];
        assert_err!(
            EthBridge::validate_peer_protocol_call(peer, &call(invalid)),
            Error::InvalidSignature
        );
        assert_eq!(
            EthBridge::validate_peer_protocol_call(peer, &call(signature.clone())),
            Ok(true)
        );
        assert!(crate::RequestApprovers::<Runtime>::get(0, hash).is_empty());
        assert_ok!(EthBridge::approve_request(
            RuntimeOrigin::signed(peer.clone()),
            public.clone(),
            hash,
            signature.clone(),
            0
        ));
        assert_err!(
            EthBridge::validate_peer_protocol_call(peer, &call(signature.clone())),
            Error::DuplicateApproval
        );
        crate::RequestApprovers::<Runtime>::remove(0, hash);
        let approvals = (0..MAX_PEERS)
            .map(|i| SignatureParams {
                r: [i as u8; 32],
                s: [1; 32],
                v: 27,
            })
            .collect::<std::collections::BTreeSet<_>>();
        crate::RequestApprovals::<Runtime>::insert(0, hash, approvals);
        assert_err!(
            EthBridge::validate_peer_protocol_call(peer, &call(signature.clone())),
            Error::TooManyApprovals
        );
        crate::RequestApprovals::<Runtime>::remove(0, hash);
        crate::RequestStatuses::<Runtime>::insert(0, hash, RequestStatus::Frozen);
        assert_err!(
            EthBridge::validate_peer_protocol_call(peer, &call(signature.clone())),
            Error::RequestIsNotReady
        );
        crate::RequestStatuses::<Runtime>::insert(0, hash, RequestStatus::Pending);
        sp_io::storage::set(crate::recovery::HALT_KEY, &[]);
        assert_err!(
            EthBridge::validate_peer_protocol_call(peer, &call(signature)),
            Error::RecoveryHalted
        );
    });
}

#[test]
fn recovery_import_reuses_only_an_exact_unique_pending_load() {
    use super::mock::{EthBridge, RuntimeOrigin, System};
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let bridge = state.networks[&0].config.bridge_account_id.clone();
        let hash = H256::repeat_byte(0xc3);
        super::request_incoming(
            state.authority_account_id.clone(),
            hash,
            IncomingTransactionRequestKind::PrepareForMigration.into(),
            0,
        )
        .unwrap();
        let OffchainRequest::LoadIncoming(load) = super::last_request(0).unwrap() else {
            panic!("load request expected")
        };
        let original = crate::Requests::<Runtime>::get(0, hash).unwrap();
        let incoming: IncomingRequest<Runtime> = IncomingPrepareForMigration {
            author: state.authority_account_id.clone(),
            tx_hash: hash,
            at_height: 80,
            timepoint: load.timepoint(),
            network_id: 0,
        }
        .into();
        let incoming_hash = OffchainRequest::incoming(incoming.clone()).hash();
        sp_io::storage::set(crate::recovery::HALT_KEY, &[]);
        assert_err!(
            EthBridge::import_incoming_request(
                RuntimeOrigin::signed(bridge.clone()),
                load.clone(),
                Ok(incoming.clone())
            ),
            Error::RecoveryHalted
        );
        let block = System::block_number() as u32;
        sp_io::storage::set(crate::recovery::INBOUND_CONTEXT_KEY, &block.encode());
        assert_eq!(EthBridge::retained_recovery_load(&load), Ok(true));
        let mut different = load.clone();
        if let LoadIncomingRequest::Transaction(value) = &mut different {
            value.author = [0xc4; 32].into();
        }
        assert_eq!(EthBridge::retained_recovery_load(&different), Ok(false));
        crate::RequestStatuses::<Runtime>::insert(0, hash, RequestStatus::Frozen);
        assert_err!(
            EthBridge::retained_recovery_load(&load),
            Error::RequestIsAlreadyRegistered
        );
        crate::RequestStatuses::<Runtime>::insert(0, hash, RequestStatus::Pending);
        crate::RequestsQueue::<Runtime>::insert(0, vec![hash, hash]);
        assert_err!(
            EthBridge::retained_recovery_load(&load),
            Error::RequestIsAlreadyRegistered
        );
        crate::RequestsQueue::<Runtime>::insert(0, vec![hash]);
        crate::LoadToIncomingRequestHash::<Runtime>::insert(0, hash, incoming_hash);
        assert_err!(
            EthBridge::retained_recovery_load(&load),
            Error::RequestIsAlreadyRegistered
        );
        crate::LoadToIncomingRequestHash::<Runtime>::remove(0, hash);
        let call = crate::Call::<Runtime>::import_incoming_request {
            load_incoming_request: load.clone(),
            incoming_request_result: Ok(incoming.clone()),
        };
        assert_eq!(
            EthBridge::validate_peer_protocol_call(&bridge, &call),
            Ok(true)
        );
        assert_ok!(EthBridge::import_incoming_request(
            RuntimeOrigin::signed(bridge),
            load.clone(),
            Ok(incoming)
        ));
        assert_eq!(crate::Requests::<Runtime>::get(0, hash), Some(original));
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(0, hash),
            Some(RequestStatus::Done)
        );
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(0, incoming_hash),
            Some(RequestStatus::Done)
        );
        assert!(crate::RequestsQueue::<Runtime>::get(0).is_empty());
        assert_eq!(
            crate::LoadToIncomingRequestHash::<Runtime>::get(0, hash),
            incoming_hash
        );
        assert_err!(
            EthBridge::retained_recovery_load(&load),
            Error::RequestIsAlreadyRegistered
        );
    });
}

#[test]
fn bridge_app_rejects_foreign_networks_and_reports_unsupported_refunds() {
    use super::mock::{AccountId, AssetId, EthBridge};
    use bridge_types::traits::BridgeApp;
    use frame_support::weights::Weight;
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let wrong = bridge_types::GenericNetworkId::EVMLegacy(99);
        assert!(!<EthBridge as BridgeApp<AccountId, EthAddress, AssetId, u128>>::is_asset_supported(wrong, common::PSWAP));
        assert!(<EthBridge as BridgeApp<AccountId, EthAddress, AssetId, u128>>::list_supported_assets(wrong).is_empty());
        assert_err!(<EthBridge as BridgeApp<AccountId, EthAddress, AssetId, u128>>::transfer(wrong, common::PSWAP, state.authority_account_id.clone(), EthAddress::zero(), 1), Error::UnknownNetwork);
        assert_err!(<EthBridge as BridgeApp<AccountId, EthAddress, AssetId, u128>>::refund(bridge_types::GenericNetworkId::EVMLegacy(0), H256::zero(), state.authority_account_id.clone(), common::PSWAP, 1), Error::Unavailable);
        assert_eq!(<EthBridge as BridgeApp<AccountId, EthAddress, AssetId, u128>>::refund_weight(), Weight::zero());
        assert_eq!(<EthBridge as BridgeApp<AccountId, EthAddress, AssetId, u128>>::is_asset_supported_weight(), Weight::zero());
        assert!(<EthBridge as BridgeApp<AccountId, EthAddress, AssetId, u128>>::transfer_weight().ref_time() > 0);
        crate::BridgeContractAddress::<Runtime>::remove(0);
        crate::ValMasterContractAddress::<Runtime>::kill();
        assert!(<EthBridge as BridgeApp<AccountId, EthAddress, AssetId, u128>>::list_apps().is_empty());
    });
}

#[test]
fn cancelling_prepared_peer_addition_releases_reference_and_revokes_approvals() {
    use super::mock::System;
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let author = state.authority_account_id.clone();
        let peer: super::mock::AccountId = [0xc5; 32].into();
        System::inc_providers(&peer);
        let outgoing: OutgoingRequest<Runtime> = OutgoingAddPeer {
            author: author.clone(),
            peer_account_id: peer.clone(),
            peer_address: EthAddress::repeat_byte(0xc6),
            nonce: 0,
            network_id: 0,
            timepoint: timepoint(),
        }
        .into();
        let offchain = OffchainRequest::outgoing(outgoing.clone());
        let hash = offchain.hash();
        assert_ok!(offchain.validate());
        assert_ok!(offchain.prepare());
        assert_ok!(outgoing.finalize(hash));
        assert_eq!(System::consumers(&peer), 1);
        crate::Requests::<Runtime>::insert(0, hash, offchain);
        crate::RequestStatuses::<Runtime>::insert(0, hash, RequestStatus::ApprovalsReady);
        crate::RequestApprovers::<Runtime>::insert(
            0,
            hash,
            std::collections::BTreeSet::from([author.clone()]),
        );
        crate::RequestApprovals::<Runtime>::insert(
            0,
            hash,
            std::collections::BTreeSet::from([crate::offchain::SignatureParams {
                r: [1; 32],
                s: [2; 32],
                v: 27,
            }]),
        );
        crate::contract::FUNCTIONS.get_or_init(crate::contract::functions);
        let tx_input = encode_outgoing_request_eth_call::<Runtime>(
            *crate::contract::ADD_PEER_BY_PEER_ID.get().unwrap(),
            &outgoing,
            hash,
        )
        .unwrap();
        let initial_hash = H256::repeat_byte(0xc7);
        let incoming: IncomingRequest<Runtime> = IncomingCancelOutgoingRequest {
            outgoing_request: outgoing,
            outgoing_request_hash: hash,
            initial_request_hash: initial_hash,
            tx_input,
            author: author.clone(),
            tx_hash: H256::repeat_byte(0xc8),
            at_height: 91,
            timepoint: timepoint(),
            network_id: 0,
        }
        .into();
        assert_eq!(incoming.hash(), initial_hash);
        assert_eq!(incoming.at_height(), 91);
        assert_eq!(incoming.timepoint(), timepoint());
        assert_ok!(incoming.prepare());
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(0, hash),
            Some(RequestStatus::Frozen)
        );
        assert_ok!(incoming.cancel());
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(0, hash),
            Some(RequestStatus::ApprovalsReady)
        );
        assert_ok!(incoming.prepare());
        assert_eq!(incoming.finalize(), Ok(initial_hash));
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(0, hash),
            Some(RequestStatus::Failed(Error::Cancelled.into()))
        );
        assert_eq!(crate::PendingPeer::<Runtime>::get(0), None);
        assert_eq!(System::consumers(&peer), 0);
        assert!(crate::RequestApprovers::<Runtime>::get(0, hash).is_empty());
        assert!(crate::RequestApprovals::<Runtime>::get(0, hash).is_empty());
        let done: IncomingRequest<Runtime> =
            IncomingRequest::MarkAsDone(IncomingMarkAsDoneRequest {
                outgoing_request_hash: hash,
                initial_request_hash: H256::repeat_byte(0xc9),
                author,
                at_height: 92,
                timepoint: timepoint(),
                network_id: 0,
            });
        assert_eq!(done.timepoint(), timepoint());
        assert_eq!(done.at_height(), 92);
        assert_err!(done.validate(), Error::RequestIsNotReady);
        assert_ok!(done.cancel());
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(0, hash),
            Some(RequestStatus::Failed(Error::Cancelled.into()))
        );
        crate::RequestStatuses::<Runtime>::insert(0, hash, RequestStatus::ApprovalsReady);
        assert_eq!(done.finalize(), Ok(H256::repeat_byte(0xc9)));
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(0, hash),
            Some(RequestStatus::Done)
        );
    });
}

#[test]
fn recovery_transfer_resolution_excludes_custody_or_credits_only_its_committed_beneficiary() {
    use super::mock::{EthBridge, System};
    use crate::recovery::{HashiResolution, HALT_KEY, HASHI_RESOLUTION_KEY, INBOUND_CONTEXT_KEY};
    use common::{AssetInfoProvider, AssetName, AssetSymbol};
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let bridge = state.networks[&0].config.bridge_account_id.clone();
        let author = state.authority_account_id.clone();
        let recipient: super::mock::AccountId = [0xca; 32].into();
        let beneficiary: super::mock::AccountId = [0xcb; 32].into();
        let sidechain_asset = EthBridge::register_sidechain_asset(
            EthAddress::repeat_byte(0xcc),
            18,
            AssetSymbol(b"REC".to_vec()),
            AssetName(b"Recovery Coin".to_vec()),
            0,
        )
        .unwrap();
        sp_io::storage::set(HALT_KEY, &[]);
        sp_io::storage::set(
            INBOUND_CONTEXT_KEY,
            &(System::block_number() as u32).encode(),
        );
        for (asset_id, asset_kind, excluded) in [
            (common::VAL, AssetKind::SidechainOwned, true),
            (sidechain_asset, AssetKind::Sidechain, true),
            (common::VAL, AssetKind::SidechainOwned, false),
            (sidechain_asset, AssetKind::Sidechain, false),
        ] {
            let request = IncomingTransfer::<Runtime> {
                from: EthAddress::repeat_byte(0xcd),
                to: recipient.clone(),
                asset_id,
                asset_kind,
                amount: 100,
                author: author.clone(),
                tx_hash: H256::repeat_byte(0xce),
                at_height: 93,
                timepoint: timepoint(),
                network_id: 0,
                should_take_fee: false,
            };
            let resolution = HashiResolution {
                block: System::block_number() as u32,
                request_hash: sp_io::hashing::blake2_256(&request.encode()),
                beneficiary: beneficiary.clone(),
                excluded,
                consumed: false,
            };
            sp_io::storage::set(HASHI_RESOLUTION_KEY, &resolution.encode());
            let custody_before =
                assets::Pallet::<Runtime>::total_balance(&asset_id, &bridge).unwrap();
            let beneficiary_before =
                assets::Pallet::<Runtime>::total_balance(&asset_id, &beneficiary).unwrap();
            let recipient_before =
                assets::Pallet::<Runtime>::total_balance(&asset_id, &recipient).unwrap();
            assert_ok!(request.prepare());
            assert_eq!(request.finalize(), Ok(request.tx_hash));
            assert_eq!(
                assets::Pallet::<Runtime>::total_balance(&asset_id, &recipient).unwrap(),
                recipient_before
            );
            assert_eq!(
                assets::Pallet::<Runtime>::total_balance(&asset_id, &beneficiary).unwrap(),
                beneficiary_before + if excluded { 0 } else { 100 }
            );
            assert_eq!(
                assets::Pallet::<Runtime>::total_balance(&asset_id, &bridge).unwrap(),
                custody_before
                    - if !excluded && asset_kind.is_owned() {
                        100
                    } else {
                        0
                    }
            );
            assert_err!(request.finalize(), Error::RecoveryHalted);
        }
        sp_io::storage::clear(HALT_KEY);
        let mut charged = IncomingTransfer::<Runtime> {
            from: EthAddress::zero(),
            to: recipient,
            asset_id: common::VAL,
            asset_kind: AssetKind::SidechainOwned,
            amount: IncomingTransfer::<Runtime>::fee_amount() - 1,
            author,
            tx_hash: H256::repeat_byte(0xcf),
            at_height: 94,
            timepoint: timepoint(),
            network_id: 0,
            should_take_fee: false,
        };
        assert_ok!(charged.validate());
        charged.enable_taking_fee();
        assert!(charged.should_take_fee);
        assert_err!(charged.validate(), Error::UnableToPayFees);
    });
}

#[test]
fn peer_events_resolve_identity_and_complete_synchronization_in_either_order() {
    use super::mock::System;
    use crate::contract::ContractEvent;
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let author = state.authority_account_id.clone();
        let peer: super::mock::AccountId = [0xd1; 32].into();
        let address = EthAddress::repeat_byte(0xd2);
        let load = LoadIncomingTransactionRequest::<Runtime>::new(
            author.clone(),
            H256::repeat_byte(0xd3),
            timepoint(),
            IncomingTransactionRequestKind::AddPeer,
            0,
        );
        assert_err!(
            IncomingRequest::try_from_contract_event(
                ContractEvent::ChangePeers(address, false),
                load.clone(),
                95
            ),
            Error::UnknownPeerAddress
        );
        let unknown_removed = IncomingRequest::try_from_contract_event(
            ContractEvent::ChangePeers(address, true),
            load.clone(),
            95,
        )
        .unwrap();
        assert!(matches!(
            unknown_removed,
            IncomingRequest::ChangePeers(IncomingChangePeers {
                peer_account_id: None,
                removed: true,
                ..
            })
        ));
        crate::PeerAccountId::<Runtime>::insert(0, address, peer.clone());
        let added = IncomingRequest::try_from_contract_event(
            ContractEvent::ChangePeers(address, false),
            load,
            95,
        )
        .unwrap();
        assert_eq!(added.at_height(), 95);
        assert_eq!(added.timepoint(), timepoint());
        assert_ok!(added.cancel());
        let mut compat: IncomingRequest<Runtime> = IncomingChangePeersCompat {
            peer_account_id: peer.clone(),
            peer_address: address,
            added: true,
            contract: ChangePeersContract::VAL,
            author,
            tx_hash: H256::repeat_byte(0xd4),
            at_height: 96,
            timepoint: timepoint(),
            network_id: 0,
        }
        .into();
        assert_eq!(compat.at_height(), 96);
        assert_eq!(compat.timepoint(), timepoint());
        assert_ok!(compat.cancel());
        crate::PendingPeer::<Runtime>::insert(0, super::mock::AccountId::new([0xd5; 32]));
        assert_err!(added.finalize(), Error::WrongPendingPeer);
        assert_err!(compat.finalize(), Error::WrongPendingPeer);
        crate::PendingPeer::<Runtime>::insert(0, peer.clone());
        System::inc_providers(&peer);
        assert_ok!(System::inc_consumers(&peer));
        assert_eq!(compat.finalize(), Ok(H256::repeat_byte(0xd4)));
        assert!(!crate::Peers::<Runtime>::get(0).contains(&peer));
        assert_eq!(added.finalize(), Ok(H256::repeat_byte(0xd3)));
        assert!(crate::Peers::<Runtime>::get(0).contains(&peer));
        assert_eq!(crate::PendingPeer::<Runtime>::get(0), None);
        assert!(!crate::PendingEthPeersSync::<Runtime>::get().is_ready());
        // Removal acknowledges an already removed peer; Main may arrive before VAL.
        let removal = OutgoingRemovePeer::<Runtime> {
            author: state.authority_account_id.clone(),
            peer_account_id: peer.clone(),
            peer_address: address,
            nonce: 1,
            network_id: 0,
            timepoint: timepoint(),
            compat_hash: None,
        };
        assert_ok!(removal.finalize());
        assert!(!crate::Peers::<Runtime>::get(0).contains(&peer));
        crate::PendingPeer::<Runtime>::insert(0, peer.clone());
        let mut removed = match added {
            IncomingRequest::ChangePeers(value) => value,
            _ => unreachable!(),
        };
        removed.removed = true;
        assert_eq!(removed.finalize(), Ok(removed.tx_hash));
        if let IncomingRequest::ChangePeersCompat(value) = &mut compat {
            value.added = false;
        }
        assert_eq!(compat.finalize(), Ok(H256::repeat_byte(0xd4)));
        assert_eq!(System::consumers(&peer), 0);
        assert_eq!(crate::PendingPeer::<Runtime>::get(0), None);
        assert!(!crate::PendingEthPeersSync::<Runtime>::get().is_ready());
    });
}

#[test]
fn halted_dispatch_and_internal_registration_leave_request_bookkeeping_unchanged() {
    use super::mock::{EthBridge, RuntimeOrigin};
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let author = state.authority_account_id.clone();
        let bridge = state.networks[&0].config.bridge_account_id.clone();
        let incoming: IncomingRequest<Runtime> = IncomingPrepareForMigration {
            author: author.clone(),
            tx_hash: H256::repeat_byte(0xe1),
            at_height: 100,
            timepoint: timepoint(),
            network_id: 0,
        }
        .into();
        let offchain = OffchainRequest::incoming(incoming.clone());
        let hash = offchain.hash();
        let load = LoadIncomingRequest::Transaction(LoadIncomingTransactionRequest::new(
            author.clone(),
            incoming.hash(),
            timepoint(),
            IncomingTransactionRequestKind::PrepareForMigration,
            0,
        ));
        crate::RequestStatuses::<Runtime>::insert(0, hash, RequestStatus::Pending);
        sp_io::storage::set(crate::recovery::HALT_KEY, &[]);
        let before = sp_io::storage::root(sp_runtime::StateVersion::V1);
        assert_err!(
            EthBridge::request_from_sidechain(
                RuntimeOrigin::signed(author),
                incoming.hash(),
                IncomingRequestKind::Transaction(
                    IncomingTransactionRequestKind::PrepareForMigration
                ),
                0
            ),
            Error::RecoveryHalted
        );
        assert_err!(
            EthBridge::register_incoming_request(
                RuntimeOrigin::signed(bridge.clone()),
                incoming.clone()
            ),
            Error::RecoveryHalted
        );
        assert_err!(
            EthBridge::finalize_incoming_request(RuntimeOrigin::signed(bridge), hash, 0),
            Error::RecoveryHalted
        );
        assert_err!(
            EthBridge::validate_fresh_incoming(&incoming, 0),
            Error::RecoveryHalted
        );
        assert_err!(
            EthBridge::register_incoming_request_inner(&offchain, 0),
            Error::RecoveryHalted
        );
        assert_err!(EthBridge::add_request(&offchain), Error::RecoveryHalted);
        assert_err!(
            EthBridge::finalize_incoming_request_inner(&incoming, hash, 0),
            Error::RecoveryHalted
        );
        assert_err!(
            EthBridge::inner_import_incoming_request(0, load, Ok(incoming)),
            Error::RecoveryHalted
        );
        assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), before);
    });
}

#[test]
fn rejected_imports_close_the_load_without_creating_an_executable_incoming_request() {
    use super::mock::EthBridge;
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let author = state.authority_account_id.clone();
        for (index, queue_len, fail_preparation) in
            [(0u8, 63usize, false), (1, 0, true), (2, 0, false)]
        {
            let tx_hash = H256::repeat_byte(0xe2 + index);
            let load = LoadIncomingRequest::Transaction(LoadIncomingTransactionRequest::new(
                author.clone(),
                tx_hash,
                timepoint(),
                IncomingTransactionRequestKind::PrepareForMigration,
                0,
            ));
            let incoming: IncomingRequest<Runtime> = IncomingPrepareForMigration {
                author: author.clone(),
                tx_hash,
                at_height: 100,
                timepoint: timepoint(),
                network_id: 0,
            }
            .into();
            let incoming_hash = OffchainRequest::incoming(incoming.clone()).hash();
            let queue = (0..queue_len)
                .map(|i| H256::from_low_u64_be(i as u64 + 1))
                .collect::<Vec<_>>();
            crate::RequestsQueue::<Runtime>::insert(0, queue.clone());
            crate::BridgeStatuses::<Runtime>::insert(
                0,
                if fail_preparation {
                    BridgeStatus::Migrating
                } else {
                    BridgeStatus::Initialized
                },
            );
            let error: sp_runtime::DispatchError = if queue_len > 0 {
                Error::RequestsQueueFull
            } else if fail_preparation {
                Error::ContractIsAlreadyInMigrationStage
            } else {
                Error::InvalidFunctionInput
            }
            .into();
            let result = if index == 2 { Err(error) } else { Ok(incoming) };
            assert_ok!(EthBridge::inner_import_incoming_request(0, load, result));
            assert_eq!(
                crate::RequestStatuses::<Runtime>::get(0, tx_hash),
                Some(RequestStatus::Failed(error.into()))
            );
            assert_eq!(crate::RequestsQueue::<Runtime>::get(0), queue);
            assert!(!crate::Requests::<Runtime>::contains_key(0, incoming_hash));
            assert_eq!(
                crate::RequestStatuses::<Runtime>::get(0, incoming_hash),
                if fail_preparation {
                    Some(RequestStatus::Failed(error.into()))
                } else {
                    None
                }
            );
        }
    });
}

#[test]
fn admission_capacity_duplicate_rows_and_nonpending_transitions_are_rejected() {
    use super::mock::{EthBridge, MaxRequestsPerAccountConst, RuntimeOrigin};
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let author = state.authority_account_id.clone();
        let incoming: IncomingRequest<Runtime> = IncomingPrepareForMigration {
            author: author.clone(),
            tx_hash: H256::repeat_byte(0xe5),
            at_height: 101,
            timepoint: timepoint(),
            network_id: 0,
        }
        .into();
        let offchain = OffchainRequest::incoming(incoming.clone());
        let hash = offchain.hash();
        let load = LoadIncomingRequest::Transaction(LoadIncomingTransactionRequest::new(
            author.clone(),
            incoming.hash(),
            timepoint(),
            IncomingTransactionRequestKind::PrepareForMigration,
            0,
        ));
        let call = crate::Call::import_incoming_request {
            load_incoming_request: load.clone(),
            incoming_request_result: Ok(incoming.clone()),
        };
        let queue = (0..64).map(H256::from_low_u64_be).collect::<Vec<_>>();
        crate::RequestsQueue::<Runtime>::insert(0, queue.clone());
        assert_err!(
            EthBridge::validate_peer_protocol_call(
                &state.networks[&0].config.bridge_account_id,
                &call
            ),
            Error::RequestsQueueFull
        );
        assert_err!(
            EthBridge::validate_fresh_incoming(&incoming, 0),
            Error::RequestsQueueFull
        );
        assert_err!(
            EthBridge::register_incoming_request_inner(&offchain, 0),
            Error::RequestsQueueFull
        );
        assert_eq!(crate::RequestsQueue::<Runtime>::get(0), queue);
        crate::RequestsQueue::<Runtime>::remove(0);
        crate::Requests::<Runtime>::insert(0, hash, offchain.clone());
        assert_err!(
            EthBridge::register_incoming_request_inner(&offchain, 0),
            Error::RequestIsAlreadyRegistered
        );
        crate::RequestStatuses::<Runtime>::insert(0, hash, RequestStatus::Done);
        assert_err!(
            EthBridge::finalize_incoming_request_inner(&incoming, hash, 0),
            Error::ExpectedPendingRequest
        );
        assert_err!(
            EthBridge::inner_abort_request(&offchain, hash, Error::Cancelled.into(), 0),
            Error::ExpectedPendingRequest
        );
        crate::Requests::<Runtime>::remove(0, hash);
        crate::RequestStatuses::<Runtime>::insert(
            0,
            hash,
            RequestStatus::Failed(Error::Cancelled.into()),
        );
        assert_err!(
            EthBridge::reset_request_signatures(RuntimeOrigin::root(), 0, hash),
            Error::UnknownRequest
        );
        crate::AccountRequests::<Runtime>::insert(&author, vec![(0, hash)]);
        MaxRequestsPerAccountConst::set(0);
        EthBridge::record_account_request(&author, 0, incoming.hash());
        assert!(crate::AccountRequests::<Runtime>::get(&author).is_empty());
        MaxRequestsPerAccountConst::set(3);
        assert_err!(
            EthBridge::register_sidechain_asset(
                EthAddress::repeat_byte(0xe5),
                19,
                common::AssetSymbol(b"TOO".to_vec()),
                common::AssetName(b"Too Precise".to_vec()),
                0
            ),
            Error::UnsupportedAssetPrecision
        );
        crate::RequestApprovers::<Runtime>::insert(
            0,
            hash,
            std::collections::BTreeSet::from([author.clone()]),
        );
        sp_io::storage::set(&bridge_types::recovery::hashi_key(0u32, hash), &[1]);
        EthBridge::clear_request_signatures(0, &hash);
        assert_eq!(
            crate::RequestApprovers::<Runtime>::get(0, hash),
            std::collections::BTreeSet::from([author])
        );
    });
}

#[test]
fn peer_requests_reject_inconsistent_membership_and_missing_account_references() {
    use super::mock::{EthBridge, RuntimeOrigin};
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let author = state.authority_account_id.clone();
        let peer: super::mock::AccountId = [0xe6; 32].into();
        let existing = state.networks[&0].ocw_keypairs[0].1.clone();
        let bridge = state.networks[&0].config.bridge_account_id.clone();
        let address = EthAddress::repeat_byte(0xe6);
        let mut add = OutgoingAddPeer::<Runtime> {
            author: author.clone(),
            peer_account_id: peer.clone(),
            peer_address: address,
            nonce: 1,
            network_id: 0,
            timepoint: timepoint(),
        };
        assert_err!(add.prepare(()), Error::IncRefError);
        assert_eq!(crate::PendingPeer::<Runtime>::get(0), None);
        add.peer_account_id = existing.clone();
        assert_err!(add.validate(), Error::PeerIsAlreadyAdded);
        let mut compat = OutgoingAddPeerCompat::<Runtime> {
            author: author.clone(),
            peer_account_id: existing.clone(),
            peer_address: address,
            nonce: 2,
            network_id: 0,
            timepoint: timepoint(),
        };
        assert_err!(compat.validate(), Error::PeerIsAlreadyAdded);
        compat.peer_account_id = peer.clone();
        assert_err!(compat.validate(), Error::NoPendingPeer);
        assert_eq!(
            OffchainRequest::outgoing(OutgoingRequest::AddPeerCompat(compat)).timepoint(),
            timepoint()
        );
        assert_ok!(add.cancel());
        let mut peers = crate::Peers::<Runtime>::get(0);
        peers.insert([0xe7; 32].into());
        crate::Peers::<Runtime>::insert(0, peers.clone());
        let mut remove = OutgoingRemovePeer::<Runtime> {
            author: author.clone(),
            peer_account_id: peer.clone(),
            peer_address: address,
            nonce: 3,
            network_id: 0,
            timepoint: timepoint(),
            compat_hash: None,
        };
        assert_err!(remove.validate(), Error::UnknownPeerId);
        assert_err!(remove.prepare(()), Error::IncRefError);
        assert_ok!(remove.cancel());
        let remove_compat = OutgoingRemovePeerCompat::<Runtime> {
            author,
            peer_account_id: peer.clone(),
            peer_address: address,
            nonce: 4,
            network_id: 0,
            timepoint: timepoint(),
        };
        assert_err!(remove_compat.validate(), Error::UnknownPeerId);
        assert_eq!(
            OffchainRequest::outgoing(OutgoingRequest::RemovePeerCompat(remove_compat)).timepoint(),
            timepoint()
        );
        remove.peer_account_id = existing.clone();
        remove.compat_hash = Some(H256::repeat_byte(0xe8));
        crate::RequestStatuses::<Runtime>::insert(
            0,
            remove.compat_hash.unwrap(),
            RequestStatus::Pending,
        );
        assert_err!(remove.finalize(), Error::RequestIsNotReady);
        remove.compat_hash = None;
        bridge_multisig::Accounts::<Runtime>::remove(&bridge);
        assert_err!(
            remove.finalize(),
            bridge_multisig::Error::<Runtime>::UnknownMultisigAccount
        );
        assert_err!(
            EthBridge::force_add_peer(RuntimeOrigin::root(), peer, address, 0),
            bridge_multisig::Error::<Runtime>::UnknownMultisigAccount
        );
        assert_ok!(EthBridge::force_add_peer(
            RuntimeOrigin::root(),
            existing.clone(),
            address,
            0
        ));
        assert_eq!(crate::Peers::<Runtime>::get(0), peers);
        crate::PeerAddress::<Runtime>::insert(0, &existing, address);
        assert_err!(
            EthBridge::remove_peer(
                RuntimeOrigin::root(),
                existing.clone(),
                Some(EthAddress::zero()),
                0
            ),
            Error::UnknownPeerId
        );
        assert_ok!(EthBridge::remove_peer(
            RuntimeOrigin::root(),
            existing,
            Some(address),
            0
        ));
    });
}

#[test]
fn incoming_peer_updates_enforce_capacity_and_multisig_consistency_on_other_networks() {
    use super::mock::System;
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let peer: super::mock::AccountId = [0xe9; 32].into();
        let author = state.authority_account_id.clone();
        let bridge = state.networks[&0].config.bridge_account_id.clone();
        crate::BridgeAccount::<Runtime>::insert(1, &bridge);
        crate::PendingPeer::<Runtime>::insert(1, &peer);
        let mut main = IncomingChangePeers::<Runtime> {
            peer_account_id: Some(peer.clone()),
            peer_address: EthAddress::repeat_byte(0xe9),
            removed: false,
            author: author.clone(),
            tx_hash: H256::repeat_byte(0xea),
            at_height: 102,
            timepoint: timepoint(),
            network_id: 1,
        };
        let compat = IncomingChangePeersCompat::<Runtime> {
            peer_account_id: peer.clone(),
            peer_address: main.peer_address,
            added: true,
            contract: ChangePeersContract::VAL,
            author,
            tx_hash: H256::repeat_byte(0xeb),
            at_height: 102,
            timepoint: timepoint(),
            network_id: 1,
        };
        crate::Peers::<Runtime>::insert(
            1,
            (0..100u8)
                .map(|i| super::mock::AccountId::new([i; 32]))
                .collect::<std::collections::BTreeSet<_>>(),
        );
        assert_err!(main.finalize(), Error::CantAddMorePeers);
        assert_err!(compat.finalize(), Error::CantAddMorePeers);
        crate::Peers::<Runtime>::remove(1);
        let multisig = bridge_multisig::Accounts::<Runtime>::take(&bridge).unwrap();
        assert_err!(
            main.finalize(),
            bridge_multisig::Error::<Runtime>::UnknownMultisigAccount
        );
        assert_err!(
            compat.finalize(),
            bridge_multisig::Error::<Runtime>::UnknownMultisigAccount
        );
        assert_eq!(crate::PendingPeer::<Runtime>::get(1), Some(peer.clone()));
        assert!(crate::Peers::<Runtime>::get(1).is_empty());
        bridge_multisig::Accounts::<Runtime>::insert(&bridge, multisig);
        System::inc_providers(&peer);
        assert_ok!(System::inc_consumers(&peer));
        assert_eq!(compat.finalize(), Ok(compat.tx_hash));
        assert!(crate::Peers::<Runtime>::get(1).contains(&peer));
        assert!(bridge_multisig::Accounts::<Runtime>::get(&bridge)
            .unwrap()
            .signatories
            .contains(&peer));
        crate::PendingPeer::<Runtime>::insert(1, &peer);
        main.removed = true;
        main.peer_account_id = None;
        assert_eq!(main.finalize(), Ok(main.tx_hash));
        assert_eq!(
            System::consumers(&peer),
            1,
            "an unknown removed peer must not release another peer's reference"
        );
        assert_eq!(crate::PendingPeer::<Runtime>::get(1), None);
        let mut sync = EthPeersSync::default();
        sync.xor_ready();
        sync.bridge_ready();
        assert!(
            !sync.is_ready(),
            "legacy XOR cannot substitute for VAL acknowledgement"
        );
        sync.val_ready();
        assert!(sync.is_ready());
        sync.reset();
        assert!(!sync.is_ready());
    });
}

#[test]
fn transfer_cancellation_restores_funds_even_after_registration_is_removed() {
    use super::mock::EthBridge;
    use common::AssetInfoProvider;
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let bridge = state.networks[&0].config.bridge_account_id.clone();
        let author = state.authority_account_id.clone();
        let token_address = EthAddress::repeat_byte(0xec);
        let asset_id = EthBridge::register_sidechain_asset(
            token_address,
            18,
            common::AssetSymbol(b"CAN".to_vec()),
            common::AssetName(b"Cancel Token".to_vec()),
            0,
        )
        .unwrap();
        assert_ok!(assets::Pallet::<Runtime>::mint_to(
            &asset_id, &bridge, &author, 100
        ));
        let request = OutgoingTransfer::<Runtime> {
            from: author.clone(),
            to: EthAddress::repeat_byte(0xed),
            asset_id,
            amount: 100,
            nonce: 1,
            network_id: 0,
            timepoint: timepoint(),
        };
        for remove_registration in [false, true] {
            assert_ok!(request.prepare(OffchainRequest::outgoing(request.clone().into()).hash()));
            assert_eq!(
                assets::Pallet::<Runtime>::total_balance(&asset_id, &author).unwrap(),
                0
            );
            if remove_registration {
                crate::RegisteredAsset::<Runtime>::remove(0, asset_id);
            }
            assert_ok!(request.cancel());
            assert_eq!(
                assets::Pallet::<Runtime>::total_balance(&asset_id, &author).unwrap(),
                100
            );
            assert_eq!(
                assets::Pallet::<Runtime>::total_balance(&asset_id, &bridge).unwrap(),
                0
            );
        }
        let duplicate = OutgoingAddToken::<Runtime> {
            author: author.clone(),
            token_address,
            symbol: "CAN".into(),
            name: "Cancel Token".into(),
            decimals: 18,
            nonce: 2,
            network_id: 0,
            timepoint: timepoint(),
        };
        assert_err!(
            duplicate.validate(),
            Error::SidechainAssetIsAlreadyRegistered
        );
        crate::RegisteredSidechainToken::<Runtime>::insert(
            0,
            common::XOR,
            crate::LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS,
        );
        let legacy = OutgoingAddAsset::<Runtime> {
            author: author.clone(),
            asset_id: common::XOR,
            nonce: 3,
            network_id: 0,
            timepoint: timepoint(),
        };
        assert_err!(legacy.validate(), Error::DeprecatedLegacyXor);
        let load = LoadIncomingTransactionRequest::<Runtime>::new(
            author.clone(),
            H256::repeat_byte(0xee),
            timepoint(),
            IncomingTransactionRequestKind::Transfer,
            0,
        );
        assert_err!(
            IncomingRequest::try_from_contract_event(
                crate::contract::ContractEvent::Deposit(crate::contract::DepositEvent::new(
                    author,
                    ethereum_types::U256::MAX,
                    EthAddress::zero(),
                    common::PSWAP.into()
                )),
                load,
                103
            ),
            Error::InvalidAmount
        );
    });
}

#[test]
fn approval_admission_rejects_waiting_peer_removals_and_retired_transfers() {
    use super::mock::{EthBridge, RuntimeOrigin};
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let peer = state.networks[&0].ocw_keypairs[0].1.clone();
        let public = sp_core::ecdsa::Public::from_raw([1; 33]);
        let signature = crate::offchain::SignatureParams {
            r: [1; 32],
            s: [2; 32],
            v: 27,
        };
        let compat_hash = H256::repeat_byte(0xef);
        let outgoing = OffchainRequest::outgoing(OutgoingRequest::RemovePeer(OutgoingRemovePeer {
            author: state.authority_account_id.clone(),
            peer_account_id: peer.clone(),
            peer_address: EthAddress::repeat_byte(0xef),
            nonce: 1,
            network_id: 0,
            timepoint: timepoint(),
            compat_hash: Some(compat_hash),
        }));
        let hash = outgoing.hash();
        crate::Requests::<Runtime>::insert(0, hash, &outgoing);
        crate::RequestStatuses::<Runtime>::insert(0, hash, RequestStatus::Pending);
        crate::RequestStatuses::<Runtime>::insert(0, compat_hash, RequestStatus::Pending);
        assert_err!(
            EthBridge::validate_outgoing_approval(&public, &hash, &signature, &peer, 0),
            Error::RequestIsNotReady
        );
        crate::BridgeStatuses::<Runtime>::insert(0, BridgeStatus::Migrating);
        assert_err!(
            EthBridge::add_request(&outgoing),
            Error::ContractIsInMigrationStage
        );
        let transfer = OffchainRequest::outgoing(OutgoingRequest::Transfer(OutgoingTransfer {
            from: peer.clone(),
            to: EthAddress::zero(),
            asset_id: common::XOR,
            amount: 100,
            nonce: 2,
            network_id: 0,
            timepoint: timepoint(),
        }));
        let transfer_hash = transfer.hash();
        crate::Requests::<Runtime>::insert(0, transfer_hash, transfer);
        crate::RequestStatuses::<Runtime>::insert(0, transfer_hash, RequestStatus::Pending);
        crate::LegacyEthereumXorDecommissioned::<Runtime>::put(true);
        crate::LegacyEthereumXorDecommissionedAt::<Runtime>::kill();
        assert_err!(
            EthBridge::validate_outgoing_approval(&public, &transfer_hash, &signature, &peer, 0),
            Error::DeprecatedLegacyXor
        );
        assert_err!(
            EthBridge::approve_request(
                RuntimeOrigin::signed(peer.clone()),
                public,
                transfer_hash,
                signature,
                0
            ),
            Error::DeprecatedLegacyXor
        );
        assert!(crate::RequestApprovers::<Runtime>::get(0, transfer_hash).is_empty());
        assert_err!(
            EthBridge::cancel_pending_multisig(
                RuntimeOrigin::signed(peer),
                0,
                [0xef; 32],
                timepoint()
            ),
            bridge_multisig::Error::<Runtime>::NotFound
        );
    });
}

#[test]
fn recovery_finalization_failure_preserves_pending_work_and_unconsumed_resolution() {
    use super::mock::{EthBridge, System};
    use crate::recovery::{HashiResolution, HALT_KEY, HASHI_RESOLUTION_KEY, INBOUND_CONTEXT_KEY};
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let transfer = IncomingTransfer::<Runtime> {
            from: EthAddress::repeat_byte(0xf3),
            to: [0xf3; 32].into(),
            asset_id: common::VAL,
            asset_kind: AssetKind::SidechainOwned,
            amount: 100,
            author: state.authority_account_id,
            tx_hash: H256::repeat_byte(0xf3),
            at_height: 104,
            timepoint: timepoint(),
            network_id: 0,
            should_take_fee: false,
        };
        let request = IncomingRequest::Transfer(transfer.clone());
        let hash = OffchainRequest::incoming(request.clone()).hash();
        crate::RequestStatuses::<Runtime>::insert(0, hash, RequestStatus::Pending);
        crate::RequestsQueue::<Runtime>::insert(0, vec![hash]);
        let block = System::block_number() as u32;
        let resolution: HashiResolution<super::mock::AccountId> = HashiResolution {
            block,
            request_hash: sp_io::hashing::blake2_256(&transfer.encode()),
            beneficiary: [0xf4; 32].into(),
            excluded: false,
            consumed: false,
        };
        sp_io::storage::set(HALT_KEY, &[]);
        sp_io::storage::set(INBOUND_CONTEXT_KEY, &block.encode());
        sp_io::storage::set(HASHI_RESOLUTION_KEY, &resolution.encode());
        let before = sp_io::storage::root(sp_runtime::StateVersion::V1);
        assert_err!(
            EthBridge::finalize_incoming_request_inner(&request, hash, 0),
            Error::FailedToUnreserve
        );
        assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), before);
        assert_eq!(crate::RequestsQueue::<Runtime>::get(0), vec![hash]);
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(0, hash),
            Some(RequestStatus::Pending)
        );
        assert_eq!(
            sp_io::storage::get(HASHI_RESOLUTION_KEY).unwrap().to_vec(),
            resolution.encode()
        );
    });
}
