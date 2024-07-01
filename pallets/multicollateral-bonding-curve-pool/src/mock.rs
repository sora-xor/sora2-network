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

use crate::{self as multicollateral_bonding_curve_pool, Config, Rewards, TotalRewards};
use common::mock::{ExistentialDeposits, GetTradingPairRestrictedFlag};
use common::prelude::{
    AssetInfoProvider, Balance, FixedWrapper, OutcomeFee, PriceToolsProvider, QuoteAmount,
    SwapAmount, SwapOutcome,
};
use common::{
    self, balance, fixed, fixed_wrapper, hash, Amount, AssetId32, AssetName, AssetSymbol,
    BuyBackHandler, DEXInfo, Fixed, LiquidityProxyTrait, LiquiditySourceFilter,
    LiquiditySourceType, PriceVariant, TechPurpose, Vesting, DAI, DEFAULT_BALANCE_PRECISION, PSWAP,
    TBCD, USDT, VAL, XOR, XST, XSTUSD,
};
use currencies::BasicCurrencyAdapter;
use frame_support::pallet_prelude::OptionQuery;
use frame_support::traits::{Everything, GenesisBuild};
use frame_support::weights::Weight;
use frame_support::{construct_runtime, parameter_types, Blake2_128Concat};
use frame_system::pallet_prelude::BlockNumberFor;
use hex_literal::hex;
use orml_traits::MultiCurrency;
use permissions::{Scope, INIT_DEX, MANAGE_DEX};
use sp_core::crypto::AccountId32;
use sp_core::H256;
use sp_runtime::testing::Header;
use sp_runtime::traits::{BlakeTwo256, IdentityLookup, Zero};
use sp_runtime::{BuildStorage, DispatchError, DispatchResult, Perbill, Percent};
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

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = Weight::from_parts(1024, 0);
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    pub const GetDefaultFee: u16 = 30;
    pub const GetDefaultProtocolFee: u16 = 0;
    pub const GetBaseAssetId: AssetId = XOR;
    pub const GetSyntheticBaseAssetId: AssetId = XST;
    pub const ExistentialDeposit: u128 = 1;
    pub const TransferFee: u128 = 0;
    pub const CreationFee: u128 = 0;
    pub const TransactionByteFee: u128 = 1;
    pub const GetNumSamples: usize = 40;
    pub GetIncentiveAssetId: AssetId = common::AssetId32::from_bytes(hex!("0200050000000000000000000000000000000000000000000000000000000000").into());
    pub GetPswapDistributionAccountId: AccountId = AccountId32::from([151; 32]);
    pub const GetDefaultSubscriptionFrequency: BlockNumber = 10;
    pub const GetBurnUpdateFrequency: BlockNumber = 14400;
    pub GetParliamentAccountId: AccountId = AccountId32::from([152; 32]);
    pub GetMarketMakerRewardsAccountId: AccountId = AccountId32::from([153; 32]);
    pub GetBondingCurveRewardsAccountId: AccountId = AccountId32::from([154; 32]);
    pub GetXykFee: Fixed = fixed!(0.003);
    pub const MinimumPeriod: u64 = 5;
    pub GetTBCBuyBackTBCDPercent: Fixed = fixed!(0.025);
}

construct_runtime! {
    pub enum Runtime {
        System: frame_system::{Pallet, Call, Config<T>, Storage, Event<T>},
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

impl frame_system::Config for Runtime {
    type BaseCallFilter = Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Nonce = u64;
    type Block = Block;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = BlockHashCount;
    type DbWeight = ();
    type Version = ();
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type PalletInfo = PalletInfo;
    type SS58Prefix = ();
    type OnSetCode = ();
    type MaxConsumers = frame_support::traits::ConstU32<65536>;
}

impl dex_manager::Config for Runtime {}

impl trading_pair::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type EnsureDEXManager = dex_manager::Pallet<Runtime>;
    type DexInfoProvider = dex_manager::Pallet<Runtime>;
    type WeightInfo = ();
}

impl mock_liquidity_source::Config<mock_liquidity_source::Instance1> for Runtime {
    type GetFee = ();
    type EnsureDEXManager = ();
    type EnsureTradingPairExists = ();
    type DexInfoProvider = dex_manager::Pallet<Runtime>;
}

impl Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type LiquidityProxy = MockDEXApi;
    type EnsureTradingPairExists = trading_pair::Pallet<Runtime>;
    type EnsureDEXManager = dex_manager::Pallet<Runtime>;
    type PriceToolsPallet = MockDEXApi;
    type VestedRewardsPallet = MockVestedRewards;
    type TradingPairSourceManager = trading_pair::Pallet<Runtime>;
    type BuyBackHandler = BuyBackHandlerImpl;
    type BuyBackTBCDPercent = GetTBCBuyBackTBCDPercent;
    type WeightInfo = ();
}

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

impl tokens::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;
    type Amount = Amount;
    type CurrencyId = <Runtime as assets::Config>::AssetId;
    type WeightInfo = ();
    type ExistentialDeposits = ExistentialDeposits;
    type CurrencyHooks = ();
    type MaxLocks = ();
    type MaxReserves = ();
    type ReserveIdentifier = ();
    type DustRemovalWhitelist = Everything;
}

impl currencies::Config for Runtime {
    type MultiCurrency = Tokens;
    type NativeCurrency = BasicCurrencyAdapter<Runtime, Balances, Amount, BlockNumber>;
    type GetNativeCurrencyId = <Runtime as assets::Config>::GetBaseAssetId;
    type WeightInfo = ();
}

impl common::Config for Runtime {
    type DEXId = DEXId;
    type LstId = common::LiquiditySourceType;
}

