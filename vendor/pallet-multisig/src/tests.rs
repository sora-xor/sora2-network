// This file is part of Substrate.

// Copyright (C) 2019-2020 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// Tests for Multisig Pallet

#![cfg(test)]

use super::*;

use crate as multisig;
use frame_support::{
    assert_err, assert_noop, assert_ok, construct_runtime, parameter_types, weights::Weight,
};
use sp_core::H256;
use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup},
    BuildStorage, Perbill,
};

// For testing the pallet, we construct most of a mock runtime. This means
// first constructing a configuration type (`Test`) which `impl`s each of the
// configuration traits of pallets we want to use.
parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = Weight::set_ref_time(Weight::zero(), 1024);
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::one();
}
type Block = frame_system::mocking::MockBlock<Test>;

impl frame_system::Config for Test {
    type BaseCallFilter = TestBaseCallFilter;
    type BlockWeights = ();
    type BlockLength = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = u64;
    type Lookup = IdentityLookup<Self::AccountId>;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = BlockHashCount;
    type DbWeight = ();
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<u64>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ();
    type OnSetCode = ();
    type MaxConsumers = frame_support::traits::ConstU32<16>;
    type Nonce = u64;
    type Block = Block;
    type RuntimeTask = ();
    type ExtensionsWeightInfo = ();
    type SingleBlockMigrations = ();
    type MultiBlockMigrator = ();
    type PreInherents = ();
    type PostInherents = ();
    type PostTransactions = ();
}

parameter_types! {
    pub const ExistentialDeposit: u64 = 1;
}

impl pallet_balances::Config for Test {
    type MaxReserves = ();
    type ReserveIdentifier = ();
    type Balance = u64;
    type RuntimeEvent = RuntimeEvent;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
    type MaxLocks = ();
    type RuntimeHoldReason = ();
    type FreezeIdentifier = ();
    type MaxFreezes = ();
    type RuntimeFreezeReason = ();
    type DoneSlashHandler = ();
}
parameter_types! {
    pub const DepositBase: u64 = 1;
    pub const DepositFactor: u64 = 1;
    pub const MaxSignatories: u16 = 4;
}
pub struct TestBaseCallFilter;

impl Contains<RuntimeCall> for TestBaseCallFilter {
    fn contains(c: &RuntimeCall) -> bool {
        match *c {
            RuntimeCall::Balances(_) => true,
            RuntimeCall::Multisig(_) => true,
            // Needed for benchmarking
            // RuntimeCall::System((_, frame_system::Call::remark(_))) => true,
            _ => false,
        }
    }
}

impl Config for Test {
    type CallFilter = TestMultisigCallFilter;
    type OperationFilter = ();

    type MaxPendingOperations = frame_support::traits::ConstU32<128>;
    type MaxCallBytes = frame_support::traits::ConstU32<16384>;

    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type Currency = Balances;
    type DepositBase = DepositBase;
    type DepositFactor = DepositFactor;
    type MaxSignatories = MaxSignatories;
    type WeightInfo = ();
}

construct_runtime!(
    pub enum Test {
        System: frame_system::{Pallet, Call, Config<T>, Storage, Event<T>},
        Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
        Multisig: multisig::{Pallet, Call, Storage, Config<T>, Event<T>},
    }
);

use crate::Call as MultisigCall;
use frame_support::dispatch::DispatchErrorWithPostInfo;
use frame_support::dispatch::Pays;
use frame_support::traits::{Contains, ExistenceRequirement};
use pallet_balances::Call as BalancesCall;

pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    pallet_balances::GenesisConfig::<Test> {
        balances: vec![(1, 10), (2, 10), (3, 10), (4, 10), (5, 2)],
        dev_accounts: None,
    }
    .assimilate_storage(&mut t)
    .unwrap();
    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}

#[allow(unused)]
fn last_event() -> RuntimeEvent {
    system::Pallet::<Test>::events()
        .pop()
        .map(|e| e.event)
        .expect("Event expected")
}

#[allow(unused)]
fn expect_event<E: Into<RuntimeEvent>>(e: E) {
    assert_eq!(last_event(), e.into());
}

fn now() -> BridgeTimepoint<u64> {
    Multisig::thischain_timepoint()
}

#[test]
fn registration_and_membership_bounds_reject_without_mutating_accounts() {
    new_test_ext().execute_with(|| {
        for members in [vec![2, 1], vec![1, 1]] {
            assert_noop!(
                Multisig::register_multisig_inner(1, members),
                Error::<Test>::SignatoriesAreNotUniqueOrUnordered
            );
        }
        let id = Multisig::register_multisig_inner(1, vec![1, 2, 3, 4]).unwrap();
        assert_noop!(
            Multisig::register_multisig_inner(1, vec![1, 2]),
            Error::<Test>::MultisigAlreadyExists
        );
        assert_noop!(
            Multisig::add_signatory(RuntimeOrigin::signed(id), 5),
            Error::<Test>::TooManySignatories
        );
        assert_noop!(
            Multisig::remove_signatory(RuntimeOrigin::signed(id), 5),
            Error::<Test>::NotInSignatories
        );
        assert_eq!(
            Multisig::accounts(id).unwrap().signatories,
            vec![1, 2, 3, 4]
        );
        let single = Multisig::register_multisig_inner(2, vec![2]).unwrap();
        assert_noop!(
            Multisig::remove_signatory(RuntimeOrigin::signed(single), 2),
            Error::<Test>::TooFewSignatories
        );
        assert_eq!(Multisig::accounts(single).unwrap().signatories, vec![2]);
    });
}

#[test]
fn nonmembers_and_wrong_threshold_cannot_admit_or_cancel_dispatched_calls() {
    new_test_ext().execute_with(|| {
        let id = Multisig::register_multisig_inner(1, vec![1, 2]).unwrap();
        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 1 });
        let hash = blake2_256(&call.encode());
        let weight = call.get_dispatch_info().total_weight();
        assert_noop!(
            Multisig::as_multi_threshold_1(
                RuntimeOrigin::signed(1),
                id,
                Box::new(call.clone()),
                now()
            ),
            Error::<Test>::TooManySignatories
        );
        assert_noop!(
            Multisig::as_multi(
                RuntimeOrigin::signed(3),
                id,
                None,
                call.encode(),
                false,
                weight
            ),
            Error::<Test>::SenderNotInSignatories
        );
        assert_noop!(
            Multisig::approve_as_multi(RuntimeOrigin::signed(3), id, None, hash, weight),
            Error::<Test>::SenderNotInSignatories
        );
        DispatchedCalls::<Test>::insert(hash, now(), ());
        assert_noop!(
            Multisig::cancel_as_multi(RuntimeOrigin::signed(1), id, now(), hash),
            Error::<Test>::AlreadyDispatched
        );
        assert!(!Multisigs::<Test>::contains_key(id, hash));
        assert!(!Calls::<Test>::contains_key(hash));
    });
}

#[test]
fn cancellation_budget_bounds_validation_and_admission_declares_inner_call_weight() {
    new_test_ext().execute_with(|| {
        let validation = Multisig::protocol_validation_weight();
        let cancellation = Multisig::cancellation_weight();
        assert!(cancellation.all_gte(validation.saturating_mul(3)));
        assert!(cancellation.ref_time() > 0);
        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 1 });
        let inner_weight = call.get_dispatch_info().total_weight();
        // Weight must remain conservative even for an unknown account that will
        // fail admission, so invalid transactions cannot bypass metering.
        let admission = RuntimeCall::Multisig(MultisigCall::as_multi {
            id: 999,
            maybe_timepoint: None,
            call: call.encode(),
            store_call: false,
            max_weight: inner_weight,
        })
        .get_dispatch_info()
        .total_weight();
        assert!(admission.all_gte(inner_weight.saturating_add(validation)));
    });
}

#[test]
fn multisig_deposit_is_taken_and_returned() {
    new_test_ext().execute_with(|| {
        let multi = Multisig::multi_account_id(&1, 1, 0);
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(1),
            vec![1, 2, 3],
        ));
        assert_ok!(Balances::transfer(
            &1,
            &multi,
            5,
            ExistenceRequirement::AllowDeath
        ));
        assert_ok!(Balances::transfer(
            &2,
            &multi,
            5,
            ExistenceRequirement::AllowDeath
        ));
        assert_ok!(Balances::transfer(
            &3,
            &multi,
            5,
            ExistenceRequirement::AllowDeath
        ));

        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 15 });
        let call_weight = call.get_dispatch_info().call_weight;
        let data = call.encode();
        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(1),
            multi,
            None,
            data.clone(),
            false,
            Weight::zero()
        ));
        assert_eq!(Balances::free_balance(1), 5);
        assert_eq!(Balances::reserved_balance(1), 0);

        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(2),
            multi,
            Some(now()),
            data,
            false,
            call_weight
        ));
        assert_eq!(Balances::free_balance(1), 5);
        assert_eq!(Balances::reserved_balance(1), 0);
    });
}

#[test]
fn multisig_deposit_is_taken_and_returned_with_call_storage() {
    new_test_ext().execute_with(|| {
        let multi = Multisig::multi_account_id(&1, 1, 0);
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(1),
            vec![1, 2, 3],
        ));
        assert_ok!(Balances::transfer(
            &1,
            &multi,
            5,
            ExistenceRequirement::AllowDeath
        ));
        assert_ok!(Balances::transfer(
            &2,
            &multi,
            5,
            ExistenceRequirement::AllowDeath
        ));
        assert_ok!(Balances::transfer(
            &3,
            &multi,
            5,
            ExistenceRequirement::AllowDeath
        ));

        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 15 });
        let call_weight = call.get_dispatch_info().call_weight;
        let data = call.encode();
        let hash = blake2_256(&data);
        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(1),
            multi,
            None,
            data,
            true,
            Weight::zero()
        ));
        assert_eq!(Balances::free_balance(1), 5);
        assert_eq!(Balances::reserved_balance(1), 0);

        assert_ok!(Multisig::approve_as_multi(
            RuntimeOrigin::signed(2),
            multi,
            Some(now()),
            hash,
            call_weight
        ));
        assert_eq!(Balances::free_balance(1), 5);
        assert_eq!(Balances::reserved_balance(1), 0);
    });
}

