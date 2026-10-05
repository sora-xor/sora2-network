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

//! This pallet provides means of migration for Iroha users.
//! It relies on some configuration provided by the genesis block:
//! * Iroha accounts
//! * Account (an account that have permissions to mint VAL, balances are migrated by minting VAL with this account)
//!
//! All migrated accounts are stored to use when their referrals migrate or when a user attempts to migrate again

#![cfg_attr(not(feature = "std"), no_std)]
// TODO #167: fix clippy warnings
#![allow(clippy::all)]

#[macro_use]
extern crate alloc;
use alloc::string::String;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub mod weights;

use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use common::prelude::Balance;
use common::{FromGenericPair, VAL};
use ed25519_dalek_iroha::{Digest, PublicKey, Signature, SIGNATURE_LENGTH};
use frame_support::dispatch::Pays;
use frame_support::ensure;
use frame_support::sp_runtime::traits::{Hash, Zero};
use frame_support::sp_runtime::DispatchError;
use frame_support::traits::{Currency, Get, WithdrawReasons};
use frame_support::weights::Weight;
use frame_system::ensure_signed;
use frame_system::pallet_prelude::BlockNumberFor;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sha3::Sha3_256;
use sp_std::convert::TryInto;
use sp_std::prelude::*;

type WeightInfoOf<T> = <T as Config>::WeightInfo;
pub use weights::WeightInfo;

pub const TECH_ACCOUNT_PREFIX: &[u8] = b"iroha-migration";
pub const TECH_ACCOUNT_MAIN: &[u8] = b"main";
const MIGRATION_SIGNING_PAYLOAD_PREFIX: &str = "SORA2-IROHA-MIGRATION-V2";

/// A voluntary authorization, never a claim on another account's funds.
/// The payment extension withdraws XOR before consuming an attempt, outside
/// the migration call's rollback layer. Quoted fees bound total exposure.
#[derive(
    Clone, Debug, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, scale_info::TypeInfo,
)]
pub struct FeeSponsorship<AccountId, BlockNumber> {
    pub sponsor: AccountId,
    pub beneficiary: AccountId,
    pub max_fee: Balance,
    pub remaining_budget: Balance,
    pub remaining_attempts: u8,
    pub valid_until: BlockNumber,
}

fn blocks_till_migration<T>() -> BlockNumberFor<T>
where
    T: frame_system::Config,
{
    // 1 month
    446400u32.into()
}

#[derive(PartialEq, Eq, Clone, Debug, Encode, Decode, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[scale_info(skip_type_params(T))]
struct PendingMultisigAccount<T>
where
    T: frame_system::Config,
{
    approving_accounts: Vec<T::AccountId>,
    migrate_at: Option<BlockNumberFor<T>>,
}

impl<T> Default for PendingMultisigAccount<T>
where
    T: frame_system::Config,
{
    fn default() -> Self {
        Self {
            approving_accounts: Default::default(),
            migrate_at: Default::default(),
        }
    }
}

impl<T: Config> Pallet<T> {
    fn can_fund_sponsorship(sponsor: &T::AccountId, budget: Balance) -> bool {
        let Some(remaining) = T::FeeCurrency::free_balance(sponsor).checked_sub(budget) else {
            return false;
        };
        remaining > 0
            && remaining >= T::FeeCurrency::minimum_balance()
            && T::FeeCurrency::ensure_can_withdraw(
                sponsor,
                budget,
                WithdrawReasons::TRANSACTION_PAYMENT,
                remaining,
            )
            .is_ok()
    }

    pub fn needs_migration(iroha_address: &String) -> bool {
        Balances::<T>::contains_key(iroha_address)
            && !MigratedAccounts::<T>::contains_key(iroha_address)
    }

    pub(crate) fn migration_signing_message(
        iroha_address: &str,
        iroha_public_key: &str,
        account: &T::AccountId,
    ) -> String {
        let genesis_hash = T::MigrationGenesisHash::get();
        format!(
            "{}\ngenesis_hash=0x{}\niroha_address={}\niroha_public_key={}\nsubstrate_account=0x{}",
            MIGRATION_SIGNING_PAYLOAD_PREFIX,
            hex::encode(genesis_hash.encode()),
            iroha_address,
            iroha_public_key,
            hex::encode(account.encode())
        )
    }

