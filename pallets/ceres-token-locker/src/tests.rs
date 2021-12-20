mod tests {
    use crate::mock::*;
    use crate::{pallet, Error};
    use common::balance;
    use frame_support::{assert_err, assert_ok};
    use sp_runtime::ModuleId;
    use sp_runtime::traits::AccountIdConversion;

    #[test]
    fn lock_tokens_invalid_number_of_tokens() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_err!(
                CeresTokenLocker::lock_tokens(Origin::signed(ALICE), CERES_ASSET_ID, frame_system::Pallet::<Runtime>::block_number() + 1, balance!(0)),
                Error::<Runtime>::InvalidNumberOfTokens
            );
        });
    }

    #[test]
    fn lock_tokens_invalid_unlocking_block() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_err!(
                CeresTokenLocker::lock_tokens(Origin::signed(ALICE), CERES_ASSET_ID, frame_system::Pallet::<Runtime>::block_number(), balance!(1)),
                Error::<Runtime>::InvalidUnlockingBlock
            );
        });
    }

    #[test]
    fn lock_tokens_not_enough_funds() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_err!(
                CeresTokenLocker::lock_tokens(Origin::signed(ALICE), CERES_ASSET_ID, frame_system::Pallet::<Runtime>::block_number() + 1, balance!(3000)),
                Error::<Runtime>::NotEnoughFunds
            );
        });
    }

    #[test]
    fn lock_tokens_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let unlocking_block = frame_system::Pallet::<Runtime>::block_number() + 1;
            let locked_tokens = balance!(2000);
            assert_ok!(
                CeresTokenLocker::lock_tokens(Origin::signed(ALICE), CERES_ASSET_ID, unlocking_block, locked_tokens),
            );

            // Check ALICE's balances
            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &ALICE)
                    .expect("Failed to query free balance."),
                balance!(990)
            );

            // Check pallet's balances
            let token_locker = ModuleId(*b"crstlock").into_account();
            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &token_locker)
                    .expect("Failed to query free balance."),
                locked_tokens
            );

            // Check fee's balance
            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &pallet::FeesAccount::<Runtime>::get())
                    .expect("Failed to query free balance."),
                balance!(10)
            );

            // Check TokenLockerData map
            let token_locker_vec = pallet::TokenLockerData::<Runtime>::get(&ALICE);
            assert_eq!(token_locker_vec.len(), 1);
            assert_eq!(token_locker_vec.get(0).unwrap().asset_id, CERES_ASSET_ID);
            assert_eq!(token_locker_vec.get(0).unwrap().unlocking_block, unlocking_block);
            assert_eq!(token_locker_vec.get(0).unwrap().tokens, locked_tokens);
        });
    }

    #[test]
    fn withdraw_tokens_invalid_number_of_tokens() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_err!(
                CeresTokenLocker::withdraw_tokens(Origin::signed(ALICE), CERES_ASSET_ID, frame_system::Pallet::<Runtime>::block_number() + 1, balance!(0)),
                Error::<Runtime>::InvalidNumberOfTokens
            );
        });
    }

    #[test]
    fn withdraw_tokens_not_unlocked_yet() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_err!(
                CeresTokenLocker::withdraw_tokens(Origin::signed(ALICE), CERES_ASSET_ID, frame_system::Pallet::<Runtime>::block_number(), balance!(1)),
                Error::<Runtime>::NotUnlockedYet
            );
        });
    }

    #[test]
    fn withdraw_tokens_lock_info_does_not_exist() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            run_to_block(5);
            assert_err!(
                CeresTokenLocker::withdraw_tokens(Origin::signed(ALICE), CERES_ASSET_ID, 1u32.into(), balance!(1)),
                Error::<Runtime>::LockInfoDoesNotExist
            );
        });
    }

    #[test]
    fn withdraw_tokens_lock_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let unlocking_block = frame_system::Pallet::<Runtime>::block_number() + 1;
            let locked_tokens = balance!(2000);

            // Lock tokens
            assert_ok!(
                CeresTokenLocker::lock_tokens(Origin::signed(ALICE), CERES_ASSET_ID, unlocking_block, locked_tokens),
            );

            // Check TokenLockerData map
            let mut token_locker_vec = pallet::TokenLockerData::<Runtime>::get(&ALICE);
            assert_eq!(token_locker_vec.len(), 1);

            run_to_block(5);

            // Unlock tokens
            assert_ok!(
                CeresTokenLocker::withdraw_tokens(Origin::signed(ALICE), CERES_ASSET_ID, unlocking_block, locked_tokens),
            );

            // Check TokenLockerData map
            token_locker_vec = pallet::TokenLockerData::<Runtime>::get(&ALICE);
            assert_eq!(token_locker_vec.len(), 0);

            // Check ALICE's balances
            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &ALICE)
                    .expect("Failed to query free balance."),
                balance!(2990)
            );

            // Check pallet's balances
            let token_locker = ModuleId(*b"crstlock").into_account();
            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &token_locker)
                    .expect("Failed to query free balance."),
                balance!(0)
            );
        });
    }

    #[test]
    fn change_fee_unauthorized() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_err!(
                CeresTokenLocker::change_fee(Origin::signed(ALICE), balance!(0.01)),
                Error::<Runtime>::Unauthorized
            );
        });
    }

    #[test]
    fn change_fee_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let new_fee = balance!(0.01);

            assert_ok!(CeresTokenLocker::change_fee(Origin::signed(pallet::AuthorityAccount::<Runtime>::get()), new_fee));

            assert_eq!(
                pallet::FeeAmount::<Runtime>::get(),
                new_fee
            );
        });
    }
}
