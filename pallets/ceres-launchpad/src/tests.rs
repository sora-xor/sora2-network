mod tests {
    use crate::mock::*;
    use crate::{pallet, Error, Pallet as CeresLaunchpadPallet};
    use common::balance;
    use frame_support::{assert_err, assert_ok};
    use sp_runtime::traits::AccountIdConversion;
    use sp_runtime::ModuleId;

    #[test]
    fn create_ilo_ilo_price_zero() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let current_block = frame_system::Pallet::<Runtime>::block_number();
            assert_err!(
                CeresLaunchpadPallet::<Runtime>::create_ilo(
                    Origin::signed(ALICE),
                    CERES_ASSET_ID.into(),
                    balance!(1),
                    balance!(2),
                    balance!(0),
                    balance!(100),
                    balance!(150),
                    balance!(0.2),
                    balance!(0.25),
                    true,
                    balance!(0.75),
                    balance!(0.6),
                    31,
                    current_block + 5,
                    current_block + 10,
                    balance!(0.1),
                    current_block + 3,
                    balance!(20)
                ),
                Error::<Runtime>::ParameterCantBeZero
            );
        });
    }

    #[test]
    fn create_ilo_hard_cap_zero() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let current_block = frame_system::Pallet::<Runtime>::block_number();
            assert_err!(
                CeresLaunchpadPallet::<Runtime>::create_ilo(
                    Origin::signed(ALICE),
                    CERES_ASSET_ID.into(),
                    balance!(1),
                    balance!(2),
                    balance!(0.25),
                    balance!(100),
                    balance!(0),
                    balance!(0.2),
                    balance!(0.25),
                    true,
                    balance!(0.75),
                    balance!(0.6),
                    31,
                    current_block + 5,
                    current_block + 10,
                    balance!(0.1),
                    current_block + 3,
                    balance!(20)
                ),
                Error::<Runtime>::ParameterCantBeZero
            );
        });
    }

    #[test]
    fn create_ilo_invalid_soft_cap() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let current_block = frame_system::Pallet::<Runtime>::block_number();
            assert_err!(
                CeresLaunchpadPallet::<Runtime>::create_ilo(
                    Origin::signed(ALICE),
                    CERES_ASSET_ID.into(),
                    balance!(1),
                    balance!(2),
                    balance!(0.25),
                    balance!(100),
                    balance!(220),
                    balance!(0.2),
                    balance!(0.25),
                    true,
                    balance!(0.75),
                    balance!(0.6),
                    31,
                    current_block + 5,
                    current_block + 10,
                    balance!(0.1),
                    current_block + 3,
                    balance!(20)
                ),
                Error::<Runtime>::InvalidSoftCap
            );
        });
    }

    #[test]
    fn create_ilo_invalid_minimum_contribution() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let current_block = frame_system::Pallet::<Runtime>::block_number();
            assert_err!(
                CeresLaunchpadPallet::<Runtime>::create_ilo(
                    Origin::signed(ALICE),
                    CERES_ASSET_ID.into(),
                    balance!(1),
                    balance!(2),
                    balance!(0.25),
                    balance!(120),
                    balance!(220),
                    balance!(0.009),
                    balance!(0.25),
                    true,
                    balance!(0.75),
                    balance!(0.6),
                    31,
                    current_block + 5,
                    current_block + 10,
                    balance!(0.1),
                    current_block + 3,
                    balance!(20)
                ),
                Error::<Runtime>::InvalidMinimumContribution
            );
        });
    }

    #[test]
    fn create_ilo_invalid_maximum_contribution() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let current_block = frame_system::Pallet::<Runtime>::block_number();
            assert_err!(
                CeresLaunchpadPallet::<Runtime>::create_ilo(
                    Origin::signed(ALICE),
                    CERES_ASSET_ID.into(),
                    balance!(1),
                    balance!(2),
                    balance!(0.25),
                    balance!(120),
                    balance!(220),
                    balance!(0.25),
                    balance!(0.25),
                    true,
                    balance!(0.75),
                    balance!(0.6),
                    31,
                    current_block + 5,
                    current_block + 10,
                    balance!(0.1),
                    current_block + 3,
                    balance!(20)
                ),
                Error::<Runtime>::InvalidMaximumContribution
            );
        });
    }

    #[test]
    fn create_ilo_invalid_liquidity_percent() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let current_block = frame_system::Pallet::<Runtime>::block_number();
            assert_err!(
                CeresLaunchpadPallet::<Runtime>::create_ilo(
                    Origin::signed(ALICE),
                    CERES_ASSET_ID.into(),
                    balance!(1),
                    balance!(2),
                    balance!(0.25),
                    balance!(120),
                    balance!(220),
                    balance!(0.2),
                    balance!(0.25),
                    true,
                    balance!(0.50),
                    balance!(0.6),
                    31,
                    current_block + 5,
                    current_block + 10,
                    balance!(0.1),
                    current_block + 3,
                    balance!(20)
                ),
                Error::<Runtime>::InvalidLiquidityPercent
            );
        });
    }

    #[test]
    fn create_ilo_invalid_lockup_days() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let current_block = frame_system::Pallet::<Runtime>::block_number();
            assert_err!(
                CeresLaunchpadPallet::<Runtime>::create_ilo(
                    Origin::signed(ALICE),
                    CERES_ASSET_ID.into(),
                    balance!(1),
                    balance!(2),
                    balance!(0.25),
                    balance!(120),
                    balance!(220),
                    balance!(0.2),
                    balance!(0.25),
                    true,
                    balance!(0.75),
                    balance!(0.6),
                    29,
                    current_block + 5,
                    current_block + 10,
                    balance!(0.1),
                    current_block + 3,
                    balance!(20)
                ),
                Error::<Runtime>::InvalidLockupDays
            );
        });
    }

    #[test]
    fn create_ilo_invalid_start_block() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let current_block = frame_system::Pallet::<Runtime>::block_number();
            assert_err!(
                CeresLaunchpadPallet::<Runtime>::create_ilo(
                    Origin::signed(ALICE),
                    CERES_ASSET_ID.into(),
                    balance!(1),
                    balance!(2),
                    balance!(0.25),
                    balance!(120),
                    balance!(220),
                    balance!(0.2),
                    balance!(0.25),
                    true,
                    balance!(0.75),
                    balance!(0.6),
                    31,
                    current_block,
                    current_block + 10,
                    balance!(0.1),
                    current_block + 3,
                    balance!(20)
                ),
                Error::<Runtime>::InvalidStartBlock
            );
        });
    }

    #[test]
    fn create_ilo_invalid_end_block() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let current_block = frame_system::Pallet::<Runtime>::block_number();
            assert_err!(
                CeresLaunchpadPallet::<Runtime>::create_ilo(
                    Origin::signed(ALICE),
                    CERES_ASSET_ID.into(),
                    balance!(1),
                    balance!(2),
                    balance!(0.25),
                    balance!(120),
                    balance!(220),
                    balance!(0.2),
                    balance!(0.25),
                    true,
                    balance!(0.75),
                    balance!(0.6),
                    31,
                    current_block + 5,
                    current_block + 2,
                    balance!(0.1),
                    current_block + 3,
                    balance!(20)
                ),
                Error::<Runtime>::InvalidEndBlock
            );
        });
    }

    #[test]
    fn create_ilo_invalid_price() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let current_block = frame_system::Pallet::<Runtime>::block_number();
            assert_err!(
                CeresLaunchpadPallet::<Runtime>::create_ilo(
                    Origin::signed(ALICE),
                    CERES_ASSET_ID.into(),
                    balance!(1),
                    balance!(2),
                    balance!(0.25),
                    balance!(120),
                    balance!(220),
                    balance!(0.2),
                    balance!(0.25),
                    true,
                    balance!(0.75),
                    balance!(0.1),
                    31,
                    current_block + 5,
                    current_block + 10,
                    balance!(0.1),
                    current_block + 3,
                    balance!(20)
                ),
                Error::<Runtime>::InvalidPrice
            );
        });
    }

    #[test]
    fn create_ilo_invalid_number_of_tokens_for_ilo() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let current_block = frame_system::Pallet::<Runtime>::block_number();
            assert_err!(
                CeresLaunchpadPallet::<Runtime>::create_ilo(
                    Origin::signed(ALICE),
                    CERES_ASSET_ID.into(),
                    balance!(7692.30769231),
                    balance!(2),
                    balance!(0.13),
                    balance!(600),
                    balance!(1000),
                    balance!(0.2),
                    balance!(0.25),
                    true,
                    balance!(0.75),
                    balance!(0.20),
                    31,
                    current_block + 5,
                    current_block + 10,
                    balance!(0.1),
                    current_block + 3,
                    balance!(20)
                ),
                Error::<Runtime>::InvalidNumberOfTokensForILO
            );
        });
    }

    #[test]
    fn create_ilo_invalid_number_of_tokens_for_liquidity() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let current_block = frame_system::Pallet::<Runtime>::block_number();
            assert_err!(
                CeresLaunchpadPallet::<Runtime>::create_ilo(
                    Origin::signed(ALICE),
                    CERES_ASSET_ID.into(),
                    balance!(7693),
                    balance!(1000),
                    balance!(0.13),
                    balance!(600),
                    balance!(1000),
                    balance!(0.2),
                    balance!(0.25),
                    true,
                    balance!(0.75),
                    balance!(0.20),
                    31,
                    current_block + 5,
                    current_block + 10,
                    balance!(0.1),
                    current_block + 3,
                    balance!(20)
                ),
                Error::<Runtime>::InvalidNumberOfTokensForLiquidity
            );
        });
    }

    #[test]
    fn create_ilo_invalid_first_release_percent() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let current_block = frame_system::Pallet::<Runtime>::block_number();
            assert_err!(
                CeresLaunchpadPallet::<Runtime>::create_ilo(
                    Origin::signed(ALICE),
                    CERES_ASSET_ID.into(),
                    balance!(7693),
                    balance!(3000),
                    balance!(0.13),
                    balance!(600),
                    balance!(1000),
                    balance!(0.2),
                    balance!(0.25),
                    true,
                    balance!(0.75),
                    balance!(0.25),
                    31,
                    current_block + 5,
                    current_block + 10,
                    balance!(0),
                    current_block + 3,
                    balance!(20)
                ),
                Error::<Runtime>::InvalidFirstReleasePercent
            );
        });
    }

    #[test]
    fn create_ilo_invalid_vesting_percent() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let current_block = frame_system::Pallet::<Runtime>::block_number();
            assert_err!(
                CeresLaunchpadPallet::<Runtime>::create_ilo(
                    Origin::signed(ALICE),
                    CERES_ASSET_ID.into(),
                    balance!(7693),
                    balance!(3000),
                    balance!(0.13),
                    balance!(600),
                    balance!(1000),
                    balance!(0.2),
                    balance!(0.25),
                    true,
                    balance!(0.75),
                    balance!(0.25),
                    31,
                    current_block + 5,
                    current_block + 10,
                    balance!(0.2),
                    current_block + 3,
                    balance!(0)
                ),
                Error::<Runtime>::InvalidVestingPercent
            );
        });
    }

    #[test]
    fn create_ilo_invalid_vesting_percent_overflow() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let current_block = frame_system::Pallet::<Runtime>::block_number();
            assert_err!(
                CeresLaunchpadPallet::<Runtime>::create_ilo(
                    Origin::signed(ALICE),
                    CERES_ASSET_ID.into(),
                    balance!(7693),
                    balance!(3000),
                    balance!(0.13),
                    balance!(600),
                    balance!(1000),
                    balance!(0.2),
                    balance!(0.25),
                    true,
                    balance!(0.75),
                    balance!(0.25),
                    31,
                    current_block + 5,
                    current_block + 10,
                    balance!(0.2),
                    current_block + 3,
                    balance!(0.9)
                ),
                Error::<Runtime>::InvalidVestingPercent
            );
        });
    }

    #[test]
    fn create_ilo_invalid_vesting_percent_remainder() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let current_block = frame_system::Pallet::<Runtime>::block_number();
            assert_err!(
                CeresLaunchpadPallet::<Runtime>::create_ilo(
                    Origin::signed(CHARLES),
                    CERES_ASSET_ID.into(),
                    balance!(7693),
                    balance!(3000),
                    balance!(0.13),
                    balance!(600),
                    balance!(1000),
                    balance!(0.2),
                    balance!(0.25),
                    true,
                    balance!(0.75),
                    balance!(0.25),
                    31,
                    current_block + 5,
                    current_block + 10,
                    balance!(0.2),
                    current_block + 3,
                    balance!(0.3)
                ),
                Error::<Runtime>::InvalidVestingPercent
            );
        });
    }

    #[test]
    fn create_ilo_invalid_vesting_period() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let current_block = frame_system::Pallet::<Runtime>::block_number();
            assert_err!(
                CeresLaunchpadPallet::<Runtime>::create_ilo(
                    Origin::signed(ALICE),
                    CERES_ASSET_ID.into(),
                    balance!(7693),
                    balance!(3000),
                    balance!(0.13),
                    balance!(600),
                    balance!(1000),
                    balance!(0.2),
                    balance!(0.25),
                    true,
                    balance!(0.75),
                    balance!(0.25),
                    31,
                    current_block + 5,
                    current_block + 10,
                    balance!(0.2),
                    0u32.into(),
                    balance!(0.2)
                ),
                Error::<Runtime>::InvalidVestingPeriod
            );
        });
    }

    #[test]
    fn create_ilo_not_enough_ceres() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let current_block = frame_system::Pallet::<Runtime>::block_number();
            assert_err!(
                CeresLaunchpadPallet::<Runtime>::create_ilo(
                    Origin::signed(BOB),
                    CERES_ASSET_ID.into(),
                    balance!(7693),
                    balance!(3000),
                    balance!(0.13),
                    balance!(600),
                    balance!(1000),
                    balance!(0.2),
                    balance!(0.25),
                    true,
                    balance!(0.75),
                    balance!(0.25),
                    31,
                    current_block + 5,
                    current_block + 10,
                    balance!(0.2),
                    current_block + 3,
                    balance!(0.2)
                ),
                Error::<Runtime>::NotEnoughCeres
            );
        });
    }

    #[test]
    fn create_ilo_not_enough_tokens() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let current_block = frame_system::Pallet::<Runtime>::block_number();
            assert_err!(
                CeresLaunchpadPallet::<Runtime>::create_ilo(
                    Origin::signed(CHARLES),
                    CERES_ASSET_ID.into(),
                    balance!(7693),
                    balance!(3000),
                    balance!(0.13),
                    balance!(600),
                    balance!(1000),
                    balance!(0.2),
                    balance!(0.25),
                    true,
                    balance!(0.75),
                    balance!(0.25),
                    31,
                    current_block + 5,
                    current_block + 10,
                    balance!(0.2),
                    current_block + 3,
                    balance!(0.2)
                ),
                Error::<Runtime>::NotEnoughTokens
            );
        });
    }

    #[test]
    fn create_ilo_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let current_block = frame_system::Pallet::<Runtime>::block_number();
            assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
                Origin::signed(ALICE),
                CERES_ASSET_ID.into(),
                balance!(7693),
                balance!(3000),
                balance!(0.13),
                balance!(600),
                balance!(1000),
                balance!(0.2),
                balance!(0.25),
                true,
                balance!(0.75),
                balance!(0.25),
                31,
                current_block + 5,
                current_block + 10,
                balance!(0.2),
                current_block + 3,
                balance!(0.2)
            ));

            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &ALICE)
                    .expect("Failed to query free balance."),
                balance!(4297)
            );

            let pallet_account = ModuleId(*b"crslaunc").into_account();
            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &pallet_account)
                    .expect("Failed to query free balance."),
                balance!(10693)
            );

            let ilo_info = pallet::ILOs::<Runtime>::get(&CERES_ASSET_ID);
            assert_ne!(ilo_info.ilo_price, balance!(0));
            assert_eq!(ilo_info.ilo_organizer, ALICE);
        });
    }

    #[test]
    fn create_ilo_ilo_already_exists() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let current_block = frame_system::Pallet::<Runtime>::block_number();
            assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
                Origin::signed(ALICE),
                CERES_ASSET_ID.into(),
                balance!(7693),
                balance!(3000),
                balance!(0.13),
                balance!(600),
                balance!(1000),
                balance!(0.2),
                balance!(0.25),
                true,
                balance!(0.75),
                balance!(0.25),
                31,
                current_block + 5,
                current_block + 10,
                balance!(0.2),
                current_block + 3,
                balance!(0.2)
            ));

            assert_err!(
                CeresLaunchpadPallet::<Runtime>::create_ilo(
                    Origin::signed(ALICE),
                    CERES_ASSET_ID.into(),
                    balance!(7693),
                    balance!(3000),
                    balance!(0.13),
                    balance!(600),
                    balance!(1000),
                    balance!(0.2),
                    balance!(0.25),
                    true,
                    balance!(0.75),
                    balance!(0.25),
                    31,
                    current_block + 5,
                    current_block + 10,
                    balance!(0.2),
                    current_block + 3,
                    balance!(0.2)
                ),
                Error::<Runtime>::ILOAlreadyExists
            );
        });
    }
}
