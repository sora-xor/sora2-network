use crate::migrations::{OldPollInfo, OldVotingInfo};
use crate::mock::*;
use crate::{pallet, Error};
use codec::Decode;
use codec::Encode;
use common::{
    balance, generate_storage_instance, AssetInfoProvider, BoundedString, CERES_ASSET_ID,
};
use frame_support::pallet_prelude::{StorageDoubleMap, StorageMap};
use frame_support::storage::types::ValueQuery;
use frame_support::traits::Hooks;
use frame_support::BoundedVec;
use frame_support::PalletId;
use frame_support::{assert_err, assert_ok, Identity};
use hex_literal::hex;
use sp_core::H256;
use sp_io::hashing::blake2_256;
use sp_runtime::traits::AccountIdConversion;

#[test]
fn create_poll_unauthorized_account() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_asset = CERES_ASSET_ID;
        let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let poll_end_timestamp = poll_start_timestamp + 10;
        let title = "Title";
        let description = "Description";
        let mut options = BoundedVec::default();

        options.try_push("Option 1".try_into().unwrap()).unwrap();
        options.try_push("Option 2".try_into().unwrap()).unwrap();
        options.try_push("Option 3".try_into().unwrap()).unwrap();

        assert_err!(
            CeresGovernancePlatform::create_poll(
                RuntimeOrigin::signed(ALICE),
                poll_asset,
                poll_start_timestamp,
                poll_end_timestamp,
                title.try_into().unwrap(),
                description.try_into().unwrap(),
                options
            ),
            Error::<Runtime>::Unauthorized
        );
    });
}

#[test]
fn create_poll_invalid_start_timestamp() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(1);

        let poll_asset = CERES_ASSET_ID;
        let poll_end_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let title = "Title";
        let description = "Description";
        let mut options = BoundedVec::default();

        options.try_push("Option 1".try_into().unwrap()).unwrap();
        options.try_push("Option 2".try_into().unwrap()).unwrap();
        options.try_push("Option 3".try_into().unwrap()).unwrap();

        assert_err!(
            CeresGovernancePlatform::create_poll(
                RuntimeOrigin::signed(CeresGovernancePlatform::authority_account()),
                poll_asset,
                poll_start_timestamp,
                poll_end_timestamp,
                title.try_into().unwrap(),
                description.try_into().unwrap(),
                options
            ),
            Error::<Runtime>::InvalidStartTimestamp
        );
    });
}

#[test]
fn create_poll_invalid_end_timestamp() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_end_timestamp = pallet_timestamp::Pallet::<Runtime>::get();

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(1);

        let poll_asset = CERES_ASSET_ID;
        let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let title = "Title";
        let description = "Description";
        let mut options = BoundedVec::default();

        options.try_push("Option 1".try_into().unwrap()).unwrap();
        options.try_push("Option 2".try_into().unwrap()).unwrap();
        options.try_push("Option 3".try_into().unwrap()).unwrap();

        assert_err!(
            CeresGovernancePlatform::create_poll(
                RuntimeOrigin::signed(CeresGovernancePlatform::authority_account()),
                poll_asset,
                poll_start_timestamp,
                poll_end_timestamp,
                title.try_into().unwrap(),
                description.try_into().unwrap(),
                options
            ),
            Error::<Runtime>::InvalidEndTimestamp
        );
    });
}

#[test]
fn create_poll_invalid_voting_options() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_asset = CERES_ASSET_ID;
        let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let poll_end_timestamp = poll_start_timestamp + 10;
        let title = "Title";
        let description = "Description";
        let mut options = BoundedVec::default();

        options.try_push("Option 1".try_into().unwrap()).unwrap();

        assert_err!(
            CeresGovernancePlatform::create_poll(
                RuntimeOrigin::signed(CeresGovernancePlatform::authority_account()),
                poll_asset,
                poll_start_timestamp,
                poll_end_timestamp,
                title.try_into().unwrap(),
                description.try_into().unwrap(),
                options
            ),
            Error::<Runtime>::InvalidVotingOptions
        );
    });
}

