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

use crate::jsonrpc::Params;
#[cfg(test)]
use crate::tests::mock::Mock;
use crate::types::{
    BlockNumber, Bytes, CallRequest, FilterBuilder, Log, SubstrateBlockLimited,
    SubstrateHeaderLimited, Transaction, TransactionReceipt,
};
use crate::util::serialize;
use crate::{
    types, BridgeContractAddress, Config, Error, NodeParams, Pallet, DEPOSIT_TOPIC,
    HTTP_REQUEST_TIMEOUT_SECS, STORAGE_ETH_NODE_PARAMS, STORAGE_SUB_NODE_URL_KEY, SUB_NODE_URL,
};
use alloc::string::String;
use alloc::vec::Vec;
use frame_support::fail;
use frame_support::sp_runtime::offchain as rt_offchain;
use frame_support::sp_runtime::offchain::storage::StorageValueRef;
use frame_support::traits::Get;
use frame_system::offchain::CreateSignedTransaction;
use frame_system::pallet_prelude::BlockNumberFor;
use hex_literal::hex;
use log::{error, trace, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sp_core::{H160, H256};
use sp_std::convert::TryInto;

impl<T: Config> Pallet<T> {
    /// Makes off-chain HTTP request.
    pub fn http_request(
        url: &str,
        body: Vec<u8>,
        headers: &[(&'static str, String)],
    ) -> Result<Vec<u8>, Error<T>> {
        trace!("Sending request to: {}", url);
        let mut request = rt_offchain::http::Request::post(url, vec![body.clone()]);
        let timeout = sp_io::offchain::timestamp().add(rt_offchain::Duration::from_millis(
            HTTP_REQUEST_TIMEOUT_SECS * 1000,
        ));
        for (key, value) in headers {
            request = request.add_header(*key, &*value);
        }
        #[allow(unused_mut)]
        let mut pending = request.deadline(timeout).send().map_err(|e| {
            error!("Failed to send a request {:?}", e);
            <Error<T>>::HttpFetchingError
        })?;
        #[cfg(test)]
        T::Mock::on_request(&mut pending, url, String::from_utf8_lossy(&body));
        let response = pending
            .try_wait(timeout)
            .map_err(|e| {
                error!("Failed to get a response: {:?}", e);
                <Error<T>>::HttpFetchingError
            })?
            .map_err(|e| {
                error!("Failed to get a response: {:?}", e);
                <Error<T>>::HttpFetchingError
            })?;
        if response.code != 200 {
            error!("Unexpected http request status code: {}", response.code);
            return Err(<Error<T>>::HttpFetchingError);
        }
        let resp = response.body().collect::<Vec<u8>>();
        Ok(resp)
    }

    /// Makes JSON-RPC request.
    pub fn json_rpc_request<I: Serialize, O: for<'de> Deserialize<'de>>(
        url: &str,
        id: u64,
        method: &str,
        params: &I,
        headers: &[(&'static str, String)],
    ) -> Result<O, Error<T>> {
        let params = match serialize(params) {
            Value::Null => Params::None,
            Value::Array(v) => Params::Array(v),
            Value::Object(v) => Params::Map(v),
            _ => {
                error!("json_rpc_request: got invalid params");
                fail!(Error::<T>::JsonSerializationError);
            }
        };

        let raw_response = Self::http_request(
            url,
            serde_json::to_vec(&jsonrpc::Request::Single(jsonrpc::Call::MethodCall(
                jsonrpc::MethodCall {
                    jsonrpc: Some(jsonrpc::Version::V2),
                    method: method.into(),
                    params,
                    id: jsonrpc::Id::Num(id as u64),
                },
            )))
            .map_err(|_| Error::<T>::JsonSerializationError)?,
            &headers,
        )
        .and_then(|x| {
            String::from_utf8(x).map_err(|e| {
                error!("json_rpc_request: from utf8 failed, {}", e);
                Error::<T>::HttpFetchingError
            })
        })?;
        let response = jsonrpc::Response::from_json(&raw_response)
            .map_err(|e| {
                error!("json_rpc_request: from_json failed, {}", e);
            })
            .map_err(|_| Error::<T>::FailedToLoadTransaction)?;
        let result = match response {
            jsonrpc::Response::Batch(_xs) => {
                unreachable!("we've just sent a `Single` request; qed")
            }
            jsonrpc::Response::Single(x) => x,
        };
        match result {
            jsonrpc::Output::Success(s) => {
                if s.result.is_null() {
                    Err(Error::<T>::FailedToLoadTransaction)
                } else {
                    serde_json::from_value(s.result).map_err(|e| {
                        error!("json_rpc_request: from_value failed, {}", e);
                        Error::<T>::JsonDeserializationError.into()
                    })
                }
            }
            _ => {
                error!("json_rpc_request: request failed");
                Err(Error::<T>::JsonDeserializationError.into())
            }
        }
    }

    /// Makes request to a Sidechain node. The node URL and credentials are stored in the local
    /// storage.
    pub fn eth_json_rpc_request<I: Serialize, O: for<'de> Deserialize<'de>>(
        method: &str,
        params: &I,
        network_id: T::NetworkId,
    ) -> Result<O, Error<T>> {
        let string = format!("{}-{:?}", STORAGE_ETH_NODE_PARAMS, network_id);
        let s_node_params = StorageValueRef::persistent(string.as_bytes());
        let node_params = match s_node_params.get::<NodeParams>().ok().flatten() {
            Some(v) => v,
            None => {
                warn!("Failed to make JSON-RPC request, make sure to set node parameters.");
                fail!(Error::<T>::FailedToLoadSidechainNodeParams);
            }
        };
        let mut headers: Vec<(_, String)> = vec![("content-type", "application/json".into())];
        if let Some(node_credentials) = node_params.credentials {
            headers.push(("Authorization", node_credentials));
        }
        Self::json_rpc_request(&node_params.url, 0, method, params, &headers)
    }

    /// Makes request to the local node. The node URL is stored in the local storage.
    pub fn substrate_json_rpc_request<I: Serialize, O: for<'de> Deserialize<'de>>(
        method: &str,
        params: &I,
    ) -> Result<O, Error<T>> {
        let s_node_url = StorageValueRef::persistent(STORAGE_SUB_NODE_URL_KEY);
        let node_url = s_node_url
            .get::<String>()
            .ok()
            .flatten()
            .unwrap_or_else(|| SUB_NODE_URL.into());
        let headers: Vec<(_, String)> = vec![("content-type", "application/json".into())];

        Self::json_rpc_request(&node_url, 0, method, params, &headers)
    }

    /// Queries Sidechain's contract variable `used`.
    pub fn load_is_used(hash: H256, network_id: T::NetworkId) -> Result<bool, Error<T>> {
        // `used(bytes32)`
        let mut data: Vec<_> = hex!("b07c411f").to_vec();
        data.extend(&hash.0);
        let contract_address = types::H160(BridgeContractAddress::<T>::get(network_id).0);
        let contracts = if network_id == T::GetEthNetworkId::get() {
            vec![
                contract_address,
                types::H160(Self::xor_master_contract_address().0),
                types::H160(Self::val_master_contract_address().0),
            ]
        } else {
            vec![contract_address]
        };
        for contract in contracts {
            let is_used = Self::eth_json_rpc_request::<_, bool>(
                "eth_call",
                &vec![
                    serialize(&CallRequest {
                        to: Some(contract),
                        data: Some(Bytes(data.clone())),
                        ..Default::default()
                    }),
                    Value::String("latest".into()),
                ],
                network_id,
            )?;
            if is_used {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Queries current height of Sidechain.
    pub fn load_current_height(network_id: T::NetworkId) -> Result<u64, Error<T>> {
        Self::eth_json_rpc_request::<_, types::U64>("eth_blockNumber", &(), network_id)
            .map(|x| x.as_u64())
    }

    /// Loads a Sidechain transaction by the hash and ensures that it came from a known contract.
    pub fn load_tx(hash: H256, network_id: T::NetworkId) -> Result<Transaction, Error<T>> {
        let hash = types::H256(hash.0);
        let tx_receipt = Self::eth_json_rpc_request::<_, Transaction>(
            "eth_getTransactionByHash",
            &vec![hash],
            network_id,
        )?;
        let to = tx_receipt
            .to
            .map(|x| H160(x.0))
            .ok_or(Error::<T>::UnknownContractAddress)?;
        Self::ensure_known_contract(to, network_id)?;
        Ok(tx_receipt)
    }

    /// Loads a Sidechain transaction receipt by the hash and ensures that it came from a known contract.
    // TODO: check if transaction failed due to gas limit
    pub fn load_tx_receipt(
        hash: H256,
        network_id: T::NetworkId,
    ) -> Result<TransactionReceipt, Error<T>> {
        let hash = types::H256(hash.0);
        let tx_receipt = Self::eth_json_rpc_request::<_, TransactionReceipt>(
            "eth_getTransactionReceipt",
            &vec![hash],
            network_id,
        )?;
        let to = tx_receipt
            .to
            .map(|x| H160(x.0))
            .ok_or(Error::<T>::UnknownContractAddress)?;
        Self::ensure_known_contract(to, network_id)?;
        Ok(tx_receipt)
    }

    /// Queries the current finalized block of the local node with `chain_getFinalizedHead` and
    /// `chain_getHeader` RPC calls.
    pub fn load_substrate_finalized_header() -> Result<SubstrateHeaderLimited, Error<T>>
    where
        T: CreateSignedTransaction<<T as Config>::RuntimeCall>,
    {
        let hash =
            Self::substrate_json_rpc_request::<_, types::H256>("chain_getFinalizedHead", &())?;
        let header = Self::substrate_json_rpc_request::<_, types::SubstrateHeaderLimited>(
            "chain_getHeader",
            &[hash],
        )?;
        Ok(header)
    }

    /// Queries a block at the given height of the local node with `chain_getBlockHash` and
    /// `chain_getBlock` RPC calls.
    pub fn load_substrate_block(
        number: BlockNumberFor<T>,
    ) -> Result<SubstrateBlockLimited, Error<T>>
    where
        T: CreateSignedTransaction<<T as Config>::RuntimeCall>,
    {
        let int: u32 = number
            .try_into()
            .map_err(|_| ())
            .expect("block number is always at least u32; qed");
        let hash =
            Self::substrate_json_rpc_request::<_, types::H256>("chain_getBlockHash", &[int])?;
        let block = Self::substrate_json_rpc_request::<_, types::SubstrateSignedBlockLimited>(
            "chain_getBlock",
            &[hash],
        )?;
        Ok(block.block)
    }

    /// Queries the sidechain node for the transfer logs emitted within `from_block` and `to_block`.
    ///
    /// Uses the `eth_getLogs` method with a filter on log topic.
    pub fn load_transfers_logs(
        network_id: T::NetworkId,
        from_block: u64,
        to_block: u64,
    ) -> Result<Vec<Log>, Error<T>> {
        trace!(
            "Loading transfer logs from block {:?} to block {:?}",
            from_block,
            to_block,
        );
        Self::eth_json_rpc_request(
            "eth_getLogs",
            &[FilterBuilder::default()
                .topics(Some(vec![types::H256(DEPOSIT_TOPIC.0)]), None, None, None)
                .from_block(BlockNumber::Number(from_block.into()))
                .to_block(BlockNumber::Number(to_block.into()))
                .address(vec![types::H160(
                    BridgeContractAddress::<T>::get(network_id).0,
                )])
                .build()],
            network_id,
        )
    }
}
