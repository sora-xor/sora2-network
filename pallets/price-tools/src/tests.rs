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

use std::convert::TryInto;

use crate::mock::*;
use crate::{Error, AVG_BLOCK_SPAN};
use common::prelude::Balance;
use common::{balance, DOT, ETH, PSWAP, VAL, XOR};
use frame_support::assert_noop;

fn to_avg<'a, I>(it: I, size: u32) -> Balance
where
    I: Iterator<Item = &'a Balance>,
{
    let calc_avg: u128 = it.fold(0u128, |a, b| a + b);
    let size: u128 = size.try_into().unwrap();
    calc_avg / size
}

#[test]
fn initial_setup_without_history() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        crate::EnabledTargets::<Runtime>::mutate(|set| {
            *set = [ETH, DAI, VAL, PSWAP].iter().cloned().collect()
        });
        let avg_calc = balance!(1 + AVG_BLOCK_SPAN) / 2;
        for i in 1..=AVG_BLOCK_SPAN {
            assert_noop!(
                PriceTools::get_average_price(&XOR.into(), &ETH.into()),
                Error::<Runtime>::InsufficientSpotPriceData
            );
            PriceTools::incoming_spot_price(&ETH, balance!(i)).unwrap();
        }
        assert_eq!(
            PriceTools::get_average_price(&XOR.into(), &ETH.into()).unwrap(),
            avg_calc
        );
        PriceTools::incoming_spot_price(&ETH, balance!(AVG_BLOCK_SPAN + 1)).unwrap();
        assert_eq!(
            PriceTools::get_average_price(&XOR.into(), &ETH.into()).unwrap(),
            avg_calc + avg_calc / 200 // 0.5% = 1/200
        );
    });
}

#[test]
fn average_price_same_values() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        crate::EnabledTargets::<Runtime>::mutate(|set| {
            *set = [ETH, DAI, VAL, PSWAP].iter().cloned().collect()
        });
        for _ in 1..=AVG_BLOCK_SPAN {
            assert_noop!(
                PriceTools::get_average_price(&XOR.into(), &ETH.into()),
                Error::<Runtime>::InsufficientSpotPriceData
            );
            PriceTools::incoming_spot_price(&ETH, balance!(10)).unwrap();
        }
        assert_eq!(
            PriceTools::get_average_price(&XOR.into(), &ETH.into()).unwrap(),
            balance!(10)
        );
        PriceTools::incoming_spot_price(&ETH, balance!(10)).unwrap();
        assert_eq!(
            PriceTools::get_average_price(&XOR.into(), &ETH.into()).unwrap(),
            balance!(10)
        );
    });
}

#[test]
fn average_price_smoothed_change_without_cap() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        crate::EnabledTargets::<Runtime>::mutate(|set| {
            *set = [ETH, DAI, VAL, PSWAP].iter().cloned().collect()
        });
        for _ in 1..=AVG_BLOCK_SPAN {
            PriceTools::incoming_spot_price(&ETH, balance!(1000)).unwrap();
        }
        assert_eq!(
            PriceTools::get_average_price(&XOR.into(), &ETH.into()).unwrap(),
            to_avg(PriceTools::spot_prices(&ETH).iter(), AVG_BLOCK_SPAN)
        );
        for &new_price in [999u32, 1000, 1003, 1006, 1009, 1015, 1018, 1021, 1024, 1030].iter() {
            PriceTools::incoming_spot_price(&ETH, balance!(new_price)).unwrap();
            assert_eq!(
                PriceTools::get_average_price(&XOR.into(), &ETH.into()).unwrap(),
                to_avg(PriceTools::spot_prices(&ETH).iter(), AVG_BLOCK_SPAN)
            );
        }
        for &new_price in [1033u32, 1024, 1030, 1039, 1003, 1039, 1000, 1030].iter() {
            PriceTools::incoming_spot_price(&ETH, balance!(new_price)).unwrap();
            assert_eq!(
                PriceTools::get_average_price(&XOR.into(), &ETH.into()).unwrap(),
                to_avg(PriceTools::spot_prices(&ETH).iter(), AVG_BLOCK_SPAN)
            );
        }
    });
}

#[test]
fn different_average_for_different_assets() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        crate::EnabledTargets::<Runtime>::mutate(|set| {
            *set = [ETH, DAI, VAL, PSWAP].iter().cloned().collect()
        });
        for _ in 1..=AVG_BLOCK_SPAN {
            PriceTools::incoming_spot_price(&ETH, balance!(0.5)).unwrap();
        }
        for _ in 1..=AVG_BLOCK_SPAN {
            PriceTools::incoming_spot_price(&DAI, balance!(700)).unwrap();
        }
        for _ in 1..=AVG_BLOCK_SPAN {
            PriceTools::incoming_spot_price(&VAL, balance!(2)).unwrap();
        }
        for _ in 1..=AVG_BLOCK_SPAN {
            PriceTools::incoming_spot_price(&PSWAP, balance!(1200)).unwrap();
        }
        for &new_price in [balance!(0.5), balance!(0.5001), balance!(0.5002)].iter() {
            assert_eq!(
                PriceTools::get_average_price(&XOR.into(), &ETH.into()).unwrap(),
                to_avg(PriceTools::spot_prices(&ETH).iter(), AVG_BLOCK_SPAN)
            );
            PriceTools::incoming_spot_price(&ETH, new_price).unwrap();
        }
        for &new_price in [balance!(700), balance!(700.5), balance!(700.3)].iter() {
            assert_eq!(
                PriceTools::get_average_price(&XOR.into(), &DAI.into()).unwrap(),
                to_avg(PriceTools::spot_prices(&DAI).iter(), AVG_BLOCK_SPAN)
            );
            PriceTools::incoming_spot_price(&DAI, new_price).unwrap();
        }
        for &new_price in [balance!(2), balance!(2.001), balance!(2.005)].iter() {
            assert_eq!(
                PriceTools::get_average_price(&XOR.into(), &VAL.into()).unwrap(),
                to_avg(PriceTools::spot_prices(&VAL).iter(), AVG_BLOCK_SPAN)
            );
            PriceTools::incoming_spot_price(&VAL, new_price).unwrap();
        }
        for &new_price in [balance!(1200), balance!(1201.1), balance!(1202.2)].iter() {
            assert_eq!(
                PriceTools::get_average_price(&XOR.into(), &PSWAP.into()).unwrap(),
                to_avg(PriceTools::spot_prices(&PSWAP).iter(), AVG_BLOCK_SPAN)
            );
            PriceTools::incoming_spot_price(&PSWAP, new_price).unwrap();
        }
    });
}

