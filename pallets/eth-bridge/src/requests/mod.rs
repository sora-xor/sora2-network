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

use crate::contract::{ContractEvent, DepositEvent};
use crate::offchain::SignatureParams;
use crate::{
    BridgeNetworkId, BridgeTimepoint, Config, Error, EthAddress, Pallet, PeerAccountId,
    RequestStatuses, SidechainAssetPrecision, Timepoint,
};
use codec::{Decode, Encode};
use common::AssetInfoProvider;
use common::Denominator;
use ethabi::Token;
use ethereum_types::U256;
use frame_support::dispatch::{DispatchError, DispatchResult};
use frame_support::log::warn;
use frame_support::sp_runtime::app_crypto::sp_core;
use frame_support::{ensure, sp_io, RuntimeDebug};
pub use incoming::*;
pub use outgoing::*;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_core::H256;
use sp_io::hashing::blake2_256;
use sp_std::prelude::*;

pub mod encode_packed;
mod incoming;
mod outgoing;

type Assets<T> = assets::Pallet<T>;

/// Outgoing (Thischain->Sidechain) request.
///
/// Each request, has the following properties: author, nonce, network ID, and hash (calculates
/// just-in-time).
/// And the following methods: validate, prepare, finalize, cancel.
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[scale_info(skip_type_params(T))]
pub enum OutgoingRequest<T: Config> {
    /// Outgoing transfer from Substrate to Ethereum request.
    Transfer(OutgoingTransfer<T>),
    /// 'Add new Substrate asset' request.
    AddAsset(OutgoingAddAsset<T>),
    /// 'Add new Ethereum token' request.
    AddToken(OutgoingAddToken<T>),
    /// 'Add peer' request.
    AddPeer(OutgoingAddPeer<T>),
    /// 'Remove peer' request.
    RemovePeer(OutgoingRemovePeer<T>),
    /// 'Prepare for migration' request.
    PrepareForMigration(OutgoingPrepareForMigration<T>),
    /// 'Migrate' request.
    Migrate(OutgoingMigrate<T>),
    /// 'Add peer compat' request.
    AddPeerCompat(OutgoingAddPeerCompat<T>),
    /// 'Remove peer compat' request.
    RemovePeerCompat(OutgoingRemovePeerCompat<T>),
}

impl<T: Config> OutgoingRequest<T> {
    pub fn author(&self) -> &T::AccountId {
        match self {
            OutgoingRequest::Transfer(transfer) => &transfer.from,
            OutgoingRequest::AddAsset(request) => &request.author,
            OutgoingRequest::AddToken(request) => &request.author,
            OutgoingRequest::AddPeer(request) => &request.author,
            OutgoingRequest::RemovePeer(request) => &request.author,
            OutgoingRequest::PrepareForMigration(request) => &request.author,
            OutgoingRequest::Migrate(request) => &request.author,
            OutgoingRequest::AddPeerCompat(request) => &request.author,
            OutgoingRequest::RemovePeerCompat(request) => &request.author,
        }
    }

    /// Encodes the request to a corresponding Ethereum contract function's arguments.
    /// Also, serializes some parameters with `encode` to be signed by peers.
    pub fn to_eth_abi(&self, tx_hash: H256) -> Result<OutgoingRequestEncoded, Error<T>> {
        match self {
            OutgoingRequest::Transfer(transfer) => transfer
                .to_eth_abi(tx_hash)
                .map(OutgoingRequestEncoded::Transfer),
            OutgoingRequest::AddAsset(request) => request
                .to_eth_abi(tx_hash)
                .map(OutgoingRequestEncoded::AddAsset),
            OutgoingRequest::AddToken(request) => request
                .to_eth_abi(tx_hash)
                .map(OutgoingRequestEncoded::AddToken),
            OutgoingRequest::AddPeer(request) => request
                .to_eth_abi(tx_hash)
                .map(OutgoingRequestEncoded::AddPeer),
            OutgoingRequest::RemovePeer(request) => request
                .to_eth_abi(tx_hash)
                .map(OutgoingRequestEncoded::RemovePeer),
            OutgoingRequest::PrepareForMigration(request) => request
                .to_eth_abi(tx_hash)
                .map(OutgoingRequestEncoded::PrepareForMigration),
            OutgoingRequest::Migrate(request) => request
                .to_eth_abi(tx_hash)
                .map(OutgoingRequestEncoded::Migrate),
            OutgoingRequest::AddPeerCompat(request) => request
                .to_eth_abi(tx_hash)
                .map(OutgoingRequestEncoded::AddPeer),
            OutgoingRequest::RemovePeerCompat(request) => request
                .to_eth_abi(tx_hash)
                .map(OutgoingRequestEncoded::RemovePeer),
        }
    }

