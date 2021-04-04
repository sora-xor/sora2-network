#![cfg(test)]

use crate::{Config, *};
use common::mock::ExistentialDeposits;
use common::prelude::Balance;
use common::{
    fixed_from_basis_points, hash, Amount, AssetId32, BalancePrecision, DEXInfo, Fixed,
    FromGenericPair, LiquiditySourceType, TechPurpose,
};
use currencies::BasicCurrencyAdapter;

use frame_support::traits::GenesisBuild;
use frame_support::{construct_runtime, parameter_types};
use permissions::{Scope, BURN, MANAGE_DEX, MINT, TRANSFER};
use sp_core::H256;
use sp_runtime::testing::Header;
use sp_runtime::traits::{BlakeTwo256, IdentityLookup};
use sp_runtime::AccountId32;

pub type DEXId = u32;
pub type AssetId = AssetId32<common::AssetId>;
pub type TechAssetId = common::TechAssetId<common::AssetId>;
pub type AccountId = AccountId32;
pub type BlockNumber = u64;
type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
type ReservesInit = Vec<(DEXId, AssetId, (Fixed, Fixed))>;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

pub fn alice() -> AccountId {
    AccountId32::from(hex!(
        "d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"
    ))
}

parameter_types! {
    pub GetLiquidityProxyTechAccountId: TechAccountId = {
        let tech_account_id = TechAccountId::from_generic_pair(
            liquidity_proxy::TECH_ACCOUNT_PREFIX.to_vec(),
            liquidity_proxy::TECH_ACCOUNT_MAIN.to_vec(),
        );
        tech_account_id
    };
    pub GetLiquidityProxyAccountId: AccountId = {
        let tech_account_id = GetLiquidityProxyTechAccountId::get();
        let account_id =
            technical::Module::<Runtime>::tech_account_id_to_account_id(&tech_account_id)
                .expect("Failed to get ordinary account id for technical account id.");
        account_id
    };
    pub const BlockHashCount: u64 = 250;
    pub const GetNumSamples: usize = 40;
    pub const GetBaseAssetId: AssetId = XOR;
    pub const ExistentialDeposit: u128 = 1;
    pub GetFee: Fixed = fixed_from_basis_points(0u16);
    pub GetPswapDistributionAccountId: AccountId = AccountId32::from([3; 32]);
    pub const GetDefaultSubscriptionFrequency: BlockNumber = 10;
    pub const GetBurnUpdateFrequency: BlockNumber = 10;
    pub GetIncentiveAssetId: AssetId = common::PSWAP.into();
}

construct_runtime! {
    pub enum Runtime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Module, Call, Config, Storage, Event<T>},
        LiquidityProxy: liquidity_proxy::{Module, Call, Event<T>},
        Tokens: tokens::{Module, Call, Config<T>, Storage, Event<T>},
        Currencies: currencies::{Module, Call, Storage, Event<T>},
        Assets: assets::{Module, Call, Config<T>, Storage, Event<T>},
        Balances: pallet_balances::{Module, Call, Storage, Event<T>},
        DexManager: dex_manager::{Module, Call, Config<T>, Storage},
        Technical: technical::{Module, Call, Config<T>, Storage, Event<T>},
        Permissions: permissions::{Module, Call, Config<T>, Storage, Event<T>},
        DexApi: dex_api::{Module, Call, Config, Storage, Event<T>},
        TradingPair: trading_pair::{Module, Call, Config<T>, Storage, Event<T>},
        PoolXyk: pool_xyk::{Module, Call, Storage, Event<T>},
        MBCPool: multicollateral_bonding_curve_pool::{Module, Call, Storage, Event<T>},
        PswapDistribution: pswap_distribution::{Module, Call, Config<T>, Storage, Event<T>},
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

