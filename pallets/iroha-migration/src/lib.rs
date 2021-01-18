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

use common::{prelude::Balance, VAL};
use ed25519_dalek_iroha::{Digest, PublicKey, Signature, SIGNATURE_LENGTH};
use frame_support::{
    decl_error, decl_event, decl_module, decl_storage,
    dispatch::{DispatchError, DispatchResult},
    ensure,
    sp_runtime::{traits::Zero, Percent},
    weights::Pays,
};
use frame_system::{ensure_signed, RawOrigin};
use sha3::Sha3_256;
use sp_std::prelude::*;

pub const TECH_ACCOUNT_PREFIX: &[u8] = b"iroha-migration";
pub const TECH_ACCOUNT_MAIN: &[u8] = b"main";

pub trait Trait:
    frame_system::Trait + multisig::Trait + referral_system::Trait + technical::Trait
where
    Self::Origin: From<RawOrigin<<Self as frame_system::Trait>::AccountId>>,
{
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
}

decl_storage! {
    trait Store for Module<T: Trait> as IrohaMigration {
        // Contains balances of Iroha accounts. Iroha account (represented by its address) => VAL balance
        Balances get(fn balance): map hasher(blake2_128_concat) String => Option<Balance>;

        // Contains referrers of Iroha accounts. Referral Iroha account => Referrer Iroha account
        Referrers get(fn referrer): map hasher(blake2_128_concat) String => Option<String>;

        // Contains public keys that are required to migrate the account.
        // Iroha account => public keys
        PublicKeys get(fn public_keys): map hasher(blake2_128_concat) String => Vec<(bool, String)>;

        // Contains thresholds of approval with public keys of Iroha account for Iroha account to be migrated.
        // If the account has multiple public keys.
        // Iroha account => number of keys to complete migration
        FlatThresholds get(fn flat_threshold): map hasher(blake2_128_concat) String => u8;

        // Contains thresholds of approval with public keys of Iroha account for Iroha account to be migrated.
        // If the account has multiple public keys.
        // Iroha account => percent of keys to complete migration
        PercentThresholds get(fn percent_threshold): map hasher(blake2_128_concat) String => Percent;

        // Contains the account that VAL is minted with
        Account get(fn account_id) config(account_id): T::AccountId;

        // Contains migrated accounts. Iroha account => Substrate account
        MigratedAccounts: map hasher(blake2_128_concat) String => Option<T::AccountId>;

        // Contains multi-signature accounts that wait for other key holders to migrate.
        // Iroha address => (number of public keys that approved the migration, Substrate multi-signature account)
        PendingMultiSigAccounts: map hasher(blake2_128_concat) String => T::AccountId;

        // Contains pending referrals that wait for their referrer to migrate. Referrer Iroha account => Referral Iroha accounts that migrated to Substrate
        PendingReferrals get(fn pending_referrals): map hasher(blake2_128_concat) String => Vec<T::AccountId>;
    }

    add_extra_genesis {
        config(iroha_accounts): Vec<(String, Balance, Option<String>, u8, Vec<String>)>;

        build(|config| {
            for (account_id, balance, referrer, threshold, public_keys) in &config.iroha_accounts {
                Balances::insert(account_id, *balance);
                if let Some(referrer) = referrer {
                    Referrers::insert(account_id, referrer.clone());
                }
                PublicKeys::insert(
                    account_id,
                    public_keys
                    .iter()
                    .map(|key| (false, key.clone()))
                    .collect::<Vec<_>>());
                if public_keys.len() > 1 {
                    FlatThresholds::insert(account_id, *threshold);
                    let percent = Percent::from_rational_approximation(*threshold as usize, public_keys.len());
                    PercentThresholds::insert(account_id, percent);
                }
            }
        })
    }
}

decl_event!(
    pub enum Event<T>
    where
        AccountId = <T as frame_system::Trait>::AccountId,
    {
        /// Migrated. [source, target]
        Migrated(String, AccountId),
    }
);

decl_error! {
    pub enum Error for Module<T: Trait> {
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
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        type Error = Error<T>;

        fn deposit_event() = default;

        #[weight = (0, Pays::No)]
        pub fn migrate(
            origin,
            iroha_address: String,
            iroha_public_key: String,
            iroha_signature: String
        ) -> DispatchResult {
            common::with_transaction(|| {
                let who = ensure_signed(origin)?;
                Self::verify_signature(&iroha_address, &iroha_public_key, &iroha_signature)?;
                ensure!(PublicKeys::contains_key(&iroha_address), Error::<T>::AccountNotFound);
                let (approval_count, key_count) = Self::approve_with_public_key(&iroha_address, &iroha_public_key)?;
                if key_count == 1 {
                    Self::migrate_account(iroha_address, who)
                } else {
                    Self::on_multisig_account_approved(iroha_address, who, approval_count, key_count)
                }
            })
        }
    }
}

impl<T: Trait> Module<T> {
    fn create_public_key(iroha_public_key: &str) -> Result<PublicKey, DispatchError> {
        let iroha_public_key =
            hex::decode(&iroha_public_key).map_err(|_| Error::<T>::PublicKeyParsingFailed)?;
        let public_key = PublicKey::from_bytes(iroha_public_key.as_slice())
            .map_err(|_| Error::<T>::PublicKeyParsingFailed)?;
        Ok(public_key)
    }

