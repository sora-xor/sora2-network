use super::*;
use crate::requests::{
    encode_outgoing_request_eth_call, IncomingMetaRequestKind, IncomingTransfer, OutgoingTransfer,
};
use crate::tests::{mock::*, Error, ETH_NETWORK_ID};
use common::mock::alice;
use ethabi::Token;
use sp_runtime::traits::IdentifyAccount;

#[test]
fn peer_signatures_roundtrip_and_bind_payload_key_and_author() {
    let secret = secp256k1::SecretKey::parse(&[1; 32]).unwrap();
    let (signature, public) = EthBridge::sign_message(b"bridge approval", &secret);
    let public = ecdsa::Public::from_raw(public.serialize_compressed());
    let account = sp_runtime::MultiSigner::Ecdsa(public.clone()).into_account();
    assert!(EthBridge::verify_message(
        b"bridge approval",
        &signature,
        &public,
        &account
    ));
    assert!(!EthBridge::verify_message(
        b"different request",
        &signature,
        &public,
        &account
    ));
    assert!(!EthBridge::verify_message(
        b"bridge approval",
        &signature,
        &public,
        &alice()
    ));
    assert!(!EthBridge::verify_message(
        b"bridge approval",
        &signature,
        &ecdsa::Public::from_raw([0; 33]),
        &account
    ));
    let mut malformed = signature.clone();
    malformed.r = [255; 32];
    assert!(!EthBridge::verify_message(
        b"bridge approval",
        &malformed,
        &public,
        &account
    ));
    let rendered = format!("{signature}");
    assert!(rendered.starts_with("SignatureParams {\n\tr: "));
    assert!(rendered.contains("\n\ts: "));
    assert!(rendered.contains("\n\tv: "));
}

#[test]
fn corrupt_local_storage_is_cleared_and_metrics_saturate() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let mut storage = StorageValueRef::persistent(b"coverage-test");
        storage.set(&1u8);
        assert_eq!(
            get_storage_value_or_clear::<u64>(&mut storage, "test"),
            None
        );
        assert_eq!(storage.get::<u8>().unwrap(), None);
        storage.set(&42u64);
        assert_eq!(
            get_storage_value_or_clear::<u64>(&mut storage, "test"),
            Some(42)
        );
        set_offchain_metric_u64(b"coverage-test", u64::MAX);
        increment_offchain_metric_u64(b"coverage-test");
        assert_eq!(storage.get::<u64>().unwrap(), Some(u64::MAX));
    });
}

#[test]
fn control_events_are_parsed_only_for_the_expected_request_kind() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let address = crate::types::H160(EthBridge::bridge_contract_address(0).0);
        let peer = H160::repeat_byte(7);
        let mut log = Log {
            address,
            removed: Some(false),
            topics: vec![crate::types::H256(hex!(
                "a9fac23eb012e72fbd1f453498e7069c380385436763ee2c1c057b170d88d9f9"
            ))],
            data: ethabi::encode(&[
                Token::Address(crate::types::H160(peer.0)),
                Token::Bool(true),
            ])
            .into(),
            ..Default::default()
        };
        assert_eq!(
            EthBridge::parse_main_event(
                0,
                &[log.clone()],
                IncomingTransactionRequestKind::RemovePeer
            ),
            Ok(ContractEvent::ChangePeers(peer, true))
        );
        assert_eq!(
            EthBridge::parse_main_event(0, &[log.clone()], IncomingTransactionRequestKind::Migrate),
            Err(Error::UnknownEvent)
        );
        log.data = vec![1].into();
        assert_eq!(
            EthBridge::parse_main_event(0, &[log.clone()], IncomingTransactionRequestKind::AddPeer),
            Err(Error::EthAbiDecodingError)
        );
        log.topics = vec![crate::types::H256(hex!(
            "5389de9593f75e6515eefa796bd2d3324759f441f2c9b2dcda0efb25190378ff"
        ))];
        assert_eq!(
            EthBridge::parse_main_event(
                0,
                &[log.clone()],
                IncomingTransactionRequestKind::PrepareForMigration
            ),
            Ok(ContractEvent::PreparedForMigration)
        );
        log.topics = vec![crate::types::H256(hex!(
            "a2e7361c23d7820040603b83c0cd3f494d377bac69736377d75bb56c651a5098"
        ))];
        log.data = ethabi::encode(&[Token::Address(crate::types::H160(peer.0))]).into();
        assert_eq!(
            EthBridge::parse_main_event(0, &[log.clone()], IncomingTransactionRequestKind::Migrate),
            Ok(ContractEvent::Migrated(peer))
        );
        log.removed = None;
        assert_eq!(
            EthBridge::parse_main_event(0, &[log.clone()], IncomingTransactionRequestKind::Migrate),
            Err(Error::UnknownEvent)
        );
        log.removed = Some(false);
        log.topics.clear();
        assert_eq!(
            EthBridge::parse_main_event(0, &[log], IncomingTransactionRequestKind::Migrate),
            Err(Error::UnknownEvent)
        );
    });
}

