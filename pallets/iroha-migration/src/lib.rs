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

use common::prelude::Balance;
use common::{FromGenericPair, VAL};
use ed25519_dalek_iroha::{Digest, PublicKey, Signature, SIGNATURE_LENGTH};
use frame_support::codec::{Decode, Encode};
use frame_support::dispatch::{DispatchError, Pays};
use log::error;
use frame_support::sp_runtime::traits::Zero;
use frame_support::weights::Weight;
use frame_support::{ensure, RuntimeDebug};
use frame_system::ensure_signed;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sha3::Sha3_256;
use sp_std::convert::TryInto;
use sp_std::prelude::*;

type WeightInfoOf<T> = <T as Config>::WeightInfo;
pub use weights::WeightInfo;

pub const TECH_ACCOUNT_PREFIX: &[u8] = b"iroha-migration";
pub const TECH_ACCOUNT_MAIN: &[u8] = b"main";

fn blocks_till_migration<T>() -> BlockNumberFor<T>
where
    T: frame_system::Config,
{
    // 1 month
    446400u32.into()
}

#[derive(PartialEq, Eq, Clone, RuntimeDebug, Encode, Decode, scale_info::TypeInfo)]
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
    pub fn needs_migration(iroha_address: &String) -> bool {
        Balances::<T>::contains_key(iroha_address)
            && !MigratedAccounts::<T>::contains_key(iroha_address)
    }

    fn migrate_weight(
        iroha_address: &String,
        iroha_public_key: &String,
        iroha_signature: &String,
    ) -> (Weight, Pays) {
        let pays = if Self::check_migrate(iroha_address, iroha_public_key, iroha_signature).is_ok()
        {
            Pays::No
        } else {
            Pays::Yes
        };
        (WeightInfoOf::<T>::migrate(), pays)
    }

    /// Checks if migration would succeed if the parameters were passed to migrate extrinsic.
    fn check_migrate(
        iroha_address: &String,
        iroha_public_key: &String,
        iroha_signature: &String,
    ) -> Result<(), DispatchError> {
        let iroha_public_key = iroha_public_key.to_lowercase();
        let iroha_signature = iroha_signature.to_lowercase();
        ensure!(
            !MigratedAccounts::<T>::contains_key(&iroha_address),
            Error::<T>::AccountAlreadyMigrated
        );
        // This account isn't migrated so the abusers couldn't copy signature from the blockchain for single-signature accounts.
        Self::verify_signature(&iroha_address, &iroha_public_key, &iroha_signature)?;
        // However, for multi-signature accounts, their signatures can be abused so we continue checking.
        let public_keys =
            PublicKeys::<T>::try_get(&iroha_address).map_err(|_| Error::<T>::PublicKeyNotFound)?;
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
        Ok(())
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
    ) -> Result<(), DispatchError> {
        let public_key = Self::parse_public_key(iroha_public_key)?;
        let signature = Self::parse_signature(iroha_signature)?;
        let message = format!("{}{}", iroha_address, iroha_public_key);
        error!("faucet: message: {}", message);
        let mut prehashed_message = Sha3_256::default();
        prehashed_message.update(&message[..]);
        {
            let mut prehashed_message = Sha3_256::default();
            prehashed_message.update(&message[..]);
            let hashed_message = prehashed_message.finalize();
            error!(
                "faucet: hashed_message: {}",
                hex::encode(hashed_message.as_slice())
            );
        }
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
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type WeightInfo: WeightInfo;
    }

    /// The current storage version.
    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(PhantomData<T>);

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
        #[pallet::weight(Pallet::<T>::migrate_weight(iroha_address, iroha_public_key, iroha_signature))]
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
                log::error!("faucet: iroha_public_key: {}", iroha_public_key);
                log::error!("faucet: iroha_signature: {}", iroha_signature);
                Self::verify_signature(&iroha_address, &iroha_public_key, &iroha_signature)?;
                ensure!(
                    !MigratedAccounts::<T>::contains_key(&iroha_address),
                    Error::<T>::AccountAlreadyMigrated
                );
                ensure!(
                    PublicKeys::<T>::contains_key(&iroha_address),
                    Error::<T>::AccountNotFound
                );
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
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Migrated. [source, target]
        Migrated(String, AccountIdOf<T>),
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

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                account_id: Default::default(),
                iroha_accounts: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            frame_system::Pallet::<T>::inc_consumers(&self.account_id.as_ref().unwrap()).unwrap();
            Account::<T>::put(&self.account_id.as_ref().unwrap());

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
