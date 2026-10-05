// This file is part of the SORA network and Polkaswap app.
// SPDX-License-Identifier: BSD-4-Clause

use crate::{
    AccountId, Balance, Balances, Currencies, Runtime, RuntimeCall, RuntimeEvent, RuntimeOrigin,
    Staking, System,
};
use common::{balance, VAL};
use frame_support::assert_ok;
use frame_support::traits::{Currency, Get, OnRuntimeUpgrade};
use pallet_staking::{
    ActiveEraInfo, EraRewardPoints, Exposure, IndividualExposure, RewardDestination, StakingLedger,
    ValidatorPrefs,
};
use sp_runtime::traits::Dispatchable;
use sp_runtime::{AccountId32, Percent};
use sp_staking::EraIndex;
use traits::MultiCurrency;

struct Fixture {
    validator: AccountId,
    validator_controller: AccountId,
    nominator: AccountId,
    nominator_controller: AccountId,
    era: EraIndex,
}

fn setup(staked_payee: bool) -> Fixture {
    System::set_block_number(1);
    let fixture = Fixture {
        validator: AccountId32::from([201; 32]),
        validator_controller: AccountId32::from([202; 32]),
        nominator: AccountId32::from([203; 32]),
        nominator_controller: AccountId32::from([204; 32]),
        era: 7,
    };
    for (stash, controller) in [
        (&fixture.validator, &fixture.validator_controller),
        (&fixture.nominator, &fixture.nominator_controller),
    ] {
        drop(Balances::deposit_creating(stash, balance!(200)));
        assert_ok!(
            <Balances as frame_support::traits::fungible::hold::Mutate<AccountId>>::set_on_hold(
                &pallet_staking::HoldReason::Staking.into(),
                stash,
                balance!(100),
            )
        );
        pallet_staking::Bonded::<Runtime>::insert(stash, controller);
        pallet_staking::Ledger::<Runtime>::insert(
            controller,
            StakingLedger::<Runtime> {
                stash: stash.clone(),
                total: balance!(100),
                active: balance!(100),
                unlocking: Default::default(),
                legacy_claimed_rewards: Default::default(),
                controller: Some(controller.clone()),
            },
        );
        pallet_staking::Payee::<Runtime>::insert(
            stash,
            if staked_payee {
                RewardDestination::Staked
            } else {
                RewardDestination::Stash
            },
        );
    }
    pallet_staking::CurrentEra::<Runtime>::put(fixture.era + 1);
    pallet_staking::ActiveEra::<Runtime>::put(ActiveEraInfo {
        index: fixture.era + 1,
        start: Some(0),
    });
    record_era(&fixture, fixture.era, balance!(1000));
    fixture
}

fn record_era(fixture: &Fixture, era: EraIndex, reward: Balance) {
    // The public Apps derive discovers this storage, without knowing about xor_fee.
    pallet_staking::ErasValidatorReward::<Runtime>::insert(era, reward);
    xor_fee::ValStakingEraReward::<Runtime>::insert(era, reward);
    pallet_staking::ErasRewardPoints::<Runtime>::insert(
        era,
        EraRewardPoints {
            total: 10,
            individual: vec![(fixture.validator.clone(), 10)].into_iter().collect(),
        },
    );
    pallet_staking::ErasStakersClipped::<Runtime>::insert(
        era,
        &fixture.validator,
        Exposure {
            total: 100,
            own: 40,
            others: vec![IndividualExposure {
                who: fixture.nominator.clone(),
                value: 60,
            }],
        },
    );
    pallet_staking::ErasValidatorPrefs::<Runtime>::insert(
        era,
        &fixture.validator,
        ValidatorPrefs::default(),
    );
}

fn payout(fixture: &Fixture, era: EraIndex) -> RuntimeCall {
    RuntimeCall::Staking(pallet_staking::Call::payout_stakers {
        validator_stash: fixture.validator.clone(),
        era,
    })
}

fn native_snapshot(
    fixture: &Fixture,
) -> (
    Balance,
    Balance,
    Balance,
    Balance,
    Balance,
    Balance,
    Balance,
) {
    let validator_ledger = pallet_staking::Ledger::<Runtime>::get(&fixture.validator_controller)
        .expect("validator ledger exists");
    let nominator_ledger = pallet_staking::Ledger::<Runtime>::get(&fixture.nominator_controller)
        .expect("nominator ledger exists");
    (
        Balances::total_issuance(),
        Balances::free_balance(&fixture.validator),
        Balances::free_balance(&fixture.nominator),
        validator_ledger.active,
        validator_ledger.total,
        nominator_ledger.active,
        nominator_ledger.total,
    )
}

