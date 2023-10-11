use crate::mock::*;
use crate::{pallet, Error};
use common::prelude::FixedWrapper;
use common::{balance, AssetInfoProvider, CERES_ASSET_ID};
use frame_support::{assert_err, assert_ok, PalletId};
use sp_runtime::traits::AccountIdConversion;

#[test]
fn should_not_allow_deposit_to_full_staking_pool() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_err!(
            CeresStaking::deposit(RuntimeOrigin::signed(ALICE), balance!(7201)),
            Error::<Runtime>::StakingPoolIsFull
        );
    });
}

#[test]
fn should_deposit_to_staking_pool() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        // Deposit 500 from Alice's account
        assert_ok!(CeresStaking::deposit(
            RuntimeOrigin::signed(ALICE),
            balance!(500)
        ));

        // Get staking pool account id
        let staking_pool = PalletId(*b"cerstake").into_account_truncating();

        // Check Alice's balance
        assert_eq!(
            Assets::free_balance(&CERES_ASSET_ID, &ALICE).expect("Failed to query free balance."),
            balance!(6800)
        );
        // Check staking pool's balance
        assert_eq!(
            Assets::free_balance(&CERES_ASSET_ID, &staking_pool)
                .expect("Failed to query free balance."),
            balance!(500)
        );
        // Check total deposited
        assert_eq!(pallet::TotalDeposited::<Runtime>::get(), balance!(500));
        // Check Stakers map
        let staking_info = pallet::Stakers::<Runtime>::get(ALICE);
        assert_eq!(staking_info.deposited, balance!(500));

        // Deposit 250 more from Alice's account
        assert_ok!(CeresStaking::deposit(
            RuntimeOrigin::signed(ALICE),
            balance!(250)
        ));
        // Check Alice's balance
        assert_eq!(
            Assets::free_balance(&CERES_ASSET_ID, &ALICE).expect("Failed to query free balance."),
            balance!(6550)
        );
        // Check staking pool's balance
        assert_eq!(
            Assets::free_balance(&CERES_ASSET_ID, &staking_pool)
                .expect("Failed to query free balance."),
            balance!(750)
        );
        // Check total deposited
        assert_eq!(pallet::TotalDeposited::<Runtime>::get(), balance!(750));
        // Check Stakers map
        let staking_info = pallet::Stakers::<Runtime>::get(ALICE);
        assert_eq!(staking_info.deposited, balance!(750));

        // Deposit 50 from BOB's account
        assert_ok!(CeresStaking::deposit(
            RuntimeOrigin::signed(BOB),
            balance!(50)
        ));
        // Check Bob's balance
        assert_eq!(
            Assets::free_balance(&CERES_ASSET_ID, &BOB).expect("Failed to query free balance."),
            balance!(50)
        );
        // Check staking pool's balance
        assert_eq!(
            Assets::free_balance(&CERES_ASSET_ID, &staking_pool)
                .expect("Failed to query free balance."),
            balance!(800)
        );
        // Check total deposited
        assert_eq!(pallet::TotalDeposited::<Runtime>::get(), balance!(800));
        // Check Stakers map for Alice
        let staking_info_alice = pallet::Stakers::<Runtime>::get(ALICE);
        assert_eq!(staking_info_alice.deposited, balance!(750));
        // Check Stakers map for Bob
        let staking_info_bob = pallet::Stakers::<Runtime>::get(BOB);
        assert_eq!(staking_info_bob.deposited, balance!(50));
    });
}

#[test]
fn should_withdraw_from_staking_pool() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        // Deposit 1200 from Alice's account
        assert_ok!(CeresStaking::deposit(
            RuntimeOrigin::signed(ALICE),
            balance!(1200)
        ));
        // Deposit 50 from Bob's account
        assert_ok!(CeresStaking::deposit(
            RuntimeOrigin::signed(BOB),
            balance!(50)
        ));

        // Add rewards to Alice
        let mut staking_info = pallet::Stakers::<Runtime>::get(ALICE);
        staking_info.rewards += balance!(11);
        pallet::Stakers::<Runtime>::insert(ALICE, staking_info);

        // Withdraw Alice's stake
        assert_ok!(CeresStaking::withdraw(RuntimeOrigin::signed(ALICE)));
        // Check Alice's balance
        assert_eq!(
            Assets::free_balance(&CERES_ASSET_ID, &ALICE).expect("Failed to query free balance."),
            balance!(7311)
        );
        // Check total deposited
        assert_eq!(pallet::TotalDeposited::<Runtime>::get(), balance!(50));
        // Check Stakers map
        let staking_info_alice = pallet::Stakers::<Runtime>::get(ALICE);
        assert_eq!(staking_info_alice.deposited, balance!(0));
        assert_eq!(staking_info_alice.rewards, balance!(0));
        // Check staking pool's balance
        let staking_pool = PalletId(*b"cerstake").into_account_truncating();
        assert_eq!(
            Assets::free_balance(&CERES_ASSET_ID, &staking_pool)
                .expect("Failed to query free balance."),
            balance!(39)
        );
    });
}

#[test]
fn should_calculate_rewards_and_withdraw_from_staking_pool() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        // Deposit 500 from Alice's account
        assert_ok!(CeresStaking::deposit(
            RuntimeOrigin::signed(ALICE),
            balance!(500)
        ));
        assert_ok!(CeresStaking::deposit(
            RuntimeOrigin::signed(BOB),
            balance!(50)
        ));
        run_to_block(14_440);
        let diff = FixedWrapper::from(0.0001);
        // Check remaining rewards
        let remaining_rewards = pallet::RewardsRemaining::<Runtime>::get();
        assert!((FixedWrapper::from(593.333333333) - FixedWrapper::from(remaining_rewards)) < diff);
        // Check Alice's staking rewards
        let staking_info_alice = pallet::Stakers::<Runtime>::get(ALICE);
        assert!(
            (FixedWrapper::from(staking_info_alice.rewards) - FixedWrapper::from(6.0606060606))
                < diff
        );
        // Check Bob's staking rewards
        let staking_info_bob = pallet::Stakers::<Runtime>::get(BOB);
        assert!(
            (FixedWrapper::from(staking_info_bob.rewards) - FixedWrapper::from(0.606060606)) < diff
        );
        // Withdraw Alice's stake
        assert_ok!(CeresStaking::withdraw(RuntimeOrigin::signed(ALICE)));
        // Check Alice's balance after withdrawal
        let alice_balance =
            Assets::free_balance(&CERES_ASSET_ID, &ALICE).expect("Failed to query free balance.");
        assert!((FixedWrapper::from(7306.0606060606) - FixedWrapper::from(alice_balance)) < diff);
    });
}

#[test]
fn change_rewards_remaining_unauthorized() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_err!(
            CeresStaking::change_rewards_remaining(RuntimeOrigin::signed(ALICE), balance!(100)),
            Error::<Runtime>::Unauthorized
        );
    });
}

#[test]
fn change_rewards_remaining_ok() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_ok!(CeresStaking::change_rewards_remaining(
            RuntimeOrigin::signed(pallet::AuthorityAccount::<Runtime>::get()),
            balance!(100)
        ));

        assert_eq!(pallet::RewardsRemaining::<Runtime>::get(), balance!(100));
    });
}
