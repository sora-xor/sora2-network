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

use crate::{self as multicollateral_bonding_curve_pool, Rewards, TotalRewards};
use common::mock::ExistentialDeposits;
use common::prelude::{
    AssetInfoProvider, Balance, OutcomeFee, PriceToolsProvider, QuoteAmount, SwapAmount,
    SwapOutcome,
};
use common::{
    self, balance, fixed_wrapper_u256, hash, mock_assets_config,
    mock_ceres_liquidity_locker_config, mock_common_config, mock_currencies_config,
    mock_demeter_farming_platform_config, mock_dex_manager_config, mock_frame_system_config,
    mock_liquidity_source_config, mock_multicollateral_bonding_curve_pool_config,
    mock_pallet_balances_config, mock_pallet_timestamp_config, mock_permissions_config,
    mock_pool_xyk_config, mock_price_tools_config, mock_pswap_distribution_config,
    mock_technical_config, mock_tokens_config, mock_trading_pair_config, Amount, AssetId32,
    AssetName, AssetSymbol, BuyBackHandler, DEXInfo, FixedWrapper256, LiquidityProxyTrait,
    LiquiditySourceFilter, LiquiditySourceType, PriceVariant, TechPurpose, Vesting, DAI,
    DEFAULT_BALANCE_PRECISION, KUSD, PSWAP, TBCD, USDT, VAL, VXOR, XOR, XST, XSTUSD,
};
use currencies::BasicCurrencyAdapter;
use frame_support::pallet_prelude::OptionQuery;
use frame_support::traits::GenesisBuild;
use frame_support::weights::Weight;
use frame_support::{construct_runtime, parameter_types, Blake2_128Concat};
use frame_system::pallet_prelude::BlockNumberFor;
use orml_traits::MultiCurrency;
use permissions::{Scope, INIT_DEX, MANAGE_DEX};
use sp_core::crypto::AccountId32;
use sp_runtime::traits::Zero;
use sp_runtime::{DispatchError, DispatchResult, Perbill};
use std::collections::HashMap;

pub type AccountId = AccountId32;
pub type BlockNumber = u64;
pub type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
type TechAssetId = common::TechAssetId<common::PredefinedAssetId>;
pub type ReservesAccount =
    mock_liquidity_source::ReservesAcc<Runtime, mock_liquidity_source::Instance1>;
pub type AssetId = AssetId32<common::PredefinedAssetId>;
type DEXId = common::DEXId;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

pub fn alice() -> AccountId {
    AccountId32::from([1u8; 32])
}

pub fn bob() -> AccountId {
    AccountId32::from([2u8; 32])
}

pub fn assets_owner() -> AccountId {
    AccountId32::from([3u8; 32])
}

pub fn incentives_account() -> AccountId {
    AccountId32::from([4u8; 32])
}

pub fn free_reserves_account() -> AccountId {
    AccountId32::from([5u8; 32])
}

pub fn tmp_account() -> AccountId {
    AccountId32::from([6u8; 32])
}

pub fn get_pool_reserves_account_id() -> AccountId {
    let reserves_tech_account_id = crate::ReservesAcc::<Runtime>::get();
    let reserves_account_id =
        Technical::tech_account_id_to_account_id(&reserves_tech_account_id).unwrap();
    reserves_account_id
}

pub const DEX_A_ID: DEXId = DEXId::Polkaswap;
pub const RETRY_DISTRIBUTION_FREQUENCY: BlockNumber = 1000;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = Weight::from_parts(1024, 0);
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    pub const GetDefaultFee: u16 = 30;
    pub const GetDefaultProtocolFee: u16 = 0;
    pub const GetBaseAssetId: AssetId = XOR;
    pub const GetSyntheticBaseAssetId: AssetId = XST;
    pub const TransferFee: u128 = 0;
    pub const CreationFee: u128 = 0;
    pub const TransactionByteFee: u128 = 1;
    pub const GetNumSamples: usize = 40;
    pub GetPswapDistributionAccountId: AccountId = AccountId32::from([151; 32]);
    pub const GetDefaultSubscriptionFrequency: BlockNumber = 10;
    pub const GetBurnUpdateFrequency: BlockNumber = 14400;
    pub GetParliamentAccountId: AccountId = AccountId32::from([152; 32]);
    pub GetMarketMakerRewardsAccountId: AccountId = AccountId32::from([153; 32]);
    pub GetBondingCurveRewardsAccountId: AccountId = AccountId32::from([154; 32]);
}