#[test]
fn multisig_deposit_is_taken_and_returned_with_alt_call_storage() {
    new_test_ext().execute_with(|| {
        let multi = Multisig::multi_account_id(&1, 1, 0);
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(1),
            vec![1, 2, 3],
        ));
        assert_ok!(Balances::transfer(
            &1,
            &multi,
            5,
            ExistenceRequirement::AllowDeath
        ));
        assert_ok!(Balances::transfer(
            &2,
            &multi,
            5,
            ExistenceRequirement::AllowDeath
        ));
        assert_ok!(Balances::transfer(
            &3,
            &multi,
            5,
            ExistenceRequirement::AllowDeath
        ));

        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 15 });

        let call_weight = call.get_dispatch_info().call_weight;
        let data = call.encode();
        let hash = blake2_256(&data);

        assert_ok!(Multisig::approve_as_multi(
            RuntimeOrigin::signed(1),
            multi,
            None,
            hash.clone(),
            Weight::zero()
        ));
        assert_eq!(Balances::free_balance(1), 5);
        assert_eq!(Balances::reserved_balance(1), 0);

        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(2),
            multi,
            Some(now()),
            data,
            true,
            Weight::zero()
        ));
        assert_eq!(Balances::free_balance(2), 5);
        assert_eq!(Balances::reserved_balance(2), 0);
        assert_eq!(Balances::free_balance(1), 5);
        assert_eq!(Balances::reserved_balance(1), 0);

        assert_ok!(Multisig::approve_as_multi(
            RuntimeOrigin::signed(3),
            multi,
            Some(now()),
            hash,
            call_weight
        ));
        assert_eq!(Balances::free_balance(1), 5);
        assert_eq!(Balances::reserved_balance(1), 0);
        assert_eq!(Balances::free_balance(2), 5);
        assert_eq!(Balances::reserved_balance(2), 0);
    });
}

#[test]
fn cancel_multisig_returns_deposit() {
    new_test_ext().execute_with(|| {
        let multi = Multisig::multi_account_id(&1, 1, 0);
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(1),
            vec![1, 2, 3],
        ));

        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 15 })
            .encode();
        let hash = blake2_256(&call);
        assert_ok!(Multisig::approve_as_multi(
            RuntimeOrigin::signed(1),
            multi,
            None,
            hash.clone(),
            Weight::zero()
        ));
        assert_ok!(Multisig::approve_as_multi(
            RuntimeOrigin::signed(2),
            multi,
            Some(now()),
            hash.clone(),
            Weight::zero()
        ));
        assert_eq!(Balances::free_balance(1), 10);
        assert_eq!(Balances::reserved_balance(1), 0);
        assert_ok!(Multisig::cancel_as_multi(
            RuntimeOrigin::signed(1),
            multi,
            now(),
            hash.clone()
        ),);
        assert_eq!(Balances::free_balance(1), 10);
        assert_eq!(Balances::reserved_balance(1), 0);
    });
}

#[test]
fn already_dispatched_checking_works() {
    new_test_ext().execute_with(|| {
        let multi = Multisig::multi_account_id(&1, 1, 0);
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(1),
            vec![1, 2, 3, 4],
        ));
        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 15 });
        let call_weight = call.get_dispatch_info().call_weight;
        let call_encoded = call.encode();
        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(1),
            multi,
            Some(now()),
            call_encoded.clone(),
            false,
            call_weight
        ));
        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(2),
            multi,
            Some(now()),
            call_encoded.clone(),
            false,
            call_weight
        ));
        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(3),
            multi,
            Some(now()),
            call_encoded.clone(),
            false,
            call_weight
        ));

        assert_noop!(
            Multisig::as_multi(
                RuntimeOrigin::signed(4),
                multi,
                Some(now()),
                call_encoded.clone(),
                false,
                call_weight
            ),
            DispatchErrorWithPostInfo {
                error: Error::<Test>::AlreadyDispatched.into(),
                post_info: Pays::Yes.into()
            },
        );
    });
}

#[test]
fn already_dispatched_checking_works_for_threshold_1() {
    new_test_ext().execute_with(|| {
        let multi = Multisig::multi_account_id(&1, 1, 0);
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(1),
            vec![1],
        ));
        assert_ok!(Balances::transfer(
            &1,
            &multi,
            5,
            ExistenceRequirement::AllowDeath
        ));

        let boxed_call = Box::new(RuntimeCall::Balances(BalancesCall::transfer_allow_death {
            dest: 6,
            value: 5,
        }));
        let timepoint = now();
        assert_ok!(Multisig::as_multi_threshold_1(
            RuntimeOrigin::signed(1),
            multi,
            boxed_call.clone(),
            timepoint.clone()
        ));
        assert_noop!(
            Multisig::as_multi_threshold_1(
                RuntimeOrigin::signed(1),
                multi,
                boxed_call,
                timepoint.clone()
            ),
            DispatchErrorWithPostInfo {
                error: Error::<Test>::AlreadyDispatched.into(),
                post_info: Pays::Yes.into()
            }
        );
    });
}

#[test]
fn timepoint_checking_works() {
    new_test_ext().execute_with(|| {
        let multi = Multisig::multi_account_id(&1, 1, 0);
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(1),
            vec![1, 2, 3],
        ));
        assert_ok!(Balances::transfer(
            &1,
            &multi,
            5,
            ExistenceRequirement::AllowDeath
        ));
        assert_ok!(Balances::transfer(
            &2,
            &multi,
            5,
            ExistenceRequirement::AllowDeath
        ));
        assert_ok!(Balances::transfer(
            &3,
            &multi,
            5,
            ExistenceRequirement::AllowDeath
        ));

        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 7 })
            .encode();
        let hash = blake2_256(&call);

        assert_ok!(Multisig::approve_as_multi(
            RuntimeOrigin::signed(2),
            multi,
            Some(now()),
            hash.clone(),
            Weight::zero()
        ));

        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 8 })
            .encode();
        let hash = blake2_256(&call);

        assert_ok!(Multisig::approve_as_multi(
            RuntimeOrigin::signed(1),
            multi,
            None,
            hash,
            Weight::zero()
        ));
        assert_noop!(
            Multisig::as_multi(
                RuntimeOrigin::signed(2),
                multi,
                None,
                call.clone(),
                false,
                Weight::zero()
            ),
            DispatchErrorWithPostInfo {
                error: Error::<Test>::NoTimepoint.into(),
                post_info: Pays::Yes.into()
            }
        );
        let later = BridgeTimepoint { index: 1, ..now() };
        assert_noop!(
            Multisig::as_multi(
                RuntimeOrigin::signed(2),
                multi,
                Some(later),
                call.clone(),
                false,
                Weight::zero()
            ),
            DispatchErrorWithPostInfo {
                error: Error::<Test>::WrongTimepoint.into(),
                post_info: Pays::Yes.into()
            }
        );
    });
}

#[test]
fn multisig_2_of_2_works_with_call_storing() {
    new_test_ext().execute_with(|| {
        let multi = Multisig::multi_account_id(&1, 1, 0);
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(1),
            vec![1, 2],
        ));
        assert_ok!(Balances::transfer(
            &1,
            &multi,
            5,
            ExistenceRequirement::AllowDeath
        ));
        assert_ok!(Balances::transfer(
            &2,
            &multi,
            5,
            ExistenceRequirement::AllowDeath
        ));

        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 10 });
        let call_weight = call.get_dispatch_info().call_weight;
        let data = call.encode();
        let hash = blake2_256(&data);
        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(1),
            multi,
            None,
            data,
            true,
            Weight::zero()
        ));
        assert_eq!(Balances::free_balance(6), 0);

        assert_ok!(Multisig::approve_as_multi(
            RuntimeOrigin::signed(2),
            multi,
            Some(now()),
            hash,
            call_weight
        ));
        assert_eq!(Balances::free_balance(6), 10);
    });
}

#[test]
fn multisig_2_of_2_works() {
    new_test_ext().execute_with(|| {
        let multi = Multisig::multi_account_id(&1, 1, 0);
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(1),
            vec![1, 2],
        ));
        assert_ok!(Balances::transfer(
            &1,
            &multi,
            5,
            ExistenceRequirement::AllowDeath
        ));
        assert_ok!(Balances::transfer(
            &2,
            &multi,
            5,
            ExistenceRequirement::AllowDeath
        ));
        assert_ok!(Balances::transfer(
            &3,
            &multi,
            5,
            ExistenceRequirement::AllowDeath
        ));

        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 15 });
        let call_weight = call.get_dispatch_info().call_weight;
        let data = call.encode();
        let hash = blake2_256(&data);
        assert_ok!(Multisig::approve_as_multi(
            RuntimeOrigin::signed(1),
            multi,
            None,
            hash,
            Weight::zero()
        ));
        assert_eq!(Balances::free_balance(6), 0);

        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(2),
            multi,
            Some(now()),
            data,
            false,
            call_weight
        ));
        assert_eq!(Balances::free_balance(6), 15);
    });
}

#[test]
fn multisig_3_of_3_works() {
    new_test_ext().execute_with(|| {
        let multi = Multisig::multi_account_id(&1, 1, 0);
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(1),
            vec![1, 2, 3],
        ));
        assert_ok!(Balances::transfer(
            &1,
            &multi,
            5,
            ExistenceRequirement::AllowDeath
        ));
        assert_ok!(Balances::transfer(
            &2,
            &multi,
            5,
            ExistenceRequirement::AllowDeath
        ));
        assert_ok!(Balances::transfer(
            &3,
            &multi,
            5,
            ExistenceRequirement::AllowDeath
        ));

        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 15 });
        let call_weight = call.get_dispatch_info().call_weight;
        let data = call.encode();
        let hash = blake2_256(&data);
        assert_ok!(Multisig::approve_as_multi(
            RuntimeOrigin::signed(1),
            multi,
            None,
            hash.clone(),
            Weight::zero()
        ));
        assert_ok!(Multisig::approve_as_multi(
            RuntimeOrigin::signed(2),
            multi,
            Some(now()),
            hash.clone(),
            Weight::zero()
        ));
        assert_eq!(Balances::free_balance(6), 0);

        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(3),
            multi,
            Some(now()),
            data,
            false,
            call_weight
        ));
        assert_eq!(Balances::free_balance(6), 15);
    });
}

