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

use frame_support::{assert_err, error::BadOrigin};

use crate::{mock::*, Error, Rate};

#[test]
fn add_and_remove_relayers_should_work() {
    new_test_ext().execute_with(|| {
        assert!(Band::trusted_relayers().is_none());

        let relayers = vec![1, 2, 3, 4, 5];
        Band::add_relayers(Origin::root(), relayers.clone()).expect("Failed to add relayers");

        let trusted_relayers = Band::trusted_relayers().expect("Expected initialized relayers");
        for relayer in &relayers {
            assert!(trusted_relayers.contains(relayer));
        }

        let to_remove = vec![3, 1];
        Band::remove_relayers(Origin::root(), to_remove.clone())
            .expect("Failed to remove relayers");
        for relayer in &to_remove {
            assert!(!Band::trusted_relayers().unwrap().contains(relayer));
        }
    });
}

#[test]
fn add_and_remove_relayers_should_forbid_non_root_call() {
    new_test_ext().execute_with(|| {
        let relayers = vec![1, 2, 3, 4, 5];

        assert_err!(
            Band::add_relayers(Origin::signed(10), relayers.clone()),
            BadOrigin
        );

        assert_err!(
            Band::remove_relayers(Origin::signed(11), relayers.clone()),
            BadOrigin
        );

        assert!(Band::trusted_relayers().is_none());
    });
}

#[test]
fn relay_should_work() {
    new_test_ext().execute_with(|| {
        let symbols = vec!["USD".to_owned(), "RUB".to_owned(), "YEN".to_owned()];
        let rates = vec![1, 2, 3];
        let relayer = 1;
        let initial_resolve_time = 100;

        for symbol in &symbols {
            assert_eq!(Band::rates(symbol), None);
        }

        Band::add_relayers(Origin::root(), vec![relayer]).expect("Failed to add relayers");
        Band::relay(
            Origin::signed(relayer),
            symbols.clone(),
            rates.clone(),
            initial_resolve_time,
            0,
        )
        .expect("Failed to relay rates");

        for (symbol, rate) in symbols.iter().zip(rates) {
            assert_eq!(
                Band::rates(symbol),
                Some(Rate {
                    value: rate,
                    last_updated: initial_resolve_time
                })
            );
        }
    });
}

#[test]
fn relay_should_not_update_if_time_is_lower_than_last_stored() {
    new_test_ext().execute_with(|| {
        let relayer = 1;
        let initial_resolve_time = 100;

        Band::add_relayers(Origin::root(), vec![relayer]).expect("Failed to add relayers");
        Band::relay(
            Origin::signed(relayer),
            vec!["USD".to_owned(), "RUB".to_owned(), "YEN".to_owned()],
            vec![1, 2, 3],
            initial_resolve_time,
            0,
        )
        .expect("Failed to relay rates");

        Band::relay(
            Origin::signed(relayer),
            vec!["RUB".to_owned()],
            vec![4],
            initial_resolve_time - 1,
            0,
        )
        .expect("Failed to relay rates");

        assert_eq!(
            Band::rates("RUB"),
            Some(Rate {
                value: 2,
                last_updated: initial_resolve_time,
            })
        );
    });
}

#[test]
fn force_relay_should_rewrite_rates_without_time_check() {
    new_test_ext().execute_with(|| {
        let relayer = 1;
        let initial_resolve_time = 100;

        Band::add_relayers(Origin::root(), vec![relayer]).expect("Failed to add relayers");
        Band::relay(
            Origin::signed(relayer),
            vec!["USD".to_owned(), "RUB".to_owned(), "YEN".to_owned()],
            vec![1, 2, 3],
            initial_resolve_time,
            0,
        )
        .expect("Failed to relay rates");

        let new_rub_rate = 4;
        let new_resolve_time = initial_resolve_time - 1;
        Band::force_relay(
            Origin::signed(relayer),
            vec!["RUB".to_owned()],
            vec![new_rub_rate],
            new_resolve_time,
            0,
        )
        .expect("Failed to force relay rates");

        assert_eq!(
            Band::rates("RUB"),
            Some(Rate {
                value: new_rub_rate,
                last_updated: new_resolve_time
            })
        );
    });
}

#[test]
fn relay_should_check_for_trusted_relayer() {
    new_test_ext().execute_with(|| {
        let relayer = 1;
        let initial_resolve_time = 100;

        Band::add_relayers(Origin::root(), vec![relayer]).expect("Failed to add relayers");
        assert_err!(
            Band::relay(
                Origin::signed(relayer + 1),
                vec!["USD".to_owned(), "RUB".to_owned(), "YEN".to_owned()],
                vec![1, 2, 3],
                initial_resolve_time,
                0,
            ),
            Error::<Runtime>::NotATrustedRelayer
        );
    });
}

#[test]
fn force_relay_should_check_for_trusted_relayer() {
    new_test_ext().execute_with(|| {
        let relayer = 1;
        let initial_resolve_time = 100;

        Band::add_relayers(Origin::root(), vec![relayer]).expect("Failed to add relayers");
        assert_err!(
            Band::force_relay(
                Origin::signed(relayer + 1),
                vec!["USD".to_owned(), "RUB".to_owned(), "YEN".to_owned()],
                vec![1, 2, 3],
                initial_resolve_time,
                0,
            ),
            Error::<Runtime>::NotATrustedRelayer
        );
    });
}

#[test]
fn relay_should_check_length_of_symbols_and_rates() {
    new_test_ext().execute_with(|| {
        let relayer = 1;
        let initial_resolve_time = 100;

        Band::add_relayers(Origin::root(), vec![relayer]).expect("Failed to add relayers");
        assert_err!(
            Band::relay(
                Origin::signed(relayer),
                vec!["USD".to_owned(), "RUB".to_owned(), "YEN".to_owned()],
                vec![1, 2],
                initial_resolve_time,
                0,
            ),
            Error::<Runtime>::DivergedLengthsOfSymbolsAndRates
        );
    });
}

#[test]
fn force_relay_should_check_length_of_symbols_and_rates() {
    new_test_ext().execute_with(|| {
        let relayer = 1;
        let initial_resolve_time = 100;

        Band::add_relayers(Origin::root(), vec![relayer]).expect("Failed to add relayers");
        assert_err!(
            Band::force_relay(
                Origin::signed(relayer),
                vec!["USD".to_owned(), "RUB".to_owned(), "YEN".to_owned()],
                vec![1, 2],
                initial_resolve_time,
                0,
            ),
            Error::<Runtime>::DivergedLengthsOfSymbolsAndRates
        );
    });
}
