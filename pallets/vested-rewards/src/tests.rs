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
use crate::{Error, MarketMakerInfo, RewardInfo};
use common::{
    balance, Balance, OnPswapBurned, PswapRemintInfo, RewardReason, VestedRewardsTrait, PSWAP,
};
use frame_support::assert_noop;
use sp_std::collections::btree_map::BTreeMap;
use traits::currency::MultiCurrency;

fn deposit_rewards_to_reserves(amount: Balance) {
    Currencies::deposit(PSWAP, &GetBondingCurveRewardsAccountId::get(), amount).unwrap();
    Currencies::deposit(PSWAP, &GetMarketMakerRewardsAccountId::get(), amount).unwrap();
}

#[test]
fn should_add_market_maker_infos_single_user() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_eq!(
            VestedRewards::market_makers_registry(&alice()),
            MarketMakerInfo {
                count: 0,
                volume: balance!(0)
            }
        );

        // first add
        VestedRewards::update_market_maker_records(&alice(), balance!(123), 1).unwrap();
        let expected_1 = MarketMakerInfo {
            count: 1,
            volume: balance!(123),
        };
        assert_eq!(VestedRewards::market_makers_registry(&alice()), expected_1);

        // second add
        VestedRewards::update_market_maker_records(&alice(), balance!(123), 1).unwrap();
        let expected_2 = MarketMakerInfo {
            count: 2,
            volume: balance!(246),
        };
        assert_eq!(
            VestedRewards::market_makers_registry(&alice()),
            expected_2.clone()
        );

        // add with less than 1 xor
        VestedRewards::update_market_maker_records(&alice(), balance!(0.9), 1).unwrap();
        assert_eq!(VestedRewards::market_makers_registry(&alice()), expected_2);

        // add with multiplier
        VestedRewards::update_market_maker_records(&alice(), balance!(123), 2).unwrap();
        let expected_3 = MarketMakerInfo {
            count: 4,
            volume: balance!(492),
        };
        assert_eq!(VestedRewards::market_makers_registry(&alice()), expected_3);
    });
}

#[test]
fn should_add_market_maker_infos_multiple_users() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        VestedRewards::update_market_maker_records(&alice(), balance!(111), 1).unwrap();
        VestedRewards::update_market_maker_records(&bob(), balance!(111), 2).unwrap();
        VestedRewards::update_market_maker_records(&eve(), balance!(111), 3).unwrap();
        assert_eq!(
            VestedRewards::market_makers_registry(&alice()),
            MarketMakerInfo {
                count: 1,
                volume: balance!(111)
            }
        );
        assert_eq!(
            VestedRewards::market_makers_registry(&bob()),
            MarketMakerInfo {
                count: 2,
                volume: balance!(222)
            }
        );
        assert_eq!(
            VestedRewards::market_makers_registry(&eve()),
            MarketMakerInfo {
                count: 3,
                volume: balance!(333)
            }
        );
    });
}

#[test]
fn migration_v0_1_0_to_v0_2_0() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        use crate::{Rewards as VRewards, TotalRewards as VTotalRewards};
        use multicollateral_bonding_curve_pool::Rewards as MBCRewards;

        MBCRewards::<Runtime>::insert(alice(), (balance!(0.5), balance!(100)));
        MBCRewards::<Runtime>::insert(bob(), (balance!(0), balance!(10)));
        MBCRewards::<Runtime>::insert(eve(), (balance!(0.5), balance!(1)));

        assert_eq!(
            MBCRewards::<Runtime>::get(alice()),
            (balance!(0.5), balance!(100))
        );
        assert_eq!(
            MBCRewards::<Runtime>::get(bob()),
            (balance!(0), balance!(10))
        );
        assert_eq!(
            MBCRewards::<Runtime>::get(eve()),
            (balance!(0.5), balance!(1))
        );

        assert_eq!(
            VRewards::<Runtime>::get(alice()),
            crate::RewardInfo {
                limit: 0,
                total_available: 0,
                rewards: BTreeMap::new()
            }
        );
        assert_eq!(
            VRewards::<Runtime>::get(bob()),
            crate::RewardInfo {
                limit: 0,
                total_available: 0,
                rewards: BTreeMap::new()
            }
        );
        assert_eq!(
            VRewards::<Runtime>::get(eve()),
            crate::RewardInfo {
                limit: 0,
                total_available: 0,
                rewards: BTreeMap::new()
            }
        );

        crate::migration::migrate_rewards_from_tbc::<Runtime>();

        assert_eq!(
            MBCRewards::<Runtime>::get(alice()),
            (balance!(0), balance!(0))
        );
        assert_eq!(
            MBCRewards::<Runtime>::get(bob()),
            (balance!(0), balance!(0))
        );
        assert_eq!(
            MBCRewards::<Runtime>::get(eve()),
            (balance!(0), balance!(0))
        );

        assert_eq!(
            VRewards::<Runtime>::get(alice()),
            crate::RewardInfo {
                limit: balance!(0.5),
                total_available: balance!(680),
                rewards: [(RewardReason::BuyOnBondingCurve, balance!(680))]
                    .iter()
                    .cloned()
                    .collect()
            }
        );
        assert_eq!(
            VRewards::<Runtime>::get(bob()),
            crate::RewardInfo {
                limit: balance!(0),
                total_available: balance!(68),
                rewards: [(RewardReason::BuyOnBondingCurve, balance!(68))]
                    .iter()
                    .cloned()
                    .collect()
            }
        );
        assert_eq!(
            VRewards::<Runtime>::get(eve()),
            crate::RewardInfo {
                limit: balance!(0.5),
                total_available: balance!(6.8),
                rewards: [(RewardReason::BuyOnBondingCurve, balance!(6.8))]
                    .iter()
                    .cloned()
                    .collect()
            }
        );
        assert_eq!(
            VTotalRewards::<Runtime>::get(),
            balance!(680) + balance!(68) + balance!(6.8)
        );
    });
}

