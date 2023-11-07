use crate::Config;
use common::{AssetInfoProvider, Balance, PriceVariant};
use frame_support::pallet_prelude::*;
use frame_support::sp_runtime::traits::Zero;
use frame_support::traits::Time;
use frame_system::pallet_prelude::*;
use order_book::DataLayer;
use order_book::{MomentOf, OrderBook, OrderBookId};
use order_book::{OrderPrice, OrderVolume};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use sp_std::iter::repeat;
use sp_std::prelude::*;

pub mod settings {
    use codec::{Decode, Encode};
    use common::Balance;
    use order_book::OrderVolume;

    /// Parameters for filling one order book side
    #[derive(Encode, Decode, Clone, Debug, PartialEq, Eq, scale_info::TypeInfo)]
    pub struct SideFill {
        /// highest price for bids; lowest for asks
        pub best_price: Balance,
        /// lowest price for bids; highest for asks
        pub worst_price: Balance,
        pub price_step: Balance,
        pub orders_per_price: u32,
        /// Default: `min_lot_size..=max_lot_size`
        pub amount_range_inclusive: Option<(Balance, Balance)>,
    }

    /// Parameters for orders amount generation
    #[derive(Encode, Decode, Clone, PartialEq, Eq, scale_info::TypeInfo)]
    #[cfg_attr(feature = "std", derive(Debug))]
    pub struct RandomAmount {
        min: OrderVolume,
        max: OrderVolume,
    }

    impl RandomAmount {
        pub fn new(min: OrderVolume, max: OrderVolume) -> Option<Self> {
            if max >= min {
                Some(Self { max, min })
            } else {
                None
            }
        }
    }

    #[derive(Encode, Decode, Clone, PartialEq, Eq, scale_info::TypeInfo)]
    #[cfg_attr(feature = "std", derive(Debug))]
    pub struct OrderBookFill<Moment, BlockNumber> {
        /// Best price = highest, worst = lowest.
        pub bids: SideFill,
        /// Best price = lowest, worst = highest.
        pub asks: SideFill,
        /// Lifespan of inserted orders, max by default.
        pub lifespan: Option<Moment>,
        /// Seed for producing random values during the fill process. If `None`,
        /// current block is chosen
        pub random_seed: Option<BlockNumber>,
    }
}

/// Does not create an order book if it already exists
///
/// `who` is just some account. Used to mint non-divisible assets for creating corresponding
/// order book(-s).
pub fn create_multiple_empty_unchecked<T: Config>(
    who: &T::AccountId,
    order_book_ids: Vec<OrderBookId<T::AssetId, T::DEXId>>,
) -> Result<(), DispatchError> {
    let to_create_ids: Vec<_> = order_book_ids
        .into_iter()
        .filter(|id| !<order_book::OrderBooks<T>>::contains_key(id))
        .collect();
    for order_book_id in &to_create_ids {
        if !trading_pair::Pallet::<T>::is_trading_pair_enabled(
            &order_book_id.dex_id,
            &order_book_id.quote.into(),
            &order_book_id.base.into(),
        )? {
            trading_pair::Pallet::<T>::register_pair(
                order_book_id.dex_id,
                order_book_id.quote.into(),
                order_book_id.base.into(),
            )?;
        }
        if <T as Config>::AssetInfoProvider::is_non_divisible(&order_book_id.base)
            && <T as Config>::AssetInfoProvider::total_balance(&order_book_id.base, &who)?
                == Balance::zero()
        {
            assets::Pallet::<T>::mint_unchecked(&order_book_id.base, &who, 1)?;
        }
        order_book::Pallet::<T>::verify_create_orderbook_params(who, &order_book_id)?;
    }

    for order_book_id in to_create_ids {
        order_book::Pallet::<T>::create_orderbook_unchecked(&order_book_id)?;
    }
    Ok(())
}

/// Place orders into the orderbook.
///
/// In fill settings, `best_bid_price` should be at least 3 price steps from the
/// lowest accepted price, and `best_ask_price` - at least 3 steps below
/// maximum price.
pub fn fill_multiple_empty_unchecked<T: Config>(
    bids_owner: T::AccountId,
    asks_owner: T::AccountId,
    fill_settings: Vec<(
        OrderBookId<T::AssetId, T::DEXId>,
        settings::OrderBookFill<MomentOf<T>, BlockNumberFor<T>>,
    )>,
) -> Result<(), DispatchError> {
    let now = <T as order_book::Config>::Time::now();
    let current_block = frame_system::Pallet::<T>::block_number();
    let mut data = order_book::cache_data_layer::CacheDataLayer::<T>::new();

    for (order_book_id, settings) in fill_settings {
        fill_order_book(
            &mut data,
            order_book_id,
            asks_owner.clone(),
            bids_owner.clone(),
            settings,
            now,
            current_block,
        )?;
    }
    data.commit();
    Ok(())
}

