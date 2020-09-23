#![cfg_attr(not(feature = "std"), no_std)]

use assets::AssetIdOf;
use common::{hash, in_basis_points_range, prelude::EnsureDEXOwner, BasisPoints};
use frame_support::{
    decl_error, decl_event, decl_module, decl_storage, dispatch::DispatchResult, ensure,
    sp_runtime::DispatchError, traits::Get, IterableStorageMap,
};
use frame_system::{self as system, ensure_signed, RawOrigin};
use permissions::{INIT_DEX, MANAGE_DEX};
use sp_std::vec::Vec;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

type DEXInfo<T> = common::prelude::DEXInfo<AssetIdOf<T>>;

pub trait Trait: common::Trait + assets::Trait {
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
    type GetDefaultFee: Get<u16>;
    type GetDefaultProtocolFee: Get<u16>;
}

decl_storage! {
    trait Store for Module<T: Trait> as DEXManager {
        // TODO: compare performance with separate tables
        pub DEXInfos get(fn dex_id): map hasher(twox_64_concat) T::DEXId => DEXInfo<T>;
    }
    add_extra_genesis {
        config(dex_list): Vec<(T::DEXId, DEXInfo<T>)>;

        build(|config: &GenesisConfig<T>| {
            config.dex_list.iter().for_each(|(dex_id, dex_info)| {
                <DEXInfos<T>>::insert(dex_id.clone(), dex_info);
            })
        })
    }
}

decl_event!(
    pub enum Event<T>
    where
        DEXId = <T as common::Trait>::DEXId,
    {
        DEXInitialized(DEXId),
        FeeChanged(DEXId, u16),
        ProtocolFeeChanged(DEXId, u16),
    }
);

decl_error! {
    pub enum Error for Module<T: Trait> {
        /// DEX with given id is already registered.
        DEXIdAlreadyExists,
        /// DEX with given Id is not registered.
        DEXDoesNotExist,
        /// Numeric value provided as fee is not valid, e.g. out of basis-point range.
        InvalidFeeValue,
        /// Account with given Id is not registered.
        InvalidAccountId,
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call
    where
        origin: T::Origin,
    {
        type Error = Error<T>;

        fn deposit_event() = default;

        /// Initialize DEX in network with given Id, Base Asset, if fees are not given then defaults are applied.
        ///
        /// - `dex_id`: ID of the exchange.
        /// - `fee`: value of fee on swaps in basis points.
        /// - `protocol_fee`: value of fee fraction for protocol beneficiary in basis points.
        ///
        /// TODO: add information about weight
        #[weight = 0]
        pub fn initialize_dex(origin, dex_id: T::DEXId, base_asset_id: T::AssetId, owner_account_id: T::AccountId, fee: Option<u16>, protocol_fee: Option<u16>) -> DispatchResult {
            let who = ensure_signed(origin)?;
            permissions::Module::<T>::check_permission(who.clone(), INIT_DEX)?;
            ensure!(!<DEXInfos<T>>::contains_key(&dex_id), <Error<T>>::DEXIdAlreadyExists);
            let fee = match fee {
                Some(val) => val,
                None => T::GetDefaultFee::get(),
            };
            let protocol_fee = match protocol_fee {
                Some(val) => val,
                None => T::GetDefaultProtocolFee::get(),
            };
            let new_dex_info = DEXInfo::<T> {
                base_asset_id,
                default_fee: fee,
                default_protocol_fee: protocol_fee,
            };
            <DEXInfos<T>>::insert(dex_id.clone(), new_dex_info);
            let permission = permissions::Permission::<T>::with_parameters(
                owner_account_id.clone(),
                hash(&dex_id),
            );
            permissions::Module::<T>::create_permission(
                owner_account_id.clone(),
                owner_account_id.clone(),
                MANAGE_DEX,
                permission.clone(),
            )?;
            Self::deposit_event(RawEvent::DEXInitialized(dex_id));
            Ok(())
        }

        /// Set fee deduced from tokens during swaps.
        ///
        /// - `dex_id`: ID of the exchange.
        /// - `fee`: value of fee on swaps in basis points.
        ///
        /// TODO: add information about weight
        #[weight = 0]
        pub fn set_fee(origin, dex_id: T::DEXId, fee: BasisPoints) -> DispatchResult {
            let _who = Self::ensure_dex_owner(&dex_id, origin)?;
            if !in_basis_points_range(fee) {
                return Err(<Error<T>>::InvalidFeeValue.into());
            }
            <DEXInfos<T>>::mutate(&dex_id, |dex_info| dex_info.default_fee = fee);
            Self::deposit_event(RawEvent::FeeChanged(dex_id, fee));
            Ok(())
        }

        /// Set fee deduced from swaps fee for protocol beneficiary.
        ///
        /// - `dex_id`: ID of the exchange.
        /// - `protocol_fee`: value of fee fraction for protocol beneficiary in basis points.
        ///
        /// TODO: add information about weight
        #[weight = 0]
        pub fn set_protocol_fee(origin, dex_id: T::DEXId, protocol_fee: BasisPoints) -> DispatchResult {
            let _who = Self::ensure_dex_owner(&dex_id, origin)?;
            if !in_basis_points_range(protocol_fee) {
                return Err(<Error<T>>::InvalidFeeValue.into());
            }
            <DEXInfos<T>>::mutate(&dex_id, |dex_info| dex_info.default_protocol_fee = protocol_fee);
            Self::deposit_event(RawEvent::ProtocolFeeChanged(dex_id, protocol_fee));
            Ok(())
        }
    }
}

impl<T: Trait> EnsureDEXOwner<T::DEXId, T::AccountId, DispatchError> for Module<T> {
    fn ensure_dex_owner<OuterOrigin>(
        dex_id: &T::DEXId,
        origin: OuterOrigin,
    ) -> Result<Option<T::AccountId>, DispatchError>
    where
        OuterOrigin: Into<Result<RawOrigin<T::AccountId>, OuterOrigin>>,
    {
        match origin.into() {
            Ok(RawOrigin::Signed(who)) => {
                ensure!(
                    <DEXInfos<T>>::contains_key(&dex_id),
                    <Error<T>>::DEXDoesNotExist
                );
                permissions::Module::<T>::check_permission_with_parameters(
                    who.clone(),
                    MANAGE_DEX,
                    hash(&dex_id),
                )?;
                Ok(Some(who))
            }
            Ok(RawOrigin::Root) => Ok(None),
            _ => Err(<Error<T>>::InvalidAccountId.into()),
        }
    }
}

impl<T: Trait> Module<T> {
    pub fn list_dex_ids() -> Vec<T::DEXId> {
        <DEXInfos<T>>::iter().map(|(k, _)| k.clone()).collect()
    }
}
