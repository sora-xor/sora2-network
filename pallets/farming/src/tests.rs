use crate::mock::*;
use crate::FarmId;
use common::{
    balance,
    prelude::{Balance, SwapAmount},
    AssetSymbol, ToFeeAccount, DOT, PSWAP, XOR,
};
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
        let dex_id = DEX_A_ID;
        let gt: crate::mock::AssetId = XOR;
        let bp: crate::mock::AssetId = DOT;

        ext.execute_with(|| {
            assert_ok!(assets::Module::<Testtime>::register_asset_id(
                ALICE(),
                XOR,
                AssetSymbol(b"XOR".to_vec()),
                18,
                Balance::from(0u32),
                true,
            ));

            assert_ok!(assets::Module::<Testtime>::register_asset_id(
                ALICE(),
                DOT,
                AssetSymbol(b"DOT".to_vec()),
                18,
                Balance::from(0u32),
                true,
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

            assert_ok!(assets::Module::<Testtime>::mint_to(
                &gt,
                &ALICE(),
                &ALICE(),
                balance!(900000)
            ));

            assert_ok!(assets::Module::<Testtime>::mint_to(
                &gt,
                &ALICE(),
                &BOB(),
                balance!(900000)
            ));

            assert_eq!(
                assets::Module::<Testtime>::free_balance(&gt, &ALICE()).unwrap(),
                balance!(900000),
            );
            assert_eq!(
                assets::Module::<Testtime>::free_balance(&bp, &ALICE()).unwrap(),
                balance!(2000000),
            );
            assert_eq!(
                assets::Module::<Testtime>::free_balance(&gt, &repr.clone()).unwrap(),
                0
            );

            assert_eq!(
                assets::Module::<Testtime>::free_balance(&bp, &repr.clone()).unwrap(),
                0
            );
            assert_eq!(
                assets::Module::<Testtime>::free_balance(&gt, &fee_repr.clone()).unwrap(),
                0
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
                    balance!(360000),
                    balance!(144000),
                    balance!(36000),
                    balance!(14400),
                ));

                assert_ok!(pool_xyk::Module::<Testtime>::deposit_liquidity(
                    Origin::signed(BOB()),
                    dex_id,
                    XOR,
                    DOT,
                    balance!(360000),
                    balance!(144000),
                    balance!(36000),
                    balance!(14400),
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
                balance!(10000),
            ));

            crate::Module::<Testtime>::run_to_block(3000);
            let a = Origin::signed(ALICE());
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(a, farm_id).unwrap(),
                103975138541464779121884
            );

            crate::Module::<Testtime>::run_to_block(5000);

            assert_ok!(pool_xyk::Module::<Testtime>::swap_pair(
                Origin::signed(ALICE()),
                ALICE(),
                dex_id,
                XOR,
                DOT,
                SwapAmount::WithDesiredOutput {
                    desired_amount_out: balance!(33000),
                    max_amount_in: balance!(99999999),
                }
            ));

            crate::Module::<Testtime>::run_to_block(6000);
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(ALICE()), farm_id,)
                    .unwrap(),
                103978267222392724771206
            );

            assert_ok!(crate::Module::<Testtime>::unlock_from_farm(
                Origin::signed(ALICE()),
                dex_id,
                farm_id,
                Some(mark_asset),
                Some(balance!(1000)),
            ));

            crate::Module::<Testtime>::run_to_block(20000);

            assert_ok!(crate::Module::<Testtime>::lock_to_farm(
                Origin::signed(ALICE()),
                dex_id,
                farm_id,
                mark_asset,
                balance!(1000),
            ));

            crate::Module::<Testtime>::run_to_block(30000);

            assert_ok!(crate::Module::<Testtime>::claim(
                Origin::signed(ALICE()),
                farm_id,
                Some(balance!(10)),
            ));

            crate::Module::<Testtime>::run_to_block(35000);

            assert_ok!(crate::Module::<Testtime>::lock_to_farm(
                Origin::signed(ALICE()),
                dex_id,
                farm_id,
                mark_asset,
                balance!(1000),
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
                    Some(balance!(1)),
                ),
                crate::Error::<Testtime>::NothingToClaim
            );

            crate::Module::<Testtime>::run_to_block(60000);

            assert_noop!(
                crate::Module::<Testtime>::claim(
                    Origin::signed(ALICE()),
                    farm_id,
                    Some(balance!(999)),
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
                balance!(10000),
            ));

            assert_ok!(crate::Module::<Testtime>::lock_to_farm(
                Origin::signed(BOB()),
                dex_id,
                farm_id,
                mark_asset,
                balance!(10000),
            ));

            crate::Module::<Testtime>::run_to_block(3000);
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(ALICE()), farm_id,)
                    .unwrap(),
                51987569270732389560942
            );

            crate::Module::<Testtime>::run_to_block(3000);
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(BOB()), farm_id,).unwrap(),
                51987569270732389560942
            );

            crate::Module::<Testtime>::run_to_block(5000);

            assert_ok!(pool_xyk::Module::<Testtime>::swap_pair(
                Origin::signed(ALICE()),
                ALICE(),
                dex_id,
                XOR,
                DOT,
                SwapAmount::WithDesiredOutput {
                    desired_amount_out: balance!(33000),
                    max_amount_in: balance!(99999999),
                }
            ));

            assert_ok!(pool_xyk::Module::<Testtime>::swap_pair(
                Origin::signed(ALICE()),
                BOB(),
                dex_id,
                XOR,
                DOT,
                SwapAmount::WithDesiredOutput {
                    desired_amount_out: balance!(33000),
                    max_amount_in: balance!(99999999),
                }
            ));

            crate::Module::<Testtime>::run_to_block(6000);

            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(ALICE()), farm_id,)
                    .unwrap(),
                51989133611196362385603
            );

            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(BOB()), farm_id,).unwrap(),
                51989133611196362385603
            );

            assert_ok!(crate::Module::<Testtime>::unlock_from_farm(
                Origin::signed(ALICE()),
                dex_id,
                farm_id,
                Some(mark_asset),
                Some(balance!(1000)),
            ));

            assert_ok!(crate::Module::<Testtime>::unlock_from_farm(
                Origin::signed(BOB()),
                dex_id,
                farm_id,
                Some(mark_asset),
                Some(balance!(1000)),
            ));

            crate::Module::<Testtime>::run_to_block(20000);
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(ALICE()), farm_id,)
                    .unwrap(),
                52451700702476025417273
            );
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(BOB()), farm_id,).unwrap(),
                52451700702476025417273
            );

            crate::Module::<Testtime>::run_to_block(21000);
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(ALICE()), farm_id,)
                    .unwrap(),
                52487858899033049591217
            );
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(BOB()), farm_id,).unwrap(),
                52487858899033049591217
            );

            crate::Module::<Testtime>::run_to_block(22000);
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(ALICE()), farm_id,)
                    .unwrap(),
                52505454948186526233939
            );
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(BOB()), farm_id,).unwrap(),
                52505454948186526233939
            );

            crate::Module::<Testtime>::run_to_block(23000);
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(ALICE()), farm_id,)
                    .unwrap(),
                52530526066639891663368
            );
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(BOB()), farm_id,).unwrap(),
                52530526066639891663368
            );

            assert_ok!(crate::Module::<Testtime>::lock_to_farm(
                Origin::signed(ALICE()),
                dex_id,
                farm_id,
                mark_asset,
                balance!(1000),
            ));

            assert_ok!(crate::Module::<Testtime>::lock_to_farm(
                Origin::signed(BOB()),
                dex_id,
                farm_id,
                mark_asset,
                balance!(1500),
            ));

            crate::Module::<Testtime>::run_to_block(24000);
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(ALICE()), farm_id,)
                    .unwrap(),
                52473261848944591846985
            );
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(BOB()), farm_id,).unwrap(),
                52645211368930210402144
            );

            crate::Module::<Testtime>::run_to_block(25000);
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(ALICE()), farm_id,)
                    .unwrap(),
                52415419296342604706870
            );
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(BOB()), farm_id,).unwrap(),
                52742419572690985932385
            );

            crate::Module::<Testtime>::run_to_block(29000);
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(ALICE()), farm_id,)
                    .unwrap(),
                52261271263478340093362
            );
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(BOB()), farm_id,).unwrap(),
                53081592068137254514308
            );

            crate::Module::<Testtime>::run_to_block(30000);
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(ALICE()), farm_id,)
                    .unwrap(),
                52245597363990619535308
            );
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(BOB()), farm_id,).unwrap(),
                53165260016667738554569
            );

            assert_ok!(crate::Module::<Testtime>::claim(
                Origin::signed(ALICE()),
                farm_id,
                Some(balance!(10)),
            ));

            assert_ok!(crate::Module::<Testtime>::claim(
                Origin::signed(BOB()),
                farm_id,
                Some(balance!(40000)),
            ));

            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(ALICE()), farm_id,)
                    .unwrap(),
                52235597363990619534674
            );
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(BOB()), farm_id,).unwrap(),
                13165260016667738554690
            );

            crate::Module::<Testtime>::run_to_block(35000);

            assert_ok!(crate::Module::<Testtime>::lock_to_farm(
                Origin::signed(ALICE()),
                dex_id,
                farm_id,
                mark_asset,
                balance!(1000),
            ));

            assert_ok!(crate::Module::<Testtime>::lock_to_farm(
                Origin::signed(BOB()),
                dex_id,
                farm_id,
                mark_asset,
                balance!(1000),
            ));

            crate::Module::<Testtime>::run_to_block(50000);
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(ALICE()), farm_id,)
                    .unwrap(),
                40716972520757701551844
            );
            assert_eq!(
                crate::Module::<Testtime>::discover_claim(Origin::signed(BOB()), farm_id,).unwrap(),
                25369990980909847626935
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
                    Some(balance!(1)),
                ),
                crate::Error::<Testtime>::NothingToClaim
            );

            assert_noop!(
                crate::Module::<Testtime>::claim(Origin::signed(BOB()), farm_id, Some(balance!(1))),
                crate::Error::<Testtime>::NothingToClaim
            );

            crate::Module::<Testtime>::run_to_block(60000);

            assert_noop!(
                crate::Module::<Testtime>::claim(
                    Origin::signed(ALICE()),
                    farm_id,
                    Some(balance!(999)),
                ),
                crate::Error::<Testtime>::AmountIsOutOfAvailableValue
            );

            assert_noop!(
                crate::Module::<Testtime>::claim(
                    Origin::signed(BOB()),
                    farm_id,
                    Some(balance!(999)),
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
                balance!(10000),
            ));
            assert_ok!(crate::Module::<Testtime>::unlock_from_farm(
                Origin::signed(ALICE()),
                dex_id,
                farm_id,
                Some(mark_asset),
                Some(balance!(10000)),
            ));
            assert_noop!(
                crate::Module::<Testtime>::unlock_from_farm(
                    Origin::signed(ALICE()),
                    dex_id,
                    farm_id,
                    Some(mark_asset),
                    Some(balance!(1)),
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
                    Some(balance!(1)),
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
                    Some(balance!(1)),
                ),
                crate::Error::<Testtime>::CalculationOrOperationWithFarmingStateIsFailed
            );
        },
    ]);
}
