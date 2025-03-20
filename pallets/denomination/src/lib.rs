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

use codec::HasCompact;
use codec::{Decode, Encode};
use codec::{FullCodec, MaxEncodedLen};
use frame_support::log;
use frame_support::{dispatch::DispatchResult, weights::Weight};
use frame_support::{
    CloneNoBound, EqNoBound, PartialEqNoBound, RuntimeDebug, RuntimeDebugNoBound,
    StoragePrefixedMap,
};
use frame_system::pallet_prelude::BlockNumberFor;
use scale_info::TypeInfo;
use sp_core::bounded::BoundedVec;
use sp_runtime::traits::{Convert, ConvertBack, One, Saturating, Zero};
use sp_runtime::Perbill;
use sp_staking::EraIndex;
use sp_std::prelude::*;

pub use pallet::*;

#[derive(Clone, Copy, Default, PartialEq, Eq, Encode, Decode, TypeInfo, MaxEncodedLen, Debug)]
pub enum MigrationStage {
    #[default]
    Initial,
    RemoveAccounts,
    Denomination,
    Complete,
}

/// A pending slash record. The value of the slash has been computed but not applied yet,
/// rather deferred for several eras.
#[derive(Encode, Decode, RuntimeDebug, TypeInfo)]
pub struct UnappliedSlash<AccountId, Balance: HasCompact> {
    /// The stash ID of the offending validator.
    validator: AccountId,
    /// The validator's own slash.
    own: Balance,
    /// All other slashed stakers and amounts.
    others: Vec<(AccountId, Balance)>,
    /// Reporters of the offence; bounty payout recipients.
    reporters: Vec<AccountId>,
    /// The amount of payout.
    payout: Balance,
}

/// A slashing-span record for a particular stash.
#[derive(Encode, Decode, Default, TypeInfo, MaxEncodedLen)]
pub struct SpanRecord<Balance> {
    slashed: Balance,
    paid_out: Balance,
}

/// Just a Balance/BlockNumber tuple to encode when a chunk of funds will be unlocked.
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct UnlockChunk<Balance: HasCompact + MaxEncodedLen> {
    /// Amount of funds to be unlocked.
    #[codec(compact)]
    value: Balance,
    /// Era number at which point it'll be unlocked.
    #[codec(compact)]
    era: EraIndex,
}

/// The ledger of a (bonded) stash.
#[derive(
    PartialEqNoBound,
    EqNoBound,
    CloneNoBound,
    Encode,
    Decode,
    RuntimeDebugNoBound,
    TypeInfo,
    MaxEncodedLen,
)]
#[scale_info(skip_type_params(T))]
pub struct StakingLedger<T: Config> {
    /// The stash account whose balance is actually locked and at stake.
    pub stash: T::AccountId,
    /// The total amount of the stash's balance that we are currently accounting for.
    /// It's just `active` plus all the `unlocking` balances.
    #[codec(compact)]
    pub total: BalanceOf<T>,
    /// The total amount of the stash's balance that will be at stake in any forthcoming
    /// rounds.
    #[codec(compact)]
    pub active: BalanceOf<T>,
    /// Any balance that is becoming free, which may eventually be transferred out of the stash
    /// (assuming it doesn't get slashed first). It is assumed that this will be treated as a first
    /// in, first out queue where the new (higher value) eras get pushed on the back.
    pub unlocking: BoundedVec<UnlockChunk<BalanceOf<T>>, T::MaxUnlockingChunks>,
    /// List of eras for which the stakers behind a validator have claimed rewards. Only updated
    /// for validators.
    pub claimed_rewards: BoundedVec<EraIndex, T::HistoryDepth>,
}

pub trait ShouldRemoveAccount<AccountData> {
    fn should_remove_account(data: &AccountData) -> bool;
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use common::OnDenominate;
    use frame_support::{pallet_prelude::*, traits::Currency};
    use frame_system::pallet_prelude::*;
    use sp_runtime::traits::{ConvertBack, One};

    pub type AssetId = common::AssetId32<common::PredefinedAssetId>;

    pub type BalanceOf<T> = <T as pallet_balances::Config>::Balance;

