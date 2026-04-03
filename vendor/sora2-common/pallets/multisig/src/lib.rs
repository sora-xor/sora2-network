// This file is part of Substrate.

// Copyright (C) 2019-2020 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! # Multisig Pallet
//! A module for doing multisig dispatch.
//!
//! - [`multisig::Config`](./trait.Config.html)
//! - [`Call`](./enum.Call.html)
//!
//! ## Overview
//!
//! This module contains functionality for multi-signature dispatch, a (potentially) stateful
//! operation, allowing multiple signed
//! origins (accounts) to coordinate and dispatch a call from a well-known origin, derivable
//! deterministically from the set of account IDs and the threshold number of accounts from the
//! set that must approve it. In the case that the threshold is just one then this is a stateless
//! operation. This is useful for multisig wallets where cryptographic threshold signatures are
//! not available or desired.
//!
//! ## Interface
//!
//! ### Dispatchable Functions
//!
//! * `as_multi` - Approve and if possible dispatch a call from a composite origin formed from a
//!   number of signed origins.
//! * `approve_as_multi` - Approve a call from a composite origin.
//! * `cancel_as_multi` - Cancel a call from a composite origin.
//!
//! [`Call`]: ./enum.Call.html
//! [`Config`]: ./trait.Config.html

// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;
mod benchmarking;
#[cfg(test)]
mod tests;
pub mod weights;

use codec::{Decode, Encode};
use frame_support::{
    dispatch::{DispatchResultWithPostInfo, GetDispatchInfo, PostDispatchInfo},
    traits::{Currency, Get, ReservableCurrency},
    weights::{
        constants::{WEIGHT_REF_TIME_PER_MICROS, WEIGHT_REF_TIME_PER_NANOS},
        Weight,
    },
};
use frame_support::{ensure, Parameter};
use frame_system::pallet_prelude::BlockNumberFor;
use frame_system::{self as system, ensure_signed, RawOrigin};
use scale_info::prelude::vec;
use scale_info::TypeInfo;
use sp_io::hashing::blake2_256;
use sp_runtime::DispatchError;
use sp_runtime::RuntimeDebug;
use sp_runtime::{
    traits::{Dispatchable, Zero},
    Percent,
};
use sp_runtime::{Deserialize, Serialize};
use sp_std::prelude::*;

type BalanceOf<T> =
    <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
/// Just a bunch of bytes, but they should decode to a valid `Call`.
pub type OpaqueCall = Vec<u8>;

pub use pallet::*;

