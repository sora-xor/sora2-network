#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode, MaxEncodedLen};
use common::{balance, AccountIdOf, Balance};
pub use pallet::*;
use scale_info::TypeInfo;
use sp_arithmetic::FixedU128;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

// Risk management parameters for the specific collateral type.
#[derive(Debug, Clone, Encode, Decode, MaxEncodedLen, TypeInfo, PartialEq, Eq, PartialOrd, Ord)]
pub struct CollateralRiskParameters {
    // Hard cap of total KUSD issued for the collateral.
    pub max_supply: Balance,

    // Loan-to-value liquidation threshold
    pub liquidation_ratio: FixedU128,

    pub stability_fee_rate: FixedU128,
}

// CDP - Collateralized Debt Position. It is a single collateral/debt record.
#[derive(Debug, Clone, Encode, Decode, MaxEncodedLen, TypeInfo, PartialEq, Eq, PartialOrd, Ord)]
pub struct CollateralizedDebtPosition<AccountId, AssetId, Moment> {
    // CDP owner
    pub owner: AccountId,

    /// Collateral
    pub collateral_asset_id: AssetId,
    pub collateral_amount: Balance,

    /// normalized outstanding debt in KUSD
    pub debt: Balance,

    /// positive balance that the protocol owe to the CDP owner
    pub balance: Balance,

    /// the last timestamp when stability fee was accrued
    pub last_fee_update_time: Moment,
}

impl<AccountId, AssetId, Moment> CollateralizedDebtPosition<AccountId, AssetId, Moment> {
    pub fn outstanding_debt(&self) -> Balance {
        if self.debt > self.balance {
            self.debt - self.balance
        } else {
            0
        }
    }
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use common::{AccountIdOf, ReferencePriceProvider};
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::{BlockNumberFor, *};
    use pallet_timestamp as timestamp;
    use sp_arithmetic::traits::{CheckedMul, EnsureAdd};
    use sp_core::{H256, U256};
    use sp_runtime::BoundedVec;
    use traits::MultiCurrency;

