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

use frame_support::{assert_noop, assert_ok};

use common::{
    balance, Balance, DEXId, FixedWrapper256, RewardReason, TradingPair, DOT, PSWAP, VAL, XOR,
    XSTUSD,
};
use frame_support::__private::log::debug;
use frame_system;
use frame_system::RawOrigin;
use pool_xyk::{PoolProviders, Properties, TotalIssuances};
use sp_runtime::traits::BadOrigin;
use std::collections::BTreeMap;
use vested_rewards::{Rewards, TotalRewards};

use crate::mock::{
    self, run_to_block, AssetId, ExtBuilder, Runtime, RuntimeOrigin, ALICE, BOB, CHARLIE, DAVE,
    DEX_A_ID, DEX_B_ID, EVE, REFRESH_FREQUENCY, VESTING_FREQUENCY,
};
use crate::{Event, Pallet, PoolFarmer, PoolFarmers};

type System = frame_system::Pallet<Runtime>;

fn init_pool(dex_id: DEXId, base_asset: AssetId, other_asset: AssetId) {
    assert_ok!(trading_pair::Pallet::<Runtime>::register(
        RuntimeOrigin::signed(BOB()),
        dex_id,
        base_asset,
        other_asset
    ));

    assert_ok!(pool_xyk::Pallet::<Runtime>::initialize_pool(
        RuntimeOrigin::signed(BOB()),
        dex_id,
        base_asset,
        other_asset,
    ));
}

#[test]
fn get_account_weight_handles_large_base_asset_amount() {
    ExtBuilder::default().build().execute_with(|| {
        let trading_pair = TradingPair {
            base_asset_id: XOR,
            target_asset_id: XSTUSD,
        };
        let base_asset_amount = i128::MAX as Balance + 1;

        let weight = Pallet::<Runtime>::get_account_weight(
            &trading_pair,
            FixedWrapper256::from(balance!(1)),
            base_asset_amount,
            balance!(1),
            balance!(1),
        )
        .unwrap();

        assert_eq!(weight, base_asset_amount);
    });
}

#[test]
fn get_account_weight_rejects_doubling_overflow() {
    ExtBuilder::default().build().execute_with(|| {
        let trading_pair = TradingPair {
            base_asset_id: XOR,
            target_asset_id: DOT,
        };

        let err = Pallet::<Runtime>::get_account_weight(
            &trading_pair,
            FixedWrapper256::from(balance!(1)),
            Balance::MAX / 2 + 1,
            balance!(1),
            balance!(1),
        )
        .unwrap_err();

        assert_eq!(err, crate::Error::<Runtime>::ArithmeticError.into());
    });
}

#[test]
fn get_account_weight_rejects_multiplier_overflow() {
    ExtBuilder::default().build().execute_with(|| {
        let trading_pair = TradingPair {
            base_asset_id: XOR,
            target_asset_id: XSTUSD,
        };

        let err = Pallet::<Runtime>::get_account_weight(
            &trading_pair,
            FixedWrapper256::from(Balance::MAX),
            balance!(2),
            balance!(1),
            balance!(1),
        )
        .unwrap_err();

        assert_eq!(err, crate::Error::<Runtime>::ArithmeticError.into());
    });
}

#[test]
fn prepare_account_rewards_handles_large_weights() {
    ExtBuilder::default().build().execute_with(|| {
        let mut accounts = BTreeMap::new();
        accounts.insert(ALICE(), FixedWrapper256::from(i128::MAX as Balance + 1));

        let rewards = Pallet::<Runtime>::prepare_account_rewards(accounts).unwrap();

        assert!(rewards.get(&ALICE()).copied().unwrap_or_default() > 0);
    });
}

#[test]
fn prepare_account_rewards_rejects_zero_total_weight() {
    ExtBuilder::default().build().execute_with(|| {
        let mut accounts = BTreeMap::new();
        accounts.insert(ALICE(), FixedWrapper256::from(0));
        accounts.insert(BOB(), FixedWrapper256::from(0));

        let err = Pallet::<Runtime>::prepare_account_rewards(accounts).unwrap_err();

        assert_eq!(err, crate::Error::<Runtime>::ArithmeticError.into());
    });
}

