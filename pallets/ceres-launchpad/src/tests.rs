use crate::mock::*;
use crate::{pallet, Error, FeePercentOnRaisedFunds, Pallet as CeresLaunchpadPallet};
use common::fixnum::ops::CheckedAdd;
use common::prelude::FixedWrapper;
use common::{
    balance, AssetInfoProvider, AssetName, AssetSymbol, Balance, PoolXykPallet, CERES_ASSET_ID,
    DEFAULT_BALANCE_PRECISION, PSWAP, XOR, XSTUSD,
};
use frame_support::{assert_err, assert_ok, PalletId};
use pswap_distribution::{ClaimableShares, ShareholderAccounts};
use sp_runtime::traits::AccountIdConversion;

fn preset_initial<Fun>(tests: Fun)
where
    Fun: Fn(),
{
    let mut ext = ExtBuilder::default().build();
    let xor: AssetId = XOR;
    let xstusd: AssetId = XSTUSD;
    let ceres: AssetId = CERES_ASSET_ID;

    ext.execute_with(|| {
        assert_ok!(assets::Pallet::<Runtime>::register_asset_id(
            ALICE,
            XOR,
            AssetSymbol(b"XOR".to_vec()),
            AssetName(b"SORA".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::from(0u32),
            true,
            None,
            None,
        ));

        assert_ok!(assets::Pallet::<Runtime>::register_asset_id(
            ALICE,
            XSTUSD,
            AssetSymbol(b"XSTUSD".to_vec()),
            AssetName(b"SORA Synthetic USD".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::from(0u32),
            true,
            None,
            None,
        ));

        assert_ok!(assets::Pallet::<Runtime>::register_asset_id(
            ALICE,
            CERES_ASSET_ID,
            AssetSymbol(b"CERES".to_vec()),
            AssetName(b"Ceres".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::from(0u32),
            true,
            None,
            None,
        ));

        assert_ok!(assets::Pallet::<Runtime>::register_asset_id(
            ALICE,
            GetIncentiveAssetId::get(),
            AssetSymbol(b"XOR".to_vec()),
            AssetName(b"SORA".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::from(0u32),
            true,
            None,
            None,
        ));

        assert_ok!(assets::Pallet::<Runtime>::mint_to(
            &xor,
            &ALICE,
            &ALICE,
            balance!(900000)
        ));

        assert_ok!(assets::Pallet::<Runtime>::mint_to(
            &ceres,
            &ALICE,
            &ALICE,
            balance!(1000)
        ));

        assert_ok!(assets::Pallet::<Runtime>::mint_to(
            &xor,
            &ALICE,
            &CHARLES,
            balance!(2000)
        ));

        assert_ok!(assets::Pallet::<Runtime>::mint_to(
            &xstusd,
            &ALICE,
            &CHARLES,
            balance!(1000)
        ));

        assert_ok!(assets::Pallet::<Runtime>::mint_to(
            &ceres,
            &ALICE,
            &CHARLES,
            balance!(2000)
        ));

        assert_ok!(assets::Pallet::<Runtime>::mint_to(
            &ceres,
            &ALICE,
            &DAN,
            balance!(11000)
        ));

        assert_ok!(assets::Pallet::<Runtime>::mint_to(
            &PSWAP,
            &ALICE,
            &GetPswapDistributionAccountId::get(),
            balance!(900000)
        ));

        assert_eq!(
            assets::Pallet::<Runtime>::free_balance(&xor, &ALICE).unwrap(),
            balance!(900000)
        );
        assert_eq!(
            assets::Pallet::<Runtime>::free_balance(&ceres, &ALICE).unwrap(),
            balance!(16000)
        );

        assert_eq!(
            assets::Pallet::<Runtime>::free_balance(&xor, &CHARLES).unwrap(),
            balance!(2000)
        );
        assert_eq!(
            assets::Pallet::<Runtime>::free_balance(&xstusd, &CHARLES).unwrap(),
            balance!(1000)
        );
        assert_eq!(
            assets::Pallet::<Runtime>::free_balance(&ceres, &CHARLES).unwrap(),
            balance!(5000)
        );

        pallet::WhitelistedIloOrganizers::<Runtime>::append(ALICE);
        pallet::WhitelistedIloOrganizers::<Runtime>::append(BOB);
        pallet::WhitelistedIloOrganizers::<Runtime>::append(CHARLES);
        pallet::WhitelistedIloOrganizers::<Runtime>::append(DAN);

        pallet::WhitelistedContributors::<Runtime>::append(ALICE);
        pallet::WhitelistedContributors::<Runtime>::append(BOB);
        pallet::WhitelistedContributors::<Runtime>::append(CHARLES);
        pallet::WhitelistedContributors::<Runtime>::append(DAN);

        tests();
    });
}

#[test]
fn create_ilo_ilo_price_zero() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_err!(
            CeresLaunchpadPallet::<Runtime>::create_ilo(
                RuntimeOrigin::signed(ALICE),
                base_asset,
                CERES_ASSET_ID,
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
                current_timestamp + 5,
                current_timestamp + 10,
                balance!(1000),
                balance!(0.2),
                current_timestamp + 3,
                balance!(0.2),
                balance!(0.1),
                current_timestamp + 3,
                balance!(20)
            ),
            Error::<Runtime>::ParameterCantBeZero
        );
    });
}

#[test]
fn create_ilo_hard_cap_zero() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_err!(
            CeresLaunchpadPallet::<Runtime>::create_ilo(
                RuntimeOrigin::signed(ALICE),
                base_asset,
                CERES_ASSET_ID,
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
                current_timestamp + 5,
                current_timestamp + 10,
                balance!(1000),
                balance!(0.2),
                current_timestamp + 3,
                balance!(0.2),
                balance!(0.1),
                current_timestamp + 3,
                balance!(20)
            ),
            Error::<Runtime>::ParameterCantBeZero
        );
    });
}

#[test]
fn create_ilo_invalid_soft_cap() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_err!(
            CeresLaunchpadPallet::<Runtime>::create_ilo(
                RuntimeOrigin::signed(ALICE),
                base_asset,
                CERES_ASSET_ID,
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
                current_timestamp + 5,
                current_timestamp + 10,
                balance!(1000),
                balance!(0.2),
                current_timestamp + 3,
                balance!(0.2),
                balance!(0.1),
                current_timestamp + 3,
                balance!(20)
            ),
            Error::<Runtime>::InvalidSoftCap
        );
    });
}

#[test]
fn create_ilo_invalid_minimum_contribution() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_err!(
            CeresLaunchpadPallet::<Runtime>::create_ilo(
                RuntimeOrigin::signed(ALICE),
                base_asset,
                CERES_ASSET_ID,
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
                current_timestamp + 5,
                current_timestamp + 10,
                balance!(1000),
                balance!(0.2),
                current_timestamp + 3,
                balance!(0.2),
                balance!(0.1),
                current_timestamp + 3,
                balance!(20)
            ),
            Error::<Runtime>::InvalidMinimumContribution
        );
    });
}

#[test]
fn create_ilo_invalid_maximum_contribution() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_err!(
            CeresLaunchpadPallet::<Runtime>::create_ilo(
                RuntimeOrigin::signed(ALICE),
                base_asset,
                CERES_ASSET_ID,
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
                current_timestamp + 5,
                current_timestamp + 10,
                balance!(1000),
                balance!(0.2),
                current_timestamp + 3,
                balance!(0.2),
                balance!(0.1),
                current_timestamp + 3,
                balance!(20)
            ),
            Error::<Runtime>::InvalidMaximumContribution
        );
    });
}

#[test]
fn create_ilo_invalid_liquidity_percent() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_err!(
            CeresLaunchpadPallet::<Runtime>::create_ilo(
                RuntimeOrigin::signed(ALICE),
                base_asset,
                CERES_ASSET_ID,
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
                current_timestamp + 5,
                current_timestamp + 10,
                balance!(1000),
                balance!(0.2),
                current_timestamp + 3,
                balance!(0.2),
                balance!(0.1),
                current_timestamp + 3,
                balance!(20)
            ),
            Error::<Runtime>::InvalidLiquidityPercent
        );
    });
}

#[test]
fn create_ilo_invalid_lockup_days() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_err!(
            CeresLaunchpadPallet::<Runtime>::create_ilo(
                RuntimeOrigin::signed(ALICE),
                base_asset,
                CERES_ASSET_ID,
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
                current_timestamp + 5,
                current_timestamp + 10,
                balance!(1000),
                balance!(0.2),
                current_timestamp + 3,
                balance!(0.2),
                balance!(0.1),
                current_timestamp + 3,
                balance!(20)
            ),
            Error::<Runtime>::InvalidLockupDays
        );
    });
}

#[test]
fn create_ilo_invalid_start_timestamp() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_err!(
            CeresLaunchpadPallet::<Runtime>::create_ilo(
                RuntimeOrigin::signed(ALICE),
                base_asset.into(),
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
                current_timestamp,
                current_timestamp + 10,
                balance!(1000),
                balance!(0.2),
                current_timestamp + 3,
                balance!(0.2),
                balance!(0.1),
                current_timestamp + 3,
                balance!(20)
            ),
            Error::<Runtime>::InvalidStartTimestamp
        );
    });
}

