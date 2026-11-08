// This file is part of the SORA network and Polkaswap app.
// SPDX-License-Identifier: BSD-4-Clause

//! # Bridge halt
//!
//! An emergency brake for every SORA bridge.
//!
//! Governance keeps a short list of guardian accounts. A guardian calls
//! [`Pallet::halt`] with a short message. The call sets the persistent bridge
//! latch [`common::recovery::BRIDGE_HALT_STORAGE_KEY`]. HASHI (`eth-bridge`),
//! `bridge-proxy`, the inbound and outbound channels, the bridge applications,
//! the data signer and the multisig verifier already check that latch: while it
//! is present they refuse new bridge work, and the HASHI offchain workers stop.
//! The message is kept in [`LastHalt`] and in [`Event::BridgeHalted`].
//!
//! The SORA runtime charges guardians no fee for `halt`. It rejects the call from
//! every other signer while validating the transaction, through
//! [`Pallet::validate_halt`], so another account's attempt never reaches a block:
//! it changes no state and costs no fee. The checks inside `halt` remain the
//! authority for wrapped calls (batch, multisig, scheduler), which pay their own
//! fees as usual.
//!
//! Guardians cannot reopen a bridge. Only [`Pallet::resume`], restricted to
//! `ResumeOrigin`, clears the latch. Clearing the raw key with Root
//! `System::kill_storage` has the same effect.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub use pallet::*;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
pub mod weights;

#[frame_support::pallet]
pub mod pallet {
    use crate::weights::WeightInfo;
    use common::recovery::BRIDGE_HALT_STORAGE_KEY;
    use frame_support::{pallet_prelude::*, BoundedBTreeSet};
    use frame_system::pallet_prelude::*;
    use sp_runtime::transaction_validity::InvalidTransaction;

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        #[allow(deprecated)]
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Adds and removes guardians.
        type GuardianOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        /// Clears the halt latch so the bridges can run again.
        type ResumeOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        /// Upper bound on the number of guardians.
        #[pallet::constant]
        type MaxGuardians: Get<u32>;

        /// Upper bound, in bytes, on the message a guardian attaches to a halt.
        #[pallet::constant]
        type MaxReasonLength: Get<u32>;

        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    /// The message a guardian attaches to a halt. UTF-8 by convention, not enforced.
    pub type ReasonOf<T> = BoundedVec<u8, <T as Config>::MaxReasonLength>;

