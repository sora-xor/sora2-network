#![cfg_attr(not(feature = "std"), no_std)]

#[macro_use]
extern crate alloc;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

use codec::{Decode, Encode};
use common::{
    fixed,
    prelude::{Balance, Error as CommonError, Fixed, FixedWrapper, SwapAmount, SwapOutcome},
    AssetId, DEXId, LiquiditySource,
};
use frame_support::traits::Get;
use frame_support::{decl_error, decl_module, decl_storage};
use permissions::{Scope, BURN, MINT, SLASH, TRANSFER};
use sp_arithmetic::traits::{CheckedAdd, Zero};
use sp_runtime::DispatchError;

pub trait Trait: common::Trait + assets::Trait + technical::Trait {
    type DEXApi: LiquiditySource<
        Self::DEXId,
        Self::AccountId,
        Self::AssetId,
        Balance,
        DispatchError,
    >;
}

type Assets<T> = assets::Module<T>;
type Technical<T> = technical::Module<T>;

#[derive(Debug, Encode, Decode, Clone)]
pub struct DistributionAccounts<T: Trait> {
    xor_allocation: T::TechAccountId,
    sora_citizens: T::TechAccountId,
    stores_and_shops: T::TechAccountId,
    parliament_and_development: T::TechAccountId,
    projects: T::TechAccountId,
}

impl<T: Trait> DistributionAccounts<T> {
    pub fn as_array(&self) -> [&T::TechAccountId; 5] {
        [
            &self.xor_allocation,
            &self.sora_citizens,
            &self.stores_and_shops,
            &self.parliament_and_development,
            &self.projects,
        ]
    }
}

impl<T: Trait> Default for DistributionAccounts<T> {
    fn default() -> Self {
        Self {
            xor_allocation: Default::default(),
            sora_citizens: Default::default(),
            stores_and_shops: Default::default(),
            parliament_and_development: Default::default(),
            projects: Default::default(),
        }
    }
}

decl_storage! {
    // TODO: make pre-check for all coefficients are <= 1.
    trait Store for Module<T: Trait> as BondingCurve {
        ReservesAcc get(fn reserves_account_id): T::TechAccountId;
        Fee get(fn fee) config(): Fixed = fixed!(0,1%);
        InitialPrice get(fn initial_price) config(): Fixed = fixed!(99,3);
        PriceChangeStep get(fn price_change_step) config(): Fixed = 5000.into();
        PriceChangeRate get(fn price_change_rate) config(): Fixed = 100.into();
        SellPriceCoefficient get(fn sell_price_coefficient) config(): Fixed = fixed!(80%);
        DistributionAccountsEntry get(fn distribution_accounts) config(): DistributionAccounts<T>;
    }
}

decl_error! {
    pub enum Error for Module<T: Trait> {
        /// An error occurred while calculating the price.
        CalculatePriceFailed,
        /// The pool can't perform exchange on itself.
        CantExchangeOnItself,
        /// It's not enough reserves in the pool to perform the operation.
        NotEnoughReserves,
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        type Error = Error<T>;
    }
}

#[allow(non_snake_case)]
impl<T: Trait> Module<T> {
    /// Calculates and returns the current buy price for one token.
    ///
    /// For every `PC_S` tokens the price goes up by `PC_R`.
    ///
    /// `P_B(Q) = Q / (PC_S * PC_R) + P_I`
    ///
    /// where
    /// `P_B(Q)`: buy price for one token
    /// `P_I`: initial token price
    /// `PC_R`: price change rate
    /// `PC_S`: price change step
    /// `Q`: token issuance (quantity)
    pub fn buy_price(out_asset_id: &T::AssetId) -> Result<Fixed, DispatchError> {
        let total_issuance = Assets::<T>::total_issuance(out_asset_id)?;
        let Q: FixedWrapper = total_issuance.into();
        let P_I = Self::initial_price();
        let PC_S = Self::price_change_step();
        let PC_R = Self::price_change_rate();
        let price = Q / (PC_S * PC_R) + P_I;
        price.get().ok_or(Error::<T>::CalculatePriceFailed.into())
    }

