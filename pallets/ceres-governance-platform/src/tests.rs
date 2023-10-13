use crate::mock::*;
use crate::{pallet, Error};
use common::{balance, generate_storage_instance, AssetInfoProvider, CERES_ASSET_ID};
use frame_support::pallet_prelude::StorageMap;
use frame_support::storage::types::ValueQuery;
use frame_support::traits::Hooks;
use frame_support::PalletId;
use frame_support::{assert_err, assert_ok, Identity};
use sp_runtime::traits::AccountIdConversion;

#[test]
fn create_poll_invalid_number_of_option() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_id = Vec::from([1, 2, 3, 4]);
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        assert_err!(
            CeresGovernancePlatform::create_poll(
                RuntimeOrigin::signed(ALICE),
                poll_id,
                1,
                current_timestamp,
                current_timestamp + 1
            ),
            Error::<Runtime>::InvalidNumberOfOption
        );
    });
}

#[test]
fn create_poll_invalid_start_timestamp() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_id = Vec::from([1, 2, 3, 4]);
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 1);

        assert_err!(
            CeresGovernancePlatform::create_poll(
                RuntimeOrigin::signed(ALICE),
                poll_id,
                3,
                current_timestamp,
                current_timestamp + 10
            ),
            Error::<Runtime>::InvalidStartTimestamp
        );
    });
}

#[test]
fn create_poll_invalid_end_timestamp() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_id = Vec::from([1, 2, 3, 4]);
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        assert_err!(
            CeresGovernancePlatform::create_poll(
                RuntimeOrigin::signed(ALICE),
                poll_id,
                2,
                current_timestamp + 1,
                current_timestamp
            ),
            Error::<Runtime>::InvalidEndTimestamp
        );
    });
}

#[test]
fn create_poll_ok() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_id = Vec::from([1, 2, 3, 4]);
        let number_of_option = 2;
        let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let poll_end_timestamp = pallet_timestamp::Pallet::<Runtime>::get() + 1;
        assert_ok!(CeresGovernancePlatform::create_poll(
            RuntimeOrigin::signed(ALICE),
            poll_id.clone(),
            number_of_option,
            poll_start_timestamp,
            poll_end_timestamp
        ));

        // Check PollData map
        let poll_info = pallet::PollData::<Runtime>::get(&poll_id);
        assert_eq!(poll_info.number_of_options, number_of_option);
        assert_eq!(poll_info.poll_start_timestamp, poll_start_timestamp);
        assert_eq!(poll_info.poll_end_timestamp, poll_end_timestamp);
    })
}

#[test]
fn create_poll_poll_id_already_exists() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_id = Vec::from([1, 2, 3, 4]);
        let number_of_option = 2;
        let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let poll_end_timestamp = pallet_timestamp::Pallet::<Runtime>::get() + 1;
        assert_ok!(CeresGovernancePlatform::create_poll(
            RuntimeOrigin::signed(ALICE),
            poll_id.clone(),
            number_of_option,
            poll_start_timestamp,
            poll_end_timestamp
        ));

        assert_err!(
            CeresGovernancePlatform::create_poll(
                RuntimeOrigin::signed(ALICE),
                poll_id,
                number_of_option,
                poll_start_timestamp,
                poll_end_timestamp
            ),
            Error::<Runtime>::PollIdAlreadyExists
        );
    })
}

#[test]
fn vote_invalid_number_of_votes() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_id = Vec::from([1, 2, 3, 4]);
        assert_err!(
            CeresGovernancePlatform::vote(RuntimeOrigin::signed(ALICE), poll_id, 2, balance!(0)),
            Error::<Runtime>::InvalidNumberOfVotes
        );
    });
}

#[test]
fn vote_poll_is_not_started() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_id = Vec::from([1, 2, 3, 4]);
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        assert_ok!(CeresGovernancePlatform::create_poll(
            RuntimeOrigin::signed(ALICE),
            poll_id.clone(),
            2,
            current_timestamp + 2,
            current_timestamp + 10
        ));
        assert_err!(
            CeresGovernancePlatform::vote(RuntimeOrigin::signed(BOB), poll_id, 2, balance!(10)),
            Error::<Runtime>::PollIsNotStarted
        );
    });
}

