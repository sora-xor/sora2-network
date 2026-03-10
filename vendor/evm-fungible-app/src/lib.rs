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

//! # EVM Fungible App
//!
//! An application that implements bridged fungible (ERC-20 and native) assets.
//!
//! ## Overview
//!
//! ETH balances are stored in the tightly-coupled [`asset`] runtime module. When an account holder
//! burns some of their balance, a `Transfer` event is emitted. An external relayer will listen for
//! this event and relay it to the other chain.
//!
//! ## Interface
//!
//! ### Dispatchable Calls
//!
//! - `burn`: Burn an ERC20 token balance.
#![cfg_attr(not(feature = "std"), no_std)]

pub const TRANSFER_MAX_GAS: u64 = 100_000;

extern crate alloc;

mod payload;
pub mod weights;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

use bridge_types::substrate::FAAppCall;
use bridge_types::traits::EVMFeeHandler;
use bridge_types::traits::EVMOutboundChannel;
use bridge_types::traits::{BalancePrecisionConverter, BridgeAssetLocker};
use bridge_types::{EVMChainId, MainnetAccountId, MainnetAssetId};
use bridge_types::{H160, U256};
use codec::{Decode, Encode};
use frame_support::dispatch::{DispatchError, DispatchResult};
use frame_support::ensure;
use frame_support::traits::{EnsureOrigin, Get};
use frame_system::ensure_signed;
use sp_runtime::traits::Zero;
use sp_std::prelude::*;

pub use pallet::*;
pub use weights::WeightInfo;

impl<T: Config> From<FAAppCall> for Call<T>
where
    T::AccountId: From<MainnetAccountId>,
    AssetIdOf<T>: From<MainnetAssetId>,
{
    fn from(value: FAAppCall) -> Self {
        match value {
            FAAppCall::Transfer {
                sender,
                recipient,
                amount,
                token,
            } => Call::mint {
                sender,
                recipient: recipient.into(),
                token,
                amount: U256::from(amount.0),
            },
            FAAppCall::FinalizeAssetRegistration { asset_id, token } => {
                Call::register_asset_internal {
                    asset_id: asset_id.into(),
                    contract: token,
                }
            }
        }
    }
}

#[derive(Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, codec::MaxEncodedLen)]
pub struct BaseFeeInfo<BlockNumber> {
    pub base_fee: U256,
    pub updated: BlockNumber,
    pub evm_block_number: u64,
}

#[frame_support::pallet]
pub mod pallet {

    use crate::payload::*;

    use super::*;

    use bridge_types::evm::*;
    use bridge_types::traits::{
        AppRegistry, BalancePrecisionConverter, BridgeApp, BridgeAssetRegistry,
        MessageStatusNotifier, OutboundChannel,
    };
    use bridge_types::traits::{BridgeAssetLocker, EVMOutboundChannel};
    use bridge_types::types::{
        AssetKind, BridgeAppInfo, BridgeAssetInfo, CallOriginOutput, GenericAdditionalInboundData,
        MessageStatus,
    };
    use bridge_types::MainnetAssetId;
    use bridge_types::{EVMChainId, GenericAccount, GenericNetworkId, H256};
    use frame_support::pallet_prelude::*;
    use frame_support::traits::Get;
    use frame_system::pallet_prelude::*;
    use frame_system::{ensure_root, RawOrigin};
    use sp_runtime::traits::Zero;
    use sp_runtime::traits::{Convert, Hash};

    type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
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
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        type OutboundChannel: OutboundChannel<EVMChainId, Self::AccountId, AdditionalEVMOutboundData>
            + EVMOutboundChannel;

        type CallOrigin: EnsureOrigin<
            Self::RuntimeOrigin,
            Success = CallOriginOutput<GenericNetworkId, H256, GenericAdditionalInboundData>,
        >;

        type MessageStatusNotifier: MessageStatusNotifier<
            AssetIdOf<Self>,
            Self::AccountId,
            BalanceOf<Self>,
        >;

        type AssetRegistry: BridgeAssetRegistry<Self::AccountId, AssetIdOf<Self>>;

        type AppRegistry: AppRegistry<EVMChainId, H160>;

        type AssetIdConverter: Convert<AssetIdOf<Self>, MainnetAssetId>;

