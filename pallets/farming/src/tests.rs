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

use frame_support::assert_ok;

use common::{balance, DOT, PSWAP, XOR};
use pool_xyk::Properties;

use crate::mock::{
    self, AssetId, ExtBuilder, Origin, Runtime, ALICE, BOB, CHARLIE, DEX_A_ID, REFRESH_FREQUENCY,
    VESTING_FREQUENCY,
};
use crate::{PoolFarmer, PoolFarmers, SavedValues};

fn init_pool(other_asset: AssetId) {
    assert_ok!(trading_pair::Module::<Runtime>::register(
        Origin::signed(BOB()),
        DEX_A_ID,
        XOR,
        other_asset
    ));

    assert_ok!(pool_xyk::Module::<Runtime>::initialize_pool(
        Origin::signed(BOB()),
        DEX_A_ID,
        XOR,
        other_asset,
    ));
}

// Checks that accounts that have more than 1 XOR are automatically added to farming each REFRESH_FREQUENCY blocks. Also, checks that accounts that no longer has 1 XOR are removed from farming.
#[test]
fn test() {
    let dex_id = DEX_A_ID;
    ExtBuilder::default().build().execute_with(|| {
        init_pool(DOT);
        init_pool(PSWAP);

        assert_ok!(pool_xyk::Module::<Runtime>::deposit_liquidity(
            Origin::signed(ALICE()),
            dex_id,
            XOR,
            DOT,
            balance!(1.1),
            balance!(4.4),
            balance!(1.1),
            balance!(4.4),
        ));

        assert_ok!(pool_xyk::Module::<Runtime>::deposit_liquidity(
            Origin::signed(BOB()),
            dex_id,
            XOR,
            DOT,
            balance!(1.1),
            balance!(4.4),
            balance!(1.1),
            balance!(4.4),
        ));

        assert_ok!(pool_xyk::Module::<Runtime>::deposit_liquidity(
            Origin::signed(ALICE()),
            dex_id,
            XOR,
            PSWAP,
            balance!(1.1),
            balance!(4.4),
            balance!(1.1),
            balance!(4.4),
        ));

        assert_ok!(pool_xyk::Module::<Runtime>::deposit_liquidity(
            Origin::signed(CHARLIE()),
            dex_id,
            XOR,
            PSWAP,
            balance!(1.1),
            balance!(4.4),
            balance!(1.1),
            balance!(4.4),
        ));

        mock::run_to_block(REFRESH_FREQUENCY);

        let dot_pool = Properties::<Runtime>::get(XOR, DOT).unwrap().0;
        let farmers = PoolFarmers::<Runtime>::get(&dot_pool);
        assert_eq!(
            farmers,
            vec![
                PoolFarmer {
                    account: ALICE(),
                    block: 200,
                    pool_tokens: balance!(2.199999999999998996),
                },
                PoolFarmer {
                    account: BOB(),
                    block: 200,
                    pool_tokens: balance!(2.199999999999999995),
                }
            ]
        );

        let pswap_pool = Properties::<Runtime>::get(XOR, PSWAP).unwrap().0;
        let farmers = PoolFarmers::<Runtime>::get(&pswap_pool);
        assert_eq!(
            farmers,
            vec![
                PoolFarmer {
                    account: ALICE(),
                    block: 200,
                    pool_tokens: balance!(2.199999999999998996),
                },
                PoolFarmer {
                    account: CHARLIE(),
                    block: 200,
                    pool_tokens: balance!(2.199999999999999995),
                }
            ]
        );

        mock::run_to_block(VESTING_FREQUENCY);

        // TBD: Remove for the next release
        let values = SavedValues::<Runtime>::get(VESTING_FREQUENCY);
        assert_eq!(
            values,
            vec![
                (
                    dot_pool,
                    vec![
                        (ALICE(), 200, balance!(1.099999999999999498)),
                        (BOB(), 200, balance!(1.099999999999999998)),
                    ]
                ),
                (
                    pswap_pool,
                    vec![
                        (ALICE(), 200, balance!(2.199999999999998996)),
                        (CHARLIE(), 200, balance!(2.199999999999999996)),
                    ]
                )
            ]
        );

        // TBD: Uncomment for the next release
        // let info = Rewards::<Runtime>::get(&ALICE());
        // assert_eq!(
        //     *info
        //         .rewards
        //         .get(&RewardReason::LiquidityProvisionFarming)
        //         .unwrap(),
        //     balance!(34626.038781163425878113)
        // );

        // let info = Rewards::<Runtime>::get(&BOB());
        // assert_eq!(
        //     *info
        //         .rewards
        //         .get(&RewardReason::LiquidityProvisionFarming)
        //         .unwrap(),
        //     balance!(34626.038781163441621885)
        // );

        // assert_ok!(pool_xyk::Module::<Runtime>::deposit_liquidity(
        //     Origin::signed(ALICE()),
        //     dex_id,
        //     XOR,
        //     DOT,
        //     balance!(0.5),
        //     balance!(2),
        //     balance!(0.3),
        //     balance!(0.5),
        // ));

        // assert_ok!(pool_xyk::Module::<Runtime>::withdraw_liquidity(
        //     Origin::signed(BOB()),
        //     dex_id,
        //     XOR,
        //     DOT,
        //     balance!(1.5),
        //     balance!(0.5),
        //     balance!(2),
        // ));

        // run_to_block(VESTING_FREQUENCY + REFRESH_FREQUENCY);

        // let farmers = PoolFarmers::<Runtime>::get(&pool_account);
        // assert_eq!(
        //     farmers,
        //     vec![
        //         PoolFarmer {
        //             account: ALICE(),
        //             block: 200,
        //             pool_tokens: balance!(3.199999999999998993),
        //         },
        //         PoolFarmer {
        //             account: BOB(),
        //             block: 200,
        //             pool_tokens: balance!(0.699999999999999995),
        //         }
        //     ]
        // );

        // run_to_block(VESTING_FREQUENCY + VESTING_FREQUENCY);

        // let info = Rewards::<Runtime>::get(&ALICE());
        // // ALICE received all PSWAP
        // assert_eq!(
        //     *info
        //         .rewards
        //         .get(&RewardReason::LiquidityProvisionFarming)
        //         .unwrap(),
        //     balance!(103878.116343490293378112)
        // );

        // let info = Rewards::<Runtime>::get(&BOB());
        // // BOB's rewards didn't change
        // assert_eq!(
        //     *info
        //         .rewards
        //         .get(&RewardReason::LiquidityProvisionFarming)
        //         .unwrap(),
        //     balance!(34626.038781163441621885)
        // );
    });
}
