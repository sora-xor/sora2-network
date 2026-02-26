// This file is part of the SORA network and Polkaswap app.
//
// Copyright (c) 2026, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use codec::Encode;
use common::{prelude::Balance, AssetManager, AssetName, AssetSymbol, FromGenericPair};
use frame_benchmarking::{benchmarks, whitelisted_caller};
use frame_support::traits::Get;
use frame_system::RawOrigin;
use sp_core::{H160, H256};
use sp_io::hashing::keccak_256;
use sp_std::prelude::*;

const BENCH_BSC_EPOCH_LENGTH: u64 = 1_000;
const BENCH_BSC_CHAIN_ID: u64 = 56;
const BENCH_BSC_TURN_LENGTH: u8 = 16;
const BENCH_TRON_ADDRESS_PREFIX: u8 = 0x41;

fn register_mintable_asset<T: Config>(
    owner: &T::AccountId,
    tag: u8,
) -> Result<AssetIdOf<T>, &'static str> {
    frame_system::Pallet::<T>::inc_account_nonce(owner);
    let suffix = b'A' + (tag % 26);
    <T as common::Config>::AssetManager::register_from(
        owner,
        AssetSymbol(vec![b'S', b'C', b'C', b'P', suffix]),
        AssetName(vec![b'S', b'C', b'C', b'P', suffix]),
        common::DEFAULT_BALANCE_PRECISION,
        0u32.into(),
        true,
        common::AssetType::Regular,
        None,
        None,
    )
    .map_err(|_| "register_from failed")
}

fn sccp_account<T: Config>() -> Result<T::AccountId, &'static str> {
    let tech_account_id = FromGenericPair::from_generic_pair(
        SCCP_TECH_ACC_PREFIX.to_vec(),
        SCCP_TECH_ACC_MAIN.to_vec(),
    );
    technical::Pallet::<T>::register_tech_account_id_if_not_exist(&tech_account_id)
        .map_err(|_| "register_tech_account_id_if_not_exist failed")?;
    technical::Pallet::<T>::tech_account_id_to_account_id(&tech_account_id)
        .map_err(|_| "tech_account_id_to_account_id failed")
}

fn set_default_remote_tokens<T: Config>(asset_id: AssetIdOf<T>) -> Result<(), &'static str> {
    Pallet::<T>::set_remote_token(
        RawOrigin::Root.into(),
        asset_id,
        SCCP_DOMAIN_ETH,
        vec![1u8; 20],
    )
    .map_err(|_| "set_remote_token eth failed")?;
    Pallet::<T>::set_remote_token(
        RawOrigin::Root.into(),
        asset_id,
        SCCP_DOMAIN_BSC,
        vec![2u8; 20],
    )
    .map_err(|_| "set_remote_token bsc failed")?;
    Pallet::<T>::set_remote_token(
        RawOrigin::Root.into(),
        asset_id,
        SCCP_DOMAIN_TRON,
        vec![3u8; 20],
    )
    .map_err(|_| "set_remote_token tron failed")?;
    Pallet::<T>::set_remote_token(
        RawOrigin::Root.into(),
        asset_id,
        SCCP_DOMAIN_SOL,
        vec![4u8; 32],
    )
    .map_err(|_| "set_remote_token sol failed")?;
    Pallet::<T>::set_remote_token(
        RawOrigin::Root.into(),
        asset_id,
        SCCP_DOMAIN_TON,
        vec![5u8; 32],
    )
    .map_err(|_| "set_remote_token ton failed")?;
    Ok(())
}

fn set_default_domain_endpoints<T: Config>() -> Result<(), &'static str> {
    Pallet::<T>::set_domain_endpoint(RawOrigin::Root.into(), SCCP_DOMAIN_ETH, vec![11u8; 20])
        .map_err(|_| "set_domain_endpoint eth failed")?;
    Pallet::<T>::set_domain_endpoint(RawOrigin::Root.into(), SCCP_DOMAIN_BSC, vec![12u8; 20])
        .map_err(|_| "set_domain_endpoint bsc failed")?;
    Pallet::<T>::set_domain_endpoint(RawOrigin::Root.into(), SCCP_DOMAIN_TRON, vec![13u8; 20])
        .map_err(|_| "set_domain_endpoint tron failed")?;
    Pallet::<T>::set_domain_endpoint(RawOrigin::Root.into(), SCCP_DOMAIN_SOL, vec![14u8; 32])
        .map_err(|_| "set_domain_endpoint sol failed")?;
    Pallet::<T>::set_domain_endpoint(RawOrigin::Root.into(), SCCP_DOMAIN_TON, vec![15u8; 32])
        .map_err(|_| "set_domain_endpoint ton failed")?;
    Ok(())
}