#[test]
fn multisig_only_multisig_can_add_or_remove_signatory() {
    new_test_ext().execute_with(|| {
        let multi = Multisig::multi_account_id(&1, 1, 0);
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(1),
            vec![1, 2, 3],
        ));

        assert_ok!(Multisig::add_signatory(RuntimeOrigin::signed(multi), 4));
        assert_ok!(Multisig::remove_signatory(RuntimeOrigin::signed(multi), 4));
        assert_err!(
            Multisig::add_signatory(RuntimeOrigin::signed(1), 2),
            Error::<Test>::UnknownMultisigAccount
        );
        assert_err!(
            Multisig::remove_signatory(RuntimeOrigin::signed(1), 2),
            Error::<Test>::UnknownMultisigAccount
        );
    });
}

#[test]
fn multisig_removed_signatory_votes_are_pruned_on_next_approval() {
    new_test_ext().execute_with(|| {
        let multi = Multisig::multi_account_id(&1, 1, 0);
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(1),
            vec![1, 2, 3, 4],
        ));

        let call = RuntimeCall::Multisig(MultisigCall::add_signatory { new_member: 5 });
        let data = call.encode();
        let hash = blake2_256(&data);
        assert_ok!(Multisig::approve_as_multi(
            RuntimeOrigin::signed(4),
            multi,
            None,
            hash.clone(),
            Weight::zero()
        ));

        let operation = <Multisigs<Test>>::get(&multi, &hash).unwrap();
        assert_eq!(operation.approvals.len(), 1);

        assert_ok!(Multisig::remove_signatory(RuntimeOrigin::signed(multi), 4));

        let operation = <Multisigs<Test>>::get(&multi, &hash).unwrap();
        assert_eq!(operation.approvals, vec![4]);
        assert_ok!(Multisig::approve_as_multi(
            RuntimeOrigin::signed(1),
            multi,
            Some(operation.when),
            hash,
            Weight::zero()
        ));
        assert_eq!(
            Multisigs::<Test>::get(&multi, &hash).unwrap().approvals,
            vec![1]
        );
    });
}

#[test]
fn multisig_3_of_3_works_with_new_signatory() {
    new_test_ext().execute_with(|| {
        let multi = Multisig::multi_account_id(&1, 1, 0);
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(1),
            vec![1, 2, 3],
        ));

        let call = RuntimeCall::Multisig(MultisigCall::add_signatory { new_member: 4 });
        let call_weight = call.get_dispatch_info().call_weight;
        let data = call.encode();
        let hash = blake2_256(&data);
        assert_ok!(Multisig::approve_as_multi(
            RuntimeOrigin::signed(1),
            multi,
            None,
            hash.clone(),
            Weight::zero()
        ));
        assert_ok!(Multisig::approve_as_multi(
            RuntimeOrigin::signed(2),
            multi,
            Some(now()),
            hash.clone(),
            Weight::zero()
        ));
        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(3),
            multi,
            Some(now()),
            data,
            false,
            call_weight
        ));

        assert_ok!(Balances::transfer(
            &2,
            &multi,
            5,
            ExistenceRequirement::AllowDeath
        ));
        assert_ok!(Balances::transfer(
            &3,
            &multi,
            5,
            ExistenceRequirement::AllowDeath
        ));
        assert_ok!(Balances::transfer(
            &4,
            &multi,
            5,
            ExistenceRequirement::AllowDeath
        ));

        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 15 });
        let call_weight = call.get_dispatch_info().call_weight;
        let data = call.encode();
        let hash = blake2_256(&data);
        assert_ok!(Multisig::approve_as_multi(
            RuntimeOrigin::signed(2),
            multi,
            None,
            hash.clone(),
            Weight::zero()
        ));
        assert_ok!(Multisig::approve_as_multi(
            RuntimeOrigin::signed(3),
            multi,
            Some(now()),
            hash.clone(),
            Weight::zero()
        ));
        assert_eq!(Balances::free_balance(6), 0);

        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(4),
            multi,
            Some(now()),
            data,
            false,
            call_weight
        ));
        assert_eq!(Balances::free_balance(6), 15);
    });
}

#[test]
fn multisig_3_of_4_works_after_removing_signatory() {
    new_test_ext().execute_with(|| {
        let multi = Multisig::multi_account_id(&1, 1, 0);
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(1),
            vec![1, 2, 3, 4],
        ));

        let call = RuntimeCall::Multisig(MultisigCall::add_signatory { new_member: 4 });
        let call_weight = call.get_dispatch_info().call_weight;
        let data = call.encode();
        let hash = blake2_256(&data);
        assert_ok!(Multisig::approve_as_multi(
            RuntimeOrigin::signed(1),
            multi,
            None,
            hash.clone(),
            Weight::zero()
        ));
        assert_ok!(Multisig::approve_as_multi(
            RuntimeOrigin::signed(2),
            multi,
            Some(now()),
            hash.clone(),
            Weight::zero()
        ));
        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(3),
            multi,
            Some(now()),
            data,
            false,
            call_weight
        ));

        assert_ok!(Balances::transfer(
            &2,
            &multi,
            5,
            ExistenceRequirement::AllowDeath
        ));
        assert_ok!(Balances::transfer(
            &3,
            &multi,
            5,
            ExistenceRequirement::AllowDeath
        ));
        assert_ok!(Balances::transfer(
            &4,
            &multi,
            5,
            ExistenceRequirement::AllowDeath
        ));

        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 15 });
        let call_weight = call.get_dispatch_info().call_weight;
        let data = call.encode();
        let hash = blake2_256(&data);
        assert_ok!(Multisig::approve_as_multi(
            RuntimeOrigin::signed(2),
            multi,
            None,
            hash.clone(),
            Weight::zero()
        ));
        assert_ok!(Multisig::approve_as_multi(
            RuntimeOrigin::signed(3),
            multi,
            Some(now()),
            hash.clone(),
            Weight::zero()
        ));
        assert_eq!(Balances::free_balance(6), 0);

        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(4),
            multi,
            Some(now()),
            data,
            false,
            call_weight
        ));
        assert_eq!(Balances::free_balance(6), 15);
    });
}

#[test]
fn cancel_multisig_works() {
    new_test_ext().execute_with(|| {
        let multi = Multisig::multi_account_id(&1, 1, 0);
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(1),
            vec![1, 2, 3],
        ));

        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 15 })
            .encode();
        let hash = blake2_256(&call);
        assert_ok!(Multisig::approve_as_multi(
            RuntimeOrigin::signed(1),
            multi,
            None,
            hash.clone(),
            Weight::zero()
        ));
        assert_ok!(Multisig::approve_as_multi(
            RuntimeOrigin::signed(2),
            multi,
            Some(now()),
            hash.clone(),
            Weight::zero()
        ));
        assert_noop!(
            Multisig::cancel_as_multi(RuntimeOrigin::signed(2), multi, now(), hash.clone()),
            Error::<Test>::NotOwner,
        );
        assert_ok!(Multisig::cancel_as_multi(
            RuntimeOrigin::signed(1),
            multi,
            now(),
            hash.clone()
        ),);
    });
}

#[test]
fn cancel_multisig_with_call_storage_works() {
    new_test_ext().execute_with(|| {
        let multi = Multisig::multi_account_id(&1, 1, 0);
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(1),
            vec![1, 2, 3],
        ));

        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 15 })
            .encode();
        let hash = blake2_256(&call);
        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(1),
            multi,
            None,
            call,
            true,
            Weight::zero()
        ));
        assert_eq!(Balances::free_balance(1), 10);
        assert_ok!(Multisig::approve_as_multi(
            RuntimeOrigin::signed(2),
            multi,
            Some(now()),
            hash.clone(),
            Weight::zero()
        ));
        assert_noop!(
            Multisig::cancel_as_multi(RuntimeOrigin::signed(2), multi, now(), hash.clone()),
            Error::<Test>::NotOwner,
        );
        assert_ok!(Multisig::cancel_as_multi(
            RuntimeOrigin::signed(1),
            multi,
            now(),
            hash.clone()
        ),);
        assert_eq!(Balances::free_balance(1), 10);
    });
}

#[test]
fn cancel_multisig_with_alt_call_storage_works() {
    new_test_ext().execute_with(|| {
        let multi = Multisig::multi_account_id(&1, 1, 0);
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(1),
            vec![1, 2, 3],
        ));

        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 15 })
            .encode();
        let hash = blake2_256(&call);
        assert_ok!(Multisig::approve_as_multi(
            RuntimeOrigin::signed(1),
            multi,
            None,
            hash.clone(),
            Weight::zero()
        ));
        assert_eq!(Balances::free_balance(1), 10);
        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(2),
            multi,
            Some(now()),
            call,
            true,
            Weight::zero()
        ));
        assert_eq!(Balances::free_balance(2), 10);
        assert_ok!(Multisig::cancel_as_multi(
            RuntimeOrigin::signed(1),
            multi,
            now(),
            hash
        ));
        assert_eq!(Balances::free_balance(1), 10);
        assert_eq!(Balances::free_balance(2), 10);
    });
}

#[test]
fn multisig_2_of_2_as_multi_works() {
    new_test_ext().execute_with(|| {
        let multi = Multisig::multi_account_id(&1, 1, 0);
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(1),
            vec![1, 2],
        ));

        assert_ok!(Balances::transfer(
            &1,
            &multi,
            5,
            ExistenceRequirement::AllowDeath
        ));
        assert_ok!(Balances::transfer(
            &2,
            &multi,
            5,
            ExistenceRequirement::AllowDeath
        ));
        assert_ok!(Balances::transfer(
            &3,
            &multi,
            5,
            ExistenceRequirement::AllowDeath
        ));

        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 15 });
        let call_weight = call.get_dispatch_info().call_weight;
        let data = call.encode();
        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(1),
            multi,
            None,
            data.clone(),
            false,
            Weight::zero()
        ));
        assert_eq!(Balances::free_balance(6), 0);

        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(2),
            multi,
            Some(now()),
            data,
            false,
            call_weight
        ));
        assert_eq!(Balances::free_balance(6), 15);
    });
}

