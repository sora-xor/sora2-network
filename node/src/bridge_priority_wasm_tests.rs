//! Offline acceptance test using the published spec 142 Wasm, a real confirmed
//! Deposit receipt and proven asset/contract storage. Peers, signing keys, pool
//! and remaining chain state belong to the disposable mainnet test fixture.

use codec::{DecodeAll, Encode};
use common::recovery;
use framenode_runtime::{eth_bridge, Header, RuntimeCall, UncheckedExtrinsic};
use serde_json::{json, Value};
use sp_core::{
    offchain::{
        testing::{
            PendingRequest, TestOffchainExt, TestPersistentOffchainDB, TestTransactionPoolExt,
        },
        OffchainStorage, OffchainWorkerExt, TransactionPoolExt,
    },
    Bytes, H256,
};
use sp_keystore::{testing::MemoryKeystore, Keystore, KeystoreExt};
use sp_runtime::{offchain::STORAGE_PREFIX, traits::Header as _};
use std::{collections::BTreeSet, str::FromStr, sync::Arc};

fn recorded_bytes(value: &Value) -> Vec<u8> {
    serde_json::from_value::<Bytes>(value.clone()).unwrap().0
}

fn response(url: &str, method: &str, params: Value, result: Value) -> PendingRequest {
    let body = format!(r#"{{"jsonrpc":"2.0","method":"{method}","params":{params},"id":0}}"#);
    PendingRequest {
        method: "POST".to_owned(),
        uri: url.to_owned(),
        body: body.into_bytes(),
        headers: vec![("content-type".into(), "application/json".into())],
        sent: true,
        response: Some(
            serde_json::to_vec(&json!({"jsonrpc":"2.0", "id":0, "result":result})).unwrap(),
        ),
        ..Default::default()
    }
}

#[test]
#[ignore = "requires a pinned public receipt/storage fixture and published spec 142 Wasm"]
fn spec142_priority_pass_checks_the_receipt_and_submits_a_canonical_import_only_to_the_test_pool() {
    let _ = sc_cli::LoggerBuilder::new("").init();
    for history_offset in [-1_000, 1_000] {
        check_canonical_import(history_offset);
    }
}

fn check_canonical_import(history_offset: i64) {
    let path =
        std::env::var("SORA_HASHI_PRIORITY_FIXTURE").expect("path to reviewed offline fixture");
    let data = std::fs::read(path).unwrap();
    let pin = H256::from_str(
        &std::env::var("SORA_HASHI_PRIORITY_FIXTURE_SHA256")
            .expect("independently checked fixture SHA-256"),
    )
    .unwrap();
    assert_eq!(H256(sp_io::hashing::sha2_256(&data)), pin);
    let fixture: Value = serde_json::from_slice(&data).unwrap();
    let recorded_header: Header = serde_json::from_value(fixture["header"].clone()).unwrap();
    assert_eq!(
        recorded_header.hash(),
        serde_json::from_value::<H256>(fixture["nativeHash"].clone()).unwrap()
    );
    let keys: Vec<_> = fixture["storage"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| recorded_bytes(&row["key"]))
        .collect();
    let nodes: BTreeSet<_> = fixture["proof"]
        .as_array()
        .unwrap()
        .iter()
        .map(recorded_bytes)
        .collect();
    let values = sp_state_machine::read_proof_check::<sp_core::Blake2Hasher, _>(
        *recorded_header.state_root(),
        DecodeAll::decode_all(&mut &nodes.encode()[..]).unwrap(),
        &keys,
    )
    .unwrap();
    let wasm = std::fs::read(std::env::var("SORA_OCW_ONLY_WASM").unwrap()).unwrap();
    assert_eq!(
        H256(sp_io::hashing::blake2_256(&wasm)),
        H256::from_str("d601b781566850086b9341552bcde75b43902665a01fc94897da2adf3cac9a26").unwrap()
    );
    let (mut ext, peers, _) = crate::ocw_only_halt_wasm_tests::setup(&wasm);
    let (acknowledgement, guardian, parent) = ext.execute_with(|| {
        for (entry, key) in fixture["storage"].as_array().unwrap().iter().zip(&keys) {
            let actual = values[key].as_ref().unwrap();
            assert_eq!(*actual, recorded_bytes(&entry["value"]));
            sp_io::storage::set(key, actual);
        }
        // A real node retains predecessor hashes for the mortal signature. Build
        // the corresponding local fixture header instead of borrowing a live hash.
        let now = *recorded_header.number();
        let era = sp_runtime::generic::Era::mortal(
            u64::from(framenode_runtime::BlockHashCount::get()),
            u64::from(now - 1),
        );
        let birth = u32::try_from(era.birth(u64::from(now))).unwrap();
        assert_eq!(birth, now - 1);
        let previous_root =
            H256::from_slice(&sp_io::storage::root(sp_core::storage::StateVersion::V1));
        let parent = Header::new(
            birth,
            H256::zero(),
            previous_root,
            H256::zero(),
            Default::default(),
        )
        .hash();
        frame_system::BlockHash::<framenode_runtime::Runtime>::insert(birth, parent);
        framenode_runtime::System::set_block_number(now);
        let completion = bridge_types::recovery::completion().unwrap().unwrap();
        let acknowledgement = bridge_types::recovery::RecoveryAcknowledgement::new(
            recovery::GENESIS_HASH,
            &completion,
        );
        (
            acknowledgement,
            common::bridge_halt::latest_guardian_halt()
                .unwrap()
                .unwrap(),
            parent,
        )
    });
    // Execute against the disposable state under its own header. The public
    // header authenticates the copied registry fields and is never presented
    // as the state root of this local signer fixture.
    let local_root = ext.execute_with(|| {
        H256::from_slice(&sp_io::storage::root(sp_core::storage::StateVersion::V1))
    });
    let header = Header::new(
        *recorded_header.number(),
        H256::zero(),
        local_root,
        parent,
        Default::default(),
    );
    assert_ne!(header.hash(), recorded_header.hash());
    let keystore = MemoryKeystore::new();
    // This publicly documented disposable seed matches the fixture peer initialized by setup().
    let signer = keystore
        .ecdsa_generate_new(
            eth_bridge::KEY_TYPE,
            Some(&format!("0x{}", "51".repeat(32))),
        )
        .unwrap();
    assert_eq!(signer, sp_core::Pair::public(&peers[0]));
    let native_url = "http://native.invalid";
    let ethereum_url = "http://ethereum.invalid";
    let target = fixture["ethereumBlock"].as_u64().unwrap();
    let history = target.checked_add_signed(history_offset).unwrap();
    let cursor = eth_bridge::eth_scan_cursor_key(0);
    let mut db = TestPersistentOffchainDB::new();
    for (key, value) in [
        (
            eth_bridge::STORAGE_NETWORK_IDS_KEY,
            BTreeSet::from([0u32]).encode(),
        ),
        (
            eth_bridge::STORAGE_PEER_MARKER_KEY,
            signer.0.to_vec().encode(),
        ),
        (
            eth_bridge::STORAGE_RESUMED_AFTER_HALT_KEY,
            guardian.encode(),
        ),
        (
            bridge_types::recovery::RECOVERY_ACK_STORAGE_KEY,
            acknowledgement.encode(),
        ),
        (
            eth_bridge::STORAGE_SUB_TO_HANDLE_FROM_HEIGHT_KEY,
            (header.number() + 1).encode(),
        ),
        (eth_bridge::STORAGE_SUB_NODE_URL_KEY, native_url.encode()),
        (
            b"eth-bridge-ocw::node-params-0",
            (ethereum_url, Option::<String>::None).encode(),
        ),
        (cursor.as_slice(), history.encode()),
    ] {
        db.set(STORAGE_PREFIX, key, &value);
    }

    let plan =
        crate::bridge_priority_scan::PriorityScanPlan::new(H256::repeat_byte(7), vec![target])
            .unwrap();
    let progress_key = plan.progress_key().to_vec();
    let passes = crate::guardian_halt::worker_guard::WorkerPasses::new(Some(plan));
    let (offchain, state) = TestOffchainExt::new();
    let (pool, pool_state) = TestTransactionPoolExt::new();
    ext.register_extension(passes.test_extension(db.clone()));
    ext.register_extension(OffchainWorkerExt::new(offchain));
    ext.register_extension(TransactionPoolExt::new(pool));
    ext.register_extension(KeystoreExt(Arc::new(keystore)));
    ext.register_extension(crate::bridge_local_quarantine::extension());
    let log = &fixture["log"];
    let receipt = &fixture["receipt"];
    assert_eq!(receipt["transactionHash"], log["transactionHash"]);
    let filter = json!([{"address":log["address"],"fromBlock":log["blockNumber"],"toBlock":log["blockNumber"],"topics":log["topics"]}]);
    for expected in [
        response(
            native_url,
            "chain_getFinalizedHead",
            Value::Null,
            json!(header.hash()),
        ),
        response(
            native_url,
            "chain_getHeader",
            json!([header.hash()]),
            json!({"number":format!("{:#x}",header.number())}),
        ),
        response(
            ethereum_url,
            "eth_blockNumber",
            Value::Null,
            json!(format!("{:#x}", target + 10_000)),
        ),
        response(ethereum_url, "eth_getLogs", filter, json!([log])),
        response(
            ethereum_url,
            "eth_getTransactionReceipt",
            json!([log["transactionHash"]]),
            receipt.clone(),
        ),
    ] {
        state.write().expect_request(expected);
    }

    let executor = sc_executor::WasmExecutor::<crate::service::HostFunctions>::builder().build();
    crate::ocw_only_halt_wasm_tests::execute(
        &executor,
        &wasm,
        &mut ext,
        "OffchainWorkerApi_offchain_worker",
        &header.encode(),
    );
    assert_eq!(
        OffchainStorage::get(&db, STORAGE_PREFIX, &cursor),
        Some(history.encode())
    );
    let metric = |key: &[u8]| {
        OffchainStorage::get(&db, STORAGE_PREFIX, key)
            .and_then(|bytes| u64::decode_all(&mut &bytes[..]).ok())
    };
    assert_eq!(
        metric(eth_bridge::STORAGE_LOCAL_SIGNING_KEY_READY_KEY),
        Some(1),
        "fixture key lookup"
    );
    assert_eq!(
        metric(eth_bridge::STORAGE_BOOTSTRAP_READY_KEY),
        Some(1),
        "fixture marker, membership and primary cursor"
    );
    assert!(
        OffchainStorage::get(&db, STORAGE_PREFIX, &progress_key).is_some(),
        "worker passed recovery and guardian checks and reserved target"
    );
    assert_eq!(
        metric(b"eth-bridge-ocw::local-peer-ready-0"),
        Some(1),
        "finalized native HTTP and enrolled network peer"
    );
    assert_eq!(
        metric(b"eth-bridge-ocw::eth-height-0"),
        Some(target + 10_000),
        "Ethereum height HTTP"
    );
    drop(ext);
    drop(state); // SDK reports any expected HTTP calls that the runtime did not make.
    let pending = pool_state.read();
    assert_eq!(pending.transactions.len(), 1);
    let xt = UncheckedExtrinsic::decode_all(&mut &pending.transactions[0][..]).unwrap();
    let RuntimeCall::BridgeMultisig(bridge_multisig::Call::as_multi { call, .. }) = xt.function
    else {
        panic!("expected the ordinary multisig proposal")
    };
    let RuntimeCall::EthBridge(eth_bridge::Call::import_incoming_request {
        load_incoming_request,
        incoming_request_result,
    }) = RuntimeCall::decode_all(&mut call.as_slice()).unwrap()
    else {
        panic!("expected authenticated deposit import")
    };
    let transfer = match incoming_request_result.unwrap() {
        eth_bridge::requests::IncomingRequest::Transfer(transfer) => transfer,
        _ => panic!("expected transfer"),
    };
    assert_eq!(
        load_incoming_request.hash(),
        serde_json::from_value(log["transactionHash"].clone()).unwrap()
    );
    assert_eq!(
        transfer.from,
        serde_json::from_value(receipt["from"].clone()).unwrap()
    );
    let expected = &fixture["expectedTransfer"];
    assert_eq!(transfer.to.encode(), recorded_bytes(&expected["recipient"]));
    assert_eq!(
        transfer.asset_id.encode(),
        recorded_bytes(&expected["asset"])
    );
    assert_eq!(
        transfer.amount,
        expected["amount"]
            .as_str()
            .unwrap()
            .parse::<u128>()
            .unwrap()
    );
}
