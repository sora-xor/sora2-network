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

use crate::mock::*;
use crate::{
    Account, Balances as IrohaBalances, Error, MigratedAccounts, Pallet, PendingMultiSigAccounts,
    PendingReferrals, PublicKeys,
};
use codec::Encode;
use common::prelude::Balance;
use common::{AssetInfoProvider, VAL};
use ed25519_dalek_iroha::{Digest, Keypair, PublicKey, SecretKey};
use frame_support::assert_noop;
use frame_support::assert_ok;
use frame_support::dispatch::CheckIfFeeless;
use frame_support::traits::OnInitialize;
use referrals::Referrers;
use sha3::Sha3_256;
use sp_core::crypto::AccountId32;

type Assets = assets::Pallet<Runtime>;

fn test_keypair(seed: u8) -> Keypair {
    let secret = SecretKey::from_bytes(&[seed; 32]).expect("test secret key must be valid");
    let public = PublicKey::from(&secret);
    Keypair { secret, public }
}

fn public_key_hex(keypair: &Keypair) -> String {
    hex::encode(keypair.public.as_bytes())
}

fn signature_for(iroha_address: &str, account: &AccountId32, keypair: &Keypair) -> String {
    let public_key = public_key_hex(keypair);
    let iroha_address = iroha_address.to_string();
    let message =
        Pallet::<Runtime>::migration_signing_message(&iroha_address, &public_key, account);
    let mut prehashed_message = Sha3_256::default();
    prehashed_message.update(message.as_bytes());
    let signature = keypair
        .sign_prehashed(prehashed_message, None)
        .expect("test signature should be produced");
    hex::encode(signature.to_bytes())
}

fn install_public_keys(iroha_address: &str, keypairs: &[&Keypair]) {
    PublicKeys::<Runtime>::insert(
        iroha_address.to_string(),
        keypairs
            .iter()
            .map(|keypair| (false, public_key_hex(keypair)))
            .collect::<Vec<_>>(),
    );
}

fn migrate_with_key(
    account: AccountId32,
    iroha_address: &str,
    keypair: &Keypair,
) -> frame_support::dispatch::DispatchResultWithPostInfo {
    let public_key = public_key_hex(keypair);
    let signature = signature_for(iroha_address, &account, keypair);
    Pallet::<Runtime>::migrate(
        RuntimeOrigin::signed(account),
        iroha_address.to_string(),
        public_key,
        signature,
    )
}

fn migrate_call_with_key(
    account: AccountId32,
    iroha_address: &str,
    keypair: &Keypair,
) -> RuntimeCall {
    RuntimeCall::IrohaMigration(crate::Call::<Runtime>::migrate {
        iroha_address: iroha_address.to_string(),
        iroha_public_key: public_key_hex(keypair),
        iroha_signature: signature_for(iroha_address, &account, keypair),
    })
}

#[test]
fn signing_message_uses_configured_genesis_hash() {
    new_test_ext().execute_with(|| {
        let storage_genesis_hash = sp_core::H256([7; 32]);
        frame_system::BlockHash::<Runtime>::insert(0, storage_genesis_hash);

        let key = test_keypair(1);
        let message = Pallet::<Runtime>::migration_signing_message(
            "did_sora_balance@sora",
            &public_key_hex(&key),
            &ALICE,
        );

        assert!(message.contains(&format!(
            "genesis_hash=0x{}",
            hex::encode(MigrationGenesisHash::get().encode())
        )));
        assert!(!message.contains(&format!(
            "genesis_hash=0x{}",
            hex::encode(storage_genesis_hash.encode())
        )));
    });
}

#[test]
fn valid_migration_call_is_feeless_before_dispatch() {
    new_test_ext().execute_with(|| {
        let key = test_keypair(1);
        let iroha_address = "did_sora_balance@sora";
        install_public_keys(iroha_address, &[&key]);

        let call = migrate_call_with_key(ALICE, iroha_address, &key);
        assert!(call.is_feeless(&RuntimeOrigin::signed(ALICE)));
        assert!(!call.is_feeless(&RuntimeOrigin::signed(BOB)));

        let bad_call = RuntimeCall::IrohaMigration(crate::Call::<Runtime>::migrate {
            iroha_address: iroha_address.to_string(),
            iroha_public_key: public_key_hex(&key),
            iroha_signature: "00".to_string(),
        });
        assert!(!bad_call.is_feeless(&RuntimeOrigin::signed(ALICE)));
    });
}

