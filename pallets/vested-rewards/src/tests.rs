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
use crate::{
    Error, MarketMakerInfo, MarketMakingPairs, RewardInfo,
    MARKET_MAKER_REWARDS_DISTRIBUTION_FREQUENCY,
};
use codec::Decode;
use common::{
    balance, Balance, Fixed, OnPswapBurned, PswapRemintInfo, RewardReason, VestedRewardsPallet,
    ETH, PSWAP, XOR, XSTUSD,
};
use frame_support::assert_noop;
use frame_support::pallet_prelude::DispatchError;
use frame_support::traits::OnInitialize;
use std::convert::TryFrom;
use traits::currency::MultiCurrency;

fn deposit_rewards_to_reserves(amount: Balance) {
    Currencies::deposit(PSWAP, &GetBondingCurveRewardsAccountId::get(), amount).unwrap();
    Currencies::deposit(PSWAP, &GetMarketMakerRewardsAccountId::get(), amount).unwrap();
}

fn prepare_mm_pairs() {
    MarketMakingPairs::<Runtime>::insert(&XOR, &ETH, ());
    MarketMakingPairs::<Runtime>::insert(&XSTUSD, &XOR, ());
}

#[test]
fn should_add_market_maker_infos_single_user() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        prepare_mm_pairs();

        assert_eq!(
            VestedRewards::market_makers_registry(&alice()),
            MarketMakerInfo {
                count: 0,
                volume: balance!(0)
            }
        );

        // first add
        VestedRewards::update_market_maker_records(
            &alice(),
            &XOR,
            balance!(123),
            1,
            &XOR,
            &ETH,
            None,
        )
        .unwrap();
        let expected_1 = MarketMakerInfo {
            count: 1,
            volume: balance!(123),
        };
        assert_eq!(VestedRewards::market_makers_registry(&alice()), expected_1);

        // second add
        VestedRewards::update_market_maker_records(
            &alice(),
            &XOR,
            balance!(123),
            1,
            &XOR,
            &ETH,
            None,
        )
        .unwrap();
        let expected_2 = MarketMakerInfo {
            count: 2,
            volume: balance!(246),
        };
        assert_eq!(
            VestedRewards::market_makers_registry(&alice()),
            expected_2.clone()
        );

        // add with less than 1 xor
        VestedRewards::update_market_maker_records(
            &alice(),
            &XOR,
            balance!(0.9),
            1,
            &XOR,
            &ETH,
            None,
        )
        .unwrap();
        assert_eq!(VestedRewards::market_makers_registry(&alice()), expected_2);

        // add with multiplier
        VestedRewards::update_market_maker_records(
            &alice(),
            &XOR,
            balance!(123),
            2,
            &XOR,
            &ETH,
            None,
        )
        .unwrap();
        let expected_3 = MarketMakerInfo {
            count: 4,
            volume: balance!(492),
        };
        assert_eq!(VestedRewards::market_makers_registry(&alice()), expected_3);
    });
}

#[test]
fn should_add_market_maker_infos_for_xstusd_dex_single_user() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        prepare_mm_pairs();

        assert_eq!(
            VestedRewards::market_makers_registry(&alice()),
            MarketMakerInfo {
                count: 0,
                volume: balance!(0)
            }
        );

        // first add
        VestedRewards::update_market_maker_records(
            &alice(),
            &XSTUSD,
            balance!(123),
            1,
            &XOR,
            &ETH,
            None,
        )
        .unwrap();
        let expected_1 = MarketMakerInfo {
            count: 1,
            volume: balance!(246),
        };
        assert_eq!(VestedRewards::market_makers_registry(&alice()), expected_1);

        // second add
        VestedRewards::update_market_maker_records(
            &alice(),
            &XSTUSD,
            balance!(123),
            1,
            &XOR,
            &ETH,
            None,
        )
        .unwrap();
        let expected_2 = MarketMakerInfo {
            count: 2,
            volume: balance!(492),
        };
        assert_eq!(
            VestedRewards::market_makers_registry(&alice()),
            expected_2.clone()
        );

        // add with less than 1 xor
        VestedRewards::update_market_maker_records(
            &alice(),
            &XSTUSD,
            balance!(0.45),
            1,
            &XOR,
            &ETH,
            None,
        )
        .unwrap();
        assert_eq!(VestedRewards::market_makers_registry(&alice()), expected_2);

        // add with multiplier
        VestedRewards::update_market_maker_records(
            &alice(),
            &XSTUSD,
            balance!(123),
            2,
            &XOR,
            &ETH,
            None,
        )
        .unwrap();
        let expected_3 = MarketMakerInfo {
            count: 4,
            volume: balance!(984),
        };
        assert_eq!(VestedRewards::market_makers_registry(&alice()), expected_3);
    });
}

