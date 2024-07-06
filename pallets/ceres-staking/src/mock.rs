use crate::{self as ceres_staking};
use common::mock::ExistentialDeposits;
use common::prelude::Balance;
pub use common::TechAssetId as Tas;
pub use common::TechPurpose::*;
use common::{
    balance, mock_assets_config, mock_common_config, mock_currencies_config,
    mock_frame_system_config, mock_pallet_balances_config, mock_permissions_config,
    mock_technical_config, mock_tokens_config, AssetId32, AssetName, AssetSymbol, BalancePrecision,
    ContentSource, DEXId, Description, CERES_ASSET_ID, XST,
};
use currencies::BasicCurrencyAdapter;
use frame_support::traits::{Everything, GenesisBuild, Hooks};
use frame_support::weights::Weight;
use frame_support::{construct_runtime, parameter_types};
use frame_system;
use frame_system::pallet_prelude::BlockNumberFor;
use hex_literal::hex;
use sp_core::crypto::AccountId32;
use sp_core::H256;
use sp_runtime::testing::Header;
use sp_runtime::traits::{BlakeTwo256, IdentityLookup, Zero};
use sp_runtime::{BuildStorage, Perbill};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;
pub const BLOCKS_PER_DAY: BlockNumberFor<Runtime> = 14_440;

construct_runtime! {
    pub enum Runtime {
        System: frame_system::{Pallet, Call, Config<T>, Storage, Event<T>},
        Assets: assets::{Pallet, Call, Config<T>, Storage, Event<T>},
        Tokens: tokens::{Pallet, Call, Config<T>, Storage, Event<T>},
        Currencies: currencies::{Pallet, Call, Storage},
        Balances: pallet_balances::{Pallet, Call, Storage, Event<T>},
        Permissions: permissions::{Pallet, Call, Config<T>, Storage, Event<T>},
        Technical: technical::{Pallet, Call, Config<T>, Storage, Event<T>},
        CeresStaking: ceres_staking::{Pallet, Call, Storage, Event<T>},
    }
}

pub type AccountId = AccountId32;
pub type BlockNumber = u64;
pub type Amount = i128;
pub type AssetId = AssetId32<common::PredefinedAssetId>;
pub type TechAssetId = common::TechAssetId<common::PredefinedAssetId>;
pub type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;

pub const ALICE: AccountId = AccountId::new(hex!(
    "0000000000000000000000000000000000000000000000000000000000000001"
));
pub const BOB: AccountId = AccountId::new(hex!(
    "0000000000000000000000000000000000000000000000000000000000000002"
));

mock_technical_config!(Runtime);
mock_currencies_config!(Runtime);
/* mock_pallet_balances_config!(Runtime); */
mock_frame_system_config!(Runtime);
mock_common_config!(Runtime);
mock_tokens_config!(Runtime);
mock_assets_config!(Runtime);
mock_permissions_config!(Runtime);

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = Weight::from_parts(1024, 0);
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
}

parameter_types! {
    pub const CeresPerDay: Balance = balance!(6.66666666667);
    pub const CeresAssetId: AssetId = CERES_ASSET_ID;
    pub const MaximumCeresInStakingPool: Balance = balance!(7200);
}

impl crate::Config for Runtime {
    const BLOCKS_PER_ONE_DAY: BlockNumberFor<Self> = BLOCKS_PER_DAY;
    type RuntimeEvent = RuntimeEvent;
    type CeresPerDay = CeresPerDay;
    type CeresAssetId = CeresAssetId;
    type MaximumCeresInStakingPool = MaximumCeresInStakingPool;
    type WeightInfo = ();
}

parameter_types! {
    pub const GetBaseAssetId: AssetId = CERES_ASSET_ID;
    pub const GetBuyBackAssetId: AssetId = XST;
}

parameter_types! {
    pub const ExistentialDeposit: u128 = 1;
    pub const TransferFee: u128 = 0;
    pub const CreationFee: u128 = 0;
    pub const TransactionByteFee: u128 = 1;
}

impl pallet_balances::Config for Runtime {
    type Balance = Balance;
    type DustRemoval = ();
    type RuntimeEvent = RuntimeEvent;
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
    type MaxLocks = ();
    type MaxReserves = ();
    type ReserveIdentifier = ();
    type RuntimeHoldReason = ();
    type FreezeIdentifier = ();
    type MaxHolds = ();
    type MaxFreezes = ();
}

pub struct ExtBuilder {
    pub endowed_assets: Vec<(
        AssetId,
        AccountId,
        AssetSymbol,
        AssetName,
        BalancePrecision,
        Balance,
        bool,
        Option<ContentSource>,
        Option<Description>,
    )>,
    pub endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
}

impl Default for ExtBuilder {
    fn default() -> Self {
        Self {
            endowed_assets: vec![(
                CERES_ASSET_ID,
                ALICE,
                AssetSymbol(b"CERES".to_vec()),
                AssetName(b"Ceres".to_vec()),
                18,
                Balance::zero(),
                true,
                None,
                None,
            )],
            endowed_accounts: vec![
                (ALICE, CERES_ASSET_ID, balance!(7300)),
                (BOB, CERES_ASSET_ID, balance!(100)),
            ],
        }
    }
}

impl ExtBuilder {
    #[allow(dead_code)]
    pub fn empty() -> Self {
        ExtBuilder {
            endowed_assets: vec![],
            endowed_accounts: vec![],
        }
    }

    pub fn build(self) -> sp_io::TestExternalities {
        let mut t = SystemConfig::default().build_storage().unwrap();

        pallet_balances::GenesisConfig::<Runtime> {
            balances: self
                .endowed_accounts
                .iter()
                .map(|(acc, _, balance)| (acc.clone(), *balance))
                .collect(),
        }
        .assimilate_storage(&mut t)
        .unwrap();

        PermissionsConfig {
            initial_permission_owners: vec![],
            initial_permissions: vec![],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        assets::GenesisConfig::<Runtime> {
            endowed_assets: self.endowed_assets,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        TokensConfig {
            balances: self.endowed_accounts,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        t.into()
    }
}

pub fn run_to_block(n: u64) {
    while System::block_number() < n {
        System::on_finalize(System::block_number());
        System::set_block_number(System::block_number() + 1);
        System::on_initialize(System::block_number());
        CeresStaking::on_initialize(System::block_number());
    }
}