const WEIGHT_PER_MICROS: Weight = Weight::from_parts(WEIGHT_REF_TIME_PER_MICROS, 0);
const WEIGHT_PER_NANOS: Weight = Weight::from_parts(WEIGHT_REF_TIME_PER_NANOS, 0);

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use crate::weights::{pays_no, pays_no_with_maybe_weight, WeightInfo};
    use frame_support::{dispatch::DispatchResultWithPostInfo, pallet_prelude::*};
    use frame_system::pallet_prelude::*;
    use sp_std::fmt::Debug;

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::config]
    /// Configuration trait.
    pub trait Config: frame_system::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The overarching call type.
        type RuntimeCall: Parameter
            + Dispatchable<RuntimeOrigin = Self::RuntimeOrigin, PostInfo = PostDispatchInfo>
            + GetDispatchInfo
            + From<frame_system::Call<Self>>
            + Debug;

        /// The currency mechanism.
        type Currency: ReservableCurrency<Self::AccountId>;

        /// The base amount of currency needed to reserve for creating a multisig execution or to store
        /// a dispatch call for later.
        ///
        /// This is held for an additional storage item whose value size is
        /// `4 + sizeof((BlockNumber, Balance, AccountId))` bytes and whose key size is
        /// `32 + sizeof(AccountId)` bytes.
        type DepositBase: Get<BalanceOf<Self>>;

        /// The amount of currency needed per unit threshold when creating a multisig execution.
        ///
        /// This is held for adding 32 bytes more into a pre-existing storage value.
        type DepositFactor: Get<BalanceOf<Self>>;

        /// The maximum amount of signatories allowed in the multisig.
        type MaxSignatories: Get<u16>;

        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Create a new multisig account.
        /// TODO: update weights for `register_multisig`
        /// # <weight>
        /// Key: M - length of members,
        /// - One storage reads - O(1)
        /// - One search in sorted list - O(logM)
        /// - Confirmation that the list is sorted - O(M)
        /// - One storage writes - O(1)
        /// - One event
        /// Total Complexity: O(M + logM)
        /// # <weight>
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(0, 0))]
        pub fn register_multisig(
            origin: OriginFor<T>,
            signatories: Vec<T::AccountId>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            Self::register_multisig_inner(who, signatories)?;
            Ok(().into())
        }

        /// Remove the signatory from the multisig account.
        /// Can only be called by a multisig account.
        ///
        /// TODO: update weights for `remove_signatory`
        /// # <weight>
        /// Key: length of members in multisigConfig: M
        /// - One storage reads - O(1)
        /// - remove items in list - O(M)
        /// Total complexity - O(M)
        /// # <weight>
        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(0, 0))]
        pub fn remove_signatory(
            origin: OriginFor<T>,
            signatory: T::AccountId,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            <Accounts<T>>::mutate(&who, |opt| {
                let multisig = opt.as_mut().ok_or(Error::<T>::UnknownMultisigAccount)?;
                // remove the signatory
                let pos = multisig
                    .signatories
                    .binary_search(&signatory)
                    .map_err(|_| Error::<T>::NotInSignatories)?;
                multisig.signatories.remove(pos);
                // remove the signatory's approvals
                let mut total_weight = Weight::zero();
                let updated_ops = Multisigs::<T>::iter_prefix(&who).filter_map(
                    |(call_hash, mut operation): (_, Multisig<_, _, _>)| {
                        let timepoint = operation.when;
                        let search_res = operation.approvals.binary_search(&signatory);
                        if let Ok(pos) = search_res {
                            operation.approvals.remove(pos);
                        }
                        let approvals = operation.approvals.len() as u16;
                        let threshold = multisig.threshold_num();
                        if approvals >= threshold {
                            if !DispatchedCalls::<T>::contains_key(&call_hash, timepoint) {
                                if let Some(call) = Self::get_call(&call_hash, None) {
                                    total_weight += Pallet::<T>::dispatch_call(
                                        &who,
                                        &who,
                                        multisig.signatories.len(),
                                        call_hash,
                                        1,
                                        timepoint,
                                        call,
                                    )
                                    .unwrap_or(Weight::zero());
                                    return Some((call_hash, operation, true));
                                }
                            }
                        }
                        if search_res.is_ok() {
                            Some((call_hash, operation, false))
                        } else {
                            None
                        }
                    },
                );
                for (hash, op, dispatched) in updated_ops {
                    if dispatched {
                        Multisigs::<T>::remove(&who, &hash);
                    } else {
                        Multisigs::<T>::insert(&who, &hash, op);
                    }
                }
                pays_no::<_, DispatchError>(Ok(()))
            })
        }

        /// Add a new signatory to the multisig account.
        /// Can only be called by a multisig account.
        ///
        /// TODO: update weights for `add_signatory`
        /// # <weight>
        /// Key: length of members in multisigConfig: M
        /// - One storage read - O(1)
        /// - search in members - O(M)
        /// - Storage write - O(M)
        /// Total complexity - O(M)
        /// # <weight>
        #[pallet::call_index(2)]
        #[pallet::weight(Weight::from_parts(0, 0))]
        pub fn add_signatory(
            origin: OriginFor<T>,
            new_member: T::AccountId,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            <Accounts<T>>::mutate(&who, |opt| {
                let multisig = opt.as_mut().ok_or(Error::<T>::UnknownMultisigAccount)?;
                ensure!(
                    !multisig.signatories.contains(&new_member),
                    Error::<T>::AlreadyInSignatories
                );
                multisig.signatories.push(new_member.clone());
                multisig.signatories.sort();
                pays_no::<_, DispatchError>(Ok(()))
            })
        }

        /// Immediately dispatch a multi-signature call using a single approval from the caller.
        ///
        /// The dispatch origin for this call must be _Signed_.
        ///
        /// - `other_signatories`: The accounts (other than the sender) who are part of the
        /// multi-signature, but do not participate in the approval process.
        /// - `call`: The call to be executed.
        ///
        /// Result is equivalent to the dispatched result.
        ///
        /// # <weight>
        /// O(Z + C) where Z is the length of the call and C its execution weight.
        /// -------------------------------
        /// - Base Weight: 33.72 + 0.002 * Z µs
        /// - DB Weight: None
        /// - Plus Call Weight
        /// # </weight>
        #[pallet::call_index(3)]
        #[pallet::weight(Weight::from_parts(0, 0))]
        pub fn as_multi_threshold_1(
            origin: OriginFor<T>,
            id: T::AccountId,
            call: Box<<T as Config>::RuntimeCall>,
            timepoint: BridgeTimepoint<BlockNumberFor<T>>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let multisig: MultisigAccount<T::AccountId> = Self::accounts(&id).unwrap();
            ensure!(
                multisig.threshold_num() == 1,
                Error::<T>::TooManySignatories
            );
            let signatories = multisig.signatories;
            ensure!(signatories.contains(&who), Error::<T>::NotInSignatories);

            pays_no_with_maybe_weight(Self::inner_as_multi_threshold_1(id, call, timepoint))
        }

        /// Register approval for a dispatch to be made from a deterministic composite account if
        /// approved by a total of `threshold - 1` of `other_signatories`.
        ///
        /// If there are enough, then dispatch the call.
        ///
        /// Payment: `DepositBase` will be reserved if this is the first approval, plus
        /// `threshold` times `DepositFactor`. It is returned once this dispatch happens or
        /// is cancelled.
        ///
        /// The dispatch origin for this call must be _Signed_.
        ///
        /// - `threshold`: The total number of approvals for this dispatch before it is executed.
        /// - `other_signatories`: The accounts (other than the sender) who can approve this
        /// dispatch. May not be empty.
        /// - `maybe_timepoint`: If this is the first approval, then this must be `None`. If it is
        /// not the first approval, then it must be `Some`, with the timepoint (block number and
        /// transaction index) of the first approval transaction.
        /// - `call`: The call to be executed.
        ///
        /// NOTE: Unless this is the final approval, you will generally want to use
        /// `approve_as_multi` instead, since it only requires a hash of the call.
        ///
        /// Result is equivalent to the dispatched result if `threshold` is exactly `1`. Otherwise
        /// on success, result is `Ok` and the result from the interior call, if it was executed,
        /// may be found in the deposited `MultisigExecuted` event.
        ///
        /// # <weight>
        /// - `O(S + Z + Call)`.
        /// - Up to one balance-reserve or unreserve operation.
        /// - One passthrough operation, one insert, both `O(S)` where `S` is the number of
        ///   signatories. `S` is capped by `MaxSignatories`, with weight being proportional.
        /// - One call encode & hash, both of complexity `O(Z)` where `Z` is tx-len.
        /// - One encode & hash, both of complexity `O(S)`.
        /// - Up to one binary search and insert (`O(logS + S)`).
        /// - I/O: 1 read `O(S)`, up to 1 mutate `O(S)`. Up to one remove.
        /// - One event.
        /// - The weight of the `call`.
        /// - Storage: inserts one item, value size bounded by `MaxSignatories`, with a
        ///   deposit taken for its lifetime of
        ///   `DepositBase + threshold * DepositFactor`.
        /// -------------------------------
        /// - Base Weight:
        ///     - Create:          41.89 + 0.118 * S + .002 * Z µs
        ///     - Create w/ Store: 53.57 + 0.119 * S + .003 * Z µs
        ///     - Approve:         31.39 + 0.136 * S + .002 * Z µs
        ///     - Complete:        39.94 + 0.26  * S + .002 * Z µs
        /// - DB Weight:
        ///     - Reads: Multisig Storage, [Caller Account], Calls (if `store_call`)
        ///     - Writes: Multisig Storage, [Caller Account], Calls (if `store_call`)
        /// - Plus Call Weight
        /// # </weight>
        #[pallet::call_index(4)]
        #[pallet::weight({
            let s = Accounts::<T>::get(&id).map(|x| x.signatories.len() as u32).unwrap_or(0);
            let z = call.len() as u32;

            let w = T::WeightInfo::as_multi_create(s, z)
            .max(T::WeightInfo::as_multi_approve(s, z))
            .max(T::WeightInfo::as_multi_complete(s, z));
            w
        })]
        pub fn as_multi(
            origin: OriginFor<T>,
            id: T::AccountId,
            maybe_timepoint: Option<BridgeTimepoint<BlockNumberFor<T>>>,
            call: OpaqueCall,
            store_call: bool,
            max_weight: Weight,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let multisig: MultisigAccount<T::AccountId> =
                Self::accounts(&id).ok_or(Error::<T>::UnknownMultisigAccount)?;
            let threshold = multisig.threshold_num();
            ensure!(threshold > 1, Error::<T>::MinimumThreshold);
            ensure!(
                multisig.is_signatory(&who),
                Error::<T>::SenderNotInSignatories
            );
            pays_no_with_maybe_weight(
                Self::operate(
                    multisig,
                    threshold,
                    who,
                    id,
                    maybe_timepoint,
                    CallOrHash::Call(call, store_call),
                    max_weight,
                )
                .map_err(|e| (None, e)),
            )
        }

        /// Register approval for a dispatch to be made from a deterministic composite account if
        /// approved by a total of `threshold - 1` of `other_signatories`.
        ///
        /// Payment: `DepositBase` will be reserved if this is the first approval, plus
        /// `threshold` times `DepositFactor`. It is returned once this dispatch happens or
        /// is cancelled.
        ///
        /// The dispatch origin for this call must be _Signed_.
        ///
        /// - `threshold`: The total number of approvals for this dispatch before it is executed.
        /// - `other_signatories`: The accounts (other than the sender) who can approve this
        /// dispatch. May not be empty.
        /// - `maybe_timepoint`: If this is the first approval, then this must be `None`. If it is
        /// not the first approval, then it must be `Some`, with the timepoint (block number and
        /// transaction index) of the first approval transaction.
        /// - `call_hash`: The hash of the call to be executed.
        ///
        /// NOTE: If this is the final approval, you will want to use `as_multi` instead.
        ///
        /// # <weight>
        /// - `O(S)`.
        /// - Up to one balance-reserve or unreserve operation.
        /// - One passthrough operation, one insert, both `O(S)` where `S` is the number of
        ///   signatories. `S` is capped by `MaxSignatories`, with weight being proportional.
        /// - One encode & hash, both of complexity `O(S)`.
        /// - Up to one binary search and insert (`O(logS + S)`).
        /// - I/O: 1 read `O(S)`, up to 1 mutate `O(S)`. Up to one remove.
        /// - One event.
        /// - Storage: inserts one item, value size bounded by `MaxSignatories`, with a
        ///   deposit taken for its lifetime of
        ///   `DepositBase + threshold * DepositFactor`.
        /// ----------------------------------
        /// - Base Weight:
        ///     - Create: 44.71 + 0.088 * S
        ///     - Approve: 31.48 + 0.116 * S
        /// - DB Weight:
        ///     - Read: Multisig Storage, [Caller Account]
        ///     - Write: Multisig Storage, [Caller Account]
        /// # </weight>
        #[pallet::call_index(5)]
        #[pallet::weight(Weight::from_parts(0, 0))]
        pub fn approve_as_multi(
            origin: OriginFor<T>,
            id: T::AccountId,
            maybe_timepoint: Option<BridgeTimepoint<BlockNumberFor<T>>>,
            call_hash: [u8; 32],
            max_weight: Weight,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let multisig: MultisigAccount<T::AccountId> =
                Self::accounts(&id).ok_or(Error::<T>::UnknownMultisigAccount)?;
            let threshold = multisig.threshold_num();
            ensure!(threshold > 1, Error::<T>::MinimumThreshold);
            ensure!(
                multisig.is_signatory(&who),
                Error::<T>::SenderNotInSignatories
            );
            pays_no_with_maybe_weight(
                Self::operate(
                    multisig,
                    threshold,
                    who,
                    id,
                    maybe_timepoint,
                    CallOrHash::Hash(call_hash),
                    max_weight,
                )
                .map_err(|e| (None, e)),
            )
        }

        /// Cancel a pre-existing, on-going multisig transaction. Any deposit reserved previously
        /// for this operation will be unreserved on success.
        ///
        /// The dispatch origin for this call must be _Signed_.
        ///
        /// - `threshold`: The total number of approvals for this dispatch before it is executed.
        /// - `other_signatories`: The accounts (other than the sender) who can approve this
        /// dispatch. May not be empty.
        /// - `timepoint`: The timepoint (block number and transaction index) of the first approval
        /// transaction for this dispatch.
        /// - `call_hash`: The hash of the call to be executed.
        ///
        /// # <weight>
        /// - `O(S)`.
        /// - Up to one balance-reserve or unreserve operation.
        /// - One passthrough operation, one insert, both `O(S)` where `S` is the number of
        ///   signatories. `S` is capped by `MaxSignatories`, with weight being proportional.
        /// - One encode & hash, both of complexity `O(S)`.
        /// - One event.
        /// - I/O: 1 read `O(S)`, one remove.
        /// - Storage: removes one item.
        /// ----------------------------------
        /// - Base Weight: 36.07 + 0.124 * S
        /// - DB Weight:
        ///     - Read: Multisig Storage, [Caller Account], Refund Account, Calls
        ///     - Write: Multisig Storage, [Caller Account], Refund Account, Calls
        /// # </weight>
        #[pallet::call_index(6)]
        #[pallet::weight(Weight::from_parts(0, 0))]
        pub fn cancel_as_multi(
            origin: OriginFor<T>,
            id: T::AccountId,
            timepoint: BridgeTimepoint<BlockNumberFor<T>>,
            call_hash: [u8; 32],
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let multisig = Self::accounts(&id).ok_or(Error::<T>::UnknownMultisigAccount)?;
            let threshold = multisig.threshold_num();
            ensure!(threshold > 1, Error::<T>::MinimumThreshold);
            ensure!(
                !DispatchedCalls::<T>::contains_key(&call_hash, timepoint),
                Error::<T>::AlreadyDispatched
            );

            let m = <Multisigs<T>>::get(&id, call_hash).ok_or(Error::<T>::NotFound)?;
            ensure!(m.when == timepoint, Error::<T>::WrongTimepoint);
            ensure!(m.depositor == who, Error::<T>::NotOwner);

            <Multisigs<T>>::remove(&id, &call_hash);
            Self::clear_call(&call_hash);

            Self::deposit_event(Event::MultisigCancelled(who, timepoint, id, call_hash));
            Ok(().into())
        }
    }

    /// Events type.
    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A new multisig created. [multisig]
        MultisigAccountCreated(T::AccountId),
        /// A new multisig operation has begun. [approving, multisig, call_hash]
        NewMultisig(T::AccountId, T::AccountId, [u8; 32]),
        /// A multisig operation has been approved by someone. [approving, timepoint, multisig, call_hash]
        MultisigApproval(
            T::AccountId,
            BridgeTimepoint<BlockNumberFor<T>>,
            T::AccountId,
            [u8; 32],
        ),
        /// A multisig operation has been executed. [approving, timepoint, multisig, call_hash]
        MultisigExecuted(
            T::AccountId,
            BridgeTimepoint<BlockNumberFor<T>>,
            T::AccountId,
            [u8; 32],
            Option<DispatchError>,
        ),
        /// A multisig operation has been cancelled. [cancelling, timepoint, multisig, call_hash]
        MultisigCancelled(
            T::AccountId,
            BridgeTimepoint<BlockNumberFor<T>>,
            T::AccountId,
            [u8; 32],
        ),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Threshold must be 2 or greater.
        MinimumThreshold,
        /// Call is already approved by this signatory.
        AlreadyApproved,
        /// Call doesn't need any (more) approvals.
        NoApprovalsNeeded,
        /// There are too few signatories in the list.
        TooFewSignatories,
        /// There are too many signatories in the list.
        TooManySignatories,
        /// The signatories were provided out of order; they should be ordered.
        SignatoriesOutOfOrder,
        /// The sender wasn't contained in the other signatories; it shouldn be.
        SenderNotInSignatories,
        /// The given account ID is not presented in the signatories.
        NotInSignatories,
        /// The given account ID is already presented in the signatories.
        AlreadyInSignatories,
        /// Multisig operation not found when attempting to cancel.
        NotFound,
        /// Only the account that originally created the multisig is able to cancel it.
        NotOwner,
        /// No timepoint was given, yet the multisig operation is already underway.
        NoTimepoint,
        /// A different timepoint was given to the multisig operation that is underway.
        WrongTimepoint,
        /// A timepoint was given, yet no multisig operation is underway.
        UnexpectedTimepoint,
        /// The data to be stored is already stored.
        AlreadyStored,
        /// The maximum weight information provided was too low.
        WeightTooLow,
        /// Threshold should not be zero.
        ZeroThreshold,
        /// The multisig account is already registered.
        MultisigAlreadyExists,
        /// Corresponding multisig account wasn't found.
        UnknownMultisigAccount,
        /// Signatories list unordered or contains duplicated entries.
        SignatoriesAreNotUniqueOrUnordered,
        /// Call with the given hash was already dispatched.
        AlreadyDispatched,
    }

    /// Multisignature accounts.
    #[pallet::storage]
    #[pallet::getter(fn accounts)]
    pub type Accounts<T: Config> =
        StorageMap<_, Twox64Concat, T::AccountId, MultisigAccount<T::AccountId>>;

    /// The set of open multisig operations.
    #[pallet::storage]
    pub type Multisigs<T: Config> = StorageDoubleMap<
        _,
        Twox64Concat,
        T::AccountId,
        Blake2_128Concat,
        [u8; 32],
        Multisig<BlockNumberFor<T>, BalanceOf<T>, T::AccountId>,
    >;

    #[pallet::storage]
    pub type Calls<T: Config> =
        StorageMap<_, Identity, [u8; 32], (OpaqueCall, T::AccountId, BalanceOf<T>)>;

    #[pallet::storage]
    pub type DispatchedCalls<T: Config> = StorageDoubleMap<
        _,
        Identity,
        [u8; 32],
        Twox64Concat,
        BridgeTimepoint<BlockNumberFor<T>>,
        (),
        ValueQuery,
    >;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub accounts: Vec<(T::AccountId, MultisigAccount<T::AccountId>)>,
    }

    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                accounts: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            {
                let data = &self.accounts;
                let data: &vec::Vec<(T::AccountId, MultisigAccount<T::AccountId>)> = data;
                data.iter().for_each(|(k, v)| {
                    <Accounts<T> as frame_support::storage::StorageMap<
                        T::AccountId,
                        MultisigAccount<T::AccountId>,
                    >>::insert::<&T::AccountId, &MultisigAccount<T::AccountId>>(
                        k, v
                    );
                });
            }
        }
    }
}

