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
use crate::vesting_currencies::{
    LinearPendingVestingSchedule, LinearVestingSchedule, VestingScheduleVariant,
};
use crate::Error::ArithmeticError;
use crate::{
    mock::*, Claim, CrowdloanInfo, CrowdloanInfos, CrowdloanUserInfo, CrowdloanUserInfos, Event,
};
#[cfg(feature = "wip")] // Auto Vesting
use crate::{ClaimSchedules, PendingClaims};
use crate::{Error, RewardInfo};
#[cfg(feature = "wip")] // ORML multi asset vesting
use crate::{VestingSchedules, VESTING_LOCK_ID};
use common::mock::charlie;
use common::{
    balance, AssetId32, AssetInfoProvider, Balance, CrowdloanTag, OnPswapBurned, PredefinedAssetId,
    PswapRemintInfo, RewardReason, Vesting, DOT, KSM, PSWAP, VAL, XOR, XSTUSD,
};
use frame_support::traits::{GetStorageVersion, OnRuntimeUpgrade, StorageVersion};
use frame_support::weights::Weight;
use frame_support::{assert_err, assert_noop, assert_ok};
use frame_system::RawOrigin;
use sp_core::bounded::BoundedVec;
use sp_runtime::traits::Dispatchable;
use tokens::BalanceLock;
use traits::currency::MultiCurrency;

fn deposit_rewards_to_reserves(amount: Balance) {
    Currencies::deposit(PSWAP, &GetBondingCurveRewardsAccountId::get(), amount).unwrap();
    Currencies::deposit(PSWAP, &GetMarketMakerRewardsAccountId::get(), amount).unwrap();
}

pub fn assert_balances(balances: Vec<(AccountId, AssetId32<PredefinedAssetId>, Balance)>) {
    for (account, asset, balance) in balances {
        assert_eq!(
            Assets::total_balance(&asset, &account),
            Ok(balance),
            "balance assert failed, account: {}, asset: {}, balance: {}",
            account,
            asset,
            balance
        );
    }
}

#[test]
fn register_crowdloan_fails() {
    ExtBuilder::default().build().execute_with(|| {
        let tag = CrowdloanTag(b"crowdloan".to_vec().try_into().unwrap());
        assert_err!(
            VestedRewards::register_crowdloan(
                RuntimeOrigin::signed(alice()),
                tag.clone(),
                0,
                100,
                vec![(XOR, balance!(100)), (PSWAP, balance!(1000))],
                vec![(alice(), balance!(5)), (bob(), balance!(15)),],
            ),
            sp_runtime::traits::BadOrigin
        );
        assert_err!(
            VestedRewards::register_crowdloan(
                RuntimeOrigin::root(),
                tag.clone(),
                0,
                100,
                vec![],
                vec![(alice(), balance!(5)), (bob(), balance!(15)),],
            ),
            Error::<Runtime>::WrongCrowdloanInfo
        );
        assert_err!(
            VestedRewards::register_crowdloan(
                RuntimeOrigin::root(),
                tag.clone(),
                0,
                100,
                vec![(XOR, balance!(100)), (PSWAP, balance!(1000))],
                vec![],
            ),
            Error::<Runtime>::WrongCrowdloanInfo
        );
        assert_err!(
            VestedRewards::register_crowdloan(
                RuntimeOrigin::root(),
                tag.clone(),
                0,
                100,
                vec![],
                vec![],
            ),
            Error::<Runtime>::WrongCrowdloanInfo
        );
        assert_ok!(VestedRewards::register_crowdloan(
            RuntimeOrigin::root(),
            tag.clone(),
            0,
            100,
            vec![(XOR, balance!(100)), (PSWAP, balance!(1000))],
            vec![(alice(), balance!(5)), (bob(), balance!(15)),],
        ),);
        assert_err!(
            VestedRewards::register_crowdloan(
                RuntimeOrigin::root(),
                tag.clone(),
                0,
                100,
                vec![(XOR, balance!(100)), (PSWAP, balance!(1000))],
                vec![(alice(), balance!(5)), (bob(), balance!(15)),],
            ),
            Error::<Runtime>::CrowdloanAlreadyExists
        );
    });
}

#[test]
fn can_claim_crowdloan_reward() {
    ExtBuilder::default().build().execute_with(|| {
        const BLOCKS_PER_DAY: u64 = 14400;
        let tag = CrowdloanTag(b"crowdloan".to_vec().try_into().unwrap());
        assert_eq!(CrowdloanUserInfos::<Runtime>::get(alice(), &tag), None);
        assert_ok!(VestedRewards::register_crowdloan(
            RuntimeOrigin::root(),
            tag.clone(),
            BLOCKS_PER_DAY,
            BLOCKS_PER_DAY * 10,
            vec![(XOR, balance!(100)), (PSWAP, balance!(1000))],
            vec![
                (alice(), balance!(5)),
                (bob(), balance!(15)),
                (charlie(), balance!(17)),
            ],
        ));
        assert_eq!(
            CrowdloanUserInfos::<Runtime>::get(alice(), &tag).unwrap(),
            CrowdloanUserInfo {
                contribution: balance!(5),
                rewarded: vec![]
            }
        );
        let crowdloan_info = CrowdloanInfos::<Runtime>::get(&tag).unwrap();
        assert_eq!(
            crowdloan_info,
            CrowdloanInfo {
                total_contribution: balance!(37),
                rewards: vec![(XOR, balance!(100)), (PSWAP, balance!(1000))],
                start_block: BLOCKS_PER_DAY,
                length: BLOCKS_PER_DAY * 10,
                account: AccountId::new(hex_literal::hex!(
                    "54734f90f971a02c609b2d684e61b557de7868ad5b1d7ffb3f91907dd08d728a"
                ))
            }
        );
        assert_balances(vec![
            (alice(), XOR, balance!(0)),
            (alice(), PSWAP, balance!(0)),
        ]);
        // Too early claim
        assert_err!(
            VestedRewards::claim_crowdloan_rewards(RuntimeOrigin::signed(alice()), tag.clone()),
            Error::<Runtime>::CrowdloanRewardsDistributionNotStarted
        );
        assert_balances(vec![
            (alice(), XOR, balance!(0)),
            (alice(), PSWAP, balance!(0)),
        ]);
        frame_system::Pallet::<Runtime>::set_block_number(BLOCKS_PER_DAY * 2);
        // Empty crowdloan tech account
        assert_err!(
            VestedRewards::claim_crowdloan_rewards(RuntimeOrigin::signed(alice()), tag.clone()),
            pallet_balances::Error::<Runtime>::InsufficientBalance
        );
        assert_balances(vec![
            (alice(), XOR, balance!(0)),
            (alice(), PSWAP, balance!(0)),
        ]);
        assert_eq!(
            CrowdloanUserInfos::<Runtime>::get(alice(), &tag).unwrap(),
            CrowdloanUserInfo {
                contribution: balance!(5),
                rewarded: vec![]
            }
        );
        Assets::mint_unchecked(&XOR, &crowdloan_info.account, balance!(100)).unwrap();
        Assets::mint_unchecked(&PSWAP, &crowdloan_info.account, balance!(1000)).unwrap();
        assert_ok!(VestedRewards::claim_crowdloan_rewards(
            RuntimeOrigin::signed(alice()),
            tag.clone()
        ),);
        assert_balances(vec![
            (alice(), XOR, balance!(1.351351351351351350)),
            (alice(), PSWAP, balance!(13.513513513513513500)),
        ]);
        assert_eq!(
            CrowdloanUserInfos::<Runtime>::get(alice(), &tag).unwrap(),
            CrowdloanUserInfo {
                contribution: balance!(5),
                rewarded: vec![
                    (XOR, balance!(1.351351351351351350)),
                    (PSWAP, balance!(13.513513513513513500))
                ]
            }
        );
        frame_system::Pallet::<Runtime>::set_block_number(BLOCKS_PER_DAY * 3 + BLOCKS_PER_DAY / 2);
        assert_ok!(VestedRewards::claim_crowdloan_rewards(
            RuntimeOrigin::signed(alice()),
            tag.clone()
        ),);
        assert_balances(vec![
            (alice(), XOR, balance!(2.702702702702702700)),
            (alice(), PSWAP, balance!(27.027027027027027000)),
        ]);
        assert_eq!(
            CrowdloanUserInfos::<Runtime>::get(alice(), &tag).unwrap(),
            CrowdloanUserInfo {
                contribution: balance!(5),
                rewarded: vec![
                    (XOR, balance!(2.702702702702702700)),
                    (PSWAP, balance!(27.027027027027027000))
                ]
            }
        );
        frame_system::Pallet::<Runtime>::set_block_number(BLOCKS_PER_DAY * 11);
        assert_ok!(VestedRewards::claim_crowdloan_rewards(
            RuntimeOrigin::signed(alice()),
            tag.clone()
        ),);
        assert_ok!(VestedRewards::claim_crowdloan_rewards(
            RuntimeOrigin::signed(bob()),
            tag.clone()
        ),);
        assert_ok!(VestedRewards::claim_crowdloan_rewards(
            RuntimeOrigin::signed(charlie()),
            tag.clone()
        ),);
        assert_balances(vec![
            (alice(), XOR, balance!(13.513513513513513500)),
            (alice(), PSWAP, balance!(135.135135135135135000)),
            (bob(), XOR, balance!(40.540540540540540500)),
            (bob(), PSWAP, balance!(405.40540540540540500)),
            (charlie(), XOR, balance!(45.945945945945945900)),
            (charlie(), PSWAP, balance!(459.45945945945945900)),
            // It's ok to have some dust after distribution because of calculations precision
            (
                crowdloan_info.account.clone(),
                XOR,
                balance!(0.0000000000000001),
            ),
            (
                crowdloan_info.account.clone(),
                PSWAP,
                balance!(0.000000000000001),
            ),
        ]);
        assert_eq!(
            Assets::total_balance(&XOR, &alice()).unwrap()
                + Assets::total_balance(&XOR, &bob()).unwrap()
                + Assets::total_balance(&XOR, &charlie()).unwrap(),
            balance!(99.999999999999999900)
        );
        assert_eq!(
            Assets::total_balance(&PSWAP, &alice()).unwrap()
                + Assets::total_balance(&PSWAP, &bob()).unwrap()
                + Assets::total_balance(&PSWAP, &charlie()).unwrap(),
            balance!(999.999999999999999000)
        );
    });
}

