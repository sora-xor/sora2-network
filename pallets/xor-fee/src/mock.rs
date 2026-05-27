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

// TODO #167: fix clippy warnings
#![allow(clippy::all)]
#![allow(deprecated)]

use common::mock::ExistentialDeposits;
use common::prelude::{Balance, FixedWrapper, QuoteAmount, SwapAmount, SwapOutcome};
#[cfg(feature = "wip")] // Dynamic fee
use common::weights::constants::SMALL_FEE;
#[cfg(feature = "wip")] // Dynamic fee
use common::DAI;
use common::{
    self, balance, mock_assets_config, mock_ceres_liquidity_locker_config, mock_common_config,
    mock_currencies_config, mock_demeter_farming_platform_config, mock_dex_manager_config,
    mock_frame_system_config, mock_pallet_balances_config, mock_pallet_timestamp_config,
    mock_pallet_transaction_payment_config, mock_permissions_config, mock_pool_xyk_config,
    mock_price_tools_config, mock_pswap_distribution_config, mock_technical_config,
    mock_tokens_config, mock_trading_pair_config, Amount, AssetId32, AssetName, AssetSymbol,
    LiquidityProxyTrait, LiquiditySourceFilter, LiquiditySourceType, OnValBurned,
    ReferrerAccountProvider, KUSD, PSWAP, TBCD, VAL, XOR,
};
use currencies::BasicCurrencyAdapter;
use frame_support::dispatch::{DispatchInfo, Pays, PostDispatchInfo};
use frame_support::pallet_prelude::{Hooks, ValueQuery};
use frame_support::traits::{Currency, ExistenceRequirement, Randomness, WithdrawReasons};
use frame_support::weights::Weight;
use frame_support::{construct_runtime, parameter_types, storage_alias};
use frame_system::pallet_prelude::BlockNumberFor;
use frame_system::EnsureRoot;
use permissions::{Scope, BURN, MINT};
use sp_arithmetic::FixedU128;
use sp_core::H256;
use sp_runtime::{AccountId32, BuildStorage, DispatchError, DispatchResult, Percent};
use traits::MultiCurrency;

pub use crate::{self as xor_fee, Config, Pallet};

// Configure a mock runtime to test the pallet.
pub type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
pub type AccountId = AccountId32;
pub type BlockNumber = u64;
type AssetId = AssetId32<common::PredefinedAssetId>;
type TechAssetId = common::TechAssetId<common::PredefinedAssetId>;
type DEXId = common::DEXId;
#[allow(dead_code)]
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

pub const SMALL_REFERENCE_AMOUNT: Balance = balance!(0.7);
pub const PRICE_XOR_DAI: Balance = balance!(800);
pub fn account_from_str(s: &str) -> AccountId {
    sp_core::blake2_256(s.as_bytes()).into()
}

parameter_types! {
    pub GetPostponeAccountId: AccountId = account_from_str("postpone");
    pub GetPaysNoAccountId: AccountId = account_from_str("pays-no");
    pub GetFeeSourceAccountId: AccountId = account_from_str("fee-source");
    pub GetReferalAccountId: AccountId = account_from_str("referal");
    pub GetReferrerAccountId: AccountId = account_from_str("referrer");
    pub const BlockHashCount: u64 = 250;
    pub const FeeReferrerWeight: u32 = 10; // 10%
    pub const FeeXorBurnedWeight: u32 = 35; // 35%
    pub const FeeValBurnedWeight: u32 = 40; // 40%
    pub const FeeKusdBurnedWeight: u32 = 0;
    pub const MinimalFeeInAsset: Balance = balance!(0.00000000000000001); // Minimal amount for proportions right calculations
    pub const RemintXorBurnPercent: Percent = Percent::from_percent(40);
    pub const RemintKusdBuyBackPercent: Percent = Percent::from_percent(0);
    pub const XorId: AssetId = XOR;
    pub const ValId: AssetId = VAL;
    pub const KusdId: AssetId = KUSD;
    pub const DEXIdValue: DEXId = DEXId::Polkaswap;
    pub const GetBaseAssetId: AssetId = XOR;
    pub GetXorFeeAccountId: AccountId = account_from_str("xor-fee");
    pub GetParliamentAccountId: AccountId = account_from_str("sora-parliament");
    pub const MaxWhiteListTokens: u32 = 2;

    pub GetPswapDistributionAccountId: AccountId = AccountId32::from([3; 32]);
    pub const GetDefaultSubscriptionFrequency: BlockNumber = 10;
    pub const GetBurnUpdateFrequency: BlockNumber = 14400;
}