        type BalancePrecisionConverter: BalancePrecisionConverter<
            AssetIdOf<Self>,
            BalanceOf<Self>,
            U256,
        >;

        type BridgeAssetLocker: BridgeAssetLocker<Self::AccountId>;

        #[pallet::constant]
        type BaseFeeLifetime: frame_support::traits::Get<BlockNumberFor<Self>>;

        #[pallet::constant]
        type PriorityFee: frame_support::traits::Get<u128>;

        type WeightInfo: WeightInfo;
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Transfer to sidechain.
        Burned {
            network_id: EVMChainId,
            asset_id: AssetIdOf<T>,
            sender: T::AccountId,
            recipient: H160,
            amount: BalanceOf<T>,
        },
        /// Transfer from sidechain.
        Minted {
            network_id: EVMChainId,
            asset_id: AssetIdOf<T>,
            sender: H160,
            recipient: T::AccountId,
            amount: BalanceOf<T>,
        },
        /// Transfer failed, tokens refunded.
        Refunded {
            network_id: EVMChainId,
            recipient: T::AccountId,
            asset_id: AssetIdOf<T>,
            amount: BalanceOf<T>,
        },
        /// New asset registered.
        AssetRegistered {
            network_id: EVMChainId,
            asset_id: AssetIdOf<T>,
        },
        /// Fees paid by relayer in EVM was claimed.
        FeesClaimed {
            recipient: T::AccountId,
            asset_id: AssetIdOf<T>,
            amount: BalanceOf<T>,
        },
    }

    #[pallet::storage]
    #[pallet::getter(fn app_address)]
    pub(super) type AppAddresses<T: Config> =
        StorageMap<_, Identity, EVMChainId, H160, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn asset_kind)]
    pub(super) type AssetKinds<T: Config> =
        StorageDoubleMap<_, Identity, EVMChainId, Identity, AssetIdOf<T>, AssetKind, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn token_address)]
    pub(super) type TokenAddresses<T: Config> =
        StorageDoubleMap<_, Identity, EVMChainId, Identity, AssetIdOf<T>, H160, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn asset_by_address)]
    pub(super) type AssetsByAddresses<T: Config> =
        StorageDoubleMap<_, Identity, EVMChainId, Identity, H160, AssetIdOf<T>, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn sidechain_precision)]
    pub(super) type SidechainPrecision<T: Config> =
        StorageDoubleMap<_, Identity, EVMChainId, Identity, AssetIdOf<T>, u8, OptionQuery>;

    /// Collected fees
    #[pallet::storage]
    #[pallet::getter(fn collected_fees)]
    pub(super) type CollectedFees<T: Config> =
        StorageMap<_, Identity, EVMChainId, U256, ValueQuery>;

    /// Base fees
    #[pallet::storage]
    #[pallet::getter(fn base_fees)]
    pub(super) type BaseFees<T: Config> =
        StorageMap<_, Identity, EVMChainId, BaseFeeInfo<BlockNumberFor<T>>, OptionQuery>;

    /// Fees spend by relayer
    #[pallet::storage]
    #[pallet::getter(fn spent_fees)]
    pub(super) type SpentFees<T: Config> =
        StorageDoubleMap<_, Identity, EVMChainId, Identity, H160, U256, ValueQuery>;

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
        /// Wrong bridge request for refund
        WrongRequest,
        /// Wrong bridge request status, must be Failed
        WrongRequestStatus,
        BaseFeeLifetimeExceeded,
        InvalidSignature,
        NothingToClaim,
        NotEnoughFeesCollected,
        BaseFeeIsNotAvailable,
        InvalidBaseFeeUpdate,
    }

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        /// [network_id, contract]
        pub apps: Vec<(EVMChainId, H160)>,
        /// [network_id, asset_id, asset_contract, asset_kind, precision]
        pub assets: Vec<(EVMChainId, AssetIdOf<T>, H160, AssetKind, u8)>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                apps: Default::default(),
                assets: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            for (network_id, contract) in self.apps.iter() {
                AppAddresses::<T>::insert(network_id, contract);
            }
            for (network_id, asset_id, contract, asset_kind, precision) in self.assets.iter() {
                Pallet::<T>::register_asset_inner(
                    *network_id,
                    asset_id.clone(),
                    *contract,
                    *asset_kind,
                    *precision,
                )
                .unwrap();
            }
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // Internal calls to be used from Ethereum side.
        // DON'T CHANGE ORDER

        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::mint())]
        pub fn mint(
            origin: OriginFor<T>,
            token: H160,
            sender: H160,
            recipient: T::AccountId,
            amount: U256,
        ) -> DispatchResult {
            let CallOriginOutput {
                network_id: GenericNetworkId::EVM(network_id),
                additional: GenericAdditionalInboundData::EVM(additional),
                message_id,
                timepoint,
            } = T::CallOrigin::ensure_origin(origin)? else {
                frame_support::fail!(DispatchError::BadOrigin);
            };
            let asset_id = AssetsByAddresses::<T>::get(network_id, token)
                // should never return this error, because called from Ethereum
                .ok_or(Error::<T>::TokenIsNotRegistered)?;
            let asset_kind = AssetKinds::<T>::get(network_id, &asset_id)
                .ok_or(Error::<T>::TokenIsNotRegistered)?;
            let app_address =
                AppAddresses::<T>::get(network_id).ok_or(Error::<T>::AppIsNotRegistered)?;
            let sidechain_precision = SidechainPrecision::<T>::get(network_id, &asset_id)
                .ok_or(Error::<T>::TokenIsNotRegistered)?;

            if additional.source != app_address {
                return Err(DispatchError::BadOrigin);
            }

            let (amount, _) = T::BalancePrecisionConverter::from_sidechain(
                &asset_id,
                sidechain_precision,
                amount,
            )
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
                GenericNetworkId::EVM(network_id),
                message_id,
                GenericAccount::EVM(sender),
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

        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::register_asset_internal())]
        pub fn register_asset_internal(
            origin: OriginFor<T>,
            asset_id: AssetIdOf<T>,
            contract: H160,
        ) -> DispatchResult {
            let CallOriginOutput {
                network_id: GenericNetworkId::EVM(network_id),
                additional: GenericAdditionalInboundData::EVM(additional),
                ..
            } = T::CallOrigin::ensure_origin(origin)? else {
                frame_support::fail!(DispatchError::BadOrigin);
            };

            let app_address =
                AppAddresses::<T>::get(network_id).ok_or(Error::<T>::AppIsNotRegistered)?;
            if additional.source != app_address {
                return Err(DispatchError::BadOrigin);
            }

            let asset_info = T::AssetRegistry::get_raw_info(asset_id.clone());
            Self::register_asset_inner(
                network_id,
                asset_id,
                contract,
                AssetKind::Thischain,
                asset_info.precision,
            )?;
            Ok(())
        }

        // Common exstrinsics

        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::burn())]
        pub fn burn(
            origin: OriginFor<T>,
            network_id: EVMChainId,
            asset_id: AssetIdOf<T>,
            recipient: H160,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            Self::burn_inner(who, network_id, asset_id, recipient, amount)?;

            Ok(())
        }

        #[pallet::call_index(3)]
        #[pallet::weight(<T as Config>::WeightInfo::register_sidechain_asset())]
        pub fn register_sidechain_asset(
            origin: OriginFor<T>,
            network_id: EVMChainId,
            address: H160,
            symbol: AssetSymbolOf<T>,
            name: AssetNameOf<T>,
            decimals: u8,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(
                !AssetsByAddresses::<T>::contains_key(network_id, address),
                Error::<T>::TokenAlreadyRegistered
            );
            let target =
                AppAddresses::<T>::get(network_id).ok_or(Error::<T>::AppIsNotRegistered)?;

            let asset_id = T::AssetRegistry::register_asset(network_id.into(), name, symbol)?;

            Self::register_asset_inner(
                network_id,
                asset_id,
                address,
                AssetKind::Sidechain,
                decimals,
            )?;

            let message = AddTokenToWhitelistPayload {
                address,
                asset_kind: payload::EthAbiAssetKind::Evm,
            };

            T::OutboundChannel::submit(
                network_id,
                &RawOrigin::Root,
                &message.encode().map_err(|_| Error::<T>::CallEncodeFailed)?,
                AdditionalEVMOutboundData {
                    target,
                    max_gas: 100000u64.into(),
                },
            )?;
            Ok(())
        }

        #[pallet::call_index(4)]
        #[pallet::weight(<T as Config>::WeightInfo::register_existing_sidechain_asset())]
        pub fn register_existing_sidechain_asset(
            origin: OriginFor<T>,
            network_id: EVMChainId,
            address: H160,
            asset_id: AssetIdOf<T>,
            decimals: u8,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(
                !AssetsByAddresses::<T>::contains_key(network_id, address),
                Error::<T>::TokenAlreadyRegistered
            );
            let target =
                AppAddresses::<T>::get(network_id).ok_or(Error::<T>::AppIsNotRegistered)?;

            Self::register_asset_inner(
                network_id,
                asset_id,
                address,
                AssetKind::Sidechain,
                decimals,
            )?;

            let message = AddTokenToWhitelistPayload {
                address,
                asset_kind: payload::EthAbiAssetKind::Evm,
            };

            T::OutboundChannel::submit(
                network_id,
                &RawOrigin::Root,
                &message.encode().map_err(|_| Error::<T>::CallEncodeFailed)?,
                AdditionalEVMOutboundData {
                    target,
                    max_gas: 100000u64.into(),
                },
            )?;
            Ok(())
        }

        #[pallet::call_index(5)]
        #[pallet::weight(<T as Config>::WeightInfo::register_thischain_asset())]
        pub fn register_thischain_asset(
            origin: OriginFor<T>,
            network_id: EVMChainId,
            asset_id: AssetIdOf<T>,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(
                !TokenAddresses::<T>::contains_key(network_id, &asset_id),
                Error::<T>::TokenAlreadyRegistered
            );
            let target =
                AppAddresses::<T>::get(network_id).ok_or(Error::<T>::AppIsNotRegistered)?;
            let asset_info = T::AssetRegistry::get_raw_info(asset_id.clone());

            let message = RegisterNativeAssetPayload {
                asset_id: T::AssetIdConverter::convert(asset_id),
                name: asset_info.name,
                symbol: asset_info.symbol,
            };

            T::OutboundChannel::submit(
                network_id,
                &RawOrigin::Root,
                &message.encode().map_err(|_| Error::<T>::CallEncodeFailed)?,
                AdditionalEVMOutboundData {
                    target,
                    max_gas: 2000000u64.into(),
                },
            )?;
            Ok(())
        }

        #[pallet::call_index(6)]
        #[pallet::weight(<T as Config>::WeightInfo::register_native_app())]
        pub fn register_network(
            origin: OriginFor<T>,
            network_id: EVMChainId,
            contract: H160,
            symbol: AssetSymbolOf<T>,
            name: AssetNameOf<T>,
            sidechain_precision: u8,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(
                !AppAddresses::<T>::contains_key(network_id),
                Error::<T>::AppAlreadyRegistered
            );
            let asset_id = T::AssetRegistry::register_asset(network_id.into(), name, symbol)?;
            AppAddresses::<T>::insert(network_id, contract);
            T::AppRegistry::register_app(network_id, contract)?;
            Self::register_asset_inner(
                network_id,
                asset_id,
                H160::zero(),
                AssetKind::Sidechain,
                sidechain_precision,
            )?;
            Ok(())
        }

        #[pallet::call_index(7)]
        #[pallet::weight(<T as Config>::WeightInfo::register_existing_native_app())]
        pub fn register_network_with_existing_asset(
            origin: OriginFor<T>,
            network_id: EVMChainId,
            contract: H160,
            asset_id: AssetIdOf<T>,
            sidechain_precision: u8,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(
                !AppAddresses::<T>::contains_key(network_id),
                Error::<T>::AppAlreadyRegistered
            );
            AppAddresses::<T>::insert(network_id, contract);
            T::AppRegistry::register_app(network_id, contract)?;
            Self::register_asset_inner(
                network_id,
                asset_id,
                H160::zero(),
                AssetKind::Sidechain,
                sidechain_precision,
            )?;
            Ok(())
        }

        #[pallet::call_index(8)]
        #[pallet::weight(<T as Config>::WeightInfo::register_native_app())]
        pub fn claim_relayer_fees(
            origin: OriginFor<T>,
            network_id: EVMChainId,
            relayer: H160,
            signature: sp_core::ecdsa::Signature,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_can_claim_relayer_fees(&who, network_id, relayer, signature)?;
            let spent_fees = SpentFees::<T>::get(network_id, relayer);
            ensure!(spent_fees > U256::zero(), Error::<T>::NothingToClaim);

            let collected_fees = CollectedFees::<T>::get(network_id);
            ensure!(
                collected_fees > U256::zero(),
                Error::<T>::NotEnoughFeesCollected
            );
            let fees_to_claim = collected_fees.min(spent_fees);

            let fee_asset = Self::get_network_fee_asset(network_id)?;
            let sidechain_precision = SidechainPrecision::<T>::get(network_id, &fee_asset)
                .ok_or(Error::<T>::TokenIsNotRegistered)?;

            let (amount, _) = T::BalancePrecisionConverter::from_sidechain(
                &fee_asset,
                sidechain_precision,
                fees_to_claim,
            )
            .ok_or(Error::<T>::WrongAmount)?;
            ensure!(amount > Zero::zero(), Error::<T>::WrongAmount);
            T::BridgeAssetLocker::refund_fee(network_id.into(), &who, &fee_asset, &amount)?;
            Self::deposit_event(Event::FeesClaimed {
                asset_id: fee_asset,
                recipient: who,
                amount,
            });

            SpentFees::<T>::insert(
                network_id,
                relayer,
                spent_fees.saturating_sub(fees_to_claim),
            );
            CollectedFees::<T>::set(network_id, collected_fees.saturating_sub(fees_to_claim));

            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        pub fn get_claim_prehashed_message(network_id: EVMChainId, who: &T::AccountId) -> H256 {
            sp_runtime::traits::Keccak256::hash_of(&(
                "claim-relayer-fees",
                &who,
                frame_system::Pallet::<T>::account_nonce(who),
                network_id,
            ))
        }

        pub fn ensure_can_claim_relayer_fees(
            who: &T::AccountId,
            network_id: EVMChainId,
            relayer: H160,
            signature: sp_core::ecdsa::Signature,
        ) -> DispatchResult {
            let message = Self::get_claim_prehashed_message(network_id, who);
            let pk = sp_io::crypto::secp256k1_ecdsa_recover(&signature.0, &message.0)
                .map_err(|_| Error::<T>::InvalidSignature)?;
            let recovered_address =
                H160::from_slice(&sp_runtime::traits::Keccak256::hash(&pk)[12..]);
            ensure!(recovered_address == relayer, Error::<T>::InvalidSignature);
            Ok(())
        }

        pub fn register_asset_inner(
            network_id: EVMChainId,
            asset_id: AssetIdOf<T>,
            contract: H160,
            asset_kind: AssetKind,
            sidechain_precision: u8,
        ) -> DispatchResult {
            ensure!(
                AppAddresses::<T>::contains_key(network_id),
                Error::<T>::AppIsNotRegistered
            );
            ensure!(
                !TokenAddresses::<T>::contains_key(network_id, &asset_id),
                Error::<T>::TokenAlreadyRegistered
            );
            TokenAddresses::<T>::insert(network_id, &asset_id, contract);
            AssetsByAddresses::<T>::insert(network_id, contract, &asset_id);
            AssetKinds::<T>::insert(network_id, &asset_id, asset_kind);
            SidechainPrecision::<T>::insert(network_id, &asset_id, sidechain_precision);
            T::AssetRegistry::manage_asset(network_id.into(), asset_id.clone())?;
            Self::deposit_event(Event::AssetRegistered {
                network_id,
                asset_id,
            });
            Ok(())
        }

        pub fn burn_inner(
            who: T::AccountId,
            network_id: EVMChainId,
            asset_id: AssetIdOf<T>,
            recipient: H160,
            amount: BalanceOf<T>,
        ) -> Result<H256, DispatchError> {
            let asset_kind = AssetKinds::<T>::get(network_id, &asset_id)
                .ok_or(Error::<T>::TokenIsNotRegistered)?;
            let target =
                AppAddresses::<T>::get(network_id).ok_or(Error::<T>::AppIsNotRegistered)?;
            let sidechain_precision = SidechainPrecision::<T>::get(network_id, &asset_id)
                .ok_or(Error::<T>::TokenIsNotRegistered)?;

            let (_, sidechain_amount) = T::BalancePrecisionConverter::to_sidechain(
                &asset_id,
                sidechain_precision,
                amount.clone(),
            )
            .ok_or(Error::<T>::WrongAmount)?;

            ensure!(sidechain_amount > 0.into(), Error::<T>::WrongAmount);

            T::BridgeAssetLocker::lock_asset(
                network_id.into(),
                asset_kind,
                &who,
                &asset_id,
                &amount,
            )?;

            let token_address = TokenAddresses::<T>::get(network_id, &asset_id)
                .ok_or(Error::<T>::TokenIsNotRegistered)?;

            let message = MintPayload {
                token: token_address,
                sender: who.clone(),
                recipient,
                amount: sidechain_amount,
            };

            let message_id = T::OutboundChannel::submit(
                network_id,
                &RawOrigin::Signed(who.clone()),
                &message.encode().map_err(|_| Error::<T>::CallEncodeFailed)?,
                AdditionalEVMOutboundData {
                    target,
                    max_gas: TRANSFER_MAX_GAS.into(),
                },
            )?;
            T::MessageStatusNotifier::outbound_request(
                GenericNetworkId::EVM(network_id),
                message_id,
                who.clone(),
                GenericAccount::EVM(recipient),
                asset_id.clone(),
                amount.clone(),
                MessageStatus::InQueue,
            );
            Self::deposit_event(Event::Burned {
                network_id,
                asset_id,
                sender: who,
                recipient,
                amount,
            });

            Ok(message_id)
        }

        pub fn refund_inner(
            network_id: EVMChainId,
            recipient: T::AccountId,
            asset_id: AssetIdOf<T>,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            ensure!(amount > Zero::zero(), Error::<T>::WrongAmount);

            let asset_kind = AssetKinds::<T>::get(network_id, &asset_id)
                .ok_or(Error::<T>::TokenIsNotRegistered)?;
            T::BridgeAssetLocker::unlock_asset(
                network_id.into(),
                asset_kind,
                &recipient,
                &asset_id,
                &amount,
            )?;

            Self::deposit_event(Event::Refunded {
                network_id,
                recipient,
                asset_id,
                amount,
            });

            Ok(())
        }
    }

    impl<T: Config> BridgeApp<T::AccountId, H160, AssetIdOf<T>, BalanceOf<T>> for Pallet<T> {
        fn is_asset_supported(network_id: GenericNetworkId, asset_id: AssetIdOf<T>) -> bool {
            let GenericNetworkId::EVM(network_id) = network_id else {
                return false;
            };
            TokenAddresses::<T>::get(network_id, asset_id).is_some()
        }

        fn transfer(
            network_id: GenericNetworkId,
            asset_id: AssetIdOf<T>,
            sender: T::AccountId,
            recipient: H160,
            amount: BalanceOf<T>,
        ) -> Result<H256, DispatchError> {
            let network_id = network_id.evm().ok_or(Error::<T>::InvalidNetwork)?;
            Pallet::<T>::burn_inner(sender, network_id, asset_id, recipient, amount)
        }

        fn refund(
            network_id: GenericNetworkId,
            _message_id: H256,
            recipient: T::AccountId,
            asset_id: AssetIdOf<T>,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let network_id = network_id.evm().ok_or(Error::<T>::InvalidNetwork)?;
            Pallet::<T>::refund_inner(network_id, recipient, asset_id, amount)
        }

        fn list_supported_assets(network_id: GenericNetworkId) -> Vec<BridgeAssetInfo> {
            let GenericNetworkId::EVM(network_id) = network_id else {
                return vec![];
            };
            AssetKinds::<T>::iter_prefix(network_id)
                .filter_map(|(asset_id, _asset_kind)| {
                    TokenAddresses::<T>::get(network_id, &asset_id)
                        .zip(SidechainPrecision::<T>::get(network_id, &asset_id))
                        .map(|(evm_address, precision)| {
                            Some(BridgeAssetInfo::EVM(EVMAssetInfo {
                                asset_id: T::AssetIdConverter::convert(asset_id),
                                app_kind: EVMAppKind::FAApp,
                                evm_address,
                                precision,
                            }))
                        })
                        .unwrap_or_default()
                })
                .collect()
        }

        fn list_apps() -> Vec<BridgeAppInfo> {
            AppAddresses::<T>::iter()
                .map(|(network_id, evm_address)| {
                    BridgeAppInfo::EVM(
                        network_id.into(),
                        EVMAppInfo {
                            app_kind: EVMAppKind::FAApp,
                            evm_address,
                        },
                    )
                })
                .collect()
        }

        fn is_asset_supported_weight() -> Weight {
            T::DbWeight::get().reads(1)
        }

        fn refund_weight() -> Weight {
            Default::default()
        }

        fn transfer_weight() -> Weight {
            <T as Config>::WeightInfo::burn()
        }
    }
}