    pub fn network_id(&self) -> T::NetworkId {
        match self {
            OutgoingRequest::Transfer(request) => request.network_id,
            OutgoingRequest::AddAsset(request) => request.network_id,
            OutgoingRequest::AddToken(request) => request.network_id,
            OutgoingRequest::AddPeer(request) => request.network_id,
            OutgoingRequest::RemovePeer(request) => request.network_id,
            OutgoingRequest::PrepareForMigration(request) => request.network_id,
            OutgoingRequest::Migrate(request) => request.network_id,
            OutgoingRequest::AddPeerCompat(request) => request.network_id,
            OutgoingRequest::RemovePeerCompat(request) => request.network_id,
        }
    }

    fn timepoint(&self) -> Timepoint<T> {
        match self {
            OutgoingRequest::Transfer(request) => request.timepoint,
            OutgoingRequest::AddAsset(request) => request.timepoint,
            OutgoingRequest::AddToken(request) => request.timepoint,
            OutgoingRequest::AddPeer(request) => request.timepoint,
            OutgoingRequest::RemovePeer(request) => request.timepoint,
            OutgoingRequest::PrepareForMigration(request) => request.timepoint,
            OutgoingRequest::Migrate(request) => request.timepoint,
            OutgoingRequest::AddPeerCompat(request) => request.timepoint,
            OutgoingRequest::RemovePeerCompat(request) => request.timepoint,
        }
    }

    /// Checks that the request can be initiated (e.g., verifies that an account has
    /// sufficient funds for transfer).
    fn validate(&self) -> Result<(), DispatchError> {
        match self {
            OutgoingRequest::Transfer(request) => request.validate(),
            OutgoingRequest::AddAsset(request) => request.validate(),
            OutgoingRequest::AddToken(request) => request.validate().map(|_| ()),
            OutgoingRequest::AddPeer(request) => request.validate().map(|_| ()),
            OutgoingRequest::RemovePeer(request) => request.validate().map(|_| ()),
            OutgoingRequest::PrepareForMigration(request) => request.validate().map(|_| ()),
            OutgoingRequest::Migrate(request) => request.validate().map(|_| ()),
            OutgoingRequest::AddPeerCompat(request) => request.validate().map(|_| ()),
            OutgoingRequest::RemovePeerCompat(request) => request.validate().map(|_| ()),
        }
    }

    fn prepare(&self, tx_hash: H256) -> Result<(), DispatchError> {
        match self {
            OutgoingRequest::Transfer(request) => request.prepare(tx_hash),
            OutgoingRequest::AddAsset(request) => request.prepare(()),
            OutgoingRequest::AddToken(request) => request.prepare(()),
            OutgoingRequest::AddPeer(request) => request.prepare(()),
            OutgoingRequest::RemovePeer(request) => request.prepare(()),
            OutgoingRequest::PrepareForMigration(request) => request.prepare(()),
            OutgoingRequest::Migrate(request) => request.prepare(()),
            OutgoingRequest::AddPeerCompat(request) => request.prepare(()),
            OutgoingRequest::RemovePeerCompat(request) => request.prepare(()),
        }
    }

    pub(crate) fn finalize(&self, tx_hash: H256) -> Result<(), DispatchError> {
        match self {
            OutgoingRequest::Transfer(request) => request.finalize(tx_hash),
            OutgoingRequest::AddAsset(request) => request.finalize(),
            OutgoingRequest::AddToken(request) => request.finalize(),
            OutgoingRequest::AddPeer(request) => request.finalize(),
            OutgoingRequest::RemovePeer(request) => request.finalize(),
            OutgoingRequest::PrepareForMigration(request) => request.finalize(),
            OutgoingRequest::Migrate(request) => request.finalize(),
            OutgoingRequest::AddPeerCompat(request) => request.finalize(),
            OutgoingRequest::RemovePeerCompat(request) => request.finalize(),
        }
    }

    pub fn cancel(&self) -> Result<(), DispatchError> {
        match self {
            OutgoingRequest::Transfer(request) => request.cancel(),
            OutgoingRequest::AddAsset(request) => request.cancel(),
            OutgoingRequest::AddToken(request) => request.cancel(),
            OutgoingRequest::AddPeer(request) => request.cancel(),
            OutgoingRequest::RemovePeer(request) => request.cancel(),
            OutgoingRequest::PrepareForMigration(request) => request.cancel(),
            OutgoingRequest::Migrate(request) => request.cancel(),
            OutgoingRequest::AddPeerCompat(request) => request.cancel(),
            OutgoingRequest::RemovePeerCompat(request) => request.cancel(),
        }
    }

