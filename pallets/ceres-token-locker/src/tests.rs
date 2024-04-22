mod tests {
    use crate::mock::*;
    use crate::{pallet, AccountIdOf, Error};
    use common::{
        balance, generate_storage_instance, AssetIdOf, AssetInfoProvider, Balance, CERES_ASSET_ID,
    };
    use frame_support::pallet_prelude::StorageMap;
    use frame_support::storage::types::ValueQuery;
    use frame_support::traits::Hooks;
    use frame_support::{assert_err, assert_ok, Identity, PalletId};
    use sp_runtime::traits::AccountIdConversion;

    #[test]
    fn lock_tokens_invalid_number_of_tokens() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_err!(
                CeresTokenLocker::lock_tokens(
                    RuntimeOrigin::signed(ALICE),
                    CERES_ASSET_ID,
                    pallet_timestamp::Pallet::<Runtime>::get() + 1,
                    balance!(0)
                ),
                Error::<Runtime>::InvalidNumberOfTokens
            );
        });
    }

    #[test]
    fn lock_tokens_invalid_unlocking_timestamp() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_err!(
                CeresTokenLocker::lock_tokens(
                    RuntimeOrigin::signed(ALICE),
                    CERES_ASSET_ID,
                    pallet_timestamp::Pallet::<Runtime>::get(),
                    balance!(1)
                ),
                Error::<Runtime>::InvalidUnlockingTimestamp
            );
        });
    }

    #[test]
    fn lock_tokens_not_enough_funds() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_err!(
                CeresTokenLocker::lock_tokens(
                    RuntimeOrigin::signed(ALICE),
                    CERES_ASSET_ID,
                    pallet_timestamp::Pallet::<Runtime>::get() + 1,
                    balance!(3000)
                ),
                Error::<Runtime>::NotEnoughFunds
            );
        });
    }

    #[test]
    fn lock_tokens_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let unlocking_timestamp = pallet_timestamp::Pallet::<Runtime>::get() + 1;
            let locked_tokens = balance!(2000);
            assert_ok!(CeresTokenLocker::lock_tokens(
                RuntimeOrigin::signed(ALICE),
                CERES_ASSET_ID,
                unlocking_timestamp,
                locked_tokens
            ),);

            // Check ALICE's balances
            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &ALICE)
                    .expect("Failed to query free balance."),
                balance!(990)
            );

            // Check pallet's balances
            let token_locker = PalletId(*b"crstlock").into_account_truncating();
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
            assert_eq!(
                token_locker_vec.get(0).unwrap().unlocking_timestamp,
                unlocking_timestamp
            );
            assert_eq!(token_locker_vec.get(0).unwrap().tokens, locked_tokens);
        });
    }

    #[test]
    fn withdraw_tokens_invalid_number_of_tokens() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_err!(
                CeresTokenLocker::withdraw_tokens(
                    RuntimeOrigin::signed(ALICE),
                    CERES_ASSET_ID,
                    pallet_timestamp::Pallet::<Runtime>::get() + 1,
                    balance!(0)
                ),
                Error::<Runtime>::InvalidNumberOfTokens
            );
        });
    }

    #[test]
    fn withdraw_tokens_not_unlocked_yet() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_err!(
                CeresTokenLocker::withdraw_tokens(
                    RuntimeOrigin::signed(ALICE),
                    CERES_ASSET_ID,
                    pallet_timestamp::Pallet::<Runtime>::get(),
                    balance!(1)
                ),
                Error::<Runtime>::NotUnlockedYet
            );
        });
    }

    #[test]
    fn withdraw_tokens_lock_info_does_not_exist() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let unlocking_timestamp = pallet_timestamp::Pallet::<Runtime>::get() + 1;
            pallet_timestamp::Pallet::<Runtime>::set_timestamp(unlocking_timestamp + 1);
            assert_err!(
                CeresTokenLocker::withdraw_tokens(
                    RuntimeOrigin::signed(ALICE),
                    CERES_ASSET_ID,
                    1u32.into(),
                    balance!(1)
                ),
                Error::<Runtime>::LockInfoDoesNotExist
            );
        });
    }

    #[test]
    fn withdraw_tokens_lock_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let unlocking_timestamp = pallet_timestamp::Pallet::<Runtime>::get() + 1;
            let locked_tokens = balance!(2000);

            // Lock tokens
            assert_ok!(CeresTokenLocker::lock_tokens(
                RuntimeOrigin::signed(ALICE),
                CERES_ASSET_ID,
                unlocking_timestamp,
                locked_tokens
            ),);

            // Check TokenLockerData map
            let mut token_locker_vec = pallet::TokenLockerData::<Runtime>::get(&ALICE);
            assert_eq!(token_locker_vec.len(), 1);

            pallet_timestamp::Pallet::<Runtime>::set_timestamp(unlocking_timestamp + 1);

            // Unlock tokens
            assert_ok!(CeresTokenLocker::withdraw_tokens(
                RuntimeOrigin::signed(ALICE),
                CERES_ASSET_ID,
                unlocking_timestamp,
                locked_tokens
            ),);

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
            let token_locker = PalletId(*b"crstlock").into_account_truncating();
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
                CeresTokenLocker::change_fee(RuntimeOrigin::signed(ALICE), balance!(0.01)),
                Error::<Runtime>::Unauthorized
            );
        });
    }

    #[test]
    fn change_fee_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let new_fee = balance!(0.01);

            assert_ok!(CeresTokenLocker::change_fee(
                RuntimeOrigin::signed(pallet::AuthorityAccount::<Runtime>::get()),
                new_fee
            ));

            assert_eq!(pallet::FeeAmount::<Runtime>::get(), new_fee);
        });
    }

    #[test]
    fn token_locker_storage_migration_works() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            generate_storage_instance!(CeresTokenLocker, TokenLockerData);
            type OldLockerData = StorageMap<
                TokenLockerDataOldInstance,
                Identity,
                AccountIdOf<Runtime>,
                Vec<(Balance, BlockNumber, AssetIdOf<Runtime>)>,
                ValueQuery,
            >;

            let mut alice_vec: Vec<(Balance, BlockNumber, AssetIdOf<Runtime>)> = Vec::new();
            alice_vec.push((balance!(5), 8660039u64, CERES_ASSET_ID));
            alice_vec.push((balance!(6), 16052893u64, CERES_ASSET_ID));

            OldLockerData::insert(ALICE, alice_vec);

            let mut bob_vec: Vec<(Balance, BlockNumber, AssetIdOf<Runtime>)> = Vec::new();
            bob_vec.push((balance!(7), 3u64, CERES_ASSET_ID));

            OldLockerData::insert(BOB, bob_vec);

            pallet_timestamp::Pallet::<Runtime>::set_timestamp(10000000);
            run_to_block(5);

            // Storage migration
            CeresTokenLocker::on_runtime_upgrade();

            let lockups_alice = pallet::TokenLockerData::<Runtime>::get(&ALICE);
            for lockup in lockups_alice {
                if lockup.tokens == balance!(5) {
                    assert_eq!(lockup.unlocking_timestamp, 51970204000);
                } else if lockup.tokens == balance!(6) {
                    assert_eq!(lockup.unlocking_timestamp, 96327328000);
                }
            }

            let lockups_bob = pallet::TokenLockerData::<Runtime>::get(&BOB);
            for lockup in lockups_bob {
                assert_eq!(lockup.unlocking_timestamp, 9988000);
            }

            // Storage version should be V2 so no changes made
            pallet_timestamp::Pallet::<Runtime>::set_timestamp(11000000);
            run_to_block(10);

            // Storage migration
            CeresTokenLocker::on_runtime_upgrade();

            let lockups_alice = pallet::TokenLockerData::<Runtime>::get(&ALICE);
            for lockup in lockups_alice {
                if lockup.tokens == balance!(5) {
                    assert_eq!(lockup.unlocking_timestamp, 51970204000);
                } else if lockup.tokens == balance!(6) {
                    assert_eq!(lockup.unlocking_timestamp, 96327328000);
                }
            }

            let lockups_bob = pallet::TokenLockerData::<Runtime>::get(&BOB);
            for lockup in lockups_bob {
                assert_eq!(lockup.unlocking_timestamp, 9988000);
            }
        });
    }
}