#[test]
fn vest_account_rewards_rejects_zero_weights_without_creating_rewards() {
    ExtBuilder::default().build().execute_with(|| {
        let mut accounts = BTreeMap::new();
        accounts.insert(ALICE(), FixedWrapper256::from(0));
        accounts.insert(BOB(), FixedWrapper256::from(0));

        let err = Pallet::<Runtime>::vest_account_rewards(accounts).unwrap_err();

        assert_eq!(err, crate::Error::<Runtime>::ArithmeticError.into());
        assert!(!Rewards::<Runtime>::contains_key(ALICE()));
        assert!(!Rewards::<Runtime>::contains_key(BOB()));
        assert_eq!(TotalRewards::<Runtime>::get(), 0);
    });
}

#[test]
fn vest_account_rewards_rolls_back_if_pending_reward_overflows() {
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(vested_rewards::Pallet::<Runtime>::add_pending_reward(
            &BOB(),
            RewardReason::LiquidityProvisionFarming,
            Balance::MAX,
        ));
        let bob_before = Rewards::<Runtime>::get(BOB());
        TotalRewards::<Runtime>::put(0);

        let mut accounts = BTreeMap::new();
        accounts.insert(ALICE(), FixedWrapper256::from(balance!(1)));
        accounts.insert(BOB(), FixedWrapper256::from(balance!(1)));

        let err = Pallet::<Runtime>::vest_account_rewards(accounts).unwrap_err();

        assert_eq!(
            err,
            vested_rewards::Error::<Runtime>::ArithmeticError.into()
        );
        assert!(!Rewards::<Runtime>::contains_key(ALICE()));
        assert_eq!(Rewards::<Runtime>::get(BOB()), bob_before);
        assert_eq!(TotalRewards::<Runtime>::get(), 0);
    });
}

#[test]
fn refresh_pool_skips_bad_provider_and_updates_valid_provider() {
    ExtBuilder::default().build().execute_with(|| {
        init_pool(DEX_A_ID, XOR, DOT);
        let pool = Properties::<Runtime>::get(XOR, DOT).unwrap().0;

        assert_ok!(assets::Pallet::<Runtime>::mint_unchecked(
            &XOR,
            &pool,
            Balance::MAX / 2 + 1
        ));
        assert_ok!(assets::Pallet::<Runtime>::mint_unchecked(
            &DOT,
            &pool,
            balance!(1)
        ));
        TotalIssuances::<Runtime>::insert(&pool, 3);
        PoolProviders::<Runtime>::insert(&pool, ALICE(), 3);
        PoolProviders::<Runtime>::insert(&pool, BOB(), 1);
        PoolFarmers::<Runtime>::insert(
            &pool,
            vec![PoolFarmer {
                account: ALICE(),
                block: 1,
                weight: 1,
            }],
        );

        let read_count = Pallet::<Runtime>::refresh_pool(pool.clone(), REFRESH_FREQUENCY);

        assert_eq!(read_count, 2);
        let farmers = PoolFarmers::<Runtime>::get(pool);
        assert_eq!(farmers.len(), 1);
        assert_eq!(farmers[0].account, BOB());
        assert_eq!(farmers[0].block, REFRESH_FREQUENCY);
        assert!(farmers[0].weight > 0);
    });
}

#[test]
fn refresh_pool_clears_stale_farmers_on_pool_level_failure() {
    ExtBuilder::default().build().execute_with(|| {
        let pool = ALICE();
        PoolFarmers::<Runtime>::insert(
            &pool,
            vec![PoolFarmer {
                account: BOB(),
                block: 1,
                weight: balance!(1),
            }],
        );

        let read_count = Pallet::<Runtime>::refresh_pool(pool.clone(), REFRESH_FREQUENCY);

        assert_eq!(read_count, 0);
        assert!(PoolFarmers::<Runtime>::get(pool).is_empty());
    });
}

