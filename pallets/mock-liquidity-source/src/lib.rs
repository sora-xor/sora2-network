#![cfg_attr(not(feature = "std"), no_std)]

use common::prelude::*;
use frame_support::{
    decl_error, decl_event, decl_module, decl_storage,
    dispatch::{DispatchError, DispatchResult},
    ensure,
    traits::Get,
};
use frame_system::ensure_signed;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub trait Trait: common::Trait + assets::Trait {
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
    type GetFee: Get<Fixed>;
    type EnsureDEXOwner: EnsureDEXOwner<Self::DEXId, Self::AccountId, DispatchError>;
    type EnsureTradingPairExists: EnsureTradingPairExists<Self::DEXId, Self::AssetId, DispatchError>;
}

decl_storage! {
    trait Store for Module<T: Trait> as MockLiquiditySourceModule {
        pub Reserves get(fn price): double_map hasher(blake2_128_concat) T::DEXId, hasher(blake2_128_concat) T::AssetId => (Fixed, Fixed);
    }

    add_extra_genesis {
        config(reserves): Vec<(T::DEXId, T::AssetId, (Fixed, Fixed))>;

        build(|config: &GenesisConfig<T>| {
            config.reserves.iter().for_each(|(dex_id, target_asset_id, reserve_pair)| {
                <Reserves<T>>::insert(dex_id, target_asset_id, reserve_pair);
            })
        })
    }
}

decl_event!(
    pub enum Event<T>
    where
        AccountId = <T as frame_system::Trait>::AccountId,
        AssetId = <T as assets::Trait>::AssetId,
    {
        ReservesUpdated(AccountId, AssetId, Fixed, AssetId, Fixed),
    }
);

// Errors inform users that something went wrong.
decl_error! {
    pub enum Error for Module<T: Trait> {
        PairDoesNotExist,
        InsufficientInputAmount,
        InsufficientOutputAmount,
        InsufficientLiquidity,
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        type Error = Error<T>;

        fn deposit_event() = default;

        // example, this checks should be called at the beginning of management functions of actual liquidity sources, e.g. register, set_fee
        #[weight = 0]
        pub fn test_access(origin, dex_id: T::DEXId, target_id: T::AssetId) -> DispatchResult {
            let _who = T::EnsureDEXOwner::ensure_dex_owner(&dex_id, origin)?;
            T::EnsureTradingPairExists::ensure_trading_pair_exists(&dex_id, &target_id)?;
            Ok(())
        }

        #[weight = 0]
        pub fn set_reserve(origin, dex_id: T::DEXId, target_id: T::AssetId, base_reserve: Fixed, target_reserve: Fixed) -> DispatchResult {
            let _who = ensure_signed(origin)?;
            <Reserves<T>>::insert(dex_id, target_id, (base_reserve, target_reserve));
            Ok(())
        }
    }
}

impl<T: Trait> Module<T> {
    fn get_base_amount_out(
        target_amount_in: Fixed,
        base_reserve: Fixed,
        target_reserve: Fixed,
    ) -> Result<SwapOutcome<Fixed>, DispatchError> {
        let zero = Fixed::from_inner(0);
        ensure!(target_amount_in > zero, <Error<T>>::InsufficientInputAmount);
        ensure!(
            base_reserve > zero && target_reserve > zero,
            <Error<T>>::InsufficientLiquidity
        );
        let numerator = target_amount_in * base_reserve;
        let denominator = target_reserve + target_amount_in;
        let amount_out_without_fee = numerator / denominator;
        let fee_amount = amount_out_without_fee * T::GetFee::get();
        Ok(SwapOutcome::new(
            amount_out_without_fee - fee_amount,
            fee_amount,
        ))
    }

    fn get_target_amount_out(
        base_amount_in: Fixed,
        base_reserve: Fixed,
        target_reserve: Fixed,
    ) -> Result<SwapOutcome<Fixed>, DispatchError> {
        let zero = Fixed::from_inner(0);
        ensure!(base_amount_in > zero, <Error<T>>::InsufficientInputAmount);
        ensure!(
            base_reserve > zero && target_reserve > zero,
            <Error<T>>::InsufficientLiquidity
        );
        let fee_amount = base_amount_in * T::GetFee::get();
        let amount_in_with_fee = base_amount_in - fee_amount;
        let numerator = amount_in_with_fee * target_reserve;
        let denominator = base_reserve + amount_in_with_fee;
        Ok(SwapOutcome::new(numerator / denominator, fee_amount))
    }

    fn get_base_amount_in(
        target_amount_out: Fixed,
        base_reserve: Fixed,
        target_reserve: Fixed,
    ) -> Result<SwapOutcome<Fixed>, DispatchError> {
        let zero = Fixed::from_inner(0);
        ensure!(
            target_amount_out > zero,
            <Error<T>>::InsufficientOutputAmount
        );
        ensure!(
            base_reserve > zero && target_reserve > zero,
            <Error<T>>::InsufficientLiquidity
        );
        let numerator = base_reserve * target_amount_out;
        let denominator = target_reserve - target_amount_out;
        let base_amount_in_without_fee = numerator / denominator;
        let base_amount_in_with_fee =
            base_amount_in_without_fee / (Fixed::from(1) - T::GetFee::get());
        let amount_in =
            if Self::get_target_amount_out(base_amount_in_with_fee, base_reserve, target_reserve)?
                .amount
                < target_amount_out
            {
                base_amount_in_with_fee + Fixed::from_inner(1)
            } else {
                base_amount_in_with_fee
            };
        Ok(SwapOutcome::new(
            amount_in,
            base_amount_in_with_fee - base_amount_in_without_fee,
        ))
    }