construct_runtime! {
    pub enum Runtime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config<T>, Storage, Event<T>},
        Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
        Currencies: currencies::{Pallet, Call, Storage},
        Tokens: tokens::{Pallet, Call, Config<T>, Storage, Event<T>},
        Permissions: permissions::{Pallet, Call, Config<T>, Storage, Event<T>},
        Assets: assets::{Pallet, Call, Config<T>, Storage, Event<T>},
        TransactionPayment: pallet_transaction_payment::{Pallet, Storage, Event<T>},
        XorFee: xor_fee::{Pallet, Call, Event<T>},
        PoolXYK: pool_xyk::{Pallet, Call, Storage, Event<T>},
        PswapDistribution: pswap_distribution::{Pallet, Call, Config<T>, Storage, Event<T>},
        DexManager: dex_manager::{Pallet, Call, Config<T>, Storage},
        TradingPair: trading_pair::{Pallet, Call, Config<T>, Storage, Event<T>},
        DemeterFarmingPlatform: demeter_farming_platform::{Pallet, Call, Storage, Event<T>},
        CeresLiquidityLocker: ceres_liquidity_locker::{Pallet, Call, Storage, Event<T>},
        Technical: technical::{Pallet, Call, Storage, Event<T>},
        Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
        PriceTools: price_tools::{Pallet, Storage, Event<T>},
    }
}

mock_assets_config!(Runtime);
mock_common_config!(Runtime);
mock_currencies_config!(Runtime);
mock_demeter_farming_platform_config!(Runtime);
mock_frame_system_config!(Runtime);
mock_pallet_balances_config!(Runtime);
mock_pallet_transaction_payment_config!(Runtime);
mock_permissions_config!(Runtime);
mock_tokens_config!(Runtime);
mock_pool_xyk_config!(Runtime);
mock_pswap_distribution_config!(Runtime, PoolXYK);
mock_dex_manager_config!(Runtime);
mock_trading_pair_config!(Runtime);
mock_ceres_liquidity_locker_config!(Runtime, PoolXYK);
mock_technical_config!(Runtime, pool_xyk::PolySwapAction<DEXId, AssetId, AccountId, TechAccountId>);
mock_pallet_timestamp_config!(Runtime);
mock_price_tools_config!(Runtime);

parameter_types! {
    pub const GetBuyBackAssetId: AssetId = TBCD;
}

parameter_types! {
    pub const ForcedMultiplierAt: BlockNumber = 0;
    pub const ForcedMultiplierValue: FixedU128 = FixedU128::from_inner(1_000_000_000_000_000_000u128);
}

pub struct CustomFees;

pub const FREE_CUSTOM_FEE_REMARK: &[u8] = b"free-custom-fee";
pub const ASSET_NOT_FOUND_REMARK: &[u8] = b"asset-not-found";
pub const FEE_CALC_FAILED_REMARK: &[u8] = b"fee-calc-failed";
pub const OTHER_WITHDRAW_ERROR_REMARK: &[u8] = b"other-withdraw-error";
pub const STAKING_VAL_PAYOUT_REMARK: &[u8] = b"staking-val-payout";

fn is_remark(call: &RuntimeCall, expected: &[u8]) -> bool {
    matches!(
        call,
        RuntimeCall::System(frame_system::Call::remark { remark })
            if remark.as_slice() == expected
    )
}

