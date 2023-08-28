#[allow(unused)]
#[cfg(not(test))]
use crate::{
    self as order_book, cache_data_layer::CacheDataLayer, traits::DataLayer, Config, Event,
    ExpirationScheduler, ExpirationsAgenda, LimitOrder, MarketRole, MomentOf, OrderAmount,
    OrderBook, OrderBookId, OrderBookStatus, OrderBooks, OrderVolume, Pallet, Payment,
};
#[allow(unused)]
#[cfg(test)]
use framenode_runtime::order_book::{
    self as order_book, cache_data_layer::CacheDataLayer, traits::DataLayer, Config, Event,
    ExpirationScheduler, ExpirationsAgenda, LimitOrder, MarketRole, MomentOf, OrderAmount,
    OrderBook, OrderBookId, OrderBookStatus, OrderBooks, OrderVolume, Pallet, Payment,
};

use assets::AssetIdOf;
use common::prelude::{BalanceUnit, Scalar};
use common::{balance, Balance, PriceVariant, ETH, VAL, XOR};
use frame_benchmarking::log::info;
use frame_benchmarking::Zero;
use frame_support::traits::Time;
use frame_system::RawOrigin;
use sp_runtime::traits::{CheckedAdd, CheckedMul, SaturatedConversion};
use sp_std::collections::btree_map::BTreeMap;
use sp_std::iter::repeat;
use sp_std::vec::Vec;

use crate::benchmarking::{assert_orders_numbers, bob, DEX};

use crate::OrderPrice;
use assets::Pallet as Assets;
use Pallet as OrderBookPallet;

// Creates and populates the order book with the following orders:
// price | volume | orders
//          Asks
//  11.5 |  255.8 | sell4, sell5, sell6
//  11.2 |  178.6 | sell2, sell3
//  11.0 |  176.3 | sell1
//  spread
//  10.0 |  168.5 | buy1
//   9.8 |  139.9 | buy2, buy3
//   9.5 |  261.3 | buy4, buy5, buy6
//          Bids
pub fn create_and_populate_order_book<T: Config>(
    order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
) {
    OrderBookPallet::<T>::create_orderbook(RawOrigin::Signed(bob::<T>()).into(), order_book_id)
        .unwrap();

    Assets::<T>::update_balance(
        RawOrigin::Root.into(),
        bob::<T>(),
        order_book_id.quote,
        balance!(1000000).try_into().unwrap(),
    )
    .unwrap();

    Assets::<T>::update_balance(
        RawOrigin::Root.into(),
        bob::<T>(),
        order_book_id.base,
        balance!(1000000).try_into().unwrap(),
    )
    .unwrap();

    let lifespan: Option<MomentOf<T>> = Some(10000u32.into());

    // prices
    let bp1 = balance!(10);
    let bp2 = balance!(9.8);
    let bp3 = balance!(9.5);
    let sp1 = balance!(11);
    let sp2 = balance!(11.2);
    let sp3 = balance!(11.5);

    // amounts
    let amount1 = balance!(168.5);
    let amount2 = balance!(95.2);
    let amount3 = balance!(44.7);
    let amount4 = balance!(56.4);
    let amount5 = balance!(89.9);
    let amount6 = balance!(115);
    let amount7 = balance!(176.3);
    let amount8 = balance!(85.4);
    let amount9 = balance!(93.2);
    let amount10 = balance!(36.6);
    let amount11 = balance!(205.5);
    let amount12 = balance!(13.7);

    OrderBookPallet::<T>::place_limit_order(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id,
        bp1,
        amount1,
        PriceVariant::Buy,
        lifespan,
    )
    .unwrap();
    OrderBookPallet::<T>::place_limit_order(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id,
        bp2,
        amount2,
        PriceVariant::Buy,
        lifespan,
    )
    .unwrap();
    OrderBookPallet::<T>::place_limit_order(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id,
        bp2,
        amount3,
        PriceVariant::Buy,
        lifespan,
    )
    .unwrap();
    OrderBookPallet::<T>::place_limit_order(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id,
        bp3,
        amount4,
        PriceVariant::Buy,
        lifespan,
    )
    .unwrap();
    OrderBookPallet::<T>::place_limit_order(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id,
        bp3,
        amount5,
        PriceVariant::Buy,
        lifespan,
    )
    .unwrap();
    OrderBookPallet::<T>::place_limit_order(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id,
        bp3,
        amount6,
        PriceVariant::Buy,
        lifespan,
    )
    .unwrap();

    OrderBookPallet::<T>::place_limit_order(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id,
        sp1,
        amount7,
        PriceVariant::Sell,
        lifespan,
    )
    .unwrap();
    OrderBookPallet::<T>::place_limit_order(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id,
        sp2,
        amount8,
        PriceVariant::Sell,
        lifespan,
    )
    .unwrap();
    OrderBookPallet::<T>::place_limit_order(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id,
        sp2,
        amount9,
        PriceVariant::Sell,
        lifespan,
    )
    .unwrap();
    OrderBookPallet::<T>::place_limit_order(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id,
        sp3,
        amount10,
        PriceVariant::Sell,
        lifespan,
    )
    .unwrap();
    OrderBookPallet::<T>::place_limit_order(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id,
        sp3,
        amount11,
        PriceVariant::Sell,
        lifespan,
    )
    .unwrap();
    OrderBookPallet::<T>::place_limit_order(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id,
        sp3,
        amount12,
        PriceVariant::Sell,
        lifespan,
    )
    .unwrap();
}

