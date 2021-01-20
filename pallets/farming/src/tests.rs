use crate::mock::*;
use crate::FarmId;
use common::{fixed, prelude::SwapAmount, AssetSymbol, ToFeeAccount, DOT, PSWAP, XOR};
use frame_support::{assert_noop, assert_ok};

impl crate::Module<Testtime> {
    fn run_to_block(n: u64) {
        while System::block_number() < n {
            //crate::Module::<Testtime>::on_finalize(System::block_number());
            System::set_block_number(System::block_number() + 1);
            crate::Module::<Testtime>::perform_per_block_update(System::block_number());
        }
    }

    fn preset01(
        tests: Vec<
            fn(
                crate::mock::DEXId,
                AssetId,
                AssetId,
                common::TradingPair<crate::mock::TechAssetId>,
                crate::mock::TechAccountId,
                crate::mock::TechAccountId,
                AccountId,
                AccountId,
                AssetId,
                FarmId,
            ) -> (),
        >,
    ) {
        let mut ext = ExtBuilder::default().build();
        let dex_id = 220;
        let gt: crate::mock::AssetId = XOR;
        let bp: crate::mock::AssetId = DOT;

        ext.execute_with(|| {
            assert_ok!(assets::Module::<Testtime>::register_asset_id(
                ALICE(),
                XOR,
                AssetSymbol(b"XOR".to_vec()),
                18
            ));

            assert_ok!(dex_manager::Module::<Testtime>::initialize_dex(
                Origin::signed(BOB()),
                dex_id.clone(),
                XOR,
                BOB(),
                None,
                None
            ));

            assert_ok!(trading_pair::Module::<Testtime>::register(
                Origin::signed(BOB()),
                dex_id.clone(),
                XOR,
                DOT
            ));

            assert_ok!(pool_xyk::Module::<Testtime>::initialize_pool(
                Origin::signed(BOB()),
                dex_id.clone(),
                XOR,
                DOT,
            ));

            let (tpair, tech_acc_id) =
                pool_xyk::Module::<Testtime>::tech_account_from_dex_and_asset_pair(
                    dex_id.clone(),
                    XOR,
                    DOT,
                )
                .unwrap();

            let fee_acc = tech_acc_id.clone().to_fee_account().unwrap();
            let repr: AccountId =
                technical::Module::<Testtime>::tech_account_id_to_account_id(&tech_acc_id).unwrap();
            let fee_repr: AccountId =
                technical::Module::<Testtime>::tech_account_id_to_account_id(&fee_acc).unwrap();
            let mark_asset =
                pool_xyk::Module::<Testtime>::get_marking_asset_repr(&tech_acc_id).unwrap();

            assert_ok!(assets::Module::<Testtime>::register_asset_id(
                ALICE(),
                DOT,
                AssetSymbol(b"DOT".to_vec()),
                18
            ));

            assert_ok!(assets::Module::<Testtime>::mint_to(
                &gt,
                &ALICE(),
                &ALICE(),
                900_000u32.into()
            ));

            assert_ok!(assets::Module::<Testtime>::mint_to(
                &gt,
                &ALICE(),
                &BOB(),
                900_000u32.into()
            ));

            assert_eq!(
                assets::Module::<Testtime>::free_balance(&gt, &ALICE()).unwrap(),
                fixed!(900000),
            );
            assert_eq!(
                assets::Module::<Testtime>::free_balance(&bp, &ALICE()).unwrap(),
                fixed!(2000000),
            );
            assert_eq!(
                assets::Module::<Testtime>::free_balance(&gt, &repr.clone()).unwrap(),
                fixed!(0),
            );

            assert_eq!(
                assets::Module::<Testtime>::free_balance(&bp, &repr.clone()).unwrap(),
                fixed!(0)
            );
            assert_eq!(
                assets::Module::<Testtime>::free_balance(&gt, &fee_repr.clone()).unwrap(),
                fixed!(0)
            );

            let farm_id = crate::Module::<Testtime>::create(Origin::signed(ALICE()), XOR, PSWAP)
                .unwrap()
                .unwrap();

            for test in &tests {
                test(
                    dex_id.clone(),
                    gt.clone(),
                    bp.clone(),
                    tpair.clone(),
                    tech_acc_id.clone(),
                    fee_acc.clone(),
                    repr.clone(),
                    fee_repr.clone(),
                    mark_asset.clone(),
                    farm_id.clone(),
                );
            }
        });
    }

