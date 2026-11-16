use super::*;
use codec::{Decode, Encode};
use frame_support::traits::ConstU32;
use sp_runtime::traits::{Hash, Keccak256};
use types::{AuxiliaryDigest, AuxiliaryDigestItem, MessageId};

#[test]
fn app_message_encoders_preserve_call_discriminants_and_payloads() {
    use substrate::{
        BridgeCall, FAAppCall, JettonAppCall, ParachainAppCall, SubstrateBridgeMessageEncode,
    };
    let parachain = ParachainAppCall::FinalizeAssetRegistration {
        asset_id: MainnetAssetId::from([7; 32]),
        asset_kind: types::AssetKind::Sidechain,
    };
    let fa = FAAppCall::FinalizeAssetRegistration {
        asset_id: MainnetAssetId::from([8; 32]),
        token: H160::repeat_byte(9),
    };
    let jetton = JettonAppCall::Transfer {
        token: ton::TonAddressWithPrefix::from(TonAddress::new(0, H256::repeat_byte(10))),
        sender: ton::TonAddressWithPrefix::from(TonAddress::new(-1, H256::repeat_byte(11))),
        recipient: MainnetAccountId::new([12; 32]),
        amount: 123u32.into(),
    };
    for (bytes, expected) in [
        (
            parachain.clone().prepare_message(),
            BridgeCall::ParachainApp(parachain),
        ),
        (fa.clone().prepare_message(), BridgeCall::FAApp(fa)),
        (
            jetton.clone().prepare_message(),
            BridgeCall::JettonApp(jetton),
        ),
    ] {
        let mut input = bytes.as_slice();
        assert_eq!(BridgeCall::decode(&mut input).unwrap(), expected);
        assert!(input.is_empty());
        assert_eq!(expected.prepare_message(), bytes);
    }
}

#[test]
fn disabled_bridge_implementations_reject_transfers_refunds_and_fee_queries() {
    use crate::traits::{
        AppRegistry, BalancePrecisionConverter, BridgeApp, BridgeAssetLockChecker,
        EVMBridgeWithdrawFee, EVMFeeHandler,
    };
    use sp_runtime::DispatchError;
    type NoApp = ();
    let network = SubNetworkId::Mainnet.into();
    let chain = H256::repeat_byte(1);
    assert!(!<NoApp as BridgeApp<u64, u64, u32, u128>>::is_asset_supported(network, 1));
    assert_eq!(
        <NoApp as BridgeApp<u64, u64, u32, u128>>::transfer(network, 1, 2, 3, 4),
        Err(DispatchError::Unavailable)
    );
    assert_eq!(
        <NoApp as BridgeApp<u64, u64, u32, u128>>::refund(network, chain, 2, 1, 4),
        Err(DispatchError::Unavailable)
    );
    assert!(<NoApp as BridgeApp<u64, u64, u32, u128>>::list_supported_assets(network).is_empty());
    assert!(<NoApp as BridgeApp<u64, u64, u32, u128>>::list_apps().is_empty());
    for weight in [
        <NoApp as BridgeApp<u64, u64, u32, u128>>::transfer_weight(),
        <NoApp as BridgeApp<u64, u64, u32, u128>>::refund_weight(),
        <NoApp as BridgeApp<u64, u64, u32, u128>>::is_asset_supported_weight(),
    ] {
        assert_eq!(weight, frame_support::weights::Weight::zero());
    }
    assert_eq!(
        <() as EVMBridgeWithdrawFee<u64, u32>>::withdraw_transfer_fee(&1, chain, 2),
        Err(DispatchError::Unavailable)
    );
    assert_eq!(
        <() as EVMFeeHandler<u32>>::get_latest_base_fee(chain),
        Err(DispatchError::Unavailable)
    );
    assert_eq!(
        <() as EVMFeeHandler<u32>>::get_network_fee_asset(chain),
        Err(DispatchError::Unavailable)
    );
    <() as EVMFeeHandler<u32>>::on_fee_paid(chain, H160::zero(), 1.into());
    <() as EVMFeeHandler<u32>>::update_base_fee(chain, 1.into(), 2);
    assert!(!<() as EVMFeeHandler<u32>>::can_update_base_fee(
        chain,
        1.into(),
        2
    ));
    assert_eq!(
        <() as AppRegistry<GenericNetworkId, u32>>::register_app(network, 1),
        Ok(())
    );
    assert_eq!(
        <() as AppRegistry<GenericNetworkId, u32>>::deregister_app(network, 1),
        Ok(())
    );
    assert_eq!(
        <() as BalancePrecisionConverter<u32, u128, u128>>::to_sidechain(&1, 18, u128::MAX),
        Some((u128::MAX, u128::MAX))
    );
    assert_eq!(
        <() as BalancePrecisionConverter<u32, u128, u128>>::from_sidechain(&1, 6, 123),
        Some((123, 123))
    );
    assert_eq!(
        <() as BridgeAssetLockChecker<u32, u128>>::before_asset_lock(
            network,
            types::AssetKind::Thischain,
            &1,
            &2
        ),
        Ok(())
    );
    assert_eq!(
        <() as BridgeAssetLockChecker<u32, u128>>::before_asset_unlock(
            network,
            types::AssetKind::Sidechain,
            &1,
            &2
        ),
        Ok(())
    );
}

