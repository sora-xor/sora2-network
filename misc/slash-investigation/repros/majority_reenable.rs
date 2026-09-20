// Isolated regression: register this file as a runtime test module to execute.
include!("../../../runtime/src/tests/liveness.rs");

#[test]
fn repro_late_reenable_manufactures_majority() {
    ext().execute_with(|| {
        let validators = Validators::new();
        let severity = sp_staking::offence::OffenceSeverity(Perbill::from_percent(7));

        // These eight healthy validators cannot author during this session.
        // Fill the real strategy's eight-slot limit using the offline severity.
        for index in 0..8 {
            assert!(Session::disable_index_with_severity(index, severity));
        }
        assert_eq!(Session::disabled_validators(), (0..8).collect::<Vec<_>>());

        // Only twelve eligible validators, indices 8..20, actually miss liveness.
        // All five remaining eligible validators have already authored blocks.
        for index in 20..25 {
            validators.note_author(index);
        }
        assert_eq!(queued_slash_count(), 0);

        // A late offence replaces index 0 with the already-online index 20.
        // Use the configured strategy, rather than editing the disabled storage.
        Session::report_offence(validators.accounts[20].clone(), severity);
        assert_eq!(
            Session::disabled_validators(),
            (1..8).chain(std::iter::once(20)).collect::<Vec<_>>()
        );
        assert_eq!(queued_slash_count(), 0);

        System::reset_events();
        <ImOnline as OneSessionHandler<AccountId>>::on_before_session_ending();

        let offline_slash_reports = System::events()
            .iter()
            .filter(|record| matches!(
                record.event,
                RuntimeEvent::Staking(pallet_staking::Event::SlashReported {
                    fraction,
                    ..
                }) if fraction == Perbill::from_percent(7)
            ))
            .count();
        let queued = queued_slash_count();
        assert_eq!(
            offline_slash_reports,
            0,
            "only 12 eligible validators missed liveness; late re-enabling of index 0 \
             must not manufacture a 13-validator majority: observed \
             {offline_slash_reports} offline 7% slash reports and {queued} queued slashes"
        );
    });
}
