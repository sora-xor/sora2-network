// This file is part of the SORA network and Polkaswap app.
// SPDX-License-Identifier: BSD-4-Clause

use crate::{mock::*, Call, Error, Event, Guardians, HaltRecord, LastHalt};
use codec::{Decode, Encode};
use common::recovery::BRIDGE_HALT_STORAGE_KEY;
use frame_support::{
    assert_noop, assert_ok,
    dispatch::{DispatchClass, GetDispatchInfo, Pays},
    BoundedVec,
};
use sp_runtime::{transaction_validity::InvalidTransaction, DispatchError, StateVersion};

fn reason(text: &str) -> BoundedVec<u8, MaxReasonLength> {
    text.as_bytes().to_vec().try_into().unwrap()
}

fn latch() -> Option<Vec<u8>> {
    sp_io::storage::get(BRIDGE_HALT_STORAGE_KEY).map(|value| value.to_vec())
}

fn with_guardian(test: impl FnOnce()) {
    new_test_ext().execute_with(|| {
        assert_ok!(BridgeHalt::add_guardian(RuntimeOrigin::root(), GUARDIAN));
        System::reset_events();
        test()
    })
}

#[test]
fn guardian_halt_sets_the_bridge_latch_and_records_the_message() {
    with_guardian(|| {
        System::set_block_number(7);
        assert_eq!(BridgeHalt::validate_halt(&GUARDIAN), Ok(()));
        assert_ok!(BridgeHalt::halt(
            RuntimeOrigin::signed(GUARDIAN),
            reason("relayer key leaked")
        ));
        assert_eq!(latch(), Some(vec![1]));
        assert!(BridgeHalt::is_halted());
        assert_eq!(
            LastHalt::<Test>::get(),
            Some(HaltRecord {
                guardian: GUARDIAN,
                block: 7,
                reason: reason("relayer key leaked"),
            })
        );
        System::assert_last_event(
            Event::BridgeHalted {
                guardian: GUARDIAN,
                reason: reason("relayer key leaked"),
            }
            .into(),
        );
    });
}

#[test]
fn halt_is_operational_and_accepts_an_empty_message() {
    with_guardian(|| {
        let info = Call::<Test>::halt { reason: reason("") }.get_dispatch_info();
        assert_eq!(info.class, DispatchClass::Operational);
        // The runtime's fee hook makes the call free, and only for guardians.
        // Pays::No would let any signer submit failing calls for free.
        assert_eq!(info.pays_fee, Pays::Yes);
        assert_ok!(BridgeHalt::halt(
            RuntimeOrigin::signed(GUARDIAN),
            reason("")
        ));
        assert!(BridgeHalt::is_halted());
    });
}

#[test]
fn other_accounts_and_origins_have_no_effect() {
    with_guardian(|| {
        let before = sp_io::storage::root(StateVersion::V1);
        assert_eq!(
            BridgeHalt::validate_halt(&OTHER),
            Err(InvalidTransaction::BadSigner.into())
        );
        assert_noop!(
            BridgeHalt::halt(RuntimeOrigin::signed(OTHER), reason("x")),
            Error::<Test>::NotGuardian
        );
        assert_noop!(
            BridgeHalt::halt(RuntimeOrigin::root(), reason("x")),
            DispatchError::BadOrigin
        );
        assert_noop!(
            BridgeHalt::halt(RuntimeOrigin::none(), reason("x")),
            DispatchError::BadOrigin
        );
        assert_eq!(latch(), None);
        assert_eq!(LastHalt::<Test>::get(), None);
        assert!(System::events().is_empty());
        assert_eq!(sp_io::storage::root(StateVersion::V1), before);
    });
}

#[test]
fn guardians_pass_admission_only_while_the_bridges_run() {
    with_guardian(|| {
        assert_ok!(BridgeHalt::halt(
            RuntimeOrigin::signed(GUARDIAN),
            reason("first")
        ));
        assert_eq!(
            BridgeHalt::validate_halt(&GUARDIAN),
            Err(InvalidTransaction::Stale.into())
        );
        assert_noop!(
            BridgeHalt::halt(RuntimeOrigin::signed(GUARDIAN), reason("second")),
            Error::<Test>::AlreadyHalted
        );
        assert_eq!(LastHalt::<Test>::get().unwrap().reason, reason("first"));
    });
}