#[test]
fn cancellation_parser_requires_failed_mined_transaction_and_request_owner() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let outgoing = OutgoingRequest::Transfer(OutgoingTransfer::<Runtime> {
            from: alice(),
            to: H160::repeat_byte(3),
            asset_id: common::XOR,
            amount: 10,
            nonce: 0,
            network_id: 0,
            timepoint: Default::default(),
        });
        let request = OffchainRequest::outgoing(outgoing.clone());
        let hash = request.hash();
        crate::Requests::<Runtime>::insert(0, hash, request);
        FUNCTIONS.get_or_init(functions);
        let input = encode_outgoing_request_eth_call::<Runtime>(
            *crate::contract::RECEIVE_BY_ETHEREUM_ASSET_ADDRESS_ID
                .get()
                .unwrap(),
            &outgoing,
            hash,
        )
        .unwrap();
        let tx_hash = H256::repeat_byte(4);
        let load = LoadIncomingMetaRequest::<Runtime> {
            author: alice(),
            hash: tx_hash,
            kind: IncomingMetaRequestKind::CancelOutgoingRequest,
            timepoint: Default::default(),
            network_id: ETH_NETWORK_ID,
        };
        let receipt = TransactionReceipt {
            transaction_hash: crate::types::H256(tx_hash.0),
            status: Some(0.into()),
            block_number: Some(55.into()),
            gas_used: Some(90.into()),
            ..Default::default()
        };
        let tx = Transaction {
            hash: crate::types::H256(tx_hash.0),
            to: Some(crate::types::H160(EthBridge::bridge_contract_address(0).0)),
            gas: 100.into(),
            input: input.clone().into(),
            ..Default::default()
        };
        let mut succeeded = receipt.clone();
        succeeded.status = Some(1.into());
        assert_eq!(
            EthBridge::parse_cancel_incoming_request(succeeded, load.clone(), hash),
            Err(Error::EthTransactionIsSucceeded)
        );
        let mut pending = receipt.clone();
        pending.block_number = None;
        assert_eq!(
            EthBridge::parse_cancel_incoming_request(pending, load.clone(), hash),
            Err(Error::EthTransactionIsPending)
        );
        let mut out_of_gas = receipt.clone();
        out_of_gas.gas_used = Some(100.into());
        push_global_json_rpc_response(tx.clone());
        assert_eq!(
            EthBridge::parse_cancel_incoming_request(out_of_gas, load.clone(), hash),
            Err(Error::TransactionMightHaveFailedDueToGasLimit)
        );
        for (input, expected) in [
            (vec![1], Error::InvalidFunctionInput),
            (
                input[..METHOD_ID_SIZE].to_vec(),
                Error::InvalidFunctionInput,
            ),
            (vec![255; 4], Error::UnknownMethodId),
        ] {
            push_global_json_rpc_response(Transaction {
                input: input.into(),
                ..tx.clone()
            });
            assert_eq!(
                EthBridge::parse_cancel_incoming_request(receipt.clone(), load.clone(), hash),
                Err(expected)
            );
        }
        push_global_json_rpc_response(tx.clone());
        let parsed =
            EthBridge::parse_cancel_incoming_request(receipt.clone(), load.clone(), hash).unwrap();
        let IncomingRequest::CancelOutgoingRequest(parsed) = parsed else {
            panic!("cancellation expected")
        };
        assert_eq!(parsed.outgoing_request_hash, hash);
        assert_eq!(parsed.tx_hash, tx_hash);
        assert_eq!(parsed.at_height, 55);
        assert_eq!(parsed.tx_input, input);
        let mut other = load.clone();
        other.author = common::mock::bob();
        push_global_json_rpc_response(tx.clone());
        assert_eq!(
            EthBridge::parse_cancel_incoming_request(receipt.clone(), other, hash),
            Err(Error::RequestIsNotOwnedByTheAuthor)
        );
        crate::Requests::<Runtime>::remove(0, hash);
        push_global_json_rpc_response(tx);
        assert_eq!(
            EthBridge::parse_cancel_incoming_request(receipt, load, hash),
            Err(Error::UnknownRequest)
        );
    });
}

