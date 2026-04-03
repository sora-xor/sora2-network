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

//! # Parachain App
//!
//! An application that implements bridged parachain/relaychain assets transfer
//!
//! ## Interface
//!
//! ### Dispatchable Calls
//!
//! - `burn`: Burn an backed parachain/relaychain or thischain token balance.
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

use bridge_types::substrate::SubAssetInfo;
use bridge_types::substrate::{ParachainAccountId, ParachainAppCall};
use bridge_types::traits::BridgeApp;
use bridge_types::traits::BridgeAssetLocker;
use bridge_types::types::{BridgeAppInfo, BridgeAssetInfo};
use bridge_types::GenericNetworkId;
use bridge_types::{MainnetAccountId, MainnetAssetId};
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

impl<T: Config> From<ParachainAppCall> for Call<T>
where
    T::AccountId: From<MainnetAccountId>,
    AssetIdOf<T>: From<MainnetAssetId>,
{
    fn from(value: ParachainAppCall) -> Self {
        match value {
            ParachainAppCall::Transfer {
                sender,
                recipient,
                amount,
                asset_id,
            } => Call::mint {
                sender,
                recipient: recipient.into(),
                asset_id: asset_id.into(),
                amount,
            },
            ParachainAppCall::FinalizeAssetRegistration {
                asset_id,
                asset_kind,
            } => Call::finalize_asset_registration {
                asset_id: asset_id.into(),
                asset_kind,
            },
            ParachainAppCall::ReportXCMTransferResult {
                message_id,
                transfer_status,
            } => Call::update_transaction_status {
                message_id,
                transfer_status,
            },
        }
    }
}

#[allow(clippy::too_many_arguments)]
#[frame_support::pallet]
pub mod pallet {

    use super::*;

    use bridge_types::substrate::XCMAppTransferStatus;
    use bridge_types::substrate::{
        ParachainAccountId, ParachainAssetId, SubstrateBridgeMessageEncode, XCMAppCall,
    };
    use bridge_types::traits::{
        BalancePrecisionConverter, BridgeAssetLocker, BridgeAssetRegistry, MessageStatusNotifier,
        OutboundChannel,
    };
    use bridge_types::types::{AssetKind, CallOriginOutput, MessageStatus};
    use bridge_types::{
        GenericAccount, GenericNetworkId, MainnetAccountId, MainnetAssetId, MainnetBalance,
        SubNetworkId, H256,
    };
    use frame_support::fail;
    use frame_support::pallet_prelude::{OptionQuery, ValueQuery, *};
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

        type AccountIdConverter: Convert<Self::AccountId, MainnetAccountId>;

        type AssetIdConverter: Convert<AssetIdOf<Self>, MainnetAssetId>;

        type BalancePrecisionConverter: BalancePrecisionConverter<
            AssetIdOf<Self>,
            BalanceOf<Self>,
            MainnetBalance,
        >;

        type BridgeAssetLocker: BridgeAssetLocker<Self::AccountId>;

