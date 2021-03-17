use crate::{self as pool_xyk, Config};
use common::balance;
use common::prelude::Balance;
use currencies::BasicCurrencyAdapter;
use frame_support::traits::GenesisBuild;
use frame_support::weights::Weight;
use frame_support::{construct_runtime, parameter_types};
use frame_system;
use hex_literal::hex;
use orml_traits::parameter_type_with_key;
use permissions::{Scope, INIT_DEX, TRANSFER};
use sp_core::crypto::AccountId32;
use sp_core::H256;
use sp_runtime::testing::Header;
use sp_runtime::traits::{BlakeTwo256, IdentityLookup};
use sp_runtime::Perbill;

pub use common::mock::ComicAssetId::*;
pub use common::mock::*;
pub use common::TechAssetId as Tas;
pub use common::TechPurpose::*;

pub type DEXId = u32;
pub type BlockNumber = u64;
pub type AccountId = AccountId32;
pub type Amount = i128;
pub type TechAssetId = common::TechAssetId<common::mock::ComicAssetId>;
pub type AssetId = common::AssetId32<common::mock::ComicAssetId>;
pub type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = 1024;
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    pub GetBaseAssetId: AssetId = common::AssetId32::from_bytes(hex!("0200000000000000000000000000000000000000000000000000000000000000").into());
    pub GetIncentiveAssetId: AssetId = common::AssetId32::from_bytes(hex!("0200050000000000000000000000000000000000000000000000000000000000").into());
    pub const ExistentialDeposit: u128 = 0;
    pub GetPswapDistributionAccountId: AccountId = AccountId32::from([3; 32]);
    pub const GetDefaultSubscriptionFrequency: BlockNumber = 10;
}

parameter_type_with_key! {
    pub ExistentialDeposits: |_currency_id: AssetId| -> Balance {
        0
    };
}

construct_runtime! {
    pub enum Runtime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Module, Call, Config, Storage, Event<T>},
        Permissions: permissions::{Module, Call, Config<T>, Storage, Event<T>},
        DexManager: dex_manager::{Module, Call, Config<T>, Storage, Event<T>},
        TradingPair: trading_pair::{Module, Call, Config<T>, Storage, Event<T>},
        Balances: pallet_balances::{Module, Call, Storage, Event<T>},
        Tokens: tokens::{Module, Call, Config<T>, Storage, Event<T>},
        Currencies: currencies::{Module, Call, Storage, Event<T>},
        Assets: assets::{Module, Call, Config<T>, Storage, Event<T>},
        Technical: technical::{Module, Call, Config<T>, Storage, Event<T>},
        PswapDistribution: pswap_distribution::{Module, Call, Config<T>, Storage, Event<T>},
        PoolXyk: pool_xyk::{Module, Call, Storage, Event<T>},
    }
}

impl frame_system::Config for Runtime {
    type BaseCallFilter = ();
    type BlockWeights = ();
    type BlockLength = ();
    type Origin = Origin;
    type Call = Call;
    type Index = u64;
    type BlockNumber = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type Event = Event;
    type BlockHashCount = BlockHashCount;
    type DbWeight = ();
    type Version = ();
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type PalletInfo = PalletInfo;
    type SS58Prefix = ();
}

impl permissions::Config for Runtime {
    type Event = Event;
}

impl dex_manager::Config for Runtime {
    type Event = Event;
    type WeightInfo = ();
}

impl trading_pair::Config for Runtime {
    type Event = Event;
    type EnsureDEXManager = dex_manager::Module<Runtime>;
    type WeightInfo = ();
}

impl common::Config for Runtime {
    type DEXId = DEXId;
    type LstId = common::LiquiditySourceType;
}

impl pallet_balances::Config for Runtime {
    type Balance = Balance;
    type Event = Event;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
    type MaxLocks = ();
}

impl tokens::Config for Runtime {
    type Event = Event;
    type Balance = Balance;
    type Amount = Amount;
    type CurrencyId = <Runtime as assets::Config>::AssetId;
    type WeightInfo = ();
    type ExistentialDeposits = ExistentialDeposits;
    type OnDust = ();
}

