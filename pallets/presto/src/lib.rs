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

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
mod coupon_info;
mod crop_receipt;
#[cfg(test)]
mod mock;
mod requests;
#[cfg(test)]
mod test;
mod treasury;
pub mod weights;

use common::{AccountIdOf, AssetInfoProvider, Balance};
use frame_support::ensure;
use frame_support::sp_runtime::DispatchError;
use frame_support::traits::Time;
use sp_core::Get;
use sp_runtime::traits::{One, Saturating, Zero};
use sp_std::vec::Vec;

pub use pallet::*;

pub type MomentOf<T> = <<T as Config>::Time as Time>::Moment;

pub const TECH_ACCOUNT_PREFIX: &[u8] = b"presto";
/// Main treasury tech account
pub const TECH_ACCOUNT_MAIN: &[u8] = b"main";
/// Buffer tech account for temp holding of withdraw request liquidity
pub const TECH_ACCOUNT_BUFFER: &[u8] = b"buffer";

const COUPON_SYMBOL: &[u8] = b"C";
const COUPON_NAME: &[u8] = b"Coupon";

#[frame_support::pallet]
#[allow(clippy::too_many_arguments)]
pub mod pallet {
    use super::*;
    use common::fixnum::ops::RoundMode;
    use common::prelude::BalanceUnit;
    use common::{
        balance, itoa, AssetIdOf, AssetManager, AssetName, AssetSymbol, AssetType, BoundedString,
        ContentSource, DEXId, ExtendedAssetsManager, ItoaInteger, OrderBookManager, PriceVariant,
        TradingPairSourceManager,
    };
    use core::fmt::Debug;
    use frame_support::pallet_prelude::*;
    use frame_support::sp_runtime::Permill;
    use frame_system::pallet_prelude::*;
    use sp_runtime::traits::{AtLeast32BitUnsigned, CheckedDiv, MaybeDisplay, Zero};
    use sp_runtime::BoundedVec;

