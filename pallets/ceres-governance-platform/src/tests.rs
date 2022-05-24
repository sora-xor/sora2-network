mod tests {
    use crate::mock::*;
    use crate::{pallet, Error};
    use common::{balance, CERES_ASSET_ID};
    use frame_support::{assert_err, assert_ok, PalletId};
    use sp_runtime::traits::AccountIdConversion;

    #[test]
    fn create_poll_invalid_number_of_option() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let poll_id = Vec::from([1, 2, 3, 4]);
            let current_block = frame_system::Pallet::<Runtime>::block_number();
            assert_err!(
                CeresGovernancePlatform::create_poll(
                    Origin::signed(ALICE),
                    poll_id,
                    1,
                    current_block,
                    current_block + 1
                ),
                Error::<Runtime>::InvalidNumberOfOption
            );
        });
    }

    #[test]
    fn create_poll_invalid_start_block() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let poll_id = Vec::from([1, 2, 3, 4]);
            let current_block = frame_system::Pallet::<Runtime>::block_number();

            run_to_block(5);

            assert_err!(
                CeresGovernancePlatform::create_poll(
                    Origin::signed(ALICE),
                    poll_id.clone(),
                    3,
                    current_block,
                    current_block + 10
                ),
                Error::<Runtime>::InvalidStartBlock
            );
        });
    }

    #[test]
    fn create_poll_invalid_end_block() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let poll_id = Vec::from([1, 2, 3, 4]);
            let current_block = frame_system::Pallet::<Runtime>::block_number();
            assert_err!(
                CeresGovernancePlatform::create_poll(
                    Origin::signed(ALICE),
                    poll_id,
                    2,
                    current_block + 1,
                    current_block
                ),
                Error::<Runtime>::InvalidEndBlock
            );
        });
    }

    #[test]
    fn create_poll_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let poll_id = Vec::from([1, 2, 3, 4]);
            let number_of_option = 2;
            let poll_start_block = frame_system::Pallet::<Runtime>::block_number();
            let poll_end_block = frame_system::Pallet::<Runtime>::block_number() + 1;
            assert_ok!(CeresGovernancePlatform::create_poll(
                Origin::signed(ALICE),
                poll_id.clone(),
                number_of_option,
                poll_start_block,
                poll_end_block
            ));

            // Check PollData map
            let poll_info = pallet::PollData::<Runtime>::get(&poll_id);
            assert_eq!(poll_info.number_of_options, number_of_option);
            assert_eq!(poll_info.poll_start_block, poll_start_block);
            assert_eq!(poll_info.poll_end_block, poll_end_block);
        })
    }

    #[test]
    fn create_poll_poll_id_already_exists() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let poll_id = Vec::from([1, 2, 3, 4]);
            let number_of_option = 2;
            let poll_start_block = frame_system::Pallet::<Runtime>::block_number();
            let poll_end_block = frame_system::Pallet::<Runtime>::block_number() + 1;
            assert_ok!(CeresGovernancePlatform::create_poll(
                Origin::signed(ALICE),
                poll_id.clone(),
                number_of_option,
                poll_start_block,
                poll_end_block
            ));

            assert_err!(
                CeresGovernancePlatform::create_poll(
                    Origin::signed(ALICE),
                    poll_id.clone(),
                    number_of_option,
                    poll_start_block,
                    poll_end_block
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
                CeresGovernancePlatform::vote(Origin::signed(ALICE), poll_id, 2, balance!(0)),
                Error::<Runtime>::InvalidNumberOfVotes
            );
        });
    }

    #[test]
    fn vote_poll_is_not_started() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let poll_id = Vec::from([1, 2, 3, 4]);
            let current_block = frame_system::Pallet::<Runtime>::block_number();
            assert_ok!(CeresGovernancePlatform::create_poll(
                Origin::signed(ALICE),
                poll_id.clone(),
                2,
                current_block + 2,
                current_block + 10
            ));
            assert_err!(
                CeresGovernancePlatform::vote(
                    Origin::signed(BOB),
                    poll_id.clone(),
                    2,
                    balance!(10)
                ),
                Error::<Runtime>::PollIsNotStarted
            );
        });
    }

    #[test]
    fn vote_poll_is_finished() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let poll_id = Vec::from([1, 2, 3, 4]);
            let current_block = frame_system::Pallet::<Runtime>::block_number();
            assert_ok!(CeresGovernancePlatform::create_poll(
                Origin::signed(ALICE),
                poll_id.clone(),
                2,
                current_block + 2,
                current_block + 5
            ));

            run_to_block(11);

            assert_err!(
                CeresGovernancePlatform::vote(
                    Origin::signed(BOB),
                    poll_id.clone(),
                    2,
                    balance!(10)
                ),
                Error::<Runtime>::PollIsFinished
            );
        });
    }

    #[test]
    fn vote_invalid_number_of_option() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let poll_id = Vec::from([1, 2, 3, 4]);
            let current_block = frame_system::Pallet::<Runtime>::block_number();
            assert_ok!(CeresGovernancePlatform::create_poll(
                Origin::signed(ALICE),
                poll_id.clone(),
                3,
                current_block,
                current_block + 10
            ));
            assert_err!(
                CeresGovernancePlatform::vote(
                    Origin::signed(ALICE),
                    poll_id.clone(),
                    4,
                    balance!(50)
                ),
                Error::<Runtime>::InvalidNumberOfOption
            );
        });
    }

    #[test]
    fn vote_vote_denied() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let poll_id = Vec::from([1, 2, 3, 4]);
            let current_block = frame_system::Pallet::<Runtime>::block_number();
            assert_ok!(CeresGovernancePlatform::create_poll(
                Origin::signed(ALICE),
                poll_id.clone(),
                3,
                current_block,
                current_block + 10
            ));
            assert_ok!(CeresGovernancePlatform::vote(
                Origin::signed(ALICE),
                poll_id.clone(),
                3,
                balance!(50)
            ));
            assert_err!(
                CeresGovernancePlatform::vote(
                    Origin::signed(ALICE),
                    poll_id.clone(),
                    2,
                    balance!(50)
                ),
                Error::<Runtime>::VoteDenied
            );
        });
    }

    #[test]
    fn vote_not_enough_funds() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let poll_id = Vec::from([1, 2, 3, 4]);
            let current_block = frame_system::Pallet::<Runtime>::block_number();
            assert_ok!(CeresGovernancePlatform::create_poll(
                Origin::signed(ALICE),
                poll_id.clone(),
                3,
                current_block,
                current_block + 10
            ));
            assert_err!(
                CeresGovernancePlatform::vote(Origin::signed(ALICE), poll_id, 3, balance!(3100)),
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
            let current_block = frame_system::Pallet::<Runtime>::block_number();
            let number_of_votes = balance!(300);
            assert_ok!(CeresGovernancePlatform::create_poll(
                Origin::signed(ALICE),
                poll_id.clone(),
                voting_option,
                current_block,
                current_block + 10
            ));
            assert_ok!(CeresGovernancePlatform::vote(
                Origin::signed(ALICE),
                poll_id.clone(),
                voting_option,
                number_of_votes
            ));

            // Check Voting map
            let voting_info = pallet::Voting::<Runtime>::get(&poll_id, &ALICE);
            assert_eq!(voting_info.voting_option, voting_option);
            assert_eq!(voting_info.number_of_votes, number_of_votes);

            // Check ALICE's balances
            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &ALICE)
                    .expect("Failed to query free balance."),
                balance!(2700)
            );

            // Check pallet's balances
            let governance = PalletId(*b"ceresgov").into_account();
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
            let current_block = frame_system::Pallet::<Runtime>::block_number();
            assert_ok!(CeresGovernancePlatform::create_poll(
                Origin::signed(ALICE),
                poll_id.clone(),
                2,
                current_block,
                current_block + 10
            ));
            assert_err!(
                CeresGovernancePlatform::withdraw(Origin::signed(BOB), poll_id.clone()),
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
            let current_block = frame_system::Pallet::<Runtime>::block_number();
            assert_ok!(CeresGovernancePlatform::create_poll(
                Origin::signed(ALICE),
                poll_id.clone(),
                voting_option,
                current_block,
                current_block + 10
            ));
            assert_ok!(CeresGovernancePlatform::vote(
                Origin::signed(ALICE),
                poll_id.clone(),
                voting_option,
                number_of_votes
            ));

            run_to_block(11);

            assert_ok!(CeresGovernancePlatform::withdraw(
                Origin::signed(ALICE),
                poll_id.clone()
            ));

            assert_err!(
                CeresGovernancePlatform::withdraw(Origin::signed(ALICE), poll_id.clone()),
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
            let current_block = frame_system::Pallet::<Runtime>::block_number();
            assert_ok!(CeresGovernancePlatform::create_poll(
                Origin::signed(ALICE),
                poll_id.clone(),
                voting_option,
                current_block,
                current_block + 10
            ));
            assert_ok!(CeresGovernancePlatform::vote(
                Origin::signed(ALICE),
                poll_id.clone(),
                voting_option,
                number_of_votes
            ));

            run_to_block(11);

            assert_ok!(CeresGovernancePlatform::withdraw(
                Origin::signed(ALICE),
                poll_id.clone()
            ));

            // Check Voting map
            let voting_info = pallet::Voting::<Runtime>::get(&poll_id, &ALICE);
            assert_eq!(voting_info.voting_option, voting_option);
            assert_eq!(voting_info.number_of_votes, number_of_votes);
            assert_eq!(voting_info.ceres_withdrawn, true);

            // Check ALICE's balances
            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &ALICE)
                    .expect("Failed to query free balance."),
                balance!(3000)
            );

            // Check pallet's balances
            let governance = PalletId(*b"ceresgov").into_account();
            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &governance)
                    .expect("Failed to query free balance."),
                balance!(0)
            );
        })
    }
}
