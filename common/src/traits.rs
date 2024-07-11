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

use crate::prelude::{
    permissions::PermissionId, ManagementMode, QuoteAmount, SwapAmount, SwapOutcome,
};
use crate::{
    Amount, AssetId32, AssetName, AssetSymbol, AssetType, BalancePrecision, ContentSource,
    Description, Fixed, LiquiditySourceFilter, LiquiditySourceId, LiquiditySourceType, Oracle,
    PredefinedAssetId, PriceVariant, PswapRemintInfo, RewardReason,
};

use frame_support::dispatch::{DispatchResult, DispatchResultWithPostInfo};
use frame_support::pallet_prelude::MaybeSerializeDeserialize;
use frame_support::sp_runtime::traits::BadOrigin;
use frame_support::sp_runtime::DispatchError;
use frame_support::weights::Weight;
use frame_support::Parameter;
use frame_system::pallet_prelude::OriginFor;
use frame_system::RawOrigin;
use orml_traits::{
    MultiCurrency, MultiCurrencyExtended, MultiLockableCurrency, MultiReservableCurrency,
};
use sp_core::{Get, H256};
use sp_runtime::traits::Member;
//FIXME maybe try info or try from is better than From and Option.
//use sp_std::convert::TryInto;
use crate::alt::DiscreteQuotation;
use crate::primitives::Balance;
use codec::{Decode, Encode, MaxEncodedLen};
use sp_std::collections::btree_set::BTreeSet;
use sp_std::vec::Vec;

/// Check on origin that it is a DEX owner.
pub trait EnsureDEXManager<DEXId, AccountId, Error> {
    fn ensure_can_manage<OuterOrigin>(
        dex_id: &DEXId,
        origin: OuterOrigin,
        mode: ManagementMode,
    ) -> Result<Option<AccountId>, Error>
    where
        OuterOrigin: Into<Result<RawOrigin<AccountId>, OuterOrigin>>;
}

impl<DEXId, AccountId> EnsureDEXManager<DEXId, AccountId, DispatchError> for () {
    fn ensure_can_manage<OuterOrigin>(
        _dex_id: &DEXId,
        origin: OuterOrigin,
        _mode: ManagementMode,
    ) -> Result<Option<AccountId>, DispatchError>
    where
        OuterOrigin: Into<Result<RawOrigin<AccountId>, OuterOrigin>>,
    {
        match origin.into() {
            Ok(RawOrigin::Signed(t)) => Ok(Some(t)),
            Ok(RawOrigin::Root) => Ok(None),
            _ => Err(BadOrigin.into()),
        }
    }
}

pub trait EnsureTradingPairExists<DEXId, AssetId, Error> {
    fn ensure_trading_pair_exists(
        dex_id: &DEXId,
        base_asset_id: &AssetId,
        target_asset_id: &AssetId,
    ) -> Result<(), Error>;
}

impl<DEXId, AssetId> EnsureTradingPairExists<DEXId, AssetId, DispatchError> for () {
    fn ensure_trading_pair_exists(
        _dex_id: &DEXId,
        _base_asset_id: &AssetId,
        _target_asset_id: &AssetId,
    ) -> Result<(), DispatchError> {
        Err(DispatchError::CannotLookup)
    }
}

pub trait TradingPairSourceManager<DEXId, AssetId> {
    fn list_enabled_sources_for_trading_pair(
        dex_id: &DEXId,
        base_asset_id: &AssetId,
        target_asset_id: &AssetId,
    ) -> Result<BTreeSet<LiquiditySourceType>, DispatchError>;

    fn is_source_enabled_for_trading_pair(
        dex_id: &DEXId,
        base_asset_id: &AssetId,
        target_asset_id: &AssetId,
        source_type: LiquiditySourceType,
    ) -> Result<bool, DispatchError>;

    fn enable_source_for_trading_pair(
        dex_id: &DEXId,
        base_asset_id: &AssetId,
        target_asset_id: &AssetId,
        source_type: LiquiditySourceType,
    ) -> DispatchResult;

    fn disable_source_for_trading_pair(
        dex_id: &DEXId,
        base_asset_id: &AssetId,
        target_asset_id: &AssetId,
        source_type: LiquiditySourceType,
    ) -> DispatchResult;

    fn is_trading_pair_enabled(
        dex_id: &DEXId,
        base_asset_id: &AssetId,
        target_asset_id: &AssetId,
    ) -> Result<bool, DispatchError>;

    fn register_pair(
        dex_id: DEXId,
        base_asset_id: AssetId,
        target_asset_id: AssetId,
    ) -> Result<(), DispatchError>;
}

impl<DEXId, AssetId> TradingPairSourceManager<DEXId, AssetId> for () {
    fn list_enabled_sources_for_trading_pair(
        _dex_id: &DEXId,
        _base_asset_id: &AssetId,
        _target_asset_id: &AssetId,
    ) -> Result<BTreeSet<LiquiditySourceType>, DispatchError> {
        Err(DispatchError::CannotLookup)
    }

    fn is_source_enabled_for_trading_pair(
        _dex_id: &DEXId,
        _base_asset_id: &AssetId,
        _target_asset_id: &AssetId,
        _source_type: LiquiditySourceType,
    ) -> Result<bool, DispatchError> {
        Err(DispatchError::CannotLookup)
    }

    fn enable_source_for_trading_pair(
        _dex_id: &DEXId,
        _base_asset_id: &AssetId,
        _target_asset_id: &AssetId,
        _source_type: LiquiditySourceType,
    ) -> DispatchResult {
        Err(DispatchError::CannotLookup)
    }

    fn disable_source_for_trading_pair(
        _dex_id: &DEXId,
        _base_asset_id: &AssetId,
        _target_asset_id: &AssetId,
        _source_type: LiquiditySourceType,
    ) -> DispatchResult {
        Err(DispatchError::CannotLookup)
    }

    fn is_trading_pair_enabled(
        _dex_id: &DEXId,
        _base_asset_id: &AssetId,
        _target_asset_id: &AssetId,
    ) -> Result<bool, DispatchError> {
        Err(DispatchError::CannotLookup)
    }

    fn register_pair(
        _dex_id: DEXId,
        _base_asset_id: AssetId,
        _target_asset_id: AssetId,
    ) -> Result<(), DispatchError> {
        Err(DispatchError::CannotLookup)
    }
}