impl xor_fee::ApplyCustomFees<RuntimeCall, AccountId> for CustomFees {
    type FeeDetails = Balance;
    fn compute_fee(call: &RuntimeCall) -> Option<(Balance, Self::FeeDetails)> {
        let fee = match call {
            _ if is_remark(call, FREE_CUSTOM_FEE_REMARK) => Some(0),
            RuntimeCall::Assets(assets::Call::register { .. }) => Some(balance!(0.007)),
            RuntimeCall::Assets(..) => Some(balance!(0.0007)),
            _ => None,
        };
        fee.map(|fee| (fee, fee))
    }

    fn compute_actual_fee(
        _post_info: &sp_runtime::traits::PostDispatchInfoOf<RuntimeCall>,
        _info: &sp_runtime::traits::DispatchInfoOf<RuntimeCall>,
        _result: &sp_runtime::DispatchResult,
        fee_details: Option<Self::FeeDetails>,
    ) -> Option<Balance> {
        fee_details
    }

    fn get_fee_source(who: &AccountId, call: &RuntimeCall, _fee: Balance) -> AccountId {
        if matches!(call, RuntimeCall::System(..)) {
            return GetFeeSourceAccountId::get();
        }
        who.clone()
    }

    fn should_be_paid(who: &AccountId, _call: &RuntimeCall) -> bool {
        if *who == GetPaysNoAccountId::get() {
            return false;
        }
        true
    }

    fn should_be_postponed(
        who: &AccountId,
        _fee_source: &AccountId,
        _call: &RuntimeCall,
        _fee: Balance,
    ) -> bool {
        if *who == GetPostponeAccountId::get() {
            return true;
        }
        false
    }
}

#[storage_alias]
pub type ValBurned<T: Config> = StorageValue<crate::Pallet<T>, Balance, ValueQuery>;

pub struct ValBurnedAggregator;

impl OnValBurned for ValBurnedAggregator {
    fn on_val_burned(amount: Balance) {
        ValBurned::<Runtime>::mutate(|x| *x += amount);
    }
}

#[storage_alias]
pub type StakingValPayoutPreCalls<T: Config> = StorageValue<crate::Pallet<T>, u32, ValueQuery>;

#[storage_alias]
pub type StakingValPayoutPostCalls<T: Config> = StorageValue<crate::Pallet<T>, u32, ValueQuery>;

#[storage_alias]
pub type StakingValPayoutLastPre<T: Config> = StorageValue<crate::Pallet<T>, Balance, ValueQuery>;

#[storage_alias]
pub type StakingValPayoutLastResultOk<T: Config> = StorageValue<crate::Pallet<T>, bool, ValueQuery>;

pub struct MockStakingValPayout;

impl xor_fee::StakingValPayout<RuntimeCall, AccountId> for MockStakingValPayout {
    type Pre = Balance;

    fn pre_dispatch(call: &RuntimeCall) -> Option<Self::Pre> {
        if !is_remark(call, STAKING_VAL_PAYOUT_REMARK) {
            return None;
        }

        StakingValPayoutPreCalls::<Runtime>::mutate(|calls| *calls = calls.saturating_add(1));
        Some(balance!(42))
    }

    fn post_dispatch(pre: Option<Self::Pre>, result: &DispatchResult) {
        if let Some(pre) = pre {
            StakingValPayoutPostCalls::<Runtime>::mutate(|calls| *calls = calls.saturating_add(1));
            StakingValPayoutLastPre::<Runtime>::put(pre);
            StakingValPayoutLastResultOk::<Runtime>::put(result.is_ok());
        }
    }
}

pub struct WithdrawFee;

