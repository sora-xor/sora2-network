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
use common::mock::{alice, bob};
use common::prelude::{FixedWrapper, SwapAmount};
use common::{balance, Balance, XykPool, ETH, KXOR, XOR};
use frame_support::{assert_noop, assert_ok};
use framenode_chain_spec::ext;
use pool_xyk::to_fixed_wrapper;
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
            balance!(5000000),
            balance!(750),
            1,
            1
        ));

        assert_ok!(PoolXYK::deposit_liquidity(
            RuntimeOrigin::signed(alice()),
            0,
            KXOR,
            ETH,
            balance!(5000000),
            balance!(750),
            1,
            1
        ));

        let (pool_account, _) = PoolXYK::properties(XOR, ETH).unwrap();

        ensure_balances(
            alice(),
            vec![
                (ETH, balance!(999998500)),
                (KXOR, balance!(995000000)),
                (XOR, balance!(995000000)),
            ],
        );
        ensure_balances(
            pool_account.clone(),
            vec![
                (ETH, balance!(1500)),
                (KXOR, balance!(5000000)),
                (XOR, balance!(5000000)),
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
                (KXOR, balance!(994990000)),
                (XOR, balance!(995000000)),
            ],
        );

        ensure_balances(
            pool_account.clone(),
            vec![
                (ETH, balance!(1498.505989528440145176)),
                (KXOR, balance!(5000000)),
                (XOR, balance!(5000000)),
            ],
        );

        assert_eq!(
            PoolXYK::reserves(XOR, ETH),
            (balance!(10000000), balance!(1498.505989528440145176))
        );

        assert_noop!(
            LiquidityProxy::swap(
                RuntimeOrigin::signed(alice()),
                0,
                ETH,
                KXOR,
                SwapAmount::with_desired_input(balance!(1500), 1),
                vec![],
                common::FilterMode::Disabled
            ),
            pool_xyk::Error::<Runtime>::NotEnoughOutputReserves
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
                (KXOR, balance!(994996648.856403124694260275)),
                (XOR, balance!(995000000)),
            ],
        );

        ensure_balances(
            pool_account.clone(),
            vec![
                (ETH, balance!(1499.505989528440145176)),
                (KXOR, balance!(4993331.137007899002747968)),
                (XOR, balance!(5000000)),
            ],
        );

        assert_eq!(
            PoolXYK::reserves(XOR, ETH),
            (
                balance!(9993331.137007899002747968),
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
                (ETH, balance!(999998485.494010471559854824)),
                (KXOR, balance!(994896682.622122020856878393)),
                (XOR, balance!(995000000)),
            ],
        );

        ensure_balances(
            pool_account.clone(),
            vec![
                (ETH, balance!(1514.505989528440145176)),
                (KXOR, balance!(5093297.371289002840129850)),
                (XOR, balance!(5000000)),
            ],
        );

        assert_eq!(
            PoolXYK::reserves(XOR, ETH),
            (
                balance!(10093297.371289002840129850),
                balance!(1514.505989528440145176)
            )
        );

        assert_eq!(
            PoolXYK::pool_providers(pool_account.clone(), alice()),
            Some(balance!(123699.635501297234447430)),
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
                (ETH, balance!(999998486.718351985090472758)),
                (KXOR, balance!(994904842.142827466391296893)),
                (XOR, balance!(995000000)),
            ],
        );

        ensure_balances(
            pool_account.clone(),
            vec![
                (ETH, balance!(1513.281648014909527242)),
                (KXOR, balance!(5085137.850583557305711350)),
                (XOR, balance!(5000000)),
            ],
        );

        assert_eq!(
            PoolXYK::reserves(XOR, ETH),
            (
                balance!(10085137.850583557305711350),
                balance!(1513.281648014909527242)
            )
        );
    });
}

