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
use crate::*;
use common::mock::{alice, bob};
use common::{balance, AssetName, AssetSymbol, ContentSource, Description};
use frame_support::assert_err;
use frame_support::assert_ok;

type Market = Pallet<Runtime>;

fn test_price_asset() -> AssetId {
    let asset_id = Assets::register_from(
        &alice(),
        AssetSymbol(b"USD".to_vec()),
        AssetName(b"USD".to_vec()),
        18,
        balance!(1000),
        true,
        AssetType::Regular,
        None,
        None,
    )
    .unwrap();
    Assets::mint_unchecked(&asset_id, &bob(), balance!(1000)).unwrap();
    asset_id
}

#[test]
fn test_create_product() {
    new_test_ext().execute_with(|| {
        let price_asset = test_price_asset();
        assert_ok!(Market::create_product(
            RuntimeOrigin::signed(alice()),
            AssetName(b"CheckIn".to_vec()),
            AssetSymbol(b"CHECKIN".to_vec()),
            Description(b"User Check-in".to_vec()),
            ContentSource(b"https://checkin".to_vec()),
            Product {
                price_asset,
                price: balance!(1),
                extensions: vec![]
            }
        ));
        let (product_id, product) = Products::<Runtime>::iter().next().unwrap();
        System::assert_has_event(
            Event::<Runtime>::ProductRegistered {
                asset_id: product_id,
            }
            .into(),
        );
        assert_eq!(
            product,
            Product {
                price_asset,
                price: balance!(1),
                extensions: vec![]
            }
        );
    })
}

#[test]
fn test_buy_product() {
    new_test_ext().execute_with(|| {
        let price_asset = test_price_asset();
        let product_id = Market::create_new_product(
            alice(),
            AssetName(b"CheckIn".to_vec()),
            AssetSymbol(b"CHECKIN".to_vec()),
            Description(b"User Check-in".to_vec()),
            ContentSource(b"https://checkin".to_vec()),
            Product {
                price_asset,
                price: balance!(1),
                extensions: vec![],
            },
        )
        .unwrap();

        assert_ok!(Market::buy(RuntimeOrigin::signed(bob()), product_id, 1));
        assert_eq!(Assets::total_balance(&product_id, &bob()).unwrap(), 1);
        assert_eq!(
            Assets::total_balance(&price_asset, &bob()).unwrap(),
            balance!(999)
        );

        assert_ok!(Market::buy(RuntimeOrigin::signed(bob()), product_id, 7));
        assert_eq!(Assets::total_balance(&product_id, &bob()).unwrap(), 8);
        assert_eq!(
            Assets::total_balance(&price_asset, &bob()).unwrap(),
            balance!(992)
        );
    })
}

#[test]
fn test_buy_expirable_product() {
    new_test_ext().execute_with(|| {
        let price_asset = test_price_asset();
        let product_id = Market::create_new_product(
            alice(),
            AssetName(b"CheckIn".to_vec()),
            AssetSymbol(b"CHECKIN".to_vec()),
            Description(b"User Check-in".to_vec()),
            ContentSource(b"https://checkin".to_vec()),
            Product {
                price_asset,
                price: balance!(1),
                extensions: vec![Extension::Expirable(100)],
            },
        )
        .unwrap();

        assert_ok!(Market::buy(RuntimeOrigin::signed(bob()), product_id, 1));
        assert_eq!(Assets::total_balance(&product_id, &bob()).unwrap(), 1);
        assert_eq!(
            Assets::total_balance(&price_asset, &bob()).unwrap(),
            balance!(999)
        );
        assert_eq!(Expirations::<Runtime>::get(101, (bob(), product_id)), 1);

        run_to_block(50);
        assert_eq!(Assets::total_balance(&product_id, &bob()).unwrap(), 1);
        assert_ok!(Market::buy(RuntimeOrigin::signed(bob()), product_id, 1));
        assert_eq!(Assets::total_balance(&product_id, &bob()).unwrap(), 2);
        assert_eq!(Expirations::<Runtime>::get(101, (bob(), product_id)), 1);
        assert_eq!(Expirations::<Runtime>::get(150, (bob(), product_id)), 1);

        run_to_block(101);
        assert_eq!(Expirations::<Runtime>::get(150, (bob(), product_id)), 1);
        assert!(!Expirations::<Runtime>::contains_key(
            101,
            (bob(), product_id)
        ));
        assert_eq!(Assets::total_balance(&product_id, &bob()).unwrap(), 1);

        run_to_block(150);
        assert!(!Expirations::<Runtime>::contains_key(
            150,
            (bob(), product_id)
        ));
        assert_eq!(Assets::total_balance(&product_id, &bob()).unwrap(), 0);
    })
}

