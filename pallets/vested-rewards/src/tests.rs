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

// #[test]
// fn multiple_users_should_be_able_to_claim_rewards() {
//     let mut ext = ExtBuilder::new(vec![
//         (alice(), XOR, balance!(700000), AssetSymbol(b"XOR".to_vec()), AssetName(b"SORA".to_vec()), 18),
//         (alice(), VAL, balance!(2000), AssetSymbol(b"VAL".to_vec()), AssetName(b"SORA Validator Token".to_vec()), 18),
//         (alice(), DAI, balance!(200000), AssetSymbol(b"DAI".to_vec()), AssetName(b"DAI".to_vec()), 18),
//         (alice(), USDT, balance!(0), AssetSymbol(b"USDT".to_vec()), AssetName(b"Tether USD".to_vec()), 18),
//         (alice(), PSWAP, balance!(0), AssetSymbol(b"PSWAP".to_vec()), AssetName(b"Polkaswap".to_vec()), 18),
//     ])
//     .build();
//     ext.execute_with(|| {
//         MockDEXApi::init().unwrap();
//         let _ = bonding_curve_pool_init(vec![]).unwrap();
//         TradingPair::register(Origin::signed(alice()),DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
//         TradingPair::register(Origin::signed(alice()),DEXId::Polkaswap.into(), XOR, DAI).expect("Failed to register trading pair.");
//         MBCPool::initialize_pool_unchecked(VAL, false).expect("Failed to initialize pool.");
//         MBCPool::initialize_pool_unchecked(DAI, false).expect("Failed to initialize pool.");
//         Assets::transfer(Origin::signed(alice()), DAI, bob(), balance!(50000)).unwrap();
//         Currencies::deposit(PSWAP, &incentives_account(), balance!(25000000)).unwrap();

//         // performing exchanges which are eligible for rewards
//         MBCPool::exchange(
//             &alice(),
//             &alice(),
//             &DEXId::Polkaswap.into(),
//             &DAI,
//             &XOR,
//             SwapAmount::with_desired_input(balance!(100000), Balance::zero()),
//         )
//         .unwrap();
//         MBCPool::exchange(
//             &bob(),
//             &bob(),
//             &DEXId::Polkaswap.into(),
//             &DAI,
//             &XOR,
//             SwapAmount::with_desired_input(balance!(50000), Balance::zero()),
//         )
//         .unwrap();

//         // trying to claim with limit of 0
//         assert!(Assets::free_balance(&PSWAP, &alice()).unwrap().is_zero());
//         assert!(Assets::free_balance(&PSWAP, &bob()).unwrap().is_zero());
//         // assert_noop!(MBCPool::claim_incentives(Origin::signed(alice())), Error::<Runtime>::NothingToClaim);
//         // assert_noop!(MBCPool::claim_incentives(Origin::signed(bob())), Error::<Runtime>::NothingToClaim);
//         assert!(Assets::free_balance(&PSWAP, &alice()).unwrap().is_zero());
//         assert!(Assets::free_balance(&PSWAP, &bob()).unwrap().is_zero());

//         // limit is updated via PSWAP burn
//         let (limit_alice, owned_alice) = MBCPool::rewards(&alice());
//         let (limit_bob, owned_bob) = MBCPool::rewards(&bob());
//         assert!(limit_alice.is_zero());
//         assert!(limit_bob.is_zero());
//         assert!(!owned_alice.is_zero());
//         assert!(!owned_bob.is_zero());
//         let vesting_amount = (FixedWrapper::from(owned_alice + owned_bob) / fixed_wrapper!(2)).into_balance();
//         let remint_info = common::PswapRemintInfo {
//             vesting: vesting_amount,
//             ..Default::default()
//         };
//         // MBCPool::on_pswap_burned(remint_info);
//         let (limit_alice, _) = MBCPool::rewards(&alice());
//         let (limit_bob, _) = MBCPool::rewards(&bob());
//         assert_eq!(limit_alice, balance!(114222.435361663749999999));
//         assert_eq!(limit_bob, balance!(57093.659227284999999999));

//         // claiming incentives partially
//         // assert_ok!(MBCPool::claim_incentives(Origin::signed(alice())));
//         // assert_ok!(MBCPool::claim_incentives(Origin::signed(bob())));
//         let (limit_alice, remaining_owned_alice) = MBCPool::rewards(&alice());
//         let (limit_bob, remaining_owned_bob) = MBCPool::rewards(&bob());
//         assert_eq!(remaining_owned_alice, balance!(114222.435361663750000001));
//         assert_eq!(remaining_owned_bob, balance!(57093.659227285000000001));
//         assert!(limit_alice.is_zero());
//         assert!(limit_bob.is_zero());
//         assert_eq!(Assets::free_balance(&PSWAP, &alice()).unwrap(), owned_alice - remaining_owned_alice);
//         assert_eq!(Assets::free_balance(&PSWAP, &bob()).unwrap(), owned_bob - remaining_owned_bob);

//         // claiming remainder
//         let remint_info = common::PswapRemintInfo {
//             vesting: vesting_amount + balance!(100),
//             ..Default::default()
//         };
//         // MBCPool::on_pswap_burned(remint_info);
//         // assert_ok!(MBCPool::claim_incentives(Origin::signed(alice())));
//         // assert_ok!(MBCPool::claim_incentives(Origin::signed(bob())));
//         let (_, empty_owned_alice) = MBCPool::rewards(&alice());
//         let (_, empty_owned_bob) = MBCPool::rewards(&bob());
//         assert!(empty_owned_alice.is_zero());
//         assert!(empty_owned_bob.is_zero());
//         assert_eq!(Assets::free_balance(&PSWAP, &alice()).unwrap(), owned_alice);
//         assert_eq!(Assets::free_balance(&PSWAP, &bob()).unwrap(), owned_bob);
//     });
// }