#[test]
fn migration_to_v2_works() {
    ExtBuilder::default().build().execute_with(|| {
        let claim_history = include_str!("../claim_history.json");
        let claim_history: Vec<(AccountId, AssetId32<PredefinedAssetId>, BlockNumber)> =
            serde_json::from_str(claim_history).unwrap();
        for (account, asset, block) in claim_history {
            crate::migrations::v4::CrowdloanClaimHistory::<Runtime>::insert(account, asset, block);
        }
        let crowdloan_rewards = include_str!("../crowdloan_rewards.json");
        let crowdloan_rewards: Vec<crate::migrations::v4::CrowdloanReward> =
            serde_json::from_str(crowdloan_rewards).unwrap();
        for reward in crowdloan_rewards {
            let account = AccountId::new(reward.address.clone().try_into().unwrap());
            crate::migrations::v4::CrowdloanRewards::<Runtime>::insert(account, reward);
        }
        StorageVersion::new(3).put::<crate::Pallet<Runtime>>();
        crate::migrations::v4::Migration::<Runtime>::on_runtime_upgrade();
        assert_eq!(crate::Pallet::<Runtime>::on_chain_storage_version(), 4);
        let info = crate::CrowdloanInfos::<Runtime>::get(CrowdloanTag(
            b"crowdloan".to_vec().try_into().unwrap(),
        ))
        .unwrap();
        assert_eq!(
            info,
            CrowdloanInfo {
                total_contribution: balance!(9653.713265551300000000),
                rewards: vec![
                    (PSWAP, balance!(9363480)),
                    (VAL, balance!(676393)),
                    (XSTUSD, balance!(77050))
                ],
                start_block: 4397212,
                length: 4579200,
                account: AccountId::new(hex_literal::hex!(
                    "54734f90f971a02c609b2d684e61b557de7868ad5b1d7ffb3f91907dd08d728a"
                ))
            }
        )
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
        VestedRewards::on_pswap_burned(PswapRemintInfo {
            vesting: balance!(170),
            ..Default::default()
        });
        assert_eq!(
            VestedRewards::rewards(&alice()),
            RewardInfo {
                limit: balance!(100),
                total_available: balance!(100),
                rewards: [(RewardReason::BuyOnBondingCurve, balance!(100)),]
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
                total_available: balance!(0),
                rewards: [].iter().cloned().collect(),
            }
        );
        assert_eq!(
            Assets::free_balance(&PSWAP, &alice()).unwrap(),
            balance!(100)
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
        VestedRewards::add_tbc_reward(&bob(), balance!(30)).expect("Failed to add reward.");
        VestedRewards::add_tbc_reward(&eve(), balance!(500)).expect("Failed to add reward.");

        VestedRewards::on_pswap_burned(PswapRemintInfo {
            vesting: total_rewards,
            ..Default::default()
        });
        assert_eq!(
            VestedRewards::rewards(&alice()),
            RewardInfo {
                limit: balance!(1),
                total_available: balance!(1),
                rewards: [(RewardReason::BuyOnBondingCurve, balance!(1)),]
                    .iter()
                    .cloned()
                    .collect(),
            }
        );
        assert_eq!(
            VestedRewards::rewards(&bob()),
            RewardInfo {
                limit: balance!(30),
                total_available: balance!(30),
                rewards: [(RewardReason::BuyOnBondingCurve, balance!(30)),]
                    .iter()
                    .cloned()
                    .collect(),
            }
        );
        assert_eq!(
            VestedRewards::rewards(&eve()),
            RewardInfo {
                limit: balance!(500),
                total_available: balance!(500),
                rewards: [(RewardReason::BuyOnBondingCurve, balance!(500)),]
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
        assert_eq!(Assets::free_balance(&PSWAP, &alice()).unwrap(), balance!(1));
        assert_eq!(Assets::free_balance(&PSWAP, &bob()).unwrap(), balance!(30));
        assert_eq!(Assets::free_balance(&PSWAP, &eve()).unwrap(), balance!(500));
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
        Currencies::deposit(PSWAP, &GetBondingCurveRewardsAccountId::get(), balance!(0)).unwrap();
        Currencies::deposit(PSWAP, &GetFarmingRewardsAccountId::get(), balance!(100)).unwrap();

        // reward amount greater than reserves is added
        VestedRewards::add_tbc_reward(&alice(), balance!(10)).expect("Failed to add reward.");
        VestedRewards::add_farming_reward(&alice(), balance!(20)).expect("Failed to add reward.");
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
                    (RewardReason::LiquidityProvisionFarming, balance!(20))
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
                limit: balance!(10),
                total_available: balance!(10),
                rewards: [(RewardReason::BuyOnBondingCurve, balance!(10)),]
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

        VestedRewards::on_pswap_burned(PswapRemintInfo {
            vesting: balance!(20),
            ..Default::default()
        });
        assert_eq!(
            VestedRewards::rewards(&alice()),
            RewardInfo {
                limit: balance!(10),
                total_available: balance!(10),
                rewards: [(RewardReason::BuyOnBondingCurve, balance!(10)),]
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
                total_available: balance!(0),
                rewards: [].iter().cloned().collect(),
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
fn market_maker_reward_pool_migration() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let mm_initial_reserve = balance!(400000000);
        let curve_initial_reserve = balance!(400000000);

        Currencies::deposit(
            PSWAP,
            &GetMarketMakerRewardsAccountId::get(),
            mm_initial_reserve,
        )
        .unwrap();

        Currencies::deposit(
            PSWAP,
            &GetBondingCurveRewardsAccountId::get(),
            curve_initial_reserve,
        )
        .unwrap();

        VestedRewards::add_pending_reward(
            &alice(),
            RewardReason::DeprecatedMarketMakerVolume,
            balance!(100),
        )
        .unwrap();
        VestedRewards::add_pending_reward(&alice(), RewardReason::BuyOnBondingCurve, balance!(200))
            .unwrap();

        crate::migrations::move_market_making_rewards_to_liquidity_provider_rewards_pool::<Runtime>(
        );

        assert_eq!(
            Currencies::free_balance(PSWAP, &GetBondingCurveRewardsAccountId::get()),
            mm_initial_reserve + curve_initial_reserve
        );

        assert_eq!(
            VestedRewards::rewards(&alice()),
            RewardInfo {
                limit: balance!(0),
                total_available: balance!(300),
                rewards: [(RewardReason::BuyOnBondingCurve, balance!(300))]
                    .iter()
                    .cloned()
                    .collect(),
            }
        );
    });
}

#[test]
fn update_rewards_works() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        VestedRewards::add_pending_reward(
            &alice(),
            RewardReason::DeprecatedMarketMakerVolume,
            balance!(100),
        )
        .unwrap();
        VestedRewards::add_pending_reward(
            &alice(),
            RewardReason::LiquidityProvisionFarming,
            balance!(200),
        )
        .unwrap();

        VestedRewards::add_pending_reward(
            &bob(),
            RewardReason::DeprecatedMarketMakerVolume,
            balance!(300),
        )
        .unwrap();
        VestedRewards::add_pending_reward(&bob(), RewardReason::BuyOnBondingCurve, balance!(400))
            .unwrap();

        VestedRewards::add_pending_reward(
            &charlie(),
            RewardReason::DeprecatedMarketMakerVolume,
            balance!(500),
        )
        .unwrap();
        VestedRewards::add_pending_reward(
            &charlie(),
            RewardReason::LiquidityProvisionFarming,
            balance!(600),
        )
        .unwrap();
        assert_eq!(crate::TotalRewards::<Runtime>::get(), balance!(2100));

        crate::migrations::move_market_making_rewards_to_liquidity_provider_rewards_pool::<Runtime>(
        );

        assert_eq!(crate::TotalRewards::<Runtime>::get(), balance!(2100));
        assert_eq!(
            crate::Rewards::<Runtime>::get(&alice()).total_available,
            balance!(300)
        );
        assert_eq!(
            crate::Rewards::<Runtime>::get(&alice()).rewards,
            vec![(RewardReason::LiquidityProvisionFarming, balance!(200))]
                .into_iter()
                .collect()
        );
        assert_eq!(
            crate::Rewards::<Runtime>::get(&bob()).total_available,
            balance!(700)
        );
        assert_eq!(
            crate::Rewards::<Runtime>::get(&bob()).rewards,
            vec![(RewardReason::BuyOnBondingCurve, balance!(700))]
                .into_iter()
                .collect()
        );
        assert_eq!(
            crate::Rewards::<Runtime>::get(&charlie()).total_available,
            balance!(1100)
        );
        assert_eq!(
            crate::Rewards::<Runtime>::get(&charlie()).rewards,
            vec![(RewardReason::LiquidityProvisionFarming, balance!(600))]
                .into_iter()
                .collect()
        );

        let rewards = vec![
            (
                alice(),
                vec![(RewardReason::BuyOnBondingCurve, balance!(100))]
                    .into_iter()
                    .collect(),
            ),
            (
                charlie(),
                vec![(RewardReason::BuyOnBondingCurve, balance!(500))]
                    .into_iter()
                    .collect(),
            ),
        ]
        .into_iter()
        .collect();
        assert_ok!(VestedRewards::update_rewards(
            RawOrigin::Root.into(),
            rewards
        ));

        assert_eq!(crate::TotalRewards::<Runtime>::get(), balance!(2100));
        assert_eq!(
            crate::Rewards::<Runtime>::get(&alice()).total_available,
            balance!(300)
        );
        assert_eq!(
            crate::Rewards::<Runtime>::get(&alice()).rewards,
            vec![
                (RewardReason::LiquidityProvisionFarming, balance!(200)),
                (RewardReason::BuyOnBondingCurve, balance!(100))
            ]
            .into_iter()
            .collect()
        );
        assert_eq!(
            crate::Rewards::<Runtime>::get(&bob()).total_available,
            balance!(700)
        );
        assert_eq!(
            crate::Rewards::<Runtime>::get(&bob()).rewards,
            vec![(RewardReason::BuyOnBondingCurve, balance!(700))]
                .into_iter()
                .collect()
        );
        assert_eq!(
            crate::Rewards::<Runtime>::get(&charlie()).total_available,
            balance!(1100)
        );
        assert_eq!(
            crate::Rewards::<Runtime>::get(&charlie()).rewards,
            vec![
                (RewardReason::LiquidityProvisionFarming, balance!(600)),
                (RewardReason::BuyOnBondingCurve, balance!(500))
            ]
            .into_iter()
            .collect()
        );
    });
}

// Tests for Linear Vesting and Vesting
#[cfg(feature = "wip")] // ORML multi asset vesting
#[cfg(feature = "wip")] // Pending Vesting
#[test]
fn linear_vested_transfer_works() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(1);

        let schedule = VestingScheduleVariant::LinearVestingSchedule(LinearVestingSchedule {
            asset_id: DOT,
            start: 0u64,
            period: 10u64,
            period_count: 1u32,
            per_period: 100,
            remainder_amount: 0,
        });
        let schedule_locked =
            VestingScheduleVariant::LinearPendingVestingSchedule(LinearPendingVestingSchedule {
                asset_id: DOT,
                manager_id: Some(alice()),
                start: None,
                period: 10u64,
                period_count: 1u32,
                per_period: 100,
                remainder_amount: 0,
            });
        assert_ok!(VestedRewards::vested_transfer(
            RuntimeOrigin::signed(alice()),
            bob(),
            schedule.clone()
        ));
        System::assert_last_event(RuntimeEvent::VestedRewards(
            crate::Event::VestingScheduleAdded {
                from: alice(),
                to: bob(),
                vesting_schedule: schedule.clone(),
            },
        ));
        assert_ok!(VestedRewards::vested_transfer(
            RuntimeOrigin::signed(alice()),
            bob(),
            schedule_locked.clone()
        ));
        System::assert_last_event(RuntimeEvent::VestedRewards(
            crate::Event::VestingScheduleAdded {
                from: alice(),
                to: bob(),
                vesting_schedule: schedule_locked.clone(),
            },
        ));
        assert_eq!(
            VestedRewards::vesting_schedules(&bob()),
            vec![schedule, schedule_locked]
        );
    });
}
#[cfg(feature = "wip")] // ORML multi asset vesting
#[test]
fn self_linear_vesting() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(1);

        let schedule = VestingScheduleVariant::LinearVestingSchedule(LinearVestingSchedule {
            asset_id: DOT,
            start: 0u64,
            period: 10u64,
            period_count: 1u32,
            per_period: ALICE_BALANCE,
            remainder_amount: 0,
        });

        let schedule_ksm = VestingScheduleVariant::LinearVestingSchedule(LinearVestingSchedule {
            asset_id: KSM,
            start: 0u64,
            period: 10u64,
            period_count: 1u32,
            per_period: ALICE_BALANCE,
            remainder_amount: 0,
        });

        let bad_schedule = VestingScheduleVariant::LinearVestingSchedule(LinearVestingSchedule {
            asset_id: DOT,
            start: 0u64,
            period: 10u64,
            period_count: 1u32,
            per_period: 10 * ALICE_BALANCE,
            remainder_amount: 0,
        });

        assert_noop!(
            VestedRewards::vested_transfer(RuntimeOrigin::signed(alice()), alice(), bad_schedule),
            crate::Error::<Runtime>::InsufficientBalanceToLock
        );

        assert_ok!(VestedRewards::vested_transfer(
            RuntimeOrigin::signed(alice()),
            alice(),
            schedule.clone()
        ));
        assert_ok!(VestedRewards::vested_transfer(
            RuntimeOrigin::signed(alice()),
            alice(),
            schedule_ksm.clone()
        ));

        assert_eq!(
            VestedRewards::vesting_schedules(&alice()),
            vec![schedule.clone(), schedule_ksm.clone()]
        );

        System::assert_last_event(RuntimeEvent::VestedRewards(
            crate::Event::VestingScheduleAdded {
                from: alice(),
                to: alice(),
                vesting_schedule: schedule_ksm,
            },
        ));
    });
}
#[cfg(feature = "wip")] // ORML multi asset vesting
#[cfg(feature = "wip")] // Pending Vesting
#[test]
fn add_new_vesting_schedule_merges_with_current_locked_balance_and_until() {
    ExtBuilder::default().build().execute_with(|| {
        let schedule = VestingScheduleVariant::LinearVestingSchedule(LinearVestingSchedule {
            asset_id: DOT,
            start: 0u64,
            period: 10u64,
            period_count: 3u32,
            per_period: 10,
            remainder_amount: 1,
        });
        assert_ok!(VestedRewards::vested_transfer(
            RuntimeOrigin::signed(alice()),
            bob(),
            schedule
        ));

        let schedule_ksm = VestingScheduleVariant::LinearVestingSchedule(LinearVestingSchedule {
            asset_id: KSM,
            start: 0u64,
            period: 10u64,
            period_count: 2u32,
            per_period: 10,
            remainder_amount: 0,
        });
        assert_ok!(VestedRewards::vested_transfer(
            RuntimeOrigin::signed(alice()),
            bob(),
            schedule_ksm
        ));

        run_to_block(12);

        let another_schedule =
            VestingScheduleVariant::LinearVestingSchedule(LinearVestingSchedule {
                asset_id: DOT,
                start: 10u64,
                period: 13u64,
                period_count: 2u32,
                per_period: 7,
                remainder_amount: 1,
            });

        assert_ok!(VestedRewards::vested_transfer(
            RuntimeOrigin::signed(alice()),
            bob(),
            another_schedule
        ));

        let another_schedule_locked =
            VestingScheduleVariant::LinearPendingVestingSchedule(LinearPendingVestingSchedule {
                asset_id: DOT,
                manager_id: Some(alice()),
                start: None,
                period: 13u64,
                period_count: 1u32,
                per_period: 7,
                remainder_amount: 0,
            });

        assert_ok!(VestedRewards::vested_transfer(
            RuntimeOrigin::signed(alice()),
            bob(),
            another_schedule_locked
        ));

        let another_schedule_ksm =
            VestingScheduleVariant::LinearVestingSchedule(LinearVestingSchedule {
                asset_id: KSM,
                start: 10u64,
                period: 13u64,
                period_count: 1u32,
                per_period: 7,
                remainder_amount: 0,
            });

        assert_ok!(VestedRewards::vested_transfer(
            RuntimeOrigin::signed(alice()),
            bob(),
            another_schedule_ksm
        ));

        assert_eq!(
            Tokens::locks(&bob(), DOT).get(0),
            Some(&BalanceLock {
                id: VESTING_LOCK_ID,
                amount: 26,
            })
        );
        assert_eq!(
            Tokens::locks(&bob(), KSM).get(0),
            Some(&BalanceLock {
                id: VESTING_LOCK_ID,
                amount: 17,
            })
        );
    });
}

