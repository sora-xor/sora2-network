#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{decl_error, decl_event, decl_module, decl_storage, dispatch};
use frame_system::ensure_signed;

use codec::{Decode, Encode};
use core::convert::TryInto;
use frame_support::weights::Weight;
use frame_support::Parameter;
use sp_runtime::RuntimeDebug;

use common::prelude::Balance;

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

#[derive(Clone, RuntimeDebug, Eq, PartialEq, Encode, Decode)]
pub struct Resource<AssetId, Balance>(AssetId, Option<Balance>);

#[derive(Clone, RuntimeDebug, Eq, PartialEq, Encode, Decode)]
pub struct ResourcePair<AssetId, Balance>(Resource<AssetId, Balance>, Resource<AssetId, Balance>);

#[derive(Clone, RuntimeDebug, Eq, PartialEq, Encode, Decode)]
pub struct PairSwapAction<AssetId, Balance, AccountId, TechAccountId> {
    client_account: Option<AccountId>,
    pool_account: TechAccountId,
    source: Resource<AssetId, Balance>,
    terminal: Resource<AssetId, Balance>,
    fee: Option<Balance>,
    fee_account: Option<TechAccountId>,
}

#[derive(Clone, RuntimeDebug, Eq, PartialEq, Encode, Decode)]
pub struct DepositLiquidityAction<AssetId, TechAssetId, Balance, AccountId, TechAccountId> {
    client_account: Option<AccountId>,
    pool_account: TechAccountId,
    source: ResourcePair<AssetId, Balance>,
    terminal: Resource<TechAssetId, Balance>,
    minliq: Option<Balance>,
}