    /// Who halted the bridges, when, and why.
    #[derive(
        Clone, Encode, Decode, DecodeWithMemTracking, Eq, PartialEq, Debug, TypeInfo, MaxEncodedLen,
    )]
    pub struct HaltRecord<AccountId, BlockNumber, Reason> {
        pub guardian: AccountId,
        pub block: BlockNumber,
        pub reason: Reason,
    }

    pub type HaltRecordOf<T> =
        HaltRecord<<T as frame_system::Config>::AccountId, BlockNumberFor<T>, ReasonOf<T>>;

    /// Accounts that may call [`Pallet::halt`].
    #[pallet::storage]
    pub type Guardians<T: Config> =
        StorageValue<_, BoundedBTreeSet<T::AccountId, T::MaxGuardians>, ValueQuery>;

    /// The most recent successful [`Pallet::halt`]. This is history: the bridges are
    /// halted exactly while the latch is present, whoever set it, and `resume` keeps
    /// this record.
    #[pallet::storage]
    pub type LastHalt<T: Config> = StorageValue<_, HaltRecordOf<T>, OptionQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A guardian halted every bridge.
        BridgeHalted {
            guardian: T::AccountId,
            reason: ReasonOf<T>,
        },
        /// The halt latch was cleared; the bridges may run again.
        BridgeResumed,
        /// An account became a guardian.
        GuardianAdded { account: T::AccountId },
        /// An account stopped being a guardian.
        GuardianRemoved { account: T::AccountId },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The account is not a guardian.
        NotGuardian,
        /// The bridges are already halted.
        AlreadyHalted,
        /// The bridges are not halted.
        NotHalted,
        /// The account is already a guardian.
        AlreadyGuardian,
        /// The guardian list is full.
        TooManyGuardians,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Halt every bridge and record why.
        ///
        /// Only a guardian can call this, and only while the bridges are running.
        /// The SORA runtime charges no fee for it and rejects it from any other
        /// signer before it reaches a block. Operational class, so it fits even
        /// when ordinary transactions fill a block.
        #[pallet::call_index(0)]
        #[pallet::weight((<T as Config>::WeightInfo::halt(), DispatchClass::Operational))]
        pub fn halt(origin: OriginFor<T>, reason: ReasonOf<T>) -> DispatchResult {
            let guardian = ensure_signed(origin)?;
            ensure!(Self::is_guardian(&guardian), Error::<T>::NotGuardian);
            ensure!(!Self::is_halted(), Error::<T>::AlreadyHalted);
            sp_io::storage::set(BRIDGE_HALT_STORAGE_KEY, &[1]);
            LastHalt::<T>::put(HaltRecord {
                guardian: guardian.clone(),
                block: frame_system::Pallet::<T>::block_number(),
                reason: reason.clone(),
            });
            Self::deposit_event(Event::BridgeHalted { guardian, reason });
            Ok(())
        }

        /// Clear the halt latch so the bridges can run again.
        ///
        /// Whoever set the latch, this removes only that key. Bridge-specific
        /// quarantines stay in place.
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::resume())]
        pub fn resume(origin: OriginFor<T>) -> DispatchResult {
            T::ResumeOrigin::ensure_origin(origin)?;
            ensure!(Self::is_halted(), Error::<T>::NotHalted);
            sp_io::storage::clear(BRIDGE_HALT_STORAGE_KEY);
            Self::deposit_event(Event::BridgeResumed);
            Ok(())
        }

        /// Allow `account` to halt the bridges.
        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::add_guardian())]
        pub fn add_guardian(origin: OriginFor<T>, account: T::AccountId) -> DispatchResult {
            T::GuardianOrigin::ensure_origin(origin)?;
            Guardians::<T>::try_mutate(|guardians| match guardians.try_insert(account.clone()) {
                Ok(true) => Ok(()),
                Ok(false) => Err(Error::<T>::AlreadyGuardian),
                Err(_) => Err(Error::<T>::TooManyGuardians),
            })?;
            Self::deposit_event(Event::GuardianAdded { account });
            Ok(())
        }

        /// Stop `account` from halting the bridges.
        #[pallet::call_index(3)]
        #[pallet::weight(<T as Config>::WeightInfo::remove_guardian())]
        pub fn remove_guardian(origin: OriginFor<T>, account: T::AccountId) -> DispatchResult {
            T::GuardianOrigin::ensure_origin(origin)?;
            Guardians::<T>::try_mutate(|guardians| {
                if guardians.remove(&account) {
                    Ok(())
                } else {
                    Err(Error::<T>::NotGuardian)
                }
            })?;
            Self::deposit_event(Event::GuardianRemoved { account });
            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        pub fn is_guardian(account: &T::AccountId) -> bool {
            Guardians::<T>::get().contains(account)
        }

        /// The bridges are halted while the latch key exists, whatever its value.
        pub fn is_halted() -> bool {
            sp_io::storage::exists(BRIDGE_HALT_STORAGE_KEY)
        }

        /// Transaction admission for a direct, signed `halt`.
        ///
        /// The runtime runs this in pool validation and again before dispatch,
        /// and charges no fee when it passes. Rejecting here, rather than failing
        /// in dispatch, keeps other signers' attempts out of blocks entirely and
        /// stops a guardian from repeating the free call once the bridges are halted.
        pub fn validate_halt(account: &T::AccountId) -> Result<(), TransactionValidityError> {
            if !Self::is_guardian(account) {
                return Err(InvalidTransaction::BadSigner.into());
            }
            if Self::is_halted() {
                return Err(InvalidTransaction::Stale.into());
            }
            Ok(())
        }
    }
}
