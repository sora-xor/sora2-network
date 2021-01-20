#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::unnecessary_mut_passed)]

use codec::Codec;
use sp_std::prelude::*;

sp_api::decl_runtime_apis! {
    pub trait TradingPairAPI<DEXId, TradingPair, AssetId, LiquiditySourceType> where
        DEXId: Codec,
        TradingPair: Codec,
        AssetId: Codec,
        LiquiditySourceType: Codec,
    {
        fn list_enabled_pairs(dex_id: DEXId) -> Vec<TradingPair>;

        fn is_pair_enabled(dex_id: DEXId, base_asset_id: AssetId, target_asset_id: AssetId) -> bool;

        fn list_enabled_sources_for_pair(
            dex_id: DEXId,
            base_asset_id: AssetId,
            target_asset_id: AssetId,
        ) -> Vec<LiquiditySourceType>;

        fn is_source_enabled_for_pair(
            dex_id: DEXId,
            base_asset_id: AssetId,
            target_asset_id: AssetId,
            source_type: LiquiditySourceType,
        ) -> bool;
    }
}