#[test]
fn vote_poll_is_finished() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_id = Vec::from([1, 2, 3, 4]);
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        assert_ok!(CeresGovernancePlatform::create_poll(
            RuntimeOrigin::signed(ALICE),
            poll_id.clone(),
            2,
            current_timestamp + 2,
            current_timestamp + 5
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 11);

        assert_err!(
            CeresGovernancePlatform::vote(RuntimeOrigin::signed(BOB), poll_id, 2, balance!(10)),
            Error::<Runtime>::PollIsFinished
        );
    });
}

#[test]
fn vote_invalid_number_of_option() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_id = Vec::from([1, 2, 3, 4]);
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        assert_ok!(CeresGovernancePlatform::create_poll(
            RuntimeOrigin::signed(ALICE),
            poll_id.clone(),
            3,
            current_timestamp,
            current_timestamp + 10
        ));
        assert_err!(
            CeresGovernancePlatform::vote(RuntimeOrigin::signed(ALICE), poll_id, 4, balance!(50)),
            Error::<Runtime>::InvalidNumberOfOption
        );
    });
}

#[test]
fn vote_vote_denied() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_id = Vec::from([1, 2, 3, 4]);
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        assert_ok!(CeresGovernancePlatform::create_poll(
            RuntimeOrigin::signed(ALICE),
            poll_id.clone(),
            3,
            current_timestamp,
            current_timestamp + 10
        ));
        assert_ok!(CeresGovernancePlatform::vote(
            RuntimeOrigin::signed(ALICE),
            poll_id.clone(),
            3,
            balance!(50)
        ));
        assert_err!(
            CeresGovernancePlatform::vote(RuntimeOrigin::signed(ALICE), poll_id, 2, balance!(50)),
            Error::<Runtime>::VoteDenied
        );
    });
}

#[test]
fn vote_not_enough_funds() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_id = Vec::from([1, 2, 3, 4]);
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        assert_ok!(CeresGovernancePlatform::create_poll(
            RuntimeOrigin::signed(ALICE),
            poll_id.clone(),
            3,
            current_timestamp,
            current_timestamp + 10
        ));
        assert_err!(
            CeresGovernancePlatform::vote(RuntimeOrigin::signed(ALICE), poll_id, 3, balance!(3100)),
            Error::<Runtime>::NotEnoughFunds
        );
    });
}

#[test]
fn vote_ok() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_id = Vec::from([1, 2, 3, 4]);
        let voting_option = 3;
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let number_of_votes = balance!(300);
        assert_ok!(CeresGovernancePlatform::create_poll(
            RuntimeOrigin::signed(ALICE),
            poll_id.clone(),
            voting_option,
            current_timestamp,
            current_timestamp + 10
        ));
        assert_ok!(CeresGovernancePlatform::vote(
            RuntimeOrigin::signed(ALICE),
            poll_id.clone(),
            voting_option,
            number_of_votes
        ));

        // Check Voting map
        let voting_info = pallet::Voting::<Runtime>::get(&poll_id, ALICE);
        assert_eq!(voting_info.voting_option, voting_option);
        assert_eq!(voting_info.number_of_votes, number_of_votes);

        // Check ALICE's balances
        assert_eq!(
            Assets::free_balance(&CERES_ASSET_ID, &ALICE).expect("Failed to query free balance."),
            balance!(2700)
        );

        // Check pallet's balances
        let governance = PalletId(*b"ceresgov").into_account_truncating();
        assert_eq!(
            Assets::free_balance(&CERES_ASSET_ID, &governance)
                .expect("Failed to query free balance."),
            number_of_votes
        );
    });
}

#[test]
fn withdraw_poll_is_not_finished() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_id = Vec::from([1, 2, 3, 4]);
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        assert_ok!(CeresGovernancePlatform::create_poll(
            RuntimeOrigin::signed(ALICE),
            poll_id.clone(),
            2,
            current_timestamp,
            current_timestamp + 10
        ));
        assert_err!(
            CeresGovernancePlatform::withdraw(RuntimeOrigin::signed(BOB), poll_id),
            Error::<Runtime>::PollIsNotFinished
        );
    });
}

