mod tests {
    use crate::mock::*;
    use crate::{Error, pallet};
    use frame_support::{assert_err, assert_ok};
    use common::Balance;
    use sp_runtime::ModuleId;
    use sp_runtime::traits::AccountIdConversion;

    #[test]
    fn should_not_allow_deposit_to_full_staking_pool() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_err!(
                CeresStaking::deposit(Origin::signed(ALICE), 7201),
                Error::<Runtime>::StakingPoolIsFull
            );
        });
    }

    #[test]
    fn should_deposit_to_staking_pool() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            // Deposit 500 from Alice's account
            assert_ok!(CeresStaking::deposit(Origin::signed(ALICE), 500));

            // Get staking pool account id
            let staking_pool = ModuleId(*b"cerstake").into_account();

            // Check Alice's balance
            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &ALICE).expect("Failed to query free balance."),
                Balance::from(6800u32),
            );
            // Check staking pool's balance
            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &staking_pool).expect("Failed to query free balance."),
                Balance::from(500u32),
            );
            // Check total deposited
            assert_eq!(
                pallet::TotalDeposited::<Runtime>::get(),
                Balance::from(500u32),
            );
            // Check Stakers map
            let staking_info = pallet::Stakers::<Runtime>::get(&ALICE);
            assert_eq!(
                staking_info.deposited,
                Balance::from(500u32),
            );

            // Deposit 250 more from Alice's account
            assert_ok!(CeresStaking::deposit(Origin::signed(ALICE), 250));
            // Check Alice's balance
            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &ALICE).expect("Failed to query free balance."),
                Balance::from(6550u32),
            );
            // Check staking pool's balance
            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &staking_pool).expect("Failed to query free balance."),
                Balance::from(750u32),
            );
            // Check total deposited
            assert_eq!(
                pallet::TotalDeposited::<Runtime>::get(),
                Balance::from(750u32),
            );
            // Check Stakers map
            let staking_info = pallet::Stakers::<Runtime>::get(&ALICE);
            assert_eq!(
                staking_info.deposited,
                Balance::from(750u32),
            );

            // Deposit 50 from BOB's account
            assert_ok!(CeresStaking::deposit(Origin::signed(BOB), 50));
            // Check Bob's balance
            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &BOB).expect("Failed to query free balance."),
                Balance::from(50u32),
            );
            // Check staking pool's balance
            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &staking_pool).expect("Failed to query free balance."),
                Balance::from(800u32),
            );
            // Check total deposited
            assert_eq!(
                pallet::TotalDeposited::<Runtime>::get(),
                Balance::from(800u32),
            );
            // Check Stakers map for Alice
            let staking_info_alice = pallet::Stakers::<Runtime>::get(&ALICE);
            assert_eq!(
                staking_info_alice.deposited,
                Balance::from(750u32),
            );
            // Check Stakers map for Bob
            let staking_info_bob = pallet::Stakers::<Runtime>::get(&BOB);
            assert_eq!(
                staking_info_bob.deposited,
                Balance::from(50u32),
            );
        });
    }

    #[test]
    fn should_withdraw_from_staking_pool() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            // Deposit 1200 from Alice's account
            assert_ok!(CeresStaking::deposit(Origin::signed(ALICE), 1200));
            // Deposit 50 from Bob's account
            assert_ok!(CeresStaking::deposit(Origin::signed(BOB), 50));

            // Add rewards to Alice
            let mut staking_info = pallet::Stakers::<Runtime>::get(&ALICE);
            staking_info.rewards = staking_info.rewards + 11;
            pallet::Stakers::<Runtime>::insert(&ALICE, staking_info);

            // Withdraw Alice's stake
            assert_ok!(CeresStaking::withdraw(Origin::signed(ALICE)));
            // Check Alice's balance
            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &ALICE).expect("Failed to query free balance."),
                Balance::from(7311u32),
            );
            // Check total deposited
            assert_eq!(
                pallet::TotalDeposited::<Runtime>::get(),
                Balance::from(50u32),
            );
            // Check Stakers map
            let staking_info_alice = pallet::Stakers::<Runtime>::get(&ALICE);
            assert_eq!(
                staking_info_alice.deposited,
                Balance::from(0u32),
            );
            assert_eq!(
                staking_info_alice.rewards,
                Balance::from(0u32),
            );
            // Check staking pool's balance
            let staking_pool = ModuleId(*b"cerstake").into_account();
            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &staking_pool).expect("Failed to query free balance."),
                Balance::from(39u32),
            );
        });
    }
}