#[test]
fn test_buy_product_with_requirement() {
    new_test_ext().execute_with(|| {
        let price_asset = test_price_asset();
        let checkin_product_id = Market::create_new_product(
            alice(),
            AssetName(b"CheckIn".to_vec()),
            AssetSymbol(b"CHECKIN".to_vec()),
            Description(b"User Check-in".to_vec()),
            ContentSource(b"https://checkin".to_vec()),
            Product {
                price_asset,
                price: balance!(1),
                extensions: vec![],
            },
        )
        .unwrap();

        let upgrade_product_id = Market::create_new_product(
            alice(),
            AssetName(b"Upgrade".to_vec()),
            AssetSymbol(b"UPGRADE".to_vec()),
            Description(b"User Upgrade".to_vec()),
            ContentSource(b"https://upgrade".to_vec()),
            Product {
                price_asset,
                price: balance!(10),
                extensions: vec![Extension::RequiredProduct(checkin_product_id, 2)],
            },
        )
        .unwrap();

        assert_err!(
            Market::buy(RuntimeOrigin::signed(bob()), upgrade_product_id, 1),
            Error::<Runtime>::MissingRequiredProduct
        );
        assert_ok!(Market::buy(
            RuntimeOrigin::signed(bob()),
            checkin_product_id,
            1
        ));
        assert_eq!(
            Assets::total_balance(&checkin_product_id, &bob()).unwrap(),
            1
        );
        assert_eq!(
            Assets::total_balance(&price_asset, &bob()).unwrap(),
            balance!(999)
        );
        assert_err!(
            Market::buy(RuntimeOrigin::signed(bob()), upgrade_product_id, 1),
            Error::<Runtime>::MissingRequiredProduct
        );
        assert_ok!(Market::buy(
            RuntimeOrigin::signed(bob()),
            checkin_product_id,
            1
        ));
        assert_eq!(
            Assets::total_balance(&checkin_product_id, &bob()).unwrap(),
            2
        );
        assert_eq!(
            Assets::total_balance(&price_asset, &bob()).unwrap(),
            balance!(998)
        );
        assert_ok!(Market::buy(
            RuntimeOrigin::signed(bob()),
            upgrade_product_id,
            1
        ));
        assert_eq!(
            Assets::total_balance(&upgrade_product_id, &bob()).unwrap(),
            1
        );
        assert_eq!(
            Assets::total_balance(&price_asset, &bob()).unwrap(),
            balance!(988)
        );
    })
}

#[test]
fn test_buy_max_amount_product() {
    new_test_ext().execute_with(|| {
        let price_asset = test_price_asset();
        let product_id = Market::create_new_product(
            alice(),
            AssetName(b"CheckIn".to_vec()),
            AssetSymbol(b"CHECKIN".to_vec()),
            Description(b"User Check-in".to_vec()),
            ContentSource(b"https://checkin".to_vec()),
            Product {
                price_asset,
                price: balance!(1),
                extensions: vec![Extension::MaxAmount(2)],
            },
        )
        .unwrap();

        assert_err!(
            Market::buy(RuntimeOrigin::signed(bob()), product_id, 3),
            Error::<Runtime>::MaxAmountExceeded
        );

        assert_ok!(Market::buy(RuntimeOrigin::signed(bob()), product_id, 1));
        assert_eq!(Assets::total_balance(&product_id, &bob()).unwrap(), 1);
        assert_eq!(
            Assets::total_balance(&price_asset, &bob()).unwrap(),
            balance!(999)
        );

        assert_err!(
            Market::buy(RuntimeOrigin::signed(bob()), product_id, 2),
            Error::<Runtime>::MaxAmountExceeded
        );

        assert_ok!(Market::buy(RuntimeOrigin::signed(bob()), product_id, 1));
        assert_eq!(Assets::total_balance(&product_id, &bob()).unwrap(), 2);
        assert_eq!(
            Assets::total_balance(&price_asset, &bob()).unwrap(),
            balance!(998)
        );

        assert_err!(
            Market::buy(RuntimeOrigin::signed(bob()), product_id, 1),
            Error::<Runtime>::MaxAmountExceeded
        );
    })
}

#[test]
fn test_buy_product_with_disallowed_product() {
    new_test_ext().execute_with(|| {
        let price_asset = test_price_asset();
        let checkin_product_id = Market::create_new_product(
            alice(),
            AssetName(b"CheckIn".to_vec()),
            AssetSymbol(b"CHECKIN".to_vec()),
            Description(b"User Check-in".to_vec()),
            ContentSource(b"https://checkin".to_vec()),
            Product {
                price_asset,
                price: balance!(1),
                extensions: vec![],
            },
        )
        .unwrap();

        let upgrade_product_id = Market::create_new_product(
            alice(),
            AssetName(b"Upgrade".to_vec()),
            AssetSymbol(b"UPGRADE".to_vec()),
            Description(b"User Upgrade".to_vec()),
            ContentSource(b"https://upgrade".to_vec()),
            Product {
                price_asset,
                price: balance!(10),
                extensions: vec![Extension::DisallowedProduct(checkin_product_id)],
            },
        )
        .unwrap();

        assert_ok!(Market::buy(
            RuntimeOrigin::signed(bob()),
            upgrade_product_id,
            1
        ),);
        assert_eq!(
            Assets::total_balance(&upgrade_product_id, &bob()).unwrap(),
            1
        );
        assert_eq!(
            Assets::total_balance(&price_asset, &bob()).unwrap(),
            balance!(990)
        );
        assert_ok!(Market::buy(
            RuntimeOrigin::signed(bob()),
            checkin_product_id,
            1
        ));
        assert_eq!(
            Assets::total_balance(&checkin_product_id, &bob()).unwrap(),
            1
        );
        assert_eq!(
            Assets::total_balance(&price_asset, &bob()).unwrap(),
            balance!(989)
        );
        assert_err!(
            Market::buy(RuntimeOrigin::signed(bob()), upgrade_product_id, 1),
            Error::<Runtime>::HaveDisallowedProduct
        );
    })
}

