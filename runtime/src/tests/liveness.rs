// This file is part of the SORA network and Polkaswap app.
// SPDX-License-Identifier: BSD-4-Clause

use crate::liveness::{DisabledDuringSession, EquivocationReports, LivenessImOnline};
use crate::{
    AccountId, Balance, Balances, ImOnline, ImOnlineId, Runtime, RuntimeEvent, RuntimeOrigin,
    Session, Staking, System,
};
use codec::Encode;
use frame_support::{
    assert_ok,
    traits::{Currency, OneSessionHandler},
};
use framenode_chain_spec::ext;
use pallet_im_online::{sr25519::AuthorityPair, Heartbeat, UnresponsivenessOffence};
use pallet_staking::{ActiveEraInfo, EraInfo, Exposure, IndividualExposure, RewardDestination};
use sp_core::Pair;
use sp_runtime::{
    traits::ValidateUnsigned,
    transaction_validity::{InvalidTransaction, TransactionSource},
    Perbill,
};
use sp_staking::offence::{Offence, OffenceError, ReportOffence};

const VALIDATORS: u32 = 25;
const OFFLINE: usize = 6;
const ERA: u32 = 100;
const FIRST_SESSION: u32 = 1_000;
const UNIT: Balance = 1_000_000_000_000_000_000;
const OWN_STAKE: Balance = 1_000 * UNIT;
const NOMINATED_STAKE: Balance = 100 * UNIT;

type Identification = pallet_session::historical::IdentificationTuple<Runtime>;

struct Validators {
    accounts: Vec<AccountId>,
    keys: Vec<ImOnlineId>,
    nominator: AccountId,
}

impl Validators {
    fn new() -> Self {
        Self::with_count(VALIDATORS)
    }

    fn with_count(count: u32) -> Self {
        System::set_block_number(1);
        let accounts = (0..count)
            .map(|index| AccountId::from([index as u8 + 150; 32]))
            .collect::<Vec<_>>();
        let keys = (0..count)
            .map(|index| authority_pair(index).public())
            .collect();
        let nominator = AccountId::from([240; 32]);

        // Install an elected set and real bonded balances. Election machinery is outside
        // these tests; liveness reporting, offence handling and monetary slashing are real.
        pallet_session::Validators::<Runtime>::put(&accounts);
        pallet_session::DisabledValidators::<Runtime>::kill();
        pallet_staking::ActiveEra::<Runtime>::put(ActiveEraInfo {
            index: ERA,
            start: Some(0),
        });
        pallet_staking::CurrentEra::<Runtime>::put(ERA);
        pallet_staking::ErasStartSessionIndex::<Runtime>::insert(ERA, FIRST_SESSION);
        pallet_staking::BondedEras::<Runtime>::put(vec![(ERA, FIRST_SESSION)]);
        pallet_staking::Invulnerables::<Runtime>::put(Vec::<AccountId>::new());

        for account in &accounts {
            fund_and_bond(account, OWN_STAKE);
        }
        fund_and_bond(&nominator, NOMINATED_STAKE * Balance::from(count));
        for account in &accounts {
            EraInfo::<Runtime>::set_exposure(
                ERA,
                account,
                Exposure {
                    total: OWN_STAKE + NOMINATED_STAKE,
                    own: OWN_STAKE,
                    others: vec![IndividualExposure {
                        who: nominator.clone(),
                        value: NOMINATED_STAKE,
                    }],
                },
            );
        }

        let validators = Self {
            accounts,
            keys,
            nominator,
        };
        validators.start_session(FIRST_SESSION);
        System::reset_events();
        validators
    }

    fn start_session(&self, session: u32) {
        pallet_session::CurrentIndex::<Runtime>::put(session);
        let validators = self.accounts.iter().zip(self.keys.iter().cloned());
        <LivenessImOnline as OneSessionHandler<AccountId>>::on_new_session(
            false,
            validators.clone(),
            validators,
        );
    }

    fn note_author(&self, index: usize) {
        <ImOnline as pallet_authorship::EventHandler<AccountId, crate::BlockNumber>>::note_author(
            self.accounts[index].clone(),
        );
    }

    fn end_session_with_missing(&self, missing: &[usize]) {
        for index in 0..self.accounts.len() {
            if !missing.contains(&index) {
                self.note_author(index);
            }
        }
        System::reset_events();
        <LivenessImOnline as OneSessionHandler<AccountId>>::on_before_session_ending();
        let offline = System::events()
            .into_iter()
            .find_map(|event| match event.event {
                RuntimeEvent::ImOnline(pallet_im_online::Event::SomeOffline { offline }) => Some(
                    offline
                        .into_iter()
                        .map(|(account, _)| account)
                        .collect::<Vec<_>>(),
                ),
                _ => None,
            })
            .expect("missing liveness remains observable, including disabled validators");
        assert_eq!(
            offline,
            missing
                .iter()
                .map(|index| self.accounts[*index].clone())
                .collect::<Vec<_>>()
        );
    }