#[test]
fn create_poll_too_many_voting_options() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_asset = CERES_ASSET_ID;
        let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let poll_end_timestamp = poll_start_timestamp + 10;
        let title = "Title";
        let description = "Description";
        let mut options = BoundedVec::default();

        options.try_push("Option 1".try_into().unwrap()).unwrap();
        options.try_push("Option 2".try_into().unwrap()).unwrap();
        options.try_push("Option 3".try_into().unwrap()).unwrap();
        options.try_push("Option 4".try_into().unwrap()).unwrap();
        options.try_push("Option 5".try_into().unwrap()).unwrap();
        options.try_push("Option 6".try_into().unwrap()).unwrap();

        assert_err!(
            CeresGovernancePlatform::create_poll(
                RuntimeOrigin::signed(CeresGovernancePlatform::authority_account()),
                poll_asset,
                poll_start_timestamp,
                poll_end_timestamp,
                title.try_into().unwrap(),
                description.try_into().unwrap(),
                options
            ),
            Error::<Runtime>::TooManyVotingOptions
        );
    });
}

#[test]
fn create_poll_duplicate_options() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_asset = CERES_ASSET_ID;
        let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let poll_end_timestamp = poll_start_timestamp + 10;
        let title = "Title";
        let description = "Description";
        let mut options = BoundedVec::default();

        options.try_push("Option 1".try_into().unwrap()).unwrap();
        options.try_push("Option 2".try_into().unwrap()).unwrap();
        options.try_push("Option 3".try_into().unwrap()).unwrap();
        options.try_push("Option 3".try_into().unwrap()).unwrap();

        assert_err!(
            CeresGovernancePlatform::create_poll(
                RuntimeOrigin::signed(CeresGovernancePlatform::authority_account()),
                poll_asset,
                poll_start_timestamp,
                poll_end_timestamp,
                title.try_into().unwrap(),
                description.try_into().unwrap(),
                options
            ),
            Error::<Runtime>::DuplicateOptions
        );
    });
}

#[test]
fn create_poll_ok() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_asset = CERES_ASSET_ID;
        let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let poll_end_timestamp = poll_start_timestamp + 10;
        let title = "Title";
        let description = "Description";
        let mut options = BoundedVec::default();

        options.try_push("Option 1".try_into().unwrap()).unwrap();
        options.try_push("Option 2".try_into().unwrap()).unwrap();
        options.try_push("Option 3".try_into().unwrap()).unwrap();

        assert_ok!(CeresGovernancePlatform::create_poll(
            RuntimeOrigin::signed(CeresGovernancePlatform::authority_account()),
            poll_asset,
            poll_start_timestamp,
            poll_end_timestamp,
            title.try_into().unwrap(),
            description.try_into().unwrap(),
            options
        ));
    });
}

#[test]
fn vote_invalid_number_of_votes() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let user = ALICE;
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(user);
        let encoded: [u8; 32] = (&user, nonce).using_encoded(blake2_256);

        let poll_id = H256::from(encoded);
        let voting_option = "Voting option";
        let number_of_votes = balance!(0);

        assert_err!(
            CeresGovernancePlatform::vote(
                RuntimeOrigin::signed(ALICE),
                poll_id,
                voting_option.try_into().unwrap(),
                number_of_votes
            ),
            Error::<Runtime>::InvalidNumberOfVotes
        );
    });
}

#[test]
fn vote_poll_does_not_exist() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let user = ALICE;
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(user);
        let encoded: [u8; 32] = (&user, nonce).using_encoded(blake2_256);

        let poll_id = H256::from(encoded);
        let voting_option = "Voting option";
        let number_of_votes = balance!(100);

        assert_err!(
            CeresGovernancePlatform::vote(
                RuntimeOrigin::signed(ALICE),
                poll_id,
                voting_option.try_into().unwrap(),
                number_of_votes
            ),
            Error::<Runtime>::PollDoesNotExist
        );
    });
}

