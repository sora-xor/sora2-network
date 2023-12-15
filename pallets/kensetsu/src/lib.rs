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

mod compounding;
pub mod weights;

pub const TECH_ACCOUNT_PREFIX: &[u8] = b"kensetsu";
pub const TECH_ACCOUNT_TREASURY_MAIN: &[u8] = b"treasury";

/// Custom errors for unsigned tx validation, InvalidTransaction::Custom(u8)
const VALIDATION_ERROR_ACCRUE: u8 = 1;
const VALIDATION_ERROR_ACCRUE_NO_DEBT: u8 = 2;
const VALIDATION_ERROR_CHECK_SAFE: u8 = 3;
const VALIDATION_ERROR_CDP_SAFE: u8 = 4;

/// Risk management parameters for the specific collateral type.
#[derive(
    Debug, Clone, Encode, Decode, MaxEncodedLen, TypeInfo, PartialEq, Eq, PartialOrd, Ord, Copy,
)]
pub struct CollateralRiskParameters {
    /// Hard cap of total KUSD issued for the collateral.
    pub hard_cap: Balance,

    /// Loan-to-value liquidation threshold
    pub liquidation_ratio: Perbill,

    /// The max amount of collateral can be liquidated in one round
    pub max_liquidation_lot: Balance,

    /// Protocol Interest rate per second
    pub stability_fee_rate: FixedU128,
}

/// Collateral parameters, includes risk info and additional data for interest rate calculation
#[derive(Debug, Clone, Encode, Decode, MaxEncodedLen, TypeInfo, PartialEq, Eq, PartialOrd, Ord)]
pub struct CollateralInfo<Moment> {
    /// Collateral Risk parameters set by risk management
    pub risk_parameters: CollateralRiskParameters,

    /// the last timestamp when stability fee was accrued
    pub last_fee_update_time: Moment,

    /// Interest accrued for collateral for all time
    pub interest_coefficient: FixedU128,
}

/// CDP - Collateralized Debt Position. It is a single collateral/debt record.
#[derive(Debug, Clone, Encode, Decode, MaxEncodedLen, TypeInfo, PartialEq, Eq, PartialOrd, Ord)]
pub struct CollateralizedDebtPosition<AccountId, AssetId> {
    /// CDP owner
    pub owner: AccountId,

    /// Collateral
    pub collateral_asset_id: AssetId,
    pub collateral_amount: Balance,

    /// normalized outstanding debt in KUSD
    pub debt: Balance,