/// Represents height on either Thischain of Sidechain.
#[derive(Copy, Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum MultiChainHeight<BlockNumber> {
    Thischain(BlockNumber),
    Sidechain(u64),
}

impl<BlockNumber: Default> Default for MultiChainHeight<BlockNumber> {
    fn default() -> Self {
        MultiChainHeight::Thischain(BlockNumber::default())
    }
}

/// A global extrinsic index, formed as the extrinsic index within a block, together with that
/// block's height. This allows a transaction in which a multisig operation of a particular
/// composite was created to be uniquely identified.
#[derive(Copy, Clone, Eq, PartialEq, Encode, Decode, Default, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct BridgeTimepoint<BlockNumber> {
    /// The height of the chain at the point in time.
    pub height: MultiChainHeight<BlockNumber>,
    /// The index of the extrinsic at the point in time.
    pub index: u32,
}

/// An open multisig operation.
#[derive(Clone, Eq, PartialEq, Encode, Decode, Default, RuntimeDebug, TypeInfo)]
pub struct Multisig<BlockNumber, Balance, AccountId> {
    /// The extrinsic when the multisig operation was opened.
    pub when: BridgeTimepoint<BlockNumber>,
    /// The amount held in reserve of the `depositor`, to be returned once the operation ends.
    pub deposit: Balance,
    /// The account who opened it (i.e. the first to approve it).
    pub depositor: AccountId,
    /// The approvals achieved so far, including the depositor. Always sorted.
    pub approvals: Vec<AccountId>,
}

