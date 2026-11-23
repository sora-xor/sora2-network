//! Unsigned construction and same-execution witnesses for council-pinned repair chunks.
use crate::{replay, Result};
use codec::{DecodeAll, DecodeWithMemLimit, Encode};
use common::recovery::{self, RepairAuthorization, RepairStagingManifest};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fs, io::Read, path::Path};

pub(crate) const AUTH_KEY: &[u8] = b":sora-nexus:recovery:repair-stage-authorization:v1";
pub(crate) const NONCE_KEY: &[u8] = b":sora-nexus:recovery:repair-stage-nonce:v1";
fn sha(raw: &[u8]) -> String {
    hex::encode(Sha256::digest(raw))
}
fn hx(raw: &[u8]) -> String {
    format!("0x{}", hex::encode(raw))
}
fn read(path: &Path, maximum: usize) -> Result<Vec<u8>> {
    let mut raw = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(maximum as u64 + 1)
        .read_to_end(&mut raw)
        .map_err(|e| e.to_string())?;
    if raw.len() > maximum {
        return Err("staging input exceeds bounded size".into());
    }
    Ok(raw)
}

pub fn payload_files(authorization: &Path, code: &Path, nonce: u64) -> Result<Value> {
    payload(
        &read(authorization, 4096)?,
        &read(code, 32 * 1024 * 1024)?,
        nonce,
    )
}

pub fn payload(authorization: &[u8], code: &[u8], nonce: u64) -> Result<Value> {
    let mut input = authorization;
    let authorization = RepairAuthorization::decode_with_mem_limit(&mut input, 16 * 1024)
        .map_err(|e| e.to_string())?;
    if !input.is_empty() {
        return Err("trailing repair authorization bytes".into());
    }
    if authorization.destination_code_hash != sp_io::hashing::blake2_256(code) {
        return Err("repair bytes differ from unsigned upgrade authorization".into());
    }
    let manifest = RepairStagingManifest {
        authorization,
        staging_nonce: nonce,
        code_len: code
            .len()
            .try_into()
            .map_err(|_| "repair code length overflow")?,
        chunk_hashes: code
            .chunks(recovery::MAX_CODE_CHUNK_BYTES)
            .map(sp_io::hashing::blake2_256)
            .collect(),
    };
    manifest
        .validate()
        .map_err(|e| format!("invalid staging manifest: {e:?}"))?;
    let encoded = manifest.encode();
    let message = manifest.signing_message();
    // Round-trip the exact object signers receive; this command never signs or
    // infers a current chain nonce/council authorization from caller inputs.
    if RepairStagingManifest::decode_all(&mut encoded.as_slice()).map_err(|e| e.to_string())?
        != manifest
    {
        return Err("noncanonical repair staging manifest".into());
    }
    Ok(
        json!({"schema":"sora-nexus-unsigned-repair-staging-payload-v1",
        "manifestScale":hx(&encoded),"manifestSha256":sha(&encoded),
        "signingMessageScale":hx(&message),"signingMessageSha256":sha(&message),
        "authorizationScale":hx(&manifest.authorization.encode()),"stagingNonce":nonce,
        "destinationCodeBlake2":hx(&manifest.authorization.destination_code_hash),
        "destinationSpec":manifest.authorization.destination_spec,"codeLength":manifest.code_len,
        "chunkHashes":manifest.chunk_hashes.iter().map(|h|hx(h)).collect::<Vec<_>>(),
        "actionIndex":12,"quorumVerified":false,"currentChainContextVerified":false,
        "runtimeVersionVerified":false,"productionReady":false}),
    )
}

fn nonce(ext: &mut sp_io::TestExternalities) -> Result<u64> {
    ext.execute_with(|| sp_io::storage::get(NONCE_KEY))
        .map(|raw| u64::decode_all(&mut raw.as_ref()).map_err(|e| e.to_string()))
        .transpose()
        .map(|value| value.unwrap_or(0))
}

