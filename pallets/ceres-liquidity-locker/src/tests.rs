use common::prelude::FixedWrapper;
use common::{
    balance, AssetName, AssetSymbol, Balance, LiquiditySourceType, ToFeeAccount,
    DEFAULT_BALANCE_PRECISION,
};
use frame_support::{assert_err, assert_ok};

use crate::mock::*;
use sp_std::rc::Rc;

type PresetFunction<'a> = Rc<dyn Fn(DEXId, AssetId, AssetId) -> () + 'a>;

fn preset_initial(tests: Vec<PresetFunction<'a>>) {
    let mut ext = ExtBuilder::default().build();
    let dex_id = DEX_A_ID;
    let gt: AssetId = GoldenTicket.into();
    let ceres: AssetId = CERES_ASSET_ID.into();

    ext.execute_with(|| {
        assert_ok!(assets::Module::<Runtime>::register_asset_id(
            ALICE(),
            GoldenTicket.into(),
            AssetSymbol(b"GT".to_vec()),
            AssetName(b"Golden Ticket".to_vec()),
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
            GoldenTicket.into(),
            CERES_ASSET_ID.into()
        ));

        assert_ok!(pool_xyk::Module::<Runtime>::initialize_pool(
            Origin::signed(BOB()),
            dex_id.clone(),
            GoldenTicket.into(),
            CERES_ASSET_ID.into(),
        ));

        assert!(
            trading_pair::Module::<Runtime>::is_source_enabled_for_trading_pair(
                &dex_id,
                &GoldenTicket.into(),
                &CERES_ASSET_ID.into(),
                LiquiditySourceType::XYKPool,
            )
                .expect("Failed to query trading pair status.")
        );

        let (_tpair, tech_acc_id) =
            pool_xyk::Module::<Runtime>::tech_account_from_dex_and_asset_pair(
                dex_id.clone(),
                GoldenTicket.into(),
                CERES_ASSET_ID.into(),
            )
                .unwrap();

        let fee_acc = tech_acc_id.clone().to_fee_account().unwrap();
        let repr: AccountId =
            technical::Module::<Runtime>::tech_account_id_to_account_id(&tech_acc_id).unwrap();
        let fee_repr: AccountId =
            technical::Module::<Runtime>::tech_account_id_to_account_id(&fee_acc).unwrap();

        assert_ok!(assets::Module::<Runtime>::mint_to(
            &gt,
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
            &gt,
            &ALICE(),
            &CHARLIE(),
            balance!(900000)
        ));

        assert_eq!(
            assets::Module::<Runtime>::free_balance(&gt, &ALICE()).unwrap(),
            balance!(900000)
        );
        assert_eq!(
            assets::Module::<Runtime>::free_balance(&ceres, &ALICE()).unwrap(),
            balance!(902000)
        );
        assert_eq!(
            assets::Module::<Runtime>::free_balance(&gt, &repr.clone()).unwrap(),
            0
        );

        assert_eq!(
            assets::Module::<Runtime>::free_balance(&ceres, &repr.clone()).unwrap(),
            0
        );
        assert_eq!(
            assets::Module::<Runtime>::free_balance(&gt, &fee_repr.clone()).unwrap(),
            0
        );

        let base_asset: AssetId = GoldenTicket.into();
        let target_asset: AssetId = CERES_ASSET_ID.into();
        assert_eq!(
            pool_xyk::Module::<Runtime>::properties(base_asset, target_asset),
            Some((repr.clone(), fee_repr.clone()))
        );
        assert_eq!(
            pswap_distribution::Module::<Runtime>::subscribed_accounts(&fee_repr),
            Some((
                dex_id.clone(),
                repr.clone(),
                GetDefaultSubscriptionFrequency::get(),
                0
            ))
        );

        for test in &tests {
            test(dex_id.clone(), gt.clone(), ceres.clone());
        }
    });
}

