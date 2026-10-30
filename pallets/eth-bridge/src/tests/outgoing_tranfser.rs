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
use super::Error;
use crate::offchain::SignatureParams;
use crate::requests::{OffchainRequest, OutgoingRequest, OutgoingTransfer, RequestStatus};
use crate::tests::{
    approve_last_request, last_event, last_outgoing_request, last_request, Assets, ETH_NETWORK_ID,
};
use crate::util::majority;
use crate::{
    AssetConfig, DeprecatedSidechainTokens, EthAddress, RegisteredSidechainToken,
    LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS,
};
use common::{eth, AssetInfoProvider, DEFAULT_BALANCE_PRECISION, KSM, PSWAP, USDT, VAL, XOR};
use ethereum_types::U256;
use frame_support::dispatch::{GetDispatchInfo, Pays, PostDispatchInfo};
use frame_support::sp_runtime::app_crypto::sp_core::{self, sr25519};
use frame_support::{assert_err, assert_err_ignore_postinfo, assert_ok};
use hex_literal::hex;
use secp256k1::{PublicKey, SecretKey};
use sp_core::{ecdsa, H160, H256};
use sp_std::prelude::*;
use std::collections::BTreeSet;
use std::str::FromStr;

fn malleate_signature(signature: &SignatureParams) -> SignatureParams {
    let curve_order = U256::from_big_endian(&hex!(
        "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141"
    ));
    let mut malleated = signature.clone();
    let mut s_bytes = [0u8; 32];
    (curve_order - U256::from_big_endian(&signature.s)).to_big_endian(&mut s_bytes);
    malleated.s = s_bytes;
    malleated.v = match signature.v {
        27 => 28,
        28 => 27,
        v => v ^ 1,
    };
    malleated
}

#[test]
fn recovery_halt_blocks_new_debits_approvals_and_outgoing_cancellation_refunds() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR.into(), &alice, &alice, 100_000u32.into()).unwrap();
        let recipient = EthAddress::repeat_byte(7);
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            XOR.into(),
            recipient,
            100u32.into(),
            net_id
        ));
        let (request, hash) = last_outgoing_request(net_id).unwrap();
        let (_, account, seed) = &state.networks[&net_id].ocw_keypairs[0];
        let (public, signature) = approval_params(&request, hash, seed);
        sp_io::storage::set(crate::recovery::HALT_KEY, &[1]);
        let before = sp_io::storage::root(sp_runtime::StateVersion::V1);
        assert_err!(
            EthBridge::transfer_to_sidechain(
                RuntimeOrigin::signed(alice),
                XOR.into(),
                recipient,
                100u32.into(),
                net_id
            ),
            Error::RecoveryHalted
        );
        assert_err!(
            EthBridge::approve_request(
                RuntimeOrigin::signed(account.clone()),
                public,
                hash,
                signature,
                net_id
            ),
            Error::RecoveryHalted
        );
        assert_err!(
            EthBridge::inner_abort_request(
                &OffchainRequest::outgoing(request),
                hash,
                Error::Forbidden.into(),
                net_id
            ),
            Error::RecoveryHalted
        );
        assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), before);
    });
}

fn approval_params(
    request: &OutgoingRequest<Runtime>,
    request_hash: H256,
    seed: &[u8; 32],
) -> (ecdsa::Public, SignatureParams) {
    let encoded = request.to_eth_abi(request_hash).unwrap();
    let secret = SecretKey::parse_slice(seed).unwrap();
    let public = PublicKey::from_secret_key(&secret);
    let signature = secp256k1::sign(&eth::prepare_message(encoded.as_raw()), &secret);

    (
        ecdsa::Public::from_raw(public.serialize_compressed()),
        super::get_signature_params(&signature),
    )
}

#[test]
fn outgoing_transfer_prepare_should_fail_for_unknown_network() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR.into(), &alice, &alice, 100u32.into()).unwrap();
        let request = OutgoingTransfer::<Runtime> {
            from: alice.clone(),
            to: EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            asset_id: XOR.into(),
            amount: 10u32.into(),
            nonce: Default::default(),
            network_id: ETH_NETWORK_ID + 1,
            timepoint: Default::default(),
        };
        assert_err!(
            request.prepare(H256::repeat_byte(0xAB)),
            Error::UnknownNetwork
        );
    });
}

#[test]
fn should_approve_outgoing_transfer() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR.into(), &alice, &alice, 100000u32.into()).unwrap();
        assert_eq!(
            Assets::total_balance(&XOR.into(), &alice).unwrap(),
            100000u32.into()
        );
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            XOR.into(),
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100_u32.into(),
            net_id,
        ));
        assert_eq!(
            Assets::total_balance(&XOR.into(), &alice).unwrap(),
            99900u32.into()
        );
        approve_last_request(&state, net_id).expect("request wasn't approved");
    });
}

#[test]
fn approve_request_retains_paid_metadata_and_rejects_duplicate_approvals() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR.into(), &alice, &alice, 100000u32.into()).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            XOR.into(),
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100_u32.into(),
            net_id,
        ));

        let (request, hash) = last_outgoing_request(net_id).expect("outgoing request exists");
        let (_signer, account_id, seed) = &state.networks[&net_id].ocw_keypairs[0];
        let (ocw_public, signature_params) = approval_params(&request, hash, seed);
        assert_ok!(
            EthBridge::approve_request(
                RuntimeOrigin::signed(account_id.clone()),
                ocw_public.clone(),
                hash,
                signature_params.clone(),
                net_id,
            ),
            PostDispatchInfo {
                pays_fee: Pays::Yes.into(),
                actual_weight: None,
            }
        );
        assert_err!(
            EthBridge::approve_request(
                RuntimeOrigin::signed(account_id.clone()),
                ocw_public,
                hash,
                signature_params,
                net_id
            ),
            Error::DuplicateApproval
        );
    });
}

#[test]
fn approve_request_retains_declared_weight_before_and_at_quorum() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR.into(), &alice, &alice, 100000u32.into()).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            XOR.into(),
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100_u32.into(),
            net_id,
        ));

        let (request, hash) = last_outgoing_request(net_id).expect("outgoing request exists");
        let keypairs = &state.networks[&net_id].ocw_keypairs;
        let additional = if EthBridge::is_additional_signature_needed(net_id, &request) {
            1
        } else {
            0
        };
        let sigs_needed = majority(keypairs.len()) + additional;

        for (i, (_signer, account_id, seed)) in keypairs.iter().take(sigs_needed).enumerate() {
            let (ocw_public, signature_params) = approval_params(&request, hash, seed);
            let info = crate::Call::<Runtime>::approve_request {
                ocw_public: ocw_public.clone(),
                hash,
                signature_params: signature_params.clone(),
                network_id: net_id,
            }
            .get_dispatch_info();
            let post_info = EthBridge::approve_request(
                RuntimeOrigin::signed(account_id.clone()),
                ocw_public,
                hash,
                signature_params,
                net_id,
            )
            .expect("valid peer approval succeeds");
            assert_eq!(
                post_info,
                PostDispatchInfo {
                    pays_fee: Pays::Yes.into(),
                    actual_weight: None,
                }
            );
            // Both branches retain the admission-validation work included in
            // the declaration instead of refunding to dispatch-only weights.
            assert_eq!(post_info.calc_actual_weight(&info), info.total_weight());
            assert_eq!(
                crate::RequestStatuses::<Runtime>::get(net_id, hash),
                Some(if i + 1 == sigs_needed {
                    RequestStatus::ApprovalsReady
                } else {
                    RequestStatus::Pending
                })
            );
        }
    });
}

