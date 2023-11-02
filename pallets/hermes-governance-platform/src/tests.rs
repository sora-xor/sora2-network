use crate::migrations::{OldHermesPollInfo, VotingOption};
use crate::mock::*;
use crate::AccountIdOf;
use crate::{pallet, Error, HermesPollInfo};
use codec::Encode;
use common::{balance, generate_storage_instance, AssetInfoProvider, HERMES_ASSET_ID};
use frame_support::pallet_prelude::{StorageDoubleMap, StorageMap};
use frame_support::storage::types::OptionQuery;
use frame_support::traits::Hooks;
use frame_support::BoundedVec;
use frame_support::PalletId;
use frame_support::{assert_err, assert_ok, Identity};
use sp_core::H256;
use sp_io::hashing::blake2_256;
use sp_runtime::traits::AccountIdConversion;

#[test]
fn create_poll_invalid_start_timestamp() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let title = "Title";
        let description = "Description";
        let mut options = BoundedVec::default();
        options.try_push("Option 1".try_into().unwrap()).unwrap();
        options.try_push("Option 2".try_into().unwrap()).unwrap();
        options.try_push("Option 3".try_into().unwrap()).unwrap();

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 1);

        assert_err!(
            HermesGovernancePlatform::create_poll(
                RuntimeOrigin::signed(ALICE),
                current_timestamp,
                current_timestamp + 10,
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
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let title = "Title";
        let description = "Description";
        let mut options = BoundedVec::default();
        options.try_push("Option 1".try_into().unwrap()).unwrap();
        options.try_push("Option 2".try_into().unwrap()).unwrap();
        options.try_push("Option 3".try_into().unwrap()).unwrap();

        assert_err!(
            HermesGovernancePlatform::create_poll(
                RuntimeOrigin::signed(ALICE),
                current_timestamp + 1,
                current_timestamp,
                title.try_into().unwrap(),
                description.try_into().unwrap(),
                options
            ),
            Error::<Runtime>::InvalidEndTimestamp
        );
    });
}

#[test]
fn create_poll_invalid_minimum_duration_of_poll() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let title = "Title";
        let description = "Description";
        let mut options = BoundedVec::default();
        options.try_push("Option 1".try_into().unwrap()).unwrap();
        options.try_push("Option 2".try_into().unwrap()).unwrap();

        assert_err!(
            HermesGovernancePlatform::create_poll(
                RuntimeOrigin::signed(ALICE),
                current_timestamp,
                current_timestamp + 15,
                title.try_into().unwrap(),
                description.try_into().unwrap(),
                options
            ),
            Error::<Runtime>::InvalidMinimumDurationOfPoll
        );
    });
}

#[test]
fn create_poll_invalid_maximum_duration_of_poll() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let title = "Title";
        let description = "Description";
        let mut options = BoundedVec::default();
        options.try_push("Option 1".try_into().unwrap()).unwrap();
        options.try_push("Option 2".try_into().unwrap()).unwrap();

        assert_err!(
            HermesGovernancePlatform::create_poll(
                RuntimeOrigin::signed(ALICE),
                current_timestamp,
                current_timestamp + 604800001,
                title.try_into().unwrap(),
                description.try_into().unwrap(),
                options
            ),
            Error::<Runtime>::InvalidMaximumDurationOfPoll
        );
    });
}

#[test]
fn create_poll_not_enough_hermes_for_creating_poll() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let title = "Title";
        let description = "Description";
        let mut options = BoundedVec::default();
        options.try_push("Option 1".try_into().unwrap()).unwrap();
        options.try_push("Option 2".try_into().unwrap()).unwrap();
        options.try_push("Option 3".try_into().unwrap()).unwrap();
        options.try_push("Option 4".try_into().unwrap()).unwrap();

        assert_err!(
            HermesGovernancePlatform::create_poll(
                RuntimeOrigin::signed(BOB),
                current_timestamp,
                current_timestamp + 14_400_000,
                title.try_into().unwrap(),
                description.try_into().unwrap(),
                options
            ),
            Error::<Runtime>::NotEnoughHermesForCreatingPoll
        );
    });
}

#[test]
fn create_poll_invalid_voting_options() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let title = "Title";
        let description = "Description";
        let mut options = BoundedVec::default();
        options.try_push("Option 1".try_into().unwrap()).unwrap();

        assert_err!(
            HermesGovernancePlatform::create_poll(
                RuntimeOrigin::signed(ALICE),
                current_timestamp,
                current_timestamp + 14_400_000,
                title.try_into().unwrap(),
                description.try_into().unwrap(),
                options
            ),
            Error::<Runtime>::InvalidVotingOptions
        );
    });
}