#[test]
fn multisig_2_of_2_as_multi_with_many_calls_works() {
    new_test_ext().execute_with(|| {
        let multi = Multisig::multi_account_id(&1, 1, 0);
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(1),
            vec![1, 2],
        ));

        assert_ok!(Balances::transfer(
            &1,
            &multi,
            5,
            ExistenceRequirement::AllowDeath
        ));
        assert_ok!(Balances::transfer(
            &2,
            &multi,
            5,
            ExistenceRequirement::AllowDeath
        ));
        assert_ok!(Balances::transfer(
            &3,
            &multi,
            5,
            ExistenceRequirement::AllowDeath
        ));

        let call1 =
            RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 10 });
        let call1_weight = call1.get_dispatch_info().call_weight;
        let data1 = call1.encode();
        let call2 = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 7, value: 5 });
        let call2_weight = call2.get_dispatch_info().call_weight;
        let data2 = call2.encode();

        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(1),
            multi,
            None,
            data1.clone(),
            false,
            Weight::zero()
        ));
        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(2),
            multi,
            None,
            data2.clone(),
            false,
            Weight::zero()
        ));
        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(2),
            multi,
            Some(now()),
            data1,
            false,
            call1_weight
        ));
        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(1),
            multi,
            Some(now()),
            data2,
            false,
            call2_weight
        ));

        assert_eq!(Balances::free_balance(6), 10);
        assert_eq!(Balances::free_balance(7), 5);
    });
}

#[test]
fn multisig_3_of_4_cannot_reissue_same_call() {
    new_test_ext().execute_with(|| {
        let multi = Multisig::multi_account_id(&1, 1, 0);
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(1),
            vec![1, 2, 3, 4],
        ));

        assert_ok!(Balances::transfer(
            &1,
            &multi,
            5,
            ExistenceRequirement::AllowDeath
        ));
        assert_ok!(Balances::transfer(
            &2,
            &multi,
            5,
            ExistenceRequirement::AllowDeath
        ));
        assert_ok!(Balances::transfer(
            &3,
            &multi,
            5,
            ExistenceRequirement::AllowDeath
        ));

        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 10 });
        let call_weight = call.get_dispatch_info().call_weight;
        let data = call.encode();
        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(1),
            multi,
            None,
            data.clone(),
            false,
            Weight::zero()
        ));
        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(2),
            multi,
            Some(now()),
            data.clone(),
            false,
            call_weight
        ));
        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(3),
            multi,
            Some(now()),
            data.clone(),
            false,
            call_weight
        ));
        assert_eq!(Balances::free_balance(multi), 5);

        assert_noop!(
            Multisig::as_multi(
                RuntimeOrigin::signed(4),
                multi,
                None,
                data.clone(),
                false,
                Weight::zero()
            ),
            DispatchErrorWithPostInfo {
                error: Error::<Test>::AlreadyDispatched.into(),
                post_info: Pays::Yes.into()
            }
        );
    });
}

#[test]
fn too_many_signatories_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Multisig::register_multisig(RuntimeOrigin::signed(1), vec![1, 2, 3, 4, 5],),
            Error::<Test>::TooManySignatories
        );
    });
}

#[test]
fn duplicate_approvals_are_ignored() {
    new_test_ext().execute_with(|| {
        let multi = Multisig::multi_account_id(&1, 1, 0);
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(1),
            vec![1, 2, 3],
        ));

        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 15 })
            .encode();
        let hash = blake2_256(&call);
        assert_ok!(Multisig::approve_as_multi(
            RuntimeOrigin::signed(1),
            multi,
            None,
            hash.clone(),
            Weight::zero()
        ));
        assert_noop!(
            Multisig::approve_as_multi(
                RuntimeOrigin::signed(1),
                multi,
                Some(now()),
                hash.clone(),
                Weight::zero()
            ),
            DispatchErrorWithPostInfo {
                error: Error::<Test>::AlreadyApproved.into(),
                post_info: Pays::Yes.into()
            }
        );
        assert_ok!(Multisig::approve_as_multi(
            RuntimeOrigin::signed(2),
            multi,
            Some(now()),
            hash.clone(),
            Weight::zero()
        ));
        assert_noop!(
            Multisig::approve_as_multi(
                RuntimeOrigin::signed(2),
                multi,
                Some(now()),
                hash.clone(),
                Weight::zero()
            ),
            DispatchErrorWithPostInfo {
                error: Error::<Test>::AlreadyApproved.into(),
                post_info: Pays::Yes.into()
            }
        );
    });
}

#[test]
fn multisig_filters() {
    new_test_ext().execute_with(|| {
        let multi = Multisig::multi_account_id(&1, 1, 0);
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(1),
            vec![1],
        ));

        let call = Box::new(RuntimeCall::System(frame_system::Call::set_code {
            code: vec![],
        }));
        let result =
            Multisig::as_multi_threshold_1(RuntimeOrigin::signed(1), multi, call.clone(), now())
                .unwrap_err();
        assert_eq!(
            result.error,
            frame_system::Error::<Test>::CallFiltered.into()
        );
        assert_eq!(result.post_info.pays_fee, Pays::Yes);
        assert!(result.post_info.actual_weight.is_some());
    });
}

#[test]
fn as_multi_threshold_1_unknown_multisig_returns_error() {
    new_test_ext().execute_with(|| {
        let unknown_multisig = 999u64;
        let call = Box::new(RuntimeCall::Balances(BalancesCall::transfer_allow_death {
            dest: 6,
            value: 1,
        }));
        assert_err!(
            Multisig::as_multi_threshold_1(RuntimeOrigin::signed(1), unknown_multisig, call, now()),
            Error::<Test>::UnknownMultisigAccount
        );
    });
}

#[test]
fn weight_check_works() {
    new_test_ext().execute_with(|| {
        let multi = Multisig::multi_account_id(&1, 1, 0);
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(1),
            vec![1, 2],
        ));

        assert_ok!(Balances::transfer(
            &1,
            &multi,
            5,
            ExistenceRequirement::AllowDeath
        ));
        assert_ok!(Balances::transfer(
            &2,
            &multi,
            5,
            ExistenceRequirement::AllowDeath
        ));
        assert_ok!(Balances::transfer(
            &3,
            &multi,
            5,
            ExistenceRequirement::AllowDeath
        ));

        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 15 });
        let data = call.encode();
        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(1),
            multi,
            None,
            data.clone(),
            false,
            Weight::zero()
        ));
        assert_eq!(Balances::free_balance(6), 0);

        assert_noop!(
            Multisig::as_multi(
                RuntimeOrigin::signed(2),
                multi,
                Some(now()),
                data,
                false,
                Weight::zero()
            ),
            DispatchErrorWithPostInfo {
                error: Error::<Test>::WeightTooLow.into(),
                post_info: Pays::Yes.into()
            }
        );
    });
}

#[test]
fn multisig_handles_no_preimage_after_all_approve() {
    // This test checks the situation where everyone approves a multi-sig, but no-one provides the call data.
    // In the end, any of the multisig callers can approve again with the call data and the call will go through.
    new_test_ext().execute_with(|| {
        let multi = Multisig::multi_account_id(&1, 1, 0);
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(1),
            vec![1, 2, 3],
        ));

        assert_ok!(Balances::transfer(
            &1,
            &multi,
            5,
            ExistenceRequirement::AllowDeath
        ));
        assert_ok!(Balances::transfer(
            &2,
            &multi,
            5,
            ExistenceRequirement::AllowDeath
        ));
        assert_ok!(Balances::transfer(
            &3,
            &multi,
            5,
            ExistenceRequirement::AllowDeath
        ));

        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 15 });
        let call_weight = call.get_dispatch_info().call_weight;
        let data = call.encode();
        let hash = blake2_256(&data);
        assert_ok!(Multisig::approve_as_multi(
            RuntimeOrigin::signed(1),
            multi,
            None,
            hash.clone(),
            Weight::zero()
        ));
        assert_ok!(Multisig::approve_as_multi(
            RuntimeOrigin::signed(2),
            multi,
            Some(now()),
            hash.clone(),
            Weight::zero()
        ));
        assert_ok!(Multisig::approve_as_multi(
            RuntimeOrigin::signed(3),
            multi,
            Some(now()),
            hash.clone(),
            Weight::zero()
        ));
        assert_eq!(Balances::free_balance(6), 0);

        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(3),
            multi,
            Some(now()),
            data,
            false,
            call_weight
        ));
        assert_eq!(Balances::free_balance(6), 15);
    });
}

#[test]
fn peer_removal_defers_execution_to_a_weighted_approval() {
    new_test_ext().execute_with(|| {
        let multi = Multisig::multi_account_id(&1, 1, 0);
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(1),
            vec![1, 2, 3],
        ));

        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 15 })
            .encode();
        let hash = blake2_256(&call);
        let timepoint = now();
        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(1),
            multi,
            Some(timepoint),
            call.clone(),
            true,
            Weight::zero()
        ));
        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(2),
            multi,
            Some(timepoint),
            call,
            true,
            Weight::zero()
        ));
        assert!(!Multisig::was_dispatched(&multi, &hash, timepoint));
        assert_ok!(Multisig::remove_signatory(RuntimeOrigin::signed(multi), 3));
        assert!(!Multisig::was_dispatched(&multi, &hash, timepoint));
        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 15 });
        let weight = call.get_dispatch_info().total_weight();
        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(1),
            multi,
            Some(timepoint),
            call.encode(),
            true,
            weight
        ));
        assert!(Multisig::was_dispatched(&multi, &hash, timepoint));
        assert!(!crate::Multisigs::<Test>::contains_key(multi, hash));
    });
}

#[test]
fn executes_call_on_peer_remove_with_post_call_provision() {
    new_test_ext().execute_with(|| {
        let multi = Multisig::multi_account_id(&1, 1, 0);
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(1),
            vec![1, 2, 3],
        ));

        let call1 =
            RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 15 });
        let call_weight = call1.get_dispatch_info().call_weight;
        let call = call1.encode();
        let hash = blake2_256(&call);
        let timepoint = now();
        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(1),
            multi,
            Some(timepoint),
            call.clone(),
            false,
            Weight::zero()
        ));
        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(2),
            multi,
            Some(timepoint),
            call.clone(),
            false,
            Weight::zero()
        ));
        assert_ok!(Multisig::remove_signatory(RuntimeOrigin::signed(multi), 3));
        assert!(!Multisig::was_dispatched(&multi, &hash, timepoint));
        assert!(crate::Multisigs::<Test>::contains_key(multi, hash));
        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(2),
            multi,
            Some(timepoint),
            call,
            false,
            call_weight
        ));
        assert!(Multisig::was_dispatched(&multi, &hash, timepoint));
        assert!(!crate::Multisigs::<Test>::contains_key(multi, hash));
    });
}