impl currencies::Config for Runtime {
    type Event = Event;
    type MultiCurrency = tokens::Module<Runtime>;
    type NativeCurrency =
        BasicCurrencyAdapter<Runtime, pallet_balances::Module<Runtime>, Amount, BlockNumber>;
    type GetNativeCurrencyId = <Runtime as assets::Config>::GetBaseAssetId;
    type WeightInfo = ();
}

impl assets::Config for Runtime {
    type Event = Event;
    type ExtraAccountId = [u8; 32];
    type ExtraAssetRecordArg =
        common::AssetIdExtraAssetRecordArg<DEXId, common::LiquiditySourceType, [u8; 32]>;
    type AssetId = AssetId;
    type GetBaseAssetId = GetBaseAssetId;
    type Currency = currencies::Module<Runtime>;
    type WeightInfo = ();
}

impl technical::Config for Runtime {
    type Event = Event;
    type TechAssetId = TechAssetId;
    type TechAccountId = TechAccountId;
    type Trigger = ();
    type Condition = ();
    type SwapAction =
        crate::PolySwapAction<AssetId, TechAssetId, Balance, AccountId, TechAccountId>;
    type WeightInfo = ();
}

impl pswap_distribution::Config for Runtime {
    type Event = Event;
    type GetIncentiveAssetId = GetIncentiveAssetId;
    type LiquidityProxy = ();
    type CompatBalance = Balance;
    type GetDefaultSubscriptionFrequency = GetDefaultSubscriptionFrequency;
    type GetTechnicalAccountId = GetPswapDistributionAccountId;
    type EnsureDEXManager = ();
    type OnPswapBurnedAggregator = ();
}

impl Config for Runtime {
    type Event = Event;
    type PairSwapAction = crate::PairSwapAction<AssetId, Balance, AccountId, TechAccountId>;
    type DepositLiquidityAction =
        crate::DepositLiquidityAction<AssetId, TechAssetId, Balance, AccountId, TechAccountId>;
    type WithdrawLiquidityAction =
        crate::WithdrawLiquidityAction<AssetId, TechAssetId, Balance, AccountId, TechAccountId>;
    type PolySwapAction =
        crate::PolySwapAction<AssetId, TechAssetId, Balance, AccountId, TechAccountId>;
    type EnsureDEXManager = dex_manager::Module<Runtime>;
    type WeightInfo = ();
}

#[allow(non_snake_case)]
pub fn ALICE() -> AccountId {
    AccountId32::from([1; 32])
}

#[allow(non_snake_case)]
pub fn BOB() -> AccountId {
    AccountId32::from([2; 32])
}

pub struct ExtBuilder {
    endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
    initial_permission_owners: Vec<(u32, Scope, Vec<AccountId>)>,
    initial_permissions: Vec<(AccountId, Scope, Vec<u32>)>,
}

impl Default for ExtBuilder {
    fn default() -> Self {
        Self {
            endowed_accounts: vec![
                (ALICE(), RedPepper.into(), balance!(99000)),
                (ALICE(), BlackPepper.into(), balance!(2000000)),
                (BOB(), RedPepper.into(), balance!(2000000)),
            ],
            initial_permission_owners: vec![
                (INIT_DEX, Scope::Unlimited, vec![BOB()]),
                (TRANSFER, Scope::Unlimited, vec![ALICE()]),
            ],
            initial_permissions: vec![(BOB(), Scope::Unlimited, vec![INIT_DEX])],
        }
    }
}

impl ExtBuilder {
    pub fn build(self) -> sp_io::TestExternalities {
        let mut t = frame_system::GenesisConfig::default()
            .build_storage::<Runtime>()
            .unwrap();

        tokens::GenesisConfig::<Runtime> {
            endowed_accounts: self.endowed_accounts,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        permissions::GenesisConfig::<Runtime> {
            initial_permission_owners: self.initial_permission_owners,
            initial_permissions: self.initial_permissions,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        t.into()
    }
}