    fn preset02(
        tests: Vec<
            fn(
                crate::mock::DEXId,
                AssetId,
                AssetId,
                common::TradingPair<crate::mock::TechAssetId>,
                crate::mock::TechAccountId,
                crate::mock::TechAccountId,
                AccountId,
                AccountId,
                AssetId,
                FarmId,
            ) -> (),
        >,
    ) {
        let mut new_tests: Vec<
            fn(
                crate::mock::DEXId,
                AssetId,
                AssetId,
                common::TradingPair<crate::mock::TechAssetId>,
                crate::mock::TechAccountId,
                crate::mock::TechAccountId,
                AccountId,
                AccountId,
                AssetId,
                FarmId,
            ) -> (),
        > = vec![
            |dex_id, _, _, _, _, _, _, _, _mark_asset_id: AssetId, _farm_id: FarmId| {
                assert_ok!(pool_xyk::Module::<Testtime>::deposit_liquidity(
                    Origin::signed(ALICE()),
                    dex_id,
                    XOR,
                    DOT,
                    360_000u32.into(),
                    144_000u32.into(),
                    360_000u32.into(),
                    144_000u32.into(),
                ));

                assert_ok!(pool_xyk::Module::<Testtime>::deposit_liquidity(
                    Origin::signed(BOB()),
                    dex_id,
                    XOR,
                    DOT,
                    360_000u32.into(),
                    144_000u32.into(),
                    360_000u32.into(),
                    144_000u32.into(),
                ));
            },
        ];
        let mut tests_to_add = tests.clone();
        new_tests.append(&mut tests_to_add);
        crate::Module::<Testtime>::preset01(new_tests);
    }
}

#[test]
fn one_farmer_working_with_farm_cascade() {
    crate::Module::<Testtime>::preset02(vec![
        |dex_id,
         _gt,
         _bp,
         _,
         _,
         _,
         _repr: AccountId,
         _fee_repr: AccountId,
         mark_asset: AssetId,
         farm_id: FarmId| {
            crate::Module::<Testtime>::run_to_block(2000);

            assert_ok!(crate::Module::<Testtime>::lock_to_farm(
                Origin::signed(ALICE()),
                dex_id,
                farm_id,
                mark_asset,
                10_000u32.into(),
            ));

            crate::Module::<Testtime>::run_to_block(3000);
            let a = Origin::signed(ALICE());
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(a, farm_id).unwrap(),
                Some(103975u64)
            );

            crate::Module::<Testtime>::run_to_block(5000);

            assert_ok!(pool_xyk::Module::<Testtime>::swap_pair(
                Origin::signed(ALICE()),
                ALICE(),
                dex_id,
                XOR,
                DOT,
                SwapAmount::WithDesiredOutput {
                    desired_amount_out: 33_000u32.into(),
                    max_amount_in: 99999999_u32.into(),
                }
            ));

            crate::Module::<Testtime>::run_to_block(6000);
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(ALICE()), farm_id,)
                    .unwrap(),
                Some(103978u64)
            );

            assert_ok!(crate::Module::<Testtime>::unlock_from_farm(
                Origin::signed(ALICE()),
                dex_id,
                farm_id,
                Some(mark_asset),
                Some(1000u32.into()),
            ));

            crate::Module::<Testtime>::run_to_block(20000);

            assert_ok!(crate::Module::<Testtime>::lock_to_farm(
                Origin::signed(ALICE()),
                dex_id,
                farm_id,
                mark_asset,
                1000u32.into(),
            ));

            crate::Module::<Testtime>::run_to_block(30000);

            assert_ok!(crate::Module::<Testtime>::claim(
                Origin::signed(ALICE()),
                farm_id,
                Some(10u32.into()),
            ));

            crate::Module::<Testtime>::run_to_block(35000);

            assert_ok!(crate::Module::<Testtime>::lock_to_farm(
                Origin::signed(ALICE()),
                dex_id,
                farm_id,
                mark_asset,
                1000u32.into(),
            ));

            crate::Module::<Testtime>::run_to_block(50000);

            assert_ok!(crate::Module::<Testtime>::claim(
                Origin::signed(ALICE()),
                farm_id,
                None,
            ));

            assert_noop!(
                crate::Module::<Testtime>::claim(
                    Origin::signed(ALICE()),
                    farm_id,
                    Some(1u32.into()),
                ),
                crate::Error::<Testtime>::NothingToClaim
            );

            crate::Module::<Testtime>::run_to_block(60000);

            assert_noop!(
                crate::Module::<Testtime>::claim(
                    Origin::signed(ALICE()),
                    farm_id,
                    Some(999u32.into()),
                ),
                crate::Error::<Testtime>::AmountIsOutOfAvailableValue
            );
        },
    ]);
}