construct_runtime! {
    pub enum Runtime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        DexManager: dex_manager::{Pallet, Call, Storage},
        TradingPair: trading_pair::{Pallet, Call, Storage, Event<T>},
        MockLiquiditySource: mock_liquidity_source::<Instance1>::{Pallet, Call, Config<T>, Storage},
        // VestedRewards: vested_rewards::{Pallet, Call, Storage, Event<T>},
        Mcbcp: multicollateral_bonding_curve_pool::{Pallet, Call, Storage, Event<T>},
        Tokens: tokens::{Pallet, Call, Config<T>, Storage, Event<T>},
        Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
        Currencies: currencies::{Pallet, Call, Storage},
        Assets: assets::{Pallet, Call, Config<T>, Storage, Event<T>},
        Permissions: permissions::{Pallet, Call, Config<T>, Storage, Event<T>},
        Technical: technical::{Pallet, Call, Storage, Event<T>},
        Balances: pallet_balances::{Pallet, Call, Storage, Event<T>},
        PoolXYK: pool_xyk::{Pallet, Call, Storage, Event<T>},
        PswapDistribution: pswap_distribution::{Pallet, Call, Storage, Event<T>},
        CeresLiquidityLocker: ceres_liquidity_locker::{Pallet, Call, Storage, Event<T>},
        DemeterFarmingPlatform: demeter_farming_platform::{Pallet, Call, Storage, Event<T>},
        PriceTools: price_tools::{Pallet, Storage, Event<T>},
    }
}

mock_assets_config!(Runtime);
mock_ceres_liquidity_locker_config!(Runtime, PoolXYK);
mock_common_config!(Runtime);
mock_currencies_config!(Runtime);
mock_demeter_farming_platform_config!(Runtime);
mock_dex_manager_config!(Runtime);
mock_frame_system_config!(Runtime);
mock_liquidity_source_config!(Runtime, mock_liquidity_source::Instance1);
mock_multicollateral_bonding_curve_pool_config!(
    Runtime,
    MockDEXApi,
    BuyBackHandlerImpl,
    MockDEXApi,
    trading_pair::Pallet<Runtime>,
    MockVestedRewards
);
mock_pallet_balances_config!(Runtime);
mock_pallet_timestamp_config!(Runtime);
mock_permissions_config!(Runtime);
mock_pool_xyk_config!(Runtime);
mock_price_tools_config!(Runtime);
mock_pswap_distribution_config!(Runtime, PoolXYK);
mock_technical_config!(Runtime, pool_xyk::PolySwapAction<DEXId, AssetId, AccountId, TechAccountId>);
mock_tokens_config!(Runtime);
mock_trading_pair_config!(Runtime);

pub struct BuyBackHandlerImpl;

impl BuyBackHandler<AccountId, AssetId> for BuyBackHandlerImpl {
    fn mint_buy_back_and_burn(
        mint_asset_id: &AssetId,
        buy_back_asset_id: &AssetId,
        amount: Balance,
    ) -> Result<Balance, DispatchError> {
        let owner = Assets::asset_owner(&mint_asset_id).unwrap();
        Assets::mint_to(&mint_asset_id, &owner, &tmp_account(), amount)?;
        let amount =
            Self::buy_back_and_burn(&tmp_account(), mint_asset_id, buy_back_asset_id, amount)?;
        Ok(amount)
    }

    fn buy_back_and_burn(
        account_id: &AccountId,
        asset_id: &AssetId,
        buy_back_asset_id: &AssetId,
        amount: Balance,
    ) -> Result<Balance, DispatchError> {
        let outcome = MockDEXApi::inner_exchange(
            account_id,
            account_id,
            &DEX_A_ID,
            asset_id,
            buy_back_asset_id,
            SwapAmount::with_desired_input(amount, 0),
        )?;
        Assets::burn_from(buy_back_asset_id, account_id, account_id, outcome.amount)?;
        Ok(outcome.amount)
    }
}

pub struct MockVestedRewards;

impl Vesting<AccountId, AssetId> for MockVestedRewards {
    fn add_tbc_reward(account: &AccountId, amount: Balance) -> DispatchResult {
        Rewards::<Runtime>::mutate(account, |(_, old_amount)| {
            *old_amount = old_amount.saturating_add(amount)
        });
        TotalRewards::<Runtime>::mutate(|old_amount| {
            *old_amount = old_amount.saturating_add(amount)
        });
        Ok(())
    }