#[test]
fn pending_peer_requires_one_additional_signature_before_finalizing() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR.into(), &alice, &alice, 100000u32.into()).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice),
            XOR.into(),
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100_u32.into(),
            net_id,
        ));
        let (request, hash) = last_outgoing_request(net_id).expect("outgoing request exists");
        crate::PendingPeer::<Runtime>::insert(net_id, AccountId::new([250u8; 32]));

        let keypairs = &state.networks[&net_id].ocw_keypairs;
        let ordinary_quorum = majority(keypairs.len());
        for (_signer, account_id, seed) in keypairs.iter().take(ordinary_quorum) {
            let (ocw_public, signature_params) = approval_params(&request, hash, seed);
            assert_ok!(
                EthBridge::approve_request(
                    RuntimeOrigin::signed(account_id.clone()),
                    ocw_public,
                    hash,
                    signature_params,
                    net_id,
                ),
                PostDispatchInfo {
                    pays_fee: Pays::Yes.into(),
                    actual_weight: None,
                }
            );
        }
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net_id, hash),
            Some(RequestStatus::Pending)
        );
        assert!(crate::RequestsQueue::<Runtime>::get(net_id).contains(&hash));

        let (_signer, account_id, seed) = &keypairs[ordinary_quorum];
        let (ocw_public, signature_params) = approval_params(&request, hash, seed);
        assert_ok!(
            EthBridge::approve_request(
                RuntimeOrigin::signed(account_id.clone()),
                ocw_public,
                hash,
                signature_params,
                net_id,
            ),
            PostDispatchInfo {
                pays_fee: Pays::Yes.into(),
                actual_weight: None,
            }
        );
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net_id, hash),
            Some(RequestStatus::ApprovalsReady)
        );
        assert!(!crate::RequestsQueue::<Runtime>::get(net_id).contains(&hash));
    });
}

#[test]
fn approve_request_retains_declared_weight_after_quorum_is_already_reached() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR.into(), &alice, &alice, 100000u32.into()).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            XOR.into(),
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100_u32.into(),
            net_id,
        ));

        let (request, hash) = last_outgoing_request(net_id).expect("outgoing request exists");
        let keypairs = &state.networks[&net_id].ocw_keypairs;
        let additional = if EthBridge::is_additional_signature_needed(net_id, &request) {
            1
        } else {
            0
        };
        let sigs_needed = majority(keypairs.len()) + additional;

        for (_signer, account_id, seed) in keypairs.iter().take(sigs_needed) {
            let (ocw_public, signature_params) = approval_params(&request, hash, seed);
            assert_ok!(EthBridge::approve_request(
                RuntimeOrigin::signed(account_id.clone()),
                ocw_public,
                hash,
                signature_params,
                net_id,
            ));
        }
        assert!(matches!(
            crate::RequestStatuses::<Runtime>::get(net_id, &hash),
            Some(RequestStatus::ApprovalsReady)
        ));
        assert!(!crate::RequestsQueue::<Runtime>::get(net_id).contains(&hash));

        System::reset_events();
        let (_signer, duplicate_account_id, duplicate_seed) = &keypairs[0];
        let (duplicate_public, duplicate_signature) =
            approval_params(&request, hash, duplicate_seed);
        assert_err!(
            EthBridge::approve_request(
                RuntimeOrigin::signed(duplicate_account_id.clone()),
                duplicate_public,
                hash,
                duplicate_signature,
                net_id
            ),
            Error::DuplicateApproval
        );
        assert!(last_event().is_none());
        assert_eq!(
            crate::RequestApprovers::<Runtime>::get(net_id, &hash).len(),
            sigs_needed
        );

        System::reset_events();
        let (_signer, late_account_id, late_seed) = &keypairs[sigs_needed];
        let (late_public, late_signature) = approval_params(&request, hash, late_seed);
        assert_ok!(
            EthBridge::approve_request(
                RuntimeOrigin::signed(late_account_id.clone()),
                late_public,
                hash,
                late_signature,
                net_id,
            ),
            PostDispatchInfo {
                pays_fee: Pays::Yes.into(),
                actual_weight: None,
            }
        );
        assert!(last_event().is_none());
        assert!(matches!(
            crate::RequestStatuses::<Runtime>::get(net_id, &hash),
            Some(RequestStatus::ApprovalsReady)
        ));
        assert!(!crate::RequestsQueue::<Runtime>::get(net_id).contains(&hash));
        assert_eq!(
            crate::RequestApprovers::<Runtime>::get(net_id, &hash).len(),
            sigs_needed + 1
        );
    });
}

#[test]
fn approve_request_rejects_more_than_the_maximum_stored_approvals() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR.into(), &alice, &alice, 100000u32.into()).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice),
            XOR.into(),
            EthAddress::repeat_byte(9),
            100u32.into(),
            net_id
        ));
        let (request, hash) = last_outgoing_request(net_id).unwrap();
        let approvals = (0..=crate::MAX_PEERS)
            .map(|index| SignatureParams {
                v: index as u8,
                ..Default::default()
            })
            .collect::<BTreeSet<_>>();
        crate::RequestApprovals::<Runtime>::insert(net_id, hash, approvals);
        let (_, account, seed) = &state.networks[&net_id].ocw_keypairs[0];
        let (public, signature) = approval_params(&request, hash, seed);
        frame_support::assert_noop!(
            EthBridge::approve_request(
                RuntimeOrigin::signed(account.clone()),
                public,
                hash,
                signature,
                net_id
            ),
            Error::TooManyApprovals
        );
    });
}

