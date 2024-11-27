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

mod crop_receipt;
#[cfg(test)]
mod mock;
mod requests;
#[cfg(test)]
mod test;
mod treasury;
pub mod weights;

use common::AccountIdOf;
use frame_support::ensure;
use frame_support::sp_runtime::DispatchError;
use frame_support::traits::Time;
use sp_runtime::traits::{One, Saturating};

use crop_receipt::CropReceipt;
use requests::{DepositRequest, Request, RequestStatus, WithdrawRequest};
use treasury::Treasury;
use weights::WeightInfo;

pub use pallet::*;

pub type MomentOf<T> = <<T as Config>::Time as Time>::Moment;

pub const TECH_ACCOUNT_PREFIX: &[u8] = b"presto";
/// Main treasury tech account
pub const TECH_ACCOUNT_MAIN: &[u8] = b"main";
/// Buffer tech account for temp holding of withdraw request liquidity
pub const TECH_ACCOUNT_BUFFER: &[u8] = b"buffer";

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use common::{AssetIdOf, Balance, BoundedString};
    use core::fmt::Debug;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use sp_runtime::traits::{AtLeast32BitUnsigned, MaybeDisplay};
    use sp_runtime::BoundedVec;

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::config]
    pub trait Config: frame_system::Config + common::Config + technical::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type PrestoUsdAssetId: Get<AssetIdOf<Self>>;
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

        #[pallet::constant]
        type MaxPrestoManagersCount: Get<u32>;

        #[pallet::constant]
        type MaxPrestoAuditorsCount: Get<u32>;

        #[pallet::constant]
        type MaxUserRequestCount: Get<u32>;

        #[pallet::constant]
        type MaxRequestPaymentReferenceSize: Get<u32>;

        #[pallet::constant]
        type MaxRequestDetailsSize: Get<u32>;

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
        StorageMap<_, Twox64Concat, T::CropReceiptId, CropReceipt, OptionQuery>;

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
        #[pallet::weight(<T as Config>::WeightInfo::mint_presto_usd())]
        pub fn mint_presto_usd(origin: OriginFor<T>, amount: Balance) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_is_manager(&who)?;
            Treasury::<T>::mint_presto_usd(amount)?;

            Self::deposit_event(Event::<T>::PrestoUsdMinted { amount, by: who });

            Ok(())
        }

        #[pallet::call_index(5)]
        #[pallet::weight(<T as Config>::WeightInfo::burn_presto_usd())]
        pub fn burn_presto_usd(origin: OriginFor<T>, amount: Balance) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_is_manager(&who)?;
            Treasury::<T>::burn_presto_usd(amount)?;

            Self::deposit_event(Event::<T>::PrestoUsdBurned { amount, by: who });

            Ok(())
        }

        #[pallet::call_index(6)]
        #[pallet::weight(<T as Config>::WeightInfo::send_presto_usd())]
        pub fn send_presto_usd(
            origin: OriginFor<T>,
            amount: Balance,
            to: AccountIdOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_is_manager(&who)?;
            Treasury::<T>::send_presto_usd(amount, &to)?;

            Ok(())
        }

        #[pallet::call_index(7)]
        #[pallet::weight(<T as Config>::WeightInfo::create_deposit_request())]
        pub fn create_deposit_request(
            origin: OriginFor<T>,
            amount: Balance,
            payment_reference: BoundedString<T::MaxRequestPaymentReferenceSize>,
            details: Option<BoundedString<T::MaxRequestDetailsSize>>,
        ) -> DispatchResult {
            let owner = ensure_signed(origin)?;

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

        #[pallet::call_index(8)]
        #[pallet::weight(<T as Config>::WeightInfo::create_withdraw_request())]
        pub fn create_withdraw_request(
            origin: OriginFor<T>,
            amount: Balance,
            details: Option<BoundedString<T::MaxRequestDetailsSize>>,
        ) -> DispatchResult {
            let owner = ensure_signed(origin)?;

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

        #[pallet::call_index(9)]
        #[pallet::weight(<T as Config>::WeightInfo::create_withdraw_request())]
        pub fn cancel_request(origin: OriginFor<T>, id: T::RequestId) -> DispatchResult {
            let who = ensure_signed(origin)?;

            Requests::<T>::try_mutate(id, |request| {
                let request = request.as_mut().ok_or(Error::<T>::RequestIsNotExists)?;

                ensure!(*request.owner() == who, Error::<T>::CallerIsNotRequestOwner);

                ensure!(
                    *request.status() == RequestStatus::Pending,
                    Error::<T>::RequestAlreadyProcessed
                );

                request.cancel()?;

                Self::deposit_event(Event::<T>::RequestCancelled { id });

                Ok(())
            })
        }

        #[pallet::call_index(10)]
        #[pallet::weight(<T as Config>::WeightInfo::approve_deposit_request())]
        pub fn approve_deposit_request(origin: OriginFor<T>, id: T::RequestId) -> DispatchResult {
            let manager = ensure_signed(origin)?;
            Self::ensure_is_manager(&manager)?;

            Requests::<T>::try_mutate(id, |request| {
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

                Self::deposit_event(Event::<T>::RequestApproved { id, by: manager });

                Ok(())
            })
        }

        #[pallet::call_index(11)]
        #[pallet::weight(<T as Config>::WeightInfo::approve_withdraw_request())]
        pub fn approve_withdraw_request(
            origin: OriginFor<T>,
            id: T::RequestId,
            payment_reference: BoundedString<T::MaxRequestPaymentReferenceSize>,
        ) -> DispatchResult {
            let manager = ensure_signed(origin)?;
            Self::ensure_is_manager(&manager)?;

            Requests::<T>::try_mutate(id, |request| {
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

                Self::deposit_event(Event::<T>::RequestApproved { id, by: manager });

                Ok(())
            })
        }

        #[pallet::call_index(12)]
        #[pallet::weight(<T as Config>::WeightInfo::decline_request())]
        pub fn decline_request(origin: OriginFor<T>, id: T::RequestId) -> DispatchResult {
            let manager = ensure_signed(origin)?;
            Self::ensure_is_manager(&manager)?;

            Requests::<T>::try_mutate(id, |request| {
                let request = request.as_mut().ok_or(Error::<T>::RequestIsNotExists)?;

                ensure!(
                    *request.status() == RequestStatus::Pending,
                    Error::<T>::RequestAlreadyProcessed
                );

                request.decline(manager.clone())?;

                Self::deposit_event(Event::<T>::RequestDeclined { id, by: manager });

                Ok(())
            })
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
}