#[test]
fn chameleon_pool_swaps_burn_kxor() {
    ext().execute_with(|| {
        common::test_utils::init_logger();
        for asset_id in vec![XOR, KXOR, ETH] {
            for account_id in vec![alice(), bob()] {
                assert_ok!(Currencies::update_balance(
                    RuntimeOrigin::root(),
                    account_id,
                    asset_id,
                    balance!(1000000000) as i128
                ));
            }
        }

        assert_ok!(PoolXYK::initialize_pool(
            RuntimeOrigin::signed(alice()),
            0,
            XOR,
            ETH
        ));

        for account_id in vec![alice(), bob()] {
            assert_ok!(PoolXYK::deposit_liquidity(
                RuntimeOrigin::signed(account_id),
                0,
                KXOR,
                ETH,
                balance!(5000000),
                balance!(750),
                1,
                1
            ));
        }

        let (pool_account, _) = PoolXYK::properties(XOR, ETH).unwrap();

        ensure_balances(
            alice(),
            vec![(ETH, balance!(999999250)), (KXOR, balance!(995000000))],
        );

        ensure_balances(
            bob(),
            vec![(ETH, balance!(999999250)), (KXOR, balance!(995000000))],
        );

        ensure_balances(
            pool_account.clone(),
            vec![
                (ETH, balance!(1500)),
                (XOR, balance!(0)),
                (KXOR, balance!(10000000)),
            ],
        );

        assert_eq!(
            PoolXYK::reserves(XOR, ETH),
            (balance!(10000000), balance!(1500))
        );

        assert_eq!(
            PoolXYK::pool_providers(pool_account.clone(), alice()),
            Some(balance!(61237.243569579452452030)),
        );

        assert_eq!(
            PoolXYK::pool_providers(pool_account.clone(), bob()),
            Some(balance!(61237.243569579452453030)),
        );

        assert_eq!(
            PoolXYK::total_issuance(&pool_account).unwrap(),
            balance!(122474.487139158904905060),
        );

        assert_ok!(LiquidityProxy::swap(
            RuntimeOrigin::signed(alice()),
            0,
            KXOR,
            ETH,
            SwapAmount::with_desired_input(balance!(10000000), 1),
            vec![],
            common::FilterMode::Disabled
        ));

        ensure_balances(
            alice(),
            vec![
                (ETH, balance!(999999998.873309964947421131)),
                (KXOR, balance!(985000000)),
            ],
        );

        ensure_balances(
            bob(),
            vec![(ETH, balance!(999999250)), (KXOR, balance!(995000000))],
        );

        ensure_balances(
            pool_account.clone(),
            vec![
                (ETH, balance!(751.126690035052578869)),
                (XOR, balance!(0)),
                (KXOR, balance!(10000000)),
            ],
        );

        assert_eq!(
            PoolXYK::reserves(XOR, ETH),
            (balance!(10000000), balance!(751.126690035052578869))
        );

        assert_eq!(
            PoolXYK::pool_providers(pool_account.clone(), alice()),
            Some(balance!(61237.243569579452452030)),
        );

        assert_eq!(
            PoolXYK::pool_providers(pool_account.clone(), bob()),
            Some(balance!(61237.243569579452453030)),
        );

        assert_eq!(
            PoolXYK::total_issuance(&pool_account).unwrap(),
            balance!(122474.487139158904905060),
        );

        assert_ok!(LiquidityProxy::swap(
            RuntimeOrigin::signed(alice()),
            0,
            KXOR,
            ETH,
            SwapAmount::with_desired_input(balance!(10000000), 1),
            vec![],
            common::FilterMode::Disabled
        ));

        ensure_balances(
            alice(),
            vec![
                (ETH, balance!(1000000373.872463677990696610)),
                (KXOR, balance!(975000000)),
                (XOR, balance!(1000000000)),
            ],
        );

        ensure_balances(
            bob(),
            vec![(ETH, balance!(999999250)), (KXOR, balance!(995000000))],
        );

        ensure_balances(
            pool_account.clone(),
            vec![
                (ETH, balance!(376.127536322009303390)),
                (XOR, balance!(0)),
                (KXOR, balance!(10000000)),
            ],
        );

        assert_eq!(
            PoolXYK::reserves(XOR, ETH),
            (balance!(10000000), balance!(376.127536322009303390))
        );

        let (reserve_x, reserve_y) = PoolXYK::reserves(XOR, ETH);
        let real_issuance = to_fixed_wrapper!(reserve_x)
            .multiply_and_sqrt(&to_fixed_wrapper!(reserve_y))
            .try_into_balance()
            .unwrap();
        assert_eq!(real_issuance, balance!(61329.237425718029497827));

        assert_eq!(
            PoolXYK::pool_providers(pool_account.clone(), alice()),
            Some(balance!(61237.243569579452452030)),
        );

        assert_eq!(
            PoolXYK::pool_providers(pool_account.clone(), bob()),
            Some(balance!(61237.243569579452453030)),
        );

        assert_eq!(
            PoolXYK::total_issuance(&pool_account).unwrap(),
            balance!(122474.487139158904905060),
        );

        assert_ok!(PoolXYK::withdraw_liquidity(
            RuntimeOrigin::signed(alice()),
            0,
            KXOR,
            ETH,
            balance!(61237.243569579452452030),
            1,
            1
        ));

        ensure_balances(
            alice(),
            vec![
                (ETH, balance!(1000000561.936231838995348301)),
                (KXOR, balance!(979999999.999999999999918350)),
                (XOR, balance!(1000000000)),
            ],
        );

        ensure_balances(
            bob(),
            vec![(ETH, balance!(999999250)), (KXOR, balance!(995000000))],
        );

        ensure_balances(
            pool_account.clone(),
            vec![
                (ETH, balance!(188.063768161004651699)),
                (XOR, balance!(0)),
                (KXOR, balance!(5000000.000000000000081650)),
            ],
        );

        assert_eq!(
            PoolXYK::reserves(XOR, ETH),
            (
                balance!(5000000.000000000000081650),
                balance!(188.063768161004651699)
            )
        );

        let (reserve_x, reserve_y) = PoolXYK::reserves(XOR, ETH);
        let real_issuance = to_fixed_wrapper!(reserve_x)
            .multiply_and_sqrt(&to_fixed_wrapper!(reserve_y))
            .try_into_balance()
            .unwrap();
        assert_eq!(real_issuance, balance!(30664.618712859014750133));

        assert_eq!(PoolXYK::pool_providers(pool_account.clone(), alice()), None);

        assert_eq!(
            PoolXYK::pool_providers(pool_account.clone(), bob()),
            Some(balance!(61237.243569579452453030)),
        );

        assert_eq!(
            PoolXYK::total_issuance(&pool_account).unwrap(),
            balance!(61237.243569579452453030),
        );

        assert_ok!(PoolXYK::deposit_liquidity(
            RuntimeOrigin::signed(alice()),
            0,
            KXOR,
            ETH,
            balance!(5000000),
            balance!(188.063768161004651699),
            1,
            1
        ));

        ensure_balances(
            alice(),
            vec![
                (ETH, balance!(1000000373.872463677990696606)),
                (KXOR, balance!(974999999.999999999999918350)),
                (XOR, balance!(1000000000)),
            ],
        );

        ensure_balances(
            bob(),
            vec![(ETH, balance!(999999250)), (KXOR, balance!(995000000))],
        );

        ensure_balances(
            pool_account.clone(),
            vec![
                (ETH, balance!(376.127536322009303394)),
                (XOR, balance!(0)),
                (KXOR, balance!(10000000.000000000000081650)),
            ],
        );

        assert_eq!(
            PoolXYK::reserves(XOR, ETH),
            (
                balance!(10000000.000000000000081650),
                balance!(376.127536322009303394)
            )
        );

        let (reserve_x, reserve_y) = PoolXYK::reserves(XOR, ETH);
        let real_issuance = to_fixed_wrapper!(reserve_x)
            .multiply_and_sqrt(&to_fixed_wrapper!(reserve_y))
            .try_into_balance()
            .unwrap();
        assert_eq!(real_issuance, balance!(61329.237425718029498079));

        assert_eq!(
            PoolXYK::pool_providers(pool_account.clone(), alice()),
            Some(balance!(61237.243569579452452727))
        );

        assert_eq!(
            PoolXYK::pool_providers(pool_account.clone(), bob()),
            Some(balance!(61237.243569579452453030)),
        );

        assert_eq!(
            PoolXYK::total_issuance(&pool_account).unwrap(),
            balance!(122474.487139158904905757),
        );
    });
}
