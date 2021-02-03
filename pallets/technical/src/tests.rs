use crate::mock::*;
use common::{prelude::Balance, AssetSymbol};
use frame_support::assert_ok;
use PolySwapActionExample::*;

#[test]
fn should_register_technical_account() {
    let mut ext = ExtBuilder::default().build();
    let tech_account_id = common::TechAccountId::Generic("Test123".into(), "Some data".into());
    let t01 = crate::Module::<Testtime>::tech_account_id_to_account_id(&tech_account_id).unwrap();

    ext.execute_with(|| {
        assert_ok!(Technical::register_tech_account_id(TechAccountId::Generic(
            "Test123".into(),
            "Some data".into()
        )));
        assert_eq!(
            crate::Module::<Testtime>::lookup_tech_account_id(&t01).unwrap(),
            tech_account_id
        );
    });
}

#[test]
fn generic_pair_swap_simple() {
    let mut ext = ExtBuilder::default().build();
    let dex = 10;
    let t01 = common::TechAccountId::Pure(
        dex,
        LiquidityKeeper(TradingPair {
            base_asset_id: common::mock::ComicAssetId::RedPepper.into(),
            target_asset_id: common::mock::ComicAssetId::BlackPepper.into(),
        }),
    );
    let repr: AccountId = Technical::tech_account_id_to_account_id(&t01).unwrap();
    let a01 = RedPepper();
    let a02 = BlackPepper();
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
        assert_ok!(assets::Module::<Testtime>::register_asset_id(
            get_alice(),
            RedPepper(),
            AssetSymbol(b"RP".to_vec()),
            18,
            Balance::from(0u32),
            true,
        ));
        assert_ok!(assets::Module::<Testtime>::register_asset_id(
            repr.clone(),
            BlackPepper(),
            AssetSymbol(b"BP".to_vec()),
            18,
            Balance::from(0u32),
            true,
        ));
        assert_ok!(assets::Module::<Testtime>::mint_to(
            &RedPepper(),
            &get_alice(),
            &get_alice(),
            9000_000u32.into()
        ));
        assert_ok!(assets::Module::<Testtime>::mint_to(
            &BlackPepper(),
            &repr,
            &repr,
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
