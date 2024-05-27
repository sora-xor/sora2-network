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

//! Kensetsu is an over collateralized lending protocol, clone of MakerDAO.
//! An individual can create a collateral debt positions (CDPs) for one of the listed token and
//! deposit or lock amount of the token in CDP as collateral. Then the individual is allowed to
//! borrow new minted stablecoins pegged to oracle price in amount up to value of collateral
//! corrected by `liquidation_ratio` coefficient. The debt in stablecoins is a subject of
//! `stability_fee` interest rate. Collateral may be unlocked only when the debt and the interest
//! are paid back. If the value of collateral has changed in a way that it does not secure the debt,
//! the collateral is liquidated to cover the debt and the interest.

pub use pallet::*;

use codec::{Decode, Encode, MaxEncodedLen};
use common::{balance, AssetIdOf, AssetManager, Balance, DataFeed, Rate, SymbolName};
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
pub mod migrations;
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

/// Staiblecoin may be pegged either to Oracle (like XAU, BTC) or Price tools AssetId (like XOR,
/// DAI).
#[derive(Debug, Clone, Encode, Decode, TypeInfo, PartialEq)]
pub enum PegAsset<AssetId> {
    OracleSymbol(SymbolName),
    SoraAssetId(AssetId),
}

/// Parameters of the tokens created by the protocol.
#[derive(Debug, Clone, Encode, Decode, TypeInfo, PartialEq)]
pub struct StablecoinParameters<AssetId> {
    /// Peg of stablecoin.
    pub peg_asset: PegAsset<AssetId>,

    /// Minimal uncollected fee in stablecoins that triggers offchain worker to call accrue.
    pub minimal_stability_fee_accrue: Balance,
}

/// Parameters and additional variables related to stablecoins.
#[derive(Debug, Clone, Encode, Decode, TypeInfo)]
pub struct StablecoinInfo<AssetId> {
    /// System bad debt, the amount of stablecoins not secured with collateral.
    pub bad_debt: Balance,

    /// Configurable parameters
    pub stablecoin_parameters: StablecoinParameters<AssetId>,
}

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
    Debug,
    Default,
    Clone,
    Encode,
    Decode,
    MaxEncodedLen,
    TypeInfo,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Copy,
)]
pub struct CollateralRiskParameters {
    /// Hard cap of total stablecoins issued for the collateral.
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

    /// Total collateral locked in all CDPs
    pub total_collateral: Balance,

    /// Amount of stablecoins issued for the collateral
    pub stablecoin_supply: Balance,

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

    // Debt asset id
    pub stablecoin_asset_id: AssetId,

    /// Normalized outstanding debt in stablecoins.
    pub debt: Balance,

    /// Interest accrued for CDP.
    /// Initializes on creation with collateral interest coefficient equal to 1.
    /// The coefficient is growing over time with interest rate.
    /// Actual interest is: `(collateral.coefficient - cdp.coefficient) / cdp.coefficient`
    pub interest_coefficient: FixedU128,
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use crate::compounding::compound;
    use crate::weights::WeightInfo;
    use common::prelude::{QuoteAmount, SwapAmount, SwapOutcome};
    use common::{
        AccountIdOf, AssetId32, AssetInfoProvider, AssetName, AssetSymbol, BalancePrecision,
        ContentSource, DEXId, Description, LiquidityProxyTrait, LiquiditySourceFilter,
        LiquiditySourceType, PriceToolsProvider, PriceVariant, TradingPairSourceManager, DAI,
        DEFAULT_BALANCE_PRECISION, KXOR, XOR,
    };
    use frame_support::pallet_prelude::*;
    use frame_support::traits::Randomness;
    use frame_system::offchain::{SendTransactionTypes, SubmitTransaction};
    use frame_system::pallet_prelude::*;
    use pallet_timestamp as timestamp;
    use sp_arithmetic::traits::{CheckedDiv, CheckedMul, CheckedSub};
    use sp_arithmetic::Percent;
    use sp_core::bounded::BoundedVec;
    use sp_runtime::traits::{CheckedConversion, One, Zero};
    use sp_std::collections::vec_deque::VecDeque;
    use sp_std::vec::Vec;

    /// CDP id type
    pub type CdpId = u128;

