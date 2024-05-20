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
// TODO #167: fix clippy warnings
#![allow(clippy::all)]

use common::{hash, AssetRegulator, PermissionId};
use frame_support::codec::{Decode, Encode};
use frame_support::sp_runtime::DispatchError;
use frame_support::{ensure, RuntimeDebug};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_core::hash::H512;
use sp_std::vec::Vec;
#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub use common::prelude::{
    BURN, CHECK_FARM, CLAIM_FROM_FARM, CREATE_FARM, GET_FARMER_INFO, GET_FARM_INFO, INIT_DEX,
    LOCK_TO_FARM, MANAGE_DEX, MINT, SLASH, UNLOCK_FROM_FARM,
};
/// The id of the account owning a permission
pub type OwnerId<T> = <T as frame_system::Config>::AccountId;
/// The id of the account having a permission
pub type HolderId<T> = <T as frame_system::Config>::AccountId;
type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

#[derive(PartialEq, Eq, Clone, Copy, RuntimeDebug, Encode, Decode, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum Scope {
    Limited(H512),
    Unlimited,
}

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
        let mut permission_found = Self::account_has_permission(&who, scope, permission_id);
        if !permission_found && *scope != Scope::Unlimited {
            permission_found = Self::account_has_permission(&who, &Scope::Unlimited, permission_id);
        }
        ensure!(permission_found, Error::Forbidden);
        Ok(())
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
            if Permissions::<T>::iter_prefix_values(&account_id).count() == 0 {
                frame_system::Pallet::<T>::inc_consumers(&account_id)
                    .map_err(|_| Error::<T>::IncRefError)?;
            }
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
        Owners::<T>::mutate(permission_id, scope, |owners| {
            if let Some(pos) = owners.iter().position(|o| o == &who) {
                owners[pos] = account_id.clone();
            } else if owners.is_empty() {
                return Err(Error::PermissionNotFound);
            } else {
                return Err(Error::PermissionNotOwned);
            }
            frame_system::Pallet::<T>::dec_consumers(&who);
            frame_system::Pallet::<T>::inc_consumers(&account_id)
                .map_err(|_| Error::<T>::IncRefError)?;
            Ok(())
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
    ) -> Result<(), Error<T>> {
        if Permissions::<T>::iter_prefix_values(&account_id).count() == 0 {
            frame_system::Pallet::<T>::inc_consumers(&account_id)
                .map_err(|_| Error::<T>::IncRefError)?;
        }
        frame_system::Pallet::<T>::inc_consumers(&owner).map_err(|_| Error::<T>::IncRefError)?;
        Owners::<T>::mutate(permission_id, scope, |owners| {
            ensure!(owners.is_empty(), Error::<T>::PermissionAlreadyExists);
            owners.push(owner);
            Ok(())
        })?;
        Permissions::<T>::mutate(&account_id, scope, |permissions| {
            if let Err(index) = permissions.binary_search(&permission_id) {
                permissions.insert(index, permission_id);
            }
        });
        Self::deposit_event(Event::<T>::PermissionCreated(permission_id, account_id));
        Ok(())
    }

    /// Makes `owner` be the owner of `permission_id` in `scope`.
    /// Also, adds the permission to `holder_id`
    pub fn assign_permission(
        owner: OwnerId<T>,
        holder_id: &HolderId<T>,
        permission_id: PermissionId,
        scope: Scope,
    ) -> Result<(), Error<T>> {
        frame_system::Pallet::<T>::inc_consumers(&owner).map_err(|_| Error::<T>::IncRefError)?;
        let made_owner = Owners::<T>::mutate(permission_id, scope, |owners| {
            if !owners.contains(&owner) {
                owners.push(owner);
                true
            } else {
                false
            }
        });
        if Permissions::<T>::iter_prefix_values(&holder_id).count() == 0 {
            frame_system::Pallet::<T>::inc_consumers(&holder_id)
                .map_err(|_| Error::<T>::IncRefError)?;
        }
        let granted_permission = Permissions::<T>::mutate(&holder_id, scope, |permissions| {
            if let Err(index) = permissions.binary_search(&permission_id) {
                permissions.insert(index, permission_id);
                true
            } else {
                false
            }
        });
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
    use frame_support::traits::StorageVersion;
    use frame_system::pallet_prelude::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Permissions pallet's events.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
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
    impl<T: Config> Pallet<T> {}

    #[pallet::event]
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
        /// The account either doesn't have the permission.
        Forbidden,
        /// Increment account reference error.
        IncRefError,
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
                    for owner in owners {
                        frame_system::Pallet::<T>::inc_consumers(owner).unwrap();
                    }
                    Owners::<T>::insert(permission, scope, owners);
                });

            self.initial_permissions
                .iter()
                .for_each(|(holder_id, scope, permissions)| {
                    let mut permissions = permissions.clone();
                    permissions.sort();
                    frame_system::Pallet::<T>::inc_consumers(&holder_id).unwrap();
                    Permissions::<T>::insert(holder_id, scope, permissions);
                });
        }
    }
}

impl<T: Config, AssetId: Encode> AssetRegulator<AccountIdOf<T>, AssetId> for Pallet<T> {
    fn assign_permission(
        owner: &AccountIdOf<T>,
        asset_id: &AssetId,
        permission_id: &PermissionId,
    ) -> Result<(), DispatchError> {
        let scope = Scope::Limited(hash(asset_id));
        Self::assign_permission(owner.clone(), &owner, *permission_id, scope)?;
        Ok(())
    }

    fn check_permission(
        issuer: &AccountIdOf<T>,
        affected_account: &AccountIdOf<T>,
        asset_id: &AssetId,
        permission_id: &PermissionId,
    ) -> Result<(), DispatchError> {
        // Holders can burn their own assets.
        if permission_id == &BURN && affected_account == issuer {
            return Ok(());
        }
        Self::check_permission_with_scope(
            issuer.clone(),
            *permission_id,
            &Scope::Limited(hash(asset_id)),
        )
        .or_else(|_| {
            Self::check_permission_with_scope(issuer.clone(), *permission_id, &Scope::Unlimited)
        })?;
        Ok(())
    }
}
