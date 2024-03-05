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
use crate::Error;
use crate::Event;
use common::balance;
use common::prelude::{AssetName, AssetSymbol, Balance};
use common::DAI;
use common::PSWAP;
use common::XST;
use common::{
    AssetId32, AssetInfoProvider, ContentSource, Description, IsValid,
    ASSET_CONTENT_SOURCE_MAX_LENGTH, ASSET_DESCRIPTION_MAX_LENGTH, DEFAULT_BALANCE_PRECISION, DOT,
    VAL, XOR,
};
use frame_support::assert_noop;
use frame_support::error::BadOrigin;
use frame_support::{assert_err, assert_ok};
use hex_literal::hex;
use sp_runtime::traits::Zero;

#[test]
fn should_gen_and_register_asset() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let next_asset_id = Assets::gen_asset_id(&ALICE);
        assert_eq!(
            next_asset_id,
            AssetId32::from_bytes(hex!(
                "00770dfe3392f9bb8ab977ce23d11c92e25140c39a9d8115714168d6e484ea41"
            ))
        );
        assert!(Assets::ensure_asset_exists(&next_asset_id).is_err());
        assert_ok!(Assets::register(
            RuntimeOrigin::signed(ALICE),
            AssetSymbol(b"ALIC".to_vec()),
            AssetName(b"ALICE".to_vec()),
            Balance::zero(),
            true,
            false,
            None,
            None,
        ));
        assert_ok!(Assets::ensure_asset_exists(&next_asset_id));
        assert_ne!(Assets::gen_asset_id(&ALICE), next_asset_id);
    });
}

#[test]
fn should_register_asset() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert!(Assets::ensure_asset_exists(&XOR).is_err());
        assert_ok!(Assets::register_asset_id(
            ALICE,
            XOR,
            AssetSymbol(b"XOR".to_vec()),
            AssetName(b"SORA".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::zero(),
            true,
            None,
            None,
        ));
        assert_ok!(Assets::ensure_asset_exists(&XOR));
    });
}

#[test]
fn should_not_register_duplicated_asset() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_ok!(Assets::register_asset_id(
            ALICE,
            XOR,
            AssetSymbol(b"XOR".to_vec()),
            AssetName(b"SORA".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::zero(),
            true,
            None,
            None,
        ));
        assert_noop!(
            Assets::register_asset_id(
                ALICE,
                XOR,
                AssetSymbol(b"XOR".to_vec()),
                AssetName(b"SORA".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                Balance::zero(),
                true,
                None,
                None,
            ),
            Error::<Runtime>::AssetIdAlreadyExists
        );
    });
}

