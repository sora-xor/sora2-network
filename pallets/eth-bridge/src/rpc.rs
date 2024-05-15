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

use crate::offchain::SignatureParams;
use crate::requests::{AssetKind, OffchainRequest, OutgoingRequestEncoded, RequestStatus};
use crate::util::iter_storage;
use crate::{
    AssetIdOf, Config, LoadToIncomingRequestHash, Pallet, RegisteredAsset,
    RegisteredSidechainToken, RequestApprovals, RequestStatuses, Requests, SidechainAssetPrecision,
};
use common::{AssetInfoProvider, BalancePrecision};
use sp_runtime::DispatchError;
use frame_support::sp_runtime::app_crypto::sp_core;
use sp_core::{H160, H256};
use sp_std::prelude::*;

impl<T: Config> Pallet<T> {
    const ITEMS_LIMIT: usize = 50;

    /// Get requests data and their statuses by hash.
    pub fn get_requests(
        hashes: &[H256],
        network_id: Option<T::NetworkId>,
        redirect_finished_load_requests: bool,
    ) -> Result<Vec<(OffchainRequest<T>, RequestStatus)>, DispatchError> {
        Ok(hashes
            .iter()
            .take(Self::ITEMS_LIMIT)
            .flat_map(|hash| {
                if let Some(net_id) = network_id {
                    <Pallet<T>>::get_request_and_status(
                        redirect_finished_load_requests,
                        hash,
                        net_id,
                    )
                } else {
                    <Pallet<T>>::get_all_requests_and_stautses(
                        redirect_finished_load_requests,
                        hash,
                    )
                }
            })
            .collect())
    }

    fn get_all_requests_and_stautses(
        redirect_finished_load_requests: bool,
        hash: &H256,
    ) -> Vec<(OffchainRequest<T>, RequestStatus)> {
        Requests::<T>::iter()
            .filter(|(_, h, _)| h == hash)
            .map(|(net_id, hash, request)| {
                let status: RequestStatus =
                    Self::request_status(net_id, hash).unwrap_or(RequestStatus::Pending);
                (net_id, request, status)
            })
            .filter_map(|(net_id, req, status)| {
                let redirect_to_incoming = redirect_finished_load_requests
                    && req.is_load_incoming()
                    && status == RequestStatus::Done;
                if redirect_to_incoming {
                    let redirect_hash = LoadToIncomingRequestHash::<T>::get(net_id, hash);
                    Requests::<T>::get(net_id, redirect_hash).map(|req| {
                        let status: RequestStatus = Self::request_status(net_id, redirect_hash)
                            .unwrap_or(RequestStatus::Pending);
                        (req, status)
                    })
                } else {
                    Some((req, status))
                }
            })
            .collect()
    }

    fn get_request_and_status(
        redirect_finished_load_requests: bool,
        hash: &H256,
        net_id: <T as Config>::NetworkId,
    ) -> Vec<(OffchainRequest<T>, RequestStatus)> {
        Requests::<T>::get(net_id, hash)
            .zip({
                let status: Option<RequestStatus> = Self::request_status(net_id, hash);
                status
            })
            .and_then(|(req, status)| {
                let redirect_to_incoming = redirect_finished_load_requests
                    && req.is_load_incoming()
                    && status == RequestStatus::Done;
                if redirect_to_incoming {
                    let redirect_hash = LoadToIncomingRequestHash::<T>::get(net_id, hash);
                    Requests::<T>::get(net_id, redirect_hash).map(|req| {
                        let status: RequestStatus = Self::request_status(net_id, redirect_hash)
                            .unwrap_or(RequestStatus::Pending);
                        (req, status)
                    })
                } else {
                    Some((req, status))
                }
            })
            .map(|x| vec![x])
            .unwrap_or_default()
    }

