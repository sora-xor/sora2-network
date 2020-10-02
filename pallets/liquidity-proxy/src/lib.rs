#![cfg_attr(not(feature = "std"), no_std)]

use common::{
    prelude::SwapAmount, Fixed, LiquidityRegistry, LiquiditySource, LiquiditySourceFilter,
    LiquiditySourceType,
};
use frame_support::{decl_error, decl_event, decl_module, sp_runtime::DispatchError};
use sp_std::vec::Vec;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub trait Trait: common::Trait + assets::Trait {
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
    type LiquidityRegistry: LiquidityRegistry<
        Self::DEXId,
        Self::AccountId,
        Self::AssetId,
        LiquiditySourceType,
        Fixed,
        DispatchError,
    >;
}

decl_event!(
    pub enum Event<T>
    where
        AccountId = <T as frame_system::Trait>::AccountId,
        AssetId = <T as assets::Trait>::AssetId,
        DEXId = <T as common::Trait>::DEXId,
    {
        /// Exchange of tokens has been performed
        /// [Caller Account, Liquidity Source Id, Input Asset Id, Output Asset Id, Input Amount, Output Amount]
        SuccessfulExchange(
            AccountId,
            DEXId,
            LiquiditySourceType,
            AssetId,
            AssetId,
            Fixed,
            Fixed,
        ),
    }
);

decl_error! {
    pub enum Error for Module<T: Trait> {
        UnavailableExchangePath,
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        type Error = Error<T>;

        fn deposit_event() = default;

        // TODO: extrinsics
    }
}

impl<T: Trait> Module<T> {
    #[allow(dead_code)]
    fn demo_function(
        dex_id: T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: Fixed,
    ) -> Result<Vec<Fixed>, DispatchError> {
        Ok(T::LiquidityRegistry::list_liquidity_sources(
            input_asset_id,
            output_asset_id,
            LiquiditySourceFilter::empty(dex_id),
        )?
        .iter()
        .map(|src| {
            // requests with certain amounts can be invalid, e.g. withdrawing more than pool reserves, here errored values are just ignored
            T::LiquidityRegistry::quote(
                src,
                input_asset_id,
                output_asset_id,
                SwapAmount::with_desired_input(amount, Fixed::from(0)),
            )
        })
        .filter_map(Result::ok)
        .map(|outcome| outcome.amount)
        .collect())
    }
}