/// Returns id of order book to delete.
pub fn prepare_delete_orderbook_benchmark<T: Config>(
    fill_settings: FillSettings<T>,
) -> OrderBookId<AssetIdOf<T>, T::DEXId> {
    let order_book_id = OrderBookId::<AssetIdOf<T>, T::DEXId> {
        dex_id: DEX.into(),
        base: VAL.into(),
        quote: XOR.into(),
    };
    OrderBookPallet::<T>::create_orderbook(RawOrigin::Signed(bob::<T>()).into(), order_book_id)
        .expect("failed to create an order book");
    let mut order_book = OrderBookPallet::<T>::order_books(order_book_id).unwrap();
    let mut data_layer = CacheDataLayer::<T>::new();
    let _ = fill_order_book_worst_case::<T>(
        fill_settings.clone(),
        &mut order_book,
        &mut data_layer,
        true,
        true,
    );
    <OrderBooks<T>>::insert(order_book_id, order_book);
    data_layer.commit();
    order_book_id
}

/// Returns parameters for placing a limit order;
/// `author` should not be from `test_utils::generate_account`
pub fn prepare_place_orderbook_benchmark<T: Config>(
    fill_settings: FillSettings<T>,
    author: T::AccountId,
) -> (
    OrderBookId<AssetIdOf<T>, T::DEXId>,
    Balance,
    Balance,
    PriceVariant,
    MomentOf<T>,
) {
    let order_book_id = OrderBookId::<AssetIdOf<T>, T::DEXId> {
        dex_id: DEX.into(),
        base: VAL.into(),
        quote: XOR.into(),
    };
    OrderBookPallet::<T>::create_orderbook(RawOrigin::Signed(bob::<T>()).into(), order_book_id)
        .expect("failed to create an order book");
    // allow to execute all orders at once
    let mut order_book = <OrderBooks<T>>::get(order_book_id).unwrap();
    let max_side_orders =
        fill_settings.max_orders_per_price as u128 * fill_settings.max_side_price_count as u128;
    OrderBookPallet::<T>::update_orderbook(
        RawOrigin::Root.into(),
        order_book_id,
        *order_book.tick_size.balance(),
        *order_book.step_lot_size.balance(),
        *order_book.min_lot_size.balance(),
        *sp_std::cmp::max(
            order_book
                .min_lot_size
                .checked_mul_by_scalar(Scalar(max_side_orders + 1))
                .unwrap(),
            order_book.max_lot_size,
        )
        .balance(),
    )
    .unwrap();
    let mut data_layer = CacheDataLayer::<T>::new();

    // Place only buy orders
    let mut buy_settings = fill_settings.clone();
    buy_settings.max_orders_per_user = 1;
    let (mut users, mut lifespans) = fill_order_book_worst_case::<T>(
        buy_settings,
        &mut order_book,
        &mut data_layer,
        true,
        false,
    );

    let order_amount = sp_std::cmp::max(order_book.step_lot_size, order_book.min_lot_size);
    let mut fill_user_settings = fill_settings.clone();
    fill_user_settings.max_orders_per_user -= 1;
    fill_user_orders(
        &mut data_layer,
        fill_user_settings,
        &mut order_book,
        PriceVariant::Sell,
        order_amount,
        author.clone(),
        &mut lifespans,
    );

    // skip to an empty block
    let filled_block = lifespans.next().unwrap();
    let mut lifespans = lifespans.skip_while(|b| *b == filled_block);
    let to_fill = lifespans.next().unwrap();
    // we are going to fill this lifespan, so skipping it for possible future use of the iter
    let mut _lifespans = lifespans.skip_while(|b| *b == to_fill);
    // different order book because we just want to fill expirations
    let order_book_id_2 = OrderBookId::<AssetIdOf<T>, T::DEXId> {
        dex_id: DEX.into(),
        base: ETH.into(),
        quote: XOR.into(),
    };
    OrderBookPallet::<T>::create_orderbook(RawOrigin::Signed(bob::<T>()).into(), order_book_id_2)
        .expect("failed to create an order book");
    let mut order_book_2 = <OrderBooks<T>>::get(order_book_id_2).unwrap();
    let order_amount_2 = sp_std::cmp::max(order_book_2.step_lot_size, order_book_2.min_lot_size);
    let mut fill_expiration_settings = fill_settings.clone();
    // leave a room for 1
    fill_expiration_settings.max_expiring_orders_per_block -= 1;
    // mint other base asset as well
    let mut users = users.inspect(move |user| {
        assets::Pallet::<T>::mint_unchecked(
            &order_book_id_2.base,
            &user,
            *order_amount_2.balance(),
        )
        .unwrap();
    });
    fill_expiration_schedule(
        &mut data_layer,
        fill_expiration_settings.clone(),
        &mut order_book_2,
        PriceVariant::Sell,
        order_amount_2,
        &mut users,
        to_fill,
    );

    <OrderBooks<T>>::insert(order_book_id, order_book.clone());
    <OrderBooks<T>>::insert(order_book_id_2, order_book_2);
    data_layer.commit();

    let price = order_book.tick_size;
    // to execute all bids
    let amount: OrderVolume = data_layer
        .get_aggregated_bids(&order_book_id)
        .iter()
        .map(|(_p, v)| *v)
        .fold(BalanceUnit::zero(), |acc, item| {
            acc.checked_add(&item).unwrap()
        });
    // to place remaining amount as limit order
    let amount = amount + order_book.min_lot_size;
    let side = PriceVariant::Sell;
    let lifespan = to_fill.saturated_into::<MomentOf<T>>();
    assets::Pallet::<T>::mint_unchecked(&order_book_id.base, &author, *amount.balance()).unwrap();

    assert_orders_numbers::<T>(
        order_book_id,
        Some(max_side_orders as usize),
        None,
        author.clone(),
        (fill_settings.max_orders_per_user - 1) as usize,
        lifespan,
        (fill_settings.max_expiring_orders_per_block - 1) as usize,
    );

    (
        order_book_id,
        *price.balance(),
        *amount.balance(),
        side,
        lifespan,
    )
}