    /// Checks if migration would succeed if the parameters were passed to migrate extrinsic.
    fn check_migrate(
        iroha_address: &String,
        iroha_public_key: &String,
        iroha_signature: &String,
        account: &T::AccountId,
    ) -> Result<(), DispatchError> {
        let iroha_public_key = iroha_public_key.to_lowercase();
        let iroha_signature = iroha_signature.to_lowercase();
        ensure!(
            !MigratedAccounts::<T>::contains_key(&iroha_address),
            Error::<T>::AccountAlreadyMigrated
        );
        let public_keys =
            PublicKeys::<T>::try_get(&iroha_address).map_err(|_| Error::<T>::AccountNotFound)?;
        let already_migrated = public_keys
            .iter()
            .find_map(|(already_migrated, key)| {
                if key == &iroha_public_key {
                    Some(already_migrated)
                } else {
                    None
                }
            })
            .ok_or(Error::<T>::AccountNotFound)?;
        ensure!(!already_migrated, Error::<T>::PublicKeyAlreadyUsed);
        Self::verify_signature(&iroha_address, &iroha_public_key, &iroha_signature, account)?;
        Ok(())
    }

    pub fn sponsorship_id(
        account: &T::AccountId,
        iroha_address: &String,
        iroha_public_key: &String,
    ) -> T::Hash {
        T::Hashing::hash_of(&(account, iroha_address, iroha_public_key.to_lowercase()))
    }

    pub fn fee_sponsor(
        account: &T::AccountId,
        iroha_address: &String,
        iroha_public_key: &String,
        iroha_signature: &String,
        fee: Balance,
        tip: Balance,
    ) -> Option<T::AccountId> {
        if !tip.is_zero()
            || fee.is_zero()
            || iroha_address.len() > 128
            || iroha_public_key.len() != 64
            || iroha_signature.len() != 128
        {
            return None;
        }
        Self::check_migrate(iroha_address, iroha_public_key, iroha_signature, account).ok()?;
        let id = Self::sponsorship_id(account, iroha_address, iroha_public_key);
        let grant = FeeSponsorships::<T>::get(id)?;
        (grant.remaining_attempts > 0
            && fee <= grant.max_fee
            && fee <= grant.remaining_budget
            && frame_system::Pallet::<T>::block_number() <= grant.valid_until)
            .then_some(grant.sponsor)
    }

    /// Called only after a matching sponsored fee has been withdrawn atomically.
    pub fn consume_fee_sponsorship(
        account: &T::AccountId,
        iroha_address: &String,
        iroha_public_key: &String,
        sponsor: &T::AccountId,
        fee: Balance,
    ) -> Result<(), DispatchError> {
        let id = Self::sponsorship_id(account, iroha_address, iroha_public_key);
        FeeSponsorships::<T>::try_mutate_exists(id, |stored| {
            let grant = stored.as_mut().ok_or(Error::<T>::InvalidFeeSponsorship)?;
            ensure!(
                &grant.sponsor == sponsor
                    && grant.remaining_attempts > 0
                    && fee <= grant.max_fee
                    && fee <= grant.remaining_budget
                    && frame_system::Pallet::<T>::block_number() <= grant.valid_until,
                Error::<T>::InvalidFeeSponsorship
            );
            grant.remaining_attempts -= 1;
            grant.remaining_budget -= fee;
            // Keep the bounded exhausted grant until explicitly cleaned up. Its
            // sufficient reference preserves the claimant's nonce through a
            // failed final attempt; revocation releases that reference.
            Self::deposit_event(Event::FeeSponsorshipUsed {
                claim: id,
                sponsor: sponsor.clone(),
                maximum_fee: fee,
            });
            Ok(())
        })
    }

