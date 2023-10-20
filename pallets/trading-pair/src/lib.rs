// This file is part of the SORA network and Polkaswap app.

// Copyright (c) 2020, 2021, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

// Redistribution and use in source and binary forms, with or without modification,
// are permitted provided that the following conditions are met:

// Redistributions of source code must retain the above copyright notice, this list
// of conditions and the following disclaimer.
// Redistributions in binary form must reproduce the above copyright notice, this
// list of conditions and the following disclaimer in the documentation and/or other
// materials provided with the distribution.
//
// All advertising materials mentioning features or use of this software must display
// the following acknowledgement: This product includes software developed by Polka Biome
// Ltd., SORA, and Polkaswap.
//
// Neither the name of the Polka Biome Ltd. nor the names of its contributors may be used
// to endorse or promote products derived from this software without specific prior written permission.

// THIS SOFTWARE IS PROVIDED BY Polka Biome Ltd. AS IS AND ANY EXPRESS OR IMPLIED WARRANTIES,
// INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL Polka Biome Ltd. BE LIABLE FOR ANY
// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
// BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS;
// OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
// USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

#![cfg_attr(not(feature = "std"), no_std)]
// TODO #167: fix clippy warnings
#![allow(clippy::all)]

#[allow(unused_imports)]
#[macro_use]
extern crate alloc;

use common::{
    AssetInfoProvider, DexInfoProvider, EnabledSourcesManager, EnsureDEXManager,
    EnsureTradingPairExists, LiquiditySourceType, LockedLiquiditySourcesManager, ManagementMode,
    RegisterManager, TradingPairSourceManager,
};
use frame_support::dispatch::{DispatchError, DispatchResult};
use frame_support::ensure;
use frame_support::pallet_prelude::DispatchResultWithPostInfo;
use frame_support::traits::IsType;
use sp_std::collections::btree_set::BTreeSet;
use sp_std::vec::Vec;

mod weights;

mod benchmarking;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub type TradingPair<T> = common::prelude::TradingPair<<T as assets::Config>::AssetId>;
type Assets<T> = assets::Pallet<T>;

pub use weights::WeightInfo;

impl<T: Config> EnsureTradingPairExists<T::DEXId, T::AssetId, DispatchError> for Pallet<T> {
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

impl<T: Config> LockedLiquiditySourcesManager<LiquiditySourceType> for Pallet<T> {
    fn get() -> Vec<LiquiditySourceType> {
        LockedLiquiditySources::<T>::get()
    }
    fn set(liquidity_source_types: Vec<LiquiditySourceType>) -> () {
        LockedLiquiditySources::<T>::set(liquidity_source_types)
    }
    fn append(liquidity_source_type: LiquiditySourceType) -> () {
        LockedLiquiditySources::<T>::append(liquidity_source_type)
    }
}

impl<T: Config> EnabledSourcesManager<T::DEXId, T::AssetId> for Pallet<T> {
    fn mutate_remove(
        dex_id: &T::DEXId,
        base_asset_id: &T::AssetId,
        target_asset_id: &T::AssetId,
    ) -> () {
        let pair = TradingPair::<T> {
            base_asset_id: base_asset_id.clone(),
            target_asset_id: target_asset_id.clone(),
        };
        EnabledSources::<T>::mutate(&dex_id, &pair, |opt_set| {
            if let Some(sources) = opt_set.as_mut() {
                sources.remove(&LiquiditySourceType::XYKPool);
            }
        })
    }
}
impl<T: Config> RegisterManager<T::DEXId, T::AssetId, <T as frame_system::Config>::RuntimeOrigin>
    for Pallet<T>
{
    fn register(
        origin: <T as frame_system::Config>::RuntimeOrigin,
        dex_id: T::DEXId,
        base_asset_id: T::AssetId,
        target_asset_id: T::AssetId,
    ) -> DispatchResultWithPostInfo {
        Self::register(origin.clone(), dex_id, base_asset_id, target_asset_id)
    }
}

impl<T: Config> TradingPairSourceManager<T::DEXId, T::AssetId> for Pallet<T> {
    fn list_enabled_sources_for_trading_pair(
        dex_id: &T::DEXId,
        &base_asset_id: &T::AssetId,
        &target_asset_id: &T::AssetId,
    ) -> Result<BTreeSet<LiquiditySourceType>, DispatchError> {
        T::DexInfoProvider::ensure_dex_exists(dex_id)?;
        let pair = TradingPair::<T> {
            base_asset_id,
            target_asset_id,
        };
        let mut sources =
            Self::enabled_sources(dex_id, &pair).ok_or(Error::<T>::TradingPairDoesntExist)?;
        let locked = LockedLiquiditySources::<T>::get();
        for locked_source in &locked {
            sources.remove(&locked_source);
        }
        Ok(sources)
    }