    fn add_farming_reward(_: &AccountId, _: Balance) -> DispatchResult {
        // do nothing
        Ok(())
    }
}

parameter_types! {
    pub const GetBuyBackAssetId: AssetId = VXOR;
}

pub struct MockDEXApi;

#[frame_support::storage_alias]
pub type MockPrices =
    StorageMap<MockDEXApi, Blake2_128Concat, (AssetId, AssetId), Balance, OptionQuery>;

impl MockDEXApi {
    pub fn with_price(asset_pair: (AssetId, AssetId), price: Balance) {
        MockPrices::insert(asset_pair.clone(), price);
        MockPrices::insert(
            (asset_pair.1, asset_pair.0),
            (fixed_wrapper_u256!(1) / FixedWrapper256::from(price))
                .try_into_balance()
                .unwrap(),
        );
    }

    fn get_mock_source_account() -> Result<(TechAccountId, AccountId), DispatchError> {
        let tech_account_id =
            TechAccountId::Pure(DEXId::Polkaswap.into(), TechPurpose::FeeCollector);
        let account_id = Technical::tech_account_id_to_account_id(&tech_account_id)?;
        Ok((tech_account_id, account_id))
    }

    pub fn init_without_reserves() -> Result<(), DispatchError> {
        let prices = get_mock_prices();
        for ((asset_a, asset_b), price) in prices {
            MockDEXApi::with_price((asset_a, asset_b), price);
        }

        let (tech_account_id, _) = Self::get_mock_source_account()?;
        Technical::register_tech_account_id(tech_account_id.clone())?;
        MockLiquiditySource::set_reserves_account_id(tech_account_id)?;
        Ok(())
    }

    pub fn add_reserves(funds: Vec<(AssetId, Balance)>) -> Result<(), DispatchError> {
        let (_, account_id) = Self::get_mock_source_account()?;
        for (asset_id, balance) in funds {
            Currencies::deposit(asset_id, &account_id, balance)?;
        }
        Ok(())
    }

    pub fn init() -> Result<(), DispatchError> {
        Self::init_without_reserves()?;
        Self::add_reserves(vec![
            (XOR, balance!(100000)),
            (VAL, balance!(100000)),
            (TBCD, balance!(100000)),
            (USDT, balance!(1000000)),
            (KUSD, balance!(1000000)),
        ])?;
        Ok(())
    }

    fn _can_exchange(
        _target_id: &DEXId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
    ) -> bool {
        MockPrices::contains_key(&(*input_asset_id, *output_asset_id))
    }

    fn get_price(input_asset_id: &AssetId, output_asset_id: &AssetId) -> Balance {
        MockPrices::get(&(*input_asset_id, *output_asset_id))
            .or_else(|| {
                frame_support::log::error!(
                    "Failed to get price {:?} -> {:?}",
                    input_asset_id,
                    output_asset_id
                );
                None
            })
            .unwrap()
    }

    fn inner_quote(
        _target_id: &DEXId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        amount: QuoteAmount<Balance>,
        deduce_fee: bool,
    ) -> Result<SwapOutcome<Balance, AssetId>, DispatchError> {
        match amount {
            QuoteAmount::WithDesiredInput {
                desired_amount_in, ..
            } if deduce_fee => {
                let amount_out = FixedWrapper256::from(desired_amount_in)
                    * Self::get_price(input_asset_id, output_asset_id);
                let fee = amount_out.clone() * balance!(0.003);
                let fee = fee.into_balance();
                let amount_out: Balance = amount_out.into_balance();
                let amount_out = amount_out - fee;
                Ok(SwapOutcome::new(amount_out, OutcomeFee::xor(fee)))
            }
            QuoteAmount::WithDesiredInput {
                desired_amount_in, ..
            } => {
                let amount_out = FixedWrapper256::from(desired_amount_in)
                    * Self::get_price(input_asset_id, output_asset_id);
                Ok(SwapOutcome::new(
                    amount_out.into_balance(),
                    OutcomeFee::new(),
                ))
            }
            QuoteAmount::WithDesiredOutput {
                desired_amount_out, ..
            } if deduce_fee => {
                let amount_in = FixedWrapper256::from(desired_amount_out)
                    / Self::get_price(input_asset_id, output_asset_id);
                let with_fee = amount_in.clone() / balance!(0.997);
                let fee = with_fee.clone() - amount_in;
                let fee = fee.into_balance();
                let with_fee = with_fee.into_balance();
                Ok(SwapOutcome::new(with_fee, OutcomeFee::xor(fee)))
            }
            QuoteAmount::WithDesiredOutput {
                desired_amount_out, ..
            } => {
                let amount_in = FixedWrapper256::from(desired_amount_out)
                    / Self::get_price(input_asset_id, output_asset_id);
                Ok(SwapOutcome::new(
                    amount_in.into_balance(),
                    OutcomeFee::new(),
                ))
            }
        }
    }

