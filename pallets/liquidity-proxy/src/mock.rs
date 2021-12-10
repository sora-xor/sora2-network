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

use crate::{self as liquidity_proxy, Config};
use common::mock::ExistentialDeposits;
use common::{
    self, balance, fixed, fixed_from_basis_points, fixed_wrapper, hash, Amount, AssetId32,
    AssetName, AssetSymbol, DEXInfo, Fixed, FromGenericPair, GetMarketInfo, LiquiditySource,
    LiquiditySourceType, RewardReason, DAI, DEFAULT_BALANCE_PRECISION, DOT, ETH, KSM, PSWAP, USDT,
    VAL, XOR, XSTUSD,
};
use currencies::BasicCurrencyAdapter;

use frame_support::traits::GenesisBuild;
use frame_support::weights::Weight;
use frame_support::{construct_runtime, ensure, fail, parameter_types};
use frame_system;
use traits::MultiCurrency;

use common::prelude::{Balance, FixedWrapper, QuoteAmount, SwapAmount, SwapOutcome};
use permissions::{Scope, INIT_DEX, MANAGE_DEX};
use sp_core::H256;
use sp_runtime::testing::Header;
use sp_runtime::traits::{BlakeTwo256, IdentityLookup};
use sp_runtime::{AccountId32, DispatchError, Perbill};
use sp_std::str::FromStr;
use std::collections::{BTreeSet, HashMap};

pub type AccountId = AccountId32;
pub type BlockNumber = u64;
pub type DEXId = u32;
type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
type TechAssetId = common::TechAssetId<common::PredefinedAssetId>;
pub type AssetId = AssetId32<common::PredefinedAssetId>;
type ReservesInit = Vec<(DEXId, AssetId, (Fixed, Fixed))>;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

pub fn alice() -> AccountId {
    AccountId32::from([1u8; 32])
}

pub const DEX_A_ID: DEXId = 1;
pub const DEX_B_ID: DEXId = 2;
pub const DEX_C_ID: DEXId = 3;
pub const DEX_D_ID: DEXId = 0;