    fn is_source_enabled_for_trading_pair(
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

    fn enable_source_for_trading_pair(
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

    fn disable_source_for_trading_pair(
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
            opt_set.as_mut().unwrap().remove(&source_type)
        });
        Ok(())
    }
}

impl<T: Config> Pallet<T> {
    pub fn register_pair(
        dex_id: T::DEXId,
        base_asset_id: T::AssetId,
        target_asset_id: T::AssetId,
    ) -> Result<(), DispatchError> {
        ensure!(
            base_asset_id != target_asset_id,
            Error::<T>::IdenticalAssetIds
        );

        let dex_info = T::DexInfoProvider::get_dex_info(&dex_id)?;
        ensure!(
            base_asset_id == dex_info.base_asset_id
                || base_asset_id == dex_info.synthetic_base_asset_id,
            Error::<T>::ForbiddenBaseAssetId
        );
        Assets::<T>::ensure_asset_exists(&base_asset_id)?;
        Assets::<T>::ensure_asset_exists(&target_asset_id)?;

        let trading_pair = TradingPair::<T> {
            base_asset_id,
            target_asset_id,
        };
        ensure!(
            Self::enabled_sources(&dex_id, &trading_pair).is_none(),
            Error::<T>::TradingPairExists
        );
        EnabledSources::<T>::insert(
            &dex_id,
            &trading_pair,
            BTreeSet::<LiquiditySourceType>::new(),
        );
        Self::deposit_event(Event::TradingPairStored(dex_id, trading_pair));
        Ok(().into())
    }

    pub fn list_trading_pairs(dex_id: &T::DEXId) -> Result<Vec<TradingPair<T>>, DispatchError> {
        T::DexInfoProvider::ensure_dex_exists(dex_id)?;
        Ok(EnabledSources::<T>::iter_prefix(dex_id)
            .map(|(pair, _)| pair)
            .collect())
    }

    pub fn is_trading_pair_enabled(
        dex_id: &T::DEXId,
        &base_asset_id: &T::AssetId,
        &target_asset_id: &T::AssetId,
    ) -> Result<bool, DispatchError> {
        T::DexInfoProvider::ensure_dex_exists(dex_id)?;
        let pair = TradingPair::<T> {
            base_asset_id,
            target_asset_id,
        };
        Ok(Self::enabled_sources(dex_id, &pair).is_some())
    }
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use common::{DEXInfo, DexIdOf};
    use frame_support::pallet_prelude::*;
    use frame_support::traits::StorageVersion;
    use frame_system::pallet_prelude::*;

    // TODO: #395 use AssetInfoProvider instead of assets pallet
    #[pallet::config]
    pub trait Config: frame_system::Config + common::Config + assets::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type EnsureDEXManager: EnsureDEXManager<Self::DEXId, Self::AccountId, DispatchError>;
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
        type DexInfoProvider: DexInfoProvider<Self::DEXId, DEXInfo<Self::AssetId>>;
    }

    /// The current storage version.
    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Register trading pair on the given DEX.
        /// Can be only called by the DEX owner.
        ///
        /// - `dex_id`: ID of the exchange.
        /// - `base_asset_id`: base asset ID.
        /// - `target_asset_id`: target asset ID.
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::register())]
        pub fn register(
            origin: OriginFor<T>,
            dex_id: T::DEXId,
            base_asset_id: T::AssetId,
            target_asset_id: T::AssetId,
        ) -> DispatchResultWithPostInfo {
            let _author =
                T::EnsureDEXManager::ensure_can_manage(&dex_id, origin, ManagementMode::Public)?;
            Self::register_pair(dex_id, base_asset_id, target_asset_id)?;
            Ok(().into())
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Trading pair has been redistered on a DEX. [DEX Id, Trading Pair]
        TradingPairStored(DexIdOf<T>, TradingPair<T>),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Registering trading pair already exists.
        TradingPairExists,
        /// The specified base asset ID for the trading pair is not allowed.
        ForbiddenBaseAssetId,
        /// The specified base asset ID is the same as target asset ID.
        IdenticalAssetIds,
        /// Trading pair is not registered for given DEXId.
        TradingPairDoesntExist,
    }

    #[pallet::storage]
    #[pallet::getter(fn enabled_sources)]
    pub type EnabledSources<T: Config> = StorageDoubleMap<
        _,
        Twox64Concat,
        T::DEXId,
        Blake2_128Concat,
        TradingPair<T>,
        BTreeSet<LiquiditySourceType>,
    >;

    #[pallet::storage]
    pub type LockedLiquiditySources<T: Config> =
        StorageValue<_, Vec<LiquiditySourceType>, ValueQuery>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub trading_pairs: Vec<(T::DEXId, TradingPair<T>)>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                trading_pairs: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            self.trading_pairs.iter().for_each(|(dex_id, pair)| {
                EnabledSources::<T>::insert(&dex_id, &pair, BTreeSet::<LiquiditySourceType>::new());
            })
        }
    }
}
