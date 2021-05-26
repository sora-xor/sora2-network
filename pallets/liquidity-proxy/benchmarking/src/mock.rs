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

#![cfg(test)]

use crate::{Config, *};
use common::mock::ExistentialDeposits;
use common::prelude::Balance;
use common::{
    fixed, fixed_from_basis_points, hash, Amount, AssetId32, BalancePrecision, DEXInfo, Fixed,
    FromGenericPair, LiquiditySourceFilter, LiquiditySourceType, PriceToolsPallet, TechPurpose,
};
use currencies::BasicCurrencyAdapter;

use frame_support::traits::GenesisBuild;
use frame_support::{construct_runtime, parameter_types};
use multicollateral_bonding_curve_pool::{
    DistributionAccount, DistributionAccountData, DistributionAccounts,
};
use permissions::{Scope, BURN, MANAGE_DEX, MINT, TRANSFER};
use sp_core::H256;
use sp_runtime::testing::Header;
use sp_runtime::traits::{BlakeTwo256, IdentityLookup};
use sp_runtime::{AccountId32, DispatchError, DispatchResult};

pub type DEXId = u32;
pub type AssetId = AssetId32<common::PredefinedAssetId>;
pub type TechAssetId = common::TechAssetId<common::PredefinedAssetId>;
pub type AccountId = AccountId32;
pub type BlockNumber = u64;
type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
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
    pub const ExistentialDeposit: u128 = 0;
    pub GetFee: Fixed = fixed_from_basis_points(0u16);
    pub GetPswapDistributionAccountId: AccountId = AccountId32::from([3; 32]);
    pub const GetDefaultSubscriptionFrequency: BlockNumber = 10;
    pub const GetBurnUpdateFrequency: BlockNumber = 10;
    pub GetIncentiveAssetId: AssetId = common::PSWAP.into();
    pub GetParliamentAccountId: AccountId = AccountId32::from([8; 32]);
    pub GetXykFee: Fixed = fixed!(0.003);
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
        VestedRewards: vested_rewards::{Module, Call, Storage, Event<T>},
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

impl dex_manager::Config for Runtime {}

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
    type SwapAction = pool_xyk::PolySwapAction<AssetId, AccountId, TechAccountId>;
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
    type PairSwapAction = pool_xyk::PairSwapAction<AssetId, AccountId, TechAccountId>;
    type DepositLiquidityAction =
        pool_xyk::DepositLiquidityAction<AssetId, AccountId, TechAccountId>;
    type WithdrawLiquidityAction =
        pool_xyk::WithdrawLiquidityAction<AssetId, AccountId, TechAccountId>;
    type PolySwapAction = pool_xyk::PolySwapAction<AssetId, AccountId, TechAccountId>;
    type EnsureDEXManager = dex_manager::Module<Runtime>;
    type GetFee = GetXykFee;
    type PswapDistributionPallet = PswapDistribution;
    type WeightInfo = ();
}

impl vested_rewards::Config for Runtime {
    type Event = Event;
    type WeightInfo = ();
}

fn bonding_curve_distribution_accounts() -> DistributionAccounts<
    DistributionAccountData<
        DistributionAccount<
            <Runtime as frame_system::Config>::AccountId,
            <Runtime as technical::Config>::TechAccountId,
        >,
    >,
> {
    use common::fixed_wrapper;
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
        DistributionAccount::TechAccount(TechAccountId::Pure(
            0u32,
            TechPurpose::Identifier(b"xor_allocation".to_vec()),
        )),
        val_holders_xor_alloc_coeff.get().unwrap(),
    );
    let sora_citizens = DistributionAccountData::new(
        DistributionAccount::TechAccount(TechAccountId::Pure(
            0u32,
            TechPurpose::Identifier(b"sora_citizens".to_vec()),
        )),
        projects_sora_citizens_coeff.get().unwrap(),
    );
    let stores_and_shops = DistributionAccountData::new(
        DistributionAccount::TechAccount(TechAccountId::Pure(
            0u32,
            TechPurpose::Identifier(b"stores_and_shops".to_vec()),
        )),
        projects_stores_and_shops_coeff.get().unwrap(),
    );
    let parliament_and_development = DistributionAccountData::new(
        DistributionAccount::Account(
            hex!("881b87c9f83664b95bd13e2bb40675bfa186287da93becc0b22683334d411e4e").into(),
        ),
        projects_parliament_and_development_coeff.get().unwrap(),
    );
    let projects = DistributionAccountData::new(
        DistributionAccount::TechAccount(TechAccountId::Pure(
            0u32,
            TechPurpose::Identifier(b"projects".to_vec()),
        )),
        projects_other_coeff.get().unwrap(),
    );
    let val_holders = DistributionAccountData::new(
        DistributionAccount::TechAccount(TechAccountId::Pure(
            0u32,
            TechPurpose::Identifier(b"val_holders".to_vec()),
        )),
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
    pub GetMbcFreeReservesTechAccountId: TechAccountId = {
        let tech_account_id = TechAccountId::from_generic_pair(
            multicollateral_bonding_curve_pool::TECH_ACCOUNT_PREFIX.to_vec(),
            multicollateral_bonding_curve_pool::TECH_ACCOUNT_FREE_RESERVES.to_vec(),
        );
        tech_account_id
    };
    pub GetMbcFreeReservesAccountId: AccountId = {
        let tech_account_id = GetMbcFreeReservesTechAccountId::get();
        let account_id =
            technical::Module::<Runtime>::tech_account_id_to_account_id(&tech_account_id)
                .expect("Failed to get ordinary account id for technical account id.");
        account_id
    };
}

