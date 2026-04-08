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

//! # Substrate App
//!
//! An application that implements bridged substrate assets transfer
//!
//! ## Interface
//!
//! ### Dispatchable Calls
//!
//! - `burn`: Burn an backed substrate or thischain token balance.
//!
#![cfg_attr(not(feature = "std"), no_std)]

pub const TRANSFER_MAX_GAS: u64 = 100_000;

extern crate alloc;

pub mod weights;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

use bridge_types::substrate::SubstrateAppCall;
use bridge_types::traits::BridgeApp;
use bridge_types::traits::BridgeAssetLocker;
use bridge_types::types::{BridgeAppInfo, BridgeAssetInfo};
use bridge_types::GenericAccount;
use bridge_types::GenericNetworkId;
use bridge_types::{GenericAssetId, GenericBalance};
use frame_support::dispatch::DispatchResult;
use frame_support::ensure;
use frame_support::traits::EnsureOrigin;
use frame_support::weights::Weight;
use frame_system::ensure_signed;
use sp_runtime::traits::{Convert, Get, Zero};
use sp_runtime::DispatchError;
use sp_std::prelude::*;

pub use weights::WeightInfo;

pub use pallet::*;

impl<T: Config> TryFrom<SubstrateAppCall> for Call<T>
where
    GenericAccount: TryInto<AccountIdOf<T>>,
    GenericAssetId: TryInto<AssetIdOf<T>>,
    GenericBalance: TryInto<BalanceOf<T>>,
{
    type Error = Error<T>;
    fn try_from(value: SubstrateAppCall) -> Result<Self, Self::Error> {
        let call = match value {
            SubstrateAppCall::Transfer {
                sender,
                recipient,
                amount,
                asset_id,
            } => Call::mint {
                sender,
                recipient: recipient
                    .try_into()
                    .map_err(|_| Error::<T>::WrongAccountId)?,
                asset_id: asset_id.try_into().map_err(|_| Error::<T>::WrongAssetId)?,
                amount,
            },
            SubstrateAppCall::FinalizeAssetRegistration {
                asset_id,
                asset_kind,
                precision,
                sidechain_asset,
            } => Call::finalize_asset_registration {
                // This is sidechain asset for another chain, for our chain it's thischain
                asset_id: sidechain_asset
                    .try_into()
                    .map_err(|_| Error::<T>::WrongAssetId)?,
                // This is thischain asset for another chain, for our chain it's sidechain
                sidechain_asset_id: asset_id,
                asset_kind,
                sidechain_precision: precision,
            },
            // // This chain for this chain is side chain on side chain
            // // That's why incoming_sidechain_asset_registration should be invoked
            SubstrateAppCall::RegisterAsset {
                asset_id,
                sidechain_asset,
            } => Call::incoming_thischain_asset_registration {
                // This is sidechain asset for another chain, for our chain it's thischain
                asset_id: sidechain_asset
                    .try_into()
                    .map_err(|_| Error::<T>::WrongAssetId)?,
                sidechain_asset_id: asset_id,
            },
            SubstrateAppCall::ReportTransferResult {
                message_id,
                message_status,
            } => Call::update_transaction_status {
                message_id,
                message_status,
            },
        };
        Ok(call)
    }
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::large_enum_variant)]
#[frame_support::pallet]
pub mod pallet {

    use super::*;

    use bridge_types::substrate::SubstrateBridgeMessageEncode;
    use bridge_types::traits::{
        BalancePrecisionConverter, BridgeAssetLocker, BridgeAssetRegistry, MessageStatusNotifier,
        OutboundChannel,
    };
    use bridge_types::types::{AssetKind, CallOriginOutput, MessageStatus};
    use bridge_types::{GenericAccount, GenericNetworkId, SubNetworkId, H256};
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use frame_system::{ensure_root, RawOrigin};

    pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

    pub type AssetIdOf<T> =
        <<T as Config>::BridgeAssetLocker as BridgeAssetLocker<AccountIdOf<T>>>::AssetId;

    pub type BalanceOf<T> =
        <<T as Config>::BridgeAssetLocker as BridgeAssetLocker<AccountIdOf<T>>>::Balance;

