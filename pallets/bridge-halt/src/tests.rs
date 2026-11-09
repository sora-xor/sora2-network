// This file is part of the SORA network and Polkaswap app.
// SPDX-License-Identifier: BSD-4-Clause

use crate::{mock::*, Call, Error, Event, Guardians, HaltRecord, LastHalt};
use codec::{Decode, Encode};
use common::bridge_halt::{latest_guardian_halt, GuardianHalt};
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

/// What bridge workers read.
fn signal() -> Option<GuardianHalt> {
    latest_guardian_halt().unwrap()
}

fn with_guardian(test: impl FnOnce()) {
    new_test_ext().execute_with(|| {
        assert_ok!(BridgeHalt::add_guardian(RuntimeOrigin::root(), GUARDIAN));
        System::reset_events();
        test()
    })
}

#[test]
fn a_guardian_halt_is_numbered_and_tells_the_workers_to_stop() {
    with_guardian(|| {
        System::set_block_number(7);
        assert_eq!(BridgeHalt::validate_halt(&GUARDIAN), Ok(()));
        assert_ok!(BridgeHalt::halt(
            RuntimeOrigin::signed(GUARDIAN),
            reason("relayer key leaked")
        ));
        assert_eq!(
            signal(),
            Some(GuardianHalt {
                number: 1,
                block: 7
            })
        );
        assert_eq!(
            LastHalt::<Test>::get(),
            Some(HaltRecord {
                number: 1,
                guardian: GUARDIAN,
                block: 7,
                reason: reason("relayer key leaked"),
            })
        );
        System::assert_last_event(
            Event::BridgeHalted {
                number: 1,
                guardian: GUARDIAN,
                reason: reason("relayer key leaked"),
            }
            .into(),
        );
        // Root's recovery latch is not involved, so no Root call is needed afterwards.
        assert_eq!(latch(), None);
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
        assert_eq!(signal().map(|halt| halt.number), Some(1));
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
        assert_eq!(signal(), None);
        assert_eq!(LastHalt::<Test>::get(), None);
        assert!(System::events().is_empty());
        assert_eq!(sp_io::storage::root(StateVersion::V1), before);
    });
}

#[test]
fn the_next_halt_waits_out_the_cooldown_and_gets_the_next_number() {
    with_guardian(|| {
        assert_ok!(BridgeHalt::add_guardian(RuntimeOrigin::root(), 3));
        System::set_block_number(7);
        assert_ok!(BridgeHalt::halt(
            RuntimeOrigin::signed(GUARDIAN),
            reason("first")
        ));
        // Any guardian is refused until the cooldown has passed.
        for (block, guardian) in [(7, GUARDIAN), (8, 3), (7 + COOLDOWN - 1, GUARDIAN)] {
            System::set_block_number(block);
            assert!(BridgeHalt::halted_recently());
            assert_eq!(
                BridgeHalt::validate_halt(&guardian),
                Err(InvalidTransaction::Stale.into())
            );
            assert_noop!(
                BridgeHalt::halt(RuntimeOrigin::signed(guardian), reason("again")),
                Error::<Test>::HaltedRecently
            );
        }
        assert_eq!(LastHalt::<Test>::get().unwrap().reason, reason("first"));

        System::set_block_number(7 + COOLDOWN);
        assert!(!BridgeHalt::halted_recently());
        assert_eq!(BridgeHalt::validate_halt(&3), Ok(()));
        assert_ok!(BridgeHalt::halt(RuntimeOrigin::signed(3), reason("second")));
        assert_eq!(
            signal(),
            Some(GuardianHalt {
                number: 2,
                block: 7 + COOLDOWN as u32
            })
        );
        let last = LastHalt::<Test>::get().unwrap();
        assert_eq!((last.number, last.guardian), (2, 3));
    });
}

#[test]
fn root_latch_neither_comes_from_a_halt_nor_blocks_one() {
    // The recovery sets [1]; bridge pallets treat any value, even empty, as halted.
    for value in [vec![1u8], vec![0], vec![]] {
        with_guardian(|| {
            sp_io::storage::set(BRIDGE_HALT_STORAGE_KEY, &value);
            assert_eq!(BridgeHalt::validate_halt(&GUARDIAN), Ok(()));
            assert_ok!(BridgeHalt::halt(
                RuntimeOrigin::signed(GUARDIAN),
                reason("x")
            ));
            assert_eq!(signal().map(|halt| halt.number), Some(1));
            assert_eq!(latch(), Some(value.clone()));
        });
    }
}

#[test]
fn the_number_saturates_but_the_block_still_tells_halts_apart() {
    with_guardian(|| {
        LastHalt::<Test>::put(HaltRecord {
            number: u32::MAX,
            guardian: GUARDIAN,
            block: 1,
            reason: reason(""),
        });
        System::set_block_number(1 + COOLDOWN);
        assert_ok!(BridgeHalt::halt(
            RuntimeOrigin::signed(GUARDIAN),
            reason("x")
        ));
        assert_eq!(
            signal(),
            Some(GuardianHalt {
                number: u32::MAX,
                block: 1 + COOLDOWN as u32
            })
        );
    });
}

#[test]
fn the_retired_resume_index_does_not_decode() {
    // Index 1 was `resume`. Nothing on chain restarts bridge workers any more.
    assert!(Call::<Test>::decode(&mut &[1u8][..]).is_err());
    assert_eq!(
        Call::<Test>::add_guardian { account: GUARDIAN }.encode()[0],
        2
    );
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
