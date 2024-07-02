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

#![cfg(feature = "wip")] // DEFI-R

use crate::mock::{Timestamp, *};
use crate::test_utils::*;
use crate::*;
use common::{TechAccountId, XOR};
use frame_support::{assert_err, assert_ok};
use permissions::MINT;
use sp_core::crypto::AccountId32;

#[test]
fn test_default_value_asset_regulated() {
    new_test_ext().execute_with(|| {
        let default_value = ExtendedAssets::regulated_asset(XOR);
        assert!(!default_value);
    })
}

#[test]
fn test_cannot_regulate_already_regulated_asset() {
    new_test_ext().execute_with(|| {
        let owner = bob();
        let asset_id = add_asset::<TestRuntime>(&owner);

        // Regulate the asset for the first time
        assert_ok!(ExtendedAssets::regulate_asset(
            RuntimeOrigin::signed(owner.clone()),
            asset_id
        ));

        // Try to regulate the already regulated asset
        assert_err!(
            ExtendedAssets::regulate_asset(RuntimeOrigin::signed(owner), asset_id),
            Error::<TestRuntime>::AssetAlreadyRegulated
        );
    })
}

#[test]
fn test_tech_account_can_pass_check_permission() {
    new_test_ext().execute_with(|| {
        let owner = bob();
        let tech_account = TechAccountId::Generic("tech".into(), "account".into());

        let asset_id = add_asset::<TestRuntime>(&owner);

        // Regulate the asset
        assert_ok!(ExtendedAssets::regulate_asset(
            RuntimeOrigin::signed(owner),
            asset_id
        ));

        mock::Technical::register_tech_account_id(tech_account.clone()).unwrap();
        let account_id = mock::Technical::tech_account_id_to_account_id(&tech_account).unwrap();

        // Tech account can pass permission check for unregulated asset
        assert_ok!(ExtendedAssets::check_permission(
            &account_id,
            &account_id,
            &asset_id,
            &TRANSFER
        ));
    })
}

#[test]
fn test_unregulated_asset_can_pass_check_permission() {
    new_test_ext().execute_with(|| {
        let owner = bob();
        let non_owner = alice();
        let asset_id = add_asset::<TestRuntime>(&owner);

        // Unregulated asset can pass permission check
        assert_ok!(ExtendedAssets::check_permission(
            &owner, &non_owner, &asset_id, &TRANSFER
        ));
    })
}

#[test]
fn test_only_asset_owner_can_regulate_asset() {
    new_test_ext().execute_with(|| {
        let owner = bob();
        let non_owner = alice();
        let asset_id = add_asset::<TestRuntime>(&owner);

        // Non-owner cannot regulate asset
        assert_err!(
            ExtendedAssets::regulate_asset(RuntimeOrigin::signed(non_owner), asset_id),
            Error::<TestRuntime>::OnlyAssetOwnerCanRegulate
        );

        // Owner can regulate asset
        assert_ok!(ExtendedAssets::regulate_asset(
            RuntimeOrigin::signed(owner),
            asset_id
        ));
    })
}

#[test]
fn test_issue_sbt_succeeds() {
    new_test_ext().execute_with(|| {
        let owner = bob();
        let asset_name = AssetName(b"Soulbound Token".to_vec());
        let asset_symbol = AssetSymbol(b"SBT".to_vec());
        let asset_id = add_asset::<TestRuntime>(&owner);

        assert_ok!(ExtendedAssets::regulate_asset(
            RuntimeOrigin::signed(owner.clone()),
            asset_id
        ));

        frame_system::Pallet::<TestRuntime>::inc_providers(&owner);
        // Owner can issue SBT
        assert_ok!(ExtendedAssets::issue_sbt(
            RuntimeOrigin::signed(owner),
            asset_symbol,
            asset_name,
            None,
            None,
            None,
        ));
    })
}

#[test]
fn test_bind_sbt_fails_due_to_invalid_regulated_asset() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let owner = bob();
        let asset_id = add_asset::<TestRuntime>(&owner);

        let sbt_asset_id = register_sbt_asset::<TestRuntime>(&owner);

        assert_err!(
            ExtendedAssets::bind_regulated_asset_to_sbt(
                RuntimeOrigin::signed(owner.clone()),
                sbt_asset_id,
                XOR
            ),
            Error::<TestRuntime>::RegulatedAssetNoOwnedBySBTIssuer
        );

        let another_sbt_asset_id = register_sbt_asset::<TestRuntime>(&owner);

        assert_err!(
            ExtendedAssets::bind_regulated_asset_to_sbt(
                RuntimeOrigin::signed(owner),
                another_sbt_asset_id,
                asset_id
            ),
            Error::<TestRuntime>::AssetNotRegulated
        );
    });
}