#[test]
fn should_not_register_invalid_asset_name() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_err!(
            Assets::register_asset_id(
                ALICE,
                XOR,
                AssetSymbol(b"XOR".to_vec()),
                AssetName(b"This is a name with length over thirty three".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                Balance::zero(),
                true,
                None,
                None,
            ),
            Error::<Runtime>::InvalidAssetName
        );

        assert_err!(
            Assets::register_asset_id(
                ALICE,
                XOR,
                AssetSymbol(b"XOR".to_vec()),
                AssetName(b"".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                Balance::zero(),
                true,
                None,
                None,
            ),
            Error::<Runtime>::InvalidAssetName
        );

        assert_err!(
            Assets::register_asset_id(
                ALICE,
                VAL,
                AssetSymbol(b"VAL".to_vec()),
                AssetName(b"This is a name with $ymbols".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                Balance::zero(),
                true,
                None,
                None,
            ),
            Error::<Runtime>::InvalidAssetName
        );

        assert_err!(
            Assets::register_asset_id(
                ALICE,
                DOT,
                AssetSymbol(b"DOT".to_vec()),
                AssetName(b"This is a name with _".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                Balance::zero(),
                true,
                None,
                None,
            ),
            Error::<Runtime>::InvalidAssetName
        );
    });
}

#[test]
fn should_not_register_invalid_asset_symbol() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_err!(
            Assets::register_asset_id(
                ALICE,
                XOR,
                AssetSymbol(b"xor".to_vec()),
                AssetName(b"Super Sora".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                Balance::zero(),
                true,
                None,
                None,
            ),
            Error::<Runtime>::InvalidAssetSymbol
        );

        assert_err!(
            Assets::register_asset_id(
                ALICE,
                XOR,
                AssetSymbol(b"".to_vec()),
                AssetName(b"Super Sora".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                Balance::zero(),
                true,
                None,
                None,
            ),
            Error::<Runtime>::InvalidAssetSymbol
        );

        assert_err!(
            Assets::register_asset_id(
                ALICE,
                VAL,
                AssetSymbol(b"VAL IS SUPER LONG".to_vec()),
                AssetName(b"Validator".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                Balance::zero(),
                true,
                None,
                None,
            ),
            Error::<Runtime>::InvalidAssetSymbol
        );

        assert_err!(
            Assets::register_asset_id(
                ALICE,
                DOT,
                AssetSymbol(b"D_OT".to_vec()),
                AssetName(b"Bad Symbol".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                Balance::zero(),
                true,
                None,
                None,
            ),
            Error::<Runtime>::InvalidAssetSymbol
        );
    });
}

#[test]
fn should_allow_operation() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_ok!(Assets::register_asset_id(
            ALICE,
            XOR,
            AssetSymbol(b"XOR".to_vec()),
            AssetName(b"SORA".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::zero(),
            true,
            None,
            None,
        ));
        assert_ok!(Assets::mint_to(&XOR, &ALICE, &ALICE, 100u32.into()));
        assert_ok!(Assets::burn_from(&XOR, &ALICE, &ALICE, 100u32.into()));
        assert_ok!(Assets::update_own_balance(&XOR, &ALICE, 100.into()));
    });
}

#[test]
fn should_not_allow_operation() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_ok!(Assets::register_asset_id(
            ALICE,
            XOR,
            AssetSymbol(b"XOR".to_vec()),
            AssetName(b"SORA".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::zero(),
            true,
            None,
            None,
        ));
        assert_noop!(
            Assets::mint_to(&XOR, &BOB, &BOB, 100u32.into()),
            permissions::Error::<Runtime>::Forbidden
        );
        assert_noop!(
            Assets::update_own_balance(&XOR, &BOB, 100u32.into()),
            permissions::Error::<Runtime>::Forbidden
        );
    });
}

#[test]
fn should_check_symbols_correctly() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert!(AssetSymbol(b"XOR".to_vec()).is_valid());
        assert!(AssetSymbol(b"DOT".to_vec()).is_valid());
        assert!(AssetSymbol(b"KSM".to_vec()).is_valid());
        assert!(AssetSymbol(b"USDT".to_vec()).is_valid());
        assert!(AssetSymbol(b"VAL".to_vec()).is_valid());
        assert!(AssetSymbol(b"PSWAP".to_vec()).is_valid());
        assert!(AssetSymbol(b"GT".to_vec()).is_valid());
        assert!(AssetSymbol(b"BP".to_vec()).is_valid());
        assert!(AssetSymbol(b"AB1".to_vec()).is_valid());

        assert!(!AssetSymbol(b"ABCDEFGH".to_vec()).is_valid());
        assert!(!AssetSymbol(b"xor".to_vec()).is_valid());
        assert!(!AssetSymbol(b"\xF0\x9F\x98\xBF".to_vec()).is_valid());
    })
}

#[test]
fn should_check_names_correctly() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert!(AssetName(b"XOR".to_vec()).is_valid());
        assert!(AssetName(b"DOT".to_vec()).is_valid());
        assert!(AssetName(b"KSM".to_vec()).is_valid());
        assert!(AssetName(b"USDT".to_vec()).is_valid());
        assert!(AssetName(b"VAL".to_vec()).is_valid());
        assert!(AssetName(b"PSWAP".to_vec()).is_valid());
        assert!(AssetName(b"GT".to_vec()).is_valid());
        assert!(AssetName(b"BP".to_vec()).is_valid());
        assert!(AssetName(b"SORA Validator Token".to_vec()).is_valid());
        assert!(AssetName(b"AB1".to_vec()).is_valid());

        assert!(!AssetName(b"This is a name with length over thirty three".to_vec()).is_valid());
        assert!(!AssetName(b"AB1_".to_vec()).is_valid());
        assert!(!AssetName(b"\xF0\x9F\x98\xBF".to_vec()).is_valid());
    })
}

