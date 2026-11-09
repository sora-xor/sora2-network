// This file is part of the SORA network and Polkaswap app.
// SPDX-License-Identifier: BSD-4-Clause

//! # Bridge halt
//!
//! An emergency stop for the SORA bridge workers.
//!
//! Governance keeps a short list of guardian accounts; the runtime may install
//! the first ones in the upgrade that adds this pallet. A guardian calls
//! [`Pallet::halt`] with a short message. The call numbers the halt, keeps the
//! message in [`LastHalt`] and [`Event::BridgeHalted`], and writes the halt's
//! number and block to [`common::bridge_halt::GUARDIAN_HALT_STORAGE_KEY`].
//!
//! Bridge workers read that key. A HASHI offchain worker stops as soon as it
//! names a halt that the node's operator has not confirmed, and stays stopped
//! until devops restart the node with `--bridge-resume-after-halt <number>`.
//! Nothing on chain restarts the workers, so bringing the bridges back needs no
//! Root call, and a guardian can stop the workers but never restart them.
//!
//! A halt leaves Root's recovery latch
//! ([`common::recovery::BRIDGE_HALT_STORAGE_KEY`]) alone. When the bridge pallets
//! themselves must refuse calls, governance sets that latch with Root.
//!
//! The SORA runtime charges guardians no fee for `halt`. It rejects the call from
//! every other signer while validating the transaction, through
//! [`Pallet::validate_halt`], so another account's attempt never reaches a block:
//! it changes no state and costs no fee. After a halt, the next one is refused
//! for [`Config::HaltCooldown`] blocks, so guardians who react to the same
//! incident send one halt and a guardian key cannot fill blocks with free calls.
//! The checks inside `halt` remain the authority for wrapped calls (batch,
//! multisig, scheduler), which pay their own fees as usual.

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
    use codec::Encode;
    use common::bridge_halt::{GuardianHalt, GUARDIAN_HALT_STORAGE_KEY};
    use frame_support::{pallet_prelude::*, BoundedBTreeSet};
    use frame_system::pallet_prelude::*;
    use sp_runtime::{
        traits::{Saturating, UniqueSaturatedInto},
        transaction_validity::InvalidTransaction,
    };

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

        /// Upper bound on the number of guardians.
        #[pallet::constant]
        type MaxGuardians: Get<u32>;

        /// Upper bound, in bytes, on the message a guardian attaches to a halt.
        #[pallet::constant]
        type MaxReasonLength: Get<u32>;

        /// Blocks after a halt during which the next halt is refused.
        #[pallet::constant]
        type HaltCooldown: Get<BlockNumberFor<Self>>;

        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    /// The message a guardian attaches to a halt. UTF-8 by convention, not enforced.
    pub type ReasonOf<T> = BoundedVec<u8, <T as Config>::MaxReasonLength>;

    /// Which halt this was, who sent it, when, and why.
    #[derive(
        Clone, Encode, Decode, DecodeWithMemTracking, Eq, PartialEq, Debug, TypeInfo, MaxEncodedLen,
    )]
    pub struct HaltRecord<AccountId, BlockNumber, Reason> {
        /// 1 for the first halt, then counting up. Operators pass it to
        /// `--bridge-resume-after-halt` to restart their workers.
        pub number: u32,
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

    /// The most recent halt. Bridge workers stay stopped after it until their
    /// operators confirm its number; the chain does not record those restarts.
    #[pallet::storage]
    pub type LastHalt<T: Config> = StorageValue<_, HaltRecordOf<T>, OptionQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A guardian halted the bridge workers.
        BridgeHalted {
            number: u32,
            guardian: T::AccountId,
            reason: ReasonOf<T>,
        },
        /// An account became a guardian.
        GuardianAdded { account: T::AccountId },
        /// An account stopped being a guardian.
        GuardianRemoved { account: T::AccountId },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The account is not a guardian.
        NotGuardian,
        /// A halt was sent less than `HaltCooldown` blocks ago.
        HaltedRecently,
        /// The account is already a guardian.
        AlreadyGuardian,
        /// The guardian list is full.
        TooManyGuardians,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Stop every bridge worker and record why.
        ///
        /// Only a guardian can call this, and not within `HaltCooldown` blocks of
        /// the previous halt. The SORA runtime charges no fee for it and rejects
        /// it from any other signer before it reaches a block. Operational class,
        /// so it fits even when ordinary transactions fill a block.
        #[pallet::call_index(0)]
        #[pallet::weight((<T as Config>::WeightInfo::halt(), DispatchClass::Operational))]
        pub fn halt(origin: OriginFor<T>, reason: ReasonOf<T>) -> DispatchResult {
            let guardian = ensure_signed(origin)?;
            ensure!(Self::is_guardian(&guardian), Error::<T>::NotGuardian);
            ensure!(!Self::halted_recently(), Error::<T>::HaltedRecently);
            let block = frame_system::Pallet::<T>::block_number();
            // The block keeps the identity unique even if the count ever saturates.
            let number = LastHalt::<T>::get()
                .map_or(0, |last| last.number)
                .saturating_add(1);
            let signal = GuardianHalt {
                number,
                block: block.unique_saturated_into(),
            };
            sp_io::storage::set(GUARDIAN_HALT_STORAGE_KEY, &signal.encode());
            LastHalt::<T>::put(HaltRecord {
                number,
                guardian: guardian.clone(),
                block,
                reason: reason.clone(),
            });
            Self::deposit_event(Event::BridgeHalted {
                number,
                guardian,
                reason,
            });
            Ok(())
        }

        // Call index 1 held `resume`, which cleared Root's latch. Halts no longer
        // set that latch, so the index stays unused.

        /// Allow `account` to halt the bridges.
        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::add_guardian())]
        pub fn add_guardian(origin: OriginFor<T>, account: T::AccountId) -> DispatchResult {
            T::GuardianOrigin::ensure_origin(origin)?;
            Self::insert_guardian(account)
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
        /// Add `account` to the guardian list, with no origin check.
        ///
        /// [`Pallet::add_guardian`] calls this after checking `GuardianOrigin`. The
        /// runtime calls it to install the first guardians in the upgrade that adds
        /// this pallet. It is not a dispatchable.
        pub fn insert_guardian(account: T::AccountId) -> DispatchResult {
            Guardians::<T>::try_mutate(|guardians| match guardians.try_insert(account.clone()) {
                Ok(true) => Ok(()),
                Ok(false) => Err(Error::<T>::AlreadyGuardian),
                Err(_) => Err(Error::<T>::TooManyGuardians),
            })?;
            Self::deposit_event(Event::GuardianAdded { account });
            Ok(())
        }

        pub fn is_guardian(account: &T::AccountId) -> bool {
            Guardians::<T>::get().contains(account)
        }

        /// The previous halt is less than `HaltCooldown` blocks old.
        pub fn halted_recently() -> bool {
            LastHalt::<T>::get().is_some_and(|last| {
                frame_system::Pallet::<T>::block_number()
                    < last.block.saturating_add(T::HaltCooldown::get())
            })
        }

        /// Transaction admission for a direct, signed `halt`.
        ///
        /// The runtime runs this in pool validation and again before dispatch,
        /// and charges no fee when it passes. Rejecting here, rather than failing
        /// in dispatch, keeps other signers' attempts out of blocks entirely and
        /// limits a guardian to one free halt per `HaltCooldown` blocks.
        pub fn validate_halt(account: &T::AccountId) -> Result<(), TransactionValidityError> {
            if !Self::is_guardian(account) {
                return Err(InvalidTransaction::BadSigner.into());
            }
            if Self::halted_recently() {
                return Err(InvalidTransaction::Stale.into());
            }
            Ok(())
        }
    }
}