fn standard_rewards() -> Vec<(AccountId, Balance)> {
    System::events()
        .into_iter()
        .filter_map(|record| match record.event {
            RuntimeEvent::Staking(pallet_staking::Event::Rewarded { stash, amount, .. }) => {
                Some((stash, amount))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn published_val_budget_direct_payout_does_not_mint_or_compound_xor() {
    for staked_payee in [false, true] {
        framenode_chain_spec::ext().execute_with(|| {
            let fixture = setup(staked_payee);
            let native_before = native_snapshot(&fixture);
            let val_before = Currencies::total_issuance(VAL.into());
            assert_eq!(
                Staking::eras_validator_reward(fixture.era),
                Some(balance!(1000)),
            );
            // Anyone may request the same direct call generated by hosted Apps.
            assert_ok!(payout(&fixture, fixture.era)
                .dispatch(RuntimeOrigin::signed(AccountId32::from([205; 32]))));
            assert_eq!(native_snapshot(&fixture), native_before);
            assert_eq!(
                Currencies::total_issuance(VAL.into()),
                val_before + balance!(1000)
            );
            assert_eq!(
                Currencies::free_balance(VAL.into(), &fixture.validator),
                balance!(400)
            );
            assert_eq!(
                Currencies::free_balance(VAL.into(), &fixture.nominator),
                balance!(600)
            );
            assert_eq!(
                standard_rewards(),
                vec![
                    (fixture.validator.clone(), balance!(400)),
                    (fixture.nominator.clone(), balance!(600)),
                ],
            );
            assert_eq!(
                System::events()
                    .iter()
                    .filter(|record| matches!(
                        &record.event,
                        RuntimeEvent::Staking(pallet_staking::Event::PayoutStarted { .. })
                    ))
                    .count(),
                1,
            );
            let events = System::events();
            let started = events
                .iter()
                .position(|record| {
                    matches!(
                        &record.event,
                        RuntimeEvent::Staking(pallet_staking::Event::PayoutStarted { .. })
                    )
                })
                .expect("payout announces its era and page");
            let rewarded = events
                .iter()
                .position(|record| {
                    matches!(
                        &record.event,
                        RuntimeEvent::Staking(pallet_staking::Event::Rewarded { .. })
                    )
                })
                .expect("VAL recipients emit standard reward events");
            assert!(started < rewarded);
            assert!(payout(&fixture, fixture.era)
                .dispatch(RuntimeOrigin::signed(fixture.validator.clone()))
                .is_err());
            assert_eq!(
                Currencies::total_issuance(VAL.into()),
                val_before + balance!(1000)
            );
            assert_eq!(standard_rewards().len(), 2);
        });
    }
}

#[test]
fn published_val_budgets_hosted_style_batch_pay_each_completed_era_once() {
    framenode_chain_spec::ext().execute_with(|| {
        let fixture = setup(true);
        record_era(&fixture, fixture.era - 1, balance!(1000));
        let native_before = native_snapshot(&fixture);
        let val_before = Currencies::total_issuance(VAL.into());
        let batch = RuntimeCall::Utility(pallet_utility::Call::batch {
            calls: vec![
                payout(&fixture, fixture.era - 1),
                payout(&fixture, fixture.era),
            ],
        });
        assert_ok!(batch.dispatch(RuntimeOrigin::signed(AccountId32::from([205; 32]))));
        assert_eq!(native_snapshot(&fixture), native_before);
        assert_eq!(
            Currencies::total_issuance(VAL.into()),
            val_before + balance!(2000)
        );
        assert_eq!(
            Currencies::free_balance(VAL.into(), &fixture.validator),
            balance!(800)
        );
        assert_eq!(
            Currencies::free_balance(VAL.into(), &fixture.nominator),
            balance!(1200)
        );
        assert_eq!(standard_rewards().len(), 4);
        for era in [fixture.era - 1, fixture.era] {
            assert!(payout(&fixture, era)
                .dispatch(RuntimeOrigin::signed(fixture.validator.clone()))
                .is_err());
        }
        assert_eq!(
            Currencies::total_issuance(VAL.into()),
            val_before + balance!(2000)
        );
    });
}

#[test]
fn published_val_budget_failed_mint_rolls_back_events_claims_and_both_currencies() {
    framenode_chain_spec::ext().execute_with(|| {
        let fixture = setup(true);
        let native_before = native_snapshot(&fixture);
        let asset = crate::AssetId::from(VAL);
        let val_before = tokens::TotalIssuance::<Runtime>::get(asset);
        // Validator's payment succeeds, but nominator issuance then overflows.
        tokens::TotalIssuance::<Runtime>::insert(asset, Balance::MAX - balance!(500));
        assert!(payout(&fixture, fixture.era)
            .dispatch(RuntimeOrigin::signed(fixture.validator.clone()))
            .is_err());
        assert_eq!(native_snapshot(&fixture), native_before);
        assert_eq!(Currencies::free_balance(VAL.into(), &fixture.validator), 0);
        assert_eq!(Currencies::free_balance(VAL.into(), &fixture.nominator), 0);
        assert_eq!(
            tokens::TotalIssuance::<Runtime>::get(asset),
            Balance::MAX - balance!(500)
        );
        assert!(
            pallet_staking::ClaimedRewards::<Runtime>::get(fixture.era, &fixture.validator)
                .is_empty()
        );
        assert!(standard_rewards().is_empty());
        assert!(!System::events().iter().any(|record| matches!(
            &record.event,
            RuntimeEvent::XorFee(xor_fee::Event::ValStakingRewardPaid(..))
        )));
        tokens::TotalIssuance::<Runtime>::insert(asset, val_before);
        assert_ok!(payout(&fixture, fixture.era)
            .dispatch(RuntimeOrigin::signed(fixture.validator.clone())));
        assert_eq!(
            Currencies::total_issuance(VAL.into()),
            val_before + balance!(1000)
        );
    });
}

#[test]
fn migration_publishes_only_retained_completed_existing_entries_and_preserves_claims() {
    framenode_chain_spec::ext().execute_with(|| {
        let fixture = setup(false);
        let current = 100;
        let active = 99;
        let depth: EraIndex = <Runtime as pallet_staking::Config>::HistoryDepth::get();
        let oldest = current - depth;
        pallet_staking::CurrentEra::<Runtime>::put(current);
        pallet_staking::ActiveEra::<Runtime>::put(ActiveEraInfo {
            index: active,
            start: Some(0),
        });
        for (era, reward) in [
            (oldest - 1, 11),
            (oldest, 22),
            (96, 33),
            (97, 0),
            (98, 44),
            (active, 55),
            (current, 66),
            (current + 1, 77),
        ] {
            xor_fee::ValStakingEraReward::<Runtime>::insert(era, reward);
            if era != 96 {
                pallet_staking::ErasValidatorReward::<Runtime>::insert(era, 0);
            }
        }
        pallet_staking::ClaimedRewards::<Runtime>::insert(98, &fixture.validator, vec![0, 2]);
        pallet_staking::Ledger::<Runtime>::mutate(&fixture.validator_controller, |ledger| {
            ledger.as_mut().unwrap().legacy_claimed_rewards = vec![oldest, 98].try_into().unwrap();
        });
        let ledger_before = pallet_staking::Ledger::<Runtime>::get(&fixture.validator_controller);
        let native_before = native_snapshot(&fixture);
        let val_before = Currencies::total_issuance(VAL.into());
        assert!(!crate::migrations::val_staking_rewards_published());
        crate::migrations::PublishValStakingRewards::on_runtime_upgrade();
        assert!(crate::migrations::val_staking_rewards_published());
        assert_eq!(Staking::eras_validator_reward(oldest), Some(22));
        assert_eq!(Staking::eras_validator_reward(97), Some(0));
        assert_eq!(Staking::eras_validator_reward(98), Some(44));
        assert_eq!(Staking::eras_validator_reward(96), None);
        for era in [oldest - 1, active, current, current + 1] {
            assert_eq!(Staking::eras_validator_reward(era), Some(0));
        }
        assert_eq!(
            pallet_staking::ClaimedRewards::<Runtime>::get(98, &fixture.validator),
            vec![0, 2]
        );
        assert_eq!(
            pallet_staking::Ledger::<Runtime>::get(&fixture.validator_controller),
            ledger_before
        );
        assert_eq!(native_snapshot(&fixture), native_before);
        assert_eq!(Currencies::total_issuance(VAL.into()), val_before);
        // A later invocation cannot republish or otherwise mutate an already upgraded era.
        pallet_staking::ErasValidatorReward::<Runtime>::insert(98, 88);
        crate::migrations::PublishValStakingRewards::on_runtime_upgrade();
        assert_eq!(Staking::eras_validator_reward(98), Some(88));
        assert_eq!(
            pallet_staking::ClaimedRewards::<Runtime>::get(98, &fixture.validator),
            vec![0, 2]
        );
    });
}

#[test]
fn era_close_publishes_the_full_val_budget_despite_native_staked_reward_cap() {
    framenode_chain_spec::ext().execute_with(|| {
        let fixture = setup(true);
        pallet_staking::ActiveEra::<Runtime>::put(ActiveEraInfo { index: fixture.era, start: Some(0) });
        pallet_staking::ErasStartSessionIndex::<Runtime>::insert(fixture.era + 1, 3);
        pallet_staking::ErasValidatorReward::<Runtime>::remove(fixture.era);
        pallet_staking::MaxStakedRewards::<Runtime>::put(Percent::from_percent(20));
        let native_before = native_snapshot(&fixture);
        let val_before = Currencies::total_issuance(VAL.into());
        assert_eq!(Staking::eras_validator_reward(fixture.era), None);
        <Staking as pallet_session::SessionManager<AccountId>>::end_session(2);
        assert_eq!(Staking::eras_validator_reward(fixture.era), Some(balance!(1000)));
        assert_eq!(native_snapshot(&fixture), native_before);
        assert_eq!(Currencies::total_issuance(VAL.into()), val_before);
        assert!(System::events().iter().any(|record| matches!(
            &record.event,
            RuntimeEvent::Staking(pallet_staking::Event::EraPaid { era_index, validator_payout, remainder })
                if *era_index == fixture.era && *validator_payout == balance!(1000) && *remainder == 0
        )));
        assert_ok!(payout(&fixture, fixture.era)
            .dispatch(RuntimeOrigin::signed(fixture.validator.clone())));
        assert_eq!(native_snapshot(&fixture), native_before);
        assert_eq!(Currencies::total_issuance(VAL.into()), val_before + balance!(1000));
    });
}
