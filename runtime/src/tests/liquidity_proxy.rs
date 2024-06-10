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

use crate::{AccountId, AssetId, Currencies, LiquidityProxy, PoolXYK, Runtime, RuntimeOrigin};
use common::mock::alice;
use common::prelude::SwapAmount;
use common::{balance, Balance, ETH, KXOR, XOR};
use frame_support::{assert_noop, assert_ok};
use framenode_chain_spec::ext;
use traits::MultiCurrency;

pub fn ensure_balances(account_id: AccountId, assets: Vec<(AssetId, Balance)>) {
    for (asset_id, expected_balance) in assets {
        let balance = Currencies::free_balance(asset_id, &account_id);
        assert_eq!(
            balance,
            expected_balance,
            "asset_id: {:?}, balance: {}, expected: {}",
            asset_id,
            balance as f64 / 1e18f64,
            expected_balance as f64 / 1e18f64
        );
    }
}

#[test]
fn chameleon_pool_swaps() {
    ext().execute_with(|| {
        common::test_utils::init_logger();
        for asset_id in vec![XOR, KXOR, ETH] {
            assert_ok!(Currencies::update_balance(
                RuntimeOrigin::root(),
                alice(),
                asset_id,
                balance!(1000000000) as i128
            ));
        }

        assert_ok!(PoolXYK::initialize_pool(
            RuntimeOrigin::signed(alice()),
            0,
            XOR,
            ETH
        ));

        assert_ok!(PoolXYK::deposit_liquidity(
            RuntimeOrigin::signed(alice()),
            0,
            XOR,
            ETH,
            balance!(10000000),
            balance!(1500),
            1,
            1
        ));

        let (pool_account, _) = PoolXYK::properties(XOR, ETH).unwrap();

        ensure_balances(
            alice(),
            vec![
                (ETH, balance!(999998500)),
                (KXOR, balance!(1000000000)),
                (XOR, balance!(990000000)),
            ],
        );
        ensure_balances(
            pool_account.clone(),
            vec![
                (ETH, balance!(1500)),
                (KXOR, balance!(0)),
                (XOR, balance!(10000000)),
            ],
        );

        assert_eq!(
            PoolXYK::reserves(XOR, ETH),
            (balance!(10000000), balance!(1500))
        );

        assert_ok!(LiquidityProxy::swap(
            RuntimeOrigin::signed(alice()),
            0,
            KXOR,
            ETH,
            SwapAmount::with_desired_input(balance!(10000), 1),
            vec![],
            common::FilterMode::Disabled
        ));

        ensure_balances(
            alice(),
            vec![
                (ETH, balance!(999998501.494010471559854824)),
                (KXOR, balance!(999990000)),
                (XOR, balance!(990000000)),
            ],
        );

        ensure_balances(
            pool_account.clone(),
            vec![
                (ETH, balance!(1498.505989528440145176)),
                (KXOR, balance!(9970)),
                (XOR, balance!(10000000)),
            ],
        );

        assert_eq!(
            PoolXYK::reserves(XOR, ETH),
            (balance!(10009970), balance!(1498.505989528440145176))
        );

        assert_noop!(
            LiquidityProxy::swap(
                RuntimeOrigin::signed(alice()),
                0,
                ETH,
                KXOR,
                SwapAmount::with_desired_input(balance!(10), 1),
                vec![],
                common::FilterMode::Disabled
            ),
            tokens::Error::<Runtime>::BalanceTooLow
        );

        assert_ok!(LiquidityProxy::swap(
            RuntimeOrigin::signed(alice()),
            0,
            ETH,
            KXOR,
            SwapAmount::with_desired_input(balance!(1), 1),
            vec![],
            common::FilterMode::Disabled
        ));

        ensure_balances(
            alice(),
            vec![
                (ETH, balance!(999998500.494010471559854824)),
                (KXOR, balance!(999996655.485312958609580453)),
                (XOR, balance!(990000000)),
            ],
        );

        ensure_balances(
            pool_account.clone(),
            vec![
                (ETH, balance!(1499.505989528440145176)),
                (KXOR, balance!(3294.488151495878053708)),
                (XOR, balance!(10000000)),
            ],
        );

        assert_eq!(
            PoolXYK::reserves(XOR, ETH),
            (
                balance!(10003294.488151495878053708),
                balance!(1499.505989528440145176)
            )
        );

        assert_ok!(PoolXYK::deposit_liquidity(
            RuntimeOrigin::signed(alice()),
            0,
            KXOR,
            ETH,
            balance!(100000),
            balance!(15),
            1,
            1
        ));

        ensure_balances(
            alice(),
            vec![
                (ETH, balance!(999998485.503889054015411849)),
                (KXOR, balance!(999896655.485312958609580453)),
                (XOR, balance!(990000000)),
            ],
        );

        ensure_balances(
            pool_account.clone(),
            vec![
                (ETH, balance!(1514.496110945984588151)),
                (KXOR, balance!(103294.488151495878053708)),
                (XOR, balance!(10000000)),
            ],
        );

        assert_eq!(
            PoolXYK::reserves(XOR, ETH),
            (
                balance!(10103294.488151495878053708),
                balance!(1514.496110945984588151)
            )
        );

        assert_eq!(
            PoolXYK::pool_providers(pool_account.clone(), alice()),
            Some(balance!(123698.828652689522840255)),
        );

        assert_noop!(
            PoolXYK::withdraw_liquidity(
                RuntimeOrigin::signed(alice()),
                0,
                KXOR,
                ETH,
                balance!(100000),
                1,
                1
            ),
            tokens::Error::<Runtime>::BalanceTooLow
        );

        assert_ok!(PoolXYK::withdraw_liquidity(
            RuntimeOrigin::signed(alice()),
            0,
            KXOR,
            ETH,
            balance!(100),
            1,
            1
        ));

        ensure_balances(
            alice(),
            vec![
                (ETH, balance!(999998486.728230567546029783)),
                (KXOR, balance!(999904823.141060547473196686)),
                (XOR, balance!(990000000)),
            ],
        );

        ensure_balances(
            pool_account.clone(),
            vec![
                (ETH, balance!(1513.271769432453970217)),
                (KXOR, balance!(95126.832403907014437475)),
                (XOR, balance!(10000000)),
            ],
        );

        assert_eq!(
            PoolXYK::reserves(XOR, ETH),
            (
                balance!(10095126.832403907014437475),
                balance!(1513.271769432453970217)
            )
        );
    });
}