#[derive(
    Clone, Eq, PartialEq, Encode, Decode, Default, RuntimeDebug, TypeInfo, Serialize, Deserialize,
)]
pub struct MultisigAccount<AccountId> {
    /// Parties of the account.
    pub signatories: Vec<AccountId>,
    /// Threshold represented in percents. Once reached,
    /// the proposal will be executed.
    ///
    /// NOTE: currently unused.
    pub threshold: Percent,
}

impl<AccountId: Ord> MultisigAccount<AccountId> {
    pub fn new(mut signatories: Vec<AccountId>) -> Self {
        signatories.sort();
        MultisigAccount {
            signatories,
            threshold: Percent::from_percent(67),
        }
    }
}

impl<AccountId: PartialEq + Ord + Encode> MultisigAccount<AccountId> {
    pub fn is_signatory(&self, who: &AccountId) -> bool {
        self.signatories.binary_search(who).is_ok()
    }

    /// Number of signatories needed for a proposal execution.
    pub fn threshold_num(&self) -> u16 {
        let signatories_count = self.signatories.len() as u16;
        signatories_count - (signatories_count - 1) / 3
    }
}

mod weight_of {
    use super::*;

    /// - Base Weight: 33.72 + 0.002 * Z µs
    /// - DB Weight: None
    /// - Plus Call Weight
    pub fn as_multi_threshold_1<T: Config>(call_len: usize, call_weight: Weight) -> Weight {
        WEIGHT_PER_MICROS
            .mul(34)
            .saturating_add((WEIGHT_PER_NANOS.mul(2)).saturating_mul(call_len as u64))
            .saturating_add(call_weight)
    }

