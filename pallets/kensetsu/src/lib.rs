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

//! Kensetsu is a over collateralized lending protocol, clone of MakerDAO.
//! An individual can create a collateral debt positions (CDPs) for one of the listed token and
//! deposit or lock amount of the token in CDP as collateral. Then the individual is allowed to
//! borrow new minted Kensetsu USD (KUSD) in amount up to value of collateral corrected by
//! `liquidation_ratio` coefficient. The debt in KUSD is a subject of `stability_fee` interest rate.
//! Collateral may be unlocked only when the debt and the interest are paid back. If the value of
//! collateral has changed in a way that it does not secure the debt, the collateral is liquidated
//! to cover the debt and the interest.

pub use pallet::*;

use assets::AssetIdOf;
use codec::{Decode, Encode, MaxEncodedLen};
use common::{balance, Balance};
use frame_support::log::{debug, warn};
use scale_info::TypeInfo;
use sp_arithmetic::{FixedU128, Perbill};

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
/// Liquidation limit reached
const VALIDATION_ERROR_LIQUIDATION_LIMIT: u8 = 5;

#[derive(
    Debug, Clone, Encode, Decode, PartialEq, Eq, PartialOrd, Ord, Copy, scale_info::TypeInfo,
)]
pub enum CdpType {
    /// Pays stability fee in underlying collateral, cannot be liquidated.
    Type1,
    /// Pays stability fee in stable coins, can be liquidated.
    Type2,
}

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

    /// Protocol Interest rate per millisecond
    pub stability_fee_rate: FixedU128,

    /// Minimal deposit in collateral AssetId.
    /// In order to protect from empty CDPs.
    pub minimal_collateral_deposit: Balance,
}

/// Collateral parameters, includes risk info and additional data for interest rate calculation
#[derive(Debug, Clone, Encode, Decode, MaxEncodedLen, TypeInfo, PartialEq, Eq, PartialOrd, Ord)]
pub struct CollateralInfo<Moment> {
    /// Collateral Risk parameters set by risk management
    pub risk_parameters: CollateralRiskParameters,

