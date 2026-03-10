// This file is part of the SORA network and Polkaswap app.
//
// Copyright (c) 2026, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

use crate::mock::*;
use crate::{
    BurnPayloadV1, Error, EvmBurnProofV1, InboundFinalityMode, SCCP_CORE_REMOTE_DOMAINS,
    SCCP_DIGEST_NETWORK_ID, SCCP_DOMAIN_BSC, SCCP_DOMAIN_ETH, SCCP_DOMAIN_SOL, SCCP_DOMAIN_SORA,
    SCCP_DOMAIN_SORA_KUSAMA, SCCP_DOMAIN_SORA_POLKADOT, SCCP_DOMAIN_TON, SCCP_DOMAIN_TRON,
    SCCP_EVM_BURNS_MAPPING_SLOT, SCCP_MSG_PREFIX_ATTEST_V1, SCCP_MSG_PREFIX_BURN_V1,
};
use bridge_types::{types::AuxiliaryDigestItem, GenericNetworkId, SubNetworkId};
use codec::{Decode, Encode};
use common::{
    prelude::Balance, AssetInfoProvider, AssetName, AssetSymbol, DEFAULT_BALANCE_PRECISION,
};
use frame_support::traits::ConstU32;
use frame_support::{assert_noop, assert_ok};
use sp_core::H256;
use sp_io::hashing::keccak_256;
use sp_runtime::BoundedVec;
use sp_runtime::DispatchError;

fn register_mintable_asset(asset_id: AssetId) {
    assert_ok!(assets::Pallet::<Runtime>::register_asset_id(
        alice(),
        asset_id,
        AssetSymbol(b"TST".to_vec()),
        AssetName(b"Test".to_vec()),
        DEFAULT_BALANCE_PRECISION,
        0u32.into(),
        true,
        common::AssetType::Regular,
        None,
        None,
    ));
}

fn set_default_remote_tokens(asset_id: AssetId) {
    // EVM-like: 20 bytes
    assert_ok!(Sccp::set_remote_token(
        RuntimeOrigin::root(),
        asset_id,
        SCCP_DOMAIN_ETH,
        vec![1u8; 20],
    ));
    assert_ok!(Sccp::set_remote_token(
        RuntimeOrigin::root(),
        asset_id,
        SCCP_DOMAIN_BSC,
        vec![2u8; 20],
    ));
    assert_ok!(Sccp::set_remote_token(
        RuntimeOrigin::root(),
        asset_id,
        SCCP_DOMAIN_TRON,
        vec![3u8; 20],
    ));
    // Solana/TON: 32 bytes
    assert_ok!(Sccp::set_remote_token(
        RuntimeOrigin::root(),
        asset_id,
        SCCP_DOMAIN_SOL,
        vec![4u8; 32],
    ));
    assert_ok!(Sccp::set_remote_token(
        RuntimeOrigin::root(),
        asset_id,
        SCCP_DOMAIN_TON,
        vec![5u8; 32],
    ));
    // SORA parachains: 32-byte identifiers
    assert_ok!(Sccp::set_remote_token(
        RuntimeOrigin::root(),
        asset_id,
        SCCP_DOMAIN_SORA_KUSAMA,
        vec![6u8; 32],
    ));
    assert_ok!(Sccp::set_remote_token(
        RuntimeOrigin::root(),
        asset_id,
        SCCP_DOMAIN_SORA_POLKADOT,
        vec![7u8; 32],
    ));
}

fn set_default_domain_endpoints() {
    // EVM-like: 20 bytes
    assert_ok!(Sccp::set_domain_endpoint(
        RuntimeOrigin::root(),
        SCCP_DOMAIN_ETH,
        vec![11u8; 20],
    ));
    assert_ok!(Sccp::set_domain_endpoint(
        RuntimeOrigin::root(),
        SCCP_DOMAIN_BSC,
        vec![12u8; 20],
    ));
    assert_ok!(Sccp::set_domain_endpoint(
        RuntimeOrigin::root(),
        SCCP_DOMAIN_TRON,
        vec![13u8; 20],
    ));
    // Solana/TON: 32 bytes
    assert_ok!(Sccp::set_domain_endpoint(
        RuntimeOrigin::root(),
        SCCP_DOMAIN_SOL,
        vec![14u8; 32],
    ));
    assert_ok!(Sccp::set_domain_endpoint(
        RuntimeOrigin::root(),
        SCCP_DOMAIN_TON,
        vec![15u8; 32],
    ));
    assert_ok!(Sccp::set_domain_endpoint(
        RuntimeOrigin::root(),
        SCCP_DOMAIN_SORA_KUSAMA,
        vec![16u8; 32],
    ));
    assert_ok!(Sccp::set_domain_endpoint(
        RuntimeOrigin::root(),
        SCCP_DOMAIN_SORA_POLKADOT,
        vec![17u8; 32],
    ));
}

fn set_dummy_evm_anchor(domain_id: u32) {
    assert_ok!(Sccp::set_inbound_finality_mode(
        RuntimeOrigin::root(),
        domain_id,
        InboundFinalityMode::EvmAnchor
    ));
    assert_ok!(Sccp::set_evm_anchor_mode_enabled(
        RuntimeOrigin::root(),
        domain_id,
        true
    ));
    assert_ok!(Sccp::set_evm_inbound_anchor(
        RuntimeOrigin::root(),
        domain_id,
        1,
        H256([1u8; 32]),
        H256([2u8; 32]),
    ));
}

fn setup_active_token(asset_id: AssetId) {
    register_mintable_asset(asset_id);
    assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
    set_default_domain_endpoints();
    set_default_remote_tokens(asset_id);
    assert_ok!(Sccp::activate_token(RuntimeOrigin::root(), asset_id));
}

fn burn_message_id_for_test(payload: &BurnPayloadV1) -> H256 {
    let mut preimage = SCCP_MSG_PREFIX_BURN_V1.to_vec();
    preimage.extend(payload.encode());
    H256::from_slice(&keccak_256(&preimage))
}

fn attest_hash_for_test(message_id: &H256) -> H256 {
    let mut preimage = SCCP_MSG_PREFIX_ATTEST_V1.to_vec();
    preimage.extend_from_slice(&message_id.0);
    H256::from_slice(&keccak_256(&preimage))
}

#[test]
fn formal_assisted_burn_payload_encoding_is_fixed_width_and_roundtrips_at_boundaries() {
    let min_payload = BurnPayloadV1 {
        version: 0,
        source_domain: 0,
        dest_domain: 0,
        nonce: 0,
        sora_asset_id: [0u8; 32],
        amount: 0u128 as Balance,
        recipient: [0u8; 32],
    };
    let max_payload = BurnPayloadV1 {
        version: u8::MAX,
        source_domain: u32::MAX,
        dest_domain: u32::MAX,
        nonce: u64::MAX,
        sora_asset_id: [0xffu8; 32],
        amount: Balance::MAX,
        recipient: [0xffu8; 32],
    };

    for payload in [min_payload, max_payload] {
        let encoded = payload.encode();
        assert_eq!(encoded.len(), 97);
        assert_eq!(encoded[0], payload.version);
        assert_eq!(&encoded[1..5], &payload.source_domain.to_le_bytes());
        assert_eq!(&encoded[5..9], &payload.dest_domain.to_le_bytes());
        assert_eq!(&encoded[9..17], &payload.nonce.to_le_bytes());
        assert_eq!(&encoded[17..49], &payload.sora_asset_id);
        assert_eq!(&encoded[49..65], &payload.amount.to_le_bytes());
        assert_eq!(&encoded[65..97], &payload.recipient);

        let decoded =
            BurnPayloadV1::decode(&mut encoded.as_slice()).expect("encoded payload should decode");
        assert_eq!(decoded, payload);
    }
}

#[test]
fn formal_assisted_burn_message_id_changes_with_nonce_and_recipient() {
    let payload = BurnPayloadV1 {
        version: 1,
        source_domain: SCCP_DOMAIN_ETH,
        dest_domain: SCCP_DOMAIN_SOL,
        nonce: 42,
        sora_asset_id: [9u8; 32],
        amount: 123_456_789u128 as Balance,
        recipient: [7u8; 32],
    };

    let mut nonce_changed = payload.clone();
    nonce_changed.nonce = nonce_changed.nonce.saturating_add(1);

    let mut recipient_changed = payload.clone();
    recipient_changed.recipient[31] ^= 0x01;

    let base_id = burn_message_id_for_test(&payload);
    let nonce_id = burn_message_id_for_test(&nonce_changed);
    let recipient_id = burn_message_id_for_test(&recipient_changed);

    assert_ne!(base_id, nonce_id);
    assert_ne!(base_id, recipient_id);
    assert_ne!(nonce_id, recipient_id);
}

#[test]
fn formal_assisted_burn_message_id_is_stable_and_sensitive_to_amount_and_domains() {
    let payload = BurnPayloadV1 {
        version: 1,
        source_domain: SCCP_DOMAIN_ETH,
        dest_domain: SCCP_DOMAIN_SOL,
        nonce: 73,
        sora_asset_id: [0xabu8; 32],
        amount: 100u128 as Balance,
        recipient: [0xcdu8; 32],
    };

    let base_a = burn_message_id_for_test(&payload);
    let base_b = burn_message_id_for_test(&payload);
    assert_eq!(base_a, base_b);

    let mut amount_changed = payload.clone();
    amount_changed.amount = amount_changed.amount.saturating_add(1);

    let mut source_changed = payload.clone();
    source_changed.source_domain = SCCP_DOMAIN_BSC;

    let mut dest_changed = payload.clone();
    dest_changed.dest_domain = SCCP_DOMAIN_TON;

    let amount_id = burn_message_id_for_test(&amount_changed);
    let source_id = burn_message_id_for_test(&source_changed);
    let dest_id = burn_message_id_for_test(&dest_changed);

    assert_ne!(base_a, amount_id);
    assert_ne!(base_a, source_id);
    assert_ne!(base_a, dest_id);
    assert_ne!(amount_id, source_id);
    assert_ne!(amount_id, dest_id);
    assert_ne!(source_id, dest_id);
}

#[test]
fn formal_assisted_burn_payload_decode_fails_closed_on_truncated_bytes() {
    let payload = BurnPayloadV1 {
        version: 1,
        source_domain: SCCP_DOMAIN_TRON,
        dest_domain: SCCP_DOMAIN_SORA,
        nonce: 555,
        sora_asset_id: [0x11u8; 32],
        amount: 777u128 as Balance,
        recipient: [0x22u8; 32],
    };
    let encoded = payload.encode();
    assert_eq!(encoded.len(), 97);

    for len in 0..encoded.len() {
        let res = BurnPayloadV1::decode(&mut &encoded[..len]);
        assert!(
            res.is_err(),
            "decode must fail for truncated payload length {}",
            len
        );
    }

    let decoded =
        BurnPayloadV1::decode(&mut encoded.as_slice()).expect("exact-length payload should decode");
    assert_eq!(decoded, payload);
}

#[test]
fn formal_assisted_burn_message_id_is_domain_separated_from_plain_payload_hash() {
    let payload = BurnPayloadV1 {
        version: 1,
        source_domain: SCCP_DOMAIN_BSC,
        dest_domain: SCCP_DOMAIN_SORA,
        nonce: 888,
        sora_asset_id: [0x31u8; 32],
        amount: 7_000u128 as Balance,
        recipient: [0x42u8; 32],
    };

    let prefixed = burn_message_id_for_test(&payload);
    let plain = H256::from_slice(&keccak_256(&payload.encode()));
    assert_ne!(prefixed, plain);

    let mut manual_prefixed_preimage = SCCP_MSG_PREFIX_BURN_V1.to_vec();
    manual_prefixed_preimage.extend(payload.encode());
    let manual_prefixed = H256::from_slice(&keccak_256(&manual_prefixed_preimage));
    assert_eq!(prefixed, manual_prefixed);
}

#[test]
fn formal_assisted_burn_message_id_is_unique_for_bounded_nonce_window() {
    let mut seen = Vec::<H256>::new();
    for nonce in 0u64..128u64 {
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SOL,
            nonce,
            sora_asset_id: [0x77u8; 32],
            amount: 555u128 as Balance,
            recipient: [0x88u8; 32],
        };
        let id = burn_message_id_for_test(&payload);
        assert!(
            !seen.contains(&id),
            "message id collision within bounded nonce window at nonce {}",
            nonce
        );
        seen.push(id);
    }
    assert_eq!(seen.len(), 128);
}

#[test]
fn formal_assisted_attest_hash_is_stable_and_sensitive_to_message_id_changes() {
    let base_payload = BurnPayloadV1 {
        version: 1,
        source_domain: SCCP_DOMAIN_ETH,
        dest_domain: SCCP_DOMAIN_SORA,
        nonce: 99,
        sora_asset_id: [0x17u8; 32],
        amount: 321u128 as Balance,
        recipient: [0x18u8; 32],
    };

    let mut changed_payload = base_payload.clone();
    changed_payload.recipient[0] ^= 0x01;

    let base_msg = burn_message_id_for_test(&base_payload);
    let changed_msg = burn_message_id_for_test(&changed_payload);
    assert_ne!(base_msg, changed_msg);

    let base_attest_a = attest_hash_for_test(&base_msg);
    let base_attest_b = attest_hash_for_test(&base_msg);
    let changed_attest = attest_hash_for_test(&changed_msg);

    assert_eq!(base_attest_a, base_attest_b);
    assert_ne!(base_attest_a, changed_attest);
}

#[test]
fn formal_assisted_attest_hash_is_unique_for_bounded_nonce_window() {
    let mut seen = Vec::<H256>::new();
    for nonce in 0u64..128u64 {
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_BSC,
            dest_domain: SCCP_DOMAIN_TON,
            nonce,
            sora_asset_id: [0x19u8; 32],
            amount: 777u128 as Balance,
            recipient: [0x20u8; 32],
        };
        let message_id = burn_message_id_for_test(&payload);
        let attest_hash = attest_hash_for_test(&message_id);
        assert!(
            !seen.contains(&attest_hash),
            "attest hash collision within bounded nonce window at nonce {}",
            nonce
        );
        seen.push(attest_hash);
    }
    assert_eq!(seen.len(), 128);
}

#[test]
fn formal_assisted_burn_message_id_is_unique_for_bounded_recipient_window() {
    let mut seen = Vec::<H256>::new();
    for i in 0u8..128u8 {
        let mut recipient = [0u8; 32];
        recipient[31] = i;
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_TRON,
            dest_domain: SCCP_DOMAIN_SOL,
            nonce: 404,
            sora_asset_id: [0x21u8; 32],
            amount: 1_000u128 as Balance,
            recipient,
        };
        let id = burn_message_id_for_test(&payload);
        assert!(
            !seen.contains(&id),
            "message id collision within bounded recipient window at index {}",
            i
        );
        seen.push(id);
    }
    assert_eq!(seen.len(), 128);
}

#[test]
fn formal_assisted_burn_message_id_is_unique_for_bounded_amount_window() {
    let mut seen = Vec::<H256>::new();
    for amount in 0u128..128u128 {
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_TON,
            nonce: 909,
            sora_asset_id: [0x24u8; 32],
            amount: amount as Balance,
            recipient: [0x25u8; 32],
        };
        let id = burn_message_id_for_test(&payload);
        assert!(
            !seen.contains(&id),
            "message id collision within bounded amount window at amount {}",
            amount
        );
        seen.push(id);
    }
    assert_eq!(seen.len(), 128);
}

#[test]
fn formal_assisted_attest_hash_is_unique_for_bounded_amount_window() {
    let mut seen = Vec::<H256>::new();
    for amount in 0u128..128u128 {
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_BSC,
            nonce: 1_010,
            sora_asset_id: [0x26u8; 32],
            amount: amount as Balance,
            recipient: [0x27u8; 32],
        };
        let message_id = burn_message_id_for_test(&payload);
        let attested = attest_hash_for_test(&message_id);
        assert!(
            !seen.contains(&attested),
            "attest hash collision within bounded amount window at amount {}",
            amount
        );
        seen.push(attested);
    }
    assert_eq!(seen.len(), 128);
}

#[test]
fn formal_assisted_attest_hash_is_unique_for_bounded_source_window() {
    let mut seen = Vec::<H256>::new();
    for source_domain in 0u32..128u32 {
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain,
            dest_domain: SCCP_DOMAIN_SOL,
            nonce: 1_011,
            sora_asset_id: [0x28u8; 32],
            amount: 5_000u128 as Balance,
            recipient: [0x29u8; 32],
        };
        let message_id = burn_message_id_for_test(&payload);
        let attested = attest_hash_for_test(&message_id);
        assert!(
            !seen.contains(&attested),
            "attest hash collision within bounded source window at source {}",
            source_domain
        );
        seen.push(attested);
    }
    assert_eq!(seen.len(), 128);
}

#[test]
fn formal_assisted_burn_message_id_is_unique_for_bounded_source_window() {
    let mut seen = Vec::<H256>::new();
    for source_domain in 0u32..128u32 {
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain,
            dest_domain: SCCP_DOMAIN_SOL,
            nonce: 1_012,
            sora_asset_id: [0x2au8; 32],
            amount: 6_000u128 as Balance,
            recipient: [0x2bu8; 32],
        };
        let id = burn_message_id_for_test(&payload);
        assert!(
            !seen.contains(&id),
            "message id collision within bounded source window at source {}",
            source_domain
        );
        seen.push(id);
    }
    assert_eq!(seen.len(), 128);
}

#[test]
fn formal_assisted_burn_message_id_is_unique_for_bounded_destination_window() {
    let mut seen = Vec::<H256>::new();
    for dest_domain in 0u32..128u32 {
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain,
            nonce: 1_013,
            sora_asset_id: [0x2cu8; 32],
            amount: 7_000u128 as Balance,
            recipient: [0x2du8; 32],
        };
        let id = burn_message_id_for_test(&payload);
        assert!(
            !seen.contains(&id),
            "message id collision within bounded destination window at destination {}",
            dest_domain
        );
        seen.push(id);
    }
    assert_eq!(seen.len(), 128);
}

#[test]
fn formal_assisted_burn_message_id_is_unique_for_bounded_version_window() {
    let mut seen = Vec::<H256>::new();
    for version in 0u8..128u8 {
        let payload = BurnPayloadV1 {
            version,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SOL,
            nonce: 1_014,
            sora_asset_id: [0x2eu8; 32],
            amount: 8_000u128 as Balance,
            recipient: [0x2fu8; 32],
        };
        let id = burn_message_id_for_test(&payload);
        assert!(
            !seen.contains(&id),
            "message id collision within bounded version window at version {}",
            version
        );
        seen.push(id);
    }
    assert_eq!(seen.len(), 128);
}

#[test]
fn formal_assisted_burn_message_id_is_unique_for_bounded_asset_id_window() {
    let mut seen = Vec::<H256>::new();
    for i in 0u8..128u8 {
        let mut sora_asset_id = [0u8; 32];
        sora_asset_id[31] = i;
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SOL,
            nonce: 1_018,
            sora_asset_id,
            amount: 12_000u128 as Balance,
            recipient: [0x35u8; 32],
        };
        let id = burn_message_id_for_test(&payload);
        assert!(
            !seen.contains(&id),
            "message id collision within bounded asset-id window at index {}",
            i
        );
        seen.push(id);
    }
    assert_eq!(seen.len(), 128);
}

#[test]
fn formal_assisted_burn_message_id_is_unique_for_bounded_source_destination_matrix() {
    let mut seen = Vec::<H256>::new();
    for source_domain in 0u32..8u32 {
        for dest_domain in 0u32..16u32 {
            let payload = BurnPayloadV1 {
                version: 1,
                source_domain,
                dest_domain,
                nonce: 1_019,
                sora_asset_id: [0x36u8; 32],
                amount: 13_000u128 as Balance,
                recipient: [0x37u8; 32],
            };
            let id = burn_message_id_for_test(&payload);
            assert!(
                !seen.contains(&id),
                "message id collision in source/destination matrix at {}->{}",
                source_domain,
                dest_domain
            );
            seen.push(id);
        }
    }
    assert_eq!(seen.len(), 128);
}

#[test]
fn formal_assisted_attest_hash_is_unique_for_bounded_destination_window() {
    let mut seen = Vec::<H256>::new();
    for dest_domain in 0u32..128u32 {
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_BSC,
            dest_domain,
            nonce: 1_015,
            sora_asset_id: [0x30u8; 32],
            amount: 9_000u128 as Balance,
            recipient: [0x31u8; 32],
        };
        let message_id = burn_message_id_for_test(&payload);
        let attested = attest_hash_for_test(&message_id);
        assert!(
            !seen.contains(&attested),
            "attest hash collision within bounded destination window at destination {}",
            dest_domain
        );
        seen.push(attested);
    }
    assert_eq!(seen.len(), 128);
}

#[test]
fn formal_assisted_attest_hash_is_unique_for_bounded_version_window() {
    let mut seen = Vec::<H256>::new();
    for version in 0u8..128u8 {
        let payload = BurnPayloadV1 {
            version,
            source_domain: SCCP_DOMAIN_BSC,
            dest_domain: SCCP_DOMAIN_TON,
            nonce: 1_020,
            sora_asset_id: [0x38u8; 32],
            amount: 14_000u128 as Balance,
            recipient: [0x39u8; 32],
        };
        let message_id = burn_message_id_for_test(&payload);
        let attested = attest_hash_for_test(&message_id);
        assert!(
            !seen.contains(&attested),
            "attest hash collision within bounded version window at version {}",
            version
        );
        seen.push(attested);
    }
    assert_eq!(seen.len(), 128);
}

#[test]
fn formal_assisted_attest_hash_is_unique_for_bounded_asset_id_window() {
    let mut seen = Vec::<H256>::new();
    for i in 0u8..128u8 {
        let mut sora_asset_id = [0u8; 32];
        sora_asset_id[31] = i;
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_BSC,
            dest_domain: SCCP_DOMAIN_TON,
            nonce: 1_021,
            sora_asset_id,
            amount: 15_000u128 as Balance,
            recipient: [0x3au8; 32],
        };
        let message_id = burn_message_id_for_test(&payload);
        let attested = attest_hash_for_test(&message_id);
        assert!(
            !seen.contains(&attested),
            "attest hash collision within bounded asset-id window at index {}",
            i
        );
        seen.push(attested);
    }
    assert_eq!(seen.len(), 128);
}