#[derive(Clone, RuntimeDebug, Eq, PartialEq, Encode, Decode)]
pub struct WithdrawLiquidityAction<AssetId, TechAssetId, Balance, AccountId, TechAccountId> {
    client_account: Option<AccountId>,
    pool_account: TechAccountId,
    source: Resource<TechAssetId, Balance>,
    terminal: ResourcePair<AssetId, Balance>,
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
pub trait Trait: technical::Trait + dex_manager::Trait {
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
        + From<
            PolySwapAction<
                AssetIdOf<Self>,
                TechAssetIdOf<Self>,
                Balance,
                AccountIdOf<Self>,
                TechAccountIdOf<Self>,
            >,
        >;
}

impl<T: Trait> common::SwapRulesValidation<AccountIdOf<T>, TechAccountIdOf<T>, T>
    for PairSwapAction<AssetIdOf<T>, Balance, AccountIdOf<T>, TechAccountIdOf<T>>
{
    fn prepare_and_validate(&mut self, source: &AccountIdOf<T>) -> DispatchResult {
        // Check that client account is same as source, because signature is checked for source.
        // TODO: In general case it is posible to use different client account, for example if
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
        let pool_account_repr_sys =
            technical::Module::<T>::tech_account_id_to_account_id(&self.pool_account)?;
        // Check that pool account is valid.
        Module::<T>::is_pool_account_valid_for(
            self.source.0.clone(),
            source.clone(),
            self.pool_account.clone(),
        )?;
        // Source balance of source account.
        let balance_ss = <assets::Module<T>>::free_balance(&(self.source.0), &source)?;
        // Source balance of technical account.
        let balance_st = <assets::Module<T>>::free_balance(&self.source.0, &pool_account_repr_sys)?;
        // Terminal balance of technical account.
        let balance_tt =
            <assets::Module<T>>::free_balance(&self.terminal.0, &pool_account_repr_sys)?;

        ensure!(
            balance_ss > 0_u32.into(),
            Error::<T>::AccountBalanceIsInvalid
        );

        if balance_st == 0_u32.into() && balance_tt == 0_u32.into() {
            Err(Error::<T>::PoolIsEmpty)?;
        } else if balance_st <= 0_u32.into() || balance_tt <= 0_u32.into() {
            Err(Error::<T>::PoolIsInvalid)?;
        }

        // Calculate pair ratio of pool, and check or correct amount of pair swap action.
        // Here source technical is devided by terminal technical.
        let ratio_a = balance_st / balance_tt;
        match (self.source.1, self.terminal.1) {
            // Case then no amount is specified, imposible to diside any amounts.
            (None, None) => {
                Err(Error::<T>::ImposibleToDesideAssetPairAmounts)?;
            }
            // Case then source amount if specified but terminal is not, it`s posible to deside it.
            (Some(sa), None) => {
                ensure!(sa > 0_u32.into(), Error::<T>::ZeroValueInAmountParameter);
                self.terminal.1 = Some(sa / ratio_a);
            }
            // Case then terminal amount if specified but source is not, it`s posible to deside it.
            (None, Some(ta)) => {
                ensure!(ta > 0_u32.into(), Error::<T>::ZeroValueInAmountParameter);
                self.source.1 = Some(ta * ratio_a);
            }
            // Case then both source and terminal amounts is specified, just checking it.
            (Some(sa), Some(ta)) => {
                ensure!(sa > 0_u32.into(), Error::<T>::ZeroValueInAmountParameter);
                ensure!(ta > 0_u32.into(), Error::<T>::ZeroValueInAmountParameter);
                let ratio_b = sa / ta;
                if ratio_a != ratio_b {
                    Err(Error::<T>::PoolPairRatioAndPairSwapRatioIsDifferent)?;
                }
            }
        }
        // Check fee account if it is specfied, or set it if not.
        match self.fee_account.clone() {
            Some(fa) => {
                // Checking that fee account is valid for this set of parameters.
                Module::<T>::is_fee_account_valid_for(
                    self.source.0.clone(),
                    source.clone(),
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
        let recom_fee = Module::<T>::get_fee_for(
            self.source.0.clone(),
            source.clone(),
            self.pool_account.clone(),
        );
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
        // Get required values, now is is always Some, it is safe to unwrap().
        let fee = self.fee.unwrap();
        let source_amount = self.source.1.unwrap();
        let terminal_amount = self.terminal.1.unwrap();
        // Checking that balances if correct and large enouth for amounts.
        // For source account balance must be not smaller than required with fee.
        if balance_ss - fee < source_amount {
            Err(Error::<T>::SourceBalanceIsNotLargeEnouth)?;
        }
        // For terminal account balance must succesefull large for this swap.
        if balance_tt < terminal_amount {
            Err(Error::<T>::TargetBalanceIsNotLargeEnouth)?;
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
    /// unwrap.
    fn reserve(&self, source: &AccountIdOf<T>) -> dispatch::DispatchResult {
        ensure!(
            Some(source.clone()) == self.client_account,
            Error::<T>::SourceAndClientAccountDoNotMatchAsEqual
        );
        technical::Module::<T>::transfer_in(
            &self.source.0,
            &source,
            &self.pool_account,
            self.source.1.unwrap(),
        )?;
        technical::Module::<T>::transfer_in(
            &self.source.0,
            &source,
            &self.fee_account.clone().unwrap(),
            self.fee.unwrap(),
        )?;
        technical::Module::<T>::transfer_out(
            &self.terminal.0,
            &self.pool_account,
            &source,
            self.terminal.1.unwrap(),
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
    fn prepare_and_validate(&mut self, source: &AccountIdOf<T>) -> DispatchResult {
        // Check that client account is same as source, because signature is checked for source.
        // TODO: In general case it is posible to use different client account, for example if
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
        let pool_account_repr_sys =
            technical::Module::<T>::tech_account_id_to_account_id(&self.pool_account)?;
        // Check that pool account is valid.
        Module::<T>::is_pool_account_valid_for(
            (self.source.0).0.clone(),
            source.clone(),
            self.pool_account.clone(),
        )?;
        // Balance of source account for asset pair basic asset.
        let balance_bs = <assets::Module<T>>::free_balance(&(self.source.0).0, &source)?;
        // Balance of source account for asset pair target asset.
        let balance_ts = <assets::Module<T>>::free_balance(&(self.source.1).0, &source)?;
        if balance_bs <= 0_u32.into() || balance_ts <= 0_u32.into() {
            Err(Error::<T>::AccountBalanceIsInvalid)?;
        }

        // Balance of pool account for asset pair basic asset.
        let balance_bp = <assets::Module<T>>::free_balance(
            &((self.source.0).0.clone()),
            &pool_account_repr_sys,
        )?;
        // Balance of pool account for asset pair target asset.
        let balance_tp = <assets::Module<T>>::free_balance(
            &((self.source.1).0.clone()),
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
        if empty_pool {
            init_x = (self.source.0)
                .1
                .ok_or(Error::<T>::InitialLiqudityDepositRatioMustBeDefined)?;
            init_y = (self.source.1)
                .1
                .ok_or(Error::<T>::InitialLiqudityDepositRatioMustBeDefined)?;
        }
        // Calculate pair ratio of pool.
        // Here basic asset balance divided by target asset balance.
        // TODO: will be used in additional verification checks.
        let _ratio_a = {
            if empty_pool {
                init_x / init_y
            } else {
                balance_bp / balance_tp
            }
        };
        // Product of pool pair amounts to get k value.
        let pool_k = {
            if empty_pool {
                init_x * init_y
            } else {
                balance_bp * balance_tp
            }
        };
        if empty_pool {
            match self.terminal.1 {
                Some(k) => {
                    ensure!(
                        k == pool_k,
                        Error::<T>::InvalidDepositLiquidityTerminalAmount
                    );
                }
                None => {
                    self.terminal.1 = Some(pool_k);
                }
            }
        } else {
            match ((self.source.0).1, (self.source.1).1, self.terminal.1) {
                // Case then no amount is specified, imposible to diside any amounts.
                (None, None, None) => {
                    Err(Error::<T>::ImposibleToDesideDepositLiquidityAmounts)?;
                }
                (ox, oy, Some(terminal_k)) => {
                    ensure!(
                        terminal_k > 0_u32.into(),
                        Error::<T>::ZeroValueInAmountParameter
                    );
                    let peace_to_add = pool_k / terminal_k;
                    let recom_x = balance_bp / peace_to_add;
                    let recom_y = balance_tp / peace_to_add;
                    match ox {
                        None => {
                            (self.source.0).1 = Some(balance_bp / peace_to_add);
                        }
                        Some(x) => {
                            if x != recom_x {
                                Err(Error::<T>::InvalidDepositLiquidityBasicAssetAmount)?
                            }
                        }
                    }
                    match oy {
                        None => {
                            (self.source.1).1 = Some(balance_tp / peace_to_add);
                        }
                        Some(y) => {
                            if y != recom_y {
                                Err(Error::<T>::InvalidDepositLiquidityTargetAssetAmount)?
                            }
                        }
                    }
                }
                (_, _, _) => {
                    Err(Error::<T>::ImposibleToDesideDepositLiquidityAmounts)?;
                }
            }
        }
        // Recommended minimum liquidity, will be used if not specified or for checking if specified.
        let recom_minliq = Module::<T>::get_minliq_for(
            (self.source.0).0.clone(),
            source.clone(),
            self.pool_account.clone(),
        );
        // Set recommended or check that `minliq` is correct.
        match self.minliq {
            // Just set it here if it not specified, this is usual case.
            None => {
                self.minliq = Some(recom_minliq);
            }
            // Case with source user `minliq` is set, checking that it is not smaller.
            Some(minliq) => {
                if minliq < recom_minliq {
                    Err(Error::<T>::PairSwapActionMinimumLiquidityIsSmallerThanRecommended)?
                }
            }
        }
        // Get required values, now is is always Some, it is safe to unwrap().
        let minliq = self.minliq.unwrap();
        let base_amount = (self.source.1).1.unwrap();
        let target_amount = (self.source.0).1.unwrap();
        let terminal_amount = self.terminal.1.unwrap();
        // Checking by minimum liquidity.
        if minliq > pool_k && terminal_amount < minliq - pool_k {
            Err(Error::<T>::TerminalAmountOfLiquidityIsNotLargeEnouth)?;
        }
        // Checking that balances if correct and large enouth for amounts.
        if balance_bs < base_amount {
            Err(Error::<T>::SourceBaseAmountIsNotLargeEnouth)?;
        }
        if balance_ts < target_amount {
            Err(Error::<T>::TargetBaseAmountIsNotLargeEnouth)?;
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
        let asset_repr = Into::<AssetIdOf<T>>::into(self.terminal.0.clone());
        let pool_account_repr_sys =
            technical::Module::<T>::tech_account_id_to_account_id(&self.pool_account)?;
        technical::Module::<T>::transfer_in(
            &(self.source.0).0,
            &source,
            &self.pool_account,
            (self.source.0).1.unwrap(),
        )?;
        technical::Module::<T>::transfer_in(
            &(self.source.1).0,
            &source,
            &self.pool_account,
            (self.source.1).1.unwrap(),
        )?;
        assets::Module::<T>::mint(
            &asset_repr,
            &pool_account_repr_sys,
            &source,
            self.terminal.1.unwrap(),
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
    fn prepare_and_validate(&mut self, source: &AccountIdOf<T>) -> DispatchResult {
        // Check that client account is same as source, because signature is checked for source.
        // TODO: In general case it is posible to use different client account, for example if
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
        let pool_account_repr_sys =
            technical::Module::<T>::tech_account_id_to_account_id(&self.pool_account)?;
        // Check that pool account is valid.
        Module::<T>::is_pool_account_valid_for(
            (self.terminal.0).0.clone(),
            source.clone(),
            self.pool_account.clone(),
        )?;

        let mark_asset = Module::<T>::get_marking_asset(self.pool_account.clone())?;
        ensure!(
            self.source.0 == mark_asset,
            Error::<T>::InvalidAssetForLiquidityMarking
        );

        let repr_k_asset_id = self.source.0.clone().into();

        // Balance of source account for k value.
        let balance_ks = <assets::Module<T>>::free_balance(&repr_k_asset_id, &source)?;
        if balance_ks <= 0_u32.into() {
            Err(Error::<T>::AccountBalanceIsInvalid)?;
        }

        // Balance of pool account for asset pair basic asset.
        let balance_bp =
            <assets::Module<T>>::free_balance(&(self.terminal.0).0, &pool_account_repr_sys)?;
        // Balance of pool account for asset pair target asset.
        let balance_tp =
            <assets::Module<T>>::free_balance(&(self.terminal.1).0, &pool_account_repr_sys)?;

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

        match (self.source.1, (self.terminal.0).1, (self.terminal.1).1) {
            // Case then no amount is specified, imposible to deside any amounts.
            (None, None, None) => {
                Err(Error::<T>::ImposibleToDesideWithdrawLiquidityAmounts)?;
            }

            (Some(source_k), ox, oy) => {
                ensure!(
                    source_k > 0_u32.into(),
                    Error::<T>::ZeroValueInAmountParameter
                );
                let peace_to_take = pool_k / source_k;
                let recom_x = balance_bp / peace_to_take;
                let recom_y = balance_tp / peace_to_take;

                match ox {
                    None => {
                        (self.terminal.0).1 = Some(recom_x);
                    }
                    Some(x) => {
                        if x != recom_x {
                            Err(Error::<T>::InvalidWithdrawLiquidityBasicAssetAmount)?;
                        }
                    }
                }

                match oy {
                    None => {
                        (self.terminal.1).1 = Some(recom_y);
                    }
                    Some(y) => {
                        if y != recom_y {
                            Err(Error::<T>::InvalidWithdrawLiquidityTargetAssetAmount)?;
                        }
                    }
                }
            }

            _ => {
                Err(Error::<T>::ImposibleToDesideDepositLiquidityAmounts)?;
            }
        }

        // Get required values, now is is always Some, it is safe to unwrap().
        let base_amount = (self.terminal.1).1.unwrap();
        let target_amount = (self.terminal.0).1.unwrap();
        let source_amount = self.source.1.unwrap();

        if source_amount > pool_k {
            Err(Error::<T>::SourceBaseAmountIsTooLarge)?;
        }

        if balance_ks < source_amount {
            Err(Error::<T>::SourceBalanceOfLiquidityTokensIsNotLargeEnouth)?;
        }

        // Checking that balances if correct and large enouth for amounts.
        if balance_bp < base_amount {
            Err(Error::<T>::TerminalBaseBalanceIsNotLargeEnouth)?;
        }
        if balance_tp < target_amount {
            Err(Error::<T>::TerminalTargetBalanceIsNotLargeEnouth)?;
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
        let asset_repr = Into::<AssetIdOf<T>>::into(self.source.0.clone());
        let pool_account_repr_sys =
            technical::Module::<T>::tech_account_id_to_account_id(&self.pool_account)?;
        technical::Module::<T>::transfer_out(
            &(self.terminal.0).0,
            &self.pool_account,
            &source,
            (self.terminal.0).1.unwrap(),
        )?;
        technical::Module::<T>::transfer_out(
            &(self.terminal.1).0,
            &self.pool_account,
            &source,
            (self.terminal.1).1.unwrap(),
        )?;
        assets::Module::<T>::burn(
            &asset_repr,
            &pool_account_repr_sys,
            &source,
            self.source.1.unwrap(),
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
    for PolySwapAction<AssetIdOf<T>, TechAssetIdOf<T>, Balance, AccountIdOf<T>, TechAccountIdOf<T>>
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
    fn prepare_and_validate(&mut self, source: &AccountIdOf<T>) -> DispatchResult {
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
    for PolySwapAction<AssetIdOf<T>, TechAssetIdOf<T>, Balance, AccountIdOf<T>, TechAccountIdOf<T>>
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
        let a = common::ToMarkerAsset::<TechAssetIdOf<T>>::to_marker_asset(&tech_acc)
            .ok_or(Error::<T>::UnableToDesideMarkerAsset)?;
        let b = Into::<AssetIdOf<T>>::into(a);
        Ok(b)
    }

    pub fn get_marking_asset(
        tech_acc: TechAccountIdOf<T>,
    ) -> Result<TechAssetIdOf<T>, DispatchError> {
        let a = common::ToMarkerAsset::<TechAssetIdOf<T>>::to_marker_asset(&tech_acc)
            .ok_or(Error::<T>::UnableToDesideMarkerAsset)?;
        Ok(a)
    }
}

impl<T: Trait> Module<T> {
    pub fn get_fee_for(
        _asset_id: AssetIdOf<T>,
        _source: AccountIdOf<T>,
        _tech_acc: TechAccountIdOf<T>,
    ) -> Balance {
        //TODO: get this value from DEXInfo.
        30_u32.into()
    }

    pub fn get_minliq_for(
        _asset_id: AssetIdOf<T>,
        _source: AccountIdOf<T>,
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
        _source: AccountIdOf<T>,
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
        _source: AccountIdOf<T>,
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
        /// It is imposible to calculate fee for some pair swap operation, or other operation.
        UnableToCalculateFee,
        UnableToGetBalance,
        ImposibleToDesideAssetPairAmounts,
        PoolPairRatioAndPairSwapRatioIsDifferent,
        PairSwapActionFeeIsSmallerThanRecommended,
        SourceBalanceIsNotLargeEnouth,
        TargetBalanceIsNotLargeEnouth,
        UnableToDiriveFeeAccount,
        FeeAccountIsInvalid,
        SourceAndClientAccountDoNotMatchAsEqual,
        AssetsMustNotBeSame,
        ImposibleToDesideDepositLiquidityAmounts,
        InvalidDepositLiquidityBasicAssetAmount,
        InvalidDepositLiquidityTargetAssetAmount,
        PairSwapActionMinimumLiquidityIsSmallerThanRecommended,
        TerminalAmountOfLiquidityIsNotLargeEnouth,
        SourceBaseAmountIsNotLargeEnouth,
        TargetBaseAmountIsNotLargeEnouth,
        PoolIsInvalid,
        PoolIsEmpty,
        ZeroValueInAmountParameter,
        AccountBalanceIsInvalid,
        InvalidDepositLiquidityTerminalAmount,
        InitialLiqudityDepositRatioMustBeDefined,
        TechAssetIsNotRepresentable,
        UnableToDesideMarkerAsset,
        UnableToGetAssetRepr,
        ImposibleToDesideWithdrawLiquidityAmounts,
        InvalidWithdrawLiquidityBasicAssetAmount,
        InvalidWithdrawLiquidityTargetAssetAmount,
        SourceBaseAmountIsTooLarge,
        SourceBalanceOfLiquidityTokensIsNotLargeEnouth,
        TerminalBaseBalanceIsNotLargeEnouth,
        TerminalTargetBalanceIsNotLargeEnouth,
        InvalidAssetForLiquidityMarking,
        AssetDecodingError,
    }
}

macro_rules! pattern01(
    ($origin: expr, $dex_id:expr, $asset_id:expr, $expr:expr) => ({
        let source = ensure_signed($origin)?;
        let dexinfo = <dex_manager::DEXInfos<T>>::get($dex_id.clone());
        let base_asset_id = dexinfo.base_asset_id;
        ensure!(base_asset_id != $asset_id, Error::<T>::AssetsMustNotBeSame);
        // If tech asset if encoded in repr, than it is checked for decoding and
        // converted.
        let ba = TryInto::<TechAssetIdOf::<T>>::try_into(
            base_asset_id.clone()).ok().ok_or(Error::<T>::AssetDecodingError)?;
        let ta = TryInto::<TechAssetIdOf::<T>>::try_into(
            $asset_id.clone()).ok().ok_or(Error::<T>::AssetDecodingError)?;
        let tpair = common::TradingPair::<TechAssetIdOf::<T>> { base_asset_id: ba, target_asset_id: ta };
        let tech_acc_id = TechAccountIdOf::<T>::to_tech_unit_from_dex_and_trading_pair($dex_id.clone(), tpair);
        $expr(source, base_asset_id, tech_acc_id)
    });
);

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin
    {
        type Error = Error<T>;
        fn deposit_event() = default;

        #[weight = 0]
        fn swap_pair(
            origin,
            dex_id: DEXIdOf<T>,
            asset_id_to_get: AssetIdOf<T>,
            amount_to_get: Balance) -> DispatchResult {
            pattern01!(origin, dex_id, asset_id_to_get,
                            |source, base_asset_id, tech_acc_id: TechAccountIdOf::<T>| {
                let action = PolySwapAction::<AssetIdOf<T>, TechAssetIdOf<T>,
                        Balance, AccountIdOf<T>, TechAccountIdOf<T>>::PairSwap(
                            PairSwapAction::<AssetIdOf<T>, Balance, AccountIdOf<T>, TechAccountIdOf<T>>{
                    client_account: None,
                    pool_account: tech_acc_id,
                    source: Resource(base_asset_id, None),
                    terminal: Resource(asset_id_to_get, Some(amount_to_get)),
                    fee: None,
                    fee_account: None,
                });
                let action2 = T::PolySwapAction::from(action);
                technical::Module::<T>::perform_create_swap(source, action2.into())?;
                Ok(())
            })
        }

        #[weight = 0]
        fn deposit_liquidity(
            origin,
            dex_id: DEXIdOf<T>,
            target_asset_id_to_put: AssetIdOf<T>,
            liq_to_get: Balance) -> DispatchResult {
            pattern01!(origin, dex_id, target_asset_id_to_put,
                            |source, base_asset_id, tech_acc_id: TechAccountIdOf::<T>| {
                let mark_asset = Module::<T>::get_marking_asset(tech_acc_id.clone())?;
                let action = PolySwapAction::<AssetIdOf<T>, TechAssetIdOf<T>,
                        Balance, AccountIdOf<T>, TechAccountIdOf<T>>::DepositLiquidity(
                            DepositLiquidityAction::<AssetIdOf<T>, TechAssetIdOf<T>,
                                Balance, AccountIdOf<T>, TechAccountIdOf<T>>{
                    client_account: None,
                    pool_account: tech_acc_id,
                    source: ResourcePair(Resource(base_asset_id, None), Resource(target_asset_id_to_put, None)),
                    terminal: Resource(mark_asset, Some(liq_to_get)),
                    minliq: None,
                });
                let action2 = T::PolySwapAction::from(action);
                technical::Module::<T>::perform_create_swap(source, action2.into())?;
                Ok(())
            })
        }

        #[weight = 0]
        fn withdraw_liquidity(
            origin,
            dex_id: DEXIdOf<T>,
            target_asset_id_to_get: AssetIdOf<T>,
            liq_to_give: Balance) -> DispatchResult {
            pattern01!(origin, dex_id, target_asset_id_to_get,
                            |source, base_asset_id, tech_acc_id: TechAccountIdOf::<T>| {
                let mark_asset = Module::<T>::get_marking_asset(tech_acc_id.clone())?;
                let action = PolySwapAction::<AssetIdOf<T>, TechAssetIdOf<T>,
                        Balance, AccountIdOf<T>, TechAccountIdOf<T>>::WithdrawLiquidity(
                            WithdrawLiquidityAction::<AssetIdOf<T>, TechAssetIdOf<T>,
                                Balance, AccountIdOf<T>, TechAccountIdOf<T>>{
                    client_account: None,
                    pool_account: tech_acc_id,
                    source: Resource(mark_asset, Some(liq_to_give)),
                    terminal: ResourcePair(Resource(base_asset_id, None), Resource(target_asset_id_to_get, None)),
                });
                let action2 = T::PolySwapAction::from(action);
                technical::Module::<T>::perform_create_swap(source, action2.into())?;
                Ok(())
            })
        }

    }
}
