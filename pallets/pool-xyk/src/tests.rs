use crate::mock::*;
use common::{hash, prelude::SwapAmount, ToFeeAccount, ToTechUnitFromDEXAndTradingPair};
use frame_support::{assert_noop, assert_ok};
use permissions::{Scope, MINT};

type TechAssetIdOf<T> = <T as technical::Trait>::TechAssetId;

macro_rules! preset01(
($test: expr) => ({
    let mut ext = ExtBuilder::default().build();
    let dex_id = 220;
    let gt: crate::mock::AssetId = GoldenTicket.into();
    let bp: crate::mock::AssetId = BlackPepper.into();
    let tpair = common::TradingPair::<TechAssetIdOf<Testtime>> {
        base_asset_id: GoldenTicket.into(),
        target_asset_id: BlackPepper.into(),
    };
    let tech_acc_id =
        <Testtime as technical::Trait>::TechAccountId::to_tech_unit_from_dex_and_trading_pair(
            dex_id.clone(),
            tpair,
        );
    let fee_acc = tech_acc_id.clone().to_fee_account().unwrap();
    let repr: AccountId =
        technical::Module::<Testtime>::tech_account_id_to_account_id(&tech_acc_id).unwrap();
    let fee_repr: AccountId =
        technical::Module::<Testtime>::tech_account_id_to_account_id(&fee_acc).unwrap();
    ext.execute_with(|| {
        assert_ok!(technical::Module::<Testtime>::register_tech_account_id(
            tech_acc_id.clone()
        ));
        assert_ok!(technical::Module::<Testtime>::register_tech_account_id(
            fee_acc.clone()
        ));
        assert_ok!(assets::Module::<Testtime>::register(
            Origin::signed(ALICE()),
            GoldenTicket.into()
        ));
        assert_ok!(assets::Module::<Testtime>::register(
            Origin::signed(repr.clone()),
            BlackPepper.into()
        ));
        assert_ok!(dex_manager::Module::<Testtime>::initialize_dex(
            Origin::signed(BOB()),
            dex_id.clone(),
            GoldenTicket.into(),
            BOB(),
            None,
            None
        ));
        assert_ok!(trading_pair::Module::<Testtime>::register(
            Origin::signed(BOB()),
            dex_id.clone(),
            GoldenTicket.into(),
            BlackPepper.into()
        ));
        assert_ok!(assets::Module::<Testtime>::mint(
            &gt,
            &ALICE(),
            &ALICE(),
            900_000u32.into()
        ));
        assert_ok!(assets::Module::<Testtime>::mint(
            &bp,
            &repr.clone(),
            &repr.clone(),
            900_000u32.into()
        ));
        assert_ok!(
            permissions::Module::<Testtime>::grant_permission_with_scope(
                ALICE(),
                repr.clone(),
                MINT,
                Scope::Limited(hash(&gt))
            )
        );
        assert_ok!(assets::Module::<Testtime>::mint(
            &gt,
            &repr.clone(),
            &repr.clone(),
            1230_000u32.into()
        ));
        assert_eq!(
            Into::<u32>::into(assets::Module::<Testtime>::free_balance(&gt, &ALICE()).unwrap()),
            900_000u32
        );
        assert_eq!(
            Into::<u32>::into(assets::Module::<Testtime>::free_balance(&bp, &ALICE()).unwrap()),
            2000_000u32
        );
        assert_eq!(
            Into::<u32>::into(
                assets::Module::<Testtime>::free_balance(&gt, &repr.clone()).unwrap()
            ),
            1230_000u32
        );
        assert_eq!(
            Into::<u32>::into(
                assets::Module::<Testtime>::free_balance(&bp, &repr.clone()).unwrap()
            ),
            900_000u32
        );
        assert_eq!(
            Into::<u32>::into(
                assets::Module::<Testtime>::free_balance(&gt, &fee_repr.clone()).unwrap()
            ),
            0_u32
        );
        $test(dex_id, gt, bp, tpair, tech_acc_id.clone(), fee_acc.clone(), repr, fee_repr);
    });
}));

#[test]
#[rustfmt::skip]
fn swap_pair_premintliq_desired_output() {
    preset01!(|dex_id, gt, bp, _, _, _, repr: AccountId, fee_repr: AccountId| {
        assert_ok!(crate::Module::<Testtime>::swap_pair(
            Origin::signed(ALICE()),
            ALICE(),
            dex_id,
            GoldenTicket.into(),
            BlackPepper.into(),
            SwapAmount::WithDesiredOutput {
                desired_amount_out: 33_000u32.into(),
                max_amount_in: 99999999_u32.into(),
            }
        ));
        assert_eq!(
            Into::<u32>::into(assets::Module::<Testtime>::free_balance(&gt, &ALICE()).unwrap()),
            854_869u32
        );
        assert_eq!(
            Into::<u32>::into(assets::Module::<Testtime>::free_balance(&bp, &ALICE()).unwrap()),
            2033_000u32
        );
        assert_eq!(
            Into::<u32>::into(
                assets::Module::<Testtime>::free_balance(&gt, &repr.clone()).unwrap()
            ),
            1275_100u32
        );
        assert_eq!(
            Into::<u32>::into(
                assets::Module::<Testtime>::free_balance(&bp, &repr.clone()).unwrap()
            ),
            867_000u32
        );
        assert_eq!(
            Into::<u32>::into(
                assets::Module::<Testtime>::free_balance(&gt, &fee_repr.clone()).unwrap()
            ),
            30_u32
        );
    });
}