#[test]
fn approve_request_rejects_terminal_request_statuses() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR.into(), &alice, &alice, 100000u32.into()).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice),
            XOR.into(),
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100_u32.into(),
            net_id,
        ));
        let (request, hash) = last_outgoing_request(net_id).expect("outgoing request exists");
        let (_signer, account_id, seed) = &state.networks[&net_id].ocw_keypairs[0];
        let (ocw_public, signature_params) = approval_params(&request, hash, seed);
        let error = Error::Other.into();
        for status in [
            RequestStatus::Frozen,
            RequestStatus::Failed(error),
            RequestStatus::Done,
            RequestStatus::Broken(error, error),
        ] {
            crate::RequestStatuses::<Runtime>::insert(net_id, hash, status);
            assert_err_ignore_postinfo!(
                EthBridge::approve_request(
                    RuntimeOrigin::signed(account_id.clone()),
                    ocw_public.clone(),
                    hash,
                    signature_params.clone(),
                    net_id,
                ),
                Error::RequestIsNotReady
            );
            assert!(crate::RequestApprovals::<Runtime>::get(net_id, hash).is_empty());
            assert!(crate::RequestApprovers::<Runtime>::get(net_id, hash).is_empty());
        }
    });
}

#[test]
fn approve_request_dispatch_info_reserves_finalizing_weight() {
    let call = crate::Call::<Runtime>::approve_request {
        ocw_public: ecdsa::Public::from_raw([0; 33]),
        hash: H256::zero(),
        signature_params: SignatureParams {
            r: [0; 32],
            s: [0; 32],
            v: 27,
        },
        network_id: ETH_NETWORK_ID,
    };

    assert!(call
        .get_dispatch_info()
        .total_weight()
        .all_gte(<() as crate::WeightInfo>::approve_request_finalize()));
}

#[test]
fn should_reject_outgoing_transfer_when_xor_mapping_points_to_legacy_ethereum_token() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        RegisteredSidechainToken::<Runtime>::insert(net_id, XOR, LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS);
        Assets::mint_to(&XOR.into(), &alice, &alice, 100000u32.into()).unwrap();

        assert_err!(
            EthBridge::transfer_to_sidechain(
                RuntimeOrigin::signed(alice.clone()),
                XOR.into(),
                EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
                100_u32.into(),
                net_id,
            ),
            Error::DeprecatedLegacyXor
        );
        assert_eq!(
            Assets::total_balance(&XOR.into(), &alice).unwrap(),
            100000u32.into()
        );
        assert!(crate::RequestsQueue::<Runtime>::get(net_id).is_empty());
    });
}

#[test]
fn should_reject_outgoing_transfer_when_any_asset_points_to_legacy_ethereum_xor_token() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        RegisteredSidechainToken::<Runtime>::insert(net_id, VAL, LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS);
        Assets::mint_to(&VAL.into(), &alice, &alice, 100000u32.into()).unwrap();

        assert_err!(
            EthBridge::transfer_to_sidechain(
                RuntimeOrigin::signed(alice.clone()),
                VAL.into(),
                EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
                100_u32.into(),
                net_id,
            ),
            Error::DeprecatedLegacyXor
        );
        assert_eq!(
            Assets::total_balance(&VAL.into(), &alice).unwrap(),
            100000u32.into()
        );
        assert!(crate::RequestsQueue::<Runtime>::get(net_id).is_empty());
    });
}

#[test]
fn should_fail_queued_outgoing_transfer_if_mapping_becomes_deprecated_before_approval() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let val_token = RegisteredSidechainToken::<Runtime>::get(net_id, VAL).unwrap();
        Assets::mint_to(&VAL.into(), &alice, &alice, 1000u32.into()).unwrap();

        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            VAL.into(),
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100_u32.into(),
            net_id,
        ));
        let (request, request_hash) = last_outgoing_request(net_id).unwrap();
        assert_eq!(
            Assets::total_balance(&VAL.into(), &alice).unwrap(),
            900u32.into()
        );

        DeprecatedSidechainTokens::<Runtime>::insert(net_id, val_token, true);

        let encoded = request.to_eth_abi(request_hash).unwrap();
        let keypairs = &state.networks[&net_id].ocw_keypairs;
        let additional = if EthBridge::is_additional_signature_needed(net_id, &request) {
            1
        } else {
            0
        };
        let sigs_needed = majority(keypairs.len()) + additional;
        for (_signer, account_id, seed) in keypairs.iter().take(sigs_needed) {
            let secret = SecretKey::parse_slice(seed).unwrap();
            let public = PublicKey::from_secret_key(&secret);
            let msg = eth::prepare_message(encoded.as_raw());
            let sig_pair = secp256k1::sign(&msg, &secret);
            let signature_params = super::get_signature_params(&sig_pair);
            assert_ok!(EthBridge::approve_request(
                RuntimeOrigin::signed(account_id.clone()),
                ecdsa::Public::from_raw(public.serialize_compressed()),
                request_hash,
                signature_params,
                net_id,
            ));
        }
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net_id, request_hash),
            Some(RequestStatus::Broken(
                Error::DeprecatedLegacyXor.into(),
                Error::PublicApprovalsRequireSettlement.into()
            ))
        );
        assert!(!crate::RequestsQueue::<Runtime>::get(net_id).contains(&request_hash));
        assert_eq!(
            crate::RequestApprovals::<Runtime>::get(net_id, request_hash).len(),
            sigs_needed
        );
        assert_eq!(
            crate::RequestApprovers::<Runtime>::get(net_id, request_hash).len(),
            sigs_needed
        );
        assert_eq!(
            Assets::total_balance(&VAL.into(), &alice).unwrap(),
            900u32.into()
        );
    });
}

#[test]
fn should_reserve_and_burn_sidechain_asset_in_outgoing_transfer() {
    let net_id = ETH_NETWORK_ID;
    let mut builder = ExtBuilder::new();
    builder.add_network(
        vec![AssetConfig::Sidechain {
            id: USDT.into(),
            sidechain_id: H160(hex!("dAC17F958D2ee523a2206206994597C13D831ec7")),
            owned: false,
            precision: DEFAULT_BALANCE_PRECISION,
        }],
        None,
        None,
        Default::default(),
    );
    let (mut ext, state) = builder.build();

    ext.execute_with(|| {
        let bridge_acc = &state.networks[&net_id].config.bridge_account_id;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&USDT.into(), &alice, &alice, 100000u32.into()).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            USDT.into(),
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100_u32.into(),
            net_id,
        ));
        assert_eq!(Assets::free_balance(&USDT.into(), &bridge_acc).unwrap(), 0);
        // Sidechain asset was reserved.
        assert_eq!(
            Assets::total_balance(&USDT.into(), &bridge_acc).unwrap(),
            100u32.into()
        );
        approve_last_request(&state, net_id).expect("request wasn't approved");
        // Sidechain asset was burnt.
        assert_eq!(Assets::total_balance(&USDT.into(), &bridge_acc).unwrap(), 0);
        assert_eq!(
            Assets::free_balance(&USDT.into(), &bridge_acc).unwrap(),
            Assets::total_balance(&USDT.into(), &bridge_acc).unwrap()
        );
    });
}