#[test]
fn formal_assisted_attest_hash_is_unique_for_bounded_source_destination_matrix() {
    let mut seen = Vec::<H256>::new();
    for source_domain in 0u32..8u32 {
        for dest_domain in 0u32..16u32 {
            let payload = BurnPayloadV1 {
                version: 1,
                source_domain,
                dest_domain,
                nonce: 1_022,
                sora_asset_id: [0x3bu8; 32],
                amount: 16_000u128 as Balance,
                recipient: [0x3cu8; 32],
            };
            let message_id = burn_message_id_for_test(&payload);
            let attested = attest_hash_for_test(&message_id);
            assert!(
                !seen.contains(&attested),
                "attest hash collision in source/destination matrix at {}->{}",
                source_domain,
                dest_domain
            );
            seen.push(attested);
        }
    }
    assert_eq!(seen.len(), 128);
}

#[test]
fn formal_assisted_burn_message_id_is_unique_for_bounded_nonce_amount_matrix() {
    let mut seen = Vec::<H256>::new();
    for nonce in 0u64..8u64 {
        for amount in 0u128..16u128 {
            let payload = BurnPayloadV1 {
                version: 1,
                source_domain: SCCP_DOMAIN_ETH,
                dest_domain: SCCP_DOMAIN_SOL,
                nonce,
                sora_asset_id: [0x3du8; 32],
                amount: amount as Balance,
                recipient: [0x3eu8; 32],
            };
            let id = burn_message_id_for_test(&payload);
            assert!(
                !seen.contains(&id),
                "message id collision in nonce/amount matrix at nonce={} amount={}",
                nonce,
                amount
            );
            seen.push(id);
        }
    }
    assert_eq!(seen.len(), 128);
}

#[test]
fn formal_assisted_attest_hash_is_unique_for_bounded_nonce_amount_matrix() {
    let mut seen = Vec::<H256>::new();
    for nonce in 0u64..8u64 {
        for amount in 0u128..16u128 {
            let payload = BurnPayloadV1 {
                version: 1,
                source_domain: SCCP_DOMAIN_BSC,
                dest_domain: SCCP_DOMAIN_TON,
                nonce,
                sora_asset_id: [0x3fu8; 32],
                amount: amount as Balance,
                recipient: [0x40u8; 32],
            };
            let message_id = burn_message_id_for_test(&payload);
            let attested = attest_hash_for_test(&message_id);
            assert!(
                !seen.contains(&attested),
                "attest hash collision in nonce/amount matrix at nonce={} amount={}",
                nonce,
                amount
            );
            seen.push(attested);
        }
    }
    assert_eq!(seen.len(), 128);
}

#[test]
fn formal_assisted_burn_message_id_is_unique_for_bounded_amount_recipient_matrix() {
    let mut seen = Vec::<H256>::new();
    for amount in 0u128..8u128 {
        for i in 0u8..16u8 {
            let mut recipient = [0u8; 32];
            recipient[31] = i;
            let payload = BurnPayloadV1 {
                version: 1,
                source_domain: SCCP_DOMAIN_ETH,
                dest_domain: SCCP_DOMAIN_SOL,
                nonce: 1_024,
                sora_asset_id: [0x43u8; 32],
                amount: amount as Balance,
                recipient,
            };
            let id = burn_message_id_for_test(&payload);
            assert!(
                !seen.contains(&id),
                "message id collision in amount/recipient matrix at amount={} recipient_index={}",
                amount,
                i
            );
            seen.push(id);
        }
    }
    assert_eq!(seen.len(), 128);
}

#[test]
fn formal_assisted_attest_hash_is_unique_for_bounded_amount_recipient_matrix() {
    let mut seen = Vec::<H256>::new();
    for amount in 0u128..8u128 {
        for i in 0u8..16u8 {
            let mut recipient = [0u8; 32];
            recipient[31] = i;
            let payload = BurnPayloadV1 {
                version: 1,
                source_domain: SCCP_DOMAIN_BSC,
                dest_domain: SCCP_DOMAIN_TON,
                nonce: 1_025,
                sora_asset_id: [0x44u8; 32],
                amount: amount as Balance,
                recipient,
            };
            let message_id = burn_message_id_for_test(&payload);
            let attested = attest_hash_for_test(&message_id);
            assert!(
                !seen.contains(&attested),
                "attest hash collision in amount/recipient matrix at amount={} recipient_index={}",
                amount,
                i
            );
            seen.push(attested);
        }
    }
    assert_eq!(seen.len(), 128);
}

#[test]
fn formal_assisted_burn_message_id_is_unique_for_bounded_nonce_asset_id_matrix() {
    let mut seen = Vec::<H256>::new();
    for nonce in 0u64..8u64 {
        for i in 0u8..16u8 {
            let mut sora_asset_id = [0u8; 32];
            sora_asset_id[31] = i;
            let payload = BurnPayloadV1 {
                version: 1,
                source_domain: SCCP_DOMAIN_ETH,
                dest_domain: SCCP_DOMAIN_SOL,
                nonce,
                sora_asset_id,
                amount: 18_000u128 as Balance,
                recipient: [0x45u8; 32],
            };
            let id = burn_message_id_for_test(&payload);
            assert!(
                !seen.contains(&id),
                "message id collision in nonce/asset-id matrix at nonce={} asset_index={}",
                nonce,
                i
            );
            seen.push(id);
        }
    }
    assert_eq!(seen.len(), 128);
}

#[test]
fn formal_assisted_attest_hash_is_unique_for_bounded_nonce_asset_id_matrix() {
    let mut seen = Vec::<H256>::new();
    for nonce in 0u64..8u64 {
        for i in 0u8..16u8 {
            let mut sora_asset_id = [0u8; 32];
            sora_asset_id[31] = i;
            let payload = BurnPayloadV1 {
                version: 1,
                source_domain: SCCP_DOMAIN_BSC,
                dest_domain: SCCP_DOMAIN_TON,
                nonce,
                sora_asset_id,
                amount: 19_000u128 as Balance,
                recipient: [0x46u8; 32],
            };
            let message_id = burn_message_id_for_test(&payload);
            let attested = attest_hash_for_test(&message_id);
            assert!(
                !seen.contains(&attested),
                "attest hash collision in nonce/asset-id matrix at nonce={} asset_index={}",
                nonce,
                i
            );
            seen.push(attested);
        }
    }
    assert_eq!(seen.len(), 128);
}

#[test]
fn formal_assisted_attest_hash_is_unique_for_bounded_recipient_window() {
    let mut seen = Vec::<H256>::new();
    for i in 0u8..128u8 {
        let mut recipient = [0u8; 32];
        recipient[31] = i;
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_TRON,
            dest_domain: SCCP_DOMAIN_SOL,
            nonce: 1_016,
            sora_asset_id: [0x32u8; 32],
            amount: 10_000u128 as Balance,
            recipient,
        };
        let message_id = burn_message_id_for_test(&payload);
        let attested = attest_hash_for_test(&message_id);
        assert!(
            !seen.contains(&attested),
            "attest hash collision within bounded recipient window at index {}",
            i
        );
        seen.push(attested);
    }
    assert_eq!(seen.len(), 128);
}

#[test]
fn formal_assisted_burn_message_id_is_direction_sensitive_for_swapped_domains() {
    let forward = BurnPayloadV1 {
        version: 1,
        source_domain: SCCP_DOMAIN_ETH,
        dest_domain: SCCP_DOMAIN_SOL,
        nonce: 1_017,
        sora_asset_id: [0x33u8; 32],
        amount: 11_000u128 as Balance,
        recipient: [0x34u8; 32],
    };
    let reverse = BurnPayloadV1 {
        source_domain: forward.dest_domain,
        dest_domain: forward.source_domain,
        ..forward.clone()
    };

    let forward_id = burn_message_id_for_test(&forward);
    let reverse_id = burn_message_id_for_test(&reverse);
    assert_ne!(forward_id, reverse_id);
}

#[test]
fn formal_assisted_attest_hash_is_direction_sensitive_for_swapped_domains() {
    let forward = BurnPayloadV1 {
        version: 1,
        source_domain: SCCP_DOMAIN_ETH,
        dest_domain: SCCP_DOMAIN_SOL,
        nonce: 1_023,
        sora_asset_id: [0x41u8; 32],
        amount: 17_000u128 as Balance,
        recipient: [0x42u8; 32],
    };
    let reverse = BurnPayloadV1 {
        source_domain: forward.dest_domain,
        dest_domain: forward.source_domain,
        ..forward.clone()
    };

    let forward_message_id = burn_message_id_for_test(&forward);
    let reverse_message_id = burn_message_id_for_test(&reverse);
    let forward_attest = attest_hash_for_test(&forward_message_id);
    let reverse_attest = attest_hash_for_test(&reverse_message_id);
    assert_ne!(forward_attest, reverse_attest);
}

#[test]
fn formal_assisted_attest_hash_is_domain_separated_from_plain_message_id_hash() {
    let payload = BurnPayloadV1 {
        version: 1,
        source_domain: SCCP_DOMAIN_BSC,
        dest_domain: SCCP_DOMAIN_ETH,
        nonce: 505,
        sora_asset_id: [0x22u8; 32],
        amount: 2_000u128 as Balance,
        recipient: [0x23u8; 32],
    };

    let message_id = burn_message_id_for_test(&payload);
    let attest_prefixed = attest_hash_for_test(&message_id);
    let plain_message_hash = H256::from_slice(&keccak_256(&message_id.0));
    assert_ne!(attest_prefixed, plain_message_hash);
}

#[test]
fn formal_assisted_prefix_literals_remain_stable() {
    assert_eq!(SCCP_MSG_PREFIX_BURN_V1, b"sccp:burn:v1");
    assert_eq!(SCCP_MSG_PREFIX_ATTEST_V1, b"sccp:attest:v1");
}

#[test]
fn formal_assisted_domain_separation_prefixes_produce_distinct_hashes() {
    assert_ne!(SCCP_MSG_PREFIX_BURN_V1, SCCP_MSG_PREFIX_ATTEST_V1);
    assert!(SCCP_MSG_PREFIX_BURN_V1.starts_with(b"sccp:"));
    assert!(SCCP_MSG_PREFIX_ATTEST_V1.starts_with(b"sccp:"));

    let payload = BurnPayloadV1 {
        version: 1,
        source_domain: SCCP_DOMAIN_BSC,
        dest_domain: SCCP_DOMAIN_TRON,
        nonce: 99,
        sora_asset_id: [3u8; 32],
        amount: 1_000u128 as Balance,
        recipient: [4u8; 32],
    };
    let message_id = burn_message_id_for_test(&payload);

    let mut burn_preimage = SCCP_MSG_PREFIX_BURN_V1.to_vec();
    burn_preimage.extend(payload.encode());
    let burn_hash = H256::from_slice(&keccak_256(&burn_preimage));

    let attest_hash = attest_hash_for_test(&message_id);

    assert_ne!(burn_hash, attest_hash);
}

#[test]
fn default_required_domains_should_include_all_core_domains_when_capacity_allows() {
    let domains: BoundedVec<u32, ConstU32<8>> =
        crate::default_required_domains_for_bound::<ConstU32<8>>();
    assert_eq!(domains.to_vec(), SCCP_CORE_REMOTE_DOMAINS.to_vec());
}

#[test]
fn default_required_domains_should_truncate_when_capacity_is_smaller_than_core_set() {
    let domains: BoundedVec<u32, ConstU32<2>> =
        crate::default_required_domains_for_bound::<ConstU32<2>>();
    assert_eq!(domains.to_vec(), vec![SCCP_DOMAIN_ETH, SCCP_DOMAIN_BSC]);
}

#[test]
fn default_required_domains_should_be_empty_when_capacity_is_zero() {
    let domains: BoundedVec<u32, ConstU32<0>> =
        crate::default_required_domains_for_bound::<ConstU32<0>>();
    assert!(domains.is_empty());
}

#[test]
fn genesis_build_canonicalizes_required_domains() {
    let mut ext = ExtBuilder::default()
        .with_required_domains(vec![
            SCCP_DOMAIN_TRON,
            SCCP_DOMAIN_SORA_POLKADOT,
            SCCP_DOMAIN_ETH,
            SCCP_DOMAIN_TON,
            SCCP_DOMAIN_SORA_KUSAMA,
            SCCP_DOMAIN_BSC,
            SCCP_DOMAIN_SOL,
        ])
        .build();

    ext.execute_with(|| {
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
fn genesis_build_rejects_invalid_required_domains() {
    let build_result = std::panic::catch_unwind(|| {
        ExtBuilder::default()
            .with_required_domains(vec![SCCP_DOMAIN_ETH])
            .build();
    });
    assert!(build_result.is_err());
}

fn last_sccp_event() -> crate::Event<Runtime> {
    System::events()
        .into_iter()
        .rev()
        .find_map(|r| match r.event {
            RuntimeEvent::Sccp(e) => Some(e),
            _ => None,
        })
        .expect("expected sccp event")
}

#[test]
fn add_activate_and_burn_creates_burn_record() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::RedPepper.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(assets::Pallet::<Runtime>::mint_to(
            &asset_id,
            &alice(),
            &alice(),
            1_000u32.into()
        ));

        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        set_default_domain_endpoints();
        set_default_remote_tokens(asset_id);
        assert_ok!(Sccp::activate_token(RuntimeOrigin::root(), asset_id));

        let amount: Balance = 100u32.into();
        // Canonical EVM recipient encoding: 20 bytes right-aligned in 32 bytes.
        let mut recipient = [0u8; 32];
        recipient[12..].copy_from_slice(&[7u8; 20]);
        assert_ok!(Sccp::burn(
            RuntimeOrigin::signed(alice()),
            asset_id,
            amount,
            SCCP_DOMAIN_ETH,
            recipient,
        ));

        // Balance reduced.
        assert_eq!(
            assets::Pallet::<Runtime>::free_balance(&asset_id, &alice()).unwrap(),
            900u32.into()
        );

        // Burn record stored under the expected message id.
        let asset_h256: H256 = asset_id.into();
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_ETH,
            nonce: 1,
            sora_asset_id: asset_h256.0,
            amount,
            recipient,
        };
        let mut preimage = SCCP_MSG_PREFIX_BURN_V1.to_vec();
        preimage.extend(payload.encode());
        let message_id = H256::from_slice(&keccak_256(&preimage));

        let rec = Sccp::burns(message_id).expect("burn record must exist");
        assert_eq!(rec.sender, alice());
        assert_eq!(rec.asset_id, asset_id);
        assert_eq!(rec.amount, amount);
        assert_eq!(rec.dest_domain, SCCP_DOMAIN_ETH);
        assert_eq!(rec.recipient, recipient);
        assert_eq!(rec.nonce, 1);

        // Burn is committed into auxiliary digest for BEEFY+MMR proofs to other chains.
        let digest_items = take_aux_digest_items();
        assert_eq!(
            digest_items,
            vec![AuxiliaryDigestItem::Commitment(
                SCCP_DIGEST_NETWORK_ID,
                message_id
            )]
        );

        // SCCP checker returns true for managed asset.
        assert!(Sccp::is_sccp_asset(&asset_id));
    });
}

#[test]
fn burn_to_evm_requires_canonical_recipient_encoding() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::BluePromise.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(assets::Pallet::<Runtime>::mint_to(
            &asset_id,
            &alice(),
            &alice(),
            1_000u32.into()
        ));

        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        set_default_domain_endpoints();
        set_default_remote_tokens(asset_id);
        assert_ok!(Sccp::activate_token(RuntimeOrigin::root(), asset_id));

        let amount: Balance = 1u32.into();

        // Non-canonical: non-zero high 12 bytes.
        let mut recipient = [0u8; 32];
        recipient[0] = 1;
        assert_noop!(
            Sccp::burn(
                RuntimeOrigin::signed(alice()),
                asset_id,
                amount,
                SCCP_DOMAIN_ETH,
                recipient,
            ),
            Error::<Runtime>::RecipientNotCanonical
        );
    });
}

#[test]
fn pause_outbound_domain_blocks_burn() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::Tomato.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(assets::Pallet::<Runtime>::mint_to(
            &asset_id,
            &alice(),
            &alice(),
            1_000u32.into()
        ));

        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        set_default_domain_endpoints();
        set_default_remote_tokens(asset_id);
        assert_ok!(Sccp::activate_token(RuntimeOrigin::root(), asset_id));

        assert_ok!(Sccp::set_outbound_domain_paused(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            true
        ));

        let amount: Balance = 1u32.into();
        let mut recipient = [0u8; 32];
        recipient[12..].copy_from_slice(&[7u8; 20]);
        assert_noop!(
            Sccp::burn(
                RuntimeOrigin::signed(alice()),
                asset_id,
                amount,
                SCCP_DOMAIN_ETH,
                recipient,
            ),
            Error::<Runtime>::OutboundDomainPaused
        );

        // Balance unchanged.
        assert_eq!(
            assets::Pallet::<Runtime>::free_balance(&asset_id, &alice()).unwrap(),
            1_000u32.into()
        );
    });
}

#[test]
fn add_token_fails_if_asset_is_on_legacy_bridge() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::Future.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        set_legacy_bridge_asset(asset_id, true);

        assert_noop!(
            Sccp::add_token(RuntimeOrigin::root(), asset_id),
            Error::<Runtime>::AssetOnLegacyBridge
        );
    });
}

#[test]
fn activate_requires_remote_tokens_for_required_domains() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::Apple.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));

        // No remote tokens set => activation must fail.
        assert_noop!(
            Sccp::activate_token(RuntimeOrigin::root(), asset_id),
            Error::<Runtime>::RemoteTokenMissing
        );
    });
}

#[test]
fn activate_requires_remote_tokens_for_all_core_domains_with_partial_configuration() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::Future.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        set_default_domain_endpoints();

        // Only ETH remote token is configured; core-domain activation requirements must still fail.
        assert_ok!(Sccp::set_remote_token(
            RuntimeOrigin::root(),
            asset_id,
            SCCP_DOMAIN_ETH,
            vec![1u8; 20],
        ));
        assert_noop!(
            Sccp::activate_token(RuntimeOrigin::root(), asset_id),
            Error::<Runtime>::RemoteTokenMissing
        );
    });
}

#[test]
fn activate_requires_domain_endpoints_for_required_domains() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::Table.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));

        // Remote tokens are configured, but endpoints are not => activation must fail.
        set_default_remote_tokens(asset_id);
        assert_noop!(
            Sccp::activate_token(RuntimeOrigin::root(), asset_id),
            Error::<Runtime>::DomainEndpointMissing
        );
    });
}

#[test]
fn set_required_domains_rejects_duplicates() {
    let mut ext = ExtBuilder::default().build();

    ext.execute_with(|| {
        let initial = Sccp::required_domains().into_inner();
        assert_noop!(
            Sccp::set_required_domains(
                RuntimeOrigin::root(),
                vec![SCCP_DOMAIN_ETH, SCCP_DOMAIN_ETH],
            ),
            Error::<Runtime>::RequiredDomainsInvalid
        );
        assert_eq!(Sccp::required_domains().into_inner(), initial);
    });
}

#[test]
fn set_required_domains_rejects_non_manager_origin() {
    let mut ext = ExtBuilder::default().build();

    ext.execute_with(|| {
        let initial = Sccp::required_domains().into_inner();
        assert_noop!(
            Sccp::set_required_domains(
                RuntimeOrigin::signed(alice()),
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
fn set_required_domains_failure_does_not_emit_event() {
    let mut ext = ExtBuilder::default().build();

    ext.execute_with(|| {
        System::set_block_number(1);
        let initial = Sccp::required_domains().into_inner();
        let events_before = System::events().len();
        assert_noop!(
            Sccp::set_required_domains(
                RuntimeOrigin::signed(alice()),
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
        assert_eq!(System::events().len(), events_before);
    });
}

#[test]
fn activate_requires_domain_endpoints_for_all_core_domains_with_partial_configuration() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::Future.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        set_default_remote_tokens(asset_id);

        // Only ETH endpoint is configured; core-domain activation requirements must still fail.
        assert_ok!(Sccp::set_domain_endpoint(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            vec![11u8; 20],
        ));
        assert_noop!(
            Sccp::activate_token(RuntimeOrigin::root(), asset_id),
            Error::<Runtime>::DomainEndpointMissing
        );
    });
}

#[test]
fn set_required_domains_rejects_missing_core_domains() {
    let mut ext = ExtBuilder::default().build();

    ext.execute_with(|| {
        let initial = Sccp::required_domains().into_inner();
        assert_noop!(
            Sccp::set_required_domains(RuntimeOrigin::root(), vec![SCCP_DOMAIN_ETH]),
            Error::<Runtime>::RequiredDomainsInvalid
        );
        assert_eq!(Sccp::required_domains().into_inner(), initial);
    });
}

#[test]
fn set_required_domains_rejects_sora_domain() {
    let mut ext = ExtBuilder::default().build();

    ext.execute_with(|| {
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
            Error::<Runtime>::DomainUnsupported
        );
        assert_eq!(Sccp::required_domains().into_inner(), initial);
    });
}

#[test]
fn set_required_domains_rejects_unknown_domain() {
    let mut ext = ExtBuilder::default().build();

    ext.execute_with(|| {
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
            Error::<Runtime>::DomainUnsupported
        );
        assert_eq!(Sccp::required_domains().into_inner(), initial);
    });
}

#[test]
fn activate_requires_core_domains_even_if_required_domains_storage_is_corrupted() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::Flower.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));

        // Simulate storage corruption: keep only ETH in governance-required domains.
        let corrupted_required_domains: BoundedVec<u32, SccpMaxDomains> =
            vec![SCCP_DOMAIN_ETH].try_into().expect("bounded");
        crate::RequiredDomains::<Runtime>::set(corrupted_required_domains);

        // Configure only ETH path; activation must still fail on missing core domains.
        assert_ok!(Sccp::set_remote_token(
            RuntimeOrigin::root(),
            asset_id,
            SCCP_DOMAIN_ETH,
            vec![1u8; 20],
        ));
        assert_ok!(Sccp::set_domain_endpoint(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            vec![11u8; 20],
        ));

        assert_noop!(
            Sccp::activate_token(RuntimeOrigin::root(), asset_id),
            Error::<Runtime>::RemoteTokenMissing
        );
    });
}

#[test]
fn activate_fails_closed_when_required_domains_storage_contains_unknown_domain() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::Flower.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        set_default_domain_endpoints();
        set_default_remote_tokens(asset_id);

        // Simulate storage corruption: inject an unsupported domain id.
        let corrupted_required_domains: BoundedVec<u32, SccpMaxDomains> = vec![
            SCCP_DOMAIN_ETH,
            SCCP_DOMAIN_BSC,
            SCCP_DOMAIN_SOL,
            SCCP_DOMAIN_TON,
            SCCP_DOMAIN_TRON,
            777,
        ]
        .try_into()
        .expect("bounded");
        crate::RequiredDomains::<Runtime>::set(corrupted_required_domains);

        // Even with forged IDs/endpoints for the unknown domain, activation must fail-closed.
        let forged_id: BoundedVec<u8, <Runtime as crate::Config>::MaxRemoteTokenIdLen> =
            vec![0xabu8; 20].try_into().expect("bounded");
        crate::pallet::RemoteToken::<Runtime>::insert(&asset_id, 777, forged_id.clone());
        crate::pallet::DomainEndpoint::<Runtime>::insert(777, forged_id);

        assert_noop!(
            Sccp::activate_token(RuntimeOrigin::root(), asset_id),
            Error::<Runtime>::DomainUnsupported
        );
    });
}

