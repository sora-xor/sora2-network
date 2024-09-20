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
use common::{
    balance, fixed_wrapper, OnPoolReservesChanged, PriceToolsProvider, PriceVariant, DOT, ETH,
    PSWAP, VAL, XOR,
};
use frame_support::assert_noop;
use sp_runtime::DispatchError;

fn to_avg<'a, I>(it: I, size: usize) -> Balance
where
    I: Iterator<Item = &'a Balance>,
{
    let calc_avg: u128 = it.fold(0u128, |a, b| a + b);
    let size: u128 = size.try_into().unwrap();
    calc_avg / size
}

pub fn get_average_price(
    input_asset: &AssetId,
    output_asset: &AssetId,
    variant: PriceVariant,
) -> Result<Balance, DispatchError> {
    PriceTools::get_average_price::<crate::PriceInfos<Runtime>>(input_asset, output_asset, variant)
}

pub fn get_fast_average_price(
    input_asset: &AssetId,
    output_asset: &AssetId,
    variant: PriceVariant,
) -> Result<Balance, DispatchError> {
    PriceTools::get_average_price::<crate::FastPriceInfos<Runtime>>(
        input_asset,
        output_asset,
        variant,
    )
}

pub fn incoming_spot_price(asset_id: &AssetId, price: Balance) {
    crate::PriceInfos::<Runtime>::mutate(asset_id, |opt_val| {
        let val = opt_val.as_mut().unwrap();
        val.buy
            .incoming_spot_price(price, PriceVariant::Buy, &crate::DEFAULT_PARAMETERS)
            .unwrap();
        val.sell
            .incoming_spot_price(price, PriceVariant::Sell, &crate::DEFAULT_PARAMETERS)
            .unwrap();
    });
    crate::FastPriceInfos::<Runtime>::mutate(asset_id, |opt_val| {
        let val = opt_val.as_mut().unwrap();
        val.buy
            .incoming_spot_price(price, PriceVariant::Buy, &crate::FAST_PARAMETERS)
            .unwrap();
        val.sell
            .incoming_spot_price(price, PriceVariant::Sell, &crate::FAST_PARAMETERS)
            .unwrap();
    })
}

#[test]
fn initial_setup_without_history() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        for asset_id in [ETH, DAI, VAL, PSWAP].iter() {
            PriceTools::register_asset(asset_id).unwrap();
        }
        let avg_calc = balance!(1 + AVG_BLOCK_SPAN) / 2;
        for i in 1..=AVG_BLOCK_SPAN {
            assert_noop!(
                get_average_price(&XOR.into(), &ETH.into(), PriceVariant::Buy,),
                Error::<Runtime>::InsufficientSpotPriceData
            );
            incoming_spot_price(&ETH, balance!(i));
        }
        assert_eq!(
            get_average_price(&XOR.into(), &ETH.into(), PriceVariant::Buy,).unwrap(),
            avg_calc
        );
        incoming_spot_price(&ETH, balance!(AVG_BLOCK_SPAN + 1));
        assert_eq!(
            get_average_price(&XOR.into(), &ETH.into(), PriceVariant::Buy,).unwrap(),
            (avg_calc + avg_calc * fixed_wrapper!(0.00197)).into_balance()
        );
    });
}

#[test]
fn average_price_same_values() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        for asset_id in [ETH, DAI, VAL, PSWAP].iter() {
            PriceTools::register_asset(asset_id).unwrap();
        }
        for _ in 1..=AVG_BLOCK_SPAN {
            assert_noop!(
                get_average_price(&XOR.into(), &ETH.into(), PriceVariant::Buy,),
                Error::<Runtime>::InsufficientSpotPriceData
            );
            incoming_spot_price(&ETH, balance!(10));
        }
        assert_eq!(
            get_average_price(&XOR.into(), &ETH.into(), PriceVariant::Buy,).unwrap(),
            balance!(10)
        );
        incoming_spot_price(&ETH, balance!(10));
        assert_eq!(
            get_average_price(&XOR.into(), &ETH.into(), PriceVariant::Buy,).unwrap(),
            balance!(10)
        );
    });
}

