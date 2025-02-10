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

use crate::crop_receipt::{
    crop_receipt_content_template, Country, CropReceipt, CropReceiptContent, Rating, Score, Status,
};
use crate::mock::{
    ext, AccountId, AssetId, PrestoBufferTechAccountId, PrestoTechAccountId, Runtime,
    RuntimeOrigin, TechAccountId,
};
use crate::requests::{DepositRequest, Request, RequestStatus, WithdrawRequest};

use common::prelude::BalanceUnit;
use common::{
    balance, AssetIdOf, AssetInfoProvider, AssetName, AssetSymbol, Balance, BoundedString, DEXId,
    OrderBookId, PRUSD, SBT_PRACS, SBT_PRCRDT, SBT_PRINVST,
};
use frame_support::sp_runtime::Permill;
use frame_support::{assert_err, assert_ok};
use sp_runtime::DispatchError::BadOrigin;
use sp_std::collections::btree_set::BTreeSet;

type PrestoPallet = Pallet<Runtime>;
type OrderBookPallet = order_book::Pallet<Runtime>;
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

fn burn_balance(asset: &AssetId, issuer: &AccountId, account: &AccountId, amount: Balance) {
    assets::Pallet::<Runtime>::burn_from(asset, issuer, account, amount).unwrap()
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

        assert_err!(
            PrestoPallet::mint_presto_usd(RuntimeOrigin::signed(alice()), balance!(0)),
            E::AmountIsZero
        );

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

        assert_err!(
            PrestoPallet::burn_presto_usd(RuntimeOrigin::signed(alice()), balance!(0)),
            E::AmountIsZero
        );

        assert_ok!(PrestoPallet::burn_presto_usd(
            RuntimeOrigin::signed(alice()),
            balance!(200)
        ));

        assert_eq!(free_balance(&PRUSD, &main_tech_account), balance!(800));
    });
}

#[test]
fn should_apply_investor_kyc() {
    ext().execute_with(|| {
        // prepare

        assert_ok!(PrestoPallet::add_presto_manager(
            RuntimeOrigin::root(),
            alice()
        ));

        assert_eq!(free_balance(&SBT_PRACS.into(), &bob()), balance!(0));
        assert_eq!(free_balance(&SBT_PRINVST.into(), &bob()), balance!(0));
        assert_eq!(free_balance(&SBT_PRCRDT.into(), &bob()), balance!(0));

        // test

        assert_err!(
            PrestoPallet::apply_investor_kyc(RuntimeOrigin::signed(charlie()), bob()),
            E::CallerIsNotManager
        );

        assert_ok!(PrestoPallet::apply_investor_kyc(
            RuntimeOrigin::signed(alice()),
            bob()
        ));

        assert_eq!(free_balance(&SBT_PRACS.into(), &bob()), 1);
        assert_eq!(free_balance(&SBT_PRINVST.into(), &bob()), 1);
        assert_eq!(free_balance(&SBT_PRCRDT.into(), &bob()), balance!(0));

        assert_err!(
            PrestoPallet::apply_investor_kyc(RuntimeOrigin::signed(alice()), bob()),
            E::KycAlreadyPassed
        );
    });
}

#[test]
fn should_apply_creditor_kyc() {
    ext().execute_with(|| {
        // prepare

        assert_ok!(PrestoPallet::add_presto_manager(
            RuntimeOrigin::root(),
            alice()
        ));

        assert_eq!(free_balance(&SBT_PRACS.into(), &bob()), balance!(0));
        assert_eq!(free_balance(&SBT_PRINVST.into(), &bob()), balance!(0));
        assert_eq!(free_balance(&SBT_PRCRDT.into(), &bob()), balance!(0));

        // test

        assert_err!(
            PrestoPallet::apply_creditor_kyc(RuntimeOrigin::signed(charlie()), bob()),
            E::CallerIsNotManager
        );

        assert_ok!(PrestoPallet::apply_creditor_kyc(
            RuntimeOrigin::signed(alice()),
            bob()
        ));

        assert_eq!(free_balance(&SBT_PRACS.into(), &bob()), 1);
        assert_eq!(free_balance(&SBT_PRINVST.into(), &bob()), balance!(0));
        assert_eq!(free_balance(&SBT_PRCRDT.into(), &bob()), 1);

        assert_err!(
            PrestoPallet::apply_creditor_kyc(RuntimeOrigin::signed(alice()), bob()),
            E::KycAlreadyPassed
        );
    });
}