/// Returns parameters for cancelling a limit order.
/// `expirations_first` switches between two cases; it's not clear which one is heavier.
/// `author` should not be from `test_utils::generate_account`.
pub fn prepare_cancel_orderbook_benchmark<T: Config>(
    fill_settings: FillSettings<T>,
    author: T::AccountId,
    place_first_expiring: bool,
) -> (OrderBookId<AssetIdOf<T>, T::DEXId>, T::OrderId) {
    let order_book_id = OrderBookId::<AssetIdOf<T>, T::DEXId> {
        dex_id: DEX.into(),
        base: VAL.into(),
        quote: XOR.into(),
    };
    OrderBookPallet::<T>::create_orderbook(RawOrigin::Signed(bob::<T>()).into(), order_book_id)
        .expect("failed to create an order book");
    let mut order_book = <OrderBooks<T>>::get(order_book_id).unwrap();
    let mut data_layer = CacheDataLayer::<T>::new();

    // fill aggregated bids
    let mut buy_settings = fill_settings.clone();
    buy_settings.max_orders_per_price = 1;
    let (mut users, mut lifespans) = fill_order_book_worst_case::<T>(
        buy_settings,
        &mut order_book,
        &mut data_layer,
        true,
        false,
    );

    // fill the price of the cancelled order
    let mut fill_price_settings = fill_settings.clone();
    // account for previous fill + room for order to cancel
    fill_price_settings.max_orders_per_price -= 2;
    // we don't want to face `MAX_PRICE_SHIFT`
    let target_price = data_layer.best_bid(&order_book_id).unwrap().0;
    let order_amount = sp_std::cmp::max(order_book.step_lot_size, order_book.min_lot_size);
    fill_price(
        &mut data_layer,
        fill_price_settings,
        &mut order_book,
        PriceVariant::Buy,
        order_amount,
        target_price,
        &mut users,
        &mut lifespans,
    );

    // fill user orders (leave a room for cancelled which will be inserted later)
    let mut fill_user_settings = fill_settings.clone();
    fill_user_settings.max_orders_per_user -= 1;
    fill_user_orders(
        &mut data_layer,
        fill_user_settings,
        &mut order_book,
        PriceVariant::Sell,
        order_amount,
        author.clone(),
        &mut lifespans,
    );

    // skip to an empty block
    let filled_block = lifespans.next().unwrap();
    let mut lifespans = lifespans.skip_while(|b| *b == filled_block);
    let to_fill = lifespans.next().unwrap();
    // we are going to fill this lifespan, so skipping it for possible future use of the iter
    let mut _lifespans = lifespans.skip_while(|b| *b == to_fill);

    assets::Pallet::<T>::mint_unchecked(
        &order_book.order_book_id.quote,
        &author,
        *target_price.checked_mul(&order_amount).unwrap().balance(),
    )
    .unwrap();
    // don't repeat this code for both `place_first_expiring` cases
    let place_to_cancel = |order_book: &mut OrderBook<T>, data_layer: &mut CacheDataLayer<T>| {
        let id = order_book.next_order_id();
        let order = LimitOrder::<T>::new(
            id,
            author.clone(),
            PriceVariant::Buy,
            target_price,
            order_amount,
            T::Time::now(),
            to_fill.saturated_into(),
            frame_system::Pallet::<T>::block_number(),
        );
        order_book.place_limit_order(order, data_layer).unwrap();
        id
    };

    let mut to_cancel_id = T::OrderId::from(0u8);
    if place_first_expiring {
        to_cancel_id = place_to_cancel(&mut order_book, &mut data_layer);
    }

    // different order book because we just want to fill expirations and don't face restrictions
    // from the first one
    let order_book_id_2 = OrderBookId::<AssetIdOf<T>, T::DEXId> {
        dex_id: DEX.into(),
        base: ETH.into(),
        quote: XOR.into(),
    };
    OrderBookPallet::<T>::create_orderbook(RawOrigin::Signed(bob::<T>()).into(), order_book_id_2)
        .expect("failed to create an order book");
    let mut order_book_2 = <OrderBooks<T>>::get(order_book_id_2).unwrap();
    let order_amount_2 = sp_std::cmp::max(order_book_2.step_lot_size, order_book_2.min_lot_size);
    let mut fill_expiration_settings = fill_settings.clone();
    // we add one more separately
    fill_expiration_settings.max_expiring_orders_per_block -= 1;
    // mint other base asset as well
    let mut users = users.inspect(move |user| {
        assets::Pallet::<T>::mint_unchecked(
            &order_book_id_2.base,
            &user,
            *order_amount_2.balance(),
        )
        .unwrap();
    });
    fill_expiration_schedule(
        &mut data_layer,
        fill_expiration_settings.clone(),
        &mut order_book_2,
        PriceVariant::Sell,
        order_amount_2,
        &mut users,
        to_fill,
    );

    if !place_first_expiring {
        to_cancel_id = place_to_cancel(&mut order_book, &mut data_layer);
    }

    data_layer.commit();
    <OrderBooks<T>>::insert(order_book_id, order_book.clone());

    (order_book_id, to_cancel_id)
}

