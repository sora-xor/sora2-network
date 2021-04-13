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