    pub type AssetIdOf<T> = <<T as Config>::Currency as MultiCurrency<AccountIdOf<T>>>::CurrencyId;

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::config]
    pub trait Config: frame_system::Config + timestamp::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type ReferencePriceProvider: ReferencePriceProvider<AccountIdOf<Self>, AssetIdOf<Self>>;
        type Currency: MultiCurrency<AccountIdOf<Self>, Balance = Balance>;
        type ReservesAccount: Get<AccountIdOf<Self>>;
        // TODO add KUSD AssetId
        // type KUSDAssetId: Get<AssetIdOf<T>>;

        // TODO fee scheduler
        type FeeScheduleMaxPerBlock: Get<u32>;

        /// Max number of CDPs per single user, 1024
        type MaxCDPsPerUser: Get<u32>;
    }

    pub type Timestamp<T> = timestamp::Pallet<T>;

    // TODO system live parameter

    // Current KUSD total supply
    #[pallet::storage]
    #[pallet::getter(fn kusd_supply)]
    pub type Supply<T> = StorageValue<_, Balance, ValueQuery>;

    // System profit in KUSD.
    #[pallet::storage]
    #[pallet::getter(fn profit)]
    pub type Profit<T> = StorageValue<_, Balance, ValueQuery>;

    // System bad debt, the amount of KUSD not secured with collateral.
    #[pallet::storage]
    #[pallet::getter(fn bad_debt)]
    pub type BadDebt<T> = StorageValue<_, Balance, ValueQuery>;

    // Risk parameters for collaterals
    #[pallet::storage]
    #[pallet::getter(fn collateral_risk_parameters)]
    pub type CollateralTypes<T> = StorageMap<_, Identity, AssetIdOf<T>, CollateralRiskParameters>;

    // Risk parameter
    // Hard cap of KUSD may be minted by the system
    // TODO add setter for risk managers
    #[pallet::storage]
    #[pallet::getter(fn max_supply)]
    pub type MaxSupply<T> = StorageValue<_, Balance, ValueQuery>;

    // Risk parameter
    // Liquidation penalty
    // TODO add setter for risk managers
    #[pallet::storage]
    #[pallet::getter(fn liquidation_penalty)]
    pub type LiquidationPenalty<T> = StorageValue<_, FixedU128, ValueQuery>;

    // The next CDP id
    #[pallet::storage]
    pub type NextCDPId<T> = StorageValue<_, U256, ValueQuery>;

    // Storage of all CDPs, where key is an unique CDP identifier
    #[pallet::storage]
    #[pallet::getter(fn cdp)]
    pub type Treasury<T: Config> = StorageMap<
        _,
        Identity,
        U256,
        CollateralizedDebtPosition<AccountIdOf<T>, AssetIdOf<T>, T::Moment>,
    >;

    // Index for [AccountId, AssetId -> Vec(CDP IDs)]
    #[pallet::storage]
    #[pallet::getter(fn user_cdps)]
    pub type UserCDPs<T: Config> = StorageDoubleMap<
        _,
        Identity,
        AccountIdOf<T>,
        Identity,
        AssetIdOf<T>,
        BoundedVec<U256, T::MaxCDPsPerUser>,
    >;

    // TODO fees offchain worker scheduler
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
        // TODO add all events
    }

    #[pallet::error]
    pub enum Error<T> {
        ArithmeticError,
        CDPNotFound,
        CollateralInfoNotFound,
        CDPUnsafe,
        NotEnoughCollateral,
        OperationPermitted,
        OutstandingDebt,
        CDPsPerUserLimitReached,
        HardCapSupply,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        // TODO why this weight?
        #[pallet::weight(10_000 + T::DbWeight::get().writes(1).ref_time())]
        pub fn create_cdp(
            origin: OriginFor<T>,
            collateral_asset_id: AssetIdOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            NextCDPId::<T>::try_mutate(|cdp_id| {
                cdp_id
                    .checked_add(U256::from(1))
                    .ok_or(Error::<T>::ArithmeticError)?;
                <Treasury<T>>::insert(
                    cdp_id.clone(),
                    CollateralizedDebtPosition {
                        owner: who.clone(),
                        collateral_asset_id,
                        collateral_amount: balance!(0),
                        debt: balance!(0),
                        balance: balance!(0),
                        last_fee_update_time: Timestamp::<T>::get(),
                    },
                );
                <UserCDPs<T>>::try_append(who, collateral_asset_id, cdp_id)
                    .map_err(|_| Error::<T>::CDPsPerUserLimitReached)
            })?;

            Ok(())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(10_000 + T::DbWeight::get().writes(1).ref_time())]
        pub fn close_cdp(origin: OriginFor<T>, cdp_id: U256) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_cdp_owner(&who, cdp_id)?;
            Self::accrue_internal(cdp_id)?;
            let cdp = <Treasury<T>>::get(cdp_id).ok_or(Error::<T>::CDPNotFound)?;
            ensure!(cdp.debt == 0, Error::<T>::OutstandingDebt);

            // TODO
            // return collateral
            // return KUSD positive amount

            <Treasury<T>>::remove(cdp_id);
            <UserCDPs<T>>::mutate(who, cdp.collateral_asset_id, |cdp_ids| {
                if let Some(cdps) = cdp_ids {
                    cdps.retain(|x| *x != cdp_id);
                    if cdps.is_empty() {
                        *cdp_ids = None;
                    }
                };
            });
            Ok(())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(10_000 + T::DbWeight::get().writes(1).ref_time())]
        pub fn deposit_collateral(
            origin: OriginFor<T>,
            cdp_id: U256,
            collateral_amount: Balance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // TODO
            // ensure cdp asset is collateral asset id
            // transfer collateral to tech account

            <Treasury<T>>::try_mutate(cdp_id, {
                |cdp| {
                    cdp.as_mut()
                        .ok_or(Error::<T>::CDPNotFound)?
                        .collateral_amount
                        .checked_add(collateral_amount)
                        .ok_or(Error::<T>::ArithmeticError)
                }
            })?;

            Ok(())
        }

        #[pallet::call_index(3)]
        #[pallet::weight(10_000 + T::DbWeight::get().writes(1).ref_time())]
        pub fn withdraw_collateral(
            origin: OriginFor<T>,
            cdp_id: U256,
            collateral_amount: Balance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_cdp_owner(&who, cdp_id)?;
            let cdp = <Treasury<T>>::get(cdp_id).ok_or(Error::<T>::CDPNotFound)?;
            ensure!(
                cdp.collateral_amount >= collateral_amount,
                Error::<T>::NotEnoughCollateral
            );
            Self::ensure_cdp_safe(
                cdp.outstanding_debt(),
                cdp.collateral_amount
                    .checked_sub(collateral_amount)
                    .ok_or(Error::<T>::ArithmeticError)?,
                cdp.collateral_asset_id,
            )?;

            // TODO
            // transfer from tech account to who

            <Treasury<T>>::try_mutate(cdp_id, {
                |cdp| {
                    cdp.as_mut()
                        .ok_or(Error::<T>::CDPNotFound)?
                        .collateral_amount
                        .checked_sub(collateral_amount)
                        .ok_or(Error::<T>::ArithmeticError)
                }
            })?;

            Ok(())
        }

        // TODO give better name that describe that user can borrow or get positive balance
        #[pallet::call_index(4)]
        #[pallet::weight(10_000 + T::DbWeight::get().writes(1).ref_time())]
        pub fn borrow(
            origin: OriginFor<T>,
            cdp_id: U256,
            will_to_borrow_amount: Balance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_cdp_owner(&who, cdp_id)?;
            Self::accrue_internal(cdp_id)?;
            let cdp = <Treasury<T>>::get(cdp_id).ok_or(Error::<T>::CDPNotFound)?;
            Self::ensure_cdp_safe(
                cdp.outstanding_debt()
                    .checked_add(will_to_borrow_amount)
                    .ok_or(Error::<T>::ArithmeticError)?,
                cdp.collateral_amount,
                cdp.collateral_asset_id,
            )?;
            let to_mint = if will_to_borrow_amount > cdp.balance {
                will_to_borrow_amount
                    .checked_sub(cdp.balance)
                    .ok_or(Error::<T>::ArithmeticError)?
            } else {
                0
            };
            let to_transfer = if cdp.balance > will_to_borrow_amount {
                cdp.balance
                    .checked_sub(will_to_borrow_amount)
                    .ok_or(Error::<T>::ArithmeticError)?
            } else {
                cdp.balance
            };
            Self::ensure_collateral_cap(cdp.collateral_asset_id, to_mint)?;
            Self::ensure_protocol_cap(to_mint)?;

            // TODO
            // mint to_mint KUSD amount to who
            // transfer to_transfer KUSD amount to who

            <Treasury<T>>::try_mutate(cdp_id, {
                |cdp| {
                    let cdp = cdp.as_mut().ok_or(Error::<T>::CDPNotFound)?;
                    cdp.debt
                        .checked_add(to_mint)
                        .ok_or(Error::<T>::ArithmeticError)?;
                    cdp.balance
                        .checked_sub(to_transfer)
                        .ok_or(Error::<T>::ArithmeticError)
                }
            })?;
            <Supply<T>>::try_mutate({
                |supply| {
                    supply
                        .checked_add(to_mint)
                        .ok_or(Error::<T>::ArithmeticError)
                }
            })?;

            Ok(())
        }

        #[pallet::call_index(5)]
        #[pallet::weight(10_000 + T::DbWeight::get().writes(1).ref_time())]
        pub fn repay_debt(origin: OriginFor<T>, cdp_id: U256, amount: Balance) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::accrue_internal(cdp_id)?;
            let cdp = <Treasury<T>>::get(cdp_id).ok_or(Error::<T>::CDPNotFound)?;

            // if repaying amount exceeds debt, leftover goes to the CDP owner
            let to_cover_debt = amount.min(cdp.debt);
            let leftover = if amount > cdp.debt {
                amount
                    .checked_sub(cdp.debt)
                    .ok_or(Error::<T>::ArithmeticError)?
            } else {
                0
            };

            // TODO
            // burn to_cover_debt KUSD

            <Treasury<T>>::try_mutate(cdp_id, {
                |cdp| {
                    let cdp = cdp.as_mut().ok_or(Error::<T>::CDPNotFound)?;
                    cdp.debt
                        .checked_sub(to_cover_debt)
                        .ok_or(Error::<T>::ArithmeticError)?;
                    cdp.balance
                        .checked_add(leftover)
                        .ok_or(Error::<T>::ArithmeticError)
                }
            })?;
            <Supply<T>>::try_mutate({
                |supply| {
                    supply
                        .checked_sub(to_cover_debt)
                        .ok_or(Error::<T>::ArithmeticError)
                }
            })?;

            Ok(())
        }

        #[pallet::call_index(6)]
        #[pallet::weight(10_000 + T::DbWeight::get().writes(1).ref_time())]
        pub fn liquidate(
            origin: OriginFor<T>,
            cdp_id: U256,
            kusd_amount: Balance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_liquidator(&who)?;
            Self::accrue_internal(cdp_id)?;

            // TODO
            // ensure CDP is unsafe, LTV threshold
            // repay_debt(kusd_amount)
            // if outstanding debt?
            //   compensate with protocol profit balance, burn leftover KUSD
            //   if not enough, increase bad debt
            // calculate liquidation_penalty
            // transfer to protocol Profit
            // leftover goes to CDP owner

            Ok(())
        }

        #[pallet::call_index(7)]
        #[pallet::weight(10_000 + T::DbWeight::get().writes(1).ref_time())]
        pub fn accrue(origin: OriginFor<T>, cdp_id: U256) -> DispatchResult {
            // TODO can unsigned do it?
            let who = ensure_signed(origin)?;
            Self::accrue_internal(cdp_id)?;
            Ok(())
        }

        #[pallet::call_index(8)]
        #[pallet::weight(10_000 + T::DbWeight::get().writes(1).ref_time())]
        pub fn add_collateral_type(
            origin: OriginFor<T>,
            collateral_asset_id: AssetIdOf<T>,
            info: CollateralRiskParameters,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_risk_manager(&who)?;
            // TODO
            // add collateral info if not exist
            Ok(())
        }

        #[pallet::call_index(9)]
        #[pallet::weight(10_000 + T::DbWeight::get().writes(1).ref_time())]
        pub fn change_collateral_risk_parameters(
            origin: OriginFor<T>,
            collateral_asset_id: AssetIdOf<T>,
            info: CollateralRiskParameters,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_risk_manager(&who)?;
            // TODO
            // accrue fee on all collateral asset id
            // change risk parameters if exist
            Ok(())
        }

        #[pallet::call_index(10)]
        #[pallet::weight(10_000 + T::DbWeight::get().writes(1).ref_time())]
        pub fn withdraw_profit(origin: OriginFor<T>, kusd_amount: Balance) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_protocol_owner(&who)?;
            // TODO
            // decrement protocol profit
            // transfer amount to account
            Ok(())
        }

        #[pallet::call_index(11)]
        #[pallet::weight(10_000 + T::DbWeight::get().writes(1).ref_time())]
        pub fn cover_bad_debt(origin: OriginFor<T>, kusd_amount: Balance) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_protocol_owner(&who)?;
            // TODO
            // decrement protocol bad debt
            // transfer amount from account to technical account
            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        // Ensure that `who` is a cdp owner
        // CDP owner can change balances on own CDP only.
        fn ensure_cdp_owner(who: &AccountIdOf<T>, cdp_id: U256) -> DispatchResult {
            let cdp = Self::cdp(&cdp_id).ok_or(Error::<T>::CDPNotFound)?;
            ensure!(*who == cdp.owner, Error::<T>::OperationPermitted);
            Ok(())
        }

        // Ensure that `who` is a liquidator
        // Liquidator is responsible to close unsafe CDP effectively.
        fn ensure_liquidator(who: &AccountIdOf<T>) -> DispatchResult {
            // TODO
            Ok(())
        }

        // Ensure that `who` is a risk manager
        // Risk manager can set protocol risk parameters.
        fn ensure_risk_manager(who: &AccountIdOf<T>) -> DispatchResult {
            // TODO
            Ok(())
        }

        // Ensure that `who` is a protocol owner
        // Protocol owner can withdraw profit from the protocol.
        fn ensure_protocol_owner(who: &AccountIdOf<T>) -> DispatchResult {
            // TODO
            Ok(())
        }

        /// Ensure loan-to-value ratio is `safe` and is not going to be liquidated
        fn ensure_cdp_safe(
            debt: Balance,
            collateral: Balance,
            collateral_asset_id: AssetIdOf<T>,
        ) -> DispatchResult {
            let collateral_risk_parameters = Self::collateral_risk_parameters(&collateral_asset_id)
                .ok_or(Error::<T>::CollateralInfoNotFound)?;
            // TODO get price
            // let collateral_reference_price = T::ReferencePriceProvider::get_reference_price(&collateral_asset_id)?;
            let collateral_reference_price = balance!(1000);
            let collateral_value = collateral_reference_price
                .checked_mul(collateral)
                .ok_or(Error::<T>::ArithmeticError)?;
            let debt = FixedU128::from_inner(debt);
            let max_safe_debt = collateral_risk_parameters
                .liquidation_ratio
                .checked_mul(&FixedU128::from_inner(collateral_value))
                .ok_or(Error::<T>::ArithmeticError)?;
            ensure!(debt < max_safe_debt, Error::<T>::CDPUnsafe);
            Ok(())
        }

        /// Ensures that new emission will not exceed collateral hard cap
        fn ensure_collateral_cap(
            collateral_asset_id: AssetIdOf<T>,
            new_emission: Balance,
        ) -> DispatchResult {
            let collateral_risk_parameters = Self::collateral_risk_parameters(&collateral_asset_id)
                .ok_or(Error::<T>::CollateralInfoNotFound)?;

            let current_supply_for_collateral = balance!(0);
            for cdp in <Treasury<T>>::iter_values() {
                if cdp.collateral_asset_id == collateral_asset_id {
                    current_supply_for_collateral
                        .checked_add(cdp.debt)
                        .ok_or(Error::<T>::ArithmeticError)?;
                }
            }
            ensure!(
                current_supply_for_collateral
                    .checked_add(new_emission)
                    .ok_or(Error::<T>::ArithmeticError)?
                    <= collateral_risk_parameters.max_supply,
                Error::<T>::HardCapSupply
            );
            Ok(())
        }

        /// Ensures that new emission will not exceed system KUSD hard cap
        fn ensure_protocol_cap(new_emission: Balance) -> DispatchResult {
            let current_supply = Self::kusd_supply();
            ensure!(
                current_supply
                    .checked_add(new_emission)
                    .ok_or(Error::<T>::ArithmeticError)?
                    <= Self::max_supply(),
                Error::<T>::HardCapSupply
            );
            Ok(())
        }

        // Accrue stability fee from CDP
        // Calculates fees accrued since last update using continuous compounding formula.
        // The fees is a protocol gain.
        fn accrue_internal(cdp_id: U256) -> DispatchResult {
            let cdp = Self::cdp(&cdp_id).ok_or(Error::<T>::CDPNotFound)?;

            // TODO use continuous compounding formula
            // calculate stability fee since last update in KUSD
            // calculate fee = f(debt * fee * time)
            // increase CDP debt
            // mint KUSD on tech account
            // increment profit
            // increase KUSD total supply
            // change last update time
            Ok(())
        }
    }
}