#[test]
fn should_reserve_and_unreserve_thischain_asset_in_outgoing_transfer() {
    let net_id = ETH_NETWORK_ID;
    let mut builder = ExtBuilder::new();
    builder.add_network(
        vec![AssetConfig::Thischain { id: PSWAP.into() }],
        None,
        None,
        Default::default(),
    );
    let (mut ext, state) = builder.build();

    ext.execute_with(|| {
        let bridge_acc = &state.networks[&net_id].config.bridge_account_id;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&PSWAP.into(), &alice, &alice, 100000u32.into()).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            PSWAP.into(),
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100_u32.into(),
            net_id,
        ));
        assert_eq!(Assets::free_balance(&PSWAP.into(), &bridge_acc).unwrap(), 0);
        // Thischain asset was reserved.
        assert_eq!(
            Assets::total_balance(&PSWAP.into(), &bridge_acc).unwrap(),
            100u32.into()
        );
        approve_last_request(&state, net_id).expect("request wasn't approved");
        // Thischain asset was unreserved.
        assert_eq!(
            Assets::total_balance(&PSWAP.into(), &bridge_acc).unwrap(),
            100u32.into()
        );
        assert_eq!(
            Assets::free_balance(&PSWAP.into(), &bridge_acc).unwrap(),
            Assets::total_balance(&PSWAP.into(), &bridge_acc).unwrap()
        );
    });
}

#[test]
fn should_not_transfer() {
    let (mut ext, _) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        assert_err!(
            EthBridge::transfer_to_sidechain(
                RuntimeOrigin::signed(alice.clone()),
                KSM.into(),
                EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
                100_u32.into(),
                net_id,
            ),
            Error::UnsupportedToken
        );
        assert!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            XOR.into(),
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100_000_000_u32.into(),
            net_id,
        )
        .is_err());
    });
}

#[test]
fn should_register_outgoing_transfer() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR.into(), &alice, &alice, 100000u32.into()).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            XOR.into(),
            EthAddress::from([1; 20]),
            100u32.into(),
            net_id,
        ));
        let outgoing_transfer = OutgoingTransfer::<Runtime> {
            from: alice.clone(),
            to: EthAddress::from([1; 20]),
            asset_id: XOR.into(),
            amount: 100_u32.into(),
            nonce: 3,
            network_id: ETH_NETWORK_ID,
            timepoint: bridge_multisig::Pallet::<Runtime>::thischain_timepoint(),
        };
        let last_request = last_request(net_id).unwrap();
        match last_request {
            OffchainRequest::Outgoing(OutgoingRequest::Transfer(r), _) => {
                assert_eq!(r, outgoing_transfer)
            }
            _ => panic!("Invalid off-chain request"),
        }
    });
}

#[test]
fn abort_request_removes_all_duplicate_hashes_from_queue() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR.into(), &alice, &alice, 100u32.into()).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            XOR.into(),
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            10u32.into(),
            net_id,
        ));

        let request_hash = last_outgoing_request(net_id)
            .expect("outgoing request should exist")
            .1;
        crate::RequestsQueue::<Runtime>::mutate(net_id, |queue| queue.push(request_hash));
        assert_eq!(
            crate::RequestsQueue::<Runtime>::get(net_id)
                .iter()
                .filter(|hash| **hash == request_hash)
                .count(),
            2
        );

        let bridge_account = state.networks[&net_id].config.bridge_account_id.clone();
        assert_ok!(EthBridge::abort_request(
            RuntimeOrigin::signed(bridge_account),
            request_hash,
            Error::Cancelled.into(),
            net_id,
        ));
        assert_eq!(
            crate::RequestsQueue::<Runtime>::get(net_id)
                .iter()
                .filter(|hash| **hash == request_hash)
                .count(),
            0
        );
        assert!(matches!(
            crate::RequestStatuses::<Runtime>::get(net_id, request_hash),
            Some(RequestStatus::Failed(_))
        ));
    });
}

#[test]
fn ocw_should_handle_outgoing_request() {
    let (mut ext, mut state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR.into(), &alice, &alice, 100).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            XOR.into(),
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100,
            net_id,
        ));
        state.run_next_offchain_and_dispatch_txs();
        let hash = last_outgoing_request(net_id).unwrap().1;
        assert_eq!(
            crate::RequestApprovals::<Runtime>::get(net_id, hash).len(),
            1
        );
        assert_eq!(
            crate::RequestApprovers::<Runtime>::get(net_id, hash).len(),
            1
        );
    });
}

#[test]
fn ocw_should_not_handle_outgoing_request_twice() {
    let (mut ext, mut state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR.into(), &alice, &alice, 100).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            XOR.into(),
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100,
            net_id,
        ));
        state.run_next_offchain_and_dispatch_txs();
        let hash = last_outgoing_request(net_id).unwrap().1;
        assert_eq!(
            crate::RequestApprovals::<Runtime>::get(net_id, hash).len(),
            1
        );
        assert_eq!(
            crate::RequestApprovers::<Runtime>::get(net_id, hash).len(),
            1
        );
        state.run_next_offchain_and_dispatch_txs();
        assert_eq!(
            crate::RequestApprovals::<Runtime>::get(net_id, hash).len(),
            1
        );
        assert_eq!(
            crate::RequestApprovers::<Runtime>::get(net_id, hash).len(),
            1
        );
    });
}

#[test]
fn outgoing_approval_rejects_invalid_signature_recovery_id_without_state_changes() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let network = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR.into(), &alice, &alice, 100).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice),
            XOR.into(),
            H160::repeat_byte(9),
            100,
            network,
        ));
        let (request, hash) = last_outgoing_request(network).unwrap();
        let (_, account, seed) = &state.networks[&network].ocw_keypairs[0];
        let (public, signature) = approval_params(&request, hash, seed);
        let wrong_parity = if signature.v == 27 { 28 } else { 27 };

        for recovery_id in [2, 3, 26, 29, 30, 35, 255, wrong_parity, wrong_parity - 27] {
            let mut invalid = signature.clone();
            invalid.v = recovery_id;
            let before = sp_io::storage::root(sp_runtime::StateVersion::V1);
            assert_err!(
                EthBridge::validate_outgoing_approval(&public, &hash, &invalid, account, network),
                Error::InvalidSignature
            );
            assert_err!(
                EthBridge::approve_request(
                    RuntimeOrigin::signed(account.clone()),
                    public.clone(),
                    hash,
                    invalid,
                    network,
                ),
                Error::InvalidSignature
            );
            assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), before);
        }
    });
}