#[test]
fn claiming_single_user() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        deposit_rewards_to_reserves(balance!(1000));
        VestedRewards::add_tbc_reward(&alice(), balance!(100)).expect("Failed to add reward.");
        VestedRewards::on_pswap_burned(PswapRemintInfo {
            vesting: balance!(12),
            ..Default::default()
        });
        assert_eq!(
            VestedRewards::rewards(&alice()),
            RewardInfo {
                limit: balance!(12),
                total_available: balance!(100),
                rewards: [(RewardReason::BuyOnBondingCurve, balance!(100))]
                    .iter()
                    .cloned()
                    .collect(),
            }
        );
        assert_eq!(Assets::free_balance(&PSWAP, &alice()).unwrap(), balance!(0));
        VestedRewards::claim_rewards_inner(&alice()).expect("Failed to claim");
        assert_eq!(
            VestedRewards::rewards(&alice()),
            RewardInfo {
                limit: balance!(0),
                total_available: balance!(88),
                rewards: [(RewardReason::BuyOnBondingCurve, balance!(88))]
                    .iter()
                    .cloned()
                    .collect(),
            }
        );
        assert_eq!(
            Assets::free_balance(&PSWAP, &alice()).unwrap(),
            balance!(12)
        );
    });
}

#[test]
fn claiming_single_user_multiple_rewards() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        deposit_rewards_to_reserves(balance!(1000));
        VestedRewards::add_tbc_reward(&alice(), balance!(100)).expect("Failed to add reward.");
        VestedRewards::add_market_maker_reward(&alice(), balance!(200))
            .expect("Failed to add reward.");
        VestedRewards::on_pswap_burned(PswapRemintInfo {
            vesting: balance!(170),
            ..Default::default()
        });
        assert_eq!(
            VestedRewards::rewards(&alice()),
            RewardInfo {
                limit: balance!(170),
                total_available: balance!(300),
                rewards: [
                    (RewardReason::BuyOnBondingCurve, balance!(100)),
                    (RewardReason::MarketMakerVolume, balance!(200))
                ]
                .iter()
                .cloned()
                .collect(),
            }
        );
        assert_eq!(Assets::free_balance(&PSWAP, &alice()).unwrap(), balance!(0));
        VestedRewards::claim_rewards_inner(&alice()).expect("Failed to claim");
        assert_eq!(
            VestedRewards::rewards(&alice()),
            RewardInfo {
                limit: balance!(0),
                total_available: balance!(130),
                rewards: [
                    (RewardReason::BuyOnBondingCurve, balance!(0)),
                    (RewardReason::MarketMakerVolume, balance!(130))
                ]
                .iter()
                .cloned()
                .collect(),
            }
        );
        assert_eq!(
            Assets::free_balance(&PSWAP, &alice()).unwrap(),
            balance!(170)
        );
    });
}

