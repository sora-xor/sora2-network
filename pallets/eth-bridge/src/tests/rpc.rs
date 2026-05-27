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

use super::mock::*;
use crate::requests::{
    AssetKind, OffchainRequest, OutgoingAddAsset, OutgoingAddToken, OutgoingRequest,
    OutgoingRequestEncoded, OutgoingTransfer, RequestStatus,
};
use crate::tests::mock::{get_account_id_from_seed, ExtBuilder};
use crate::tests::ETH_NETWORK_ID;
use crate::{EthAddress, LegacyEthereumXorDecommissioned, LegacyEthereumXorDecommissionedAt};
use common::{VAL, XOR};
use sp_core::{sr25519, H256};

#[test]
fn get_account_requests_should_ignore_entries_without_status_when_filter_is_set() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let missing_status_hash = H256::repeat_byte(0x42);
        crate::AccountRequests::<Runtime>::insert(
            &alice,
            vec![(ETH_NETWORK_ID, missing_status_hash)],
        );

        let requests = EthBridge::get_account_requests(&alice, Some(RequestStatus::Pending))
            .expect("rpc get_account_requests should succeed");
        assert!(requests.is_empty());
    });
}

#[test]
fn get_approved_requests_without_network_filters_by_requested_hash() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let insert_add_token = |nonce: u64, token_address: EthAddress| {
            let request =
                OffchainRequest::outgoing(OutgoingRequest::AddToken(OutgoingAddToken::<Runtime> {
                    author: alice.clone(),
                    token_address,
                    symbol: "TOK".into(),
                    name: "Token".into(),
                    decimals: 18,
                    nonce,
                    network_id: ETH_NETWORK_ID,
                    timepoint: Default::default(),
                }));
            let hash = request.hash();
            crate::Requests::<Runtime>::insert(ETH_NETWORK_ID, hash, request);
            crate::RequestStatuses::<Runtime>::insert(
                ETH_NETWORK_ID,
                hash,
                RequestStatus::ApprovalsReady,
            );
            hash
        };

        let requested_hash = insert_add_token(1, EthAddress::from([1; 20]));
        let other_hash = insert_add_token(2, EthAddress::from([2; 20]));

        let approved =
            EthBridge::get_approved_requests(&[requested_hash], None).expect("rpc call succeeds");
        assert_eq!(approved.len(), 1);
        match &approved[0].0 {
            OutgoingRequestEncoded::AddToken(request) => assert_eq!(request.hash, requested_hash),
            other => panic!("unexpected request: {:?}", other),
        }
        assert!(
            !approved.iter().any(|(request, _)| {
                matches!(request, OutgoingRequestEncoded::AddToken(request) if request.hash == other_hash)
            }),
            "unrequested approvals-ready request must not be returned"
        );
    });
}

#[test]
fn generic_request_rpcs_virtualize_legacy_xor_approvals_ready_after_decommission() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        frame_system::Pallet::<Runtime>::set_block_number(2);
        crate::migration::decommission_legacy_ethereum_xor::<Runtime>();

        let request =
            OffchainRequest::outgoing(OutgoingRequest::Transfer(OutgoingTransfer::<Runtime> {
                from: alice.clone(),
                to: EthAddress::from([7; 20]),
                asset_id: XOR.into(),
                amount: 10u32.into(),
                nonce: 1,
                network_id: ETH_NETWORK_ID,
                timepoint: Default::default(),
            }));
        let hash = request.hash();
        crate::Requests::<Runtime>::insert(ETH_NETWORK_ID, hash, request);
        crate::RequestStatuses::<Runtime>::insert(
            ETH_NETWORK_ID,
            hash,
            RequestStatus::ApprovalsReady,
        );
        crate::RequestSubmissionHeight::<Runtime>::insert(ETH_NETWORK_ID, hash, 1);
        crate::AccountRequests::<Runtime>::insert(&alice, vec![(ETH_NETWORK_ID, hash)]);

        let expected_status =
            RequestStatus::Failed(crate::Error::<Runtime>::DeprecatedLegacyXor.into());
        assert_eq!(
            EthBridge::get_requests(&[hash], Some(ETH_NETWORK_ID), false)
                .expect("rpc call succeeds")
                .pop()
                .map(|(_, status)| status),
            Some(expected_status.clone())
        );
        assert_eq!(
            EthBridge::get_requests(&[hash], None, false)
                .expect("rpc call succeeds")
                .pop()
                .map(|(_, status)| status),
            Some(expected_status.clone())
        );
        assert!(
            EthBridge::get_account_requests(&alice, Some(RequestStatus::ApprovalsReady))
                .expect("rpc call succeeds")
                .is_empty()
        );
        assert_eq!(
            EthBridge::get_account_requests(&alice, Some(expected_status))
                .expect("rpc call succeeds"),
            vec![(ETH_NETWORK_ID, hash)]
        );
    });
}

