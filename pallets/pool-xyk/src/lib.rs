#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use core::convert::TryInto;
use frame_support::dispatch::{DispatchError, DispatchResult};
use frame_support::traits::Get;
use frame_support::weights::Weight;
use frame_support::{dispatch, ensure, Parameter};
use frame_system::ensure_signed;
use sp_runtime::RuntimeDebug;
use sp_std::collections::btree_set::BTreeSet;

use common::prelude::{Balance, EnsureDEXManager, FixedWrapper, SwapAmount, SwapOutcome};
use common::{
    balance, hash, AssetSymbol, EnsureTradingPairExists, FromGenericPair, LiquiditySource,
    LiquiditySourceType, ManagementMode, SwapRulesValidation, ToFeeAccount,
    ToTechUnitFromDEXAndTradingPair,
};
use frame_support::debug;
use orml_traits::currency::MultiCurrency;
use permissions::{Scope, BURN, MINT};

mod weights;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

type ExtraAccountIdOf<T> = <T as assets::Config>::ExtraAccountId;

type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

type AssetIdOf<T> = <T as assets::Config>::AssetId;

type TechAssetIdOf<T> = <T as technical::Config>::TechAssetId;

type TechAccountIdOf<T> = <T as technical::Config>::TechAccountId;

type DEXIdOf<T> = <T as common::Config>::DEXId;

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

type DEXManager<T> = dex_manager::Module<T>;

const MIN_LIQUIDITY: u128 = 1000;

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

impl<T: Config> common::SwapRulesValidation<AccountIdOf<T>, TechAccountIdOf<T>, T>
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
            ensure!(balance_ss.unwrap() > 0, Error::<T>::AccountBalanceIsInvalid);
        }
        if balance_st == 0 && balance_tt == 0 {
            Err(Error::<T>::PoolIsEmpty)?;
        } else if balance_st <= 0 || balance_tt <= 0 {
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
                    ensure!(sa > 0, Error::<T>::ZeroValueInAmountParameter);
                    ensure!(ta > 0, Error::<T>::ZeroValueInAmountParameter);
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
                    ensure!(sa > 0, Error::<T>::ZeroValueInAmountParameter);
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
                            Err(Error::<T>::ImpossibleToDecideAssetPairAmounts)?;
                        }
                    }
                }
                // Case then destination amount is specified but source is not, it`s possible to decide it.
                (sa_bnd, Bounds::Desired(ta)) => {
                    ensure!(ta > 0, Error::<T>::ZeroValueInAmountParameter);
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
                            Err(Error::<T>::ImpossibleToDecideAssetPairAmounts)?;
                        }
                    }
                }
                // Case then no amount is specified, impossible to decide any amounts.
                (_, _) => {
                    Err(Error::<T>::ImpossibleToDecideAssetPairAmounts)?;
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
            let _fee = self.fee.unwrap();

            if !abstract_checking {
                // Checking that balances if correct and large enouth for amounts.
                if self.get_fee_from_destination.unwrap() {
                    // For source account balance must be not smaller than required with fee.
                    if balance_ss.unwrap() < source_amount {
                        Err(Error::<T>::SourceBalanceIsNotLargeEnough)?;
                    }

                    /*
                    TODO: find correct solution.
                    // For destination technical account balance must successful large for this swap.
                    if balance_tt - fee < destination_amount {
                        Err(Error::<T>::TargetBalanceIsNotLargeEnough)?;
                    }
                    if (self.destination.amount.unwrap() - self.fee.unwrap()) <= 0 {
                        Err(Error::<T>::GettingFeeFromDestinationIsImpossible)?;
                    }
                    */

                    if balance_tt < destination_amount {
                        Err(Error::<T>::TargetBalanceIsNotLargeEnough)?;
                    }
                } else {
                    /*
                    TODO: find correct solution.
                    // For source account balance must be not smaller than required with fee.
                    if balance_ss.unwrap() - fee < source_amount {
                        Err(Error::<T>::SourceBalanceIsNotLargeEnough)?;
                    }
                    */

                    if balance_ss.unwrap() < source_amount {
                        Err(Error::<T>::SourceBalanceIsNotLargeEnough)?;
                    }

                    // For destination technical account balance must successful large for this swap.
                    if balance_tt < destination_amount {
                        Err(Error::<T>::TargetBalanceIsNotLargeEnough)?;
                    }
                }
            }
        }
        if abstract_checking {
            return Ok(());
        }
        // This piece of code is called after validation, and every `Option` is `Some`, and it is safe to do
        // unwrap. `Bounds` is also safe to unwrap.
        // Also this computation of only things that is for security of pool, and not for applying values, so
        // this check can be simpler than actual transfering of values.
        let pool_is_valid_after_op_test = {
            let fxw_balance_st: FixedWrapper = balance_st.clone().into();
            let fxw_balance_tt: FixedWrapper = balance_tt.clone().into();
            let fxw_source_amount: FixedWrapper = self.source.amount.unwrap().into();
            let fxw_dest_amount: FixedWrapper = self.destination.amount.unwrap().into();
            let fxw_x = fxw_balance_st.clone() + fxw_source_amount;
            let fxw_y = fxw_balance_tt.clone() - fxw_dest_amount;
            let fxw_before = fxw_balance_st.clone() / fxw_balance_tt.clone();
            let fxw_after = fxw_x / fxw_y;
            let mut fxw_diff = fxw_after - fxw_before;
            fxw_diff = fxw_diff.clone() * fxw_diff.clone();
            let diff: u128 = fxw_diff
                .try_into_balance()
                .map_err(|_| Error::<T>::FixedWrapperCalculationFailed)?;
            let value = diff < balance!(100);
            if !value {
                debug::warn!(
                    "Potential swap operation is blocked because pool became invalid after it"
                );
            }
            value
        };
        ensure!(
            pool_is_valid_after_op_test,
            Error::<T>::PoolBecameInvalidAfterOperation
        );
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

