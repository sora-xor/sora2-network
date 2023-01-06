mod tests {
    use crate::mock::*;
    use crate::{pallet, Error, HermesPollInfo, Pallet as HermesGovernancePlatformPallet};
    use common::{
        balance, AssetId32, AssetName, AssetSymbol, Balance, LiquiditySourceType, PoolXykPallet,
        PredefinedAssetId, ToFeeAccount, CERES_ASSET_ID, DEFAULT_BALANCE_PRECISION,
        DEMETER_ASSET_ID, XOR, XSTUSD,
    };
    use frame_support::pallet_prelude::{StorageDoubleMap, StorageMap};
    use frame_support::storage::types::ValueQuery;
    use frame_support::PalletId;
    use frame_support::{assert_err, assert_ok};
    use sp_runtime::traits::AccountIdConversion;
    use uuid::Uuid;

    #[test]
    fn create_poll_invalid_start_timestamp() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
            let title: String = "Title".to_string();
            let description: String = "Description".to_string();

            pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 1);

            assert_err!(
                HermesGovernancePlatform::create_poll(
                    Origin::signed(ALICE),
                    current_timestamp,
                    current_timestamp + 10,
                    title,
                    description,
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
            let title: String = "Title".to_string();
            let description: String = "Description".to_string();

            assert_err!(
                HermesGovernancePlatform::create_poll(
                    Origin::signed(ALICE),
                    current_timestamp + 1,
                    current_timestamp,
                    title,
                    description,
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
            let title: String = "Title".to_string();
            let description: String = "Description".to_string();

            assert_err!(
                HermesGovernancePlatform::create_poll(
                    Origin::signed(ALICE),
                    current_timestamp,
                    current_timestamp + 15,
                    title,
                    description,
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
            let title: String = "Title".to_string();
            let description: String = "Description".to_string();

            assert_err!(
                HermesGovernancePlatform::create_poll(
                    Origin::signed(ALICE),
                    current_timestamp,
                    current_timestamp + 604800001,
                    title,
                    description,
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
            let title: String = "Title".to_string();
            let description: String = "Description".to_string();

            assert_err!(
                HermesGovernancePlatform::create_poll(
                    Origin::signed(BOB),
                    current_timestamp,
                    current_timestamp + 172800000,
                    title,
                    description,
                ),
                Error::<Runtime>::NotEnoughHermesForCreatingPoll
            );
        });
    }

    #[test]
    fn create_poll_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let poll_id: String = "Poll".to_string();
            let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
            let poll_end_timestamp = pallet_timestamp::Pallet::<Runtime>::get() + 172800000;
            let user = ALICE.into();
            let hermes_locked = pallet::MinimumHermesAmountForCreatingPoll::<Runtime>::get();
            let title = "Title".to_string();
            let description = "Description".to_string();

            let hermes_poll_info = HermesPollInfo {
                creator: user,
                hermes_locked,
                poll_start_timestamp,
                poll_end_timestamp,
                title,
                description,
                creator_hermes_withdrawn: false,
            };

            pallet::HermesPollData::<Runtime>::insert(&poll_id, &hermes_poll_info);

            let pallet_account = PalletId(*b"hermsgov").into_account_truncating();
            assert_ok!(Assets::transfer_from(
                &CERES_ASSET_ID.into(),
                &user,
                &pallet_account,
                hermes_poll_info.hermes_locked
            ));

            let poll_info = pallet::HermesPollData::<Runtime>::get(&poll_id).unwrap();
            assert_eq!(poll_info.poll_start_timestamp, poll_start_timestamp);
            assert_eq!(poll_info.poll_end_timestamp, poll_end_timestamp);
            assert_eq!(poll_info.creator_hermes_withdrawn, false);
            assert_eq!(poll_info.hermes_locked, hermes_locked);

            // Check ALICE's balances
            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &ALICE)
                    .expect("Failed to query free balance."),
                balance!(2800)
            );

            // Check pallet's balances
            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &pallet_account)
                    .expect("Failed to query free balance."),
                hermes_locked
            );
        });
    }

    #[test]
    fn vote_invalid_number_of_option() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let poll_id = "Poll".to_string();
            let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
            let poll_end_timestamp = pallet_timestamp::Pallet::<Runtime>::get() + 172800000;
            let hermes_locked = pallet::MinimumHermesAmountForCreatingPoll::<Runtime>::get();
            let user = ALICE.into();

            let hermes_poll_info = HermesPollInfo {
                creator: user,
                hermes_locked,
                poll_start_timestamp,
                poll_end_timestamp,
                title: "Title".to_string(),
                description: "Description".to_string(),
                creator_hermes_withdrawn: false,
            };

            pallet::HermesPollData::<Runtime>::insert(&poll_id, &hermes_poll_info);

            assert_err!(
                HermesGovernancePlatform::vote(Origin::signed(ALICE), poll_id, 3,),
                Error::<Runtime>::InvalidNumberOfOption
            );
        });
    }

    #[test]
    fn vote_poll_does_not_exist() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let poll_id = "Poll".to_string();

            assert_err!(
                HermesGovernancePlatform::vote(Origin::signed(ALICE), poll_id, 2,),
                Error::<Runtime>::PollDoesNotExist
            );
        });
    }

    #[test]
    fn vote_poll_is_not_started() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let poll_id = "Poll".to_string();
            let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get() + 1;
            let poll_end_timestamp = pallet_timestamp::Pallet::<Runtime>::get() + 172800000;
            let user = ALICE.into();
            let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
            let hermes_locked = pallet::MinimumHermesAmountForCreatingPoll::<Runtime>::get();

            pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp);

            let hermes_poll_info = HermesPollInfo {
                creator: user,
                hermes_locked,
                poll_start_timestamp,
                poll_end_timestamp,
                title: "Title".to_string(),
                description: "Description".to_string(),
                creator_hermes_withdrawn: false,
            };

            pallet::HermesPollData::<Runtime>::insert(&poll_id, &hermes_poll_info);

            assert_err!(
                HermesGovernancePlatform::vote(Origin::signed(ALICE), poll_id, 2,),
                Error::<Runtime>::PollIsNotStarted
            );
        });
    }

    #[test]
    fn vote_poll_is_finished() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let poll_id = "Poll".to_string();
            let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
            let poll_end_timestamp = pallet_timestamp::Pallet::<Runtime>::get() + 604800000;
            let user = ALICE.into();
            let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
            let hermes_locked = pallet::MinimumHermesAmountForCreatingPoll::<Runtime>::get();

            pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 604800001);

            let hermes_poll_info = HermesPollInfo {
                creator: user,
                hermes_locked,
                poll_start_timestamp,
                poll_end_timestamp,
                title: "Title".to_string(),
                description: "Description".to_string(),
                creator_hermes_withdrawn: false,
            };

            pallet::HermesPollData::<Runtime>::insert(&poll_id, &hermes_poll_info);

            assert_err!(
                HermesGovernancePlatform::vote(Origin::signed(ALICE), poll_id, 2,),
                Error::<Runtime>::PollIsFinished
            );
        });
    }

    #[test]
    fn vote_not_enough_hermes_for_voting() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let poll_id = "Poll".to_string();
            let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
            let poll_end_timestamp = pallet_timestamp::Pallet::<Runtime>::get() + 604800000;
            let hermes_locked = pallet::MinimumHermesAmountForCreatingPoll::<Runtime>::get();
            let user = ALICE.into();

            let hermes_poll_info = HermesPollInfo {
                creator: user,
                hermes_locked,
                poll_start_timestamp,
                poll_end_timestamp,
                title: "Title".to_string(),
                description: "Description".to_string(),
                creator_hermes_withdrawn: false,
            };

            pallet::HermesPollData::<Runtime>::insert(&poll_id, &hermes_poll_info);

            assert_err!(
                HermesGovernancePlatform::vote(Origin::signed(BOB), poll_id, 2,),
                Error::<Runtime>::NotEnoughHermesForVoting
            );
        });
    }

    #[test]
    fn vote_already_voted() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let poll_id = "Poll".to_string();
            let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
            let poll_end_timestamp = pallet_timestamp::Pallet::<Runtime>::get() + 604800000;
            let user = ALICE.into();
            let hermes_locked = pallet::MinimumHermesAmountForCreatingPoll::<Runtime>::get();

            let hermes_poll_info = HermesPollInfo {
                creator: user,
                hermes_locked,
                poll_start_timestamp,
                poll_end_timestamp,
                title: "Title".to_string(),
                description: "Description".to_string(),
                creator_hermes_withdrawn: false,
            };

            pallet::HermesPollData::<Runtime>::insert(&poll_id, &hermes_poll_info);

            assert_err!(
                HermesGovernancePlatform::vote(Origin::signed(BOB), poll_id, 1,),
                Error::<Runtime>::NotEnoughHermesForVoting
            );
        });
    }

    #[test]
    fn vote_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let poll_id = "Poll".to_string();
            let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
            let poll_end_timestamp = pallet_timestamp::Pallet::<Runtime>::get() + 604800000;
            let user = ALICE.into();
            let voting_option = 1;
            let number_of_hermes = pallet::MinimumHermesVotingAmount::<Runtime>::get();
            let hermes_locked = pallet::MinimumHermesAmountForCreatingPoll::<Runtime>::get();

            let hermes_poll_info = HermesPollInfo {
                creator: user,
                hermes_locked,
                poll_start_timestamp,
                poll_end_timestamp,
                title: "Title".to_string(),
                description: "Description".to_string(),
                creator_hermes_withdrawn: false,
            };

            pallet::HermesPollData::<Runtime>::insert(&poll_id, &hermes_poll_info);

            assert_ok!(HermesGovernancePlatform::vote(
                Origin::signed(ALICE),
                poll_id.clone(),
                voting_option,
            ));

            let hermes_voting_info = pallet::HermesVotings::<Runtime>::get(&poll_id, &ALICE);
            assert_eq!(hermes_voting_info.voting_option, voting_option);
            assert_eq!(hermes_voting_info.hermes_withdrawn, false);
            assert_eq!(hermes_voting_info.number_of_hermes, number_of_hermes);

            // Check ALICE's balances
            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &ALICE)
                    .expect("Failed to query free balance."),
                balance!(2900)
            );

            // Check pallet's balances
            let hermes_governance = PalletId(*b"hermsgov").into_account_truncating();
            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &hermes_governance)
                    .expect("Failed to query free balance."),
                number_of_hermes
            );
        });
    }

    #[test]
    fn withdraw_funds_voter_poll_does_not_exist() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let poll_id = "Poll".to_string();

            assert_err!(
                HermesGovernancePlatform::withdraw_funds_voter(Origin::signed(ALICE), poll_id,),
                Error::<Runtime>::PollDoesNotExist
            );
        });
    }

    #[test]
    fn withdraw_funds_voter_poll_is_not_finished() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let poll_id = "Poll".to_string();
            let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
            let poll_end_timestamp = pallet_timestamp::Pallet::<Runtime>::get() + 604800000;
            let user = ALICE.into();
            let voting_option = 1;
            let hermes_locked = pallet::MinimumHermesAmountForCreatingPoll::<Runtime>::get();
            let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();

            pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp);

            let hermes_poll_info = HermesPollInfo {
                creator: user,
                hermes_locked,
                poll_start_timestamp,
                poll_end_timestamp,
                title: "Title".to_string(),
                description: "Description".to_string(),
                creator_hermes_withdrawn: false,
            };

            pallet::HermesPollData::<Runtime>::insert(&poll_id, &hermes_poll_info);

            assert_ok!(HermesGovernancePlatform::vote(
                Origin::signed(ALICE),
                poll_id.clone(),
                voting_option,
            ));

            assert_err!(
                HermesGovernancePlatform::withdraw_funds_voter(Origin::signed(ALICE), poll_id,),
                Error::<Runtime>::PollIsNotFinished
            );
        });
    }

    #[test]
    fn withdraw_funds_voter_funds_already_withdrawn() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let poll_id = "Poll".to_string();
            let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
            let poll_end_timestamp = pallet_timestamp::Pallet::<Runtime>::get() + 604800000;
            let user = ALICE.into();
            let voting_option = 1;
            let hermes_locked = pallet::MinimumHermesAmountForCreatingPoll::<Runtime>::get();
            let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();

            let hermes_poll_info = HermesPollInfo {
                creator: user,
                hermes_locked,
                poll_start_timestamp,
                poll_end_timestamp,
                title: "Title".to_string(),
                description: "Description".to_string(),
                creator_hermes_withdrawn: false,
            };

            pallet::HermesPollData::<Runtime>::insert(&poll_id, &hermes_poll_info);

            assert_ok!(HermesGovernancePlatform::vote(
                Origin::signed(ALICE),
                poll_id.clone(),
                voting_option,
            ));

            pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 604900000);

            assert_ok!(HermesGovernancePlatform::withdraw_funds_voter(
                Origin::signed(ALICE),
                poll_id.clone()
            ));

            assert_err!(
                HermesGovernancePlatform::withdraw_funds_voter(
                    Origin::signed(ALICE),
                    poll_id.clone(),
                ),
                Error::<Runtime>::FundsAlreadyWithdrawn
            );
        });
    }

    #[test]
    fn withdraw_funds_voter_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let poll_id = "Poll".to_string();
            let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
            let poll_end_timestamp = pallet_timestamp::Pallet::<Runtime>::get() + 604800000;
            let user = ALICE.into();
            let voting_option = 1;
            let hermes_locked = pallet::MinimumHermesAmountForCreatingPoll::<Runtime>::get();
            let number_of_hermes = pallet::MinimumHermesVotingAmount::<Runtime>::get();
            let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();

            let hermes_poll_info = HermesPollInfo {
                creator: user,
                hermes_locked,
                poll_start_timestamp,
                poll_end_timestamp,
                title: "Title".to_string(),
                description: "Description".to_string(),
                creator_hermes_withdrawn: false,
            };

            pallet::HermesPollData::<Runtime>::insert(&poll_id, &hermes_poll_info);

            assert_ok!(HermesGovernancePlatform::vote(
                Origin::signed(ALICE),
                poll_id.clone(),
                voting_option,
            ));

            pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 604900000);

            assert_ok!(HermesGovernancePlatform::withdraw_funds_voter(
                Origin::signed(ALICE),
                poll_id.clone()
            ));

            let hermes_voting_info = pallet::HermesVotings::<Runtime>::get(&poll_id, &ALICE);
            assert_eq!(hermes_voting_info.voting_option, voting_option);
            assert_eq!(hermes_voting_info.number_of_hermes, number_of_hermes);
            assert_eq!(hermes_voting_info.hermes_withdrawn, true);

            // Check ALICE's balances
            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &ALICE)
                    .expect("Failed to query free balance."),
                balance!(3000)
            );

            // Check pallet's balances
            let hermes_governance = PalletId(*b"hermsgov").into_account_truncating();
            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &hermes_governance)
                    .expect("Failed to query free balance."),
                balance!(0)
            );
        });
    }

    #[test]
    fn withdraw_funds_creator_poll_does_not_exist() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let poll_id = "Poll".to_string();

            assert_err!(
                HermesGovernancePlatform::withdraw_funds_creator(Origin::signed(ALICE), poll_id,),
                Error::<Runtime>::PollDoesNotExist
            );
        });
    }

    #[test]
    fn withdraw_funds_creator_you_are_not_creator() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let poll_id = "Poll".to_string();
            let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
            let poll_end_timestamp = pallet_timestamp::Pallet::<Runtime>::get() + 604800000;
            let user = ALICE.into();
            let voting_option = 1;
            let hermes_locked = pallet::MinimumHermesAmountForCreatingPoll::<Runtime>::get();
            let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();

            let hermes_poll_info = HermesPollInfo {
                creator: user,
                hermes_locked,
                poll_start_timestamp,
                poll_end_timestamp,
                title: "Title".to_string(),
                description: "Description".to_string(),
                creator_hermes_withdrawn: false,
            };

            pallet::HermesPollData::<Runtime>::insert(&poll_id, &hermes_poll_info);

            pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 604900000);

            assert_err!(
                HermesGovernancePlatform::withdraw_funds_creator(
                    Origin::signed(BOB),
                    poll_id.clone(),
                ),
                Error::<Runtime>::YouAreNotCreator
            );
        });
    }

    #[test]
    fn withdraw_funds_creator_poll_is_not_finished() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let poll_id = "Poll".to_string();
            let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
            let poll_end_timestamp = pallet_timestamp::Pallet::<Runtime>::get() + 604800000;
            let user = ALICE.into();
            let voting_option = 1;
            let hermes_locked = pallet::MinimumHermesAmountForCreatingPoll::<Runtime>::get();

            let hermes_poll_info = HermesPollInfo {
                creator: user,
                hermes_locked,
                poll_start_timestamp,
                poll_end_timestamp,
                title: "Title".to_string(),
                description: "Description".to_string(),
                creator_hermes_withdrawn: false,
            };

            pallet::HermesPollData::<Runtime>::insert(&poll_id, &hermes_poll_info);

            assert_err!(
                HermesGovernancePlatform::withdraw_funds_creator(
                    Origin::signed(ALICE),
                    poll_id.clone(),
                ),
                Error::<Runtime>::PollIsNotFinished
            );
        });
    }

    #[test]
    fn withdraw_funds_creator_funds_already_withdrawn() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let poll_id = "Poll".to_string();
            let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
            let poll_end_timestamp = pallet_timestamp::Pallet::<Runtime>::get() + 604800000;
            let user = ALICE.into();
            let hermes_locked = pallet::MinimumHermesAmountForCreatingPoll::<Runtime>::get();
            let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();

            let mut hermes_poll_info = HermesPollInfo {
                creator: user,
                hermes_locked,
                poll_start_timestamp,
                poll_end_timestamp,
                title: "Title".to_string(),
                description: "Description".to_string(),
                creator_hermes_withdrawn: false,
            };

            pallet::HermesPollData::<Runtime>::insert(&poll_id, &hermes_poll_info);

            let pallet_account = PalletId(*b"hermsgov").into_account_truncating();
            assert_ok!(Assets::transfer_from(
                &CERES_ASSET_ID.into(),
                &user,
                &pallet_account,
                hermes_poll_info.hermes_locked
            ));

            pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 604900000);

            assert_ok!(HermesGovernancePlatform::withdraw_funds_creator(
                Origin::signed(ALICE),
                poll_id.clone(),
            ));

            assert_err!(
                HermesGovernancePlatform::withdraw_funds_creator(
                    Origin::signed(ALICE),
                    poll_id.clone(),
                ),
                Error::<Runtime>::FundsAlreadyWithdrawn
            );
        });
    }

    #[test]
    fn withdraw_funds_creator_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let poll_id = "Poll".to_string();
            let poll_start_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
            let poll_end_timestamp = pallet_timestamp::Pallet::<Runtime>::get() + 604800000;
            let user = ALICE.into();
            let hermes_locked = pallet::MinimumHermesAmountForCreatingPoll::<Runtime>::get();
            let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();

            let mut hermes_poll_info = HermesPollInfo {
                creator: user,
                hermes_locked,
                poll_start_timestamp,
                poll_end_timestamp,
                title: "Title".to_string(),
                description: "Description".to_string(),
                creator_hermes_withdrawn: false,
            };

            pallet::HermesPollData::<Runtime>::insert(&poll_id, &hermes_poll_info);

            let pallet_account = PalletId(*b"hermsgov").into_account_truncating();
            assert_ok!(Assets::transfer_from(
                &CERES_ASSET_ID.into(),
                &user,
                &pallet_account,
                hermes_poll_info.hermes_locked
            ));

            pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 604900000);

            assert_ok!(HermesGovernancePlatform::withdraw_funds_creator(
                Origin::signed(ALICE),
                poll_id.clone()
            ));

            let hermes_info = pallet::HermesPollData::<Runtime>::get(&poll_id).unwrap();

            assert_eq!(hermes_info.creator_hermes_withdrawn, true);

            // Check ALICE's balances
            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &ALICE)
                    .expect("Failed to query free balance."),
                balance!(3000)
            );

            // Check pallet's balances
            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &pallet_account)
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
                    Origin::signed(ALICE),
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
                Origin::signed(pallet::AuthorityAccount::<Runtime>::get()),
                balance!(100)
            ));

            assert_eq!(
                pallet::MinimumHermesVotingAmount::<Runtime>::get(),
                balance!(100),
            );
        });
    }

    #[test]
    fn change_min_hermes_for_creating_poll_unauthorized() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_err!(
                HermesGovernancePlatform::change_min_hermes_for_creating_poll(
                    Origin::signed(ALICE),
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
                    Origin::signed(pallet::AuthorityAccount::<Runtime>::get()),
                    balance!(100)
                )
            );

            assert_eq!(
                pallet::MinimumHermesAmountForCreatingPoll::<Runtime>::get(),
                balance!(100),
            );
        });
    }
}