pub(crate) struct Context<'a> {
    pub step: usize,
    pub number: u32,
    pub executing_spec: u32,
    pub executing_code: &'a [u8],
}
pub(crate) struct Prepared {
    manifest: RepairStagingManifest,
    envelope: Vec<u8>,
    status: recovery::RecoveryStatus,
    witness: Value,
}
impl Prepared {
    pub(crate) fn prepare(
        payload: &[u8],
        policy: &recovery::RecoveryPolicy,
        ext: &mut sp_io::TestExternalities,
        context: Context<'_>,
    ) -> Result<Option<Self>> {
        let envelope = recovery::RecoveryInherent::decode_remark(payload)
            .map_err(|e| format!("staging envelope: {e:?}"))?;
        let recovery::RecoveryAction::AuthorizeRepairStaging {
            manifest,
            signatures,
        } = &envelope.action
        else {
            return Ok(None);
        };
        if envelope.encode_remark() != payload {
            return Err("noncanonical staging envelope".into());
        }
        let status = ext
            .execute_with(recovery::storage::status)
            .map_err(|e| format!("staging status: {e:?}"))?
            .ok_or("staging status absent")?;
        let anchor = ext
            .execute_with(recovery::storage::recovery_anchor)
            .map_err(|e| format!("staging anchor: {e:?}"))?;
        let before = nonce(ext)?;
        manifest
            .verify_quorum(policy, &status, anchor, before, signatures)
            .map_err(|e| format!("staging quorum: {e:?}"))?;
        let code = replay::code(ext)?;
        let hash = sp_io::hashing::blake2_256(&code);
        if status.format_version != 1
            || ext
                .execute_with(|| sp_io::storage::get(recovery::POLICY_STORAGE_KEY))
                .as_deref()
                != Some(policy.commitment().as_slice())
            || context.number <= recovery::CHECKPOINT_NUMBER + 1
            || !matches!(context.executing_spec, 135 | 136)
            || manifest.authorization.destination_spec <= context.executing_spec
            || manifest.authorization.current_code_hash != hash
            || manifest.authorization.current_executor_hash != hash
            || code != context.executing_code
            || status.phase == recovery::RecoveryPhase::Complete
        {
            return Err("staging authorization differs from current execution".into());
        }
        let witness = json!({"schema":"sora-nexus-executed-repair-staging-authorization-v1",
            "step":context.step,"blockNumber":context.number,
            "policyCommitment":hx(&policy.commitment()),"recoveryAnchor":hx(&anchor),
            "authorizationDomainScale":hx(&manifest.authorization.authorization_domain.encode()),
            "phase":format!("{:?}",status.phase),"cursor":status.next_chunk,"operationCounter":status.operation_counter,
            "stagingNonceBefore":before,"stagingNonceAfter":before.checked_add(1).ok_or("staging nonce overflow")?,
            "storedCodeBlake2":hx(&hash),"executingCodeBlake2":hx(&hash),
            "destinationCodeBlake2":hx(&manifest.authorization.destination_code_hash),
            "destinationSpec":manifest.authorization.destination_spec,"codeLength":manifest.code_len,
            "chunkHashes":manifest.chunk_hashes.iter().map(|h|hx(h)).collect::<Vec<_>>(),
            "signers":signatures.iter().map(|(account,_)|hx(AsRef::<[u8]>::as_ref(account))).collect::<Vec<_>>(),
            "quorumVerified":true,"productionReady":false});
        Ok(Some(Self {
            manifest: (**manifest).clone(),
            envelope: payload.to_vec(),
            status,
            witness,
        }))
    }