    #[pallet::config]
    pub trait Config:
        common::Config
        + pallet_balances::Config
        + frame_system::Config
        + pallet_offences::Config
        + pallet_democracy::Config
        + pallet_elections_phragmen::Config
        + pallet_identity::Config
        + pallet_staking::Config
        + pallet_preimage::Config
        + tokens::Config
    {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        #[pallet::constant]
        type RemoveReadPerBlock: Get<u64>;

        type ShouldRemoveAccount: ShouldRemoveAccount<
            frame_system::AccountInfo<Self::Index, Self::AccountData>,
        >;

        type OffencesConverter: ConvertBack<
            Self::IdentificationTuple,
            (
                Self::AccountId,
                pallet_staking::Exposure<Self::AccountId, BalanceOf<Self>>,
            ),
        >;

        type DemocracyConvertBalance: Convert<
            BalanceOf<Self>,
            <<Self as pallet_democracy::Config>::Currency as Currency<Self::AccountId>>::Balance,
        >;

        type StakingConvertBalance: Convert<BalanceOf<Self>, pallet_staking::BalanceOf<Self>>;

        type TokensConvertBalance: Convert<BalanceOf<Self>, <Self as tokens::Config>::Balance>;

        type ElectionsPhragmenConvertBalance: Convert<
            BalanceOf<Self>,
            <<Self as pallet_elections_phragmen::Config>::Currency as Currency<Self::AccountId>>::Balance,
        >;

        type AssetIdConvert: Convert<AssetId, <Self as tokens::Config>::CurrencyId>;

        type OnDenominate: OnDenominate<BalanceOf<Self>>;

        type CallOrigin: EnsureOrigin<Self::RuntimeOrigin>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        RemovedAccounts { keep: u64, removed: u64 },
        Denominated { denominator: BalanceOf<T> },
        MigrationStageUpdated { stage: MigrationStage },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// This action is not allowed at current stage
        WrongMigrationStage,
    }