    /// Calculates and returns the current buy price for some amount of output token.
    ///
    /// To calculate _buy_ price for a specific amount of tokens,
    /// one needs to integrate the equation of buy price (`P_B(Q)`):
    ///
    /// ```nocompile
    /// P_BTO(Q, q) = integrate [P_B(x) dx, x = Q to Q+q]
    ///            = integrate [x / (PC_S * PC_R) + P_I dx, x = Q to Q+q]
    ///            = x^2 / (2 * PC_S * PC_R) + P_I * x, x = Q to Q+q
    ///            = (x / (2 * PC_S * PC_R) + P_I) * x
    ///            = ((Q+q) / (2 * PC_S * PC_R) + P_I) * (Q+q) -
    ///              (( Q ) / (2 * PC_S * PC_R) + P_I) * ( Q )
    /// ```
    /// where
    /// `P_BTO(Q, q)`: buy price for `q` output tokens
    /// `Q`: current token issuance (quantity)
    /// `q`: amount of tokens to buy
    #[rustfmt::skip]
    pub fn buy_tokens_out_price(out_asset_id: &T::AssetId, out_quantity: Balance) -> Result<Fixed, DispatchError> {
        let total_issuance = Assets::<T>::total_issuance(&out_asset_id)?;
        let Q = FixedWrapper::from(total_issuance);
        let P_I = Self::initial_price();
        let PC_S = FixedWrapper::from(Self::price_change_step());
        let PC_R = Self::price_change_rate();

        let Q_plus_q = Q + out_quantity;
        let two_times_PC_S_times_PC_R = 2 * PC_S * PC_R;
        let to = (Q_plus_q / two_times_PC_S_times_PC_R + P_I) * Q_plus_q;
        let from = (Q / two_times_PC_S_times_PC_R + P_I) * Q;
        let price: FixedWrapper = to - from;
        price.get().ok_or(Error::<T>::CalculatePriceFailed.into())
    }

    /// Calculates and returns the current sell price for one token.
    /// Sell price is `P_Sc`% of buy price (see `buy_price`).
    ///
    /// `P_S = P_Sc * P_B`
    /// where
    /// `P_Sc: sell price coefficient (%)`
    pub fn sell_price(in_asset_id: &T::AssetId) -> Result<Fixed, DispatchError> {
        let P_B = Self::buy_price(in_asset_id)?;
        let P_Sc = FixedWrapper::from(Self::sell_price_coefficient());
        let price = P_Sc * P_B;
        price.get().ok_or(Error::<T>::CalculatePriceFailed.into())
    }

    /// Calculates and returns the current sell price for some amount of input token.
    /// Sell tokens price is `P_Sc`% of buy tokens price (see `buy_tokens_out_price`).
    ///
    /// ```nocompile
    /// P_STI = integrate [P_S dx]
    ///      = integrate [P_Sc * P_B dx]
    ///      = P_Sc * integrate [P_B dx]
    ///      = P_Sc * P_BTO
    /// where
    /// `P_Sc: sell price coefficient (%)`
    /// ```
    pub fn sell_tokens_in_price(
        in_asset_id: &T::AssetId,
        in_quantity: Balance,
    ) -> Result<Fixed, DispatchError> {
        let P_BT = Self::buy_tokens_out_price(in_asset_id, in_quantity)?;
        let P_Sc = FixedWrapper::from(Self::sell_price_coefficient());
        let price = P_Sc * P_BT;
        price.get().ok_or(Error::<T>::CalculatePriceFailed.into())
    }

