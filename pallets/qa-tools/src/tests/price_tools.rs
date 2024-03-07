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

use super::QaToolsPallet;
use assets::AssetIdOf;
use common::{balance, PriceVariant, ETH, XOR};
use frame_support::{assert_err, assert_ok};
use framenode_chain_spec::ext;
use framenode_runtime::qa_tools;
use framenode_runtime::{Runtime, RuntimeOrigin};
use qa_tools::{pallet_tools, Error, InputAssetId};

use pallet_tools::price_tools::AssetPrices;

fn check_price_tools_set_price(asset_id: &InputAssetId<AssetIdOf<Runtime>>, prices: AssetPrices) {
    assert_ok!(QaToolsPallet::price_tools_set_asset_price(
        RuntimeOrigin::root(),
        prices.clone(),
        asset_id.clone()
    ));
    let asset_id = asset_id.clone().resolve::<Runtime>();
    assert_eq!(
        price_tools::Pallet::<Runtime>::get_average_price(&XOR, &asset_id, PriceVariant::Buy),
        Ok(prices.buy)
    );
    assert_eq!(
        price_tools::Pallet::<Runtime>::get_average_price(&XOR, &asset_id, PriceVariant::Sell),
        Ok(prices.sell)
    );
}

fn test_price_tools_set_asset_prices(asset_id: InputAssetId<AssetIdOf<Runtime>>) {
    ext().execute_with(|| {
        check_price_tools_set_price(
            &asset_id,
            AssetPrices {
                buy: balance!(1),
                sell: balance!(1),
            },
        );
        check_price_tools_set_price(
            &asset_id,
            AssetPrices {
                buy: balance!(2),
                sell: balance!(1),
            },
        );
        check_price_tools_set_price(
            &asset_id,
            AssetPrices {
                buy: balance!(365),
                sell: balance!(256),
            },
        );
        check_price_tools_set_price(
            &asset_id,
            AssetPrices {
                buy: balance!(1),
                sell: balance!(1),
            },
        );
    })
}

#[test]
fn should_set_price_tools_mcbc_base_prices() {
    test_price_tools_set_asset_prices(InputAssetId::<AssetIdOf<Runtime>>::McbcReference);
}

#[test]
fn should_set_price_tools_xst_base_prices() {
    test_price_tools_set_asset_prices(InputAssetId::<AssetIdOf<Runtime>>::XstReference);
}

#[test]
fn should_set_price_tools_other_base_prices() {
    test_price_tools_set_asset_prices(InputAssetId::<AssetIdOf<Runtime>>::Other(ETH));
}

#[test]
fn should_price_tools_reject_incorrect_prices() {
    ext().execute_with(|| {
        assert_err!(
            QaToolsPallet::price_tools_set_asset_price(
                RuntimeOrigin::root(),
                AssetPrices {
                    buy: balance!(1),
                    sell: balance!(1) + 1,
                },
                InputAssetId::<AssetIdOf<Runtime>>::McbcReference
            ),
            Error::<Runtime>::BuyLessThanSell
        );
        assert_err!(
            QaToolsPallet::price_tools_set_asset_price(
                RuntimeOrigin::root(),
                AssetPrices {
                    buy: balance!(1),
                    sell: balance!(1) + 1,
                },
                InputAssetId::<AssetIdOf<Runtime>>::XstReference
            ),
            Error::<Runtime>::BuyLessThanSell
        );
        assert_err!(
            QaToolsPallet::price_tools_set_asset_price(
                RuntimeOrigin::root(),
                AssetPrices {
                    buy: balance!(1),
                    sell: balance!(1) + 1,
                },
                InputAssetId::<AssetIdOf<Runtime>>::Other(ETH)
            ),
            Error::<Runtime>::BuyLessThanSell
        );
    })
}