#[test]
fn outgoing_approval_accepts_matching_signature_recovery_id_encodings() {
    for raw_recovery_id in [false, true] {
        for high_s in [false, true] {
            let (mut ext, state) = ExtBuilder::default().build();
            ext.execute_with(|| {
                let network = ETH_NETWORK_ID;
                let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
                Assets::mint_to(&XOR.into(), &alice, &alice, 100).unwrap();
                assert_ok!(EthBridge::transfer_to_sidechain(
                    RuntimeOrigin::signed(alice),
                    XOR.into(),
                    H160::repeat_byte(9),
                    100,
                    network,
                ));
                let (request, hash) = last_outgoing_request(network).unwrap();
                let (_, account, seed) = &state.networks[&network].ocw_keypairs[0];
                let (public, mut signature) = approval_params(&request, hash, seed);
                if high_s {
                    signature = malleate_signature(&signature);
                }
                if raw_recovery_id {
                    signature.v -= 27;
                }
                assert_ok!(EthBridge::validate_outgoing_approval(
                    &public, &hash, &signature, account, network,
                ));
                assert_ok!(EthBridge::approve_request(
                    RuntimeOrigin::signed(account.clone()),
                    public,
                    hash,
                    signature.clone(),
                    network,
                ));
                assert_eq!(
                    crate::RequestApprovals::<Runtime>::get(network, hash),
                    BTreeSet::from([signature.canonical_recovery_id().unwrap()])
                );
                assert_eq!(
                    crate::RequestApprovers::<Runtime>::get(network, hash),
                    BTreeSet::from([account.clone()])
                );
            });
        }
    }
}

#[test]
fn same_peer_malleated_signature_does_not_advance_quorum() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR.into(), &alice, &alice, 100).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            XOR.into(),
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100,
            net_id,
        ));

        let (request, hash) = last_outgoing_request(net_id).expect("outgoing request exists");
        let encoded = request.to_eth_abi(hash).unwrap();
        let (_signer, account_id, seed) = &state.networks[&net_id].ocw_keypairs[0];
        let secret = SecretKey::parse_slice(seed).unwrap();
        let public = PublicKey::from_secret_key(&secret);
        let sig_pair = secp256k1::sign(&eth::prepare_message(encoded.as_raw()), &secret);
        let signature_params = super::get_signature_params(&sig_pair);
        let malleated_signature = malleate_signature(&signature_params);
        let ocw_public = ecdsa::Public::from_raw(public.serialize_compressed());

        assert!(EthBridge::verify_message(
            encoded.as_raw(),
            &signature_params,
            &ocw_public,
            account_id,
        ));
        assert!(EthBridge::verify_message(
            encoded.as_raw(),
            &malleated_signature,
            &ocw_public,
            account_id,
        ));

        assert_ok!(EthBridge::approve_request(
            RuntimeOrigin::signed(account_id.clone()),
            ocw_public.clone(),
            hash,
            signature_params,
            net_id,
        ));
        assert_eq!(
            crate::RequestApprovals::<Runtime>::get(net_id, &hash).len(),
            1
        );
        assert_eq!(
            crate::RequestApprovers::<Runtime>::get(net_id, &hash).len(),
            1
        );
        assert!(matches!(
            crate::RequestStatuses::<Runtime>::get(net_id, &hash),
            Some(RequestStatus::Pending)
        ));

        System::reset_events();
        assert_err!(
            EthBridge::approve_request(
                RuntimeOrigin::signed(account_id.clone()),
                ocw_public,
                hash,
                malleated_signature,
                net_id,
            ),
            Error::DuplicateApproval
        );
        assert_eq!(
            crate::RequestApprovals::<Runtime>::get(net_id, &hash).len(),
            1
        );
        assert_eq!(
            crate::RequestApprovers::<Runtime>::get(net_id, &hash).len(),
            1
        );
        assert!(matches!(
            crate::RequestStatuses::<Runtime>::get(net_id, &hash),
            Some(RequestStatus::Pending)
        ));
        assert!(last_event().is_none());
    });
}

#[test]
fn quorum_counts_distinct_approvers_even_if_signature_storage_drifts() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR.into(), &alice, &alice, 100).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            XOR.into(),
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100,
            net_id,
        ));

        let (request, hash) = last_outgoing_request(net_id).expect("outgoing request exists");
        let encoded = request.to_eth_abi(hash).unwrap();
        let first_peer = &state.networks[&net_id].ocw_keypairs[0];
        let first_secret = SecretKey::parse_slice(&first_peer.2).unwrap();
        let first_public = ecdsa::Public::from_raw(
            PublicKey::from_secret_key(&first_secret).serialize_compressed(),
        );
        let first_signature = super::get_signature_params(&secp256k1::sign(
            &eth::prepare_message(encoded.as_raw()),
            &first_secret,
        ));

        assert_ok!(EthBridge::approve_request(
            RuntimeOrigin::signed(first_peer.1.clone()),
            first_public,
            hash,
            first_signature.clone(),
            net_id,
        ));

        let mut approvals = crate::RequestApprovals::<Runtime>::get(net_id, &hash);
        approvals.insert(malleate_signature(&first_signature));
        crate::RequestApprovals::<Runtime>::insert(net_id, &hash, approvals);
        assert_eq!(
            crate::RequestApprovals::<Runtime>::get(net_id, &hash).len(),
            2
        );
        assert_eq!(
            crate::RequestApprovers::<Runtime>::get(net_id, &hash).len(),
            1
        );

        let second_peer = &state.networks[&net_id].ocw_keypairs[1];
        let second_secret = SecretKey::parse_slice(&second_peer.2).unwrap();
        let second_public = ecdsa::Public::from_raw(
            PublicKey::from_secret_key(&second_secret).serialize_compressed(),
        );
        let second_signature = super::get_signature_params(&secp256k1::sign(
            &eth::prepare_message(encoded.as_raw()),
            &second_secret,
        ));

        System::reset_events();
        assert_ok!(EthBridge::approve_request(
            RuntimeOrigin::signed(second_peer.1.clone()),
            second_public,
            hash,
            second_signature,
            net_id,
        ));

        assert_eq!(
            crate::RequestApprovals::<Runtime>::get(net_id, &hash).len(),
            2
        );
        assert_eq!(
            crate::RequestApprovers::<Runtime>::get(net_id, &hash).len(),
            2
        );
        assert!(matches!(
            crate::RequestStatuses::<Runtime>::get(net_id, &hash),
            Some(RequestStatus::Pending)
        ));
        assert!(last_event().is_none());
    });
}