#[test]
fn recovery_resolution_is_block_bound_exact_and_single_use() {
    use crate::recovery::*;
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        System::set_block_number(10);
        let request = IncomingTransfer::<Runtime> {
            from: H160::repeat_byte(1),
            to: alice(),
            asset_id: common::XOR,
            asset_kind: AssetKind::Thischain,
            amount: 10,
            author: alice(),
            tx_hash: H256::repeat_byte(2),
            at_height: 9,
            timepoint: Default::default(),
            network_id: 0,
            should_take_fee: false,
        };
        assert!(transfer_resolution::<Runtime>(&request).unwrap().is_none());
        sp_io::storage::set(HALT_KEY, &[]);
        assert_eq!(
            transfer_resolution::<Runtime>(&request).err(),
            Some(Error::RecoveryHalted.into())
        );
        sp_io::storage::set(INBOUND_CONTEXT_KEY, &10u32.encode());
        assert_eq!(
            transfer_resolution::<Runtime>(&request).err(),
            Some(Error::RecoveryHalted.into())
        );
        let resolution = HashiResolution {
            block: 10,
            request_hash: sp_io::hashing::blake2_256(&request.encode()),
            beneficiary: common::mock::bob(),
            excluded: false,
            consumed: false,
        };
        for bytes in [
            vec![0; 1025],
            vec![255],
            {
                let mut bytes = resolution.encode();
                bytes.push(0);
                bytes
            },
            {
                let mut stale = resolution.clone();
                stale.block = 9;
                stale.encode()
            },
        ] {
            sp_io::storage::set(HASHI_RESOLUTION_KEY, &bytes);
            assert_eq!(
                transfer_resolution::<Runtime>(&request).err(),
                Some(Error::RecoveryHalted.into())
            );
        }
        sp_io::storage::set(HASHI_RESOLUTION_KEY, &resolution.encode());
        let mut different_request = request.clone();
        different_request.amount += 1;
        assert_eq!(
            transfer_resolution::<Runtime>(&different_request).err(),
            Some(Error::RecoveryHalted.into())
        );
        let actual = transfer_resolution::<Runtime>(&request).unwrap().unwrap();
        assert_eq!(actual.beneficiary, common::mock::bob());
        consume_transfer(actual);
        assert_eq!(
            transfer_resolution::<Runtime>(&request).err(),
            Some(Error::RecoveryHalted.into())
        );
        assert_eq!(
            resolution_weight::<Runtime>(),
            frame_support::weights::Weight::zero()
        );
        sp_io::storage::clear(HALT_KEY);
        sp_io::storage::set(b":sora-nexus:recovery:policy:v1", &[]);
        assert!(resolution_weight::<Runtime>().ref_time() >= 50_000_000);
    });
}
