//! Native-only integration target. It never changes runtime source or Wasm.
#[path = "support/hashi_parser_parity_native.rs"]
mod native;
#[path = "support/hashi_parser_parity_wire.rs"]
#[allow(dead_code)]
mod wire;
use codec::{DecodeWithMemLimit, Encode};
use sp_core::storage::{StateVersion, Storage};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
};

#[test]
#[ignore = "explicit SHA-bound checkpoint input and create-new output paths required"]
fn export_hashi_parser_parity() {
    let input_path = std::env::var_os("SORA_RECOVERY_HASHI_PARITY_INPUT").expect("input path");
    let output_path = std::env::var_os("SORA_RECOVERY_HASHI_PARITY_OUTPUT").expect("output path");
    let expected_hash =
        std::env::var("SORA_RECOVERY_HASHI_PARITY_INPUT_SHA256").expect("input SHA256");
    assert!(Path::new(&input_path).is_absolute() && Path::new(&output_path).is_absolute());
    assert_eq!(
        Path::new(&input_path).canonicalize().unwrap(),
        Path::new(&input_path)
    );
    let mut file = fs::File::open(&input_path).unwrap();
    assert!(file.metadata().unwrap().len() <= wire::MAX_INPUT as u64);
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(wire::MAX_INPUT as u64 + 1)
        .read_to_end(&mut bytes)
        .unwrap();
    assert!(bytes.len() <= wire::MAX_INPUT);
    assert_eq!(wire::hex(&sp_io::hashing::sha2_256(&bytes)), expected_hash);
    let mut cursor = bytes.as_slice();
    let input = wire::Input::decode_with_mem_limit(&mut cursor, wire::MAX_INPUT * 3).unwrap();
    assert!(cursor.is_empty() && input.encode() == bytes);
    native::validate_input(&input);
    let storage = Storage {
        top: input.top.iter().cloned().collect(),
        children_default: Default::default(),
    };
    let code = storage.top.get(b":code".as_slice()).unwrap().clone();
    let mut ext =
        sp_io::TestExternalities::new_with_code_and_state(&code, storage, StateVersion::V0);
    let (before, outcomes, after) = ext.execute_with(|| {
        let before = sp_io::storage::root(StateVersion::V0);
        assert_eq!(before, wire::CHECKPOINT_ROOT);
        let outcomes = input
            .cases
            .iter()
            .map(|case| {
                let result = native::observe(case);
                assert_eq!(
                    result, case.expected,
                    "case {} differs from original expected bytes/category",
                    case.label
                );
                result
            })
            .collect();
        let after = sp_io::storage::root(StateVersion::V0);
        assert_eq!(before, after);
        (before, outcomes, after)
    });
    let result = wire::Output {
        tag: *b"snxparo1",
        input_hash: sp_io::hashing::blake2_256(&bytes),
        scope: input.scope,
        spec_version: framenode_runtime::VERSION.spec_version,
        transaction_version: framenode_runtime::VERSION.transaction_version,
        state_root_before: before,
        state_root_after: after,
        outcomes,
    }
    .encode();
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output_path)
        .unwrap();
    output.write_all(&result).unwrap();
    output.sync_all().unwrap();
}

#[test]
fn parity_wire_rejects_duplicate_state_keys_and_changed_evidence() {
    let make = || wire::Input {
        tag: *b"snxpar01",
        scope: wire::Scope::SyntheticCheckpoint,
        top: vec![(vec![1], vec![2])],
        evidence: (0..3)
            .map(|n| wire::Blob {
                label: n.to_string(),
                sha256: sp_io::hashing::sha2_256(&[n]),
                bytes: vec![n],
            })
            .collect(),
        cases: vec![wire::Case {
            label: "x".into(),
            network: 0,
            height: 1,
            load_scale: vec![],
            logs: vec![],
            expected: wire::Outcome::UnsupportedLoadKind,
        }],
    };
    native::validate_input(&make());
    let mut duplicate = make();
    duplicate.top.push(duplicate.top[0].clone());
    assert!(std::panic::catch_unwind(|| native::validate_input(&duplicate)).is_err());
    let mut changed = make();
    changed.evidence[0].bytes.push(1);
    assert!(std::panic::catch_unwind(|| native::validate_input(&changed)).is_err());
    let mut child = make();
    child.top = vec![(b":child_storage:default:x".to_vec(), vec![2])];
    assert!(std::panic::catch_unwind(|| native::validate_input(&child)).is_err());
}
