mod tests {
    use crate::mock::*;
    use common::prelude::FixedWrapper;
    use common::{
        balance, AssetId32, AssetName, AssetSymbol, Balance, LiquiditySourceType, PoolXykPallet,
        PredefinedAssetId, ToFeeAccount, CERES_ASSET_ID, DEFAULT_BALANCE_PRECISION,
        DEMETER_ASSET_ID, XOR,
    };
    use demeter_farming_platform::{PoolData, TokenInfo, UserInfo};
    use frame_support::{assert_err, assert_ok};
    use hex_literal::hex;
    use sp_runtime::traits::AccountIdConversion;
    use sp_runtime::ModuleId;

    fn preset_initial<Fun>(tests: Fun)
    where
        Fun: Fn(),
    {
        let mut ext = ExtBuilder::default().build();
        let dex_id = DEX_A_ID;
        let xor: AssetId = XOR.into();
        let ceres: AssetId = CERES_ASSET_ID.into();
        let util: AssetId32<PredefinedAssetId> = AssetId32::from_bytes(hex!(
            "007348eb8f0f3cec730fbf5eec1b6a842c54d1df8bed75a9df084d5ee013e814"
        ));
        let pallet_account = ModuleId(*b"deofarms").into_account();

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

            frame_system::Pallet::<Runtime>::inc_providers(
                &demeter_farming_platform::AuthorityAccount::<Runtime>::get(),
            );
            assert_ok!(assets::Module::<Runtime>::register_asset_id(
                demeter_farming_platform::AuthorityAccount::<Runtime>::get(),
                DEMETER_ASSET_ID.into(),
                AssetSymbol(b"DEO".to_vec()),
                AssetName(b"Demeter".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                Balance::from(0u32),
                true,
                None,
                None,
            ));

            assert_ok!(assets::Module::<Runtime>::register_asset_id(
                ALICE,
                util.into(),
                AssetSymbol(b"UTIL".to_vec()),
                AssetName(b"Util".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                Balance::from(0u32),
                true,
                None,
                None,
            ));

            assert_ok!(trading_pair::Module::<Runtime>::register(
                Origin::signed(BOB),
                dex_id.clone(),
                XOR.into(),
                CERES_ASSET_ID.into()
            ));

            assert_ok!(pool_xyk::Module::<Runtime>::initialize_pool(
                Origin::signed(BOB),
                dex_id.clone(),
                XOR.into(),
                CERES_ASSET_ID.into(),
            ));

            assert!(
                trading_pair::Module::<Runtime>::is_source_enabled_for_trading_pair(
                    &dex_id,
                    &XOR.into(),
                    &CERES_ASSET_ID.into(),
                    LiquiditySourceType::XYKPool,
                )
                .expect("Failed to query trading pair status.")
            );

            let (_tpair, tech_acc_id) =
                pool_xyk::Module::<Runtime>::tech_account_from_dex_and_asset_pair(
                    dex_id.clone(),
                    XOR.into(),
                    CERES_ASSET_ID.into(),
                )
                .unwrap();

            let fee_acc = tech_acc_id.clone().to_fee_account().unwrap();
            let repr: AccountId =
                technical::Module::<Runtime>::tech_account_id_to_account_id(&tech_acc_id).unwrap();
            let fee_repr: AccountId =
                technical::Module::<Runtime>::tech_account_id_to_account_id(&fee_acc).unwrap();

            assert_eq!(
                pool_xyk::Module::<Runtime>::properties(xor, ceres),
                Some((repr.clone(), fee_repr.clone()))
            );

            assert_ok!(assets::Module::<Runtime>::mint_to(
                &xor,
                &ALICE,
                &ALICE,
                balance!(2000)
            ));

            assert_ok!(assets::Module::<Runtime>::mint_to(
                &ceres,
                &ALICE,
                &ALICE,
                balance!(2000)
            ));

            assert_ok!(assets::Module::<Runtime>::mint_to(
                &xor,
                &ALICE,
                &BOB,
                balance!(2000)
            ));

            assert_ok!(assets::Module::<Runtime>::mint_to(
                &ceres,
                &ALICE,
                &BOB,
                balance!(2000)
            ));

            assert_ok!(assets::Module::<Runtime>::mint_to(
                &ceres,
                &ALICE,
                &pallet_account,
                balance!(1000)
            ));

            assert_ok!(assets::Module::<Runtime>::mint_to(
                &xor,
                &ALICE,
                &pallet_account,
                balance!(1000)
            ));

            assert_ok!(assets::Module::<Runtime>::mint_to(
                &util,
                &ALICE,
                &ALICE,
                balance!(2000)
            ));

            assert_eq!(
                assets::Module::<Runtime>::free_balance(&xor, &ALICE).unwrap(),
                balance!(2000)
            );
            assert_eq!(
                assets::Module::<Runtime>::free_balance(&ceres, &ALICE).unwrap(),
                balance!(3000)
            );

            tests();
        });
    }

    #[test]
    fn register_token_unauthorized() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let pool_asset = XOR;
            let token_per_block = balance!(1);
            let farms_allocation = balance!(0.6);
            let staking_allocation = balance!(0.2);
            let team_allocation = balance!(0.2);

