pub mod source_initializers {
    use crate::{settings, Config};
    use codec::{Decode, Encode};
    use common::prelude::BalanceUnit;
    use frame_support::dispatch::DispatchResult;
    use frame_system::pallet_prelude::BlockNumberFor;
    use order_book::{MomentOf, OrderBookId};
    use sp_std::fmt::Debug;
    use sp_std::vec::Vec;

    /// Create multiple order books with default parameters if do not exist and
    /// fill them according to given parameters.
    ///
    /// Balance for placing the orders is minted automatically, trading pairs are
    /// created if needed.
    ///
    /// Parameters:
    /// - `caller`: account to mint non-divisible assets (for creating an order book)
    /// - `bids_owner`: Creator of the buy orders placed on the order books,
    /// - `asks_owner`: Creator of the sell orders placed on the order books,
    /// - `fill_settings`: Parameters for placing the orders in each order book.
    pub fn order_book<T: Config>(
        caller: T::AccountId,
        bids_owner: T::AccountId,
        asks_owner: T::AccountId,
        fill_settings: Vec<(
            OrderBookId<T::AssetId, T::DEXId>,
            settings::OrderBookFill<MomentOf<T>, BlockNumberFor<T>>,
        )>,
    ) -> DispatchResult {
        let order_book_ids: Vec<_> = fill_settings.iter().map(|(id, _)| id).cloned().collect();
        crate::pallet_tools::order_book::create_multiple_empty_unchecked::<T>(
            &caller,
            order_book_ids,
        )?;
        crate::pallet_tools::order_book::fill_multiple_empty_unchecked::<T>(
            bids_owner,
            asks_owner,
            fill_settings,
        )?;
        Ok(())
    }
}
