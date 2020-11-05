#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{decl_error, decl_event, decl_module, decl_storage, dispatch};
use frame_system::ensure_signed;

use codec::{Decode, Encode};
use core::convert::TryInto;
use frame_support::weights::Weight;
use frame_support::Parameter;
use sp_runtime::RuntimeDebug;

use common::{
    prelude::{Balance, Error as CommonError, SwapAmount, SwapOutcome},
    EnsureTradingPairExists, LiquiditySource,
};
use frame_support::traits::Get;

use frame_support::dispatch::{DispatchError, DispatchResult};

use common::SwapRulesValidation;
use common::ToFeeAccount;
use common::ToTechUnitFromDEXAndTradingPair;
use frame_support::ensure;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

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
    Calculated(Balance),
    Desired(Balance),
    Min(Balance),
    Max(Balance),
    Decide,
    Dummy,
}

impl<Balance> Bounds<Balance> {
    fn unwrap(self) -> Balance {
        match self {
            Bounds::Calculated(a) => a,
            Bounds::Desired(a) => a,
            _ => unreachable!("Must not happen, every uncalculated bound must be set in prepare_and_validate function"),
        }
    }
}

impl<Balance> From<Bounds<Balance>> for Option<Balance> {
    fn from(bounds: Bounds<Balance>) -> Self {
        match bounds {
            Bounds::Calculated(a) => Some(a),
            Bounds::Desired(a) => Some(a),
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

/// Configure the pallet by specifying the parameters and types on which it depends.
pub trait Trait: technical::Trait + dex_manager::Trait + trading_pair::Trait {
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
}

impl<T: Trait> common::SwapRulesValidation<AccountIdOf<T>, TechAccountIdOf<T>, T>
    for PairSwapAction<AssetIdOf<T>, Balance, AccountIdOf<T>, TechAccountIdOf<T>>
{
    fn is_abstract_checking(&self) -> bool {
        self.source.amount == Bounds::Dummy || self.destination.amount == Bounds::Dummy
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
        Module::<T>::is_pool_account_valid_for(
            self.source.asset.clone(),
            self.pool_account.clone(),
        )?;

        // Source balance of source account.
        let balance_ss = if abstract_checking {
            None
        } else {
            Some(<assets::Module<T>>::free_balance(
                &(self.source.asset),
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

        if !abstract_checking {
            // Calculate pair ratio of pool, and check or correct amount of pair swap action.
            // Here source technical is divided by destination technical.
            let ratio_a = balance_st / balance_tt;

            match (self.source.amount, self.destination.amount) {
                // Case then both source and destination amounts is specified, just checking it.
                (Bounds::Desired(sa), Bounds::Desired(ta)) => {
                    ensure!(sa > 0_u32.into(), Error::<T>::ZeroValueInAmountParameter);
                    ensure!(ta > 0_u32.into(), Error::<T>::ZeroValueInAmountParameter);
                    let ratio_b = sa / ta;
                    if ratio_a != ratio_b {
                        Err(Error::<T>::PoolPairRatioAndPairSwapRatioIsDifferent)?;
                    }
                }
                // Case then source amount is specified but destination is not, it`s possible to decide it.
                (Bounds::Desired(sa), ta_bnd) => {
                    ensure!(sa > 0_u32.into(), Error::<T>::ZeroValueInAmountParameter);
                    let candidate = sa / ratio_a;
                    match ta_bnd {
                        Bounds::Min(ta_min) => {
                            ensure!(
                                candidate >= ta_min,
                                Error::<T>::CalculatedValueIsOutOfDesiredBounds
                            );
                        }
                        _ => (),
                    }
                    self.destination.amount = Bounds::Calculated(candidate);
                }
                // Case then destination amount is specified but source is not, it`s possible to decide it.
                (sa_bnd, Bounds::Desired(ta)) => {
                    ensure!(ta > 0_u32.into(), Error::<T>::ZeroValueInAmountParameter);
                    let candidate = ta * ratio_a;
                    match sa_bnd {
                        Bounds::Max(sa_max) => {
                            ensure!(
                                candidate <= sa_max,
                                Error::<T>::CalculatedValueIsOutOfDesiredBounds
                            );
                        }
                        _ => (),
                    }
                    self.source.amount = Bounds::Calculated(candidate);
                }
                // Case then no amount is specified, imposible to decide any amounts.
                (_, _) => {
                    Err(Error::<T>::ImposibleToDecideAssetPairAmounts)?;
                }
            }
        }

        // Check fee account if it is specified, or set it if not.
        match self.fee_account.clone() {
            Some(fa) => {
                // Checking that fee account is valid for this set of parameters.
                Module::<T>::is_fee_account_valid_for(
                    self.source.asset.clone(),
                    self.pool_account.clone(),
                    fa,
                )?;
            }
            None => {
                let fa = Module::<T>::get_fee_account(self.pool_account.clone())?;
                self.fee_account = Some(fa);
            }
        }
        // Recommended fee, will be used if fee is not specified or for checking if specified.
        let recom_fee =
            Module::<T>::get_fee_for(self.source.asset.clone(), self.pool_account.clone());
        // Set recommended or check that fee is correct.
        match self.fee {
            // Just set it here if it not specified, this is usual case.
            None => {
                self.fee = Some(recom_fee);
            }
            // Case with source user fee is set, checking that it is not smaller.
            Some(fee) => {
                if fee < recom_fee {
                    Err(Error::<T>::PairSwapActionFeeIsSmallerThanRecommended)?
                }
            }
        }
        if !abstract_checking {
            // Get required values, now it is always Some, it is safe to unwrap().
            let fee = self.fee.unwrap();
            let source_amount = self.source.amount.unwrap();
            let destination_amount = self.destination.amount.unwrap();
            // Checking that balances if correct and large enouth for amounts.
            // For source account balance must be not smaller than required with fee.
            if balance_ss.unwrap() - fee < source_amount {
                Err(Error::<T>::SourceBalanceIsNotLargeEnouth)?;
            }
            // For destination account balance must successful large for this swap.
            if balance_tt < destination_amount {
                Err(Error::<T>::TargetBalanceIsNotLargeEnouth)?;
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
        ensure!(
            Some(source.clone()) == self.client_account,
            Error::<T>::SourceAndClientAccountDoNotMatchAsEqual
        );
        technical::Module::<T>::transfer_in(
            &self.source.asset,
            &source,
            &self.pool_account,
            self.source.amount.unwrap(),
        )?;
        technical::Module::<T>::transfer_in(
            &self.source.asset,
            &source,
            &self.fee_account.clone().unwrap(),
            self.fee.unwrap(),
        )?;
        technical::Module::<T>::transfer_out(
            &self.destination.asset,
            &self.pool_account,
            &self.receiver_account.clone().unwrap(),
            self.destination.amount.unwrap(),
        )?;
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
        Module::<T>::is_pool_account_valid_for(
            (self.source.0).asset.clone(),
            self.pool_account.clone(),
        )?;

        // Balance of source account for asset pair.
        let (balance_bs, balance_ts) = if abstract_checking {
            (None, None)
        } else {
            let source = source_opt.unwrap();
            (
                Some(<assets::Module<T>>::free_balance(
                    &(self.source.0).asset,
                    &source,
                )?),
                Some(<assets::Module<T>>::free_balance(
                    &(self.source.1).asset,
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
        let balance_bp = <assets::Module<T>>::free_balance(
            &((self.source.0).asset.clone()),
            &pool_account_repr_sys,
        )?;
        // Balance of pool account for asset pair target asset.
        let balance_tp = <assets::Module<T>>::free_balance(
            &((self.source.1).asset.clone()),
            &pool_account_repr_sys,
        )?;

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
        // Calculate pair ratio of pool.
        // Here basic asset balance divided by target asset balance.
        // TODO: will be used in additional verification checks.
        let _ratio_a = {
            if empty_pool {
                if abstract_checking {
                    None
                } else {
                    Some(init_x / init_y)
                }
            } else {
                Some(balance_bp / balance_tp)
            }
        };
        // Product of pool pair amounts to get k value.
        let pool_k = {
            if empty_pool {
                if abstract_checking {
                    None
                } else {
                    Some(init_x * init_y)
                }
            } else {
                Some(balance_bp * balance_tp)
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
                        let peace_to_add = pool_k.unwrap() / destination_k;
                        let recom_x = balance_bp / peace_to_add;
                        let recom_y = balance_tp / peace_to_add;
                        match ox {
                            Bounds::Desired(x) => {
                                if x != recom_x {
                                    Err(Error::<T>::InvalidDepositLiquidityBasicAssetAmount)?
                                }
                            }
                            _ => {
                                (self.source.0).amount =
                                    Bounds::Calculated(balance_bp / peace_to_add);
                            }
                        }
                        match oy {
                            Bounds::Desired(y) => {
                                if y != recom_y {
                                    Err(Error::<T>::InvalidDepositLiquidityTargetAssetAmount)?
                                }
                            }
                            _ => {
                                (self.source.1).amount =
                                    Bounds::Calculated(balance_tp / peace_to_add);
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
        let recom_min_liquidity = Module::<T>::get_min_liquidity_for(
            (self.source.0).asset.clone(),
            self.pool_account.clone(),
        );
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
            let base_amount = (self.source.1).amount.unwrap();
            let target_amount = (self.source.0).amount.unwrap();
            let destination_amount = self.destination.amount.unwrap();
            // Checking by minimum liquidity.
            if min_liquidity > pool_k.unwrap()
                && destination_amount < min_liquidity - pool_k.unwrap()
            {
                Err(Error::<T>::DestinationAmountOfLiquidityIsNotLargeEnouth)?;
            }
            // Checking that balances if correct and large enough for amounts.
            if balance_bs.unwrap() < base_amount {
                Err(Error::<T>::SourceBaseAmountIsNotLargeEnouth)?;
            }
            if balance_ts.unwrap() < target_amount {
                Err(Error::<T>::TargetBaseAmountIsNotLargeEnouth)?;
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
            Some(source.clone()) == self.client_account,
            Error::<T>::SourceAndClientAccountDoNotMatchAsEqual
        );
        let asset_repr = Into::<AssetIdOf<T>>::into(self.destination.asset.clone());
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
        assets::Module::<T>::mint(
            &asset_repr,
            &pool_account_repr_sys,
            &self.receiver_account.clone().unwrap(),
            self.destination.amount.unwrap(),
        )?;
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
        Module::<T>::is_pool_account_valid_for(
            (self.destination.0).asset.clone(),
            self.pool_account.clone(),
        )?;

        let mark_asset = Module::<T>::get_marking_asset(self.pool_account.clone())?;
        ensure!(
            self.source.asset == mark_asset,
            Error::<T>::InvalidAssetForLiquidityMarking
        );

        let repr_k_asset_id = self.source.asset.clone().into();

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

        // Calculate pair ratio of pool.
        // Here basic asset balance divided by target asset balance.
        // TODO: will be used in additional verification checks.
        let _ratio_a = balance_bp / balance_tp;

        // Product of pool pair amounts to get k value.
        let pool_k = balance_bp * balance_tp;

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
                let peace_to_take = pool_k / source_k;
                let recom_x = balance_bp / peace_to_take;
                let recom_y = balance_tp / peace_to_take;

                match ox {
                    Bounds::Desired(x) => {
                        if x != recom_x {
                            Err(Error::<T>::InvalidWithdrawLiquidityBasicAssetAmount)?;
                        }
                    }

                    _ => {
                        (self.destination.0).amount = Bounds::Calculated(recom_x);
                    }
                }

                match oy {
                    Bounds::Desired(y) => {
                        if y != recom_y {
                            Err(Error::<T>::InvalidWithdrawLiquidityTargetAssetAmount)?;
                        }
                    }

                    _ => {
                        (self.destination.1).amount = Bounds::Calculated(recom_y);
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
            Err(Error::<T>::SourceBalanceOfLiquidityTokensIsNotLargeEnouth)?;
        }

        // Checking that balances if correct and large enough for amounts.
        if balance_bp < base_amount {
            Err(Error::<T>::DestinationBaseBalanceIsNotLargeEnouth)?;
        }
        if balance_tp < target_amount {
            Err(Error::<T>::DestinationTargetBalanceIsNotLargeEnouth)?;
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
            Some(source.clone()) == self.client_account,
            Error::<T>::SourceAndClientAccountDoNotMatchAsEqual
        );
        let asset_repr = Into::<AssetIdOf<T>>::into(self.source.asset.clone());
        let pool_account_repr_sys =
            technical::Module::<T>::tech_account_id_to_account_id(&self.pool_account)?;
        technical::Module::<T>::transfer_out(
            &(self.destination.0).asset,
            &self.pool_account,
            &self.receiver_account_a.clone().unwrap(),
            (self.destination.0).amount.unwrap(),
        )?;
        technical::Module::<T>::transfer_out(
            &(self.destination.1).asset,
            &self.pool_account,
            &self.receiver_account_b.clone().unwrap(),
            (self.destination.1).amount.unwrap(),
        )?;
        assets::Module::<T>::burn(
            &asset_repr,
            &pool_account_repr_sys,
            &source,
            self.source.amount.unwrap(),
        )?;
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
        Something get(fn something): Option<u32>;
    }
}

impl<T: Trait> Module<T> {
    pub fn get_marking_asset_repr(
        tech_acc: TechAccountIdOf<T>,
    ) -> Result<AssetIdOf<T>, DispatchError> {
        Ok(Into::<AssetIdOf<T>>::into(
            common::ToMarkerAsset::<TechAssetIdOf<T>>::to_marker_asset(&tech_acc)
                .ok_or(Error::<T>::UnableToDecideMarkerAsset)?,
        ))
    }

    pub fn get_marking_asset(
        tech_acc: TechAccountIdOf<T>,
    ) -> Result<TechAssetIdOf<T>, DispatchError> {
        Ok(
            common::ToMarkerAsset::<TechAssetIdOf<T>>::to_marker_asset(&tech_acc)
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

    pub fn get_fee_for(_asset_id: AssetIdOf<T>, _tech_acc: TechAccountIdOf<T>) -> Balance {
        //TODO: get this value from DEXInfo.
        30_u32.into()
    }

    pub fn get_min_liquidity_for(
        _asset_id: AssetIdOf<T>,
        _tech_acc: TechAccountIdOf<T>,
    ) -> Balance {
        //TODO: get this value from DEXInfo.
        55440_u32.into()
    }

    pub fn get_fee_account(
        tech_acc: TechAccountIdOf<T>,
    ) -> Result<TechAccountIdOf<T>, DispatchError> {
        let fee_acc = tech_acc
            .to_fee_account()
            .ok_or(Error::<T>::UnableToDiriveFeeAccount)?;
        Ok(fee_acc)
    }

    pub fn is_fee_account_valid_for(
        _asset_id: AssetIdOf<T>,
        tech_acc: TechAccountIdOf<T>,
        fee_acc: TechAccountIdOf<T>,
    ) -> DispatchResult {
        let recommended = Self::get_fee_account(tech_acc)?;
        if fee_acc != recommended {
            Err(Error::<T>::FeeAccountIsInvalid)?;
        }
        Ok(())
    }

    pub fn is_pool_account_valid_for(
        _asset_id: AssetIdOf<T>,
        tech_acc: TechAccountIdOf<T>,
    ) -> DispatchResult {
        technical::Module::<T>::ensure_tech_account_registered(&tech_acc)?;
        //TODO: Maybe checking that asset and dex is exist, it is not really needed if
        //registration of technical account is a garanty that pair and dex exist.
        Ok(())
    }
}

decl_event!(
    pub enum Event<T>
    where
        AssetId = AssetIdOf<T>,
    {
        SomethingStored(u32, AssetId),
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
        SourceBalanceIsNotLargeEnouth,
        TargetBalanceIsNotLargeEnouth,
        UnableToDiriveFeeAccount,
        FeeAccountIsInvalid,
        SourceAndClientAccountDoNotMatchAsEqual,
        AssetsMustNotBeSame,
        ImposibleToDecideDepositLiquidityAmounts,
        InvalidDepositLiquidityBasicAssetAmount,
        InvalidDepositLiquidityTargetAssetAmount,
        PairSwapActionMinimumLiquidityIsSmallerThanRecommended,
        DestinationAmountOfLiquidityIsNotLargeEnouth,
        SourceBaseAmountIsNotLargeEnouth,
        TargetBaseAmountIsNotLargeEnouth,
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
        SourceBalanceOfLiquidityTokensIsNotLargeEnouth,
        DestinationBaseBalanceIsNotLargeEnouth,
        DestinationTargetBalanceIsNotLargeEnouth,
        InvalidAssetForLiquidityMarking,
        AssetDecodingError,
        CalculatedValueIsOutOfDesiredBounds,
        BaseAssetIsNotMatchedWithAnyAssetArguments,
        DestinationAmountMustBeSame,
        SourceAmountMustBeSame,
    }
}

impl<T: Trait> Module<T> {
    fn tech_account_from_dex_and_asset_pair(
        dex_id: T::DEXId,
        asset_a: T::AssetId,
        asset_b: T::AssetId,
    ) -> Result<TechAccountIdOf<T>, DispatchError> {
        let dexinfo = <dex_manager::DEXInfos<T>>::get(dex_id.clone());
        let base_asset_id = dexinfo.base_asset_id;
        ensure!(asset_a != asset_b, Error::<T>::AssetsMustNotBeSame);
        let ba = Module::<T>::try_decode_asset(base_asset_id.clone())?;
        let ta = if base_asset_id == asset_a {
            Module::<T>::try_decode_asset(asset_b.clone())?
        } else if base_asset_id == asset_b {
            Module::<T>::try_decode_asset(asset_a.clone())?
        } else {
            Err(Error::<T>::BaseAssetIsNotMatchedWithAnyAssetArguments)?
        };
        let tpair = common::TradingPair::<TechAssetIdOf<T>> {
            base_asset_id: ba,
            target_asset_id: ta,
        };
        Ok(TechAccountIdOf::<T>::to_tech_unit_from_dex_and_trading_pair(dex_id.clone(), tpair))
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
        let tech_acc_id =
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

    fn deposit_liquidity_unchecked(
        source: AccountIdOf<T>,
        receiver: AccountIdOf<T>,
        dex_id: DEXIdOf<T>,
        input_asset_a: AssetIdOf<T>,
        input_asset_b: AssetIdOf<T>,
        swap_amount_a: SwapAmount<Balance>,
        swap_amount_b: SwapAmount<Balance>,
    ) -> DispatchResult {
        let (
            source_amount_a,
            destination_amount_a,
            source_amount_b,
            destination_amount_b,
            tech_acc_id,
        ) = Module::<T>::get_bounded_asset_pair_for_liquidity(
            dex_id,
            input_asset_a,
            input_asset_b,
            swap_amount_a,
            swap_amount_b,
        )?;
        ensure!(
            destination_amount_a == destination_amount_b,
            Error::<T>::DestinationAmountMustBeSame
        );
        let mark_asset = Module::<T>::get_marking_asset(tech_acc_id.clone())?;
        let action = PolySwapActionStructOf::<T>::DepositLiquidity(DepositLiquidityActionOf::<T> {
            client_account: None,
            receiver_account: Some(receiver),
            pool_account: tech_acc_id,
            source: ResourcePair(
                Resource {
                    asset: input_asset_a,
                    amount: source_amount_a,
                },
                Resource {
                    asset: input_asset_b,
                    amount: source_amount_b,
                },
            ),
            destination: Resource {
                asset: mark_asset,
                amount: destination_amount_a,
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
        receiver_a: AccountIdOf<T>,
        receiver_b: AccountIdOf<T>,
        dex_id: DEXIdOf<T>,
        output_asset_a: AssetIdOf<T>,
        output_asset_b: AssetIdOf<T>,
        swap_amount_a: SwapAmount<Balance>,
        swap_amount_b: SwapAmount<Balance>,
    ) -> DispatchResult {
        let (
            source_amount_a,
            destination_amount_a,
            source_amount_b,
            destination_amount_b,
            tech_acc_id,
        ) = Module::<T>::get_bounded_asset_pair_for_liquidity(
            dex_id,
            output_asset_a,
            output_asset_b,
            swap_amount_a,
            swap_amount_b,
        )?;
        ensure!(
            source_amount_a == source_amount_b,
            Error::<T>::SourceAmountMustBeSame
        );
        let mark_asset = Module::<T>::get_marking_asset(tech_acc_id.clone())?;
        let action =
            PolySwapActionStructOf::<T>::WithdrawLiquidity(WithdrawLiquidityActionOf::<T> {
                client_account: None,
                receiver_account_a: Some(receiver_a),
                receiver_account_b: Some(receiver_b),
                pool_account: tech_acc_id,
                source: Resource {
                    asset: mark_asset,
                    amount: source_amount_a,
                },
                destination: ResourcePair(
                    Resource {
                        asset: output_asset_a,
                        amount: destination_amount_a,
                    },
                    Resource {
                        asset: output_asset_b,
                        amount: destination_amount_b,
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

        #[weight = 0]
        fn swap_pair(
            origin, receiver: AccountIdOf<T>, dex_id: DEXIdOf<T>,
            input_asset_id: AssetIdOf<T>, output_asset_id: AssetIdOf<T>,
            swap_amount: SwapAmount<Balance>,) -> DispatchResult {
            let source = ensure_signed(origin)?;
            Module::<T>::exchange(&source, &receiver, &dex_id, &input_asset_id, &output_asset_id, swap_amount)?;
            Ok(())
        }

        #[weight = 0]
        fn deposit_liquidity(
            origin, receiver: AccountIdOf<T>, dex_id: DEXIdOf<T>,
            input_asset_a: AssetIdOf<T>, input_asset_b: AssetIdOf<T>,
            swap_amount_a: SwapAmount<Balance>, swap_amount_b: SwapAmount<Balance>) -> DispatchResult {
            let source = ensure_signed(origin)?;
            Module::<T>::deposit_liquidity_unchecked(source, receiver, dex_id,
                input_asset_a, input_asset_b, swap_amount_a, swap_amount_b)?;
            Ok(())
        }

        #[weight = 0]
        fn withdraw_liquidity(
            origin, receiver_a: AccountIdOf<T>, receiver_b: AccountIdOf<T>, dex_id: DEXIdOf<T>,
            output_asset_a: AssetIdOf<T>, output_asset_b: AssetIdOf<T>,
            swap_amount_a: SwapAmount<Balance>, swap_amount_b: SwapAmount<Balance>) -> DispatchResult {
            let source = ensure_signed(origin)?;
            Module::<T>::withdraw_liquidity_unchecked(source, receiver_a, receiver_b, dex_id,
                output_asset_a, output_asset_b, swap_amount_a, swap_amount_b)?;
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
            let tech_acc_id = Module::<T>::tech_account_from_dex_and_asset_pair(
                dex_id.clone(),
                input_asset_id.clone(),
                output_asset_id.clone(),
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
        let tech_acc_id = Module::<T>::tech_account_from_dex_and_asset_pair(
            dex_id.clone(),
            input_asset_id.clone(),
            output_asset_id.clone(),
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
        });
        common::SwapRulesValidation::<AccountIdOf<T>, TechAccountIdOf<T>, T>::prepare_and_validate(
            &mut action,
            None,
        )?;
        // It is garanty that unwrap is always ok.
        let (fee, term_amount) = match action.clone() {
            PolySwapAction::PairSwap(a) => (a.fee.unwrap(), a.destination.amount.unwrap()),
            _ => unreachable!("we know that always PairSwap is used"),
        };
        Ok(common::prelude::SwapOutcome::new(term_amount, fee))
    }

    fn exchange(
        sender: &T::AccountId,
        receiver: &T::AccountId,
        dex_id: &T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        swap_amount: SwapAmount<Balance>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        let tech_acc_id = Module::<T>::tech_account_from_dex_and_asset_pair(
            dex_id.clone(),
            input_asset_id.clone(),
            output_asset_id.clone(),
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
        });
        common::SwapRulesValidation::<AccountIdOf<T>, TechAccountIdOf<T>, T>::prepare_and_validate(
            &mut action,
            Some(sender),
        )?;
        // It is garanty that unwrap is always ok.
        let (fee, term_amount) = match action.clone() {
            PolySwapAction::PairSwap(a) => (a.fee.unwrap(), a.destination.amount.unwrap()),
            _ => unreachable!("we know that always PairSwap is used"),
        };
        let action = T::PolySwapAction::from(action);
        let mut action = action.into();
        technical::Module::<T>::perform_create_swap_unchecked(sender.clone(), &mut action)?;
        Ok(common::prelude::SwapOutcome::new(term_amount, fee))
    }
}