#[test]
fn set_required_domains_stores_canonical_sorted_order() {
    let mut ext = ExtBuilder::default().build();

    ext.execute_with(|| {
        // All core domains provided, but in non-canonical order.
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
fn set_required_domains_event_hash_uses_canonical_sorted_order() {
    let mut ext = ExtBuilder::default().build();

    ext.execute_with(|| {
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

        match last_sccp_event() {
            crate::Event::RequiredDomainsSet { domains_hash } => {
                assert_eq!(domains_hash, expected);
            }
            other => panic!("unexpected event: {:?}", other),
        }
    });
}

#[test]
fn activate_rejects_corrupted_remote_token_length() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::Future.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        set_default_domain_endpoints();
        set_default_remote_tokens(asset_id);

        // Corrupt ETH remote token length in storage (must be 20 bytes, set to 21).
        let bad: BoundedVec<u8, <Runtime as crate::Config>::MaxRemoteTokenIdLen> =
            vec![0xabu8; 21].try_into().expect("bounded");
        crate::pallet::RemoteToken::<Runtime>::insert(&asset_id, SCCP_DOMAIN_ETH, bad);

        assert_noop!(
            Sccp::activate_token(RuntimeOrigin::root(), asset_id),
            Error::<Runtime>::RemoteTokenInvalidLength
        );
    });
}

#[test]
fn activate_rejects_corrupted_domain_endpoint_length() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::Future.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        set_default_domain_endpoints();
        set_default_remote_tokens(asset_id);

        // Corrupt SOL endpoint length in storage (must be 32 bytes, set to 31).
        let bad: BoundedVec<u8, <Runtime as crate::Config>::MaxRemoteTokenIdLen> =
            vec![0xcdu8; 31].try_into().expect("bounded");
        crate::pallet::DomainEndpoint::<Runtime>::insert(SCCP_DOMAIN_SOL, bad);

        assert_noop!(
            Sccp::activate_token(RuntimeOrigin::root(), asset_id),
            Error::<Runtime>::DomainEndpointInvalidLength
        );
    });
}

#[test]
fn remove_and_finalize_removes_token_and_remote_ids() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::BlackPepper.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        set_default_domain_endpoints();
        set_default_remote_tokens(asset_id);
        assert_ok!(Sccp::activate_token(RuntimeOrigin::root(), asset_id));

        // Speed up removal in tests.
        assert_ok!(Sccp::set_inbound_grace_period(
            RuntimeOrigin::root(),
            0u32.into()
        ));

        System::set_block_number(1);
        assert_ok!(Sccp::remove_token(RuntimeOrigin::root(), asset_id));

        // Same block finalize must fail (requires now > until).
        assert_noop!(
            Sccp::finalize_remove(RuntimeOrigin::root(), asset_id),
            Error::<Runtime>::GracePeriodNotExpired
        );

        System::set_block_number(2);
        assert_ok!(Sccp::finalize_remove(RuntimeOrigin::root(), asset_id));

        assert!(!Sccp::is_sccp_asset(&asset_id));
        assert!(Sccp::token_state(asset_id).is_none());

        // Remote token ids cleared.
        assert!(Sccp::remote_token(asset_id, SCCP_DOMAIN_ETH).is_none());
        assert!(Sccp::remote_token(asset_id, SCCP_DOMAIN_SOL).is_none());
    });
}

#[test]
fn set_inbound_grace_period_updates_storage() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_ok!(Sccp::set_inbound_grace_period(
            RuntimeOrigin::root(),
            42u32.into()
        ));
        assert_eq!(Sccp::inbound_grace_period(), 42u64);
    });
}

#[test]
fn remove_token_sets_removing_state_and_grace_deadline() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::Headphones.into();

    ext.execute_with(|| {
        setup_active_token(asset_id);
        assert_ok!(Sccp::set_inbound_grace_period(
            RuntimeOrigin::root(),
            5u32.into()
        ));

        System::set_block_number(10);
        assert_ok!(Sccp::remove_token(RuntimeOrigin::root(), asset_id));

        let state = Sccp::token_state(asset_id).expect("state must exist");
        assert!(matches!(state.status, crate::TokenStatus::Removing));
        assert!(!state.outbound_enabled);
        assert!(!state.inbound_enabled);
        assert_eq!(state.inbound_enabled_until, Some(15u64));

        match last_sccp_event() {
            crate::Event::TokenRemoved {
                asset_id: e_asset_id,
                inbound_enabled_until,
            } => {
                assert_eq!(e_asset_id, asset_id);
                assert_eq!(inbound_enabled_until, 15u64);
            }
            _ => panic!("unexpected event"),
        }
    });
}

#[test]
fn finalize_remove_rejects_corrupted_removing_state_without_deadline() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::BatteryForMusicPlayer.into();

    ext.execute_with(|| {
        setup_active_token(asset_id);
        crate::pallet::Tokens::<Runtime>::mutate(asset_id, |state| {
            let s = state.as_mut().expect("token exists");
            s.status = crate::TokenStatus::Removing;
            s.inbound_enabled_until = None;
        });

        assert_noop!(
            Sccp::finalize_remove(RuntimeOrigin::root(), asset_id),
            Error::<Runtime>::TokenNotRemoving
        );
    });
}

#[test]
fn add_token_rejects_duplicate_asset() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::AppleTree.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        assert_noop!(
            Sccp::add_token(RuntimeOrigin::root(), asset_id),
            Error::<Runtime>::TokenAlreadyExists
        );
    });
}

#[test]
fn add_token_rejects_non_mintable_asset_supply() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::AcmeSpyKit.into();

    ext.execute_with(|| {
        assert_ok!(assets::Pallet::<Runtime>::register_asset_id(
            alice(),
            asset_id,
            AssetSymbol(b"NM".to_vec()),
            AssetName(b"NonMintable".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            1u32.into(),
            false,
            common::AssetType::Regular,
            None,
            None,
        ));

        assert_noop!(
            Sccp::add_token(RuntimeOrigin::root(), asset_id),
            Error::<Runtime>::AssetSupplyNotMintable
        );
    });
}

#[test]
fn set_remote_token_rejects_unknown_token() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::Headphones.into();

    ext.execute_with(|| {
        assert_noop!(
            Sccp::set_remote_token(
                RuntimeOrigin::root(),
                asset_id,
                SCCP_DOMAIN_ETH,
                vec![1u8; 20]
            ),
            Error::<Runtime>::TokenNotFound
        );
    });
}

#[test]
fn activate_token_rejects_non_pending_state() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::MusicPlayer.into();

    ext.execute_with(|| {
        setup_active_token(asset_id);
        assert_noop!(
            Sccp::activate_token(RuntimeOrigin::root(), asset_id),
            Error::<Runtime>::TokenNotPending
        );
    });
}

#[test]
fn finalize_remove_rejects_non_removing_and_missing_token() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::BatteryForMusicPlayer.into();
    let missing: AssetId = common::mock::ComicAssetId::JesterMarotte.into();

    ext.execute_with(|| {
        setup_active_token(asset_id);
        assert_noop!(
            Sccp::finalize_remove(RuntimeOrigin::root(), asset_id),
            Error::<Runtime>::TokenNotRemoving
        );
        assert_noop!(
            Sccp::finalize_remove(RuntimeOrigin::root(), missing),
            Error::<Runtime>::TokenNotFound
        );
    });
}

#[test]
fn burn_rejects_non_active_token_state() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::MichaelJacksonCD.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(assets::Pallet::<Runtime>::mint_to(
            &asset_id,
            &alice(),
            &alice(),
            100u32.into()
        ));
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        set_default_domain_endpoints();
        set_default_remote_tokens(asset_id);

        let mut recipient = [0u8; 32];
        recipient[12..].copy_from_slice(&[7u8; 20]);
        assert_noop!(
            Sccp::burn(
                RuntimeOrigin::signed(alice()),
                asset_id,
                1u32.into(),
                SCCP_DOMAIN_ETH,
                recipient,
            ),
            Error::<Runtime>::TokenNotActive
        );
    });
}

#[test]
fn burn_rejects_outbound_disabled_and_basic_payload_guards() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::CrackedBrassBell.into();

    ext.execute_with(|| {
        setup_active_token(asset_id);
        assert_ok!(assets::Pallet::<Runtime>::mint_to(
            &asset_id,
            &alice(),
            &alice(),
            100u32.into()
        ));

        // Corrupt active token state to simulate governance-disable fail-closed behavior.
        crate::pallet::Tokens::<Runtime>::mutate(asset_id, |state| {
            let s = state.as_mut().expect("token exists");
            s.outbound_enabled = false;
        });

        let mut canonical_recipient = [0u8; 32];
        canonical_recipient[12..].copy_from_slice(&[0x11u8; 20]);
        assert_noop!(
            Sccp::burn(
                RuntimeOrigin::signed(alice()),
                asset_id,
                1u32.into(),
                SCCP_DOMAIN_ETH,
                canonical_recipient,
            ),
            Error::<Runtime>::OutboundDisabled
        );

        assert_noop!(
            Sccp::burn(
                RuntimeOrigin::signed(alice()),
                asset_id,
                0u32.into(),
                SCCP_DOMAIN_ETH,
                canonical_recipient,
            ),
            Error::<Runtime>::AmountIsZero
        );

        assert_noop!(
            Sccp::burn(
                RuntimeOrigin::signed(alice()),
                asset_id,
                1u32.into(),
                SCCP_DOMAIN_SOL,
                [0u8; 32],
            ),
            Error::<Runtime>::RecipientIsZero
        );
    });
}

#[test]
fn burn_rejects_nonce_overflow_and_preexisting_message_id() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::Potato.into();

    ext.execute_with(|| {
        setup_active_token(asset_id);
        assert_ok!(assets::Pallet::<Runtime>::mint_to(
            &asset_id,
            &alice(),
            &alice(),
            100u32.into()
        ));

        let mut recipient = [0u8; 32];
        recipient[12..].copy_from_slice(&[0x22u8; 20]);

        crate::pallet::Nonce::<Runtime>::set(u64::MAX);
        assert_noop!(
            Sccp::burn(
                RuntimeOrigin::signed(alice()),
                asset_id,
                1u32.into(),
                SCCP_DOMAIN_ETH,
                recipient,
            ),
            Error::<Runtime>::NonceOverflow
        );

        // Force next nonce to 1 and preinsert the matching burn record key.
        crate::pallet::Nonce::<Runtime>::set(0);
        let amount: Balance = 1u32.into();
        let asset_h256: H256 = asset_id.into();
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_ETH,
            nonce: 1,
            sora_asset_id: asset_h256.0,
            amount,
            recipient,
        };
        let mut preimage = SCCP_MSG_PREFIX_BURN_V1.to_vec();
        preimage.extend(payload.encode());
        let message_id = H256::from_slice(&keccak_256(&preimage));
        crate::pallet::Burns::<Runtime>::insert(
            message_id,
            crate::BurnRecord {
                sender: alice(),
                asset_id,
                amount,
                dest_domain: SCCP_DOMAIN_ETH,
                recipient,
                nonce: 1,
                block_number: System::block_number(),
            },
        );

        let nonce_before = Sccp::nonce();
        let balance_before = assets::Pallet::<Runtime>::free_balance(&asset_id, &alice()).unwrap();
        let _ = take_aux_digest_items();

        assert_noop!(
            Sccp::burn(
                RuntimeOrigin::signed(alice()),
                asset_id,
                amount,
                SCCP_DOMAIN_ETH,
                recipient,
            ),
            Error::<Runtime>::BurnRecordAlreadyExists
        );

        assert_eq!(Sccp::nonce(), nonce_before);
        assert_eq!(
            assets::Pallet::<Runtime>::free_balance(&asset_id, &alice()).unwrap(),
            balance_before
        );
        assert!(take_aux_digest_items().is_empty());
    });
}

#[test]
fn mint_from_proof_rejects_inbound_disabled_and_already_processed() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::GoldenTicket.into();

    ext.execute_with(|| {
        setup_active_token(asset_id);
        set_dummy_evm_anchor(SCCP_DOMAIN_ETH);

        let asset_h256: H256 = asset_id.into();
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 17,
            sora_asset_id: asset_h256.0,
            amount: 1u32.into(),
            recipient: [0x44u8; 32],
        };
        let mut preimage = SCCP_MSG_PREFIX_BURN_V1.to_vec();
        preimage.extend(payload.encode());
        let message_id = H256::from_slice(&keccak_256(&preimage));

        crate::pallet::Tokens::<Runtime>::mutate(asset_id, |state| {
            let s = state.as_mut().expect("token exists");
            s.inbound_enabled = false;
        });
        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_ETH,
                payload.clone(),
                vec![],
            ),
            Error::<Runtime>::InboundDisabled
        );

        crate::pallet::Tokens::<Runtime>::mutate(asset_id, |state| {
            let s = state.as_mut().expect("token exists");
            s.inbound_enabled = true;
        });
        crate::pallet::ProcessedInbound::<Runtime>::insert(message_id, true);
        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_ETH,
                payload,
                vec![],
            ),
            Error::<Runtime>::InboundAlreadyProcessed
        );
    });
}

#[test]
fn mint_from_proof_rejects_zero_amount_zero_recipient_and_missing_anchor() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::Apple.into();

    ext.execute_with(|| {
        setup_active_token(asset_id);

        // Explicitly require EVM anchor mode for ETH but do not configure an anchor.
        assert_ok!(Sccp::set_inbound_finality_mode(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            InboundFinalityMode::EvmAnchor,
        ));
        assert_ok!(Sccp::set_evm_anchor_mode_enabled(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            true
        ));

        let asset_h256: H256 = asset_id.into();
        let base_payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 23,
            sora_asset_id: asset_h256.0,
            amount: 1u32.into(),
            recipient: [0x55u8; 32],
        };

        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_ETH,
                base_payload.clone(),
                vec![],
            ),
            Error::<Runtime>::EvmInboundAnchorMissing
        );

        // Switch to attester quorum so amount/recipient guards are reached before proof decoding.
        let a0 = sp_core::H160::from_low_u64_be(1);
        assert_ok!(Sccp::set_inbound_attesters(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            vec![a0],
            1
        ));
        assert_ok!(Sccp::set_inbound_finality_mode(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            InboundFinalityMode::AttesterQuorum,
        ));

        let mut amount_zero = base_payload.clone();
        amount_zero.amount = 0u32.into();
        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_ETH,
                amount_zero,
                vec![],
            ),
            Error::<Runtime>::AmountIsZero
        );

        let mut recipient_zero = base_payload;
        recipient_zero.recipient = [0u8; 32];
        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_ETH,
                recipient_zero,
                vec![],
            ),
            Error::<Runtime>::RecipientIsZero
        );
    });
}

#[test]
fn mint_from_proof_rejects_evm_proof_structural_guardrails() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::BlackPepper.into();

    ext.execute_with(|| {
        setup_active_token(asset_id);
        set_dummy_evm_anchor(SCCP_DOMAIN_ETH);

        let asset_h256: H256 = asset_id.into();
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 39,
            sora_asset_id: asset_h256.0,
            amount: 1u32.into(),
            recipient: [0x33u8; 32],
        };

        let wrong_anchor = EvmBurnProofV1 {
            anchor_block_hash: H256([9u8; 32]),
            account_proof: vec![],
            storage_proof: vec![],
        };
        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_ETH,
                payload.clone(),
                wrong_anchor.encode(),
            ),
            Error::<Runtime>::ProofVerificationFailed
        );

        let too_many_nodes = EvmBurnProofV1 {
            anchor_block_hash: H256([1u8; 32]),
            account_proof: vec![vec![1u8; 1]; crate::SCCP_MAX_EVM_PROOF_NODES + 1],
            storage_proof: vec![],
        };
        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_ETH,
                payload.clone(),
                too_many_nodes.encode(),
            ),
            Error::<Runtime>::ProofVerificationFailed
        );

        let oversized_node = EvmBurnProofV1 {
            anchor_block_hash: H256([1u8; 32]),
            account_proof: vec![vec![1u8; crate::SCCP_MAX_EVM_PROOF_NODE_BYTES + 1]],
            storage_proof: vec![],
        };
        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_ETH,
                payload.clone(),
                oversized_node.encode(),
            ),
            Error::<Runtime>::ProofVerificationFailed
        );

        let per_node =
            (crate::SCCP_MAX_EVM_PROOF_TOTAL_BYTES / crate::SCCP_MAX_EVM_PROOF_NODES) + 1;
        assert!(per_node <= crate::SCCP_MAX_EVM_PROOF_NODE_BYTES);
        let total_overflow = EvmBurnProofV1 {
            anchor_block_hash: H256([1u8; 32]),
            account_proof: vec![vec![1u8; per_node]; crate::SCCP_MAX_EVM_PROOF_NODES],
            storage_proof: vec![],
        };
        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_ETH,
                payload.clone(),
                total_overflow.encode(),
            ),
            Error::<Runtime>::ProofVerificationFailed
        );

        let mut trailing_bytes = EvmBurnProofV1 {
            anchor_block_hash: H256([1u8; 32]),
            account_proof: vec![],
            storage_proof: vec![],
        }
        .encode();
        trailing_bytes.push(0x99);
        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_ETH,
                payload,
                trailing_bytes,
            ),
            Error::<Runtime>::ProofVerificationFailed
        );
    });
}

#[test]
fn mint_from_proof_rejects_payload_sanity_domain_guards() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 33,
            sora_asset_id: [1u8; 32],
            amount: 1u32.into(),
            recipient: [2u8; 32],
        };

        let mut bad_version = payload.clone();
        bad_version.version = 2;
        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_ETH,
                bad_version,
                vec![],
            ),
            Error::<Runtime>::DomainUnsupported
        );

        let mut bad_dest = payload.clone();
        bad_dest.dest_domain = SCCP_DOMAIN_SOL;
        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_ETH,
                bad_dest,
                vec![],
            ),
            Error::<Runtime>::DomainUnsupported
        );

        let mut mismatch_source = payload.clone();
        mismatch_source.source_domain = SCCP_DOMAIN_SOL;
        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_ETH,
                mismatch_source,
                vec![],
            ),
            Error::<Runtime>::DomainUnsupported
        );

        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_SORA,
                payload,
                vec![],
            ),
            Error::<Runtime>::DomainUnsupported
        );
    });
}

#[test]
fn mint_from_proof_rejects_token_not_found_with_corrupted_remote_token_storage() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::GreenPromise.into();

    ext.execute_with(|| {
        // Configure source domain and finality as available.
        set_default_domain_endpoints();
        set_dummy_evm_anchor(SCCP_DOMAIN_ETH);

        // Insert remote-token mapping without creating token state.
        let remote: BoundedVec<u8, <Runtime as crate::Config>::MaxRemoteTokenIdLen> =
            vec![0x11u8; 20].try_into().expect("bounded");
        crate::pallet::RemoteToken::<Runtime>::insert(asset_id, SCCP_DOMAIN_ETH, remote);

        let asset_h256: H256 = asset_id.into();
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 34,
            sora_asset_id: asset_h256.0,
            amount: 1u32.into(),
            recipient: [3u8; 32],
        };

        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_ETH,
                payload,
                vec![],
            ),
            Error::<Runtime>::TokenNotFound
        );
    });
}