    fn balances_and_stakes(&self) -> Vec<(Balance, Balance)> {
        self.accounts
            .iter()
            .chain(std::iter::once(&self.nominator))
            .map(|account| {
                (
                    <Balances as Currency<AccountId>>::total_balance(account),
                    pallet_staking::Ledger::<Runtime>::get(account)
                        .expect("fixture account is bonded to itself")
                        .active,
                )
            })
            .collect()
    }
}

fn authority_pair(index: u32) -> AuthorityPair {
    AuthorityPair::from_seed(&[index as u8 + 1; 32])
}

fn fund_and_bond(account: &AccountId, stake: Balance) {
    let _ = <Balances as Currency<AccountId>>::make_free_balance_be(account, stake + 100 * UNIT);
    assert_ok!(Staking::bond(
        RuntimeOrigin::signed(account.clone()),
        stake,
        RewardDestination::Stash,
    ));
}

fn assert_no_offence_or_slash() {
    assert_eq!(pallet_offences::Reports::<Runtime>::iter().count(), 0);
    assert_eq!(
        pallet_staking::UnappliedSlashes::<Runtime>::iter().count(),
        0
    );
    assert_eq!(
        pallet_staking::ValidatorSlashInEra::<Runtime>::iter().count(),
        0
    );
    assert!(!System::events().iter().any(|event| matches!(
        event.event,
        RuntimeEvent::Staking(pallet_staking::Event::SlashReported { .. })
            | RuntimeEvent::Staking(pallet_staking::Event::Slashed { .. })
    )));
}

#[test]
fn repeated_missing_liveness_is_observed_without_disabling_or_slashing() {
    ext().execute_with(|| {
        let validators = Validators::new();
        let balances_and_stakes = validators.balances_and_stakes();

        for session in FIRST_SESSION..FIRST_SESSION + 2 {
            validators.start_session(session);
            for index in OFFLINE..VALIDATORS as usize {
                validators.note_author(index);
            }
            System::reset_events();
            <LivenessImOnline as OneSessionHandler<AccountId>>::on_before_session_ending();

            let offline = System::events()
                .into_iter()
                .find_map(|event| match event.event {
                    RuntimeEvent::ImOnline(pallet_im_online::Event::SomeOffline { offline }) => {
                        Some(
                            offline
                                .into_iter()
                                .map(|(account, _)| account)
                                .collect::<Vec<_>>(),
                        )
                    }
                    _ => None,
                })
                .expect("missing heartbeats and authored blocks remain observable");
            assert_eq!(offline, validators.accounts[..OFFLINE]);
            assert_no_offence_or_slash();
            assert!(Session::disabled_validators().is_empty());
            assert_eq!(validators.balances_and_stakes(), balances_and_stakes);
            assert_eq!(
                pallet_im_online::AuthoredBlocks::<Runtime>::iter_prefix(session).count(),
                0
            );
        }
    });
}

fn queued_slash_count() -> usize {
    pallet_staking::UnappliedSlashes::<Runtime>::iter()
        .map(|(_, slashes)| slashes.len())
        .sum()
}