/// Indicates that particular object can be used to perform exchanges.
pub trait LiquiditySource<TargetId, AccountId, AssetId: Ord + Clone, Amount, Error> {
    /// Check if liquidity source provides an exchange from given input asset to output asset.
    fn can_exchange(
        target_id: &TargetId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
    ) -> bool;

    /// Get spot price of tokens based on desired amount.
    fn quote(
        target_id: &TargetId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        amount: QuoteAmount<Amount>,
        deduce_fee: bool,
    ) -> Result<(SwapOutcome<Amount, AssetId>, Weight), Error>;

    /// Get the input/output liquidity divided into steps based on the desired amount.
    /// The count of steps may differ from the `recommended_samples_count`
    fn step_quote(
        target_id: &TargetId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        amount: QuoteAmount<Amount>,
        recommended_samples_count: usize,
        deduce_fee: bool,
    ) -> Result<(DiscreteQuotation<AssetId, Amount>, Weight), Error>;

    /// Perform exchange based on desired amount.
    fn exchange(
        sender: &AccountId,
        receiver: &AccountId,
        target_id: &TargetId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        swap_amount: SwapAmount<Amount>,
    ) -> Result<(SwapOutcome<Amount, AssetId>, Weight), Error>;

    /// Get rewards that are given for perfoming given exchange.
    fn check_rewards(
        target_id: &TargetId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        input_amount: Amount,
        output_amount: Amount,
    ) -> Result<(Vec<(Amount, AssetId, RewardReason)>, Weight), Error>;

    /// Get spot price of tokens based on desired amount, ignoring non-linearity
    /// of underlying liquidity source.
    fn quote_without_impact(
        target_id: &TargetId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        amount: QuoteAmount<Amount>,
        deduce_fee: bool,
    ) -> Result<SwapOutcome<Amount, AssetId>, Error>;

    /// Get weight of quote
    fn quote_weight() -> Weight;

    /// Get weight of step quote
    fn step_quote_weight(samples_count: usize) -> Weight;

    /// Get weight of exchange
    fn exchange_weight() -> Weight;

    /// Get weight of exchange
    fn check_rewards_weight() -> Weight;
}

/// Implements trading pairs LockedLiquiditySources storage
pub trait LockedLiquiditySourcesManager<LiquiditySourceType> {
    fn get() -> Vec<LiquiditySourceType>;
    fn set(liquidity_source_types: Vec<LiquiditySourceType>);
    fn append(liquidity_source_type: LiquiditySourceType);
}

/// Implements trading pair EnabledSources storage
pub trait EnabledSourcesManager<DEXId, AssetId> {
    fn mutate_remove(dex_id: &DEXId, base_asset_id: &AssetId, target_asset_id: &AssetId);
}

impl<DEXId, AssetId> EnabledSourcesManager<DEXId, AssetId> for () {
    fn mutate_remove(_dex_id: &DEXId, _baset_asset_id: &AssetId, _target_asset_id: &AssetId) {
        unimplemented!()
    }
}

/// *Hook*-like trait for oracles to capture newly relayed symbols.
///
/// A struct implementing this trait can be specified in oracle pallet *Config*
/// so that it will be called every time new symbols were relayed.
pub trait OnNewSymbolsRelayed<Symbol> {
    /// Upload newly relayed symbols to oracle proxy
    /// - `symbols`: which symbols to upload
    fn on_new_symbols_relayed(
        oracle_variant: Oracle,
        symbols: BTreeSet<Symbol>,
    ) -> Result<(), DispatchError>;
}

impl<Symbol> OnNewSymbolsRelayed<Symbol> for () {
    fn on_new_symbols_relayed(
        _oracle_variant: Oracle,
        _symbols: BTreeSet<Symbol>,
    ) -> Result<(), DispatchError> {
        Ok(())
    }
}

/// `DataFeed` trait indicates that particular object could be used for querying oracle data.
pub trait DataFeed<Symbol, Rate, ResolveTime> {
    /// Get rate for the specified symbol
    /// - `symbol`: which symbol to query
    fn quote(symbol: &Symbol) -> Result<Option<Rate>, DispatchError>;

    /// Get all supported symbols and their last update time
    fn list_enabled_symbols() -> Result<Vec<(Symbol, ResolveTime)>, DispatchError>;

    /// Get rate for the specified symbol without any checks
    /// - `symbol`: which symbol to query
    fn quote_unchecked(symbol: &Symbol) -> Option<Rate>;
}

impl<Symbol, Rate, ResolveTime> DataFeed<Symbol, Rate, ResolveTime> for () {
    fn quote(_symbol: &Symbol) -> Result<Option<Rate>, DispatchError> {
        Ok(None)
    }

    fn list_enabled_symbols() -> Result<Vec<(Symbol, ResolveTime)>, DispatchError> {
        Ok(Vec::new())
    }

    fn quote_unchecked(_symbol: &Symbol) -> Option<Rate> {
        None
    }
}

pub trait OnSymbolDisabled<Symbol> {
    fn disable_symbol(symbol: &Symbol);
}

impl<Symbol> OnSymbolDisabled<Symbol> for () {
    fn disable_symbol(_symbol: &Symbol) {}
}