#[test]
fn migrated_account_is_not_feeless_again() {
    new_test_ext().execute_with(|| {
        let key = test_keypair(1);
        let iroha_address = "did_sora_balance@sora";
        install_public_keys(iroha_address, &[&key]);

        let call = migrate_call_with_key(ALICE, iroha_address, &key);
        assert!(call.is_feeless(&RuntimeOrigin::signed(ALICE)));

        assert_ok!(migrate_with_key(ALICE, iroha_address, &key));

        let repeated_call = migrate_call_with_key(ALICE, iroha_address, &key);
        assert!(!repeated_call.is_feeless(&RuntimeOrigin::signed(ALICE)));
    });
}

#[test]
fn test_verification_failed() {
    new_test_ext().execute_with(|| {
        let key = test_keypair(1);
        let iroha_address = "did_sora_d9bda3688c6f608ab15c@sora";
        install_public_keys(iroha_address, &[&key]);
        assert_noop!(Pallet::<Runtime>::migrate(
            RuntimeOrigin::signed(ALICE),
            iroha_address.to_string(),
            public_key_hex(&key),
            "fffffffffb19abcfc869eae8f14389680aecc7afb5959fb87c2fee65951a46a7507f8bf11ee0c609fb101fd41d6534b84bb8c3e55a79189de96bcc8227fa5c01".to_string()
        ),
            Error::<Runtime>::SignatureVerificationFailed);
    });
}

#[test]
fn test_account_not_found() {
    new_test_ext().execute_with(|| {
        let key = test_keypair(1);
        assert_noop!(
            migrate_with_key(ALICE, "did_sora_1@sora", &key),
            Error::<Runtime>::AccountNotFound
        );
    });
}

#[test]
fn legacy_signature_is_rejected() {
    new_test_ext().execute_with(|| {
        assert_noop!(Pallet::<Runtime>::migrate(
            RuntimeOrigin::signed(ALICE),
            "did_sora_balance@sora".to_string(),
            "9a685d77bcd3f60e6cc1e91eedc7a48e11bbcf1a036b920f3bae0372a78a5432".to_string(),
            "233896712f752760713539f56c92534ff8f4f290812e8f129ce0b513b99cbdffcea95abeed68edd1b0a4e4b52877c13c26c6c89e5bb6bf023ac6c0f4f53c0c02".to_string()
        ),
            Error::<Runtime>::SignatureVerificationFailed
        );
    });
}

#[test]
fn bound_signature_cannot_be_submitted_by_another_account() {
    new_test_ext().execute_with(|| {
        let key = test_keypair(1);
        let iroha_address = "did_sora_balance@sora";
        install_public_keys(iroha_address, &[&key]);
        let public_key = public_key_hex(&key);
        let signature = signature_for(iroha_address, &ALICE, &key);

        assert_noop!(
            Pallet::<Runtime>::migrate(
                RuntimeOrigin::signed(BOB),
                iroha_address.to_string(),
                public_key,
                signature
            ),
            Error::<Runtime>::SignatureVerificationFailed
        );
        assert!(!MigratedAccounts::<Runtime>::contains_key(
            iroha_address.to_string()
        ));
    });
}

#[test]
fn test_already_migrated() {
    new_test_ext().execute_with(|| {
        let key = test_keypair(1);
        let iroha_address = "did_sora_d9bda3688c6f608ab15c@sora";
        install_public_keys(iroha_address, &[&key]);

        assert_ok!(migrate_with_key(ALICE, iroha_address, &key));
        assert!(MigratedAccounts::<Runtime>::contains_key(
            iroha_address.to_string()
        ));
        assert_noop!(
            migrate_with_key(ALICE, iroha_address, &key),
            Error::<Runtime>::AccountAlreadyMigrated
        );
    });
}

