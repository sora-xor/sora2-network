#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode, MaxEncodedLen};
use common::Balance;
/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://docs.substrate.io/reference/frame-pallets/>
pub use pallet::*;
use scale_info::TypeInfo;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[derive(Debug, Clone, Encode, Decode, MaxEncodedLen, TypeInfo, PartialEq, Eq, PartialOrd, Ord)]
pub struct CollateralInfo {
    pub min_ratio: sp_arithmetic::FixedU128,
    pub deposit_ratio: sp_arithmetic::FixedU128,
    pub stability_fee: sp_arithmetic::FixedU128,
}

#[derive(Debug, Clone, Encode, Decode, MaxEncodedLen, TypeInfo, PartialEq, Eq, PartialOrd, Ord)]
pub struct VaultInfo<AccountId, AssetId> {
    pub issuer: AccountId,
    pub asset_id: AssetId,
    pub collateral_amount: Balance,
    pub kusd_amount: Balance,
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use common::{AccountIdOf, ReferencePriceProvider};
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::{BlockNumberFor, *};
    use sp_arithmetic::traits::CheckedMul;
    use sp_core::H256;
    use sp_runtime::BoundedVec;
    use traits::MultiCurrency;

    pub type AssetIdOf<T> = <<T as Config>::Currency as MultiCurrency<AccountIdOf<T>>>::CurrencyId;

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type ReferencePriceProvider: ReferencePriceProvider<AccountIdOf<Self>, AssetIdOf<Self>>;
        type Currency: MultiCurrency<AccountIdOf<Self>, Balance = Balance>;
        type ReservesAccount: Get<AccountIdOf<Self>>;
        type KUSDAssetId: Get<AssetIdOf<T>>;
        type FeeScheduleMaxPerBlock: Get<u32>;
    }

    #[pallet::storage]
    #[pallet::getter(fn collateral_info)]
    pub type Collaterals<T> = StorageMap<_, Identity, AssetIdOf<T>, CollateralInfo>;

    #[pallet::storage]
    #[pallet::getter(fn vault_info)]
    pub type Vaults<T> = StorageMap<_, Identity, H256, VaultInfo<AccountIdOf<T>, AssetIdOf<T>>>;

    #[pallet::storage]
    #[pallet::getter(fn user_vault)]
    pub type UserVaults<T> =
        StorageDoubleMap<_, Identity, AccountIdOf<T>, Identity, AssetIdOf<T>, H256>;

    #[pallet::storage]
    #[pallet::getter(fn fee_schedule)]
    pub type StabilityFeeSchedule<T: Config> = StorageMap<
        _,
        Identity,
        BlockNumberFor<T>,
        BoundedVec<H256, T::FeeScheduleMaxPerBlock>,
        ValueQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        Deposit {},
    }

    #[pallet::error]
    pub enum Error<T> {
        ArithmeticError,
        VaultNotFound,
        CollateralInfoNotFound,
        NotEnoughCollateral,
        NotEnoughKUSD,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(10_000 + T::DbWeight::get().writes(1).ref_time())]
        pub fn create_vault(
            origin: OriginFor<T>,
            collateral_asset_id: AssetIdOf<T>,
            collateral_amount: Balance,
            desired_kusd_amount: Balance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            T::Currency::ensure_can_withdraw(collateral_asset_id, &who, collateral_amount)?;

            ensure!(
                Self::deposit_collateral_amount(collateral_asset_id, desired_kusd_amount)?
                    <= collateral_amount,
                Error::<T>::NotEnoughCollateral
            );

            T::Currency::transfer(
                &collateral_asset_id,
                &who,
                &T::ReservesAccount::get(),
                collateral_amount,
            )?;
            T::Currency::deposit(&T::KUSDAssetId::get(), &who, &desired_kusd_amount)?;

            Ok(())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(10_000 + T::DbWeight::get().writes(1).ref_time())]
        pub fn destroy_vault(origin: OriginFor<T>, vault_id: H256) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Ok(())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(10_000 + T::DbWeight::get().writes(1).ref_time())]
        pub fn deposit(
            origin: OriginFor<T>,
            vault_id: H256,
            collateral_amount: Balance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Ok(())
        }

        #[pallet::call_index(3)]
        #[pallet::weight(10_000 + T::DbWeight::get().writes(1).ref_time())]
        pub fn withdraw(
            origin: OriginFor<T>,
            vault_id: H256,
            collateral_amount: Balance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Ok(())
        }

        #[pallet::call_index(4)]
        #[pallet::weight(10_000 + T::DbWeight::get().writes(1).ref_time())]
        pub fn borrow(
            origin: OriginFor<T>,
            vault_id: H256,
            kusd_amount: Balance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Ok(())
        }

        #[pallet::call_index(5)]
        #[pallet::weight(10_000 + T::DbWeight::get().writes(1).ref_time())]
        pub fn unborrow(
            origin: OriginFor<T>,
            vault_id: H256,
            kusd_amount: Balance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Ok(())
        }

        #[pallet::call_index(6)]
        #[pallet::weight(10_000 + T::DbWeight::get().writes(1).ref_time())]
        pub fn add_collateral_asset(
            origin: OriginFor<T>,
            collateral_asset_id: AssetIdOf<T>,
            info: CollateralInfo,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Ok(())
        }

        #[pallet::call_index(7)]
        #[pallet::weight(10_000 + T::DbWeight::get().writes(1).ref_time())]
        pub fn disable_collateral_asset(
            origin: OriginFor<T>,
            collateral_asset_id: AssetIdOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Ok(())
        }

        #[pallet::call_index(8)]
        #[pallet::weight(10_000 + T::DbWeight::get().writes(1).ref_time())]
        pub fn enable_collateral_asset(
            origin: OriginFor<T>,
            collateral_asset_id: AssetIdOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        fn ensure_may_destroy_vault(who: AccountIdOf<T>, vault_id: H256) -> DispatchResult {
            let vault_info = Self::vault_info(&vault_id).ok_or(Error::<T>::VaultNotFound)?;
            if who == vault_info.issuer {
                return Ok(());
            }
            Ok(())
        }

        fn min_collateral_amount(
            collateral_asset_id: AssetIdOf<T>,
            kusd_amount: Balance,
        ) -> Result<Balance, DispatchError> {
            let collateral_info = Self::collateral_info(&collateral_asset_id)
                .ok_or(Error::<T>::CollateralInfoNotFound)?;
            let collateral_reference_price =
                T::ReferencePriceProvider::get_reference_price(&collateral_asset_id)?;
            let collateral_reference_price =
                sp_arithmetic::FixedU128::from_inner(collateral_reference_price);
            let kusd_amount = sp_arithmetic::FixedU128::from_inner(kusd_amount);
            let min_collateral_amount = collateral_reference_price
                .checked_mul(&collateral_info.min_ratio)
                .and_then(|x| x.checked_mul(&kusd_amount))
                .ok_or(Error::<T>::ArithmeticError)?;
            Ok(min_collateral_amount.into_inner())
        }

        fn deposit_collateral_amount(
            collateral_asset_id: AssetIdOf<T>,
            kusd_amount: Balance,
        ) -> Result<Balance, DispatchError> {
            let collateral_info = Self::collateral_info(&collateral_asset_id)
                .ok_or(Error::<T>::CollateralInfoNotFound)?;
            let collateral_reference_price =
                T::ReferencePriceProvider::get_reference_price(&collateral_asset_id)?;
            let collateral_reference_price =
                sp_arithmetic::FixedU128::from_inner(collateral_reference_price);
            let kusd_amount = sp_arithmetic::FixedU128::from_inner(kusd_amount);
            let min_collateral_amount = collateral_reference_price
                .checked_mul(&collateral_info.deposit_ratio)
                .and_then(|x| x.checked_mul(&kusd_amount))
                .ok_or(Error::<T>::ArithmeticError)?;
            Ok(min_collateral_amount.into_inner())
        }
    }
}
