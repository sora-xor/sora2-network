// SPDX-License-Identifier: Apache-2.0
//! SORA Nexus compatibility policies exercised against the upstream asynchronous state machine.
use super::*;
use crate::session_rotation::Eras;
use frame_support::{
	dispatch::{GetDispatchInfo, WithPostDispatchInfo},
	traits::{LockableCurrency, WithdrawReasons},
	StorageDoubleMap,
};

#[test]
fn legacy_currency_cleanup_preserves_already_held_liability_and_unlocking() {
	for old_lock in [400, 1000, 9000] {
		ExtBuilder::default().build_and_execute(|| {
			let who = 300;
			bond(who, 1000);
			testing_utils::migrate_to_old_currency::<Test>(who);
			let mut ledger = Staking::ledger(StakingAccount::Stash(who)).unwrap();
			ledger.active = 700;
			ledger.unlocking = bounded_vec![UnlockChunk { value: 300, era: 100 }];
			ledger.clone().update().unwrap();
			// Native ledger updates establish the hold without clearing an old lock.
			Balances::set_lock(STAKING_ID, &who, old_lock, WithdrawReasons::all());
			Balances::set_lock(*b"otherlok", &who, 1, WithdrawReasons::all());
			assert_eq!(asset::free_to_stake::<Test>(&who), 0);
			assert_eq!(asset::staked::<Test>(&who), 1000);
			let currency = asset::total_balance::<Test>(&who);
			let issuance = asset::total_issuance::<Test>();
			let before_ledger = Ledger::<Test>::get(who).unwrap().encode();
			assert_storage_noop!({
				assert_eq!(Staking::inspect_bond_state(&who), Ok(LedgerIntegrityState::Ok));
			});
			assert_ok!(Staking::migrate_currency(RuntimeOrigin::signed(1), who));
			assert_eq!(Ledger::<Test>::get(who).unwrap().encode(), before_ledger);
			assert_eq!(asset::staked::<Test>(&who), ledger.total);
			assert_eq!(asset::total_balance::<Test>(&who), currency);
			assert_eq!(asset::total_issuance::<Test>(), issuance);
			assert_eq!(Balances::balance_locked(STAKING_ID, &who), 0);
			assert_eq!(Balances::balance_locked(*b"otherlok", &who), 1);
			assert_noop!(
				Staking::migrate_currency(RuntimeOrigin::signed(1), who),
				Error::<Test>::AlreadyMigrated
			);
		});
	}
}

#[test]
fn legacy_currency_zero_ledger_keeps_residual_lock_until_explicit_cleanup() {
	ExtBuilder::default().build_and_execute(|| {
		let who = 300;
		bond(who, 1000);
		testing_utils::migrate_to_old_currency::<Test>(who);
		let mut ledger = Staking::ledger(StakingAccount::Stash(who)).unwrap();
		ledger.total = 0;
		ledger.active = 0;
		ledger.update().unwrap();
		let before = Ledger::<Test>::get(who).unwrap().encode();
		let currency = asset::total_balance::<Test>(&who);
		assert_eq!(asset::staked::<Test>(&who), 0);
		assert_storage_noop!({
			assert_eq!(Staking::inspect_bond_state(&who), Ok(LedgerIntegrityState::Ok));
		});
		assert_eq!(Balances::balance_locked(STAKING_ID, &who), 1000);
		assert_ok!(Staking::migrate_currency(RuntimeOrigin::signed(1), who));
		assert_eq!(Ledger::<Test>::get(who).unwrap().encode(), before);
		assert_eq!(asset::staked::<Test>(&who), 0);
		assert_eq!(asset::total_balance::<Test>(&who), currency);
		assert_eq!(Balances::balance_locked(STAKING_ID, &who), 0);
		assert_noop!(
			Staking::migrate_currency(RuntimeOrigin::signed(1), who),
			Error::<Test>::AlreadyMigrated
		);
	});
}

