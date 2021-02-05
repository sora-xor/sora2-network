use crate::{GenesisConfig, Module, Trait};
use common::prelude::Balance;
use common::{
    fixed, fixed_from_basis_points, AssetSymbol, BalancePrecision, Fixed, FromGenericPair,
};
use currencies::BasicCurrencyAdapter;
use frame_support::{impl_outer_origin, parameter_types, weights::Weight};
use frame_system as system;
use hex_literal::hex;
use permissions::Scope;
use sp_core::H256;
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup, Zero},
    Perbill,
};

pub type AccountId = u32;
pub type BlockNumber = u64;
pub type Amount = i128;
pub type AssetId = common::AssetId32<common::AssetId>;

pub const ALICE: AccountId = 1;
pub const FEES_ACCOUNT_A: AccountId = 11;
pub const FEES_ACCOUNT_B: AccountId = 12;
pub const LIQUIDITY_PROVIDER_A: AccountId = 21;
pub const LIQUIDITY_PROVIDER_B: AccountId = 22;
pub const LIQUIDITY_PROVIDER_C: AccountId = 23;
pub const DEX_A_ID: DEXId = common::DEXId::Polkaswap;

type TechAssetId = common::TechAssetId<common::AssetId, DEXId, common::LiquiditySourceType>;
pub type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;

impl_outer_origin! {
    pub enum Origin for Runtime {}
}

parameter_types! {
    pub GetBaseAssetId: AssetId = common::XOR.into();
    pub GetIncentiveAssetId: AssetId = common::PSWAP.into();
    pub const PoolTokenAId: AssetId = common::AssetId32::from_bytes(hex!("0211110000000000000000000000000000000000000000000000000000000000"));
    pub const PoolTokenBId: AssetId = common::AssetId32::from_bytes(hex!("0222220000000000000000000000000000000000000000000000000000000000"));
}

#[derive(Clone, Eq, PartialEq)]
pub struct Runtime;
parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = 1024;
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    pub const GetDefaultFee: u16 = 30;
    pub const GetDefaultProtocolFee: u16 = 0;
}

