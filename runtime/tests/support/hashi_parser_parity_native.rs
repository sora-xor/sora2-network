//! Shared native integration-test parser calls. No hooks or delivery admission.
use super::wire::{self, Case, Outcome};
use codec::{Decode, DecodeLimit, Encode};
use eth_bridge::requests::{
    IncomingRequest, IncomingTransactionRequestKind, LoadIncomingRequest,
    LoadIncomingTransactionRequest,
};
use framenode_runtime::{EthBridge, Runtime};
use sp_core::{storage::StateVersion, H160};

pub fn observe(case: &Case) -> Outcome {
    assert!(!case.load_scale.is_empty() && case.load_scale.len() <= 2048);
    assert!(!case.logs.is_empty() && case.logs.len() <= 4096);
    let root = sp_io::storage::root(StateVersion::V0);
    let mut encoded = case.load_scale.as_slice();
    let load = {
        let mut tracked = codec::MemTrackingInput::new(&mut encoded, 8192);
        LoadIncomingRequest::<Runtime>::decode_with_depth_limit(16, &mut tracked).unwrap()
    };
    assert!(encoded.is_empty() && load.encode() == case.load_scale);
    let transfer_load = LoadIncomingTransactionRequest::new(
        load.author().clone(),
        load.hash(),
        load.timepoint(),
        IncomingTransactionRequestKind::Transfer,
        load.network_id(),
    );
    let result = match &load {
        LoadIncomingRequest::Transaction(_)
            if load == LoadIncomingRequest::Transaction(transfer_load.clone()) =>
        {
            assert_eq!(load.network_id(), case.network);
            let logs = case
                .logs
                .iter()
                .map(|log| {
                    assert!(log.topics.len() <= 4 && log.data.len() <= 1024 * 1024);
                    assert_eq!(log.transaction_hash, load.hash().0);
                    eth_bridge::types::Log {
                        address: log.address.into(),
                        topics: log.topics.iter().map(|v| (*v).into()).collect(),
                        data: log.data.clone().into(),
                        block_hash: Some(log.block_hash.into()),
                        block_number: Some(case.height.into()),
                        transaction_hash: Some(log.transaction_hash.into()),
                        transaction_index: Some(log.transaction_index.into()),
                        log_index: Some(log.global_index.into()),
                        transaction_log_index: Some(log.receipt_index.into()),
                        removed: Some(false),
                        log_type: None,
                    }
                })
                .collect::<Vec<_>>();
            match EthBridge::parse_main_event(
                case.network,
                &logs,
                IncomingTransactionRequestKind::Transfer,
            ) {
                Err(error) => Outcome::EventError(format!("{error:?}")),
                Ok(event) => {
                    match IncomingRequest::<Runtime>::try_from_contract_event(
                        event,
                        transfer_load,
                        case.height,
                    ) {
                        Err(error) => Outcome::RequestError(format!("{error:?}")),
                        Ok(incoming) => {
                            let IncomingRequest::Transfer(transfer) = &incoming else {
                                panic!("non-Transfer Deposit result")
                            };
                            assert_eq!(transfer.from, H160::zero());
                            let matching = case
                                .logs
                                .iter()
                                .filter(|log| {
                                    log.topics.first() == Some(&eth_bridge::DEPOSIT_TOPIC.0)
                                        && EthBridge::ensure_known_contract(
                                            H160(log.address),
                                            case.network,
                                        )
                                        .is_ok()
                                })
                                .collect::<Vec<_>>();
                            assert_eq!(matching.len(), 1);
                            assert_eq!(matching[0].data.len(), 128);
                            let beneficiary_bytes: &[u8] = transfer.to.as_ref();
                            assert_eq!(beneficiary_bytes, &matching[0].data[..32]);
                            assert!(!transfer.should_take_fee);
                            assert_eq!(&transfer.author, load.author());
                            assert_eq!(transfer.timepoint, load.timepoint());
                            assert_eq!(transfer.tx_hash, load.hash());
                            assert_eq!(transfer.network_id, case.network);
                            assert_eq!(transfer.at_height, case.height);
                            let bytes = incoming.encode();
                            assert_eq!(
                                IncomingRequest::<Runtime>::decode(&mut bytes.as_slice()).unwrap(),
                                incoming
                            );
                            Outcome::Accepted(bytes)
                        }
                    }
                }
            }
        }
        _ => Outcome::UnsupportedLoadKind,
    };
    assert_eq!(
        sp_io::storage::root(StateVersion::V0),
        root,
        "parser wrote storage"
    );
    result
}