    pub type AssetNameOf<T> = <<T as Config>::AssetRegistry as BridgeAssetRegistry<
        AccountIdOf<T>,
        AssetIdOf<T>,
    >>::AssetName;

    pub type AssetSymbolOf<T> = <<T as Config>::AssetRegistry as BridgeAssetRegistry<
        AccountIdOf<T>,
        AssetIdOf<T>,
    >>::AssetSymbol;

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {

        type OutboundChannel: OutboundChannel<SubNetworkId, Self::AccountId, ()>;

        type CallOrigin: EnsureOrigin<
            Self::RuntimeOrigin,
            Success = CallOriginOutput<SubNetworkId, H256, ()>,
        >;

        type MessageStatusNotifier: MessageStatusNotifier<
            AssetIdOf<Self>,
            Self::AccountId,
            BalanceOf<Self>,
        >;

        type AssetRegistry: BridgeAssetRegistry<Self::AccountId, AssetIdOf<Self>>;

        type AccountIdConverter: Convert<Self::AccountId, GenericAccount>;

        type AssetIdConverter: Convert<AssetIdOf<Self>, GenericAssetId>;

        type BalancePrecisionConverter: BalancePrecisionConverter<
            AssetIdOf<Self>,
            BalanceOf<Self>,
            GenericBalance,
        >;

        type BridgeAssetLocker: BridgeAssetLocker<Self::AccountId>;

        type WeightInfo: WeightInfo;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        Burned {
            network_id: SubNetworkId,
            asset_id: AssetIdOf<T>,
            sender: T::AccountId,
            recipient: GenericAccount,
            amount: BalanceOf<T>,
        },
        Minted {
            network_id: SubNetworkId,
            asset_id: AssetIdOf<T>,
            sender: GenericAccount,
            recipient: T::AccountId,
            amount: BalanceOf<T>,
        },
        FailedToMint(H256, DispatchError),
        AssetRegistrationProceed(AssetIdOf<T>),
        AssetRegistrationFinalized(AssetIdOf<T>),
    }