fn assert_majority_slashes(validators: &Validators, penalized: &[usize]) {
    let fraction = Perbill::from_percent(7);
    let expected_validator_slash = fraction * OWN_STAKE;
    assert_eq!(
        pallet_offences::Reports::<Runtime>::iter().count(),
        penalized.len()
    );
    let reported = System::events()
        .into_iter()
        .filter_map(|event| match event.event {
            RuntimeEvent::Staking(pallet_staking::Event::SlashReported {
                validator,
                fraction: reported_fraction,
                slash_era,
            }) => {
                assert_eq!(reported_fraction, fraction);
                assert_eq!(slash_era, ERA);
                Some(validator)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(reported.len(), penalized.len());
    for (index, account) in validators.accounts.iter().enumerate() {
        let expected = if penalized.contains(&index) {
            assert!(reported.contains(account));
            Some((fraction, expected_validator_slash))
        } else {
            assert!(!reported.contains(account));
            None
        };
        assert_eq!(
            pallet_staking::ValidatorSlashInEra::<Runtime>::get(ERA, account),
            expected
        );
    }
    let defer = <Runtime as pallet_staking::Config>::SlashDeferDuration::get();
    if defer > 0 {
        let queued = pallet_staking::UnappliedSlashes::<Runtime>::get(ERA + defer + 1);
        assert_eq!(queued.len(), penalized.len());
        assert_eq!(queued_slash_count(), penalized.len());
        for slash in queued {
            assert_eq!(slash.own, expected_validator_slash);
            assert!(penalized
                .iter()
                .any(|index| validators.accounts[*index] == slash.validator));
        }
    } else {
        assert_eq!(queued_slash_count(), 0);
    }
}

#[test]
fn offline_slashes_require_a_strict_majority_in_odd_and_even_validator_sets() {
    for (count, missing, should_slash) in [
        (25, 12, false),
        (25, 13, true),
        (24, 12, false),
        (24, 13, true),
    ] {
        ext().execute_with(|| {
            let validators = Validators::with_count(count);
            let before = validators.balances_and_stakes();
            let missing = (0..missing).collect::<Vec<_>>();
            validators.end_session_with_missing(&missing);
            if should_slash {
                assert_majority_slashes(&validators, &missing);
                assert!(Session::disabled_validators().is_empty());
            } else {
                assert_no_offence_or_slash();
                assert!(Session::disabled_validators().is_empty());
            }
            if !should_slash || <Runtime as pallet_staking::Config>::SlashDeferDuration::get() > 0 {
                assert_eq!(validators.balances_and_stakes(), before);
            }
        });
    }
}

#[test]
fn minority_reports_from_different_sessions_do_not_accumulate_into_a_majority() {
    ext().execute_with(|| {
        let validators = Validators::new();
        let before = validators.balances_and_stakes();
        for (session_offset, missing) in [
            (0, (0..12).collect::<Vec<_>>()),
            (1, (12..24).collect::<Vec<_>>()),
        ] {
            validators.start_session(FIRST_SESSION + session_offset);
            validators.end_session_with_missing(&missing);
            assert_no_offence_or_slash();
            assert!(Session::disabled_validators().is_empty());
        }
        assert_eq!(validators.balances_and_stakes(), before);
    });
}

#[test]
fn duplicate_or_nonvalidator_reports_cannot_inflate_a_minority_into_a_majority() {
    ext().execute_with(|| {
        let validators = Validators::new();
        let mut offenders = validators.accounts[..12]
            .iter()
            .cloned()
            .map(|account| (account, Exposure::default()))
            .collect::<Vec<_>>();
        offenders.push(offenders[0].clone());
        offenders.push((AccountId::from([149; 32]), Exposure::default()));
        assert_ok!(
            <Runtime as pallet_im_online::Config>::ReportUnresponsiveness::report_offence(
                vec![],
                UnresponsivenessOffence {
                    session_index: FIRST_SESSION,
                    validator_set_count: VALIDATORS,
                    offenders,
                },
            )
        );
        assert_no_offence_or_slash();
        assert!(Session::disabled_validators().is_empty());
    });
}

#[test]
fn mismatched_session_or_validator_count_is_rejected_without_penalties() {
    ext().execute_with(|| {
        let validators = Validators::new();
        for (session_index, validator_set_count) in [
            (FIRST_SESSION - 1, VALIDATORS),
            (FIRST_SESSION, VALIDATORS + 1),
        ] {
            let offenders = validators.accounts[..13]
                .iter()
                .cloned()
                .map(|account| (account, Exposure::default()))
                .collect::<Vec<_>>();
            assert_eq!(
                <Runtime as pallet_im_online::Config>::ReportUnresponsiveness::report_offence(
                    vec![],
                    UnresponsivenessOffence {
                        session_index,
                        validator_set_count,
                        offenders,
                    },
                ),
                Err(OffenceError::Other(1)),
            );
            assert_no_offence_or_slash();
            assert!(Session::disabled_validators().is_empty());
        }
    });
}

#[test]
fn already_disabled_missing_validator_does_not_push_a_minority_above_the_threshold() {
    for disabled_count in [1, 2] {
        ext().execute_with(|| {
            let validators = Validators::new();
            let disabled = (0..disabled_count).collect::<Vec<_>>();
            for index in &disabled {
                assert!(Session::disable_index(*index));
            }
            let before = validators.balances_and_stakes();
            // Twelve eligible validators stay below the full-set threshold of 13,
            // even when two disablements leave only 23 eligible validators.
            validators
                .end_session_with_missing(&(0..12 + disabled_count as usize).collect::<Vec<_>>());
            assert_no_offence_or_slash();
            assert_eq!(Session::disabled_validators(), disabled);
            assert_eq!(validators.balances_and_stakes(), before);
        });
    }
}

#[test]
fn majority_penalty_excludes_already_disabled_missing_validators() {
    ext().execute_with(|| {
        let validators = Validators::new();
        assert!(Session::disable_index(0));
        let before = validators.balances_and_stakes();
        validators.end_session_with_missing(&(0..14).collect::<Vec<_>>());
        // Thirteen eligible missing validators form a majority of the full set of 25.
        // The previously disabled validator stays observable but receives no new offence.
        let penalized = (1..14).collect::<Vec<_>>();
        let disabled = Session::disabled_validators();
        assert_majority_slashes(&validators, &penalized);
        assert_eq!(disabled, vec![0]);
        if <Runtime as pallet_staking::Config>::SlashDeferDuration::get() > 0 {
            assert_eq!(validators.balances_and_stakes(), before);
        }
    });
}

#[test]
fn recovering_after_a_majority_outage_does_not_create_further_offline_penalties() {
    ext().execute_with(|| {
        let validators = Validators::new();
        validators.end_session_with_missing(&(0..13).collect::<Vec<_>>());
        assert_majority_slashes(&validators, &(0..13).collect::<Vec<_>>());
        let reports = pallet_offences::Reports::<Runtime>::iter().count();
        let queued = queued_slash_count();
        let disabled = Session::disabled_validators();
        let before = validators.balances_and_stakes();

        validators.start_session(FIRST_SESSION + 1);
        assert!(
            disabled.is_empty(),
            "offline reports must not disable block production"
        );
        for index in 0..VALIDATORS as usize {
            validators.note_author(index);
        }
        System::reset_events();
        <LivenessImOnline as OneSessionHandler<AccountId>>::on_before_session_ending();
        System::assert_last_event(RuntimeEvent::ImOnline(pallet_im_online::Event::AllGood));
        assert_eq!(pallet_offences::Reports::<Runtime>::iter().count(), reports);
        assert_eq!(queued_slash_count(), queued);
        assert!(!System::events().iter().any(|event| matches!(
            event.event,
            RuntimeEvent::Staking(pallet_staking::Event::SlashReported { .. })
                | RuntimeEvent::Staking(pallet_staking::Event::Slashed { .. })
        )));
        assert_eq!(Session::disabled_validators(), disabled);
        assert_eq!(validators.balances_and_stakes(), before);
    });
}

#[test]
fn disabled_validator_can_prove_liveness_with_a_signed_heartbeat_next_session() {
    ext().execute_with(|| {
        let validators = Validators::new();
        assert!(Session::disable_index(0));
        validators.start_session(FIRST_SESSION + 1);
        assert_eq!(Session::disabled_validators(), vec![0]);
        assert!(!ImOnline::is_online(0));

        let heartbeat = Heartbeat {
            block_number: System::block_number(),
            session_index: FIRST_SESSION + 1,
            authority_index: 0,
            validators_len: VALIDATORS,
        };
        let invalid_call = pallet_im_online::Call::<Runtime>::heartbeat {
            heartbeat: heartbeat.clone(),
            signature: authority_pair(1).sign(&heartbeat.encode()),
        };
        assert_eq!(
            <ImOnline as ValidateUnsigned>::validate_unsigned(
                TransactionSource::External,
                &invalid_call
            ),
            InvalidTransaction::BadProof.into(),
        );

        let signature = authority_pair(0).sign(&heartbeat.encode());
        let call = pallet_im_online::Call::<Runtime>::heartbeat {
            heartbeat: heartbeat.clone(),
            signature: signature.clone(),
        };
        assert_ok!(<ImOnline as ValidateUnsigned>::validate_unsigned(
            TransactionSource::External,
            &call,
        ));
        assert_ok!(ImOnline::heartbeat(
            RuntimeOrigin::none(),
            heartbeat,
            signature
        ));
        assert!(ImOnline::is_online(0));
        assert_eq!(
            <ImOnline as ValidateUnsigned>::validate_unsigned(TransactionSource::External, &call),
            InvalidTransaction::Stale.into(),
        );

        for index in 1..VALIDATORS as usize {
            validators.note_author(index);
        }
        System::reset_events();
        <LivenessImOnline as OneSessionHandler<AccountId>>::on_before_session_ending();
        System::assert_last_event(RuntimeEvent::ImOnline(pallet_im_online::Event::AllGood));
        assert_no_offence_or_slash();
        // Reporting policy does not erase a pre-existing consensus disablement.
        assert_eq!(Session::disabled_validators(), vec![0]);
    });
}

#[test]
fn late_reenable_does_not_manufacture_an_offline_majority() {
    ext().execute_with(|| {
        let validators = Validators::new();
        let severity = sp_staking::offence::OffenceSeverity(Perbill::from_percent(7));

        // Eight healthy validators are prevented from authoring. Twelve other
        // validators (8..20) genuinely miss liveness; 20..25 have authored blocks.
        for index in 0..8 {
            assert!(Session::disable_index_with_severity(index, severity));
        }
        for index in 20..25 {
            validators.note_author(index);
        }

        // Exercise the real replacement strategy in the same block: an offence
        // disables online validator 20 and re-enables previously disabled 0.
        Session::report_offence(validators.accounts[20].clone(), severity);
        assert_eq!(
            Session::disabled_validators(),
            (1..8).chain(std::iter::once(20)).collect::<Vec<_>>()
        );
        let (tracked_session, disabled) = DisabledDuringSession::<Runtime>::get().unwrap();
        assert_eq!(tracked_session, FIRST_SESSION);
        assert!(disabled.contains(&0));
        assert!(disabled.contains(&20));

        let before = validators.balances_and_stakes();
        validators.end_session_with_missing(&(0..20).collect::<Vec<_>>());
        assert_no_offence_or_slash();
        assert_eq!(validators.balances_and_stakes(), before);
    });
}

#[test]
fn same_session_disable_reenable_history_expires_after_a_full_session_boundary() {
    ext().execute_with(|| {
        let validators = Validators::new();
        // Even a disable/re-enable pair within one block restricts authorship.
        assert!(Session::disable_index(0));
        assert!(Session::reenable_index(0));
        validators.end_session_with_missing(&(0..13).collect::<Vec<_>>());
        assert_no_offence_or_slash();

        // Its old exemption must not hide a genuine majority in the next session.
        validators.start_session(FIRST_SESSION + 1);
        let (tracked_session, disabled) = DisabledDuringSession::<Runtime>::get().unwrap();
        assert_eq!(tracked_session, FIRST_SESSION + 1);
        assert!(disabled.is_empty());
        validators.end_session_with_missing(&(0..13).collect::<Vec<_>>());
        assert_majority_slashes(&validators, &(0..13).collect::<Vec<_>>());
    });
}

#[test]
fn inherited_disablement_is_remembered_if_reenabled_during_the_next_session() {
    ext().execute_with(|| {
        let validators = Validators::new();
        assert!(Session::disable_index(0));
        validators.start_session(FIRST_SESSION + 1);
        assert!(Session::reenable_index(0));

        // No fresh on_disabled callback occurred in this session, so the history
        // must include the disablement inherited through its start hook.
        validators.end_session_with_missing(&(0..13).collect::<Vec<_>>());
        assert_no_offence_or_slash();
    });
}

#[test]
fn upgrade_grace_waits_for_a_complete_session_before_offline_penalties() {
    for stale_session in [None, Some(FIRST_SESSION - 1)] {
        ext().execute_with(|| {
            let validators = Validators::new();
            match stale_session {
                None => DisabledDuringSession::<Runtime>::kill(),
                Some(session) => DisabledDuringSession::<Runtime>::mutate(|history| {
                    history.as_mut().unwrap().0 = session;
                }),
            }

            // Seeing an isolated callback after an upgrade cannot reconstruct
            // the earlier part of the current session and must not end grace.
            assert!(Session::disable_index(0));
            assert!(Session::reenable_index(0));
            validators.end_session_with_missing(&(0..14).collect::<Vec<_>>());
            assert_no_offence_or_slash();

            validators.start_session(FIRST_SESSION + 1);
            validators.end_session_with_missing(&(0..13).collect::<Vec<_>>());
            assert_majority_slashes(&validators, &(0..13).collect::<Vec<_>>());
        });
    }
}

#[test]
fn repeated_disablement_does_not_grow_history_for_the_same_validator() {
    ext().execute_with(|| {
        let validators = Validators::new();
        for _ in 0..100 {
            assert!(Session::disable_index(0));
            assert!(Session::reenable_index(0));
        }
        let (tracked_session, disabled) = DisabledDuringSession::<Runtime>::get().unwrap();
        assert_eq!(tracked_session, FIRST_SESSION);
        assert_eq!(disabled.into_iter().collect::<Vec<_>>(), vec![0]);
        validators.end_session_with_missing(&(0..13).collect::<Vec<_>>());
        assert_no_offence_or_slash();
    });
}

fn assert_equivocation_slashes<O: Offence<Identification>>(validators: &Validators, offence: O) {
    let fraction = offence.slash_fraction(1);
    let expected_validator_slash = fraction * OWN_STAKE;
    let expected_nominator_slash = fraction * NOMINATED_STAKE;
    let before = validators.balances_and_stakes();
    assert!(expected_validator_slash > 0);
    assert!(expected_nominator_slash > 0);

    assert_ok!(<EquivocationReports as ReportOffence<
        AccountId,
        Identification,
        O,
    >>::report_offence(vec![], offence,));
    assert_eq!(pallet_offences::Reports::<Runtime>::iter().count(), 1);
    assert_eq!(
        pallet_staking::ValidatorSlashInEra::<Runtime>::get(ERA, &validators.accounts[0]),
        Some((fraction, expected_validator_slash)),
    );
    assert_eq!(Session::disabled_validators(), vec![0]);

    let defer = <Runtime as pallet_staking::Config>::SlashDeferDuration::get();
    if defer > 0 {
        let slashes = pallet_staking::UnappliedSlashes::<Runtime>::get(ERA + defer + 1);
        assert_eq!(slashes.len(), 1);
        assert_eq!(slashes[0].validator, validators.accounts[0]);
        assert_eq!(slashes[0].own, expected_validator_slash);
        assert_eq!(
            slashes[0].others,
            vec![(validators.nominator.clone(), expected_nominator_slash)]
        );
        assert_eq!(validators.balances_and_stakes(), before);

        // Exercise the real era-start hook that applies deferred financial penalties.
        for step in 1..=defer + 1 {
            let era = ERA + step;
            let session = FIRST_SESSION + step * crate::SessionsPerEra::get();
            pallet_staking::CurrentEra::<Runtime>::put(era);
            pallet_staking::ErasStartSessionIndex::<Runtime>::insert(era, session);
            pallet_session::CurrentIndex::<Runtime>::put(session);
            <Staking as pallet_session::SessionManager<AccountId>>::start_session(session);
            assert_eq!(Staking::active_era().unwrap().index, era);
        }
        assert!(pallet_staking::UnappliedSlashes::<Runtime>::get(ERA + defer + 1).is_empty());
    }

    let after = validators.balances_and_stakes();
    assert_eq!(before[0].0 - after[0].0, expected_validator_slash);
    assert_eq!(before[0].1 - after[0].1, expected_validator_slash);
    assert_eq!(
        before[VALIDATORS as usize].0 - after[VALIDATORS as usize].0,
        expected_nominator_slash
    );
    assert_eq!(
        before[VALIDATORS as usize].1 - after[VALIDATORS as usize].1,
        expected_nominator_slash
    );
    assert_eq!(
        before[1..VALIDATORS as usize],
        after[1..VALIDATORS as usize]
    );
}

#[test]
fn babe_equivocation_still_slashes_validator_and_nominator() {
    ext().execute_with(|| {
        let validators = Validators::new();
        let offence = pallet_babe::EquivocationOffence {
            slot: 42u64.into(),
            session_index: FIRST_SESSION,
            validator_set_count: VALIDATORS,
            offender: (validators.accounts[0].clone(), Exposure::default()),
        };
        assert_equivocation_slashes(&validators, offence);
    });
}

#[test]
fn grandpa_equivocation_still_slashes_validator_and_nominator() {
    ext().execute_with(|| {
        let validators = Validators::new();
        let offence = pallet_grandpa::EquivocationOffence {
            time_slot: pallet_grandpa::TimeSlot {
                set_id: 1,
                round: 42,
            },
            session_index: FIRST_SESSION,
            validator_set_count: VALIDATORS,
            offender: (validators.accounts[0].clone(), Exposure::default()),
        };
        assert_equivocation_slashes(&validators, offence);
    });
}

#[test]
fn concurrent_equivocations_update_existing_disable_severity_and_reject_duplicates() {
    ext().execute_with(|| {
        let validators = Validators::new();
        let offence = |index: usize| pallet_babe::EquivocationOffence {
            slot: 42u64.into(),
            session_index: FIRST_SESSION,
            validator_set_count: VALIDATORS,
            offender: (validators.accounts[index].clone(), Exposure::default()),
        };
        let single_fraction = offence(0).slash_fraction(1);
        let concurrent_fraction = offence(1).slash_fraction(2);
        assert!(concurrent_fraction > single_fraction);
        assert_ok!(EquivocationReports::report_offence(vec![], offence(0)));
        assert_ok!(EquivocationReports::report_offence(vec![], offence(1)));
        assert_eq!(
            pallet_session::DisabledValidators::<Runtime>::get(),
            vec![
                (0, sp_staking::offence::OffenceSeverity(concurrent_fraction)),
                (1, sp_staking::offence::OffenceSeverity(concurrent_fraction)),
            ]
        );
        let before = queued_slash_count();
        assert!(Session::reenable_index(0));
        assert_eq!(
            EquivocationReports::report_offence(vec![], offence(1)),
            Err(OffenceError::DuplicateReport),
        );
        assert_eq!(Session::disabled_validators(), vec![1]);
        assert_eq!(queued_slash_count(), before);
        assert_eq!(pallet_offences::Reports::<Runtime>::iter().count(), 2);
    });
}

#[test]
fn invulnerable_equivocation_does_not_disable_or_slash() {
    ext().execute_with(|| {
        let validators = Validators::new();
        pallet_staking::Invulnerables::<Runtime>::put(vec![validators.accounts[0].clone()]);
        let offence = pallet_babe::EquivocationOffence {
            slot: 42u64.into(),
            session_index: FIRST_SESSION,
            validator_set_count: VALIDATORS,
            offender: (validators.accounts[0].clone(), Exposure::default()),
        };
        assert_ok!(EquivocationReports::report_offence(vec![], offence));
        assert!(Session::disabled_validators().is_empty());
        assert_eq!(queued_slash_count(), 0);
        assert_eq!(
            pallet_staking::ValidatorSlashInEra::<Runtime>::iter().count(),
            0
        );
        assert_eq!(pallet_offences::Reports::<Runtime>::iter().count(), 1);
    });
}

#[test]
fn historical_equivocation_slashes_without_disabling_current_era() {
    ext().execute_with(|| {
        let validators = Validators::new();
        let next_era = ERA + 1;
        let next_session = FIRST_SESSION + crate::SessionsPerEra::get();
        pallet_staking::ActiveEra::<Runtime>::put(ActiveEraInfo {
            index: next_era,
            start: Some(0),
        });
        pallet_staking::CurrentEra::<Runtime>::put(next_era);
        pallet_staking::ErasStartSessionIndex::<Runtime>::insert(next_era, next_session);
        pallet_staking::BondedEras::<Runtime>::put(vec![
            (ERA, FIRST_SESSION),
            (next_era, next_session),
        ]);
        validators.start_session(next_session);
        let offence = pallet_babe::EquivocationOffence {
            slot: 42u64.into(),
            session_index: FIRST_SESSION,
            validator_set_count: VALIDATORS,
            offender: (validators.accounts[0].clone(), Exposure::default()),
        };
        let fraction = offence.slash_fraction(1);
        assert_ok!(EquivocationReports::report_offence(vec![], offence));
        assert!(Session::disabled_validators().is_empty());
        assert_eq!(
            pallet_staking::ValidatorSlashInEra::<Runtime>::get(ERA, &validators.accounts[0]),
            Some((fraction, fraction * OWN_STAKE)),
        );
        assert_eq!(pallet_offences::Reports::<Runtime>::iter().count(), 1);
    });
}

fn liveness_babe_report_weight() -> frame_support::weights::Weight {
    <crate::liveness::EquivocationWeights as pallet_babe::WeightInfo>::report_equivocation(
        VALIDATORS,
        crate::MaxNominators::get(),
    )
}

#[test]
fn exposure_budget_counts_repeated_nominators_across_all_validators() {
    ext().execute_with(|| {
        let _validators = Validators::new();
        let budget = crate::liveness::ExposureWork::<Runtime>::get().unwrap();
        // The fixture has one nominator, but a separate exposure row for every
        // validator. Concurrent offences process all those rows independently.
        assert_eq!(budget.max_nomination_edges, VALIDATORS);
        assert_eq!(budget.max_validators, VALIDATORS);
        assert!(liveness_babe_report_weight().all_lte(crate::BlockWeights::get().max_block));
    });
}

#[test]
fn exposure_budget_retains_the_largest_historical_aggregate() {
    ext().execute_with(|| {
        let validators = Validators::new();
        let current_weight = liveness_babe_report_weight();
        let previous_era = ERA - 1;
        let historical_rows = 100u32;
        EraInfo::<Runtime>::set_exposure(
            previous_era,
            &validators.accounts[0],
            Exposure {
                total: OWN_STAKE + Balance::from(historical_rows) * UNIT,
                own: OWN_STAKE,
                others: (0..historical_rows)
                    .map(|index| IndividualExposure {
                        who: AccountId::from([index as u8 + 1; 32]),
                        value: UNIT,
                    })
                    .collect(),
            },
        );
        pallet_staking::BondedEras::<Runtime>::put(vec![
            (previous_era, FIRST_SESSION - crate::SessionsPerEra::get()),
            (ERA, FIRST_SESSION),
        ]);
        crate::liveness::refresh_exposure_work();
        let budget = crate::liveness::ExposureWork::<Runtime>::get().unwrap();
        assert_eq!(budget.max_nomination_edges, historical_rows);
        assert!(liveness_babe_report_weight().ref_time() > current_weight.ref_time());
    });
}

#[test]
fn missing_or_stale_exposure_budget_keeps_the_full_static_fallback() {
    for stale in [false, true] {
        ext().execute_with(|| {
            let _validators = Validators::new();
            if stale {
                pallet_staking::CurrentEra::<Runtime>::put(ERA + 1);
            } else {
                crate::liveness::ExposureWork::<Runtime>::kill();
            }
            let static_weight = <() as pallet_babe::WeightInfo>::report_equivocation(
                VALIDATORS,
                crate::MaxNominators::get(),
            );
            let weight = liveness_babe_report_weight();
            assert!(weight.ref_time() >= static_weight.ref_time());
            assert!(weight.ref_time() > crate::BlockWeights::get().max_block.ref_time());
        });
    }
}

#[test]
#[allow(deprecated)]
fn any_legacy_exposure_prevents_a_partial_paged_weight_estimate() {
    ext().execute_with(|| {
        let validators = Validators::new();
        pallet_staking::ErasStakers::<Runtime>::insert(
            ERA,
            &validators.accounts[0],
            Exposure::<AccountId, Balance>::default(),
        );
        crate::liveness::refresh_exposure_work();
        assert!(crate::liveness::ExposureWork::<Runtime>::get().is_none());
        assert!(
            liveness_babe_report_weight().ref_time()
                > crate::BlockWeights::get().max_block.ref_time()
        );
    });
}

#[test]
fn exposure_scan_limit_never_publishes_a_partial_budget() {
    ext().execute_with(|| {
        let validators = Validators::new();
        let overview =
            pallet_staking::ErasStakersOverview::<Runtime>::get(ERA, &validators.accounts[0])
                .unwrap();
        for index in 0u32..4_097 {
            let mut account = [248u8; 32];
            account[28..].copy_from_slice(&index.to_le_bytes());
            pallet_staking::ErasStakersOverview::<Runtime>::insert(
                ERA,
                AccountId::from(account),
                overview.clone(),
            );
        }
        crate::liveness::refresh_exposure_work();
        assert!(crate::liveness::ExposureWork::<Runtime>::get().is_none());
        assert!(
            liveness_babe_report_weight().ref_time()
                > crate::BlockWeights::get().max_block.ref_time()
        );
    });
}

#[test]
fn equivocation_weight_tracks_growing_future_queues_without_a_session_refresh() {
    ext().execute_with(|| {
        let validators = Validators::new();
        let before = liveness_babe_report_weight();
        let slash = pallet_staking::UnappliedSlash {
            validator: validators.accounts[0].clone(),
            own: UNIT,
            others: vec![(validators.nominator.clone(), UNIT); 100],
            reporters: vec![validators.accounts[1].clone()],
            payout: 0,
        };
        // An overdue queue is not read by late immediate application.
        pallet_staking::UnappliedSlashes::<Runtime>::insert(ERA, vec![slash.clone(); 20]);
        assert_eq!(liveness_babe_report_weight(), before);

        let future_era = ERA + <Runtime as pallet_staking::Config>::SlashDeferDuration::get() + 1;
        pallet_staking::UnappliedSlashes::<Runtime>::insert(future_era, vec![slash; 20]);
        let after = liveness_babe_report_weight();
        assert!(after.ref_time() > before.ref_time());
        assert!(after.proof_size() > before.proof_size());
        assert_eq!(
            crate::liveness::ExposureWork::<Runtime>::get()
                .unwrap()
                .max_nomination_edges,
            VALIDATORS,
        );
    });
}

#[test]
#[cfg(not(feature = "private-net"))]
fn queue_growth_envelope_covers_reports_priced_before_earlier_wrapped_calls() {
    ext().execute_with(|| {
        let validators = Validators::new();
        let budget = crate::liveness::ExposureWork::<Runtime>::get().unwrap();
        let growth = crate::liveness::EquivocationWeights::full_block_queue_growth(
            budget.max_validators,
            budget.max_nomination_edges,
        );
        let due = ERA + <Runtime as pallet_staking::Config>::SlashDeferDuration::get() + 1;
        let before_bytes = pallet_staking::UnappliedSlashes::<Runtime>::get(due)
            .encode()
            .len();
        // Increasing concurrent severity causes earlier offenders to queue new
        // financial deltas, reproducing the state growth inside wrapped reports.
        for index in 0..7 {
            let offence = pallet_babe::EquivocationOffence {
                slot: 42u64.into(),
                session_index: FIRST_SESSION,
                validator_set_count: VALIDATORS,
                offender: (validators.accounts[index].clone(), Exposure::default()),
            };
            assert_ok!(EquivocationReports::report_offence(vec![], offence));
        }
        let after_bytes = pallet_staking::UnappliedSlashes::<Runtime>::get(due)
            .encode()
            .len();
        assert!(after_bytes > before_bytes);
        assert!((after_bytes - before_bytes) as u64 <= growth);
    });
}

#[test]
fn runtime_upgrade_rebuilds_the_known_exposure_budget_with_charged_bounded_work() {
    ext().execute_with(|| {
        let _validators = Validators::new();
        crate::liveness::ExposureWork::<Runtime>::kill();
        let weight = <crate::Liveness as frame_support::traits::Hooks<crate::BlockNumber>>::on_runtime_upgrade();
        assert!(crate::liveness::ExposureWork::<Runtime>::get().is_some());
        assert!(weight.ref_time() > 0);
        assert!(weight.all_lte(crate::BlockWeights::get().max_block));
        assert!(liveness_babe_report_weight().all_lte(crate::BlockWeights::get().max_block));
    });
}

#[cfg(not(feature = "private-net"))]
mod deferred_slashes;
mod equivocation_admission;
mod session_boundaries;