#[test]
fn two_farmers_working_with_farm_cascade() {
    crate::Module::<Testtime>::preset02(vec![
        |dex_id,
         _gt,
         _bp,
         _,
         _,
         _,
         _repr: AccountId,
         _fee_repr: AccountId,
         mark_asset: AssetId,
         farm_id: FarmId| {
            crate::Module::<Testtime>::run_to_block(2000);

            assert_ok!(crate::Module::<Testtime>::lock_to_farm(
                Origin::signed(ALICE()),
                dex_id,
                farm_id,
                mark_asset,
                10_000u32.into(),
            ));

            assert_ok!(crate::Module::<Testtime>::lock_to_farm(
                Origin::signed(BOB()),
                dex_id,
                farm_id,
                mark_asset,
                10_000u32.into(),
            ));

            crate::Module::<Testtime>::run_to_block(3000);
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(ALICE()), farm_id,)
                    .unwrap(),
                Some(51988u64)
            );

            crate::Module::<Testtime>::run_to_block(3000);
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(BOB()), farm_id,).unwrap(),
                Some(51988u64)
            );

            crate::Module::<Testtime>::run_to_block(5000);

            assert_ok!(pool_xyk::Module::<Testtime>::swap_pair(
                Origin::signed(ALICE()),
                ALICE(),
                dex_id,
                XOR,
                DOT,
                SwapAmount::WithDesiredOutput {
                    desired_amount_out: 33_000u32.into(),
                    max_amount_in: 99999999_u32.into(),
                }
            ));

            assert_ok!(pool_xyk::Module::<Testtime>::swap_pair(
                Origin::signed(ALICE()),
                BOB(),
                dex_id,
                XOR,
                DOT,
                SwapAmount::WithDesiredOutput {
                    desired_amount_out: 33_000u32.into(),
                    max_amount_in: 99999999_u32.into(),
                }
            ));

            crate::Module::<Testtime>::run_to_block(6000);

            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(ALICE()), farm_id,)
                    .unwrap(),
                Some(51989u64)
            );

            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(BOB()), farm_id,).unwrap(),
                Some(51989u64)
            );

            assert_ok!(crate::Module::<Testtime>::unlock_from_farm(
                Origin::signed(ALICE()),
                dex_id,
                farm_id,
                Some(mark_asset),
                Some(1000u32.into()),
            ));

            assert_ok!(crate::Module::<Testtime>::unlock_from_farm(
                Origin::signed(BOB()),
                dex_id,
                farm_id,
                Some(mark_asset),
                Some(1000u32.into()),
            ));

            crate::Module::<Testtime>::run_to_block(20000);
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(ALICE()), farm_id,)
                    .unwrap(),
                Some(52452u64)
            );
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(BOB()), farm_id,).unwrap(),
                Some(52452u64)
            );

            crate::Module::<Testtime>::run_to_block(21000);
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(ALICE()), farm_id,)
                    .unwrap(),
                Some(52488u64)
            );
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(BOB()), farm_id,).unwrap(),
                Some(52488u64)
            );

            crate::Module::<Testtime>::run_to_block(22000);
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(ALICE()), farm_id,)
                    .unwrap(),
                Some(52505u64)
            );
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(BOB()), farm_id,).unwrap(),
                Some(52505u64)
            );

            crate::Module::<Testtime>::run_to_block(23000);
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(ALICE()), farm_id,)
                    .unwrap(),
                Some(52531u64)
            );
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(BOB()), farm_id,).unwrap(),
                Some(52531u64)
            );

            assert_ok!(crate::Module::<Testtime>::lock_to_farm(
                Origin::signed(ALICE()),
                dex_id,
                farm_id,
                mark_asset,
                1000u32.into(),
            ));

            assert_ok!(crate::Module::<Testtime>::lock_to_farm(
                Origin::signed(BOB()),
                dex_id,
                farm_id,
                mark_asset,
                1500u32.into(),
            ));

            crate::Module::<Testtime>::run_to_block(24000);
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(ALICE()), farm_id,)
                    .unwrap(),
                Some(52473u64)
            );
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(BOB()), farm_id,).unwrap(),
                Some(52645u64)
            );

            crate::Module::<Testtime>::run_to_block(25000);
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(ALICE()), farm_id,)
                    .unwrap(),
                Some(52415u64)
            );
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(BOB()), farm_id,).unwrap(),
                Some(52742u64)
            );

            crate::Module::<Testtime>::run_to_block(29000);
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(ALICE()), farm_id,)
                    .unwrap(),
                Some(52261u64)
            );
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(BOB()), farm_id,).unwrap(),
                Some(53082u64)
            );

            crate::Module::<Testtime>::run_to_block(30000);
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(ALICE()), farm_id,)
                    .unwrap(),
                Some(52246u64)
            );
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(BOB()), farm_id,).unwrap(),
                Some(53165u64)
            );

            assert_ok!(crate::Module::<Testtime>::claim(
                Origin::signed(ALICE()),
                farm_id,
                Some(10u32.into()),
            ));

            assert_ok!(crate::Module::<Testtime>::claim(
                Origin::signed(BOB()),
                farm_id,
                Some(40000u32.into()),
            ));

            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(ALICE()), farm_id,)
                    .unwrap(),
                Some(52236u64)
            );
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(BOB()), farm_id,).unwrap(),
                Some(13165u64)
            );

            crate::Module::<Testtime>::run_to_block(35000);

            assert_ok!(crate::Module::<Testtime>::lock_to_farm(
                Origin::signed(ALICE()),
                dex_id,
                farm_id,
                mark_asset,
                1000u32.into(),
            ));

            assert_ok!(crate::Module::<Testtime>::lock_to_farm(
                Origin::signed(BOB()),
                dex_id,
                farm_id,
                mark_asset,
                1000u32.into(),
            ));

            crate::Module::<Testtime>::run_to_block(50000);
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(ALICE()), farm_id,)
                    .unwrap(),
                Some(40717u64)
            );
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(BOB()), farm_id,).unwrap(),
                Some(25370u64)
            );

            assert_ok!(crate::Module::<Testtime>::claim(
                Origin::signed(ALICE()),
                farm_id,
                None,
            ));

            assert_ok!(crate::Module::<Testtime>::claim(
                Origin::signed(BOB()),
                farm_id,
                None,
            ));

            assert_noop!(
                crate::Module::<Testtime>::claim(
                    Origin::signed(ALICE()),
                    farm_id,
                    Some(1u32.into()),
                ),
                crate::Error::<Testtime>::NothingToClaim
            );

            assert_noop!(
                crate::Module::<Testtime>::claim(Origin::signed(BOB()), farm_id, Some(1u32.into()),),
                crate::Error::<Testtime>::NothingToClaim
            );

            crate::Module::<Testtime>::run_to_block(60000);

            assert_noop!(
                crate::Module::<Testtime>::claim(
                    Origin::signed(ALICE()),
                    farm_id,
                    Some(999u32.into()),
                ),
                crate::Error::<Testtime>::AmountIsOutOfAvailableValue
            );

            assert_noop!(
                crate::Module::<Testtime>::claim(
                    Origin::signed(BOB()),
                    farm_id,
                    Some(999u32.into()),
                ),
                crate::Error::<Testtime>::AmountIsOutOfAvailableValue
            );
        },
    ]);
}