#[test]
fn lock_liquidity_ok_with_first_fee_option() {
    preset_initial(vec![Rc::new(|dex_id, _gt, _bp| {
        let base_asset: AssetId = GoldenTicket.into();
        let target_asset: AssetId = CERES_ASSET_ID.into();

        // Deposit liquidity to GoldenTicket/CERES pair
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
    })]);
}

#[test]
fn lock_liquidity_ok_with_second_fee_option() {
    preset_initial(vec![Rc::new(|dex_id, _gt, _bp| {
        let base_asset: AssetId = GoldenTicket.into();
        let target_asset: AssetId = CERES_ASSET_ID.into();

        // Deposit liquidity to GoldenTicket/CERES pair
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
                fee_account,
            )
                .expect("User is not pool provider");
        assert_eq!(fee_account_pool_tokens_after_locking, lp_fee);
    })]);
}

#[test]
fn lock_liquidity_invalid_percentage() {
    preset_initial(vec![Rc::new(|_dex_id, _gt, _bp| {
        assert_err!(
            ceres_liquidity_locker::Pallet::<Runtime>::lock_liquidity(
                Origin::signed(ALICE()),
                GoldenTicket.into(),
                CERES_ASSET_ID.into(),
                frame_system::Pallet::<Runtime>::block_number(),
                balance!(1.1),
                true,
            ),
            ceres_liquidity_locker::Error::<Runtime>::InvalidPercentage
        );
    })]);
}

#[test]
#[should_panic(expected = "Pool does not exist")]
fn lock_liquidity_pool_does_not_exist() {
    preset_initial(vec![Rc::new(|_dex_id, _gt, _bp| {
        let _ = ceres_liquidity_locker::Pallet::<Runtime>::lock_liquidity(
            Origin::signed(ALICE()),
            GoldenTicket.into(),
            BlackPepper.into(),
            frame_system::Pallet::<Runtime>::block_number(),
            balance!(0.5),
            true,
        );
    })]);
}

#[test]
#[should_panic(expected = "User is not pool provider")]
fn lock_liquidity_user_is_not_pool_provider() {
    preset_initial(vec![Rc::new(|_dex_id, _gt, _bp| {
        let _ = ceres_liquidity_locker::Pallet::<Runtime>::lock_liquidity(
            Origin::signed(ALICE()),
            GoldenTicket.into(),
            CERES_ASSET_ID.into(),
            frame_system::Pallet::<Runtime>::block_number(),
            balance!(0.5),
            true,
        );
    })]);
}

#[test]
fn lock_liquidity_insufficient_liquidity_to_lock() {
    preset_initial(vec![Rc::new(|dex_id, _gt, _bp| {
        assert_ok!(pool_xyk::Pallet::<Runtime>::deposit_liquidity(
            Origin::signed(ALICE()),
            dex_id,
            GoldenTicket.into(),
            CERES_ASSET_ID.into(),
            balance!(360000),
            balance!(144000),
            balance!(360000),
            balance!(144000),
        ));

        // Lock 50% of LP tokens
        assert_ok!(ceres_liquidity_locker::Pallet::<Runtime>::lock_liquidity(
            Origin::signed(ALICE()),
            GoldenTicket.into(),
            CERES_ASSET_ID.into(),
            frame_system::Pallet::<Runtime>::block_number() + 5,
            balance!(0.5),
            true
        ));

        // Lock 30% of LP tokens
        assert_ok!(ceres_liquidity_locker::Pallet::<Runtime>::lock_liquidity(
            Origin::signed(ALICE()),
            GoldenTicket.into(),
            CERES_ASSET_ID.into(),
            frame_system::Pallet::<Runtime>::block_number() + 5,
            balance!(0.3),
            true
        ));

        // Try to lock 30% of LP tokens
        assert_err!(
            ceres_liquidity_locker::Pallet::<Runtime>::lock_liquidity(
                Origin::signed(ALICE()),
                GoldenTicket.into(),
                CERES_ASSET_ID.into(),
                frame_system::Pallet::<Runtime>::block_number() + 5,
                balance!(0.3),
                true
            ),
            ceres_liquidity_locker::Error::<Runtime>::InsufficientLiquidityToLock
        );
    })]);
}