#[cfg(feature = "wip")] // ORML multi asset vesting
#[cfg(feature = "wip")] // Pending Vesting
#[test]
fn cannot_use_fund_if_not_claimed_from_linear() {
    ExtBuilder::default().build().execute_with(|| {
        let schedule = VestingScheduleVariant::LinearVestingSchedule(LinearVestingSchedule {
            asset_id: DOT,
            start: 10u64,
            period: 10u64,
            period_count: 1u32,
            per_period: 50,
            remainder_amount: 0,
        });
        assert_ok!(VestedRewards::vested_transfer(
            RuntimeOrigin::signed(alice()),
            bob(),
            schedule
        ));
        let schedule_locked =
            VestingScheduleVariant::LinearPendingVestingSchedule(LinearPendingVestingSchedule {
                asset_id: DOT,
                manager_id: Some(alice()),
                start: None,
                period: 10u64,
                period_count: 1u32,
                per_period: 50,
                remainder_amount: 0,
            });
        assert_ok!(VestedRewards::vested_transfer(
            RuntimeOrigin::signed(alice()),
            bob(),
            schedule_locked
        ));
        assert!(Tokens::ensure_can_withdraw(DOT, &bob(), 51).is_err())
    });
}

#[cfg(feature = "wip")] // Pending Vesting
#[test]
fn linear_vesting_unlock_correct() {
    ExtBuilder::default().build().execute_with(|| {
        let schedule_locked =
            VestingScheduleVariant::LinearPendingVestingSchedule(LinearPendingVestingSchedule {
                asset_id: DOT,
                manager_id: Some(alice()),
                start: None,
                period: 10u64,
                period_count: 2u32,
                per_period: 100,
                remainder_amount: 2,
            });

        let schedule_unlocked =
            VestingScheduleVariant::LinearPendingVestingSchedule(LinearPendingVestingSchedule {
                asset_id: DOT,
                manager_id: None,
                start: Some(12_u64),
                period: 10u64,
                period_count: 2u32,
                per_period: 100,
                remainder_amount: 2,
            });

        assert_ok!(VestedRewards::vested_transfer(
            RuntimeOrigin::signed(alice()),
            bob(),
            schedule_locked.clone()
        ));
        assert_err!(
            VestedRewards::unlock_pending_schedule_by_manager(
                RuntimeOrigin::signed(bob()),
                bob(),
                None,
                schedule_locked.clone(),
            ),
            Error::<Runtime>::PendingScheduleNotExist
        );
        run_to_block(12);
        assert_ok!(VestedRewards::unlock_pending_schedule_by_manager(
            RuntimeOrigin::signed(alice()),
            bob(),
            None,
            schedule_locked,
        ));
        System::assert_last_event(RuntimeEvent::VestedRewards(
            Event::PendingScheduleUnlocked {
                dest: bob(),
                pending_schedule: schedule_unlocked.clone(),
            },
        ));
        assert_eq!(
            VestedRewards::vesting_schedules(&bob()),
            vec![schedule_unlocked]
        );
    })
}

