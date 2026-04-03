// // This file is part of the SORA network and Polkaswap app.

// // Copyright (c) 2020, 2021, Polka Biome Ltd. All rights reserved.
// // SPDX-License-Identifier: BSD-4-Clause

// // Redistribution and use in source and binary forms, with or without modification,
// // are permitted provided that the following conditions are met:

// // Redistributions of source code must retain the above copyright notice, this list
// // of conditions and the following disclaimer.
// // Redistributions in binary form must reproduce the above copyright notice, this
// // list of conditions and the following disclaimer in the documentation and/or other
// // materials provided with the distribution.
// //
// // All advertising materials mentioning features or use of this software must display
// // the following acknowledgement: This product includes software developed by Polka Biome
// // Ltd., SORA, and Polkaswap.
// //
// // Neither the name of the Polka Biome Ltd. nor the names of its contributors may be used
// // to endorse or promote products derived from this software without specific prior written permission.

// // THIS SOFTWARE IS PROVIDED BY Polka Biome Ltd. AS IS AND ANY EXPRESS OR IMPLIED WARRANTIES,
// // INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// // A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL Polka Biome Ltd. BE LIABLE FOR ANY
// // DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
// // BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS;
// // OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// // STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
// // USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
#![cfg_attr(not(feature = "std"), no_std)]

use bridge_types::traits::TimepointProvider;
use bridge_types::types::MessageDirection;
use bridge_types::types::MessageStatus;
use bridge_types::GenericAccount;
use bridge_types::GenericNetworkId;
use bridge_types::GenericTimepoint;
use bridge_types::LiberlandAssetId;
use frame_support::fail;
use frame_support::pallet_prelude::*;
use frame_support::traits::fungibles::{
    metadata::Inspect as InspectMetadata, metadata::Mutate as MetadataMutate, Create, Inspect,
    Mutate,
};
use frame_support::traits::Currency;
use frame_support::traits::ExistenceRequirement;
pub use pallet::*;
use sp_core::H256;
use sp_io::hashing::blake2_256;
use sp_runtime::AccountId32;
use sp_std::prelude::*;

use frame_support::traits::tokens::{Fortitude, Precision, Preservation};

#[derive(Clone, RuntimeDebug, Encode, Decode, PartialEq, Eq, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct BridgeRequest<Balance> {
    source: GenericAccount,
    dest: GenericAccount,
    asset_id: LiberlandAssetId,
    amount: Balance,
    status: MessageStatus,
    start_timepoint: GenericTimepoint,
    end_timepoint: GenericTimepoint,
    direction: MessageDirection,
}

#[frame_support::pallet]
pub mod pallet {
    #![allow(missing_docs)]
    use crate::BridgeRequest;
    use bridge_types::traits::BridgeApp;
    use bridge_types::traits::TimepointProvider;
    use bridge_types::types::MessageStatus;
    use bridge_types::GenericAccount;
    use bridge_types::GenericNetworkId;
    use bridge_types::LiberlandAssetId;
    use frame_support::pallet_prelude::{ValueQuery, *};
    use frame_system::pallet_prelude::*;
    use sp_core::H256;
    use sp_runtime::traits::Convert;
    use sp_runtime::AccountId32;