    /// This function is used by `exchange` function to transfer calculated `input_amount` of
    /// `in_asset_id` to reserves and mint `output_amount` of `out_asset_id`.
    ///
    /// If there's enough reserves in the pool, this function will also distribute some free amount
    /// to accounts specified in `DistributionAccounts` struct and buy-back and burn some amount
    /// of VAL token.
    ///
    /// Note: all fees are going to reserves.
    ///
    /// TODO: add distribution algorithm description
    /// Tokens distribution algorithm:
    /// 1. a. if R < R_e, then ...
    ///    b. else (if R >= R_e), then ...
    /// 2.
    fn buy_out(
        _dex_id: &T::DEXId,
        in_asset_id: &T::AssetId,
        out_asset_id: &T::AssetId,
        output_amount: Balance,
        from_account_id: &T::AccountId,
        to_account_id: &T::AccountId,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        let input_amount = Balance(Self::buy_tokens_out_price(out_asset_id, output_amount)?);
        let reserves_tech_account_id = Self::reserves_account_id();
        let reserves_account_id =
            Technical::<T>::tech_account_id_to_account_id(&reserves_tech_account_id)?;
        let mut R = Assets::<T>::total_balance(in_asset_id, &reserves_account_id)?;
        let total_issuance = Assets::<T>::total_issuance(out_asset_id)?;
        let R_expected = Balance(Self::sell_tokens_in_price(out_asset_id, total_issuance)?);
        let input_amount_free;
        let amount_free_coefficient: Balance = fixed!(20%).into();
        if R < R_expected {
            Technical::<T>::transfer_in(
                in_asset_id,
                &from_account_id,
                &reserves_tech_account_id,
                input_amount,
            )?;
            R = R
                .checked_add(&input_amount)
                .ok_or(Error::<T>::CalculatePriceFailed)?;
            if R > R_expected {
                input_amount_free = amount_free_coefficient * (R - R_expected);
            } else {
                input_amount_free = Balance::zero();
            }
        } else {
            input_amount_free = amount_free_coefficient * input_amount;
            let reserved_amount = input_amount - input_amount_free;
            Technical::<T>::transfer_in(
                in_asset_id,
                &from_account_id,
                &reserves_tech_account_id,
                reserved_amount,
            )?;
            R = R
                .checked_add(&reserved_amount)
                .ok_or(Error::<T>::CalculatePriceFailed)?;
        }

        if input_amount_free > Balance::zero() {
            let swapped_xor_amount = T::DEXApi::exchange(
                &reserves_account_id,
                &reserves_account_id,
                &DEXId::Polkaswap.into(),
                in_asset_id,
                out_asset_id,
                SwapAmount::with_desired_input(input_amount_free, Balance::zero()), // TODO: do we need to set `min_amount_out`?
            )?
            .amount;
            Technical::<T>::burn(out_asset_id, &reserves_tech_account_id, swapped_xor_amount)?;
            Technical::<T>::mint(out_asset_id, &reserves_tech_account_id, swapped_xor_amount)?;

            let val_holders_coefficient: Fixed = fixed!(50%);
            let val_holders_xor_alloc_coeff = val_holders_coefficient * fixed!(90%);
            let val_holders_buy_back_coefficient =
                val_holders_coefficient * (fixed!(100%) - fixed!(90%));
            let projects_coefficient = fixed!(100%) - val_holders_coefficient;
            let projects_sora_citizens_coeff = projects_coefficient * fixed!(1%);
            let projects_stores_and_shops_coeff = projects_coefficient * fixed!(4%);
            let projects_parliament_and_development_coeff = projects_coefficient * fixed!(5%);
            let projects_other_coeff = projects_coefficient * fixed!(90%);
            let distribution_accounts: DistributionAccounts<T> = Self::distribution_accounts();

            debug_assert_eq!(
                fixed!(100%),
                val_holders_xor_alloc_coeff
                    + val_holders_buy_back_coefficient
                    + projects_sora_citizens_coeff
                    + projects_stores_and_shops_coeff
                    + projects_parliament_and_development_coeff
                    + projects_other_coeff
            );

            #[rustfmt::skip]
            let distributions = vec![
                (distribution_accounts.xor_allocation, val_holders_xor_alloc_coeff),
                (distribution_accounts.sora_citizens, projects_sora_citizens_coeff),
                (distribution_accounts.stores_and_shops, projects_stores_and_shops_coeff),
                (distribution_accounts.parliament_and_development, projects_parliament_and_development_coeff),
                (distribution_accounts.projects, projects_other_coeff),
            ];
            for (to_tech_account_id, coefficient) in distributions {
                technical::Module::<T>::transfer(
                    out_asset_id,
                    &reserves_tech_account_id,
                    &to_tech_account_id,
                    swapped_xor_amount * Balance(coefficient),
                )?;
            }

            let val_amount = T::DEXApi::exchange(
                &reserves_account_id,
                &reserves_account_id,
                &DEXId::Polkaswap.into(),
                out_asset_id,
                &AssetId::VAL.into(),
                SwapAmount::with_desired_input(
                    swapped_xor_amount * Balance(val_holders_buy_back_coefficient),
                    Balance::zero(),
                ),
            )?
            .amount;
            Technical::<T>::burn(&AssetId::VAL.into(), &reserves_tech_account_id, val_amount)?;
            R = R - input_amount_free;
        }
        debug_assert_eq!(
            R,
            Assets::<T>::total_balance(in_asset_id, &reserves_account_id)?
        );
        // TODO: deal with fee.
        let fee_amount = Balance(Self::fee()) * output_amount;
        let transfer_amount = output_amount - fee_amount;
        Assets::<T>::mint(
            out_asset_id,
            &reserves_account_id,
            to_account_id,
            transfer_amount,
        )?;
        Ok(SwapOutcome::new(transfer_amount, fee_amount))
    }

