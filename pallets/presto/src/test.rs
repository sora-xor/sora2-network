// This file is part of the SORA network and Polkaswap app.

// Copyright (c) 2020, 2021, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

// Redistribution and use in source and binary forms, with or without modification,
// are permitted provided that the following conditions are met:

// Redistributions of source code must retain the above copyright notice, this list
// of conditions and the following disclaimer.
// Redistributions in binary form must reproduce the above copyright notice, this
// list of conditions and the following disclaimer in the documentation and/or other
// materials provided with the distribution.
//
// All advertising materials mentioning features or use of this software must display
// the following acknowledgement: This product includes software developed by Polka Biome
// Ltd., SORA, and Polkaswap.
//
// Neither the name of the Polka Biome Ltd. nor the names of its contributors may be used
// to endorse or promote products derived from this software without specific prior written permission.

// THIS SOFTWARE IS PROVIDED BY Polka Biome Ltd. AS IS AND ANY EXPRESS OR IMPLIED WARRANTIES,
// INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL Polka Biome Ltd. BE LIABLE FOR ANY
// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
// BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS;
// OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
// USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

#![cfg(feature = "wip")] // presto

use super::*;

use crate::mock::{
    ext, AccountId, AssetId, PrestoBufferTechAccountId, PrestoTechAccountId, Runtime,
    RuntimeOrigin, TechAccountId,
};
use crate::requests::{DepositRequest, Request, RequestStatus, WithdrawRequest};
use common::{balance, AssetInfoProvider, Balance, BoundedString, PRUSD};
use frame_support::{assert_err, assert_ok};
use sp_runtime::DispatchError::BadOrigin;

type PrestoPallet = Pallet<Runtime>;
type E = Error<Runtime>;

fn alice() -> AccountId {
    AccountId::from([1u8; 32])
}
fn bob() -> AccountId {
    AccountId::from([2u8; 32])
}
fn charlie() -> AccountId {
    AccountId::from([3u8; 32])
}
fn dave() -> AccountId {
    AccountId::from([4u8; 32])
}

fn free_balance(asset: &AssetId, account: &AccountId) -> Balance {
    assets::Pallet::<Runtime>::free_balance(asset, account).unwrap()
}

fn tech_account_id_to_account_id(tech: &TechAccountId) -> AccountId {
    technical::Pallet::<Runtime>::tech_account_id_to_account_id(tech).unwrap()
}

#[test]
fn should_add_manager() {
    ext().execute_with(|| {
        assert_eq!(PrestoPallet::managers(), vec![]);

        assert_err!(
            PrestoPallet::add_presto_manager(RuntimeOrigin::signed(bob()), alice()),
            BadOrigin
        );

        assert_ok!(PrestoPallet::add_presto_manager(
            RuntimeOrigin::root(),
            alice()
        ));

        assert_eq!(PrestoPallet::managers(), vec![alice()]);

        assert_err!(
            PrestoPallet::add_presto_manager(RuntimeOrigin::root(), alice()),
            E::ManagerAlreadyAdded
        );
    });
}

#[test]
fn should_remove_manager() {
    ext().execute_with(|| {
        // prepare
        assert_ok!(PrestoPallet::add_presto_manager(
            RuntimeOrigin::root(),
            alice()
        ));
        assert_ok!(PrestoPallet::add_presto_manager(
            RuntimeOrigin::root(),
            bob()
        ));

        assert_eq!(PrestoPallet::managers(), vec![alice(), bob()]);

        // test

        assert_err!(
            PrestoPallet::remove_presto_manager(RuntimeOrigin::signed(dave()), charlie()),
            BadOrigin
        );

        assert_err!(
            PrestoPallet::remove_presto_manager(RuntimeOrigin::root(), charlie()),
            E::ManagerNotExists
        );

        assert_ok!(PrestoPallet::remove_presto_manager(
            RuntimeOrigin::root(),
            alice()
        ));

        assert_eq!(PrestoPallet::managers(), vec![bob()]);

        assert_err!(
            PrestoPallet::remove_presto_manager(RuntimeOrigin::root(), alice()),
            E::ManagerNotExists
        );
    });
}