#[test]
fn create_poll_duplicate_options() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let title = "Title";
        let description = "Description";
        let mut options = BoundedVec::default();
        options.try_push("Option 1".try_into().unwrap()).unwrap();
        options.try_push("Option 2".try_into().unwrap()).unwrap();
        options.try_push("Option 1".try_into().unwrap()).unwrap();

        assert_err!(
            HermesGovernancePlatform::create_poll(
                RuntimeOrigin::signed(ALICE),
                current_timestamp,
                current_timestamp + 14_400_000,
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
        let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let poll_end_timestamp = pallet_timestamp::Pallet::<Runtime>::get() + 14_400_000;
        let user = RuntimeOrigin::signed(ALICE);
        let hermes_locked = pallet::MinimumHermesAmountForCreatingPoll::<Runtime>::get();
        let title = "Title";
        let description = "Description";
        let mut options = BoundedVec::default();
        options.try_push("Option 1".try_into().unwrap()).unwrap();
        options.try_push("Option 2".try_into().unwrap()).unwrap();
        options.try_push("Option 3".try_into().unwrap()).unwrap();
        options.try_push("Option 4".try_into().unwrap()).unwrap();
        options.try_push("Option 5".try_into().unwrap()).unwrap();

        assert_ok!(HermesGovernancePlatform::create_poll(
            user,
            poll_start_timestamp,
            poll_end_timestamp,
            title.try_into().unwrap(),
            description.try_into().unwrap(),
            options
        ));

        for (_, p_info) in pallet::HermesPollData::<Runtime>::iter() {
            assert_eq!(p_info.poll_start_timestamp, poll_start_timestamp);
            assert_eq!(p_info.poll_end_timestamp, poll_end_timestamp);
            assert!(!p_info.creator_hermes_withdrawn);
            assert_eq!(p_info.hermes_locked, hermes_locked);
        }

        // Check ALICE's balances
        assert_eq!(
            Assets::free_balance(&HERMES_ASSET_ID, &ALICE).expect("Failed to query free balance."),
            balance!(200000)
        );

        // Check pallet's balances
        let pallet_account = PalletId(*b"hermsgov").into_account_truncating();
        assert_eq!(
            Assets::free_balance(&HERMES_ASSET_ID, &pallet_account)
                .expect("Failed to query free balance."),
            hermes_locked
        );
    });
}

#[test]
fn vote_poll_does_not_exist() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let user = ALICE;
        let voting_option = "Option 1".try_into().unwrap();

        let nonce = frame_system::Pallet::<Runtime>::account_nonce(user);
        let encoded: [u8; 32] = (&user, nonce).using_encoded(blake2_256);
        let poll_id = H256::from(encoded);

        assert_err!(
            HermesGovernancePlatform::vote(RuntimeOrigin::signed(ALICE), poll_id, voting_option,),
            Error::<Runtime>::PollDoesNotExist
        );
    });
}

#[test]
fn vote_poll_is_not_started() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get() + 1;
        let poll_end_timestamp = pallet_timestamp::Pallet::<Runtime>::get() + 14_400_000;
        let user = ALICE;
        let hermes_locked = pallet::MinimumHermesAmountForCreatingPoll::<Runtime>::get();
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(user);
        let encoded: [u8; 32] = (&user, nonce).using_encoded(blake2_256);
        let poll_id = H256::from(encoded);
        let voting_option = "Option 1".try_into().unwrap();
        let mut options = BoundedVec::default();
        options.try_push("Option 1".try_into().unwrap()).unwrap();
        options.try_push("Option 2".try_into().unwrap()).unwrap();
        options.try_push("Option 3".try_into().unwrap()).unwrap();
        options.try_push("Option 4".try_into().unwrap()).unwrap();
        options.try_push("Option 5".try_into().unwrap()).unwrap();

        let hermes_poll_info = HermesPollInfo {
            creator: user,
            hermes_locked,
            poll_start_timestamp,
            poll_end_timestamp,
            title: "Title".try_into().unwrap(),
            description: "Description".try_into().unwrap(),
            creator_hermes_withdrawn: false,
            options,
        };

        pallet::HermesPollData::<Runtime>::insert(poll_id, hermes_poll_info);

        assert_err!(
            HermesGovernancePlatform::vote(RuntimeOrigin::signed(ALICE), poll_id, voting_option),
            Error::<Runtime>::PollIsNotStarted
        );
    });
}