    /// This function is used by `exchange` function to burn `input_amount` of `in_asset_id`
    /// and transfer calculated amount of `out_asset_id` to the receiver from reserves.
    ///
    /// If there's not enough reserves in the pool, `NotEnoughReserves` error will be returned.
    ///
    /// Note: all fees will are burned in the current version.
    fn sell_in(
        _dex_id: &T::DEXId,
        in_asset_id: &T::AssetId,
        out_asset_id: &T::AssetId,
        input_amount: Balance,
        from_account_id: &T::AccountId,
        to_account_id: &T::AccountId,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        let reserves_tech_account_id = Self::reserves_account_id();
        let reserves_account_id =
            Technical::<T>::tech_account_id_to_account_id(&reserves_tech_account_id)?;
        let output_amount = Balance(Self::sell_tokens_in_price(in_asset_id, input_amount)?);
        // TODO: deal with fee.
        Assets::<T>::burn(
            in_asset_id,
            &reserves_account_id,
            from_account_id,
            input_amount,
        )?;
        let fee_amount = Balance(Self::fee()) * output_amount;
        let transfer_amount = output_amount - fee_amount;
        let reserves_amount = Assets::<T>::total_balance(out_asset_id, &reserves_account_id)?;
        if reserves_amount < transfer_amount {
            return Err(Error::<T>::NotEnoughReserves.into());
        }
        technical::Module::<T>::transfer_out(
            out_asset_id,
            &reserves_tech_account_id,
            &to_account_id,
            transfer_amount,
        )?;
        Ok(SwapOutcome::new(transfer_amount, fee_amount))
    }

    pub fn set_reserves_account_id(account: T::TechAccountId) -> Result<(), DispatchError> {
        ReservesAcc::<T>::set(account.clone());
        let account_id = Technical::<T>::tech_account_id_to_account_id(&account)?;
        let permissions = [BURN, MINT, TRANSFER, SLASH];
        for permission in &permissions {
            permissions::Module::<T>::assign_permission(
                account_id.clone(),
                &account_id,
                *permission,
                Scope::Unlimited,
            )?;
        }
        Ok(())
    }

