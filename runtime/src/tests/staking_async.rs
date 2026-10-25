// SPDX-License-Identifier: BSD-4-Clause
use crate::staking_async::{BridgeState, LocalElection, LocalElectionData, LocalSession, State};
use crate::{
    BlockNumber, BlockWeights, ElectionProviderMultiPhase, Runtime, Session, Staking, System,
};
use frame_election_provider_support::{ElectionDataProvider, ElectionProvider};
use frame_support::{
    traits::{EstimateNextNewSession, Hooks},
    weights::WeightMeter,
};
use pallet_staking_async_rc_client::RcClientInterface;
use sp_runtime::Perbill;

fn start_session(index: u32, block: BlockNumber) {
    System::set_block_number(block);
    pallet_session::CurrentIndex::<Runtime>::put(index);
    pallet_babe::GenesisSlot::<Runtime>::put(sp_consensus_babe::Slot::from(1));
    pallet_babe::CurrentSlot::<Runtime>::put(sp_consensus_babe::Slot::from(u64::from(block)));
    pallet_babe::EpochIndex::<Runtime>::put(u64::from(index));
    <LocalSession as pallet_session::SessionManager<crate::AccountId>>::start_session(index);
}

#[test]
fn genesis_era_clock_starts_at_first_block_finalization() {
    framenode_chain_spec::ext().execute_with(|| {
        assert_eq!(State::<Runtime>::get().timestamp_era, Some(0));
        let before = pallet_staking::ActiveEra::<Runtime>::get().unwrap();
        assert_eq!(before.index, 0);
        System::set_block_number(1);
        let first_timestamp = 1_792_876_800_000;
        pallet_timestamp::Now::<Runtime>::put(first_timestamp);
        <crate::StakingAsyncBridge as Hooks<BlockNumber>>::on_finalize(1);
        let first = pallet_staking::ActiveEra::<Runtime>::get().unwrap();
        assert_eq!(first.index, before.index);
        assert_eq!(first.start, Some(first_timestamp));
        assert!(State::<Runtime>::get().timestamp_era.is_none());

        // The ordinary next block must not move the same era's start time.
        System::set_block_number(2);
        pallet_timestamp::Now::<Runtime>::put(first_timestamp + 6_000);
        <crate::StakingAsyncBridge as Hooks<BlockNumber>>::on_finalize(2);
        assert_eq!(pallet_staking::ActiveEra::<Runtime>::get(), Some(first));
    });
}

#[test]
fn async_staking_keeps_six_sessions_and_both_election_windows() {
    framenode_chain_spec::ext().execute_with(|| {
        let period = Session::average_session_length();
        let initial = pallet_staking::ActiveEra::<Runtime>::get().unwrap().index;
        State::<Runtime>::kill();
        pallet_staking::ForceEra::<Runtime>::put(pallet_staking::Forcing::NotForcing);
        for index in 1..=4 {
            start_session(index, index * period + 1);
            assert_eq!(
                pallet_staking::ActiveEra::<Runtime>::get().unwrap().index,
                initial
            );
            assert_eq!(pallet_staking::CurrentEra::<Runtime>::get(), Some(initial));
            assert!(
                <LocalSession as pallet_session::SessionManager<crate::AccountId>>::new_session(
                    index + 1
                )
                .is_none()
            );
        }
        let deadline = LocalElectionData::next_election_prediction(System::block_number());
        assert_eq!(deadline, 5 * period + 1);
        let signed = crate::SignedPhase::get();
        let unsigned = crate::UnsignedPhase::get();
        for (block, expected) in [
            (deadline - signed - unsigned - 1, 0),
            (deadline - signed - unsigned, 1),
            (deadline - unsigned, 2),
        ] {
            System::set_block_number(block);
            pallet_babe::CurrentSlot::<Runtime>::put(sp_consensus_babe::Slot::from(u64::from(
                block,
            )));
            <ElectionProviderMultiPhase as Hooks<BlockNumber>>::on_initialize(block);
            let phase = pallet_election_provider_multi_phase::CurrentPhase::<Runtime>::get();
            assert!(matches!(
                (expected, phase),
                (0, pallet_election_provider_multi_phase::Phase::Off)
                    | (1, pallet_election_provider_multi_phase::Phase::Signed)
                    | (2, pallet_election_provider_multi_phase::Phase::Unsigned(_))
            ));
            assert_eq!(LocalElectionData::next_election_prediction(block), deadline);
            assert_eq!(LocalElection::status(), Err(()));
        }
        // A missing offchain submission still uses the existing on-chain fallback.
        assert!(!pallet_election_provider_multi_phase::QueuedSolution::<
            Runtime,
        >::exists());
        start_session(5, deadline);
        let mut meter = WeightMeter::with_limit(BlockWeights::get().max_block);
        <Staking as Hooks<BlockNumber>>::on_poll(deadline, &mut meter);
        assert_eq!(meter.consumed(), BlockWeights::get().max_block);
        assert!(State::<Runtime>::get().pending.is_some());
        let winners =
            <LocalSession as pallet_session::SessionManager<crate::AccountId>>::new_session(6)
                .unwrap();
        assert!(!winners.is_empty());
        assert_eq!(State::<Runtime>::get().queued_era, Some((6, initial + 1)));
        assert_eq!(
            pallet_staking::ActiveEra::<Runtime>::get().unwrap().index,
            initial
        );
        start_session(6, deadline + period);
        assert_eq!(
            pallet_staking::ActiveEra::<Runtime>::get().unwrap().index,
            initial + 1
        );
        assert!(State::<Runtime>::get().queued_era.is_none());
        assert_eq!(
            pallet_staking::BondedEras::<Runtime>::get().last(),
            Some(&(initial + 1, 6))
        );
    });
}

