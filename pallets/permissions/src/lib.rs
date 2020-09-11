//! Permissions pallet provides an ability to configure an access via permissions.

#![warn(
    anonymous_parameters,
    missing_copy_implementations,
    missing_debug_implementations,
    rust_2018_idioms,
    private_doc_tests,
    trivial_casts,
    trivial_numeric_casts,
    unused,
    future_incompatible,
    nonstandard_style,
    unsafe_code,
    unused_import_braces,
    unused_results,
    variant_size_differences
)]
#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
    codec::{Decode, Encode},
    decl_error, decl_event, decl_module, decl_storage, dispatch,
    traits::Get,
};
use frame_system::ensure_signed;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub const TRANSFER: u32 = 1;
pub const EXCHANGE: u32 = 2;
pub type PermissionId = u32;

/// Permission container with parameters and information about it's owner.
#[derive(PartialEq, Eq, Debug, Clone, Default, Encode, Decode)]
pub struct Permission<T: frame_system::Trait> {
    owner_id: T::AccountId,
    params: [u32; 32],
}

/// Pallet's configuration with parameters and types on which it depends.
pub trait Trait: frame_system::Trait {
    /// Permissions pallet's events.
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
}

decl_storage! {
    trait Store for Module<T: Trait> as PermissionsStoreModule {
        /// Storage with double keys (permission_id, holder_id).
        pub Permissions build(|config: &GenesisConfig<T>|
                              config.initial_permissions.iter()
                              .cloned()
                              .map(|(permission_id, holder_id, owner_id)| (permission_id, holder_id, Permission::<T> {
                                  owner_id,
                                  params: [0;32]
                              })).collect::<Vec<_>>()
                             ): double_map hasher(opaque_blake2_256) PermissionId, hasher(opaque_blake2_256) T::AccountId => Option<Permission<T>>;
    }

    add_extra_genesis {
        config(initial_permissions): Vec<(PermissionId, T::AccountId, T::AccountId)>;
    }
}

decl_event!(
    pub enum Event<T>
    where
        AccountId = <T as frame_system::Trait>::AccountId,
    {
        /// Permission was granted to a holder. [permission, who]
        PermissionGranted(PermissionId, AccountId),
        /// Permission was transfered to a new owner. [permission, who]
        PermissionTransfered(PermissionId, AccountId),
    }
);

decl_error! {
    /// Errors related to Permissions pallet.
    pub enum Error for Module<T: Trait> {
        /// Account doesn't hold a permission.
        PermissionNotFound,
        /// Account doesn't own a permission.
        PermissionNotOwned,
    }
}

decl_module! {
    /// Permissions module declaration.
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        type Error = Error<T>;

        fn deposit_event() = default;

        /// Dispatchable that checks a permission of an Account.
        #[weight = 10_000 + T::DbWeight::get().writes(1)]
        pub fn check_permission(origin, permission_id: PermissionId) -> dispatch::DispatchResult {
            let who = ensure_signed(origin)?;
            if Permissions::<T>::get(permission_id, &who).is_some() {
                Ok(())
            } else {
                Err(Error::<T>::PermissionNotFound)?
            }
        }

        /// Dispatchable that checks a permission of an Account with defined parameters.
        #[weight = 10_000 + T::DbWeight::get().writes(1)]
        pub fn check_permission_with_parameters(origin, permission_id: PermissionId, parameters: [u32; 32]) -> dispatch::DispatchResult {
            let who = ensure_signed(origin)?;
            let permission = Permissions::<T>::get(permission_id, &who).ok_or(Error::<T>::PermissionNotFound)?;
            if permission.params == parameters {
                Ok(())
            } else {
                Err(Error::<T>::PermissionNotFound)?
            }
        }

        /// Dispatchable that grants a permission to an Account.
        #[weight = 10_000 + T::DbWeight::get().reads_writes(1,1)]
        pub fn grant_permission(origin, account_id: T::AccountId, permission_id: PermissionId) -> dispatch::DispatchResult {
            let who = ensure_signed(origin)?;
            let permission = Permissions::<T>::get(permission_id, &who).ok_or(Error::<T>::PermissionNotFound)?;
            if permission.owner_id == who {
                Permissions::insert(permission_id, account_id.clone(), permission);
                Self::deposit_event(RawEvent::PermissionGranted(permission_id, account_id));
                Ok(())
            } else {
                Err(Error::<T>::PermissionNotOwned)?
            }
        }

        /// Dispatchable that transfers a permission from owner to another Account.
        #[weight = 10_000 + T::DbWeight::get().reads_writes(1,1)]
        pub fn transfer_permission(origin, account_id: T::AccountId, permission_id: PermissionId) -> dispatch::DispatchResult {
            let who = ensure_signed(origin)?;
            let permission = Permissions::<T>::get(permission_id, &who).ok_or(Error::<T>::PermissionNotFound)?;
            if permission.owner_id == who {
                Permissions::insert(permission_id, account_id.clone(), permission);
                Permissions::<T>::remove(permission_id, who);
                Self::deposit_event(RawEvent::PermissionTransfered(permission_id, account_id));
                Ok(())
            } else {
                Err(Error::<T>::PermissionNotOwned)?
            }
        }
    }
}