pub fn validate_input(input: &wire::Input) {
    assert_eq!(&input.tag, b"snxpar01");
    assert!(!input.top.is_empty() && input.top.len() <= 4_000_000);
    assert!(!input.cases.is_empty() && input.cases.len() <= wire::MAX_CASES);
    assert!(input.top.windows(2).all(|pair| pair[0].0 < pair[1].0));
    assert!(input.top.iter().all(|(key, value)| !key.is_empty()
        && key.len() <= 4096
        && value.len() <= 16 * 1024 * 1024
        && !key.starts_with(b":child_storage:")));
    assert!((3..=136).contains(&input.evidence.len()));
    let mut labels = std::collections::BTreeSet::new();
    for blob in &input.evidence {
        assert!(!blob.label.is_empty() && blob.label.len() <= 256 && labels.insert(&blob.label));
        assert!(blob.bytes.len() <= 96 * 1024 * 1024);
        assert_eq!(sp_io::hashing::sha2_256(&blob.bytes), blob.sha256);
    }
    let mut cases = std::collections::BTreeSet::new();
    for case in &input.cases {
        assert!(!case.label.is_empty() && case.label.len() <= 256 && cases.insert(&case.label));
    }
    if input.scope == wire::Scope::AssociatedRows {
        validate_associated_rows(input);
    }
}

// A parity result must continue to describe the supplied committed row bytes,
// even if an input file is constructed without the companion generator.
fn validate_associated_rows(input: &wire::Input) {
    use framenode_runtime::incident_recovery_incoming::{IncomingChannel, IncomingOperation};
    let evidence = |label: &str| {
        input
            .evidence
            .iter()
            .find(|blob| blob.label == label)
            .expect("bound evidence")
    };
    let manifest: serde_json::Value =
        serde_json::from_slice(&evidence("association-manifest.json").bytes).unwrap();
    let rows = manifest["rows"].as_array().unwrap();
    assert_eq!(rows.len(), input.cases.len());
    for (row, case) in rows.iter().zip(&input.cases) {
        assert_eq!(row["network"].as_u64(), Some(u64::from(case.network)));
        assert_eq!(
            wire::unhex(row["loadScale"].as_str().unwrap()),
            case.load_scale
        );
        let operation_bytes = wire::unhex(row["incomingOperationScale"].as_str().unwrap());
        assert!(operation_bytes.len() <= 512 * 1024);
        let mut cursor = operation_bytes.as_slice();
        let mut tracked = codec::MemTrackingInput::new(&mut cursor, 2 * 1024 * 1024);
        let operation = IncomingOperation::decode_with_depth_limit(16, &mut tracked).unwrap();
        assert!(cursor.is_empty() && operation.encode() == operation_bytes);
        let IncomingChannel::HashiAuthenticatedSource {
            network, request, ..
        } = operation.channel
        else {
            panic!("source-proof HASHI row required")
        };
        assert_eq!(network, case.network);
        assert_eq!(case.expected, Outcome::Accepted(request));
        let reference = &row["sourceReport"];
        let source = evidence(&format!("bound:{}", reference["file"].as_str().unwrap()));
        assert_eq!(
            &wire::hex(&source.sha256)[2..],
            reference["sha256"].as_str().unwrap()
        );
        assert_eq!(
            source.bytes.len() as u64,
            reference["bytes"].as_u64().unwrap()
        );
        let source: serde_json::Value = serde_json::from_slice(&source.bytes).unwrap();
        let tx = row["transactionHash"].as_str().unwrap();
        let logs = source["range"]["logs"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|log| log["transactionHash"].as_str() == Some(tx))
            .collect::<Vec<_>>();
        assert_eq!(logs.len(), case.logs.len());
        for (log, original) in case.logs.iter().zip(logs) {
            assert_eq!(
                wire::hex(&log.address),
                original["address"].as_str().unwrap()
            );
            assert_eq!(
                log.topics
                    .iter()
                    .map(|topic| wire::hex(topic))
                    .collect::<Vec<_>>(),
                original["topics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|topic| topic.as_str().unwrap().to_owned())
                    .collect::<Vec<_>>()
            );
            assert_eq!(wire::hex(&log.data), original["data"].as_str().unwrap());
            assert_eq!(
                wire::hex(&log.block_hash),
                original["blockHash"].as_str().unwrap()
            );
            assert_eq!(wire::hex(&log.transaction_hash), tx);
            assert_eq!(
                case.height.to_string(),
                original["blockNumber"].as_str().unwrap()
            );
            assert_eq!(
                u64::from(log.transaction_index),
                original["transactionIndex"].as_u64().unwrap()
            );
            assert_eq!(
                u64::from(log.global_index),
                original["logIndex"].as_u64().unwrap()
            );
            assert_eq!(
                u64::from(log.receipt_index),
                original["receiptLogIndex"].as_u64().unwrap()
            );
            assert_eq!(original["status"].as_u64(), Some(1));
        }
    }
}