#[test]
fn average_price_same_asset() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_eq!(
            get_average_price(&XOR.into(), &XOR.into(), PriceVariant::Buy,).unwrap(),
            balance!(1)
        );
        assert_eq!(
            get_average_price(&XOR.into(), &XOR.into(), PriceVariant::Sell,).unwrap(),
            balance!(1)
        );
        PriceTools::register_asset(&ETH).unwrap();
        assert_eq!(
            get_average_price(&ETH.into(), &ETH.into(), PriceVariant::Sell,).unwrap(),
            balance!(1)
        );
    });
}

#[test]
fn average_price_smoothed_change_without_cap() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        for asset_id in [ETH, DAI, VAL, PSWAP].iter() {
            PriceTools::register_asset(asset_id).unwrap();
        }
        for _ in 1..=AVG_BLOCK_SPAN {
            incoming_spot_price(&ETH, balance!(1000));
        }
        assert_eq!(
            get_average_price(&XOR.into(), &ETH.into(), PriceVariant::Buy,).unwrap(),
            to_avg(
                PriceTools::price_infos(&ETH)
                    .unwrap()
                    .price_of(PriceVariant::Buy)
                    .clone()
                    .spot_prices
                    .iter(),
                AVG_BLOCK_SPAN
            )
        );
        for &new_price in [999u32, 1000, 1003, 1006, 1009, 1015, 1018, 1021, 1024, 1030].iter() {
            incoming_spot_price(&ETH, balance!(new_price));
            assert_eq!(
                get_average_price(&XOR.into(), &ETH.into(), PriceVariant::Buy,).unwrap(),
                to_avg(
                    PriceTools::price_infos(&ETH)
                        .unwrap()
                        .price_of(PriceVariant::Buy)
                        .clone()
                        .spot_prices
                        .iter(),
                    AVG_BLOCK_SPAN
                )
            );
        }
        for &new_price in [1033u32, 1024, 1030, 1039, 1003, 1039, 1000, 1030].iter() {
            incoming_spot_price(&ETH, balance!(new_price));
            assert_eq!(
                get_average_price(&XOR.into(), &ETH.into(), PriceVariant::Buy,).unwrap(),
                to_avg(
                    PriceTools::price_infos(&ETH)
                        .unwrap()
                        .price_of(PriceVariant::Buy)
                        .clone()
                        .spot_prices
                        .iter(),
                    AVG_BLOCK_SPAN
                )
            );
        }
    });
}