    pub fn is_allowed_during_migration(&self) -> bool {
        matches!(self, OutgoingRequest::Migrate(_))
    }

    pub fn should_be_skipped(&self) -> bool {
        match self {
            OutgoingRequest::RemovePeer(req) => req.should_be_skipped(),
            _ => false,
        }
    }
}

/// Types of transaction-requests that can be made from a sidechain.
#[derive(Clone, Copy, Encode, Decode, RuntimeDebug, PartialEq, Eq, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum IncomingTransactionRequestKind {
    Transfer,
    AddAsset,
    AddPeer,
    RemovePeer,
    PrepareForMigration,
    Migrate,
    AddPeerCompat,
    RemovePeerCompat,
    /// A special case of transferring XOR asset with post-taking fees.
    TransferXOR,
}

/// Types of meta-requests that can be made.
#[derive(Clone, Copy, Encode, Decode, RuntimeDebug, PartialEq, Eq, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum IncomingMetaRequestKind {
    CancelOutgoingRequest,
    MarkAsDone,
}

/// Types of requests that can be made from a sidechain.
#[derive(Clone, Copy, Encode, Decode, RuntimeDebug, PartialEq, Eq, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum IncomingRequestKind {
    Transaction(IncomingTransactionRequestKind),
    Meta(IncomingMetaRequestKind),
}

impl From<IncomingTransactionRequestKind> for IncomingRequestKind {
    fn from(v: IncomingTransactionRequestKind) -> Self {
        Self::Transaction(v)
    }
}

impl From<IncomingMetaRequestKind> for IncomingRequestKind {
    fn from(v: IncomingMetaRequestKind) -> Self {
        Self::Meta(v)
    }
}

impl IncomingTransactionRequestKind {
    /// Returns `true` if the request should be used with XOR and VAL contracts.
    pub fn is_compat(&self) -> bool {
        *self == Self::AddPeerCompat || *self == Self::RemovePeerCompat
    }
}

/// Incoming (Sidechain->Thischain) request.
///
/// Each request, has the following properties: transaction hash, height, network ID, and timepoint.
/// And the following methods: validate, prepare, finalize, cancel.
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[scale_info(skip_type_params(T))]
pub enum IncomingRequest<T: Config> {
    Transfer(IncomingTransfer<T>),
    AddToken(IncomingAddToken<T>),
    ChangePeers(IncomingChangePeers<T>),
    CancelOutgoingRequest(IncomingCancelOutgoingRequest<T>),
    MarkAsDone(IncomingMarkAsDoneRequest<T>),
    PrepareForMigration(IncomingPrepareForMigration<T>),
    Migrate(IncomingMigrate<T>),
    ChangePeersCompat(IncomingChangePeersCompat<T>),
}

impl<T: Config> IncomingRequest<T> {
    pub fn try_from_contract_event(
        event: ContractEvent<EthAddress, T::AccountId, U256>,
        incoming_request: LoadIncomingTransactionRequest<T>,
        at_height: u64,
    ) -> Result<Self, Error<T>> {
        let network_id = incoming_request.network_id;
        let timepoint = incoming_request.timepoint;
        let author = incoming_request.author;
        let tx_hash = incoming_request.hash;

        let req = match event {
            ContractEvent::Deposit(DepositEvent {
                destination: to,
                amount,
                token: token_address,
                sidechain_asset: raw_asset_id,
            }) => {
                let (asset_id, asset_kind) = Pallet::<T>::get_asset_by_raw_asset_id(
                    raw_asset_id,
                    &token_address,
                    network_id,
                )?
                .ok_or(Error::<T>::UnsupportedAssetId)?;
                let denomination_factor = T::Denominator::current_factor(&asset_id);
                let amount = u128::try_from(
                    amount
                        .checked_div(denomination_factor.into())
                        .ok_or(Error::<T>::FailedToApplyDenomination)?,
                )
                .map_err(|_| Error::<T>::InvalidAmount)?;
                let amount = if !asset_kind.is_owned() {
                    let sidechain_precision =
                        SidechainAssetPrecision::<T>::get(network_id, &asset_id);
                    let thischain_precision = assets::Pallet::<T>::get_asset_info(&asset_id).2;
                    Pallet::<T>::convert_precision(
                        sidechain_precision,
                        thischain_precision,
                        amount,
                    )?
                    .0
                } else {
                    amount
                };
                IncomingRequest::Transfer(IncomingTransfer {
                    from: Default::default(),
                    to,
                    asset_id,
                    asset_kind,
                    amount,
                    author,
                    tx_hash,
                    at_height,
                    timepoint,
                    network_id,
                    should_take_fee: false,
                })
            }
            ContractEvent::ChangePeers(peer_address, removed) => {
                let peer_account_id = PeerAccountId::<T>::get(network_id, &peer_address);
                ensure!(
                    removed || peer_account_id.is_some(),
                    Error::<T>::UnknownPeerAddress
                );
                IncomingRequest::ChangePeers(IncomingChangePeers {
                    peer_account_id,
                    peer_address,
                    removed,
                    author,
                    tx_hash,
                    at_height,
                    timepoint,
                    network_id,
                })
            }
            ContractEvent::PreparedForMigration => {
                IncomingRequest::PrepareForMigration(IncomingPrepareForMigration {
                    author,
                    tx_hash,
                    at_height,
                    timepoint,
                    network_id,
                })
            }
            ContractEvent::Migrated(to) => IncomingRequest::Migrate(IncomingMigrate {
                new_contract_address: to,
                author,
                tx_hash,
                at_height,
                timepoint,
                network_id,
            }),
        };
        Ok(req)
    }

