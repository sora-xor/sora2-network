pub mod source_initializers {
    use crate::{Config, OrderBookFillSettings};
    use frame_support::dispatch::DispatchResult;
    use order_book::{MomentOf, OrderBookId};

    pub fn xst() {}

    pub fn order_book<T: Config>(
        caller: T::AccountId,
        bids_owner: T::AccountId,
        asks_owner: T::AccountId,
        fill_settings: Vec<(
            OrderBookId<T::AssetId, T::DEXId>,
            OrderBookFillSettings<MomentOf<T>>,
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