#[test]
fn withdraw_funds_already_withdrawn() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let poll_id = Vec::from([1, 2, 3, 4]);
        let voting_option = 2;
        let number_of_votes = balance!(300);
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        assert_ok!(CeresGovernancePlatform::create_poll(
            RuntimeOrigin::signed(ALICE),
            poll_id.clone(),
            voting_option,
            current_timestamp,
            current_timestamp + 10
        ));
        assert_ok!(CeresGovernancePlatform::vote(
            RuntimeOrigin::signed(ALICE),
            poll_id.clone(),
            voting_option,
            number_of_votes
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 11);

        assert_ok!(CeresGovernancePlatform::withdraw(
            RuntimeOrigin::signed(ALICE),
            poll_id.clone()
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
        let poll_id = Vec::from([1, 2, 3, 4]);
        let voting_option = 2;
        let number_of_votes = balance!(300);
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        assert_ok!(CeresGovernancePlatform::create_poll(
            RuntimeOrigin::signed(ALICE),
            poll_id.clone(),
            voting_option,
            current_timestamp,
            current_timestamp + 10
        ));
        assert_ok!(CeresGovernancePlatform::vote(
            RuntimeOrigin::signed(ALICE),
            poll_id.clone(),
            voting_option,
            number_of_votes
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 11);

        assert_ok!(CeresGovernancePlatform::withdraw(
            RuntimeOrigin::signed(ALICE),
            poll_id.clone()
        ));

        // Check Voting map
        let voting_info = pallet::Voting::<Runtime>::get(&poll_id, ALICE);
        assert_eq!(voting_info.voting_option, voting_option);
        assert_eq!(voting_info.number_of_votes, number_of_votes);
        assert!(voting_info.ceres_withdrawn);

        // Check ALICE's balances
        assert_eq!(
            Assets::free_balance(&CERES_ASSET_ID, &ALICE).expect("Failed to query free balance."),
            balance!(3000)
        );

        // Check pallet's balances
        let governance = PalletId(*b"ceresgov").into_account_truncating();
        assert_eq!(
            Assets::free_balance(&CERES_ASSET_ID, &governance)
                .expect("Failed to query free balance."),
            balance!(0)
        );
    })
}

#[test]
fn governance_storage_migration_works() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        generate_storage_instance!(CeresGovernancePlatform, PollData);
        type OldPollData = StorageMap<
            PollDataOldInstance,
            Identity,
            Vec<u8>,
            (u32, BlockNumber, BlockNumber),
            ValueQuery,
        >;

        let poll_id_a = Vec::from([1, 2, 3, 4]);
        let poll_id_b = Vec::from([1, 2, 3, 5]);

        OldPollData::insert(&poll_id_a, (2, 4482112u64, 4496512u64));

        OldPollData::insert(&poll_id_b, (3, 529942780u64, 529942790u64));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(10000000);
        run_to_block(5);

        // Storage migration
        CeresGovernancePlatform::on_runtime_upgrade();

        let poll_a = pallet::PollData::<Runtime>::get(&poll_id_a);
        assert_eq!(poll_a.poll_start_timestamp, 26902642000);
        assert_eq!(poll_a.poll_end_timestamp, 26989042000);

        let poll_b = pallet::PollData::<Runtime>::get(&poll_id_b);
        assert_eq!(poll_b.poll_start_timestamp, 3179666650000);
        assert_eq!(poll_b.poll_end_timestamp, 3179666710000);

        // Storage version should be V2 so no changes made
        pallet_timestamp::Pallet::<Runtime>::set_timestamp(11000000);
        run_to_block(10);

        // Storage migration
        CeresGovernancePlatform::on_runtime_upgrade();

        let poll_a = pallet::PollData::<Runtime>::get(&poll_id_a);
        assert_eq!(poll_a.poll_start_timestamp, 26902642000);
        assert_eq!(poll_a.poll_end_timestamp, 26989042000);

        let poll_b = pallet::PollData::<Runtime>::get(&poll_id_b);
        assert_eq!(poll_b.poll_start_timestamp, 3179666650000);
        assert_eq!(poll_b.poll_end_timestamp, 3179666710000);
    });
}