    pub fn set_distribution_accounts(distribution_accounts: DistributionAccounts<T>) {
        DistributionAccountsEntry::<T>::set(distribution_accounts);
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
        let base_asset_id = &T::GetBaseAssetId::get();
        // Can trade only with XOR (base asset) and USD on Polkaswap.
        *dex_id == DEXId::Polkaswap.into()
            && ((input_asset_id == &AssetId::USD.into() && output_asset_id == base_asset_id)
                || (output_asset_id == &AssetId::USD.into() && input_asset_id == base_asset_id))
    }

    fn quote(
        dex_id: &T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        swap_amount: SwapAmount<Balance>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        if !Self::can_exchange(dex_id, input_asset_id, output_asset_id) {
            return Err(CommonError::<T>::CantExchange.into());
        }
        let base_asset_id = &T::GetBaseAssetId::get();
        if input_asset_id == base_asset_id {
            match swap_amount {
                SwapAmount::WithDesiredInput {
                    desired_amount_in: base_amount_in,
                    ..
                } => {
                    let amount =
                        Self::sell_tokens_in_price(input_asset_id, base_amount_in.into())?.into();
                    Ok(SwapOutcome::new(amount, amount * Balance(Self::fee())))
                }
                SwapAmount::WithDesiredOutput {
                    desired_amount_out: _target_amount_out,
                    ..
                } => {
                    return Err(CommonError::<T>::UnsupportedSwapMethod.into());
                }
            }
        } else {
            match swap_amount {
                SwapAmount::WithDesiredInput {
                    desired_amount_in: _target_amount_in,
                    ..
                } => {
                    return Err(CommonError::<T>::UnsupportedSwapMethod.into());
                }
                SwapAmount::WithDesiredOutput {
                    desired_amount_out: base_amount_out,
                    ..
                } => {
                    let amount =
                        Self::buy_tokens_out_price(output_asset_id, base_amount_out.into())?.into();
                    Ok(SwapOutcome::new(amount, amount * Balance(Self::fee())))
                }
            }
        }
    }

    fn exchange(
        sender: &T::AccountId,
        receiver: &T::AccountId,
        dex_id: &T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        desired_amount: SwapAmount<Balance>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        let reserves_account_id =
            &Technical::<T>::tech_account_id_to_account_id(&Self::reserves_account_id())?;
        // This is needed to prevent recursion calls.
        if sender == reserves_account_id && receiver == reserves_account_id {
            return Err(Error::<T>::CantExchangeOnItself.into());
        }
        if !Self::can_exchange(dex_id, input_asset_id, output_asset_id) {
            return Err(CommonError::<T>::CantExchange.into());
        }
        let base_asset_id = &T::GetBaseAssetId::get();
        if input_asset_id == base_asset_id {
            match desired_amount {
                SwapAmount::WithDesiredInput {
                    desired_amount_in: base_amount_in,
                    ..
                } => Self::sell_in(
                    dex_id,
                    input_asset_id,
                    output_asset_id,
                    base_amount_in,
                    sender,
                    receiver,
                ),
                SwapAmount::WithDesiredOutput {
                    desired_amount_out: _target_amount_out,
                    ..
                } => {
                    return Err(CommonError::<T>::UnsupportedSwapMethod.into());
                }
            }
        } else {
            match desired_amount {
                SwapAmount::WithDesiredInput {
                    desired_amount_in: _target_amount_in,
                    ..
                } => {
                    return Err(CommonError::<T>::UnsupportedSwapMethod.into());
                }
                SwapAmount::WithDesiredOutput {
                    desired_amount_out: base_amount_out,
                    ..
                } => Self::buy_out(
                    dex_id,
                    input_asset_id,
                    output_asset_id,
                    base_amount_out,
                    sender,
                    receiver,
                ),
            }
        }
    }
}
