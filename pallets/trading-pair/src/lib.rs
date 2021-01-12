#![cfg_attr(not(feature = "std"), no_std)]

#[allow(unused_imports)]
#[macro_use]
extern crate alloc;

use common::{EnsureDEXOwner, EnsureTradingPairExists, LiquiditySourceType};
use frame_support::{
    decl_error, decl_event, decl_module, decl_storage,
    dispatch::{DispatchError, DispatchResult},
    ensure,
    traits::Get,
    weights::Weight,
    IterableStorageDoubleMap,
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
type Assets<T> = assets::Module<T>;
type DEXManager<T> = dex_manager::Module<T>;

pub trait WeightInfo {
    fn register() -> Weight;
}

pub trait Trait: common::Trait + assets::Trait + dex_manager::Trait {
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
    type EnsureDEXOwner: EnsureDEXOwner<Self::DEXId, Self::AccountId, DispatchError>;

    /// Weight information for extrinsics in this pallet.
    type WeightInfo: WeightInfo;
}

decl_storage! {
    trait Store for Module<T: Trait> as TradingPairModule {
        EnabledSources get(fn enabled_sources): double_map hasher(twox_64_concat) T::DEXId,
                                                         hasher(blake2_128_concat) TradingPair<T> => Option<BTreeSet<LiquiditySourceType>>;
    }
    add_extra_genesis {
        config(trading_pairs): Vec<(T::DEXId, TradingPair<T>)>;

        build(|config: &GenesisConfig<T>| {
            config.trading_pairs.iter().for_each(|(dex_id, pair)| {
                EnabledSources::<T>::insert(&dex_id, &pair, BTreeSet::<LiquiditySourceType>::new());
            })
        })
    }
}

decl_event!(
    pub enum Event<T>
    where
        DEXId = <T as common::Trait>::DEXId,
        TradingPair = TradingPair<T>,
    {
        /// Trading pair has been redistered on a DEX. [DEX Id, Trading Pair]
        TradingPairStored(DEXId, TradingPair),
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
            let _author = T::EnsureDEXOwner::ensure_can_manage(&dex_id, origin)?;
            Assets::<T>::ensure_asset_exists(&base_asset_id)?;
            Assets::<T>::ensure_asset_exists(&target_asset_id)?;
            ensure!(base_asset_id != target_asset_id, Error::<T>::IdenticalAssetIds);
            ensure!(base_asset_id == T::GetBaseAssetId::get(), Error::<T>::ForbiddenBaseAssetId);
            let trading_pair = TradingPair::<T> {
                base_asset_id,
                target_asset_id
            };
            ensure!(Self::enabled_sources(&dex_id, &trading_pair).is_none(), Error::<T>::TradingPairExists);
            EnabledSources::<T>::insert(&dex_id, &trading_pair, BTreeSet::<LiquiditySourceType>::new());
            Self::deposit_event(RawEvent::TradingPairStored(dex_id, trading_pair));
            Ok(())
        }
    }
}

impl<T: Trait> EnsureTradingPairExists<T::DEXId, T::AssetId, DispatchError> for Module<T> {
    fn ensure_trading_pair_exists(
        dex_id: &T::DEXId,
        base_asset_id: &T::AssetId,
        target_asset_id: &T::AssetId,
    ) -> DispatchResult {
        ensure!(
            Self::is_trading_pair_enabled(dex_id, base_asset_id, target_asset_id)?,
            Error::<T>::TradingPairDoesntExist
        );
        Ok(())
    }
}

impl<T: Trait> Module<T> {
    pub fn list_trading_pairs(dex_id: &T::DEXId) -> Result<Vec<TradingPair<T>>, DispatchError> {
        DEXManager::<T>::ensure_dex_exists(dex_id)?;
        Ok(EnabledSources::<T>::iter_prefix(dex_id)
            .map(|(pair, _)| pair)
            .collect())
    }

    pub fn is_trading_pair_enabled(
        dex_id: &T::DEXId,
        &base_asset_id: &T::AssetId,
        &target_asset_id: &T::AssetId,
    ) -> Result<bool, DispatchError> {
        DEXManager::<T>::ensure_dex_exists(dex_id)?;
        let pair = TradingPair::<T> {
            base_asset_id,
            target_asset_id,
        };
        Ok(Self::enabled_sources(dex_id, &pair).is_some())
    }

    pub fn list_enabled_sources_for_trading_pair(
        dex_id: &T::DEXId,
        &base_asset_id: &T::AssetId,
        &target_asset_id: &T::AssetId,
    ) -> Result<BTreeSet<LiquiditySourceType>, DispatchError> {
        DEXManager::<T>::ensure_dex_exists(dex_id)?;
        let pair = TradingPair::<T> {
            base_asset_id,
            target_asset_id,
        };
        Ok(Self::enabled_sources(dex_id, &pair).ok_or(Error::<T>::TradingPairDoesntExist)?)
    }

    pub fn is_source_enabled_for_trading_pair(
        dex_id: &T::DEXId,
        base_asset_id: &T::AssetId,
        target_asset_id: &T::AssetId,
        source_type: LiquiditySourceType,
    ) -> Result<bool, DispatchError> {
        Ok(
            Self::list_enabled_sources_for_trading_pair(dex_id, base_asset_id, target_asset_id)?
                .contains(&source_type),
        )
    }

    pub fn enable_source_for_trading_pair(
        dex_id: &T::DEXId,
        &base_asset_id: &T::AssetId,
        &target_asset_id: &T::AssetId,
        source_type: LiquiditySourceType,
    ) -> DispatchResult {
        Self::ensure_trading_pair_exists(dex_id, &base_asset_id, &target_asset_id)?;
        let pair = TradingPair::<T> {
            base_asset_id,
            target_asset_id,
        };
        // This logic considers Ok if source is already enabled.
        // unwrap() is safe, check done in `ensure_trading_pair_exists`.
        EnabledSources::<T>::mutate(dex_id, &pair, |opt_set| {
            opt_set.as_mut().unwrap().insert(source_type)
        });
        Ok(())
    }
}