#[test]
fn different_average_for_different_assets() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        for asset_id in [ETH, DAI, VAL, PSWAP].iter() {
            PriceTools::register_asset(asset_id).unwrap();
        }
        for _ in 1..=AVG_BLOCK_SPAN {
            incoming_spot_price(&ETH, balance!(0.5));
        }
        for _ in 1..=AVG_BLOCK_SPAN {
            incoming_spot_price(&DAI, balance!(700));
        }
        for _ in 1..=AVG_BLOCK_SPAN {
            incoming_spot_price(&VAL, balance!(2));
        }
        for _ in 1..=AVG_BLOCK_SPAN {
            incoming_spot_price(&PSWAP, balance!(1200));
        }
        for &new_price in [balance!(0.5), balance!(0.5001), balance!(0.5002)].iter() {
            assert_eq!(
                get_average_price(&XOR.into(), &ETH.into(), PriceVariant::Buy,).unwrap(),
                to_avg(
                    PriceTools::price_infos(&ETH)
                        .unwrap()
                        .price_of(PriceVariant::Buy)
                        .clone()
                        .spot_prices
                        .iter(),
                    AVG_BLOCK_SPAN
                )
            );
            assert_eq!(
                get_fast_average_price(&XOR.into(), &ETH.into(), PriceVariant::Buy,).unwrap(),
                to_avg(
                    PriceTools::price_infos(&ETH)
                        .unwrap()
                        .price_of(PriceVariant::Buy)
                        .clone()
                        .spot_prices
                        .iter(),
                    AVG_BLOCK_SPAN
                )
            );
            incoming_spot_price(&ETH, new_price);
        }
        for &new_price in [balance!(700), balance!(700.5), balance!(700.3)].iter() {
            assert_eq!(
                get_average_price(&XOR.into(), &DAI.into(), PriceVariant::Buy,).unwrap(),
                to_avg(
                    PriceTools::price_infos(&DAI)
                        .unwrap()
                        .price_of(PriceVariant::Buy)
                        .clone()
                        .spot_prices
                        .iter(),
                    AVG_BLOCK_SPAN
                )
            );
            assert_eq!(
                get_fast_average_price(&XOR.into(), &DAI.into(), PriceVariant::Buy,).unwrap(),
                to_avg(
                    PriceTools::price_infos(&DAI)
                        .unwrap()
                        .price_of(PriceVariant::Buy)
                        .clone()
                        .spot_prices
                        .iter(),
                    AVG_BLOCK_SPAN
                )
            );
            incoming_spot_price(&DAI, new_price);
        }
        for &new_price in [balance!(2), balance!(2.001), balance!(2.005)].iter() {
            assert_eq!(
                get_average_price(&XOR.into(), &VAL.into(), PriceVariant::Buy,).unwrap(),
                to_avg(
                    PriceTools::price_infos(&VAL)
                        .unwrap()
                        .price_of(PriceVariant::Buy)
                        .clone()
                        .spot_prices
                        .iter(),
                    AVG_BLOCK_SPAN
                )
            );
            assert_eq!(
                get_fast_average_price(&XOR.into(), &VAL.into(), PriceVariant::Buy,).unwrap(),
                to_avg(
                    PriceTools::price_infos(&VAL)
                        .unwrap()
                        .price_of(PriceVariant::Buy)
                        .clone()
                        .spot_prices
                        .iter(),
                    AVG_BLOCK_SPAN
                )
            );
            incoming_spot_price(&VAL, new_price);
        }
        for &new_price in [balance!(1200), balance!(1201.1), balance!(1202.2)].iter() {
            assert_eq!(
                get_average_price(&XOR.into(), &PSWAP.into(), PriceVariant::Buy,).unwrap(),
                to_avg(
                    PriceTools::price_infos(&PSWAP)
                        .unwrap()
                        .price_of(PriceVariant::Buy)
                        .clone()
                        .spot_prices
                        .iter(),
                    AVG_BLOCK_SPAN
                )
            );
            assert_eq!(
                get_fast_average_price(&XOR.into(), &PSWAP.into(), PriceVariant::Buy,).unwrap(),
                to_avg(
                    PriceTools::price_infos(&PSWAP)
                        .unwrap()
                        .price_of(PriceVariant::Buy)
                        .clone()
                        .spot_prices
                        .iter(),
                    AVG_BLOCK_SPAN
                )
            );
            incoming_spot_price(&PSWAP, new_price);
        }
    });
}