#[test]
fn should_add_market_maker_infos_multiple_users() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        prepare_mm_pairs();

        VestedRewards::update_market_maker_records(
            &alice(),
            &XOR,
            balance!(111),
            1,
            &XOR,
            &ETH,
            None,
        )
        .unwrap();
        VestedRewards::update_market_maker_records(
            &bob(),
            &XOR,
            balance!(111),
            2,
            &XOR,
            &ETH,
            None,
        )
        .unwrap();
        VestedRewards::update_market_maker_records(
            &eve(),
            &XOR,
            balance!(111),
            3,
            &XOR,
            &ETH,
            None,
        )
        .unwrap();
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
fn should_add_market_maker_infos_for_xstusd_dex_multiple_users() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        prepare_mm_pairs();

        VestedRewards::update_market_maker_records(
            &alice(),
            &XSTUSD,
            balance!(111),
            1,
            &XOR,
            &ETH,
            None,
        )
        .unwrap();
        VestedRewards::update_market_maker_records(
            &bob(),
            &XSTUSD,
            balance!(111),
            2,
            &XOR,
            &ETH,
            None,
        )
        .unwrap();
        VestedRewards::update_market_maker_records(
            &eve(),
            &XSTUSD,
            balance!(111),
            3,
            &XOR,
            &ETH,
            None,
        )
        .unwrap();
        assert_eq!(
            VestedRewards::market_makers_registry(&alice()),
            MarketMakerInfo {
                count: 1,
                volume: balance!(222)
            }
        );
        assert_eq!(
            VestedRewards::market_makers_registry(&bob()),
            MarketMakerInfo {
                count: 2,
                volume: balance!(444)
            }
        );
        assert_eq!(
            VestedRewards::market_makers_registry(&eve()),
            MarketMakerInfo {
                count: 3,
                volume: balance!(666)
            }
        );
    });
}

#[test]
fn should_update_market_maker_with_allowed_pair_only() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        prepare_mm_pairs();

        assert_eq!(
            VestedRewards::market_makers_registry(&alice()),
            MarketMakerInfo {
                count: 0,
                volume: balance!(0)
            }
        );

        // ok
        VestedRewards::update_market_maker_records(
            &alice(),
            &XOR,
            balance!(123),
            1,
            &XOR,
            &ETH,
            None,
        )
        .unwrap();
        let expected = MarketMakerInfo {
            count: 1,
            volume: balance!(123),
        };
        assert_eq!(
            VestedRewards::market_makers_registry(&alice()),
            expected.clone()
        );

        // with intermediate
        VestedRewards::update_market_maker_records(
            &alice(),
            &XOR,
            balance!(123),
            1,
            &XSTUSD,
            &ETH,
            Some(&XOR),
        )
        .unwrap();
        let expected = MarketMakerInfo {
            count: 2,
            volume: balance!(246),
        };
        assert_eq!(
            VestedRewards::market_makers_registry(&alice()),
            expected.clone()
        );

        // not allowed
        VestedRewards::update_market_maker_records(
            &alice(),
            &XOR,
            balance!(123),
            1,
            &ETH,
            &XOR,
            None,
        )
        .unwrap();
        assert_eq!(VestedRewards::market_makers_registry(&alice()), expected);
    });
}

