use crate::*;

pub struct StakingMigrationV11OldPallet;
impl Get<&'static str> for StakingMigrationV11OldPallet {
    fn get() -> &'static str {
        "BagsList"
    }
}

pub type Migrations = (
    pallet_staking::migrations::v10::MigrateToV10<Runtime>,
    pallet_staking::migrations::v11::MigrateToV11<Runtime, BagsList, StakingMigrationV11OldPallet>,
    pallet_staking::migrations::v12::MigrateToV12<Runtime>,
    pallet_preimage::migration::v1::Migration<Runtime>,
    pallet_scheduler::migration::v3::MigrateToV4<Runtime>,
    pallet_democracy::migrations::v1::Migration<Runtime>,
    pallet_multisig::migrations::v1::MigrateToV1<Runtime>,
);
