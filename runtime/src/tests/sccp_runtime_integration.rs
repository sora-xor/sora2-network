// This file is part of the SORA network and Polkaswap app.
//
// Copyright (c) 2026, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

use bridge_types::ton::{TonAddress, TonNetworkId};
use bridge_types::traits::{
    BridgeAssetLockChecker, BridgeAssetLocker, BridgeAssetRegistry, EVMBridgeWithdrawFee,
};
use bridge_types::{GenericAccount, GenericNetworkId, H160};
use codec::Encode;
use common::{AssetName, AssetSymbol, DEFAULT_BALANCE_PRECISION};
use frame_support::{assert_noop, assert_ok};
use framenode_chain_spec::ext;
use sccp::{
    BurnPayloadV1, InboundFinalityMode, LegacyBridgeAssetChecker, SolanaVoteAuthorityV1,
    SCCP_CORE_REMOTE_DOMAINS, SCCP_DOMAIN_BSC, SCCP_DOMAIN_ETH, SCCP_DOMAIN_SOL, SCCP_DOMAIN_SORA,
    SCCP_DOMAIN_SORA_KUSAMA, SCCP_DOMAIN_SORA_POLKADOT, SCCP_DOMAIN_TON, SCCP_DOMAIN_TRON,
    SCCP_MAX_BSC_HEADER_RLP_BYTES, SCCP_MAX_TRON_RAW_DATA_BYTES, SCCP_MSG_PREFIX_BURN_V1,
};
use sp_core::{ecdsa, keccak_256, Pair, H256};
use sp_runtime::DispatchError;
use traits::MultiCurrency;

use crate::{Assets, Currencies, LegacyBridgeChecker, Runtime, RuntimeOrigin, Sccp, System};

fn sccp_test_remote_token_bytes(domain: u32) -> Vec<u8> {
    match domain {
        SCCP_DOMAIN_ETH => vec![0x11u8; 20],
        SCCP_DOMAIN_BSC => vec![0x12u8; 20],
        SCCP_DOMAIN_TRON => vec![0x13u8; 20],
        SCCP_DOMAIN_SOL => vec![0x14u8; 32],
        SCCP_DOMAIN_TON => vec![0x15u8; 32],
        SCCP_DOMAIN_SORA_KUSAMA => vec![0x16u8; 32],
        SCCP_DOMAIN_SORA_POLKADOT => vec![0x17u8; 32],
        _ => unreachable!("core domain expected"),
    }
}

fn sccp_test_domain_endpoint_bytes(domain: u32) -> Vec<u8> {
    match domain {
        SCCP_DOMAIN_ETH => vec![0x21u8; 20],
        SCCP_DOMAIN_BSC => vec![0x22u8; 20],
        SCCP_DOMAIN_TRON => vec![0x23u8; 20],
        SCCP_DOMAIN_SOL => vec![0x24u8; 32],
        SCCP_DOMAIN_TON => vec![0x25u8; 32],
        SCCP_DOMAIN_SORA_KUSAMA => vec![0x26u8; 32],
        SCCP_DOMAIN_SORA_POLKADOT => vec![0x27u8; 32],
        _ => unreachable!("core domain expected"),
    }
}

fn sccp_test_burn_payload(source_domain: u32, dest_domain: u32) -> BurnPayloadV1 {
    BurnPayloadV1 {
        version: 1,
        source_domain,
        dest_domain,
        nonce: 1,
        sora_asset_id: [0x42u8; 32],
        amount: 1u128,
        recipient: [0x11u8; 32],
    }
}

fn sccp_test_canonical_evm_recipient() -> [u8; 32] {
    let mut recipient = [0u8; 32];
    recipient[12..].copy_from_slice(&[0xabu8; 20]);
    recipient
}

fn sccp_test_message_id(payload: &BurnPayloadV1) -> H256 {
    let mut preimage = SCCP_MSG_PREFIX_BURN_V1.to_vec();
    preimage.extend(payload.encode());
    H256::from_slice(&keccak_256(&preimage))
}

fn sccp_enable_solana_finality() {
    assert_ok!(Sccp::set_solana_vote_authorities(
        RuntimeOrigin::root(),
        vec![SolanaVoteAuthorityV1 {
            authority_pubkey: [0x31; 32],
            stake: 1,
        }],
    ));
}

fn sccp_test_eth_address_from_pair(pair: &ecdsa::Pair) -> H160 {
    let msg = H256([9u8; 32]);
    let sig = pair.sign_prehashed(&msg.0);
    let public_key = match sp_io::crypto::secp256k1_ecdsa_recover(&sig.0, &msg.0) {
        Ok(pk) => pk,
        Err(_) => panic!("valid test signature"),
    };
    H160::from_slice(&keccak_256(&public_key)[12..])
}

fn sccp_test_pb_varint(mut value: u64) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            break;
        }
    }
    out
}

fn sccp_test_tron_raw_data(
    parent_hash: H256,
    number: u64,
    signer: H160,
    state_root: H256,
) -> Vec<u8> {
    let mut witness_address = [0u8; 21];
    witness_address[0] = 0x41;
    witness_address[1..].copy_from_slice(signer.as_bytes());

    // parentHash (field 3, bytes) => 0x1a
    // number (field 7, varint) => 0x38
    // witness_address (field 9, bytes) => 0x4a
    // accountStateRoot (field 11, bytes) => 0x5a
    let mut raw_data = Vec::new();
    raw_data.push(0x1a);
    raw_data.extend_from_slice(&sccp_test_pb_varint(32));
    raw_data.extend_from_slice(parent_hash.as_bytes());
    raw_data.push(0x38);
    raw_data.extend_from_slice(&sccp_test_pb_varint(number));
    raw_data.push(0x4a);
    raw_data.extend_from_slice(&sccp_test_pb_varint(21));
    raw_data.extend_from_slice(&witness_address);
    raw_data.push(0x5a);
    raw_data.extend_from_slice(&sccp_test_pb_varint(32));
    raw_data.extend_from_slice(state_root.as_bytes());
    raw_data
}

fn sccp_test_tron_witness_signature(raw_data: &[u8], signer: &ecdsa::Pair) -> Vec<u8> {
    let raw_hash = sp_io::hashing::sha2_256(raw_data);
    signer.sign_prehashed(&raw_hash).0.to_vec()
}

fn sccp_test_tron_block_id(number: u64, raw_data: &[u8]) -> H256 {
    let mut raw_hash = sp_io::hashing::sha2_256(raw_data);
    raw_hash[..8].copy_from_slice(&number.to_be_bytes());
    H256(raw_hash)
}

fn sccp_test_bsc_header_rlp(
    parent_hash: H256,
    beneficiary: H160,
    state_root: H256,
    number: u64,
    difficulty: u64,
    extra_data: &[u8],
) -> Vec<u8> {
    use rlp::RlpStream;

    let ommers_hash = [0x11u8; 32];
    let tx_root = [0x33u8; 32];
    let receipts_root = [0x44u8; 32];
    let logs_bloom = [0u8; 256];
    let gas_limit = 1_000_000u64;
    let gas_used = 0u64;
    let timestamp = number;
    let mix_hash = [0u8; 32];
    let nonce = [0u8; 8];

    let mut s = RlpStream::new_list(15);
    s.append(&parent_hash.as_bytes());
    s.append(&ommers_hash.as_ref());
    s.append(&beneficiary.as_bytes());
    s.append(&state_root.as_bytes());
    s.append(&tx_root.as_ref());
    s.append(&receipts_root.as_ref());
    s.append(&logs_bloom.as_ref());
    s.append(&difficulty);
    s.append(&number);
    s.append(&gas_limit);
    s.append(&gas_used);
    s.append(&timestamp);
    s.append(&extra_data);
    s.append(&mix_hash.as_ref());
    s.append(&nonce.as_ref());
    s.out().to_vec()
}

fn sccp_test_build_signed_bsc_header(
    parent_hash: H256,
    state_root: H256,
    number: u64,
    difficulty: u64,
    signer: &ecdsa::Pair,
) -> (Vec<u8>, H256) {
    use rlp::RlpStream;

    let vanity = [0u8; 32];
    let chain_id = 56u64;
    let beneficiary = sccp_test_eth_address_from_pair(signer);
    let ommers_hash = [0x11u8; 32];
    let tx_root = [0x33u8; 32];
    let receipts_root = [0x44u8; 32];
    let logs_bloom = [0u8; 256];
    let gas_limit = 1_000_000u64;
    let gas_used = 0u64;
    let timestamp = number;
    let mix_hash = [0u8; 32];
    let nonce = [0u8; 8];

    // Mirror BSC/Parlia seal-hash layout for legacy 15-field headers.
    let mut seal = RlpStream::new_list(16);
    seal.append(&chain_id);
    seal.append(&parent_hash.as_bytes());
    seal.append(&ommers_hash.as_ref());
    seal.append(&beneficiary.as_bytes());
    seal.append(&state_root.as_bytes());
    seal.append(&tx_root.as_ref());
    seal.append(&receipts_root.as_ref());
    seal.append(&logs_bloom.as_ref());
    seal.append(&difficulty);
    seal.append(&number);
    seal.append(&gas_limit);
    seal.append(&gas_used);
    seal.append(&timestamp);
    seal.append(&vanity.as_slice()); // extra without signature
    seal.append(&mix_hash.as_ref());
    seal.append(&nonce.as_ref());
    let seal_hash = H256::from_slice(&keccak_256(&seal.out()));
    let sig = signer.sign_prehashed(&seal_hash.0);

    let mut extra = vanity.to_vec();
    extra.extend_from_slice(&sig.0[..64]);
    extra.push(sig.0[64].saturating_add(27));

    let full_rlp = sccp_test_bsc_header_rlp(
        parent_hash,
        beneficiary,
        state_root,
        number,
        difficulty,
        &extra,
    );
    let full_hash = H256::from_slice(&keccak_256(&full_rlp));
    (full_rlp, full_hash)
}

#[test]
fn sccp_default_required_domains_include_all_core_domains_in_runtime() {
    ext().execute_with(|| {
        assert_eq!(
            Sccp::required_domains().into_inner(),
            SCCP_CORE_REMOTE_DOMAINS.to_vec()
        );
    });
}

#[test]
fn sccp_set_required_domains_stores_canonical_order_in_runtime() {
    ext().execute_with(|| {
        assert_ok!(Sccp::set_required_domains(
            RuntimeOrigin::root(),
            vec![
                SCCP_DOMAIN_TRON,
                SCCP_DOMAIN_SORA_POLKADOT,
                SCCP_DOMAIN_ETH,
                SCCP_DOMAIN_TON,
                SCCP_DOMAIN_SORA_KUSAMA,
                SCCP_DOMAIN_BSC,
                SCCP_DOMAIN_SOL,
            ],
        ));

        assert_eq!(
            Sccp::required_domains().into_inner(),
            vec![
                SCCP_DOMAIN_ETH,
                SCCP_DOMAIN_BSC,
                SCCP_DOMAIN_SOL,
                SCCP_DOMAIN_TON,
                SCCP_DOMAIN_TRON,
                SCCP_DOMAIN_SORA_KUSAMA,
                SCCP_DOMAIN_SORA_POLKADOT,
            ]
        );
    });
}

#[test]
fn sccp_set_required_domains_event_hash_uses_canonical_sorted_order_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        let input = vec![
            SCCP_DOMAIN_TRON,
            SCCP_DOMAIN_SORA_POLKADOT,
            SCCP_DOMAIN_ETH,
            SCCP_DOMAIN_TON,
            SCCP_DOMAIN_SORA_KUSAMA,
            SCCP_DOMAIN_BSC,
            SCCP_DOMAIN_SOL,
        ];
        assert_ok!(Sccp::set_required_domains(
            RuntimeOrigin::root(),
            input.clone(),
        ));

        let mut sorted = input;
        sorted.sort();
        let expected = H256::from_slice(&keccak_256(&sorted.encode()));

        let event = frame_system::Pallet::<Runtime>::events()
            .into_iter()
            .rev()
            .find_map(|record| match record.event {
                crate::RuntimeEvent::Sccp(sccp::Event::RequiredDomainsSet { domains_hash }) => {
                    Some(domains_hash)
                }
                _ => None,
            });
        assert_eq!(event, Some(expected));
    });
}

#[test]
fn sccp_set_required_domains_rejects_sora_domain_in_runtime() {
    ext().execute_with(|| {
        let initial = Sccp::required_domains().into_inner();
        assert_noop!(
            Sccp::set_required_domains(
                RuntimeOrigin::root(),
                vec![
                    SCCP_DOMAIN_SORA,
                    SCCP_DOMAIN_ETH,
                    SCCP_DOMAIN_BSC,
                    SCCP_DOMAIN_SOL,
                    SCCP_DOMAIN_TON,
                    SCCP_DOMAIN_TRON,
                ],
            ),
            sccp::Error::<Runtime>::DomainUnsupported
        );
        assert_eq!(Sccp::required_domains().into_inner(), initial);
    });
}

#[test]
fn sccp_set_required_domains_rejects_unknown_domain_in_runtime() {
    ext().execute_with(|| {
        let initial = Sccp::required_domains().into_inner();
        assert_noop!(
            Sccp::set_required_domains(
                RuntimeOrigin::root(),
                vec![
                    SCCP_DOMAIN_ETH,
                    SCCP_DOMAIN_BSC,
                    SCCP_DOMAIN_SOL,
                    SCCP_DOMAIN_TON,
                    SCCP_DOMAIN_TRON,
                    777,
                ],
            ),
            sccp::Error::<Runtime>::DomainUnsupported
        );
        assert_eq!(Sccp::required_domains().into_inner(), initial);
    });
}

#[test]
fn sccp_set_required_domains_rejects_duplicates_in_runtime() {
    ext().execute_with(|| {
        let initial = Sccp::required_domains().into_inner();
        assert_noop!(
            Sccp::set_required_domains(
                RuntimeOrigin::root(),
                vec![SCCP_DOMAIN_ETH, SCCP_DOMAIN_ETH],
            ),
            sccp::Error::<Runtime>::RequiredDomainsInvalid
        );
        assert_eq!(Sccp::required_domains().into_inner(), initial);
    });
}

#[test]
fn sccp_set_required_domains_rejects_missing_core_domains_in_runtime() {
    ext().execute_with(|| {
        let initial = Sccp::required_domains().into_inner();
        assert_noop!(
            Sccp::set_required_domains(RuntimeOrigin::root(), vec![SCCP_DOMAIN_ETH]),
            sccp::Error::<Runtime>::RequiredDomainsInvalid
        );
        assert_eq!(Sccp::required_domains().into_inner(), initial);
    });
}

#[test]
fn sccp_set_required_domains_rejects_non_manager_origin_in_runtime() {
    ext().execute_with(|| {
        let initial = Sccp::required_domains().into_inner();
        assert_noop!(
            Sccp::set_required_domains(
                RuntimeOrigin::signed(common::mock::alice()),
                vec![
                    SCCP_DOMAIN_ETH,
                    SCCP_DOMAIN_BSC,
                    SCCP_DOMAIN_SOL,
                    SCCP_DOMAIN_TON,
                    SCCP_DOMAIN_TRON,
                ],
            ),
            DispatchError::BadOrigin
        );
        assert_eq!(Sccp::required_domains().into_inner(), initial);
    });
}

#[test]
fn sccp_set_required_domains_failure_does_not_emit_event_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        let initial = Sccp::required_domains().into_inner();
        let events_before = frame_system::Pallet::<Runtime>::events().len();
        assert_noop!(
            Sccp::set_required_domains(
                RuntimeOrigin::signed(common::mock::alice()),
                vec![
                    SCCP_DOMAIN_ETH,
                    SCCP_DOMAIN_BSC,
                    SCCP_DOMAIN_SOL,
                    SCCP_DOMAIN_TON,
                    SCCP_DOMAIN_TRON,
                ],
            ),
            DispatchError::BadOrigin
        );
        assert_eq!(Sccp::required_domains().into_inner(), initial);
        assert_eq!(
            frame_system::Pallet::<Runtime>::events().len(),
            events_before
        );
    });
}

