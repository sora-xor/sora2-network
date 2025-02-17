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

use crate::{self as liquidity_proxy, LiquidityProxyBuyBackHandler};
use common::alt::{DiscreteQuotation, SwapChunk};
use common::mock::ExistentialDeposits;
use common::{
    self, balance, fixed, fixed_from_basis_points, fixed_wrapper, hash, mock_assets_config,
    mock_ceres_liquidity_locker_config, mock_common_config, mock_currencies_config,
    mock_demeter_farming_platform_config, mock_dex_api_config, mock_dex_manager_config,
    mock_extended_assets_config, mock_frame_system_config, mock_liquidity_proxy_config,
    mock_liquidity_source_config, mock_multicollateral_bonding_curve_pool_config,
    mock_pallet_balances_config, mock_pallet_timestamp_config, mock_permissions_config,
    mock_pool_xyk_config, mock_pswap_distribution_config, mock_technical_config,
    mock_tokens_config, mock_trading_pair_config, mock_vested_rewards_config, Amount, AssetId32,
    AssetName, AssetSymbol, DEXInfo, Fixed, FromGenericPair, GetMarketInfo, LiquiditySource,
    LiquiditySourceType, RewardReason, DAI, DEFAULT_BALANCE_PRECISION, DOT, ETH, KSM, PSWAP, USDT,
    VAL, VXOR, XOR, XST, XSTUSD,
};
use currencies::BasicCurrencyAdapter;

use frame_support::traits::{ConstU32, GenesisBuild};
use frame_support::weights::Weight;
use frame_support::{construct_runtime, ensure, fail, parameter_types};
use frame_system;
use traits::MultiCurrency;

use common::prelude::{Balance, FixedWrapper, OutcomeFee, QuoteAmount, SwapAmount, SwapOutcome};
use frame_system::pallet_prelude::BlockNumberFor;
use permissions::{Scope, INIT_DEX, MANAGE_DEX};
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
pub fn bob() -> AccountId {
    AccountId32::from([2u8; 32])
}
pub fn charlie() -> AccountId {
    AccountId32::from([3u8; 32])
}
pub fn dave() -> AccountId {
    AccountId32::from([4u8; 32])
}
pub fn adar() -> AccountId {
    GetADARAccountId::get()
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
    pub const MaximumBlockWeight: Weight = Weight::from_parts(1024, 0);
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    pub const GetNumSamples: usize = 1000;
    pub const GetBaseAssetId: AssetId = XOR;
    pub const GetSyntheticBaseAssetId: AssetId = XST;
    pub GetFee0: Fixed = fixed_from_basis_points(0u16);
    pub GetFee10: Fixed = fixed_from_basis_points(10u16);
    pub GetFee20: Fixed = fixed_from_basis_points(20u16);
    pub GetFee30: Fixed = fixed_from_basis_points(30u16);
    pub GetPswapDistributionAccountId: AccountId = AccountId32::from([151; 32]);
    pub const GetDefaultSubscriptionFrequency: BlockNumber = 10;
    pub const GetBurnUpdateFrequency: BlockNumber = 14400;
    pub GetParliamentAccountId: AccountId = AccountId32::from([152; 32]);
    pub GetMarketMakerRewardsAccountId: AccountId = AccountId32::from([9; 32]);
    pub GetBondingCurveRewardsAccountId: AccountId = AccountId32::from([10; 32]);
    pub GetFarmingRewardsAccountId: AccountId = AccountId32::from([12; 32]);
    pub GetCrowdloanRewardsAccountId: AccountId = AccountId32::from([13; 32]);
    pub GetADARAccountId: AccountId = AccountId32::from([14; 32]);
}

