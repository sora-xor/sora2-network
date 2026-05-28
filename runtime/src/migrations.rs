// This file is part of the SORA network and Polkaswap app.

// Copyright (c) 2020, 2021, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

// Redistribution and use in source and binary forms, with or without modification,
// are permitted provided that the following conditions are met:

// Redistributions of source code must retain the above copyright notice, this list
// of conditions and the following disclaimer.
// Redistributions in binary form must reproduce the above copyright notice, this
// list of conditions and the following disclaimer in the documentation and/or other
// materials provided with the distribution.
//
// All advertising materials mentioning features or use of this software must display
// the following acknowledgement: This product includes software developed by Polka Biome
// Ltd., SORA, and Polkaswap.
//
// Neither the name of the Polka Biome Ltd. nor the names of its contributors may be used
// to endorse or promote products derived from this software without specific prior written permission.

// THIS SOFTWARE IS PROVIDED BY Polka Biome Ltd. AS IS AND ANY EXPRESS OR IMPLIED WARRANTIES,
// INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL Polka Biome Ltd. BE LIABLE FOR ANY
// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
// BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS;
// OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
// USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

#[cfg(feature = "try-runtime")]
use codec::{Decode, Encode};
use frame_support::dispatch::DispatchResult;
use frame_support::traits::{
    GetStorageVersion, Hooks, OnRuntimeUpgrade, StorageVersion, UncheckedOnRuntimeUpgrade,
};
use frame_support::weights::Weight;
#[cfg(feature = "try-runtime")]
use sp_runtime::TryRuntimeError;
#[cfg(feature = "try-runtime")]
use sp_std::prelude::Vec;

const IDENTITY_V1_KEY_LIMIT: u64 = 10_000;

pub type Migrations = (
    order_book::migrations::burn_xor_in_tech_accounts::Migrate<crate::Runtime>,
    kensetsu::migrations::v5_to_v6::PurgeXorCollateral<crate::Runtime>,
    BandMigrateToV2IfNeeded,
    pallet_offences::migration::v1::MigrateToV1<crate::Runtime>,
    StakingStorageVersionV16,
    pallet_session::migrations::v1::MigrateV0ToV1<
        crate::Runtime,
        pallet_staking::migrations::v17::MigrateDisabledToSession<crate::Runtime>,
    >,
    pallet_grandpa::migrations::MigrateV4ToV5<crate::Runtime>,
    pallet_im_online::migration::v1::Migration<crate::Runtime>,
    pallet_identity::migration::versioned::V0ToV1<crate::Runtime, IDENTITY_V1_KEY_LIMIT>,
    OracleProxyStorageVersionV1,
    BridgeInboundChannelStorageVersionV1,
    SubstrateBridgeInboundChannelStorageVersionV1,
    SubstrateBridgeOutboundChannelStorageVersionV1,
    BridgePeerIsolationAudit,
    DecommissionLegacyEthereumXor,
    FinalizeEthereumXorThischainAddAsset,
    DemeterFarmingPlatformStorageVersionV3,
    RepairXorTbcdRewardDenomination,
    pallet_polkamarkt::migrations::v2::Migrate<crate::Runtime>,
    pallet_polkamarkt::migrations::v3::Migrate<crate::Runtime>,
    pallet_polkamarkt::migrations::v4::Migrate<crate::Runtime>,
    PrivateNetMigrations,
    WipMigrations,
    xor_fee::migrations::v3::Migrate<crate::Runtime>,
);

pub type MultiBlockMigrations =
    (pallet_identity::migration::v2::LazyMigrationV1ToV2<crate::Runtime>,);

#[cfg(feature = "try-runtime")]
fn decode_storage_version(
    state: Vec<u8>,
    name: &'static str,
) -> Result<StorageVersion, TryRuntimeError> {
    StorageVersion::decode(&mut &state[..]).map_err(|_| {
        TryRuntimeError::Other(Box::leak(
            format!("failed to decode previous storage version for {name}").into_boxed_str(),
        ))
    })
}