// Checks that accounts that have more than 1 XOR are automatically added to farming each
// REFRESH_FREQUENCY blocks. Also, checks that accounts that no longer have 1 XOR are removed from farming.
#[test]
fn test() {
    let dex_id = DEX_A_ID;
    ExtBuilder::default().build().execute_with(|| {
        // Check default value for lp_min_xor_for_bonus_reward
        assert_eq!(
            <Pallet<Runtime>>::lp_min_xor_for_bonus_reward(),
            balance!(3000000)
        );
        // Update lp_min_xor_for_bonus_reward
        <Pallet<Runtime>>::set_lp_min_xor_for_bonus_reward(RawOrigin::Root.into(), balance!(1))
            .unwrap();

        // Check lp_min_xor_for_bonus_reward updated
        assert_eq!(
            <Pallet<Runtime>>::lp_min_xor_for_bonus_reward(),
            balance!(1)
        );

        init_pool(DEX_A_ID, XOR, DOT);
        init_pool(DEX_A_ID, XOR, PSWAP);
        init_pool(DEX_A_ID, XOR, XSTUSD);
        init_pool(DEX_B_ID, XSTUSD, VAL);
        init_pool(DEX_B_ID, XSTUSD, PSWAP);

        let xor_dot_pool = Properties::<Runtime>::get(XOR, DOT).unwrap().0;
        debug!("xor_dot_pool: {}", xor_dot_pool);
        let xor_pswap_pool = Properties::<Runtime>::get(XOR, PSWAP).unwrap().0;
        debug!("xor_pswap_pool: {}", xor_pswap_pool);
        let xor_xstusd_pool = Properties::<Runtime>::get(XOR, XSTUSD).unwrap().0;
        debug!("xor_xstusd_pool: {}", xor_xstusd_pool);
        let xstusd_val_pool = Properties::<Runtime>::get(XSTUSD, VAL).unwrap().0;
        debug!("xstusd_val_pool: {}", xstusd_val_pool);
        let xstusd_pswap_pool = Properties::<Runtime>::get(XSTUSD, PSWAP).unwrap().0;
        debug!("xstusd_pswap_pool: {}", xstusd_pswap_pool);

        debug!("alice: {}", ALICE());
        debug!("bob: {}", BOB());
        debug!("charlie: {}", CHARLIE());
        debug!("dave: {}", DAVE());
        debug!("eve: {}", EVE());

        // Add liquidity before the first refresh
        {
            assert_ok!(pool_xyk::Pallet::<Runtime>::deposit_liquidity(
                RuntimeOrigin::signed(EVE()),
                DEX_A_ID,
                XOR,
                XSTUSD,
                balance!(10),
                balance!(30),
                balance!(10),
                balance!(30),
            ));

            assert_ok!(pool_xyk::Pallet::<Runtime>::deposit_liquidity(
                RuntimeOrigin::signed(EVE()),
                DEX_B_ID,
                XSTUSD,
                VAL,
                balance!(3.3),
                balance!(0.5),
                balance!(3.3),
                balance!(0.5),
            ));

            assert_ok!(pool_xyk::Pallet::<Runtime>::deposit_liquidity(
                RuntimeOrigin::signed(ALICE()),
                DEX_A_ID,
                XOR,
                DOT,
                balance!(1.1),
                balance!(4.4),
                balance!(1.1),
                balance!(4.4),
            ));

            assert_ok!(pool_xyk::Pallet::<Runtime>::deposit_liquidity(
                RuntimeOrigin::signed(BOB()),
                DEX_A_ID,
                XOR,
                DOT,
                balance!(1.1),
                balance!(4.4),
                balance!(1.1),
                balance!(4.4),
            ));

            assert_ok!(pool_xyk::Pallet::<Runtime>::deposit_liquidity(
                RuntimeOrigin::signed(ALICE()),
                DEX_A_ID,
                XOR,
                PSWAP,
                balance!(1.1),
                balance!(4.4),
                balance!(1.1),
                balance!(4.4),
            ));

            assert_ok!(pool_xyk::Pallet::<Runtime>::deposit_liquidity(
                RuntimeOrigin::signed(ALICE()),
                DEX_B_ID,
                XSTUSD,
                VAL,
                balance!(3.3),
                balance!(0.5),
                balance!(3.3),
                balance!(0.5),
            ));

            assert_ok!(pool_xyk::Pallet::<Runtime>::deposit_liquidity(
                RuntimeOrigin::signed(DAVE()),
                DEX_B_ID,
                XSTUSD,
                PSWAP,
                balance!(3.3),
                balance!(20),
                balance!(3.3),
                balance!(20),
            ));
        }

        mock::run_to_block(REFRESH_FREQUENCY);

        // Check that after the first refresh both Alice and Bob are farmers
        {
            // double reward for DOT
            let farmers = PoolFarmers::<Runtime>::get(&xor_dot_pool);
            assert_eq!(
                farmers,
                vec![
                    PoolFarmer {
                        account: ALICE(),
                        block: REFRESH_FREQUENCY,
                        weight: balance!(2.199999999999999500),
                    },
                    PoolFarmer {
                        account: BOB(),
                        block: REFRESH_FREQUENCY,
                        weight: balance!(2.200000000000000500),
                    }
                ]
            );

            let farmers = PoolFarmers::<Runtime>::get(&xor_pswap_pool);
            assert_eq!(
                farmers,
                vec![PoolFarmer {
                    account: ALICE(),
                    block: REFRESH_FREQUENCY,
                    weight: balance!(2.2),
                },]
            );

            let farmers = PoolFarmers::<Runtime>::get(&xstusd_pswap_pool);
            assert_eq!(
                farmers,
                vec![PoolFarmer {
                    account: DAVE(),
                    block: REFRESH_FREQUENCY,
                    weight: balance!(2.275862068965517242),
                }]
            );

            let farmers = PoolFarmers::<Runtime>::get(&xstusd_val_pool);
            assert_eq!(
                farmers,
                vec![
                    PoolFarmer {
                        account: ALICE(),
                        block: REFRESH_FREQUENCY,
                        weight: balance!(2.275862068965518128),
                    },
                    PoolFarmer {
                        account: EVE(),
                        block: REFRESH_FREQUENCY,
                        weight: balance!(2.275862068965516358),
                    },
                ]
            );

            let farmers = PoolFarmers::<Runtime>::get(&xor_xstusd_pool);
            assert_eq!(
                farmers,
                vec![PoolFarmer {
                    account: EVE(),
                    block: REFRESH_FREQUENCY,
                    weight: balance!(10),
                },]
            );
        }

        // Remove Alice and add Charlie before the second refresh
        assert_ok!(pool_xyk::Pallet::<Runtime>::withdraw_liquidity(
            RuntimeOrigin::signed(ALICE()),
            dex_id,
            XOR,
            PSWAP,
            balance!(1),
            balance!(0.1),
            balance!(0.1),
        ));
        assert_ok!(pool_xyk::Pallet::<Runtime>::deposit_liquidity(
            RuntimeOrigin::signed(CHARLIE()),
            dex_id,
            XOR,
            PSWAP,
            balance!(10),
            balance!(40),
            balance!(5),
            balance!(5),
        ));

        mock::run_to_block(REFRESH_FREQUENCY * 2);

        // Check that after the second refresh Alice, Bob and Charlie are farmers
        {
            // double reward for DOT
            let farmers = PoolFarmers::<Runtime>::get(&xor_dot_pool);
            assert_eq!(
                farmers,
                vec![
                    PoolFarmer {
                        account: ALICE(),
                        block: REFRESH_FREQUENCY,
                        weight: balance!(2.199999999999999500),
                    },
                    PoolFarmer {
                        account: BOB(),
                        block: REFRESH_FREQUENCY,
                        weight: balance!(2.200000000000000500),
                    }
                ]
            );

            let farmers = PoolFarmers::<Runtime>::get(&xor_pswap_pool);
            assert_eq!(
                farmers,
                vec![PoolFarmer {
                    account: CHARLIE(),
                    block: REFRESH_FREQUENCY * 2,
                    weight: balance!(20.000000000000000942),
                },]
            );
        }

        // Add Alice
        assert_ok!(pool_xyk::Pallet::<Runtime>::deposit_liquidity(
            RuntimeOrigin::signed(ALICE()),
            dex_id,
            XOR,
            PSWAP,
            balance!(10.1),
            balance!(40.4),
            balance!(1.1),
            balance!(4.4),
        ));

        mock::run_to_block(VESTING_FREQUENCY);

        // TODO #886: fix magic numbers, use some formulae in comments or explicitly in code

        let alice_reward = *Rewards::<Runtime>::get(&ALICE())
            .rewards
            .get(&RewardReason::LiquidityProvisionFarming)
            .unwrap();
        assert_eq!(alice_reward, balance!(157125.633737642270157117));

        let bob_reward = *Rewards::<Runtime>::get(&BOB())
            .rewards
            .get(&RewardReason::LiquidityProvisionFarming)
            .unwrap();
        assert_eq!(bob_reward, balance!(37993.673033658488758777));

        let charlie_reward = *Rewards::<Runtime>::get(&CHARLIE())
            .rewards
            .get(&RewardReason::LiquidityProvisionFarming)
            .unwrap();
        assert_eq!(charlie_reward, balance!(176843.278120301293662255));

        let dave_reward = *Rewards::<Runtime>::get(&DAVE())
            .rewards
            .get(&RewardReason::LiquidityProvisionFarming)
            .unwrap();
        assert_eq!(dave_reward, balance!(39303.799689991531173596));

        let eve_reward = *Rewards::<Runtime>::get(&EVE())
            .rewards
            .get(&RewardReason::LiquidityProvisionFarming)
            .unwrap();
        assert_eq!(eve_reward, balance!(212002.313479348243748252));

        assert_ok!(pool_xyk::Pallet::<Runtime>::deposit_liquidity(
            RuntimeOrigin::signed(ALICE()),
            dex_id,
            XOR,
            DOT,
            balance!(0.5),
            balance!(2),
            balance!(0.3),
            balance!(0.5),
        ));

        assert_ok!(pool_xyk::Pallet::<Runtime>::withdraw_liquidity(
            RuntimeOrigin::signed(BOB()),
            dex_id,
            XOR,
            DOT,
            balance!(1.5),
            balance!(0.5),
            balance!(2),
        ));

        run_to_block(VESTING_FREQUENCY + REFRESH_FREQUENCY);

        // double reward for DOT
        let farmers = PoolFarmers::<Runtime>::get(&xor_dot_pool);
        assert_eq!(
            farmers,
            vec![PoolFarmer {
                account: ALICE(),
                block: REFRESH_FREQUENCY,
                weight: balance!(3.199999999999999822),
            }]
        );

        debug!("second vesting");

        run_to_block(VESTING_FREQUENCY + VESTING_FREQUENCY);

        let alice_reward = *Rewards::<Runtime>::get(&ALICE())
            .rewards
            .get(&RewardReason::LiquidityProvisionFarming)
            .unwrap();
        assert_eq!(alice_reward, balance!(386271.068658756425841800));

        // BOB's rewards didn't change
        let bob_reward = *Rewards::<Runtime>::get(&BOB())
            .rewards
            .get(&RewardReason::LiquidityProvisionFarming)
            .unwrap();
        assert_eq!(bob_reward, balance!(37993.673033658488758777));

        let charlie_reward = *Rewards::<Runtime>::get(&CHARLIE())
            .rewards
            .get(&RewardReason::LiquidityProvisionFarming)
            .unwrap();
        assert_eq!(charlie_reward, balance!(377066.713911616265057332));

        let dave_reward = *Rewards::<Runtime>::get(&DAVE())
            .rewards
            .get(&RewardReason::LiquidityProvisionFarming)
            .unwrap();
        assert_eq!(dave_reward, balance!(69629.365104687832138789));

        let eve_reward = *Rewards::<Runtime>::get(&EVE())
            .rewards
            .get(&RewardReason::LiquidityProvisionFarming)
            .unwrap();
        assert_eq!(eve_reward, balance!(375576.575413164643203297));
    });
}