#[test]
fn claiming_multiple_users() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let total_rewards = balance!(1 + 2 + 30 + 40 + 500 + 600);
        deposit_rewards_to_reserves(total_rewards);
        VestedRewards::add_tbc_reward(&alice(), balance!(1)).expect("Failed to add reward.");
        VestedRewards::add_market_maker_reward(&alice(), balance!(2))
            .expect("Failed to add reward.");
        VestedRewards::add_tbc_reward(&bob(), balance!(30)).expect("Failed to add reward.");
        VestedRewards::add_market_maker_reward(&bob(), balance!(40))
            .expect("Failed to add reward.");
        VestedRewards::add_tbc_reward(&eve(), balance!(500)).expect("Failed to add reward.");
        VestedRewards::add_market_maker_reward(&eve(), balance!(600))
            .expect("Failed to add reward.");

        VestedRewards::on_pswap_burned(PswapRemintInfo {
            vesting: total_rewards,
            ..Default::default()
        });
        assert_eq!(
            VestedRewards::rewards(&alice()),
            RewardInfo {
                limit: balance!(3),
                total_available: balance!(3),
                rewards: [
                    (RewardReason::BuyOnBondingCurve, balance!(1)),
                    (RewardReason::MarketMakerVolume, balance!(2))
                ]
                .iter()
                .cloned()
                .collect(),
            }
        );
        assert_eq!(
            VestedRewards::rewards(&bob()),
            RewardInfo {
                limit: balance!(70),
                total_available: balance!(70),
                rewards: [
                    (RewardReason::BuyOnBondingCurve, balance!(30)),
                    (RewardReason::MarketMakerVolume, balance!(40))
                ]
                .iter()
                .cloned()
                .collect(),
            }
        );
        assert_eq!(
            VestedRewards::rewards(&eve()),
            RewardInfo {
                limit: balance!(1100),
                total_available: balance!(1100),
                rewards: [
                    (RewardReason::BuyOnBondingCurve, balance!(500)),
                    (RewardReason::MarketMakerVolume, balance!(600))
                ]
                .iter()
                .cloned()
                .collect(),
            }
        );
        assert_eq!(Assets::free_balance(&PSWAP, &alice()).unwrap(), balance!(0));
        assert_eq!(Assets::free_balance(&PSWAP, &bob()).unwrap(), balance!(0));
        assert_eq!(Assets::free_balance(&PSWAP, &eve()).unwrap(), balance!(0));
        VestedRewards::claim_rewards_inner(&alice()).expect("Failed to claim");
        VestedRewards::claim_rewards_inner(&bob()).expect("Failed to claim");
        VestedRewards::claim_rewards_inner(&eve()).expect("Failed to claim");
        assert_eq!(
            VestedRewards::rewards(&alice()),
            RewardInfo {
                limit: balance!(0),
                total_available: balance!(0),
                rewards: [
                    (RewardReason::BuyOnBondingCurve, balance!(0)),
                    (RewardReason::MarketMakerVolume, balance!(0))
                ]
                .iter()
                .cloned()
                .collect(),
            }
        );
        assert_eq!(
            VestedRewards::rewards(&alice()),
            RewardInfo {
                limit: balance!(0),
                total_available: balance!(0),
                rewards: [
                    (RewardReason::BuyOnBondingCurve, balance!(0)),
                    (RewardReason::MarketMakerVolume, balance!(0))
                ]
                .iter()
                .cloned()
                .collect(),
            }
        );
        assert_eq!(
            VestedRewards::rewards(&alice()),
            RewardInfo {
                limit: balance!(0),
                total_available: balance!(0),
                rewards: [
                    (RewardReason::BuyOnBondingCurve, balance!(0)),
                    (RewardReason::MarketMakerVolume, balance!(0))
                ]
                .iter()
                .cloned()
                .collect(),
            }
        );
        assert_eq!(Assets::free_balance(&PSWAP, &alice()).unwrap(), balance!(3));
        assert_eq!(Assets::free_balance(&PSWAP, &bob()).unwrap(), balance!(70));
        assert_eq!(
            Assets::free_balance(&PSWAP, &eve()).unwrap(),
            balance!(1100)
        );
    });
}