#[test]
fn legacy_currency_cleanup_rejects_ambiguous_holds_locks_and_orphan_bonds() {
	ExtBuilder::default().build_and_execute(|| {
		let who = 300;
		bond(who, 1000);
		testing_utils::migrate_to_old_currency::<Test>(who);
		for held in [500, 1100] {
			hypothetically!({
				assert!(asset::mint_into_existing::<Test>(&who, 200).is_some());
				assert_ok!(asset::update_stake::<Test>(&who, held));
				assert_eq!(
					Staking::inspect_bond_state(&who),
					Ok(LedgerIntegrityState::LockCorrupted)
				);
				assert_noop!(
					Staking::migrate_currency(RuntimeOrigin::signed(1), who),
					Error::<Test>::BadState
				);
			});
		}
		hypothetically!({
			Balances::set_lock(STAKING_ID, &who, 9000, WithdrawReasons::all());
			assert_noop!(
				Staking::migrate_currency(RuntimeOrigin::signed(1), who),
				Error::<Test>::BadState
			);
		});
		hypothetically!({
			Bonded::<Test>::remove(who);
			assert_noop!(
				Staking::migrate_currency(RuntimeOrigin::signed(1), who),
				Error::<Test>::NotStash
			);
		});
		hypothetically!({
			Ledger::<Test>::mutate(who, |ledger| ledger.as_mut().unwrap().active = 999);
			assert_noop!(
				Staking::migrate_currency(RuntimeOrigin::signed(1), who),
				Error::<Test>::BadState
			);
		});
		// A zero/absent lock cannot trigger repeated reference cleanup.
		hypothetically!({
			Balances::remove_lock(STAKING_ID, &who);
			assert_ok!(asset::update_stake::<Test>(&who, 1000));
			assert_noop!(
				Staking::migrate_currency(RuntimeOrigin::signed(1), who),
				Error::<Test>::AlreadyMigrated
			);
		});
	});
}

fn prepare_reward() {
	Staking::reward_by_ids(vec![(11, 10)]);
	Session::roll_until_active_era(2);
	assert!(ErasValidatorReward::<Test>::get(1).unwrap() > 0);
}

#[test]
fn replacement_reward_preserves_native_balances_and_staked_ledgers() {
	for by_page in [false, true] {
		ExtBuilder::default().legacy_reward_mode().build_and_execute(|| {
			prepare_reward();
			let exposure = Eras::<Test>::get_paged_exposure(1, &11, 0).unwrap();
			let recipients: Vec<_> = core::iter::once(11)
				.chain(exposure.others().iter().map(|other| other.who))
				.collect();
			for who in &recipients {
				Payee::<Test>::insert(who, RewardDestination::Staked);
			}
			let before: Vec<_> = recipients
				.iter()
				.map(|who| {
					(
						*who,
						asset::total_balance::<Test>(who),
						Staking::ledger(StakingAccount::Stash(*who)).unwrap(),
					)
				})
				.collect();
			let issuance = asset::total_issuance::<Test>();
			sp_io::storage::set(REPLACEMENT_REWARD_MODE, &[1]);
			sp_io::storage::set(ADDITIONAL_PAYOUT_MODE, &[1]);
			let result = if by_page {
				Staking::payout_stakers_by_page(RuntimeOrigin::signed(1337), 11, 1, 0)
			} else {
				Staking::payout_stakers(RuntimeOrigin::signed(1337), 11, 1)
			};
			assert_ok!(result);
			assert_eq!(
				result.unwrap().actual_weight,
				Some(
					<Test as Config>::WeightInfo::payout_stakers_alive_staked(0).saturating_add(
						MockAdditionalPayout::weight(exposure.others().len() as u32)
					)
				)
			);
			assert_eq!(asset::total_issuance::<Test>(), issuance);
			for (who, balance, ledger) in before {
				assert_eq!(asset::total_balance::<Test>(&who), balance);
				assert_eq!(Staking::ledger(StakingAccount::Stash(who)).unwrap(), ledger);
			}
			assert_eq!(ClaimedRewards::<Test>::get(1, 11), vec![0]);
			assert_eq!(
				sp_io::storage::get(ADDITIONAL_PAYOUT_RECORD).unwrap().as_ref(),
				&(11u64, 1u32, 0u32).encode()[..]
			);
		});
	}
}