#[test]
fn sccp_set_inbound_grace_period_updates_state_and_emits_event_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        let blocks: u32 = 42;
        assert_ok!(Sccp::set_inbound_grace_period(
            RuntimeOrigin::root(),
            blocks,
        ));
        assert_eq!(Sccp::inbound_grace_period(), blocks);

        let event = frame_system::Pallet::<Runtime>::events()
            .into_iter()
            .rev()
            .find_map(|record| match record.event {
                crate::RuntimeEvent::Sccp(sccp::Event::InboundGracePeriodSet { blocks }) => {
                    Some(blocks)
                }
                _ => None,
            });
        assert_eq!(event, Some(blocks));
    });
}

#[test]
fn sccp_set_inbound_grace_period_rejects_non_manager_origin_in_runtime() {
    ext().execute_with(|| {
        let initial = Sccp::inbound_grace_period();
        assert_noop!(
            Sccp::set_inbound_grace_period(
                RuntimeOrigin::signed(common::mock::alice()),
                7u32.into()
            ),
            DispatchError::BadOrigin
        );
        assert_eq!(Sccp::inbound_grace_period(), initial);
    });
}

#[test]
fn sccp_set_inbound_grace_period_failure_does_not_emit_event_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        let initial = Sccp::inbound_grace_period();
        let events_before = frame_system::Pallet::<Runtime>::events().len();
        assert_noop!(
            Sccp::set_inbound_grace_period(
                RuntimeOrigin::signed(common::mock::alice()),
                9u32.into()
            ),
            DispatchError::BadOrigin
        );
        assert_eq!(Sccp::inbound_grace_period(), initial);
        assert_eq!(
            frame_system::Pallet::<Runtime>::events().len(),
            events_before
        );
    });
}

#[test]
fn sccp_set_inbound_domain_paused_updates_state_and_emits_event_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(Sccp::set_inbound_domain_paused(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            true,
        ));
        assert!(Sccp::inbound_domain_paused(SCCP_DOMAIN_ETH));

        let event = frame_system::Pallet::<Runtime>::events()
            .into_iter()
            .rev()
            .find_map(|record| match record.event {
                crate::RuntimeEvent::Sccp(sccp::Event::InboundDomainPausedSet {
                    domain_id,
                    paused,
                }) => Some((domain_id, paused)),
                _ => None,
            });
        assert_eq!(event, Some((SCCP_DOMAIN_ETH, true)));
    });
}

#[test]
fn sccp_set_inbound_domain_paused_rejects_sora_domain_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::set_inbound_domain_paused(RuntimeOrigin::root(), SCCP_DOMAIN_SORA, true),
            sccp::Error::<Runtime>::DomainUnsupported
        );
    });
}

#[test]
fn sccp_set_inbound_domain_paused_rejects_unknown_domain_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::set_inbound_domain_paused(RuntimeOrigin::root(), 777, true),
            sccp::Error::<Runtime>::DomainUnsupported
        );
    });
}

#[test]
fn sccp_set_inbound_domain_paused_rejects_non_manager_origin_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::set_inbound_domain_paused(
                RuntimeOrigin::signed(common::mock::alice()),
                SCCP_DOMAIN_ETH,
                true,
            ),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn sccp_set_inbound_domain_paused_failure_does_not_emit_event_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        let events_before = frame_system::Pallet::<Runtime>::events().len();
        assert_noop!(
            Sccp::set_inbound_domain_paused(RuntimeOrigin::root(), SCCP_DOMAIN_SORA, true),
            sccp::Error::<Runtime>::DomainUnsupported
        );
        assert_eq!(
            frame_system::Pallet::<Runtime>::events().len(),
            events_before
        );
    });
}

#[test]
fn sccp_set_inbound_domain_paused_toggle_roundtrip_in_runtime() {
    ext().execute_with(|| {
        assert_ok!(Sccp::set_inbound_domain_paused(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            true,
        ));
        assert!(Sccp::inbound_domain_paused(SCCP_DOMAIN_ETH));
        assert_ok!(Sccp::set_inbound_domain_paused(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            false,
        ));
        assert!(!Sccp::inbound_domain_paused(SCCP_DOMAIN_ETH));
    });
}

#[test]
fn sccp_set_outbound_domain_paused_updates_state_and_emits_event_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(Sccp::set_outbound_domain_paused(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_BSC,
            true,
        ));
        assert!(Sccp::outbound_domain_paused(SCCP_DOMAIN_BSC));

        let event = frame_system::Pallet::<Runtime>::events()
            .into_iter()
            .rev()
            .find_map(|record| match record.event {
                crate::RuntimeEvent::Sccp(sccp::Event::OutboundDomainPausedSet {
                    domain_id,
                    paused,
                }) => Some((domain_id, paused)),
                _ => None,
            });
        assert_eq!(event, Some((SCCP_DOMAIN_BSC, true)));
    });
}

#[test]
fn sccp_set_outbound_domain_paused_rejects_unknown_domain_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::set_outbound_domain_paused(RuntimeOrigin::root(), 777, true),
            sccp::Error::<Runtime>::DomainUnsupported
        );
    });
}

#[test]
fn sccp_set_outbound_domain_paused_rejects_sora_domain_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::set_outbound_domain_paused(RuntimeOrigin::root(), SCCP_DOMAIN_SORA, true),
            sccp::Error::<Runtime>::DomainUnsupported
        );
    });
}

#[test]
fn sccp_set_outbound_domain_paused_rejects_non_manager_origin_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::set_outbound_domain_paused(
                RuntimeOrigin::signed(common::mock::alice()),
                SCCP_DOMAIN_BSC,
                true,
            ),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn sccp_set_outbound_domain_paused_failure_does_not_emit_event_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        let events_before = frame_system::Pallet::<Runtime>::events().len();
        assert_noop!(
            Sccp::set_outbound_domain_paused(RuntimeOrigin::root(), 777, true),
            sccp::Error::<Runtime>::DomainUnsupported
        );
        assert_eq!(
            frame_system::Pallet::<Runtime>::events().len(),
            events_before
        );
    });
}

#[test]
fn sccp_set_outbound_domain_paused_toggle_roundtrip_in_runtime() {
    ext().execute_with(|| {
        assert_ok!(Sccp::set_outbound_domain_paused(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_BSC,
            true,
        ));
        assert!(Sccp::outbound_domain_paused(SCCP_DOMAIN_BSC));
        assert_ok!(Sccp::set_outbound_domain_paused(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_BSC,
            false,
        ));
        assert!(!Sccp::outbound_domain_paused(SCCP_DOMAIN_BSC));
    });
}

#[test]
fn sccp_set_inbound_finality_mode_updates_override_and_emits_event_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(Sccp::set_inbound_finality_mode(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_SOL,
            InboundFinalityMode::SolanaLightClient,
        ));
        assert_eq!(
            Sccp::inbound_finality_mode_override(SCCP_DOMAIN_SOL),
            Some(InboundFinalityMode::SolanaLightClient)
        );

        let event = frame_system::Pallet::<Runtime>::events()
            .into_iter()
            .rev()
            .find_map(|record| match record.event {
                crate::RuntimeEvent::Sccp(sccp::Event::InboundFinalityModeSet {
                    domain_id,
                    mode,
                }) => Some((domain_id, mode)),
                _ => None,
            });
        assert_eq!(
            event,
            Some((SCCP_DOMAIN_SOL, InboundFinalityMode::SolanaLightClient))
        );
    });
}

#[test]
fn sccp_set_inbound_finality_mode_rejects_unsupported_mode_for_domain_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::set_inbound_finality_mode(
                RuntimeOrigin::root(),
                SCCP_DOMAIN_ETH,
                InboundFinalityMode::SolanaLightClient,
            ),
            sccp::Error::<Runtime>::InboundFinalityModeUnsupported
        );
    });
}

#[test]
fn sccp_set_inbound_finality_mode_rejects_sora_domain_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::set_inbound_finality_mode(
                RuntimeOrigin::root(),
                SCCP_DOMAIN_SORA,
                InboundFinalityMode::EthBeaconLightClient,
            ),
            sccp::Error::<Runtime>::DomainUnsupported
        );
    });
}

#[test]
fn sccp_set_inbound_finality_mode_rejects_unknown_domain_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::set_inbound_finality_mode(
                RuntimeOrigin::root(),
                777,
                InboundFinalityMode::EthBeaconLightClient,
            ),
            sccp::Error::<Runtime>::DomainUnsupported
        );
    });
}

#[test]
fn sccp_set_inbound_finality_mode_rejects_non_manager_origin_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::set_inbound_finality_mode(
                RuntimeOrigin::signed(common::mock::alice()),
                SCCP_DOMAIN_SOL,
                InboundFinalityMode::SolanaLightClient,
            ),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn sccp_set_inbound_finality_mode_failure_does_not_emit_event_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        let events_before = frame_system::Pallet::<Runtime>::events().len();
        assert_noop!(
            Sccp::set_inbound_finality_mode(
                RuntimeOrigin::root(),
                SCCP_DOMAIN_ETH,
                InboundFinalityMode::SolanaLightClient,
            ),
            sccp::Error::<Runtime>::InboundFinalityModeUnsupported
        );
        assert_eq!(
            frame_system::Pallet::<Runtime>::events().len(),
            events_before
        );
    });
}

#[test]
fn sccp_set_inbound_finality_mode_overwrite_updates_override_in_runtime() {
    ext().execute_with(|| {
        assert_ok!(Sccp::set_inbound_finality_mode(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            InboundFinalityMode::Disabled,
        ));
        assert_eq!(
            Sccp::inbound_finality_mode_override(SCCP_DOMAIN_ETH),
            Some(InboundFinalityMode::Disabled)
        );

        assert_ok!(Sccp::set_inbound_finality_mode(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            InboundFinalityMode::EthBeaconLightClient,
        ));
        assert_eq!(
            Sccp::inbound_finality_mode_override(SCCP_DOMAIN_ETH),
            Some(InboundFinalityMode::EthBeaconLightClient)
        );
    });
}

#[test]
fn sccp_init_bsc_light_client_rejects_non_manager_origin_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::init_bsc_light_client(
                RuntimeOrigin::signed(common::mock::alice()),
                vec![0x01u8],
                vec![H160::from_low_u64_be(1)],
                1,
                0,
                56,
                1,
            ),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn sccp_init_bsc_light_client_rejects_oversized_header_in_runtime() {
    ext().execute_with(|| {
        let oversized = vec![0u8; SCCP_MAX_BSC_HEADER_RLP_BYTES + 1];
        assert_noop!(
            Sccp::init_bsc_light_client(
                RuntimeOrigin::root(),
                oversized,
                vec![H160::from_low_u64_be(1)],
                1,
                0,
                56,
                1,
            ),
            sccp::Error::<Runtime>::BscHeaderTooLarge
        );
    });
}

#[test]
fn sccp_init_bsc_light_client_failure_does_not_emit_event_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        let events_before = frame_system::Pallet::<Runtime>::events().len();
        let oversized = vec![0u8; SCCP_MAX_BSC_HEADER_RLP_BYTES + 1];
        assert_noop!(
            Sccp::init_bsc_light_client(
                RuntimeOrigin::root(),
                oversized,
                vec![H160::from_low_u64_be(1)],
                1,
                0,
                56,
                1,
            ),
            sccp::Error::<Runtime>::BscHeaderTooLarge
        );
        assert_eq!(
            frame_system::Pallet::<Runtime>::events().len(),
            events_before
        );
    });
}

#[test]
fn sccp_init_bsc_light_client_accepts_real_header_fixture_in_runtime() {
    use core::str::FromStr;

    ext().execute_with(|| {
        System::set_block_number(1);
        // Historical BSC mainnet header fixture (block 81,094,034).
        let header_rlp =
            include_bytes!("../../../pallets/sccp/src/fixtures/bsc_header_81094034.rlp").to_vec();
        let signer = H160::from_str("0x9f1b7fae54be07f4fee34eb1aacb39a1f7b6fc92").unwrap();
        let expected_hash =
            H256::from_str("0x61a7d2bdc5faf4bac24fc5f3fbeb4c810b05bc41b37fd1b8e86a26a66027225f")
                .unwrap();
        let expected_state_root =
            H256::from_str("0x687fc026dcc35d9f9e95c85b3692335885449560f1453d4179919ccd97a4590c")
                .unwrap();

        assert_ok!(Sccp::init_bsc_light_client(
            RuntimeOrigin::root(),
            header_rlp,
            vec![signer],
            1000,
            0,
            56,
            16,
        ));

        let head = Sccp::bsc_head().expect("bsc head must be set");
        assert_eq!(head.hash, expected_hash);
        assert_eq!(head.number, 81_094_034);
        assert_eq!(head.state_root, expected_state_root);
        assert_eq!(head.signer, signer);

        let finalized = Sccp::bsc_finalized().expect("bsc finalized must be set");
        assert_eq!(finalized.hash, expected_hash);
        assert_eq!(finalized.number, 81_094_034);

        let init_event = frame_system::Pallet::<Runtime>::events()
            .into_iter()
            .rev()
            .find_map(|record| match record.event {
                crate::RuntimeEvent::Sccp(sccp::Event::BscLightClientInitialized {
                    head_hash,
                    head_number,
                }) => Some((head_hash, head_number)),
                _ => None,
            })
            .expect("BscLightClientInitialized event expected");
        assert_eq!(init_event.0, expected_hash);
        assert_eq!(init_event.1, 81_094_034);

        let finalized_event = frame_system::Pallet::<Runtime>::events()
            .into_iter()
            .rev()
            .find_map(|record| match record.event {
                crate::RuntimeEvent::Sccp(sccp::Event::BscFinalizedUpdated {
                    hash,
                    number,
                    state_root,
                }) => Some((hash, number, state_root)),
                _ => None,
            })
            .expect("BscFinalizedUpdated event expected");
        assert_eq!(finalized_event.0, expected_hash);
        assert_eq!(finalized_event.1, 81_094_034);
        assert_eq!(finalized_event.2, expected_state_root);
    });
}

#[test]
fn sccp_submit_bsc_header_imports_linear_extension_and_updates_finalized_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        let v0 = ecdsa::Pair::from_seed(&[1u8; 32]);
        let v1 = ecdsa::Pair::from_seed(&[2u8; 32]);
        let v2 = ecdsa::Pair::from_seed(&[3u8; 32]);
        let mut validators = vec![
            (sccp_test_eth_address_from_pair(&v0), v0),
            (sccp_test_eth_address_from_pair(&v1), v1),
            (sccp_test_eth_address_from_pair(&v2), v2),
        ];
        validators.sort_by_key(|(addr, _)| addr.0);
        let validator_addrs: Vec<H160> = validators.iter().map(|(a, _)| *a).collect();

        let (checkpoint_rlp, checkpoint_hash) = sccp_test_build_signed_bsc_header(
            H256::zero(),
            H256::repeat_byte(0x81),
            0,
            2,
            &validators[0].1,
        );
        assert_ok!(Sccp::init_bsc_light_client(
            RuntimeOrigin::root(),
            checkpoint_rlp,
            validator_addrs,
            200,
            0,
            56,
            1,
        ));

        let next_state_root = H256::repeat_byte(0x82);
        let (header_rlp, header_hash) = sccp_test_build_signed_bsc_header(
            checkpoint_hash,
            next_state_root,
            1,
            2,
            &validators[1].1,
        );
        assert_ok!(Sccp::submit_bsc_header(
            RuntimeOrigin::signed(common::mock::alice()),
            header_rlp,
        ));

        let head = Sccp::bsc_head().expect("bsc head must be set");
        assert_eq!(head.hash, header_hash);
        assert_eq!(head.number, 1);
        assert_eq!(head.state_root, next_state_root);
        assert_eq!(head.signer, validators[1].0);

        let finalized = Sccp::bsc_finalized().expect("bsc finalized must be set");
        assert_eq!(finalized.hash, header_hash);
        assert_eq!(finalized.number, 1);
        assert_eq!(finalized.state_root, next_state_root);
        assert_eq!(finalized.signer, validators[1].0);

        let imported_event = frame_system::Pallet::<Runtime>::events()
            .into_iter()
            .rev()
            .find_map(|record| match record.event {
                crate::RuntimeEvent::Sccp(sccp::Event::BscHeaderImported {
                    hash,
                    number,
                    signer,
                    state_root,
                }) => Some((hash, number, signer, state_root)),
                _ => None,
            })
            .expect("BscHeaderImported event expected");
        assert_eq!(imported_event.0, header_hash);
        assert_eq!(imported_event.1, 1);
        assert_eq!(imported_event.2, validators[1].0);
        assert_eq!(imported_event.3, next_state_root);

        let finalized_event = frame_system::Pallet::<Runtime>::events()
            .into_iter()
            .rev()
            .find_map(|record| match record.event {
                crate::RuntimeEvent::Sccp(sccp::Event::BscFinalizedUpdated {
                    hash,
                    number,
                    state_root,
                }) => Some((hash, number, state_root)),
                _ => None,
            })
            .expect("BscFinalizedUpdated event expected");
        assert_eq!(finalized_event.0, header_hash);
        assert_eq!(finalized_event.1, 1);
        assert_eq!(finalized_event.2, next_state_root);
    });
}