impl<T: Config> common::SwapAction<AccountIdOf<T>, TechAccountIdOf<T>, T>
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

impl<T: Config> common::SwapRulesValidation<AccountIdOf<T>, TechAccountIdOf<T>, T>
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

        let mark_asset = Module::<T>::get_marking_asset(&self.pool_account)?;
        ensure!(
            self.destination.asset == mark_asset,
            Error::<T>::InvalidAssetForLiquidityMarking
        );

        let repr_k_asset_id = self.destination.asset.into();

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

        if !abstract_checking && (balance_bs.unwrap() <= 0 || balance_ts.unwrap() <= 0) {
            Err(Error::<T>::AccountBalanceIsInvalid)?;
        }

        // Balance of pool account for asset pair basic asset.
        let balance_bp =
            <assets::Module<T>>::free_balance(&self.source.0.asset, &pool_account_repr_sys)?;
        // Balance of pool account for asset pair target asset.
        let balance_tp =
            <assets::Module<T>>::free_balance(&self.source.1.asset, &pool_account_repr_sys)?;

        let mut empty_pool = false;
        if balance_bp == 0 && balance_tp == 0 {
            empty_pool = true;
        } else if balance_bp <= 0 {
            Err(Error::<T>::PoolIsInvalid)?;
        } else if balance_tp <= 0 {
            Err(Error::<T>::PoolIsInvalid)?;
        }

        #[allow(unused_variables)]
        let mut init_x = 0;
        #[allow(unused_variables)]
        let mut init_y = 0;
        if !abstract_checking && empty_pool {
            // Convertation from `Bounds` to `Option` is used here, and it is posible that value
            // None value returned from conversion.
            init_x = Option::<Balance>::from((self.source.0).amount)
                .ok_or(Error::<T>::InitialLiqudityDepositRatioMustBeDefined)?;
            init_y = Option::<Balance>::from((self.source.1).amount)
                .ok_or(Error::<T>::InitialLiqudityDepositRatioMustBeDefined)?;
        }

        // FixedWrapper version of variables.
        let fxw_balance_bp = FixedWrapper::from(balance_bp);
        let fxw_balance_tp = FixedWrapper::from(balance_tp);

        // Product of pool pair amounts to get k value.
        let (pool_k, fxw_pool_k) = {
            if empty_pool {
                if abstract_checking {
                    (None, None)
                } else {
                    let fxw_init_x = FixedWrapper::from(init_x);
                    let fxw_init_y: FixedWrapper = FixedWrapper::from(init_y);
                    let fxw_value: FixedWrapper = fxw_init_x.multiply_and_sqrt(&fxw_init_y);
                    let value = fxw_value
                        .clone()
                        .try_into_balance()
                        .map_err(|_| Error::<T>::FixedWrapperCalculationFailed)?;
                    (Some(value), Some(fxw_value))
                }
            } else {
                let fxw_value: FixedWrapper = fxw_balance_bp.multiply_and_sqrt(&fxw_balance_tp);
                let value = fxw_value
                    .clone()
                    .try_into_balance()
                    .map_err(|_| Error::<T>::FixedWrapperCalculationFailed)?;
                (Some(value), Some(fxw_value))
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
                        ensure!(destination_k > 0, Error::<T>::ZeroValueInAmountParameter);
                        let fxw_destination_k = FixedWrapper::from(init_x);
                        let fxw_piece_to_add = fxw_pool_k.unwrap() / fxw_destination_k;
                        let fxw_recom_x = fxw_balance_bp.clone() / fxw_piece_to_add.clone();
                        let fxw_recom_y = fxw_balance_tp.clone() / fxw_piece_to_add.clone();
                        let recom_x = fxw_recom_x
                            .try_into_balance()
                            .map_err(|_| Error::<T>::FixedWrapperCalculationFailed)?;
                        let recom_y = fxw_recom_y
                            .try_into_balance()
                            .map_err(|_| Error::<T>::FixedWrapperCalculationFailed)?;
                        match ox {
                            Bounds::Desired(x) => {
                                if x != recom_x {
                                    Err(Error::<T>::InvalidDepositLiquidityBasicAssetAmount)?
                                }
                            }
                            bounds => {
                                let value = (fxw_balance_bp / fxw_piece_to_add.clone())
                                    .try_into_balance()
                                    .map_err(|_| Error::<T>::FixedWrapperCalculationFailed)?;
                                let calc = Bounds::Calculated(value);
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
                                let value = (fxw_balance_tp / fxw_piece_to_add)
                                    .try_into_balance()
                                    .map_err(|_| Error::<T>::FixedWrapperCalculationFailed)?;
                                let calc = Bounds::Calculated(value);
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

                        let total_iss = assets::Module::<T>::total_issuance(&repr_k_asset_id)?;

                        let (calc_xdes, calc_ydes, calc_marker) =
                            Module::<T>::calc_deposit_liquidity_1(
                                total_iss, balance_bp, balance_tp, xdes, ydes, xmin, ymin,
                            )?;

                        self.source.0.amount = Bounds::Calculated(calc_xdes);
                        self.source.1.amount = Bounds::Calculated(calc_ydes);

                        match dest_amount {
                            Bounds::Desired(_) => {
                                return Err(Error::<T>::ThisCaseIsNotSupported.into());
                            }
                            _ => {
                                self.destination.amount = Bounds::Calculated(calc_marker);
                            }
                        }
                    }
                    // Case then no amount is specified (or something needed is not specified),
                    // impossible to decide any amounts.
                    (_, _, _) => {
                        Err(Error::<T>::ImpossibleToDecideDepositLiquidityAmounts)?;
                    }
                }
            }
        }

        // Recommended minimum liquidity, will be used if not specified or for checking if specified.
        let recom_min_liquidity = MIN_LIQUIDITY;
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

        //TODO: for abstract_checking, check that is enough liquidity in pool.
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

        if empty_pool {
            // Previous checks guarantee that unwrap and subtraction are safe.
            self.destination.amount =
                Bounds::Calculated(self.destination.amount.unwrap() - self.min_liquidity.unwrap());
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

impl<T: Config> common::SwapAction<AccountIdOf<T>, TechAccountIdOf<T>, T>
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

impl<T: Config> common::SwapRulesValidation<AccountIdOf<T>, TechAccountIdOf<T>, T>
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
        if balance_ks <= 0 {
            Err(Error::<T>::AccountBalanceIsInvalid)?;
        }

        // Balance of pool account for asset pair basic asset.
        let balance_bp =
            <assets::Module<T>>::free_balance(&(self.destination.0).asset, &pool_account_repr_sys)?;
        // Balance of pool account for asset pair target asset.
        let balance_tp =
            <assets::Module<T>>::free_balance(&(self.destination.1).asset, &pool_account_repr_sys)?;

        if balance_bp == 0 && balance_tp == 0 {
            Err(Error::<T>::PoolIsEmpty)?;
        } else if balance_bp <= 0 {
            Err(Error::<T>::PoolIsInvalid)?;
        } else if balance_tp <= 0 {
            Err(Error::<T>::PoolIsInvalid)?;
        }

        let fxw_balance_bp = FixedWrapper::from(balance_bp);
        let fxw_balance_tp = FixedWrapper::from(balance_tp);

        let total_iss = assets::Module::<T>::total_issuance(&repr_k_asset_id)?;
        // Adding min liquidity to pretend that initial provider has locked amount, which actually is not reflected in total supply.
        let fxw_total_iss = FixedWrapper::from(total_iss) + MIN_LIQUIDITY;

        match (
            self.source.amount,
            (self.destination.0).amount,
            (self.destination.1).amount,
        ) {
            (Bounds::Desired(source_k), ox, oy) => {
                ensure!(source_k > 0, Error::<T>::ZeroValueInAmountParameter);
                let fxw_source_k = FixedWrapper::from(source_k);
                // let fxw_piece_to_take = fxw_total_iss / fxw_source_k;
                let fxw_recom_x = fxw_balance_bp * fxw_source_k.clone() / fxw_total_iss.clone();
                let fxw_recom_y = fxw_balance_tp * fxw_source_k / fxw_total_iss;
                let recom_x: Balance = fxw_recom_x
                    .try_into_balance()
                    .map_err(|_| Error::<T>::FixedWrapperCalculationFailed)?;
                let recom_y = fxw_recom_y
                    .try_into_balance()
                    .map_err(|_| Error::<T>::FixedWrapperCalculationFailed)?;

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
                Err(Error::<T>::ImpossibleToDecideDepositLiquidityAmounts)?;
            }
        }

        // Get required values, now it is always Some, it is safe to unwrap().
        let _base_amount = (self.destination.1).amount.unwrap();
        let _target_amount = (self.destination.0).amount.unwrap();
        let source_amount = self.source.amount.unwrap();

        if balance_ks < source_amount {
            Err(Error::<T>::SourceBalanceOfLiquidityTokensIsNotLargeEnough)?;
        }

        //TODO: Debug why in this place checking is failed, but in transfer checks is success.
        /*
        // Checking that balances if correct and large enough for amounts.
        if balance_bp < base_amount {
            Err(Error::<T>::DestinationBaseBalanceIsNotLargeEnough)?;
        }
        if balance_tp < target_amount {
            Err(Error::<T>::DestinationTargetBalanceIsNotLargeEnough)?;
        }
        */
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

impl<T: Config> common::SwapAction<AccountIdOf<T>, TechAccountIdOf<T>, T>
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

impl<T: Config> common::SwapRulesValidation<AccountIdOf<T>, TechAccountIdOf<T>, T>
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

impl<T: Config> common::SwapAction<AccountIdOf<T>, TechAccountIdOf<T>, T>
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

impl<T: Config> Module<T> {
    fn initialize_pool_properties(
        dex_id: &T::DEXId,
        asset_a: &T::AssetId,
        asset_b: &T::AssetId,
        reserves_account_id: &T::AccountId,
        fees_account_id: &T::AccountId,
        marker_asset_id: &T::AssetId,
    ) -> DispatchResult {
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
        trading_pair::Module::<T>::enable_source_for_trading_pair(
            dex_id,
            sorted_asset_a,
            sorted_asset_b,
            LiquiditySourceType::XYKPool,
        )?;
        Properties::<T>::insert(
            sorted_asset_a,
            sorted_asset_b,
            (
                reserves_account_id.clone(),
                fees_account_id.clone(),
                marker_asset_id.clone(),
            ),
        );
        Ok(())
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
        use assets::AssetRecord::*;
        use assets::AssetRecordArg::*;
        use common::AssetIdExtraAssetRecordArg::*;
        let repr_extra: ExtraAccountIdOf<T> =
            technical::Module::<T>::tech_account_id_to_account_id(&tech_acc)?.into();
        let tag = GenericU128(common::hash_to_u128_pair(b"Marking asset").0);
        let lst_extra = Extra(LstId(common::LiquiditySourceType::XYKPool.into()).into());
        let acc_extra = Extra(AccountId(repr_extra).into());
        let asset_id =
            assets::Module::<T>::register_asset_id_from_tuple(&Arity3(tag, lst_extra, acc_extra));
        Ok(asset_id)
    }

    pub fn get_marking_asset(
        tech_acc: &TechAccountIdOf<T>,
    ) -> Result<TechAssetIdOf<T>, DispatchError> {
        let asset_id = Module::<T>::get_marking_asset_repr(tech_acc)?;
        asset_id
            .try_into()
            .map_err(|_| Error::<T>::UnableToConvertAssetToTechAssetId.into())
    }
}

impl<T: Config> Module<T> {
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
        let fxw_x_in = FixedWrapper::from(*x_in);
        //TODO: get this value from DEXInfo.
        let result =
            (fxw_x_in * FixedWrapper::from(balance!(3))) / FixedWrapper::from(balance!(1000));
        Ok(result
            .try_into_balance()
            .map_err(|_| Error::<T>::FixedWrapperCalculationFailed)?)
    }

    #[inline]
    pub fn get_fee_for_destination(
        _asset_id: &AssetIdOf<T>,
        _tech_acc: &TechAccountIdOf<T>,
        y_out: &Balance,
    ) -> Result<Balance, DispatchError> {
        let fxw_y_out = FixedWrapper::from(*y_out);
        //TODO: get this value from DEXInfo.
        let result =
            (fxw_y_out * FixedWrapper::from(balance!(3))) / FixedWrapper::from(balance!(1000));
        Ok(result
            .try_into_balance()
            .map_err(|_| Error::<T>::FixedWrapperCalculationFailed)?)
    }

    pub fn calculate_optimal_deposit(
        total_supply: Balance,
        reserve_a: Balance,
        reserve_b: Balance,
        amount_a_desired: Balance,
        amount_b_desired: Balance,
        amount_a_min: Balance,
        amount_b_min: Balance,
    ) -> Result<(Balance, Balance), DispatchError> {
        let _fxw_total_supply = FixedWrapper::from(total_supply);

        let fxw_am_a_des = FixedWrapper::from(amount_a_desired);
        let fxw_am_b_des = FixedWrapper::from(amount_b_desired);

        let fxw_reserve_a = FixedWrapper::from(reserve_a);
        let fxw_reserve_b = FixedWrapper::from(reserve_b);

        let fxw_opt_am_a_des: FixedWrapper =
            fxw_am_b_des.clone() / (fxw_reserve_b.clone() / fxw_reserve_a.clone());
        let fxw_opt_am_b_des: FixedWrapper =
            fxw_am_a_des.clone() / (fxw_reserve_a.clone() / fxw_reserve_b.clone());

        let opt_am_a_des = fxw_opt_am_a_des
            .try_into_balance()
            .map_err(|_| Error::<T>::FixedWrapperCalculationFailed)?;
        let opt_am_b_des = fxw_opt_am_b_des
            .try_into_balance()
            .map_err(|_| Error::<T>::FixedWrapperCalculationFailed)?;

        if opt_am_b_des <= amount_b_desired {
            ensure!(
                opt_am_b_des >= amount_b_min,
                Error::<T>::ImpossibleToDecideValidPairValuesFromRangeForThisPool
            );
            Ok((amount_a_desired, opt_am_b_des))
        } else {
            ensure!(
                opt_am_a_des >= amount_a_min && opt_am_a_des <= amount_a_desired,
                Error::<T>::ImpossibleToDecideValidPairValuesFromRangeForThisPool
            );
            Ok((opt_am_a_des, amount_b_desired))
        }
    }

    pub fn calc_deposit_liquidity_1(
        total_supply: Balance,
        reserve_a: Balance,
        reserve_b: Balance,
        amount_a_desired: Balance,
        amount_b_desired: Balance,
        amount_a_min: Balance,
        amount_b_min: Balance,
    ) -> Result<(Balance, Balance, Balance), DispatchError> {
        let (am_a_des, am_b_des) = Module::<T>::calculate_optimal_deposit(
            total_supply,
            reserve_a,
            reserve_b,
            amount_a_desired,
            amount_b_desired,
            amount_a_min,
            amount_b_min,
        )?;
        let fxw_am_a_des = FixedWrapper::from(am_a_des);
        let fxw_am_b_des = FixedWrapper::from(am_b_des);
        let fxw_reserve_a = FixedWrapper::from(reserve_a);
        let fxw_reserve_b = FixedWrapper::from(reserve_b);
        let fxw_total_supply = FixedWrapper::from(total_supply);
        let fxw_lhs = fxw_am_a_des.clone() / (fxw_reserve_a.clone() / fxw_total_supply.clone());
        let fxw_rhs = fxw_am_b_des.clone() / (fxw_reserve_b.clone() / fxw_total_supply.clone());
        let lhs: Balance = fxw_lhs
            .try_into_balance()
            .map_err(|_| Error::<T>::FixedWrapperCalculationFailed)?;
        let rhs = fxw_rhs
            .try_into_balance()
            .map_err(|_| Error::<T>::FixedWrapperCalculationFailed)?;
        let min_value = lhs.min(rhs);
        Ok((am_a_des, am_b_des, min_value))
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
        let fxw_x = FixedWrapper::from(x.clone());
        let fxw_y = FixedWrapper::from(y.clone());
        let fxw_x_in = FixedWrapper::from(x_in.clone());
        if get_fee_from_destination {
            Module::<T>::guard_fee_from_destination(asset_a, asset_b)?;
            //let fxw_y1 = (fxw_x_in.clone() * fxw_y) / (fxw_x + fxw_x_in);
            let fxw_y1 = fxw_x_in.clone() / ((fxw_x + fxw_x_in) / fxw_y);
            let y1 = fxw_y1
                .try_into_balance()
                .map_err(|_| Error::<T>::FixedWrapperCalculationFailed)?;
            let fee_of_y1 = Module::<T>::get_fee_for_destination(asset_a, tech_acc, &y1)?;
            Ok((y1, fee_of_y1))
        } else {
            Module::<T>::guard_fee_from_source(asset_a, asset_b)?;
            let fee_of_x_in = Module::<T>::get_fee_for_source(asset_a, tech_acc, x_in)?;
            let fxw_fee_of_x_in = FixedWrapper::from(fee_of_x_in);
            let fxw_x_in_subfee = fxw_x_in - fxw_fee_of_x_in;
            //TODO: this comments exist now for comparation of multiplication version, please remove it
            //than precision problems will finally set to best solution.
            //let fxw_y_out = (fxw_x_in_subfee.clone() * fxw_y) / (fxw_x + fxw_x_in_subfee);
            let fxw_y_out = fxw_x_in_subfee.clone() / ((fxw_x + fxw_x_in_subfee) / fxw_y);
            let y_out = fxw_y_out
                .try_into_balance()
                .map_err(|_| Error::<T>::FixedWrapperCalculationFailed)?;
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
        let fxw_x = FixedWrapper::from(x.clone());
        let fxw_y = FixedWrapper::from(y.clone());
        let fxw_y_out = FixedWrapper::from(y_out.clone());
        if get_fee_from_destination {
            Module::<T>::guard_fee_from_destination(asset_a, asset_b)?;
            let unit = balance!(1);
            let fract_a: Balance = Module::<T>::get_fee_for_destination(asset_a, tech_acc, &unit)?;
            let fract_b: Balance = unit - fract_a;
            let fxw_fract_b = FixedWrapper::from(fract_b);
            let fxw_y1 = fxw_y_out.clone() / fxw_fract_b;
            //let fxw_x_in = (fxw_x * fxw_y1.clone()) / (fxw_y - fxw_y1.clone());
            let fxw_x_in = fxw_x / ((fxw_y - fxw_y1.clone()) / fxw_y1.clone());
            let fxw_fee = fxw_y1 - fxw_y_out;
            let x_in = fxw_x_in
                .try_into_balance()
                .map_err(|_| Error::<T>::FixedWrapperCalculationFailed)?;
            let fee = fxw_fee
                .try_into_balance()
                .map_err(|_| Error::<T>::FixedWrapperCalculationFailed)?;
            Ok((x_in, fee))
        } else {
            Module::<T>::guard_fee_from_source(asset_a, asset_b)?;
            let y_minus_y_out = *y - *y_out;
            let ymyo_fee = Module::<T>::get_fee_for_source(asset_a, tech_acc, &y_minus_y_out)?;
            let ymyo_subfee = y_minus_y_out - ymyo_fee;
            let fxw_ymyo_subfee = FixedWrapper::from(ymyo_subfee);
            //TODO: this comments exist now for comparation of multiplication version, please remove it
            //than precision problems will finally set to best solution.
            //let fxw_x_in = (fxw_x * fxw_y_out) / fxw_ymyo_subfee;
            let fxw_x_in = fxw_x / (fxw_ymyo_subfee / fxw_y_out);
            let x_in = fxw_x_in
                .try_into_balance()
                .map_err(|_| Error::<T>::FixedWrapperCalculationFailed)?;
            let fee = Module::<T>::get_fee_for_source(asset_a, tech_acc, &x_in)?;
            Ok((x_in, fee))
        }
    }

    pub fn get_min_liquidity_for(
        _asset_id: AssetIdOf<T>,
        _tech_acc: &TechAccountIdOf<T>,
    ) -> Balance {
        //TODO: get this value from DEXInfo.
        1000
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

impl<T: Config> Module<T> {
    pub fn get_xor_part_from_pool_account(
        pool_acc: T::AccountId,
        liq_amount: Balance,
    ) -> Result<Balance, DispatchError> {
        let tech_acc = technical::Module::<T>::lookup_tech_account_id(&pool_acc)?;
        let trading_pair = match tech_acc.into() {
            common::TechAccountId::Pure(_, common::TechPurpose::LiquidityKeeper(trading_pair)) => {
                trading_pair
            }
            _ => {
                return Err(Error::<T>::UnableToGetXORPartFromMarkerAsset.into());
            }
        };
        let b_in_pool =
            assets::Module::<T>::free_balance(&trading_pair.base_asset_id.into(), &pool_acc)?;
        let t_in_pool =
            assets::Module::<T>::free_balance(&trading_pair.target_asset_id.into(), &pool_acc)?;
        let fxw_b_in_pool = FixedWrapper::from(b_in_pool);
        let fxw_t_in_pool = FixedWrapper::from(t_in_pool);
        let fxw_liq_in_pool = fxw_b_in_pool.multiply_and_sqrt(&fxw_t_in_pool);
        let fxw_liq_amount = FixedWrapper::from(liq_amount);
        let fxw_piece = fxw_liq_in_pool / fxw_liq_amount;
        let fxw_value = fxw_b_in_pool / fxw_piece;
        let value = fxw_value
            .try_into_balance()
            .map_err(|_| Error::<T>::FixedWrapperCalculationFailed)?;
        Ok(value)
    }

    pub fn tech_account_from_dex_and_asset_pair(
        dex_id: T::DEXId,
        asset_a: T::AssetId,
        asset_b: T::AssetId,
    ) -> Result<(common::TradingPair<TechAssetIdOf<T>>, TechAccountIdOf<T>), DispatchError> {
        let dexinfo = DEXManager::<T>::get_dex_info(&dex_id)?;
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
                    &trading_pair.base_asset_id.into(),
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

impl<T: Config> LiquiditySource<T::DEXId, T::AccountId, T::AssetId, Balance, DispatchError>
    for Module<T>
{
    fn can_exchange(
        dex_id: &T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
    ) -> bool {
        common::with_benchmark("pool-xyk.can_exchange", || {
            // Function clause is used here, because in this case it is other scope and it not
            // conflicted with bool type.
            let res = || {
                let tech_acc_id = T::TechAccountId::from_generic_pair(
                    "PoolXYK".into(),
                    "CanExchangeOperation".into(),
                );
                //TODO: Account registration is not needed to do operation, is this ok?
                //Technical::register_tech_account_id(tech_acc_id)?;
                let repr = technical::Module::<T>::tech_account_id_to_account_id(&tech_acc_id)?;
                //FIXME: Use special max variable that is good for this operation.
                T::Currency::deposit(input_asset_id.clone(), &repr, balance!(999999999))?;
                let swap_amount = common::prelude::SwapAmount::WithDesiredInput {
                    //FIXME: Use special max variable that is good for this operation.
                    desired_amount_in: balance!(0.000000001),
                    min_amount_out: 0,
                };
                Module::<T>::exchange(
                    &repr,
                    &repr,
                    dex_id,
                    input_asset_id,
                    output_asset_id,
                    swap_amount,
                )?;
                Ok(())
            };
            frame_support::storage::with_transaction(|| {
                let v: DispatchResult = res();
                sp_runtime::TransactionOutcome::Rollback(v.is_ok())
            })
        })
    }

    fn quote(
        dex_id: &T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        swap_amount: SwapAmount<Balance>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        common::with_benchmark("pool-xyk.quote", || {
            let res = || {
                let tech_acc_id =
                    T::TechAccountId::from_generic_pair("PoolXYK".into(), "QuoteOperation".into());
                //TODO: Account registration is not needed to do operation, is this ok?
                //Technical::register_tech_account_id(tech_acc_id)?;
                let repr = technical::Module::<T>::tech_account_id_to_account_id(&tech_acc_id)?;
                //FIXME: Use special max variable that is good for this operation.
                T::Currency::deposit(input_asset_id.clone(), &repr, balance!(999999999))?;
                Module::<T>::exchange(
                    &repr,
                    &repr,
                    dex_id,
                    input_asset_id,
                    output_asset_id,
                    swap_amount,
                )
            };
            frame_support::storage::with_transaction(|| {
                let v: Result<SwapOutcome<Balance>, DispatchError> = res();
                sp_runtime::TransactionOutcome::Rollback(v)
            })
        })
    }

    fn exchange(
        sender: &T::AccountId,
        receiver: &T::AccountId,
        dex_id: &T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        swap_amount: SwapAmount<Balance>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        common::with_benchmark("pool-xyk.exchange", || {
            let (_, tech_acc_id) = Module::<T>::tech_account_from_dex_and_asset_pair(
                *dex_id,
                *input_asset_id,
                *output_asset_id,
            )?;
            let (source_amount, destination_amount) =
                Module::<T>::get_bounds_from_swap_amount(swap_amount.clone())?;
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

            // It is guarantee that unwrap is always ok.
            // Clone is used here because action is used for perform_create_swap_unchecked.
            let retval = match action.clone() {
                PolySwapAction::PairSwap(a) => {
                    let mut desired_in = false;
                    let (fee, amount) = match swap_amount {
                        SwapAmount::WithDesiredInput { .. } => {
                            desired_in = true;
                            (a.fee.unwrap(), a.destination.amount.unwrap())
                        }
                        SwapAmount::WithDesiredOutput { .. } => {
                            (a.fee.unwrap(), a.source.amount.unwrap())
                        }
                    };
                    if a.get_fee_from_destination.unwrap() && desired_in {
                        Ok(common::prelude::SwapOutcome::new(amount - fee, fee))
                    } else {
                        Ok(common::prelude::SwapOutcome::new(amount, fee))
                    }
                }
                _ => unreachable!("we know that always PairSwap is used"),
            };

            let action = T::PolySwapAction::from(action);
            let mut action = action.into();
            technical::Module::<T>::perform_create_swap_unchecked(sender.clone(), &mut action)?;

            retval
        })
    }
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::config]
    pub trait Config:
        frame_system::Config
        + technical::Config
        + dex_manager::Config
        + trading_pair::Config
        + pswap_distribution::Config
    {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        //TODO: implement and use + Into<SwapActionOf<T> for this types.
        type PairSwapAction: common::SwapAction<AccountIdOf<Self>, TechAccountIdOf<Self>, Self>
            + Parameter;
        type DepositLiquidityAction: common::SwapAction<AccountIdOf<Self>, TechAccountIdOf<Self>, Self>
            + Parameter;
        type WithdrawLiquidityAction: common::SwapAction<AccountIdOf<Self>, TechAccountIdOf<Self>, Self>
            + Parameter;
        type PolySwapAction: common::SwapAction<AccountIdOf<Self>, TechAccountIdOf<Self>, Self>
            + Parameter
            + Into<<Self as technical::Config>::SwapAction>
            + From<PolySwapActionStructOf<Self>>;
        type EnsureDEXManager: EnsureDEXManager<Self::DEXId, Self::AccountId, DispatchError>;

        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(<T as Config>::WeightInfo::swap_pair())]
        pub fn swap_pair(
            origin: OriginFor<T>,
            receiver: AccountIdOf<T>,
            dex_id: DEXIdOf<T>,
            input_asset_id: AssetIdOf<T>,
            output_asset_id: AssetIdOf<T>,
            swap_amount: SwapAmount<Balance>,
        ) -> DispatchResultWithPostInfo {
            let source = ensure_signed(origin)?;
            Module::<T>::exchange(
                &source,
                &receiver,
                &dex_id,
                &input_asset_id,
                &output_asset_id,
                swap_amount,
            )?;
            Ok(().into())
        }

        #[pallet::weight(<T as Config>::WeightInfo::deposit_liquidity())]
        pub fn deposit_liquidity(
            origin: OriginFor<T>,
            dex_id: DEXIdOf<T>,
            input_asset_a: AssetIdOf<T>,
            input_asset_b: AssetIdOf<T>,
            input_a_desired: Balance,
            input_b_desired: Balance,
            input_a_min: Balance,
            input_b_min: Balance,
        ) -> DispatchResultWithPostInfo {
            common::with_benchmark("pool-xyk.deposit_liquidity", || {
                let source = ensure_signed(origin)?;
                Module::<T>::deposit_liquidity_unchecked(
                    source,
                    dex_id,
                    input_asset_a,
                    input_asset_b,
                    input_a_desired,
                    input_b_desired,
                    input_a_min,
                    input_b_min,
                )?;
                Ok(().into())
            })
        }

        #[pallet::weight(<T as Config>::WeightInfo::withdraw_liquidity())]
        pub fn withdraw_liquidity(
            origin: OriginFor<T>,
            dex_id: DEXIdOf<T>,
            output_asset_a: AssetIdOf<T>,
            output_asset_b: AssetIdOf<T>,
            marker_asset_desired: Balance,
            output_a_min: Balance,
            output_b_min: Balance,
        ) -> DispatchResultWithPostInfo {
            common::with_benchmark("pool-xyk.withdraw_liquidity", || {
                let source = ensure_signed(origin)?;
                Module::<T>::withdraw_liquidity_unchecked(
                    source,
                    dex_id,
                    output_asset_a,
                    output_asset_b,
                    marker_asset_desired,
                    output_a_min,
                    output_b_min,
                )?;
                Ok(().into())
            })
        }

        #[pallet::weight(<T as Config>::WeightInfo::initialize_pool())]
        pub fn initialize_pool(
            origin: OriginFor<T>,
            dex_id: DEXIdOf<T>,
            asset_a: AssetIdOf<T>,
            asset_b: AssetIdOf<T>,
        ) -> DispatchResultWithPostInfo {
            common::with_benchmark("pool-xyk.initialize_pool", || {
                common::with_transaction(|| {
                    let source = ensure_signed(origin.clone())?;
                    <T as Config>::EnsureDEXManager::ensure_can_manage(
                        &dex_id,
                        origin.clone(),
                        ManagementMode::Public,
                    )?;
                    let (_, tech_account_id, fees_account_id, mark_asset) =
                        Module::<T>::initialize_pool_unchecked(
                            source.clone(),
                            dex_id,
                            asset_a,
                            asset_b,
                        )?;
                    let mark_asset_repr: T::AssetId = mark_asset.into();
                    assets::Module::<T>::register_asset_id(
                        source.clone(),
                        mark_asset_repr,
                        AssetSymbol(b"XYKPOOL".to_vec()),
                        18,
                        0,
                        true,
                    )?;
                    let ta_repr =
                        technical::Module::<T>::tech_account_id_to_account_id(&tech_account_id)?;
                    let fees_ta_repr =
                        technical::Module::<T>::tech_account_id_to_account_id(&fees_account_id)?;
                    // Minting permission is needed for technical account to mint markered tokens of
                    // liquidity into account who deposit liquidity.
                    permissions::Module::<T>::grant_permission_with_scope(
                        source.clone(),
                        ta_repr.clone(),
                        MINT,
                        Scope::Limited(hash(&Into::<AssetIdOf<T>>::into(mark_asset.clone()))),
                    )?;
                    permissions::Module::<T>::grant_permission_with_scope(
                        source,
                        ta_repr.clone(),
                        BURN,
                        Scope::Limited(hash(&Into::<AssetIdOf<T>>::into(mark_asset.clone()))),
                    )?;
                    Module::<T>::initialize_pool_properties(
                        &dex_id,
                        &asset_a,
                        &asset_b,
                        &ta_repr,
                        &fees_ta_repr,
                        &mark_asset_repr,
                    )?;
                    pswap_distribution::Module::<T>::subscribe(
                        fees_ta_repr,
                        dex_id,
                        mark_asset_repr,
                        None,
                    )?;
                    MarkerTokensIndex::<T>::mutate(|mti| mti.insert(mark_asset_repr));
                    Self::deposit_event(Event::PoolIsInitialized(ta_repr));
                    Ok(().into())
                })
            })
        }
    }

    #[pallet::event]
    #[pallet::metadata(AccountIdOf<T> = "AccountId")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        // New pool for particular pair was initialized. [Reserves Account Id]
        PoolIsInitialized(AccountIdOf<T>),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// It is impossible to calculate fee for some pair swap operation, or other operation.
        UnableToCalculateFee,
        /// Is is impossible to get balance.
        UnableToGetBalance,
        /// Impossible to decide asset pair amounts.
        ImpossibleToDecideAssetPairAmounts,
        /// Pool pair ratio and pair swap ratio are different.
        PoolPairRatioAndPairSwapRatioIsDifferent,
        /// Pair swap action fee is smaller than recommended.
        PairSwapActionFeeIsSmallerThanRecommended,
        /// Source balance is not large enough.
        SourceBalanceIsNotLargeEnough,
        /// Target balance is not large enough.
        TargetBalanceIsNotLargeEnough,
        /// It is not possible to derive fee account.
        UnableToDeriveFeeAccount,
        /// The fee account is invalid.
        FeeAccountIsInvalid,
        /// Source and client accounts do not match as equal.
        SourceAndClientAccountDoNotMatchAsEqual,
        /// In this case assets must not be same.
        AssetsMustNotBeSame,
        /// Impossible to decide deposit liquidity amounts.
        ImpossibleToDecideDepositLiquidityAmounts,
        /// Invalid deposit liquidity base asset amount.
        InvalidDepositLiquidityBasicAssetAmount,
        /// Invalid deposit liquidity target asset amount.
        InvalidDepositLiquidityTargetAssetAmount,
        /// Pair swap action minimum liquidity is smaller than recommended.
        PairSwapActionMinimumLiquidityIsSmallerThanRecommended,
        /// Destination amount of liquidity is not large enough.
        DestinationAmountOfLiquidityIsNotLargeEnough,
        /// Source base amount is not large enough.
        SourceBaseAmountIsNotLargeEnough,
        /// Target base amount is not large enough.
        TargetBaseAmountIsNotLargeEnough,
        /// The balance structure of pool is invalid.
        PoolIsInvalid,
        /// The pool has empty liquidity.
        PoolIsEmpty,
        /// Amount parameter has zero value, it is invalid.
        ZeroValueInAmountParameter,
        /// The account balance is invalid.
        AccountBalanceIsInvalid,
        /// Invalid deposit liquidity destination amount.
        InvalidDepositLiquidityDestinationAmount,
        /// Initial liquidity deposit ratio must be defined.
        InitialLiqudityDepositRatioMustBeDefined,
        /// Technical asset is not representable.
        TechAssetIsNotRepresentable,
        /// Unable or impossible to decide marker asset.
        UnableToDecideMarkerAsset,
        /// Unable or impossible to get asset representation.
        UnableToGetAssetRepr,
        /// Impossible to decide withdraw liquidity amounts.
        ImpossibleToDecideWithdrawLiquidityAmounts,
        /// Invalid withdraw liquidity base asset amount.
        InvalidWithdrawLiquidityBasicAssetAmount,
        /// Invalud withdraw liquidity target asset amount.
        InvalidWithdrawLiquidityTargetAssetAmount,
        /// Source base amount is too large.
        SourceBaseAmountIsTooLarge,
        /// Source balance of liquidity is not large enough.
        SourceBalanceOfLiquidityTokensIsNotLargeEnough,
        /// Destination base balance is not large enough.
        DestinationBaseBalanceIsNotLargeEnough,
        /// Destination base balance is not large enough.
        DestinationTargetBalanceIsNotLargeEnough,
        /// Asset for liquidity marking is invalid.
        InvalidAssetForLiquidityMarking,
        /// Error in asset decoding.
        AssetDecodingError,
        /// Calculated value is out of desired bounds.
        CalculatedValueIsOutOfDesiredBounds,
        /// The base asset is not matched with any asset arguments.
        BaseAssetIsNotMatchedWithAnyAssetArguments,
        /// Some values need to be same, the destination amount must be same.
        DestinationAmountMustBeSame,
        /// Some values need to be same, the source amount must be same.
        SourceAmountMustBeSame,
        /// The pool initialization is invalid and has failed.
        PoolInitializationIsInvalid,
        /// The pool is already initialized.
        PoolIsAlreadyInitialized,
        /// The minimum bound values of balance are invalid.
        InvalidMinimumBoundValueOfBalance,
        /// It is impossible to decide valid pair values from range for this pool.
        ImpossibleToDecideValidPairValuesFromRangeForThisPool,
        /// This range values is not validy by rules of correct range.
        RangeValuesIsInvalid,
        /// The values that is calculated is out out of required bounds.
        CalculatedValueIsNotMeetsRequiredBoundaries,
        /// In this case getting fee from destination is impossible.
        GettingFeeFromDestinationIsImpossible,
        /// Math calculation with fixed number has failed to complete.
        FixedWrapperCalculationFailed,
        /// This case if not supported by logic of pool of validation code.
        ThisCaseIsNotSupported,
        /// Pool becomes invalid after operation.
        PoolBecameInvalidAfterOperation,
        /// Unable to convert asset to tech asset id.
        UnableToConvertAssetToTechAssetId,
        /// Unable to get XOR part from marker asset.
        UnableToGetXORPartFromMarkerAsset,
    }

    /// Updated after last liquidity change operation.
    /// [Base Asset Id (XOR) -> Target Asset Id => (Base Balance, Target Balance)].
    /// This storage records is not used as source of information, but used as quick cache for
    /// information that comes from balances for assets from technical accounts.
    /// For example, communication with technical accounts and their storage is not needed, and this
    /// pair to balance cache can be used quickly.
    #[pallet::storage]
    #[pallet::getter(fn reserves)]
    pub type Reserves<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AssetId,
        Blake2_128Concat,
        T::AssetId,
        (Balance, Balance),
        ValueQuery,
    >;

    /// Collection of all registered marker tokens.
    #[pallet::storage]
    #[pallet::getter(fn marker_tokens_index)]
    pub type MarkerTokensIndex<T: Config> = StorageValue<_, BTreeSet<T::AssetId>, ValueQuery>;

    /// Properties of particular pool. [Reserves Account Id, Fees Account Id, Marker Asset Id]
    #[pallet::storage]
    #[pallet::getter(fn properties)]
    pub type Properties<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AssetId,
        Blake2_128Concat,
        T::AssetId,
        (T::AccountId, T::AccountId, T::AssetId),
    >;
}