#[test]
fn should_add_auditor() {
    ext().execute_with(|| {
        assert_eq!(PrestoPallet::auditors(), vec![]);

        assert_err!(
            PrestoPallet::add_presto_auditor(RuntimeOrigin::signed(bob()), alice()),
            BadOrigin
        );

        assert_ok!(PrestoPallet::add_presto_auditor(
            RuntimeOrigin::root(),
            alice()
        ));

        assert_eq!(PrestoPallet::auditors(), vec![alice()]);

        assert_err!(
            PrestoPallet::add_presto_auditor(RuntimeOrigin::root(), alice()),
            E::AuditorAlreadyAdded
        );
    });
}

#[test]
fn should_remove_auditor() {
    ext().execute_with(|| {
        // prepare

        assert_ok!(PrestoPallet::add_presto_auditor(
            RuntimeOrigin::root(),
            alice()
        ));
        assert_ok!(PrestoPallet::add_presto_auditor(
            RuntimeOrigin::root(),
            bob()
        ));

        assert_eq!(PrestoPallet::auditors(), vec![alice(), bob()]);

        // test

        assert_err!(
            PrestoPallet::remove_presto_auditor(RuntimeOrigin::signed(dave()), charlie()),
            BadOrigin
        );

        assert_err!(
            PrestoPallet::remove_presto_auditor(RuntimeOrigin::root(), charlie()),
            E::AuditorNotExists
        );

        assert_ok!(PrestoPallet::remove_presto_auditor(
            RuntimeOrigin::root(),
            alice()
        ));

        assert_eq!(PrestoPallet::auditors(), vec![bob()]);

        assert_err!(
            PrestoPallet::remove_presto_auditor(RuntimeOrigin::root(), alice()),
            E::AuditorNotExists
        );
    });
}

#[test]
fn should_mint_presto_usd() {
    ext().execute_with(|| {
        // prepare

        let main_tech_account = tech_account_id_to_account_id(&PrestoTechAccountId::get());
        let amount = balance!(1000);

        assert_eq!(free_balance(&PRUSD, &main_tech_account), balance!(0));

        // test

        assert_err!(
            PrestoPallet::mint_presto_usd(RuntimeOrigin::signed(alice()), amount),
            E::CallerIsNotManager
        );

        assert_ok!(PrestoPallet::add_presto_manager(
            RuntimeOrigin::root(),
            alice()
        ));

        assert_ok!(PrestoPallet::mint_presto_usd(
            RuntimeOrigin::signed(alice()),
            amount
        ));

        assert_eq!(free_balance(&PRUSD, &main_tech_account), amount);
    });
}

#[test]
fn should_burn_presto_usd() {
    ext().execute_with(|| {
        // prepare

        let main_tech_account = tech_account_id_to_account_id(&PrestoTechAccountId::get());

        assert_ok!(PrestoPallet::add_presto_manager(
            RuntimeOrigin::root(),
            alice()
        ));

        assert_ok!(PrestoPallet::mint_presto_usd(
            RuntimeOrigin::signed(alice()),
            balance!(1000)
        ));

        assert_eq!(free_balance(&PRUSD, &main_tech_account), balance!(1000));

        // test

        assert_err!(
            PrestoPallet::burn_presto_usd(RuntimeOrigin::signed(bob()), balance!(200)),
            E::CallerIsNotManager
        );

        assert_ok!(PrestoPallet::burn_presto_usd(
            RuntimeOrigin::signed(alice()),
            balance!(200)
        ));

        assert_eq!(free_balance(&PRUSD, &main_tech_account), balance!(800));
    });
}