    /// - Base Weight:
    ///     - Create:          38.82 + 0.121 * S + .001 * Z µs
    ///     - Create w/ Store: 54.22 + 0.120 * S + .003 * Z µs
    ///     - Approve:         29.86 + 0.143 * S + .001 * Z µs
    ///     - Complete:        39.55 + 0.267 * S + .002 * Z µs
    /// - DB Weight:
    ///     - Reads: Multisig Storage, [Caller Account], Calls, Depositor Account
    ///     - Writes: Multisig Storage, [Caller Account], Calls, Depositor Account
    /// - Plus Call Weight
    pub fn as_multi<T: Config>(
        sig_len: usize,
        call_len: usize,
        call_weight: Weight,
        calls_write: bool,
        refunded: bool,
    ) -> Weight {
        call_weight
            .saturating_add(WEIGHT_PER_MICROS.mul(55))
            .saturating_add((WEIGHT_PER_NANOS.mul(250)).saturating_mul(sig_len as u64))
            .saturating_add((WEIGHT_PER_NANOS.mul(3)).saturating_mul(call_len as u64))
            .saturating_add(T::DbWeight::get().reads_writes(1, 1)) // Multisig read/write
            .saturating_add(T::DbWeight::get().reads(1)) // Calls read
            .saturating_add(T::DbWeight::get().writes(calls_write.into())) // Calls write
            .saturating_add(T::DbWeight::get().reads_writes(refunded.into(), refunded.into()))
        // Deposit refunded
    }
}