#[test]
fn generic_request_rpcs_keep_new_xor_visible_at_decommission_height_boundary() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");

        frame_system::Pallet::<Runtime>::set_block_number(10);
        crate::migration::decommission_legacy_ethereum_xor::<Runtime>();
        OutgoingAddAsset::<Runtime> {
            author: EthBridge::authority_account().unwrap(),
            asset_id: XOR.into(),
            nonce: Default::default(),
            network_id: ETH_NETWORK_ID,
            timepoint: Default::default(),
        }
        .finalize()
        .unwrap();

        let insert_xor_transfer = |nonce: u64, submitted_at: u64| {
            let request =
                OffchainRequest::outgoing(OutgoingRequest::Transfer(OutgoingTransfer::<Runtime> {
                    from: alice.clone(),
                    to: EthAddress::from([nonce as u8; 20]),
                    asset_id: XOR.into(),
                    amount: 10u32.into(),
                    nonce,
                    network_id: ETH_NETWORK_ID,
                    timepoint: Default::default(),
                }));
            let hash = request.hash();
            crate::Requests::<Runtime>::insert(ETH_NETWORK_ID, hash, request);
            crate::RequestStatuses::<Runtime>::insert(
                ETH_NETWORK_ID,
                hash,
                RequestStatus::ApprovalsReady,
            );
            crate::RequestSubmissionHeight::<Runtime>::insert(ETH_NETWORK_ID, hash, submitted_at);
            hash
        };

        let old_hash = insert_xor_transfer(1, 9);
        let new_hash = insert_xor_transfer(2, 10);
        let legacy_status =
            RequestStatus::Failed(crate::Error::<Runtime>::DeprecatedLegacyXor.into());

        assert!(
            EthBridge::is_decommissioned_legacy_ethereum_xor_outgoing_transfer_request(
                ETH_NETWORK_ID,
                &old_hash
            )
        );
        assert!(
            !EthBridge::is_decommissioned_legacy_ethereum_xor_outgoing_transfer_request(
                ETH_NETWORK_ID,
                &new_hash
            )
        );
        assert_eq!(
            EthBridge::get_requests(&[old_hash], Some(ETH_NETWORK_ID), false)
                .expect("rpc call succeeds")
                .pop()
                .map(|(_, status)| status),
            Some(legacy_status)
        );
        assert_eq!(
            EthBridge::get_requests(&[new_hash], Some(ETH_NETWORK_ID), false)
                .expect("rpc call succeeds")
                .pop()
                .map(|(_, status)| status),
            Some(RequestStatus::ApprovalsReady)
        );
        assert!(
            EthBridge::get_approved_requests(&[old_hash], Some(ETH_NETWORK_ID))
                .expect("rpc call succeeds")
                .is_empty()
        );
        let approved = EthBridge::get_approved_requests(&[new_hash], Some(ETH_NETWORK_ID))
            .expect("rpc call succeeds");
        assert_eq!(approved.len(), 1);
        match &approved[0].0 {
            OutgoingRequestEncoded::Transfer(request) => {
                assert_eq!(request.tx_hash, new_hash);
                assert_eq!(request.amount, 10u32.into());
            }
            other => panic!("unexpected request: {:?}", other),
        }
    });
}