#[test]
fn sccp_submit_bsc_header_rejects_when_not_initialized_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::submit_bsc_header(RuntimeOrigin::signed(common::mock::alice()), vec![]),
            sccp::Error::<Runtime>::BscLightClientNotInitialized
        );
    });
}

#[test]
fn sccp_submit_bsc_header_rejects_unsigned_origin_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::submit_bsc_header(RuntimeOrigin::root(), vec![]),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn sccp_submit_bsc_header_rejects_oversized_header_in_runtime() {
    ext().execute_with(|| {
        let oversized = vec![0u8; SCCP_MAX_BSC_HEADER_RLP_BYTES + 1];
        assert_noop!(
            Sccp::submit_bsc_header(RuntimeOrigin::signed(common::mock::alice()), oversized),
            sccp::Error::<Runtime>::BscHeaderTooLarge
        );
    });
}

#[test]
fn sccp_submit_bsc_header_failure_does_not_emit_event_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        let v0 = ecdsa::Pair::from_seed(&[1u8; 32]);
        let v1 = ecdsa::Pair::from_seed(&[2u8; 32]);
        let v2 = ecdsa::Pair::from_seed(&[3u8; 32]);
        let mut validators = vec![
            (sccp_test_eth_address_from_pair(&v0), v0),
            (sccp_test_eth_address_from_pair(&v1), v1),
            (sccp_test_eth_address_from_pair(&v2), v2),
        ];
        validators.sort_by_key(|(addr, _)| addr.0);
        let validator_addrs: Vec<H160> = validators.iter().map(|(a, _)| *a).collect();
        let (checkpoint_rlp, _) = sccp_test_build_signed_bsc_header(
            H256::zero(),
            H256::repeat_byte(0x91),
            0,
            2,
            &validators[0].1,
        );
        assert_ok!(Sccp::init_bsc_light_client(
            RuntimeOrigin::root(),
            checkpoint_rlp,
            validator_addrs,
            200,
            0,
            56,
            1,
        ));

        let events_before = frame_system::Pallet::<Runtime>::events().len();
        let oversized = vec![0u8; SCCP_MAX_BSC_HEADER_RLP_BYTES + 1];
        assert_noop!(
            Sccp::submit_bsc_header(RuntimeOrigin::signed(common::mock::alice()), oversized),
            sccp::Error::<Runtime>::BscHeaderTooLarge
        );
        assert_eq!(
            frame_system::Pallet::<Runtime>::events().len(),
            events_before
        );
    });
}

#[test]
fn sccp_set_bsc_validators_updates_state_and_emits_event_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        let v1 = H160::from_low_u64_be(1);
        let v2 = H160::from_low_u64_be(2);
        let expected_sorted = vec![v1, v2];
        let expected_hash = H256::from_slice(&keccak_256(&expected_sorted.encode()));

        assert_ok!(Sccp::set_bsc_validators(
            RuntimeOrigin::root(),
            vec![v2, v1],
        ));

        let stored = Sccp::bsc_validators().map(|set| set.into_inner());
        assert_eq!(stored, Some(expected_sorted));

        let event = frame_system::Pallet::<Runtime>::events()
            .into_iter()
            .rev()
            .find_map(|record| match record.event {
                crate::RuntimeEvent::Sccp(sccp::Event::BscValidatorsUpdated {
                    number,
                    validators_hash,
                }) => Some((number, validators_hash)),
                _ => None,
            });
        assert_eq!(event, Some((0, expected_hash)));
    });
}

#[test]
fn sccp_set_bsc_validators_rejects_duplicate_entries_in_runtime() {
    ext().execute_with(|| {
        let v = H160::from_low_u64_be(1);
        assert_noop!(
            Sccp::set_bsc_validators(RuntimeOrigin::root(), vec![v, v]),
            sccp::Error::<Runtime>::BscValidatorsInvalid
        );
    });
}

#[test]
fn sccp_set_bsc_validators_rejects_non_manager_origin_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::set_bsc_validators(
                RuntimeOrigin::signed(common::mock::alice()),
                vec![H160::from_low_u64_be(1)],
            ),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn sccp_set_bsc_validators_failure_does_not_emit_event_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        let v = H160::from_low_u64_be(1);
        let events_before = frame_system::Pallet::<Runtime>::events().len();
        assert_noop!(
            Sccp::set_bsc_validators(RuntimeOrigin::root(), vec![v, v]),
            sccp::Error::<Runtime>::BscValidatorsInvalid
        );
        assert_eq!(
            frame_system::Pallet::<Runtime>::events().len(),
            events_before
        );
    });
}

#[test]
fn sccp_init_tron_light_client_rejects_non_manager_origin_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::init_tron_light_client(
                RuntimeOrigin::signed(common::mock::alice()),
                vec![0x01u8],
                vec![0u8; 65],
                vec![H160::from_low_u64_be(1)],
                0x41,
            ),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn sccp_init_tron_light_client_rejects_oversized_raw_data_in_runtime() {
    ext().execute_with(|| {
        let oversized = vec![0u8; SCCP_MAX_TRON_RAW_DATA_BYTES + 1];
        assert_noop!(
            Sccp::init_tron_light_client(
                RuntimeOrigin::root(),
                oversized,
                vec![0u8; 65],
                vec![H160::from_low_u64_be(1)],
                0x41,
            ),
            sccp::Error::<Runtime>::TronHeaderTooLarge
        );
    });
}

#[test]
fn sccp_init_tron_light_client_rejects_invalid_signature_length_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::init_tron_light_client(
                RuntimeOrigin::root(),
                vec![0x01u8],
                vec![0u8; 64],
                vec![H160::from_low_u64_be(1)],
                0x41,
            ),
            sccp::Error::<Runtime>::TronHeaderInvalid
        );
    });
}

#[test]
fn sccp_init_tron_light_client_failure_does_not_emit_event_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        let events_before = frame_system::Pallet::<Runtime>::events().len();
        assert_noop!(
            Sccp::init_tron_light_client(
                RuntimeOrigin::root(),
                vec![0x01u8],
                vec![0u8; 64],
                vec![H160::from_low_u64_be(1)],
                0x41,
            ),
            sccp::Error::<Runtime>::TronHeaderInvalid
        );
        assert_eq!(
            frame_system::Pallet::<Runtime>::events().len(),
            events_before
        );
    });
}

#[test]
fn sccp_init_tron_light_client_sets_head_and_finalized_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        let p0 = ecdsa::Pair::from_seed(&[1u8; 32]);
        let p1 = ecdsa::Pair::from_seed(&[2u8; 32]);
        let p2 = ecdsa::Pair::from_seed(&[3u8; 32]);
        let w0 = sccp_test_eth_address_from_pair(&p0);
        let w1 = sccp_test_eth_address_from_pair(&p1);
        let w2 = sccp_test_eth_address_from_pair(&p2);
        let mut witnesses = vec![w0, w1, w2];
        witnesses.sort_by_key(|a| a.0);
        let expected_witnesses_hash = H256::from_slice(&keccak_256(&witnesses.encode()));

        let number = 1u64;
        let state_root = H256::repeat_byte(0x61);
        let raw_data = sccp_test_tron_raw_data(H256::zero(), number, w0, state_root);
        let signature = sccp_test_tron_witness_signature(&raw_data, &p0);

        assert_ok!(Sccp::init_tron_light_client(
            RuntimeOrigin::root(),
            raw_data.clone(),
            signature,
            witnesses,
            0x41,
        ));

        let expected_hash = sccp_test_tron_block_id(number, &raw_data);
        let head = Sccp::tron_head().expect("tron head must be set");
        assert_eq!(head.hash, expected_hash);
        assert_eq!(head.number, number);
        assert_eq!(head.state_root, state_root);
        assert_eq!(head.signer, w0);

        let finalized = Sccp::tron_finalized().expect("tron finalized must be set");
        assert_eq!(finalized.hash, expected_hash);
        assert_eq!(finalized.number, number);
        assert_eq!(finalized.state_root, state_root);
        assert_eq!(finalized.signer, w0);

        let init_event = frame_system::Pallet::<Runtime>::events()
            .into_iter()
            .rev()
            .find_map(|record| match record.event {
                crate::RuntimeEvent::Sccp(sccp::Event::TronLightClientInitialized {
                    head_hash,
                    head_number,
                }) => Some((head_hash, head_number)),
                _ => None,
            })
            .expect("TronLightClientInitialized event expected");
        assert_eq!(init_event.0, expected_hash);
        assert_eq!(init_event.1, number);

        let finalized_event = frame_system::Pallet::<Runtime>::events()
            .into_iter()
            .rev()
            .find_map(|record| match record.event {
                crate::RuntimeEvent::Sccp(sccp::Event::TronFinalizedUpdated {
                    hash,
                    number,
                    state_root,
                }) => Some((hash, number, state_root)),
                _ => None,
            })
            .expect("TronFinalizedUpdated event expected");
        assert_eq!(finalized_event.0, expected_hash);
        assert_eq!(finalized_event.1, number);
        assert_eq!(finalized_event.2, state_root);

        let witnesses_event = frame_system::Pallet::<Runtime>::events()
            .into_iter()
            .rev()
            .find_map(|record| match record.event {
                crate::RuntimeEvent::Sccp(sccp::Event::TronWitnessesUpdated {
                    number,
                    witnesses_hash,
                }) => Some((number, witnesses_hash)),
                _ => None,
            })
            .expect("TronWitnessesUpdated event expected");
        assert_eq!(witnesses_event.0, number);
        assert_eq!(witnesses_event.1, expected_witnesses_hash);
    });
}

#[test]
fn sccp_submit_tron_header_rejects_when_not_initialized_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::submit_tron_header(
                RuntimeOrigin::signed(common::mock::alice()),
                vec![],
                vec![0u8; 65],
            ),
            sccp::Error::<Runtime>::TronLightClientNotInitialized
        );
    });
}

#[test]
fn sccp_submit_tron_header_rejects_unsigned_origin_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::submit_tron_header(RuntimeOrigin::root(), vec![], vec![0u8; 65]),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn sccp_submit_tron_header_rejects_oversized_raw_data_in_runtime() {
    ext().execute_with(|| {
        let oversized = vec![0u8; SCCP_MAX_TRON_RAW_DATA_BYTES + 1];
        assert_noop!(
            Sccp::submit_tron_header(
                RuntimeOrigin::signed(common::mock::alice()),
                oversized,
                vec![0u8; 65],
            ),
            sccp::Error::<Runtime>::TronHeaderTooLarge
        );
    });
}

#[test]
fn sccp_submit_tron_header_rejects_invalid_signature_length_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::submit_tron_header(
                RuntimeOrigin::signed(common::mock::alice()),
                vec![0x01u8],
                vec![0u8; 64],
            ),
            sccp::Error::<Runtime>::TronHeaderInvalid
        );
    });
}

#[test]
fn sccp_submit_tron_header_failure_does_not_emit_event_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        let p0 = ecdsa::Pair::from_seed(&[1u8; 32]);
        let p1 = ecdsa::Pair::from_seed(&[2u8; 32]);
        let p2 = ecdsa::Pair::from_seed(&[3u8; 32]);
        let w0 = sccp_test_eth_address_from_pair(&p0);
        let w1 = sccp_test_eth_address_from_pair(&p1);
        let w2 = sccp_test_eth_address_from_pair(&p2);
        let mut witnesses = vec![w0, w1, w2];
        witnesses.sort_by_key(|a| a.0);

        let raw_data_1 = sccp_test_tron_raw_data(H256::zero(), 1, w0, H256::repeat_byte(0x81));
        let sig_1 = sccp_test_tron_witness_signature(&raw_data_1, &p0);
        assert_ok!(Sccp::init_tron_light_client(
            RuntimeOrigin::root(),
            raw_data_1.clone(),
            sig_1,
            witnesses,
            0x41,
        ));

        let hash_1 = sccp_test_tron_block_id(1, &raw_data_1);
        let raw_data_2 = sccp_test_tron_raw_data(hash_1, 2, w1, H256::repeat_byte(0x82));
        let events_before = frame_system::Pallet::<Runtime>::events().len();
        assert_noop!(
            Sccp::submit_tron_header(
                RuntimeOrigin::signed(common::mock::alice()),
                raw_data_2,
                vec![0u8; 64],
            ),
            sccp::Error::<Runtime>::TronHeaderInvalid
        );
        assert_eq!(
            frame_system::Pallet::<Runtime>::events().len(),
            events_before
        );
    });
}

#[test]
fn sccp_submit_tron_header_imports_linear_extension_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        let p0 = ecdsa::Pair::from_seed(&[1u8; 32]);
        let p1 = ecdsa::Pair::from_seed(&[2u8; 32]);
        let p2 = ecdsa::Pair::from_seed(&[3u8; 32]);
        let w0 = sccp_test_eth_address_from_pair(&p0);
        let w1 = sccp_test_eth_address_from_pair(&p1);
        let w2 = sccp_test_eth_address_from_pair(&p2);
        let mut witnesses = vec![w0, w1, w2];
        witnesses.sort_by_key(|a| a.0);

        let raw_data_1 = sccp_test_tron_raw_data(H256::zero(), 1, w0, H256::repeat_byte(0x71));
        let sig_1 = sccp_test_tron_witness_signature(&raw_data_1, &p0);
        assert_ok!(Sccp::init_tron_light_client(
            RuntimeOrigin::root(),
            raw_data_1.clone(),
            sig_1,
            witnesses,
            0x41,
        ));

        let hash_1 = sccp_test_tron_block_id(1, &raw_data_1);
        let raw_data_2 = sccp_test_tron_raw_data(hash_1, 2, w1, H256::repeat_byte(0x72));
        let sig_2 = sccp_test_tron_witness_signature(&raw_data_2, &p1);
        assert_ok!(Sccp::submit_tron_header(
            RuntimeOrigin::signed(common::mock::alice()),
            raw_data_2.clone(),
            sig_2,
        ));

        let hash_2 = sccp_test_tron_block_id(2, &raw_data_2);
        let head = Sccp::tron_head().expect("tron head must be set");
        assert_eq!(head.hash, hash_2);
        assert_eq!(head.number, 2);
        assert_eq!(head.state_root, H256::repeat_byte(0x72));
        assert_eq!(head.signer, w1);

        // With 3 witnesses, solidification threshold is 3, so one imported extension is not
        // enough to advance finalized yet.
        let finalized = Sccp::tron_finalized().expect("tron finalized must be set");
        assert_eq!(finalized.number, 1);
        assert_eq!(finalized.hash, hash_1);

        let imported_event = frame_system::Pallet::<Runtime>::events()
            .into_iter()
            .rev()
            .find_map(|record| match record.event {
                crate::RuntimeEvent::Sccp(sccp::Event::TronHeaderImported {
                    hash,
                    number,
                    signer,
                    state_root,
                }) => Some((hash, number, signer, state_root)),
                _ => None,
            })
            .expect("TronHeaderImported event expected");
        assert_eq!(imported_event.0, hash_2);
        assert_eq!(imported_event.1, 2);
        assert_eq!(imported_event.2, w1);
        assert_eq!(imported_event.3, H256::repeat_byte(0x72));
    });
}

#[test]
fn sccp_set_tron_witnesses_updates_state_and_emits_event_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        let w1 = H160::from_low_u64_be(11);
        let w2 = H160::from_low_u64_be(22);
        let expected_sorted = vec![w1, w2];
        let expected_hash = H256::from_slice(&keccak_256(&expected_sorted.encode()));

        assert_ok!(Sccp::set_tron_witnesses(
            RuntimeOrigin::root(),
            vec![w2, w1],
        ));

        let stored = Sccp::tron_witnesses().map(|set| set.into_inner());
        assert_eq!(stored, Some(expected_sorted));

        let event = frame_system::Pallet::<Runtime>::events()
            .into_iter()
            .rev()
            .find_map(|record| match record.event {
                crate::RuntimeEvent::Sccp(sccp::Event::TronWitnessesUpdated {
                    number,
                    witnesses_hash,
                }) => Some((number, witnesses_hash)),
                _ => None,
            });
        assert_eq!(event, Some((0, expected_hash)));
    });
}

