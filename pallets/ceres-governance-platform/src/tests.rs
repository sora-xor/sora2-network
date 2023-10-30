use crate::{mock::*, Voting};
use crate::{pallet, Error};
use codec::Encode;
use common::{
    balance, generate_storage_instance, AssetInfoProvider, BoundedString, CERES_ASSET_ID,
};
use frame_support::pallet_prelude::StorageMap;
use frame_support::storage::types::ValueQuery;
use frame_support::traits::Hooks;
use frame_support::BoundedVec;
use frame_support::PalletId;
use frame_support::{assert_err, assert_ok, Identity};
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
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(&user);
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
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(&user);
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
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(&user);
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
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(&user);
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
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(&user);
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
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(&user);
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
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(&user);
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
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(&user);
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
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(&user);
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
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(&user);
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
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(&user);
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
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(&user);
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
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(&user);
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
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(&user);
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