#[test]
fn vote_poll_is_not_started() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_asset = CERES_ASSET_ID;
        let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get() + 10;
        let poll_end_timestamp = poll_start_timestamp + 100;
        let title = "Title";
        let description = "Description";
        let mut options = BoundedVec::default();

        options.try_push("Option 1".try_into().unwrap()).unwrap();
        options.try_push("Option 2".try_into().unwrap()).unwrap();
        options.try_push("Option 3".try_into().unwrap()).unwrap();

        assert_ok!(CeresGovernancePlatform::create_poll(
            RuntimeOrigin::signed(CeresGovernancePlatform::authority_account()),
            poll_asset,
            poll_start_timestamp,
            poll_end_timestamp,
            title.try_into().unwrap(),
            description.try_into().unwrap(),
            options
        ));

        let user = CeresGovernancePlatform::authority_account();
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(user);
        let encoded: [u8; 32] = (&user, nonce).using_encoded(blake2_256);

        let poll_id = H256::from(encoded);
        let voting_option = "Option 1";
        let number_of_votes = balance!(100);

        assert_err!(
            CeresGovernancePlatform::vote(
                RuntimeOrigin::signed(ALICE),
                poll_id,
                voting_option.try_into().unwrap(),
                number_of_votes
            ),
            Error::<Runtime>::PollIsNotStarted
        );
    });
}

#[test]
fn vote_poll_is_finished() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_asset = CERES_ASSET_ID;
        let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get() + 10;
        let poll_end_timestamp = poll_start_timestamp + 100;
        let title = "Title";
        let description = "Description";
        let mut options = BoundedVec::default();

        options.try_push("Option 1".try_into().unwrap()).unwrap();
        options.try_push("Option 2".try_into().unwrap()).unwrap();
        options.try_push("Option 3".try_into().unwrap()).unwrap();

        assert_ok!(CeresGovernancePlatform::create_poll(
            RuntimeOrigin::signed(CeresGovernancePlatform::authority_account()),
            poll_asset,
            poll_start_timestamp,
            poll_end_timestamp,
            title.try_into().unwrap(),
            description.try_into().unwrap(),
            options
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(1000);

        let user = CeresGovernancePlatform::authority_account();
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(user);
        let encoded: [u8; 32] = (&user, nonce).using_encoded(blake2_256);

        let poll_id = H256::from(encoded);
        let voting_option = "Option 1";
        let number_of_votes = balance!(100);

        assert_err!(
            CeresGovernancePlatform::vote(
                RuntimeOrigin::signed(ALICE),
                poll_id,
                voting_option.try_into().unwrap(),
                number_of_votes
            ),
            Error::<Runtime>::PollIsFinished
        );
    });
}

#[test]
fn vote_invalid_option() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_asset = CERES_ASSET_ID;
        let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let poll_end_timestamp = poll_start_timestamp + 100;
        let title = "Title";
        let description = "Description";
        let mut options = BoundedVec::default();

        options.try_push("Option 1".try_into().unwrap()).unwrap();
        options.try_push("Option 2".try_into().unwrap()).unwrap();
        options.try_push("Option 3".try_into().unwrap()).unwrap();

        assert_ok!(CeresGovernancePlatform::create_poll(
            RuntimeOrigin::signed(CeresGovernancePlatform::authority_account()),
            poll_asset,
            poll_start_timestamp,
            poll_end_timestamp,
            title.try_into().unwrap(),
            description.try_into().unwrap(),
            options
        ));

        let user = CeresGovernancePlatform::authority_account();
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(user);
        let encoded: [u8; 32] = (&user, nonce).using_encoded(blake2_256);

        let poll_id = H256::from(encoded);
        let voting_option = "Option 5";
        let number_of_votes = balance!(100);

        assert_err!(
            CeresGovernancePlatform::vote(
                RuntimeOrigin::signed(ALICE),
                poll_id,
                voting_option.try_into().unwrap(),
                number_of_votes
            ),
            Error::<Runtime>::InvalidOption
        );
    });
}

#[test]
fn vote_denied() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_asset = CERES_ASSET_ID;
        let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let poll_end_timestamp = poll_start_timestamp + 100;
        let title = "Title";
        let description = "Description";
        let mut options = BoundedVec::default();

        options.try_push("Option 1".try_into().unwrap()).unwrap();
        options.try_push("Option 2".try_into().unwrap()).unwrap();
        options.try_push("Option 3".try_into().unwrap()).unwrap();

        assert_ok!(CeresGovernancePlatform::create_poll(
            RuntimeOrigin::signed(CeresGovernancePlatform::authority_account()),
            poll_asset,
            poll_start_timestamp,
            poll_end_timestamp,
            title.try_into().unwrap(),
            description.try_into().unwrap(),
            options
        ));

        let user = CeresGovernancePlatform::authority_account();
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(user);
        let encoded: [u8; 32] = (&user, nonce).using_encoded(blake2_256);

        let poll_id = H256::from(encoded);
        let first_voting_option = "Option 1";
        let number_of_votes = balance!(100);

        assert_ok!(CeresGovernancePlatform::vote(
            RuntimeOrigin::signed(ALICE),
            poll_id,
            first_voting_option.try_into().unwrap(),
            number_of_votes
        ));

        let second_voting_option = "Option 2";

        assert_err!(
            CeresGovernancePlatform::vote(
                RuntimeOrigin::signed(ALICE),
                poll_id,
                second_voting_option.try_into().unwrap(),
                number_of_votes
            ),
            Error::<Runtime>::VoteDenied
        );
    });
}