#[cfg(feature = "wip")] // ORML multi asset vesting
#[cfg(feature = "wip")] // Pending Vesting
#[test]
fn linear_vested_transfer_fails_if_zero_period_or_count() {
    ExtBuilder::default().build().execute_with(|| {
        let schedule = VestingScheduleVariant::LinearVestingSchedule(LinearVestingSchedule {
            asset_id: DOT,
            start: 1u64,
            period: 0u64,
            period_count: 1u32,
            per_period: 100,
            remainder_amount: 0,
        });
        let schedule_locked =
            VestingScheduleVariant::LinearPendingVestingSchedule(LinearPendingVestingSchedule {
                asset_id: DOT,
                manager_id: Some(alice()),
                start: None,
                period: 0u64,
                period_count: 1u32,
                per_period: 100,
                remainder_amount: 0,
            });
        assert_noop!(
            VestedRewards::vested_transfer(RuntimeOrigin::signed(alice()), bob(), schedule),
            Error::<Runtime>::ZeroVestingPeriod
        );
        assert_noop!(
            VestedRewards::vested_transfer(RuntimeOrigin::signed(alice()), bob(), schedule_locked),
            Error::<Runtime>::ZeroVestingPeriod
        );

        let schedule = VestingScheduleVariant::LinearVestingSchedule(LinearVestingSchedule {
            asset_id: DOT,
            start: 1u64,
            period: 1u64,
            period_count: 0u32,
            per_period: 100,
            remainder_amount: 0,
        });
        let schedule_with_remainder =
            VestingScheduleVariant::LinearVestingSchedule(LinearVestingSchedule {
                asset_id: DOT,
                start: 1u64,
                period: 1u64,
                period_count: 1u32,
                per_period: 100,
                remainder_amount: 1,
            });
        let schedule_locked =
            VestingScheduleVariant::LinearPendingVestingSchedule(LinearPendingVestingSchedule {
                asset_id: DOT,
                manager_id: Some(alice()),
                start: None,
                period: 1u64,
                period_count: 0u32,
                per_period: 100,
                remainder_amount: 0,
            });
        let schedule_locked_with_remainder =
            VestingScheduleVariant::LinearPendingVestingSchedule(LinearPendingVestingSchedule {
                asset_id: DOT,
                manager_id: Some(alice()),
                start: None,
                period: 1u64,
                period_count: 1u32,
                per_period: 100,
                remainder_amount: 1,
            });
        assert_noop!(
            VestedRewards::vested_transfer(RuntimeOrigin::signed(alice()), bob(), schedule),
            Error::<Runtime>::WrongVestingPeriodCount
        );
        assert_noop!(
            VestedRewards::vested_transfer(
                RuntimeOrigin::signed(alice()),
                bob(),
                schedule_with_remainder
            ),
            Error::<Runtime>::WrongVestingPeriodCount
        );
        assert_noop!(
            VestedRewards::vested_transfer(RuntimeOrigin::signed(alice()), bob(), schedule_locked),
            Error::<Runtime>::WrongVestingPeriodCount
        );
        assert_noop!(
            VestedRewards::vested_transfer(
                RuntimeOrigin::signed(alice()),
                bob(),
                schedule_locked_with_remainder
            ),
            Error::<Runtime>::WrongVestingPeriodCount
        );
    });
}