#[test]
fn vote_poll_is_finished() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let poll_end_timestamp = pallet_timestamp::Pallet::<Runtime>::get() + 604800000;
        let user = ALICE;
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let hermes_locked = pallet::MinimumHermesAmountForCreatingPoll::<Runtime>::get();
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(user);
        let encoded: [u8; 32] = (&user, nonce).using_encoded(blake2_256);
        let poll_id = H256::from(encoded);
        let voting_option = "Option 1".try_into().unwrap();
        let mut options = BoundedVec::default();
        options.try_push("Option 1".try_into().unwrap()).unwrap();
        options.try_push("Option 2".try_into().unwrap()).unwrap();
        options.try_push("Option 3".try_into().unwrap()).unwrap();

        let hermes_poll_info = HermesPollInfo {
            creator: user,
            hermes_locked,
            poll_start_timestamp,
            poll_end_timestamp,
            title: "Title".try_into().unwrap(),
            description: "Description".try_into().unwrap(),
            creator_hermes_withdrawn: false,
            options,
        };

        pallet::HermesPollData::<Runtime>::insert(poll_id, hermes_poll_info);
        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 604800001);

        assert_err!(
            HermesGovernancePlatform::vote(RuntimeOrigin::signed(ALICE), poll_id, voting_option,),
            Error::<Runtime>::PollIsFinished
        );
    });
}

#[test]
fn vote_invalid_option() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let poll_end_timestamp = pallet_timestamp::Pallet::<Runtime>::get() + 604800000;
        let user = ALICE;
        let hermes_locked = pallet::MinimumHermesAmountForCreatingPoll::<Runtime>::get();
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(user);
        let encoded: [u8; 32] = (&user, nonce).using_encoded(blake2_256);
        let poll_id = H256::from(encoded);
        let voting_option = "Option 5".try_into().unwrap();
        let mut options = BoundedVec::default();
        options.try_push("Option 1".try_into().unwrap()).unwrap();
        options.try_push("Option 2".try_into().unwrap()).unwrap();
        options.try_push("Option 3".try_into().unwrap()).unwrap();

        let hermes_poll_info = HermesPollInfo {
            creator: user,
            hermes_locked,
            poll_start_timestamp,
            poll_end_timestamp,
            title: "Title".try_into().unwrap(),
            description: "Description".try_into().unwrap(),
            creator_hermes_withdrawn: false,
            options,
        };

        pallet::HermesPollData::<Runtime>::insert(poll_id, hermes_poll_info);

        assert_err!(
            HermesGovernancePlatform::vote(RuntimeOrigin::signed(ALICE), poll_id, voting_option,),
            Error::<Runtime>::InvalidOption
        );
    });
}

#[test]
fn vote_not_enough_hermes_for_voting() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let poll_end_timestamp = pallet_timestamp::Pallet::<Runtime>::get() + 604800000;
        let hermes_locked = pallet::MinimumHermesAmountForCreatingPoll::<Runtime>::get();
        let user = ALICE;
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(user);
        let encoded: [u8; 32] = (&user, nonce).using_encoded(blake2_256);
        let poll_id = H256::from(encoded);
        let voting_option = "Option 1".try_into().unwrap();
        let mut options = BoundedVec::default();
        options.try_push("Option 1".try_into().unwrap()).unwrap();
        options.try_push("Option 2".try_into().unwrap()).unwrap();

        let hermes_poll_info = HermesPollInfo {
            creator: user,
            hermes_locked,
            poll_start_timestamp,
            poll_end_timestamp,
            title: "Title".try_into().unwrap(),
            description: "Description".try_into().unwrap(),
            creator_hermes_withdrawn: false,
            options,
        };

        pallet::HermesPollData::<Runtime>::insert(poll_id, hermes_poll_info);

        assert_err!(
            HermesGovernancePlatform::vote(RuntimeOrigin::signed(BOB), poll_id, voting_option),
            Error::<Runtime>::NotEnoughHermesForVoting
        );
    });
}

