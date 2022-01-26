mod tests {
    use crate::mock::*;
    use crate::{pallet, Error, Pallet as CeresLaunchpadPallet};
    use common::balance;
    use common::prelude::FixedWrapper;
    use common::PredefinedAssetId::XOR;
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

    #[test]
    fn contribute_ilo_does_not_exist() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_err!(
                CeresLaunchpadPallet::<Runtime>::contribute(
                    Origin::signed(ALICE),
                    CERES_ASSET_ID.into(),
                    balance!(0.21)
                ),
                Error::<Runtime>::ILODoesNotExist
            );
        });
    }

    #[test]
    fn contribute_ilo_not_started() {
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
                CeresLaunchpadPallet::<Runtime>::contribute(
                    Origin::signed(ALICE),
                    CERES_ASSET_ID.into(),
                    balance!(0.21)
                ),
                Error::<Runtime>::ILONotStarted
            );
        });
    }

    #[test]
    fn contribute_ilo_is_finished() {
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

            run_to_block(11);

            assert_err!(
                CeresLaunchpadPallet::<Runtime>::contribute(
                    Origin::signed(ALICE),
                    CERES_ASSET_ID.into(),
                    balance!(0.21)
                ),
                Error::<Runtime>::ILOIsFinished
            );
        });
    }

    #[test]
    fn contribute_contribution_is_lower_then_min() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let asset_id = CERES_ASSET_ID;
            let current_block = frame_system::Pallet::<Runtime>::block_number();
            assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
                Origin::signed(ALICE),
                asset_id.into(),
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

            run_to_block(6);

            assert_err!(
                CeresLaunchpadPallet::<Runtime>::contribute(
                    Origin::signed(ALICE),
                    asset_id.into(),
                    balance!(0.1)
                ),
                Error::<Runtime>::ContributionIsLowerThenMin
            );
        });
    }

    #[test]
    fn contribute_contribution_is_bigger_then_max() {
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

            run_to_block(6);

            assert_ok!(CeresLaunchpadPallet::<Runtime>::contribute(
                Origin::signed(ALICE),
                CERES_ASSET_ID.into(),
                balance!(0.2)
            ));
            assert_err!(
                CeresLaunchpadPallet::<Runtime>::contribute(
                    Origin::signed(ALICE),
                    CERES_ASSET_ID.into(),
                    balance!(0.2)
                ),
                Error::<Runtime>::ContributionIsBiggerThenMax
            );
        });
    }

    #[test]
    fn contribute_hard_cap_is_hit() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let current_block = frame_system::Pallet::<Runtime>::block_number();
            let asset_id = CERES_ASSET_ID;
            assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
                Origin::signed(ALICE),
                asset_id.into(),
                balance!(7693),
                balance!(3000),
                balance!(0.13),
                balance!(600),
                balance!(1000),
                balance!(0.2),
                balance!(2000),
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

            run_to_block(6);

            assert_ok!(CeresLaunchpadPallet::<Runtime>::contribute(
                Origin::signed(ALICE),
                CERES_ASSET_ID.into(),
                balance!(100)
            ));

            assert_err!(
                CeresLaunchpadPallet::<Runtime>::contribute(
                    Origin::signed(BOB),
                    CERES_ASSET_ID.into(),
                    balance!(901)
                ),
                Error::<Runtime>::HardCapIsHit
            );
        });
    }

    #[test]
    fn contribute_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let current_block = frame_system::Pallet::<Runtime>::block_number();
            let asset_id = CERES_ASSET_ID;
            assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
                Origin::signed(ALICE),
                asset_id.into(),
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

            run_to_block(6);
            let funds_to_contribute = balance!(0.21);

            assert_ok!(CeresLaunchpadPallet::<Runtime>::contribute(
                Origin::signed(CHARLES),
                CERES_ASSET_ID.into(),
                funds_to_contribute
            ));

            let ilo_info = pallet::ILOs::<Runtime>::get(&CERES_ASSET_ID);
            assert_eq!(ilo_info.funds_raised, funds_to_contribute);
            let tokens_bought = (FixedWrapper::from(funds_to_contribute)
                / FixedWrapper::from(ilo_info.ilo_price))
            .try_into_balance()
            .unwrap_or(0);

            let contribution_info =
                pallet::Contributions::<Runtime>::get(&CERES_ASSET_ID, &CHARLES);

            assert_eq!(ilo_info.sold_tokens, tokens_bought);
            assert_eq!(contribution_info.funds_contributed, funds_to_contribute);
            assert_eq!(contribution_info.tokens_bought, tokens_bought);

            assert_eq!(
                Assets::free_balance(&XOR.into(), &CHARLES).expect("Failed to query free balance."),
                balance!(2999.79)
            );
        });
    }

    #[test]
    fn emergency_withdraw_ilo_does_not_exist() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_err!(
                CeresLaunchpadPallet::<Runtime>::emergency_withdraw(
                    Origin::signed(ALICE),
                    CERES_ASSET_ID.into()
                ),
                Error::<Runtime>::ILODoesNotExist
            );
        });
    }

    #[test]
    fn emergency_withdraw_ilo_not_started() {
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
                CeresLaunchpadPallet::<Runtime>::emergency_withdraw(
                    Origin::signed(ALICE),
                    CERES_ASSET_ID.into()
                ),
                Error::<Runtime>::ILONotStarted
            );
        });
    }

    #[test]
    fn emergency_withdraw_ilo_is_finished() {
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

            run_to_block(11);

            assert_err!(
                CeresLaunchpadPallet::<Runtime>::emergency_withdraw(
                    Origin::signed(ALICE),
                    CERES_ASSET_ID.into()
                ),
                Error::<Runtime>::ILOIsFinished
            );
        });
    }

    #[test]
    fn emergency_withdraw_not_enough_funds() {
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

            run_to_block(6);

            assert_err!(
                CeresLaunchpadPallet::<Runtime>::emergency_withdraw(
                    Origin::signed(ALICE),
                    CERES_ASSET_ID.into()
                ),
                Error::<Runtime>::NotEnoughFunds
            );
        });
    }

    #[test]
    fn emergency_withdraw_ok() {
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

            run_to_block(6);

            let funds_to_contribute = balance!(0.21);

            assert_ok!(CeresLaunchpadPallet::<Runtime>::contribute(
                Origin::signed(CHARLES),
                CERES_ASSET_ID.into(),
                funds_to_contribute
            ));

            let mut contribution_info =
                pallet::Contributions::<Runtime>::get(&CERES_ASSET_ID, &CHARLES);

            assert_ok!(CeresLaunchpadPallet::<Runtime>::emergency_withdraw(
                Origin::signed(CHARLES),
                CERES_ASSET_ID.into(),
            ));

            let funds_to_claim = (FixedWrapper::from(contribution_info.funds_contributed)
                * FixedWrapper::from(0.8))
            .try_into_balance()
            .unwrap_or(0);

            let penalty = contribution_info.funds_contributed - funds_to_claim;
            let ilo_info = pallet::ILOs::<Runtime>::get(&CERES_ASSET_ID);

            assert_eq!(
                Assets::free_balance(&XOR.into(), &CHARLES).expect("Failed to query free balance."),
                balance!(2999.958)
            );

            assert_eq!(
                Assets::free_balance(&XOR.into(), &pallet::PenaltiesAccount::<Runtime>::get())
                    .expect("Failed to query free balance."),
                penalty
            );

            assert_eq!(ilo_info.funds_raised, balance!(0));
            assert_eq!(ilo_info.sold_tokens, balance!(0));

            contribution_info = pallet::Contributions::<Runtime>::get(&CERES_ASSET_ID, &CHARLES);

            assert_eq!(contribution_info.funds_contributed, balance!(0));
        });
    }

    #[test]
    fn claim_lp_tokens_ilo_does_not_exist() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_err!(
                CeresLaunchpadPallet::<Runtime>::claim_lp_tokens(
                    Origin::signed(ALICE),
                    CERES_ASSET_ID.into()
                ),
                Error::<Runtime>::ILODoesNotExist
            );
        });
    }

    /*
    #[test]
    fn claim_lp_tokens_cant_claim_lp_tokens_already_claimed() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {


            assert_err!(
                CeresLaunchpadPallet::<Runtime>::emergency_withdraw(
                    Origin::signed(ALICE),
                    CERES_ASSET_ID.into()
                ),
                Error::<Runtime>::ILODoesNotExist
            );
        });
    }
    */

    #[test]
    fn claim_lp_tokens_cant_claim_lp_tokens() {
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
                CeresLaunchpadPallet::<Runtime>::claim_lp_tokens(
                    Origin::signed(ALICE),
                    CERES_ASSET_ID.into()
                ),
                Error::<Runtime>::CantClaimLPTokens
            );
        });
    }

    #[test]
    fn claim_lp_tokens_unauthorized() {
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
                CeresLaunchpadPallet::<Runtime>::claim_lp_tokens(
                    Origin::signed(CHARLES),
                    CERES_ASSET_ID.into()
                ),
                Error::<Runtime>::CantClaimLPTokens
            );
        });
    }
}
