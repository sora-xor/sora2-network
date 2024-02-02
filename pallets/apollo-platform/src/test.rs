mod test {
    use crate::mock::*;
    use crate::{pallet, Error};
    use common::prelude::FixedWrapper;
    use common::{
        balance, AssetInfoProvider, Balance, DEXId, DEXId::Polkaswap, DAI, DOT, KSM, XOR,
    };
    use frame_support::PalletId;
    use frame_support::{assert_err, assert_ok};
    use sp_runtime::traits::AccountIdConversion;

    fn get_pallet_account() -> AccountId {
        PalletId(*b"apollolb").into_account_truncating()
    }

    fn calculate_lending_earnings(
        user: AccountId,
        asset_id: AssetId,
        block_number: BlockNumber,
    ) -> (Balance, Balance) {
        let user_info = pallet::UserLendingInfo::<Runtime>::get(user, asset_id).unwrap();
        let pool_info = pallet::PoolData::<Runtime>::get(asset_id).unwrap();

        let total_lending_blocks: u128 = block_number.into();

        let share_in_pool = FixedWrapper::from(user_info.lending_amount)
            / FixedWrapper::from(pool_info.total_liquidity);

        // Rewards from initial APOLLO distribution
        let basic_reward_per_block =
            FixedWrapper::from(pool_info.basic_lending_rate) * share_in_pool.clone();

        // Rewards from profit made through repayments and liquidations
        let profit_reward_per_block =
            FixedWrapper::from(pool_info.profit_lending_rate) * share_in_pool.clone();

        // Return (basic_lending_interest, profit_lending_interest)
        (
            (basic_reward_per_block * FixedWrapper::from(total_lending_blocks))
                .try_into_balance()
                .unwrap_or(0),
            (profit_reward_per_block * FixedWrapper::from(total_lending_blocks))
                .try_into_balance()
                .unwrap_or(0),
        )
    }

    fn static_set_dex() {
        init_pool(Polkaswap, XOR, DAI);
        init_pool(Polkaswap, XOR, DOT);
        init_pool(Polkaswap, XOR, KSM);
        // assert_ok!(trading_pair::Pallet::<Runtime>::register(
        //     RuntimeOrigin::signed(CHARLES),
        //     Polkaswap,
        //     XOR,
        //     DAI
        // ));

        // assert_ok!(trading_pair::Pallet::<Runtime>::register(
        //     RuntimeOrigin::signed(CHARLES),
        //     Polkaswap,
        //     XOR,
        //     DOT
        // ));

        // assert_ok!(trading_pair::Pallet::<Runtime>::register(
        //     RuntimeOrigin::signed(CHARLES),
        //     Polkaswap,
        //     XOR,
        //     KSM
        // ));

        // assert_ok!(pool_xyk::Pallet::<Runtime>::initialize_pool(
        //     RuntimeOrigin::signed(CHARLES),
        //     Polkaswap,
        //     XOR,
        //     DAI,
        // ));

        // assert_ok!(assets::Pallet::<Runtime>::mint_to(
        //     &DAI,
        //     &ALICE,
        //     &CHARLES,
        //     balance!(360000)
        // ));

        // assert_ok!(assets::Pallet::<Runtime>::mint_to(
        //     &XOR,
        //     &ALICE,
        //     &CHARLES,
        //     balance!(144000)
        // ));

        // assert_ok!(pool_xyk::Pallet::<Runtime>::deposit_liquidity(
        //     RuntimeOrigin::signed(CHARLES),
        //     Polkaswap,
        //     DAI,
        //     XOR,
        //     balance!(360000),
        //     balance!(144000),
        //     balance!(360000),
        //     balance!(144000),
        // ));
    }

    fn init_pool(dex_id: DEXId, base_asset: AssetId, other_asset: AssetId) {
        assert_ok!(trading_pair::Pallet::<Runtime>::register(
            RuntimeOrigin::signed(ALICE),
            dex_id,
            base_asset,
            other_asset
        ));

        assert_ok!(pool_xyk::Pallet::<Runtime>::initialize_pool(
            RuntimeOrigin::signed(ALICE),
            dex_id,
            base_asset,
            other_asset,
        ));
    }

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
                    XOR,
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
                    XOR,
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
                XOR,
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
                    XOR,
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
                XOR,
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
                DOT,
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
                KSM,
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

            for (_asset_id, pool_info) in pallet::PoolData::<Runtime>::iter() {
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
                XOR,
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

    #[test]
    fn lend_pool_does_not_exist() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let user = RuntimeOrigin::signed(ALICE);
            let lending_amount = balance!(100);

            assert_err!(
                ApolloPlatform::lend(user, XOR, lending_amount),
                Error::<Runtime>::PoolDoesNotExist
            );
        });
    }

    #[test]
    fn lend_can_not_transfer_lending_amount() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let pool_creator = RuntimeOrigin::signed(ApolloPlatform::authority_account());
            let loan_to_value = balance!(1);
            let liquidation_threshold = balance!(1);
            let optimal_utilization_rate = balance!(1);
            let base_rate = balance!(1);
            let slope_rate_1 = balance!(1);
            let slope_rate_2 = balance!(1);
            let reserve_factor = balance!(1);

            assert_ok!(ApolloPlatform::add_pool(
                pool_creator,
                XOR,
                loan_to_value,
                liquidation_threshold,
                optimal_utilization_rate,
                base_rate,
                slope_rate_1,
                slope_rate_2,
                reserve_factor,
            ));

            assert_err!(
                ApolloPlatform::lend(RuntimeOrigin::signed(ALICE), XOR, balance!(100000),),
                Error::<Runtime>::CanNotTransferLendingAmount
            );
        });
    }

    #[test]
    fn lend_new_user_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &XOR,
                &ALICE,
                &ALICE,
                balance!(300000)
            ));

            let pool_creator = RuntimeOrigin::signed(ApolloPlatform::authority_account());
            let loan_to_value = balance!(1);
            let liquidation_threshold = balance!(1);
            let optimal_utilization_rate = balance!(1);
            let base_rate = balance!(1);
            let slope_rate_1 = balance!(1);
            let slope_rate_2 = balance!(1);
            let reserve_factor = balance!(1);

            assert_ok!(ApolloPlatform::add_pool(
                pool_creator,
                XOR,
                loan_to_value,
                liquidation_threshold,
                optimal_utilization_rate,
                base_rate,
                slope_rate_1,
                slope_rate_2,
                reserve_factor,
            ));

            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR, &ALICE).unwrap(),
                balance!(300000)
            );

            assert_ok!(ApolloPlatform::lend(
                RuntimeOrigin::signed(ALICE),
                XOR,
                balance!(100000),
            ));

            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR.into(), &ALICE).unwrap(),
                balance!(200000)
            );

            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR.into(), &get_pallet_account())
                    .unwrap(),
                balance!(100000)
            );

            let lending_user_info = pallet::UserLendingInfo::<Runtime>::get(ALICE, XOR).unwrap();
            let pool_info = pallet::PoolData::<Runtime>::get(XOR).unwrap();

            assert_eq!(lending_user_info.last_lending_block, 0);
            assert_eq!(lending_user_info.lending_amount, balance!(100000));
            assert_eq!(lending_user_info.lending_interest, balance!(0));

            assert_eq!(pool_info.total_liquidity, balance!(100000));
        });
    }

    #[test]
    fn lend_old_user_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &XOR,
                &ALICE,
                &ALICE,
                balance!(300000)
            ));

            let pool_creator = RuntimeOrigin::signed(ApolloPlatform::authority_account());
            let loan_to_value = balance!(1);
            let liquidation_threshold = balance!(1);
            let optimal_utilization_rate = balance!(1);
            let base_rate = balance!(1);
            let slope_rate_1 = balance!(1);
            let slope_rate_2 = balance!(1);
            let reserve_factor = balance!(1);

            assert_ok!(ApolloPlatform::add_pool(
                pool_creator,
                XOR,
                loan_to_value,
                liquidation_threshold,
                optimal_utilization_rate,
                base_rate,
                slope_rate_1,
                slope_rate_2,
                reserve_factor,
            ));

            assert_ok!(ApolloPlatform::lend(
                RuntimeOrigin::signed(ALICE),
                XOR,
                balance!(100000)
            ));

            run_to_block(100);

            let lending_user_info = pallet::UserLendingInfo::<Runtime>::get(ALICE, XOR).unwrap();

            assert_eq!(lending_user_info.last_lending_block, 0);
            assert_eq!(lending_user_info.lending_amount, balance!(100000));
            assert_eq!(lending_user_info.lending_interest, balance!(0));

            assert_ok!(ApolloPlatform::lend(
                RuntimeOrigin::signed(ALICE),
                XOR,
                balance!(100000)
            ));

            let lending_user_info = pallet::UserLendingInfo::<Runtime>::get(ALICE, XOR).unwrap();
            let pool_info = pallet::PoolData::<Runtime>::get(XOR).unwrap();

            let lending_interest_gains = calculate_lending_earnings(ALICE, XOR, 100);
            let lending_interest_gain = lending_interest_gains.0 + lending_interest_gains.1;

            assert_eq!(lending_user_info.last_lending_block, 100);
            assert_eq!(lending_user_info.lending_amount, balance!(200000));
            assert_eq!(lending_user_info.lending_interest, lending_interest_gain);

            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR, &ALICE).unwrap(),
                balance!(100000)
            );

            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR.into(), &get_pallet_account())
                    .unwrap(),
                balance!(200000)
            );

            assert_eq!(pool_info.total_liquidity, balance!(200000));
        });
    }

    #[test]
    fn borrow_borrow_pool_does_not_exist() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let collateral_asset = XOR;
            let borrowing_asset = DOT;
            let borrowing_amount = balance!(100);

            assert_err!(
                ApolloPlatform::borrow(
                    RuntimeOrigin::signed(ALICE),
                    collateral_asset,
                    borrowing_asset,
                    borrowing_amount
                ),
                Error::<Runtime>::PoolDoesNotExist
            );
        });
    }

    #[test]
    fn borrow_no_liquidity_for_borrowing_asset() {
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
                XOR,
                loan_to_value,
                liquidation_threshold,
                optimal_utilization_rate,
                base_rate,
                slope_rate_1,
                slope_rate_2,
                reserve_factor,
            ));

            let collateral_asset = DOT;
            let borrowing_asset = XOR;
            let borrowing_amount = balance!(100);

            assert_err!(
                ApolloPlatform::borrow(
                    RuntimeOrigin::signed(ALICE),
                    collateral_asset,
                    borrowing_asset,
                    borrowing_amount
                ),
                Error::<Runtime>::NoLiquidityForBorrowingAsset
            );
        });
    }

    #[test]
    fn borrow_collateral_pool_does_not_exist() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &XOR,
                &ALICE,
                &BOB,
                balance!(300000)
            ));

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
                XOR,
                loan_to_value,
                liquidation_threshold,
                optimal_utilization_rate,
                base_rate,
                slope_rate_1,
                slope_rate_2,
                reserve_factor,
            ));

            assert_ok!(ApolloPlatform::lend(
                RuntimeOrigin::signed(BOB),
                XOR,
                balance!(300000),
            ));

            let collateral_asset = DOT;
            let borrowing_asset = XOR;
            let borrowing_amount = balance!(100);

            assert_err!(
                ApolloPlatform::borrow(
                    RuntimeOrigin::signed(ALICE),
                    collateral_asset,
                    borrowing_asset,
                    borrowing_amount
                ),
                Error::<Runtime>::PoolDoesNotExist
            );
        });
    }

    #[test]
    fn borrow_nothing_lended() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &XOR,
                &ALICE,
                &BOB,
                balance!(300000)
            ));

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
                XOR,
                loan_to_value,
                liquidation_threshold,
                optimal_utilization_rate,
                base_rate,
                slope_rate_1,
                slope_rate_2,
                reserve_factor,
            ));

            assert_ok!(ApolloPlatform::add_pool(
                user,
                DOT,
                loan_to_value,
                liquidation_threshold,
                optimal_utilization_rate,
                base_rate,
                slope_rate_1,
                slope_rate_2,
                reserve_factor,
            ));

            assert_ok!(ApolloPlatform::lend(
                RuntimeOrigin::signed(BOB),
                XOR,
                balance!(300000),
            ));

            let collateral_asset = DOT;
            let borrowing_asset = XOR;
            let borrowing_amount = balance!(100);

            assert_err!(
                ApolloPlatform::borrow(
                    RuntimeOrigin::signed(ALICE),
                    collateral_asset,
                    borrowing_asset,
                    borrowing_amount
                ),
                Error::<Runtime>::NothingLended
            );
        });
    }

    #[test]
    fn borrow_invalid_collateral_amount() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            static_set_dex();

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &DOT,
                &ALICE,
                &ALICE,
                balance!(200)
            ));

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &XOR,
                &ALICE,
                &BOB,
                balance!(300000)
            ));

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
                XOR,
                loan_to_value,
                liquidation_threshold,
                optimal_utilization_rate,
                base_rate,
                slope_rate_1,
                slope_rate_2,
                reserve_factor,
            ));

            assert_ok!(ApolloPlatform::add_pool(
                user,
                DOT,
                loan_to_value,
                liquidation_threshold,
                optimal_utilization_rate,
                base_rate,
                slope_rate_1,
                slope_rate_2,
                reserve_factor,
            ));

            assert_ok!(ApolloPlatform::lend(
                RuntimeOrigin::signed(ALICE),
                DOT,
                balance!(99),
            ));

            assert_ok!(ApolloPlatform::lend(
                RuntimeOrigin::signed(BOB),
                XOR,
                balance!(300000),
            ));

            assert_ok!(ApolloPlatform::borrow(
                RuntimeOrigin::signed(ALICE),
                DOT,
                XOR,
                balance!(100)
            ));
        });
    }
}
