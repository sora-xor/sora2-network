use crate::constants::BABE_GENESIS_EPOCH_CONFIG;
use crate::*;
use frame_support::traits::OnRuntimeUpgrade;
use frame_support::traits::PalletInfo;
use sp_core::ecdsa;

pub type Migrations = (
    SessionKeysMigration,
    ElectionsPhragmenPrefixMigration,
    BabeConfigMigration,
    StakingV6Migration,
    SystemDualRefToTripleRefMigration,
    OffencesUpdateMigration,
    GrandpaStoragePrefixMigration,
    StakingV7Migration,
    TechnicalMembershipStoragePrefixMigration,
    CouncilStoragePrefixMigration,
    TechnicalCommitteeStoragePrefixMigration,
    MigratePalletVersionToStorageVersion,
    StakingV8Migration,
    HistoricalStoragePrefixMigration,
    SchedulerV3Migration,
    ElectionsPhragmenV5Migration,
    StakingV9Migration,
    vested_rewards::migrations::ResetClaimingForCrowdloadErrors<Runtime>,
);

impl_opaque_keys! {
    pub struct SessionKeysOld {
        pub babe: Babe,
        pub grandpa: Grandpa,
        pub im_online: ImOnline,
    }
}

/// Generates a `BeefyId` from the given `AccountId`. The resulting `BeefyId` is
/// a dummy value and this is a utility function meant to be used when migration
/// session keys.
pub fn dummy_beefy_id_from_account_id(a: AccountId) -> BeefyId {
    let mut id_raw = [0u8; 33];

    // NOTE: AccountId is 32 bytes, whereas BeefyId is 33 bytes.
    id_raw[1..].copy_from_slice(a.as_ref());
    id_raw[0..4].copy_from_slice(b"beef");

    ecdsa::Public(id_raw).into()
}

pub struct SessionKeysMigration;

impl OnRuntimeUpgrade for SessionKeysMigration {
    fn on_runtime_upgrade() -> Weight {
        Session::upgrade_keys::<SessionKeysOld, _>(|id, keys| opaque::SessionKeys {
            babe: keys.babe,
            grandpa: keys.grandpa,
            im_online: keys.im_online,
            beefy: dummy_beefy_id_from_account_id(id),
        });
        BlockWeights::get().max_block
    }
}

/// https://github.com/paritytech/substrate/pull/8044
pub struct ElectionsPhragmenPrefixMigration;

impl OnRuntimeUpgrade for ElectionsPhragmenPrefixMigration {
    fn on_runtime_upgrade() -> Weight {
        frame_support::log::warn!("Run migration ElectionsPhragmenPrefixMigration");
        let name = <Runtime as frame_system::Config>::PalletInfo::name::<ElectionsPhragmen>()
            .expect(
                "elections phragmen is part of pallets in construct_runtime, so it has a name; qed",
            );
        pallet_elections_phragmen::migrations::v4::migrate::<Runtime, _>(name)
    }
}

/// https://github.com/paritytech/substrate/pull/8072
pub struct BabeConfigMigration;

impl pallet_babe::migrations::BabePalletPrefix for Runtime {
    fn pallet_prefix() -> &'static str {
        let name = <Runtime as frame_system::Config>::PalletInfo::name::<Babe>()
            .expect("babe is part of pallets in construct_runtime, so it has a name; qed");
        name
    }
}

impl OnRuntimeUpgrade for BabeConfigMigration {
    fn on_runtime_upgrade() -> Weight {
        frame_support::log::warn!("Run migration BabeConfigMigration");
        pallet_babe::migrations::add_epoch_configuration::<Runtime>(BABE_GENESIS_EPOCH_CONFIG)
    }
}

/// https://github.com/paritytech/substrate/pull/8113
pub struct StakingV6Migration;

impl OnRuntimeUpgrade for StakingV6Migration {
    fn on_runtime_upgrade() -> Weight {
        frame_support::log::warn!("Run migration StakingV6Migration");
        pallet_staking::migrations::v6::migrate::<Runtime>()
    }
}

/// https://github.com/paritytech/substrate/pull/8221
pub struct SystemDualRefToTripleRefMigration;

impl frame_system::migrations::V2ToV3 for SystemDualRefToTripleRefMigration {
    type Pallet = System;
    type AccountId = <Runtime as frame_system::Config>::AccountId;
    type Index = <Runtime as frame_system::Config>::Index;
    type AccountData = <Runtime as frame_system::Config>::AccountData;
}

impl OnRuntimeUpgrade for SystemDualRefToTripleRefMigration {
    fn on_runtime_upgrade() -> Weight {
        frame_support::log::warn!("Run migration SystemDualRefToTripleRefMigration");
        frame_system::migrations::migrate_from_dual_to_triple_ref_count::<Self, Runtime>()
    }
}

/// https://github.com/paritytech/substrate/pull/8414
pub struct OffencesUpdateMigration;

impl OnRuntimeUpgrade for OffencesUpdateMigration {
    fn on_runtime_upgrade() -> Weight {
        frame_support::log::warn!("Run migration OffencesUpdateMigration");
        pallet_offences::migration::remove_deferred_storage::<Runtime>()
    }
}

/// https://github.com/paritytech/substrate/pull/8724
pub struct GrandpaStoragePrefixMigration;