#[test]
fn network_account_asset_and_balance_conversions_reject_other_domains() {
    let evm = GenericNetworkId::from(H256::repeat_byte(1));
    let sub = GenericNetworkId::from(SubNetworkId::Liberland);
    let ton = GenericNetworkId::from(TonNetworkId::Testnet);
    let legacy = GenericNetworkId::EVMLegacy(42);
    assert_eq!(evm.evm(), Some(H256::repeat_byte(1)));
    assert_eq!(sub.sub(), Some(SubNetworkId::Liberland));
    assert_eq!(legacy.evm_legacy(), Some(42));
    assert_eq!(ton.evm(), None);
    assert_eq!(evm.sub(), None);
    assert_eq!(sub.evm_legacy(), None);
    assert_eq!(GenericNetworkId::default(), SubNetworkId::Mainnet.into());
    let account = MainnetAccountId::new([7; 32]);
    for value in [
        GenericAccount::Sora(account.clone()),
        GenericAccount::Liberland(account.clone()),
    ] {
        assert_eq!(
            TryInto::<MainnetAccountId>::try_into(value),
            Ok(account.clone())
        );
    }
    assert!(TryInto::<MainnetAccountId>::try_into(GenericAccount::Root).is_err());
    assert_eq!(
        TryInto::<LiberlandAssetId>::try_into(GenericAssetId::Liberland(9.into())),
        Ok(LiberlandAssetId::Asset(9))
    );
    assert!(TryInto::<LiberlandAssetId>::try_into(GenericAssetId::EVM(H160::zero())).is_err());
    assert_eq!(
        TryInto::<MainnetBalance>::try_into(GenericBalance::Substrate(123)),
        Ok(123)
    );
    assert!(TryInto::<MainnetBalance>::try_into(GenericBalance::TON(12u32.into())).is_err());
    assert!(matches!(
        DecodeError::from(rlp::DecoderError::RlpIsTooShort),
        DecodeError::InvalidRLP(_)
    ));
    assert!(matches!(
        DecodeError::from(ethabi::Error::InvalidData),
        DecodeError::InvalidABI(_)
    ));
}

#[test]
fn ton_balances_and_addresses_preserve_network_wire_format() {
    use ton::{TonAddressWithPrefix, TonBalance};
    let value = 0x0102030405060708090a0b0c0d0e0f10u128;
    let balance = TonBalance::new(value);
    assert_eq!(balance.encode(), value.to_be_bytes());
    assert_eq!(u128::from(balance.clone()), value);
    assert_eq!(TonBalance::from(99u32).balance(), 99);
    assert_eq!(TonBalance::from(99u64).balance(), 99);
    assert_eq!(TonBalance::from(value).balance(), value);
    assert!(format!("{balance:?}").contains(&value.to_string()));
    let address = TonAddress::new(-1, H256::repeat_byte(2));
    assert_eq!(TonAddressWithPrefix::from(address).address(), Some(address));
    assert_eq!(
        TonAddressWithPrefix::new(4, address).address(),
        Some(address)
    );
    assert_eq!(
        TonAddressWithPrefix::new(0, TonAddress::empty()).address(),
        Some(TonAddress::empty())
    );
    assert_eq!(TonAddressWithPrefix::new(0, address).address(), None);
    assert_eq!(
        TonAddressWithPrefix::new(3, TonAddress::empty()).address(),
        None
    );
}