    pub(crate) fn hash(&self) -> H256 {
        match self {
            IncomingRequest::Transfer(request) => request.tx_hash,
            IncomingRequest::AddToken(request) => request.tx_hash,
            IncomingRequest::ChangePeers(request) => request.tx_hash,
            IncomingRequest::CancelOutgoingRequest(request) => request.initial_request_hash,
            IncomingRequest::MarkAsDone(request) => request.initial_request_hash,
            IncomingRequest::PrepareForMigration(request) => request.tx_hash,
            IncomingRequest::Migrate(request) => request.tx_hash,
            IncomingRequest::ChangePeersCompat(request) => request.tx_hash,
        }
    }

    pub fn network_id(&self) -> T::NetworkId {
        match self {
            IncomingRequest::Transfer(request) => request.network_id,
            IncomingRequest::AddToken(request) => request.network_id,
            IncomingRequest::ChangePeers(request) => request.network_id,
            IncomingRequest::CancelOutgoingRequest(request) => request.network_id,
            IncomingRequest::MarkAsDone(request) => request.network_id,
            IncomingRequest::PrepareForMigration(request) => request.network_id,
            IncomingRequest::Migrate(request) => request.network_id,
            IncomingRequest::ChangePeersCompat(request) => request.network_id,
        }
    }

    /// A sidechain height at which the sidechain transaction was added.
    pub(crate) fn at_height(&self) -> u64 {
        match self {
            IncomingRequest::Transfer(request) => request.at_height,
            IncomingRequest::AddToken(request) => request.at_height,
            IncomingRequest::ChangePeers(request) => request.at_height,
            IncomingRequest::CancelOutgoingRequest(request) => request.at_height,
            IncomingRequest::MarkAsDone(request) => request.at_height,
            IncomingRequest::PrepareForMigration(request) => request.at_height,
            IncomingRequest::Migrate(request) => request.at_height,
            IncomingRequest::ChangePeersCompat(request) => request.at_height,
        }
    }

    pub fn validate(&self) -> Result<(), DispatchError> {
        match self {
            IncomingRequest::Transfer(request) => request.validate(),
            IncomingRequest::AddToken(_request) => Ok(()),
            IncomingRequest::ChangePeers(_request) => Ok(()),
            IncomingRequest::CancelOutgoingRequest(_request) => Ok(()),
            IncomingRequest::MarkAsDone(request) => request.validate(),
            IncomingRequest::PrepareForMigration(_request) => Ok(()),
            IncomingRequest::Migrate(_request) => Ok(()),
            IncomingRequest::ChangePeersCompat(_request) => Ok(()),
        }
    }

    pub fn prepare(&self) -> Result<(), DispatchError> {
        match self {
            IncomingRequest::Transfer(request) => request.prepare(),
            IncomingRequest::AddToken(_request) => Ok(()),
            IncomingRequest::ChangePeers(_request) => Ok(()),
            IncomingRequest::CancelOutgoingRequest(request) => request.prepare(),
            IncomingRequest::MarkAsDone(request) => request.prepare(),
            IncomingRequest::PrepareForMigration(request) => request.prepare(),
            IncomingRequest::Migrate(request) => request.prepare(),
            IncomingRequest::ChangePeersCompat(_request) => Ok(()),
        }
    }