#[test]
fn failed_finalization_allows_fresh_approvals_after_resubmission() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let amount = 100u32.into();
        Assets::mint_to(&XOR.into(), &alice, &alice, 1_000_000u32.into()).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            XOR.into(),
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            amount,
            net_id,
        ));
        let (request, hash) = last_outgoing_request(net_id).expect("outgoing request exists");
        let bridge_account = state.networks[&net_id].config.bridge_account_id.clone();
        Assets::unreserve(&XOR.into(), &bridge_account, amount).unwrap();
        System::reset_events();

        let encoded = request.to_eth_abi(hash).unwrap();
        let keypairs = &state.networks[&net_id].ocw_keypairs;
        let additional = if EthBridge::is_additional_signature_needed(net_id, &request) {
            1
        } else {
            0
        };
        let sigs_needed = majority(keypairs.len()) + additional;

        for (i, (_signer, account_id, seed)) in keypairs.iter().enumerate().take(sigs_needed) {
            let secret = SecretKey::parse_slice(seed).unwrap();
            let public = PublicKey::from_secret_key(&secret);
            let msg = eth::prepare_message(encoded.as_raw());
            let sig_pair = secp256k1::sign(&msg, &secret);
            let signature_params = super::get_signature_params(&sig_pair);
            assert_ok!(EthBridge::approve_request(
                RuntimeOrigin::signed(account_id.clone()),
                ecdsa::Public::from_raw(public.serialize_compressed()),
                hash,
                signature_params,
                net_id,
            ));
            if i + 1 == sigs_needed {
                let event_hash = match last_event().expect("event expected") {
                    RuntimeEvent::EthBridge(crate::Event::RequestFinalizationFailed(
                        event_hash,
                    ))
                    | RuntimeEvent::EthBridge(crate::Event::CancellationFailed(event_hash)) => {
                        event_hash
                    }
                    other => panic!("unexpected event: {:?}", other),
                };
                assert_eq!(event_hash, hash);
            }
        }

        match crate::RequestStatuses::<Runtime>::get(net_id, &hash) {
            Some(RequestStatus::Failed(_)) | Some(RequestStatus::Broken(_, _)) => {}
            other => panic!("unexpected status: {:?}", other),
        }
        assert!(
            !crate::RequestApprovals::<Runtime>::get(net_id, &hash).is_empty(),
            "approvals should remain until operators explicitly reset them"
        );
        assert!(
            !crate::RequestApprovers::<Runtime>::get(net_id, &hash).is_empty(),
            "approvers should remain until operators explicitly reset them"
        );

        assert_ok!(EthBridge::reset_request_signatures(
            RuntimeOrigin::root(),
            net_id,
            hash
        ));
        match last_event().expect("signatures cleared event expected") {
            RuntimeEvent::EthBridge(crate::Event::RequestSignaturesCleared(event_hash)) => {
                assert_eq!(event_hash, hash);
            }
            other => panic!("unexpected event: {:?}", other),
        }

        assert!(
            crate::RequestApprovals::<Runtime>::get(net_id, &hash).is_empty(),
            "reset must clear approvals before retry"
        );
        assert!(
            crate::RequestApprovers::<Runtime>::get(net_id, &hash).is_empty(),
            "reset must clear approvers before retry"
        );

        crate::RequestStatuses::<Runtime>::insert(net_id, &hash, RequestStatus::Pending);
        crate::RequestsQueue::<Runtime>::mutate(net_id, |queue| queue.push(hash));
        System::reset_events();

        let (_signer, account_id, seed) = &keypairs[0];
        let secret = SecretKey::parse_slice(seed).unwrap();
        let public = PublicKey::from_secret_key(&secret);
        let msg = eth::prepare_message(encoded.as_raw());
        let sig_pair = secp256k1::sign(&msg, &secret);
        let signature_params = super::get_signature_params(&sig_pair);
        assert_ok!(EthBridge::approve_request(
            RuntimeOrigin::signed(account_id.clone()),
            ecdsa::Public::from_raw(public.serialize_compressed()),
            hash,
            signature_params,
            net_id,
        ));
        assert_eq!(
            crate::RequestApprovals::<Runtime>::get(net_id, &hash).len(),
            1
        );
        assert_eq!(
            crate::RequestApprovers::<Runtime>::get(net_id, &hash).len(),
            1
        );
    });
}

#[test]
fn reset_request_signatures_requires_failed_status() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR.into(), &alice, &alice, 1_000_000u32.into()).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            XOR.into(),
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100u32.into(),
            net_id,
        ));
        let (_request, hash) = last_outgoing_request(net_id).expect("outgoing request exists");
        assert_err!(
            EthBridge::reset_request_signatures(RuntimeOrigin::root(), net_id, hash),
            Error::RequestStatusNotResettable
        );

        // Force the request into Failed state and confirm the extrinsic succeeds afterward.
        crate::RequestStatuses::<Runtime>::insert(
            net_id,
            &hash,
            RequestStatus::Failed(Error::Cancelled.into()),
        );
        assert_ok!(EthBridge::reset_request_signatures(
            RuntimeOrigin::root(),
            net_id,
            hash
        ));
        match last_event().expect("signatures cleared event expected") {
            RuntimeEvent::EthBridge(crate::Event::RequestSignaturesCleared(event_hash)) => {
                assert_eq!(event_hash, hash);
            }
            other => panic!("unexpected event: {:?}", other),
        }
    });
}

#[test]
fn requests_queue_respects_limit() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let max = MaxRequestsPerQueueConst::get() as usize;
        Assets::mint_to(&XOR.into(), &alice, &alice, 10_000_000u32.into()).unwrap();

        for _ in 0..max {
            assert_ok!(EthBridge::transfer_to_sidechain(
                RuntimeOrigin::signed(alice.clone()),
                XOR.into(),
                EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
                10u32.into(),
                net_id,
            ));
        }

        assert_err!(
            EthBridge::transfer_to_sidechain(
                RuntimeOrigin::signed(alice.clone()),
                XOR.into(),
                EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
                10u32.into(),
                net_id,
            ),
            Error::RequestsQueueFull
        );
        assert_eq!(crate::Requests::<Runtime>::iter_prefix(net_id).count(), max);
        assert_eq!(crate::RequestsQueue::<Runtime>::get(net_id).len(), max);
    });
}

#[test]
fn should_block_v1_signature_domain_requests_without_toggle() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&PSWAP.into(), &alice, &alice, 1000).unwrap();

        crate::BridgeSignatureVersions::<Runtime>::insert(
            net_id,
            crate::BridgeSignatureVersion::V1,
        );
        assert_err!(
            EthBridge::transfer_to_sidechain(
                RuntimeOrigin::signed(alice),
                PSWAP.into(),
                EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
                10,
                net_id,
            ),
            Error::WeakLegacySigningDisabled
        );
    });
}

#[test]
fn should_use_legacy_master_contract_path_for_val_only() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let to = EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap();
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(&alice);
        let timepoint = bridge_multisig::Pallet::<Runtime>::thischain_timepoint();

        let xor_req = OutgoingTransfer::<Runtime> {
            from: alice.clone(),
            to,
            asset_id: XOR.into(),
            amount: 10,
            nonce,
            network_id: net_id,
            timepoint,
        };
        let val_req = OutgoingTransfer::<Runtime> {
            from: alice,
            to,
            asset_id: VAL.into(),
            amount: 10,
            nonce,
            network_id: net_id,
            timepoint,
        };

        assert!(!xor_req.uses_legacy_master_contract_path());
        assert!(val_req.uses_legacy_master_contract_path());
    });
}