#[test]
fn all_exchange_paths_work() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        for asset_id in [ETH, DAI, VAL, PSWAP].iter() {
            PriceTools::register_asset(asset_id).unwrap();
        }
        for _ in 1..=AVG_BLOCK_SPAN {
            incoming_spot_price(&ETH, balance!(0.5));
            incoming_spot_price(&ETH, balance!(0.5));
        }
        for _ in 1..=AVG_BLOCK_SPAN {
            incoming_spot_price(&DAI, balance!(800));
            incoming_spot_price(&DAI, balance!(800));
        }
        // XOR(1)->ETH(0.5)
        assert_eq!(
            get_average_price(&XOR.into(), &ETH.into(), PriceVariant::Buy,).unwrap(),
            balance!(0.5)
        );
        assert_eq!(
            get_fast_average_price(&XOR.into(), &ETH.into(), PriceVariant::Buy,).unwrap(),
            balance!(0.5)
        );
        // ETH(1)->XOR(2)
        assert_eq!(
            get_average_price(&ETH.into(), &XOR.into(), PriceVariant::Buy,).unwrap(),
            balance!(2)
        );
        assert_eq!(
            get_fast_average_price(&ETH.into(), &XOR.into(), PriceVariant::Buy,).unwrap(),
            balance!(2)
        );
        // XOR(1)->DAI(800)
        assert_eq!(
            get_average_price(&XOR.into(), &DAI.into(), PriceVariant::Buy,).unwrap(),
            balance!(800)
        );
        assert_eq!(
            get_fast_average_price(&XOR.into(), &DAI.into(), PriceVariant::Buy,).unwrap(),
            balance!(800)
        );
        // DAI(1)->XOR(0.00125)
        assert_eq!(
            get_average_price(&DAI.into(), &XOR.into(), PriceVariant::Buy,).unwrap(),
            balance!(0.00125)
        );
        assert_eq!(
            get_fast_average_price(&DAI.into(), &XOR.into(), PriceVariant::Buy,).unwrap(),
            balance!(0.00125)
        );
        // ETH(1)->XOR(2)->DAI(1600)
        assert_eq!(
            get_average_price(&ETH.into(), &DAI.into(), PriceVariant::Buy,).unwrap(),
            balance!(1600)
        );
        assert_eq!(
            get_fast_average_price(&ETH.into(), &DAI.into(), PriceVariant::Buy,).unwrap(),
            balance!(1600)
        );
        // DAI(1)->XOR(0.00125)->ETH(0.000625)
        assert_eq!(
            get_average_price(&DAI.into(), &ETH.into(), PriceVariant::Buy,).unwrap(),
            balance!(0.000625)
        );
        assert_eq!(
            get_fast_average_price(&DAI.into(), &ETH.into(), PriceVariant::Buy,).unwrap(),
            balance!(0.000625)
        );
    });
}

#[test]
fn price_quote_continuous_failure() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        for asset_id in [ETH, DAI, VAL, PSWAP].iter() {
            PriceTools::register_asset(asset_id).unwrap();
        }
        // initialization period
        for _ in 1..=AVG_BLOCK_SPAN {
            assert_noop!(
                get_average_price(&XOR.into(), &ETH.into(), PriceVariant::Buy,),
                Error::<Runtime>::InsufficientSpotPriceData
            );
            assert_noop!(
                get_fast_average_price(&XOR.into(), &ETH.into(), PriceVariant::Buy,),
                Error::<Runtime>::InsufficientSpotPriceData
            );
            incoming_spot_price(&ETH, balance!(10));
        }
        assert_eq!(
            get_average_price(&XOR.into(), &ETH.into(), PriceVariant::Buy,).unwrap(),
            balance!(10)
        );
        assert_eq!(
            get_fast_average_price(&XOR.into(), &ETH.into(), PriceVariant::Buy,).unwrap(),
            balance!(10)
        );
        PriceTools::reserves_changed(&ETH);
        // failure period
        for _ in 1..AVG_BLOCK_SPAN {
            PriceTools::average_prices_calculation_routine();
        }
        assert_eq!(
            get_average_price(&XOR.into(), &ETH.into(), PriceVariant::Buy,).unwrap(),
            balance!(10)
        );
        assert_eq!(
            get_fast_average_price(&XOR.into(), &ETH.into(), PriceVariant::Buy,).unwrap(),
            balance!(10)
        );
        PriceTools::average_prices_calculation_routine();

        // recovery period
        for _ in 1..=AVG_BLOCK_SPAN {
            assert_noop!(
                get_average_price(&XOR.into(), &ETH.into(), PriceVariant::Buy,),
                Error::<Runtime>::InsufficientSpotPriceData
            );
            assert_noop!(
                get_fast_average_price(&XOR.into(), &ETH.into(), PriceVariant::Buy,),
                Error::<Runtime>::InsufficientSpotPriceData
            );
            incoming_spot_price(&ETH, balance!(20));
        }
        assert_eq!(
            get_average_price(&XOR.into(), &ETH.into(), PriceVariant::Buy,).unwrap(),
            balance!(20)
        );
        assert_eq!(
            get_fast_average_price(&XOR.into(), &ETH.into(), PriceVariant::Buy,).unwrap(),
            balance!(20)
        );
    });
}