    pub type AssetIdOf<T> = <T as Config>::AssetId;

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::storage]
    #[pallet::getter(fn asset_nonce)]
    pub(super) type AssetNonce<T: Config> = StorageValue<_, u32, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn transactions)]
    pub(super) type Transactions<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        (GenericNetworkId, GenericAccount),
        Blake2_128Concat,
        H256,
        BridgeRequest<T::Balance>,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn sender)]
    pub(super) type Senders<T: Config> = StorageDoubleMap<
        _,
        Twox64Concat,
        GenericNetworkId,
        Blake2_128Concat,
        H256,
        GenericAccount,
        OptionQuery,
    >;

    /// The module's configuration trait.
    #[pallet::config]
    #[pallet::disable_frame_system_supertrait_check]
    pub trait Config: frame_system::Config + pallet_assets::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        type MinBalance: Get<<Self as pallet_assets::Config>::Balance>;

        type AssetId: Member
            + Parameter
            + Copy
            + MaybeSerializeDeserialize
            + MaxEncodedLen
            + From<<Self as pallet_assets::Config>::AssetId>;

        type Balances: frame_support::traits::Currency<Self::AccountId>;

        type SoraApp: BridgeApp<Self::AccountId, GenericAccount, LiberlandAssetId, Self::Balance>;

        type AccountIdConverter: Convert<AccountId32, Self::AccountId>;

        type TimepointProvider: TimepointProvider;

        #[pallet::constant]
        type SoraMainnetTechAcc: Get<Self::AccountId>;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        AssetCreated(AssetIdOf<T>),
        RefundFailed(H256),
        RequestStatusUpdate(H256, MessageStatus),
        RefundInvoked(H256, T::Balance),
    }

    #[pallet::error]
    pub enum Error<T> {
        FailedToCreateAsset,
        NoTechAccFound,
        WrongAccount,
        WrongSidechainAsset,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    impl<T: Config> Pallet<T> {
        pub fn refund(
            network_id: GenericNetworkId,
            message_id: H256,
            generic_beneficiary: GenericAccount,
            asset_id: LiberlandAssetId,
            amount: T::Balance,
        ) -> DispatchResult {
            let GenericAccount::Liberland(beneficiary_liberland) = generic_beneficiary else {
                frame_support::fail!(Error::<T>::WrongAccount)
            };
            let beneficiary = T::AccountIdConverter::convert(beneficiary_liberland);
            if T::SoraApp::is_asset_supported(network_id, asset_id) {
                T::SoraApp::refund(network_id, message_id, beneficiary, asset_id, amount)?;
            }
            Self::deposit_event(Event::<T>::RefundInvoked(message_id, amount));
            Ok(())
        }
    }
}

impl<T: Config> bridge_types::traits::BridgeAssetRegistry<T::AccountId, LiberlandAssetId>
    for Pallet<T>
where
    <T as pallet_assets::Config>::AssetId: Into<u32> + From<u32>,
    AssetIdOf<T>: From<LiberlandAssetId>,
{
    type AssetName = Vec<u8>;
    type AssetSymbol = Vec<u8>;

    fn register_asset(
        _network_id: GenericNetworkId,
        name: <Self as bridge_types::traits::BridgeAssetRegistry<T::AccountId, LiberlandAssetId>>::AssetName,
        symbol: <Self as bridge_types::traits::BridgeAssetRegistry<
            T::AccountId,
            LiberlandAssetId,
        >>::AssetSymbol,
    ) -> Result<LiberlandAssetId, DispatchError> {
        let nonce = Self::asset_nonce();
        AssetNonce::<T>::set(nonce + 1);
        let tech_acc = T::SoraMainnetTechAcc::get();
        // let's take 3  itrations to create a new asset id, considering that collision can happen
        let iter = 3;
        for i in 0..iter {
            let hash = {
                let mut vector = name.clone();
                vector.extend_from_slice(&symbol);
                vector.extend_from_slice(&(nonce + i).encode());
                let hash = blake2_256(&vector);
                H256::from_slice(&hash)
            };
            let asset_id = {
                let arr: [u8; 4] = hash[..4].try_into().unwrap_or_default();
                u32::from_be_bytes(arr)
            };
            let res = <pallet_assets::Pallet<T> as Create<T::AccountId>>::create(
                asset_id.into(),
                tech_acc.clone(),
                true,
                T::MinBalance::get(),
            );
            if res.is_ok() {
                <pallet_assets::Pallet<T> as MetadataMutate<T::AccountId>>::set(
                    asset_id.into(),
                    &tech_acc,
                    name,
                    symbol,
                    18,
                )?;
                Self::deposit_event(Event::AssetCreated(
                    LiberlandAssetId::Asset(asset_id).into(),
                ));
                return Ok(LiberlandAssetId::Asset(asset_id));
            }
        }
        fail!(Error::<T>::FailedToCreateAsset)
    }

    fn manage_asset(_: GenericNetworkId, _: LiberlandAssetId) -> Result<(), DispatchError> {
        Ok(())
    }

    fn ensure_asset_exists(asset_id: LiberlandAssetId) -> bool {
        match asset_id {
            LiberlandAssetId::LLD => true,
            LiberlandAssetId::Asset(asset_id) => {
                <pallet_assets::Pallet<T> as Inspect<T::AccountId>>::asset_exists(asset_id.into())
            }
        }
    }

    fn get_raw_info(asset_id: LiberlandAssetId) -> bridge_types::types::RawAssetInfo {
        use frame_support::traits::fungibles::metadata::Inspect;

        match asset_id {
            LiberlandAssetId::LLD => bridge_types::types::RawAssetInfo {
                name: b"Liberland".to_vec(),
                symbol: b"LLD".to_vec(),
                precision: 12,
            },
            LiberlandAssetId::Asset(asset_id) => {
                let name = <pallet_assets::Pallet<T> as InspectMetadata<T::AccountId>>::name(
                    asset_id.into(),
                );
                let symbol = pallet_assets::Pallet::<T>::symbol(asset_id.into());
                let precision = pallet_assets::Pallet::<T>::decimals(asset_id.into());
                bridge_types::types::RawAssetInfo {
                    name,
                    symbol,
                    precision,
                }
            }
        }
    }
}