#[test]
fn permanent_request_quarantine_blocks_approval_refund_and_execution_after_halt_removal() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR.into(), &alice, &alice, 100_000u32.into()).unwrap();
        let recipient = EthAddress::repeat_byte(7);
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            XOR.into(),
            recipient,
            100u32.into(),
            net_id
        ));
        let (request, hash) = last_outgoing_request(net_id).unwrap();
        let (_, account, seed) = &state.networks[&net_id].ocw_keypairs[0];
        let (public, signature) = approval_params(&request, hash, seed);
        sp_io::storage::set(&bridge_types::recovery::hashi_key(net_id, hash), &[1]);
        sp_io::storage::clear(crate::recovery::HALT_KEY);
        let before = sp_io::storage::root(sp_runtime::StateVersion::V1);
        assert_err!(request.cancel(), Error::RecoveryHalted);
        assert_err!(request.finalize(hash), Error::RecoveryHalted);
        assert_err!(
            EthBridge::approve_request(
                RuntimeOrigin::signed(account.clone()),
                public,
                hash,
                signature,
                net_id
            ),
            Error::RecoveryHalted
        );
        assert_err!(
            EthBridge::inner_abort_request(
                &OffchainRequest::outgoing(request),
                hash,
                Error::Forbidden.into(),
                net_id
            ),
            Error::RecoveryHalted
        );
        assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), before);
    });
}

#[test]
fn permanent_quarantine_blocks_referenced_incoming_work_and_legacy_aggregate_burn() {
    use crate::requests::{
        IncomingCancelOutgoingRequest, IncomingMetaRequestKind, IncomingRequest,
        LoadIncomingMetaRequest, LoadIncomingRequest,
    };
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let network = ETH_NETWORK_ID;
        let owner = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR.into(), &owner, &owner, 1_000u32.into()).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(owner.clone()),
            XOR.into(),
            EthAddress::repeat_byte(7),
            100u32.into(),
            network
        ));
        let (outgoing, id) = last_outgoing_request(network).unwrap();
        let incoming = IncomingRequest::CancelOutgoingRequest(IncomingCancelOutgoingRequest {
            outgoing_request: outgoing,
            outgoing_request_hash: id,
            initial_request_hash: H256::repeat_byte(20),
            tx_input: vec![],
            author: owner.clone(),
            tx_hash: H256::repeat_byte(21),
            at_height: 1,
            timepoint: Default::default(),
            network_id: network,
        });
        let request = OffchainRequest::incoming(incoming.clone());
        crate::Requests::<Runtime>::insert(network, request.hash(), &request);
        crate::RequestStatuses::<Runtime>::insert(network, request.hash(), RequestStatus::Pending);
        let load = LoadIncomingRequest::Meta(
            LoadIncomingMetaRequest::new(
                owner,
                id,
                Default::default(),
                IncomingMetaRequestKind::MarkAsDone,
                network,
            ),
            H256::repeat_byte(22),
        );
        sp_io::storage::set(&bridge_types::recovery::hashi_key(network, id), &[1]);
        sp_io::storage::set(common::recovery::POLICY_STORAGE_KEY, &[1; 32]);
        sp_io::storage::clear(crate::recovery::HALT_KEY);
        let before = sp_io::storage::root(sp_runtime::StateVersion::V1);
        assert_err!(
            EthBridge::finalize_incoming_request_inner(&incoming, request.hash(), network),
            Error::RecoveryHalted
        );
        assert_err!(
            EthBridge::inner_abort_request(
                &request,
                request.hash(),
                Error::Forbidden.into(),
                network
            ),
            Error::RecoveryHalted
        );
        assert_err!(load.validate(), Error::RecoveryHalted);
        assert!(
            crate::recovery::ensure_request_active::<Runtime>(&OffchainRequest::LoadIncoming(load))
                .is_err()
        );
        crate::migration::decommission_legacy_ethereum_xor::<Runtime>();
        assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), before);
    });
}

#[test]
fn current_peer_quorum_excludes_retired_approvers_and_accepts_more_than_threshold() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let net = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR.into(), &alice, &alice, 1000).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice),
            XOR.into(),
            EthAddress::repeat_byte(7),
            100,
            net
        ));
        let (request, hash) = last_outgoing_request(net).unwrap();
        let peers = &state.networks[&net].ocw_keypairs;
        for index in [0, 1] {
            let (_, account, seed) = &peers[index];
            let (public, signature) = approval_params(&request, hash, seed);
            assert_ok!(EthBridge::approve_request(
                RuntimeOrigin::signed(account.clone()),
                public,
                hash,
                signature,
                net
            ));
        }
        crate::Peers::<Runtime>::mutate(net, |set| {
            set.remove(&peers[0].1);
            set.insert(AccountId::new([248; 32]));
        });
        let (public, signature) = approval_params(&request, hash, &peers[2].2);
        assert_ok!(EthBridge::approve_request(
            RuntimeOrigin::signed(peers[2].1.clone()),
            public,
            hash,
            signature,
            net
        ));
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net, hash),
            Some(RequestStatus::Pending)
        );
        assert_eq!(crate::RequestApprovals::<Runtime>::get(net, hash).len(), 2);
        assert!(!crate::RequestApprovers::<Runtime>::get(net, hash).contains(&peers[0].1));
        let (public, signature) = approval_params(&request, hash, &peers[3].2);
        assert_ok!(EthBridge::approve_request(
            RuntimeOrigin::signed(peers[3].1.clone()),
            public,
            hash,
            signature,
            net
        ));
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net, hash),
            Some(RequestStatus::ApprovalsReady)
        );
    });
    // The pending-peer extra signature disappears between approvals. The next
    // valid vote makes four current approvals against a threshold of three.
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let net = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR.into(), &alice, &alice, 1000).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice),
            XOR.into(),
            EthAddress::repeat_byte(8),
            100,
            net
        ));
        let (request, hash) = last_outgoing_request(net).unwrap();
        crate::PendingPeer::<Runtime>::insert(net, AccountId::new([249; 32]));
        for (_, account, seed) in state.networks[&net].ocw_keypairs.iter().take(3) {
            let (public, signature) = approval_params(&request, hash, seed);
            assert_ok!(EthBridge::approve_request(
                RuntimeOrigin::signed(account.clone()),
                public,
                hash,
                signature,
                net
            ));
        }
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net, hash),
            Some(RequestStatus::Pending)
        );
        crate::PendingPeer::<Runtime>::remove(net);
        let (_, account, seed) = &state.networks[&net].ocw_keypairs[3];
        let (public, signature) = approval_params(&request, hash, seed);
        assert_ok!(EthBridge::approve_request(
            RuntimeOrigin::signed(account.clone()),
            public,
            hash,
            signature,
            net
        ));
        assert_eq!(crate::RequestApprovers::<Runtime>::get(net, hash).len(), 4);
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net, hash),
            Some(RequestStatus::ApprovalsReady)
        );
    });
}

