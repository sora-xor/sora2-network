mod test {
    use crate::{mock::*, PoolInfo};
    use crate::{pallet, Error};
    use codec::Decode;
    use common::prelude::FixedWrapper;
    use common::APOLLO_ASSET_ID;
    use common::CERES_ASSET_ID;
    use common::{
        balance, AssetInfoProvider, Balance, DEXId, DEXId::Polkaswap, DAI, DOT, KSM, XOR,
    };
    use frame_support::PalletId;
    use frame_support::{assert_err, assert_ok};
    use hex_literal::hex;
    use sp_runtime::traits::AccountIdConversion;

    fn get_pallet_account() -> AccountId {
        PalletId(*b"apollolb").into_account_truncating()
    }

    fn get_authority_account() -> AccountId {
        let bytes = hex!("96ea3c9c0be7bbc7b0656a1983db5eed75210256891a9609012362e36815b132");
        AccountId::decode(&mut &bytes[..]).unwrap()
    }

    fn get_treasury_account() -> AccountId {
        let bytes = hex!("04621d8671f5ccc9c4dd86898e346f8ca163bc290ceee6bd2a8376a6d6dd8547");
        AccountId::decode(&mut &bytes[..]).unwrap()
    }

    fn calculate_lending_earnings(
        user: AccountId,
        asset_id: AssetId,
        block_number: BlockNumber,
    ) -> (Balance, Balance) {
        let user_info = pallet::UserLendingInfo::<Runtime>::get(asset_id, user).unwrap();
        let pool_info = pallet::PoolData::<Runtime>::get(asset_id).unwrap();

        let total_lending_blocks = balance!(block_number);

        let share_in_pool = FixedWrapper::from(user_info.lending_amount)
            / FixedWrapper::from(pool_info.total_liquidity);

        // Rewards from initial APOLLO distribution
        let basic_reward_per_block =
            FixedWrapper::from(pool_info.basic_lending_rate) * share_in_pool.clone();

        // Rewards from profit made through repayments and liquidations
        let profit_reward_per_block =
            FixedWrapper::from(pool_info.profit_lending_rate) * share_in_pool.clone();

        let basic_lending_interest = (basic_reward_per_block
            * FixedWrapper::from(total_lending_blocks))
        .try_into_balance()
        .unwrap_or(0);

        let profit_lending_interest = (profit_reward_per_block
            * FixedWrapper::from(total_lending_blocks))
        .try_into_balance()
        .unwrap_or(0);

        // Return (basic_lending_interest, profit_lending_interest)
        (basic_lending_interest, profit_lending_interest)
    }

    fn calculate_borrowing_interest(
        user: AccountId,
        borrowing_asset_id: AssetId,
        collateral_asset_id: AssetId,
        block_number: BlockNumber,
    ) -> (Balance, Balance) {
        let borrow_user_info =
            pallet::UserBorrowingInfo::<Runtime>::get(borrowing_asset_id, user).unwrap();
        let borrowing_user_debt = borrow_user_info.get(&collateral_asset_id).unwrap();
        let borrowing_asset_pool_info =
            pallet::PoolData::<Runtime>::get(borrowing_asset_id).unwrap();

        let total_borrowing_blocks = balance!(block_number);

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

    fn calculate_reserve_amounts(
        asset_id: AssetId,
        amount: Balance,
    ) -> (Balance, Balance, Balance) {
        let pool_info = pallet::PoolData::<Runtime>::get(asset_id).unwrap();

        // Calculate rewards and reserves amounts based on Reserve Factor
        let reserves_amount = (FixedWrapper::from(pool_info.reserve_factor)
            * FixedWrapper::from(amount))
        .try_into_balance()
        .unwrap_or(0);

        // Calculate reserve amounts (treasury, burn, developer)
        // Treasury reserve -> Apollo tokens
        // Burn (reserve) amount -> Ceres tokens
        // Developer (reserve) amount -> XOR tokens

        let treasury_reserve = (FixedWrapper::from(reserves_amount)
            * FixedWrapper::from(balance!(0.6)))
        .try_into_balance()
        .unwrap_or(0);

        let burn_reserve = (FixedWrapper::from(reserves_amount)
            * FixedWrapper::from(balance!(0.2)))
        .try_into_balance()
        .unwrap_or(0);

        let developer_reserve = (FixedWrapper::from(reserves_amount)
            * FixedWrapper::from(balance!(0.2)))
        .try_into_balance()
        .unwrap_or(0);

        (treasury_reserve, burn_reserve, developer_reserve)
    }

    fn calculate_rates(pool_info: &PoolInfo) -> (Balance, Balance) {
        let utilization_rate = (FixedWrapper::from(pool_info.total_borrowed)
            / (FixedWrapper::from(pool_info.total_borrowed)
                + FixedWrapper::from(pool_info.total_liquidity)))
        .try_into_balance()
        .unwrap_or(0);

        let mut profit_lending_rate: u128 = 0;
        let mut borrowing_rate: u128 = 0;

        if utilization_rate < pool_info.optimal_utilization_rate {
            // Update lending rate
            profit_lending_rate = (FixedWrapper::from(pool_info.rewards)
                / FixedWrapper::from(balance!(5256000)))
            .try_into_balance()
            .unwrap_or(0);

            // Update borrowing_rate -> Rt = (R0 + (U / Uopt) * Rslope1) / one_year
            borrowing_rate = ((FixedWrapper::from(pool_info.base_rate)
                + (FixedWrapper::from(utilization_rate)
                    / FixedWrapper::from(pool_info.optimal_utilization_rate))
                    * FixedWrapper::from(pool_info.slope_rate_1))
                / FixedWrapper::from(balance!(5256000)))
            .try_into_balance()
            .unwrap_or(0);
        } else {
            // Update lending rate
            profit_lending_rate = ((FixedWrapper::from(pool_info.rewards)
                / FixedWrapper::from(balance!(5256000)))
                * (FixedWrapper::from(balance!(1)) + FixedWrapper::from(utilization_rate)))
            .try_into_balance()
            .unwrap_or(0);

            // Update borrowing_rate -> Rt = (R0 + Rslope1 + ((Ut - Uopt) / (1 - Uopt)) * Rslope2) / one_year
            borrowing_rate = ((FixedWrapper::from(pool_info.base_rate)
                + FixedWrapper::from(pool_info.slope_rate_1)
                + ((FixedWrapper::from(utilization_rate)
                    - FixedWrapper::from(pool_info.optimal_utilization_rate))
                    / (FixedWrapper::from(balance!(1))
                        - FixedWrapper::from(pool_info.optimal_utilization_rate)))
                    * FixedWrapper::from(pool_info.slope_rate_2))
                / FixedWrapper::from(balance!(5256000)))
            .try_into_balance()
            .unwrap_or(0);
        }

        return (profit_lending_rate, borrowing_rate);
    }

    fn static_set_dex() {
        init_pool(Polkaswap, XOR, DAI);
        init_pool(Polkaswap, XOR, DOT);
        init_pool(Polkaswap, XOR, KSM);
        init_pool(Polkaswap, XOR, APOLLO_ASSET_ID);

        assert_ok!(assets::Pallet::<Runtime>::mint_to(
            &APOLLO_ASSET_ID,
            &alice(),
            &charles(),
            balance!(10000)
        ));

        assert_ok!(assets::Pallet::<Runtime>::mint_to(
            &XOR,
            &alice(),
            &charles(),
            balance!(450000)
        ));

        assert_ok!(pool_xyk::Pallet::<Runtime>::deposit_liquidity(
            RuntimeOrigin::signed(charles()),
            Polkaswap,
            XOR,
            APOLLO_ASSET_ID,
            balance!(450000),
            balance!(10000),
            balance!(450000),
            balance!(10000),
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

    fn init_exchange() {
        assert_ok!(assets::Pallet::<Runtime>::mint_to(
            &APOLLO_ASSET_ID,
            &alice(),
            &exchange_account(),
            balance!(100000)
        ));

        assert_ok!(assets::Pallet::<Runtime>::mint_to(
            &XOR,
            &alice(),
            &exchange_account(),
            balance!(100000)
        ));

        assert_ok!(assets::Pallet::<Runtime>::mint_to(
            &KSM,
            &alice(),
            &exchange_account(),
            balance!(100000)
        ));

        assert_ok!(assets::Pallet::<Runtime>::mint_to(
            &DOT,
            &alice(),
            &exchange_account(),
            balance!(100000)
        ));

        assert_ok!(assets::Pallet::<Runtime>::mint_to(
            &DAI,
            &alice(),
            &exchange_account(),
            balance!(100000)
        ));

        assert_ok!(assets::Pallet::<Runtime>::mint_to(
            &CERES_ASSET_ID,
            &alice(),
            &exchange_account(),
            balance!(1000)
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
                    / FixedWrapper::from(balance!(3)))
                .try_into_balance()
                .unwrap_or(0);

            let new_borrowing_rewards_rate =
                (FixedWrapper::from(ApolloPlatform::borrowing_rewards_per_block())
                    / FixedWrapper::from(balance!(3)))
                .try_into_balance()
                .unwrap_or(0);

            for (_asset_id, pool_info) in pallet::PoolData::<Runtime>::iter() {
                assert_eq!(pool_info.basic_lending_rate, new_basic_lending_rate);
                assert_eq!(pool_info.borrowing_rewards_rate, new_borrowing_rewards_rate);
            }

            let first_pool = pallet::PoolsByBlock::<Runtime>::get(0).unwrap();
            let second_pool = pallet::PoolsByBlock::<Runtime>::get(1).unwrap();
            let third_pool = pallet::PoolsByBlock::<Runtime>::get(2).unwrap();

            assert_eq!(first_pool, XOR);
            assert_eq!(second_pool, DOT);
            assert_eq!(third_pool, KSM);
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

            let lending_user_info = pallet::UserLendingInfo::<Runtime>::get(XOR, alice()).unwrap();
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

            run_to_block(151);

            let lending_user_info = pallet::UserLendingInfo::<Runtime>::get(XOR, alice()).unwrap();
            let lending_interest_gains = calculate_lending_earnings(alice(), XOR, 150);
            let lending_interest_gain = lending_interest_gains.0 + lending_interest_gains.1;

            assert_eq!(lending_user_info.lending_amount, balance!(100000));
            assert_eq!(lending_user_info.lending_interest, lending_interest_gain);

            assert_ok!(ApolloPlatform::lend(
                RuntimeOrigin::signed(alice()),
                XOR,
                balance!(100000)
            ));

            let lending_user_info = pallet::UserLendingInfo::<Runtime>::get(XOR, alice()).unwrap();
            let pool_info = pallet::PoolData::<Runtime>::get(XOR).unwrap();

            assert_eq!(lending_user_info.last_lending_block, 151);
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
                pallet::UserBorrowingInfo::<Runtime>::get(XOR, alice()).unwrap();
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

            run_to_block(151);

            // Get data before second borrow
            let borrowing_user_info =
                pallet::UserBorrowingInfo::<Runtime>::get(XOR, alice()).unwrap();
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

            // From 1st to 150th block, there are 149 blocks and minus 1st block when rates are 0 = 148
            let calculated_borrowing_interest =
                calculate_borrowing_interest(alice(), XOR, DOT, 148);

            // Borrowing user tests (before borrow)
            assert_eq!(borrowing_user_debt.last_borrowing_block, 150);
            assert_eq!(borrowing_user_debt.collateral_amount, balance!(50));
            assert_eq!(borrowing_user_debt.borrowing_amount, balance!(50));
            assert_eq!(
                borrowing_user_debt.borrowing_interest,
                calculated_borrowing_interest.0
            );

            // 148 + 1 block (there are rewards in the 1st block)
            assert_eq!(
                borrowing_user_debt.borrowing_rewards,
                calculated_borrowing_interest.1 + balance!(0.009512935)
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
                pallet::UserBorrowingInfo::<Runtime>::get(XOR, alice()).unwrap();
            let borrowing_user_debt = borrowing_user_info.get(&DOT).unwrap();
            let borrowing_asset_pool_info = pallet::PoolData::<Runtime>::get(XOR).unwrap();
            let collateral_asset_pool_info = pallet::PoolData::<Runtime>::get(DOT).unwrap();

            // Collateral asset pool tests (after borrow)
            assert_eq!(collateral_asset_pool_info.total_liquidity, balance!(0));
            assert_eq!(collateral_asset_pool_info.total_collateral, balance!(100));

            // Borrowing asset pool tests (after borrow)
            assert_eq!(borrowing_asset_pool_info.total_liquidity, balance!(299900));
            assert_eq!(borrowing_asset_pool_info.total_borrowed, balance!(100));

            // From 1st to 151st block, there are 150 blocks and minus 1st block when rates are 0 = 149
            // The result will be divided by 2, because borrowing_amount has been raised from 50 to 100
            let calculated_borrowing_interest =
                calculate_borrowing_interest(alice(), XOR, DOT, 149);

            // Borrowing user tests (after borrow)
            assert_eq!(borrowing_user_debt.last_borrowing_block, 151);
            assert_eq!(borrowing_user_debt.collateral_amount, balance!(100));
            assert_eq!(borrowing_user_debt.borrowing_amount, balance!(100));
            assert_eq!(
                calculated_borrowing_interest.0 / 2,
                borrowing_user_debt.borrowing_interest
            );

            assert_eq!(
                borrowing_user_debt.borrowing_rewards,
                calculated_borrowing_interest.1
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
    fn get_lending_rewards_ok() {
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

            run_to_block(151);

            let lending_user_info = pallet::UserLendingInfo::<Runtime>::get(XOR, alice()).unwrap();

            // From 1st to 151st block, there are 150 blocks
            let lending_earnings = calculate_lending_earnings(alice(), XOR, 150);
            let lending_interest = lending_earnings.0 + lending_earnings.1;

            assert_eq!(lending_user_info.lending_interest, lending_interest);

            assert_ok!(ApolloPlatform::get_rewards(
                RuntimeOrigin::signed(alice()),
                XOR,
                true
            ));

            let lending_user_info = pallet::UserLendingInfo::<Runtime>::get(XOR, alice()).unwrap();

            assert_eq!(lending_user_info.lending_interest, balance!(0));

            let lending_earnings = calculate_lending_earnings(alice(), XOR, 150);
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

            let borrow_user_info = pallet::UserBorrowingInfo::<Runtime>::get(XOR, alice()).unwrap();
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

            let borrow_user_info = pallet::UserBorrowingInfo::<Runtime>::get(XOR, alice()).unwrap();
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
                new_pallet_balance
            );

            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&APOLLO_ASSET_ID, &alice()).unwrap(),
                new_user_balance
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
            let lending_user_info = pallet::UserLendingInfo::<Runtime>::get(XOR, alice()).unwrap();
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
            let lending_user_info = pallet::UserLendingInfo::<Runtime>::get(XOR, alice()).unwrap();
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
            let lending_user_info = pallet::UserLendingInfo::<Runtime>::get(XOR, alice()).unwrap();
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
            let lending_user_info = pallet::UserLendingInfo::<Runtime>::get(XOR, alice());
            assert_eq!(lending_user_info, None);
        });
    }

    #[test]
    fn repay_borrowing_asset_pool_does_not_exist() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_err!(
                ApolloPlatform::repay(RuntimeOrigin::signed(alice()), DOT, XOR, balance!(100)),
                Error::<Runtime>::PoolDoesNotExist
            );
        });
    }

    #[test]
    fn repay_collateral_asset_pool_does_not_exist() {
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
                ApolloPlatform::repay(RuntimeOrigin::signed(alice()), DOT, XOR, balance!(100)),
                Error::<Runtime>::PoolDoesNotExist
            );
        });
    }

    #[test]
    fn repay_nothing_borrowed() {
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

            assert_err!(
                ApolloPlatform::repay(RuntimeOrigin::signed(alice()), DOT, XOR, balance!(100)),
                Error::<Runtime>::NothingBorrowed
            );
        });
    }

    #[test]
    fn repay_nonexistent_borrowing_position() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
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
                balance!(1000)
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
                KSM,
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
                RuntimeOrigin::signed(bob()),
                XOR,
                balance!(1000)
            ));

            assert_ok!(ApolloPlatform::lend(
                RuntimeOrigin::signed(alice()),
                KSM,
                balance!(200)
            ));

            assert_ok!(ApolloPlatform::borrow(
                RuntimeOrigin::signed(alice()),
                KSM,
                XOR,
                balance!(200)
            ));

            assert_err!(
                ApolloPlatform::repay(RuntimeOrigin::signed(alice()), DOT, XOR, balance!(100)),
                Error::<Runtime>::NonexistentBorrowingPosition
            );
        });
    }

    #[test]
    fn repay_only_interest_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            static_set_dex();
            init_exchange();

            run_to_block(1);

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
                balance!(1000)
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
                balance!(0.1),
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
                RuntimeOrigin::signed(bob()),
                XOR,
                balance!(1000)
            ));

            assert_ok!(ApolloPlatform::lend(
                RuntimeOrigin::signed(alice()),
                DOT,
                balance!(200)
            ));

            assert_ok!(ApolloPlatform::borrow(
                RuntimeOrigin::signed(alice()),
                DOT,
                XOR,
                balance!(200)
            ));

            run_to_block(151);

            let borrow_user_info = pallet::UserBorrowingInfo::<Runtime>::get(XOR, alice()).unwrap();
            let borrowing_user_debt = borrow_user_info.get(&DOT).unwrap();
            let borrowing_interest = borrowing_user_debt.borrowing_interest;

            // Check Alice position values before repay
            assert_eq!(borrowing_user_debt.borrowing_amount, balance!(200));

            // Check borrowing asset pool values before repay
            let borrowing_asset_pool_info = pallet::PoolData::<Runtime>::get(XOR).unwrap();

            assert_eq!(borrowing_asset_pool_info.rewards, balance!(0));
            assert_eq!(borrowing_asset_pool_info.total_borrowed, balance!(200));
            assert_eq!(borrowing_asset_pool_info.total_liquidity, balance!(800));

            // Check Alice interest rate before repay
            let calculated_borrowing_interests =
                calculate_borrowing_interest(alice(), XOR, DOT, 148);
            let calculated_borrowing_interest = calculated_borrowing_interests.0;

            assert_eq!(borrowing_interest, calculated_borrowing_interest);

            // Check balances before repay
            // Pool
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR.into(), &get_pallet_account())
                    .unwrap(),
                balance!(800)
            );
            // Alice
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR.into(), &alice()).unwrap(),
                balance!(200)
            );
            // Treasury
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(
                    &APOLLO_ASSET_ID.into(),
                    &get_treasury_account()
                )
                .unwrap(),
                balance!(0)
            );
            // Developer
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR.into(), &get_authority_account())
                    .unwrap(),
                balance!(0)
            );

            let borrowing_interest_one_more_block =
                calculate_borrowing_interest(alice(), XOR, DOT, 149);
            let repayed_amount = borrowing_interest_one_more_block.0;

            // Reserve amounts (treasury, burn, developer)
            let (treasury_reserve, _, developer_reserve) =
                calculate_reserve_amounts(XOR, repayed_amount);

            assert_ok!(ApolloPlatform::repay(
                RuntimeOrigin::signed(alice()),
                DOT,
                XOR,
                repayed_amount
            ));

            // Check borrowing asset pool values after repay
            let borrowing_asset_pool_info = pallet::PoolData::<Runtime>::get(XOR).unwrap();

            let reserves_amount = (FixedWrapper::from(borrowing_asset_pool_info.reserve_factor)
                * FixedWrapper::from(repayed_amount))
            .try_into_balance()
            .unwrap_or(0);
            let rewards_amount = repayed_amount - reserves_amount;

            assert_eq!(borrowing_asset_pool_info.rewards, rewards_amount);
            assert_eq!(borrowing_asset_pool_info.total_borrowed, balance!(200));
            assert_eq!(borrowing_asset_pool_info.total_liquidity, balance!(800));

            // Check Alice interest rate after repay
            let borrow_user_info = pallet::UserBorrowingInfo::<Runtime>::get(XOR, alice()).unwrap();
            let borrowing_user_debt = borrow_user_info.get(&DOT).unwrap();
            let borrowing_interest = borrowing_user_debt.borrowing_interest;

            let new_alice_balance = balance!(200) - repayed_amount;

            assert_eq!(borrowing_interest, balance!(0));

            // Check Alice position values after repay
            assert_eq!(borrowing_user_debt.borrowing_amount, balance!(200));

            // Check balances after repay
            // Pool
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR.into(), &get_pallet_account())
                    .unwrap(),
                balance!(800)
            );
            // Alice
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR.into(), &alice()).unwrap(),
                new_alice_balance
            );
            // Treasury
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(
                    &APOLLO_ASSET_ID.into(),
                    &get_treasury_account()
                )
                .unwrap(),
                treasury_reserve
            );
            // Developer
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR.into(), &get_authority_account())
                    .unwrap(),
                developer_reserve
            );
        });
    }

    #[test]
    fn repay_full_interest_and_part_of_loan_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            static_set_dex();
            init_exchange();

            run_to_block(1);

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
                balance!(1000)
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
                balance!(0.1),
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
                RuntimeOrigin::signed(bob()),
                XOR,
                balance!(1000)
            ));

            assert_ok!(ApolloPlatform::lend(
                RuntimeOrigin::signed(alice()),
                DOT,
                balance!(200)
            ));

            assert_ok!(ApolloPlatform::borrow(
                RuntimeOrigin::signed(alice()),
                DOT,
                XOR,
                balance!(200)
            ));

            run_to_block(151);

            let borrow_user_info = pallet::UserBorrowingInfo::<Runtime>::get(XOR, alice()).unwrap();
            let borrowing_user_debt = borrow_user_info.get(&DOT).unwrap();
            let borrowing_interest = borrowing_user_debt.borrowing_interest;

            // Check Alice position values before repay
            assert_eq!(borrowing_user_debt.borrowing_amount, balance!(200));

            // Check borrowing asset pool values before repay
            let borrowing_asset_pool_info = pallet::PoolData::<Runtime>::get(XOR).unwrap();

            assert_eq!(borrowing_asset_pool_info.rewards, balance!(0));
            assert_eq!(borrowing_asset_pool_info.total_borrowed, balance!(200));
            assert_eq!(borrowing_asset_pool_info.total_liquidity, balance!(800));

            // Check Alice interest rate before repay
            let calculated_borrowing_interests =
                calculate_borrowing_interest(alice(), XOR, DOT, 148);
            let calculated_borrowing_interest = calculated_borrowing_interests.0;

            assert_eq!(borrowing_interest, calculated_borrowing_interest);

            // Check balances before repay
            // Pool
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR.into(), &get_pallet_account())
                    .unwrap(),
                balance!(800)
            );
            // Alice
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR.into(), &alice()).unwrap(),
                balance!(200)
            );
            // Treasury
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(
                    &APOLLO_ASSET_ID.into(),
                    &get_treasury_account()
                )
                .unwrap(),
                balance!(0)
            );
            // Developer
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR.into(), &get_authority_account())
                    .unwrap(),
                balance!(0)
            );

            let borrowing_interest_one_more_block =
                calculate_borrowing_interest(alice(), XOR, DOT, 149);
            let repayed_amount = borrowing_interest_one_more_block.0 + balance!(1);

            // Reserve amounts (treasury, burn, developer)
            let (treasury_reserve, _, developer_reserve) =
                calculate_reserve_amounts(XOR, borrowing_interest_one_more_block.0);

            assert_ok!(ApolloPlatform::repay(
                RuntimeOrigin::signed(alice()),
                DOT,
                XOR,
                repayed_amount
            ));

            let borrow_user_info = pallet::UserBorrowingInfo::<Runtime>::get(XOR, alice()).unwrap();
            let borrowing_user_debt = borrow_user_info.get(&DOT).unwrap();
            let borrowing_interest = borrowing_user_debt.borrowing_interest;

            let new_alice_balance = balance!(200) - repayed_amount;

            // Check Alice position values after repay
            assert_eq!(borrowing_user_debt.borrowing_amount, balance!(199));

            // Check borrowing asset pool values after repay
            let borrowing_asset_pool_info = pallet::PoolData::<Runtime>::get(XOR).unwrap();

            let reserves_amount = (FixedWrapper::from(borrowing_asset_pool_info.reserve_factor)
                * FixedWrapper::from(borrowing_interest_one_more_block.0))
            .try_into_balance()
            .unwrap_or(0);
            let rewards_amount = borrowing_interest_one_more_block.0 - reserves_amount;

            assert_eq!(borrowing_asset_pool_info.rewards, rewards_amount);
            assert_eq!(borrowing_asset_pool_info.total_borrowed, balance!(199));
            assert_eq!(borrowing_asset_pool_info.total_liquidity, balance!(801));

            // Check Alice interest rate after repay
            assert_eq!(borrowing_interest, balance!(0));

            // Check balances after repay
            // Pool
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR.into(), &get_pallet_account())
                    .unwrap(),
                balance!(801)
            );
            // Alice
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR.into(), &alice()).unwrap(),
                new_alice_balance
            );
            // Treasury
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(
                    &APOLLO_ASSET_ID.into(),
                    &get_treasury_account()
                )
                .unwrap(),
                treasury_reserve
            );
            // Developer
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR.into(), &get_authority_account())
                    .unwrap(),
                developer_reserve
            );
        });
    }

    #[test]
    fn repay_full_loan_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            static_set_dex();
            init_exchange();

            run_to_block(1);

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &APOLLO_ASSET_ID,
                &alice(),
                &get_pallet_account(),
                balance!(100)
            ));

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &XOR,
                &alice(),
                &alice(),
                balance!(100)
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
                balance!(1000)
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
                balance!(0.1),
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
                RuntimeOrigin::signed(bob()),
                XOR,
                balance!(1000)
            ));

            assert_ok!(ApolloPlatform::lend(
                RuntimeOrigin::signed(alice()),
                DOT,
                balance!(200)
            ));

            assert_ok!(ApolloPlatform::borrow(
                RuntimeOrigin::signed(alice()),
                DOT,
                XOR,
                balance!(200)
            ));

            run_to_block(151);

            let borrow_user_info = pallet::UserBorrowingInfo::<Runtime>::get(XOR, alice()).unwrap();
            let borrowing_user_debt = borrow_user_info.get(&DOT).unwrap();
            let borrowing_interest = borrowing_user_debt.borrowing_interest;
            let borrowing_rewards = borrowing_user_debt.borrowing_rewards;

            // Check Alice interest rate before repay
            let calculated_borrowing_interests =
                calculate_borrowing_interest(alice(), XOR, DOT, 148);
            let calculated_borrowing_interest = calculated_borrowing_interests.0;

            assert_eq!(borrowing_interest, calculated_borrowing_interest);

            // Check Alice position values before repay
            assert_eq!(borrowing_user_debt.borrowing_amount, balance!(200));
            assert_eq!(borrowing_user_debt.collateral_amount, balance!(200));

            // Check borrowing asset pool values before repay
            let borrowing_asset_pool_info = pallet::PoolData::<Runtime>::get(XOR).unwrap();
            let collateral_asset_pool_info = pallet::PoolData::<Runtime>::get(DOT).unwrap();

            assert_eq!(borrowing_asset_pool_info.rewards, balance!(0));
            assert_eq!(borrowing_asset_pool_info.total_borrowed, balance!(200));
            assert_eq!(borrowing_asset_pool_info.total_liquidity, balance!(800));
            assert_eq!(collateral_asset_pool_info.total_collateral, balance!(200));

            // Check balances before repay
            // Pool
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR.into(), &get_pallet_account())
                    .unwrap(),
                balance!(800)
            );
            // Alice
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR.into(), &alice()).unwrap(),
                balance!(300)
            );
            // Treasury
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(
                    &APOLLO_ASSET_ID.into(),
                    &get_treasury_account()
                )
                .unwrap(),
                balance!(0)
            );
            // Developer
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR.into(), &get_authority_account())
                    .unwrap(),
                balance!(0)
            );

            let borrowing_interest_one_more_block =
                calculate_borrowing_interest(alice(), XOR, DOT, 149);
            let repayed_amount = borrowing_interest_one_more_block.0 + balance!(200);

            // Reserve amounts (treasury, burn, developer)
            let (treasury_reserve, _, developer_reserve) =
                calculate_reserve_amounts(XOR, borrowing_interest_one_more_block.0);

            assert_ok!(ApolloPlatform::repay(
                RuntimeOrigin::signed(alice()),
                DOT,
                XOR,
                repayed_amount
            ));

            // Check borrowing asset pool values after repay
            let borrowing_asset_pool_info = pallet::PoolData::<Runtime>::get(XOR).unwrap();
            let collateral_asset_pool_info = pallet::PoolData::<Runtime>::get(DOT).unwrap();

            let reserves_amount = (FixedWrapper::from(borrowing_asset_pool_info.reserve_factor)
                * FixedWrapper::from(borrowing_interest_one_more_block.0))
            .try_into_balance()
            .unwrap_or(0);
            let rewards_amount = borrowing_interest_one_more_block.0 - reserves_amount;

            assert_eq!(borrowing_asset_pool_info.rewards, rewards_amount);
            assert_eq!(borrowing_asset_pool_info.total_borrowed, balance!(0));
            assert_eq!(borrowing_asset_pool_info.total_liquidity, balance!(1000));
            assert_eq!(collateral_asset_pool_info.total_collateral, balance!(0));

            let borrow_user_info = pallet::UserBorrowingInfo::<Runtime>::get(XOR, alice());

            let new_alice_balance = balance!(300) - repayed_amount;

            // Check if Alice's position exists after repay
            assert_eq!(borrow_user_info, None);

            // Check balances after repay
            // Pool
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR.into(), &get_pallet_account())
                    .unwrap(),
                balance!(1000)
            );
            // Alice
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR.into(), &alice()).unwrap(),
                new_alice_balance
            );
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&DOT.into(), &alice()).unwrap(),
                balance!(200)
            );
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&APOLLO_ASSET_ID.into(), &alice()).unwrap(),
                balance!(300000) + borrowing_rewards
            );
            // Treasury
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(
                    &APOLLO_ASSET_ID.into(),
                    &get_treasury_account()
                )
                .unwrap(),
                treasury_reserve
            );
            // Developer
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR.into(), &get_authority_account())
                    .unwrap(),
                developer_reserve
            );
        });
    }

    #[test]
    fn change_rewards_amount_unauthorized() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_err!(
                ApolloPlatform::change_rewards_amount(
                    RuntimeOrigin::signed(alice()),
                    true,
                    balance!(1)
                ),
                Error::<Runtime>::Unauthorized
            );
        });
    }

    #[test]
    fn change_lending_rewards_amount_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            // Check lending rewards value before change
            assert_eq!(pallet::LendingRewards::<Runtime>::get(), balance!(200000));

            assert_ok!(ApolloPlatform::change_rewards_amount(
                RuntimeOrigin::signed(ApolloPlatform::authority_account()),
                true,
                balance!(1)
            ));

            // Check lending rewards value after change
            assert_eq!(pallet::LendingRewards::<Runtime>::get(), balance!(1));
        });
    }

    #[test]
    fn change_borrowing_rewards_amount_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            // Check lending rewards value before change
            assert_eq!(pallet::BorrowingRewards::<Runtime>::get(), balance!(100000));

            assert_ok!(ApolloPlatform::change_rewards_amount(
                RuntimeOrigin::signed(ApolloPlatform::authority_account()),
                false,
                balance!(1)
            ));

            // Check lending rewards value after change
            assert_eq!(pallet::BorrowingRewards::<Runtime>::get(), balance!(1));
        });
    }

    #[test]
    fn change_rewards_per_block_unauthorized() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_err!(
                ApolloPlatform::change_rewards_per_block(
                    RuntimeOrigin::signed(alice()),
                    false,
                    balance!(1)
                ),
                Error::<Runtime>::Unauthorized
            );
        });
    }

    #[test]
    fn change_lending_rewards_per_block_ok() {
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

            assert_ok!(ApolloPlatform::add_pool(
                RuntimeOrigin::signed(ApolloPlatform::authority_account()),
                KSM,
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
            ));

            // Check pool basic lending rates before change
            let new_basic_lending_rate =
                (FixedWrapper::from(ApolloPlatform::lending_rewards_per_block())
                    / FixedWrapper::from(balance!(3)))
                .try_into_balance()
                .unwrap_or(0);

            let new_borrowing_rewards_rate =
                (FixedWrapper::from(ApolloPlatform::borrowing_rewards_per_block())
                    / FixedWrapper::from(balance!(3)))
                .try_into_balance()
                .unwrap_or(0);

            for (_asset_id, pool_info) in pallet::PoolData::<Runtime>::iter() {
                assert_eq!(pool_info.basic_lending_rate, new_basic_lending_rate);
                assert_eq!(pool_info.borrowing_rewards_rate, new_borrowing_rewards_rate);
            }

            // Check lending rewards value before change
            assert_eq!(
                pallet::LendingRewardsPerBlock::<Runtime>::get(),
                balance!(0.03805175)
            );

            assert_ok!(ApolloPlatform::change_rewards_per_block(
                RuntimeOrigin::signed(ApolloPlatform::authority_account()),
                true,
                balance!(1)
            ));

            // Check lending rewards value after change
            assert_eq!(
                pallet::LendingRewardsPerBlock::<Runtime>::get(),
                balance!(1)
            );

            // Check pool basic lending rates after change
            let new_basic_lending_rate =
                (FixedWrapper::from(ApolloPlatform::lending_rewards_per_block())
                    / FixedWrapper::from(balance!(3)))
                .try_into_balance()
                .unwrap_or(0);

            let new_borrowing_rewards_rate =
                (FixedWrapper::from(ApolloPlatform::borrowing_rewards_per_block())
                    / FixedWrapper::from(balance!(3)))
                .try_into_balance()
                .unwrap_or(0);

            for (_asset_id, pool_info) in pallet::PoolData::<Runtime>::iter() {
                assert_eq!(pool_info.basic_lending_rate, new_basic_lending_rate);
                assert_eq!(pool_info.borrowing_rewards_rate, new_borrowing_rewards_rate);
            }
        });
    }

    #[test]
    fn change_borrowing_rewards_per_block_ok() {
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

            assert_ok!(ApolloPlatform::add_pool(
                RuntimeOrigin::signed(ApolloPlatform::authority_account()),
                KSM,
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(1),
            ));

            // Check pool basic lending rates before change
            let new_basic_lending_rate =
                (FixedWrapper::from(ApolloPlatform::lending_rewards_per_block())
                    / FixedWrapper::from(balance!(3)))
                .try_into_balance()
                .unwrap_or(0);

            let new_borrowing_rewards_rate =
                (FixedWrapper::from(ApolloPlatform::borrowing_rewards_per_block())
                    / FixedWrapper::from(balance!(3)))
                .try_into_balance()
                .unwrap_or(0);

            for (_asset_id, pool_info) in pallet::PoolData::<Runtime>::iter() {
                assert_eq!(pool_info.basic_lending_rate, new_basic_lending_rate);
                assert_eq!(pool_info.borrowing_rewards_rate, new_borrowing_rewards_rate);
            }

            // Check borrowing rewards value before change
            assert_eq!(
                pallet::BorrowingRewardsPerBlock::<Runtime>::get(),
                balance!(0.01902587)
            );

            assert_ok!(ApolloPlatform::change_rewards_per_block(
                RuntimeOrigin::signed(ApolloPlatform::authority_account()),
                false,
                balance!(1)
            ));

            // Check borrowing rewards value after change
            assert_eq!(
                pallet::BorrowingRewardsPerBlock::<Runtime>::get(),
                balance!(1)
            );

            // Check pool basic lending rates after change
            let new_basic_lending_rate =
                (FixedWrapper::from(ApolloPlatform::lending_rewards_per_block())
                    / FixedWrapper::from(balance!(3)))
                .try_into_balance()
                .unwrap_or(0);

            let new_borrowing_rewards_rate =
                (FixedWrapper::from(ApolloPlatform::borrowing_rewards_per_block())
                    / FixedWrapper::from(balance!(3)))
                .try_into_balance()
                .unwrap_or(0);

            for (_asset_id, pool_info) in pallet::PoolData::<Runtime>::iter() {
                assert_eq!(pool_info.basic_lending_rate, new_basic_lending_rate);
                assert_eq!(pool_info.borrowing_rewards_rate, new_borrowing_rewards_rate);
            }
        });
    }

    #[test]
    fn get_price_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let xor_price = ApolloPlatform::get_price(XOR);
            let dot_price = ApolloPlatform::get_price(DOT);
            let dai_price = ApolloPlatform::get_price(DAI);
            let ksm_price = ApolloPlatform::get_price(KSM);

            assert_eq!(xor_price, balance!(1));
            assert_eq!(dot_price, balance!(1));
            assert_eq!(dai_price, balance!(1));
            assert_eq!(ksm_price, balance!(1));
        });
    }

    #[test]
    fn calculate_lending_earnings_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            // Note: We do not use the run_to_block() function as it would then update the last_lending_block for each user and thus
            // give is a 0 when we calculate the lending interest via the pallets' calculate_lending_earnings() function.

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
                balance!(300),
            ));

            let lending_earnings = ApolloPlatform::calculate_lending_earnings(&alice(), XOR, 100);
            let correct_lending_earnings = calculate_lending_earnings(alice(), XOR, 100);

            assert_eq!(lending_earnings.0, correct_lending_earnings.0);
            assert_eq!(lending_earnings.1, correct_lending_earnings.1);
        });
    }

    #[test]
    fn calculate_borrowing_interest_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            // Note: We do not use the run_to_block() function as it would then update the last_borrowing_block for each user and thus
            // give is a 0 when we calculate the borrowing interest via the pallets' calculate_borrowing_interest_and_reward() function.

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
                balance!(300)
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
                balance!(300),
            ));

            assert_ok!(ApolloPlatform::borrow(
                RuntimeOrigin::signed(alice()),
                DOT,
                XOR,
                balance!(100)
            ));

            let borrowing_interest =
                ApolloPlatform::calculate_borrowing_interest_and_reward(&alice(), XOR, DOT, 100);
            let correct_borrowing_interest = calculate_borrowing_interest(alice(), XOR, DOT, 100);

            assert_eq!(borrowing_interest.0, correct_borrowing_interest.0);
            assert_eq!(borrowing_interest.1, correct_borrowing_interest.1);
        });
    }

    #[test]
    fn distribute_protocol_interest_pool_does_not_exist() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_err!(
                ApolloPlatform::distribute_protocol_interest(XOR, balance!(100)),
                Error::<Runtime>::PoolDoesNotExist
            );
        });
    }

    #[test]
    fn distribute_protocol_interest_can_not_transfer_amount_to_developers() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            init_exchange();

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &XOR,
                &alice(),
                &get_pallet_account(),
                balance!(10)
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

            assert_err!(
                ApolloPlatform::distribute_protocol_interest(XOR, balance!(100)),
                Error::<Runtime>::CanNotTransferAmountToDevelopers
            );
        });
    }

    #[test]
    fn distribute_protocol_interest_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            init_exchange();

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &XOR,
                &alice(),
                &get_pallet_account(),
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
                balance!(0.1),
            ));

            // Check balances before distribution of rewards
            // Pallet
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR.into(), &get_pallet_account())
                    .unwrap(),
                balance!(300)
            );

            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(
                    &APOLLO_ASSET_ID.into(),
                    &get_pallet_account()
                )
                .unwrap(),
                balance!(0)
            );

            assert_ok!(ApolloPlatform::distribute_protocol_interest(
                XOR,
                balance!(100)
            ));

            // Check borrowing asset pool values before repay
            let borrowing_asset_pool_info = pallet::PoolData::<Runtime>::get(XOR).unwrap();
            assert_eq!(borrowing_asset_pool_info.rewards, balance!(90));

            // Check balances after distribution of rewards
            let (treasury_reserve, _, developer_amount) =
                calculate_reserve_amounts(XOR, balance!(100));

            // Pallet
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR.into(), &get_pallet_account())
                    .unwrap(),
                balance!(200)
            );

            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(
                    &APOLLO_ASSET_ID.into(),
                    &get_pallet_account()
                )
                .unwrap(),
                balance!(90)
            );

            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(
                    &CERES_ASSET_ID.into(),
                    &get_pallet_account()
                )
                .unwrap(),
                balance!(0)
            );

            // Exchange
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(
                    &CERES_ASSET_ID.into(),
                    &exchange_account()
                )
                .unwrap(),
                balance!(998)
            );

            // Treasury
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(
                    &APOLLO_ASSET_ID.into(),
                    &get_treasury_account()
                )
                .unwrap(),
                treasury_reserve
            );

            // Developer / Authority
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&XOR.into(), &get_authority_account())
                    .unwrap(),
                developer_amount
            );
        });
    }

    #[test]
    fn update_rates_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            static_set_dex();
            init_exchange();

            run_to_block(1);

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &DOT,
                &alice(),
                &alice(),
                balance!(100)
            ));

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &XOR,
                &alice(),
                &bob(),
                balance!(100)
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
                balance!(0.1),
            ));

            assert_ok!(ApolloPlatform::add_pool(
                RuntimeOrigin::signed(ApolloPlatform::authority_account()),
                DOT,
                balance!(1),
                balance!(1),
                balance!(0.4),
                balance!(1),
                balance!(1),
                balance!(1),
                balance!(0.1),
            ));

            // Lend assets to collateral pools
            assert_ok!(ApolloPlatform::lend(
                RuntimeOrigin::signed(alice()),
                DOT,
                balance!(100)
            ));

            assert_ok!(ApolloPlatform::lend(
                RuntimeOrigin::signed(bob()),
                XOR,
                balance!(100)
            ));

            // Borrow assets
            assert_ok!(ApolloPlatform::borrow(
                RuntimeOrigin::signed(alice()),
                DOT,
                XOR,
                balance!(40)
            ));

            assert_ok!(ApolloPlatform::borrow(
                RuntimeOrigin::signed(bob()),
                XOR,
                DOT,
                balance!(40)
            ));

            run_to_block(3);

            // Check pool rates before repayment
            // XOR pool
            let xor_pool_info = pallet::PoolData::<Runtime>::get(XOR).unwrap();
            let xor_pool_profit_lending_rate = xor_pool_info.profit_lending_rate;
            let xor_pool_borrowing_rate = xor_pool_info.borrowing_rate;

            let (xor_pool_current_profit_lending_rate, xor_pool_current_borrowing_rate) =
                calculate_rates(&xor_pool_info);

            assert_eq!(
                xor_pool_profit_lending_rate,
                xor_pool_current_profit_lending_rate
            );
            assert_eq!(xor_pool_borrowing_rate, xor_pool_current_borrowing_rate);

            // DOT pool
            let dot_pool_info = pallet::PoolData::<Runtime>::get(DOT).unwrap();
            let dot_pool_profit_lending_rate = dot_pool_info.profit_lending_rate;
            let dot_pool_borrowing_rate = dot_pool_info.borrowing_rate;

            let (dot_pool_current_profit_lending_rate, dot_pool_current_borrowing_rate) =
                calculate_rates(&dot_pool_info);

            assert_eq!(
                dot_pool_profit_lending_rate,
                dot_pool_current_profit_lending_rate
            );
            assert_eq!(dot_pool_borrowing_rate, dot_pool_current_borrowing_rate);

            // Calculate interest for Alice and Bob
            let alice_user_info = pallet::UserBorrowingInfo::<Runtime>::get(XOR, alice()).unwrap();
            let alice_xor_borrowing_position = alice_user_info.get(&DOT).unwrap();
            let alice_repay_amount = alice_xor_borrowing_position.borrowing_interest;

            let bob_user_info = pallet::UserBorrowingInfo::<Runtime>::get(DOT, bob()).unwrap();
            let bob_xor_borrowing_position = bob_user_info.get(&XOR).unwrap();
            let bob_repay_amount = bob_xor_borrowing_position.borrowing_interest;

            // Repay interest for Alice and Bob
            assert_ok!(ApolloPlatform::repay(
                RuntimeOrigin::signed(alice()),
                DOT,
                XOR,
                alice_repay_amount
            ));

            assert_ok!(ApolloPlatform::repay(
                RuntimeOrigin::signed(bob()),
                XOR,
                DOT,
                bob_repay_amount
            ));

            run_to_block(4);

            // Check pool rates after repayment
            // XOR pool
            let xor_pool_info = pallet::PoolData::<Runtime>::get(XOR).unwrap();
            let xor_pool_profit_lending_rate = xor_pool_info.profit_lending_rate;
            let xor_pool_borrowing_rate = xor_pool_info.borrowing_rate;

            let (xor_pool_current_profit_lending_rate, xor_pool_current_borrowing_rate) =
                calculate_rates(&xor_pool_info);

            assert_eq!(
                xor_pool_profit_lending_rate,
                xor_pool_current_profit_lending_rate
            );
            assert_eq!(xor_pool_borrowing_rate, xor_pool_current_borrowing_rate);

            // DOT pool
            let dot_pool_info = pallet::PoolData::<Runtime>::get(DOT).unwrap();
            let dot_pool_profit_lending_rate = dot_pool_info.profit_lending_rate;
            let dot_pool_borrowing_rate = dot_pool_info.borrowing_rate;

            let (dot_pool_current_profit_lending_rate, dot_pool_current_borrowing_rate) =
                calculate_rates(&dot_pool_info);

            assert_eq!(
                dot_pool_profit_lending_rate,
                dot_pool_current_profit_lending_rate
            );
            assert_eq!(dot_pool_borrowing_rate, dot_pool_current_borrowing_rate);
        });
    }
}