    pub fn cancel(&self) -> Result<(), DispatchError> {
        match self {
            IncomingRequest::Transfer(request) => request.cancel(),
            IncomingRequest::AddToken(_request) => Ok(()),
            IncomingRequest::ChangePeers(_request) => Ok(()),
            IncomingRequest::CancelOutgoingRequest(request) => request.cancel(),
            IncomingRequest::MarkAsDone(request) => request.cancel(),
            IncomingRequest::PrepareForMigration(request) => request.cancel(),
            IncomingRequest::Migrate(request) => request.cancel(),
            IncomingRequest::ChangePeersCompat(_request) => Ok(()),
        }
    }

    pub fn finalize(&self) -> Result<H256, DispatchError> {
        match self {
            IncomingRequest::Transfer(request) => request.finalize(),
            IncomingRequest::AddToken(request) => request.finalize(),
            IncomingRequest::ChangePeers(request) => request.finalize(),
            IncomingRequest::CancelOutgoingRequest(request) => request.finalize(),
            IncomingRequest::MarkAsDone(request) => request.finalize(),
            IncomingRequest::PrepareForMigration(request) => request.finalize(),
            IncomingRequest::Migrate(request) => request.finalize(),
            IncomingRequest::ChangePeersCompat(request) => request.finalize(),
        }
    }

    /// A timepoint at which the request was registered in thischain. Used my the `bridge-multisig`
    /// pallet.
    pub fn timepoint(&self) -> Timepoint<T> {
        match self {
            IncomingRequest::Transfer(request) => request.timepoint(),
            IncomingRequest::AddToken(request) => request.timepoint(),
            IncomingRequest::ChangePeers(request) => request.timepoint(),
            IncomingRequest::CancelOutgoingRequest(request) => request.timepoint(),
            IncomingRequest::MarkAsDone(request) => request.timepoint(),
            IncomingRequest::PrepareForMigration(request) => request.timepoint(),
            IncomingRequest::Migrate(request) => request.timepoint(),
            IncomingRequest::ChangePeersCompat(request) => request.timepoint(),
        }
    }

    pub fn author(&self) -> &T::AccountId {
        match self {
            IncomingRequest::Transfer(request) => request.author(),
            IncomingRequest::AddToken(request) => request.author(),
            IncomingRequest::ChangePeers(request) => request.author(),
            IncomingRequest::CancelOutgoingRequest(request) => request.author(),
            IncomingRequest::MarkAsDone(request) => request.author(),
            IncomingRequest::PrepareForMigration(request) => request.author(),
            IncomingRequest::Migrate(request) => request.author(),
            IncomingRequest::ChangePeersCompat(request) => request.author(),
        }
    }

    /// Check that the incoming requests still exists on Sidechain.
    pub fn check_existence(&self) -> Result<bool, Error<T>> {
        let network_id = self.network_id();
        match self {
            IncomingRequest::CancelOutgoingRequest(request) => {
                let hash = request.tx_hash;
                let tx = Pallet::<T>::load_tx_receipt(hash, network_id)?;
                Ok(tx.is_approved() == false) // TODO: check for gas limit
            }
            IncomingRequest::MarkAsDone(request) => {
                Pallet::<T>::load_is_used(request.outgoing_request_hash, request.network_id)
            }
            _ => {
                let hash = self.hash();
                let tx = Pallet::<T>::load_tx_receipt(hash, network_id)?;
                Ok(tx.is_approved())
            }
        }
    }
}

#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[scale_info(skip_type_params(T))]
pub enum LoadIncomingRequest<T: Config> {
    Transaction(LoadIncomingTransactionRequest<T>),
    Meta(LoadIncomingMetaRequest<T>, H256),
}

impl<T: Config> LoadIncomingRequest<T> {
    pub fn hash(&self) -> H256 {
        match self {
            Self::Transaction(request) => request.hash,
            Self::Meta(_, hash) => *hash,
        }
    }

    pub fn set_hash(&mut self, new_hash: H256) {
        match self {
            Self::Transaction(_request) => {
                warn!("Attempt to set hash for a 'load transaction' request.");
            } // should not be the case
            Self::Meta(_, hash) => *hash = new_hash,
        }
    }

    pub fn network_id(&self) -> T::NetworkId {
        match self {
            Self::Transaction(request) => request.network_id,
            Self::Meta(request, _) => request.network_id,
        }
    }