enum CallOrHash {
    Call(OpaqueCall, bool),
    Hash([u8; 32]),
}

impl<T: Config> Pallet<T> {
    pub fn register_multisig_inner(
        creator: T::AccountId,
        signatories: Vec<T::AccountId>,
    ) -> Result<T::AccountId, DispatchError> {
        let block_num = <system::Pallet<T>>::block_number();
        let nonce = <system::Pallet<T>>::account_nonce(&creator);
        let multisig_account_id = Self::multi_account_id(&creator, block_num, nonce);
        ensure!(
            Self::accounts(&multisig_account_id).is_none(),
            Error::<T>::MultisigAlreadyExists
        );
        let max_sigs = T::MaxSignatories::get() as usize;
        ensure!(
            signatories.len() <= max_sigs,
            Error::<T>::TooManySignatories
        );
        ensure!(
            Self::is_sort_and_unique(&signatories),
            Error::<T>::SignatoriesAreNotUniqueOrUnordered
        );
        let multisig_config = MultisigAccount::new(signatories);
        <Accounts<T>>::insert(&multisig_account_id, multisig_config);
        frame_system::Pallet::<T>::inc_providers(&multisig_account_id);
        Self::deposit_event(Event::MultisigAccountCreated(multisig_account_id.clone()));
        Ok(multisig_account_id)
    }

