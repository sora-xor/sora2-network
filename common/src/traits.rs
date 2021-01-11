use crate::{
    prelude::{SwapAmount, SwapOutcome},
    Fixed, LiquiditySourceFilter, LiquiditySourceId,
};
use frame_support::{
    dispatch::DispatchResult,
    sp_runtime::{traits::BadOrigin, DispatchError},
    weights::Weight,
    Parameter,
};
use frame_system::RawOrigin;
//FIXME maybe try info or try from is better than From and Option.
//use sp_std::convert::TryInto;
use crate::balance::Balance;
use sp_std::vec::Vec;

/// Check on origin that it is a DEX owner.
pub trait EnsureDEXOwner<DEXId, AccountId, Error> {
    fn ensure_can_manage<OuterOrigin>(
        dex_id: &DEXId,
        origin: OuterOrigin,
    ) -> Result<Option<AccountId>, Error>
    where
        OuterOrigin: Into<Result<RawOrigin<AccountId>, OuterOrigin>>;
}

impl<DEXId, AccountId> EnsureDEXOwner<DEXId, AccountId, DispatchError> for () {
    fn ensure_can_manage<OuterOrigin>(
        _dex_id: &DEXId,
        origin: OuterOrigin,
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

pub type AccountIdOf<T> = <T as frame_system::Trait>::AccountId;

/// Common DEX trait. Used for DEX-related pallets.
pub trait Trait: frame_system::Trait + currencies::Trait {
    /// DEX identifier.
    type DEXId: Parameter + Ord + Copy + Default + From<crate::primitives::DEXId>;
}

/// Definition of a pending atomic swap action. It contains the following three phrases:
///
/// - **Reserve**: reserve the resources needed for a swap. This is to make sure that **Claim**
/// succeeds with best efforts.
/// - **Claim**: claim any resources reserved in the first phrase.
/// - **Cancel**: cancel any resources reserved in the first phrase.
pub trait SwapAction<SourceAccountId, TargetAccountId, T: Trait> {
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
impl<SourceAccountId, TargetAccountId, T: Trait> SwapAction<SourceAccountId, TargetAccountId, T>
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

pub trait SwapRulesValidation<SourceAccountId, TargetAccountId, T: Trait>:
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

impl<SourceAccountId, TargetAccountId, T: Trait>
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