#[test]
fn should_mint_initial_supply_for_owner() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_ok!(Assets::register_asset_id(
            ALICE,
            XOR,
            AssetSymbol(b"XOR".to_vec()),
            AssetName(b"SORA".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::from(123u32),
            true,
            None,
            None,
        ));
        assert_eq!(
            Assets::free_balance(&XOR, &ALICE).expect("Failed to query free balance."),
            Balance::from(123u32),
        );
        assert_ok!(Assets::register_asset_id(
            ALICE,
            VAL,
            AssetSymbol(b"VAL".to_vec()),
            AssetName(b"SORA Validator Token".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::from(321u32),
            false,
            None,
            None,
        ));
        assert_eq!(
            Assets::free_balance(&VAL, &ALICE).expect("Failed to query free balance."),
            Balance::from(321u32),
        );
    })
}

#[test]
fn should_not_allow_dead_asset() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_eq!(
            Assets::register_asset_id(
                ALICE,
                DOT,
                AssetSymbol(b"DOT".to_vec()),
                AssetName(b"Polkadot".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                Balance::from(0u32),
                false,
                None,
                None,
            ),
            Err(Error::<Runtime>::DeadAsset.into())
        );
    })
}

#[test]
fn should_fail_with_non_mintable_asset_supply() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_ok!(Assets::register_asset_id(
            ALICE,
            XOR,
            AssetSymbol(b"XOR".to_vec()),
            AssetName(b"SORA".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::from(10u32),
            false,
            None,
            None,
        ));
        assert_noop!(
            Assets::mint_to(&XOR, &ALICE, &ALICE, Balance::from(10u32)),
            Error::<Runtime>::AssetSupplyIsNotMintable
        );
        assert_noop!(
            Assets::mint_to(&XOR, &ALICE, &BOB, Balance::from(10u32)),
            Error::<Runtime>::AssetSupplyIsNotMintable
        );
        assert_noop!(
            Assets::update_own_balance(&XOR, &ALICE, 1i128),
            Error::<Runtime>::AssetSupplyIsNotMintable
        );
        assert_ok!(Assets::update_own_balance(&XOR, &ALICE, 0i128),);
        assert_ok!(Assets::update_own_balance(&XOR, &ALICE, -1i128),);
    })
}

#[test]
fn should_mint_for_mintable_asset() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_ok!(Assets::register_asset_id(
            ALICE,
            XOR,
            AssetSymbol(b"XOR".to_vec()),
            AssetName(b"SORA".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::from(10u32),
            true,
            None,
            None,
        ));
        assert_ok!(Assets::mint_to(&XOR, &ALICE, &ALICE, Balance::from(10u32)),);
        assert_ok!(Assets::mint_to(&XOR, &ALICE, &BOB, Balance::from(10u32)),);
        assert_ok!(Assets::update_own_balance(&XOR, &ALICE, 1i128),);
        assert_ok!(Assets::update_own_balance(&XOR, &ALICE, 0i128),);
        assert_ok!(Assets::update_own_balance(&XOR, &ALICE, -1i128),);

        assert_noop!(
            Assets::set_non_mintable_from(&XOR, &BOB),
            Error::<Runtime>::InvalidAssetOwner
        );
        assert_ok!(Assets::set_non_mintable_from(&XOR, &ALICE));

        assert_noop!(
            Assets::mint_to(&XOR, &ALICE, &ALICE, Balance::from(10u32)),
            Error::<Runtime>::AssetSupplyIsNotMintable
        );
        assert_noop!(
            Assets::mint_to(&XOR, &ALICE, &BOB, Balance::from(10u32)),
            Error::<Runtime>::AssetSupplyIsNotMintable
        );
        assert_noop!(
            Assets::update_own_balance(&XOR, &ALICE, 1i128),
            Error::<Runtime>::AssetSupplyIsNotMintable
        );
        assert_ok!(Assets::update_own_balance(&XOR, &ALICE, 0i128),);
        assert_ok!(Assets::update_own_balance(&XOR, &ALICE, -1i128),);
    })
}