    fn inner_exchange(
        sender: &AccountId,
        receiver: &AccountId,
        target_id: &DEXId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        swap_amount: SwapAmount<Balance>,
    ) -> Result<SwapOutcome<Balance, AssetId>, DispatchError> {
        match swap_amount {
            SwapAmount::WithDesiredInput {
                desired_amount_in, ..
            } => {
                let outcome = Self::inner_quote(
                    target_id,
                    input_asset_id,
                    output_asset_id,
                    swap_amount.into(),
                    true,
                )?;
                let reserves_account_id =
                    &Technical::tech_account_id_to_account_id(&ReservesAccount::get())?;
                assert_ne!(desired_amount_in, 0);
                let old = Assets::total_balance(input_asset_id, sender)?;
                Assets::transfer_from(
                    input_asset_id,
                    sender,
                    reserves_account_id,
                    desired_amount_in,
                )?;
                let new = Assets::total_balance(input_asset_id, sender)?;
                assert_ne!(old, new);
                Assets::transfer_from(
                    output_asset_id,
                    reserves_account_id,
                    receiver,
                    outcome.amount,
                )?;
                Ok(SwapOutcome::new(outcome.amount, outcome.fee))
            }
            SwapAmount::WithDesiredOutput {
                desired_amount_out, ..
            } => {
                let outcome = Self::inner_quote(
                    target_id,
                    input_asset_id,
                    output_asset_id,
                    swap_amount.into(),
                    true,
                )?;
                let reserves_account_id =
                    &Technical::tech_account_id_to_account_id(&ReservesAccount::get())?;
                assert_ne!(outcome.amount, 0);
                let old = Assets::total_balance(input_asset_id, sender)?;
                Assets::transfer_from(input_asset_id, sender, reserves_account_id, outcome.amount)?;
                let new = Assets::total_balance(input_asset_id, sender)?;
                assert_ne!(old, new);
                Assets::transfer_from(
                    output_asset_id,
                    reserves_account_id,
                    receiver,
                    desired_amount_out,
                )?;
                Ok(SwapOutcome::new(outcome.amount, outcome.fee))
            }
        }
    }
}

pub fn get_mock_prices() -> HashMap<(AssetId, AssetId), Balance> {
    let prices = vec![
        ((XOR, VAL), balance!(2.0)),
        // USDT
        ((XOR, USDT), balance!(100.0)),
        ((VAL, USDT), balance!(50.0)),
        // DAI
        ((XOR, DAI), balance!(102.0)),
        ((VAL, DAI), balance!(51.0)),
        ((USDT, DAI), balance!(1.02)),
        ((XSTUSD, DAI), balance!(1)),
        // PSWAP
        ((XOR, PSWAP), balance!(10)),
        ((VAL, PSWAP), balance!(5)),
        ((USDT, PSWAP), balance!(0.1)),
        ((DAI, PSWAP), balance!(0.098)),
        ((XSTUSD, PSWAP), balance!(1)),
        // XSTUSD
        ((XOR, XSTUSD), balance!(102.0)),
        // TBCD
        ((XOR, TBCD), balance!(103.0)),
        // XST
        ((XOR, XST), balance!(0.001)),
        // KUSD
        ((XOR, KUSD), balance!(20.0)),
    ];
    prices.into_iter().collect()
}

impl LiquidityProxyTrait<DEXId, AccountId, AssetId> for MockDEXApi {
    fn exchange(
        _dex_id: DEXId,
        sender: &AccountId,
        receiver: &AccountId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        amount: SwapAmount<Balance>,
        filter: LiquiditySourceFilter<DEXId, LiquiditySourceType>,
    ) -> Result<SwapOutcome<Balance, AssetId>, DispatchError> {
        Self::inner_exchange(
            sender,
            receiver,
            &filter.dex_id,
            input_asset_id,
            output_asset_id,
            amount,
        )
    }

