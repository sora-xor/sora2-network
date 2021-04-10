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

mod tests {
    use crate::mock::*;
    use crate::Error;
    use common::prelude::{AssetName, AssetSymbol, Balance};
    use common::{AssetId32, DOT, VAL, XOR};
    use frame_support::{assert_err, assert_noop, assert_ok};
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
                Origin::signed(ALICE),
                AssetSymbol(b"ALIC".to_vec()),
                AssetName(b"ALICE".to_vec()),
                Balance::zero(),
                true,
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
                18,
                Balance::zero(),
                true,
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
                18,
                Balance::zero(),
                true,
            ));
            assert_noop!(
                Assets::register_asset_id(
                    ALICE,
                    XOR,
                    AssetSymbol(b"XOR".to_vec()),
                    AssetName(b"SORA".to_vec()),
                    18,
                    Balance::zero(),
                    true,
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
                    18,
                    Balance::zero(),
                    true,
                ),
                Error::<Runtime>::InvalidAssetName
            );

            assert_err!(
                Assets::register_asset_id(
                    ALICE,
                    VAL,
                    AssetSymbol(b"VAL".to_vec()),
                    AssetName(b"This is a name with $ymbols".to_vec()),
                    18,
                    Balance::zero(),
                    true,
                ),
                Error::<Runtime>::InvalidAssetName
            );

            assert_err!(
                Assets::register_asset_id(
                    ALICE,
                    DOT,
                    AssetSymbol(b"DOT".to_vec()),
                    AssetName(b"This is a name with _".to_vec()),
                    18,
                    Balance::zero(),
                    true,
                ),
                Error::<Runtime>::InvalidAssetName
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
                18,
                Balance::zero(),
                true,
            ));
            assert_ok!(Assets::mint_to(&XOR, &ALICE, &ALICE, 100u32.into()));
            assert_ok!(Assets::burn_from(&XOR, &ALICE, &ALICE, 100u32.into()));
            assert_ok!(Assets::update_balance(&XOR, &ALICE, 100.into()));
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
                18,
                Balance::zero(),
                true,
            ));
            assert_noop!(
                Assets::mint_to(&XOR, &BOB, &BOB, 100u32.into()),
                permissions::Error::<Runtime>::Forbidden
            );
            assert_noop!(
                Assets::update_balance(&XOR, &BOB, 100u32.into()),
                permissions::Error::<Runtime>::Forbidden
            );
        });
    }

    #[test]
    fn should_check_symbols_correctly() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert!(crate::is_symbol_valid(&AssetSymbol(b"XOR".to_vec())));
            assert!(crate::is_symbol_valid(&AssetSymbol(b"DOT".to_vec())));
            assert!(crate::is_symbol_valid(&AssetSymbol(b"KSM".to_vec())));
            assert!(crate::is_symbol_valid(&AssetSymbol(b"USDT".to_vec())));
            assert!(crate::is_symbol_valid(&AssetSymbol(b"VAL".to_vec())));
            assert!(crate::is_symbol_valid(&AssetSymbol(b"PSWAP".to_vec())));
            assert!(crate::is_symbol_valid(&AssetSymbol(b"GT".to_vec())));
            assert!(crate::is_symbol_valid(&AssetSymbol(b"BP".to_vec())));

            assert!(!crate::is_symbol_valid(&AssetSymbol(b"ABCDEFGH".to_vec())));
            assert!(!crate::is_symbol_valid(&AssetSymbol(b"AB1".to_vec())));
            assert!(!crate::is_symbol_valid(&AssetSymbol(
                b"\xF0\x9F\x98\xBF".to_vec()
            )));
        })
    }

    #[test]
    fn should_check_names_correctly() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert!(crate::is_name_valid(&AssetName(b"XOR".to_vec())));
            assert!(crate::is_name_valid(&AssetName(b"DOT".to_vec())));
            assert!(crate::is_name_valid(&AssetName(b"KSM".to_vec())));
            assert!(crate::is_name_valid(&AssetName(b"USDT".to_vec())));
            assert!(crate::is_name_valid(&AssetName(b"VAL".to_vec())));
            assert!(crate::is_name_valid(&AssetName(b"PSWAP".to_vec())));
            assert!(crate::is_name_valid(&AssetName(b"GT".to_vec())));
            assert!(crate::is_name_valid(&AssetName(b"BP".to_vec())));
            assert!(crate::is_name_valid(&AssetName(
                b"SORA Validator Token".to_vec()
            )));
            assert!(crate::is_name_valid(&AssetName(b"AB1".to_vec())));

            assert!(!crate::is_name_valid(&AssetName(
                b"This is a name with length over thirty three".to_vec()
            )));
            assert!(!crate::is_name_valid(&AssetName(b"AB1_".to_vec())));
            assert!(!crate::is_name_valid(&AssetName(
                b"\xF0\x9F\x98\xBF".to_vec()
            )));
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
                18,
                Balance::from(123u32),
                true,
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
                18,
                Balance::from(321u32),
                false,
            ));
            assert_eq!(
                Assets::free_balance(&VAL, &ALICE).expect("Failed to query free balance."),
                Balance::from(321u32),
            );
            assert_ok!(Assets::register_asset_id(
                ALICE,
                DOT,
                AssetSymbol(b"DOT".to_vec()),
                AssetName(b"Polkadot".to_vec()),
                18,
                Balance::from(0u32),
                false,
            ));
            assert_eq!(
                Assets::free_balance(&DOT, &ALICE).expect("Failed to query free balance."),
                Balance::zero(),
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
                18,
                Balance::from(10u32),
                false,
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
                Assets::update_balance(&XOR, &ALICE, 1i128),
                Error::<Runtime>::AssetSupplyIsNotMintable
            );
            assert_ok!(Assets::update_balance(&XOR, &ALICE, 0i128),);
            assert_ok!(Assets::update_balance(&XOR, &ALICE, -1i128),);
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
                18,
                Balance::from(10u32),
                true,
            ));
            assert_ok!(Assets::mint_to(&XOR, &ALICE, &ALICE, Balance::from(10u32)),);
            assert_ok!(Assets::mint_to(&XOR, &ALICE, &BOB, Balance::from(10u32)),);
            assert_ok!(Assets::update_balance(&XOR, &ALICE, 1i128),);
            assert_ok!(Assets::update_balance(&XOR, &ALICE, 0i128),);
            assert_ok!(Assets::update_balance(&XOR, &ALICE, -1i128),);

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
                Assets::update_balance(&XOR, &ALICE, 1i128),
                Error::<Runtime>::AssetSupplyIsNotMintable
            );
            assert_ok!(Assets::update_balance(&XOR, &ALICE, 0i128),);
            assert_ok!(Assets::update_balance(&XOR, &ALICE, -1i128),);
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
                18,
                Balance::from(10u32),
                true,
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
                18,
                Balance::from(10u32),
                true,
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
                18,
                Balance::from(10u32),
                true,
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
                18,
                Balance::from(10u32),
                true,
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
}