    #[pallet::storage]
    #[pallet::getter(fn asset_kind)]
    pub(super) type AssetKinds<T: Config> =
        StorageDoubleMap<_, Identity, SubNetworkId, Identity, AssetIdOf<T>, AssetKind, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn sidechain_precision)]
    pub(super) type SidechainPrecision<T: Config> =
        StorageDoubleMap<_, Identity, SubNetworkId, Identity, AssetIdOf<T>, u8, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn sidechain_asset_id)]
    pub(super) type SidechainAssetId<T: Config> = StorageDoubleMap<
        _,
        Identity,
        SubNetworkId,
        Identity,
        AssetIdOf<T>,
        GenericAssetId,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn thischain_asset_id)]
    pub(super) type ThischainAssetId<T: Config> = StorageDoubleMap<
        _,
        Identity,
        SubNetworkId,
        Identity,
        GenericAssetId,
        AssetIdOf<T>,
        OptionQuery,
    >;

    #[pallet::error]
    pub enum Error<T> {
        TokenIsNotRegistered,
        InvalidNetwork,
        TokenAlreadyRegistered,
        /// Call encoding failed.
        CallEncodeFailed,
        /// Amount must be > 0
        WrongAmount,
        UnknownPrecision,
        WrongAssetId,
        WrongAccountId,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Function used to mint or unlock tokens
        /// The Origin for this call is the Bridge Origin
        /// Only the relayer can call this function
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::mint())]
        pub fn mint(
            origin: OriginFor<T>,
            asset_id: AssetIdOf<T>,
            sender: GenericAccount,
            recipient: AccountIdOf<T>,
            amount: GenericBalance,
        ) -> DispatchResult {
            let CallOriginOutput {
                network_id,
                message_id,
                timepoint,
                ..
            } = T::CallOrigin::ensure_origin(origin.clone())?;

            // Here we need a logic that does not fail the extrinic
            if let Err(error) = Self::mint_inner(
                asset_id, sender, recipient, amount, network_id, message_id, timepoint,
            ) {
                Self::deposit_event(Event::FailedToMint(message_id, error));
                T::OutboundChannel::submit(
                    network_id,
                    &RawOrigin::Root,
                    &SubstrateAppCall::ReportTransferResult {
                        message_id,
                        message_status: MessageStatus::Failed,
                    }
                    .prepare_message(),
                    (),
                )?;
            } else {
                T::OutboundChannel::submit(
                    network_id,
                    &RawOrigin::Root,
                    &SubstrateAppCall::ReportTransferResult {
                        message_id,
                        message_status: MessageStatus::Done,
                    }
                    .prepare_message(),
                    (),
                )?;
            }

            Ok(())
        }

        /// Function used to finalize asset registration if everything went well on the sidechain
        /// The Origin for this call is the Bridge Origin
        /// Only the relayer can call this function
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::finalize_asset_registration())]
        pub fn finalize_asset_registration(
            origin: OriginFor<T>,
            asset_id: AssetIdOf<T>,
            sidechain_asset_id: GenericAssetId,
            asset_kind: AssetKind,
            sidechain_precision: u8,
        ) -> DispatchResult {
            let CallOriginOutput { network_id, .. } = T::CallOrigin::ensure_origin(origin.clone())?;
            SidechainPrecision::<T>::insert(network_id, asset_id.clone(), sidechain_precision);
            AssetKinds::<T>::insert(network_id, asset_id.clone(), asset_kind);
            ThischainAssetId::<T>::insert(network_id, sidechain_asset_id, asset_id.clone());
            SidechainAssetId::<T>::insert(network_id, asset_id.clone(), sidechain_asset_id);
            Self::deposit_event(Event::<T>::AssetRegistrationFinalized(asset_id));
            Ok(())
        }

        /// Function used to register this chain asset
        /// The Origin for this call is the Bridge Origin
        /// Only the relayer can call this function
        /// Sends the message to sidechain to finalize asset registration
        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::incoming_thischain_asset_registration())]
        pub fn incoming_thischain_asset_registration(
            origin: OriginFor<T>,
            asset_id: AssetIdOf<T>,
            sidechain_asset_id: GenericAssetId,
        ) -> DispatchResult {
            let CallOriginOutput { network_id, .. } = T::CallOrigin::ensure_origin(origin.clone())?;
            ensure!(
                T::AssetRegistry::ensure_asset_exists(asset_id.clone()),
                Error::<T>::TokenIsNotRegistered
            );
            let asset_kind = AssetKind::Thischain;

            let precision = T::AssetRegistry::get_raw_info(asset_id.clone()).precision;

            T::AssetRegistry::manage_asset(network_id.into(), asset_id.clone())?;

            SidechainPrecision::<T>::insert(network_id, asset_id.clone(), precision);
            AssetKinds::<T>::insert(network_id, asset_id.clone(), asset_kind);
            ThischainAssetId::<T>::insert(network_id, sidechain_asset_id, asset_id.clone());
            SidechainAssetId::<T>::insert(network_id, asset_id.clone(), sidechain_asset_id);

            T::OutboundChannel::submit(
                network_id,
                &RawOrigin::Root,
                &SubstrateAppCall::FinalizeAssetRegistration {
                    asset_id: T::AssetIdConverter::convert(asset_id.clone()),
                    sidechain_asset: sidechain_asset_id,
                    asset_kind: AssetKind::Sidechain,
                    precision,
                }
                .prepare_message(),
                (),
            )?;
            Self::deposit_event(Event::<T>::AssetRegistrationProceed(asset_id));
            Ok(())
        }

        /// Function used by users to send tokens to the sidechain
        #[pallet::call_index(3)]
        #[pallet::weight(<T as Config>::WeightInfo::burn())]
        pub fn burn(
            origin: OriginFor<T>,
            network_id: SubNetworkId,
            asset_id: AssetIdOf<T>,
            recipient: GenericAccount,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            Self::burn_inner(who, network_id, asset_id, recipient, amount)?;

            Ok(())
        }

        /// Function used to register sidechain asset
        /// The Origin for this call is the Root Origin
        /// Only the root can call this function
        /// Sends the message to sidechain to register asset
        #[pallet::call_index(4)]
        #[pallet::weight(<T as Config>::WeightInfo::register_sidechain_asset())]
        pub fn register_sidechain_asset(
            origin: OriginFor<T>,
            network_id: SubNetworkId,
            sidechain_asset: GenericAssetId,
            symbol: AssetSymbolOf<T>,
            name: AssetNameOf<T>,
        ) -> DispatchResult {
            ensure_root(origin)?;

            // Ensure that asset had not been registered for current network id
            ensure!(
                ThischainAssetId::<T>::get(network_id, sidechain_asset).is_none(),
                Error::<T>::TokenAlreadyRegistered
            );

            let asset_id =
                T::AssetRegistry::register_asset(network_id.into(), name.clone(), symbol.clone())?;

            T::AssetRegistry::manage_asset(network_id.into(), asset_id.clone())?;

            T::OutboundChannel::submit(
                network_id,
                &RawOrigin::Root,
                &bridge_types::substrate::SubstrateAppCall::RegisterAsset {
                    asset_id: T::AssetIdConverter::convert(asset_id),
                    sidechain_asset,
                }
                .prepare_message(),
                (),
            )?;
            Ok(())
        }

        /// Function used to update transaction status
        /// The Origin for this call is the Bridge Origin
        /// Only the relayer can call this function
        #[pallet::call_index(5)]
        #[pallet::weight(<T as Config>::WeightInfo::update_transaction_status())]
        pub fn update_transaction_status(
            origin: OriginFor<T>,
            message_id: H256,
            message_status: MessageStatus,
        ) -> DispatchResult {
            let CallOriginOutput {
                network_id,
                timepoint,
                ..
            } = T::CallOrigin::ensure_origin(origin)?;

            T::MessageStatusNotifier::update_status(
                network_id.into(),
                message_id,
                message_status,
                timepoint,
            );
            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        pub fn mint_inner(
            asset_id: AssetIdOf<T>,
            sender: GenericAccount,
            recipient: AccountIdOf<T>,
            amount: GenericBalance,
            network_id: SubNetworkId,
            message_id: H256,
            timepoint: bridge_types::GenericTimepoint,
        ) -> DispatchResult {
            let asset_kind = AssetKinds::<T>::get(network_id, &asset_id)
                .ok_or(Error::<T>::TokenIsNotRegistered)?;

            let precision = SidechainPrecision::<T>::get(network_id, &asset_id)
                .ok_or(Error::<T>::UnknownPrecision)?;
            let (amount, _) =
                T::BalancePrecisionConverter::from_sidechain(&asset_id, precision, amount)
                    .ok_or(Error::<T>::WrongAmount)?;
            ensure!(amount > Zero::zero(), Error::<T>::WrongAmount);

            T::BridgeAssetLocker::unlock_asset(
                network_id.into(),
                asset_kind,
                &recipient,
                &asset_id,
                &amount,
            )?;

            T::MessageStatusNotifier::inbound_request(
                GenericNetworkId::Sub(network_id),
                message_id,
                sender.clone(),
                recipient.clone(),
                asset_id.clone(),
                amount.clone(),
                timepoint,
                MessageStatus::Done,
            );

            Self::deposit_event(Event::Minted {
                network_id,
                asset_id,
                sender,
                recipient,
                amount,
            });

            Ok(())
        }

        pub fn burn_inner(
            sender: T::AccountId,
            network_id: SubNetworkId,
            asset_id: AssetIdOf<T>,
            recipient: GenericAccount,
            amount: BalanceOf<T>,
        ) -> Result<H256, DispatchError> {
            ensure!(amount > BalanceOf::<T>::zero(), Error::<T>::WrongAmount);

            let asset_kind = AssetKinds::<T>::get(network_id, &asset_id)
                .ok_or(Error::<T>::TokenIsNotRegistered)?;

            let precision = SidechainPrecision::<T>::get(network_id, &asset_id)
                .ok_or(Error::<T>::UnknownPrecision)?;

            ensure!(!amount.is_zero(), Error::<T>::WrongAmount);

            let sidechain_asset_id = SidechainAssetId::<T>::get(network_id, asset_id.clone())
                .ok_or(Error::<T>::TokenIsNotRegistered)?;

            let (_, sidechain_amount) =
                T::BalancePrecisionConverter::to_sidechain(&asset_id, precision, amount.clone())
                    .ok_or(Error::<T>::WrongAmount)?;

            T::BridgeAssetLocker::lock_asset(
                network_id.into(),
                asset_kind,
                &sender,
                &asset_id,
                &amount,
            )?;

            let message = bridge_types::substrate::SubstrateAppCall::Transfer {
                recipient: recipient.clone(),
                amount: sidechain_amount,
                asset_id: sidechain_asset_id,
                sender: T::AccountIdConverter::convert(sender.clone()),
            };

            let message_id = T::OutboundChannel::submit(
                network_id,
                &RawOrigin::Signed(sender.clone()),
                &message.prepare_message(),
                (),
            )?;

            T::MessageStatusNotifier::outbound_request(
                GenericNetworkId::Sub(network_id),
                message_id,
                sender.clone(),
                recipient.clone(),
                asset_id.clone(),
                amount.clone(),
                MessageStatus::InQueue,
            );

            Self::deposit_event(Event::Burned {
                network_id,
                asset_id,
                sender,
                recipient,
                amount,
            });

            Ok(Default::default())
        }
    }

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub assets: Vec<(SubNetworkId, AssetIdOf<T>, AssetKind)>,
    }

    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                assets: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            for (network_id, asset_id, asset_kind) in &self.assets {
                AssetKinds::<T>::insert(network_id, asset_id, asset_kind);
            }
        }
    }
}