pub mod presets {
    use crate::benchmarking::preparation::FillSettings;
    #[cfg(not(test))]
    use crate::Config;
    #[cfg(test)]
    use framenode_runtime::order_book::Config;

    pub fn preset_1<T: Config>() -> FillSettings<T> {
        FillSettings::<T>::new(16, 16, 16, 128)
    }

    pub fn preset_2<T: Config>() -> FillSettings<T> {
        FillSettings::<T>::new(32, 32, 32, 256)
    }

    pub fn preset_3<T: Config>() -> FillSettings<T> {
        FillSettings::<T>::new(64, 64, 64, 512)
    }

    pub fn preset_4<T: Config>() -> FillSettings<T> {
        FillSettings::<T>::new(128, 128, 128, 1024)
    }

    pub fn preset_5<T: Config>() -> FillSettings<T> {
        FillSettings::<T>::new(256, 256, 256, 2048)
    }

    pub fn preset_6<T: Config>() -> FillSettings<T> {
        FillSettings::<T>::new(512, 512, 512, 4096)
    }

    pub fn preset_7<T: Config>() -> FillSettings<T> {
        FillSettings::<T>::new(1024, 1024, 1024, 8192)
    }
}

#[derive(Clone, Debug)]
pub struct FillSettings<T: Config> {
    pub now: <<T as Config>::Time as Time>::Moment,
    pub max_side_price_count: u32,
    pub max_orders_per_price: u32,
    pub max_orders_per_user: u32,
    pub max_expiring_orders_per_block: u32,
}