#[test]
fn governance_calls_reject_bad_origin_for_domain_controls() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_noop!(
            Sccp::set_domain_endpoint(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_ETH,
                vec![1u8; 20]
            ),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Sccp::clear_domain_endpoint(RuntimeOrigin::signed(alice()), SCCP_DOMAIN_ETH),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Sccp::set_evm_inbound_anchor(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_ETH,
                1,
                H256([1u8; 32]),
                H256([2u8; 32]),
            ),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Sccp::clear_evm_inbound_anchor(RuntimeOrigin::signed(alice()), SCCP_DOMAIN_ETH),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Sccp::set_evm_anchor_mode_enabled(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_ETH,
                true
            ),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Sccp::set_inbound_domain_paused(RuntimeOrigin::signed(alice()), SCCP_DOMAIN_ETH, true),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Sccp::set_outbound_domain_paused(RuntimeOrigin::signed(alice()), SCCP_DOMAIN_ETH, true),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Sccp::invalidate_inbound_message(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_ETH,
                H256([3u8; 32]),
            ),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Sccp::clear_invalidated_inbound_message(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_ETH,
                H256([3u8; 32]),
            ),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Sccp::set_inbound_grace_period(RuntimeOrigin::signed(alice()), 1u32.into()),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn governance_calls_reject_bad_origin_for_token_lifecycle_and_light_clients() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::Mango.into();
    ext.execute_with(|| {
        assert_noop!(
            Sccp::add_token(RuntimeOrigin::signed(alice()), asset_id),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Sccp::set_remote_token(
                RuntimeOrigin::signed(alice()),
                asset_id,
                SCCP_DOMAIN_ETH,
                vec![1u8; 20]
            ),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Sccp::activate_token(RuntimeOrigin::signed(alice()), asset_id),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Sccp::remove_token(RuntimeOrigin::signed(alice()), asset_id),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Sccp::finalize_remove(RuntimeOrigin::signed(alice()), asset_id),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Sccp::init_bsc_light_client(
                RuntimeOrigin::signed(alice()),
                vec![0u8; 1],
                vec![sp_core::H160::from_low_u64_be(1)],
                200,
                0,
                56,
                1,
            ),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Sccp::set_bsc_validators(
                RuntimeOrigin::signed(alice()),
                vec![sp_core::H160::from_low_u64_be(1)]
            ),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Sccp::init_tron_light_client(
                RuntimeOrigin::signed(alice()),
                vec![0u8; 1],
                vec![0u8; 65],
                vec![sp_core::H160::from_low_u64_be(1)],
                0x41,
            ),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Sccp::set_tron_witnesses(
                RuntimeOrigin::signed(alice()),
                vec![sp_core::H160::from_low_u64_be(1)]
            ),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn governance_domain_controls_reject_sora_and_unknown_domains() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_noop!(
            Sccp::set_domain_endpoint(RuntimeOrigin::root(), SCCP_DOMAIN_SORA, vec![1u8; 20]),
            Error::<Runtime>::DomainUnsupported
        );
        assert_noop!(
            Sccp::set_domain_endpoint(RuntimeOrigin::root(), 99, vec![1u8; 20]),
            Error::<Runtime>::DomainUnsupported
        );
        assert_noop!(
            Sccp::clear_domain_endpoint(RuntimeOrigin::root(), SCCP_DOMAIN_SORA),
            Error::<Runtime>::DomainUnsupported
        );
        assert_noop!(
            Sccp::clear_domain_endpoint(RuntimeOrigin::root(), 99),
            Error::<Runtime>::DomainUnsupported
        );
        assert_noop!(
            Sccp::set_evm_inbound_anchor(
                RuntimeOrigin::root(),
                SCCP_DOMAIN_SORA,
                1,
                H256([1u8; 32]),
                H256([2u8; 32]),
            ),
            Error::<Runtime>::DomainUnsupported
        );
        assert_noop!(
            Sccp::set_evm_inbound_anchor(
                RuntimeOrigin::root(),
                SCCP_DOMAIN_SOL,
                1,
                H256([1u8; 32]),
                H256([2u8; 32]),
            ),
            Error::<Runtime>::DomainUnsupported
        );
        assert_noop!(
            Sccp::clear_evm_inbound_anchor(RuntimeOrigin::root(), SCCP_DOMAIN_SORA),
            Error::<Runtime>::DomainUnsupported
        );
        assert_noop!(
            Sccp::clear_evm_inbound_anchor(RuntimeOrigin::root(), SCCP_DOMAIN_SOL),
            Error::<Runtime>::DomainUnsupported
        );
        assert_noop!(
            Sccp::set_evm_anchor_mode_enabled(RuntimeOrigin::root(), SCCP_DOMAIN_SORA, true),
            Error::<Runtime>::DomainUnsupported
        );
        assert_noop!(
            Sccp::set_evm_anchor_mode_enabled(RuntimeOrigin::root(), SCCP_DOMAIN_SOL, true),
            Error::<Runtime>::DomainUnsupported
        );
        assert_noop!(
            Sccp::set_inbound_domain_paused(RuntimeOrigin::root(), SCCP_DOMAIN_SORA, true),
            Error::<Runtime>::DomainUnsupported
        );
        assert_noop!(
            Sccp::set_inbound_domain_paused(RuntimeOrigin::root(), 99, true),
            Error::<Runtime>::DomainUnsupported
        );
        assert_noop!(
            Sccp::set_outbound_domain_paused(RuntimeOrigin::root(), SCCP_DOMAIN_SORA, true),
            Error::<Runtime>::DomainUnsupported
        );
        assert_noop!(
            Sccp::set_outbound_domain_paused(RuntimeOrigin::root(), 99, true),
            Error::<Runtime>::DomainUnsupported
        );
        assert_noop!(
            Sccp::invalidate_inbound_message(
                RuntimeOrigin::root(),
                SCCP_DOMAIN_SORA,
                H256([0u8; 32])
            ),
            Error::<Runtime>::DomainUnsupported
        );
        assert_noop!(
            Sccp::invalidate_inbound_message(RuntimeOrigin::root(), 99, H256([0u8; 32])),
            Error::<Runtime>::DomainUnsupported
        );
        assert_noop!(
            Sccp::clear_invalidated_inbound_message(
                RuntimeOrigin::root(),
                SCCP_DOMAIN_SORA,
                H256([0u8; 32]),
            ),
            Error::<Runtime>::DomainUnsupported
        );
        assert_noop!(
            Sccp::clear_invalidated_inbound_message(RuntimeOrigin::root(), 99, H256([0u8; 32])),
            Error::<Runtime>::DomainUnsupported
        );
    });
}

#[test]
fn set_remote_token_rejects_sora_and_unknown_domains() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::AppleTree.into();
    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));

        assert_noop!(
            Sccp::set_remote_token(
                RuntimeOrigin::root(),
                asset_id,
                SCCP_DOMAIN_SORA,
                vec![1u8; 20]
            ),
            Error::<Runtime>::DomainUnsupported
        );
        assert_noop!(
            Sccp::set_remote_token(RuntimeOrigin::root(), asset_id, 99, vec![1u8; 20]),
            Error::<Runtime>::DomainUnsupported
        );
    });
}

#[test]
fn remove_token_rejects_missing_token() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let missing: AssetId = common::mock::ComicAssetId::Future.into();
        assert_noop!(
            Sccp::remove_token(RuntimeOrigin::root(), missing),
            Error::<Runtime>::TokenNotFound
        );
    });
}

#[test]
fn set_inbound_finality_mode_rejects_bad_origin_and_domain_guards() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_noop!(
            Sccp::set_inbound_finality_mode(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_ETH,
                InboundFinalityMode::EvmAnchor
            ),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Sccp::set_inbound_finality_mode(
                RuntimeOrigin::root(),
                SCCP_DOMAIN_SORA,
                InboundFinalityMode::EvmAnchor
            ),
            Error::<Runtime>::DomainUnsupported
        );
        assert_noop!(
            Sccp::set_inbound_finality_mode(
                RuntimeOrigin::root(),
                99,
                InboundFinalityMode::EvmAnchor
            ),
            Error::<Runtime>::DomainUnsupported
        );
    });
}

#[test]
fn set_domain_endpoint_and_set_remote_token_reject_invalid_lengths_directly() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::Teapot.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));

        assert_noop!(
            Sccp::set_domain_endpoint(RuntimeOrigin::root(), SCCP_DOMAIN_ETH, vec![0u8; 19]),
            Error::<Runtime>::DomainEndpointInvalidLength
        );
        assert_noop!(
            Sccp::set_domain_endpoint(RuntimeOrigin::root(), SCCP_DOMAIN_SOL, vec![0u8; 31]),
            Error::<Runtime>::DomainEndpointInvalidLength
        );

        assert_noop!(
            Sccp::set_remote_token(
                RuntimeOrigin::root(),
                asset_id,
                SCCP_DOMAIN_ETH,
                vec![0u8; 19]
            ),
            Error::<Runtime>::RemoteTokenInvalidLength
        );
        assert_noop!(
            Sccp::set_remote_token(
                RuntimeOrigin::root(),
                asset_id,
                SCCP_DOMAIN_SOL,
                vec![0u8; 31]
            ),
            Error::<Runtime>::RemoteTokenInvalidLength
        );
    });
}

#[test]
fn clear_domain_endpoint_and_clear_evm_anchor_are_idempotent() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_ok!(Sccp::clear_domain_endpoint(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH
        ));
        assert_ok!(Sccp::clear_domain_endpoint(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH
        ));

        assert_ok!(Sccp::clear_evm_inbound_anchor(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH
        ));
        assert_ok!(Sccp::clear_evm_inbound_anchor(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH
        ));
    });
}

#[test]
fn set_evm_anchor_mode_enabled_toggles_storage() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_ok!(Sccp::set_evm_anchor_mode_enabled(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            true
        ));
        assert!(crate::pallet::EvmAnchorModeEnabled::<Runtime>::get(
            SCCP_DOMAIN_ETH
        ));
        assert_ok!(Sccp::set_evm_anchor_mode_enabled(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            false
        ));
        assert!(!crate::pallet::EvmAnchorModeEnabled::<Runtime>::get(
            SCCP_DOMAIN_ETH
        ));
    });
}

#[test]
fn clear_domain_endpoint_and_clear_evm_anchor_remove_stored_values() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_ok!(Sccp::set_domain_endpoint(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            vec![9u8; 20]
        ));
        assert!(crate::pallet::DomainEndpoint::<Runtime>::get(SCCP_DOMAIN_ETH).is_some());
        assert_ok!(Sccp::clear_domain_endpoint(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH
        ));
        assert!(crate::pallet::DomainEndpoint::<Runtime>::get(SCCP_DOMAIN_ETH).is_none());

        assert_ok!(Sccp::set_evm_inbound_anchor(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            100,
            H256([1u8; 32]),
            H256([2u8; 32]),
        ));
        assert!(crate::pallet::EvmInboundAnchors::<Runtime>::get(SCCP_DOMAIN_ETH).is_some());
        assert_ok!(Sccp::clear_evm_inbound_anchor(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH
        ));
        assert!(crate::pallet::EvmInboundAnchors::<Runtime>::get(SCCP_DOMAIN_ETH).is_none());
    });
}

#[test]
fn invalidate_and_clear_invalidated_inbound_message_toggle_storage() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let message_id = H256([7u8; 32]);
        assert_ok!(Sccp::invalidate_inbound_message(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            message_id
        ));
        assert!(crate::pallet::InvalidatedInbound::<Runtime>::get(
            SCCP_DOMAIN_ETH,
            message_id
        ));

        assert_ok!(Sccp::clear_invalidated_inbound_message(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            message_id
        ));
        assert!(!crate::pallet::InvalidatedInbound::<Runtime>::get(
            SCCP_DOMAIN_ETH,
            message_id
        ));
    });
}

#[test]
fn init_bsc_light_client_rejects_invalid_parameter_values_before_parsing() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let signer = sp_core::H160::from_low_u64_be(1);
        let header = vec![0u8; 1];

        assert_noop!(
            Sccp::init_bsc_light_client(
                RuntimeOrigin::root(),
                header.clone(),
                vec![signer],
                0,
                0,
                56,
                1,
            ),
            Error::<Runtime>::BscValidatorsInvalid
        );
        assert_noop!(
            Sccp::init_bsc_light_client(
                RuntimeOrigin::root(),
                header.clone(),
                vec![signer],
                200,
                crate::SCCP_BSC_HEADER_RETENTION,
                56,
                1,
            ),
            Error::<Runtime>::BscValidatorsInvalid
        );
        assert_noop!(
            Sccp::init_bsc_light_client(
                RuntimeOrigin::root(),
                header.clone(),
                vec![signer],
                200,
                0,
                0,
                1,
            ),
            Error::<Runtime>::BscValidatorsInvalid
        );
        assert_noop!(
            Sccp::init_bsc_light_client(
                RuntimeOrigin::root(),
                header,
                vec![signer],
                200,
                0,
                56,
                0,
            ),
            Error::<Runtime>::BscValidatorsInvalid
        );
    });
}

#[test]
fn init_bsc_light_client_rejects_too_large_checkpoint_header() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_noop!(
            Sccp::init_bsc_light_client(
                RuntimeOrigin::root(),
                vec![0u8; crate::SCCP_MAX_BSC_HEADER_RLP_BYTES + 1],
                vec![sp_core::H160::from_low_u64_be(1)],
                200,
                0,
                56,
                1,
            ),
            Error::<Runtime>::BscHeaderTooLarge
        );
    });
}

#[test]
fn init_bsc_light_client_rejects_validator_set_overflow_before_parsing() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let count = SccpMaxBscValidators::get() as usize + 1;
        let validators: Vec<sp_core::H160> = (0..count)
            .map(|i| sp_core::H160::from_low_u64_be((i + 1) as u64))
            .collect();
        assert_noop!(
            Sccp::init_bsc_light_client(
                RuntimeOrigin::root(),
                vec![0u8; 1],
                validators,
                200,
                0,
                56,
                1,
            ),
            Error::<Runtime>::BscValidatorsInvalid
        );
    });
}

#[test]
fn set_bsc_validators_rejects_empty_and_updates_storage() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_noop!(
            Sccp::set_bsc_validators(RuntimeOrigin::root(), vec![]),
            Error::<Runtime>::BscValidatorsInvalid
        );
        let a0 = sp_core::H160::from_low_u64_be(2);
        let a1 = sp_core::H160::from_low_u64_be(1);
        assert_ok!(Sccp::set_bsc_validators(
            RuntimeOrigin::root(),
            vec![a0, a1]
        ));
        let stored = crate::pallet::BscValidators::<Runtime>::get().expect("stored validators");
        assert_eq!(stored.into_inner(), vec![a1, a0]);
    });
}

#[test]
fn set_bsc_validators_event_uses_current_head_number() {
    use core::str::FromStr;
    use sp_core::H160;

    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        System::set_block_number(1);
        let header_rlp = include_bytes!("fixtures/bsc_header_81094034.rlp").to_vec();
        let signer = H160::from_str("0x9f1b7fae54be07f4fee34eb1aacb39a1f7b6fc92").unwrap();
        assert_ok!(Sccp::init_bsc_light_client(
            RuntimeOrigin::root(),
            header_rlp,
            vec![signer],
            1000,
            0,
            56,
            16,
        ));

        let override_validator = H160::from_low_u64_be(42);
        assert_ok!(Sccp::set_bsc_validators(
            RuntimeOrigin::root(),
            vec![override_validator]
        ));

        match last_sccp_event() {
            crate::Event::BscValidatorsUpdated {
                number,
                validators_hash,
            } => {
                assert_eq!(number, 81_094_034);
                assert_eq!(
                    validators_hash,
                    H256::from_slice(&keccak_256(&vec![override_validator].encode()))
                );
            }
            _ => panic!("unexpected event"),
        }
    });
}

#[test]
fn set_bsc_validators_event_uses_zero_number_without_head() {
    use sp_core::H160;

    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        System::set_block_number(1);
        let a0 = H160::from_low_u64_be(2);
        let a1 = H160::from_low_u64_be(1);
        assert_ok!(Sccp::set_bsc_validators(
            RuntimeOrigin::root(),
            vec![a0, a1]
        ));

        match last_sccp_event() {
            crate::Event::BscValidatorsUpdated {
                number,
                validators_hash,
            } => {
                assert_eq!(number, 0);
                assert_eq!(
                    validators_hash,
                    H256::from_slice(&keccak_256(&vec![a1, a0].encode()))
                );
            }
            _ => panic!("unexpected event"),
        }
    });
}

#[test]
fn set_bsc_validators_rejects_validator_set_overflow() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let count = SccpMaxBscValidators::get() as usize + 1;
        let validators: Vec<sp_core::H160> = (0..count)
            .map(|i| sp_core::H160::from_low_u64_be((i + 1) as u64))
            .collect();

        assert_noop!(
            Sccp::set_bsc_validators(RuntimeOrigin::root(), validators),
            Error::<Runtime>::BscValidatorsInvalid
        );
    });
}

#[test]
fn init_tron_light_client_rejects_size_signature_and_empty_witness_set() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_noop!(
            Sccp::init_tron_light_client(
                RuntimeOrigin::root(),
                vec![0u8; crate::SCCP_MAX_TRON_RAW_DATA_BYTES + 1],
                vec![0u8; 65],
                vec![sp_core::H160::from_low_u64_be(1)],
                0x41,
            ),
            Error::<Runtime>::TronHeaderTooLarge
        );
        assert_noop!(
            Sccp::init_tron_light_client(
                RuntimeOrigin::root(),
                vec![0u8; 1],
                vec![0u8; 64],
                vec![sp_core::H160::from_low_u64_be(1)],
                0x41,
            ),
            Error::<Runtime>::TronHeaderInvalid
        );
        assert_noop!(
            Sccp::init_tron_light_client(
                RuntimeOrigin::root(),
                vec![0u8; 1],
                vec![0u8; 65],
                vec![],
                0x41,
            ),
            Error::<Runtime>::TronWitnessesInvalid
        );
    });
}

#[test]
fn init_tron_light_client_rejects_witness_set_overflow_before_parsing() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let count = SccpMaxBscValidators::get() as usize + 1;
        let witnesses: Vec<sp_core::H160> = (0..count)
            .map(|i| sp_core::H160::from_low_u64_be((i + 1) as u64))
            .collect();
        assert_noop!(
            Sccp::init_tron_light_client(
                RuntimeOrigin::root(),
                vec![0u8; 1],
                vec![0u8; 65],
                witnesses,
                0x41,
            ),
            Error::<Runtime>::TronWitnessesInvalid
        );
    });
}

#[test]
fn init_tron_light_client_rejects_witness_prefix_mismatch() {
    use sp_core::ecdsa;
    use sp_core::Pair;
    use sp_core::H160;

    fn pb_varint(mut v: u64) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            let mut b = (v & 0x7f) as u8;
            v >>= 7;
            if v != 0 {
                b |= 0x80;
            }
            out.push(b);
            if v == 0 {
                break;
            }
        }
        out
    }

    fn eth_address_from_pair(pair: &ecdsa::Pair) -> H160 {
        let msg = H256([9u8; 32]);
        let sig = pair.sign_prehashed(&msg.0);
        let pk = sp_io::crypto::secp256k1_ecdsa_recover(&sig.0, &msg.0)
            .ok()
            .unwrap();
        H160::from_slice(&keccak_256(&pk)[12..])
    }

    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let p0 = ecdsa::Pair::from_seed(&[1u8; 32]);
        let w0 = eth_address_from_pair(&p0);

        let mut witness_addr = [0u8; 21];
        witness_addr[0] = 0x41;
        witness_addr[1..].copy_from_slice(w0.as_bytes());

        let mut raw_data = Vec::new();
        raw_data.push(0x1a);
        raw_data.extend_from_slice(&pb_varint(32));
        raw_data.extend_from_slice(H256([0u8; 32]).as_bytes());
        raw_data.push(0x38);
        raw_data.extend_from_slice(&pb_varint(1));
        raw_data.push(0x4a);
        raw_data.extend_from_slice(&pb_varint(21));
        raw_data.extend_from_slice(&witness_addr);
        raw_data.push(0x5a);
        raw_data.extend_from_slice(&pb_varint(32));
        raw_data.extend_from_slice(H256([1u8; 32]).as_bytes());

        let raw_hash = sp_io::hashing::sha2_256(&raw_data);
        let sig = p0.sign_prehashed(&raw_hash).0.to_vec();

        assert_noop!(
            Sccp::init_tron_light_client(RuntimeOrigin::root(), raw_data, sig, vec![w0], 0x42,),
            Error::<Runtime>::TronHeaderInvalid
        );
    });
}

#[test]
fn set_tron_witnesses_rejects_empty_and_updates_storage() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_noop!(
            Sccp::set_tron_witnesses(RuntimeOrigin::root(), vec![]),
            Error::<Runtime>::TronWitnessesInvalid
        );

        let w0 = sp_core::H160::from_low_u64_be(2);
        let w1 = sp_core::H160::from_low_u64_be(1);
        assert_ok!(Sccp::set_tron_witnesses(
            RuntimeOrigin::root(),
            vec![w0, w1]
        ));
        let stored = crate::pallet::TronWitnesses::<Runtime>::get().expect("stored witnesses");
        assert_eq!(stored.into_inner(), vec![w1, w0]);
    });
}

#[test]
fn set_tron_witnesses_rejects_duplicate_entries() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let w0 = sp_core::H160::from_low_u64_be(1);
        assert_noop!(
            Sccp::set_tron_witnesses(RuntimeOrigin::root(), vec![w0, w0]),
            Error::<Runtime>::TronWitnessesInvalid
        );
    });
}

#[test]
fn set_tron_witnesses_rejects_witness_set_overflow() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let count = SccpMaxBscValidators::get() as usize + 1;
        let witnesses: Vec<sp_core::H160> = (0..count)
            .map(|i| sp_core::H160::from_low_u64_be((i + 1) as u64))
            .collect();

        assert_noop!(
            Sccp::set_tron_witnesses(RuntimeOrigin::root(), witnesses),
            Error::<Runtime>::TronWitnessesInvalid
        );
    });
}

#[test]
fn set_tron_witnesses_updates_params_when_light_client_initialized() {
    use sp_core::ecdsa;
    use sp_core::Pair;
    use sp_core::H160;

    fn pb_varint(mut v: u64) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            let mut b = (v & 0x7f) as u8;
            v >>= 7;
            if v != 0 {
                b |= 0x80;
            }
            out.push(b);
            if v == 0 {
                break;
            }
        }
        out
    }

    fn eth_address_from_pair(pair: &ecdsa::Pair) -> H160 {
        let msg = H256([9u8; 32]);
        let sig = pair.sign_prehashed(&msg.0);
        let pk = sp_io::crypto::secp256k1_ecdsa_recover(&sig.0, &msg.0)
            .ok()
            .unwrap();
        H160::from_slice(&keccak_256(&pk)[12..])
    }

    fn tron_raw_data(
        parent_hash: H256,
        number: u64,
        witness_address: &[u8; 21],
        state_root: H256,
    ) -> Vec<u8> {
        let mut raw_data = Vec::new();
        raw_data.push(0x1a);
        raw_data.extend_from_slice(&pb_varint(32));
        raw_data.extend_from_slice(parent_hash.as_bytes());
        raw_data.push(0x38);
        raw_data.extend_from_slice(&pb_varint(number));
        raw_data.push(0x4a);
        raw_data.extend_from_slice(&pb_varint(21));
        raw_data.extend_from_slice(witness_address);
        raw_data.push(0x5a);
        raw_data.extend_from_slice(&pb_varint(32));
        raw_data.extend_from_slice(state_root.as_bytes());
        raw_data
    }

    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let p0 = ecdsa::Pair::from_seed(&[1u8; 32]);
        let p1 = ecdsa::Pair::from_seed(&[2u8; 32]);
        let w0 = eth_address_from_pair(&p0);
        let w1 = eth_address_from_pair(&p1);

        let mut witness_addr = [0u8; 21];
        witness_addr[0] = 0x41;
        witness_addr[1..].copy_from_slice(w0.as_bytes());
        let raw_data = tron_raw_data(H256([0u8; 32]), 1, &witness_addr, H256([1u8; 32]));
        let raw_hash = sp_io::hashing::sha2_256(&raw_data);
        let sig = p0.sign_prehashed(&raw_hash).0.to_vec();

        assert_ok!(Sccp::init_tron_light_client(
            RuntimeOrigin::root(),
            raw_data,
            sig,
            vec![w0],
            0x41,
        ));
        let initial = Sccp::tron_params().expect("params exist");
        assert_eq!(initial.witness_count, 1);
        assert_eq!(initial.solidification_threshold, 1);
        assert_eq!(initial.address_prefix, 0x41);

        assert_ok!(Sccp::set_tron_witnesses(
            RuntimeOrigin::root(),
            vec![w1, w0]
        ));
        let updated = Sccp::tron_params().expect("params updated");
        assert_eq!(updated.witness_count, 2);
        assert_eq!(updated.solidification_threshold, 2);
        assert_eq!(updated.address_prefix, 0x41);
    });
}

#[test]
fn set_tron_witnesses_without_light_client_keeps_params_none_and_uses_zero_number_event() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        System::set_block_number(1);
        let w0 = sp_core::H160::from_low_u64_be(2);
        let w1 = sp_core::H160::from_low_u64_be(1);
        assert_ok!(Sccp::set_tron_witnesses(
            RuntimeOrigin::root(),
            vec![w0, w1]
        ));
        assert!(Sccp::tron_params().is_none());

        match last_sccp_event() {
            crate::Event::TronWitnessesUpdated {
                number,
                witnesses_hash,
            } => {
                assert_eq!(number, 0);
                assert_eq!(
                    witnesses_hash,
                    H256::from_slice(&keccak_256(&vec![w1, w0].encode()))
                );
            }
            _ => panic!("unexpected event"),
        }
    });
}