impl xor_fee::WithdrawFee<Runtime> for WithdrawFee {
    fn can_withdraw_fee(
        _who: &AccountId,
        fee_source: &AccountId,
        call: &RuntimeCall,
        fee: Balance,
    ) -> Result<(), DispatchError> {
        if is_remark(call, ASSET_NOT_FOUND_REMARK) {
            return Err(xor_fee::Error::<Runtime>::AssetNotFound.into());
        }
        if is_remark(call, FEE_CALC_FAILED_REMARK) {
            return Err(xor_fee::Error::<Runtime>::FeeCalculationFailed.into());
        }
        if is_remark(call, OTHER_WITHDRAW_ERROR_REMARK) {
            return Err(DispatchError::Other("mock withdraw error"));
        }

        let current_balance = Balances::free_balance(fee_source);
        let resulting_balance = current_balance
            .checked_sub(fee)
            .ok_or(xor_fee::Error::<Runtime>::FeeCalculationFailed)?;
        Balances::ensure_can_withdraw(
            fee_source,
            fee,
            WithdrawReasons::TRANSACTION_PAYMENT,
            resulting_balance,
        )?;
        Ok(())
    }

    fn withdraw_fee(
        _who: &AccountId,
        fee_source: &AccountId,
        call: &RuntimeCall,
        fee: Balance,
    ) -> Result<
        (
            AccountId,
            Option<crate::NegativeImbalanceOf<Runtime>>,
            Option<AssetId>,
        ),
        DispatchError,
    > {
        if is_remark(call, ASSET_NOT_FOUND_REMARK) {
            return Err(xor_fee::Error::<Runtime>::AssetNotFound.into());
        }
        if is_remark(call, FEE_CALC_FAILED_REMARK) {
            return Err(xor_fee::Error::<Runtime>::FeeCalculationFailed.into());
        }
        if is_remark(call, OTHER_WITHDRAW_ERROR_REMARK) {
            return Err(DispatchError::Other("mock withdraw error"));
        }

        Ok((
            fee_source.clone(),
            Some(Balances::withdraw(
                fee_source,
                fee,
                WithdrawReasons::TRANSACTION_PAYMENT,
                ExistenceRequirement::KeepAlive,
            )?),
            None,
        ))
    }
}

pub struct MockRandomness;

impl Randomness<H256, BlockNumber> for MockRandomness {
    fn random(_subject: &[u8]) -> (H256, BlockNumber) {
        (
            H256::from_low_u64_be(System::block_number()),
            System::block_number(),
        )
    }
}

impl Config for Runtime {
    type XorCurrency = Balances;
    type KusdId = KusdId;
    type ValId = ValId;
    type XorId = XorId;
    type ForcedMultiplierAt = ForcedMultiplierAt;
    type ForcedMultiplier = ForcedMultiplierValue;
    type FeeReferrerWeight = FeeReferrerWeight;
    type FeeXorBurnedWeight = FeeXorBurnedWeight;
    type FeeValBurnedWeight = FeeValBurnedWeight;
    type FeeKusdBurnedWeight = FeeKusdBurnedWeight;
    type RemintXorBurnPercent = RemintXorBurnPercent;
    type RemintKusdBuyBackPercent = RemintKusdBuyBackPercent;
    type DEXIdValue = DEXIdValue;
    type LiquidityProxy = MockLiquidityProxy;
    type OnValBurned = ValBurnedAggregator;
    type StakingValPayout = MockStakingValPayout;
    type CustomFees = CustomFees;
    type GetTechnicalAccountId = GetXorFeeAccountId;
    type WithdrawFee = WithdrawFee;
    type FullIdentification = ();
    type BuyBackHandler = ();
    type ReferrerAccountProvider = MockReferrerAccountProvider;
    type WeightInfo = ();
    #[cfg(not(feature = "wip"))] // Dynamic fee
    type DynamicMultiplier = ();
    #[cfg(feature = "wip")] // Dynamic fee
    type DynamicMultiplier = DynamicMultiplier;
    type PermittedSetPeriod = EnsureRoot<AccountId>;
    type MaxWhiteListTokens = MaxWhiteListTokens;
    type RuntimeCall = RuntimeCall;
    type PoolXyk = PoolXYK;
    type WhiteListOrigin = EnsureRoot<AccountId>;
    type PriceTools = price_tools::FastPriceTools<Runtime>;
    type MinimalFeeInAsset = ();
    type Randomness = MockRandomness;
}

