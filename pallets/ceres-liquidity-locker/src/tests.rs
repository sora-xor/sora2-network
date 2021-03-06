use common::prelude::FixedWrapper;
use common::{
    balance, AssetName, AssetSymbol, Balance, LiquiditySourceType, ToFeeAccount,
    DEFAULT_BALANCE_PRECISION, DOT, XOR,
};
use frame_support::{assert_err, assert_ok};

use crate::mock::*;

fn preset_initial<Fun>(tests: Fun)
where
    Fun: Fn(DEXId),
{
    let mut ext = ExtBuilder::default().build();
    let dex_id = DEX_A_ID;
    let xor: AssetId = XOR.into();
    let ceres: AssetId = CERES_ASSET_ID.into();

    ext.execute_with(|| {
        assert_ok!(assets::Module::<Runtime>::register_asset_id(
            ALICE(),
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
            ALICE(),
            CERES_ASSET_ID.into(),
            AssetSymbol(b"CERES".to_vec()),
            AssetName(b"Ceres".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::from(0u32),
            true,
            None,
            None,
        ));

        assert_ok!(trading_pair::Module::<Runtime>::register(
            Origin::signed(BOB()),
            dex_id.clone(),
            XOR.into(),
            CERES_ASSET_ID.into()
        ));

        assert_ok!(pool_xyk::Module::<Runtime>::initialize_pool(
            Origin::signed(BOB()),
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

        assert_ok!(assets::Module::<Runtime>::mint_to(
            &xor,
            &ALICE(),
            &ALICE(),
            balance!(900000)
        ));

        assert_ok!(assets::Module::<Runtime>::mint_to(
            &ceres,
            &ALICE(),
            &ALICE(),
            balance!(900000)
        ));

        assert_ok!(assets::Module::<Runtime>::mint_to(
            &xor,
            &ALICE(),
            &BOB(),
            balance!(900000)
        ));

        assert_ok!(assets::Module::<Runtime>::mint_to(
            &ceres,
            &ALICE(),
            &BOB(),
            balance!(900000)
        ));

        assert_eq!(
            assets::Module::<Runtime>::free_balance(&xor, &ALICE()).unwrap(),
            balance!(900000)
        );
        assert_eq!(
            assets::Module::<Runtime>::free_balance(&ceres, &ALICE()).unwrap(),
            balance!(902000)
        );
        assert_eq!(
            assets::Module::<Runtime>::free_balance(&xor, &repr.clone()).unwrap(),
            0
        );

        assert_eq!(
            assets::Module::<Runtime>::free_balance(&ceres, &repr.clone()).unwrap(),
            0
        );
        assert_eq!(
            assets::Module::<Runtime>::free_balance(&xor, &fee_repr.clone()).unwrap(),
            0
        );

        assert_eq!(
            pool_xyk::Module::<Runtime>::properties(xor, ceres),
            Some((repr.clone(), fee_repr.clone()))
        );

        tests(dex_id);
    });
}

#[test]
fn lock_liquidity_ok_with_first_fee_option() {
    preset_initial(|dex_id| {
        let base_asset: AssetId = XOR.into();
        let target_asset: AssetId = CERES_ASSET_ID.into();

        // Deposit liquidity to XOR/CERES pair
        assert_ok!(pool_xyk::Pallet::<Runtime>::deposit_liquidity(
            Origin::signed(ALICE()),
            dex_id,
            base_asset,
            target_asset,
            balance!(360000),
            balance!(144000),
            balance!(360000),
            balance!(144000),
        ));

        // Get pool account
        let pool_account: AccountId =
            <Runtime as ceres_liquidity_locker::Config>::XYKPool::properties(
                base_asset,
                target_asset,
            )
            .expect("Pool does not exist")
            .0;

        // Calculate number of pool tokens of user's account
        let pool_tokens: Balance =
            <Runtime as ceres_liquidity_locker::Config>::XYKPool::pool_providers(
                pool_account.clone(),
                ALICE(),
            )
            .expect("User is not pool provider");

        // Percentage of LP to lock and fee percentage for Option 1
        let lp_percentage = balance!(0.5);
        let fee_percentage = FixedWrapper::from(0.01);

        // Number of pool tokens to lock and fee in LP tokens
        let pool_tokens_to_lock =
            FixedWrapper::from(pool_tokens) * FixedWrapper::from(lp_percentage);
        let lp_fee = (pool_tokens_to_lock * fee_percentage)
            .try_into_balance()
            .unwrap_or(0);

        assert_ok!(ceres_liquidity_locker::Pallet::<Runtime>::lock_liquidity(
            Origin::signed(ALICE()),
            base_asset,
            target_asset,
            frame_system::Pallet::<Runtime>::block_number() + 5,
            lp_percentage,
            true
        ));

        // Calculate number of user's pool tokens after locking
        let pool_tokens_after_locking =
            <Runtime as ceres_liquidity_locker::Config>::XYKPool::pool_providers(
                pool_account.clone(),
                ALICE(),
            )
            .expect("User is not pool provider");

        let lp_to_check = pool_tokens - lp_fee;
        assert_eq!(pool_tokens_after_locking, lp_to_check);

        // Calculate number of fee account pool tokens after locking
        let fee_account: AccountId = ceres_liquidity_locker::FeesOptionOneAccount::<Runtime>::get();
        let fee_account_pool_tokens_after_locking =
            <Runtime as ceres_liquidity_locker::Config>::XYKPool::pool_providers(
                pool_account.clone(),
                fee_account,
            )
            .expect("User is not pool provider");
        assert_eq!(fee_account_pool_tokens_after_locking, lp_fee);

        // Check if added to account_pools
        let target_asset_expected =
            <Runtime as ceres_liquidity_locker::Config>::XYKPool::account_pools(fee_account);
        assert_eq!(
            target_asset_expected.get(&target_asset),
            Some(&target_asset)
        );
    });
}

#[test]
fn lock_liquidity_ok_with_second_fee_option() {
    preset_initial(|dex_id| {
        let base_asset: AssetId = XOR.into();
        let target_asset: AssetId = CERES_ASSET_ID.into();

        // Deposit liquidity to XOR/CERES pair
        assert_ok!(pool_xyk::Pallet::<Runtime>::deposit_liquidity(
            Origin::signed(ALICE()),
            dex_id,
            base_asset,
            target_asset,
            balance!(360000),
            balance!(144000),
            balance!(360000),
            balance!(144000),
        ));

        // Get pool account
        let pool_account: AccountId =
            <Runtime as ceres_liquidity_locker::Config>::XYKPool::properties(
                base_asset,
                target_asset,
            )
            .expect("Pool does not exist")
            .0;

        // Calculate number of pool tokens of user's account
        let pool_tokens: Balance =
            <Runtime as ceres_liquidity_locker::Config>::XYKPool::pool_providers(
                pool_account.clone(),
                ALICE(),
            )
            .expect("User is not pool provider");

        // Percentage of LP to lock and fee percentage for Option 1
        let lp_percentage = balance!(0.5);
        let fee_percentage = FixedWrapper::from(0.005);

        // Number of pool tokens to lock and fee in LP tokens
        let pool_tokens_to_lock =
            FixedWrapper::from(pool_tokens) * FixedWrapper::from(lp_percentage);
        let lp_fee = (pool_tokens_to_lock * fee_percentage)
            .try_into_balance()
            .unwrap_or(0);

        assert_ok!(ceres_liquidity_locker::Pallet::<Runtime>::lock_liquidity(
            Origin::signed(ALICE()),
            base_asset,
            target_asset,
            frame_system::Pallet::<Runtime>::block_number() + 5,
            lp_percentage,
            false
        ));

        // Check if 20 CERES fee is paid
        let fee_account: AccountId = ceres_liquidity_locker::FeesOptionTwoAccount::<Runtime>::get();
        assert_eq!(
            assets::Pallet::<Runtime>::free_balance(&target_asset, &fee_account)
                .expect("Failed to query free balance."),
            balance!(20)
        );

        // Calculate number of user's pool tokens after locking
        let pool_tokens_after_locking =
            <Runtime as ceres_liquidity_locker::Config>::XYKPool::pool_providers(
                pool_account.clone(),
                ALICE(),
            )
            .expect("User is not pool provider");

        let lp_to_check = pool_tokens - lp_fee;
        assert_eq!(pool_tokens_after_locking, lp_to_check);

        // Calculate number of fee account pool tokens after locking
        let fee_account_pool_tokens_after_locking =
            <Runtime as ceres_liquidity_locker::Config>::XYKPool::pool_providers(
                pool_account.clone(),
                fee_account.clone(),
            )
            .expect("User is not pool provider");
        assert_eq!(fee_account_pool_tokens_after_locking, lp_fee);

        // Check if added to account_pools
        let target_asset_expected =
            <Runtime as ceres_liquidity_locker::Config>::XYKPool::account_pools(fee_account);
        assert_eq!(
            target_asset_expected.get(&target_asset),
            Some(&target_asset)
        );
    });
}

#[test]
fn lock_liquidity_invalid_percentage() {
    preset_initial(|_dex_id| {
        assert_err!(
            ceres_liquidity_locker::Pallet::<Runtime>::lock_liquidity(
                Origin::signed(ALICE()),
                XOR.into(),
                CERES_ASSET_ID.into(),
                frame_system::Pallet::<Runtime>::block_number() + 1,
                balance!(1.1),
                true,
            ),
            ceres_liquidity_locker::Error::<Runtime>::InvalidPercentage
        );
    });
}

#[test]
fn lock_liquidity_invalid_unlocking_block() {
    preset_initial(|_dex_id| {
        assert_err!(
            ceres_liquidity_locker::Pallet::<Runtime>::lock_liquidity(
                Origin::signed(ALICE()),
                XOR.into(),
                CERES_ASSET_ID.into(),
                frame_system::Pallet::<Runtime>::block_number(),
                balance!(0.8),
                true,
            ),
            ceres_liquidity_locker::Error::<Runtime>::InvalidUnlockingBlock
        );
    });
}

#[test]
fn lock_liquidity_pool_does_not_exist() {
    preset_initial(|_dex_id| {
        assert_err!(
            ceres_liquidity_locker::Pallet::<Runtime>::lock_liquidity(
                Origin::signed(ALICE()),
                XOR.into(),
                DOT.into(),
                frame_system::Pallet::<Runtime>::block_number() + 1,
                balance!(0.5),
                true,
            ),
            ceres_liquidity_locker::Error::<Runtime>::PoolDoesNotExist
        );
    });
}

#[test]
fn lock_liquidity_user_is_not_pool_provider() {
    preset_initial(|_dex_id| {
        assert_err!(
            ceres_liquidity_locker::Pallet::<Runtime>::lock_liquidity(
                Origin::signed(ALICE()),
                XOR.into(),
                CERES_ASSET_ID.into(),
                frame_system::Pallet::<Runtime>::block_number() + 1,
                balance!(0.5),
                true
            ),
            ceres_liquidity_locker::Error::<Runtime>::InsufficientLiquidityToLock
        );
    });
}

#[test]
fn lock_liquidity_insufficient_liquidity_to_lock() {
    preset_initial(|dex_id| {
        assert_ok!(pool_xyk::Pallet::<Runtime>::deposit_liquidity(
            Origin::signed(ALICE()),
            dex_id,
            XOR.into(),
            CERES_ASSET_ID.into(),
            balance!(360000),
            balance!(144000),
            balance!(360000),
            balance!(144000),
        ));

        // Lock 50% of LP tokens
        assert_ok!(ceres_liquidity_locker::Pallet::<Runtime>::lock_liquidity(
            Origin::signed(ALICE()),
            XOR.into(),
            CERES_ASSET_ID.into(),
            frame_system::Pallet::<Runtime>::block_number() + 5,
            balance!(0.5),
            true
        ));

        // Lock 30% of LP tokens
        assert_ok!(ceres_liquidity_locker::Pallet::<Runtime>::lock_liquidity(
            Origin::signed(ALICE()),
            XOR.into(),
            CERES_ASSET_ID.into(),
            frame_system::Pallet::<Runtime>::block_number() + 5,
            balance!(0.3),
            true
        ));

        // Try to lock 30% of LP tokens
        assert_err!(
            ceres_liquidity_locker::Pallet::<Runtime>::lock_liquidity(
                Origin::signed(ALICE()),
                XOR.into(),
                CERES_ASSET_ID.into(),
                frame_system::Pallet::<Runtime>::block_number() + 5,
                balance!(0.3),
                true
            ),
            ceres_liquidity_locker::Error::<Runtime>::InsufficientLiquidityToLock
        );
    });
}

#[test]
fn change_ceres_fee_unauthorized() {
    preset_initial(|_dex_id| {
        assert_err!(
            ceres_liquidity_locker::Pallet::<Runtime>::change_ceres_fee(
                Origin::signed(ALICE()),
                balance!(100)
            ),
            ceres_liquidity_locker::Error::<Runtime>::Unauthorized
        );
    });
}

#[test]
fn change_ceres_fee_ok() {
    preset_initial(|_dex_id| {
        assert_ok!(ceres_liquidity_locker::Pallet::<Runtime>::change_ceres_fee(
            Origin::signed(AUTHORITY::<Runtime>()),
            balance!(100)
        ));

        assert_eq!(
            ceres_liquidity_locker::FeesOptionTwoCeresAmount::<Runtime>::get(),
            balance!(100)
        );
    });
}

#[test]
fn should_remove_expired_lockups() {
    preset_initial(|dex_id| {
        assert_ok!(pool_xyk::Pallet::<Runtime>::deposit_liquidity(
            Origin::signed(ALICE()),
            dex_id,
            XOR.into(),
            CERES_ASSET_ID.into(),
            balance!(360000),
            balance!(144000),
            balance!(360000),
            balance!(144000),
        ));

        // Lock 50% of LP tokens
        assert_ok!(ceres_liquidity_locker::Pallet::<Runtime>::lock_liquidity(
            Origin::signed(ALICE()),
            XOR.into(),
            CERES_ASSET_ID.into(),
            frame_system::Pallet::<Runtime>::block_number() + 5,
            balance!(0.5),
            true
        ));

        // Lock 30% of LP tokens
        assert_ok!(ceres_liquidity_locker::Pallet::<Runtime>::lock_liquidity(
            Origin::signed(ALICE()),
            XOR.into(),
            CERES_ASSET_ID.into(),
            frame_system::Pallet::<Runtime>::block_number() + 500,
            balance!(0.3),
            true
        ));

        assert_ok!(pool_xyk::Pallet::<Runtime>::deposit_liquidity(
            Origin::signed(BOB()),
            dex_id,
            XOR.into(),
            CERES_ASSET_ID.into(),
            balance!(360000),
            balance!(144000),
            balance!(360000),
            balance!(144000),
        ));

        // Lock 50% of LP tokens
        assert_ok!(ceres_liquidity_locker::Pallet::<Runtime>::lock_liquidity(
            Origin::signed(BOB()),
            XOR.into(),
            CERES_ASSET_ID.into(),
            frame_system::Pallet::<Runtime>::block_number() + 250,
            balance!(0.5),
            true
        ));

        // Lock 30% of LP tokens
        assert_ok!(ceres_liquidity_locker::Pallet::<Runtime>::lock_liquidity(
            Origin::signed(BOB()),
            XOR.into(),
            CERES_ASSET_ID.into(),
            frame_system::Pallet::<Runtime>::block_number() + 20000,
            balance!(0.3),
            true
        ));

        let mut lockups_alice = ceres_liquidity_locker::LockerData::<Runtime>::get(ALICE());
        assert_eq!(lockups_alice.len(), 2);
        let mut lockups_bob = ceres_liquidity_locker::LockerData::<Runtime>::get(BOB());
        assert_eq!(lockups_bob.len(), 2);

        run_to_block(14_440);

        lockups_alice = ceres_liquidity_locker::LockerData::<Runtime>::get(ALICE());
        assert_eq!(lockups_alice.len(), 0);
        lockups_bob = ceres_liquidity_locker::LockerData::<Runtime>::get(BOB());
        assert_eq!(lockups_bob.len(), 1);

        assert_eq!(lockups_bob.get(0).unwrap().unlocking_block, 20000);
    });
}

#[test]
fn check_if_has_enough_unlocked_liquidity_pool_does_not_exist() {
    preset_initial(|_dex_id| {
        assert_eq!(
            ceres_liquidity_locker::Pallet::<Runtime>::check_if_has_enough_unlocked_liquidity(
                &ALICE(),
                XOR.into(),
                DOT.into(),
                balance!(0.3),
            ),
            false
        );
    });
}

#[test]
fn check_if_has_enough_unlocked_liquidity_user_is_not_pool_provider() {
    preset_initial(|_dex_id| {
        assert_eq!(
            ceres_liquidity_locker::Pallet::<Runtime>::check_if_has_enough_unlocked_liquidity(
                &ALICE(),
                XOR.into(),
                CERES_ASSET_ID.into(),
                balance!(0.3)
            ),
            false
        );
    });
}

#[test]
fn check_if_has_enough_unlocked_liquidity_true() {
    preset_initial(|dex_id| {
        assert_ok!(pool_xyk::Pallet::<Runtime>::deposit_liquidity(
            Origin::signed(ALICE()),
            dex_id,
            XOR.into(),
            CERES_ASSET_ID.into(),
            balance!(360),
            balance!(144),
            balance!(360),
            balance!(144),
        ));

        // Lock 50% of LP tokens
        assert_ok!(ceres_liquidity_locker::Pallet::<Runtime>::lock_liquidity(
            Origin::signed(ALICE()),
            XOR.into(),
            CERES_ASSET_ID.into(),
            frame_system::Pallet::<Runtime>::block_number() + 5,
            balance!(0.5),
            true
        ));

        assert_eq!(
            ceres_liquidity_locker::Pallet::<Runtime>::check_if_has_enough_unlocked_liquidity(
                &ALICE(),
                XOR.into(),
                CERES_ASSET_ID.into(),
                balance!(1)
            ),
            true
        );
    });
}

#[test]
fn check_if_has_enough_unlocked_liquidity_false() {
    preset_initial(|dex_id| {
        assert_ok!(pool_xyk::Pallet::<Runtime>::deposit_liquidity(
            Origin::signed(ALICE()),
            dex_id,
            XOR.into(),
            CERES_ASSET_ID.into(),
            balance!(360),
            balance!(144),
            balance!(360),
            balance!(144),
        ));

        // Lock 50% of LP tokens
        assert_ok!(ceres_liquidity_locker::Pallet::<Runtime>::lock_liquidity(
            Origin::signed(ALICE()),
            XOR.into(),
            CERES_ASSET_ID.into(),
            frame_system::Pallet::<Runtime>::block_number() + 5,
            balance!(1),
            true
        ));

        assert_eq!(
            ceres_liquidity_locker::Pallet::<Runtime>::check_if_has_enough_unlocked_liquidity(
                &ALICE(),
                XOR.into(),
                CERES_ASSET_ID.into(),
                balance!(10)
            ),
            false
        );
    });
}
