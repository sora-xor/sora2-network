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

use crate::prelude::{ManagementMode, SwapAmount, SwapOutcome};
use crate::{Fixed, LiquiditySourceFilter, LiquiditySourceId, PswapRemintInfo, RewardReason};
use frame_support::dispatch::DispatchResult;
use frame_support::pallet_prelude::MaybeSerializeDeserialize;
use frame_support::sp_runtime::traits::BadOrigin;
use frame_support::sp_runtime::DispatchError;
use frame_support::weights::Weight;
use frame_support::Parameter;
use frame_system::RawOrigin;
//FIXME maybe try info or try from is better than From and Option.
//use sp_std::convert::TryInto;
use crate::primitives::Balance;
use codec::{Decode, Encode};
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

/// Indicates that particular object can be used to perform exchanges.
pub trait LiquiditySource<TargetId, AccountId, AssetId, Amount, Error> {
    /// Check if liquidity source provides an exchange from given input asset to output asset.
    fn can_exchange(
        target_id: &TargetId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
    ) -> bool;

    /// Get spot price of tokens based on desired amount, None returned if liquidity source
    /// does not have available exchange methods for indicated path.
    fn quote(
        target_id: &TargetId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        swap_amount: SwapAmount<Amount>,
    ) -> Result<SwapOutcome<Amount>, DispatchError>;

    /// Perform exchange based on desired amount.
    fn exchange(
        sender: &AccountId,
        receiver: &AccountId,
        target_id: &TargetId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        swap_amount: SwapAmount<Amount>,
    ) -> Result<SwapOutcome<Amount>, DispatchError>;

    /// Get rewards that are given for perfoming given exchange.
    fn check_rewards(
        target_id: &TargetId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        input_amount: Amount,
        output_amount: Amount,
    ) -> Result<Vec<(Amount, AssetId, RewardReason)>, DispatchError>;
}

impl<DEXId, AccountId, AssetId> LiquiditySource<DEXId, AccountId, AssetId, Fixed, DispatchError>
    for ()
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
        _swap_amount: SwapAmount<Fixed>,
    ) -> Result<SwapOutcome<Fixed>, DispatchError> {
        Err(DispatchError::CannotLookup)
    }

    fn exchange(
        _sender: &AccountId,
        _receiver: &AccountId,
        _target_id: &DEXId,
        _input_asset_id: &AssetId,
        _output_asset_id: &AssetId,
        _swap_amount: SwapAmount<Fixed>,
    ) -> Result<SwapOutcome<Fixed>, DispatchError> {
        Err(DispatchError::CannotLookup)
    }

    fn check_rewards(
        _target_id: &DEXId,
        _input_asset_id: &AssetId,
        _output_asset_id: &AssetId,
        _input_amount: Fixed,
        _output_amount: Fixed,
    ) -> Result<Vec<(Fixed, AssetId, RewardReason)>, DispatchError> {
        Err(DispatchError::CannotLookup)
    }
}

impl<DEXId, AccountId, AssetId> LiquiditySource<DEXId, AccountId, AssetId, Balance, DispatchError>
    for ()
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
        _swap_amount: SwapAmount<Balance>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        Err(DispatchError::CannotLookup)
    }

    fn exchange(
        _sender: &AccountId,
        _receiver: &AccountId,
        _target_id: &DEXId,
        _input_asset_id: &AssetId,
        _output_asset_id: &AssetId,
        _swap_amount: SwapAmount<Balance>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        Err(DispatchError::CannotLookup)
    }

    fn check_rewards(
        _target_id: &DEXId,
        _input_asset_id: &AssetId,
        _output_asset_id: &AssetId,
        _input_amount: Balance,
        _output_amount: Balance,
    ) -> Result<Vec<(Balance, AssetId, RewardReason)>, DispatchError> {
        Err(DispatchError::CannotLookup)
    }
}

pub trait LiquidityRegistry<DEXId, AccountId, AssetId, LiquiditySourceIndex, Amount, Error>:
    LiquiditySource<LiquiditySourceId<DEXId, LiquiditySourceIndex>, AccountId, AssetId, Amount, Error>