impl<T: Config> FillSettings<T> {
    pub fn new(
        max_side_price_count: u32,
        max_orders_per_price: u32,
        max_orders_per_user: u32,
        max_expiring_orders_per_block: u32,
    ) -> Self {
        Self {
            now: T::Time::now(),
            max_side_price_count,
            max_orders_per_price,
            max_orders_per_user,
            max_expiring_orders_per_block,
        }
    }
}

fn fill_expiration_schedule<T: Config>(
    data: &mut impl DataLayer<T>,
    settings: FillSettings<T>,
    order_book: &mut OrderBook<T>,
    side: PriceVariant,
    order_amount: OrderVolume,
    users: &mut impl Iterator<Item = T::AccountId>,
    lifespan: u64,
) {
    let mut lifespans = repeat(lifespan).take(settings.max_expiring_orders_per_block as usize);
    match side {
        PriceVariant::Buy => fill_order_book_side(
            data,
            settings.clone(),
            order_book,
            side,
            order_amount,
            &mut bid_prices_iterator(order_book.tick_size, settings.max_side_price_count),
            users,
            &mut lifespans,
        ),
        PriceVariant::Sell => fill_order_book_side(
            data,
            settings.clone(),
            order_book,
            side,
            order_amount,
            &mut ask_prices_iterator(order_book.tick_size, settings.max_side_price_count),
            users,
            &mut lifespans,
        ),
    }
}