#[test]
fn create_ilo_invalid_end_timestamp() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_err!(
            CeresLaunchpadPallet::<Runtime>::create_ilo(
                RuntimeOrigin::signed(ALICE),
                base_asset,
                CERES_ASSET_ID,
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
                current_timestamp + 5,
                current_timestamp + 2,
                balance!(1000),
                balance!(0.2),
                current_timestamp + 3,
                balance!(0.2),
                balance!(0.1),
                current_timestamp + 3,
                balance!(20)
            ),
            Error::<Runtime>::InvalidEndTimestamp
        );
    });
}

#[test]
fn create_ilo_invalid_price() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_err!(
            CeresLaunchpadPallet::<Runtime>::create_ilo(
                RuntimeOrigin::signed(ALICE),
                base_asset,
                CERES_ASSET_ID,
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
                current_timestamp + 5,
                current_timestamp + 10,
                balance!(1000),
                balance!(0.2),
                current_timestamp + 3,
                balance!(0.2),
                balance!(0.1),
                current_timestamp + 3,
                balance!(20)
            ),
            Error::<Runtime>::InvalidPrice
        );
    });
}

#[test]
fn create_ilo_invalid_number_of_tokens_for_ilo() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_err!(
            CeresLaunchpadPallet::<Runtime>::create_ilo(
                RuntimeOrigin::signed(ALICE),
                base_asset,
                CERES_ASSET_ID,
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
                current_timestamp + 5,
                current_timestamp + 10,
                balance!(1000),
                balance!(0.2),
                current_timestamp + 3,
                balance!(0.2),
                balance!(0.1),
                current_timestamp + 3,
                balance!(20)
            ),
            Error::<Runtime>::InvalidNumberOfTokensForILO
        );
    });
}

#[test]
fn create_ilo_invalid_number_of_tokens_for_liquidity() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_err!(
            CeresLaunchpadPallet::<Runtime>::create_ilo(
                RuntimeOrigin::signed(ALICE),
                base_asset,
                CERES_ASSET_ID,
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
                current_timestamp + 5,
                current_timestamp + 10,
                balance!(1000),
                balance!(0.2),
                current_timestamp + 3,
                balance!(0.2),
                balance!(0.1),
                current_timestamp + 3,
                balance!(20)
            ),
            Error::<Runtime>::InvalidNumberOfTokensForLiquidity
        );
    });
}

#[test]
fn create_ilo_invalid_team_first_release_percent() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_err!(
            CeresLaunchpadPallet::<Runtime>::create_ilo(
                RuntimeOrigin::signed(ALICE),
                base_asset,
                CERES_ASSET_ID,
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
                current_timestamp + 5,
                current_timestamp + 10,
                balance!(1000),
                balance!(0),
                current_timestamp + 3,
                balance!(0.2),
                balance!(0.2),
                current_timestamp + 3,
                balance!(0.2),
            ),
            Error::<Runtime>::InvalidTeamFirstReleasePercent
        );
    });
}

#[test]
fn create_ilo_invalid_team_vesting_percent() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_err!(
            CeresLaunchpadPallet::<Runtime>::create_ilo(
                RuntimeOrigin::signed(ALICE),
                base_asset,
                CERES_ASSET_ID,
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
                current_timestamp + 5,
                current_timestamp + 10,
                balance!(1000),
                balance!(0.2),
                current_timestamp + 3,
                balance!(0),
                balance!(0.2),
                current_timestamp + 3,
                balance!(0.2),
            ),
            Error::<Runtime>::InvalidTeamVestingPercent
        );
    });
}

#[test]
fn create_ilo_invalid_team_vesting_percent_overflow() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_err!(
            CeresLaunchpadPallet::<Runtime>::create_ilo(
                RuntimeOrigin::signed(ALICE),
                base_asset,
                CERES_ASSET_ID,
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
                current_timestamp + 5,
                current_timestamp + 10,
                balance!(1000),
                balance!(0.2),
                current_timestamp + 3,
                balance!(0.9),
                balance!(0.2),
                current_timestamp + 3,
                balance!(0.2),
            ),
            Error::<Runtime>::InvalidTeamVestingPercent
        );
    });
}

#[test]
fn create_ilo_invalid_team_vesting_percent_remainder() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_err!(
            CeresLaunchpadPallet::<Runtime>::create_ilo(
                RuntimeOrigin::signed(ALICE),
                base_asset,
                CERES_ASSET_ID,
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
                current_timestamp + 5,
                current_timestamp + 10,
                balance!(1000),
                balance!(0.2),
                current_timestamp + 3,
                balance!(0.3),
                balance!(0.2),
                current_timestamp + 3,
                balance!(0.2),
            ),
            Error::<Runtime>::InvalidTeamVestingPercent
        );
    });
}

#[test]
fn create_ilo_invalid_team_vesting_period() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_err!(
            CeresLaunchpadPallet::<Runtime>::create_ilo(
                RuntimeOrigin::signed(ALICE),
                base_asset,
                CERES_ASSET_ID,
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
                current_timestamp + 5,
                current_timestamp + 10,
                balance!(1000),
                balance!(0.2),
                0u32.into(),
                balance!(0.2),
                balance!(0.2),
                current_timestamp + 3,
                balance!(0.2),
            ),
            Error::<Runtime>::InvalidTeamVestingPeriod
        );
    });
}

#[test]
fn create_ilo_invalid_first_release_percent() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_err!(
            CeresLaunchpadPallet::<Runtime>::create_ilo(
                RuntimeOrigin::signed(ALICE),
                base_asset,
                CERES_ASSET_ID,
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
                current_timestamp + 5,
                current_timestamp + 10,
                balance!(1000),
                balance!(0.2),
                current_timestamp + 3,
                balance!(0.2),
                balance!(0),
                current_timestamp + 3,
                balance!(20)
            ),
            Error::<Runtime>::InvalidFirstReleasePercent
        );
    });
}

#[test]
fn create_ilo_invalid_vesting_percent() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_err!(
            CeresLaunchpadPallet::<Runtime>::create_ilo(
                RuntimeOrigin::signed(ALICE),
                base_asset,
                CERES_ASSET_ID,
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
                current_timestamp + 5,
                current_timestamp + 10,
                balance!(1000),
                balance!(0.2),
                current_timestamp + 3,
                balance!(0.2),
                balance!(0.2),
                current_timestamp + 3,
                balance!(0)
            ),
            Error::<Runtime>::InvalidVestingPercent
        );
    });
}

#[test]
fn create_ilo_invalid_vesting_percent_overflow() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_err!(
            CeresLaunchpadPallet::<Runtime>::create_ilo(
                RuntimeOrigin::signed(ALICE),
                base_asset,
                CERES_ASSET_ID,
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
                current_timestamp + 5,
                current_timestamp + 10,
                balance!(1000),
                balance!(0.2),
                current_timestamp + 3,
                balance!(0.2),
                balance!(0.2),
                current_timestamp + 3,
                balance!(0.9)
            ),
            Error::<Runtime>::InvalidVestingPercent
        );
    });
}

#[test]
fn create_ilo_invalid_vesting_percent_remainder() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_err!(
            CeresLaunchpadPallet::<Runtime>::create_ilo(
                RuntimeOrigin::signed(CHARLES),
                base_asset,
                CERES_ASSET_ID,
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
                current_timestamp + 5,
                current_timestamp + 10,
                balance!(1000),
                balance!(0.2),
                current_timestamp + 3,
                balance!(0.2),
                balance!(0.2),
                current_timestamp + 3,
                balance!(0.3)
            ),
            Error::<Runtime>::InvalidVestingPercent
        );
    });
}

#[test]
fn create_ilo_invalid_vesting_period() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_err!(
            CeresLaunchpadPallet::<Runtime>::create_ilo(
                RuntimeOrigin::signed(ALICE),
                base_asset,
                CERES_ASSET_ID,
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
                current_timestamp + 5,
                current_timestamp + 10,
                balance!(1000),
                balance!(0.2),
                current_timestamp + 3,
                balance!(0.2),
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
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_err!(
            CeresLaunchpadPallet::<Runtime>::create_ilo(
                RuntimeOrigin::signed(BOB),
                base_asset,
                CERES_ASSET_ID,
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
                current_timestamp + 5,
                current_timestamp + 10,
                balance!(1000),
                balance!(0.2),
                current_timestamp + 3,
                balance!(0.2),
                balance!(0.2),
                current_timestamp + 3,
                balance!(0.2)
            ),
            Error::<Runtime>::NotEnoughCeres
        );
    });
}

#[test]
fn create_ilo_not_enough_tokens() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_err!(
            CeresLaunchpadPallet::<Runtime>::create_ilo(
                RuntimeOrigin::signed(CHARLES),
                base_asset,
                CERES_ASSET_ID,
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
                current_timestamp + 5,
                current_timestamp + 10,
                balance!(1000),
                balance!(0.2),
                current_timestamp + 3,
                balance!(0.2),
                balance!(0.2),
                current_timestamp + 3,
                balance!(0.2)
            ),
            Error::<Runtime>::NotEnoughTokens
        );
    });
}