    fn get_target_amount_in(
        base_amount_out: Fixed,
        base_reserve: Fixed,
        target_reserve: Fixed,
    ) -> Result<SwapOutcome<Fixed>, DispatchError> {
        let zero = Fixed::from_inner(0);
        ensure!(base_amount_out > zero, <Error<T>>::InsufficientOutputAmount);
        ensure!(
            base_reserve > zero && target_reserve > zero,
            <Error<T>>::InsufficientLiquidity
        );
        let base_amount_out_with_fee = base_amount_out / (Fixed::from(1) - T::GetFee::get());
        let numerator = target_reserve * base_amount_out_with_fee;
        let denominator = base_reserve - base_amount_out_with_fee;
        let target_amount_in = numerator / denominator;
        let amount_in =
            if Self::get_base_amount_out(target_amount_in, base_reserve, target_reserve)?.amount
                < base_amount_out
            {
                target_amount_in + Fixed::from_inner(1)
            } else {
                target_amount_in
            };
        Ok(SwapOutcome::new(
            amount_in,
            base_amount_out_with_fee - base_amount_out,
        ))
    }
}

impl<T: Trait> LiquiditySource<T::DEXId, T::AccountId, T::AssetId, Fixed, DispatchError>
    for Module<T>
{
    fn can_exchange(
        dex_id: &T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
    ) -> bool {
        let base_asset_id = &T::GetBaseAssetId::get();
        if input_asset_id == base_asset_id {
            <Reserves<T>>::contains_key(dex_id, output_asset_id)
        } else if output_asset_id == base_asset_id {
            <Reserves<T>>::contains_key(dex_id, input_asset_id)
        } else {
            <Reserves<T>>::contains_key(dex_id, output_asset_id)
                && <Reserves<T>>::contains_key(dex_id, input_asset_id)
        }
    }

    fn quote(
        dex_id: &T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        swap_amount: SwapAmount<Fixed>,
    ) -> Result<SwapOutcome<Fixed>, DispatchError> {
        let base_asset_id = &T::GetBaseAssetId::get();
        if input_asset_id == base_asset_id {
            let (base_reserve, target_reserve) = <Reserves<T>>::get(dex_id, output_asset_id);
            match swap_amount {
                SwapAmount::WithDesiredInput {
                    desired_amount_in: base_amount_in,
                    ..
                } => Ok(Self::get_target_amount_out(
                    base_amount_in,
                    base_reserve,
                    target_reserve,
                )?),
                SwapAmount::WithDesiredOutput {
                    desired_amount_out: target_amount_out,
                    ..
                } => Ok(Self::get_base_amount_in(
                    target_amount_out,
                    base_reserve,
                    target_reserve,
                )?),
            }
        } else if output_asset_id == base_asset_id {
            let (base_reserve, target_reserve) = <Reserves<T>>::get(dex_id, input_asset_id);
            match swap_amount {
                SwapAmount::WithDesiredInput {
                    desired_amount_in: target_amount_in,
                    ..
                } => Ok(Self::get_base_amount_out(
                    target_amount_in,
                    base_reserve,
                    target_reserve,
                )?),
                SwapAmount::WithDesiredOutput {
                    desired_amount_out: base_amount_out,
                    ..
                } => Ok(Self::get_target_amount_in(
                    base_amount_out,
                    base_reserve,
                    target_reserve,
                )?),
            }
        } else {
            let (base_reserve_a, target_reserve_a) = <Reserves<T>>::get(dex_id, input_asset_id);
            let (base_reserve_b, target_reserve_b) = <Reserves<T>>::get(dex_id, output_asset_id);
            match swap_amount {
                SwapAmount::WithDesiredInput {
                    desired_amount_in, ..
                } => {
                    let outcome_a = Self::get_base_amount_out(
                        desired_amount_in,
                        base_reserve_a,
                        target_reserve_a,
                    )?;
                    let outcome_b = Self::get_target_amount_out(
                        outcome_a.amount,
                        base_reserve_b,
                        target_reserve_b,
                    )?;
                    Ok(SwapOutcome::new(
                        outcome_b.amount,
                        outcome_a.fee + outcome_b.fee,
                    ))
                }
                SwapAmount::WithDesiredOutput {
                    desired_amount_out, ..
                } => {
                    let outcome_b = Self::get_base_amount_in(
                        desired_amount_out,
                        base_reserve_b,
                        target_reserve_b,
                    )?;
                    let outcome_a = Self::get_target_amount_in(
                        outcome_b.amount,
                        base_reserve_a,
                        target_reserve_a,
                    )?;
                    Ok(SwapOutcome::new(
                        outcome_a.amount,
                        outcome_b.fee + outcome_a.fee,
                    ))
                }
            }
        }
    }

    fn exchange(
        _sender: &T::AccountId,
        _receiver: &T::AccountId,
        dex_id: &T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        desired_amount: SwapAmount<Fixed>,
    ) -> Result<SwapOutcome<Fixed>, DispatchError> {
        // actual exchange does not happen
        Self::quote(dex_id, input_asset_id, output_asset_id, desired_amount)
    }
}
