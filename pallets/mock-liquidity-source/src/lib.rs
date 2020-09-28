#![cfg_attr(not(feature = "std"), no_std)]

use common::prelude::*;
use frame_support::{
    decl_error, decl_event, decl_module, decl_storage,
    dispatch::{DispatchError, DispatchResult},
    ensure,
    traits::Get,
};
use frame_system::ensure_signed;
use permissions::{BURN, EXCHANGE, MINT, SLASH, TRANSFER};

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub trait Trait<I: Instance>: common::Trait + assets::Trait + technical::Trait {
    type Event: From<Event<Self, I>> + Into<<Self as frame_system::Trait>::Event>;
    type GetFee: Get<Fixed>;
    type EnsureDEXOwner: EnsureDEXOwner<Self::DEXId, Self::AccountId, DispatchError>;
    type EnsureTradingPairExists: EnsureTradingPairExists<Self::DEXId, Self::AssetId, DispatchError>;
}

decl_storage! {
    trait Store for Module<T: Trait<I>, I: Instance> as MockLiquiditySourceModule {
        pub Reserves get(fn reserves): double_map hasher(blake2_128_concat) T::DEXId, hasher(blake2_128_concat) T::AssetId => (Fixed, Fixed);
        pub ReservesAcc get(fn reserves_account_id): T::TechAccountId;
    }

    add_extra_genesis {
        config(phantom): sp_std::marker::PhantomData<I>;
        config(reserves): Vec<(T::DEXId, T::AssetId, (Fixed, Fixed))>;
        build(|config| Module::<T, I>::initialize_reserves(&config.reserves))
    }
}

decl_event!(
    pub enum Event<T, I>
    where
        AccountId = <T as frame_system::Trait>::AccountId,
        AssetId = <T as assets::Trait>::AssetId,
    {
        ReservesUpdated(AccountId, AssetId, Fixed, AssetId, Fixed),
    }
);

// Errors inform users that something went wrong.
decl_error! {
    pub enum Error for Module<T: Trait<I>, I: Instance> {
        PairDoesNotExist,
        InsufficientInputAmount,
        InsufficientOutputAmount,
        InsufficientLiquidity,
    }
}

decl_module! {
    pub struct Module<T: Trait<I>, I: Instance> for enum Call where origin: T::Origin {
        type Error = Error<T, I>;

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
            <Reserves<T, I>>::insert(dex_id, target_id, (base_reserve, target_reserve));
            Ok(())
        }
    }
}

#[allow(non_snake_case)]
impl<T: Trait<I>, I: Instance> Module<T, I> {
    fn initialize_reserves(reserves: &[(T::DEXId, T::AssetId, (Fixed, Fixed))]) {
        reserves
            .iter()
            .for_each(|(dex_id, target_asset_id, reserve_pair)| {
                <Reserves<T, I>>::insert(dex_id, target_asset_id, reserve_pair);
            })
    }

    fn get_base_amount_out(
        target_amount_in: Fixed,
        base_reserve: Fixed,
        target_reserve: Fixed,
    ) -> Result<SwapOutcome<Fixed>, DispatchError> {
        let zero = Fixed::from_inner(0);
        ensure!(
            target_amount_in > zero,
            <Error<T, I>>::InsufficientInputAmount
        );
        ensure!(
            base_reserve > zero && target_reserve > zero,
            <Error<T, I>>::InsufficientLiquidity
        );
        let X: FixedWrapper = base_reserve.into();
        let Y: FixedWrapper = target_reserve.into();
        let d_Y: FixedWrapper = target_amount_in.into();

        let amount_out_without_fee = (d_Y * X / (Y + d_Y))
            .get()
            .ok_or(Error::<T, I>::InsufficientLiquidity)?;

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
        ensure!(
            base_amount_in > zero,
            <Error<T, I>>::InsufficientInputAmount
        );
        ensure!(
            base_reserve > zero && target_reserve > zero,
            <Error<T, I>>::InsufficientLiquidity
        );
        let fee_amount = base_amount_in * T::GetFee::get();
        let amount_in_with_fee = base_amount_in - fee_amount;
        
        let X: FixedWrapper = base_reserve.into();
        let Y: FixedWrapper = target_reserve.into();
        let d_X: FixedWrapper = amount_in_with_fee.into();

        let amount_out = (Y * d_X / (X + d_X))
            .get()
            .ok_or(Error::<T, I>::InsufficientLiquidity)?;

        Ok(SwapOutcome::new(amount_out, fee_amount))
    }

