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
use common::mock::alice;
use common::prelude::*;
use common::{balance, fixed};

#[test]
fn test_provides_exchange_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        MockLiquiditySource::set_reserve(
            RuntimeOrigin::signed(alice()),
            DEX_A_ID,
            DOT,
            fixed!(5000),
            fixed!(7000),
        )
        .expect("Failed to set reserve.");
        assert!(MockLiquiditySource::can_exchange(
            &DEX_A_ID,
            &DOT,
            &GetBaseAssetId::get()
        ));
        assert!(MockLiquiditySource::can_exchange(
            &DEX_A_ID,
            &GetBaseAssetId::get(),
            &DOT
        ));
    });
}

#[test]
fn test_doesnt_provide_exchange_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert!(!MockLiquiditySource::can_exchange(
            &DEX_A_ID,
            &DOT,
            &GetBaseAssetId::get()
        ));
        assert!(!MockLiquiditySource::can_exchange(
            &DEX_A_ID,
            &GetBaseAssetId::get(),
            &DOT
        ));
        // check again, so they are not created via get()'s
        assert!(!MockLiquiditySource::can_exchange(
            &DEX_A_ID,
            &DOT,
            &GetBaseAssetId::get()
        ));
        assert!(!MockLiquiditySource::can_exchange(
            &DEX_A_ID,
            &GetBaseAssetId::get(),
            &DOT
        ));
    });
}

#[test]
fn test_support_multiple_dexes_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        MockLiquiditySource::set_reserve(
            RuntimeOrigin::signed(alice()),
            DEX_A_ID,
            DOT,
            fixed!(1000),
            fixed!(1000),
        )
        .expect("Failed to set reserve.");
        MockLiquiditySource::set_reserve(
            RuntimeOrigin::signed(alice()),
            DEX_B_ID,
            KSM,
            fixed!(1000),
            fixed!(1000),
        )
        .expect("Failed to set reserve.");
        assert!(MockLiquiditySource::can_exchange(
            &DEX_A_ID,
            &DOT,
            &GetBaseAssetId::get()
        ));
        assert!(!MockLiquiditySource::can_exchange(
            &DEX_A_ID,
            &KSM,
            &GetBaseAssetId::get()
        ));
        assert!(!MockLiquiditySource::can_exchange(
            &DEX_B_ID,
            &DOT,
            &GetBaseAssetId::get()
        ));
        assert!(MockLiquiditySource::can_exchange(
            &DEX_B_ID,
            &KSM,
            &GetBaseAssetId::get()
        ));
    });
}

#[test]
fn test_quote_base_to_target_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        MockLiquiditySource::set_reserve(
            RuntimeOrigin::signed(alice()),
            DEX_A_ID,
            DOT,
            fixed!(5000),
            fixed!(7000),
        )
        .expect("Failed to set reserve.");
        let (outcome, _) = MockLiquiditySource::quote(
            &DEX_A_ID,
            &GetBaseAssetId::get(),
            &DOT,
            QuoteAmount::with_desired_input(balance!(100)),
            true,
        )
        .unwrap();
        assert_eq!(outcome.amount, balance!(136.851187324744592819));
        let (outcome, _) = MockLiquiditySource::quote(
            &DEX_A_ID,
            &GetBaseAssetId::get(),
            &DOT,
            QuoteAmount::with_desired_output(balance!(136.851187324744592819)),
            true,
        )
        .unwrap();
        assert_eq!(outcome.amount, balance!(99.999999999999999999));
    });
}

#[test]
fn test_quote_target_to_base_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        MockLiquiditySource::set_reserve(
            RuntimeOrigin::signed(alice()),
            DEX_A_ID,
            DOT,
            fixed!(5000),
            fixed!(7000),
        )
        .expect("Failed to set reserve.");
        let (outcome, _) = MockLiquiditySource::quote(
            &DEX_A_ID,
            &DOT,
            &GetBaseAssetId::get(),
            QuoteAmount::with_desired_input(balance!(100)),
            true,
        )
        .unwrap();
        assert_eq!(outcome.amount, balance!(70.211267605633802817));
        let (outcome, _) = MockLiquiditySource::quote(
            &DEX_A_ID,
            &DOT,
            &GetBaseAssetId::get(),
            QuoteAmount::with_desired_output(balance!(70.211267605633802817)),
            true,
        )
        .unwrap();
        assert_eq!(outcome.amount, balance!(99.999999999999999999));
    });
}

#[test]
fn test_quote_target_to_target_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        MockLiquiditySource::set_reserve(
            RuntimeOrigin::signed(alice()),
            DEX_A_ID,
            DOT,
            fixed!(5000),
            fixed!(7000),
        )
        .expect("Failed to set reserve.");
        MockLiquiditySource::set_reserve(
            RuntimeOrigin::signed(alice()),
            DEX_A_ID,
            KSM,
            fixed!(5500),
            fixed!(3000),
        )
        .expect("Failed to set reserve.");
        let (outcome, _) = MockLiquiditySource::quote(
            &DEX_A_ID,
            &KSM,
            &DOT,
            QuoteAmount::with_desired_input(balance!(100)),
            true,
        )
        .unwrap();
        assert_eq!(outcome.amount, balance!(238.487257161165663484));
        let (outcome, _) = MockLiquiditySource::quote(
            &DEX_A_ID,
            &KSM,
            &DOT,
            QuoteAmount::with_desired_output(balance!(238.487257161165663484)),
            true,
        )
        .unwrap();
        assert_eq!(outcome.amount, balance!(100));
    });
}

#[test]
fn test_quote_different_modules_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        MockLiquiditySource::set_reserve(
            RuntimeOrigin::signed(alice()),
            DEX_A_ID,
            DOT,
            fixed!(5000),
            fixed!(7000),
        )
        .expect("Failed to set reserve.");
        MockLiquiditySource2::set_reserve(
            RuntimeOrigin::signed(alice()),
            DEX_A_ID,
            DOT,
            fixed!(5500),
            fixed!(3000),
        )
        .expect("Failed to set reserve.");
        let (outcome, _) = MockLiquiditySource::quote(
            &DEX_A_ID,
            &GetBaseAssetId::get(),
            &DOT,
            QuoteAmount::with_desired_input(balance!(100)),
            true,
        )
        .unwrap();
        assert_eq!(outcome.amount, balance!(136.851187324744592819));
        let (outcome, _) = MockLiquiditySource2::quote(
            &DEX_A_ID,
            &GetBaseAssetId::get(),
            &DOT,
            QuoteAmount::with_desired_input(balance!(100)),
            true,
        )
        .unwrap();
        assert_eq!(outcome.amount, balance!(53.413575727271103809));
    });
}