#[test]
fn should_remove_investor_kyc() {
    ext().execute_with(|| {
        // prepare

        assert_ok!(PrestoPallet::add_presto_manager(
            RuntimeOrigin::root(),
            alice()
        ));

        assert_ok!(PrestoPallet::apply_investor_kyc(
            RuntimeOrigin::signed(alice()),
            bob()
        ));

        assert_ok!(PrestoPallet::mint_presto_usd(
            RuntimeOrigin::signed(alice()),
            balance!(1000)
        ));

        assert_ok!(PrestoPallet::send_presto_usd(
            RuntimeOrigin::signed(alice()),
            balance!(200),
            bob()
        ));

        // test

        assert_err!(
            PrestoPallet::remove_investor_kyc(RuntimeOrigin::signed(charlie()), bob()),
            E::CallerIsNotManager
        );

        assert_err!(
            PrestoPallet::remove_investor_kyc(RuntimeOrigin::signed(alice()), bob()),
            E::AccountHasPrestoAssets
        );

        let main_tech_account = tech_account_id_to_account_id(&PrestoTechAccountId::get());
        burn_balance(&PRUSD, &main_tech_account, &bob(), balance!(200));

        assert_ok!(PrestoPallet::remove_investor_kyc(
            RuntimeOrigin::signed(alice()),
            bob()
        ));

        assert_eq!(free_balance(&SBT_PRACS.into(), &bob()), balance!(0));
        assert_eq!(free_balance(&SBT_PRINVST.into(), &bob()), balance!(0));
        assert_eq!(free_balance(&SBT_PRCRDT.into(), &bob()), balance!(0));

        assert_err!(
            PrestoPallet::remove_investor_kyc(RuntimeOrigin::signed(alice()), bob()),
            E::KycNotPassed
        );
    });
}