    /// The current storage version.
    const STORAGE_VERSION: StorageVersion = StorageVersion::new(2);

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
                        debug!(
                            "Failed in offchain_worker send accrue(cdp_id: {:?}): {:?}",
                            cdp_id, err
                        );
                    }
                }

                // Liquidation
                match Self::check_cdp_is_safe(&cdp) {
                    Ok(true) => {}
                    Ok(false) => {
                        debug!("CDP {:?} unsafe", cdp_id);
                        unsafe_cdp_ids.push_back(cdp_id);
                    }
                    Err(err) => {
                        debug!(
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
        frame_system::Config
        + technical::Config
        + timestamp::Config
        + SendTransactionTypes<Call<Self>>
    {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type Randomness: Randomness<Option<Self::Hash>, Self::BlockNumber>;
        type AssetInfoProvider: AssetInfoProvider<
            AssetIdOf<Self>,
            Self::AccountId,
            AssetSymbol,
            AssetName,
            BalancePrecision,
            ContentSource,
            Description,
        >;
        type PriceTools: PriceToolsProvider<AssetIdOf<Self>>;
        type LiquidityProxy: LiquidityProxyTrait<Self::DEXId, Self::AccountId, AssetIdOf<Self>>;
        type Oracle: DataFeed<SymbolName, Rate, u64>;
        type TradingPairSourceManager: TradingPairSourceManager<Self::DEXId, AssetIdOf<Self>>;
        type TreasuryTechAccount: Get<Self::TechAccountId>;
        type KenAssetId: Get<AssetIdOf<Self>>;
        type KarmaAssetId: Get<AssetIdOf<Self>>;
        type TbcdAssetId: Get<AssetIdOf<Self>>;

        /// Percent of KEN buy back that is reminted and goes to Demeter farming incentivization.
        #[pallet::constant]
        type KenIncentiveRemintPercent: Get<Percent>;

        /// Percent of KARMA buy back that is reminted and goes to Demeter farming incentivization.
        #[pallet::constant]
        type KarmaIncentiveRemintPercent: Get<Percent>;

        /// Maximum number of CDP that one user can create
        #[pallet::constant]
        type MaxCdpsPerOwner: Get<u32>;

        /// Minimal uncollected fee in KUSD that triggers offchain worker to call accrue.
        #[pallet::constant]
        type MinimalStabilityFeeAccrue: Get<Balance>;

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

    /// Stablecoin parameters
    #[pallet::storage]
    #[pallet::getter(fn stablecoin_infos)]
    #[pallet::unbounded]
    pub type StablecoinInfos<T: Config> =
        StorageMap<_, Identity, AssetIdOf<T>, StablecoinInfo<AssetIdOf<T>>>;

    /// Parameters for collaterals, include risk parameters and interest recalculation coefficients.
    /// Map (Collateral asset id, Stablecoin asset id => CollateralInfo)
    #[pallet::storage]
    #[pallet::getter(fn collateral_infos)]
    pub type CollateralInfos<T: Config> = StorageDoubleMap<
        _,
        Identity,
        AssetIdOf<T>,
        Identity,
        AssetIdOf<T>,
        CollateralInfo<T::Moment>,
    >;

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

    /// Configuration parameters of predefined assets. Populates storage StablecoinInfos with
    /// predefined assets on initialization. Contains list of:
    /// - predefined asset id;
    /// - peg asset id;
    /// - minimal stability fee accrue.
    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub predefined_stablecoin_infos: Vec<(AssetIdOf<T>, AssetIdOf<T>, Balance)>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                predefined_stablecoin_infos: Default::default(),
            }
        }
    }

    /// Populates StablecoinInfos with passed parameters. Used for populating with predefined
    /// stable assets.
    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            self.predefined_stablecoin_infos.iter().cloned().for_each(
                |(predefined_asset_id, peg_asset_id, minimal_stability_fee_accrue)| {
                    StablecoinInfos::<T>::insert(
                        predefined_asset_id,
                        StablecoinInfo {
                            bad_debt: Balance::zero(),
                            stablecoin_parameters: StablecoinParameters {
                                peg_asset: PegAsset::SoraAssetId(peg_asset_id),
                                minimal_stability_fee_accrue,
                            },
                        },
                    );
                },
            )
        }
    }

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
            // stablecoin amount paid off
            amount: Balance,
        },
        Liquidated {
            cdp_id: CdpId,
            // what was liquidated
            collateral_asset_id: AssetIdOf<T>,
            collateral_amount: Balance,
            debt_asset_id: AssetIdOf<T>,
            // stablecoin amount from liquidation to cover debt
            proceeds: Balance,
            // liquidation penalty
            penalty: Balance,
        },
        CollateralRiskParametersUpdated {
            collateral_asset_id: AssetIdOf<T>,
            risk_parameters: CollateralRiskParameters,
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
        StablecoinRegistered {
            stablecoin_asset_id: AssetIdOf<T>,
            new_stablecoin_parameters: StablecoinParameters<AssetIdOf<T>>,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        ArithmeticError,
        SymbolNotEnabledByOracle,
        WrongAssetId,
        CDPNotFound,
        CollateralInfoNotFound,
        CollateralBelowMinimal,
        CDPSafe,
        CDPUnsafe,
        /// Too many CDPs per user
        CDPLimitPerUser,
        StablecoinInfoNotFound,
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
        /// Collateral must be registered in PriceTools.
        CollateralNotRegisteredInPriceTools,
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
            stablecoin_asset_id: AssetIdOf<T>,
            borrow_amount_min: Balance,
            borrow_amount_max: Balance,
            _cdp_type: CdpType,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(
                borrow_amount_min <= borrow_amount_max,
                Error::<T>::WrongBorrowAmounts
            );

            // checks minimal collateral deposit requirement
            let collateral_info = Self::collateral_infos(collateral_asset_id, stablecoin_asset_id)
                .ok_or(Error::<T>::CollateralInfoNotFound)?;
            ensure!(
                collateral_amount >= collateral_info.risk_parameters.minimal_collateral_deposit,
                Error::<T>::CollateralBelowMinimal
            );

            let cdp_id = Self::insert_cdp(CollateralizedDebtPosition {
                owner: who.clone(),
                collateral_asset_id,
                collateral_amount: balance!(0),
                stablecoin_asset_id,
                debt: balance!(0),
                interest_coefficient: collateral_info.interest_coefficient,
            })?;
            Self::deposit_event(Event::CDPCreated {
                cdp_id,
                owner: who.clone(),
                collateral_asset_id,
                debt_asset_id: stablecoin_asset_id,
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
        ///  will be transferred.
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::close_cdp())]
        pub fn close_cdp(origin: OriginFor<T>, cdp_id: CdpId) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let cdp = Self::get_cdp_updated(cdp_id)?;
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
            let who = ensure_signed(origin)?;
            let cdp = Self::get_cdp_updated(cdp_id)?;
            ensure!(who == cdp.owner, Error::<T>::OperationNotPermitted);
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
            // only one liquidation per block
            ensure!(
                Self::check_liquidation_available(),
                Error::<T>::LiquidationLimit
            );

            let cdp = Self::get_cdp_updated(cdp_id)?;
            ensure!(!Self::check_cdp_is_safe(&cdp)?, Error::<T>::CDPSafe);
            let (collateral_liquidated, proceeds, penalty) =
                Self::liquidate_internal(cdp_id, &cdp)?;

            Self::deposit_event(Event::Liquidated {
                cdp_id,
                collateral_asset_id: cdp.collateral_asset_id,
                collateral_amount: collateral_liquidated,
                debt_asset_id: cdp.stablecoin_asset_id,
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
            Self::get_cdp_updated(cdp_id)?;
            Ok(())
        }

        /// Updates the risk parameters for a specific collateral asset.
        ///
        /// ## Parameters
        ///
        /// - `origin`: The origin of the transaction.
        /// - `collateral_asset_id`: The identifier of the collateral asset. If collateral asset id
        /// is not tracked by PriceTools, registers the asset id in PriceTools.
        /// - `new_risk_parameters`: The new risk parameters to be set for the collateral asset.
        #[pallet::call_index(7)]
        #[pallet::weight(<T as Config>::WeightInfo::update_collateral_risk_parameters())]
        pub fn update_collateral_risk_parameters(
            origin: OriginFor<T>,
            collateral_asset_id: AssetIdOf<T>,
            stablecoin_asset_id: AssetIdOf<T>,
            new_risk_parameters: CollateralRiskParameters,
        ) -> DispatchResult {
            ensure_root(origin)?;
            if !T::PriceTools::is_asset_registered(&collateral_asset_id) {
                T::PriceTools::register_asset(&collateral_asset_id)?;
            }
            Self::upsert_collateral_info(
                &collateral_asset_id,
                &stablecoin_asset_id,
                new_risk_parameters,
            )?;
            Self::deposit_event(Event::CollateralRiskParametersUpdated {
                collateral_asset_id,
                risk_parameters: new_risk_parameters,
            });

            Ok(())
        }

        /// Updates the borrow tax applied during borrow.
        ///
        /// ## Parameters
        ///
        /// - `origin`: The origin of the transaction.
        /// - `new_borrow_tax`: The new borrow tax percentage to be set.
        #[pallet::call_index(8)]
        #[pallet::weight(<T as Config>::WeightInfo::update_borrow_tax())]
        pub fn update_borrow_tax(origin: OriginFor<T>, new_borrow_tax: Percent) -> DispatchResult {
            ensure_root(origin)?;
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
        #[pallet::call_index(9)]
        #[pallet::weight(<T as Config>::WeightInfo::update_liquidation_penalty())]
        pub fn update_liquidation_penalty(
            origin: OriginFor<T>,
            new_liquidation_penalty: Percent,
        ) -> DispatchResult {
            ensure_root(origin)?;
            let old_liquidation_penalty = LiquidationPenalty::<T>::get();
            LiquidationPenalty::<T>::set(new_liquidation_penalty);
            Self::deposit_event(Event::LiquidationPenaltyUpdated {
                new_liquidation_penalty,
                old_liquidation_penalty,
            });

            Ok(())
        }

        /// Withdraws protocol profit in the form of stablecoin.
        ///
        /// ## Parameters
        ///
        /// - `origin`: The origin of the transaction.
        /// - `beneficiary` : The destination account where assets will be withdrawn.
        /// - `stablecoin_asset_id` - The asset id of stablecoin.
        /// - `amount`: The amount of stablecoin to withdraw as protocol profit.
        #[pallet::call_index(10)]
        #[pallet::weight(<T as Config>::WeightInfo::withdraw_profit())]
        pub fn withdraw_profit(
            origin: OriginFor<T>,
            beneficiary: T::AccountId,
            stablecoin_asset_id: AssetIdOf<T>,
            amount: Balance,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(
                stablecoin_asset_id == T::KenAssetId::get()
                    || StablecoinInfos::<T>::contains_key(stablecoin_asset_id),
                Error::<T>::WrongAssetId
            );
            technical::Pallet::<T>::transfer_out(
                &stablecoin_asset_id,
                &T::TreasuryTechAccount::get(),
                &beneficiary,
                amount,
            )?;
            Self::deposit_event(Event::ProfitWithdrawn {
                debt_asset_id: stablecoin_asset_id,
                amount,
            });

            Ok(())
        }

        /// Donates stablecoin to cover protocol bad debt.
        ///
        /// ## Parameters
        ///
        /// - `origin`: The origin of the transaction.
        /// - `stablecoin_asset_id` - The asset id of stablecoin.
        /// - `amount`: The amount of stablecoin to donate to cover bad debt.
        #[pallet::call_index(11)]
        #[pallet::weight(<T as Config>::WeightInfo::donate())]
        pub fn donate(
            origin: OriginFor<T>,
            stablecoin_asset_id: AssetIdOf<T>,
            amount: Balance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            technical::Pallet::<T>::transfer_in(
                &stablecoin_asset_id,
                &who,
                &T::TreasuryTechAccount::get(),
                amount,
            )?;
            Self::cover_bad_debt(&stablecoin_asset_id, amount)?;
            Self::deposit_event(Event::Donation {
                debt_asset_id: stablecoin_asset_id,
                amount,
            });

            Ok(())
        }

        /// Adds new stablecoin mutating StablecoinInfo.
        ///
        /// ##Parameters
        /// - stablecoin_asset_id - asset id of new stablecoin, must be mintable and total supply
        /// must be 0.
        /// - new_stablecoin_parameters - parameters for peg.
        #[pallet::call_index(12)]
        #[pallet::weight(<T as Config>::WeightInfo::register_stablecoin())]
        pub fn register_stablecoin(
            origin: OriginFor<T>,
            new_stablecoin_parameters: StablecoinParameters<AssetIdOf<T>>,
        ) -> DispatchResult {
            ensure_root(origin)?;

            let stable_asset_id = Self::register_asset_id(&new_stablecoin_parameters)?;
            Self::peg_stablecoin(&stable_asset_id, &new_stablecoin_parameters)?;
            Self::register_trading_pair(&stable_asset_id)?;

            Self::deposit_event(Event::StablecoinRegistered {
                stablecoin_asset_id: stable_asset_id,
                new_stablecoin_parameters,
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
                    let cdp = Self::get_cdp_updated(*cdp_id)
                        .map_err(|_| InvalidTransaction::Custom(VALIDATION_ERROR_CHECK_SAFE))?;
                    if !Self::check_cdp_is_safe(&cdp)
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
        /// Registers asset id for stablecoin.
        fn register_asset_id(
            stablecoin_parameters: &StablecoinParameters<AssetIdOf<T>>,
        ) -> Result<AssetIdOf<T>, DispatchError> {
            let (vec_symbol, stable_asset_id) = match &stablecoin_parameters.peg_asset {
                PegAsset::OracleSymbol(symbol) => {
                    let mut vec_symbol = symbol.clone().0;
                    vec_symbol.insert(0, b'K');
                    let stable_asset_id: AssetIdOf<T> =
                        AssetId32::<common::PredefinedAssetId>::from_kensetsu_oracle_peg_symbol(
                            &vec_symbol,
                        )
                        .into();
                    (vec_symbol, stable_asset_id)
                }
                PegAsset::SoraAssetId(peg_asset_id) => {
                    let (symbol, ..) =
                        <T as Config>::AssetInfoProvider::get_asset_info(peg_asset_id);
                    let mut vec_symbol = symbol.0;
                    vec_symbol.insert(0, b'K');
                    let stable_asset_id: AssetIdOf<T> =
                        AssetId32::<common::PredefinedAssetId>::from_kensetsu_sora_peg_symbol(
                            &vec_symbol,
                        )
                        .into();
                    (vec_symbol, stable_asset_id)
                }
            };

            let technical_account_id = technical::Pallet::<T>::tech_account_id_to_account_id(
                &T::TreasuryTechAccount::get(),
            )?;

            T::AssetManager::register_asset_id(
                technical_account_id,
                stable_asset_id,
                AssetSymbol(vec_symbol.clone()),
                AssetName(vec_symbol),
                DEFAULT_BALANCE_PRECISION,
                balance!(0),
                true,
                None,
                None,
            )?;

            Ok(stable_asset_id)
        }

        /// Adds stablecoin info.
        /// Stablecoin can be either symbol supported by Band Oracle or asset id supported by
        /// PriceTools.
        fn peg_stablecoin(
            stablecoin_asset_id: &AssetIdOf<T>,
            new_stablecoin_parameters: &StablecoinParameters<AssetIdOf<T>>,
        ) -> DispatchResult {
            match &new_stablecoin_parameters.peg_asset {
                PegAsset::OracleSymbol(symbol) => {
                    ensure!(
                        <T>::Oracle::list_enabled_symbols()?
                            .iter()
                            .any(|(supported_symbol, _)| { *supported_symbol == *symbol }),
                        Error::<T>::SymbolNotEnabledByOracle
                    );
                }
                PegAsset::SoraAssetId(asset_id) => {
                    ensure!(
                        <T as Config>::AssetInfoProvider::asset_exists(asset_id),
                        Error::<T>::WrongAssetId
                    );
                    // cannot be pegged to KEN or other stablecoin
                    ensure!(
                        *asset_id != T::KenAssetId::get()
                            || !StablecoinInfos::<T>::contains_key(stablecoin_asset_id),
                        Error::<T>::WrongAssetId
                    );
                }
            }

            StablecoinInfos::<T>::try_mutate(*stablecoin_asset_id, |option_stablecoin_info| {
                match option_stablecoin_info {
                    Some(stablecoin_info) => {
                        stablecoin_info.stablecoin_parameters = new_stablecoin_parameters.clone();
                    }
                    None => {
                        let _ = option_stablecoin_info.insert(StablecoinInfo {
                            bad_debt: balance!(0),
                            stablecoin_parameters: new_stablecoin_parameters.clone(),
                        });
                    }
                }
                Ok(())
            })
        }

        /// Registers trading pair
        fn register_trading_pair(asset_id: &AssetIdOf<T>) -> sp_runtime::DispatchResult {
            if T::TradingPairSourceManager::is_trading_pair_enabled(
                &DEXId::Polkaswap.into(),
                &XOR.into(),
                asset_id,
            )? {
                return Ok(());
            }

            T::TradingPairSourceManager::register_pair(
                DEXId::Polkaswap.into(),
                XOR.into(),
                *asset_id,
            )?;

            T::TradingPairSourceManager::enable_source_for_trading_pair(
                &DEXId::Polkaswap.into(),
                &XOR.into(),
                asset_id,
                LiquiditySourceType::XYKPool,
            )?;

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
        /// - `stablecoin_asset_id`: The asset ID associated with the debt in the CDP.
        fn get_max_safe_debt(
            collateral_asset_id: AssetIdOf<T>,
            collateral: Balance,
            stablecoin_asset_id: AssetIdOf<T>,
        ) -> Result<Balance, DispatchError> {
            let liquidation_ratio =
                Self::collateral_infos(collateral_asset_id, stablecoin_asset_id)
                    .ok_or(Error::<T>::CollateralInfoNotFound)?
                    .risk_parameters
                    .liquidation_ratio;

            // collateral price in pegged asset
            let peg_asset = Self::stablecoin_infos(stablecoin_asset_id)
                .ok_or(Error::<T>::StablecoinInfoNotFound)?
                .stablecoin_parameters
                .peg_asset;
            let collateral_reference_price = match peg_asset {
                PegAsset::OracleSymbol(symbol) => {
                    // collateral price in DAI assumed as price in $
                    let collateral_price_dai =
                        FixedU128::from_inner(T::PriceTools::get_average_price(
                            &collateral_asset_id,
                            &DAI.into(),
                            PriceVariant::Sell,
                        )?);
                    let stablecoin_price = FixedU128::from_inner(
                        <T>::Oracle::quote(&symbol)?
                            .ok_or(Error::<T>::SymbolNotEnabledByOracle)?
                            .value,
                    );
                    collateral_price_dai
                        .checked_div(&stablecoin_price)
                        .ok_or(Error::<T>::ArithmeticError)?
                }
                PegAsset::SoraAssetId(asset_id) => {
                    FixedU128::from_inner(T::PriceTools::get_average_price(
                        &collateral_asset_id,
                        &asset_id,
                        PriceVariant::Sell,
                    )?)
                }
            };
            let collateral_volume = collateral_reference_price
                .checked_mul(&FixedU128::from_inner(collateral))
                .ok_or(Error::<T>::ArithmeticError)?;
            let max_safe_debt = FixedU128::from_perbill(liquidation_ratio)
                .checked_mul(&collateral_volume)
                .ok_or(Error::<T>::ArithmeticError)?;
            Ok(max_safe_debt.into_inner())
        }

        /// Checks whether a Collateralized Debt Position (CDP) is currently considered safe based on its debt and collateral.
        /// The function evaluates the safety of a CDP based on predefined liquidation ratios and collateral values,
        /// providing an indication of its current safety status.
        ///
        /// ## Parameters
        ///
        /// - `cdp` - Collateralized Debt Position.
        fn check_cdp_is_safe(
            cdp: &CollateralizedDebtPosition<AccountIdOf<T>, AssetIdOf<T>>,
        ) -> Result<bool, DispatchError> {
            if cdp.debt == Balance::zero() {
                Ok(true)
            } else {
                let max_safe_debt = Self::get_max_safe_debt(
                    cdp.collateral_asset_id,
                    cdp.collateral_amount,
                    cdp.stablecoin_asset_id,
                )?;
                Ok(cdp.debt <= max_safe_debt)
            }
        }

        /// Ensures that new emission will not exceed collateral hard cap
        fn ensure_collateral_cap(
            collateral_asset_id: AssetIdOf<T>,
            stablecoin_asset_id: AssetIdOf<T>,
            new_emission: Balance,
        ) -> DispatchResult {
            let collateral_info = Self::collateral_infos(collateral_asset_id, stablecoin_asset_id)
                .ok_or(Error::<T>::CollateralInfoNotFound)?;
            let hard_cap = collateral_info.risk_parameters.hard_cap;
            ensure!(
                collateral_info
                    .stablecoin_supply
                    .checked_add(new_emission)
                    .ok_or(Error::<T>::ArithmeticError)?
                    <= hard_cap,
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

        /// Charges borrow taxes.
        /// Applies borrow tax of 1% on borrow to buy back and burn KEN.
        ///
        /// ## Parameters
        /// - `collateral_asset_id`
        /// - `stablecoin_asset_id`
        /// - `borrow_amount_min` - borrow amount with slippage tolerance
        /// - `borrow_amount_min` - borrow amount with slippage tolerance
        /// - `borrow_amount_safe_with_tax` - borrow amount limit
        fn charge_borrow_tax(
            collateral_asset_id: &AssetIdOf<T>,
            stablecoin_asset_id: &AssetIdOf<T>,
            borrow_amount_min: Balance,
            borrow_amount_max: Balance,
            borrow_amount_safe_with_tax: Balance,
        ) -> Result<(Balance, Balance), DispatchError> {
            struct BorrowTax<T: Config> {
                pub incentive_asset_id: AssetIdOf<T>,
                pub tax_percent: Percent,
                pub remint_percent: Percent,
            }

            let mut taxes: Vec<BorrowTax<T>> = Vec::new();
            taxes.push(BorrowTax {
                incentive_asset_id: T::KenAssetId::get(),
                tax_percent: Self::borrow_tax(),
                remint_percent: T::KenIncentiveRemintPercent::get(),
            });

            // charge 1% for $KEN buyback
            let mut total_borrow_tax_percent = Self::borrow_tax();

            // for XOR/KXOR cdps:
            // - 1% for KARMA buyback
            // - 1% for TBCD buyback
            if *collateral_asset_id == Into::<AssetIdOf<T>>::into(XOR)
                && *stablecoin_asset_id == Into::<AssetIdOf<T>>::into(KXOR)
            {
                taxes.push(BorrowTax {
                    incentive_asset_id: T::KarmaAssetId::get(),
                    tax_percent: Percent::from_percent(1),
                    remint_percent: T::KarmaIncentiveRemintPercent::get(),
                });
                taxes.push(BorrowTax {
                    incentive_asset_id: T::TbcdAssetId::get(),
                    tax_percent: Percent::from_percent(1),
                    remint_percent: Percent::zero(),
                });
                total_borrow_tax_percent = total_borrow_tax_percent + Percent::from_percent(2);
            }

            let borrow_amount_safe = FixedU128::from_inner(borrow_amount_safe_with_tax)
                .checked_div(&(FixedU128::one() + FixedU128::from(total_borrow_tax_percent)))
                .ok_or(Error::<T>::ArithmeticError)?
                .into_inner();

            let borrow_tax_min = total_borrow_tax_percent * borrow_amount_min;
            let borrow_amount_min_with_tax = borrow_amount_min
                .checked_add(borrow_tax_min)
                .ok_or(Error::<T>::ArithmeticError)?;

            let borrow_tax_max = total_borrow_tax_percent * borrow_amount_max;
            let borrow_amount_max_with_tax = borrow_amount_max
                .checked_add(borrow_tax_max)
                .ok_or(Error::<T>::ArithmeticError)?;
            ensure!(
                borrow_amount_min_with_tax <= borrow_amount_safe_with_tax,
                Error::<T>::CDPUnsafe
            );

            let (borrow_amount_with_tax, expected_borrow_amount) =
                if borrow_amount_max_with_tax <= borrow_amount_safe_with_tax {
                    (borrow_amount_max_with_tax, borrow_amount_max)
                } else {
                    (borrow_amount_safe_with_tax, borrow_amount_safe)
                };

            // borrow amount may differ from expected_borrow_amount due to rounding
            let mut final_borrow_amount = borrow_amount_with_tax;
            for tax in taxes {
                let borrow_tax = tax.tax_percent * expected_borrow_amount;
                final_borrow_amount = final_borrow_amount
                    .checked_sub(borrow_tax)
                    .ok_or(Error::<T>::ArithmeticError)?;
                Self::incentivize_token(
                    stablecoin_asset_id,
                    borrow_tax,
                    &tax.incentive_asset_id,
                    tax.remint_percent,
                )?;
            }

            Ok((borrow_amount_with_tax, final_borrow_amount))
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
            let cdp = Self::get_cdp_updated(cdp_id)?;
            ensure!(*who == cdp.owner, Error::<T>::OperationNotPermitted);
            let max_safe_debt = Self::get_max_safe_debt(
                cdp.collateral_asset_id,
                cdp.collateral_amount,
                cdp.stablecoin_asset_id,
            )?;
            let borrow_amount_safe_with_tax = max_safe_debt
                .checked_sub(cdp.debt)
                .ok_or(Error::<T>::ArithmeticError)?;
            let (borrow_amount_with_tax, borrow_amount) = Self::charge_borrow_tax(
                &cdp.collateral_asset_id,
                &cdp.stablecoin_asset_id,
                borrow_amount_min,
                borrow_amount_max,
                borrow_amount_safe_with_tax,
            )?;
            Self::ensure_collateral_cap(
                cdp.collateral_asset_id,
                cdp.stablecoin_asset_id,
                borrow_amount_with_tax,
            )?;
            Self::mint_to(who, &cdp.stablecoin_asset_id, borrow_amount)?;
            Self::increase_cdp_debt(cdp_id, borrow_amount_with_tax)?;
            Self::deposit_event(Event::DebtIncreased {
                cdp_id,
                owner: who.clone(),
                debt_asset_id: cdp.stablecoin_asset_id,
                amount: borrow_amount_with_tax,
            });

            Ok(())
        }

        /// Repays debt.
        /// Burns stablecoin amount from CDP owner, updates CDP balances.
        ///
        /// ## Parameters
        ///
        /// - 'cdp_id' - CDP id
        /// - `amount` - The maximum amount to repay, if exceeds debt, the debt amount is repayed.
        fn repay_debt_internal(cdp_id: CdpId, amount: Balance) -> DispatchResult {
            let cdp = Self::get_cdp_updated(cdp_id)?;
            // if repaying amount exceeds debt, leftover is not burned
            let to_cover_debt = amount.min(cdp.debt);
            Self::burn_from(&cdp.owner, &cdp.stablecoin_asset_id, to_cover_debt)?;
            Self::decrease_cdp_debt(cdp_id, to_cover_debt)?;
            Self::deposit_event(Event::DebtPayment {
                cdp_id,
                owner: cdp.owner,
                debt_asset_id: cdp.stablecoin_asset_id,
                amount: to_cover_debt,
            });

            Ok(())
        }

        /// Covers bad debt using a specified amount of stablecoin.
        /// The function facilitates the covering of bad debt using stablecoin from a specific account,
        /// handling the transfer and burning of stablecoin as needed to cover the bad debt.
        ///
        /// ## Parameters
        ///
        /// - `from`: The account from which the stablecoin will be used to cover bad debt.
        /// - `amount`: The amount of stablecoin to cover bad debt.
        fn cover_bad_debt(stablecoin_asset_id: &AssetIdOf<T>, amount: Balance) -> DispatchResult {
            let bad_debt = StablecoinInfos::<T>::get(stablecoin_asset_id)
                .ok_or(Error::<T>::StablecoinInfoNotFound)?
                .bad_debt;
            let bad_debt_change = bad_debt.min(amount);
            Self::burn_treasury(stablecoin_asset_id, bad_debt_change)?;
            StablecoinInfos::<T>::try_mutate(stablecoin_asset_id, |stablecoin_info| {
                let stablecoin_info = stablecoin_info
                    .as_mut()
                    .ok_or(Error::<T>::CollateralInfoNotFound)?;
                stablecoin_info.bad_debt = bad_debt
                    .checked_sub(bad_debt_change)
                    .ok_or(Error::<T>::ArithmeticError)?;
                DispatchResult::Ok(())
            })?;

            Ok(())
        }

        /// Returns true if CDP has debt and uncollected stability fee is more than threshold.
        fn is_accruable(cdp_id: &CdpId) -> Result<bool, DispatchError> {
            let cdp = Self::cdp(cdp_id).ok_or(Error::<T>::CDPNotFound)?;
            if cdp.debt > 0 {
                let (uncollected_stability_fee, _) = Self::calculate_stability_fee(*cdp_id)?;
                let minimal_accruable_fee = Self::stablecoin_infos(cdp.stablecoin_asset_id)
                    .ok_or(Error::<T>::StablecoinInfoNotFound)?
                    .stablecoin_parameters
                    .minimal_stability_fee_accrue;
                Ok(uncollected_stability_fee >= minimal_accruable_fee)
            } else {
                Ok(false)
            }
        }

        /// Recalculates collateral interest coefficient with the current timestamp.
        ///
        /// Note:
        /// In the case of update this code do not forget to update front-end logic:
        /// `sora2-substrate-js-library/packages/util/src/kensetsu/index.ts`
        /// function `updateCollateralInterestCoefficient`
        fn calculate_collateral_interest_coefficient(
            collateral_asset_id: &AssetIdOf<T>,
            stablecoin_asset_id: &AssetIdOf<T>,
        ) -> Result<CollateralInfo<T::Moment>, DispatchError> {
            let mut collateral_info =
                CollateralInfos::<T>::get(collateral_asset_id, stablecoin_asset_id)
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
        /// Returns:
        /// - `stability_fee`: Balance
        /// - `updated_interest_coefficient`: FixedU128
        ///
        /// Note:
        /// In the case of update this code do not forget to update front-end logic:
        /// `sora2-substrate-js-library/packages/util/src/kensetsu/index.ts`
        /// function `calcNewDebt`
        fn calculate_stability_fee(cdp_id: CdpId) -> Result<(Balance, FixedU128), DispatchError> {
            let cdp = Self::cdp(cdp_id).ok_or(Error::<T>::CDPNotFound)?;
            let collateral_info = Self::calculate_collateral_interest_coefficient(
                &cdp.collateral_asset_id,
                &cdp.stablecoin_asset_id,
            )?;
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
            Ok((stability_fee, interest_coefficient))
        }

        /// Updates Collateralized Debt Position (CDP) fields to the current time and saves storage:
        /// - debt,
        /// - interest coefficient
        ///
        /// ## Parameters
        /// - `cdp_id`: The ID of the CDP for interest accrual.
        ///
        /// ## Returns
        /// - updated cdp
        fn get_cdp_updated(
            cdp_id: CdpId,
        ) -> Result<CollateralizedDebtPosition<AccountIdOf<T>, AssetIdOf<T>>, DispatchError>
        {
            let (mut stability_fee, new_coefficient) = Self::calculate_stability_fee(cdp_id)?;
            let cdp = CDPDepository::<T>::try_mutate(cdp_id, |cdp| {
                let cdp = cdp.as_mut().ok_or(Error::<T>::CDPNotFound)?;
                cdp.debt = cdp
                    .debt
                    .checked_add(stability_fee)
                    .ok_or(Error::<T>::ArithmeticError)?;
                cdp.interest_coefficient = new_coefficient;
                Ok::<CollateralizedDebtPosition<T::AccountId, AssetIdOf<T>>, DispatchError>(
                    cdp.clone(),
                )
            })?;
            Self::increase_collateral_stablecoin_supply(
                &cdp.collateral_asset_id,
                &cdp.stablecoin_asset_id,
                stability_fee,
            )?;
            let mut new_bad_debt = StablecoinInfos::<T>::get(cdp.stablecoin_asset_id)
                .ok_or(Error::<T>::StablecoinInfoNotFound)?
                .bad_debt;
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
                StablecoinInfos::<T>::try_mutate(cdp.stablecoin_asset_id, |stablecoin_info| {
                    let stablecoin_info = stablecoin_info
                        .as_mut()
                        .ok_or(Error::<T>::CollateralInfoNotFound)?;
                    stablecoin_info.bad_debt = new_bad_debt;
                    DispatchResult::Ok(())
                })?;
            }
            Self::mint_treasury(&cdp.stablecoin_asset_id, stability_fee)?;

            Ok(cdp)
        }

        /// Mint token to protocol technical account
        fn mint_treasury(asset_id: &AssetIdOf<T>, amount: Balance) -> DispatchResult {
            technical::Pallet::<T>::mint(asset_id, &T::TreasuryTechAccount::get(), amount)?;
            Ok(())
        }

        /// Mint token to AccountId
        fn mint_to(
            account: &AccountIdOf<T>,
            stablecoin_asset_id: &AssetIdOf<T>,
            amount: Balance,
        ) -> DispatchResult {
            let technical_account_id = technical::Pallet::<T>::tech_account_id_to_account_id(
                &T::TreasuryTechAccount::get(),
            )?;
            T::AssetManager::mint_to(stablecoin_asset_id, &technical_account_id, account, amount)?;
            Ok(())
        }

        /// Burns tokens from treasury technical account
        fn burn_treasury(stablecoin_asset_id: &AssetIdOf<T>, to_burn: Balance) -> DispatchResult {
            let technical_account_id = technical::Pallet::<T>::tech_account_id_to_account_id(
                &T::TreasuryTechAccount::get(),
            )?;
            T::AssetManager::burn_from(
                stablecoin_asset_id,
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
        /// - `stablecoin_asset_id`: The asset id to be burnt.
        /// - `amount`: The amount of the asset to be burnt.
        fn burn_from(
            account: &AccountIdOf<T>,
            stablecoin_asset_id: &AssetIdOf<T>,
            amount: Balance,
        ) -> DispatchResult {
            let technical_account_id = technical::Pallet::<T>::tech_account_id_to_account_id(
                &T::TreasuryTechAccount::get(),
            )?;
            T::AssetManager::burn_from(
                stablecoin_asset_id,
                &technical_account_id,
                account,
                amount,
            )?;
            Ok(())
        }

        /// Swaps collateral for stablecoin
        /// ## Returns
        /// - sold - collateral sold (in swap amount)
        /// - proceeds - stablecoin got from swap (out amount) minus liquidation penalty
        /// - penalty - liquidation penalty
        fn liquidate_internal(
            cdp_id: CdpId,
            cdp: &CollateralizedDebtPosition<AccountIdOf<T>, AssetIdOf<T>>,
        ) -> Result<(Balance, Balance, Balance), DispatchError> {
            let risk_parameters =
                Self::collateral_infos(cdp.collateral_asset_id, cdp.stablecoin_asset_id)
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
                &cdp.stablecoin_asset_id,
                QuoteAmount::WithDesiredInput {
                    desired_amount_in: collateral_to_liquidate,
                },
                LiquiditySourceFilter::empty(DEXId::Polkaswap.into()),
                true,
            )?;
            let desired_amount = cdp
                .debt
                .checked_add(Self::liquidation_penalty() * cdp.debt)
                .ok_or(Error::<T>::ArithmeticError)?;
            let swap_amount = if amount > desired_amount {
                SwapAmount::with_desired_output(desired_amount, collateral_to_liquidate)
            } else {
                SwapAmount::with_desired_input(collateral_to_liquidate, Balance::zero())
            };

            // Since there is an issue with LiquidityProxy exchange amount that may differ from
            // requested one, we check balances here.
            let treasury_account_id = technical::Pallet::<T>::tech_account_id_to_account_id(
                &T::TreasuryTechAccount::get(),
            )?;
            let stablecoin_balance_before = <T as Config>::AssetInfoProvider::free_balance(
                &cdp.stablecoin_asset_id,
                &treasury_account_id,
            )?;
            let collateral_balance_before = <T as Config>::AssetInfoProvider::free_balance(
                &cdp.collateral_asset_id,
                &treasury_account_id,
            )?;

            let technical_account_id = technical::Pallet::<T>::tech_account_id_to_account_id(
                &T::TreasuryTechAccount::get(),
            )?;
            T::LiquidityProxy::exchange(
                DEXId::Polkaswap.into(),
                &technical_account_id,
                &technical_account_id,
                &cdp.collateral_asset_id,
                &cdp.stablecoin_asset_id,
                swap_amount,
                LiquiditySourceFilter::empty(DEXId::Polkaswap.into()),
            )?;

            let stablecoin_balance_after = <T as Config>::AssetInfoProvider::free_balance(
                &cdp.stablecoin_asset_id,
                &treasury_account_id,
            )?;
            let collateral_balance_after = <T as Config>::AssetInfoProvider::free_balance(
                &cdp.collateral_asset_id,
                &treasury_account_id,
            )?;
            // This value may differ from `desired_amount`, so this is calculation of actual
            // amount swapped.
            let stablecoin_swapped = stablecoin_balance_after
                .checked_sub(stablecoin_balance_before)
                .ok_or(Error::<T>::ArithmeticError)?;
            let collateral_liquidated = collateral_balance_before
                .checked_sub(collateral_balance_after)
                .ok_or(Error::<T>::ArithmeticError)?;

            // penalty is a protocol profit which stays on treasury tech account
            let penalty = Self::liquidation_penalty() * stablecoin_swapped.min(cdp.debt);
            Self::cover_bad_debt(&cdp.stablecoin_asset_id, penalty)?;
            let proceeds = stablecoin_swapped - penalty;
            Self::update_cdp_collateral(
                cdp_id,
                cdp.collateral_amount
                    .checked_sub(collateral_liquidated)
                    .ok_or(Error::<T>::ArithmeticError)?,
            )?;
            if cdp.debt > proceeds {
                Self::burn_treasury(&cdp.stablecoin_asset_id, proceeds)?;
                if cdp.collateral_amount <= collateral_liquidated {
                    // no collateral, total default
                    // CDP debt is not covered with liquidation, now it is a protocol bad debt
                    let shortage = cdp
                        .debt
                        .checked_sub(proceeds)
                        .ok_or(Error::<T>::ArithmeticError)?;
                    Self::cover_with_protocol(&cdp.stablecoin_asset_id, shortage)?;
                    // close empty CDP, debt == 0, collateral == 0
                    Self::decrease_cdp_debt(cdp_id, cdp.debt)?;
                    Self::delete_cdp(cdp_id)?;
                } else {
                    // partly covered
                    Self::decrease_cdp_debt(cdp_id, proceeds)?;
                }
            } else {
                Self::burn_treasury(&cdp.stablecoin_asset_id, cdp.debt)?;
                // CDP debt is covered
                Self::decrease_cdp_debt(cdp_id, cdp.debt)?;
                // There is more stablecoins than to cover debt and penalty, leftover goes to cdp.owner
                let leftover = proceeds
                    .checked_sub(cdp.debt)
                    .ok_or(Error::<T>::ArithmeticError)?;
                T::AssetManager::transfer_from(
                    &cdp.stablecoin_asset_id,
                    &technical_account_id,
                    &cdp.owner,
                    leftover,
                )?;
            };
            LiquidatedThisBlock::<T>::put(true);

            Ok((collateral_liquidated, proceeds, penalty))
        }

        /// Buys back token with stablecoin and burns. Then `remint_percent` of burned is reminted
        /// for incentivization with Demeter farming for liquidity providers.
        ///
        /// ## Parameters
        /// - stablecoin_asset_id - asset id of tax;
        /// - borrow_tax - borrow tax from borrowing amount in stablecoins;
        /// - incentive_asset_id - token to buy back;
        /// - remint_percent - remint after burn.
        fn incentivize_token(
            stablecoin_asset_id: &AssetIdOf<T>,
            borrow_tax: Balance,
            incentive_asset_id: &AssetIdOf<T>,
            remint_percent: Percent,
        ) -> DispatchResult {
            if borrow_tax > 0 {
                Self::mint_treasury(stablecoin_asset_id, borrow_tax)?;
                let technical_account_id = technical::Pallet::<T>::tech_account_id_to_account_id(
                    &T::TreasuryTechAccount::get(),
                )?;
                let swap_outcome = T::LiquidityProxy::exchange(
                    DEXId::Polkaswap.into(),
                    &technical_account_id,
                    &technical_account_id,
                    stablecoin_asset_id,
                    incentive_asset_id,
                    SwapAmount::with_desired_input(borrow_tax, balance!(0)),
                    LiquiditySourceFilter::empty(DEXId::Polkaswap.into()),
                )?;
                T::AssetManager::burn_from(
                    incentive_asset_id,
                    &technical_account_id,
                    &technical_account_id,
                    swap_outcome.amount,
                )?;
                let to_remint = remint_percent * swap_outcome.amount;
                Self::mint_treasury(incentive_asset_id, to_remint)?;
            }

            Ok(())
        }

        /// Cover CDP debt with protocol balance
        /// If protocol balance is less than amount to cover, it is a bad debt
        /// Returns amount burnt.
        fn cover_with_protocol(
            stablecoin_asset_id: &AssetIdOf<T>,
            amount: Balance,
        ) -> Result<Balance, DispatchError> {
            let treasury_account_id = technical::Pallet::<T>::tech_account_id_to_account_id(
                &T::TreasuryTechAccount::get(),
            )?;
            let protocol_positive_balance = <T as Config>::AssetInfoProvider::free_balance(
                stablecoin_asset_id,
                &treasury_account_id,
            )?;
            let to_burn = if amount <= protocol_positive_balance {
                amount
            } else {
                StablecoinInfos::<T>::try_mutate(stablecoin_asset_id, |stablecoin_info| {
                    let stablecoin_info = stablecoin_info
                        .as_mut()
                        .ok_or(Error::<T>::CollateralInfoNotFound)?;
                    stablecoin_info.bad_debt = stablecoin_info
                        .bad_debt
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
            Self::burn_treasury(stablecoin_asset_id, to_burn)?;

            Ok(to_burn)
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
            cdp: CollateralizedDebtPosition<AccountIdOf<T>, AssetIdOf<T>>,
        ) -> Result<CdpId, DispatchError> {
            let cdp_id = Self::increment_cdp_id()?;
            CdpOwnerIndex::<T>::try_append(&cdp.owner, cdp_id)
                .map_err(|_| Error::<T>::CDPLimitPerUser)?;
            CDPDepository::<T>::insert(cdp_id, cdp);
            Ok(cdp_id)
        }

        /// Updates CDP collateral balance
        fn update_cdp_collateral(cdp_id: CdpId, collateral_amount: Balance) -> DispatchResult {
            CDPDepository::<T>::try_mutate(cdp_id, |cdp| {
                let cdp = cdp.as_mut().ok_or(Error::<T>::CDPNotFound)?;
                let old_collateral = cdp.collateral_amount;
                CollateralInfos::<T>::try_mutate(
                    cdp.collateral_asset_id,
                    cdp.stablecoin_asset_id,
                    |collateral_info| {
                        let collateral_info = collateral_info
                            .as_mut()
                            .ok_or(Error::<T>::CollateralInfoNotFound)?;
                        collateral_info.total_collateral = collateral_info
                            .total_collateral
                            .checked_sub(old_collateral)
                            .ok_or(Error::<T>::ArithmeticError)?
                            .checked_add(collateral_amount)
                            .ok_or(Error::<T>::ArithmeticError)?;
                        Ok::<(), Error<T>>(())
                    },
                )?;
                cdp.collateral_amount = collateral_amount;
                Ok(())
            })
        }

        /// Updates CDP debt by increasing the value.
        fn increase_cdp_debt(cdp_id: CdpId, debt_change: Balance) -> DispatchResult {
            CDPDepository::<T>::try_mutate(cdp_id, |cdp| {
                let cdp = cdp.as_mut().ok_or(Error::<T>::CDPNotFound)?;
                cdp.debt = cdp
                    .debt
                    .checked_add(debt_change)
                    .ok_or(Error::<T>::ArithmeticError)?;
                Self::increase_collateral_stablecoin_supply(
                    &cdp.collateral_asset_id,
                    &cdp.stablecoin_asset_id,
                    debt_change,
                )
            })
        }

        /// Updates CDP debt by decreasing the value.
        fn decrease_cdp_debt(cdp_id: CdpId, debt_change: Balance) -> DispatchResult {
            CDPDepository::<T>::try_mutate(cdp_id, |cdp| {
                let cdp = cdp.as_mut().ok_or(Error::<T>::CDPNotFound)?;
                cdp.debt = cdp
                    .debt
                    .checked_sub(debt_change)
                    .ok_or(Error::<T>::ArithmeticError)?;
                CollateralInfos::<T>::try_mutate(
                    cdp.collateral_asset_id,
                    cdp.stablecoin_asset_id,
                    |collateral_info| {
                        let collateral_info = collateral_info
                            .as_mut()
                            .ok_or(Error::<T>::CollateralInfoNotFound)?;
                        collateral_info.stablecoin_supply = collateral_info
                            .stablecoin_supply
                            .checked_sub(debt_change)
                            .ok_or(Error::<T>::ArithmeticError)?;
                        Ok(())
                    },
                )
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
            CollateralInfos::<T>::try_mutate(
                cdp.collateral_asset_id,
                cdp.stablecoin_asset_id,
                |collateral_info| {
                    let collateral_info = collateral_info
                        .as_mut()
                        .ok_or(Error::<T>::CollateralInfoNotFound)?;
                    collateral_info.total_collateral = collateral_info
                        .total_collateral
                        .checked_sub(transfer_out)
                        .ok_or(Error::<T>::ArithmeticError)?;
                    Ok::<(), Error<T>>(())
                },
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

        /// Inserts or updates `CollateralRiskParameters` for collateral asset id.
        /// If `CollateralRiskParameters` exists for asset id, then updates them.
        /// Else if `CollateralRiskParameters` does not exist, inserts a new value.
        fn upsert_collateral_info(
            collateral_asset_id: &AssetIdOf<T>,
            stablecoin_asset_id: &AssetIdOf<T>,
            new_risk_parameters: CollateralRiskParameters,
        ) -> DispatchResult {
            ensure!(
                <T as Config>::AssetInfoProvider::asset_exists(collateral_asset_id),
                Error::<T>::WrongAssetId
            );
            ensure!(
                collateral_asset_id != stablecoin_asset_id,
                Error::<T>::WrongAssetId
            );
            ensure!(
                *collateral_asset_id != T::KenAssetId::get(),
                Error::<T>::WrongAssetId
            );
            ensure!(
                StablecoinInfos::<T>::contains_key(stablecoin_asset_id),
                Error::<T>::StablecoinInfoNotFound
            );

            CollateralInfos::<T>::try_mutate(
                collateral_asset_id,
                stablecoin_asset_id,
                |option_collateral_info| {
                    match option_collateral_info {
                        Some(collateral_info) => {
                            let mut new_info = Self::calculate_collateral_interest_coefficient(
                                collateral_asset_id,
                                stablecoin_asset_id,
                            )?;
                            new_info.risk_parameters = new_risk_parameters;
                            *collateral_info = new_info;
                        }
                        None => {
                            let _ = option_collateral_info.insert(CollateralInfo {
                                risk_parameters: new_risk_parameters,
                                total_collateral: Balance::zero(),
                                stablecoin_supply: balance!(0),
                                last_fee_update_time: Timestamp::<T>::get(),
                                interest_coefficient: FixedU128::one(),
                            });
                        }
                    }
                    Ok(())
                },
            )
        }

        fn increase_collateral_stablecoin_supply(
            collateral_asset_id: &AssetIdOf<T>,
            stablecoin_asset_id: &AssetIdOf<T>,
            supply_change: Balance,
        ) -> DispatchResult {
            CollateralInfos::<T>::try_mutate(
                collateral_asset_id,
                stablecoin_asset_id,
                |collateral_info| {
                    let collateral_info = collateral_info
                        .as_mut()
                        .ok_or(Error::<T>::CollateralInfoNotFound)?;
                    collateral_info.stablecoin_supply = collateral_info
                        .stablecoin_supply
                        .checked_add(supply_change)
                        .ok_or(Error::<T>::ArithmeticError)?;
                    Ok(())
                },
            )
        }
    }
}
