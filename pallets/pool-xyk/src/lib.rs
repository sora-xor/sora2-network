#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{decl_error, decl_event, decl_module, decl_storage, dispatch};
use frame_system::ensure_signed;

use codec::{Decode, Encode};
use core::convert::TryInto;
use frame_support::weights::Weight;
use frame_support::Parameter;
use sp_runtime::RuntimeDebug;

use common::{
    hash,
    prelude::{Balance, EnsureDEXOwner, FixedWrapper, SwapAmount, SwapOutcome},
    AssetSymbol, EnsureTradingPairExists, LiquiditySource,
};

use frame_support::dispatch::{DispatchError, DispatchResult};

use common::Fixed;
use common::SwapRulesValidation;
use common::ToFeeAccount;
use common::ToTechUnitFromDEXAndTradingPair;
use frame_support::ensure;
use frame_support::traits::Get;
use permissions::{Scope, BURN, MINT};
use sp_std::collections::btree_set::BTreeSet;

mod weights;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

type LstId = common::LiquiditySourceType;

type AccountIdOf<T> = <T as frame_system::Trait>::AccountId;

type AssetIdOf<T> = <T as assets::Trait>::AssetId;

type TechAssetIdOf<T> = <T as technical::Trait>::TechAssetId;

type TechAccountIdOf<T> = <T as technical::Trait>::TechAccountId;

type DEXIdOf<T> = <T as common::Trait>::DEXId;

type PolySwapActionStructOf<T> =
    PolySwapAction<AssetIdOf<T>, TechAssetIdOf<T>, Balance, AccountIdOf<T>, TechAccountIdOf<T>>;

type PairSwapActionOf<T> =
    PairSwapAction<AssetIdOf<T>, Balance, AccountIdOf<T>, TechAccountIdOf<T>>;

type WithdrawLiquidityActionOf<T> = WithdrawLiquidityAction<
    AssetIdOf<T>,
    TechAssetIdOf<T>,
    Balance,
    AccountIdOf<T>,
    TechAccountIdOf<T>,
>;

type DepositLiquidityActionOf<T> = DepositLiquidityAction<
    AssetIdOf<T>,
    TechAssetIdOf<T>,
    Balance,
    AccountIdOf<T>,
    TechAccountIdOf<T>,
>;

/// Bounds enum, used for cases than min max limits is used. Also used for cases than values is
/// Desired by used or Calculated by forumula. Dummy is used to abstract checking.
#[derive(Clone, Copy, RuntimeDebug, Eq, PartialEq, Encode, Decode)]
pub enum Bounds<Balance> {
    /// This is consequence of computations, and not sed by used.
    Calculated(Balance),
    /// This values set by used as fixed and determed value.
    Desired(Balance),
    /// This is undetermined value, bounded by some logic or ranges.
    Min(Balance),
    Max(Balance),
    /// This is determined value than pool is emply, then pool is not empty this works like range.
    RangeFromDesiredToMin(Balance, Balance),
    /// This is just unknown value that must be calulated and filled.
    Decide,
    /// This is used in some checks tests and predicates, than value is not needed.
    Dummy,
}

impl<Balance: Ord + Eq + Clone> Bounds<Balance> {
    /// Unwrap only known values, min and max is not known for final value.
    fn unwrap(self) -> Balance {
        match self {
            Bounds::Calculated(a) => a,
            Bounds::Desired(a) => a,
            Bounds::RangeFromDesiredToMin(a, _) => a,
            _ => unreachable!("Must not happen, every uncalculated bound must be set in prepare_and_validate function"),
        }
    }

    fn meets_the_boundaries(&self, rhs: &Self) -> bool {
        use Bounds::*;
        match (
            self,
            Option::<&Balance>::from(self),
            Option::<&Balance>::from(rhs),
        ) {
            (Min(a), _, Some(b)) => a <= b,
            (Max(a), _, Some(b)) => a >= b,
            (RangeFromDesiredToMin(a, b), _, Some(c)) => a >= c && c <= b,
            (_, Some(a), Some(b)) => a == b,
            _ => false,
        }
    }

    #[allow(dead_code)]
    fn meets_the_boundaries_mutally(&self, rhs: &Self) -> bool {
        self.meets_the_boundaries(rhs) || rhs.meets_the_boundaries(self)
    }
}

impl<Balance> From<Bounds<Balance>> for Option<Balance> {
    fn from(bounds: Bounds<Balance>) -> Self {
        match bounds {
            Bounds::Calculated(a) => Some(a),
            Bounds::Desired(a) => Some(a),
            Bounds::RangeFromDesiredToMin(a, _) => Some(a),
            _ => None,
        }
    }
}

impl<'a, Balance> From<&'a Bounds<Balance>> for Option<&'a Balance> {
    fn from(bounds: &'a Bounds<Balance>) -> Self {
        match bounds {
            Bounds::Calculated(a) => Some(a),
            Bounds::Desired(a) => Some(a),
            Bounds::RangeFromDesiredToMin(a, _) => Some(a),
            _ => None,
        }
    }
}

#[derive(Clone, RuntimeDebug, Eq, PartialEq, Encode, Decode)]
pub struct Resource<AssetId, Balance> {
    // This is `AssetId` of `Resource`.
    pub asset: AssetId,
    // This is amount of `Resurce`.
    pub amount: Bounds<Balance>,
}

#[derive(Clone, RuntimeDebug, Eq, PartialEq, Encode, Decode)]
pub struct ResourcePair<AssetId, Balance>(Resource<AssetId, Balance>, Resource<AssetId, Balance>);

#[derive(Clone, RuntimeDebug, Eq, PartialEq, Encode, Decode)]
pub struct PairSwapAction<AssetId, Balance, AccountId, TechAccountId> {
    client_account: Option<AccountId>,
    receiver_account: Option<AccountId>,
    pool_account: TechAccountId,
    source: Resource<AssetId, Balance>,
    destination: Resource<AssetId, Balance>,
    fee: Option<Balance>,
    fee_account: Option<TechAccountId>,
    get_fee_from_destination: Option<bool>,
}

#[derive(Clone, RuntimeDebug, Eq, PartialEq, Encode, Decode)]
pub struct DepositLiquidityAction<AssetId, TechAssetId, Balance, AccountId, TechAccountId> {
    client_account: Option<AccountId>,
    receiver_account: Option<AccountId>,
    pool_account: TechAccountId,
    source: ResourcePair<AssetId, Balance>,
    destination: Resource<TechAssetId, Balance>,
    min_liquidity: Option<Balance>,
}

#[derive(Clone, RuntimeDebug, Eq, PartialEq, Encode, Decode)]
pub struct WithdrawLiquidityAction<AssetId, TechAssetId, Balance, AccountId, TechAccountId> {
    client_account: Option<AccountId>,
    receiver_account_a: Option<AccountId>,
    receiver_account_b: Option<AccountId>,
    pool_account: TechAccountId,
    source: Resource<TechAssetId, Balance>,
    destination: ResourcePair<AssetId, Balance>,
}

#[derive(Clone, RuntimeDebug, Eq, PartialEq, Encode, Decode)]
pub enum PolySwapAction<AssetId, TechAssetId, Balance, AccountId, TechAccountId> {
    PairSwap(PairSwapAction<AssetId, Balance, AccountId, TechAccountId>),
    DepositLiquidity(
        DepositLiquidityAction<AssetId, TechAssetId, Balance, AccountId, TechAccountId>,
    ),
    WithdrawLiquidity(
        WithdrawLiquidityAction<AssetId, TechAssetId, Balance, AccountId, TechAccountId>,
    ),
}

pub trait WeightInfo {
    fn swap_pair() -> Weight;
    fn deposit_liquidity() -> Weight;
    fn withdraw_liquidity() -> Weight;
    fn initialize_pool() -> Weight;
}

/// Configure the pallet by specifying the parameters and types on which it depends.
pub trait Trait:
    technical::Trait + dex_manager::Trait + trading_pair::Trait + pswap_distribution::Trait
{
    /// Because this pallet emits events, it depends on the runtime's definition of an event.
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;

    //TODO: implement and use + Into<SwapActionOf<T> for this types.
    type PairSwapAction: common::SwapAction<AccountIdOf<Self>, TechAccountIdOf<Self>, Self>
        + Parameter;
    type DepositLiquidityAction: common::SwapAction<AccountIdOf<Self>, TechAccountIdOf<Self>, Self>
        + Parameter;
    type WithdrawLiquidityAction: common::SwapAction<AccountIdOf<Self>, TechAccountIdOf<Self>, Self>
        + Parameter;
    type PolySwapAction: common::SwapAction<AccountIdOf<Self>, TechAccountIdOf<Self>, Self>
        + Parameter
        + Into<<Self as technical::Trait>::SwapAction>
        + From<PolySwapActionStructOf<Self>>;
    type EnsureDEXOwner: EnsureDEXOwner<Self::DEXId, Self::AccountId, DispatchError>;

    /// Weight information for extrinsics in this pallet.
    type WeightInfo: WeightInfo;
}