#[test]
fn message_ids_and_auxiliary_digests_keep_domain_and_nonce_separation() {
    let from = SubNetworkId::Mainnet.into();
    let to = SubNetworkId::Liberland.into();
    let basic = MessageId::basic(from, to, 3);
    let batch = MessageId::batched(from, to, 0, 3);
    assert_eq!(basic.hash(), Keccak256::hash_of(&basic));
    assert_ne!(basic.hash(), batch.hash());
    assert_ne!(basic.hash(), MessageId::basic(to, from, 3).hash());
    let item = AuxiliaryDigestItem::Commitment(to, batch.hash());
    let encoded: sp_runtime::DigestItem = item.into();
    assert_eq!(AuxiliaryDigestItem::try_from(encoded.clone()), Ok(item));
    assert!(AuxiliaryDigestItem::try_from(sp_runtime::DigestItem::Other(vec![255])).is_err());
    assert!(
        AuxiliaryDigestItem::try_from(sp_runtime::DigestItem::PreRuntime(*b"test", vec![]))
            .is_err()
    );
    let digest: AuxiliaryDigest = sp_runtime::Digest {
        logs: vec![encoded, sp_runtime::DigestItem::Other(vec![])],
    }
    .into();
    assert_eq!(digest.logs, vec![item]);
    assert_eq!(
        utils::make_offchain_key(from, 7),
        (utils::CHANNEL_INDEXING_PREFIX, from, 7u64).encode()
    );
    for (peers, expected) in [(0, 0), (1, 1), (2, 2), (3, 3), (4, 3), (7, 5), (10, 7)] {
        assert_eq!(utils::threshold(peers), expected);
    }
}

#[test]
fn recovery_markers_remain_distinct_and_fail_closed_for_malformed_values() {
    use recovery::*;
    sp_io::TestExternalities::default().execute_with(|| {
        let network = SubNetworkId::Mainnet.into();
        let hash = H256::repeat_byte(5);
        assert!(!hashi_quarantined(0u32, hash));
        assert!(!message_quarantined(network, hash));
        assert!(!batch_quarantined(0, network, 3));
        assert!(!commitment_quarantined(network, hash));
        let keys = [
            hashi_key(0u32, hash),
            historical_hashi_key(0u32, hash),
            message_key(network, hash),
            historical_message_key(network, hash),
            batch_key(0, network, 3),
            batch_key(1, network, 3),
            commitment_key(network, hash),
            historical_commitment_key(network, hash),
        ];
        let unique: std::collections::BTreeSet<_> = keys.iter().collect();
        assert_eq!(unique.len(), keys.len());
        for key in &keys {
            sp_io::storage::set(key, &[255]);
        }
        assert!(hashi_quarantined(0u32, hash));
        assert!(message_quarantined(network, hash));
        assert!(batch_quarantined(0, network, 3));
        assert!(commitment_quarantined(network, hash));
        for key in [
            hashi_key(0u32, hash),
            message_key(network, hash),
            commitment_key(network, hash),
        ] {
            sp_io::storage::clear(&key);
        }
        assert!(hashi_quarantined(0u32, hash));
        assert!(message_quarantined(network, hash));
        assert!(commitment_quarantined(network, hash));
        assert!(!hashi_quarantined(1u32, hash));
        assert!(!message_quarantined(SubNetworkId::Liberland.into(), hash));
        assert!(!batch_quarantined(0, network, 4));
    });
    let valid = HistoricalMessageTombstone {
        tag: HistoricalMessageTombstone::TAG,
        source_record_hash: [1; 32],
        original_owner: [2; 32],
        effective_owner: [3; 32],
        evidence: [4; 32],
        refund_authorized: false,
    };
    assert!(valid.is_valid());
    for bad in [
        HistoricalMessageTombstone {
            tag: [0; 8],
            ..valid.clone()
        },
        HistoricalMessageTombstone {
            source_record_hash: [0; 32],
            ..valid.clone()
        },
        HistoricalMessageTombstone {
            evidence: [0; 32],
            ..valid.clone()
        },
        HistoricalMessageTombstone {
            refund_authorized: true,
            ..valid.clone()
        },
    ] {
        assert!(!bad.is_valid());
    }
}