#[test]
fn does_not_execute_call_on_peer_remove() {
    new_test_ext().execute_with(|| {
        let multi = Multisig::multi_account_id(&1, 1, 0);
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(1),
            vec![1, 2, 3, 4],
        ));

        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 15 })
            .encode();
        let hash = blake2_256(&call);
        let timepoint = now();
        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(1),
            multi,
            Some(timepoint),
            call.clone(),
            true,
            Weight::zero()
        ));
        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(2),
            multi,
            Some(timepoint),
            call,
            true,
            Weight::zero()
        ));
        assert!(!Multisig::was_dispatched(&multi, &hash, timepoint));
        assert_ok!(Multisig::remove_signatory(RuntimeOrigin::signed(multi), 3));
        assert!(!Multisig::was_dispatched(&multi, &hash, timepoint));
        assert!(crate::Multisigs::<Test>::contains_key(multi, hash));
    });
}

#[test]
fn legacy_numeric_deposits_never_unreserve_currency() {
    new_test_ext().execute_with(|| {
        let multi = Multisig::multi_account_id(&1, 1, 0);
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(1),
            vec![1, 2]
        ));
        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 1 })
            .encode();
        let hash = blake2_256(&call);
        crate::Calls::<Test>::insert(hash, (call, 1, 999));
        crate::Multisigs::<Test>::insert(
            multi,
            hash,
            crate::Multisig {
                when: now(),
                deposit: 999,
                depositor: 1,
                approvals: vec![1],
            },
        );
        assert_ok!(Balances::reserve(&1, 2)); // an unrelated legitimate reserve
        assert_ok!(Multisig::cancel_as_multi(
            RuntimeOrigin::signed(1),
            multi,
            now(),
            hash
        ));
        assert_eq!(Balances::reserved_balance(1), 2);
        assert_eq!(Balances::free_balance(1), 8);
    });
}

#[test]
fn ordinary_bridge_dispatch_and_inner_failure_remain_paid() {
    new_test_ext().execute_with(|| {
        let multi = Multisig::multi_account_id(&1, 1, 0);
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(1),
            vec![1, 2]
        ));
        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 15 });
        let data = call.encode();
        let weight = call.get_dispatch_info().total_weight();
        let first = Multisig::as_multi(
            RuntimeOrigin::signed(1),
            multi,
            Some(now()),
            data.clone(),
            true,
            weight,
        )
        .unwrap();
        assert_eq!(first.pays_fee, Pays::Yes);
        assert_err!(
            Multisig::as_multi(
                RuntimeOrigin::signed(1),
                multi,
                Some(now()),
                data.clone(),
                true,
                weight
            ),
            DispatchErrorWithPostInfo {
                post_info: Pays::Yes.into(),
                error: Error::<Test>::AlreadyApproved.into()
            }
        );
        let final_approval = Multisig::as_multi(
            RuntimeOrigin::signed(2),
            multi,
            Some(now()),
            data,
            true,
            weight,
        )
        .unwrap();
        assert_eq!(final_approval.pays_fee, Pays::Yes);
        assert!(System::events().iter().any(|record| matches!(
            record.event,
            RuntimeEvent::Multisig(Event::MultisigExecuted(_, _, _, _, Some(_)))
        )));
        assert_eq!(Balances::free_balance(6), 0);
    });
}

#[test]
fn legacy_backlog_is_grandfathered_and_new_admission_slots_are_bounded() {
    new_test_ext().execute_with(|| {
        let multi = Multisig::multi_account_id(&1, 1, 0);
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(1),
            vec![1, 2]
        ));
        for n in 0..129u16 {
            let mut hash = [0u8; 32];
            hash[..2].copy_from_slice(&n.to_le_bytes());
            crate::Multisigs::<Test>::insert(
                multi,
                hash,
                crate::Multisig {
                    when: now(),
                    deposit: 999,
                    depositor: 1,
                    approvals: vec![1],
                },
            );
        }
        assert_eq!(crate::NewPendingOperations::<Test>::get(multi), 0);
        for n in 0..128u64 {
            let call =
                RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: n })
                    .encode();
            assert_ok!(Multisig::as_multi(
                RuntimeOrigin::signed(n % 2 + 1),
                multi,
                Some(now()),
                call,
                true,
                Weight::zero()
            ));
        }
        assert_eq!(crate::NewPendingOperations::<Test>::get(multi), 128);
        let extra = RuntimeCall::Balances(BalancesCall::transfer_allow_death {
            dest: 6,
            value: 128,
        })
        .encode();
        assert_err!(
            Multisig::as_multi(
                RuntimeOrigin::signed(1),
                multi,
                Some(now()),
                extra.clone(),
                true,
                Weight::zero()
            ),
            Error::<Test>::TooManyPendingOperations
        );
        // Removing a grandfathered operation neither frees a new slot nor underflows.
        assert_ok!(Multisig::cancel_as_multi(
            RuntimeOrigin::signed(1),
            multi,
            now(),
            [0; 32]
        ));
        assert_eq!(crate::NewPendingOperations::<Test>::get(multi), 128);
        let first = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 0 })
            .encode();
        assert_ok!(Multisig::cancel_as_multi(
            RuntimeOrigin::signed(1),
            multi,
            now(),
            blake2_256(&first)
        ));
        assert_eq!(crate::NewPendingOperations::<Test>::get(multi), 127);
        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(1),
            multi,
            Some(now()),
            extra,
            true,
            Weight::zero()
        ));
        assert_eq!(crate::NewPendingOperations::<Test>::get(multi), 128);
    });
}

#[test]
fn proposer_quota_preserves_capacity_for_other_members() {
    new_test_ext().execute_with(|| {
        let multi = Multisig::multi_account_id(&1, 1, 0);
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(1),
            vec![1, 2, 3, 4]
        ));
        let call =
            |value| RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value });
        for value in 0..32 {
            assert_ok!(Multisig::as_multi(
                RuntimeOrigin::signed(1),
                multi,
                Some(now()),
                call(value).encode(),
                true,
                Weight::zero()
            ));
        }
        assert_eq!(
            crate::PendingOperationsByProposer::<Test>::get(multi, 1),
            32
        );
        assert_eq!(crate::NewPendingOperations::<Test>::get(multi), 32);
        let extra = call(32);
        let hash = blake2_256(&extra.encode());
        let before = sp_io::storage::root(sp_runtime::StateVersion::V1);
        assert_err!(
            Multisig::validate_protocol_operation(
                &1,
                &multi,
                Some(now()),
                &hash,
                &extra,
                true,
                Weight::zero(),
                false
            ),
            Error::<Test>::TooManyPendingOperationsByProposer
        );
        assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), before);
        assert_err!(
            Multisig::as_multi(
                RuntimeOrigin::signed(1),
                multi,
                Some(now()),
                extra.encode(),
                true,
                Weight::zero()
            ),
            Error::<Test>::TooManyPendingOperationsByProposer
        );
        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(2),
            multi,
            Some(now()),
            extra.encode(),
            true,
            Weight::zero()
        ));
        assert_eq!(crate::PendingOperationsByProposer::<Test>::get(multi, 2), 1);
        assert_eq!(crate::NewPendingOperations::<Test>::get(multi), 33);
        let first_hash = blake2_256(&call(0).encode());
        assert_ok!(Multisig::cancel_as_multi(
            RuntimeOrigin::signed(1),
            multi,
            now(),
            first_hash
        ));
        assert_eq!(
            crate::PendingOperationsByProposer::<Test>::get(multi, 1),
            31
        );
        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(1),
            multi,
            Some(now()),
            call(33).encode(),
            true,
            Weight::zero()
        ));
        assert_eq!(
            crate::PendingOperationsByProposer::<Test>::get(multi, 1),
            32
        );
    });
}

#[test]
fn legacy_stored_call_loader_accepts_valid_encoding_at_the_call_limit() {
    new_test_ext().execute_with(|| {
        let cap = <<Test as Config>::MaxCallBytes as Get<u32>>::get() as usize;
        let call = RuntimeCall::System(frame_system::Call::remark {
            remark: vec![7; cap - 4],
        });
        let encoded = call.encode();
        assert_eq!(encoded.len(), cap);
        let hash = blake2_256(&encoded);
        let stored = (encoded, 1u64, 9u64).encode();
        let key = crate::Calls::<Test>::hashed_key_for(hash);
        sp_io::storage::set(&key, &stored);
        assert_eq!(Multisig::decode_call(&hash, None), Some(call));
        assert_eq!(
            sp_io::storage::get(&key).unwrap().as_ref(),
            stored.as_slice()
        );
    });
}

#[test]
fn legacy_stored_call_loader_rejects_oversized_and_malformed_values_without_mutation() {
    new_test_ext().execute_with(|| {
        let cap = <<Test as Config>::MaxCallBytes as Get<u32>>::get() as usize;
        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 1 });
        let encoded = call.encode();
        let hash = blake2_256(&encoded);
        let key = crate::Calls::<Test>::hashed_key_for(hash);
        assert!(Multisig::decode_call(&hash, None).is_none());
        let valid = (encoded.clone(), 1u64, 0u64).encode();
        let mut trailing = valid.clone();
        trailing.push(0);
        let mut oversized_suffix = valid.clone();
        oversized_suffix.extend(vec![0; cap]);
        let mut truncated_suffix = valid.clone();
        truncated_suffix.pop();
        let cases = [
            // This one-byte overflow still fits the conservative total tuple bound;
            // it must be rejected by the declared call length itself.
            (vec![0u8; cap + 1], 1u64, 0u64).encode(),
            (vec![0u8; cap * 2], 1u64, 0u64).encode(),
            oversized_suffix,
            vec![0xff; 5],    // Compact<u32> claims an unsupported integer width.
            vec![0x01, 0x00], // Non-canonical two-byte encoding of zero.
            vec![0x02],       // Truncated four-byte compact prefix.
            Compact(cap as u32).encode(), // Complete prefix, missing call body.
            truncated_suffix,
            trailing,
        ];
        for value in cases {
            sp_io::storage::set(&key, &value);
            assert!(Multisig::decode_call(&hash, None).is_none());
            assert_eq!(
                sp_io::storage::get(&key).unwrap().as_ref(),
                value.as_slice()
            );
            // Supplying valid call bytes does not require decoding the historical value.
            assert_eq!(
                Multisig::decode_call(&hash, Some(&encoded)),
                Some(call.clone())
            );
        }
    });
}