#[test]
fn async_staking_migrates_an_already_queued_era_without_another_election() {
    framenode_chain_spec::ext().execute_with(|| {
        pallet_session::CurrentIndex::<Runtime>::put(5);
        pallet_staking::CurrentEra::<Runtime>::put(1);
        pallet_staking::ActiveEra::<Runtime>::put(pallet_staking::ActiveEraInfo {
            index: 0,
            start: Some(0),
        });
        crate::staking_async::initialize_from_sync(0, 1, Some(6), 6).unwrap();
        assert_eq!(State::<Runtime>::get().queued_era, Some((6, 1)));
        assert_eq!(LocalElection::status(), Err(()));
        start_session(6, 10);
        assert_eq!(
            pallet_staking::ActiveEra::<Runtime>::get().unwrap().index,
            1
        );
        assert!(State::<Runtime>::get().queued_era.is_none());
        let before = State::<Runtime>::get();
        assert!(crate::staking_async::initialize_from_sync(1, 3, Some(7), 7).is_err());
        assert_eq!(State::<Runtime>::get(), before);
    });
}

#[test]
fn async_staking_rejects_empty_oversized_and_overwritten_validator_reports() {
    framenode_chain_spec::ext().execute_with(|| {
        State::<Runtime>::put(BridgeState::default());
        pallet_staking::CurrentEra::<Runtime>::put(1);
        pallet_staking::ActiveEra::<Runtime>::put(pallet_staking::ActiveEraInfo {
            index: 0,
            start: Some(0),
        });
        LocalSession::validator_set(Vec::new(), 1, None);
        LocalSession::validator_set(vec![crate::AccountId::from([1; 32]); 1201], 1, None);
        LocalSession::validator_set(vec![crate::AccountId::from([1; 32])], 2, None);
        assert!(State::<Runtime>::get().pending.is_none());
        LocalSession::validator_set(vec![crate::AccountId::from([1; 32])], 1, None);
        let first = State::<Runtime>::get();
        LocalSession::validator_set(vec![crate::AccountId::from([2; 32])], 1, None);
        assert_eq!(State::<Runtime>::get(), first);
    });
}

#[test]
fn forced_elections_keep_immediate_deadline_and_failure_keeps_active_authorities() {
    for forcing in [
        pallet_staking::Forcing::ForceNew,
        pallet_staking::Forcing::ForceAlways,
    ] {
        framenode_chain_spec::ext().execute_with(|| {
            State::<Runtime>::kill();
            pallet_staking::ForceEra::<Runtime>::put(forcing);
            let authorities = Session::validators();
            start_session(1, 11);
            assert_eq!(State::<Runtime>::get().election_deadline, Some(11));
            // Force before the snapshot window: old multi-phase fallback fails
            // closed too. Neither an empty set nor activation is manufactured.
            pallet_election_provider_multi_phase::Snapshot::<Runtime>::kill();
            pallet_election_provider_multi_phase::QueuedSolution::<Runtime>::kill();
            assert!(
                <LocalSession as pallet_session::SessionManager<crate::AccountId>>::new_session(2)
                    .is_none()
            );
            assert_eq!(Session::validators(), authorities);
            assert!(State::<Runtime>::get().queued_era.is_none());
            assert_eq!(
                pallet_staking::ActiveEra::<Runtime>::get().unwrap().index,
                0
            );
        });
    }
}