#[test]
fn any_existing_latch_counts_as_halted() {
    // The recovery set [1]; bridge pallets treat any value, even empty, as halted.
    for value in [vec![1u8], vec![0], vec![]] {
        with_guardian(|| {
            sp_io::storage::set(BRIDGE_HALT_STORAGE_KEY, &value);
            assert!(BridgeHalt::is_halted());
            assert_eq!(
                BridgeHalt::validate_halt(&GUARDIAN),
                Err(InvalidTransaction::Stale.into())
            );
            assert_noop!(
                BridgeHalt::halt(RuntimeOrigin::signed(GUARDIAN), reason("x")),
                Error::<Test>::AlreadyHalted
            );
            assert_ok!(BridgeHalt::resume(RuntimeOrigin::root()));
            assert_eq!(latch(), None);
        });
    }
}

#[test]
fn only_the_resume_origin_reopens_the_bridges() {
    with_guardian(|| {
        assert_noop!(
            BridgeHalt::resume(RuntimeOrigin::root()),
            Error::<Test>::NotHalted
        );
        assert_ok!(BridgeHalt::halt(
            RuntimeOrigin::signed(GUARDIAN),
            reason("x")
        ));
        assert_noop!(
            BridgeHalt::resume(RuntimeOrigin::signed(GUARDIAN)),
            DispatchError::BadOrigin
        );
        assert_noop!(
            BridgeHalt::resume(RuntimeOrigin::signed(OTHER)),
            DispatchError::BadOrigin
        );
        assert_ok!(BridgeHalt::resume(RuntimeOrigin::root()));
        assert_eq!(latch(), None);
        System::assert_last_event(Event::BridgeResumed.into());
        // The record stays as history, and a guardian can halt again.
        assert!(LastHalt::<Test>::get().is_some());
        assert_eq!(BridgeHalt::validate_halt(&GUARDIAN), Ok(()));
        assert_ok!(BridgeHalt::halt(
            RuntimeOrigin::signed(GUARDIAN),
            reason("again")
        ));
    });
}

#[test]
fn governance_manages_the_guardian_list() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            BridgeHalt::add_guardian(RuntimeOrigin::signed(GUARDIAN), GUARDIAN),
            DispatchError::BadOrigin
        );
        assert_ok!(BridgeHalt::add_guardian(RuntimeOrigin::root(), GUARDIAN));
        System::assert_last_event(Event::GuardianAdded { account: GUARDIAN }.into());
        assert_noop!(
            BridgeHalt::add_guardian(RuntimeOrigin::root(), GUARDIAN),
            Error::<Test>::AlreadyGuardian
        );
        assert_ok!(BridgeHalt::add_guardian(RuntimeOrigin::root(), 3));
        assert_ok!(BridgeHalt::add_guardian(RuntimeOrigin::root(), 4));
        assert_noop!(
            BridgeHalt::add_guardian(RuntimeOrigin::root(), 5),
            Error::<Test>::TooManyGuardians
        );
        // A full list still reports an existing member as one.
        assert_noop!(
            BridgeHalt::add_guardian(RuntimeOrigin::root(), 4),
            Error::<Test>::AlreadyGuardian
        );
        assert_eq!(
            Guardians::<Test>::get().into_iter().collect::<Vec<_>>(),
            vec![GUARDIAN, 3, 4]
        );

        assert_noop!(
            BridgeHalt::remove_guardian(RuntimeOrigin::signed(GUARDIAN), GUARDIAN),
            DispatchError::BadOrigin
        );
        assert_noop!(
            BridgeHalt::remove_guardian(RuntimeOrigin::root(), OTHER),
            Error::<Test>::NotGuardian
        );
        assert_ok!(BridgeHalt::remove_guardian(RuntimeOrigin::root(), GUARDIAN));
        System::assert_last_event(Event::GuardianRemoved { account: GUARDIAN }.into());
        // A removed guardian loses the power at once.
        assert_eq!(
            BridgeHalt::validate_halt(&GUARDIAN),
            Err(InvalidTransaction::BadSigner.into())
        );
        assert_noop!(
            BridgeHalt::halt(RuntimeOrigin::signed(GUARDIAN), reason("x")),
            Error::<Test>::NotGuardian
        );
    });
}

#[test]
fn an_overlong_message_does_not_decode() {
    let encoded = |length: usize| {
        let mut bytes = Call::<Test>::halt { reason: reason("") }.encode();
        // Replace the empty message with one of the given length.
        bytes.truncate(1);
        bytes.extend(vec![b'x'; length].encode());
        bytes
    };
    assert!(Call::<Test>::decode(&mut &encoded(64)[..]).is_ok());
    assert!(Call::<Test>::decode(&mut &encoded(65)[..]).is_err());
}