#[test]
fn pause_inbound_domain_blocks_mint_from_proof() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::GreenPromise.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        set_default_domain_endpoints();
        set_default_remote_tokens(asset_id);
        assert_ok!(Sccp::activate_token(RuntimeOrigin::root(), asset_id));

        set_dummy_evm_anchor(SCCP_DOMAIN_ETH);
        assert_ok!(Sccp::set_inbound_domain_paused(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            true
        ));

        let asset_h256: H256 = asset_id.into();
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 1,
            sora_asset_id: asset_h256.0,
            amount: 10u32.into(),
            recipient: [9u8; 32],
        };

        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_ETH,
                payload,
                vec![],
            ),
            Error::<Runtime>::InboundDomainPaused
        );
    });
}

#[test]
fn invalidated_inbound_message_blocks_mint_from_proof() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::BluePromise.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        set_default_domain_endpoints();
        set_default_remote_tokens(asset_id);
        assert_ok!(Sccp::activate_token(RuntimeOrigin::root(), asset_id));
        set_dummy_evm_anchor(SCCP_DOMAIN_ETH);

        let asset_h256: H256 = asset_id.into();
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 777,
            sora_asset_id: asset_h256.0,
            amount: 10u32.into(),
            recipient: [4u8; 32],
        };
        let mut preimage = SCCP_MSG_PREFIX_BURN_V1.to_vec();
        preimage.extend(payload.encode());
        let message_id = H256::from_slice(&keccak_256(&preimage));

        assert_ok!(Sccp::invalidate_inbound_message(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            message_id
        ));

        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_ETH,
                payload.clone(),
                vec![],
            ),
            Error::<Runtime>::ProofInvalidated
        );

        assert_ok!(Sccp::clear_invalidated_inbound_message(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            message_id
        ));

        // Verification is fail-closed today; after clearing invalidation we should now fail on
        // verification, not on the invalidation gate.
        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_ETH,
                payload,
                vec![],
            ),
            Error::<Runtime>::ProofVerificationFailed
        );
    });
}

#[test]
fn set_inbound_finality_mode_rejects_unsupported_mode_for_domain() {
    let mut ext = ExtBuilder::default().build();

    ext.execute_with(|| {
        assert_noop!(
            Sccp::set_inbound_finality_mode(
                RuntimeOrigin::root(),
                SCCP_DOMAIN_ETH,
                InboundFinalityMode::SolanaLightClient,
            ),
            Error::<Runtime>::InboundFinalityModeUnsupported
        );

        assert_ok!(Sccp::set_inbound_finality_mode(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_SOL,
            InboundFinalityMode::SolanaLightClient,
        ));
        assert_eq!(
            Sccp::inbound_finality_mode_override(SCCP_DOMAIN_SOL),
            Some(InboundFinalityMode::SolanaLightClient)
        );
    });
}

#[test]
fn set_inbound_finality_mode_validates_full_domain_mode_matrix() {
    let mut ext = ExtBuilder::default().build();

    ext.execute_with(|| {
        let all_modes = vec![
            InboundFinalityMode::Disabled,
            InboundFinalityMode::EvmAnchor,
            InboundFinalityMode::BscLightClient,
            InboundFinalityMode::BscLightClientOrAnchor,
            InboundFinalityMode::TronLightClient,
            InboundFinalityMode::EthBeaconLightClient,
            InboundFinalityMode::SolanaLightClient,
            InboundFinalityMode::TonLightClient,
            InboundFinalityMode::SubstrateLightClient,
            InboundFinalityMode::AttesterQuorum,
        ];

        let support_matrix = vec![
            (
                SCCP_DOMAIN_ETH,
                vec![
                    InboundFinalityMode::Disabled,
                    InboundFinalityMode::EvmAnchor,
                    InboundFinalityMode::EthBeaconLightClient,
                    InboundFinalityMode::AttesterQuorum,
                ],
            ),
            (
                SCCP_DOMAIN_BSC,
                vec![
                    InboundFinalityMode::Disabled,
                    InboundFinalityMode::EvmAnchor,
                    InboundFinalityMode::BscLightClient,
                    InboundFinalityMode::BscLightClientOrAnchor,
                    InboundFinalityMode::AttesterQuorum,
                ],
            ),
            (
                SCCP_DOMAIN_SOL,
                vec![
                    InboundFinalityMode::Disabled,
                    InboundFinalityMode::SolanaLightClient,
                    InboundFinalityMode::AttesterQuorum,
                ],
            ),
            (
                SCCP_DOMAIN_TON,
                vec![
                    InboundFinalityMode::Disabled,
                    InboundFinalityMode::TonLightClient,
                    InboundFinalityMode::AttesterQuorum,
                ],
            ),
            (
                SCCP_DOMAIN_TRON,
                vec![
                    InboundFinalityMode::Disabled,
                    InboundFinalityMode::EvmAnchor,
                    InboundFinalityMode::TronLightClient,
                    InboundFinalityMode::AttesterQuorum,
                ],
            ),
            (
                SCCP_DOMAIN_SORA_KUSAMA,
                vec![
                    InboundFinalityMode::Disabled,
                    InboundFinalityMode::SubstrateLightClient,
                    InboundFinalityMode::AttesterQuorum,
                ],
            ),
            (
                SCCP_DOMAIN_SORA_POLKADOT,
                vec![
                    InboundFinalityMode::Disabled,
                    InboundFinalityMode::SubstrateLightClient,
                    InboundFinalityMode::AttesterQuorum,
                ],
            ),
        ];

        for (domain, supported) in support_matrix {
            for mode in all_modes.iter().copied() {
                if supported.contains(&mode) {
                    assert_ok!(Sccp::set_inbound_finality_mode(
                        RuntimeOrigin::root(),
                        domain,
                        mode
                    ));
                    assert_eq!(Sccp::inbound_finality_mode_override(domain), Some(mode));
                } else {
                    assert_noop!(
                        Sccp::set_inbound_finality_mode(RuntimeOrigin::root(), domain, mode),
                        Error::<Runtime>::InboundFinalityModeUnsupported
                    );
                }
            }
        }
    });
}

#[test]
fn mint_from_proof_fails_closed_on_corrupted_mode_domain_mismatch() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::Mango.into();

    ext.execute_with(|| {
        setup_active_token(asset_id);

        // Simulate storage corruption: mode unsupported for ETH.
        crate::InboundFinalityModes::<Runtime>::insert(
            SCCP_DOMAIN_ETH,
            InboundFinalityMode::BscLightClient,
        );

        let asset_h256: H256 = asset_id.into();
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 4401,
            sora_asset_id: asset_h256.0,
            amount: 1u32.into(),
            recipient: [0x44u8; 32],
        };

        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_ETH,
                payload,
                vec![],
            ),
            Error::<Runtime>::InboundFinalityModeUnsupported
        );
    });
}

#[test]
fn attest_burn_fails_closed_on_corrupted_mode_domain_mismatch() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::AppleTree.into();

    ext.execute_with(|| {
        setup_active_token(asset_id);

        // Simulate storage corruption: mode unsupported for ETH.
        crate::InboundFinalityModes::<Runtime>::insert(
            SCCP_DOMAIN_ETH,
            InboundFinalityMode::SolanaLightClient,
        );

        let asset_h256: H256 = asset_id.into();
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_TON,
            nonce: 4402,
            sora_asset_id: asset_h256.0,
            amount: 1u32.into(),
            recipient: [0x55u8; 32],
        };

        assert_noop!(
            Sccp::attest_burn(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_ETH,
                payload,
                vec![],
            ),
            Error::<Runtime>::InboundFinalityModeUnsupported
        );
    });
}

#[test]
fn strict_default_inbound_modes_fail_closed_until_verifiers_are_initialized() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::Apple.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        set_default_domain_endpoints();
        set_default_remote_tokens(asset_id);
        assert_ok!(Sccp::activate_token(RuntimeOrigin::root(), asset_id));

        let asset_h256: H256 = asset_id.into();
        let eth_payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 200,
            sora_asset_id: asset_h256.0,
            amount: 4u32.into(),
            recipient: [0x22u8; 32],
        };
        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_ETH,
                eth_payload,
                vec![],
            ),
            Error::<Runtime>::InboundFinalityUnavailable
        );

        let bsc_payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_BSC,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 201,
            sora_asset_id: asset_h256.0,
            amount: 5u32.into(),
            recipient: [0x33u8; 32],
        };
        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_BSC,
                bsc_payload,
                vec![],
            ),
            Error::<Runtime>::InboundFinalityUnavailable
        );

        let sol_payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SOL,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 202,
            sora_asset_id: asset_h256.0,
            amount: 6u32.into(),
            recipient: [0x44u8; 32],
        };
        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_SOL,
                sol_payload,
                vec![],
            ),
            Error::<Runtime>::InboundFinalityUnavailable
        );

        let ton_payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_TON,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 203,
            sora_asset_id: asset_h256.0,
            amount: 7u32.into(),
            recipient: [0x55u8; 32],
        };
        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_TON,
                ton_payload,
                vec![],
            ),
            Error::<Runtime>::InboundFinalityUnavailable
        );

        let tron_payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_TRON,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 204,
            sora_asset_id: asset_h256.0,
            amount: 8u32.into(),
            recipient: [0x66u8; 32],
        };
        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_TRON,
                tron_payload,
                vec![],
            ),
            Error::<Runtime>::InboundFinalityUnavailable
        );
    });
}

#[test]
fn eth_beacon_mode_uses_finalized_state_provider_for_proof_path() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::Future.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        set_default_domain_endpoints();
        set_default_remote_tokens(asset_id);
        assert_ok!(Sccp::activate_token(RuntimeOrigin::root(), asset_id));

        let asset_h256: H256 = asset_id.into();
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 205,
            sora_asset_id: asset_h256.0,
            amount: 9u32.into(),
            recipient: [0x77u8; 32],
        };

        // With no finalized ETH state provider value, ETH beacon mode is fail-closed.
        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_ETH,
                payload.clone(),
                vec![],
            ),
            Error::<Runtime>::InboundFinalityUnavailable
        );

        // Provide finalized ETH state. Availability gate passes and execution reaches proof
        // verification (empty proof => verification failure).
        set_eth_finalized_state(H256([0x11; 32]), H256([0x22; 32]));
        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_ETH,
                payload,
                vec![],
            ),
            Error::<Runtime>::ProofVerificationFailed
        );
    });
}

#[test]
fn solana_light_client_mode_uses_provider_for_proof_path() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::Future.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        set_default_domain_endpoints();
        set_default_remote_tokens(asset_id);
        assert_ok!(Sccp::activate_token(RuntimeOrigin::root(), asset_id));

        let asset_h256: H256 = asset_id.into();
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SOL,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 206,
            sora_asset_id: asset_h256.0,
            amount: 9u32.into(),
            recipient: [0x78u8; 32],
        };
        let message_id = {
            let mut preimage = SCCP_MSG_PREFIX_BURN_V1.to_vec();
            preimage.extend(payload.encode());
            H256::from_slice(&keccak_256(&preimage))
        };

        // Without a Solana finalized-proof verifier, mode is fail-closed.
        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_SOL,
                payload.clone(),
                vec![],
            ),
            Error::<Runtime>::InboundFinalityUnavailable
        );

        // Provider available, invalid proof path => verification failure.
        set_solana_finalized_verify_result(Some(false));
        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_SOL,
                payload.clone(),
                vec![],
            ),
            Error::<Runtime>::ProofVerificationFailed
        );

        // Provider available, valid proof path => mint succeeds.
        set_solana_finalized_verify_result(Some(true));
        assert_ok!(Sccp::mint_from_proof(
            RuntimeOrigin::signed(alice()),
            SCCP_DOMAIN_SOL,
            payload,
            vec![],
        ));
        assert!(Sccp::processed_inbound(message_id));
    });
}

#[test]
fn ton_light_client_mode_uses_provider_for_proof_path() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::Pan.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        set_default_domain_endpoints();
        set_default_remote_tokens(asset_id);
        assert_ok!(Sccp::activate_token(RuntimeOrigin::root(), asset_id));

        let asset_h256: H256 = asset_id.into();
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_TON,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 207,
            sora_asset_id: asset_h256.0,
            amount: 9u32.into(),
            recipient: [0x79u8; 32],
        };
        let message_id = {
            let mut preimage = SCCP_MSG_PREFIX_BURN_V1.to_vec();
            preimage.extend(payload.encode());
            H256::from_slice(&keccak_256(&preimage))
        };

        // Without a TON finalized-proof verifier, mode is fail-closed.
        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_TON,
                payload.clone(),
                vec![],
            ),
            Error::<Runtime>::InboundFinalityUnavailable
        );

        // Provider available, invalid proof path => verification failure.
        set_ton_finalized_verify_result(Some(false));
        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_TON,
                payload.clone(),
                vec![],
            ),
            Error::<Runtime>::ProofVerificationFailed
        );

        // Provider available, valid proof path => mint succeeds.
        set_ton_finalized_verify_result(Some(true));
        assert_ok!(Sccp::mint_from_proof(
            RuntimeOrigin::signed(alice()),
            SCCP_DOMAIN_TON,
            payload,
            vec![],
        ));
        assert!(Sccp::processed_inbound(message_id));
    });
}

#[test]
fn substrate_light_client_mode_uses_provider_for_proof_path() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::Pan.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        set_default_domain_endpoints();
        set_default_remote_tokens(asset_id);
        assert_ok!(Sccp::activate_token(RuntimeOrigin::root(), asset_id));

        let asset_h256: H256 = asset_id.into();
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA_KUSAMA,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 208,
            sora_asset_id: asset_h256.0,
            amount: 9u32.into(),
            recipient: [0x7au8; 32],
        };
        let message_id = {
            let mut preimage = SCCP_MSG_PREFIX_BURN_V1.to_vec();
            preimage.extend(payload.encode());
            H256::from_slice(&keccak_256(&preimage))
        };

        // Without a Substrate finalized-proof verifier, mode is fail-closed.
        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_SORA_KUSAMA,
                payload.clone(),
                vec![],
            ),
            Error::<Runtime>::InboundFinalityUnavailable
        );

        // Provider available, invalid proof path => verification failure.
        set_substrate_finalized_verify_result(Some(false));
        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_SORA_KUSAMA,
                payload.clone(),
                vec![],
            ),
            Error::<Runtime>::ProofVerificationFailed
        );

        // Provider available, valid proof path => mint succeeds.
        set_substrate_finalized_verify_result(Some(true));
        assert_ok!(Sccp::mint_from_proof(
            RuntimeOrigin::signed(alice()),
            SCCP_DOMAIN_SORA_KUSAMA,
            payload,
            vec![],
        ));
        assert!(Sccp::processed_inbound(message_id));
    });
}

#[test]
fn burn_to_sora_parachain_domains_uses_substrate_digest_network_ids() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::RedPepper.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(assets::Pallet::<Runtime>::mint_to(
            &asset_id,
            &alice(),
            &alice(),
            1_000u32.into()
        ));
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        set_default_domain_endpoints();
        set_default_remote_tokens(asset_id);
        assert_ok!(Sccp::activate_token(RuntimeOrigin::root(), asset_id));

        let amount: Balance = 10u32.into();
        let recipient = [0x55u8; 32];
        assert_ok!(Sccp::burn(
            RuntimeOrigin::signed(alice()),
            asset_id,
            amount,
            SCCP_DOMAIN_SORA_KUSAMA,
            recipient,
        ));
        let payload_kusama = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_SORA_KUSAMA,
            nonce: 1,
            sora_asset_id: H256::from(asset_id).0,
            amount,
            recipient,
        };
        let mut preimage = SCCP_MSG_PREFIX_BURN_V1.to_vec();
        preimage.extend(payload_kusama.encode());
        let kusama_message_id = H256::from_slice(&keccak_256(&preimage));
        assert_eq!(
            take_aux_digest_items(),
            vec![AuxiliaryDigestItem::Commitment(
                GenericNetworkId::Sub(SubNetworkId::Kusama),
                kusama_message_id
            )]
        );

        assert_ok!(Sccp::burn(
            RuntimeOrigin::signed(alice()),
            asset_id,
            amount,
            SCCP_DOMAIN_SORA_POLKADOT,
            recipient,
        ));
        let payload_polkadot = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_SORA_POLKADOT,
            nonce: 2,
            sora_asset_id: H256::from(asset_id).0,
            amount,
            recipient,
        };
        let mut preimage = SCCP_MSG_PREFIX_BURN_V1.to_vec();
        preimage.extend(payload_polkadot.encode());
        let polkadot_message_id = H256::from_slice(&keccak_256(&preimage));
        assert_eq!(
            take_aux_digest_items(),
            vec![AuxiliaryDigestItem::Commitment(
                GenericNetworkId::Sub(SubNetworkId::Polkadot),
                polkadot_message_id
            )]
        );
    });
}

#[test]
fn attest_burn_to_sora_parachain_domain_uses_substrate_digest_network_id() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::Pan.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        set_default_domain_endpoints();
        set_default_remote_tokens(asset_id);
        assert_ok!(Sccp::activate_token(RuntimeOrigin::root(), asset_id));

        set_solana_finalized_verify_result(Some(true));

        let asset_h256: H256 = asset_id.into();
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SOL,
            dest_domain: SCCP_DOMAIN_SORA_POLKADOT,
            nonce: 209,
            sora_asset_id: asset_h256.0,
            amount: 9u32.into(),
            recipient: [0x7bu8; 32],
        };
        let mut preimage = SCCP_MSG_PREFIX_BURN_V1.to_vec();
        preimage.extend(payload.encode());
        let message_id = H256::from_slice(&keccak_256(&preimage));

        assert_ok!(Sccp::attest_burn(
            RuntimeOrigin::signed(alice()),
            SCCP_DOMAIN_SOL,
            payload,
            vec![],
        ));
        assert_eq!(
            take_aux_digest_items(),
            vec![AuxiliaryDigestItem::Commitment(
                GenericNetworkId::Sub(SubNetworkId::Polkadot),
                message_id
            )]
        );
    });
}

#[test]
fn solana_attester_quorum_mode_allows_mint_from_proof() {
    use sp_core::ecdsa;
    use sp_core::Pair;

    fn eth_address_from_pair(pair: &ecdsa::Pair) -> sp_core::H160 {
        let msg = H256([9u8; 32]);
        let sig = pair.sign_prehashed(&msg.0);
        let pk = sp_io::crypto::secp256k1_ecdsa_recover(&sig.0, &msg.0)
            .ok()
            .unwrap();
        sp_core::H160::from_slice(&keccak_256(&pk)[12..])
    }

    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::Apple.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        set_default_domain_endpoints();
        set_default_remote_tokens(asset_id);
        assert_ok!(Sccp::activate_token(RuntimeOrigin::root(), asset_id));

        // Configure attester quorum for SOL (2-of-3).
        let p0 = ecdsa::Pair::from_seed(&[1u8; 32]);
        let p1 = ecdsa::Pair::from_seed(&[2u8; 32]);
        let p2 = ecdsa::Pair::from_seed(&[3u8; 32]);
        let a0 = eth_address_from_pair(&p0);
        let a1 = eth_address_from_pair(&p1);
        let a2 = eth_address_from_pair(&p2);
        assert_ok!(Sccp::set_inbound_attesters(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_SOL,
            vec![a0, a1, a2],
            2
        ));
        assert_ok!(Sccp::set_inbound_finality_mode(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_SOL,
            InboundFinalityMode::AttesterQuorum,
        ));

        let asset_h256: H256 = asset_id.into();
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SOL,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 900,
            sora_asset_id: asset_h256.0,
            amount: 2u32.into(),
            recipient: [0x11u8; 32],
        };
        let message_id = {
            let mut preimage = SCCP_MSG_PREFIX_BURN_V1.to_vec();
            preimage.extend(payload.encode());
            H256::from_slice(&keccak_256(&preimage))
        };

        let sig_hash = {
            let mut preimage = SCCP_MSG_PREFIX_ATTEST_V1.to_vec();
            preimage.extend_from_slice(&message_id.0);
            H256::from_slice(&keccak_256(&preimage))
        };
        let sig0 = p0.sign_prehashed(&sig_hash.0);
        let sig1 = p1.sign_prehashed(&sig_hash.0);

        let sigs: BoundedVec<[u8; 65], SccpMaxAttesters> = vec![sig0.0, sig1.0].try_into().unwrap();
        let mut proof = vec![1u8];
        proof.extend(sigs.encode());

        assert_ok!(Sccp::mint_from_proof(
            RuntimeOrigin::signed(alice()),
            SCCP_DOMAIN_SOL,
            payload,
            proof
        ));
        assert!(Sccp::processed_inbound(message_id));
    });
}

#[test]
fn ton_attester_quorum_mode_allows_mint_from_proof() {
    use sp_core::ecdsa;
    use sp_core::Pair;

    fn eth_address_from_pair(pair: &ecdsa::Pair) -> sp_core::H160 {
        let msg = H256([9u8; 32]);
        let sig = pair.sign_prehashed(&msg.0);
        let pk = sp_io::crypto::secp256k1_ecdsa_recover(&sig.0, &msg.0)
            .ok()
            .unwrap();
        sp_core::H160::from_slice(&keccak_256(&pk)[12..])
    }

    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::Tomato.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        set_default_domain_endpoints();
        set_default_remote_tokens(asset_id);
        assert_ok!(Sccp::activate_token(RuntimeOrigin::root(), asset_id));

        // Configure attester quorum for TON (2-of-3).
        let p0 = ecdsa::Pair::from_seed(&[1u8; 32]);
        let p1 = ecdsa::Pair::from_seed(&[2u8; 32]);
        let p2 = ecdsa::Pair::from_seed(&[3u8; 32]);
        let a0 = eth_address_from_pair(&p0);
        let a1 = eth_address_from_pair(&p1);
        let a2 = eth_address_from_pair(&p2);
        assert_ok!(Sccp::set_inbound_attesters(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_TON,
            vec![a0, a1, a2],
            2
        ));
        assert_ok!(Sccp::set_inbound_finality_mode(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_TON,
            InboundFinalityMode::AttesterQuorum,
        ));

        let asset_h256: H256 = asset_id.into();
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_TON,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 901,
            sora_asset_id: asset_h256.0,
            amount: 2u32.into(),
            recipient: [0x22u8; 32],
        };
        let message_id = {
            let mut preimage = SCCP_MSG_PREFIX_BURN_V1.to_vec();
            preimage.extend(payload.encode());
            H256::from_slice(&keccak_256(&preimage))
        };

        let sig_hash = {
            let mut preimage = SCCP_MSG_PREFIX_ATTEST_V1.to_vec();
            preimage.extend_from_slice(&message_id.0);
            H256::from_slice(&keccak_256(&preimage))
        };
        let sig0 = p0.sign_prehashed(&sig_hash.0);
        let sig1 = p1.sign_prehashed(&sig_hash.0);

        let sigs: BoundedVec<[u8; 65], SccpMaxAttesters> = vec![sig0.0, sig1.0].try_into().unwrap();
        let mut proof = vec![1u8];
        proof.extend(sigs.encode());

        assert_ok!(Sccp::mint_from_proof(
            RuntimeOrigin::signed(alice()),
            SCCP_DOMAIN_TON,
            payload,
            proof
        ));
        assert!(Sccp::processed_inbound(message_id));
    });
}

