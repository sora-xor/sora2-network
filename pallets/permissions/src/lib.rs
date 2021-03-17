//! Permissions pallet provides an ability to configure an access via permissions.

#![warn(
    anonymous_parameters,
    rust_2018_idioms,
    trivial_casts,
    trivial_numeric_casts,
    unused,
    // The macro construct_runtime! (in mock.rs) expands to other macros and they have trailing semicolon and the compiler doesn't like it
    //future_incompatible,
    nonstandard_style,
    unsafe_code,
    unused_import_braces,
    unused_results,
    variant_size_differences
)]
#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
    codec::{Decode, Encode},
    ensure, RuntimeDebug,
};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_core::hash::H512;
use sp_std::vec::Vec;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

/// The id of the account owning a permission
pub type OwnerId<T> = <T as frame_system::Config>::AccountId;
/// The id of the account having a permission
pub type HolderId<T> = <T as frame_system::Config>::AccountId;
pub type PermissionId = u32;
type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

#[derive(PartialEq, Eq, Clone, Copy, RuntimeDebug, Encode, Decode)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum Scope {
    Limited(H512),
    Unlimited,
}

#[derive(PartialEq, Eq, Clone, Copy, RuntimeDebug, Encode, Decode)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[repr(u8)]
pub enum Mode {
    // The action associated with the permission is permitted if the account has the permission, otherwise it's forbidden
    Permit,
    // The action associated with the permission is forbidden if the account has the permission, otherwise it's permitted
    Forbid,
}

pub const TRANSFER: PermissionId = 1;
pub const MINT: PermissionId = 2;
pub const BURN: PermissionId = 3;
pub const SLASH: PermissionId = 4;
pub const INIT_DEX: PermissionId = 5;
pub const MANAGE_DEX: PermissionId = 6;
pub const CREATE_FARM: PermissionId = 7;
pub const CHECK_FARM: PermissionId = 8;
pub const LOCK_TO_FARM: PermissionId = 9;
pub const UNLOCK_FROM_FARM: PermissionId = 13;
pub const CLAIM_FROM_FARM: PermissionId = 10;
pub const GET_FARM_INFO: PermissionId = 11;
pub const GET_FARMER_INFO: PermissionId = 12;

/// Permissions module declaration.
impl<T: Config> Pallet<T> {
    /// Method checks a permission of an Account.
    pub fn check_permission(who: HolderId<T>, permission_id: PermissionId) -> Result<(), Error<T>> {
        Self::check_permission_with_scope(who, permission_id, &Scope::Unlimited)
    }

    /// Method checks a permission with defined scope of an Account.
    pub fn check_permission_with_scope(
        who: HolderId<T>,
        permission_id: PermissionId,
        scope: &Scope,
    ) -> Result<(), Error<T>> {
        ensure!(
            Modes::<T>::contains_key(permission_id),
            Error::PermissionNotFound
        );
        let mode = Modes::<T>::get(permission_id);
        let mut permission_found = Self::account_has_permission(&who, scope, permission_id);
        if !permission_found && *scope != Scope::Unlimited {
            permission_found = Self::account_has_permission(&who, &Scope::Unlimited, permission_id);
        }
        match (mode, permission_found) {
            (Mode::Permit, true) | (Mode::Forbid, false) => Ok(()),
            _ => Err(Error::Forbidden),
        }
    }

    /// Method grants a permission to an Account.
    pub fn grant_permission(
        who: OwnerId<T>,
        account_id: HolderId<T>,
        permission_id: PermissionId,
    ) -> Result<(), Error<T>> {
        Self::grant_permission_with_scope(who, account_id, permission_id, Scope::Unlimited)
    }

    /// Method grants a permission with defined scope to an Account.
    pub fn grant_permission_with_scope(
        who: OwnerId<T>,
        account_id: HolderId<T>,
        permission_id: PermissionId,
        scope: Scope,
    ) -> Result<(), Error<T>> {
        let (permission_found, owns_permission) = {
            let owners = Owners::<T>::get(permission_id, &scope);
            if owners.contains(&who) {
                (true, true)
            } else if scope != Scope::Unlimited {
                let owners = Owners::<T>::get(permission_id, Scope::Unlimited);
                (!owners.is_empty(), owners.contains(&who))
            } else {
                (!owners.is_empty(), false)
            }
        };
        if owns_permission {
            Permissions::<T>::mutate(&account_id, &scope, |permissions| {
                if let Err(index) = permissions.binary_search(&permission_id) {
                    permissions.insert(index, permission_id);
                }
            });
            Self::deposit_event(Event::<T>::PermissionGranted(permission_id, account_id));
            Ok(())
        } else if permission_found {
            Err(Error::PermissionNotOwned)
        } else {
            Err(Error::PermissionNotFound)
        }
    }

    /// Method transfers a permission from owner to another Account.
    pub fn transfer_permission(
        who: OwnerId<T>,
        account_id: HolderId<T>,
        permission_id: PermissionId,
        scope: Scope,
    ) -> Result<(), Error<T>> {
        ensure!(
            Modes::<T>::contains_key(&permission_id),
            Error::PermissionNotFound
        );
        Owners::<T>::mutate(permission_id, scope, |owners| {
            if let Some(pos) = owners.iter().position(|o| o == &who) {
                owners[pos] = account_id.clone();
                Ok(())
            } else if owners.is_empty() {
                Err(Error::PermissionNotFound)
            } else {
                Err(Error::PermissionNotOwned)
            }
        })?;
        Self::deposit_event(Event::<T>::PermissionTransfered(permission_id, account_id));
        Ok(())
    }