#[test]
fn should_send_presto_usd() {
    ext().execute_with(|| {
        // prepare

        let main_tech_account = tech_account_id_to_account_id(&PrestoTechAccountId::get());

        assert_ok!(PrestoPallet::add_presto_manager(
            RuntimeOrigin::root(),
            alice()
        ));

        assert_ok!(PrestoPallet::mint_presto_usd(
            RuntimeOrigin::signed(alice()),
            balance!(1000)
        ));

        assert_eq!(free_balance(&PRUSD, &main_tech_account), balance!(1000));
        assert_eq!(free_balance(&PRUSD, &dave()), balance!(0));

        // test

        assert_err!(
            PrestoPallet::send_presto_usd(RuntimeOrigin::signed(bob()), balance!(200), dave()),
            E::CallerIsNotManager
        );

        assert_ok!(PrestoPallet::send_presto_usd(
            RuntimeOrigin::signed(alice()),
            balance!(200),
            dave()
        ));

        assert_eq!(free_balance(&PRUSD, &main_tech_account), balance!(800));
        assert_eq!(free_balance(&PRUSD, &dave()), balance!(200));
    });
}

#[test]
fn should_create_deposit_request() {
    ext().execute_with(|| {
        // prepare

        assert_ok!(PrestoPallet::add_presto_manager(
            RuntimeOrigin::root(),
            alice()
        ));

        // test

        assert_eq!(PrestoPallet::requests(1), None);
        assert_eq!(PrestoPallet::requests(2), None);

        assert_ok!(PrestoPallet::create_deposit_request(
            RuntimeOrigin::signed(bob()),
            balance!(100),
            BoundedString::truncate_from("payment reference"),
            Some(BoundedString::truncate_from("details"))
        ));

        assert_eq!(
            PrestoPallet::requests(1).unwrap(),
            Request::Deposit(DepositRequest {
                owner: bob(),
                time: 0,
                amount: balance!(100),
                payment_reference: BoundedString::truncate_from("payment reference"),
                details: Some(BoundedString::truncate_from("details")),
                status: RequestStatus::Pending
            })
        );

        assert_ok!(PrestoPallet::create_deposit_request(
            RuntimeOrigin::signed(charlie()),
            balance!(200),
            BoundedString::truncate_from("payment reference"),
            None
        ));

        assert_eq!(
            PrestoPallet::requests(2).unwrap(),
            Request::Deposit(DepositRequest {
                owner: charlie(),
                time: 0,
                amount: balance!(200),
                payment_reference: BoundedString::truncate_from("payment reference"),
                details: None,
                status: RequestStatus::Pending
            })
        );
    });
}

#[test]
fn should_create_withdraw_request() {
    ext().execute_with(|| {
        // prepare

        let main_tech_account = tech_account_id_to_account_id(&PrestoTechAccountId::get());
        let buffer_tech_account = tech_account_id_to_account_id(&PrestoBufferTechAccountId::get());

        assert_ok!(PrestoPallet::add_presto_manager(
            RuntimeOrigin::root(),
            alice()
        ));

        assert_ok!(PrestoPallet::mint_presto_usd(
            RuntimeOrigin::signed(alice()),
            balance!(1000)
        ));

        assert_ok!(PrestoPallet::send_presto_usd(
            RuntimeOrigin::signed(alice()),
            balance!(700),
            bob()
        ));

        assert_eq!(free_balance(&PRUSD, &main_tech_account), balance!(300));
        assert_eq!(free_balance(&PRUSD, &buffer_tech_account), balance!(0));
        assert_eq!(free_balance(&PRUSD, &bob()), balance!(700));

        // test

        assert_eq!(PrestoPallet::requests(1), None);

        assert_ok!(PrestoPallet::create_withdraw_request(
            RuntimeOrigin::signed(bob()),
            balance!(200),
            Some(BoundedString::truncate_from("details"))
        ));

        assert_eq!(
            PrestoPallet::requests(1).unwrap(),
            Request::Withdraw(WithdrawRequest {
                owner: bob(),
                time: 0,
                amount: balance!(200),
                payment_reference: None,
                details: Some(BoundedString::truncate_from("details")),
                status: RequestStatus::Pending
            })
        );

        assert_eq!(free_balance(&PRUSD, &main_tech_account), balance!(300));
        assert_eq!(free_balance(&PRUSD, &buffer_tech_account), balance!(200));
        assert_eq!(free_balance(&PRUSD, &bob()), balance!(500));
    });
}

