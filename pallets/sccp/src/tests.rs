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
use codec::Encode;
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
