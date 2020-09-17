use crate::mock::*;
use frame_support::assert_ok;

use PolySwapActionExample::*;

#[test]
fn repr_back_map() {
    let mut ext = ExtBuilder::default().build();
    let dex = 10;
    let t01 = crate::Module::<Testtime>::tech_acc_id_from_primitive(common::TechAccountId::Pure(
        dex,
        LiquidityKeeper(TradingPair {
            base_asset_id: RedPepper,
            target_asset_id: BlackPepper,
        }),
    ));
    ext.execute_with(|| {
        assert_ok!(Technical::register_tech_account_id(t01.clone()));
        assert_eq!(
            crate::TechAccountIdReprCompat::<
                Testtime,
                <Testtime as crate::Trait>::TechAccountIdPrimitive,
            >::from(AccountId::from(t01.clone())),
            t01.clone()
        );
    });
}

#[test]
fn generic_pair_swap_simple() {
    let mut ext = ExtBuilder::default().build();
    let dex = 10;
    let t01 = crate::Module::<Testtime>::tech_acc_id_from_primitive(common::TechAccountId::Pure(
        dex,
        LiquidityKeeper(TradingPair {
            base_asset_id: RedPepper,
            target_asset_id: BlackPepper,
        }),
    ));
    let repr: AccountId = t01.clone().into();
    let a01 = RedPepper;
    let a02 = BlackPepper;
    let s01 = GenericPair(GenericPairSwapActionExample {
        give_minted: false,
        give_asset: a01,
        give_amount: 330_000u32.into(),
        take_burn: false,
        take_asset: a02,
        take_amount: 1000_000u32.into(),
        take_account: t01.clone(),
    });
    ext.execute_with(|| {
        assert_ok!(assets::Module::<Testtime>::register(
            Origin::signed(get_alice()),
            RedPepper
        ));
        assert_ok!(assets::Module::<Testtime>::register(
            Origin::signed(repr.clone()),
            BlackPepper
        ));
        assert_ok!(assets::Module::<Testtime>::mint(
            &RedPepper,
            &get_alice(),
            9000_000u32.into()
        ));
        assert_ok!(assets::Module::<Testtime>::mint(
            &BlackPepper,
            &repr.clone(),
            9000_000u32.into()
        ));
        assert_ok!(Technical::register_tech_account_id(t01));
        assert_eq!(
            assets::Module::<Testtime>::free_balance(&a01, &get_alice()).unwrap(),
            9099000u32.into()
        );
        assert_eq!(
            assets::Module::<Testtime>::free_balance(&a02, &get_alice()).unwrap(),
            2000000u32.into()
        );
        assert_eq!(
            assets::Module::<Testtime>::free_balance(&a01, &repr).unwrap(),
            0u32.into()
        );
        assert_eq!(
            assets::Module::<Testtime>::free_balance(&a02, &repr).unwrap(),
            9000000u32.into()
        );
        assert_ok!(Technical::create_swap(Origin::signed(get_alice()), s01));
        assert_eq!(
            assets::Module::<Testtime>::free_balance(&a01, &get_alice()).unwrap(),
            8769000u32.into()
        );
        assert_eq!(
            assets::Module::<Testtime>::free_balance(&a02, &get_alice()).unwrap(),
            3000000u32.into()
        );
        assert_eq!(
            assets::Module::<Testtime>::free_balance(&a02, &repr).unwrap(),
            8000000u32.into()
        );
        assert_eq!(
            assets::Module::<Testtime>::free_balance(&a01, &repr).unwrap(),
            330000u32.into()
        );
    });
}