#[test]
fn additional_payout_failure_rolls_back_claim_hook_writes_and_events() {
	for replacement in [false, true] {
		for by_page in [false, true] {
			ExtBuilder::default().legacy_reward_mode().build_and_execute(|| {
				prepare_reward();
				if replacement {
					sp_io::storage::set(REPLACEMENT_REWARD_MODE, &[1]);
				}
				sp_io::storage::set(ADDITIONAL_PAYOUT_MODE, &[2]);
				let weight = <Test as Config>::WeightInfo::payout_stakers_alive_staked(
					MaxExposurePageSize::get(),
				)
				.saturating_add(MockAdditionalPayout::weight(MaxExposurePageSize::get()));
				assert_noop!(
					if by_page {
						Staking::payout_stakers_by_page(RuntimeOrigin::signed(1337), 11, 1, 0)
					} else {
						Staking::payout_stakers(RuntimeOrigin::signed(1337), 11, 1)
					},
					sp_runtime::DispatchError::Other("additional reward failed")
						.with_weight(weight)
				);
				assert!(ClaimedRewards::<Test>::get(1, 11).is_empty());
				assert!(!sp_io::storage::exists(ADDITIONAL_PAYOUT_RECORD));
				sp_io::storage::set(ADDITIONAL_PAYOUT_MODE, &[3]);
				System::reset_events();
				assert_ok!(Staking::payout_stakers_by_page(RuntimeOrigin::signed(1337), 11, 1, 0));
				if replacement {
					assert_eq!(
						staking_events(),
						vec![
							Event::PayoutStarted {
								era_index: 1,
								validator_stash: 11,
								page: 0,
								next: None
							},
							Event::Rewarded {
								stash: 11,
								dest: RewardDestination::Stash,
								amount: 1
							},
						]
					);
				}
			});
		}
	}
}

#[test]
fn replacement_reward_budget_bypasses_native_cap_and_remainder() {
	ExtBuilder::default().legacy_reward_mode().build_and_execute(|| {
		sp_io::storage::set(REPLACEMENT_REWARD_MODE, &[1]);
		MaxStakedRewards::<Test>::set(Some(Percent::from_percent(1)));
		RewardRemainderUnbalanced::set(0);
		let total = time_per_era() as Balance;
		let remainder = RemainderRatio::get() * total;
		let issuance = asset::total_issuance::<Test>();
		Session::roll_until_active_era(2);
		assert_eq!(ErasValidatorReward::<Test>::get(1), Some(total - remainder));
		assert_eq!(asset::total_issuance::<Test>(), issuance);
		assert_eq!(RewardRemainderUnbalanced::get(), 0);
	});
}

#[test]
fn additional_payout_weight_is_retained_for_zero_reward() {
	ExtBuilder::default().legacy_reward_mode().build_and_execute(|| {
		prepare_reward();
		ErasValidatorReward::<Test>::insert(1, 0);
		sp_io::storage::set(ADDITIONAL_PAYOUT_MODE, &[1]);
		let exposure = Eras::<Test>::get_paged_exposure(1, &11, 0).unwrap();
		let declared =
			<Test as Config>::WeightInfo::payout_stakers_alive_staked(MaxExposurePageSize::get())
				.saturating_add(MockAdditionalPayout::weight(MaxExposurePageSize::get()));
		assert_eq!(
			Call::<Test>::payout_stakers { validator_stash: 11, era: 1 }
				.get_dispatch_info()
				.call_weight,
			declared
		);
		assert_eq!(
			Call::<Test>::payout_stakers_by_page { validator_stash: 11, era: 1, page: 0 }
				.get_dispatch_info()
				.call_weight,
			declared
		);
		let result =
			Staking::payout_stakers_by_page(RuntimeOrigin::signed(1337), 11, 1, 0).unwrap();
		assert_eq!(
			result.actual_weight,
			Some(
				<Test as Config>::WeightInfo::payout_stakers_alive_staked(0)
					.saturating_add(MockAdditionalPayout::weight(exposure.others().len() as u32))
			)
		);
		assert!(sp_io::storage::exists(ADDITIONAL_PAYOUT_RECORD));
	});
}

#[test]
fn legacy_ledger_claim_prevents_duplicate_reward() {
	ExtBuilder::default().legacy_reward_mode().build_and_execute(|| {
		prepare_reward();
		Ledger::<Test>::mutate(11, |ledger| {
			ledger.as_mut().unwrap().legacy_claimed_rewards.try_push(1).unwrap()
		});
		assert_noop!(
			Staking::payout_stakers_by_page(RuntimeOrigin::signed(1337), 11, 1, 0),
			Error::<Test>::AlreadyClaimed
				.with_weight(<Test as Config>::WeightInfo::payout_stakers_alive_staked(0))
		);
	});
}