#[test]
fn should_not_allow_duplicate_set_non_mintable() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_ok!(Assets::register_asset_id(
            ALICE,
            XOR,
            AssetSymbol(b"XOR".to_vec()),
            AssetName(b"SORA".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::from(10u32),
            true,
            None,
            None,
        ));
        assert_ok!(Assets::set_non_mintable_from(&XOR, &ALICE));
        assert_noop!(
            Assets::set_non_mintable_from(&XOR, &ALICE),
            Error::<Runtime>::AssetSupplyIsNotMintable
        );
    })
}

#[test]
fn should_burn_from() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_ok!(Assets::register_asset_id(
            ALICE,
            XOR,
            AssetSymbol(b"XOR".to_vec()),
            AssetName(b"SORA".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::from(10u32),
            true,
            None,
            None,
        ));
        assert_eq!(
            Assets::free_balance(&XOR, &ALICE).expect("Failed to query free balance."),
            Balance::from(10u32),
        );
        assert_ok!(Assets::burn_from(
            &XOR,
            &ALICE,
            &ALICE,
            Balance::from(10u32)
        ));
        assert_eq!(
            Assets::free_balance(&XOR, &ALICE).expect("Failed to query free balance."),
            Balance::from(0u32),
        );
    })
}

#[test]
fn should_not_allow_burn_from_due_to_permissions() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_ok!(Assets::register_asset_id(
            ALICE,
            XOR,
            AssetSymbol(b"XOR".to_vec()),
            AssetName(b"SORA".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::from(10u32),
            true,
            None,
            None,
        ));
        assert_noop!(
            Assets::burn_from(&XOR, &BOB, &ALICE, Balance::from(10u32)),
            permissions::Error::<Runtime>::Forbidden
        );
    })
}

#[test]
fn should_allow_burn_from_self_without_a_permissions() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_ok!(Assets::register_asset_id(
            ALICE,
            XOR,
            AssetSymbol(b"XOR".to_vec()),
            AssetName(b"SORA".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::from(10u32),
            true,
            None,
            None,
        ));
        assert_ok!(Assets::mint_to(&XOR, &ALICE, &BOB, Balance::from(10u32)));
        assert_eq!(
            Assets::free_balance(&XOR, &BOB).expect("Failed to query free balance."),
            Balance::from(10u32)
        );
        assert_ok!(Assets::burn_from(&XOR, &BOB, &BOB, Balance::from(10u32)));
        assert_eq!(
            Assets::free_balance(&XOR, &BOB).expect("Failed to query free balance."),
            Balance::from(0u32)
        );
    })
}

#[test]
fn should_update_balance_correctly() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_ok!(Assets::register_asset_id(
            ALICE,
            XOR,
            AssetSymbol(b"XOR".to_vec()),
            AssetName(b"SORA".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::from(10u32),
            true,
            None,
            None,
        ));
        assert_ok!(Assets::update_balance(RuntimeOrigin::root(), BOB, XOR, 100));
        assert_eq!(
            Assets::free_balance(&XOR, &BOB).expect("Failed to query free balance."),
            Balance::from(100u32)
        );

        assert_ok!(Assets::update_balance(RuntimeOrigin::root(), BOB, XOR, -10));
        assert_eq!(
            Assets::free_balance(&XOR, &BOB).expect("Failed to query free balance."),
            Balance::from(90u32)
        );

        assert_err!(
            Assets::update_balance(RuntimeOrigin::signed(ALICE), BOB, XOR, -10),
            BadOrigin
        );
        assert_eq!(
            Assets::free_balance(&XOR, &BOB).expect("Failed to query free balance."),
            Balance::from(90u32)
        );

        assert_noop!(
            Assets::update_balance(RuntimeOrigin::root(), BOB, XOR, -100),
            pallet_balances::Error::<Runtime>::InsufficientBalance
        );
        assert_eq!(
            Assets::free_balance(&XOR, &BOB).expect("Failed to query free balance."),
            Balance::from(90u32)
        );
    })
}