#[test]
fn all_exchange_paths_work() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        crate::EnabledTargets::<Runtime>::mutate(|set| {
            *set = [ETH, DAI, VAL, PSWAP].iter().cloned().collect()
        });
        for _ in 1..=AVG_BLOCK_SPAN {
            PriceTools::incoming_spot_price(&ETH, balance!(0.5)).unwrap();
        }
        for _ in 1..=AVG_BLOCK_SPAN {
            PriceTools::incoming_spot_price(&DAI, balance!(800)).unwrap();
        }
        // XOR(1)->ETH(0.5)
        assert_eq!(
            PriceTools::get_average_price(&XOR.into(), &ETH.into()).unwrap(),
            balance!(0.5)
        );
        // ETH(1)->XOR(2)
        assert_eq!(
            PriceTools::get_average_price(&ETH.into(), &XOR.into()).unwrap(),
            balance!(2)
        );
        // XOR(1)->DAI(800)
        assert_eq!(
            PriceTools::get_average_price(&XOR.into(), &DAI.into()).unwrap(),
            balance!(800)
        );
        // DAI(1)->XOR(0.00125)
        assert_eq!(
            PriceTools::get_average_price(&DAI.into(), &XOR.into()).unwrap(),
            balance!(0.00125)
        );
        // ETH(1)->XOR(2)->DAI(1600)
        assert_eq!(
            PriceTools::get_average_price(&ETH.into(), &DAI.into()).unwrap(),
            balance!(1600)
        );
        // DAI(1)->XOR(0.00125)->ETH(0.000625)
        assert_eq!(
            PriceTools::get_average_price(&DAI.into(), &ETH.into()).unwrap(),
            balance!(0.000625)
        );
    });
}

#[test]
fn price_quote_continuous_failure() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        crate::EnabledTargets::<Runtime>::mutate(|set| {
            *set = [ETH, DAI, VAL, PSWAP].iter().cloned().collect()
        });
        // initialization period
        for _ in 1..=AVG_BLOCK_SPAN {
            assert_noop!(
                PriceTools::get_average_price(&XOR.into(), &ETH.into()),
                Error::<Runtime>::InsufficientSpotPriceData
            );
            PriceTools::incoming_spot_price(&ETH, balance!(10)).unwrap();
        }
        assert_eq!(
            PriceTools::get_average_price(&XOR.into(), &ETH.into()).unwrap(),
            balance!(10)
        );
        // failure period
        for _ in 1..AVG_BLOCK_SPAN {
            PriceTools::average_prices_calculation_routine();
        }
        assert_eq!(
            PriceTools::get_average_price(&XOR.into(), &ETH.into()).unwrap(),
            balance!(10)
        );
        PriceTools::average_prices_calculation_routine();

        // recovery period
        for _ in 1..=AVG_BLOCK_SPAN {
            assert_noop!(
                PriceTools::get_average_price(&XOR.into(), &ETH.into()),
                Error::<Runtime>::InsufficientSpotPriceData
            );
            PriceTools::incoming_spot_price(&ETH, balance!(20)).unwrap();
        }
        assert_eq!(
            PriceTools::get_average_price(&XOR.into(), &ETH.into()).unwrap(),
            balance!(20)
        );
    });
}

#[test]
fn failure_for_unsupported_assets() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        crate::EnabledTargets::<Runtime>::mutate(|set| {
            *set = [ETH, DAI, VAL, PSWAP].iter().cloned().collect()
        });
        for _ in 1..=AVG_BLOCK_SPAN {
            assert_noop!(
                PriceTools::get_average_price(&XOR.into(), &ETH.into()),
                Error::<Runtime>::InsufficientSpotPriceData
            );
            PriceTools::incoming_spot_price(&ETH, balance!(10)).unwrap();
        }
        assert_eq!(
            PriceTools::get_average_price(&XOR.into(), &ETH.into()).unwrap(),
            balance!(10)
        );
        assert_noop!(
            PriceTools::get_average_price(&XOR.into(), &DOT.into()),
            Error::<Runtime>::UnsupportedQuotePath
        );
        assert_noop!(
            PriceTools::get_average_price(&DOT.into(), &XOR.into()),
            Error::<Runtime>::UnsupportedQuotePath
        );
        assert_noop!(
            PriceTools::get_average_price(&DOT.into(), &ETH.into()),
            Error::<Runtime>::UnsupportedQuotePath
        );
        assert_noop!(
            PriceTools::get_average_price(&ETH.into(), &DOT.into()),
            Error::<Runtime>::UnsupportedQuotePath
        );
    });
}