    /// Amount of KUSD issued for the collateral
    pub kusd_supply: Balance,

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
    /// Initializes on creation with collateral interest coefficient equal to 1.
    /// The coefficient is growing over time with interest rate.
    /// Actual interest is: (collateral.coefficient - cdp.coefficient) / cdp.coefficient
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
        DEXId, Description, LiquidityProxyTrait, LiquiditySourceFilter, PriceToolsProvider,
        PriceVariant, DAI,
    };
    use frame_support::pallet_prelude::*;
    use frame_support::traits::Randomness;
    use frame_system::offchain::{SendTransactionTypes, SubmitTransaction};
    use frame_system::pallet_prelude::*;
    use pallet_timestamp as timestamp;
    use sp_arithmetic::traits::{CheckedDiv, CheckedMul, CheckedSub};
    use sp_arithmetic::Percent;
    use sp_core::bounded::{BoundedBTreeSet, BoundedVec};
    use sp_runtime::traits::{CheckedConversion, One, Zero};
    use sp_std::collections::vec_deque::VecDeque;
    use sp_std::vec::Vec;

    /// CDP id type
    pub type CdpId = u128;

    /// The current storage version.
    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        /// Resets liquidation flag.
        fn on_initialize(_now: T::BlockNumber) -> Weight {
            LiquidatedThisBlock::<T>::put(false);
            T::DbWeight::get().writes(1)
        }

        /// Main off-chain worker procedure.
        ///
        /// Accrues fees and calls liquidations
        fn offchain_worker(block_number: T::BlockNumber) {
            debug!(
                "Entering off-chain worker, block number is {:?}",
                block_number
            );
            let mut unsafe_cdp_ids = VecDeque::<CdpId>::new();
            for (cdp_id, cdp) in <CDPDepository<T>>::iter() {
                if let Ok(true) = Self::is_accruable(&cdp_id) {
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

                // Liquidation
                match Self::check_cdp_is_safe(
                    cdp.debt,
                    cdp.collateral_amount,
                    cdp.collateral_asset_id,
                ) {
                    Ok(true) => {}
                    Ok(false) => {
                        debug!("CDP {:?} unsafe", cdp_id);
                        unsafe_cdp_ids.push_back(cdp_id);
                    }
                    Err(err) => {
                        warn!(
                            "Failed in offchain_worker check cdp {:?} safety: {:?}",
                            cdp_id, err
                        );
                    }
                }
            }
            if !unsafe_cdp_ids.is_empty() {
                // Randomly choose one of CDPs to liquidate.
                // This CDP id can be predicted and manipulated in front-running attack. It is a
                // known problem. The purpose of the code is not to protect from the attack but to
                // make choosing of CDP to liquidate more 'fair' then incremental order.
                let (randomness, _) = T::Randomness::random(&b"kensetsu"[..]);
                match randomness {
                    Some(randomness) => {
                        match u32::decode(&mut randomness.as_ref()) {
                            Ok(random_number) => {
                                // Random bias by modulus operation is acceptable here
                                let random_id = random_number as usize % unsafe_cdp_ids.len();
                                unsafe_cdp_ids
                                    .get(random_id)
                                    .map_or_else(
                                        || {
                                            warn!("Failed to get random cdp_id {}.", random_id);
                                        },
                                        |cdp_id| {
                                        debug!("Liquidation of CDP {:?}", cdp_id);
                                        let call = Call::<T>::liquidate { cdp_id: *cdp_id };
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
                                    });
                            }
                            Err(error) => {
                                warn!("Failed to get randomness during liquidation: {}", error);
                            }
                        }
                    }
                    None => {
                        warn!("No randomness provided.");
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
        type Randomness: Randomness<Option<Self::Hash>, Self::BlockNumber>;
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
        type KenAssetId: Get<Self::AssetId>;
        type KusdAssetId: Get<Self::AssetId>;
        type PriceTools: PriceToolsProvider<Self::AssetId>;
        type LiquidityProxy: LiquidityProxyTrait<Self::DEXId, Self::AccountId, Self::AssetId>;

        /// Percent of KEN that is reminted and goes to Demeter farming incentivization
        #[pallet::constant]
        type KenIncentiveRemintPercent: Get<Percent>;

        /// Maximum number of CDP that one user can create
        #[pallet::constant]
        type MaxCdpsPerOwner: Get<u32>;

        /// Maximum number of risk manager team members
        #[pallet::constant]
        type MaxRiskManagementTeamSize: Get<u32>;

        /// A configuration for base priority of unsigned transactions.
        #[pallet::constant]
        type UnsignedPriority: Get<TransactionPriority>;

        /// Minimal uncollected fee in KUSD that triggers offchain worker to call accrue.
        #[pallet::constant]
        type MinimalStabilityFeeAccrue: Get<Balance>;

        /// A configuration for longevity of unsigned transactions.
        #[pallet::constant]
        type UnsignedLongevity: Get<u64>;

        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    pub type Timestamp<T> = timestamp::Pallet<T>;

    /// Default value for LiquidatedThisBlock storage
    #[pallet::type_value]
    pub fn DefaultLiquidatedThisBlock() -> bool {
        false
    }

    /// Flag indicates that liquidation took place in this block. Only one liquidation per block is
    /// allowed, the flag is dropped every block.
    #[pallet::storage]
    #[pallet::getter(fn liquidated_this_block)]
    pub type LiquidatedThisBlock<T> = StorageValue<_, bool, ValueQuery, DefaultLiquidatedThisBlock>;

    /// System bad debt, the amount of KUSD not secured with collateral.
    #[pallet::storage]
    #[pallet::getter(fn bad_debt)]
    pub type BadDebt<T> = StorageValue<_, Balance, ValueQuery>;

    /// Parametes for collaterals, include risk parameters and interest recalculation coefficients
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
    /// Borrows tax to buy back and burn KEN
    #[pallet::storage]
    #[pallet::getter(fn borrow_tax)]
    pub type BorrowTax<T> = StorageValue<_, Percent, ValueQuery>;

    /// Liquidation penalty
    #[pallet::storage]
    #[pallet::getter(fn liquidation_penalty)]
    pub type LiquidationPenalty<T> = StorageValue<_, Percent, ValueQuery>;

    /// CDP counter used for CDP id
    #[pallet::storage]
    pub type NextCDPId<T> = StorageValue<_, CdpId, ValueQuery>;

    /// Storage of all CDPs, where key is a unique CDP identifier
    #[pallet::storage]
    #[pallet::getter(fn cdp)]
    pub type CDPDepository<T: Config> =
        StorageMap<_, Identity, CdpId, CollateralizedDebtPosition<AccountIdOf<T>, AssetIdOf<T>>>;

    /// Index links owner to CDP ids, not needed by protocol, but used by front-end
    #[pallet::storage]
    #[pallet::getter(fn cdp_owner_index)]
    pub type CdpOwnerIndex<T: Config> =
        StorageMap<_, Identity, AccountIdOf<T>, BoundedVec<CdpId, T::MaxCdpsPerOwner>>;

    /// Accounts of risk management team
    #[pallet::storage]
    #[pallet::getter(fn risk_managers)]
    pub type RiskManagers<T: Config> =
        StorageValue<_, BoundedBTreeSet<T::AccountId, T::MaxRiskManagementTeamSize>>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        CDPCreated {
            cdp_id: CdpId,
            owner: AccountIdOf<T>,
            collateral_asset_id: AssetIdOf<T>,
            debt_asset_id: AssetIdOf<T>,
            cdp_type: CdpType,
        },
        CDPClosed {
            cdp_id: CdpId,
            owner: AccountIdOf<T>,
            collateral_asset_id: AssetIdOf<T>,
            /// Amount of collateral returned to the CDP owner. 0 means the collateral was
            /// liquidated.
            collateral_amount: Balance,
        },
        CollateralDeposit {
            cdp_id: CdpId,
            owner: AccountIdOf<T>,
            collateral_asset_id: AssetIdOf<T>,
            amount: Balance,
        },
        DebtIncreased {
            cdp_id: CdpId,
            owner: AccountIdOf<T>,
            debt_asset_id: AssetIdOf<T>,
            /// Amount borrowed in debt asset id.
            amount: Balance,
        },
        DebtPayment {
            cdp_id: CdpId,
            owner: AccountIdOf<T>,
            debt_asset_id: AssetIdOf<T>,
            // KUSD amount paid off
            amount: Balance,
        },
        Liquidated {
            cdp_id: CdpId,
            // what was liquidated
            collateral_asset_id: AssetIdOf<T>,
            collateral_amount: Balance,
            debt_asset_id: AssetIdOf<T>,
            // KUSD amount from liquidation to cover debt
            proceeds: Balance,
            // liquidation penalty
            penalty: Balance,
        },
        CollateralRiskParametersUpdated {
            collateral_asset_id: AssetIdOf<T>,
            risk_parameters: CollateralRiskParameters,
        },
        DebtTokenHardCapUpdated {
            debt_asset_id: AssetIdOf<T>,
            new_hard_cap: Balance,
            old_hard_cap: Balance,
        },
        BorrowTaxUpdated {
            old_borrow_tax: Percent,
            new_borrow_tax: Percent,
        },
        LiquidationPenaltyUpdated {
            new_liquidation_penalty: Percent,
            old_liquidation_penalty: Percent,
        },
        ProfitWithdrawn {
            debt_asset_id: AssetIdOf<T>,
            amount: Balance,
        },
        Donation {
            debt_asset_id: AssetIdOf<T>,
            amount: Balance,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        ArithmeticError,
        WrongAssetId,
        CDPNotFound,
        CollateralInfoNotFound,
        CollateralBelowMinimal,
        CDPSafe,
        CDPUnsafe,
        /// Too many CDPs per user
        CDPLimitPerUser,
        /// Risk management team size exceeded
        TooManyManagers,
        OperationNotPermitted,
        /// Uncollected stability fee is too small for accrue
        UncollectedStabilityFeeTooSmall,
        HardCapSupply,
        AccrueWrongTime,
        /// Liquidation lot set in risk parameters is zero, cannot liquidate
        ZeroLiquidationLot,
        /// Liquidation limit reached
        LiquidationLimit,
        /// Wrong borrow amounts
        WrongBorrowAmounts,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Creates a Collateralized Debt Position (CDP).
        /// The extrinsic combines depositing collateral and borrowing.
        /// Borrow amount will be as max as possible in the range
        /// `[borrow_amount_min, borrow_amount_max]` in order to confrom the slippage tolerance.
        ///
        /// ## Parameters
        ///
        /// - `origin`: The origin of the transaction.
        /// - `collateral_asset_id`: The identifier of the asset used as collateral.
        /// - `collateral_amount`: The amount of collateral to be deposited.
        /// - `borrow_amount_min`: The minimum amount the user wants to borrow.
        /// - `borrow_amount_max`: The maximum amount the user wants to borrow.
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::create_cdp())]
        pub fn create_cdp(
            origin: OriginFor<T>,
            collateral_asset_id: AssetIdOf<T>,
            collateral_amount: Balance,
            borrow_amount_min: Balance,
            borrow_amount_max: Balance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(
                borrow_amount_min <= borrow_amount_max,
                Error::<T>::WrongBorrowAmounts
            );
            let collateral_info = Self::collateral_infos(collateral_asset_id)
                .ok_or(Error::<T>::CollateralInfoNotFound)?;
            let interest_coefficient = collateral_info.interest_coefficient;
            ensure!(
                collateral_amount >= collateral_info.risk_parameters.minimal_collateral_deposit,
                Error::<T>::CollateralBelowMinimal
            );
            let cdp_id = Self::increment_cdp_id()?;
            Self::insert_cdp(
                &who,
                cdp_id,
                CollateralizedDebtPosition {
                    owner: who.clone(),
                    collateral_asset_id,
                    collateral_amount: balance!(0),
                    debt: balance!(0),
                    interest_coefficient,
                },
            )?;
            Self::deposit_event(Event::CDPCreated {
                cdp_id,
                owner: who.clone(),
                collateral_asset_id,
                debt_asset_id: T::KusdAssetId::get(),
                cdp_type: CdpType::Type2,
            });
            if collateral_amount > 0 {
                Self::deposit_internal(&who, cdp_id, collateral_amount)?;
            }
            if borrow_amount_max > 0 {
                Self::borrow_internal(&who, cdp_id, borrow_amount_min, borrow_amount_max)?;
            }
            Ok(())
        }

        /// Closes a Collateralized Debt Position (CDP).
        ///
        /// If a CDP has outstanding debt, this amount is covered with owner balance. Collateral
        /// then is returned to the owner and CDP is deleted.
        ///
        /// ## Parameters
        ///
        /// - `origin`: The origin of the transaction, only CDP owner is allowed.
        /// - `cdp_id`: The ID of the CDP to be closed.
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::close_cdp())]
        pub fn close_cdp(origin: OriginFor<T>, cdp_id: CdpId) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let cdp = Self::cdp(cdp_id).ok_or(Error::<T>::CDPNotFound)?;
            ensure!(who == cdp.owner, Error::<T>::OperationNotPermitted);
            Self::repay_debt_internal(cdp_id, cdp.debt)?;
            Self::delete_cdp(cdp_id)
        }

        /// Deposits collateral into a Collateralized Debt Position (CDP).
        ///
        /// ## Parameters
        ///
        /// - `origin`: The origin of the transaction.
        /// - `cdp_id`: The ID of the CDP to deposit collateral into.
        /// - `collateral_amount`: The amount of collateral to deposit.
        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::deposit_collateral())]
        pub fn deposit_collateral(
            origin: OriginFor<T>,
            cdp_id: CdpId,
            collateral_amount: Balance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::deposit_internal(&who, cdp_id, collateral_amount)
        }

        /// Borrows funds against a Collateralized Debt Position (CDP).
        /// Borrow amount will be as max as possible in the range
        /// `[borrow_amount_min, borrow_amount_max]` in order to confrom the slippage tolerance.
        /// ## Parameters
        ///
        /// - `origin`: The origin of the transaction.
        /// - `cdp_id`: The ID of the CDP to borrow against.
        /// - `borrow_amount_min`: The minimum amount the user wants to borrow.
        /// - `borrow_amount_max`: The maximum amount the user wants to borrow.
        #[pallet::call_index(3)]
        #[pallet::weight(<T as Config>::WeightInfo::borrow())]
        pub fn borrow(
            origin: OriginFor<T>,
            cdp_id: CdpId,
            borrow_amount_min: Balance,
            borrow_amount_max: Balance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(
                borrow_amount_min <= borrow_amount_max,
                Error::<T>::WrongBorrowAmounts
            );
            Self::borrow_internal(&who, cdp_id, borrow_amount_min, borrow_amount_max)
        }

        /// Repays debt against a Collateralized Debt Position (CDP).
        ///
        /// ## Parameters
        ///
        /// - `origin`: The origin of the transaction.
        /// - `cdp_id`: The ID of the CDP to repay debt for.
        /// - `amount`: The amount to repay against the CDP's debt.
        #[pallet::call_index(4)]
        #[pallet::weight(<T as Config>::WeightInfo::repay_debt())]
        pub fn repay_debt(origin: OriginFor<T>, cdp_id: CdpId, amount: Balance) -> DispatchResult {
            // any account can repay debt
            let _ = ensure_signed(origin)?;
            Self::repay_debt_internal(cdp_id, amount)
        }

        /// Liquidates a Collateralized Debt Position (CDP) if it becomes unsafe.
        ///
        /// ## Parameters
        ///
        /// - `_origin`: The origin of the transaction (unused).
        /// - `cdp_id`: The ID of the CDP to be liquidated.
        #[pallet::call_index(5)]
        #[pallet::weight(<T as Config>::WeightInfo::liquidate())]
        pub fn liquidate(_origin: OriginFor<T>, cdp_id: CdpId) -> DispatchResult {
            ensure!(
                Self::check_liquidation_available(),
                Error::<T>::LiquidationLimit
            );
            let cdp = Self::accrue_internal(cdp_id)?;
            ensure!(
                !Self::check_cdp_is_safe(cdp.debt, cdp.collateral_amount, cdp.collateral_asset_id)?,
                Error::<T>::CDPSafe
            );
            let technical_account_id = technical::Pallet::<T>::tech_account_id_to_account_id(
                &T::TreasuryTechAccount::get(),
            )?;
            let (collateral_liquidated, proceeds, penalty) =
                Self::swap(&cdp, &technical_account_id)?;
            Self::update_cdp_collateral(
                cdp_id,
                cdp.collateral_amount
                    .checked_sub(collateral_liquidated)
                    .ok_or(Error::<T>::ArithmeticError)?,
            )?;
            // KUSD supply change for collateral.
            let kusd_supply_change: Balance;
            if cdp.debt >= proceeds {
                Self::burn_treasury(proceeds)?;
                let shortage = cdp
                    .debt
                    .checked_sub(proceeds)
                    .ok_or(Error::<T>::CDPNotFound)?;
                if cdp.collateral_amount <= collateral_liquidated {
                    // no collateral, total default
                    // CDP debt is not covered with liquidation, now it is a protocol bad debt
                    Self::cover_with_protocol(shortage)?;
                    // close empty CDP, debt == 0, collateral == 0
                    Self::delete_cdp(cdp_id)?;
                    kusd_supply_change = cdp.debt;
                } else {
                    // partly covered
                    Self::update_cdp_debt(cdp_id, shortage)?;
                    kusd_supply_change = proceeds;
                }
            } else {
                Self::burn_treasury(cdp.debt)?;
                // CDP debt is covered
                Self::update_cdp_debt(cdp_id, 0)?;
                kusd_supply_change = cdp.debt;
                // There is more KUSD than to cover debt and penalty, leftover goes to cdp.owner
                let leftover = proceeds
                    .checked_sub(cdp.debt)
                    .ok_or(Error::<T>::ArithmeticError)?;
                assets::Pallet::<T>::transfer_from(
                    &T::KusdAssetId::get(),
                    &technical_account_id,
                    &cdp.owner,
                    leftover,
                )?;
            };
            Self::decrease_collateral_kusd_supply(&cdp.collateral_asset_id, kusd_supply_change)?;
            LiquidatedThisBlock::<T>::put(true);
            Self::deposit_event(Event::Liquidated {
                cdp_id,
                collateral_asset_id: cdp.collateral_asset_id,
                collateral_amount: collateral_liquidated,
                debt_asset_id: T::KusdAssetId::get(),
                proceeds,
                penalty,
            });

            Ok(())
        }

        /// Accrues interest on a Collateralized Debt Position (CDP).
        ///
        /// ## Parameters
        ///
        /// - `_origin`: The origin of the transaction (unused).
        /// - `cdp_id`: The ID of the CDP to accrue interest on.
        #[pallet::call_index(6)]
        #[pallet::weight(<T as Config>::WeightInfo::accrue())]
        pub fn accrue(_origin: OriginFor<T>, cdp_id: CdpId) -> DispatchResult {
            ensure!(
                Self::is_accruable(&cdp_id)?,
                Error::<T>::UncollectedStabilityFeeTooSmall
            );
            Self::accrue_internal(cdp_id)?;
            Ok(())
        }

        /// Updates the risk parameters for a specific collateral asset.
        ///
        /// ## Parameters
        ///
        /// - `origin`: The origin of the transaction.
        /// - `collateral_asset_id`: The identifier of the collateral asset.
        /// - `new_risk_parameters`: The new risk parameters to be set for the collateral asset.
        #[pallet::call_index(7)]
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
            Self::upsert_collateral_info(&collateral_asset_id, new_risk_parameters)?;
            Self::deposit_event(Event::CollateralRiskParametersUpdated {
                collateral_asset_id,
                risk_parameters: new_risk_parameters,
            });

            Ok(())
        }

        /// Updates the hard cap for the total supply of a stablecoin.
        ///
        /// ## Parameters
        ///
        /// - `origin`: The origin of the transaction.
        /// - `new_hard_cap`: The new hard cap value to be set for the total supply.
        #[pallet::call_index(8)]
        #[pallet::weight(<T as Config>::WeightInfo::update_hard_cap_total_supply())]
        pub fn update_hard_cap_total_supply(
            origin: OriginFor<T>,
            new_hard_cap: Balance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_risk_manager(&who)?;
            let old_hard_cap = KusdHardCap::<T>::get();
            KusdHardCap::<T>::set(new_hard_cap);
            Self::deposit_event(Event::DebtTokenHardCapUpdated {
                debt_asset_id: T::KusdAssetId::get(),
                new_hard_cap,
                old_hard_cap,
            });
            Ok(())
        }

        /// Updates the borrow tax applied during borrow.
        ///
        /// ## Parameters
        ///
        /// - `origin`: The origin of the transaction.
        /// - `new_borrow_tax`: The new borrow tax percentage to be set.
        #[pallet::call_index(9)]
        #[pallet::weight(<T as Config>::WeightInfo::update_borrow_tax())]
        pub fn update_borrow_tax(origin: OriginFor<T>, new_borrow_tax: Percent) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_risk_manager(&who)?;
            let old_borrow_tax = BorrowTax::<T>::get();
            BorrowTax::<T>::set(new_borrow_tax);
            Self::deposit_event(Event::BorrowTaxUpdated {
                new_borrow_tax,
                old_borrow_tax,
            });

            Ok(())
        }

        /// Updates the liquidation penalty applied during CDP liquidation.
        ///
        /// ## Parameters
        ///
        /// - `origin`: The origin of the transaction.
        /// - `new_liquidation_penalty`: The new liquidation penalty percentage to be set.
        #[pallet::call_index(10)]
        #[pallet::weight(<T as Config>::WeightInfo::update_liquidation_penalty())]
        pub fn update_liquidation_penalty(
            origin: OriginFor<T>,
            new_liquidation_penalty: Percent,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_risk_manager(&who)?;
            let old_liquidation_penalty = LiquidationPenalty::<T>::get();
            LiquidationPenalty::<T>::set(new_liquidation_penalty);
            Self::deposit_event(Event::LiquidationPenaltyUpdated {
                new_liquidation_penalty,
                old_liquidation_penalty,
            });

            Ok(())
        }

        /// Withdraws protocol profit in the form of stablecoin (KUSD).
        ///
        /// ## Parameters
        ///
        /// - `origin`: The origin of the transaction.
        /// - `kusd_amount`: The amount of stablecoin (KUSD) to withdraw as protocol profit.
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
                debt_asset_id: T::KusdAssetId::get(),
                amount: kusd_amount,
            });

            Ok(())
        }

        /// Donates stablecoin (KUSD) to cover protocol bad debt.
        ///
        /// ## Parameters
        ///
        /// - `origin`: The origin of the transaction.
        /// - `kusd_amount`: The amount of stablecoin (KUSD) to donate to cover bad debt.
        #[pallet::call_index(12)]
        #[pallet::weight(<T as Config>::WeightInfo::donate())]
        pub fn donate(origin: OriginFor<T>, kusd_amount: Balance) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::cover_bad_debt(&who, kusd_amount)?;
            Self::deposit_event(Event::Donation {
                debt_asset_id: T::KusdAssetId::get(),
                amount: kusd_amount,
            });

            Ok(())
        }

        /// Adds a new account ID to the set of risk managers.
        ///
        /// ## Parameters
        ///
        /// - `origin`: The origin of the transaction.
        /// - `account_id`: The account ID to be added as a risk manager.
        #[pallet::call_index(13)]
        #[pallet::weight(<T as Config>::WeightInfo::add_risk_manager())]
        pub fn add_risk_manager(origin: OriginFor<T>, account_id: T::AccountId) -> DispatchResult {
            ensure_root(origin)?;
            RiskManagers::<T>::try_mutate(|option_risk_managers| {
                option_risk_managers
                    .get_or_insert(BoundedBTreeSet::new())
                    .try_insert(account_id)
                    .map_err(|_| Error::<T>::TooManyManagers)
            })?;

            Ok(())
        }

        /// Removes an account ID from the set of risk managers.
        ///
        /// ## Parameters
        ///
        /// - `origin`: The origin of the transaction.
        /// - `account_id`: The account ID to be removed from the set of risk managers.
        #[pallet::call_index(14)]
        #[pallet::weight(<T as Config>::WeightInfo::remove_risk_manager())]
        pub fn remove_risk_manager(
            origin: OriginFor<T>,
            account_id: T::AccountId,
        ) -> DispatchResult {
            ensure_root(origin)?;
            RiskManagers::<T>::mutate(|option_risk_managers| match option_risk_managers {
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

        /// It is allowed to call accrue() and liquidate() only if it fulfills conditions.
        fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
            if !Self::check_liquidation_available() {
                return InvalidTransaction::Custom(VALIDATION_ERROR_LIQUIDATION_LIMIT).into();
            }
            match call {
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
        /// Ensures that `who` is a risk manager.
        /// Risk manager can set protocol risk parameters.
        fn ensure_risk_manager(who: &AccountIdOf<T>) -> DispatchResult {
            if !Self::risk_managers().map_or(false, |risk_managers| risk_managers.contains(who)) {
                return Err(Error::<T>::OperationNotPermitted.into());
            }

            Ok(())
        }

        /// Ensures that `who` is a protocol owner.
        /// Protocol owner can withdraw profit from the protocol.
        fn ensure_protocol_owner(who: &AccountIdOf<T>) -> DispatchResult {
            if !Self::risk_managers().map_or(false, |risk_managers| risk_managers.contains(who)) {
                return Err(Error::<T>::OperationNotPermitted.into());
            }

            Ok(())
        }

        /// Checks if liquidation is available now.
        /// Returns `false` if liquidation took place this block since only one liquidation per
        /// block is allowed.
        fn check_liquidation_available() -> bool {
            !LiquidatedThisBlock::<T>::get()
        }

        /// Checks whether a Collateralized Debt Position (CDP) is currently considered safe based on its debt and collateral.
        /// The function evaluates the safety of a CDP based on predefined liquidation ratios and collateral values,
        /// providing an indication of its current safety status.
        ///
        /// ## Parameters
        ///
        /// - `debt`: The current debt amount in the CDP.
        /// - `collateral`: The current collateral amount in the CDP.
        /// - `collateral_asset_id`: The asset ID associated with the collateral in the CDP.
        pub(crate) fn get_max_safe_debt(
            collateral: Balance,
            collateral_asset_id: AssetIdOf<T>,
        ) -> Result<Balance, DispatchError> {
            let liquidation_ratio = Self::collateral_infos(collateral_asset_id)
                .ok_or(Error::<T>::CollateralInfoNotFound)?
                .risk_parameters
                .liquidation_ratio;
            // DAI is assumed as $1
            let collateral_reference_price =
                FixedU128::from_inner(T::PriceTools::get_average_price(
                    &collateral_asset_id,
                    &DAI.into(),
                    PriceVariant::Sell,
                )?);
            let collateral_volume = collateral_reference_price
                .checked_mul(&FixedU128::from_inner(collateral))
                .ok_or(Error::<T>::ArithmeticError)?;
            let res = FixedU128::from_perbill(liquidation_ratio)
                .checked_mul(&collateral_volume)
                .ok_or(Error::<T>::ArithmeticError)?;
            Ok(res.into_inner())
        }

        /// Checks whether a Collateralized Debt Position (CDP) is currently considered safe based on its debt and collateral.
        /// The function evaluates the safety of a CDP based on predefined liquidation ratios and collateral values,
        /// providing an indication of its current safety status.
        ///
        /// ## Parameters
        ///
        /// - `debt`: The current debt amount in the CDP.
        /// - `collateral`: The current collateral amount in the CDP.
        /// - `collateral_asset_id`: The asset ID associated with the collateral in the CDP.
        pub(crate) fn check_cdp_is_safe(
            debt: Balance,
            collateral: Balance,
            collateral_asset_id: AssetIdOf<T>,
        ) -> Result<bool, DispatchError> {
            if debt == Balance::zero() {
                Ok(true)
            } else {
                let max_safe_debt = Self::get_max_safe_debt(collateral, collateral_asset_id)?;
                Ok(debt <= max_safe_debt)
            }
        }

        /// Ensures that new emission will not exceed collateral hard cap
        fn ensure_collateral_cap(
            collateral_asset_id: AssetIdOf<T>,
            new_emission: Balance,
        ) -> DispatchResult {
            let collateral_info = Self::collateral_infos(collateral_asset_id)
                .ok_or(Error::<T>::CollateralInfoNotFound)?;
            let hard_cap = collateral_info.risk_parameters.hard_cap;
            ensure!(
                collateral_info
                    .kusd_supply
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

        /// Deposits collateral to CDP.
        /// Handles internal deposit of collateral into a Collateralized Debt Position (CDP).
        ///
        /// ## Parameters
        ///
        /// - `who`: The account making the collateral deposit.
        /// - `cdp_id`: The ID of the CDP where the collateral is being deposited.
        /// - `collateral_amount`: The amount of collateral being deposited.
        fn deposit_internal(
            who: &AccountIdOf<T>,
            cdp_id: CdpId,
            collateral_amount: Balance,
        ) -> DispatchResult {
            let cdp = Self::cdp(cdp_id).ok_or(Error::<T>::CDPNotFound)?;
            technical::Pallet::<T>::transfer_in(
                &cdp.collateral_asset_id,
                who,
                &T::TreasuryTechAccount::get(),
                collateral_amount,
            )?;
            Self::update_cdp_collateral(
                cdp_id,
                cdp.collateral_amount
                    .checked_add(collateral_amount)
                    .ok_or(Error::<T>::ArithmeticError)?,
            )?;
            Self::deposit_event(Event::CollateralDeposit {
                cdp_id,
                owner: who.clone(),
                collateral_asset_id: cdp.collateral_asset_id,
                amount: collateral_amount,
            });

            Ok(())
        }

        /// Handles the internal borrowing operation within a Collateralized Debt Position (CDP).
        /// Borrow amount will be as max as possible in the range
        /// `[borrow_amount_min, borrow_amount_max]` in order to confrom the slippage tolerance.
        /// ## Parameters
        ///
        /// - `who`: The account ID initiating the borrowing operation.
        /// - `cdp_id`: The ID of the CDP involved in the borrowing.
        /// - `will_to_borrow_amount`: The amount to be borrowed.
        fn borrow_internal(
            who: &AccountIdOf<T>,
            cdp_id: CdpId,
            borrow_amount_min: Balance,
            borrow_amount_max: Balance,
        ) -> DispatchResult {
            let cdp = Self::accrue_internal(cdp_id)?;
            ensure!(*who == cdp.owner, Error::<T>::OperationNotPermitted);
            let max_safe_debt =
                Self::get_max_safe_debt(cdp.collateral_amount, cdp.collateral_asset_id)?;
            let borrow_amount_safe_with_tax = max_safe_debt
                .checked_sub(cdp.debt)
                .ok_or(Error::<T>::ArithmeticError)?;

            let borrow_amount_safe = FixedU128::from_inner(borrow_amount_safe_with_tax)
                .checked_div(&(FixedU128::one() + FixedU128::from(Self::borrow_tax())))
                .ok_or(Error::<T>::ArithmeticError)?
                .into_inner();
            let borrow_tax_safe = borrow_amount_safe_with_tax
                .checked_sub(borrow_amount_safe)
                .ok_or(Error::<T>::ArithmeticError)?;

            let borrow_tax_min = Self::borrow_tax() * borrow_amount_min;
            let borrow_amount_min_with_tax = borrow_amount_min
                .checked_add(borrow_tax_min)
                .ok_or(Error::<T>::ArithmeticError)?;

            let borrow_tax_max = Self::borrow_tax() * borrow_amount_max;
            let borrow_amount_max_with_tax = borrow_amount_max
                .checked_add(borrow_tax_max)
                .ok_or(Error::<T>::ArithmeticError)?;
            ensure!(
                borrow_amount_min_with_tax <= borrow_amount_safe_with_tax,
                Error::<T>::CDPUnsafe
            );

            let (borrow_amount, borrow_tax, borrow_amount_with_tax) =
                if borrow_amount_max_with_tax <= borrow_amount_safe_with_tax {
                    (
                        borrow_amount_max,
                        borrow_tax_max,
                        borrow_amount_max_with_tax,
                    )
                } else {
                    (
                        borrow_amount_safe,
                        borrow_tax_safe,
                        borrow_amount_safe_with_tax,
                    )
                };

            // stablecoin minted is taxed by `borrow_tax` to buy back and burn KEN, the tax
            // increases debt
            Self::incentivize_ken_token(borrow_tax)?;
            let new_debt = cdp
                .debt
                .checked_add(borrow_amount_with_tax)
                .ok_or(Error::<T>::ArithmeticError)?;

            Self::ensure_collateral_cap(cdp.collateral_asset_id, borrow_amount_with_tax)?;
            Self::ensure_protocol_cap(borrow_amount_with_tax)?;
            Self::mint_to(who, borrow_amount)?;
            Self::update_cdp_debt(cdp_id, new_debt)?;
            CollateralInfos::<T>::try_mutate(cdp.collateral_asset_id, |collateral_info| {
                let collateral_info = collateral_info
                    .as_mut()
                    .ok_or(Error::<T>::CollateralInfoNotFound)?;
                collateral_info.kusd_supply = collateral_info
                    .kusd_supply
                    .checked_add(borrow_amount_with_tax)
                    .ok_or(Error::<T>::ArithmeticError)?;
                DispatchResult::Ok(())
            })?;
            Self::deposit_event(Event::DebtIncreased {
                cdp_id,
                owner: who.clone(),
                debt_asset_id: T::KusdAssetId::get(),
                amount: borrow_amount_with_tax,
            });

            Ok(())
        }

        /// Repays debt.
        /// Burns KUSD amount from CDP owner, updates CDP balances.
        ///
        /// ## Parameters
        ///
        /// - 'cdp_id' - CDP id
        /// - `amount` - The maximum amount to repay, if exceeds debt, the debt amount is repayed.
        fn repay_debt_internal(cdp_id: CdpId, amount: Balance) -> DispatchResult {
            let cdp = Self::accrue_internal(cdp_id)?;
            // if repaying amount exceeds debt, leftover is not burned
            let to_cover_debt = amount.min(cdp.debt);
            Self::burn_from(&cdp.owner, to_cover_debt)?;
            Self::update_cdp_debt(
                cdp_id,
                cdp.debt
                    .checked_sub(to_cover_debt)
                    .ok_or(Error::<T>::ArithmeticError)?,
            )?;
            Self::decrease_collateral_kusd_supply(&cdp.collateral_asset_id, to_cover_debt)?;
            Self::deposit_event(Event::DebtPayment {
                cdp_id,
                owner: cdp.owner,
                debt_asset_id: T::KusdAssetId::get(),
                amount: to_cover_debt,
            });

            Ok(())
        }

        /// Covers bad debt using a specified amount of stablecoin (KUSD).
        /// The function facilitates the covering of bad debt using stablecoin from a specific account,
        /// handling the transfer and burning of stablecoin as needed to cover the bad debt.
        ///
        /// ## Parameters
        ///
        /// - `from`: The account from which the stablecoin will be used to cover bad debt.
        /// - `kusd_amount`: The amount of stablecoin to cover bad debt.
        fn cover_bad_debt(from: &AccountIdOf<T>, kusd_amount: Balance) -> DispatchResult {
            let bad_debt = BadDebt::<T>::get();
            let to_cover_debt = if kusd_amount <= bad_debt {
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
            BadDebt::<T>::try_mutate(|bad_debt| {
                *bad_debt = bad_debt
                    .checked_sub(to_cover_debt)
                    .ok_or(Error::<T>::ArithmeticError)?;
                DispatchResult::Ok(())
            })?;

            Ok(())
        }

        /// Returns true if CDP has debt and uncollected stability fee is more than threshold.
        fn is_accruable(cdp_id: &CdpId) -> Result<bool, DispatchError> {
            let cdp = Self::cdp(cdp_id).ok_or(Error::<T>::CDPNotFound)?;
            if cdp.debt > 0 {
                let uncollected_stability_fee = Self::calculate_stability_fee(*cdp_id)?;
                Ok(uncollected_stability_fee >= T::MinimalStabilityFeeAccrue::get())
            } else {
                Ok(false)
            }
        }

        /// Recalculates collateral interest coefficient with the current timestamp
        ///
        /// Note:
        /// In the case of update this code do not forget to update front-end logic:
        /// `sora2-substrate-js-library/packages/util/src/kensetsu/index.ts`
        /// function `updateCollateralInterestCoefficient`
        fn calculate_collateral_interest_coefficient(
            collateral_asset_id: &AssetIdOf<T>,
        ) -> Result<CollateralInfo<T::Moment>, DispatchError> {
            let mut collateral_info = CollateralInfos::<T>::get(collateral_asset_id)
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
                collateral_info.interest_coefficient = FixedU128::from_inner(new_coefficient);
            }
            Ok(collateral_info)
        }

        /// Calculates stability fee for the CDP for the current time.
        ///
        /// Note:
        /// In the case of update this code do not forget to update front-end logic:
        /// `sora2-substrate-js-library/packages/util/src/kensetsu/index.ts`
        /// function `calcNewDebt`
        fn calculate_stability_fee(cdp_id: CdpId) -> Result<Balance, DispatchError> {
            let cdp = Self::cdp(cdp_id).ok_or(Error::<T>::CDPNotFound)?;
            let collateral_info =
                Self::calculate_collateral_interest_coefficient(&cdp.collateral_asset_id)?;
            let interest_coefficient = collateral_info.interest_coefficient;
            let interest_percent = interest_coefficient
                .checked_sub(&cdp.interest_coefficient)
                .ok_or(Error::<T>::ArithmeticError)?
                .checked_div(&cdp.interest_coefficient)
                .ok_or(Error::<T>::ArithmeticError)?;
            let stability_fee = FixedU128::from_inner(cdp.debt)
                .checked_mul(&interest_percent)
                .ok_or(Error::<T>::ArithmeticError)?
                .into_inner();
            Ok(stability_fee)
        }

        /// Accrues interest on a Collateralized Debt Position (CDP) and updates relevant parameters.
        ///
        /// ## Parameters
        ///
        /// - `cdp_id`: The ID of the CDP for interest accrual.
        fn accrue_internal(
            cdp_id: CdpId,
        ) -> Result<CollateralizedDebtPosition<AccountIdOf<T>, AssetIdOf<T>>, DispatchError>
        {
            let mut cdp = Self::cdp(cdp_id).ok_or(Error::<T>::CDPNotFound)?;
            let collateral_info =
                Self::calculate_collateral_interest_coefficient(&cdp.collateral_asset_id)?;
            let new_coefficient = collateral_info.interest_coefficient;
            let mut stability_fee = Self::calculate_stability_fee(cdp_id)?;
            let new_debt = cdp
                .debt
                .checked_add(stability_fee)
                .ok_or(Error::<T>::ArithmeticError)?;
            Self::increase_collateral_kusd_supply(&cdp.collateral_asset_id, stability_fee)?;
            cdp = CDPDepository::<T>::try_mutate(cdp_id, |cdp| {
                let cdp = cdp.as_mut().ok_or(Error::<T>::CDPNotFound)?;
                cdp.debt = new_debt;
                cdp.interest_coefficient = new_coefficient;
                Ok::<CollateralizedDebtPosition<T::AccountId, T::AssetId>, DispatchError>(
                    cdp.clone(),
                )
            })?;
            let mut new_bad_debt = BadDebt::<T>::get();
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
                BadDebt::<T>::try_mutate(|bad_debt| {
                    *bad_debt = new_bad_debt;
                    DispatchResult::Ok(())
                })?;
            }
            Self::mint_treasury(&T::KusdAssetId::get(), stability_fee)?;

            Ok(cdp)
        }

        /// Mint token to protocol technical account
        fn mint_treasury(asset_id: &T::AssetId, amount: Balance) -> DispatchResult {
            technical::Pallet::<T>::mint(asset_id, &T::TreasuryTechAccount::get(), amount)?;
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

        /// Burns a specified amount of an asset from an account.
        ///
        /// ## Parameters
        ///
        /// - `account`: The account from which the asset will be burnt.
        /// - `amount`: The amount of the asset to be burnt.
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

        /// Swaps collateral for KUSD
        /// ## Returns
        /// - sold - collateral sold (in swap amount)
        /// - proceeds - KUSD got from swap (out amount) minus liquidation penalty
        /// - penalty - liquidation penalty
        fn swap(
            cdp: &CollateralizedDebtPosition<AccountIdOf<T>, AssetIdOf<T>>,
            technical_account_id: &AccountIdOf<T>,
        ) -> Result<(Balance, Balance, Balance), DispatchError> {
            let risk_parameters = Self::collateral_infos(cdp.collateral_asset_id)
                .ok_or(Error::<T>::CollateralInfoNotFound)?
                .risk_parameters;
            let collateral_to_liquidate = cdp
                .collateral_amount
                .min(risk_parameters.max_liquidation_lot);
            ensure!(collateral_to_liquidate > 0, Error::<T>::ZeroLiquidationLot);

            // With quote before exchange we are sure that it will not result in infinite amount in for exchange and
            // there is enough liquidity for swap.
            let SwapOutcome { amount, .. } = T::LiquidityProxy::quote(
                DEXId::Polkaswap.into(),
                &cdp.collateral_asset_id,
                &T::KusdAssetId::get(),
                QuoteAmount::WithDesiredInput {
                    desired_amount_in: collateral_to_liquidate,
                },
                LiquiditySourceFilter::empty(DEXId::Polkaswap.into()),
                true,
            )?;
            let desired_kusd_amount = cdp
                .debt
                .checked_add(Self::liquidation_penalty() * cdp.debt)
                .ok_or(Error::<T>::ArithmeticError)?;
            let swap_amount = if amount > desired_kusd_amount {
                SwapAmount::with_desired_output(desired_kusd_amount, collateral_to_liquidate)
            } else {
                SwapAmount::with_desired_input(collateral_to_liquidate, Balance::zero())
            };

            // Since there is an issue with LiquidityProxy exchange amount that may differ from
            // requested one, we check balances here.
            let treasury_account_id = technical::Pallet::<T>::tech_account_id_to_account_id(
                &T::TreasuryTechAccount::get(),
            )?;
            let kusd_balance_before =
                T::AssetInfoProvider::free_balance(&T::KusdAssetId::get(), &treasury_account_id)?;
            let collateral_balance_before =
                T::AssetInfoProvider::free_balance(&cdp.collateral_asset_id, &treasury_account_id)?;

            T::LiquidityProxy::exchange(
                DEXId::Polkaswap.into(),
                technical_account_id,
                technical_account_id,
                &cdp.collateral_asset_id,
                &T::KusdAssetId::get(),
                swap_amount,
                LiquiditySourceFilter::empty(DEXId::Polkaswap.into()),
            )?;

            let kusd_balance_after =
                T::AssetInfoProvider::free_balance(&T::KusdAssetId::get(), &treasury_account_id)?;
            let collateral_balance_after =
                T::AssetInfoProvider::free_balance(&cdp.collateral_asset_id, &treasury_account_id)?;
            // This value may differ from `desired_kusd_amount`, so this is calculation of actual
            // amount swapped.
            let kusd_swapped = kusd_balance_after
                .checked_sub(kusd_balance_before)
                .ok_or(Error::<T>::ArithmeticError)?;
            let collateral_liquidated = collateral_balance_before
                .checked_sub(collateral_balance_after)
                .ok_or(Error::<T>::ArithmeticError)?;

            // penalty is a protocol profit which stays on treasury tech account
            let penalty = Self::liquidation_penalty() * kusd_swapped.min(cdp.debt);
            let proceeds = kusd_swapped - penalty;
            Ok((collateral_liquidated, proceeds, penalty))
        }

        /// Buys back KEN token with stablecoin and burns. Then 80% of burned is reminted for
        /// incentivization with Demeter farming for XOR/KUSD liquidity providers.
        ///
        /// ## Parameters
        /// - borrow_tax_kusd - borrow tax from borrowing amount.
        fn incentivize_ken_token(borrow_tax_kusd: Balance) -> DispatchResult {
            if borrow_tax_kusd > 0 {
                Self::mint_treasury(&T::KusdAssetId::get(), borrow_tax_kusd)?;
                let technical_account_id = technical::Pallet::<T>::tech_account_id_to_account_id(
                    &T::TreasuryTechAccount::get(),
                )?;
                let swap_outcome = T::LiquidityProxy::exchange(
                    DEXId::Polkaswap.into(),
                    &technical_account_id,
                    &technical_account_id,
                    &T::KusdAssetId::get(),
                    &T::KenAssetId::get(),
                    SwapAmount::with_desired_input(borrow_tax_kusd, balance!(0)),
                    LiquiditySourceFilter::empty(DEXId::Polkaswap.into()),
                )?;
                assets::Pallet::<T>::burn_from(
                    &T::KenAssetId::get(),
                    &technical_account_id,
                    &technical_account_id,
                    swap_outcome.amount,
                )?;
                let to_remint = T::KenIncentiveRemintPercent::get() * swap_outcome.amount;
                Self::mint_treasury(&T::KenAssetId::get(), to_remint)?;
            }

            Ok(())
        }

        /// Cover CDP debt with protocol balance
        /// If protocol balance is less than amount to cover, it is a bad debt
        fn cover_with_protocol(amount: Balance) -> DispatchResult {
            let treasury_account_id = technical::Pallet::<T>::tech_account_id_to_account_id(
                &T::TreasuryTechAccount::get(),
            )?;
            let protocol_positive_balance =
                T::AssetInfoProvider::free_balance(&T::KusdAssetId::get(), &treasury_account_id)?;
            let to_burn = if amount <= protocol_positive_balance {
                amount
            } else {
                BadDebt::<T>::try_mutate(|bad_debt| {
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

        /// Increments CDP Id counter, changes storage state.
        fn increment_cdp_id() -> Result<CdpId, DispatchError> {
            NextCDPId::<T>::try_mutate(|cdp_id| {
                *cdp_id = cdp_id.checked_add(1).ok_or(Error::<T>::ArithmeticError)?;
                Ok(*cdp_id)
            })
        }

        /// Inserts a new CDP
        /// Updates CDP storage and updates index owner -> CDP
        fn insert_cdp(
            owner: &AccountIdOf<T>,
            cdp_id: CdpId,
            cdp: CollateralizedDebtPosition<AccountIdOf<T>, AssetIdOf<T>>,
        ) -> DispatchResult {
            CDPDepository::<T>::insert(cdp_id, cdp);
            CdpOwnerIndex::<T>::try_append(owner, cdp_id)
                .map_err(|_| Error::<T>::CDPLimitPerUser.into())
        }

        /// Updates CDP collateral balance
        fn update_cdp_collateral(cdp_id: CdpId, collateral_amount: Balance) -> DispatchResult {
            CDPDepository::<T>::try_mutate(cdp_id, |cdp| {
                let cdp = cdp.as_mut().ok_or(Error::<T>::CDPNotFound)?;
                cdp.collateral_amount = collateral_amount;
                Ok(())
            })
        }

        /// Updates CDP debt balance
        fn update_cdp_debt(cdp_id: CdpId, debt: Balance) -> DispatchResult {
            CDPDepository::<T>::try_mutate(cdp_id, |cdp| {
                let cdp = cdp.as_mut().ok_or(Error::<T>::CDPNotFound)?;
                cdp.debt = debt;
                Ok(())
            })
        }

        /// Removes CDP entry from the storage and sends collateral to the owner.
        fn delete_cdp(cdp_id: CdpId) -> DispatchResult {
            let cdp = CDPDepository::<T>::take(cdp_id).ok_or(Error::<T>::CDPNotFound)?;
            let transfer_out = cdp.collateral_amount;
            technical::Pallet::<T>::transfer_out(
                &cdp.collateral_asset_id,
                &T::TreasuryTechAccount::get(),
                &cdp.owner,
                transfer_out,
            )?;
            if let Some(mut cdp_ids) = CdpOwnerIndex::<T>::take(&cdp.owner) {
                cdp_ids.retain(|&x| x != cdp_id);
                if !cdp_ids.is_empty() {
                    CdpOwnerIndex::<T>::insert(&cdp.owner, cdp_ids);
                }
            }
            Self::deposit_event(Event::CDPClosed {
                cdp_id,
                owner: cdp.owner,
                collateral_asset_id: cdp.collateral_asset_id,
                collateral_amount: transfer_out,
            });
            Ok(())
        }

        /// Increases tracker of KUSD supply for collateral asset
        fn increase_collateral_kusd_supply(
            collateral_asset_id: &AssetIdOf<T>,
            additional_kusd_supply: Balance,
        ) -> DispatchResult {
            CollateralInfos::<T>::try_mutate(collateral_asset_id, |collateral_info| {
                let collateral_info = collateral_info
                    .as_mut()
                    .ok_or(Error::<T>::CollateralInfoNotFound)?;
                collateral_info.kusd_supply = collateral_info
                    .kusd_supply
                    .checked_add(additional_kusd_supply)
                    .ok_or(Error::<T>::ArithmeticError)?;
                Ok(())
            })
        }

        /// Decreases tracker of KUSD supply for collateral asset
        fn decrease_collateral_kusd_supply(
            collateral_asset_id: &AssetIdOf<T>,
            seized_kusd_supply: Balance,
        ) -> DispatchResult {
            CollateralInfos::<T>::try_mutate(collateral_asset_id, |collateral_info| {
                let collateral_info = collateral_info
                    .as_mut()
                    .ok_or(Error::<T>::CollateralInfoNotFound)?;
                collateral_info.kusd_supply = collateral_info
                    .kusd_supply
                    .checked_sub(seized_kusd_supply)
                    .ok_or(Error::<T>::ArithmeticError)?;
                Ok(())
            })
        }

        /// Inserts or updates `CollateralRiskParameters` for collateral asset id.
        /// If `CollateralRiskParameters` exists for asset id, then updates them.
        /// Else if `CollateralRiskParameters` does not exist, inserts a new value.
        fn upsert_collateral_info(
            collateral_asset_id: &AssetIdOf<T>,
            new_risk_parameters: CollateralRiskParameters,
        ) -> DispatchResult {
            CollateralInfos::<T>::try_mutate(collateral_asset_id, |option_collateral_info| {
                match option_collateral_info {
                    Some(collateral_info) => {
                        let mut new_info =
                            Self::calculate_collateral_interest_coefficient(collateral_asset_id)?;
                        new_info.risk_parameters = new_risk_parameters;
                        *collateral_info = new_info;
                    }
                    None => {
                        let _ = option_collateral_info.insert(CollateralInfo {
                            risk_parameters: new_risk_parameters,
                            kusd_supply: balance!(0),
                            last_fee_update_time: Timestamp::<T>::get(),
                            interest_coefficient: FixedU128::one(),
                        });
                    }
                }
                Ok(())
            })
        }

        /// Returns CDP ids where the account id is owner
        pub fn get_account_cdp_ids(
            account_id: &AccountIdOf<T>,
        ) -> Result<Vec<CdpId>, DispatchError> {
            Ok(CDPDepository::<T>::iter()
                .filter(|(_, cdp)| cdp.owner == *account_id)
                .map(|(cdp_id, _)| cdp_id)
                .collect())
        }
    }
}