#[cfg(feature = "try-runtime")]
fn validate_storage_version_transition(
    name: &'static str,
    previous: StorageVersion,
    expected_after: StorageVersion,
    actual_after: StorageVersion,
) -> Result<(), TryRuntimeError> {
    if actual_after == expected_after {
        Ok(())
    } else {
        Err(TryRuntimeError::Other(Box::leak(
            format!(
                "{name}: expected storage version {:?} after {:?}, found {:?}",
                expected_after, previous, actual_after
            )
            .into_boxed_str(),
        )))
    }
}

#[cfg(feature = "try-runtime")]
fn bridge_peer_audit_error(name: &'static str, err: impl core::fmt::Debug) -> TryRuntimeError {
    TryRuntimeError::Other(Box::leak(
        format!("{name} bridge peer audit failed: {err:?}").into_boxed_str(),
    ))
}

#[cfg(feature = "try-runtime")]
fn audit_bridge_peer_isolation() -> Result<(), TryRuntimeError> {
    bridge_data_signer::Pallet::<crate::Runtime>::ensure_unique_substrate_peers()
        .map_err(|err| bridge_peer_audit_error("BridgeDataSigner", err))?;
    multisig_verifier::Pallet::<crate::Runtime>::ensure_unique_substrate_peers()
        .map_err(|err| bridge_peer_audit_error("MultisigVerifier", err))?;
    Ok(())
}

#[cfg(feature = "try-runtime")]
fn legacy_ethereum_xor_decommission_error(message: &'static str) -> TryRuntimeError {
    TryRuntimeError::Other(message)
}

#[cfg(feature = "try-runtime")]
fn log_legacy_ethereum_xor_decommission_blockers() {
    let blockers =
        eth_bridge::migration::legacy_ethereum_xor_decommission_blockers::<crate::Runtime>();
    if blockers != 0 {
        frame_support::__private::log::warn!(
            "legacy Ethereum XOR decommission will quarantine {blockers} unsafe outgoing transfer requests"
        );
    }
}

#[cfg(feature = "try-runtime")]
fn audit_legacy_ethereum_xor_decommission_cleanup() -> Result<(), TryRuntimeError> {
    let blockers =
        eth_bridge::migration::legacy_ethereum_xor_decommission_blockers::<crate::Runtime>();
    if blockers == 0 {
        Ok(())
    } else {
        Err(TryRuntimeError::Other(Box::leak(
            format!(
            "legacy Ethereum XOR decommission left {blockers} unsafe outgoing transfer requests"
        )
            .into_boxed_str(),
        )))
    }
}

const ETHEREUM_XOR_THISCHAIN_ADD_ASSET_FINALIZED_KEY: &[u8] =
    b"runtime:migrations:ethereum_xor_thischain_add_asset_finalized";
const ETHEREUM_XOR_THISCHAIN_ADD_ASSET_REQUEST_HASH: sp_core::H256 = sp_core::H256([
    0xf4, 0x6f, 0xb9, 0x3b, 0x99, 0xc8, 0x75, 0x6b, 0xf3, 0x32, 0x4e, 0x18, 0xbb, 0x4c, 0x20, 0x43,
    0x08, 0x8b, 0xc2, 0x62, 0xb7, 0x5b, 0xb9, 0x80, 0x7a, 0xc9, 0x6c, 0xf8, 0xf7, 0xbb, 0x07, 0xd2,
]);
const ETHEREUM_XOR_THISCHAIN_MARK_AS_DONE_LOAD_HASH: sp_core::H256 = sp_core::H256([
    0xf3, 0x30, 0xa3, 0xca, 0xa7, 0x3b, 0x9f, 0xf4, 0x6f, 0x26, 0x21, 0x33, 0xd8, 0x7b, 0x90, 0x45,
    0xc8, 0x75, 0xf5, 0xbe, 0x2b, 0x1e, 0x72, 0x07, 0xb1, 0xd6, 0x0e, 0xfb, 0xa5, 0x8b, 0xe8, 0xa8,
]);
const ETHEREUM_XOR_THISCHAIN_MARK_AS_DONE_RETRY_LOAD_HASH: sp_core::H256 = sp_core::H256([
    0x63, 0x8b, 0x39, 0x43, 0x45, 0x81, 0x9d, 0xe4, 0x4f, 0x9c, 0xc7, 0x92, 0x51, 0xf7, 0x96, 0x2e,
    0x8d, 0x48, 0x4c, 0xd8, 0xe9, 0x9f, 0xf8, 0xf0, 0x59, 0x32, 0x1f, 0x4b, 0xa1, 0xe6, 0xec, 0x94,
]);

