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

use codec::alloc::collections::HashSet;
use common::{fixed, DataFeed, Rate};
use common::{prelude::FixedWrapper, Balance, Fixed};
use frame_support::traits::Hooks;
use frame_support::{assert_noop, error::BadOrigin};
use frame_system::pallet_prelude::BlockNumberFor;
use sp_core::TryCollect;
use sp_std::collections::btree_set::BTreeSet;

use crate::{mock::*, BandRate, Error, FeeCalculationParameters, SymbolCheckBlock};

pub fn band_rate_into_balance(rate: u64) -> Balance {
    let fixed = Fixed::from_bits(rate as i128 * super::RATE_MULTIPLIER);
    let fixed_wrapper = FixedWrapper::from(fixed);
    fixed_wrapper
        .try_into_balance()
        .expect("Failed to convert fixed wrapper to balance")
}

#[test]
fn add_and_remove_relayers_should_work() {
    new_test_ext().execute_with(|| {
        assert!(Band::trusted_relayers().is_none());

        let relayers = vec![1, 2, 3, 4, 5];
        Band::add_relayers(RuntimeOrigin::root(), relayers.clone())
            .expect("Failed to add relayers");

        let trusted_relayers = Band::trusted_relayers().expect("Expected initialized relayers");
        for relayer in &relayers {
            assert!(trusted_relayers.contains(relayer));
        }

        let to_remove = vec![3, 1];
        Band::remove_relayers(RuntimeOrigin::root(), to_remove.clone())
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

        assert_noop!(
            Band::add_relayers(RuntimeOrigin::signed(10), relayers.clone()),
            BadOrigin
        );

        assert_noop!(
            Band::remove_relayers(RuntimeOrigin::signed(11), relayers),
            BadOrigin
        );

        assert!(Band::trusted_relayers().is_none());
    });
}

#[test]
fn add_relayers_should_check_if_relayer_was_already_added() {
    new_test_ext().execute_with(|| {
        let relayers = vec![1, 2, 3, 4, 5];
        Band::add_relayers(RuntimeOrigin::root(), relayers).expect("Failed to add relayers");

        assert_noop!(
            Band::add_relayers(RuntimeOrigin::root(), vec![1]),
            Error::<Runtime>::AlreadyATrustedRelayer
        );
    });
}

#[test]
fn remove_relayers_should_check_if_no_such_relayer_exists() {
    new_test_ext().execute_with(|| {
        let relayers = vec![1, 2, 3, 4, 5];
        Band::add_relayers(RuntimeOrigin::root(), relayers).expect("Failed to add relayers");

        assert_noop!(
            Band::remove_relayers(RuntimeOrigin::root(), vec![6]),
            Error::<Runtime>::NoSuchRelayer,
        );
    });
}

#[test]
fn add_relayers_should_ignore_duplicates() {
    new_test_ext().execute_with(|| {
        let relayers = vec![1, 2, 3, 4, 5, 3, 5, 4];
        Band::add_relayers(RuntimeOrigin::root(), relayers).expect("Failed to add relayers");

        assert_eq!(
            Band::trusted_relayers().expect("Expected initialized relayers"),
            BTreeSet::from([1, 2, 3, 4, 5]),
        );
    });
}

#[test]
fn remove_relayers_should_ignore_duplicates() {
    new_test_ext().execute_with(|| {
        let relayers = vec![1, 2, 3, 4, 5];
        Band::add_relayers(RuntimeOrigin::root(), relayers).expect("Failed to add relayers");

        Band::remove_relayers(RuntimeOrigin::root(), vec![1, 2, 3, 2, 1, 1, 3])
            .expect("Failed to remove relayers");

        assert_eq!(
            Band::trusted_relayers().expect("Expected initialized relayers"),
            BTreeSet::from([4, 5]),
        );
    });
}