impl OnRuntimeUpgrade for GrandpaStoragePrefixMigration {
    fn on_runtime_upgrade() -> frame_support::weights::Weight {
        frame_support::log::warn!("Run migration GrandpaStoragePrefixMigration");
        let name = <Runtime as frame_system::Config>::PalletInfo::name::<Grandpa>()
            .expect("grandpa is part of pallets in construct_runtime, so it has a name; qed");
        pallet_grandpa::migrations::v4::migrate::<Runtime, _>(name)
    }
}

/// https://github.com/paritytech/substrate/pull/8920
pub struct StakingV7Migration;

impl OnRuntimeUpgrade for StakingV7Migration {
    fn on_runtime_upgrade() -> Weight {
        frame_support::log::warn!("Run migration StakingV7Migration");
        pallet_staking::migrations::v7::migrate::<Runtime>()
    }
}

/// https://github.com/paritytech/substrate/pull/9080
pub struct TechnicalMembershipStoragePrefixMigration;

const TECHNICAL_MEMBERSHIP_OLD_PREFIX: &str = "Instance1Membership";

impl OnRuntimeUpgrade for TechnicalMembershipStoragePrefixMigration {
    fn on_runtime_upgrade() -> Weight {
        frame_support::log::warn!("Run migration TechnicalMembershipStoragePrefixMigration");
        let name = <Runtime as frame_system::Config>::PalletInfo::name::<TechnicalMembership>()
            .expect("TechnicalMembership is part of runtime, so it has a name; qed");
        pallet_membership::migrations::v4::migrate::<Runtime, TechnicalMembership, _>(
            TECHNICAL_MEMBERSHIP_OLD_PREFIX,
            name,
        )
    }
}

/// https://github.com/paritytech/substrate/pull/9115
pub struct CouncilStoragePrefixMigration;

const COUNCIL_OLD_PREFIX: &str = "Instance1Collective";

impl OnRuntimeUpgrade for CouncilStoragePrefixMigration {
    fn on_runtime_upgrade() -> Weight {
        frame_support::log::warn!("Run migration CouncilStoragePrefixMigration");
        pallet_collective::migrations::v4::migrate::<Runtime, Council, _>(COUNCIL_OLD_PREFIX)
    }
}

pub struct TechnicalCommitteeStoragePrefixMigration;

const TECHNICAL_COMMITTEE_OLD_PREFIX: &str = "Instance2Collective";

impl OnRuntimeUpgrade for TechnicalCommitteeStoragePrefixMigration {
    fn on_runtime_upgrade() -> Weight {
        frame_support::log::warn!("Run migration TechnicalCommitteeStoragePrefixMigration");
        pallet_collective::migrations::v4::migrate::<Runtime, TechnicalCommittee, _>(
            TECHNICAL_COMMITTEE_OLD_PREFIX,
        )
    }
}

/// https://github.com/paritytech/substrate/pull/9165
/// Migrate from `PalletVersion` to the new `StorageVersion`
pub struct MigratePalletVersionToStorageVersion;

impl OnRuntimeUpgrade for MigratePalletVersionToStorageVersion {
    fn on_runtime_upgrade() -> frame_support::weights::Weight {
        frame_support::log::warn!("Run migration MigratePalletVersionToStorageVersion");
        frame_support::migrations::migrate_from_pallet_version_to_storage_version::<
            AllPalletsWithSystem,
        >(&RocksDbWeight::get());
        <Runtime as frame_system::Config>::BlockWeights::get().max_block
    }
}

/// https://github.com/paritytech/substrate/pull/9507
pub struct StakingV8Migration;

impl OnRuntimeUpgrade for StakingV8Migration {
    fn on_runtime_upgrade() -> Weight {
        frame_support::log::warn!("Run migration StakingV8Migration");
        pallet_staking::migrations::v8::migrate::<Runtime>()
    }
}

/// https://github.com/paritytech/substrate/pull/9878
pub struct HistoricalStoragePrefixMigration;

impl OnRuntimeUpgrade for HistoricalStoragePrefixMigration {
    fn on_runtime_upgrade() -> Weight {
        frame_support::log::warn!("Run migration HistoricalStoragePrefixMigration");
        pallet_session::migrations::v1::migrate::<Runtime, Historical>()
    }
}

/// https://github.com/paritytech/substrate/pull/10356
pub struct SchedulerV3Migration;

impl OnRuntimeUpgrade for SchedulerV3Migration {
    fn on_runtime_upgrade() -> Weight {
        frame_support::log::warn!("Run migration SchedulerV3Migration");
        Scheduler::migrate_v2_to_v3()
    }
}

/// https://github.com/paritytech/substrate/pull/10649
pub struct ElectionsPhragmenV5Migration;

impl OnRuntimeUpgrade for ElectionsPhragmenV5Migration {
    fn on_runtime_upgrade() -> Weight {
        frame_support::log::warn!("Run migration ElectionsPhragmenV5Migration");
        // TODO: Find affected accounts and migrate them.
        pallet_elections_phragmen::migrations::v5::migrate::<Runtime>(Default::default())
    }
}

/// https://github.com/paritytech/substrate/pull/10821
pub struct StakingV9Migration;

impl OnRuntimeUpgrade for StakingV9Migration {
    fn on_runtime_upgrade() -> Weight {
        frame_support::log::warn!("Run migration StakingV9Migration");
        pallet_staking::migrations::v9::InjectValidatorsIntoVoterList::<Runtime>::on_runtime_upgrade(
        )
    }
}