#[test]
fn create_ilo_account_is_not_whitelisted() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_err!(
            CeresLaunchpadPallet::<Runtime>::create_ilo(
                RuntimeOrigin::signed(ALICE),
                base_asset,
                CERES_ASSET_ID,
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
                current_timestamp + 5,
                current_timestamp + 10,
                balance!(1000),
                balance!(0.2),
                current_timestamp + 3,
                balance!(0.2),
                balance!(0.2),
                current_timestamp + 3,
                balance!(0.2)
            ),
            Error::<Runtime>::AccountIsNotWhitelisted
        );
    });
}

#[test]
fn create_ilo_ok() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        pallet::WhitelistedIloOrganizers::<Runtime>::append(ALICE);
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
            RuntimeOrigin::signed(ALICE),
            base_asset,
            CERES_ASSET_ID,
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
            current_timestamp + 5,
            current_timestamp + 10,
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2)
        ));

        assert_eq!(
            Assets::free_balance(&CERES_ASSET_ID, &ALICE).expect("Failed to query free balance."),
            balance!(4297)
        );

        let pallet_account = PalletId(*b"crslaunc").into_account_truncating();
        assert_eq!(
            Assets::free_balance(&CERES_ASSET_ID, &pallet_account)
                .expect("Failed to query free balance."),
            balance!(10693)
        );

        let ilo_info = pallet::ILOs::<Runtime>::get(&CERES_ASSET_ID).unwrap();
        assert_ne!(ilo_info.ilo_price, balance!(0));
        assert_eq!(ilo_info.ilo_organizer, ALICE);
    });
}

#[test]
fn create_ilo_ilo_already_exists() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
            RuntimeOrigin::signed(ALICE),
            base_asset,
            CERES_ASSET_ID,
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
            current_timestamp + 5,
            current_timestamp + 10,
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2)
        ));

        assert_err!(
            CeresLaunchpadPallet::<Runtime>::create_ilo(
                RuntimeOrigin::signed(ALICE),
                base_asset,
                CERES_ASSET_ID,
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
                current_timestamp + 5,
                current_timestamp + 10,
                balance!(1000),
                balance!(0.2),
                current_timestamp + 3,
                balance!(0.2),
                balance!(0.2),
                current_timestamp + 3,
                balance!(0.2)
            ),
            Error::<Runtime>::ILOAlreadyExists
        );
    });
}

#[test]
fn contribute_ilo_does_not_exist() {
    preset_initial(|| {
        assert_err!(
            CeresLaunchpadPallet::<Runtime>::contribute(
                RuntimeOrigin::signed(ALICE),
                CERES_ASSET_ID.into(),
                balance!(0.21)
            ),
            Error::<Runtime>::ILODoesNotExist
        );
    });
}

#[test]
fn contribute_not_enough_ceres() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
            RuntimeOrigin::signed(ALICE),
            base_asset,
            CERES_ASSET_ID,
            balance!(7693),
            balance!(3000),
            balance!(0.13),
            balance!(600),
            balance!(1000),
            balance!(0.5),
            balance!(10),
            true,
            balance!(0.75),
            balance!(0.25),
            31,
            current_timestamp + 5,
            current_timestamp + 10,
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2)
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 6);

        assert_ok!(Assets::transfer_from(
            &CERES_ASSET_ID.into(),
            &DAN,
            &BOB,
            balance!(11000)
        ));

        assert_err!(
            CeresLaunchpadPallet::<Runtime>::contribute(
                RuntimeOrigin::signed(DAN),
                CERES_ASSET_ID.into(),
                balance!(0.6)
            ),
            Error::<Runtime>::NotEnoughCeres
        );
    });
}

#[test]
fn contribute_ilo_not_started() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
            RuntimeOrigin::signed(ALICE),
            base_asset,
            CERES_ASSET_ID,
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
            current_timestamp + 5,
            current_timestamp + 10,
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2)
        ));
        assert_err!(
            CeresLaunchpadPallet::<Runtime>::contribute(
                RuntimeOrigin::signed(ALICE),
                CERES_ASSET_ID.into(),
                balance!(0.21)
            ),
            Error::<Runtime>::ILONotStarted
        );
    });
}

#[test]
fn contribute_ilo_is_finished() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
            RuntimeOrigin::signed(ALICE),
            base_asset,
            CERES_ASSET_ID,
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
            current_timestamp + 5,
            current_timestamp + 10,
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2)
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 11);

        assert_err!(
            CeresLaunchpadPallet::<Runtime>::contribute(
                RuntimeOrigin::signed(ALICE),
                CERES_ASSET_ID.into(),
                balance!(0.21)
            ),
            Error::<Runtime>::ILOIsFinished
        );
    });
}

#[test]
fn contribute_contribution_is_lower_then_min() {
    preset_initial(|| {
        let asset_id = CERES_ASSET_ID;
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
            RuntimeOrigin::signed(ALICE),
            base_asset.into(),
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
            current_timestamp + 5,
            current_timestamp + 10,
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2)
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 6);

        assert_err!(
            CeresLaunchpadPallet::<Runtime>::contribute(
                RuntimeOrigin::signed(ALICE),
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
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
            RuntimeOrigin::signed(ALICE),
            base_asset.into(),
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
            current_timestamp + 5,
            current_timestamp + 10,
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2)
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 6);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::contribute(
            RuntimeOrigin::signed(ALICE),
            CERES_ASSET_ID.into(),
            balance!(0.2)
        ));
        assert_err!(
            CeresLaunchpadPallet::<Runtime>::contribute(
                RuntimeOrigin::signed(ALICE),
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
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let asset_id = CERES_ASSET_ID;
        let base_asset = XOR;
        assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
            RuntimeOrigin::signed(ALICE),
            base_asset.into(),
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
            current_timestamp + 5,
            current_timestamp + 10,
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2)
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 6);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::contribute(
            RuntimeOrigin::signed(ALICE),
            CERES_ASSET_ID.into(),
            balance!(100)
        ));

        assert_err!(
            CeresLaunchpadPallet::<Runtime>::contribute(
                RuntimeOrigin::signed(BOB),
                CERES_ASSET_ID.into(),
                balance!(901)
            ),
            Error::<Runtime>::HardCapIsHit
        );
    });
}

#[test]
fn contribute_account_is_not_whitelisted() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
            RuntimeOrigin::signed(ALICE),
            base_asset.into(),
            CERES_ASSET_ID.into(),
            balance!(7693),
            balance!(3000),
            balance!(0.13),
            balance!(600),
            balance!(1000),
            balance!(0.5),
            balance!(10),
            true,
            balance!(0.75),
            balance!(0.25),
            31,
            current_timestamp + 5,
            current_timestamp + 10,
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2)
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 6);

        assert_err!(
            CeresLaunchpadPallet::<Runtime>::contribute(
                RuntimeOrigin::signed(EMILY),
                CERES_ASSET_ID.into(),
                balance!(0.6)
            ),
            Error::<Runtime>::AccountIsNotWhitelisted
        );
    });
}

#[test]
fn contribute_ok() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let asset_id = CERES_ASSET_ID;
        let base_asset = XOR;
        assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
            RuntimeOrigin::signed(ALICE),
            base_asset.into(),
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
            current_timestamp + 5,
            current_timestamp + 10,
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2)
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 6);
        let funds_to_contribute = balance!(0.21);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::contribute(
            RuntimeOrigin::signed(CHARLES),
            CERES_ASSET_ID.into(),
            funds_to_contribute
        ));

        let ilo_info = pallet::ILOs::<Runtime>::get(&CERES_ASSET_ID).unwrap();
        assert_eq!(ilo_info.funds_raised, funds_to_contribute);
        let tokens_bought = (FixedWrapper::from(funds_to_contribute)
            / FixedWrapper::from(ilo_info.ilo_price))
        .try_into_balance()
        .unwrap_or(0);

        let contribution_info = pallet::Contributions::<Runtime>::get(&CERES_ASSET_ID, &CHARLES);

        assert_eq!(ilo_info.sold_tokens, tokens_bought);
        assert_eq!(contribution_info.funds_contributed, funds_to_contribute);
        assert_eq!(contribution_info.tokens_bought, tokens_bought);

        assert_eq!(
            Assets::free_balance(&base_asset.into(), &CHARLES)
                .expect("Failed to query free balance."),
            balance!(1999.79)
        );
    });
}

