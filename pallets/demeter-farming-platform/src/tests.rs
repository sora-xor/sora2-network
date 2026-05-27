mod tests {
    use crate::mock::*;
    use crate::{AccountIdOf, AssetIdOf};
    use common::prelude::FixedWrapper;
    use common::{
        balance, generate_storage_instance, AssetId32, AssetInfoProvider, AssetName, AssetSymbol,
        Balance, DemeterFarming, LiquiditySourceType, OnDenominate, PredefinedAssetId,
        ToFeeAccount, TradingPairSourceManager, XykPool, CERES_ASSET_ID, DEFAULT_BALANCE_PRECISION,
        DEMETER_ASSET_ID, TBCD, XOR, XSTUSD,
    };
    use demeter_farming_platform::{PoolData, TokenInfo, UserInfo};
    use frame_support::pallet_prelude::{StorageDoubleMap, StorageMap};
    use frame_support::storage::types::ValueQuery;
    use frame_support::traits::Hooks;
    use frame_support::{assert_err, assert_ok, Identity, PalletId};
    use hex_literal::hex;
    use sp_runtime::traits::AccountIdConversion;

    fn preset_initial<Fun>(tests: Fun)
    where
        Fun: Fn(),
    {
        let mut ext = ExtBuilder::default().build();
        let dex_id = DEX_A_ID;
        let dex_id_xst = DEX_B_ID;
        let xor: AssetId = XOR.into();
        let ceres: AssetId = CERES_ASSET_ID.into();
        let xstusd: AssetId = XSTUSD.into();
        let util: AssetId32<PredefinedAssetId> = AssetId32::from_bytes(hex!(
            "007348eb8f0f3cec730fbf5eec1b6a842c54d1df8bed75a9df084d5ee013e814"
        ));
        let pallet_account = PalletId(*b"deofarms").into_account_truncating();

        ext.execute_with(|| {
            assert_ok!(assets::Pallet::<Runtime>::register_asset_id(
                ALICE,
                XOR.into(),
                AssetSymbol(b"XOR".to_vec()),
                AssetName(b"SORA".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                Balance::from(0u32),
                true,
                common::AssetType::Regular,
                None,
                None,
            ));

            assert_ok!(assets::Pallet::<Runtime>::register_asset_id(
                ALICE,
                CERES_ASSET_ID.into(),
                AssetSymbol(b"CERES".to_vec()),
                AssetName(b"Ceres".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                Balance::from(0u32),
                true,
                common::AssetType::Regular,
                None,
                None,
            ));

            assert_ok!(assets::Pallet::<Runtime>::register_asset_id(
                ALICE,
                XSTUSD.into(),
                AssetSymbol(b"XSTUSD".to_vec()),
                AssetName(b"SORA Synthetic USD".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                Balance::from(0u32),
                true,
                common::AssetType::Regular,
                None,
                None,
            ));

            frame_system::Pallet::<Runtime>::inc_providers(
                &demeter_farming_platform::AuthorityAccount::<Runtime>::get(),
            );
            assert_ok!(assets::Pallet::<Runtime>::register_asset_id(
                demeter_farming_platform::AuthorityAccount::<Runtime>::get(),
                DEMETER_ASSET_ID.into(),
                AssetSymbol(b"DEO".to_vec()),
                AssetName(b"Demeter".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                Balance::from(0u32),
                true,
                common::AssetType::Regular,
                None,
                None,
            ));

            assert_ok!(assets::Pallet::<Runtime>::register_asset_id(
                ALICE,
                util.into(),
                AssetSymbol(b"UTIL".to_vec()),
                AssetName(b"Util".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                Balance::from(0u32),
                true,
                common::AssetType::Regular,
                None,
                None,
            ));

            /************ XOR DEX ************/
            assert_ok!(trading_pair::Pallet::<Runtime>::register(
                RuntimeOrigin::signed(BOB),
                dex_id.clone(),
                XOR.into(),
                CERES_ASSET_ID.into()
            ));

            assert_ok!(pool_xyk::Pallet::<Runtime>::initialize_pool(
                RuntimeOrigin::signed(BOB),
                dex_id.clone(),
                XOR.into(),
                CERES_ASSET_ID.into(),
            ));

            assert!(
                trading_pair::Pallet::<Runtime>::is_source_enabled_for_trading_pair(
                    &dex_id,
                    &XOR.into(),
                    &CERES_ASSET_ID.into(),
                    LiquiditySourceType::XYKPool,
                )
                .expect("Failed to query trading pair status.")
            );

            let (_tpair, tech_acc_id) =
                pool_xyk::Pallet::<Runtime>::tech_account_from_dex_and_asset_pair(
                    dex_id.clone(),
                    XOR.into(),
                    CERES_ASSET_ID.into(),
                )
                .unwrap();

            let fee_acc = tech_acc_id.clone().to_fee_account().unwrap();
            let repr: AccountId =
                technical::Pallet::<Runtime>::tech_account_id_to_account_id(&tech_acc_id).unwrap();
            let fee_repr: AccountId =
                technical::Pallet::<Runtime>::tech_account_id_to_account_id(&fee_acc).unwrap();

            assert_eq!(
                pool_xyk::Pallet::<Runtime>::properties(xor, ceres),
                Some((repr.clone(), fee_repr.clone()))
            );

            /********* XSTUSD DEX ********/
            assert_ok!(trading_pair::Pallet::<Runtime>::register(
                RuntimeOrigin::signed(BOB),
                dex_id_xst.clone(),
                XSTUSD.into(),
                CERES_ASSET_ID.into()
            ));

            assert_ok!(pool_xyk::Pallet::<Runtime>::initialize_pool(
                RuntimeOrigin::signed(BOB),
                dex_id_xst.clone(),
                XSTUSD.into(),
                CERES_ASSET_ID.into(),
            ));

            assert!(
                trading_pair::Pallet::<Runtime>::is_source_enabled_for_trading_pair(
                    &dex_id_xst,
                    &XSTUSD.into(),
                    &CERES_ASSET_ID.into(),
                    LiquiditySourceType::XYKPool,
                )
                .expect("Failed to query trading pair status.")
            );

            let (_tpair_xst, tech_acc_id_xst) =
                pool_xyk::Pallet::<Runtime>::tech_account_from_dex_and_asset_pair(
                    dex_id_xst.clone(),
                    XSTUSD.into(),
                    CERES_ASSET_ID.into(),
                )
                .unwrap();

            let fee_acc_xst = tech_acc_id_xst.clone().to_fee_account().unwrap();
            let repr_xst: AccountId =
                technical::Pallet::<Runtime>::tech_account_id_to_account_id(&tech_acc_id_xst)
                    .unwrap();
            let fee_repr_xst: AccountId =
                technical::Pallet::<Runtime>::tech_account_id_to_account_id(&fee_acc_xst).unwrap();

            assert_eq!(
                pool_xyk::Pallet::<Runtime>::properties(xstusd, ceres),
                Some((repr_xst.clone(), fee_repr_xst.clone()))
            );

            /********** MINTS ***********/
            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &xor,
                &ALICE,
                &ALICE,
                balance!(2000)
            ));

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &xstusd,
                &ALICE,
                &ALICE,
                balance!(2000)
            ));

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &ceres,
                &ALICE,
                &ALICE,
                balance!(2000)
            ));

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &xor,
                &ALICE,
                &BOB,
                balance!(2000)
            ));

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &xstusd,
                &ALICE,
                &BOB,
                balance!(2000)
            ));

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &ceres,
                &ALICE,
                &BOB,
                balance!(2000)
            ));

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &ceres,
                &ALICE,
                &pallet_account,
                balance!(1000)
            ));

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &xor,
                &ALICE,
                &pallet_account,
                balance!(1000)
            ));

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &xstusd,
                &ALICE,
                &pallet_account,
                balance!(1000)
            ));

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &util,
                &ALICE,
                &ALICE,
                balance!(2000)
            ));

            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&xor, &ALICE).unwrap(),
                balance!(2000)
            );
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&xstusd, &ALICE).unwrap(),
                balance!(2000)
            );
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&ceres, &ALICE).unwrap(),
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
                    RuntimeOrigin::signed(ALICE),
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
                    RuntimeOrigin::signed(
                        demeter_farming_platform::AuthorityAccount::<Runtime>::get()
                    ),
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
                    RuntimeOrigin::signed(
                        demeter_farming_platform::AuthorityAccount::<Runtime>::get()
                    ),
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
                    RuntimeOrigin::signed(
                        demeter_farming_platform::AuthorityAccount::<Runtime>::get()
                    ),
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
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                reward_asset,
                token_per_block,
                farms_allocation,
                staking_allocation,
                team_allocation,
                BOB
            ));

            let token_info =
                demeter_farming_platform::TokenInfos::<Runtime>::get(&reward_asset).unwrap();

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
                    RuntimeOrigin::signed(ALICE),
                    pool_asset,
                    pool_asset,
                    reward_asset,
                    is_farm,
                    multiplier,
                    deposit_fee,
                    is_core,
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
                    RuntimeOrigin::signed(
                        demeter_farming_platform::AuthorityAccount::<Runtime>::get()
                    ),
                    pool_asset,
                    pool_asset,
                    reward_asset,
                    is_farm,
                    multiplier,
                    deposit_fee,
                    is_core,
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
                    RuntimeOrigin::signed(
                        demeter_farming_platform::AuthorityAccount::<Runtime>::get()
                    ),
                    pool_asset,
                    pool_asset,
                    reward_asset,
                    is_farm,
                    multiplier,
                    deposit_fee,
                    is_core,
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
                    RuntimeOrigin::signed(
                        demeter_farming_platform::AuthorityAccount::<Runtime>::get()
                    ),
                    pool_asset,
                    pool_asset,
                    reward_asset,
                    is_farm,
                    multiplier,
                    deposit_fee,
                    is_core,
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
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
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
                reward_per_token: 0,
                is_removed: false,
                base_asset: XOR,
            };

            demeter_farming_platform::Pools::<Runtime>::append(
                &pool_asset,
                &reward_asset,
                pool_info,
            );

            assert_err!(
                demeter_farming_platform::Pallet::<Runtime>::add_pool(
                    RuntimeOrigin::signed(
                        demeter_farming_platform::AuthorityAccount::<Runtime>::get()
                    ),
                    pool_asset,
                    pool_asset,
                    reward_asset,
                    is_farm,
                    multiplier,
                    deposit_fee,
                    is_core,
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
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                reward_asset,
                token_per_block,
                farms_allocation,
                staking_allocation,
                team_allocation,
                BOB
            ));

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::add_pool(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                pool_asset,
                pool_asset,
                reward_asset,
                is_farm,
                multiplier,
                deposit_fee,
                is_core,
            ));

            let token_info =
                demeter_farming_platform::TokenInfos::<Runtime>::get(&reward_asset).unwrap();
            assert_eq!(token_info.farms_total_multiplier, multiplier);

            let pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(&pool_asset, &reward_asset);
            for pool_info in pool_infos {
                if !pool_info.is_removed
                    && pool_info.is_farm == is_farm
                    && pool_info.base_asset == pool_asset
                {
                    assert_eq!(pool_info.multiplier, multiplier);
                    assert_eq!(pool_info.is_core, is_core);
                    assert_eq!(pool_info.deposit_fee, deposit_fee);
                }
            }
        });
    }

    #[test]
    fn add_pool_xstusd_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let pool_asset = XSTUSD;
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
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                reward_asset,
                token_per_block,
                farms_allocation,
                staking_allocation,
                team_allocation,
                BOB
            ));

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::add_pool(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                pool_asset,
                pool_asset,
                reward_asset,
                is_farm,
                multiplier,
                deposit_fee,
                is_core,
            ));

            let token_info =
                demeter_farming_platform::TokenInfos::<Runtime>::get(&reward_asset).unwrap();
            assert_eq!(token_info.farms_total_multiplier, multiplier);

            let pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(&pool_asset, &reward_asset);
            for pool_info in pool_infos {
                if !pool_info.is_removed
                    && pool_info.is_farm == is_farm
                    && pool_info.base_asset == pool_asset
                {
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
                    RuntimeOrigin::signed(ALICE),
                    pool_asset,
                    pool_asset,
                    reward_asset,
                    is_farm,
                    balance!(10),
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
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                reward_asset,
                token_per_block,
                farms_allocation,
                staking_allocation,
                team_allocation,
                BOB
            ));

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::add_pool(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                pool_asset,
                pool_asset,
                reward_asset,
                is_farm,
                multiplier,
                deposit_fee,
                is_core,
            ));

            assert_err!(
                demeter_farming_platform::Pallet::<Runtime>::deposit(
                    RuntimeOrigin::signed(ALICE),
                    pool_asset,
                    pool_asset,
                    reward_asset,
                    is_farm,
                    balance!(10000),
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
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                reward_asset,
                token_per_block,
                farms_allocation,
                staking_allocation,
                team_allocation,
                BOB
            ));

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::add_pool(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                pool_asset,
                pool_asset,
                reward_asset,
                is_farm,
                multiplier,
                deposit_fee,
                is_core,
            ));

            assert_err!(
                demeter_farming_platform::Pallet::<Runtime>::deposit(
                    RuntimeOrigin::signed(ALICE),
                    pool_asset,
                    pool_asset,
                    reward_asset,
                    is_farm,
                    balance!(10000),
                ),
                demeter_farming_platform::Error::<Runtime>::PoolDoesNotExist
            );
        });
    }

    #[test]
    fn deposit_insufficient_lp_tokens() {
        preset_initial(|| {
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
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                reward_asset,
                token_per_block,
                farms_allocation,
                staking_allocation,
                team_allocation,
                BOB
            ));

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::add_pool(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                pool_asset,
                reward_asset,
                reward_asset,
                is_farm,
                multiplier,
                deposit_fee,
                is_core,
            ));

            let pooled_tokens = balance!(10000);
            assert_err!(
                demeter_farming_platform::Pallet::<Runtime>::deposit(
                    RuntimeOrigin::signed(ALICE),
                    pool_asset,
                    reward_asset,
                    reward_asset,
                    is_farm,
                    pooled_tokens,
                ),
                demeter_farming_platform::Error::<Runtime>::InsufficientLPTokens
            );
        });
    }

    #[test]
    fn deposit_zero_amount_does_not_create_user_info() {
        preset_initial(|| {
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
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                reward_asset,
                token_per_block,
                farms_allocation,
                staking_allocation,
                team_allocation,
                BOB
            ));

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::add_pool(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                reward_asset,
                reward_asset,
                reward_asset,
                is_farm,
                multiplier,
                deposit_fee,
                is_core,
            ));

            assert_err!(
                demeter_farming_platform::Pallet::<Runtime>::deposit(
                    RuntimeOrigin::signed(ALICE),
                    reward_asset,
                    reward_asset,
                    reward_asset,
                    is_farm,
                    0,
                ),
                demeter_farming_platform::Error::<Runtime>::ZeroDeposit
            );

            assert!(demeter_farming_platform::UserInfos::<Runtime>::get(&ALICE).is_empty());
            let pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(&reward_asset, &reward_asset);
            assert_eq!(pool_infos[0].total_tokens_in_pool, 0);
        });
    }

    #[test]
    fn deposit_net_zero_after_fee_does_not_transfer_or_create_user_info() {
        preset_initial(|| {
            let reward_asset = CERES_ASSET_ID;
            let is_farm = false;
            let multiplier = 1;
            let deposit_fee = balance!(1);
            let is_core = true;
            let token_per_block = balance!(1);
            let farms_allocation = balance!(0.6);
            let staking_allocation = balance!(0.2);
            let team_allocation = balance!(0.2);
            let pallet_account = PalletId(*b"deofarms").into_account_truncating();
            let fee_account = demeter_farming_platform::FeeAccount::<Runtime>::get();
            let alice_before = Assets::free_balance(&reward_asset, &ALICE).unwrap();
            let pallet_before = Assets::free_balance(&reward_asset, &pallet_account).unwrap();
            let fee_before = Assets::free_balance(&reward_asset, &fee_account).unwrap_or(0);

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::register_token(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                reward_asset,
                token_per_block,
                farms_allocation,
                staking_allocation,
                team_allocation,
                BOB
            ));

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::add_pool(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                reward_asset,
                reward_asset,
                reward_asset,
                is_farm,
                multiplier,
                deposit_fee,
                is_core,
            ));

            assert_err!(
                demeter_farming_platform::Pallet::<Runtime>::deposit(
                    RuntimeOrigin::signed(ALICE),
                    reward_asset,
                    reward_asset,
                    reward_asset,
                    is_farm,
                    balance!(10),
                ),
                demeter_farming_platform::Error::<Runtime>::ZeroDeposit
            );

            assert!(demeter_farming_platform::UserInfos::<Runtime>::get(&ALICE).is_empty());
            assert_eq!(
                Assets::free_balance(&reward_asset, &ALICE).unwrap(),
                alice_before
            );
            assert_eq!(
                Assets::free_balance(&reward_asset, &pallet_account).unwrap(),
                pallet_before
            );
            assert_eq!(
                Assets::free_balance(&reward_asset, &fee_account).unwrap_or(0),
                fee_before
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
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                reward_asset,
                token_per_block,
                farms_allocation,
                staking_allocation,
                team_allocation,
                BOB
            ));

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::add_pool(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                reward_asset,
                reward_asset,
                reward_asset,
                is_farm,
                multiplier,
                deposit_fee,
                is_core,
            ));

            let mut pooled_tokens = balance!(10);
            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::deposit(
                RuntimeOrigin::signed(ALICE),
                reward_asset,
                reward_asset,
                reward_asset,
                is_farm,
                pooled_tokens,
            ));

            let fee = (FixedWrapper::from(pooled_tokens) * FixedWrapper::from(deposit_fee))
                .try_into_balance()
                .unwrap_or(0);
            pooled_tokens -= fee;

            let pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(&pool_asset, &reward_asset);
            for p_info in &pool_infos {
                if !p_info.is_removed
                    && p_info.is_farm == is_farm
                    && p_info.base_asset == reward_asset
                {
                    assert_eq!(p_info.total_tokens_in_pool, pooled_tokens);
                }
            }

            let user_infos = demeter_farming_platform::UserInfos::<Runtime>::get(&ALICE);
            for u_info in &user_infos {
                if u_info.is_farm == is_farm && u_info.base_asset == reward_asset {
                    assert_eq!(u_info.pooled_tokens, pooled_tokens);
                }
            }

            assert_eq!(
                Assets::free_balance(&CERES_ASSET_ID, &ALICE)
                    .expect("Failed to query free balance."),
                balance!(2990)
            );

            let pallet_account = PalletId(*b"deofarms").into_account_truncating();
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
            let asset_xor = XOR;
            let asset_ceres = CERES_ASSET_ID;
            let is_farm = true;
            let multiplier = 1;
            let deposit_fee = balance!(0.04);
            let is_core = true;
            let token_per_block = balance!(1);
            let farms_allocation = balance!(0.6);
            let staking_allocation = balance!(0.2);
            let team_allocation = balance!(0.2);

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::register_token(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                asset_ceres,
                token_per_block,
                farms_allocation,
                staking_allocation,
                team_allocation,
                BOB
            ));

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::register_token(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                asset_xor,
                token_per_block,
                farms_allocation,
                staking_allocation,
                team_allocation,
                BOB
            ));

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::add_pool(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                asset_xor,
                asset_ceres,
                asset_ceres,
                is_farm,
                multiplier,
                deposit_fee,
                is_core,
            ));

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::add_pool(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                asset_xor,
                asset_ceres,
                asset_xor,
                is_farm,
                multiplier,
                deposit_fee,
                is_core,
            ));

            assert_ok!(pool_xyk::Pallet::<Runtime>::deposit_liquidity(
                RuntimeOrigin::signed(ALICE),
                dex_id,
                asset_xor,
                asset_ceres,
                balance!(500),
                balance!(700),
                balance!(500),
                balance!(700),
            ));

            // Get pool account
            let pool_account: AccountId =
                <Runtime as ceres_liquidity_locker::Config>::XYKPool::properties(
                    asset_xor,
                    asset_ceres,
                )
                .expect("Pool does not exist")
                .0;

            // Calculate number of pool tokens of user's account
            let mut pooled_tokens: Balance =
                <Runtime as ceres_liquidity_locker::Config>::XYKPool::pool_providers(
                    pool_account.clone(),
                    ALICE,
                )
                .expect("User is not pool provider");

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::deposit(
                RuntimeOrigin::signed(ALICE),
                asset_xor,
                asset_ceres,
                asset_ceres,
                is_farm,
                pooled_tokens,
            ));

            let fee = (FixedWrapper::from(pooled_tokens) * FixedWrapper::from(deposit_fee))
                .try_into_balance()
                .unwrap_or(0);
            pooled_tokens -= fee;

            let mut pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(&asset_ceres, &asset_ceres);
            for p_info in &pool_infos {
                if !p_info.is_removed && p_info.is_farm == is_farm && p_info.base_asset == asset_xor
                {
                    assert_eq!(p_info.total_tokens_in_pool, pooled_tokens);
                }
            }

            let user_infos = demeter_farming_platform::UserInfos::<Runtime>::get(&ALICE);
            for u_info in &user_infos {
                if u_info.is_farm == is_farm && u_info.base_asset == asset_xor {
                    assert_eq!(u_info.pooled_tokens, pooled_tokens);
                }
            }

            let lp_tokens = pool_xyk::Pallet::<Runtime>::balance_of_pool_provider(
                pool_account.clone(),
                demeter_farming_platform::FeeAccount::<Runtime>::get(),
            )
            .unwrap_or(0);
            assert_eq!(lp_tokens, fee);

            // Deposit to other XOR/CERES pool with different reward token
            pooled_tokens = <Runtime as ceres_liquidity_locker::Config>::XYKPool::pool_providers(
                pool_account.clone(),
                ALICE,
            )
            .expect("User is not pool provider");

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::deposit(
                RuntimeOrigin::signed(ALICE),
                asset_xor,
                asset_ceres,
                asset_xor,
                is_farm,
                pooled_tokens,
            ));

            let user_infos = demeter_farming_platform::UserInfos::<Runtime>::get(&ALICE);
            let mut first_pool = balance!(0);
            let mut second_pool = balance!(0);
            for u_info in &user_infos {
                if u_info.pool_asset == asset_ceres
                    && u_info.reward_asset == asset_ceres
                    && u_info.is_farm == is_farm
                    && u_info.base_asset == asset_xor
                {
                    first_pool = u_info.pooled_tokens;
                } else if u_info.pool_asset == asset_ceres
                    && u_info.reward_asset == asset_xor
                    && u_info.is_farm == is_farm
                    && u_info.base_asset == asset_xor
                {
                    second_pool = u_info.pooled_tokens;
                }
            }
            assert_eq!(first_pool, second_pool);

            pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(&asset_ceres, &asset_ceres);
            for p_info in &pool_infos {
                if !p_info.is_removed && p_info.is_farm == is_farm && p_info.base_asset == asset_xor
                {
                    assert_eq!(p_info.total_tokens_in_pool, first_pool);
                }
            }
        });
    }

    #[test]
    fn deposit_xstusd_ok_farm() {
        preset_initial(|| {
            let dex_id = DEX_B_ID;
            let asset_xstusd = XSTUSD;
            let asset_ceres = CERES_ASSET_ID;
            let is_farm = true;
            let multiplier = 1;
            let deposit_fee = balance!(0.04);
            let is_core = true;
            let token_per_block = balance!(1);
            let farms_allocation = balance!(0.6);
            let staking_allocation = balance!(0.2);
            let team_allocation = balance!(0.2);

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::register_token(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                asset_ceres,
                token_per_block,
                farms_allocation,
                staking_allocation,
                team_allocation,
                BOB
            ));

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::register_token(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                asset_xstusd,
                token_per_block,
                farms_allocation,
                staking_allocation,
                team_allocation,
                BOB
            ));

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::add_pool(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                asset_xstusd,
                asset_ceres,
                asset_ceres,
                is_farm,
                multiplier,
                deposit_fee,
                is_core,
            ));

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::add_pool(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                asset_xstusd,
                asset_ceres,
                asset_xstusd,
                is_farm,
                multiplier,
                deposit_fee,
                is_core,
            ));

            assert_ok!(pool_xyk::Pallet::<Runtime>::deposit_liquidity(
                RuntimeOrigin::signed(ALICE),
                dex_id,
                asset_xstusd,
                asset_ceres,
                balance!(500),
                balance!(700),
                balance!(500),
                balance!(700),
            ));

            // Get pool account
            let pool_account: AccountId =
                <Runtime as ceres_liquidity_locker::Config>::XYKPool::properties(
                    asset_xstusd,
                    asset_ceres,
                )
                .expect("Pool does not exist")
                .0;

            // Calculate number of pool tokens of user's account
            let mut pooled_tokens: Balance =
                <Runtime as ceres_liquidity_locker::Config>::XYKPool::pool_providers(
                    pool_account.clone(),
                    ALICE,
                )
                .expect("User is not pool provider");

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::deposit(
                RuntimeOrigin::signed(ALICE),
                asset_xstusd,
                asset_ceres,
                asset_ceres,
                is_farm,
                pooled_tokens,
            ));
            let fee = (FixedWrapper::from(pooled_tokens) * FixedWrapper::from(deposit_fee))
                .try_into_balance()
                .unwrap_or(0);
            pooled_tokens -= fee;

            let mut pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(&asset_ceres, &asset_ceres);
            for p_info in &pool_infos {
                if !p_info.is_removed
                    && p_info.is_farm == is_farm
                    && p_info.base_asset == asset_xstusd
                {
                    assert_eq!(p_info.total_tokens_in_pool, pooled_tokens);
                }
            }

            let user_infos = demeter_farming_platform::UserInfos::<Runtime>::get(&ALICE);
            for u_info in &user_infos {
                if u_info.is_farm == is_farm && u_info.base_asset == asset_xstusd {
                    assert_eq!(u_info.pooled_tokens, pooled_tokens);
                }
            }

            let lp_tokens = pool_xyk::Pallet::<Runtime>::balance_of_pool_provider(
                pool_account.clone(),
                demeter_farming_platform::FeeAccount::<Runtime>::get(),
            )
            .unwrap_or(0);
            assert_eq!(lp_tokens, fee);

            // Deposit to other XSTUSD/CERES pool with different reward token
            pooled_tokens = <Runtime as ceres_liquidity_locker::Config>::XYKPool::pool_providers(
                pool_account.clone(),
                ALICE,
            )
            .expect("User is not pool provider");

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::deposit(
                RuntimeOrigin::signed(ALICE),
                asset_xstusd,
                asset_ceres,
                asset_xstusd,
                is_farm,
                pooled_tokens,
            ));

            let user_infos = demeter_farming_platform::UserInfos::<Runtime>::get(&ALICE);
            let mut first_pool = balance!(0);
            let mut second_pool = balance!(0);
            for u_info in &user_infos {
                if u_info.pool_asset == asset_ceres
                    && u_info.reward_asset == asset_ceres
                    && u_info.is_farm == is_farm
                    && u_info.base_asset == asset_xstusd
                {
                    first_pool = u_info.pooled_tokens;
                } else if u_info.pool_asset == asset_ceres
                    && u_info.reward_asset == asset_xstusd
                    && u_info.is_farm == is_farm
                    && u_info.base_asset == asset_xstusd
                {
                    second_pool = u_info.pooled_tokens;
                }
            }
            assert_eq!(first_pool, second_pool);

            pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(&asset_ceres, &asset_ceres);
            for p_info in &pool_infos {
                if !p_info.is_removed
                    && p_info.is_farm == is_farm
                    && p_info.base_asset == asset_xstusd
                {
                    assert_eq!(p_info.total_tokens_in_pool, first_pool);
                }
            }
        });
    }

    #[test]
    fn get_rewards_pool_does_not_exist() {
        preset_initial(|| {
            let pool_asset = XOR;
            let reward_asset = CERES_ASSET_ID;
            let is_farm = true;
            let pooled_tokens = 10;
            let rewards = 1;

            let user_info = UserInfo {
                base_asset: XOR,
                pool_asset,
                reward_asset,
                is_farm,
                pooled_tokens,
                rewards,
                reward_per_token_paid: 0,
            };

            demeter_farming_platform::UserInfos::<Runtime>::append(ALICE, user_info);

            assert_err!(
                demeter_farming_platform::Pallet::<Runtime>::get_rewards(
                    RuntimeOrigin::signed(ALICE),
                    pool_asset,
                    pool_asset,
                    reward_asset,
                    is_farm,
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
                reward_per_token: 0,
                is_removed: false,
                base_asset: XOR,
            };

            demeter_farming_platform::Pools::<Runtime>::append(
                &pool_asset,
                &reward_asset,
                pool_info,
            );

            let user_info = UserInfo {
                base_asset: XOR,
                pool_asset,
                reward_asset,
                is_farm,
                pooled_tokens,
                rewards,
                reward_per_token_paid: 0,
            };

            demeter_farming_platform::UserInfos::<Runtime>::append(ALICE, user_info);

            assert_err!(
                demeter_farming_platform::Pallet::<Runtime>::get_rewards(
                    RuntimeOrigin::signed(ALICE),
                    pool_asset,
                    pool_asset,
                    reward_asset,
                    is_farm,
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
                reward_per_token: 0,
                is_removed: false,
                base_asset: XOR,
            };

            demeter_farming_platform::Pools::<Runtime>::append(
                &pool_asset,
                &reward_asset,
                pool_info,
            );

            let user_info = UserInfo {
                base_asset: XOR,
                pool_asset,
                reward_asset,
                is_farm,
                pooled_tokens,
                rewards,
                reward_per_token_paid: 0,
            };

            demeter_farming_platform::UserInfos::<Runtime>::append(ALICE, user_info);

            assert_err!(
                demeter_farming_platform::Pallet::<Runtime>::get_rewards(
                    RuntimeOrigin::signed(ALICE),
                    pool_asset,
                    pool_asset,
                    reward_asset,
                    is_farm,
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
                reward_per_token: 0,
                is_removed: false,
                base_asset: XOR,
            };

            demeter_farming_platform::Pools::<Runtime>::append(
                &pool_asset,
                &reward_asset,
                &pool_info,
            );

            let user_info = UserInfo {
                base_asset: XOR,
                pool_asset,
                reward_asset,
                is_farm,
                pooled_tokens: balance!(1000),
                rewards: balance!(100),
                reward_per_token_paid: 0,
            };

            demeter_farming_platform::UserInfos::<Runtime>::append(ALICE, user_info);

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::get_rewards(
                RuntimeOrigin::signed(ALICE),
                pool_asset,
                pool_asset,
                reward_asset,
                is_farm,
            ));

            let mut pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(&pool_asset, &reward_asset);
            for p_info in pool_infos.iter_mut() {
                if p_info.is_farm == is_farm && p_info.base_asset == pool_asset {
                    assert_eq!(p_info.rewards, balance!(0))
                }
            }

            let mut user_infos = demeter_farming_platform::UserInfos::<Runtime>::get(ALICE);
            for u_info in user_infos.iter_mut() {
                if u_info.pool_asset == pool_asset
                    && u_info.reward_asset == reward_asset
                    && u_info.is_farm == is_farm
                    && u_info.base_asset == pool_asset
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
    fn get_rewards_xstusd_ok() {
        preset_initial(|| {
            let pool_asset = XSTUSD;
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
                reward_per_token: 0,
                is_removed: false,
                base_asset: XSTUSD,
            };

            let token_info = TokenInfo {
                farms_total_multiplier: 1,
                staking_total_multiplier: 0,
                token_per_block: balance!(1),
                farms_allocation: balance!(0.6),
                staking_allocation: balance!(0.2),
                team_allocation: balance!(0.2),
                team_account: BOB,
            };
            demeter_farming_platform::TokenInfos::<Runtime>::insert(&reward_asset, &token_info);

            demeter_farming_platform::Pools::<Runtime>::append(
                &pool_asset,
                &reward_asset,
                &pool_info,
            );

            let user_info = UserInfo {
                base_asset: XSTUSD,
                pool_asset,
                reward_asset,
                is_farm,
                pooled_tokens: balance!(1000),
                rewards: balance!(100),
                reward_per_token_paid: 0,
            };

            demeter_farming_platform::UserInfos::<Runtime>::append(ALICE, user_info);

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::get_rewards(
                RuntimeOrigin::signed(ALICE),
                pool_asset,
                pool_asset,
                reward_asset,
                is_farm,
            ));

            let mut pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(&pool_asset, &reward_asset);
            for p_info in pool_infos.iter_mut() {
                if p_info.is_farm == is_farm && p_info.base_asset == pool_asset {
                    assert_eq!(p_info.rewards, balance!(0))
                }
            }

            let mut user_infos = demeter_farming_platform::UserInfos::<Runtime>::get(ALICE);
            for u_info in user_infos.iter_mut() {
                if u_info.pool_asset == pool_asset
                    && u_info.reward_asset == reward_asset
                    && u_info.is_farm == is_farm
                    && u_info.base_asset == pool_asset
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
            let base_asset = XSTUSD;
            let pooled_tokens = 30;
            let is_farm = true;

            let user_info = UserInfo {
                base_asset,
                pool_asset,
                reward_asset,
                is_farm,
                pooled_tokens: 20,
                rewards: 1,
                reward_per_token_paid: 0,
            };

            demeter_farming_platform::UserInfos::<Runtime>::append(ALICE, user_info);

            assert_err!(
                demeter_farming_platform::Pallet::<Runtime>::withdraw(
                    RuntimeOrigin::signed(ALICE),
                    base_asset,
                    pool_asset,
                    reward_asset,
                    pooled_tokens,
                    is_farm,
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
            let base_asset = XSTUSD;
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
                reward_per_token: 0,
                is_removed: false,
                base_asset,
            };

            demeter_farming_platform::Pools::<Runtime>::append(
                &pool_asset,
                &reward_asset,
                &pool_info,
            );

            let user_info = UserInfo {
                base_asset,
                pool_asset,
                reward_asset,
                is_farm,
                pooled_tokens,
                rewards: 1,
                reward_per_token_paid: 0,
            };

            demeter_farming_platform::UserInfos::<Runtime>::append(ALICE, user_info);

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::withdraw(
                RuntimeOrigin::signed(ALICE),
                base_asset,
                pool_asset,
                reward_asset,
                pooled_tokens,
                is_farm,
            ));

            let mut user_infos = demeter_farming_platform::UserInfos::<Runtime>::get(&ALICE);

            for user_info in user_infos.iter_mut() {
                if user_info.pool_asset == pool_asset
                    && user_info.reward_asset == reward_asset
                    && user_info.is_farm == is_farm
                    && user_info.base_asset == base_asset
                {
                    assert_eq!(user_info.pooled_tokens, balance!(0));
                }
            }

            let mut pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(&pool_asset, &reward_asset);
            for p_info in pool_infos.iter_mut() {
                if p_info.is_farm == is_farm && p_info.base_asset == base_asset {
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
    fn withdraw_xstusd_ok() {
        preset_initial(|| {
            let dex_id = DEX_B_ID;
            let asset_xstusd = XSTUSD;
            let asset_ceres = CERES_ASSET_ID;
            let is_farm = true;
            let multiplier = 1;
            let deposit_fee = balance!(0.04);
            let is_core = true;
            let token_per_block = balance!(1);
            let farms_allocation = balance!(0.6);
            let staking_allocation = balance!(0.2);
            let team_allocation = balance!(0.2);

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::register_token(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                asset_ceres,
                token_per_block,
                farms_allocation,
                staking_allocation,
                team_allocation,
                BOB
            ));

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::add_pool(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                asset_xstusd,
                asset_ceres,
                asset_ceres,
                is_farm,
                multiplier,
                deposit_fee,
                is_core,
            ));

            assert_ok!(pool_xyk::Pallet::<Runtime>::deposit_liquidity(
                RuntimeOrigin::signed(ALICE),
                dex_id,
                asset_xstusd,
                asset_ceres,
                balance!(500),
                balance!(700),
                balance!(500),
                balance!(700),
            ));

            // Get pool account
            let pool_account: AccountId =
                <Runtime as ceres_liquidity_locker::Config>::XYKPool::properties(
                    asset_xstusd,
                    asset_ceres,
                )
                .expect("Pool does not exist")
                .0;

            // Calculate number of pool tokens of user's account
            let mut pooled_tokens: Balance =
                <Runtime as ceres_liquidity_locker::Config>::XYKPool::pool_providers(
                    pool_account.clone(),
                    ALICE,
                )
                .expect("User is not pool provider");

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::deposit(
                RuntimeOrigin::signed(ALICE),
                asset_xstusd,
                asset_ceres,
                asset_ceres,
                is_farm,
                pooled_tokens,
            ));

            let fee = (FixedWrapper::from(pooled_tokens) * FixedWrapper::from(deposit_fee))
                .try_into_balance()
                .unwrap_or(0);
            pooled_tokens -= fee;

            let mut pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(&asset_ceres, &asset_ceres);
            for p_info in &pool_infos {
                if !p_info.is_removed
                    && p_info.is_farm == is_farm
                    && p_info.base_asset == asset_xstusd
                {
                    assert_eq!(p_info.total_tokens_in_pool, pooled_tokens);
                }
            }

            let mut user_infos = demeter_farming_platform::UserInfos::<Runtime>::get(&ALICE);
            for u_info in &user_infos {
                if u_info.is_farm == is_farm && u_info.base_asset == asset_xstusd {
                    assert_eq!(u_info.pooled_tokens, pooled_tokens);
                }
            }

            let lp_tokens = pool_xyk::Pallet::<Runtime>::balance_of_pool_provider(
                pool_account.clone(),
                demeter_farming_platform::FeeAccount::<Runtime>::get(),
            )
            .unwrap_or(0);
            assert_eq!(lp_tokens, fee);

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::withdraw(
                RuntimeOrigin::signed(ALICE),
                asset_xstusd,
                asset_ceres,
                asset_ceres,
                pooled_tokens,
                is_farm,
            ));

            user_infos = demeter_farming_platform::UserInfos::<Runtime>::get(&ALICE);

            for user_info in user_infos.iter_mut() {
                if user_info.pool_asset == asset_ceres
                    && user_info.reward_asset == asset_ceres
                    && user_info.is_farm == is_farm
                    && user_info.base_asset == asset_xstusd
                {
                    assert_eq!(user_info.pooled_tokens, balance!(0));
                }
            }

            pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(&asset_ceres, &asset_ceres);
            for p_info in pool_infos.iter_mut() {
                if p_info.is_farm == is_farm && p_info.base_asset == asset_xstusd {
                    assert_eq!(p_info.total_tokens_in_pool, balance!(0))
                }
            }
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
                    RuntimeOrigin::signed(ALICE),
                    pool_asset,
                    pool_asset,
                    reward_asset,
                    is_farm,
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
                rewards_to_be_distributed: balance!(16),
                reward_per_token: 0,
                is_removed: false,
                base_asset: XOR,
            };

            let token_info = TokenInfo {
                farms_total_multiplier: 1,
                staking_total_multiplier: 0,
                token_per_block: balance!(1),
                farms_allocation: balance!(0.6),
                staking_allocation: balance!(0.2),
                team_allocation: balance!(0.2),
                team_account: BOB,
            };
            demeter_farming_platform::TokenInfos::<Runtime>::insert(&reward_asset, &token_info);

            demeter_farming_platform::Pools::<Runtime>::append(
                &pool_asset,
                &reward_asset,
                &pool_info,
            );

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::remove_pool(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                pool_asset,
                pool_asset,
                reward_asset,
                is_farm,
            ));

            let token_info =
                demeter_farming_platform::TokenInfos::<Runtime>::get(&reward_asset).unwrap();
            assert_eq!(token_info.farms_total_multiplier, 0);

            let mut pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(&pool_asset, &reward_asset);
            for pool_info in pool_infos.iter_mut() {
                if pool_info.is_farm == is_farm && pool_info.base_asset == pool_asset {
                    assert!(pool_info.is_removed);
                    assert_eq!(pool_info.rewards_to_be_distributed, 0);
                }
            }
        });
    }

    #[test]
    fn remove_pool_xstusd_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let pool_asset = XSTUSD;
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
                reward_per_token: 0,
                is_removed: false,
                base_asset: XSTUSD,
            };

            let token_info = TokenInfo {
                farms_total_multiplier: 1,
                staking_total_multiplier: 0,
                token_per_block: balance!(1),
                farms_allocation: balance!(0.6),
                staking_allocation: balance!(0.2),
                team_allocation: balance!(0.2),
                team_account: BOB,
            };
            demeter_farming_platform::TokenInfos::<Runtime>::insert(&reward_asset, &token_info);

            demeter_farming_platform::Pools::<Runtime>::append(
                &pool_asset,
                &reward_asset,
                &pool_info,
            );

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::remove_pool(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                pool_asset,
                pool_asset,
                reward_asset,
                is_farm,
            ));

            let token_info =
                demeter_farming_platform::TokenInfos::<Runtime>::get(&reward_asset).unwrap();
            assert_eq!(token_info.farms_total_multiplier, 0);

            let mut pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(&pool_asset, &reward_asset);
            for pool_info in pool_infos.iter_mut() {
                if pool_info.is_farm == is_farm && pool_info.base_asset == pool_asset {
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
                    RuntimeOrigin::signed(ALICE),
                    pool_asset,
                    pool_asset,
                    reward_asset,
                    is_farm,
                    new_multiplier,
                ),
                demeter_farming_platform::Error::<Runtime>::Unauthorized
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
                    RuntimeOrigin::signed(
                        demeter_farming_platform::AuthorityAccount::<Runtime>::get()
                    ),
                    pool_asset,
                    pool_asset,
                    reward_asset,
                    is_farm,
                    new_multiplier,
                ),
                demeter_farming_platform::Error::<Runtime>::PoolDoesNotExist
            )
        });
    }

    #[test]
    fn change_pool_multiplier_zero_is_invalid() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let pool_asset = XOR;
            let reward_asset = CERES_ASSET_ID;
            let is_farm = true;

            assert_err!(
                demeter_farming_platform::Pallet::<Runtime>::change_pool_multiplier(
                    RuntimeOrigin::signed(
                        demeter_farming_platform::AuthorityAccount::<Runtime>::get()
                    ),
                    pool_asset,
                    pool_asset,
                    reward_asset,
                    is_farm,
                    0,
                ),
                demeter_farming_platform::Error::<Runtime>::InvalidMultiplier
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

            let pool_info = PoolData {
                multiplier: 1,
                deposit_fee: balance!(0),
                is_core: true,
                is_farm,
                total_tokens_in_pool: 0,
                rewards: 0,
                rewards_to_be_distributed: 0,
                reward_per_token: 0,
                is_removed: false,
                base_asset: XOR,
            };

            let mut token_info = TokenInfo {
                farms_total_multiplier: 1,
                staking_total_multiplier: 0,
                token_per_block: balance!(1),
                farms_allocation: balance!(0.6),
                staking_allocation: balance!(0.2),
                team_allocation: balance!(0.2),
                team_account: BOB,
            };
            demeter_farming_platform::TokenInfos::<Runtime>::insert(&reward_asset, &token_info);

            demeter_farming_platform::Pools::<Runtime>::append(
                &pool_asset,
                &reward_asset,
                &pool_info,
            );

            assert_ok!(
                demeter_farming_platform::Pallet::<Runtime>::change_pool_multiplier(
                    RuntimeOrigin::signed(
                        demeter_farming_platform::AuthorityAccount::<Runtime>::get()
                    ),
                    pool_asset,
                    pool_asset,
                    reward_asset,
                    is_farm,
                    new_multiplier,
                )
            );

            token_info =
                demeter_farming_platform::TokenInfos::<Runtime>::get(&reward_asset).unwrap();
            let mut pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(&pool_asset, &reward_asset);
            for pool_info in pool_infos.iter_mut() {
                if pool_info.is_farm == is_farm
                    && !pool_info.is_removed
                    && pool_info.base_asset == pool_asset
                {
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
                reward_per_token: 0,
                is_removed: false,
                base_asset: XOR,
            };

            demeter_farming_platform::Pools::<Runtime>::append(
                &pool_asset,
                &reward_asset,
                &pool_info,
            );

            assert_ok!(
                demeter_farming_platform::Pallet::<Runtime>::change_pool_multiplier(
                    RuntimeOrigin::signed(
                        demeter_farming_platform::AuthorityAccount::<Runtime>::get()
                    ),
                    pool_asset,
                    pool_asset,
                    reward_asset,
                    is_farm,
                    new_multiplier,
                )
            );

            token_info =
                demeter_farming_platform::TokenInfos::<Runtime>::get(&reward_asset).unwrap();
            let mut pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(&pool_asset, &reward_asset);
            for pool_info in pool_infos.iter_mut() {
                if pool_info.is_farm == is_farm
                    && !pool_info.is_removed
                    && pool_info.base_asset == pool_asset
                {
                    assert_eq!(pool_info.multiplier, new_multiplier);
                }
            }
            assert_eq!(token_info.staking_total_multiplier, new_multiplier);
        });
    }

    #[test]
    fn change_pool_multiplier_is_farm_xstusd_true() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let pool_asset = XSTUSD;
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
                reward_per_token: 0,
                is_removed: false,
                base_asset: XSTUSD,
            };

            demeter_farming_platform::Pools::<Runtime>::append(
                &pool_asset,
                &reward_asset,
                &pool_info,
            );

            assert_ok!(
                demeter_farming_platform::Pallet::<Runtime>::change_pool_multiplier(
                    RuntimeOrigin::signed(
                        demeter_farming_platform::AuthorityAccount::<Runtime>::get()
                    ),
                    pool_asset,
                    pool_asset,
                    reward_asset,
                    is_farm,
                    new_multiplier,
                )
            );

            token_info =
                demeter_farming_platform::TokenInfos::<Runtime>::get(&reward_asset).unwrap();
            let mut pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(&pool_asset, &reward_asset);
            for pool_info in pool_infos.iter_mut() {
                if pool_info.is_farm == is_farm
                    && !pool_info.is_removed
                    && pool_info.base_asset == pool_asset
                {
                    assert_eq!(pool_info.multiplier, new_multiplier);
                }
            }
            assert_eq!(token_info.farms_total_multiplier, new_multiplier);
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
                    RuntimeOrigin::signed(ALICE),
                    pool_asset,
                    pool_asset,
                    reward_asset,
                    is_farm,
                    deposit_fee,
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
                    RuntimeOrigin::signed(
                        demeter_farming_platform::AuthorityAccount::<Runtime>::get()
                    ),
                    pool_asset,
                    pool_asset,
                    reward_asset,
                    is_farm,
                    deposit_fee,
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
                reward_per_token: 0,
                is_removed: false,
                base_asset: XOR,
            };

            demeter_farming_platform::Pools::<Runtime>::append(
                &pool_asset,
                &reward_asset,
                &pool_info,
            );

            deposit_fee = balance!(1.2);

            assert_err!(
                demeter_farming_platform::Pallet::<Runtime>::change_pool_deposit_fee(
                    RuntimeOrigin::signed(
                        demeter_farming_platform::AuthorityAccount::<Runtime>::get()
                    ),
                    pool_asset,
                    pool_asset,
                    reward_asset,
                    is_farm,
                    deposit_fee,
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
                reward_per_token: 0,
                is_removed: false,
                base_asset: XOR,
            };

            demeter_farming_platform::Pools::<Runtime>::append(
                &pool_asset,
                &reward_asset,
                &pool_info,
            );

            deposit_fee = balance!(0.8);
            assert_ok!(
                demeter_farming_platform::Pallet::<Runtime>::change_pool_deposit_fee(
                    RuntimeOrigin::signed(
                        demeter_farming_platform::AuthorityAccount::<Runtime>::get()
                    ),
                    pool_asset,
                    pool_asset,
                    reward_asset,
                    is_farm,
                    deposit_fee,
                )
            );

            let pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(&pool_asset, &reward_asset);
            for p_info in pool_infos {
                if !p_info.is_removed
                    && p_info.is_farm == is_farm
                    && p_info.base_asset == pool_asset
                {
                    assert_eq!(p_info.deposit_fee, deposit_fee)
                }
            }
        });
    }

    #[test]
    fn change_pool_deposit_fee_xstusd_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let pool_asset = XSTUSD;
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
                reward_per_token: 0,
                is_removed: false,
                base_asset: XSTUSD,
            };

            demeter_farming_platform::Pools::<Runtime>::append(
                &pool_asset,
                &reward_asset,
                &pool_info,
            );

            deposit_fee = balance!(0.8);
            assert_ok!(
                demeter_farming_platform::Pallet::<Runtime>::change_pool_deposit_fee(
                    RuntimeOrigin::signed(
                        demeter_farming_platform::AuthorityAccount::<Runtime>::get()
                    ),
                    pool_asset,
                    pool_asset,
                    reward_asset,
                    is_farm,
                    deposit_fee,
                )
            );

            let pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(&pool_asset, &reward_asset);
            for p_info in pool_infos {
                if !p_info.is_removed
                    && p_info.is_farm == is_farm
                    && p_info.base_asset == pool_asset
                {
                    assert_eq!(p_info.deposit_fee, deposit_fee)
                }
            }
        });
    }

    #[test]
    fn change_token_info_unauthorized() {
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
                demeter_farming_platform::Pallet::<Runtime>::change_token_info(
                    RuntimeOrigin::signed(ALICE),
                    reward_asset,
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
                    RuntimeOrigin::signed(
                        demeter_farming_platform::AuthorityAccount::<Runtime>::get()
                    ),
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
                    RuntimeOrigin::signed(
                        demeter_farming_platform::AuthorityAccount::<Runtime>::get()
                    ),
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
                    RuntimeOrigin::signed(
                        demeter_farming_platform::AuthorityAccount::<Runtime>::get()
                    ),
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
                    RuntimeOrigin::signed(BOB),
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
                reward_per_token: 0,
                is_removed: false,
                base_asset: XOR,
            };

            demeter_farming_platform::Pools::<Runtime>::append(
                &pool_asset,
                &reward_asset,
                &pool_info,
            );

            assert_ok!(
                demeter_farming_platform::Pallet::<Runtime>::change_total_tokens(
                    RuntimeOrigin::signed(
                        demeter_farming_platform::AuthorityAccount::<Runtime>::get()
                    ),
                    pool_asset,
                    pool_asset,
                    reward_asset,
                    is_farm,
                    total_tokens,
                )
            );

            let mut pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(&pool_asset, &reward_asset);
            for pool_info in pool_infos.iter_mut() {
                if pool_info.is_farm == is_farm
                    && !pool_info.is_removed
                    && pool_info.base_asset == pool_asset
                {
                    assert_eq!(pool_info.total_tokens_in_pool, total_tokens);
                }
            }
        });
    }

    #[test]
    fn change_total_tokens_xstusd_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let pool_asset = XSTUSD;
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
                reward_per_token: 0,
                is_removed: false,
                base_asset: XSTUSD,
            };

            demeter_farming_platform::Pools::<Runtime>::append(
                &pool_asset,
                &reward_asset,
                &pool_info,
            );

            assert_ok!(
                demeter_farming_platform::Pallet::<Runtime>::change_total_tokens(
                    RuntimeOrigin::signed(
                        demeter_farming_platform::AuthorityAccount::<Runtime>::get()
                    ),
                    pool_asset,
                    pool_asset,
                    reward_asset,
                    is_farm,
                    total_tokens,
                )
            );

            let mut pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(&pool_asset, &reward_asset);
            for pool_info in pool_infos.iter_mut() {
                if pool_info.is_farm == is_farm
                    && !pool_info.is_removed
                    && pool_info.base_asset == pool_asset
                {
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
            let base_asset = XSTUSD;
            let is_farm = true;
            let pooled_tokens = 10;
            let rewards = 1;

            let pool_info = PoolData {
                multiplier: 1,
                deposit_fee: balance!(0),
                is_core: true,
                is_farm,
                total_tokens_in_pool: pooled_tokens,
                rewards: 0,
                rewards_to_be_distributed: 0,
                reward_per_token: 0,
                is_removed: false,
                base_asset,
            };
            demeter_farming_platform::Pools::<Runtime>::append(
                &pool_asset,
                &reward_asset,
                pool_info,
            );

            let user_info = UserInfo {
                base_asset,
                pool_asset,
                reward_asset,
                is_farm,
                pooled_tokens,
                rewards,
                reward_per_token_paid: 0,
            };

            demeter_farming_platform::UserInfos::<Runtime>::append(ALICE, user_info);

            let pool_tokens = balance!(69);
            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::change_info(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                ALICE.into(),
                base_asset,
                pool_asset,
                reward_asset,
                is_farm,
                pool_tokens,
            ));

            let user_info_alice = demeter_farming_platform::UserInfos::<Runtime>::get(&ALICE);
            for user_info in &user_info_alice {
                if user_info.pool_asset == pool_asset
                    && user_info.reward_asset == reward_asset
                    && user_info.is_farm == is_farm
                    && user_info.base_asset == base_asset
                {
                    assert_eq!(user_info.pooled_tokens, pool_tokens);
                }
            }
        });
    }

    #[test]
    fn change_info_xstusd_ok() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let pool_asset = XSTUSD;
            let reward_asset = CERES_ASSET_ID;
            let is_farm = true;
            let pooled_tokens = 10;
            let rewards = 1;

            let pool_info = PoolData {
                multiplier: 1,
                deposit_fee: balance!(0),
                is_core: true,
                is_farm,
                total_tokens_in_pool: pooled_tokens,
                rewards: 0,
                rewards_to_be_distributed: 0,
                reward_per_token: 0,
                is_removed: false,
                base_asset: XSTUSD,
            };
            demeter_farming_platform::Pools::<Runtime>::append(
                &pool_asset,
                &reward_asset,
                pool_info,
            );

            let user_info = UserInfo {
                base_asset: XSTUSD,
                pool_asset,
                reward_asset,
                is_farm,
                pooled_tokens,
                rewards,
                reward_per_token_paid: 0,
            };

            demeter_farming_platform::UserInfos::<Runtime>::append(ALICE, user_info);

            let pool_tokens = balance!(69);
            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::change_info(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                ALICE.into(),
                pool_asset,
                pool_asset,
                reward_asset,
                is_farm,
                pool_tokens,
            ));

            let user_info_alice = demeter_farming_platform::UserInfos::<Runtime>::get(&ALICE);
            for user_info in &user_info_alice {
                if user_info.pool_asset == pool_asset
                    && user_info.reward_asset == reward_asset
                    && user_info.is_farm == is_farm
                    && user_info.base_asset == pool_asset
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
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
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
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                util,
                token_per_block,
                farms_allocation,
                staking_allocation,
                team_allocation,
                CHARLES
            ));

            // XOR/CERES - reward DEO
            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::add_pool(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                xor,
                ceres,
                deo,
                is_farm,
                10,
                deposit_fee,
                is_core
            ));

            // XOR/CERES - reward UTIL
            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::add_pool(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                xor,
                ceres,
                util,
                is_farm,
                5,
                deposit_fee,
                is_core
            ));

            // CERES - reward DEO
            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::add_pool(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                ceres,
                ceres,
                deo,
                !is_farm,
                1,
                deposit_fee,
                is_core
            ));

            // CERES - reward UTIL
            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::add_pool(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                ceres,
                ceres,
                util,
                !is_farm,
                1,
                deposit_fee,
                is_core
            ));

            let pallet_account = PalletId(*b"deofarms").into_account_truncating();
            assert_ok!(assets::Pallet::<Runtime>::transfer_from(
                &util,
                &ALICE,
                &pallet_account,
                balance!(1500),
            ));

            assert_ok!(pool_xyk::Pallet::<Runtime>::deposit_liquidity(
                RuntimeOrigin::signed(ALICE),
                dex_id,
                xor,
                ceres,
                balance!(500),
                balance!(700),
                balance!(500),
                balance!(700),
            ));

            assert_ok!(pool_xyk::Pallet::<Runtime>::deposit_liquidity(
                RuntimeOrigin::signed(BOB),
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
                RuntimeOrigin::signed(ALICE),
                xor,
                ceres,
                deo,
                is_farm,
                balance!(2)
            ));

            // DEPOSIT TO XOR/CERES POOL - reward DEO
            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::deposit(
                RuntimeOrigin::signed(BOB),
                xor,
                ceres,
                deo,
                is_farm,
                balance!(2)
            ));

            // DEPOSIT TO XOR/CERES POOL - reward UTIL
            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::deposit(
                RuntimeOrigin::signed(ALICE),
                xor,
                ceres,
                util,
                is_farm,
                balance!(1)
            ));

            // DEPOSIT TO CERES POOL - reward DEO
            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::deposit(
                RuntimeOrigin::signed(BOB),
                ceres,
                ceres,
                deo,
                !is_farm,
                balance!(100)
            ));

            // DEPOSIT TO CERES POOL - reward UTIL
            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::deposit(
                RuntimeOrigin::signed(BOB),
                ceres,
                ceres,
                util,
                !is_farm,
                balance!(100)
            ));

            run_to_block(16201);

            // Check XOR/CERES pool and CERES pool - reward DEO
            let pool_infos = demeter_farming_platform::Pools::<Runtime>::get(&ceres, &deo);
            for pool_info in pool_infos {
                if pool_info.is_farm && pool_info.base_asset == xor {
                    assert_eq!(pool_info.total_tokens_in_pool, balance!(3.84));
                    assert_eq!(pool_info.rewards_to_be_distributed, balance!(8640));
                    assert_eq!(pool_info.rewards, balance!(1080));
                } else {
                    assert_eq!(pool_info.total_tokens_in_pool, balance!(96));
                    assert_eq!(pool_info.rewards_to_be_distributed, balance!(5184));
                    assert_eq!(pool_info.rewards, balance!(648));
                }
            }
            let token_info = demeter_farming_platform::TokenInfos::<Runtime>::get(&deo).unwrap();
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&deo, &token_info.team_account).unwrap(),
                balance!(576)
            );
            let user_info_alice = demeter_farming_platform::UserInfos::<Runtime>::get(&ALICE);
            let alice_deo_farm = user_info_alice
                .iter()
                .find(|user_info| {
                    user_info.pool_asset == ceres
                        && user_info.reward_asset == deo
                        && user_info.is_farm
                        && user_info.base_asset == xor
                })
                .expect("Alice DEO farm position should exist");
            assert_eq!(alice_deo_farm.rewards, 0);

            let user_info_bob = demeter_farming_platform::UserInfos::<Runtime>::get(&BOB);
            let bob_deo_farm = user_info_bob
                .iter()
                .find(|user_info| {
                    user_info.pool_asset == ceres
                        && user_info.reward_asset == deo
                        && user_info.is_farm
                        && user_info.base_asset == xor
                })
                .expect("Bob DEO farm position should exist");
            assert_eq!(bob_deo_farm.rewards, 0);
            let bob_deo_staking = user_info_bob
                .iter()
                .find(|user_info| {
                    user_info.pool_asset == ceres
                        && user_info.reward_asset == deo
                        && !user_info.is_farm
                })
                .expect("Bob DEO staking position should exist");
            assert_eq!(bob_deo_staking.rewards, 0);

            // Check XOR/CERES pool and CERES pool - reward UTIL
            let pool_infos = demeter_farming_platform::Pools::<Runtime>::get(&ceres, &util);
            for pool_info in pool_infos {
                if pool_info.is_farm && pool_info.base_asset == xor {
                    assert_eq!(pool_info.total_tokens_in_pool, balance!(0.96));
                    assert_eq!(pool_info.rewards_to_be_distributed, balance!(72));
                    assert_eq!(pool_info.rewards, balance!(9));
                } else {
                    assert_eq!(pool_info.total_tokens_in_pool, balance!(96));
                    assert_eq!(pool_info.rewards_to_be_distributed, balance!(57.6));
                    assert_eq!(pool_info.rewards, balance!(7.2));
                }
            }
            let token_info = demeter_farming_platform::TokenInfos::<Runtime>::get(&util).unwrap();
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&util, &token_info.team_account).unwrap(),
                balance!(14.4)
            );
            let alice_util_farm = user_info_alice
                .iter()
                .find(|user_info| {
                    user_info.pool_asset == ceres
                        && user_info.reward_asset == util
                        && user_info.is_farm
                        && user_info.base_asset == xor
                })
                .expect("Alice UTIL farm position should exist");
            assert_eq!(alice_util_farm.rewards, 0);
            let bob_util_staking = user_info_bob
                .iter()
                .find(|user_info| {
                    user_info.pool_asset == ceres
                        && user_info.reward_asset == util
                        && !user_info.is_farm
                })
                .expect("Bob UTIL staking position should exist");
            assert_eq!(bob_util_staking.rewards, 0);

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::get_rewards(
                RuntimeOrigin::signed(ALICE),
                xor,
                ceres,
                deo,
                is_farm
            ));

            assert_eq!(
                Assets::free_balance(&deo, &ALICE).expect("Failed to query free balance."),
                balance!(540)
            );

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::get_rewards(
                RuntimeOrigin::signed(BOB),
                xor,
                ceres,
                deo,
                is_farm
            ));

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::get_rewards(
                RuntimeOrigin::signed(BOB),
                ceres,
                ceres,
                deo,
                !is_farm
            ));

            assert_eq!(
                Assets::free_balance(&deo, &BOB).expect("Failed to query free balance."),
                balance!(1764)
            );

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::get_rewards(
                RuntimeOrigin::signed(ALICE),
                xor,
                ceres,
                util,
                is_farm
            ));

            assert_eq!(
                Assets::free_balance(&util, &ALICE).expect("Failed to query free balance."),
                balance!(509)
            );

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::get_rewards(
                RuntimeOrigin::signed(BOB),
                ceres,
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
    fn lazy_reward_accounting_leaves_only_rounding_dust_in_pool() {
        preset_initial(|| {
            let pool_asset = CERES_ASSET_ID;
            let reward_asset = CERES_ASSET_ID;
            let is_farm = false;

            let pool_info = PoolData {
                multiplier: 1,
                deposit_fee: 0,
                is_core: true,
                is_farm,
                total_tokens_in_pool: balance!(3),
                rewards: 0,
                rewards_to_be_distributed: balance!(1),
                reward_per_token: 0,
                is_removed: false,
                base_asset: pool_asset,
            };
            demeter_farming_platform::Pools::<Runtime>::append(
                &pool_asset,
                &reward_asset,
                &pool_info,
            );

            for user in [ALICE, BOB, CHARLES] {
                demeter_farming_platform::UserInfos::<Runtime>::append(
                    user,
                    UserInfo {
                        base_asset: pool_asset,
                        pool_asset,
                        reward_asset,
                        is_farm,
                        pooled_tokens: balance!(1),
                        rewards: 0,
                        reward_per_token_paid: 0,
                    },
                );
            }

            demeter_farming_platform::Pallet::<Runtime>::on_initialize(900);
            let pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(&pool_asset, &reward_asset);
            assert_eq!(pool_infos[0].rewards, balance!(0.0625));
            assert_eq!(
                pool_infos[0].reward_per_token,
                balance!(0.020833333333333333)
            );

            for user in [ALICE, BOB, CHARLES] {
                assert_ok!(demeter_farming_platform::Pallet::<Runtime>::get_rewards(
                    RuntimeOrigin::signed(user),
                    pool_asset,
                    pool_asset,
                    reward_asset,
                    is_farm,
                ));
            }

            let pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(&pool_asset, &reward_asset);
            assert_eq!(pool_infos[0].rewards, 1);
        });
    }

    #[test]
    fn check_if_has_enough_liquidity_out_of_farming_true() {
        preset_initial(|| {
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
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                reward_asset,
                token_per_block,
                farms_allocation,
                staking_allocation,
                team_allocation,
                BOB
            ));

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::register_token(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                pool_asset,
                token_per_block,
                farms_allocation,
                staking_allocation,
                team_allocation,
                BOB
            ));

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::add_pool(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                pool_asset,
                reward_asset,
                reward_asset,
                is_farm,
                multiplier,
                deposit_fee,
                is_core
            ));

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::add_pool(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                pool_asset,
                reward_asset,
                pool_asset,
                is_farm,
                multiplier,
                deposit_fee,
                is_core
            ));

            assert_ok!(pool_xyk::Pallet::<Runtime>::deposit_liquidity(
                RuntimeOrigin::signed(ALICE),
                DEX_A_ID.into(),
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
                RuntimeOrigin::signed(ALICE),
                pool_asset,
                reward_asset,
                reward_asset,
                is_farm,
                pool_tokens
            ));

            let pooled_tokens = (FixedWrapper::from(pool_tokens)
                * FixedWrapper::from(balance!(0.96)))
            .try_into_balance()
            .unwrap_or(0);

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::deposit(
                RuntimeOrigin::signed(ALICE),
                pool_asset,
                reward_asset,
                pool_asset,
                is_farm,
                pooled_tokens
            ));

            let mut pooled_tokens_to_withdraw = (FixedWrapper::from(pooled_tokens)
                / FixedWrapper::from(balance!(2)))
            .try_into_balance()
            .unwrap_or(0);

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::withdraw(
                RuntimeOrigin::signed(ALICE),
                pool_asset,
                reward_asset,
                reward_asset,
                pooled_tokens_to_withdraw,
                is_farm
            ));

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::withdraw(
                RuntimeOrigin::signed(ALICE),
                pool_asset,
                reward_asset,
                pool_asset,
                pooled_tokens_to_withdraw,
                is_farm
            ));

            pooled_tokens_to_withdraw = (FixedWrapper::from(pooled_tokens)
                / FixedWrapper::from(balance!(3)))
            .try_into_balance()
            .unwrap_or(0);

            assert_eq!(
                demeter_farming_platform::Pallet::<Runtime>::check_if_has_enough_liquidity_out_of_farming(
                    &ALICE,
                    pool_asset,
                    reward_asset,
                    pooled_tokens_to_withdraw,
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
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                reward_asset,
                token_per_block,
                farms_allocation,
                staking_allocation,
                team_allocation,
                BOB
            ));

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::add_pool(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                pool_asset,
                reward_asset,
                reward_asset,
                is_farm,
                multiplier,
                deposit_fee,
                is_core
            ));

            assert_ok!(pool_xyk::Pallet::<Runtime>::deposit_liquidity(
                RuntimeOrigin::signed(ALICE),
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
                RuntimeOrigin::signed(ALICE),
                pool_asset,
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

    #[test]
    fn check_if_has_enough_liquidity_out_of_farming_xstusd_true() {
        preset_initial(|| {
            let pool_asset = XSTUSD;
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
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                reward_asset,
                token_per_block,
                farms_allocation,
                staking_allocation,
                team_allocation,
                BOB
            ));

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::register_token(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                pool_asset,
                token_per_block,
                farms_allocation,
                staking_allocation,
                team_allocation,
                BOB
            ));

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::add_pool(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                pool_asset,
                reward_asset,
                reward_asset,
                is_farm,
                multiplier,
                deposit_fee,
                is_core
            ));

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::add_pool(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                pool_asset,
                reward_asset,
                pool_asset,
                is_farm,
                multiplier,
                deposit_fee,
                is_core
            ));

            assert_ok!(pool_xyk::Pallet::<Runtime>::deposit_liquidity(
                RuntimeOrigin::signed(ALICE),
                DEX_B_ID.into(),
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
                RuntimeOrigin::signed(ALICE),
                pool_asset,
                reward_asset,
                reward_asset,
                is_farm,
                pool_tokens
            ));

            let pooled_tokens = (FixedWrapper::from(pool_tokens)
                * FixedWrapper::from(balance!(0.96)))
            .try_into_balance()
            .unwrap_or(0);

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::deposit(
                RuntimeOrigin::signed(ALICE),
                pool_asset,
                reward_asset,
                pool_asset,
                is_farm,
                pooled_tokens
            ));

            let mut pooled_tokens_to_withdraw = (FixedWrapper::from(pooled_tokens)
                / FixedWrapper::from(balance!(2)))
            .try_into_balance()
            .unwrap_or(0);

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::withdraw(
                RuntimeOrigin::signed(ALICE),
                pool_asset,
                reward_asset,
                reward_asset,
                pooled_tokens_to_withdraw,
                is_farm
            ));

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::withdraw(
                RuntimeOrigin::signed(ALICE),
                pool_asset,
                reward_asset,
                pool_asset,
                pooled_tokens_to_withdraw,
                is_farm
            ));

            pooled_tokens_to_withdraw = (FixedWrapper::from(pooled_tokens)
                / FixedWrapper::from(balance!(3)))
            .try_into_balance()
            .unwrap_or(0);

            assert_eq!(
                demeter_farming_platform::Pallet::<Runtime>::check_if_has_enough_liquidity_out_of_farming(
                    &ALICE,
                    pool_asset,
                    reward_asset,
                    pooled_tokens_to_withdraw,
                ),
                true
            );
        });
    }

    #[test]
    fn check_if_has_enough_liquidity_out_of_farming_xstusd_false() {
        preset_initial(|| {
            let dex_id = DEX_B_ID;
            let pool_asset = XSTUSD;
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
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                reward_asset,
                token_per_block,
                farms_allocation,
                staking_allocation,
                team_allocation,
                BOB
            ));

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::add_pool(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                pool_asset,
                reward_asset,
                reward_asset,
                is_farm,
                multiplier,
                deposit_fee,
                is_core
            ));

            assert_ok!(pool_xyk::Pallet::<Runtime>::deposit_liquidity(
                RuntimeOrigin::signed(ALICE),
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
                RuntimeOrigin::signed(ALICE),
                pool_asset,
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

    #[test]
    fn check_if_user_lp_is_changed_after_locking() {
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
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                reward_asset,
                token_per_block,
                farms_allocation,
                staking_allocation,
                team_allocation,
                BOB
            ));

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::add_pool(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                pool_asset,
                reward_asset,
                reward_asset,
                is_farm,
                multiplier,
                deposit_fee,
                is_core
            ));

            assert_ok!(pool_xyk::Pallet::<Runtime>::deposit_liquidity(
                RuntimeOrigin::signed(ALICE),
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
            let mut pool_tokens: Balance =
                <Runtime as ceres_liquidity_locker::Config>::XYKPool::pool_providers(
                    pool_account.clone(),
                    ALICE,
                )
                .expect("User is not pool provider");

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::deposit(
                RuntimeOrigin::signed(ALICE),
                pool_asset,
                reward_asset,
                reward_asset,
                is_farm,
                pool_tokens
            ));

            assert_ok!(ceres_liquidity_locker::Pallet::<Runtime>::lock_liquidity(
                RuntimeOrigin::signed(ALICE),
                pool_asset,
                reward_asset,
                10u32.into(),
                balance!(1),
                true
            ));

            pool_tokens = <Runtime as ceres_liquidity_locker::Config>::XYKPool::pool_providers(
                pool_account.clone(),
                ALICE,
            )
            .expect("User is not pool provider");

            let user_infos = demeter_farming_platform::UserInfos::<Runtime>::get(&ALICE);
            for u_info in &user_infos {
                if u_info.is_farm == is_farm && u_info.base_asset == pool_asset {
                    assert_eq!(u_info.pooled_tokens, pool_tokens);
                }
            }
            let pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(&reward_asset, &reward_asset);
            for p_info in &pool_infos {
                if !p_info.is_removed
                    && p_info.is_farm == is_farm
                    && p_info.base_asset == pool_asset
                {
                    assert_eq!(p_info.total_tokens_in_pool, pool_tokens);
                }
            }

            assert_err!(
                demeter_farming_platform::Pallet::<Runtime>::deposit(
                    RuntimeOrigin::signed(ALICE),
                    pool_asset,
                    reward_asset,
                    reward_asset,
                    is_farm,
                    balance!(14000)
                ),
                demeter_farming_platform::Error::<Runtime>::InsufficientLPTokens
            );
        });
    }

    #[test]
    fn check_if_user_lp_is_changed_after_locking_xstusd() {
        preset_initial(|| {
            let dex_id = DEX_B_ID;
            let pool_asset = XSTUSD;
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
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                reward_asset,
                token_per_block,
                farms_allocation,
                staking_allocation,
                team_allocation,
                BOB
            ));

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::add_pool(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                pool_asset,
                reward_asset,
                reward_asset,
                is_farm,
                multiplier,
                deposit_fee,
                is_core
            ));

            assert_ok!(pool_xyk::Pallet::<Runtime>::deposit_liquidity(
                RuntimeOrigin::signed(ALICE),
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
            let mut pool_tokens: Balance =
                <Runtime as ceres_liquidity_locker::Config>::XYKPool::pool_providers(
                    pool_account.clone(),
                    ALICE,
                )
                .expect("User is not pool provider");

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::deposit(
                RuntimeOrigin::signed(ALICE),
                pool_asset,
                reward_asset,
                reward_asset,
                is_farm,
                pool_tokens
            ));

            assert_ok!(ceres_liquidity_locker::Pallet::<Runtime>::lock_liquidity(
                RuntimeOrigin::signed(ALICE),
                pool_asset,
                reward_asset,
                10u32.into(),
                balance!(1),
                true
            ));

            pool_tokens = <Runtime as ceres_liquidity_locker::Config>::XYKPool::pool_providers(
                pool_account.clone(),
                ALICE,
            )
            .expect("User is not pool provider");

            let user_infos = demeter_farming_platform::UserInfos::<Runtime>::get(&ALICE);
            for u_info in &user_infos {
                if u_info.is_farm == is_farm && u_info.base_asset == pool_asset {
                    assert_eq!(u_info.pooled_tokens, pool_tokens);
                }
            }
            let pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(&reward_asset, &reward_asset);
            for p_info in &pool_infos {
                if !p_info.is_removed
                    && p_info.is_farm == is_farm
                    && p_info.base_asset == pool_asset
                {
                    assert_eq!(p_info.total_tokens_in_pool, pool_tokens);
                }
            }

            assert_err!(
                demeter_farming_platform::Pallet::<Runtime>::deposit(
                    RuntimeOrigin::signed(ALICE),
                    pool_asset,
                    reward_asset,
                    reward_asset,
                    is_farm,
                    balance!(14000)
                ),
                demeter_farming_platform::Error::<Runtime>::InsufficientLPTokens
            );
        });
    }

    #[test]
    fn demeter_farming_platform_storage_migration_works() {
        preset_initial(|| {
            generate_storage_instance!(DemeterFarmingPlatform, Pools);
            type OldPools = StorageDoubleMap<
                PoolsOldInstance,
                Identity,
                AssetIdOf<Runtime>,
                Identity,
                AssetIdOf<Runtime>,
                Vec<(u32, Balance, bool, bool, Balance, Balance, Balance, bool)>,
                ValueQuery,
            >;

            generate_storage_instance!(DemeterFarmingPlatform, UserInfos);
            type OldUserInfos = StorageMap<
                UserInfosOldInstance,
                Identity,
                AccountIdOf<Runtime>,
                Vec<(
                    AssetIdOf<Runtime>,
                    AssetIdOf<Runtime>,
                    bool,
                    Balance,
                    Balance,
                )>,
                ValueQuery,
            >;

            let asset_xor: AssetId = XOR.into();
            let asset_ceres: AssetId = CERES_ASSET_ID.into();
            let asset_xstusd: AssetId = XSTUSD.into();

            let mut vec_a: Vec<(u32, Balance, bool, bool, Balance, Balance, Balance, bool)> =
                Vec::new();
            vec_a.push((
                2u32,
                balance!(0.02),
                false,
                true,
                balance!(100),
                balance!(20),
                balance!(12),
                false,
            ));
            vec_a.push((
                3u32,
                balance!(0.01),
                true,
                false,
                balance!(120),
                balance!(10),
                balance!(2),
                false,
            ));
            OldPools::insert(asset_ceres, asset_xstusd, vec_a);

            let mut vec_b: Vec<(u32, Balance, bool, bool, Balance, Balance, Balance, bool)> =
                Vec::new();
            vec_b.push((
                4u32,
                balance!(0.03),
                false,
                true,
                balance!(130),
                balance!(25),
                balance!(8),
                false,
            ));
            OldPools::insert(asset_ceres, asset_ceres, vec_b);

            let mut user_a: Vec<(
                AssetIdOf<Runtime>,
                AssetIdOf<Runtime>,
                bool,
                Balance,
                Balance,
            )> = Vec::new();
            user_a.push((asset_ceres, asset_xstusd, true, balance!(5), balance!(10)));
            user_a.push((asset_ceres, asset_xstusd, false, balance!(3), balance!(9)));
            OldUserInfos::insert(ALICE, user_a);

            let mut user_b: Vec<(
                AssetIdOf<Runtime>,
                AssetIdOf<Runtime>,
                bool,
                Balance,
                Balance,
            )> = Vec::new();
            user_b.push((asset_ceres, asset_ceres, false, balance!(2), balance!(4)));
            OldUserInfos::insert(BOB, user_b);

            // Storage migration
            demeter_farming_platform::Pallet::<Runtime>::on_runtime_upgrade();

            let pools_a =
                demeter_farming_platform::Pools::<Runtime>::get(asset_ceres, asset_xstusd);
            for p_info in pools_a.iter() {
                if p_info.is_farm {
                    assert_eq!(p_info.multiplier, 2u32);
                    assert_eq!(p_info.deposit_fee, balance!(0.02));
                    assert_eq!(p_info.is_core, false);
                    assert_eq!(p_info.total_tokens_in_pool, balance!(100));
                    assert_eq!(p_info.rewards, balance!(20));
                    assert_eq!(p_info.rewards_to_be_distributed, balance!(12));
                    assert_eq!(p_info.is_removed, false);
                    assert_eq!(p_info.base_asset, asset_xor);
                } else {
                    assert_eq!(p_info.multiplier, 3u32);
                    assert_eq!(p_info.deposit_fee, balance!(0.01));
                    assert_eq!(p_info.is_core, true);
                    assert_eq!(p_info.total_tokens_in_pool, balance!(120));
                    assert_eq!(p_info.rewards, balance!(10));
                    assert_eq!(p_info.rewards_to_be_distributed, balance!(2));
                    assert_eq!(p_info.is_removed, false);
                    assert_eq!(p_info.base_asset, asset_ceres);
                }
            }

            let pools_b = demeter_farming_platform::Pools::<Runtime>::get(asset_ceres, asset_ceres);
            for p_info in pools_b.iter() {
                assert_eq!(p_info.multiplier, 4u32);
                assert_eq!(p_info.deposit_fee, balance!(0.03));
                assert_eq!(p_info.is_core, false);
                assert_eq!(p_info.total_tokens_in_pool, balance!(130));
                assert_eq!(p_info.rewards, balance!(25));
                assert_eq!(p_info.rewards_to_be_distributed, balance!(8));
                assert_eq!(p_info.is_removed, false);
                assert_eq!(p_info.base_asset, asset_xor);
            }

            let users_a = demeter_farming_platform::UserInfos::<Runtime>::get(ALICE);
            for u_info in users_a.iter() {
                if u_info.is_farm {
                    assert_eq!(u_info.base_asset, asset_xor);
                    assert_eq!(u_info.pool_asset, asset_ceres);
                    assert_eq!(u_info.reward_asset, asset_xstusd);
                    assert_eq!(u_info.pooled_tokens, balance!(5));
                    assert_eq!(u_info.rewards, balance!(10));
                } else {
                    assert_eq!(u_info.base_asset, asset_ceres);
                    assert_eq!(u_info.pool_asset, asset_ceres);
                    assert_eq!(u_info.reward_asset, asset_xstusd);
                    assert_eq!(u_info.pooled_tokens, balance!(3));
                    assert_eq!(u_info.rewards, balance!(9));
                }
            }

            let users_b = demeter_farming_platform::UserInfos::<Runtime>::get(BOB);
            for u_info in users_b.iter() {
                assert_eq!(u_info.base_asset, asset_ceres);
                assert_eq!(u_info.pool_asset, asset_ceres);
                assert_eq!(u_info.reward_asset, asset_ceres);
                assert_eq!(u_info.pooled_tokens, balance!(2));
                assert_eq!(u_info.rewards, balance!(4));
            }

            // Storage migration (no change)
            demeter_farming_platform::Pallet::<Runtime>::on_runtime_upgrade();

            let pools_a =
                demeter_farming_platform::Pools::<Runtime>::get(asset_ceres, asset_xstusd);
            for p_info in pools_a.iter() {
                if p_info.is_farm {
                    assert_eq!(p_info.multiplier, 2u32);
                    assert_eq!(p_info.deposit_fee, balance!(0.02));
                    assert_eq!(p_info.is_core, false);
                    assert_eq!(p_info.total_tokens_in_pool, balance!(100));
                    assert_eq!(p_info.rewards, balance!(20));
                    assert_eq!(p_info.rewards_to_be_distributed, balance!(12));
                    assert_eq!(p_info.is_removed, false);
                    assert_eq!(p_info.base_asset, asset_xor);
                } else {
                    assert_eq!(p_info.multiplier, 3u32);
                    assert_eq!(p_info.deposit_fee, balance!(0.01));
                    assert_eq!(p_info.is_core, true);
                    assert_eq!(p_info.total_tokens_in_pool, balance!(120));
                    assert_eq!(p_info.rewards, balance!(10));
                    assert_eq!(p_info.rewards_to_be_distributed, balance!(2));
                    assert_eq!(p_info.is_removed, false);
                    assert_eq!(p_info.base_asset, asset_ceres);
                }
            }

            let pools_b = demeter_farming_platform::Pools::<Runtime>::get(asset_ceres, asset_ceres);
            for p_info in pools_b.iter() {
                assert_eq!(p_info.multiplier, 4u32);
                assert_eq!(p_info.deposit_fee, balance!(0.03));
                assert_eq!(p_info.is_core, false);
                assert_eq!(p_info.total_tokens_in_pool, balance!(130));
                assert_eq!(p_info.rewards, balance!(25));
                assert_eq!(p_info.rewards_to_be_distributed, balance!(8));
                assert_eq!(p_info.is_removed, false);
                assert_eq!(p_info.base_asset, asset_xor);
            }

            let users_a = demeter_farming_platform::UserInfos::<Runtime>::get(ALICE);
            for u_info in users_a.iter() {
                if u_info.is_farm {
                    assert_eq!(u_info.base_asset, asset_xor);
                    assert_eq!(u_info.pool_asset, asset_ceres);
                    assert_eq!(u_info.reward_asset, asset_xstusd);
                    assert_eq!(u_info.pooled_tokens, balance!(5));
                    assert_eq!(u_info.rewards, balance!(10));
                } else {
                    assert_eq!(u_info.base_asset, asset_ceres);
                    assert_eq!(u_info.pool_asset, asset_ceres);
                    assert_eq!(u_info.reward_asset, asset_xstusd);
                    assert_eq!(u_info.pooled_tokens, balance!(3));
                    assert_eq!(u_info.rewards, balance!(9));
                }
            }

            let users_b = demeter_farming_platform::UserInfos::<Runtime>::get(BOB);
            for u_info in users_b.iter() {
                assert_eq!(u_info.base_asset, asset_ceres);
                assert_eq!(u_info.pool_asset, asset_ceres);
                assert_eq!(u_info.reward_asset, asset_ceres);
                assert_eq!(u_info.pooled_tokens, balance!(2));
                assert_eq!(u_info.rewards, balance!(4));
            }
        });
    }

    #[test]
    fn demeter_farming_platform_v2_to_v3_migration_works() {
        preset_initial(|| {
            generate_storage_instance!(DemeterFarmingPlatform, Pools);
            type V2Pools = StorageDoubleMap<
                PoolsOldInstance,
                Identity,
                AssetIdOf<Runtime>,
                Identity,
                AssetIdOf<Runtime>,
                Vec<(
                    u32,
                    Balance,
                    bool,
                    bool,
                    Balance,
                    Balance,
                    Balance,
                    bool,
                    AssetIdOf<Runtime>,
                )>,
                ValueQuery,
            >;

            generate_storage_instance!(DemeterFarmingPlatform, UserInfos);
            type V2UserInfos = StorageMap<
                UserInfosOldInstance,
                Identity,
                AccountIdOf<Runtime>,
                Vec<(
                    AssetIdOf<Runtime>,
                    AssetIdOf<Runtime>,
                    AssetIdOf<Runtime>,
                    bool,
                    Balance,
                    Balance,
                )>,
                ValueQuery,
            >;

            let asset_xor: AssetId = XOR.into();
            let asset_ceres: AssetId = CERES_ASSET_ID.into();

            let token_info = TokenInfo {
                farms_total_multiplier: 99,
                staking_total_multiplier: 88,
                token_per_block: balance!(1),
                farms_allocation: balance!(0.6),
                staking_allocation: balance!(0.2),
                team_allocation: balance!(0.2),
                team_account: BOB,
            };
            demeter_farming_platform::TokenInfos::<Runtime>::insert(&asset_ceres, &token_info);

            V2Pools::insert(
                asset_ceres,
                asset_ceres,
                vec![
                    (
                        2u32,
                        balance!(0.02),
                        false,
                        true,
                        balance!(100),
                        balance!(20),
                        balance!(12),
                        false,
                        asset_xor,
                    ),
                    (
                        3u32,
                        balance!(0.03),
                        false,
                        true,
                        balance!(120),
                        balance!(30),
                        balance!(18),
                        false,
                        asset_xor,
                    ),
                    (
                        5u32,
                        balance!(0.01),
                        true,
                        true,
                        balance!(140),
                        balance!(40),
                        balance!(24),
                        true,
                        asset_xor,
                    ),
                    (
                        7u32,
                        balance!(0.04),
                        true,
                        false,
                        balance!(160),
                        balance!(50),
                        balance!(30),
                        false,
                        asset_ceres,
                    ),
                ],
            );
            V2UserInfos::insert(
                ALICE,
                vec![(
                    asset_xor,
                    asset_ceres,
                    asset_ceres,
                    true,
                    balance!(10),
                    balance!(1),
                )],
            );

            demeter_farming_platform::PalletStorageVersion::<Runtime>::put(
                demeter_farming_platform::StorageVersion::V2,
            );

            demeter_farming_platform::Pallet::<Runtime>::on_runtime_upgrade();

            assert!(
                demeter_farming_platform::PalletStorageVersion::<Runtime>::get()
                    == demeter_farming_platform::StorageVersion::V3
            );
            let pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(asset_ceres, asset_ceres);
            assert_eq!(pool_infos.len(), 4);
            assert_eq!(
                pool_infos
                    .iter()
                    .filter(|pool_info| {
                        !pool_info.is_removed
                            && pool_info.is_farm
                            && pool_info.base_asset == asset_xor
                    })
                    .count(),
                1
            );
            for pool_info in pool_infos.iter() {
                assert_eq!(pool_info.reward_per_token, 0);
                if pool_info.multiplier == 3 {
                    assert!(pool_info.is_removed);
                }
            }

            let token_info =
                demeter_farming_platform::TokenInfos::<Runtime>::get(&asset_ceres).unwrap();
            assert_eq!(token_info.farms_total_multiplier, 2);
            assert_eq!(token_info.staking_total_multiplier, 7);

            let users = demeter_farming_platform::UserInfos::<Runtime>::get(ALICE);
            assert_eq!(users.len(), 1);
            assert_eq!(users[0].reward_per_token_paid, 0);
        });
    }

    #[test]
    fn activate_removed_pool_ok() {
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
                total_tokens_in_pool: balance!(1),
                rewards: 100,
                rewards_to_be_distributed: balance!(16),
                reward_per_token: 0,
                is_removed: false,
                base_asset: XOR,
            };

            let token_info = TokenInfo {
                farms_total_multiplier: 1,
                staking_total_multiplier: 0,
                token_per_block: balance!(1),
                farms_allocation: balance!(0.6),
                staking_allocation: balance!(0.2),
                team_allocation: balance!(0.2),
                team_account: BOB,
            };
            demeter_farming_platform::TokenInfos::<Runtime>::insert(&reward_asset, &token_info);

            demeter_farming_platform::Pools::<Runtime>::append(
                &pool_asset,
                &reward_asset,
                &pool_info,
            );

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::remove_pool(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                pool_asset,
                pool_asset,
                reward_asset,
                is_farm,
            ));

            let token_info =
                demeter_farming_platform::TokenInfos::<Runtime>::get(&reward_asset).unwrap();
            assert_eq!(token_info.farms_total_multiplier, 0);

            let mut pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(&pool_asset, &reward_asset);
            for pool_info in pool_infos.iter_mut() {
                if pool_info.is_farm == is_farm && pool_info.base_asset == pool_asset {
                    assert_eq!(pool_info.is_removed, true);
                    pool_info.rewards_to_be_distributed = balance!(16);
                }
            }
            demeter_farming_platform::Pools::<Runtime>::insert(
                &pool_asset,
                &reward_asset,
                pool_infos,
            );

            assert_ok!(
                demeter_farming_platform::Pallet::<Runtime>::activate_removed_pool(
                    RuntimeOrigin::signed(
                        demeter_farming_platform::AuthorityAccount::<Runtime>::get()
                    ),
                    pool_asset,
                    pool_asset,
                    reward_asset,
                    is_farm,
                )
            );

            let token_info =
                demeter_farming_platform::TokenInfos::<Runtime>::get(&reward_asset).unwrap();
            assert_eq!(token_info.farms_total_multiplier, 1);

            let mut pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(&pool_asset, &reward_asset);
            for pool_info in pool_infos.iter_mut() {
                if pool_info.is_farm == is_farm && pool_info.base_asset == pool_asset {
                    assert_eq!(pool_info.is_removed, false);
                    assert_eq!(pool_info.rewards_to_be_distributed, 0);
                    assert_eq!(pool_info.reward_per_token, 0);
                }
            }

            demeter_farming_platform::Pallet::<Runtime>::on_initialize(900);

            let pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(&pool_asset, &reward_asset);
            for pool_info in pool_infos {
                if pool_info.is_farm == is_farm && pool_info.base_asset == pool_asset {
                    assert_eq!(pool_info.rewards_to_be_distributed, 0);
                    assert_eq!(pool_info.reward_per_token, 0);
                }
            }
        });
    }

    #[test]
    fn accrue_user_rewards_handles_large_pooled_tokens() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let large_balance = i128::MAX as Balance + 1;
            let user_info = UserInfo {
                base_asset: XOR,
                pool_asset: XSTUSD,
                reward_asset: CERES_ASSET_ID,
                is_farm: true,
                pooled_tokens: large_balance,
                rewards: 0,
                reward_per_token_paid: 0,
            };
            let pool_info = PoolData {
                multiplier: 1,
                deposit_fee: 0,
                is_core: true,
                is_farm: true,
                total_tokens_in_pool: large_balance,
                rewards: large_balance,
                rewards_to_be_distributed: 0,
                reward_per_token: balance!(1),
                is_removed: false,
                base_asset: XOR,
            };

            demeter_farming_platform::Pools::<Runtime>::insert(
                XSTUSD,
                CERES_ASSET_ID,
                vec![pool_info],
            );
            demeter_farming_platform::UserInfos::<Runtime>::insert(ALICE, vec![user_info]);

            assert_ok!(demeter_farming_platform::Pallet::<Runtime>::withdraw(
                RuntimeOrigin::signed(ALICE),
                XOR,
                XSTUSD,
                CERES_ASSET_ID,
                balance!(1),
                true,
            ));

            let user_infos = demeter_farming_platform::UserInfos::<Runtime>::get(ALICE);
            assert_eq!(user_infos[0].pooled_tokens, large_balance - balance!(1));
            assert_eq!(user_infos[0].rewards, large_balance);
            assert_eq!(user_infos[0].reward_per_token_paid, balance!(1));
        });
    }

    #[test]
    fn accrue_user_rewards_preserves_checkpoint_on_overflow() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let user_info = UserInfo {
                base_asset: XOR,
                pool_asset: XSTUSD,
                reward_asset: CERES_ASSET_ID,
                is_farm: true,
                pooled_tokens: balance!(1),
                rewards: Balance::MAX,
                reward_per_token_paid: 0,
            };
            let pool_info = PoolData {
                multiplier: 1,
                deposit_fee: 0,
                is_core: true,
                is_farm: true,
                total_tokens_in_pool: balance!(1),
                rewards: balance!(1),
                rewards_to_be_distributed: 0,
                reward_per_token: balance!(1),
                is_removed: false,
                base_asset: XOR,
            };

            demeter_farming_platform::Pools::<Runtime>::insert(
                XSTUSD,
                CERES_ASSET_ID,
                vec![pool_info],
            );
            demeter_farming_platform::UserInfos::<Runtime>::insert(ALICE, vec![user_info]);

            assert_err!(
                demeter_farming_platform::Pallet::<Runtime>::withdraw(
                    RuntimeOrigin::signed(ALICE),
                    XOR,
                    XSTUSD,
                    CERES_ASSET_ID,
                    balance!(1),
                    true,
                ),
                demeter_farming_platform::Error::<Runtime>::ArithmeticError
            );

            let user_infos = demeter_farming_platform::UserInfos::<Runtime>::get(ALICE);
            assert_eq!(user_infos[0].pooled_tokens, balance!(1));
            assert_eq!(user_infos[0].rewards, Balance::MAX);
            assert_eq!(user_infos[0].reward_per_token_paid, 0);
        });
    }

    #[test]
    fn distribute_rewards_to_users_updates_pool_accounting_atomically() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let pool_info = PoolData {
                multiplier: 1,
                deposit_fee: 0,
                is_core: true,
                is_farm: true,
                total_tokens_in_pool: balance!(1),
                rewards: Balance::MAX,
                rewards_to_be_distributed: balance!(16),
                reward_per_token: 0,
                is_removed: false,
                base_asset: XOR,
            };
            demeter_farming_platform::Pools::<Runtime>::insert(
                XOR,
                CERES_ASSET_ID,
                vec![pool_info],
            );

            demeter_farming_platform::Pallet::<Runtime>::on_initialize(900);

            let pool_infos = demeter_farming_platform::Pools::<Runtime>::get(XOR, CERES_ASSET_ID);
            assert_eq!(pool_infos[0].rewards, Balance::MAX);
            assert_eq!(pool_infos[0].reward_per_token, 0);
        });
    }

    #[test]
    fn distribute_rewards_to_users_skips_zero_reward_per_token() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let pool_info = PoolData {
                multiplier: 1,
                deposit_fee: 0,
                is_core: true,
                is_farm: true,
                total_tokens_in_pool: Balance::MAX,
                rewards: 0,
                rewards_to_be_distributed: balance!(16),
                reward_per_token: 0,
                is_removed: false,
                base_asset: XOR,
            };
            demeter_farming_platform::Pools::<Runtime>::insert(
                XOR,
                CERES_ASSET_ID,
                vec![pool_info],
            );

            demeter_farming_platform::Pallet::<Runtime>::on_initialize(900);

            let pool_infos = demeter_farming_platform::Pools::<Runtime>::get(XOR, CERES_ASSET_ID);
            assert_eq!(pool_infos[0].rewards, 0);
            assert_eq!(pool_infos[0].reward_per_token, 0);
        });
    }

    #[test]
    fn update_pool_tokens_rolls_back_on_later_pool_error() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let first_user_info = UserInfo {
                base_asset: XOR,
                pool_asset: XSTUSD,
                reward_asset: CERES_ASSET_ID,
                is_farm: true,
                pooled_tokens: balance!(10),
                rewards: 0,
                reward_per_token_paid: 0,
            };
            let second_user_info = UserInfo {
                base_asset: XOR,
                pool_asset: XSTUSD,
                reward_asset: TBCD,
                is_farm: true,
                pooled_tokens: balance!(10),
                rewards: 0,
                reward_per_token_paid: 0,
            };
            let first_pool_info = PoolData {
                multiplier: 1,
                deposit_fee: 0,
                is_core: true,
                is_farm: true,
                total_tokens_in_pool: balance!(10),
                rewards: balance!(10),
                rewards_to_be_distributed: 0,
                reward_per_token: balance!(1),
                is_removed: false,
                base_asset: XOR,
            };
            let second_pool_info = PoolData {
                multiplier: 1,
                deposit_fee: 0,
                is_core: true,
                is_farm: true,
                total_tokens_in_pool: balance!(1),
                rewards: 0,
                rewards_to_be_distributed: 0,
                reward_per_token: 0,
                is_removed: false,
                base_asset: XOR,
            };

            demeter_farming_platform::UserInfos::<Runtime>::insert(
                ALICE,
                vec![first_user_info, second_user_info],
            );
            demeter_farming_platform::Pools::<Runtime>::insert(
                XSTUSD,
                CERES_ASSET_ID,
                vec![first_pool_info],
            );
            demeter_farming_platform::Pools::<Runtime>::insert(
                XSTUSD,
                TBCD,
                vec![second_pool_info],
            );

            assert_err!(
                demeter_farming_platform::Pallet::<Runtime>::update_pool_tokens(
                    ALICE,
                    balance!(5),
                    XOR,
                    XSTUSD
                ),
                demeter_farming_platform::Error::<Runtime>::ArithmeticError
            );

            let user_infos = demeter_farming_platform::UserInfos::<Runtime>::get(ALICE);
            assert_eq!(user_infos[0].pooled_tokens, balance!(10));
            assert_eq!(user_infos[0].rewards, 0);
            assert_eq!(user_infos[0].reward_per_token_paid, 0);
            let first_pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(XSTUSD, CERES_ASSET_ID);
            assert_eq!(first_pool_infos[0].total_tokens_in_pool, balance!(10));
            let second_pool_infos = demeter_farming_platform::Pools::<Runtime>::get(XSTUSD, TBCD);
            assert_eq!(second_pool_infos[0].total_tokens_in_pool, balance!(1));
        });
    }

    #[test]
    fn distribute_rewards_to_pools_skips_reserved_rewards_overflow() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let token_info = TokenInfo {
                farms_total_multiplier: 2,
                staking_total_multiplier: 0,
                token_per_block: balance!(1),
                farms_allocation: balance!(1),
                staking_allocation: 0,
                team_allocation: 0,
                team_account: BOB,
            };
            let first_pool_info = PoolData {
                multiplier: 1,
                deposit_fee: 0,
                is_core: true,
                is_farm: true,
                total_tokens_in_pool: Balance::MAX,
                rewards: Balance::MAX,
                rewards_to_be_distributed: balance!(16),
                reward_per_token: 0,
                is_removed: false,
                base_asset: XOR,
            };
            let second_pool_info = PoolData {
                multiplier: 1,
                deposit_fee: 0,
                is_core: true,
                is_farm: true,
                total_tokens_in_pool: Balance::MAX,
                rewards: 1,
                rewards_to_be_distributed: balance!(16),
                reward_per_token: 0,
                is_removed: false,
                base_asset: XOR,
            };

            demeter_farming_platform::TokenInfos::<Runtime>::insert(CERES_ASSET_ID, token_info);
            demeter_farming_platform::Pools::<Runtime>::insert(
                XSTUSD,
                CERES_ASSET_ID,
                vec![first_pool_info],
            );
            demeter_farming_platform::Pools::<Runtime>::insert(
                XOR,
                CERES_ASSET_ID,
                vec![second_pool_info],
            );

            demeter_farming_platform::Pallet::<Runtime>::on_initialize(14440);

            let first_pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(XSTUSD, CERES_ASSET_ID);
            let second_pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(XOR, CERES_ASSET_ID);
            assert_eq!(first_pool_infos[0].rewards_to_be_distributed, 0);
            assert_eq!(second_pool_infos[0].rewards_to_be_distributed, 0);
            assert_eq!(first_pool_infos[0].reward_per_token, 0);
            assert_eq!(second_pool_infos[0].reward_per_token, 0);

            demeter_farming_platform::Pallet::<Runtime>::on_initialize(15300);

            let first_pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(XSTUSD, CERES_ASSET_ID);
            let second_pool_infos =
                demeter_farming_platform::Pools::<Runtime>::get(XOR, CERES_ASSET_ID);
            assert_eq!(first_pool_infos[0].rewards_to_be_distributed, 0);
            assert_eq!(second_pool_infos[0].rewards_to_be_distributed, 0);
            assert_eq!(first_pool_infos[0].reward_per_token, 0);
            assert_eq!(second_pool_infos[0].reward_per_token, 0);
        });
    }

    #[test]
    fn denominate_returns_error_without_silent_fallback() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let token_info = TokenInfo {
                farms_total_multiplier: 0,
                staking_total_multiplier: 0,
                token_per_block: balance!(1),
                farms_allocation: 0,
                staking_allocation: 0,
                team_allocation: 0,
                team_account: ALICE,
            };
            demeter_farming_platform::TokenInfos::<Runtime>::insert(XOR, token_info);

            assert_err!(
                demeter_farming_platform::DenominateXorAndTbcd::<Runtime>::on_denominate(&0),
                demeter_farming_platform::Error::<Runtime>::ArithmeticError
            );

            assert_eq!(
                demeter_farming_platform::TokenInfos::<Runtime>::get(XOR)
                    .unwrap()
                    .token_per_block,
                balance!(1)
            );
        });
    }

    #[test]
    fn denominate_zero_factor_rolls_back_pools_users_and_token_infos() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let token_info = || TokenInfo {
                farms_total_multiplier: 0,
                staking_total_multiplier: 0,
                token_per_block: balance!(100),
                farms_allocation: balance!(0.6),
                staking_allocation: balance!(0.2),
                team_allocation: balance!(0.2),
                team_account: ALICE,
            };
            let pool_info = || PoolData {
                multiplier: 1,
                deposit_fee: 0,
                is_core: true,
                is_farm: true,
                total_tokens_in_pool: balance!(50),
                rewards: balance!(20),
                rewards_to_be_distributed: balance!(10),
                reward_per_token: balance!(2),
                is_removed: false,
                base_asset: XOR,
            };
            let user_info = || UserInfo {
                base_asset: XOR,
                pool_asset: XSTUSD,
                reward_asset: XOR,
                is_farm: true,
                pooled_tokens: balance!(5),
                rewards: balance!(3),
                reward_per_token_paid: balance!(2),
            };

            demeter_farming_platform::TokenInfos::<Runtime>::insert(XOR, token_info());
            demeter_farming_platform::Pools::<Runtime>::insert(XSTUSD, XOR, vec![pool_info()]);
            demeter_farming_platform::UserInfos::<Runtime>::insert(ALICE, vec![user_info()]);

            assert_err!(
                demeter_farming_platform::DenominateXorAndTbcd::<Runtime>::on_denominate(&0),
                demeter_farming_platform::Error::<Runtime>::ArithmeticError
            );

            assert_eq!(
                demeter_farming_platform::TokenInfos::<Runtime>::get(XOR).unwrap(),
                token_info()
            );
            assert_eq!(
                demeter_farming_platform::Pools::<Runtime>::get(XSTUSD, XOR),
                vec![pool_info()]
            );
            assert_eq!(
                demeter_farming_platform::UserInfos::<Runtime>::get(ALICE),
                vec![user_info()]
            );
        });
    }

    #[test]
    fn distribute_rewards_to_pools_clears_stale_distribution_when_reserved_exceeds_available() {
        preset_initial(|| {
            let token_info = TokenInfo {
                farms_total_multiplier: 1,
                staking_total_multiplier: 0,
                token_per_block: balance!(1),
                farms_allocation: balance!(1),
                staking_allocation: 0,
                team_allocation: 0,
                team_account: BOB,
            };
            let pool_info = PoolData {
                multiplier: 1,
                deposit_fee: 0,
                is_core: true,
                is_farm: true,
                total_tokens_in_pool: balance!(10),
                rewards: balance!(2000),
                rewards_to_be_distributed: balance!(16),
                reward_per_token: 0,
                is_removed: false,
                base_asset: XOR,
            };

            demeter_farming_platform::TokenInfos::<Runtime>::insert(CERES_ASSET_ID, token_info);
            demeter_farming_platform::Pools::<Runtime>::insert(
                XOR,
                CERES_ASSET_ID,
                vec![pool_info],
            );

            demeter_farming_platform::Pallet::<Runtime>::on_initialize(14440);

            let pool_infos = demeter_farming_platform::Pools::<Runtime>::get(XOR, CERES_ASSET_ID);
            assert_eq!(pool_infos[0].rewards, balance!(2000));
            assert_eq!(pool_infos[0].rewards_to_be_distributed, 0);
            assert_eq!(pool_infos[0].reward_per_token, 0);
        });
    }

    #[test]
    fn activate_removed_pool_rejects_active_duplicate() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let pool_asset = XOR;
            let reward_asset = CERES_ASSET_ID;
            let is_farm = true;

            let active_pool_info = PoolData {
                multiplier: 1,
                deposit_fee: balance!(0),
                is_core: true,
                is_farm,
                total_tokens_in_pool: 0,
                rewards: 0,
                rewards_to_be_distributed: 0,
                reward_per_token: 0,
                is_removed: false,
                base_asset: XOR,
            };
            let removed_pool_info = PoolData {
                multiplier: 1,
                deposit_fee: balance!(0),
                is_core: true,
                is_farm,
                total_tokens_in_pool: 0,
                rewards: 0,
                rewards_to_be_distributed: 0,
                reward_per_token: 0,
                is_removed: true,
                base_asset: XOR,
            };
            let token_info = TokenInfo {
                farms_total_multiplier: 1,
                staking_total_multiplier: 0,
                token_per_block: balance!(1),
                farms_allocation: balance!(0.6),
                staking_allocation: balance!(0.2),
                team_allocation: balance!(0.2),
                team_account: BOB,
            };

            demeter_farming_platform::TokenInfos::<Runtime>::insert(&reward_asset, &token_info);
            demeter_farming_platform::Pools::<Runtime>::append(
                &pool_asset,
                &reward_asset,
                &active_pool_info,
            );
            demeter_farming_platform::Pools::<Runtime>::append(
                &pool_asset,
                &reward_asset,
                &removed_pool_info,
            );

            assert_err!(
                demeter_farming_platform::Pallet::<Runtime>::activate_removed_pool(
                    RuntimeOrigin::signed(
                        demeter_farming_platform::AuthorityAccount::<Runtime>::get()
                    ),
                    pool_asset,
                    pool_asset,
                    reward_asset,
                    is_farm,
                ),
                demeter_farming_platform::Error::<Runtime>::PoolAlreadyExists
            );

            let token_info =
                demeter_farming_platform::TokenInfos::<Runtime>::get(&reward_asset).unwrap();
            assert_eq!(token_info.farms_total_multiplier, 1);
        });
    }

    #[test]
    fn denominate_updates_xor_and_tbcd_token_info_without_cross_write() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let xor = XOR;
            let tbcd = TBCD;
            let factor = 10;
            let xor_info = TokenInfo {
                farms_total_multiplier: 7,
                staking_total_multiplier: 11,
                token_per_block: balance!(100),
                farms_allocation: balance!(0.6),
                staking_allocation: balance!(0.2),
                team_allocation: balance!(0.2),
                team_account: ALICE,
            };
            let tbcd_info = TokenInfo {
                farms_total_multiplier: 13,
                staking_total_multiplier: 17,
                token_per_block: balance!(50),
                farms_allocation: balance!(0.2),
                staking_allocation: balance!(0.3),
                team_allocation: balance!(0.5),
                team_account: BOB,
            };

            demeter_farming_platform::TokenInfos::<Runtime>::insert(&xor, &xor_info);
            demeter_farming_platform::TokenInfos::<Runtime>::insert(&tbcd, &tbcd_info);

            assert_ok!(
                demeter_farming_platform::DenominateXorAndTbcd::<Runtime>::on_denominate(&factor)
            );

            let xor_after = demeter_farming_platform::TokenInfos::<Runtime>::get(&xor).unwrap();
            let tbcd_after = demeter_farming_platform::TokenInfos::<Runtime>::get(&tbcd).unwrap();

            assert_eq!(xor_after.token_per_block, balance!(10));
            assert_eq!(tbcd_after.token_per_block, balance!(5));
            assert_eq!(xor_after.farms_allocation, balance!(0.6));
            assert_eq!(tbcd_after.farms_allocation, balance!(0.2));
            assert_eq!(xor_after.farms_total_multiplier, 7);
            assert_eq!(tbcd_after.farms_total_multiplier, 13);
            assert_eq!(xor_after.team_account, ALICE);
            assert_eq!(tbcd_after.team_account, BOB);
        });
    }

    #[test]
    fn denominate_scales_lp_positions_and_reward_checkpoints_by_issuance_ratio() {
        preset_initial(|| {
            let dex_id = DEX_A_ID;
            let xor = XOR;
            let ceres = CERES_ASSET_ID;
            let factor = 10;

            assert_ok!(pool_xyk::Pallet::<Runtime>::deposit_liquidity(
                RuntimeOrigin::signed(ALICE),
                dex_id,
                xor,
                ceres,
                balance!(500),
                balance!(700),
                balance!(500),
                balance!(700),
            ));

            let pool_account: AccountId =
                <Runtime as ceres_liquidity_locker::Config>::XYKPool::properties(xor, ceres)
                    .expect("Pool does not exist")
                    .0;
            let lp_tokens = <Runtime as ceres_liquidity_locker::Config>::XYKPool::pool_providers(
                pool_account.clone(),
                ALICE,
            )
            .expect("User is not pool provider");

            let pool_info = PoolData {
                multiplier: 1,
                deposit_fee: 0,
                is_core: true,
                is_farm: true,
                total_tokens_in_pool: lp_tokens,
                rewards: balance!(20),
                rewards_to_be_distributed: 0,
                reward_per_token: balance!(2),
                is_removed: false,
                base_asset: xor,
            };
            demeter_farming_platform::Pools::<Runtime>::append(&ceres, &ceres, &pool_info);
            demeter_farming_platform::UserInfos::<Runtime>::append(
                ALICE,
                UserInfo {
                    base_asset: xor,
                    pool_asset: ceres,
                    reward_asset: ceres,
                    is_farm: true,
                    pooled_tokens: lp_tokens,
                    rewards: balance!(5),
                    reward_per_token_paid: balance!(2),
                },
            );

            assert_ok!(assets::Pallet::<Runtime>::update_balance(
                RuntimeOrigin::root(),
                pool_account,
                xor,
                -(balance!(450) as i128),
            ));

            assert_ok!(
                demeter_farming_platform::DenominateXorAndTbcd::<Runtime>::on_denominate(&factor)
            );

            let pool_infos = demeter_farming_platform::Pools::<Runtime>::get(&ceres, &ceres);
            let updated_pool = pool_infos
                .iter()
                .find(|pool_info| pool_info.base_asset == xor && pool_info.is_farm)
                .expect("pool should remain after denomination");
            let user_infos = demeter_farming_platform::UserInfos::<Runtime>::get(ALICE);
            let updated_user = user_infos
                .iter()
                .find(|user_info| {
                    user_info.base_asset == xor
                        && user_info.pool_asset == ceres
                        && user_info.reward_asset == ceres
                        && user_info.is_farm
                })
                .expect("user should remain after denomination");

            assert!(updated_pool.total_tokens_in_pool < lp_tokens);
            assert!(updated_user.pooled_tokens < lp_tokens);
            assert!(updated_pool.reward_per_token > balance!(2));
            assert!(updated_user.reward_per_token_paid > balance!(2));
            assert_eq!(
                updated_pool.reward_per_token,
                updated_user.reward_per_token_paid
            );
            assert_eq!(updated_pool.rewards, balance!(20));
            assert_eq!(updated_user.rewards, balance!(5));
        });
    }
}