#[test]
fn should_register_indivisible() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let next_asset_id = Assets::gen_asset_id(&ALICE);
        assert_ok!(Assets::register(
            RuntimeOrigin::signed(ALICE),
            AssetSymbol(b"ALIC".to_vec()),
            AssetName(b"ALICE".to_vec()),
            5,
            true,
            true,
            None,
            None,
        ));
        let (_, _, precision, ..) = Assets::asset_infos(next_asset_id);
        assert_eq!(precision, 0u8);
    })
}

#[test]
fn should_associate_content_source() {
    let mut ext = ExtBuilder::default().build();
    let content_src = ContentSource(b"https://imgur.com/gallery/24O4LUX".to_vec());
    ext.execute_with(|| {
        assert_ok!(Assets::register_asset_id(
            ALICE,
            XOR,
            AssetSymbol(b"XOR".to_vec()),
            AssetName(b"SORA".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::from(10u32),
            true,
            Some(content_src.clone()),
            None,
        ));
        assert_eq!(Assets::get_asset_content_src(&XOR), Some(content_src));
    })
}

#[test]
fn should_fail_content_source() {
    let mut ext = ExtBuilder::default().build();
    let source: Vec<u8> = vec![0; ASSET_CONTENT_SOURCE_MAX_LENGTH + 1];
    let content_src = ContentSource(source);
    ext.execute_with(|| {
        assert_err!(
            Assets::register_asset_id(
                ALICE,
                XOR,
                AssetSymbol(b"XOR".to_vec()),
                AssetName(b"SORA".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                Balance::from(10u32),
                true,
                Some(content_src.clone()),
                None,
            ),
            Error::<Runtime>::InvalidContentSource
        );
    })
}

#[test]
fn should_associate_desciption() {
    let mut ext = ExtBuilder::default().build();
    let desc = Description(b"Lorem ipsum".to_vec());
    ext.execute_with(|| {
        assert_ok!(Assets::register_asset_id(
            ALICE,
            XOR,
            AssetSymbol(b"XOR".to_vec()),
            AssetName(b"SORA".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::from(10u32),
            true,
            None,
            Some(desc.clone()),
        ));
        assert_eq!(Assets::get_asset_description(&XOR), Some(desc));
    })
}

#[test]
fn should_fail_description() {
    let mut ext = ExtBuilder::default().build();
    let text: Vec<u8> = vec![0; ASSET_DESCRIPTION_MAX_LENGTH + 1];
    let desc = Description(text);
    ext.execute_with(|| {
        assert_err!(
            Assets::register_asset_id(
                ALICE,
                XOR,
                AssetSymbol(b"XOR".to_vec()),
                AssetName(b"SORA".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                Balance::from(10u32),
                true,
                None,
                Some(desc.clone()),
            ),
            Error::<Runtime>::InvalidDescription
        );
    })
}

#[test]
fn buy_back_and_burn_should_be_performed() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let xst_balance = balance!(1000);
        Assets::register_asset_id(
            MOCK_LIQUIDITY_PROXY_TECH_ACCOUNT,
            XST,
            AssetSymbol(b"XST".to_vec()),
            AssetName(b"Sora Synthetics".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            xst_balance,
            true,
            None,
            None,
        )
        .expect("Failed to register XST asset");

        let xst_total = Tokens::total_issuance(XST);
        // Just a sanity check, not a real test
        assert_eq!(xst_total, xst_balance);

        let pswap_balance = balance!(10);
        Assets::register_asset_id(
            ALICE,
            PSWAP,
            AssetSymbol(b"PSWAP".to_vec()),
            AssetName(b"Polkaswap".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            pswap_balance,
            true,
            None,
            None,
        )
        .expect("Failed to register PSWAP asset");

        let amount_to_mint = balance!(100);
        Assets::force_mint(RuntimeOrigin::root(), PSWAP, ALICE, amount_to_mint)
            .expect("Failed to mint PSWAP");

        let pswap_balance_after = Assets::free_balance(&PSWAP, &ALICE)
            .expect("Failed to query PSWAP free balance after mint.");
        assert_eq!(
            pswap_balance_after,
            pswap_balance + (amount_to_mint * 9 / 10)
        );

        // Same as `Assets::free_balance(&XST, &MOCK_LIQUIDITY_PROXY_TECH_ACCOUNT)`,
        // but it better represents the meaning of buy-back and burning
        let xst_total_after = Tokens::total_issuance(XST);

        // Since `MockLiquidityProxy` exchanges 1 to 1
        // there is no need to calculate PSWAP-XST swap-rate
        assert_eq!(xst_total_after, xst_total - (amount_to_mint / 10));
    })
}

#[test]
fn test_update_asset_info() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        // To be able to assert events
        frame_system::Pallet::<Runtime>::set_block_number(1);
        assert_ok!(Assets::register_asset_id(
            ALICE,
            XOR,
            AssetSymbol(b"XOR".to_vec()),
            AssetName(b"SORA".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::zero(),
            true,
            None,
            None,
        ));

        let val_symbol = AssetSymbol(b"VAL".to_vec());
        let val_name = AssetName(b"VAL Token".to_vec());
        assert_ok!(Assets::update_info(
            RuntimeOrigin::root(),
            XOR,
            Some(val_symbol.clone()),
            Some(val_name.clone())
        ));
        frame_system::Pallet::<Runtime>::assert_last_event(
            Event::AssetUpdated(XOR, Some(val_symbol.clone()), Some(val_name.clone())).into(),
        );
        assert_eq!(
            Assets::asset_infos(XOR),
            (
                val_symbol.clone(),
                val_name,
                DEFAULT_BALANCE_PRECISION,
                true,
                None,
                None
            )
        );

        let pswap_symbol = AssetSymbol(b"PSWAP".to_vec());
        let pswap_name = AssetName(b"Polkaswap".to_vec());
        assert_ok!(Assets::update_info(
            RuntimeOrigin::root(),
            XOR,
            None,
            Some(pswap_name.clone())
        ));
        frame_system::Pallet::<Runtime>::assert_last_event(
            Event::AssetUpdated(XOR, None, Some(pswap_name.clone())).into(),
        );
        assert_eq!(
            Assets::asset_infos(XOR),
            (
                val_symbol,
                pswap_name.clone(),
                DEFAULT_BALANCE_PRECISION,
                true,
                None,
                None
            )
        );

        assert_ok!(Assets::update_info(
            RuntimeOrigin::root(),
            XOR,
            Some(pswap_symbol.clone()),
            None
        ));
        frame_system::Pallet::<Runtime>::assert_last_event(
            Event::AssetUpdated(XOR, Some(pswap_symbol.clone()), None).into(),
        );
        assert_eq!(
            Assets::asset_infos(XOR),
            (
                pswap_symbol,
                pswap_name,
                DEFAULT_BALANCE_PRECISION,
                true,
                None,
                None
            )
        );

        assert_noop!(
            Assets::update_info(
                RuntimeOrigin::root(),
                XOR,
                Some(AssetSymbol(b"Dai".to_vec())),
                None
            ),
            Error::<Runtime>::InvalidAssetSymbol
        );
        assert_noop!(
            Assets::update_info(
                RuntimeOrigin::root(),
                XOR,
                None,
                Some(AssetName(b"D@I".to_vec()))
            ),
            Error::<Runtime>::InvalidAssetName
        );
        assert_noop!(
            Assets::update_info(
                RuntimeOrigin::root(),
                XOR,
                None,
                Some(AssetName(b"".to_vec()))
            ),
            Error::<Runtime>::InvalidAssetName
        );
        assert_noop!(
            Assets::update_info(
                RuntimeOrigin::root(),
                XOR,
                Some(AssetSymbol(b"".to_vec())),
                None
            ),
            Error::<Runtime>::InvalidAssetSymbol
        );
        assert_eq!(
            Assets::asset_infos(XOR),
            (
                AssetSymbol(b"PSWAP".to_vec()),
                AssetName(b"Polkaswap".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                true,
                None,
                None
            )
        );
        assert_noop!(
            Assets::update_info(
                RuntimeOrigin::root(),
                DAI,
                Some(AssetSymbol(b"DAI".to_vec())),
                Some(AssetName(b"DAI stablecoin".to_vec()))
            ),
            Error::<Runtime>::AssetIdNotExists
        );
    });
}
