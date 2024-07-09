use crate::{AssetIdBounds, OrderIdBounds};
use common::{Balance, DEXId, PriceVariant};
use order_book::OrderBookId;
use scale::Encode;

/// It is a part of a pallet dispatchables API.
/// The indexes can be found in your pallet code's #[pallet::call] section and check #[pallet::call_index(x)] attribute of the call.
/// If these attributes are missing, use source-code order (0-based).
/// You may found list of callable extrinsic in `pallet_contracts::Config::CallFilter`
#[derive(Encode)]
pub enum OrderBookCall<AssetId: AssetIdBounds, OrderId: OrderIdBounds> {
    /// Places the limit order into the order book
    /// `order_book::pallet::place_limit_order`
    #[codec(index = 4)]
    PlaceLimitOrder {
        order_book_id: OrderBookId<AssetId, DEXId>,
        price: Balance,
        amount: Balance,
        side: PriceVariant,
        lifespan: u64,
    },
    /// Cancels the limit order
    /// `order_book::pallet::cancel_limit_order`
    #[codec(index = 5)]
    CancelLimitOrder {
        order_book_id: OrderBookId<AssetId, DEXId>,
        order_id: OrderId,
    },
    /// Cancels the list of limit orders
    /// `order_book::pallet::cancel_limit_orders_batch`
    #[codec(index = 6)]
    CancelLimitOrdersBatch {
        limit_orders_to_cancel: Vec<(OrderBookId<AssetId, DEXId>, Vec<OrderId>)>,
    },
    /// Executes the market order
    /// `order_book::pallet::execute_market_order`
    #[codec(index = 7)]
    ExecuteMarketOrder {
        order_book_id: OrderBookId<AssetId, DEXId>,
        direction: PriceVariant,
        amount: Balance,
    },
}