fn fill_user_orders<T: Config>(
    data: &mut impl DataLayer<T>,
    settings: FillSettings<T>,
    order_book: &mut OrderBook<T>,
    side: PriceVariant,
    order_amount: OrderVolume,
    author: T::AccountId,
    lifespans: &mut impl Iterator<Item = u64>,
) {
    // fill user orders of the author
    let FillSettings {
        now: _,
        max_side_price_count,
        max_orders_per_price: _,
        max_orders_per_user,
        max_expiring_orders_per_block: _,
    } = settings;
    // Since we fill orders of the user, it is the only author
    match side {
        PriceVariant::Buy => {
            let max_price = order_book
                .tick_size
                .checked_mul_by_scalar(Scalar(max_side_price_count))
                .unwrap();
            assets::Pallet::<T>::mint_unchecked(
                &order_book.order_book_id.quote,
                &author,
                *max_price
                    .checked_mul(&order_amount)
                    .unwrap()
                    .checked_mul_by_scalar(Scalar(max_orders_per_user))
                    .unwrap()
                    .balance(),
            )
            .unwrap()
        }
        PriceVariant::Sell => assets::Pallet::<T>::mint_unchecked(
            &order_book.order_book_id.base,
            &author,
            *order_amount
                .checked_mul_by_scalar(Scalar(max_orders_per_user))
                .unwrap()
                .balance(),
        )
        .unwrap(),
    }
    let mut users = repeat(author).take(max_orders_per_user.try_into().unwrap());
    match side {
        PriceVariant::Buy => fill_order_book_side(
            data,
            settings,
            order_book,
            side,
            order_amount,
            &mut bid_prices_iterator(order_book.tick_size, max_side_price_count),
            &mut users,
            lifespans,
        ),
        PriceVariant::Sell => fill_order_book_side(
            data,
            settings,
            order_book,
            side,
            order_amount,
            &mut ask_prices_iterator(order_book.tick_size, max_side_price_count),
            &mut users,
            lifespans,
        ),
    };
}

/// Fill `side` of an `order_book` according to `settings`.
/// Places `settings.max_orders_per_price * prices.len()` orders at max.
///
/// Each order is for `orders_amount`. `users`, and `lifespans` specify
/// corresponding fields for each order. If at least one of the iterators finishes, it stops.
fn fill_order_book_side<T: Config>(
    data: &mut impl DataLayer<T>,
    settings: FillSettings<T>,
    order_book: &mut OrderBook<T>,
    side: PriceVariant,
    orders_amount: OrderVolume,
    prices: &mut impl Iterator<Item = OrderPrice>,
    users: &mut impl Iterator<Item = T::AccountId>,
    lifespans: &mut impl Iterator<Item = u64>,
) {
    #[cfg(feature = "std")]
    use std::io::Write;

    let current_block = frame_system::Pallet::<T>::block_number();
    let mut total_payment = Payment::new(order_book.order_book_id);
    let mut to_expire = BTreeMap::<_, Vec<_>>::new();
    #[cfg(feature = "std")]
    println!("inserting orders");
    for (i, price) in prices.enumerate() {
        #[cfg(feature = "std")]
        {
            print!(
                "\r{}/{} ({}%)",
                i,
                settings.max_side_price_count,
                100.0 * (i as f32) / (settings.max_side_price_count as f32)
            );
            std::io::stdout().flush().unwrap();
        }
        fill_price_inner(
            data,
            settings.clone(),
            order_book,
            side,
            orders_amount,
            price,
            users,
            lifespans,
            current_block,
            &mut total_payment,
            &mut to_expire,
        );
    }
    #[cfg(feature = "std")]
    println!("\nlocking payments");
    total_payment
        .execute_all::<OrderBookPallet<T>, OrderBookPallet<T>>()
        .unwrap();
    #[cfg(feature = "std")]
    println!("scheduling expirations");
    #[cfg(feature = "std")]
    let total_expirations = to_expire.len();
    for (i, (expires_at, orders)) in to_expire.into_iter().enumerate() {
        #[cfg(feature = "std")]
        {
            print!(
                "\r{}/{} ({}%)",
                i,
                total_expirations,
                100.0 * (i as f32) / (total_expirations as f32)
            );
            std::io::stdout().flush().unwrap();
        }
        <ExpirationsAgenda<T>>::try_mutate(expires_at, |block_expirations| {
            block_expirations.try_extend(
                orders
                    .into_iter()
                    .map(|order_id| (order_book.order_book_id, order_id)),
            )
        })
        .expect("Failed to schedule orders for expiration");
    }
    #[cfg(feature = "std")]
    println!();
}

