use crate::{AccountIdBounds, AssetIdBounds};
use common::Balance;
use scale::Encode;

/// It is a part of a pallet dispatchables API.
/// The indexes can be found in your pallet code's #[pallet::call] section and check #[pallet::call_index(x)] attribute of the call.
/// If these attributes are missing, use source-code order (0-based).
/// You may found list of callable extrinsic in `pallet_contracts::Config::CallFilter`
#[derive(Encode)]
pub enum AssetsCall<AssetId: AssetIdBounds, AccountId: AccountIdBounds> {
    /// Transfer amount of asset from caller to another account.
    /// `assets::pallet::transfer`
    #[codec(index = 1)]
    Transfer {
        asset_id: AssetId,
        to: AccountId,
        amount: Balance,
    },
}