    /// Interest accrued for CDP.
    /// Initializes on creation with collateral interest coefficient equal to 1. The coefficient is
    /// growing over time with interest rate. Actual interest is:
    /// (collateral.coefficient - cdp.coefficient) / cdp.coefficient
    pub interest_coefficient: FixedU128,
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use crate::compounding::compound;
    use crate::weights::WeightInfo;
    use common::prelude::{QuoteAmount, SwapAmount, SwapOutcome};
    use common::{
        AccountIdOf, AssetInfoProvider, AssetName, AssetSymbol, BalancePrecision, ContentSource,
        DEXId, Description, LiquidityProxyTrait, LiquiditySourceFilter, ReferencePriceProvider,
    };
    use frame_support::pallet_prelude::*;
    use frame_system::offchain::{SendTransactionTypes, SubmitTransaction};
    use frame_system::pallet_prelude::*;
    use pallet_timestamp as timestamp;
    use sp_arithmetic::traits::{CheckedMul, Saturating};
    use sp_arithmetic::Percent;
    use sp_core::U256;
    use sp_runtime::traits::{CheckedConversion, CheckedDiv, CheckedSub, One};
    use sp_std::collections::btree_set::BTreeSet;
    use sp_std::vec::Vec;

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::without_storage_info]
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
            // TODO implement better solution, with offchain storage
            let now = Timestamp::<T>::get();
            let outdated_timestamp = now.saturating_sub(T::AccrueInterestPeriod::get());
            let mut collaterals_to_update = BTreeSet::new();
            for (collateral_asset_id, collateral_info) in <CollateralInfos<T>>::iter() {
                if collateral_info.last_fee_update_time <= outdated_timestamp {
                    collaterals_to_update.insert(collateral_asset_id);
                }
            }
            for (cdp_id, cdp) in <CDPDepository<T>>::iter() {
                // tODO or debt
                if collaterals_to_update.contains(&cdp.collateral_asset_id) {
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
                match Self::check_cdp_is_safe(
                    cdp.debt,
                    cdp.collateral_amount,
                    cdp.collateral_asset_id,
                ) {
                    Ok(cdp_is_safe) => {
                        if !cdp_is_safe {
                            let call = Call::<T>::liquidate { cdp_id };
                            if let Err(err) =
                                SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(
                                    call.into(),
                                )
                            {
                                warn!(
                                    "Failed in offchain_worker send liquidate(cdp_id: {:?}): {:?}",
                                    cdp_id, err
                                );
                            }
                        }
                    }
                    Err(err) => {
                        warn!(
                            "Failed in offchain_worker check cdp {:?} safety: {:?}",
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
        type LiquidityProxy: LiquidityProxyTrait<Self::DEXId, Self::AccountId, Self::AssetId>;

        /// Accrue() for a single CDP can be called once per this period
        #[pallet::constant]
        type AccrueInterestPeriod: Get<Self::Moment>;

        /// A configuration for base priority of unsigned transactions.
        #[pallet::constant]
        type UnsignedPriority: Get<TransactionPriority>;

        /// A configuration for longevity of unsigned transactions.
        #[pallet::constant]
        type UnsignedLongevity: Get<u64>;

        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    pub type Timestamp<T> = timestamp::Pallet<T>;

    // TODO system live parameter

    /// System bad debt, the amount of KUSD not secured with collateral.
    #[pallet::storage]
    #[pallet::getter(fn bad_debt)]
    pub type BadDebt<T> = StorageValue<_, Balance, ValueQuery>;

    /// Risk parameters for collaterals
    #[pallet::storage]
    #[pallet::getter(fn collateral_infos)]
    pub type CollateralInfos<T: Config> =
        StorageMap<_, Identity, AssetIdOf<T>, CollateralInfo<T::Moment>>;

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

    /// CDP counter used for CDP id
    #[pallet::storage]
    pub type NextCDPId<T> = StorageValue<_, U256, ValueQuery>;

    /// Storage of all CDPs, where key is an unique CDP identifier
    #[pallet::storage]
    #[pallet::getter(fn cdp)]
    pub type CDPDepository<T: Config> =
        StorageMap<_, Identity, U256, CollateralizedDebtPosition<AccountIdOf<T>, AssetIdOf<T>>>;

    #[pallet::storage]
    #[pallet::getter(fn risk_managers)]
    pub type RiskManagers<T: Config> = StorageValue<_, BTreeSet<T::AccountId>>;

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
            // revenue from liquidation to cover debt
            kusd_amount: Balance,
            // liquidation penalty
            penalty: Balance,
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

    #[pallet::error]
    pub enum Error<T> {
        ArithmeticError,
        WrongAssetId,
        CDPNotFound,
        CollateralInfoNotFound,
        CDPSafe,
        CDPUnsafe,
        NotEnoughCollateral,
        OperationNotPermitted,
        OutstandingDebt,
        NoDebt,
        CDPsPerUserLimitReached,
        HardCapSupply,
        BalanceNotEnough,
        WrongCollateralAssetId,
        AccrueWrongTime,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::create_cdp())]
        pub fn create_cdp(
            origin: OriginFor<T>,
            collateral_asset_id: AssetIdOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(
                <CollateralInfos<T>>::contains_key(collateral_asset_id),
                Error::<T>::CollateralInfoNotFound
            );
            let interest_coefficient = Self::collateral_infos(collateral_asset_id)
                .ok_or(Error::<T>::CollateralInfoNotFound)?
                .interest_coefficient;
            NextCDPId::<T>::try_mutate(|cdp_id| {
                *cdp_id = cdp_id
                    .checked_add(U256::from(1))
                    .ok_or(Error::<T>::ArithmeticError)?;
                Self::deposit_event(Event::CDPCreated {
                    cdp_id: *cdp_id,
                    owner: who.clone(),
                    collateral_asset_id,
                });
                <CDPDepository<T>>::insert(
                    cdp_id,
                    CollateralizedDebtPosition {
                        owner: who,
                        collateral_asset_id,
                        collateral_amount: balance!(0),
                        debt: balance!(0),
                        interest_coefficient,
                    },
                );
                DispatchResult::Ok(())
            })?;
            Ok(())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::close_cdp())]
        pub fn close_cdp(origin: OriginFor<T>, cdp_id: U256) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::accrue_internal(cdp_id)?;
            let cdp = Self::cdp(cdp_id).ok_or(Error::<T>::CDPNotFound)?;
            ensure!(who == cdp.owner, Error::<T>::OperationNotPermitted);
            ensure!(cdp.debt == 0, Error::<T>::OutstandingDebt);
            technical::Pallet::<T>::transfer_out(
                &cdp.collateral_asset_id,
                &T::TreasuryTechAccount::get(),
                &who,
                cdp.collateral_amount,
            )?;
            <CDPDepository<T>>::remove(cdp_id);
            Self::deposit_event(Event::CDPClosed {
                cdp_id,
                owner: who,
                collateral_asset_id: cdp.collateral_asset_id,
            });
            Ok(())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::deposit_collateral())]
        pub fn deposit_collateral(
            origin: OriginFor<T>,
            cdp_id: U256,
            collateral_amount: Balance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let cdp = Self::cdp(cdp_id).ok_or(Error::<T>::CDPNotFound)?;
            technical::Pallet::<T>::transfer_in(
                &cdp.collateral_asset_id,
                &who,
                &T::TreasuryTechAccount::get(),
                collateral_amount,
            )?;
            <CDPDepository<T>>::try_mutate(cdp_id, {
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
        #[pallet::weight(<T as Config>::WeightInfo::withdraw_collateral())]
        pub fn withdraw_collateral(
            origin: OriginFor<T>,
            cdp_id: U256,
            collateral_amount: Balance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::accrue_internal(cdp_id)?;
            let cdp = Self::cdp(cdp_id).ok_or(Error::<T>::CDPNotFound)?;
            ensure!(who == cdp.owner, Error::<T>::OperationNotPermitted);
            ensure!(
                cdp.collateral_amount >= collateral_amount,
                Error::<T>::NotEnoughCollateral
            );
            let new_collateral_amount = cdp
                .collateral_amount
                .checked_sub(collateral_amount)
                .ok_or(Error::<T>::ArithmeticError)?;
            ensure!(
                Self::check_cdp_is_safe(cdp.debt, new_collateral_amount, cdp.collateral_asset_id,)?,
                Error::<T>::CDPUnsafe
            );
            technical::Pallet::<T>::transfer_out(
                &cdp.collateral_asset_id,
                &T::TreasuryTechAccount::get(),
                &who,
                collateral_amount,
            )?;
            <CDPDepository<T>>::try_mutate(cdp_id, {
                |cdp| {
                    let cdp = cdp.as_mut().ok_or(Error::<T>::CDPNotFound)?;
                    cdp.collateral_amount = new_collateral_amount;
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
        #[pallet::weight(<T as Config>::WeightInfo::borrow())]
        pub fn borrow(
            origin: OriginFor<T>,
            cdp_id: U256,
            will_to_borrow_amount: Balance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::accrue_internal(cdp_id)?;
            let cdp = Self::cdp(cdp_id).ok_or(Error::<T>::CDPNotFound)?;
            ensure!(who == cdp.owner, Error::<T>::OperationNotPermitted);
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
            <CDPDepository<T>>::try_mutate(cdp_id, {
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
        #[pallet::weight(<T as Config>::WeightInfo::repay_debt())]
        pub fn repay_debt(origin: OriginFor<T>, cdp_id: U256, amount: Balance) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::accrue_internal(cdp_id)?;
            let cdp = Self::cdp(cdp_id).ok_or(Error::<T>::CDPNotFound)?;
            // if repaying amount exceeds debt, leftover is not burned
            let to_cover_debt = amount.min(cdp.debt);
            Self::burn_from(&who, to_cover_debt)?;
            <CDPDepository<T>>::try_mutate(cdp_id, {
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
        #[pallet::weight(<T as Config>::WeightInfo::liquidate())]
        pub fn liquidate(_origin: OriginFor<T>, cdp_id: U256) -> DispatchResult {
            Self::accrue_internal(cdp_id)?;
            let cdp = Self::cdp(cdp_id).ok_or(Error::<T>::CDPNotFound)?;
            let cdp_debt = cdp.debt;
            let cdp_collateral_amount = cdp.collateral_amount;
            ensure!(
                !Self::check_cdp_is_safe(cdp_debt, cdp_collateral_amount, cdp.collateral_asset_id)?,
                Error::<T>::CDPSafe
            );
            let risk_parameters = Self::collateral_infos(cdp.collateral_asset_id)
                .ok_or(Error::<T>::CollateralInfoNotFound)?
                .risk_parameters;
            let desired_kusd_amount = cdp_debt
                .checked_add(Self::liquidation_penalty() * cdp_debt)
                .ok_or(Error::<T>::ArithmeticError)?;
            let SwapOutcome { amount, .. } = T::LiquidityProxy::quote(
                DEXId::Polkaswap.into(),
                &cdp.collateral_asset_id,
                &T::KusdAssetId::get(),
                QuoteAmount::WithDesiredOutput {
                    desired_amount_out: desired_kusd_amount,
                },
                LiquiditySourceFilter::empty(DEXId::Polkaswap.into()),
                true,
            )?;
            let collateral_to_liquidate = amount
                .min(cdp.collateral_amount)
                .min(risk_parameters.max_liquidation_lot);
            let technical_account_id = technical::Pallet::<T>::tech_account_id_to_account_id(
                &T::TreasuryTechAccount::get(),
            )?;
            let swap_outcome = T::LiquidityProxy::exchange(
                DEXId::Polkaswap.into(),
                &technical_account_id,
                &technical_account_id,
                &cdp.collateral_asset_id,
                &T::KusdAssetId::get(),
                // desired output
                SwapAmount::with_desired_input(collateral_to_liquidate, balance!(0)),
                LiquiditySourceFilter::empty(DEXId::Polkaswap.into()),
            )?;
            <CDPDepository<T>>::try_mutate(cdp_id, {
                |cdp| {
                    let cdp = cdp.as_mut().ok_or(Error::<T>::CDPNotFound)?;
                    cdp.collateral_amount = cdp
                        .collateral_amount
                        .checked_sub(collateral_to_liquidate)
                        .ok_or(Error::<T>::ArithmeticError)?;
                    DispatchResult::Ok(())
                }
            })?;
            // penalty is a protocol profit which stays on treasury tech account
            let penalty = Self::liquidation_penalty() * swap_outcome.amount.min(cdp_debt);
            let kusd_amount = swap_outcome.amount - penalty;
            if cdp_debt >= kusd_amount {
                Self::burn_treasury(kusd_amount)?;
                let shortage = cdp_debt
                    .checked_sub(kusd_amount)
                    .ok_or(Error::<T>::CDPNotFound)?;
                if cdp_collateral_amount <= collateral_to_liquidate {
                    // no collateral, total default
                    // CDP debt is not covered with liquidation, now it is a protocol bad debt
                    Self::cover_with_protocol(shortage)?;
                    // close empty CDP, debt == 0, collateral == 0
                    <CDPDepository<T>>::remove(cdp_id);
                    Self::deposit_event(Event::CDPClosed {
                        cdp_id,
                        owner: cdp.owner,
                        collateral_asset_id: cdp.collateral_asset_id,
                    });
                } else {
                    // partly covered
                    <CDPDepository<T>>::try_mutate(cdp_id, {
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
            } else {
                Self::burn_treasury(cdp_debt)?;
                // CDP debt is covered
                <CDPDepository<T>>::try_mutate(cdp_id, {
                    |cdp| {
                        let cdp = cdp.as_mut().ok_or(Error::<T>::CDPNotFound)?;
                        cdp.debt = 0;
                        DispatchResult::Ok(())
                    }
                })?;
                // There is more KUSD than to cover debt and penalty, leftover goes to cdp.owner
                let leftover = kusd_amount
                    .checked_sub(cdp_debt)
                    .ok_or(Error::<T>::CDPNotFound)?;
                assets::Pallet::<T>::transfer_from(
                    &T::KusdAssetId::get(),
                    &technical_account_id,
                    &cdp.owner,
                    leftover,
                )?;
            };
            Self::deposit_event(Event::Liquidated {
                cdp_id,
                collateral_asset_id: cdp.collateral_asset_id,
                collateral_amount: collateral_to_liquidate,
                kusd_amount,
                penalty,
            });

            Ok(())
        }

        /// Updates cdp debt with interest
        /// Unsigned call possible
        #[pallet::call_index(7)]
        #[pallet::weight(<T as Config>::WeightInfo::accrue())]
        pub fn accrue(_origin: OriginFor<T>, cdp_id: U256) -> DispatchResult {
            ensure!(Self::is_accruable(&cdp_id)?, Error::<T>::NoDebt);
            Self::accrue_internal(cdp_id)?;
            Ok(())
        }

        /// Updates collateral risk parameters
        /// Is set by risk management
        #[pallet::call_index(8)]
        #[pallet::weight(<T as Config>::WeightInfo::update_collateral_risk_parameters())]
        pub fn update_collateral_risk_parameters(
            origin: OriginFor<T>,
            collateral_asset_id: AssetIdOf<T>,
            new_risk_parameters: CollateralRiskParameters,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_risk_manager(&who)?;
            ensure!(
                T::AssetInfoProvider::asset_exists(&collateral_asset_id),
                Error::<T>::WrongAssetId
            );
            if Self::collateral_infos(collateral_asset_id).map_or(false, |old_info| {
                old_info.risk_parameters.stability_fee_rate
                    != new_risk_parameters.stability_fee_rate
            }) {
                Self::update_collateral_interest_coefficient(collateral_asset_id)?;
                <CollateralInfos<T>>::try_mutate(collateral_asset_id, |collateral_info| {
                    let collateral_info = collateral_info
                        .as_mut()
                        .ok_or(Error::<T>::CollateralInfoNotFound)?;
                    collateral_info.risk_parameters = new_risk_parameters;
                    DispatchResult::Ok(())
                })?;
            } else {
                <CollateralInfos<T>>::insert(
                    collateral_asset_id,
                    CollateralInfo {
                        risk_parameters: new_risk_parameters.clone(),
                        last_fee_update_time: Timestamp::<T>::get(),
                        interest_coefficient: FixedU128::one(),
                    },
                );
            }
            Self::deposit_event(Event::CollateralRiskParametersUpdated {
                collateral_asset_id,
                risk_parameters: new_risk_parameters,
            });

            Ok(())
        }

        /// Sets hard cap for total KUSD supply
        /// Is set by risk management
        #[pallet::call_index(9)]
        #[pallet::weight(<T as Config>::WeightInfo::update_hard_cap_total_supply())]
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
        #[pallet::weight(<T as Config>::WeightInfo::update_liquidation_penalty())]
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
        #[pallet::weight(<T as Config>::WeightInfo::withdraw_profit())]
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
        #[pallet::weight(<T as Config>::WeightInfo::donate())]
        pub fn donate(origin: OriginFor<T>, kusd_amount: Balance) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::cover_bad_debt(&who, kusd_amount)?;
            Self::deposit_event(Event::Donation {
                amount: kusd_amount,
            });

            Ok(())
        }

        /// Adds risk manager account
        #[pallet::call_index(13)]
        #[pallet::weight(<T as Config>::WeightInfo::add_risk_manager())]
        pub fn add_risk_manager(origin: OriginFor<T>, account_id: T::AccountId) -> DispatchResult {
            ensure_root(origin)?;
            <RiskManagers<T>>::mutate(|option_risk_managers| match option_risk_managers {
                Some(risk_managers) => {
                    let _ = risk_managers.insert(account_id);
                }
                None => {
                    let mut risk_managers = BTreeSet::new();
                    let _ = risk_managers.insert(account_id);
                    let _ = option_risk_managers.insert(risk_managers);
                }
            });

            Ok(())
        }

        // Removes risk manager account
        #[pallet::call_index(14)]
        #[pallet::weight(<T as Config>::WeightInfo::remove_risk_manager())]
        pub fn remove_risk_manager(
            origin: OriginFor<T>,
            account_id: T::AccountId,
        ) -> DispatchResult {
            ensure_root(origin)?;
            <RiskManagers<T>>::mutate(|option_risk_managers| match option_risk_managers {
                Some(risk_managers) => {
                    let _ = risk_managers.remove(&account_id);
                }
                None => {}
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
                // TODO spamming with accrue calls, add some filter to not call too often
                Call::accrue { cdp_id } => {
                    if Self::is_accruable(cdp_id)
                        .map_err(|_| InvalidTransaction::Custom(VALIDATION_ERROR_ACCRUE))?
                    {
                        ValidTransaction::with_tag_prefix("Kensetsu::accrue")
                            .priority(T::UnsignedPriority::get())
                            .longevity(T::UnsignedLongevity::get())
                            .and_provides([&cdp_id])
                            .propagate(true)
                            .build()
                    } else {
                        InvalidTransaction::Custom(VALIDATION_ERROR_ACCRUE_NO_DEBT).into()
                    }
                }
                Call::liquidate { cdp_id } => {
                    let cdp = Self::cdp(cdp_id)
                        .ok_or(InvalidTransaction::Custom(VALIDATION_ERROR_CHECK_SAFE))?;
                    if !Self::check_cdp_is_safe(
                        cdp.debt,
                        cdp.collateral_amount,
                        cdp.collateral_asset_id,
                    )
                    .map_err(|_| InvalidTransaction::Custom(VALIDATION_ERROR_CHECK_SAFE))?
                    {
                        ValidTransaction::with_tag_prefix("Kensetsu::liquidate")
                            .priority(T::UnsignedPriority::get())
                            .longevity(T::UnsignedLongevity::get())
                            .and_provides([&cdp_id])
                            .propagate(true)
                            .build()
                    } else {
                        InvalidTransaction::Custom(VALIDATION_ERROR_CDP_SAFE).into()
                    }
                }
                _ => {
                    warn!("Unknown unsigned call {:?}", call);
                    InvalidTransaction::Call.into()
                }
            }
        }
    }

    impl<T: Config> Pallet<T> {
        /// Ensures that `who` is a risk manager
        /// Risk manager can set protocol risk parameters.
        fn ensure_risk_manager(who: &AccountIdOf<T>) -> DispatchResult {
            if !Self::risk_managers().map_or(false, |risk_managers| risk_managers.contains(who)) {
                return Err(Error::<T>::OperationNotPermitted.into());
            }

            Ok(())
        }

        /// Ensures that `who` is a protocol owner
        /// Protocol owner can withdraw profit from the protocol.
        fn ensure_protocol_owner(who: &AccountIdOf<T>) -> DispatchResult {
            if !Self::risk_managers().map_or(false, |risk_managers| risk_managers.contains(who)) {
                return Err(Error::<T>::OperationNotPermitted.into());
            }

            Ok(())
        }

        /// Checks loan-to-value ratio is `safe` and is not going to be liquidated
        /// Returns true if CDP is safe, LTV <= liquidation threshold
        pub(crate) fn check_cdp_is_safe(
            debt: Balance,
            collateral: Balance,
            collateral_asset_id: AssetIdOf<T>,
        ) -> Result<bool, DispatchError> {
            let liquidation_ratio = Self::collateral_infos(collateral_asset_id)
                .ok_or(Error::<T>::CollateralInfoNotFound)?
                .risk_parameters
                .liquidation_ratio;
            let collateral_reference_price = FixedU128::from_inner(
                T::ReferencePriceProvider::get_reference_price(&collateral_asset_id)?,
            );
            let collateral_value = collateral_reference_price
                .checked_mul(&FixedU128::from_inner(collateral))
                .ok_or(Error::<T>::ArithmeticError)?;
            let max_safe_debt = FixedU128::from_perbill(liquidation_ratio)
                .checked_mul(&collateral_value)
                .ok_or(Error::<T>::ArithmeticError)?;
            let debt = FixedU128::from_inner(debt);
            Ok(debt <= max_safe_debt)
        }

        /// Ensures that new emission will not exceed collateral hard cap
        fn ensure_collateral_cap(
            collateral_asset_id: AssetIdOf<T>,
            new_emission: Balance,
        ) -> DispatchResult {
            let hard_cap = Self::collateral_infos(collateral_asset_id)
                .ok_or(Error::<T>::CollateralInfoNotFound)?
                .risk_parameters
                .hard_cap;

            let current_supply_for_collateral = balance!(0);
            for cdp in <CDPDepository<T>>::iter_values() {
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
                    <= hard_cap,
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

        /// Returns true if CDP has debt.
        fn is_accruable(cdp_id: &U256) -> Result<bool, DispatchError> {
            let cdp = Self::cdp(cdp_id).ok_or(Error::<T>::CDPNotFound)?;
            Ok(cdp.debt > 0)
        }

        /// Recalculates collateral interest coefficient with the current timestamp
        fn update_collateral_interest_coefficient(
            collateral_asset_id: AssetIdOf<T>,
        ) -> Result<CollateralInfo<T::Moment>, DispatchError> {
            let collateral_info = <CollateralInfos<T>>::try_mutate(collateral_asset_id, {
                |collateral_info| {
                    let collateral_info = collateral_info
                        .as_mut()
                        .ok_or(Error::<T>::CollateralInfoNotFound)?;
                    let now = Timestamp::<T>::get();
                    ensure!(
                        now >= collateral_info.last_fee_update_time,
                        Error::<T>::AccrueWrongTime
                    );
                    // do not update if time is the same
                    if now > collateral_info.last_fee_update_time {
                        let time_passed = now
                            .checked_sub(&collateral_info.last_fee_update_time)
                            .ok_or(Error::<T>::ArithmeticError)?;
                        let new_coefficient = compound(
                            collateral_info.interest_coefficient.into_inner(),
                            collateral_info.risk_parameters.stability_fee_rate,
                            time_passed
                                .checked_into::<u64>()
                                .ok_or(Error::<T>::ArithmeticError)?,
                        )
                        .map_err(|_| Error::<T>::ArithmeticError)?;
                        collateral_info.last_fee_update_time = now;
                        collateral_info.interest_coefficient =
                            FixedU128::from_inner(new_coefficient);
                    }
                    Ok::<CollateralInfo<T::Moment>, DispatchError>(collateral_info.clone())
                }
            })?;

            Ok(collateral_info)
        }

        // TODO optimization - return cdp and collateral info
        /// Accrue stability fee from CDP
        /// Calculates fees accrued since last update using continuous compounding formula.
        /// The fees is a protocol gain.
        fn accrue_internal(
            cdp_id: U256,
        ) -> Result<CollateralizedDebtPosition<AccountIdOf<T>, AssetIdOf<T>>, DispatchError>
        {
            let mut cdp = Self::cdp(cdp_id).ok_or(Error::<T>::CDPNotFound)?;
            let collateral_info =
                Self::update_collateral_interest_coefficient(cdp.collateral_asset_id)?;
            let new_coefficient = collateral_info.interest_coefficient;
            let interest_percent = (new_coefficient
                .checked_sub(&cdp.interest_coefficient)
                .ok_or(Error::<T>::ArithmeticError)?)
            .checked_div(&cdp.interest_coefficient)
            .ok_or(Error::<T>::ArithmeticError)?;
            let mut stability_fee = FixedU128::from_inner(cdp.debt)
                .checked_mul(&interest_percent)
                .ok_or(Error::<T>::ArithmeticError)?
                .into_inner();
            let new_debt = cdp
                .debt
                .checked_add(stability_fee)
                .ok_or(Error::<T>::ArithmeticError)?;
            cdp = <CDPDepository<T>>::try_mutate(cdp_id, {
                |cdp| {
                    let cdp = cdp.as_mut().ok_or(Error::<T>::CDPNotFound)?;
                    cdp.debt = new_debt;
                    cdp.interest_coefficient = new_coefficient;
                    Ok::<CollateralizedDebtPosition<T::AccountId, T::AssetId>, DispatchError>(
                        cdp.clone(),
                    )
                }
            })?;
            let mut new_bad_debt = <BadDebt<T>>::get();
            if new_bad_debt > 0 {
                if stability_fee <= new_bad_debt {
                    new_bad_debt = new_bad_debt
                        .checked_sub(stability_fee)
                        .ok_or(Error::<T>::ArithmeticError)?;
                    stability_fee = 0;
                } else {
                    stability_fee = stability_fee
                        .checked_sub(new_bad_debt)
                        .ok_or(Error::<T>::ArithmeticError)?;
                    new_bad_debt = balance!(0);
                };
                <BadDebt<T>>::try_mutate(|bad_debt| {
                    *bad_debt = new_bad_debt;
                    DispatchResult::Ok(())
                })?;
            }
            Self::mint_treasury(stability_fee)?;

            Ok(cdp)
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
            let to_burn = if amount <= protocol_positive_balance {
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
            Ok(<CDPDepository<T>>::iter()
                .filter(|(_, cdp)| cdp.owner == *account_id)
                .map(|(cdp_id, _)| cdp_id)
                .collect())
        }
    }
}
