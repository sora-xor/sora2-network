use clap::{Parser, Subcommand};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Parser)]
#[command(about = "Offline SORA Nexus incident recovery evidence tools")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Conditional native HASHI event/request association; no source finality or delivery.
    #[cfg(feature = "runtime-work-types")]
    VerifyHashiAssociation {
        #[arg(long)]
        capture: PathBuf,
        #[arg(long)]
        manifest: PathBuf,
    },
    /// Build explicitly synthetic bridge work for exact-Wasm fixture rehearsals.
    #[cfg(feature = "bridge-fixture")]
    PrepareMixedBridgeFixture {
        #[arg(long)]
        capture: PathBuf,
    },
    /// Public HASHI peers and mapped-owner work; never production authorization.
    #[cfg(feature = "bridge-fixture")]
    PrepareHashiBridgeFixture {
        #[arg(long)]
        capture: PathBuf,
    },
    /// Published test-key replacement proof; never victim authorization.
    #[cfg(feature = "bridge-fixture")]
    PrepareFundedStakingIdentity {
        /// Root-verified typed checkpoint council, separate from the public signer.
        #[arg(long)]
        capture: Option<PathBuf>,
        #[arg(long)]
        old: String,
        #[arg(long, default_value_t = 0)]
        signer_index: usize,
    },
    /// PUBLIC FIXTURE ONLY: fund a typed mapped public signer through native HASHI.
    #[cfg(feature = "bridge-fixture")]
    PrepareFundedStakingFixture {
        #[arg(long)]
        capture: PathBuf,
        #[arg(long)]
        manifest: PathBuf,
    },
    /// Check native economic/control relationships without a release mapping.
    VerifyCheckpointControls {
        #[arg(long)]
        capture: PathBuf,
        #[arg(long)]
        staking_history: PathBuf,
        #[arg(long)]
        democracy: PathBuf,
        #[arg(long)]
        scheduler: Option<PathBuf>,
        #[arg(long, requires = "operators_scale")]
        staking_roles: Option<PathBuf>,
        #[arg(long, requires = "staking_roles")]
        operators_scale: Option<PathBuf>,
        #[arg(long)]
        elections: Option<PathBuf>,
        #[arg(long)]
        offences: Option<PathBuf>,
        #[cfg(feature = "runtime-work-types")]
        #[arg(long)]
        bridge_history: Option<PathBuf>,
    },
    /// Verify the complete native staking bags graph in authenticated checkpoint state.
    VerifyCheckpointBags {
        #[arg(long)]
        capture: PathBuf,
    },
    /// Assemble bounded typed work without authenticating evidence or executing it.
    #[cfg(feature = "runtime-work-types")]
    CompileMixedWork {
        #[arg(long)]
        manifest: PathBuf,
    },
    /// Bind all native checkpoint pending incoming records and declared external
    /// obligations to exact typed work; does not authenticate external finality.
    #[cfg(feature = "runtime-work-types")]
    VerifyIncomingCoverage {
        #[arg(long)]
        capture: PathBuf,
        #[arg(long)]
        policy: PathBuf,
        #[arg(long)]
        work: PathBuf,
        #[arg(long)]
        inventory: PathBuf,
        #[arg(long)]
        reconciliation: PathBuf,
        #[arg(long)]
        external_heads: PathBuf,
        /// Re-execute a pinned maintenance manifest to authenticate prior-policy
        /// companions. Serialized replay reports are never accepted as proof.
        #[arg(long, requires = "recovery_output")]
        recovery_replay: Option<PathBuf>,
        #[arg(long, requires = "recovery_replay")]
        recovery_output: Option<PathBuf>,
    },
    /// PUBLIC FIXTURE ONLY ordinary final-138 HASHI diagnostic from a pinned completed snapshot.
    #[cfg(feature = "bridge-fixture")]
    ProbePostCompleteHashi {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// PUBLIC FIXTURE ONLY final-138 native Substrate channel diagnostic from mixed39.
    #[cfg(feature = "bridge-fixture")]
    ProbePostCompleteChannel {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Public TON/Parachain diagnostic using an already built mixed-policy138 artifact.
    #[cfg(feature = "bridge-fixture")]
    ProbePostCompleteFamily {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Execute exact recovery Wasm with persisted-state restart after every block.
    RehearseMaintenance {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// PUBLIC FIXTURE ONLY: check a companion Wasm against exact activation state and blocks.
    #[cfg(feature = "bridge-fixture")]
    RehearseTryRuntimeCompanion {
        #[arg(long)]
        manifest: PathBuf,
    },
    /// Construct unsigned, domain-separated council repair-staging payloads.
    RepairStagingPayloads {
        #[arg(long)]
        authorization: PathBuf,
        #[arg(long)]
        code: PathBuf,
        #[arg(long)]
        staging_nonce: u64,
    },
    /// Fresh normal activation and read-only final typed-ownership/financial checks.
    RehearseStorageSemantics {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// PUBLIC TEST KEYS ONLY: inspect full-base final state before signed staking calls.
    #[cfg(feature = "bridge-fixture")]
    RehearsePublicStorageSemantics {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Execute a fresh baseline, then SIGKILL exact-Wasm importers at committed snapshots.
    #[cfg(all(unix, feature = "runtime-work-types"))]
    RehearseMaintenanceCrashes {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        output: PathBuf,
        /// Require the same opaque prior-policy certificate as incoming coverage.
        #[arg(long)]
        require_prior_incoming_evidence: bool,
    },
    /// Internal bounded importer; its report alone is never release evidence.
    #[cfg(all(unix, feature = "runtime-work-types"))]
    #[command(hide = true)]
    MaintenanceCrashWorker {
        #[arg(long)]
        job: PathBuf,
    },
    /// Verify authenticated Burn-event history and export trusted extractor input.
    VerifyBurnHistory {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Incrementally prove current event/code reads for bounded offline capture.
    BurnProofSession {
        #[arg(long)]
        capture: PathBuf,
    },
    /// Rehearse exact bridge bookkeeping cancellation on the checkpoint state.
    /// This diagnostic does not approve a mapping or recovered council.
    RehearseBridgeCancellations {
        #[arg(long)]
        capture: PathBuf,
        #[arg(long)]
        plan: PathBuf,
    },
    /// Print immutable final-runtime source from a validated raw SCALE policy.
    /// This does not authorize a release or modify the working runtime source.
    EmitFinalPolicy {
        #[arg(long)]
        policy: PathBuf,
    },
    /// Re-execute retained blocks under deployed and exact substitute Wasm.
    ReplayHistory {
        #[arg(long)]
        manifest: PathBuf,
    },
    /// Independently reconstruct the captured top/child state and header roots.
    VerifyCapture {
        #[arg(long)]
        capture: PathBuf,
    },
    /// Execute checkpoint Wasm and compare its metadata with the captured schema.
    VerifyMetadata {
        #[arg(long)]
        capture: PathBuf,
    },
    /// Print unsigned messages for the NEW account owners to sign externally.
    MappingPayloads {
        #[arg(long)]
        csv: PathBuf,
    },
    /// Verify key possession only; this does not certify unused destinations.
    VerifyOwnership {
        #[arg(long)]
        csv: PathBuf,
        #[arg(long)]
        proofs: PathBuf,
        /// Authenticate derived controls against exact checkpoint state/metadata.
        #[arg(long, requires_all = ["semantic", "derived_inventory"])]
        capture: Option<PathBuf>,
        /// The same semantic plan consumed by storage composition.
        #[arg(long, requires_all = ["capture", "derived_inventory"])]
        semantic: Option<PathBuf>,
        /// Exact JSON derivedAccounts array, bound separately in the release.
        #[arg(long, requires_all = ["capture", "semantic"])]
        derived_inventory: Option<PathBuf>,
    },
    /// Derive retained consensus key inventories from authenticated checkpoint state.
    #[cfg(feature = "runtime-work-types")]
    DeriveCheckpointConsensus {
        #[arg(long)]
        capture: PathBuf,
    },
    /// Admit the trusted release CSV and generate state-bearing utility mappings.
    VerifyTrustedMapping {
        #[arg(long)]
        csv: PathBuf,
        #[arg(long)]
        capture: PathBuf,
        #[arg(long)]
        inventory: PathBuf,
        #[arg(long)]
        semantic: PathBuf,
    },
    /// Enumerate every storage prefix and report unresolved semantic coverage.
    Inventory {
        #[arg(long)]
        capture: PathBuf,
        #[arg(long)]
        csv: Option<PathBuf>,
        #[arg(long)]
        dispositions: Option<PathBuf>,
    },
    /// Compile evidence-bound burn/claim candidates, retaining all blockers.
    EconomicCandidates {
        #[arg(long)]
        capture: PathBuf,
        #[arg(long)]
        plan: PathBuf,
    },
    /// Compile reviewed control-pointer and pending-operation changes.
    SemanticCandidates {
        #[arg(long)]
        capture: PathBuf,
        #[arg(long)]
        csv: PathBuf,
        #[arg(long)]
        proofs: PathBuf,
        #[arg(long)]
        plan: PathBuf,
    },
    /// Compose complete typed storage work and execute its guards and audits.
    /// Consensus, bridge admission and exact artifacts remain separate gates.
    ComposeStorageWork(Box<ComposeStorageWorkArgs>),
}

#[derive(clap::Args)]
struct ComposeStorageWorkArgs {
    /// Complete bridge owner/message pairs and terminal refund suppression.
    #[cfg(feature = "runtime-work-types")]
    #[arg(long)]
    bridge_history: Option<PathBuf>,
    /// Complete native approvals/pending/latest-commitment cancellation review.
    #[cfg(feature = "runtime-work-types")]
    #[arg(long)]
    bridge_signing: Option<PathBuf>,
    /// Complete native HASHI request/signature/index cancellation commitments.
    #[cfg(feature = "runtime-work-types")]
    #[arg(long)]
    bridge_hashi: Option<PathBuf>,
    /// Complete council-election records and retained financial backing.
    #[arg(long)]
    elections: Option<PathBuf>,
    /// Complete historical offence report IDs and concurrent indices.
    #[arg(long)]
    offences: Option<PathBuf>,
    #[arg(long)]
    capture: PathBuf,
    #[arg(long)]
    csv: PathBuf,
    #[arg(long)]
    proofs: Option<PathBuf>,
    #[arg(long)]
    inventory: PathBuf,
    #[arg(long)]
    economic: PathBuf,
    /// Optional create-new JSON artifact for the complete native attacker watch.
    #[arg(long)]
    attacker_coverage_output: Option<PathBuf>,
    /// Create-new typed-interest diagnostic from this complete native inventory.
    /// Does not authorize economic actions or waive later composition failures.
    #[arg(long)]
    attacker_interest_output: Option<PathBuf>,
    #[arg(long)]
    semantic: PathBuf,
    /// Exact roster/pending-collective inventory for governance substitutions.
    #[arg(long)]
    governance: Option<PathBuf>,
    /// Complete paired referral graph/reference review for grant suspension.
    #[arg(long)]
    referral: Option<PathBuf>,
    /// Exact coupled legacy staking controller/ledger review.
    #[arg(long)]
    staking_controls: Option<PathBuf>,
    /// Complete role/list inventory and optional new-owner nomination consent.
    #[arg(long, requires = "operators_scale")]
    staking_roles: Option<PathBuf>,
    /// The exact public operator inventory pinned into bootstrap Wasm.
    #[arg(long, requires = "staking_roles")]
    operators_scale: Option<PathBuf>,
    /// Complete current-state historical staking liability/reward review.
    #[arg(long)]
    staking_history: Option<PathBuf>,
    /// Complete finished-referendum/delegation graph and retained lock review.
    #[arg(long)]
    democracy: Option<PathBuf>,
    /// Exact stale Root schedule and retained preimage/deposit review.
    #[arg(long)]
    scheduler: Option<PathBuf>,
    /// Optional legacy consistency check; council is derived from checkpoint plus CSV.
    #[arg(long)]
    council: Option<PathBuf>,
    /// Commitment to the independently reviewed public evidence manifest.
    #[arg(long)]
    evidence_commitment: String,
    /// Exact chain-spec ID. Synthetic fixtures use a separate domain; exact-artifact
    /// rehearsals retain the production policy and separate signatures by branch hash.
    #[arg(long)]
    chain_spec_id: String,
    /// Unique, independently chosen nonzero 32-byte public release nonce (hex).
    #[arg(long)]
    release_nonce: String,
}

fn read_bounded(path: &Path, limit: u64) -> incident_recovery::Result<Vec<u8>> {
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() > limit {
        return Err("review document exceeds byte bound".into());
    }
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("review document exceeds byte bound".into());
    }
    Ok(bytes)
}

fn read_json<T: serde::de::DeserializeOwned>(path: PathBuf) -> incident_recovery::Result<T> {
    let bytes = read_bounded(&path, 64 * 1024 * 1024)?;
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}

fn read_mapping(
    path: &Path,
) -> incident_recovery::Result<Vec<common::recovery::AccountReplacement>> {
    let bytes = read_bounded(path, 2 * 1024 * 1024)?;
    incident_recovery::parse_mapping_csv(std::str::from_utf8(&bytes).map_err(|e| e.to_string())?)
}

struct TrustedMappingInput {
    trusted: incident_recovery::trusted_mapping::TrustedMapping,
    options: incident_recovery::inventory::InventoryOptions,
    semantic: incident_recovery::substitution_semantics::SemanticPlan,
    report: serde_json::Value,
}

fn trusted_mapping_input(
    checkpoint: &incident_recovery::VerifiedCheckpoint,
    csv: &Path,
    inventory: &Path,
    semantic: &Path,
) -> incident_recovery::Result<TrustedMappingInput> {
    use sha2::{Digest, Sha256};
    let csv_bytes = read_bounded(csv, 2 * 1024 * 1024)?;
    let inventory_bytes = read_bounded(inventory, 64 * 1024 * 1024)?;
    let semantic_bytes = read_bounded(semantic, 64 * 1024 * 1024)?;
    let map = incident_recovery::parse_mapping_csv(
        std::str::from_utf8(&csv_bytes).map_err(|e| e.to_string())?,
    )?;
    let options = serde_json::from_slice(&inventory_bytes).map_err(|e| e.to_string())?;
    let semantic = serde_json::from_slice(&semantic_bytes).map_err(|e| e.to_string())?;
    let trusted = incident_recovery::trusted_mapping::verify(
        &checkpoint.schema,
        &checkpoint.storage,
        &map,
        &options,
    )?;
    let mut report = serde_json::to_value(&trusted.report).map_err(|e| e.to_string())?;
    report["mappingCsvSha256"] = hex::encode(Sha256::digest(&csv_bytes)).into();
    report["inventorySha256"] = hex::encode(Sha256::digest(&inventory_bytes)).into();
    report["semanticPlanSha256"] = hex::encode(Sha256::digest(&semantic_bytes)).into();
    report["checkpointStateRoot"] = checkpoint.state.state_root.clone().into();
    report["captureManifestSha256"] = checkpoint.state.capture_manifest_sha256.clone().into();
    report["checkpointMetadata"] = checkpoint.metadata_witness.clone();
    Ok(TrustedMappingInput {
        trusted,
        options,
        semantic,
        report,
    })
}

fn verify_ownership_command(
    csv: &Path,
    proofs: &Path,
    capture: Option<&Path>,
    semantic: Option<&Path>,
    derived_inventory: Option<&Path>,
) -> incident_recovery::Result<serde_json::Value> {
    use sha2::{Digest, Sha256};
    let csv_bytes = read_bounded(csv, 2 * 1024 * 1024)?;
    let proof_bytes = read_bounded(proofs, 64 * 1024 * 1024)?;
    let map = incident_recovery::parse_mapping_csv(
        std::str::from_utf8(&csv_bytes).map_err(|e| e.to_string())?,
    )?;
    let proofs: Vec<incident_recovery::OwnershipProof> =
        serde_json::from_slice(&proof_bytes).map_err(|e| e.to_string())?;
    let mut result = serde_json::json!({
        "schema": "sora-nexus-account-ownership-verification-v1",
        "checkpointHash": format!("0x{}", hex::encode(common::recovery::CHECKPOINT_HASH)),
        "ownershipVerified": true, "derivedControlVerified": false,
        "accounts": map.len(), "productionReady": false,
        "mappingCsvSha256": hex::encode(Sha256::digest(&csv_bytes)),
        "ownershipProofsSha256": hex::encode(Sha256::digest(&proof_bytes)),
        "remaining": "complete derived-account discovery, unused destinations, historical control review, state/economic and consensus admission"
    });
    match (capture, semantic, derived_inventory) {
        (None, None, None) => incident_recovery::verify_ownership_proofs(&map, &proofs)?,
        (Some(capture), Some(semantic), Some(derived_inventory)) => {
            let semantic_bytes = read_bounded(semantic, 64 * 1024 * 1024)?;
            let inventory_bytes = read_bounded(derived_inventory, 64 * 1024 * 1024)?;
            let plan: incident_recovery::substitution_semantics::SemanticPlan =
                serde_json::from_slice(&semantic_bytes).map_err(|e| e.to_string())?;
            // Parse the independent inventory as the native strict type too;
            // unknown fields cannot disappear in a generic JSON comparison.
            let _: Vec<incident_recovery::substitution_semantics::DerivedAccount> =
                serde_json::from_slice(&inventory_bytes).map_err(|e| e.to_string())?;
            let plan_json: serde_json::Value =
                serde_json::from_slice(&semantic_bytes).map_err(|e| e.to_string())?;
            let inventory_json: serde_json::Value =
                serde_json::from_slice(&inventory_bytes).map_err(|e| e.to_string())?;
            if plan_json
                .get("derivedAccounts")
                .unwrap_or(&serde_json::json!([]))
                != &inventory_json
            {
                return Err("derived inventory differs from semantic plan".into());
            }
            let checkpoint = incident_recovery::verify_checkpoint(capture)?;
            let controls = incident_recovery::substitution_semantics::verify_semantic_ownership(
                &checkpoint.schema,
                &checkpoint.storage,
                &map,
                &proofs,
                &plan,
            )?;
            result["schema"] = "sora-nexus-account-control-verification-v1".into();
            result["derivedControlVerified"] = true.into();
            result["checkpointStateRoot"] = checkpoint.state.state_root.into();
            result["captureManifestSha256"] = checkpoint.state.capture_manifest_sha256.into();
            result["checkpointMetadata"] = checkpoint.metadata_witness;
            result["semanticPlanSha256"] = hex::encode(Sha256::digest(&semantic_bytes)).into();
            result["derivedInventorySha256"] = hex::encode(Sha256::digest(&inventory_bytes)).into();
            result["controls"] = serde_json::to_value(controls).map_err(|e| e.to_string())?;
        }
        _ => {
            return Err(
                "derived ownership requires checkpoint, semantic plan and inventory together"
                    .into(),
            )
        }
    }
    Ok(result)
}

fn require_bridge_control_features(
    storage: &sp_core::storage::Storage,
    runtime_work_types: bool,
) -> incident_recovery::Result<()> {
    if runtime_work_types {
        return Ok(());
    }
    // Native prefixes are computed independently of caller metadata. A reduced
    // feature build must not omit bridge gates just because their CLI fields and
    // native adapters were not compiled into that executable.
    for (pallet, item) in [
        ("BridgeProxy", "Senders"),
        ("BridgeProxy", "Transactions"),
        ("BridgeDataSigner", "Approvals"),
        ("BridgeDataSigner", "PendingApprovals"),
        ("BridgeOutboundChannel", "LatestCommitment"),
        ("SubstrateBridgeOutboundChannel", "LatestCommitment"),
        ("BridgeOutboundChannel", "MessageQueues"),
        ("SubstrateBridgeOutboundChannel", "MessageQueues"),
        ("EthBridge", "Requests"),
        ("EthBridge", "RequestStatuses"),
        ("EthBridge", "RequestApprovals"),
        ("EthBridge", "RequestApprovers"),
        ("EthBridge", "RequestSubmissionHeight"),
        ("EthBridge", "RequestsQueue"),
        ("EthBridge", "AccountRequests"),
    ] {
        let prefix = [
            sp_io::hashing::twox_128(pallet.as_bytes()),
            sp_io::hashing::twox_128(item.as_bytes()),
        ]
        .concat();
        if storage
            .top
            .range(prefix.clone()..)
            .next()
            .is_some_and(|(key, _)| key.starts_with(&prefix))
        {
            return Err("populated checkpoint bridge control prefixes require a tool built with runtime-work-types".into());
        }
    }
    Ok(())
}

fn read_operators(path: &PathBuf) -> incident_recovery::Result<Vec<u8>> {
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(2 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 2 * 1024 * 1024 {
        return Err("operator inventory exceeds bound".into());
    }
    Ok(bytes)
}

fn run() -> incident_recovery::Result<()> {
    match Cli::parse().command {
        #[cfg(feature = "bridge-fixture")]
        Command::PrepareMixedBridgeFixture { capture } => {
            let report = incident_recovery::mixed_bridge_fixture::generate(
                incident_recovery::verify_checkpoint(&capture)?,
            )?;
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
            );
        }
        #[cfg(feature = "bridge-fixture")]
        Command::PrepareHashiBridgeFixture { capture } => {
            let result = incident_recovery::hashi_bridge_fixture::generate(
                incident_recovery::verify_checkpoint(&capture)?,
            )?;
            println!(
                "{}",
                serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?
            );
        }
        #[cfg(feature = "bridge-fixture")]
        Command::PrepareFundedStakingIdentity {
            old,
            signer_index,
            capture,
        } => {
            let result = if let Some(capture) = capture {
                incident_recovery::funded_staking_fixture::identity_at(
                    incident_recovery::verify_checkpoint(&capture)?,
                    &old,
                    signer_index,
                )?
            } else {
                incident_recovery::funded_staking_fixture::identity(&old, signer_index)?
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?
            );
        }
        #[cfg(feature = "bridge-fixture")]
        Command::PrepareFundedStakingFixture { capture, manifest } => {
            let result = incident_recovery::funded_staking_fixture::generate(
                incident_recovery::verify_checkpoint(&capture)?,
                &manifest,
            )?;
            println!(
                "{}",
                serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?
            );
        }
        Command::VerifyCheckpointControls {
            capture,
            staking_history,
            democracy,
            scheduler,
            staking_roles,
            operators_scale,
            elections,
            offences,
            #[cfg(feature = "runtime-work-types")]
            bridge_history,
        } => {
            let verified = incident_recovery::verify_checkpoint(&capture)?;
            let history = incident_recovery::staking_history::compile(
                &verified.schema,
                &verified.storage,
                &[],
                &read_json(staking_history)?,
            )?;
            history.verify_groups(&[], &[])?;
            let democracy = incident_recovery::democracy::validate_checkpoint(
                &verified.schema,
                &verified.storage,
                &read_json(democracy)?,
            )?;
            let scheduler = scheduler
                .map(|path| {
                    let compiled = incident_recovery::scheduler_controls::compile(
                        &verified.schema,
                        &verified.storage,
                        &[],
                        &read_json(path)?,
                    )?;
                    compiled.verify_preservation(&compiled.groups)?;
                    Ok::<_, String>(compiled.report)
                })
                .transpose()?;
            let roles = staking_roles
                .map(|path| {
                    let operators =
                        read_operators(operators_scale.as_ref().ok_or("missing operators-scale")?)?;
                    let compiled = incident_recovery::staking_roles::compile(
                        &verified.schema,
                        &verified.storage,
                        &[],
                        &operators,
                        &read_json(path)?,
                    )?;
                    compiled.verify_groups(&[], &[])?;
                    Ok::<_, String>(compiled.report)
                })
                .transpose()?;
            let elections = elections
                .map(|path| {
                    let compiled = incident_recovery::elections::compile(
                        &verified.schema,
                        &verified.storage,
                        &[],
                        &read_json(path)?,
                    )?;
                    compiled.verify_groups(&[], &[])?;
                    Ok::<_, String>(compiled.report)
                })
                .transpose()?;
            let offences = offences
                .map(|path| {
                    let compiled = incident_recovery::offences::compile(
                        &verified.schema,
                        &verified.storage,
                        &[],
                        &read_json(path)?,
                    )?;
                    compiled.verify_groups(&[], &[])?;
                    Ok::<_, String>(compiled.report)
                })
                .transpose()?;
            #[cfg(feature = "runtime-work-types")]
            let bridge_history = bridge_history
                .map(|path| {
                    let compiled = incident_recovery::bridge_history::compile(
                        &verified.schema,
                        &verified.storage,
                        &[],
                        &read_json(path)?,
                    )?;
                    compiled.verify_groups(&compiled.groups, &[])?;
                    Ok::<_, String>(compiled.report)
                })
                .transpose()?;
            let report = serde_json::json!({
                "schema": "sora-nexus-checkpoint-controls-diagnostic-v1",
                "stakingEconomicHistory": history.report,
                "democracy": democracy,
                "scheduler": scheduler,
                "stakingRoles": roles,
                "elections": elections,
                "offences": offences,
                "productionReady": false,
                "mappingApproved": false,
            });
            #[cfg(feature = "runtime-work-types")]
            let report = {
                let mut report = report;
                report["bridgeHistory"] =
                    serde_json::to_value(bridge_history).map_err(|e| e.to_string())?;
                report
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
            );
        }
        #[cfg(feature = "runtime-work-types")]
        Command::VerifyHashiAssociation { capture, manifest } => {
            let report = incident_recovery::hashi_association::verify_file(&capture, &manifest)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
            );
        }
        Command::VerifyCheckpointBags { capture } => {
            let verified = incident_recovery::verify_checkpoint(&capture)?;
            let result = incident_recovery::bags_list::verify_groups(
                &verified.schema,
                &verified.storage,
                &[],
                &[],
            )?;
            println!(
                "{}",
                serde_json::to_string_pretty(&result.report).map_err(|e| e.to_string())?
            );
        }
        #[cfg(feature = "runtime-work-types")]
        Command::CompileMixedWork { manifest } => {
            let report = incident_recovery::mixed_work::compile_file(&manifest)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
            );
        }
        #[cfg(feature = "runtime-work-types")]
        Command::VerifyIncomingCoverage {
            capture,
            policy,
            work,
            inventory,
            reconciliation,
            external_heads,
            recovery_replay,
            recovery_output,
        } => {
            use codec::{Decode, Encode};
            use sha2::{Digest as _, Sha256};
            let policy_bytes = read_bounded(&policy, 2 * 1024 * 1024)?;
            let mut input = policy_bytes.as_slice();
            let policy = common::recovery::RecoveryPolicy::decode(
                &mut codec::MemTrackingInput::new(&mut input, 8 * 1024 * 1024),
            )
            .map_err(|e| e.to_string())?;
            if !input.is_empty() || policy.encode() != policy_bytes {
                return Err("noncanonical incoming coverage policy".into());
            }
            let work_bytes = read_bounded(&work, 64 * 1024 * 1024)?;
            let inventory_bytes = read_bounded(&inventory, 64 * 1024 * 1024)?;
            let reconciliation_bytes = read_bounded(&reconciliation, 64 * 1024 * 1024)?;
            let heads_bytes = read_bounded(&external_heads, 64 * 1024 * 1024)?;
            let verified = incident_recovery::verify_checkpoint(&capture)?;
            let replay = match (recovery_replay, recovery_output) {
                (Some(manifest), Some(output)) => {
                    let (report, certificate) =
                        incident_recovery::maintenance::run_with_incoming_evidence(
                            &manifest, &output,
                        )?;
                    if report.capture_manifest_sha256 != verified.state.capture_manifest_sha256 {
                        return Err(
                            "incoming coverage replay uses a different verified checkpoint capture"
                                .into(),
                        );
                    }
                    Some((report, certificate))
                }
                (None, None) => None,
                _ => return Err("recovery replay and output must be provided together".into()),
            };
            let work_root = work
                .parent()
                .ok_or("incoming work index has no parent directory")?;
            let index = serde_json::from_slice(&work_bytes).map_err(|e| e.to_string())?;
            let work = match &replay {
                Some((_, certificate)) => {
                    incident_recovery::incoming_coverage::load_work_with_replay(
                        work_root,
                        index,
                        &policy,
                        certificate,
                    )?
                }
                None => incident_recovery::incoming_coverage::load_work(work_root, index, &policy)?,
            };
            let inventory = serde_json::from_slice(&inventory_bytes).map_err(|e| e.to_string())?;
            let reconciliation =
                serde_json::from_slice(&reconciliation_bytes).map_err(|e| e.to_string())?;
            let report = incident_recovery::incoming_coverage::verify(
                &verified.schema,
                &verified.storage,
                &policy,
                &work,
                incident_recovery::incoming_coverage::Evidence {
                    inventory: &inventory,
                    reconciliation: &reconciliation,
                    inventory_sha256: &hex::encode(Sha256::digest(&inventory_bytes)),
                    finalized_heads_sha256: &hex::encode(Sha256::digest(&heads_bytes)),
                },
            )?;
            let mut result = serde_json::to_value(report).map_err(|e| e.to_string())?;
            for (name, bytes) in [
                ("policySha256", &policy_bytes),
                ("incomingWorkSha256", &work_bytes),
                ("incomingInventorySha256", &inventory_bytes),
                ("incomingReconciliationSha256", &reconciliation_bytes),
                ("externalFinalizedHeadsSha256", &heads_bytes),
            ] {
                result[name] = hex::encode(Sha256::digest(bytes)).into();
            }
            result["captureManifestSha256"] = verified.state.capture_manifest_sha256.into();
            result["checkpointStateRoot"] = verified.state.state_root.into();
            result["checkpointMetadata"] = verified.metadata_witness;
            if let Some((report, _)) = replay {
                result["recoveryReplay"] =
                    serde_json::to_value(report).map_err(|e| e.to_string())?;
            }
            println!(
                "{}",
                serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?
            );
        }
        #[cfg(feature = "bridge-fixture")]
        Command::ProbePostCompleteHashi { manifest, output } => {
            let report = incident_recovery::maintenance::post_complete::run(&manifest, &output)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
            );
        }
        #[cfg(feature = "bridge-fixture")]
        Command::ProbePostCompleteChannel { manifest, output } => {
            let report =
                incident_recovery::maintenance::post_complete_channel::run(&manifest, &output)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
            );
        }
        #[cfg(feature = "bridge-fixture")]
        Command::ProbePostCompleteFamily { manifest, output } => {
            let report = incident_recovery::maintenance::post_complete_channel::families::run(
                &manifest, &output,
            )?;
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
            );
        }
        Command::RehearseMaintenance { manifest, output } => {
            let report = incident_recovery::maintenance::run(&manifest, &output)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
            );
        }
        #[cfg(feature = "bridge-fixture")]
        Command::RehearseTryRuntimeCompanion { manifest } => {
            let report = incident_recovery::maintenance::try_runtime::run(&manifest)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
            );
        }
        Command::RepairStagingPayloads {
            authorization,
            code,
            staging_nonce,
        } => {
            let payload = incident_recovery::repair_staging::payload_files(
                &authorization,
                &code,
                staging_nonce,
            )?;
            println!(
                "{}",
                serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())?
            );
        }
        Command::RehearseStorageSemantics { manifest, output } => {
            let report =
                incident_recovery::maintenance::run_with_storage_semantics(&manifest, &output)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
            );
        }
        #[cfg(feature = "bridge-fixture")]
        Command::RehearsePublicStorageSemantics { manifest, output } => {
            let report =
                incident_recovery::maintenance::run_public_storage_semantics(&manifest, &output)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
            );
        }
        #[cfg(all(unix, feature = "runtime-work-types"))]
        Command::RehearseMaintenanceCrashes {
            manifest,
            output,
            require_prior_incoming_evidence,
        } => {
            let report = incident_recovery::maintenance::process_death::run(
                &manifest,
                &output,
                require_prior_incoming_evidence,
            )?;
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
            );
        }
        #[cfg(all(unix, feature = "runtime-work-types"))]
        Command::MaintenanceCrashWorker { job } => {
            incident_recovery::maintenance::process_death::worker(&job)?;
        }
        Command::VerifyBurnHistory { manifest, output } => {
            let report = incident_recovery::burn_history::verify(&manifest, &output)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
            );
        }
        Command::BurnProofSession { capture } => {
            incident_recovery::burn_history::proof_session(&capture)?;
        }
        Command::RehearseBridgeCancellations { capture, plan } => {
            if fs::metadata(&plan).map_err(|e| e.to_string())?.len() > 64 * 1024 * 1024 {
                return Err("cancellation evidence exceeds 64 MiB".into());
            }
            let bytes = fs::read(plan).map_err(|e| e.to_string())?;
            let verified = incident_recovery::verify_checkpoint(&capture)?;
            let report = incident_recovery::rehearsal::bridge_cancellations(verified, &bytes)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
            );
        }
        Command::EmitFinalPolicy { policy } => {
            use codec::DecodeWithMemLimit;
            if fs::metadata(&policy).map_err(|e| e.to_string())?.len()
                > common::recovery::MAX_INHERENT_BYTES as u64
            {
                return Err("policy exceeds bounded release transport".into());
            }
            let bytes = fs::read(policy).map_err(|e| e.to_string())?;
            if bytes.len() > common::recovery::MAX_INHERENT_BYTES {
                return Err("policy exceeds bounded release transport".into());
            }
            let mut input = bytes.as_slice();
            let policy = common::recovery::RecoveryPolicy::decode_with_mem_limit(
                &mut input,
                common::recovery::MAX_INHERENT_BYTES * 3,
            )
            .map_err(|e| e.to_string())?;
            if !input.is_empty() {
                return Err("trailing bytes in release policy".into());
            }
            print!(
                "{}",
                incident_recovery::composer::final_runtime_source(&policy)?
            );
        }
        Command::ReplayHistory { manifest } => {
            println!(
                "{}",
                serde_json::to_string_pretty(&incident_recovery::replay::replay(&manifest)?)
                    .map_err(|e| e.to_string())?
            );
        }
        Command::ComposeStorageWork(args) => {
            let ComposeStorageWorkArgs {
                #[cfg(feature = "runtime-work-types")]
                bridge_history,
                #[cfg(feature = "runtime-work-types")]
                bridge_signing,
                #[cfg(feature = "runtime-work-types")]
                bridge_hashi,
                elections,
                offences,
                capture,
                csv,
                proofs,
                inventory,
                economic,
                attacker_coverage_output,
                attacker_interest_output,
                semantic,
                governance,
                referral,
                staking_controls,
                staking_roles,
                operators_scale,
                staking_history,
                democracy,
                scheduler,
                council,
                evidence_commitment,
                chain_spec_id,
                release_nonce,
            } = *args;
            // Authenticate the registry against executed checkpoint Wasm,
            // not merely a caller-provided metadata hash or success flag.
            let verified = incident_recovery::verify_checkpoint(&capture)?;
            let admitted = trusted_mapping_input(&verified, &csv, &inventory, &semantic)?;
            let trusted_report = admitted.report;
            let trusted = admitted.trusted;
            let map = trusted.replacements().to_vec();
            let recovered_council = trusted.council().to_vec();
            let (storage, schema) = (verified.storage, verified.schema);
            require_bridge_control_features(&storage, cfg!(feature = "runtime-work-types"))?;
            // Retained for old command lines only. These records do not supply
            // authorization under the explicit trusted-CSV release policy.
            if let Some(path) = proofs {
                let _: Vec<incident_recovery::OwnershipProof> = read_json(path)?;
            }
            let mut options = admitted.options;
            let mut economic_plan: incident_recovery::economics::EconomicPlan =
                read_json(economic)?;
            economic_plan.protect_replacements(&map)?;
            incident_recovery::economics::require_attacker_watch(&economic_plan, &mut options)?;
            let semantic_plan = admitted.semantic;
            let mut semantics = incident_recovery::substitution_semantics::compile_trusted(
                &schema,
                &storage,
                &trusted,
                &semantic_plan,
            )?;
            let iroha_pending = incident_recovery::iroha_pending::compile(&schema, &storage, &map)?;
            semantics.edits.extend(iroha_pending.edits.clone());
            semantics.groups.extend(iroha_pending.groups.clone());
            semantics
                .resolutions
                .extend(iroha_pending.resolutions.clone());
            semantics
                .preservation_witnesses
                .extend(iroha_pending.witnesses.clone());
            let session_controls =
                incident_recovery::session_controls::compile(&schema, &storage, &map)?;
            semantics.edits.extend(session_controls.edits.clone());
            semantics.groups.extend(session_controls.groups.clone());
            semantics
                .resolutions
                .extend(session_controls.resolutions.clone());
            semantics
                .preservation_witnesses
                .extend(session_controls.witnesses.clone());
            let permissions_controls = incident_recovery::permissions_controls::compile(
                &schema, &storage, &map, &options,
            )?;
            semantics.edits.extend(permissions_controls.edits.clone());
            semantics.groups.extend(permissions_controls.groups.clone());
            semantics
                .resolutions
                .extend(permissions_controls.resolutions.clone());
            semantics
                .preservation_witnesses
                .extend(permissions_controls.witnesses.clone());
            let preimage_ownership =
                incident_recovery::preimage_ownership::compile(&schema, &storage, &map)?;
            semantics.edits.extend(preimage_ownership.edits.clone());
            semantics.groups.extend(preimage_ownership.groups.clone());
            semantics
                .resolutions
                .extend(preimage_ownership.resolutions.clone());
            semantics
                .preservation_witnesses
                .extend(preimage_ownership.witnesses.clone());
            let governance = governance
                .map(|path| {
                    let review = read_json::<incident_recovery::governance::Review>(path)?;
                    incident_recovery::governance::compile(&schema, &storage, &map, &review)
                })
                .transpose()?;
            if let Some(compiled) = &governance {
                semantics.edits.extend(compiled.edits.clone());
                semantics.groups.extend(compiled.groups.clone());
                semantics.resolutions.extend(compiled.resolutions.clone());
                semantics
                    .preservation_witnesses
                    .extend(compiled.witnesses.clone());
            }
            let referral = referral
                .map(|path| {
                    let review = read_json::<incident_recovery::referral::Review>(path)?;
                    incident_recovery::referral::compile(&schema, &storage, &map, &review)
                })
                .transpose()?;
            if let Some(compiled) = &referral {
                semantics.edits.extend(compiled.edits.clone());
                semantics.groups.extend(compiled.groups.clone());
                semantics.resolutions.extend(compiled.resolutions.clone());
                semantics
                    .preservation_witnesses
                    .extend(compiled.witnesses.clone());
            }
            let staking_controls = staking_controls
                .map(|path| {
                    let review = read_json::<incident_recovery::staking_controls::Review>(path)?;
                    incident_recovery::staking_controls::compile(&schema, &storage, &map, &review)
                })
                .transpose()?;
            if let Some(compiled) = &staking_controls {
                semantics.edits.extend(compiled.edits.clone());
                semantics.groups.extend(compiled.groups.clone());
                semantics.resolutions.extend(compiled.resolutions.clone());
                semantics
                    .preservation_witnesses
                    .extend(compiled.witnesses.clone());
            }
            let staking_roles = staking_roles
                .map(|path| {
                    let review = read_json::<incident_recovery::staking_roles::Review>(path)?;
                    let operators = read_operators(
                        operators_scale
                            .as_ref()
                            .ok_or("staking roles require operators-scale")?,
                    )?;
                    incident_recovery::staking_roles::compile(
                        &schema, &storage, &map, &operators, &review,
                    )
                })
                .transpose()?;
            if let Some(compiled) = &staking_roles {
                semantics.edits.extend(compiled.edits.clone());
                semantics.groups.extend(compiled.groups.clone());
                semantics.resolutions.extend(compiled.resolutions.clone());
                semantics
                    .preservation_witnesses
                    .extend(compiled.witnesses.clone());
            }
            let staking_history = staking_history
                .map(|path| {
                    let review = read_json::<incident_recovery::staking_history::Review>(path)?;
                    incident_recovery::staking_history::compile(&schema, &storage, &map, &review)
                })
                .transpose()?;
            if let Some(compiled) = &staking_history {
                semantics.edits.extend(compiled.edits.clone());
                semantics.groups.extend(compiled.groups.clone());
                semantics.resolutions.extend(compiled.resolutions.clone());
                semantics
                    .preservation_witnesses
                    .extend(compiled.witnesses.clone());
            }
            let democracy = democracy
                .map(|path| {
                    let review = read_json::<incident_recovery::democracy::Review>(path)?;
                    incident_recovery::democracy::compile(&schema, &storage, &map, &review)
                })
                .transpose()?;
            if let Some(compiled) = &democracy {
                semantics.edits.extend(compiled.edits.clone());
                semantics.groups.extend(compiled.groups.clone());
                semantics.resolutions.extend(compiled.resolutions.clone());
                semantics
                    .preservation_witnesses
                    .extend(compiled.witnesses.clone());
            }
            let scheduler = scheduler
                .map(|path| {
                    let review = read_json::<incident_recovery::scheduler_controls::Review>(path)?;
                    incident_recovery::scheduler_controls::compile(&schema, &storage, &map, &review)
                })
                .transpose()?;
            if let Some(compiled) = &scheduler {
                semantics.edits.extend(compiled.edits.clone());
                semantics.groups.extend(compiled.groups.clone());
                semantics.resolutions.extend(compiled.resolutions.clone());
                semantics
                    .preservation_witnesses
                    .extend(compiled.witnesses.clone());
            } else if schema
                .descriptors
                .iter()
                .filter(|d| d.name == "Scheduler.Agenda")
                .any(|d| storage.top.keys().any(|key| key.starts_with(&d.prefix)))
            {
                return Err("retained Root scheduler authorizations require their complete typed cancellation review".into());
            }
            let elections = elections
                .map(|path| {
                    let review = read_json::<incident_recovery::elections::Review>(path)?;
                    incident_recovery::elections::compile(&schema, &storage, &map, &review)
                })
                .transpose()?;
            if let Some(compiled) = &elections {
                semantics.edits.extend(compiled.edits.clone());
                semantics.groups.extend(compiled.groups.clone());
                semantics.resolutions.extend(compiled.resolutions.clone());
                semantics
                    .preservation_witnesses
                    .extend(compiled.witnesses.clone());
                semantics
                    .prefix_witnesses
                    .extend(compiled.prefix_witnesses.clone());
            }
            let offences = offences
                .map(|path| {
                    let review = read_json::<incident_recovery::offences::Review>(path)?;
                    incident_recovery::offences::compile(&schema, &storage, &map, &review)
                })
                .transpose()?;
            if let Some(compiled) = &offences {
                semantics.edits.extend(compiled.edits.clone());
                semantics.groups.extend(compiled.groups.clone());
                semantics.resolutions.extend(compiled.resolutions.clone());
                semantics
                    .preservation_witnesses
                    .extend(compiled.witnesses.clone());
                semantics
                    .prefix_witnesses
                    .extend(compiled.prefix_witnesses.clone());
            }
            #[cfg(feature = "runtime-work-types")]
            let bridge_history = bridge_history
                .map(|path| {
                    let review = read_json::<incident_recovery::bridge_history::Review>(path)?;
                    if !review.cancel_all_terminal_outgoing || !review.quarantine_all_nonterminal_outgoing {
                        return Err(
                            "release work requires complete terminal and nonterminal outgoing cancellation".into(),
                        );
                    }
                    incident_recovery::bridge_history::compile(&schema, &storage, &map, &review)
                })
                .transpose()?;
            #[cfg(feature = "runtime-work-types")]
            if let Some(compiled) = &bridge_history {
                semantics.edits.extend(compiled.edits.clone());
                semantics.groups.extend(compiled.groups.clone());
                semantics.resolutions.extend(compiled.resolutions.clone());
                semantics
                    .preservation_witnesses
                    .extend(compiled.witnesses.clone());
            } else if schema
                .descriptors
                .iter()
                .filter(|d| {
                    ["BridgeProxy.Senders", "BridgeProxy.Transactions"].contains(&d.name.as_str())
                })
                .any(|d| {
                    storage
                        .top
                        .range(d.prefix.clone()..)
                        .next()
                        .is_some_and(|(key, _)| key.starts_with(&d.prefix))
                })
            {
                return Err("checkpoint bridge ownership and outgoing liabilities require their complete cancellation review".into());
            }
            #[cfg(feature = "runtime-work-types")]
            let bridge_signing = bridge_signing
                .map(|path| {
                    let review = read_json::<incident_recovery::bridge_signing::Review>(path)?;
                    incident_recovery::bridge_signing::compile(&schema, &storage, &review)
                })
                .transpose()?;
            #[cfg(feature = "runtime-work-types")]
            if let Some(compiled) = &bridge_signing {
                semantics.groups.extend(compiled.groups.clone());
                semantics
                    .preservation_witnesses
                    .extend(compiled.witnesses.clone());
                semantics
                    .prefix_witnesses
                    .extend(compiled.prefix_witnesses.clone());
            } else if schema
                .descriptors
                .iter()
                .filter(|d| incident_recovery::bridge_signing::ITEMS.contains(&d.name.as_str()))
                .any(|d| {
                    storage
                        .top
                        .range(d.prefix.clone()..)
                        .next()
                        .is_some_and(|(key, _)| key.starts_with(&d.prefix))
                })
            {
                return Err("checkpoint bridge signing authorizations require their complete cancellation review".into());
            }
            #[cfg(feature = "runtime-work-types")]
            let bridge_hashi = bridge_hashi
                .map(|path| {
                    let review = read_json::<incident_recovery::bridge_hashi::Review>(path)?;
                    incident_recovery::bridge_hashi::compile(&schema, &storage, &map, &review)
                })
                .transpose()?;
            #[cfg(feature = "runtime-work-types")]
            if let Some(compiled) = &bridge_hashi {
                semantics.edits.extend(compiled.edits.clone());
                semantics.groups.extend(compiled.groups.clone());
                semantics.resolutions.extend(compiled.resolutions.clone());
                semantics
                    .preservation_witnesses
                    .extend(compiled.witnesses.clone());
                semantics
                    .prefix_witnesses
                    .extend(compiled.prefix_witnesses.clone());
            } else if schema
                .descriptors
                .iter()
                .filter(|d| incident_recovery::bridge_hashi::ITEMS.contains(&d.name.as_str()))
                .any(|d| {
                    storage
                        .top
                        .range(d.prefix.clone()..)
                        .next()
                        .is_some_and(|(key, _)| key.starts_with(&d.prefix))
                })
            {
                return Err("native HASHI outgoing requests and authorizations require their complete cancellation review".into());
            }
            let inventory = incident_recovery::inventory::inventory_with_resolutions(
                &schema,
                &storage,
                &map.iter()
                    .map(|m| (*m.old.as_ref(), *m.new.as_ref()))
                    .collect(),
                &options,
                &semantics.resolutions,
                &semantics.edits,
            )?;
            if !inventory.all_entries_classified
                || !inventory.semantic_dispositions_complete
                || !inventory.unused_destinations_verified
                || !inventory.used_destinations.is_empty()
            {
                return Err("incomplete typed inventory or unresolved semantic dispositions; no work policy emitted".into());
            }
            if let Some(path) = attacker_interest_output {
                use std::io::Write;
                let assessment = incident_recovery::economics::assess_attacker_interests(
                    &economic_plan,
                    &inventory,
                )?;
                let report = serde_json::json!({
                    "schema": "sora-nexus-checkpoint-attacker-interest-assessment-v1",
                    "checkpoint": verified.state,
                    "metadataWitness": verified.metadata_witness,
                    "assessment": assessment,
                    "compositionSucceeded": false,
                    "scope": "Diagnostic after complete native inventory with source-proven adapter resolutions. Later economic/compilation checks remain mandatory; this artifact cannot certify composition success.",
                    "productionReady": false,
                });
                let bytes = serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?;
                let mut file = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(path)
                    .map_err(|e| e.to_string())?;
                file.write_all(&bytes).map_err(|e| e.to_string())?;
                file.write_all(b"\n").map_err(|e| e.to_string())?;
                file.sync_all().map_err(|e| e.to_string())?;
            }
            let attacker_coverage = incident_recovery::economics::validate_attacker_coverage(
                &economic_plan,
                &inventory,
            )?;
            let economics =
                incident_recovery::economics::compile(&schema, &storage, &economic_plan)?;
            if !semantics.report.blockers.is_empty() || !economics.report.blockers.is_empty() {
                return Err(format!(
                    "adapter blockers: {:?}; {:?}",
                    semantics.report.blockers, economics.report.blockers
                ));
            }
            let mut groups = incident_recovery::composer::mapping_groups(&inventory.candidates)?;
            groups.append(&mut semantics.groups);
            // Preserve the coupled balance/issuance changes in one transaction.
            // Oversized economic groups require an explicit adapter partition.
            if !economics.edits.is_empty() {
                groups.push(economics.edits);
            }
            semantics.verify_membership_groups(&groups)?;
            session_controls.verify_groups(&groups, &map)?;
            permissions_controls.verify_groups(&groups, &map)?;
            preimage_ownership.verify_groups(&groups, &map)?;
            iroha_pending.verify_groups(&groups, &map)?;
            #[cfg(feature = "runtime-work-types")]
            if let Some(compiled) = &bridge_history {
                compiled.verify_groups(&groups, &map)?;
            }
            #[cfg(feature = "runtime-work-types")]
            if let Some(compiled) = &bridge_signing {
                compiled.verify_groups(&groups)?;
            }
            #[cfg(feature = "runtime-work-types")]
            if let Some(compiled) = &bridge_hashi {
                compiled.verify_groups(&groups, &map)?;
            }
            if let Some(compiled) = &elections {
                compiled.verify_groups(&groups, &map)?;
            }
            if let Some(compiled) = &offences {
                compiled.verify_groups(&groups, &map)?;
            }
            if let Some(compiled) = &staking_controls {
                compiled.verify_backing_moves(&groups)?;
            }
            if let Some(compiled) = &staking_history {
                compiled.verify_groups(&groups, &map)?;
            }
            if let Some(compiled) = &staking_roles {
                compiled.verify_groups(&groups, &map)?;
            }
            if let Some(compiled) = &democracy {
                compiled.verify_backing_moves(&groups)?;
            }
            if let Some(compiled) = &scheduler {
                compiled.verify_preservation(&groups)?;
            }
            let bags = incident_recovery::bags_list::verify_groups_with_certificate(
                &schema,
                &storage,
                &groups,
                &map,
                staking_roles
                    .as_ref()
                    .map(|compiled| compiled.graph_certificate()),
            )?;
            if let Some(path) = council {
                let mut supplied = read_json::<Vec<String>>(path)?
                    .iter()
                    .map(|s| incident_recovery::account(s))
                    .collect::<incident_recovery::Result<Vec<_>>>()?;
                supplied.sort();
                if supplied != recovered_council {
                    return Err(
                        "supplied council differs from checkpoint-derived CSV-remapped council"
                            .into(),
                    );
                }
            }
            let council = recovered_council;
            let projection = incident_recovery::release_checks::check_with_governance_preservation(
                &schema,
                &storage,
                &groups,
                &inventory.candidates,
                &council,
                economics.terminal_audit,
                incident_recovery::release_checks::GovernanceReview {
                    replacements: &map,
                    preservation: semantic_plan.governance_preservation.as_ref(),
                    transformation: governance.as_ref(),
                },
            )?;
            let mut witnesses: Vec<_> = projection
                .governance_witnesses
                .into_iter()
                .map(|(witness, _)| witness)
                .collect();
            witnesses.extend(semantics.preservation_witnesses);
            witnesses.extend(bags.witnesses);
            #[cfg(feature = "runtime-work-types")]
            let incoming_preservation =
                incident_recovery::incoming_coverage::retained_storage_witnesses(
                    &schema, &storage,
                )?;
            #[cfg(feature = "runtime-work-types")]
            witnesses.extend(incoming_preservation.witnesses);
            let commitment = incident_recovery::hex_bytes(&evidence_commitment)?
                .try_into()
                .map_err(|_| "evidence commitment must be 32 bytes")?;
            let mut empty_prefixes = staking_history
                .as_ref()
                .map(|compiled| compiled.prefix_witnesses.clone())
                .unwrap_or_default();
            if let Some(compiled) = &scheduler {
                empty_prefixes.extend(compiled.empty_prefixes.clone());
            }
            empty_prefixes.extend(semantics.prefix_witnesses);
            let result = incident_recovery::composer::compose_with_audits(
                &storage,
                map,
                council,
                incident_recovery::composer::PolicyIdentity {
                    evidence_commitment: commitment,
                    authorization_domain: common::recovery::RecoveryDomain {
                        chain_spec_id: chain_spec_id.into_bytes(),
                        release_nonce: incident_recovery::hex_bytes(&release_nonce)?
                            .try_into()
                            .map_err(|_| "release nonce must be 32 bytes")?,
                    },
                },
                groups,
                incident_recovery::composer::AuditRequests {
                    keys: witnesses,
                    empty_prefixes,
                },
                projection.terminal,
            )?;
            let mut report = serde_json::to_value(&result.report).map_err(|e| e.to_string())?;
            report["trustedMapping"] = trusted_report;
            report["authorizationPolicy"] =
                incident_recovery::trusted_mapping::AUTHORIZATION_POLICY.into();
            report["legacyOwnershipProofsUsed"] = false.into();
            #[cfg(feature = "runtime-work-types")]
            {
                report["incomingCheckpointPreservation"] =
                    serde_json::to_value(incoming_preservation.report)
                        .map_err(|e| e.to_string())?;
            }
            report["attackerCoverage"] =
                serde_json::to_value(&attacker_coverage).map_err(|e| e.to_string())?;
            report["projectedStateChecks"] =
                serde_json::to_value(projection.report).map_err(|e| e.to_string())?;
            report["bagsListGraph"] =
                serde_json::to_value(bags.report).map_err(|e| e.to_string())?;
            #[cfg(feature = "runtime-work-types")]
            if let Some(compiled) = bridge_history {
                report["bridgeHistory"] =
                    serde_json::to_value(compiled.report).map_err(|e| e.to_string())?;
            }
            #[cfg(feature = "runtime-work-types")]
            if let Some(compiled) = bridge_signing {
                report["bridgeSigning"] =
                    serde_json::to_value(compiled.report).map_err(|e| e.to_string())?;
            }
            #[cfg(feature = "runtime-work-types")]
            if let Some(compiled) = bridge_hashi {
                report["nativeHashi"] =
                    serde_json::to_value(compiled.report).map_err(|e| e.to_string())?;
            }
            if let Some(compiled) = elections {
                report["elections"] =
                    serde_json::to_value(compiled.report).map_err(|e| e.to_string())?;
            }
            if let Some(compiled) = offences {
                report["offences"] =
                    serde_json::to_value(compiled.report).map_err(|e| e.to_string())?;
            }
            if let Some(governance) = governance {
                report["governanceSubstitution"] =
                    serde_json::to_value(governance.report).map_err(|e| e.to_string())?;
            }
            if let Some(referral) = referral {
                report["referralSuspension"] =
                    serde_json::to_value(referral.report).map_err(|e| e.to_string())?;
            }
            if let Some(staking) = staking_controls {
                report["stakingControls"] =
                    serde_json::to_value(staking.report).map_err(|e| e.to_string())?;
            }
            report["irohaPendingApprovers"] =
                serde_json::to_value(iroha_pending.report).map_err(|e| e.to_string())?;
            report["sessionControls"] =
                serde_json::to_value(session_controls.report).map_err(|e| e.to_string())?;
            report["permissionsControls"] =
                serde_json::to_value(permissions_controls.report).map_err(|e| e.to_string())?;
            report["preimageOwnership"] =
                serde_json::to_value(preimage_ownership.report).map_err(|e| e.to_string())?;
            if let Some(staking) = staking_roles {
                report["stakingRoles"] =
                    serde_json::to_value(staking.report).map_err(|e| e.to_string())?;
            }
            if let Some(staking) = staking_history {
                report["stakingEconomicHistory"] =
                    serde_json::to_value(staking.report).map_err(|e| e.to_string())?;
            }
            if let Some(democracy) = democracy {
                report["democracyDelegation"] =
                    serde_json::to_value(democracy.report).map_err(|e| e.to_string())?;
            }
            if let Some(scheduler) = scheduler {
                report["schedulerCancellation"] =
                    serde_json::to_value(scheduler.report).map_err(|e| e.to_string())?;
            }
            if let Some(path) = attacker_coverage_output {
                use std::io::Write;
                let bytes =
                    serde_json::to_vec_pretty(&attacker_coverage).map_err(|e| e.to_string())?;
                let mut file = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(path)
                    .map_err(|e| e.to_string())?;
                file.write_all(&bytes).map_err(|e| e.to_string())?;
                file.write_all(b"\n").map_err(|e| e.to_string())?;
                file.sync_all().map_err(|e| e.to_string())?;
            }
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
            );
        }
        Command::VerifyMetadata { capture } => {
            println!(
                "{}",
                serde_json::to_string_pretty(&incident_recovery::verify_metadata(&capture)?)
                    .map_err(|e| e.to_string())?
            );
        }
        Command::EconomicCandidates { capture, plan } => {
            let checkpoint = incident_recovery::verify_checkpoint(&capture)?;
            let plan = read_json(plan)?;
            let result = incident_recovery::economics::compile(
                &checkpoint.schema,
                &checkpoint.storage,
                &plan,
            )?;
            println!(
                "{}",
                serde_json::to_string_pretty(&result.report).map_err(|e| e.to_string())?
            );
        }
        Command::SemanticCandidates {
            capture,
            csv,
            proofs,
            plan,
        } => {
            let checkpoint = incident_recovery::verify_checkpoint(&capture)?;
            let map = read_mapping(&csv)?;
            let proofs: Vec<incident_recovery::OwnershipProof> = read_json(proofs)?;
            let plan = read_json(plan)?;
            let result = incident_recovery::substitution_semantics::compile(
                &checkpoint.schema,
                &checkpoint.storage,
                &map,
                &proofs,
                &plan,
            )?;
            println!(
                "{}",
                serde_json::to_string_pretty(&result.report).map_err(|e| e.to_string())?
            );
        }
        Command::Inventory {
            capture,
            csv,
            dispositions,
        } => {
            let checkpoint = incident_recovery::verify_checkpoint(&capture)?;
            let map = csv
                .map(|path| read_mapping(&path))
                .transpose()?
                .unwrap_or_default();
            let mappings = map
                .iter()
                .map(|m| (*m.old.as_ref(), *m.new.as_ref()))
                .collect();
            let options = dispositions.map(read_json).transpose()?.unwrap_or_default();
            let report = incident_recovery::inventory::inventory(
                &checkpoint.schema,
                &checkpoint.storage,
                &mappings,
                &options,
            )?;
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
            );
        }
        Command::VerifyCapture { capture } => {
            let (report, _) = incident_recovery::verify_capture(&capture)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
            );
        }
        Command::MappingPayloads { csv } => {
            let map = read_mapping(&csv)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&incident_recovery::ownership_payloads(&map))
                    .map_err(|e| e.to_string())?
            );
        }
        #[cfg(feature = "runtime-work-types")]
        Command::DeriveCheckpointConsensus { capture } => {
            let checkpoint = incident_recovery::verify_checkpoint(&capture)?;
            let mut report = incident_recovery::checkpoint_consensus::report(&checkpoint.storage)?;
            report["checkpointStateRoot"] = checkpoint.state.state_root.into();
            report["captureManifestSha256"] = checkpoint.state.capture_manifest_sha256.into();
            report["checkpointMetadata"] = checkpoint.metadata_witness;
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
            );
        }
        Command::VerifyTrustedMapping {
            csv,
            capture,
            inventory,
            semantic,
        } => {
            let checkpoint = incident_recovery::verify_checkpoint(&capture)?;
            let result = trusted_mapping_input(&checkpoint, &csv, &inventory, &semantic)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&result.report).map_err(|e| e.to_string())?
            );
        }
        Command::VerifyOwnership {
            csv,
            proofs,
            capture,
            semantic,
            derived_inventory,
        } => {
            println!(
                "{}",
                verify_ownership_command(
                    &csv,
                    &proofs,
                    capture.as_deref(),
                    semantic.as_deref(),
                    derived_inventory.as_deref(),
                )?
            );
        }
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Recovery evidence rejected: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod feature_tests {
    use super::*;

    #[test]
    fn trusted_mapping_cli_requires_checkpoint_inputs_not_ownership_signatures() {
        assert!(Cli::try_parse_from([
            "incident-recovery",
            "verify-trusted-mapping",
            "--csv",
            "mapping.csv",
            "--capture",
            "capture",
            "--inventory",
            "inventory.json",
            "--semantic",
            "semantic.json",
        ])
        .is_ok());
        assert!(Cli::try_parse_from([
            "incident-recovery",
            "verify-trusted-mapping",
            "--csv",
            "mapping.csv",
        ])
        .is_err());
        assert!(Cli::try_parse_from([
            "incident-recovery",
            "compose-storage-work",
            "--csv",
            "mapping.csv",
            "--capture",
            "capture",
            "--inventory",
            "inventory.json",
            "--semantic",
            "semantic.json",
            "--economic",
            "economic.json",
            "--evidence-commitment",
            "0x01",
            "--chain-spec-id",
            "sora-nexus-recovery",
            "--release-nonce",
            "0x02",
        ])
        .is_ok());
        assert!(Cli::try_parse_from([
            "incident-recovery",
            "verify-ownership",
            "--csv",
            "mapping.csv",
        ])
        .is_err());
    }

    #[test]
    fn ownership_cli_requires_all_authenticated_derived_inputs_together() {
        for extra in [
            vec!["--capture", "capture"],
            vec!["--semantic", "semantic.json"],
            vec!["--derived-inventory", "derived.json"],
        ] {
            let mut args = vec![
                "incident-recovery",
                "verify-ownership",
                "--csv",
                "mapping.csv",
                "--proofs",
                "proofs.json",
            ];
            args.extend(extra);
            assert!(Cli::try_parse_from(args).is_err());
        }
        assert!(Cli::try_parse_from([
            "incident-recovery",
            "verify-ownership",
            "--csv",
            "mapping.csv",
            "--proofs",
            "proofs.json"
        ])
        .is_ok());
        assert!(Cli::try_parse_from([
            "incident-recovery",
            "verify-ownership",
            "--csv",
            "mapping.csv",
            "--proofs",
            "proofs.json",
            "--capture",
            "capture",
            "--semantic",
            "semantic.json",
            "--derived-inventory",
            "derived.json"
        ])
        .is_ok());
    }

    #[test]
    fn ownership_receipt_binds_exact_inputs_and_direct_mode_rejects_old_signer() {
        use codec::Encode;
        use common::recovery::{replacement_ownership_message, AccountReplacement};
        use sha2::{Digest, Sha256};
        use sp_core::{ed25519, Pair};
        let old = ed25519::Pair::from_seed(&[1; 32]);
        let new = ed25519::Pair::from_seed(&[2; 32]);
        let entry = AccountReplacement {
            old: old.public().0.into(),
            new: new.public().0.into(),
        };
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "sora-nexus-control-proof-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&directory).unwrap();
        let csv = directory.join("mapping.csv");
        let proofs = directory.join("proofs.json");
        fs::write(
            &csv,
            format!(
                "old account address,new account address\n0x{},0x{}\n",
                hex::encode(old.public().0),
                hex::encode(new.public().0)
            ),
        )
        .unwrap();
        let proof = |key: &ed25519::Pair| {
            serde_json::json!([{
                "old": format!("0x{}", hex::encode(old.public().0)),
                "new": format!("0x{}", hex::encode(new.public().0)),
                "signature": format!("0x{}", hex::encode(sp_runtime::MultiSignature::from(key.sign(&replacement_ownership_message(&entry))).encode()))
            }])
        };
        fs::write(&proofs, proof(&new).to_string()).unwrap();
        let report = verify_ownership_command(&csv, &proofs, None, None, None).unwrap();
        assert_eq!(report["ownershipVerified"], true);
        assert_eq!(report["derivedControlVerified"], false);
        assert_eq!(
            report["mappingCsvSha256"],
            hex::encode(Sha256::digest(fs::read(&csv).unwrap()))
        );
        assert_eq!(
            report["ownershipProofsSha256"],
            hex::encode(Sha256::digest(fs::read(&proofs).unwrap()))
        );
        fs::write(&proofs, proof(&old).to_string()).unwrap();
        assert!(verify_ownership_command(&csv, &proofs, None, None, None).is_err());
        fs::write(&proofs, proof(&new).to_string()).unwrap();
        let semantic = directory.join("semantic.json");
        let derived = directory.join("derived.json");
        fs::write(&semantic, r#"{"derivedAccounts":[]}"#).unwrap();
        fs::write(&derived, serde_json::json!([{
            "old": format!("0x{}", hex::encode(old.public().0)), "new": format!("0x{}", hex::encode(new.public().0)),
            "derivation": {"kind":"utility", "controller":format!("0x{}", hex::encode(old.public().0)), "index":0}
        }]).to_string()).unwrap();
        assert!(verify_ownership_command(
            &csv,
            &proofs,
            Some(&directory),
            Some(&semantic),
            Some(&derived)
        )
        .unwrap_err()
        .contains("differs from semantic plan"));
        fs::write(&derived, "[]").unwrap();
        // Even a valid direct signature and consistent graph cannot waive the
        // full checkpoint reconstruction and checkpoint Wasm metadata execution.
        assert!(verify_ownership_command(
            &csv,
            &proofs,
            Some(&directory),
            Some(&semantic),
            Some(&derived)
        )
        .is_err());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn reduced_build_cannot_bypass_populated_bridge_control_gates() {
        let mut storage = sp_core::storage::Storage::default();
        assert!(require_bridge_control_features(&storage, false).is_ok());
        for (pallet, item) in [
            ("BridgeProxy", "Transactions"),
            ("BridgeDataSigner", "Approvals"),
            ("BridgeDataSigner", "PendingApprovals"),
            ("SubstrateBridgeOutboundChannel", "LatestCommitment"),
            ("BridgeOutboundChannel", "MessageQueues"),
            ("EthBridge", "Requests"),
            ("EthBridge", "RequestApprovals"),
            ("EthBridge", "AccountRequests"),
        ] {
            let mut key = [
                sp_io::hashing::twox_128(pallet.as_bytes()),
                sp_io::hashing::twox_128(item.as_bytes()),
            ]
            .concat();
            // Even an empty-valued row is present; decoding cannot waive it.
            key.extend_from_slice(&[0; 32]);
            storage.top.insert(key, vec![]);
            assert!(require_bridge_control_features(&storage, false)
                .unwrap_err()
                .contains("runtime-work-types"));
            assert!(require_bridge_control_features(&storage, true).is_ok());
            storage.top.clear();
        }
        storage.top.insert(b"unrelated".to_vec(), vec![1]);
        assert!(require_bridge_control_features(&storage, false).is_ok());
    }
}
