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
use common::prelude::Balance;
use common::{AssetInfoProvider, AssetName, AssetSymbol, DEXId, DEFAULT_BALANCE_PRECISION};
use frame_support::assert_ok;
use orml_traits::MultiCurrency;
use PolySwapActionExample::*;

#[test]
fn should_register_technical_account() {
    let mut ext = ExtBuilder::default().build();
    let tech_account_id = common::TechAccountId::Generic("Test123".into(), "Some data".into());
    let t01 = crate::Pallet::<Runtime>::tech_account_id_to_account_id(&tech_account_id).unwrap();

    ext.execute_with(|| {
        assert_ok!(Technical::register_tech_account_id(TechAccountId::Generic(
            "Test123".into(),
            "Some data".into()
        )));
        assert_eq!(
            crate::Pallet::<Runtime>::lookup_tech_account_id(&t01).unwrap(),
            tech_account_id
        );
    });
}

#[test]
fn generic_pair_swap_simple() {
    let mut ext = ExtBuilder::default().build();
    let dex = DEXId::Polkaswap;
    let t01 = common::TechAccountId::Pure(
        dex,
        XykLiquidityKeeper(TradingPair {
            base_asset_id: common::mock::ComicAssetId::RedPepper.into(),
            target_asset_id: common::mock::ComicAssetId::BlackPepper.into(),
        }),
    );
    let repr: AccountId = Technical::tech_account_id_to_account_id(&t01).unwrap();
    let a01 = RedPepper();
    let a02 = BlackPepper();
    let mut s01 = GenericPair(GenericPairSwapActionExample {
        give_minted: false,
        give_asset: a01,
        give_amount: 330_000u32.into(),
        take_burn: false,
        take_asset: a02,
        take_amount: 1000_000u32.into(),
        take_account: t01.clone(),
    });
    ext.execute_with(|| {
        assert_ok!(Technical::register_tech_account_id(t01));
        assert_ok!(assets::Pallet::<Runtime>::register_asset_id(
            get_alice(),
            RedPepper(),
            AssetSymbol(b"RP".to_vec()),
            AssetName(b"Red Pepper".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::from(0u32),
            true,
            None,
            None,
        ));
        assert_ok!(assets::Pallet::<Runtime>::register_asset_id(
            repr.clone(),
            BlackPepper(),
            AssetSymbol(b"BP".to_vec()),
            AssetName(b"Black Pepper".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::from(0u32),
            true,
            None,
            None,
        ));
        assert_ok!(assets::Pallet::<Runtime>::mint_to(
            &RedPepper(),
            &get_alice(),
            &get_alice(),
            9000_000u32.into()
        ));
        assert_ok!(assets::Pallet::<Runtime>::mint_to(
            &BlackPepper(),
            &repr,
            &repr,
            9000_000u32.into()
        ));
        assert_eq!(
            assets::Pallet::<Runtime>::free_balance(&a01, &get_alice()).unwrap(),
            9099000u32.into()
        );
        assert_eq!(
            assets::Pallet::<Runtime>::free_balance(&a02, &get_alice()).unwrap(),
            2000000u32.into()
        );
        assert_eq!(
            assets::Pallet::<Runtime>::free_balance(&a01, &repr).unwrap(),
            0
        );
        assert_eq!(
            assets::Pallet::<Runtime>::free_balance(&a02, &repr).unwrap(),
            9000000u32.into()
        );
        Technical::create_swap(get_alice(), &mut s01, &RedPepper()).unwrap();
        assert_eq!(
            assets::Pallet::<Runtime>::free_balance(&a01, &get_alice()).unwrap(),
            8769000u32.into()
        );
        assert_eq!(
            assets::Pallet::<Runtime>::free_balance(&a02, &get_alice()).unwrap(),
            3000000u32.into()
        );
        assert_eq!(
            assets::Pallet::<Runtime>::free_balance(&a02, &repr).unwrap(),
            8000000u32.into()
        );
        assert_eq!(
            assets::Pallet::<Runtime>::free_balance(&a01, &repr).unwrap(),
            330000u32.into()
        );
    });
}

#[test]
fn should_have_same_nonce_on_dust_tech_account() {
    let mut ext = ExtBuilder::default().build();
    let tech_account_id = common::TechAccountId::Generic("Test123".into(), "Some data".into());
    let t01 = crate::Pallet::<Runtime>::tech_account_id_to_account_id(&tech_account_id).unwrap();

    ext.execute_with(|| {
        assert_ok!(Technical::register_tech_account_id(TechAccountId::Generic(
            "Test123".into(),
            "Some data".into()
        )));
        assert_eq!(
            crate::Pallet::<Runtime>::lookup_tech_account_id(&t01).unwrap(),
            tech_account_id
        );
        frame_system::Pallet::<Runtime>::inc_account_nonce(&t01);
        assert_eq!(frame_system::Pallet::<Runtime>::account_nonce(&t01), 1);
        tokens::Pallet::<Runtime>::deposit(RedPepper(), &t01, 1u32.into()).unwrap();
        // Would remove the account if its providers count were 0.
        tokens::Pallet::<Runtime>::withdraw(RedPepper(), &t01, 1u32.into()).unwrap();
        assert_eq!(frame_system::Pallet::<Runtime>::account_nonce(&t01), 1);
    });
}
