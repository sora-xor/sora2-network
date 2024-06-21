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

use crate::mock::*;
use crate::*;
use common::{Balance, TechAccountId, DEFAULT_BALANCE_PRECISION, XOR};
use frame_support::{assert_err, assert_ok};
use mock::Timestamp;
use permissions::MINT;
use sp_core::crypto::AccountId32;

type RegulatedAssets = Pallet<TestRuntime>;

pub fn alice() -> AccountId32 {
    AccountId32::from([1; 32])
}

pub fn bob() -> AccountId32 {
    AccountId32::from([2; 32])
}

pub fn add_asset<T: Config>(owner: &T::AccountId) -> AssetIdOf<T> {
    frame_system::Pallet::<T>::inc_providers(owner);

    T::AssetManager::register_from(
        owner,
        AssetSymbol(b"TOKEN".to_vec()),
        AssetName(b"TOKEN".to_vec()),
        DEFAULT_BALANCE_PRECISION,
        Balance::from(0u32),
        true,
        None,
        None,
    )
    .expect("Failed to register asset")
}

fn get_sbt_id_from_events<T: Config>() -> AssetIdOf<T> {
    // Extract the issued SBT asset ID
    let event = frame_system::Pallet::<TestRuntime>::events()
        .pop()
        .expect("Expected at least one event")
        .event;
    let sbt_asset_id = match event {
        RuntimeEvent::RegulatedAssets(crate::Event::SoulboundTokenIssued { asset_id, .. }) => {
            asset_id
        }
        _ => panic!("Unexpected event: {:?}", event),
    };
    sbt_asset_id.into()
}

#[test]
fn test_default_value_asset_regulated() {
    new_test_ext().execute_with(|| {
        let default_value = RegulatedAssets::regulated_asset(XOR);
        assert!(!default_value);
    })
}

#[test]
fn test_cannot_regulate_already_regulated_asset() {
    new_test_ext().execute_with(|| {
        let owner = bob();
        let asset_id = add_asset::<TestRuntime>(&owner);

        // Regulate the asset for the first time
        assert_ok!(RegulatedAssets::regulate_asset(
            RuntimeOrigin::signed(owner.clone()),
            asset_id
        ));

        // Try to regulate the already regulated asset
        assert_err!(
            RegulatedAssets::regulate_asset(RuntimeOrigin::signed(owner), asset_id),
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
        assert_ok!(RegulatedAssets::regulate_asset(
            RuntimeOrigin::signed(owner),
            asset_id
        ));

        mock::Technical::register_tech_account_id(tech_account.clone()).unwrap();
        let tech_account_id =
            mock::Technical::tech_account_id_to_account_id(&tech_account).unwrap();

        // Tech account can pass permission check for unregulated asset
        assert_ok!(RegulatedAssets::check_permission(
            &tech_account_id,
            &tech_account_id,
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
        assert_ok!(RegulatedAssets::check_permission(
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
            RegulatedAssets::regulate_asset(RuntimeOrigin::signed(non_owner), asset_id),
            Error::<TestRuntime>::OnlyAssetOwnerCanRegulate
        );

        // Owner can regulate asset
        assert_ok!(RegulatedAssets::regulate_asset(
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
        let bounded_vec_assets = BoundedVec::try_from(vec![asset_id]).unwrap();
        assert_ok!(RegulatedAssets::regulate_asset(
            RuntimeOrigin::signed(owner.clone()),
            asset_id
        ));

        frame_system::Pallet::<TestRuntime>::inc_providers(&owner);
        // Owner can issue SBT
        assert_ok!(RegulatedAssets::issue_sbt(
            RuntimeOrigin::signed(owner),
            asset_symbol,
            asset_name,
            None,
            None,
            None,
            None,
            bounded_vec_assets,
        ));
    })
}

#[test]
fn test_issue_sbt_fails_due_to_invalid_expires_at() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let owner = bob();
        let asset_name = AssetName(b"Soulbound Token".to_vec());
        let asset_symbol = AssetSymbol(b"SBT".to_vec());

        let now_timestamp = Timestamp::now();

        let result_invalid_expires_at = RegulatedAssets::issue_sbt(
            RuntimeOrigin::signed(owner),
            asset_symbol,
            asset_name,
            None,
            None,
            None,
            Some(now_timestamp.saturating_sub(1)),
            BoundedVec::try_from(vec![XOR]).unwrap(),
        );
        assert_err!(
            result_invalid_expires_at,
            Error::<TestRuntime>::SBTExpirationDateCannotBeInThePast
        );
    });
}

#[test]
fn test_issue_sbt_fails_due_to_invalid_allowed_assets() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let owner = bob();
        let asset_name = AssetName(b"Soulbound Token".to_vec());
        let asset_symbol = AssetSymbol(b"SBT".to_vec());

        let now_timestamp = Timestamp::now();

        let result_invalid_allowed_asset_not_onwer = RegulatedAssets::issue_sbt(
            RuntimeOrigin::signed(owner.clone()),
            asset_symbol.clone(),
            asset_name.clone(),
            None,
            None,
            None,
            Some(now_timestamp.saturating_add(10)),
            BoundedVec::try_from(vec![XOR]).unwrap(),
        );
        assert_err!(
            result_invalid_allowed_asset_not_onwer,
            Error::<TestRuntime>::AllowedAssetsMustBeOwnedBySBTIssuer
        );

        let asset_id = add_asset::<TestRuntime>(&owner);
        let bounded_vec_assets = BoundedVec::try_from(vec![asset_id]).unwrap();

        let result_invalid_allowed_asset_unregulated = RegulatedAssets::issue_sbt(
            RuntimeOrigin::signed(owner),
            asset_symbol,
            asset_name,
            None,
            None,
            None,
            Some(now_timestamp.saturating_add(10)),
            bounded_vec_assets,
        );
        assert_err!(
            result_invalid_allowed_asset_unregulated,
            Error::<TestRuntime>::AllowedAssetsMustBeRegulated
        );
    });
}

