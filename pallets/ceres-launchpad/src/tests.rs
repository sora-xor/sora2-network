mod tests {
    use crate::mock::*;
    use crate::{pallet, Error, Pallet as CeresLaunchpadPallet};
    use common::fixnum::ops::CheckedAdd;
    use common::prelude::FixedWrapper;
    use common::{balance, AssetName, AssetSymbol, Balance, DEFAULT_BALANCE_PRECISION, PSWAP, XOR};
    use frame_support::{assert_err, assert_ok};
    use pswap_distribution::{ClaimableShares, ShareholderAccounts};
    use sp_runtime::traits::AccountIdConversion;
    use sp_runtime::ModuleId;

    fn preset_initial<Fun>(tests: Fun)
    where
        Fun: Fn(),
    {
        let mut ext = ExtBuilder::default().build();
        let xor: AssetId = XOR.into();
        let ceres: AssetId = CERES_ASSET_ID.into();

        ext.execute_with(|| {
            assert_ok!(assets::Module::<Runtime>::register_asset_id(
                ALICE,
                XOR.into(),
                AssetSymbol(b"XOR".to_vec()),
                AssetName(b"SORA".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                Balance::from(0u32),
                true,
                None,
                None,
            ));

            assert_ok!(assets::Module::<Runtime>::register_asset_id(
                ALICE,
                CERES_ASSET_ID.into(),
                AssetSymbol(b"CERES".to_vec()),
                AssetName(b"Ceres".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                Balance::from(0u32),
                true,
                None,
                None,
            ));

            assert_ok!(assets::Module::<Runtime>::register_asset_id(
                ALICE,
                GetIncentiveAssetId::get().into(),
                AssetSymbol(b"XOR".to_vec()),
                AssetName(b"SORA".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                Balance::from(0u32),
                true,
                None,
                None,
            ));

            assert_ok!(assets::Module::<Runtime>::mint_to(
                &xor,
                &ALICE,
                &ALICE,
                balance!(900000)
            ));

            assert_ok!(assets::Module::<Runtime>::mint_to(
                &ceres,
                &ALICE,
                &ALICE,
                balance!(1000)
            ));

            assert_ok!(assets::Module::<Runtime>::mint_to(
                &xor,
                &ALICE,
                &CHARLES,
                balance!(2000)
            ));

            assert_ok!(assets::Module::<Runtime>::mint_to(
                &ceres,
                &ALICE,
                &CHARLES,
                balance!(2000)
            ));

            assert_ok!(assets::Module::<Runtime>::mint_to(
                &PSWAP,
                &ALICE,
                &GetPswapDistributionAccountId::get(),
                balance!(900000)
            ));

            assert_eq!(
                assets::Module::<Runtime>::free_balance(&xor, &ALICE).unwrap(),
                balance!(900000)
            );
            assert_eq!(
                assets::Module::<Runtime>::free_balance(&ceres, &ALICE).unwrap(),
                balance!(16000)
            );

            assert_eq!(
                assets::Module::<Runtime>::free_balance(&xor, &CHARLES).unwrap(),
                balance!(2000)
            );
            assert_eq!(
                assets::Module::<Runtime>::free_balance(&ceres, &CHARLES).unwrap(),
                balance!(5000)
            );

            tests();
        });
    }

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
        preset_initial(|| {
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
        preset_initial(|| {
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
        preset_initial(|| {
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
                balance!(1999.79)
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
        preset_initial(|| {
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
                balance!(1999.958)
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
    fn finish_ilo_ilo_does_not_exist() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_err!(
                CeresLaunchpadPallet::<Runtime>::finish_ilo(
                    Origin::signed(ALICE),
                    CERES_ASSET_ID.into()
                ),
                Error::<Runtime>::ILODoesNotExist
            );
        });
    }

    #[test]
    fn finish_ilo_ilo_is_not_finished() {
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
                CeresLaunchpadPallet::<Runtime>::finish_ilo(
                    Origin::signed(ALICE),
                    CERES_ASSET_ID.into()
                ),
                Error::<Runtime>::ILOIsNotFinished
            );
        });
    }

    #[test]
    fn finish_ilo_unauthorized() {
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
                CeresLaunchpadPallet::<Runtime>::finish_ilo(
                    Origin::signed(CHARLES),
                    CERES_ASSET_ID.into()
                ),
                Error::<Runtime>::Unauthorized
            );
        });
    }

    #[test]
    fn finish_ilo_ilo_failed_refunded_ok() {
        preset_initial(|| {
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

            assert_ok!(CeresLaunchpadPallet::<Runtime>::finish_ilo(
                Origin::signed(ALICE),
                CERES_ASSET_ID.into()
            ),);

            let pallet_account = ModuleId(*b"crslaunc").into_account();
            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &pallet_account)
                    .expect("Failed to query free balance."),
                balance!(0)
            );

            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &ALICE)
                    .expect("Failed to query free balance."),
                balance!(15990)
            );

            let ilo_info = pallet::ILOs::<Runtime>::get(&CERES_ASSET_ID);
            assert_eq!(ilo_info.failed, true);
        });
    }

    #[test]
    fn finish_ilo_ilo_failed_burned_ok() {
        preset_initial(|| {
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
                false,
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

            assert_ok!(CeresLaunchpadPallet::<Runtime>::finish_ilo(
                Origin::signed(ALICE),
                CERES_ASSET_ID.into()
            ),);

            let pallet_account = ModuleId(*b"crslaunc").into_account();
            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &pallet_account)
                    .expect("Failed to query free balance."),
                balance!(0)
            );

            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &ALICE)
                    .expect("Failed to query free balance."),
                balance!(5297)
            );

            let ilo_info = pallet::ILOs::<Runtime>::get(&CERES_ASSET_ID);
            assert_eq!(ilo_info.failed, true);
        });
    }

    #[test]
    fn finish_ilo_ilo_is_failed() {
        preset_initial(|| {
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

            assert_ok!(CeresLaunchpadPallet::<Runtime>::finish_ilo(
                Origin::signed(ALICE),
                CERES_ASSET_ID.into()
            ),);

            assert_err!(
                CeresLaunchpadPallet::<Runtime>::finish_ilo(
                    Origin::signed(ALICE),
                    CERES_ASSET_ID.into()
                ),
                Error::<Runtime>::ILOIsFailed
            );
        });
    }

    #[test]
    fn finish_ilo_not_filled_hard_cap_ok() {
        preset_initial(|| {
            let mut current_block = frame_system::Pallet::<Runtime>::block_number();
            assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
                Origin::signed(ALICE),
                CERES_ASSET_ID.into(),
                balance!(7693),
                balance!(3000),
                balance!(0.13),
                balance!(600),
                balance!(1000),
                balance!(0.2),
                balance!(1500),
                false,
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

            let funds_to_contribute = balance!(800);

            assert_ok!(CeresLaunchpadPallet::<Runtime>::contribute(
                Origin::signed(CHARLES),
                CERES_ASSET_ID.into(),
                funds_to_contribute
            ));

            run_to_block(11);

            current_block = frame_system::Pallet::<Runtime>::block_number();
            assert_ok!(CeresLaunchpadPallet::<Runtime>::finish_ilo(
                Origin::signed(ALICE),
                CERES_ASSET_ID.into()
            ),);

            let ilo_info = pallet::ILOs::<Runtime>::get(&CERES_ASSET_ID);

            let funds_for_liquidity = (FixedWrapper::from(ilo_info.funds_raised)
                * FixedWrapper::from(ilo_info.liquidity_percent))
            .try_into_balance()
            .unwrap_or(0);
            let funds_for_team = ilo_info.funds_raised - funds_for_liquidity;
            assert_eq!(
                Assets::free_balance(&XOR, &ALICE).expect("Failed to query free balance."),
                funds_for_team + balance!(900000)
            );

            let (xor_liq, ceres_liq) = pool_xyk::Reserves::<Runtime>::get(XOR, CERES_ASSET_ID);
            assert_eq!(xor_liq, funds_for_liquidity);

            let tokens_for_liquidity = (FixedWrapper::from(funds_for_liquidity)
                / FixedWrapper::from(ilo_info.listing_price))
            .try_into_balance()
            .unwrap_or(0);
            assert_eq!(ceres_liq, tokens_for_liquidity);

            let pallet_account = ModuleId(*b"crslaunc").into_account();
            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &pallet_account)
                    .expect("Failed to query free balance."),
                ilo_info.sold_tokens
            );

            assert_err!(
                pool_xyk::Pallet::<Runtime>::withdraw_liquidity(
                    Origin::signed(pallet_account),
                    DEX_A_ID,
                    XOR.into(),
                    CERES_ASSET_ID.into(),
                    ilo_info.lp_tokens,
                    balance!(0),
                    balance!(0)
                ),
                pool_xyk::Error::<Runtime>::NotEnoughUnlockedLiquidity
            );

            assert_eq!(ilo_info.finish_block, current_block);
        });
    }

    #[test]
    fn finish_ilo_filled_hard_cap_ok() {
        preset_initial(|| {
            let mut current_block = frame_system::Pallet::<Runtime>::block_number();
            assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
                Origin::signed(ALICE),
                CERES_ASSET_ID.into(),
                balance!(7693),
                balance!(3000),
                balance!(0.13),
                balance!(600),
                balance!(1000),
                balance!(0.2),
                balance!(1500),
                false,
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

            let funds_to_contribute = balance!(1000);

            assert_ok!(CeresLaunchpadPallet::<Runtime>::contribute(
                Origin::signed(CHARLES),
                CERES_ASSET_ID.into(),
                funds_to_contribute
            ));

            run_to_block(11);

            current_block = frame_system::Pallet::<Runtime>::block_number();
            assert_ok!(CeresLaunchpadPallet::<Runtime>::finish_ilo(
                Origin::signed(ALICE),
                CERES_ASSET_ID.into()
            ),);

            let ilo_info = pallet::ILOs::<Runtime>::get(&CERES_ASSET_ID);

            let funds_for_liquidity = (FixedWrapper::from(ilo_info.funds_raised)
                * FixedWrapper::from(ilo_info.liquidity_percent))
            .try_into_balance()
            .unwrap_or(0);
            let funds_for_team = ilo_info.funds_raised - funds_for_liquidity;
            assert_eq!(
                Assets::free_balance(&XOR, &ALICE).expect("Failed to query free balance."),
                funds_for_team + balance!(900000)
            );

            let (xor_liq, ceres_liq) = pool_xyk::Reserves::<Runtime>::get(XOR, CERES_ASSET_ID);
            assert_eq!(xor_liq, funds_for_liquidity);

            let tokens_for_liquidity = (FixedWrapper::from(funds_for_liquidity)
                / FixedWrapper::from(ilo_info.listing_price))
            .try_into_balance()
            .unwrap_or(0);
            assert_eq!(ceres_liq, tokens_for_liquidity);

            let pallet_account = ModuleId(*b"crslaunc").into_account();
            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &pallet_account)
                    .expect("Failed to query free balance."),
                ilo_info.sold_tokens
            );

            assert_err!(
                pool_xyk::Pallet::<Runtime>::withdraw_liquidity(
                    Origin::signed(pallet_account),
                    DEX_A_ID,
                    XOR.into(),
                    CERES_ASSET_ID.into(),
                    ilo_info.lp_tokens,
                    balance!(0),
                    balance!(0)
                ),
                pool_xyk::Error::<Runtime>::NotEnoughUnlockedLiquidity
            );

            assert_eq!(ilo_info.finish_block, current_block);
        });
    }

    #[test]
    fn claim_ilo_does_not_exist() {
        preset_initial(|| {
            assert_err!(
                CeresLaunchpadPallet::<Runtime>::claim(
                    Origin::signed(CHARLES),
                    CERES_ASSET_ID.into(),
                ),
                Error::<Runtime>::ILODoesNotExist
            );
        });
    }

    #[test]
    fn claim_ilo_is_not_finished() {
        preset_initial(|| {
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
                CeresLaunchpadPallet::<Runtime>::claim(
                    Origin::signed(CHARLES),
                    CERES_ASSET_ID.into(),
                ),
                Error::<Runtime>::ILOIsNotFinished
            );
        });
    }

    #[test]
    fn claim_ilo_failed_ok() {
        preset_initial(|| {
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

            run_to_block(11);

            assert_ok!(CeresLaunchpadPallet::<Runtime>::finish_ilo(
                Origin::signed(ALICE),
                CERES_ASSET_ID.into()
            ),);

            assert_ok!(CeresLaunchpadPallet::<Runtime>::claim(
                Origin::signed(CHARLES),
                CERES_ASSET_ID.into(),
            ));

            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &CHARLES)
                    .expect("Failed to query free balance."),
                balance!(5000)
            );

            let contribution_info =
                pallet::Contributions::<Runtime>::get(&CERES_ASSET_ID, &CHARLES);
            assert_eq!(contribution_info.claiming_finished, true);
        });
    }

    #[test]
    fn claim_funds_already_claimed() {
        preset_initial(|| {
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

            run_to_block(11);

            assert_ok!(CeresLaunchpadPallet::<Runtime>::finish_ilo(
                Origin::signed(ALICE),
                CERES_ASSET_ID.into()
            ),);

            assert_ok!(CeresLaunchpadPallet::<Runtime>::claim(
                Origin::signed(CHARLES),
                CERES_ASSET_ID.into(),
            ));

            assert_err!(
                CeresLaunchpadPallet::<Runtime>::claim(
                    Origin::signed(CHARLES),
                    CERES_ASSET_ID.into(),
                ),
                Error::<Runtime>::FundsAlreadyClaimed
            );
        });
    }

    #[test]
    fn claim_first_release_claim_ok() {
        preset_initial(|| {
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
                balance!(1500),
                false,
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

            let funds_to_contribute = balance!(1000);

            assert_ok!(CeresLaunchpadPallet::<Runtime>::contribute(
                Origin::signed(CHARLES),
                CERES_ASSET_ID.into(),
                funds_to_contribute
            ));

            run_to_block(11);

            assert_ok!(CeresLaunchpadPallet::<Runtime>::finish_ilo(
                Origin::signed(ALICE),
                CERES_ASSET_ID.into()
            ),);

            assert_ok!(CeresLaunchpadPallet::<Runtime>::claim(
                Origin::signed(CHARLES),
                CERES_ASSET_ID.into(),
            ));

            let ilo_info = pallet::ILOs::<Runtime>::get(&CERES_ASSET_ID);
            let contribution_info =
                pallet::Contributions::<Runtime>::get(&CERES_ASSET_ID, &CHARLES);

            let tokens_to_claim = (FixedWrapper::from(contribution_info.tokens_bought)
                * FixedWrapper::from(ilo_info.token_vesting.first_release_percent))
            .try_into_balance()
            .unwrap_or(0);

            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &CHARLES)
                    .expect("Failed to query free balance."),
                balance!(5000) + tokens_to_claim
            );

            assert_eq!(contribution_info.tokens_claimed, tokens_to_claim);
        });
    }

    #[test]
    fn claim_no_potential_claims() {
        preset_initial(|| {
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
                balance!(1500),
                false,
                balance!(0.75),
                balance!(0.25),
                31,
                current_block + 5,
                current_block + 10,
                balance!(0.2),
                current_block + 50,
                balance!(0.2)
            ));

            run_to_block(6);

            let funds_to_contribute = balance!(1000);

            assert_ok!(CeresLaunchpadPallet::<Runtime>::contribute(
                Origin::signed(CHARLES),
                CERES_ASSET_ID.into(),
                funds_to_contribute
            ));

            run_to_block(11);

            assert_ok!(CeresLaunchpadPallet::<Runtime>::finish_ilo(
                Origin::signed(ALICE),
                CERES_ASSET_ID.into()
            ),);

            assert_ok!(CeresLaunchpadPallet::<Runtime>::claim(
                Origin::signed(CHARLES),
                CERES_ASSET_ID.into(),
            ));

            run_to_block(12);

            assert_err!(
                CeresLaunchpadPallet::<Runtime>::claim(
                    Origin::signed(CHARLES),
                    CERES_ASSET_ID.into(),
                ),
                Error::<Runtime>::NothingToClaim
            );
        });
    }

    #[test]
    fn claim_ok() {
        preset_initial(|| {
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
                balance!(1500),
                false,
                balance!(0.75),
                balance!(0.25),
                31,
                current_block + 5,
                current_block + 10,
                balance!(0.1),
                30u32.into(),
                balance!(0.18)
            ));

            run_to_block(6);

            let funds_to_contribute = balance!(1000);

            assert_ok!(CeresLaunchpadPallet::<Runtime>::contribute(
                Origin::signed(CHARLES),
                CERES_ASSET_ID.into(),
                funds_to_contribute
            ));

            run_to_block(11);

            assert_ok!(CeresLaunchpadPallet::<Runtime>::finish_ilo(
                Origin::signed(ALICE),
                CERES_ASSET_ID.into()
            ),);

            let ilo_info = pallet::ILOs::<Runtime>::get(&CERES_ASSET_ID);
            let mut contribution_info =
                pallet::Contributions::<Runtime>::get(&CERES_ASSET_ID, &CHARLES);

            let first_release = (FixedWrapper::from(contribution_info.tokens_bought)
                * FixedWrapper::from(ilo_info.token_vesting.first_release_percent))
            .try_into_balance()
            .unwrap_or(0);

            let tokens_per_claim = (FixedWrapper::from(contribution_info.tokens_bought)
                * FixedWrapper::from(ilo_info.token_vesting.vesting_percent))
            .try_into_balance()
            .unwrap_or(0);

            assert_ok!(CeresLaunchpadPallet::<Runtime>::claim(
                Origin::signed(CHARLES),
                CERES_ASSET_ID.into(),
            ));

            run_to_block(43);

            assert_ok!(CeresLaunchpadPallet::<Runtime>::claim(
                Origin::signed(CHARLES),
                CERES_ASSET_ID.into(),
            ));

            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &CHARLES)
                    .expect("Failed to query free balance."),
                balance!(5000) + first_release + tokens_per_claim
            );

            run_to_block(103);

            assert_ok!(CeresLaunchpadPallet::<Runtime>::claim(
                Origin::signed(CHARLES),
                CERES_ASSET_ID.into(),
            ));
            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &CHARLES)
                    .expect("Failed to query free balance."),
                balance!(5000) + first_release + tokens_per_claim * 3
            );

            run_to_block(163);

            assert_ok!(CeresLaunchpadPallet::<Runtime>::claim(
                Origin::signed(CHARLES),
                CERES_ASSET_ID.into(),
            ));
            contribution_info = pallet::Contributions::<Runtime>::get(&CERES_ASSET_ID, &CHARLES);
            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &CHARLES)
                    .expect("Failed to query free balance."),
                balance!(5000) + first_release + tokens_per_claim * 5
            );
            assert_eq!(contribution_info.claiming_finished, true);
        });
    }

    #[test]
    fn change_ceres_burn_fee_unauthorized() {
        preset_initial(|| {
            assert_err!(
                CeresLaunchpadPallet::<Runtime>::change_ceres_burn_fee(
                    Origin::signed(ALICE),
                    balance!(100)
                ),
                Error::<Runtime>::Unauthorized
            );
        });
    }

    #[test]
    fn change_ceres_burn_fee_ok() {
        preset_initial(|| {
            assert_ok!(CeresLaunchpadPallet::<Runtime>::change_ceres_burn_fee(
                Origin::signed(pallet::AuthorityAccount::<Runtime>::get()),
                balance!(100)
            ));

            assert_eq!(pallet::CeresBurnFeeAmount::<Runtime>::get(), balance!(100));
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

    /*#[test]
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
    }*/

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

    /*#[test]
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
    }*/

    #[test]
    fn claim_pswap_rewards_unauthorized() {
        preset_initial(|| {
            assert_err!(
                CeresLaunchpadPallet::<Runtime>::claim_pswap_rewards(Origin::signed(ALICE)),
                Error::<Runtime>::Unauthorized
            );
        });
    }

    #[test]
    fn claim_pswap_rewards_ok() {
        preset_initial(|| {
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
                balance!(1500),
                false,
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

            let funds_to_contribute = balance!(1000);

            assert_ok!(CeresLaunchpadPallet::<Runtime>::contribute(
                Origin::signed(CHARLES),
                CERES_ASSET_ID.into(),
                funds_to_contribute
            ));

            run_to_block(11);

            assert_ok!(CeresLaunchpadPallet::<Runtime>::finish_ilo(
                Origin::signed(ALICE),
                CERES_ASSET_ID.into()
            ));

            run_to_block(20000);

            let pallet_account = ModuleId(*b"crslaunc").into_account();
            let share = FixedWrapper::from(1.00).get().unwrap();
            ShareholderAccounts::<Runtime>::mutate(&pallet_account, |current| {
                *current = current.saturating_add(share)
            });
            ClaimableShares::<Runtime>::mutate(|current| *current = current.saturating_add(share));

            assert_ok!(CeresLaunchpadPallet::<Runtime>::claim_pswap_rewards(
                Origin::signed(pallet::AuthorityAccount::<Runtime>::get())
            ));

            assert_eq!(
                Assets::free_balance(&PSWAP, &pallet_account)
                    .expect("Failed to query free balance."),
                balance!(0)
            );

            assert_eq!(
                Assets::free_balance(&PSWAP, &pallet::AuthorityAccount::<Runtime>::get())
                    .expect("Failed to query free balance."),
                balance!(share)
            );
        });
    }

    #[test]
    fn on_initialize_fail_ilo() {
        preset_initial(|| {
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
                balance!(1500),
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

            let funds_to_contribute = balance!(1000);

            assert_ok!(CeresLaunchpadPallet::<Runtime>::contribute(
                Origin::signed(CHARLES),
                CERES_ASSET_ID.into(),
                funds_to_contribute
            ));

            run_to_block(300000);

            let ilo_info = pallet::ILOs::<Runtime>::get(&CERES_ASSET_ID);
            let pallet_account = ModuleId(*b"crslaunc").into_account();
            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &pallet_account)
                    .expect("Failed to query free balance."),
                balance!(0)
            );

            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &ALICE)
                    .expect("Failed to query free balance."),
                balance!(15990)
            );

            assert_eq!(ilo_info.failed, true);
        });
    }
}
