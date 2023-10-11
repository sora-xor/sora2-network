use crate::mock::*;
use crate::{AccountIdOf, AssetIdOf};
use common::prelude::FixedWrapper;
use common::{
    balance, generate_storage_instance, AssetId32, AssetInfoProvider, AssetName, AssetSymbol,
    Balance, LiquiditySourceType, PoolXykPallet, PredefinedAssetId, ToFeeAccount,
    TradingPairSourceManager, CERES_ASSET_ID, DEFAULT_BALANCE_PRECISION, DEMETER_ASSET_ID, XOR,
    XSTUSD,
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
    let xor: AssetId = XOR;
    let ceres: AssetId = CERES_ASSET_ID;
    let xstusd: AssetId = XSTUSD;
    let util: AssetId32<PredefinedAssetId> = AssetId32::from_bytes(hex!(
        "007348eb8f0f3cec730fbf5eec1b6a842c54d1df8bed75a9df084d5ee013e814"
    ));
    let pallet_account = PalletId(*b"deofarms").into_account_truncating();

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
            XSTUSD,
            AssetSymbol(b"XSTUSD".to_vec()),
            AssetName(b"SORA Synthetic USD".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::from(0u32),
            true,
            None,
            None,
        ));

        frame_system::Pallet::<Runtime>::inc_providers(
            &demeter_farming_platform::AuthorityAccount::<Runtime>::get(),
        );
        assert_ok!(assets::Pallet::<Runtime>::register_asset_id(
            demeter_farming_platform::AuthorityAccount::<Runtime>::get(),
            DEMETER_ASSET_ID,
            AssetSymbol(b"DEO".to_vec()),
            AssetName(b"Demeter".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::from(0u32),
            true,
            None,
            None,
        ));

        assert_ok!(assets::Pallet::<Runtime>::register_asset_id(
            ALICE,
            util,
            AssetSymbol(b"UTIL".to_vec()),
            AssetName(b"Util".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::from(0u32),
            true,
            None,
            None,
        ));

        /************ XOR DEX ************/
        assert_ok!(trading_pair::Pallet::<Runtime>::register(
            RuntimeOrigin::signed(BOB),
            dex_id,
            XOR,
            CERES_ASSET_ID
        ));

        assert_ok!(pool_xyk::Pallet::<Runtime>::initialize_pool(
            RuntimeOrigin::signed(BOB),
            dex_id,
            XOR,
            CERES_ASSET_ID,
        ));

        assert!(
            trading_pair::Pallet::<Runtime>::is_source_enabled_for_trading_pair(
                &dex_id,
                &XOR,
                &CERES_ASSET_ID,
                LiquiditySourceType::XYKPool,
            )
            .expect("Failed to query trading pair status.")
        );

        let (_tpair, tech_acc_id) =
            pool_xyk::Pallet::<Runtime>::tech_account_from_dex_and_asset_pair(
                dex_id,
                XOR,
                CERES_ASSET_ID,
            )
            .unwrap();

        let fee_acc = tech_acc_id.to_fee_account().unwrap();
        let repr: AccountId =
            technical::Pallet::<Runtime>::tech_account_id_to_account_id(&tech_acc_id).unwrap();
        let fee_repr: AccountId =
            technical::Pallet::<Runtime>::tech_account_id_to_account_id(&fee_acc).unwrap();

        assert_eq!(
            pool_xyk::Pallet::<Runtime>::properties(xor, ceres),
            Some((repr, fee_repr))
        );

        /********* XSTUSD DEX ********/
        assert_ok!(trading_pair::Pallet::<Runtime>::register(
            RuntimeOrigin::signed(BOB),
            dex_id_xst,
            XSTUSD,
            CERES_ASSET_ID
        ));

        assert_ok!(pool_xyk::Pallet::<Runtime>::initialize_pool(
            RuntimeOrigin::signed(BOB),
            dex_id_xst,
            XSTUSD,
            CERES_ASSET_ID,
        ));

        assert!(
            trading_pair::Pallet::<Runtime>::is_source_enabled_for_trading_pair(
                &dex_id_xst,
                &XSTUSD,
                &CERES_ASSET_ID,
                LiquiditySourceType::XYKPool,
            )
            .expect("Failed to query trading pair status.")
        );

        let (_tpair_xst, tech_acc_id_xst) =
            pool_xyk::Pallet::<Runtime>::tech_account_from_dex_and_asset_pair(
                dex_id_xst,
                XSTUSD,
                CERES_ASSET_ID,
            )
            .unwrap();

        let fee_acc_xst = tech_acc_id_xst.to_fee_account().unwrap();
        let repr_xst: AccountId =
            technical::Pallet::<Runtime>::tech_account_id_to_account_id(&tech_acc_id_xst).unwrap();
        let fee_repr_xst: AccountId =
            technical::Pallet::<Runtime>::tech_account_id_to_account_id(&fee_acc_xst).unwrap();

        assert_eq!(
            pool_xyk::Pallet::<Runtime>::properties(xstusd, ceres),
            Some((repr_xst, fee_repr_xst))
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

        demeter_farming_platform::TokenInfos::<Runtime>::insert(reward_asset, token_info);

        assert_err!(
            demeter_farming_platform::Pallet::<Runtime>::register_token(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
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
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
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
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
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
            demeter_farming_platform::TokenInfos::<Runtime>::get(reward_asset).unwrap();

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
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
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
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
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
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
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
            is_removed: false,
            base_asset: XOR,
        };

        demeter_farming_platform::Pools::<Runtime>::append(pool_asset, reward_asset, pool_info);

        assert_err!(
            demeter_farming_platform::Pallet::<Runtime>::add_pool(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
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
            demeter_farming_platform::TokenInfos::<Runtime>::get(reward_asset).unwrap();
        assert_eq!(token_info.farms_total_multiplier, multiplier);

        let pool_infos = demeter_farming_platform::Pools::<Runtime>::get(pool_asset, reward_asset);
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
            demeter_farming_platform::TokenInfos::<Runtime>::get(reward_asset).unwrap();
        assert_eq!(token_info.farms_total_multiplier, multiplier);

        let pool_infos = demeter_farming_platform::Pools::<Runtime>::get(pool_asset, reward_asset);
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

        let pool_infos = demeter_farming_platform::Pools::<Runtime>::get(pool_asset, reward_asset);
        for p_info in &pool_infos {
            if !p_info.is_removed && p_info.is_farm == is_farm && p_info.base_asset == reward_asset
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
            Assets::free_balance(&CERES_ASSET_ID, &ALICE).expect("Failed to query free balance."),
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
            demeter_farming_platform::Pools::<Runtime>::get(asset_ceres, asset_ceres);
        for p_info in &pool_infos {
            if !p_info.is_removed && p_info.is_farm == is_farm && p_info.base_asset == asset_xor {
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
            pool_account,
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

        pool_infos = demeter_farming_platform::Pools::<Runtime>::get(asset_ceres, asset_ceres);
        for p_info in &pool_infos {
            if !p_info.is_removed && p_info.is_farm == is_farm && p_info.base_asset == asset_xor {
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
            demeter_farming_platform::Pools::<Runtime>::get(asset_ceres, asset_ceres);
        for p_info in &pool_infos {
            if !p_info.is_removed && p_info.is_farm == is_farm && p_info.base_asset == asset_xstusd
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
            pool_account,
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

        pool_infos = demeter_farming_platform::Pools::<Runtime>::get(asset_ceres, asset_ceres);
        for p_info in &pool_infos {
            if !p_info.is_removed && p_info.is_farm == is_farm && p_info.base_asset == asset_xstusd
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
            is_removed: false,
            base_asset: XOR,
        };

        demeter_farming_platform::Pools::<Runtime>::append(pool_asset, reward_asset, pool_info);

        let user_info = UserInfo {
            base_asset: XOR,
            pool_asset,
            reward_asset,
            is_farm,
            pooled_tokens,
            rewards,
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
            is_removed: false,
            base_asset: XOR,
        };

        demeter_farming_platform::Pools::<Runtime>::append(pool_asset, reward_asset, pool_info);

        let user_info = UserInfo {
            base_asset: XOR,
            pool_asset,
            reward_asset,
            is_farm,
            pooled_tokens,
            rewards,
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
            is_removed: false,
            base_asset: XOR,
        };

        demeter_farming_platform::Pools::<Runtime>::append(pool_asset, reward_asset, &pool_info);

        let user_info = UserInfo {
            base_asset: XOR,
            pool_asset,
            reward_asset,
            is_farm,
            pooled_tokens: balance!(1000),
            rewards: balance!(100),
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
            demeter_farming_platform::Pools::<Runtime>::get(pool_asset, reward_asset);
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
            Assets::free_balance(&CERES_ASSET_ID, &ALICE).expect("Failed to query free balance."),
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
            is_removed: false,
            base_asset: XSTUSD,
        };

        demeter_farming_platform::Pools::<Runtime>::append(pool_asset, reward_asset, &pool_info);

        let user_info = UserInfo {
            base_asset: XSTUSD,
            pool_asset,
            reward_asset,
            is_farm,
            pooled_tokens: balance!(1000),
            rewards: balance!(100),
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
            demeter_farming_platform::Pools::<Runtime>::get(pool_asset, reward_asset);
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
            Assets::free_balance(&CERES_ASSET_ID, &ALICE).expect("Failed to query free balance."),
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
            is_removed: false,
            base_asset,
        };

        demeter_farming_platform::Pools::<Runtime>::append(pool_asset, reward_asset, &pool_info);

        let user_info = UserInfo {
            base_asset,
            pool_asset,
            reward_asset,
            is_farm,
            pooled_tokens,
            rewards: 1,
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
            demeter_farming_platform::Pools::<Runtime>::get(pool_asset, reward_asset);
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
            demeter_farming_platform::Pools::<Runtime>::get(asset_ceres, asset_ceres);
        for p_info in &pool_infos {
            if !p_info.is_removed && p_info.is_farm == is_farm && p_info.base_asset == asset_xstusd
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
            pool_account,
            demeter_farming_platform::FeeAccount::<Runtime>::get(),
        )
        .unwrap_or(0);
        assert_eq!(lp_tokens, fee);

        assert_ok!(demeter_farming_platform::Pallet::<Runtime>::withdraw(
            RuntimeOrigin::signed(ALICE),
            asset_xstusd,
            asset_ceres,
            asset_xstusd,
            pooled_tokens,
            is_farm,
        ));

        user_infos = demeter_farming_platform::UserInfos::<Runtime>::get(&ALICE);

        for user_info in user_infos.iter_mut() {
            if user_info.pool_asset == asset_ceres
                && user_info.reward_asset == asset_xstusd
                && user_info.is_farm == is_farm
                && user_info.base_asset == asset_xstusd
            {
                assert_eq!(user_info.pooled_tokens, balance!(0));
            }
        }

        pool_infos = demeter_farming_platform::Pools::<Runtime>::get(asset_ceres, asset_xstusd);
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
            rewards_to_be_distributed: 0,
            is_removed: false,
            base_asset: XOR,
        };

        demeter_farming_platform::Pools::<Runtime>::append(pool_asset, reward_asset, &pool_info);

        assert_ok!(demeter_farming_platform::Pallet::<Runtime>::remove_pool(
            RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
            pool_asset,
            pool_asset,
            reward_asset,
            is_farm,
        ));

        let mut pool_infos =
            demeter_farming_platform::Pools::<Runtime>::get(pool_asset, reward_asset);
        for pool_info in pool_infos.iter_mut() {
            if pool_info.is_farm == is_farm && pool_info.base_asset == pool_asset {
                pool_info.is_removed = true;
            }
            assert!(pool_info.is_removed);
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
            is_removed: false,
            base_asset: XSTUSD,
        };

        demeter_farming_platform::Pools::<Runtime>::append(pool_asset, reward_asset, &pool_info);

        assert_ok!(demeter_farming_platform::Pallet::<Runtime>::remove_pool(
            RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
            pool_asset,
            pool_asset,
            reward_asset,
            is_farm,
        ));

        let mut pool_infos =
            demeter_farming_platform::Pools::<Runtime>::get(pool_asset, reward_asset);
        for pool_info in pool_infos.iter_mut() {
            if pool_info.is_farm == is_farm && pool_info.base_asset == pool_asset {
                pool_info.is_removed = true;
            }
            assert!(pool_info.is_removed);
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

        demeter_farming_platform::TokenInfos::<Runtime>::insert(reward_asset, token_info);

        assert_err!(
            demeter_farming_platform::Pallet::<Runtime>::change_pool_multiplier(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
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

        demeter_farming_platform::TokenInfos::<Runtime>::insert(reward_asset, &token_info);

        let pool_info = PoolData {
            multiplier: 1,
            deposit_fee: balance!(0),
            is_core: true,
            is_farm,
            total_tokens_in_pool: 0,
            rewards: 0,
            rewards_to_be_distributed: 0,
            is_removed: false,
            base_asset: XOR,
        };

        demeter_farming_platform::Pools::<Runtime>::append(pool_asset, reward_asset, &pool_info);

        assert_ok!(
            demeter_farming_platform::Pallet::<Runtime>::change_pool_multiplier(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                pool_asset,
                pool_asset,
                reward_asset,
                is_farm,
                new_multiplier,
            )
        );

        token_info = demeter_farming_platform::TokenInfos::<Runtime>::get(reward_asset).unwrap();
        let mut pool_infos =
            demeter_farming_platform::Pools::<Runtime>::get(pool_asset, reward_asset);
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

        demeter_farming_platform::TokenInfos::<Runtime>::insert(reward_asset, &token_info);

        let pool_info = PoolData {
            multiplier: 1,
            deposit_fee: balance!(0),
            is_core: true,
            is_farm,
            total_tokens_in_pool: 0,
            rewards: 0,
            rewards_to_be_distributed: 0,
            is_removed: false,
            base_asset: XOR,
        };

        demeter_farming_platform::Pools::<Runtime>::append(pool_asset, reward_asset, &pool_info);

        assert_ok!(
            demeter_farming_platform::Pallet::<Runtime>::change_pool_multiplier(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                pool_asset,
                pool_asset,
                reward_asset,
                is_farm,
                new_multiplier,
            )
        );

        token_info = demeter_farming_platform::TokenInfos::<Runtime>::get(reward_asset).unwrap();
        let mut pool_infos =
            demeter_farming_platform::Pools::<Runtime>::get(pool_asset, reward_asset);
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

        demeter_farming_platform::TokenInfos::<Runtime>::insert(reward_asset, &token_info);

        let pool_info = PoolData {
            multiplier: 1,
            deposit_fee: balance!(0),
            is_core: true,
            is_farm,
            total_tokens_in_pool: 0,
            rewards: 0,
            rewards_to_be_distributed: 0,
            is_removed: false,
            base_asset: XSTUSD,
        };

        demeter_farming_platform::Pools::<Runtime>::append(pool_asset, reward_asset, &pool_info);

        assert_ok!(
            demeter_farming_platform::Pallet::<Runtime>::change_pool_multiplier(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                pool_asset,
                pool_asset,
                reward_asset,
                is_farm,
                new_multiplier,
            )
        );

        token_info = demeter_farming_platform::TokenInfos::<Runtime>::get(reward_asset).unwrap();
        let mut pool_infos =
            demeter_farming_platform::Pools::<Runtime>::get(pool_asset, reward_asset);
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
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
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
            is_removed: false,
            base_asset: XOR,
        };

        demeter_farming_platform::Pools::<Runtime>::append(pool_asset, reward_asset, pool_info);

        deposit_fee = balance!(1.2);

        assert_err!(
            demeter_farming_platform::Pallet::<Runtime>::change_pool_deposit_fee(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
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
            is_removed: false,
            base_asset: XOR,
        };

        demeter_farming_platform::Pools::<Runtime>::append(pool_asset, reward_asset, &pool_info);

        deposit_fee = balance!(0.8);
        assert_ok!(
            demeter_farming_platform::Pallet::<Runtime>::change_pool_deposit_fee(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                pool_asset,
                pool_asset,
                reward_asset,
                is_farm,
                deposit_fee,
            )
        );

        let pool_infos = demeter_farming_platform::Pools::<Runtime>::get(pool_asset, reward_asset);
        for p_info in pool_infos {
            if !p_info.is_removed && p_info.is_farm == is_farm && p_info.base_asset == pool_asset {
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
            is_removed: false,
            base_asset: XSTUSD,
        };

        demeter_farming_platform::Pools::<Runtime>::append(pool_asset, reward_asset, &pool_info);

        deposit_fee = balance!(0.8);
        assert_ok!(
            demeter_farming_platform::Pallet::<Runtime>::change_pool_deposit_fee(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                pool_asset,
                pool_asset,
                reward_asset,
                is_farm,
                deposit_fee,
            )
        );

        let pool_infos = demeter_farming_platform::Pools::<Runtime>::get(pool_asset, reward_asset);
        for p_info in pool_infos {
            if !p_info.is_removed && p_info.is_farm == is_farm && p_info.base_asset == pool_asset {
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
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
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

        demeter_farming_platform::TokenInfos::<Runtime>::insert(reward_asset, token_info);

        assert_err!(
            demeter_farming_platform::Pallet::<Runtime>::change_token_info(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
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

        demeter_farming_platform::TokenInfos::<Runtime>::insert(reward_asset, token_info);

        assert_err!(
            demeter_farming_platform::Pallet::<Runtime>::change_token_info(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
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

        demeter_farming_platform::TokenInfos::<Runtime>::insert(reward_asset, &token_info);

        assert_ok!(
            demeter_farming_platform::Pallet::<Runtime>::change_token_info(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
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
            base_asset: XOR,
        };

        demeter_farming_platform::Pools::<Runtime>::append(pool_asset, reward_asset, &pool_info);

        assert_ok!(
            demeter_farming_platform::Pallet::<Runtime>::change_total_tokens(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                pool_asset,
                pool_asset,
                reward_asset,
                is_farm,
                total_tokens,
            )
        );

        let mut pool_infos =
            demeter_farming_platform::Pools::<Runtime>::get(pool_asset, reward_asset);
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
            is_removed: false,
            base_asset: XSTUSD,
        };

        demeter_farming_platform::Pools::<Runtime>::append(pool_asset, reward_asset, &pool_info);

        assert_ok!(
            demeter_farming_platform::Pallet::<Runtime>::change_total_tokens(
                RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
                pool_asset,
                pool_asset,
                reward_asset,
                is_farm,
                total_tokens,
            )
        );

        let mut pool_infos =
            demeter_farming_platform::Pools::<Runtime>::get(pool_asset, reward_asset);
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

        let user_info = UserInfo {
            base_asset,
            pool_asset,
            reward_asset,
            is_farm,
            pooled_tokens,
            rewards,
        };

        demeter_farming_platform::UserInfos::<Runtime>::append(ALICE, user_info);

        let pool_tokens = balance!(69);
        assert_ok!(demeter_farming_platform::Pallet::<Runtime>::change_info(
            RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
            ALICE,
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

        let user_info = UserInfo {
            base_asset: XSTUSD,
            pool_asset,
            reward_asset,
            is_farm,
            pooled_tokens,
            rewards,
        };

        demeter_farming_platform::UserInfos::<Runtime>::append(ALICE, user_info);

        let pool_tokens = balance!(69);
        assert_ok!(demeter_farming_platform::Pallet::<Runtime>::change_info(
            RuntimeOrigin::signed(demeter_farming_platform::AuthorityAccount::<Runtime>::get()),
            ALICE,
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
        let pool_infos = demeter_farming_platform::Pools::<Runtime>::get(ceres, deo);
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
        let token_info = demeter_farming_platform::TokenInfos::<Runtime>::get(deo).unwrap();
        assert_eq!(
            assets::Pallet::<Runtime>::free_balance(&deo, &token_info.team_account).unwrap(),
            balance!(576)
        );
        let user_info_alice = demeter_farming_platform::UserInfos::<Runtime>::get(&ALICE);
        for user_info in &user_info_alice {
            if user_info.pool_asset == ceres
                && user_info.reward_asset == deo
                && user_info.is_farm
                && user_info.base_asset == xor
            {
                assert_eq!(user_info.rewards, balance!(540));
            }
        }
        let user_info_bob = demeter_farming_platform::UserInfos::<Runtime>::get(&BOB);
        for user_info in &user_info_bob {
            if user_info.pool_asset == ceres
                && user_info.reward_asset == deo
                && user_info.is_farm
                && user_info.base_asset == xor
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
        let pool_infos = demeter_farming_platform::Pools::<Runtime>::get(ceres, util);
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
        let token_info = demeter_farming_platform::TokenInfos::<Runtime>::get(util).unwrap();
        assert_eq!(
            assets::Pallet::<Runtime>::free_balance(&util, &token_info.team_account).unwrap(),
            balance!(14.4)
        );
        for user_info in &user_info_alice {
            if user_info.pool_asset == ceres
                && user_info.reward_asset == util
                && user_info.is_farm
                && user_info.base_asset == xor
            {
                assert_eq!(user_info.rewards, balance!(9));
            }
        }
        for user_info in &user_info_bob {
            if user_info.pool_asset == ceres && user_info.reward_asset == util && !user_info.is_farm
            {
                assert_eq!(user_info.rewards, balance!(7.2));
            }
        }

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
            DEX_A_ID,
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
                pool_account,
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

        let pooled_tokens = (FixedWrapper::from(pool_tokens) * FixedWrapper::from(balance!(0.96)))
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

        assert!(
            demeter_farming_platform::Pallet::<Runtime>::check_if_has_enough_liquidity_out_of_farming(
                &ALICE,
                pool_asset,
                reward_asset,
                pooled_tokens_to_withdraw,
            )
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
                pool_account,
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

        assert!(
            !demeter_farming_platform::Pallet::<Runtime>::check_if_has_enough_liquidity_out_of_farming(
                &ALICE,
                pool_asset,
                reward_asset,
                pool_tokens,
            )
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
            DEX_B_ID,
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
                pool_account,
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

        let pooled_tokens = (FixedWrapper::from(pool_tokens) * FixedWrapper::from(balance!(0.96)))
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

        assert!(
            demeter_farming_platform::Pallet::<Runtime>::check_if_has_enough_liquidity_out_of_farming(
                &ALICE,
                pool_asset,
                reward_asset,
                pooled_tokens_to_withdraw,
            )
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
                pool_account,
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

        assert!(
            !demeter_farming_platform::Pallet::<Runtime>::check_if_has_enough_liquidity_out_of_farming(
                &ALICE,
                pool_asset,
                reward_asset,
                pool_tokens,
            )
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
            pool_account,
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
            demeter_farming_platform::Pools::<Runtime>::get(reward_asset, reward_asset);
        for p_info in &pool_infos {
            if !p_info.is_removed && p_info.is_farm == is_farm && p_info.base_asset == pool_asset {
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
            pool_account,
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
            demeter_farming_platform::Pools::<Runtime>::get(reward_asset, reward_asset);
        for p_info in &pool_infos {
            if !p_info.is_removed && p_info.is_farm == is_farm && p_info.base_asset == pool_asset {
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

        let asset_xor: AssetId = XOR;
        let asset_ceres: AssetId = CERES_ASSET_ID;
        let asset_xstusd: AssetId = XSTUSD;

        #[allow(clippy::type_complexity)]
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

        #[allow(clippy::type_complexity)]
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

        #[allow(clippy::type_complexity)]
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

        #[allow(clippy::type_complexity)]
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

        let pools_a = demeter_farming_platform::Pools::<Runtime>::get(asset_ceres, asset_xstusd);
        for p_info in pools_a.iter() {
            if p_info.is_farm {
                assert_eq!(p_info.multiplier, 2u32);
                assert_eq!(p_info.deposit_fee, balance!(0.02));
                assert!(!p_info.is_core);
                assert_eq!(p_info.total_tokens_in_pool, balance!(100));
                assert_eq!(p_info.rewards, balance!(20));
                assert_eq!(p_info.rewards_to_be_distributed, balance!(12));
                assert!(!p_info.is_removed);
                assert_eq!(p_info.base_asset, asset_xor);
            } else {
                assert_eq!(p_info.multiplier, 3u32);
                assert_eq!(p_info.deposit_fee, balance!(0.01));
                assert!(p_info.is_core);
                assert_eq!(p_info.total_tokens_in_pool, balance!(120));
                assert_eq!(p_info.rewards, balance!(10));
                assert_eq!(p_info.rewards_to_be_distributed, balance!(2));
                assert!(!p_info.is_removed);
                assert_eq!(p_info.base_asset, asset_ceres);
            }
        }

        let pools_b = demeter_farming_platform::Pools::<Runtime>::get(asset_ceres, asset_ceres);
        for p_info in pools_b.iter() {
            assert_eq!(p_info.multiplier, 4u32);
            assert_eq!(p_info.deposit_fee, balance!(0.03));
            assert!(!p_info.is_core);
            assert_eq!(p_info.total_tokens_in_pool, balance!(130));
            assert_eq!(p_info.rewards, balance!(25));
            assert_eq!(p_info.rewards_to_be_distributed, balance!(8));
            assert!(!p_info.is_removed);
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

        let pools_a = demeter_farming_platform::Pools::<Runtime>::get(asset_ceres, asset_xstusd);
        for p_info in pools_a.iter() {
            if p_info.is_farm {
                assert_eq!(p_info.multiplier, 2u32);
                assert_eq!(p_info.deposit_fee, balance!(0.02));
                assert!(!p_info.is_core);
                assert_eq!(p_info.total_tokens_in_pool, balance!(100));
                assert_eq!(p_info.rewards, balance!(20));
                assert_eq!(p_info.rewards_to_be_distributed, balance!(12));
                assert!(!p_info.is_removed);
                assert_eq!(p_info.base_asset, asset_xor);
            } else {
                assert_eq!(p_info.multiplier, 3u32);
                assert_eq!(p_info.deposit_fee, balance!(0.01));
                assert!(p_info.is_core);
                assert_eq!(p_info.total_tokens_in_pool, balance!(120));
                assert_eq!(p_info.rewards, balance!(10));
                assert_eq!(p_info.rewards_to_be_distributed, balance!(2));
                assert!(!p_info.is_removed);
                assert_eq!(p_info.base_asset, asset_ceres);
            }
        }

        let pools_b = demeter_farming_platform::Pools::<Runtime>::get(asset_ceres, asset_ceres);
        for p_info in pools_b.iter() {
            assert_eq!(p_info.multiplier, 4u32);
            assert_eq!(p_info.deposit_fee, balance!(0.03));
            assert!(!p_info.is_core);
            assert_eq!(p_info.total_tokens_in_pool, balance!(130));
            assert_eq!(p_info.rewards, balance!(25));
            assert_eq!(p_info.rewards_to_be_distributed, balance!(8));
            assert!(!p_info.is_removed);
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