#[test]
fn local_financial_offence_adapter_never_disables_consensus_authorities() {
    use sp_staking::offence::{OffenceDetails, OnOffenceHandler};
    framenode_chain_spec::ext().execute_with(|| {
        let validator = Session::validators()[0].clone();
        let before = Session::disabled_validators();
        crate::staking_async::LocalOffences::on_offence(
            &[OffenceDetails {
                offender: (validator, Default::default()),
                reporters: Vec::new(),
            }],
            &[Perbill::from_percent(1)],
            Session::current_index(),
        );
        assert_eq!(Session::disabled_validators(), before);
    });
}

#[test]
fn upgrade_at_every_session_and_late_election_phase_preserves_activation_session() {
    // Include late session-four blocks with an existing signed/unsigned snapshot.
    for position in 0..8 {
        framenode_chain_spec::ext().execute_with(|| {
            let period = Session::average_session_length();
            let session = position.min(5);
            let extra = match position { 6 => period / 2, 7 => period * 3 / 4, _ => 0 };
            let session = if position >= 6 { 4 } else { session };
            let block = session * period + 1 + extra;
            State::<Runtime>::kill();
            pallet_staking::ForceEra::<Runtime>::put(pallet_staking::Forcing::NotForcing);
            pallet_session::CurrentIndex::<Runtime>::put(session);
            System::set_block_number(block);
            pallet_babe::GenesisSlot::<Runtime>::put(sp_consensus_babe::Slot::from(1));
            pallet_babe::CurrentSlot::<Runtime>::put(sp_consensus_babe::Slot::from(u64::from(block)));
            pallet_babe::EpochIndex::<Runtime>::put(u64::from(session));
            let expected = 5 * period + 1;
            if session == 5 {
                pallet_staking::CurrentEra::<Runtime>::put(1);
                crate::staking_async::initialize_from_sync(0, 1, Some(6), 6).unwrap();
            } else {
                crate::staking_async::initialize_from_sync(0, 0, Some(0), session + 1).unwrap();
                assert_eq!(LocalElectionData::next_election_prediction(block), expected);
                if extra > 0 {
                    <ElectionProviderMultiPhase as Hooks<BlockNumber>>::on_initialize(block);
                    assert!(pallet_election_provider_multi_phase::Snapshot::<Runtime>::exists());
                }
                // The next actual report at session five must consume, rather
                // than restart, the existing election window.
                start_session(5, expected);
                assert_eq!(State::<Runtime>::get().election_deadline, Some(expected));
                LocalSession::validator_set(Session::validators(), 1, None);
                assert!(<LocalSession as pallet_session::SessionManager<crate::AccountId>>::new_session(6).is_some());
            }
            start_session(6, 6 * period + 1);
            assert_eq!(pallet_staking::ActiveEra::<Runtime>::get().unwrap().index, 1);
            assert_eq!(pallet_staking::BondedEras::<Runtime>::get().last(), Some(&(1, 6)));
        });
    }
}

#[test]
fn undersized_election_rolls_back_round_and_retains_previous_authorities() {
    framenode_chain_spec::ext().execute_with(|| {
        let validators = Session::validators();
        let period = Session::average_session_length();
        start_session(4, 4 * period + 1);
        let deadline = LocalElectionData::next_election_prediction(System::block_number());
        let open = deadline - crate::SignedPhase::get() - crate::UnsignedPhase::get();
        System::set_block_number(open);
        pallet_babe::CurrentSlot::<Runtime>::put(sp_consensus_babe::Slot::from(u64::from(open)));
        <ElectionProviderMultiPhase as Hooks<BlockNumber>>::on_initialize(open);
        let round = pallet_election_provider_multi_phase::Round::<Runtime>::get();
        let snapshot = pallet_election_provider_multi_phase::Snapshot::<Runtime>::get();
        pallet_staking::legacy::MinimumValidatorCount::<Runtime>::put(validators.len() as u32 + 1);
        assert_eq!(
            LocalElection::elect(0),
            Err(crate::staking_async::LocalElectionError::TooFewValidators)
        );
        assert_eq!(
            pallet_election_provider_multi_phase::Round::<Runtime>::get(),
            round
        );
        assert_eq!(
            pallet_election_provider_multi_phase::Snapshot::<Runtime>::get(),
            snapshot
        );
        assert_eq!(Session::validators(), validators);
        assert!(State::<Runtime>::get().pending.is_none());
        assert_eq!(
            pallet_staking::ActiveEra::<Runtime>::get().unwrap().index,
            0
        );
    });
}