#[cfg(feature = "wip")] // ORML multi asset vesting
#[cfg(feature = "wip")] // Pending Vesting
#[test]
fn vested_transfer_fails_if_transfer_err() {
    ExtBuilder::default().build().execute_with(|| {
        let schedule = VestingScheduleVariant::LinearVestingSchedule(LinearVestingSchedule {
            asset_id: DOT,
            start: 1u64,
            period: 1u64,
            period_count: 1u32,
            per_period: 100,
            remainder_amount: 0,
        });
        let schedule_locked =
            VestingScheduleVariant::LinearPendingVestingSchedule(LinearPendingVestingSchedule {
                asset_id: DOT,
                manager_id: None::<AccountId>,
                start: Some(1_u64),
                period: 1u64,
                period_count: 1u32,
                per_period: 100,
                remainder_amount: 0,
            });
        assert_noop!(
            VestedRewards::vested_transfer(RuntimeOrigin::signed(bob()), alice(), schedule),
            tokens::Error::<Runtime>::BalanceTooLow
        );
        assert_noop!(
            VestedRewards::vested_transfer(RuntimeOrigin::signed(bob()), alice(), schedule_locked),
            tokens::Error::<Runtime>::BalanceTooLow
        );
    });
}
#[cfg(feature = "wip")] // ORML multi asset vesting
#[cfg(feature = "wip")] // Pending Vesting
#[test]
fn vested_linear_transfer_and_unlock_pending_fails_if_overflow() {
    ExtBuilder::default().build().execute_with(|| {
        let schedule = VestingScheduleVariant::LinearVestingSchedule(LinearVestingSchedule {
            asset_id: DOT,
            start: 1u64,
            period: 1u64,
            period_count: 2u32,
            per_period: Balance::MAX,
            remainder_amount: 0,
        });
        let schedule_remainder =
            VestingScheduleVariant::LinearVestingSchedule(LinearVestingSchedule {
                asset_id: DOT,
                start: 1u64,
                period: 1u64,
                period_count: 2u32,
                per_period: 1,
                remainder_amount: Balance::MAX,
            });
        let schedule_locked =
            VestingScheduleVariant::LinearPendingVestingSchedule(LinearPendingVestingSchedule {
                asset_id: DOT,
                manager_id: None::<AccountId>,
                start: Some(1_u64),
                period: 10u64,
                period_count: 1000u32,
                per_period: Balance::MAX,
                remainder_amount: 0,
            });
        let schedule_locked_remainder =
            VestingScheduleVariant::LinearPendingVestingSchedule(LinearPendingVestingSchedule {
                asset_id: DOT,
                manager_id: None::<AccountId>,
                start: Some(1_u64),
                period: 10u64,
                period_count: 1000u32,
                per_period: 1,
                remainder_amount: Balance::MAX,
            });
        assert_noop!(
            VestedRewards::vested_transfer(RuntimeOrigin::signed(alice()), bob(), schedule),
            ArithmeticError::<Runtime>,
        );
        assert_noop!(
            VestedRewards::vested_transfer(RuntimeOrigin::signed(alice()), bob(), schedule_locked),
            ArithmeticError::<Runtime>,
        );
        assert_noop!(
            VestedRewards::vested_transfer(
                RuntimeOrigin::signed(alice()),
                bob(),
                schedule_remainder
            ),
            ArithmeticError::<Runtime>,
        );
        assert_noop!(
            VestedRewards::vested_transfer(
                RuntimeOrigin::signed(alice()),
                bob(),
                schedule_locked_remainder
            ),
            ArithmeticError::<Runtime>,
        );

        let schedule_locked_right =
            VestingScheduleVariant::LinearPendingVestingSchedule(LinearPendingVestingSchedule {
                asset_id: DOT,
                manager_id: Some(alice()),
                start: Some(1_u64),
                period: 1u64,
                period_count: 10u32,
                per_period: 1,
                remainder_amount: 1,
            });
        assert_ok!(VestedRewards::vested_transfer(
            RuntimeOrigin::signed(alice()),
            bob(),
            schedule_locked_right.clone()
        ));
        assert_noop!(
            VestedRewards::unlock_pending_schedule_by_manager(
                RuntimeOrigin::signed(alice()),
                bob(),
                Some(u64::MAX),
                schedule_locked_right
            ),
            ArithmeticError::<Runtime>,
        );

        let another_schedule =
            VestingScheduleVariant::LinearVestingSchedule(LinearVestingSchedule {
                asset_id: DOT,
                start: u64::MAX,
                period: 1u64,
                period_count: 2u32,
                per_period: 1,
                remainder_amount: 0,
            });

        assert_noop!(
            VestedRewards::vested_transfer(RuntimeOrigin::signed(alice()), bob(), another_schedule),
            ArithmeticError::<Runtime>,
        );

        run_to_block(20);
        let schedule_locked_right =
            VestingScheduleVariant::LinearPendingVestingSchedule(LinearPendingVestingSchedule {
                asset_id: DOT,
                manager_id: Some(alice()),
                start: None,
                period: 3689348814741910320u64,
                period_count: 5u32,
                per_period: 1,
                remainder_amount: 0,
            });
        assert_noop!(
            VestedRewards::vested_transfer(
                RuntimeOrigin::signed(alice()),
                bob(),
                schedule_locked_right.clone()
            ),
            Error::<Runtime>::ArithmeticError
        );
    });
}
#[cfg(feature = "wip")] // ORML multi asset vesting
#[test]
fn vested_transfer_check_for_min() {
    ExtBuilder::default().build().execute_with(|| {
        let schedule = VestingScheduleVariant::LinearVestingSchedule(LinearVestingSchedule {
            asset_id: DOT,
            start: 1u64,
            period: 10u64,
            period_count: 2u32,
            per_period: 2,
            remainder_amount: 1,
        });
        assert_noop!(
            VestedRewards::vested_transfer(RuntimeOrigin::signed(bob()), alice(), schedule),
            Error::<Runtime>::AmountLow
        );
    });
}
#[cfg(feature = "wip")] // ORML multi asset vesting
#[cfg(feature = "wip")] // Pending Vesting
#[test]
fn claim_linear_works() {
    ExtBuilder::default().build().execute_with(|| {
        let schedule = VestingScheduleVariant::LinearVestingSchedule(LinearVestingSchedule {
            asset_id: DOT,
            start: 0u64,
            period: 10u64,
            period_count: 2u32,
            per_period: 10,
            remainder_amount: 10,
        });
        let schedule_locked =
            VestingScheduleVariant::LinearPendingVestingSchedule(LinearPendingVestingSchedule {
                asset_id: DOT,
                manager_id: Some(alice()),
                start: None,
                period: 10u64,
                period_count: 2u32,
                per_period: 10,
                remainder_amount: 10,
            });
        let schedule_ksm = VestingScheduleVariant::LinearVestingSchedule(LinearVestingSchedule {
            asset_id: KSM,
            start: 0u64,
            period: 50u64,
            period_count: 1u32,
            per_period: 10,
            remainder_amount: 0,
        });
        assert_ok!(VestedRewards::vested_transfer(
            RuntimeOrigin::signed(alice()),
            bob(),
            schedule
        ));
        assert_ok!(VestedRewards::vested_transfer(
            RuntimeOrigin::signed(alice()),
            bob(),
            schedule_locked.clone()
        ));

        assert_ok!(VestedRewards::unlock_pending_schedule_by_manager(
            RuntimeOrigin::signed(alice()),
            bob(),
            Some(0_u64),
            schedule_locked
        ),);
        assert_ok!(VestedRewards::vested_transfer(
            RuntimeOrigin::signed(alice()),
            bob(),
            schedule_ksm
        ));

        // not run because hook would claim
        System::set_block_number(11);

        // remain locked if not claimed
        assert!(Tokens::transfer(RuntimeOrigin::signed(bob()), alice(), DOT, 20).is_err());
        // unlocked after claiming
        assert_ok!(VestedRewards::claim_unlocked(
            RuntimeOrigin::signed(bob()),
            DOT
        ));
        assert!(VestingSchedules::<Runtime>::contains_key(bob()));
        assert_ok!(Tokens::transfer(
            RuntimeOrigin::signed(bob()),
            alice(),
            DOT,
            20,
        ));
        // more are still locked
        assert!(Tokens::transfer(RuntimeOrigin::signed(bob()), alice(), DOT, 1).is_err());

        System::set_block_number(21);
        // claim more
        assert_ok!(VestedRewards::claim_unlocked(
            RuntimeOrigin::signed(bob()),
            DOT
        ));
        assert!(VestingSchedules::<Runtime>::contains_key(bob()));
        assert_ok!(Tokens::transfer(
            RuntimeOrigin::signed(bob()),
            alice(),
            DOT,
            20,
        ));
        // all used up
        assert_eq!(Tokens::free_balance(DOT, &bob()), 0);

        // no locks anymore
        assert_eq!(Tokens::locks(bob(), DOT), vec![]);
        System::set_block_number(50);
        assert_ok!(VestedRewards::claim_unlocked(
            RuntimeOrigin::signed(bob()),
            KSM
        ));
        assert!(!VestingSchedules::<Runtime>::contains_key(bob()));
    });
}
#[cfg(feature = "wip")] // ORML multi asset vesting
#[test]
fn claim_for_works() {
    ExtBuilder::default().build().execute_with(|| {
        let schedule = VestingScheduleVariant::LinearVestingSchedule(LinearVestingSchedule {
            asset_id: DOT,
            start: 0u64,
            period: 10u64,
            period_count: 2u32,
            per_period: 10,
            remainder_amount: 10,
        });

        assert_ok!(VestedRewards::vested_transfer(
            RuntimeOrigin::signed(alice()),
            bob(),
            schedule
        ));

        assert_ok!(VestedRewards::claim_for(
            RuntimeOrigin::signed(alice()),
            DOT,
            bob()
        ));

        assert_eq!(
            Tokens::locks(bob(), DOT).get(0),
            Some(&BalanceLock {
                id: VESTING_LOCK_ID,
                amount: 20,
            })
        );
        assert!(VestingSchedules::<Runtime>::contains_key(&bob()));

        run_to_block(21);

        assert_ok!(VestedRewards::claim_for(
            RuntimeOrigin::signed(alice()),
            DOT,
            bob()
        ));

        // no locks anymore
        assert_eq!(Tokens::locks(bob(), DOT), vec![]);
        assert!(!VestingSchedules::<Runtime>::contains_key(&bob()));
    });
}
#[cfg(feature = "wip")] // ORML multi asset vesting
#[cfg(feature = "wip")] // Pending Vesting
#[test]
fn update_vesting_schedules_works() {
    ExtBuilder::default().build().execute_with(|| {
        let schedule = VestingScheduleVariant::LinearVestingSchedule(LinearVestingSchedule {
            asset_id: DOT,
            start: 0u64,
            period: 10u64,
            period_count: 2u32,
            per_period: 10,
            remainder_amount: 10,
        });
        assert_ok!(VestedRewards::vested_transfer(
            RuntimeOrigin::signed(alice()),
            bob(),
            schedule
        ));

        let schedule_ksm = VestingScheduleVariant::LinearVestingSchedule(LinearVestingSchedule {
            asset_id: KSM,
            start: 0u64,
            period: 10u64,
            period_count: 2u32,
            per_period: 10,
            remainder_amount: 0,
        });
        assert_ok!(VestedRewards::vested_transfer(
            RuntimeOrigin::signed(alice()),
            bob(),
            schedule_ksm
        ));

        let schedule_locked =
            VestingScheduleVariant::LinearPendingVestingSchedule(LinearPendingVestingSchedule {
                asset_id: DOT,
                manager_id: Some(alice()),
                start: None,
                period: 10u64,
                period_count: 2u32,
                per_period: 10,
                remainder_amount: 0,
            });
        assert_ok!(VestedRewards::vested_transfer(
            RuntimeOrigin::signed(alice()),
            bob(),
            schedule_locked
        ));

        let updated_schedule =
            VestingScheduleVariant::LinearVestingSchedule(LinearVestingSchedule {
                asset_id: DOT,
                start: 0u64,
                period: 20u64,
                period_count: 2u32,
                per_period: 10,
                remainder_amount: 0,
            });
        let updated_schedule_locked =
            VestingScheduleVariant::LinearPendingVestingSchedule(LinearPendingVestingSchedule {
                asset_id: DOT,
                manager_id: Some(alice()),
                start: Some(0),
                period: 20u64,
                period_count: 2u32,
                per_period: 5,
                remainder_amount: 5,
            });
        assert_ok!(VestedRewards::update_vesting_schedules(
            RuntimeOrigin::root(),
            bob(),
            BoundedVec::try_from(vec![updated_schedule, updated_schedule_locked]).unwrap(),
        ));

        assert_eq!(Tokens::free_balance(KSM, &bob()), 20);
        assert!(Tokens::locks(bob(), KSM).get(0).is_none());
        run_to_block(11);
        assert_ok!(VestedRewards::claim_unlocked(
            RuntimeOrigin::signed(bob()),
            DOT
        ));
        assert!(Tokens::transfer(RuntimeOrigin::signed(bob()), alice(), DOT, 11).is_err());

        run_to_block(21);
        assert_ok!(VestedRewards::claim_unlocked(
            RuntimeOrigin::signed(bob()),
            DOT
        ));
        assert_ok!(Tokens::transfer(
            RuntimeOrigin::signed(bob()),
            alice(),
            DOT,
            15,
        ));

        // empty vesting schedules cleanup the storage and unlock the fund
        assert!(VestingSchedules::<Runtime>::contains_key(bob()));
        assert_eq!(
            Tokens::locks(bob(), DOT).get(0),
            Some(&BalanceLock {
                id: VESTING_LOCK_ID,
                amount: 15,
            })
        );
        assert_ok!(VestedRewards::update_vesting_schedules(
            RuntimeOrigin::root(),
            bob(),
            BoundedVec::default()
        ));
        assert!(!VestingSchedules::<Runtime>::contains_key(bob()));
        assert_eq!(Tokens::locks(bob(), DOT), vec![]);
    });
}
#[cfg(feature = "wip")] // ORML multi asset vesting
#[test]
fn multiple_vesting_linear_schedule_claim_works() {
    ExtBuilder::default().build().execute_with(|| {
        let schedule = VestingScheduleVariant::LinearVestingSchedule(LinearVestingSchedule {
            asset_id: DOT,
            start: 0u64,
            period: 10u64,
            period_count: 2u32,
            per_period: 10,
            remainder_amount: 0,
        });
        assert_ok!(VestedRewards::vested_transfer(
            RuntimeOrigin::signed(alice()),
            bob(),
            schedule.clone()
        ));
        let schedule2 = VestingScheduleVariant::LinearVestingSchedule(LinearVestingSchedule {
            asset_id: DOT,
            start: 0u64,
            period: 10u64,
            period_count: 3u32,
            per_period: 10,
            remainder_amount: 10,
        });
        assert_ok!(VestedRewards::vested_transfer(
            RuntimeOrigin::signed(alice()),
            bob(),
            schedule2.clone()
        ));

        assert_eq!(
            VestedRewards::vesting_schedules(&bob()),
            vec![schedule, schedule2.clone()]
        );

        run_to_block(21);

        assert_ok!(VestedRewards::claim_unlocked(
            RuntimeOrigin::signed(bob()),
            DOT
        ));

        assert_eq!(VestedRewards::vesting_schedules(&bob()), vec![schedule2]);

        run_to_block(31);

        assert_ok!(VestedRewards::claim_unlocked(
            RuntimeOrigin::signed(bob()),
            DOT
        ));

        println!(
            "VESTING SCHEDULE BOB: {:?}",
            VestingSchedules::<Runtime>::get(bob())
        );
        println!("LOKS: {:?}", Tokens::locks(bob(), DOT));
        assert!(!VestingSchedules::<Runtime>::contains_key(&bob()));

        assert_eq!(Tokens::locks(bob(), DOT), vec![]);
    });
}
#[cfg(feature = "wip")] // ORML multi asset vesting
#[test]
fn exceeding_maximum_schedules_should_fail() {
    ExtBuilder::default().build().execute_with(|| {
        let schedule = VestingScheduleVariant::LinearVestingSchedule(LinearVestingSchedule {
            asset_id: DOT,
            start: 0u64,
            period: 10u64,
            period_count: 2u32,
            per_period: 10,
            remainder_amount: 0,
        });
        for _ in 0u32..MaxVestingSchedules::get() {
            assert_ok!(VestedRewards::vested_transfer(
                RuntimeOrigin::signed(alice()),
                bob(),
                schedule.clone()
            ));
        }

        let create = RuntimeCall::VestedRewards(crate::Call::<Runtime>::vested_transfer {
            dest: bob(),
            schedule: schedule.clone(),
        });
        assert_noop!(
            create.dispatch(RuntimeOrigin::signed(alice())),
            Error::<Runtime>::MaxVestingSchedulesExceeded
        );
    });
}
#[cfg(feature = "wip")] // ORML multi asset vesting
#[test]
fn cliff_vesting_linear_works() {
    const VESTING_AMOUNT: Balance = 12;
    const VESTING_PERIOD: u64 = 20;

    ExtBuilder::default().build().execute_with(|| {
        let cliff_schedule = VestingScheduleVariant::LinearVestingSchedule(LinearVestingSchedule {
            asset_id: DOT,
            start: VESTING_PERIOD - 1,
            period: 1,
            period_count: 1,
            per_period: VESTING_AMOUNT,
            remainder_amount: 0,
        });

        let balance_lock = BalanceLock {
            id: VESTING_LOCK_ID,
            amount: VESTING_AMOUNT,
        };

        assert_eq!(Tokens::free_balance(DOT, &bob()), 0);
        assert_ok!(VestedRewards::vested_transfer(
            RuntimeOrigin::signed(alice()),
            bob(),
            cliff_schedule
        ));
        assert_eq!(Tokens::free_balance(DOT, &bob()), VESTING_AMOUNT);
        assert_eq!(Tokens::locks(bob(), DOT), vec![balance_lock.clone()]);

        for i in 1..VESTING_PERIOD {
            run_to_block(i);
            assert_ok!(VestedRewards::claim_unlocked(
                RuntimeOrigin::signed(bob()),
                DOT
            ));
            assert_eq!(Tokens::free_balance(DOT, &bob()), VESTING_AMOUNT);
            assert_eq!(Tokens::locks(bob(), DOT), vec![balance_lock.clone()]);
            assert_noop!(
                Tokens::transfer(RuntimeOrigin::signed(bob()), charlie(), DOT, VESTING_AMOUNT),
                // for new version TokenError::Frozen,
                tokens::Error::<Runtime>::LiquidityRestrictions,
            );
        }

        run_to_block(VESTING_PERIOD);
        assert_ok!(VestedRewards::claim_unlocked(
            RuntimeOrigin::signed(bob()),
            DOT
        ));
        assert!(Tokens::locks(bob(), DOT).is_empty());
        assert_ok!(Tokens::transfer(
            RuntimeOrigin::signed(bob()),
            charlie(),
            DOT,
            VESTING_AMOUNT,
        ));
    });
}