#[test]
fn permissionless_recheck_finishes_existing_quorum_after_threshold_drop() {
    for resubmit_existing_peer in [false, true] {
        let (mut ext, state) = ExtBuilder::default().build();
        ext.execute_with(|| {
            let net = ETH_NETWORK_ID;
            let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
            Assets::mint_to(&XOR.into(), &alice, &alice, 1000).unwrap();
            assert_ok!(EthBridge::transfer_to_sidechain(
                RuntimeOrigin::signed(alice.clone()),
                XOR.into(),
                EthAddress::repeat_byte(9),
                100,
                net
            ));
            let (request, hash) = last_outgoing_request(net).unwrap();
            crate::PendingPeer::<Runtime>::insert(net, AccountId::new([249; 32]));
            for (_, account, seed) in state.networks[&net].ocw_keypairs.iter().take(3) {
                let (public, signature) = approval_params(&request, hash, seed);
                assert_ok!(EthBridge::approve_request(
                    RuntimeOrigin::signed(account.clone()),
                    public,
                    hash,
                    signature,
                    net
                ));
            }
            frame_support::assert_noop!(
                EthBridge::finalize_approved_request(
                    RuntimeOrigin::signed(alice.clone()),
                    net,
                    hash
                ),
                Error::InsufficientCurrentApprovals
            );
            crate::PendingPeer::<Runtime>::remove(net);
            if resubmit_existing_peer {
                let (_, account, seed) = &state.networks[&net].ocw_keypairs[0];
                let (public, signature) = approval_params(&request, hash, seed);
                assert_ok!(EthBridge::validate_outgoing_approval(
                    &public, &hash, &signature, account, net
                ));
                assert_ok!(EthBridge::approve_request(
                    RuntimeOrigin::signed(account.clone()),
                    public,
                    hash,
                    signature,
                    net
                ));
            } else {
                assert_ok!(EthBridge::finalize_approved_request(
                    RuntimeOrigin::signed(alice.clone()),
                    net,
                    hash
                ));
            }
            assert_eq!(
                crate::RequestStatuses::<Runtime>::get(net, hash),
                Some(RequestStatus::ApprovalsReady)
            );
            frame_support::assert_noop!(
                EthBridge::finalize_approved_request(RuntimeOrigin::signed(alice), net, hash),
                Error::RequestIsNotReady
            );
        });
    }
}

#[test]
fn public_outgoing_approvals_prevent_refund_and_failed_finalization_can_retry() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let net = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR.into(), &alice, &alice, 1000).unwrap();
        let original = Assets::total_balance(&XOR.into(), &alice).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            XOR.into(),
            EthAddress::repeat_byte(10),
            100,
            net
        ));
        let (request, hash) = last_outgoing_request(net).unwrap();
        let bridge = &state.networks[&net].config.bridge_account_id;
        let reserved_before = Assets::total_balance(&XOR.into(), bridge).unwrap()
            - Assets::free_balance(&XOR.into(), bridge).unwrap();
        let token =
            crate::RegisteredSidechainToken::<Runtime>::get(net, AssetId::from(XOR)).unwrap();
        // Deprecation after preparation makes local finalization fail while the
        // already signed Ethereum payload and the reserved balance stay valid.
        crate::DeprecatedSidechainTokens::<Runtime>::insert(net, token, true);
        for (index, (_, account, seed)) in
            state.networks[&net].ocw_keypairs.iter().take(3).enumerate()
        {
            let (public, signature) = approval_params(&request, hash, seed);
            assert_ok!(EthBridge::approve_request(
                RuntimeOrigin::signed(account.clone()),
                public,
                hash,
                signature,
                net
            ));
            if index == 0 {
                frame_support::assert_noop!(
                    EthBridge::abort_request(
                        RuntimeOrigin::signed(bridge.clone()),
                        hash,
                        Error::Other.into(),
                        net
                    ),
                    Error::PublicApprovalsRequireSettlement
                );
            }
        }
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net, hash),
            Some(RequestStatus::Broken(
                Error::DeprecatedLegacyXor.into(),
                Error::PublicApprovalsRequireSettlement.into()
            ))
        );
        assert_eq!(
            Assets::total_balance(&XOR.into(), &alice).unwrap(),
            original - 100
        );
        assert_eq!(
            Assets::total_balance(&XOR.into(), bridge).unwrap()
                - Assets::free_balance(&XOR.into(), bridge).unwrap(),
            reserved_before
        );
        assert_eq!(crate::RequestApprovals::<Runtime>::get(net, hash).len(), 3);
        let (_, peer, seed) = &state.networks[&net].ocw_keypairs[0];
        let (public, signature) = approval_params(&request, hash, seed);
        frame_support::assert_noop!(
            EthBridge::validate_outgoing_approval(&public, &hash, &signature, peer, net),
            Error::DuplicateApproval
        );
        frame_support::assert_noop!(
            EthBridge::approve_request(
                RuntimeOrigin::signed(peer.clone()),
                public,
                hash,
                signature,
                net
            ),
            Error::DuplicateApproval
        );
        frame_support::assert_noop!(
            EthBridge::abort_request(
                RuntimeOrigin::signed(bridge.clone()),
                hash,
                Error::Other.into(),
                net
            ),
            Error::PublicApprovalsRequireSettlement
        );
        crate::DeprecatedSidechainTokens::<Runtime>::remove(net, token);
        assert_ok!(EthBridge::finalize_approved_request(
            RuntimeOrigin::signed(alice.clone()),
            net,
            hash
        ));
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net, hash),
            Some(RequestStatus::ApprovalsReady)
        );
        assert_eq!(
            Assets::total_balance(&XOR.into(), &alice).unwrap(),
            original - 100
        );
        assert_eq!(
            Assets::total_balance(&XOR.into(), bridge).unwrap()
                - Assets::free_balance(&XOR.into(), bridge).unwrap(),
            reserved_before - 100
        );
    });
}

#[test]
fn zero_outgoing_transfer_is_rejected_before_nonce_queue_or_balance_changes() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let recipient = EthAddress::repeat_byte(42);
        for asset in [PSWAP, VAL] {
            let before = sp_io::storage::root(sp_runtime::StateVersion::V1);
            assert_err!(
                EthBridge::transfer_to_sidechain(
                    RuntimeOrigin::signed(alice.clone()),
                    asset,
                    recipient,
                    0,
                    ETH_NETWORK_ID,
                ),
                Error::InvalidAmount
            );
            assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), before);
            assert_err!(
                <EthBridge as bridge_types::traits::BridgeApp<
                    AccountId,
                    EthAddress,
                    AssetId,
                    common::Balance,
                >>::transfer(
                    bridge_types::GenericNetworkId::EVMLegacy(ETH_NETWORK_ID),
                    asset,
                    alice.clone(),
                    recipient,
                    0,
                ),
                Error::InvalidAmount
            );
            assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), before);
        }
    });
}
