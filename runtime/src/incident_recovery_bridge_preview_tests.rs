//! This exporter is compiled only into the native unit-test binary. It uses the
//! existing test policy scope; no override exists in any node or Wasm build.
use crate::incident_recovery_preview_audits as preview_audits;

#[path = "incident_recovery_bridge_preview_codec.rs"]
mod wire;
use crate::{AccountId, Balances, Runtime, System};
use bridge_types::SubNetworkId;
use codec::{Decode, DecodeWithMemLimit, Encode};
use common::recovery;
use sp_core::storage::{StateVersion, Storage};
use std::{
    collections::BTreeSet,
    fs,
    io::{Read, Write},
    path::Path,
};

fn prefix(pallet: &[u8], item: &[u8]) -> Vec<u8> {
    [
        sp_io::hashing::twox_128(pallet),
        sp_io::hashing::twox_128(item),
    ]
    .concat()
}
fn consensus(key: &[u8]) -> bool {
    key == b":extrinsic_index"
        || (!key.starts_with(&prefix(b"System", b"Account"))
            && [
                b"System".as_slice(),
                b"Babe",
                b"Grandpa",
                b"Session",
                b"Timestamp",
            ]
            .iter()
            .any(|p| key.starts_with(&sp_io::hashing::twox_128(p))))
}
fn read(ext: &mut sp_io::TestExternalities, key: &[u8]) -> Option<Vec<u8>> {
    ext.execute_with(|| sp_io::storage::get(key).map(|v| v.to_vec()))
}
fn observed(ext: &mut sp_io::TestExternalities) -> wire::Values {
    let keys: Vec<_> = ext
        .overlayed_changes()
        .changes()
        .map(|(k, _)| k.clone())
        .filter(|k| !consensus(k))
        .collect();
    keys.into_iter()
        .map(|k| {
            let value = read(ext, &k);
            (k, value)
        })
        .collect()
}
fn diagnostic(input: &wire::Input) -> String {
    let phase = recovery::storage::status();
    sp_io::storage::start_transaction();
    sp_io::storage::set(
        recovery::BRIDGE_INBOUND_CONTEXT_STORAGE_KEY,
        &System::block_number().encode(),
    );
    let operation = crate::incident_recovery_incoming::IncomingOperation::decode(
        &mut input.incoming.as_slice(),
    )
    .unwrap();
    let (admission, amount) = match operation.channel {
        crate::incident_recovery_incoming::IncomingChannel::Substrate {
            network,
            commitment,
            proof,
        } => {
            let amount = match &commitment {
                bridge_types::GenericCommitment::Sub(commitment) => {
                    let call = bridge_types::substrate::BridgeCall::decode(
                        &mut commitment.messages[0].payload.as_slice(),
                    )
                    .unwrap();
                    match call {
                        bridge_types::substrate::BridgeCall::SubstrateApp(
                            bridge_types::substrate::SubstrateAppCall::Transfer { amount, .. },
                        ) => amount,
                        _ => unreachable!(),
                    }
                }
                _ => unreachable!(),
            };
            (
                crate::SubstrateBridgeInboundChannel::validate_submission(
                    network,
                    &commitment,
                    &proof,
                ),
                amount,
            )
        }
        _ => unreachable!(),
    };
    let mint = crate::SubstrateBridgeApp::mint_inner(
        common::XOR,
        bridge_types::GenericAccount::Liberland(AccountId::new([1; 32])),
        input.replacement.clone(),
        amount,
        SubNetworkId::Liberland,
        bridge_types::H256::repeat_byte(1),
        bridge_types::GenericTimepoint::Sora(recovery::CHECKPOINT_NUMBER),
    );
    let result = format!("phase={phase:?}, native admission={admission:?}, native mint={mint:?}");
    sp_io::storage::rollback_transaction();
    result
}
struct Preview {
    values: wire::Values,
    operations: Vec<(u32, wire::Values)>,
    retained_configuration: wire::Values,
}

