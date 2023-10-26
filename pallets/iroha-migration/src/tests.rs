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

use crate::mock::*;
use crate::{Error, MigratedAccounts, Pallet, PendingMultiSigAccounts, PendingReferrals};
use common::{AssetInfoProvider, VAL};
use frame_support::assert_noop;
use frame_support::assert_ok;
use frame_support::traits::OnInitialize;
use referrals::Referrers;

type Assets = assets::Pallet<Runtime>;

#[test]
fn test_verification_failed() {
    new_test_ext().execute_with(|| {
        assert_noop!(Pallet::<Runtime>::migrate(
            RuntimeOrigin::signed(ALICE),
             "did_sora_d9bda3688c6f608ab15c@sora".to_string(),
              "d9bda3688c6f608ab15c03a55B171DA0413788a40a25722b4ae4d3672890bcd7".to_string(),
              "fffffffffb19abcfc869eae8f14389680aeCC7afb5959fb87c2fee65951a46a7507f8bf11ee0c609fb101fd41d6534b84bb8c3e55a79189de96bcc8227fa5c01".to_string()),
            Error::<Runtime>::SignatureVerificationFailed);
    });
}

#[test]
fn test_account_not_found() {
    new_test_ext().execute_with(|| {
        assert_noop!(Pallet::<Runtime>::migrate(
            RuntimeOrigin::signed(ALICE),
             "did_sora_1@sora".to_string(),
              "b6deadb8ac430c0c8ed33ff6e170708ec838a215ba70c30CF8602328834912c7".to_string(),
              "4edc624abe4747f3bb4854dda0325d31869ff71bb00771865DC1b31d510df26994e88ba202aafc084832d9ed7d0ac71df2fe9fa99d72a3e5b7729e2c729dbe08".to_string()),
            Error::<Runtime>::AccountNotFound);
    });
}

#[test]
fn test_already_migrated() {
    new_test_ext().execute_with(|| {
        assert_ok!(Pallet::<Runtime>::migrate(
            RuntimeOrigin::signed(ALICE),
             "did_sora_d9bda3688c6f608ab15c@sora".to_string(),
              "d9bda3688c6f608ab15c03a55B171da0413788a40a25722b4ae4d3672890bcd7".to_string(),
              "c3cdb9a20b19abcfc869eae8f14389680aecc7afb5959fb87C2fee65951a46a7507f8bf11ee0c609fb101fd41d6534b84bb8c3e55a79189de96bcc8227fa5c01".to_string()));
              assert!(MigratedAccounts::<Runtime>::contains_key("did_sora_d9bda3688c6f608ab15c@sora".to_string()));
              assert_noop!(Pallet::<Runtime>::migrate(
                RuntimeOrigin::signed(ALICE),
                 "did_sora_d9bda3688c6f608ab15c@sora".to_string(),
                  "d9bda3688c6f608ab15c03a55b171da0413788a40a25722b4ae4d3672890BCD7".to_string(),
                  "c3cdb9a20b19abcfc869eae8f14389680aecc7afb5959fb87c2fee65951a46a7507f8bf11ee0c609fb101fd41d6534b84bb8c3e55a79189de96bcc8227FA5c01".to_string()),
                Error::<Runtime>::AccountAlreadyMigrated);
    });
}

#[test]
fn test_migrate_balance() {
    new_test_ext().execute_with(|| {
        assert_eq!(Assets::free_balance(&VAL, &ALICE).unwrap(), 0u128);
        assert_ok!(Pallet::<Runtime>::migrate(
            RuntimeOrigin::signed(ALICE),
             "did_sora_balance@sora".to_string(),
              "9a685d77bcd3f60e6cc1e91eedc7a48e11bbcf1a036b920f3bae0372a78A5432".to_string(),
              "233896712f752760713539f56c92534ff8f4f290812e8f129Ce0b513b99cbdffcea95abeed68edd1b0a4e4b52877c13c26c6c89e5bb6bf023ac6c0f4f53c0c02".to_string()));
              assert_eq!(Assets::free_balance(&VAL, &ALICE).unwrap(), 300u128);
    });
}

#[test]
fn test_migrate_referrer_migrates_first() {
    new_test_ext().execute_with(|| {
        assert_ok!(Pallet::<Runtime>::migrate(
            RuntimeOrigin::signed(ALICE), "did_sora_referrer@sora".to_string(),
            "dd54e9efb95531154316cf3e28e2232abab349296dDe94353febc9ebbb3ff283".to_string(),
            "f87bfa375cb4be3ee530ca6d76790b6aac9dbbbbff5dCeb58021491a1d83526e31685c8d38f8c2dcb932939599ab4ff6733f0547c362322f1a51a666877ab003".to_string()));
        assert_ok!(Pallet::<Runtime>::migrate(
            RuntimeOrigin::signed(BOB),
            "did_sora_referral@sora".to_string(),
            "cba1c8c2eeaf287d734bd167b10d762e89c0ee8327a29e04f064ae94086ef1e9".to_string(),
            "dd878f4223026ad274212bf153a59fffff0a84a2ef5c40C60905b1fd2219508296eecd8f56618986352653757628e41fcaaab202cfe6cf3abcc28d7972a68e06".to_string()));
        assert_eq!(Referrers::<Runtime>::get(BOB), Some(ALICE));
        assert!(PendingReferrals::<Runtime>::get("did_sora_referrer@sora".to_string()).is_empty());
    });
}