parameter_types! {
    pub const GetBuyBackAssetId: AssetId = TBCD;
    pub GetBuyBackSupplyAssets: Vec<AssetId> = vec![VAL, PSWAP];
    pub const GetBuyBackPercentage: u8 = 10;
    pub const GetBuyBackAccountId: AccountId = AccountId::new(hex!(
            "0000000000000000000000000000000000000000000000000000000000000023"
    ));
    pub const GetBuyBackDexId: DEXId = DEXId::Polkaswap;
}

impl assets::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type ExtraAccountId = [u8; 32];
    type ExtraAssetRecordArg =
        common::AssetIdExtraAssetRecordArg<common::DEXId, common::LiquiditySourceType, [u8; 32]>;
    type AssetId = AssetId;
    type GetBaseAssetId = GetBaseAssetId;
    type GetBuyBackAssetId = GetBuyBackAssetId;
    type GetBuyBackSupplyAssets = GetBuyBackSupplyAssets;
    type GetBuyBackPercentage = GetBuyBackPercentage;
    type GetBuyBackAccountId = GetBuyBackAccountId;
    type GetBuyBackDexId = GetBuyBackDexId;
    type BuyBackLiquidityProxy = ();
    type Currency = currencies::Pallet<Runtime>;
    type GetTotalBalance = ();
    type WeightInfo = ();
}

impl permissions::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
}

impl technical::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type TechAssetId = TechAssetId;
    type TechAccountId = TechAccountId;
    type Trigger = ();
    type Condition = ();
    type SwapAction = pool_xyk::PolySwapAction<DEXId, AssetId, AccountId, TechAccountId>;
}

impl pallet_balances::Config for Runtime {
    type Balance = Balance;
    type RuntimeEvent = RuntimeEvent;
    type DustRemoval = ();
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

impl pswap_distribution::Config for Runtime {
    const PSWAP_BURN_PERCENT: Percent = Percent::from_percent(3);
    type RuntimeEvent = RuntimeEvent;
    type GetIncentiveAssetId = GetIncentiveAssetId;
    type GetTBCDAssetId = GetBuyBackAssetId;
    type LiquidityProxy = ();
    type CompatBalance = Balance;
    type GetDefaultSubscriptionFrequency = GetDefaultSubscriptionFrequency;
    type GetBurnUpdateFrequency = GetBurnUpdateFrequency;
    type GetTechnicalAccountId = GetPswapDistributionAccountId;
    type EnsureDEXManager = ();
    type OnPswapBurnedAggregator = ();
    type WeightInfo = ();
    type GetParliamentAccountId = GetParliamentAccountId;
    type PoolXykPallet = PoolXYK;
    type BuyBackHandler = ();
    type DexInfoProvider = dex_manager::Pallet<Runtime>;
}

impl price_tools::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type LiquidityProxy = ();
    type TradingPairSourceManager = trading_pair::Pallet<Runtime>;
    type WeightInfo = price_tools::weights::SubstrateWeight<Runtime>;
}

impl demeter_farming_platform::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type DemeterAssetId = ();
    const BLOCKS_PER_HOUR_AND_A_HALF: BlockNumberFor<Self> = 900;
    type WeightInfo = ();
}

impl pool_xyk::Config for Runtime {
    const MIN_XOR: Balance = balance!(0.0007);
    type RuntimeEvent = RuntimeEvent;
    type PairSwapAction = pool_xyk::PairSwapAction<DEXId, AssetId, AccountId, TechAccountId>;
    type DepositLiquidityAction =
        pool_xyk::DepositLiquidityAction<AssetId, AccountId, TechAccountId>;
    type WithdrawLiquidityAction =
        pool_xyk::WithdrawLiquidityAction<AssetId, AccountId, TechAccountId>;
    type PolySwapAction = pool_xyk::PolySwapAction<DEXId, AssetId, AccountId, TechAccountId>;
    type EnsureDEXManager = dex_manager::Pallet<Runtime>;
    type TradingPairSourceManager = trading_pair::Pallet<Runtime>;
    type DexInfoProvider = dex_manager::Pallet<Runtime>;
    type EnsureTradingPairExists = trading_pair::Pallet<Runtime>;
    type EnabledSourcesManager = trading_pair::Pallet<Runtime>;
    type GetFee = GetXykFee;
    type OnPoolCreated = PswapDistribution;
    type OnPoolReservesChanged = ();
    type WeightInfo = ();
    type XSTMarketInfo = ();
    type GetTradingPairRestrictedFlag = GetTradingPairRestrictedFlag;
}
impl pallet_timestamp::Config for Runtime {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = MinimumPeriod;
    type WeightInfo = ();
}

impl ceres_liquidity_locker::Config for Runtime {
    const BLOCKS_PER_ONE_DAY: BlockNumberFor<Self> = 14_440;
    type RuntimeEvent = RuntimeEvent;
    type XYKPool = PoolXYK;
    type DemeterFarmingPlatform = DemeterFarmingPlatform;
    type CeresAssetId = ();
    type WeightInfo = ();
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
            (fixed_wrapper!(1) / FixedWrapper::from(price))
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
        MockPrices::get(&(*input_asset_id, *output_asset_id)).unwrap()
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
                let amount_out = FixedWrapper::from(desired_amount_in)
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
                let amount_out = FixedWrapper::from(desired_amount_in)
                    * Self::get_price(input_asset_id, output_asset_id);
                Ok(SwapOutcome::new(
                    amount_out.into_balance(),
                    OutcomeFee::new(),
                ))
            }
            QuoteAmount::WithDesiredOutput {
                desired_amount_out, ..
            } if deduce_fee => {
                let amount_in = FixedWrapper::from(desired_amount_out)
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
                let amount_in = FixedWrapper::from(desired_amount_out)
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
        let mut t = frame_system::GenesisConfig::<Runtime>::default()
            .build_storage()
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