#[cfg(feature = "wip")] // Auto Vesting
#[test]
fn auto_claim_hook_works_fine() {
    ExtBuilder::default().build().execute_with(|| {
        let schedule = VestingScheduleVariant::LinearVestingSchedule(LinearVestingSchedule {
            asset_id: DOT,
            start: 0u64,
            period: 10u64,
            period_count: 2u32,
            per_period: 10,
            remainder_amount: 10,
        });
        let schedule_locked =
            VestingScheduleVariant::LinearPendingVestingSchedule(LinearPendingVestingSchedule {
                asset_id: DOT,
                manager_id: Some(alice()),
                start: None,
                period: 10u64,
                period_count: 2u32,
                per_period: 10,
                remainder_amount: 10,
            });
        let schedule_ksm = VestingScheduleVariant::LinearVestingSchedule(LinearVestingSchedule {
            asset_id: KSM,
            start: 10u64,
            period: 40u64,
            period_count: 1u32,
            per_period: 10,
            remainder_amount: 0,
        });
        assert_ok!(VestedRewards::vested_transfer(
            RuntimeOrigin::signed(alice()),
            bob(),
            schedule
        ));
        assert_ok!(VestedRewards::vested_transfer(
            RuntimeOrigin::signed(alice()),
            bob(),
            schedule_locked.clone()
        ));

        assert_ok!(VestedRewards::unlock_pending_schedule_by_manager(
            RuntimeOrigin::signed(alice()),
            bob(),
            Some(0_u64),
            schedule_locked
        ),);
        assert_ok!(VestedRewards::vested_transfer(
            RuntimeOrigin::signed(alice()),
            bob(),
            schedule_ksm
        ));

        run_to_block(10);

        assert_ok!(Tokens::transfer(
            RuntimeOrigin::signed(bob()),
            alice(),
            DOT,
            20
        ));

        assert!(VestingSchedules::<Runtime>::contains_key(bob()));

        // more are still locked
        assert!(Tokens::transfer(RuntimeOrigin::signed(bob()), alice(), DOT, 1).is_err());

        run_to_block(20);
        assert!(VestingSchedules::<Runtime>::contains_key(bob()));
        assert!(!ClaimSchedules::<Runtime>::contains_key(30));
        assert_ok!(Tokens::transfer(
            RuntimeOrigin::signed(bob()),
            alice(),
            DOT,
            20,
        ));
        // all used up
        assert_eq!(Tokens::free_balance(DOT, &bob()), 0);

        // no locks anymore
        assert_eq!(Tokens::locks(bob(), DOT), vec![]);
        run_to_block(50);
        assert!(!VestingSchedules::<Runtime>::contains_key(bob()));
    })
}