impl<T: Config> BridgeApp<T::AccountId, GenericAccount, AssetIdOf<T>, BalanceOf<T>> for Pallet<T> {
    fn is_asset_supported(network_id: GenericNetworkId, asset_id: AssetIdOf<T>) -> bool {
        let GenericNetworkId::Sub(network_id) = network_id else {
            return false;
        };
        AssetKinds::<T>::contains_key(network_id, asset_id)
    }

    fn transfer(
        network_id: GenericNetworkId,
        asset_id: AssetIdOf<T>,
        sender: T::AccountId,
        recipient: GenericAccount,
        amount: BalanceOf<T>,
    ) -> Result<bridge_types::H256, DispatchError> {
        let network_id = network_id.sub().ok_or(Error::<T>::InvalidNetwork)?;
        Self::burn_inner(sender, network_id, asset_id, recipient, amount)
    }

    fn refund(
        network_id: GenericNetworkId,
        _message_id: bridge_types::H256,
        recipient: T::AccountId,
        asset_id: AssetIdOf<T>,
        amount: BalanceOf<T>,
    ) -> DispatchResult {
        let network_id = network_id.sub().ok_or(Error::<T>::InvalidNetwork)?;
        let asset_kind =
            AssetKinds::<T>::get(network_id, &asset_id).ok_or(Error::<T>::TokenIsNotRegistered)?;

        T::BridgeAssetLocker::unlock_asset(
            network_id.into(),
            asset_kind,
            &recipient,
            &asset_id,
            &amount,
        )?;
        Ok(())
    }

    fn list_supported_assets(
        network_id: GenericNetworkId,
    ) -> Vec<bridge_types::types::BridgeAssetInfo> {
        let GenericNetworkId::Sub(network_id) = network_id else {
            return vec![];
        };
        AssetKinds::<T>::iter_prefix(network_id)
            .map(|(asset_id, _asset_kind)| {
                let _asset_id = T::AssetIdConverter::convert(asset_id);
                BridgeAssetInfo::Liberland
            })
            .collect()
    }

    fn list_apps() -> Vec<bridge_types::types::BridgeAppInfo> {
        AssetKinds::<T>::iter_keys()
            .map(|(network_id, _asset_id)| BridgeAppInfo::Sub(network_id.into()))
            .fold(vec![], |mut acc, value| {
                if !acc.iter().any(|x| value == *x) {
                    acc.push(value);
                }
                acc
            })
    }

    fn is_asset_supported_weight() -> Weight {
        T::DbWeight::get().reads(1)
    }

    fn refund_weight() -> Weight {
        <T as Config>::WeightInfo::update_transaction_status()
    }

    fn transfer_weight() -> Weight {
        <T as Config>::WeightInfo::burn()
    }
}