#[test]
fn should_update_market_making_pairs_correctly() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        prepare_mm_pairs();

        let origin = RuntimeOrigin::none();

        assert_noop!(
            VestedRewards::set_asset_pair(origin.clone(), ETH, XOR, true),
            DispatchError::BadOrigin
        );

        let origin = RuntimeOrigin::root();

        VestedRewards::set_asset_pair(origin.clone(), ETH, XOR, true).unwrap();

        assert!(MarketMakingPairs::<Runtime>::contains_key(&ETH, &XOR));

        // we already have this pair, so it should return an error
        assert_noop!(
            VestedRewards::set_asset_pair(origin.clone(), XOR, ETH, true),
            Error::<Runtime>::MarketMakingPairAlreadyAllowed
        );

        let origin = RuntimeOrigin::none();

        assert_noop!(
            VestedRewards::set_asset_pair(origin.clone(), ETH, XOR, false),
            DispatchError::BadOrigin
        );

        let origin = RuntimeOrigin::root();

        VestedRewards::set_asset_pair(origin.clone(), ETH, XOR, false).unwrap();

        // we don't have this pair anymore, so it should return an error
        assert_noop!(
            VestedRewards::set_asset_pair(origin, ETH, XOR, false),
            Error::<Runtime>::MarketMakingPairAlreadyDisallowed
        );
    });
}

#[test]
fn trying_to_add_market_maker_entry_no_side_effect() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        prepare_mm_pairs();

        let root_a = frame_support::storage_root(frame_support::StateVersion::V1);
        VestedRewards::update_market_maker_records(
            &alice(),
            &XOR,
            balance!(1),
            1,
            &XOR,
            &ETH,
            None,
        )
        .unwrap();
        let root_b = frame_support::storage_root(frame_support::StateVersion::V1);
        assert_ne!(root_a, root_b);
        // adding record should not add default value explicitly for non-eligible volume
        VestedRewards::update_market_maker_records(
            &alice(),
            &XOR,
            balance!(0.99),
            1,
            &XOR,
            &ETH,
            None,
        )
        .unwrap();
        let root_c = frame_support::storage_root(frame_support::StateVersion::V1);
        assert_eq!(root_b, root_c);
    });
}

#[test]
fn can_claim_crowdloan_reward() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        use crate::{CrowdloanReward, CrowdloanRewards};

        let tech_account = GetCrowdloanRewardsAccountId::get();
        currencies::Pallet::<Runtime>::update_balance(
            RuntimeOrigin::root(),
            tech_account,
            PSWAP.into(),
            balance!(1000000) as <Runtime as tokens::Config>::Amount,
        )
        .unwrap();

        let mut raw_address = &[
            92, 92, 122, 119, 21, 211, 27, 86, 74, 193, 56, 61, 11, 124, 127, 100, 234, 233, 8,
            200, 238, 178, 238, 40, 215, 84, 140, 255, 219, 251, 115, 41,
        ][..];
        let account =
            <Runtime as frame_system::Config>::AccountId::decode(&mut raw_address).unwrap();
        let pswap_reward = Fixed::try_from(60000).unwrap();

        CrowdloanRewards::<Runtime>::insert(
            &account,
            CrowdloanReward {
                address: raw_address.into(),
                pswap_reward,
                ..Default::default()
            },
        );

        let number_of_days = 20;
        let current_block_number =
            (crate::BLOCKS_PER_DAY * number_of_days + crate::LEASE_START_BLOCK) as u64;

        frame_system::Pallet::<Runtime>::set_block_number(current_block_number);

        crate::Pallet::<Runtime>::claim_crowdloan_rewards(Some(account.clone()).into(), PSWAP)
            .unwrap();

        assert_eq!(
            3773584905660377358480,
            assets::Pallet::<Runtime>::total_balance(&PSWAP, &account).unwrap()
        );

        // second claim for the same period doesn't change the balance

        crate::Pallet::<Runtime>::claim_crowdloan_rewards(Some(account.clone()).into(), PSWAP)
            .unwrap();

        assert_eq!(
            3773584905660377358480,
            assets::Pallet::<Runtime>::total_balance(&PSWAP, &account).unwrap()
        );

        // claim after the end of the lease period

        let current_block_number =
            (crate::BLOCKS_PER_DAY * crate::LEASE_TOTAL_DAYS + crate::LEASE_START_BLOCK) as u64;

        frame_system::Pallet::<Runtime>::set_block_number(current_block_number);

        crate::Pallet::<Runtime>::claim_crowdloan_rewards(Some(account.clone()).into(), PSWAP)
            .unwrap();

        assert_eq!(
            59999999999999999999832,
            assets::Pallet::<Runtime>::total_balance(&PSWAP, &account).unwrap()
        );
    });
}