pub struct MockPriceTools;

impl PriceToolsPallet<AssetId> for MockPriceTools {
    fn get_average_price(
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
    ) -> Result<Balance, DispatchError> {
        let res = <LiquidityProxy as LiquidityProxyTrait<DEXId, AccountId, AssetId>>::quote(
            input_asset_id,
            output_asset_id,
            SwapAmount::with_desired_input(balance!(1), balance!(0)),
            LiquiditySourceFilter::with_allowed(
                0u32,
                [LiquiditySourceType::XYKPool].iter().cloned().collect(),
            ),
        );
        Ok(res?.amount)
    }

    fn register_asset(_: &AssetId) -> DispatchResult {
        // do nothing
        Ok(())
    }
}

impl multicollateral_bonding_curve_pool::Config for Runtime {
    type Event = Event;
    type LiquidityProxy = liquidity_proxy::Module<Runtime>;
    type EnsureDEXManager = dex_manager::Module<Runtime>;
    type EnsureTradingPairExists = trading_pair::Module<Runtime>;
    type PriceToolsPallet = MockPriceTools;
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
    type WeightInfo = ();
    type GetParliamentAccountId = GetParliamentAccountId;
    type PoolXykPallet = PoolXyk;
}

impl Config for Runtime {}

pub struct ExtBuilder {
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
                (
                    GetMbcReservesAccountId::get(),
                    Scope::Unlimited,
                    vec![MINT, BURN],
                ),
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
                (
                    GetLiquidityProxyAccountId::get(),
                    GetLiquidityProxyTechAccountId::get(),
                ),
            ],
            endowed_assets: vec![
                (
                    common::XOR.into(),
                    alice(),
                    AssetSymbol(b"XOR".to_vec()),
                    AssetName(b"SORA".to_vec()),
                    18,
                    balance!(350000),
                    true,
                ),
                (
                    common::DOT.into(),
                    alice(),
                    AssetSymbol(b"DOT".to_vec()),
                    AssetName(b"DOT".to_vec()),
                    10,
                    balance!(0),
                    true,
                ),
                (
                    common::VAL.into(),
                    alice(),
                    AssetSymbol(b"VAL".to_vec()),
                    AssetName(b"VAL".to_vec()),
                    18,
                    balance!(0),
                    true,
                ),
                (
                    common::USDT.into(),
                    alice(),
                    AssetSymbol(b"USDT".to_vec()),
                    AssetName(b"USDT".to_vec()),
                    18,
                    balance!(0),
                    true,
                ),
                (
                    common::PSWAP.into(),
                    alice(),
                    AssetSymbol(b"PSWAP".to_vec()),
                    AssetName(b"PSWAP".to_vec()),
                    18,
                    balance!(0),
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

        let accounts = bonding_curve_distribution_accounts();
        let mut tech_accounts = self.tech_accounts.clone();
        for account in &accounts.accounts() {
            match account {
                DistributionAccount::Account(_) => continue,
                DistributionAccount::TechAccount(account_id) => {
                    tech_accounts.push((
                        Technical::tech_account_id_to_account_id(&account_id).unwrap(),
                        account_id.clone(),
                    ));
                }
            }
        }

        pallet_balances::GenesisConfig::<Runtime> {
            balances: vec![
                (alice(), 0),
                (
                    if let DistributionAccount::TechAccount(account_id) =
                        &accounts.val_holders.account
                    {
                        Technical::tech_account_id_to_account_id(account_id).unwrap()
                    } else {
                        panic!("not a tech account")
                    },
                    0,
                ),
                (GetMbcReservesAccountId::get(), 0),
                (GetMbcRewardsAccountId::get(), 0),
                (GetLiquidityProxyAccountId::get(), 0),
            ],
        }
        .assimilate_storage(&mut t)
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
            account_ids_to_tech_account_ids: tech_accounts,
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
            distribution_accounts: accounts,
            reserves_account_id: GetMbcReservesTechAccountId::get(),
            reference_asset_id: USDT.into(),
            incentives_account_id: GetMbcRewardsAccountId::get(),
            initial_collateral_assets: Default::default(),
            free_reserves_account_id: GetMbcFreeReservesAccountId::get(),
        }
        .assimilate_storage(&mut t)
        .unwrap();

        t.into()
    }
}
