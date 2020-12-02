#![cfg_attr(not(feature = "std"), no_std)]

#[allow(unused_imports)]
#[macro_use]
extern crate alloc;

use common::{EnsureDEXOwner, EnsureTradingPairExists};
use frame_support::{
    decl_error, decl_event, decl_module, decl_storage,
    dispatch::{DispatchError, DispatchResult},
    ensure,
    traits::Get,
    weights::Weight,
};
use sp_std::collections::btree_set::BTreeSet;
use sp_std::vec::Vec;

mod weights;

mod benchmarking;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

type TradingPair<T> = common::prelude::TradingPair<<T as assets::Trait>::AssetId>;

pub trait WeightInfo {
    fn register() -> Weight;
}

pub trait Trait: common::Trait + assets::Trait {
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
    type EnsureDEXOwner: EnsureDEXOwner<Self::DEXId, Self::AccountId, DispatchError>;

    /// Weight information for extrinsics in this pallet.
    type WeightInfo: WeightInfo;
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
        /// Trading pair is not registered for given DEXId.
        TradingPairDoesntExist,
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
        #[weight = <T as Trait>::WeightInfo::register()]
        pub fn register(origin, dex_id: T::DEXId, base_asset_id: T::AssetId, target_asset_id: T::AssetId) -> DispatchResult {
            let _author = T::EnsureDEXOwner::ensure_dex_owner(&dex_id, origin)?;
            //TODO: check token existence
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

impl<T: Trait> EnsureTradingPairExists<T::DEXId, T::AssetId, DispatchError> for Module<T> {
    fn ensure_trading_pair_exists(
        dex_id: &T::DEXId,
        target_asset_id: &T::AssetId,
    ) -> Result<(), DispatchError> {
        let trading_pair = TradingPair::<T> {
            base_asset_id: T::GetBaseAssetId::get(),
            target_asset_id: target_asset_id.clone(),
        };
        ensure!(
            Self::trading_pairs(dex_id).contains(&trading_pair),
            Error::<T>::TradingPairDoesntExist
        );
        Ok(())
    }
}

impl<T: Trait> Module<T> {
    pub fn list_trading_pairs(dex_id: T::DEXId) -> Vec<TradingPair<T>> {
        Self::trading_pairs(dex_id).iter().cloned().collect()
    }

    pub fn is_trading_pair_enabled(
        dex_id: T::DEXId,
        base_asset_id: T::AssetId,
        target_asset_id: T::AssetId,
    ) -> bool {
        let pair = TradingPair::<T> {
            base_asset_id,
            target_asset_id,
        };
        Self::trading_pairs(dex_id).contains(&pair)
    }
}
