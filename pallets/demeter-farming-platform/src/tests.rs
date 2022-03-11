mod tests {
    use crate::mock::*;
    use crate::{pallet, Error, PoolInfo, Pools, TokenInfo, TokenInfos};
    use common::{balance, CERES_ASSET_ID, XOR};
    use frame_support::{assert_err, assert_ok};
    use sp_runtime::traits::AccountIdConversion;
    use sp_runtime::ModuleId;

    #[test]
    fn change_pool_multiplier_unauthorized() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let pool_asset = XOR;
            let reward_asset = CERES_ASSET_ID;
            let is_farm = true;
            let new_multiplier = 1;

            assert_err!(
                DemeterFarmingPlatform::change_pool_multiplier(
                    Origin::signed(ALICE),
                    pool_asset,
                    reward_asset,
                    is_farm,
                    new_multiplier
                ),
                Error::<Runtime>::Unauthorized
            )
        });
    }

    #[test]
    fn change_pool_multiplier_invalid_multiplier() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let pool_asset = XOR;
            let reward_asset = CERES_ASSET_ID;
            let is_farm = true;
            let new_multiplier = 0;

            let token_info = TokenInfo {
                farms_total_multiplier: 0,
                staking_total_multiplier: 0,
                token_per_block: balance!(1),
                farms_allocation: balance!(0.2),
                staking_allocation: balance!(0.4),
                team_allocation: balance!(0.4),
            };

            <TokenInfos<Runtime>>::insert(&reward_asset, &token_info);

            let pool_info = PoolInfo {
                multiplier: 1,
                deposit_fee: balance!(0),
                is_core: true,
                is_farm,
                total_tokens_in_pool: 0,
                rewards: 0,
                rewards_to_be_distributed: 0,
                is_removed: false,
            };

            <Pools<Runtime>>::append(&pool_asset, &reward_asset, pool_info);

            assert_err!(
                DemeterFarmingPlatform::change_pool_multiplier(
                    Origin::signed(pallet::AuthorityAccount::<Runtime>::get()),
                    pool_asset,
                    reward_asset,
                    is_farm,
                    new_multiplier
                ),
                Error::<Runtime>::InvalidMultiplier
            )
        });
    }

    #[test]
    fn change_pool_multiplier_pool_does_not_exist() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let pool_asset = XOR;
            let reward_asset = CERES_ASSET_ID;
            let is_farm = true;
            let new_multiplier = 1;

            let token_info = TokenInfo {
                farms_total_multiplier: 0,
                staking_total_multiplier: 0,
                token_per_block: balance!(1),
                farms_allocation: balance!(0.2),
                staking_allocation: balance!(0.4),
                team_allocation: balance!(0.4),
            };

            <TokenInfos<Runtime>>::insert(&reward_asset, &token_info);

            assert_err!(
                DemeterFarmingPlatform::change_pool_multiplier(
                    Origin::signed(pallet::AuthorityAccount::<Runtime>::get()),
                    pool_asset,
                    reward_asset,
                    is_farm,
                    new_multiplier
                ),
                Error::<Runtime>::PoolDoesNotExist
            )
        });
    }

    #[test]
    fn change_pool_multiplier_is_farm_true() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let pool_asset = XOR;
            let reward_asset = CERES_ASSET_ID;
            let is_farm = true;
            let new_multiplier = 2;

            let mut token_info = TokenInfo {
                farms_total_multiplier: 1,
                staking_total_multiplier: 0,
                token_per_block: balance!(1),
                farms_allocation: balance!(0.2),
                staking_allocation: balance!(0.4),
                team_allocation: balance!(0.4),
            };

            <TokenInfos<Runtime>>::insert(&reward_asset, &token_info);

            let pool_info = PoolInfo {
                multiplier: 1,
                deposit_fee: balance!(0),
                is_core: true,
                is_farm,
                total_tokens_in_pool: 0,
                rewards: 0,
                rewards_to_be_distributed: 0,
                is_removed: false,
            };

            <Pools<Runtime>>::append(&pool_asset, &reward_asset, &pool_info);

            assert_ok!(DemeterFarmingPlatform::change_pool_multiplier(
                Origin::signed(pallet::AuthorityAccount::<Runtime>::get()),
                pool_asset,
                reward_asset,
                is_farm,
                new_multiplier
            ));

            token_info = <TokenInfos<Runtime>>::get(&reward_asset);
            let mut pool_infos = <Pools<Runtime>>::get(&pool_asset, &reward_asset);
            for pool_info in pool_infos.iter_mut() {
                if pool_info.is_farm == is_farm && !pool_info.is_removed {
                    assert_eq!(pool_info.multiplier, new_multiplier);
                }
            }
            assert_eq!(token_info.farms_total_multiplier, new_multiplier);
        });
    }

    #[test]
    fn change_pool_multiplier_is_farm_false() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let pool_asset = XOR;
            let reward_asset = CERES_ASSET_ID;
            let is_farm = false;
            let new_multiplier = 2;

            let mut token_info = TokenInfo {
                farms_total_multiplier: 1,
                staking_total_multiplier: 1,
                token_per_block: balance!(1),
                farms_allocation: balance!(0.2),
                staking_allocation: balance!(0.4),
                team_allocation: balance!(0.4),
            };

            <TokenInfos<Runtime>>::insert(&reward_asset, &token_info);

            let pool_info = PoolInfo {
                multiplier: 1,
                deposit_fee: balance!(0),
                is_core: true,
                is_farm,
                total_tokens_in_pool: 0,
                rewards: 0,
                rewards_to_be_distributed: 0,
                is_removed: false,
            };

            <Pools<Runtime>>::append(&pool_asset, &reward_asset, &pool_info);

            assert_ok!(DemeterFarmingPlatform::change_pool_multiplier(
                Origin::signed(pallet::AuthorityAccount::<Runtime>::get()),
                pool_asset,
                reward_asset,
                is_farm,
                new_multiplier
            ));

            token_info = <TokenInfos<Runtime>>::get(&reward_asset);
            let mut pool_infos = <Pools<Runtime>>::get(&pool_asset, &reward_asset);
            for pool_info in pool_infos.iter_mut() {
                if pool_info.is_farm == is_farm && !pool_info.is_removed {
                    assert_eq!(pool_info.multiplier, new_multiplier);
                }
            }
            assert_eq!(token_info.staking_total_multiplier, new_multiplier);
        });
    }

    #[test]
    fn change_pool_deposit_fee_unauthorized() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let pool_asset = XOR;
            let reward_asset = CERES_ASSET_ID;
            let is_farm = true;
            let deposit_fee = balance!(1);

            assert_err!(
                DemeterFarmingPlatform::change_pool_deposit_fee(
                    Origin::signed(ALICE),
                    pool_asset,
                    reward_asset,
                    is_farm,
                    deposit_fee
                ),
                Error::<Runtime>::Unauthorized
            )
        });
    }

    #[test]
    fn change_pool_deposit_fee_pool_does_not_exist() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let pool_asset = XOR;
            let reward_asset = CERES_ASSET_ID;
            let is_farm = true;
            let deposit_fee = balance!(1);

            assert_err!(
                DemeterFarmingPlatform::change_pool_deposit_fee(
                    Origin::signed(pallet::AuthorityAccount::<Runtime>::get()),
                    pool_asset,
                    reward_asset,
                    is_farm,
                    deposit_fee
                ),
                Error::<Runtime>::PoolDoesNotExist
            )
        });
    }

    #[test]
    fn change_pool_deposit_fee_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let pool_asset = XOR;
            let reward_asset = CERES_ASSET_ID;
            let is_farm = true;
            let deposit_fee = balance!(1);

            let pool_info = PoolInfo {
                multiplier: 1,
                deposit_fee,
                is_core: true,
                is_farm,
                total_tokens_in_pool: 0,
                rewards: 0,
                rewards_to_be_distributed: 0,
                is_removed: false,
            };

            <Pools<Runtime>>::append(&pool_asset, &reward_asset, &pool_info);

            let mut pool_infos = <Pools<Runtime>>::get(&pool_asset, &reward_asset);
            for p_info in pool_infos.iter_mut() {
                if !p_info.is_removed && p_info.is_farm == is_farm {
                    assert_eq!(pool_info.deposit_fee, deposit_fee)
                }
            }
        });
    }

    #[test]
    fn change_token_info_unauthorized() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let pool_asset = XOR;
            let token_per_block = balance!(1);
            let farms_allocation = balance!(0.6);
            let staking_allocation = balance!(0.2);
            let team_allocation = balance!(0.2);

            assert_err!(
                DemeterFarmingPlatform::change_token_info(
                    Origin::signed(ALICE),
                    pool_asset,
                    token_per_block,
                    farms_allocation,
                    staking_allocation,
                    team_allocation
                ),
                Error::<Runtime>::Unauthorized
            )
        });
    }

    #[test]
    fn change_token_info_reward_token_is_not_registered() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let pool_asset = XOR;
            let reward_asset = CERES_ASSET_ID;
            let token_per_block = balance!(1);
            let farms_allocation = balance!(0.2);
            let staking_allocation = balance!(0.4);
            let team_allocation = balance!(0.4);

            let token_info = TokenInfo {
                farms_total_multiplier: 1,
                staking_total_multiplier: 1,
                token_per_block,
                farms_allocation,
                staking_allocation,
                team_allocation,
            };

            <TokenInfos<Runtime>>::insert(&reward_asset, &token_info);

            assert_err!(
                DemeterFarmingPlatform::change_token_info(
                    Origin::signed(pallet::AuthorityAccount::<Runtime>::get()),
                    pool_asset,
                    token_per_block,
                    farms_allocation,
                    staking_allocation,
                    team_allocation
                ),
                Error::<Runtime>::RewardTokenIsNotRegistered
            )
        });
    }

    #[test]
    fn change_token_info_reward_token_per_block_cant_be_zero() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let reward_asset = CERES_ASSET_ID;
            let token_per_block = balance!(0);
            let farms_allocation = balance!(0.2);
            let staking_allocation = balance!(0.4);
            let team_allocation = balance!(0.4);

            let token_info = TokenInfo {
                farms_total_multiplier: 1,
                staking_total_multiplier: 1,
                token_per_block: balance!(1),
                farms_allocation,
                staking_allocation,
                team_allocation,
            };

            <TokenInfos<Runtime>>::insert(&reward_asset, &token_info);

            assert_err!(
                DemeterFarmingPlatform::change_token_info(
                    Origin::signed(pallet::AuthorityAccount::<Runtime>::get()),
                    reward_asset,
                    token_per_block,
                    farms_allocation,
                    staking_allocation,
                    team_allocation
                ),
                Error::<Runtime>::TokenPerBlockCantBeZero
            )
        });
    }

    #[test]
    fn change_token_info_invalid_allocation_parameters() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let reward_asset = CERES_ASSET_ID;
            let token_per_block = balance!(1);
            let farms_allocation = balance!(0.1);
            let staking_allocation = balance!(0.4);
            let team_allocation = balance!(0.4);

            let token_info = TokenInfo {
                farms_total_multiplier: 1,
                staking_total_multiplier: 1,
                token_per_block,
                farms_allocation,
                staking_allocation,
                team_allocation,
            };

            <TokenInfos<Runtime>>::insert(&reward_asset, &token_info);

            assert_err!(
                DemeterFarmingPlatform::change_token_info(
                    Origin::signed(pallet::AuthorityAccount::<Runtime>::get()),
                    reward_asset,
                    token_per_block,
                    farms_allocation,
                    staking_allocation,
                    team_allocation
                ),
                Error::<Runtime>::InvalidAllocationParameters
            )
        });
    }

    #[test]
    fn change_token_info_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let reward_asset = CERES_ASSET_ID;
            let token_per_block = balance!(1);
            let farms_allocation = balance!(0.1);
            let staking_allocation = balance!(0.4);
            let team_allocation = balance!(0.4);

            let token_info = TokenInfo {
                farms_total_multiplier: 1,
                staking_total_multiplier: 1,
                token_per_block,
                farms_allocation,
                staking_allocation,
                team_allocation,
            };

            <TokenInfos<Runtime>>::insert(&reward_asset, &token_info);

            assert_eq!(token_info.token_per_block, token_per_block);
            assert_eq!(token_info.farms_allocation, farms_allocation);
            assert_eq!(token_info.staking_allocation, staking_allocation);
            assert_eq!(token_info.team_allocation, team_allocation);
        });
    }
}