#[test]
fn should_cancel_request() {
    ext().execute_with(|| {
        // prepare

        let main_tech_account = tech_account_id_to_account_id(&PrestoTechAccountId::get());
        let buffer_tech_account = tech_account_id_to_account_id(&PrestoBufferTechAccountId::get());

        assert_ok!(PrestoPallet::add_presto_manager(
            RuntimeOrigin::root(),
            alice()
        ));

        assert_ok!(PrestoPallet::mint_presto_usd(
            RuntimeOrigin::signed(alice()),
            balance!(1000)
        ));

        assert_ok!(PrestoPallet::send_presto_usd(
            RuntimeOrigin::signed(alice()),
            balance!(300),
            bob()
        ));

        assert_ok!(PrestoPallet::send_presto_usd(
            RuntimeOrigin::signed(alice()),
            balance!(200),
            charlie()
        ));

        assert_eq!(free_balance(&PRUSD, &main_tech_account), balance!(500));
        assert_eq!(free_balance(&PRUSD, &buffer_tech_account), balance!(0));
        assert_eq!(free_balance(&PRUSD, &bob()), balance!(300));
        assert_eq!(free_balance(&PRUSD, &charlie()), balance!(200));

        assert_ok!(PrestoPallet::create_deposit_request(
            RuntimeOrigin::signed(bob()),
            balance!(100),
            BoundedString::truncate_from("payment reference"),
            Some(BoundedString::truncate_from("details1"))
        ));

        assert_ok!(PrestoPallet::create_withdraw_request(
            RuntimeOrigin::signed(charlie()),
            balance!(50),
            Some(BoundedString::truncate_from("details2"))
        ));

        assert_eq!(free_balance(&PRUSD, &main_tech_account), balance!(500));
        assert_eq!(free_balance(&PRUSD, &buffer_tech_account), balance!(50));
        assert_eq!(free_balance(&PRUSD, &bob()), balance!(300));
        assert_eq!(free_balance(&PRUSD, &charlie()), balance!(150));

        assert_eq!(
            *PrestoPallet::requests(1).unwrap().status(),
            RequestStatus::Pending
        );
        assert_eq!(
            *PrestoPallet::requests(2).unwrap().status(),
            RequestStatus::Pending
        );

        // test

        assert_err!(
            PrestoPallet::cancel_request(RuntimeOrigin::signed(dave()), 1),
            E::CallerIsNotRequestOwner
        );
        assert_err!(
            PrestoPallet::cancel_request(RuntimeOrigin::signed(dave()), 2),
            E::CallerIsNotRequestOwner
        );
        assert_err!(
            PrestoPallet::cancel_request(RuntimeOrigin::signed(dave()), 3),
            E::RequestIsNotExists
        );

        assert_ok!(PrestoPallet::cancel_request(
            RuntimeOrigin::signed(bob()),
            1
        ));
        assert_ok!(PrestoPallet::cancel_request(
            RuntimeOrigin::signed(charlie()),
            2
        ));

        assert_eq!(
            *PrestoPallet::requests(1).unwrap().status(),
            RequestStatus::Cancelled
        );
        assert_eq!(
            *PrestoPallet::requests(2).unwrap().status(),
            RequestStatus::Cancelled
        );

        assert_eq!(free_balance(&PRUSD, &main_tech_account), balance!(500));
        assert_eq!(free_balance(&PRUSD, &buffer_tech_account), balance!(0));
        assert_eq!(free_balance(&PRUSD, &bob()), balance!(300));
        assert_eq!(free_balance(&PRUSD, &charlie()), balance!(200));

        assert_err!(
            PrestoPallet::cancel_request(RuntimeOrigin::signed(bob()), 1),
            E::RequestAlreadyProcessed
        );
        assert_err!(
            PrestoPallet::cancel_request(RuntimeOrigin::signed(charlie()), 2),
            E::RequestAlreadyProcessed
        );
    });
}

