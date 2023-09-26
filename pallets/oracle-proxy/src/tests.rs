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
use common::{fixed, DataFeed};
use frame_support::{assert_err, error::BadOrigin};
use sp_core::TryCollect;

use crate::{mock::*, Oracle, Rate};

fn relay_symbols() {
    let symbols = vec!["USD".to_owned(), "RUB".to_owned(), "YEN".to_owned()];
    let rates = vec![1, 2, 3];
    let relayer = 1;
    let initial_resolve_time = 0;

    Band::add_relayers(RuntimeOrigin::root(), vec![relayer]).expect("Failed to add relayers");
    Band::relay(
        RuntimeOrigin::signed(relayer),
        symbols
            .into_iter()
            .zip(rates.into_iter())
            .try_collect()
            .unwrap(),
        initial_resolve_time,
        0,
    )
    .expect("Failed to relay rates");
}

#[test]
fn enable_and_disable_oracles_should_work() {
    new_test_ext().execute_with(|| {
        assert!(OracleProxy::enabled_oracles().is_empty());

        let oracle = Oracle::BandChainFeed;
        OracleProxy::enable_oracle(RuntimeOrigin::root(), oracle.clone())
            .expect("Failed to enable oracle");

        let enabled_oracles = OracleProxy::enabled_oracles();
        assert!(enabled_oracles.contains(&oracle));

        OracleProxy::disable_oracle(RuntimeOrigin::root(), oracle.clone())
            .expect("Failed to disable oracle");

        assert!(!OracleProxy::enabled_oracles().contains(&oracle));
    });
}

#[test]
fn enable_and_disable_oracles_should_forbid_non_root_call() {
    new_test_ext().execute_with(|| {
        let oracle = Oracle::BandChainFeed;
        assert_err!(
            OracleProxy::enable_oracle(RuntimeOrigin::signed(1), oracle.clone()),
            BadOrigin
        );

        assert!(OracleProxy::enabled_oracles().is_empty());

        assert_err!(
            OracleProxy::disable_oracle(RuntimeOrigin::signed(2), oracle.clone()),
            BadOrigin
        );
    });
}

#[test]
fn quote_and_list_enabled_symbols_should_work() {
    new_test_ext().execute_with(|| {
        relay_symbols();

        let oracle = Oracle::BandChainFeed;
        OracleProxy::enable_oracle(RuntimeOrigin::root(), oracle.clone())
            .expect("Failed to enable oracle");

        let symbols = vec!["USD".to_owned(), "RUB".to_owned(), "YEN".to_owned()];
        let rates = vec![1, 2, 3];
        let resolve_time = 0;

        symbols
            .iter()
            .zip(rates.iter())
            .for_each(|(symbol, value)| {
                let rate = Rate {
                    value: Band::raw_rate_into_balance(value.clone())
                        .expect("failed to convert rate into Balance"),
                    last_updated: resolve_time,
                    dynamic_fee: fixed!(0),
                };
                assert_eq!(
                    <OracleProxy as DataFeed<String, Rate, u64>>::quote(symbol),
                    Ok(Some(rate))
                );
            });

        let enabled_symbols: HashSet<(String, u64)> =
            symbols.into_iter().map(|sym| (sym, resolve_time)).collect();

        let enabled_symbols_res: HashSet<(String, u64)> =
            <OracleProxy as DataFeed<String, Rate, u64>>::list_enabled_symbols()
                .expect("Failed to resolve symbols")
                .into_iter()
                .collect();

        assert_eq!(enabled_symbols_res, enabled_symbols)
    });
}

#[test]
fn quote_and_list_enabled_symbols_should_not_work_with_disabled_oracle() {
    new_test_ext().execute_with(|| {
        relay_symbols();

        assert_eq!(
            <OracleProxy as DataFeed<String, Rate, u64>>::quote(&"USD".to_string()),
            Ok(None)
        );

        assert_eq!(
            <OracleProxy as DataFeed<String, Rate, u64>>::list_enabled_symbols(),
            Ok(vec![])
        )
    });
}