pub fn ethereum_xor_thischain_add_asset_finalized() -> bool {
    frame_support::storage::unhashed::get(ETHEREUM_XOR_THISCHAIN_ADD_ASSET_FINALIZED_KEY)
        .unwrap_or(false)
}

fn mark_ethereum_xor_thischain_add_asset_finalized() {
    frame_support::storage::unhashed::put(ETHEREUM_XOR_THISCHAIN_ADD_ASSET_FINALIZED_KEY, &true);
}

fn ethereum_xor_thischain_add_asset_is_registered() -> bool {
    let network_id = crate::GetEthNetworkId::get();
    let xor_asset_id: crate::AssetId = common::XOR.into();
    eth_bridge::Pallet::<crate::Runtime>::is_ethereum_xor_thischain_registration(
        network_id,
        &xor_asset_id,
    )
}

fn finalize_ethereum_xor_thischain_add_asset() -> Weight {
    let db_weight = <crate::Runtime as frame_system::Config>::DbWeight::get();
    let mut weight = db_weight.reads(1);

    if ethereum_xor_thischain_add_asset_finalized() {
        return weight;
    }

    let network_id = crate::GetEthNetworkId::get();
    weight.saturating_accrue(db_weight.reads(4));
    if !ethereum_xor_thischain_add_asset_is_registered() {
        return weight;
    }

    let load_hashes = [
        ETHEREUM_XOR_THISCHAIN_MARK_AS_DONE_LOAD_HASH,
        ETHEREUM_XOR_THISCHAIN_MARK_AS_DONE_RETRY_LOAD_HASH,
    ];
    let mut repaired_any = false;
    for load_hash in load_hashes {
        match eth_bridge::migration::finalize_stuck_mark_as_done_request::<crate::Runtime>(
            network_id,
            load_hash,
            ETHEREUM_XOR_THISCHAIN_ADD_ASSET_REQUEST_HASH,
        ) {
            Ok(true) => {
                repaired_any = true;
                weight.saturating_accrue(db_weight.writes(6));
            }
            Ok(false) => {}
            Err(error) => {
                frame_support::__private::log::error!(
                    "Failed to finalize Ethereum XOR Thischain add-asset request: {:?}",
                    error
                );
                panic!(
                    "Failed to finalize Ethereum XOR Thischain add-asset request: {:?}",
                    error
                );
            }
        }
    }

    if repaired_any
        || eth_bridge::RequestStatuses::<crate::Runtime>::get(
            network_id,
            ETHEREUM_XOR_THISCHAIN_ADD_ASSET_REQUEST_HASH,
        ) == Some(eth_bridge::requests::RequestStatus::Done)
    {
        mark_ethereum_xor_thischain_add_asset_finalized();
        return weight.saturating_add(db_weight.writes(1));
    }

    weight
}

#[cfg(feature = "try-runtime")]
fn ethereum_xor_thischain_add_asset_error(message: &'static str) -> TryRuntimeError {
    TryRuntimeError::Other(message)
}

#[cfg(feature = "try-runtime")]
#[derive(Encode, Decode)]
enum BandMigrationState {
    Execute(Vec<u8>),
    Skip(StorageVersion),
}

pub struct BandMigrateToV2IfNeeded;

