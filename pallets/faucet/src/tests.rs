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

use common::balance;
use common::prelude::FixedWrapper;
use frame_support::{assert_noop, assert_ok};

use crate::mock::*;
use crate::*;

type Pallet = crate::Pallet<Runtime>;
type Assets = assets::Pallet<Runtime>;
type System = frame_system::Pallet<Runtime>;

#[test]
fn transfer_passes_unsigned() {
    ExtBuilder::build().execute_with(|| {
        // Receive the Limit in two transfers
        assert_ok!(Pallet::transfer(
            RuntimeOrigin::none(),
            XOR,
            bob(),
            (Pallet::transfer_limit() * FixedWrapper::from(0.5)).into_balance()
        ));
        assert_ok!(Pallet::transfer(
            RuntimeOrigin::none(),
            XOR,
            bob(),
            (Pallet::transfer_limit() * FixedWrapper::from(0.5)).into_balance()
        ));
        assert_eq!(
            Assets::free_balance(&XOR, &account_id()).unwrap(),
            (Pallet::transfer_limit() * FixedWrapper::from(0.5)).into_balance()
        );
        assert_eq!(
            Assets::free_balance(&XOR, &bob()).unwrap(),
            Pallet::transfer_limit()
        );
    });
}

#[test]
fn transfer_passes_native_currency() {
    ExtBuilder::build().execute_with(|| {
        // Receive the Limit in two transfers
        assert_ok!(Pallet::transfer(
            RuntimeOrigin::signed(alice()),
            XOR,
            bob(),
            (Pallet::transfer_limit() * FixedWrapper::from(0.5)).into_balance()
        ));
        assert_ok!(Pallet::transfer(
            RuntimeOrigin::signed(alice()),
            XOR,
            bob(),
            (Pallet::transfer_limit() * FixedWrapper::from(0.5)).into_balance()
        ));
        assert_eq!(
            Assets::free_balance(&XOR, &account_id()).unwrap(),
            (Pallet::transfer_limit() * FixedWrapper::from(0.5)).into_balance()
        );
        assert_eq!(Assets::free_balance(&XOR, &alice()).unwrap(), 1);
        assert_eq!(
            Assets::free_balance(&XOR, &bob()).unwrap(),
            Pallet::transfer_limit()
        );
    });
}

#[test]
fn transfer_passes_multiple_assets() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(Pallet::transfer(
            RuntimeOrigin::signed(alice()),
            XOR,
            bob(),
            Pallet::transfer_limit()
        ));
        assert_eq!(
            Assets::free_balance(&XOR, &account_id()).unwrap(),
            (Pallet::transfer_limit() * FixedWrapper::from(0.5)).into_balance()
        );
        assert_eq!(Assets::free_balance(&XOR, &alice()).unwrap(), 1);
        assert_eq!(
            Assets::free_balance(&XOR, &bob()).unwrap(),
            Pallet::transfer_limit()
        );

        assert_ok!(Pallet::transfer(
            RuntimeOrigin::signed(alice()),
            VAL,
            bob(),
            Pallet::transfer_limit()
        ));
        assert_eq!(
            Assets::free_balance(&VAL, &account_id()).unwrap(),
            (Pallet::transfer_limit() * FixedWrapper::from(0.5)).into_balance()
        );
        assert_eq!(Assets::free_balance(&VAL, &alice()).unwrap(), 0);
        assert_eq!(
            Assets::free_balance(&VAL, &bob()).unwrap(),
            Pallet::transfer_limit()
        );
    });
}

#[test]
fn transfer_passes_after_limit_is_reset() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(Pallet::transfer(
            RuntimeOrigin::signed(alice()),
            XOR,
            bob(),
            Pallet::transfer_limit()
        ));
        System::set_block_number(14401);
        assert_ok!(Pallet::transfer(
            RuntimeOrigin::signed(alice()),
            XOR,
            bob(),
            (Pallet::transfer_limit() * FixedWrapper::from(0.5)).into_balance()
        ));
        assert_eq!(
            Assets::free_balance(&XOR, &account_id()).unwrap(),
            balance!(0)
        );
        assert_eq!(Assets::free_balance(&XOR, &alice()).unwrap(), 1);
        assert_eq!(
            Assets::free_balance(&XOR, &bob()).unwrap(),
            (Pallet::transfer_limit() * FixedWrapper::from(1.5)).into_balance()
        );
    });
}

#[test]
fn transfer_fails_with_asset_not_supported() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            Pallet::transfer(
                RuntimeOrigin::signed(alice()),
                NOT_SUPPORTED_ASSET_ID,
                bob(),
                Pallet::transfer_limit()
            ),
            crate::Error::<Runtime>::AssetNotSupported
        );
    });
}

#[test]
fn transfer_fails_with_amount_above_limit() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(Pallet::transfer(
            RuntimeOrigin::signed(alice()),
            XOR,
            bob(),
            Pallet::transfer_limit(),
        ));
        assert_noop!(
            Pallet::transfer(
                RuntimeOrigin::signed(alice()),
                XOR,
                bob(),
                (Pallet::transfer_limit() * FixedWrapper::from(2.0)).into_balance()
            ),
            crate::Error::<Runtime>::AmountAboveLimit
        );
    });
}

#[test]
fn transfer_fails_with_not_enough_reserves() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(Pallet::transfer(
            RuntimeOrigin::signed(alice()),
            XOR,
            bob(),
            Pallet::transfer_limit()
        ));
        assert_noop!(
            Pallet::transfer(
                RuntimeOrigin::signed(bob()),
                XOR,
                alice(),
                Pallet::transfer_limit()
            ),
            crate::Error::<Runtime>::NotEnoughReserves
        );
    });
}

#[test]
fn limit_increase_works() {
    ExtBuilder::build().execute_with(|| {
        // Set new limit
        let new_limit = (FixedWrapper::from(1.3) * DEFAULT_LIMIT).into_balance();
        assert_ok!(Pallet::update_limit(RuntimeOrigin::root(), new_limit));
        assert_eq!(Pallet::transfer_limit(), new_limit);

        // Try to transfer assets
        assert_ok!(Pallet::transfer(
            RuntimeOrigin::signed(alice()),
            XOR,
            bob(),
            new_limit
        ));
    })
}

#[test]
fn limit_decrease_works() {
    ExtBuilder::build().execute_with(|| {
        // Set new limit
        let new_limit = (FixedWrapper::from(0.3) * DEFAULT_LIMIT).into_balance();
        assert_ok!(Pallet::update_limit(RuntimeOrigin::root(), new_limit,));
        assert_eq!(Pallet::transfer_limit(), new_limit);

        // Try to transfer assets
        assert_noop!(
            Pallet::transfer(RuntimeOrigin::signed(alice()), XOR, bob(), DEFAULT_LIMIT),
            crate::Error::<Runtime>::AmountAboveLimit
        );
        assert_ok!(Pallet::transfer(
            RuntimeOrigin::signed(alice()),
            XOR,
            bob(),
            new_limit,
        ));
    })
}
