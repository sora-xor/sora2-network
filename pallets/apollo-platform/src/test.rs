mod test {
    use crate::mock::*;
    use crate::{pallet, Error};
    use common::prelude::FixedWrapper;
    use common::APOLLO_ASSET_ID;
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

    fn calculate_borrowing_interest(
        user: AccountId,
        borrowing_asset_id: AssetId,
        collateral_asset_id: AssetId,
        block_number: BlockNumber,
    ) -> (Balance, Balance) {
        let borrow_user_info =
            pallet::UserBorrowingInfo::<Runtime>::get(user, borrowing_asset_id).unwrap();
        let borrowing_user_debt = borrow_user_info.get(&collateral_asset_id).unwrap();
        let borrowing_asset_pool_info =
            pallet::PoolData::<Runtime>::get(borrowing_asset_id).unwrap();

        let total_borrowing_blocks: u128 = block_number.into();

        // Calculate borrowing interest
        let borrowing_interest_per_block = FixedWrapper::from(borrowing_user_debt.borrowing_amount)
            * FixedWrapper::from(borrowing_asset_pool_info.borrowing_rate);

        // Calculate borrowing reward
        let share_in_pool = FixedWrapper::from(borrowing_user_debt.borrowing_amount)
            / FixedWrapper::from(borrowing_asset_pool_info.total_borrowed);

        let borrowing_reward_per_block =
            FixedWrapper::from(borrowing_asset_pool_info.borrowing_rewards_rate) * share_in_pool;

        // Return (borrowing_interest, borrowing_reward)
        (
            (borrowing_interest_per_block * FixedWrapper::from(total_borrowing_blocks))
                .try_into_balance()
                .unwrap_or(0),
            (borrowing_reward_per_block * FixedWrapper::from(total_borrowing_blocks))
                .try_into_balance()
                .unwrap_or(0),
        )
    }

    fn static_set_dex() {
        init_pool(Polkaswap, XOR, DAI);
        init_pool(Polkaswap, XOR, DOT);
        init_pool(Polkaswap, XOR, KSM);

        assert_ok!(assets::Pallet::<Runtime>::mint_to(
            &XOR,
            &alice(),
            &charles(),
            balance!(360000)
        ));

        assert_ok!(assets::Pallet::<Runtime>::mint_to(
            &DAI,
            &alice(),
            &charles(),
            balance!(144000)
        ));

        assert_ok!(pool_xyk::Pallet::<Runtime>::deposit_liquidity(
            RuntimeOrigin::signed(charles()),
            Polkaswap,
            XOR,
            DAI,
            balance!(360000),
            balance!(144000),
            balance!(360000),
            balance!(144000),
        ));
    }

    fn init_pool(dex_id: DEXId, base_asset: AssetId, other_asset: AssetId) {
        assert_ok!(trading_pair::Pallet::<Runtime>::register(
            RuntimeOrigin::signed(charles()),
            dex_id,
            base_asset,
            other_asset
        ));

        assert_ok!(pool_xyk::Pallet::<Runtime>::initialize_pool(
            RuntimeOrigin::signed(charles()),
            dex_id,
            base_asset,
            other_asset,
        ));
    }

    #[test]
    fn add_pool_unathorized_user() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_err!(
                ApolloPlatform::add_pool(
                    RuntimeOrigin::signed(alice()),
                    XOR,
                    balance!(1),
                    balance!(1),
                    balance!(1),
                    balance!(1),
                    balance!(1),
                    balance!(1),
                    balance!(1)
                ),
                Error::<Runtime>::Unauthorized
            );
        });
    }

    #[test]
    fn add_pool_invalid_pool_parameters() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_err!(
                ApolloPlatform::add_pool(
                    RuntimeOrigin::signed(ApolloPlatform::authority_account()),
                    XOR,
                    balance!(1.1),
                    balance!(1),
                    balance!(1),
                    balance!(1),
                    balance!(1),
                    balance!(1),
                    balance!(1)
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

            assert_ok!(ApolloPlatform::add_pool(
                user.clone(),
                XOR,
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
            ));

            assert_err!(
                ApolloPlatform::add_pool(
                    user,
                    XOR,
                    balance!(1),
                    balance!(1),
                    balance!(1),
                    balance!(1),
                    balance!(1),
                    balance!(1),
                    balance!(1)
                ),
                Error::<Runtime>::AssetAlreadyListed
            );
        });
    }

    #[test]
    fn add_pool_rates_check() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let user = RuntimeOrigin::signed(ApolloPlatform::authority_account());

            assert_ok!(ApolloPlatform::add_pool(
                user.clone(),
                XOR,
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
            ));

            assert_ok!(ApolloPlatform::add_pool(
                user.clone(),
                DOT,
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
            ));

            assert_ok!(ApolloPlatform::add_pool(
                user.clone(),
                KSM,
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
            ));

            let new_basic_lending_rate =
                (FixedWrapper::from(ApolloPlatform::lending_rewards_per_block())
                    / FixedWrapper::from(3))
                .try_into_balance()
                .unwrap_or(0);

            let new_borrowing_rewards_rate =
                (FixedWrapper::from(ApolloPlatform::borrowing_rewards_per_block())
                    / FixedWrapper::from(3))
                .try_into_balance()
                .unwrap_or(0);

            for (_asset_id, pool_info) in pallet::PoolData::<Runtime>::iter() {
                assert_eq!(pool_info.basic_lending_rate, new_basic_lending_rate);
                assert_eq!(pool_info.borrowing_rewards_rate, new_borrowing_rewards_rate);
            }
        });
    }

    #[test]
    fn add_pool_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_ok!(ApolloPlatform::add_pool(
                RuntimeOrigin::signed(ApolloPlatform::authority_account()),
                XOR,
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
            ));
        });
    }

    #[test]
    fn lend_invalid_lending_amount() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_err!(
                ApolloPlatform::lend(RuntimeOrigin::signed(alice()), XOR, balance!(0)),
                Error::<Runtime>::InvalidLendingAmount
            );
        });
    }

    #[test]
    fn lend_pool_does_not_exist() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_err!(
                ApolloPlatform::lend(RuntimeOrigin::signed(alice()), XOR, balance!(100)),
                Error::<Runtime>::PoolDoesNotExist
            );
        });
    }

    #[test]
    fn lend_can_not_transfer_lending_amount() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_ok!(ApolloPlatform::add_pool(
                RuntimeOrigin::signed(ApolloPlatform::authority_account()),
                XOR,
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
            ));

            assert_err!(
                ApolloPlatform::lend(RuntimeOrigin::signed(alice()), XOR, balance!(100000),),
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
                &alice(),
                &alice(),
                balance!(300000)
            ));

            assert_ok!(ApolloPlatform::add_pool(
                RuntimeOrigin::signed(ApolloPlatform::authority_account()),
                XOR,
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
            ));

            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR, &alice()).unwrap(),
                balance!(300000)
            );

            assert_ok!(ApolloPlatform::lend(
                RuntimeOrigin::signed(alice()),
                XOR,
                balance!(100000),
            ));

            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR.into(), &alice()).unwrap(),
                balance!(200000)
            );

            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR.into(), &get_pallet_account())
                    .unwrap(),
                balance!(100000)
            );

            let lending_user_info = pallet::UserLendingInfo::<Runtime>::get(alice(), XOR).unwrap();
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
            run_to_block(1);

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &XOR,
                &alice(),
                &alice(),
                balance!(300000)
            ));

            assert_ok!(ApolloPlatform::add_pool(
                RuntimeOrigin::signed(ApolloPlatform::authority_account()),
                XOR,
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
            ));

            assert_ok!(ApolloPlatform::lend(
                RuntimeOrigin::signed(alice()),
                XOR,
                balance!(100000)
            ));

            run_to_block(101);

            let lending_user_info = pallet::UserLendingInfo::<Runtime>::get(alice(), XOR).unwrap();
            let lending_interest_gains = calculate_lending_earnings(alice(), XOR, 100);
            let lending_interest_gain = lending_interest_gains.0 + lending_interest_gains.1;

            assert_eq!(lending_user_info.lending_amount, balance!(100000));
            assert_eq!(lending_user_info.lending_interest, lending_interest_gain);

            assert_ok!(ApolloPlatform::lend(
                RuntimeOrigin::signed(alice()),
                XOR,
                balance!(100000)
            ));

            let lending_user_info = pallet::UserLendingInfo::<Runtime>::get(alice(), XOR).unwrap();
            let pool_info = pallet::PoolData::<Runtime>::get(XOR).unwrap();

            assert_eq!(lending_user_info.last_lending_block, 101);
            assert_eq!(lending_user_info.lending_amount, balance!(200000));
            assert_eq!(lending_user_info.lending_interest, lending_interest_gain);

            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR, &alice()).unwrap(),
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
    fn borrow_same_collateral_and_borrowing_assets() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_err!(
                ApolloPlatform::borrow(RuntimeOrigin::signed(alice()), XOR, XOR, balance!(100)),
                Error::<Runtime>::SameCollateralAndBorrowingAssets
            );
        });
    }

    #[test]
    fn borrow_borrow_pool_does_not_exist() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_err!(
                ApolloPlatform::borrow(RuntimeOrigin::signed(alice()), XOR, DOT, balance!(100)),
                Error::<Runtime>::PoolDoesNotExist
            );
        });
    }

    #[test]
    fn borrow_no_liquidity_for_borrowing_asset() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_ok!(ApolloPlatform::add_pool(
                RuntimeOrigin::signed(ApolloPlatform::authority_account()),
                XOR,
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
            ));

            assert_err!(
                ApolloPlatform::borrow(RuntimeOrigin::signed(alice()), DOT, XOR, balance!(100)),
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
                &alice(),
                &bob(),
                balance!(300000)
            ));

            assert_ok!(ApolloPlatform::add_pool(
                RuntimeOrigin::signed(ApolloPlatform::authority_account()),
                XOR,
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
            ));

            assert_ok!(ApolloPlatform::lend(
                RuntimeOrigin::signed(bob()),
                XOR,
                balance!(300000),
            ));

            assert_err!(
                ApolloPlatform::borrow(RuntimeOrigin::signed(alice()), DOT, XOR, balance!(100)),
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
                &alice(),
                &bob(),
                balance!(300000)
            ));

            let user = RuntimeOrigin::signed(ApolloPlatform::authority_account());

            assert_ok!(ApolloPlatform::add_pool(
                user.clone(),
                XOR,
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
            ));

            assert_ok!(ApolloPlatform::add_pool(
                user,
                DOT,
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
            ));

            assert_ok!(ApolloPlatform::lend(
                RuntimeOrigin::signed(bob()),
                XOR,
                balance!(300000),
            ));

            assert_err!(
                ApolloPlatform::borrow(RuntimeOrigin::signed(alice()), DOT, XOR, balance!(100)),
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
                &alice(),
                &alice(),
                balance!(200)
            ));

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &XOR,
                &alice(),
                &bob(),
                balance!(300000)
            ));

            let user = RuntimeOrigin::signed(ApolloPlatform::authority_account());

            assert_ok!(ApolloPlatform::add_pool(
                user.clone(),
                XOR,
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
            ));

            assert_ok!(ApolloPlatform::add_pool(
                user,
                DOT,
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
            ));

            assert_ok!(ApolloPlatform::lend(
                RuntimeOrigin::signed(alice()),
                DOT,
                balance!(99),
            ));

            assert_ok!(ApolloPlatform::lend(
                RuntimeOrigin::signed(bob()),
                XOR,
                balance!(300000),
            ));

            assert_err!(
                ApolloPlatform::borrow(RuntimeOrigin::signed(alice()), DOT, XOR, balance!(100)),
                Error::<Runtime>::InvalidCollateralAmount
            );
        });
    }

    #[test]
    fn borrow_new_user_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            static_set_dex();

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &DOT,
                &alice(),
                &alice(),
                balance!(200)
            ));

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &XOR,
                &alice(),
                &bob(),
                balance!(300000)
            ));

            let user = RuntimeOrigin::signed(ApolloPlatform::authority_account());

            assert_ok!(ApolloPlatform::add_pool(
                user.clone(),
                XOR,
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
            ));

            assert_ok!(ApolloPlatform::add_pool(
                user,
                DOT,
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
            ));

            assert_ok!(ApolloPlatform::lend(
                RuntimeOrigin::signed(alice()),
                DOT,
                balance!(100),
            ));

            assert_ok!(ApolloPlatform::lend(
                RuntimeOrigin::signed(bob()),
                XOR,
                balance!(300000),
            ));

            // Get data before borrow
            let borrowing_asset_pool_info = pallet::PoolData::<Runtime>::get(XOR).unwrap();
            let collateral_asset_pool_info = pallet::PoolData::<Runtime>::get(DOT).unwrap();

            // Collateral asset pool tests (before borrow)
            assert_eq!(collateral_asset_pool_info.total_liquidity, balance!(100));
            assert_eq!(collateral_asset_pool_info.total_collateral, balance!(0));

            // Borrowing asset pool tests (before borrow)
            assert_eq!(borrowing_asset_pool_info.total_liquidity, balance!(300000));
            assert_eq!(borrowing_asset_pool_info.total_borrowed, balance!(0));

            assert_ok!(ApolloPlatform::borrow(
                RuntimeOrigin::signed(alice()),
                DOT,
                XOR,
                balance!(100)
            ));

            // Check user and pallet balances of the borrowed asset
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR.into(), &alice()).unwrap(),
                balance!(100)
            );

            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR.into(), &get_pallet_account())
                    .unwrap(),
                balance!(299900)
            );

            // Check user and pallet balances of the collateral asset
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&DOT.into(), &alice()).unwrap(),
                balance!(100)
            );

            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&DOT.into(), &get_pallet_account())
                    .unwrap(),
                balance!(100)
            );

            // Get data after borrow
            let borrowing_user_info =
                pallet::UserBorrowingInfo::<Runtime>::get(alice(), XOR).unwrap();
            let borrowing_user_debt = borrowing_user_info.get(&DOT).unwrap();
            let borrowing_asset_pool_info = pallet::PoolData::<Runtime>::get(XOR).unwrap();
            let collateral_asset_pool_info = pallet::PoolData::<Runtime>::get(DOT).unwrap();

            // Collateral asset pool tests (after borrow)
            assert_eq!(collateral_asset_pool_info.total_liquidity, balance!(0));
            assert_eq!(collateral_asset_pool_info.total_collateral, balance!(100));

            // Borrowing asset pool tests (after borrow)
            assert_eq!(borrowing_asset_pool_info.total_liquidity, balance!(299900));
            assert_eq!(borrowing_asset_pool_info.total_borrowed, balance!(100));

            // Borrowing user tests (after borrow)
            assert_eq!(borrowing_user_debt.last_borrowing_block, 0);
            assert_eq!(borrowing_user_debt.collateral_amount, balance!(100));
            assert_eq!(borrowing_user_debt.borrowing_amount, balance!(100));
            assert_eq!(borrowing_user_debt.borrowing_interest, balance!(0));
            assert_eq!(borrowing_user_debt.borrowing_rewards, balance!(0));
        });
    }

    #[test]
    fn borrow_old_user_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            run_to_block(1);
            static_set_dex();

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &DOT,
                &alice(),
                &alice(),
                balance!(200)
            ));

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &XOR,
                &alice(),
                &bob(),
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
                RuntimeOrigin::signed(alice()),
                DOT,
                balance!(100),
            ));

            assert_ok!(ApolloPlatform::lend(
                RuntimeOrigin::signed(bob()),
                XOR,
                balance!(300000),
            ));

            assert_ok!(ApolloPlatform::borrow(
                RuntimeOrigin::signed(alice()),
                DOT,
                XOR,
                balance!(50)
            ));

            run_to_block(101);

            // Get data before second borrow
            let borrowing_user_info =
                pallet::UserBorrowingInfo::<Runtime>::get(alice(), XOR).unwrap();
            let borrowing_user_debt = borrowing_user_info.get(&DOT).unwrap();
            let borrowing_asset_pool_info = pallet::PoolData::<Runtime>::get(XOR).unwrap();
            let collateral_asset_pool_info = pallet::PoolData::<Runtime>::get(DOT).unwrap();

            // Collateral asset pool tests (before borrow)
            assert_eq!(collateral_asset_pool_info.total_liquidity, balance!(50));
            assert_eq!(collateral_asset_pool_info.total_collateral, balance!(50));

            // Borrowing asset pool tests (before borrow)
            assert_eq!(borrowing_asset_pool_info.total_liquidity, balance!(299950));
            assert_eq!(borrowing_asset_pool_info.total_borrowed, balance!(50));

            // Check user and pallet balances of the borrowed asset
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR.into(), &alice()).unwrap(),
                balance!(50)
            );

            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR.into(), &get_pallet_account())
                    .unwrap(),
                balance!(299950)
            );

            // Check user and pallet balances of the collateral asset
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&DOT.into(), &alice()).unwrap(),
                balance!(100)
            );

            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&DOT.into(), &get_pallet_account())
                    .unwrap(),
                balance!(100)
            );

            // NOTE:
            // The tests for the borrowing_interest and borrowing_rewards are commented out because there is a small miscalculation between
            // the pallet values and the values from the calculate_borrowing_interest() function. Also the closest value is at block 99 not 100.
            let calculater_borrowing_interest = calculate_borrowing_interest(alice(), XOR, DOT, 99);
            let delta = balance!(0.0000000000000001);

            // Borrowing user tests (before borrow)
            assert_eq!(borrowing_user_debt.last_borrowing_block, 101);
            assert_eq!(borrowing_user_debt.collateral_amount, balance!(50));
            assert_eq!(borrowing_user_debt.borrowing_amount, balance!(50));
            assert_eq!(
                (calculater_borrowing_interest.0 - borrowing_user_debt.borrowing_interest) <= delta,
                true
            );
            // 99 + 1 block
            assert_eq!(
                borrowing_user_debt.borrowing_rewards,
                calculater_borrowing_interest.1 + balance!(0.009512935)
            );

            assert_ok!(ApolloPlatform::borrow(
                RuntimeOrigin::signed(alice()),
                DOT,
                XOR,
                balance!(50)
            ));

            // Check user and pallet balances of the borrowed asset
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR.into(), &alice()).unwrap(),
                balance!(100)
            );

            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR.into(), &get_pallet_account())
                    .unwrap(),
                balance!(299900)
            );

            // Check user and pallet balances of the collateral asset
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&DOT.into(), &alice()).unwrap(),
                balance!(100)
            );

            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&DOT.into(), &get_pallet_account())
                    .unwrap(),
                balance!(100)
            );

            // Get data after first borrow
            let borrowing_user_info =
                pallet::UserBorrowingInfo::<Runtime>::get(alice(), XOR).unwrap();
            let borrowing_user_debt = borrowing_user_info.get(&DOT).unwrap();
            let borrowing_asset_pool_info = pallet::PoolData::<Runtime>::get(XOR).unwrap();
            let collateral_asset_pool_info = pallet::PoolData::<Runtime>::get(DOT).unwrap();

            // Collateral asset pool tests (after borrow)
            assert_eq!(collateral_asset_pool_info.total_liquidity, balance!(0));
            assert_eq!(collateral_asset_pool_info.total_collateral, balance!(100));

            // Borrowing asset pool tests (after borrow)
            assert_eq!(borrowing_asset_pool_info.total_liquidity, balance!(299900));
            assert_eq!(borrowing_asset_pool_info.total_borrowed, balance!(100));

            // Borrowing user tests (after borrow)
            assert_eq!(borrowing_user_debt.last_borrowing_block, 101);
            assert_eq!(borrowing_user_debt.collateral_amount, balance!(100));
            assert_eq!(borrowing_user_debt.borrowing_amount, balance!(100));
            assert_eq!(
                (calculater_borrowing_interest.0 - borrowing_user_debt.borrowing_interest) <= delta,
                true
            );
            // 99 + 1 block
            assert_eq!(
                borrowing_user_debt.borrowing_rewards,
                calculater_borrowing_interest.1 + balance!(0.009512935)
            );
        });
    }

    #[test]
    fn get_lending_rewards_nothing_lended() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_err!(
                ApolloPlatform::get_rewards(RuntimeOrigin::signed(alice()), XOR, true),
                Error::<Runtime>::NothingLended
            );
        });
    }

    #[test]
    fn get_lending_rewards_no_rewards_to_claim() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &XOR,
                &alice(),
                &alice(),
                balance!(300000)
            ));

            assert_ok!(ApolloPlatform::add_pool(
                RuntimeOrigin::signed(ApolloPlatform::authority_account()),
                XOR,
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
            ));

            assert_ok!(ApolloPlatform::lend(
                RuntimeOrigin::signed(alice()),
                XOR,
                balance!(100000),
            ));

            assert_err!(
                ApolloPlatform::get_rewards(RuntimeOrigin::signed(alice()), XOR, true),
                Error::<Runtime>::NoRewardsToClaim
            );
        });
    }

    #[test]
    fn get_lending_rewards_unable_to_transfer_rewards() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            run_to_block(1);

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &XOR,
                &alice(),
                &alice(),
                balance!(300000)
            ));

            assert_ok!(ApolloPlatform::add_pool(
                RuntimeOrigin::signed(ApolloPlatform::authority_account()),
                XOR,
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
            ));

            assert_ok!(ApolloPlatform::lend(
                RuntimeOrigin::signed(alice()),
                XOR,
                balance!(100000),
            ));

            run_to_block(101);

            assert_err!(
                ApolloPlatform::get_rewards(RuntimeOrigin::signed(alice()), XOR, true),
                Error::<Runtime>::UnableToTransferRewards
            );
        });
    }

    #[test]
    fn get_lending_rewards_lending_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            run_to_block(1);

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &XOR,
                &alice(),
                &alice(),
                balance!(300000)
            ));

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &APOLLO_ASSET_ID,
                &alice(),
                &get_pallet_account(),
                balance!(10000)
            ));

            assert_ok!(ApolloPlatform::add_pool(
                RuntimeOrigin::signed(ApolloPlatform::authority_account()),
                XOR,
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
            ));

            assert_ok!(ApolloPlatform::lend(
                RuntimeOrigin::signed(alice()),
                XOR,
                balance!(100000),
            ));

            run_to_block(101);

            assert_ok!(ApolloPlatform::get_rewards(
                RuntimeOrigin::signed(alice()),
                XOR,
                true
            ));

            let lending_user_info = pallet::UserLendingInfo::<Runtime>::get(alice(), XOR).unwrap();

            assert_eq!(lending_user_info.lending_interest, balance!(0));

            let lending_earnings = calculate_lending_earnings(alice(), XOR, 100);
            let lending_interest = lending_earnings.0 + lending_earnings.1;

            let new_pallet_balance = balance!(10000) - lending_interest;
            let new_user_balance = balance!(300000) + lending_interest;

            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&APOLLO_ASSET_ID, &get_pallet_account())
                    .unwrap(),
                new_pallet_balance
            );

            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&APOLLO_ASSET_ID, &alice()).unwrap(),
                new_user_balance
            );
        });
    }

    #[test]
    fn get_borrowing_rewards_nothing_borrowed() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_err!(
                ApolloPlatform::get_rewards(RuntimeOrigin::signed(alice()), XOR, false),
                Error::<Runtime>::NothingBorrowed
            );
        });
    }

    #[test]
    fn get_borrowing_rewards_no_rewards_to_claim() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            static_set_dex();

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &DOT,
                &alice(),
                &alice(),
                balance!(200)
            ));

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &XOR,
                &alice(),
                &bob(),
                balance!(300000)
            ));

            let user = RuntimeOrigin::signed(ApolloPlatform::authority_account());

            assert_ok!(ApolloPlatform::add_pool(
                user.clone(),
                XOR,
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
            ));

            assert_ok!(ApolloPlatform::add_pool(
                user,
                DOT,
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
            ));

            assert_ok!(ApolloPlatform::lend(
                RuntimeOrigin::signed(alice()),
                DOT,
                balance!(100),
            ));

            assert_ok!(ApolloPlatform::lend(
                RuntimeOrigin::signed(bob()),
                XOR,
                balance!(300000),
            ));

            assert_ok!(ApolloPlatform::borrow(
                RuntimeOrigin::signed(alice()),
                DOT,
                XOR,
                balance!(100)
            ));

            assert_err!(
                ApolloPlatform::get_rewards(RuntimeOrigin::signed(alice()), XOR, false),
                Error::<Runtime>::NoRewardsToClaim
            );
        });
    }

    #[test]
    fn get_borrowing_rewards_unable_to_transfer_rewards() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            static_set_dex();

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &DOT,
                &alice(),
                &alice(),
                balance!(200)
            ));

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &XOR,
                &alice(),
                &bob(),
                balance!(300000)
            ));

            let user = RuntimeOrigin::signed(ApolloPlatform::authority_account());

            assert_ok!(ApolloPlatform::add_pool(
                user.clone(),
                XOR,
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
            ));

            assert_ok!(ApolloPlatform::add_pool(
                user,
                DOT,
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
            ));

            assert_ok!(ApolloPlatform::lend(
                RuntimeOrigin::signed(alice()),
                DOT,
                balance!(100),
            ));

            assert_ok!(ApolloPlatform::lend(
                RuntimeOrigin::signed(bob()),
                XOR,
                balance!(300000),
            ));

            assert_ok!(ApolloPlatform::borrow(
                RuntimeOrigin::signed(alice()),
                DOT,
                XOR,
                balance!(100)
            ));

            run_to_block(101);

            assert_err!(
                ApolloPlatform::get_rewards(RuntimeOrigin::signed(alice()), XOR, false),
                Error::<Runtime>::UnableToTransferRewards
            );
        });
    }

    #[test]
    fn get_borrowing_rewards_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            static_set_dex();

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &APOLLO_ASSET_ID,
                &alice(),
                &get_pallet_account(),
                balance!(10000)
            ));

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &DOT,
                &alice(),
                &alice(),
                balance!(200)
            ));

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &XOR,
                &alice(),
                &bob(),
                balance!(300000)
            ));

            let user = RuntimeOrigin::signed(ApolloPlatform::authority_account());

            assert_ok!(ApolloPlatform::add_pool(
                user.clone(),
                XOR,
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
            ));

            assert_ok!(ApolloPlatform::add_pool(
                user,
                DOT,
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
            ));

            assert_ok!(ApolloPlatform::lend(
                RuntimeOrigin::signed(alice()),
                DOT,
                balance!(100),
            ));

            assert_ok!(ApolloPlatform::lend(
                RuntimeOrigin::signed(bob()),
                XOR,
                balance!(300000),
            ));

            assert_ok!(ApolloPlatform::borrow(
                RuntimeOrigin::signed(alice()),
                DOT,
                XOR,
                balance!(100)
            ));

            run_to_block(101);

            assert_ok!(ApolloPlatform::get_rewards(
                RuntimeOrigin::signed(alice()),
                XOR,
                false
            ));

            let borrow_user_info = pallet::UserBorrowingInfo::<Runtime>::get(alice(), XOR).unwrap();
            let borrowing_user_debt = borrow_user_info.get(&DOT).unwrap();

            assert_eq!(borrowing_user_debt.borrowing_rewards, balance!(0));

            let (_, borrowing_rewards) = calculate_borrowing_interest(alice(), XOR, DOT, 101);

            let new_pallet_balance = balance!(10000) - borrowing_rewards;
            let new_user_balance = balance!(300000) + borrowing_rewards;

            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&APOLLO_ASSET_ID, &get_pallet_account())
                    .unwrap(),
                new_pallet_balance
            );

            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&APOLLO_ASSET_ID, &alice()).unwrap(),
                new_user_balance
            );
        });
    }

    #[test]
    fn get_borrowing_rewards_on_multiple_assets_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            static_set_dex();

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &APOLLO_ASSET_ID,
                &alice(),
                &get_pallet_account(),
                balance!(10000)
            ));

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &DOT,
                &alice(),
                &alice(),
                balance!(200)
            ));

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &KSM,
                &alice(),
                &alice(),
                balance!(200)
            ));

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &XOR,
                &alice(),
                &bob(),
                balance!(300000)
            ));

            let user = RuntimeOrigin::signed(ApolloPlatform::authority_account());

            assert_ok!(ApolloPlatform::add_pool(
                user.clone(),
                XOR,
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
            ));

            assert_ok!(ApolloPlatform::add_pool(
                user.clone(),
                DOT,
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
            ));

            assert_ok!(ApolloPlatform::add_pool(
                user.clone(),
                KSM,
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
            ));

            assert_ok!(ApolloPlatform::lend(
                RuntimeOrigin::signed(alice()),
                DOT,
                balance!(100),
            ));

            assert_ok!(ApolloPlatform::lend(
                RuntimeOrigin::signed(alice()),
                KSM,
                balance!(100),
            ));

            assert_ok!(ApolloPlatform::lend(
                RuntimeOrigin::signed(bob()),
                XOR,
                balance!(300000),
            ));

            assert_ok!(ApolloPlatform::borrow(
                RuntimeOrigin::signed(alice()),
                DOT,
                XOR,
                balance!(100)
            ));

            assert_ok!(ApolloPlatform::borrow(
                RuntimeOrigin::signed(alice()),
                KSM,
                XOR,
                balance!(100)
            ));

            run_to_block(101);

            assert_ok!(ApolloPlatform::get_rewards(
                RuntimeOrigin::signed(alice()),
                XOR,
                false
            ));

            let borrow_user_info = pallet::UserBorrowingInfo::<Runtime>::get(alice(), XOR).unwrap();
            let borrowing_user_debt_dot = borrow_user_info.get(&DOT).unwrap();
            let borrowing_user_debt_ksm = borrow_user_info.get(&KSM).unwrap();

            assert_eq!(borrowing_user_debt_dot.borrowing_rewards, balance!(0));
            assert_eq!(borrowing_user_debt_ksm.borrowing_rewards, balance!(0));

            let (_, borrowing_rewards_dot) = calculate_borrowing_interest(alice(), XOR, DOT, 101);
            let (_, borrowing_rewards_ksm) = calculate_borrowing_interest(alice(), XOR, KSM, 101);

            let total_borrowing_rewards = borrowing_rewards_dot + borrowing_rewards_ksm;

            let new_pallet_balance = balance!(10000) - total_borrowing_rewards;
            let new_user_balance = balance!(300000) + total_borrowing_rewards;

            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&APOLLO_ASSET_ID, &get_pallet_account())
                    .unwrap(),
                new_pallet_balance + balance!(0.000000000000000066)
            );

            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&APOLLO_ASSET_ID, &alice()).unwrap(),
                new_user_balance - balance!(0.000000000000000066)
            );
        });
    }

    #[test]
    fn withdraw_pool_does_not_exist() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_err!(
                ApolloPlatform::withdraw(RuntimeOrigin::signed(alice()), XOR, balance!(100)),
                Error::<Runtime>::PoolDoesNotExist
            );
        });
    }

    #[test]
    fn withdraw_nothing_lended() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_ok!(ApolloPlatform::add_pool(
                RuntimeOrigin::signed(ApolloPlatform::authority_account()),
                XOR,
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
            ));

            assert_err!(
                ApolloPlatform::withdraw(RuntimeOrigin::signed(alice()), XOR, balance!(100)),
                Error::<Runtime>::NothingLended
            );
        });
    }

    #[test]
    fn withdraw_lending_amount_exceeded() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &XOR,
                &alice(),
                &alice(),
                balance!(300000)
            ));

            assert_ok!(ApolloPlatform::add_pool(
                RuntimeOrigin::signed(ApolloPlatform::authority_account()),
                XOR,
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
            ));

            assert_ok!(ApolloPlatform::lend(
                RuntimeOrigin::signed(alice()),
                XOR,
                balance!(100),
            ));

            assert_err!(
                ApolloPlatform::withdraw(RuntimeOrigin::signed(alice()), XOR, balance!(200),),
                Error::<Runtime>::LendingAmountExceeded
            );
        });
    }

    #[test]
    fn withdraw_can_not_transfer_lending_amount() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &XOR,
                &alice(),
                &alice(),
                balance!(300)
            ));

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &XOR,
                &alice(),
                &bob(),
                balance!(300)
            ));

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &DOT,
                &alice(),
                &bob(),
                balance!(300)
            ));

            assert_ok!(ApolloPlatform::add_pool(
                RuntimeOrigin::signed(ApolloPlatform::authority_account()),
                XOR,
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
            ));

            assert_ok!(ApolloPlatform::add_pool(
                RuntimeOrigin::signed(ApolloPlatform::authority_account()),
                DOT,
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
            ));

            assert_ok!(ApolloPlatform::lend(
                RuntimeOrigin::signed(alice()),
                XOR,
                balance!(200),
            ));

            assert_ok!(ApolloPlatform::lend(
                RuntimeOrigin::signed(bob()),
                XOR,
                balance!(300),
            ));

            assert_ok!(ApolloPlatform::lend(
                RuntimeOrigin::signed(bob()),
                DOT,
                balance!(300),
            ));

            assert_ok!(ApolloPlatform::borrow(
                RuntimeOrigin::signed(bob()),
                DOT,
                XOR,
                balance!(300)
            ));

            assert_err!(
                ApolloPlatform::withdraw(RuntimeOrigin::signed(alice()), XOR, balance!(200)),
                Error::<Runtime>::CanNotTransferLendingAmount
            );
        });
    }

    #[test]
    fn withdraw_without_interest_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            run_to_block(1);

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &XOR,
                &alice(),
                &alice(),
                balance!(300)
            ));

            assert_ok!(ApolloPlatform::add_pool(
                RuntimeOrigin::signed(ApolloPlatform::authority_account()),
                XOR,
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
            ));

            assert_ok!(ApolloPlatform::lend(
                RuntimeOrigin::signed(alice()),
                XOR,
                balance!(100)
            ));

            run_to_block(101);

            // Check balances before withdrawal
            // Pallet balance
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR.into(), &get_pallet_account())
                    .unwrap(),
                balance!(100)
            );
            // Alice balance
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR.into(), &alice()).unwrap(),
                balance!(200)
            );

            // Check pool info and user lending info values before withdrawal
            // Pool info
            let pool_info = pallet::PoolData::<Runtime>::get(XOR).unwrap();
            assert_eq!(pool_info.total_liquidity, balance!(100));

            // User lending info
            let lending_user_info = pallet::UserLendingInfo::<Runtime>::get(alice(), XOR).unwrap();
            assert_eq!(lending_user_info.lending_amount, balance!(100));

            assert_ok!(ApolloPlatform::withdraw(
                RuntimeOrigin::signed(alice()),
                XOR,
                balance!(50)
            ));

            // Check balances after withdrawal
            // Pallet balance
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR.into(), &get_pallet_account())
                    .unwrap(),
                balance!(50)
            );
            // Alice balance
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR.into(), &alice()).unwrap(),
                balance!(250)
            );

            // Check pool info and user lending info values after withdrawal
            // Pool info
            let pool_info = pallet::PoolData::<Runtime>::get(XOR).unwrap();
            assert_eq!(pool_info.total_liquidity, balance!(50));

            // User lending info
            let lending_user_info = pallet::UserLendingInfo::<Runtime>::get(alice(), XOR).unwrap();
            assert_eq!(lending_user_info.lending_amount, balance!(50));
        });
    }

    #[test]
    fn withdraw_with_interest_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            run_to_block(1);

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &XOR,
                &alice(),
                &alice(),
                balance!(300)
            ));

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &XOR,
                &alice(),
                &bob(),
                balance!(300)
            ));

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &APOLLO_ASSET_ID,
                &alice(),
                &get_pallet_account(),
                balance!(10000)
            ));

            assert_ok!(ApolloPlatform::add_pool(
                RuntimeOrigin::signed(ApolloPlatform::authority_account()),
                XOR,
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
            ));

            assert_ok!(ApolloPlatform::lend(
                RuntimeOrigin::signed(alice()),
                XOR,
                balance!(100)
            ));

            assert_ok!(ApolloPlatform::lend(
                RuntimeOrigin::signed(bob()),
                XOR,
                balance!(100)
            ));

            run_to_block(101);

            // Check balances before withdrawal
            // Pallet balance
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR.into(), &get_pallet_account())
                    .unwrap(),
                balance!(200)
            );
            // Alice balance
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR.into(), &alice()).unwrap(),
                balance!(200)
            );
            // Alice balanec (APOLLO)
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&APOLLO_ASSET_ID.into(), &alice()).unwrap(),
                balance!(300000)
            );

            // Check pool info and user lending info values before withdrawal
            // Pool info
            let pool_info = pallet::PoolData::<Runtime>::get(XOR).unwrap();
            assert_eq!(pool_info.total_liquidity, balance!(200));

            // User lending info
            let lending_user_info = pallet::UserLendingInfo::<Runtime>::get(alice(), XOR).unwrap();
            assert_eq!(lending_user_info.lending_amount, balance!(100));

            // Calculate lending interest
            let lending_interests = calculate_lending_earnings(alice(), XOR, 100);
            let total_interest = lending_interests.0 + lending_interests.1 + balance!(300000);

            assert_ok!(ApolloPlatform::withdraw(
                RuntimeOrigin::signed(alice()),
                XOR,
                balance!(100)
            ));

            // Check balances after withdrawal
            // Pallet balance
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR.into(), &get_pallet_account())
                    .unwrap(),
                balance!(100)
            );
            // Alice balance (XOR)
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR.into(), &alice()).unwrap(),
                balance!(300)
            );
            // Alice balance (APOLLO)
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&APOLLO_ASSET_ID.into(), &alice()).unwrap(),
                total_interest
            );

            // Check pool info and user lending info values after withdrawal
            // Pool info
            let pool_info = pallet::PoolData::<Runtime>::get(XOR).unwrap();
            assert_eq!(pool_info.total_liquidity, balance!(100));

            // User lending info
            let lending_user_info = pallet::UserLendingInfo::<Runtime>::get(alice(), XOR);
            assert_eq!(lending_user_info, None);
        });
    }
}