#[test]
fn quorum_cancellation_removes_oversized_legacy_call_and_releases_capacity() {
    new_test_ext().execute_with(|| {
        let multi = Multisig::multi_account_id(&1, 1, 0);
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(1),
            vec![1, 2]
        ));
        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 1 });
        let hash = blake2_256(&call.encode());
        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(1),
            multi,
            Some(now()),
            call.encode(),
            true,
            Weight::zero()
        ));
        let oversized = vec![0u8; <<Test as Config>::MaxCallBytes as Get<u32>>::get() as usize + 1];
        crate::Calls::<Test>::insert(hash, (oversized, 1u64, 0u64));
        assert!(Multisig::decode_call(&hash, None).is_none());
        assert_eq!(crate::NewPendingOperations::<Test>::get(multi), 1);
        assert_eq!(crate::PendingOperationsByProposer::<Test>::get(multi, 1), 1);
        for signer in [1, 2] {
            assert_ok!(Multisig::approve_cancellation(
                &signer,
                &multi,
                now(),
                &hash,
                |_| true
            ));
        }
        assert!(!crate::Calls::<Test>::contains_key(hash));
        assert!(!crate::Multisigs::<Test>::contains_key(multi, hash));
        assert_eq!(crate::NewPendingOperations::<Test>::get(multi), 0);
        assert_eq!(crate::PendingOperationsByProposer::<Test>::get(multi, 1), 0);
        assert!(!Multisig::was_dispatched(&multi, &hash, now()));
        assert_eq!(Balances::free_balance(6), 0);
    });
}

#[test]
fn quorum_cancellation_frees_full_capacity_without_depositor_or_call_bytes() {
    new_test_ext().execute_with(|| {
        let multi = Multisig::multi_account_id(&1, 1, 0);
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(1),
            vec![1, 2, 3, 4]
        ));
        for value in 0..128u64 {
            let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value });
            assert_ok!(Multisig::as_multi(
                RuntimeOrigin::signed(value % 4 + 1),
                multi,
                Some(now()),
                call.encode(),
                true,
                Weight::zero()
            ));
        }
        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 0 });
        let hash = blake2_256(&call.encode());
        // A different account can dispatch the same payload/timepoint, removing
        // shared call bytes and recording its own marker while our entry stays.
        let foreign = Multisig::multi_account_id(&5, 1, 0);
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(5),
            vec![5, 6]
        ));
        for signer in [5, 6] {
            assert_ok!(Multisig::as_multi(
                RuntimeOrigin::signed(signer),
                foreign,
                Some(now()),
                call.encode(),
                false,
                call.get_dispatch_info().total_weight()
            ));
        }
        assert!(Multisig::was_dispatched(&foreign, &hash, now()));
        assert!(!Multisig::was_dispatched(&multi, &hash, now()));
        assert!(crate::Multisigs::<Test>::contains_key(multi, hash));
        assert!(!crate::Calls::<Test>::contains_key(hash));
        assert_eq!(crate::NewPendingOperations::<Test>::get(multi), 128);
        assert_ok!(Multisig::validate_cancellation(
            &2,
            &multi,
            now(),
            &hash,
            |_| true
        ));
        assert_ok!(Multisig::approve_cancellation(
            &2,
            &multi,
            now(),
            &hash,
            |_| true
        ));
        assert_eq!(
            crate::CancellationApprovals::<Test>::get(multi, hash)
                .unwrap()
                .1
                .as_slice(),
            &[2]
        );
        assert_eq!(crate::NewPendingOperations::<Test>::get(multi), 128);
        let before = sp_io::storage::root(sp_runtime::StateVersion::V1);
        assert_err!(
            Multisig::validate_cancellation(&2, &multi, now(), &hash, |_| true),
            Error::<Test>::AlreadyApproved
        );
        assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), before);
        assert_err!(
            Multisig::approve_cancellation(&2, &multi, now(), &hash, |_| true),
            Error::<Test>::AlreadyApproved
        );
        assert_err!(
            Multisig::validate_cancellation(&5, &multi, now(), &hash, |_| true),
            Error::<Test>::NotInSignatories
        );
        assert_err!(
            Multisig::validate_cancellation(&3, &multi, now(), &hash, |_| false),
            Error::<Test>::NotInSignatories
        );
        let mut wrong_timepoint = now();
        wrong_timepoint.index = wrong_timepoint.index.saturating_add(1);
        assert_err!(
            Multisig::validate_cancellation(&3, &multi, wrong_timepoint, &hash, |_| true),
            Error::<Test>::WrongTimepoint
        );
        assert_ok!(Multisig::approve_cancellation(
            &3,
            &multi,
            now(),
            &hash,
            |_| true
        ));
        assert_eq!(crate::NewPendingOperations::<Test>::get(multi), 128);
        assert_ok!(Multisig::approve_cancellation(
            &4,
            &multi,
            now(),
            &hash,
            |_| true
        ));
        assert_eq!(crate::NewPendingOperations::<Test>::get(multi), 127);
        assert_eq!(
            crate::PendingOperationsByProposer::<Test>::get(multi, 1),
            31
        );
        for peer in 2..=4 {
            assert_eq!(
                crate::PendingOperationsByProposer::<Test>::get(multi, peer),
                32
            );
        }
        assert!(!crate::Multisigs::<Test>::contains_key(multi, hash));
        assert!(!crate::Calls::<Test>::contains_key(hash));
        assert!(!crate::CancellationApprovals::<Test>::contains_key(
            multi, hash
        ));
        assert!(!crate::CountedNewOperations::<Test>::contains_key(
            multi, hash
        ));
        assert!(!crate::ProposerCountedOperations::<Test>::contains_key(
            multi, hash
        ));
        assert_eq!(Balances::free_balance(6), 0);
        assert!(Multisig::was_dispatched(&foreign, &hash, now()));
        assert!(!Multisig::was_dispatched(&multi, &hash, now()));
        assert_err!(
            Multisig::approve_cancellation(&4, &multi, now(), &hash, |_| true),
            Error::<Test>::NotFound
        );
        assert_eq!(crate::NewPendingOperations::<Test>::get(multi), 127);
    });
}

#[test]
fn cancellation_prunes_peers_removed_from_the_bridge_membership() {
    new_test_ext().execute_with(|| {
        let multi = Multisig::multi_account_id(&1, 1, 0);
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(1),
            vec![1, 2, 3, 4]
        ));
        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 1 });
        let hash = blake2_256(&call.encode());
        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(1),
            multi,
            Some(now()),
            call.encode(),
            false,
            Weight::zero()
        ));
        assert_ok!(Multisig::approve_cancellation(
            &2,
            &multi,
            now(),
            &hash,
            |_| true
        ));
        assert_ok!(Multisig::approve_cancellation(
            &3,
            &multi,
            now(),
            &hash,
            |_| true
        ));
        let eligible = |peer: &u64| *peer != 3;
        assert_ok!(Multisig::approve_cancellation(
            &4,
            &multi,
            now(),
            &hash,
            eligible
        ));
        assert_eq!(
            crate::CancellationApprovals::<Test>::get(multi, hash)
                .unwrap()
                .1
                .as_slice(),
            &[2, 4]
        );
        assert!(crate::Multisigs::<Test>::contains_key(multi, hash));
        assert_ok!(Multisig::approve_cancellation(
            &1,
            &multi,
            now(),
            &hash,
            eligible
        ));
        assert!(!crate::Multisigs::<Test>::contains_key(multi, hash));
    });
}

#[test]
fn cancellation_prunes_signatories_and_finishes_after_threshold_reduction() {
    new_test_ext().execute_with(|| {
        let multi = Multisig::multi_account_id(&1, 1, 0);
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(1),
            vec![1, 2, 3, 4]
        ));
        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 1 });
        let hash = blake2_256(&call.encode());
        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(4),
            multi,
            Some(now()),
            call.encode(),
            false,
            Weight::zero()
        ));
        assert_ok!(Multisig::approve_cancellation(
            &1,
            &multi,
            now(),
            &hash,
            |_| true
        ));
        assert_ok!(Multisig::approve_cancellation(
            &2,
            &multi,
            now(),
            &hash,
            |_| true
        ));
        assert_ok!(Multisig::remove_signatory(RuntimeOrigin::signed(multi), 1));
        assert_ok!(Multisig::approve_cancellation(
            &3,
            &multi,
            now(),
            &hash,
            |_| true
        ));
        assert_eq!(
            crate::CancellationApprovals::<Test>::get(multi, hash)
                .unwrap()
                .1
                .as_slice(),
            &[2, 3]
        );
        assert_ok!(Multisig::remove_signatory(RuntimeOrigin::signed(multi), 4));
        assert_ok!(Multisig::validate_cancellation(
            &2,
            &multi,
            now(),
            &hash,
            |_| true
        ));
        assert_ok!(Multisig::approve_cancellation(
            &2,
            &multi,
            now(),
            &hash,
            |_| true
        ));
        assert_eq!(crate::PendingOperationsByProposer::<Test>::get(multi, 4), 0);
        assert_eq!(crate::NewPendingOperations::<Test>::get(multi), 0);
        assert!(!crate::CancellationApprovals::<Test>::contains_key(
            multi, hash
        ));
    });
}

#[test]
fn cancellation_completes_an_orphan_after_threshold_drops_to_one() {
    new_test_ext().execute_with(|| {
        let multi = Multisig::multi_account_id(&1, 1, 0);
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(1),
            vec![1, 2]
        ));
        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 1 });
        let hash = blake2_256(&call.encode());
        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(1),
            multi,
            Some(now()),
            call.encode(),
            false,
            Weight::zero()
        ));
        assert_ok!(Multisig::approve_cancellation(
            &2,
            &multi,
            now(),
            &hash,
            |_| true
        ));
        assert_ok!(Multisig::remove_signatory(RuntimeOrigin::signed(multi), 1));
        assert_ok!(Multisig::validate_cancellation(
            &2,
            &multi,
            now(),
            &hash,
            |_| true
        ));
        assert_ok!(Multisig::approve_cancellation(
            &2,
            &multi,
            now(),
            &hash,
            |_| true
        ));
        assert_eq!(crate::PendingOperationsByProposer::<Test>::get(multi, 1), 0);
        assert_eq!(crate::NewPendingOperations::<Test>::get(multi), 0);
        assert!(!crate::Multisigs::<Test>::contains_key(multi, hash));
    });
}

