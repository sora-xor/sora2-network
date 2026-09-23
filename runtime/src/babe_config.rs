// This file is part of the SORA network and Polkaswap app.
// SPDX-License-Identifier: BSD-4-Clause

//! Keep BABE's bootstrap API consistent with its active epoch, and announce the
//! one-time repair of mainnet's historical Plain/VRF configuration discrepancy.

use frame_support::storage::{storage_prefix, unhashed};
use frame_support::traits::OnRuntimeUpgrade;
use frame_support::weights::Weight;
use sp_consensus_babe::digests::NextConfigDescriptor;
use sp_consensus_babe::{AllowedSlots, BabeConfiguration, BabeEpochConfiguration};

use crate::{Babe, EpochDuration, Runtime, RuntimeOrigin, System};

pub(crate) const MAINNET_GENESIS_HASH: sp_core::H256 = sp_core::H256(hex_literal::hex!(
    "7e4e32d0feafd4f9c9414b0be86373f9a1efa904809b683453a9af6856d38ad5"
));

pub(crate) fn configuration() -> BabeConfiguration {
    let epoch_config = pallet_babe::EpochConfig::<Runtime>::get()
        .unwrap_or(crate::constants::BABE_GENESIS_EPOCH_CONFIG);
    BabeConfiguration {
        slot_duration: Babe::slot_duration(),
        epoch_length: EpochDuration::get(),
        c: epoch_config.c,
        authorities: Babe::authorities().to_vec(),
        randomness: Babe::randomness(),
        allowed_slots: epoch_config.allowed_slots,
    }
}

fn repair_marker_key() -> [u8; 32] {
    storage_prefix(b"Babe", b"MainnetPlainConfigRepairV1")
}

pub(crate) fn repair_attempted() -> bool {
    // Existence, rather than decoding a boolean, also prevents a malformed
    // marker from accidentally rescheduling a consensus configuration change.
    unhashed::exists(&repair_marker_key())
}

fn legacy_configuration() -> BabeEpochConfiguration {
    BabeEpochConfiguration {
        c: (1, 4),
        allowed_slots: AllowedSlots::PrimaryAndSecondaryVRFSlots,
    }
}

fn plain_configuration() -> NextConfigDescriptor {
    NextConfigDescriptor::V1 {
        c: (1, 4),
        allowed_slots: AllowedSlots::PrimaryAndSecondaryPlainSlots,
    }
}

/// Schedule the canonical mainnet slot mode through BABE's ordinary two-epoch
/// transition. Never rewrite the current epoch or emit a mid-epoch digest.
///
/// The marker records that this release considered the repair, including when
/// governance has already changed the configuration. Later upgrades must not
/// reverse a legitimate future transition back to VRF.
pub struct ScheduleMainnetPlainEpochConfig;

impl OnRuntimeUpgrade for ScheduleMainnetPlainEpochConfig {
    fn on_runtime_upgrade() -> Weight {
        let db = <Runtime as frame_system::Config>::DbWeight::get();
        if System::block_hash(0) != MAINNET_GENESIS_HASH {
            return db.reads(1);
        }
        if repair_attempted() {
            return db.reads(2);
        }

        let current = pallet_babe::EpochConfig::<Runtime>::get();
        let next = pallet_babe::NextEpochConfig::<Runtime>::get();
        let pending = pallet_babe::PendingEpochConfigChange::<Runtime>::get();
        unhashed::put(&repair_marker_key(), &true);
        let weight = db.reads_writes(5, 1);

        if current.as_ref() != Some(&legacy_configuration())
            || next.as_ref() != Some(&legacy_configuration())
            || pending.is_some()
        {
            log::info!(
                target: "runtime::babe",
                "Preserving existing BABE configuration; the legacy mainnet repair precondition does not hold"
            );
            return weight;
        }

        // The root-only pallet entry point validates and queues the descriptor.
        // BABE announces it at the first epoch boundary and activates it at the
        // second, so persistent clients observe the same transition as state sync.
        if let Err(error) = Babe::plan_config_change(RuntimeOrigin::root(), plain_configuration()) {
            log::error!(target: "runtime::babe", "Could not schedule mainnet BABE repair: {:?}", error);
            return weight;
        }
        log::info!(target: "runtime::babe", "Scheduled mainnet BABE Plain configuration repair");
        weight.saturating_add(db.writes(1))
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<alloc::vec::Vec<u8>, sp_runtime::TryRuntimeError> {
        use codec::Encode;
        Ok((
            System::block_hash(0) == MAINNET_GENESIS_HASH,
            repair_attempted(),
            pallet_babe::EpochConfig::<Runtime>::get(),
            pallet_babe::NextEpochConfig::<Runtime>::get(),
            pallet_babe::PendingEpochConfigChange::<Runtime>::get(),
        )
            .encode())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(state: alloc::vec::Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
        use codec::Decode;
        type Before = (
            bool,
            bool,
            Option<BabeEpochConfiguration>,
            Option<BabeEpochConfiguration>,
            Option<NextConfigDescriptor>,
        );
        let (mainnet, attempted, current, next, pending) =
            Before::decode(&mut &state[..]).map_err(|_| "Invalid BABE repair pre-upgrade state")?;
        if pallet_babe::EpochConfig::<Runtime>::get() != current
            || pallet_babe::NextEpochConfig::<Runtime>::get() != next
        {
            return Err(
                "BABE repair must not change active or next epoch before announcement".into(),
            );
        }
        let should_schedule = mainnet
            && !attempted
            && current.as_ref() == Some(&legacy_configuration())
            && next.as_ref() == Some(&legacy_configuration())
            && pending.is_none();
        let expected_pending = if should_schedule {
            Some(plain_configuration())
        } else {
            pending
        };
        if pallet_babe::PendingEpochConfigChange::<Runtime>::get() != expected_pending {
            return Err("Unexpected pending BABE configuration after repair".into());
        }
        if repair_attempted() != (mainnet || attempted) {
            return Err("Unexpected BABE repair marker after upgrade".into());
        }
        Ok(())
    }
}