#[test]
fn sequential_claims_until_reserves_are_depleted() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        deposit_rewards_to_reserves(balance!(60));
        // reward amount greater than reserves is added
        VestedRewards::add_tbc_reward(&alice(), balance!(61)).expect("Failed to add reward.");
        // portion of reward is vested
        VestedRewards::on_pswap_burned(PswapRemintInfo {
            vesting: balance!(10),
            ..Default::default()
        });
        assert_eq!(
            VestedRewards::rewards(&alice()),
            RewardInfo {
                limit: balance!(10),
                total_available: balance!(61),
                rewards: [(RewardReason::BuyOnBondingCurve, balance!(61))]
                    .iter()
                    .cloned()
                    .collect(),
            }
        );
        // no claim yet, another portion of reward is vested
        VestedRewards::on_pswap_burned(PswapRemintInfo {
            vesting: balance!(20),
            ..Default::default()
        });
        assert_eq!(
            VestedRewards::rewards(&alice()),
            RewardInfo {
                limit: balance!(30),
                total_available: balance!(61),
                rewards: [(RewardReason::BuyOnBondingCurve, balance!(61))]
                    .iter()
                    .cloned()
                    .collect(),
            }
        );
        // user claims existing reward
        assert_eq!(Assets::free_balance(&PSWAP, &alice()).unwrap(), balance!(0));
        VestedRewards::claim_rewards_inner(&alice()).expect("Failed to claim");
        assert_eq!(
            VestedRewards::rewards(&alice()),
            RewardInfo {
                limit: balance!(0),
                total_available: balance!(31),
                rewards: [(RewardReason::BuyOnBondingCurve, balance!(31))]
                    .iter()
                    .cloned()
                    .collect(),
            }
        );
        assert_eq!(
            Assets::free_balance(&PSWAP, &alice()).unwrap(),
            balance!(30)
        );
        // remaining portion is vested
        VestedRewards::on_pswap_burned(PswapRemintInfo {
            vesting: balance!(30),
            ..Default::default()
        });
        assert_eq!(
            VestedRewards::rewards(&alice()),
            RewardInfo {
                limit: balance!(30),
                total_available: balance!(31),
                rewards: [(RewardReason::BuyOnBondingCurve, balance!(31))]
                    .iter()
                    .cloned()
                    .collect(),
            }
        );
        // remaining portion is vested
        VestedRewards::on_pswap_burned(PswapRemintInfo {
            vesting: balance!(40),
            ..Default::default()
        });
        assert_eq!(
            VestedRewards::rewards(&alice()),
            RewardInfo {
                limit: balance!(70),
                total_available: balance!(31),
                rewards: [(RewardReason::BuyOnBondingCurve, balance!(31))]
                    .iter()
                    .cloned()
                    .collect(),
            }
        );
        // trying to claim remaining amount, amount is limited because reserves are depleted
        VestedRewards::claim_rewards_inner(&alice()).expect("Failed to claim");
        assert_eq!(
            VestedRewards::rewards(&alice()),
            RewardInfo {
                limit: balance!(40),
                total_available: balance!(1),
                rewards: [(RewardReason::BuyOnBondingCurve, balance!(1))]
                    .iter()
                    .cloned()
                    .collect(),
            }
        );
        assert_eq!(
            Assets::free_balance(&PSWAP, &alice()).unwrap(),
            balance!(60)
        );
        assert_noop!(
            VestedRewards::claim_rewards_inner(&alice()),
            Error::<Runtime>::RewardsSupplyShortage
        );
        assert_eq!(
            VestedRewards::rewards(&alice()),
            RewardInfo {
                limit: balance!(40),
                total_available: balance!(1),
                rewards: [(RewardReason::BuyOnBondingCurve, balance!(1))]
                    .iter()
                    .cloned()
                    .collect(),
            }
        );
        assert_eq!(
            Assets::free_balance(&PSWAP, &alice()).unwrap(),
            balance!(60)
        );
    });
}

#[test]
fn some_rewards_reserves_are_depleted() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        // deposit pswap only to tbc rewards account
        Currencies::deposit(PSWAP, &GetMarketMakerRewardsAccountId::get(), balance!(100)).unwrap();
        // reward amount greater than reserves is added
        VestedRewards::add_tbc_reward(&alice(), balance!(10)).expect("Failed to add reward.");
        VestedRewards::add_market_maker_reward(&alice(), balance!(20))
            .expect("Failed to add reward.");
        // full amount is vested
        VestedRewards::on_pswap_burned(PswapRemintInfo {
            vesting: balance!(30),
            ..Default::default()
        });
        assert_eq!(
            VestedRewards::rewards(&alice()),
            RewardInfo {
                limit: balance!(30),
                total_available: balance!(30),
                rewards: [
                    (RewardReason::BuyOnBondingCurve, balance!(10)),
                    (RewardReason::MarketMakerVolume, balance!(20))
                ]
                .iter()
                .cloned()
                .collect(),
            }
        );
        VestedRewards::claim_rewards_inner(&alice()).unwrap();
        assert_eq!(
            VestedRewards::rewards(&alice()),
            RewardInfo {
                limit: balance!(10),
                total_available: balance!(10),
                rewards: [
                    (RewardReason::BuyOnBondingCurve, balance!(10)),
                    (RewardReason::MarketMakerVolume, balance!(0))
                ]
                .iter()
                .cloned()
                .collect(),
            }
        );
        assert_noop!(
            VestedRewards::claim_rewards_inner(&alice()),
            Error::<Runtime>::RewardsSupplyShortage
        );
    });
}

// claiming with limit: none rewards, less than one reward, exactly one reward, less than two rewards, exactly two rewards, limit is greater than available
// claiming with not enough reserves: in all accs, in single acc, in multiple accs
// trying to claim error cases
