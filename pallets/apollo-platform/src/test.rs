mod test {
    use crate::mock::*;
    use crate::{pallet, Error};
    use common::prelude::FixedWrapper;
    use common::{balance, CERES_ASSET_ID, HERMES_ASSET_ID, XOR};
    use frame_support::{assert_err, assert_ok};

    #[test]
    fn add_pool_unathorized_user() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let user = RuntimeOrigin::signed(ALICE);
            let loan_to_value = balance!(1);
            let liquidation_threshold = balance!(1);
            let optimal_utilization_rate = balance!(1);
            let base_rate = balance!(1);
            let slope_rate_1 = balance!(1);
            let slope_rate_2 = balance!(1);
            let reserve_factor = balance!(1);

            assert_err!(
                ApolloPlatform::add_pool(
                    user,
                    CERES_ASSET_ID,
                    loan_to_value,
                    liquidation_threshold,
                    optimal_utilization_rate,
                    base_rate,
                    slope_rate_1,
                    slope_rate_2,
                    reserve_factor
                ),
                Error::<Runtime>::Unauthorized
            );
        });
    }

    #[test]
    fn add_pool_invalid_pool_parameters() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let user = RuntimeOrigin::signed(ApolloPlatform::authority_account());
            let loan_to_value = balance!(1.1);
            let liquidation_threshold = balance!(1);
            let optimal_utilization_rate = balance!(1);
            let base_rate = balance!(1);
            let slope_rate_1 = balance!(1);
            let slope_rate_2 = balance!(1);
            let reserve_factor = balance!(1);

            assert_err!(
                ApolloPlatform::add_pool(
                    user,
                    CERES_ASSET_ID,
                    loan_to_value,
                    liquidation_threshold,
                    optimal_utilization_rate,
                    base_rate,
                    slope_rate_1,
                    slope_rate_2,
                    reserve_factor
                ),
                Error::<Runtime>::InvalidPoolParameters
            );
        });
    }

    #[test]
    fn add_pool_asset_already_listed() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let user = RuntimeOrigin::signed(ApolloPlatform::authority_account());
            let loan_to_value = balance!(1);
            let liquidation_threshold = balance!(1);
            let optimal_utilization_rate = balance!(1);
            let base_rate = balance!(1);
            let slope_rate_1 = balance!(1);
            let slope_rate_2 = balance!(1);
            let reserve_factor = balance!(1);

            assert_ok!(ApolloPlatform::add_pool(
                user.clone(),
                CERES_ASSET_ID,
                loan_to_value,
                liquidation_threshold,
                optimal_utilization_rate,
                base_rate,
                slope_rate_1,
                slope_rate_2,
                reserve_factor,
            ));

            assert_err!(
                ApolloPlatform::add_pool(
                    user,
                    CERES_ASSET_ID,
                    loan_to_value,
                    liquidation_threshold,
                    optimal_utilization_rate,
                    base_rate,
                    slope_rate_1,
                    slope_rate_2,
                    reserve_factor
                ),
                Error::<Runtime>::AssetAlreadyListed
            );
        });
    }

    #[test]
    fn add_pool_base_rate_check() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let user = RuntimeOrigin::signed(ApolloPlatform::authority_account());
            let loan_to_value = balance!(1);
            let liquidation_threshold = balance!(1);
            let optimal_utilization_rate = balance!(1);
            let base_rate = balance!(1);
            let slope_rate_1 = balance!(1);
            let slope_rate_2 = balance!(1);
            let reserve_factor = balance!(1);

            assert_ok!(ApolloPlatform::add_pool(
                user.clone(),
                CERES_ASSET_ID,
                loan_to_value,
                liquidation_threshold,
                optimal_utilization_rate,
                base_rate,
                slope_rate_1,
                slope_rate_2,
                reserve_factor,
            ));

            assert_ok!(ApolloPlatform::add_pool(
                user.clone(),
                HERMES_ASSET_ID,
                loan_to_value,
                liquidation_threshold,
                optimal_utilization_rate,
                base_rate,
                slope_rate_1,
                slope_rate_2,
                reserve_factor,
            ));

            assert_ok!(ApolloPlatform::add_pool(
                user.clone(),
                XOR,
                loan_to_value,
                liquidation_threshold,
                optimal_utilization_rate,
                base_rate,
                slope_rate_1,
                slope_rate_2,
                reserve_factor,
            ));

            let new_basic_lending_rate =
                (FixedWrapper::from(ApolloPlatform::lending_rewards_per_block())
                    / FixedWrapper::from(3))
                .try_into_balance()
                .unwrap_or(0);

            for (asset_id, pool_info) in pallet::PoolData::<Runtime>::iter() {
                assert_eq!(pool_info.basic_lending_rate, new_basic_lending_rate);
            }
        });
    }

    #[test]
    fn add_pool_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let user = RuntimeOrigin::signed(ApolloPlatform::authority_account());
            let loan_to_value = balance!(1);
            let liquidation_threshold = balance!(1);
            let optimal_utilization_rate = balance!(1);
            let base_rate = balance!(1);
            let slope_rate_1 = balance!(1);
            let slope_rate_2 = balance!(1);
            let reserve_factor = balance!(1);

            assert_ok!(ApolloPlatform::add_pool(
                user,
                CERES_ASSET_ID,
                loan_to_value,
                liquidation_threshold,
                optimal_utilization_rate,
                base_rate,
                slope_rate_1,
                slope_rate_2,
                reserve_factor,
            ));
        });
    }
}