#[test]
fn generic_commitments_preserve_hash_and_nonce_of_every_protocol_variant() {
    type N = ConstU32<8>;
    let evm_inbound = evm::InboundCommitment::<N> {
        channel: H160::repeat_byte(1),
        source: H160::repeat_byte(2),
        nonce: 3,
        block_number: 4,
        payload: vec![5].try_into().unwrap(),
    };
    let status = evm::StatusReport::<N> {
        channel: H160::repeat_byte(1),
        block_number: 4,
        relayer: H160::repeat_byte(2),
        nonce: 6,
        results: vec![true, false].try_into().unwrap(),
        gas_spent: 7.into(),
        base_fee: 8.into(),
    };
    let fee = evm::BaseFeeUpdate {
        new_base_fee: 9.into(),
        evm_block_number: 10,
    };
    let outbound = evm::OutboundCommitment::<N, N> {
        nonce: 11,
        total_max_gas: 12.into(),
        messages: vec![evm::Message {
            target: H160::repeat_byte(2),
            max_gas: 12.into(),
            payload: vec![3].try_into().unwrap(),
        }]
        .try_into()
        .unwrap(),
    };
    let expected_outbound = H256::from(sp_io::hashing::keccak_256(&ethabi::encode(&[
        ethabi::Token::Tuple(vec![
            ethabi::Token::Uint(11.into()),
            ethabi::Token::Uint(12.into()),
            ethabi::Token::Array(vec![ethabi::Token::Tuple(vec![
                ethabi::Token::Address(ethabi::ethereum_types::H160::repeat_byte(2)),
                ethabi::Token::Uint(12.into()),
                ethabi::Token::Bytes(vec![3]),
            ])]),
        ]),
    ])));
    assert_eq!(outbound.hash(), expected_outbound);
    for (value, hash, nonce) in [
        (
            evm::Commitment::<N, N>::Inbound(evm_inbound.clone()),
            Keccak256::hash_of(&("evm-inbound", evm_inbound)),
            3,
        ),
        (
            evm::Commitment::StatusReport(status.clone()),
            Keccak256::hash_of(&("evm-status-report", status)),
            6,
        ),
        (
            evm::Commitment::BaseFeeUpdate(fee.clone()),
            Keccak256::hash_of(&("base-fee-update", fee)),
            0,
        ),
        (evm::Commitment::Outbound(outbound), expected_outbound, 11),
    ] {
        let commitment = GenericCommitment::EVM(value);
        assert_eq!(commitment.hash(), hash);
        assert_eq!(commitment.nonce(), nonce);
        let roundtrip = GenericCommitment::<N, N>::decode(&mut &commitment.encode()[..]).unwrap();
        assert_eq!(roundtrip.hash(), hash);
    }
    let sub = substrate::Commitment::<N, N> {
        messages: Default::default(),
        nonce: 13,
    };
    let hash = Keccak256::hash_of(&sub);
    let sub = GenericCommitment::Sub(sub);
    assert_eq!(sub.hash(), hash);
    assert_eq!(sub.nonce(), 13);
    let ton = ton::InboundCommitment::<N> {
        channel: TonAddress::empty(),
        source: TonAddress::new(0, H256::repeat_byte(3)),
        nonce: 14,
        transaction_id: Default::default(),
        payload: vec![15].try_into().unwrap(),
    };
    let expected = Keccak256::hash_of(&("ton-inbound", ton.clone()));
    let ton = GenericCommitment::<N, N>::TON(ton::Commitment::Inbound(ton));
    assert_eq!(ton.hash(), expected);
    assert_eq!(ton.nonce(), 14);
}
