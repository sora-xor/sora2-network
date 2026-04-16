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
use frame_support::traits::{
    GetStorageVersion, OnRuntimeUpgrade, StorageVersion, UncheckedOnRuntimeUpgrade,
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
    PrivateNetMigrations,
    WipMigrations,
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