impl<DEXId, AccountId, AssetId: Ord + Clone>
    LiquiditySource<DEXId, AccountId, AssetId, Fixed, DispatchError> for ()
{
    fn can_exchange(
        _target_id: &DEXId,
        _input_asset_id: &AssetId,
        _output_asset_id: &AssetId,
    ) -> bool {
        false
    }

    fn quote(
        _target_id: &DEXId,
        _input_asset_id: &AssetId,
        _output_asset_id: &AssetId,
        _amount: QuoteAmount<Fixed>,
        _deduce_fee: bool,
    ) -> Result<(SwapOutcome<Fixed, AssetId>, Weight), DispatchError> {
        Err(DispatchError::CannotLookup)
    }

    fn step_quote(
        _target_id: &DEXId,
        _input_asset_id: &AssetId,
        _output_asset_id: &AssetId,
        _amount: QuoteAmount<Fixed>,
        _recommended_samples_count: usize,
        _deduce_fee: bool,
    ) -> Result<(DiscreteQuotation<AssetId, Fixed>, Weight), DispatchError> {
        Err(DispatchError::CannotLookup)
    }

    fn exchange(
        _sender: &AccountId,
        _receiver: &AccountId,
        _target_id: &DEXId,
        _input_asset_id: &AssetId,
        _output_asset_id: &AssetId,
        _swap_amount: SwapAmount<Fixed>,
    ) -> Result<(SwapOutcome<Fixed, AssetId>, Weight), DispatchError> {
        Err(DispatchError::CannotLookup)
    }

    fn check_rewards(
        _target_id: &DEXId,
        _input_asset_id: &AssetId,
        _output_asset_id: &AssetId,
        _input_amount: Fixed,
        _output_amount: Fixed,
    ) -> Result<(Vec<(Fixed, AssetId, RewardReason)>, Weight), DispatchError> {
        Err(DispatchError::CannotLookup)
    }

    fn quote_without_impact(
        _target_id: &DEXId,
        _input_asset_id: &AssetId,
        _output_asset_id: &AssetId,
        _amount: QuoteAmount<Fixed>,
        _deduce_fee: bool,
    ) -> Result<SwapOutcome<Fixed, AssetId>, DispatchError> {
        Err(DispatchError::CannotLookup)
    }

    fn quote_weight() -> Weight {
        Weight::zero()
    }

    fn step_quote_weight(_samples_count: usize) -> Weight {
        Weight::zero()
    }

    fn exchange_weight() -> Weight {
        Weight::zero()
    }

    fn check_rewards_weight() -> Weight {
        Weight::zero()
    }
}

impl<DEXId, AccountId, AssetId: Ord + Clone>
    LiquiditySource<DEXId, AccountId, AssetId, Balance, DispatchError> for ()
{
    fn can_exchange(
        _target_id: &DEXId,
        _input_asset_id: &AssetId,
        _output_asset_id: &AssetId,
    ) -> bool {
        false
    }

    fn quote(
        _target_id: &DEXId,
        _input_asset_id: &AssetId,
        _output_asset_id: &AssetId,
        _amount: QuoteAmount<Balance>,
        _deduce_fee: bool,
    ) -> Result<(SwapOutcome<Balance, AssetId>, Weight), DispatchError> {
        Err(DispatchError::CannotLookup)
    }

    fn step_quote(
        _target_id: &DEXId,
        _input_asset_id: &AssetId,
        _output_asset_id: &AssetId,
        _amount: QuoteAmount<Balance>,
        _recommended_samples_count: usize,
        _deduce_fee: bool,
    ) -> Result<(DiscreteQuotation<AssetId, Balance>, Weight), DispatchError> {
        Err(DispatchError::CannotLookup)
    }

    fn exchange(
        _sender: &AccountId,
        _receiver: &AccountId,
        _target_id: &DEXId,
        _input_asset_id: &AssetId,
        _output_asset_id: &AssetId,
        _swap_amount: SwapAmount<Balance>,
    ) -> Result<(SwapOutcome<Balance, AssetId>, Weight), DispatchError> {
        Err(DispatchError::CannotLookup)
    }

    fn check_rewards(
        _target_id: &DEXId,
        _input_asset_id: &AssetId,
        _output_asset_id: &AssetId,
        _input_amount: Balance,
        _output_amount: Balance,
    ) -> Result<(Vec<(Balance, AssetId, RewardReason)>, Weight), DispatchError> {
        Err(DispatchError::CannotLookup)
    }

    fn quote_without_impact(
        _target_id: &DEXId,
        _input_asset_id: &AssetId,
        _output_asset_id: &AssetId,
        _amount: QuoteAmount<Balance>,
        _deduce_fee: bool,
    ) -> Result<SwapOutcome<Balance, AssetId>, DispatchError> {
        Err(DispatchError::CannotLookup)
    }

    fn quote_weight() -> Weight {
        Weight::zero()
    }

    fn step_quote_weight(_samples_count: usize) -> Weight {
        Weight::zero()
    }

    fn exchange_weight() -> Weight {
        Weight::zero()
    }

    fn check_rewards_weight() -> Weight {
        Weight::zero()
    }
}

pub trait LiquidityRegistry<
    DEXId,
    AccountId,
    AssetId: Ord + Clone,
    LiquiditySourceIndex,
    Amount,
    Error,
>:
    LiquiditySource<LiquiditySourceId<DEXId, LiquiditySourceIndex>, AccountId, AssetId, Amount, Error> where
    DEXId: PartialEq + Clone + Copy,
    LiquiditySourceIndex: PartialEq + Clone + Copy,
{
    /// Enumerate available liquidity sources which provide
    /// exchange with for given input->output tokens.
    fn list_liquidity_sources(
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        filter: &LiquiditySourceFilter<DEXId, LiquiditySourceIndex>,
    ) -> Result<Vec<LiquiditySourceId<DEXId, LiquiditySourceIndex>>, Error>;

    fn exchange_weight_filtered(
        enabled_sources: impl Iterator<Item = LiquiditySourceIndex>,
    ) -> Weight;
}

pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
pub type DexIdOf<T> = <T as Config>::DEXId;
pub type AssetIdOf<T> = <<T as Config>::AssetManager as AssetManager<
    T,
    AssetSymbol,
    AssetName,
    BalancePrecision,
    AssetType,
    ContentSource,
    Description,
>>::AssetId;
pub type CurrencyIdOf<T> = <<T as Config>::MultiCurrency as MultiCurrency<
    <T as frame_system::Config>::AccountId,
>>::CurrencyId;
pub type AmountOf<T> = <<T as Config>::MultiCurrency as MultiCurrencyExtended<
    <T as frame_system::Config>::AccountId,
>>::Amount;

pub type GetBaseAssetIdOf<T> = <<T as Config>::AssetManager as AssetManager<
    T,
    AssetSymbol,
    AssetName,
    BalancePrecision,
    AssetType,
    ContentSource,
    Description,
>>::GetBaseAssetId;

pub type BalanceOf<T> = <<T as Config>::MultiCurrency as MultiCurrency<AccountIdOf<T>>>::Balance;

/// Common DEX trait. Used for DEX-related pallets.
pub trait Config: frame_system::Config {
    /// DEX identifier.
    type DEXId: Parameter
        + MaybeSerializeDeserialize
        + Ord
        + Copy
        + Default
        + From<crate::primitives::DEXId>
        + Clone
        + Encode
        + Decode
        + Eq
        + PartialEq
        + MaxEncodedLen;