#[test]
fn unlock_exact() {
    crate::Module::<Testtime>::preset02(vec![
        |dex_id, _gt, _bp, _, _, _, _, _, mark_asset: AssetId, farm_id: FarmId| {
            assert_ok!(crate::Module::<Testtime>::lock_to_farm(
                Origin::signed(ALICE()),
                dex_id,
                farm_id,
                mark_asset,
                10_000u32.into(),
            ));
            assert_ok!(crate::Module::<Testtime>::unlock_from_farm(
                Origin::signed(ALICE()),
                dex_id,
                farm_id,
                Some(mark_asset),
                Some(10_000u32.into()),
            ));
            assert_noop!(
                crate::Module::<Testtime>::unlock_from_farm(
                    Origin::signed(ALICE()),
                    dex_id,
                    farm_id,
                    Some(mark_asset),
                    Some(1u32.into()),
                ),
                crate::Error::<Testtime>::CalculationOrOperationWithFarmingStateIsFailed
            );
        },
    ]);
}

#[test]
fn unlock_more_than_exist_must_fail() {
    crate::Module::<Testtime>::preset02(vec![
        |dex_id, _gt, _bp, _, _, _, _, _, mark_asset: AssetId, farm_id: FarmId| {
            assert_ok!(crate::Module::<Testtime>::lock_to_farm(
                Origin::signed(ALICE()),
                dex_id,
                farm_id,
                mark_asset,
                10_000u32.into(),
            ));
            assert_noop!(
                crate::Module::<Testtime>::unlock_from_farm(
                    Origin::signed(ALICE()),
                    dex_id,
                    farm_id,
                    Some(mark_asset),
                    Some(10_001u32.into()),
                ),
                crate::Error::<Testtime>::CalculationOrOperationWithFarmingStateIsFailed
            );
        },
    ]);
}

