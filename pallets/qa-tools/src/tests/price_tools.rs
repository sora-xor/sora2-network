use super::QaToolsPallet;
use assets::AssetIdOf;
use common::{balance, PriceVariant, ETH, XOR};
use frame_support::{assert_err, assert_ok};
use framenode_chain_spec::ext;
use framenode_runtime::qa_tools;
use framenode_runtime::{Runtime, RuntimeOrigin};
use qa_tools::{pallet_tools, Error, InputAssetId};

use pallet_tools::price_tools::AssetPrices;

fn check_price_tools_set_price(asset_id: &InputAssetId<AssetIdOf<Runtime>>, prices: AssetPrices) {
    assert_ok!(QaToolsPallet::price_tools_set_asset_price(
        RuntimeOrigin::root(),
        prices.clone(),
        asset_id.clone()
    ));
    let asset_id = asset_id.clone().resolve::<Runtime>();
    assert_eq!(
        price_tools::Pallet::<Runtime>::get_average_price(&XOR, &asset_id, PriceVariant::Buy),
        Ok(prices.buy)
    );
    assert_eq!(
        price_tools::Pallet::<Runtime>::get_average_price(&XOR, &asset_id, PriceVariant::Sell),
        Ok(prices.sell)
    );
}

fn test_price_tools_set_asset_prices(asset_id: InputAssetId<AssetIdOf<Runtime>>) {
    ext().execute_with(|| {
        check_price_tools_set_price(
            &asset_id,
            AssetPrices {
                buy: balance!(1),
                sell: balance!(1),
            },
        );
        check_price_tools_set_price(
            &asset_id,
            AssetPrices {
                buy: balance!(2),
                sell: balance!(1),
            },
        );
        check_price_tools_set_price(
            &asset_id,
            AssetPrices {
                buy: balance!(365),
                sell: balance!(256),
            },
        );
        check_price_tools_set_price(
            &asset_id,
            AssetPrices {
                buy: balance!(1),
                sell: balance!(1),
            },
        );
    })
}

// todo: uncomment
// #[test]
// fn should_set_price_tools_mcbc_base_prices() {
//     test_price_tools_set_asset_prices(InputAssetId::<AssetIdOf<Runtime>>::McbcReference);
// }

#[test]
fn should_set_price_tools_xst_base_prices() {
    test_price_tools_set_asset_prices(InputAssetId::<AssetIdOf<Runtime>>::XstReference);
}

#[test]
fn should_set_price_tools_other_base_prices() {
    test_price_tools_set_asset_prices(InputAssetId::<AssetIdOf<Runtime>>::Other(ETH));
}

#[test]
fn should_price_tools_reject_incorrect_prices() {
    ext().execute_with(|| {
        // todo: uncomment
        // assert_err!(
        //     QaToolsPallet::price_tools_set_asset_price(
        //         RuntimeOrigin::root(),
        //         AssetPrices {
        //             buy: balance!(1),
        //             sell: balance!(1) + 1,
        //         },
        //         InputAssetId::<AssetIdOf<Runtime>>::McbcReference
        //     ),
        //     Error::<Runtime>::BuyLessThanSell
        // );
        assert_err!(
            QaToolsPallet::price_tools_set_asset_price(
                RuntimeOrigin::root(),
                AssetPrices {
                    buy: balance!(1),
                    sell: balance!(1) + 1,
                },
                InputAssetId::<AssetIdOf<Runtime>>::XstReference
            ),
            Error::<Runtime>::BuyLessThanSell
        );
        assert_err!(
            QaToolsPallet::price_tools_set_asset_price(
                RuntimeOrigin::root(),
                AssetPrices {
                    buy: balance!(1),
                    sell: balance!(1) + 1,
                },
                InputAssetId::<AssetIdOf<Runtime>>::Other(ETH)
            ),
            Error::<Runtime>::BuyLessThanSell
        );
    })
}