#[test]
fn crowdloan_reward_period_is_whole_days() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        use crate::{CrowdloanReward, CrowdloanRewards};

        let tech_account = GetCrowdloanRewardsAccountId::get();
        currencies::Pallet::<Runtime>::update_balance(
            RuntimeOrigin::root(),
            tech_account,
            PSWAP.into(),
            balance!(1000) as <Runtime as tokens::Config>::Amount,
        )
        .unwrap();

        let mut raw_address = &[
            92, 92, 122, 119, 21, 211, 27, 86, 74, 193, 56, 61, 11, 124, 127, 100, 234, 233, 8,
            200, 238, 178, 238, 40, 215, 84, 140, 255, 219, 251, 115, 41,
        ][..];
        let account =
            <Runtime as frame_system::Config>::AccountId::decode(&mut raw_address).unwrap();
        let pswap_reward = Fixed::try_from(100).unwrap();

        CrowdloanRewards::<Runtime>::insert(
            &account,
            CrowdloanReward {
                address: raw_address.into(),
                pswap_reward,
                ..Default::default()
            },
        );

        let number_of_days = 3;
        let half_day = crate::BLOCKS_PER_DAY / 2;
        let current_block_number =
            (crate::BLOCKS_PER_DAY * number_of_days + half_day + crate::LEASE_START_BLOCK) as u64;

        frame_system::Pallet::<Runtime>::set_block_number(current_block_number);

        crate::Pallet::<Runtime>::claim_crowdloan_rewards(Some(account.clone()).into(), PSWAP)
            .unwrap();

        assert_eq!(
            943396226415094338,
            assets::Pallet::<Runtime>::total_balance(&PSWAP, &account).unwrap()
        );

        let current_block_number = current_block_number + (half_day as u64);
        frame_system::Pallet::<Runtime>::set_block_number(current_block_number);
        crate::Pallet::<Runtime>::claim_crowdloan_rewards(Some(account.clone()).into(), PSWAP)
            .unwrap();

        assert_eq!(
            1257861635220125784,
            assets::Pallet::<Runtime>::total_balance(&PSWAP, &account).unwrap()
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
        VestedRewards::claim_rewards(RuntimeOrigin::signed(alice())).expect("Failed to claim");
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
        VestedRewards::claim_rewards(RuntimeOrigin::signed(alice())).expect("Failed to claim");
        assert_eq!(
            VestedRewards::rewards(&alice()),
            RewardInfo {
                limit: balance!(0),
                total_available: balance!(130),
                rewards: [(RewardReason::MarketMakerVolume, balance!(130))]
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
        VestedRewards::claim_rewards(RuntimeOrigin::signed(alice())).expect("Failed to claim");
        VestedRewards::claim_rewards(RuntimeOrigin::signed(bob())).expect("Failed to claim");
        VestedRewards::claim_rewards(RuntimeOrigin::signed(eve())).expect("Failed to claim");
        assert_eq!(
            VestedRewards::rewards(&alice()),
            RewardInfo {
                limit: balance!(0),
                total_available: balance!(0),
                rewards: Default::default(),
            }
        );
        assert_eq!(
            VestedRewards::rewards(&bob()),
            RewardInfo {
                limit: balance!(0),
                total_available: balance!(0),
                rewards: Default::default(),
            }
        );
        assert_eq!(
            VestedRewards::rewards(&eve()),
            RewardInfo {
                limit: balance!(0),
                total_available: balance!(0),
                rewards: Default::default(),
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
        VestedRewards::claim_rewards(RuntimeOrigin::signed(alice())).expect("Failed to claim");
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
                limit: balance!(31),
                total_available: balance!(31),
                rewards: [(RewardReason::BuyOnBondingCurve, balance!(31))]
                    .iter()
                    .cloned()
                    .collect(),
            }
        );
        // trying to claim remaining amount, amount is limited because reserves are depleted
        VestedRewards::claim_rewards(RuntimeOrigin::signed(alice())).expect("Failed to claim");
        assert_eq!(
            VestedRewards::rewards(&alice()),
            RewardInfo {
                limit: balance!(1),
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
            VestedRewards::claim_rewards(RuntimeOrigin::signed(alice())),
            Error::<Runtime>::RewardsSupplyShortage
        );
        assert_eq!(
            VestedRewards::rewards(&alice()),
            RewardInfo {
                limit: balance!(1),
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
        VestedRewards::claim_rewards(RuntimeOrigin::signed(alice())).unwrap();
        assert_eq!(
            VestedRewards::rewards(&alice()),
            RewardInfo {
                limit: balance!(10),
                total_available: balance!(10),
                rewards: [(RewardReason::BuyOnBondingCurve, balance!(10))]
                    .iter()
                    .cloned()
                    .collect(),
            }
        );
        assert_noop!(
            VestedRewards::claim_rewards(RuntimeOrigin::signed(alice())),
            Error::<Runtime>::RewardsSupplyShortage
        );
    });
}

#[test]
fn all_rewards_reserves_are_depleted() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        // no funds are added to reserves
        VestedRewards::add_tbc_reward(&alice(), balance!(10)).expect("Failed to add reward.");
        VestedRewards::add_market_maker_reward(&alice(), balance!(20))
            .expect("Failed to add reward.");
        // full amount is vested
        VestedRewards::on_pswap_burned(PswapRemintInfo {
            vesting: balance!(40),
            ..Default::default()
        });
        assert_noop!(
            VestedRewards::claim_rewards(RuntimeOrigin::signed(alice())),
            Error::<Runtime>::RewardsSupplyShortage
        );
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
    });
}