#[test]
fn should_approve_deposit_request() {
    ext().execute_with(|| {
        // prepare

        let main_tech_account = tech_account_id_to_account_id(&PrestoTechAccountId::get());
        let buffer_tech_account = tech_account_id_to_account_id(&PrestoBufferTechAccountId::get());

        assert_ok!(PrestoPallet::add_presto_manager(
            RuntimeOrigin::root(),
            alice()
        ));

        assert_ok!(PrestoPallet::mint_presto_usd(
            RuntimeOrigin::signed(alice()),
            balance!(1000)
        ));

        assert_ok!(PrestoPallet::send_presto_usd(
            RuntimeOrigin::signed(alice()),
            balance!(300),
            bob()
        ));

        assert_ok!(PrestoPallet::send_presto_usd(
            RuntimeOrigin::signed(alice()),
            balance!(200),
            charlie()
        ));

        assert_eq!(free_balance(&PRUSD, &main_tech_account), balance!(500));
        assert_eq!(free_balance(&PRUSD, &buffer_tech_account), balance!(0));
        assert_eq!(free_balance(&PRUSD, &bob()), balance!(300));
        assert_eq!(free_balance(&PRUSD, &charlie()), balance!(200));

        assert_ok!(PrestoPallet::create_deposit_request(
            RuntimeOrigin::signed(bob()),
            balance!(100),
            BoundedString::truncate_from("payment reference"),
            Some(BoundedString::truncate_from("details1"))
        ));

        assert_ok!(PrestoPallet::create_withdraw_request(
            RuntimeOrigin::signed(charlie()),
            balance!(50),
            Some(BoundedString::truncate_from("details2"))
        ));

        assert_eq!(free_balance(&PRUSD, &main_tech_account), balance!(500));
        assert_eq!(free_balance(&PRUSD, &buffer_tech_account), balance!(50));
        assert_eq!(free_balance(&PRUSD, &bob()), balance!(300));
        assert_eq!(free_balance(&PRUSD, &charlie()), balance!(150));

        assert_eq!(
            *PrestoPallet::requests(1).unwrap().status(),
            RequestStatus::Pending
        );
        assert_eq!(
            *PrestoPallet::requests(2).unwrap().status(),
            RequestStatus::Pending
        );

        // test

        assert_err!(
            PrestoPallet::approve_deposit_request(RuntimeOrigin::signed(dave()), 1),
            E::CallerIsNotManager
        );
        assert_err!(
            PrestoPallet::approve_deposit_request(RuntimeOrigin::signed(dave()), 2),
            E::CallerIsNotManager
        );
        assert_err!(
            PrestoPallet::approve_deposit_request(RuntimeOrigin::signed(alice()), 3),
            E::RequestIsNotExists
        );

        assert_ok!(PrestoPallet::approve_deposit_request(
            RuntimeOrigin::signed(alice()),
            1
        ));
        assert_err!(
            PrestoPallet::approve_deposit_request(RuntimeOrigin::signed(alice()), 2),
            E::WrongRequestType
        );

        assert_eq!(
            *PrestoPallet::requests(1).unwrap().status(),
            RequestStatus::Approved {
                by: alice(),
                time: 0
            }
        );
        assert_eq!(
            *PrestoPallet::requests(2).unwrap().status(),
            RequestStatus::Pending
        );

        assert_eq!(free_balance(&PRUSD, &main_tech_account), balance!(400));
        assert_eq!(free_balance(&PRUSD, &buffer_tech_account), balance!(50));
        assert_eq!(free_balance(&PRUSD, &bob()), balance!(400));
        assert_eq!(free_balance(&PRUSD, &charlie()), balance!(150));

        assert_err!(
            PrestoPallet::approve_deposit_request(RuntimeOrigin::signed(alice()), 1),
            E::RequestAlreadyProcessed
        );
    });
}