            assert_err!(
                demeter_farming_platform::Pallet::<Runtime>::register_token(
                    Origin::signed(ALICE),
                    pool_asset,
                    token_per_block,
                    farms_allocation,
                    staking_allocation,
                    team_allocation,
                    BOB
                ),
                demeter_farming_platform::Error::<Runtime>::Unauthorized
            )
        });
    }

    #[test]
    fn register_token_token_already_registered() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let reward_asset = XOR;
            let token_per_block = balance!(1);
            let farms_allocation = balance!(0.6);
            let staking_allocation = balance!(0.2);
            let team_allocation = balance!(0.2);

            let token_info = TokenInfo {
                farms_total_multiplier: 0,
                staking_total_multiplier: 0,
                token_per_block,
                farms_allocation,
                staking_allocation,
                team_allocation,
                team_account: BOB,
            };

            demeter_farming_platform::TokenInfos::<Runtime>::insert(&reward_asset, &token_info);

            assert_err!(
                demeter_farming_platform::Pallet::<Runtime>::register_token(
                    Origin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                    reward_asset,
                    token_per_block,
                    farms_allocation,
                    staking_allocation,
                    team_allocation,
                    BOB
                ),
                demeter_farming_platform::Error::<Runtime>::TokenAlreadyRegistered
            )
        });
    }

    #[test]
    fn register_token_token_per_block_cant_be_zero() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let reward_asset = XOR;
            let token_per_block = balance!(0);
            let farms_allocation = balance!(0.6);
            let staking_allocation = balance!(0.2);
            let team_allocation = balance!(0.2);

            assert_err!(
                demeter_farming_platform::Pallet::<Runtime>::register_token(
                    Origin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                    reward_asset,
                    token_per_block,
                    farms_allocation,
                    staking_allocation,
                    team_allocation,
                    BOB
                ),
                demeter_farming_platform::Error::<Runtime>::TokenPerBlockCantBeZero
            )
        });
    }

    #[test]
    fn register_token_invalid_allocation_parameters() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let reward_asset = XOR;
            let token_per_block = balance!(1);
            let farms_allocation = balance!(0.6);
            let staking_allocation = balance!(0.3);
            let team_allocation = balance!(0.2);

            assert_err!(
                demeter_farming_platform::Pallet::<Runtime>::register_token(
                    Origin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                    reward_asset,
                    token_per_block,
                    farms_allocation,
                    staking_allocation,
                    team_allocation,
                    BOB
                ),
                demeter_farming_platform::Error::<Runtime>::InvalidAllocationParameters
            )
        });
    }

    #[test]
    fn register_token_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let reward_asset = XOR;
            let token_per_block = balance!(1);
            let farms_allocation = balance!(0.6);
            let staking_allocation = balance!(0.2);
            let team_allocation = balance!(0.2);

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::register_token(
                Origin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                reward_asset,
                token_per_block,
                farms_allocation,
                staking_allocation,
                team_allocation,
                BOB
            ));

            let token_info = demeter_farming_platform::TokenInfos::<Runtime>::get(&reward_asset)
                .unwrap_or_default();

            assert_eq!(token_info.token_per_block, token_per_block);
            assert_eq!(token_info.farms_allocation, farms_allocation);
            assert_eq!(token_info.staking_allocation, staking_allocation);
            assert_eq!(token_info.team_allocation, team_allocation);
        });
    }

    #[test]
    fn add_pool_unauthorized() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let pool_asset = XOR;
            let reward_asset = CERES_ASSET_ID;
            let is_farm = true;
            let multiplier = 1;
            let deposit_fee = balance!(0.4);
            let is_core = true;

            assert_err!(
                demeter_farming_platform::Pallet::<Runtime>::add_pool(
                    Origin::signed(ALICE),
                    pool_asset,
                    reward_asset,
                    is_farm,
                    multiplier,
                    deposit_fee,
                    is_core
                ),
                demeter_farming_platform::Error::<Runtime>::Unauthorized
            )
        });
    }

    #[test]
    fn add_pool_invalid_multiplier() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let pool_asset = XOR;
            let reward_asset = CERES_ASSET_ID;
            let is_farm = true;
            let multiplier = 0;
            let deposit_fee = balance!(0.4);
            let is_core = true;

            assert_err!(
                demeter_farming_platform::Pallet::<Runtime>::add_pool(
                    Origin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                    pool_asset,
                    reward_asset,
                    is_farm,
                    multiplier,
                    deposit_fee,
                    is_core
                ),
                demeter_farming_platform::Error::<Runtime>::InvalidMultiplier
            )
        });
    }

    #[test]
    fn add_pool_invalid_deposit_fee() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let pool_asset = XOR;
            let reward_asset = CERES_ASSET_ID;
            let is_farm = true;
            let multiplier = 1;
            let deposit_fee = balance!(1.1);
            let is_core = true;

            assert_err!(
                demeter_farming_platform::Pallet::<Runtime>::add_pool(
                    Origin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                    pool_asset,
                    reward_asset,
                    is_farm,
                    multiplier,
                    deposit_fee,
                    is_core
                ),
                demeter_farming_platform::Error::<Runtime>::InvalidDepositFee
            )
        });
    }

    #[test]
    fn add_pool_reward_token_is_not_registered() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let pool_asset = XOR;
            let reward_asset = CERES_ASSET_ID;
            let is_farm = true;
            let multiplier = 1;
            let deposit_fee = balance!(0.4);
            let is_core = true;

            assert_err!(
                demeter_farming_platform::Pallet::<Runtime>::add_pool(
                    Origin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                    pool_asset,
                    reward_asset,
                    is_farm,
                    multiplier,
                    deposit_fee,
                    is_core
                ),
                demeter_farming_platform::Error::<Runtime>::RewardTokenIsNotRegistered
            )
        });
    }

    #[test]
    fn add_pool_pool_already_exists() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let pool_asset = XOR;
            let reward_asset = CERES_ASSET_ID;
            let is_farm = true;
            let multiplier = 1;
            let deposit_fee = balance!(0.4);
            let is_core = true;
            let token_per_block = balance!(1);
            let farms_allocation = balance!(0.6);
            let staking_allocation = balance!(0.2);
            let team_allocation = balance!(0.2);

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::register_token(
                Origin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                reward_asset,
                token_per_block,
                farms_allocation,
                staking_allocation,
                team_allocation,
                BOB
            ));

            let pool_info = PoolData {
                multiplier,
                deposit_fee,
                is_core,
                is_farm,
                total_tokens_in_pool: 0,
                rewards: 0,
                rewards_to_be_distributed: 0,
                is_removed: false,
            };

            demeter_farming_platform::Pools::<Runtime>::append(
                &pool_asset,
                &reward_asset,
                pool_info,
            );

            assert_err!(
                demeter_farming_platform::Pallet::<Runtime>::add_pool(
                    Origin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                    pool_asset,
                    reward_asset,
                    is_farm,
                    multiplier,
                    deposit_fee,
                    is_core
                ),
                demeter_farming_platform::Error::<Runtime>::PoolAlreadyExists
            )
        });
    }

    #[test]
    fn add_pool_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let pool_asset = XOR;
            let reward_asset = CERES_ASSET_ID;
            let is_farm = true;
            let multiplier = 1;
            let deposit_fee = balance!(0.4);
            let is_core = true;
            let token_per_block = balance!(1);
            let farms_allocation = balance!(0.6);
            let staking_allocation = balance!(0.2);
            let team_allocation = balance!(0.2);

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::register_token(
                Origin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                reward_asset,
                token_per_block,
                farms_allocation,
                staking_allocation,
                team_allocation,
                BOB
            ));

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::add_pool(
                Origin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                pool_asset,
                reward_asset,
                is_farm,
                multiplier,
                deposit_fee,
                is_core
            ));

            let token_info = demeter_farming_platform::TokenInfos::<Runtime>::get(&reward_asset)
                .unwrap_or_default();
            assert_eq!(token_info.farms_total_multiplier, multiplier);

            let pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(&pool_asset, &reward_asset);
            for pool_info in pool_infos {
                if !pool_info.is_removed && pool_info.is_farm == is_farm {
                    assert_eq!(pool_info.multiplier, multiplier);
                    assert_eq!(pool_info.is_core, is_core);
                    assert_eq!(pool_info.deposit_fee, deposit_fee);
                }
            }
        });
    }

    #[test]
    fn deposit_farming_pool_does_not_exist() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let pool_asset = XOR;
            let reward_asset = CERES_ASSET_ID;
            let is_farm = true;

            assert_err!(
                demeter_farming_platform::Pallet::<Runtime>::deposit(
                    Origin::signed(ALICE),
                    pool_asset,
                    reward_asset,
                    is_farm,
                    balance!(10)
                ),
                demeter_farming_platform::Error::<Runtime>::PoolDoesNotExist
            );
        });
    }

    #[test]
    fn deposit_insufficient_funds() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let pool_asset = XOR;
            let reward_asset = CERES_ASSET_ID;
            let is_farm = false;
            let multiplier = 1;
            let deposit_fee = balance!(0.4);
            let is_core = true;
            let token_per_block = balance!(1);
            let farms_allocation = balance!(0.6);
            let staking_allocation = balance!(0.2);
            let team_allocation = balance!(0.2);

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::register_token(
                Origin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                reward_asset,
                token_per_block,
                farms_allocation,
                staking_allocation,
                team_allocation,
                BOB
            ));

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::add_pool(
                Origin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                pool_asset,
                reward_asset,
                is_farm,
                multiplier,
                deposit_fee,
                is_core
            ));

            assert_err!(
                demeter_farming_platform::Pallet::<Runtime>::deposit(
                    Origin::signed(ALICE),
                    pool_asset,
                    reward_asset,
                    is_farm,
                    balance!(10000)
                ),
                demeter_farming_platform::Error::<Runtime>::InsufficientFunds
            );
        });
    }

    #[test]
    fn deposit_liquidity_pool_does_not_exist() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let pool_asset = XOR;
            let reward_asset = CERES_ASSET_ID;
            let is_farm = true;
            let multiplier = 1;
            let deposit_fee = balance!(0.4);
            let is_core = true;
            let token_per_block = balance!(1);
            let farms_allocation = balance!(0.6);
            let staking_allocation = balance!(0.2);
            let team_allocation = balance!(0.2);

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::register_token(
                Origin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                reward_asset,
                token_per_block,
                farms_allocation,
                staking_allocation,
                team_allocation,
                BOB
            ));

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::add_pool(
                Origin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                pool_asset,
                reward_asset,
                is_farm,
                multiplier,
                deposit_fee,
                is_core
            ));

            assert_err!(
                demeter_farming_platform::Pallet::<Runtime>::deposit(
                    Origin::signed(ALICE),
                    pool_asset,
                    reward_asset,
                    is_farm,
                    balance!(10000)
                ),
                demeter_farming_platform::Error::<Runtime>::PoolDoesNotExist
            );
        });
    }

    #[test]
    fn deposit_insufficient_lp_tokens() {
        preset_initial(|| {
            let reward_asset = CERES_ASSET_ID;
            let is_farm = true;
            let multiplier = 1;
            let deposit_fee = balance!(0.4);
            let is_core = true;
            let token_per_block = balance!(1);
            let farms_allocation = balance!(0.6);
            let staking_allocation = balance!(0.2);
            let team_allocation = balance!(0.2);

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::register_token(
                Origin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                reward_asset,
                token_per_block,
                farms_allocation,
                staking_allocation,
                team_allocation,
                BOB
            ));

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::add_pool(
                Origin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                reward_asset,
                reward_asset,
                is_farm,
                multiplier,
                deposit_fee,
                is_core
            ));

            let pooled_tokens = balance!(10000);
            assert_err!(
                demeter_farming_platform::Pallet::<Runtime>::deposit(
                    Origin::signed(ALICE),
                    reward_asset,
                    reward_asset,
                    is_farm,
                    pooled_tokens
                ),
                demeter_farming_platform::Error::<Runtime>::InsufficientLPTokens
            );
        });
    }

    #[test]
    fn deposit_ok_not_farm() {
        preset_initial(|| {
            let pool_asset = XOR;
            let reward_asset = CERES_ASSET_ID;
            let is_farm = false;
            let multiplier = 1;
            let deposit_fee = balance!(0.04);
            let is_core = true;
            let token_per_block = balance!(1);
            let farms_allocation = balance!(0.6);
            let staking_allocation = balance!(0.2);
            let team_allocation = balance!(0.2);

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::register_token(
                Origin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                reward_asset,
                token_per_block,
                farms_allocation,
                staking_allocation,
                team_allocation,
                BOB
            ));

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::add_pool(
                Origin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                reward_asset,
                reward_asset,
                is_farm,
                multiplier,
                deposit_fee,
                is_core
            ));

            let mut pooled_tokens = balance!(10);
            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::deposit(
                Origin::signed(ALICE),
                reward_asset,
                reward_asset,
                is_farm,
                pooled_tokens
            ));

            let fee = (FixedWrapper::from(pooled_tokens) * FixedWrapper::from(deposit_fee))
                .try_into_balance()
                .unwrap_or(0);
            pooled_tokens -= fee;

            let pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(&pool_asset, &reward_asset);
            for p_info in &pool_infos {
                if !p_info.is_removed && p_info.is_farm == is_farm {
                    assert_eq!(p_info.total_tokens_in_pool, pooled_tokens);
                }
            }

            let user_infos = demeter_farming_platform::UserInfos::<Runtime>::get(&ALICE);
            for u_info in &user_infos {
                if u_info.is_farm == is_farm {
                    assert_eq!(u_info.pooled_tokens, pooled_tokens);
                }
            }

            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &ALICE)
                    .expect("Failed to query free balance."),
                balance!(2990)
            );

            let pallet_account = ModuleId(*b"deofarms").into_account();
            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &pallet_account)
                    .expect("Failed to query free balance."),
                balance!(1000) + pooled_tokens
            );

            assert_eq!(
                Assets::free_balance(
                    &CERES_ASSET_ID,
                    &demeter_farming_platform::FeeAccount::<Runtime>::get()
                )
                .expect("Failed to query free balance."),
                fee
            );
        });
    }

    #[test]
    fn deposit_ok_farm() {
        preset_initial(|| {
            let dex_id = DEX_A_ID;
            let pool_asset = XOR;
            let reward_asset = CERES_ASSET_ID;
            let is_farm = true;
            let multiplier = 1;
            let deposit_fee = balance!(0.04);
            let is_core = true;
            let token_per_block = balance!(1);
            let farms_allocation = balance!(0.6);
            let staking_allocation = balance!(0.2);
            let team_allocation = balance!(0.2);

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::register_token(
                Origin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                reward_asset,
                token_per_block,
                farms_allocation,
                staking_allocation,
                team_allocation,
                BOB
            ));

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::add_pool(
                Origin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                reward_asset,
                reward_asset,
                is_farm,
                multiplier,
                deposit_fee,
                is_core
            ));

            assert_ok!(pool_xyk::Pallet::<Runtime>::deposit_liquidity(
                Origin::signed(ALICE),
                dex_id,
                pool_asset,
                reward_asset,
                balance!(500),
                balance!(700),
                balance!(500),
                balance!(700),
            ));

            let mut pooled_tokens = balance!(1);
            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::deposit(
                Origin::signed(ALICE),
                reward_asset,
                reward_asset,
                is_farm,
                pooled_tokens
            ));
            let fee = (FixedWrapper::from(pooled_tokens) * FixedWrapper::from(deposit_fee))
                .try_into_balance()
                .unwrap_or(0);
            pooled_tokens -= fee;

            let pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(&pool_asset, &reward_asset);
            for p_info in &pool_infos {
                if !p_info.is_removed && p_info.is_farm == is_farm {
                    assert_eq!(p_info.total_tokens_in_pool, pooled_tokens);
                }
            }

            let user_infos = demeter_farming_platform::UserInfos::<Runtime>::get(&ALICE);
            for u_info in &user_infos {
                if u_info.is_farm == is_farm {
                    assert_eq!(u_info.pooled_tokens, pooled_tokens);
                }
            }

            let pool_account = pool_xyk::Pallet::<Runtime>::properties_of_pool(
                pool_asset.clone(),
                reward_asset.clone(),
            )
            .expect("Pool doesn't exist")
            .0;
            let lp_tokens = pool_xyk::Pallet::<Runtime>::balance_of_pool_provider(
                pool_account.clone(),
                demeter_farming_platform::FeeAccount::<Runtime>::get(),
            )
            .unwrap_or(0);
            assert_eq!(lp_tokens, fee);
        });
    }

    #[test]
    fn get_rewards_pool_does_not_exist() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let pool_asset = XOR;
            let reward_asset = CERES_ASSET_ID;
            let is_farm = true;
            let pooled_tokens = 10;
            let rewards = 1;

            let user_info = UserInfo {
                pool_asset,
                reward_asset,
                is_farm,
                pooled_tokens,
                rewards,
            };

            demeter_farming_platform::UserInfos::<Runtime>::append(ALICE, user_info);

            assert_err!(
                demeter_farming_platform::Pallet::<Runtime>::get_rewards(
                    Origin::signed(ALICE),
                    pool_asset,
                    reward_asset,
                    is_farm
                ),
                demeter_farming_platform::Error::<Runtime>::PoolDoesNotExist
            );
        });
    }

    #[test]
    fn get_rewards_zero_rewards() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let pool_asset = XOR;
            let reward_asset = CERES_ASSET_ID;
            let is_farm = true;
            let pooled_tokens = 10;
            let rewards = 0;

            let pool_info = PoolData {
                multiplier: 1,
                deposit_fee: balance!(0),
                is_core: true,
                is_farm,
                total_tokens_in_pool: 0,
                rewards,
                rewards_to_be_distributed: 0,
                is_removed: false,
            };

            demeter_farming_platform::Pools::<Runtime>::append(
                &pool_asset,
                &reward_asset,
                pool_info,
            );

            let user_info = UserInfo {
                pool_asset,
                reward_asset,
                is_farm,
                pooled_tokens,
                rewards,
            };

            demeter_farming_platform::UserInfos::<Runtime>::append(ALICE, user_info);

            assert_err!(
                demeter_farming_platform::Pallet::<Runtime>::get_rewards(
                    Origin::signed(ALICE),
                    pool_asset,
                    reward_asset,
                    is_farm
                ),
                demeter_farming_platform::Error::<Runtime>::ZeroRewards
            );
        });
    }

    #[test]
    fn get_rewards_pool_does_not_have_rewards() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let pool_asset = XOR;
            let reward_asset = CERES_ASSET_ID;
            let is_farm = true;
            let pooled_tokens = 10;
            let rewards = 2;

            let pool_info = PoolData {
                multiplier: 1,
                deposit_fee: balance!(0),
                is_core: true,
                is_farm,
                total_tokens_in_pool: 0,
                rewards: 1,
                rewards_to_be_distributed: 0,
                is_removed: false,
            };

            demeter_farming_platform::Pools::<Runtime>::append(
                &pool_asset,
                &reward_asset,
                pool_info,
            );

            let user_info = UserInfo {
                pool_asset,
                reward_asset,
                is_farm,
                pooled_tokens,
                rewards,
            };

            demeter_farming_platform::UserInfos::<Runtime>::append(ALICE, user_info);

            assert_err!(
                demeter_farming_platform::Pallet::<Runtime>::get_rewards(
                    Origin::signed(ALICE),
                    pool_asset,
                    reward_asset,
                    is_farm
                ),
                demeter_farming_platform::Error::<Runtime>::PoolDoesNotHaveRewards
            );
        });
    }

    #[test]
    fn get_rewards_ok() {
        preset_initial(|| {
            let pool_asset = XOR;
            let reward_asset = CERES_ASSET_ID;
            let is_farm = true;

            let pool_info = PoolData {
                multiplier: 1,
                deposit_fee: balance!(0),
                is_core: true,
                is_farm,
                total_tokens_in_pool: balance!(1000),
                rewards: balance!(100),
                rewards_to_be_distributed: 0,
                is_removed: false,
            };

            demeter_farming_platform::Pools::<Runtime>::append(
                &pool_asset,
                &reward_asset,
                &pool_info,
            );

            let user_info = UserInfo {
                pool_asset,
                reward_asset,
                is_farm,
                pooled_tokens: balance!(1000),
                rewards: balance!(100),
            };

            demeter_farming_platform::UserInfos::<Runtime>::append(ALICE, user_info);

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::get_rewards(
                Origin::signed(ALICE),
                pool_asset,
                reward_asset,
                is_farm
            ));

            let mut pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(&pool_asset, &reward_asset);
            for p_info in pool_infos.iter_mut() {
                if p_info.is_farm == is_farm {
                    assert_eq!(p_info.rewards, balance!(0))
                }
            }

            let mut user_infos = demeter_farming_platform::UserInfos::<Runtime>::get(ALICE);
            for u_info in user_infos.iter_mut() {
                if u_info.pool_asset == pool_asset
                    && u_info.reward_asset == reward_asset
                    && u_info.is_farm == is_farm
                {
                    assert_eq!(u_info.rewards, balance!(0))
                }
            }

            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &ALICE)
                    .expect("Failed to query free balance."),
                balance!(3100)
            );
        });
    }

    #[test]
    fn withdraw_insufficient_funds() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let pool_asset = XOR;
            let reward_asset = CERES_ASSET_ID;
            let pooled_tokens = 30;
            let is_farm = true;

            let user_info = UserInfo {
                pool_asset,
                reward_asset,
                is_farm,
                pooled_tokens: 20,
                rewards: 1,
            };

            demeter_farming_platform::UserInfos::<Runtime>::append(ALICE, user_info);

            assert_err!(
                demeter_farming_platform::Pallet::<Runtime>::withdraw(
                    Origin::signed(ALICE),
                    pool_asset,
                    reward_asset,
                    pooled_tokens,
                    is_farm
                ),
                demeter_farming_platform::Error::<Runtime>::InsufficientFunds
            );
        });
    }

    #[test]
    fn withdraw_ok() {
        preset_initial(|| {
            let pool_asset = XOR;
            let reward_asset = CERES_ASSET_ID;
            let pooled_tokens = balance!(30);
            let is_farm = false;

            let pool_info = PoolData {
                multiplier: 1,
                deposit_fee: balance!(0),
                is_core: true,
                is_farm,
                total_tokens_in_pool: balance!(1000),
                rewards: balance!(100),
                rewards_to_be_distributed: 0,
                is_removed: false,
            };

            demeter_farming_platform::Pools::<Runtime>::append(
                &pool_asset,
                &reward_asset,
                &pool_info,
            );

            let user_info = UserInfo {
                pool_asset,
                reward_asset,
                is_farm,
                pooled_tokens,
                rewards: 1,
            };

            demeter_farming_platform::UserInfos::<Runtime>::append(ALICE, user_info);

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::withdraw(
                Origin::signed(ALICE),
                pool_asset,
                reward_asset,
                pooled_tokens,
                is_farm
            ));

            let mut user_infos = demeter_farming_platform::UserInfos::<Runtime>::get(&ALICE);

            for user_info in user_infos.iter_mut() {
                if user_info.pool_asset == pool_asset
                    && user_info.reward_asset == reward_asset
                    && user_info.is_farm == is_farm
                {
                    assert_eq!(user_info.pooled_tokens, balance!(0));
                }
            }

            let mut pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(&pool_asset, &reward_asset);
            for p_info in pool_infos.iter_mut() {
                if p_info.is_farm == is_farm {
                    assert_eq!(p_info.total_tokens_in_pool, balance!(970))
                }
            }

            assert_eq!(
                Assets::free_balance(&XOR, &ALICE).expect("Failed to query free balance."),
                balance!(2030)
            );
        });
    }

    #[test]
    fn remove_pool_unauthorized() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let pool_asset = XOR;
            let reward_asset = CERES_ASSET_ID;
            let is_farm = true;

            assert_err!(
                demeter_farming_platform::Pallet::<Runtime>::remove_pool(
                    Origin::signed(ALICE),
                    pool_asset,
                    reward_asset,
                    is_farm
                ),
                demeter_farming_platform::Error::<Runtime>::Unauthorized
            );
        });
    }

    #[test]
    fn remove_pool_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let pool_asset = XOR;
            let reward_asset = CERES_ASSET_ID;
            let is_farm = true;

            let pool_info = PoolData {
                multiplier: 1,
                deposit_fee: balance!(0),
                is_core: true,
                is_farm,
                total_tokens_in_pool: 0,
                rewards: 100,
                rewards_to_be_distributed: 0,
                is_removed: false,
            };

            demeter_farming_platform::Pools::<Runtime>::append(
                &pool_asset,
                &reward_asset,
                &pool_info,
            );

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::remove_pool(
                Origin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                pool_asset,
                reward_asset,
                is_farm
            ));

            let mut pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(&pool_asset, &reward_asset);
            for pool_info in pool_infos.iter_mut() {
                if pool_info.is_farm == is_farm {
                    pool_info.is_removed = true;
                }
                assert_eq!(pool_info.is_removed, true);
            }
        });
    }

    #[test]
    fn change_pool_multiplier_unauthorized() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let pool_asset = XOR;
            let reward_asset = CERES_ASSET_ID;
            let is_farm = true;
            let new_multiplier = 1;

            assert_err!(
                demeter_farming_platform::Pallet::<Runtime>::change_pool_multiplier(
                    Origin::signed(ALICE),
                    pool_asset,
                    reward_asset,
                    is_farm,
                    new_multiplier
                ),
                demeter_farming_platform::Error::<Runtime>::Unauthorized
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
                team_account: BOB,
            };

            demeter_farming_platform::TokenInfos::<Runtime>::insert(&reward_asset, &token_info);

            let pool_info = PoolData {
                multiplier: 1,
                deposit_fee: balance!(0),
                is_core: true,
                is_farm,
                total_tokens_in_pool: 0,
                rewards: 0,
                rewards_to_be_distributed: 0,
                is_removed: false,
            };

            demeter_farming_platform::Pools::<Runtime>::append(
                &pool_asset,
                &reward_asset,
                pool_info,
            );

            assert_err!(
                demeter_farming_platform::Pallet::<Runtime>::change_pool_multiplier(
                    Origin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                    pool_asset,
                    reward_asset,
                    is_farm,
                    new_multiplier
                ),
                demeter_farming_platform::Error::<Runtime>::InvalidMultiplier
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
                team_account: BOB,
            };

            demeter_farming_platform::TokenInfos::<Runtime>::insert(&reward_asset, &token_info);

            assert_err!(
                demeter_farming_platform::Pallet::<Runtime>::change_pool_multiplier(
                    Origin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                    pool_asset,
                    reward_asset,
                    is_farm,
                    new_multiplier
                ),
                demeter_farming_platform::Error::<Runtime>::PoolDoesNotExist
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
                team_account: BOB,
            };

            demeter_farming_platform::TokenInfos::<Runtime>::insert(&reward_asset, &token_info);

            let pool_info = PoolData {
                multiplier: 1,
                deposit_fee: balance!(0),
                is_core: true,
                is_farm,
                total_tokens_in_pool: 0,
                rewards: 0,
                rewards_to_be_distributed: 0,
                is_removed: false,
            };

            demeter_farming_platform::Pools::<Runtime>::append(
                &pool_asset,
                &reward_asset,
                &pool_info,
            );

            assert_ok!(
                demeter_farming_platform::Pallet::<Runtime>::change_pool_multiplier(
                    Origin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                    pool_asset,
                    reward_asset,
                    is_farm,
                    new_multiplier
                )
            );

            token_info = demeter_farming_platform::TokenInfos::<Runtime>::get(&reward_asset)
                .unwrap_or_default();
            let mut pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(&pool_asset, &reward_asset);
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
                team_account: BOB,
            };

            demeter_farming_platform::TokenInfos::<Runtime>::insert(&reward_asset, &token_info);

            let pool_info = PoolData {
                multiplier: 1,
                deposit_fee: balance!(0),
                is_core: true,
                is_farm,
                total_tokens_in_pool: 0,
                rewards: 0,
                rewards_to_be_distributed: 0,
                is_removed: false,
            };

            demeter_farming_platform::Pools::<Runtime>::append(
                &pool_asset,
                &reward_asset,
                &pool_info,
            );

            assert_ok!(
                demeter_farming_platform::Pallet::<Runtime>::change_pool_multiplier(
                    Origin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                    pool_asset,
                    reward_asset,
                    is_farm,
                    new_multiplier
                )
            );

            token_info = demeter_farming_platform::TokenInfos::<Runtime>::get(&reward_asset)
                .unwrap_or_default();
            let mut pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(&pool_asset, &reward_asset);
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
                demeter_farming_platform::Pallet::<Runtime>::change_pool_deposit_fee(
                    Origin::signed(ALICE),
                    pool_asset,
                    reward_asset,
                    is_farm,
                    deposit_fee
                ),
                demeter_farming_platform::Error::<Runtime>::Unauthorized
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
                demeter_farming_platform::Pallet::<Runtime>::change_pool_deposit_fee(
                    Origin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                    pool_asset,
                    reward_asset,
                    is_farm,
                    deposit_fee
                ),
                demeter_farming_platform::Error::<Runtime>::PoolDoesNotExist
            )
        });
    }

    #[test]
    fn change_pool_deposit_fee_invalid_deposit_fee() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let pool_asset = XOR;
            let reward_asset = CERES_ASSET_ID;
            let is_farm = true;
            let mut deposit_fee = balance!(0.8);

            let pool_info = PoolData {
                multiplier: 1,
                deposit_fee,
                is_core: true,
                is_farm,
                total_tokens_in_pool: 0,
                rewards: 0,
                rewards_to_be_distributed: 0,
                is_removed: false,
            };

            demeter_farming_platform::Pools::<Runtime>::append(
                &pool_asset,
                &reward_asset,
                &pool_info,
            );

            deposit_fee = balance!(1.2);

            assert_err!(
                demeter_farming_platform::Pallet::<Runtime>::change_pool_deposit_fee(
                    Origin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                    pool_asset,
                    reward_asset,
                    is_farm,
                    deposit_fee
                ),
                demeter_farming_platform::Error::<Runtime>::InvalidDepositFee
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
            let mut deposit_fee = balance!(1);

            let pool_info = PoolData {
                multiplier: 1,
                deposit_fee,
                is_core: true,
                is_farm,
                total_tokens_in_pool: 0,
                rewards: 0,
                rewards_to_be_distributed: 0,
                is_removed: false,
            };

            demeter_farming_platform::Pools::<Runtime>::append(
                &pool_asset,
                &reward_asset,
                &pool_info,
            );

            deposit_fee = balance!(0.8);
            assert_ok!(
                demeter_farming_platform::Pallet::<Runtime>::change_pool_deposit_fee(
                    Origin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                    pool_asset,
                    reward_asset,
                    is_farm,
                    deposit_fee
                )
            );

            let pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(&pool_asset, &reward_asset);
            for p_info in pool_infos {
                if !p_info.is_removed && p_info.is_farm == is_farm {
                    assert_eq!(p_info.deposit_fee, deposit_fee)
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
                demeter_farming_platform::Pallet::<Runtime>::change_token_info(
                    Origin::signed(ALICE),
                    pool_asset,
                    token_per_block,
                    farms_allocation,
                    staking_allocation,
                    team_allocation,
                    BOB
                ),
                demeter_farming_platform::Error::<Runtime>::Unauthorized
            )
        });
    }

    #[test]
    fn change_token_info_reward_token_is_not_registered() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let reward_asset = CERES_ASSET_ID;
            let token_per_block = balance!(1);
            let farms_allocation = balance!(0.2);
            let staking_allocation = balance!(0.4);
            let team_allocation = balance!(0.4);

            assert_err!(
                demeter_farming_platform::Pallet::<Runtime>::change_token_info(
                    Origin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                    reward_asset,
                    token_per_block,
                    farms_allocation,
                    staking_allocation,
                    team_allocation,
                    BOB
                ),
                demeter_farming_platform::Error::<Runtime>::RewardTokenIsNotRegistered
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
                team_account: BOB,
            };

            demeter_farming_platform::TokenInfos::<Runtime>::insert(&reward_asset, &token_info);

            assert_err!(
                demeter_farming_platform::Pallet::<Runtime>::change_token_info(
                    Origin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                    reward_asset,
                    token_per_block,
                    farms_allocation,
                    staking_allocation,
                    team_allocation,
                    BOB
                ),
                demeter_farming_platform::Error::<Runtime>::TokenPerBlockCantBeZero
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
                team_account: BOB,
            };

            demeter_farming_platform::TokenInfos::<Runtime>::insert(&reward_asset, &token_info);

            assert_err!(
                demeter_farming_platform::Pallet::<Runtime>::change_token_info(
                    Origin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                    reward_asset,
                    token_per_block,
                    farms_allocation,
                    staking_allocation,
                    team_allocation,
                    BOB
                ),
                demeter_farming_platform::Error::<Runtime>::InvalidAllocationParameters
            )
        });
    }

    #[test]
    fn change_token_info_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
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
                team_account: BOB,
            };

            demeter_farming_platform::TokenInfos::<Runtime>::insert(&reward_asset, &token_info);

            assert_ok!(
                demeter_farming_platform::Pallet::<Runtime>::change_token_info(
                    Origin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                    reward_asset,
                    token_per_block,
                    farms_allocation,
                    team_allocation,
                    staking_allocation,
                    BOB
                )
            );

            assert_eq!(token_info.token_per_block, token_per_block);
            assert_eq!(token_info.farms_allocation, farms_allocation);
            assert_eq!(token_info.staking_allocation, staking_allocation);
            assert_eq!(token_info.team_allocation, team_allocation);
        });
    }

    #[test]
    fn change_total_tokens_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let pool_asset = XOR;
            let reward_asset = CERES_ASSET_ID;
            let is_farm = true;
            let total_tokens = balance!(200);

            let pool_info = PoolData {
                multiplier: 1,
                deposit_fee: balance!(0),
                is_core: true,
                is_farm,
                total_tokens_in_pool: balance!(100),
                rewards: 0,
                rewards_to_be_distributed: 0,
                is_removed: false,
            };

            demeter_farming_platform::Pools::<Runtime>::append(
                &pool_asset,
                &reward_asset,
                &pool_info,
            );

            assert_ok!(
                demeter_farming_platform::Pallet::<Runtime>::change_total_tokens(
                    Origin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                    pool_asset,
                    reward_asset,
                    is_farm,
                    total_tokens
                )
            );

            let mut pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(&pool_asset, &reward_asset);
            for pool_info in pool_infos.iter_mut() {
                if pool_info.is_farm == is_farm && !pool_info.is_removed {
                    assert_eq!(pool_info.total_tokens_in_pool, total_tokens);
                }
            }
        });
    }

    #[test]
    fn change_info_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let pool_asset = XOR;
            let reward_asset = CERES_ASSET_ID;
            let is_farm = true;
            let pooled_tokens = 10;
            let rewards = 1;

            let user_info = UserInfo {
                pool_asset,
                reward_asset,
                is_farm,
                pooled_tokens,
                rewards,
            };

            demeter_farming_platform::UserInfos::<Runtime>::append(ALICE, user_info);

            let pool_tokens = balance!(69);
            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::change_info(
                Origin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                ALICE.into(),
                pool_asset,
                reward_asset,
                is_farm,
                pool_tokens
            ));

            let user_info_alice = demeter_farming_platform::UserInfos::<Runtime>::get(&ALICE);
            for user_info in &user_info_alice {
                if user_info.pool_asset == pool_asset
                    && user_info.reward_asset == reward_asset
                    && user_info.is_farm == is_farm
                {
                    assert_eq!(user_info.pooled_tokens, pool_tokens);
                }
            }
        });
    }

    #[test]
    fn on_initialize_ok() {
        preset_initial(|| {
            let dex_id = DEX_A_ID;
            let xor = XOR;
            let ceres = CERES_ASSET_ID;
            let deo = DEMETER_ASSET_ID;
            let util = AssetId32::from_bytes(hex!(
                "007348eb8f0f3cec730fbf5eec1b6a842c54d1df8bed75a9df084d5ee013e814"
            ));

            let is_farm = true;
            let deposit_fee = balance!(0.04);
            let is_core = true;
            let token_per_block = balance!(1);
            let farms_allocation = balance!(0.6);
            let staking_allocation = balance!(0.36);
            let team_allocation = balance!(0.04);

            // Register DEO
            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::register_token(
                Origin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                deo,
                token_per_block,
                farms_allocation,
                staking_allocation,
                team_allocation,
                BOB
            ));

            let token_per_block = balance!(0.01);
            let farms_allocation = balance!(0.5);
            let staking_allocation = balance!(0.4);
            let team_allocation = balance!(0.1);
            // Register UTIL
            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::register_token(
                Origin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                util,
                token_per_block,
                farms_allocation,
                staking_allocation,
                team_allocation,
                CHARLES
            ));

            // XOR/CERES - reward DEO
            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::add_pool(
                Origin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                ceres,
                deo,
                is_farm,
                10,
                deposit_fee,
                is_core
            ));

            // XOR/CERES - reward UTIL
            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::add_pool(
                Origin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                ceres,
                util,
                is_farm,
                5,
                deposit_fee,
                is_core
            ));

            // CERES - reward DEO
            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::add_pool(
                Origin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                ceres,
                deo,
                !is_farm,
                1,
                deposit_fee,
                is_core
            ));

            // CERES - reward UTIL
            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::add_pool(
                Origin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                ceres,
                util,
                !is_farm,
                1,
                deposit_fee,
                is_core
            ));

            let pallet_account = ModuleId(*b"deofarms").into_account();
            assert_ok!(assets::Pallet::<Runtime>::transfer_from(
                &util,
                &ALICE,
                &pallet_account,
                balance!(1500),
            ));

            assert_ok!(pool_xyk::Pallet::<Runtime>::deposit_liquidity(
                Origin::signed(ALICE),
                dex_id,
                xor,
                ceres,
                balance!(500),
                balance!(700),
                balance!(500),
                balance!(700),
            ));

            assert_ok!(pool_xyk::Pallet::<Runtime>::deposit_liquidity(
                Origin::signed(BOB),
                dex_id,
                xor,
                ceres,
                balance!(500),
                balance!(700),
                balance!(500),
                balance!(700),
            ));

            // DEPOSIT TO XOR/CERES POOL - reward DEO
            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::deposit(
                Origin::signed(ALICE),
                ceres,
                deo,
                is_farm,
                balance!(2)
            ));

            // DEPOSIT TO XOR/CERES POOL - reward DEO
            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::deposit(
                Origin::signed(BOB),
                ceres,
                deo,
                is_farm,
                balance!(2)
            ));

            // DEPOSIT TO XOR/CERES POOL - reward UTIL
            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::deposit(
                Origin::signed(ALICE),
                ceres,
                util,
                is_farm,
                balance!(1)
            ));

            // DEPOSIT TO CERES POOL - reward DEO
            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::deposit(
                Origin::signed(BOB),
                ceres,
                deo,
                !is_farm,
                balance!(100)
            ));

            // DEPOSIT TO CERES POOL - reward UTIL
            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::deposit(
                Origin::signed(BOB),
                ceres,
                util,
                !is_farm,
                balance!(100)
            ));

            run_to_block(16201);

            // Check XOR/CERES pool and CERES pool - reward DEO
            let pool_infos = demeter_farming_platform::Pools::<Runtime>::get(&ceres, &deo);
            for pool_info in pool_infos {
                if pool_info.is_farm {
                    assert_eq!(pool_info.total_tokens_in_pool, balance!(3.84));
                    assert_eq!(pool_info.rewards_to_be_distributed, balance!(8640));
                    assert_eq!(pool_info.rewards, balance!(1080));
                } else {
                    assert_eq!(pool_info.total_tokens_in_pool, balance!(96));
                    assert_eq!(pool_info.rewards_to_be_distributed, balance!(5184));
                    assert_eq!(pool_info.rewards, balance!(648));
                }
            }
            let token_info =
                demeter_farming_platform::TokenInfos::<Runtime>::get(&deo).unwrap_or_default();
            assert_eq!(
                assets::Module::<Runtime>::free_balance(&deo, &token_info.team_account).unwrap(),
                balance!(576)
            );
            let user_info_alice = demeter_farming_platform::UserInfos::<Runtime>::get(&ALICE);
            for user_info in &user_info_alice {
                if user_info.pool_asset == ceres
                    && user_info.reward_asset == deo
                    && user_info.is_farm
                {
                    assert_eq!(user_info.rewards, balance!(540));
                }
            }
            let user_info_bob = demeter_farming_platform::UserInfos::<Runtime>::get(&BOB);
            for user_info in &user_info_bob {
                if user_info.pool_asset == ceres
                    && user_info.reward_asset == deo
                    && user_info.is_farm
                {
                    assert_eq!(user_info.rewards, balance!(540));
                } else if user_info.pool_asset == ceres
                    && user_info.reward_asset == deo
                    && !user_info.is_farm
                {
                    assert_eq!(user_info.rewards, balance!(648));
                }
            }

            // Check XOR/CERES pool and CERES pool - reward UTIL
            let pool_infos = demeter_farming_platform::Pools::<Runtime>::get(&ceres, &util);
            for pool_info in pool_infos {
                if pool_info.is_farm {
                    assert_eq!(pool_info.total_tokens_in_pool, balance!(0.96));
                    assert_eq!(pool_info.rewards_to_be_distributed, balance!(72));
                    assert_eq!(pool_info.rewards, balance!(9));
                } else {
                    assert_eq!(pool_info.total_tokens_in_pool, balance!(96));
                    assert_eq!(pool_info.rewards_to_be_distributed, balance!(57.6));
                    assert_eq!(pool_info.rewards, balance!(7.2));
                }
            }
            let token_info =
                demeter_farming_platform::TokenInfos::<Runtime>::get(&util).unwrap_or_default();
            assert_eq!(
                assets::Module::<Runtime>::free_balance(&util, &token_info.team_account).unwrap(),
                balance!(14.4)
            );
            for user_info in &user_info_alice {
                if user_info.pool_asset == ceres
                    && user_info.reward_asset == util
                    && user_info.is_farm
                {
                    assert_eq!(user_info.rewards, balance!(9));
                }
            }
            for user_info in &user_info_bob {
                if user_info.pool_asset == ceres
                    && user_info.reward_asset == util
                    && !user_info.is_farm
                {
                    assert_eq!(user_info.rewards, balance!(7.2));
                }
            }

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::get_rewards(
                Origin::signed(ALICE),
                ceres,
                deo,
                is_farm
            ));

            assert_eq!(
                Assets::free_balance(&deo, &ALICE).expect("Failed to query free balance."),
                balance!(540)
            );

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::get_rewards(
                Origin::signed(BOB),
                ceres,
                util,
                !is_farm
            ));

            assert_eq!(
                Assets::free_balance(&util, &BOB).expect("Failed to query free balance."),
                balance!(7.2)
            );
        });
    }

    #[test]
    fn check_if_has_enough_liquidity_out_of_farming_true() {
        preset_initial(|| {
            let xor: AssetId = XOR.into();
            let ceres: AssetId = CERES_ASSET_ID.into();

            assert_ok!(pool_xyk::Pallet::<Runtime>::deposit_liquidity(
                Origin::signed(ALICE),
                DEX_A_ID.into(),
                xor,
                ceres,
                balance!(500),
                balance!(700),
                balance!(500),
                balance!(700),
            ));

            assert_eq!(
                demeter_farming_platform::Pallet::<Runtime>::check_if_has_enough_liquidity_out_of_farming(
                    &ALICE,
                    xor,
                    ceres,
                    balance!(0.3),
                ),
                true
            );
        });
    }

    #[test]
    fn check_if_has_enough_liquidity_out_of_farming_false() {
        preset_initial(|| {
            let dex_id = DEX_A_ID;
            let pool_asset = XOR;
            let reward_asset = CERES_ASSET_ID;
            let is_farm = true;
            let multiplier = 1;
            let deposit_fee = balance!(0.04);
            let is_core = true;
            let token_per_block = balance!(1);
            let farms_allocation = balance!(0.6);
            let staking_allocation = balance!(0.2);
            let team_allocation = balance!(0.2);

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::register_token(
                Origin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                reward_asset,
                token_per_block,
                farms_allocation,
                staking_allocation,
                team_allocation,
                BOB
            ));

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::add_pool(
                Origin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                reward_asset,
                reward_asset,
                is_farm,
                multiplier,
                deposit_fee,
                is_core
            ));

            assert_ok!(pool_xyk::Pallet::<Runtime>::deposit_liquidity(
                Origin::signed(ALICE),
                dex_id,
                pool_asset,
                reward_asset,
                balance!(500),
                balance!(700),
                balance!(500),
                balance!(700),
            ));

            // Get pool account
            let pool_account: AccountId =
                <Runtime as ceres_liquidity_locker::Config>::XYKPool::properties(
                    pool_asset,
                    reward_asset,
                )
                .expect("Pool does not exist")
                .0;

            // Calculate number of pool tokens of user's account
            let pool_tokens: Balance =
                <Runtime as ceres_liquidity_locker::Config>::XYKPool::pool_providers(
                    pool_account.clone(),
                    ALICE,
                )
                .expect("User is not pool provider");

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::deposit(
                Origin::signed(ALICE),
                reward_asset,
                reward_asset,
                is_farm,
                pool_tokens - balance!(1)
            ));

            assert_eq!(
                demeter_farming_platform::Pallet::<Runtime>::check_if_has_enough_liquidity_out_of_farming(
                    &ALICE,
                    pool_asset,
                    reward_asset,
                    pool_tokens,
                ),
                false
            );
        });
    }
}
