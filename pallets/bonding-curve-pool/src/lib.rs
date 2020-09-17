#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

use common::prelude::{Balance, Fixed, FixedWrapper};
use frame_support::{decl_error, decl_module, decl_storage};
use sp_runtime::{DispatchError, FixedPointNumber};

pub trait Trait: common::Trait + assets::Trait {}

decl_storage! {
    trait Store for Module<T: Trait> as BondingCurve {
        InitialPrice get(fn initial_price) config(): Fixed = Fixed::saturating_from_rational(993, 10);
        PriceChangeStep get(fn price_change_step) config(): Fixed = 5000.into();
        PriceChangeRate get(fn price_change_rate) config(): Fixed = 100.into();
        SellPriceCoefficient get(fn sell_price_coefficient) config(): Fixed = Fixed::saturating_from_rational(8, 10);
    }
}

decl_error! {
    pub enum Error for Module<T: Trait> {
        /// An error occurred while calculating the price.
        CalculatePriceFailed,
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        type Error = Error<T>;
    }
}

impl<T: Trait> Module<T> {
    /// Calculates and returns current buy price for one token.
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
    #[allow(non_snake_case)]
    pub fn buy_price(asset_id: T::AssetId) -> Result<Fixed, DispatchError> {
        let total_issuance_integer = assets::Module::<T>::total_issuance(&asset_id)?;
        let Q: FixedWrapper = total_issuance_integer.into();
        let P_I = Self::initial_price();
        let PC_S = Self::price_change_step();
        let PC_R = Self::price_change_rate();
        let price = Q / (PC_S * PC_R) + P_I;
        price.get().ok_or(Error::<T>::CalculatePriceFailed.into())
    }

    /// Calculates and returns current buy price for some amount of token.
    ///
    /// To calculate _buy_ price for a specific amount of tokens,
    /// one needs to integrate the equation of buy price (`P_B(Q)`):
    ///
    /// ```nocompile
    /// P_BT(Q, q) = integrate [P_B(x) dx, x = Q to Q+q]
    ///            = integrate [x / (PC_S * PC_R) + P_I dx, x = Q to Q+q]
    ///            = x^2 / (2 * PC_S * PC_R) + P_I * x, x = Q to Q+q
    ///            = (Q+q)^2 / (2 * PC_S * PC_R) + P_I * (Q+q) -
    ///              ( Q )^2 / (2 * PC_S * PC_R) + P_I * ( Q )
    /// ```
    /// where
    /// `P_BT(Q, q)`: buy price for `q` tokens
    /// `Q`: current token issuance (quantity)
    /// `q`: amount of tokens to buy
    #[allow(non_snake_case)]
    #[rustfmt::skip]
    pub fn buy_tokens_price(asset_id: T::AssetId, quantity: Balance) -> Result<Fixed, DispatchError> {
        let total_issuance_integer = assets::Module::<T>::total_issuance(&asset_id)?;
        let Q = FixedWrapper::from(total_issuance_integer);
        let P_I = Self::initial_price();
        let PC_S = FixedWrapper::from(Self::price_change_step());
        let PC_R = Self::price_change_rate();

        let Q_plus_q = Q + quantity;
        let two_times_PC_S_times_PC_R = 2 * PC_S * PC_R;
        let to   = Q_plus_q * Q_plus_q / two_times_PC_S_times_PC_R + P_I * Q_plus_q;
        let from = Q        * Q        / two_times_PC_S_times_PC_R + P_I * Q;
        let price: FixedWrapper = to - from;
        price.get().ok_or(Error::<T>::CalculatePriceFailed.into())
    }

    /// Calculates and returns current sell price for one token.
    /// Sell price is `P_Sc`% of buy price (see `buy_price`).
    ///
    /// `P_S = P_Sc * P_B`
    /// where
    /// `P_Sc: sell price coefficient (%)`
    #[allow(non_snake_case)]
    pub fn sell_price(asset_id: T::AssetId) -> Result<Fixed, DispatchError> {
        let P_B = Self::buy_price(asset_id)?;
        let P_Sc = FixedWrapper::from(Self::sell_price_coefficient());
        let price = P_Sc * P_B;
        price.get().ok_or(Error::<T>::CalculatePriceFailed.into())
    }

    /// Calculates and returns current sell price for some amount of token.
    /// Sell tokens price is `P_Sc`% of buy tokens price (see `buy_tokens_price`).
    ///
    /// ```nocompile
    /// P_ST = integrate [P_S dx]
    ///      = integrate [P_Sc * P_B dx]
    ///      = P_Sc * integrate [P_B dx]
    ///      = P_Sc * P_BT
    /// where
    /// `P_Sc: sell price coefficient (%)`
    /// ```
    #[allow(non_snake_case)]
    pub fn sell_tokens_price(
        asset_id: T::AssetId,
        quantity: Balance,
    ) -> Result<Fixed, DispatchError> {
        let P_BT = Self::buy_tokens_price(asset_id, quantity)?;
        let P_Sc = FixedWrapper::from(Self::sell_price_coefficient());
        let price = P_Sc * P_BT;
        price.get().ok_or(Error::<T>::CalculatePriceFailed.into())
    }
}