impl liquidity_proxy::Config for Runtime {
    type Event = Event;
    type LiquidityRegistry = dex_api::Module<Runtime>;
    type GetNumSamples = GetNumSamples;
    type GetTechnicalAccountId = GetLiquidityProxyAccountId;
    type WeightInfo = ();
    type PrimaryMarket = ();
    type SecondaryMarket = ();
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
    type MultiCurrency = Tokens;
    type NativeCurrency = BasicCurrencyAdapter<Runtime, Balances, Amount, BlockNumber>;
    type GetNativeCurrencyId = GetBaseAssetId;
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

impl common::Config for Runtime {
    type DEXId = DEXId;
    type LstId = common::LiquiditySourceType;
}

impl pallet_balances::Config for Runtime {
    type Balance = Balance;
    type DustRemoval = ();
    type Event = Event;
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
    type MaxLocks = ();
}

impl dex_manager::Config for Runtime {
    type WeightInfo = ();
}

impl mock_liquidity_source::Config<mock_liquidity_source::Instance1> for Runtime {
    type GetFee = GetFee;
    type EnsureDEXManager = dex_manager::Module<Runtime>;
    type EnsureTradingPairExists = trading_pair::Module<Runtime>;
}

impl mock_liquidity_source::Config<mock_liquidity_source::Instance2> for Runtime {
    type GetFee = GetFee;
    type EnsureDEXManager = dex_manager::Module<Runtime>;
    type EnsureTradingPairExists = trading_pair::Module<Runtime>;
}

impl mock_liquidity_source::Config<mock_liquidity_source::Instance3> for Runtime {
    type GetFee = GetFee;
    type EnsureDEXManager = dex_manager::Module<Runtime>;
    type EnsureTradingPairExists = trading_pair::Module<Runtime>;
}

impl mock_liquidity_source::Config<mock_liquidity_source::Instance4> for Runtime {
    type GetFee = GetFee;
    type EnsureDEXManager = dex_manager::Module<Runtime>;
    type EnsureTradingPairExists = trading_pair::Module<Runtime>;
}

impl technical::Config for Runtime {
    type Event = Event;
    type TechAssetId = TechAssetId;
    type TechAccountId = TechAccountId;
    type Trigger = ();
    type Condition = ();
    type SwapAction =
        pool_xyk::PolySwapAction<AssetId, TechAssetId, Balance, AccountId, TechAccountId>;
    type WeightInfo = ();
}

impl permissions::Config for Runtime {
    type Event = Event;
}

impl dex_api::Config for Runtime {
    type Event = Event;
    type MockLiquiditySource = ();
    type MockLiquiditySource2 = ();
    type MockLiquiditySource3 = ();
    type MockLiquiditySource4 = ();
    type XYKPool = pool_xyk::Module<Runtime>;
    type BondingCurvePool = ();
    type MulticollateralBondingCurvePool = multicollateral_bonding_curve_pool::Module<Runtime>;
    type WeightInfo = ();
}

impl trading_pair::Config for Runtime {
    type Event = Event;
    type EnsureDEXManager = dex_manager::Module<Runtime>;
    type WeightInfo = ();
}

impl pool_xyk::Config for Runtime {
    type Event = Event;
    type PairSwapAction = pool_xyk::PairSwapAction<AssetId, Balance, AccountId, TechAccountId>;
    type DepositLiquidityAction =
        pool_xyk::DepositLiquidityAction<AssetId, TechAssetId, Balance, AccountId, TechAccountId>;
    type WithdrawLiquidityAction =
        pool_xyk::WithdrawLiquidityAction<AssetId, TechAssetId, Balance, AccountId, TechAccountId>;
    type PolySwapAction =
        pool_xyk::PolySwapAction<AssetId, TechAssetId, Balance, AccountId, TechAccountId>;
    type EnsureDEXManager = dex_manager::Module<Runtime>;
    type WeightInfo = ();
}

fn bonding_curve_distribution_accounts(
) -> DistributionAccounts<DistributionAccountData<<Runtime as technical::Config>::TechAccountId>> {
    use common::fixed_wrapper;
    use common::prelude::fixnum::ops::One;
    let val_holders_coefficient = fixed_wrapper!(0.5);
    let val_holders_xor_alloc_coeff = fixed_wrapper!(0.9) * val_holders_coefficient.clone();
    let val_holders_buy_back_coefficient =
        val_holders_coefficient.clone() * (fixed_wrapper!(1) - fixed_wrapper!(0.9));
    let projects_coefficient = fixed_wrapper!(1) - val_holders_coefficient;
    let projects_sora_citizens_coeff = projects_coefficient.clone() * fixed_wrapper!(0.01);
    let projects_stores_and_shops_coeff = projects_coefficient.clone() * fixed_wrapper!(0.04);
    let projects_parliament_and_development_coeff =
        projects_coefficient.clone() * fixed_wrapper!(0.05);
    let projects_other_coeff = projects_coefficient.clone() * fixed_wrapper!(0.9);

    let xor_allocation = DistributionAccountData::new(
        TechAccountId::Pure(0u32, TechPurpose::Identifier(b"xor_allocation".to_vec())),
        val_holders_xor_alloc_coeff.get().unwrap(),
    );
    let sora_citizens = DistributionAccountData::new(
        TechAccountId::Pure(0u32, TechPurpose::Identifier(b"sora_citizens".to_vec())),
        projects_sora_citizens_coeff.get().unwrap(),
    );
    let stores_and_shops = DistributionAccountData::new(
        TechAccountId::Pure(0u32, TechPurpose::Identifier(b"stores_and_shops".to_vec())),
        projects_stores_and_shops_coeff.get().unwrap(),
    );
    let parliament_and_development = DistributionAccountData::new(
        TechAccountId::Pure(
            0u32,
            TechPurpose::Identifier(b"parliament_and_development".to_vec()),
        ),
        projects_parliament_and_development_coeff.get().unwrap(),
    );
    let projects = DistributionAccountData::new(
        TechAccountId::Pure(0u32, TechPurpose::Identifier(b"projects".to_vec())),
        projects_other_coeff.get().unwrap(),
    );
    let val_holders = DistributionAccountData::new(
        TechAccountId::Pure(0u32, TechPurpose::Identifier(b"val_holders".to_vec())),
        val_holders_buy_back_coefficient.get().unwrap(),
    );
    DistributionAccounts::<_> {
        xor_allocation,
        sora_citizens,
        stores_and_shops,
        parliament_and_development,
        projects,
        val_holders,
    }
}

parameter_types! {
    pub GetMbcReservesTechAccountId: TechAccountId = {
        let tech_account_id = TechAccountId::from_generic_pair(
            multicollateral_bonding_curve_pool::TECH_ACCOUNT_PREFIX.to_vec(),
            multicollateral_bonding_curve_pool::TECH_ACCOUNT_RESERVES.to_vec(),
        );
        tech_account_id
    };
    pub GetMbcReservesAccountId: AccountId = {
        let tech_account_id = GetMbcReservesTechAccountId::get();
        let account_id =
            technical::Module::<Runtime>::tech_account_id_to_account_id(&tech_account_id)
                .expect("Failed to get ordinary account id for technical account id.");
        account_id
    };
    pub GetMbcRewardsTechAccountId: TechAccountId = {
        let tech_account_id = TechAccountId::from_generic_pair(
            multicollateral_bonding_curve_pool::TECH_ACCOUNT_PREFIX.to_vec(),
            multicollateral_bonding_curve_pool::TECH_ACCOUNT_REWARDS.to_vec(),
        );
        tech_account_id
    };
    pub GetMbcRewardsAccountId: AccountId = {
        let tech_account_id = GetMbcRewardsTechAccountId::get();
        let account_id =
            technical::Module::<Runtime>::tech_account_id_to_account_id(&tech_account_id)
                .expect("Failed to get ordinary account id for technical account id.");
        account_id
    };
}

impl multicollateral_bonding_curve_pool::Config for Runtime {
    type Event = Event;
    type LiquidityProxy = liquidity_proxy::Module<Runtime>;
    type EnsureDEXManager = dex_manager::Module<Runtime>;
    type EnsureTradingPairExists = trading_pair::Module<Runtime>;
    type WeightInfo = ();
}

impl pswap_distribution::Config for Runtime {
    type Event = Event;
    type GetIncentiveAssetId = GetIncentiveAssetId;
    type LiquidityProxy = liquidity_proxy::Module<Runtime>;
    type CompatBalance = Balance;
    type GetDefaultSubscriptionFrequency = GetDefaultSubscriptionFrequency;
    type GetBurnUpdateFrequency = GetBurnUpdateFrequency;
    type GetTechnicalAccountId = GetPswapDistributionAccountId;
    type EnsureDEXManager = ();
    type OnPswapBurnedAggregator = ();
}

impl Config for Runtime {}

pub struct ExtBuilder {
    reserves: ReservesInit,
    reserves_2: ReservesInit,
    reserves_3: ReservesInit,
    reserves_4: ReservesInit,
    dex_list: Vec<(DEXId, DEXInfo<AssetId>)>,
    initial_permission_owners: Vec<(u32, Scope, Vec<AccountId>)>,
    initial_permissions: Vec<(AccountId, Scope, Vec<u32>)>,
    source_types: Vec<LiquiditySourceType>,
    tech_accounts: Vec<(AccountId, TechAccountId)>,
    endowed_assets: Vec<(
        AssetId,
        AccountId,
        AssetSymbol,
        AssetName,
        BalancePrecision,
        Balance,
        bool,
    )>,
}

impl Default for ExtBuilder {
    fn default() -> Self {
        Self {
            reserves: ReservesInit::new(),
            reserves_2: ReservesInit::new(),
            reserves_3: ReservesInit::new(),
            reserves_4: ReservesInit::new(),
            dex_list: vec![(
                0_u32,
                DEXInfo {
                    base_asset_id: GetBaseAssetId::get(),
                    is_public: true,
                },
            )],
            initial_permission_owners: vec![
                (TRANSFER, Scope::Unlimited, vec![alice()]),
                (MINT, Scope::Unlimited, vec![alice()]),
                (BURN, Scope::Unlimited, vec![alice()]),
                (MANAGE_DEX, Scope::Unlimited, vec![alice()]),
            ],
            initial_permissions: vec![
                (alice(), Scope::Unlimited, vec![MINT, BURN]),
                (alice(), Scope::Limited(hash(&0_u32)), vec![MANAGE_DEX]),
            ],
            source_types: vec![
                LiquiditySourceType::MulticollateralBondingCurvePool,
                LiquiditySourceType::XYKPool,
            ],
            tech_accounts: vec![
                (
                    GetMbcReservesAccountId::get(),
                    GetMbcReservesTechAccountId::get(),
                ),
                (
                    GetMbcRewardsAccountId::get(),
                    GetMbcRewardsTechAccountId::get(),
                ),
            ],
            endowed_assets: vec![
                (
                    common::XOR.into(),
                    alice(),
                    AssetSymbol(b"XOR".to_vec()),
                    AssetName(b"SORA".to_vec()),
                    18,
                    balance!(40000),
                    true,
                ),
                (
                    common::DOT.into(),
                    alice(),
                    AssetSymbol(b"DOT".to_vec()),
                    AssetName(b"DOT".to_vec()),
                    10,
                    balance!(10000),
                    true,
                ),
                (
                    common::VAL.into(),
                    alice(),
                    AssetSymbol(b"VAL".to_vec()),
                    AssetName(b"VAL".to_vec()),
                    18,
                    balance!(10000),
                    true,
                ),
                (
                    common::USDT.into(),
                    alice(),
                    AssetSymbol(b"USDT".to_vec()),
                    AssetName(b"USDT".to_vec()),
                    18,
                    balance!(10000),
                    true,
                ),
                (
                    common::PSWAP.into(),
                    alice(),
                    AssetSymbol(b"PSWAP".to_vec()),
                    AssetName(b"PSWAP".to_vec()),
                    18,
                    balance!(10000),
                    true,
                ),
            ],
        }
    }
}

impl ExtBuilder {
    pub fn build(self) -> sp_io::TestExternalities {
        let mut t = frame_system::GenesisConfig::default()
            .build_storage::<Runtime>()
            .unwrap();

        permissions::GenesisConfig::<Runtime> {
            initial_permission_owners: self.initial_permission_owners,
            initial_permissions: self.initial_permissions,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        dex_manager::GenesisConfig::<Runtime> {
            dex_list: self.dex_list,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        AssetsConfig {
            endowed_assets: self.endowed_assets,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        technical::GenesisConfig::<Runtime> {
            account_ids_to_tech_account_ids: self.tech_accounts,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        <dex_api::GenesisConfig as GenesisBuild<Runtime>>::assimilate_storage(
            &dex_api::GenesisConfig {
                source_types: self.source_types,
            },
            &mut t,
        )
        .unwrap();

        multicollateral_bonding_curve_pool::GenesisConfig::<Runtime> {
            distribution_accounts: bonding_curve_distribution_accounts(),
            reserves_account_id: GetMbcReservesTechAccountId::get(),
            reference_asset_id: USDT.into(),
            incentives_account_id: GetMbcRewardsAccountId::get(),
        }
        .assimilate_storage(&mut t)
        .unwrap();

        t.into()
    }
}