    pub fn timepoint(&self) -> Timepoint<T> {
        match self {
            Self::Transaction(request) => request.timepoint,
            Self::Meta(request, _) => request.timepoint,
        }
    }

    pub fn author(&self) -> &T::AccountId {
        match self {
            Self::Transaction(request) => &request.author,
            Self::Meta(request, _) => &request.author,
        }
    }

    /// Checks that the request can be initiated.
    pub fn validate(&self) -> Result<(), DispatchError> {
        match self {
            Self::Transaction(_request) => Ok(()),
            Self::Meta(request, _) => {
                match request.kind {
                    IncomingMetaRequestKind::MarkAsDone => {
                        let request_status =
                            RequestStatuses::<T>::get(request.network_id, request.hash)
                                .ok_or(Error::<T>::UnknownRequest)?;
                        ensure!(
                            request_status == RequestStatus::ApprovalsReady,
                            Error::<T>::RequestIsNotReady
                        );
                    }
                    _ => (),
                }
                Ok(())
            }
        }
    }

    pub fn prepare(&self) -> Result<(), DispatchError> {
        Ok(())
    }

    pub fn cancel(&self) -> Result<(), DispatchError> {
        Ok(())
    }

    pub fn finalize(&self) -> DispatchResult {
        Ok(())
    }
}

/// Information needed for a request to be loaded from sidechain. Basically it's
/// a hash of the transaction and the type of the request.
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[scale_info(skip_type_params(T))]
pub struct LoadIncomingTransactionRequest<T: Config> {
    pub(crate) author: T::AccountId,
    pub(crate) hash: H256,
    pub(crate) timepoint: BridgeTimepoint<T>,
    pub(crate) kind: IncomingTransactionRequestKind,
    pub(crate) network_id: BridgeNetworkId<T>,
}

impl<T: Config> LoadIncomingTransactionRequest<T> {
    pub fn new(
        author: T::AccountId,
        hash: H256,
        timepoint: Timepoint<T>,
        kind: IncomingTransactionRequestKind,
        network_id: T::NetworkId,
    ) -> Self {
        LoadIncomingTransactionRequest {
            author,
            hash,
            timepoint,
            kind,
            network_id,
        }
    }
}

/// Information needed for a request to be loaded from sidechain. Basically it's
/// a hash of the transaction and the type of the request.
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[scale_info(skip_type_params(T))]
pub struct LoadIncomingMetaRequest<T: Config> {
    pub(crate) author: T::AccountId,
    pub(crate) hash: H256,
    pub(crate) timepoint: BridgeTimepoint<T>,
    pub(crate) kind: IncomingMetaRequestKind,
    pub(crate) network_id: BridgeNetworkId<T>,
}

impl<T: Config> LoadIncomingMetaRequest<T> {
    pub fn new(
        author: T::AccountId,
        hash: H256,
        timepoint: Timepoint<T>,
        kind: IncomingMetaRequestKind,
        network_id: T::NetworkId,
    ) -> Self {
        LoadIncomingMetaRequest {
            author,
            hash,
            timepoint,
            kind,
            network_id,
        }
    }
}

/// A bridge operation handled by off-chain workers.
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[scale_info(skip_type_params(T))]
pub enum OffchainRequest<T: Config> {
    /// Thischain->Sidechain request with its hash.
    Outgoing(OutgoingRequest<T>, H256),
    /// Information required to load a corresponding incoming request from the sidechain network.
    LoadIncoming(LoadIncomingRequest<T>),
    /// Sidechain->Thischain request with its hash.
    Incoming(IncomingRequest<T>, H256),
}

impl<T: Config> OffchainRequest<T> {
    pub fn outgoing(request: OutgoingRequest<T>) -> Self {
        let mut request = Self::Outgoing(request, H256::zero());
        let hash = request.using_encoded(blake2_256);
        request.set_hash(H256(hash));
        request
    }

    pub fn load_incoming_meta(request: LoadIncomingMetaRequest<T>) -> Self {
        let mut request = Self::LoadIncoming(LoadIncomingRequest::Meta(request, H256::zero()));
        let hash = request.using_encoded(blake2_256);
        request.set_hash(H256(hash));
        request
    }

    pub fn incoming(request: IncomingRequest<T>) -> Self {
        let mut request = Self::Incoming(request, H256::zero());
        let hash = request.using_encoded(blake2_256);
        request.set_hash(H256(hash));
        request
    }