#[test]
fn sccp_set_tron_witnesses_rejects_duplicate_entries_in_runtime() {
    ext().execute_with(|| {
        let w = H160::from_low_u64_be(1);
        assert_noop!(
            Sccp::set_tron_witnesses(RuntimeOrigin::root(), vec![w, w]),
            sccp::Error::<Runtime>::TronWitnessesInvalid
        );
    });
}

#[test]
fn sccp_set_tron_witnesses_rejects_non_manager_origin_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::set_tron_witnesses(
                RuntimeOrigin::signed(common::mock::alice()),
                vec![H160::from_low_u64_be(1)],
            ),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn sccp_set_tron_witnesses_failure_does_not_emit_event_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        let w = H160::from_low_u64_be(1);
        let events_before = frame_system::Pallet::<Runtime>::events().len();
        assert_noop!(
            Sccp::set_tron_witnesses(RuntimeOrigin::root(), vec![w, w]),
            sccp::Error::<Runtime>::TronWitnessesInvalid
        );
        assert_eq!(
            frame_system::Pallet::<Runtime>::events().len(),
            events_before
        );
    });
}

#[test]
fn sccp_set_solana_vote_authorities_updates_state_and_emits_event_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        let input = vec![
            sccp::SolanaVoteAuthorityV1 {
                authority_pubkey: [0x33u8; 32],
                stake: 30,
            },
            sccp::SolanaVoteAuthorityV1 {
                authority_pubkey: [0x11u8; 32],
                stake: 10,
            },
            sccp::SolanaVoteAuthorityV1 {
                authority_pubkey: [0x22u8; 32],
                stake: 20,
            },
        ];
        assert_ok!(Sccp::set_solana_vote_authorities(
            RuntimeOrigin::root(),
            input.clone(),
        ));

        let mut sorted = input;
        sorted.sort_by(|a, b| a.authority_pubkey.cmp(&b.authority_pubkey));
        let total_stake = sorted.iter().map(|authority| authority.stake).sum::<u64>();
        let threshold_stake = (total_stake * 2) / 3 + 1;
        let expected_hash = H256::from_slice(&keccak_256(&sorted.encode()));

        assert_eq!(
            Sccp::solana_vote_authorities().map(|v| v.into_inner()),
            Some(sorted)
        );

        let event = frame_system::Pallet::<Runtime>::events()
            .into_iter()
            .rev()
            .find_map(|record| match record.event {
                crate::RuntimeEvent::Sccp(sccp::Event::SolanaVoteAuthoritiesSet {
                    authorities_hash,
                    total_stake,
                    threshold_stake,
                }) => Some((authorities_hash, total_stake, threshold_stake)),
                _ => None,
            });
        assert_eq!(event, Some((expected_hash, total_stake, threshold_stake)));
    });
}

#[test]
fn sccp_set_solana_vote_authorities_rejects_invalid_entries_and_origin_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::set_solana_vote_authorities(
                RuntimeOrigin::root(),
                vec![
                    sccp::SolanaVoteAuthorityV1 {
                        authority_pubkey: [0x11u8; 32],
                        stake: 1,
                    },
                    sccp::SolanaVoteAuthorityV1 {
                        authority_pubkey: [0x11u8; 32],
                        stake: 2,
                    },
                ],
            ),
            sccp::Error::<Runtime>::SolanaVoteAuthoritiesInvalid
        );
        assert_noop!(
            Sccp::set_solana_vote_authorities(
                RuntimeOrigin::root(),
                vec![sccp::SolanaVoteAuthorityV1 {
                    authority_pubkey: [0x22u8; 32],
                    stake: 0,
                }],
            ),
            sccp::Error::<Runtime>::SolanaVoteAuthoritiesInvalid
        );
        assert_noop!(
            Sccp::set_solana_vote_authorities(
                RuntimeOrigin::signed(common::mock::alice()),
                vec![sccp::SolanaVoteAuthorityV1 {
                    authority_pubkey: [0x33u8; 32],
                    stake: 1,
                }],
            ),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn sccp_clear_solana_vote_authorities_clears_state_and_emits_event_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(Sccp::set_solana_vote_authorities(
            RuntimeOrigin::root(),
            vec![sccp::SolanaVoteAuthorityV1 {
                authority_pubkey: [0x44u8; 32],
                stake: 10,
            }],
        ));
        assert!(Sccp::solana_vote_authorities().is_some());

        assert_ok!(Sccp::clear_solana_vote_authorities(RuntimeOrigin::root()));
        assert!(Sccp::solana_vote_authorities().is_none());

        let event = frame_system::Pallet::<Runtime>::events()
            .into_iter()
            .rev()
            .find_map(|record| match record.event {
                crate::RuntimeEvent::Sccp(sccp::Event::SolanaVoteAuthoritiesCleared) => Some(()),
                _ => None,
            });
        assert_eq!(event, Some(()));
    });
}

#[test]
fn sccp_invalidate_inbound_message_updates_state_and_emits_event_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        let message_id = H256::repeat_byte(0x42);
        assert_ok!(Sccp::invalidate_inbound_message(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            message_id,
        ));
        assert!(Sccp::invalidated_inbound(SCCP_DOMAIN_ETH, message_id));

        let event = frame_system::Pallet::<Runtime>::events()
            .into_iter()
            .rev()
            .find_map(|record| match record.event {
                crate::RuntimeEvent::Sccp(sccp::Event::InboundMessageInvalidated {
                    source_domain,
                    message_id,
                }) => Some((source_domain, message_id)),
                _ => None,
            });
        assert_eq!(event, Some((SCCP_DOMAIN_ETH, message_id)));
    });
}

#[test]
fn sccp_clear_invalidated_inbound_message_updates_state_and_emits_event_in_runtime() {
    ext().execute_with(|| {
        let message_id = H256::repeat_byte(0x24);
        assert_ok!(Sccp::invalidate_inbound_message(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            message_id,
        ));
        assert!(Sccp::invalidated_inbound(SCCP_DOMAIN_ETH, message_id));

        System::set_block_number(1);
        assert_ok!(Sccp::clear_invalidated_inbound_message(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            message_id,
        ));
        assert!(!Sccp::invalidated_inbound(SCCP_DOMAIN_ETH, message_id));

        let event = frame_system::Pallet::<Runtime>::events()
            .into_iter()
            .rev()
            .find_map(|record| match record.event {
                crate::RuntimeEvent::Sccp(sccp::Event::InboundMessageRevalidated {
                    source_domain,
                    message_id,
                }) => Some((source_domain, message_id)),
                _ => None,
            });
        assert_eq!(event, Some((SCCP_DOMAIN_ETH, message_id)));
    });
}

#[test]
fn sccp_invalidate_inbound_message_rejects_sora_domain_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::invalidate_inbound_message(
                RuntimeOrigin::root(),
                SCCP_DOMAIN_SORA,
                H256::repeat_byte(0x11),
            ),
            sccp::Error::<Runtime>::DomainUnsupported
        );
    });
}

#[test]
fn sccp_invalidate_inbound_message_rejects_unknown_domain_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::invalidate_inbound_message(RuntimeOrigin::root(), 777, H256::repeat_byte(0x31),),
            sccp::Error::<Runtime>::DomainUnsupported
        );
    });
}

#[test]
fn sccp_clear_invalidated_inbound_message_rejects_unknown_domain_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::clear_invalidated_inbound_message(
                RuntimeOrigin::root(),
                777,
                H256::repeat_byte(0x12),
            ),
            sccp::Error::<Runtime>::DomainUnsupported
        );
    });
}

#[test]
fn sccp_clear_invalidated_inbound_message_rejects_sora_domain_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::clear_invalidated_inbound_message(
                RuntimeOrigin::root(),
                SCCP_DOMAIN_SORA,
                H256::repeat_byte(0x32),
            ),
            sccp::Error::<Runtime>::DomainUnsupported
        );
    });
}

#[test]
fn sccp_invalidate_inbound_message_rejects_non_manager_origin_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::invalidate_inbound_message(
                RuntimeOrigin::signed(common::mock::alice()),
                SCCP_DOMAIN_ETH,
                H256::repeat_byte(0x13),
            ),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn sccp_clear_invalidated_inbound_message_rejects_non_manager_origin_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::clear_invalidated_inbound_message(
                RuntimeOrigin::signed(common::mock::alice()),
                SCCP_DOMAIN_ETH,
                H256::repeat_byte(0x14),
            ),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn sccp_invalidate_inbound_message_failure_does_not_emit_event_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        let events_before = frame_system::Pallet::<Runtime>::events().len();
        assert_noop!(
            Sccp::invalidate_inbound_message(
                RuntimeOrigin::root(),
                SCCP_DOMAIN_SORA,
                H256::repeat_byte(0x15),
            ),
            sccp::Error::<Runtime>::DomainUnsupported
        );
        assert_eq!(
            frame_system::Pallet::<Runtime>::events().len(),
            events_before
        );
    });
}

#[test]
fn sccp_clear_invalidated_inbound_message_failure_does_not_emit_event_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        let events_before = frame_system::Pallet::<Runtime>::events().len();
        assert_noop!(
            Sccp::clear_invalidated_inbound_message(
                RuntimeOrigin::root(),
                SCCP_DOMAIN_SORA,
                H256::repeat_byte(0x16),
            ),
            sccp::Error::<Runtime>::DomainUnsupported
        );
        assert_eq!(
            frame_system::Pallet::<Runtime>::events().len(),
            events_before
        );
    });
}

#[test]
fn sccp_mint_from_proof_rejects_non_sora_destination_in_runtime() {
    ext().execute_with(|| {
        let payload = sccp_test_burn_payload(SCCP_DOMAIN_ETH, SCCP_DOMAIN_BSC);
        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(common::mock::alice()),
                SCCP_DOMAIN_ETH,
                payload,
                vec![],
            ),
            sccp::Error::<Runtime>::DomainUnsupported
        );
    });
}

#[test]
fn sccp_mint_from_proof_rejects_unsigned_origin_in_runtime() {
    ext().execute_with(|| {
        let payload = sccp_test_burn_payload(SCCP_DOMAIN_ETH, SCCP_DOMAIN_SORA);
        assert_noop!(
            Sccp::mint_from_proof(RuntimeOrigin::root(), SCCP_DOMAIN_ETH, payload, vec![]),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn sccp_mint_from_proof_rejects_source_domain_mismatch_in_runtime() {
    ext().execute_with(|| {
        let payload = sccp_test_burn_payload(SCCP_DOMAIN_ETH, SCCP_DOMAIN_SORA);
        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(common::mock::alice()),
                SCCP_DOMAIN_BSC,
                payload,
                vec![],
            ),
            sccp::Error::<Runtime>::DomainUnsupported
        );
    });
}

#[test]
fn sccp_mint_from_proof_rejects_sora_source_domain_in_runtime() {
    ext().execute_with(|| {
        let payload = sccp_test_burn_payload(SCCP_DOMAIN_SORA, SCCP_DOMAIN_SORA);
        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(common::mock::alice()),
                SCCP_DOMAIN_SORA,
                payload,
                vec![],
            ),
            sccp::Error::<Runtime>::DomainUnsupported
        );
    });
}

#[test]
fn sccp_mint_from_proof_rejects_unknown_source_domain_in_runtime() {
    ext().execute_with(|| {
        let payload = sccp_test_burn_payload(777, SCCP_DOMAIN_SORA);
        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(common::mock::alice()),
                777,
                payload,
                vec![],
            ),
            sccp::Error::<Runtime>::DomainUnsupported
        );
    });
}

#[test]
fn sccp_mint_from_proof_rejects_missing_domain_endpoint_in_runtime() {
    ext().execute_with(|| {
        let payload = sccp_test_burn_payload(SCCP_DOMAIN_ETH, SCCP_DOMAIN_SORA);
        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(common::mock::alice()),
                SCCP_DOMAIN_ETH,
                payload,
                vec![],
            ),
            sccp::Error::<Runtime>::DomainEndpointMissing
        );
    });
}

#[test]
fn sccp_mint_from_proof_rejects_inbound_finality_unavailable_in_runtime() {
    ext().execute_with(|| {
        assert_ok!(Sccp::set_domain_endpoint(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            vec![0x61u8; 20],
        ));
        let payload = sccp_test_burn_payload(SCCP_DOMAIN_ETH, SCCP_DOMAIN_SORA);
        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(common::mock::alice()),
                SCCP_DOMAIN_ETH,
                payload,
                vec![],
            ),
            sccp::Error::<Runtime>::InboundFinalityUnavailable
        );
    });
}

#[test]
fn sccp_mint_from_proof_rejects_inbound_paused_domain_in_runtime() {
    ext().execute_with(|| {
        assert_ok!(Sccp::set_domain_endpoint(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_SOL,
            vec![0x62u8; 32],
        ));
        sccp_enable_solana_finality();
        assert_ok!(Sccp::set_inbound_domain_paused(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_SOL,
            true,
        ));

        let payload = sccp_test_burn_payload(SCCP_DOMAIN_SOL, SCCP_DOMAIN_SORA);
        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(common::mock::alice()),
                SCCP_DOMAIN_SOL,
                payload,
                vec![],
            ),
            sccp::Error::<Runtime>::InboundDomainPaused
        );
    });
}

#[test]
fn sccp_mint_from_proof_failure_does_not_emit_event_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        let payload = sccp_test_burn_payload(SCCP_DOMAIN_ETH, SCCP_DOMAIN_BSC);
        let events_before = frame_system::Pallet::<Runtime>::events().len();
        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(common::mock::alice()),
                SCCP_DOMAIN_ETH,
                payload,
                vec![],
            ),
            sccp::Error::<Runtime>::DomainUnsupported
        );
        assert_eq!(
            frame_system::Pallet::<Runtime>::events().len(),
            events_before
        );
    });
}

#[test]
fn sccp_attest_burn_rejects_sora_destination_in_runtime() {
    ext().execute_with(|| {
        let payload = sccp_test_burn_payload(SCCP_DOMAIN_ETH, SCCP_DOMAIN_SORA);
        assert_noop!(
            Sccp::attest_burn(
                RuntimeOrigin::signed(common::mock::alice()),
                SCCP_DOMAIN_ETH,
                payload,
                vec![],
            ),
            sccp::Error::<Runtime>::DomainUnsupported
        );
    });
}