#[test]
fn deferred_slash_offset_preserves_original_era_and_immediate_mode() {
	ExtBuilder::default().build_and_execute(|| {
		SlashDeferOffset::set(1);
		SlashDeferDuration::set(2);
		assert_eq!(Staking::deferred_slash_delay(), 3);
		assert_eq!(Staking::offence_era_of(4), 1);
		SlashDeferDuration::set(0);
		assert_eq!(Staking::deferred_slash_delay(), 0);
		assert_eq!(Staking::offence_era_of(4), 4);
	});
}

#[test]
fn deferred_slash_offset_holds_same_era_unlocks_until_queue_applies() {
	ExtBuilder::default().slash_defer_duration(2).build_and_execute(|| {
		SlashDeferOffset::set(1);
		add_slash(11);
		Session::roll_next();
		assert!(AsyncUnappliedSlashes::<Test>::contains_prefix(4));
		assert!(!AsyncUnappliedSlashes::<Test>::contains_prefix(3));
		assert_ok!(Staking::chill(RuntimeOrigin::signed(11)));
		assert_ok!(Staking::unbond(RuntimeOrigin::signed(11), 1000));
		Session::roll_until_active_era(3);
		assert_eq!(asset::stakeable_balance::<Test>(&11), 1000);
		Session::roll_until_active_era(4);
		assert_noop!(
			Staking::withdraw_unbonded(RuntimeOrigin::signed(11), 0),
			Error::<Test>::UnappliedSlashesInPreviousEra
		);
		Session::roll_next();
		assert_eq!(asset::stakeable_balance::<Test>(&11), 900);
		assert_ok!(Staking::withdraw_unbonded(RuntimeOrigin::signed(11), 0));
		assert!(!Bonded::<Test>::contains_key(11));
	});
}

#[test]
fn deferred_slash_offset_late_report_and_increment_are_applied_without_stranding() {
	ExtBuilder::default().slash_defer_duration(2).build_and_execute(|| {
		SlashDeferOffset::set(1);
		Session::roll_until_active_era(4);
		add_slash_in_era(11, 1, Perbill::from_percent(10));
		Session::roll_next();
		assert_eq!(asset::stakeable_balance::<Test>(&11), 900);
		assert!(!AsyncUnappliedSlashes::<Test>::contains_prefix(4));
		add_slash_in_era(11, 1, Perbill::from_percent(20));
		Session::roll_next();
		assert_eq!(asset::stakeable_balance::<Test>(&11), 800);
		assert!(!AsyncUnappliedSlashes::<Test>::contains_prefix(4));
	});
}

#[test]
fn historical_exposure_page_metadata_and_encoding_are_preserved() {
	type HistoricalPage = sp_staking::ExposurePage<u64, u128>;
	assert_eq!(
		scale_info::meta_type::<BoundedExposurePage<Test>>(),
		scale_info::meta_type::<HistoricalPage>()
	);
	assert_eq!(
		<BoundedExposurePage<Test> as scale_info::TypeInfo>::type_info(),
		<HistoricalPage as scale_info::TypeInfo>::type_info()
	);
	let exposure = HistoricalPage {
		page_total: 123,
		others: vec![IndividualExposure { who: 42, value: 123 }],
	};
	let encoded = exposure.encode();
	let bounded = BoundedExposurePage::<Test>::decode(&mut encoded.as_slice()).unwrap();
	assert_eq!(bounded.encode(), encoded);
	assert_eq!(bounded.0, exposure);
}

#[test]
fn historical_event_error_and_call_encodings_are_preserved() {
	// These numeric tags and field tuples are part of the production runtime interface.
	assert_eq!(
		Event::<Test>::Rewarded { stash: 11, dest: RewardDestination::Stash, amount: 9 }.encode(),
		(1u8, 11u64, RewardDestination::<u64>::Stash, 9u128).encode()
	);
	assert_eq!(
		Event::<Test>::Unbonded { stash: 11, amount: 9 }.encode(),
		(7u8, 11u64, 9u128).encode()
	);
	assert_eq!(
		Event::<Test>::PayoutStarted { era_index: 2, validator_stash: 11, page: 3, next: Some(4) }
			.encode(),
		(12u8, 2u32, 11u64, 3u32, Some(4u32)).encode()
	);
	assert_eq!(Error::<Test>::InvalidSlashIndex.encode(), vec![6]);
	assert_eq!(Error::<Test>::NotSortedAndUnique.encode(), vec![13]);
	assert_eq!(Error::<Test>::AlreadyClaimed.encode(), vec![14]);
	assert_eq!(Error::<Test>::Restricted.encode(), vec![33]);
	assert_eq!(
		Call::<Test>::cancel_deferred_slash { era: 2, slash_indices: vec![1, 3] }.encode(),
		(17u8, 2u32, vec![1u32, 3u32]).encode()
	);
	assert_eq!(
		Call::<Test>::set_invulnerables { invulnerables: vec![11] }.encode(),
		(14u8, vec![11u64]).encode()
	);
	assert_eq!(
		Call::<Test>::manual_slash {
			validator_stash: 11,
			era: 2,
			slash_fraction: Perbill::from_percent(10),
		}
		.encode(),
		(33u8, 11u64, 2u32, Perbill::from_percent(10)).encode()
	);
}

