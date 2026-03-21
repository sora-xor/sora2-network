use crate::EthereumBeaconClient;
use codec::{Decode, Encode};
use snowbridge_beacon_primitives::{
    merkle_proof::{generalized_index_length, subtree_index},
    verify_merkle_branch, ExecutionProof,
};
use sp_core::H256;

fn verify_execution_proof(execution_proof: &ExecutionProof) -> bool {
    let Some(latest_finalized_state) = EthereumBeaconClient::finalized_beacon_state(
        EthereumBeaconClient::latest_finalized_block_root(),
    ) else {
        return false;
    };

    if execution_proof.header.slot > latest_finalized_state.slot {
        return false;
    }

    let Ok(beacon_block_root) = execution_proof.header.hash_tree_root() else {
        return false;
    };

    match &execution_proof.ancestry_proof {
        Some(proof) => {
            let Some(state) =
                EthereumBeaconClient::finalized_beacon_state(proof.finalized_block_root)
            else {
                return false;
            };
            if execution_proof.header.slot >= state.slot {
                return false;
            }

            let slots_per_historical_root =
                snowbridge_pallet_ethereum_client::config::SLOTS_PER_HISTORICAL_ROOT as u64;
            let index_in_array = execution_proof.header.slot % slots_per_historical_root;
            let leaf_index = slots_per_historical_root + index_in_array;

            if !verify_merkle_branch(
                beacon_block_root,
                &proof.header_branch,
                leaf_index as usize,
                snowbridge_pallet_ethereum_client::config::BLOCK_ROOT_AT_INDEX_DEPTH,
                state.block_roots_root,
            ) {
                return false;
            }
        }
        None => {
            let Some(state) = EthereumBeaconClient::finalized_beacon_state(beacon_block_root)
            else {
                return false;
            };
            if execution_proof.header.slot != state.slot {
                return false;
            }
        }
    }

    let Ok(execution_header_root) = execution_proof.execution_header.hash_tree_root() else {
        return false;
    };

    let execution_header_gindex = EthereumBeaconClient::execution_header_gindex();
    verify_merkle_branch(
        execution_header_root,
        &execution_proof.execution_branch,
        subtree_index(execution_header_gindex),
        generalized_index_length(execution_header_gindex),
        execution_proof.header.body_root,
    )
}

fn abi_word_to_usize(word: &[u8]) -> Option<usize> {
    if word.len() != 32 {
        return None;
    }

    let width = core::mem::size_of::<usize>();
    if word[..32 - width].iter().any(|&b| b != 0) {
        return None;
    }

    let mut out = [0u8; core::mem::size_of::<usize>()];
    out.copy_from_slice(&word[32 - width..]);
    Some(usize::from_be_bytes(out))
}

fn decode_sccp_burn_log_payload(data: &[u8]) -> Option<&[u8]> {
    const HEAD_LEN: usize = 5 * 32;

    if data.len() < HEAD_LEN + 32 {
        return None;
    }

    let payload_offset = abi_word_to_usize(&data[4 * 32..5 * 32])?;
    if payload_offset != HEAD_LEN || payload_offset + 32 > data.len() {
        return None;
    }

    let payload_len = abi_word_to_usize(&data[payload_offset..payload_offset + 32])?;
    let start = payload_offset + 32;
    let end = start.checked_add(payload_len)?;
    if end > data.len() {
        return None;
    }

    Some(&data[start..end])
}

fn receipt_contains_sccp_burn(
    receipt: &snowbridge_ethereum::ReceiptEnvelope,
    message_id: H256,
    payload: &sccp::BurnPayloadV1,
    router_address: [u8; 20],
) -> bool {
    let expected_payload = payload.encode();

    receipt.logs().iter().any(|log| {
        if log.address.0 != router_address {
            return false;
        }

        let topics = log.topics();
        if topics.len() != 4 {
            return false;
        }
        if H256(topics[0].0) != sccp::SCCP_ETH_BURN_EVENT_TOPIC0 {
            return false;
        }
        if H256(topics[1].0) != message_id {
            return false;
        }
        if topics[2].0 != payload.sora_asset_id {
            return false;
        }

        let Some(log_payload) = decode_sccp_burn_log_payload(log.data.data.0.as_ref()) else {
            return false;
        };
        log_payload == expected_payload.as_slice()
    })
}

pub(crate) fn verify_finalized_burn_proof_v1(
    message_id: H256,
    payload: &sccp::BurnPayloadV1,
    router_address: [u8; 20],
    proof: &sccp::EthFinalizedBurnProofV1,
) -> bool {
    let mut input = proof.execution_proof.as_slice();
    let Ok(execution_proof) = ExecutionProof::decode(&mut input) else {
        return false;
    };
    if !input.is_empty() || !verify_execution_proof(&execution_proof) {
        return false;
    }

    let Some(receipt) = snowbridge_beacon_primitives::verify_receipt_proof(
        execution_proof.execution_header.receipts_root(),
        &proof.receipt_proof,
    ) else {
        return false;
    };

    receipt_contains_sccp_burn(&receipt, message_id, payload, router_address)
}

#[cfg(test)]
mod tests {
    use super::*;
    use frame_support::assert_ok;
    use framenode_chain_spec::ext;
    use snowbridge_pallet_ethereum_client::mock::{
        get_message_verification_payload, initialize_storage, load_execution_proof_fixture,
    };

    #[test]
    fn snowbridge_inbound_fixture_verifies_real_execution_and_receipt_proofs_in_runtime() {
        let mut ext = ext();

        ext.execute_with(|| {
            assert_ok!(initialize_storage());

            let execution_proof = load_execution_proof_fixture();
            let (event_log, proof) = get_message_verification_payload();

            assert_eq!(
                proof.execution_proof.execution_header.block_hash(),
                execution_proof.execution_header.block_hash(),
            );
            assert!(verify_execution_proof(&execution_proof));

            let receipt = snowbridge_beacon_primitives::verify_receipt_proof(
                execution_proof.execution_header.receipts_root(),
                &proof.receipt_proof,
            )
            .expect("Snowbridge receipt proof fixture must decode");

            assert!(crate::EthereumBeaconClient::verify_receipt_inclusion(
                execution_proof.execution_header.receipts_root(),
                &proof.receipt_proof,
                &event_log,
            )
            .is_ok());

            let payload = sccp::BurnPayloadV1 {
                version: 1,
                source_domain: sccp::SCCP_DOMAIN_ETH,
                dest_domain: sccp::SCCP_DOMAIN_SORA,
                nonce: 1,
                sora_asset_id: [0x11; 32],
                amount: 1,
                recipient: [0x22; 32],
            };

            assert!(!receipt_contains_sccp_burn(
                &receipt,
                H256::zero(),
                &payload,
                [0u8; 20],
            ));
        });
    }
}