    /// Calculates or returns an already calculated request hash.
    pub(crate) fn hash(&self) -> H256 {
        match self {
            OffchainRequest::Outgoing(_request, hash) => *hash,
            OffchainRequest::LoadIncoming(request) => request.hash(),
            OffchainRequest::Incoming(_request, hash) => *hash,
        }
    }

    /// Calculates or returns an already calculated request hash.
    fn set_hash(&mut self, new_hash: H256) {
        match self {
            OffchainRequest::Outgoing(_request, hash) => *hash = new_hash,
            OffchainRequest::LoadIncoming(request) => request.set_hash(new_hash),
            OffchainRequest::Incoming(_request, hash) => *hash = new_hash,
        }
    }

    /// The request's network.
    pub(crate) fn network_id(&self) -> T::NetworkId {
        match self {
            OffchainRequest::Outgoing(request, _) => request.network_id(),
            OffchainRequest::LoadIncoming(request) => request.network_id(),
            OffchainRequest::Incoming(request, _) => request.network_id(),
        }
    }

    /// The request's timepoint.
    pub(crate) fn timepoint(&self) -> Timepoint<T> {
        match self {
            OffchainRequest::Outgoing(request, _) => request.timepoint(),
            OffchainRequest::LoadIncoming(request) => request.timepoint(),
            OffchainRequest::Incoming(request, _) => request.timepoint(),
        }
    }

    /// An initiator of the request.
    pub(crate) fn author(&self) -> &T::AccountId {
        match self {
            OffchainRequest::Outgoing(request, _) => request.author(),
            OffchainRequest::LoadIncoming(request) => request.author(),
            OffchainRequest::Incoming(request, _) => request.author(),
        }
    }

    /// Checks that the request can be initiated (e.g., verifies that an account has
    /// sufficient funds for transfer).
    pub(crate) fn validate(&self) -> Result<(), DispatchError> {
        match self {
            OffchainRequest::Outgoing(request, _) => request.validate(),
            OffchainRequest::LoadIncoming(request) => request.validate(),
            OffchainRequest::Incoming(request, _) => request.validate(),
        }
    }

    /// Performs additional state changes for the request (e.g., reserves funds for a transfer).
    pub(crate) fn prepare(&self) -> Result<(), DispatchError> {
        match self {
            OffchainRequest::Outgoing(request, tx_hash) => request.prepare(*tx_hash),
            OffchainRequest::LoadIncoming(request) => request.prepare(),
            OffchainRequest::Incoming(request, _) => request.prepare(),
        }
    }

    /// Undos the state changes done in the `prepare` function.
    pub fn cancel(&self) -> Result<(), DispatchError> {
        match self {
            OffchainRequest::Outgoing(request, _) => request.cancel(),
            OffchainRequest::LoadIncoming(request) => request.cancel(),
            OffchainRequest::Incoming(request, _) => request.cancel(),
        }
    }

    pub fn finalize(&self) -> DispatchResult {
        match self {
            OffchainRequest::Outgoing(r, tx_hash) => r.finalize(*tx_hash),
            OffchainRequest::Incoming(r, _) => r.finalize().map(|_| ()),
            OffchainRequest::LoadIncoming(r) => r.finalize(),
        }
    }

    pub fn as_outgoing(&self) -> Option<(&OutgoingRequest<T>, H256)> {
        match self {
            OffchainRequest::Outgoing(r, h) => Some((r, *h)),
            _ => None,
        }
    }

    pub fn into_outgoing(self) -> Option<(OutgoingRequest<T>, H256)> {
        match self {
            OffchainRequest::Outgoing(r, h) => Some((r, h)),
            _ => None,
        }
    }

    pub fn into_incoming(self) -> Option<(IncomingRequest<T>, H256)> {
        match self {
            OffchainRequest::Incoming(r, h) => Some((r, h)),
            _ => None,
        }
    }

    pub fn as_incoming(&self) -> Option<(&IncomingRequest<T>, H256)> {
        match self {
            OffchainRequest::Incoming(r, h) => Some((r, *h)),
            _ => None,
        }
    }

    pub fn is_load_incoming(&self) -> bool {
        match self {
            OffchainRequest::LoadIncoming(..) => true,
            _ => false,
        }
    }

    pub fn is_incoming(&self) -> bool {
        match self {
            OffchainRequest::Incoming(..) => true,
            _ => false,
        }
    }

    pub fn should_be_skipped(&self) -> bool {
        match self {
            OffchainRequest::Outgoing(req, _) => req.should_be_skipped(),
            _ => false,
        }
    }
}