#[test]
fn set_inbound_attesters_rejects_duplicate_addresses() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let a0 = sp_core::H160::from_low_u64_be(1);

        assert_noop!(
            Sccp::set_inbound_attesters(RuntimeOrigin::root(), SCCP_DOMAIN_SOL, vec![a0, a0], 1),
            Error::<Runtime>::InboundAttestersInvalid
        );
        assert!(crate::InboundAttesters::<Runtime>::get(SCCP_DOMAIN_SOL).is_none());
        assert!(crate::InboundAttesterThreshold::<Runtime>::get(SCCP_DOMAIN_SOL).is_none());
    });
}

#[test]
fn set_inbound_attesters_rejects_zero_and_excess_thresholds() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let a0 = sp_core::H160::from_low_u64_be(1);
        let a1 = sp_core::H160::from_low_u64_be(2);

        assert_noop!(
            Sccp::set_inbound_attesters(RuntimeOrigin::root(), SCCP_DOMAIN_SOL, vec![a0, a1], 0),
            Error::<Runtime>::InboundAttestersInvalid
        );
        assert_noop!(
            Sccp::set_inbound_attesters(RuntimeOrigin::root(), SCCP_DOMAIN_SOL, vec![a0, a1], 3),
            Error::<Runtime>::InboundAttestersInvalid
        );
        assert!(crate::InboundAttesters::<Runtime>::get(SCCP_DOMAIN_SOL).is_none());
        assert!(crate::InboundAttesterThreshold::<Runtime>::get(SCCP_DOMAIN_SOL).is_none());
    });
}

#[test]
fn set_inbound_attesters_rejects_attester_set_overflow() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let count = SccpMaxAttesters::get() as usize + 1;
        let attesters: Vec<sp_core::H160> = (0..count)
            .map(|i| sp_core::H160::from_low_u64_be((i + 1) as u64))
            .collect();

        assert_noop!(
            Sccp::set_inbound_attesters(RuntimeOrigin::root(), SCCP_DOMAIN_SOL, attesters, 1),
            Error::<Runtime>::InboundAttestersInvalid
        );
    });
}

#[test]
fn set_inbound_attesters_rejects_non_manager_origin() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let a0 = sp_core::H160::from_low_u64_be(1);
        assert_noop!(
            Sccp::set_inbound_attesters(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_SOL,
                vec![a0],
                1
            ),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn set_inbound_attesters_rejects_sora_and_unknown_domains() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let a0 = sp_core::H160::from_low_u64_be(1);
        assert_noop!(
            Sccp::set_inbound_attesters(RuntimeOrigin::root(), SCCP_DOMAIN_SORA, vec![a0], 1),
            Error::<Runtime>::DomainUnsupported
        );
        assert_noop!(
            Sccp::set_inbound_attesters(RuntimeOrigin::root(), 99, vec![a0], 1),
            Error::<Runtime>::DomainUnsupported
        );
    });
}

#[test]
fn clear_inbound_attesters_rejects_non_manager_origin() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_noop!(
            Sccp::clear_inbound_attesters(RuntimeOrigin::signed(alice()), SCCP_DOMAIN_SOL),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn clear_inbound_attesters_rejects_sora_and_unknown_domains() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_noop!(
            Sccp::clear_inbound_attesters(RuntimeOrigin::root(), SCCP_DOMAIN_SORA),
            Error::<Runtime>::DomainUnsupported
        );
        assert_noop!(
            Sccp::clear_inbound_attesters(RuntimeOrigin::root(), 99),
            Error::<Runtime>::DomainUnsupported
        );
    });
}

#[test]
fn clear_inbound_attesters_disables_attester_quorum_fail_closed() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::Table.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        set_default_domain_endpoints();
        set_default_remote_tokens(asset_id);
        assert_ok!(Sccp::activate_token(RuntimeOrigin::root(), asset_id));

        let a0 = sp_core::H160::from_low_u64_be(1);
        let a1 = sp_core::H160::from_low_u64_be(2);
        assert_ok!(Sccp::set_inbound_attesters(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_SOL,
            vec![a0, a1],
            1,
        ));
        assert_ok!(Sccp::set_inbound_finality_mode(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_SOL,
            InboundFinalityMode::AttesterQuorum,
        ));

        assert_ok!(Sccp::clear_inbound_attesters(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_SOL
        ));
        assert!(crate::InboundAttesters::<Runtime>::get(SCCP_DOMAIN_SOL).is_none());
        assert!(crate::InboundAttesterThreshold::<Runtime>::get(SCCP_DOMAIN_SOL).is_none());

        let asset_h256: H256 = asset_id.into();
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SOL,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 777,
            sora_asset_id: asset_h256.0,
            amount: 1u32.into(),
            recipient: [0x55u8; 32],
        };
        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_SOL,
                payload,
                vec![],
            ),
            Error::<Runtime>::InboundFinalityUnavailable
        );
    });
}

#[test]
fn attester_quorum_rejects_malformed_and_trailing_bytes_proofs() {
    use sp_core::ecdsa;
    use sp_core::Pair;

    fn eth_address_from_pair(pair: &ecdsa::Pair) -> sp_core::H160 {
        let msg = H256([9u8; 32]);
        let sig = pair.sign_prehashed(&msg.0);
        let pk = sp_io::crypto::secp256k1_ecdsa_recover(&sig.0, &msg.0)
            .ok()
            .unwrap();
        sp_core::H160::from_slice(&keccak_256(&pk)[12..])
    }

    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::Future.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        set_default_domain_endpoints();
        set_default_remote_tokens(asset_id);
        assert_ok!(Sccp::activate_token(RuntimeOrigin::root(), asset_id));

        let p0 = ecdsa::Pair::from_seed(&[1u8; 32]);
        let a0 = eth_address_from_pair(&p0);
        assert_ok!(Sccp::set_inbound_attesters(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_SOL,
            vec![a0],
            1
        ));
        assert_ok!(Sccp::set_inbound_finality_mode(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_SOL,
            InboundFinalityMode::AttesterQuorum,
        ));

        let asset_h256: H256 = asset_id.into();
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SOL,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 903,
            sora_asset_id: asset_h256.0,
            amount: 1u32.into(),
            recipient: [0x77u8; 32],
        };

        // Malformed: cannot decode proof format at all.
        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_SOL,
                payload.clone(),
                vec![],
            ),
            Error::<Runtime>::ProofVerificationFailed
        );

        // Valid outer structure with trailing bytes must fail closed.
        let sigs: BoundedVec<[u8; 65], SccpMaxAttesters> = vec![].try_into().unwrap();
        let mut trailing = vec![1u8];
        trailing.extend(sigs.encode());
        trailing.push(0x42);
        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_SOL,
                payload,
                trailing,
            ),
            Error::<Runtime>::ProofVerificationFailed
        );
    });
}

#[test]
fn attester_quorum_rejects_unsupported_proof_version() {
    use sp_core::ecdsa;
    use sp_core::Pair;

    fn eth_address_from_pair(pair: &ecdsa::Pair) -> sp_core::H160 {
        let msg = H256([9u8; 32]);
        let sig = pair.sign_prehashed(&msg.0);
        let pk = sp_io::crypto::secp256k1_ecdsa_recover(&sig.0, &msg.0)
            .ok()
            .unwrap();
        sp_core::H160::from_slice(&keccak_256(&pk)[12..])
    }

    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::Teapot.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        set_default_domain_endpoints();
        set_default_remote_tokens(asset_id);
        assert_ok!(Sccp::activate_token(RuntimeOrigin::root(), asset_id));

        let p0 = ecdsa::Pair::from_seed(&[1u8; 32]);
        let a0 = eth_address_from_pair(&p0);
        assert_ok!(Sccp::set_inbound_attesters(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_SOL,
            vec![a0],
            1
        ));
        assert_ok!(Sccp::set_inbound_finality_mode(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_SOL,
            InboundFinalityMode::AttesterQuorum,
        ));

        let asset_h256: H256 = asset_id.into();
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SOL,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 904,
            sora_asset_id: asset_h256.0,
            amount: 1u32.into(),
            recipient: [0x88u8; 32],
        };

        let sigs: BoundedVec<[u8; 65], SccpMaxAttesters> = vec![].try_into().unwrap();
        let mut wrong_version = vec![2u8];
        wrong_version.extend(sigs.encode());
        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_SOL,
                payload,
                wrong_version,
            ),
            Error::<Runtime>::ProofVerificationFailed
        );
    });
}

#[test]
fn attester_quorum_rejects_duplicate_signer_signatures() {
    use sp_core::ecdsa;
    use sp_core::Pair;

    fn eth_address_from_pair(pair: &ecdsa::Pair) -> sp_core::H160 {
        let msg = H256([9u8; 32]);
        let sig = pair.sign_prehashed(&msg.0);
        let pk = sp_io::crypto::secp256k1_ecdsa_recover(&sig.0, &msg.0)
            .ok()
            .unwrap();
        sp_core::H160::from_slice(&keccak_256(&pk)[12..])
    }

    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::Pan.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        set_default_domain_endpoints();
        set_default_remote_tokens(asset_id);
        assert_ok!(Sccp::activate_token(RuntimeOrigin::root(), asset_id));

        let p0 = ecdsa::Pair::from_seed(&[1u8; 32]);
        let p1 = ecdsa::Pair::from_seed(&[2u8; 32]);
        let a0 = eth_address_from_pair(&p0);
        let a1 = eth_address_from_pair(&p1);
        assert_ok!(Sccp::set_inbound_attesters(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_SOL,
            vec![a0, a1],
            2
        ));
        assert_ok!(Sccp::set_inbound_finality_mode(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_SOL,
            InboundFinalityMode::AttesterQuorum,
        ));

        let asset_h256: H256 = asset_id.into();
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SOL,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 902,
            sora_asset_id: asset_h256.0,
            amount: 2u32.into(),
            recipient: [0x66u8; 32],
        };
        let message_id = {
            let mut preimage = SCCP_MSG_PREFIX_BURN_V1.to_vec();
            preimage.extend(payload.encode());
            H256::from_slice(&keccak_256(&preimage))
        };

        let sig_hash = {
            let mut preimage = SCCP_MSG_PREFIX_ATTEST_V1.to_vec();
            preimage.extend_from_slice(&message_id.0);
            H256::from_slice(&keccak_256(&preimage))
        };
        let sig0 = p0.sign_prehashed(&sig_hash.0);
        let sigs: BoundedVec<[u8; 65], SccpMaxAttesters> = vec![sig0.0, sig0.0].try_into().unwrap();
        let mut proof = vec![1u8];
        proof.extend(sigs.encode());

        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_SOL,
                payload,
                proof,
            ),
            Error::<Runtime>::ProofVerificationFailed
        );
        assert!(!Sccp::processed_inbound(message_id));
    });
}

#[test]
fn attester_quorum_rejects_non_attester_and_invalid_recovery_id_signatures() {
    use sp_core::ecdsa;
    use sp_core::Pair;

    fn eth_address_from_pair(pair: &ecdsa::Pair) -> sp_core::H160 {
        let msg = H256([9u8; 32]);
        let sig = pair.sign_prehashed(&msg.0);
        let pk = sp_io::crypto::secp256k1_ecdsa_recover(&sig.0, &msg.0)
            .ok()
            .unwrap();
        sp_core::H160::from_slice(&keccak_256(&pk)[12..])
    }

    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::Tomato.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        set_default_domain_endpoints();
        set_default_remote_tokens(asset_id);
        assert_ok!(Sccp::activate_token(RuntimeOrigin::root(), asset_id));

        let p0 = ecdsa::Pair::from_seed(&[1u8; 32]);
        let p1 = ecdsa::Pair::from_seed(&[2u8; 32]);
        let a0 = eth_address_from_pair(&p0);
        assert_ok!(Sccp::set_inbound_attesters(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_SOL,
            vec![a0],
            1
        ));
        assert_ok!(Sccp::set_inbound_finality_mode(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_SOL,
            InboundFinalityMode::AttesterQuorum,
        ));

        let asset_h256: H256 = asset_id.into();
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SOL,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 904,
            sora_asset_id: asset_h256.0,
            amount: 1u32.into(),
            recipient: [0x78u8; 32],
        };
        let message_id = {
            let mut preimage = SCCP_MSG_PREFIX_BURN_V1.to_vec();
            preimage.extend(payload.encode());
            H256::from_slice(&keccak_256(&preimage))
        };
        let sig_hash = {
            let mut preimage = SCCP_MSG_PREFIX_ATTEST_V1.to_vec();
            preimage.extend_from_slice(&message_id.0);
            H256::from_slice(&keccak_256(&preimage))
        };

        // Signature from non-attester should not count.
        let sig_non_attester = p1.sign_prehashed(&sig_hash.0);
        let sigs_non_attester: BoundedVec<[u8; 65], SccpMaxAttesters> =
            vec![sig_non_attester.0].try_into().unwrap();
        let mut proof_non_attester = vec![1u8];
        proof_non_attester.extend(sigs_non_attester.encode());
        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_SOL,
                payload.clone(),
                proof_non_attester,
            ),
            Error::<Runtime>::ProofVerificationFailed
        );

        // Invalid recovery id (>3) should be rejected.
        let mut sig_invalid_v = p0.sign_prehashed(&sig_hash.0).0;
        sig_invalid_v[64] = 7;
        let sigs_invalid_v: BoundedVec<[u8; 65], SccpMaxAttesters> =
            vec![sig_invalid_v].try_into().unwrap();
        let mut proof_invalid_v = vec![1u8];
        proof_invalid_v.extend(sigs_invalid_v.encode());
        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_SOL,
                payload,
                proof_invalid_v,
            ),
            Error::<Runtime>::ProofVerificationFailed
        );
    });
}

#[test]
fn attester_quorum_accepts_legacy_27_28_recovery_id_signatures() {
    use sp_core::ecdsa;
    use sp_core::Pair;

    fn eth_address_from_pair(pair: &ecdsa::Pair) -> sp_core::H160 {
        let msg = H256([9u8; 32]);
        let sig = pair.sign_prehashed(&msg.0);
        let pk = sp_io::crypto::secp256k1_ecdsa_recover(&sig.0, &msg.0)
            .ok()
            .unwrap();
        sp_core::H160::from_slice(&keccak_256(&pk)[12..])
    }

    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::Potato.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        set_default_domain_endpoints();
        set_default_remote_tokens(asset_id);
        assert_ok!(Sccp::activate_token(RuntimeOrigin::root(), asset_id));

        let p0 = ecdsa::Pair::from_seed(&[1u8; 32]);
        let a0 = eth_address_from_pair(&p0);
        assert_ok!(Sccp::set_inbound_attesters(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_SOL,
            vec![a0],
            1
        ));
        assert_ok!(Sccp::set_inbound_finality_mode(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_SOL,
            InboundFinalityMode::AttesterQuorum,
        ));

        let asset_h256: H256 = asset_id.into();
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SOL,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 905,
            sora_asset_id: asset_h256.0,
            amount: 2u32.into(),
            recipient: [0x79u8; 32],
        };
        let message_id = {
            let mut preimage = SCCP_MSG_PREFIX_BURN_V1.to_vec();
            preimage.extend(payload.encode());
            H256::from_slice(&keccak_256(&preimage))
        };
        let sig_hash = {
            let mut preimage = SCCP_MSG_PREFIX_ATTEST_V1.to_vec();
            preimage.extend_from_slice(&message_id.0);
            H256::from_slice(&keccak_256(&preimage))
        };

        // Legacy Ethereum style recovery id in {27,28}.
        let mut sig = p0.sign_prehashed(&sig_hash.0).0;
        sig[64] = sig[64].saturating_add(27);
        let sigs: BoundedVec<[u8; 65], SccpMaxAttesters> = vec![sig].try_into().unwrap();
        let mut proof = vec![1u8];
        proof.extend(sigs.encode());

        assert_ok!(Sccp::mint_from_proof(
            RuntimeOrigin::signed(alice()),
            SCCP_DOMAIN_SOL,
            payload,
            proof
        ));
        assert!(Sccp::processed_inbound(message_id));
    });
}

#[test]
fn attester_quorum_fails_closed_when_threshold_storage_is_corrupted() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::Mango.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        set_default_domain_endpoints();
        set_default_remote_tokens(asset_id);
        assert_ok!(Sccp::activate_token(RuntimeOrigin::root(), asset_id));

        let a0 = sp_core::H160::from_low_u64_be(1);
        assert_ok!(Sccp::set_inbound_attesters(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_SOL,
            vec![a0],
            1
        ));
        assert_ok!(Sccp::set_inbound_finality_mode(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_SOL,
            InboundFinalityMode::AttesterQuorum,
        ));
        // Simulate storage corruption: threshold exceeds attester set length.
        crate::InboundAttesterThreshold::<Runtime>::insert(SCCP_DOMAIN_SOL, 2);

        let asset_h256: H256 = asset_id.into();
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SOL,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 906,
            sora_asset_id: asset_h256.0,
            amount: 1u32.into(),
            recipient: [0x80u8; 32],
        };
        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_SOL,
                payload,
                vec![],
            ),
            Error::<Runtime>::InboundFinalityUnavailable
        );
    });
}

#[test]
fn bsc_light_client_mode_disables_anchor_fallback() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::Apple.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        set_default_domain_endpoints();
        set_default_remote_tokens(asset_id);
        assert_ok!(Sccp::activate_token(RuntimeOrigin::root(), asset_id));

        // Only anchor is configured; no BSC finalized header exists.
        set_dummy_evm_anchor(SCCP_DOMAIN_BSC);
        assert_ok!(Sccp::set_inbound_finality_mode(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_BSC,
            InboundFinalityMode::BscLightClient,
        ));

        let asset_h256: H256 = asset_id.into();
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_BSC,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 11,
            sora_asset_id: asset_h256.0,
            amount: 10u32.into(),
            recipient: [7u8; 32],
        };

        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_BSC,
                payload,
                vec![],
            ),
            Error::<Runtime>::InboundFinalityUnavailable
        );
    });
}

#[test]
fn bsc_evm_anchor_mode_allows_proof_path_without_light_client() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::Table.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        set_default_domain_endpoints();
        set_default_remote_tokens(asset_id);
        assert_ok!(Sccp::activate_token(RuntimeOrigin::root(), asset_id));

        set_dummy_evm_anchor(SCCP_DOMAIN_BSC);
        assert_ok!(Sccp::set_inbound_finality_mode(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_BSC,
            InboundFinalityMode::EvmAnchor,
        ));

        let asset_h256: H256 = asset_id.into();
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_BSC,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 12,
            sora_asset_id: asset_h256.0,
            amount: 10u32.into(),
            recipient: [8u8; 32],
        };

        // Finality availability passes (anchor mode), then fails on empty proof verification.
        assert_noop!(
            Sccp::mint_from_proof(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_BSC,
                payload,
                vec![],
            ),
            Error::<Runtime>::ProofVerificationFailed
        );
    });
}

#[test]
fn set_bsc_validators_rejects_duplicate_entries() {
    let mut ext = ExtBuilder::default().build();

    ext.execute_with(|| {
        let v = sp_core::H160::repeat_byte(0x11);
        assert_noop!(
            Sccp::set_bsc_validators(RuntimeOrigin::root(), vec![v, v]),
            Error::<Runtime>::BscValidatorsInvalid
        );
    });
}

#[test]
fn init_bsc_light_client_rejects_duplicate_entries() {
    let mut ext = ExtBuilder::default().build();

    ext.execute_with(|| {
        let v = sp_core::H160::repeat_byte(0x11);
        assert_noop!(
            Sccp::init_bsc_light_client(
                RuntimeOrigin::root(),
                vec![0xc0], // syntactically minimal RLP list, parser should not be reached
                vec![v, v],
                1,  // epoch_length
                1,  // confirmation_depth
                56, // chain_id
                1,  // turn_length
            ),
            Error::<Runtime>::BscValidatorsInvalid
        );
    });
}

#[test]
fn evm_storage_proof_against_anchor_mints_on_sora() {
    use rlp::RlpStream;

    fn leaf_path_for_hashed_key(key32: &[u8; 32]) -> Vec<u8> {
        // Even length (64 nibbles) leaf => 0x20 prefix, then raw key bytes.
        let mut out = Vec::with_capacity(33);
        out.push(0x20);
        out.extend_from_slice(key32);
        out
    }

    fn rlp_leaf_node(compact_path: &[u8], value: &[u8]) -> Vec<u8> {
        let mut s = RlpStream::new_list(2);
        s.append(&compact_path);
        s.append(&value);
        s.out().to_vec()
    }

    fn rlp_account_value(storage_root: H256) -> Vec<u8> {
        let mut s = RlpStream::new_list(4);
        s.append(&1u8); // nonce
        s.append(&0u8); // balance
        s.append(&storage_root.as_bytes());
        s.append(&[7u8; 32].as_slice()); // dummy code hash
        s.out().to_vec()
    }

    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::GreenPromise.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        set_default_domain_endpoints();
        set_default_remote_tokens(asset_id);
        assert_ok!(Sccp::activate_token(RuntimeOrigin::root(), asset_id));

        let asset_h256: H256 = asset_id.into();
        let amount: Balance = 10u32.into();
        let recipient = [9u8; 32];

        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 1,
            sora_asset_id: asset_h256.0,
            amount,
            recipient,
        };
        let mut preimage = SCCP_MSG_PREFIX_BURN_V1.to_vec();
        preimage.extend(payload.encode());
        let message_id = H256::from_slice(&keccak_256(&preimage));

        // Router address configured via `set_default_domain_endpoints()`.
        let router_addr = vec![11u8; 20];

        // Compute storage trie key for burns[messageId].sender.
        let mut slot_bytes = [0u8; 32];
        slot_bytes[24..].copy_from_slice(&SCCP_EVM_BURNS_MAPPING_SLOT.to_be_bytes());
        let mut slot_preimage = [0u8; 64];
        slot_preimage[..32].copy_from_slice(&message_id.0);
        slot_preimage[32..].copy_from_slice(&slot_bytes);
        let slot_base = keccak_256(&slot_preimage);
        let storage_key = keccak_256(&slot_base);

        // Storage trie: a single leaf proving a non-zero value at `storage_key`.
        let storage_path = leaf_path_for_hashed_key(&storage_key);
        let storage_value_rlp = vec![0x01u8]; // RLP(uint256(1)) = 0x01
        let storage_leaf = rlp_leaf_node(&storage_path, &storage_value_rlp);
        let storage_root = H256::from_slice(&keccak_256(&storage_leaf));

        // Account trie: a single leaf for router account, with `storageRoot = storage_root`.
        let mut addr20 = [0u8; 20];
        addr20.copy_from_slice(&router_addr);
        let account_key = keccak_256(&addr20);
        let account_path = leaf_path_for_hashed_key(&account_key);
        let account_value = rlp_account_value(storage_root);
        let account_leaf = rlp_leaf_node(&account_path, &account_value);
        let state_root = H256::from_slice(&keccak_256(&account_leaf));

        let anchor_block_hash = H256([3u8; 32]);
        assert_ok!(Sccp::set_inbound_finality_mode(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            InboundFinalityMode::EvmAnchor,
        ));
        assert_ok!(Sccp::set_evm_anchor_mode_enabled(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            true
        ));
        assert_ok!(Sccp::set_evm_inbound_anchor(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            123,
            anchor_block_hash,
            state_root,
        ));

        let proof = EvmBurnProofV1 {
            anchor_block_hash,
            account_proof: vec![account_leaf],
            storage_proof: vec![storage_leaf],
        };

        let recipient_acc = AccountId::from(recipient);
        let before = assets::Pallet::<Runtime>::free_balance(&asset_id, &recipient_acc)
            .unwrap_or_else(|_| 0u32.into());

        assert_ok!(Sccp::mint_from_proof(
            RuntimeOrigin::signed(alice()),
            SCCP_DOMAIN_ETH,
            payload,
            proof.encode(),
        ));

        let after = assets::Pallet::<Runtime>::free_balance(&asset_id, &recipient_acc).unwrap();
        assert_eq!(after - before, amount);
    });
}