#[test]
fn vote_already_voted() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let poll_end_timestamp = pallet_timestamp::Pallet::<Runtime>::get() + 604800000;
        let user = ALICE;
        let hermes_locked = pallet::MinimumHermesAmountForCreatingPoll::<Runtime>::get();
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(user);
        let encoded: [u8; 32] = (&user, nonce).using_encoded(blake2_256);
        let poll_id = H256::from(encoded);
        let voting_option = "Option 1";
        let mut options = BoundedVec::default();
        options.try_push("Option 1".try_into().unwrap()).unwrap();
        options.try_push("Option 2".try_into().unwrap()).unwrap();
        options.try_push("Option 3".try_into().unwrap()).unwrap();

        let hermes_poll_info = HermesPollInfo {
            creator: user,
            hermes_locked,
            poll_start_timestamp,
            poll_end_timestamp,
            title: "Title".try_into().unwrap(),
            description: "Description".try_into().unwrap(),
            creator_hermes_withdrawn: false,
            options,
        };

        pallet::HermesPollData::<Runtime>::insert(poll_id, hermes_poll_info);

        assert_ok!(HermesGovernancePlatform::vote(
            RuntimeOrigin::signed(ALICE),
            poll_id,
            voting_option.try_into().unwrap()
        ));

        assert_err!(
            HermesGovernancePlatform::vote(
                RuntimeOrigin::signed(ALICE),
                poll_id,
                voting_option.try_into().unwrap()
            ),
            Error::<Runtime>::AlreadyVoted
        );
    });
}

#[test]
fn vote_ok() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let poll_end_timestamp = pallet_timestamp::Pallet::<Runtime>::get() + 604800000;
        let user = ALICE;
        let number_of_hermes = pallet::MinimumHermesVotingAmount::<Runtime>::get();
        let hermes_locked = pallet::MinimumHermesAmountForCreatingPoll::<Runtime>::get();
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(user);
        let encoded: [u8; 32] = (&user, nonce).using_encoded(blake2_256);
        let poll_id = H256::from(encoded);
        let voting_option = "Option 1";
        let mut options = BoundedVec::default();
        options.try_push("Option 1".try_into().unwrap()).unwrap();
        options.try_push("Option 2".try_into().unwrap()).unwrap();
        options.try_push("Option 3".try_into().unwrap()).unwrap();
        options.try_push("Option 4".try_into().unwrap()).unwrap();

        let hermes_poll_info = HermesPollInfo {
            creator: user,
            hermes_locked,
            poll_start_timestamp,
            poll_end_timestamp,
            title: "Title".try_into().unwrap(),
            description: "Description".try_into().unwrap(),
            creator_hermes_withdrawn: false,
            options,
        };

        pallet::HermesPollData::<Runtime>::insert(poll_id, hermes_poll_info);

        assert_ok!(HermesGovernancePlatform::vote(
            RuntimeOrigin::signed(ALICE),
            poll_id,
            voting_option.try_into().unwrap(),
        ));

        let hermes_voting_info = pallet::HermesVotings::<Runtime>::get(poll_id, ALICE).unwrap();

        assert_eq!(
            hermes_voting_info.voting_option,
            voting_option.try_into().unwrap()
        );
        assert!(!hermes_voting_info.hermes_withdrawn);
        assert_eq!(hermes_voting_info.number_of_hermes, number_of_hermes);

        // Check ALICE's balances
        assert_eq!(
            Assets::free_balance(&HERMES_ASSET_ID, &ALICE).expect("Failed to query free balance."),
            balance!(299000)
        );

        // Check pallet's balances
        let hermes_governance = PalletId(*b"hermsgov").into_account_truncating();
        assert_eq!(
            Assets::free_balance(&HERMES_ASSET_ID, &hermes_governance)
                .expect("Failed to query free balance."),
            number_of_hermes
        );
    });
}

#[test]
fn withdraw_funds_voter_poll_does_not_exist() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let user = ALICE;
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(user);
        let encoded: [u8; 32] = (&user, nonce).using_encoded(blake2_256);
        let poll_id = H256::from(encoded);

        assert_err!(
            HermesGovernancePlatform::withdraw_funds_voter(RuntimeOrigin::signed(ALICE), poll_id,),
            Error::<Runtime>::PollDoesNotExist
        );
    });
}

#[test]
fn withdraw_funds_voter_poll_is_not_finished() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let poll_end_timestamp = pallet_timestamp::Pallet::<Runtime>::get() + 604800000;
        let user = ALICE;
        let hermes_locked = pallet::MinimumHermesAmountForCreatingPoll::<Runtime>::get();
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(user);
        let encoded: [u8; 32] = (&user, nonce).using_encoded(blake2_256);
        let poll_id = H256::from(encoded);
        let voting_option = "Option 1".try_into().unwrap();
        let mut options = BoundedVec::default();
        options.try_push("Option 1".try_into().unwrap()).unwrap();
        options.try_push("Option 2".try_into().unwrap()).unwrap();
        options.try_push("Option 3".try_into().unwrap()).unwrap();

        let hermes_poll_info = HermesPollInfo {
            creator: user,
            hermes_locked,
            poll_start_timestamp,
            poll_end_timestamp,
            title: "Title".try_into().unwrap(),
            description: "Description".try_into().unwrap(),
            creator_hermes_withdrawn: false,
            options,
        };

        pallet::HermesPollData::<Runtime>::insert(poll_id, hermes_poll_info);

        assert_ok!(HermesGovernancePlatform::vote(
            RuntimeOrigin::signed(ALICE),
            poll_id,
            voting_option,
        ));

        assert_err!(
            HermesGovernancePlatform::withdraw_funds_voter(RuntimeOrigin::signed(ALICE), poll_id,),
            Error::<Runtime>::PollIsNotFinished
        );
    });
}