#[test]
fn vote_not_enough_funds() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_asset = CERES_ASSET_ID;
        let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let poll_end_timestamp = poll_start_timestamp + 100;
        let title = "Title";
        let description = "Description";
        let mut options = BoundedVec::default();

        options.try_push("Option 1".try_into().unwrap()).unwrap();
        options.try_push("Option 2".try_into().unwrap()).unwrap();
        options.try_push("Option 3".try_into().unwrap()).unwrap();

        assert_ok!(CeresGovernancePlatform::create_poll(
            RuntimeOrigin::signed(CeresGovernancePlatform::authority_account()),
            poll_asset,
            poll_start_timestamp,
            poll_end_timestamp,
            title.try_into().unwrap(),
            description.try_into().unwrap(),
            options
        ));

        let user = CeresGovernancePlatform::authority_account();
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(user);
        let encoded: [u8; 32] = (&user, nonce).using_encoded(blake2_256);

        let poll_id = H256::from(encoded);
        let voting_option = "Option 1";
        let number_of_votes = balance!(5100);

        assert_err!(
            CeresGovernancePlatform::vote(
                RuntimeOrigin::signed(ALICE),
                poll_id,
                voting_option.try_into().unwrap(),
                number_of_votes
            ),
            Error::<Runtime>::NotEnoughFunds
        );
    });
}

#[test]
fn vote_ok() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_asset = CERES_ASSET_ID;
        let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let poll_end_timestamp = poll_start_timestamp + 100;
        let title = "Title";
        let description = "Description";
        let mut options = BoundedVec::default();

        options.try_push("Option 1".try_into().unwrap()).unwrap();
        options.try_push("Option 2".try_into().unwrap()).unwrap();
        options.try_push("Option 3".try_into().unwrap()).unwrap();

        assert_ok!(CeresGovernancePlatform::create_poll(
            RuntimeOrigin::signed(CeresGovernancePlatform::authority_account()),
            poll_asset,
            poll_start_timestamp,
            poll_end_timestamp,
            title.try_into().unwrap(),
            description.try_into().unwrap(),
            options
        ));

        let user = CeresGovernancePlatform::authority_account();
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(user);
        let encoded: [u8; 32] = (&user, nonce).using_encoded(blake2_256);

        let poll_id = H256::from(encoded);
        let voting_option = "Option 1";
        let number_of_votes = balance!(100);

        assert_ok!(CeresGovernancePlatform::vote(
            RuntimeOrigin::signed(ALICE),
            poll_id,
            voting_option.try_into().unwrap(),
            number_of_votes
        ));
    });
}

#[test]
fn vote_multiple_times_ok() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_asset = CERES_ASSET_ID;
        let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let poll_end_timestamp = poll_start_timestamp + 100;
        let title = "Title";
        let description = "Description";
        let mut options = BoundedVec::default();

        options.try_push("Option 1".try_into().unwrap()).unwrap();
        options.try_push("Option 2".try_into().unwrap()).unwrap();
        options.try_push("Option 3".try_into().unwrap()).unwrap();

        assert_ok!(CeresGovernancePlatform::create_poll(
            RuntimeOrigin::signed(CeresGovernancePlatform::authority_account()),
            poll_asset,
            poll_start_timestamp,
            poll_end_timestamp,
            title.try_into().unwrap(),
            description.try_into().unwrap(),
            options
        ));

        let user = CeresGovernancePlatform::authority_account();
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(user);
        let encoded: [u8; 32] = (&user, nonce).using_encoded(blake2_256);

        let poll_id = H256::from(encoded);
        let voting_option = "Option 1";
        let number_of_votes = balance!(100);

        assert_ok!(CeresGovernancePlatform::vote(
            RuntimeOrigin::signed(ALICE),
            poll_id,
            voting_option.try_into().unwrap(),
            number_of_votes
        ));

        assert_ok!(CeresGovernancePlatform::vote(
            RuntimeOrigin::signed(ALICE),
            poll_id,
            voting_option.try_into().unwrap(),
            number_of_votes
        ));

        let pallet_account = PalletId(*b"ceresgov").into_account_truncating();
        assert_eq!(
            Assets::free_balance(&CERES_ASSET_ID, &pallet_account)
                .expect("Failed to query free balance."),
            balance!(200)
        );

        // Check ALICE of balance
        assert_eq!(
            Assets::free_balance(&CERES_ASSET_ID, &ALICE).expect("Failed to query free balance."),
            balance!(2800)
        );
    });
}