#[cfg(feature = "wip")] // Dynamic fee
pub struct DynamicMultiplier;

#[cfg(feature = "wip")] // Dynamic fee
impl xor_fee::CalculateMultiplier<common::AssetIdOf<Runtime>, DispatchError> for DynamicMultiplier {
    fn calculate_multiplier(
        input_asset: &AssetId,
        ref_asset: &AssetId,
    ) -> Result<FixedU128, DispatchError> {
        let price: FixedWrapper = FixedWrapper::from(match (input_asset, ref_asset) {
            (&XOR, &DAI) => PRICE_XOR_DAI,
            _ => balance!(0.000000000000000001),
        });
        let new_multiplier: Balance = (SMALL_REFERENCE_AMOUNT / (SMALL_FEE * price))
            .try_into_balance()
            .map_err(|_| xor_fee::pallet::Error::<Runtime>::MultiplierCalculationFailed)?;
        Ok(FixedU128::from_inner(new_multiplier))
    }
}

pub struct MockReferrerAccountProvider;

impl ReferrerAccountProvider<AccountId> for MockReferrerAccountProvider {
    fn get_referrer_account(who: &AccountId) -> Option<AccountId> {
        if *who == GetReferalAccountId::get() {
            Some(GetReferrerAccountId::get())
        } else {
            None
        }
    }
}

pub struct MockLiquidityProxy;

impl MockLiquidityProxy {
    fn mock_price(asset_id: &AssetId) -> Balance {
        match asset_id {
            &XOR => balance!(1.0),
            &VAL => balance!(3.1),
            &PSWAP => balance!(13),
            _ => balance!(2.5),
        }
    }

    fn exchange_inner(
        sender: Option<&AccountId>,
        receiver: Option<&AccountId>,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        amount: QuoteAmount<Balance>,
    ) -> Result<SwapOutcome<Balance, AssetId>, DispatchError> {
        let input_price = Self::mock_price(input_asset_id);
        let output_price = Self::mock_price(output_asset_id);
        let price = FixedWrapper::from(output_price) / FixedWrapper::from(input_price);
        let (input_amount, output_amount, is_reversed) = match amount {
            QuoteAmount::WithDesiredInput {
                desired_amount_in, ..
            } => (
                desired_amount_in,
                (FixedWrapper::from(desired_amount_in) * price)
                    .try_into_balance()
                    .unwrap(),
                false,
            ),
            QuoteAmount::WithDesiredOutput {
                desired_amount_out, ..
            } => (
                (FixedWrapper::from(desired_amount_out) / price)
                    .try_into_balance()
                    .unwrap(),
                desired_amount_out,
                true,
            ),
        };
        if let Some((sender, receiver)) = sender.zip(receiver) {
            Currencies::withdraw(
                *input_asset_id,
                sender,
                input_amount,
                ExistenceRequirement::AllowDeath,
            )?;
            Currencies::deposit(*output_asset_id, receiver, output_amount)?;
        }
        if is_reversed {
            Ok(SwapOutcome::new(input_amount, Default::default()))
        } else {
            Ok(SwapOutcome::new(output_amount, Default::default()))
        }
    }
}

impl LiquidityProxyTrait<DEXId, AccountId, AssetId> for MockLiquidityProxy {
    fn exchange(
        _dex_id: DEXId,
        sender: &AccountId,
        receiver: &AccountId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        amount: SwapAmount<Balance>,
        _filter: LiquiditySourceFilter<DEXId, LiquiditySourceType>,
    ) -> Result<SwapOutcome<Balance, AssetId>, DispatchError> {
        Self::exchange_inner(
            Some(sender),
            Some(receiver),
            input_asset_id,
            output_asset_id,
            amount.into(),
        )
    }