    #[pallet::storage]
    #[pallet::getter(fn current_migration_stage)]
    pub(super) type CurrentMigrationStage<T: Config> = StorageValue<_, MigrationStage, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn remove_accounts_cursor)]
    pub(super) type RemoveAccountsCursor<T: Config> = StorageValue<_, Vec<u8>>;

    #[pallet::storage]
    #[pallet::getter(fn denominator)]
    pub(super) type Denominator<T: Config> =
        StorageValue<_, BalanceOf<T>, ValueQuery, DefaultDenominator<T>>;

    #[pallet::type_value]
    pub fn DefaultDenominator<T: Config>() -> BalanceOf<T> {
        One::one()
    }

    #[frame_support::storage_alias]
    pub type IdentityOf<T: Config> = StorageMap<
        pallet_identity::Pallet<T>,
        Twox64Concat,
        <T as frame_system::Config>::AccountId,
        pallet_identity::Registration<
            BalanceOf<T>,
            <T as pallet_identity::Config>::MaxRegistrars,
            <T as pallet_identity::Config>::MaxAdditionalFields,
        >,
        OptionQuery,
    >;

    #[frame_support::storage_alias]
    pub type Registrars<T: Config> = StorageValue<
        pallet_identity::Pallet<T>,
        BoundedVec<
            Option<
                pallet_identity::RegistrarInfo<
                    BalanceOf<T>,
                    <T as frame_system::Config>::AccountId,
                >,
            >,
            <T as pallet_identity::Config>::MaxRegistrars,
        >,
        ValueQuery,
    >;

    #[frame_support::storage_alias]
    pub type Ledger<T: Config> = StorageMap<
        pallet_staking::Pallet<T>,
        Blake2_128Concat,
        <T as frame_system::Config>::AccountId,
        StakingLedger<T>,
    >;

    #[frame_support::storage_alias]
    pub type ValidatorSlashInEra<T: Config> = StorageDoubleMap<
        pallet_staking::Pallet<T>,
        Twox64Concat,
        EraIndex,
        Twox64Concat,
        <T as frame_system::Config>::AccountId,
        (Perbill, BalanceOf<T>),
    >;

    #[frame_support::storage_alias]
    pub type NominatorSlashInEra<T: Config> = StorageDoubleMap<
        pallet_staking::Pallet<T>,
        Twox64Concat,
        EraIndex,
        Twox64Concat,
        <T as frame_system::Config>::AccountId,
        BalanceOf<T>,
    >;

    #[frame_support::storage_alias]
    pub type SpanSlash<T: Config> = StorageMap<
        pallet_staking::Pallet<T>,
        Twox64Concat,
        (
            <T as frame_system::Config>::AccountId,
            pallet_staking::slashing::SpanIndex,
        ),
        SpanRecord<BalanceOf<T>>,
        ValueQuery,
    >;

    #[frame_support::storage_alias]
    pub type UnappliedSlashes<T: Config> = StorageMap<
        pallet_staking::Pallet<T>,
        Twox64Concat,
        EraIndex,
        Vec<UnappliedSlash<<T as frame_system::Config>::AccountId, BalanceOf<T>>>,
        ValueQuery,
    >;

    #[frame_support::storage_alias]
    pub type StatusFor<T: Config> = StorageMap<
        pallet_preimage::Pallet<T>,
        Identity,
        <T as frame_system::Config>::Hash,
        pallet_preimage::RequestStatus<<T as frame_system::Config>::AccountId, BalanceOf<T>>,
    >;

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(_n: T::BlockNumber) -> Weight {
            if Self::current_migration_stage() != MigrationStage::RemoveAccounts {
                return T::DbWeight::get().reads(1);
            }
            let mut last_key = None;
            let max_read = T::RemoveReadPerBlock::get();
            let mut read = 0;
            let mut removed = 0;
            for (account, data) in frame_system::Account::<T>::iter_from(
                Self::remove_accounts_cursor().unwrap_or_default(),
            ) {
                if read >= max_read {
                    break;
                }
                if T::ShouldRemoveAccount::should_remove_account(&data) {
                    frame_system::Account::<T>::remove(&account);
                    removed += 1;
                }
                read += 1;
                last_key = Some(account);
            }
            if let Some(last_key) = last_key {
                RemoveAccountsCursor::<T>::set(Some(frame_system::Account::<T>::hashed_key_for(
                    last_key,
                )));
            } else {
                RemoveAccountsCursor::<T>::set(None);
                CurrentMigrationStage::<T>::set(MigrationStage::Denomination);
                Self::deposit_event(Event::<T>::MigrationStageUpdated {
                    stage: MigrationStage::Denomination,
                });
            }

            Self::deposit_event(Event::<T>::RemovedAccounts {
                keep: read.saturating_sub(removed),
                removed,
            });
            T::DbWeight::get().reads_writes(read, removed)
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(common::weights::constants::EXTRINSIC_FIXED_WEIGHT)]
        pub fn init(origin: OriginFor<T>) -> DispatchResult {
            T::CallOrigin::ensure_origin(origin)?;
            ensure!(
                CurrentMigrationStage::<T>::get() == MigrationStage::Initial,
                Error::<T>::WrongMigrationStage
            );
            CurrentMigrationStage::<T>::set(MigrationStage::RemoveAccounts);
            Self::deposit_event(Event::<T>::MigrationStageUpdated {
                stage: MigrationStage::RemoveAccounts,
            });
            Ok(())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(common::weights::constants::EXTRINSIC_FIXED_WEIGHT)]
        pub fn start_denomination(
            origin: OriginFor<T>,
            denominator: BalanceOf<T>,
        ) -> DispatchResult {
            T::CallOrigin::ensure_origin(origin)?;
            ensure!(
                CurrentMigrationStage::<T>::get() == MigrationStage::Denomination,
                Error::<T>::WrongMigrationStage
            );
            Self::denominate(denominator)?;
            T::OnDenominate::on_denominate(&denominator)?;
            CurrentMigrationStage::<T>::set(MigrationStage::Complete);
            Denominator::<T>::set(denominator);
            Self::deposit_event(Event::<T>::Denominated { denominator });
            Self::deposit_event(Event::<T>::MigrationStageUpdated {
                stage: MigrationStage::Complete,
            });
            Ok(())
        }
    }
}

impl<T: Config> common::Denominator<AssetId, BalanceOf<T>> for Pallet<T> {
    fn current_factor(asset_id: &AssetId) -> BalanceOf<T> {
        if *asset_id == common::XOR || *asset_id == common::TBCD {
            Denominator::<T>::get()
        } else {
            One::one()
        }
    }
}

trait StorageMapExt<Value> {
    fn modify_values<F: FnMut(&mut Value)>(f: F);
}