#[test]
fn claiming_without_rewards() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        // deposit pswap for one user
        Currencies::deposit(
            PSWAP,
            &GetBondingCurveRewardsAccountId::get(),
            balance!(100),
        )
        .unwrap();
        VestedRewards::add_tbc_reward(&alice(), balance!(10)).expect("Failed to add reward.");
        VestedRewards::on_pswap_burned(PswapRemintInfo {
            vesting: balance!(30),
            ..Default::default()
        });
        assert_eq!(
            VestedRewards::rewards(&bob()),
            RewardInfo {
                limit: balance!(0),
                total_available: balance!(0),
                rewards: Default::default(),
            }
        );
        assert_noop!(
            VestedRewards::claim_rewards(RuntimeOrigin::signed(bob())),
            Error::<Runtime>::NothingToClaim
        );
        VestedRewards::add_tbc_reward(&bob(), balance!(10)).expect("Failed to add reward.");
        assert_noop!(
            VestedRewards::claim_rewards(RuntimeOrigin::signed(bob())),
            Error::<Runtime>::ClaimLimitExceeded
        );
        VestedRewards::on_pswap_burned(PswapRemintInfo {
            vesting: balance!(30),
            ..Default::default()
        });
        assert_eq!(Assets::free_balance(&PSWAP, &bob()).unwrap(), balance!(0));
        VestedRewards::claim_rewards(RuntimeOrigin::signed(bob()))
            .expect("Failed to claim reward.");
        assert_eq!(Assets::free_balance(&PSWAP, &bob()).unwrap(), balance!(10));
    });
}

#[test]
fn empty_reward_entries_are_removed() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        // deposit pswap for one user
        Currencies::deposit(
            PSWAP,
            &GetBondingCurveRewardsAccountId::get(),
            balance!(100),
        )
        .unwrap();
        Currencies::deposit(PSWAP, &GetMarketMakerRewardsAccountId::get(), balance!(100)).unwrap();
        VestedRewards::add_tbc_reward(&alice(), balance!(10)).expect("Failed to add reward.");
        VestedRewards::add_market_maker_reward(&alice(), balance!(15))
            .expect("Failed to add reward.");
        VestedRewards::on_pswap_burned(PswapRemintInfo {
            vesting: balance!(20),
            ..Default::default()
        });
        assert_eq!(
            VestedRewards::rewards(&alice()),
            RewardInfo {
                limit: balance!(20),
                total_available: balance!(25),
                rewards: [
                    (RewardReason::BuyOnBondingCurve, balance!(10)),
                    (RewardReason::MarketMakerVolume, balance!(15))
                ]
                .iter()
                .cloned()
                .collect(),
            }
        );
        VestedRewards::claim_rewards(RuntimeOrigin::signed(alice())).unwrap();
        // zeroed entry is removed
        assert_eq!(
            VestedRewards::rewards(&alice()),
            RewardInfo {
                limit: balance!(0),
                total_available: balance!(5),
                rewards: [(RewardReason::MarketMakerVolume, balance!(5))]
                    .iter()
                    .cloned()
                    .collect(),
            }
        );
    });
}