#[test]
fn evm_storage_proof_against_anchor_attests_burn_and_commits_digest() {
    use rlp::RlpStream;

    fn leaf_path_for_hashed_key(key32: &[u8; 32]) -> Vec<u8> {
        // Even length (64 nibbles) leaf => 0x20 prefix, then raw key bytes.
        let mut out = Vec::with_capacity(33);
        out.push(0x20);
        out.extend_from_slice(key32);
        out
    }

    fn rlp_leaf_node(compact_path: &[u8], value: &[u8]) -> Vec<u8> {
        let mut s = RlpStream::new_list(2);
        s.append(&compact_path);
        s.append(&value);
        s.out().to_vec()
    }

    fn rlp_account_value(storage_root: H256) -> Vec<u8> {
        let mut s = RlpStream::new_list(4);
        s.append(&1u8); // nonce
        s.append(&0u8); // balance
        s.append(&storage_root.as_bytes());
        s.append(&[7u8; 32].as_slice()); // dummy code hash
        s.out().to_vec()
    }

    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::GreenPromise.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        set_default_domain_endpoints();
        set_default_remote_tokens(asset_id);
        assert_ok!(Sccp::activate_token(RuntimeOrigin::root(), asset_id));

        let asset_h256: H256 = asset_id.into();
        let amount: Balance = 10u32.into();
        let recipient = [9u8; 32];

        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SOL,
            nonce: 1,
            sora_asset_id: asset_h256.0,
            amount,
            recipient,
        };
        let mut preimage = SCCP_MSG_PREFIX_BURN_V1.to_vec();
        preimage.extend(payload.encode());
        let message_id = H256::from_slice(&keccak_256(&preimage));

        // Router address configured via `set_default_domain_endpoints()`.
        let router_addr = vec![11u8; 20];

        // Compute storage trie key for burns[messageId].sender.
        let mut slot_bytes = [0u8; 32];
        slot_bytes[24..].copy_from_slice(&SCCP_EVM_BURNS_MAPPING_SLOT.to_be_bytes());
        let mut slot_preimage = [0u8; 64];
        slot_preimage[..32].copy_from_slice(&message_id.0);
        slot_preimage[32..].copy_from_slice(&slot_bytes);
        let slot_base = keccak_256(&slot_preimage);
        let storage_key = keccak_256(&slot_base);

        // Storage trie: a single leaf proving a non-zero value at `storage_key`.
        let storage_path = leaf_path_for_hashed_key(&storage_key);
        let storage_value_rlp = vec![0x01u8]; // RLP(uint256(1)) = 0x01
        let storage_leaf = rlp_leaf_node(&storage_path, &storage_value_rlp);
        let storage_root = H256::from_slice(&keccak_256(&storage_leaf));

        // Account trie: a single leaf for router account, with `storageRoot = storage_root`.
        let mut addr20 = [0u8; 20];
        addr20.copy_from_slice(&router_addr);
        let account_key = keccak_256(&addr20);
        let account_path = leaf_path_for_hashed_key(&account_key);
        let account_value = rlp_account_value(storage_root);
        let account_leaf = rlp_leaf_node(&account_path, &account_value);
        let state_root = H256::from_slice(&keccak_256(&account_leaf));

        let anchor_block_hash = H256([3u8; 32]);
        assert_ok!(Sccp::set_inbound_finality_mode(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            InboundFinalityMode::EvmAnchor,
        ));
        assert_ok!(Sccp::set_evm_anchor_mode_enabled(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            true
        ));
        assert_ok!(Sccp::set_evm_inbound_anchor(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            123,
            anchor_block_hash,
            state_root,
        ));

        let proof = EvmBurnProofV1 {
            anchor_block_hash,
            account_proof: vec![account_leaf],
            storage_proof: vec![storage_leaf],
        };

        assert_ok!(Sccp::attest_burn(
            RuntimeOrigin::signed(alice()),
            SCCP_DOMAIN_ETH,
            payload.clone(),
            proof.encode(),
        ));

        // Committed into auxiliary digest for BEEFY+MMR proofs to other chains.
        let digest_items = take_aux_digest_items();
        assert_eq!(
            digest_items,
            vec![AuxiliaryDigestItem::Commitment(
                SCCP_DIGEST_NETWORK_ID,
                message_id
            )]
        );

        assert!(Sccp::attested_outbound(message_id));

        assert_noop!(
            Sccp::attest_burn(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_ETH,
                payload,
                proof.encode(),
            ),
            Error::<Runtime>::BurnAlreadyAttested
        );
        assert!(take_aux_digest_items().is_empty());
        assert!(Sccp::attested_outbound(message_id));
    });
}

#[test]
fn attest_burn_rejects_payload_sanity_domain_guards() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SOL,
            nonce: 1,
            sora_asset_id: [1u8; 32],
            amount: 1u32.into(),
            recipient: [2u8; 32],
        };

        let mut bad_version = payload.clone();
        bad_version.version = 2;
        assert_noop!(
            Sccp::attest_burn(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_ETH,
                bad_version,
                vec![],
            ),
            Error::<Runtime>::DomainUnsupported
        );

        let mut mismatched_source = payload.clone();
        mismatched_source.source_domain = SCCP_DOMAIN_SOL;
        assert_noop!(
            Sccp::attest_burn(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_ETH,
                mismatched_source,
                vec![],
            ),
            Error::<Runtime>::DomainUnsupported
        );

        assert_noop!(
            Sccp::attest_burn(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_SORA,
                payload.clone(),
                vec![],
            ),
            Error::<Runtime>::DomainUnsupported
        );

        let mut dest_sora = payload.clone();
        dest_sora.dest_domain = SCCP_DOMAIN_SORA;
        assert_noop!(
            Sccp::attest_burn(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_ETH,
                dest_sora,
                vec![],
            ),
            Error::<Runtime>::DomainUnsupported
        );

        let mut dest_same = payload;
        dest_same.dest_domain = SCCP_DOMAIN_ETH;
        assert_noop!(
            Sccp::attest_burn(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_ETH,
                dest_same,
                vec![],
            ),
            Error::<Runtime>::DomainUnsupported
        );
    });
}

#[test]
fn attest_burn_rejects_zero_amount_zero_recipient_and_missing_source_endpoint() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let base = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SOL,
            dest_domain: SCCP_DOMAIN_TON,
            nonce: 2,
            sora_asset_id: [3u8; 32],
            amount: 1u32.into(),
            recipient: [4u8; 32],
        };

        let mut amount_zero = base.clone();
        amount_zero.amount = 0u32.into();
        assert_noop!(
            Sccp::attest_burn(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_SOL,
                amount_zero,
                vec![],
            ),
            Error::<Runtime>::AmountIsZero
        );

        let mut recipient_zero = base.clone();
        recipient_zero.recipient = [0u8; 32];
        assert_noop!(
            Sccp::attest_burn(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_SOL,
                recipient_zero,
                vec![],
            ),
            Error::<Runtime>::RecipientIsZero
        );

        // No source endpoint configured for SOL in this isolated setup.
        assert_noop!(
            Sccp::attest_burn(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_SOL,
                base,
                vec![]
            ),
            Error::<Runtime>::DomainEndpointMissing
        );
    });
}

#[test]
fn attest_burn_rejects_token_not_found_before_remote_token_checks() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        set_default_domain_endpoints();
        set_solana_finalized_verify_result(Some(true));
        assert_ok!(Sccp::set_inbound_finality_mode(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_SOL,
            InboundFinalityMode::SolanaLightClient,
        ));

        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SOL,
            dest_domain: SCCP_DOMAIN_TON,
            nonce: 3,
            sora_asset_id: [9u8; 32],
            amount: 1u32.into(),
            recipient: [5u8; 32],
        };
        assert_noop!(
            Sccp::attest_burn(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_SOL,
                payload,
                vec![]
            ),
            Error::<Runtime>::TokenNotFound
        );
    });
}

#[test]
fn attest_burn_rejects_inbound_disabled_token_state() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::Flower.into();

    ext.execute_with(|| {
        setup_active_token(asset_id);
        set_dummy_evm_anchor(SCCP_DOMAIN_ETH);

        crate::pallet::Tokens::<Runtime>::mutate(asset_id, |state| {
            let s = state.as_mut().expect("token exists");
            s.inbound_enabled = false;
        });

        let asset_h256: H256 = asset_id.into();
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SOL,
            nonce: 4,
            sora_asset_id: asset_h256.0,
            amount: 1u32.into(),
            recipient: [6u8; 32],
        };
        assert_noop!(
            Sccp::attest_burn(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_ETH,
                payload,
                vec![],
            ),
            Error::<Runtime>::InboundDisabled
        );
    });
}

#[test]
fn attest_burn_respects_inbound_and_outbound_pause_controls() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::Table.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        set_default_domain_endpoints();
        set_default_remote_tokens(asset_id);
        assert_ok!(Sccp::activate_token(RuntimeOrigin::root(), asset_id));
        // Make ETH inbound finality available so the invalidation gate is reached.
        set_dummy_evm_anchor(SCCP_DOMAIN_ETH);

        let asset_h256: H256 = asset_id.into();
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SOL,
            nonce: 41,
            sora_asset_id: asset_h256.0,
            amount: 1u32.into(),
            recipient: [0x77u8; 32],
        };

        assert_ok!(Sccp::set_inbound_domain_paused(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            true
        ));
        assert_noop!(
            Sccp::attest_burn(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_ETH,
                payload.clone(),
                vec![],
            ),
            Error::<Runtime>::InboundDomainPaused
        );

        assert_ok!(Sccp::set_inbound_domain_paused(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            false
        ));
        assert_ok!(Sccp::set_outbound_domain_paused(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_SOL,
            true
        ));
        assert_noop!(
            Sccp::attest_burn(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_ETH,
                payload,
                vec![],
            ),
            Error::<Runtime>::OutboundDomainPaused
        );
    });
}

#[test]
fn attest_burn_rejects_invalidated_message() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::Table.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        set_default_domain_endpoints();
        set_default_remote_tokens(asset_id);
        assert_ok!(Sccp::activate_token(RuntimeOrigin::root(), asset_id));
        set_dummy_evm_anchor(SCCP_DOMAIN_ETH);

        let asset_h256: H256 = asset_id.into();
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SOL,
            nonce: 42,
            sora_asset_id: asset_h256.0,
            amount: 1u32.into(),
            recipient: [0x78u8; 32],
        };

        let mut preimage = SCCP_MSG_PREFIX_BURN_V1.to_vec();
        preimage.extend(payload.encode());
        let message_id = H256::from_slice(&keccak_256(&preimage));

        assert_ok!(Sccp::invalidate_inbound_message(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_ETH,
            message_id
        ));
        assert_noop!(
            Sccp::attest_burn(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_ETH,
                payload,
                vec![],
            ),
            Error::<Runtime>::ProofInvalidated
        );
    });
}

#[test]
fn attest_burn_enforces_canonical_recipient_for_evm_destination() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::Table.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        set_default_domain_endpoints();
        set_default_remote_tokens(asset_id);
        assert_ok!(Sccp::activate_token(RuntimeOrigin::root(), asset_id));

        let asset_h256: H256 = asset_id.into();
        let mut recipient = [0u8; 32];
        recipient[0] = 1; // non-canonical for EVM (high 12 bytes must be zero)
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_BSC,
            nonce: 43,
            sora_asset_id: asset_h256.0,
            amount: 1u32.into(),
            recipient,
        };

        assert_noop!(
            Sccp::attest_burn(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_ETH,
                payload,
                vec![],
            ),
            Error::<Runtime>::RecipientNotCanonical
        );
    });
}

#[test]
fn attest_burn_fails_closed_when_source_finality_is_unavailable() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::Table.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        set_default_domain_endpoints();
        set_default_remote_tokens(asset_id);
        assert_ok!(Sccp::activate_token(RuntimeOrigin::root(), asset_id));

        let asset_h256: H256 = asset_id.into();
        // EVM destination requires canonical encoding (right-aligned 20-byte address).
        let mut recipient = [0u8; 32];
        recipient[12..].copy_from_slice(&[0x22u8; 20]);
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SOL,
            dest_domain: SCCP_DOMAIN_ETH,
            nonce: 44,
            sora_asset_id: asset_h256.0,
            amount: 1u32.into(),
            recipient,
        };

        assert_noop!(
            Sccp::attest_burn(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_SOL,
                payload,
                vec![],
            ),
            Error::<Runtime>::InboundFinalityUnavailable
        );
    });
}

#[test]
fn attest_burn_eth_requires_explicit_anchor_override_when_eth_light_client_unavailable() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::BluePromise.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        set_default_domain_endpoints();
        set_default_remote_tokens(asset_id);
        assert_ok!(Sccp::activate_token(RuntimeOrigin::root(), asset_id));

        let asset_h256: H256 = asset_id.into();
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SOL,
            nonce: 501,
            sora_asset_id: asset_h256.0,
            amount: 10u32.into(),
            recipient: [0x77u8; 32],
        };

        // Default ETH finality mode is EthBeaconLightClient and is fail-closed until integrated.
        assert_noop!(
            Sccp::attest_burn(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_ETH,
                payload.clone(),
                vec![],
            ),
            Error::<Runtime>::InboundFinalityUnavailable
        );

        // Explicit governance override to EVM anchor mode enables the proof path.
        set_dummy_evm_anchor(SCCP_DOMAIN_ETH);
        assert_noop!(
            Sccp::attest_burn(
                RuntimeOrigin::signed(alice()),
                SCCP_DOMAIN_ETH,
                payload,
                vec![],
            ),
            Error::<Runtime>::ProofVerificationFailed
        );
    });
}

#[test]
fn bsc_light_client_finalized_state_root_allows_mint_from_proof() {
    use rlp::RlpStream;
    use sp_core::ecdsa;
    use sp_core::Pair;
    use sp_core::H160;

    fn leaf_path_for_hashed_key(key32: &[u8; 32]) -> Vec<u8> {
        // Even length (64 nibbles) leaf => 0x20 prefix, then raw key bytes.
        let mut out = Vec::with_capacity(33);
        out.push(0x20);
        out.extend_from_slice(key32);
        out
    }

    fn rlp_leaf_node(compact_path: &[u8], value: &[u8]) -> Vec<u8> {
        let mut s = RlpStream::new_list(2);
        s.append(&compact_path);
        s.append(&value);
        s.out().to_vec()
    }

    fn rlp_account_value(storage_root: H256) -> Vec<u8> {
        let mut s = RlpStream::new_list(4);
        s.append(&1u8); // nonce
        s.append(&0u8); // balance
        s.append(&storage_root.as_bytes());
        s.append(&[7u8; 32].as_slice()); // dummy code hash
        s.out().to_vec()
    }

    fn header_rlp(
        parent_hash: H256,
        beneficiary: H160,
        state_root: H256,
        number: u64,
        difficulty: u64,
        extra_data: &[u8],
    ) -> Vec<u8> {
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

    fn eth_address_from_pair(pair: &ecdsa::Pair) -> H160 {
        let msg = H256([9u8; 32]);
        let sig = pair.sign_prehashed(&msg.0);
        let pk = sp_io::crypto::secp256k1_ecdsa_recover(&sig.0, &msg.0)
            .ok()
            .unwrap();
        H160::from_slice(&keccak_256(&pk)[12..])
    }

    fn build_signed_header(
        parent_hash: H256,
        state_root: H256,
        number: u64,
        difficulty: u64,
        signer: &ecdsa::Pair,
    ) -> (Vec<u8>, H256) {
        let vanity = [0u8; 32];
        let chain_id = 56u64;
        let beneficiary = eth_address_from_pair(signer);
        let seal_rlp = {
            // Mirror `bnb-chain/bsc/core/types.EncodeSigHeader` for legacy 15-field headers:
            // rlp([chainId, header_fields..., extra_no_sig]).
            let ommers_hash = [0x11u8; 32];
            let tx_root = [0x33u8; 32];
            let receipts_root = [0x44u8; 32];
            let logs_bloom = [0u8; 256];
            let gas_limit = 1_000_000u64;
            let gas_used = 0u64;
            let timestamp = number;
            let mix_hash = [0u8; 32];
            let nonce = [0u8; 8];

            let mut s = RlpStream::new_list(16);
            s.append(&chain_id);
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
            s.append(&vanity.as_slice()); // extra without signature
            s.append(&mix_hash.as_ref());
            s.append(&nonce.as_ref());
            s.out().to_vec()
        };
        let seal_hash = H256::from_slice(&keccak_256(&seal_rlp));
        let sig = signer.sign_prehashed(&seal_hash.0);

        let mut extra = vanity.to_vec();
        extra.extend_from_slice(&sig.0);
        let full_rlp = header_rlp(
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

    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::BlackPepper.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        set_default_domain_endpoints();
        set_default_remote_tokens(asset_id);
        assert_ok!(Sccp::activate_token(RuntimeOrigin::root(), asset_id));

        // Burn payload on BSC -> mint on SORA.
        let asset_h256: H256 = asset_id.into();
        let amount: Balance = 10u32.into();
        let recipient = [8u8; 32];
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_BSC,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 1,
            sora_asset_id: asset_h256.0,
            amount,
            recipient,
        };
        let mut preimage = SCCP_MSG_PREFIX_BURN_V1.to_vec();
        preimage.extend(payload.encode());
        let message_id = H256::from_slice(&keccak_256(&preimage));

        // Router address configured via `set_default_domain_endpoints()`.
        let router_addr = vec![12u8; 20];

        // Compute storage trie key for burns[messageId].sender.
        let mut slot_bytes = [0u8; 32];
        slot_bytes[24..].copy_from_slice(&SCCP_EVM_BURNS_MAPPING_SLOT.to_be_bytes());
        let mut slot_preimage = [0u8; 64];
        slot_preimage[..32].copy_from_slice(&message_id.0);
        slot_preimage[32..].copy_from_slice(&slot_bytes);
        let slot_base = keccak_256(&slot_preimage);
        let storage_key = keccak_256(&slot_base);

        // Storage trie: a single leaf proving a non-zero value at `storage_key`.
        let storage_path = leaf_path_for_hashed_key(&storage_key);
        let storage_value_rlp = vec![0x01u8]; // RLP(uint256(1)) = 0x01
        let storage_leaf = rlp_leaf_node(&storage_path, &storage_value_rlp);
        let storage_root = H256::from_slice(&keccak_256(&storage_leaf));

        // Account trie: a single leaf for router account, with `storageRoot = storage_root`.
        let mut addr20 = [0u8; 20];
        addr20.copy_from_slice(&router_addr);
        let account_key = keccak_256(&addr20);
        let account_path = leaf_path_for_hashed_key(&account_key);
        let account_value = rlp_account_value(storage_root);
        let account_leaf = rlp_leaf_node(&account_path, &account_value);
        let state_root = H256::from_slice(&keccak_256(&account_leaf));

        // Initialize BSC light client with 3 validators.
        let v0 = ecdsa::Pair::from_seed(&[1u8; 32]);
        let v1 = ecdsa::Pair::from_seed(&[2u8; 32]);
        let v2 = ecdsa::Pair::from_seed(&[3u8; 32]);
        let mut validators = vec![
            (eth_address_from_pair(&v0), v0),
            (eth_address_from_pair(&v1), v1),
            (eth_address_from_pair(&v2), v2),
        ];
        validators.sort_by_key(|(addr, _)| addr.0);
        let validator_addrs: Vec<H160> = validators.iter().map(|(a, _)| *a).collect();

        // Checkpoint header #0 signed by validator[0].
        let (checkpoint_rlp, checkpoint_hash) =
            build_signed_header(H256([0u8; 32]), H256([1u8; 32]), 0, 2, &validators[0].1);
        assert_ok!(Sccp::init_bsc_light_client(
            RuntimeOrigin::root(),
            checkpoint_rlp,
            validator_addrs.clone(),
            200,
            0,  // finalized == head
            56, // BSC chain id
            1,  // turn length
        ));

        // Header #1 contains the desired `state_root`, signed by validator[1].
        let (h1_rlp, h1_hash) =
            build_signed_header(checkpoint_hash, state_root, 1, 2, &validators[1].1);
        assert_ok!(Sccp::submit_bsc_header(
            RuntimeOrigin::signed(alice()),
            h1_rlp
        ));

        // Mint proof must target the finalized block hash.
        let proof = EvmBurnProofV1 {
            anchor_block_hash: h1_hash,
            account_proof: vec![account_leaf],
            storage_proof: vec![storage_leaf],
        };

        let recipient_acc = AccountId::from(recipient);
        let before = assets::Pallet::<Runtime>::free_balance(&asset_id, &recipient_acc)
            .unwrap_or_else(|_| 0u32.into());

        assert_ok!(Sccp::mint_from_proof(
            RuntimeOrigin::signed(alice()),
            SCCP_DOMAIN_BSC,
            payload,
            proof.encode(),
        ));

        let after = assets::Pallet::<Runtime>::free_balance(&asset_id, &recipient_acc).unwrap();
        assert_eq!(after - before, amount);
    });
}