    use coupon_info::CouponInfo;
    use crop_receipt::{Country, CropReceipt, CropReceiptContent, Rating, Status};
    use requests::{DepositRequest, Request, RequestStatus, WithdrawRequest};
    use treasury::Treasury;
    use weights::WeightInfo;

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::config]
    pub trait Config: frame_system::Config + common::Config + technical::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type TradingPairSourceManager: TradingPairSourceManager<Self::DEXId, AssetIdOf<Self>>;
        type OrderBookManager: OrderBookManager<
            AccountIdOf<Self>,
            AssetIdOf<Self>,
            Self::DEXId,
            MomentOf<Self>,
        >;
        type ExtendedAssetsManager: ExtendedAssetsManager<
            AssetIdOf<Self>,
            MomentOf<Self>,
            ContentSource,
        >;
        type PrestoUsdAssetId: Get<AssetIdOf<Self>>;
        type PrestoKycAssetId: Get<AssetIdOf<Self>>;
        type PrestoKycInvestorAssetId: Get<AssetIdOf<Self>>;
        type PrestoKycCreditorAssetId: Get<AssetIdOf<Self>>;
        type PrestoTechAccount: Get<Self::TechAccountId>;
        type PrestoBufferTechAccount: Get<Self::TechAccountId>;
        type RequestId: Parameter
            + Member
            + MaybeSerializeDeserialize
            + Debug
            + Default
            + MaybeDisplay
            + AtLeast32BitUnsigned
            + Copy
            + Ord
            + PartialEq
            + Eq
            + MaxEncodedLen
            + scale_info::TypeInfo;

        type CropReceiptId: Parameter
            + Member
            + MaybeSerializeDeserialize
            + Debug
            + Default
            + MaybeDisplay
            + AtLeast32BitUnsigned
            + Copy
            + Ord
            + PartialEq
            + Eq
            + MaxEncodedLen
            + scale_info::TypeInfo;

        type CouponId: Parameter
            + Member
            + MaybeSerializeDeserialize
            + Debug
            + Default
            + MaybeDisplay
            + AtLeast32BitUnsigned
            + Copy
            + Ord
            + PartialEq
            + Eq
            + MaxEncodedLen
            + scale_info::TypeInfo
            + ItoaInteger;

        #[pallet::constant]
        type MaxPrestoManagersCount: Get<u32>;

        #[pallet::constant]
        type MaxPrestoAuditorsCount: Get<u32>;

        #[pallet::constant]
        type MaxUserRequestCount: Get<u32>;

        #[pallet::constant]
        type MaxUserCropReceiptCount: Get<u32>;

        #[pallet::constant]
        type MaxRequestPaymentReferenceSize: Get<u32>;

        #[pallet::constant]
        type MaxRequestDetailsSize: Get<u32>;

        #[pallet::constant]
        type MaxPlaceOfIssueSize: Get<u32>;

        #[pallet::constant]
        type MaxDebtorSize: Get<u32>;

        #[pallet::constant]
        type MaxCreditorSize: Get<u32>;

        #[pallet::constant]
        type MaxCropReceiptContentSize: Get<u32>;

        type Time: Time;
        type WeightInfo: WeightInfo;
    }

    /// Presto managers
    #[pallet::storage]
    #[pallet::getter(fn managers)]
    pub type Managers<T: Config> =
        StorageValue<_, BoundedVec<AccountIdOf<T>, T::MaxPrestoManagersCount>, ValueQuery>;

    /// Presto auditors
    #[pallet::storage]
    #[pallet::getter(fn auditors)]
    pub type Auditors<T: Config> =
        StorageValue<_, BoundedVec<AccountIdOf<T>, T::MaxPrestoAuditorsCount>, ValueQuery>;

    /// Counter to generate new Crop Receipt Ids
    #[pallet::storage]
    #[pallet::getter(fn last_crop_receipt_id)]
    pub type LastCropReceiptId<T: Config> = StorageValue<_, T::CropReceiptId, ValueQuery>;

    /// Crop receipts
    #[pallet::storage]
    #[pallet::getter(fn crop_receipts)]
    pub type CropReceipts<T: Config> =
        StorageMap<_, Twox64Concat, T::CropReceiptId, CropReceipt<T>, OptionQuery>;

    /// Crop receipts content
    #[pallet::storage]
    #[pallet::getter(fn crop_receipts_content)]
    pub type CropReceiptsContent<T: Config> =
        StorageMap<_, Twox64Concat, T::CropReceiptId, CropReceiptContent<T>, OptionQuery>;

    /// Crop receipts index by user
    #[pallet::storage]
    #[pallet::getter(fn user_crop_receipts)]
    pub type UserCropReceipts<T: Config> = StorageMap<
        _,
        Twox64Concat,
        AccountIdOf<T>,
        BoundedVec<T::CropReceiptId, T::MaxUserCropReceiptCount>,
        ValueQuery,
    >;

    /// Counter to generate new Request Ids
    #[pallet::storage]
    #[pallet::getter(fn last_request_id)]
    pub type LastRequestId<T: Config> = StorageValue<_, T::RequestId, ValueQuery>;

    /// Requests
    #[pallet::storage]
    #[pallet::getter(fn requests)]
    pub type Requests<T: Config> =
        StorageMap<_, Twox64Concat, T::RequestId, Request<T>, OptionQuery>;

    /// Requests index by users
    #[pallet::storage]
    #[pallet::getter(fn user_requests)]
    pub type UserRequests<T: Config> = StorageMap<
        _,
        Twox64Concat,
        AccountIdOf<T>,
        BoundedVec<T::RequestId, T::MaxUserRequestCount>,
        ValueQuery,
    >;

    /// Counter to generate new Coupon Ids
    #[pallet::storage]
    #[pallet::getter(fn last_coupon_id)]
    pub type LastCouponId<T: Config> = StorageValue<_, T::CouponId, ValueQuery>;

    /// Coupons
    #[pallet::storage]
    #[pallet::getter(fn coupons)]
    pub type Coupons<T: Config> =
        StorageMap<_, Twox64Concat, AssetIdOf<T>, CouponInfo<T>, OptionQuery>;

    /// Index to map Crop Receipt with emitted Coupon
    #[pallet::storage]
    #[pallet::getter(fn crop_receipt_to_coupon)]
    pub type CropReceiptToCoupon<T: Config> =
        StorageMap<_, Twox64Concat, T::CropReceiptId, AssetIdOf<T>, OptionQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        ManagerAdded {
            manager: AccountIdOf<T>,
        },
        ManagerRemoved {
            manager: AccountIdOf<T>,
        },
        AuditorAdded {
            auditor: AccountIdOf<T>,
        },
        AuditorRemoved {
            auditor: AccountIdOf<T>,
        },
        PrestoUsdMinted {
            amount: Balance,
            by: AccountIdOf<T>,
        },
        PrestoUsdBurned {
            amount: Balance,
            by: AccountIdOf<T>,
        },
        RequestCreated {
            id: T::RequestId,
            by: AccountIdOf<T>,
        },
        RequestCancelled {
            id: T::RequestId,
        },
        RequestApproved {
            id: T::RequestId,
            by: AccountIdOf<T>,
        },
        RequestDeclined {
            id: T::RequestId,
            by: AccountIdOf<T>,
        },
        CropReceiptCreated {
            id: T::CropReceiptId,
            by: AccountIdOf<T>,
        },
        CropReceiptRated {
            id: T::CropReceiptId,
            by: AccountIdOf<T>,
        },
        CropReceiptDeclined {
            id: T::CropReceiptId,
        },
        CropReceiptPublished {
            id: T::CropReceiptId,
            coupon_asset_id: AssetIdOf<T>,
        },
        CropReceiptClosed {
            id: T::CropReceiptId,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// This account already was added as a manager before
        ManagerAlreadyAdded,
        /// Managers storage has reached its limit
        ManagersAreOverloaded,
        /// There is no such manager
        ManagerNotExists,
        /// This account already was added as an auditor before
        AuditorAlreadyAdded,
        /// Auditors storage has reached its limit
        AuditorsAreOverloaded,
        /// There is no such auditor
        AuditorNotExists,
        /// This account is not a manager
        CallerIsNotManager,
        /// This account is not an auditor
        CallerIsNotAuditor,
        /// Account already passed KYC
        KycAlreadyPassed,
        /// Account not passed KYC
        KycNotPassed,
        /// Account not passed KYC as investor
        InvestorKycNotPassed,
        /// Account not passed KYC as creditor
        CreditorKycNotPassed,
        /// Account has any Presto asset
        AccountHasPrestoAssets,
        /// Zero amount doesn't make sense
        AmountIsZero,
        /// This account has reached the max count of requests
        RequestsCountForUserOverloaded,
        /// There is no such request
        RequestIsNotExists,
        /// This account is not an owner of the request
        CallerIsNotRequestOwner,
        /// This request was already processed by manager
        RequestAlreadyProcessed,
        /// The actual request type by provided RequestId is different
        WrongRequestType,
        /// This account has reached the max count of crop receipts
        CropReceiptsCountForUserOverloaded,
        /// There is no such crop receipt
        CropReceiptIsNotExists,
        /// The crop receipt already has been rated
        CropReceiptAlreadyRated,
        /// This account is not an owner of the crop receipt
        CallerIsNotCropReceiptOwner,
        /// The operation cannot be performed until the crop receipt has been rated
        CropReceiptWaitingForRate,
        /// The crop receipt already has a decision
        CropReceiptAlreadyHasDecision,
        /// The crop receipt has been closed
        CropReceiptHasBeenClosed,
        /// The crop receipt cannot be closed
        CropReceiptCannotBeClosed,
        /// The crop receipt is not closed yet
        CropReceiptIsNotClosedYet,
        /// Coupon supply cannot be bigger than requested amount in crop receipt
        TooBigCouponSupply,
        /// Fail of coupon public offering
        CouponOfferingFail,
        /// Error during calculations
        CalculationError,
        /// There is no data about emitted coupon for the crop receipt
        NoCouponData,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::add_presto_manager())]
        pub fn add_presto_manager(origin: OriginFor<T>, manager: AccountIdOf<T>) -> DispatchResult {
            ensure_root(origin)?;

            let mut managers = Managers::<T>::get();
            ensure!(
                !managers.contains(&manager),
                Error::<T>::ManagerAlreadyAdded
            );
            managers
                .try_push(manager.clone())
                .map_err(|_| Error::<T>::ManagersAreOverloaded)?;
            Managers::<T>::set(managers);

            Self::deposit_event(Event::<T>::ManagerAdded { manager });

            Ok(())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::remove_presto_manager())]
        pub fn remove_presto_manager(
            origin: OriginFor<T>,
            manager: AccountIdOf<T>,
        ) -> DispatchResult {
            ensure_root(origin)?;

            let mut managers = Managers::<T>::get();
            ensure!(managers.contains(&manager), Error::<T>::ManagerNotExists);
            managers.retain(|x| *x != manager);
            Managers::<T>::set(managers);

            Self::deposit_event(Event::<T>::ManagerRemoved { manager });

            Ok(())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::add_presto_auditor())]
        pub fn add_presto_auditor(origin: OriginFor<T>, auditor: AccountIdOf<T>) -> DispatchResult {
            ensure_root(origin)?;

            let mut auditors = Auditors::<T>::get();
            ensure!(
                !auditors.contains(&auditor),
                Error::<T>::AuditorAlreadyAdded
            );
            auditors
                .try_push(auditor.clone())
                .map_err(|_| Error::<T>::AuditorsAreOverloaded)?;
            Auditors::<T>::set(auditors);

            Self::deposit_event(Event::<T>::AuditorAdded { auditor });

            Ok(())
        }

        #[pallet::call_index(3)]
        #[pallet::weight(<T as Config>::WeightInfo::remove_presto_auditor())]
        pub fn remove_presto_auditor(
            origin: OriginFor<T>,
            auditor: AccountIdOf<T>,
        ) -> DispatchResult {
            ensure_root(origin)?;

            let mut auditors = Auditors::<T>::get();
            ensure!(auditors.contains(&auditor), Error::<T>::AuditorNotExists);
            auditors.retain(|x| *x != auditor);
            Auditors::<T>::set(auditors);

            Self::deposit_event(Event::<T>::AuditorRemoved { auditor });

            Ok(())
        }

        #[pallet::call_index(4)]
        #[pallet::weight(<T as Config>::WeightInfo::apply_investor_kyc())]
        pub fn apply_investor_kyc(
            origin: OriginFor<T>,
            investor: AccountIdOf<T>,
        ) -> DispatchResult {
            let manager = ensure_signed(origin)?;
            Self::ensure_is_manager(&manager)?;

            Self::ensure_no_kyc(&investor)?;

            let presto_tech_account_id = technical::Pallet::<T>::tech_account_id_to_account_id(
                &T::PrestoTechAccount::get(),
            )?;

            T::AssetManager::mint_to(
                &T::PrestoKycAssetId::get(),
                &presto_tech_account_id,
                &investor,
                1,
            )?;

            T::AssetManager::mint_to(
                &T::PrestoKycInvestorAssetId::get(),
                &presto_tech_account_id,
                &investor,
                1,
            )?;

            Ok(())
        }

        #[pallet::call_index(5)]
        #[pallet::weight(<T as Config>::WeightInfo::apply_creditor_kyc())]
        pub fn apply_creditor_kyc(
            origin: OriginFor<T>,
            creditor: AccountIdOf<T>,
        ) -> DispatchResult {
            let manager = ensure_signed(origin)?;
            Self::ensure_is_manager(&manager)?;

            Self::ensure_no_kyc(&creditor)?;

            let presto_tech_account_id = technical::Pallet::<T>::tech_account_id_to_account_id(
                &T::PrestoTechAccount::get(),
            )?;

            T::AssetManager::mint_to(
                &T::PrestoKycAssetId::get(),
                &presto_tech_account_id,
                &creditor,
                1,
            )?;

            T::AssetManager::mint_to(
                &T::PrestoKycCreditorAssetId::get(),
                &presto_tech_account_id,
                &creditor,
                1,
            )?;

            Ok(())
        }

        #[pallet::call_index(6)]
        #[pallet::weight(<T as Config>::WeightInfo::remove_investor_kyc())]
        pub fn remove_investor_kyc(
            origin: OriginFor<T>,
            investor: AccountIdOf<T>,
        ) -> DispatchResult {
            let manager = ensure_signed(origin)?;
            Self::ensure_is_manager(&manager)?;

            let kyc_amount = Self::ensure_has_kyc(&investor)?;
            let investor_kyc_amount = Self::ensure_has_investor_kyc(&investor)?;
            Self::ensure_no_presto_assets(&investor)?;

            let presto_tech_account_id = technical::Pallet::<T>::tech_account_id_to_account_id(
                &T::PrestoTechAccount::get(),
            )?;

            T::AssetManager::burn_from(
                &T::PrestoKycAssetId::get(),
                &presto_tech_account_id,
                &investor,
                kyc_amount,
            )?;

            T::AssetManager::burn_from(
                &T::PrestoKycInvestorAssetId::get(),
                &presto_tech_account_id,
                &investor,
                investor_kyc_amount,
            )?;

            Ok(())
        }

        #[pallet::call_index(7)]
        #[pallet::weight(<T as Config>::WeightInfo::remove_creditor_kyc())]
        pub fn remove_creditor_kyc(
            origin: OriginFor<T>,
            creditor: AccountIdOf<T>,
        ) -> DispatchResult {
            let manager = ensure_signed(origin)?;
            Self::ensure_is_manager(&manager)?;

            let kyc_amount = Self::ensure_has_kyc(&creditor)?;
            let creditor_kyc_amount = Self::ensure_has_creditor_kyc(&creditor)?;
            Self::ensure_no_presto_assets(&creditor)?;

            let presto_tech_account_id = technical::Pallet::<T>::tech_account_id_to_account_id(
                &T::PrestoTechAccount::get(),
            )?;

            T::AssetManager::burn_from(
                &T::PrestoKycAssetId::get(),
                &presto_tech_account_id,
                &creditor,
                kyc_amount,
            )?;

            T::AssetManager::burn_from(
                &T::PrestoKycCreditorAssetId::get(),
                &presto_tech_account_id,
                &creditor,
                creditor_kyc_amount,
            )?;

            Ok(())
        }

        #[pallet::call_index(8)]
        #[pallet::weight(<T as Config>::WeightInfo::mint_presto_usd())]
        pub fn mint_presto_usd(origin: OriginFor<T>, amount: Balance) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_is_manager(&who)?;
            ensure!(!amount.is_zero(), Error::<T>::AmountIsZero);
            Treasury::<T>::mint_presto_usd(amount)?;

            Self::deposit_event(Event::<T>::PrestoUsdMinted { amount, by: who });

            Ok(())
        }

        #[pallet::call_index(9)]
        #[pallet::weight(<T as Config>::WeightInfo::burn_presto_usd())]
        pub fn burn_presto_usd(origin: OriginFor<T>, amount: Balance) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_is_manager(&who)?;
            ensure!(!amount.is_zero(), Error::<T>::AmountIsZero);
            Treasury::<T>::burn_presto_usd(amount)?;

            Self::deposit_event(Event::<T>::PrestoUsdBurned { amount, by: who });

            Ok(())
        }

        #[pallet::call_index(10)]
        #[pallet::weight(<T as Config>::WeightInfo::send_presto_usd())]
        pub fn send_presto_usd(
            origin: OriginFor<T>,
            amount: Balance,
            to: AccountIdOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_is_manager(&who)?;
            ensure!(!amount.is_zero(), Error::<T>::AmountIsZero);
            Treasury::<T>::send_presto_usd(amount, &to)?;

            Ok(())
        }

        #[pallet::call_index(11)]
        #[pallet::weight(<T as Config>::WeightInfo::create_deposit_request())]
        pub fn create_deposit_request(
            origin: OriginFor<T>,
            amount: Balance,
            payment_reference: BoundedString<T::MaxRequestPaymentReferenceSize>,
            details: Option<BoundedString<T::MaxRequestDetailsSize>>,
        ) -> DispatchResult {
            let owner = ensure_signed(origin)?;

            ensure!(!amount.is_zero(), Error::<T>::AmountIsZero);
            Self::ensure_has_kyc(&owner)?;
            let id = Self::next_request_id();

            let request = Request::Deposit(DepositRequest::new(
                owner.clone(),
                amount,
                payment_reference,
                details,
            ));

            Requests::<T>::insert(id, request);
            UserRequests::<T>::try_mutate(&owner, |ids| {
                ids.try_push(id)
                    .map_err(|_| Error::<T>::RequestsCountForUserOverloaded)?;
                Ok::<(), Error<T>>(())
            })?;

            Self::deposit_event(Event::<T>::RequestCreated { id, by: owner });

            Ok(())
        }

        #[pallet::call_index(12)]
        #[pallet::weight(<T as Config>::WeightInfo::create_withdraw_request())]
        pub fn create_withdraw_request(
            origin: OriginFor<T>,
            amount: Balance,
            details: Option<BoundedString<T::MaxRequestDetailsSize>>,
        ) -> DispatchResult {
            let owner = ensure_signed(origin)?;

            ensure!(!amount.is_zero(), Error::<T>::AmountIsZero);
            Self::ensure_has_kyc(&owner)?;
            let id = Self::next_request_id();

            let request = Request::Withdraw(WithdrawRequest::new(owner.clone(), amount, details)?);

            Requests::<T>::insert(id, request);
            UserRequests::<T>::try_mutate(&owner, |ids| {
                ids.try_push(id)
                    .map_err(|_| Error::<T>::RequestsCountForUserOverloaded)?;
                Ok::<(), Error<T>>(())
            })?;

            Self::deposit_event(Event::<T>::RequestCreated { id, by: owner });

            Ok(())
        }

        #[pallet::call_index(13)]
        #[pallet::weight(<T as Config>::WeightInfo::cancel_request())]
        pub fn cancel_request(origin: OriginFor<T>, request_id: T::RequestId) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_has_kyc(&who)?;

            Requests::<T>::try_mutate(request_id, |request| {
                let request = request.as_mut().ok_or(Error::<T>::RequestIsNotExists)?;

                request.ensure_is_owner(&who)?;

                ensure!(
                    *request.status() == RequestStatus::Pending,
                    Error::<T>::RequestAlreadyProcessed
                );

                request.cancel()?;

                Self::deposit_event(Event::<T>::RequestCancelled { id: request_id });

                Ok(())
            })
        }

        #[pallet::call_index(14)]
        #[pallet::weight(<T as Config>::WeightInfo::approve_deposit_request())]
        pub fn approve_deposit_request(
            origin: OriginFor<T>,
            request_id: T::RequestId,
        ) -> DispatchResult {
            let manager = ensure_signed(origin)?;
            Self::ensure_is_manager(&manager)?;

            Requests::<T>::try_mutate(request_id, |request| {
                let request = request.as_mut().ok_or(Error::<T>::RequestIsNotExists)?;

                ensure!(
                    *request.status() == RequestStatus::Pending,
                    Error::<T>::RequestAlreadyProcessed
                );

                if let Request::Deposit(deposit_request) = request {
                    deposit_request.approve(manager.clone())?;
                } else {
                    return Err(Error::<T>::WrongRequestType.into());
                }

                Self::deposit_event(Event::<T>::RequestApproved {
                    id: request_id,
                    by: manager,
                });

                Ok(())
            })
        }

        #[pallet::call_index(15)]
        #[pallet::weight(<T as Config>::WeightInfo::approve_withdraw_request())]
        pub fn approve_withdraw_request(
            origin: OriginFor<T>,
            request_id: T::RequestId,
            payment_reference: BoundedString<T::MaxRequestPaymentReferenceSize>,
        ) -> DispatchResult {
            let manager = ensure_signed(origin)?;
            Self::ensure_is_manager(&manager)?;

            Requests::<T>::try_mutate(request_id, |request| {
                let request = request.as_mut().ok_or(Error::<T>::RequestIsNotExists)?;

                ensure!(
                    *request.status() == RequestStatus::Pending,
                    Error::<T>::RequestAlreadyProcessed
                );

                if let Request::Withdraw(withdraw_request) = request {
                    withdraw_request.approve(manager.clone(), payment_reference)?;
                } else {
                    return Err(Error::<T>::WrongRequestType.into());
                }

                Self::deposit_event(Event::<T>::RequestApproved {
                    id: request_id,
                    by: manager,
                });

                Ok(())
            })
        }

        #[pallet::call_index(16)]
        #[pallet::weight(<T as Config>::WeightInfo::decline_request())]
        pub fn decline_request(origin: OriginFor<T>, request_id: T::RequestId) -> DispatchResult {
            let manager = ensure_signed(origin)?;
            Self::ensure_is_manager(&manager)?;

            Requests::<T>::try_mutate(request_id, |request| {
                let request = request.as_mut().ok_or(Error::<T>::RequestIsNotExists)?;

                ensure!(
                    *request.status() == RequestStatus::Pending,
                    Error::<T>::RequestAlreadyProcessed
                );

                request.decline(manager.clone())?;

                Self::deposit_event(Event::<T>::RequestDeclined {
                    id: request_id,
                    by: manager,
                });

                Ok(())
            })
        }

        #[pallet::call_index(17)]
        #[pallet::weight(<T as Config>::WeightInfo::create_crop_receipt())]
        pub fn create_crop_receipt(
            origin: OriginFor<T>,
            amount: Balance,
            profit: Permill,
            country: Country,
            close_initial_period: MomentOf<T>,
            date_of_issue: MomentOf<T>,
            place_of_issue: BoundedString<T::MaxPlaceOfIssueSize>,
            debtor: BoundedString<T::MaxDebtorSize>,
            creditor: BoundedString<T::MaxCreditorSize>,
            perfomance_time: MomentOf<T>,
            data: BoundedString<T::MaxCropReceiptContentSize>,
        ) -> DispatchResult {
            let owner = ensure_signed(origin)?;
            ensure!(!amount.is_zero(), Error::<T>::AmountIsZero);
            Self::ensure_has_creditor_kyc(&owner)?;
            let id = Self::next_crop_receipt_id();

            let crop_receipt = CropReceipt::<T>::new(
                owner.clone(),
                amount,
                profit,
                country,
                close_initial_period,
                date_of_issue,
                place_of_issue,
                debtor,
                creditor,
                perfomance_time,
            );

            let content = CropReceiptContent::<T> { json: data };

            CropReceipts::<T>::insert(id, crop_receipt);
            CropReceiptsContent::<T>::insert(id, content);

            UserCropReceipts::<T>::try_mutate(&owner, |ids| {
                ids.try_push(id)
                    .map_err(|_| Error::<T>::CropReceiptsCountForUserOverloaded)?;
                Ok::<(), Error<T>>(())
            })?;

            Self::deposit_event(Event::<T>::CropReceiptCreated { id, by: owner });

            Ok(())
        }

        #[pallet::call_index(18)]
        #[pallet::weight(<T as Config>::WeightInfo::rate_crop_receipt())]
        pub fn rate_crop_receipt(
            origin: OriginFor<T>,
            crop_receipt_id: T::CropReceiptId,
            rating: Rating,
        ) -> DispatchResult {
            let auditor = ensure_signed(origin)?;
            Self::ensure_is_auditor(&auditor)?;

            CropReceipts::<T>::try_mutate(crop_receipt_id, |crop_receipt| {
                let crop_receipt = crop_receipt
                    .as_mut()
                    .ok_or(Error::<T>::CropReceiptIsNotExists)?;

                crop_receipt.rate(rating, auditor.clone())?;

                Self::deposit_event(Event::<T>::CropReceiptRated {
                    id: crop_receipt_id,
                    by: auditor,
                });

                Ok(())
            })
        }

        #[pallet::call_index(19)]
        #[pallet::weight(<T as Config>::WeightInfo::decline_crop_receipt())]
        pub fn decline_crop_receipt(
            origin: OriginFor<T>,
            crop_receipt_id: T::CropReceiptId,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_has_creditor_kyc(&who)?;

            CropReceipts::<T>::try_mutate(crop_receipt_id, |crop_receipt| {
                let crop_receipt = crop_receipt
                    .as_mut()
                    .ok_or(Error::<T>::CropReceiptIsNotExists)?;

                crop_receipt.ensure_is_owner(&who)?;
                crop_receipt.decline()?;

                Self::deposit_event(Event::<T>::CropReceiptDeclined {
                    id: crop_receipt_id,
                });

                Ok(())
            })
        }

        #[pallet::call_index(20)]
        #[pallet::weight(<T as Config>::WeightInfo::publish_crop_receipt())]
        pub fn publish_crop_receipt(
            origin: OriginFor<T>,
            crop_receipt_id: T::CropReceiptId,
            supply: Balance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(!supply.is_zero(), Error::<T>::AmountIsZero);
            Self::ensure_has_creditor_kyc(&who)?;

            let coupon_supply = BalanceUnit::indivisible(supply);

            let mut country = Country::Other;
            let mut amount = BalanceUnit::default();
            let mut profit = Permill::default();
            CropReceipts::<T>::try_mutate(crop_receipt_id, |crop_receipt| {
                let crop_receipt = crop_receipt
                    .as_mut()
                    .ok_or(Error::<T>::CropReceiptIsNotExists)?;

                crop_receipt.ensure_is_owner(&who)?;
                country = crop_receipt.country;
                amount = BalanceUnit::divisible(crop_receipt.amount);
                profit = crop_receipt.profit;

                // The initial price (amount / supply) must be >= 1.00
                ensure!(coupon_supply <= amount, Error::<T>::TooBigCouponSupply);

                crop_receipt.publish()?;
                DispatchResult::Ok(())
            })?;

            let presto_tech_account_id = technical::Pallet::<T>::tech_account_id_to_account_id(
                &T::PrestoTechAccount::get(),
            )?;

            // use `itoa` to convert `CouponId` to Vec<u8> string in no-std env.
            let coupon_id = itoa(Self::next_coupon_id());

            let symbol = AssetSymbol([country.symbol(), COUPON_SYMBOL, &coupon_id].concat());
            let name = AssetName([country.name(), COUPON_NAME, &coupon_id].join(&b' '));

            let coupon_asset_id = T::AssetManager::register_from(
                &presto_tech_account_id,
                symbol,
                name,
                0,
                supply,
                false,
                AssetType::Regulated,
                None,
                None,
            )?;

            T::ExtendedAssetsManager::bind_regulated_asset_to_sbt_asset(
                &T::PrestoKycAssetId::get(),
                &coupon_asset_id,
            )?;

            T::AssetManager::transfer_from(
                &coupon_asset_id,
                &presto_tech_account_id,
                &who,
                supply,
            )?;

            T::TradingPairSourceManager::register_pair(
                DEXId::PolkaswapPresto.into(),
                T::PrestoUsdAssetId::get(),
                coupon_asset_id,
            )?;

            let order_book_id = T::OrderBookManager::assemble_order_book_id(
                DEXId::PolkaswapPresto.into(),
                &T::PrestoUsdAssetId::get(),
                &coupon_asset_id,
            )
            .ok_or(Error::<T>::CouponOfferingFail)?;

            // Tick size always is 0.01 PRUSD
            let tick_size = balance!(0.01);
            // Since Coupon is non-divisible asset, the step lot size is always 1 Coupon
            let step_lot_size = 1;

            // This value must correlate with `order_book::Config` const `MaxLimitOrdersForPrice`.
            // It shouldn't be equal, but it must always be not higher: `max_orders_count` <= `MaxLimitOrdersForPrice`
            let max_orders_count = BalanceUnit::divisible(balance!(1000));

            // This value must correlate with `order_book::Config` const `SOFT_MIN_MAX_RATIO`.
            // It shouldn't be equal, but it must always be not higher: `max_orders_count` <= `SOFT_MIN_MAX_RATIO`
            let min_max_ratio = BalanceUnit::divisible(balance!(1000));

            // Calculate the max lot size amount that is suitable to offer all Coupon supply at one price in order book
            let max = coupon_supply
                .checked_div(&max_orders_count)
                .ok_or(Error::<T>::CalculationError)?;

            // default values
            let default_min_lot_size = 1;
            let default_max_lot_size = 1000;

            let (min_lot_size, max_lot_size) =
                if max <= BalanceUnit::indivisible(default_max_lot_size) {
                    (default_min_lot_size, default_max_lot_size)
                } else {
                    // if necessary max lot size exceeds the default value 1000 - calculate suitable min value
                    let min = max
                        .checked_div(&min_max_ratio)
                        .ok_or(Error::<T>::CalculationError)?;
                    (
                        *min.into_indivisible(RoundMode::Ceil).balance(),
                        *max.into_indivisible(RoundMode::Ceil).balance(),
                    )
                };

            let offer_price = *amount
                .checked_div(&coupon_supply)
                .ok_or(Error::<T>::CalculationError)?
                .into_divisible()
                .ok_or(Error::<T>::CalculationError)?
                .balance();
            let offer_price = Self::align_price(offer_price, tick_size)?;

            let refund_price = offer_price
                .checked_add(profit * offer_price)
                .ok_or(Error::<T>::CalculationError)?;
            let refund_price = BalanceUnit::divisible(refund_price);

            Coupons::<T>::insert(
                coupon_asset_id,
                CouponInfo {
                    crop_receipt_id,
                    supply: coupon_supply,
                    refund_price,
                },
            );

            CropReceiptToCoupon::<T>::insert(crop_receipt_id, coupon_asset_id);

            // create order book
            T::OrderBookManager::initialize_orderbook(
                &order_book_id,
                tick_size,
                step_lot_size,
                min_lot_size,
                max_lot_size,
            )?;

            let order_book_account_id =
                T::OrderBookManager::tech_account_id_for_order_book(&order_book_id)?;

            // Presto KYC SBT for order book tech account
            T::AssetManager::mint_to(
                &T::PrestoKycAssetId::get(),
                &presto_tech_account_id,
                &order_book_account_id,
                1,
            )?;

            // place all supply in order book in according with `max_lot_size` limitation
            let mut remaining_amount = supply;
            while !remaining_amount.is_zero() {
                let qty = if remaining_amount > max_lot_size {
                    max_lot_size
                } else {
                    remaining_amount
                };

                T::OrderBookManager::place_limit_order(
                    who.clone(),
                    order_book_id,
                    offer_price,
                    qty,
                    PriceVariant::Sell,
                    None,
                )?;

                remaining_amount = remaining_amount.saturating_sub(qty);
            }

            Self::deposit_event(Event::<T>::CropReceiptPublished {
                id: crop_receipt_id,
                coupon_asset_id,
            });

            Ok(())
        }

        #[pallet::call_index(21)]
        #[pallet::weight(<T as Config>::WeightInfo::publish_crop_receipt())]
        pub fn pay_off_crop_receipt(
            origin: OriginFor<T>,
            crop_receipt_id: T::CropReceiptId,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_has_creditor_kyc(&who)?;

            CropReceipts::<T>::try_mutate(crop_receipt_id, |crop_receipt| {
                let crop_receipt = crop_receipt
                    .as_mut()
                    .ok_or(Error::<T>::CropReceiptIsNotExists)?;

                crop_receipt.ensure_is_owner(&who)?;
                crop_receipt.close()
            })?;

            let coupon_asset_id =
                CropReceiptToCoupon::<T>::get(crop_receipt_id).ok_or(Error::<T>::NoCouponData)?;

            let coupon_info = Coupons::<T>::get(coupon_asset_id).ok_or(Error::<T>::NoCouponData)?;

            let mut total_debt = coupon_info.total_debt_cost()?;

            // if creditor has any amount of coupons - they are burned, but the total pay off debt is reduced by the coupons cost
            let coupon_amount = T::AssetInfoProvider::free_balance(&coupon_asset_id, &who)?;
            if coupon_amount > Balance::zero() {
                let coupons_cost = coupon_info.coupons_cost(coupon_amount)?;
                total_debt = total_debt
                    .checked_sub(coupons_cost)
                    .ok_or(Error::<T>::CalculationError)?;

                let presto_tech_account_id = technical::Pallet::<T>::tech_account_id_to_account_id(
                    &T::PrestoTechAccount::get(),
                )?;

                T::AssetManager::burn_from(
                    &coupon_asset_id,
                    &presto_tech_account_id,
                    &who,
                    coupon_amount,
                )?;
            }

            Treasury::<T>::transfer_to_buffer(total_debt, &who)?;

            Self::deposit_event(Event::<T>::CropReceiptClosed {
                id: crop_receipt_id,
            });

            Ok(())
        }

        #[pallet::call_index(22)]
        #[pallet::weight(<T as Config>::WeightInfo::claim_refund())]
        pub fn claim_refund(
            origin: OriginFor<T>,
            coupon_asset_id: AssetIdOf<T>,
            coupon_amount: Balance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_has_investor_kyc(&who)?;

            let presto_tech_account_id = technical::Pallet::<T>::tech_account_id_to_account_id(
                &T::PrestoTechAccount::get(),
            )?;

            T::AssetManager::burn_from(
                &coupon_asset_id,
                &presto_tech_account_id,
                &who,
                coupon_amount,
            )?;

            let coupon_info = Coupons::<T>::get(coupon_asset_id).ok_or(Error::<T>::NoCouponData)?;

            let crop_receipt = CropReceipts::<T>::get(coupon_info.crop_receipt_id)
                .ok_or(Error::<T>::CropReceiptIsNotExists)?;
            ensure!(
                crop_receipt.status == Status::Closed,
                Error::<T>::CropReceiptIsNotClosedYet
            );

            let refund_amount = coupon_info.coupons_cost(coupon_amount)?;
            Treasury::<T>::transfer_from_buffer(refund_amount, &who)?;

            Ok(())
        }
    }
}