construct_runtime! {
    pub enum Runtime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        LiquidityProxy: liquidity_proxy::{Pallet, Call, Event<T>},
        Tokens: tokens::{Pallet, Call, Config<T>, Storage, Event<T>},
        Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
        Currencies: currencies::{Pallet, Call, Storage},
        Assets: assets::{Pallet, Call, Config<T>, Storage, Event<T>},
        Balances: pallet_balances::{Pallet, Call, Storage, Event<T>},
        DexManager: dex_manager::{Pallet, Call, Storage},
        MockLiquiditySource: mock_liquidity_source::<Instance1>::{Pallet, Call, Config<T>, Storage},
        MockLiquiditySource2: mock_liquidity_source::<Instance2>::{Pallet, Call, Config<T>, Storage},
        MockLiquiditySource3: mock_liquidity_source::<Instance3>::{Pallet, Call, Config<T>, Storage},
        MockLiquiditySource4: mock_liquidity_source::<Instance4>::{Pallet, Call, Config<T>, Storage},
        Technical: technical::{Pallet, Call, Storage, Event<T>},
        Permissions: permissions::{Pallet, Call, Config<T>, Storage, Event<T>},
        DexApi: dex_api::{Pallet, Call, Config, Storage, Event<T>},
        TradingPair: trading_pair::{Pallet, Call, Storage, Event<T>},
        VestedRewards: vested_rewards::{Pallet, Call, Storage, Event<T>},
        PoolXyk: pool_xyk::{Pallet, Call, Storage, Event<T>},
        PswapDistribution: pswap_distribution::{Pallet, Call, Storage, Event<T>},
        MBCPool: multicollateral_bonding_curve_pool::{Pallet, Call, Storage, Event<T>},
        CeresLiquidityLocker: ceres_liquidity_locker::{Pallet, Call, Storage, Event<T>},
        DemeterFarmingPlatform: demeter_farming_platform::{Pallet, Call, Storage, Event<T>},
        ExtendedAssets: extended_assets::{Pallet, Call, Storage, Event<T>},
    }
}

mock_assets_config!(Runtime);
mock_ceres_liquidity_locker_config!(Runtime, PoolXyk);
mock_common_config!(Runtime);
mock_currencies_config!(Runtime);
mock_demeter_farming_platform_config!(Runtime);
mock_dex_api_config!(
    Runtime,
    MockMCBCPool,
    pool_xyk::Pallet<Runtime>,
    MockXSTPool,
    mock_liquidity_source::Instance1,
    mock_liquidity_source::Instance2,
    mock_liquidity_source::Instance3,
    mock_liquidity_source::Instance4
);
mock_dex_manager_config!(Runtime);
mock_extended_assets_config!(Runtime);
mock_frame_system_config!(Runtime);
mock_liquidity_proxy_config!(
    Runtime,
    MockMCBCPool,
    MockXSTPool,
    mock_liquidity_source::Pallet<Runtime, mock_liquidity_source::Instance1>
);
mock_liquidity_source_config!(
    Runtime,
    mock_liquidity_source::Instance1,
    dex_manager::Pallet<Runtime>,
    GetFee0,
    trading_pair::Pallet<Runtime>
);
mock_liquidity_source_config!(
    Runtime,
    mock_liquidity_source::Instance2,
    dex_manager::Pallet<Runtime>,
    GetFee10,
    trading_pair::Pallet<Runtime>
);
mock_liquidity_source_config!(
    Runtime,
    mock_liquidity_source::Instance3,
    dex_manager::Pallet<Runtime>,
    GetFee20,
    trading_pair::Pallet<Runtime>
);
mock_liquidity_source_config!(
    Runtime,
    mock_liquidity_source::Instance4,
    dex_manager::Pallet<Runtime>,
    GetFee30,
    trading_pair::Pallet<Runtime>
);
mock_multicollateral_bonding_curve_pool_config!(Runtime);
mock_pallet_balances_config!(Runtime);
mock_pallet_timestamp_config!(Runtime);
mock_permissions_config!(Runtime);
mock_pool_xyk_config!(
    Runtime,
    trading_pair::Pallet<Runtime>,
    trading_pair::Pallet<Runtime>,
    pswap_distribution::Pallet<Runtime>
);
mock_pswap_distribution_config!(Runtime, pool_xyk::Pallet<Runtime>, common::mock::GetChameleonPools, (), LiquidityProxyBuyBackHandler<Runtime, GetBuyBackDexId>);
mock_technical_config!(Runtime, pool_xyk::PolySwapAction<DEXId, AssetId, AccountId, TechAccountId>);
mock_tokens_config!(Runtime);
mock_trading_pair_config!(Runtime);
mock_vested_rewards_config!(Runtime);