#[test]
fn contribute_base_asset_xstusd_ok() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let asset_id = CERES_ASSET_ID;
        let base_asset = XSTUSD;
        assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
            RuntimeOrigin::signed(ALICE),
            base_asset.into(),
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
            current_timestamp + 5,
            current_timestamp + 10,
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2)
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 6);
        let funds_to_contribute = balance!(0.21);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::contribute(
            RuntimeOrigin::signed(CHARLES),
            CERES_ASSET_ID.into(),
            funds_to_contribute
        ));

        let ilo_info = pallet::ILOs::<Runtime>::get(&CERES_ASSET_ID).unwrap();
        assert_eq!(ilo_info.funds_raised, funds_to_contribute);
        let tokens_bought = (FixedWrapper::from(funds_to_contribute)
            / FixedWrapper::from(ilo_info.ilo_price))
        .try_into_balance()
        .unwrap_or(0);

        let contribution_info = pallet::Contributions::<Runtime>::get(&CERES_ASSET_ID, &CHARLES);

        assert_eq!(ilo_info.sold_tokens, tokens_bought);
        assert_eq!(contribution_info.funds_contributed, funds_to_contribute);
        assert_eq!(contribution_info.tokens_bought, tokens_bought);

        assert_eq!(
            Assets::free_balance(&base_asset.into(), &CHARLES)
                .expect("Failed to query free balance."),
            balance!(999.79)
        );
    });
}

#[test]
fn emergency_withdraw_ilo_does_not_exist() {
    preset_initial(|| {
        assert_err!(
            CeresLaunchpadPallet::<Runtime>::emergency_withdraw(
                RuntimeOrigin::signed(ALICE),
                CERES_ASSET_ID.into()
            ),
            Error::<Runtime>::ILODoesNotExist
        );
    });
}

#[test]
fn emergency_withdraw_ilo_not_started() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
            RuntimeOrigin::signed(ALICE),
            base_asset.into(),
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
            current_timestamp + 5,
            current_timestamp + 10,
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2)
        ));

        assert_err!(
            CeresLaunchpadPallet::<Runtime>::emergency_withdraw(
                RuntimeOrigin::signed(ALICE),
                CERES_ASSET_ID.into()
            ),
            Error::<Runtime>::ILONotStarted
        );
    });
}

#[test]
fn emergency_withdraw_ilo_is_finished() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
            RuntimeOrigin::signed(ALICE),
            base_asset.into(),
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
            current_timestamp + 5,
            current_timestamp + 10,
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2)
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 11);

        assert_err!(
            CeresLaunchpadPallet::<Runtime>::emergency_withdraw(
                RuntimeOrigin::signed(ALICE),
                CERES_ASSET_ID.into()
            ),
            Error::<Runtime>::ILOIsFinished
        );
    });
}

#[test]
fn emergency_withdraw_not_enough_funds() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
            RuntimeOrigin::signed(ALICE),
            base_asset.into(),
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
            current_timestamp + 5,
            current_timestamp + 10,
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2)
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 6);

        assert_err!(
            CeresLaunchpadPallet::<Runtime>::emergency_withdraw(
                RuntimeOrigin::signed(ALICE),
                CERES_ASSET_ID.into()
            ),
            Error::<Runtime>::NotEnoughFunds
        );
    });
}

#[test]
fn emergency_withdraw_ok() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
            RuntimeOrigin::signed(ALICE),
            base_asset.into(),
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
            current_timestamp + 5,
            current_timestamp + 10,
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2)
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 6);

        let funds_to_contribute = balance!(0.21);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::contribute(
            RuntimeOrigin::signed(CHARLES),
            CERES_ASSET_ID.into(),
            funds_to_contribute
        ));

        let mut contribution_info =
            pallet::Contributions::<Runtime>::get(&CERES_ASSET_ID, &CHARLES);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::emergency_withdraw(
            RuntimeOrigin::signed(CHARLES),
            CERES_ASSET_ID.into(),
        ));

        let funds_to_claim = (FixedWrapper::from(contribution_info.funds_contributed)
            * FixedWrapper::from(0.8))
        .try_into_balance()
        .unwrap_or(0);

        let penalty = contribution_info.funds_contributed - funds_to_claim;
        let ilo_info = pallet::ILOs::<Runtime>::get(&CERES_ASSET_ID).unwrap();

        assert_eq!(
            Assets::free_balance(&base_asset.into(), &CHARLES)
                .expect("Failed to query free balance."),
            balance!(1999.958)
        );

        assert_eq!(
            Assets::free_balance(
                &base_asset.into(),
                &pallet::PenaltiesAccount::<Runtime>::get()
            )
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
fn emergency_withdraw_base_asset_xstusd_ok() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XSTUSD;
        assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
            RuntimeOrigin::signed(ALICE),
            base_asset.into(),
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
            current_timestamp + 5,
            current_timestamp + 10,
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2)
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 6);

        let funds_to_contribute = balance!(0.21);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::contribute(
            RuntimeOrigin::signed(CHARLES),
            CERES_ASSET_ID.into(),
            funds_to_contribute
        ));

        let mut contribution_info =
            pallet::Contributions::<Runtime>::get(&CERES_ASSET_ID, &CHARLES);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::emergency_withdraw(
            RuntimeOrigin::signed(CHARLES),
            CERES_ASSET_ID.into(),
        ));

        let funds_to_claim = (FixedWrapper::from(contribution_info.funds_contributed)
            * FixedWrapper::from(0.8))
        .try_into_balance()
        .unwrap_or(0);

        let penalty = contribution_info.funds_contributed - funds_to_claim;
        let ilo_info = pallet::ILOs::<Runtime>::get(&CERES_ASSET_ID).unwrap();

        assert_eq!(
            Assets::free_balance(&base_asset.into(), &CHARLES)
                .expect("Failed to query free balance."),
            balance!(999.958)
        );

        assert_eq!(
            Assets::free_balance(
                &base_asset.into(),
                &pallet::PenaltiesAccount::<Runtime>::get()
            )
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
    preset_initial(|| {
        assert_err!(
            CeresLaunchpadPallet::<Runtime>::finish_ilo(
                RuntimeOrigin::signed(ALICE),
                CERES_ASSET_ID.into()
            ),
            Error::<Runtime>::ILODoesNotExist
        );
    });
}

#[test]
fn finish_ilo_ilo_is_not_finished() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
            RuntimeOrigin::signed(ALICE),
            base_asset.into(),
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
            current_timestamp + 5,
            current_timestamp + 10,
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2)
        ));

        assert_err!(
            CeresLaunchpadPallet::<Runtime>::finish_ilo(
                RuntimeOrigin::signed(ALICE),
                CERES_ASSET_ID.into()
            ),
            Error::<Runtime>::ILOIsNotFinished
        );
    });
}

#[test]
fn finish_ilo_unauthorized() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
            RuntimeOrigin::signed(ALICE),
            base_asset.into(),
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
            current_timestamp + 5,
            current_timestamp + 10,
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2)
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 11);

        assert_err!(
            CeresLaunchpadPallet::<Runtime>::finish_ilo(
                RuntimeOrigin::signed(CHARLES),
                CERES_ASSET_ID.into()
            ),
            Error::<Runtime>::Unauthorized
        );
    });
}

#[test]
fn finish_ilo_ilo_failed_refunded_ok() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
            RuntimeOrigin::signed(ALICE),
            base_asset.into(),
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
            current_timestamp + 5,
            current_timestamp + 10,
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2)
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 11);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::finish_ilo(
            RuntimeOrigin::signed(ALICE),
            CERES_ASSET_ID.into()
        ),);

        let pallet_account = PalletId(*b"crslaunc").into_account_truncating();
        assert_eq!(
            Assets::free_balance(&CERES_ASSET_ID, &pallet_account)
                .expect("Failed to query free balance."),
            balance!(0)
        );

        assert_eq!(
            Assets::free_balance(&CERES_ASSET_ID, &ALICE).expect("Failed to query free balance."),
            balance!(15990)
        );

        let ilo_info = pallet::ILOs::<Runtime>::get(&CERES_ASSET_ID).unwrap();
        assert_eq!(ilo_info.failed, true);
    });
}

#[test]
fn finish_ilo_ilo_failed_burned_ok() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
            RuntimeOrigin::signed(ALICE),
            base_asset.into(),
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
            current_timestamp + 5,
            current_timestamp + 10,
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2)
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 11);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::finish_ilo(
            RuntimeOrigin::signed(ALICE),
            CERES_ASSET_ID.into()
        ),);

        let pallet_account = PalletId(*b"crslaunc").into_account_truncating();
        assert_eq!(
            Assets::free_balance(&CERES_ASSET_ID, &pallet_account)
                .expect("Failed to query free balance."),
            balance!(0)
        );

        assert_eq!(
            Assets::free_balance(&CERES_ASSET_ID, &ALICE).expect("Failed to query free balance."),
            balance!(5297)
        );

        let ilo_info = pallet::ILOs::<Runtime>::get(&CERES_ASSET_ID).unwrap();
        assert_eq!(ilo_info.failed, true);
    });
}

#[test]
fn finish_ilo_ilo_is_failed() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
            RuntimeOrigin::signed(ALICE),
            base_asset.into(),
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
            current_timestamp + 5,
            current_timestamp + 10,
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2)
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 11);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::finish_ilo(
            RuntimeOrigin::signed(ALICE),
            CERES_ASSET_ID.into()
        ),);

        assert_err!(
            CeresLaunchpadPallet::<Runtime>::finish_ilo(
                RuntimeOrigin::signed(ALICE),
                CERES_ASSET_ID.into()
            ),
            Error::<Runtime>::ILOIsFailed
        );
    });
}