    fn get_base_amount_in(
        target_amount_out: Fixed,
        base_reserve: Fixed,
        target_reserve: Fixed,
    ) -> Result<SwapOutcome<Fixed>, DispatchError> {
        let zero = Fixed::from_inner(0);
        ensure!(
            target_amount_out > zero,
            <Error<T, I>>::InsufficientOutputAmount
        );
        ensure!(
            base_reserve > zero && target_reserve > zero,
            <Error<T, I>>::InsufficientLiquidity
        );

        let X: FixedWrapper = base_reserve.into();
        let Y: FixedWrapper = target_reserve.into();
        let d_Y: FixedWrapper = target_amount_out.into();

        let base_amount_in_without_fee = (X * d_Y/ (Y - d_Y))
            .get()
            .ok_or(Error::<T, I>::InsufficientLiquidity)?;

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
        ensure!(
            base_amount_out > zero,
            <Error<T, I>>::InsufficientOutputAmount
        );
        ensure!(
            base_reserve > zero && target_reserve > zero,
            <Error<T, I>>::InsufficientLiquidity
        );

        let base_amount_out_with_fee = base_amount_out / (Fixed::from(1) - T::GetFee::get());

        let X: FixedWrapper = base_reserve.into();
        let Y: FixedWrapper = target_reserve.into();
        let d_X: FixedWrapper = base_amount_out_with_fee.into();

        let target_amount_in = (Y * d_X/ (X - d_X))
            .get()
            .ok_or(Error::<T, I>::InsufficientLiquidity)?;

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

impl<T: Trait<I>, I: Instance> Module<T, I> {
    pub fn set_reserves_account_id(account: T::TechAccountId) -> Result<(), DispatchError> {
        ReservesAcc::<T, I>::set(account.clone());
        let account_id = technical::Module::<T>::tech_account_id_to_account_id(&account)?;
        let permission_obj = permissions::Permission::<T>::any(account_id.clone());
        let permissions = [BURN, MINT, TRANSFER, SLASH, EXCHANGE];
        for permission in &permissions {
            permissions::Module::<T>::create_permission(
                account_id.clone(),
                account_id.clone(),
                *permission,
                permission_obj.clone(),
            )?;
        }
        Ok(())
    }
}

impl<T: Trait<I>, I: Instance>
    LiquiditySource<T::DEXId, T::AccountId, T::AssetId, Fixed, DispatchError> for Module<T, I>
{
    fn can_exchange(
        dex_id: &T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
    ) -> bool {
        let base_asset_id = &T::GetBaseAssetId::get();
        if input_asset_id == base_asset_id {
            <Reserves<T, I>>::contains_key(dex_id, output_asset_id)
        } else if output_asset_id == base_asset_id {
            <Reserves<T, I>>::contains_key(dex_id, input_asset_id)
        } else {
            <Reserves<T, I>>::contains_key(dex_id, output_asset_id)
                && <Reserves<T, I>>::contains_key(dex_id, input_asset_id)
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
            let (base_reserve, target_reserve) = <Reserves<T, I>>::get(dex_id, output_asset_id);
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
            let (base_reserve, target_reserve) = <Reserves<T, I>>::get(dex_id, input_asset_id);
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
            let (base_reserve_a, target_reserve_a) = <Reserves<T, I>>::get(dex_id, input_asset_id);
            let (base_reserve_b, target_reserve_b) = <Reserves<T, I>>::get(dex_id, output_asset_id);
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