#[test]
fn test_sbt_only_operationable_by_its_owner() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let owner = bob();
        let non_owner = alice();
        let asset_name = AssetName(b"Soulbound Token".to_vec());
        let asset_symbol = AssetSymbol(b"SBT".to_vec());

        let asset_id = add_asset::<TestRuntime>(&owner);
        let bounded_vec_assets = BoundedVec::try_from(vec![asset_id]).unwrap();
        assert_ok!(RegulatedAssets::regulate_asset(
            RuntimeOrigin::signed(owner.clone()),
            asset_id
        ));

        frame_system::Pallet::<TestRuntime>::inc_providers(&owner);
        // Issue SBT
        assert_ok!(RegulatedAssets::issue_sbt(
            RuntimeOrigin::signed(owner.clone()),
            asset_symbol,
            asset_name,
            None,
            None,
            None,
            None,
            bounded_vec_assets,
        ));
        let sbt_asset_id = get_sbt_id_from_events::<TestRuntime>();

        // SBT operations by non-owner should fail
        assert_err!(
            RegulatedAssets::check_permission(&non_owner, &non_owner, &sbt_asset_id, &TRANSFER),
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
        let asset_name = AssetName(b"Soulbound Token".to_vec());
        let asset_symbol = AssetSymbol(b"SBT".to_vec());

        let asset_id = add_asset::<TestRuntime>(&owner);
        let bounded_vec_assets = BoundedVec::try_from(vec![asset_id]).unwrap();
        assert_ok!(RegulatedAssets::regulate_asset(
            RuntimeOrigin::signed(owner.clone()),
            asset_id
        ));

        frame_system::Pallet::<TestRuntime>::inc_providers(&owner);
        // Owner can issue SBT
        assert_ok!(RegulatedAssets::issue_sbt(
            RuntimeOrigin::signed(owner.clone()),
            asset_symbol,
            asset_name,
            None,
            None,
            None,
            None,
            bounded_vec_assets,
        ));

        let sbt_asset_id = get_sbt_id_from_events::<TestRuntime>();

        assert_err!(
            Assets::transfer(RuntimeOrigin::signed(owner), sbt_asset_id, non_owner, 1),
            Error::<TestRuntime>::SoulboundAssetNotTransferable
        );
    })
}