fn setup_pending_token<T: Config>(
    owner: &T::AccountId,
    tag: u8,
) -> Result<AssetIdOf<T>, &'static str> {
    let asset_id = register_mintable_asset::<T>(owner, tag)?;
    Pallet::<T>::add_token(RawOrigin::Root.into(), asset_id).map_err(|_| "add_token failed")?;
    Ok(asset_id)
}

fn setup_active_token<T: Config>(
    owner: &T::AccountId,
    tag: u8,
) -> Result<AssetIdOf<T>, &'static str> {
    let asset_id = setup_pending_token::<T>(owner, tag)?;
    set_default_domain_endpoints::<T>()?;
    set_default_remote_tokens::<T>(asset_id)?;
    Pallet::<T>::activate_token(RawOrigin::Root.into(), asset_id)
        .map_err(|_| "activate_token failed")?;
    Ok(asset_id)
}

fn setup_active_token_with_balance<T: Config>(
    owner: &T::AccountId,
    tag: u8,
    amount: Balance,
) -> Result<AssetIdOf<T>, &'static str> {
    let asset_id = setup_active_token::<T>(owner, tag)?;
    let issuer = sccp_account::<T>()?;
    <T as common::Config>::AssetManager::mint_to(&asset_id, &issuer, owner, amount)
        .map_err(|_| "mint_to failed")?;
    Ok(asset_id)
}

fn canonical_evm_recipient(fill: u8) -> [u8; 32] {
    let mut recipient = [0u8; 32];
    recipient[12..].copy_from_slice(&[fill; 20]);
    recipient
}

fn burn_message_id(payload: &BurnPayloadV1) -> H256 {
    let mut preimage = SCCP_MSG_PREFIX_BURN_V1.to_vec();
    preimage.extend(payload.encode());
    H256::from_slice(&keccak_256(&preimage))
}