#[test]
fn unlock_all_for_asset() {
    crate::Module::<Testtime>::preset02(vec![
        |dex_id, _gt, _bp, _, _, _, _, _, mark_asset: AssetId, farm_id: FarmId| {
            assert_ok!(crate::Module::<Testtime>::lock_to_farm(
                Origin::signed(ALICE()),
                dex_id,
                farm_id,
                mark_asset,
                10_000u32.into(),
            ));
            assert_ok!(crate::Module::<Testtime>::unlock_from_farm(
                Origin::signed(ALICE()),
                dex_id,
                farm_id,
                Some(mark_asset),
                None,
            ));
            assert_noop!(
                crate::Module::<Testtime>::unlock_from_farm(
                    Origin::signed(ALICE()),
                    dex_id,
                    farm_id,
                    Some(mark_asset),
                    Some(1u32.into()),
                ),
                crate::Error::<Testtime>::CalculationOrOperationWithFarmingStateIsFailed
            );
        },
    ]);
}

#[test]
fn unlock_all_assets() {
    crate::Module::<Testtime>::preset02(vec![
        |dex_id, _gt, _bp, _, _, _, _, _, mark_asset: AssetId, farm_id: FarmId| {
            assert_ok!(crate::Module::<Testtime>::lock_to_farm(
                Origin::signed(ALICE()),
                dex_id,
                farm_id,
                mark_asset,
                10_000u32.into(),
            ));
            assert_ok!(crate::Module::<Testtime>::unlock_from_farm(
                Origin::signed(ALICE()),
                dex_id,
                farm_id,
                None,
                None,
            ));
            assert_noop!(
                crate::Module::<Testtime>::unlock_from_farm(
                    Origin::signed(ALICE()),
                    dex_id,
                    farm_id,
                    Some(mark_asset),
                    Some(1u32.into()),
                ),
                crate::Error::<Testtime>::CalculationOrOperationWithFarmingStateIsFailed
            );
        },
    ]);
}