#[test]
fn withdraw_poll_does_not_exist() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let user = CeresGovernancePlatform::authority_account();
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(user);
        let encoded: [u8; 32] = (&user, nonce).using_encoded(blake2_256);
        let poll_id = H256::from(encoded);

        assert_err!(
            CeresGovernancePlatform::withdraw(RuntimeOrigin::signed(ALICE), poll_id),
            Error::<Runtime>::PollDoesNotExist
        );
    });
}

#[test]
fn withdraw_poll_is_not_finished() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_asset = CERES_ASSET_ID;
        let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let poll_end_timestamp = poll_start_timestamp + 100;
        let title = "Title";
        let description = "Description";
        let mut options = BoundedVec::default();

        options.try_push("Option 1".try_into().unwrap()).unwrap();
        options.try_push("Option 2".try_into().unwrap()).unwrap();
        options.try_push("Option 3".try_into().unwrap()).unwrap();

        assert_ok!(CeresGovernancePlatform::create_poll(
            RuntimeOrigin::signed(CeresGovernancePlatform::authority_account()),
            poll_asset,
            poll_start_timestamp,
            poll_end_timestamp,
            title.try_into().unwrap(),
            description.try_into().unwrap(),
            options
        ));

        let user = CeresGovernancePlatform::authority_account();
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(user);
        let encoded: [u8; 32] = (&user, nonce).using_encoded(blake2_256);

        let poll_id = H256::from(encoded);
        let voting_option = "Option 1";
        let number_of_votes = balance!(100);

        assert_ok!(CeresGovernancePlatform::vote(
            RuntimeOrigin::signed(ALICE),
            poll_id,
            voting_option.try_into().unwrap(),
            number_of_votes
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(99);

        assert_err!(
            CeresGovernancePlatform::withdraw(RuntimeOrigin::signed(ALICE), poll_id),
            Error::<Runtime>::PollIsNotFinished
        );
    });
}

#[test]
fn withdraw_not_voted() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_asset = CERES_ASSET_ID;
        let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let poll_end_timestamp = poll_start_timestamp + 100;
        let title = "Title";
        let description = "Description";
        let mut options = BoundedVec::default();

        options.try_push("Option 1".try_into().unwrap()).unwrap();
        options.try_push("Option 2".try_into().unwrap()).unwrap();
        options.try_push("Option 3".try_into().unwrap()).unwrap();

        assert_ok!(CeresGovernancePlatform::create_poll(
            RuntimeOrigin::signed(CeresGovernancePlatform::authority_account()),
            poll_asset,
            poll_start_timestamp,
            poll_end_timestamp,
            title.try_into().unwrap(),
            description.try_into().unwrap(),
            options
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(101);

        let user = CeresGovernancePlatform::authority_account();
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(user);
        let encoded: [u8; 32] = (&user, nonce).using_encoded(blake2_256);

        let poll_id = H256::from(encoded);

        assert_err!(
            CeresGovernancePlatform::withdraw(RuntimeOrigin::signed(ALICE), poll_id),
            Error::<Runtime>::NotVoted
        );
    });
}

