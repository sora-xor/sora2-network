use common::{
    balance, AssetName, AssetSymbol, Balance, LiquiditySource, LiquiditySourceType, ToFeeAccount,
    DEFAULT_BALANCE_PRECISION,
};
use frame_support::{assert_noop, assert_ok};

use sp_std::rc::Rc;
use crate::mock::*;

pub struct Module<T: Config>(ceres_liquidity_locker::Module<T>);
pub trait Config: ceres_liquidity_locker::Config {}

type PresetFunction<'a> = Rc<
    dyn Fn(
        DEXId,
        AssetId,
        AssetId,
    ) -> ()
    + 'a,
>;

impl<'a> Module<Runtime> {
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

            let (tpair, tech_acc_id) =
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
                balance!(2000)
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
                test(
                    dex_id.clone(),
                    gt.clone(),
                    ceres.clone(),
                );
            }
        });
    }
}