#[test]
fn failure_for_unsupported_assets() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        for asset_id in [ETH, DAI, VAL, PSWAP].iter() {
            PriceTools::register_asset(asset_id).unwrap();
        }
        for _ in 1..=AVG_BLOCK_SPAN {
            assert_noop!(
                get_average_price(&XOR.into(), &ETH.into(), PriceVariant::Buy,),
                Error::<Runtime>::InsufficientSpotPriceData
            );
            assert_noop!(
                get_fast_average_price(&XOR.into(), &ETH.into(), PriceVariant::Buy,),
                Error::<Runtime>::InsufficientSpotPriceData
            );
            incoming_spot_price(&ETH, balance!(10));
        }
        assert_eq!(
            get_average_price(&XOR.into(), &ETH.into(), PriceVariant::Buy,).unwrap(),
            balance!(10)
        );
        assert_eq!(
            get_fast_average_price(&XOR.into(), &ETH.into(), PriceVariant::Buy,).unwrap(),
            balance!(10)
        );
        assert_noop!(
            get_average_price(&XOR.into(), &DOT.into(), PriceVariant::Buy,),
            Error::<Runtime>::UnsupportedQuotePath
        );
        assert_noop!(
            get_fast_average_price(&XOR.into(), &DOT.into(), PriceVariant::Buy,),
            Error::<Runtime>::UnsupportedQuotePath
        );
        assert_noop!(
            get_average_price(&DOT.into(), &XOR.into(), PriceVariant::Buy,),
            Error::<Runtime>::UnsupportedQuotePath
        );
        assert_noop!(
            get_fast_average_price(&DOT.into(), &XOR.into(), PriceVariant::Buy,),
            Error::<Runtime>::UnsupportedQuotePath
        );
        assert_noop!(
            get_average_price(&DOT.into(), &ETH.into(), PriceVariant::Buy,),
            Error::<Runtime>::UnsupportedQuotePath
        );
        assert_noop!(
            get_fast_average_price(&DOT.into(), &ETH.into(), PriceVariant::Buy,),
            Error::<Runtime>::UnsupportedQuotePath
        );
        assert_noop!(
            get_average_price(&ETH.into(), &DOT.into(), PriceVariant::Buy,),
            Error::<Runtime>::UnsupportedQuotePath
        );
        assert_noop!(
            get_fast_average_price(&ETH.into(), &DOT.into(), PriceVariant::Buy,),
            Error::<Runtime>::UnsupportedQuotePath
        );
    });
}

