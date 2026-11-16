//! Narrow checkpoint rehearsals. These do not certify omitted recovery work or
//! supply a missing victim mapping, operator approval or recovered council list.

use crate::{
    composer, economics, release_checks, substitution_semantics, Result, VerifiedCheckpoint,
};
use codec::Encode;
use common::recovery::{invariants::TerminalAudit, StorageScope, StorageWitness};
use sha2::{Digest, Sha256};

/// Execute only reviewed BridgeMultisig bookkeeping cancellation against the
/// authenticated checkpoint. The observed council is a diagnostic fixture; it
/// must never be mistaken for the approved recovered council in a release.
pub fn bridge_cancellations(
    verified: VerifiedCheckpoint,
    plan_bytes: &[u8],
) -> Result<serde_json::Value> {
    let plan: substitution_semantics::SemanticPlan =
        serde_json::from_slice(plan_bytes).map_err(|e| e.to_string())?;
    if !plan.controls.is_empty()
        || !plan.derived_accounts.is_empty()
        || plan.bridge_cancellations.is_empty()
    {
        return Err(
            "bridge-only rehearsal requires cancellations and no account-control changes".into(),
        );
    }
    let compiled =
        substitution_semantics::compile(&verified.schema, &verified.storage, &[], &[], &plan)?;
    if !compiled.report.blockers.is_empty() {
        return Err(format!(
            "cancellation blockers: {:?}",
            compiled.report.blockers
        ));
    }
    let council = release_checks::checkpoint_council(&verified.schema, &verified.storage)?;
    let native_asset = common::XOR
        .encode()
        .try_into()
        .map_err(|_| "native asset codec")?;
    let terminal = TerminalAudit {
        issuance: vec![economics::issuance_baseline(
            &verified.schema,
            &verified.storage,
            native_asset,
        )?],
        collateral: vec![],
        zero_claims: vec![],
        absent: vec![],
    };
    let mut witnesses = compiled.preservation_witnesses;
    // This diagnostic makes no governance repair. Preserve exact presence and
    // absence, including pre-existing membership/collective discrepancies.
    let mut governance = Vec::new();
    for name in [
        "Council.Members",
        "Council.Prime",
        "TechnicalMembership.Members",
        "TechnicalMembership.Prime",
        "TechnicalCommittee.Members",
        "TechnicalCommittee.Prime",
    ] {
        let d = verified
            .schema
            .descriptors
            .iter()
            .find(|d| d.trie == "top" && d.name == name)
            .ok_or_else(|| format!("missing governance metadata: {name}"))?;
        if !d.hashers.is_empty() {
            return Err("unexpected governance storage map".into());
        }
        let value_hash = verified
            .storage
            .top
            .get(&d.prefix)
            .map(|bytes| sp_io::hashing::blake2_256(bytes));
        witnesses.push(StorageWitness {
            scope: StorageScope::Top,
            key: d.prefix.clone(),
            value_hash,
        });
        governance
            .push(serde_json::json!({"storage":name, "valueHash":value_hash.map(hex::encode)}));
    }
    // Every registered token's issuance must remain byte-identical in this
    // bookkeeping-only rehearsal. These are separate bounded audit chunks.
    let prefix = &verified
        .schema
        .descriptors
        .iter()
        .find(|d| d.trie == "top" && d.name == "Tokens.TotalIssuance")
        .ok_or("missing token issuance metadata")?
        .prefix;
    for (key, value) in verified.storage.top.range(prefix.clone()..) {
        if !key.starts_with(prefix) {
            break;
        }
        witnesses.push(StorageWitness {
            scope: StorageScope::Top,
            key: key.clone(),
            value_hash: Some(sp_io::hashing::blake2_256(value)),
        });
    }
    let plan_hash = Sha256::digest(plan_bytes);
    let evidence = sp_io::hashing::blake2_256(
        &(
            b"sora-nexus/bridge-cancellation-rehearsal/v1".as_slice(),
            common::recovery::CHECKPOINT_HASH,
            plan_hash.as_slice(),
        )
            .encode(),
    );
    let work = composer::compose(
        &verified.storage,
        vec![],
        council,
        composer::PolicyIdentity::public_fixture(evidence),
        compiled.groups,
        witnesses,
        terminal,
    )?;
    Ok(serde_json::json!({
        "schema": "sora-nexus-bridge-cancellation-rehearsal-v1",
        "checkpoint": verified.state,
        "metadataWitness": verified.metadata_witness,
        "planSha256": hex::encode(plan_hash),
        "cancellations": plan.bridge_cancellations.len(),
        "semanticChecks": compiled.report,
        "preservedGovernance": governance,
        "work": work.report,
        "councilSource": "observed checkpoint membership, diagnostic fixture only",
        "nominalDepositRefunds": false,
        "historicalCallsExecuted": false,
        "productionReady": false,
        "remaining": "Actual mapping and recovered council, all other state transformations, consensus and exact-artifact recovery rehearsals remain required"
    }))
}