#[test]
fn relay_should_work() {
    new_test_ext().execute_with(|| {
        let rates = vec![
            ("USD".to_owned(), 1),
            ("RUB".to_owned(), 2),
            ("YEN".to_owned(), 3),
        ];
        let relayer = 1;
        let initial_resolve_time = 100;
        let request_id = 0;

        for symbol in rates.iter().map(|(s, _r)| s) {
            assert_eq!(Band::rates(symbol), None);
        }

        Band::add_relayers(RuntimeOrigin::root(), vec![relayer]).expect("Failed to add relayers");
        Band::relay(
            RuntimeOrigin::signed(relayer),
            rates.clone().try_into().unwrap(),
            initial_resolve_time,
            request_id,
        )
        .expect("Failed to relay rates");

        for (symbol, rate) in rates {
            assert_eq!(
                Band::rates(symbol),
                Some(BandRate {
                    value: band_rate_into_balance(rate),
                    last_updated: initial_resolve_time,
                    request_id,
                    dynamic_fee: fixed!(0),
                    last_updated_block: 0u64,
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
        let request_id = 0;

        Band::add_relayers(RuntimeOrigin::root(), vec![relayer]).expect("Failed to add relayers");
        Band::relay(
            RuntimeOrigin::signed(relayer),
            vec![
                ("USD".to_owned(), 1),
                ("RUB".to_owned(), 2),
                ("YEN".to_owned(), 3),
            ]
            .try_into()
            .unwrap(),
            initial_resolve_time,
            request_id,
        )
        .expect("Failed to relay rates");

        let new_request_id = 1;
        Band::relay(
            RuntimeOrigin::signed(relayer),
            vec![("RUB".to_owned(), 4)].try_into().unwrap(),
            initial_resolve_time - 1,
            new_request_id,
        )
        .expect("Failed to relay rates");

        assert_eq!(
            Band::rates("RUB"),
            Some(BandRate {
                value: band_rate_into_balance(2),
                last_updated: initial_resolve_time,
                request_id,
                dynamic_fee: fixed!(0),
                last_updated_block: 0u64,
            })
        );
    });
}

#[test]
fn force_relay_should_rewrite_rates_without_time_check() {
    new_test_ext().execute_with(|| {
        let relayer = 1;
        let initial_resolve_time = 100;
        let request_id = 0;

        Band::add_relayers(RuntimeOrigin::root(), vec![relayer]).expect("Failed to add relayers");
        Band::relay(
            RuntimeOrigin::signed(relayer),
            vec![
                ("USD".to_owned(), 1),
                ("RUB".to_owned(), 2),
                ("YEN".to_owned(), 3),
            ]
            .try_into()
            .unwrap(),
            initial_resolve_time,
            request_id,
        )
        .expect("Failed to relay rates");

        let new_rub_rate = 4;
        let new_resolve_time = initial_resolve_time - 1;
        let new_request_id = 1;
        Band::force_relay(
            RuntimeOrigin::signed(relayer),
            vec![("RUB".to_owned(), new_rub_rate)].try_into().unwrap(),
            new_resolve_time,
            new_request_id,
        )
        .expect("Failed to force relay rates");

        assert_eq!(
            Band::rates("RUB"),
            Some(BandRate {
                value: band_rate_into_balance(new_rub_rate),
                last_updated: new_resolve_time,
                request_id: new_request_id,
                dynamic_fee: fixed!(0),
                last_updated_block: 0u64,
            })
        );
    });
}

#[test]
fn relay_should_check_for_trusted_relayer() {
    new_test_ext().execute_with(|| {
        let relayer = 1;
        let initial_resolve_time = 100;

        Band::add_relayers(RuntimeOrigin::root(), vec![relayer]).expect("Failed to add relayers");
        assert_noop!(
            Band::relay(
                RuntimeOrigin::signed(relayer + 1),
                vec![
                    ("USD".to_owned(), 1),
                    ("RUB".to_owned(), 2),
                    ("YEN".to_owned(), 3),
                ]
                .try_into()
                .unwrap(),
                initial_resolve_time,
                0,
            ),
            Error::<Runtime>::UnauthorizedRelayer
        );
    });
}

#[test]
fn force_relay_should_check_for_trusted_relayer() {
    new_test_ext().execute_with(|| {
        let relayer = 1;
        let initial_resolve_time = 100;

        Band::add_relayers(RuntimeOrigin::root(), vec![relayer]).expect("Failed to add relayers");
        assert_noop!(
            Band::force_relay(
                RuntimeOrigin::signed(relayer + 1),
                vec![
                    ("USD".to_owned(), 1),
                    ("RUB".to_owned(), 2),
                    ("YEN".to_owned(), 3),
                ]
                .try_into()
                .unwrap(),
                initial_resolve_time,
                0,
            ),
            Error::<Runtime>::UnauthorizedRelayer
        );
    });
}

#[test]
fn relay_should_store_last_duplicated_rate() {
    new_test_ext().execute_with(|| {
        let relayer = 1;
        let initial_resolve_time = 100;

        Band::add_relayers(RuntimeOrigin::root(), vec![relayer]).expect("Failed to add relayers");
        Band::relay(
            RuntimeOrigin::signed(relayer),
            vec![
                ("USD".to_owned(), 1),
                ("RUB".to_owned(), 2),
                ("YEN".to_owned(), 3),
                ("USD".to_owned(), 4),
            ]
            .try_into()
            .unwrap(),
            initial_resolve_time,
            0,
        )
        .expect("Failed to relay rates");

        assert_eq!(
            Band::rates("USD"),
            Some(BandRate {
                value: band_rate_into_balance(4),
                last_updated: initial_resolve_time,
                request_id: 0,
                dynamic_fee: fixed!(1),
                last_updated_block: 0u64,
            })
        );
    });
}

#[test]
fn force_relay_should_store_last_duplicated_rate() {
    new_test_ext().execute_with(|| {
        let relayer = 1;
        let initial_resolve_time = 100;

        Band::add_relayers(RuntimeOrigin::root(), vec![relayer]).expect("Failed to add relayers");
        Band::force_relay(
            RuntimeOrigin::signed(relayer),
            vec![
                ("USD".to_owned(), 1),
                ("RUB".to_owned(), 2),
                ("YEN".to_owned(), 3),
                ("USD".to_owned(), 4),
            ]
            .try_into()
            .unwrap(),
            initial_resolve_time,
            0,
        )
        .expect("Failed to relay rates");

        assert_eq!(
            Band::rates("USD"),
            Some(BandRate {
                value: band_rate_into_balance(4),
                last_updated: initial_resolve_time,
                request_id: 0,
                dynamic_fee: fixed!(0),
                last_updated_block: 0u64,
            })
        );
    });
}

#[test]
fn quote_and_list_enabled_symbols_should_work() {
    new_test_ext().execute_with(|| {
        let symbols = vec!["USD".to_owned(), "RUB".to_owned(), "YEN".to_owned()];
        let rates = vec![1, 2, 3];
        let relayer = 1;
        let initial_resolve_time = 0;

        for symbol in &symbols {
            assert_eq!(Band::rates(symbol), None);
        }

        Band::add_relayers(RuntimeOrigin::root(), vec![relayer]).expect("Failed to add relayers");
        Band::relay(
            RuntimeOrigin::signed(relayer),
            symbols
                .iter()
                .cloned()
                .zip(rates.iter().cloned())
                .try_collect()
                .unwrap(),
            initial_resolve_time,
            0,
        )
        .expect("Failed to relay rates");

        for (symbol, rate) in symbols.iter().zip(rates) {
            assert_eq!(
                Band::rates(symbol),
                Some(BandRate {
                    value: band_rate_into_balance(rate),
                    last_updated: initial_resolve_time,
                    request_id: 0,
                    dynamic_fee: fixed!(0),
                    last_updated_block: 0u64,
                })
            );
        }

        let usd_rate = BandRate {
            value: band_rate_into_balance(1),
            last_updated: initial_resolve_time,
            request_id: 0,
            dynamic_fee: fixed!(0),
            last_updated_block: 0u64,
        };

        assert_eq!(
            <Band as DataFeed<String, Rate, u64>>::quote(&"USD".to_owned()),
            Ok(Some(usd_rate.into()))
        );

        let enabled_symbols: HashSet<(String, u64)> = symbols
            .iter()
            .cloned()
            .map(|sym| (sym, initial_resolve_time))
            .collect();

        let enabled_symbols_res: HashSet<(String, u64)> =
            <Band as DataFeed<String, Rate, u64>>::list_enabled_symbols()
                .expect("Failed to resolve symbols")
                .iter()
                .cloned()
                .collect();

        assert_eq!(enabled_symbols_res, enabled_symbols)
    });
}

#[test]
fn quote_invalid_rate_should_fail() {
    new_test_ext().execute_with(|| {
        let relayer = 1;

        assert_eq!(Band::rates("USD".to_owned()), None);
        assert_eq!(Band::rates("RUB".to_owned()), None);

        Band::add_relayers(RuntimeOrigin::root(), vec![relayer]).expect("Failed to add relayers");
        Band::relay(
            RuntimeOrigin::signed(relayer),
            vec![("USD".to_owned(), 1)].try_into().unwrap(),
            0,
            0,
        )
        .expect("Failed to relay rates");

        Timestamp::set_timestamp(GetBandRateStalePeriod::get() + 10);

        assert_eq!(
            <Band as DataFeed<String, Rate, u64>>::quote(&"USD".to_owned()),
            Err(Error::<Runtime>::RateExpired.into())
        );

        Band::relay(
            RuntimeOrigin::signed(relayer),
            vec![("RUB".to_owned(), 1)].try_into().unwrap(),
            60 * 5 + 1,
            0,
        )
        .expect("Failed to relay rates");

        assert_eq!(
            <Band as DataFeed<String, Rate, u64>>::quote(&"RUB".to_owned()),
            Err(Error::<Runtime>::RateHasInvalidTimestamp.into())
        );
    })
}

#[test]
fn check_block_symbol_should_work() {
    new_test_ext().execute_with(|| {
        let relayer = 1;

        assert_eq!(Band::rates("USD".to_owned()), None);

        System::set_block_number(1);

        Band::add_relayers(RuntimeOrigin::root(), vec![relayer]).expect("Failed to add relayers");
        Band::relay(
            RuntimeOrigin::signed(relayer),
            vec![("USD".to_owned(), 1)].try_into().unwrap(),
            0,
            0,
        )
        .expect("Failed to relay rates");

        assert!(SymbolCheckBlock::<Runtime>::get(
            1 + GetBandRateStaleBlockPeriod::get(),
            "USD".to_owned()
        ));

        <Band as Hooks<BlockNumberFor<Runtime>>>::on_initialize(601);

        assert!(!SymbolCheckBlock::<Runtime>::get(601u64, "USD".to_owned()))
    })
}

#[test]
fn set_dynamic_fee_parameters_should_work() {
    new_test_ext().execute_with(|| {
        let parameters = FeeCalculationParameters::new(fixed!(0.1), fixed!(1), fixed!(0.05));

        Band::set_dynamic_fee_parameters(RuntimeOrigin::root(), parameters.clone())
            .expect("Expected to set the dynamic fee calculation parameters");
        let uploaded_parameters = Band::dynamic_fee_parameters();

        assert_eq!(parameters, uploaded_parameters,);
    })
}

#[test]
fn set_invalid_dynamic_fee_parameters_should_fail() {
    new_test_ext().execute_with(|| {
        let parameters = FeeCalculationParameters::new(fixed!(-0.1), fixed!(0), fixed!(0));
        assert_eq!(
            Band::set_dynamic_fee_parameters(RuntimeOrigin::root(), parameters),
            Err(Error::<Runtime>::InvalidDynamicFeeParameters.into())
        );

        let parameters = FeeCalculationParameters::new(fixed!(0), fixed!(-1), fixed!(0));
        assert_eq!(
            Band::set_dynamic_fee_parameters(RuntimeOrigin::root(), parameters),
            Err(Error::<Runtime>::InvalidDynamicFeeParameters.into())
        );

        let parameters = FeeCalculationParameters::new(fixed!(0), fixed!(0), fixed!(-1));
        assert_eq!(
            Band::set_dynamic_fee_parameters(RuntimeOrigin::root(), parameters),
            Err(Error::<Runtime>::InvalidDynamicFeeParameters.into())
        );

        let parameters = FeeCalculationParameters::new(fixed!(1), fixed!(0), fixed!(0));
        assert_eq!(
            Band::set_dynamic_fee_parameters(RuntimeOrigin::root(), parameters),
            Err(Error::<Runtime>::InvalidDynamicFeeParameters.into())
        );
    })
}

#[test]
fn should_calculate_dynamic_fee() {
    new_test_ext().execute_with(|| {
        let parameters = FeeCalculationParameters::new(fixed!(0.1), fixed!(0.01), fixed!(0.05));

        Band::set_dynamic_fee_parameters(RuntimeOrigin::root(), parameters)
            .expect("Expected to set the dynamic fee calculation parameters");

        let relayer = 1;
        let symbol_name = "USD".to_owned();
        Timestamp::set_timestamp(10_000);

        Band::add_relayers(RuntimeOrigin::root(), vec![relayer]).expect("Failed to add relayers");
        Band::relay(
            RuntimeOrigin::signed(relayer),
            vec![(symbol_name.clone(), 1_000_000_000)]
                .try_into()
                .unwrap(),
            0,
            0,
        )
        .expect("Failed to relay rates");

        let rate_a = Band::quote(&symbol_name)
            .expect("Expected to get the Ok result from quote")
            .expect("Expected to get the rate of symbol");

        Band::relay(
            RuntimeOrigin::signed(relayer),
            vec![("USD".to_owned(), 1_500_000_000)].try_into().unwrap(),
            1,
            0,
        )
        .expect("Failed to relay rates");

        let rate_b = Band::quote(&symbol_name)
            .expect("Expected to get the Ok result from quote")
            .expect("Expected to get the rate of symbol");

        Band::relay(
            RuntimeOrigin::signed(relayer),
            vec![("USD".to_owned(), 1_500_000_000)].try_into().unwrap(),
            2,
            0,
        )
        .expect("Failed to relay rates");

        let rate_c = Band::quote(&symbol_name)
            .expect("Expected to get the Ok result from quote")
            .expect("Expected to get the rate of symbol");

        Band::relay(
            RuntimeOrigin::signed(relayer),
            vec![("USD".to_owned(), 1_000_000_000_000_000)]
                .try_into()
                .unwrap(),
            2,
            0,
        )
        .expect("Failed to relay rates");

        let rate_d = Band::quote(&symbol_name)
            .expect("Expected to get the Ok result from quote")
            .expect("Expected to get the rate of symbol");

        assert_eq!(rate_a.dynamic_fee, fixed!(0),);
        assert_eq!(rate_b.dynamic_fee, fixed!(0.39));
        assert_eq!(rate_c.dynamic_fee, fixed!(0.039));
        assert_eq!(rate_d.dynamic_fee, fixed!(1));
    })
}