#[test]
fn withdraw_funds_voter_not_voted() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let poll_end_timestamp = pallet_timestamp::Pallet::<Runtime>::get() + 604800000;
        let user = ALICE;
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let hermes_locked = pallet::MinimumHermesAmountForCreatingPoll::<Runtime>::get();
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(user);
        let encoded: [u8; 32] = (&user, nonce).using_encoded(blake2_256);
        let poll_id = H256::from(encoded);
        let mut options = BoundedVec::default();
        options.try_push("Option 1".try_into().unwrap()).unwrap();
        options.try_push("Option 2".try_into().unwrap()).unwrap();
        options.try_push("Option 3".try_into().unwrap()).unwrap();

        let hermes_poll_info = HermesPollInfo {
            creator: user,
            hermes_locked,
            poll_start_timestamp,
            poll_end_timestamp,
            title: "Title".try_into().unwrap(),
            description: "Description".try_into().unwrap(),
            creator_hermes_withdrawn: false,
            options,
        };

        pallet::HermesPollData::<Runtime>::insert(poll_id, hermes_poll_info);

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 604900000);

        assert_err!(
            HermesGovernancePlatform::withdraw_funds_voter(RuntimeOrigin::signed(ALICE), poll_id,),
            Error::<Runtime>::NotVoted
        );
    });
}

#[test]
fn withdraw_funds_voter_funds_already_withdrawn() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let poll_end_timestamp = pallet_timestamp::Pallet::<Runtime>::get() + 604800000;
        let user = ALICE;
        let hermes_locked = pallet::MinimumHermesAmountForCreatingPoll::<Runtime>::get();
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(user);
        let encoded: [u8; 32] = (&user, nonce).using_encoded(blake2_256);
        let poll_id = H256::from(encoded);
        let voting_option = "Option 1".try_into().unwrap();
        let mut options = BoundedVec::default();
        options.try_push("Option 1".try_into().unwrap()).unwrap();
        options.try_push("Option 2".try_into().unwrap()).unwrap();

        let hermes_poll_info = HermesPollInfo {
            creator: user,
            hermes_locked,
            poll_start_timestamp,
            poll_end_timestamp,
            title: "Title".try_into().unwrap(),
            description: "Description".try_into().unwrap(),
            creator_hermes_withdrawn: false,
            options,
        };

        pallet::HermesPollData::<Runtime>::insert(poll_id, hermes_poll_info);

        assert_ok!(HermesGovernancePlatform::vote(
            RuntimeOrigin::signed(ALICE),
            poll_id,
            voting_option,
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 604900000);

        assert_ok!(HermesGovernancePlatform::withdraw_funds_voter(
            RuntimeOrigin::signed(ALICE),
            poll_id
        ));

        assert_err!(
            HermesGovernancePlatform::withdraw_funds_voter(RuntimeOrigin::signed(ALICE), poll_id,),
            Error::<Runtime>::FundsAlreadyWithdrawn
        );
    });
}