#[test]
fn sccp_attest_burn_rejects_unsigned_origin_in_runtime() {
    ext().execute_with(|| {
        let payload = sccp_test_burn_payload(SCCP_DOMAIN_ETH, SCCP_DOMAIN_BSC);
        assert_noop!(
            Sccp::attest_burn(RuntimeOrigin::root(), SCCP_DOMAIN_ETH, payload, vec![]),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn sccp_attest_burn_rejects_source_domain_mismatch_in_runtime() {
    ext().execute_with(|| {
        let payload = sccp_test_burn_payload(SCCP_DOMAIN_ETH, SCCP_DOMAIN_BSC);
        assert_noop!(
            Sccp::attest_burn(
                RuntimeOrigin::signed(common::mock::alice()),
                SCCP_DOMAIN_SOL,
                payload,
                vec![],
            ),
            sccp::Error::<Runtime>::DomainUnsupported
        );
    });
}

#[test]
fn sccp_attest_burn_rejects_same_source_and_destination_in_runtime() {
    ext().execute_with(|| {
        let payload = sccp_test_burn_payload(SCCP_DOMAIN_ETH, SCCP_DOMAIN_ETH);
        assert_noop!(
            Sccp::attest_burn(
                RuntimeOrigin::signed(common::mock::alice()),
                SCCP_DOMAIN_ETH,
                payload,
                vec![],
            ),
            sccp::Error::<Runtime>::DomainUnsupported
        );
    });
}

#[test]
fn sccp_attest_burn_rejects_unknown_source_domain_in_runtime() {
    ext().execute_with(|| {
        let payload = sccp_test_burn_payload(777, SCCP_DOMAIN_ETH);
        assert_noop!(
            Sccp::attest_burn(
                RuntimeOrigin::signed(common::mock::alice()),
                777,
                payload,
                vec![],
            ),
            sccp::Error::<Runtime>::DomainUnsupported
        );
    });
}

#[test]
fn sccp_attest_burn_rejects_non_canonical_evm_recipient_in_runtime() {
    ext().execute_with(|| {
        let mut payload = sccp_test_burn_payload(SCCP_DOMAIN_SOL, SCCP_DOMAIN_ETH);
        payload.recipient = [0x77u8; 32];
        assert_noop!(
            Sccp::attest_burn(
                RuntimeOrigin::signed(common::mock::alice()),
                SCCP_DOMAIN_SOL,
                payload,
                vec![],
            ),
            sccp::Error::<Runtime>::RecipientNotCanonical
        );
    });
}

#[test]
fn sccp_attest_burn_rejects_inbound_paused_domain_in_runtime() {
    ext().execute_with(|| {
        assert_ok!(Sccp::set_inbound_domain_paused(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            true,
        ));
        let payload = sccp_test_burn_payload(SCCP_DOMAIN_ETH, SCCP_DOMAIN_BSC);
        assert_noop!(
            Sccp::attest_burn(
                RuntimeOrigin::signed(common::mock::alice()),
                SCCP_DOMAIN_ETH,
                payload,
                vec![],
            ),
            sccp::Error::<Runtime>::InboundDomainPaused
        );
    });
}

#[test]
fn sccp_attest_burn_rejects_outbound_paused_destination_in_runtime() {
    ext().execute_with(|| {
        assert_ok!(Sccp::set_outbound_domain_paused(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_BSC,
            true,
        ));
        let payload = sccp_test_burn_payload(SCCP_DOMAIN_ETH, SCCP_DOMAIN_BSC);
        assert_noop!(
            Sccp::attest_burn(
                RuntimeOrigin::signed(common::mock::alice()),
                SCCP_DOMAIN_ETH,
                payload,
                vec![],
            ),
            sccp::Error::<Runtime>::OutboundDomainPaused
        );
    });
}

#[test]
fn sccp_attest_burn_rejects_inbound_finality_unavailable_in_runtime() {
    ext().execute_with(|| {
        assert_ok!(Sccp::set_domain_endpoint(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            vec![0x63u8; 20],
        ));
        let payload = sccp_test_burn_payload(SCCP_DOMAIN_ETH, SCCP_DOMAIN_SOL);
        assert_noop!(
            Sccp::attest_burn(
                RuntimeOrigin::signed(common::mock::alice()),
                SCCP_DOMAIN_ETH,
                payload,
                vec![],
            ),
            sccp::Error::<Runtime>::InboundFinalityUnavailable
        );
    });
}

#[test]
fn sccp_attest_burn_rejects_zero_amount_before_endpoint_checks_in_runtime() {
    ext().execute_with(|| {
        let mut payload = sccp_test_burn_payload(SCCP_DOMAIN_SOL, SCCP_DOMAIN_ETH);
        payload.recipient = sccp_test_canonical_evm_recipient();
        payload.amount = 0;
        assert_noop!(
            Sccp::attest_burn(
                RuntimeOrigin::signed(common::mock::alice()),
                SCCP_DOMAIN_SOL,
                payload,
                vec![],
            ),
            sccp::Error::<Runtime>::AmountIsZero
        );
    });
}

#[test]
fn sccp_attest_burn_rejects_zero_recipient_before_endpoint_checks_in_runtime() {
    ext().execute_with(|| {
        let mut payload = sccp_test_burn_payload(SCCP_DOMAIN_SOL, SCCP_DOMAIN_ETH);
        payload.recipient = [0u8; 32];
        assert_noop!(
            Sccp::attest_burn(
                RuntimeOrigin::signed(common::mock::alice()),
                SCCP_DOMAIN_SOL,
                payload,
                vec![],
            ),
            sccp::Error::<Runtime>::RecipientIsZero
        );
    });
}

#[test]
fn sccp_attest_burn_failure_does_not_emit_event_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        let payload = sccp_test_burn_payload(SCCP_DOMAIN_ETH, SCCP_DOMAIN_SORA);
        let events_before = frame_system::Pallet::<Runtime>::events().len();
        assert_noop!(
            Sccp::attest_burn(
                RuntimeOrigin::signed(common::mock::alice()),
                SCCP_DOMAIN_ETH,
                payload,
                vec![],
            ),
            sccp::Error::<Runtime>::DomainUnsupported
        );
        assert_eq!(
            frame_system::Pallet::<Runtime>::events().len(),
            events_before
        );
    });
}

#[test]
fn sccp_mint_from_proof_truncated_proofs_fail_closed_without_replay_poisoning_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"SCCPMTP".to_vec()),
            AssetName(b"SCCP Mint Truncated Proof".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u128,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        for domain in SCCP_CORE_REMOTE_DOMAINS {
            assert_ok!(Sccp::set_remote_token(
                RuntimeOrigin::root(),
                asset_id,
                domain,
                sccp_test_remote_token_bytes(domain),
            ));
            assert_ok!(Sccp::set_domain_endpoint(
                RuntimeOrigin::root(),
                domain,
                sccp_test_domain_endpoint_bytes(domain),
            ));
        }
        assert_ok!(Sccp::activate_token(RuntimeOrigin::root(), asset_id));
        sccp_enable_solana_finality();

        let asset_h256: H256 = asset_id.into();
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SOL,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 278,
            sora_asset_id: asset_h256.0,
            amount: 9u128,
            recipient: [0x33u8; 32],
        };
        let message_id = sccp_test_message_id(&payload);

        let events_before = frame_system::Pallet::<Runtime>::events().len();
        for cut in 0..4 {
            let result = Sccp::mint_from_proof(
                RuntimeOrigin::signed(owner.clone()),
                SCCP_DOMAIN_SOL,
                payload.clone(),
                vec![0x99; cut],
            );
            assert!(
                result.is_err(),
                "truncated proof unexpectedly accepted at cut={}",
                cut
            );
            assert!(!Sccp::processed_inbound(message_id));
            assert_eq!(
                frame_system::Pallet::<Runtime>::events().len(),
                events_before
            );
        }
    });
}

#[test]
fn sccp_burn_rejects_zero_amount_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::burn(
                RuntimeOrigin::signed(common::mock::alice()),
                common::XOR.into(),
                0u128,
                SCCP_DOMAIN_ETH,
                sccp_test_canonical_evm_recipient(),
            ),
            sccp::Error::<Runtime>::AmountIsZero
        );
    });
}

#[test]
fn sccp_burn_rejects_unsigned_origin_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::burn(
                RuntimeOrigin::root(),
                common::XOR.into(),
                1u128,
                SCCP_DOMAIN_ETH,
                sccp_test_canonical_evm_recipient(),
            ),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn sccp_burn_rejects_zero_recipient_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::burn(
                RuntimeOrigin::signed(common::mock::alice()),
                common::XOR.into(),
                1u128,
                SCCP_DOMAIN_ETH,
                [0u8; 32],
            ),
            sccp::Error::<Runtime>::RecipientIsZero
        );
    });
}

#[test]
fn sccp_burn_rejects_sora_destination_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::burn(
                RuntimeOrigin::signed(common::mock::alice()),
                common::XOR.into(),
                1u128,
                SCCP_DOMAIN_SORA,
                [0x11u8; 32],
            ),
            sccp::Error::<Runtime>::DomainUnsupported
        );
    });
}

#[test]
fn sccp_burn_rejects_unknown_destination_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::burn(
                RuntimeOrigin::signed(common::mock::alice()),
                common::XOR.into(),
                1u128,
                777,
                [0x11u8; 32],
            ),
            sccp::Error::<Runtime>::DomainUnsupported
        );
    });
}

#[test]
fn sccp_burn_rejects_non_canonical_evm_recipient_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::burn(
                RuntimeOrigin::signed(common::mock::alice()),
                common::XOR.into(),
                1u128,
                SCCP_DOMAIN_ETH,
                [0x22u8; 32],
            ),
            sccp::Error::<Runtime>::RecipientNotCanonical
        );
    });
}

#[test]
fn sccp_burn_rejects_outbound_paused_domain_in_runtime() {
    ext().execute_with(|| {
        assert_ok!(Sccp::set_outbound_domain_paused(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            true,
        ));
        assert_noop!(
            Sccp::burn(
                RuntimeOrigin::signed(common::mock::alice()),
                common::XOR.into(),
                1u128,
                SCCP_DOMAIN_ETH,
                sccp_test_canonical_evm_recipient(),
            ),
            sccp::Error::<Runtime>::OutboundDomainPaused
        );
    });
}

#[test]
fn sccp_burn_rejects_missing_domain_endpoint_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::burn(
                RuntimeOrigin::signed(common::mock::alice()),
                common::XOR.into(),
                1u128,
                SCCP_DOMAIN_ETH,
                sccp_test_canonical_evm_recipient(),
            ),
            sccp::Error::<Runtime>::DomainEndpointMissing
        );
    });
}

#[test]
fn sccp_burn_rejects_token_not_found_after_endpoint_validation_in_runtime() {
    ext().execute_with(|| {
        assert_ok!(Sccp::set_domain_endpoint(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            vec![0x44u8; 20],
        ));
        assert_noop!(
            Sccp::burn(
                RuntimeOrigin::signed(common::mock::alice()),
                common::XOR.into(),
                1u128,
                SCCP_DOMAIN_ETH,
                sccp_test_canonical_evm_recipient(),
            ),
            sccp::Error::<Runtime>::TokenNotFound
        );
    });
}

#[test]
fn sccp_burn_rejects_token_not_active_in_runtime() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"SCCPBRP".to_vec()),
            AssetName(b"SCCP Burn Pending".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u128,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));

        assert_ok!(Sccp::set_domain_endpoint(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            vec![0x55u8; 20],
        ));

        assert_noop!(
            Sccp::burn(
                RuntimeOrigin::signed(owner),
                asset_id,
                1u128,
                SCCP_DOMAIN_ETH,
                sccp_test_canonical_evm_recipient(),
            ),
            sccp::Error::<Runtime>::TokenNotActive
        );
    });
}

#[test]
fn sccp_burn_failure_does_not_emit_event_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        let events_before = frame_system::Pallet::<Runtime>::events().len();
        assert_noop!(
            Sccp::burn(
                RuntimeOrigin::signed(common::mock::alice()),
                common::XOR.into(),
                1u128,
                SCCP_DOMAIN_SORA,
                [0x11u8; 32],
            ),
            sccp::Error::<Runtime>::DomainUnsupported
        );
        assert_eq!(
            frame_system::Pallet::<Runtime>::events().len(),
            events_before
        );
    });
}

#[test]
fn sccp_burn_records_burn_and_emits_event_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"SCCPBOK".to_vec()),
            AssetName(b"SCCP Burn Success".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u128,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            asset_id,
            100i128,
        ));
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        for domain in SCCP_CORE_REMOTE_DOMAINS {
            assert_ok!(Sccp::set_remote_token(
                RuntimeOrigin::root(),
                asset_id,
                domain,
                sccp_test_remote_token_bytes(domain),
            ));
            assert_ok!(Sccp::set_domain_endpoint(
                RuntimeOrigin::root(),
                domain,
                sccp_test_domain_endpoint_bytes(domain),
            ));
        }
        assert_ok!(Sccp::activate_token(RuntimeOrigin::root(), asset_id));

        let amount = 7u128;
        let recipient = sccp_test_canonical_evm_recipient();
        assert_ok!(Sccp::burn(
            RuntimeOrigin::signed(owner),
            asset_id,
            amount,
            SCCP_DOMAIN_ETH,
            recipient,
        ));

        assert_eq!(Sccp::nonce(), 1);

        let event = frame_system::Pallet::<Runtime>::events()
            .into_iter()
            .rev()
            .find_map(|record| match record.event {
                crate::RuntimeEvent::Sccp(sccp::Event::SccpBurned {
                    message_id,
                    asset_id,
                    amount,
                    dest_domain,
                    recipient,
                    nonce,
                }) => Some((message_id, asset_id, amount, dest_domain, recipient, nonce)),
                _ => None,
            })
            .expect("SccpBurned event expected");

        assert_eq!(event.1, asset_id);
        assert_eq!(event.2, amount);
        assert_eq!(event.3, SCCP_DOMAIN_ETH);
        assert_eq!(event.4, recipient);
        assert_eq!(event.5, 1);

        let burn_record = Sccp::burns(event.0).expect("burn record expected");
        assert_eq!(burn_record.asset_id, asset_id);
        assert_eq!(burn_record.amount, amount);
        assert_eq!(burn_record.dest_domain, SCCP_DOMAIN_ETH);
        assert_eq!(burn_record.recipient, recipient);
        assert_eq!(burn_record.nonce, 1);
    });
}

#[test]
fn sccp_add_token_rejects_non_manager_origin_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::add_token(
                RuntimeOrigin::signed(common::mock::alice()),
                common::XOR.into(),
            ),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn sccp_add_token_rejects_token_already_exists_in_runtime() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"SCCPADP".to_vec()),
            AssetName(b"SCCP Add Duplicate".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u128,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        assert_noop!(
            Sccp::add_token(RuntimeOrigin::root(), asset_id),
            sccp::Error::<Runtime>::TokenAlreadyExists
        );
    });
}

#[test]
fn sccp_add_token_failure_does_not_emit_event_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"SCCPADE".to_vec()),
            AssetName(b"SCCP Add Event".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u128,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        let events_before = frame_system::Pallet::<Runtime>::events().len();
        assert_noop!(
            Sccp::add_token(RuntimeOrigin::root(), asset_id),
            sccp::Error::<Runtime>::TokenAlreadyExists
        );
        assert_eq!(
            frame_system::Pallet::<Runtime>::events().len(),
            events_before
        );
    });
}

#[test]
fn sccp_add_token_emits_event_and_stores_pending_state_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"SCCPADD".to_vec()),
            AssetName(b"SCCP Add Event Success".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u128,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();

        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));

        let token_state = Sccp::token_state(asset_id).expect("token should be registered");
        assert_eq!(token_state.status, sccp::TokenStatus::Pending);

        let event = frame_system::Pallet::<Runtime>::events()
            .into_iter()
            .rev()
            .find_map(|record| match record.event {
                crate::RuntimeEvent::Sccp(sccp::Event::TokenAdded { asset_id }) => Some(asset_id),
                _ => None,
            });
        assert_eq!(event, Some(asset_id));
    });
}

#[test]
fn sccp_activate_token_rejects_non_manager_origin_in_runtime() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"SCCPACN".to_vec()),
            AssetName(b"SCCP Activate NonManager".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u128,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        for domain in SCCP_CORE_REMOTE_DOMAINS {
            assert_ok!(Sccp::set_remote_token(
                RuntimeOrigin::root(),
                asset_id,
                domain,
                sccp_test_remote_token_bytes(domain),
            ));
            assert_ok!(Sccp::set_domain_endpoint(
                RuntimeOrigin::root(),
                domain,
                sccp_test_domain_endpoint_bytes(domain),
            ));
        }
        assert_noop!(
            Sccp::activate_token(RuntimeOrigin::signed(owner), asset_id),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn sccp_activate_token_rejects_token_not_found_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::activate_token(RuntimeOrigin::root(), common::XOR.into()),
            sccp::Error::<Runtime>::TokenNotFound
        );
    });
}

#[test]
fn sccp_activate_token_rejects_token_not_pending_in_runtime() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"SCCPACT".to_vec()),
            AssetName(b"SCCP Activate Twice".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u128,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        for domain in SCCP_CORE_REMOTE_DOMAINS {
            assert_ok!(Sccp::set_remote_token(
                RuntimeOrigin::root(),
                asset_id,
                domain,
                sccp_test_remote_token_bytes(domain),
            ));
            assert_ok!(Sccp::set_domain_endpoint(
                RuntimeOrigin::root(),
                domain,
                sccp_test_domain_endpoint_bytes(domain),
            ));
        }
        assert_ok!(Sccp::activate_token(RuntimeOrigin::root(), asset_id));
        assert_noop!(
            Sccp::activate_token(RuntimeOrigin::root(), asset_id),
            sccp::Error::<Runtime>::TokenNotPending
        );
    });
}

#[test]
fn sccp_activate_token_failure_does_not_emit_event_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        let events_before = frame_system::Pallet::<Runtime>::events().len();
        assert_noop!(
            Sccp::activate_token(RuntimeOrigin::root(), common::XOR.into()),
            sccp::Error::<Runtime>::TokenNotFound
        );
        assert_eq!(
            frame_system::Pallet::<Runtime>::events().len(),
            events_before
        );
    });
}