#[test]
fn finish_ilo_not_filled_hard_cap_ok() {
    preset_initial(|| {
        let mut current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
            RuntimeOrigin::signed(ALICE),
            base_asset.into(),
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
            current_timestamp + 5,
            current_timestamp + 10,
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2)
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 6);

        let funds_to_contribute = balance!(800);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::contribute(
            RuntimeOrigin::signed(CHARLES),
            CERES_ASSET_ID.into(),
            funds_to_contribute
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 11);

        current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        assert_ok!(CeresLaunchpadPallet::<Runtime>::finish_ilo(
            RuntimeOrigin::signed(ALICE),
            CERES_ASSET_ID.into()
        ),);

        let ilo_info = pallet::ILOs::<Runtime>::get(&CERES_ASSET_ID).unwrap();

        let funds_raised_fee = (FixedWrapper::from(ilo_info.funds_raised)
            * FixedWrapper::from(FeePercentOnRaisedFunds::<Runtime>::get()))
        .try_into_balance()
        .unwrap_or(0);
        let raised_funds_without_fee = ilo_info.funds_raised - funds_raised_fee;
        let funds_for_liquidity = (FixedWrapper::from(raised_funds_without_fee)
            * FixedWrapper::from(ilo_info.liquidity_percent))
        .try_into_balance()
        .unwrap_or(0);
        let funds_for_team = raised_funds_without_fee - funds_for_liquidity;

        assert_eq!(
            Assets::free_balance(&base_asset, &pallet::AuthorityAccount::<Runtime>::get())
                .expect("Failed to query free balance."),
            funds_raised_fee
        );
        assert_eq!(
            Assets::free_balance(&base_asset, &ALICE).expect("Failed to query free balance."),
            funds_for_team + balance!(900000)
        );

        let (xor_liq, ceres_liq) = pool_xyk::Reserves::<Runtime>::get(base_asset, CERES_ASSET_ID);
        assert_eq!(xor_liq, funds_for_liquidity);

        let tokens_for_liquidity = (FixedWrapper::from(funds_for_liquidity)
            / FixedWrapper::from(ilo_info.listing_price))
        .try_into_balance()
        .unwrap_or(0);
        assert_eq!(ceres_liq, tokens_for_liquidity);

        let pallet_account = PalletId(*b"crslaunc").into_account_truncating();
        assert_eq!(
            Assets::free_balance(&CERES_ASSET_ID, &pallet_account)
                .expect("Failed to query free balance."),
            ilo_info.sold_tokens
        );

        assert_err!(
            pool_xyk::Pallet::<Runtime>::withdraw_liquidity(
                RuntimeOrigin::signed(pallet_account),
                DEX_A_ID,
                base_asset.into(),
                CERES_ASSET_ID.into(),
                ilo_info.lp_tokens,
                balance!(1),
                balance!(1)
            ),
            pool_xyk::Error::<Runtime>::NotEnoughUnlockedLiquidity
        );

        assert_eq!(ilo_info.finish_timestamp, current_timestamp);
    });
}

#[test]
fn finish_ilo_not_enough_team_tokens_to_lock() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
            RuntimeOrigin::signed(DAN),
            base_asset.into(),
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
            current_timestamp + 5,
            current_timestamp + 10,
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2)
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 6);

        let funds_to_contribute = balance!(1000);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::contribute(
            RuntimeOrigin::signed(CHARLES),
            CERES_ASSET_ID.into(),
            funds_to_contribute
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 11);

        assert_err!(
            CeresLaunchpadPallet::<Runtime>::finish_ilo(
                RuntimeOrigin::signed(DAN),
                CERES_ASSET_ID.into()
            ),
            Error::<Runtime>::NotEnoughTeamTokensToLock
        );
    });
}

#[test]
fn finish_ilo_filled_hard_cap_ok() {
    preset_initial(|| {
        let mut current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
            RuntimeOrigin::signed(ALICE),
            base_asset.into(),
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
            current_timestamp + 5,
            current_timestamp + 10,
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2)
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 6);

        let funds_to_contribute = balance!(1000);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::contribute(
            RuntimeOrigin::signed(CHARLES),
            CERES_ASSET_ID.into(),
            funds_to_contribute
        ));

        current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        assert_ok!(CeresLaunchpadPallet::<Runtime>::finish_ilo(
            RuntimeOrigin::signed(ALICE),
            CERES_ASSET_ID.into()
        ),);

        let ilo_info = pallet::ILOs::<Runtime>::get(&CERES_ASSET_ID).unwrap();

        let funds_raised_fee = (FixedWrapper::from(ilo_info.funds_raised)
            * FixedWrapper::from(FeePercentOnRaisedFunds::<Runtime>::get()))
        .try_into_balance()
        .unwrap_or(0);
        let raised_funds_without_fee = ilo_info.funds_raised - funds_raised_fee;
        let funds_for_liquidity = (FixedWrapper::from(raised_funds_without_fee)
            * FixedWrapper::from(ilo_info.liquidity_percent))
        .try_into_balance()
        .unwrap_or(0);
        let funds_for_team = raised_funds_without_fee - funds_for_liquidity;

        assert_eq!(
            Assets::free_balance(&base_asset, &pallet::AuthorityAccount::<Runtime>::get())
                .expect("Failed to query free balance."),
            funds_raised_fee
        );
        assert_eq!(
            Assets::free_balance(&base_asset, &ALICE).expect("Failed to query free balance."),
            funds_for_team + balance!(900000)
        );

        let (xor_liq, ceres_liq) = pool_xyk::Reserves::<Runtime>::get(base_asset, CERES_ASSET_ID);
        assert_eq!(xor_liq, funds_for_liquidity);

        let tokens_for_liquidity = (FixedWrapper::from(funds_for_liquidity)
            / FixedWrapper::from(ilo_info.listing_price))
        .try_into_balance()
        .unwrap_or(0);
        assert_eq!(ceres_liq, tokens_for_liquidity);

        let pallet_account = PalletId(*b"crslaunc").into_account_truncating();
        assert_eq!(
            Assets::free_balance(&CERES_ASSET_ID, &pallet_account)
                .expect("Failed to query free balance."),
            ilo_info.sold_tokens
        );

        assert_err!(
            pool_xyk::Pallet::<Runtime>::withdraw_liquidity(
                RuntimeOrigin::signed(pallet_account),
                DEX_A_ID,
                base_asset.into(),
                CERES_ASSET_ID.into(),
                ilo_info.lp_tokens,
                balance!(1),
                balance!(1)
            ),
            pool_xyk::Error::<Runtime>::NotEnoughUnlockedLiquidity
        );

        let token_locker_data = ceres_token_locker::TokenLockerData::<Runtime>::get(ALICE);
        assert_eq!(token_locker_data.len(), 4 as usize);
        let mut unlocking_timestamp = current_timestamp + ilo_info.team_vesting.team_vesting_period;
        for token_lock_info in token_locker_data {
            assert_eq!(token_lock_info.asset_id, CERES_ASSET_ID.into());
            assert_eq!(token_lock_info.tokens, balance!(200));
            assert_eq!(token_lock_info.unlocking_timestamp, unlocking_timestamp);
            unlocking_timestamp += ilo_info.team_vesting.team_vesting_period;
        }

        assert_eq!(ilo_info.finish_timestamp, current_timestamp);
    });
}

