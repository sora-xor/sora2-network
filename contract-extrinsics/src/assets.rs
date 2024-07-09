use crate::{AccountIdBounds, AssetIdBounds};
use common::Balance;
use scale::Encode;

#[derive(Encode)]
pub enum AssetsCall<AssetId: AssetIdBounds, AccountId: AccountIdBounds> {
    /// Transfer amount of asset from caller to another account.
    /// You may found list of callable extrinsic in `pallet_contracts::Config::CallFilter`
    #[codec(index = 1)]
    Transfer {
        asset_id: AssetId,
        to: AccountId,
        amount: Balance,
    },
}
