use common::mock::alice;
use common::PriceVariant;
use price_tools::AVG_BLOCK_SPAN;

use crate::{AssetId, PoolXYK, PriceTools, RuntimeOrigin};

pub fn ensure_pool_initialized(asset_a: AssetId, asset_b: AssetId) {
    PoolXYK::initialize_pool(RuntimeOrigin::signed(alice()), 0, asset_a, asset_b).unwrap();
}

pub fn fill_spot_price() {
    for _ in 0..AVG_BLOCK_SPAN {
        PriceTools::average_prices_calculation_routine(PriceVariant::Buy);
        PriceTools::average_prices_calculation_routine(PriceVariant::Sell);
    }
}
