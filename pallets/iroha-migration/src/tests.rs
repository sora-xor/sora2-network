use crate::{mock::*, Error, MigratedAccounts, Module, PendingMultiSigAccounts, PendingReferrals};
use common::{prelude::Balance, VAL};
use frame_support::{assert_noop, assert_ok, storage::StorageMap};
use referral_system::Referrers;

type Assets = assets::Module<Test>;

#[test]
fn test_verification_failed() {
    new_test_ext().execute_with(|| {
        assert_noop!(Module::<Test>::migrate(
            Origin::signed(ALICE),
             "did_sora_d9bda3688c6f608ab15c@sora".to_string(),
              "d9bda3688c6f608ab15c03a55b171da0413788a40a25722b4ae4d3672890bcd7".to_string(),
              "fffffffffb19abcfc869eae8f14389680aecc7afb5959fb87c2fee65951a46a7507f8bf11ee0c609fb101fd41d6534b84bb8c3e55a79189de96bcc8227fa5c01".to_string()),
            Error::<Test>::SignatureVerificationFailed);
    });
}

#[test]
fn test_account_not_found() {
    new_test_ext().execute_with(|| {
        assert_noop!(Module::<Test>::migrate(
            Origin::signed(ALICE),
             "did_sora_1@sora".to_string(),
              "b6deadb8ac430c0c8ed33ff6e170708ec838a215ba70c30cf8602328834912c7".to_string(),
              "4edc624abe4747f3bb4854dda0325d31869ff71bb00771865dc1b31d510df26994e88ba202aafc084832d9ed7d0ac71df2fe9fa99d72a3e5b7729e2c729dbe08".to_string()), Error::<Test>::AccountNotFound);
    });
}

#[test]
fn test_already_migrated() {
    new_test_ext().execute_with(|| {
        assert_ok!(Module::<Test>::migrate(
            Origin::signed(ALICE),
             "did_sora_d9bda3688c6f608ab15c@sora".to_string(),
              "d9bda3688c6f608ab15c03a55b171da0413788a40a25722b4ae4d3672890bcd7".to_string(),
              "c3cdb9a20b19abcfc869eae8f14389680aecc7afb5959fb87c2fee65951a46a7507f8bf11ee0c609fb101fd41d6534b84bb8c3e55a79189de96bcc8227fa5c01".to_string()));
              assert!(MigratedAccounts::<Test>::contains_key("did_sora_d9bda3688c6f608ab15c@sora".to_string()));
              assert_noop!(Module::<Test>::migrate(
                Origin::signed(ALICE),
                 "did_sora_d9bda3688c6f608ab15c@sora".to_string(),
                  "d9bda3688c6f608ab15c03a55b171da0413788a40a25722b4ae4d3672890bcd7".to_string(),
                  "c3cdb9a20b19abcfc869eae8f14389680aecc7afb5959fb87c2fee65951a46a7507f8bf11ee0c609fb101fd41d6534b84bb8c3e55a79189de96bcc8227fa5c01".to_string()),
                Error::<Test>::AccountAlreadyMigrated);
    });
}

#[test]
fn test_migrate_balance() {
    new_test_ext().execute_with(|| {
        assert_eq!(Assets::free_balance(&VAL, &ALICE).unwrap(), Balance::from(0u128));
        assert_ok!(Module::<Test>::migrate(
            Origin::signed(ALICE),
             "did_sora_balance@sora".to_string(),
              "9a685d77bcd3f60e6cc1e91eedc7a48e11bbcf1a036b920f3bae0372a78a5432".to_string(),
              "233896712f752760713539f56c92534ff8f4f290812e8f129ce0b513b99cbdffcea95abeed68edd1b0a4e4b52877c13c26c6c89e5bb6bf023ac6c0f4f53c0c02".to_string()));
              assert_eq!(Assets::free_balance(&VAL, &ALICE).unwrap(), Balance::from(300u128));
            });
}

#[test]
fn test_migrate_referrer_migrates_first() {
    new_test_ext().execute_with(|| {
        assert_ok!(Module::<Test>::migrate(
        Origin::signed(ALICE),
         "did_sora_referrer@sora".to_string(),
          "dd54e9efb95531154316cf3e28e2232abab349296dde94353febc9ebbb3ff283".to_string(),
          "f87bfa375cb4be3ee530ca6d76790b6aac9dbbbbff5dceb58021491a1d83526e31685c8d38f8c2dcb932939599ab4ff6733f0547c362322f1a51a666877ab003".to_string()));
          assert_ok!(Module::<Test>::migrate(
            Origin::signed(BOB),
             "did_sora_referral@sora".to_string(),
              "cba1c8c2eeaf287d734bd167b10d762e89c0ee8327a29e04f064ae94086ef1e9".to_string(),
              "dd878f4223026ad274212bf153a59fffff0a84a2ef5c40c60905b1fd2219508296eecd8f56618986352653757628e41fcaaab202cfe6cf3abcc28d7972a68e06".to_string()));
              assert_eq!(Referrers::<Test>::get(&BOB), Some(ALICE));
              assert!(PendingReferrals::<Test>::get(&"did_sora_referrer@sora".to_string()).is_empty());
          });
}

