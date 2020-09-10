#![cfg_attr(not(feature = "std"), no_std)]

#[allow(unused_imports)]
#[macro_use]
extern crate alloc;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

use frame_support::{
    decl_error, decl_event, decl_module, decl_storage, dispatch, ensure, traits::Get,
};
use sp_std::collections::btree_set::BTreeSet;

type TradingPair<T> = common::TradingPair<<T as assets::Trait>::AssetId>;

pub trait Trait: common::Trait + assets::Trait {
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
}

decl_storage! {
    trait Store for Module<T: Trait> as TradingPairModule {
        TradingPairs get(fn trading_pairs): map hasher(twox_64_concat) T::DEXId => BTreeSet<TradingPair<T>>;
    }
}

decl_event!(
    pub enum Event<T>
    where
        DEXId = <T as common::Trait>::DEXId,
        TP = TradingPair<T>,
    {
        TradingPairStored(DEXId, TP),
    }
);

decl_error! {
    pub enum Error for Module<T: Trait> {
        /// Registering trading pair already exists.
        TradingPairExists,
        /// The specified base asset ID for the trading pair is not allowed.
        ForbiddenBaseAssetId,
        /// The specified base asset ID is the same as target asset ID.
        IdenticalAssetIds,
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        type Error = Error<T>;

        fn deposit_event() = default;

        /// Register trading pair on the given DEX.
        /// Can be only called by the DEX owner.
        ///
        /// - `dex_id`: ID of the exchange.
        /// - `base_asset_id`: base asset ID.
        /// - `target_asset_id`: target asset ID.
        ///
        /// TODO: add information about weight
        #[weight = 10_000 + T::DbWeight::get().writes(1)]
        pub fn register(origin, dex_id: T::DEXId, base_asset_id: T::AssetId, target_asset_id: T::AssetId) -> dispatch::DispatchResult {
            let _author = T::ensure_dex_owner(&dex_id, origin)?;
            ensure!(base_asset_id != target_asset_id, Error::<T>::IdenticalAssetIds);
            ensure!(base_asset_id == T::GetBaseAssetId::get(), Error::<T>::ForbiddenBaseAssetId);
            let trading_pair = TradingPair::<T> {
                base_asset_id,
                target_asset_id
            };
            let inserted = <TradingPairs<T>>::mutate(&dex_id, |vec| vec.insert(trading_pair.clone()));
            ensure!(inserted, Error::<T>::TradingPairExists);
            Self::deposit_event(RawEvent::TradingPairStored(dex_id, trading_pair));
            Ok(())
        }
    }
}