#[test]
fn unbonding_before_and_after_planning_keeps_legacy_unlock_eras() {
    use frame_support::{assert_ok, traits::Currency};
    framenode_chain_spec::ext().execute_with(|| {
        let period = Session::average_session_length();
        let stake = 1_000_000_000_000_000_000_000u128;
        let before = crate::AccountId::from([81; 32]);
        let after = crate::AccountId::from([82; 32]);
        for account in [&before, &after] {
            let _ = <crate::Balances as Currency<crate::AccountId>>::make_free_balance_be(
                account,
                stake * 2,
            );
            assert_ok!(Staking::bond(
                crate::RuntimeOrigin::signed(account.clone()),
                stake,
                pallet_staking::RewardDestination::Stash
            ));
        }
        start_session(4, 4 * period + 1);
        assert_eq!(pallet_staking::CurrentEra::<Runtime>::get(), Some(0));
        assert_ok!(Staking::unbond(
            crate::RuntimeOrigin::signed(before.clone()),
            stake
        ));
        start_session(5, 5 * period + 1);
        assert_eq!(pallet_staking::CurrentEra::<Runtime>::get(), Some(1));
        assert_ok!(Staking::unbond(
            crate::RuntimeOrigin::signed(after.clone()),
            stake
        ));
        let duration = <Runtime as pallet_staking::Config>::BondingDuration::get();
        assert_eq!(
            pallet_staking::Ledger::<Runtime>::get(before)
                .unwrap()
                .unlocking[0]
                .era,
            duration
        );
        assert_eq!(
            pallet_staking::Ledger::<Runtime>::get(after)
                .unwrap()
                .unlocking[0]
                .era,
            duration + 1
        );
    });
}

#[test]
fn actual_session_queue_announces_before_activating_async_election() {
    framenode_chain_spec::ext().execute_with(|| {
        let period = Session::average_session_length();
        let initial_validators = Session::validators();
        let deadline = 5 * period + 1;
        pallet_staking::ForceEra::<Runtime>::put(pallet_staking::Forcing::NotForcing);
        for index in 1..=6 {
            if index == 5 {
                for block in [
                    deadline - crate::SignedPhase::get() - crate::UnsignedPhase::get(),
                    deadline - crate::UnsignedPhase::get(),
                ] {
                    System::set_block_number(block);
                    pallet_babe::CurrentSlot::<Runtime>::put(sp_consensus_babe::Slot::from(
                        u64::from(block),
                    ));
                    <ElectionProviderMultiPhase as Hooks<BlockNumber>>::on_initialize(block);
                }
            }
            let queued_before = pallet_session::QueuedKeys::<Runtime>::get()
                .into_iter()
                .map(|(account, _)| account)
                .collect::<Vec<_>>();
            let block = index * period + 1;
            System::set_block_number(block);
            pallet_babe::GenesisSlot::<Runtime>::put(sp_consensus_babe::Slot::from(1));
            pallet_babe::CurrentSlot::<Runtime>::put(sp_consensus_babe::Slot::from(u64::from(
                block,
            )));
            pallet_babe::EpochIndex::<Runtime>::put(u64::from(index));
            pallet_babe::Initialized::<Runtime>::put(None::<sp_consensus_babe::digests::PreDigest>);
            Session::rotate_session();
            assert_eq!(Session::current_index(), index);
            assert_eq!(Session::validators(), queued_before);
            if index < 6 {
                assert_eq!(
                    pallet_staking::ActiveEra::<Runtime>::get().unwrap().index,
                    0
                );
                assert_eq!(Session::validators(), initial_validators);
            }
            if index == 5 {
                assert_eq!(State::<Runtime>::get().queued_era, Some((6, 1)));
                assert!(!pallet_session::QueuedKeys::<Runtime>::get().is_empty());
            }
        }
        assert_eq!(
            pallet_staking::ActiveEra::<Runtime>::get().unwrap().index,
            1
        );
        assert!(State::<Runtime>::get().queued_era.is_none());
    });
}