/// Ethereum-encoded `OutgoingRequest`. Contains a payload for signing by peers. Also, can be used
/// by client apps for more convenient contract function calls.
#[derive(Clone, Encode, Decode, RuntimeDebug, PartialEq, Eq, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum OutgoingRequestEncoded {
    /// ETH-encoded incoming transfer from Substrate to Ethereum request.
    Transfer(OutgoingTransferEncoded),
    /// ETH-encoded 'add new asset' request.
    AddAsset(OutgoingAddAssetEncoded),
    /// ETH-encoded 'add new token' request.
    AddToken(OutgoingAddTokenEncoded),
    /// ETH-encoded 'add peer' request.
    AddPeer(OutgoingAddPeerEncoded),
    /// ETH-encoded 'remove peer' request.
    RemovePeer(OutgoingRemovePeerEncoded),
    /// ETH-encoded 'prepare for migration' request.
    PrepareForMigration(OutgoingPrepareForMigrationEncoded),
    /// ETH-encoded 'migrate' request.
    Migrate(OutgoingMigrateEncoded),
}

impl OutgoingRequestEncoded {
    #[allow(unused)]
    fn hash(&self) -> H256 {
        let hash = match self {
            OutgoingRequestEncoded::Transfer(transfer) => transfer.tx_hash,
            OutgoingRequestEncoded::AddAsset(request) => request.hash,
            OutgoingRequestEncoded::AddToken(request) => request.hash,
            OutgoingRequestEncoded::AddPeer(request) => request.tx_hash,
            OutgoingRequestEncoded::RemovePeer(request) => request.tx_hash,
            OutgoingRequestEncoded::PrepareForMigration(request) => request.tx_hash,
            OutgoingRequestEncoded::Migrate(request) => request.tx_hash,
        };
        H256(hash.0)
    }

    pub(crate) fn as_raw(&self) -> &[u8] {
        match self {
            OutgoingRequestEncoded::Transfer(transfer) => &transfer.raw,
            OutgoingRequestEncoded::AddAsset(request) => &request.raw,
            OutgoingRequestEncoded::AddToken(request) => &request.raw,
            OutgoingRequestEncoded::AddPeer(request) => &request.raw,
            OutgoingRequestEncoded::RemovePeer(request) => &request.raw,
            OutgoingRequestEncoded::PrepareForMigration(request) => &request.raw,
            OutgoingRequestEncoded::Migrate(request) => &request.raw,
        }
    }

    /// Returns Ethereum tokens needed for the corresponding contract function.
    pub fn input_tokens(&self, signatures: Option<Vec<SignatureParams>>) -> Vec<Token> {
        match self {
            OutgoingRequestEncoded::Transfer(request) => request.input_tokens(signatures),
            OutgoingRequestEncoded::AddAsset(request) => request.input_tokens(signatures),
            OutgoingRequestEncoded::AddToken(request) => request.input_tokens(signatures),
            OutgoingRequestEncoded::AddPeer(request) => request.input_tokens(signatures),
            OutgoingRequestEncoded::RemovePeer(request) => request.input_tokens(signatures),
            OutgoingRequestEncoded::PrepareForMigration(request) => {
                request.input_tokens(signatures)
            }
            OutgoingRequestEncoded::Migrate(request) => request.input_tokens(signatures),
        }
    }
}

/// Status of a registered request.
///
/// - Pending: request hasn't been approved yet.
/// - Frozen: request stopped receiving confirmations (signatures) from peers.
///   E.g. when the request is in 'cancellation' stage.
/// - ApprovalsReady: request was approved and can be used in the sidechain.
/// - Failed: an error occurred in one of the previous stages.
/// - Done: request was finalized.
#[derive(PartialEq, Eq, Encode, Decode, RuntimeDebug, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum RequestStatus {
    Pending,
    Frozen,
    ApprovalsReady,
    Failed(DispatchError),
    Done,
    /// Request is broken. Tried to abort with the first error but got another one when cancelling.
    Broken(DispatchError, DispatchError),
}

/// A type of asset registered on a bridge.
///
/// - Thischain: a Sora asset.
/// - Sidechain: an Ethereum token.
/// - SidechainOwned: an Ethereum token that can be minted on Sora.
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Clone, Copy, Encode, Decode, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo)]
pub enum AssetKind {
    Thischain,
    Sidechain,
    SidechainOwned,
}

impl AssetKind {
    pub fn is_owned(&self) -> bool {
        self == &Self::Thischain || self == &Self::SidechainOwned
    }
}
