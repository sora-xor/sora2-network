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

use crate::contract::{
    functions, init_add_peer_by_peer_fn, init_remove_peer_by_peer_fn, ContractEvent, DepositEvent,
    ADD_PEER_BY_PEER_FN, ADD_PEER_BY_PEER_ID, ADD_PEER_BY_PEER_TX_HASH_ARG_POS, FUNCTIONS,
    METHOD_ID_SIZE, REMOVE_PEER_BY_PEER_FN, REMOVE_PEER_BY_PEER_ID,
    REMOVE_PEER_BY_PEER_TX_HASH_ARG_POS,
};
use crate::requests::{
    parse_hash_from_call, AssetKind, ChangePeersContract, IncomingCancelOutgoingRequest,
    IncomingChangePeersCompat, IncomingRequest, IncomingTransactionRequestKind,
    LoadIncomingMetaRequest, LoadIncomingTransactionRequest, OffchainRequest,
    OutgoingAddPeerCompat, OutgoingRemovePeerCompat, OutgoingRequest,
};
use crate::types::{Log, Transaction, TransactionReceipt};
use crate::util::Decoder;
use crate::{
    BridgeContractAddress, Config, Error, EthAddress, Pallet, Requests, DEPOSIT_TOPIC,
    STORAGE_NETWORK_IDS_KEY,
};
use alloc::string::String;
use codec::{Decode, Encode};
use common::{eth, Balance};
use ethabi::ParamType;
use frame_support::sp_runtime::app_crypto::{ecdsa, sp_core};
use frame_support::sp_runtime::offchain::storage::StorageValueRef;
use frame_support::sp_runtime::traits::IdentifyAccount;
use frame_support::sp_runtime::MultiSigner;
use frame_support::traits::Get;
use frame_support::{ensure, fail, RuntimeDebug};
use frame_system::offchain::CreateSignedTransaction;
pub use handle::*;
use hex_literal::hex;
pub use http::*;
use rustc_hex::ToHex;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_core::crypto::ByteArray;
use sp_core::{H160, H256};
use sp_std::collections::btree_set::BTreeSet;
use sp_std::convert::TryInto;
use sp_std::fmt;
use sp_std::fmt::Formatter;
pub use transaction::*;

mod handle;
mod http;
mod transaction;

/// Cryptography used by off-chain workers.
pub mod crypto {
    use crate::KEY_TYPE;

    use frame_support::sp_runtime::app_crypto::{app_crypto, ecdsa};
    use frame_support::sp_runtime::{MultiSignature, MultiSigner};

    app_crypto!(ecdsa, KEY_TYPE);

    pub struct TestAuthId;

    // implemented for ocw-runtime
    impl frame_system::offchain::AppCrypto<MultiSigner, MultiSignature> for TestAuthId {
        type RuntimeAppPublic = Public;
        type GenericPublic = ecdsa::Public;
        type GenericSignature = ecdsa::Signature;
    }
}

impl<T: Config> Pallet<T> {
    fn parse_deposit_event(
        log: &Log,
    ) -> Result<DepositEvent<EthAddress, T::AccountId, Balance>, Error<T>> {
        if log.removed.unwrap_or(true) {
            return Err(Error::<T>::EthLogWasRemoved);
        }
        let types = [
            ParamType::FixedBytes(32),
            ParamType::Uint(256),
            ParamType::Address,
            ParamType::FixedBytes(32),
        ];
        let decoded =
            ethabi::decode(&types, &log.data.0).map_err(|_| Error::<T>::EthAbiDecodingError)?;
        let mut decoder = Decoder::<T>::new(decoded);
        let sidechain_asset = decoder.next_h256()?;
        let token = decoder.next_address()?;
        let amount = decoder.next_amount()?;
        let destination = decoder.next_account_id()?;
        Ok(DepositEvent {
            destination,
            amount,
            token,
            sidechain_asset,
        })
    }

