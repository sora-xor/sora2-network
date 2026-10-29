//! Native election behavior after a guarded recovery ownership projection.
//!
//! Public test identities only. These tests exercise native economic calls and
//! the generated RuntimeCall retirement hook; they do not certify a release or
//! execute a maintenance block through the recovery executive.
use crate::{
    AccountId, Balance, Balances, ElectionsPhragmen, Runtime, RuntimeCall, RuntimeOrigin, System,
};
use codec::Encode;
use common::{
    balance,
    recovery::{
        self, AccountReplacement, GuardedStorageEdit, RecoveryChunk, RecoveryPolicy, StorageScope,
    },
};
use frame_support::{
    assert_ok,
    traits::{Currency, OnInitialize},
};
use sp_runtime::traits::Dispatchable;

fn owner(n: u8) -> AccountId {
    AccountId::new([n; 32])
}
fn vote(who: AccountId, targets: Vec<AccountId>, value: Balance) {
    assert_ok!(
        RuntimeCall::ElectionsPhragmen(pallet_elections_phragmen::Call::vote {
            votes: targets,
            value
        })
        .dispatch(RuntimeOrigin::signed(who))
    );
}
fn seed() {
    System::set_block_number(1);
    for n in [201, 202, 203, 204] {
        drop(Balances::deposit_creating(&owner(n), balance!(2000)));
    }
    assert_ok!(ElectionsPhragmen::submit_candidacy(
        RuntimeOrigin::signed(owner(203)),
        0
    ));
    assert_ok!(ElectionsPhragmen::submit_candidacy(
        RuntimeOrigin::signed(owner(204)),
        1
    ));
    vote(owner(201), vec![owner(203)], balance!(1000));
    vote(owner(202), vec![owner(204)], balance!(100));
}
fn move_record(
    edits: &mut Vec<GuardedStorageEdit>,
    source: Vec<u8>,
    destination: Vec<u8>,
    after: Option<Vec<u8>>,
) {
    let Some(bytes) = sp_io::storage::get(&source) else {
        assert!(after.is_none());
        return;
    };
    assert!(!sp_io::storage::exists(&destination));
    edits.extend([
        GuardedStorageEdit {
            scope: StorageScope::Top,
            key: source,
            before: Some(sp_io::hashing::blake2_256(&bytes)),
            after: None,
        },
        GuardedStorageEdit {
            scope: StorageScope::Top,
            key: destination,
            before: None,
            after: Some(after.unwrap_or_else(|| bytes.to_vec())),
        },
    ]);
}
fn recover() -> RecoveryPolicy {
    let mut edits = vec![];
    for (source, dest) in [
        (
            frame_system::Account::<Runtime>::hashed_key_for(owner(201)),
            frame_system::Account::<Runtime>::hashed_key_for(owner(211)),
        ),
        (
            pallet_balances::Locks::<Runtime>::hashed_key_for(owner(201)),
            pallet_balances::Locks::<Runtime>::hashed_key_for(owner(211)),
        ),
    ] {
        move_record(&mut edits, source, dest, None);
    }
    let mut voter = pallet_elections_phragmen::Voting::<Runtime>::get(owner(201));
    voter.votes.clear();
    move_record(
        &mut edits,
        pallet_elections_phragmen::Voting::<Runtime>::hashed_key_for(owner(201)),
        pallet_elections_phragmen::Voting::<Runtime>::hashed_key_for(owner(211)),
        Some(voter.encode()),
    );
    edits.sort_by(|a, b| a.key.cmp(&b.key));
    let chunk = RecoveryChunk { index: 0, edits };
    chunk.validate().unwrap();
    let policy = RecoveryPolicy {
        format_version: 2,
        authorization_domain: common::recovery::RecoveryDomain::public_fixture(),
        checkpoint_hash: recovery::CHECKPOINT_HASH,
        checkpoint_state_root: recovery::CHECKPOINT_STATE_ROOT,
        replacements: vec![AccountReplacement {
            old: owner(201),
            new: owner(211),
        }],
        council: vec![owner(250)],
        chunk_hashes: vec![chunk.commitment()],
        audit_chunk_hashes: vec![[1; 32]],
        evidence_commitment: [2; 32],
        terminal_audit_commitment: [3; 32],
    };
    recovery::storage::initialize(&policy).unwrap();
    recovery::storage::apply_chunk(&policy, &chunk).unwrap();
    System::set_block_number(recovery::CHECKPOINT_NUMBER + 2);
    policy
}
fn assert_old_rejected() {
    for call in [
        pallet_elections_phragmen::Call::vote {
            votes: vec![owner(204)],
            value: balance!(100),
        },
        pallet_elections_phragmen::Call::remove_voter {},
    ] {
        let before = sp_io::storage::root(sp_runtime::StateVersion::V1);
        assert_eq!(
            RuntimeCall::ElectionsPhragmen(call)
                .dispatch(RuntimeOrigin::signed(owner(201)))
                .unwrap_err()
                .error,
            sp_runtime::DispatchError::Other("Retired recovery account")
        );
        assert_eq!(before, sp_io::storage::root(sp_runtime::StateVersion::V1));
    }
}
fn remove_new() {
    assert_ok!(
        RuntimeCall::ElectionsPhragmen(pallet_elections_phragmen::Call::remove_voter {})
            .dispatch(RuntimeOrigin::signed(owner(211)))
    );
}
#[test]
fn suspended_election_voter_has_no_approval_and_native_revote_uses_retained_deposit() {
    sp_io::TestExternalities::default().execute_with(|| {
        seed();
        let original = pallet_elections_phragmen::Voting::<Runtime>::get(owner(201));
        let info = System::account(owner(201));
        let locks = Balances::locks(&owner(201));
        let issuance = Balances::total_issuance();
        let innocent = pallet_elections_phragmen::Voting::<Runtime>::get(owner(202));
        let policy = recover();
        crate::incident_recovery::with_test_policy(&policy, || {
            assert_eq!(Balances::total_issuance(), issuance);
            assert_eq!(System::account(owner(211)), info);
            assert_eq!(Balances::locks(&owner(211)), locks);
            assert!(!frame_system::Account::<Runtime>::contains_key(owner(201)));
            assert_old_rejected();
            let saved = pallet_elections_phragmen::Voting::<Runtime>::get(owner(211));
            assert!(saved.votes.is_empty());
            assert_eq!(
                (saved.stake, saved.deposit),
                (original.stake, original.deposit)
            );
            assert_eq!(
                pallet_elections_phragmen::Voting::<Runtime>::iter().count(),
                2
            );
            let block = crate::ElectionsTermDuration::get();
            ElectionsPhragmen::on_initialize(block);
            let members = pallet_elections_phragmen::Members::<Runtime>::get();
            assert_eq!(
                members.iter().map(|m| m.who.clone()).collect::<Vec<_>>(),
                vec![owner(204)]
            );
            assert_eq!(
                pallet_collective::Prime::<Runtime, pallet_collective::Instance1>::get(),
                Some(owner(204))
            );
            assert_eq!(
                pallet_elections_phragmen::Voting::<Runtime>::get(owner(211)),
                saved
            );
            assert_eq!(
                pallet_elections_phragmen::Voting::<Runtime>::get(owner(202)),
                innocent
            );
            assert_eq!(System::account(owner(211)), info);
            assert_eq!(Balances::locks(&owner(211)), locks);
            let reserved = Balances::reserved_balance(owner(211));
            vote(owner(211), vec![owner(204)], balance!(250));
            assert_eq!(Balances::reserved_balance(owner(211)), reserved);
            let current = pallet_elections_phragmen::Voting::<Runtime>::get(owner(211));
            assert_eq!(current.votes, vec![owner(204)]);
            assert_eq!(current.deposit, original.deposit);
            assert_eq!(current.stake, balance!(250));
            let free = Balances::free_balance(owner(211));
            remove_new();
            assert!(!pallet_elections_phragmen::Voting::<Runtime>::contains_key(
                owner(211)
            ));
            assert_eq!(
                Balances::reserved_balance(owner(211)),
                reserved - original.deposit
            );
            assert_eq!(Balances::free_balance(owner(211)), free + original.deposit);
            assert!(Balances::locks(&owner(211))
                .iter()
                .all(|l| l.id != *b"phrelect"));
            let info = System::account(owner(211));
            assert_eq!(
                RuntimeCall::ElectionsPhragmen(pallet_elections_phragmen::Call::remove_voter {})
                    .dispatch(RuntimeOrigin::signed(owner(211)))
                    .unwrap_err()
                    .error,
                pallet_elections_phragmen::Error::<Runtime>::MustBeVoter.into()
            );
            assert_eq!(System::account(owner(211)), info);
        });
    });
}
#[test]
fn suspended_empty_target_voter_can_remove_directly_without_reminting_or_double_refund() {
    sp_io::TestExternalities::default().execute_with(|| {
        seed();
        let policy = recover();
        crate::incident_recovery::with_test_policy(&policy, || {
            let saved = pallet_elections_phragmen::Voting::<Runtime>::get(owner(211));
            assert!(saved.votes.is_empty());
            let free = Balances::free_balance(owner(211));
            let reserved = Balances::reserved_balance(owner(211));
            let issuance = Balances::total_issuance();
            remove_new();
            assert_eq!(Balances::free_balance(owner(211)), free + saved.deposit);
            assert_eq!(
                Balances::reserved_balance(owner(211)),
                reserved - saved.deposit
            );
            assert_eq!(Balances::total_issuance(), issuance);
            assert_old_rejected();
            assert!(!pallet_elections_phragmen::Voting::<Runtime>::contains_key(
                owner(211)
            ));
            assert!(Balances::locks(&owner(211))
                .iter()
                .all(|l| l.id != *b"phrelect"));
        });
    });
}

