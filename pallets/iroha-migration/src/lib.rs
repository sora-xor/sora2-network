//! This pallet provides means of migration for Iroha users.
//! It relies on some configuration provided by the genesis block:
//! * Iroha accounts
//! * Account (an account that have permissions to mint VAL, balances are migrated by minting VAL with this account)
//!
//! All migrated accounts are stored to use when their referrals migrate or when a user attempts to migrate again

#![cfg_attr(not(feature = "std"), no_std)]

#[macro_use]
extern crate alloc;
use alloc::string::String;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

use common::prelude::Balance;
use common::VAL;
use ed25519_dalek_iroha::{Digest, PublicKey, Signature, SIGNATURE_LENGTH};
use frame_support::codec::{Decode, Encode};
use frame_support::dispatch::DispatchError;
use frame_support::sp_runtime::traits::Zero;
use frame_support::weights::Pays;
use frame_support::{ensure, RuntimeDebug};
use frame_system::ensure_signed;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sha3::Sha3_256;
use sp_std::convert::TryInto;
use sp_std::prelude::*;

pub const TECH_ACCOUNT_PREFIX: &[u8] = b"iroha-migration";
pub const TECH_ACCOUNT_MAIN: &[u8] = b"main";

fn blocks_till_migration<T>() -> T::BlockNumber
where
    T: frame_system::Config,
{
    // 1 month
    446400u32.into()
}

#[derive(PartialEq, Eq, Clone, RuntimeDebug, Encode, Decode)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
struct PendingMultisigAccount<T>
where
    T: frame_system::Config,
{
    approving_accounts: Vec<T::AccountId>,
    migrate_at: Option<T::BlockNumber>,
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
        Ok(Signature::new(signature_bytes))
    }

    fn verify_signature(
        iroha_address: &str,
        iroha_public_key: &str,
        iroha_signature: &str,
    ) -> Result<(), DispatchError> {
        let public_key = Self::parse_public_key(iroha_public_key)?;
        let signature = Self::parse_signature(iroha_signature)?;
        let message = format!("{}{}", iroha_address, iroha_public_key);
        frame_support::debug::error!("faucet: message: {}", message);
        let mut prehashed_message = Sha3_256::default();
        prehashed_message.update(&message[..]);
        {
            let mut prehashed_message = Sha3_256::default();
            prehashed_message.update(&message[..]);
            let hashed_message = prehashed_message.finalize();
            frame_support::debug::error!("faucet: hashed_message: {}", hex::encode(hashed_message.as_slice()));
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
                pallet_multisig::Module::<T>::multi_account_id(&signatories, quorum as u16);
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
                assets::Pallet::<T>::mint_to(&VAL.into(), &Account::<T>::get(), account, balance)?;
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
                referral_system::Pallet::<T>::set_referrer_to(&account, referrer)
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
            referral_system::Pallet::<T>::set_referrer_to(referral, account.clone())
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
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::config]
    pub trait Config:
        frame_system::Config + pallet_multisig::Config + referral_system::Config + technical::Config
    {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_finalize(block_number: T::BlockNumber) {
            common::with_benchmark(
                common::location_stamp!("iroha-migration.on_finalize"),
                || {
                    // Migrate accounts whose quorum has been reached and enough time has passed since then
                    PendingMultiSigAccounts::<T>::translate(
                        |key, mut value: PendingMultisigAccount<T>| {
                            if let Some(migrate_at) = value.migrate_at {
                                if block_number >= migrate_at {
                                    value.approving_accounts.sort();
                                    let quorum = Quorums::<T>::take(&key);
                                    let multi_account =
                                        pallet_multisig::Module::<T>::multi_account_id(
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
                        },
                    )
                },
            )
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight((0, Pays::No))]
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
                frame_support::debug::error!("faucet: iroha_public_key: {}", iroha_public_key);
                frame_support::debug::error!("faucet: iroha_signature: {}", iroha_signature);
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
                Ok(().into())
            })
        }
    }

    #[pallet::event]
    #[pallet::metadata(AccountIdOf<T> = "AccountId")]
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
    pub(super) type Account<T: Config> = StorageValue<_, T::AccountId, ValueQuery>;

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
        pub account_id: T::AccountId,
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
            Account::<T>::put(&self.account_id);

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