impl<T: Config> bridge_types::traits::BridgeAssetLocker<T::AccountId> for Pallet<T>
    where <T as pallet_assets::Config>::AssetId: Into<u32> + From<u32>,
    <T as pallet_assets::Config>::Balance: Into<<<T as pallet::Config>::Balances as Currency<<T as frame_system::Config>::AccountId>>::Balance>,
{
    type AssetId = LiberlandAssetId;
    type Balance = <T as pallet_assets::Config>::Balance;

    fn lock_asset(
        _network_id: GenericNetworkId,
        asset_kind: bridge_types::types::AssetKind,
        who: &T::AccountId,
        asset_id: &Self::AssetId,
        amount: &Self::Balance,
    ) -> DispatchResult {
        let tech_acc = T::SoraMainnetTechAcc::get();
        match asset_id {
                LiberlandAssetId::LLD => {
                match asset_kind {
                    bridge_types::types::AssetKind::Thischain => {
                        T::Balances::transfer(
                            who,
                            &tech_acc,
                            (*amount).into(),
                            ExistenceRequirement::AllowDeath,
                        )?;
                    },
                    bridge_types::types::AssetKind::Sidechain => fail!(Error::<T>::WrongSidechainAsset),
                }
            },
            LiberlandAssetId::Asset(asset) => {
                match asset_kind {
                    bridge_types::types::AssetKind::Thischain => {
                        <pallet_assets::Pallet<T> as Mutate<T::AccountId>>::transfer(
                            (*asset).into(),
                            who,
                            &tech_acc,
                            *amount,
                            Preservation::Expendable,
                        )?;
                    },
                    bridge_types::types::AssetKind::Sidechain => {
                        <pallet_assets::Pallet<T> as Mutate<T::AccountId>>::burn_from(
                            (*asset).into(),
                            who,
                            *amount,
                            Precision::Exact,
                            Fortitude::Polite,
                        )?;
                    },
                }
            }
        }
        Ok(())
    }

    fn unlock_asset(
        _network_id: GenericNetworkId,
        asset_kind: bridge_types::types::AssetKind,
        who: &T::AccountId,
        asset_id: &Self::AssetId,
        amount: &Self::Balance,
    ) -> DispatchResult {
        let tech_acc = T::SoraMainnetTechAcc::get();
        match asset_id {
            LiberlandAssetId::LLD => {
                match asset_kind {
                    bridge_types::types::AssetKind::Thischain => {
                        T::Balances::transfer(
                            &tech_acc,
                            who,
                            (*amount).into(),
                            ExistenceRequirement::AllowDeath,
                        )?;
                    },
                    bridge_types::types::AssetKind::Sidechain => fail!(Error::<T>::WrongSidechainAsset),
                }
            },
            LiberlandAssetId::Asset(asset) => {
                match asset_kind {
                    bridge_types::types::AssetKind::Thischain => {
                        <pallet_assets::Pallet<T> as Mutate<T::AccountId>>::transfer(
                            (*asset).into(),
                            &tech_acc,
                            who,
                            *amount,
                            Preservation::Expendable,
                        )?;
                    },
                    bridge_types::types::AssetKind::Sidechain => {
                        <pallet_assets::Pallet<T> as Mutate<T::AccountId>>::mint_into(
                            (*asset).into(),
                            who,
                            *amount,
                        )?;
                    }
                }
            }
        }
        Ok(())
    }

    fn refund_fee(
            _network_id: GenericNetworkId,
            _who: &T::AccountId,
            _asset_id: &Self::AssetId,
            _amount: &Self::Balance,
        ) -> DispatchResult {
            Err(DispatchError::Unavailable)
    }

    fn withdraw_fee(
            _network_id: GenericNetworkId,
            _who: &T::AccountId,
            _asset_id: &Self::AssetId,
            _amount: &Self::Balance,
        ) -> DispatchResult {
            Err(DispatchError::Unavailable)
    }
}