#[test]
fn test_migrate_referral_migrates_first() {
    new_test_ext().execute_with(|| {
        assert_ok!(Pallet::<Runtime>::migrate(
          RuntimeOrigin::signed(BOB),
           "did_sora_referral@sora".to_string(),
            "cba1c8c2eeaf287d734bd167b10d762e89c0ee8327A29e04f064ae94086ef1e9".to_string(),
            "dd878f4223026ad274212bf153a59fffff0a84a2Ef5c40c60905b1fd2219508296eecd8f56618986352653757628e41fcaaab202cfe6cf3abcc28d7972a68e06".to_string()));
        assert_eq!(PendingReferrals::<Runtime>::get("did_sora_referrer@sora".to_string()), vec![BOB]);
        assert_ok!(Pallet::<Runtime>::migrate(
            RuntimeOrigin::signed(ALICE),
             "did_sora_referrer@sora".to_string(),
              "dd54e9efb95531154316cf3e28e2232abab349296dDe94353febc9ebbb3ff283".to_string(),
              "f87bfa375cb4be3ee530ca6d76790b6aac9dbbbbff5dceb58021491a1d83526e31685c8d38f8c2dcb932939599ab4ff6733f0547c362322f1a51a666877ab003".to_string()));
        assert_eq!(Referrers::<Runtime>::get(BOB), Some(ALICE));
    });
}

#[test]
fn test_migrate_multi_sig() {
    new_test_ext().execute_with(|| {
        let iroha_address = "did_sora_multi_sig@sora".to_string();
        assert_ok!(Pallet::<Runtime>::migrate(
            RuntimeOrigin::signed(ALICE),
            iroha_address.clone(),
            "f7d89d39d48a67e4741a612de10650234f9148e84fe9E8b2a9fad322b0d8e5bc".to_string(),
            "d5f6dcc6967aa05df71894dd2c253085b236026efc1C66d4b33ee88dda20fc751b516aef631d1f96919f8cba2e15334022e04ef6602298d6b9820daeefe13e03".to_string())
        );
        assert!(!MigratedAccounts::<Runtime>::contains_key(&iroha_address));
        assert!(PendingMultiSigAccounts::<Runtime>::contains_key(&iroha_address));
        let multi_account = {
            let mut signatories = [ALICE, BOB, CHARLIE];
            signatories.sort();
            pallet_multisig::Pallet::<Runtime>::multi_account_id(&signatories, 2)
        };
        assert_eq!(Assets::free_balance(&VAL, &multi_account).unwrap(), 0u128);
        assert_ok!(Pallet::<Runtime>::migrate(
            RuntimeOrigin::signed(BOB),
            iroha_address.clone(),
            "f56b4880ed91a25b257144acab749f615855c4b1b6A5d7891e1a6cdd9fd695e9".to_string(),
            "5c0f4296175b9836baac7c2d92116c90961bb80f87C30e3e2e2b2d5819d0c278fa55d3f04793d7fbf19a78afeb8b52f17b5ba55bf7373e726723da7155cad70d".to_string())
        );
        assert!(PendingMultiSigAccounts::<Runtime>::contains_key(&iroha_address));
        assert_ok!(Pallet::<Runtime>::migrate(
            RuntimeOrigin::signed(CHARLIE),
            iroha_address.clone(),
            "57571ec82cff710143eba60c05d88de14a22799048137162D63c534a8b02dc20".to_string(),
            "3cfd2e95676ec7f4a7eb6f8bf91b447990c1bb4d771784e5E5d6027852eef75c13ad911d6fac9130b24f67e2088c3b908d25c092f87b77ed8a44dcd62572cc0f".to_string())
        );
        assert!(MigratedAccounts::<Runtime>::contains_key(&iroha_address));
        assert!(!PendingMultiSigAccounts::<Runtime>::contains_key(&iroha_address));
        assert_eq!(Assets::free_balance(&VAL, &multi_account).unwrap(), 1000u128);
    });
}