impl<T: Trait> common::SwapRulesValidation<AccountIdOf<T>, TechAccountIdOf<T>, T>
    for PairSwapAction<AssetIdOf<T>, Balance, AccountIdOf<T>, TechAccountIdOf<T>>
{
    fn is_abstract_checking(&self) -> bool {
        self.source.amount == Bounds::Dummy || self.destination.amount == Bounds::Dummy
    }

    fn prepare_and_validate(&mut self, source_opt: Option<&AccountIdOf<T>>) -> DispatchResult {
        let abstract_checking_from_method = common::SwapRulesValidation::<
            AccountIdOf<T>,
            TechAccountIdOf<T>,
            T,
        >::is_abstract_checking(self);
        let abstract_checking = source_opt.is_none() || abstract_checking_from_method;
        let abstract_checking_for_quote = source_opt.is_none() && !abstract_checking_from_method;

        // Check that client account is same as source, because signature is checked for source.
        // Signature checking is used in extrinsics for example, and source is derived from origin.
        // TODO: In general case it is possible to use different client account, for example if
        // signature of source is legal for some source accounts.
        if !abstract_checking {
            let source = source_opt.unwrap();
            match &self.client_account {
                // Just use `client_account` as copy of source.
                None => {
                    self.client_account = Some(source.clone());
                }
                Some(ca) => {
                    if ca != source {
                        Err(Error::<T>::SourceAndClientAccountDoNotMatchAsEqual)?;
                    }
                }
            }

            // Dealing with receiver account, for example case then not swapping to self, but to
            // other account.
            match &self.receiver_account {
                // Just use `client_account` as same account, swapping to self.
                None => {
                    self.receiver_account = self.client_account.clone();
                }
                _ => (),
            }
        }

        let pool_account_repr_sys =
            technical::Module::<T>::tech_account_id_to_account_id(&self.pool_account)?;
        // Check that pool account is valid.
        Module::<T>::is_pool_account_valid_for(self.source.asset, &self.pool_account)?;

        // Source balance of source account.
        let balance_ss = if abstract_checking {
            None
        } else {
            Some(<assets::Module<T>>::free_balance(
                &self.source.asset,
                &source_opt.unwrap(),
            )?)
        };
        // Source balance of technical account.
        let balance_st =
            <assets::Module<T>>::free_balance(&self.source.asset, &pool_account_repr_sys)?;
        // Destination balance of technical account.
        let balance_tt =
            <assets::Module<T>>::free_balance(&self.destination.asset, &pool_account_repr_sys)?;
        if !abstract_checking {
            ensure!(
                balance_ss.unwrap() > 0_u32.into(),
                Error::<T>::AccountBalanceIsInvalid
            );
        }
        if balance_st == 0_u32.into() && balance_tt == 0_u32.into() {
            Err(Error::<T>::PoolIsEmpty)?;
        } else if balance_st <= 0_u32.into() || balance_tt <= 0_u32.into() {
            Err(Error::<T>::PoolIsInvalid)?;
        }

        match self.get_fee_from_destination {
            None => {
                let is_fee_from_d = Module::<T>::decide_is_fee_from_destination(
                    &self.source.asset,
                    &self.destination.asset,
                )?;
                self.get_fee_from_destination = Some(is_fee_from_d);
            }
            _ => (),
        }

        // Recommended fee, will be used if fee is not specified or for checking if specified.
        let mut recom_fee: Option<Balance> = None;

        if abstract_checking_for_quote || !abstract_checking {
            match (self.source.amount, self.destination.amount) {
                // Case then both source and destination amounts is specified, just checking it.
                (Bounds::Desired(sa), Bounds::Desired(ta)) => {
                    ensure!(sa > 0_u32.into(), Error::<T>::ZeroValueInAmountParameter);
                    ensure!(ta > 0_u32.into(), Error::<T>::ZeroValueInAmountParameter);
                    let y_out_pair = Module::<T>::calc_output_for_exact_input(
                        &self.source.asset,
                        &self.destination.asset,
                        &self.pool_account,
                        self.get_fee_from_destination.unwrap(),
                        &balance_st,
                        &balance_tt,
                        &sa,
                    )?;
                    let x_in_pair = Module::<T>::calc_input_for_exact_output(
                        &self.source.asset,
                        &self.destination.asset,
                        &self.pool_account,
                        self.get_fee_from_destination.unwrap(),
                        &balance_st,
                        &balance_tt,
                        &ta,
                    )?;
                    if y_out_pair.0 != ta || x_in_pair.0 != sa || y_out_pair.1 != x_in_pair.1 {
                        Err(Error::<T>::PoolPairRatioAndPairSwapRatioIsDifferent)?;
                    }
                    recom_fee = Some(y_out_pair.1);
                }
                // Case then source amount is specified but destination is not, it`s possible to decide it.
                (Bounds::Desired(sa), ta_bnd) => {
                    ensure!(sa > 0_u32.into(), Error::<T>::ZeroValueInAmountParameter);
                    match ta_bnd {
                        Bounds::Min(ta_min) => {
                            let (calculated, fee) = Module::<T>::calc_output_for_exact_input(
                                &self.source.asset,
                                &self.destination.asset,
                                &self.pool_account,
                                self.get_fee_from_destination.unwrap(),
                                &balance_st,
                                &balance_tt,
                                &sa,
                            )?;

                            ensure!(
                                calculated >= ta_min,
                                Error::<T>::CalculatedValueIsOutOfDesiredBounds
                            );
                            self.destination.amount = Bounds::Calculated(calculated);
                            recom_fee = Some(fee);
                        }
                        _ => {
                            Err(Error::<T>::ImposibleToDecideAssetPairAmounts)?;
                        }
                    }
                }
                // Case then destination amount is specified but source is not, it`s possible to decide it.
                (sa_bnd, Bounds::Desired(ta)) => {
                    ensure!(ta > 0_u32.into(), Error::<T>::ZeroValueInAmountParameter);
                    match sa_bnd {
                        Bounds::Max(sa_max) => {
                            let (calculated, fee) = Module::<T>::calc_input_for_exact_output(
                                &self.source.asset,
                                &self.destination.asset,
                                &self.pool_account,
                                self.get_fee_from_destination.unwrap(),
                                &balance_st,
                                &balance_tt,
                                &ta,
                            )?;
                            ensure!(
                                calculated <= sa_max,
                                Error::<T>::CalculatedValueIsOutOfDesiredBounds
                            );
                            self.source.amount = Bounds::Calculated(calculated);
                            recom_fee = Some(fee);
                        }
                        _ => {
                            Err(Error::<T>::ImposibleToDecideAssetPairAmounts)?;
                        }
                    }
                }
                // Case then no amount is specified, imposible to decide any amounts.
                (_, _) => {
                    Err(Error::<T>::ImposibleToDecideAssetPairAmounts)?;
                }
            }
        }

        // Check fee account if it is specified, or set it if not.
        match self.fee_account {
            Some(ref fa) => {
                // Checking that fee account is valid for this set of parameters.
                Module::<T>::is_fee_account_valid_for(self.source.asset, &self.pool_account, fa)?;
            }
            None => {
                let fa = Module::<T>::get_fee_account(&self.pool_account)?;
                self.fee_account = Some(fa);
            }
        }

        if abstract_checking_for_quote || !abstract_checking {
            let source_amount = self.source.amount.unwrap();
            let destination_amount = self.destination.amount.unwrap();

            // Set recommended or check that fee is correct.
            match self.fee {
                // Just set it here if it not specified, this is usual case.
                None => {
                    self.fee = recom_fee;
                }
                // Case with source user fee is set, checking that it is not smaller.
                Some(fee) => {
                    if fee < recom_fee.unwrap() {
                        Err(Error::<T>::PairSwapActionFeeIsSmallerThanRecommended)?
                    }
                }
            }
            // Get required values, now it is always Some, it is safe to unwrap().
            let fee = self.fee.unwrap();

            if !abstract_checking {
                // Checking that balances if correct and large enouth for amounts.
                if self.get_fee_from_destination.unwrap() {
                    // For source account balance must be not smaller than required with fee.
                    if balance_ss.unwrap() < source_amount {
                        Err(Error::<T>::SourceBalanceIsNotLargeEnough)?;
                    }
                    // For destination technical account balance must successful large for this swap.
                    if balance_tt - fee < destination_amount {
                        Err(Error::<T>::TargetBalanceIsNotLargeEnough)?;
                    }
                    if (self.destination.amount.unwrap() - self.fee.unwrap()) <= 0u32.into() {
                        Err(Error::<T>::GettingFeeFromDestinationInImposible)?;
                    }
                } else {
                    // For source account balance must be not smaller than required with fee.
                    if balance_ss.unwrap() - fee < source_amount {
                        Err(Error::<T>::SourceBalanceIsNotLargeEnough)?;
                    }
                    // For destination technical account balance must successful large for this swap.
                    if balance_tt < destination_amount {
                        Err(Error::<T>::TargetBalanceIsNotLargeEnough)?;
                    }
                }
            }
        }
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

impl<T: Trait> common::SwapAction<AccountIdOf<T>, TechAccountIdOf<T>, T>
    for PairSwapAction<AssetIdOf<T>, Balance, AccountIdOf<T>, TechAccountIdOf<T>>
{
    /// This function is called after validation, and every `Option` is `Some`, and it is safe to do
    /// unwrap. `Bounds` is also safe to unwrap.
    fn reserve(&self, source: &AccountIdOf<T>) -> dispatch::DispatchResult {
        common::with_transaction(|| {
            if Some(source) != self.client_account.as_ref() {
                let e = Error::<T>::SourceAndClientAccountDoNotMatchAsEqual.into();
                return Err(e);
            }
            ensure!(
                Some(source) == self.client_account.as_ref(),
                Error::<T>::SourceAndClientAccountDoNotMatchAsEqual
            );
            let fee_account_repr_sys = technical::Module::<T>::tech_account_id_to_account_id(
                self.fee_account.as_ref().unwrap(),
            )?;

            if self.get_fee_from_destination.unwrap() {
                technical::Module::<T>::transfer_in(
                    &self.source.asset,
                    &source,
                    &self.pool_account,
                    self.source.amount.unwrap(),
                )?;
                technical::Module::<T>::transfer_out(
                    &self.destination.asset,
                    &self.pool_account,
                    &fee_account_repr_sys,
                    self.fee.unwrap(),
                )?;
                technical::Module::<T>::transfer_out(
                    &self.destination.asset,
                    &self.pool_account,
                    self.receiver_account.as_ref().unwrap(),
                    self.destination.amount.unwrap() - self.fee.unwrap(),
                )?;
            } else {
                technical::Module::<T>::transfer_in(
                    &self.source.asset,
                    &source,
                    &self.pool_account,
                    self.source.amount.unwrap() - self.fee.unwrap(),
                )?;
                technical::Module::<T>::transfer_in(
                    &self.source.asset,
                    &source,
                    self.fee_account.as_ref().unwrap(),
                    self.fee.unwrap(),
                )?;
                technical::Module::<T>::transfer_out(
                    &self.destination.asset,
                    &self.pool_account,
                    self.receiver_account.as_ref().unwrap(),
                    self.destination.amount.unwrap(),
                )?;
            }

            let pool_account_repr_sys =
                technical::Module::<T>::tech_account_id_to_account_id(&self.pool_account)?;
            let balance_a =
                <assets::Module<T>>::free_balance(&self.source.asset, &pool_account_repr_sys)?;
            let balance_b =
                <assets::Module<T>>::free_balance(&self.destination.asset, &pool_account_repr_sys)?;
            Module::<T>::update_reserves(
                &self.source.asset,
                &self.destination.asset,
                (&balance_a, &balance_b),
            );
            Ok(())
        })
    }
    fn claim(&self, _source: &AccountIdOf<T>) -> bool {
        true
    }
    fn weight(&self) -> Weight {
        unimplemented!()
    }
    fn cancel(&self, _source: &AccountIdOf<T>) {
        unimplemented!()
    }
}

impl<T: Trait> common::SwapRulesValidation<AccountIdOf<T>, TechAccountIdOf<T>, T>
    for DepositLiquidityAction<
        AssetIdOf<T>,
        TechAssetIdOf<T>,
        Balance,
        AccountIdOf<T>,
        TechAccountIdOf<T>,
    >
{
    fn is_abstract_checking(&self) -> bool {
        (self.source.0).amount == Bounds::Dummy
            || (self.source.1).amount == Bounds::Dummy
            || self.destination.amount == Bounds::Dummy
    }

    fn prepare_and_validate(&mut self, source_opt: Option<&AccountIdOf<T>>) -> DispatchResult {
        let abstract_checking = source_opt.is_none() || common::SwapRulesValidation::<AccountIdOf<T>, TechAccountIdOf<T>, T>::is_abstract_checking(self);

        // Check that client account is same as source, because signature is checked for source.
        // Signature checking is used in extrinsics for example, and source is derived from origin.
        // TODO: In general case it is possible to use different client account, for example if
        // signature of source is legal for some source accounts.
        if !abstract_checking {
            let source = source_opt.unwrap();
            match &self.client_account {
                // Just use `client_account` as copy of source.
                None => {
                    self.client_account = Some(source.clone());
                }
                Some(ca) => {
                    if ca != source {
                        Err(Error::<T>::SourceAndClientAccountDoNotMatchAsEqual)?;
                    }
                }
            }

            // Dealing with receiver account, for example case then not swapping to self, but to
            // other account.
            match &self.receiver_account {
                // Just use `client_account` as same account, swapping to self.
                None => {
                    self.receiver_account = self.client_account.clone();
                }
                _ => (),
            }
        }

        let pool_account_repr_sys =
            technical::Module::<T>::tech_account_id_to_account_id(&self.pool_account)?;
        // Check that pool account is valid.
        Module::<T>::is_pool_account_valid_for(self.source.0.asset, &self.pool_account)?;

        // Balance of source account for asset pair.
        let (balance_bs, balance_ts) = if abstract_checking {
            (None, None)
        } else {
            let source = source_opt.unwrap();
            (
                Some(<assets::Module<T>>::free_balance(
                    &self.source.0.asset,
                    &source,
                )?),
                Some(<assets::Module<T>>::free_balance(
                    &self.source.1.asset,
                    &source,
                )?),
            )
        };

        if !abstract_checking
            && (balance_bs.unwrap() <= 0_u32.into() || balance_ts.unwrap() <= 0_u32.into())
        {
            Err(Error::<T>::AccountBalanceIsInvalid)?;
        }

        // Balance of pool account for asset pair basic asset.
        let balance_bp =
            <assets::Module<T>>::free_balance(&self.source.0.asset, &pool_account_repr_sys)?;
        // Balance of pool account for asset pair target asset.
        let balance_tp =
            <assets::Module<T>>::free_balance(&self.source.1.asset, &pool_account_repr_sys)?;

        let mut empty_pool = false;
        if balance_bp == 0_u32.into() && balance_tp == 0_u32.into() {
            empty_pool = true;
        } else if balance_bp <= 0_u32.into() {
            Err(Error::<T>::PoolIsInvalid)?;
        } else if balance_tp <= 0_u32.into() {
            Err(Error::<T>::PoolIsInvalid)?;
        }

        let mut init_x = 0_u32.into();
        let mut init_y = 0_u32.into();
        if !abstract_checking && empty_pool {
            // Convertation from `Bounds` to `Option` is used here, and it is posible that value
            // None value returned from conversion.
            init_x = Option::<Balance>::from((self.source.0).amount)
                .ok_or(Error::<T>::InitialLiqudityDepositRatioMustBeDefined)?;
            init_y = Option::<Balance>::from((self.source.1).amount)
                .ok_or(Error::<T>::InitialLiqudityDepositRatioMustBeDefined)?;
        }

        // FixedWrapper version of variables.
        let fxw_balance_bp: FixedWrapper = balance_bp.into();
        let fxw_balance_tp: FixedWrapper = balance_tp.into();

        // Product of pool pair amounts to get k value.
        let (pool_k, fxw_pool_k) = {
            if empty_pool {
                if abstract_checking {
                    (None, None)
                } else {
                    let fxw_init_x: FixedWrapper = init_x.into();
                    let fxw_init_y: FixedWrapper = init_x.into();
                    let fxw_value: FixedWrapper =
                        fxw_init_x.sqrt_accurate() * fxw_init_y.sqrt_accurate();
                    let value: Fixed = fxw_value
                        .get()
                        .ok_or(Error::<T>::FixedWrapperCalculationFailed)?;
                    (Some(value.into()), Some(fxw_value))
                }
            } else {
                let fxw_value: FixedWrapper =
                    fxw_balance_bp.sqrt_accurate() * fxw_balance_tp.sqrt_accurate();
                let value: Fixed = fxw_value
                    .get()
                    .ok_or(Error::<T>::FixedWrapperCalculationFailed)?;
                (Some(value.into()), Some(fxw_value))
            }
        };
        if !abstract_checking {
            if empty_pool {
                match self.destination.amount {
                    Bounds::Desired(k) => {
                        ensure!(
                            k == pool_k.unwrap(),
                            Error::<T>::InvalidDepositLiquidityDestinationAmount
                        );
                    }
                    _ => {
                        self.destination.amount = Bounds::Calculated(pool_k.unwrap());
                    }
                }
            } else {
                match (
                    (self.source.0).amount,
                    (self.source.1).amount,
                    self.destination.amount,
                ) {
                    (ox, oy, Bounds::Desired(destination_k)) => {
                        ensure!(
                            destination_k > 0_u32.into(),
                            Error::<T>::ZeroValueInAmountParameter
                        );
                        let fxw_destination_k: FixedWrapper = init_x.into();
                        let fxw_peace_to_add = fxw_pool_k.unwrap() / fxw_destination_k;
                        let fxw_recom_x = fxw_balance_bp / fxw_peace_to_add;
                        let fxw_recom_y = fxw_balance_tp / fxw_peace_to_add;
                        let recom_x = (fxw_recom_x
                            .get()
                            .ok_or(Error::<T>::FixedWrapperCalculationFailed)?)
                        .into();
                        let recom_y = (fxw_recom_y
                            .get()
                            .ok_or(Error::<T>::FixedWrapperCalculationFailed)?)
                        .into();
                        match ox {
                            Bounds::Desired(x) => {
                                if x != recom_x {
                                    Err(Error::<T>::InvalidDepositLiquidityBasicAssetAmount)?
                                }
                            }
                            bounds => {
                                let value: Fixed = (fxw_balance_bp / fxw_peace_to_add)
                                    .get()
                                    .ok_or(Error::<T>::FixedWrapperCalculationFailed)?;
                                let calc = Bounds::Calculated(value.into());
                                ensure!(
                                    bounds.meets_the_boundaries(&calc),
                                    Error::<T>::CalculatedValueIsNotMeetsRequiredBoundaries
                                );
                                (self.source.0).amount = calc;
                            }
                        }
                        match oy {
                            Bounds::Desired(y) => {
                                if y != recom_y {
                                    Err(Error::<T>::InvalidDepositLiquidityTargetAssetAmount)?
                                }
                            }
                            bounds => {
                                let value: Fixed = (fxw_balance_tp / fxw_peace_to_add)
                                    .get()
                                    .ok_or(Error::<T>::FixedWrapperCalculationFailed)?;
                                let calc = Bounds::Calculated(value.into());
                                ensure!(
                                    bounds.meets_the_boundaries(&calc),
                                    Error::<T>::CalculatedValueIsNotMeetsRequiredBoundaries
                                );
                                (self.source.1).amount = calc;
                            }
                        }
                    }
                    (
                        Bounds::RangeFromDesiredToMin(xdes, xmin),
                        Bounds::RangeFromDesiredToMin(ydes, ymin),
                        dest_amount,
                    ) => {
                        ensure!(
                            xdes >= xmin && ydes >= ymin,
                            Error::<T>::RangeValuesIsInvalid
                        );
                        let fxw_xdes: FixedWrapper = xdes.into();
                        let fxw_ydes: FixedWrapper = ydes.into();
                        let fxw_desliq = fxw_xdes.sqrt_accurate() * fxw_ydes.sqrt_accurate();
                        let fxw_piece = fxw_pool_k.unwrap() / fxw_desliq;
                        let fxw_bp_tmp = fxw_balance_bp / fxw_piece;
                        let fxw_tp_tmp = fxw_balance_tp / fxw_piece;
                        let fxw_bp_down = fxw_bp_tmp / fxw_xdes;
                        let fxw_tp_down = fxw_tp_tmp / fxw_ydes;
                        let bp_down: Balance = (fxw_bp_down
                            .get()
                            .ok_or(Error::<T>::FixedWrapperCalculationFailed)?)
                        .into();
                        let tp_down: Balance = (fxw_tp_down
                            .get()
                            .ok_or(Error::<T>::FixedWrapperCalculationFailed)?)
                        .into();
                        let fxw_down: FixedWrapper = bp_down.max(tp_down).into();
                        let fxw_bp_corr1 = fxw_bp_tmp / fxw_down;
                        let fxw_tp_corr1 = fxw_tp_tmp / fxw_down;
                        let bp_corr1: Balance = (fxw_bp_corr1
                            .get()
                            .ok_or(Error::<T>::FixedWrapperCalculationFailed)?)
                        .into();
                        let tp_corr1: Balance = (fxw_tp_corr1
                            .get()
                            .ok_or(Error::<T>::FixedWrapperCalculationFailed)?)
                        .into();
                        ensure!(
                            bp_corr1 >= xmin && tp_corr1 >= ymin,
                            Error::<T>::ImposibleToDecideValidPairValuesFromRangeForThisPool
                        );
                        (self.source.0).amount = Bounds::Calculated(bp_corr1);
                        (self.source.1).amount = Bounds::Calculated(tp_corr1);
                        match dest_amount {
                            Bounds::Desired(_) => {
                                return Err(Error::<T>::ThisCaseIsNotSupported.into());
                            }
                            _ => {
                                let calc: Balance = ((fxw_bp_corr1.sqrt_accurate()
                                    * fxw_tp_corr1.sqrt_accurate())
                                .get()
                                .ok_or(Error::<T>::FixedWrapperCalculationFailed)?)
                                .into();
                                self.destination.amount = Bounds::Calculated(calc);
                            }
                        }
                    }
                    // Case then no amount is specified (or something needed is not specified),
                    // impossible to decide any amounts.
                    (_, _, _) => {
                        Err(Error::<T>::ImposibleToDecideDepositLiquidityAmounts)?;
                    }
                }
            }
        }

        // Recommended minimum liquidity, will be used if not specified or for checking if specified.
        let recom_min_liquidity =
            Module::<T>::get_min_liquidity_for(self.source.0.asset, &self.pool_account);
        // Set recommended or check that `min_liquidity` is correct.
        match self.min_liquidity {
            // Just set it here if it not specified, this is usual case.
            None => {
                self.min_liquidity = Some(recom_min_liquidity);
            }
            // Case with source user `min_liquidity` is set, checking that it is not smaller.
            Some(min_liquidity) => {
                if min_liquidity < recom_min_liquidity {
                    Err(Error::<T>::PairSwapActionMinimumLiquidityIsSmallerThanRecommended)?
                }
            }
        }

        //TODO: for abstract_checking, check that is enouth liquidity in pool.
        if !abstract_checking {
            // Get required values, now it is always Some, it is safe to unwrap().
            let min_liquidity = self.min_liquidity.unwrap();
            let base_amount = (self.source.0).amount.unwrap();
            let target_amount = (self.source.1).amount.unwrap();
            let destination_amount = self.destination.amount.unwrap();
            // Checking by minimum liquidity.
            if min_liquidity > pool_k.unwrap()
                && destination_amount < min_liquidity - pool_k.unwrap()
            {
                Err(Error::<T>::DestinationAmountOfLiquidityIsNotLargeEnough)?;
            }
            // Checking that balances if correct and large enough for amounts.
            if balance_bs.unwrap() < base_amount {
                Err(Error::<T>::SourceBaseAmountIsNotLargeEnough)?;
            }
            if balance_ts.unwrap() < target_amount {
                Err(Error::<T>::TargetBaseAmountIsNotLargeEnough)?;
            }
        }

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

impl<T: Trait> common::SwapAction<AccountIdOf<T>, TechAccountIdOf<T>, T>
    for DepositLiquidityAction<
        AssetIdOf<T>,
        TechAssetIdOf<T>,
        Balance,
        AccountIdOf<T>,
        TechAccountIdOf<T>,
    >
{
    fn reserve(&self, source: &AccountIdOf<T>) -> dispatch::DispatchResult {
        ensure!(
            Some(source) == self.client_account.as_ref(),
            Error::<T>::SourceAndClientAccountDoNotMatchAsEqual
        );
        let asset_repr = Into::<AssetIdOf<T>>::into(self.destination.asset);
        let pool_account_repr_sys =
            technical::Module::<T>::tech_account_id_to_account_id(&self.pool_account)?;
        technical::Module::<T>::transfer_in(
            &(self.source.0).asset,
            &source,
            &self.pool_account,
            (self.source.0).amount.unwrap(),
        )?;
        technical::Module::<T>::transfer_in(
            &(self.source.1).asset,
            &source,
            &self.pool_account,
            (self.source.1).amount.unwrap(),
        )?;
        assets::Module::<T>::mint_to(
            &asset_repr,
            &pool_account_repr_sys,
            self.receiver_account.as_ref().unwrap(),
            self.destination.amount.unwrap(),
        )?;
        let pool_account_repr_sys =
            technical::Module::<T>::tech_account_id_to_account_id(&self.pool_account)?;
        let balance_a =
            <assets::Module<T>>::free_balance(&(self.source.0).asset, &pool_account_repr_sys)?;
        let balance_b =
            <assets::Module<T>>::free_balance(&(self.source.1).asset, &pool_account_repr_sys)?;
        Module::<T>::update_reserves(
            &(self.source.0).asset,
            &(self.source.1).asset,
            (&balance_a, &balance_b),
        );
        Ok(())
    }
    fn claim(&self, _source: &AccountIdOf<T>) -> bool {
        true
    }
    fn weight(&self) -> Weight {
        unimplemented!()
    }
    fn cancel(&self, _source: &AccountIdOf<T>) {
        unimplemented!()
    }
}

impl<T: Trait> common::SwapRulesValidation<AccountIdOf<T>, TechAccountIdOf<T>, T>
    for WithdrawLiquidityAction<
        AssetIdOf<T>,
        TechAssetIdOf<T>,
        Balance,
        AccountIdOf<T>,
        TechAccountIdOf<T>,
    >
{
    fn is_abstract_checking(&self) -> bool {
        (self.destination.0).amount == Bounds::Dummy
            || (self.destination.1).amount == Bounds::Dummy
            || self.source.amount == Bounds::Dummy
    }

    fn prepare_and_validate(&mut self, source_opt: Option<&AccountIdOf<T>>) -> DispatchResult {
        //TODO: replace unwrap.
        let source = source_opt.unwrap();
        // Check that client account is same as source, because signature is checked for source.
        // Signature checking is used in extrinsics for example, and source is derived from origin.
        // TODO: In general case it is possible to use different client account, for example if
        // signature of source is legal for some source accounts.
        match &self.client_account {
            // Just use `client_account` as copy of source.
            None => {
                self.client_account = Some(source.clone());
            }
            Some(ca) => {
                if ca != source {
                    Err(Error::<T>::SourceAndClientAccountDoNotMatchAsEqual)?;
                }
            }
        }

        // Dealing with receiver account, for example case then not swapping to self, but to
        // other account.
        match &self.receiver_account_a {
            // Just use `client_account` as same account, swapping to self.
            None => {
                self.receiver_account_a = self.client_account.clone();
            }
            _ => (),
        }
        match &self.receiver_account_b {
            // Just use `client_account` as same account, swapping to self.
            None => {
                self.receiver_account_b = self.client_account.clone();
            }
            _ => (),
        }
        let pool_account_repr_sys =
            technical::Module::<T>::tech_account_id_to_account_id(&self.pool_account)?;
        // Check that pool account is valid.
        Module::<T>::is_pool_account_valid_for(self.destination.0.asset, &self.pool_account)?;

        let mark_asset = Module::<T>::get_marking_asset(&self.pool_account)?;
        ensure!(
            self.source.asset == mark_asset,
            Error::<T>::InvalidAssetForLiquidityMarking
        );

        let repr_k_asset_id = self.source.asset.into();

        // Balance of source account for k value.
        let balance_ks = <assets::Module<T>>::free_balance(&repr_k_asset_id, &source)?;
        if balance_ks <= 0_u32.into() {
            Err(Error::<T>::AccountBalanceIsInvalid)?;
        }

        // Balance of pool account for asset pair basic asset.
        let balance_bp =
            <assets::Module<T>>::free_balance(&(self.destination.0).asset, &pool_account_repr_sys)?;
        // Balance of pool account for asset pair target asset.
        let balance_tp =
            <assets::Module<T>>::free_balance(&(self.destination.1).asset, &pool_account_repr_sys)?;

        if balance_bp == 0_u32.into() && balance_tp == 0_u32.into() {
            Err(Error::<T>::PoolIsEmpty)?;
        } else if balance_bp <= 0_u32.into() {
            Err(Error::<T>::PoolIsInvalid)?;
        } else if balance_tp <= 0_u32.into() {
            Err(Error::<T>::PoolIsInvalid)?;
        }

        let fxw_balance_bp: FixedWrapper = balance_bp.into();
        let fxw_balance_tp: FixedWrapper = balance_tp.into();

        // Product of pool pair amounts to get k value.
        let fxw_pool_k = fxw_balance_bp.sqrt_accurate() * fxw_balance_tp.sqrt_accurate();
        let pool_k: Balance = (fxw_pool_k
            .get()
            .ok_or(Error::<T>::FixedWrapperCalculationFailed)?)
        .into();

        match (
            self.source.amount,
            (self.destination.0).amount,
            (self.destination.1).amount,
        ) {
            (Bounds::Desired(source_k), ox, oy) => {
                ensure!(
                    source_k > 0_u32.into(),
                    Error::<T>::ZeroValueInAmountParameter
                );
                let fxw_source_k: FixedWrapper = source_k.into();
                let fxw_peace_to_take = fxw_pool_k / fxw_source_k;
                let fxw_recom_x = fxw_balance_bp / fxw_peace_to_take;
                let fxw_recom_y = fxw_balance_tp / fxw_peace_to_take;
                let recom_x: Balance = (fxw_recom_x
                    .get()
                    .ok_or(Error::<T>::FixedWrapperCalculationFailed)?)
                .into();
                let recom_y: Balance = (fxw_recom_y
                    .get()
                    .ok_or(Error::<T>::FixedWrapperCalculationFailed)?)
                .into();

                match ox {
                    Bounds::Desired(x) => {
                        if x != recom_x {
                            Err(Error::<T>::InvalidWithdrawLiquidityBasicAssetAmount)?;
                        }
                    }
                    bounds => {
                        let calc = Bounds::Calculated(recom_x);
                        ensure!(
                            bounds.meets_the_boundaries(&calc),
                            Error::<T>::CalculatedValueIsNotMeetsRequiredBoundaries
                        );
                        (self.destination.0).amount = calc;
                    }
                }

                match oy {
                    Bounds::Desired(y) => {
                        if y != recom_y {
                            Err(Error::<T>::InvalidWithdrawLiquidityTargetAssetAmount)?;
                        }
                    }
                    bounds => {
                        let calc = Bounds::Calculated(recom_y);
                        ensure!(
                            bounds.meets_the_boundaries(&calc),
                            Error::<T>::CalculatedValueIsNotMeetsRequiredBoundaries
                        );
                        (self.destination.1).amount = calc;
                    }
                }
            }

            _ => {
                Err(Error::<T>::ImposibleToDecideDepositLiquidityAmounts)?;
            }
        }

        // Get required values, now it is always Some, it is safe to unwrap().
        let base_amount = (self.destination.1).amount.unwrap();
        let target_amount = (self.destination.0).amount.unwrap();
        let source_amount = self.source.amount.unwrap();

        if source_amount > pool_k {
            Err(Error::<T>::SourceBaseAmountIsTooLarge)?;
        }

        if balance_ks < source_amount {
            Err(Error::<T>::SourceBalanceOfLiquidityTokensIsNotLargeEnough)?;
        }

        // Checking that balances if correct and large enough for amounts.
        if balance_bp < base_amount {
            Err(Error::<T>::DestinationBaseBalanceIsNotLargeEnough)?;
        }
        if balance_tp < target_amount {
            Err(Error::<T>::DestinationTargetBalanceIsNotLargeEnough)?;
        }
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

impl<T: Trait> common::SwapAction<AccountIdOf<T>, TechAccountIdOf<T>, T>
    for WithdrawLiquidityAction<
        AssetIdOf<T>,
        TechAssetIdOf<T>,
        Balance,
        AccountIdOf<T>,
        TechAccountIdOf<T>,
    >
{
    fn reserve(&self, source: &AccountIdOf<T>) -> dispatch::DispatchResult {
        ensure!(
            Some(source) == self.client_account.as_ref(),
            Error::<T>::SourceAndClientAccountDoNotMatchAsEqual
        );
        let asset_repr = Into::<AssetIdOf<T>>::into(self.source.asset);
        let pool_account_repr_sys =
            technical::Module::<T>::tech_account_id_to_account_id(&self.pool_account)?;
        technical::Module::<T>::transfer_out(
            &(self.destination.0).asset,
            &self.pool_account,
            self.receiver_account_a.as_ref().unwrap(),
            self.destination.0.amount.unwrap(),
        )?;
        technical::Module::<T>::transfer_out(
            &(self.destination.1).asset,
            &self.pool_account,
            self.receiver_account_b.as_ref().unwrap(),
            self.destination.1.amount.unwrap(),
        )?;
        assets::Module::<T>::burn_from(
            &asset_repr,
            &pool_account_repr_sys,
            &source,
            self.source.amount.unwrap(),
        )?;
        let pool_account_repr_sys =
            technical::Module::<T>::tech_account_id_to_account_id(&self.pool_account)?;
        let balance_a =
            <assets::Module<T>>::free_balance(&(self.destination.0).asset, &pool_account_repr_sys)?;
        let balance_b =
            <assets::Module<T>>::free_balance(&(self.destination.1).asset, &pool_account_repr_sys)?;
        Module::<T>::update_reserves(
            &(self.destination.0).asset,
            &(self.destination.1).asset,
            (&balance_a, &balance_b),
        );
        Ok(())
    }
    fn claim(&self, _source: &AccountIdOf<T>) -> bool {
        true
    }
    fn weight(&self) -> Weight {
        unimplemented!()
    }
    fn cancel(&self, _source: &AccountIdOf<T>) {
        unimplemented!()
    }
}

impl<T: Trait> common::SwapRulesValidation<AccountIdOf<T>, TechAccountIdOf<T>, T>
    for PolySwapActionStructOf<T>
where
    PairSwapAction<AssetIdOf<T>, Balance, AccountIdOf<T>, TechAccountIdOf<T>>:
        SwapRulesValidation<AccountIdOf<T>, TechAccountIdOf<T>, T>,
    DepositLiquidityAction<
        AssetIdOf<T>,
        TechAssetIdOf<T>,
        Balance,
        AccountIdOf<T>,
        TechAccountIdOf<T>,
    >: SwapRulesValidation<AccountIdOf<T>, TechAccountIdOf<T>, T>,
    WithdrawLiquidityAction<
        AssetIdOf<T>,
        TechAssetIdOf<T>,
        Balance,
        AccountIdOf<T>,
        TechAccountIdOf<T>,
    >: SwapRulesValidation<AccountIdOf<T>, TechAccountIdOf<T>, T>,
{
    fn is_abstract_checking(&self) -> bool {
        match self {
            PolySwapAction::PairSwap(a) => a.is_abstract_checking(),
            PolySwapAction::DepositLiquidity(a) => a.is_abstract_checking(),
            PolySwapAction::WithdrawLiquidity(a) => a.is_abstract_checking(),
        }
    }
    fn prepare_and_validate(&mut self, source: Option<&AccountIdOf<T>>) -> DispatchResult {
        match self {
            PolySwapAction::PairSwap(a) => a.prepare_and_validate(source),
            PolySwapAction::DepositLiquidity(a) => a.prepare_and_validate(source),
            PolySwapAction::WithdrawLiquidity(a) => a.prepare_and_validate(source),
        }
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

impl<T: Trait> common::SwapAction<AccountIdOf<T>, TechAccountIdOf<T>, T>
    for PolySwapActionStructOf<T>
where
    PairSwapAction<AssetIdOf<T>, Balance, AccountIdOf<T>, TechAccountIdOf<T>>:
        common::SwapAction<AccountIdOf<T>, TechAccountIdOf<T>, T>,
    DepositLiquidityAction<
        AssetIdOf<T>,
        TechAssetIdOf<T>,
        Balance,
        AccountIdOf<T>,
        TechAccountIdOf<T>,
    >: common::SwapAction<AccountIdOf<T>, TechAccountIdOf<T>, T>,
    WithdrawLiquidityAction<
        AssetIdOf<T>,
        TechAssetIdOf<T>,
        Balance,
        AccountIdOf<T>,
        TechAccountIdOf<T>,
    >: common::SwapAction<AccountIdOf<T>, TechAccountIdOf<T>, T>,
{
    fn reserve(&self, source: &AccountIdOf<T>) -> dispatch::DispatchResult {
        match self {
            PolySwapAction::PairSwap(a) => a.reserve(source),
            PolySwapAction::DepositLiquidity(a) => a.reserve(source),
            PolySwapAction::WithdrawLiquidity(a) => a.reserve(source),
        }
    }
    fn claim(&self, _source: &AccountIdOf<T>) -> bool {
        true
    }
    fn weight(&self) -> Weight {
        unimplemented!()
    }
    fn cancel(&self, _source: &AccountIdOf<T>) {
        unimplemented!()
    }
}

decl_storage! {
    trait Store for Module<T: Trait> as PoolXYK {
        /// Updated after last liquidity change operation.
        /// [Base Asset Id (XOR) -> Target Asset Id => (Base Balance, Target Balance)].
        /// This storage records is not used as source of information, but used as quick cache for
        /// information that comes from balances for assets from technical accounts.
        /// For example, communication with technical accounts and their storage is not needed, and this
        /// pair to balance cache can be used quickly.
        pub Reserves get(fn reserves): double_map
              hasher(blake2_128_concat) T::AssetId,
              hasher(blake2_128_concat) T::AssetId => (Balance, Balance);
        /// Collection of all registered marker tokens.
        pub MarkerTokensIndex get(fn marker_tokens_index): BTreeSet<T::AssetId>;
        /// Properties of particular pool. [Reserves Account Id, Fees Account Id, Marker Asset Id]
        pub Properties get(fn properties): double_map
              hasher(blake2_128_concat) T::AssetId,
              hasher(blake2_128_concat) T::AssetId => Option<(T::AccountId, T::AccountId, T::AssetId)>;
    }
}

impl<T: Trait> Module<T> {
    fn initialize_pool_properties(
        asset_a: &T::AssetId,
        asset_b: &T::AssetId,
        reserves_account_id: &T::AccountId,
        fees_account_id: &T::AccountId,
        marker_asset_id: &T::AssetId,
    ) {
        let base_asset_id: T::AssetId = T::GetBaseAssetId::get();
        let (sorted_asset_a, sorted_asset_b) = if &base_asset_id == asset_a {
            (asset_a, asset_b)
        } else if &base_asset_id == asset_b {
            (asset_b, asset_a)
        } else {
            let hash_key = common::comm_merkle_op(asset_a, asset_b);
            let (asset_a_pair, asset_b_pair) =
                common::sort_with_hash_key(hash_key, (asset_a, &()), (asset_b, &()));
            (asset_a_pair.0, asset_b_pair.0)
        };
        Properties::<T>::insert(
            sorted_asset_a,
            sorted_asset_b,
            (
                reserves_account_id.clone(),
                fees_account_id.clone(),
                marker_asset_id.clone(),
            ),
        )
    }

    fn update_reserves(
        asset_a: &T::AssetId,
        asset_b: &T::AssetId,
        balance_pair: (&Balance, &Balance),
    ) {
        let base_asset_id: T::AssetId = T::GetBaseAssetId::get();
        if base_asset_id == asset_a.clone() {
            Reserves::<T>::insert(asset_a, asset_b, (balance_pair.0, balance_pair.1));
        } else if base_asset_id == asset_b.clone() {
            Reserves::<T>::insert(asset_b, asset_a, (balance_pair.1, balance_pair.0));
        } else {
            let hash_key = common::comm_merkle_op(asset_a, asset_b);
            let (pair_u, pair_v) = common::sort_with_hash_key(
                hash_key,
                (asset_a, balance_pair.0),
                (asset_b, balance_pair.1),
            );
            Reserves::<T>::insert(pair_u.0, pair_v.0, (pair_u.1, pair_v.1));
        }
    }

    pub fn get_marking_asset_repr(
        tech_acc: &TechAccountIdOf<T>,
    ) -> Result<AssetIdOf<T>, DispatchError> {
        Ok(Into::<AssetIdOf<T>>::into(
            common::ToMarkerAsset::<TechAssetIdOf<T>, LstId>::to_marker_asset(
                tech_acc,
                common::LiquiditySourceType::XYKPool,
            )
            .ok_or(Error::<T>::UnableToDecideMarkerAsset)?,
        ))
    }

    pub fn get_marking_asset(
        tech_acc: &TechAccountIdOf<T>,
    ) -> Result<TechAssetIdOf<T>, DispatchError> {
        Ok(
            common::ToMarkerAsset::<TechAssetIdOf<T>, LstId>::to_marker_asset(
                tech_acc,
                common::LiquiditySourceType::XYKPool,
            )
            .ok_or(Error::<T>::UnableToDecideMarkerAsset)?,
        )
    }
}

impl<T: Trait> Module<T> {
    /// Using try into to get Result with some error, after this convert Result into Option,
    /// after this AssetDecodingError is used if None.
    pub fn try_decode_asset(asset: AssetIdOf<T>) -> Result<TechAssetIdOf<T>, DispatchError> {
        TryInto::<TechAssetIdOf<T>>::try_into(asset)
            .map_err(|_| Error::<T>::AssetDecodingError.into())
    }

    pub fn decide_is_fee_from_destination(
        asset_a: &AssetIdOf<T>,
        asset_b: &AssetIdOf<T>,
    ) -> Result<bool, DispatchError> {
        let base_asset_id: T::AssetId = T::GetBaseAssetId::get();
        if &base_asset_id == asset_a {
            Ok(false)
        } else if &base_asset_id == asset_b {
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn guard_fee_from_destination(
        _asset_a: &AssetIdOf<T>,
        _asset_b: &AssetIdOf<T>,
    ) -> DispatchResult {
        Ok(())
    }

    pub fn guard_fee_from_source(
        _asset_a: &AssetIdOf<T>,
        _asset_b: &AssetIdOf<T>,
    ) -> DispatchResult {
        Ok(())
    }

    #[inline]
    pub fn get_fee_for_source(
        _asset_id: &AssetIdOf<T>,
        _tech_acc: &TechAccountIdOf<T>,
        x_in: &Balance,
    ) -> Result<Balance, DispatchError> {
        //TODO: get this value from DEXInfo.
        let nat1000: Balance = 1000_u32.into();
        let fee: Balance = 3_u32.into();
        Ok((*x_in * fee) / nat1000)
    }

    #[inline]
    pub fn get_fee_for_destination(
        _asset_id: &AssetIdOf<T>,
        _tech_acc: &TechAccountIdOf<T>,
        y_out: &Balance,
    ) -> Result<Balance, DispatchError> {
        //TODO: get this value from DEXInfo.
        let nat1000: Balance = 1000_u32.into();
        let fee: Balance = 3_u32.into();
        Ok((*y_out * fee) / nat1000)
    }

    /// Calulate (y_output,fee) pair where fee can be fee_of_y1 or fee_of_x_in, and output is
    /// without fee.
    pub fn calc_output_for_exact_input(
        asset_a: &AssetIdOf<T>,
        asset_b: &AssetIdOf<T>,
        tech_acc: &TechAccountIdOf<T>,
        get_fee_from_destination: bool,
        x: &Balance,
        y: &Balance,
        x_in: &Balance,
    ) -> Result<(Balance, Balance), DispatchError> {
        let fxw_x: FixedWrapper = x.clone().into();
        let fxw_y: FixedWrapper = y.clone().into();
        let fxw_x_in: FixedWrapper = x_in.clone().into();
        if get_fee_from_destination {
            Module::<T>::guard_fee_from_destination(asset_a, asset_b)?;
            let fxw_y1 = (fxw_x_in * fxw_y) / (fxw_x + fxw_x_in);
            let y1: Balance = (fxw_y1
                .get()
                .ok_or(Error::<T>::FixedWrapperCalculationFailed)?)
            .into();
            let fee_of_y1 = Module::<T>::get_fee_for_destination(asset_a, tech_acc, &y1)?;
            Ok((y1, fee_of_y1))
        } else {
            Module::<T>::guard_fee_from_source(asset_a, asset_b)?;
            let fee_of_x_in = Module::<T>::get_fee_for_source(asset_a, tech_acc, x_in)?;
            let fxw_fee_of_x_in: FixedWrapper = fee_of_x_in.into();
            let fxw_x_in_subfee = fxw_x_in - fxw_fee_of_x_in;
            let fxw_y_out = (fxw_x_in_subfee * fxw_y) / (fxw_x + fxw_x_in_subfee);
            let y_out: Balance = (fxw_y_out
                .get()
                .ok_or(Error::<T>::FixedWrapperCalculationFailed)?)
            .into();
            Ok((y_out, fee_of_x_in))
        }
    }

    /// Calulate (x_input,fee) pair where fee can be fee_of_y1 or fee_of_x_in, and input is
    /// without fee.
    pub fn calc_input_for_exact_output(
        asset_a: &AssetIdOf<T>,
        asset_b: &AssetIdOf<T>,
        tech_acc: &TechAccountIdOf<T>,
        get_fee_from_destination: bool,
        x: &Balance,
        y: &Balance,
        y_out: &Balance,
    ) -> Result<(Balance, Balance), DispatchError> {
        let fxw_x: FixedWrapper = x.clone().into();
        let fxw_y: FixedWrapper = y.clone().into();
        let fxw_y_out: FixedWrapper = y_out.clone().into();
        if get_fee_from_destination {
            Module::<T>::guard_fee_from_destination(asset_a, asset_b)?;
            let unit: Balance = 1_u32.into();
            let fract_a: Balance = Module::<T>::get_fee_for_destination(asset_a, tech_acc, &unit)?;
            let fract_b: Balance = unit - fract_a;
            let fxw_fract_b: FixedWrapper = fract_b.into();
            let fxw_y1 = fxw_y_out / fxw_fract_b;
            let fxw_x_in = (fxw_x * fxw_y1) / (fxw_y - fxw_y1);
            let fxw_fee = fxw_y1 - fxw_y_out;
            let x_in: Balance = (fxw_x_in
                .get()
                .ok_or(Error::<T>::FixedWrapperCalculationFailed)?)
            .into();
            let fee: Balance = (fxw_fee
                .get()
                .ok_or(Error::<T>::FixedWrapperCalculationFailed)?)
            .into();
            Ok((x_in, fee))
        } else {
            Module::<T>::guard_fee_from_source(asset_a, asset_b)?;
            let y_minus_y_out = *y - *y_out;
            let ymyo_fee = Module::<T>::get_fee_for_source(asset_a, tech_acc, &y_minus_y_out)?;
            let ymyo_subfee = y_minus_y_out - ymyo_fee;
            let fxw_ymyo_subfee: FixedWrapper = ymyo_subfee.into();
            let fxw_x_in = (fxw_x * fxw_y_out) / fxw_ymyo_subfee;
            let x_in: Balance = (fxw_x_in
                .get()
                .ok_or(Error::<T>::FixedWrapperCalculationFailed)?)
            .into();
            let fee = Module::<T>::get_fee_for_source(asset_a, tech_acc, &x_in)?;
            Ok((x_in, fee))
        }
    }

    pub fn get_min_liquidity_for(
        _asset_id: AssetIdOf<T>,
        _tech_acc: &TechAccountIdOf<T>,
    ) -> Balance {
        //TODO: get this value from DEXInfo.
        Fixed::from_inner(1000).into()
    }

    pub fn get_fee_account(
        tech_acc: &TechAccountIdOf<T>,
    ) -> Result<TechAccountIdOf<T>, DispatchError> {
        let fee_acc = tech_acc
            .to_fee_account()
            .ok_or(Error::<T>::UnableToDeriveFeeAccount)?;
        Ok(fee_acc)
    }

    pub fn is_fee_account_valid_for(
        _asset_id: AssetIdOf<T>,
        tech_acc: &TechAccountIdOf<T>,
        fee_acc: &TechAccountIdOf<T>,
    ) -> DispatchResult {
        let recommended = Self::get_fee_account(tech_acc)?;
        if fee_acc != &recommended {
            Err(Error::<T>::FeeAccountIsInvalid)?;
        }
        Ok(())
    }

    pub fn is_pool_account_valid_for(
        _asset_id: AssetIdOf<T>,
        tech_acc: &TechAccountIdOf<T>,
    ) -> DispatchResult {
        technical::Module::<T>::ensure_tech_account_registered(tech_acc)?;
        //TODO: Maybe checking that asset and dex is exist, it is not really needed if
        //registration of technical account is a garanty that pair and dex exist.
        Ok(())
    }
}

decl_event!(
    pub enum Event<T>
    where
        AccountId = <T as frame_system::Trait>::AccountId,
    {
        // New pool for particular pair was initialized. [Reserves Account Id]
        PoolIsInitialized(AccountId),
    }
);

decl_error! {
    pub enum Error for Module<T: Trait>
    {
        /// It is impossible to calculate fee for some pair swap operation, or other operation.
        UnableToCalculateFee,
        UnableToGetBalance,
        ImposibleToDecideAssetPairAmounts,
        PoolPairRatioAndPairSwapRatioIsDifferent,
        PairSwapActionFeeIsSmallerThanRecommended,
        SourceBalanceIsNotLargeEnough,
        TargetBalanceIsNotLargeEnough,
        UnableToDeriveFeeAccount,
        FeeAccountIsInvalid,
        SourceAndClientAccountDoNotMatchAsEqual,
        AssetsMustNotBeSame,
        ImposibleToDecideDepositLiquidityAmounts,
        InvalidDepositLiquidityBasicAssetAmount,
        InvalidDepositLiquidityTargetAssetAmount,
        PairSwapActionMinimumLiquidityIsSmallerThanRecommended,
        DestinationAmountOfLiquidityIsNotLargeEnough,
        SourceBaseAmountIsNotLargeEnough,
        TargetBaseAmountIsNotLargeEnough,
        PoolIsInvalid,
        PoolIsEmpty,
        ZeroValueInAmountParameter,
        AccountBalanceIsInvalid,
        InvalidDepositLiquidityDestinationAmount,
        InitialLiqudityDepositRatioMustBeDefined,
        TechAssetIsNotRepresentable,
        UnableToDecideMarkerAsset,
        UnableToGetAssetRepr,
        ImposibleToDecideWithdrawLiquidityAmounts,
        InvalidWithdrawLiquidityBasicAssetAmount,
        InvalidWithdrawLiquidityTargetAssetAmount,
        SourceBaseAmountIsTooLarge,
        SourceBalanceOfLiquidityTokensIsNotLargeEnough,
        DestinationBaseBalanceIsNotLargeEnough,
        DestinationTargetBalanceIsNotLargeEnough,
        InvalidAssetForLiquidityMarking,
        AssetDecodingError,
        CalculatedValueIsOutOfDesiredBounds,
        BaseAssetIsNotMatchedWithAnyAssetArguments,
        DestinationAmountMustBeSame,
        SourceAmountMustBeSame,
        PoolInitializationIsInvalid,
        PoolIsAlreadyInitialized,
        InvalidMinimumBoundValueOfBalance,
        ImposibleToDecideValidPairValuesFromRangeForThisPool,
        RangeValuesIsInvalid,
        CalculatedValueIsNotMeetsRequiredBoundaries,
        GettingFeeFromDestinationInImposible,
        FixedWrapperCalculationFailed,
        ThisCaseIsNotSupported,
    }
}

impl<T: Trait> Module<T> {
    pub fn get_xor_part_from_trading_pair(
        dex_id: T::DEXId,
        trading_pair: common::TradingPair<AssetIdOf<T>>,
        liq_amount: Balance,
    ) -> Result<Balance, DispatchError> {
        let (_, pool_acc) = Module::<T>::tech_account_from_dex_and_asset_pair(
            dex_id,
            trading_pair.base_asset_id,
            trading_pair.target_asset_id,
        )?;
        let pool_acc_sys = technical::Module::<T>::tech_account_id_to_account_id(&pool_acc)?;
        let b_in_pool =
            assets::Module::<T>::free_balance(&trading_pair.base_asset_id, &pool_acc_sys)?;
        let t_in_pool =
            assets::Module::<T>::free_balance(&trading_pair.target_asset_id, &pool_acc_sys)?;
        let fxw_b_in_pool: FixedWrapper = b_in_pool.into();
        let fxw_t_in_pool: FixedWrapper = t_in_pool.into();
        let fxw_liq_in_pool = fxw_b_in_pool.sqrt_accurate() * fxw_t_in_pool.sqrt_accurate();
        let fxw_liq_amount: FixedWrapper = liq_amount.into();
        let fxw_peace: FixedWrapper = fxw_liq_in_pool / fxw_liq_amount;
        let fxw_value: FixedWrapper = fxw_b_in_pool / fxw_peace;
        let value: Balance = (fxw_value
            .get()
            .ok_or(Error::<T>::FixedWrapperCalculationFailed)?)
        .into();
        Ok(value)
    }

    pub fn tech_account_from_dex_and_asset_pair(
        dex_id: T::DEXId,
        asset_a: T::AssetId,
        asset_b: T::AssetId,
    ) -> Result<(common::TradingPair<TechAssetIdOf<T>>, TechAccountIdOf<T>), DispatchError> {
        let dexinfo = <dex_manager::DEXInfos<T>>::get(dex_id);
        let base_asset_id = dexinfo.base_asset_id;
        ensure!(asset_a != asset_b, Error::<T>::AssetsMustNotBeSame);
        let ba = Module::<T>::try_decode_asset(base_asset_id)?;
        let ta = if base_asset_id == asset_a {
            Module::<T>::try_decode_asset(asset_b)?
        } else if base_asset_id == asset_b {
            Module::<T>::try_decode_asset(asset_a)?
        } else {
            Err(Error::<T>::BaseAssetIsNotMatchedWithAnyAssetArguments)?
        };
        let tpair = common::TradingPair::<TechAssetIdOf<T>> {
            base_asset_id: ba,
            target_asset_id: ta,
        };
        Ok((
            tpair,
            TechAccountIdOf::<T>::to_tech_unit_from_dex_and_trading_pair(dex_id, tpair),
        ))
    }

    fn get_bounds_from_swap_amount(
        swap_amount: SwapAmount<Balance>,
    ) -> Result<(Bounds<Balance>, Bounds<Balance>), DispatchError> {
        match swap_amount {
            SwapAmount::WithDesiredInput {
                desired_amount_in,
                min_amount_out,
            } => Ok((
                Bounds::Desired(desired_amount_in),
                Bounds::Min(min_amount_out),
            )),
            SwapAmount::WithDesiredOutput {
                desired_amount_out,
                max_amount_in,
            } => Ok((
                Bounds::Max(max_amount_in),
                Bounds::Desired(desired_amount_out),
            )),
        }
    }

    #[allow(dead_code)]
    fn get_bounded_asset_pair_for_liquidity(
        dex_id: T::DEXId,
        asset_a: T::AssetId,
        asset_b: T::AssetId,
        swap_amount_a: SwapAmount<Balance>,
        swap_amount_b: SwapAmount<Balance>,
    ) -> Result<
        (
            Bounds<Balance>,
            Bounds<Balance>,
            Bounds<Balance>,
            Bounds<Balance>,
            TechAccountIdOf<T>,
        ),
        DispatchError,
    > {
        let (_, tech_acc_id) =
            Module::<T>::tech_account_from_dex_and_asset_pair(dex_id, asset_a, asset_b)?;
        let (source_amount_a, destination_amount_a) =
            Module::<T>::get_bounds_from_swap_amount(swap_amount_a)?;
        let (source_amount_b, destination_amount_b) =
            Module::<T>::get_bounds_from_swap_amount(swap_amount_b)?;
        Ok((
            source_amount_a,
            destination_amount_a,
            source_amount_b,
            destination_amount_b,
            tech_acc_id,
        ))
    }

    pub fn initialize_pool_unchecked(
        _source: AccountIdOf<T>,
        dex_id: DEXIdOf<T>,
        asset_a: AssetIdOf<T>,
        asset_b: AssetIdOf<T>,
    ) -> Result<
        (
            common::TradingPair<TechAssetIdOf<T>>,
            TechAccountIdOf<T>,
            TechAccountIdOf<T>,
            TechAssetIdOf<T>,
        ),
        DispatchError,
    > {
        let (trading_pair, tech_acc_id) =
            Module::<T>::tech_account_from_dex_and_asset_pair(dex_id, asset_a, asset_b)?;
        let fee_acc_id = tech_acc_id.to_fee_account().unwrap();
        let mark_asset = Module::<T>::get_marking_asset(&tech_acc_id)?;
        // Function initialize_pools is usually called once, just quick check if tech
        // account is not registered is enough to do the job.
        // If function is called second time, than this is not usual case and additional checks
        // can be done, check every condition for `PoolIsAlreadyInitialized`.
        if technical::Module::<T>::ensure_tech_account_registered(&tech_acc_id).is_ok() {
            if technical::Module::<T>::ensure_tech_account_registered(&fee_acc_id).is_ok()
                && assets::Module::<T>::ensure_asset_exists(&mark_asset.into()).is_ok()
                && trading_pair::Module::<T>::ensure_trading_pair_exists(
                    &dex_id,
                    &trading_pair.target_asset_id.into(),
                )
                .is_ok()
            {
                Err(Error::<T>::PoolIsAlreadyInitialized)?;
            } else {
                Err(Error::<T>::PoolInitializationIsInvalid)?;
            }
        }
        technical::Module::<T>::register_tech_account_id(tech_acc_id.clone())?;
        technical::Module::<T>::register_tech_account_id(fee_acc_id.clone())?;
        Ok((trading_pair, tech_acc_id, fee_acc_id, mark_asset))
    }

    fn deposit_liquidity_unchecked(
        source: AccountIdOf<T>,
        dex_id: DEXIdOf<T>,
        input_asset_a: AssetIdOf<T>,
        input_asset_b: AssetIdOf<T>,
        input_a_desired: Balance,
        input_b_desired: Balance,
        input_a_min: Balance,
        input_b_min: Balance,
    ) -> DispatchResult {
        let (_, tech_acc_id) = Module::<T>::tech_account_from_dex_and_asset_pair(
            dex_id,
            input_asset_a,
            input_asset_b,
        )?;
        ensure!(
            input_a_desired >= input_a_min && input_b_desired >= input_b_min,
            Error::<T>::InvalidMinimumBoundValueOfBalance
        );
        let mark_asset = Module::<T>::get_marking_asset(&tech_acc_id)?;
        let action = PolySwapActionStructOf::<T>::DepositLiquidity(DepositLiquidityActionOf::<T> {
            client_account: None,
            receiver_account: None,
            pool_account: tech_acc_id,
            source: ResourcePair(
                Resource {
                    asset: input_asset_a,
                    amount: Bounds::<Balance>::RangeFromDesiredToMin(input_a_desired, input_a_min),
                },
                Resource {
                    asset: input_asset_b,
                    amount: Bounds::<Balance>::RangeFromDesiredToMin(input_b_desired, input_b_min),
                },
            ),
            destination: Resource {
                asset: mark_asset,
                amount: Bounds::Decide,
            },
            min_liquidity: None,
        });
        let action = T::PolySwapAction::from(action);
        let mut action = action.into();
        technical::Module::<T>::perform_create_swap(source, &mut action)?;
        Ok(())
    }

    fn withdraw_liquidity_unchecked(
        source: AccountIdOf<T>,
        dex_id: DEXIdOf<T>,
        output_asset_a: AssetIdOf<T>,
        output_asset_b: AssetIdOf<T>,
        marker_asset_desired: Balance,
        output_a_min: Balance,
        output_b_min: Balance,
    ) -> DispatchResult {
        let (_, tech_acc_id) = Module::<T>::tech_account_from_dex_and_asset_pair(
            dex_id,
            output_asset_a,
            output_asset_b,
        )?;
        let mark_asset = Module::<T>::get_marking_asset(&tech_acc_id)?;
        let action =
            PolySwapActionStructOf::<T>::WithdrawLiquidity(WithdrawLiquidityActionOf::<T> {
                client_account: None,
                receiver_account_a: None,
                receiver_account_b: None,
                pool_account: tech_acc_id,
                source: Resource {
                    asset: mark_asset,
                    amount: Bounds::Desired(marker_asset_desired),
                },
                destination: ResourcePair(
                    Resource {
                        asset: output_asset_a,
                        amount: Bounds::Min(output_a_min),
                    },
                    Resource {
                        asset: output_asset_b,
                        amount: Bounds::Min(output_b_min),
                    },
                ),
            });
        let action = T::PolySwapAction::from(action);
        let mut action = action.into();
        technical::Module::<T>::perform_create_swap(source, &mut action)?;
        Ok(())
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin
    {
        type Error = Error<T>;
        fn deposit_event() = default;

        #[weight = <T as Trait>::WeightInfo::swap_pair()]
        pub fn swap_pair(
            origin, receiver: AccountIdOf<T>, dex_id: DEXIdOf<T>,
            input_asset_id: AssetIdOf<T>, output_asset_id: AssetIdOf<T>,
            swap_amount: SwapAmount<Balance>,) -> DispatchResult {
            let source = ensure_signed(origin)?;
            Module::<T>::exchange(&source, &receiver, &dex_id, &input_asset_id, &output_asset_id, swap_amount)?;
            Ok(())
        }

        #[weight = <T as Trait>::WeightInfo::deposit_liquidity()]
        pub fn deposit_liquidity(
            origin,
            dex_id: DEXIdOf<T>,
            input_asset_a: AssetIdOf<T>,
            input_asset_b: AssetIdOf<T>,
            input_a_desired: Balance,
            input_b_desired: Balance,
            input_a_min: Balance,
            input_b_min: Balance,
        ) -> DispatchResult {
            let source = ensure_signed(origin)?;
            Module::<T>::deposit_liquidity_unchecked(source, dex_id,
                input_asset_a, input_asset_b, input_a_desired, input_b_desired, input_a_min, input_b_min)?;
            Ok(())
        }

        #[weight = <T as Trait>::WeightInfo::withdraw_liquidity()]
        pub fn withdraw_liquidity(
            origin,
            dex_id: DEXIdOf<T>,
            output_asset_a: AssetIdOf<T>,
            output_asset_b: AssetIdOf<T>,
            marker_asset_desired: Balance,
            output_a_min: Balance,
            output_b_min: Balance,
        ) -> DispatchResult {
            let source = ensure_signed(origin)?;
            Module::<T>::withdraw_liquidity_unchecked(source, dex_id,
                output_asset_a, output_asset_b, marker_asset_desired, output_a_min, output_b_min)?;
            Ok(())
        }

        #[weight = <T as Trait>::WeightInfo::initialize_pool()]
        pub fn initialize_pool(
            origin,
            dex_id: DEXIdOf<T>,
            asset_a: AssetIdOf<T>,
            asset_b: AssetIdOf<T>,
            ) -> DispatchResult
        {
                let source = ensure_signed(origin.clone())?;
                <T as Trait>::EnsureDEXOwner::ensure_dex_owner(&dex_id, origin.clone())?;
                let (_,tech_account_id, fees_account_id, mark_asset) = Module::<T>::initialize_pool_unchecked(source.clone(), dex_id, asset_a, asset_b)?;
                let mark_asset_repr: T::AssetId = mark_asset.into();
                assets::Module::<T>::register_asset_id(source.clone(), mark_asset_repr, AssetSymbol(b"XYKPOOL".to_vec()), 18)?;
                let ta_repr = technical::Module::<T>::tech_account_id_to_account_id(&tech_account_id)?;
                let fees_ta_repr = technical::Module::<T>::tech_account_id_to_account_id(&fees_account_id)?;
                // Minting permission is needed for technical account to mint markered tokens of
                // liquidity into account who deposit liquidity.
                permissions::Module::<T>::grant_permission_with_scope(
                   source.clone(),
                   ta_repr.clone(),
                   MINT,
                   Scope::Limited(hash(&Into::<AssetIdOf::<T>>::into(mark_asset.clone())))
                   )?;
                permissions::Module::<T>::grant_permission_with_scope(
                   source,
                   ta_repr.clone(),
                   BURN,
                   Scope::Limited(hash(&Into::<AssetIdOf::<T>>::into(mark_asset.clone())))
                   )?;
                Module::<T>::initialize_pool_properties(&asset_a, &asset_b, &ta_repr, &fees_ta_repr, &mark_asset_repr);
                pswap_distribution::Module::<T>::subscribe(fees_ta_repr, dex_id, mark_asset_repr, None)?;
                MarkerTokensIndex::<T>::mutate( |mti| {mti.insert(mark_asset_repr)});
                Self::deposit_event(RawEvent::PoolIsInitialized(ta_repr));
                Ok(())
        }


    }
}

impl<T: Trait> LiquiditySource<T::DEXId, T::AccountId, T::AssetId, Balance, DispatchError>
    for Module<T>
{
    fn can_exchange(
        dex_id: &T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
    ) -> bool {
        // Function clause is used here, because in this case it is other scope and it not
        // conflicted with bool type.
        let res = || {
            let (_, tech_acc_id) = Module::<T>::tech_account_from_dex_and_asset_pair(
                *dex_id,
                *input_asset_id,
                *output_asset_id,
            )?;
            let mut action = PolySwapActionStructOf::<T>::PairSwap(PairSwapActionOf::<T> {
                client_account: None,
                receiver_account: None,
                pool_account: tech_acc_id,
                source: Resource {
                    asset: *input_asset_id,
                    amount: Bounds::Dummy,
                },
                destination: Resource {
                    asset: *output_asset_id,
                    amount: Bounds::Dummy,
                },
                fee: None,
                fee_account: None,
                get_fee_from_destination: None,
            });
            common::SwapRulesValidation::<AccountIdOf<T>, TechAccountIdOf<T>, T>::
                        prepare_and_validate(&mut action, None)
        };
        res().is_ok()
    }

    fn quote(
        dex_id: &T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        swap_amount: SwapAmount<Balance>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        let (_, tech_acc_id) = Module::<T>::tech_account_from_dex_and_asset_pair(
            *dex_id,
            *input_asset_id,
            *output_asset_id,
        )?;
        let (source_amount, destination_amount) =
            Module::<T>::get_bounds_from_swap_amount(swap_amount)?;
        let mut action = PolySwapActionStructOf::<T>::PairSwap(PairSwapActionOf::<T> {
            client_account: None,
            receiver_account: None,
            pool_account: tech_acc_id,
            source: Resource {
                asset: *input_asset_id,
                amount: source_amount,
            },
            destination: Resource {
                asset: *output_asset_id,
                amount: destination_amount,
            },
            fee: None,
            fee_account: None,
            get_fee_from_destination: None,
        });
        common::SwapRulesValidation::<AccountIdOf<T>, TechAccountIdOf<T>, T>::prepare_and_validate(
            &mut action,
            None,
        )?;
        // It is garanty that unwrap is always ok.
        match action {
            PolySwapAction::PairSwap(a) => {
                let (fee, amount) = match swap_amount {
                    SwapAmount::WithDesiredInput {
                        desired_amount_in: _,
                        min_amount_out: _,
                    } => (a.fee.unwrap(), a.destination.amount.unwrap()),
                    SwapAmount::WithDesiredOutput {
                        desired_amount_out: _,
                        max_amount_in: _,
                    } => (a.fee.unwrap(), a.source.amount.unwrap()),
                };
                if a.get_fee_from_destination.unwrap() {
                    Ok(common::prelude::SwapOutcome::new(amount - fee, fee))
                } else {
                    Ok(common::prelude::SwapOutcome::new(amount, fee))
                }
            }
            _ => unreachable!("we know that always PairSwap is used"),
        }
    }

    fn exchange(
        sender: &T::AccountId,
        receiver: &T::AccountId,
        dex_id: &T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        swap_amount: SwapAmount<Balance>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        let (_, tech_acc_id) = Module::<T>::tech_account_from_dex_and_asset_pair(
            *dex_id,
            *input_asset_id,
            *output_asset_id,
        )?;
        let (source_amount, destination_amount) =
            Module::<T>::get_bounds_from_swap_amount(swap_amount)?;
        let mut action = PolySwapActionStructOf::<T>::PairSwap(PairSwapActionOf::<T> {
            client_account: None,
            receiver_account: Some(receiver.clone()),
            pool_account: tech_acc_id,
            source: Resource {
                asset: *input_asset_id,
                amount: source_amount,
            },
            destination: Resource {
                asset: *output_asset_id,
                amount: destination_amount,
            },
            fee: None,
            fee_account: None,
            get_fee_from_destination: None,
        });
        common::SwapRulesValidation::<AccountIdOf<T>, TechAccountIdOf<T>, T>::prepare_and_validate(
            &mut action,
            Some(sender),
        )?;
        // It is garanty that unwrap is always ok.
        let (fee, term_amount) = match action {
            PolySwapAction::PairSwap(ref a) => (a.fee.unwrap(), a.destination.amount.unwrap()),
            _ => unreachable!("we know that always PairSwap is used"),
        };
        let action = T::PolySwapAction::from(action);
        let mut action = action.into();
        technical::Module::<T>::perform_create_swap_unchecked(sender.clone(), &mut action)?;
        Ok(common::prelude::SwapOutcome::new(term_amount, fee))
    }
}