    fn create_signature(iroha_signature: &str) -> Result<Signature, DispatchError> {
        let iroha_signature =
            hex::decode(&iroha_signature).map_err(|_| Error::<T>::SignatureParsingFailed)?;
        ensure!(
            iroha_signature.len() == SIGNATURE_LENGTH,
            Error::<T>::SignatureParsingFailed
        );
        let mut signature_bytes = [0; SIGNATURE_LENGTH];
        for i in 0..SIGNATURE_LENGTH {
            signature_bytes[i] = iroha_signature[i];
        }
        Ok(Signature::new(signature_bytes))
    }

    fn verify_signature(
        iroha_address: &str,
        iroha_public_key: &str,
        iroha_signature: &str,
    ) -> Result<(), DispatchError> {
        let public_key = Self::create_public_key(iroha_public_key)?;
        let signature = Self::create_signature(iroha_signature)?;
        let message = format!("{}{}", iroha_address, iroha_public_key);
        let mut prehashed_message = Sha3_256::default();
        prehashed_message.update(&message[..]);
        public_key
            .verify_prehashed(prehashed_message, None, &signature)
            .map_err(|_| Error::<T>::SignatureVerificationFailed)?;
        Ok(())
    }

    fn approve_with_public_key(
        iroha_address: &String,
        iroha_public_key: &String,
    ) -> Result<(usize, usize), DispatchError> {
        PublicKeys::mutate(iroha_address, |keys| {
            if let Some((already_approved, _)) =
                keys.iter_mut().find(|(_, key)| key == iroha_public_key)
            {
                if !*already_approved {
                    *already_approved = true;
                } else if MigratedAccounts::<T>::contains_key(iroha_address) {
                    return Err(Error::<T>::AccountAlreadyMigrated.into());
                } else {
                    return Err(Error::<T>::PublicKeyAlreadyUsed.into());
                }
            } else {
                return Err(Error::<T>::PublicKeyNotFound.into());
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
        let multisig_account_id = if approval_count == 1 {
            let block_num = frame_system::Module::<T>::block_number();
            let nonce = frame_system::Module::<T>::account_nonce(&account);
            let multisig_account_id =
                multisig::Module::<T>::multi_account_id(&account, block_num, nonce);
            let percent_threshold = Self::percent_threshold(&iroha_address);
            multisig::Module::<T>::register_multisig(
                RawOrigin::Signed(account.clone()).into(),
                vec![account.clone()],
                percent_threshold,
            )
            .map_err(|_| Error::<T>::MultiSigCreationFailed)?;
            PendingMultiSigAccounts::<T>::insert(&iroha_address, &multisig_account_id);
            multisig_account_id
        } else {
            let multisig_account_id = PendingMultiSigAccounts::<T>::get(&iroha_address);
            multisig::Module::<T>::add_signatory(
                RawOrigin::Signed(multisig_account_id.clone()).into(),
                account,
            )
            .map_err(|_| Error::<T>::SignatoryAdditionFailed)?;
            multisig_account_id
        };
        if approval_count == public_key_count {
            // Free up memory
            FlatThresholds::remove(&iroha_address);
            PercentThresholds::remove(&iroha_address);
            PendingMultiSigAccounts::<T>::remove(&iroha_address);
        }
        let flat_threshold = Self::flat_threshold(&iroha_address);
        if approval_count == flat_threshold as usize {
            Self::migrate_account(iroha_address, multisig_account_id)?;
        }
        Ok(())
    }

    fn migrate_account(iroha_address: String, account: T::AccountId) -> Result<(), DispatchError> {
        Self::migrate_balance(&iroha_address, &account)?;
        Self::migrate_referrals(&iroha_address, &account)?;
        MigratedAccounts::<T>::insert(&iroha_address, &account);
        Self::deposit_event(RawEvent::Migrated(iroha_address, account));
        Ok(())
    }

    fn migrate_balance(
        iroha_address: &String,
        account: &T::AccountId,
    ) -> Result<(), DispatchError> {
        if let Some(balance) = Balances::get(iroha_address) {
            if !balance.is_zero() {
                assets::Module::<T>::mint_to(&VAL.into(), &Account::<T>::get(), account, balance)?;
            }
            Balances::remove(iroha_address); // Free up memory
        }
        Ok(())
    }

    fn migrate_referrals(
        iroha_address: &String,
        account: &T::AccountId,
    ) -> Result<(), DispatchError> {
        // Migrate a referral to their referrer
        if let Some(referrer) = Referrers::get(iroha_address) {
            // Free up memory
            Referrers::remove(iroha_address);
            if let Some(referrer) = MigratedAccounts::<T>::get(&referrer) {
                referral_system::Module::<T>::set_referrer_to(&account, referrer)
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
            referral_system::Module::<T>::set_referrer_to(referral, account.clone())
                .map_err(|_| Error::<T>::ReferralMigrationFailed)?;
        }
        Ok(())
    }
}