benchmarks! {
    where_clause { where AssetIdOf<T>: From<H256> + Into<H256> }

    add_token {
        let owner: T::AccountId = whitelisted_caller();
        frame_system::Pallet::<T>::inc_providers(&owner);
        let asset_id = register_mintable_asset::<T>(&owner, 1)?;
    }: _(RawOrigin::Root, asset_id)

    set_remote_token {
        let owner: T::AccountId = whitelisted_caller();
        frame_system::Pallet::<T>::inc_providers(&owner);
        let asset_id = setup_pending_token::<T>(&owner, 2)?;
    }: _(RawOrigin::Root, asset_id, SCCP_DOMAIN_ETH, vec![1u8; 20])

    set_domain_endpoint {
    }: _(RawOrigin::Root, SCCP_DOMAIN_ETH, vec![11u8; 20])

    clear_domain_endpoint {
        Pallet::<T>::set_domain_endpoint(RawOrigin::Root.into(), SCCP_DOMAIN_ETH, vec![11u8; 20])
            .map_err(|_| "set_domain_endpoint setup failed")?;
    }: _(RawOrigin::Root, SCCP_DOMAIN_ETH)

    set_evm_anchor_mode_enabled {
    }: _(RawOrigin::Root, SCCP_DOMAIN_ETH, true)

    init_bsc_light_client {
        let a in 1 .. 2;
        let _ = a;
        let checkpoint_header_rlp = include_bytes!("fixtures/bsc_header_81094034.rlp").to_vec();
        let validators = vec![H160::from_slice(&[
            0x9f, 0x1b, 0x7f, 0xae, 0x54, 0xbe, 0x07, 0xf4, 0xfe, 0xe3,
            0x4e, 0xb1, 0xaa, 0xcb, 0x39, 0xa1, 0xf7, 0xb6, 0xfc, 0x92,
        ])];
    }: _(RawOrigin::Root, checkpoint_header_rlp, validators, BENCH_BSC_EPOCH_LENGTH, 0u64, BENCH_BSC_CHAIN_ID, BENCH_BSC_TURN_LENGTH)

    submit_bsc_header {
        let a in 1 .. 2;
        let _ = a;
        let caller: T::AccountId = whitelisted_caller();
        frame_system::Pallet::<T>::inc_providers(&caller);

        let checkpoint_header_rlp = include_bytes!("fixtures/bsc_header_81094034.rlp").to_vec();
        let validators = vec![H160::from_slice(&[
            0x9f, 0x1b, 0x7f, 0xae, 0x54, 0xbe, 0x07, 0xf4, 0xfe, 0xe3,
            0x4e, 0xb1, 0xaa, 0xcb, 0x39, 0xa1, 0xf7, 0xb6, 0xfc, 0x92,
        ])];
        Pallet::<T>::init_bsc_light_client(
            RawOrigin::Root.into(),
            checkpoint_header_rlp.clone(),
            validators,
            BENCH_BSC_EPOCH_LENGTH,
            0,
            BENCH_BSC_CHAIN_ID,
            BENCH_BSC_TURN_LENGTH,
        )
        .map_err(|_| "init_bsc_light_client setup failed")?;

        let bad_header = checkpoint_header_rlp;
    }: {
        let _ = Pallet::<T>::submit_bsc_header(RawOrigin::Signed(caller).into(), bad_header);
    }

    set_bsc_validators {
        let a in 1 .. T::MaxBscValidators::get();
        let mut validators = Vec::new();
        for i in 0..a {
            validators.push(H160::from_low_u64_be((i + 1) as u64));
        }
    }: _(RawOrigin::Root, validators)

    init_tron_light_client {
        let a in 1 .. 2;
        let _ = a;
        let raw_data = vec![0u8; 10];
        let signature = vec![0u8; 65];
        let witnesses = vec![H160::from_low_u64_be(1)];
    }: {
        let _ = Pallet::<T>::init_tron_light_client(
            RawOrigin::Root.into(),
            raw_data,
            signature,
            witnesses,
            BENCH_TRON_ADDRESS_PREFIX,
        );
    }

    submit_tron_header {
        let a in 1 .. 2;
        let _ = a;
        let caller: T::AccountId = whitelisted_caller();
        frame_system::Pallet::<T>::inc_providers(&caller);
        let raw_data = vec![0u8; 10];
        let signature = vec![0u8; 65];
    }: {
        let _ = Pallet::<T>::submit_tron_header(
            RawOrigin::Signed(caller).into(),
            raw_data,
            signature,
        );
    }

    set_tron_witnesses {
        let a in 1 .. T::MaxBscValidators::get();
        let mut witnesses = Vec::new();
        for i in 0..a {
            witnesses.push(H160::from_low_u64_be((i + 1) as u64));
        }
    }: _(RawOrigin::Root, witnesses)

    set_inbound_attesters {
        let a in 1 .. T::MaxAttesters::get();
        let mut attesters = Vec::new();
        for i in 0..a {
            attesters.push(H160::from_low_u64_be((i + 1) as u64));
        }
        let threshold = 1u32;
    }: _(RawOrigin::Root, SCCP_DOMAIN_SOL, attesters, threshold)

    clear_inbound_attesters {
        let attester = H160::from_low_u64_be(1);
        Pallet::<T>::set_inbound_attesters(
            RawOrigin::Root.into(),
            SCCP_DOMAIN_SOL,
            vec![attester],
            1,
        )
        .map_err(|_| "set_inbound_attesters setup failed")?;
    }: _(RawOrigin::Root, SCCP_DOMAIN_SOL)

    activate_token {
        let owner: T::AccountId = whitelisted_caller();
        frame_system::Pallet::<T>::inc_providers(&owner);
        let asset_id = setup_pending_token::<T>(&owner, 3)?;
        set_default_domain_endpoints::<T>()?;
        set_default_remote_tokens::<T>(asset_id)?;
    }: _(RawOrigin::Root, asset_id)

    remove_token {
        let owner: T::AccountId = whitelisted_caller();
        frame_system::Pallet::<T>::inc_providers(&owner);
        let asset_id = setup_active_token::<T>(&owner, 4)?;
    }: _(RawOrigin::Root, asset_id)

    finalize_remove {
        let owner: T::AccountId = whitelisted_caller();
        frame_system::Pallet::<T>::inc_providers(&owner);
        let asset_id = setup_active_token::<T>(&owner, 5)?;
        Pallet::<T>::set_inbound_grace_period(RawOrigin::Root.into(), 0u32.into())
            .map_err(|_| "set_inbound_grace_period setup failed")?;
        Pallet::<T>::remove_token(RawOrigin::Root.into(), asset_id)
            .map_err(|_| "remove_token setup failed")?;
        frame_system::Pallet::<T>::set_block_number(1u32.into());
    }: _(RawOrigin::Root, asset_id)

    set_inbound_grace_period {
    }: _(RawOrigin::Root, 42u32.into())

    set_required_domains {
        let a in 5 .. 6;
        let _ = a;
        let domains = SCCP_CORE_REMOTE_DOMAINS.to_vec();
    }: _(RawOrigin::Root, domains)

    set_inbound_finality_mode {
    }: _(RawOrigin::Root, SCCP_DOMAIN_SOL, InboundFinalityMode::AttesterQuorum)

    set_inbound_domain_paused {
    }: _(RawOrigin::Root, SCCP_DOMAIN_ETH, true)

    set_outbound_domain_paused {
    }: _(RawOrigin::Root, SCCP_DOMAIN_ETH, true)

    invalidate_inbound_message {
        let message_id = H256::repeat_byte(0x11);
    }: _(RawOrigin::Root, SCCP_DOMAIN_ETH, message_id)

    clear_invalidated_inbound_message {
        let message_id = H256::repeat_byte(0x22);
        Pallet::<T>::invalidate_inbound_message(
            RawOrigin::Root.into(),
            SCCP_DOMAIN_ETH,
            message_id,
        )
        .map_err(|_| "invalidate_inbound_message setup failed")?;
    }: _(RawOrigin::Root, SCCP_DOMAIN_ETH, message_id)

    burn {
        let caller: T::AccountId = whitelisted_caller();
        frame_system::Pallet::<T>::inc_providers(&caller);
        let asset_id = setup_active_token_with_balance::<T>(&caller, 6, 1_000_000u32.into())?;
        let amount: Balance = 1_000u32.into();
        let recipient = canonical_evm_recipient(7);
    }: _(RawOrigin::Signed(caller), asset_id, amount, SCCP_DOMAIN_ETH, recipient)

    mint_from_proof {
        let caller: T::AccountId = whitelisted_caller();
        frame_system::Pallet::<T>::inc_providers(&caller);
        let asset_id = setup_active_token::<T>(&caller, 7)?;
        Pallet::<T>::set_inbound_finality_mode(
            RawOrigin::Root.into(),
            SCCP_DOMAIN_ETH,
            InboundFinalityMode::EvmAnchor,
        )
        .map_err(|_| "set_inbound_finality_mode setup failed")?;
        Pallet::<T>::set_evm_anchor_mode_enabled(
            RawOrigin::Root.into(),
            SCCP_DOMAIN_ETH,
            true,
        )
        .map_err(|_| "set_evm_anchor_mode_enabled setup failed")?;
        Pallet::<T>::set_evm_inbound_anchor(
            RawOrigin::Root.into(),
            SCCP_DOMAIN_ETH,
            1,
            H256::repeat_byte(0x33),
            H256::repeat_byte(0x44),
        )
        .map_err(|_| "set_evm_inbound_anchor setup failed")?;

        let asset_h256: H256 = asset_id.into();
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 1,
            sora_asset_id: asset_h256.0,
            amount: 10u32.into(),
            recipient: [8u8; 32],
        };
        let proof = vec![];
    }: {
        let _ = Pallet::<T>::mint_from_proof(
            RawOrigin::Signed(caller).into(),
            SCCP_DOMAIN_ETH,
            payload,
            proof,
        );
    }

    attest_burn {
        let caller: T::AccountId = whitelisted_caller();
        frame_system::Pallet::<T>::inc_providers(&caller);
        let asset_id = setup_active_token::<T>(&caller, 8)?;
        Pallet::<T>::set_inbound_finality_mode(
            RawOrigin::Root.into(),
            SCCP_DOMAIN_ETH,
            InboundFinalityMode::EvmAnchor,
        )
        .map_err(|_| "set_inbound_finality_mode setup failed")?;
        Pallet::<T>::set_evm_anchor_mode_enabled(
            RawOrigin::Root.into(),
            SCCP_DOMAIN_ETH,
            true,
        )
        .map_err(|_| "set_evm_anchor_mode_enabled setup failed")?;
        Pallet::<T>::set_evm_inbound_anchor(
            RawOrigin::Root.into(),
            SCCP_DOMAIN_ETH,
            1,
            H256::repeat_byte(0x55),
            H256::repeat_byte(0x66),
        )
        .map_err(|_| "set_evm_inbound_anchor setup failed")?;

        let asset_h256: H256 = asset_id.into();
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SOL,
            nonce: 2,
            sora_asset_id: asset_h256.0,
            amount: 10u32.into(),
            recipient: [9u8; 32],
        };
        let _message_id = burn_message_id(&payload);
        let proof = vec![];
    }: {
        let _ = Pallet::<T>::attest_burn(
            RawOrigin::Signed(caller).into(),
            SCCP_DOMAIN_ETH,
            payload,
            proof,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use frame_benchmarking::impl_benchmark_test_suite;

    impl_benchmark_test_suite!(
        Pallet,
        crate::mock::ExtBuilder::default().build(),
        crate::mock::Runtime
    );
}