    /// Loops through the given array of logs and finds the first one that matches the type
    /// and topic.
    pub fn parse_main_event(
        network_id: T::NetworkId,
        logs: &[Log],
        kind: IncomingTransactionRequestKind,
    ) -> Result<ContractEvent<EthAddress, T::AccountId, Balance>, Error<T>> {
        for log in logs {
            // Check address to be sure what it came from our contract
            if Self::ensure_known_contract(log.address.0.into(), network_id).is_err() {
                continue;
            }
            if log.removed.unwrap_or(true) {
                continue;
            }
            let topic = match log.topics.get(0) {
                Some(x) => &x.0,
                None => continue,
            };
            match *topic {
                topic
                    if topic == DEPOSIT_TOPIC.0
                        && (kind == IncomingTransactionRequestKind::Transfer
                            || kind == IncomingTransactionRequestKind::TransferXOR) =>
                {
                    return Ok(ContractEvent::Deposit(Self::parse_deposit_event(log)?));
                }
                // ChangePeers(address,bool)
                topic
                    if topic
                        == hex!(
                            "a9fac23eb012e72fbd1f453498e7069c380385436763ee2c1c057b170d88d9f9"
                        )
                        && (kind == IncomingTransactionRequestKind::AddPeer
                            || kind == IncomingTransactionRequestKind::RemovePeer) =>
                {
                    let types = [ParamType::Address, ParamType::Bool];
                    let decoded = ethabi::decode(&types, &log.data.0)
                        .map_err(|_| Error::<T>::EthAbiDecodingError)?;
                    let mut decoder = Decoder::<T>::new(decoded);
                    let removed = decoder.next_bool()?;
                    let peer_address = decoder.next_address()?;
                    return Ok(ContractEvent::ChangePeers(H160(peer_address.0), removed));
                }
                topic
                    if topic
                        == hex!(
                            "5389de9593f75e6515eefa796bd2d3324759f441f2c9b2dcda0efb25190378ff"
                        )
                        && kind == IncomingTransactionRequestKind::PrepareForMigration =>
                {
                    return Ok(ContractEvent::PreparedForMigration);
                }
                topic
                    if topic
                        == hex!(
                            "a2e7361c23d7820040603b83c0cd3f494d377bac69736377d75bb56c651a5098"
                        )
                        && kind == IncomingTransactionRequestKind::Migrate =>
                {
                    let types = [ParamType::Address];
                    let decoded = ethabi::decode(&types, &log.data.0)
                        .map_err(|_| Error::<T>::EthAbiDecodingError)?;
                    let mut decoder = Decoder::<T>::new(decoded);
                    let account_id = decoder.next_address()?;
                    return Ok(ContractEvent::Migrated(account_id));
                }
                _ => (),
            }
        }
        Err(Error::<T>::UnknownEvent.into())
    }

    /// Verifies the message signed by a peer. Also, compares the given `AccountId` with the given
    /// public key.
    pub(crate) fn verify_message(
        msg: &[u8],
        signature: &SignatureParams,
        ecdsa_public_key: &ecdsa::Public,
        author: &T::AccountId,
    ) -> bool {
        let message = eth::prepare_message(msg);
        let sig_bytes = signature.to_bytes();
        let res = secp256k1::Signature::parse_standard_slice(&sig_bytes[..64]).and_then(|sig| {
            secp256k1::PublicKey::parse_slice(ecdsa_public_key.as_slice(), None).map(|pk| (sig, pk))
        });
        if let Ok((signature, public_key)) = res {
            let signer_account = MultiSigner::Ecdsa(ecdsa_public_key.clone()).into_account();
            let verified = secp256k1::verify(&message, &signature, &public_key);
            signer_account.encode() == author.encode() && verified
        } else {
            false
        }
    }

    /// Signs a message with a peer's secret key.
    pub(crate) fn sign_message(
        msg: &[u8],
        secret_key: &secp256k1::SecretKey,
    ) -> (SignatureParams, secp256k1::PublicKey) {
        let message = eth::prepare_message(msg);
        let (sig, v) = secp256k1::sign(&message, secret_key);
        let pk = secp256k1::PublicKey::from_secret_key(secret_key);
        let v = v.serialize();
        let sig_ser = sig.serialize();
        (
            SignatureParams {
                r: sig_ser[..32].try_into().unwrap(),
                s: sig_ser[32..].try_into().unwrap(),
                v,
            },
            pk,
        )
    }