#[test]
fn should_remove_creditor_kyc() {
    ext().execute_with(|| {
        // prepare

        assert_ok!(PrestoPallet::add_presto_manager(
            RuntimeOrigin::root(),
            alice()
        ));

        assert_ok!(PrestoPallet::apply_creditor_kyc(
            RuntimeOrigin::signed(alice()),
            bob()
        ));

        assert_ok!(PrestoPallet::mint_presto_usd(
            RuntimeOrigin::signed(alice()),
            balance!(1000)
        ));

        assert_ok!(PrestoPallet::send_presto_usd(
            RuntimeOrigin::signed(alice()),
            balance!(200),
            bob()
        ));

        // test

        assert_err!(
            PrestoPallet::remove_creditor_kyc(RuntimeOrigin::signed(charlie()), bob()),
            E::CallerIsNotManager
        );

        assert_err!(
            PrestoPallet::remove_creditor_kyc(RuntimeOrigin::signed(alice()), bob()),
            E::AccountHasPrestoAssets
        );

        let main_tech_account = tech_account_id_to_account_id(&PrestoTechAccountId::get());
        burn_balance(&PRUSD, &main_tech_account, &bob(), balance!(200));

        assert_ok!(PrestoPallet::remove_creditor_kyc(
            RuntimeOrigin::signed(alice()),
            bob()
        ));

        assert_eq!(free_balance(&SBT_PRACS.into(), &bob()), balance!(0));
        assert_eq!(free_balance(&SBT_PRINVST.into(), &bob()), balance!(0));
        assert_eq!(free_balance(&SBT_PRCRDT.into(), &bob()), balance!(0));

        assert_err!(
            PrestoPallet::remove_creditor_kyc(RuntimeOrigin::signed(alice()), bob()),
            E::KycNotPassed
        );
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

        assert_ok!(PrestoPallet::apply_investor_kyc(
            RuntimeOrigin::signed(alice()),
            dave()
        ));

        assert_eq!(free_balance(&PRUSD, &main_tech_account), balance!(1000));
        assert_eq!(free_balance(&PRUSD, &dave()), balance!(0));

        // test

        assert_err!(
            PrestoPallet::send_presto_usd(RuntimeOrigin::signed(bob()), balance!(200), dave()),
            E::CallerIsNotManager
        );

        assert_err!(
            PrestoPallet::send_presto_usd(RuntimeOrigin::signed(alice()), balance!(0), dave()),
            E::AmountIsZero
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

        assert_ok!(PrestoPallet::apply_investor_kyc(
            RuntimeOrigin::signed(alice()),
            bob()
        ));

        // test

        assert_eq!(PrestoPallet::requests(1), None);
        assert_eq!(PrestoPallet::requests(2), None);

        assert_err!(
            PrestoPallet::create_deposit_request(
                RuntimeOrigin::signed(bob()),
                balance!(0),
                BoundedString::truncate_from("payment reference"),
                Some(BoundedString::truncate_from("details"))
            ),
            E::AmountIsZero
        );

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

        assert_err!(
            PrestoPallet::create_deposit_request(
                RuntimeOrigin::signed(charlie()),
                balance!(200),
                BoundedString::truncate_from("payment reference"),
                None
            ),
            E::KycNotPassed
        );

        assert_ok!(PrestoPallet::apply_investor_kyc(
            RuntimeOrigin::signed(alice()),
            charlie()
        ));

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

        assert_ok!(PrestoPallet::apply_investor_kyc(
            RuntimeOrigin::signed(alice()),
            bob()
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

        assert_err!(
            PrestoPallet::create_withdraw_request(
                RuntimeOrigin::signed(bob()),
                balance!(0),
                Some(BoundedString::truncate_from("details"))
            ),
            E::AmountIsZero
        );

        assert_err!(
            PrestoPallet::create_withdraw_request(
                RuntimeOrigin::signed(charlie()),
                balance!(200),
                Some(BoundedString::truncate_from("details"))
            ),
            E::KycNotPassed
        );

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

        assert_ok!(PrestoPallet::apply_investor_kyc(
            RuntimeOrigin::signed(alice()),
            bob()
        ));

        assert_ok!(PrestoPallet::apply_investor_kyc(
            RuntimeOrigin::signed(alice()),
            charlie()
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
            E::KycNotPassed
        );

        assert_ok!(PrestoPallet::apply_investor_kyc(
            RuntimeOrigin::signed(alice()),
            dave()
        ));

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

        assert_ok!(PrestoPallet::apply_investor_kyc(
            RuntimeOrigin::signed(alice()),
            bob()
        ));

        assert_ok!(PrestoPallet::apply_investor_kyc(
            RuntimeOrigin::signed(alice()),
            charlie()
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

        assert_ok!(PrestoPallet::apply_investor_kyc(
            RuntimeOrigin::signed(alice()),
            bob()
        ));

        assert_ok!(PrestoPallet::apply_investor_kyc(
            RuntimeOrigin::signed(alice()),
            charlie()
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

        assert_ok!(PrestoPallet::apply_investor_kyc(
            RuntimeOrigin::signed(alice()),
            bob()
        ));

        assert_ok!(PrestoPallet::apply_investor_kyc(
            RuntimeOrigin::signed(alice()),
            charlie()
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

#[test]
fn should_create_crop_receipt() {
    ext().execute_with(|| {
        assert_eq!(PrestoPallet::user_crop_receipts(bob()), vec![]);

        let amount = balance!(10000);
        let profit = Permill::from_percent(5);
        let close_initial_period = 123;
        let date_of_issue = 234;
        let place_of_issue = BoundedString::truncate_from("place of issue");
        let debtor = BoundedString::truncate_from("debtor");
        let creditor = BoundedString::truncate_from("creditor");
        let perfomance_time = 345;
        let data = crop_receipt_content_template::<Runtime>();

        assert_err!(
            PrestoPallet::create_crop_receipt(
                RuntimeOrigin::signed(bob()),
                balance!(0),
                profit,
                Country::Brazil,
                close_initial_period,
                date_of_issue,
                place_of_issue.clone(),
                debtor.clone(),
                creditor.clone(),
                perfomance_time,
                data.clone()
            ),
            E::AmountIsZero
        );

        assert_err!(
            PrestoPallet::create_crop_receipt(
                RuntimeOrigin::signed(bob()),
                amount,
                profit,
                Country::Brazil,
                close_initial_period,
                date_of_issue,
                place_of_issue.clone(),
                debtor.clone(),
                creditor.clone(),
                perfomance_time,
                data.clone()
            ),
            E::CreditorKycNotPassed
        );

        assert_ok!(PrestoPallet::add_presto_manager(
            RuntimeOrigin::root(),
            alice()
        ));

        assert_ok!(PrestoPallet::apply_creditor_kyc(
            RuntimeOrigin::signed(alice()),
            bob()
        ));

        assert_ok!(PrestoPallet::create_crop_receipt(
            RuntimeOrigin::signed(bob()),
            amount,
            profit,
            Country::Brazil,
            close_initial_period,
            date_of_issue,
            place_of_issue.clone(),
            debtor.clone(),
            creditor.clone(),
            perfomance_time,
            data.clone()
        ));

        assert_eq!(
            PrestoPallet::crop_receipts(1).unwrap(),
            CropReceipt::<Runtime> {
                owner: bob(),
                time: 0,
                status: Status::Rating,
                amount,
                profit,
                country: Country::Brazil,
                score: None,
                close_initial_period,
                date_of_issue,
                place_of_issue,
                debtor,
                creditor,
                perfomance_time
            }
        );

        assert_eq!(
            PrestoPallet::crop_receipts_content(1).unwrap(),
            CropReceiptContent::<Runtime> { json: data }
        );

        assert_eq!(PrestoPallet::user_crop_receipts(bob()), vec![1]);
    });
}

#[test]
fn should_rate_crop_receipt() {
    ext().execute_with(|| {
        // prepare

        assert_ok!(PrestoPallet::add_presto_auditor(
            RuntimeOrigin::root(),
            alice()
        ));

        assert_ok!(PrestoPallet::add_presto_manager(
            RuntimeOrigin::root(),
            dave()
        ));

        assert_ok!(PrestoPallet::apply_creditor_kyc(
            RuntimeOrigin::signed(dave()),
            bob()
        ));

        assert_ok!(PrestoPallet::apply_creditor_kyc(
            RuntimeOrigin::signed(dave()),
            charlie()
        ));

        assert_ok!(PrestoPallet::create_crop_receipt(
            RuntimeOrigin::signed(bob()),
            balance!(10000),
            Permill::from_percent(5),
            Country::Brazil,
            100,
            200,
            BoundedString::truncate_from("place of issue"),
            BoundedString::truncate_from("debtor"),
            BoundedString::truncate_from("creditor"),
            300,
            crop_receipt_content_template::<Runtime>()
        ));

        // test

        assert_err!(
            PrestoPallet::rate_crop_receipt(RuntimeOrigin::signed(charlie()), 1, Rating::AA),
            E::CallerIsNotAuditor
        );

        assert_err!(
            PrestoPallet::rate_crop_receipt(RuntimeOrigin::signed(alice()), 2, Rating::AA),
            E::CropReceiptIsNotExists
        );

        assert_ok!(PrestoPallet::rate_crop_receipt(
            RuntimeOrigin::signed(alice()),
            1,
            Rating::AA
        ));

        let crop_receipt = PrestoPallet::crop_receipts(1).unwrap();

        assert_eq!(crop_receipt.status, Status::Decision);

        assert_eq!(
            crop_receipt.score.unwrap(),
            Score {
                rating: Rating::AA,
                by_auditor: alice()
            }
        );

        assert_err!(
            PrestoPallet::rate_crop_receipt(RuntimeOrigin::signed(alice()), 1, Rating::A),
            E::CropReceiptAlreadyRated
        );
    });
}

#[test]
fn should_decline_crop_receipt() {
    ext().execute_with(|| {
        // prepare

        assert_ok!(PrestoPallet::add_presto_auditor(
            RuntimeOrigin::root(),
            alice()
        ));

        assert_ok!(PrestoPallet::add_presto_manager(
            RuntimeOrigin::root(),
            dave()
        ));

        assert_ok!(PrestoPallet::apply_creditor_kyc(
            RuntimeOrigin::signed(dave()),
            bob()
        ));

        assert_ok!(PrestoPallet::apply_creditor_kyc(
            RuntimeOrigin::signed(dave()),
            charlie()
        ));

        assert_ok!(PrestoPallet::create_crop_receipt(
            RuntimeOrigin::signed(bob()),
            balance!(10000),
            Permill::from_percent(5),
            Country::Brazil,
            100,
            200,
            BoundedString::truncate_from("place of issue"),
            BoundedString::truncate_from("debtor"),
            BoundedString::truncate_from("creditor"),
            300,
            crop_receipt_content_template::<Runtime>()
        ));

        // test

        assert_err!(
            PrestoPallet::decline_crop_receipt(RuntimeOrigin::signed(bob()), 2),
            E::CropReceiptIsNotExists
        );

        assert_err!(
            PrestoPallet::decline_crop_receipt(RuntimeOrigin::signed(charlie()), 1),
            E::CallerIsNotCropReceiptOwner
        );

        assert_err!(
            PrestoPallet::decline_crop_receipt(RuntimeOrigin::signed(bob()), 1),
            E::CropReceiptWaitingForRate
        );

        assert_ok!(PrestoPallet::rate_crop_receipt(
            RuntimeOrigin::signed(alice()),
            1,
            Rating::AA
        ));

        assert_ok!(PrestoPallet::decline_crop_receipt(
            RuntimeOrigin::signed(bob()),
            1
        ));

        assert_eq!(
            PrestoPallet::crop_receipts(1).unwrap().status,
            Status::Declined
        );

        assert_err!(
            PrestoPallet::decline_crop_receipt(RuntimeOrigin::signed(bob()), 1),
            E::CropReceiptAlreadyHasDecision
        );
    });
}

#[test]
fn should_publish_crop_receipt() {
    ext().execute_with(|| {
        // prepare

        assert_ok!(PrestoPallet::add_presto_auditor(
            RuntimeOrigin::root(),
            alice()
        ));

        assert_ok!(PrestoPallet::add_presto_manager(
            RuntimeOrigin::root(),
            dave()
        ));

        assert_ok!(PrestoPallet::apply_creditor_kyc(
            RuntimeOrigin::signed(dave()),
            bob()
        ));

        assert_ok!(PrestoPallet::apply_creditor_kyc(
            RuntimeOrigin::signed(dave()),
            charlie()
        ));

        assert_ok!(PrestoPallet::create_crop_receipt(
            RuntimeOrigin::signed(bob()),
            balance!(100000),
            Permill::from_percent(5),
            Country::Brazil,
            100,
            200,
            BoundedString::truncate_from("place of issue"),
            BoundedString::truncate_from("debtor"),
            BoundedString::truncate_from("creditor"),
            300,
            crop_receipt_content_template::<Runtime>()
        ));

        // test

        assert_eq!(
            extended_assets::Pallet::<Runtime>::soulbound_asset(SBT_PRACS.into_predefined())
                .unwrap()
                .regulated_assets,
            BTreeSet::from([PRUSD])
        );
        assert_eq!(
            extended_assets::Pallet::<Runtime>::regulated_asset_to_sbt(PRUSD),
            SBT_PRACS.into()
        );

        let supply = 10000;

        assert_err!(
            PrestoPallet::publish_crop_receipt(RuntimeOrigin::signed(bob()), 1, 0),
            E::AmountIsZero
        );

        assert_err!(
            PrestoPallet::publish_crop_receipt(RuntimeOrigin::signed(bob()), 2, supply),
            E::CropReceiptIsNotExists
        );

        assert_err!(
            PrestoPallet::publish_crop_receipt(RuntimeOrigin::signed(charlie()), 1, supply),
            E::CallerIsNotCropReceiptOwner
        );

        assert_err!(
            PrestoPallet::publish_crop_receipt(RuntimeOrigin::signed(bob()), 1, 1000000),
            E::TooBigCouponSupply
        );

        assert_err!(
            PrestoPallet::publish_crop_receipt(RuntimeOrigin::signed(bob()), 1, supply),
            E::CropReceiptWaitingForRate
        );

        assert_ok!(PrestoPallet::rate_crop_receipt(
            RuntimeOrigin::signed(alice()),
            1,
            Rating::AA
        ));

        assert_ok!(PrestoPallet::publish_crop_receipt(
            RuntimeOrigin::signed(bob()),
            1,
            supply
        ));

        assert_err!(
            PrestoPallet::publish_crop_receipt(RuntimeOrigin::signed(bob()), 1, supply),
            E::CropReceiptAlreadyHasDecision
        );

        let coupon_asset_id = Coupons::<Runtime>::iter()
            .collect::<Vec<_>>()
            .first()
            .unwrap()
            .0;

        let coupon_asset_info = assets::Pallet::<Runtime>::asset_infos(coupon_asset_id);

        assert_eq!(coupon_asset_info.0, AssetSymbol(b"BRC1".to_vec()));
        assert_eq!(coupon_asset_info.1, AssetName(b"Brazil Coupon 1".to_vec()));

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEXId::PolkaswapPresto,
            base: coupon_asset_id,
            quote: PRUSD,
        };

        let order_book = OrderBookPallet::order_books(order_book_id).unwrap();

        assert_eq!(order_book.tick_size, BalanceUnit::divisible(balance!(0.01)));
        assert_eq!(order_book.step_lot_size, BalanceUnit::indivisible(1));
        assert_eq!(order_book.min_lot_size, BalanceUnit::indivisible(1));
        assert_eq!(order_book.max_lot_size, BalanceUnit::indivisible(1000));

        let price = BalanceUnit::divisible(balance!(10));

        let volume = *OrderBookPallet::aggregated_asks(order_book_id)
            .get(&price)
            .unwrap();
        assert_eq!(volume, BalanceUnit::indivisible(supply));

        let ids = OrderBookPallet::asks(order_book_id, price).unwrap();
        assert_eq!(ids.len(), 10);

        for id in ids {
            let order = OrderBookPallet::limit_orders(order_book_id, id).unwrap();
            assert_eq!(order.owner, bob());
        }

        assert_eq!(
            extended_assets::Pallet::<Runtime>::soulbound_asset(SBT_PRACS.into_predefined())
                .unwrap()
                .regulated_assets,
            BTreeSet::from([PRUSD, coupon_asset_id])
        );
        assert_eq!(
            extended_assets::Pallet::<Runtime>::regulated_asset_to_sbt(PRUSD),
            SBT_PRACS.into()
        );
        assert_eq!(
            extended_assets::Pallet::<Runtime>::regulated_asset_to_sbt(coupon_asset_id),
            SBT_PRACS.into()
        );
    });
}