#[test]
fn sccp_activate_token_emits_event_on_success_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"SCCPACE".to_vec()),
            AssetName(b"SCCP Activate Event".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u128,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        for domain in SCCP_CORE_REMOTE_DOMAINS {
            assert_ok!(Sccp::set_remote_token(
                RuntimeOrigin::root(),
                asset_id,
                domain,
                sccp_test_remote_token_bytes(domain),
            ));
            assert_ok!(Sccp::set_domain_endpoint(
                RuntimeOrigin::root(),
                domain,
                sccp_test_domain_endpoint_bytes(domain),
            ));
        }

        assert_ok!(Sccp::activate_token(RuntimeOrigin::root(), asset_id));
        let event = frame_system::Pallet::<Runtime>::events()
            .into_iter()
            .rev()
            .find_map(|record| match record.event {
                crate::RuntimeEvent::Sccp(sccp::Event::TokenActivated { asset_id }) => {
                    Some(asset_id)
                }
                _ => None,
            });
        assert_eq!(event, Some(asset_id));
    });
}

#[test]
fn sccp_remove_token_rejects_non_manager_origin_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::remove_token(
                RuntimeOrigin::signed(common::mock::alice()),
                common::XOR.into(),
            ),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn sccp_remove_token_rejects_token_not_found_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::remove_token(RuntimeOrigin::root(), common::XOR.into()),
            sccp::Error::<Runtime>::TokenNotFound
        );
    });
}

#[test]
fn sccp_remove_token_failure_does_not_emit_event_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        let events_before = frame_system::Pallet::<Runtime>::events().len();
        assert_noop!(
            Sccp::remove_token(RuntimeOrigin::root(), common::XOR.into()),
            sccp::Error::<Runtime>::TokenNotFound
        );
        assert_eq!(
            frame_system::Pallet::<Runtime>::events().len(),
            events_before
        );
    });
}

#[test]
fn sccp_remove_token_updates_state_and_event_until_according_to_grace_period_in_runtime() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"SCCPRUS".to_vec()),
            AssetName(b"SCCP Remove Until State".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u128,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        for domain in SCCP_CORE_REMOTE_DOMAINS {
            assert_ok!(Sccp::set_remote_token(
                RuntimeOrigin::root(),
                asset_id,
                domain,
                sccp_test_remote_token_bytes(domain),
            ));
            assert_ok!(Sccp::set_domain_endpoint(
                RuntimeOrigin::root(),
                domain,
                sccp_test_domain_endpoint_bytes(domain),
            ));
        }
        assert_ok!(Sccp::activate_token(RuntimeOrigin::root(), asset_id));

        assert_ok!(Sccp::set_inbound_grace_period(
            RuntimeOrigin::root(),
            5u32.into(),
        ));
        System::set_block_number(10);
        assert_ok!(Sccp::pause_token(RuntimeOrigin::root(), asset_id));
        assert_ok!(Sccp::remove_token(RuntimeOrigin::root(), asset_id));

        let token_state = Sccp::token_state(asset_id).expect("token should exist");
        assert_eq!(token_state.status, sccp::TokenStatus::Removing);
        assert!(!token_state.outbound_enabled);
        assert!(!token_state.inbound_enabled);
        assert_eq!(token_state.inbound_enabled_until, Some(15u32.into()));

        let event = frame_system::Pallet::<Runtime>::events()
            .into_iter()
            .rev()
            .find_map(|record| match record.event {
                crate::RuntimeEvent::Sccp(sccp::Event::TokenRemoved {
                    asset_id,
                    inbound_enabled_until,
                }) => Some((asset_id, inbound_enabled_until)),
                _ => None,
            });
        assert_eq!(event, Some((asset_id, 15u32.into())));
    });
}

#[test]
fn sccp_finalize_remove_rejects_non_manager_origin_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::finalize_remove(
                RuntimeOrigin::signed(common::mock::alice()),
                common::XOR.into(),
            ),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn sccp_finalize_remove_rejects_token_not_found_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::finalize_remove(RuntimeOrigin::root(), common::XOR.into()),
            sccp::Error::<Runtime>::TokenNotFound
        );
    });
}

#[test]
fn sccp_finalize_remove_rejects_token_not_removing_in_runtime() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"SCCPFNR".to_vec()),
            AssetName(b"SCCP Finalize NotRemoving".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u128,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        assert_noop!(
            Sccp::finalize_remove(RuntimeOrigin::root(), asset_id),
            sccp::Error::<Runtime>::TokenNotRemoving
        );
    });
}

#[test]
fn sccp_finalize_remove_failure_does_not_emit_event_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        let events_before = frame_system::Pallet::<Runtime>::events().len();
        assert_noop!(
            Sccp::finalize_remove(RuntimeOrigin::root(), common::XOR.into()),
            sccp::Error::<Runtime>::TokenNotFound
        );
        assert_eq!(
            frame_system::Pallet::<Runtime>::events().len(),
            events_before
        );
    });
}

#[test]
fn sccp_finalize_remove_grace_period_not_expired_does_not_emit_event_in_runtime() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"SCCPFGN".to_vec()),
            AssetName(b"SCCP Finalize Grace Event".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u128,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        for domain in SCCP_CORE_REMOTE_DOMAINS {
            assert_ok!(Sccp::set_remote_token(
                RuntimeOrigin::root(),
                asset_id,
                domain,
                sccp_test_remote_token_bytes(domain),
            ));
            assert_ok!(Sccp::set_domain_endpoint(
                RuntimeOrigin::root(),
                domain,
                sccp_test_domain_endpoint_bytes(domain),
            ));
        }
        assert_ok!(Sccp::activate_token(RuntimeOrigin::root(), asset_id));
        assert_ok!(Sccp::set_inbound_grace_period(
            RuntimeOrigin::root(),
            3u32.into(),
        ));
        System::set_block_number(1);
        assert_ok!(Sccp::pause_token(RuntimeOrigin::root(), asset_id));
        assert_ok!(Sccp::remove_token(RuntimeOrigin::root(), asset_id));

        let events_before = frame_system::Pallet::<Runtime>::events().len();
        assert_noop!(
            Sccp::finalize_remove(RuntimeOrigin::root(), asset_id),
            sccp::Error::<Runtime>::GracePeriodNotExpired
        );
        assert_eq!(
            frame_system::Pallet::<Runtime>::events().len(),
            events_before
        );
    });
}

#[test]
fn sccp_activate_requires_remote_tokens_for_all_core_domains_with_partial_configuration_in_runtime()
{
    ext().execute_with(|| {
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"SCCPART".to_vec()),
            AssetName(b"SCCP Partial Remote Tokens".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u128,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();

        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));

        for domain in SCCP_CORE_REMOTE_DOMAINS {
            assert_ok!(Sccp::set_domain_endpoint(
                RuntimeOrigin::root(),
                domain,
                sccp_test_domain_endpoint_bytes(domain),
            ));
        }

        // Only ETH remote token is configured.
        assert_ok!(Sccp::set_remote_token(
            RuntimeOrigin::root(),
            asset_id,
            SCCP_DOMAIN_ETH,
            vec![0x22u8; 20],
        ));

        assert_noop!(
            Sccp::activate_token(RuntimeOrigin::root(), asset_id),
            sccp::Error::<Runtime>::RemoteTokenMissing
        );
    });
}

#[test]
fn sccp_activate_requires_domain_endpoints_for_all_core_domains_with_partial_configuration_in_runtime(
) {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"SCCPAEP".to_vec()),
            AssetName(b"SCCP Partial Endpoints".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u128,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();

        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));

        for domain in SCCP_CORE_REMOTE_DOMAINS {
            assert_ok!(Sccp::set_remote_token(
                RuntimeOrigin::root(),
                asset_id,
                domain,
                sccp_test_remote_token_bytes(domain),
            ));
        }

        // Only ETH endpoint is configured.
        assert_ok!(Sccp::set_domain_endpoint(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            vec![0x44u8; 20],
        ));

        assert_noop!(
            Sccp::activate_token(RuntimeOrigin::root(), asset_id),
            sccp::Error::<Runtime>::DomainEndpointMissing
        );
    });
}

#[test]
fn sccp_activate_token_succeeds_with_all_core_domain_prerequisites_in_runtime() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"SCCPACT".to_vec()),
            AssetName(b"SCCP Activate Success".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u128,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();

        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));

        for domain in SCCP_CORE_REMOTE_DOMAINS {
            assert_ok!(Sccp::set_remote_token(
                RuntimeOrigin::root(),
                asset_id,
                domain,
                sccp_test_remote_token_bytes(domain),
            ));
            assert_ok!(Sccp::set_domain_endpoint(
                RuntimeOrigin::root(),
                domain,
                sccp_test_domain_endpoint_bytes(domain),
            ));
        }

        assert_ok!(Sccp::activate_token(RuntimeOrigin::root(), asset_id));
        let token_state = Sccp::token_state(asset_id).expect("token should exist");
        assert_eq!(token_state.status, sccp::TokenStatus::Active);
    });
}

#[test]
fn sccp_set_remote_token_rejects_invalid_length_for_eth_domain_in_runtime() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"SCCPRLE".to_vec()),
            AssetName(b"SCCP Remote Len ETH".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u128,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));

        // ETH requires 20-byte identifier, so 32-byte should fail.
        assert_noop!(
            Sccp::set_remote_token(
                RuntimeOrigin::root(),
                asset_id,
                SCCP_DOMAIN_ETH,
                vec![1u8; 32]
            ),
            sccp::Error::<Runtime>::RemoteTokenInvalidLength
        );
    });
}

#[test]
fn sccp_set_remote_token_rejects_invalid_length_for_solana_domain_in_runtime() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"SCCPRLS".to_vec()),
            AssetName(b"SCCP Remote Len SOL".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u128,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));

        // Solana requires 32-byte identifier, so 20-byte should fail.
        assert_noop!(
            Sccp::set_remote_token(
                RuntimeOrigin::root(),
                asset_id,
                SCCP_DOMAIN_SOL,
                vec![1u8; 20]
            ),
            sccp::Error::<Runtime>::RemoteTokenInvalidLength
        );
    });
}

#[test]
fn sccp_set_domain_endpoint_rejects_invalid_length_for_tron_domain_in_runtime() {
    ext().execute_with(|| {
        // TRON expects 20-byte endpoint, so 32-byte should fail.
        assert_noop!(
            Sccp::set_domain_endpoint(RuntimeOrigin::root(), SCCP_DOMAIN_TRON, vec![1u8; 32]),
            sccp::Error::<Runtime>::DomainEndpointInvalidLength
        );
    });
}

#[test]
fn sccp_set_domain_endpoint_rejects_invalid_length_for_ton_domain_in_runtime() {
    ext().execute_with(|| {
        // TON expects 32-byte endpoint, so 20-byte should fail.
        assert_noop!(
            Sccp::set_domain_endpoint(RuntimeOrigin::root(), SCCP_DOMAIN_TON, vec![1u8; 20]),
            sccp::Error::<Runtime>::DomainEndpointInvalidLength
        );
    });
}

#[test]
fn sccp_set_remote_token_rejects_sora_domain_in_runtime() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"SCCPRSD".to_vec()),
            AssetName(b"SCCP Remote SORA Domain".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u128,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));

        assert_noop!(
            Sccp::set_remote_token(
                RuntimeOrigin::root(),
                asset_id,
                SCCP_DOMAIN_SORA,
                vec![1u8; 20]
            ),
            sccp::Error::<Runtime>::DomainUnsupported
        );
    });
}

#[test]
fn sccp_set_remote_token_rejects_unknown_domain_in_runtime() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"SCCPRUD".to_vec()),
            AssetName(b"SCCP Remote Unknown Domain".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u128,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));

        assert_noop!(
            Sccp::set_remote_token(RuntimeOrigin::root(), asset_id, 777, vec![1u8; 20]),
            sccp::Error::<Runtime>::DomainUnsupported
        );
    });
}

#[test]
fn sccp_set_domain_endpoint_rejects_sora_domain_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::set_domain_endpoint(RuntimeOrigin::root(), SCCP_DOMAIN_SORA, vec![1u8; 20]),
            sccp::Error::<Runtime>::DomainUnsupported
        );
    });
}

#[test]
fn sccp_set_domain_endpoint_rejects_unknown_domain_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::set_domain_endpoint(RuntimeOrigin::root(), 777, vec![1u8; 20]),
            sccp::Error::<Runtime>::DomainUnsupported
        );
    });
}

#[test]
fn sccp_set_remote_token_rejects_non_manager_origin_in_runtime() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"SCCPRNO".to_vec()),
            AssetName(b"SCCP Remote Non Manager".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u128,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));

        assert_noop!(
            Sccp::set_remote_token(
                RuntimeOrigin::signed(owner),
                asset_id,
                SCCP_DOMAIN_ETH,
                vec![1u8; 20],
            ),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn sccp_set_domain_endpoint_rejects_non_manager_origin_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::set_domain_endpoint(
                RuntimeOrigin::signed(common::mock::alice()),
                SCCP_DOMAIN_ETH,
                vec![1u8; 20],
            ),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn sccp_set_remote_token_rejects_token_not_found_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::set_remote_token(
                RuntimeOrigin::root(),
                common::XOR.into(),
                SCCP_DOMAIN_ETH,
                vec![1u8; 20],
            ),
            sccp::Error::<Runtime>::TokenNotFound
        );
    });
}

#[test]
fn sccp_set_remote_token_failure_does_not_emit_event_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        let events_before = frame_system::Pallet::<Runtime>::events().len();
        assert_noop!(
            Sccp::set_remote_token(
                RuntimeOrigin::root(),
                common::XOR.into(),
                SCCP_DOMAIN_ETH,
                vec![1u8; 20],
            ),
            sccp::Error::<Runtime>::TokenNotFound
        );
        assert_eq!(
            frame_system::Pallet::<Runtime>::events().len(),
            events_before
        );
    });
}

#[test]
fn sccp_set_domain_endpoint_failure_does_not_emit_event_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        let events_before = frame_system::Pallet::<Runtime>::events().len();
        assert_noop!(
            Sccp::set_domain_endpoint(RuntimeOrigin::root(), 777, vec![1u8; 20]),
            sccp::Error::<Runtime>::DomainUnsupported
        );
        assert_eq!(
            frame_system::Pallet::<Runtime>::events().len(),
            events_before
        );
    });
}

#[test]
fn sccp_set_remote_token_emits_event_with_expected_hash_in_runtime() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"SCCPREV".to_vec()),
            AssetName(b"SCCP Remote Event Hash".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u128,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));

        System::set_block_number(1);
        let remote_token_id = vec![0xabu8; 20];
        let expected_hash = H256::from_slice(&keccak_256(&remote_token_id));

        assert_ok!(Sccp::set_remote_token(
            RuntimeOrigin::root(),
            asset_id,
            SCCP_DOMAIN_ETH,
            remote_token_id,
        ));

        let event = frame_system::Pallet::<Runtime>::events()
            .into_iter()
            .rev()
            .find_map(|record| match record.event {
                crate::RuntimeEvent::Sccp(sccp::Event::RemoteTokenSet {
                    asset_id,
                    domain_id,
                    id_hash,
                }) => Some((asset_id, domain_id, id_hash)),
                _ => None,
            });
        assert_eq!(event, Some((asset_id, SCCP_DOMAIN_ETH, expected_hash)));
    });
}

#[test]
fn sccp_set_domain_endpoint_emits_event_with_expected_hash_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        let endpoint_id = vec![0xcdu8; 20];
        let expected_hash = H256::from_slice(&keccak_256(&endpoint_id));

        assert_ok!(Sccp::set_domain_endpoint(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            endpoint_id,
        ));

        let event = frame_system::Pallet::<Runtime>::events()
            .into_iter()
            .rev()
            .find_map(|record| match record.event {
                crate::RuntimeEvent::Sccp(sccp::Event::DomainEndpointSet {
                    domain_id,
                    id_hash,
                }) => Some((domain_id, id_hash)),
                _ => None,
            });
        assert_eq!(event, Some((SCCP_DOMAIN_ETH, expected_hash)));
    });
}