#[test]
fn bsc_light_client_accepts_real_bsc_header_fixture() {
    use core::str::FromStr;
    use sp_core::{H160, H256};

    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        // Historical BSC mainnet header fixture.
        // Block: 81,094,034
        // Hash: 0x61a7d2bdc5faf4bac24fc5f3fbeb4c810b05bc41b37fd1b8e86a26a66027225f
        // Miner/signer: 0x9f1b7fae54be07f4fee34eb1aacb39a1f7b6fc92
        let header_rlp = include_bytes!("fixtures/bsc_header_81094034.rlp").to_vec();
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
            1000, // Maxwell-era epoch length
            0,    // finalized == head
            56,   // BSC mainnet chain id
            16,   // Maxwell-era turn length (sprint length)
        ));

        let head = Sccp::bsc_head().unwrap();
        assert_eq!(head.hash, expected_hash);
        assert_eq!(head.number, 81_094_034);
        assert_eq!(head.state_root, expected_state_root);
        assert_eq!(head.signer, signer);

        let finalized = Sccp::bsc_finalized().unwrap();
        assert_eq!(finalized.hash, expected_hash);
        assert_eq!(finalized.number, 81_094_034);
    });
}

#[test]
fn submit_bsc_header_rejects_too_large_header() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let huge = vec![0u8; crate::SCCP_MAX_BSC_HEADER_RLP_BYTES + 1];
        assert_noop!(
            Sccp::submit_bsc_header(RuntimeOrigin::signed(alice()), huge),
            Error::<Runtime>::BscHeaderTooLarge
        );
    });
}

#[test]
fn submit_bsc_header_rejects_when_light_client_not_initialized() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_noop!(
            Sccp::submit_bsc_header(RuntimeOrigin::signed(alice()), vec![0u8; 1]),
            Error::<Runtime>::BscLightClientNotInitialized
        );
    });
}

#[test]
fn submit_bsc_header_rejects_parent_mismatch_and_recent_signer_rule() {
    use rlp::RlpStream;
    use sp_core::ecdsa;
    use sp_core::Pair;
    use sp_core::H160;

    fn eth_address_from_pair(pair: &ecdsa::Pair) -> H160 {
        let msg = H256([9u8; 32]);
        let sig = pair.sign_prehashed(&msg.0);
        let pk = sp_io::crypto::secp256k1_ecdsa_recover(&sig.0, &msg.0)
            .ok()
            .unwrap();
        H160::from_slice(&keccak_256(&pk)[12..])
    }

    fn header_rlp(
        parent_hash: H256,
        beneficiary: H160,
        state_root: H256,
        number: u64,
        difficulty: u64,
        extra_data: &[u8],
    ) -> Vec<u8> {
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

    fn build_signed_header(
        parent_hash: H256,
        state_root: H256,
        number: u64,
        difficulty: u64,
        signer: &ecdsa::Pair,
        chain_id: u64,
    ) -> (Vec<u8>, H256) {
        let vanity = [0u8; 32];
        let beneficiary = eth_address_from_pair(signer);
        let seal_rlp = {
            let ommers_hash = [0x11u8; 32];
            let tx_root = [0x33u8; 32];
            let receipts_root = [0x44u8; 32];
            let logs_bloom = [0u8; 256];
            let gas_limit = 1_000_000u64;
            let gas_used = 0u64;
            let timestamp = number;
            let mix_hash = [0u8; 32];
            let nonce = [0u8; 8];

            let mut s = RlpStream::new_list(16);
            s.append(&chain_id);
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
            s.append(&vanity.as_slice()); // extra without signature
            s.append(&mix_hash.as_ref());
            s.append(&nonce.as_ref());
            s.out().to_vec()
        };
        let seal_hash = H256::from_slice(&keccak_256(&seal_rlp));
        let sig = signer.sign_prehashed(&seal_hash.0);

        let mut extra = vanity.to_vec();
        extra.extend_from_slice(&sig.0);
        let full_rlp = header_rlp(
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

    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let v0 = ecdsa::Pair::from_seed(&[1u8; 32]);
        let v1 = ecdsa::Pair::from_seed(&[2u8; 32]);
        let mut validators = vec![
            (eth_address_from_pair(&v0), v0),
            (eth_address_from_pair(&v1), v1),
        ];
        validators.sort_by_key(|(addr, _)| addr.0);
        let validator_addrs: Vec<H160> = validators.iter().map(|(a, _)| *a).collect();

        let (checkpoint_rlp, checkpoint_hash) =
            build_signed_header(H256([0u8; 32]), H256([1u8; 32]), 0, 2, &validators[0].1, 56);
        assert_ok!(Sccp::init_bsc_light_client(
            RuntimeOrigin::root(),
            checkpoint_rlp,
            validator_addrs,
            200,
            0,
            56,
            1,
        ));

        // Re-submitting a non-advancing block number is rejected.
        let (non_linear_h0, _) =
            build_signed_header(H256([0u8; 32]), H256([1u8; 32]), 0, 2, &validators[0].1, 56);
        assert_noop!(
            Sccp::submit_bsc_header(RuntimeOrigin::signed(alice()), non_linear_h0),
            Error::<Runtime>::BscHeaderInvalid
        );

        // Number is valid (+1), but parent hash is not the current head.
        let (bad_parent_h1, _) =
            build_signed_header(H256([9u8; 32]), H256([2u8; 32]), 1, 2, &validators[1].1, 56);
        assert_noop!(
            Sccp::submit_bsc_header(RuntimeOrigin::signed(alice()), bad_parent_h1),
            Error::<Runtime>::BscHeaderInvalid
        );

        // Header #1 signer is in-turn, so difficulty must be 2.
        let (bad_diff_h1, _) =
            build_signed_header(checkpoint_hash, H256([2u8; 32]), 1, 1, &validators[1].1, 56);
        assert_noop!(
            Sccp::submit_bsc_header(RuntimeOrigin::signed(alice()), bad_diff_h1),
            Error::<Runtime>::BscHeaderInvalid
        );

        // Header #1 signed by validator[0] is out-of-turn but valid with difficulty=1.
        let (h1_rlp, h1_hash) =
            build_signed_header(checkpoint_hash, H256([3u8; 32]), 1, 1, &validators[0].1, 56);
        assert_ok!(Sccp::submit_bsc_header(
            RuntimeOrigin::signed(alice()),
            h1_rlp
        ));

        // Header #2 by the same signer violates recent-signer rule (turn_length=1).
        let (h2_rlp, _) = build_signed_header(h1_hash, H256([4u8; 32]), 2, 2, &validators[0].1, 56);
        assert_noop!(
            Sccp::submit_bsc_header(RuntimeOrigin::signed(alice()), h2_rlp),
            Error::<Runtime>::BscHeaderInvalid
        );
    });
}

#[test]
fn bsc_light_client_rejects_malleable_high_s_header_signature() {
    use core::str::FromStr;

    use rlp::{Rlp, RlpStream};
    use sp_core::{H160, U256};

    const SECP256K1N: [u8; 32] = [
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xfe, 0xba, 0xae, 0xdc, 0xe6, 0xaf, 0x48, 0xa0, 0x3b, 0xbf, 0xd2, 0x5e, 0x8c, 0xd0, 0x36,
        0x41, 0x41,
    ];
    const SECP256K1N_HALF_ORDER: [u8; 32] = [
        0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x5d, 0x57, 0x6e, 0x73, 0x57, 0xa4, 0x50, 0x1d, 0xdf, 0xe9, 0x2f, 0x46, 0x68, 0x1b,
        0x20, 0xa0,
    ];

    fn malleate_high_s(header_rlp: &[u8]) -> Vec<u8> {
        let hdr = Rlp::new(header_rlp);
        assert!(hdr.is_list(), "header must be an RLP list");
        let n = hdr.item_count().expect("rlp item_count") as usize;
        assert!(n >= 15, "expected >=15 header fields");

        let mut fields: Vec<Vec<u8>> = (0..n)
            .map(|i| hdr.at(i).unwrap().data().unwrap().to_vec())
            .collect();

        // extraData is field 12 in legacy 15-field headers (and remains in that position when
        // optional fields are appended).
        let mut extra = fields[12].clone();
        assert!(
            extra.len() >= 32 + 65,
            "extraData must include a 65-byte signature"
        );
        let sig_start = extra.len() - 65;

        let mut sig = [0u8; 65];
        sig.copy_from_slice(&extra[sig_start..]);

        // Flip s -> n - s and flip recovery parity to obtain a malleable signature.
        let n_u = U256::from_big_endian(&SECP256K1N);
        let half_u = U256::from_big_endian(&SECP256K1N_HALF_ORDER);
        let mut s_bytes = [0u8; 32];
        s_bytes.copy_from_slice(&sig[32..64]);
        let s_u = U256::from_big_endian(&s_bytes);
        assert!(s_u <= half_u, "fixture signature must be canonical low-s");

        let s2_u = n_u - s_u;
        assert!(s2_u > half_u, "malleated signature must be high-s");
        s2_u.to_big_endian(&mut s_bytes);
        sig[32..64].copy_from_slice(&s_bytes);
        sig[64] ^= 1;

        extra[sig_start..].copy_from_slice(&sig);
        fields[12] = extra;

        let mut out = RlpStream::new_list(n);
        for f in fields {
            out.append(&f.as_slice());
        }
        out.out().to_vec()
    }

    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        // Historical BSC mainnet header fixture (known-valid).
        let header_rlp = include_bytes!("fixtures/bsc_header_81094034.rlp").to_vec();
        let signer = H160::from_str("0x9f1b7fae54be07f4fee34eb1aacb39a1f7b6fc92").unwrap();

        let bad = malleate_high_s(&header_rlp);

        assert_noop!(
            Sccp::init_bsc_light_client(
                RuntimeOrigin::root(),
                bad,
                vec![signer],
                1000, // Maxwell-era epoch length
                0,    // finalized == head
                56,   // BSC mainnet chain id
                16,   // Maxwell-era turn length (sprint length)
            ),
            Error::<Runtime>::BscHeaderInvalid
        );
    });
}

#[test]
fn submit_tron_header_rejects_too_large_raw_data() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let huge = vec![0u8; crate::SCCP_MAX_TRON_RAW_DATA_BYTES + 1];
        let sig = vec![0u8; 65];
        assert_noop!(
            Sccp::submit_tron_header(RuntimeOrigin::signed(alice()), huge, sig),
            Error::<Runtime>::TronHeaderTooLarge
        );
    });
}

#[test]
fn submit_tron_header_rejects_invalid_signature_length() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_noop!(
            Sccp::submit_tron_header(RuntimeOrigin::signed(alice()), vec![0u8; 1], vec![0u8; 64]),
            Error::<Runtime>::TronHeaderInvalid
        );
    });
}

#[test]
fn submit_tron_header_rejects_when_light_client_not_initialized() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_noop!(
            Sccp::submit_tron_header(RuntimeOrigin::signed(alice()), vec![0u8; 1], vec![0u8; 65]),
            Error::<Runtime>::TronLightClientNotInitialized
        );
    });
}

#[test]
fn submit_tron_header_rejects_non_linear_and_parent_mismatch_extensions() {
    use sp_core::ecdsa;
    use sp_core::Pair;
    use sp_core::H160;

    fn pb_varint(mut v: u64) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            let mut b = (v & 0x7f) as u8;
            v >>= 7;
            if v != 0 {
                b |= 0x80;
            }
            out.push(b);
            if v == 0 {
                break;
            }
        }
        out
    }

    fn eth_address_from_pair(pair: &ecdsa::Pair) -> H160 {
        let msg = H256([9u8; 32]);
        let sig = pair.sign_prehashed(&msg.0);
        let pk = sp_io::crypto::secp256k1_ecdsa_recover(&sig.0, &msg.0)
            .ok()
            .unwrap();
        H160::from_slice(&keccak_256(&pk)[12..])
    }

    fn tron_raw_data(
        parent_hash: H256,
        number: u64,
        witness_address: &[u8; 21],
        state_root: H256,
    ) -> Vec<u8> {
        let mut raw_data = Vec::new();
        raw_data.push(0x1a); // parentHash (field 3, bytes)
        raw_data.extend_from_slice(&pb_varint(32));
        raw_data.extend_from_slice(parent_hash.as_bytes());
        raw_data.push(0x38); // number (field 7, varint)
        raw_data.extend_from_slice(&pb_varint(number));
        raw_data.push(0x4a); // witness_address (field 9, bytes)
        raw_data.extend_from_slice(&pb_varint(21));
        raw_data.extend_from_slice(witness_address);
        raw_data.push(0x5a); // accountStateRoot (field 11, bytes)
        raw_data.extend_from_slice(&pb_varint(32));
        raw_data.extend_from_slice(state_root.as_bytes());
        raw_data
    }

    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let p0 = ecdsa::Pair::from_seed(&[1u8; 32]);
        let w0 = eth_address_from_pair(&p0);

        let mut witness_addr = [0u8; 21];
        witness_addr[0] = 0x41;
        witness_addr[1..].copy_from_slice(w0.as_bytes());

        let checkpoint_raw = tron_raw_data(H256([0u8; 32]), 1, &witness_addr, H256([1u8; 32]));
        let checkpoint_hash = sp_io::hashing::sha2_256(&checkpoint_raw);
        let checkpoint_sig = p0.sign_prehashed(&checkpoint_hash).0.to_vec();

        assert_ok!(Sccp::init_tron_light_client(
            RuntimeOrigin::root(),
            checkpoint_raw.clone(),
            checkpoint_sig.clone(),
            vec![w0],
            0x41,
        ));

        // Re-submitting the current head is rejected (non-linear extension).
        assert_noop!(
            Sccp::submit_tron_header(
                RuntimeOrigin::signed(alice()),
                checkpoint_raw,
                checkpoint_sig
            ),
            Error::<Runtime>::TronHeaderInvalid
        );

        // Number is valid (+1), but parent hash mismatches current head.
        let bad_parent_raw = tron_raw_data(H256([9u8; 32]), 2, &witness_addr, H256([2u8; 32]));
        let bad_parent_hash = sp_io::hashing::sha2_256(&bad_parent_raw);
        let bad_parent_sig = p0.sign_prehashed(&bad_parent_hash).0.to_vec();
        assert_noop!(
            Sccp::submit_tron_header(
                RuntimeOrigin::signed(alice()),
                bad_parent_raw,
                bad_parent_sig
            ),
            Error::<Runtime>::TronHeaderInvalid
        );
    });
}

#[test]
fn init_tron_light_client_rejects_duplicate_entries() {
    use sp_core::ecdsa;
    use sp_core::Pair;

    fn eth_address_from_pair(pair: &ecdsa::Pair) -> sp_core::H160 {
        let msg = sp_core::H256([9u8; 32]);
        let sig = pair.sign_prehashed(&msg.0);
        let pk = sp_io::crypto::secp256k1_ecdsa_recover(&sig.0, &msg.0)
            .ok()
            .unwrap();
        sp_core::H160::from_slice(&keccak_256(&pk)[12..])
    }

    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let p0 = ecdsa::Pair::from_seed(&[1u8; 32]);
        let a0 = eth_address_from_pair(&p0);

        // Minimal raw_data with required fields (content irrelevant for duplicate rejection).
        let raw_data = vec![0u8; 10];
        let sig = p0.sign_prehashed(&[0u8; 32]).0.to_vec();

        assert_noop!(
            Sccp::init_tron_light_client(RuntimeOrigin::root(), raw_data, sig, vec![a0, a0], 0x41,),
            Error::<Runtime>::TronWitnessesInvalid
        );
    });
}

#[test]
fn tron_light_client_finalized_state_root_allows_mint_from_proof() {
    use rlp::RlpStream;
    use sp_core::ecdsa;
    use sp_core::Pair;
    use sp_core::H160;

    fn pb_varint(mut v: u64) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            let mut b = (v & 0x7f) as u8;
            v >>= 7;
            if v != 0 {
                b |= 0x80;
            }
            out.push(b);
            if v == 0 {
                break;
            }
        }
        out
    }

    fn eth_address_from_pair(pair: &ecdsa::Pair) -> H160 {
        let msg = sp_core::H256([9u8; 32]);
        let sig = pair.sign_prehashed(&msg.0);
        let pk = sp_io::crypto::secp256k1_ecdsa_recover(&sig.0, &msg.0)
            .ok()
            .unwrap();
        H160::from_slice(&keccak_256(&pk)[12..])
    }

    fn leaf_path_for_hashed_key(key32: &[u8; 32]) -> Vec<u8> {
        // Even length (64 nibbles) leaf => 0x20 prefix, then raw key bytes.
        let mut out = Vec::with_capacity(33);
        out.push(0x20);
        out.extend_from_slice(key32);
        out
    }

    fn rlp_leaf_node(compact_path: &[u8], value: &[u8]) -> Vec<u8> {
        let mut s = RlpStream::new_list(2);
        s.append(&compact_path);
        s.append(&value);
        s.out().to_vec()
    }

    fn rlp_account_value(storage_root: sp_core::H256) -> Vec<u8> {
        let mut s = RlpStream::new_list(4);
        s.append(&1u8); // nonce
        s.append(&0u8); // balance
        s.append(&storage_root.as_bytes());
        s.append(&[7u8; 32].as_slice()); // dummy code hash
        s.out().to_vec()
    }

    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::GreenPromise.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id);
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        set_default_domain_endpoints();
        set_default_remote_tokens(asset_id);
        assert_ok!(Sccp::activate_token(RuntimeOrigin::root(), asset_id));

        // Switch TRON to trustless light-client mode.
        assert_ok!(Sccp::set_inbound_finality_mode(
            RuntimeOrigin::root(),
            SCCP_DOMAIN_TRON,
            InboundFinalityMode::TronLightClient
        ));

        // Burn payload on TRON -> mint on SORA.
        let asset_h256: sp_core::H256 = asset_id.into();
        let amount: Balance = 10u32.into();
        let recipient = [8u8; 32];
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_TRON,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 1,
            sora_asset_id: asset_h256.0,
            amount,
            recipient,
        };
        let mut preimage = SCCP_MSG_PREFIX_BURN_V1.to_vec();
        preimage.extend(payload.encode());
        let message_id = sp_core::H256::from_slice(&keccak_256(&preimage));

        // Router address configured via `set_default_domain_endpoints()`.
        let router_addr = vec![13u8; 20];

        // Compute storage trie key for burns[messageId].sender.
        let mut slot_bytes = [0u8; 32];
        slot_bytes[24..].copy_from_slice(&SCCP_EVM_BURNS_MAPPING_SLOT.to_be_bytes());
        let mut slot_preimage = [0u8; 64];
        slot_preimage[..32].copy_from_slice(&message_id.0);
        slot_preimage[32..].copy_from_slice(&slot_bytes);
        let slot_base = keccak_256(&slot_preimage);
        let storage_key = keccak_256(&slot_base);

        // Storage trie: a single leaf proving a non-zero value at `storage_key`.
        let storage_path = leaf_path_for_hashed_key(&storage_key);
        let storage_value_rlp = vec![0x01u8]; // RLP(uint256(1)) = 0x01
        let storage_leaf = rlp_leaf_node(&storage_path, &storage_value_rlp);
        let storage_root = sp_core::H256::from_slice(&keccak_256(&storage_leaf));

        // Account trie: a single leaf for router account, with `storageRoot = storage_root`.
        let mut addr20 = [0u8; 20];
        addr20.copy_from_slice(&router_addr);
        let account_key = keccak_256(&addr20);
        let account_path = leaf_path_for_hashed_key(&account_key);
        let account_value = rlp_account_value(storage_root);
        let account_leaf = rlp_leaf_node(&account_path, &account_value);
        let state_root = sp_core::H256::from_slice(&keccak_256(&account_leaf));

        // Synthetic witness set + checkpoint header (protobuf raw_data).
        let p0 = ecdsa::Pair::from_seed(&[1u8; 32]);
        let p1 = ecdsa::Pair::from_seed(&[2u8; 32]);
        let p2 = ecdsa::Pair::from_seed(&[3u8; 32]);
        let w0 = eth_address_from_pair(&p0);
        let w1 = eth_address_from_pair(&p1);
        let w2 = eth_address_from_pair(&p2);

        let mut witnesses = vec![w0, w1, w2];
        witnesses.sort_by_key(|a| a.0);

        let number = 1u64;
        let parent_hash = sp_core::H256([0u8; 32]);
        let mut witness_addr = [0u8; 21];
        witness_addr[0] = 0x41;
        witness_addr[1..].copy_from_slice(&w0.as_bytes());

        // Protobuf encoding of required fields:
        // parentHash (field 3, bytes) => 0x1a
        // number (field 7, varint) => 0x38
        // witness_address (field 9, bytes) => 0x4a
        // accountStateRoot (field 11, bytes) => 0x5a
        let mut raw_data = Vec::new();
        raw_data.push(0x1a);
        raw_data.extend_from_slice(&pb_varint(32));
        raw_data.extend_from_slice(parent_hash.as_bytes());
        raw_data.push(0x38);
        raw_data.extend_from_slice(&pb_varint(number));
        raw_data.push(0x4a);
        raw_data.extend_from_slice(&pb_varint(21));
        raw_data.extend_from_slice(&witness_addr);
        raw_data.push(0x5a);
        raw_data.extend_from_slice(&pb_varint(32));
        raw_data.extend_from_slice(state_root.as_bytes());

        let raw_hash = sp_io::hashing::sha2_256(&raw_data);
        let sig = p0.sign_prehashed(&raw_hash).0.to_vec();

        assert_ok!(Sccp::init_tron_light_client(
            RuntimeOrigin::root(),
            raw_data,
            sig,
            witnesses.clone(),
            0x41,
        ));

        let f = Sccp::tron_finalized().expect("finalized must be set");

        let proof = EvmBurnProofV1 {
            anchor_block_hash: f.hash,
            account_proof: vec![account_leaf],
            storage_proof: vec![storage_leaf],
        };

        let recipient_acc = AccountId::from(recipient);
        let before = assets::Pallet::<Runtime>::free_balance(&asset_id, &recipient_acc)
            .unwrap_or_else(|_| 0u32.into());

        assert_ok!(Sccp::mint_from_proof(
            RuntimeOrigin::signed(alice()),
            SCCP_DOMAIN_TRON,
            payload,
            proof.encode(),
        ));

        let after = assets::Pallet::<Runtime>::free_balance(&asset_id, &recipient_acc).unwrap();
        assert_eq!(after - before, amount);
    });
}