#[test]
fn withdraw_funds_already_withdrawn() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_asset = CERES_ASSET_ID;
        let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let poll_end_timestamp = poll_start_timestamp + 100;
        let title = "Title";
        let description = "Description";
        let mut options = BoundedVec::default();

        options.try_push("Option 1".try_into().unwrap()).unwrap();
        options.try_push("Option 2".try_into().unwrap()).unwrap();
        options.try_push("Option 3".try_into().unwrap()).unwrap();

        assert_ok!(CeresGovernancePlatform::create_poll(
            RuntimeOrigin::signed(CeresGovernancePlatform::authority_account()),
            poll_asset,
            poll_start_timestamp,
            poll_end_timestamp,
            title.try_into().unwrap(),
            description.try_into().unwrap(),
            options
        ));

        let user = CeresGovernancePlatform::authority_account();
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(user);
        let encoded: [u8; 32] = (&user, nonce).using_encoded(blake2_256);

        let poll_id = H256::from(encoded);
        let voting_option = "Option 1";
        let number_of_votes = balance!(100);

        assert_ok!(CeresGovernancePlatform::vote(
            RuntimeOrigin::signed(ALICE),
            poll_id,
            voting_option.try_into().unwrap(),
            number_of_votes
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(101);

        assert_ok!(CeresGovernancePlatform::withdraw(
            RuntimeOrigin::signed(ALICE),
            poll_id
        ));

        assert_err!(
            CeresGovernancePlatform::withdraw(RuntimeOrigin::signed(ALICE), poll_id),
            Error::<Runtime>::FundsAlreadyWithdrawn
        );
    });
}

#[test]
fn withdraw_ok() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_asset = CERES_ASSET_ID;
        let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let poll_end_timestamp = poll_start_timestamp + 100;
        let title = "Title";
        let description = "Description";
        let mut options = BoundedVec::default();

        options.try_push("Option 1".try_into().unwrap()).unwrap();
        options.try_push("Option 2".try_into().unwrap()).unwrap();
        options.try_push("Option 3".try_into().unwrap()).unwrap();

        assert_ok!(CeresGovernancePlatform::create_poll(
            RuntimeOrigin::signed(CeresGovernancePlatform::authority_account()),
            poll_asset,
            poll_start_timestamp,
            poll_end_timestamp,
            title.try_into().unwrap(),
            description.try_into().unwrap(),
            options
        ));

        let user = CeresGovernancePlatform::authority_account();
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(user);
        let encoded: [u8; 32] = (&user, nonce).using_encoded(blake2_256);

        let poll_id = H256::from(encoded);
        let voting_option = "Option 1";
        let number_of_votes = balance!(100);

        assert_ok!(CeresGovernancePlatform::vote(
            RuntimeOrigin::signed(ALICE),
            poll_id,
            voting_option.try_into().unwrap(),
            number_of_votes
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(101);

        assert_ok!(CeresGovernancePlatform::withdraw(
            RuntimeOrigin::signed(ALICE),
            poll_id
        ));
    });
}

