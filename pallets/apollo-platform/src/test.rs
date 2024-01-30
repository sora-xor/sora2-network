mod test {
    use crate::mock::*;
    use crate::Error;
    use common::{balance, CERES_ASSET_ID};
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