#[test]
fn test_migrate_multi_sig_after_timeout() {
    new_test_ext().execute_with(|| {
        let iroha_address = "did_sora_multi_sig@sora".to_string();
        assert_ok!(Pallet::<Runtime>::migrate(
            RuntimeOrigin::signed(ALICE),
            iroha_address.clone(),
            "f7d89d39d48a67e4741a612de10650234f9148e84fE9e8b2a9fad322b0d8e5bc".to_string(),
            "d5f6dcc6967aa05df71894dd2c253085b236026efC1c66d4b33ee88dda20fc751b516aef631d1f96919f8cba2e15334022e04ef6602298d6b9820daeefe13e03".to_string())
        );

        assert!(!MigratedAccounts::<Runtime>::contains_key(&iroha_address));
        assert!(PendingMultiSigAccounts::<Runtime>::contains_key(&iroha_address));
        let multi_account_of_2 = {
            let mut signatories = [ALICE, BOB];
            signatories.sort();
            pallet_multisig::Pallet::<Runtime>::multi_account_id(&signatories, 2)
        };
        let multi_account_of_3 = {
            let mut signatories = [ALICE, BOB, CHARLIE];
            signatories.sort();
            pallet_multisig::Pallet::<Runtime>::multi_account_id(&signatories, 2)
        };
        assert_eq!(Assets::free_balance(&VAL, &multi_account_of_2).unwrap(), 0u128);
        assert_eq!(Assets::free_balance(&VAL, &multi_account_of_3).unwrap(), 0u128);

        assert_ok!(Pallet::<Runtime>::migrate(
            RuntimeOrigin::signed(BOB),
            iroha_address.clone(),
            "f56b4880ed91a25b257144acab749f615855c4b1b6A5d7891e1a6cdd9fd695e9".to_string(),
            "5c0f4296175b9836baac7c2d92116c90961bb80f87C30e3e2e2b2d5819d0c278fa55d3f04793d7fbf19a78afeb8b52f17b5ba55bf7373e726723da7155cad70d".to_string())
        );

        assert!(!MigratedAccounts::<Runtime>::contains_key(&iroha_address));
        assert!(PendingMultiSigAccounts::<Runtime>::contains_key(&iroha_address));
        assert_eq!(Assets::free_balance(&VAL, &multi_account_of_2).unwrap(), 0u128);
        assert_eq!(Assets::free_balance(&VAL, &multi_account_of_3).unwrap(), 0u128);

        Pallet::<Runtime>::on_initialize(crate::blocks_till_migration::<Runtime>() + 1);

        assert!(MigratedAccounts::<Runtime>::contains_key(&iroha_address));
        assert!(!PendingMultiSigAccounts::<Runtime>::contains_key(&iroha_address));
        assert_eq!(Assets::free_balance(&VAL, &multi_account_of_2).unwrap(), 1000u128);
        assert_eq!(Assets::free_balance(&VAL, &multi_account_of_3).unwrap(), 0u128);

        assert_noop!(Pallet::<Runtime>::migrate(
            RuntimeOrigin::signed(CHARLIE),
            iroha_address,
            "57571ec82cff710143eba60c05d88de14a22799048137162d63C534a8b02dc20".to_string(),
            "3cfd2e95676ec7f4a7eb6f8bf91b447990c1bb4d771784e5e5D6027852eef75c13ad911d6fac9130b24f67e2088c3b908d25c092f87b77ed8a44dcd62572cc0f".to_string()),
            Error::<Runtime>::AccountAlreadyMigrated,
        );
    });
}

#[test]
fn test_migrate_multi_sig_public_key_already_used() {
    new_test_ext().execute_with(|| {
        let iroha_address = "did_sora_multi_sig@sora".to_string();
        assert_ok!(Pallet::<Runtime>::migrate(
            RuntimeOrigin::signed(ALICE),
            iroha_address.clone(),
            "f7d89d39d48a67e4741a612de10650234f9148e84fE9e8b2a9fad322b0d8e5bc".to_string(),
            "d5f6dcc6967aa05df71894dd2c253085b236026efC1c66d4b33ee88dda20fc751b516aef631d1f96919f8cba2e15334022e04ef6602298d6b9820daeefe13e03".to_string())
        );
        assert!(!MigratedAccounts::<Runtime>::contains_key(&iroha_address));
        assert!(PendingMultiSigAccounts::<Runtime>::contains_key(&iroha_address));
        assert_noop!(Pallet::<Runtime>::migrate(
            RuntimeOrigin::signed(ALICE),
            iroha_address,
            "f7d89d39d48a67e4741a612de10650234f9148e84fE9e8b2a9fad322b0d8e5bc".to_string(),
            "d5f6dcc6967aa05df71894dd2c253085b236026efC1c66d4b33ee88dda20fc751b516aef631d1f96919f8cba2e15334022e04ef6602298d6b9820daeefe13e03".to_string()),
            Error::<Runtime>::PublicKeyAlreadyUsed
        );
    });
}