#[test]
fn accounts_with_no_rewards_are_removed() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        // deposit pswap for one user
        Currencies::deposit(
            PSWAP,
            &GetBondingCurveRewardsAccountId::get(),
            balance!(100),
        )
        .unwrap();
        VestedRewards::add_tbc_reward(&alice(), balance!(10)).expect("Failed to add reward.");
        VestedRewards::on_pswap_burned(PswapRemintInfo {
            vesting: balance!(10),
            ..Default::default()
        });
        assert_eq!(
            VestedRewards::rewards(&alice()),
            RewardInfo {
                limit: balance!(10),
                total_available: balance!(10),
                rewards: [(RewardReason::BuyOnBondingCurve, balance!(10))]
                    .iter()
                    .cloned()
                    .collect(),
            }
        );
        let accounts: Vec<_> = crate::Rewards::<Runtime>::iter().collect();
        assert_eq!(accounts.len(), 1);

        VestedRewards::claim_rewards(RuntimeOrigin::signed(alice())).unwrap();
        // account has zeroed values, default is returned on query:
        assert_eq!(
            VestedRewards::rewards(&alice()),
            RewardInfo {
                limit: balance!(0),
                total_available: balance!(0),
                rewards: Default::default(),
            }
        );

        let accounts: Vec<_> = crate::Rewards::<Runtime>::iter().collect();
        assert!(accounts.is_empty());
    });
}

#[test]
fn distributing_with_all_eligible_accounts() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        prepare_mm_pairs();

        Currencies::deposit(
            PSWAP,
            &GetMarketMakerRewardsAccountId::get(),
            balance!(400000000),
        )
        .unwrap();
        VestedRewards::update_market_maker_records(
            &alice(),
            &XOR,
            balance!(10),
            500,
            &XOR,
            &ETH,
            None,
        )
        .unwrap();
        VestedRewards::update_market_maker_records(
            &bob(),
            &XOR,
            balance!(20),
            1000,
            &XOR,
            &ETH,
            None,
        )
        .unwrap();
        VestedRewards::update_market_maker_records(
            &eve(),
            &XOR,
            balance!(30),
            2000,
            &XOR,
            &ETH,
            None,
        )
        .unwrap();

        for block_n in 1..MARKET_MAKER_REWARDS_DISTRIBUTION_FREQUENCY {
            VestedRewards::on_initialize(block_n.into());
        }
        assert_eq!(
            VestedRewards::rewards(&alice()),
            RewardInfo {
                limit: balance!(0),
                total_available: balance!(0),
                rewards: Default::default(),
            }
        );
        assert_eq!(
            VestedRewards::rewards(&bob()),
            RewardInfo {
                limit: balance!(0),
                total_available: balance!(0),
                rewards: Default::default(),
            }
        );
        assert_eq!(
            VestedRewards::rewards(&eve()),
            RewardInfo {
                limit: balance!(0),
                total_available: balance!(0),
                rewards: Default::default(),
            }
        );
        assert_eq!(
            VestedRewards::market_makers_registry(&alice()),
            MarketMakerInfo {
                count: 500,
                volume: balance!(5000),
            }
        );
        assert_eq!(
            VestedRewards::market_makers_registry(&bob()),
            MarketMakerInfo {
                count: 1000,
                volume: balance!(20000),
            }
        );
        assert_eq!(
            VestedRewards::market_makers_registry(&eve()),
            MarketMakerInfo {
                count: 2000,
                volume: balance!(60000),
            }
        );
        // invoking distribution routine
        VestedRewards::on_initialize(MARKET_MAKER_REWARDS_DISTRIBUTION_FREQUENCY.into());
        let reward_alice = balance!(1176470.588235294117647058);
        let reward_bob = balance!(4705882.352941176470588235);
        let reward_eve = balance!(14117647.058823529411764705);
        assert_eq!(
            VestedRewards::rewards(&alice()),
            RewardInfo {
                limit: balance!(0),
                total_available: reward_alice,
                rewards: [(RewardReason::MarketMakerVolume, reward_alice)]
                    .iter()
                    .cloned()
                    .collect(),
            }
        );
        assert_eq!(
            VestedRewards::rewards(&bob()),
            RewardInfo {
                limit: balance!(0),
                total_available: reward_bob,
                rewards: [(RewardReason::MarketMakerVolume, reward_bob)]
                    .iter()
                    .cloned()
                    .collect(),
            }
        );
        assert_eq!(
            VestedRewards::rewards(&eve()),
            RewardInfo {
                limit: balance!(0),
                total_available: reward_eve,
                rewards: [(RewardReason::MarketMakerVolume, reward_eve)]
                    .iter()
                    .cloned()
                    .collect(),
            }
        );
        // close to 20M but with precision mismatch
        assert_eq!(
            reward_alice + reward_bob + reward_eve,
            balance!(19999999.999999999999999998)
        );
        // values are reset after distribution
        assert_eq!(
            VestedRewards::market_makers_registry(&alice()),
            MarketMakerInfo {
                count: 0,
                volume: balance!(0),
            }
        );
        assert_eq!(
            VestedRewards::market_makers_registry(&bob()),
            MarketMakerInfo {
                count: 0,
                volume: balance!(0),
            }
        );
        assert_eq!(
            VestedRewards::market_makers_registry(&eve()),
            MarketMakerInfo {
                count: 0,
                volume: balance!(0),
            }
        );
    });
}