impl<T: Config> Pallet<T> {
    pub fn ensure_is_manager(account: &AccountIdOf<T>) -> Result<(), DispatchError> {
        ensure!(
            Managers::<T>::get().contains(account),
            Error::<T>::CallerIsNotManager
        );
        Ok(())
    }

    pub fn ensure_is_auditor(account: &AccountIdOf<T>) -> Result<(), DispatchError> {
        ensure!(
            Auditors::<T>::get().contains(account),
            Error::<T>::CallerIsNotAuditor
        );
        Ok(())
    }

    pub fn next_request_id() -> T::RequestId {
        let id = LastRequestId::<T>::get().saturating_add(T::RequestId::one());
        LastRequestId::<T>::set(id);
        id
    }

    pub fn next_crop_receipt_id() -> T::CropReceiptId {
        let id = LastCropReceiptId::<T>::get().saturating_add(T::CropReceiptId::one());
        LastCropReceiptId::<T>::set(id);
        id
    }

    pub fn next_coupon_id() -> T::CouponId {
        let id = LastCouponId::<T>::get().saturating_add(T::CouponId::one());
        LastCouponId::<T>::set(id);
        id
    }

    fn align_price(price: Balance, tick_size: Balance) -> Result<Balance, DispatchError> {
        ensure!(tick_size != 0, Error::<T>::CalculationError);
        let steps = price.saturating_div(tick_size);
        Ok(tick_size.saturating_mul(steps))
    }