parameter_types! {
    pub const GetBuyBackAssetId: AssetId = VXOR;
}

pub struct ExtBuilder {
    pub total_supply: Balance,
    pub xyk_reserves: Vec<(DEXId, AssetId, (Balance, Balance))>,
    pub reserves: ReservesInit,
    pub reserves_2: ReservesInit,
    pub reserves_3: ReservesInit,
    pub reserves_4: ReservesInit,
    pub dex_list: Vec<(DEXId, DEXInfo<AssetId>)>,
    pub initial_permission_owners: Vec<(u32, Scope, Vec<AccountId>)>,
    pub initial_permissions: Vec<(AccountId, Scope, Vec<u32>)>,
    pub source_types: Vec<LiquiditySourceType>,
    pub endowed_accounts: Vec<(AccountId, AssetId, Balance, AssetSymbol, AssetName, u8)>,
    pub is_permissioned_xyk_pool: bool,
}

impl Default for ExtBuilder {
    fn default() -> Self {
        Self {
            total_supply: balance!(360000),
            xyk_reserves: Default::default(),
            reserves: vec![
                (DEX_A_ID, DOT, (fixed!(5000), fixed!(7000))),
                (DEX_A_ID, KSM, (fixed!(5500), fixed!(4000))),
                (DEX_B_ID, DOT, (fixed!(100), fixed!(45))),
                (DEX_C_ID, DOT, (fixed!(520), fixed!(550))),
                (DEX_D_ID, VAL, (fixed!(1000), fixed!(200000))),
                (DEX_D_ID, KSM, (fixed!(1000), fixed!(1000))),
                (DEX_D_ID, DOT, (fixed!(1000), fixed!(9000))),
                (DEX_D_ID, XST, (fixed!(1000), fixed!(9000))),
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
                        synthetic_base_asset_id: GetSyntheticBaseAssetId::get(),
                        is_public: true,
                    },
                ),
                (
                    DEX_B_ID,
                    DEXInfo {
                        base_asset_id: GetBaseAssetId::get(),
                        synthetic_base_asset_id: GetSyntheticBaseAssetId::get(),
                        is_public: true,
                    },
                ),
                (
                    DEX_C_ID,
                    DEXInfo {
                        base_asset_id: GetBaseAssetId::get(),
                        synthetic_base_asset_id: GetSyntheticBaseAssetId::get(),
                        is_public: true,
                    },
                ),
                (
                    DEX_D_ID,
                    DEXInfo {
                        base_asset_id: GetBaseAssetId::get(),
                        synthetic_base_asset_id: GetSyntheticBaseAssetId::get(),
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
                LiquiditySourceType::XSTPool,
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
                (
                    alice(),
                    XSTUSD,
                    balance!(0),
                    AssetSymbol(b"XSTUSD".to_vec()),
                    AssetName(b"XSTUSD".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                ),
                (
                    alice(),
                    XST,
                    balance!(0),
                    AssetSymbol(b"XST".to_vec()),
                    AssetName(b"XST".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                ),
                (
                    alice(),
                    KSM,
                    balance!(0),
                    AssetSymbol(b"KSM".to_vec()),
                    AssetName(b"Kusama".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                ),
            ],
            is_permissioned_xyk_pool: false,
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
        let total_supply = pallet_balances::Pallet::<Runtime>::total_issuance();
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
    ) -> Result<(SwapOutcome<Balance, AssetId>, Weight), DispatchError> {
        if !Self::can_exchange(dex_id, input_asset_id, output_asset_id) {
            panic!("Can't exchange");
        }
        let base_asset_id = &GetBaseAssetId::get();
        let reserves_tech_account_id =
            TechAccountId::Generic(b"mcbc_pool".to_vec(), b"main".to_vec());
        let reserves_account_id =
            Technical::tech_account_id_to_account_id(&reserves_tech_account_id)?;
        let current_supply = pallet_balances::Pallet::<Runtime>::total_issuance();

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
            QuoteAmount::WithDesiredInput { .. } => Ok((
                SwapOutcome::new(output_amount, OutcomeFee::xor(fee_amount)),
                Self::quote_weight(),
            )),
            QuoteAmount::WithDesiredOutput { .. } => Ok((
                SwapOutcome::new(input_amount, OutcomeFee::xor(fee_amount)),
                Self::quote_weight(),
            )),
        }
    }

    fn step_quote(
        dex_id: &DEXId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        amount: QuoteAmount<Balance>,
        recommended_samples_count: usize,
        deduce_fee: bool,
    ) -> Result<(DiscreteQuotation<AssetId, Balance>, Weight), DispatchError> {
        if !Self::can_exchange(dex_id, input_asset_id, output_asset_id) {
            panic!("Can't exchange");
        }

        let mut quotation = DiscreteQuotation::new();

        if amount.amount() == 0 {
            return Ok((quotation, Weight::zero()));
        }

        let step = amount.amount() / recommended_samples_count as Balance;

        let mut sub_in = 0;
        let mut sub_out = 0;
        let mut sub_fee = Default::default();

        for i in 1..=recommended_samples_count {
            let volume = amount.copy_direction(step * i as Balance);

            let (outcome, _weight) =
                Self::quote(dex_id, input_asset_id, output_asset_id, volume, deduce_fee)?;

            let (input, output, fee) = match volume {
                QuoteAmount::WithDesiredInput { desired_amount_in } => {
                    (desired_amount_in, outcome.amount, outcome.fee)
                }
                QuoteAmount::WithDesiredOutput { desired_amount_out } => {
                    (outcome.amount, desired_amount_out, outcome.fee)
                }
            };

            let input_chunk = input - sub_in;
            let output_chunk = output - sub_out;
            let fee_chunk = fee.clone().subtract(sub_fee);

            sub_in = input;
            sub_out = output;
            sub_fee = fee;

            quotation
                .chunks
                .push_back(SwapChunk::new(input_chunk, output_chunk, fee_chunk));
        }

        Ok((
            quotation,
            Self::step_quote_weight(recommended_samples_count),
        ))
    }

    fn exchange(
        _sender: &AccountId,
        _receiver: &AccountId,
        _dex_id: &DEXId,
        _input_asset_id: &AssetId,
        _output_asset_id: &AssetId,
        _desired_amount: SwapAmount<Balance>,
    ) -> Result<(SwapOutcome<Balance, AssetId>, Weight), DispatchError> {
        unimplemented!()
    }

    fn check_rewards(
        _dex_id: &DEXId,
        _input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        _input_amount: Balance,
        output_amount: Balance,
    ) -> Result<(Vec<(Balance, AssetId, RewardReason)>, Weight), DispatchError> {
        // for mock just return like in input
        if output_asset_id == &GetBaseAssetId::get() {
            Ok((
                vec![(
                    output_amount,
                    output_asset_id.clone(),
                    RewardReason::BuyOnBondingCurve,
                )],
                Weight::zero(),
            ))
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
    ) -> Result<SwapOutcome<Balance, AssetId>, DispatchError> {
        // TODO: implement if needed
        Self::quote(dex_id, input_asset_id, output_asset_id, amount, deduce_fee)
            .map(|(outcome, _)| outcome)
    }

    fn quote_weight() -> Weight {
        Weight::zero()
    }

    fn step_quote_weight(_samples_count: usize) -> Weight {
        Weight::zero()
    }

    fn exchange_weight() -> Weight {
        Weight::from_all(1)
    }

    fn check_rewards_weight() -> Weight {
        Weight::zero()
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
        (XOR, balance!(5.0)),
        (VAL, balance!(2.0)),
        (PSWAP, balance!(0.098)),
        (USDT, balance!(1.01)),
        (KSM, balance!(450.0)),
        (DOT, balance!(50.0)),
        (XST, balance!(182.9)),
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
                    synthetic_base_asset_id: GetSyntheticBaseAssetId::get(),
                    is_public: true,
                },
            )],
            ..Default::default()
        }
    }