    /// Get approved outgoing requests data and proofs.
    pub fn get_approved_requests(
        hashes: &[H256],
        network_id: Option<T::NetworkId>,
    ) -> Result<Vec<(OutgoingRequestEncoded, Vec<SignatureParams>)>, DispatchError> {
        let items = hashes
            .iter()
            .take(Self::ITEMS_LIMIT)
            .filter_map(|hash| {
                if let Some(net_id) = network_id {
                    if Self::request_status(net_id, hash)? == RequestStatus::ApprovalsReady {
                        let request: OffchainRequest<T> = Requests::get(net_id, hash)?;
                        match request {
                            OffchainRequest::Outgoing(request, hash) => {
                                let encoded = request
                                    .to_eth_abi(hash)
                                    .expect("this conversion was already tested; qed");
                                Self::get_approvals(&[hash], Some(net_id))
                                    .ok()?
                                    .pop()
                                    .map(|approvals| vec![(encoded, approvals)])
                            }
                            _ => None,
                        }
                    } else {
                        None
                    }
                } else {
                    Some(
                        RequestStatuses::<T>::iter()
                            .filter(|(_, _hash, v)| v == &RequestStatus::ApprovalsReady)
                            .filter_map(|(net_id, hash, _v)| {
                                let request: OffchainRequest<T> = Requests::get(net_id, hash)?;
                                match request {
                                    OffchainRequest::Outgoing(request, hash) => {
                                        let encoded = request
                                            .to_eth_abi(hash)
                                            .expect("this conversion was already tested; qed");
                                        Self::get_approvals(&[hash], Some(net_id))
                                            .ok()?
                                            .pop()
                                            .map(|approvals| (encoded, approvals))
                                    }
                                    _ => None,
                                }
                            })
                            .collect(),
                    )
                }
            })
            .flatten()
            .collect();
        Ok(items)
    }

    /// Get requests approvals.
    pub fn get_approvals(
        hashes: &[H256],
        network_id: Option<T::NetworkId>,
    ) -> Result<Vec<Vec<SignatureParams>>, DispatchError> {
        Ok(hashes
            .iter()
            .take(Self::ITEMS_LIMIT)
            .flat_map(|hash| {
                if let Some(net_id) = network_id {
                    vec![RequestApprovals::<T>::get(net_id, hash)
                        .into_iter()
                        .collect()]
                } else {
                    RequestApprovals::<T>::iter()
                        .filter(|(_, h, _)| h == hash)
                        .map(|(_, _, v)| v.into_iter().collect::<Vec<_>>())
                        .collect()
                }
            })
            .collect())
    }

    /// Get account requests list.
    pub fn get_account_requests(
        account: &T::AccountId,
        status_filter: Option<RequestStatus>,
    ) -> Result<Vec<(T::NetworkId, H256)>, DispatchError> {
        let mut requests: Vec<(T::NetworkId, H256)> = Self::account_requests(account);
        if let Some(filter) = status_filter {
            requests.retain(|(net_id, h)| Self::request_status(net_id, h).unwrap() == filter)
        }
        Ok(requests)
    }

    /// Get registered assets and tokens.
    pub fn get_registered_assets(
        network_id: Option<T::NetworkId>,
    ) -> Result<
        Vec<(
            AssetKind,
            (AssetIdOf<T>, BalancePrecision),
            Option<(H160, BalancePrecision)>,
        )>,
        DispatchError,
    > {
        Ok(iter_storage::<RegisteredAsset<T>, _, _, _, _, _>(
            network_id,
            |(network_id, asset_id, kind)| {
                let token_info = RegisteredSidechainToken::<T>::get(network_id, &asset_id)
                    .map(|x| H160(x.0))
                    .map(|address| {
                        let precision = SidechainAssetPrecision::<T>::get(network_id, &asset_id);
                        (address, precision)
                    });
                let asset_precision = assets::Pallet::<T>::get_asset_info(&asset_id).2;
                (kind, (asset_id, asset_precision), token_info)
            },
        ))
    }
}