#[test]
fn normal_dispatch_and_owner_cancel_clear_pending_cancellation_votes() {
    new_test_ext().execute_with(|| {
        let multi = Multisig::multi_account_id(&1, 1, 0);
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(1),
            vec![1, 2]
        ));
        let _ = Balances::make_free_balance_be(&multi, 10);
        for value in 1..=2 {
            let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value });
            let hash = blake2_256(&call.encode());
            let weight = call.get_dispatch_info().total_weight();
            assert_ok!(Multisig::as_multi(
                RuntimeOrigin::signed(1),
                multi,
                Some(now()),
                call.encode(),
                true,
                weight
            ));
            assert_ok!(Multisig::approve_cancellation(
                &2,
                &multi,
                now(),
                &hash,
                |_| true
            ));
            assert!(crate::CancellationApprovals::<Test>::contains_key(
                multi, hash
            ));
            if value == 1 {
                assert_ok!(Multisig::as_multi(
                    RuntimeOrigin::signed(2),
                    multi,
                    Some(now()),
                    call.encode(),
                    true,
                    weight
                ));
            } else {
                assert_ok!(Multisig::cancel_as_multi(
                    RuntimeOrigin::signed(1),
                    multi,
                    now(),
                    hash
                ));
            }
            assert!(!crate::CancellationApprovals::<Test>::contains_key(
                multi, hash
            ));
            assert!(!crate::ProposerCountedOperations::<Test>::contains_key(
                multi, hash
            ));
            assert_eq!(crate::PendingOperationsByProposer::<Test>::get(multi, 1), 0);
            assert_eq!(crate::NewPendingOperations::<Test>::get(multi), 0);
        }
        assert_eq!(Balances::free_balance(6), 1);
    });
}

#[test]
fn cancelling_legacy_operations_does_not_debit_unrecorded_proposer_counts() {
    new_test_ext().execute_with(|| {
        let multi = Multisig::multi_account_id(&1, 1, 0);
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(1),
            vec![1, 2]
        ));
        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 1 });
        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(1),
            multi,
            Some(now()),
            call.encode(),
            true,
            Weight::zero()
        ));
        for counted in [false, true] {
            let hash = [u8::from(counted) + 10; 32];
            crate::Multisigs::<Test>::insert(
                multi,
                hash,
                crate::Multisig {
                    when: now(),
                    deposit: 999,
                    depositor: 1,
                    approvals: vec![1],
                },
            );
            if counted {
                crate::CountedNewOperations::<Test>::insert(multi, hash, ());
                crate::NewPendingOperations::<Test>::mutate(multi, |count| *count += 1);
            }
            assert_ok!(Multisig::approve_cancellation(
                &1,
                &multi,
                now(),
                &hash,
                |_| true
            ));
            assert_ok!(Multisig::approve_cancellation(
                &2,
                &multi,
                now(),
                &hash,
                |_| true
            ));
            assert_eq!(crate::NewPendingOperations::<Test>::get(multi), 1);
            assert_eq!(crate::PendingOperationsByProposer::<Test>::get(multi, 1), 1);
        }
    });
}

#[test]
fn dispatch_only_decrements_marked_new_operations() {
    new_test_ext().execute_with(|| {
        let multi = Multisig::multi_account_id(&1, 1, 0);
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(1),
            vec![1, 2]
        ));
        let _ = Balances::make_free_balance_be(&multi, 10);
        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 1 });
        let weight = call.get_dispatch_info().total_weight();
        let encoded = call.encode();
        let hash = blake2_256(&encoded);
        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(1),
            multi,
            Some(now()),
            encoded.clone(),
            true,
            weight
        ));
        assert_eq!(crate::NewPendingOperations::<Test>::get(multi), 1);
        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(2),
            multi,
            Some(now()),
            encoded,
            true,
            weight
        ));
        assert_eq!(crate::NewPendingOperations::<Test>::get(multi), 0);
        assert!(!crate::CountedNewOperations::<Test>::contains_key(
            multi, hash
        ));
        // A legacy operation dispatch has no corresponding currency hold or counter marker.
        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 2 });
        let encoded = call.encode();
        let hash = blake2_256(&encoded);
        crate::Calls::<Test>::insert(hash, (encoded.clone(), 1, 999));
        crate::Multisigs::<Test>::insert(
            multi,
            hash,
            crate::Multisig {
                when: now(),
                deposit: 999,
                depositor: 1,
                approvals: vec![1],
            },
        );
        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(2),
            multi,
            Some(now()),
            encoded,
            true,
            call.get_dispatch_info().total_weight()
        ));
        assert_eq!(crate::NewPendingOperations::<Test>::get(multi), 0);
    });
}

#[test]
fn protocol_admission_checks_membership_timepoint_duplicates_and_execution_weight() {
    new_test_ext().execute_with(|| {
        let id = Multisig::register_multisig_inner(1, vec![1, 2, 3, 4]).unwrap();
        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 1 });
        let hash = blake2_256(&call.encode());
        let weight = call.get_dispatch_info().total_weight();
        assert_err!(
            Multisig::validate_protocol_operation(
                &1, &999, None, &hash, &call, false, weight, false
            ),
            Error::<Test>::UnknownMultisigAccount
        );
        assert_err!(
            Multisig::validate_protocol_operation(
                &5, &id, None, &hash, &call, false, weight, false
            ),
            Error::<Test>::NotInSignatories
        );
        assert_err!(
            Multisig::validate_protocol_operation(
                &1,
                &id,
                Some(now()),
                &hash,
                &call,
                false,
                weight,
                true
            ),
            Error::<Test>::TooManySignatories
        );
        assert_ok!(Multisig::validate_protocol_operation(
            &1,
            &id,
            None,
            &hash,
            &call,
            false,
            Weight::zero(),
            false
        ));
        assert!(!Multisigs::<Test>::contains_key(id, hash));
        assert_ok!(Multisig::approve_as_multi(
            RuntimeOrigin::signed(1),
            id,
            None,
            hash,
            Weight::zero()
        ));
        let when = now();
        let operation = Multisigs::<Test>::get(id, hash).unwrap();
        assert_err!(
            Multisig::validate_protocol_operation(
                &2, &id, None, &hash, &call, false, weight, false
            ),
            Error::<Test>::NoTimepoint
        );
        assert_err!(
            Multisig::validate_protocol_operation(
                &2,
                &id,
                Some(Multisig::sidechain_timepoint(99, 1)),
                &hash,
                &call,
                false,
                weight,
                false
            ),
            Error::<Test>::WrongTimepoint
        );
        assert_err!(
            Multisig::validate_protocol_operation(
                &1,
                &id,
                Some(when),
                &hash,
                &call,
                false,
                weight,
                false
            ),
            Error::<Test>::AlreadyApproved
        );
        assert_ok!(Multisig::validate_protocol_operation(
            &1,
            &id,
            Some(when),
            &hash,
            &call,
            true,
            Weight::zero(),
            false
        ));
        assert_ok!(Multisig::validate_protocol_operation(
            &2,
            &id,
            Some(when),
            &hash,
            &call,
            false,
            Weight::zero(),
            false
        ));
        assert_eq!(Multisigs::<Test>::get(id, hash), Some(operation));
        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(2),
            id,
            Some(when),
            call.encode(),
            true,
            weight
        ));
        assert_err!(
            Multisig::validate_protocol_operation(
                &1,
                &id,
                Some(when),
                &hash,
                &call,
                true,
                weight,
                false
            ),
            Error::<Test>::AlreadyApproved
        );
        assert_err!(
            Multisig::validate_protocol_operation(
                &3,
                &id,
                Some(when),
                &hash,
                &call,
                false,
                Weight::zero(),
                false
            ),
            Error::<Test>::WeightTooLow
        );
        assert_ok!(Multisig::validate_protocol_operation(
            &3,
            &id,
            Some(when),
            &hash,
            &call,
            false,
            weight,
            false
        ));
        DispatchedCalls::<Test>::insert(hash, when, ());
        assert_err!(
            Multisig::validate_protocol_operation(
                &3,
                &id,
                Some(when),
                &hash,
                &call,
                false,
                weight,
                false
            ),
            Error::<Test>::AlreadyDispatched
        );
    });
}

#[test]
fn threshold_one_protocol_admission_rejects_replay_and_invalid_account_state() {
    new_test_ext().execute_with(|| {
        let id = Multisig::register_multisig_inner(1, vec![1]).unwrap();
        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 1 });
        let hash = blake2_256(&call.encode());
        let when = Multisig::sidechain_timepoint(17, 2);
        let weight = call.get_dispatch_info().total_weight();
        assert_err!(
            Multisig::validate_protocol_operation(
                &1,
                &id,
                Some(when),
                &hash,
                &call,
                false,
                weight,
                false
            ),
            Error::<Test>::MinimumThreshold
        );
        assert_err!(
            Multisig::validate_protocol_operation(&1, &id, None, &hash, &call, false, weight, true),
            Error::<Test>::NoTimepoint
        );
        assert_ok!(Multisig::validate_protocol_operation(
            &1,
            &id,
            Some(when),
            &hash,
            &call,
            false,
            weight,
            true
        ));
        DispatchedCalls::<Test>::insert(hash, when, ());
        assert_err!(
            Multisig::validate_protocol_operation(
                &1,
                &id,
                Some(when),
                &hash,
                &call,
                false,
                weight,
                true
            ),
            Error::<Test>::AlreadyDispatched
        );
        Accounts::<Test>::insert(id, MultisigAccount::new(Vec::<u64>::new()));
        assert_err!(
            Multisig::validate_protocol_operation(
                &1,
                &id,
                Some(when),
                &hash,
                &call,
                false,
                weight,
                true
            ),
            Error::<Test>::TooFewSignatories
        );
    });
}