#[test]
fn test_sbt_only_operationable_by_its_owner() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let owner = bob();
        let non_owner = alice();

        let asset_id = add_asset::<TestRuntime>(&owner);

        assert_ok!(ExtendedAssets::regulate_asset(
            RuntimeOrigin::signed(owner.clone()),
            asset_id
        ));

        let sbt_asset_id = register_sbt_asset::<TestRuntime>(&owner);

        assert_ok!(ExtendedAssets::bind_regulated_asset_to_sbt(
            RuntimeOrigin::signed(owner.clone()),
            sbt_asset_id,
            asset_id
        ));

        // SBT operations by non-owner should fail
        assert_err!(
            ExtendedAssets::check_permission(&non_owner, &non_owner, &sbt_asset_id, &TRANSFER),
            Error::<TestRuntime>::SoulboundAssetNotOperationable
        );

        assert_ok!(Assets::mint_to(&sbt_asset_id, &owner, &non_owner, 1));

        // SBT operations by non-owner should fail
        assert_err!(
            Assets::transfer(RuntimeOrigin::signed(non_owner), sbt_asset_id, owner, 1),
            Error::<TestRuntime>::SoulboundAssetNotOperationable
        );
    })
}

#[test]
fn test_sbt_cannot_be_transferred() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let owner = bob();
        let non_owner = alice();

        let asset_id = add_asset::<TestRuntime>(&owner);
        assert_ok!(ExtendedAssets::regulate_asset(
            RuntimeOrigin::signed(owner.clone()),
            asset_id
        ));

        let sbt_asset_id = register_sbt_asset::<TestRuntime>(&owner);
        assert_ok!(ExtendedAssets::bind_regulated_asset_to_sbt(
            RuntimeOrigin::signed(owner.clone()),
            sbt_asset_id,
            asset_id
        ));

        assert_err!(
            Assets::transfer(RuntimeOrigin::signed(owner), sbt_asset_id, non_owner, 1),
            Error::<TestRuntime>::SoulboundAssetNotTransferable
        );
    })
}

#[test]
fn test_not_allowed_to_regulate_sbt() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let owner = bob();

        let asset_id = add_asset::<TestRuntime>(&owner);
        assert_ok!(ExtendedAssets::regulate_asset(
            RuntimeOrigin::signed(owner.clone()),
            asset_id
        ));

        let sbt_asset_id = register_sbt_asset::<TestRuntime>(&owner);
        assert_ok!(ExtendedAssets::bind_regulated_asset_to_sbt(
            RuntimeOrigin::signed(owner.clone()),
            sbt_asset_id,
            asset_id
        ));

        assert_err!(
            ExtendedAssets::regulate_asset(RuntimeOrigin::signed(owner), sbt_asset_id),
            Error::<TestRuntime>::NotAllowedToRegulateSoulboundAsset
        );
    });
}

#[test]
fn test_check_permission_pass_only_if_all_invloved_accounts_have_valid_sbt() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let owner = bob();
        let non_owner = alice();
        let another_account = AccountId32::from([3u8; 32]);

        // Regulate an asset
        let asset_id = add_asset::<TestRuntime>(&owner);
        assert_ok!(ExtendedAssets::regulate_asset(
            RuntimeOrigin::signed(owner.clone()),
            asset_id
        ));

        let sbt_asset_id = register_sbt_asset::<TestRuntime>(&owner);

        assert_ok!(ExtendedAssets::bind_regulated_asset_to_sbt(
            RuntimeOrigin::signed(owner.clone()),
            sbt_asset_id,
            asset_id
        ));

        // Give SBT to another_account
        assert_ok!(Assets::mint_to(&sbt_asset_id, &owner, &another_account, 1));
        assert_ok!(Assets::mint_to(&sbt_asset_id, &owner, &owner, 1));

        // Check permission should pass only if all involved accounts have SBT
        assert_ok!(ExtendedAssets::check_permission(
            &owner,
            &another_account,
            &asset_id,
            &MINT
        ));
        assert_err!(
            ExtendedAssets::check_permission(&owner, &non_owner, &asset_id, &MINT),
            Error::<TestRuntime>::AllInvolvedUsersShouldHoldValidSBT
        );
        assert_err!(
            ExtendedAssets::check_permission(&non_owner, &another_account, &asset_id, &MINT),
            Error::<TestRuntime>::AllInvolvedUsersShouldHoldValidSBT
        );

        // Owner can burn SBT from another_account (revoke)
        assert_ok!(Assets::burn_from(
            &sbt_asset_id,
            &owner,
            &another_account,
            1
        ));

        assert_err!(
            ExtendedAssets::check_permission(&owner, &another_account, &asset_id, &MINT),
            Error::<TestRuntime>::AllInvolvedUsersShouldHoldValidSBT
        );
    })
}