#[test]
fn finish_ilo_filled_hard_cap_base_asset_xstusd_ok() {
    preset_initial(|| {
        let mut current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XSTUSD;
        assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
            RuntimeOrigin::signed(ALICE),
            base_asset.into(),
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
            current_timestamp + 5,
            current_timestamp + 10,
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2)
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 6);

        let funds_to_contribute = balance!(1000);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::contribute(
            RuntimeOrigin::signed(CHARLES),
            CERES_ASSET_ID.into(),
            funds_to_contribute
        ));

        current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        assert_ok!(CeresLaunchpadPallet::<Runtime>::finish_ilo(
            RuntimeOrigin::signed(ALICE),
            CERES_ASSET_ID.into()
        ),);

        let ilo_info = pallet::ILOs::<Runtime>::get(&CERES_ASSET_ID).unwrap();

        let funds_raised_fee = (FixedWrapper::from(ilo_info.funds_raised)
            * FixedWrapper::from(FeePercentOnRaisedFunds::<Runtime>::get()))
        .try_into_balance()
        .unwrap_or(0);
        let raised_funds_without_fee = ilo_info.funds_raised - funds_raised_fee;
        let funds_for_liquidity = (FixedWrapper::from(raised_funds_without_fee)
            * FixedWrapper::from(ilo_info.liquidity_percent))
        .try_into_balance()
        .unwrap_or(0);
        let funds_for_team = raised_funds_without_fee - funds_for_liquidity;

        assert_eq!(
            Assets::free_balance(&base_asset, &pallet::AuthorityAccount::<Runtime>::get())
                .expect("Failed to query free balance."),
            funds_raised_fee
        );
        assert_eq!(
            Assets::free_balance(&base_asset, &ALICE).expect("Failed to query free balance."),
            funds_for_team
        );

        let (xor_liq, ceres_liq) = pool_xyk::Reserves::<Runtime>::get(base_asset, CERES_ASSET_ID);
        assert_eq!(xor_liq, funds_for_liquidity);

        let tokens_for_liquidity = (FixedWrapper::from(funds_for_liquidity)
            / FixedWrapper::from(ilo_info.listing_price))
        .try_into_balance()
        .unwrap_or(0);
        assert_eq!(ceres_liq, tokens_for_liquidity);

        let pallet_account = PalletId(*b"crslaunc").into_account_truncating();
        assert_eq!(
            Assets::free_balance(&CERES_ASSET_ID, &pallet_account)
                .expect("Failed to query free balance."),
            ilo_info.sold_tokens
        );

        assert_err!(
            pool_xyk::Pallet::<Runtime>::withdraw_liquidity(
                RuntimeOrigin::signed(pallet_account),
                DEX_B_ID,
                base_asset.into(),
                CERES_ASSET_ID.into(),
                ilo_info.lp_tokens,
                balance!(1),
                balance!(1)
            ),
            pool_xyk::Error::<Runtime>::NotEnoughUnlockedLiquidity
        );

        let token_locker_data = ceres_token_locker::TokenLockerData::<Runtime>::get(ALICE);
        assert_eq!(token_locker_data.len(), 4 as usize);
        let mut unlocking_timestamp = current_timestamp + ilo_info.team_vesting.team_vesting_period;
        for token_lock_info in token_locker_data {
            assert_eq!(token_lock_info.asset_id, CERES_ASSET_ID.into());
            assert_eq!(token_lock_info.tokens, balance!(200));
            assert_eq!(token_lock_info.unlocking_timestamp, unlocking_timestamp);
            unlocking_timestamp += ilo_info.team_vesting.team_vesting_period;
        }

        assert_eq!(ilo_info.finish_timestamp, current_timestamp);
    });
}

#[test]
fn claim_ilo_does_not_exist() {
    preset_initial(|| {
        assert_err!(
            CeresLaunchpadPallet::<Runtime>::claim(
                RuntimeOrigin::signed(CHARLES),
                CERES_ASSET_ID.into(),
            ),
            Error::<Runtime>::ILODoesNotExist
        );
    });
}

#[test]
fn claim_ilo_is_not_finished() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
            RuntimeOrigin::signed(ALICE),
            base_asset.into(),
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
            current_timestamp + 5,
            current_timestamp + 10,
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2)
        ));

        assert_err!(
            CeresLaunchpadPallet::<Runtime>::claim(
                RuntimeOrigin::signed(CHARLES),
                CERES_ASSET_ID.into(),
            ),
            Error::<Runtime>::ILOIsNotFinished
        );
    });
}

#[test]
fn claim_ilo_failed_ok() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
            RuntimeOrigin::signed(ALICE),
            base_asset.into(),
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
            current_timestamp + 5,
            current_timestamp + 10,
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2)
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 6);

        let funds_to_contribute = balance!(0.21);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::contribute(
            RuntimeOrigin::signed(CHARLES),
            CERES_ASSET_ID.into(),
            funds_to_contribute
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 11);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::finish_ilo(
            RuntimeOrigin::signed(ALICE),
            CERES_ASSET_ID.into()
        ),);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::claim(
            RuntimeOrigin::signed(CHARLES),
            CERES_ASSET_ID.into(),
        ));

        assert_eq!(
            Assets::free_balance(&CERES_ASSET_ID, &CHARLES).expect("Failed to query free balance."),
            balance!(5000)
        );

        let contribution_info = pallet::Contributions::<Runtime>::get(&CERES_ASSET_ID, &CHARLES);
        assert_eq!(contribution_info.claiming_finished, true);
    });
}

#[test]
fn claim_funds_already_claimed() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
            RuntimeOrigin::signed(ALICE),
            base_asset.into(),
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
            current_timestamp + 5,
            current_timestamp + 10,
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2)
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 6);

        let funds_to_contribute = balance!(0.21);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::contribute(
            RuntimeOrigin::signed(CHARLES),
            CERES_ASSET_ID.into(),
            funds_to_contribute
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 11);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::finish_ilo(
            RuntimeOrigin::signed(ALICE),
            CERES_ASSET_ID.into()
        ),);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::claim(
            RuntimeOrigin::signed(CHARLES),
            CERES_ASSET_ID.into(),
        ));

        assert_err!(
            CeresLaunchpadPallet::<Runtime>::claim(
                RuntimeOrigin::signed(CHARLES),
                CERES_ASSET_ID.into(),
            ),
            Error::<Runtime>::FundsAlreadyClaimed
        );
    });
}

#[test]
fn claim_first_release_claim_ok() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
            RuntimeOrigin::signed(ALICE),
            base_asset.into(),
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
            current_timestamp + 5,
            current_timestamp + 10,
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2)
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 6);

        let funds_to_contribute = balance!(1000);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::contribute(
            RuntimeOrigin::signed(CHARLES),
            CERES_ASSET_ID.into(),
            funds_to_contribute
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 11);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::finish_ilo(
            RuntimeOrigin::signed(ALICE),
            CERES_ASSET_ID.into()
        ),);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::claim(
            RuntimeOrigin::signed(CHARLES),
            CERES_ASSET_ID.into(),
        ));

        let ilo_info = pallet::ILOs::<Runtime>::get(&CERES_ASSET_ID).unwrap();
        let contribution_info = pallet::Contributions::<Runtime>::get(&CERES_ASSET_ID, &CHARLES);

        let tokens_to_claim = (FixedWrapper::from(contribution_info.tokens_bought)
            * FixedWrapper::from(ilo_info.contributors_vesting.first_release_percent))
        .try_into_balance()
        .unwrap_or(0);

        assert_eq!(
            Assets::free_balance(&CERES_ASSET_ID, &CHARLES).expect("Failed to query free balance."),
            balance!(5000) + tokens_to_claim
        );

        assert_eq!(contribution_info.tokens_claimed, tokens_to_claim);
    });
}

#[test]
fn claim_no_potential_claims() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
            RuntimeOrigin::signed(ALICE),
            base_asset.into(),
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
            current_timestamp + 5,
            current_timestamp + 10,
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2),
            balance!(0.2),
            current_timestamp + 50,
            balance!(0.2)
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 6);

        let funds_to_contribute = balance!(1000);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::contribute(
            RuntimeOrigin::signed(CHARLES),
            CERES_ASSET_ID.into(),
            funds_to_contribute
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 11);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::finish_ilo(
            RuntimeOrigin::signed(ALICE),
            CERES_ASSET_ID.into()
        ),);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::claim(
            RuntimeOrigin::signed(CHARLES),
            CERES_ASSET_ID.into(),
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 12);

        assert_err!(
            CeresLaunchpadPallet::<Runtime>::claim(
                RuntimeOrigin::signed(CHARLES),
                CERES_ASSET_ID.into(),
            ),
            Error::<Runtime>::NothingToClaim
        );
    });
}

#[test]
fn claim_ok() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
            RuntimeOrigin::signed(ALICE),
            base_asset.into(),
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
            current_timestamp + 5,
            current_timestamp + 10,
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2),
            balance!(0.1),
            30u32.into(),
            balance!(0.18)
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 6);

        let funds_to_contribute = balance!(1000);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::contribute(
            RuntimeOrigin::signed(CHARLES),
            CERES_ASSET_ID.into(),
            funds_to_contribute
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 11);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::finish_ilo(
            RuntimeOrigin::signed(ALICE),
            CERES_ASSET_ID.into()
        ),);

        let ilo_info = pallet::ILOs::<Runtime>::get(&CERES_ASSET_ID).unwrap();
        let mut contribution_info =
            pallet::Contributions::<Runtime>::get(&CERES_ASSET_ID, &CHARLES);

        let first_release = (FixedWrapper::from(contribution_info.tokens_bought)
            * FixedWrapper::from(ilo_info.contributors_vesting.first_release_percent))
        .try_into_balance()
        .unwrap_or(0);

        let tokens_per_claim = (FixedWrapper::from(contribution_info.tokens_bought)
            * FixedWrapper::from(ilo_info.contributors_vesting.vesting_percent))
        .try_into_balance()
        .unwrap_or(0);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::claim(
            RuntimeOrigin::signed(CHARLES),
            CERES_ASSET_ID.into(),
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 43);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::claim(
            RuntimeOrigin::signed(CHARLES),
            CERES_ASSET_ID.into(),
        ));

        assert_eq!(
            Assets::free_balance(&CERES_ASSET_ID, &CHARLES).expect("Failed to query free balance."),
            balance!(5000) + first_release + tokens_per_claim
        );

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 103);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::claim(
            RuntimeOrigin::signed(CHARLES),
            CERES_ASSET_ID.into(),
        ));
        assert_eq!(
            Assets::free_balance(&CERES_ASSET_ID, &CHARLES).expect("Failed to query free balance."),
            balance!(5000) + first_release + tokens_per_claim * 3
        );

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 163);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::claim(
            RuntimeOrigin::signed(CHARLES),
            CERES_ASSET_ID.into(),
        ));
        contribution_info = pallet::Contributions::<Runtime>::get(&CERES_ASSET_ID, &CHARLES);
        assert_eq!(
            Assets::free_balance(&CERES_ASSET_ID, &CHARLES).expect("Failed to query free balance."),
            balance!(5000) + first_release + tokens_per_claim * 5
        );
        assert_eq!(contribution_info.claiming_finished, true);
    });
}