impl<T: Config> bridge_types::traits::EVMBridgeWithdrawFee<T::AccountId, AssetIdOf<T>>
    for Pallet<T>
{
    fn withdraw_transfer_fee(
        who: &T::AccountId,
        chain_id: bridge_types::EVMChainId,
        _asset_id: AssetIdOf<T>,
    ) -> DispatchResult {
        let gas = T::OutboundChannel::submit_gas(chain_id)?.saturating_add(TRANSFER_MAX_GAS.into());
        let fee_asset = Self::get_network_fee_asset(chain_id)?;
        let base_fee =
            Self::get_latest_base_fee(chain_id)?.saturating_add(T::PriorityFee::get().into());
        let fee = gas.saturating_mul(base_fee);
        let sidechain_precision = SidechainPrecision::<T>::get(chain_id, &fee_asset)
            .ok_or(Error::<T>::TokenIsNotRegistered)?;

        let (amount, _) =
            T::BalancePrecisionConverter::from_sidechain(&fee_asset, sidechain_precision, fee)
                .ok_or(Error::<T>::WrongAmount)?;
        ensure!(amount > Zero::zero(), Error::<T>::WrongAmount);
        T::BridgeAssetLocker::withdraw_fee(chain_id.into(), who, &fee_asset, &amount)?;
        CollectedFees::<T>::mutate(chain_id, |fees| {
            *fees = fees.saturating_add(fee);
        });
        Ok(())
    }
}