impl<Value: FullCodec, Map: StoragePrefixedMap<Value>> StorageMapExt<Value> for Map {
    fn modify_values<F: FnMut(&mut Value)>(mut f: F) {
        let mut count = 0usize;
        <Self as StoragePrefixedMap<Value>>::translate_values(|mut v| {
            f(&mut v);
            count += 1;
            Some(v)
        });
        let module = core::str::from_utf8(Self::module_prefix()).unwrap_or_default();
        let storage = core::str::from_utf8(Self::storage_prefix()).unwrap_or_default();
        log::info!(
            "Denominated {} values in storage {}::{}",
            count,
            module,
            storage
        );
    }
}

impl<T: Config> Pallet<T> {
    fn denominate(denominator: BalanceOf<T>) -> DispatchResult {
        Self::denominate_balances(denominator)?;
        Self::denominate_democracy(denominator)?;
        Self::denominate_elections_phragmen(denominator)?;
        Self::denominate_identity(denominator)?;
        Self::denominate_offences(denominator)?;
        Self::denominate_staking(denominator)?;
        Self::denominate_tokens(denominator)?;
        Self::denominate_preimage(denominator)?;
        Ok(())
    }

    fn denominate_balances(denominator: BalanceOf<T>) -> DispatchResult {
        let mut total_issuance = BalanceOf::<T>::zero();
        for account in frame_system::Account::<T>::iter_keys() {
            pallet_balances::Pallet::<T>::mutate_account(&account, |account| {
                account.free /= denominator;
                account.reserved /= denominator;
                account.misc_frozen /= denominator;
                account.fee_frozen /= denominator;
                total_issuance = total_issuance
                    .saturating_add(account.free)
                    .saturating_add(account.reserved);
            })?;
        }
        pallet_balances::Locks::<T>::modify_values(|locks| {
            *locks = locks
                .clone()
                .try_mutate(|locks| {
                    for lock in locks.iter_mut() {
                        lock.amount /= denominator;
                    }
                })
                .unwrap_or_default();
        });
        pallet_balances::TotalIssuance::<T>::set(total_issuance);
        pallet_balances::InactiveIssuance::<T>::mutate(|value| {
            *value /= denominator;
        });
        Ok(())
    }

    fn denominate_offences(denominator: BalanceOf<T>) -> DispatchResult {
        pallet_offences::Reports::<T>::modify_values(|v| {
            let (offender, mut exposure) = T::OffencesConverter::convert(v.offender.clone());
            let mut total = Zero::zero();
            exposure.own /= denominator;
            total += exposure.own;
            for exp in exposure.others.iter_mut() {
                exp.value /= denominator;
                total += exp.value;
            }
            exposure.total = total;
            v.offender = T::OffencesConverter::convert_back((offender, exposure));
        });
        Ok(())
    }

    fn denominate_tokens(denominator: BalanceOf<T>) -> DispatchResult {
        let denominator = T::TokensConvertBalance::convert(denominator);
        let mut new_issuance = Zero::zero();
        let tbcd = T::AssetIdConvert::convert(common::TBCD);
        tokens::Accounts::<T>::translate::<tokens::AccountData<<T as tokens::Config>::Balance>, _>(
            |_, asset_id, mut data| {
                if asset_id == tbcd {
                    data.free /= denominator;
                    data.reserved /= denominator;
                    data.frozen /= denominator;
                    new_issuance += data.free;
                }
                Some(data)
            },
        );
        tokens::Locks::<T>::translate::<
            BoundedVec<
                tokens::BalanceLock<<T as tokens::Config>::Balance>,
                <T as tokens::Config>::MaxLocks,
            >,
            _,
        >(|_, asset_id, mut locks| {
            if asset_id == tbcd {
                for lock in locks.iter_mut() {
                    lock.amount /= denominator;
                }
            }
            Some(locks)
        });
        tokens::TotalIssuance::<T>::mutate(tbcd, |issuance| {
            *issuance = new_issuance;
        });
        Ok(())
    }

    fn denominate_democracy(denominator: BalanceOf<T>) -> DispatchResult {
        let denominator = T::DemocracyConvertBalance::convert(denominator);
        pallet_democracy::VotingOf::<T>::modify_values(|v| match v {
            pallet_democracy::Voting::Delegating {
                balance,
                delegations,
                ..
            } => {
                *balance /= denominator;
                delegations.votes /= denominator;
                delegations.capital /= denominator;
            }
            pallet_democracy::Voting::Direct {
                votes, delegations, ..
            } => {
                delegations.votes /= denominator;
                delegations.capital /= denominator;
                for (_, vote) in votes.iter_mut() {
                    match vote {
                        pallet_democracy::AccountVote::Split { aye, nay } => {
                            *aye /= denominator;
                            *nay /= denominator;
                        }
                        pallet_democracy::AccountVote::Standard { balance, .. } => {
                            *balance /= denominator;
                        }
                    }
                }
            }
        });
        Ok(())
    }