#[cfg(feature = "wip")] // Auto Vesting
#[test]
fn auto_claim_works_fine_for_pending() {
    ExtBuilder::default().build().execute_with(|| {
        let schedule = VestingScheduleVariant::LinearVestingSchedule(LinearVestingSchedule {
            asset_id: DOT,
            start: 0u64,
            period: 10u64,
            period_count: 2u32,
            per_period: 10,
            remainder_amount: 10,
        });
        let schedule_new = VestingScheduleVariant::LinearVestingSchedule(LinearVestingSchedule {
            asset_id: DOT,
            start: 0u64,
            period: 10u64,
            period_count: 2u32,
            per_period: 10,
            remainder_amount: 0,
        });

        assert_ok!(VestedRewards::vested_transfer(
            RuntimeOrigin::signed(alice()),
            alice(),
            schedule
        ));

        assert_ok!(VestedRewards::vested_transfer(
            RuntimeOrigin::signed(alice()),
            bob(),
            schedule_new
        ));

        let claim_weight = Weight::from_parts(100, 0);

        PendingClaims::<Runtime>::put(Vec::from([
            Claim {
                account_id: alice(),
                asset_id: DOT,
            },
            Claim {
                account_id: bob(),
                asset_id: DOT,
            },
        ]));

        System::set_block_number(11);

        assert_eq!(
            VestedRewards::auto_claim(11, claim_weight),
            Weight::default()
        );

        let claim_weight = Weight::from_parts(100, 100);

        assert_eq!(
            VestedRewards::auto_claim(11, claim_weight),
            claim_weight.saturating_mul(2)
        );

        assert_ok!(Tokens::transfer(
            RuntimeOrigin::signed(bob()),
            alice(),
            DOT,
            10,
        ));
        assert_eq!(Tokens::free_balance(DOT, &bob()), 10);
    })
}