fn default_amount_range<T: Config>(order_book: &OrderBook<T>) -> (Balance, Balance) {
    (
        *order_book.min_lot_size.balance(),
        *order_book.max_lot_size.balance(),
    )
}

/// Fill a single order book.
fn fill_order_book<T: Config>(
    data: &mut impl DataLayer<T>,
    book_id: OrderBookId<T::AssetId, T::DEXId>,
    asks_owner: T::AccountId,
    bids_owner: T::AccountId,
    settings: settings::OrderBookFill<MomentOf<T>, BlockNumberFor<T>>,
    now: MomentOf<T>,
    current_block: BlockNumberFor<T>,
) -> Result<(), DispatchError> {
    let lifespan = settings
        .lifespan
        .unwrap_or(<T as order_book::Config>::MAX_ORDER_LIFESPAN);
    let mut order_book = <order_book::OrderBooks<T>>::get(book_id)
        .ok_or(crate::Error::<T>::CannotFillUnknownOrderBook)?;

    let tick = order_book.tick_size.balance();
    ensure!(
        settings.bids.price_step % tick == 0
            && settings.asks.price_step % tick == 0
            && settings.bids.best_price % tick == 0
            && settings.asks.best_price % tick == 0
            && settings.bids.worst_price % tick == 0
            && settings.asks.worst_price % tick == 0,
        crate::Error::<T>::IncorrectPrice
    );

    let buy_prices = (0..)
        .map(|step| settings.bids.best_price - step * settings.bids.price_step)
        .take_while(|price| *price >= settings.bids.worst_price);
    let sell_prices = (0..)
        .map(|step| settings.asks.best_price + step * settings.asks.price_step)
        .take_while(|price| *price <= settings.asks.worst_price);

    let seed: u64 = settings
        .random_seed
        .unwrap_or(current_block)
        .try_into()
        .unwrap_or(0);
    let mut rand_generator = ChaCha8Rng::seed_from_u64(seed);
    let buy_amount_range = settings
        .bids
        .amount_range_inclusive
        .clone()
        .unwrap_or_else(|| default_amount_range(&order_book));
    let buy_orders: Vec<_> = buy_prices
        .flat_map(|price| {
            repeat(OrderPrice::divisible(price)).take(settings.bids.orders_per_price as usize)
        })
        .map(|price| {
            (
                price,
                order_book.align_amount(order_book.step_lot_size.copy_divisibility(
                    rand_generator.gen_range(buy_amount_range.0..=buy_amount_range.1),
                )),
            )
        })
        .collect();
    let sell_amount_range = settings
        .asks
        .amount_range_inclusive
        .clone()
        .unwrap_or_else(|| default_amount_range(&order_book));
    let sell_orders: Vec<_> = sell_prices
        .flat_map(|price| {
            repeat(OrderPrice::divisible(price)).take(settings.asks.orders_per_price as usize)
        })
        .map(|price| {
            (
                price,
                order_book.align_amount(order_book.step_lot_size.copy_divisibility(
                    rand_generator.gen_range(sell_amount_range.0..=sell_amount_range.1),
                )),
            )
        })
        .collect();

    // Total amount of assets to be locked
    let buy_quote_locked: Balance = buy_orders
        .iter()
        .map(|(quote, base)| *(*quote * (*base)).balance())
        .sum();
    let sell_base_locked: Balance = sell_orders.iter().map(|(_, base)| *base.balance()).sum();

    // mint required amount to make this extrinsic self-sufficient
    assets::Pallet::<T>::mint_unchecked(&book_id.quote, &bids_owner, buy_quote_locked)?;
    assets::Pallet::<T>::mint_unchecked(&book_id.base, &asks_owner, sell_base_locked)?;

    // place buy orders
    place_multiple_orders(
        data,
        &mut order_book,
        bids_owner.clone(),
        PriceVariant::Buy,
        buy_orders.into_iter(),
        now,
        lifespan,
        current_block,
    )?;

    // place sell orders
    place_multiple_orders(
        data,
        &mut order_book,
        asks_owner.clone(),
        PriceVariant::Sell,
        sell_orders.into_iter(),
        now,
        lifespan,
        current_block,
    )?;

    <order_book::OrderBooks<T>>::set(book_id, Some(order_book));
    Ok(())
}

fn place_multiple_orders<T: Config>(
    data: &mut impl DataLayer<T>,
    book: &mut OrderBook<T>,
    owner: T::AccountId,
    side: PriceVariant,
    orders: impl Iterator<Item = (OrderPrice, OrderVolume)>,
    time: MomentOf<T>,
    lifespan: MomentOf<T>,
    current_block: BlockNumberFor<T>,
) -> Result<(), DispatchError> {
    for (price, amount) in orders {
        let order_id = book.next_order_id();
        let order = order_book::LimitOrder::<T>::new(
            order_id,
            owner.clone(),
            side,
            price,
            amount,
            time,
            lifespan,
            current_block,
        );
        book.place_limit_order(order, data)?;
    }
    Ok(())
}
