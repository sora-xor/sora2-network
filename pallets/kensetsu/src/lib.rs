// This file is part of the SORA network and Polkaswap app.

// Copyright (c) 2020, 2021, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

// Redistribution and use in source and binary forms, with or without modification,
// are permitted provided that the following conditions are met:

// Redistributions of source code must retain the above copyright notice, this list
// of conditions and the following disclaimer.
// Redistributions in binary form must reproduce the above copyright notice, this
// list of conditions and the following disclaimer in the documentation and/or other
// materials provided with the distribution.
//
// All advertising materials mentioning features or use of this software must display
// the following acknowledgement: This product includes software developed by Polka Biome
// Ltd., SORA, and Polkaswap.
//
// Neither the name of the Polka Biome Ltd. nor the names of its contributors may be used
// to endorse or promote products derived from this software without specific prior written permission.

// THIS SOFTWARE IS PROVIDED BY Polka Biome Ltd. AS IS AND ANY EXPRESS OR IMPLIED WARRANTIES,
// INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL Polka Biome Ltd. BE LIABLE FOR ANY
// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
// BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS;
// OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
// USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

#![cfg_attr(not(feature = "std"), no_std)]

use assets::AssetIdOf;
use codec::{Decode, Encode, MaxEncodedLen};
use common::{balance, Balance};
use compounding::get_accrued_interest;
pub use pallet::*;
use scale_info::TypeInfo;
use sp_arithmetic::traits::{EnsureDiv, EnsureMul, Saturating};
use sp_arithmetic::FixedU128;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
mod compounding;

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

    /// the last timestamp when stability fee was accrued
    pub last_fee_update_time: Moment,
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use common::{AccountIdOf, ReferencePriceProvider};
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::{BlockNumberFor, *};
    use pallet_timestamp as timestamp;
    use sp_arithmetic::traits::CheckedMul;
    use sp_core::U256;
    use sp_runtime::traits::CheckedConversion;
    use traits::MultiCurrency;

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::config]
    pub trait Config:
        assets::Config + frame_system::Config + technical::Config + timestamp::Config
    {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type TreasuryTechAccountId: Get<Self::TechAccountId>;

        // TODO
        // type ReferencePriceProvider: ReferencePriceProvider<AccountIdOf<Self>, AssetIdOf<Self>>;
        // type Currency: MultiCurrency<AccountIdOf<Self>, Balance = Balance>;

        // TODO add KUSD AssetId
        // type ReservesAccount: Get<AccountIdOf<Self>>;
        // type KUSDAssetId: Get<AssetIdOf<T>>;

        // TODO fee scheduler
        // type FeeScheduleMaxPerBlock: Get<u32>;

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

    // TODO fees offchain worker scheduler
    // #[pallet::storage]
    // #[pallet::getter(fn fee_schedule)]
    // pub type StabilityFeeSchedule<T: Config> = StorageMap<
    //     _,
    //     Identity,
    //     BlockNumberFor<T>,
    //     BoundedVec<H256, T::FeeScheduleMaxPerBlock>,
    //     ValueQuery,
    // >;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
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
        BalanceNotEnough,
        WrongCollateralAssetId,
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
            ensure!(
                <CollateralTypes<T>>::contains_key(collateral_asset_id),
                Error::<T>::CollateralInfoNotFound
            );
            NextCDPId::<T>::try_mutate(|cdp_id| {
                *cdp_id = cdp_id
                    .checked_add(U256::from(1))
                    .ok_or(Error::<T>::ArithmeticError)?;
                <Treasury<T>>::insert(
                    cdp_id,
                    CollateralizedDebtPosition {
                        owner: who.clone(),
                        collateral_asset_id,
                        collateral_amount: balance!(0),
                        debt: balance!(0),
                        last_fee_update_time: Timestamp::<T>::get(),
                    },
                );
                DispatchResult::Ok(())
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
            technical::Pallet::<T>::transfer_out(
                &cdp.collateral_asset_id,
                &T::TreasuryTechAccountId::get(),
                &who,
                cdp.collateral_amount,
            )?;
            <Treasury<T>>::remove(cdp_id);

            Ok(())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(10_000 + T::DbWeight::get().writes(1).ref_time())]
        pub fn deposit_collateral(
            origin: OriginFor<T>,
            cdp_id: U256,
            collateral_asset_id: AssetIdOf<T>,
            collateral_amount: Balance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let cdp = <Treasury<T>>::get(cdp_id).ok_or(Error::<T>::CDPNotFound)?;
            ensure!(
                collateral_asset_id == cdp.collateral_asset_id,
                Error::<T>::WrongCollateralAssetId
            );
            technical::Pallet::<T>::transfer_in(
                &collateral_asset_id,
                &who,
                &T::TreasuryTechAccountId::get(),
                collateral_amount,
            )?;
            <Treasury<T>>::try_mutate(cdp_id, {
                |cdp| {
                    let cdp = cdp.as_mut().ok_or(Error::<T>::CDPNotFound)?;
                    cdp.collateral_amount = cdp
                        .collateral_amount
                        .checked_add(collateral_amount)
                        .ok_or(Error::<T>::ArithmeticError)?;
                    DispatchResult::Ok(())
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
                cdp.debt,
                cdp.collateral_amount
                    .checked_sub(collateral_amount)
                    .ok_or(Error::<T>::ArithmeticError)?,
                cdp.collateral_asset_id,
            )?;
            technical::Pallet::<T>::transfer_out(
                &cdp.collateral_asset_id,
                &T::TreasuryTechAccountId::get(),
                &who,
                collateral_amount,
            )?;
            <Treasury<T>>::try_mutate(cdp_id, {
                |cdp| {
                    let cdp = cdp.as_mut().ok_or(Error::<T>::CDPNotFound)?;
                    cdp.collateral_amount = cdp
                        .collateral_amount
                        .checked_sub(collateral_amount)
                        .ok_or(Error::<T>::ArithmeticError)?;
                    DispatchResult::Ok(())
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
            let new_debt = cdp
                .debt
                .checked_add(will_to_borrow_amount)
                .ok_or(Error::<T>::ArithmeticError)?;
            Self::ensure_cdp_safe(new_debt, cdp.collateral_amount, cdp.collateral_asset_id)?;
            Self::ensure_collateral_cap(cdp.collateral_asset_id, will_to_borrow_amount)?;
            Self::ensure_protocol_cap(will_to_borrow_amount)?;

            // TODO
            // mint to_mint KUSD amount to who
            // transfer to_transfer KUSD amount to who

            <Treasury<T>>::try_mutate(cdp_id, {
                |cdp| {
                    let cdp = cdp.as_mut().ok_or(Error::<T>::CDPNotFound)?;
                    cdp.debt = cdp
                        .debt
                        .checked_add(will_to_borrow_amount)
                        .ok_or(Error::<T>::ArithmeticError)?;
                    DispatchResult::Ok(())
                }
            })?;
            <Supply<T>>::try_mutate({
                |supply| {
                    *supply = supply
                        .checked_add(will_to_borrow_amount)
                        .ok_or(Error::<T>::ArithmeticError)?;
                    DispatchResult::Ok(())
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

            // if repaying amount exceeds debt, leftover goes to the caller
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
            // TODO return leftover to who

            <Treasury<T>>::try_mutate(cdp_id, {
                |cdp| {
                    let cdp = cdp.as_mut().ok_or(Error::<T>::CDPNotFound)?;
                    cdp.debt = cdp
                        .debt
                        .checked_sub(to_cover_debt)
                        .ok_or(Error::<T>::ArithmeticError)?;
                    DispatchResult::Ok(())
                }
            })?;
            <Supply<T>>::try_mutate({
                |supply| {
                    *supply = supply
                        .checked_sub(to_cover_debt)
                        .ok_or(Error::<T>::ArithmeticError)?;
                    DispatchResult::Ok(())
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
        pub fn update_collateral_risk_parameters(
            origin: OriginFor<T>,
            collateral_asset_id: AssetIdOf<T>,
            info: CollateralRiskParameters,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_risk_manager(&who)?;

            for (cdp_id, cdp) in <Treasury<T>>::iter() {
                if cdp.collateral_asset_id == collateral_asset_id {
                    Self::accrue_internal(cdp_id)?;
                }
            }
            <CollateralTypes<T>>::insert(collateral_asset_id, info);

            Ok(())
        }

        /// Sets max hard cap supply of KUSD
        #[pallet::call_index(9)]
        #[pallet::weight(10_000 + T::DbWeight::get().writes(1).ref_time())]
        pub fn update_hard_cap_total_supply(
            origin: OriginFor<T>,
            new_hard_cap: Balance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_risk_manager(&who)?;
            <MaxSupply<T>>::mutate({
                |hard_cap| {
                    *hard_cap = new_hard_cap;
                }
            });

            Ok(())
        }

        /// Sets liquidation penalty
        #[pallet::call_index(10)]
        #[pallet::weight(10_000 + T::DbWeight::get().writes(1).ref_time())]
        pub fn update_liquidation_penalty(
            origin: OriginFor<T>,
            new_liquidation_penalty: FixedU128,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_risk_manager(&who)?;
            <LiquidationPenalty<T>>::mutate({
                |liquidation_penalty| {
                    *liquidation_penalty = new_liquidation_penalty;
                }
            });

            Ok(())
        }

        #[pallet::call_index(11)]
        #[pallet::weight(10_000 + T::DbWeight::get().writes(1).ref_time())]
        pub fn withdraw_profit(origin: OriginFor<T>, kusd_amount: Balance) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_protocol_owner(&who)?;
            let profit = <Profit<T>>::get();
            ensure!(kusd_amount <= profit, Error::<T>::BalanceNotEnough);

            <Profit<T>>::try_mutate(|profit| {
                *profit = profit
                    .checked_sub(kusd_amount)
                    .ok_or(Error::<T>::ArithmeticError)?;
                DispatchResult::Ok(())
            })?;

            // TODO
            // technical::Pallet::<T>::transfer_out(
            //     &T::KUSDAssetId::get(),
            //     &T::TreasuryTechAccountId::get(),
            //     &who,
            //     kusd_amount,
            // )?;

            Ok(())
        }

        /// Donate KUSD to the protocol
        #[pallet::call_index(12)]
        #[pallet::weight(10_000 + T::DbWeight::get().writes(1).ref_time())]
        pub fn cover_bad_debt(origin: OriginFor<T>, kusd_amount: Balance) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let bad_debt = <BadDebt<T>>::get();
            let to_cover_debt = if kusd_amount < bad_debt {
                kusd_amount
            } else {
                bad_debt
            };
            let leftover = kusd_amount
                .checked_sub(bad_debt)
                .ok_or(Error::<T>::ArithmeticError)?;

            <BadDebt<T>>::try_mutate(|bad_debt| {
                *bad_debt = bad_debt
                    .checked_sub(to_cover_debt)
                    .ok_or(Error::<T>::ArithmeticError)?;
                DispatchResult::Ok(())
            })?;
            <Profit<T>>::try_mutate(|profit| {
                *profit = profit
                    .checked_add(leftover)
                    .ok_or(Error::<T>::ArithmeticError)?;
                DispatchResult::Ok(())
            })?;

            // TODO
            // technical::Pallet::<T>::transfer_in(
            //     &T::KUSDAssetId::get(),),
            //     &who,
            //     &T::TreasuryTechAccountId::get(
            //     kusd_amount,
            // )?;

            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        // Ensure that `who` is a cdp owner
        // CDP owner can change balances on own CDP only.
        fn ensure_cdp_owner(who: &AccountIdOf<T>, cdp_id: U256) -> DispatchResult {
            let cdp = Self::cdp(cdp_id).ok_or(Error::<T>::CDPNotFound)?;
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
            let collateral_risk_parameters = Self::collateral_risk_parameters(collateral_asset_id)
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
            ensure!(debt <= max_safe_debt, Error::<T>::CDPUnsafe);
            Ok(())
        }

        /// Ensures that new emission will not exceed collateral hard cap
        fn ensure_collateral_cap(
            collateral_asset_id: AssetIdOf<T>,
            new_emission: Balance,
        ) -> DispatchResult {
            let collateral_risk_parameters = Self::collateral_risk_parameters(collateral_asset_id)
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
            let cdp = Self::cdp(cdp_id).ok_or(Error::<T>::CDPNotFound)?;
            let collateral_info = Self::collateral_risk_parameters(cdp.collateral_asset_id)
                .ok_or(Error::<T>::CollateralInfoNotFound)?;
            let now = Timestamp::<T>::get();
            let stability_fee = get_accrued_interest(
                cdp.debt,
                collateral_info.stability_fee_rate,
                now.checked_into::<u64>()
                    .ok_or(Error::<T>::ArithmeticError)?,
            )
            .map_err(|_| Error::<T>::ArithmeticError)?;
            <Treasury<T>>::try_mutate(cdp_id, {
                |cdp| {
                    let cdp = cdp.as_mut().ok_or(Error::<T>::CDPNotFound)?;
                    cdp.debt = cdp
                        .debt
                        .checked_add(stability_fee)
                        .ok_or(Error::<T>::ArithmeticError)?;
                    cdp.last_fee_update_time = now;
                    DispatchResult::Ok(())
                }
            })?;

            // TODO
            // mint stability_fee KUSD on tech account

            <Profit<T>>::try_mutate(|profit| {
                *profit = profit
                    .checked_add(stability_fee)
                    .ok_or(Error::<T>::ArithmeticError)?;
                DispatchResult::Ok(())
            })?;
            <Supply<T>>::try_mutate({
                |supply| {
                    *supply = supply
                        .checked_add(stability_fee)
                        .ok_or(Error::<T>::ArithmeticError)?;
                    DispatchResult::Ok(())
                }
            })?;

            Ok(())
        }
    }
}
