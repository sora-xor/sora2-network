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

    use common::balance;
    use common::prelude::{AssetName, AssetSymbol, Balance};
    use common::{
        AssetId32, ContentSource, Description, IsValid, ASSET_CONTENT_SOURCE_MAX_LENGTH,
        ASSET_DESCRIPTION_MAX_LENGTH, DEFAULT_BALANCE_PRECISION, PSWAP, XOR,
    };
    use frame_support::assert_noop;
    use frame_support::error::BadOrigin;
    use frame_support::{assert_err, assert_ok};
    use framenode_chain_spec::ext;
    use framenode_runtime::{
        assets, frame_system, AccountId, AssetId, Assets, Origin, Runtime, Tokens,
    };
    use hex_literal::hex;
    use sp_runtime::traits::Zero;

    // pub const ALICE: AccountId = AccountId::new([1;32]);
    fn alice() -> AccountId {
        let account = AccountId::new([1; 32]);
        frame_system::Pallet::<Runtime>::inc_providers(&account);
        account
    }

    fn mock_liquidity_proxy_tech_account() -> AccountId {
        let account = AccountId::new([24; 32]);
        frame_system::Pallet::<Runtime>::inc_providers(&account);
        account
    }

    pub const BOB: AccountId = AccountId::new([2; 32]);
    pub const BUY_BACK_ACCOUNT: AccountId = AccountId::new([3; 32]);
    pub const MOCK_XOR: AssetId = AssetId32::from_bytes([100; 32]);
    pub const MOCK_VAL: AssetId = AssetId32::from_bytes([101; 32]);
    pub const MOCK_DOT: AssetId = AssetId32::from_bytes([102; 32]);
    pub const MOCK_XST: AssetId = AssetId32::from_bytes([103; 32]);

    type E = assets::Error<Runtime>;

    #[test]
    fn should_gen_and_register_asset() {
        ext().execute_with(|| {
            let next_asset_id = Assets::gen_asset_id(&alice());
            assert_eq!(
                next_asset_id,
                AssetId32::from_bytes(hex!(
                    "00ce7f334a5f1fbad2ab64bedaa9b85072968f6d7598f60bf4012dff66b16aa2"
                ))
            );
            assert!(Assets::ensure_asset_exists(&next_asset_id).is_err());
            assert_ok!(Assets::register(
                Origin::signed(alice()),
                AssetSymbol(b"ALIC".to_vec()),
                AssetName(b"ALICE".to_vec()),
                Balance::zero(),
                true,
                false,
                None,
                None,
            ));
            assert_ok!(Assets::ensure_asset_exists(&next_asset_id));
            assert_ne!(Assets::gen_asset_id(&alice()), next_asset_id);
        });
    }

    #[test]
    fn should_register_asset() {
        ext().execute_with(|| {
            assert!(Assets::ensure_asset_exists(&MOCK_XOR).is_err());
            assert_ok!(Assets::register_asset_id(
                alice(),
                MOCK_XOR,
                AssetSymbol(b"XOR".to_vec()),
                AssetName(b"SORA".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                Balance::zero(),
                true,
                None,
                None,
            ));
            assert_ok!(Assets::ensure_asset_exists(&MOCK_XOR));
        });
    }

    #[test]
    fn should_not_register_duplicated_asset() {
        ext().execute_with(|| {
            assert_ok!(Assets::register_asset_id(
                alice(),
                MOCK_XOR,
                AssetSymbol(b"XOR".to_vec()),
                AssetName(b"SORA".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                Balance::zero(),
                true,
                None,
                None,
            ));
            assert_eq!(
                Assets::register_asset_id(
                    alice(),
                    MOCK_XOR,
                    AssetSymbol(b"XOR".to_vec()),
                    AssetName(b"SORA".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    Balance::zero(),
                    true,
                    None,
                    None,
                ),
                Err(E::AssetIdAlreadyExists.into())
            );
        });
    }

    #[test]
    fn should_not_register_invalid_asset_name() {
        ext().execute_with(|| {
            assert_err!(
                Assets::register_asset_id(
                    alice(),
                    MOCK_XOR,
                    AssetSymbol(b"XOR".to_vec()),
                    AssetName(b"This is a name with length over thirty three".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    Balance::zero(),
                    true,
                    None,
                    None,
                ),
                E::InvalidAssetName
            );

            assert_err!(
                Assets::register_asset_id(
                    alice(),
                    MOCK_XOR,
                    AssetSymbol(b"XOR".to_vec()),
                    AssetName(b"".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    Balance::zero(),
                    true,
                    None,
                    None,
                ),
                E::InvalidAssetName
            );

            assert_err!(
                Assets::register_asset_id(
                    alice(),
                    MOCK_VAL,
                    AssetSymbol(b"VAL".to_vec()),
                    AssetName(b"This is a name with $ymbols".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    Balance::zero(),
                    true,
                    None,
                    None,
                ),
                E::InvalidAssetName
            );

            assert_err!(
                Assets::register_asset_id(
                    alice(),
                    MOCK_DOT,
                    AssetSymbol(b"DOT".to_vec()),
                    AssetName(b"This is a name with _".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    Balance::zero(),
                    true,
                    None,
                    None,
                ),
                E::InvalidAssetName
            );
        });
    }

    #[test]
    fn should_not_register_invalid_asset_symbol() {
        ext().execute_with(|| {
            assert_err!(
                Assets::register_asset_id(
                    alice(),
                    MOCK_XOR,
                    AssetSymbol(b"xor".to_vec()),
                    AssetName(b"Super Sora".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    Balance::zero(),
                    true,
                    None,
                    None,
                ),
                E::InvalidAssetSymbol
            );

            assert_err!(
                Assets::register_asset_id(
                    alice(),
                    MOCK_XOR,
                    AssetSymbol(b"".to_vec()),
                    AssetName(b"Super Sora".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    Balance::zero(),
                    true,
                    None,
                    None,
                ),
                E::InvalidAssetSymbol
            );

            assert_err!(
                Assets::register_asset_id(
                    alice(),
                    MOCK_VAL,
                    AssetSymbol(b"VAL IS SUPER LONG".to_vec()),
                    AssetName(b"Validator".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    Balance::zero(),
                    true,
                    None,
                    None,
                ),
                E::InvalidAssetSymbol
            );

            assert_err!(
                Assets::register_asset_id(
                    alice(),
                    MOCK_DOT,
                    AssetSymbol(b"D_OT".to_vec()),
                    AssetName(b"Bad Symbol".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    Balance::zero(),
                    true,
                    None,
                    None,
                ),
                E::InvalidAssetSymbol
            );
        });
    }

    #[test]
    fn should_allow_operation() {
        ext().execute_with(|| {
            assert_ok!(Assets::register_asset_id(
                alice(),
                MOCK_XOR,
                AssetSymbol(b"XOR".to_vec()),
                AssetName(b"SORA".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                Balance::zero(),
                true,
                None,
                None,
            ));
            assert_ok!(Assets::mint_to(
                &MOCK_XOR,
                &alice(),
                &alice(),
                100u32.into()
            ));
            assert_ok!(Assets::burn_from(
                &MOCK_XOR,
                &alice(),
                &alice(),
                100u32.into()
            ));
            assert_ok!(Assets::update_own_balance(&MOCK_XOR, &alice(), 100.into()));
        });
    }

    #[test]
    fn should_not_allow_operation() {
        ext().execute_with(|| {
            assert_ok!(Assets::register_asset_id(
                alice(),
                MOCK_XOR,
                AssetSymbol(b"XOR".to_vec()),
                AssetName(b"SORA".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                Balance::zero(),
                true,
                None,
                None,
            ));
            assert_noop!(
                Assets::mint_to(&MOCK_XOR, &BOB, &BOB, 100u32.into()),
                permissions::Error::<Runtime>::Forbidden
            );
            assert_noop!(
                Assets::update_own_balance(&MOCK_XOR, &BOB, 100u32.into()),
                permissions::Error::<Runtime>::Forbidden
            );
        });
    }

    #[test]
    fn should_check_symbols_correctly() {
        ext().execute_with(|| {
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
        ext().execute_with(|| {
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

            assert!(
                !AssetName(b"This is a name with length over thirty three".to_vec()).is_valid()
            );
            assert!(!AssetName(b"AB1_".to_vec()).is_valid());
            assert!(!AssetName(b"\xF0\x9F\x98\xBF".to_vec()).is_valid());
        })
    }

    #[test]
    fn should_mint_initial_supply_for_owner() {
        ext().execute_with(|| {
            assert_ok!(Assets::register_asset_id(
                alice(),
                MOCK_XOR,
                AssetSymbol(b"XOR".to_vec()),
                AssetName(b"SORA".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                Balance::from(123u32),
                true,
                None,
                None,
            ));
            assert_eq!(
                Assets::free_balance(&MOCK_XOR, &alice()).expect("Failed to query free balance."),
                Balance::from(123u32),
            );
            assert_ok!(Assets::register_asset_id(
                alice(),
                MOCK_VAL,
                AssetSymbol(b"VAL".to_vec()),
                AssetName(b"SORA Validator Token".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                Balance::from(321u32),
                false,
                None,
                None,
            ));
            assert_eq!(
                Assets::free_balance(&MOCK_VAL, &alice()).expect("Failed to query free balance."),
                Balance::from(321u32),
            );
        })
    }

    #[test]
    fn should_not_allow_dead_asset() {
        ext().execute_with(|| {
            assert_eq!(
                Assets::register_asset_id(
                    alice(),
                    MOCK_DOT,
                    AssetSymbol(b"DOT".to_vec()),
                    AssetName(b"Polkadot".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    Balance::from(0u32),
                    false,
                    None,
                    None,
                ),
                Err(E::DeadAsset.into())
            );
        })
    }

    #[test]
    fn should_fail_with_non_mintable_asset_supply() {
        ext().execute_with(|| {
            assert_ok!(Assets::register_asset_id(
                alice(),
                MOCK_XOR,
                AssetSymbol(b"XOR".to_vec()),
                AssetName(b"SORA".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                Balance::from(10u32),
                false,
                None,
                None,
            ));
            assert_eq!(
                Assets::mint_to(&MOCK_XOR, &alice(), &alice(), Balance::from(10u32)),
                Err(E::AssetSupplyIsNotMintable.into())
            );
            assert_eq!(
                Assets::mint_to(&MOCK_XOR, &alice(), &BOB, Balance::from(10u32)),
                Err(E::AssetSupplyIsNotMintable.into())
            );
            assert_eq!(
                Assets::update_own_balance(&MOCK_XOR, &alice(), 1i128),
                Err(E::AssetSupplyIsNotMintable.into())
            );
            assert_ok!(Assets::update_own_balance(&MOCK_XOR, &alice(), 0i128),);
            assert_ok!(Assets::update_own_balance(&MOCK_XOR, &alice(), -1i128),);
        })
    }

    #[test]
    fn should_mint_for_mintable_asset() {
        ext().execute_with(|| {
            assert_ok!(Assets::register_asset_id(
                alice(),
                MOCK_XOR,
                AssetSymbol(b"XOR".to_vec()),
                AssetName(b"SORA".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                Balance::from(10u32),
                true,
                None,
                None,
            ));
            assert_ok!(Assets::mint_to(
                &MOCK_XOR,
                &alice(),
                &alice(),
                Balance::from(10u32)
            ),);
            assert_ok!(Assets::mint_to(
                &MOCK_XOR,
                &alice(),
                &BOB,
                Balance::from(10u32)
            ),);
            assert_ok!(Assets::update_own_balance(&MOCK_XOR, &alice(), 1i128),);
            assert_ok!(Assets::update_own_balance(&MOCK_XOR, &alice(), 0i128),);
            assert_ok!(Assets::update_own_balance(&MOCK_XOR, &alice(), -1i128),);

            assert_eq!(
                Assets::set_non_mintable_from(&MOCK_XOR, &BOB),
                Err(E::InvalidAssetOwner.into())
            );
            assert_ok!(Assets::set_non_mintable_from(&MOCK_XOR, &alice()));

            assert_eq!(
                Assets::mint_to(&MOCK_XOR, &alice(), &alice(), Balance::from(10u32)),
                Err(E::AssetSupplyIsNotMintable.into())
            );
            assert_eq!(
                Assets::mint_to(&MOCK_XOR, &alice(), &BOB, Balance::from(10u32)),
                Err(E::AssetSupplyIsNotMintable.into())
            );
            assert_eq!(
                Assets::update_own_balance(&MOCK_XOR, &alice(), 1i128),
                Err(E::AssetSupplyIsNotMintable.into())
            );
            assert_ok!(Assets::update_own_balance(&MOCK_XOR, &alice(), 0i128),);
            assert_ok!(Assets::update_own_balance(&MOCK_XOR, &alice(), -1i128),);
        })
    }

    #[test]
    fn should_not_allow_duplicate_set_non_mintable() {
        ext().execute_with(|| {
            assert_ok!(Assets::register_asset_id(
                alice(),
                MOCK_XOR,
                AssetSymbol(b"XOR".to_vec()),
                AssetName(b"SORA".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                Balance::from(10u32),
                true,
                None,
                None,
            ));
            assert_ok!(Assets::set_non_mintable_from(&MOCK_XOR, &alice()));
            assert_eq!(
                Assets::set_non_mintable_from(&MOCK_XOR, &alice()),
                Err(E::AssetSupplyIsNotMintable.into())
            );
        })
    }

    #[test]
    fn should_burn_from() {
        ext().execute_with(|| {
            assert_ok!(Assets::register_asset_id(
                alice(),
                MOCK_XOR,
                AssetSymbol(b"XOR".to_vec()),
                AssetName(b"SORA".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                Balance::from(10u32),
                true,
                None,
                None,
            ));
            assert_eq!(
                Assets::free_balance(&MOCK_XOR, &alice()).expect("Failed to query free balance."),
                Balance::from(10u32),
            );
            assert_ok!(Assets::burn_from(
                &MOCK_XOR,
                &alice(),
                &alice(),
                Balance::from(10u32)
            ));
            assert_eq!(
                Assets::free_balance(&MOCK_XOR, &alice()).expect("Failed to query free balance."),
                Balance::from(0u32),
            );
        })
    }

    #[test]
    fn should_not_allow_burn_from_due_to_permissions() {
        ext().execute_with(|| {
            assert_ok!(Assets::register_asset_id(
                alice(),
                MOCK_XOR,
                AssetSymbol(b"XOR".to_vec()),
                AssetName(b"SORA".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                Balance::from(10u32),
                true,
                None,
                None,
            ));
            assert_noop!(
                Assets::burn_from(&MOCK_XOR, &BOB, &alice(), Balance::from(10u32)),
                permissions::Error::<Runtime>::Forbidden
            );
        })
    }

    #[test]
    fn should_allow_burn_from_self_without_a_permissions() {
        ext().execute_with(|| {
            assert_ok!(Assets::register_asset_id(
                alice(),
                MOCK_XOR,
                AssetSymbol(b"XOR".to_vec()),
                AssetName(b"SORA".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                Balance::from(10u32),
                true,
                None,
                None,
            ));
            assert_ok!(Assets::mint_to(
                &MOCK_XOR,
                &alice(),
                &BOB,
                Balance::from(10u32)
            ));
            assert_eq!(
                Assets::free_balance(&MOCK_XOR, &BOB).expect("Failed to query free balance."),
                Balance::from(10u32)
            );
            assert_ok!(Assets::burn_from(
                &MOCK_XOR,
                &BOB,
                &BOB,
                Balance::from(10u32)
            ));
            assert_eq!(
                Assets::free_balance(&MOCK_XOR, &BOB).expect("Failed to query free balance."),
                Balance::from(0u32)
            );
        })
    }

    #[test]
    fn should_update_balance_correctly() {
        ext().execute_with(|| {
            assert_ok!(Assets::register_asset_id(
                alice(),
                XOR,
                AssetSymbol(b"XOR".to_vec()),
                AssetName(b"SORA".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                Balance::from(10u32),
                true,
                None,
                None,
            )
            .or_else(|_| Assets::ensure_asset_exists(&XOR)));
            assert_ok!(Assets::update_balance(Origin::root(), BOB, XOR, 100));
            assert_eq!(
                Assets::free_balance(&XOR, &BOB).expect("Failed to query free balance."),
                Balance::from(100u32)
            );

            assert_ok!(Assets::update_balance(Origin::root(), BOB, XOR, -10));
            assert_eq!(
                Assets::free_balance(&XOR, &BOB).expect("Failed to query free balance."),
                Balance::from(90u32)
            );

            assert_err!(
                Assets::update_balance(Origin::signed(alice()), BOB, XOR, -10),
                BadOrigin
            );
            assert_eq!(
                Assets::free_balance(&XOR, &BOB).expect("Failed to query free balance."),
                Balance::from(90u32)
            );

            assert_noop!(
                Assets::update_balance(Origin::root(), BOB, XOR, -100),
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
        ext().execute_with(|| {
            let next_asset_id = Assets::gen_asset_id(&alice());
            assert_ok!(Assets::register(
                Origin::signed(alice()),
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
        let content_src = ContentSource(b"https://imgur.com/gallery/24O4LUX".to_vec());
        ext().execute_with(|| {
            assert_ok!(Assets::register_asset_id(
                alice(),
                MOCK_XOR,
                AssetSymbol(b"XOR".to_vec()),
                AssetName(b"SORA".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                Balance::from(10u32),
                true,
                Some(content_src.clone()),
                None,
            ));
            assert_eq!(Assets::get_asset_content_src(&MOCK_XOR), Some(content_src));
        })
    }

    #[test]
    fn should_fail_content_source() {
        let source: Vec<u8> = vec![0; ASSET_CONTENT_SOURCE_MAX_LENGTH + 1];
        let content_src = ContentSource(source);
        ext().execute_with(|| {
            assert_err!(
                Assets::register_asset_id(
                    alice(),
                    MOCK_XOR,
                    AssetSymbol(b"XOR".to_vec()),
                    AssetName(b"SORA".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    Balance::from(10u32),
                    true,
                    Some(content_src.clone()),
                    None,
                ),
                E::InvalidContentSource
            );
        })
    }

    #[test]
    fn should_associate_desciption() {
        let desc = Description(b"Lorem ipsum".to_vec());
        ext().execute_with(|| {
            assert_ok!(Assets::register_asset_id(
                alice(),
                MOCK_XOR,
                AssetSymbol(b"XOR".to_vec()),
                AssetName(b"SORA".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                Balance::from(10u32),
                true,
                None,
                Some(desc.clone()),
            ));
            assert_eq!(Assets::get_asset_description(&MOCK_XOR), Some(desc));
        })
    }

    #[test]
    fn should_fail_description() {
        let text: Vec<u8> = vec![0; ASSET_DESCRIPTION_MAX_LENGTH + 1];
        let desc = Description(text);
        ext().execute_with(|| {
            assert_err!(
                Assets::register_asset_id(
                    alice(),
                    MOCK_XOR,
                    AssetSymbol(b"XOR".to_vec()),
                    AssetName(b"SORA".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    Balance::from(10u32),
                    true,
                    None,
                    Some(desc.clone()),
                ),
                E::InvalidDescription
            );
        })
    }

    #[test]
    fn buy_back_and_burn_should_be_performed() {
        ext().execute_with(|| {
            let xst_balance = balance!(1000);
            Assets::register_asset_id(
                mock_liquidity_proxy_tech_account(),
                MOCK_XST,
                AssetSymbol(b"XST".to_vec()),
                AssetName(b"Sora Synthetics".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                xst_balance,
                true,
                None,
                None,
            )
            .expect("Failed to register XST asset");

            let xst_total = Tokens::total_issuance(MOCK_XST);
            // Just a sanity check, not a real test
            assert_eq!(xst_total, xst_balance);

            let pswap_balance = balance!(10);

            Assets::register_asset_id(
                alice(),
                PSWAP,
                AssetSymbol(b"PSWAP".to_vec()),
                AssetName(b"Polkaswap".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                pswap_balance,
                true,
                None,
                None,
            )
            .or_else(|_| Assets::ensure_asset_exists(&PSWAP))
            .expect("Failed to register PSWAP asset");

            let amount_to_mint = balance!(100);
            Assets::force_mint(Origin::root(), PSWAP, alice(), amount_to_mint)
                .expect("Failed to mint PSWAP");

            let pswap_balance_after = Assets::free_balance(&PSWAP, &alice())
                .expect("Failed to query PSWAP free balance after mint.");
            assert_eq!(
                pswap_balance_after,
                pswap_balance + (amount_to_mint * 9 / 10)
            );

            // Same as `Assets::free_balance(&MOCK_XST, &mock_liquidity_proxy_tech_account())`,
            // but it better represents the meaning of buy-back and burning
            let xst_total_after = Tokens::total_issuance(MOCK_XST);

            // Since `MockLiquidityProxy` exchanges 1 to 1
            // there is no need to calculate PSWAP-XST swap-rate
            assert_eq!(xst_total_after, xst_total - (amount_to_mint / 10));
        })
    }
}