    /// Method creates a permission from scratch.
    pub fn create_permission(
        owner: OwnerId<T>,
        account_id: HolderId<T>,
        permission_id: PermissionId,
        scope: Scope,
        mode: Mode,
    ) -> Result<(), Error<T>> {
        ensure!(
            !Modes::<T>::contains_key(permission_id),
            Error::PermissionAlreadyExists
        );
        Modes::<T>::insert(permission_id, mode);
        Owners::<T>::mutate(permission_id, scope, |owners| {
            owners.push(owner);
        });
        Permissions::<T>::mutate(&account_id, scope, |permissions| {
            if let Err(index) = permissions.binary_search(&permission_id) {
                permissions.insert(index, permission_id);
            }
        });
        Self::deposit_event(Event::<T>::PermissionCreated(permission_id, account_id));
        Ok(())
    }

    /// Makes `owner` be the owner of `permission_id` in `scope`.
    /// Also, if the permission, that `permission_id` represents, has mode `Mode::Permit`, adds the permission to `holder_id`
    pub fn assign_permission(
        owner: OwnerId<T>,
        holder_id: &HolderId<T>,
        permission_id: PermissionId,
        scope: Scope,
    ) -> Result<(), Error<T>> {
        ensure!(
            Modes::<T>::contains_key(permission_id),
            Error::PermissionNotFound
        );
        let made_owner = Owners::<T>::mutate(permission_id, scope, |owners| {
            if !owners.contains(&owner) {
                owners.push(owner);
                true
            } else {
                false
            }
        });
        let granted_permission = if let Mode::Permit = Modes::<T>::get(permission_id) {
            Permissions::<T>::mutate(&holder_id, scope, |permissions| {
                if let Err(index) = permissions.binary_search(&permission_id) {
                    permissions.insert(index, permission_id);
                    true
                } else {
                    false
                }
            })
        } else {
            false
        };
        if made_owner || granted_permission {
            Ok(())
        } else {
            Err(Error::PermissionAlreadyExists)
        }
    }

    fn account_has_permission(
        holder_id: &HolderId<T>,
        scope: &Scope,
        permission_id: PermissionId,
    ) -> bool {
        let permissions = Permissions::<T>::get(holder_id, scope);
        permissions.binary_search(&permission_id).is_ok()
    }
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Permissions pallet's events.
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {}

    #[pallet::event]
    #[pallet::metadata(AccountIdOf<T> = "AccountId")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Permission was granted to a holder. [permission, who]
        PermissionGranted(u32, AccountIdOf<T>),
        /// Permission was transfered to a new owner. [permission, who]
        PermissionTransfered(u32, AccountIdOf<T>),
        /// Permission was created with an owner. [permission, who]
        PermissionCreated(u32, AccountIdOf<T>),
        /// Permission was assigned to the account in the scope. [permission, who]
        PermissionAssigned(u32, AccountIdOf<T>),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Account doesn't hold a permission.
        PermissionNotFound,
        /// Account doesn't own a permission.
        PermissionNotOwned,
        /// Permission already exists in the system.
        PermissionAlreadyExists,
        /// The account either doesn't have the permission or has the restriction.
        Forbidden,
    }

    #[pallet::storage]
    pub type Owners<T: Config> = StorageDoubleMap<
        _,
        Blake2_256,
        PermissionId,
        Blake2_256,
        Scope,
        Vec<OwnerId<T>>,
        ValueQuery,
    >;

    #[pallet::type_value]
    pub fn DefaultForModes() -> Mode {
        Mode::Permit
    }

    #[pallet::storage]
    pub type Modes<T: Config> =
        StorageMap<_, Blake2_256, PermissionId, Mode, ValueQuery, DefaultForModes>;

    #[pallet::storage]
    pub type Permissions<T: Config> = StorageDoubleMap<
        _,
        Blake2_256,
        HolderId<T>,
        Blake2_256,
        Scope,
        Vec<PermissionId>,
        ValueQuery,
    >;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub initial_permission_owners: Vec<(PermissionId, Scope, Vec<OwnerId<T>>)>,
        pub initial_permissions: Vec<(HolderId<T>, Scope, Vec<PermissionId>)>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                initial_permission_owners: Default::default(),
                initial_permissions: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            self.initial_permission_owners
                .iter()
                .for_each(|(permission, scope, owners)| {
                    Owners::<T>::insert(permission, scope, owners);
                });

            [
                (TRANSFER, Mode::Forbid),
                (MINT, Mode::Permit),
                (BURN, Mode::Permit),
                (SLASH, Mode::Permit),
                (INIT_DEX, Mode::Permit),
                (MANAGE_DEX, Mode::Permit),
                (CREATE_FARM, Mode::Permit),
                (CHECK_FARM, Mode::Permit),
                (LOCK_TO_FARM, Mode::Permit),
                (UNLOCK_FROM_FARM, Mode::Permit),
                (CLAIM_FROM_FARM, Mode::Permit),
                (GET_FARM_INFO, Mode::Permit),
                (GET_FARMER_INFO, Mode::Permit),
            ]
            .iter()
            .for_each(|(permission, mode)| {
                Modes::<T>::insert(permission, mode);
            });

            self.initial_permissions
                .iter()
                .for_each(|(holder_id, scope, permissions)| {
                    let mut permissions = permissions.clone();
                    permissions.sort();
                    Permissions::<T>::insert(holder_id, scope, permissions);
                });
        }
    }
}