#[test]
fn distributing_with_partially_eligible_accounts() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        prepare_mm_pairs();

        let initial_reserve = balance!(400000000);
        Currencies::deposit(
            PSWAP,
            &GetMarketMakerRewardsAccountId::get(),
            initial_reserve,
        )
        .unwrap();
        VestedRewards::update_market_maker_records(
            &alice(),
            &XOR,
            balance!(10),
            499,
            &XOR,
            &ETH,
            None,
        )
        .unwrap();
        VestedRewards::update_market_maker_records(
            &bob(),
            &XOR,
            balance!(0.9),
            1000,
            &XOR,
            &ETH,
            None,
        )
        .unwrap();
        VestedRewards::update_market_maker_records(
            &eve(),
            &XOR,
            balance!(30),
            2000,
            &XOR,
            &ETH,
            None,
        )
        .unwrap();

        for block_n in 1..MARKET_MAKER_REWARDS_DISTRIBUTION_FREQUENCY {
            VestedRewards::on_initialize(block_n.into());
        }
        assert_eq!(
            VestedRewards::market_makers_registry(&alice()),
            MarketMakerInfo {
                count: 499,
                volume: balance!(4990),
            }
        );
        assert_eq!(
            VestedRewards::market_makers_registry(&bob()),
            MarketMakerInfo {
                count: 0,
                volume: balance!(0),
            }
        );
        assert_eq!(
            VestedRewards::market_makers_registry(&eve()),
            MarketMakerInfo {
                count: 2000,
                volume: balance!(60000),
            }
        );
        // invoking distribution routine
        VestedRewards::on_initialize(MARKET_MAKER_REWARDS_DISTRIBUTION_FREQUENCY.into());
        let reward_alice = balance!(0);
        let reward_bob = balance!(0);
        let reward_eve = balance!(20000000);
        assert_eq!(
            VestedRewards::rewards(&alice()),
            RewardInfo {
                limit: balance!(0),
                total_available: reward_alice,
                rewards: Default::default(),
            }
        );
        assert_eq!(
            VestedRewards::rewards(&bob()),
            RewardInfo {
                limit: balance!(0),
                total_available: reward_bob,
                rewards: Default::default(),
            }
        );
        assert_eq!(
            VestedRewards::rewards(&eve()),
            RewardInfo {
                limit: balance!(0),
                total_available: reward_eve,
                rewards: [(RewardReason::MarketMakerVolume, reward_eve)]
                    .iter()
                    .cloned()
                    .collect(),
            }
        );
        assert_eq!(reward_alice + reward_bob + reward_eve, balance!(20000000));
        // values are reset after distribution
        assert_eq!(
            VestedRewards::market_makers_registry(&alice()),
            MarketMakerInfo {
                count: 0,
                volume: balance!(0),
            }
        );
        assert_eq!(
            VestedRewards::market_makers_registry(&bob()),
            MarketMakerInfo {
                count: 0,
                volume: balance!(0),
            }
        );
        assert_eq!(
            VestedRewards::market_makers_registry(&eve()),
            MarketMakerInfo {
                count: 0,
                volume: balance!(0),
            }
        );

        assert_eq!(
            Currencies::free_balance(PSWAP, &GetMarketMakerRewardsAccountId::get()),
            initial_reserve
        );

        // check balance for market makers reserve
        VestedRewards::on_pswap_burned(PswapRemintInfo {
            vesting: reward_eve,
            ..Default::default()
        });
        VestedRewards::claim_rewards(RuntimeOrigin::signed(eve())).unwrap();
        assert_eq!(
            Currencies::free_balance(PSWAP, &GetMarketMakerRewardsAccountId::get()),
            initial_reserve - reward_eve
        );
    });
}