#[test]
fn test_extensions_verification() {
    new_test_ext().execute_with(|| {
        let price_asset = test_price_asset();
        let product_id_2 = Market::create_new_product(
            alice(),
            AssetName(b"CheckIn".to_vec()),
            AssetSymbol(b"CHECKIN".to_vec()),
            Description(b"User Check-in".to_vec()),
            ContentSource(b"https://checkin".to_vec()),
            Product {
                price_asset,
                price: balance!(1),
                extensions: vec![Extension::MaxAmount(2)],
            },
        )
        .unwrap();
        let product_id_1 = Market::create_new_product(
            alice(),
            AssetName(b"CheckIn".to_vec()),
            AssetSymbol(b"CHECKIN".to_vec()),
            Description(b"User Check-in".to_vec()),
            ContentSource(b"https://checkin".to_vec()),
            Product {
                price_asset,
                price: balance!(1),
                extensions: vec![Extension::MaxAmount(2)],
            },
        )
        .unwrap();
        for (extensions, err) in [
            (vec![], None),
            (vec![Extension::MaxAmount(1)], None),
            (
                vec![Extension::MaxAmount(0)],
                Some(Error::<Runtime>::ZeroMaxAmount),
            ),
            (
                vec![Extension::MaxAmount(1), Extension::MaxAmount(2)],
                Some(Error::<Runtime>::MultipleExtensionsNotAllowed),
            ),
            (vec![Extension::Expirable(1)], None),
            (
                vec![Extension::Expirable(0)],
                Some(Error::<Runtime>::ZeroExpiration),
            ),
            (
                vec![Extension::Expirable(1), Extension::Expirable(2)],
                Some(Error::<Runtime>::MultipleExtensionsNotAllowed),
            ),
            (vec![Extension::Expirable(1), Extension::MaxAmount(1)], None),
            (
                vec![
                    Extension::MaxAmount(1),
                    Extension::Expirable(1),
                    Extension::Expirable(2),
                ],
                Some(Error::<Runtime>::MultipleExtensionsNotAllowed),
            ),
            (vec![Extension::RequiredProduct(product_id_1, 1)], None),
            (
                vec![
                    Extension::RequiredProduct(product_id_1, 1),
                    Extension::DisallowedProduct(product_id_2),
                ],
                None,
            ),
            (
                vec![
                    Extension::RequiredProduct(product_id_1, 1),
                    Extension::RequiredProduct(product_id_1, 1),
                ],
                Some(Error::AmbigiousProductRequirements),
            ),
            (
                vec![
                    Extension::RequiredProduct(product_id_1, 1),
                    Extension::DisallowedProduct(product_id_1),
                ],
                Some(Error::AmbigiousProductRequirements),
            ),
            (
                vec![
                    Extension::DisallowedProduct(product_id_1),
                    Extension::RequiredProduct(product_id_1, 1),
                ],
                Some(Error::AmbigiousProductRequirements),
            ),
            (
                vec![
                    Extension::DisallowedProduct(product_id_1),
                    Extension::DisallowedProduct(product_id_1),
                ],
                Some(Error::AmbigiousProductRequirements),
            ),
        ] {
            if let Some(err) = err {
                assert_err!(
                    Market::create_product(
                        RuntimeOrigin::signed(alice()),
                        AssetName(b"CheckIn".to_vec()),
                        AssetSymbol(b"CHECKIN".to_vec()),
                        Description(b"User Check-in".to_vec()),
                        ContentSource(b"https://checkin".to_vec()),
                        Product {
                            price_asset,
                            price: balance!(1),
                            extensions
                        }
                    ),
                    err
                );
            } else {
                assert_ok!(Market::create_product(
                    RuntimeOrigin::signed(alice()),
                    AssetName(b"CheckIn".to_vec()),
                    AssetSymbol(b"CHECKIN".to_vec()),
                    Description(b"User Check-in".to_vec()),
                    ContentSource(b"https://checkin".to_vec()),
                    Product {
                        price_asset,
                        price: balance!(1),
                        extensions
                    }
                ));
            }
        }
    })
}
