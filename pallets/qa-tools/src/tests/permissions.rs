use super::alice;
use common::{balance, AccountIdOf};
use frame_support::assert_err;
use frame_support::dispatch::RawOrigin;
use framenode_chain_spec::ext;
use framenode_runtime::qa_tools;
use framenode_runtime::Runtime;
use qa_tools::pallet_tools::price_tools::AssetPrices;
use qa_tools::InputAssetId;
use sp_runtime::DispatchError;

fn check_all_extrinsics_are_denied(origin: RawOrigin<AccountIdOf<Runtime>>) {
    assert_err!(
        qa_tools::Pallet::<Runtime>::order_book_create_and_fill_batch(
            origin.clone().into(),
            alice(),
            alice(),
            vec![],
        ),
        DispatchError::BadOrigin
    );
    assert_err!(
        qa_tools::Pallet::<Runtime>::order_book_fill_batch(
            origin.clone().into(),
            alice(),
            alice(),
            vec![],
        ),
        DispatchError::BadOrigin
    );
    assert_err!(
        qa_tools::Pallet::<Runtime>::xyk_initialize(origin.clone().into(), alice(), vec![],),
        DispatchError::BadOrigin
    );
    assert_err!(
        qa_tools::Pallet::<Runtime>::xst_initialize(origin.clone().into(), None, vec![], alice()),
        DispatchError::BadOrigin
    );
    assert_err!(
        qa_tools::Pallet::<Runtime>::mcbc_initialize(origin.clone().into(), None, vec![], None,),
        DispatchError::BadOrigin
    );
    assert_err!(
        qa_tools::Pallet::<Runtime>::price_tools_set_asset_price(
            origin.into(),
            AssetPrices {
                buy: balance!(1),
                sell: balance!(1),
            },
            InputAssetId::McbcReference,
        ),
        DispatchError::BadOrigin
    );
}

#[test]
fn should_deny_non_root_callers() {
    ext().execute_with(|| {
        check_all_extrinsics_are_denied(RawOrigin::Signed(alice()));
        check_all_extrinsics_are_denied(RawOrigin::None);
    })
}
