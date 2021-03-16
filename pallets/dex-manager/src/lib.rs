#![cfg_attr(not(feature = "std"), no_std)]

use assets::AssetIdOf;
use common::{hash, prelude::EnsureDEXManager, ManagementMode};
use frame_support::{dispatch::DispatchResult, ensure, sp_runtime::DispatchError, weights::Weight};
use frame_system::{ensure_signed, RawOrigin};
use permissions::{Scope, INIT_DEX, MANAGE_DEX};
use sp_std::vec::Vec;

mod weights;

mod benchmarking;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

type DEXInfo<T> = common::prelude::DEXInfo<AssetIdOf<T>>;

pub trait WeightInfo {
    fn initialize_dex() -> Weight;
}

impl<T: Config> EnsureDEXManager<T::DEXId, T::AccountId, DispatchError> for Module<T> {
    fn ensure_can_manage<OuterOrigin>(
        dex_id: &T::DEXId,
        origin: OuterOrigin,
        mode: ManagementMode,
    ) -> Result<Option<T::AccountId>, DispatchError>
    where
        OuterOrigin: Into<Result<RawOrigin<T::AccountId>, OuterOrigin>>,
    {
        match origin.into() {
            Ok(RawOrigin::Signed(who)) => {
                let dex_info = Self::get_dex_info(&dex_id)?;
                // If DEX is public, anyone can manage it, otherwise confirm ownership.
                if !dex_info.is_public || mode != ManagementMode::Public {
                    Self::ensure_direct_manager(&dex_id, &who)?;
                }
                Ok(Some(who))
            }
            _ => Err(Error::<T>::InvalidAccountId.into()),
        }
    }
}

impl<T: Config> Module<T> {
    pub fn get_dex_info(dex_id: &T::DEXId) -> Result<DEXInfo<T>, DispatchError> {
        Ok(DEXInfos::<T>::get(&dex_id).ok_or(Error::<T>::DEXDoesNotExist)?)
    }

    pub fn ensure_dex_exists(dex_id: &T::DEXId) -> DispatchResult {
        ensure!(
            DEXInfos::<T>::contains_key(&dex_id),
            Error::<T>::DEXDoesNotExist
        );
        Ok(())
    }

    pub fn list_dex_ids() -> Vec<T::DEXId> {
        DEXInfos::<T>::iter().map(|(k, _)| k).collect()
    }

    fn ensure_direct_manager(dex_id: &T::DEXId, who: &T::AccountId) -> DispatchResult {
        permissions::Module::<T>::check_permission_with_scope(
            who.clone(),
            MANAGE_DEX,
            &Scope::Limited(hash(&dex_id)),
        )
        .map_err(|e| e.into())
    }
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use common::DexIdOf;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + common::Config + assets::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Initialize DEX in network with given Id, Base Asset, if fees are not given then defaults are applied.
        ///
        /// - `dex_id`: ID of the exchange.
        /// - `fee`: value of fee on swaps in basis points.
        /// - `protocol_fee`: value of fee fraction for protocol beneficiary in basis points.
        #[pallet::weight(<T as Config>::WeightInfo::initialize_dex())]
        pub fn initialize_dex(
            origin: OriginFor<T>,
            dex_id: T::DEXId,
            base_asset_id: T::AssetId,
            owner_account_id: T::AccountId,
            is_public: bool,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            permissions::Module::<T>::check_permission(who.clone(), INIT_DEX)?;
            ensure!(
                !DEXInfos::<T>::contains_key(&dex_id),
                Error::<T>::DEXIdAlreadyExists
            );
            // Construct DEX information.
            let new_dex_info = DEXInfo::<T> {
                base_asset_id,
                is_public,
            };
            DEXInfos::<T>::insert(dex_id.clone(), new_dex_info);
            // Create permission for designated owner account.
            match permissions::Module::<T>::assign_permission(
                owner_account_id.clone(),
                &owner_account_id,
                MANAGE_DEX,
                Scope::Limited(hash(&dex_id)),
            ) {
                Err(permissions::Error::<T>::PermissionAlreadyExists) => {}
                result => result?,
            }
            Self::deposit_event(Event::DEXInitialized(dex_id));
            Ok(().into())
        }
    }

    #[pallet::event]
    #[pallet::metadata(DexIdOf<T> = "DEXId")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// New DEX has been registered. [DEX Id]
        DEXInitialized(DexIdOf<T>),
        /// Default fee setting has been changed. [DEX Id, Swap fee in basis points]
        FeeChanged(DexIdOf<T>, u16),
        /// Default protocol fee setting has been changed. [DEX Id, Protocol fee in basis points]
        ProtocolFeeChanged(DexIdOf<T>, u16),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// DEX with given id is already registered.
        DEXIdAlreadyExists,
        /// DEX with given Id is not registered.
        DEXDoesNotExist,
        /// Numeric value provided as fee is not valid, e.g. out of basis-point range.
        InvalidFeeValue,
        /// Account with given Id is not registered.
        InvalidAccountId,
    }

    #[pallet::storage]
    #[pallet::getter(fn dex_id)]
    pub type DEXInfos<T: Config> = StorageMap<_, Twox64Concat, T::DEXId, DEXInfo<T>>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub dex_list: Vec<(T::DEXId, DEXInfo<T>)>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                dex_list: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            self.dex_list.iter().for_each(|(dex_id, dex_info)| {
                DEXInfos::<T>::insert(dex_id.clone(), dex_info);
            })
        }
    }
}