    type LstId: Clone
        + Copy
        + Encode
        + Decode
        + Eq
        + PartialEq
        + MaxEncodedLen
        + From<crate::primitives::LiquiditySourceType>;

    type AssetManager: AssetManager<
        Self,
        AssetSymbol,
        AssetName,
        BalancePrecision,
        AssetType,
        ContentSource,
        Description,
    >;

    /// Currency to transfer, reserve/unreserve, lock/unlock assets
    type MultiCurrency: MultiLockableCurrency<
            Self::AccountId,
            Moment = Self::BlockNumber,
            CurrencyId = AssetIdOf<Self>,
            Balance = Balance,
        > + MultiReservableCurrency<Self::AccountId, CurrencyId = AssetIdOf<Self>, Balance = Balance>
        + MultiCurrencyExtended<Self::AccountId, Amount = Amount>;
}

/// Definition of a pending atomic swap action. It contains the following three phrases:
///
/// - **Reserve**: reserve the resources needed for a swap. This is to make sure that **Claim**
/// succeeds with best efforts.
/// - **Claim**: claim any resources reserved in the first phrase.
/// - **Cancel**: cancel any resources reserved in the first phrase.
pub trait SwapAction<SourceAccountId, TargetAccountId, AssetId, T: Config> {
    /// Reserve the resources needed for the swap, from the given `source`. The reservation is
    /// allowed to fail. If that is the case, the the full swap creation operation is cancelled.
    fn reserve(&self, source: &SourceAccountId, base_asset_id: &AssetId) -> DispatchResult;
    /// Claim the reserved resources, with `source`. Returns whether the claim succeeds.
    fn claim(&self, source: &SourceAccountId) -> bool;
    /// Weight for executing the operation.
    fn weight(&self) -> Weight;
    /// Cancel the resources reserved in `source`.
    fn cancel(&self, source: &SourceAccountId);
}

/// Dummy implementation for cases then () used in runtime as empty SwapAction.
impl<SourceAccountId, TargetAccountId, AssetId, T: Config>
    SwapAction<SourceAccountId, TargetAccountId, AssetId, T> for ()
{
    fn reserve(&self, _source: &SourceAccountId, _base_asset_id: &AssetId) -> DispatchResult {
        Ok(())
    }
    fn claim(&self, _source: &SourceAccountId) -> bool {
        true
    }
    fn weight(&self) -> Weight {
        unimplemented!()
    }
    fn cancel(&self, _source: &SourceAccountId) {
        unimplemented!()
    }
}

pub trait SwapRulesValidation<SourceAccountId, TargetAccountId, AssetId, T: Config>:
    SwapAction<SourceAccountId, TargetAccountId, AssetId, T>
{
    /// If action is only for abstract checking, shoud not apply by `reserve` function.
    fn is_abstract_checking(&self) -> bool;

    /// Validate action if next steps must be applied by `reserve` function
    /// or if source account is None, than just ability to do operation is checked.
    fn prepare_and_validate(
        &mut self,
        source: Option<&SourceAccountId>,
        base_asset_id: &AssetId,
    ) -> DispatchResult;

    /// Instant auto claim is performed just after reserve.
    /// If triggered is not used, than it is one time auto claim, it will be canceled if it fails.
    fn instant_auto_claim_used(&self) -> bool;

    /// Triggered auto claim can be used for example for crowd like schemes.
    /// for example: when crowd aggregation if succesefull event is fired by consensus, and it is trigger.
    fn triggered_auto_claim_used(&self) -> bool;

    /// Predicate for posibility to claim, timeout for example, or one time for crowd schemes/
    fn is_able_to_claim(&self) -> bool;
}

impl<SourceAccountId, TargetAccountId, AssetId, T: Config>
    SwapRulesValidation<SourceAccountId, TargetAccountId, AssetId, T> for ()
{
    fn is_abstract_checking(&self) -> bool {
        true
    }
    fn prepare_and_validate(
        &mut self,
        _source: Option<&SourceAccountId>,
        _base_asset_id: &AssetId,
    ) -> DispatchResult {
        Ok(())
    }
    fn instant_auto_claim_used(&self) -> bool {
        true
    }
    fn triggered_auto_claim_used(&self) -> bool {
        false
    }
    fn is_able_to_claim(&self) -> bool {
        true
    }
}

pub trait PureOrWrapped<Regular>: From<Regular> + Into<Option<Regular>> {
    /// Not any data is wrapped.
    fn is_pure(&self) -> bool;

    /// The entity is a wrapped `Regular`.
    fn is_wrapped_regular(&self) -> bool;

    /// The entity is wrapped.
    fn is_wrapped(&self) -> bool;
}

pub trait IsRepresentation {
    fn is_representation(&self) -> bool;
}

pub trait WrappedRepr<Repr> {
    fn wrapped_repr(repr: Repr) -> Self;
}

pub trait IsRepresentable<A>: PureOrWrapped<A> {
    /// The entity can be represented or already represented.
    fn is_representable(&self) -> bool;
}

/// This is default generic implementation for IsRepresentable trait.
impl<A, B> IsRepresentable<A> for B
where
    B: PureOrWrapped<A> + IsRepresentation,
{
    fn is_representable(&self) -> bool {
        self.is_pure() || self.is_representation()
    }
}

pub trait ToFeeAccount: Sized {
    fn to_fee_account(&self) -> Option<Self>;
}

pub trait ToXykTechUnitFromDEXAndTradingPair<DEXId, TradingPair>: Sized {
    fn to_xyk_tech_unit_from_dex_and_trading_pair(dex_id: DEXId, trading_pair: TradingPair)
        -> Self;
}

pub trait ToOrderTechUnitFromDEXAndTradingPair<DEXId, TradingPair>: Sized {
    fn to_order_tech_unit_from_dex_and_trading_pair(
        dex_id: DEXId,
        trading_pair: TradingPair,
    ) -> Self;
}

/// PureOrWrapped is reflexive.
impl<A> PureOrWrapped<A> for A {
    fn is_pure(&self) -> bool {
        false
    }
    fn is_wrapped_regular(&self) -> bool {
        true
    }
    fn is_wrapped(&self) -> bool {
        true
    }
}