fn retained_configuration(
    ext: &mut sp_io::TestExternalities,
    setup: &recovery::RecoveryChunk,
) -> Result<wire::Values, String> {
    setup
        .validate()
        .map_err(|e| format!("invalid public setup: {e:?}"))?;
    if read(ext, recovery::BRIDGE_HALT_STORAGE_KEY).as_deref() != Some(&[1][..]) {
        return Err("public configuration requires the persistent bridge halt".into());
    }
    let mut retained = wire::Values::new();
    for edit in &setup.edits {
        if edit
            .key
            .starts_with(&sp_io::hashing::twox_128(b"SubstrateBridgeOutboundChannel"))
            || edit
                .key
                .starts_with(&sp_io::hashing::twox_128(b"BridgeProxy"))
        {
            continue;
        }
        let actual = read(ext, &edit.key);
        if edit.scope != recovery::StorageScope::Top || actual != edit.after {
            return Err("public configuration changed during native bridge work".into());
        }
        retained.insert(edit.key.clone(), actual);
    }
    if retained.is_empty() {
        return Err("public configuration inventory absent".into());
    }
    Ok(retained)
}

fn run(input: &wire::Input) -> Result<Preview, String> {
    if input.tag != wire::INPUT_TAG
        || input.checkpoint_top.len() > 2_000_000
        || input.stable_keys.len() > 4096
        || input.policy.authorization_domain != recovery::RecoveryDomain::public_fixture()
        || input.policy.chunk_hashes.len() != 3
        || input.setup.index != 0
        || input.old
            != AccountId::new(sp_io::hashing::blake2_256(
                b"sora-nexus/PUBLIC-FIXTURE-ONLY/mixed-bridge/v1/old-owner",
            ))
        || input.replacement
            != AccountId::new(sp_io::hashing::blake2_256(
                b"sora-nexus/PUBLIC-FIXTURE-ONLY/mixed-bridge/v1/replacement-owner",
            ))
        || input.mint != 1
        || input.outgoing_nonce == 0
        || input.stable_keys.iter().cloned().collect::<BTreeSet<_>>()
            != input
                .setup
                .edits
                .iter()
                .map(|edit| edit.key.clone())
                .collect()
        || input.stable_keys.len() != input.setup.edits.len()
    {
        return Err("public fixture bounds/tag".into());
    }
    let storage = Storage {
        top: input.checkpoint_top.clone(),
        children_default: Default::default(),
    };
    let code = storage
        .top
        .get(b":code".as_slice())
        .ok_or("missing checkpoint code")?
        .clone();
    let mut ext =
        sp_io::TestExternalities::new_with_code_and_state(&code, storage.clone(), StateVersion::V0);
    if ext.execute_with(|| sp_io::storage::root(StateVersion::V0))
        != recovery::CHECKPOINT_STATE_ROOT
    {
        return Err("native preview checkpoint root mismatch".into());
    }
    crate::incident_recovery::with_test_policy(&input.policy, || {
        ext.execute_with(|| -> Result<(), String> {
            System::set_block_number(recovery::CHECKPOINT_NUMBER + 1);
            recovery::storage::initialize(&input.policy)
                .map_err(|e| format!("initialize {e:?}"))?;
            recovery::storage::apply_chunk(&input.policy, &input.setup)
                .map_err(|e| format!("setup {e:?}"))?;
            Ok(())
        })?;
        let mut prior = observed(&mut ext);
        let mut operations = Vec::new();
        for index in [1u32, 2] {
            ext.execute_with(|| -> Result<(), String> {
                System::set_block_number(recovery::CHECKPOINT_NUMBER + 1 + index);
                match index {
                    1 => crate::incident_recovery_incoming::apply_encoded_with_policy(
                        &input.policy,
                        &input.incoming,
                    )
                    .map(|_| ())
                    .map_err(|e| format!("incoming {e:?}; {}", diagnostic(input)))?,
                    _ => crate::incident_recovery_outgoing::apply_encoded_with_policy(
                        &input.policy,
                        &input.outgoing,
                    )
                    .map(|_| ())
                    .map_err(|e| format!("outgoing {e:?}"))?,
                }
                Ok(())
            })?;
            let now = observed(&mut ext);
            let delta = now
                .iter()
                .filter(|(k, v)| {
                    let before = prior
                        .get(*k)
                        .cloned()
                        .unwrap_or_else(|| storage.top.get(*k).cloned());
                    &before != *v
                })
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            operations.push((index, delta));
            prior = now;
        }
        ext.execute_with(|| -> Result<(), String> {
            if Balances::free_balance(&input.replacement) != input.mint
                || Balances::free_balance(&input.old) != 0
            {
                return Err("replacement credit mismatch".into());
            }
            if substrate_bridge_channel::inbound::ChannelNonces::<Runtime>::get(
                SubNetworkId::Liberland,
            ) != input.incoming_nonce
                || substrate_bridge_channel::outbound::ChannelNonces::<Runtime>::get(
                    SubNetworkId::Liberland,
                ) != input.outgoing_nonce - 1
            {
                return Err("nonce mismatch".into());
            }
            Ok(())
        })?;
        ext.execute_with(|| preview_audits::run(&input.policy, &input.audits))?;
        prior = observed(&mut ext);
        let keys: BTreeSet<_> = input
            .stable_keys
            .iter()
            .cloned()
            .chain([
                frame_system::Account::<Runtime>::hashed_key_for(&input.old),
                frame_system::Account::<Runtime>::hashed_key_for(&input.replacement),
                frame_system::Account::<Runtime>::hashed_key_for(AccountId::new(
                    recovery::ATTACKER_ACCOUNT,
                )),
                prefix(b"Balances", b"TotalIssuance"),
                substrate_bridge_channel::outbound::ChannelNonces::<Runtime>::hashed_key_for(
                    SubNetworkId::Liberland,
                ),
            ])
            .collect();
        for key in keys {
            prior.insert(key.clone(), read(&mut ext, &key));
        }
        let attacker = frame_system::Account::<Runtime>::hashed_key_for(AccountId::new(
            recovery::ATTACKER_ACCOUNT,
        ));
        if prior.get(&attacker).and_then(Option::as_ref) != storage.top.get(&attacker) {
            return Err("attacker credit changed".into());
        }
        for key in [
            recovery::STATUS_STORAGE_KEY,
            recovery::AUDIT_CURSOR_STORAGE_KEY,
        ] {
            prior.remove(key);
        }
        let retained_configuration = retained_configuration(&mut ext, &input.setup)?;
        Ok(Preview {
            values: prior,
            operations,
            retained_configuration,
        })
    })
}

