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
use crate::MarketMakerInfo;
use common::{balance, RewardReason, VestedRewardsTrait};
use sp_std::collections::btree_map::BTreeMap;

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
        use multicollateral_bonding_curve_pool::{
            Rewards as MBCRewards, TotalRewards as MBCTotalRewards,
        };

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