#[test]
fn should_approve_withdraw_request() {
    ext().execute_with(|| {
        // prepare

        let main_tech_account = tech_account_id_to_account_id(&PrestoTechAccountId::get());
        let buffer_tech_account = tech_account_id_to_account_id(&PrestoBufferTechAccountId::get());

        assert_ok!(PrestoPallet::add_presto_manager(
            RuntimeOrigin::root(),
            alice()
        ));

        assert_ok!(PrestoPallet::mint_presto_usd(
            RuntimeOrigin::signed(alice()),
            balance!(1000)
        ));

        assert_ok!(PrestoPallet::send_presto_usd(
            RuntimeOrigin::signed(alice()),
            balance!(300),
            bob()
        ));

        assert_ok!(PrestoPallet::send_presto_usd(
            RuntimeOrigin::signed(alice()),
            balance!(200),
            charlie()
        ));

        assert_eq!(free_balance(&PRUSD, &main_tech_account), balance!(500));
        assert_eq!(free_balance(&PRUSD, &buffer_tech_account), balance!(0));
        assert_eq!(free_balance(&PRUSD, &bob()), balance!(300));
        assert_eq!(free_balance(&PRUSD, &charlie()), balance!(200));

        assert_ok!(PrestoPallet::create_deposit_request(
            RuntimeOrigin::signed(bob()),
            balance!(100),
            BoundedString::truncate_from("payment reference"),
            Some(BoundedString::truncate_from("details1"))
        ));

        assert_ok!(PrestoPallet::create_withdraw_request(
            RuntimeOrigin::signed(charlie()),
            balance!(50),
            Some(BoundedString::truncate_from("details2"))
        ));

        assert_eq!(free_balance(&PRUSD, &main_tech_account), balance!(500));
        assert_eq!(free_balance(&PRUSD, &buffer_tech_account), balance!(50));
        assert_eq!(free_balance(&PRUSD, &bob()), balance!(300));
        assert_eq!(free_balance(&PRUSD, &charlie()), balance!(150));

        assert_eq!(
            *PrestoPallet::requests(1).unwrap().status(),
            RequestStatus::Pending
        );
        assert_eq!(
            *PrestoPallet::requests(2).unwrap().status(),
            RequestStatus::Pending
        );

        // test

        assert_err!(
            PrestoPallet::approve_withdraw_request(
                RuntimeOrigin::signed(dave()),
                1,
                BoundedString::truncate_from("payment reference")
            ),
            E::CallerIsNotManager
        );
        assert_err!(
            PrestoPallet::approve_withdraw_request(
                RuntimeOrigin::signed(dave()),
                2,
                BoundedString::truncate_from("payment reference")
            ),
            E::CallerIsNotManager
        );
        assert_err!(
            PrestoPallet::approve_withdraw_request(
                RuntimeOrigin::signed(alice()),
                3,
                BoundedString::truncate_from("payment reference")
            ),
            E::RequestIsNotExists
        );

        assert_err!(
            PrestoPallet::approve_withdraw_request(
                RuntimeOrigin::signed(alice()),
                1,
                BoundedString::truncate_from("payment reference")
            ),
            E::WrongRequestType
        );
        assert_ok!(PrestoPallet::approve_withdraw_request(
            RuntimeOrigin::signed(alice()),
            2,
            BoundedString::truncate_from("payment reference")
        ));

        assert_eq!(
            *PrestoPallet::requests(1).unwrap().status(),
            RequestStatus::Pending
        );
        assert_eq!(
            *PrestoPallet::requests(2).unwrap().status(),
            RequestStatus::Approved {
                by: alice(),
                time: 0
            }
        );

        assert_eq!(free_balance(&PRUSD, &main_tech_account), balance!(550));
        assert_eq!(free_balance(&PRUSD, &buffer_tech_account), balance!(0));
        assert_eq!(free_balance(&PRUSD, &bob()), balance!(300));
        assert_eq!(free_balance(&PRUSD, &charlie()), balance!(150));

        assert_err!(
            PrestoPallet::approve_withdraw_request(
                RuntimeOrigin::signed(alice()),
                2,
                BoundedString::truncate_from("payment reference")
            ),
            E::RequestAlreadyProcessed
        );
    });
}