where
    DEXId: PartialEq + Clone + Copy,
    LiquiditySourceIndex: PartialEq + Clone + Copy,
{
    /// Enumerate available liquidity sources which provide
    /// exchange with for given input->output tokens.
    fn list_liquidity_sources(
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        filter: LiquiditySourceFilter<DEXId, LiquiditySourceIndex>,
    ) -> Result<Vec<LiquiditySourceId<DEXId, LiquiditySourceIndex>>, Error>;
}

pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
pub type DexIdOf<T> = <T as Config>::DEXId;

/// Common DEX trait. Used for DEX-related pallets.
pub trait Config: frame_system::Config + currencies::Config {
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
        + PartialEq;
    type LstId: Clone
        + Copy
        + Encode
        + Decode
        + Eq
        + PartialEq
        + From<crate::primitives::LiquiditySourceType>;
}

/// Definition of a pending atomic swap action. It contains the following three phrases:
///
/// - **Reserve**: reserve the resources needed for a swap. This is to make sure that **Claim**
/// succeeds with best efforts.
/// - **Claim**: claim any resources reserved in the first phrase.
/// - **Cancel**: cancel any resources reserved in the first phrase.
pub trait SwapAction<SourceAccountId, TargetAccountId, T: Config> {
    /// Reserve the resources needed for the swap, from the given `source`. The reservation is
    /// allowed to fail. If that is the case, the the full swap creation operation is cancelled.
    fn reserve(&self, source: &SourceAccountId) -> DispatchResult;
    /// Claim the reserved resources, with `source`. Returns whether the claim succeeds.
    fn claim(&self, source: &SourceAccountId) -> bool;
    /// Weight for executing the operation.
    fn weight(&self) -> Weight;
    /// Cancel the resources reserved in `source`.
    fn cancel(&self, source: &SourceAccountId);
}

