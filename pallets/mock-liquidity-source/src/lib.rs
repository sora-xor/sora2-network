#![cfg_attr(not(feature = "std"), no_std)]

use common::fixnum::ops::Numeric;
use common::{fixed, prelude::*};
use frame_support::{
    decl_error, decl_event, decl_module, decl_storage,
    dispatch::{DispatchError, DispatchResult},
    ensure,
    traits::Get,
};
use frame_system::ensure_signed;
use permissions::{Scope, BURN, MINT, SLASH, TRANSFER};

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub trait Trait<I: Instance>: common::Trait + assets::Trait + technical::Trait {
    type Event: From<Event<Self, I>> + Into<<Self as frame_system::Trait>::Event>;
    type GetFee: Get<Fixed>;
    type EnsureDEXManager: EnsureDEXManager<Self::DEXId, Self::AccountId, DispatchError>;
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
        /// Specified parameters lead to arithmetic error
        CalculationError,
    }
}

decl_module! {
    pub struct Module<T: Trait<I>, I: Instance> for enum Call where origin: T::Origin {
        type Error = Error<T, I>;

        fn deposit_event() = default;

        // example, this checks should be called at the beginning of management functions of actual liquidity sources, e.g. register, set_fee
        #[weight = 0]
        pub fn test_access(origin, dex_id: T::DEXId, target_id: T::AssetId) -> DispatchResult {
            let _who = T::EnsureDEXManager::ensure_can_manage(&dex_id, origin, ManagementMode::PublicCreation)?;
            T::EnsureTradingPairExists::ensure_trading_pair_exists(&dex_id, &T::GetBaseAssetId::get(), &target_id)?;
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
        let zero = fixed!(0);
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

        let amount_out_without_fee = (d_Y.clone() * X / (Y + d_Y))
            .get()
            .map_err(|_| Error::<T, I>::InsufficientLiquidity)?;

        let fee_fraction: FixedWrapper = T::GetFee::get().into();
        let fee_amount = amount_out_without_fee * fee_fraction;
        Ok(SwapOutcome::new(
            (amount_out_without_fee - fee_amount.clone())
                .get()
                .map_err(|_| Error::<T, I>::CalculationError)?,
            fee_amount
                .get()
                .map_err(|_| Error::<T, I>::CalculationError)?,
        ))
    }

    fn get_target_amount_out(
        base_amount_in: Fixed,
        base_reserve: Fixed,
        target_reserve: Fixed,
    ) -> Result<SwapOutcome<Fixed>, DispatchError> {
        let zero = fixed!(0);
        ensure!(
            base_amount_in > zero,
            <Error<T, I>>::InsufficientInputAmount
        );
        ensure!(
            base_reserve > zero && target_reserve > zero,
            <Error<T, I>>::InsufficientLiquidity
        );
        let fee_fraction: FixedWrapper = T::GetFee::get().into();
        let fee_amount = base_amount_in * fee_fraction;
        let amount_in_with_fee = base_amount_in - fee_amount.clone();

        let X: FixedWrapper = base_reserve.into();
        let Y: FixedWrapper = target_reserve.into();
        let d_X: FixedWrapper = amount_in_with_fee.into();

        let amount_out = (Y * d_X.clone() / (X + d_X))
            .get()
            .map_err(|_| Error::<T, I>::InsufficientLiquidity)?;
        let fee_amount = fee_amount
            .get()
            .map_err(|_| Error::<T, I>::CalculationError)?;

        Ok(SwapOutcome::new(amount_out, fee_amount))
    }

    fn get_base_amount_in(
        target_amount_out: Fixed,
        base_reserve: Fixed,
        target_reserve: Fixed,
    ) -> Result<SwapOutcome<Fixed>, DispatchError> {
        let zero = fixed!(0);
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

        let base_amount_in_without_fee = (X * d_Y.clone() / (Y - d_Y))
            .get()
            .map_err(|_| Error::<T, I>::InsufficientLiquidity)?;

        let fee_fraction: FixedWrapper = T::GetFee::get().into();
        let base_amount_in_with_fee = FixedWrapper::from(base_amount_in_without_fee)
            / (FixedWrapper::from(Fixed::ONE) - fee_fraction);
        let actual_target_amount_out = Self::get_target_amount_out(
            base_amount_in_with_fee
                .clone()
                .get()
                .map_err(|_| Error::<T, I>::CalculationError)?,
            base_reserve,
            target_reserve,
        )?
        .amount;
        let amount_in = if actual_target_amount_out < target_amount_out {
            base_amount_in_with_fee.clone() + Fixed::from_bits(1)
        } else {
            base_amount_in_with_fee.clone()
        };
        Ok(SwapOutcome::new(
            amount_in
                .get()
                .map_err(|_| Error::<T, I>::CalculationError)?,
            (base_amount_in_with_fee - base_amount_in_without_fee)
                .get()
                .map_err(|_| Error::<T, I>::CalculationError)?,
        ))
    }

    fn get_target_amount_in(
        base_amount_out: Fixed,
        base_reserve: Fixed,
        target_reserve: Fixed,
    ) -> Result<SwapOutcome<Fixed>, DispatchError> {
        let zero = fixed!(0);
        ensure!(
            base_amount_out > zero,
            <Error<T, I>>::InsufficientOutputAmount
        );
        ensure!(
            base_reserve > zero && target_reserve > zero,
            <Error<T, I>>::InsufficientLiquidity
        );

        let one: FixedWrapper = fixed!(1);
        let base_amount_out_wrapper: FixedWrapper = base_amount_out.into();
        let base_amount_out_with_fee = base_amount_out_wrapper / (one - T::GetFee::get());

        let X: FixedWrapper = base_reserve.into();
        let Y: FixedWrapper = target_reserve.into();
        let d_X = base_amount_out_with_fee.clone();

        let target_amount_in: Fixed = (Y * d_X.clone() / (X - d_X))
            .get()
            .map_err(|_| Error::<T, I>::InsufficientLiquidity)?;
        let actual_base_amount_out =
            Self::get_base_amount_out(target_amount_in, base_reserve, target_reserve)?.amount;

        let amount_in = if actual_base_amount_out < base_amount_out {
            target_amount_in + Fixed::from_bits(1).into()
        } else {
            target_amount_in.into()
        };
        let amount_in = amount_in
            .get()
            .map_err(|_| Error::<T, I>::CalculationError)?;
        let fee = (base_amount_out_with_fee - base_amount_out)
            .get()
            .map_err(|_| Error::<T, I>::CalculationError)?;
        Ok(SwapOutcome::new(amount_in, fee))
    }
}

impl<T: Trait<I>, I: Instance> Module<T, I> {
    pub fn set_reserves_account_id(account: T::TechAccountId) -> Result<(), DispatchError> {
        ReservesAcc::<T, I>::set(account.clone());
        let account_id = technical::Module::<T>::tech_account_id_to_account_id(&account)?;
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
                    let outcome_a_fee: FixedWrapper = outcome_a.fee.into();
                    let outcome_b_fee: FixedWrapper = outcome_b.fee.into();
                    let fee = (outcome_a_fee + outcome_b_fee)
                        .get()
                        .map_err(|_| Error::<T, I>::CalculationError)?;
                    Ok(SwapOutcome::new(outcome_b.amount, fee))
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
                    let outcome_a_fee: FixedWrapper = outcome_a.fee.into();
                    let outcome_b_fee: FixedWrapper = outcome_b.fee.into();
                    let fee = (outcome_b_fee + outcome_a_fee)
                        .get()
                        .map_err(|_| Error::<T, I>::CalculationError)?;
                    Ok(SwapOutcome::new(outcome_a.amount, fee))
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