#[test]
fn test_check_permission_pass_only_if_all_invloved_accounts_have_valid_sbt() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let owner = bob();
        let non_owner = alice();
        let another_account = AccountId32::from([3u8; 32]);
        let asset_name = AssetName(b"Soulbound Token".to_vec());
        let asset_symbol = AssetSymbol(b"SBT".to_vec());

        // Regulate an asset
        let asset_id = add_asset::<TestRuntime>(&owner);
        assert_ok!(RegulatedAssets::regulate_asset(
            RuntimeOrigin::signed(owner.clone()),
            asset_id
        ));

        let bounded_vec_assets = BoundedVec::try_from(vec![asset_id]).unwrap();

        // Issue SBT
        let result = RegulatedAssets::issue_sbt(
            RuntimeOrigin::signed(owner.clone()),
            asset_symbol,
            asset_name,
            None,
            None,
            None,
            None,
            bounded_vec_assets,
        );
        assert_ok!(result);

        let sbt_asset_id = get_sbt_id_from_events::<TestRuntime>();

        // Give SBT to another_account
        assert_ok!(Assets::mint_to(&sbt_asset_id, &owner, &another_account, 1));
        assert_ok!(Assets::mint_to(&sbt_asset_id, &owner, &owner, 1));

        // Check permission should pass only if all involved accounts have SBT
        assert_ok!(RegulatedAssets::check_permission(
            &owner,
            &another_account,
            &asset_id,
            &MINT
        ));
        assert_err!(
            RegulatedAssets::check_permission(&owner, &non_owner, &asset_id, &MINT),
            Error::<TestRuntime>::AllInvolvedUsersShouldHoldValidSBT
        );
        assert_err!(
            RegulatedAssets::check_permission(&non_owner, &another_account, &asset_id, &MINT),
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
            RegulatedAssets::check_permission(&owner, &another_account, &asset_id, &MINT),
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
        let asset_name = AssetName(b"Soulbound Token".to_vec());
        let asset_symbol = AssetSymbol(b"SBT".to_vec());
        let expiration_timestamp = Timestamp::now().saturating_add(100);
        let later_expiration_timestamp = Timestamp::now().saturating_add(200);
        // Regulate an asset
        let asset_id = add_asset::<TestRuntime>(&owner);
        assert_ok!(RegulatedAssets::regulate_asset(
            RuntimeOrigin::signed(owner.clone()),
            asset_id
        ));

        let bounded_vec_assets = BoundedVec::try_from(vec![asset_id]).unwrap();

        // Issue SBT
        let result_sbt_soon_expires = RegulatedAssets::issue_sbt(
            RuntimeOrigin::signed(owner.clone()),
            asset_symbol.clone(),
            asset_name.clone(),
            None,
            None,
            None,
            Some(expiration_timestamp),
            bounded_vec_assets.clone(),
        );
        assert_ok!(result_sbt_soon_expires);

        let soon_expires_sbt_asset_id = get_sbt_id_from_events::<TestRuntime>();

        // Issue SBT
        let result_sbt_expires_later = RegulatedAssets::issue_sbt(
            RuntimeOrigin::signed(owner.clone()),
            asset_symbol,
            asset_name,
            None,
            None,
            None,
            Some(later_expiration_timestamp),
            bounded_vec_assets,
        );
        assert_ok!(result_sbt_expires_later);

        let expires_later_sbt_asset_id = get_sbt_id_from_events::<TestRuntime>();

        // Give SBT to another_account
        assert_ok!(Assets::mint_to(
            &expires_later_sbt_asset_id,
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

        // Check permission should pass only if all involved accounts have SBT
        // before expiration happens
        assert_ok!(RegulatedAssets::check_permission(
            &owner,
            &another_account,
            &asset_id,
            &TRANSFER
        ));

        // Move time to make sure the SBT(marked as soon_expires) has expired
        Timestamp::set_timestamp(expiration_timestamp.saturating_add(1));

        assert_err!(
            RegulatedAssets::check_permission(&owner, &another_account, &asset_id, &TRANSFER),
            Error::<TestRuntime>::AllInvolvedUsersShouldHoldValidSBT
        );
    })
}