#[test]
fn average_price_large_change_before_no_update_streak_positive() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        for asset_id in [ETH, DAI, VAL, PSWAP].iter() {
            PriceTools::register_asset(asset_id).unwrap();
        }
        for _ in 1..=AVG_BLOCK_SPAN {
            incoming_spot_price(&ETH, balance!(1000));
        }
        assert_eq!(
            PriceTools::price_infos(&ETH)
                .unwrap()
                .price_of(PriceVariant::Buy)
                .clone()
                .last_spot_price,
            balance!(1000)
        );
        assert_eq!(
            get_average_price(&XOR.into(), &ETH.into(), PriceVariant::Buy,).unwrap(),
            to_avg(
                PriceTools::price_infos(&ETH)
                    .unwrap()
                    .price_of(PriceVariant::Buy)
                    .clone()
                    .spot_prices
                    .iter(),
                AVG_BLOCK_SPAN
            )
        );
        assert_eq!(
            get_fast_average_price(&XOR.into(), &ETH.into(), PriceVariant::Buy,).unwrap(),
            to_avg(
                PriceTools::price_infos(&ETH)
                    .unwrap()
                    .price_of(PriceVariant::Buy)
                    .clone()
                    .spot_prices
                    .iter(),
                AVG_BLOCK_SPAN
            )
        );
        // change of 300% occurs, price smoothing kicks in
        for _ in 1..=AVG_BLOCK_SPAN {
            incoming_spot_price(&ETH, balance!(4000));
        }
        assert_eq!(
            get_average_price(&XOR.into(), &ETH.into(), PriceVariant::Buy,).unwrap(),
            balance!(1060.819648734858925676) // not 300% exactly because of compunding effect
        );
        assert_eq!(
            get_fast_average_price(&XOR.into(), &ETH.into(), PriceVariant::Buy,).unwrap(),
            balance!(1060.819648734858925676) // not 300% exactly because of compunding effect
        );
        assert_eq!(
            PriceTools::price_infos(&ETH)
                .unwrap()
                .price_of(PriceVariant::Buy)
                .clone()
                .last_spot_price,
            balance!(4000)
        );
        // same price, continues to repeat, average price is still updated
        for _ in 1..=AVG_BLOCK_SPAN * 23 {
            incoming_spot_price(&ETH, balance!(4000));
        }
        assert_eq!(
            get_average_price(&XOR.into(), &ETH.into(), PriceVariant::Buy,).unwrap(),
            balance!(4000) // reaches target price eventually
        );
        assert_eq!(
            get_fast_average_price(&XOR.into(), &ETH.into(), PriceVariant::Buy,).unwrap(),
            balance!(4000) // reaches target price eventually
        );
    });
}

#[test]
fn average_price_large_change_before_no_update_streak_negative() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        for asset_id in [ETH, DAI, VAL, PSWAP].iter() {
            PriceTools::register_asset(asset_id).unwrap();
        }
        for _ in 1..=AVG_BLOCK_SPAN {
            incoming_spot_price(&ETH, balance!(4000));
        }
        assert_eq!(
            PriceTools::price_infos(&ETH)
                .unwrap()
                .price_of(PriceVariant::Buy)
                .clone()
                .last_spot_price,
            balance!(4000)
        );
        assert_eq!(
            get_average_price(&XOR.into(), &ETH.into(), PriceVariant::Buy,).unwrap(),
            to_avg(
                PriceTools::price_infos(&ETH)
                    .unwrap()
                    .price_of(PriceVariant::Buy)
                    .clone()
                    .spot_prices
                    .iter(),
                AVG_BLOCK_SPAN
            )
        );
        assert_eq!(
            get_fast_average_price(&XOR.into(), &ETH.into(), PriceVariant::Buy,).unwrap(),
            to_avg(
                PriceTools::price_infos(&ETH)
                    .unwrap()
                    .price_of(PriceVariant::Buy)
                    .clone()
                    .spot_prices
                    .iter(),
                AVG_BLOCK_SPAN
            )
        );
        // change over 15% occurs, price smoothing kicks in
        for _ in 1..=AVG_BLOCK_SPAN {
            incoming_spot_price(&ETH, balance!(700));
        }
        assert_eq!(
            get_average_price(&XOR.into(), &ETH.into(), PriceVariant::Buy,).unwrap(),
            balance!(3997.600695870097537361) // not 15% exactly because of compunding effect
        );
        assert_eq!(
            get_fast_average_price(&XOR.into(), &ETH.into(), PriceVariant::Buy,).unwrap(),
            balance!(3770.230239396238063333) // not 15% exactly because of compunding effect
        );
        assert_eq!(
            PriceTools::price_infos(&ETH)
                .unwrap()
                .price_of(PriceVariant::Buy)
                .clone()
                .last_spot_price,
            balance!(700)
        );
        // same price, continues to repeat, average price is still updated
        for _ in 1..=AVG_BLOCK_SPAN * 8000 {
            incoming_spot_price(&ETH, balance!(700));
        }
        assert_eq!(
            get_average_price(&XOR.into(), &ETH.into(), PriceVariant::Buy,).unwrap(),
            balance!(700) // reaches target price eventually
        );
        assert_eq!(
            get_fast_average_price(&XOR.into(), &ETH.into(), PriceVariant::Buy,).unwrap(),
            balance!(700) // reaches target price eventually
        );
    });
}

