use common::{AssetId32, Balance, PredefinedAssetId};
use ink_primitives::AccountId;
use scale::Encode;

/// It is a part of the runtime dispatchables API.
/// `Ink!` doesn't expose the real enum, so we need a partial definition matching our targets.
/// You should get or count index of the pallet, using `construct_runtime!`, it is zero based
#[derive(Encode)]
pub enum RuntimeCall {
    #[codec(index = 21)]
    Assets(AssetsCall),
}

/// It is a part of a pallet dispatchables API.
/// The indexes can be found in your pallet code's #[pallet::call] section and check #[pallet::call_index(x)] attribute of the call.
/// If these attributes are missing, use source-code order (0-based).
#[derive(Encode)]
pub enum AssetsCall {
    /// Transfer amount of asset from caller to another account.
    /// You may found list of callable extrinsic in `pallet_contracts::Config::CallFilter`
    #[codec(index = 1)]
    Transfer {
        asset_id: AssetId32<PredefinedAssetId>,
        to: AccountId,
        amount: Balance,
    },
}