#[test]
fn checkpoint_style_zero_stake_zero_deposit_voter_can_be_removed_without_creating_money() {
    sp_io::TestExternalities::default().execute_with(|| {
        seed();
        // The authenticated checkpoint contains retained zero-stake/deposit
        // Voting records and zero-valued phrelect locks. Reproduce that legacy
        // state explicitly; current vote dispatch would not create it.
        let deposit = pallet_elections_phragmen::Voting::<Runtime>::get(owner(201)).deposit;
        assert_eq!(
            <Balances as frame_support::traits::ReservableCurrency<AccountId>>::unreserve(
                &owner(201),
                deposit
            ),
            0
        );
        pallet_elections_phragmen::Voting::<Runtime>::mutate(owner(201), |v| {
            v.stake = 0;
            v.deposit = 0;
        });
        pallet_balances::Locks::<Runtime>::mutate(owner(201), |locks| {
            let mut changed = locks.clone().into_inner();
            for lock in &mut changed {
                if lock.id == *b"phrelect" {
                    lock.amount = 0;
                }
            }
            *locks = changed.try_into().unwrap();
        });
        frame_system::Account::<Runtime>::mutate(owner(201), |info| info.data.frozen = 0);
        let policy = recover();
        crate::incident_recovery::with_test_policy(&policy, || {
            let saved = pallet_elections_phragmen::Voting::<Runtime>::get(owner(211));
            assert!(saved.votes.is_empty());
            assert_eq!((saved.stake, saved.deposit), (0, 0));
            let free = Balances::free_balance(owner(211));
            let reserved = Balances::reserved_balance(owner(211));
            let issuance = Balances::total_issuance();
            remove_new();
            assert_eq!(Balances::free_balance(owner(211)), free);
            assert_eq!(Balances::reserved_balance(owner(211)), reserved);
            assert_eq!(Balances::total_issuance(), issuance);
            assert!(!pallet_elections_phragmen::Voting::<Runtime>::contains_key(
                owner(211)
            ));
            assert!(Balances::locks(&owner(211))
                .iter()
                .all(|l| l.id != *b"phrelect"));
        });
    });
}