#[test]
fn protocol_call_decoder_rejects_trailing_bytes_and_oversize_payloads() {
    new_test_ext().execute_with(|| {
        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 1 });
        let data = call.encode();
        let hash = blake2_256(&data);
        assert_eq!(
            Multisig::decode_call(&hash, Some(&data)),
            Some(call.clone())
        );
        assert_eq!(Multisig::decode_call(&hash, Some(&[])), None);
        let mut trailing = data.clone();
        trailing.push(0);
        assert_eq!(Multisig::decode_call(&hash, Some(&trailing)), None);
        let max_call_bytes: u32 = <Test as Config>::MaxCallBytes::get();
        let oversized = vec![0; max_call_bytes as usize + 1];
        assert_eq!(Multisig::decode_call(&hash, Some(&oversized)), None);
        let oversized_call = RuntimeCall::System(frame_system::Call::remark { remark: oversized });
        let id = Multisig::register_multisig_inner(1, vec![1, 2, 3]).unwrap();
        assert_err!(
            Multisig::validate_protocol_operation(
                &1,
                &id,
                None,
                &hash,
                &oversized_call,
                false,
                Weight::MAX,
                false
            ),
            Error::<Test>::CallTooLarge
        );
    });
}

// Model a runtime call-filter change independently of FRAME's outer call filter.
pub struct TestMultisigCallFilter;
impl Contains<RuntimeCall> for TestMultisigCallFilter {
    fn contains(_: &RuntimeCall) -> bool {
        !sp_io::storage::exists(b"test/multisig-call-filter-closed")
    }
}

#[test]
fn private_admission_defenses_reject_oversized_preimages_and_replayed_pending_calls() {
    new_test_ext().execute_with(|| {
        let id = Multisig::register_multisig_inner(1, vec![1, 2]).unwrap();
        let oversized = RuntimeCall::System(frame_system::Call::remark {
            remark: vec![0; 16_385],
        });
        let data = oversized.encode();
        assert_noop!(
            Multisig::operate(
                Multisig::accounts(id).unwrap(),
                2,
                1,
                id,
                None,
                CallOrHash::Call(data.clone(), true),
                Weight::MAX
            ),
            Error::<Test>::CallTooLarge
        );
        assert_noop!(
            Multisig::inner_as_multi_threshold_1(1, id, Box::new(oversized), now()),
            Error::<Test>::CallTooLarge
        );
        assert!(Multisig::get_call(&blake2_256(&data), Some(&data)).is_none());
        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 1 });
        let hash = blake2_256(&call.encode());
        assert_ok!(Multisig::approve_as_multi(
            RuntimeOrigin::signed(1),
            id,
            None,
            hash,
            Weight::zero()
        ));
        DispatchedCalls::<Test>::insert(hash, now(), ());
        assert_noop!(
            Multisig::operate(
                Multisig::accounts(id).unwrap(),
                2,
                2,
                id,
                Some(now()),
                CallOrHash::Hash(hash),
                Weight::MAX
            ),
            Error::<Test>::AlreadyDispatched
        );
        assert_eq!(Multisigs::<Test>::get(id, hash).unwrap().approvals, vec![1]);
    });
}

#[test]
fn filtered_pending_call_is_cleaned_up_without_dispatching_or_refunding_fees() {
    new_test_ext().execute_with(|| {
        let id = Multisig::register_multisig_inner(1, vec![1, 2]).unwrap();
        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 1 });
        let bytes = call.encode();
        let hash = blake2_256(&bytes);
        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(1),
            id,
            None,
            bytes.clone(),
            true,
            Weight::zero()
        ));
        sp_io::storage::set(b"test/multisig-call-filter-closed", &[1]);
        assert_noop!(
            Multisig::inner_as_multi_threshold_1(1, id, Box::new(call.clone()), now()),
            frame_system::Error::<Test>::CallFiltered
        );
        assert_noop!(
            Multisig::validate_protocol_operation(
                &1,
                &id,
                Some(now()),
                &hash,
                &call,
                true,
                Weight::MAX,
                false
            ),
            frame_system::Error::<Test>::CallFiltered
        );
        let post = Multisig::dispatch_call(&2, &id, 2, hash, bytes.len(), now(), call).unwrap();
        assert_eq!(post.pays_fee, Pays::Yes);
        assert!(post.actual_weight.is_some());
        assert_eq!(Balances::free_balance(6), 0);
        assert!(!Multisigs::<Test>::contains_key(id, hash));
        assert!(!Calls::<Test>::contains_key(hash));
        assert_eq!(NewPendingOperations::<Test>::get(id), 0);
        assert_eq!(PendingOperationsByProposer::<Test>::get(id, 1), 0);
        assert!(Multisig::was_dispatched(&id, &hash, now()));
        expect_event(Event::<Test>::MultisigExecuted(
            2,
            now(),
            id,
            hash,
            Some(frame_system::Error::<Test>::CallFiltered.into()),
        ));
    });
}

#[test]
fn protocol_admission_rechecks_capacity_and_shared_replay_markers_before_recording() {
    new_test_ext().execute_with(|| {
        let id = Multisig::register_multisig_inner(1, vec![1, 2]).unwrap();
        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 1 });
        let hash = blake2_256(&call.encode());
        NewPendingOperations::<Test>::insert(id, 128);
        assert_noop!(
            Multisig::validate_protocol_operation(
                &1,
                &id,
                None,
                &hash,
                &call,
                false,
                Weight::MAX,
                false
            ),
            Error::<Test>::TooManyPendingOperations
        );
        NewPendingOperations::<Test>::remove(id);
        DispatchedCalls::<Test>::insert(hash, now(), ());
        assert_noop!(
            Multisig::validate_protocol_operation(
                &1,
                &id,
                None,
                &hash,
                &call,
                false,
                Weight::MAX,
                false
            ),
            Error::<Test>::AlreadyDispatched
        );
        assert!(!Multisigs::<Test>::contains_key(id, hash));
        let declared = RuntimeCall::Multisig(MultisigCall::as_multi {
            id,
            maybe_timepoint: None,
            call: call.encode(),
            store_call: false,
            max_weight: Weight::MAX,
        })
        .get_dispatch_info()
        .total_weight();
        assert!(declared.all_gte(Multisig::protocol_validation_weight()));
    });
}

#[test]
fn cancellation_rejects_empty_membership_and_orphan_markers_preserve_unrelated_proposer_counts() {
    new_test_ext().execute_with(|| {
        let id = Multisig::register_multisig_inner(1, vec![1, 2]).unwrap();
        let hash = [76; 32];
        Accounts::<Test>::insert(id, MultisigAccount::new(vec![]));
        assert_noop!(
            Multisig::validate_cancellation(&1, &id, now(), &hash, |_| true),
            Error::<Test>::TooFewSignatories
        );
        ProposerCountedOperations::<Test>::insert(id, hash, ());
        PendingOperationsByProposer::<Test>::insert(id, 1, 7);
        Multisig::remove_pending_operation(&id, &hash);
        assert!(!ProposerCountedOperations::<Test>::contains_key(id, hash));
        assert_eq!(PendingOperationsByProposer::<Test>::get(id, 1), 7);
    });
}

#[test]
fn failed_foreign_dispatch_cannot_consume_another_accounts_replay_marker() {
    new_test_ext().execute_with(|| {
        let victim = Multisig::multi_account_id(&1, 1, 0);
        let foreign = Multisig::multi_account_id(&5, 1, 0);
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(1),
            vec![1, 2]
        ));
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(5),
            vec![5, 6]
        ));
        assert_ok!(Balances::transfer_allow_death(
            RuntimeOrigin::signed(1),
            victim,
            5
        ));
        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 4 });
        let hash = blake2_256(&call.encode());
        let weight = call.get_dispatch_info().total_weight();
        let before = Balances::free_balance(6);
        // The foreign multisig has no funds. Its inner failure still consumes its
        // own operation, but cannot poison the identical call/timepoint for victim.
        for signer in [5, 6] {
            assert_ok!(Multisig::as_multi(
                RuntimeOrigin::signed(signer),
                foreign,
                Some(now()),
                call.encode(),
                true,
                weight
            ));
        }
        assert_eq!(Balances::free_balance(6), before);
        assert!(Multisig::was_dispatched(&foreign, &hash, now()));
        assert!(!Multisig::was_dispatched(&victim, &hash, now()));
        assert!(!DispatchedCalls::<Test>::contains_key(hash, now()));
        assert_noop!(
            Multisig::as_multi(
                RuntimeOrigin::signed(5),
                foreign,
                Some(now()),
                call.encode(),
                true,
                weight
            ),
            Error::<Test>::AlreadyDispatched
        );
        for signer in [1, 2] {
            assert_ok!(Multisig::as_multi(
                RuntimeOrigin::signed(signer),
                victim,
                Some(now()),
                call.encode(),
                true,
                weight
            ));
        }
        assert_eq!(Balances::free_balance(6), before + 4);
        assert!(Multisig::was_dispatched(&victim, &hash, now()));
        assert_noop!(
            Multisig::as_multi(
                RuntimeOrigin::signed(1),
                victim,
                Some(now()),
                call.encode(),
                true,
                weight
            ),
            Error::<Test>::AlreadyDispatched
        );
    });
}

#[test]
fn legacy_global_marker_remains_a_replay_barrier_without_migration() {
    new_test_ext().execute_with(|| {
        let call = RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest: 6, value: 4 });
        let hash = blake2_256(&call.encode());
        DispatchedCalls::<Test>::insert(hash, now(), ());
        for (creator, members) in [(1, vec![1]), (5, vec![5, 6])] {
            let id = Multisig::multi_account_id(&creator, 1, 0);
            assert_ok!(Multisig::register_multisig(
                RuntimeOrigin::signed(creator),
                members.clone()
            ));
            assert!(Multisig::was_dispatched(&id, &hash, now()));
            assert_noop!(
                Multisig::validate_protocol_operation(
                    &creator,
                    &id,
                    Some(now()),
                    &hash,
                    &call,
                    true,
                    Weight::MAX,
                    members.len() == 1
                ),
                Error::<Test>::AlreadyDispatched
            );
            if members.len() == 1 {
                assert_noop!(
                    Multisig::as_multi_threshold_1(
                        RuntimeOrigin::signed(creator),
                        id,
                        Box::new(call.clone()),
                        now()
                    ),
                    Error::<Test>::AlreadyDispatched
                );
            } else {
                assert_noop!(
                    Multisig::as_multi(
                        RuntimeOrigin::signed(creator),
                        id,
                        Some(now()),
                        call.encode(),
                        true,
                        Weight::MAX
                    ),
                    Error::<Test>::AlreadyDispatched
                );
            }
            assert!(!AccountDispatchedCalls::<Test>::contains_key(
                id,
                (hash, now())
            ));
        }
        assert!(DispatchedCalls::<Test>::contains_key(hash, now()));
    });
}