        type WeightInfo: WeightInfo;
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// [network_id, asset_id, sender, recepient, amount]
        Burned(
            SubNetworkId,
            AssetIdOf<T>,
            T::AccountId,
            ParachainAccountId,
            BalanceOf<T>,
        ),
        /// [network_id, asset_id, sender, recepient, amount]
        Minted(
            SubNetworkId,
            AssetIdOf<T>,
            Option<ParachainAccountId>,
            T::AccountId,
            BalanceOf<T>,
        ),
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
    #[pallet::getter(fn allowed_parachain_assets)]
    pub(super) type AllowedParachainAssets<T: Config> =
        StorageDoubleMap<_, Identity, SubNetworkId, Identity, u32, Vec<AssetIdOf<T>>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn relaychain_asset)]
    pub(super) type RelaychainAsset<T: Config> =
        StorageMap<_, Identity, SubNetworkId, AssetIdOf<T>, OptionQuery>;

    #[pallet::error]
    pub enum Error<T> {
        TokenIsNotRegistered,
        AppIsNotRegistered,
        NotEnoughFunds,
        InvalidNetwork,
        TokenAlreadyRegistered,
        AppAlreadyRegistered,
        /// Call encoding failed.
        CallEncodeFailed,
        /// Amount must be > 0
        WrongAmount,
        TransferLimitReached,
        UnknownPrecision,
        MessageIdNotFound,
        InvalidDestinationParachain,
        InvalidDestinationParams,
        RelaychainAssetNotRegistered,
        NotRelayTransferableAsset,
        RelaychainAssetRegistered,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // Internal calls to be used from Parachain side.

        // TODO: make benchmarks
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::mint())]
        pub fn mint(
            origin: OriginFor<T>,
            asset_id: AssetIdOf<T>,
            sender: Option<ParachainAccountId>,
            recipient: T::AccountId,
            amount: MainnetBalance,
        ) -> DispatchResult {
            let CallOriginOutput {
                network_id,
                message_id,
                timepoint,
                ..
            } = T::CallOrigin::ensure_origin(origin.clone())?;

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
                sender
                    .clone()
                    .map(GenericAccount::Parachain)
                    .unwrap_or(GenericAccount::Unknown),
                recipient.clone(),
                asset_id.clone(),
                amount.clone(),
                timepoint,
                MessageStatus::Done,
            );
            Self::deposit_event(Event::Minted(
                network_id, asset_id, sender, recipient, amount,
            ));
            Ok(())
        }

        // TODO: make benchmarks
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::finalize_asset_registration())]
        pub fn finalize_asset_registration(
            origin: OriginFor<T>,
            asset_id: AssetIdOf<T>,
            asset_kind: AssetKind,
        ) -> DispatchResult {
            let CallOriginOutput { network_id, .. } = T::CallOrigin::ensure_origin(origin.clone())?;
            ensure!(
                SidechainPrecision::<T>::contains_key(network_id, &asset_id),
                Error::<T>::TokenIsNotRegistered
            );
            AssetKinds::<T>::insert(network_id, asset_id, asset_kind);
            Ok(())
        }

        // Common exstrinsics

        // TODO: make benchmarks
        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::burn())]
        pub fn burn(
            origin: OriginFor<T>,
            network_id: SubNetworkId,
            asset_id: AssetIdOf<T>,
            recipient: ParachainAccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            Self::burn_inner(who, network_id, asset_id, recipient, amount)?;

            Ok(())
        }

        #[pallet::call_index(3)]
        #[pallet::weight(<T as Config>::WeightInfo::register_thischain_asset(allowed_parachains.len() as u32))]
        pub fn register_thischain_asset(
            origin: OriginFor<T>,
            network_id: SubNetworkId,
            asset_id: AssetIdOf<T>,
            sidechain_asset: ParachainAssetId,
            allowed_parachains: Vec<u32>,
            minimal_xcm_amount: BalanceOf<T>,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(
                !AssetKinds::<T>::contains_key(network_id, &asset_id),
                Error::<T>::TokenAlreadyRegistered
            );

            let sidechain_precision = T::AssetRegistry::get_raw_info(asset_id.clone()).precision;

            let (_, minimal_xcm_amount) = T::BalancePrecisionConverter::to_sidechain(
                &asset_id,
                sidechain_precision,
                minimal_xcm_amount,
            )
            .ok_or(Error::<T>::WrongAmount)?;

            ensure!(minimal_xcm_amount > 0, Error::<T>::WrongAmount);

            Self::register_asset_inner(
                network_id,
                asset_id,
                sidechain_asset,
                AssetKind::Thischain,
                sidechain_precision,
                allowed_parachains,
                minimal_xcm_amount,
            )?;

            Ok(())
        }

        #[pallet::call_index(4)]
        #[pallet::weight(<T as Config>::WeightInfo::register_sidechain_asset(allowed_parachains.len() as u32))]
        pub fn register_sidechain_asset(
            origin: OriginFor<T>,
            network_id: SubNetworkId,
            sidechain_asset: ParachainAssetId,
            symbol: AssetSymbolOf<T>,
            name: AssetNameOf<T>,
            decimals: u8,
            allowed_parachains: Vec<u32>,
            minimal_xcm_amount: BalanceOf<T>,
        ) -> DispatchResult {
            ensure_root(origin)?;

            let asset_id = T::AssetRegistry::register_asset(network_id.into(), name, symbol)?;
            let (_, minimal_xcm_amount) =
                T::BalancePrecisionConverter::to_sidechain(&asset_id, decimals, minimal_xcm_amount)
                    .ok_or(Error::<T>::WrongAmount)?;

            ensure!(minimal_xcm_amount > 0, Error::<T>::WrongAmount);

            Self::register_asset_inner(
                network_id,
                asset_id,
                sidechain_asset,
                AssetKind::Sidechain,
                decimals,
                allowed_parachains,
                minimal_xcm_amount,
            )?;
            Ok(())
        }

        #[pallet::call_index(5)]
        #[pallet::weight(<T as Config>::WeightInfo::add_assetid_paraid())]
        pub fn add_assetid_paraid(
            origin: OriginFor<T>,
            network_id: SubNetworkId,
            para_id: u32,
            asset_id: AssetIdOf<T>,
        ) -> DispatchResult {
            ensure_root(origin)?;
            AssetKinds::<T>::get(network_id, &asset_id).ok_or(Error::<T>::TokenIsNotRegistered)?;

            AllowedParachainAssets::<T>::try_mutate(network_id, para_id, |x| -> DispatchResult {
                x.push(asset_id);
                Ok(())
            })?;

            Ok(())
        }

        #[pallet::call_index(6)]
        #[pallet::weight(<T as Config>::WeightInfo::remove_assetid_paraid())]
        pub fn remove_assetid_paraid(
            origin: OriginFor<T>,
            network_id: SubNetworkId,
            para_id: u32,
            asset_id: AssetIdOf<T>,
        ) -> DispatchResult {
            ensure_root(origin)?;
            AssetKinds::<T>::get(network_id, &asset_id).ok_or(Error::<T>::TokenIsNotRegistered)?;

            AllowedParachainAssets::<T>::try_mutate(network_id, para_id, |x| -> DispatchResult {
                x.retain(|el| *el != asset_id);
                Ok(())
            })?;

            Ok(())
        }

        #[pallet::call_index(7)]
        #[pallet::weight(<T as Config>::WeightInfo::update_transaction_status())]
        pub fn update_transaction_status(
            origin: OriginFor<T>,
            message_id: H256,
            transfer_status: XCMAppTransferStatus,
        ) -> DispatchResult {
            let CallOriginOutput {
                network_id,
                timepoint,
                ..
            } = T::CallOrigin::ensure_origin(origin)?;

            let message_status = match transfer_status {
                XCMAppTransferStatus::Success => MessageStatus::Done,
                XCMAppTransferStatus::XCMTransferError => MessageStatus::Failed,
            };
            T::MessageStatusNotifier::update_status(
                network_id.into(),
                message_id,
                message_status,
                timepoint,
            );
            Ok(())
        }

        // TODO: make benchmarks
        #[pallet::call_index(8)]
        #[pallet::weight(<T as Config>::WeightInfo::mint())]
        pub fn set_minimum_xcm_incoming_asset_count(
            origin: OriginFor<T>,
            network_id: SubNetworkId,
            asset_id: AssetIdOf<T>,
            minimal_xcm_amount: BalanceOf<T>,
        ) -> DispatchResult {
            ensure_root(origin)?;

            let Some(sidechain_precision) = Self::sidechain_precision(network_id, &asset_id) else {
                fail!(Error::<T>::UnknownPrecision);
            };

            let (_, minimal_xcm_amount) = T::BalancePrecisionConverter::to_sidechain(
                &asset_id,
                sidechain_precision,
                minimal_xcm_amount,
            )
            .ok_or(Error::<T>::WrongAmount)?;

            ensure!(minimal_xcm_amount > 0, Error::<T>::WrongAmount);

            T::OutboundChannel::submit(
                network_id,
                &RawOrigin::Root,
                &XCMAppCall::SetAssetMinAmount {
                    asset_id: T::AssetIdConverter::convert(asset_id.clone()),
                    minimal_xcm_amount,
                }
                .prepare_message(),
                (),
            )?;
            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        pub fn register_asset_inner(
            network_id: SubNetworkId,
            asset_id: AssetIdOf<T>,
            sidechain_asset: ParachainAssetId,
            asset_kind: AssetKind,
            sidechain_precision: u8,
            allowed_parachains: Vec<u32>,
            minimal_xcm_amount: u128,
        ) -> DispatchResult {
            T::AssetRegistry::manage_asset(network_id.into(), asset_id.clone())?;
            SidechainPrecision::<T>::insert(network_id, &asset_id, sidechain_precision);

            for paraid in allowed_parachains {
                AllowedParachainAssets::<T>::try_mutate(
                    network_id,
                    paraid,
                    |x| -> DispatchResult {
                        x.push(asset_id.clone());
                        Ok(())
                    },
                )?;
            }

            // if it is a native relaychain asset - register it on the pallet to identify if it is transferred
            if sidechain_asset == bridge_types::substrate::PARENT_PARACHAIN_ASSET {
                ensure!(
                    Self::relaychain_asset(network_id).is_none(),
                    Error::<T>::RelaychainAssetRegistered
                );
                RelaychainAsset::<T>::insert(network_id, asset_id.clone());
            }

            T::OutboundChannel::submit(
                network_id,
                &RawOrigin::Root,
                &XCMAppCall::RegisterAsset {
                    asset_id: T::AssetIdConverter::convert(asset_id),
                    sidechain_asset,
                    asset_kind,
                    minimal_xcm_amount,
                }
                .prepare_message(),
                (),
            )?;
            Ok(())
        }

        pub fn burn_inner(
            who: T::AccountId,
            network_id: SubNetworkId,
            asset_id: AssetIdOf<T>,
            recipient: ParachainAccountId,
            amount: BalanceOf<T>,
        ) -> Result<H256, DispatchError> {
            ensure!(amount > BalanceOf::<T>::zero(), Error::<T>::WrongAmount);

            let asset_kind = AssetKinds::<T>::get(network_id, &asset_id)
                .ok_or(Error::<T>::TokenIsNotRegistered)?;

            Self::check_parachain_transfer_params(network_id, asset_id.clone(), recipient.clone())?;

            let precision = SidechainPrecision::<T>::get(network_id, &asset_id)
                .ok_or(Error::<T>::UnknownPrecision)?;

            let (amount, sidechain_amount) =
                T::BalancePrecisionConverter::to_sidechain(&asset_id, precision, amount)
                    .ok_or(Error::<T>::WrongAmount)?;

            ensure!(sidechain_amount > 0, Error::<T>::WrongAmount);
            T::BridgeAssetLocker::lock_asset(
                network_id.into(),
                asset_kind,
                &who,
                &asset_id,
                &amount,
            )?;

            let message_id = T::OutboundChannel::submit(
                network_id,
                &RawOrigin::Signed(who.clone()),
                &XCMAppCall::Transfer {
                    recipient: recipient.clone(),
                    amount: sidechain_amount,
                    asset_id: T::AssetIdConverter::convert(asset_id.clone()),
                    sender: T::AccountIdConverter::convert(who.clone()),
                }
                .prepare_message(),
                (),
            )?;

            T::MessageStatusNotifier::outbound_request(
                GenericNetworkId::Sub(network_id),
                message_id,
                who.clone(),
                GenericAccount::Parachain(recipient.clone()),
                asset_id.clone(),
                amount.clone(),
                MessageStatus::InQueue,
            );

            Self::deposit_event(Event::Burned(network_id, asset_id, who, recipient, amount));

            Ok(Default::default())
        }

        fn check_parachain_transfer_params(
            network_id: SubNetworkId,
            asset_id: AssetIdOf<T>,
            recipient: ParachainAccountId,
        ) -> DispatchResult {
            use bridge_types::substrate::{Junction, VersionedMultiLocation::V3};

            let V3(ml) = recipient else {
                fail!(Error::<T>::InvalidDestinationParams)
            };

            // parents should be == 1
            if ml.parents != 1 {
                fail!(Error::<T>::InvalidDestinationParams)
            }

            if ml.interior.len() == 1 {
                // len == 1 is transfer to the relay chain

                let Some(relaychain_asset) = Self::relaychain_asset(network_id) else {
                    fail!(Error::<T>::RelaychainAssetNotRegistered)
                };

                // only native relaychain asset can be transferred to the relaychain
                ensure!(
                    asset_id == relaychain_asset,
                    Error::<T>::NotRelayTransferableAsset
                );
            } else if ml.interior.len() == 2 {
                // len == 2 is transfer to a parachain

                let mut parachains: Vec<u32> = Vec::with_capacity(1);
                for x in ml.interior {
                    if let Junction::Parachain(id) = x {
                        parachains.push(id)
                    }
                }

                // Only one parachain is allowed in query
                ensure!(parachains.len() == 1, Error::<T>::InvalidDestinationParams);

                // ensure that destination para id is allowed to transfer to
                ensure!(
                    Self::allowed_parachain_assets(network_id, parachains[0]).contains(&asset_id),
                    Error::<T>::InvalidDestinationParachain
                );
            } else {
                fail!(Error::<T>::InvalidDestinationParams)
            }
            Ok(())
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

impl<T: Config> BridgeApp<T::AccountId, ParachainAccountId, AssetIdOf<T>, BalanceOf<T>>
    for Pallet<T>
{
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
        recipient: ParachainAccountId,
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
            .map(|(asset_id, asset_kind)| {
                let asset_id = T::AssetIdConverter::convert(asset_id);
                BridgeAssetInfo::Sub(SubAssetInfo {
                    asset_id,
                    asset_kind,
                    precision: 18,
                })
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
        <T as Config>::WeightInfo::refund()
    }

    fn transfer_weight() -> Weight {
        <T as Config>::WeightInfo::burn()
    }
}
