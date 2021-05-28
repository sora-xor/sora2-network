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

use common::{assert_noop_msg, balance, PSWAP, VAL};
use frame_support::{assert_noop, assert_ok};
use hex_literal::hex;

use crate::migration::*;
use crate::mock::*;

type Pallet = crate::Pallet<Runtime>;
type Error = crate::Error<Runtime>;
type Assets = assets::Pallet<Runtime>;

fn account() -> AccountId {
    hex!("f08879dab4530529153a1bdb63e27cd3be45f1574a122b7e88579b6e5e60bd43").into()
}

fn origin() -> Origin {
    Origin::signed(account())
}

#[test]
fn claim_fails_signature_invalid() {
    ExtBuilder::with_rewards(true).build().execute_with(|| {
        assert_noop!(
            Pallet::claim(
                origin(),
                hex!("bb7009c977888910a96d499f802e4524a939702aa6fc8ed473829bffce9289d850b97a720aa05d4a7e70e15733eeebc4fe862dcb").into(),
            ),
            Error::SignatureInvalid
        );
    });
}

#[test]
fn claim_succeeds_zero_v() {
    let account_id: AccountId =
        hex!("7c0f877cd5720eee40d1183556f1fbd34931a6ee08c5299b4de2b2b43176831a").into();
    ExtBuilder::with_rewards(true).build().execute_with(|| {
        let signature = hex!("22bea4c62999dc1be10cb603956b5731dfd296c9e0b0040e5fe8056db1e8df5648c519b704acdcdcf0d04ab01f81f2ed899edef437a4be8f36980d7f1119d7ce00").into();
        assert_ok!(Pallet::claim(Origin::signed(account_id.clone()), signature));
        assert_eq!(
            Assets::free_balance(&PSWAP, &account_id).unwrap(),
            balance!(100)
        );
    });
}

#[test]
fn claim_succeeds() {
    ExtBuilder::with_rewards(true).build().execute_with(|| {
        let signature = hex!("eb7009c977888910a96d499f802e4524a939702aa6fc8ed473829bffce9289d850b97a720aa05d4a7e70e15733eeebc4fe862dcb60e018c0bf560b2de013078f1c").into();
        assert_ok!(Pallet::claim(origin(), signature));
        assert_eq!(
            Assets::free_balance(&VAL, &account()).unwrap(),
            balance!(111)
        );
        assert_eq!(
            Assets::free_balance(&PSWAP, &account()).unwrap(),
            balance!(555)
        );
    });
}

#[test]
fn claim_fails_already_claimed() {
    ExtBuilder::with_rewards(true).build().execute_with(|| {
        let signature: Vec<u8> = hex!("eb7009c977888910a96d499f802e4524a939702aa6fc8ed473829bffce9289d850b97a720aa05d4a7e70e15733eeebc4fe862dcb60e018c0bf560b2de013078f1c").into();
        assert_ok!(Pallet::claim(origin(), signature.clone()));
        assert_noop!(Pallet::claim(origin(), signature), Error::AlreadyClaimed);
    });
}

#[test]
fn claim_fails_no_rewards() {
    ExtBuilder::with_rewards(true).build().execute_with(|| {
        let signature = hex!("6619441577e5173239a52ee52cc7d2eaf57b294defeb0a564e11c4e3c197a95574d81bd4bc747976c1e163be5adecf6bc6ceff69ef3ee2948ff90fdcaa02d5411c").into();
        assert_noop!(Pallet::claim(origin(), signature), Error::NoRewards);
    });
}

#[test]
fn claim_over_limit() {
    ExtBuilder::with_rewards(true).build().execute_with(|| {
        let signature = hex!("20994c1a98b6818832555f5ab840ef6c7d468f46e192bed4921724629475975f440582a9f1416ffd7720538d30af601cbe18ffded8e0eea38c18d24714b57e381b").into();
        assert_noop_msg!(Pallet::claim(origin(), signature), "BalanceTooLow");
    });
}

#[test]
fn val_emission_works() {
    ExtBuilder::with_rewards(true).build().execute_with(|| {
        let rewards_tech_acc = crate::ReservesAcc::<Runtime>::get();
        let rewards_account_id =
            technical::Module::<Runtime>::tech_account_id_to_account_id(&rewards_tech_acc).unwrap();
        let val_minted = balance!(30000); // Sum of allocated VAL rewards in genesis
        assert_eq!(
            technical::Module::<Runtime>::total_balance(&VAL, &rewards_tech_acc).unwrap(),
            balance!(30000)
        );

        let w = mint_remaining_val::<Runtime>(val_minted);
        assert_eq!(w, 1200);

        assert_eq!(
            Assets::free_balance(&VAL, &rewards_account_id).unwrap(),
            balance!(33100000)
        );
    });
}

#[test]
fn storage_migration_v2_works() {
    use crate::EthereumAddress;
    ExtBuilder::with_rewards(true).build().execute_with(|| {
        // Claim some VAL first
        let signature = hex!("eb7009c977888910a96d499f802e4524a939702aa6fc8ed473829bffce9289d850b97a720aa05d4a7e70e15733eeebc4fe862dcb60e018c0bf560b2de013078f1c").into();
        assert_ok!(Pallet::claim(origin(), signature));
        assert_eq!(
            Assets::free_balance(&VAL, &account()).unwrap(),
            balance!(111)
        );
        let diff = vec![
            (
                hex!("886021f300dc809269cfc758a2364a2baf63af0c").into(),
                balance!(2.9933),
            ),
            (
                hex!("a65612f6a7998cbe1b27098f57b3a65612f6a799").into(),
                balance!(55.55),
            ),
        ];

        let w = update_val_airdrop_data::<Runtime>(diff);
        assert_eq!(w, 2600);

        assert_eq!(
            // VAL claimed, no adjustment made
            crate::ValOwners::<Runtime>::get(EthereumAddress::from(hex!("21Bc9f4a3d9Dc86f142F802668dB7D908cF0A636"))),
            balance!(0)
        );
        assert_eq!(
            // No adjustment made, original value must remain
            crate::ValOwners::<Runtime>::get(EthereumAddress::from(hex!("d170a274320333243b9f860e8891c6792de1ec19"))),
            balance!(2888.9933)
        );
        assert_eq!(
            // Added 2.9933 to the original 0.0067
            crate::ValOwners::<Runtime>::get(EthereumAddress::from(hex!("886021f300dc809269cfc758a2364a2baf63af0c"))),
            balance!(3)
        );
        assert_eq!(
            // Newly added address
            crate::ValOwners::<Runtime>::get(EthereumAddress::from(hex!("a65612f6a7998cbe1b27098f57b3a65612f6a799"))),
            balance!(55.55)
        );
        assert_eq!(
            // A Uniswap liquidiy pool account, should have been removed
            crate::ValOwners::<Runtime>::get(EthereumAddress::from(hex!("01962144d41415cca072900fe87bbe2992a99f10"))),
            balance!(0)
        );
        assert_eq!(
            // A Mooniswap liquidiy pool account, should have been removed
            crate::ValOwners::<Runtime>::get(EthereumAddress::from(hex!("215470102a05b02a3a2898f317b5382f380afc0e"))),
            balance!(0)
        );
    });
}
