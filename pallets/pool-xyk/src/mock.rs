use crate::Trait;
use common::{balance, prelude::Balance};
use currencies::BasicCurrencyAdapter;
use frame_support::{impl_outer_origin, parameter_types, weights::Weight};
use frame_system as system;
use hex_literal::hex;
use permissions::{Scope, INIT_DEX, TRANSFER};
use sp_core::crypto::AccountId32;
use sp_core::H256;
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
    Perbill,
};

pub use common::{mock::ComicAssetId::*, mock::*, TechAssetId as Tas, TechPurpose::*, TradingPair};

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

impl_outer_origin! {
    pub enum Origin for Testtime {}
}

// Configure a mock runtime to test the pallet.

#[derive(Clone, Eq, PartialEq)]
pub struct Testtime;
parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = 1024;
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
}

impl system::Trait for Testtime {
    type BaseCallFilter = ();
    type Origin = Origin;
    type Call = ();
    type Index = u64;
    type BlockNumber = BlockNumber;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type Event = ();
    type BlockHashCount = BlockHashCount;
    type MaximumBlockWeight = MaximumBlockWeight;
    type DbWeight = ();
    type BlockExecutionWeight = ();
    type ExtrinsicBaseWeight = ();
    type MaximumExtrinsicWeight = MaximumBlockWeight;
    type MaximumBlockLength = MaximumBlockLength;
    type AvailableBlockRatio = AvailableBlockRatio;
    type Version = ();
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type PalletInfo = ();
}

impl permissions::Trait for Testtime {
    type Event = ();
}

impl dex_manager::Trait for Testtime {
    type Event = ();
    type WeightInfo = ();
}

impl trading_pair::Trait for Testtime {
    type Event = ();
    type EnsureDEXManager = dex_manager::Module<Testtime>;
    type WeightInfo = ();
}

pub type DEXId = u32;

pub type BlockNumber = u64;
pub type AccountId = AccountId32;
pub type Amount = i128;

impl common::Trait for Testtime {
    type DEXId = DEXId;
    type LstId = common::LiquiditySourceType;
}

parameter_types! {
    pub GetBaseAssetId: AssetId = common::AssetId32::from_bytes(hex!("0200000000000000000000000000000000000000000000000000000000000000").into());
    pub GetIncentiveAssetId: AssetId = common::AssetId32::from_bytes(hex!("0200050000000000000000000000000000000000000000000000000000000000").into());
}

parameter_types! {
    pub const ExistentialDeposit: u128 = 0;
}

pub type System = frame_system::Module<Testtime>;

impl pallet_balances::Trait for Testtime {
    type Balance = Balance;
    type Event = ();
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
    type MaxLocks = ();
}

impl tokens::Trait for Testtime {
    type Event = ();
    type Balance = Balance;
    type Amount = Amount;
    type CurrencyId = <Testtime as assets::Trait>::AssetId;
    type OnReceived = ();
    type WeightInfo = ();
}

impl currencies::Trait for Testtime {
    type Event = ();
    type MultiCurrency = tokens::Module<Testtime>;
    type NativeCurrency =
        BasicCurrencyAdapter<Testtime, pallet_balances::Module<Testtime>, Amount, BlockNumber>;
    type GetNativeCurrencyId = <Testtime as assets::Trait>::GetBaseAssetId;
    type WeightInfo = ();
}

impl assets::Trait for Testtime {
    type Event = ();
    type ExtraAccountId = [u8; 32];
    type ExtraTupleArg = common::AssetIdExtraTupleArg<DEXId, common::LiquiditySourceType, [u8; 32]>;
    type AssetId = AssetId;
    type GetBaseAssetId = GetBaseAssetId;
    type Currency = currencies::Module<Testtime>;
    type WeightInfo = ();
}

pub type TechAssetId = common::TechAssetId<common::mock::ComicAssetId>;
pub type AssetId = common::AssetId32<common::mock::ComicAssetId>;
pub type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;

impl technical::Trait for Testtime {
    type Event = ();
    type TechAssetId = TechAssetId;
    type TechAccountId = TechAccountId;
    type Trigger = ();
    type Condition = ();
    type SwapAction =
        crate::PolySwapAction<AssetId, TechAssetId, Balance, AccountId, TechAccountId>;
    type WeightInfo = ();
}

parameter_types! {
    pub GetPswapDistributionAccountId: AccountId = AccountId32::from([3; 32]);
    pub const GetDefaultSubscriptionFrequency: BlockNumber = 10;
}

impl pswap_distribution::Trait for Testtime {
    type Event = ();
    type GetIncentiveAssetId = GetIncentiveAssetId;
    type LiquidityProxy = ();
    type CompatBalance = Balance;
    type GetDefaultSubscriptionFrequency = GetDefaultSubscriptionFrequency;
    type GetTechnicalAccountId = GetPswapDistributionAccountId;
    type EnsureDEXManager = ();
}

impl Trait for Testtime {
    type Event = ();
    type PairSwapAction = crate::PairSwapAction<AssetId, Balance, AccountId, TechAccountId>;
    type DepositLiquidityAction =
        crate::DepositLiquidityAction<AssetId, TechAssetId, Balance, AccountId, TechAccountId>;
    type WithdrawLiquidityAction =
        crate::WithdrawLiquidityAction<AssetId, TechAssetId, Balance, AccountId, TechAccountId>;
    type PolySwapAction =
        crate::PolySwapAction<AssetId, TechAssetId, Balance, AccountId, TechAccountId>;
    type EnsureDEXManager = dex_manager::Module<Testtime>;
    type WeightInfo = ();
}

impl ExtBuilder {
    pub fn build(self) -> sp_io::TestExternalities {
        let mut t = system::GenesisConfig::default()
            .build_storage::<Testtime>()
            .unwrap();

        tokens::GenesisConfig::<Testtime> {
            endowed_accounts: self.endowed_accounts,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        permissions::GenesisConfig::<Testtime> {
            initial_permission_owners: self.initial_permission_owners,
            initial_permissions: self.initial_permissions,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        t.into()
    }
}