fn fill_price<T: Config>(
    data: &mut impl DataLayer<T>,
    settings: FillSettings<T>,
    order_book: &mut OrderBook<T>,
    side: PriceVariant,
    orders_amount: OrderVolume,
    price: OrderPrice,
    users: &mut impl Iterator<Item = T::AccountId>,
    lifespans: &mut impl Iterator<Item = u64>,
) {
    let current_block = frame_system::Pallet::<T>::block_number();
    let mut total_payment = Payment::new(order_book.order_book_id);
    let mut to_expire = BTreeMap::<_, Vec<_>>::new();
    fill_price_inner(
        data,
        settings,
        order_book,
        side,
        orders_amount,
        price,
        users,
        lifespans,
        current_block,
        &mut total_payment,
        &mut to_expire,
    );
    total_payment
        .execute_all::<OrderBookPallet<T>, OrderBookPallet<T>>()
        .unwrap();
    // should avoid duplicating with `fill_order_book_worst_case` somehow
    for (expires_at, orders) in to_expire.into_iter() {
        <ExpirationsAgenda<T>>::try_mutate(expires_at, |block_expirations| {
            block_expirations.try_extend(
                orders
                    .into_iter()
                    .map(|order_id| (order_book.order_book_id, order_id)),
            )
        })
        .expect("Failed to schedule orders for expiration");
    }
}

#[inline]
/// Version of `fill_price` for optimized execution that aggregates payments/expirations (and
/// avoids multiple calls to retrieve block #)
fn fill_price_inner<T: Config>(
    data: &mut impl DataLayer<T>,
    settings: FillSettings<T>,
    order_book: &mut OrderBook<T>,
    side: PriceVariant,
    orders_amount: OrderVolume,
    price: OrderPrice,
    users: &mut impl Iterator<Item = T::AccountId>,
    lifespans: &mut impl Iterator<Item = u64>,
    current_block: T::BlockNumber,
    total_payment: &mut Payment<T::AssetId, T::AccountId, T::DEXId>,
    to_expire: &mut BTreeMap<T::BlockNumber, Vec<T::OrderId>>,
) {
    for _ in 0..settings.max_orders_per_price {
        let Some(user) = users.next() else { break };
        let Some(lifespan) = lifespans.next() else { break };
        let order = LimitOrder::<T>::new(
            order_book.next_order_id(),
            user.clone(),
            side,
            price,
            orders_amount,
            settings.now.clone(),
            lifespan.saturated_into(),
            current_block,
        );
        // Instead of `order_book.place_limit_order(order, data)` we do the same steps manually
        // in order to avoid overhead on checking various restrictions and other unnecessary
        // stuff

        let order_id = order.id;
        let expires_at = order.expires_at;
        // lock corresponding currency
        let lock_amount = order.deal_amount(MarketRole::Taker, None).unwrap();
        let lock_asset = lock_amount.associated_asset(&order_book.order_book_id);
        total_payment
            .to_lock
            .entry(*lock_asset)
            .or_default()
            .entry(order.owner.clone())
            .and_modify(|amount| *amount += *lock_amount.value())
            .or_insert(*lock_amount.value());
        // insert the order in storages
        data.insert_limit_order(&order_book.order_book_id, order)
            .unwrap();
        // schedule its expiration
        to_expire.entry(expires_at).or_default().push(order_id);
    }
}

fn bid_prices_iterator(
    tick_size: OrderPrice,
    max_side_price_count: u32,
) -> impl Iterator<Item = BalanceUnit> {
    (1..=max_side_price_count).map(move |i| tick_size * Scalar(i))
}

fn ask_prices_iterator(
    tick_size: OrderPrice,
    max_side_price_count: u32,
) -> impl Iterator<Item = BalanceUnit> {
    (max_side_price_count + 1..=2 * max_side_price_count)
        .rev()
        .map(move |i| tick_size * Scalar(i))
}