    fn parse_public_key(iroha_public_key: &str) -> Result<PublicKey, DispatchError> {
        let iroha_public_key =
            hex::decode(&iroha_public_key).map_err(|_| Error::<T>::PublicKeyParsingFailed)?;
        let public_key = PublicKey::from_bytes(iroha_public_key.as_slice())
            .map_err(|_| Error::<T>::PublicKeyParsingFailed)?;
        Ok(public_key)
    }

    fn parse_signature(iroha_signature: &str) -> Result<Signature, DispatchError> {
        let iroha_signature =
            hex::decode(iroha_signature).map_err(|_| Error::<T>::SignatureParsingFailed)?;
        let signature_bytes: [u8; SIGNATURE_LENGTH] = iroha_signature
            .as_slice()
            .try_into()
            .map_err(|_| Error::<T>::SignatureParsingFailed)?;
        Ok(Signature::from(signature_bytes))
    }

    fn verify_signature(
        iroha_address: &str,
        iroha_public_key: &str,
        iroha_signature: &str,
        account: &T::AccountId,
    ) -> Result<(), DispatchError> {
        let public_key = Self::parse_public_key(iroha_public_key)?;
        let signature = Self::parse_signature(iroha_signature)?;
        let message = Self::migration_signing_message(iroha_address, iroha_public_key, account);
        let mut prehashed_message = Sha3_256::default();
        prehashed_message.update(&message[..]);
        public_key
            .verify_prehashed(prehashed_message, None, &signature)
            .map_err(|_| Error::<T>::SignatureVerificationFailed)?;
        Ok(())
    }

    fn approve_with_public_key(
        iroha_address: &String,
        iroha_public_key: &str,
    ) -> Result<(usize, usize), DispatchError> {
        PublicKeys::<T>::mutate(iroha_address, |keys| {
            {
                let already_approved = keys
                    .iter_mut()
                    .find(|(_, key)| key == iroha_public_key)
                    .map(|(already_approved, _)| already_approved)
                    .ok_or(Error::<T>::PublicKeyNotFound)?;
                ensure!(!*already_approved, Error::<T>::PublicKeyAlreadyUsed);
                *already_approved = true;
            }
            let approved_count = keys
                .iter()
                .filter(|(already_approved, _)| *already_approved)
                .count();
            Ok((approved_count, keys.len()))
        })
    }

    fn on_multisig_account_approved(
        iroha_address: String,
        account: T::AccountId,
        approval_count: usize,
        public_key_count: usize,
    ) -> Result<(), DispatchError> {
        if approval_count == public_key_count {
            let quorum = Quorums::<T>::take(&iroha_address);
            let signatories = {
                let mut pending_account = PendingMultiSigAccounts::<T>::take(&iroha_address);
                pending_account.approving_accounts.push(account);
                pending_account.approving_accounts.sort();
                pending_account.approving_accounts
            };
            let multi_account =
                pallet_multisig::Pallet::<T>::multi_account_id(&signatories, quorum as u16);
            Self::migrate_account(iroha_address, multi_account)?;
        } else {
            let quorum = Quorums::<T>::get(&iroha_address) as usize;
            if approval_count == quorum {
                PendingMultiSigAccounts::<T>::mutate(&iroha_address, |a| {
                    a.approving_accounts.push(account);
                    let migrate_at =
                        frame_system::Pallet::<T>::block_number() + blocks_till_migration::<T>();
                    a.migrate_at = Some(migrate_at);
                });
            } else if approval_count < quorum {
                PendingMultiSigAccounts::<T>::mutate(&iroha_address, |a| {
                    a.approving_accounts.push(account);
                });
            }
        }
        Ok(())
    }

    fn migrate_account(iroha_address: String, account: T::AccountId) -> Result<(), DispatchError> {
        Self::migrate_balance(&iroha_address, &account)?;
        Self::migrate_referrals(&iroha_address, &account)?;
        PublicKeys::<T>::remove(&iroha_address);
        MigratedAccounts::<T>::insert(&iroha_address, &account);
        Self::deposit_event(Event::Migrated(iroha_address, account));
        Ok(())
    }