    fn quote(
        _dex_id: DEXId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        amount: QuoteAmount<Balance>,
        _filter: LiquiditySourceFilter<DEXId, LiquiditySourceType>,
        _deduce_fee: bool,
    ) -> Result<SwapOutcome<Balance, AssetId>, DispatchError> {
        Self::exchange_inner(None, None, input_asset_id, output_asset_id, amount)
    }
}

pub fn initial_balance() -> Balance {
    balance!(1000)
}

pub fn initial_reserves() -> Balance {
    balance!(10000)
}

pub struct ExtBuilder;

impl ExtBuilder {
    pub fn build() -> sp_io::TestExternalities {
        let mut t = frame_system::GenesisConfig::<Runtime>::default()
            .build_storage()
            .unwrap();

        pallet_balances::GenesisConfig::<Runtime> {
            balances: vec![(GetXorFeeAccountId::get(), initial_balance())],
            dev_accounts: None,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        permissions::GenesisConfig::<Runtime> {
            initial_permission_owners: vec![
                (MINT, Scope::Unlimited, vec![GetXorFeeAccountId::get()]),
                (BURN, Scope::Unlimited, vec![GetXorFeeAccountId::get()]),
            ],
            initial_permissions: vec![(
                GetXorFeeAccountId::get(),
                Scope::Unlimited,
                vec![MINT, BURN],
            )],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        assets::GenesisConfig::<Runtime> {
            endowed_assets: vec![
                (
                    XOR,
                    GetXorFeeAccountId::get(),
                    AssetSymbol(b"XOR".to_vec()),
                    AssetName(b"SORA".to_vec()),
                    18,
                    Balance::from(0u32),
                    true,
                    None,
                    None,
                ),
                (
                    VAL,
                    GetXorFeeAccountId::get(),
                    AssetSymbol(b"VAL".to_vec()),
                    AssetName(b"SORA Validator Token".to_vec()),
                    18,
                    Balance::from(0u32),
                    true,
                    None,
                    None,
                ),
                (
                    TBCD,
                    GetXorFeeAccountId::get(),
                    AssetSymbol(b"TBCD".to_vec()),
                    AssetName(b"TBCD".to_vec()),
                    18,
                    balance!(100),
                    true,
                    None,
                    None,
                ),
            ],
            regulated_assets: Default::default(),
            sbt_assets: Default::default(),
        }
        .assimilate_storage(&mut t)
        .unwrap();

        tokens::GenesisConfig::<Runtime> {
            balances: vec![(GetXorFeeAccountId::get().clone(), VAL, balance!(1000))],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        t.into()
    }
}

/// create a transaction info struct from weight. Handy to avoid building the whole struct.
pub fn info_from_weight(w: Weight) -> DispatchInfo {
    // pays_fee: Pays::Yes -- class: DispatchClass::Normal
    DispatchInfo {
        call_weight: w,
        extension_weight: Weight::zero(),
        ..Default::default()
    }
}

pub fn info_pays_no(w: Weight) -> DispatchInfo {
    DispatchInfo {
        pays_fee: Pays::No,
        call_weight: w,
        extension_weight: Weight::zero(),
        ..Default::default()
    }
}

pub fn default_post_info() -> PostDispatchInfo {
    PostDispatchInfo {
        actual_weight: None,
        pays_fee: Default::default(),
    }
}

pub fn post_info_from_weight(w: Weight) -> PostDispatchInfo {
    PostDispatchInfo {
        actual_weight: Some(w),
        pays_fee: Default::default(),
    }
}

pub fn post_info_pays_no() -> PostDispatchInfo {
    PostDispatchInfo {
        actual_weight: None,
        pays_fee: Pays::No,
    }
}

pub fn run_to_block(n: u64) {
    while System::block_number() < n {
        System::on_initialize(System::block_number());
        System::set_block_number(System::block_number() + 1);
        System::on_finalize(System::block_number());
        XorFee::on_initialize(System::block_number());
    }
}