#[test]
fn change_fee_percent_for_raised_funds_unauthorized() {
    preset_initial(|| {
        assert_err!(
            CeresLaunchpadPallet::<Runtime>::change_fee_percent_for_raised_funds(
                RuntimeOrigin::signed(ALICE),
                balance!(0.02)
            ),
            Error::<Runtime>::Unauthorized
        );
    });
}

#[test]
fn change_fee_percent_for_raised_funds_invalid_fee_percent() {
    preset_initial(|| {
        assert_err!(
            CeresLaunchpadPallet::<Runtime>::change_fee_percent_for_raised_funds(
                RuntimeOrigin::signed(pallet::AuthorityAccount::<Runtime>::get()),
                balance!(1.2)
            ),
            Error::<Runtime>::InvalidFeePercent
        );
    });
}

#[test]
fn change_fee_percent_for_raised_funds_ok() {
    preset_initial(|| {
        assert_ok!(
            CeresLaunchpadPallet::<Runtime>::change_fee_percent_for_raised_funds(
                RuntimeOrigin::signed(pallet::AuthorityAccount::<Runtime>::get()),
                balance!(0.02)
            )
        );

        assert_eq!(
            pallet::FeePercentOnRaisedFunds::<Runtime>::get(),
            balance!(0.02)
        );
    });
}

#[test]
fn change_ceres_burn_fee_unauthorized() {
    preset_initial(|| {
        assert_err!(
            CeresLaunchpadPallet::<Runtime>::change_ceres_burn_fee(
                RuntimeOrigin::signed(ALICE),
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
            RuntimeOrigin::signed(pallet::AuthorityAccount::<Runtime>::get()),
            balance!(100)
        ));

        assert_eq!(pallet::CeresBurnFeeAmount::<Runtime>::get(), balance!(100));
    });
}

#[test]
fn claim_lp_tokens_ilo_does_not_exist() {
    preset_initial(|| {
        assert_err!(
            CeresLaunchpadPallet::<Runtime>::claim_lp_tokens(
                RuntimeOrigin::signed(ALICE),
                CERES_ASSET_ID.into()
            ),
            Error::<Runtime>::ILODoesNotExist
        );
    });
}

#[test]
fn claim_lp_tokens_cant_claim_lp_tokens() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
            RuntimeOrigin::signed(ALICE),
            base_asset.into(),
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
            current_timestamp + 5,
            current_timestamp + 10,
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2)
        ));

        assert_err!(
            CeresLaunchpadPallet::<Runtime>::claim_lp_tokens(
                RuntimeOrigin::signed(ALICE),
                CERES_ASSET_ID.into()
            ),
            Error::<Runtime>::CantClaimLPTokens
        );
    });
}

#[test]
fn claim_lp_tokens_unauthorized() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
            RuntimeOrigin::signed(ALICE),
            base_asset.into(),
            CERES_ASSET_ID.into(),
            balance!(7693),
            balance!(3000),
            balance!(0.13),
            balance!(600),
            balance!(1000),
            balance!(0.2),
            balance!(850),
            true,
            balance!(0.75),
            balance!(0.25),
            31,
            current_timestamp + 5,
            current_timestamp + 10,
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2)
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 6);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::contribute(
            RuntimeOrigin::signed(CHARLES),
            CERES_ASSET_ID.into(),
            balance!(800)
        ),);

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 11);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::finish_ilo(
            RuntimeOrigin::signed(ALICE),
            CERES_ASSET_ID.into()
        ),);

        let ilo_info = pallet::ILOs::<Runtime>::get(&CERES_ASSET_ID).unwrap();
        let unlocking_timestamp = ilo_info
            .finish_timestamp
            .saturating_add(86_400_000u64.saturating_mul(ilo_info.lockup_days.into()));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(unlocking_timestamp + 1);

        assert_err!(
            CeresLaunchpadPallet::<Runtime>::claim_lp_tokens(
                RuntimeOrigin::signed(CHARLES),
                CERES_ASSET_ID.into()
            ),
            Error::<Runtime>::Unauthorized
        );
    });
}

#[test]
fn claim_lp_tokens_ok() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
            RuntimeOrigin::signed(ALICE),
            base_asset.into(),
            CERES_ASSET_ID.into(),
            balance!(7693),
            balance!(3000),
            balance!(0.13),
            balance!(600),
            balance!(1000),
            balance!(0.2),
            balance!(850),
            true,
            balance!(0.75),
            balance!(0.25),
            31,
            current_timestamp + 5,
            current_timestamp + 10,
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2)
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 6);

        let funds_to_contribute = balance!(800);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::contribute(
            RuntimeOrigin::signed(CHARLES),
            CERES_ASSET_ID.into(),
            funds_to_contribute
        ),);

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 11);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::finish_ilo(
            RuntimeOrigin::signed(ALICE),
            CERES_ASSET_ID.into()
        ),);

        let mut ilo_info = pallet::ILOs::<Runtime>::get(&CERES_ASSET_ID).unwrap();

        let unlocking_timestamp = ilo_info
            .finish_timestamp
            .saturating_add(86_400_000u64.saturating_mul(ilo_info.lockup_days.into()));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(unlocking_timestamp + 1);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::claim_lp_tokens(
            RuntimeOrigin::signed(ALICE),
            CERES_ASSET_ID
        ));

        ilo_info = pallet::ILOs::<Runtime>::get(&CERES_ASSET_ID).unwrap();

        let pallet_account = PalletId(*b"crslaunc").into_account_truncating();
        let pool_account = pool_xyk::Pallet::<Runtime>::properties_of_pool(
            base_asset.into(),
            CERES_ASSET_ID.into(),
        )
        .expect("Pool doesn't exist")
        .0;
        let lp_tokens =
            pool_xyk::Pallet::<Runtime>::balance_of_pool_provider(pool_account, pallet_account)
                .unwrap_or(0);

        assert_eq!(lp_tokens, balance!(0));

        let lp_tokens_alice =
            pool_xyk::Pallet::<Runtime>::balance_of_pool_provider(pool_account, ALICE).unwrap_or(0);

        assert_eq!(lp_tokens_alice, ilo_info.lp_tokens);
        assert_eq!(ilo_info.claimed_lp_tokens, true);
    });
}

#[test]
fn claim_lp_tokens_base_asset_xstusd_ok() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XSTUSD;
        assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
            RuntimeOrigin::signed(ALICE),
            base_asset.into(),
            CERES_ASSET_ID.into(),
            balance!(7693),
            balance!(3000),
            balance!(0.13),
            balance!(600),
            balance!(1000),
            balance!(0.2),
            balance!(850),
            true,
            balance!(0.75),
            balance!(0.25),
            31,
            current_timestamp + 5,
            current_timestamp + 10,
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2)
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 6);

        let funds_to_contribute = balance!(800);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::contribute(
            RuntimeOrigin::signed(CHARLES),
            CERES_ASSET_ID.into(),
            funds_to_contribute
        ),);

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 11);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::finish_ilo(
            RuntimeOrigin::signed(ALICE),
            CERES_ASSET_ID.into()
        ),);

        let mut ilo_info = pallet::ILOs::<Runtime>::get(&CERES_ASSET_ID).unwrap();

        let unlocking_timestamp = ilo_info
            .finish_timestamp
            .saturating_add(86_400_000u64.saturating_mul(ilo_info.lockup_days.into()));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(unlocking_timestamp + 1);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::claim_lp_tokens(
            RuntimeOrigin::signed(ALICE),
            CERES_ASSET_ID
        ));

        ilo_info = pallet::ILOs::<Runtime>::get(&CERES_ASSET_ID).unwrap();

        let pallet_account = PalletId(*b"crslaunc").into_account_truncating();
        let pool_account = pool_xyk::Pallet::<Runtime>::properties_of_pool(
            base_asset.into(),
            CERES_ASSET_ID.into(),
        )
        .expect("Pool doesn't exist")
        .0;
        let lp_tokens =
            pool_xyk::Pallet::<Runtime>::balance_of_pool_provider(pool_account, pallet_account)
                .unwrap_or(0);

        assert_eq!(lp_tokens, balance!(0));

        let lp_tokens_alice =
            pool_xyk::Pallet::<Runtime>::balance_of_pool_provider(pool_account, ALICE).unwrap_or(0);

        assert_eq!(lp_tokens_alice, ilo_info.lp_tokens);
        assert_eq!(ilo_info.claimed_lp_tokens, true);
    });
}

