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
    decl_error, decl_event, decl_module, decl_storage, RuntimeDebug,
};
use sp_core::hash::H512;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub const TRANSFER: u32 = 1;
pub const EXCHANGE: u32 = 2;
pub const MINT: u32 = 3;
pub const BURN: u32 = 4;
pub const SLASH: u32 = 5;
pub const INIT_DEX: u32 = 6;
pub const MANAGE_DEX: u32 = 7;

/// Permission container with parameters and information about it's owner.
#[derive(PartialEq, Eq, Clone, Default, Encode, Decode, RuntimeDebug)]
pub struct Permission<T: frame_system::Trait> {
    owner_id: T::AccountId,
    params: H512,
}

impl<T: Trait> Permission<T> {
    pub fn new(owner_id: T::AccountId) -> Self {
        Self {
            owner_id,
            params: H512::zero(),
        }
    }

    pub fn with_parameters(owner_id: T::AccountId, params: H512) -> Self {
        Self { owner_id, params }
    }
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
                              .map(|(permission_id, holder_id, owner_id, params)| (permission_id, holder_id, Permission::<T> {
                                  owner_id,
                                  params: params.unwrap_or(H512::zero()),
                              })).collect::<Vec<_>>()
                             ): double_map hasher(opaque_blake2_256) u32, hasher(opaque_blake2_256) T::AccountId => Option<Permission<T>>;
    }

    add_extra_genesis {
        config(initial_permissions): Vec<(u32, T::AccountId, T::AccountId, Option<H512>)>;
    }
}

decl_event!(
    pub enum Event<T>
    where
        AccountId = <T as frame_system::Trait>::AccountId,
    {
        /// Permission was granted to a holder. [permission, who]
        PermissionGranted(u32, AccountId),
        /// Permission was transfered to a new owner. [permission, who]
        PermissionTransfered(u32, AccountId),
        /// Permission was created with an owner. [permission, who]
        PermissionCreated(u32, AccountId),
    }
);

decl_error! {
    /// Errors related to Permissions pallet.
    pub enum Error for Module<T: Trait> {
        /// Account doesn't hold a permission.
        PermissionNotFound,
        /// Account doesn't own a permission.
        PermissionNotOwned,
        /// Permission already exists in the system.
        PermissionAlreadyExists,
    }
}

/// Permissions module declaration.
impl<T: Trait> Module<T> {
    /// Method checks a permission of an Account.
    pub fn check_permission(who: T::AccountId, permission_id: u32) -> Result<(), Error<T>> {
        if Permissions::<T>::get(permission_id, &who).is_some() {
            Ok(())
        } else {
            Err(Error::<T>::PermissionNotFound)
        }
    }

    /// Method checks a permission with defined parameters of an Account.
    pub fn check_permission_with_parameters(
        who: T::AccountId,
        permission_id: u32,
        parameters: H512,
    ) -> Result<(), Error<T>> {
        let permission =
            Permissions::<T>::get(permission_id, &who).ok_or(Error::<T>::PermissionNotFound)?;
        if permission.params == parameters {
            Ok(())
        } else {
            Err(Error::<T>::PermissionNotFound)
        }
    }

    /// Method grants a permission to an Account.
    pub fn grant_permission(
        who: T::AccountId,
        account_id: T::AccountId,
        permission_id: u32,
    ) -> Result<(), Error<T>> {
        let permission =
            Permissions::<T>::get(permission_id, &who).ok_or(Error::<T>::PermissionNotFound)?;
        if permission.owner_id == who {
            Permissions::insert(permission_id, account_id.clone(), permission);
            Self::deposit_event(RawEvent::PermissionGranted(permission_id, account_id));
            Ok(())
        } else {
            Err(Error::<T>::PermissionNotOwned)
        }
    }

    /// Method grants a permission with defined parameters to an Account.
    pub fn grant_permission_with_parameters(
        who: T::AccountId,
        account_id: T::AccountId,
        permission_id: u32,
        parameters: H512,
    ) -> Result<(), Error<T>> {
        let permission =
            Permissions::<T>::get(permission_id, &who).ok_or(Error::<T>::PermissionNotFound)?;
        if permission.params == parameters {
            if permission.owner_id == who {
                Permissions::insert(permission_id, account_id.clone(), permission);
                Self::deposit_event(RawEvent::PermissionGranted(permission_id, account_id));
                Ok(())
            } else {
                Err(Error::<T>::PermissionNotOwned)
            }
        } else {
            Err(Error::<T>::PermissionNotFound)
        }
    }

    /// Method transfers a permission from owner to another Account.
    pub fn transfer_permission(
        who: T::AccountId,
        account_id: T::AccountId,
        permission_id: u32,
    ) -> Result<(), Error<T>> {
        let permission =
            Permissions::<T>::get(permission_id, &who).ok_or(Error::<T>::PermissionNotFound)?;
        if permission.owner_id == who {
            Permissions::insert(permission_id, account_id.clone(), permission);
            Permissions::<T>::remove(permission_id, who);
            Self::deposit_event(RawEvent::PermissionTransfered(permission_id, account_id));
            Ok(())
        } else {
            Err(Error::<T>::PermissionNotOwned)
        }
    }

    /// Method creates a permission from scratch.
    pub fn create_permission(
        _who: T::AccountId,
        account_id: T::AccountId,
        permission_id: u32,
        permission: Permission<T>,
    ) -> Result<(), Error<T>> {
        if Permissions::<T>::get(permission_id, &account_id).is_some() {
            Err(Error::<T>::PermissionAlreadyExists)
        } else {
            Permissions::insert(permission_id, account_id.clone(), permission);
            Self::deposit_event(RawEvent::PermissionCreated(permission_id, account_id));
            Ok(())
        }
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        type Error = Error<T>;
        fn deposit_event() = default;
    }
}
