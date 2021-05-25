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
use crate::{Error, Module, AVG_BLOCK_SPAN};
use common::{
    balance, EnsureTradingPairExists, LiquiditySourceType, TradingPair, ETH, KSM, PSWAP, VAL, XOR,
};
use frame_support::{assert_err, assert_noop, assert_ok};

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
            avg_calc + avg_calc / 100
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
        for i in 1..=AVG_BLOCK_SPAN {
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
fn average_price_multiple_periods() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        crate::EnabledTargets::<Runtime>::mutate(|set| {
            *set = [ETH, DAI, VAL, PSWAP].iter().cloned().collect()
        });
        for i in 1..=AVG_BLOCK_SPAN {
            assert_noop!(
                PriceTools::get_average_price(&XOR.into(), &ETH.into()),
                Error::<Runtime>::InsufficientSpotPriceData
            );
            PriceTools::incoming_spot_price(&ETH, balance!(1000)).unwrap();
        }
        assert_eq!(
            PriceTools::get_average_price(&XOR.into(), &ETH.into()).unwrap(),
            balance!(1000)
        );
        for i in 1..=AVG_BLOCK_SPAN {
            PriceTools::incoming_spot_price(&ETH, balance!(1001)).unwrap();
        }
        assert_eq!(
            PriceTools::get_average_price(&XOR.into(), &ETH.into()).unwrap(),
            balance!(1001)
        );
        for i in 1..=AVG_BLOCK_SPAN {
            PriceTools::incoming_spot_price(&ETH, balance!(1002)).unwrap();
        }
        assert_eq!(
            PriceTools::get_average_price(&XOR.into(), &ETH.into()).unwrap(),
            balance!(1002)
        );
    });
}

#[test]
fn average_price_smoothed_change() {}

#[test]
fn price_quote_anecdotal_failure() {}

#[test]
fn price_quote_continuous_failure() {}