    /// Parses a 'cancel' incoming request from the given transaction receipt and pre-request.
    ///
    /// This special flow (unlike `parse_incoming_request`) requires transaction to be _failed_,
    /// because only in this case the initial request can be cancelled.
    fn parse_cancel_incoming_request(
        tx_receipt: TransactionReceipt,
        pre_request: LoadIncomingMetaRequest<T>,
        pre_request_hash: H256,
    ) -> Result<IncomingRequest<T>, Error<T>> {
        let tx_approved = tx_receipt.is_approved();
        ensure!(!tx_approved, Error::<T>::EthTransactionIsSucceeded);
        let at_height = tx_receipt
            .block_number
            .expect("'block_number' is null only when the log/transaction is pending; qed")
            .as_u64();
        let tx = Self::load_tx(H256(tx_receipt.transaction_hash.0), pre_request.network_id)?;
        ensure!(
            tx_receipt
                .gas_used
                .map(|used| used != tx.gas)
                .unwrap_or(false),
            Error::<T>::TransactionMightHaveFailedDueToGasLimit
        );
        ensure!(
            tx.input.0.len() >= METHOD_ID_SIZE,
            Error::<T>::InvalidFunctionInput
        );
        let mut method_id = [0u8; METHOD_ID_SIZE];
        method_id.clone_from_slice(&tx.input.0[..METHOD_ID_SIZE]);
        let funs = FUNCTIONS.get_or_init(functions);
        let fun_meta = funs.get(&method_id).ok_or(Error::<T>::UnknownMethodId)?;
        let fun = &fun_meta.function;
        let tokens = fun
            .decode_input(&tx.input.0)
            .map_err(|_| Error::<T>::InvalidFunctionInput)?;
        let hash = parse_hash_from_call::<T>(tokens, fun_meta.tx_hash_arg_pos)?;
        let oc_request: OffchainRequest<T> =
            crate::Requests::<T>::get(pre_request.network_id, hash)
                .ok_or(Error::<T>::UnknownRequest)?;
        let request = oc_request
            .into_outgoing()
            .ok_or(Error::<T>::ExpectedOutgoingRequest)?
            .0;
        let author = pre_request.author;
        ensure!(
            request.author() == &author,
            Error::<T>::RequestIsNotOwnedByTheAuthor
        );
        Ok(IncomingRequest::CancelOutgoingRequest(
            IncomingCancelOutgoingRequest {
                outgoing_request: request,
                outgoing_request_hash: hash,
                initial_request_hash: pre_request_hash,
                author,
                tx_input: tx.input.0,
                tx_hash: pre_request.hash,
                at_height,
                timepoint: pre_request.timepoint,
                network_id: pre_request.network_id,
            },
        ))
    }

    /// Gets Thischain asset id and its kind. If the `raw_asset_id` is `zero`, it means that it's
    /// a Sidechain(Owned) asset, otherwise, Thischain.
    pub(crate) fn get_asset_by_raw_asset_id(
        raw_asset_id: H256,
        token_address: &EthAddress,
        network_id: T::NetworkId,
    ) -> Result<Option<(T::AssetId, AssetKind)>, Error<T>> {
        let is_sidechain_token = raw_asset_id == H256::zero();
        if is_sidechain_token {
            let asset_id = match Self::registered_sidechain_asset(network_id, &token_address) {
                Some(asset_id) => asset_id,
                _ => {
                    return Ok(None);
                }
            };
            Ok(Some((
                asset_id,
                Self::registered_asset(network_id, &asset_id).unwrap_or(AssetKind::Sidechain),
            )))
        } else {
            let asset_id = T::AssetId::from(H256(raw_asset_id.0));
            let asset_kind = Self::registered_asset(network_id, &asset_id);
            if asset_kind.is_none() || asset_kind.unwrap() == AssetKind::Sidechain {
                fail!(Error::<T>::UnknownAssetId);
            }
            Ok(Some((asset_id, AssetKind::Thischain)))
        }
    }

    /// Tries to parse a method call on one of old Sora contracts (XOR and VAL).
    ///
    /// Since the XOR and VAL contracts don't have the same interface and events that the modern
    /// bridge contract have, and since they can't be changed we have to provide a special parsing
    /// flow for some of the methods that we might use.
    pub fn parse_old_incoming_request_method_call(
        incoming_request: LoadIncomingTransactionRequest<T>,
        tx: Transaction,
    ) -> Result<IncomingRequest<T>, Error<T>> {
        let (fun, arg_pos, tail, added) = if let Some(tail) = tx
            .input
            .0
            .strip_prefix(&*ADD_PEER_BY_PEER_ID.get_or_init(init_add_peer_by_peer_fn))
        {
            (
                &ADD_PEER_BY_PEER_FN,
                ADD_PEER_BY_PEER_TX_HASH_ARG_POS,
                tail,
                true,
            )
        } else if let Some(tail) = tx
            .input
            .0
            .strip_prefix(&*REMOVE_PEER_BY_PEER_ID.get_or_init(init_remove_peer_by_peer_fn))
        {
            (
                &REMOVE_PEER_BY_PEER_FN,
                REMOVE_PEER_BY_PEER_TX_HASH_ARG_POS,
                tail,
                false,
            )
        } else {
            fail!(Error::<T>::UnknownMethodId);
        };

        let tokens = (*fun.get().unwrap())
            .decode_input(tail)
            .map_err(|_| Error::<T>::EthAbiDecodingError)?;
        let request_hash = parse_hash_from_call::<T>(tokens, arg_pos)?;
        let author = incoming_request.author;
        let oc_request: OffchainRequest<T> =
            Requests::<T>::get(T::GetEthNetworkId::get(), request_hash)
                .ok_or(Error::<T>::UnknownRequest)?;
        match oc_request {
            OffchainRequest::Outgoing(
                OutgoingRequest::AddPeerCompat(OutgoingAddPeerCompat {
                    peer_address,
                    peer_account_id,
                    ..
                }),
                _,
            )
            | OffchainRequest::Outgoing(
                OutgoingRequest::RemovePeerCompat(OutgoingRemovePeerCompat {
                    peer_address,
                    peer_account_id,
                    ..
                }),
                _,
            ) => {
                let contract = match tx.to {
                    Some(x) if x.0 == Self::xor_master_contract_address().0 => {
                        ChangePeersContract::XOR
                    }
                    Some(x) if x.0 == Self::val_master_contract_address().0 => {
                        ChangePeersContract::VAL
                    }
                    _ => fail!(Error::<T>::UnknownContractAddress),
                };
                let at_height = tx
                    .block_number
                    .expect("'block_number' is null only when the log/transaction is pending; qed")
                    .as_u64();
                let request = IncomingRequest::ChangePeersCompat(IncomingChangePeersCompat {
                    peer_account_id,
                    peer_address,
                    added,
                    contract,
                    author,
                    tx_hash: incoming_request.hash,
                    at_height,
                    timepoint: incoming_request.timepoint,
                    network_id: incoming_request.network_id,
                });
                Ok(request)
            }
            _ => fail!(Error::<T>::InvalidFunctionInput),
        }
    }