#[test]
fn test_update_sbt_expiration_succeeds() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let owner = bob();
        let asset_name = AssetName(b"Soulbound Token".to_vec());
        let asset_symbol = AssetSymbol(b"SBT".to_vec());
        let expiration_timestamp = Timestamp::now().saturating_add(100);
        let new_expiration_timestamp = expiration_timestamp.saturating_add(100);

        let asset_id = add_asset::<TestRuntime>(&owner);
        assert_ok!(RegulatedAssets::regulate_asset(
            RuntimeOrigin::signed(owner.clone()),
            asset_id
        ));

        let bounded_vec_assets = BoundedVec::try_from(vec![asset_id]).unwrap();

        // Issue SBT
        assert_ok!(RegulatedAssets::issue_sbt(
            RuntimeOrigin::signed(owner.clone()),
            asset_symbol,
            asset_name,
            None,
            None,
            None,
            Some(expiration_timestamp),
            bounded_vec_assets,
        ));

        let sbt_asset_id = get_sbt_id_from_events::<TestRuntime>();
        // Update expiration date
        assert_ok!(RegulatedAssets::update_sbt_expiration(
            RuntimeOrigin::signed(owner),
            sbt_asset_id,
            Some(new_expiration_timestamp)
        ));

        let metadata = RegulatedAssets::soulbound_asset(sbt_asset_id).unwrap();
        assert_eq!(metadata.expires_at, Some(new_expiration_timestamp));
    });
}

#[test]
fn test_update_sbt_expiration_fails_for_non_owner() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let owner = bob();
        let non_owner = alice();
        let asset_name = AssetName(b"Soulbound Token".to_vec());
        let asset_symbol = AssetSymbol(b"SBT".to_vec());
        let expiration_timestamp = Timestamp::now().saturating_add(100);
        let new_expiration_timestamp = expiration_timestamp.saturating_add(100);

        let asset_id = add_asset::<TestRuntime>(&owner);
        assert_ok!(RegulatedAssets::regulate_asset(
            RuntimeOrigin::signed(owner.clone()),
            asset_id
        ));

        let bounded_vec_assets = BoundedVec::try_from(vec![asset_id]).unwrap();

        // Issue SBT
        assert_ok!(RegulatedAssets::issue_sbt(
            RuntimeOrigin::signed(owner),
            asset_symbol,
            asset_name,
            None,
            None,
            None,
            Some(expiration_timestamp),
            bounded_vec_assets,
        ));

        // Attempt to update expiration date by non-owner
        assert_err!(
            RegulatedAssets::update_sbt_expiration(
                RuntimeOrigin::signed(non_owner),
                get_sbt_id_from_events::<TestRuntime>(),
                Some(new_expiration_timestamp)
            ),
            Error::<TestRuntime>::NotSBTOwner
        );
    });
}

#[test]
fn test_update_sbt_expiration_fails_for_past_timestamp() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let owner = bob();
        let asset_name = AssetName(b"Soulbound Token".to_vec());
        let asset_symbol = AssetSymbol(b"SBT".to_vec());
        let expiration_timestamp = Timestamp::now().saturating_add(100);
        let new_expiration_timestamp = Timestamp::now().saturating_sub(1);

        let asset_id = add_asset::<TestRuntime>(&owner);
        assert_ok!(RegulatedAssets::regulate_asset(
            RuntimeOrigin::signed(owner.clone()),
            asset_id
        ));

        let bounded_vec_assets = BoundedVec::try_from(vec![asset_id]).unwrap();

        // Issue SBT
        assert_ok!(RegulatedAssets::issue_sbt(
            RuntimeOrigin::signed(owner.clone()),
            asset_symbol,
            asset_name,
            None,
            None,
            None,
            Some(expiration_timestamp),
            bounded_vec_assets,
        ));

        // Attempt to update expiration date with a past timestamp
        assert_err!(
            RegulatedAssets::update_sbt_expiration(
                RuntimeOrigin::signed(owner),
                get_sbt_id_from_events::<TestRuntime>(),
                Some(new_expiration_timestamp)
            ),
            Error::<TestRuntime>::SBTExpirationDateCannotBeInThePast
        );
    });
}

#[test]
fn test_update_sbt_expiration_fails_for_non_existent_sbt() {
    new_test_ext().execute_with(|| {
        let owner = bob();
        let non_existent_sbt_id = XOR;

        // Attempt to update expiration date for a non-existent SBT
        assert_err!(
            RegulatedAssets::update_sbt_expiration(
                RuntimeOrigin::signed(owner),
                non_existent_sbt_id,
                None
            ),
            Error::<TestRuntime>::SBTNotFound
        );
    });
}