    fn quote(
        _dex_id: DEXId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        amount: QuoteAmount<Balance>,
        filter: LiquiditySourceFilter<DEXId, LiquiditySourceType>,
        deduce_fee: bool,
    ) -> Result<SwapOutcome<Balance, AssetId>, DispatchError> {
        Self::inner_quote(
            &filter.dex_id,
            input_asset_id,
            output_asset_id,
            amount,
            deduce_fee,
        )
    }
}

impl PriceToolsProvider<AssetId> for MockDEXApi {
    fn is_asset_registered(_asset_id: &AssetId) -> bool {
        unimplemented!()
    }

    fn get_average_price(
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        _price_variant: PriceVariant,
    ) -> Result<Balance, DispatchError> {
        Ok(Self::inner_quote(
            &DEXId::Polkaswap.into(),
            input_asset_id,
            output_asset_id,
            QuoteAmount::with_desired_input(balance!(1)),
            true,
        )?
        .amount)
    }

    fn register_asset(_: &AssetId) -> DispatchResult {
        // do nothing
        Ok(())
    }
}

pub struct ExtBuilder {
    endowed_accounts: Vec<(AccountId, AssetId, Balance, AssetSymbol, AssetName, u8)>,
    trading_pairs: Vec<(DEXId, trading_pair::TradingPair<Runtime>)>,
    dex_list: Vec<(DEXId, DEXInfo<AssetId>)>,
    initial_permission_owners: Vec<(u32, Scope, Vec<AccountId>)>,
    initial_permissions: Vec<(AccountId, Scope, Vec<u32>)>,
    reference_asset_id: AssetId,
}