/// Abstract trait to get data type from generic pair name and data.
pub trait FromGenericPair {
    fn from_generic_pair(tag: Vec<u8>, data: Vec<u8>) -> Self;
}

/// Trait for bounding liquidity proxy associated type representing primary market in TBC.
pub trait GetMarketInfo<AssetId> {
    /// The price in terms of the `target_asset` at which one can buy
    /// a unit of the `base_asset` on the primary market (e.g. from the bonding curve pool or xst).
    fn buy_price(base_asset: &AssetId, target_asset: &AssetId) -> Result<Fixed, DispatchError>;
    /// The price in terms of the `target_asset` at which one can sell
    /// a unit of the `base_asset` on the primary market (e.g. to the bonding curve pool or xst).
    fn sell_price(base_asset: &AssetId, target_asset: &AssetId) -> Result<Fixed, DispatchError>;
    /// Returns set of enabled collateral/synthetic/reserve assets on bonding curve.
    fn enabled_target_assets() -> BTreeSet<AssetId>;
}

impl<AssetId: Ord> GetMarketInfo<AssetId> for () {
    fn buy_price(
        _base_asset: &AssetId,
        _collateral_asset: &AssetId,
    ) -> Result<Fixed, DispatchError> {
        Ok(Default::default())
    }

    fn sell_price(
        _base_asset: &AssetId,
        _collateral_asset: &AssetId,
    ) -> Result<Fixed, DispatchError> {
        Ok(Default::default())
    }

    fn enabled_target_assets() -> BTreeSet<AssetId> {
        Default::default()
    }
}

/// Trait for bounding liquidity proxy associated type representing secondary market.
pub trait GetPoolReserves<AssetId> {
    /// Returns the amount of the `(base_asset, other_asset)` pair reserves in a liquidity pool
    /// or the default value if such pair doesn't exist.
    fn reserves(base_asset: &AssetId, other_asset: &AssetId) -> (Balance, Balance);
}

impl<AssetId> GetPoolReserves<AssetId> for () {
    fn reserves(_base_asset: &AssetId, _other_asset: &AssetId) -> (Balance, Balance) {
        Default::default()
    }
}

/// General trait for passing pswap amount burned information to required pallets.
pub trait OnPswapBurned {
    /// Report amount and fractions of burned pswap at the moment of invocation.
    fn on_pswap_burned(distribution: PswapRemintInfo);
}

impl OnPswapBurned for () {
    fn on_pswap_burned(_distribution: PswapRemintInfo) {
        // do nothing
    }
}

/// Trait to abstract interface of VestedRewards pallet, in order for pallets with rewards sources avoid having dependency issues.
pub trait Vesting<AccountId, AssetId> {
    /// Report that account has received pswap reward for buying from tbc.
    fn add_tbc_reward(account_id: &AccountId, pswap_amount: Balance) -> DispatchResult;

    /// Report that account has received farmed pswap reward for providing liquidity on secondary market.
    fn add_farming_reward(account_id: &AccountId, pswap_amount: Balance) -> DispatchResult;
}

pub trait XykPool<AccountId, AssetId> {
    type PoolProvidersOutput: IntoIterator<Item = (AccountId, Balance)>;
    type PoolPropertiesOutput: IntoIterator<Item = (AssetId, AssetId, (AccountId, AccountId))>;

    fn pool_providers(pool_account: &AccountId) -> Self::PoolProvidersOutput;

    fn total_issuance(pool_account: &AccountId) -> Result<Balance, DispatchError>;

    fn all_properties() -> Self::PoolPropertiesOutput;

    fn properties_of_pool(
        _base_asset_id: AssetId,
        _target_asset_id: AssetId,
    ) -> Option<(AccountId, AccountId)> {
        None
    }

    fn balance_of_pool_provider(
        _pool_account: AccountId,
        _liquidity_provider_account: AccountId,
    ) -> Option<Balance> {
        None
    }

    fn transfer_lp_tokens(
        _pool_account: AccountId,
        _asset_a: AssetId,
        _asset_b: AssetId,
        _base_account_id: AccountId,
        _target_account_id: AccountId,
        _pool_tokens: Balance,
    ) -> Result<(), DispatchError> {
        Err(DispatchError::CannotLookup)
    }
}

pub trait DemeterFarming<AccountId, AssetId> {
    fn update_pool_tokens(
        _user: AccountId,
        _pool_tokens: Balance,
        _base_asset: AssetId,
        _pool_asset: AssetId,
    ) -> Result<(), DispatchError> {
        Err(DispatchError::CannotLookup)
    }
}

pub trait OnPoolCreated {
    type AccountId;
    type DEXId;

    fn on_pool_created(
        fee_account: Self::AccountId,
        dex_id: Self::DEXId,
        pool_account: Self::AccountId,
    ) -> DispatchResult;
}

pub trait PriceToolsProvider<AssetId> {
    /// Checks if asset is registered in PriceTools.
    fn is_asset_registered(asset_id: &AssetId) -> bool;

    /// Get amount of `output_asset_id` corresponding to a unit (1) of `input_asset_id`.
    /// `price_variant` specifies the correction for price, either for buy or sell.
    fn get_average_price(
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        price_variant: PriceVariant,
    ) -> Result<Balance, DispatchError>;

    /// Add asset to be tracked for average price.
    fn register_asset(asset_id: &AssetId) -> DispatchResult;
}

impl<AssetId> PriceToolsProvider<AssetId> for () {
    fn is_asset_registered(_asset_id: &AssetId) -> bool {
        unimplemented!()
    }

    fn get_average_price(
        _: &AssetId,
        _: &AssetId,
        _: PriceVariant,
    ) -> Result<Balance, DispatchError> {
        unimplemented!()
    }

    fn register_asset(_: &AssetId) -> DispatchResult {
        unimplemented!()
    }
}

impl<AccountId, DEXId, A, B> OnPoolCreated for (A, B)
where
    AccountId: Clone,
    DEXId: Clone,
    A: OnPoolCreated<AccountId = AccountId, DEXId = DEXId>,
    B: OnPoolCreated<AccountId = AccountId, DEXId = DEXId>,
{
    type AccountId = AccountId;
    type DEXId = DEXId;

    fn on_pool_created(
        fee_account: Self::AccountId,
        dex_id: Self::DEXId,
        pool_account: Self::AccountId,
    ) -> DispatchResult {
        A::on_pool_created(fee_account.clone(), dex_id.clone(), pool_account.clone())?;
        B::on_pool_created(fee_account, dex_id, pool_account)
    }
}