    /// Derive a multi-account ID from the sorted list of accounts and the threshold that are
    /// required.
    ///
    /// NOTE: `who` must be sorted. If it is not, then you'll get the wrong answer.
    pub fn multi_account_id(
        creator: &T::AccountId,
        block_number: BlockNumberFor<T>,
        salt: T::Nonce,
    ) -> T::AccountId {
        let entropy = (b"modlpy/utilisuba", creator, block_number, salt).using_encoded(blake2_256);
        T::AccountId::decode(&mut &entropy[..]).unwrap()
    }

    fn is_sort_and_unique(members: &[T::AccountId]) -> bool {
        members.windows(2).all(|m| m[0] < m[1])
    }

    fn operate(
        multisig: MultisigAccount<T::AccountId>,
        threshold: u16,
        who: T::AccountId,
        id: T::AccountId,
        maybe_timepoint: Option<BridgeTimepoint<BlockNumberFor<T>>>,
        call_or_hash: CallOrHash,
        max_weight: Weight,
    ) -> Result<Option<Weight>, DispatchError> {
        let signatories = multisig.signatories;
        let signatories_len = signatories.len();

        // Threshold > 1; this means it's a multi-step operation. We extract the `call_hash`.
        let (call_hash, call_len, maybe_call, store) = match call_or_hash {
            CallOrHash::Call(call, should_store) => {
                let call_hash = blake2_256(&call);
                let call_len = call.len();
                (call_hash, call_len, Some(call), should_store)
            }
            CallOrHash::Hash(h) => (h, 0, None, false),
        };

        // Branch on whether the operation has already started or not.
        if let Some(mut m) = <Multisigs<T>>::get(&id, call_hash) {
            // Yes; ensure that the timepoint exists and agrees.
            let timepoint = maybe_timepoint.ok_or(Error::<T>::NoTimepoint)?;
            ensure!(m.when == timepoint, Error::<T>::WrongTimepoint);
            ensure!(
                !DispatchedCalls::<T>::contains_key(&call_hash, timepoint),
                Error::<T>::AlreadyDispatched
            );

            // Ensure that either we have not yet signed or that it is at threshold.
            let mut approvals = m.approvals.len() as u16;
            // We only bother with the approval if we're below threshold.
            let maybe_pos = m
                .approvals
                .binary_search(&who)
                .err()
                .filter(|_| approvals < threshold);
            // Bump approvals if not yet voted and the vote is needed.
            if maybe_pos.is_some() {
                approvals += 1;
            }

            // We only bother fetching/decoding call if we know that we're ready to execute.
            let maybe_approved_call = if approvals >= threshold {
                Self::get_call(&call_hash, maybe_call.as_ref().map(|c| c.as_ref()))
            } else {
                None
            };

            if let Some(call) = maybe_approved_call {
                // verify weight
                ensure!(
                    max_weight.all_gte(call.get_dispatch_info().weight),
                    Error::<T>::WeightTooLow
                );
                <Multisigs<T>>::remove(&id, call_hash);
                let post_info = <Pallet<T>>::dispatch_call(
                    &who,
                    &id,
                    signatories_len,
                    call_hash,
                    call_len,
                    timepoint,
                    call,
                );
                Ok(post_info)
            } else {
                // We cannot dispatch the call now; either it isn't available, or it is, but we
                // don't have threshold approvals even with our signature.

                // Store the call if desired.
                let stored = if let Some(data) = maybe_call.filter(|_| store) {
                    Self::store_call_and_reserve(
                        who.clone(),
                        &call_hash,
                        data,
                        BalanceOf::<T>::zero(),
                    );
                    true
                } else {
                    false
                };

                if let Some(pos) = maybe_pos {
                    // Record approval.
                    m.approvals.insert(pos, who.clone());
                    <Multisigs<T>>::insert(&id, call_hash, m);
                    Self::deposit_event(Event::MultisigApproval(
                        who,
                        timepoint.clone(),
                        id,
                        call_hash,
                    ));
                } else {
                    // If we already approved and didn't store the Call, then this was useless and
                    // we report an error.
                    ensure!(stored, Error::<T>::AlreadyApproved);
                }

                // Call is not made, so the actual weight does not include call
                Ok(Some(weight_of::as_multi::<T>(
                    signatories_len,
                    call_len,
                    Weight::zero(),
                    stored, // Call stored?
                    false,  // No refund
                )))
            }
        } else {
            // Just start the operation by recording it in storage.
            let deposit = T::DepositBase::get() + T::DepositFactor::get() * threshold.into();

            // Store the call if desired.
            let stored = if let Some(data) = maybe_call.filter(|_| store) {
                Self::store_call_and_reserve(who.clone(), &call_hash, data, deposit);
                true
            } else {
                false
            };

            let timepoint = maybe_timepoint.unwrap_or_else(|| Self::thischain_timepoint());
            ensure!(
                !DispatchedCalls::<T>::contains_key(&call_hash, timepoint),
                Error::<T>::AlreadyDispatched
            );

            <Multisigs<T>>::insert(
                &id,
                call_hash,
                Multisig {
                    when: timepoint,
                    deposit,
                    depositor: who.clone(),
                    approvals: vec![who.clone()],
                },
            );
            Self::deposit_event(Event::NewMultisig(who, id, call_hash));
            // Call is not made, so we can return that weight
            return Ok(Some(weight_of::as_multi::<T>(
                signatories_len,
                call_len,
                Weight::zero(),
                stored, // Call stored?
                false,  // No refund
            )));
        }
    }