impl Default for ExtBuilder {
    fn default() -> Self {
        Self {
            endowed_accounts: vec![
                (
                    alice(),
                    USDT,
                    0,
                    AssetSymbol(b"USDT".to_vec()),
                    AssetName(b"Tether USD".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                ),
                (
                    alice(),
                    XOR,
                    balance!(350000),
                    AssetSymbol(b"XOR".to_vec()),
                    AssetName(b"SORA".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                ),
                (
                    alice(),
                    VAL,
                    balance!(500000),
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
                    XSTUSD,
                    balance!(100),
                    AssetSymbol(b"XSTUSD".to_vec()),
                    AssetName(b"SORA Synthetic USD".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                ),
                (
                    alice(),
                    DAI,
                    balance!(100),
                    AssetSymbol(b"DAI".to_vec()),
                    AssetName(b"DAI".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                ),
                (
                    alice(),
                    KUSD,
                    balance!(100),
                    AssetSymbol(b"KUSD".to_vec()),
                    AssetName(b"KUSD".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                ),
            ],
            trading_pairs: vec![(
                DEX_A_ID,
                trading_pair::TradingPair::<Runtime> {
                    base_asset_id: XOR,
                    target_asset_id: XST,
                },
            )],
            dex_list: vec![(
                DEX_A_ID,
                DEXInfo {
                    base_asset_id: GetBaseAssetId::get(),
                    synthetic_base_asset_id: GetSyntheticBaseAssetId::get(),
                    is_public: true,
                },
            )],
            initial_permission_owners: vec![
                (INIT_DEX, Scope::Unlimited, vec![alice()]),
                (MANAGE_DEX, Scope::Limited(hash(&DEX_A_ID)), vec![alice()]),
            ],
            initial_permissions: vec![
                (alice(), Scope::Unlimited, vec![INIT_DEX]),
                (alice(), Scope::Limited(hash(&DEX_A_ID)), vec![MANAGE_DEX]),
                (
                    assets_owner(),
                    Scope::Unlimited,
                    vec![permissions::MINT, permissions::BURN],
                ),
                (
                    free_reserves_account(),
                    Scope::Unlimited,
                    vec![permissions::MINT, permissions::BURN],
                ),
            ],
            reference_asset_id: USDT,
        }
    }
}

impl ExtBuilder {
    pub fn new(
        endowed_accounts: Vec<(AccountId, AssetId, Balance, AssetSymbol, AssetName, u8)>,
    ) -> Self {
        Self {
            endowed_accounts,
            ..Default::default()
        }
    }

    pub fn with_tbcd(mut self) -> Self {
        self.endowed_accounts.push((
            alice(),
            TBCD,
            balance!(500000),
            AssetSymbol(b"TBCD".to_vec()),
            AssetName(b"Token Bonding Curve Dollar".to_vec()),
            DEFAULT_BALANCE_PRECISION,
        ));
        self
    }

    pub fn with_tbcd_pool(mut self) -> Self {
        self.trading_pairs.push((
            DEX_A_ID,
            trading_pair::TradingPair::<Runtime> {
                base_asset_id: XOR,
                target_asset_id: TBCD,
            },
        ));
        self
    }

    #[allow(dead_code)]
    pub fn bench_init() -> Self {
        Self {
            endowed_accounts: vec![
                (
                    alice(),
                    XOR,
                    balance!(350000),
                    AssetSymbol(b"XOR".to_vec()),
                    AssetName(b"SORA".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                ),
                (
                    alice(),
                    VAL,
                    balance!(500000),
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
                    XSTUSD,
                    balance!(100),
                    AssetSymbol(b"XSTUSD".to_vec()),
                    AssetName(b"SORA Synthetic USD".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                ),
                (
                    alice(),
                    DAI,
                    balance!(100000),
                    AssetSymbol(b"DAI".to_vec()),
                    AssetName(b"DAI".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                ),
            ],
            dex_list: vec![(
                DEX_A_ID,
                DEXInfo {
                    base_asset_id: GetBaseAssetId::get(),
                    synthetic_base_asset_id: GetSyntheticBaseAssetId::get(),
                    is_public: true,
                },
            )],
            initial_permission_owners: vec![
                (INIT_DEX, Scope::Unlimited, vec![alice()]),
                (MANAGE_DEX, Scope::Limited(hash(&DEX_A_ID)), vec![alice()]),
            ],
            initial_permissions: vec![
                (
                    alice(),
                    Scope::Unlimited,
                    vec![INIT_DEX, permissions::MINT, permissions::BURN],
                ),
                (
                    alice(),
                    Scope::Limited(hash(&DEX_A_ID)),
                    vec![MANAGE_DEX, permissions::MINT, permissions::BURN],
                ),
                (
                    assets_owner(),
                    Scope::Unlimited,
                    vec![permissions::MINT, permissions::BURN],
                ),
                (
                    free_reserves_account(),
                    Scope::Unlimited,
                    vec![permissions::MINT, permissions::BURN],
                ),
            ],
            trading_pairs: vec![
                (
                    DEX_A_ID,
                    trading_pair::TradingPair::<Runtime> {
                        base_asset_id: XOR,
                        target_asset_id: DAI,
                    },
                ),
                (
                    DEX_A_ID,
                    trading_pair::TradingPair::<Runtime> {
                        base_asset_id: XOR,
                        target_asset_id: VAL,
                    },
                ),
                (
                    DEX_A_ID,
                    trading_pair::TradingPair::<Runtime> {
                        base_asset_id: XOR,
                        target_asset_id: TBCD,
                    },
                ),
                (
                    DEX_A_ID,
                    trading_pair::TradingPair::<Runtime> {
                        base_asset_id: XOR,
                        target_asset_id: XST,
                    },
                ),
            ],
            reference_asset_id: USDT,
        }
    }

    pub fn build(self) -> sp_io::TestExternalities {
        common::test_utils::init_logger();
        let mut t = frame_system::GenesisConfig::default()
            .build_storage::<Runtime>()
            .unwrap();

        pallet_balances::GenesisConfig::<Runtime> {
            balances: self
                .endowed_accounts
                .iter()
                .cloned()
                .filter_map(|(account_id, asset_id, balance, ..)| {
                    if asset_id == GetBaseAssetId::get() {
                        Some((account_id, balance))
                    } else {
                        None
                    }
                })
                .chain(vec![
                    (bob(), 0),
                    (assets_owner(), 0),
                    (incentives_account(), 0),
                    (free_reserves_account(), 0),
                ])
                .collect(),
        }
        .assimilate_storage(&mut t)
        .unwrap();

        crate::GenesisConfig::<Runtime> {
            distribution_accounts: Default::default(),
            reserves_account_id: Default::default(),
            reference_asset_id: self.reference_asset_id,
            incentives_account_id: Some(incentives_account()),
            initial_collateral_assets: Default::default(),
            free_reserves_account_id: Some(free_reserves_account()),
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
                        Balance::zero(),
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

        tokens::GenesisConfig::<Runtime> {
            balances: self
                .endowed_accounts
                .into_iter()
                .map(|(account_id, asset_id, balance, ..)| (account_id, asset_id, balance))
                .collect(),
        }
        .assimilate_storage(&mut t)
        .unwrap();

        trading_pair::GenesisConfig::<Runtime> {
            trading_pairs: self.trading_pairs,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        t.into()
    }
}
