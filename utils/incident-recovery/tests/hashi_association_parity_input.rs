//! Explicit offline test exporters. These never submit requests or invent finality.
#![cfg(feature = "runtime-work-types")]
#[path = "../../../runtime/tests/support/hashi_parser_parity_native.rs"]
mod native;
#[path = "../../../runtime/tests/support/hashi_parser_parity_wire.rs"]
#[allow(dead_code)]
mod wire;
use codec::{Decode, DecodeWithMemLimit, Encode};
use common::AssetInfoProvider;
use eth_bridge::requests::{
    AssetKind, IncomingTransactionRequestKind as Kind, LoadIncomingRequest,
    LoadIncomingTransactionRequest,
};
use framenode_runtime::{AccountId, Assets, BridgeMultisig, EthBridge, Runtime};
use serde_json::{json, Value};
use sp_core::{storage::StateVersion, H160, H256, U256};
use std::{
    collections::BTreeSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

fn absolute_env(name: &str) -> PathBuf {
    let path = PathBuf::from(std::env::var_os(name).expect(name));
    assert!(path.is_absolute());
    path
}
fn read(path: &Path, bound: usize) -> Vec<u8> {
    assert_eq!(path.canonicalize().unwrap(), path);
    assert!(fs::metadata(path).unwrap().len() <= bound as u64);
    let bytes = fs::read(path).unwrap();
    assert!(bytes.len() <= bound);
    bytes
}
fn blob(label: &str, bytes: Vec<u8>) -> wire::Blob {
    wire::Blob {
        label: label.into(),
        sha256: sp_io::hashing::sha2_256(&bytes),
        bytes,
    }
}
fn write_new(path: &Path, bytes: &[u8]) {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
}
fn evidence(capture: &Path, verified: &incident_recovery::VerifiedCheckpoint) -> Vec<wire::Blob> {
    assert!(verified.storage.children_default.is_empty());
    let capture_bytes = read(&capture.join("capture-complete.json"), 64 * 1024 * 1024);
    assert_eq!(
        wire::hex(&sp_io::hashing::sha2_256(&capture_bytes)),
        format!("0x{}", verified.state.capture_manifest_sha256)
    );
    vec![
        blob("capture-complete.json", capture_bytes),
        blob(
            "runtime-metadata.scale",
            read(&capture.join("runtime-metadata.scale"), 32 * 1024 * 1024),
        ),
        blob(
            "executed-metadata-witness.json",
            serde_json::to_vec(&verified.metadata_witness).unwrap(),
        ),
    ]
}
fn save(input: wire::Input, directory: &Path, extra: Value) {
    native::validate_input(&input);
    let bytes = input.encode();
    assert!(bytes.len() <= wire::MAX_INPUT);
    fs::create_dir(directory).expect("create-new output directory");
    write_new(&directory.join("input.scale"), &bytes);
    let report = json!({"schema":"sora-nexus-hashi-parser-parity-input-generation-v1",
        "inputSha256":wire::hex(&sp_io::hashing::sha2_256(&bytes)),"inputBytes":bytes.len(),
        "scope":format!("{:?}", input.scope),"checkpointStateRoot":wire::hex(&wire::CHECKPOINT_ROOT),
        "evidence":input.evidence.iter().map(|v|json!({"label":v.label,"sha256":wire::hex(&v.sha256),"bytes":v.bytes.len()})).collect::<Vec<_>>(),
        "caseLabels":input.cases.iter().map(|c|c.label.clone()).collect::<Vec<_>>(),"details":extra,
        "childStorageDropped":false,"nativeDeliveryAdmissionVerified":false,"nativeQuorumAuthenticated":false,
        "externalFinalityVerified":false,"sourceConfigurationEpochsAuthenticated":false,"allNativeObligationsCovered":false,
        "legacyParserExecutionEquivalenceVerified":false,"checkpointDeployedParserExecutionVerified":false,
        "economicExecutionPerformed":false,"bridgeHaltChanged":false,"productionReady":false});
    write_new(
        &directory.join("generation.json"),
        &serde_json::to_vec_pretty(&report).unwrap(),
    );
}

fn synthetic_case(
    label: String,
    network: u32,
    contract: H160,
    token: H160,
    raw_asset: [u8; 32],
    amount: U256,
    distinct_author: bool,
) -> wire::Case {
    let beneficiary = AccountId::new(sp_io::hashing::blake2_256(
        b"SORA Nexus PUBLIC PARSER PARITY beneficiary",
    ));
    let author = if distinct_author {
        AccountId::new(sp_io::hashing::blake2_256(
            b"SORA Nexus PUBLIC PARSER PARITY distinct author",
        ))
    } else {
        beneficiary.clone()
    };
    let tx_hash = H256(sp_io::hashing::keccak_256(label.as_bytes()));
    let height = 123u64;
    let load = LoadIncomingRequest::Transaction(LoadIncomingTransactionRequest::<Runtime>::new(
        author,
        tx_hash,
        BridgeMultisig::sidechain_timepoint(height, 7),
        Kind::Transfer,
        network,
    ));
    let mut data = beneficiary.encode();
    data.extend(amount.to_big_endian());
    data.extend([0; 12]);
    data.extend(token.as_bytes());
    data.extend(raw_asset);
    wire::Case {
        label,
        network,
        height,
        load_scale: load.encode(),
        logs: vec![wire::Log {
            address: contract.0,
            topics: vec![eth_bridge::DEPOSIT_TOPIC.0],
            data,
            block_hash: sp_io::hashing::keccak_256(
                b"SORA Nexus PUBLIC PARSER PARITY synthetic external block",
            ),
            transaction_hash: tx_hash.0,
            transaction_index: 7,
            global_index: 4,
            receipt_index: 0,
        }],
        expected: wire::Outcome::UnsupportedLoadKind,
    }
}

#[test]
#[ignore = "requires root-authenticated checkpoint capture and create-new output directory"]
fn export_synthetic_checkpoint_hashi_parser_input() {
    let capture = absolute_env("SORA_RECOVERY_HASHI_PARITY_CAPTURE");
    let output = absolute_env("SORA_RECOVERY_HASHI_PARITY_INPUT_DIRECTORY");
    let verified = incident_recovery::verify_checkpoint(&capture).unwrap();
    let evidence = evidence(&capture, &verified);
    let code = verified.storage.top.get(b":code".as_slice()).unwrap();
    let mut ext = sp_io::TestExternalities::new_with_code_and_state(
        code,
        verified.storage.clone(),
        StateVersion::V0,
    );
    let (cases, configurations) = ext.execute_with(|| {
        let before = sp_io::storage::root(StateVersion::V0);
        assert_eq!(before, wire::CHECKPOINT_ROOT);
        let network = framenode_runtime::GetEthNetworkId::get();
        let contract = EthBridge::bridge_contract_address(network);
        let mut cases = Vec::new(); let mut configurations = Vec::new();
        for (name, asset) in [("DAI", common::DAI), ("XOR", common::XOR), ("VAL", common::VAL)] {
            let Some(kind) = EthBridge::registered_asset(network, asset) else {
                configurations.push(json!({"asset":name,"registered":false,"covered":false})); continue;
            };
            let token = EthBridge::registered_sidechain_token(network, asset);
            if kind != AssetKind::Thischain && token.is_none() {
                configurations.push(json!({"asset":name,"registered":true,"tokenMissing":true,"covered":false})); continue;
            }
            let token = token.unwrap_or_default();
            let raw_asset = if kind != AssetKind::Thischain { [0;32] } else { asset.encode().try_into().unwrap() };
            let factor = U256::from(EthBridge::bridge_denomination_factor(network, &asset));
            let mut amounts = BTreeSet::from([U256::zero(), U256::one(), factor, U256::from(u128::MAX), U256::MAX]);
            if factor > U256::zero() { amounts.insert(factor-1); }
            if let Some(n) = factor.checked_add(U256::one()) { amounts.insert(n); }
            if let Some(n) = factor.checked_mul(U256::from(u128::MAX)) {
                amounts.insert(n); if let Some(next) = n.checked_add(U256::one()) { amounts.insert(next); }
            }
            let local = Assets::get_asset_info(&asset).2;
            let side = EthBridge::sidechain_asset_precision(network, asset);
            if side > local {
                if let Some(step) = U256::from(10).checked_pow(U256::from(side-local)).and_then(|n| n.checked_mul(factor)) {
                    amounts.insert(step); if step > U256::zero() { amounts.insert(step-1); }
                    if let Some(next) = step.checked_add(U256::one()) { amounts.insert(next); }
                }
            }
            configurations.push(json!({"asset":name,"registered":true,"covered":true,"assetKind":format!("{kind:?}"),"token":wire::hex(token.as_bytes()),"factor":factor.to_string(),"localPrecision":local,"sidechainPrecision":side,"precisionDownObserved":side>local}));
            for (index, amount) in amounts.into_iter().enumerate() {
                let mut case = synthetic_case(format!("synthetic-{name}-{index}-author-independent"), network, contract, token, raw_asset, amount, true);
                case.expected = native::observe(&case); cases.push(case);
            }
            let mut automatic = synthetic_case(format!("synthetic-{name}-automatic-author-recipe"), network, contract, token, raw_asset, factor, false);
            automatic.expected = native::observe(&automatic); cases.push(automatic);
        }
        assert!(cases.iter().any(|c|matches!(c.expected,wire::Outcome::Accepted(_))),"actual checkpoint supplies no accepted representative Deposit");
        let positive = cases.iter().find(|c|matches!(c.expected,wire::Outcome::Accepted(_))).unwrap().clone();
        let mut unknown = positive.clone(); unknown.label = "synthetic-unknown-contract".into();
        let address = H160([0x5e; 20]);
        assert!(EthBridge::ensure_known_contract(address, network).is_err());
        unknown.logs[0].address=address.0; unknown.expected=native::observe(&unknown); assert!(matches!(unknown.expected,wire::Outcome::EventError(_))); cases.push(unknown);
        let mut unsupported = positive.clone();
        unsupported.label = "synthetic-transfer-xor-load-quarantine".into();
        let load = LoadIncomingRequest::<Runtime>::decode(&mut unsupported.load_scale.as_slice()).unwrap();
        assert!(matches!(load, LoadIncomingRequest::Transaction(_)));
        unsupported.load_scale = LoadIncomingRequest::Transaction(LoadIncomingTransactionRequest::<Runtime>::new(
            load.author().clone(), load.hash(), load.timepoint(), Kind::TransferXOR, load.network_id(),
        )).encode();
        unsupported.expected = native::observe(&unsupported);
        assert_eq!(unsupported.expected, wire::Outcome::UnsupportedLoadKind);
        cases.push(unsupported);
        let mut malformed=positive;malformed.label="synthetic-truncated-deposit-data".into();malformed.logs[0].data.pop();malformed.expected=native::observe(&malformed);assert!(matches!(malformed.expected,wire::Outcome::EventError(_)));cases.push(malformed);
        assert_eq!(sp_io::storage::root(StateVersion::V0),before);
        (cases,configurations)
    });
    save(
        wire::Input {
            tag: *b"snxpar01",
            scope: wire::Scope::SyntheticCheckpoint,
            top: verified.storage.top.into_iter().collect(),
            evidence,
            cases,
        },
        &output,
        json!({"sourceEventsInvented":true,"expectedOutcomesObservedFromCurrentNativeParser":true,"noRealTransactionOrReceiptClaim":true,"configurationOverlays":[],"configurations":configurations,"authorTimepointsSynthetic":true}),
    );
}

#[test]
#[ignore = "requires a genuinely accepted association manifest; never synthesize its admission receipt"]
fn export_associated_hashi_parser_input() {
    let capture = absolute_env("SORA_RECOVERY_HASHI_PARITY_CAPTURE");
    let manifest_path = absolute_env("SORA_RECOVERY_HASHI_ASSOCIATION_MANIFEST");
    let output = absolute_env("SORA_RECOVERY_HASHI_PARITY_INPUT_DIRECTORY");
    let manifest_bytes = read(&manifest_path, 64 * 1024 * 1024);
    let report =
        incident_recovery::hashi_association::verify_file(&capture, &manifest_path).unwrap();
    assert_eq!(
        report["manifestSha256"].as_str().unwrap(),
        &wire::hex(&sp_io::hashing::sha2_256(&manifest_bytes))[2..]
    );
    let manifest: Value = serde_json::from_slice(&manifest_bytes).unwrap();
    let verified = incident_recovery::verify_checkpoint(&capture).unwrap();
    let mut evidence = evidence(&capture, &verified);
    evidence.push(blob("association-manifest.json", manifest_bytes.clone()));
    evidence.push(blob(
        "association-report.json",
        serde_json::to_vec(&report).unwrap(),
    ));
    let base = manifest_path.parent().unwrap();
    let rows = manifest["rows"].as_array().unwrap();
    let associations = report["associations"].as_array().unwrap();
    assert_eq!(rows.len(), associations.len());
    let mut cases = Vec::new();
    let mut included = BTreeSet::new();
    for reference in report["inputs"].as_array().unwrap() {
        let name = reference["file"].as_str().unwrap();
        let path = base.join(name);
        let bytes = read(&path, 96 * 1024 * 1024);
        assert!(path.starts_with(base));
        assert_eq!(bytes.len() as u64, reference["bytes"].as_u64().unwrap());
        assert_eq!(
            &wire::hex(&sp_io::hashing::sha2_256(&bytes))[2..],
            reference["sha256"].as_str().unwrap()
        );
        if included.insert(name.to_owned()) {
            evidence.push(blob(&format!("bound:{name}"), bytes));
        }
    }
    for (index, (row, association)) in rows.iter().zip(associations).enumerate() {
        let source = evidence
            .iter()
            .find(|b| b.label == format!("bound:{}", row["sourceReport"]["file"].as_str().unwrap()))
            .unwrap();
        let source: Value = serde_json::from_slice(&source.bytes).unwrap();
        let tx = row["transactionHash"].as_str().unwrap();
        let logs = source["range"]["logs"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|l| l["transactionHash"].as_str() == Some(tx))
            .map(|l| wire::Log {
                address: wire::unhex(l["address"].as_str().unwrap())
                    .try_into()
                    .unwrap(),
                topics: l["topics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| wire::unhex(v.as_str().unwrap()).try_into().unwrap())
                    .collect(),
                data: wire::unhex(l["data"].as_str().unwrap()),
                block_hash: wire::unhex(l["blockHash"].as_str().unwrap())
                    .try_into()
                    .unwrap(),
                transaction_hash: wire::unhex(tx).try_into().unwrap(),
                transaction_index: u32::try_from(l["transactionIndex"].as_u64().unwrap()).unwrap(),
                global_index: u32::try_from(l["logIndex"].as_u64().unwrap()).unwrap(),
                receipt_index: u32::try_from(l["receiptLogIndex"].as_u64().unwrap()).unwrap(),
            })
            .collect();
        cases.push(wire::Case {
            label: format!("associated-{index}-{tx}"),
            network: u32::try_from(row["network"].as_u64().unwrap()).unwrap(),
            height: association["sourceHeight"]
                .as_str()
                .unwrap()
                .parse()
                .unwrap(),
            load_scale: wire::unhex(row["loadScale"].as_str().unwrap()),
            logs,
            expected: wire::Outcome::Accepted(wire::unhex(
                association["originalRequestScale"].as_str().unwrap(),
            )),
        });
    }
    assert_eq!(read(&manifest_path, 64 * 1024 * 1024), manifest_bytes);
    save(
        wire::Input {
            tag: *b"snxpar01",
            scope: wire::Scope::AssociatedRows,
            top: verified.storage.top.into_iter().collect(),
            evidence,
            cases,
        },
        &output,
        json!({"associationAdapterActuallyRan":true,"sourceReportCryptographyReverifiedByThisExporter":false,"originalExpectedBytesFromCommittedRequests":true}),
    );
}

#[test]
#[ignore = "requires two separately executed, pinned current and legacy native outputs"]
fn compare_hashi_parser_parity_outputs() {
    let current = read(
        &absolute_env("SORA_RECOVERY_HASHI_PARITY_CURRENT_OUTPUT"),
        8 * 1024 * 1024,
    );
    let legacy = read(
        &absolute_env("SORA_RECOVERY_HASHI_PARITY_LEGACY_OUTPUT"),
        8 * 1024 * 1024,
    );
    let decode = |bytes: &[u8]| {
        let mut cursor = bytes;
        let output = wire::Output::decode_with_mem_limit(&mut cursor, 32 * 1024 * 1024).unwrap();
        assert!(cursor.is_empty() && output.encode() == bytes);
        output
    };
    let a = decode(&current);
    let b = decode(&legacy);
    validate_pair(&a, &b);
    let report = json!({"schema":"sora-nexus-hashi-native-parser-parity-comparison-v1","scope":format!("{:?}",a.scope),"inputBlake2":wire::hex(&a.input_hash),"currentOutputSha256":wire::hex(&sp_io::hashing::sha2_256(&current)),"legacyOutputSha256":wire::hex(&sp_io::hashing::sha2_256(&legacy)),"comparedCases":a.outcomes.len(),"encodedOutputsAndErrorCategoriesIdentical":true,"checkpointRootUnchanged":true,"externalProcessAndExecutablePinsVerifiedByThisComparison":false,"deployedCheckpointWasmParityVerified":false,"externalFinalityVerified":false,"nativeDeliveryAdmissionVerified":false,"productionReady":false});
    write_new(
        &absolute_env("SORA_RECOVERY_HASHI_PARITY_COMPARISON_OUTPUT"),
        &serde_json::to_vec_pretty(&report).unwrap(),
    );
}

fn validate_pair(a: &wire::Output, b: &wire::Output) {
    assert_eq!(&a.tag, b"snxparo1");
    assert_eq!(a.tag, b.tag);
    assert_eq!(a.input_hash, b.input_hash);
    assert_eq!(a.scope, b.scope);
    assert_eq!((a.spec_version, a.transaction_version), (138, 134));
    assert_eq!((b.spec_version, b.transaction_version), (134, 131));
    assert_eq!(a.state_root_before, wire::CHECKPOINT_ROOT);
    assert_eq!(a.state_root_before, a.state_root_after);
    assert_eq!(a.state_root_before, b.state_root_before);
    assert_eq!(b.state_root_before, b.state_root_after);
    assert!(!a.outcomes.is_empty());
    assert_eq!(a.outcomes, b.outcomes);
}

#[test]
fn parity_comparison_rejects_wrong_version_input_root_and_outcome() {
    let output = |legacy| wire::Output {
        tag: *b"snxparo1",
        input_hash: [1; 32],
        scope: wire::Scope::SyntheticCheckpoint,
        spec_version: if legacy { 134 } else { 138 },
        transaction_version: if legacy { 131 } else { 134 },
        state_root_before: wire::CHECKPOINT_ROOT.to_vec(),
        state_root_after: wire::CHECKPOINT_ROOT.to_vec(),
        outcomes: vec![wire::Outcome::Accepted(vec![1, 2, 3])],
    };
    validate_pair(&output(false), &output(true));
    for mutation in 0..5 {
        let mut bad = output(true);
        match mutation {
            0 => bad.spec_version = 138,
            1 => bad.input_hash[0] ^= 1,
            2 => bad.state_root_after[0] ^= 1,
            3 => bad.outcomes[0] = wire::Outcome::Accepted(vec![1, 2, 4]),
            4 => bad.scope = wire::Scope::AssociatedRows,
            _ => unreachable!(),
        }
        assert!(std::panic::catch_unwind(|| validate_pair(&output(false), &bad)).is_err());
    }
}