#[test]
fn price_should_go_up_faster_than_going_down() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        for asset_id in [ETH, DAI, VAL, PSWAP].iter() {
            PriceTools::register_asset(asset_id).unwrap();
        }
        let price_a = balance!(1);
        let price_b = balance!(100);
        for _ in 1..=AVG_BLOCK_SPAN {
            assert_noop!(
                get_average_price(&XOR.into(), &DAI.into(), PriceVariant::Buy,),
                Error::<Runtime>::InsufficientSpotPriceData
            );
            incoming_spot_price(&DAI, price_a);
        }
        assert_eq!(
            get_average_price(&XOR.into(), &DAI.into(), PriceVariant::Buy,).unwrap(),
            price_a
        );
        let mut n = 0;
        // Increasing price from `price_a` to `price_b`
        loop {
            incoming_spot_price(&DAI, price_b);
            let actual_price =
                get_average_price(&XOR.into(), &DAI.into(), PriceVariant::Buy).unwrap();

            n += 1;
            if actual_price == price_b {
                break;
            }
        }

        let mut m = 0;
        // Decreasing price from `price_b` to `price_a`
        loop {
            incoming_spot_price(&DAI, price_a);
            let actual_price =
                get_average_price(&XOR.into(), &DAI.into(), PriceVariant::Buy).unwrap();

            m += 1;
            if actual_price == price_a {
                break;
            }
        }
        assert_eq!(n, 2355);
        assert_eq!(m, 231690);
    });
}

#[test]
fn fast_price_should_go_up_faster_than_going_down() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        for asset_id in [ETH, DAI, VAL, PSWAP].iter() {
            PriceTools::register_asset(asset_id).unwrap();
        }
        let price_a = balance!(1);
        let price_b = balance!(100);
        for _ in 1..=AVG_BLOCK_SPAN {
            assert_noop!(
                get_fast_average_price(&XOR.into(), &DAI.into(), PriceVariant::Buy,),
                Error::<Runtime>::InsufficientSpotPriceData
            );
            incoming_spot_price(&DAI, price_a);
        }
        assert_eq!(
            get_fast_average_price(&XOR.into(), &DAI.into(), PriceVariant::Buy,).unwrap(),
            price_a
        );
        let mut n = 0;
        // Increasing price from `price_a` to `price_b`
        loop {
            incoming_spot_price(&DAI, price_b);
            let actual_price =
                get_fast_average_price(&XOR.into(), &DAI.into(), PriceVariant::Buy).unwrap();

            n += 1;
            if actual_price == price_b {
                break;
            }
        }

        let mut m = 0;
        // Decreasing price from `price_b` to `price_a`
        loop {
            incoming_spot_price(&DAI, price_a);
            let actual_price =
                get_fast_average_price(&XOR.into(), &DAI.into(), PriceVariant::Buy).unwrap();

            m += 1;
            if actual_price == price_a {
                break;
            }
        }
        assert_eq!(n, 2355);
        assert_eq!(m, 3420);
    });
}

#[test]
fn asset_already_registered() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        PriceTools::register_asset(&ETH).unwrap();
        assert_noop!(
            PriceTools::register_asset(&ETH),
            Error::<Runtime>::AssetAlreadyRegistered
        );
    });
}