    fn denominate_elections_phragmen(denominator: BalanceOf<T>) -> DispatchResult {
        let denominator = T::ElectionsPhragmenConvertBalance::convert(denominator);
        pallet_elections_phragmen::Candidates::<T>::mutate(|v| {
            for (_, deposit) in v.iter_mut() {
                *deposit /= denominator;
            }
        });
        pallet_elections_phragmen::RunnersUp::<T>::mutate(|v| {
            for holder in v.iter_mut() {
                holder.stake /= denominator;
                holder.deposit /= denominator;
            }
        });
        pallet_elections_phragmen::Members::<T>::mutate(|v| {
            for holder in v.iter_mut() {
                holder.stake /= denominator;
                holder.deposit /= denominator;
            }
        });
        pallet_elections_phragmen::Voting::<T>::modify_values(|v| {
            v.stake /= denominator;
            v.deposit /= denominator;
        });
        Ok(())
    }

    fn denominate_identity(denominator: BalanceOf<T>) -> DispatchResult {
        IdentityOf::<T>::modify_values(|v| {
            v.deposit /= denominator;
            for (_, judgement) in v.judgements.iter_mut() {
                match judgement {
                    pallet_identity::Judgement::FeePaid(fee) => {
                        *fee /= denominator;
                    }
                    _ => {}
                }
            }
        });
        Registrars::<T>::mutate(|v| {
            for registrar in v.iter_mut() {
                if let Some(registrar) = registrar {
                    registrar.fee /= denominator;
                }
            }
        });
        Ok(())
    }

    fn denominate_staking(denominator: BalanceOf<T>) -> DispatchResult {
        Ledger::<T>::modify_values(|v| {
            let mut total = Zero::zero();
            v.active /= denominator;
            total += v.active;
            for chunk in v.unlocking.iter_mut() {
                chunk.value /= denominator;
                total += chunk.value;
            }
            v.total = total;
        });
        pallet_staking::MinimumActiveStake::<T>::mutate(|v| {
            *v /= T::StakingConvertBalance::convert(denominator);
        });
        pallet_staking::ErasStakers::<T>::modify_values(|v| {
            let mut total = Zero::zero();
            v.own /= T::StakingConvertBalance::convert(denominator);
            total += v.own;
            for exp in v.others.iter_mut() {
                exp.value /= T::StakingConvertBalance::convert(denominator);
                total += exp.value;
            }
            v.total = total;
        });
        pallet_staking::ErasStakersClipped::<T>::modify_values(|v| {
            let mut total = Zero::zero();
            v.own /= T::StakingConvertBalance::convert(denominator);
            total += v.own;
            for exp in v.others.iter_mut() {
                exp.value /= T::StakingConvertBalance::convert(denominator);
                total += exp.value;
            }
            v.total = total;
        });
        pallet_staking::CanceledSlashPayout::<T>::mutate(|v| {
            *v /= T::StakingConvertBalance::convert(denominator);
        });
        UnappliedSlashes::<T>::modify_values(|v| {
            for slash in v.iter_mut() {
                slash.own /= denominator;
                slash.payout /= denominator;
                for (_, amount) in slash.others.iter_mut() {
                    *amount /= denominator;
                }
            }
        });
        ValidatorSlashInEra::<T>::modify_values(|(_, amount)| {
            *amount /= denominator;
        });
        NominatorSlashInEra::<T>::modify_values(|v| {
            *v /= denominator;
        });
        SpanSlash::<T>::modify_values(|v| {
            v.slashed /= denominator;
            v.paid_out /= denominator;
        });

        Ok(())
    }

    fn denominate_preimage(denominator: BalanceOf<T>) -> DispatchResult {
        StatusFor::<T>::modify_values(|v| match v {
            pallet_preimage::RequestStatus::Unrequested {
                deposit: (_, balance),
                ..
            }
            | pallet_preimage::RequestStatus::Requested {
                deposit: Some((_, balance)),
                ..
            } => {
                *balance /= denominator;
            }
            _ => {}
        });
        Ok(())
    }
}