fn users_iterator<T: Config>(
    order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
    max_order_amount: OrderVolume,
    max_price: OrderPrice,
    max_orders_per_user: u32,
) -> impl Iterator<Item = T::AccountId> {
    let mint_per_user = max_order_amount * Scalar(max_orders_per_user);
    (1..)
        .map(crate::test_utils::generate_account::<T>)
        // each user receives assets that should be enough for placing their orders
        .inspect(move |user| {
            assets::Pallet::<T>::mint_unchecked(
                &order_book_id.base,
                &user,
                *mint_per_user.balance(),
            )
            .unwrap();
            assets::Pallet::<T>::mint_unchecked(
                &order_book_id.quote,
                &user,
                *max_price.checked_mul(&mint_per_user).unwrap().balance(),
            )
            .unwrap();
        })
        // yield same user for `max_orders_per_user` orders.
        // `inspect` is still called only once for each user.
        .flat_map(move |user| repeat(user).take(max_orders_per_user.try_into().unwrap()))
}

fn lifespans_iterator<T: Config>(
    max_expiring_orders_per_block: u32,
    start_from_block: u64,
) -> impl Iterator<Item = u64> {
    (start_from_block..)
        .map(|i| {
            i * T::MILLISECS_PER_BLOCK.saturated_into::<u64>()
                + T::MIN_ORDER_LIFESPAN.saturated_into::<u64>()
        })
        // same lifespan should be yielded for `max_expiring_orders_per_block` orders
        .flat_map(move |lifespan| {
            repeat(lifespan).take(max_expiring_orders_per_block.try_into().unwrap())
        })
}

/// Returns per-order iterators for users and lifespans. They can be used for proceeding with
/// filling respective storages (user orders and expiration schedules respectively). Can be seen
/// as cursors.
pub fn fill_order_book_worst_case<T: Config + assets::Config>(
    settings: FillSettings<T>,
    order_book: &mut OrderBook<T>,
    data: &mut impl DataLayer<T>,
    place_buy: bool,
    place_sell: bool,
) -> (
    impl Iterator<Item = T::AccountId>,
    impl Iterator<Item = u64>,
) {
    let FillSettings {
        now: _,
        max_side_price_count,
        max_orders_per_price,
        max_orders_per_user,
        max_expiring_orders_per_block,
    } = settings;

    let order_amount = sp_std::cmp::max(order_book.step_lot_size, order_book.min_lot_size);
    let max_price = order_book.tick_size * Scalar(2 * max_side_price_count);

    // Owners for each placed order
    let mut users = users_iterator::<T>(
        order_book.order_book_id,
        order_amount,
        max_price,
        max_orders_per_user,
    );
    // Lifespans for each placed order
    let mut lifespans = lifespans_iterator::<T>(max_expiring_orders_per_block, 1);

    if place_buy {
        let mut bid_prices = bid_prices_iterator(order_book.tick_size, max_side_price_count);
        #[cfg(feature = "std")]
        let start_time = std::time::Instant::now();
        #[cfg(feature = "std")]
        println!(
            "Starting placement of bid orders, {} orders per price",
            max_orders_per_price
        );
        info!("Placing bids...");
        fill_order_book_side(
            data,
            settings.clone(),
            order_book,
            PriceVariant::Buy,
            order_amount,
            &mut bid_prices,
            &mut users,
            &mut lifespans,
        );

        info!("Placed all bids");
        #[cfg(feature = "std")]
        println!("\nprocessed all bid prices in {:?}", start_time.elapsed());
    }

    if place_sell {
        let mut ask_prices = ask_prices_iterator(order_book.tick_size, max_side_price_count);
        #[cfg(feature = "std")]
        let start_time = std::time::Instant::now();
        #[cfg(feature = "std")]
        println!(
            "Starting placement of ask orders, {} orders per price",
            max_orders_per_price
        );
        info!("Placing asks...");
        fill_order_book_side(
            data,
            settings,
            order_book,
            PriceVariant::Sell,
            order_amount,
            &mut ask_prices,
            &mut users,
            &mut lifespans,
        );
        info!("Placed all asks");
        #[cfg(feature = "std")]
        println!("\nprocessed all ask prices in {:?}", start_time.elapsed());
    }
    (users, lifespans)
}