#[test]
fn test_migrate_balance() {
    new_test_ext().execute_with(|| {
        let key = test_keypair(1);
        let iroha_address = "did_sora_balance@sora";
        install_public_keys(iroha_address, &[&key]);

        assert_eq!(
            Assets::free_balance(&VAL, &ALICE).unwrap(),
            Balance::from(0u128)
        );
        assert_ok!(migrate_with_key(ALICE, iroha_address, &key));
        assert_eq!(
            Assets::free_balance(&VAL, &ALICE).unwrap(),
            Balance::from(300u128)
        );
    });
}

#[test]
fn test_migrate_referrer_migrates_first() {
    new_test_ext().execute_with(|| {
        let referrer_key = test_keypair(1);
        let referral_key = test_keypair(2);
        install_public_keys("did_sora_referrer@sora", &[&referrer_key]);
        install_public_keys("did_sora_referral@sora", &[&referral_key]);

        assert_ok!(migrate_with_key(
            ALICE,
            "did_sora_referrer@sora",
            &referrer_key
        ));
        assert_ok!(migrate_with_key(
            BOB,
            "did_sora_referral@sora",
            &referral_key
        ));
        assert_eq!(Referrers::<Runtime>::get(&BOB), Some(ALICE));
        assert!(PendingReferrals::<Runtime>::get(&"did_sora_referrer@sora".to_string()).is_empty());
    });
}

#[test]
fn test_migrate_referral_migrates_first() {
    new_test_ext().execute_with(|| {
        let referrer_key = test_keypair(1);
        let referral_key = test_keypair(2);
        install_public_keys("did_sora_referrer@sora", &[&referrer_key]);
        install_public_keys("did_sora_referral@sora", &[&referral_key]);

        assert_ok!(migrate_with_key(
            BOB,
            "did_sora_referral@sora",
            &referral_key
        ));
        assert_eq!(
            PendingReferrals::<Runtime>::get(&"did_sora_referrer@sora".to_string()),
            vec![BOB]
        );
        assert_ok!(migrate_with_key(
            ALICE,
            "did_sora_referrer@sora",
            &referrer_key
        ));
        assert_eq!(Referrers::<Runtime>::get(&BOB), Some(ALICE));
    });
}

#[test]
fn test_migrate_multi_sig() {
    new_test_ext().execute_with(|| {
        let iroha_address = "did_sora_multi_sig@sora".to_string();
        let alice_key = test_keypair(1);
        let bob_key = test_keypair(2);
        let charlie_key = test_keypair(3);
        install_public_keys(&iroha_address, &[&alice_key, &bob_key, &charlie_key]);

        assert_ok!(migrate_with_key(ALICE, &iroha_address, &alice_key));
        assert!(!MigratedAccounts::<Runtime>::contains_key(&iroha_address));
        assert!(PendingMultiSigAccounts::<Runtime>::contains_key(
            &iroha_address
        ));
        let multi_account = {
            let mut signatories = [ALICE, BOB, CHARLIE];
            signatories.sort();
            pallet_multisig::Pallet::<Runtime>::multi_account_id(&signatories, 2)
        };
        assert_eq!(
            Assets::free_balance(&VAL, &multi_account).unwrap(),
            Balance::from(0u128)
        );
        assert_ok!(migrate_with_key(BOB, &iroha_address, &bob_key));
        assert!(PendingMultiSigAccounts::<Runtime>::contains_key(
            &iroha_address
        ));
        assert_ok!(migrate_with_key(CHARLIE, &iroha_address, &charlie_key));
        assert!(MigratedAccounts::<Runtime>::contains_key(&iroha_address));
        assert!(!PendingMultiSigAccounts::<Runtime>::contains_key(
            &iroha_address
        ));
        assert_eq!(
            Assets::free_balance(&VAL, &multi_account).unwrap(),
            Balance::from(1000u128)
        );
    });
}