impl<T: Config>
    bridge_types::traits::MessageStatusNotifier<LiberlandAssetId, AccountId32, T::Balance>
    for Pallet<T>
{
    fn update_status(
        network_id: GenericNetworkId,
        message_id: H256,
        mut new_status: MessageStatus,
        end_timepoint: GenericTimepoint,
    ) {
        let sender = match Senders::<T>::get(network_id, message_id) {
            Some(sender) => sender,
            None => {
                log::warn!(
                    "Message status update called for unknown message: {:?} {:?}",
                    network_id,
                    message_id
                );
                return;
            }
        };
        Transactions::<T>::mutate((network_id, sender), message_id, |req| {
            if let Some(req) = req {
                if new_status == MessageStatus::Failed
                    && req.direction == MessageDirection::Outbound
                {
                    match Pallet::<T>::refund(
                        network_id,
                        message_id,
                        req.source.clone(),
                        req.asset_id,
                        req.amount,
                    ) {
                        Ok(_) => {
                            new_status = MessageStatus::Refunded;
                        }
                        Err(_) => {
                            Self::deposit_event(Event::RefundFailed(message_id));
                        }
                    }
                }
                req.status = new_status;
                req.end_timepoint = end_timepoint;

                Self::deposit_event(Event::RequestStatusUpdate(message_id, new_status));
            }
        })
    }

    fn inbound_request(
        network_id: GenericNetworkId,
        message_id: H256,
        source: GenericAccount,
        dest32: AccountId32,
        asset_id: LiberlandAssetId,
        amount: T::Balance,
        start_timepoint: GenericTimepoint,
        status: MessageStatus,
    ) {
        Self::deposit_event(Event::RequestStatusUpdate(message_id, status));
        let dest = GenericAccount::Liberland(dest32);
        Senders::<T>::insert(network_id, message_id, &dest);

        let bridge_request = BridgeRequest {
            source,
            dest: dest.clone(),
            asset_id,
            amount,
            status,
            start_timepoint,
            end_timepoint: T::TimepointProvider::get_timepoint(),
            direction: MessageDirection::Inbound,
        };

        Transactions::<T>::insert((&network_id, &dest), message_id, bridge_request);
    }

    fn outbound_request(
        network_id: GenericNetworkId,
        message_id: H256,
        source32: AccountId32,
        dest: GenericAccount,
        asset_id: LiberlandAssetId,
        amount: T::Balance,
        status: MessageStatus,
    ) {
        Self::deposit_event(Event::RequestStatusUpdate(message_id, status));
        let source = GenericAccount::Liberland(source32);
        Senders::<T>::insert(network_id, message_id, &source);
        let bridge_request = BridgeRequest {
            source: source.clone(),
            dest,
            asset_id,
            amount,
            status,
            start_timepoint: T::TimepointProvider::get_timepoint(),
            end_timepoint: GenericTimepoint::Pending,
            direction: MessageDirection::Outbound,
        };
        Transactions::<T>::insert((&network_id, &source), message_id, bridge_request);
    }
}