#[test]
fn sccp_set_remote_token_overwrite_updates_storage_and_event_hash_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"SCCPROW".to_vec()),
            AssetName(b"SCCP Remote Overwrite".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u128,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));

        let first = vec![0x11u8; 20];
        let second = vec![0x22u8; 20];
        assert_ok!(Sccp::set_remote_token(
            RuntimeOrigin::root(),
            asset_id,
            SCCP_DOMAIN_ETH,
            first,
        ));
        assert_ok!(Sccp::set_remote_token(
            RuntimeOrigin::root(),
            asset_id,
            SCCP_DOMAIN_ETH,
            second.clone(),
        ));

        let stored = Sccp::remote_token(asset_id, SCCP_DOMAIN_ETH).map(|v| v.into_inner());
        assert_eq!(stored, Some(second.clone()));

        let expected_hash = H256::from_slice(&keccak_256(&second));
        let event = frame_system::Pallet::<Runtime>::events()
            .into_iter()
            .rev()
            .find_map(|record| match record.event {
                crate::RuntimeEvent::Sccp(sccp::Event::RemoteTokenSet { id_hash, .. }) => {
                    Some(id_hash)
                }
                _ => None,
            });
        assert_eq!(event, Some(expected_hash));
    });
}

#[test]
fn sccp_set_domain_endpoint_overwrite_updates_storage_and_event_hash_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        let first = vec![0x33u8; 20];
        let second = vec![0x44u8; 20];
        assert_ok!(Sccp::set_domain_endpoint(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            first,
        ));
        assert_ok!(Sccp::set_domain_endpoint(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            second.clone(),
        ));

        let stored = Sccp::domain_endpoint(SCCP_DOMAIN_ETH).map(|v| v.into_inner());
        assert_eq!(stored, Some(second.clone()));

        let expected_hash = H256::from_slice(&keccak_256(&second));
        let event = frame_system::Pallet::<Runtime>::events()
            .into_iter()
            .rev()
            .find_map(|record| match record.event {
                crate::RuntimeEvent::Sccp(sccp::Event::DomainEndpointSet { id_hash, .. }) => {
                    Some(id_hash)
                }
                _ => None,
            });
        assert_eq!(event, Some(expected_hash));
    });
}

#[test]
fn sccp_clear_domain_endpoint_clears_state_and_emits_event_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(Sccp::set_domain_endpoint(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            vec![0xaau8; 20],
        ));
        assert!(Sccp::domain_endpoint(SCCP_DOMAIN_ETH).is_some());

        assert_ok!(Sccp::clear_domain_endpoint(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
        ));
        assert!(Sccp::domain_endpoint(SCCP_DOMAIN_ETH).is_none());

        let event = frame_system::Pallet::<Runtime>::events()
            .into_iter()
            .rev()
            .find_map(|record| match record.event {
                crate::RuntimeEvent::Sccp(sccp::Event::DomainEndpointCleared { domain_id }) => {
                    Some(domain_id)
                }
                _ => None,
            });
        assert_eq!(event, Some(SCCP_DOMAIN_ETH));
    });
}

#[test]
fn sccp_clear_domain_endpoint_rejects_sora_domain_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::clear_domain_endpoint(RuntimeOrigin::root(), SCCP_DOMAIN_SORA),
            sccp::Error::<Runtime>::DomainUnsupported
        );
    });
}

#[test]
fn sccp_clear_domain_endpoint_rejects_unknown_domain_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::clear_domain_endpoint(RuntimeOrigin::root(), 777),
            sccp::Error::<Runtime>::DomainUnsupported
        );
    });
}

#[test]
fn sccp_clear_domain_endpoint_rejects_non_manager_origin_in_runtime() {
    ext().execute_with(|| {
        assert_noop!(
            Sccp::clear_domain_endpoint(
                RuntimeOrigin::signed(common::mock::alice()),
                SCCP_DOMAIN_ETH
            ),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn sccp_clear_domain_endpoint_failure_does_not_emit_event_in_runtime() {
    ext().execute_with(|| {
        System::set_block_number(1);
        let events_before = frame_system::Pallet::<Runtime>::events().len();
        assert_noop!(
            Sccp::clear_domain_endpoint(
                RuntimeOrigin::signed(common::mock::alice()),
                SCCP_DOMAIN_ETH
            ),
            DispatchError::BadOrigin
        );
        assert_eq!(
            frame_system::Pallet::<Runtime>::events().len(),
            events_before
        );
    });
}

#[test]
fn sccp_activate_token_rejects_after_clearing_endpoint_for_core_domain_in_runtime() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"SCCPCLE".to_vec()),
            AssetName(b"SCCP Cleared Endpoint".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u128,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();

        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        for domain in SCCP_CORE_REMOTE_DOMAINS {
            assert_ok!(Sccp::set_remote_token(
                RuntimeOrigin::root(),
                asset_id,
                domain,
                sccp_test_remote_token_bytes(domain),
            ));
            assert_ok!(Sccp::set_domain_endpoint(
                RuntimeOrigin::root(),
                domain,
                sccp_test_domain_endpoint_bytes(domain),
            ));
        }

        assert_ok!(Sccp::clear_domain_endpoint(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
        ));

        assert_noop!(
            Sccp::activate_token(RuntimeOrigin::root(), asset_id),
            sccp::Error::<Runtime>::DomainEndpointMissing
        );
    });
}

#[test]
fn sccp_remove_and_finalize_clears_token_and_remote_ids_in_runtime() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"SCCPRMF".to_vec()),
            AssetName(b"SCCP Remove Finalize".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u128,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();

        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        for domain in SCCP_CORE_REMOTE_DOMAINS {
            assert_ok!(Sccp::set_remote_token(
                RuntimeOrigin::root(),
                asset_id,
                domain,
                sccp_test_remote_token_bytes(domain),
            ));
            assert_ok!(Sccp::set_domain_endpoint(
                RuntimeOrigin::root(),
                domain,
                sccp_test_domain_endpoint_bytes(domain),
            ));
        }
        assert_ok!(Sccp::activate_token(RuntimeOrigin::root(), asset_id));

        assert_ok!(Sccp::set_inbound_grace_period(RuntimeOrigin::root(), 0u32,));

        System::set_block_number(1);
        assert_ok!(Sccp::pause_token(RuntimeOrigin::root(), asset_id));
        assert_ok!(Sccp::remove_token(RuntimeOrigin::root(), asset_id));
        assert_noop!(
            Sccp::finalize_remove(RuntimeOrigin::root(), asset_id),
            sccp::Error::<Runtime>::GracePeriodNotExpired
        );

        System::set_block_number(2);
        assert_ok!(Sccp::finalize_remove(RuntimeOrigin::root(), asset_id));

        assert!(!Sccp::is_sccp_asset(&asset_id));
        assert!(Sccp::token_state(asset_id).is_none());
        for domain in SCCP_CORE_REMOTE_DOMAINS {
            assert!(Sccp::remote_token(asset_id, domain).is_none());
        }
    });
}

#[test]
fn sccp_remove_and_finalize_emit_expected_events_in_runtime() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"SCCPREM".to_vec()),
            AssetName(b"SCCP Remove Events".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u128,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();

        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        for domain in SCCP_CORE_REMOTE_DOMAINS {
            assert_ok!(Sccp::set_remote_token(
                RuntimeOrigin::root(),
                asset_id,
                domain,
                sccp_test_remote_token_bytes(domain),
            ));
            assert_ok!(Sccp::set_domain_endpoint(
                RuntimeOrigin::root(),
                domain,
                sccp_test_domain_endpoint_bytes(domain),
            ));
        }
        assert_ok!(Sccp::activate_token(RuntimeOrigin::root(), asset_id));
        assert_ok!(Sccp::set_inbound_grace_period(RuntimeOrigin::root(), 0u32,));

        System::set_block_number(1);
        assert_ok!(Sccp::pause_token(RuntimeOrigin::root(), asset_id));
        assert_ok!(Sccp::remove_token(RuntimeOrigin::root(), asset_id));
        let removed_event = frame_system::Pallet::<Runtime>::events()
            .into_iter()
            .rev()
            .find_map(|record| match record.event {
                crate::RuntimeEvent::Sccp(sccp::Event::TokenRemoved { asset_id, .. }) => {
                    Some(asset_id)
                }
                _ => None,
            });
        assert_eq!(removed_event, Some(asset_id));

        System::set_block_number(2);
        assert_ok!(Sccp::finalize_remove(RuntimeOrigin::root(), asset_id));
        let finalized_event = frame_system::Pallet::<Runtime>::events()
            .into_iter()
            .rev()
            .find_map(|record| match record.event {
                crate::RuntimeEvent::Sccp(sccp::Event::TokenRemovalFinalized { asset_id }) => {
                    Some(asset_id)
                }
                _ => None,
            });
        assert_eq!(finalized_event, Some(asset_id));
    });
}

#[test]
fn sccp_add_token_rejects_asset_on_legacy_eth_bridge() {
    ext().execute_with(|| {
        let asset_id = common::XOR.into();
        assert_noop!(
            Sccp::add_token(RuntimeOrigin::root(), asset_id),
            sccp::Error::<Runtime>::AssetOnLegacyBridge
        );
    });
}

#[test]
fn sccp_add_token_rejects_asset_with_pending_legacy_eth_add_asset_request() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"SCCPPA".to_vec()),
            AssetName(b"SCCP Pending Add".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u128,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();

        let evm_net_id = <Runtime as eth_bridge::Config>::GetEthNetworkId::get();
        assert_ok!(crate::EthBridge::add_asset(
            RuntimeOrigin::root(),
            asset_id,
            evm_net_id,
        ));
        assert!(crate::EthBridge::registered_asset(evm_net_id, asset_id).is_none());
        assert!(LegacyBridgeChecker::is_legacy_bridge_asset(&asset_id));

        assert_noop!(
            Sccp::add_token(RuntimeOrigin::root(), asset_id),
            sccp::Error::<Runtime>::AssetOnLegacyBridge
        );
    });
}

#[test]
fn sccp_add_token_rejects_asset_on_secondary_legacy_eth_bridge_network() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"SCCP2E".to_vec()),
            AssetName(b"SCCP Legacy EVM Net".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u128,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();

        let secondary_net_id = crate::EthBridge::next_network_id();
        assert_ok!(crate::EthBridge::register_bridge(
            RuntimeOrigin::root(),
            H160::repeat_byte(0x31),
            vec![owner],
            eth_bridge::BridgeSignatureVersion::V3,
        ));
        assert_ok!(crate::EthBridge::register_existing_sidechain_asset(
            RuntimeOrigin::root(),
            asset_id,
            H160::repeat_byte(0x32),
            secondary_net_id,
        ));
        assert!(crate::EthBridge::registered_asset(secondary_net_id, asset_id).is_some());
        assert!(LegacyBridgeChecker::is_legacy_bridge_asset(&asset_id));

        assert_noop!(
            Sccp::add_token(RuntimeOrigin::root(), asset_id),
            sccp::Error::<Runtime>::AssetOnLegacyBridge
        );
    });
}

#[test]
fn sccp_add_token_rejects_asset_with_pending_legacy_eth_add_asset_on_secondary_network() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"SCCPP2".to_vec()),
            AssetName(b"SCCP Pending Other Net".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u128,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();

        let secondary_net_id = crate::EthBridge::next_network_id();
        assert_ok!(crate::EthBridge::register_bridge(
            RuntimeOrigin::root(),
            H160::repeat_byte(0x33),
            vec![owner],
            eth_bridge::BridgeSignatureVersion::V3,
        ));
        assert_ok!(crate::EthBridge::add_asset(
            RuntimeOrigin::root(),
            asset_id,
            secondary_net_id,
        ));
        assert!(crate::EthBridge::registered_asset(secondary_net_id, asset_id).is_none());
        assert!(LegacyBridgeChecker::is_legacy_bridge_asset(&asset_id));

        assert_noop!(
            Sccp::add_token(RuntimeOrigin::root(), asset_id),
            sccp::Error::<Runtime>::AssetOnLegacyBridge
        );
    });
}

#[test]
fn sccp_add_token_rejects_asset_on_legacy_ton_bridge() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"SCCPTON".to_vec()),
            AssetName(b"SCCP TON Legacy".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u128,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();

        assert_ok!(crate::JettonApp::register_network_with_existing_asset(
            RuntimeOrigin::root(),
            TonNetworkId::Mainnet,
            TonAddress::new(0, H256::repeat_byte(0x44)),
            asset_id,
            9,
        ));

        assert_noop!(
            Sccp::add_token(RuntimeOrigin::root(), asset_id),
            sccp::Error::<Runtime>::AssetOnLegacyBridge
        );
    });
}

#[test]
fn sccp_add_token_accepts_non_legacy_asset() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"SCCPNL".to_vec()),
            AssetName(b"SCCP Non Legacy".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u128,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();
        assert!(!LegacyBridgeChecker::is_legacy_bridge_asset(&asset_id));

        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        let token_state = Sccp::token_state(asset_id).expect("token should be registered");
        assert_eq!(token_state.status, sccp::TokenStatus::Pending);
    });
}

#[test]
fn sccp_asset_blocks_eth_bridge_add_asset_in_runtime() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"SCCPEVM".to_vec()),
            AssetName(b"SCCP EVM Blocked".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u128,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();

        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        let evm_net_id = <Runtime as eth_bridge::Config>::GetEthNetworkId::get();
        assert_noop!(
            crate::EthBridge::add_asset(RuntimeOrigin::root(), asset_id, evm_net_id),
            eth_bridge::Error::<Runtime>::SccpAssetNotAllowed
        );
    });
}

#[test]
fn sccp_asset_does_not_block_eth_bridge_add_sidechain_token_for_new_asset_id() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"SCCPEST".to_vec()),
            AssetName(b"SCCP Eth Sidechain Token".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u128,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();

        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        let evm_net_id = <Runtime as eth_bridge::Config>::GetEthNetworkId::get();
        let token_address = H160::repeat_byte(0x9a);
        assert_ok!(crate::EthBridge::add_sidechain_token(
            RuntimeOrigin::root(),
            token_address,
            "SCCPETH".into(),
            "SCCP Eth Sidechain".into(),
            18,
            evm_net_id,
        ));
        assert!(crate::EthBridge::registered_sidechain_asset(evm_net_id, token_address).is_none());
        assert!(crate::EthBridge::is_add_token_request_pending(
            evm_net_id,
            token_address
        ));
    });
}

#[test]
fn sccp_asset_blocks_jetton_register_network_with_existing_asset_in_runtime() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"SCCPTOB".to_vec()),
            AssetName(b"SCCP TON Blocked".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u128,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();

        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        let err = crate::JettonApp::register_network_with_existing_asset(
            RuntimeOrigin::root(),
            TonNetworkId::Mainnet,
            TonAddress::new(0, H256::repeat_byte(0x66)),
            asset_id,
            9,
        )
        .unwrap_err();
        assert_eq!(
            err,
            bridge_proxy::Error::<Runtime>::PathIsNotAvailable.into()
        );
    });
}

#[test]
fn sccp_asset_blocks_bridge_proxy_burn_in_runtime() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"SCCPBP".to_vec()),
            AssetName(b"SCCP BridgeProxy Burn".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u128,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();

        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        let evm_net_id = <Runtime as eth_bridge::Config>::GetEthNetworkId::get();
        assert_noop!(
            crate::BridgeProxy::burn(
                RuntimeOrigin::signed(owner),
                GenericNetworkId::EVMLegacy(evm_net_id),
                asset_id,
                GenericAccount::EVM(H160::repeat_byte(0x11)),
                1u32.into(),
            ),
            bridge_proxy::Error::<Runtime>::PathIsNotAvailable
        );
    });
}

#[test]
fn sccp_asset_blocks_eth_bridge_transfer_to_sidechain_in_runtime() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"SCCPET".to_vec()),
            AssetName(b"SCCP Eth Transfer".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u128,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();

        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        let evm_net_id = <Runtime as eth_bridge::Config>::GetEthNetworkId::get();
        assert_noop!(
            crate::EthBridge::transfer_to_sidechain(
                RuntimeOrigin::signed(owner),
                asset_id,
                H160::repeat_byte(0x22),
                1u32.into(),
                evm_net_id,
            ),
            eth_bridge::Error::<Runtime>::SccpAssetNotAllowed
        );
    });
}