#[test]
fn claim_lp_tokens_cant_claim_lp_tokens_already_claimed() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
            RuntimeOrigin::signed(ALICE),
            base_asset.into(),
            CERES_ASSET_ID.into(),
            balance!(7693),
            balance!(3000),
            balance!(0.13),
            balance!(600),
            balance!(1000),
            balance!(0.2),
            balance!(850),
            true,
            balance!(0.75),
            balance!(0.25),
            31,
            current_timestamp + 5,
            current_timestamp + 10,
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2)
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 6);

        let funds_to_contribute = balance!(800);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::contribute(
            RuntimeOrigin::signed(CHARLES),
            CERES_ASSET_ID.into(),
            funds_to_contribute
        ),);

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 11);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::finish_ilo(
            RuntimeOrigin::signed(ALICE),
            CERES_ASSET_ID.into()
        ),);

        let ilo_info = pallet::ILOs::<Runtime>::get(&CERES_ASSET_ID).unwrap();

        let unlocking_timestamp = ilo_info
            .finish_timestamp
            .saturating_add(86_400_000u64.saturating_mul(ilo_info.lockup_days.into()));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(unlocking_timestamp + 1);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::claim_lp_tokens(
            RuntimeOrigin::signed(ALICE),
            CERES_ASSET_ID
        ));

        assert_err!(
            CeresLaunchpadPallet::<Runtime>::claim_lp_tokens(
                RuntimeOrigin::signed(ALICE),
                CERES_ASSET_ID
            ),
            Error::<Runtime>::CantClaimLPTokens
        );
    });
}

#[test]
fn claim_pswap_rewards_unauthorized() {
    preset_initial(|| {
        assert_err!(
            CeresLaunchpadPallet::<Runtime>::claim_pswap_rewards(RuntimeOrigin::signed(ALICE)),
            Error::<Runtime>::Unauthorized
        );
    });
}

#[test]
fn claim_pswap_rewards_ok() {
    preset_initial(|| {
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
            RuntimeOrigin::signed(ALICE),
            base_asset.into(),
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
            current_timestamp + 5,
            current_timestamp + 10,
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2)
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 6);

        let funds_to_contribute = balance!(1000);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::contribute(
            RuntimeOrigin::signed(CHARLES),
            CERES_ASSET_ID.into(),
            funds_to_contribute
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 11);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::finish_ilo(
            RuntimeOrigin::signed(ALICE),
            CERES_ASSET_ID.into()
        ));

        run_to_block(20000);

        let pallet_account = PalletId(*b"crslaunc").into_account_truncating();
        let share = FixedWrapper::from(1.00).get().unwrap();
        ShareholderAccounts::<Runtime>::mutate(&pallet_account, |current| {
            *current = current.saturating_add(share)
        });
        ClaimableShares::<Runtime>::mutate(|current| *current = current.saturating_add(share));

        assert_ok!(CeresLaunchpadPallet::<Runtime>::claim_pswap_rewards(
            RuntimeOrigin::signed(pallet::AuthorityAccount::<Runtime>::get())
        ));

        assert_eq!(
            Assets::free_balance(&PSWAP, &pallet_account).expect("Failed to query free balance."),
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
        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let base_asset = XOR;
        assert_ok!(CeresLaunchpadPallet::<Runtime>::create_ilo(
            RuntimeOrigin::signed(ALICE),
            base_asset.into(),
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
            current_timestamp + 5,
            current_timestamp + 10,
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2),
            balance!(0.2),
            current_timestamp + 3,
            balance!(0.2)
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 6);

        let funds_to_contribute = balance!(1000);

        assert_ok!(CeresLaunchpadPallet::<Runtime>::contribute(
            RuntimeOrigin::signed(CHARLES),
            CERES_ASSET_ID.into(),
            funds_to_contribute
        ));

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(current_timestamp + 15 * 86_400_000u64);
        run_to_block(300000);

        let ilo_info = pallet::ILOs::<Runtime>::get(&CERES_ASSET_ID).unwrap();
        let pallet_account = PalletId(*b"crslaunc").into_account_truncating();
        assert_eq!(
            Assets::free_balance(&CERES_ASSET_ID, &pallet_account)
                .expect("Failed to query free balance."),
            balance!(0)
        );

        assert_eq!(
            Assets::free_balance(&CERES_ASSET_ID, &ALICE).expect("Failed to query free balance."),
            balance!(15990)
        );

        assert_eq!(ilo_info.failed, true);
    });
}

#[test]
fn change_ceres_contribution_fee_unauthorized() {
    preset_initial(|| {
        assert_err!(
            CeresLaunchpadPallet::<Runtime>::change_ceres_contribution_fee(
                RuntimeOrigin::signed(ALICE),
                balance!(100)
            ),
            Error::<Runtime>::Unauthorized
        );
    });
}

#[test]
fn change_ceres_contribution_fee_ok() {
    preset_initial(|| {
        assert_ok!(
            CeresLaunchpadPallet::<Runtime>::change_ceres_contribution_fee(
                RuntimeOrigin::signed(pallet::AuthorityAccount::<Runtime>::get()),
                balance!(100)
            )
        );

        assert_eq!(
            pallet::CeresForContributionInILO::<Runtime>::get(),
            balance!(100)
        );
    });
}

#[test]
fn add_whitelisted_contributor_unauthorized() {
    preset_initial(|| {
        assert_err!(
            CeresLaunchpadPallet::<Runtime>::add_whitelisted_contributor(
                RuntimeOrigin::signed(ALICE),
                EMILY
            ),
            Error::<Runtime>::Unauthorized
        );
    });
}

#[test]
fn add_whitelisted_contributor_ok() {
    preset_initial(|| {
        assert_ok!(
            CeresLaunchpadPallet::<Runtime>::add_whitelisted_contributor(
                RuntimeOrigin::signed(pallet::AuthorityAccount::<Runtime>::get()),
                EMILY
            )
        );

        assert_eq!(
            pallet::WhitelistedContributors::<Runtime>::get().contains(&EMILY),
            true
        );
    });
}

#[test]
fn remove_whitelisted_contributor_unauthorized() {
    preset_initial(|| {
        assert_err!(
            CeresLaunchpadPallet::<Runtime>::remove_whitelisted_contributor(
                RuntimeOrigin::signed(ALICE),
                EMILY
            ),
            Error::<Runtime>::Unauthorized
        );
    });
}

#[test]
fn remove_whitelisted_contributor_ok() {
    preset_initial(|| {
        assert_ok!(
            CeresLaunchpadPallet::<Runtime>::remove_whitelisted_contributor(
                RuntimeOrigin::signed(pallet::AuthorityAccount::<Runtime>::get()),
                ALICE
            )
        );

        assert_eq!(
            pallet::WhitelistedContributors::<Runtime>::get().contains(&BOB),
            true
        );

        assert_err!(
            CeresLaunchpadPallet::<Runtime>::contribute(
                RuntimeOrigin::signed(ALICE),
                CERES_ASSET_ID.into(),
                balance!(0.21)
            ),
            Error::<Runtime>::AccountIsNotWhitelisted
        );
    });
}

#[test]
fn add_whitelisted_ilo_organizer_unauthorized() {
    preset_initial(|| {
        assert_err!(
            CeresLaunchpadPallet::<Runtime>::add_whitelisted_ilo_organizer(
                RuntimeOrigin::signed(ALICE),
                DAN
            ),
            Error::<Runtime>::Unauthorized
        );
    });
}

#[test]
fn add_whitelisted_ilo_organizer_ok() {
    preset_initial(|| {
        assert_ok!(
            CeresLaunchpadPallet::<Runtime>::add_whitelisted_ilo_organizer(
                RuntimeOrigin::signed(pallet::AuthorityAccount::<Runtime>::get()),
                DAN
            )
        );

        assert_eq!(
            pallet::WhitelistedIloOrganizers::<Runtime>::get().contains(&DAN),
            true
        );
    });
}

#[test]
fn remove_whitelisted_ilo_organizer_unauthorized() {
    preset_initial(|| {
        assert_err!(
            CeresLaunchpadPallet::<Runtime>::remove_whitelisted_ilo_organizer(
                RuntimeOrigin::signed(ALICE),
                EMILY
            ),
            Error::<Runtime>::Unauthorized
        );
    });
}

#[test]
fn remove_whitelisted_ilo_organizer_ok() {
    preset_initial(|| {
        let base_asset = XOR;
        assert_ok!(
            CeresLaunchpadPallet::<Runtime>::remove_whitelisted_ilo_organizer(
                RuntimeOrigin::signed(pallet::AuthorityAccount::<Runtime>::get()),
                ALICE
            )
        );

        assert_eq!(
            pallet::WhitelistedIloOrganizers::<Runtime>::get().contains(&BOB),
            true
        );

        let current_timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        assert_err!(
            CeresLaunchpadPallet::<Runtime>::create_ilo(
                RuntimeOrigin::signed(ALICE),
                base_asset.into(),
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
                current_timestamp + 5,
                current_timestamp + 10,
                balance!(1000),
                balance!(0.2),
                current_timestamp + 3,
                balance!(0.2),
                balance!(0.2),
                current_timestamp + 3,
                balance!(0.2)
            ),
            Error::<Runtime>::AccountIsNotWhitelisted
        );
    });
}