#[test]
fn test_migrate_multi_sig_after_timeout() {
    new_test_ext().execute_with(|| {
        let iroha_address = "did_sora_multi_sig@sora".to_string();
        let alice_key = test_keypair(1);
        let bob_key = test_keypair(2);
        let charlie_key = test_keypair(3);
        install_public_keys(&iroha_address, &[&alice_key, &bob_key, &charlie_key]);

        assert_ok!(migrate_with_key(ALICE, &iroha_address, &alice_key));

        assert!(!MigratedAccounts::<Runtime>::contains_key(&iroha_address));
        assert!(PendingMultiSigAccounts::<Runtime>::contains_key(
            &iroha_address
        ));
        let multi_account_of_2 = {
            let mut signatories = [ALICE, BOB];
            signatories.sort();
            pallet_multisig::Pallet::<Runtime>::multi_account_id(&signatories, 2)
        };
        let multi_account_of_3 = {
            let mut signatories = [ALICE, BOB, CHARLIE];
            signatories.sort();
            pallet_multisig::Pallet::<Runtime>::multi_account_id(&signatories, 2)
        };
        assert_eq!(
            Assets::free_balance(&VAL, &multi_account_of_2).unwrap(),
            Balance::from(0u128)
        );
        assert_eq!(
            Assets::free_balance(&VAL, &multi_account_of_3).unwrap(),
            Balance::from(0u128)
        );

        assert_ok!(migrate_with_key(BOB, &iroha_address, &bob_key));

        assert!(!MigratedAccounts::<Runtime>::contains_key(&iroha_address));
        assert!(PendingMultiSigAccounts::<Runtime>::contains_key(
            &iroha_address
        ));
        assert_eq!(
            Assets::free_balance(&VAL, &multi_account_of_2).unwrap(),
            Balance::from(0u128)
        );
        assert_eq!(
            Assets::free_balance(&VAL, &multi_account_of_3).unwrap(),
            Balance::from(0u128)
        );

        Pallet::<Runtime>::on_initialize(crate::blocks_till_migration::<Runtime>() + 1);

        assert!(MigratedAccounts::<Runtime>::contains_key(&iroha_address));
        assert!(!PendingMultiSigAccounts::<Runtime>::contains_key(
            &iroha_address
        ));
        assert_eq!(
            Assets::free_balance(&VAL, &multi_account_of_2).unwrap(),
            Balance::from(1000u128)
        );
        assert_eq!(
            Assets::free_balance(&VAL, &multi_account_of_3).unwrap(),
            Balance::from(0u128)
        );

        assert_noop!(
            Pallet::<Runtime>::migrate(
                RuntimeOrigin::signed(CHARLIE),
                iroha_address.clone(),
                public_key_hex(&charlie_key),
                signature_for(&iroha_address, &CHARLIE, &charlie_key)
            ),
            Error::<Runtime>::AccountAlreadyMigrated,
        );
    });
}

#[test]
fn test_migrate_multi_sig_public_key_already_used() {
    new_test_ext().execute_with(|| {
        let iroha_address = "did_sora_multi_sig@sora".to_string();
        let alice_key = test_keypair(1);
        let bob_key = test_keypair(2);
        install_public_keys(&iroha_address, &[&alice_key, &bob_key]);

        assert_ok!(migrate_with_key(ALICE, &iroha_address, &alice_key));
        assert!(!MigratedAccounts::<Runtime>::contains_key(&iroha_address));
        assert!(PendingMultiSigAccounts::<Runtime>::contains_key(
            &iroha_address
        ));
        assert_noop!(
            Pallet::<Runtime>::migrate(
                RuntimeOrigin::signed(ALICE),
                iroha_address.clone(),
                public_key_hex(&alice_key),
                signature_for(&iroha_address, &ALICE, &alice_key)
            ),
            Error::<Runtime>::PublicKeyAlreadyUsed
        );
    });
}

#[test]
fn genesis_should_set_account_when_configured() {
    test_ext_with_account_id(true, Some(MINTING_ACCOUNT)).execute_with(|| {
        assert_eq!(Account::<Runtime>::get(), Some(MINTING_ACCOUNT));
    });
}

#[test]
fn genesis_should_not_set_account_when_account_id_missing() {
    test_ext_with_account_id(true, None).execute_with(|| {
        assert_eq!(Account::<Runtime>::get(), None);
        assert_eq!(
            IrohaBalances::<Runtime>::get("did_sora_balance@sora"),
            Some(Balance::from(300u128))
        );
    });
}