#[test]
fn sccp_failed_incoming_transfer_rolls_back_bridge_lock_accounting_in_runtime() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"SCCPIR".to_vec()),
            AssetName(b"SCCP Incoming Revert".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u128,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();

        let evm_net_id = <Runtime as eth_bridge::Config>::GetEthNetworkId::get();
        let network_id = GenericNetworkId::EVMLegacy(evm_net_id);
        let amount = 7u32.into();
        let incoming_transfer = eth_bridge::requests::IncomingTransfer::<Runtime> {
            from: H160::repeat_byte(0x2b),
            to: owner.clone(),
            asset_id,
            asset_kind: eth_bridge::requests::AssetKind::Sidechain,
            amount,
            author: owner.clone(),
            tx_hash: H256::repeat_byte(0xa7),
            at_height: 1,
            timepoint: Default::default(),
            network_id: evm_net_id,
            should_take_fee: false,
        };
        assert_ok!(incoming_transfer.prepare());
        assert_eq!(
            crate::BridgeProxy::locked_assets(network_id, asset_id),
            amount
        );

        let incoming_request = eth_bridge::requests::IncomingRequest::Transfer(incoming_transfer);
        let offchain_request = eth_bridge::requests::OffchainRequest::incoming(incoming_request);
        let request_hash = match &offchain_request {
            eth_bridge::requests::OffchainRequest::Incoming(_, hash) => *hash,
            _ => unreachable!(),
        };
        eth_bridge::Requests::<Runtime>::insert(evm_net_id, request_hash, offchain_request);
        eth_bridge::RequestsQueue::<Runtime>::mutate(evm_net_id, |queue| queue.push(request_hash));
        eth_bridge::RequestStatuses::<Runtime>::insert(
            evm_net_id,
            request_hash,
            eth_bridge::requests::RequestStatus::Pending,
        );

        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));

        let bridge_account =
            crate::EthBridge::bridge_account(evm_net_id).expect("bridge account must exist");
        assert_ok!(crate::EthBridge::finalize_incoming_request(
            RuntimeOrigin::signed(bridge_account),
            request_hash,
            evm_net_id,
        ));

        assert_eq!(
            crate::EthBridge::request_status(evm_net_id, request_hash),
            Some(eth_bridge::requests::RequestStatus::Failed(
                eth_bridge::Error::<Runtime>::SccpAssetNotAllowed.into()
            ))
        );
        assert_eq!(
            crate::BridgeProxy::locked_assets(network_id, asset_id),
            0u128
        );
    });
}

#[test]
fn abort_outgoing_transfer_rolls_back_bridge_lock_accounting_in_runtime() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        let asset_id = common::XOR.into();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            asset_id,
            100i128,
        ));

        let evm_net_id = <Runtime as eth_bridge::Config>::GetEthNetworkId::get();
        let network_id = GenericNetworkId::EVMLegacy(evm_net_id);
        let amount = 9u32.into();
        let baseline_locked = crate::BridgeProxy::locked_assets(network_id, asset_id);

        assert_ok!(crate::EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(owner.clone()),
            asset_id,
            H160::repeat_byte(0x55),
            amount,
            evm_net_id,
        ));
        let request_hash = *crate::EthBridge::requests_queue(evm_net_id)
            .last()
            .expect("outgoing request hash should be queued");
        assert_eq!(
            crate::BridgeProxy::locked_assets(network_id, asset_id),
            baseline_locked + amount
        );

        let bridge_account =
            crate::EthBridge::bridge_account(evm_net_id).expect("bridge account must exist");
        assert_ok!(crate::EthBridge::abort_request(
            RuntimeOrigin::signed(bridge_account),
            request_hash,
            eth_bridge::Error::<Runtime>::Cancelled.into(),
            evm_net_id,
        ));

        assert_eq!(
            crate::EthBridge::request_status(evm_net_id, request_hash),
            Some(eth_bridge::requests::RequestStatus::Failed(
                eth_bridge::Error::<Runtime>::Cancelled.into()
            ))
        );
        assert_eq!(
            crate::BridgeProxy::locked_assets(network_id, asset_id),
            baseline_locked
        );
    });
}

#[test]
fn incoming_transfer_prepare_failure_rolls_back_bridge_lock_accounting_in_runtime() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        let asset_id = common::XOR.into();
        let evm_net_id = <Runtime as eth_bridge::Config>::GetEthNetworkId::get();
        let network_id = GenericNetworkId::EVMLegacy(evm_net_id);
        let bridge_account =
            crate::EthBridge::bridge_account(evm_net_id).expect("bridge account must exist");

        let bridge_free_balance = Currencies::free_balance(asset_id, &bridge_account);
        let amount = bridge_free_balance.saturating_add(1u32.into());
        let baseline_locked = crate::BridgeProxy::locked_assets(network_id, asset_id);

        assert_ok!(crate::BridgeProxy::before_asset_lock(
            network_id,
            bridge_types::types::AssetKind::Thischain,
            &asset_id,
            &amount,
        ));
        assert_eq!(
            crate::BridgeProxy::locked_assets(network_id, asset_id),
            baseline_locked + amount
        );

        let incoming_transfer = eth_bridge::requests::IncomingTransfer::<Runtime> {
            from: H160::repeat_byte(0x33),
            to: owner,
            asset_id,
            asset_kind: eth_bridge::requests::AssetKind::Thischain,
            amount,
            author: common::mock::bob(),
            tx_hash: H256::repeat_byte(0xb3),
            at_height: 1,
            timepoint: Default::default(),
            network_id: evm_net_id,
            should_take_fee: false,
        };

        assert!(incoming_transfer.prepare().is_err());
        assert_eq!(
            crate::BridgeProxy::locked_assets(network_id, asset_id),
            baseline_locked + amount
        );
    });
}

#[test]
fn sccp_incoming_queue_full_registration_does_not_change_bridge_lock_accounting_in_runtime() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        let evm_net_id = <Runtime as eth_bridge::Config>::GetEthNetworkId::get();
        let network_id = GenericNetworkId::EVMLegacy(evm_net_id);
        let asset_id = common::XOR.into();
        let baseline_locked = crate::BridgeProxy::locked_assets(network_id, asset_id);
        let sidechain_tx_hash = H256::repeat_byte(0x71);

        assert_ok!(crate::EthBridge::request_from_sidechain(
            RuntimeOrigin::signed(owner.clone()),
            sidechain_tx_hash,
            eth_bridge::requests::IncomingRequestKind::Transaction(
                eth_bridge::requests::IncomingTransactionRequestKind::Transfer
            ),
            evm_net_id,
        ));

        eth_bridge::RequestsQueue::<Runtime>::mutate(evm_net_id, |queue| {
            for i in 0..2048u64 {
                queue.push(H256::from_low_u64_be(10_000 + i));
            }
        });

        let incoming_transfer = eth_bridge::requests::IncomingRequest::Transfer(
            eth_bridge::requests::IncomingTransfer::<Runtime> {
                from: H160::repeat_byte(0x44),
                to: owner.clone(),
                asset_id,
                asset_kind: eth_bridge::requests::AssetKind::Sidechain,
                amount: 1u32.into(),
                author: owner,
                tx_hash: sidechain_tx_hash,
                at_height: 1,
                timepoint: Default::default(),
                network_id: evm_net_id,
                should_take_fee: false,
            },
        );

        let bridge_account =
            crate::EthBridge::bridge_account(evm_net_id).expect("bridge account must exist");
        let err = crate::EthBridge::register_incoming_request(
            RuntimeOrigin::signed(bridge_account),
            incoming_transfer,
        )
        .unwrap_err();
        assert_eq!(
            err.error,
            eth_bridge::Error::<Runtime>::RequestsQueueFull.into()
        );

        assert_eq!(
            crate::BridgeProxy::locked_assets(network_id, asset_id),
            baseline_locked
        );
    });
}

#[test]
fn sccp_import_incoming_registration_failure_aborts_load_request_in_runtime() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"SCCPIM".to_vec()),
            AssetName(b"SCCP Import Failure".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u128,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));

        let evm_net_id = <Runtime as eth_bridge::Config>::GetEthNetworkId::get();
        let network_id = GenericNetworkId::EVMLegacy(evm_net_id);
        let baseline_locked = crate::BridgeProxy::locked_assets(network_id, asset_id);
        let sidechain_tx_hash = H256::repeat_byte(0x72);

        let load_incoming_request = eth_bridge::requests::LoadIncomingRequest::Transaction(
            eth_bridge::requests::LoadIncomingTransactionRequest::new(
                owner.clone(),
                sidechain_tx_hash,
                Default::default(),
                eth_bridge::requests::IncomingTransactionRequestKind::Transfer,
                evm_net_id,
            ),
        );
        let incoming_request = eth_bridge::requests::IncomingRequest::Transfer(
            eth_bridge::requests::IncomingTransfer::<Runtime> {
                from: H160::repeat_byte(0x45),
                to: owner.clone(),
                asset_id,
                asset_kind: eth_bridge::requests::AssetKind::Sidechain,
                amount: 1u32.into(),
                author: owner,
                tx_hash: sidechain_tx_hash,
                at_height: 1,
                timepoint: Default::default(),
                network_id: evm_net_id,
                should_take_fee: false,
            },
        );
        let load_hash = sidechain_tx_hash;
        let incoming_offchain_request =
            eth_bridge::requests::OffchainRequest::incoming(incoming_request.clone());
        let incoming_hash = match &incoming_offchain_request {
            eth_bridge::requests::OffchainRequest::Incoming(_, hash) => *hash,
            _ => unreachable!(),
        };

        let bridge_account =
            crate::EthBridge::bridge_account(evm_net_id).expect("bridge account must exist");
        assert_ok!(crate::EthBridge::import_incoming_request(
            RuntimeOrigin::signed(bridge_account),
            load_incoming_request,
            Ok(incoming_request),
        ));

        assert_eq!(
            crate::EthBridge::request_status(evm_net_id, load_hash),
            Some(eth_bridge::requests::RequestStatus::Failed(
                eth_bridge::Error::<Runtime>::SccpAssetNotAllowed.into()
            ))
        );
        assert_eq!(
            crate::EthBridge::request_status(evm_net_id, incoming_hash),
            Some(eth_bridge::requests::RequestStatus::Failed(
                eth_bridge::Error::<Runtime>::SccpAssetNotAllowed.into()
            ))
        );
        assert!(!eth_bridge::RequestsQueue::<Runtime>::get(evm_net_id).contains(&load_hash));
        assert!(eth_bridge::Requests::<Runtime>::get(evm_net_id, incoming_hash).is_none());
        assert_eq!(
            eth_bridge::LoadToIncomingRequestHash::<Runtime>::get(evm_net_id, load_hash),
            H256::zero()
        );
        assert_eq!(
            crate::BridgeProxy::locked_assets(network_id, asset_id),
            baseline_locked
        );
    });
}

#[test]
fn sccp_import_incoming_network_mismatch_aborts_load_request_in_runtime() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        let evm_net_id = <Runtime as eth_bridge::Config>::GetEthNetworkId::get();
        let sidechain_tx_hash = H256::repeat_byte(0x73);

        let load_incoming_request = eth_bridge::requests::LoadIncomingRequest::Transaction(
            eth_bridge::requests::LoadIncomingTransactionRequest::new(
                owner.clone(),
                sidechain_tx_hash,
                Default::default(),
                eth_bridge::requests::IncomingTransactionRequestKind::Transfer,
                evm_net_id,
            ),
        );
        let incoming_request = eth_bridge::requests::IncomingRequest::Transfer(
            eth_bridge::requests::IncomingTransfer::<Runtime> {
                from: H160::repeat_byte(0x46),
                to: owner,
                asset_id: common::XOR.into(),
                asset_kind: eth_bridge::requests::AssetKind::Thischain,
                amount: 1u32.into(),
                author: common::mock::bob(),
                tx_hash: sidechain_tx_hash,
                at_height: 1,
                timepoint: Default::default(),
                network_id: evm_net_id.saturating_add(1),
                should_take_fee: false,
            },
        );
        let load_hash = sidechain_tx_hash;
        let incoming_offchain_request =
            eth_bridge::requests::OffchainRequest::incoming(incoming_request.clone());
        let incoming_hash = match &incoming_offchain_request {
            eth_bridge::requests::OffchainRequest::Incoming(_, hash) => *hash,
            _ => unreachable!(),
        };

        let bridge_account =
            crate::EthBridge::bridge_account(evm_net_id).expect("bridge account must exist");
        assert_ok!(crate::EthBridge::import_incoming_request(
            RuntimeOrigin::signed(bridge_account),
            load_incoming_request,
            Ok(incoming_request),
        ));

        assert_eq!(
            crate::EthBridge::request_status(evm_net_id, load_hash),
            Some(eth_bridge::requests::RequestStatus::Failed(
                eth_bridge::Error::<Runtime>::UnknownNetwork.into()
            ))
        );
        assert!(eth_bridge::Requests::<Runtime>::get(evm_net_id, incoming_hash).is_none());
        assert_eq!(
            eth_bridge::LoadToIncomingRequestHash::<Runtime>::get(evm_net_id, load_hash),
            H256::zero()
        );
        assert!(!eth_bridge::RequestsQueue::<Runtime>::get(evm_net_id).contains(&load_hash));
    });
}

#[test]
fn sccp_asset_blocks_bridge_proxy_manage_asset_in_runtime() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"SCCPBM".to_vec()),
            AssetName(b"SCCP BridgeProxy Manage".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u128,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();

        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        let evm_net_id = <Runtime as eth_bridge::Config>::GetEthNetworkId::get();
        assert_noop!(
            crate::BridgeProxy::manage_asset(GenericNetworkId::EVMLegacy(evm_net_id), asset_id),
            bridge_proxy::Error::<Runtime>::PathIsNotAvailable
        );
    });
}

#[test]
fn sccp_asset_blocks_eth_bridge_register_existing_sidechain_asset_in_runtime() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"SCCPER".to_vec()),
            AssetName(b"SCCP Eth Register".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u128,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();

        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        let evm_net_id = <Runtime as eth_bridge::Config>::GetEthNetworkId::get();
        assert_noop!(
            crate::EthBridge::register_existing_sidechain_asset(
                RuntimeOrigin::root(),
                asset_id,
                H160::repeat_byte(0x99),
                evm_net_id,
            ),
            eth_bridge::Error::<Runtime>::SccpAssetNotAllowed
        );
    });
}

#[test]
fn sccp_asset_blocks_bridge_proxy_refund_in_runtime() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"SCCPRF".to_vec()),
            AssetName(b"SCCP BridgeProxy Refund".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u128,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();

        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        let evm_net_id = <Runtime as eth_bridge::Config>::GetEthNetworkId::get();
        assert_noop!(
            crate::BridgeProxy::refund(
                GenericNetworkId::EVMLegacy(evm_net_id),
                H256::repeat_byte(0x42),
                GenericAccount::Sora(owner.clone().into()),
                asset_id,
                1u32.into(),
            ),
            bridge_proxy::Error::<Runtime>::PathIsNotAvailable
        );
    });
}

#[test]
fn sccp_asset_blocks_bridge_proxy_lock_unlock_and_fee_paths_in_runtime() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"SCCPLF".to_vec()),
            AssetName(b"SCCP BridgeProxy Lock Fee".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u128,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();

        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        let network_id =
            GenericNetworkId::EVMLegacy(<Runtime as eth_bridge::Config>::GetEthNetworkId::get());
        let amount = 1u32.into();

        assert_noop!(
            crate::BridgeProxy::lock_asset(
                network_id,
                bridge_types::types::AssetKind::Thischain,
                &owner,
                &asset_id,
                &amount,
            ),
            bridge_proxy::Error::<Runtime>::PathIsNotAvailable
        );
        assert_noop!(
            crate::BridgeProxy::unlock_asset(
                network_id,
                bridge_types::types::AssetKind::Thischain,
                &owner,
                &asset_id,
                &amount,
            ),
            bridge_proxy::Error::<Runtime>::PathIsNotAvailable
        );
        assert_noop!(
            crate::BridgeProxy::withdraw_fee(network_id, &owner, &asset_id, &amount),
            bridge_proxy::Error::<Runtime>::PathIsNotAvailable
        );
        assert_noop!(
            crate::BridgeProxy::refund_fee(network_id, &owner, &asset_id, &amount),
            bridge_proxy::Error::<Runtime>::PathIsNotAvailable
        );
        assert_noop!(
            crate::BridgeProxy::withdraw_transfer_fee(&owner, H256::zero(), asset_id,),
            bridge_proxy::Error::<Runtime>::PathIsNotAvailable
        );
    });
}