pub fn special_asset() -> AssetId {
    AssetId::from_str("0x02ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff").unwrap()
}

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = 1024;
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    pub GetLiquidityProxyTechAccountId: TechAccountId = {
        let tech_account_id = TechAccountId::from_generic_pair(
            crate::TECH_ACCOUNT_PREFIX.to_vec(),
            crate::TECH_ACCOUNT_MAIN.to_vec(),
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
    pub const GetNumSamples: usize = 40;
    pub const GetBaseAssetId: AssetId = XOR;
    pub const ExistentialDeposit: u128 = 0;
    pub GetFee0: Fixed = fixed_from_basis_points(0u16);
    pub GetFee10: Fixed = fixed_from_basis_points(10u16);
    pub GetFee20: Fixed = fixed_from_basis_points(20u16);
    pub GetFee30: Fixed = fixed_from_basis_points(30u16);
    pub GetIncentiveAssetId: AssetId = common::PSWAP.into();
    pub GetPswapDistributionAccountId: AccountId = AccountId32::from([151; 32]);
    pub const GetDefaultSubscriptionFrequency: BlockNumber = 10;
    pub const GetBurnUpdateFrequency: BlockNumber = 14400;
    pub GetParliamentAccountId: AccountId = AccountId32::from([152; 32]);
    pub GetMarketMakerRewardsAccountId: AccountId = AccountId32::from([9; 32]);
    pub GetBondingCurveRewardsAccountId: AccountId = AccountId32::from([10; 32]);
    pub GetTeamReservesAccountId: AccountId = AccountId32::from([11; 32]);
    pub GetFarmingRewardsAccountId: AccountId = AccountId32::from([12; 32]);
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
        DexManager: dex_manager::{Module, Call, Storage},
        MockLiquiditySource: mock_liquidity_source::<Instance1>::{Module, Call, Config<T>, Storage},
        MockLiquiditySource2: mock_liquidity_source::<Instance2>::{Module, Call, Config<T>, Storage},
        MockLiquiditySource3: mock_liquidity_source::<Instance3>::{Module, Call, Config<T>, Storage},
        MockLiquiditySource4: mock_liquidity_source::<Instance4>::{Module, Call, Config<T>, Storage},
        Technical: technical::{Module, Call, Storage, Event<T>},
        Permissions: permissions::{Module, Call, Config<T>, Storage, Event<T>},
        DexApi: dex_api::{Module, Call, Config, Storage, Event<T>},
        TradingPair: trading_pair::{Module, Call, Storage, Event<T>},
        VestedRewards: vested_rewards::{Module, Call, Storage, Event<T>},
        PoolXyk: pool_xyk::{Module, Call, Storage, Event<T>},
        PswapDistribution: pswap_distribution::{Module, Call, Storage, Event<T>},
        MBCPool: multicollateral_bonding_curve_pool::{Module, Call, Storage, Event<T>},
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

impl Config for Runtime {
    type Event = Event;
    type LiquidityRegistry = dex_api::Module<Runtime>;
    type GetNumSamples = GetNumSamples;
    type GetTechnicalAccountId = GetLiquidityProxyAccountId;
    type WeightInfo = ();
    type PrimaryMarketTBC = MockMCBCPool;
    type PrimaryMarketXST = MockXSTPool;
    type SecondaryMarket = mock_liquidity_source::Module<Runtime, mock_liquidity_source::Instance1>;
    type VestedRewardsPallet = vested_rewards::Module<Runtime>;
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
    type GetTeamReservesAccountId = GetTeamReservesAccountId;
    type GetTotalBalance = ();
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
    type GetFee = GetFee0;
    type EnsureDEXManager = dex_manager::Module<Runtime>;
    type EnsureTradingPairExists = trading_pair::Module<Runtime>;
}

impl mock_liquidity_source::Config<mock_liquidity_source::Instance2> for Runtime {
    type GetFee = GetFee10;
    type EnsureDEXManager = dex_manager::Module<Runtime>;
    type EnsureTradingPairExists = trading_pair::Module<Runtime>;
}

impl mock_liquidity_source::Config<mock_liquidity_source::Instance3> for Runtime {
    type GetFee = GetFee20;
    type EnsureDEXManager = dex_manager::Module<Runtime>;
    type EnsureTradingPairExists = trading_pair::Module<Runtime>;
}

impl mock_liquidity_source::Config<mock_liquidity_source::Instance4> for Runtime {
    type GetFee = GetFee30;
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
}

impl permissions::Config for Runtime {
    type Event = Event;
}

impl dex_api::Config for Runtime {
    type Event = Event;
    type MockLiquiditySource =
        mock_liquidity_source::Module<Runtime, mock_liquidity_source::Instance1>;
    type MockLiquiditySource2 =
        mock_liquidity_source::Module<Runtime, mock_liquidity_source::Instance2>;
    type MockLiquiditySource3 =
        mock_liquidity_source::Module<Runtime, mock_liquidity_source::Instance3>;
    type MockLiquiditySource4 =
        mock_liquidity_source::Module<Runtime, mock_liquidity_source::Instance4>;
    type XYKPool = ();
    type MulticollateralBondingCurvePool = MockMCBCPool;
    type XSTPool = MockXSTPool;
    type WeightInfo = ();
}

impl trading_pair::Config for Runtime {
    type Event = Event;
    type EnsureDEXManager = dex_manager::Module<Runtime>;
    type WeightInfo = ();
}

impl pswap_distribution::Config for Runtime {
    type Event = Event;
    type GetIncentiveAssetId = GetIncentiveAssetId;
    type LiquidityProxy = ();
    type CompatBalance = Balance;
    type GetDefaultSubscriptionFrequency = GetDefaultSubscriptionFrequency;
    type GetBurnUpdateFrequency = GetBurnUpdateFrequency;
    type GetTechnicalAccountId = GetPswapDistributionAccountId;
    type EnsureDEXManager = ();
    type OnPswapBurnedAggregator = ();
    type PoolXykPallet = pool_xyk::Module<Runtime>;
    type WeightInfo = ();
    type GetParliamentAccountId = GetParliamentAccountId;
}

impl pool_xyk::Config for Runtime {
    const MIN_XOR: Balance = balance!(0.007);
    type Event = Event;
    type PairSwapAction = pool_xyk::PairSwapAction<AssetId, AccountId, TechAccountId>;
    type DepositLiquidityAction =
        pool_xyk::DepositLiquidityAction<AssetId, AccountId, TechAccountId>;
    type WithdrawLiquidityAction =
        pool_xyk::WithdrawLiquidityAction<AssetId, AccountId, TechAccountId>;
    type PolySwapAction = pool_xyk::PolySwapAction<AssetId, AccountId, TechAccountId>;
    type EnsureDEXManager = dex_manager::Module<Runtime>;
    type OnPoolCreated = pswap_distribution::Module<Runtime>;
    type OnPoolReservesChanged = ();
    type GetFee = GetXykFee;
    type WeightInfo = ();
}

impl multicollateral_bonding_curve_pool::Config for Runtime {
    type Event = Event;
    type LiquidityProxy = ();
    type EnsureTradingPairExists = trading_pair::Module<Runtime>;
    type EnsureDEXManager = dex_manager::Module<Runtime>;
    type VestedRewardsPallet = VestedRewards;
    type PriceToolsPallet = ();
    type WeightInfo = ();
}

impl vested_rewards::Config for Runtime {
    type Event = Event;
    type GetMarketMakerRewardsAccountId = GetMarketMakerRewardsAccountId;
    type GetBondingCurveRewardsAccountId = GetBondingCurveRewardsAccountId;
    type GetFarmingRewardsAccountId = GetFarmingRewardsAccountId;
    type WeightInfo = ();
}

pub struct ExtBuilder {
    pub total_supply: Balance,
    pub reserves: ReservesInit,
    pub reserves_2: ReservesInit,
    pub reserves_3: ReservesInit,
    pub reserves_4: ReservesInit,
    pub dex_list: Vec<(DEXId, DEXInfo<AssetId>)>,
    pub initial_permission_owners: Vec<(u32, Scope, Vec<AccountId>)>,
    pub initial_permissions: Vec<(AccountId, Scope, Vec<u32>)>,
    pub source_types: Vec<LiquiditySourceType>,
    pub endowed_accounts: Vec<(AccountId, AssetId, Balance, AssetSymbol, AssetName, u8)>,
}

impl Default for ExtBuilder {
    fn default() -> Self {
        Self {
            total_supply: balance!(360000),
            reserves: vec![
                (DEX_A_ID, DOT, (fixed!(5000), fixed!(7000))),
                (DEX_A_ID, KSM, (fixed!(5500), fixed!(4000))),
                (DEX_B_ID, DOT, (fixed!(100), fixed!(45))),
                (DEX_C_ID, DOT, (fixed!(520), fixed!(550))),
                (DEX_D_ID, VAL, (fixed!(1000), fixed!(200000))),
                (DEX_D_ID, KSM, (fixed!(1000), fixed!(1000))),
                (DEX_D_ID, DOT, (fixed!(1000), fixed!(9000))),
            ],
            reserves_2: vec![
                (DEX_A_ID, DOT, (fixed!(6000), fixed!(6000))),
                (DEX_A_ID, KSM, (fixed!(6500), fixed!(3000))),
                (DEX_B_ID, DOT, (fixed!(200), fixed!(45))),
                (DEX_C_ID, DOT, (fixed!(550), fixed!(700))),
            ],
            reserves_3: vec![
                (DEX_A_ID, DOT, (fixed!(7000), fixed!(5000))),
                (DEX_A_ID, KSM, (fixed!(7500), fixed!(2000))),
                (DEX_B_ID, DOT, (fixed!(300), fixed!(45))),
                (DEX_C_ID, DOT, (fixed!(400), fixed!(380))),
            ],
            reserves_4: vec![
                (DEX_A_ID, DOT, (fixed!(8000), fixed!(4000))),
                (DEX_A_ID, KSM, (fixed!(8500), fixed!(1000))),
                (DEX_B_ID, DOT, (fixed!(400), fixed!(45))),
                (DEX_C_ID, DOT, (fixed!(1300), fixed!(1800))),
            ],
            dex_list: vec![
                (
                    DEX_A_ID,
                    DEXInfo {
                        base_asset_id: GetBaseAssetId::get(),
                        is_public: true,
                    },
                ),
                (
                    DEX_B_ID,
                    DEXInfo {
                        base_asset_id: GetBaseAssetId::get(),
                        is_public: true,
                    },
                ),
                (
                    DEX_C_ID,
                    DEXInfo {
                        base_asset_id: GetBaseAssetId::get(),
                        is_public: true,
                    },
                ),
                (
                    DEX_D_ID,
                    DEXInfo {
                        base_asset_id: GetBaseAssetId::get(),
                        is_public: true,
                    },
                ),
            ],
            initial_permission_owners: vec![
                (INIT_DEX, Scope::Unlimited, vec![alice()]),
                (MANAGE_DEX, Scope::Limited(hash(&DEX_A_ID)), vec![alice()]),
                (MANAGE_DEX, Scope::Limited(hash(&DEX_B_ID)), vec![alice()]),
            ],
            initial_permissions: vec![
                (alice(), Scope::Unlimited, vec![INIT_DEX]),
                (alice(), Scope::Limited(hash(&DEX_A_ID)), vec![MANAGE_DEX]),
                (alice(), Scope::Limited(hash(&DEX_B_ID)), vec![MANAGE_DEX]),
            ],
            source_types: vec![
                LiquiditySourceType::MulticollateralBondingCurvePool,
                LiquiditySourceType::MockPool,
                LiquiditySourceType::MockPool2,
                LiquiditySourceType::MockPool3,
                LiquiditySourceType::MockPool4,
            ],
            endowed_accounts: vec![
                (
                    alice(),
                    XOR,
                    balance!(0),
                    AssetSymbol(b"XOR".to_vec()),
                    AssetName(b"SORA".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                ),
                (
                    alice(),
                    VAL,
                    balance!(0),
                    AssetSymbol(b"VAL".to_vec()),
                    AssetName(b"SORA Validator Token".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                ),
                (
                    alice(),
                    PSWAP,
                    balance!(0),
                    AssetSymbol(b"PSWAP".to_vec()),
                    AssetName(b"Polkaswap Token".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                ),
                (
                    alice(),
                    USDT,
                    balance!(0),
                    AssetSymbol(b"USDT".to_vec()),
                    AssetName(b"Tether".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                ),
            ],
        }
    }
}

pub struct MockMCBCPool;

impl MockMCBCPool {
    pub fn init(reserves: Vec<(AssetId, Balance)>) -> Result<(), DispatchError> {
        let reserves_tech_account_id =
            TechAccountId::Generic(b"mcbc_pool".to_vec(), b"main".to_vec());
        let reserves_account_id =
            Technical::tech_account_id_to_account_id(&reserves_tech_account_id)?;
        Technical::register_tech_account_id(reserves_tech_account_id.clone())?;
        MockLiquiditySource::set_reserves_account_id(reserves_tech_account_id)?;
        for r in reserves {
            Currencies::deposit(r.0, &reserves_account_id, r.1)?;
        }
        Ok(())
    }

    fn _spot_price(collateral_asset: &AssetId) -> Fixed {
        let total_supply = pallet_balances::Module::<Runtime>::total_issuance();
        Self::_price_at(collateral_asset, total_supply)
    }

    fn _price_at(collateral_asset: &AssetId, base_supply: Balance) -> Fixed {
        if *collateral_asset == GetBaseAssetId::get() {
            return fixed!(1.0);
        }
        let initial_price = get_initial_price();
        let x: FixedWrapper = base_supply.into();
        let b: FixedWrapper = initial_price.into();
        let m: FixedWrapper = fixed_wrapper!(1) / fixed_wrapper!(1337);

        let base_price_wrt_ref: FixedWrapper = m * x + b;

        let collateral_price_per_reference_unit: FixedWrapper =
            get_reference_prices()[collateral_asset].into();
        (base_price_wrt_ref / collateral_price_per_reference_unit)
            .get()
            .unwrap()
    }
}

impl LiquiditySource<DEXId, AccountId, AssetId, Balance, DispatchError> for MockMCBCPool {
    fn can_exchange(_dex_id: &DEXId, _input_asset_id: &AssetId, output_asset_id: &AssetId) -> bool {
        if output_asset_id == &XOR.into() {
            return true;
        }
        let reserves_tech_account_id =
            TechAccountId::Generic(b"mcbc_pool".to_vec(), b"main".to_vec());
        let reserves_account_id =
            Technical::tech_account_id_to_account_id(&reserves_tech_account_id).unwrap();
        let free_balance = Currencies::free_balance(*output_asset_id, &reserves_account_id);
        free_balance > 0
    }

    fn quote(
        dex_id: &DEXId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        amount: QuoteAmount<Balance>,
        deduce_fee: bool,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        if !Self::can_exchange(dex_id, input_asset_id, output_asset_id) {
            panic!("Can't exchange");
        }
        let base_asset_id = &GetBaseAssetId::get();
        let reserves_tech_account_id =
            TechAccountId::Generic(b"mcbc_pool".to_vec(), b"main".to_vec());
        let reserves_account_id =
            Technical::tech_account_id_to_account_id(&reserves_tech_account_id)?;
        let current_supply = pallet_balances::Module::<Runtime>::total_issuance();

        let (input_amount, output_amount, fee_amount) = if input_asset_id == base_asset_id {
            // Selling XOR
            let collateral_reserves: FixedWrapper =
                Currencies::free_balance(*output_asset_id, &reserves_account_id).into();
            let buy_spot_price: FixedWrapper = Self::_spot_price(output_asset_id).into();
            let sell_spot_price: FixedWrapper = buy_spot_price.clone() * fixed_wrapper!(0.8);
            let pretended_base_reserves = collateral_reserves.clone() / sell_spot_price.clone();

            let ideal_reserves: FixedWrapper = (buy_spot_price
                + get_initial_price()
                    / FixedWrapper::from(get_reference_prices()[output_asset_id]))
                * fixed_wrapper!(0.4)
                * FixedWrapper::from(current_supply);
            let collateralization = (collateral_reserves.clone() / ideal_reserves)
                .get()
                .unwrap();

            let extra_fee = if deduce_fee {
                FixedWrapper::from(undercollaterization_charge(collateralization))
            } else {
                0.into()
            };

            match amount {
                QuoteAmount::WithDesiredInput {
                    desired_amount_in, ..
                } => {
                    let input_wrapped: FixedWrapper = desired_amount_in.into();
                    let input_after_fee: FixedWrapper =
                        input_wrapped * (fixed_wrapper!(1) - extra_fee.clone());
                    let output_collateral = (input_after_fee.clone() * collateral_reserves)
                        / (pretended_base_reserves + input_after_fee);
                    let output_amount: Balance = output_collateral.try_into_balance().unwrap();

                    (desired_amount_in, output_amount, 0)
                }
                QuoteAmount::WithDesiredOutput {
                    desired_amount_out, ..
                } => {
                    let output_wrapped: FixedWrapper = desired_amount_out.into();
                    ensure!(
                        output_wrapped < collateral_reserves,
                        crate::Error::<Runtime>::InsufficientLiquidity
                    );
                    let input_base = (pretended_base_reserves * output_wrapped.clone())
                        / (collateral_reserves - output_wrapped);

                    let input_base_after_fee = input_base / (fixed_wrapper!(1) - extra_fee);

                    let input_amount: Balance = input_base_after_fee.try_into_balance().unwrap();
                    (input_amount, desired_amount_out, 0)
                }
            }
        } else {
            // Buying XOR
            let buy_spot_price: FixedWrapper = Self::_spot_price(input_asset_id).into();
            let m: FixedWrapper = fixed_wrapper!(1) / fixed_wrapper!(1337);

            match amount {
                QuoteAmount::WithDesiredInput {
                    desired_amount_in: collateral_quantity,
                    ..
                } => {
                    let under_pow = buy_spot_price.clone() / m.clone() * fixed_wrapper!(2.0);
                    let under_sqrt = under_pow.clone() * under_pow
                        + fixed_wrapper!(8.0) * collateral_quantity / m.clone();
                    let base_output =
                        under_sqrt.sqrt_accurate() / fixed_wrapper!(2.0) - buy_spot_price / m;
                    let output_amount: Balance = base_output.try_into_balance().unwrap();
                    (collateral_quantity, output_amount, 0)
                }
                QuoteAmount::WithDesiredOutput {
                    desired_amount_out: base_quantity,
                    ..
                } => {
                    let projected_supply: Balance = current_supply + base_quantity;
                    let new_buy_price: FixedWrapper =
                        Self::_price_at(input_asset_id, projected_supply).into();
                    let collateral_input =
                        ((buy_spot_price + new_buy_price) / fixed_wrapper!(2.0)) * base_quantity;
                    let input_amount: Balance = collateral_input.try_into_balance().unwrap();

                    (input_amount, base_quantity, 0)
                }
            }
        };
        match amount {
            QuoteAmount::WithDesiredInput { .. } => Ok(SwapOutcome::new(output_amount, fee_amount)),
            QuoteAmount::WithDesiredOutput { .. } => Ok(SwapOutcome::new(input_amount, fee_amount)),
        }
    }

    fn exchange(
        _sender: &AccountId,
        _receiver: &AccountId,
        _dex_id: &DEXId,
        _input_asset_id: &AssetId,
        _output_asset_id: &AssetId,
        _desired_amount: SwapAmount<Balance>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        unimplemented!()
    }

    fn check_rewards(
        _dex_id: &DEXId,
        _input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        _input_amount: Balance,
        output_amount: Balance,
    ) -> Result<Vec<(Balance, AssetId, RewardReason)>, DispatchError> {
        // for mock just return like in input
        if output_asset_id == &GetBaseAssetId::get() {
            Ok(vec![(
                output_amount,
                output_asset_id.clone(),
                RewardReason::BuyOnBondingCurve,
            )])
        } else {
            fail!(crate::Error::<Runtime>::UnavailableExchangePath);
        }
    }

    fn quote_without_impact(
        dex_id: &DEXId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        amount: QuoteAmount<Balance>,
        deduce_fee: bool,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        // TODO: implement if needed
        Self::quote(dex_id, input_asset_id, output_asset_id, amount, deduce_fee)
    }
}

impl GetMarketInfo<AssetId> for MockMCBCPool {
    fn buy_price(
        _base_asset: &AssetId,
        collateral_asset: &AssetId,
    ) -> Result<Fixed, DispatchError> {
        if collateral_asset == &special_asset() {
            fail!(crate::Error::<Runtime>::CalculationError);
        }
        Ok(Self::_spot_price(collateral_asset))
    }

    fn sell_price(
        base_asset: &AssetId,
        collateral_asset: &AssetId,
    ) -> Result<Fixed, DispatchError> {
        let buy_price = Self::buy_price(base_asset, collateral_asset)?;
        let buy_price: FixedWrapper = FixedWrapper::from(buy_price);
        let output = (buy_price * fixed_wrapper!(0.8)).get().unwrap();
        Ok(output)
    }

    fn enabled_target_assets() -> BTreeSet<AssetId> {
        [VAL, PSWAP, DAI, ETH].iter().cloned().collect()
    }
}

pub fn get_reference_prices() -> HashMap<AssetId, Balance> {
    let prices = vec![
        (VAL, balance!(2.0)),
        (PSWAP, balance!(0.098)),
        (USDT, balance!(1.01)),
        (KSM, balance!(450.0)),
        (DOT, balance!(50.0)),
        (XSTUSD, balance!(1.02)),
    ];
    prices.into_iter().collect()
}

pub fn get_mcbc_reserves_normal() -> Vec<(AssetId, Balance)> {
    vec![
        (VAL, balance!(100000)),
        (DOT, balance!(100000)),
        (KSM, balance!(100000)),
    ]
}

pub fn get_mcbc_reserves_undercollateralized() -> Vec<(AssetId, Balance)> {
    vec![
        (VAL, balance!(5000)),
        (DOT, balance!(200)),
        (KSM, balance!(100)),
    ]
}

pub fn get_initial_price() -> Fixed {
    fixed!(200)
}

fn undercollaterization_charge(fraction: Fixed) -> Fixed {
    if fraction < fixed!(0.05) {
        fixed!(0.09)
    } else if fraction < fixed!(0.1) {
        fixed!(0.06)
    } else if fraction < fixed!(0.2) {
        fixed!(0.03)
    } else if fraction < fixed!(0.3) {
        fixed!(0.01)
    } else {
        fixed!(0)
    }
}

impl ExtBuilder {
    pub fn with_enabled_sources(sources: Vec<LiquiditySourceType>) -> Self {
        Self {
            source_types: sources,
            ..Default::default()
        }
    }

    pub fn with_total_supply_and_reserves(
        base_total_supply: Balance,
        xyk_reserves: ReservesInit,
    ) -> Self {
        Self {
            total_supply: base_total_supply,
            reserves: xyk_reserves,
            dex_list: vec![(
                0,
                DEXInfo {
                    base_asset_id: GetBaseAssetId::get(),
                    is_public: true,
                },
            )],
            ..Default::default()
        }
    }

    pub fn build(self) -> sp_io::TestExternalities {
        let mut t = frame_system::GenesisConfig::default()
            .build_storage::<Runtime>()
            .unwrap();

        pallet_balances::GenesisConfig::<Runtime> {
            balances: vec![(alice(), self.total_supply)],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        dex_manager::GenesisConfig::<Runtime> {
            dex_list: self.dex_list,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        permissions::GenesisConfig::<Runtime> {
            initial_permission_owners: self.initial_permission_owners,
            initial_permissions: self.initial_permissions,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        mock_liquidity_source::GenesisConfig::<Runtime, mock_liquidity_source::Instance1> {
            reserves: self.reserves,
            phantom: Default::default(),
        }
        .assimilate_storage(&mut t)
        .unwrap();

        mock_liquidity_source::GenesisConfig::<Runtime, mock_liquidity_source::Instance2> {
            reserves: self.reserves_2,
            phantom: Default::default(),
        }
        .assimilate_storage(&mut t)
        .unwrap();

        mock_liquidity_source::GenesisConfig::<Runtime, mock_liquidity_source::Instance3> {
            reserves: self.reserves_3,
            phantom: Default::default(),
        }
        .assimilate_storage(&mut t)
        .unwrap();

        mock_liquidity_source::GenesisConfig::<Runtime, mock_liquidity_source::Instance4> {
            reserves: self.reserves_4,
            phantom: Default::default(),
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

        assets::GenesisConfig::<Runtime> {
            endowed_assets: self
                .endowed_accounts
                .iter()
                .cloned()
                .map(|(account_id, asset_id, _, symbol, name, precision)| {
                    (
                        asset_id,
                        account_id,
                        symbol,
                        name,
                        precision,
                        balance!(0),
                        true,
                        None,
                        None,
                    )
                })
                .collect(),
        }
        .assimilate_storage(&mut t)
        .unwrap();

        t.into()
    }
}

pub struct MockXSTPool;

#[allow(unused)]
impl MockXSTPool {
    pub fn init() -> Result<(), DispatchError> {
        let reserves_tech_account_id =
            TechAccountId::Generic(b"xst_pool".to_vec(), b"main".to_vec());
        let _reserves_account_id =
            Technical::tech_account_id_to_account_id(&reserves_tech_account_id)?;
        Technical::register_tech_account_id(reserves_tech_account_id.clone())?;
        MockLiquiditySource::set_reserves_account_id(reserves_tech_account_id)?;
        Ok(())
    }
}

impl LiquiditySource<DEXId, AccountId, AssetId, Balance, DispatchError> for MockXSTPool {
    fn can_exchange(_dex_id: &DEXId, input_asset_id: &AssetId, output_asset_id: &AssetId) -> bool {
        if output_asset_id == &XOR.into() && input_asset_id == &XSTUSD.into() {
            return true;
        } else if input_asset_id == &XOR.into() && output_asset_id == &XSTUSD.into() {
            return true;
        } else {
            return false;
        }
    }

    fn quote(
        dex_id: &DEXId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        amount: QuoteAmount<Balance>,
        _deduce_fee: bool,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        if !Self::can_exchange(dex_id, input_asset_id, output_asset_id) {
            panic!("Can't exchange");
        }
        let base_asset_id = &GetBaseAssetId::get();
        let reserves_tech_account_id =
            TechAccountId::Generic(b"xst_pool".to_vec(), b"main".to_vec());
        let _reserves_account_id =
            Technical::tech_account_id_to_account_id(&reserves_tech_account_id)?;

        let base_asset_price: Balance = get_reference_prices()[base_asset_id].into();

        let (input_amount, output_amount, fee_amount) = if input_asset_id == base_asset_id {
            // Selling XOR

            match amount {
                QuoteAmount::WithDesiredInput {
                    desired_amount_in, ..
                } => {
                    let output_amount: Balance = desired_amount_in * base_asset_price;
                    (desired_amount_in, output_amount, 0)
                }
                QuoteAmount::WithDesiredOutput {
                    desired_amount_out, ..
                } => {
                    let input_amount = desired_amount_out / base_asset_price;
                    (input_amount, desired_amount_out, 0)
                }
            }
        } else {
            // Buying XOR
            match amount {
                QuoteAmount::WithDesiredInput {
                    desired_amount_in: synthetics_quantity,
                    ..
                } => {
                    //TODO: here we assume only DAI-pegged XST(USD) synthetics. Need to have a price oracle to handle other synthetics in the future!
                    let output_amount = synthetics_quantity / base_asset_price;
                    (synthetics_quantity, output_amount, 0)
                }
                QuoteAmount::WithDesiredOutput {
                    desired_amount_out: base_quantity,
                    ..
                } => {
                    //TODO: here we assume only DAI-pegged XST(USD) synthetics. Need to have a price oracle to handle other synthetics in the future!
                    let input_amount = base_quantity * base_asset_price;

                    (input_amount, base_quantity, 0)
                }
            }
        };
        match amount {
            QuoteAmount::WithDesiredInput { .. } => Ok(SwapOutcome::new(output_amount, fee_amount)),
            QuoteAmount::WithDesiredOutput { .. } => Ok(SwapOutcome::new(input_amount, fee_amount)),
        }
    }

    fn exchange(
        _sender: &AccountId,
        _receiver: &AccountId,
        _dex_id: &DEXId,
        _input_asset_id: &AssetId,
        _output_asset_id: &AssetId,
        _desired_amount: SwapAmount<Balance>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        unimplemented!()
    }

    fn check_rewards(
        _dex_id: &DEXId,
        _input_asset_id: &AssetId,
        _output_asset_id: &AssetId,
        _input_amount: Balance,
        _output_amount: Balance,
    ) -> Result<Vec<(Balance, AssetId, RewardReason)>, DispatchError> {
        Ok(Vec::new()) // no rewards for XST
    }

    fn quote_without_impact(
        dex_id: &DEXId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        amount: QuoteAmount<Balance>,
        deduce_fee: bool,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        // TODO: implement if needed
        Self::quote(dex_id, input_asset_id, output_asset_id, amount, deduce_fee)
    }
}

impl GetMarketInfo<AssetId> for MockXSTPool {
    fn buy_price(
        _base_asset_id: &AssetId,
        synthetic_asset: &AssetId,
    ) -> Result<Fixed, DispatchError> {
        let synthetic_asset_price: FixedWrapper = get_reference_prices()[synthetic_asset].into();
        let output = synthetic_asset_price
            .get()
            .map_err(|_| crate::Error::<Runtime>::CalculationError)?;
        Ok(output)
    }

    fn sell_price(
        _base_asset: &AssetId,
        synthetic_asset: &AssetId,
    ) -> Result<Fixed, DispatchError> {
        let synthetic_asset_price: FixedWrapper = get_reference_prices()[synthetic_asset].into();
        let output = synthetic_asset_price
            .get()
            .map_err(|_| crate::Error::<Runtime>::CalculationError)?;
        Ok(output)
    }

    /// `target_assets` refer to synthetic assets
    fn enabled_target_assets() -> BTreeSet<AssetId> {
        [XSTUSD].iter().cloned().collect()
    }
}