#[test]
#[rustfmt::skip]
fn swap_pair_premintliq_desired_input() {
    preset01!(|dex_id, gt, bp, _, _, _, repr: AccountId, fee_repr: AccountId| {
        assert_ok!(crate::Module::<Testtime>::swap_pair(
            Origin::signed(ALICE()),
            ALICE(),
            dex_id,
            GoldenTicket.into(),
            BlackPepper.into(),
            SwapAmount::WithDesiredInput {
                desired_amount_in: 33_000u32.into(),
                min_amount_out: 0_u32.into(),
            }
        ));
        assert_eq!(
            Into::<u32>::into(assets::Module::<Testtime>::free_balance(&gt, &ALICE()).unwrap()),
            866_970u32
        );
        assert_eq!(
            Into::<u32>::into(assets::Module::<Testtime>::free_balance(&bp, &ALICE()).unwrap()),
            2024_146u32
        );
        assert_eq!(
            Into::<u32>::into(
                assets::Module::<Testtime>::free_balance(&gt, &repr.clone()).unwrap()
            ),
            1263_000u32
        );
        assert_eq!(
            Into::<u32>::into(
                assets::Module::<Testtime>::free_balance(&bp, &repr.clone()).unwrap()
            ),
            875_853u32
        );
        assert_eq!(
            Into::<u32>::into(
                assets::Module::<Testtime>::free_balance(&gt, &fee_repr.clone()).unwrap()
            ),
            30_u32
        );
    });
}

#[test]
#[rustfmt::skip]
fn swap_pair_invalid_dex_id() {
    preset01!(|_, _, _, _, _, _, _, _| {
        assert_noop!(
            crate::Module::<Testtime>::swap_pair(
                Origin::signed(ALICE()),
                ALICE(),
                380,
                GoldenTicket.into(),
                BlackPepper.into(),
                SwapAmount::WithDesiredOutput {
                    desired_amount_out: 33_000u32.into(),
                    max_amount_in: 99999999_u32.into(),
                }
            ),
            technical::Error::<Testtime>::TechAccountIdIsNotRegistered
        );
    });
}

#[test]
#[rustfmt::skip]
fn swap_pair_different_asset_pair() {
    preset01!(|dex_id, _, _, _, _, _, _, _| {
        assert_noop!(
            crate::Module::<Testtime>::swap_pair(
                Origin::signed(ALICE()),
                ALICE(),
                dex_id,
                GoldenTicket.into(),
                RedPepper.into(),
                SwapAmount::WithDesiredOutput {
                    desired_amount_out: 33_000u32.into(),
                    max_amount_in: 99999999_u32.into(),
                }
            ),
            technical::Error::<Testtime>::TechAccountIdIsNotRegistered
        );
    });
}

#[test]
#[rustfmt::skip]
fn swap_pair_large_swap_fail_with_out_of_bounds() {
    preset01!(|dex_id, _, _, _, _, _, _, _| {
        assert_noop!(
            crate::Module::<Testtime>::swap_pair(
                Origin::signed(ALICE()),
                ALICE(),
                dex_id,
                GoldenTicket.into(),
                BlackPepper.into(),
                SwapAmount::WithDesiredOutput {
                    desired_amount_out: 99999_000u32.into(),
                    max_amount_in: 99999999_u32.into(),
                }
            ),
            crate::Error::<Testtime>::CalculatedValueIsOutOfDesiredBounds
        );
    });
}

#[test]
#[rustfmt::skip]
fn swap_pair_large_swap_fail_with_source_balance_not_large_enouth() {
    preset01!(|dex_id, _, _, _, _, _, _, _| {
        assert_noop!(
            crate::Module::<Testtime>::swap_pair(
                Origin::signed(ALICE()),
                ALICE(),
                dex_id,
                GoldenTicket.into(),
                BlackPepper.into(),
                SwapAmount::WithDesiredOutput {
                    desired_amount_out: 999999u32.into(),
                    max_amount_in: 999999999u32.into(),
                }
            ),
            crate::Error::<Testtime>::SourceBalanceIsNotLargeEnough
        );
    });
}

#[test]
#[rustfmt::skip]
fn swap_pair_swap_fail_with_target_balance_not_large_enoth() {
    preset01!(|dex_id, gt, _, _, _, _, _, _| {
        assert_ok!(assets::Module::<Testtime>::mint(
            &gt,
            &ALICE(),
            &ALICE(),
            99999_000u32.into()
        ));
        assert_noop!(
            crate::Module::<Testtime>::swap_pair(
                Origin::signed(ALICE()),
                ALICE(),
                dex_id,
                GoldenTicket.into(),
                BlackPepper.into(),
                SwapAmount::WithDesiredOutput {
                    desired_amount_out: 9999_000u32.into(),
                    max_amount_in: 999999999u32.into(),
                }
            ),
            crate::Error::<Testtime>::TargetBalanceIsNotLargeEnough
        );
    });
}

#[test]
#[rustfmt::skip]
fn swap_pair_swap_fail_with_invalid_balance() {
    preset01!(|dex_id, _, _, _, _, _, _, _| {
        assert_noop!(
            crate::Module::<Testtime>::swap_pair(
                Origin::signed(BOB()),
                BOB(),
                dex_id,
                GoldenTicket.into(),
                BlackPepper.into(),
                SwapAmount::WithDesiredOutput {
                    desired_amount_out: 33_000u32.into(),
                    max_amount_in: 999999999u32.into(),
                }
            ),
            crate::Error::<Testtime>::AccountBalanceIsInvalid
        );
    });
}
