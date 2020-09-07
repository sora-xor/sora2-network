#![cfg_attr(not(feature = "std"), no_std)]

use assets::AssetIdOf;
use common::in_basis_points_range;
use common::prelude::{AccountIdOf, EnsureDEXOwner};
use frame_support::sp_runtime::DispatchError;
use frame_support::{
    decl_error, decl_event, decl_module, decl_storage, dispatch::DispatchResult, ensure,
    traits::Get,
};
use frame_system::{self as system, ensure_signed, RawOrigin};

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

type DEXInfo<T> = common::prelude::DEXInfo<AccountIdOf<T>, AssetIdOf<T>>;

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
                <DEXInfos<T>>::insert(dex_id, dex_info);
            })
        })
    }
}

decl_event!(
    pub enum Event<T>
    where
        DEXId = <T as common::Trait>::DEXId,
        AccountId = <T as system::Trait>::AccountId,
    {
        DEXInitialized(DEXId),
        FeeChanged(DEXId, u16),
        ProtocolFeeChanged(DEXId, u16),
        OwnerChanged(DEXId, AccountId),
    }
);

decl_error! {
    pub enum Error for Module<T: Trait> {
        DEXIdAlreadyExists,
        DEXDoesNotExist,
        InvalidFeeValue,
        InvalidAccountId,
        WrongOwnerAccountId,
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call
    where
        origin: T::Origin,
    {
        type Error = Error<T>;

        fn deposit_event() = default;

        /// Trading fee rate in basis points
        const GetDefaultFee: u16 = T::GetDefaultFee::get();

        /// Protocol fee rate in basis points
        const GetDefaultProtocolFee: u16 = T::GetDefaultProtocolFee::get();

        /// Initialize DEX in network with given Id, Base Asset, if fees are not given then defaults are applied.
        ///
        /// - `dex_id`: ID of the exchange.
        /// - `fee`: value of fee on swaps in basis points.
        /// - `protocol_fee`: value of fee fraction for protocol beneficiary in basis points.
        ///
        /// TODO: add information about weight
        #[weight = 0]
        pub fn initialize_dex(origin, dex_id: T::DEXId, base_asset_id: T::AssetId, fee: Option<u16>, protocol_fee: Option<u16>) -> DispatchResult {
            // TODO: check permissions, i.e. ability to init DEX
            let who = ensure_signed(origin)?;
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
                owner_account_id: who,
                base_asset_id,
                default_fee: fee,
                default_protocol_fee: protocol_fee,
            };
            <DEXInfos<T>>::insert(dex_id.clone(), new_dex_info);
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
        pub fn set_fee(origin, dex_id: T::DEXId, fee: u16) -> DispatchResult {
            let _who = T::ensure_dex_owner(&dex_id, origin)?;
            if !in_basis_points_range(fee) {
                return Err(<Error<T>>::InvalidFeeValue.into());
            }
            <DEXInfos<T>>::mutate(&dex_id, |dex_info| dex_info.default_fee = fee);
            Ok(())
        }

        /// Set fee deduced from swaps fee for protocol beneficiary.
        ///
        /// - `dex_id`: ID of the exchange.
        /// - `protocol_fee`: value of fee fraction for protocol beneficiary in basis points.
        ///
        /// TODO: add information about weight
        #[weight = 0]
        pub fn set_protocol_fee(origin, dex_id: T::DEXId, protocol_fee: u16) -> DispatchResult {
            let _who = T::ensure_dex_owner(&dex_id, origin)?;
            if !in_basis_points_range(protocol_fee) {
                return Err(<Error<T>>::InvalidFeeValue.into());
            }
            <DEXInfos<T>>::mutate(&dex_id, |dex_info| dex_info.default_protocol_fee = protocol_fee);
            Ok(())
        }

        /// Transfer ownership of DEX to indicated account,
        /// caller must be current owner, after execution caller will lose access to this DEX.
        ///
        /// - `dex_id`: ID of the exchange.
        /// - `fee`: value of fee on swaps in basis points.
        ///
        /// TODO: add information about weight
        #[weight = 0]
        pub fn transfer_ownership(origin, dex_id: T::DEXId, new_owner: T::AccountId) -> DispatchResult {
            let _who = T::ensure_dex_owner(&dex_id, origin)?;
            // FIXME: check account validity
            // ensure!(!<pallet_balances::Module<T>>::is_dead_account(&new_owner), <Error<T>>::InvalidAccountId);
            <DEXInfos<T>>::mutate(&dex_id, |dex_info| dex_info.owner_account_id = new_owner);
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
                let dex_info = <DEXInfos<T>>::get(&dex_id);
                ensure!(
                    dex_info.owner_account_id == who,
                    <Error<T>>::WrongOwnerAccountId
                );
                Ok(Some(who))
            }
            Ok(RawOrigin::Root) => Ok(None),
            _ => Err(<Error<T>>::InvalidAccountId.into()),
        }
    }
}