#[test]
fn ceres_governance_migration_works() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        generate_storage_instance!(CeresGovernancePlatform, PollData);
        generate_storage_instance!(CeresGovernancePlatform, Voting);

        let user = ALICE;
        let user1 = BOB;
        let old_poll_id_a = "16171D34600005D".as_bytes().to_vec();
        let old_poll_id_b = "16171D346000060".as_bytes().to_vec();
        let user_auth = pallet::AuthorityAccount::<Runtime>::get();
        let bytes = hex!("c4e7d5a63d8e887932bb6dc505dd204005c3ecfb6de5f1f0d3ac0a308b2b2915");
        let first_poll_creator = AccountId::decode(&mut &bytes[..]).unwrap();

        type OldPollData<Moment> =
            StorageMap<PollDataOldInstance, Identity, Vec<u8>, OldPollInfo<Moment>, ValueQuery>;

        type OldVoting = StorageDoubleMap<
            VotingOldInstance,
            Identity,
            Vec<u8>,
            Identity,
            AccountId,
            OldVotingInfo,
            ValueQuery,
        >;

        let old_voting_info_a = OldVotingInfo {
            voting_option: 1,
            number_of_votes: balance!(100),
            ceres_withdrawn: false,
        };

        let old_voting_info_b = OldVotingInfo {
            voting_option: 2,
            number_of_votes: balance!(100),
            ceres_withdrawn: false,
        };

        let old_voting_info_c = OldVotingInfo {
            voting_option: 2,
            number_of_votes: balance!(69),
            ceres_withdrawn: true,
        };

        let old_poll_info_a = OldPollInfo {
            number_of_options: 2,
            poll_start_timestamp: 1647612888000u64,
            poll_end_timestamp: 1647699288000u64,
        };

        let old_poll_info_b = OldPollInfo {
            number_of_options: 2,
            poll_start_timestamp: 1648804056000u64,
            poll_end_timestamp: 1648890456000u64,
        };

        OldPollData::insert(&old_poll_id_a, old_poll_info_a);
        OldPollData::insert(&old_poll_id_b, old_poll_info_b);

        OldVoting::insert(&old_poll_id_a, user, old_voting_info_a);
        OldVoting::insert(&old_poll_id_a, user1, old_voting_info_c);
        OldVoting::insert(&old_poll_id_b, user1, old_voting_info_b);

        run_to_block(5);

        pallet::PalletStorageVersion::<Runtime>::put(crate::StorageVersion::V2);
        assert_eq!(
            pallet::Pallet::<Runtime>::pallet_storage_version(),
            crate::StorageVersion::V2
        );

        //Storage migration
        pallet::Pallet::<Runtime>::on_runtime_upgrade();

        assert_eq!(
            pallet::Pallet::<Runtime>::pallet_storage_version(),
            crate::StorageVersion::V3
        );

        let nonce_a: <Runtime as frame_system::Config>::Index = 305u32.into();
        let encoded = (&first_poll_creator, nonce_a).using_encoded(blake2_256);
        let poll_id_a = H256::from(encoded);

        let nonce_b: <Runtime as frame_system::Config>::Index = 15u32.into();
        let encoded = (&user_auth, nonce_b).using_encoded(blake2_256);
        let poll_id_b = H256::from(encoded);

        let poll_a = pallet::PollData::<Runtime>::get(poll_id_a).unwrap();
        let poll_b = pallet::PollData::<Runtime>::get(poll_id_b).unwrap();

        assert_eq!(poll_a.poll_asset, CERES_ASSET_ID);
        assert_eq!(poll_a.poll_start_timestamp, 1647612888000u64);
        assert_eq!(poll_a.poll_end_timestamp, 1647699288000u64);
        assert_eq!(poll_a.title, BoundedString::truncate_from(
                    "Do you want Ceres staking v2 with rewards pool of 300 CERES to go live?"));
        assert_eq!(poll_a.description, BoundedString::truncate_from(
            "The Ceres v2 staking pool would have 300 CERES rewards taken from the Ceres Treasury wallet. Staking would have a 14,400 CERES pool limit and would last a month and a half with minimum APR 16.66%."));  
        assert_eq!(poll_a.options, vec![BoundedString::truncate_from("Yes"), BoundedString::truncate_from("No")]);  

        assert_eq!(poll_b.poll_asset, CERES_ASSET_ID);
        assert_eq!(poll_b.poll_start_timestamp, 1648804056000u64);
        assert_eq!(poll_b.poll_end_timestamp, 1648890456000u64);
        assert_eq!(poll_b.title, BoundedString::truncate_from(
            "Can Launchpad costs be paid from the Treasury wallet?"));  
        assert_eq!(poll_b.description, BoundedString::truncate_from("Ceres Launchpad is coming soon with new SORA runtime release. Launchpad requires KYC services which should be paid (about $11,740)."));  
        assert_eq!(poll_b.options, vec![BoundedString::truncate_from("Yes"), BoundedString::truncate_from("No")]);

        let voting_a = pallet::Voting::<Runtime>::get(poll_id_a, user).unwrap();
        let voting_b = pallet::Voting::<Runtime>::get(poll_id_a, user1).unwrap();
        let voting_c = pallet::Voting::<Runtime>::get(poll_id_b, user1).unwrap();

        assert_eq!(voting_a.voting_option, BoundedString::truncate_from("Yes"));
        assert_eq!(voting_b.voting_option, BoundedString::truncate_from("No"));
        assert_eq!(voting_c.voting_option, BoundedString::truncate_from("No"));
        assert_eq!(voting_a.number_of_votes, balance!(100));
        assert_eq!(voting_b.number_of_votes, balance!(69));
        assert_eq!(voting_c.number_of_votes, balance!(100));
        assert!(!voting_a.asset_withdrawn);
        assert!(voting_b.asset_withdrawn);
        assert!(!voting_c.asset_withdrawn);
    });
}