#[test]
fn withdraw_funds_voter_ok() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let poll_end_timestamp = pallet_timestamp::Pallet::<Runtime>::get() + 604800000;
        let user = ALICE;
        let hermes_locked = pallet::MinimumHermesAmountForCreatingPoll::<Runtime>::get();
        let number_of_hermes = pallet::MinimumHermesVotingAmount::<Runtime>::get();
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(user);
        let encoded: [u8; 32] = (&user, nonce).using_encoded(blake2_256);
        let poll_id = H256::from(encoded);
        let voting_option = "Option 1";
        let mut options = BoundedVec::default();
        options.try_push("Option 1".try_into().unwrap()).unwrap();
        options.try_push("Option 2".try_into().unwrap()).unwrap();
        options.try_push("Option 3".try_into().unwrap()).unwrap();

        let hermes_poll_info = HermesPollInfo {
            creator: user,
            hermes_locked,
            poll_start_timestamp,
            poll_end_timestamp,
            title: "Title".try_into().unwrap(),
            description: "Description".try_into().unwrap(),
            creator_hermes_withdrawn: false,
            options,
        };

        pallet::HermesPollData::<Runtime>::insert(poll_id, hermes_poll_info);

        assert_ok!(HermesGovernancePlatform::vote(
            RuntimeOrigin::signed(ALICE),
            poll_id,
            voting_option.try_into().unwrap()
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 604900000);

        assert_ok!(HermesGovernancePlatform::withdraw_funds_voter(
            RuntimeOrigin::signed(ALICE),
            poll_id
        ));

        let hermes_voting_info = pallet::HermesVotings::<Runtime>::get(poll_id, ALICE).unwrap();

        assert_eq!(
            hermes_voting_info.voting_option,
            voting_option.try_into().unwrap()
        );
        assert_eq!(hermes_voting_info.number_of_hermes, number_of_hermes);
        assert!(hermes_voting_info.hermes_withdrawn);

        // Check ALICE's balances
        assert_eq!(
            Assets::free_balance(&HERMES_ASSET_ID, &ALICE).expect("Failed to query free balance."),
            balance!(300000)
        );

        // Check pallet's balances
        let hermes_governance = PalletId(*b"hermsgov").into_account_truncating();
        assert_eq!(
            Assets::free_balance(&HERMES_ASSET_ID, &hermes_governance)
                .expect("Failed to query free balance."),
            balance!(0)
        );
    });
}

#[test]
fn withdraw_funds_creator_poll_does_not_exist() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let user = ALICE;
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(user);
        let encoded: [u8; 32] = (&user, nonce).using_encoded(blake2_256);
        let poll_id = H256::from(encoded);

        assert_err!(
            HermesGovernancePlatform::withdraw_funds_creator(RuntimeOrigin::signed(ALICE), poll_id,),
            Error::<Runtime>::PollDoesNotExist
        );
    });
}

#[test]
fn withdraw_funds_creator_you_are_not_creator() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let poll_end_timestamp = pallet_timestamp::Pallet::<Runtime>::get() + 604800000;
        let user = ALICE;
        let hermes_locked = pallet::MinimumHermesAmountForCreatingPoll::<Runtime>::get();
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(user);
        let encoded: [u8; 32] = (&user, nonce).using_encoded(blake2_256);
        let poll_id = H256::from(encoded);
        let mut options = BoundedVec::default();
        options.try_push("Option 1".try_into().unwrap()).unwrap();
        options.try_push("Option 2".try_into().unwrap()).unwrap();

        let hermes_poll_info = HermesPollInfo {
            creator: user,
            hermes_locked,
            poll_start_timestamp,
            poll_end_timestamp,
            title: "Title".try_into().unwrap(),
            description: "Description".try_into().unwrap(),
            creator_hermes_withdrawn: false,
            options,
        };

        pallet::HermesPollData::<Runtime>::insert(poll_id, hermes_poll_info);

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 604900000);

        assert_err!(
            HermesGovernancePlatform::withdraw_funds_creator(RuntimeOrigin::signed(BOB), poll_id,),
            Error::<Runtime>::YouAreNotCreator
        );
    });
}

#[test]
fn withdraw_funds_creator_poll_is_not_finished() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let poll_end_timestamp = pallet_timestamp::Pallet::<Runtime>::get() + 604800000;
        let user = ALICE;
        let hermes_locked = pallet::MinimumHermesAmountForCreatingPoll::<Runtime>::get();
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(user);
        let encoded: [u8; 32] = (&user, nonce).using_encoded(blake2_256);
        let poll_id = H256::from(encoded);
        let mut options = BoundedVec::default();
        options.try_push("Option 1".try_into().unwrap()).unwrap();
        options.try_push("Option 2".try_into().unwrap()).unwrap();
        options.try_push("Option 3".try_into().unwrap()).unwrap();

        let hermes_poll_info = HermesPollInfo {
            creator: user,
            hermes_locked,
            poll_start_timestamp,
            poll_end_timestamp,
            title: "Title".try_into().unwrap(),
            description: "Description".try_into().unwrap(),
            creator_hermes_withdrawn: false,
            options,
        };

        pallet::HermesPollData::<Runtime>::insert(poll_id, hermes_poll_info);

        assert_err!(
            HermesGovernancePlatform::withdraw_funds_creator(
                RuntimeOrigin::signed(ALICE),
                poll_id,
            ),
            Error::<Runtime>::PollIsNotFinished
        );
    });
}