impl<T: Config> EVMFeeHandler<AssetIdOf<T>> for Pallet<T> {
    fn get_latest_base_fee(network_id: EVMChainId) -> Result<U256, DispatchError> {
        let base_fee = BaseFees::<T>::get(network_id).ok_or(Error::<T>::BaseFeeIsNotAvailable)?;
        ensure!(
            frame_system::Pallet::<T>::block_number()
                <= base_fee.updated + T::BaseFeeLifetime::get(),
            Error::<T>::BaseFeeLifetimeExceeded
        );
        Ok(base_fee.base_fee)
    }
    fn get_network_fee_asset(network_id: EVMChainId) -> Result<AssetIdOf<T>, DispatchError> {
        let asset_id = AssetsByAddresses::<T>::get(network_id, H160::zero())
            .ok_or(Error::<T>::AppIsNotRegistered)?;
        Ok(asset_id)
    }

    fn on_fee_paid(network_id: EVMChainId, relayer: H160, amount: U256) {
        SpentFees::<T>::mutate(network_id, relayer, |fees| {
            *fees = fees.saturating_add(amount);
        })
    }

    fn can_update_base_fee(
        network_id: EVMChainId,
        _new_base_fee: U256,
        evm_block_number: u64,
    ) -> bool {
        let Ok(base_fee) = BaseFees::<T>::get(network_id).ok_or(Error::<T>::BaseFeeIsNotAvailable) else {
            // Probably it's first base fee update
            return true;
        };
        base_fee.evm_block_number < evm_block_number
    }

    fn update_base_fee(network_id: EVMChainId, new_base_fee: U256, evm_block_number: u64) {
        if Self::can_update_base_fee(network_id, new_base_fee, evm_block_number) {
            let block_number = frame_system::Pallet::<T>::block_number();
            BaseFees::<T>::insert(
                network_id,
                BaseFeeInfo {
                    base_fee: new_base_fee,
                    updated: block_number,
                    evm_block_number,
                },
            );
        }
    }
}