#[test]
fn deferred_slash_cursor_drains_backlog_after_era_advances() {
	ExtBuilder::default().slash_defer_duration(2).build_and_execute(|| {
		SlashDeferOffset::set(1);
		// A backlog larger than an era must remain executable at its original deadline.
		for page in 0..100 {
			AsyncUnappliedSlashes::<Test>::insert(
				4,
				(11, Perbill::from_percent(10), page),
				UnappliedSlash::<Test> {
					validator: 11,
					own: 0,
					others: Default::default(),
					reporter: None,
					payout: 0,
				},
			);
		}
		NextDeferredSlashEra::<Test>::put(4);
		assert_ok!(Staking::chill(RuntimeOrigin::signed(11)));
		assert_ok!(Staking::unbond(RuntimeOrigin::signed(11), 1000));
		Session::roll_until_active_era(5);
		assert!(AsyncUnappliedSlashes::<Test>::contains_prefix(4));
		assert_ok!(Staking::do_try_state(System::block_number()));
		let saved_cursor = NextDeferredSlashEra::<Test>::get().unwrap();
		NextDeferredSlashEra::<Test>::put(5);
		assert!(
			Staking::do_try_state(System::block_number()).is_err(),
			"try-state must reject a cursor that could skip an overdue queue"
		);
		NextDeferredSlashEra::<Test>::put(saved_cursor);
		assert_noop!(
			Staking::withdraw_unbonded(RuntimeOrigin::signed(11), 0),
			Error::<Test>::UnappliedSlashesInPreviousEra
		);
		while AsyncUnappliedSlashes::<Test>::contains_prefix(4) {
			Session::roll_next();
		}
		// Empty-era cursor advancement is bounded to one step per block as well.
		while NextDeferredSlashEra::<Test>::get().unwrap() <= active_era() {
			Session::roll_next();
		}
		assert_ok!(Staking::withdraw_unbonded(RuntimeOrigin::signed(11), 0));
	});
}

#[test]
fn historical_invulnerables_remain_exempt_in_new_async_eras() {
	ExtBuilder::default().build_and_execute(|| {
		assert_ok!(Staking::set_invulnerables(RuntimeOrigin::root(), vec![11]));
		add_slash(11);
		Session::roll_next();
		assert_eq!(asset::stakeable_balance::<Test>(&11), 1000);
		assert!(!ValidatorSlashInEra::<Test>::contains_key(1, 11));
		assert!(!OffenceQueue::<Test>::contains_key(1, 11));
		assert_ok!(Staking::set_invulnerables(RuntimeOrigin::root(), vec![]));
		add_slash(11);
		Session::roll_next();
		assert_eq!(asset::stakeable_balance::<Test>(&11), 900);
	});
}

#[test]
fn historical_span_count_is_validated_before_force_unstake() {
	ExtBuilder::default().build_and_execute(|| {
		crate::legacy::CutoffEra::<Test>::put(1);
		add_slash(11);
		assert!(crate::legacy::SlashingSpans::<Test>::contains_key(11));
		let count = crate::legacy::SlashingSpans::<Test>::get(11).unwrap().iter().count() as u32;
		assert!(count > 0);
		assert_noop!(
			Staking::force_unstake(RuntimeOrigin::root(), 11, count - 1),
			Error::<Test>::IncorrectSlashingSpans
		);
		assert_ok!(Staking::force_unstake(RuntimeOrigin::root(), 11, count));
		assert!(!crate::legacy::SlashingSpans::<Test>::contains_key(11));
		assert!(!Bonded::<Test>::contains_key(11));
		for index in 0..count {
			assert!(!crate::legacy::SpanSlash::<Test>::contains_key((11, index)));
		}
	});
}