#[test]
fn withdraw_funds_creator_funds_already_withdrawn() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let poll_end_timestamp = pallet_timestamp::Pallet::<Runtime>::get() + 604800000;
        let user = ALICE;
        let hermes_locked = pallet::MinimumHermesAmountForCreatingPoll::<Runtime>::get();
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(user);
        let encoded: [u8; 32] = (&user, nonce).using_encoded(blake2_256);
        let poll_id = H256::from(encoded);
        let mut options = BoundedVec::default();
        options.try_push("Option 1".try_into().unwrap()).unwrap();
        options.try_push("Option 2".try_into().unwrap()).unwrap();
        options.try_push("Option 3".try_into().unwrap()).unwrap();
        options.try_push("Option 4".try_into().unwrap()).unwrap();

        let hermes_poll_info = HermesPollInfo {
            creator: user,
            hermes_locked,
            poll_start_timestamp,
            poll_end_timestamp,
            title: "Title".try_into().unwrap(),
            description: "Description".try_into().unwrap(),
            creator_hermes_withdrawn: false,
            options,
        };

        pallet::HermesPollData::<Runtime>::insert(poll_id, &hermes_poll_info);

        let pallet_account = PalletId(*b"hermsgov").into_account_truncating();
        assert_ok!(Assets::transfer_from(
            &HERMES_ASSET_ID,
            &user,
            &pallet_account,
            hermes_poll_info.hermes_locked
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 604900000);

        assert_ok!(HermesGovernancePlatform::withdraw_funds_creator(
            RuntimeOrigin::signed(ALICE),
            poll_id,
        ));

        assert_err!(
            HermesGovernancePlatform::withdraw_funds_creator(
                RuntimeOrigin::signed(ALICE),
                poll_id,
            ),
            Error::<Runtime>::FundsAlreadyWithdrawn
        );
    });
}

#[test]
fn withdraw_funds_creator_ok() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let poll_end_timestamp = pallet_timestamp::Pallet::<Runtime>::get() + 604800000;
        let user = ALICE;
        let hermes_locked = pallet::MinimumHermesAmountForCreatingPoll::<Runtime>::get();
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(user);
        let encoded: [u8; 32] = (&user, nonce).using_encoded(blake2_256);
        let poll_id = H256::from(encoded);
        let mut options = BoundedVec::default();
        options.try_push("Option 1".try_into().unwrap()).unwrap();
        options.try_push("Option 2".try_into().unwrap()).unwrap();
        options.try_push("Option 3".try_into().unwrap()).unwrap();

        let hermes_poll_info = HermesPollInfo {
            creator: user,
            hermes_locked,
            poll_start_timestamp,
            poll_end_timestamp,
            title: "Title".try_into().unwrap(),
            description: "Description".try_into().unwrap(),
            creator_hermes_withdrawn: false,
            options,
        };

        pallet::HermesPollData::<Runtime>::insert(poll_id, &hermes_poll_info);

        let pallet_account = PalletId(*b"hermsgov").into_account_truncating();
        assert_ok!(Assets::transfer_from(
            &HERMES_ASSET_ID,
            &user,
            &pallet_account,
            hermes_poll_info.hermes_locked
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 604900000);

        assert_ok!(HermesGovernancePlatform::withdraw_funds_creator(
            RuntimeOrigin::signed(ALICE),
            poll_id
        ));

        let hermes_info = pallet::HermesPollData::<Runtime>::get(poll_id).unwrap();
        assert!(hermes_info.creator_hermes_withdrawn);

        // Check ALICE's balances
        assert_eq!(
            Assets::free_balance(&HERMES_ASSET_ID, &ALICE).expect("Failed to query free balance."),
            balance!(300000)
        );

        // Check pallet's balances
        assert_eq!(
            Assets::free_balance(&HERMES_ASSET_ID, &pallet_account)
                .expect("Failed to query free balance."),
            balance!(0)
        );
    });
}

#[test]
fn change_min_hermes_for_voting_unauthorized() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_err!(
            HermesGovernancePlatform::change_min_hermes_for_voting(
                RuntimeOrigin::signed(ALICE),
                balance!(100)
            ),
            Error::<Runtime>::Unauthorized
        );
    });
}

#[test]
fn change_min_hermes_for_voting_ok() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_ok!(HermesGovernancePlatform::change_min_hermes_for_voting(
            RuntimeOrigin::signed(pallet::AuthorityAccount::<Runtime>::get()),
            balance!(300)
        ));

        assert_eq!(
            pallet::MinimumHermesVotingAmount::<Runtime>::get(),
            balance!(300),
        );
    });
}