#[test]
fn test_check_permission_fails_if_one_invloved_account_has_not_valid_sbt_due_to_expiration() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let owner = bob();
        let another_account = AccountId32::from([3u8; 32]);
        let expiration_timestamp = Timestamp::now().saturating_add(100);
        let later_expiration_timestamp = Timestamp::now().saturating_add(200);

        // Regulate an asset
        let regulated_asset_id = add_asset::<TestRuntime>(&owner);
        assert_ok!(ExtendedAssets::regulate_asset(
            RuntimeOrigin::signed(owner.clone()),
            regulated_asset_id
        ));

        let soon_expires_sbt_asset_id = register_sbt_asset::<TestRuntime>(&owner);
        assert_ok!(ExtendedAssets::bind_regulated_asset_to_sbt(
            RuntimeOrigin::signed(owner.clone()),
            soon_expires_sbt_asset_id,
            regulated_asset_id
        ));

        // Give SBT to another_account
        assert_ok!(Assets::mint_to(
            &soon_expires_sbt_asset_id,
            &owner,
            &another_account,
            1
        ));
        assert_ok!(Assets::mint_to(
            &soon_expires_sbt_asset_id,
            &owner,
            &owner,
            1
        ));

        assert_ok!(ExtendedAssets::set_sbt_expiration(
            RuntimeOrigin::signed(owner.clone()),
            another_account.clone(),
            soon_expires_sbt_asset_id,
            Some(expiration_timestamp)
        ));

        // Check permission should pass only if all involved accounts have SBT
        // before expiration happens
        assert_ok!(ExtendedAssets::check_permission(
            &owner,
            &another_account,
            &regulated_asset_id,
            &TRANSFER
        ));

        // Move time to make sure the SBT(marked as soon_expires) has expired
        Timestamp::set_timestamp(later_expiration_timestamp);

        assert_err!(
            ExtendedAssets::check_permission(
                &owner,
                &another_account,
                &regulated_asset_id,
                &TRANSFER
            ),
            Error::<TestRuntime>::AllInvolvedUsersShouldHoldValidSBT
        );
    })
}

#[test]
fn test_set_sbt_expiration_succeeds() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let owner = bob();
        let non_owner = alice();
        let new_expiration_timestamp = Timestamp::now().saturating_add(100);

        let asset_id = add_asset::<TestRuntime>(&owner);
        assert_ok!(ExtendedAssets::regulate_asset(
            RuntimeOrigin::signed(owner.clone()),
            asset_id
        ));

        let sbt_asset_id = register_sbt_asset::<TestRuntime>(&owner);
        assert_ok!(ExtendedAssets::bind_regulated_asset_to_sbt(
            RuntimeOrigin::signed(owner.clone()),
            sbt_asset_id,
            asset_id
        ));

        // Update expiration date
        assert_ok!(ExtendedAssets::set_sbt_expiration(
            RuntimeOrigin::signed(owner.clone()),
            owner.clone(),
            sbt_asset_id,
            Some(new_expiration_timestamp)
        ));

        let updated_for_owner_expires_at =
            ExtendedAssets::sbt_asset_expiration(owner, sbt_asset_id);
        assert_eq!(updated_for_owner_expires_at, Some(new_expiration_timestamp));

        let not_updated_for_non_owner_expires_at =
            ExtendedAssets::sbt_asset_expiration(non_owner, sbt_asset_id);
        assert_eq!(not_updated_for_non_owner_expires_at, None);
    });
}

#[test]
fn test_set_sbt_expiration_fails_for_non_owner() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let owner = bob();
        let non_owner = alice();
        let expiration_timestamp = Timestamp::now().saturating_add(100);
        let new_expiration_timestamp = expiration_timestamp.saturating_add(100);

        let asset_id = add_asset::<TestRuntime>(&owner);
        assert_ok!(ExtendedAssets::regulate_asset(
            RuntimeOrigin::signed(owner.clone()),
            asset_id
        ));

        let sbt_asset_id = register_sbt_asset::<TestRuntime>(&owner);
        assert_ok!(ExtendedAssets::bind_regulated_asset_to_sbt(
            RuntimeOrigin::signed(owner),
            sbt_asset_id,
            asset_id
        ));

        // Attempt to update expiration date by non-owner
        assert_err!(
            ExtendedAssets::set_sbt_expiration(
                RuntimeOrigin::signed(non_owner.clone()),
                non_owner,
                sbt_asset_id,
                Some(new_expiration_timestamp)
            ),
            Error::<TestRuntime>::NotSBTOwner
        );
    });
}

#[test]
fn test_set_sbt_expiration_fails_for_non_existent_sbt() {
    new_test_ext().execute_with(|| {
        let owner = bob();
        let non_existent_sbt_id = XOR;

        // Attempt to update expiration date for a non-existent SBT
        assert_err!(
            ExtendedAssets::set_sbt_expiration(
                RuntimeOrigin::signed(owner.clone()),
                owner,
                non_existent_sbt_id,
                None
            ),
            Error::<TestRuntime>::SBTNotFound
        );
    });
}
