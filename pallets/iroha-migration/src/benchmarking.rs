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

use alloc::string::{String, ToString};

use codec::Decode;
use common::AssetInfoProvider;
use ed25519_dalek_iroha::{Digest, Keypair, PublicKey, SecretKey};
use frame_benchmarking::benchmarks;
use frame_support::assert_ok;
use frame_support::traits::OnInitialize;
use frame_system::{EventRecord, RawOrigin};
use hex_literal::hex;
use sha3::Sha3_256;
use sp_std::prelude::*;

use common::VAL;

use crate::{
    Balances, Config, Event, MigratedAccounts, Pallet, PendingMultiSigAccounts, PublicKeys, Quorums,
};

fn alice<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).expect("Failed to decode account ID")
}

fn bob<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27f");
    T::AccountId::decode(&mut &bytes[..]).expect("Failed to decode account ID")
}

fn test_keypair(seed: u8) -> Keypair {
    let secret = SecretKey::from_bytes(&[seed; 32]).expect("benchmark secret key must be valid");
    let public = PublicKey::from(&secret);
    Keypair { secret, public }
}

fn public_key_hex(keypair: &Keypair) -> String {
    hex::encode(keypair.public.as_bytes())
}

fn signature_for<T: Config>(
    iroha_address: &str,
    account: &T::AccountId,
    keypair: &Keypair,
) -> String {
    let public_key = public_key_hex(keypair);
    let iroha_address = iroha_address.to_string();
    let message = Pallet::<T>::migration_signing_message(&iroha_address, &public_key, account);
    let mut prehashed_message = Sha3_256::default();
    prehashed_message.update(message.as_bytes());
    let signature = keypair
        .sign_prehashed(prehashed_message, None)
        .expect("benchmark signature should be produced");
    hex::encode(signature.to_bytes())
}

// Adds `n` of unaccessible accounts and after adds 1 account that will be migrated
fn add_accounts<T: Config>(n: u32) {
    let unaccessible_account_id = "did_sora_d9bda3688c6f608ab15c@sora".to_string();
    for _i in 0..n {
        Balances::<T>::insert(&unaccessible_account_id, 0);
        PublicKeys::<T>::insert(
            &unaccessible_account_id,
            vec![(
                false,
                "D9BDA3688c6f608ab15c03a55b171da0413788a40a25722b4ae4d3672890bcd7".to_lowercase(),
            )],
        );
    }

    let account_id = "did_sora_balance@sora".to_string();
    Balances::<T>::insert(&account_id, 300);
    let balance_key = test_keypair(1);
    PublicKeys::<T>::insert(&account_id, vec![(false, public_key_hex(&balance_key))]);

    let multi_sig_account_id = "did_sora_multi_sig@sora".to_string();
    Balances::<T>::insert(&multi_sig_account_id, 1000);
    let multi_sig_key_1 = test_keypair(2);
    let multi_sig_key_2 = test_keypair(3);
    let multi_sig_key_3 = test_keypair(4);
    PublicKeys::<T>::insert(
        &multi_sig_account_id,
        vec![
            (false, public_key_hex(&multi_sig_key_1)),
            (false, public_key_hex(&multi_sig_key_2)),
            (false, public_key_hex(&multi_sig_key_3)),
        ],
    );
    Quorums::<T>::insert(&multi_sig_account_id, 2);
}

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
    let events = frame_system::Pallet::<T>::events();
    let system_event: <T as frame_system::Config>::RuntimeEvent = generic_event.into();
    // compare to the last event record
    let EventRecord { event, .. } = &events[events.len() - 1];
    assert_eq!(event, &system_event);
}

benchmarks! {
    migrate {
        add_accounts::<T>(100);
        let caller = alice::<T>();
        let caller_origin: <T as frame_system::Config>::RuntimeOrigin = RawOrigin::Signed(caller.clone()).into();
        let keypair = test_keypair(1);
        let iroha_address = "did_sora_balance@sora".to_string();
        let public_key = public_key_hex(&keypair);
        let signature = signature_for::<T>(&iroha_address, &caller, &keypair);
    }: {
        Pallet::<T>::migrate(
            caller_origin,
            iroha_address,
            public_key,
            signature)?;
    }
    verify {
        assert_last_event::<T>(Event::<T>::Migrated("did_sora_balance@sora".to_string(), caller).into())
    }

    on_initialize {
        add_accounts::<T>(100);
        let alice = alice::<T>();
        let alice_origin: <T as frame_system::Config>::RuntimeOrigin = RawOrigin::Signed(alice.clone()).into();
        let iroha_address = "did_sora_multi_sig@sora".to_string();
        let alice_key = test_keypair(2);
        let alice_public_key = public_key_hex(&alice_key);
        let alice_signature = signature_for::<T>(&iroha_address, &alice, &alice_key);
        assert_ok!(Pallet::<T>::migrate(
            alice_origin,
            iroha_address.clone(),
            alice_public_key,
            alice_signature)
        );
        let bob = bob::<T>();
        let bob_origin: <T as frame_system::Config>::RuntimeOrigin = RawOrigin::Signed(bob.clone()).into();
        let bob_key = test_keypair(3);
        let bob_public_key = public_key_hex(&bob_key);
        let bob_signature = signature_for::<T>(&iroha_address, &bob, &bob_key);
        assert_ok!(Pallet::<T>::migrate(
            bob_origin,
            iroha_address.clone(),
            bob_public_key,
            bob_signature)
        );
        let multi_account_of_2 = {
            let mut signatories = [alice, bob];
            signatories.sort();
            pallet_multisig::Pallet::<T>::multi_account_id(&signatories, 2)
        };
    }: {
        Pallet::<T>::on_initialize(crate::blocks_till_migration::<T>() + 1u32.into())
    }
    verify {
        assert!(MigratedAccounts::<T>::contains_key(&iroha_address));
        assert!(!PendingMultiSigAccounts::<T>::contains_key(&iroha_address));
        assert_eq!(<T as technical::Config>::AssetInfoProvider::free_balance(&VAL.into(), &multi_account_of_2).unwrap(), 1000);
    }
}

#[cfg(test)]
mod tests {
    use frame_support::assert_ok;

    use crate::mock::{self, Runtime};
    use crate::Pallet;

    #[test]
    fn migrate() {
        mock::test_ext(false).execute_with(|| {
            assert_ok!(Pallet::<Runtime>::test_benchmark_migrate());
            assert_ok!(Pallet::<Runtime>::test_benchmark_on_initialize());
        });
    }
}
