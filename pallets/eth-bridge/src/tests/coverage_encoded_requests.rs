// SPDX-License-Identifier: BSD-4-Clause

use super::*;
use crate::tests::mock::{ExtBuilder, Runtime};

#[test]
fn every_abi_variant_preserves_its_request_hash_and_signature_timepoint() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let author = state.authority_account_id;
        let peer_address = EthAddress::repeat_byte(0xf0);
        let timepoint = bridge_multisig::BridgeTimepoint {
            height: bridge_multisig::MultiChainHeight::Thischain(105),
            index: 9,
        };
        let requests: Vec<OutgoingRequest<Runtime>> = vec![
            OutgoingTransfer {
                from: author.clone(),
                to: peer_address,
                asset_id: common::PSWAP,
                amount: 100,
                nonce: 1,
                network_id: 0,
                timepoint,
            }
            .into(),
            OutgoingAddAsset {
                author: author.clone(),
                asset_id: common::PSWAP,
                nonce: 2,
                network_id: 0,
                timepoint,
            }
            .into(),
            OutgoingAddToken {
                author: author.clone(),
                token_address: peer_address,
                symbol: "ABI".into(),
                name: "ABI Token".into(),
                decimals: 18,
                nonce: 3,
                network_id: 0,
                timepoint,
            }
            .into(),
            OutgoingAddPeer {
                author: author.clone(),
                peer_account_id: [0xf1; 32].into(),
                peer_address,
                nonce: 4,
                network_id: 0,
                timepoint,
            }
            .into(),
            OutgoingRemovePeer {
                author: author.clone(),
                peer_account_id: [0xf2; 32].into(),
                peer_address,
                nonce: 5,
                network_id: 0,
                timepoint,
                compat_hash: None,
            }
            .into(),
            OutgoingPrepareForMigration {
                author: author.clone(),
                nonce: 6,
                network_id: 0,
                timepoint,
            }
            .into(),
            OutgoingMigrate {
                author,
                new_contract_address: peer_address,
                erc20_native_tokens: vec![],
                nonce: 7,
                network_id: 0,
                timepoint,
                new_signature_version: crate::BridgeSignatureVersion::V3,
            }
            .into(),
        ];
        let mut hashes = std::collections::BTreeSet::new();
        for request in requests {
            assert_eq!(request.timepoint(), timepoint);
            let hash = OffchainRequest::outgoing(request.clone()).hash();
            assert!(hashes.insert(hash));
            let encoded = request.to_eth_abi(hash).unwrap();
            assert_eq!(encoded.hash(), hash);
            assert!(
                encoded
                    .as_raw()
                    .windows(32)
                    .any(|bytes| bytes == hash.as_bytes()),
                "the signed ABI payload must contain the same request identity"
            );
            if let OutgoingRequestEncoded::Transfer(transfer) = encoded {
                assert_eq!(transfer.input_tokens(None).len(), 5);
                assert_eq!(transfer.input_tokens(Some(vec![])).len(), 8);
            }
        }
    });
}