#[test]
fn missing_decommission_block_conservatively_hides_xor_outgoing_requests() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");

        LegacyEthereumXorDecommissioned::<Runtime>::put(true);
        LegacyEthereumXorDecommissionedAt::<Runtime>::kill();
        crate::RegisteredAsset::<Runtime>::insert(ETH_NETWORK_ID, XOR, AssetKind::Thischain);

        let request =
            OffchainRequest::outgoing(OutgoingRequest::Transfer(OutgoingTransfer::<Runtime> {
                from: alice,
                to: EthAddress::from([7; 20]),
                asset_id: XOR.into(),
                amount: 10u32.into(),
                nonce: 1,
                network_id: ETH_NETWORK_ID,
                timepoint: Default::default(),
            }));
        let hash = request.hash();
        crate::Requests::<Runtime>::insert(ETH_NETWORK_ID, hash, request);
        crate::RequestStatuses::<Runtime>::insert(
            ETH_NETWORK_ID,
            hash,
            RequestStatus::ApprovalsReady,
        );

        let expected_status =
            RequestStatus::Failed(crate::Error::<Runtime>::DeprecatedLegacyXor.into());
        assert_eq!(
            EthBridge::get_requests(&[hash], Some(ETH_NETWORK_ID), false)
                .expect("rpc call succeeds")
                .pop()
                .map(|(_, status)| status),
            Some(expected_status)
        );
        assert!(
            EthBridge::get_approved_requests(&[hash], Some(ETH_NETWORK_ID))
                .expect("rpc call succeeds")
                .is_empty()
        );
    });
}

#[test]
fn get_registered_assets_filters_deprecated_sidechain_mappings() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let val_token =
            crate::RegisteredSidechainToken::<Runtime>::get(ETH_NETWORK_ID, VAL).unwrap();
        crate::DeprecatedSidechainTokens::<Runtime>::insert(ETH_NETWORK_ID, val_token, true);

        let registered = EthBridge::get_registered_assets(Some(ETH_NETWORK_ID))
            .expect("rpc get_registered_assets should succeed");
        assert!(
            !registered
                .iter()
                .any(|(_, (asset_id, _), _)| *asset_id == VAL.into()),
            "deprecated sidechain token mappings must not be advertised"
        );
    });
}

#[test]
fn get_registered_assets_filters_unfinished_and_poisoned_ethereum_xor_states() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        crate::RegisteredAsset::<Runtime>::insert(ETH_NETWORK_ID, XOR, AssetKind::Thischain);
        crate::RegisteredSidechainToken::<Runtime>::remove(ETH_NETWORK_ID, XOR);

        let registered = EthBridge::get_registered_assets(Some(ETH_NETWORK_ID))
            .expect("rpc get_registered_assets should succeed");
        assert!(
            !registered
                .iter()
                .any(|(_, (asset_id, _), _)| *asset_id == XOR.into()),
            "pre-decommission XOR Thischain-looking storage must not be advertised"
        );

        crate::migration::decommission_legacy_ethereum_xor::<Runtime>();
        crate::RegisteredAsset::<Runtime>::insert(ETH_NETWORK_ID, XOR, AssetKind::Thischain);
        crate::RegisteredSidechainToken::<Runtime>::insert(
            ETH_NETWORK_ID,
            XOR,
            EthAddress::from([9; 20]),
        );

        let registered = EthBridge::get_registered_assets(Some(ETH_NETWORK_ID))
            .expect("rpc get_registered_assets should succeed");
        assert!(
            !registered
                .iter()
                .any(|(_, (asset_id, _), _)| *asset_id == XOR.into()),
            "XOR with stale sidechain mapping must not be advertised"
        );

        crate::RegisteredSidechainToken::<Runtime>::remove(ETH_NETWORK_ID, XOR);
        let registered = EthBridge::get_registered_assets(Some(ETH_NETWORK_ID))
            .expect("rpc get_registered_assets should succeed");
        assert!(
            registered.iter().any(|(kind, (asset_id, _), token_info)| {
                *kind == AssetKind::Thischain && *asset_id == XOR.into() && token_info.is_none()
            }),
            "clean post-decommission XOR Thischain registration should be advertised"
        );
    });
}