#[test]
fn test_migrate_referral_migrates_first() {
    new_test_ext().execute_with(|| {
        assert_ok!(Module::<Test>::migrate(
          Origin::signed(BOB),
           "did_sora_referral@sora".to_string(),
            "cba1c8c2eeaf287d734bd167b10d762e89c0ee8327a29e04f064ae94086ef1e9".to_string(),
            "dd878f4223026ad274212bf153a59fffff0a84a2ef5c40c60905b1fd2219508296eecd8f56618986352653757628e41fcaaab202cfe6cf3abcc28d7972a68e06".to_string()));
            assert_eq!(PendingReferrals::<Test>::get(&"did_sora_referrer@sora".to_string()), vec![BOB]);
            assert_ok!(Module::<Test>::migrate(
                Origin::signed(ALICE),
                 "did_sora_referrer@sora".to_string(),
                  "dd54e9efb95531154316cf3e28e2232abab349296dde94353febc9ebbb3ff283".to_string(),
                  "f87bfa375cb4be3ee530ca6d76790b6aac9dbbbbff5dceb58021491a1d83526e31685c8d38f8c2dcb932939599ab4ff6733f0547c362322f1a51a666877ab003".to_string()));
            assert_eq!(Referrers::<Test>::get(&BOB), Some(ALICE));
        });
}

#[test]
fn test_migrate_multi_sig() {
    new_test_ext().execute_with(|| {
        let iroha_address = "did_sora_multi_sig@sora".to_string();
        assert_ok!(Module::<Test>::migrate(
            Origin::signed(ALICE),
            iroha_address.clone(),
            "f7d89d39d48a67e4741a612de10650234f9148e84fe9e8b2a9fad322b0d8e5bc".to_string(),
            "d5f6dcc6967aa05df71894dd2c253085b236026efc1c66d4b33ee88dda20fc751b516aef631d1f96919f8cba2e15334022e04ef6602298d6b9820daeefe13e03".to_string())
        );
        assert!(!MigratedAccounts::<Test>::contains_key(&iroha_address));
        assert!(PendingMultiSigAccounts::<Test>::contains_key(&iroha_address));
        let multi_sig_account = PendingMultiSigAccounts::<Test>::get(&iroha_address);
        assert_eq!(Assets::free_balance(&VAL, &multi_sig_account).unwrap(), Balance::from(0u128));
        assert_ok!(Module::<Test>::migrate(
            Origin::signed(BOB),
            iroha_address.clone(),
            "f56b4880ed91a25b257144acab749f615855c4b1b6a5d7891e1a6cdd9fd695e9".to_string(),
            "5c0f4296175b9836baac7c2d92116c90961bb80f87c30e3e2e2b2d5819d0c278fa55d3f04793d7fbf19a78afeb8b52f17b5ba55bf7373e726723da7155cad70d".to_string())
        );
        assert!(MigratedAccounts::<Test>::contains_key(&iroha_address));
        assert!(PendingMultiSigAccounts::<Test>::contains_key(&iroha_address));
        assert_eq!(Assets::free_balance(&VAL, &multi_sig_account).unwrap(), Balance::from(1000u128));
        assert_ok!(Module::<Test>::migrate(
            Origin::signed(CHARLIE),
            iroha_address.clone(),
            "57571ec82cff710143eba60c05d88de14a22799048137162d63c534a8b02dc20".to_string(),
            "3cfd2e95676ec7f4a7eb6f8bf91b447990c1bb4d771784e5e5d6027852eef75c13ad911d6fac9130b24f67e2088c3b908d25c092f87b77ed8a44dcd62572cc0f".to_string())
        );
        assert!(!PendingMultiSigAccounts::<Test>::contains_key(&iroha_address));
    });
}

#[test]
fn test_migrate_multi_sig_public_key_already_used() {
    new_test_ext().execute_with(|| {
        let iroha_address = "did_sora_multi_sig@sora".to_string();
        assert_ok!(Module::<Test>::migrate(
            Origin::signed(ALICE),
            iroha_address.clone(),
            "f7d89d39d48a67e4741a612de10650234f9148e84fe9e8b2a9fad322b0d8e5bc".to_string(),
            "d5f6dcc6967aa05df71894dd2c253085b236026efc1c66d4b33ee88dda20fc751b516aef631d1f96919f8cba2e15334022e04ef6602298d6b9820daeefe13e03".to_string())
        );
        assert!(!MigratedAccounts::<Test>::contains_key(&iroha_address));
        assert!(PendingMultiSigAccounts::<Test>::contains_key(&iroha_address));
        assert_noop!(Module::<Test>::migrate(
            Origin::signed(ALICE),
            iroha_address.clone(),
            "f7d89d39d48a67e4741a612de10650234f9148e84fe9e8b2a9fad322b0d8e5bc".to_string(),
            "d5f6dcc6967aa05df71894dd2c253085b236026efc1c66d4b33ee88dda20fc751b516aef631d1f96919f8cba2e15334022e04ef6602298d6b9820daeefe13e03".to_string()),
            Error::<Test>::PublicKeyAlreadyUsed
        );
    });
}