impl system::Trait for Runtime {
    type BaseCallFilter = ();
    type Origin = Origin;
    type Call = ();
    type Index = u64;
    type BlockNumber = u64;
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

parameter_types! {
    pub GetPswapDistributionTechAccountId: TechAccountId = {
        let tech_account_id = TechAccountId::from_generic_pair(
            crate::TECH_ACCOUNT_PREFIX.to_vec(),
            crate::TECH_ACCOUNT_MAIN.to_vec(),
        );
        tech_account_id
    };
    pub GetPswapDistributionAccountId: AccountId = {
        let tech_account_id = GetPswapDistributionTechAccountId::get();
        let account_id =
            technical::Module::<Runtime>::tech_account_id_to_account_id(&tech_account_id)
                .expect("Failed to get ordinary account id for technical account id.");
        account_id
    };
    pub const GetDefaultSubscriptionFrequency: BlockNumber = 10;
}

impl Trait for Runtime {
    type Event = ();
    type GetIncentiveAssetId = GetIncentiveAssetId;
    type LiquidityProxy = ();
    type CompatBalance = Balance;
    type GetDefaultSubscriptionFrequency = GetDefaultSubscriptionFrequency;
    type GetTechnicalAccountId = GetPswapDistributionAccountId;
    type EnsureDEXManager = DexManager;
}

impl tokens::Trait for Runtime {
    type Event = ();
    type Balance = Balance;
    type Amount = Amount;
    type CurrencyId = <Runtime as assets::Trait>::AssetId;
    type OnReceived = ();
    type WeightInfo = ();
}

impl permissions::Trait for Runtime {
    type Event = ();
}

impl currencies::Trait for Runtime {
    type Event = ();
    type MultiCurrency = Tokens;
    type NativeCurrency =
        BasicCurrencyAdapter<Runtime, pallet_balances::Module<Runtime>, Amount, BlockNumber>;
    type GetNativeCurrencyId = <Runtime as assets::Trait>::GetBaseAssetId;
    type WeightInfo = ();
}

type DEXId = common::DEXId;

impl assets::Trait for Runtime {
    type Event = ();
    type AssetId = AssetId;
    type GetBaseAssetId = GetBaseAssetId;
    type Currency = currencies::Module<Runtime>;
    type WeightInfo = ();
}

impl common::Trait for Runtime {
    type DEXId = DEXId;
}

parameter_types! {
    pub const ExistentialDeposit: u128 = 1;
    pub const TransferFee: u128 = 0;
    pub const CreationFee: u128 = 0;
    pub const TransactionByteFee: u128 = 1;
}

impl pallet_balances::Trait for Runtime {
    type Balance = Balance;
    type Event = ();
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
    type MaxLocks = ();
}

parameter_types! {
    pub GetFee: Fixed = fixed_from_basis_points(30u16);
}

impl technical::Trait for Runtime {
    type Event = ();
    type TechAssetId = TechAssetId;
    type TechAccountId = TechAccountId;
    type Trigger = ();
    type Condition = ();
    type SwapAction = ();
    type WeightInfo = ();
}

impl dex_manager::Trait for Runtime {
    type Event = ();
    type WeightInfo = ();
}

pub type System = frame_system::Module<Runtime>;
pub type Tokens = tokens::Module<Runtime>;
pub type PswapDistrModule = Module<Runtime>;
pub type DexManager = dex_manager::Module<Runtime>;
pub type Assets = assets::Module<Runtime>;

pub struct ExtBuilder {
    endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
    endowed_assets: Vec<(
        AssetId,
        AccountId,
        AssetSymbol,
        BalancePrecision,
        Balance,
        bool,
    )>,
    initial_permission_owners: Vec<(u32, Scope, Vec<AccountId>)>,
    initial_permissions: Vec<(AccountId, Scope, Vec<u32>)>,
    subscribed_accounts: Vec<(AccountId, (DEXId, AssetId, BlockNumber, BlockNumber))>,
    burn_info: (Fixed, Fixed, Fixed, BlockNumber),
}

impl ExtBuilder {
    pub fn uninitialized() -> Self {
        Self {
            endowed_accounts: Vec::new(),
            endowed_assets: vec![(
                PoolTokenAId::get(),
                ALICE,
                AssetSymbol(b"POOL".to_vec()),
                18,
                Balance::from(0u32),
                true,
            )],
            initial_permission_owners: Vec::new(),
            initial_permissions: Vec::new(),
            subscribed_accounts: Vec::new(),
            burn_info: (fixed!(0), fixed!(0.10), fixed!(0.30), 3),
        }
    }
}

impl ExtBuilder {
    pub fn with_accounts(accounts: Vec<(AccountId, AssetId, Balance)>) -> Self {
        let permissioned_account_id = GetPswapDistributionAccountId::get();
        Self {
            endowed_accounts: accounts,
            endowed_assets: vec![
                (
                    common::XOR.into(),
                    ALICE,
                    AssetSymbol(b"XOR".to_vec()),
                    18,
                    Balance::zero(),
                    true,
                ),
                (
                    common::PSWAP.into(),
                    ALICE,
                    AssetSymbol(b"PSWAP".to_vec()),
                    10,
                    Balance::zero(),
                    true,
                ),
                (
                    PoolTokenAId::get(),
                    ALICE,
                    AssetSymbol(b"POOLA".to_vec()),
                    18,
                    Balance::zero(),
                    true,
                ),
                (
                    PoolTokenBId::get(),
                    ALICE,
                    AssetSymbol(b"POOLB".to_vec()),
                    18,
                    Balance::zero(),
                    true,
                ),
            ],
            initial_permission_owners: vec![],
            initial_permissions: vec![(
                permissioned_account_id,
                Scope::Unlimited,
                vec![permissions::MINT, permissions::BURN],
            )],
            subscribed_accounts: vec![
                (FEES_ACCOUNT_A, (DEX_A_ID, PoolTokenAId::get(), 5, 0)),
                (FEES_ACCOUNT_B, (DEX_A_ID, PoolTokenBId::get(), 7, 0)),
            ],
            burn_info: (fixed!(0), fixed!(0.10), fixed!(0.30), 3),
        }
    }
}

impl Default for ExtBuilder {
    fn default() -> Self {
        ExtBuilder::with_accounts(vec![
            (FEES_ACCOUNT_A, common::XOR.into(), Balance(fixed!(1))),
            (FEES_ACCOUNT_A, common::PSWAP.into(), Balance(fixed!(6))),
            (
                LIQUIDITY_PROVIDER_A,
                PoolTokenAId::get(),
                Balance(fixed!(3)),
            ),
            (
                LIQUIDITY_PROVIDER_B,
                PoolTokenAId::get(),
                Balance(fixed!(2)),
            ),
            (
                LIQUIDITY_PROVIDER_C,
                PoolTokenAId::get(),
                Balance(fixed!(1)),
            ),
            (
                LIQUIDITY_PROVIDER_A,
                PoolTokenBId::get(),
                Balance(fixed!(10)),
            ),
            (
                LIQUIDITY_PROVIDER_B,
                PoolTokenBId::get(),
                Balance(fixed!(10)),
            ),
            (
                LIQUIDITY_PROVIDER_C,
                PoolTokenBId::get(),
                Balance(fixed!(10)),
            ),
        ])
    }
}

impl ExtBuilder {
    pub fn build(self) -> sp_io::TestExternalities {
        let mut t = system::GenesisConfig::default()
            .build_storage::<Runtime>()
            .unwrap();

        permissions::GenesisConfig::<Runtime> {
            initial_permissions: self.initial_permissions,
            initial_permission_owners: self.initial_permission_owners,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        tokens::GenesisConfig::<Runtime> {
            endowed_accounts: self.endowed_accounts,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        assets::GenesisConfig::<Runtime> {
            endowed_assets: self.endowed_assets,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        GenesisConfig::<Runtime> {
            subscribed_accounts: self.subscribed_accounts,
            burn_info: self.burn_info,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        t.into()
    }
}