    pub(crate) fn accepted(
        mut self,
        ext: &mut sp_io::TestExternalities,
        row: &crate::maintenance::BlockWitness,
        output: &Path,
    ) -> Result<Value> {
        let status = ext
            .execute_with(recovery::storage::status)
            .map_err(|e| format!("staging status: {e:?}"))?
            .ok_or("staging status absent")?;
        let encoded = self.manifest.encode();
        if nonce(ext)? != self.manifest.staging_nonce + 1
            || status != self.status
            || ext
                .execute_with(|| sp_io::storage::get(AUTH_KEY))
                .as_deref()
                != Some(encoded.as_slice())
            || sp_io::hashing::blake2_256(&replay::code(ext)?)
                != self.manifest.authorization.current_code_hash
            || row.number as u64
                != self.witness["blockNumber"]
                    .as_u64()
                    .ok_or("staging block absent")?
            || row.executing_code_blake2
                != hex::encode(self.manifest.authorization.current_executor_hash)
            || row.phase != format!("{:?}", self.status.phase)
            || row.cursor != self.status.next_chunk
            || row.operation_counter != self.status.operation_counter
            || !row.maintenance_session_frozen
            || row.state_root != format!("{:#x}", replay::root(ext))
            || !row.independent_import_root_matches
            || !row.disk_restart_root_matches
        {
            return Err(
                "staging authorization was not retained by exact import and restart".into(),
            );
        }
        self.witness["blockHash"] = json!(row.hash);
        self.witness["stateRoot"] = json!(row.state_root);
        self.witness["diskSnapshotManifestSha256"] = json!(row.disk_snapshot_manifest_sha256);
        self.witness["independentImportAndRestartVerified"] = json!(true);
        for (suffix, field, bytes) in [
            ("manifest.scale", "manifestSha256", encoded),
            (
                "signing-message.scale",
                "signingMessageSha256",
                self.manifest.signing_message(),
            ),
            ("envelope.scale", "envelopeSha256", self.envelope),
        ] {
            let file = format!("{}.repair-staging.{suffix}", row.number);
            fs::write(output.join(&file), &bytes).map_err(|e| e.to_string())?;
            self.witness[field] = json!(sha(&bytes));
        }
        fs::write(
            output.join(format!("{}.repair-staging.authorization.json", row.number)),
            serde_json::to_vec_pretty(&self.witness).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        Ok(self.witness)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn make_authorization(code: &[u8]) -> RepairAuthorization {
        RepairAuthorization {
            policy: [1; 32],
            phase: recovery::RecoveryPhase::Hold,
            cursor: 2,
            operation_counter: 1,
            current_code_hash: [2; 32],
            current_executor_hash: [2; 32],
            destination_code_hash: sp_io::hashing::blake2_256(code),
            destination_spec: 136,
            revised_policy: [3; 32],
            authorization_domain: recovery::RecoveryDomain::public_fixture(),
            recovery_anchor: [4; 32],
        }
    }
    #[test]
    fn unsigned_payload_binds_exact_ordered_chunks_without_claiming_authorization() {
        let mut code = vec![7; recovery::MAX_CODE_CHUNK_BYTES];
        code.extend_from_slice(b"last");
        let authorization = make_authorization(&code);
        let result = payload(&authorization.encode(), &code, 9).unwrap();
        let raw = crate::hex_bytes(result["manifestScale"].as_str().unwrap()).unwrap();
        let manifest = RepairStagingManifest::decode_all(&mut raw.as_slice()).unwrap();
        assert_eq!(manifest.authorization, authorization);
        assert_eq!(manifest.staging_nonce, 9);
        assert_eq!(manifest.code_len, code.len() as u32);
        assert_eq!(
            manifest.chunk_hashes,
            vec![
                sp_io::hashing::blake2_256(&code[..recovery::MAX_CODE_CHUNK_BYTES]),
                sp_io::hashing::blake2_256(b"last")
            ]
        );
        assert_eq!(
            result["signingMessageScale"],
            hx(&manifest.signing_message())
        );
        assert_eq!(result["actionIndex"], 12);
        for flag in [
            "quorumVerified",
            "currentChainContextVerified",
            "runtimeVersionVerified",
            "productionReady",
        ] {
            assert_eq!(result[flag], false);
        }
    }
    #[test]
    fn execution_observer_requires_current_signed_context_and_retained_native_transition() {
        use sp_core::Pair;
        let pair = sp_core::ed25519::Pair::from_seed(&[19; 32]);
        let policy = recovery::RecoveryPolicy {
            format_version: 2,
            authorization_domain: recovery::RecoveryDomain::public_fixture(),
            checkpoint_hash: recovery::CHECKPOINT_HASH,
            checkpoint_state_root: recovery::CHECKPOINT_STATE_ROOT,
            replacements: vec![],
            council: vec![pair.public().0.into()],
            chunk_hashes: vec![[2; 32]],
            audit_chunk_hashes: vec![[3; 32]],
            evidence_commitment: [4; 32],
            terminal_audit_commitment: [5; 32],
        };
        let code = b"installed-hold";
        let mut ext = sp_io::TestExternalities::default();
        let status = ext.execute_with(|| {
            sp_io::storage::set(b":code", code);
            let status = recovery::storage::initialize(&policy).unwrap();
            recovery::storage::record_recovery_anchor([4; 32]).unwrap();
            status
        });
        let mut authorization = make_authorization(b"future-repair");
        authorization.policy = policy.commitment();
        authorization.phase = status.phase;
        authorization.cursor = status.next_chunk;
        authorization.operation_counter = status.operation_counter;
        authorization.current_code_hash = sp_io::hashing::blake2_256(code);
        authorization.current_executor_hash = authorization.current_code_hash;
        let manifest = RepairStagingManifest {
            authorization,
            staging_nonce: 0,
            code_len: 13,
            chunk_hashes: vec![sp_io::hashing::blake2_256(b"future-repair")],
        };
        let envelope = recovery::RecoveryInherent {
            format_version: 1,
            action: recovery::RecoveryAction::AuthorizeRepairStaging {
                manifest: Box::new(manifest.clone()),
                signatures: vec![(
                    pair.public().0.into(),
                    sp_runtime::MultiSignature::Ed25519(pair.sign(&manifest.signing_message())),
                )],
            },
        }
        .encode_remark();
        let context = || Context {
            step: 2,
            number: recovery::CHECKPOINT_NUMBER + 3,
            executing_spec: 135,
            executing_code: code,
        };
        let prepared = Prepared::prepare(&envelope, &policy, &mut ext, context())
            .unwrap()
            .unwrap();
        // A sidecar/prepared signature cannot count as an executed authorization.
        let row = crate::maintenance::BlockWitness {
            number: recovery::CHECKPOINT_NUMBER + 3,
            hash: String::new(),
            state_root: format!("{:#x}", replay::root(&mut ext)),
            executing_code_blake2: hex::encode(sp_io::hashing::blake2_256(code)),
            resulting_spec: 135,
            phase: format!("{:?}", status.phase),
            cursor: status.next_chunk,
            operation_counter: status.operation_counter,
            block_sha256: String::new(),
            disk_snapshot_manifest_sha256: String::new(),
            forbidden_extrinsics_rejected: 0,
            maintenance_session_frozen: true,
            independent_import_root_matches: true,
            disk_restart_root_matches: true,
            generated_authorization_sha256: None,
        };
        assert!(prepared
            .accepted(&mut ext, &row, Path::new("/unused-no-write"))
            .is_err());
        ext.execute_with(|| sp_io::storage::set(NONCE_KEY, &1u64.encode()));
        assert!(Prepared::prepare(&envelope, &policy, &mut ext, context()).is_err());
        ext.execute_with(|| {
            sp_io::storage::clear(NONCE_KEY);
            sp_io::storage::set(recovery::POLICY_STORAGE_KEY, &[9; 32]);
        });
        assert!(Prepared::prepare(&envelope, &policy, &mut ext, context()).is_err());
    }

    #[test]
    fn unsigned_payload_rejects_other_code_trailing_authorization_and_exhausted_nonce() {
        let code = b"bounded-code";
        let authorization = make_authorization(code);
        assert!(payload(&authorization.encode(), b"different", 0).is_err());
        let mut trailing = authorization.encode();
        trailing.push(0);
        assert!(payload(&trailing, code, 0).is_err());
        assert!(payload(&authorization.encode(), code, u64::MAX).is_err());
        let empty = make_authorization(&[]);
        assert!(payload(&empty.encode(), &[], 0).is_err());
    }
}