    fn migrate_balance(
        iroha_address: &String,
        account: &T::AccountId,
    ) -> Result<(), DispatchError> {
        if let Some(balance) = Balances::<T>::take(iroha_address) {
            if !balance.is_zero() {
                let eth_bridge_tech_account_id = <T>::TechAccountId::from_generic_pair(
                    eth_bridge::TECH_ACCOUNT_PREFIX.to_vec(),
                    eth_bridge::TECH_ACCOUNT_MAIN.to_vec(),
                );

                technical::Pallet::<T>::transfer_out(
                    &VAL.into(),
                    &eth_bridge_tech_account_id,
                    account,
                    balance,
                )?;
            }
        }
        Ok(())
    }

    fn migrate_referrals(
        iroha_address: &String,
        account: &T::AccountId,
    ) -> Result<(), DispatchError> {
        // Migrate a referral to their referrer
        if let Some(referrer) = Referrers::<T>::get(iroha_address) {
            // Free up memory
            Referrers::<T>::remove(iroha_address);
            if let Some(referrer) = MigratedAccounts::<T>::get(&referrer) {
                referrals::Pallet::<T>::set_referrer_to(&account, referrer)
                    .map_err(|_| Error::<T>::ReferralMigrationFailed)?;
            } else {
                PendingReferrals::<T>::mutate(&referrer, |referrals| {
                    referrals.push(account.clone());
                });
            }
        }
        // Migrate pending referrals to their referrer
        let referrals = PendingReferrals::<T>::take(iroha_address);
        for referral in &referrals {
            referrals::Pallet::<T>::set_referrer_to(referral, account.clone())
                .map_err(|_| Error::<T>::ReferralMigrationFailed)?;
        }
        Ok(())
    }
}
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use common::AccountIdOf;
    use frame_support::dispatch::PostDispatchInfo;
    use frame_support::pallet_prelude::*;
    use frame_support::traits::StorageVersion;
    use frame_system::pallet_prelude::*;

    #[pallet::config]
    pub trait Config:
        frame_system::Config + pallet_multisig::Config + referrals::Config + technical::Config
    {
        #[allow(deprecated)]
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type MigrationGenesisHash: Get<Self::Hash>;
        type FeeCurrency: Currency<Self::AccountId, Balance = Balance>;
        type WeightInfo: WeightInfo;
    }

    /// The current storage version.
    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::storage]
    pub type FeeSponsorships<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::Hash,
        FeeSponsorship<T::AccountId, BlockNumberFor<T>>,
        OptionQuery,
    >;

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(block_number: BlockNumberFor<T>) -> Weight {
            // Migrate accounts whose quorum has been reached and enough time has passed since then
            PendingMultiSigAccounts::<T>::translate(|key, mut value: PendingMultisigAccount<T>| {
                if let Some(migrate_at) = value.migrate_at {
                    if block_number > migrate_at {
                        value.approving_accounts.sort();
                        let quorum = Quorums::<T>::take(&key);
                        let multi_account = pallet_multisig::Pallet::<T>::multi_account_id(
                            &value.approving_accounts,
                            quorum as u16,
                        );
                        let _ = Self::migrate_account(key, multi_account);
                        None
                    } else {
                        Some(value)
                    }
                } else {
                    Some(value)
                }
            });
            WeightInfoOf::<T>::on_initialize()
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        // Cover ownership checks in payment admission as well as dispatch.
        #[pallet::weight((WeightInfoOf::<T>::migrate().saturating_mul(3), Pays::Yes))]
        pub fn migrate(
            origin: OriginFor<T>,
            iroha_address: String,
            iroha_public_key: String,
            iroha_signature: String,
        ) -> DispatchResultWithPostInfo {
            common::with_transaction(|| {
                let who = ensure_signed(origin)?;
                let iroha_public_key = iroha_public_key.to_lowercase();
                let iroha_signature = iroha_signature.to_lowercase();
                Self::check_migrate(&iroha_address, &iroha_public_key, &iroha_signature, &who)?;
                let (approval_count, key_count) =
                    Self::approve_with_public_key(&iroha_address, &iroha_public_key)?;
                if key_count == 1 {
                    Self::migrate_account(iroha_address, who)?;
                } else {
                    Self::on_multisig_account_approved(
                        iroha_address,
                        who,
                        approval_count,
                        key_count,
                    )?;
                }
                // The user doesn't have to pay fees if the migration is succeeded
                Ok(PostDispatchInfo {
                    actual_weight: None,
                    pays_fee: Pays::No,
                })
            })
        }

        /// Authorize up to three fee-funded migration attempts for one claimant.
        /// The sponsor's available XOR is checked and withdrawn at execution;
        /// an unfunded authorization never admits an unpaid migration.
        #[pallet::call_index(1)]
        #[pallet::weight(WeightInfoOf::<T>::migrate())]
        pub fn sponsor_migration(
            origin: OriginFor<T>,
            beneficiary: T::AccountId,
            iroha_address: String,
            iroha_public_key: String,
            max_fee: Balance,
            attempts: u8,
            valid_until: BlockNumberFor<T>,
        ) -> DispatchResult {
            let sponsor = ensure_signed(origin)?;
            ensure!(
                iroha_address.len() <= 128 && iroha_public_key.len() == 64,
                Error::<T>::InvalidMigrationInput
            );
            ensure!(
                attempts > 0 && attempts <= 3 && max_fee > 0,
                Error::<T>::InvalidFeeSponsorship
            );
            ensure!(
                valid_until > frame_system::Pallet::<T>::block_number(),
                Error::<T>::InvalidFeeSponsorship
            );
            let key = iroha_public_key.to_lowercase();
            ensure!(
                PublicKeys::<T>::get(&iroha_address)
                    .iter()
                    .any(|(used, public)| !used && public == &key),
                Error::<T>::AccountNotFound
            );
            ensure!(
                !MigratedAccounts::<T>::contains_key(&iroha_address),
                Error::<T>::AccountAlreadyMigrated
            );
            let claim = Self::sponsorship_id(&beneficiary, &iroha_address, &key);
            let remaining_budget = max_fee
                .checked_mul(attempts as Balance)
                .ok_or(Error::<T>::InvalidFeeSponsorship)?;
            ensure!(
                Self::can_fund_sponsorship(&sponsor, remaining_budget),
                Error::<T>::InvalidFeeSponsorship
            );
            // A funded live grant cannot be displaced by a stranger. If its
            // sponsor withdraws its funding, another volunteer may replace the
            // unusable promise with an authorization backed by its own funds.
            if let Some(existing) = FeeSponsorships::<T>::get(claim) {
                ensure!(
                    existing.sponsor == sponsor
                        || existing.remaining_attempts == 0
                        || existing.valid_until < frame_system::Pallet::<T>::block_number()
                        || !Self::can_fund_sponsorship(
                            &existing.sponsor,
                            existing.remaining_budget
                        ),
                    Error::<T>::NotFeeSponsor
                );
            } else {
                frame_system::Pallet::<T>::inc_sufficients(&beneficiary);
            }
            FeeSponsorships::<T>::insert(
                claim,
                FeeSponsorship {
                    sponsor: sponsor.clone(),
                    beneficiary: beneficiary.clone(),
                    max_fee,
                    remaining_budget,
                    remaining_attempts: attempts,
                    valid_until,
                },
            );
            Self::deposit_event(Event::FeeSponsorshipGranted {
                claim,
                sponsor,
                beneficiary,
            });
            Ok(())
        }

        /// Revoke a grant, or remove an expired grant without spending its funds.
        #[pallet::call_index(2)]
        #[pallet::weight(WeightInfoOf::<T>::migrate())]
        pub fn revoke_sponsorship(origin: OriginFor<T>, claim: T::Hash) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let grant =
                FeeSponsorships::<T>::get(claim).ok_or(Error::<T>::InvalidFeeSponsorship)?;
            ensure!(
                grant.sponsor == who
                    || grant.beneficiary == who
                    || grant.valid_until < frame_system::Pallet::<T>::block_number(),
                Error::<T>::NotFeeSponsor
            );
            FeeSponsorships::<T>::remove(claim);
            frame_system::Pallet::<T>::dec_sufficients(&grant.beneficiary);
            Self::deposit_event(Event::FeeSponsorshipRevoked { claim });
            Ok(())
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Migrated. [source, target]
        Migrated(String, AccountIdOf<T>),
        FeeSponsorshipGranted {
            claim: T::Hash,
            sponsor: T::AccountId,
            beneficiary: T::AccountId,
        },
        FeeSponsorshipUsed {
            claim: T::Hash,
            sponsor: T::AccountId,
            maximum_fee: Balance,
        },
        FeeSponsorshipRevoked {
            claim: T::Hash,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Failed to parse public key
        PublicKeyParsingFailed,
        /// Failed to parse signature
        SignatureParsingFailed,
        /// Failed to verify signature
        SignatureVerificationFailed,
        /// Iroha account is not found
        AccountNotFound,
        /// Public key is not found
        PublicKeyNotFound,
        /// Public key is already used
        PublicKeyAlreadyUsed,
        /// Iroha account is already migrated
        AccountAlreadyMigrated,
        /// Referral migration failed
        ReferralMigrationFailed,
        /// Milti-signature account creation failed
        MultiSigCreationFailed,
        /// Signatory addition to multi-signature account failed
        SignatoryAdditionFailed,
        InvalidMigrationInput,
        InvalidFeeSponsorship,
        NotFeeSponsor,
    }

    #[pallet::storage]
    pub(super) type Balances<T: Config> = StorageMap<_, Blake2_128Concat, String, Balance>;

    #[pallet::storage]
    pub(super) type Referrers<T: Config> = StorageMap<_, Blake2_128Concat, String, String>;

    #[pallet::storage]
    pub(super) type PublicKeys<T: Config> =
        StorageMap<_, Blake2_128Concat, String, Vec<(bool, String)>, ValueQuery>;

    #[pallet::storage]
    pub(super) type Quorums<T: Config> = StorageMap<_, Blake2_128Concat, String, u8, ValueQuery>;

    #[pallet::storage]
    pub(super) type Account<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

    #[pallet::storage]
    pub(super) type MigratedAccounts<T: Config> =
        StorageMap<_, Blake2_128Concat, String, T::AccountId>;

    #[pallet::storage]
    pub(super) type PendingMultiSigAccounts<T: Config> =
        StorageMap<_, Blake2_128Concat, String, PendingMultisigAccount<T>, ValueQuery>;

    #[pallet::storage]
    pub(super) type PendingReferrals<T: Config> =
        StorageMap<_, Blake2_128Concat, String, Vec<T::AccountId>, ValueQuery>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub account_id: Option<T::AccountId>,
        pub iroha_accounts: Vec<(String, Balance, Option<String>, u8, Vec<String>)>,
    }

    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                account_id: Default::default(),
                iroha_accounts: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            if let Some(account_id) = self.account_id.as_ref() {
                if frame_system::Pallet::<T>::inc_consumers(account_id).is_ok() {
                    Account::<T>::put(account_id);
                } else {
                    frame_support::__private::log::error!(
                        "IrohaMigration genesis failed to increase consumers for account: {:?}",
                        account_id
                    );
                }
            } else {
                frame_support::__private::log::error!(
                    "IrohaMigration genesis account_id is not configured"
                );
            }

            for (account_id, balance, referrer, threshold, public_keys) in &self.iroha_accounts {
                Balances::<T>::insert(account_id, *balance);
                if let Some(referrer) = referrer {
                    Referrers::<T>::insert(account_id, referrer.clone());
                }
                PublicKeys::<T>::insert(
                    account_id,
                    public_keys
                        .iter()
                        .map(|key| (false, key.to_lowercase()))
                        .collect::<Vec<_>>(),
                );
                if public_keys.len() > 1 {
                    Quorums::<T>::insert(account_id, *threshold);
                }
            }
        }
    }
}