#[test]
fn set_lp_min_xor_for_bonus_reward_should_forbid_for_non_root_call() {
    ExtBuilder::default().build().execute_with(|| {
        assert_noop!(
            Pallet::<Runtime>::set_lp_min_xor_for_bonus_reward(
                RuntimeOrigin::signed(ALICE()),
                balance!(100000)
            ),
            BadOrigin
        );
    });
}

#[test]
fn set_lp_min_xor_for_bonus_reward_should_work() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(1);
        let modified_min_xor = balance!(3 * (10_i32.pow(6)));
        let old_lp_min_xor_for_bonus_reward = Pallet::<Runtime>::lp_min_xor_for_bonus_reward();
        assert_ok!(Pallet::<Runtime>::set_lp_min_xor_for_bonus_reward(
            RawOrigin::Root.into(),
            modified_min_xor
        ));
        let new_lp_min_xor_for_bonus_reward = Pallet::<Runtime>::lp_min_xor_for_bonus_reward();
        assert_eq!(new_lp_min_xor_for_bonus_reward, modified_min_xor);
        System::assert_has_event(
            Event::LpMinXorForBonusRewardUpdated {
                new_lp_min_xor_for_bonus_reward,
                old_lp_min_xor_for_bonus_reward,
            }
            .into(),
        );
    });
}