#[test]
fn should_decline_request() {
    ext().execute_with(|| {
        // prepare

        let main_tech_account = tech_account_id_to_account_id(&PrestoTechAccountId::get());
        let buffer_tech_account = tech_account_id_to_account_id(&PrestoBufferTechAccountId::get());

        assert_ok!(PrestoPallet::add_presto_manager(
            RuntimeOrigin::root(),
            alice()
        ));

        assert_ok!(PrestoPallet::mint_presto_usd(
            RuntimeOrigin::signed(alice()),
            balance!(1000)
        ));

        assert_ok!(PrestoPallet::send_presto_usd(
            RuntimeOrigin::signed(alice()),
            balance!(300),
            bob()
        ));

        assert_ok!(PrestoPallet::send_presto_usd(
            RuntimeOrigin::signed(alice()),
            balance!(200),
            charlie()
        ));

        assert_eq!(free_balance(&PRUSD, &main_tech_account), balance!(500));
        assert_eq!(free_balance(&PRUSD, &buffer_tech_account), balance!(0));
        assert_eq!(free_balance(&PRUSD, &bob()), balance!(300));
        assert_eq!(free_balance(&PRUSD, &charlie()), balance!(200));

        assert_ok!(PrestoPallet::create_deposit_request(
            RuntimeOrigin::signed(bob()),
            balance!(100),
            BoundedString::truncate_from("payment reference"),
            Some(BoundedString::truncate_from("details1"))
        ));

        assert_ok!(PrestoPallet::create_withdraw_request(
            RuntimeOrigin::signed(charlie()),
            balance!(50),
            Some(BoundedString::truncate_from("details2"))
        ));

        assert_eq!(free_balance(&PRUSD, &main_tech_account), balance!(500));
        assert_eq!(free_balance(&PRUSD, &buffer_tech_account), balance!(50));
        assert_eq!(free_balance(&PRUSD, &bob()), balance!(300));
        assert_eq!(free_balance(&PRUSD, &charlie()), balance!(150));

        assert_eq!(
            *PrestoPallet::requests(1).unwrap().status(),
            RequestStatus::Pending
        );
        assert_eq!(
            *PrestoPallet::requests(2).unwrap().status(),
            RequestStatus::Pending
        );

        // test

        assert_err!(
            PrestoPallet::decline_request(RuntimeOrigin::signed(dave()), 1),
            E::CallerIsNotManager
        );
        assert_err!(
            PrestoPallet::decline_request(RuntimeOrigin::signed(dave()), 2),
            E::CallerIsNotManager
        );
        assert_err!(
            PrestoPallet::decline_request(RuntimeOrigin::signed(alice()), 3),
            E::RequestIsNotExists
        );

        assert_ok!(PrestoPallet::decline_request(
            RuntimeOrigin::signed(alice()),
            1
        ));
        assert_ok!(PrestoPallet::decline_request(
            RuntimeOrigin::signed(alice()),
            2
        ));

        assert_eq!(
            *PrestoPallet::requests(1).unwrap().status(),
            RequestStatus::Declined {
                by: alice(),
                time: 0
            }
        );
        assert_eq!(
            *PrestoPallet::requests(2).unwrap().status(),
            RequestStatus::Declined {
                by: alice(),
                time: 0
            }
        );

        assert_eq!(free_balance(&PRUSD, &main_tech_account), balance!(500));
        assert_eq!(free_balance(&PRUSD, &buffer_tech_account), balance!(0));
        assert_eq!(free_balance(&PRUSD, &bob()), balance!(300));
        assert_eq!(free_balance(&PRUSD, &charlie()), balance!(200));

        assert_err!(
            PrestoPallet::decline_request(RuntimeOrigin::signed(alice()), 1),
            E::RequestAlreadyProcessed
        );
        assert_err!(
            PrestoPallet::decline_request(RuntimeOrigin::signed(alice()), 2),
            E::RequestAlreadyProcessed
        );
    });
}