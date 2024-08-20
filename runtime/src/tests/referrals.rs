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

use common::mock::{alice, bob, charlie};
use common::prelude::constants::SMALL_FEE;
use common::{AssetInfoProvider, XOR};
use frame_support::{assert_err, assert_ok};
use framenode_chain_spec::ext;
use sp_runtime::TokenError;

use crate::{Assets, Currencies, Referrals, Runtime, RuntimeOrigin};

type E = referrals::Error<Runtime>;

#[test]
fn set_referrer_to() {
    ext().execute_with(|| {
        assert_ok!(Referrals::set_referrer_to(&alice(), alice()));
        assert_eq!(
            referrals::Referrers::<Runtime>::get(&alice()),
            Some(alice())
        );
        assert_eq!(
            referrals::Referrals::<Runtime>::get(&alice()),
            vec![alice()]
        );
    });
}

#[test]
fn set_referrer_to_has_referrer() {
    ext().execute_with(|| {
        assert_ok!(Referrals::set_referrer_to(&alice(), bob()));

        assert_err!(
            Referrals::set_referrer_to(&alice(), charlie()),
            E::AlreadyHasReferrer
        );
    });
}

#[test]
fn reserve_insufficient_balance() {
    ext().execute_with(|| {
        assert_err!(
            Referrals::reserve(RuntimeOrigin::signed(alice()), 1),
            TokenError::FundsUnavailable
        );
    })
}

#[test]
fn reserve_unreserve() {
    ext().execute_with(|| {
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            alice(),
            XOR.into(),
            SMALL_FEE as i128 * 3
        ));

        assert_ok!(Referrals::reserve(
            RuntimeOrigin::signed(alice()),
            3 * SMALL_FEE
        ));

        assert!(referrals::ReferrerBalances::<Runtime>::contains_key(
            &alice()
        ));

        for _ in 0..3 {
            assert_ok!(Referrals::unreserve(
                RuntimeOrigin::signed(alice()),
                SMALL_FEE
            ));
        }

        assert_eq!(
            Assets::free_balance(&XOR.into(), &alice()),
            Ok(SMALL_FEE * 3)
        );

        assert!(!referrals::ReferrerBalances::<Runtime>::contains_key(
            &alice()
        ));
    })
}

#[test]
fn withdraw_fee_insufficient_balance() {
    ext().execute_with(|| {
        assert_err!(
            Referrals::withdraw_fee(&alice(), SMALL_FEE),
            referrals::Error::<Runtime>::ReferrerInsufficientBalance
        );
    })
}

#[test]
fn withdraw() {
    ext().execute_with(|| {
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            alice(),
            XOR.into(),
            SMALL_FEE as i128
        ));

        assert_ok!(Referrals::reserve(
            RuntimeOrigin::signed(alice()),
            SMALL_FEE
        ));

        assert_ok!(Referrals::withdraw_fee(&alice(), SMALL_FEE));
    })
}
