#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::unnecessary_mut_passed)]

use codec::Codec;
use sp_runtime::traits::{MaybeDisplay, MaybeFromStr};
use sp_std::prelude::*;

sp_api::decl_runtime_apis! {
    pub trait DEXAPI<AssetId, DEXId, Balance, LiquiditySourceType> where
        AssetId: Codec,
        DEXId: Codec,
        LiquiditySourceType: Codec,
        Balance: Codec + MaybeDisplay + MaybeFromStr,
    {
        fn get_price_with_desired_input(
            dex_id: DEXId,
            liquidity_source_type: LiquiditySourceType,
            input_asset_id: AssetId,
            output_asset_id: AssetId,
            desired_input_amount: Balance,
        ) -> Option<Balance>;

        fn get_price_with_desired_output(
            dex_id: DEXId,
            liquidity_source_type: LiquiditySourceType,
            input_asset_id: AssetId,
            output_asset_id: AssetId,
            desired_output_amount: Balance,
        ) -> Option<Balance>;
    }
}