    pub fn ensure_no_kyc(account: &AccountIdOf<T>) -> Result<(), DispatchError> {
        ensure!(
            T::AssetInfoProvider::free_balance(&T::PrestoKycAssetId::get(), account)?.is_zero(),
            Error::<T>::KycAlreadyPassed
        );
        Ok(())
    }

    pub fn ensure_has_kyc(account: &AccountIdOf<T>) -> Result<Balance, DispatchError> {
        let amount = T::AssetInfoProvider::free_balance(&T::PrestoKycAssetId::get(), account)?;
        ensure!(amount > Zero::zero(), Error::<T>::KycNotPassed);
        Ok(amount)
    }

    pub fn ensure_has_investor_kyc(account: &AccountIdOf<T>) -> Result<Balance, DispatchError> {
        let amount =
            T::AssetInfoProvider::free_balance(&T::PrestoKycInvestorAssetId::get(), account)?;
        ensure!(amount > Zero::zero(), Error::<T>::InvestorKycNotPassed);
        Ok(amount)
    }

    pub fn ensure_has_creditor_kyc(account: &AccountIdOf<T>) -> Result<Balance, DispatchError> {
        let amount =
            T::AssetInfoProvider::free_balance(&T::PrestoKycCreditorAssetId::get(), account)?;
        ensure!(amount > Zero::zero(), Error::<T>::CreditorKycNotPassed);
        Ok(amount)
    }

    pub fn ensure_no_presto_assets(account: &AccountIdOf<T>) -> Result<(), DispatchError> {
        let mut presto_assets = Coupons::<T>::iter_keys().collect::<Vec<_>>();
        presto_assets.push(T::PrestoUsdAssetId::get());

        for asset in presto_assets {
            ensure!(
                T::AssetInfoProvider::free_balance(&asset, account)?.is_zero(),
                Error::<T>::AccountHasPrestoAssets
            );
        }
        Ok(())
    }
}