#[test]
fn change_min_hermes_for_creating_poll_unauthorized() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_err!(
            HermesGovernancePlatform::change_min_hermes_for_creating_poll(
                RuntimeOrigin::signed(ALICE),
                balance!(100)
            ),
            Error::<Runtime>::Unauthorized
        );
    });
}

#[test]
fn change_min_hermes_for_creating_poll_ok() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_ok!(
            HermesGovernancePlatform::change_min_hermes_for_creating_poll(
                RuntimeOrigin::signed(pallet::AuthorityAccount::<Runtime>::get()),
                balance!(100)
            )
        );

        assert_eq!(
            pallet::MinimumHermesAmountForCreatingPoll::<Runtime>::get(),
            balance!(100),
        );
    });
}

#[test]
fn hermes_governance_storage_migration_works() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        generate_storage_instance!(HermesGovernancePlatform, HermesVotings);
        generate_storage_instance!(HermesGovernancePlatform, HermesPollData);

        let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let poll_end_timestamp = pallet_timestamp::Pallet::<Runtime>::get() + 604800000;
        let user = ALICE;
        let user1 = CHARLES;
        let number_of_hermes = pallet::MinimumHermesVotingAmount::<Runtime>::get();
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(user);
        let encoded: [u8; 32] = (&user, nonce).using_encoded(blake2_256);
        let poll_id_a = H256::from(encoded);
        let poll_id_b = H256::from(encoded);
        let options = vec!["Yes".try_into().unwrap(), "No".try_into().unwrap()];

        type OldHermesVotings = StorageDoubleMap<
            HermesVotingsOldInstance,
            Identity,
            H256,
            Identity,
            AccountIdOf<Runtime>,
            OldHermesVotingInfo,
            OptionQuery,
        >;

        type OldHermesPollData<Moment> = StorageMap<
            HermesPollDataOldInstance,
            Identity,
            H256,
            OldHermesPollInfo<AccountIdOf<Runtime>, Moment>,
            OptionQuery,
        >;

        let old_voting_info_a = OldHermesVotingInfo {
            voting_option: VotingOption::Yes,
            number_of_hermes,
            hermes_withdrawn: false,
        };

        let old_voting_info_b = OldHermesVotingInfo {
            voting_option: VotingOption::No,
            number_of_hermes,
            hermes_withdrawn: false,
        };

        let old_poll_data = OldHermesPollInfo {
            creator: user,
            hermes_locked: number_of_hermes,
            poll_start_timestamp,
            poll_end_timestamp,
            title: "Titile".try_into().unwrap(),
            description: "Description".try_into().unwrap(),
            creator_hermes_withdrawn: false,
        };

        OldHermesVotings::insert(poll_id_a, user, old_voting_info_a);
        OldHermesVotings::insert(poll_id_b, user1, old_voting_info_b);

        OldHermesPollData::insert(poll_id_a, &old_poll_data);
        OldHermesPollData::insert(poll_id_b, &old_poll_data);

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(10000000);
        run_to_block(5);

        //Storage migration
        pallet::Pallet::<Runtime>::on_runtime_upgrade();

        let poll_a = pallet::HermesPollData::<Runtime>::get(poll_id_a).unwrap();
        let voting_a = pallet::HermesVotings::<Runtime>::get(poll_id_a, user).unwrap();
        assert_eq!(poll_a.options, options);
        assert_eq!(voting_a.voting_option, "Yes".try_into().unwrap());

        let poll_b = pallet::HermesPollData::<Runtime>::get(poll_id_b).unwrap();
        let voting_b = pallet::HermesVotings::<Runtime>::get(poll_id_b, user1).unwrap();
        assert_eq!(poll_b.options, options);
        assert_eq!(voting_b.voting_option, "No".try_into().unwrap());

        // Storage version should be V2 so no changes made
        pallet_timestamp::Pallet::<Runtime>::set_timestamp(11000000);
        run_to_block(10);

        // Storage migration
        pallet::Pallet::<Runtime>::on_runtime_upgrade();

        let poll_a = pallet::HermesPollData::<Runtime>::get(poll_id_a).unwrap();
        let voting_a = pallet::HermesVotings::<Runtime>::get(poll_id_a, user).unwrap();
        assert_eq!(poll_a.options, options);
        assert_eq!(voting_a.voting_option, "Yes".try_into().unwrap());

        let poll_b = pallet::HermesPollData::<Runtime>::get(poll_id_b).unwrap();
        let voting_b = pallet::HermesVotings::<Runtime>::get(poll_id_b, user1).unwrap();
        assert_eq!(poll_b.options, options);
        assert_eq!(voting_b.voting_option, "No".try_into().unwrap());
    });
}