    pub fn with_xyk_pool_xstusd(mut self) -> Self {
        self.xyk_reserves
            .push((DEX_D_ID, XSTUSD, (balance!(1000), balance!(1000))));
        self
    }

    pub fn with_xyk_pool(mut self) -> Self {
        self.xyk_reserves = vec![
            (DEX_A_ID, USDT, (balance!(1000), balance!(1000))),
            (DEX_A_ID, KSM, (balance!(1000), balance!(2000))),
            (DEX_C_ID, USDT, (balance!(600), balance!(10000))),
            (DEX_D_ID, USDT, (balance!(1000), balance!(1000))),
        ];
        self.source_types.push(LiquiditySourceType::XYKPool);
        self
    }

    pub fn with_permissioned_xyk_pool(mut self) -> Self {
        self = self.with_xyk_pool();
        self.is_permissioned_xyk_pool = true;
        self
    }

    fn prepare_asset_for_permissioned_pool(owner: &AccountId, asset_id: &AssetId) {
        use extended_assets::test_utils::register_sbt_asset;
        use frame_support::assert_ok;

        System::set_block_number(1);
        let owner_origin = RuntimeOrigin::signed(owner.clone());
        if !ExtendedAssets::is_asset_regulated(asset_id) {
            assets::Pallet::<Runtime>::update_asset_type(asset_id, &common::AssetType::Regulated)
                .expect("Failed to regulate Asset");
        }

        let sbt_asset_id = register_sbt_asset::<Runtime>(owner);
        assert_ok!(ExtendedAssets::bind_regulated_asset_to_sbt(
            owner_origin,
            sbt_asset_id,
            *asset_id
        ));
        Assets::mint_to(&sbt_asset_id, &owner, &owner, 1).expect("Failed to mint SBT");
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

        technical::GenesisConfig::<Runtime> {
            register_tech_accounts: vec![(
                GetLiquidityProxyAccountId::get().into(),
                GetLiquidityProxyTechAccountId::get(),
            )],
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
            regulated_assets: Default::default(),
            sbt_assets: Default::default(),
        }
        .assimilate_storage(&mut t)
        .unwrap();

        let owner = alice();
        let owner_origin: <Runtime as frame_system::Config>::RuntimeOrigin =
            frame_system::RawOrigin::Signed(owner.clone()).into();

        let mut ext: sp_io::TestExternalities = t.into();
        ext.execute_with(|| {
            for (dex_id, asset, (base_reserve, asset_reserve)) in self.xyk_reserves {
                let mint_amount: Balance = asset_reserve * 2;

                trading_pair::Pallet::<Runtime>::register(
                    owner_origin.clone(),
                    dex_id.into(),
                    XOR.into(),
                    asset.into(),
                )
                .unwrap();
                if self.is_permissioned_xyk_pool {
                    Self::prepare_asset_for_permissioned_pool(&owner, &asset.into());
                }
                assets::Pallet::<Runtime>::mint_to(&asset.into(), &owner, &owner, mint_amount)
                    .unwrap();
                pool_xyk::Pallet::<Runtime>::initialize_pool(
                    owner_origin.clone(),
                    dex_id.into(),
                    XOR.into(),
                    asset.into(),
                )
                .unwrap();
                if asset_reserve != balance!(0) && base_reserve != balance!(0) {
                    pool_xyk::Pallet::<Runtime>::deposit_liquidity(
                        owner_origin.clone(),
                        dex_id.into(),
                        XOR.into(),
                        asset.into(),
                        base_reserve,
                        asset_reserve,
                        balance!(1),
                        balance!(1),
                    )
                    .unwrap();
                }
            }
            // Set block number to 1 to start events tracking
            frame_system::Pallet::<Runtime>::set_block_number(1);
        });

        ext
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
        if output_asset_id == &XST.into() && input_asset_id == &XSTUSD.into() {
            return true;
        } else if input_asset_id == &XST.into() && output_asset_id == &XSTUSD.into() {
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
    ) -> Result<(SwapOutcome<Balance, AssetId>, Weight), DispatchError> {
        if !Self::can_exchange(dex_id, input_asset_id, output_asset_id) {
            panic!("Can't exchange");
        }
        let reserves_tech_account_id =
            TechAccountId::Generic(b"xst_pool".to_vec(), b"main".to_vec());
        let _reserves_account_id =
            Technical::tech_account_id_to_account_id(&reserves_tech_account_id)?;

        let input_asset_price: FixedWrapper = get_reference_prices()[input_asset_id].into();
        let output_asset_price: FixedWrapper = get_reference_prices()[output_asset_id].into();

        match amount {
            QuoteAmount::WithDesiredInput { desired_amount_in } => {
                let output_amount = desired_amount_in * input_asset_price / output_asset_price;
                Ok((
                    SwapOutcome::new(output_amount.try_into_balance().unwrap(), OutcomeFee::new()),
                    Self::quote_weight(),
                ))
            }
            QuoteAmount::WithDesiredOutput { desired_amount_out } => {
                let input_amount = desired_amount_out * output_asset_price / input_asset_price;
                Ok((
                    SwapOutcome::new(input_amount.try_into_balance().unwrap(), OutcomeFee::new()),
                    Self::quote_weight(),
                ))
            }
        }
    }

    fn step_quote(
        dex_id: &DEXId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        amount: QuoteAmount<Balance>,
        recommended_samples_count: usize,
        deduce_fee: bool,
    ) -> Result<(DiscreteQuotation<AssetId, Balance>, Weight), DispatchError> {
        if !Self::can_exchange(dex_id, input_asset_id, output_asset_id) {
            panic!("Can't exchange");
        }

        let mut quotation = DiscreteQuotation::new();

        if amount.amount() == 0 {
            return Ok((quotation, Weight::zero()));
        }

        let samples_count = if recommended_samples_count < 1 {
            1
        } else {
            recommended_samples_count
        };

        let (outcome, _weight) =
            Self::quote(dex_id, input_asset_id, output_asset_id, amount, deduce_fee)?;

        let (input, output, fee) = match amount {
            QuoteAmount::WithDesiredInput { desired_amount_in } => {
                (desired_amount_in, outcome.amount, outcome.fee)
            }
            QuoteAmount::WithDesiredOutput { desired_amount_out } => {
                (outcome.amount, desired_amount_out, outcome.fee)
            }
        };

        let chunk_fee = fee
            .rescale_by_ratio((fixed_wrapper!(1) / FixedWrapper::from(samples_count)))
            .unwrap();

        let chunk = SwapChunk::new(
            input / samples_count as Balance,
            output / samples_count as Balance,
            chunk_fee,
        );

        quotation.chunks = vec![chunk; samples_count].into();

        Ok((quotation, Self::step_quote_weight(samples_count)))
    }

    fn exchange(
        _sender: &AccountId,
        _receiver: &AccountId,
        _dex_id: &DEXId,
        _input_asset_id: &AssetId,
        _output_asset_id: &AssetId,
        _desired_amount: SwapAmount<Balance>,
    ) -> Result<(SwapOutcome<Balance, AssetId>, Weight), DispatchError> {
        unimplemented!()
    }

    fn check_rewards(
        _dex_id: &DEXId,
        _input_asset_id: &AssetId,
        _output_asset_id: &AssetId,
        _input_amount: Balance,
        _output_amount: Balance,
    ) -> Result<(Vec<(Balance, AssetId, RewardReason)>, Weight), DispatchError> {
        Ok((Vec::new(), Weight::zero())) // no rewards for XST
    }

    fn quote_without_impact(
        dex_id: &DEXId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        amount: QuoteAmount<Balance>,
        deduce_fee: bool,
    ) -> Result<SwapOutcome<Balance, AssetId>, DispatchError> {
        // TODO: implement if needed
        Self::quote(dex_id, input_asset_id, output_asset_id, amount, deduce_fee)
            .map(|(outcome, _)| outcome)
    }

    fn quote_weight() -> Weight {
        Weight::zero()
    }

    fn step_quote_weight(_samples_count: usize) -> Weight {
        Weight::zero()
    }

    fn exchange_weight() -> Weight {
        Weight::from_all(1)
    }

    fn check_rewards_weight() -> Weight {
        Weight::zero()
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