#[test]
fn retained_public_configuration_requires_halt_and_exact_native_values() {
    let key = prefix(b"MultisigVerifier", b"PeerKeys");
    let setup = recovery::RecoveryChunk {
        index: 0,
        edits: vec![recovery::GuardedStorageEdit {
            scope: recovery::StorageScope::Top,
            key: key.clone(),
            before: None,
            after: Some(vec![42]),
        }],
    };
    let mut ext = sp_io::TestExternalities::default();
    ext.execute_with(|| sp_io::storage::set(&key, &[42]));
    assert!(retained_configuration(&mut ext, &setup).is_err());
    ext.execute_with(|| sp_io::storage::set(recovery::BRIDGE_HALT_STORAGE_KEY, &[1]));
    assert_eq!(
        retained_configuration(&mut ext, &setup).unwrap().get(&key),
        Some(&Some(vec![42]))
    );
    ext.execute_with(|| sp_io::storage::set(&key, &[43]));
    assert!(retained_configuration(&mut ext, &setup).is_err());
    ext.execute_with(|| sp_io::storage::clear(&key));
    assert!(retained_configuration(&mut ext, &setup).is_err());
}

#[test]
#[ignore = "explicit public fixture input/output paths and pinned native test binary required"]
fn export_public_mixed_bridge_preview() {
    let input = std::env::var_os("SORA_RECOVERY_PUBLIC_PREVIEW_INPUT")
        .expect("explicit fixture input required");
    let output = std::env::var_os("SORA_RECOVERY_PUBLIC_PREVIEW_OUTPUT")
        .expect("explicit fixture output required");
    assert!(Path::new(&input).is_absolute() && Path::new(&output).is_absolute());
    let mut file = fs::File::open(input).unwrap();
    assert!(file.metadata().unwrap().len() <= 1024 * 1024 * 1024);
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).unwrap();
    let input_hash = sp_io::hashing::blake2_256(&bytes);
    let mut encoded = bytes.as_slice();
    let decoded =
        wire::Input::decode_with_mem_limit(&mut encoded, 3 * 1024 * 1024 * 1024usize).unwrap();
    assert!(encoded.is_empty() && decoded.encode() == bytes);
    let preview = run(&decoded).unwrap();
    let result = wire::Output {
        tag: wire::OUTPUT_TAG,
        input_hash,
        values: preview.values,
        operations: preview.operations,
        retained_configuration: preview.retained_configuration,
        bridge_halt_verified: true,
    }
    .encode();
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .unwrap();
    output.write_all(&result).unwrap();
    output.sync_all().unwrap();
}