/// Dummy implementation for cases then () used in runtime as empty SwapAction.
impl<SourceAccountId, TargetAccountId, T: Config> SwapAction<SourceAccountId, TargetAccountId, T>
    for ()
{
    fn reserve(&self, _source: &SourceAccountId) -> DispatchResult {
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

pub trait SwapRulesValidation<SourceAccountId, TargetAccountId, T: Config>:
    SwapAction<SourceAccountId, TargetAccountId, T>
{
    /// If action is only for abstract checking, shoud not apply by `reserve` function.
    fn is_abstract_checking(&self) -> bool;

    /// Validate action if next steps must be applied by `reserve` function
    /// or if source account is None, than just ability to do operation is checked.
    fn prepare_and_validate(&mut self, source: Option<&SourceAccountId>) -> DispatchResult;

    /// Instant auto claim is performed just after reserve.
    /// If triggered is not used, than it is one time auto claim, it will be canceled if it fails.
    fn instant_auto_claim_used(&self) -> bool;

    /// Triggered auto claim can be used for example for crowd like schemes.
    /// for example: when crowd aggregation if succesefull event is fired by consensus, and it is trigger.
    fn triggered_auto_claim_used(&self) -> bool;

    /// Predicate for posibility to claim, timeout for example, or one time for crowd schemes/
    fn is_able_to_claim(&self) -> bool;
}

impl<SourceAccountId, TargetAccountId, T: Config>
    SwapRulesValidation<SourceAccountId, TargetAccountId, T> for ()
{
    fn is_abstract_checking(&self) -> bool {
        true
    }
    fn prepare_and_validate(&mut self, _source: Option<&SourceAccountId>) -> DispatchResult {
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

pub trait ToMarkerAsset<TechAssetId, LstId>: Sized {
    fn to_marker_asset(&self, lst_id: LstId) -> Option<TechAssetId>;
}

pub trait GetTechAssetWithLstTag<LstId, AssetId>: Sized {
    fn get_tech_asset_with_lst_tag(tag: LstId, asset_id: AssetId) -> Result<Self, ()>;
}

pub trait GetLstIdAndTradingPairFromTechAsset<LstId, TradingPair> {
    fn get_lst_id_and_trading_pair_from_tech_asset(&self) -> Option<(LstId, TradingPair)>;
}

pub trait ToTechUnitFromDEXAndAsset<DEXId, AssetId>: Sized {
    fn to_tech_unit_from_dex_and_asset(dex_id: DEXId, asset_id: AssetId) -> Self;
}

pub trait ToTechUnitFromDEXAndTradingPair<DEXId, TradingPair>: Sized {
    fn to_tech_unit_from_dex_and_trading_pair(dex_id: DEXId, trading_pair: TradingPair) -> Self;
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

/// Trait for bounding liquidity proxy associated type representing primary market.
pub trait GetMarketInfo<AssetId> {
    /// The price in terms of the `collateral_asset` at which one can buy
    /// a unit of the `base_asset` on the primary market (e.g. from the bonding curve pool).
    fn buy_price(base_asset: &AssetId, collateral_asset: &AssetId) -> Result<Fixed, DispatchError>;
    /// The price in terms of the `collateral_asset` at which one can sell
    /// a unit of the `base_asset` on the primary market (e.g. to the bonding curve pool).
    fn sell_price(base_asset: &AssetId, collateral_asset: &AssetId)
        -> Result<Fixed, DispatchError>;
    /// The amount of the `asset_id` token reserves stored with the primary market liquidity provider
    /// (a multi-collateral bonding curve pool) that backs a part of the base currency in circulation.
    fn collateral_reserves(asset_id: &AssetId) -> Result<Balance, DispatchError>;
    /// Returns set of enabled collateral/reserve assets on bonding curve.
    fn enabled_collaterals() -> BTreeSet<AssetId>;
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

    fn collateral_reserves(_asset_id: &AssetId) -> Result<Balance, DispatchError> {
        Ok(Default::default())
    }

    fn enabled_collaterals() -> BTreeSet<AssetId> {
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
    /// Report amount and fractions of burned pswap at the moment of invokation.
    fn on_pswap_burned(distribution: PswapRemintInfo);
}

impl OnPswapBurned for () {
    fn on_pswap_burned(_distribution: PswapRemintInfo) {
        // do nothing
    }
}

/// Trait to abstract interface of VestedRewards pallet, in order for pallets with rewards sources avoid having dependency issues.
pub trait VestedRewardsPallet<AccountId> {
    /// Report that swaps with xor were performed.
    /// - `account_id`: account performing transaction.
    /// - `xor_volume`: amount of xor passed in transaction.
    /// - `count`: number of equal swaps, if there are multiple - means that each has amount equal to `xor_volume`.
    fn update_market_maker_records(
        account_id: &AccountId,
        xor_volume: Balance,
        count: u32,
    ) -> DispatchResult;

    /// Report that account has received pswap reward for buying from tbc.
    fn add_tbc_reward(account_id: &AccountId, pswap_amount: Balance) -> DispatchResult;

    /// Report that account has received farmed pswap reward for providing liquidity on secondary market.
    fn add_farming_reward(account_id: &AccountId, pswap_amount: Balance) -> DispatchResult;

    /// Report that account has received pswap reward for performing large volume trade over month.
    fn add_market_maker_reward(account_id: &AccountId, pswap_amount: Balance) -> DispatchResult;
}

pub trait PoolXykPallet {
    type AccountId;
    type AssetId;
    type PoolProvidersOutput: IntoIterator<Item = (Self::AccountId, Balance)>;
    type PoolPropertiesOutput: IntoIterator<
        Item = (
            Self::AssetId,
            Self::AssetId,
            (Self::AccountId, Self::AccountId),
        ),
    >;

    fn pool_providers(pool_account: &Self::AccountId) -> Self::PoolProvidersOutput;

    fn total_issuance(pool_account: &Self::AccountId) -> Result<Balance, DispatchError>;

    fn all_properties() -> Self::PoolPropertiesOutput;
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

pub trait PriceToolsPallet<AssetId> {
    /// Get amount of `output_asset_id` corresponding to a unit (1) of `input_asset_id`.
    fn get_average_price(
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
    ) -> Result<Balance, DispatchError>;

    /// Add asset to be tracked for average price.
    fn register_asset(asset_id: &AssetId) -> DispatchResult;
}

impl<AssetId> PriceToolsPallet<AssetId> for () {
    fn get_average_price(_: &AssetId, _: &AssetId) -> Result<Balance, DispatchError> {
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
