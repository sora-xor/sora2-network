// Local runtime reproductions. The included fixture exercises the real offence,
// staking and era-start handlers. It does not fabricate or verify signed BABE
// equivocation evidence or a historical session-key ownership proof.
include!("../../../runtime/src/tests/liveness.rs");

fn deferred_repro_report(validators: &Validators) -> Perbill {
    let offence = pallet_babe::EquivocationOffence {
        slot: 42u64.into(),
        session_index: FIRST_SESSION,
        validator_set_count: VALIDATORS,
        offender: (validators.accounts[0].clone(), Exposure::default()),
    };
    let fraction = offence.slash_fraction(1);
    assert_ok!(
        <Offences as ReportOffence<AccountId, Identification, _>>::report_offence(vec![], offence,)
    );
    fraction
}

fn deferred_repro_advance_to(target_era: u32) {
    while Staking::active_era().unwrap().index < target_era {
        let era = Staking::active_era().unwrap().index + 1;
        let session = FIRST_SESSION + (era - ERA) * crate::SessionsPerEra::get();
        pallet_staking::CurrentEra::<Runtime>::put(era);
        pallet_staking::ErasStartSessionIndex::<Runtime>::insert(era, session);
        pallet_session::CurrentIndex::<Runtime>::put(session);
        <Staking as pallet_session::SessionManager<AccountId>>::start_session(session);
        assert_eq!(Staking::active_era().unwrap().index, era);
    }
}

#[test]
fn repro_late_first_report_must_not_remain_in_an_already_drained_era() {
    ext().execute_with(|| {
        let validators = Validators::new();
        let account = &validators.accounts[0];
        let before = <Balances as Currency<AccountId>>::total_balance(account);
        let defer = <Runtime as pallet_staking::Config>::SlashDeferDuration::get();
        assert!(
            defer > 0,
            "this reproduction requires mainnet deferred slashing"
        );
        let due = ERA + defer + 1;
        deferred_repro_advance_to(due);
        assert!(pallet_staking::BondedEras::<Runtime>::get()
            .iter()
            .any(|(era, _)| *era == ERA));
        let fraction = deferred_repro_report(&validators);
        assert_eq!(pallet_offences::Reports::<Runtime>::iter().count(), 1);
        deferred_repro_advance_to(due + 1);
        assert_eq!(
            before - <Balances as Currency<AccountId>>::total_balance(account),
            fraction * OWN_STAKE,
            "an admitted late report must still collect its financial penalty",
        );
        assert!(pallet_staking::UnappliedSlashes::<Runtime>::get(due).is_empty());
    });
}

#[test]
fn repro_planned_era_must_not_unlock_funds_before_deferred_application() {
    ext().execute_with(|| {
        let validators = Validators::new();
        let account = &validators.accounts[0];
        let before = <Balances as Currency<AccountId>>::total_balance(account);
        let fraction = deferred_repro_report(&validators);
        assert_ok!(Staking::chill(RuntimeOrigin::signed(account.clone())));
        assert_ok!(Staking::unbond(
            RuntimeOrigin::signed(account.clone()),
            OWN_STAKE
        ));
        let bonding = <Runtime as pallet_staking::Config>::BondingDuration::get();
        let defer = <Runtime as pallet_staking::Config>::SlashDeferDuration::get();
        assert_eq!(bonding, defer + 1);
        let due = ERA + bonding;
        deferred_repro_advance_to(due - 1);
        // Normal final-session state: the next era is elected/planned one session
        // before it becomes active. The era-start hook has not applied its queue.
        pallet_staking::CurrentEra::<Runtime>::put(due);
        let next_session = FIRST_SESSION + (due - ERA) * crate::SessionsPerEra::get();
        pallet_staking::ErasStartSessionIndex::<Runtime>::insert(due, next_session);
        // One reported offence closes the initial span and starts one new span.
        // The public dispatch accepts this exact span count; iter() is private.
        let spans = 2;
        assert_ok!(Staking::withdraw_unbonded(
            RuntimeOrigin::signed(account.clone()),
            spans
        ));
        assert_eq!(
            pallet_staking::Ledger::<Runtime>::get(account).map(|ledger| ledger.total),
            Some(OWN_STAKE),
            "the pending slash's stake must remain held until its active execution era",
        );
        deferred_repro_advance_to(due);
        assert_eq!(
            before - <Balances as Currency<AccountId>>::total_balance(account),
            fraction * OWN_STAKE,
        );
    });
}

#[test]
fn repro_deferred_application_must_use_the_original_offence_era() {
    ext().execute_with(|| {
        let validators = Validators::new();
        let account = &validators.accounts[0];
        let fraction = deferred_repro_report(&validators);
        assert_ok!(Staking::chill(RuntimeOrigin::signed(account.clone())));
        assert_ok!(Staking::unbond(
            RuntimeOrigin::signed(account.clone()),
            OWN_STAKE / 2
        ));
        let defer = <Runtime as pallet_staking::Config>::SlashDeferDuration::get();
        assert!(defer > 0);
        deferred_repro_advance_to(ERA + defer + 1);
        let ledger = pallet_staking::Ledger::<Runtime>::get(account).unwrap();
        let remaining_half = OWN_STAKE / 2 - (fraction * OWN_STAKE) / 2;
        assert_eq!(
            (ledger.active, ledger.unlocking[0].value),
            (remaining_half, remaining_half),
            "stake unbonded in the offence era must share the slash proportionally",
        );
    });
}
