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
    BuildStorage, DispatchError, ModuleError, Perbill,
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
    type MaxHolds = ();
    type MaxFreezes = ();
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
use frame_support::traits::Contains;
use pallet_balances::Call as BalancesCall;

pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    pallet_balances::GenesisConfig::<Test> {
        balances: vec![(1, 10), (2, 10), (3, 10), (4, 10), (5, 2)],
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
fn multisig_deposit_is_taken_and_returned() {
    new_test_ext().execute_with(|| {
        let multi = Multisig::multi_account_id(&1, 1, 0);
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(1),
            vec![1, 2, 3],
        ));
        assert_ok!(Balances::transfer(RuntimeOrigin::signed(1), multi, 5));
        assert_ok!(Balances::transfer(RuntimeOrigin::signed(2), multi, 5));
        assert_ok!(Balances::transfer(RuntimeOrigin::signed(3), multi, 5));

        let call = RuntimeCall::Balances(BalancesCall::transfer { dest: 6, value: 15 });
        let call_weight = call.get_dispatch_info().weight;
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
        assert_ok!(Balances::transfer(RuntimeOrigin::signed(1), multi, 5));
        assert_ok!(Balances::transfer(RuntimeOrigin::signed(2), multi, 5));
        assert_ok!(Balances::transfer(RuntimeOrigin::signed(3), multi, 5));

        let call = RuntimeCall::Balances(BalancesCall::transfer { dest: 6, value: 15 });
        let call_weight = call.get_dispatch_info().weight;
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
        assert_ok!(Balances::transfer(RuntimeOrigin::signed(1), multi, 5));
        assert_ok!(Balances::transfer(RuntimeOrigin::signed(2), multi, 5));
        assert_ok!(Balances::transfer(RuntimeOrigin::signed(3), multi, 5));

        let call = RuntimeCall::Balances(BalancesCall::transfer { dest: 6, value: 15 });

        let call_weight = call.get_dispatch_info().weight;
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

        let call = RuntimeCall::Balances(BalancesCall::transfer { dest: 6, value: 15 }).encode();
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
        let call = RuntimeCall::Balances(BalancesCall::transfer { dest: 6, value: 15 });
        let call_weight = call.get_dispatch_info().weight;
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
                post_info: Pays::No.into()
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
        assert_ok!(Balances::transfer(RuntimeOrigin::signed(1), multi, 5));

        let boxed_call = Box::new(RuntimeCall::Balances(BalancesCall::transfer {
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
                post_info: Pays::No.into()
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
        assert_ok!(Balances::transfer(RuntimeOrigin::signed(1), multi, 5));
        assert_ok!(Balances::transfer(RuntimeOrigin::signed(2), multi, 5));
        assert_ok!(Balances::transfer(RuntimeOrigin::signed(3), multi, 5));

        let call = RuntimeCall::Balances(BalancesCall::transfer { dest: 6, value: 7 }).encode();
        let hash = blake2_256(&call);

        assert_ok!(Multisig::approve_as_multi(
            RuntimeOrigin::signed(2),
            multi,
            Some(now()),
            hash.clone(),
            Weight::zero()
        ));

        let call = RuntimeCall::Balances(BalancesCall::transfer { dest: 6, value: 8 }).encode();
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
                post_info: Pays::No.into()
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
                post_info: Pays::No.into()
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
        assert_ok!(Balances::transfer(RuntimeOrigin::signed(1), multi, 5));
        assert_ok!(Balances::transfer(RuntimeOrigin::signed(2), multi, 5));

        let call = RuntimeCall::Balances(BalancesCall::transfer { dest: 6, value: 10 });
        let call_weight = call.get_dispatch_info().weight;
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
        assert_ok!(Balances::transfer(RuntimeOrigin::signed(1), multi, 5));
        assert_ok!(Balances::transfer(RuntimeOrigin::signed(2), multi, 5));
        assert_ok!(Balances::transfer(RuntimeOrigin::signed(3), multi, 5));

        let call = RuntimeCall::Balances(BalancesCall::transfer { dest: 6, value: 15 });
        let call_weight = call.get_dispatch_info().weight;
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
        assert_ok!(Balances::transfer(RuntimeOrigin::signed(1), multi, 5));
        assert_ok!(Balances::transfer(RuntimeOrigin::signed(2), multi, 5));
        assert_ok!(Balances::transfer(RuntimeOrigin::signed(3), multi, 5));

        let call = RuntimeCall::Balances(BalancesCall::transfer { dest: 6, value: 15 });
        let call_weight = call.get_dispatch_info().weight;
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
fn multisig_signatory_approve_removes_with_the_signatory() {
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
        assert!(operation.approvals.is_empty());
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
        let call_weight = call.get_dispatch_info().weight;
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

        assert_ok!(Balances::transfer(RuntimeOrigin::signed(2), multi, 5));
        assert_ok!(Balances::transfer(RuntimeOrigin::signed(3), multi, 5));
        assert_ok!(Balances::transfer(RuntimeOrigin::signed(4), multi, 5));

        let call = RuntimeCall::Balances(BalancesCall::transfer { dest: 6, value: 15 });
        let call_weight = call.get_dispatch_info().weight;
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
        let call_weight = call.get_dispatch_info().weight;
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

        assert_ok!(Balances::transfer(RuntimeOrigin::signed(2), multi, 5));
        assert_ok!(Balances::transfer(RuntimeOrigin::signed(3), multi, 5));
        assert_ok!(Balances::transfer(RuntimeOrigin::signed(4), multi, 5));

        let call = RuntimeCall::Balances(BalancesCall::transfer { dest: 6, value: 15 });
        let call_weight = call.get_dispatch_info().weight;
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

        let call = RuntimeCall::Balances(BalancesCall::transfer { dest: 6, value: 15 }).encode();
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

        let call = RuntimeCall::Balances(BalancesCall::transfer { dest: 6, value: 15 }).encode();
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

        let call = RuntimeCall::Balances(BalancesCall::transfer { dest: 6, value: 15 }).encode();
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

        assert_ok!(Balances::transfer(RuntimeOrigin::signed(1), multi, 5));
        assert_ok!(Balances::transfer(RuntimeOrigin::signed(2), multi, 5));
        assert_ok!(Balances::transfer(RuntimeOrigin::signed(3), multi, 5));

        let call = RuntimeCall::Balances(BalancesCall::transfer { dest: 6, value: 15 });
        let call_weight = call.get_dispatch_info().weight;
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

        assert_ok!(Balances::transfer(RuntimeOrigin::signed(1), multi, 5));
        assert_ok!(Balances::transfer(RuntimeOrigin::signed(2), multi, 5));
        assert_ok!(Balances::transfer(RuntimeOrigin::signed(3), multi, 5));

        let call1 = RuntimeCall::Balances(BalancesCall::transfer { dest: 6, value: 10 });
        let call1_weight = call1.get_dispatch_info().weight;
        let data1 = call1.encode();
        let call2 = RuntimeCall::Balances(BalancesCall::transfer { dest: 7, value: 5 });
        let call2_weight = call2.get_dispatch_info().weight;
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

        assert_ok!(Balances::transfer(RuntimeOrigin::signed(1), multi, 5));
        assert_ok!(Balances::transfer(RuntimeOrigin::signed(2), multi, 5));
        assert_ok!(Balances::transfer(RuntimeOrigin::signed(3), multi, 5));

        let call = RuntimeCall::Balances(BalancesCall::transfer { dest: 6, value: 10 });
        let call_weight = call.get_dispatch_info().weight;
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
                post_info: Pays::No.into()
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

        let call = RuntimeCall::Balances(BalancesCall::transfer { dest: 6, value: 15 }).encode();
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
                post_info: Pays::No.into()
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
                post_info: Pays::No.into()
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
        assert_err!(
            Multisig::as_multi_threshold_1(RuntimeOrigin::signed(1), multi, call.clone(), now()),
            DispatchErrorWithPostInfo {
                error: DispatchError::Module(ModuleError {
                    index: 0,
                    error: 5i32.to_le_bytes(),
                    message: Some("CallFiltered")
                }),
                post_info: Pays::No.into()
            }
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

        assert_ok!(Balances::transfer(RuntimeOrigin::signed(1), multi, 5));
        assert_ok!(Balances::transfer(RuntimeOrigin::signed(2), multi, 5));
        assert_ok!(Balances::transfer(RuntimeOrigin::signed(3), multi, 5));

        let call = RuntimeCall::Balances(BalancesCall::transfer { dest: 6, value: 15 });
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
                post_info: Pays::No.into()
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

        assert_ok!(Balances::transfer(RuntimeOrigin::signed(1), multi, 5));
        assert_ok!(Balances::transfer(RuntimeOrigin::signed(2), multi, 5));
        assert_ok!(Balances::transfer(RuntimeOrigin::signed(3), multi, 5));

        let call = RuntimeCall::Balances(BalancesCall::transfer { dest: 6, value: 15 });
        let call_weight = call.get_dispatch_info().weight;
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
fn executes_call_on_peer_remove() {
    new_test_ext().execute_with(|| {
        let multi = Multisig::multi_account_id(&1, 1, 0);
        assert_ok!(Multisig::register_multisig(
            RuntimeOrigin::signed(1),
            vec![1, 2, 3],
        ));

        let call = RuntimeCall::Balances(BalancesCall::transfer { dest: 6, value: 15 }).encode();
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
        assert!(!crate::DispatchedCalls::<Test>::contains_key(
            hash, timepoint
        ));
        assert_ok!(Multisig::remove_signatory(RuntimeOrigin::signed(multi), 3));
        assert!(crate::DispatchedCalls::<Test>::contains_key(
            hash, timepoint
        ));
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

        let call1 = RuntimeCall::Balances(BalancesCall::transfer { dest: 6, value: 15 });
        let call_weight = call1.get_dispatch_info().weight;
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
        assert!(!crate::DispatchedCalls::<Test>::contains_key(
            hash, timepoint
        ));
        assert!(crate::Multisigs::<Test>::contains_key(multi, hash));
        assert_ok!(Multisig::as_multi(
            RuntimeOrigin::signed(2),
            multi,
            Some(timepoint),
            call,
            false,
            call_weight
        ));
        assert!(crate::DispatchedCalls::<Test>::contains_key(
            hash, timepoint
        ));
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

        let call = RuntimeCall::Balances(BalancesCall::transfer { dest: 6, value: 15 }).encode();
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
        assert!(!crate::DispatchedCalls::<Test>::contains_key(
            hash, timepoint
        ));
        assert_ok!(Multisig::remove_signatory(RuntimeOrigin::signed(multi), 3));
        assert!(!crate::DispatchedCalls::<Test>::contains_key(
            hash, timepoint
        ));
        assert!(crate::Multisigs::<Test>::contains_key(multi, hash));
    });
}