impl OnRuntimeUpgrade for BandMigrateToV2IfNeeded {
    fn on_runtime_upgrade() -> Weight {
        let db_weight = <crate::Runtime as frame_system::Config>::DbWeight::get();
        let on_chain = band::Pallet::<crate::Runtime>::on_chain_storage_version();

        if on_chain == StorageVersion::new(1) {
            return db_weight
                .reads(1)
                .saturating_add(
                    band::migrations::v2::BandUpdateV2::<crate::Runtime>::on_runtime_upgrade(),
                );
        }

        if on_chain != StorageVersion::new(2) {
            frame_support::__private::log::warn!(
                "Band storage version {:?} is neither v1 nor v2, skipping runtime wrapper migration",
                on_chain
            );
        }

        db_weight.reads(1)
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, TryRuntimeError> {
        let on_chain = band::Pallet::<crate::Runtime>::on_chain_storage_version();
        let state = if on_chain == StorageVersion::new(1) {
            BandMigrationState::Execute(
                band::migrations::v2::BandUpdateV2::<crate::Runtime>::pre_upgrade()?,
            )
        } else {
            BandMigrationState::Skip(on_chain)
        };
        Ok(state.encode())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(state: Vec<u8>) -> Result<(), TryRuntimeError> {
        let on_chain = band::Pallet::<crate::Runtime>::on_chain_storage_version();
        let state = BandMigrationState::decode(&mut &state[..])
            .map_err(|_| TryRuntimeError::Other("failed to decode Band migration wrapper state"))?;
        match state {
            BandMigrationState::Execute(state) => {
                band::migrations::v2::BandUpdateV2::<crate::Runtime>::post_upgrade(state)?;
                validate_storage_version_transition(
                    "Band",
                    StorageVersion::new(1),
                    StorageVersion::new(2),
                    on_chain,
                )
            }
            BandMigrationState::Skip(previous) => {
                validate_storage_version_transition("Band", previous, previous, on_chain)
            }
        }
    }
}

pub struct StakingStorageVersionV16;

impl OnRuntimeUpgrade for StakingStorageVersionV16 {
    fn on_runtime_upgrade() -> Weight {
        let db_weight = <crate::Runtime as frame_system::Config>::DbWeight::get();
        let on_chain = pallet_staking::Pallet::<crate::Runtime>::on_chain_storage_version();
        let mut weight = db_weight.reads(1);

        match on_chain {
            v if v == StorageVersion::new(13) => {
                StorageVersion::new(14).put::<pallet_staking::Pallet<crate::Runtime>>();
                weight.saturating_accrue(db_weight.writes(1));
                weight.saturating_accrue(
                    pallet_staking::migrations::v15::VersionUncheckedMigrateV14ToV15::<
                        crate::Runtime,
                    >::on_runtime_upgrade(),
                );
                StorageVersion::new(15).put::<pallet_staking::Pallet<crate::Runtime>>();
                weight.saturating_accrue(db_weight.writes(1));
                weight.saturating_accrue(
                    pallet_staking::migrations::v16::VersionUncheckedMigrateV15ToV16::<
                        crate::Runtime,
                    >::on_runtime_upgrade(),
                );
                StorageVersion::new(16).put::<pallet_staking::Pallet<crate::Runtime>>();
                weight.saturating_accrue(db_weight.writes(1));
            }
            v if v == StorageVersion::new(14) => {
                weight.saturating_accrue(
                    pallet_staking::migrations::v15::VersionUncheckedMigrateV14ToV15::<
                        crate::Runtime,
                    >::on_runtime_upgrade(),
                );
                StorageVersion::new(15).put::<pallet_staking::Pallet<crate::Runtime>>();
                weight.saturating_accrue(db_weight.writes(1));
                weight.saturating_accrue(
                    pallet_staking::migrations::v16::VersionUncheckedMigrateV15ToV16::<
                        crate::Runtime,
                    >::on_runtime_upgrade(),
                );
                StorageVersion::new(16).put::<pallet_staking::Pallet<crate::Runtime>>();
                weight.saturating_accrue(db_weight.writes(1));
            }
            v if v == StorageVersion::new(15) => {
                weight.saturating_accrue(
                    pallet_staking::migrations::v16::VersionUncheckedMigrateV15ToV16::<
                        crate::Runtime,
                    >::on_runtime_upgrade(),
                );
                StorageVersion::new(16).put::<pallet_staking::Pallet<crate::Runtime>>();
                weight.saturating_accrue(db_weight.writes(1));
            }
            _ => {}
        }

        weight
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, TryRuntimeError> {
        Ok(pallet_staking::Pallet::<crate::Runtime>::on_chain_storage_version().encode())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(state: Vec<u8>) -> Result<(), TryRuntimeError> {
        let previous = decode_storage_version(state, "Staking")?;
        let expected = match previous {
            version if version == StorageVersion::new(13) => StorageVersion::new(16),
            version if version == StorageVersion::new(14) => StorageVersion::new(16),
            version if version == StorageVersion::new(15) => StorageVersion::new(16),
            version => version,
        };
        validate_storage_version_transition(
            "Staking",
            previous,
            expected,
            pallet_staking::Pallet::<crate::Runtime>::on_chain_storage_version(),
        )
    }
}

pub struct BridgePeerIsolationAudit;

impl OnRuntimeUpgrade for BridgePeerIsolationAudit {
    fn on_runtime_upgrade() -> Weight {
        Weight::zero()
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, TryRuntimeError> {
        audit_bridge_peer_isolation()?;
        Ok(Vec::new())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(_state: Vec<u8>) -> Result<(), TryRuntimeError> {
        audit_bridge_peer_isolation()
    }
}

pub struct DecommissionLegacyEthereumXor;

impl OnRuntimeUpgrade for DecommissionLegacyEthereumXor {
    fn on_runtime_upgrade() -> Weight {
        eth_bridge::migration::decommission_legacy_ethereum_xor::<crate::Runtime>()
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, TryRuntimeError> {
        log_legacy_ethereum_xor_decommission_blockers();
        Ok(Vec::new())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(_state: Vec<u8>) -> Result<(), TryRuntimeError> {
        audit_legacy_ethereum_xor_decommission_cleanup()?;
        if !eth_bridge::migration::is_legacy_ethereum_xor_decommissioned::<crate::Runtime>() {
            return Err(legacy_ethereum_xor_decommission_error(
                "legacy Ethereum XOR was not decommissioned",
            ));
        }
        if eth_bridge::migration::legacy_ethereum_xor_decommissioned_at::<crate::Runtime>()
            .is_none()
        {
            return Err(legacy_ethereum_xor_decommission_error(
                "legacy Ethereum XOR decommission block was not recorded",
            ));
        }
        Ok(())
    }
}

pub struct FinalizeEthereumXorThischainAddAsset;

impl OnRuntimeUpgrade for FinalizeEthereumXorThischainAddAsset {
    fn on_runtime_upgrade() -> Weight {
        finalize_ethereum_xor_thischain_add_asset()
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, TryRuntimeError> {
        Ok(ethereum_xor_thischain_add_asset_finalized().encode())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(state: Vec<u8>) -> Result<(), TryRuntimeError> {
        let was_finalized = bool::decode(&mut &state[..]).map_err(|_| {
            TryRuntimeError::Other("failed to decode Ethereum XOR add-asset finalization marker")
        })?;
        if was_finalized || ethereum_xor_thischain_add_asset_finalized() {
            return Ok(());
        }
        let network_id = crate::GetEthNetworkId::get();
        for load_hash in [
            ETHEREUM_XOR_THISCHAIN_MARK_AS_DONE_LOAD_HASH,
            ETHEREUM_XOR_THISCHAIN_MARK_AS_DONE_RETRY_LOAD_HASH,
        ] {
            if eth_bridge::RequestStatuses::<crate::Runtime>::get(network_id, load_hash)
                == Some(eth_bridge::requests::RequestStatus::Pending)
            {
                return Err(ethereum_xor_thischain_add_asset_error(
                    "Ethereum XOR MarkAsDone load request is still pending",
                ));
            }
        }
        Ok(())
    }
}

const XOR_TBCD_REWARD_DENOMINATION_REPAIR_KEY: &[u8] =
    b"runtime:migrations:xor_tbcd_reward_denomination_repaired";

pub fn xor_tbcd_reward_denomination_repaired() -> bool {
    frame_support::storage::unhashed::get(XOR_TBCD_REWARD_DENOMINATION_REPAIR_KEY).unwrap_or(false)
}

fn mark_xor_tbcd_reward_denomination_repaired() {
    frame_support::storage::unhashed::put(XOR_TBCD_REWARD_DENOMINATION_REPAIR_KEY, &true);
}

fn current_xor_tbcd_denominator() -> crate::Balance {
    <crate::Denomination as common::Denominator<crate::AssetId, crate::Balance>>::current_factor(
        &common::XOR.into(),
    )
}

fn repair_xor_tbcd_reward_denomination() -> Weight {
    let db_weight = <crate::Runtime as frame_system::Config>::DbWeight::get();
    let mut weight = db_weight.reads(1);

    if xor_tbcd_reward_denomination_repaired() {
        return weight;
    }

    let factor = current_xor_tbcd_denominator();
    weight.saturating_accrue(db_weight.reads(1));

    if factor <= 1 {
        mark_xor_tbcd_reward_denomination_repaired();
        return weight.saturating_add(db_weight.writes(1));
    }

    if let Err(error) = common::with_transaction(|| -> DispatchResult {
        apollo_platform::Pallet::<crate::Runtime>::denominate_xor_and_tbcd_state(&factor)?;
        vested_rewards::Pallet::<crate::Runtime>::denominate_crowdloan_reward_storage(&factor)?;
        Ok(())
    }) {
        frame_support::__private::log::error!(
            "Failed to repair stale XOR/TBCD reward denomination state: {:?}",
            error
        );
        panic!(
            "Failed to repair stale XOR/TBCD reward denomination state: {:?}",
            error
        );
    }

    mark_xor_tbcd_reward_denomination_repaired();
    <crate::Runtime as frame_system::Config>::BlockWeights::get().max_block
}

pub struct RepairXorTbcdRewardDenomination;

impl OnRuntimeUpgrade for RepairXorTbcdRewardDenomination {
    fn on_runtime_upgrade() -> Weight {
        repair_xor_tbcd_reward_denomination()
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, TryRuntimeError> {
        Ok(xor_tbcd_reward_denomination_repaired().encode())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(state: Vec<u8>) -> Result<(), TryRuntimeError> {
        let was_repaired = bool::decode(&mut &state[..]).map_err(|_| {
            TryRuntimeError::Other("failed to decode XOR/TBCD reward denomination repair marker")
        })?;
        if was_repaired || xor_tbcd_reward_denomination_repaired() {
            Ok(())
        } else {
            Err(TryRuntimeError::Other(
                "XOR/TBCD reward denomination repair marker was not written",
            ))
        }
    }
}

pub struct DemeterFarmingPlatformStorageVersionV3;

impl OnRuntimeUpgrade for DemeterFarmingPlatformStorageVersionV3 {
    fn on_runtime_upgrade() -> Weight {
        <demeter_farming_platform::Pallet<crate::Runtime> as Hooks<crate::BlockNumber>>::on_runtime_upgrade()
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, TryRuntimeError> {
        Ok(demeter_farming_platform::PalletStorageVersion::<crate::Runtime>::get().encode())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(state: Vec<u8>) -> Result<(), TryRuntimeError> {
        let previous =
            demeter_farming_platform::StorageVersion::decode(&mut &state[..]).map_err(|_| {
                TryRuntimeError::Other("failed to decode previous Demeter storage version")
            })?;
        let expected = match previous {
            demeter_farming_platform::StorageVersion::V1
            | demeter_farming_platform::StorageVersion::V2
            | demeter_farming_platform::StorageVersion::V3 => {
                demeter_farming_platform::StorageVersion::V3
            }
        };
        let current = demeter_farming_platform::PalletStorageVersion::<crate::Runtime>::get();
        if current == expected {
            Ok(())
        } else {
            Err(TryRuntimeError::Other(
                "Demeter storage version did not reach the expected version",
            ))
        }
    }
}

macro_rules! version_only_storage_version_v1_migration {
    ($name:ident, $pallet:ty, $label:literal) => {
        pub struct $name;

        impl OnRuntimeUpgrade for $name {
            fn on_runtime_upgrade() -> Weight {
                if <$pallet>::on_chain_storage_version() == StorageVersion::new(0) {
                    StorageVersion::new(1).put::<$pallet>();
                    return <crate::Runtime as frame_system::Config>::DbWeight::get()
                        .reads_writes(1, 1);
                }
                <crate::Runtime as frame_system::Config>::DbWeight::get().reads(1)
            }

            #[cfg(feature = "try-runtime")]
            fn pre_upgrade() -> Result<Vec<u8>, TryRuntimeError> {
                Ok(<$pallet>::on_chain_storage_version().encode())
            }

            #[cfg(feature = "try-runtime")]
            fn post_upgrade(state: Vec<u8>) -> Result<(), TryRuntimeError> {
                let previous = decode_storage_version(state, $label)?;
                let expected = if previous == StorageVersion::new(0) {
                    StorageVersion::new(1)
                } else {
                    previous
                };
                validate_storage_version_transition(
                    $label,
                    previous,
                    expected,
                    <$pallet>::on_chain_storage_version(),
                )
            }
        }
    };
}

version_only_storage_version_v1_migration!(
    OracleProxyStorageVersionV1,
    oracle_proxy::Pallet<crate::Runtime>,
    "OracleProxy"
);
version_only_storage_version_v1_migration!(
    BridgeInboundChannelStorageVersionV1,
    bridge_channel::inbound::Pallet<crate::Runtime>,
    "BridgeInboundChannel"
);
version_only_storage_version_v1_migration!(
    SubstrateBridgeInboundChannelStorageVersionV1,
    substrate_bridge_channel::inbound::Pallet<crate::Runtime>,
    "SubstrateBridgeInboundChannel"
);
version_only_storage_version_v1_migration!(
    SubstrateBridgeOutboundChannelStorageVersionV1,
    substrate_bridge_channel::outbound::Pallet<crate::Runtime>,
    "SubstrateBridgeOutboundChannel"
);

#[cfg(feature = "wip")]
pub type WipMigrations = (
    xor_fee::migrations::add_white_listed_assets_for_xorless_fee::Migrate<crate::Runtime>,
    xor_fee::migrations::v2::Migrate<crate::Runtime>,
);

#[cfg(not(feature = "wip"))]
pub type WipMigrations = ();

#[cfg(feature = "private-net")]
pub type PrivateNetMigrations = (ClearSudoKey,);

#[cfg(not(feature = "private-net"))]
pub type PrivateNetMigrations = ();

#[cfg(feature = "private-net")]
pub struct ClearSudoKey;

#[cfg(feature = "private-net")]
impl OnRuntimeUpgrade for ClearSudoKey {
    fn on_runtime_upgrade() -> Weight {
        let db_weight = <crate::Runtime as frame_system::Config>::DbWeight::get();
        if pallet_sudo::Key::<crate::Runtime>::get().is_some() {
            pallet_sudo::Key::<crate::Runtime>::kill();
            return db_weight.reads_writes(1, 1);
        }
        db_weight.reads(1)
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, TryRuntimeError> {
        Ok(pallet_sudo::Key::<crate::Runtime>::get().encode())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(state: Vec<u8>) -> Result<(), TryRuntimeError> {
        let previous = Option::<crate::AccountId>::decode(&mut &state[..])
            .map_err(|_| TryRuntimeError::Other("failed to decode previous sudo key"))?;
        let current = pallet_sudo::Key::<crate::Runtime>::get();
        if current.is_none() {
            Ok(())
        } else {
            Err(TryRuntimeError::Other(Box::leak(
                format!(
                    "expected sudo key to be cleared after {:?}, found {:?}",
                    previous, current
                )
                .into_boxed_str(),
            )))
        }
    }
}