    fn inner_as_multi_threshold_1(
        id: T::AccountId,
        call: Box<<T as Config>::RuntimeCall>,
        timepoint: BridgeTimepoint<BlockNumberFor<T>>,
    ) -> Result<Option<Weight>, (Option<Weight>, DispatchError)> {
        let (call_len, call_hash) = call.using_encoded(|c| (c.len(), blake2_256(c)));
        ensure!(
            !DispatchedCalls::<T>::contains_key(&call_hash, timepoint.clone()),
            (None, Error::<T>::AlreadyDispatched.into())
        );
        DispatchedCalls::<T>::insert(&call_hash, timepoint, ());
        let origin = RawOrigin::Signed(id).into();
        let result = call.dispatch(origin);
        result
            .map(|post_dispatch_info| {
                post_dispatch_info.actual_weight.map(|actual_weight| {
                    weight_of::as_multi_threshold_1::<T>(call_len, actual_weight)
                })
            })
            .map_err(|err| (err.post_info.actual_weight, err.error.into()))
    }

    fn dispatch_call(
        who: &T::AccountId,
        id: &T::AccountId,
        signatories_len: usize,
        call_hash: [u8; 32],
        call_len: usize,
        timepoint: BridgeTimepoint<BlockNumberFor<T>>,
        call: <T as Config>::RuntimeCall,
    ) -> Option<Weight> {
        // Clean up storage before executing call to avoid an possibility of reentrancy
        // attack.
        Self::clear_call(&call_hash);

        let origin = RawOrigin::Signed(id.clone()).into();
        let result = call.dispatch(origin);
        DispatchedCalls::<T>::insert(&call_hash, timepoint.clone(), ());
        Self::deposit_event(Event::MultisigExecuted(
            who.clone(),
            timepoint,
            id.clone(),
            call_hash,
            result.err().map(|x| x.error),
        ));
        get_result_weight(result).map(|actual_weight| {
            weight_of::as_multi::<T>(
                signatories_len,
                call_len,
                actual_weight,
                true, // Call is removed
                true, // User is refunded
            )
        })
    }

    /// Place a call's encoded data in storage, reserving funds as appropriate.
    ///
    /// We store `data` here because storing `call` would result in needing another `.encode`.
    ///
    /// Returns a `bool` indicating whether the data did end up being stored.
    fn store_call_and_reserve(
        who: T::AccountId,
        hash: &[u8; 32],
        data: OpaqueCall,
        other_deposit: BalanceOf<T>,
    ) {
        if !Calls::<T>::contains_key(hash) {
            let deposit = other_deposit
                + T::DepositBase::get()
                + T::DepositFactor::get() * BalanceOf::<T>::from(((data.len() + 31) / 32) as u32);
            Calls::<T>::insert(&hash, (data, who, deposit));
        }
    }

    /// Attempt to decode and return the call, provided by the user or from storage.
    fn get_call(hash: &[u8; 32], maybe_known: Option<&[u8]>) -> Option<<T as Config>::RuntimeCall> {
        maybe_known.map_or_else(
            || Calls::<T>::get(hash).and_then(|(data, ..)| Decode::decode(&mut &data[..]).ok()),
            |data| Decode::decode(&mut &data[..]).ok(),
        )
    }

    /// Attempt to remove a call from storage, returning any deposit on it to the owner.
    fn clear_call(hash: &[u8; 32]) {
        let _ = Calls::<T>::take(hash);
    }

    /// The current `BridgeTimepoint` at Thischain.
    pub fn thischain_timepoint() -> BridgeTimepoint<BlockNumberFor<T>> {
        BridgeTimepoint {
            height: MultiChainHeight::Thischain(<system::Pallet<T>>::block_number()),
            index: <system::Pallet<T>>::extrinsic_index().unwrap_or_default(),
        }
    }

    /// Construct `BridgeTimepoint` at Sidehcain.
    pub fn sidechain_timepoint(height: u64, index: u32) -> BridgeTimepoint<BlockNumberFor<T>> {
        BridgeTimepoint {
            height: MultiChainHeight::Sidechain(height),
            index,
        }
    }
}

/// Return the weight of a dispatch call result as an `Option`.
///
/// Will return the weight regardless of what the state of the result is.
fn get_result_weight(result: DispatchResultWithPostInfo) -> Option<Weight> {
    match result {
        Ok(post_info) => post_info.actual_weight,
        Err(err) => err.post_info.actual_weight,
    }
}