#[test]
fn distributing_with_no_eligible_accounts_is_postponed() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        prepare_mm_pairs();

        let initial_reserve = balance!(400000000);
        Currencies::deposit(
            PSWAP,
            &GetMarketMakerRewardsAccountId::get(),
            initial_reserve,
        )
        .unwrap();
        VestedRewards::update_market_maker_records(
            &alice(),
            &XOR,
            balance!(0.5),
            10,
            &XOR,
            &ETH,
            None,
        )
        .unwrap();
        VestedRewards::update_market_maker_records(
            &bob(),
            &XOR,
            balance!(0.7),
            20,
            &XOR,
            &ETH,
            None,
        )
        .unwrap();
        VestedRewards::update_market_maker_records(
            &eve(),
            &XOR,
            balance!(0.9),
            30,
            &XOR,
            &ETH,
            None,
        )
        .unwrap();
        for block_n in 1..MARKET_MAKER_REWARDS_DISTRIBUTION_FREQUENCY * 10 {
            VestedRewards::on_initialize(block_n.into());
        }

        assert_eq!(VestedRewards::rewards(&alice()), Default::default());
        assert_eq!(VestedRewards::rewards(&bob()), Default::default());
        assert_eq!(VestedRewards::rewards(&eve()), Default::default());

        assert_eq!(
            VestedRewards::market_makers_registry(&alice()),
            Default::default()
        );
        assert_eq!(
            VestedRewards::market_makers_registry(&bob()),
            Default::default()
        );
        assert_eq!(
            VestedRewards::market_makers_registry(&eve()),
            Default::default()
        );

        assert_eq!(
            Currencies::free_balance(PSWAP, &GetMarketMakerRewardsAccountId::get()),
            initial_reserve
        );
    });
}

#[test]
fn distributing_with_no_accounts_is_postponed() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let initial_reserve = balance!(400000000);
        Currencies::deposit(
            PSWAP,
            &GetMarketMakerRewardsAccountId::get(),
            initial_reserve,
        )
        .unwrap();
        for block_n in 1..MARKET_MAKER_REWARDS_DISTRIBUTION_FREQUENCY * 10 {
            VestedRewards::on_initialize(block_n.into());
        }

        assert_noop!(
            VestedRewards::claim_rewards(RuntimeOrigin::signed(alice())),
            Error::<Runtime>::NothingToClaim
        );
        assert_noop!(
            VestedRewards::claim_rewards(RuntimeOrigin::signed(bob())),
            Error::<Runtime>::NothingToClaim
        );
        assert_noop!(
            VestedRewards::claim_rewards(RuntimeOrigin::signed(eve())),
            Error::<Runtime>::NothingToClaim
        );

        assert_eq!(
            Currencies::free_balance(PSWAP, &GetMarketMakerRewardsAccountId::get()),
            initial_reserve
        );
    });
}