pub trait OnPoolReservesChanged<AssetId> {
    // Reserves of given pool has either changed proportion or volume.
    fn reserves_changed(target_asset_id: &AssetId);
}

impl<AssetId> OnPoolReservesChanged<AssetId> for () {
    fn reserves_changed(_: &AssetId) {
        // do nothing
    }
}

/// General trait for passing on the amount of burned VAL.
pub trait OnValBurned {
    /// Report amount and fractions of burned pswap at the moment of invokation.
    fn on_val_burned(amount: Balance);
}

impl OnValBurned for () {
    fn on_val_burned(_: Balance) {
        // do nothing
    }
}

/// Indicates that particular object can be used to perform exchanges with aggregation capability.
pub trait LiquidityProxyTrait<DEXId: PartialEq + Copy, AccountId, AssetId: Ord> {
    /// Get spot price of tokens based on desired amount, None returned if liquidity source
    /// does not have available exchange methods for indicated path.
    fn quote(
        dex_id: DEXId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        amount: QuoteAmount<Balance>,
        filter: LiquiditySourceFilter<DEXId, LiquiditySourceType>,
        deduce_fee: bool,
    ) -> Result<SwapOutcome<Balance, AssetId>, DispatchError>;

    /// Perform exchange based on desired amount.
    fn exchange(
        dex_id: DEXId,
        sender: &AccountId,
        receiver: &AccountId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        amount: SwapAmount<Balance>,
        filter: LiquiditySourceFilter<DEXId, LiquiditySourceType>,
    ) -> Result<SwapOutcome<Balance, AssetId>, DispatchError>;
}

impl<DEXId: PartialEq + Copy, AccountId, AssetId: Ord>
    LiquidityProxyTrait<DEXId, AccountId, AssetId> for ()
{
    fn quote(
        _dex_id: DEXId,
        _input_asset_id: &AssetId,
        _output_asset_id: &AssetId,
        _amount: QuoteAmount<Balance>,
        _filter: LiquiditySourceFilter<DEXId, LiquiditySourceType>,
        _deduce_fee: bool,
    ) -> Result<SwapOutcome<Balance, AssetId>, DispatchError> {
        unimplemented!()
    }

    fn exchange(
        _dex_id: DEXId,
        _sender: &AccountId,
        _receiver: &AccountId,
        _input_asset_id: &AssetId,
        _output_asset_id: &AssetId,
        _amount: SwapAmount<Balance>,
        _filter: LiquiditySourceFilter<DEXId, LiquiditySourceType>,
    ) -> Result<SwapOutcome<Balance, AssetId>, DispatchError> {
        unimplemented!()
    }
}

/// Trait to provide DEXInfo
pub trait DexInfoProvider<
    DEXId: Eq + PartialEq + Copy + Clone + PartialOrd + Ord,
    DEXInfo: Clone + PartialEq + Eq + Default,
>
{
    fn get_dex_info(dex_id: &DEXId) -> Result<DEXInfo, DispatchError>;

    fn ensure_dex_exists(dex_id: &DEXId) -> DispatchResult;

    fn list_dex_ids() -> Vec<DEXId>;
}

impl<
        DEXId: Eq + PartialEq + Copy + Clone + PartialOrd + Ord,
        DEXInfo: Clone + PartialEq + Eq + Default,
    > DexInfoProvider<DEXId, DEXInfo> for ()
{
    fn get_dex_info(_dex_id: &DEXId) -> Result<DEXInfo, DispatchError> {
        unimplemented!()
    }

    fn ensure_dex_exists(_dex_id: &DEXId) -> DispatchResult {
        unimplemented!()
    }

    fn list_dex_ids() -> Vec<DEXId> {
        unimplemented!()
    }
}

/// Trait to provide info about assets
pub trait AssetInfoProvider<
    AssetId,
    AccountId,
    AssetSymbol,
    AssetName,
    BalancePrecision,
    ContentSource,
    Description,
>
{
    fn asset_exists(asset_id: &AssetId) -> bool;

    fn ensure_asset_exists(asset_id: &AssetId) -> DispatchResult;

    fn is_asset_owner(asset_id: &AssetId, account_id: &AccountId) -> bool;

    fn get_asset_info(
        asset_id: &AssetId,
    ) -> (
        AssetSymbol,
        AssetName,
        BalancePrecision,
        bool,
        Option<ContentSource>,
        Option<Description>,
    );

    fn is_non_divisible(asset_id: &AssetId) -> bool;

    fn get_asset_content_src(asset_id: &AssetId) -> Option<ContentSource>;

    fn get_asset_description(asset_id: &AssetId) -> Option<Description>;

    fn total_issuance(asset_id: &AssetId) -> Result<Balance, DispatchError>;

    fn total_balance(asset_id: &AssetId, who: &AccountId) -> Result<Balance, DispatchError>;

    fn free_balance(asset_id: &AssetId, who: &AccountId) -> Result<Balance, DispatchError>;

    fn ensure_can_withdraw(asset_id: &AssetId, who: &AccountId, amount: Balance) -> DispatchResult;

    fn get_asset_owner(asset_id: &AssetId) -> Result<AccountId, DispatchError>;
}

