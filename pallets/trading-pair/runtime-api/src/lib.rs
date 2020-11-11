#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::unnecessary_mut_passed)]

use codec::Codec;
use sp_std::prelude::*;

sp_api::decl_runtime_apis! {
    pub trait TradingPairAPI<DEXId, TradingPair, AssetId> where
        DEXId: Codec,
        TradingPair: Codec,
        AssetId: Codec,
    {
        fn list_enabled_pairs(dex_id: DEXId) -> Vec<TradingPair>;

        fn is_pair_enabled(dex_id: DEXId, base_asset_id: AssetId, target_asset_id: AssetId) -> bool;
    }
}