    /// Tries to parse incoming request from the given pre-request and transaction receipt.
    ///
    /// The transaction should be approved and contain a known event from which the request
    /// is built.
    fn parse_incoming_request(
        tx_receipt: TransactionReceipt,
        incoming_pre_request: LoadIncomingTransactionRequest<T>,
    ) -> Result<IncomingRequest<T>, Error<T>> {
        let tx_approved = tx_receipt.is_approved();
        ensure!(tx_approved, Error::<T>::EthTransactionIsFailed);
        let kind = incoming_pre_request.kind;
        let network_id = incoming_pre_request.network_id;

        // For XOR and VAL contracts compatibility.
        if kind.is_compat() {
            let tx = Self::load_tx(H256(tx_receipt.transaction_hash.0), network_id)?;
            return Self::parse_old_incoming_request_method_call(incoming_pre_request, tx);
        }

        let at_height = tx_receipt
            .block_number
            .expect("'block_number' is null only when the log/transaction is pending; qed")
            .as_u64();

        let call = Self::parse_main_event(network_id, &tx_receipt.logs, kind)?;
        // TODO (optimization): pre-validate the parsed calls.
        IncomingRequest::<T>::try_from_contract_event(call, incoming_pre_request, at_height)
    }

    /// Checks that the given contract address is known to the bridge network.
    ///
    /// There are special cases for XOR and VAL contracts.
    pub fn ensure_known_contract(to: EthAddress, network_id: T::NetworkId) -> Result<(), Error<T>> {
        if network_id == T::GetEthNetworkId::get() {
            ensure!(
                to == BridgeContractAddress::<T>::get(network_id)
                    || to == Self::xor_master_contract_address()
                    || to == Self::val_master_contract_address(),
                Error::<T>::UnknownContractAddress
            );
        } else {
            ensure!(
                to == BridgeContractAddress::<T>::get(network_id),
                Error::<T>::UnknownContractAddress
            );
        }
        Ok(())
    }

    /// Handles registered networks.
    pub(crate) fn offchain()
    where
        T: CreateSignedTransaction<<T as Config>::RuntimeCall>,
    {
        let s_networks_ids = StorageValueRef::persistent(STORAGE_NETWORK_IDS_KEY);

        let substrate_finalized_height = match Self::handle_substrate() {
            Ok(v) => v,
            Err(_e) => {
                return;
            }
        };

        let network_ids = s_networks_ids
            .get::<BTreeSet<T::NetworkId>>()
            .ok()
            .flatten()
            .unwrap_or_default();
        for network_id in network_ids {
            Self::handle_network(network_id, substrate_finalized_height);
        }
    }
}

/// Separated components of a secp256k1 signature.
#[derive(
    Encode, Decode, Eq, PartialEq, Clone, PartialOrd, Ord, RuntimeDebug, scale_info::TypeInfo,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "runtime-benchmarks"), derive(Default))]
#[repr(C)]
pub struct SignatureParams {
    pub r: [u8; 32],
    pub s: [u8; 32],
    pub v: u8,
}

impl SignatureParams {
    fn to_bytes(&self) -> [u8; 65] {
        let mut arr = [0u8; 65];
        arr[..32].copy_from_slice(&self.r[..]);
        arr[32..64].copy_from_slice(&self.s[..]);
        arr[64] = self.v;
        arr
    }
}

impl fmt::Display for SignatureParams {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&format!(
            "SignatureParams {{\n\tr: {},\n\ts: {},\n\tv: {}\n}}",
            self.r.to_hex::<String>(),
            self.s.to_hex::<String>(),
            [self.v].to_hex::<String>()
        ))
    }
}