impl<AssetId, AccountId, AssetSymbol, AssetName, BalancePrecision, ContentSource, Description>
    AssetInfoProvider<
        AssetId,
        AccountId,
        AssetSymbol,
        AssetName,
        BalancePrecision,
        ContentSource,
        Description,
    > for ()
{
    fn asset_exists(_asset_id: &AssetId) -> bool {
        unimplemented!()
    }

    fn ensure_asset_exists(_asset_id: &AssetId) -> DispatchResult {
        unimplemented!()
    }

    fn is_asset_owner(_asset_id: &AssetId, _account_id: &AccountId) -> bool {
        unimplemented!()
    }

    fn get_asset_info(
        _asset_id: &AssetId,
    ) -> (
        AssetSymbol,
        AssetName,
        BalancePrecision,
        bool,
        Option<ContentSource>,
        Option<Description>,
    ) {
        unimplemented!()
    }

    fn is_non_divisible(_asset_id: &AssetId) -> bool {
        unimplemented!()
    }

    fn get_asset_content_src(_asset_id: &AssetId) -> Option<ContentSource> {
        unimplemented!()
    }

    fn get_asset_description(_asset_id: &AssetId) -> Option<Description> {
        unimplemented!()
    }

    fn total_balance(_asset_id: &AssetId, _who: &AccountId) -> Result<Balance, DispatchError> {
        unimplemented!()
    }

    fn total_issuance(_asset_id: &AssetId) -> Result<Balance, DispatchError> {
        unimplemented!()
    }

    fn free_balance(_asset_id: &AssetId, _who: &AccountId) -> Result<Balance, DispatchError> {
        unimplemented!()
    }

    fn ensure_can_withdraw(
        _asset_id: &AssetId,
        _who: &AccountId,
        _amount: Balance,
    ) -> DispatchResult {
        unimplemented!()
    }

    fn get_asset_owner(_asset_id: &AssetId) -> Result<AccountId, DispatchError> {
        unimplemented!()
    }
}

pub trait AssetManager<
    T: Config,
    AssetSymbol,
    AssetName,
    BalancePrecision,
    AssetType,
    ContentSource,
    Description,
>
{
    type AssetId: Parameter
        + Member
        + Copy
        + MaybeSerializeDeserialize
        + Ord
        + Default
        + Clone
        + From<AssetId32<PredefinedAssetId>>
        + From<H256>
        + Into<H256>
        + Into<CurrencyIdOf<T>>
        + MaxEncodedLen;

    type GetBaseAssetId: Get<Self::AssetId>;

    #[allow(clippy::too_many_arguments)]
    fn register_from(
        account_id: &T::AccountId,
        symbol: AssetSymbol,
        name: AssetName,
        precision: BalancePrecision,
        initial_supply: Balance,
        is_mintable: bool,
        asset_type: AssetType,
        opt_content_src: Option<ContentSource>,
        opt_desc: Option<Description>,
    ) -> Result<Self::AssetId, DispatchError>;

    fn update_balance(
        origin: OriginFor<T>,
        who: T::AccountId,
        currency_id: CurrencyIdOf<T>,
        amount: AmountOf<T>,
    ) -> DispatchResult;

    fn gen_asset_id(account_id: &T::AccountId) -> Self::AssetId;

    fn gen_asset_id_from_any(value: &impl Encode) -> Self::AssetId;

    #[allow(clippy::too_many_arguments)]
    fn register_asset_id(
        account_id: T::AccountId,
        asset_id: Self::AssetId,
        symbol: AssetSymbol,
        name: AssetName,
        precision: BalancePrecision,
        initial_supply: Balance,
        is_mintable: bool,
        asset_type: AssetType,
        opt_content_src: Option<ContentSource>,
        opt_desc: Option<Description>,
    ) -> DispatchResult;

    fn burn_from(
        asset_id: &Self::AssetId,
        issuer: &T::AccountId,
        from: &T::AccountId,
        amount: Balance,
    ) -> DispatchResult;

    fn transfer_from(
        asset_id: &Self::AssetId,
        from: &T::AccountId,
        to: &T::AccountId,
        amount: Balance,
    ) -> DispatchResult;

    fn mint_to(
        asset_id: &Self::AssetId,
        issuer: &T::AccountId,
        to: &T::AccountId,
        amount: Balance,
    ) -> DispatchResult;

    fn mint_unchecked(
        asset_id: &Self::AssetId,
        to: &T::AccountId,
        amount: Balance,
    ) -> DispatchResult;

    fn burn(
        origin: OriginFor<T>,
        asset_id: Self::AssetId,
        amount: Balance,
    ) -> DispatchResultWithPostInfo;

    fn mint(
        origin: OriginFor<T>,
        asset_id: Self::AssetId,
        to: T::AccountId,
        amount: Balance,
    ) -> DispatchResultWithPostInfo;

    #[allow(clippy::too_many_arguments)]
    fn register(
        origin: OriginFor<T>,
        symbol: AssetSymbol,
        name: AssetName,
        initial_supply: Balance,
        is_mintable: bool,
        is_indivisible: bool,
        opt_content_src: Option<ContentSource>,
        opt_desc: Option<Description>,
    ) -> DispatchResultWithPostInfo;

    fn update_asset_type(asset_id: &Self::AssetId, asset_type: &AssetType) -> DispatchResult;
}

impl<
        T: Config,
        AssetSymbol,
        AssetName,
        BalancePrecision,
        AssetType,
        ContentSource,
        Description,
    >
    AssetManager<T, AssetSymbol, AssetName, BalancePrecision, AssetType, ContentSource, Description>
    for ()
{
    type AssetId = AssetId32<PredefinedAssetId>;
    type GetBaseAssetId = ();

    fn register_from(
        _account_id: &T::AccountId,
        _symbol: AssetSymbol,
        _name: AssetName,
        _precision: BalancePrecision,
        _initial_supply: Balance,
        _is_mintable: bool,
        _asset_type: AssetType,
        _opt_content_src: Option<ContentSource>,
        _opt_desc: Option<Description>,
    ) -> Result<Self::AssetId, DispatchError> {
        unimplemented!()
    }

    fn update_balance(
        _origin: OriginFor<T>,
        _who: T::AccountId,
        _currency_id: CurrencyIdOf<T>,
        _amount: AmountOf<T>,
    ) -> DispatchResult {
        unimplemented!()
    }

    fn gen_asset_id(_account_id: &<T>::AccountId) -> Self::AssetId {
        unimplemented!()
    }

    fn gen_asset_id_from_any(_value: &impl Encode) -> Self::AssetId {
        unimplemented!()
    }

    fn register_asset_id(
        _account_id: T::AccountId,
        _asset_id: Self::AssetId,
        _symbol: AssetSymbol,
        _name: AssetName,
        _precision: BalancePrecision,
        _initial_supply: Balance,
        _is_mintable: bool,
        _asset_type: AssetType,
        _opt_content_src: Option<ContentSource>,
        _opt_desc: Option<Description>,
    ) -> DispatchResult {
        unimplemented!()
    }

    fn burn_from(
        _asset_id: &Self::AssetId,
        _issuer: &<T as frame_system::Config>::AccountId,
        _from: &<T as frame_system::Config>::AccountId,
        _amount: Balance,
    ) -> DispatchResult {
        unimplemented!()
    }

    fn transfer_from(
        _asset_id: &Self::AssetId,
        _from: &<T as frame_system::Config>::AccountId,
        _to: &<T as frame_system::Config>::AccountId,
        _amount: Balance,
    ) -> DispatchResult {
        unimplemented!()
    }

    fn mint_to(
        _asset_id: &Self::AssetId,
        _issuer: &<T as frame_system::Config>::AccountId,
        _to: &<T as frame_system::Config>::AccountId,
        _amount: Balance,
    ) -> DispatchResult {
        unimplemented!()
    }

    fn mint_unchecked(
        _asset_id: &Self::AssetId,
        _to: &T::AccountId,
        _amount: Balance,
    ) -> DispatchResult {
        unimplemented!()
    }

    fn burn(
        _origin: OriginFor<T>,
        _asset_id: Self::AssetId,
        _amount: Balance,
    ) -> DispatchResultWithPostInfo {
        unimplemented!()
    }

    fn mint(
        _origin: OriginFor<T>,
        _asset_id: Self::AssetId,
        _to: <T as frame_system::Config>::AccountId,
        _amount: Balance,
    ) -> DispatchResultWithPostInfo {
        unimplemented!()
    }

    fn register(
        _origin: OriginFor<T>,
        _symbol: AssetSymbol,
        _name: AssetName,
        _initial_supply: Balance,
        _is_mintable: bool,
        _is_indivisible: bool,
        _opt_content_src: Option<ContentSource>,
        _opt_desc: Option<Description>,
    ) -> DispatchResultWithPostInfo {
        unimplemented!()
    }

    fn update_asset_type(_asset_id: &Self::AssetId, _asset_type: &AssetType) -> DispatchResult {
        unimplemented!()
    }
}