#[cfg(feature = "wip")] // Auto Vesting
#[test]
fn auto_claim_works_fine_for_pending_and_block() {
    ExtBuilder::default().build().execute_with(|| {
        let schedule_alice = VestingScheduleVariant::LinearVestingSchedule(LinearVestingSchedule {
            asset_id: DOT,
            start: 0u64,
            period: 11u64,
            period_count: 2u32,
            per_period: 10,
            remainder_amount: 10,
        });
        let schedule_bob = VestingScheduleVariant::LinearVestingSchedule(LinearVestingSchedule {
            asset_id: KSM,
            start: 0u64,
            period: 11u64,
            period_count: 2u32,
            per_period: 10,
            remainder_amount: 10,
        });
        let schedule_eve = VestingScheduleVariant::LinearVestingSchedule(LinearVestingSchedule {
            asset_id: DOT,
            start: 0u64,
            period: 23u64,
            period_count: 1u32,
            per_period: 10,
            remainder_amount: 0,
        });
        assert_ok!(VestedRewards::vested_transfer(
            RuntimeOrigin::signed(alice()),
            alice(),
            schedule_alice
        ));
        assert_ok!(VestedRewards::vested_transfer(
            RuntimeOrigin::signed(alice()),
            bob(),
            schedule_bob
        ));
        assert_ok!(VestedRewards::vested_transfer(
            RuntimeOrigin::signed(alice()),
            eve(),
            schedule_eve
        ));
        let claim_weight = Weight::from_parts(100000000000, 100);

        // Check claim from pending and from block fine
        PendingClaims::<Runtime>::put(Vec::from([Claim {
            account_id: alice(),
            asset_id: DOT,
        }]));
        System::set_block_number(11);
        assert_eq!(
            ClaimSchedules::<Runtime>::get(11),
            Vec::from([
                Claim {
                    account_id: alice(),
                    asset_id: DOT
                },
                Claim {
                    account_id: bob(),
                    asset_id: KSM
                },
            ])
        );
        assert_eq!(
            VestedRewards::auto_claim(11, claim_weight),
            claim_weight.saturating_mul(2)
        );
        assert_eq!(PendingClaims::<Runtime>::try_get(), Err(()));
        assert_ok!(Tokens::transfer(
            RuntimeOrigin::signed(bob()),
            alice(),
            KSM,
            10,
        ));
        assert_eq!(Tokens::free_balance(KSM, &bob()), 10);

        // Check claim from pending and add block to pending fine
        PendingClaims::<Runtime>::put(Vec::from([
            Claim {
                account_id: alice(),
                asset_id: DOT,
            },
            Claim {
                account_id: bob(),
                asset_id: KSM,
            },
        ]));
        System::set_block_number(23);
        assert_eq!(
            VestedRewards::auto_claim(23, claim_weight),
            claim_weight.saturating_mul(2)
        );
        assert_eq!(
            PendingClaims::<Runtime>::get(),
            Vec::from([Claim {
                account_id: eve(),
                asset_id: DOT
            }])
        );
        assert_ok!(Tokens::transfer(
            RuntimeOrigin::signed(bob()),
            alice(),
            KSM,
            10,
        ));
        assert_eq!(Tokens::free_balance(KSM, &bob()), 0);

        // Claim remaining
        assert_eq!(
            VestedRewards::auto_claim(23, claim_weight),
            claim_weight.saturating_mul(1)
        );
        assert_eq!(PendingClaims::<Runtime>::try_get(), Err(()));
        assert_ok!(Tokens::transfer(
            RuntimeOrigin::signed(eve()),
            alice(),
            DOT,
            10,
        ));
        assert_eq!(Tokens::free_balance(DOT, &eve()), 0);

        assert_eq!(
            VestedRewards::auto_claim(23, claim_weight),
            Weight::default()
        );
    })
}
