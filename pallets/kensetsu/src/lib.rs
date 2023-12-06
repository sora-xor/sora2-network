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
use frame_support::log::{debug, warn};
pub use pallet::*;
use scale_info::TypeInfo;
use sp_arithmetic::FixedU128;
use sp_arithmetic::Perbill;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod test_utils;

mod benchmarking;
mod compounding;

pub const TECH_ACCOUNT_PREFIX: &[u8] = b"kensetsu";
pub const TECH_ACCOUNT_TREASURY_MAIN: &[u8] = b"treasury";

/// Risk management parameters for the specific collateral type.
#[derive(Debug, Clone, Encode, Decode, MaxEncodedLen, TypeInfo, PartialEq, Eq, PartialOrd, Ord)]
pub struct CollateralRiskParameters {
    /// Hard cap of total KUSD issued for the collateral.
    pub max_supply: Balance,

    /// Loan-to-value liquidation threshold
    pub liquidation_ratio: Perbill,

    /// Protocol Interest rate per second
    pub stability_fee_rate: FixedU128,
}

/// CDP - Collateralized Debt Position. It is a single collateral/debt record.
#[derive(Debug, Clone, Encode, Decode, MaxEncodedLen, TypeInfo, PartialEq, Eq, PartialOrd, Ord)]
pub struct CollateralizedDebtPosition<AccountId, AssetId, Moment> {
    /// CDP owner
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
    use common::{
        AccountIdOf, AssetInfoProvider, AssetName, AssetSymbol, BalancePrecision, ContentSource,
        Description, ReferencePriceProvider,
    };
    use frame_support::pallet_prelude::*;
    use frame_system::offchain::{SendTransactionTypes, SubmitTransaction};
    use frame_system::pallet_prelude::*;
    use pallet_timestamp as timestamp;
    use sp_arithmetic::traits::{CheckedMul, Saturating};
    use sp_arithmetic::Percent;
    use sp_core::U256;
    use sp_runtime::traits::{CheckedConversion, CheckedSub};
    use sp_std::vec::Vec;

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        /// Main off-chain worker procedure.
        ///
        /// Accrues fees and calls liquidations
        fn offchain_worker(block_number: T::BlockNumber) {
            debug!(
                "Entering off-chain worker, block number is {:?}",
                block_number
            );
            let now = Timestamp::<T>::get();
            let outdated_timestamp = now.saturating_sub(T::AccrueInterestPeriod::get());
            for (cdp_id, cdp) in <Treasury<T>>::iter() {
                if cdp.last_fee_update_time <= outdated_timestamp {
                    debug!("Accrue for CDP {:?}", cdp_id);
                    let call = Call::<T>::accrue { cdp_id };
                    if let Err(err) =
                        SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into())
                    {
                        warn!(
                            "Failed in offchain_worker send accrue(cdp_id: {:?}): {:?}",
                            cdp_id, err
                        );
                    }
                }
            }
        }
    }

    #[pallet::config]
    pub trait Config:
        assets::Config
        + frame_system::Config
        + technical::Config
        + timestamp::Config
        + SendTransactionTypes<Call<Self>>
    {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type AssetInfoProvider: AssetInfoProvider<
            Self::AssetId,
            Self::AccountId,
            AssetSymbol,
            AssetName,
            BalancePrecision,
            ContentSource,
            Description,
        >;
        type TreasuryTechAccount: Get<Self::TechAccountId>;
        type KusdAssetId: Get<Self::AssetId>;
        type ReferencePriceProvider: ReferencePriceProvider<AssetIdOf<Self>, Balance>;

        /// Accrue() for a single CDP can be called once per this period
        #[pallet::constant]
        type AccrueInterestPeriod: Get<Self::Moment>;

        /// A configuration for base priority of unsigned transactions.
        #[pallet::constant]
        type UnsignedPriority: Get<TransactionPriority>;

        /// A configuration for longevity of unsigned transactions.
        #[pallet::constant]
        type UnsignedLongevity: Get<u64>;
    }

    pub type Timestamp<T> = timestamp::Pallet<T>;

    // TODO system live parameter

    /// System bad debt, the amount of KUSD not secured with collateral.
    #[pallet::storage]
    #[pallet::getter(fn bad_debt)]
    pub type BadDebt<T> = StorageValue<_, Balance, ValueQuery>;

    /// Risk parameters for collaterals
    #[pallet::storage]
    #[pallet::getter(fn collateral_risk_parameters)]
    pub type CollateralTypes<T> = StorageMap<_, Identity, AssetIdOf<T>, CollateralRiskParameters>;

    /// Risk parameter
    /// Hard cap of KUSD may be minted by the system
    #[pallet::storage]
    #[pallet::getter(fn max_supply)]
    pub type KusdHardCap<T> = StorageValue<_, Balance, ValueQuery>;

    /// Risk parameter
    /// Liquidation penalty
    #[pallet::storage]
    #[pallet::getter(fn liquidation_penalty)]
    pub type LiquidationPenalty<T> = StorageValue<_, Percent, ValueQuery>;

    /// The next CDP id
    #[pallet::storage]
    pub type NextCDPId<T> = StorageValue<_, U256, ValueQuery>;

    /// Storage of all CDPs, where key is an unique CDP identifier
    #[pallet::storage]
    #[pallet::getter(fn cdp)]
    pub type Treasury<T: Config> = StorageMap<
        _,
        Identity,
        U256,
        CollateralizedDebtPosition<AccountIdOf<T>, AssetIdOf<T>, T::Moment>,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        CDPCreated {
            cdp_id: U256,
            owner: AccountIdOf<T>,
            collateral_asset_id: AssetIdOf<T>,
        },
        CDPClosed {
            cdp_id: U256,
            owner: AccountIdOf<T>,
            collateral_asset_id: AssetIdOf<T>,
        },
        CollateralDeposit {
            cdp_id: U256,
            owner: AccountIdOf<T>,
            collateral_asset_id: AssetIdOf<T>,
            amount: Balance,
        },
        CollateralWithdrawn {
            cdp_id: U256,
            owner: AccountIdOf<T>,
            collateral_asset_id: AssetIdOf<T>,
            amount: Balance,
        },
        DebtIncreased {
            cdp_id: U256,
            owner: AccountIdOf<T>,
            collateral_asset_id: AssetIdOf<T>,
            // KUSD amount borrowed
            amount: Balance,
        },
        DebtPayment {
            cdp_id: U256,
            owner: AccountIdOf<T>,
            collateral_asset_id: AssetIdOf<T>,
            // KUSD amount payed off
            amount: Balance,
        },
        Liquidated {
            cdp_id: U256,
            // what was liquidated
            collateral_asset_id: AssetIdOf<T>,
            collateral_amount: Balance,
            // revenue from liquidation
            kusd_amount: Balance,
        },
        CollateralRiskParametersUpdated {
            collateral_asset_id: AssetIdOf<T>,
            risk_parameters: CollateralRiskParameters,
        },
        KusdHardCapUpdated {
            hard_cap: Balance,
        },
        LiquidationPenaltyUpdated {
            liquidation_penalty: Percent,
        },
        ProfitWithdrawn {
            amount: Balance,
        },
        Donation {
            amount: Balance,
        },
    }

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub protocol_owner: Option<T::AccountId>,
        pub risk_manager: Option<T::AccountId>,
        // TODO
        // Set risk manager account
        // Set protocol owner account
        // Set liquidator account
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                protocol_owner: Default::default(),
                risk_manager: Default::default(),
                // TODO  default tech accounts
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            // TODO register tech accounts
        }
    }

    #[pallet::error]
    pub enum Error<T> {
        ArithmeticError,
        CDPNotFound,
        CollateralInfoNotFound,
        CDPSafe,
        CDPUnsafe,
        NotEnoughCollateral,
        OperationPermitted,
        OutstandingDebt,
        CDPsPerUserLimitReached,
        HardCapSupply,
        BalanceNotEnough,
        WrongCollateralAssetId,
        AccrueWrongTime,
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
                Self::deposit_event(Event::CDPCreated {
                    cdp_id: *cdp_id,
                    owner: who.clone(),
                    collateral_asset_id,
                });
                <Treasury<T>>::insert(
                    cdp_id,
                    CollateralizedDebtPosition {
                        owner: who,
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
                &T::TreasuryTechAccount::get(),
                &who,
                cdp.collateral_amount,
            )?;
            <Treasury<T>>::remove(cdp_id);
            Self::deposit_event(Event::CDPClosed {
                cdp_id,
                owner: who,
                collateral_asset_id: cdp.collateral_asset_id,
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
            let cdp = <Treasury<T>>::get(cdp_id).ok_or(Error::<T>::CDPNotFound)?;
            technical::Pallet::<T>::transfer_in(
                &cdp.collateral_asset_id,
                &who,
                &T::TreasuryTechAccount::get(),
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
            Self::deposit_event(Event::CollateralDeposit {
                cdp_id,
                owner: who,
                collateral_asset_id: cdp.collateral_asset_id,
                amount: collateral_amount,
            });

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
            Self::accrue_internal(cdp_id)?;
            ensure!(
                Self::check_cdp_is_safe(
                    cdp.debt,
                    cdp.collateral_amount
                        .checked_sub(collateral_amount)
                        .ok_or(Error::<T>::ArithmeticError)?,
                    cdp.collateral_asset_id,
                )?,
                Error::<T>::CDPUnsafe
            );
            technical::Pallet::<T>::transfer_out(
                &cdp.collateral_asset_id,
                &T::TreasuryTechAccount::get(),
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
            Self::deposit_event(Event::CollateralWithdrawn {
                cdp_id,
                owner: who,
                collateral_asset_id: cdp.collateral_asset_id,
                amount: collateral_amount,
            });

            Ok(())
        }

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
            ensure!(
                Self::check_cdp_is_safe(new_debt, cdp.collateral_amount, cdp.collateral_asset_id)?,
                Error::<T>::CDPUnsafe
            );
            Self::ensure_collateral_cap(cdp.collateral_asset_id, will_to_borrow_amount)?;
            Self::ensure_protocol_cap(will_to_borrow_amount)?;
            Self::mint_to(&who, will_to_borrow_amount)?;
            <Treasury<T>>::try_mutate(cdp_id, {
                |cdp| {
                    let cdp = cdp.as_mut().ok_or(Error::<T>::CDPNotFound)?;
                    cdp.debt = new_debt;
                    DispatchResult::Ok(())
                }
            })?;
            Self::deposit_event(Event::DebtIncreased {
                cdp_id,
                owner: who,
                collateral_asset_id: cdp.collateral_asset_id,
                amount: will_to_borrow_amount,
            });

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
            Self::burn_from(&who, to_cover_debt)?;
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
            Self::deposit_event(Event::DebtPayment {
                cdp_id,
                owner: who,
                collateral_asset_id: cdp.collateral_asset_id,
                amount: to_cover_debt,
            });

            Ok(())
        }

        /// Liquidates part of unsafe CDP
        #[pallet::call_index(6)]
        #[pallet::weight(10_000 + T::DbWeight::get().writes(1).ref_time())]
        pub fn liquidate(
            origin: OriginFor<T>,
            cdp_id: U256,
            collateral_amount: Balance,
            kusd_amount: Balance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_liquidator(&who)?;
            Self::accrue_internal(cdp_id)?;
            let cdp = Self::cdp(cdp_id).ok_or(Error::<T>::CDPNotFound)?;
            let cdp_debt = cdp.debt;
            let cdp_collateral_amount = cdp.collateral_amount;
            ensure!(
                !Self::check_cdp_is_safe(cdp_debt, cdp_collateral_amount, cdp.collateral_asset_id)?,
                Error::<T>::CDPSafe
            );
            technical::Pallet::<T>::transfer_out(
                &cdp.collateral_asset_id,
                &T::TreasuryTechAccount::get(),
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
            let to_burn = if cdp_debt >= kusd_amount {
                let shortage = cdp_debt
                    .checked_sub(kusd_amount)
                    .ok_or(Error::<T>::CDPNotFound)?;
                if cdp_collateral_amount <= collateral_amount {
                    // no collateral, total default
                    // CDP debt is not covered with liquidation, now it is a protocol bad debt
                    Self::cover_with_protocol(shortage)?;
                    // close empty CDP, debt == 0, collateral == 0
                    <Treasury<T>>::remove(cdp_id);
                    Self::deposit_event(Event::CDPClosed {
                        cdp_id,
                        owner: cdp.owner,
                        collateral_asset_id: cdp.collateral_asset_id,
                    });
                } else {
                    // partly covered
                    <Treasury<T>>::try_mutate(cdp_id, {
                        |cdp| {
                            let cdp = cdp.as_mut().ok_or(Error::<T>::CDPNotFound)?;
                            cdp.debt = cdp
                                .debt
                                .checked_sub(kusd_amount)
                                .ok_or(Error::<T>::CDPNotFound)?;
                            DispatchResult::Ok(())
                        }
                    })?;
                }

                kusd_amount
            } else {
                // CDP debt is covered
                let leftover = kusd_amount
                    .checked_sub(cdp_debt)
                    .ok_or(Error::<T>::CDPNotFound)?;
                let liquidation_penalty = Self::liquidation_penalty() * cdp_debt;
                <Treasury<T>>::try_mutate(cdp_id, {
                    |cdp| {
                        let cdp = cdp.as_mut().ok_or(Error::<T>::CDPNotFound)?;
                        cdp.debt = 0;
                        DispatchResult::Ok(())
                    }
                })?;
                let penalty = if liquidation_penalty >= leftover {
                    // not enough KUSD for full penalty
                    leftover
                } else {
                    // There is more KUSD than to cover debt and penalty, leftover goes to cdp.owner
                    assets::Pallet::<T>::transfer_from(
                        &T::KusdAssetId::get(),
                        &who,
                        &cdp.owner,
                        leftover
                            .checked_sub(liquidation_penalty)
                            .ok_or(Error::<T>::CDPNotFound)?,
                    )?;
                    liquidation_penalty
                };
                Self::cover_bad_debt(&who, penalty)?;

                cdp_debt
            };
            Self::burn_from(&who, to_burn)?;
            Self::deposit_event(Event::Liquidated {
                cdp_id,
                collateral_asset_id: cdp.collateral_asset_id,
                collateral_amount,
                kusd_amount,
            });

            Ok(())
        }

        /// Updates cdp debt with interest
        /// Unsigned call possible
        #[pallet::call_index(7)]
        #[pallet::weight(10_000 + T::DbWeight::get().writes(1).ref_time())]
        pub fn accrue(_origin: OriginFor<T>, cdp_id: U256) -> DispatchResult {
            ensure!(
                Self::is_it_time_to_accrue(&cdp_id)?,
                Error::<T>::AccrueWrongTime
            );
            Self::accrue_internal(cdp_id)?;
            Ok(())
        }

        /// Updates collateral risk parameters
        /// Is set by risk management
        #[pallet::call_index(8)]
        #[pallet::weight(10_000 + T::DbWeight::get().writes(1).ref_time())]
        pub fn update_collateral_risk_parameters(
            origin: OriginFor<T>,
            collateral_asset_id: AssetIdOf<T>,
            info: CollateralRiskParameters,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_risk_manager(&who)?;
            ensure!(
                T::AssetInfoProvider::asset_exists(&T::KusdAssetId::get()),
                Error::<T>::CollateralInfoNotFound
            );
            for (cdp_id, cdp) in <Treasury<T>>::iter() {
                if cdp.collateral_asset_id == collateral_asset_id {
                    Self::accrue_internal(cdp_id)?;
                }
            }
            <CollateralTypes<T>>::insert(collateral_asset_id, info.clone());
            Self::deposit_event(Event::CollateralRiskParametersUpdated {
                collateral_asset_id,
                risk_parameters: info,
            });

            Ok(())
        }

        /// Sets max hard cap supply of KUSD
        /// Is set by risk management
        #[pallet::call_index(9)]
        #[pallet::weight(10_000 + T::DbWeight::get().writes(1).ref_time())]
        pub fn update_hard_cap_total_supply(
            origin: OriginFor<T>,
            new_hard_cap: Balance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_risk_manager(&who)?;
            <KusdHardCap<T>>::mutate({
                |hard_cap| {
                    *hard_cap = new_hard_cap;
                }
            });
            Self::deposit_event(Event::KusdHardCapUpdated {
                hard_cap: new_hard_cap,
            });
            Ok(())
        }

        /// Sets liquidation penalty
        /// Is set by risk management
        #[pallet::call_index(10)]
        #[pallet::weight(10_000 + T::DbWeight::get().writes(1).ref_time())]
        pub fn update_liquidation_penalty(
            origin: OriginFor<T>,
            new_liquidation_penalty: Percent,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_risk_manager(&who)?;
            <LiquidationPenalty<T>>::mutate({
                |liquidation_penalty| {
                    *liquidation_penalty = new_liquidation_penalty;
                }
            });
            Self::deposit_event(Event::LiquidationPenaltyUpdated {
                liquidation_penalty: new_liquidation_penalty,
            });

            Ok(())
        }

        /// Withdraws profit from protocol treasury
        /// Is called by protocol owner
        #[pallet::call_index(11)]
        #[pallet::weight(10_000 + T::DbWeight::get().writes(1).ref_time())]
        pub fn withdraw_profit(origin: OriginFor<T>, kusd_amount: Balance) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_protocol_owner(&who)?;
            technical::Pallet::<T>::transfer_out(
                &T::KusdAssetId::get(),
                &T::TreasuryTechAccount::get(),
                &who,
                kusd_amount,
            )?;
            Self::deposit_event(Event::ProfitWithdrawn {
                amount: kusd_amount,
            });

            Ok(())
        }

        /// Donate KUSD to the protocol to cover bad debt or increase protocol profit
        #[pallet::call_index(12)]
        #[pallet::weight(10_000 + T::DbWeight::get().writes(1).ref_time())]
        pub fn donate(origin: OriginFor<T>, kusd_amount: Balance) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::cover_bad_debt(&who, kusd_amount)?;
            Self::deposit_event(Event::Donation {
                amount: kusd_amount,
            });

            Ok(())
        }
    }

    /// Validate unsigned call to this pallet.
    #[pallet::validate_unsigned]
    impl<T: Config> ValidateUnsigned for Pallet<T> {
        type Call = Call<T>;

        /// It is allowed to call only accrue() and liquidate() and only if
        /// it fulfills conditions.
        fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
            match call {
                Call::accrue { cdp_id } => {
                    if Self::is_it_time_to_accrue(cdp_id).map_err(|_| {
                        // TODO custom error
                        InvalidTransaction::Custom(1u8)
                    })? {
                        ValidTransaction::with_tag_prefix("Kensetsu::accrue")
                            .priority(T::UnsignedPriority::get())
                            .longevity(T::UnsignedLongevity::get())
                            .and_provides([&cdp_id])
                            .propagate(true)
                            .build()
                    } else {
                        InvalidTransaction::Future.into()
                    }
                }
                // TODO add liquidate
                _ => {
                    warn!("Unknown unsigned call {:?}", call);
                    InvalidTransaction::Call.into()
                }
            }
        }
    }

    impl<T: Config> Pallet<T> {
        /// Ensures that `who` is a cdp owner
        /// CDP owner can change balances on own CDP only.
        fn ensure_cdp_owner(who: &AccountIdOf<T>, cdp_id: U256) -> DispatchResult {
            let cdp = Self::cdp(cdp_id).ok_or(Error::<T>::CDPNotFound)?;
            ensure!(*who == cdp.owner, Error::<T>::OperationPermitted);
            Ok(())
        }

        /// Ensures that `who` is a liquidator
        /// Liquidator is responsible to close unsafe CDP effectively.
        fn ensure_liquidator(_who: &AccountIdOf<T>) -> DispatchResult {
            // TODO
            Ok(())
        }

        /// Ensures that `who` is a risk manager
        /// Risk manager can set protocol risk parameters.
        fn ensure_risk_manager(_who: &AccountIdOf<T>) -> DispatchResult {
            // TODO
            Ok(())
        }

        /// Ensures that `who` is a protocol owner
        /// Protocol owner can withdraw profit from the protocol.
        fn ensure_protocol_owner(_who: &AccountIdOf<T>) -> DispatchResult {
            // TODO
            Ok(())
        }

        /// Checks loan-to-value ratio is `safe` and is not going to be liquidated
        /// Returns true if CDP is safe, LTV <= liquidation threshold
        pub(crate) fn check_cdp_is_safe(
            debt: Balance,
            collateral: Balance,
            collateral_asset_id: AssetIdOf<T>,
        ) -> Result<bool, DispatchError> {
            let collateral_risk_parameters = Self::collateral_risk_parameters(collateral_asset_id)
                .ok_or(Error::<T>::CollateralInfoNotFound)?;
            let collateral_reference_price =
                T::ReferencePriceProvider::get_reference_price(&collateral_asset_id)?;
            let collateral_reference_price = FixedU128::from_inner(collateral_reference_price);
            let collateral_value = collateral_reference_price
                .checked_mul(&FixedU128::from_inner(collateral))
                .ok_or(Error::<T>::ArithmeticError)?;
            let debt = FixedU128::from_inner(debt);
            let max_safe_debt =
                FixedU128::from_perbill(collateral_risk_parameters.liquidation_ratio)
                    .checked_mul(&collateral_value)
                    .ok_or(Error::<T>::ArithmeticError)?;
            Ok(debt <= max_safe_debt)
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
            let current_supply = T::AssetInfoProvider::total_issuance(&T::KusdAssetId::get())?;
            ensure!(
                current_supply
                    .checked_add(new_emission)
                    .ok_or(Error::<T>::ArithmeticError)?
                    <= Self::max_supply(),
                Error::<T>::HardCapSupply
            );
            Ok(())
        }

        /// Recalculates bad debt with `kusd_amount` profit, leftover goes to protocol profit
        fn cover_bad_debt(from: &AccountIdOf<T>, kusd_amount: Balance) -> DispatchResult {
            let bad_debt = <BadDebt<T>>::get();
            let to_cover_debt = if kusd_amount < bad_debt {
                kusd_amount
            } else {
                technical::Pallet::<T>::transfer_in(
                    &T::KusdAssetId::get(),
                    from,
                    &T::TreasuryTechAccount::get(),
                    kusd_amount
                        .checked_sub(bad_debt)
                        .ok_or(Error::<T>::ArithmeticError)?,
                )?;
                bad_debt
            };
            Self::burn_from(from, to_cover_debt)?;
            <BadDebt<T>>::try_mutate(|bad_debt| {
                *bad_debt = bad_debt
                    .checked_sub(to_cover_debt)
                    .ok_or(Error::<T>::ArithmeticError)?;
                DispatchResult::Ok(())
            })?;

            Ok(())
        }

        fn is_it_time_to_accrue(cdp_id: &U256) -> Result<bool, DispatchError> {
            let cdp = Self::cdp(cdp_id).ok_or(Error::<T>::CDPNotFound)?;
            ensure!(cdp.debt > 0, Error::<T>::CDPSafe);
            let now = Timestamp::<T>::get();
            let outdated_timestamp = now.saturating_sub(T::AccrueInterestPeriod::get());
            Ok(cdp.debt > 0 && cdp.last_fee_update_time <= outdated_timestamp)
        }

        /// Accrue stability fee from CDP
        /// Calculates fees accrued since last update using continuous compounding formula.
        /// The fees is a protocol gain.
        fn accrue_internal(cdp_id: U256) -> DispatchResult {
            let cdp = Self::cdp(cdp_id).ok_or(Error::<T>::CDPNotFound)?;
            let now = Timestamp::<T>::get();
            ensure!(now >= cdp.last_fee_update_time, Error::<T>::AccrueWrongTime);
            let time_passed = now
                .checked_sub(&cdp.last_fee_update_time)
                .ok_or(Error::<T>::ArithmeticError)?;
            let collateral_info = Self::collateral_risk_parameters(cdp.collateral_asset_id)
                .ok_or(Error::<T>::CollateralInfoNotFound)?;
            let stability_fee = get_accrued_interest(
                cdp.debt,
                collateral_info.stability_fee_rate,
                time_passed
                    .checked_into::<u64>()
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
            Self::mint_treasury(stability_fee)?;

            Ok(())
        }

        /// Mint token to protocol technical account
        fn mint_treasury(amount: Balance) -> DispatchResult {
            technical::Pallet::<T>::mint(
                &T::KusdAssetId::get(),
                &T::TreasuryTechAccount::get(),
                amount,
            )?;
            Ok(())
        }

        /// Mint token to AccountId
        fn mint_to(account: &AccountIdOf<T>, amount: Balance) -> DispatchResult {
            let technical_account_id = technical::Pallet::<T>::tech_account_id_to_account_id(
                &T::TreasuryTechAccount::get(),
            )?;
            assets::Pallet::<T>::mint_to(
                &T::KusdAssetId::get(),
                &technical_account_id,
                account,
                amount,
            )?;
            Ok(())
        }

        /// Burns tokens from treasury technical account
        fn burn_treasury(to_burn: Balance) -> DispatchResult {
            let technical_account_id = technical::Pallet::<T>::tech_account_id_to_account_id(
                &T::TreasuryTechAccount::get(),
            )?;
            assets::Pallet::<T>::burn_from(
                &T::KusdAssetId::get(),
                &technical_account_id,
                &technical_account_id,
                to_burn,
            )?;
            Ok(())
        }

        /// Burns tokens from AccountId
        fn burn_from(account: &AccountIdOf<T>, amount: Balance) -> DispatchResult {
            let technical_account_id = technical::Pallet::<T>::tech_account_id_to_account_id(
                &T::TreasuryTechAccount::get(),
            )?;
            assets::Pallet::<T>::burn_from(
                &T::KusdAssetId::get(),
                &technical_account_id,
                account,
                amount,
            )?;
            Ok(())
        }

        /// Cover CDP debt with protocol balance
        /// If protocol balance is less thatn amount to cover, it is a bad debt
        fn cover_with_protocol(amount: Balance) -> DispatchResult {
            let treasury_account_id = technical::Pallet::<T>::tech_account_id_to_account_id(
                &T::TreasuryTechAccount::get(),
            )?;
            let protocol_positive_balance =
                T::AssetInfoProvider::free_balance(&T::KusdAssetId::get(), &treasury_account_id)?;
            let to_burn = if amount < protocol_positive_balance {
                amount
            } else {
                <BadDebt<T>>::try_mutate(|bad_debt| {
                    *bad_debt = bad_debt
                        .checked_add(
                            amount
                                .checked_sub(protocol_positive_balance)
                                .ok_or(Error::<T>::ArithmeticError)?,
                        )
                        .ok_or(Error::<T>::ArithmeticError)?;
                    DispatchResult::Ok(())
                })?;
                protocol_positive_balance
            };
            Self::burn_treasury(to_burn)?;

            Ok(())
        }

        /// Returns CDP ids where the account id is owner
        pub fn get_account_cdp_ids(
            account_id: &AccountIdOf<T>,
        ) -> Result<Vec<U256>, DispatchError> {
            Ok(<Treasury<T>>::iter()
                .filter(|(_, cdp)| cdp.owner == *account_id)
                .map(|(cdp_id, _)| cdp_id)
                .collect())
        }
    }
}