pub trait SyntheticInfoProvider<AssetId> {
    fn is_synthetic(asset_id: &AssetId) -> bool;

    fn get_synthetic_assets() -> Vec<AssetId>;
}

impl<AssetId> SyntheticInfoProvider<AssetId> for () {
    fn is_synthetic(_asset_id: &AssetId) -> bool {
        unimplemented!()
    }

    fn get_synthetic_assets() -> Vec<AssetId> {
        unimplemented!()
    }
}

pub trait IsValid {
    fn is_valid(&self) -> bool;
}

pub trait BuyBackHandler<AccountId, AssetId> {
    /// Mint `amount` of `mint_asset_id`, exchange to `buy_back_asset_id` and burn result amount
    ///
    /// Returns burned amount
    fn mint_buy_back_and_burn(
        mint_asset_id: &AssetId,
        buy_back_asset_id: &AssetId,
        amount: Balance,
    ) -> Result<Balance, DispatchError>;

    /// Exchange `amount` of `asset_id` from `account_id` to `buy_back_asset_id` and burn result amount
    ///
    /// Returns burned amount
    fn buy_back_and_burn(
        account_id: &AccountId,
        asset_id: &AssetId,
        buy_back_asset_id: &AssetId,
        amount: Balance,
    ) -> Result<Balance, DispatchError>;
}

impl<AssetId, AccountId> BuyBackHandler<AccountId, AssetId> for () {
    fn mint_buy_back_and_burn(
        _mint_asset_id: &AssetId,
        _buy_back_asset_id: &AssetId,
        _amount: Balance,
    ) -> Result<Balance, DispatchError> {
        Ok(0)
    }

    fn buy_back_and_burn(
        _account_id: &AccountId,
        _asset_id: &AssetId,
        _buy_back_asset_id: &AssetId,
        _amount: Balance,
    ) -> Result<Balance, DispatchError> {
        Ok(0)
    }
}

pub trait ReferencePriceProvider<AssetId, Balance> {
    fn get_reference_price(asset_id: &AssetId) -> Result<Balance, DispatchError>;
}

pub trait ReferrerAccountProvider<AccountId> {
    fn get_referrer_account(who: &AccountId) -> Option<AccountId>;
}

impl<AccountId> ReferrerAccountProvider<AccountId> for () {
    fn get_referrer_account(_who: &AccountId) -> Option<AccountId> {
        None
    }
}

/// Trait to manage permissions/regulations for assets operations
pub trait AssetRegulator<AccountId, AssetId> {
    /// Assign `permission_id` for a specific `account_id` to a specific `asset_id`
    fn assign_permission(
        owner: &AccountId,
        asset_id: &AssetId,
        permission_id: &PermissionId,
    ) -> Result<(), DispatchError>;

    /// Check the permission `permission_id` of `issuer` for `asset_id`
    /// with respect to `affected_account`
    fn check_permission(
        issuer: &AccountId,
        affected_account: &AccountId,
        asset_id: &AssetId,
        permission_id: &PermissionId,
    ) -> Result<(), DispatchError>;
}

impl<AccountId, AssetId, A, B> AssetRegulator<AccountId, AssetId> for (A, B)
where
    A: AssetRegulator<AccountId, AssetId>,
    B: AssetRegulator<AccountId, AssetId>,
{
    fn assign_permission(
        owner: &AccountId,
        asset_id: &AssetId,
        permission_id: &PermissionId,
    ) -> Result<(), DispatchError> {
        A::assign_permission(owner, asset_id, permission_id)?;
        B::assign_permission(owner, asset_id, permission_id)?;
        Ok(())
    }

    fn check_permission(
        issuer: &AccountId,
        affected_account: &AccountId,
        asset_id: &AssetId,
        permission_id: &PermissionId,
    ) -> Result<(), DispatchError> {
        A::check_permission(issuer, affected_account, asset_id, permission_id)?;
        B::check_permission(issuer, affected_account, asset_id, permission_id)?;
        Ok(())
    }
}

impl<AccountId, AssetId> AssetRegulator<AccountId, AssetId> for () {
    fn assign_permission(
        _owner: &AccountId,
        _asset_id: &AssetId,
        _permission_id: &PermissionId,
    ) -> Result<(), DispatchError> {
        Ok(())
    }

    fn check_permission(
        _issuer: &AccountId,
        _affected_account: &AccountId,
        _asset_id: &AssetId,
        _permission_id: &PermissionId,
    ) -> Result<(), DispatchError> {
        Ok(())
    }
}
