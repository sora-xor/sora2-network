//! Admission and import guards for the manifest-bound recovery branch.

use codec::{Decode, DecodeAll, Encode};
use common::recovery::{
    parse_lineage_marker, CHECKPOINT_HASH, CHECKPOINT_NUMBER, CHECKPOINT_STATE_ROOT, GENESIS_HASH,
    POLICY_STORAGE_KEY,
};
use framenode_chain_spec::recovery::{RecoveryConfig, CHAIN_ID, FORK_ID, PROTOCOL_ID};
use framenode_runtime::opaque::Block;
use sc_client_api::{AuxStore, StorageProvider};
use sc_consensus::{BlockCheckParams, BlockImport, BlockImportParams, ImportResult};
use sc_executor::RuntimeVersionOf;
use sc_finality_grandpa::{AuthoritySetHardFork, SharedAuthoritySet};
use sc_network_sync::strategy::warp::{
    EncodedProof, VerificationResult, Verifier, WarpSyncProvider,
};
use sc_service::{Configuration, Error as ServiceError};
use sp_api::{Core as _, ProvideRuntimeApi};
use sp_blockchain::HeaderBackend;
use sp_core::traits::{RuntimeCode, WrappedRuntimeCode};
use sp_core::{storage::StorageKey, ByteArray, H256};
use sp_runtime::traits::{Block as BlockT, Header as HeaderT};
use sp_runtime::DigestItem;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const CHECKPOINT_IMPORTED_KEY: &[u8] = b"sora-nexus/recovery/checkpoint-imported/v1";
const MAX_WARP_PROOF_BYTES: usize = 8 * 1024 * 1024;

/// This release must never accidentally join the abandoned production network.
/// Explicit offline historical tools remain available without the new spec.
pub fn validate_startup_network(
    genesis: H256,
    has_recovery: bool,
    offline: bool,
) -> Result<(), String> {
    if genesis == H256::from(GENESIS_HASH) && !has_recovery && !offline {
        return Err("node 4.8.13 requires the approved recovery chain specification for SORA Nexus mainnet; ordinary mainnet is available only to explicit offline historical commands".into());
    }
    Ok(())
}

/// Public, runtime-authenticated work. Files are selected by exact parent hash,
/// so a delayed/restarted author never consumes work intended for another state.
pub struct RecoveryInherentProvider(Option<Vec<u8>>);

impl RecoveryInherentProvider {
    /// Read the bounded work envelope for the exact authoring parent.
    pub fn for_parent(
        client: &crate::service::FullClient,
        parent: H256,
        recovery: Option<&RecoveryConfig>,
        directory: Option<&Path>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        use common::recovery::{RecoveryAction, RecoveryInherent, RecoveryPhase, RecoveryStatus};
        let Some(recovery) = recovery else {
            return Ok(Self(None));
        };
        let header = client
            .header(parent)?
            .ok_or("unknown recovery authoring parent")?;
        validate_authoring_parent(
            &header,
            parent,
            recovery,
            client.get_aux(CHECKPOINT_IMPORTED_KEY)?.as_deref(),
        )?;
        if *header.number() > CHECKPOINT_NUMBER {
            let encoded = client
                .storage(
                    parent,
                    &StorageKey(common::recovery::STATUS_STORAGE_KEY.to_vec()),
                )?
                .ok_or("missing recovery status at authoring parent")?;
            let status = RecoveryStatus::decode_all(&mut encoded.0.as_slice())?;
            if status.format_version != 1 || status.policy != recovery.policy_commitment.0 {
                return Err("authoring parent has a different recovery policy or format".into());
            }
            if status.phase == RecoveryPhase::Complete {
                return Ok(Self(None));
            }
        }
        let payload = match directory {
            Some(directory) => {
                let path = directory.join(format!("{parent:#x}.scale"));
                match std::fs::File::open(&path) {
                    Ok(file) => {
                        let mut bytes = Vec::new();
                        file.take(common::recovery::MAX_INHERENT_BYTES as u64 + 1)
                            .read_to_end(&mut bytes)?;
                        RecoveryInherent::decode_remark(&bytes)
                            .map_err(|_| "invalid or oversized recovery inherent payload")?;
                        bytes
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        RecoveryInherent {
                            format_version: 1,
                            action: RecoveryAction::Tick,
                        }
                        .encode_remark()
                    }
                    Err(error) => return Err(error.into()),
                }
            }
            None => RecoveryInherent {
                format_version: 1,
                action: RecoveryAction::Tick,
            }
            .encode_remark(),
        };
        validate_escape_executor(client, parent, recovery, &payload)?;
        Ok(Self(Some(payload)))
    }
}

/// Importing retained history is allowed; producing new blocks there is not.
/// This check also applies with --force-authoring and before the first child.
fn validate_authoring_parent(
    header: &<Block as BlockT>::Header,
    parent: H256,
    recovery: &RecoveryConfig,
    imported_policy: Option<&[u8]>,
) -> Result<(), String> {
    if *header.number() < CHECKPOINT_NUMBER {
        return Err("recovery authoring waits for the exact checkpoint import".into());
    }
    if imported_policy != Some(recovery.policy_commitment.as_bytes()) {
        return Err("authoring parent lacks verified checkpoint import provenance".into());
    }
    validate_header(header, parent, recovery)
}

fn validate_escape_executor(
    client: &crate::service::FullClient,
    parent: H256,
    recovery: &RecoveryConfig,
    bytes: &[u8],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let envelope = common::recovery::RecoveryInherent::decode_remark(bytes)
        .map_err(|_| "invalid recovery envelope")?;
    if let common::recovery::RecoveryAction::Escape { authorization, .. } = envelope.action {
        let version = client.runtime_api().version(parent)?;
        let expected = if version.spec_version == 134 {
            recovery.bootstrap_code_hash.0
        } else {
            let code = client
                .storage(parent, &StorageKey(b":code".to_vec()))?
                .ok_or("missing parent code")?;
            sp_io::hashing::blake2_256(&code.0)
        };
        if authorization.current_executor_hash != expected {
            return Err("escape authorization binds a different executing runtime".into());
        }
    }
    Ok(())
}

/// Decode only the stable bare System::remark prefix, not unrelated calls using
/// recovery metadata against historical spec-134 encodings.
fn recovery_remark(extrinsic: &sp_runtime::OpaqueExtrinsic) -> Result<Option<Vec<u8>>, String> {
    let encoded = extrinsic.encode();
    let bytes =
        Vec::<u8>::decode_all(&mut encoded.as_slice()).map_err(|_| "invalid opaque extrinsic")?;
    if bytes.len() < 3 || !matches!(bytes[0], 4 | 5) || bytes[1..3] != [0, 0] {
        return Ok(None);
    }
    let remark = Vec::<u8>::decode_all(&mut &bytes[3..]).map_err(|_| "invalid System remark")?;
    Ok(remark
        .starts_with(&common::recovery::INHERENT_IDENTIFIER)
        .then_some(remark))
}

#[async_trait::async_trait]
impl sp_inherents::InherentDataProvider for RecoveryInherentProvider {
    async fn provide_inherent_data(
        &self,
        data: &mut sp_inherents::InherentData,
    ) -> Result<(), sp_inherents::Error> {
        if let Some(bytes) = &self.0 {
            data.put_data(common::recovery::INHERENT_IDENTIFIER, bytes)?;
        }
        Ok(())
    }

    async fn try_handle_error(
        &self,
        identifier: &sp_inherents::InherentIdentifier,
        _: &[u8],
    ) -> Option<Result<(), sp_inherents::Error>> {
        (*identifier == common::recovery::INHERENT_IDENTIFIER).then(|| {
            Err(sp_inherents::Error::Application(
                "recovery inherent rejected by runtime".into(),
            ))
        })
    }
}

/// Offline preparation; refuses to overwrite an existing output specification.
#[derive(Debug, clap::Parser)]
pub struct PrepareRecoverySpecCmd {
    /// JSON file containing reviewed, public RecoveryConfig inputs.
    #[clap(long)]
    pub recovery_config: PathBuf,
    /// Rehearsed immutable spec-134 bootstrap Wasm.
    #[clap(long)]
    pub bootstrap_wasm: PathBuf,
    /// A new output file; existing files are never replaced.
    #[clap(long)]
    pub output: PathBuf,
}

impl PrepareRecoverySpecCmd {
    /// Create a new raw specification from reviewed public artifacts.
    pub fn run(&self) -> Result<(), String> {
        let recovery = serde_json::from_slice::<RecoveryConfig>(
            &std::fs::read(&self.recovery_config).map_err(|error| error.to_string())?,
        )
        .map_err(|error| format!("invalid recovery configuration: {error}"))?;
        let bootstrap = std::fs::read(&self.bootstrap_wasm).map_err(|error| error.to_string())?;
        let original = framenode_chain_spec::main_net()?;
        let prepared = framenode_chain_spec::recovery::prepare(&original, recovery, &bootstrap)?;
        validate_spec(&prepared)?;
        let bytes = sc_service::ChainSpec::as_json(&prepared, true)?;
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.output)
            .map_err(|error| format!("cannot create new recovery specification: {error}"))?;
        output
            .write_all(bytes.as_bytes())
            .map_err(|error| error.to_string())?;
        output.sync_all().map_err(|error| error.to_string())
    }
}

/// Return the exact same descriptor for ordinary import and warp verification.
pub fn hard_forks(spec: &dyn sc_service::ChainSpec) -> Vec<AuthoritySetHardFork<Block>> {
    framenode_chain_spec::recovery::config(spec)
        .map(|recovery| AuthoritySetHardFork {
            set_id: recovery.previous_grandpa_set_id + 1,
            block: (CHECKPOINT_HASH.into(), CHECKPOINT_NUMBER),
            authorities: recovery.grandpa_authorities.clone(),
            last_finalized: Some(CHECKPOINT_NUMBER - 1),
        })
        .into_iter()
        .collect()
}

/// Check substitutes before the SDK can silently overwrite a duplicate version.
pub fn validate_spec(spec: &dyn sc_service::ChainSpec) -> Result<(), String> {
    let recovery = framenode_chain_spec::recovery::config(spec);
    if recovery.is_none() && spec.id() == CHAIN_ID {
        return Err("recovery chain ID requires its complete recovery extension".into());
    }
    if let Some(recovery) = recovery {
        recovery.validate()?;
        if cfg!(feature = "private-net") {
            return Err("production recovery cannot use a private-net binary".into());
        }
        if spec.id() != CHAIN_ID
            || spec.protocol_id() != Some(PROTOCOL_ID)
            || spec.fork_id() != Some(FORK_ID)
        {
            return Err(
                "recovery requires its separate chain, protocol and fork identities".into(),
            );
        }
    }
    let executor = sc_executor::WasmExecutor::<crate::service::HostFunctions>::builder().build();
    let mut versions = BTreeMap::new();
    let mut recovery_substitute_found = false;
    for (height, code) in spec.code_substitutes() {
        let number = height
            .parse::<u32>()
            .map_err(|_| "invalid code-substitute height")?;
        if height != number.to_string() {
            return Err("code-substitute heights must use canonical decimal encoding".into());
        }
        let code_hash = sp_io::hashing::blake2_256(&code);
        let mut externalities = sp_state_machine::BasicExternalities::default();
        let version = executor
            .runtime_version(
                &mut externalities,
                &RuntimeCode {
                    code_fetcher: &WrappedRuntimeCode(code.as_slice().into()),
                    heap_pages: None,
                    hash: code_hash.to_vec(),
                },
            )
            .map_err(|error| format!("cannot read substitute version at {height}: {error}"))?;
        if let Some(prior) = versions.insert(version.spec_version, number) {
            return Err(format!(
                "duplicate substitute spec version {} at {prior} and {number}",
                version.spec_version
            ));
        }
        if let Some(recovery) = recovery {
            if number == CHECKPOINT_NUMBER {
                if version.spec_name != "sora-substrate"
                    || version.spec_version != 134
                    || version.transaction_version != 131
                    || H256::from(code_hash) != recovery.bootstrap_code_hash
                {
                    return Err(
                        "checkpoint substitute differs from the pinned spec-134/tx-131 bootstrap"
                            .into(),
                    );
                }
                recovery_substitute_found = true;
            } else if number > CHECKPOINT_NUMBER || version.spec_version >= 134 {
                return Err("unapproved substitute overlaps recovery execution".into());
            }
        }
    }
    if recovery.is_some() && !recovery_substitute_found {
        return Err("recovery specification has no pinned checkpoint bootstrap".into());
    }
    Ok(())
}

#[allow(
    clippy::result_large_err,
    reason = "preserve the upstream SDK service error type at the node integration boundary"
)]
pub fn validate_configuration(config: &Configuration) -> Result<(), ServiceError> {
    validate_spec(&*config.chain_spec).map_err(ServiceError::Other)?;
    if framenode_chain_spec::recovery::config(&*config.chain_spec).is_some()
        && config.wasm_runtime_overrides.is_some()
    {
        return Err(ServiceError::Other(
            "runtime overrides are forbidden on the recovery branch".into(),
        ));
    }
    Ok(())
}

pub fn validate_grandpa_keys(
    recovery: &RecoveryConfig,
    local_keys: &[Vec<u8>],
    is_authority: bool,
    maintenance: bool,
) -> Result<(), String> {
    if recovery.retired_grandpa_keys.iter().any(|key| {
        local_keys
            .iter()
            .any(|local| local.as_slice() == key.as_slice())
    }) {
        return Err("remove old GRANDPA signing keys before opening the recovery node".into());
    }
    if maintenance
        && local_keys.iter().any(|local| {
            !recovery
                .approved_grandpa_keys
                .iter()
                .any(|key| local.as_slice() == key.as_slice())
        })
    {
        return Err("recovery keystore contains an unapproved GRANDPA signing key".into());
    }
    if is_authority
        && maintenance
        && !recovery.approved_grandpa_keys.iter().any(|key| {
            local_keys
                .iter()
                .any(|local| local.as_slice() == key.as_slice())
        })
    {
        return Err("recovery validator has no manifest-listed fresh GRANDPA key".into());
    }
    Ok(())
}

#[allow(
    clippy::result_large_err,
    reason = "preserve the upstream SDK service error type at the node integration boundary"
)]
pub fn maintenance_active(
    client: &crate::service::FullClient,
    recovery: &RecoveryConfig,
) -> Result<bool, ServiceError> {
    let info = client.info();
    if info.best_number <= CHECKPOINT_NUMBER {
        return Ok(true);
    }
    let encoded = client
        .storage(
            info.best_hash,
            &StorageKey(common::recovery::STATUS_STORAGE_KEY.to_vec()),
        )?
        .ok_or_else(|| ServiceError::Other("recovery head has no recovery status".into()))?;
    let status = common::recovery::RecoveryStatus::decode_all(&mut encoded.0.as_slice())
        .map_err(|error| ServiceError::Other(format!("invalid recovery head status: {error}")))?;
    if status.format_version != 1 || status.policy != recovery.policy_commitment.0 {
        return Err(ServiceError::Other(
            "recovery head status has the wrong policy".into(),
        ));
    }
    Ok(status.phase != common::recovery::RecoveryPhase::Complete)
}

/// Do not invoke historical runtime offchain workers while a recovery node
/// catches up to the checkpoint. Start the runner only for imports following
/// the completed handoff, after the final runtime can execute its migrations.
#[allow(
    clippy::result_large_err,
    reason = "preserve the upstream SDK service error type at the node integration boundary"
)]
pub async fn wait_for_activation(
    client: Arc<crate::service::FullClient>,
    recovery: &RecoveryConfig,
) -> Result<(), ServiceError> {
    use futures::StreamExt;
    use sc_client_api::BlockchainEvents;
    let mut imports = client.import_notification_stream();
    loop {
        match maintenance_active(&client, recovery) {
            Ok(false) => return Ok(()),
            Ok(true) => {}
            // Header-only warp import can temporarily leave the state absent.
            // Never enable workers on an unreadable or malformed status; retry
            // only after another import has supplied a new state.
            Err(error) => log::debug!("Awaiting verified recovery activation state: {error}"),
        }
        if imports.next().await.is_none() {
            return Err(ServiceError::Other(
                "recovery import stream closed before activation; workers remain stopped".into(),
            ));
        }
    }
}

// Narrow read-only database view. Production and missing-index regression tests
// run the same guard; the test adapter cannot bypass any admission decision.
struct RecoveryDatabaseInfo {
    genesis_hash: H256,
    heads: [(u32, H256); 2],
}

#[allow(
    clippy::result_large_err,
    reason = "preserve the upstream SDK service error type at the node integration boundary"
)]
trait RecoveryDatabaseView {
    fn recovery_info(&self) -> RecoveryDatabaseInfo;
    fn recovery_header(
        &self,
        hash: H256,
    ) -> Result<Option<<Block as BlockT>::Header>, ServiceError>;
    fn recovery_number_hash(&self, number: u32) -> Result<Option<H256>, ServiceError>;
    fn recovery_provenance(&self) -> Result<Option<Vec<u8>>, ServiceError>;
    fn recovery_policy(&self, hash: H256) -> Result<Option<Vec<u8>>, ServiceError>;
}

#[allow(
    clippy::result_large_err,
    reason = "preserve the upstream SDK service error type at the node integration boundary"
)]
impl RecoveryDatabaseView for crate::service::FullClient {
    fn recovery_info(&self) -> RecoveryDatabaseInfo {
        let info = self.info();
        RecoveryDatabaseInfo {
            genesis_hash: info.genesis_hash,
            heads: [
                (info.best_number, info.best_hash),
                (info.finalized_number, info.finalized_hash),
            ],
        }
    }

    fn recovery_header(
        &self,
        hash: H256,
    ) -> Result<Option<<Block as BlockT>::Header>, ServiceError> {
        self.header(hash).map_err(ServiceError::from)
    }

    fn recovery_number_hash(&self, number: u32) -> Result<Option<H256>, ServiceError> {
        self.hash(number).map_err(ServiceError::from)
    }

    fn recovery_provenance(&self) -> Result<Option<Vec<u8>>, ServiceError> {
        self.get_aux(CHECKPOINT_IMPORTED_KEY)
            .map_err(ServiceError::from)
    }

    fn recovery_policy(&self, hash: H256) -> Result<Option<Vec<u8>>, ServiceError> {
        self.storage(hash, &StorageKey(POLICY_STORAGE_KEY.to_vec()))
            .map(|value| value.map(|data| data.0))
            .map_err(ServiceError::from)
    }
}

/// Refuse a reused old-chain database, including an already-imported checkpoint.
#[allow(
    clippy::result_large_err,
    reason = "preserve the upstream SDK service error type at the node integration boundary"
)]
pub fn validate_database(
    client: &crate::service::FullClient,
    recovery: &RecoveryConfig,
) -> Result<(), ServiceError> {
    validate_database_view(client, recovery)
}

#[allow(
    clippy::result_large_err,
    reason = "preserve the upstream SDK service error type at the node integration boundary"
)]
fn validate_database_view(
    client: &impl RecoveryDatabaseView,
    recovery: &RecoveryConfig,
) -> Result<(), ServiceError> {
    let info = client.recovery_info();
    if info.genesis_hash != H256::from(GENESIS_HASH) {
        return Err(ServiceError::Other(
            "recovery database has a different genesis".into(),
        ));
    }
    for (number, hash) in info.heads {
        if number < CHECKPOINT_NUMBER {
            continue;
        }
        // Warp sync stores the checkpoint header but indexes it by number only
        // once gap sync reaches that height, so look it up by hash. Where the
        // number index exists, it must still name the checkpoint.
        let checkpoint = client
            .recovery_header(H256::from(CHECKPOINT_HASH))?
            .ok_or_else(|| ServiceError::Other("recovery checkpoint header is missing".into()))?;
        if *checkpoint.number() != CHECKPOINT_NUMBER
            || checkpoint.hash() != H256::from(CHECKPOINT_HASH)
        {
            return Err(ServiceError::Other(
                "block does not match the recovery checkpoint header and state root".into(),
            ));
        }
        validate_header(&checkpoint, checkpoint.hash(), recovery).map_err(ServiceError::Other)?;
        if client
            .recovery_number_hash(CHECKPOINT_NUMBER)?
            .is_some_and(|canonical| canonical != H256::from(CHECKPOINT_HASH))
        {
            return Err(ServiceError::Other(
                "database checkpoint hash is not canonical".into(),
            ));
        }
        if client.recovery_provenance()?.as_deref() != Some(recovery.policy_commitment.as_bytes()) {
            return Err(ServiceError::Other(
                "checkpoint lacks recovery import provenance; prepare a separate database ending before the checkpoint".into(),
            ));
        }
        let header = client
            .recovery_header(hash)?
            .ok_or_else(|| ServiceError::Other("recovery database head is missing".into()))?;
        validate_header(&header, hash, recovery).map_err(ServiceError::Other)?;
        if number > CHECKPOINT_NUMBER {
            let policy = client.recovery_policy(hash)?;
            if policy.as_deref() != Some(recovery.policy_commitment.as_bytes()) {
                return Err(ServiceError::Other(
                    "recovery database policy commitment differs".into(),
                ));
            }
        }
    }
    Ok(())
}

fn validate_header(
    header: &<Block as BlockT>::Header,
    post_hash: H256,
    recovery: &RecoveryConfig,
) -> Result<(), String> {
    let number = *header.number();
    if number == CHECKPOINT_NUMBER {
        if post_hash != H256::from(CHECKPOINT_HASH)
            || *header.state_root() != H256::from(CHECKPOINT_STATE_ROOT)
        {
            return Err(
                "block does not match the recovery checkpoint header and state root".into(),
            );
        }
    } else if number > CHECKPOINT_NUMBER {
        let markers: Vec<_> = header
            .digest()
            .logs()
            .iter()
            .filter_map(|digest| match digest {
                DigestItem::Other(bytes) => parse_lineage_marker(bytes),
                _ => None,
            })
            .collect();
        if markers != [recovery.policy_commitment.0] {
            return Err("recovery descendant lacks its unique policy lineage marker".into());
        }
        if number == CHECKPOINT_NUMBER + 1 {
            if *header.parent_hash() != H256::from(CHECKPOINT_HASH) {
                return Err("first recovery child does not extend the checkpoint".into());
            }
            use sp_consensus_babe::digests::CompatibleDigestItem;
            let slots: Vec<_> = header
                .digest()
                .logs()
                .iter()
                .filter_map(|digest| digest.as_babe_pre_digest().map(|pre| u64::from(pre.slot())))
                .collect();
            if slots.len() != 1 || slots[0] < recovery.first_recovery_slot {
                return Err("first recovery child reuses an abandoned signing slot".into());
            }
        }
    }
    Ok(())
}

/// Outer importer checks full headers before BABE/GRANDPA alter the import state.
#[derive(Clone)]
pub struct RecoveryBlockImport<I> {
    inner: I,
    recovery: Option<RecoveryConfig>,
    authorities: SharedAuthoritySet<H256, u32>,
    client: Arc<crate::service::FullClient>,
    evidence: Option<Arc<crate::recovery_evidence::Recorder>>,
}

impl<I> RecoveryBlockImport<I> {
    /// Wrap BABE import with checkpoint, lineage and authority admission.
    pub fn new(
        inner: I,
        recovery: Option<RecoveryConfig>,
        authorities: SharedAuthoritySet<H256, u32>,
        client: Arc<crate::service::FullClient>,
    ) -> Self {
        Self {
            inner,
            recovery,
            authorities,
            client,
            evidence: None,
        }
    }

    /// Attach a passive verification journal without changing ordinary imports.
    pub fn with_evidence(
        mut self,
        evidence: Option<Arc<crate::recovery_evidence::Recorder>>,
    ) -> Self {
        self.evidence = evidence;
        self
    }
}

#[async_trait::async_trait]
impl<I> BlockImport<Block> for RecoveryBlockImport<I>
where
    I: BlockImport<Block, Error = sp_consensus::Error> + Send + Sync,
{
    type Error = sp_consensus::Error;

    async fn check_block(
        &self,
        block: BlockCheckParams<Block>,
    ) -> Result<ImportResult, Self::Error> {
        if self.recovery.is_some()
            && block.number == CHECKPOINT_NUMBER
            && block.hash != H256::from(CHECKPOINT_HASH)
        {
            return Err(sp_consensus::Error::ClientImport(
                "incorrect recovery checkpoint".into(),
            ));
        }
        self.inner.check_block(block).await
    }

    async fn import_block(
        &self,
        mut block: BlockImportParams<Block>,
    ) -> Result<ImportResult, Self::Error> {
        if let Some(recovery) = &self.recovery {
            validate_header(&block.header, block.post_hash(), recovery)
                .map_err(sp_consensus::Error::ClientImport)?;
            if *block.header.number() > CHECKPOINT_NUMBER {
                if let Some(body) = &block.body {
                    for extrinsic in body {
                        if let Some(remark) =
                            recovery_remark(extrinsic).map_err(sp_consensus::Error::ClientImport)?
                        {
                            validate_escape_executor(
                                &self.client,
                                *block.header.parent_hash(),
                                recovery,
                                &remark,
                            )
                            .map_err(|error| {
                                sp_consensus::Error::ClientImport(error.to_string())
                            })?;
                        }
                    }
                }
                if let sc_consensus::StateAction::ApplyChanges(
                    sc_consensus::StorageChanges::Import(state),
                ) = &block.state_action
                {
                    let top: Vec<_> = state
                        .state
                        .0
                        .iter()
                        .filter(|level| level.state_root.is_empty())
                        .collect();
                    let policies: Vec<_> = top
                        .iter()
                        .flat_map(|level| &level.key_values)
                        .filter(|(key, _)| key.as_slice() == POLICY_STORAGE_KEY)
                        .map(|(_, value)| value.as_slice())
                        .collect();
                    if top.len() != 1 || policies != [recovery.policy_commitment.as_bytes()] {
                        return Err(sp_consensus::Error::ClientImport(
                            "imported recovery state lacks its unique policy commitment".into(),
                        ));
                    }
                }
            }
            if *block.header.number() == CHECKPOINT_NUMBER {
                let warp = block.origin == sp_consensus::BlockOrigin::WarpSync;
                let previously_imported = self
                    .client
                    .get_aux(CHECKPOINT_IMPORTED_KEY)
                    .map_err(|error| sp_consensus::Error::ChainLookup(error.to_string()))?
                    .as_deref()
                    == Some(recovery.policy_commitment.as_bytes());
                if !warp
                    && !previously_imported
                    && self.authorities.set_id() != recovery.previous_grandpa_set_id
                {
                    return Err(sp_consensus::Error::ClientImport(
                        "pre-checkpoint GRANDPA set ID differs from the recovery manifest".into(),
                    ));
                }
                let actual: BTreeSet<_> = self
                    .authorities
                    .current_authorities()
                    .iter()
                    .map(|(id, info)| (id.clone(), info.weight().get()))
                    .collect();
                let expected: BTreeSet<_> = recovery
                    .previous_grandpa_authorities
                    .iter()
                    .cloned()
                    .collect();
                if !warp && !previously_imported && actual != expected {
                    return Err(sp_consensus::Error::ClientImport(
                        "pre-checkpoint GRANDPA authorities differ from the recovery manifest"
                            .into(),
                    ));
                }
                block.auxiliary.push((
                    CHECKPOINT_IMPORTED_KEY.to_vec(),
                    Some(recovery.policy_commitment.as_bytes().to_vec()),
                ));
            }
        }
        let observation = self.evidence.as_ref().map(|_| serde_json::json!({
            "kind":"import-attempt", "hash":format!("{:#x}",block.post_hash()),
            "number":*block.header.number(), "parentHash":format!("{:#x}",block.header.parent_hash()),
            "origin":format!("{:?}",block.origin), "headerOnly":block.body.is_none(),
        }));
        if let (Some(recorder), Some(value)) = (&self.evidence, &observation) {
            recorder
                .record(value.clone(), None)
                .map_err(sp_consensus::Error::ClientImport)?;
        }
        let result = self.inner.import_block(block).await;
        if let (Some(recorder), Some(mut value)) = (&self.evidence, observation) {
            value["kind"] = "import-result".into();
            value["outcome"] = format!("{result:?}").into();
            value["imported"] = matches!(&result, Ok(ImportResult::Imported(_))).into();
            if let Ok(ImportResult::Imported(aux)) = &result {
                value["headerOnly"] = aux.header_only.into();
                value["badJustification"] = aux.bad_justification.into();
            }
            recorder
                .record(value, None)
                .map_err(sp_consensus::Error::ClientImport)?;
        }
        result
    }
}

/// SCALE wire shape of the SDK proof, whose fields are private. Keep the format
/// equality test when updating the SDK. No alternative finality verifier is used.
#[derive(Encode, Decode)]
struct WarpProofWire {
    proofs: Vec<sc_finality_grandpa::warp_proof::WarpSyncFragment<Block>>,
    is_finished: bool,
}

/// Recovery adds an explicitly trusted checkpoint transition before the SDK's
/// cryptographically verified suffix. The stock generator otherwise stops at a
/// forced change without a scheduled-change digest.
pub struct RecoveryWarpProvider {
    inner: Arc<dyn WarpSyncProvider<Block>>,
    client: Arc<crate::service::FullClient>,
    recovery: Option<RecoveryConfig>,
    evidence: Option<Arc<crate::recovery_evidence::Recorder>>,
}

impl RecoveryWarpProvider {
    /// Add the pinned recovery checkpoint to the stock warp implementation.
    pub fn new(
        inner: Arc<dyn WarpSyncProvider<Block>>,
        client: Arc<crate::service::FullClient>,
        recovery: Option<RecoveryConfig>,
    ) -> Self {
        Self {
            inner,
            client,
            recovery,
            evidence: None,
        }
    }

    /// Retain successful SDK verification only when explicitly requested.
    pub fn with_evidence(
        mut self,
        evidence: Option<Arc<crate::recovery_evidence::Recorder>>,
    ) -> Self {
        self.evidence = evidence;
        self
    }
}

fn trusted_boundary_fragment(
    header: <Block as BlockT>::Header,
) -> sc_finality_grandpa::warp_proof::WarpSyncFragment<Block> {
    // The configured SDK hard-fork rule trusts this exact header/hash and does
    // not inspect an old-set justification. This empty-signature value is a
    // wire-format boundary marker, NOT a purported finality proof. Original
    // justifications remain separate historical evidence.
    let commit = sc_finality_grandpa::Commit::<<Block as BlockT>::Header> {
        target_hash: header.hash(),
        target_number: *header.number(),
        precommits: Vec::new(),
    };
    let encoded = (0u64, commit, Vec::<<Block as BlockT>::Header>::new()).encode();
    let justification =
        sc_finality_grandpa::GrandpaJustification::<Block>::decode_all(&mut encoded.as_slice())
            .expect("the SDK GRANDPA justification has the same SCALE fields; covered by tests");
    sc_finality_grandpa::warp_proof::WarpSyncFragment {
        header,
        justification,
    }
}

impl WarpSyncProvider<Block> for RecoveryWarpProvider {
    fn generate(
        &self,
        start: H256,
    ) -> Result<EncodedProof, Box<dyn std::error::Error + Send + Sync>> {
        let Some(recovery) = &self.recovery else {
            return self.inner.generate(start);
        };
        let number = self
            .client
            .number(start)?
            .ok_or("unknown warp start block")?;
        if self.client.hash(number)? != Some(start) {
            return Err("warp start is not on the canonical branch".into());
        }
        if self.client.info().finalized_number <= CHECKPOINT_NUMBER {
            return Err("recovery warp sync requires a finalized recovery descendant".into());
        }
        if number >= CHECKPOINT_NUMBER {
            return self.inner.generate(start);
        }
        let checkpoint = self
            .client
            .header(CHECKPOINT_HASH.into())?
            .ok_or("checkpoint header is unavailable for recovery warp sync")?;
        validate_header(&checkpoint, checkpoint.hash(), recovery)?;
        let suffix = self.inner.generate(CHECKPOINT_HASH.into())?;
        if suffix.0.len() > MAX_WARP_PROOF_BYTES {
            return Err("oversized SDK warp suffix".into());
        }
        let mut proof = WarpProofWire::decode_all(&mut suffix.0.as_slice())?;
        if proof
            .proofs
            .iter()
            .any(|fragment| *fragment.header.number() <= CHECKPOINT_NUMBER)
        {
            return Err("recovery warp suffix overlaps the trusted checkpoint".into());
        }
        proof
            .proofs
            .insert(0, trusted_boundary_fragment(checkpoint));
        // Return a partial boundary when adding it crosses the SDK byte cap.
        // A subsequent request starts at that boundary and obtains a real suffix.
        while proof.encoded_size() > MAX_WARP_PROOF_BYTES && proof.proofs.len() > 1 {
            proof.proofs.pop();
            proof.is_finished = false;
        }
        if proof.proofs.len() == 1 {
            proof.is_finished = false;
        }
        Ok(EncodedProof(proof.encode()))
    }

    fn create_verifier(&self) -> Box<dyn Verifier<Block>> {
        Box::new(RecoveryWarpVerifier {
            inner: self.inner.create_verifier(),
            recovery: self.recovery.clone(),
            last_number: None,
            boundary_seen: false,
            evidence: self.evidence.clone(),
        })
    }
}

struct RecoveryWarpVerifier {
    inner: Box<dyn Verifier<Block>>,
    recovery: Option<RecoveryConfig>,
    last_number: Option<u32>,
    boundary_seen: bool,
    evidence: Option<Arc<crate::recovery_evidence::Recorder>>,
}

/// A matching lineage marker is not a finality proof. Every verifier must cross
/// the exact forced boundary before accepting any descendant, including when a
/// compromised old authority signs an otherwise well-formed fabricated header.
fn validate_warp_fragments(
    proof: &WarpProofWire,
    recovery: &RecoveryConfig,
    mut last_number: Option<u32>,
    mut boundary_seen: bool,
) -> Result<(Option<u32>, bool), String> {
    if proof.proofs.is_empty() {
        return Err("empty recovery warp proof".into());
    }
    for fragment in &proof.proofs {
        let number = *fragment.header.number();
        if last_number.is_some_and(|last| number <= last) {
            return Err("recovery warp proof does not advance monotonically".into());
        }
        if number > CHECKPOINT_NUMBER && !boundary_seen {
            return Err("recovery warp proof skipped the forced checkpoint boundary".into());
        }
        validate_header(&fragment.header, fragment.header.hash(), recovery)?;
        if number == CHECKPOINT_NUMBER {
            boundary_seen = true;
        }
        last_number = Some(number);
    }
    if proof.is_finished
        && (!boundary_seen || last_number.is_none_or(|number| number <= CHECKPOINT_NUMBER))
    {
        return Err("trusted checkpoint alone cannot complete recovery warp sync".into());
    }
    Ok((last_number, boundary_seen))
}

impl Verifier<Block> for RecoveryWarpVerifier {
    fn verify(
        &mut self,
        encoded: &EncodedProof,
    ) -> Result<VerificationResult<Block>, Box<dyn std::error::Error + Send + Sync>> {
        let mut next = (self.last_number, self.boundary_seen);
        if let Some(recovery) = &self.recovery {
            if encoded.0.len() > MAX_WARP_PROOF_BYTES {
                return Err("oversized recovery warp proof".into());
            }
            let proof = WarpProofWire::decode_all(&mut encoded.0.as_slice())?;
            next = validate_warp_fragments(&proof, recovery, next.0, next.1)?;
        }
        let context = self
            .evidence
            .as_ref()
            .map(|_| self.inner.next_proof_context());
        let result = self.inner.verify(encoded)?;
        if let (Some(recorder), Some(context)) = (&self.evidence, context) {
            recorder.record(
                serde_json::json!({"kind":"warp-accepted", "context":format!("{context:#x}"),
                "complete":matches!(&result, VerificationResult::Complete(_, _)),
                "target":format!("{:#x}",self.inner.next_proof_context())}),
                Some(&encoded.0),
            )?;
        }
        // Only commit the boundary after the SDK accepted the complete proof.
        (self.last_number, self.boundary_seen) = next;
        Ok(result)
    }

    fn next_proof_context(&self) -> H256 {
        self.inner.next_proof_context()
    }

    fn status(&self) -> Option<String> {
        self.inner.status()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sp_runtime::generic::Header;
    use sp_runtime::traits::BlakeTwo256;

    #[test]
    fn production_full_startup_requires_recovery_even_for_rpc_nodes() {
        let production = H256::from(GENESIS_HASH);
        assert!(validate_startup_network(production, false, false).is_err());
        assert!(validate_startup_network(production, false, true).is_ok());
        assert!(validate_startup_network(production, true, false).is_ok());
        assert!(validate_startup_network(H256::repeat_byte(7), false, false).is_ok());
    }

    #[test]
    fn recovery_remark_uses_preserved_system_indices_and_bare_encoding() {
        let remark = common::recovery::RecoveryInherent {
            format_version: 1,
            action: common::recovery::RecoveryAction::Tick,
        }
        .encode_remark();
        let call =
            framenode_runtime::RuntimeCall::System(framenode_runtime::frame_system::Call::remark {
                remark: remark.clone(),
            });
        let xt = framenode_runtime::UncheckedExtrinsic::new_bare_legacy(call);
        let opaque = sp_runtime::OpaqueExtrinsic::try_from_encoded_extrinsic(&xt.encode()).unwrap();
        assert_eq!(recovery_remark(&opaque).unwrap(), Some(remark));
        let ordinary = framenode_runtime::UncheckedExtrinsic::new_bare_legacy(
            framenode_runtime::RuntimeCall::System(framenode_runtime::frame_system::Call::remark {
                remark: b"ordinary".to_vec(),
            }),
        );
        let opaque =
            sp_runtime::OpaqueExtrinsic::try_from_encoded_extrinsic(&ordinary.encode()).unwrap();
        assert_eq!(recovery_remark(&opaque).unwrap(), None);
    }

    fn fixture() -> RecoveryConfig {
        let policy = common::recovery::RecoveryPolicy {
            format_version: 2,
            checkpoint_hash: CHECKPOINT_HASH,
            checkpoint_state_root: CHECKPOINT_STATE_ROOT,
            replacements: vec![],
            council: vec![sp_runtime::AccountId32::new([7; 32])],
            chunk_hashes: vec![[8; 32]],
            audit_chunk_hashes: vec![[9; 32]],
            evidence_commitment: [5; 32],
            terminal_audit_commitment: [10; 32],
            authorization_domain: common::recovery::RecoveryDomain {
                chain_spec_id: CHAIN_ID.as_bytes().to_vec(),
                release_nonce: [11; 32],
            },
        };
        let key = |byte| sp_core::ed25519::Public::from_raw([byte; 32]).into();
        RecoveryConfig {
            schema_version: 2,
            checkpoint_hash: CHECKPOINT_HASH.into(),
            policy_commitment: policy.commitment().into(),
            policy_scale: sp_core::Bytes(policy.encode()),
            bootstrap_code_hash: [2; 32].into(),
            final_code_hash: [3; 32].into(),
            hold_code_hash: [4; 32].into(),
            evidence_commitment: [5; 32].into(),
            previous_grandpa_set_id: 7,
            previous_grandpa_authorities: vec![(key(1), 1)],
            grandpa_authorities: vec![(key(2), 1)],
            retired_grandpa_keys: vec![key(1), key(3)],
            approved_grandpa_keys: vec![key(2), key(4)],
            last_abandoned_signed_slot: 99,
            first_recovery_slot: 100,
        }
    }

    fn header(number: u32) -> Header<u32, BlakeTwo256> {
        Header::new(
            number,
            H256::zero(),
            CHECKPOINT_STATE_ROOT.into(),
            CHECKPOINT_HASH.into(),
            Default::default(),
        )
    }

    struct MockRecoveryDatabase {
        genesis: H256,
        heads: [(u32, H256); 2],
        headers: BTreeMap<H256, <Block as BlockT>::Header>,
        checkpoint_number_hash: Option<H256>,
        provenance: Option<Vec<u8>>,
        policies: BTreeMap<H256, Vec<u8>>,
    }

    #[allow(
        clippy::result_large_err,
        reason = "exercise the production database guard with its SDK error type"
    )]
    impl RecoveryDatabaseView for MockRecoveryDatabase {
        fn recovery_info(&self) -> RecoveryDatabaseInfo {
            RecoveryDatabaseInfo {
                genesis_hash: self.genesis,
                heads: self.heads,
            }
        }

        fn recovery_header(
            &self,
            hash: H256,
        ) -> Result<Option<<Block as BlockT>::Header>, ServiceError> {
            Ok(self.headers.get(&hash).cloned())
        }

        fn recovery_number_hash(&self, number: u32) -> Result<Option<H256>, ServiceError> {
            assert_eq!(number, CHECKPOINT_NUMBER);
            Ok(self.checkpoint_number_hash)
        }

        fn recovery_provenance(&self) -> Result<Option<Vec<u8>>, ServiceError> {
            Ok(self.provenance.clone())
        }

        fn recovery_policy(&self, hash: H256) -> Result<Option<Vec<u8>>, ServiceError> {
            Ok(self.policies.get(&hash).cloned())
        }
    }

    fn warp_restart_database(recovery: &RecoveryConfig) -> MockRecoveryDatabase {
        use sp_consensus_babe::digests::{
            CompatibleDigestItem, PreDigest, SecondaryPlainPreDigest,
        };
        let checkpoint: <Block as BlockT>::Header =
            serde_json::from_str(include_str!("testdata/recovery_checkpoint_header.json")).unwrap();
        assert_eq!(checkpoint.hash(), H256::from(CHECKPOINT_HASH));
        let mut child = header(CHECKPOINT_NUMBER + 1);
        child
            .digest_mut()
            .push(DigestItem::Other(common::recovery::lineage_marker(
                recovery.policy_commitment.0,
            )));
        child
            .digest_mut()
            .push(DigestItem::babe_pre_digest(PreDigest::SecondaryPlain(
                SecondaryPlainPreDigest {
                    authority_index: 0,
                    slot: recovery.first_recovery_slot.into(),
                },
            )));
        let child_hash = child.hash();
        MockRecoveryDatabase {
            genesis: H256::from(GENESIS_HASH),
            heads: [(CHECKPOINT_NUMBER + 1, child_hash); 2],
            headers: [(checkpoint.hash(), checkpoint), (child_hash, child)]
                .into_iter()
                .collect(),
            checkpoint_number_hash: None,
            provenance: Some(recovery.policy_commitment.as_bytes().to_vec()),
            policies: [(child_hash, recovery.policy_commitment.as_bytes().to_vec())]
                .into_iter()
                .collect(),
        }
    }

    #[test]
    fn warp_restart_accepts_checkpoint_header_before_number_index_is_rebuilt() {
        let recovery = fixture();
        let mut database = warp_restart_database(&recovery);
        assert!(database.checkpoint_number_hash.is_none());
        assert!(database.headers.contains_key(&H256::from(CHECKPOINT_HASH)));
        assert!(validate_database_view(&database, &recovery).is_ok());
        database.checkpoint_number_hash = Some(H256::from(CHECKPOINT_HASH));
        assert!(validate_database_view(&database, &recovery).is_ok());
    }

    #[test]
    fn warp_restart_missing_number_index_does_not_bypass_other_guards() {
        let recovery = fixture();
        let reject = |database: MockRecoveryDatabase, expected: &str| {
            let error = validate_database_view(&database, &recovery).unwrap_err();
            assert!(
                error.to_string().contains(expected),
                "unexpected error: {error}"
            );
        };
        let mut database = warp_restart_database(&recovery);
        database.checkpoint_number_hash = Some(H256::repeat_byte(42));
        reject(database, "database checkpoint hash is not canonical");
        let mut database = warp_restart_database(&recovery);
        database.headers.remove(&H256::from(CHECKPOINT_HASH));
        reject(database, "recovery checkpoint header is missing");
        let mut database = warp_restart_database(&recovery);
        database
            .headers
            .get_mut(&H256::from(CHECKPOINT_HASH))
            .unwrap()
            .set_state_root(H256::zero());
        reject(database, "block does not match the recovery checkpoint");
        let mut database = warp_restart_database(&recovery);
        database
            .headers
            .get_mut(&H256::from(CHECKPOINT_HASH))
            .unwrap()
            .set_number(CHECKPOINT_NUMBER - 1);
        reject(database, "block does not match the recovery checkpoint");
        for provenance in [None, Some(vec![0; 32])] {
            let mut database = warp_restart_database(&recovery);
            database.provenance = provenance;
            reject(database, "checkpoint lacks recovery import provenance");
        }
        for policy in [None, Some(vec![0; 32])] {
            let mut database = warp_restart_database(&recovery);
            let hash = database.heads[0].1;
            match policy {
                Some(value) => {
                    database.policies.insert(hash, value);
                }
                None => {
                    database.policies.remove(&hash);
                }
            }
            reject(database, "recovery database policy commitment differs");
        }
        let mut database = warp_restart_database(&recovery);
        database.genesis = H256::zero();
        reject(database, "recovery database has a different genesis");
        let mut database = warp_restart_database(&recovery);
        let head = database.heads[0].1;
        database.headers.remove(&head);
        reject(database, "recovery database head is missing");
        let mut database = warp_restart_database(&recovery);
        let head = database.heads[0].1;
        database
            .headers
            .get_mut(&head)
            .unwrap()
            .digest_mut()
            .logs
            .clear();
        reject(
            database,
            "recovery descendant lacks its unique policy lineage marker",
        );
    }

    #[test]
    fn warp_restart_validates_finalized_head_independently_of_best_head() {
        let recovery = fixture();
        let mut database = warp_restart_database(&recovery);
        let finalized = database.heads[1].1;
        let mut best = database.headers[&finalized].clone();
        best.set_number(CHECKPOINT_NUMBER + 2);
        best.set_parent_hash(finalized);
        let best_hash = best.hash();
        database.heads[0] = (CHECKPOINT_NUMBER + 2, best_hash);
        database.headers.insert(best_hash, best);
        database
            .policies
            .insert(best_hash, recovery.policy_commitment.as_bytes().to_vec());
        assert!(validate_database_view(&database, &recovery).is_ok());
        database.policies.insert(finalized, vec![0; 32]);
        assert!(validate_database_view(&database, &recovery)
            .unwrap_err()
            .to_string()
            .contains("recovery database policy commitment differs"));
    }

    #[test]
    fn rejects_abandoned_headers_and_duplicate_lineage() {
        let recovery = fixture();
        let mut header = header(CHECKPOINT_NUMBER + 2);
        assert!(validate_header(&header, header.hash(), &recovery).is_err());
        header
            .digest_mut()
            .push(DigestItem::Other(common::recovery::lineage_marker(
                recovery.policy_commitment.0,
            )));
        assert!(validate_header(&header, header.hash(), &recovery).is_ok());
        header
            .digest_mut()
            .push(DigestItem::Other(common::recovery::lineage_marker(
                recovery.policy_commitment.0,
            )));
        assert!(validate_header(&header, header.hash(), &recovery).is_err());
    }

    #[test]
    fn checks_checkpoint_hash_and_root_but_leaves_earlier_headers_unchanged() {
        let recovery = fixture();
        let mut checkpoint = header(CHECKPOINT_NUMBER);
        assert!(validate_header(&checkpoint, CHECKPOINT_HASH.into(), &recovery).is_ok());
        assert!(validate_header(&checkpoint, H256::zero(), &recovery).is_err());
        checkpoint.set_state_root(H256::zero());
        assert!(validate_header(&checkpoint, CHECKPOINT_HASH.into(), &recovery).is_err());
        assert!(validate_header(&header(CHECKPOINT_NUMBER - 1), H256::zero(), &recovery).is_ok());
    }

    #[test]
    fn recovery_authoring_cannot_extend_unimported_history() {
        let recovery = fixture();
        let provenance = Some(recovery.policy_commitment.as_bytes());
        assert!(validate_authoring_parent(
            &header(CHECKPOINT_NUMBER - 1),
            H256::zero(),
            &recovery,
            provenance,
        )
        .is_err());
        assert!(validate_authoring_parent(
            &header(CHECKPOINT_NUMBER),
            CHECKPOINT_HASH.into(),
            &recovery,
            None,
        )
        .is_err());
        assert!(validate_authoring_parent(
            &header(CHECKPOINT_NUMBER),
            H256::zero(),
            &recovery,
            provenance,
        )
        .is_err());
        assert!(validate_authoring_parent(
            &header(CHECKPOINT_NUMBER),
            CHECKPOINT_HASH.into(),
            &recovery,
            provenance,
        )
        .is_ok());
        let abandoned = header(CHECKPOINT_NUMBER + 2);
        assert!(
            validate_authoring_parent(&abandoned, abandoned.hash(), &recovery, provenance,)
                .is_err()
        );
    }

    #[test]
    fn first_child_requires_fresh_slot() {
        use sp_consensus_babe::digests::{
            CompatibleDigestItem, PreDigest, SecondaryPlainPreDigest,
        };
        let recovery = fixture();
        for (slot, accepted) in [(99, false), (100, true)] {
            let mut header = header(CHECKPOINT_NUMBER + 1);
            header
                .digest_mut()
                .push(DigestItem::Other(common::recovery::lineage_marker(
                    recovery.policy_commitment.0,
                )));
            header
                .digest_mut()
                .push(DigestItem::babe_pre_digest(PreDigest::SecondaryPlain(
                    SecondaryPlainPreDigest {
                        authority_index: 0,
                        slot: slot.into(),
                    },
                )));
            assert_eq!(
                validate_header(&header, header.hash(), &recovery).is_ok(),
                accepted
            );
        }
    }

    #[test]
    fn warp_lineage_marker_cannot_bypass_the_forced_boundary() {
        let recovery = fixture();
        let checkpoint: <Block as BlockT>::Header =
            serde_json::from_str(include_str!("testdata/recovery_checkpoint_header.json")).unwrap();
        assert_eq!(checkpoint.hash(), H256::from(CHECKPOINT_HASH));
        let mut target = header(CHECKPOINT_NUMBER + 2);
        target
            .digest_mut()
            .push(DigestItem::Other(common::recovery::lineage_marker(
                recovery.policy_commitment.0,
            )));
        let suffix = WarpProofWire {
            proofs: vec![trusted_boundary_fragment(target)],
            is_finished: true,
        };
        assert!(validate_warp_fragments(&suffix, &recovery, None, false).is_err());
        let boundary = WarpProofWire {
            proofs: vec![trusted_boundary_fragment(checkpoint)],
            is_finished: false,
        };
        let (last, seen) = validate_warp_fragments(&boundary, &recovery, None, false).unwrap();
        assert_eq!(last, Some(CHECKPOINT_NUMBER));
        assert!(seen);
        assert!(validate_warp_fragments(&suffix, &recovery, last, seen).is_ok());
        let complete_boundary = WarpProofWire {
            is_finished: true,
            ..boundary
        };
        assert!(validate_warp_fragments(&complete_boundary, &recovery, None, false).is_err());
    }

    #[test]
    fn trusted_boundary_wire_is_sdk_compatible_and_has_no_signatures() {
        let fragment = trusted_boundary_fragment(header(CHECKPOINT_NUMBER));
        assert!(fragment
            .justification
            .justification
            .commit
            .precommits
            .is_empty());
        let proof = WarpProofWire {
            proofs: vec![fragment],
            is_finished: false,
        };
        let encoded = proof.encode();
        assert!(
            sc_finality_grandpa::warp_proof::WarpSyncProof::<Block>::decode_all(
                &mut encoded.as_slice(),
            )
            .is_ok()
        );
    }

    fn sdk_fixture_verifier(
        boundary: &Header<u32, BlakeTwo256>,
        old: &sp_core::ed25519::Pair,
        fresh: &sp_core::ed25519::Pair,
    ) -> Box<dyn Verifier<Block>> {
        use sp_core::Pair;
        // Construct the SDK's persisted authority-set shape without accessing
        // private fields: authorities, set ID, empty fork tree, forced changes,
        // and change history. Decode fails if its SCALE layout changes.
        let authorities: Vec<(sc_finality_grandpa::AuthorityId, u64)> =
            vec![(old.public().into(), 1)];
        let encoded = (
            authorities,
            7u64,
            Vec::<u8>::new(),
            Option::<u32>::None,
            Vec::<u8>::new(),
            Vec::<(u64, u32)>::new(),
        )
            .encode();
        let authority_set =
            sc_finality_grandpa::AuthoritySet::<H256, u32>::decode_all(&mut encoded.as_slice())
                .unwrap();
        let backend = Arc::new(
            sc_client_db::Backend::<Block>::new(
                sc_client_db::DatabaseSettings {
                    trie_cache_maximum_size: None,
                    state_pruning: None,
                    source: sc_client_db::DatabaseSource::Custom {
                        db: Arc::new(sp_database::MemDb::default()),
                        require_create_flag: true,
                    },
                    blocks_pruning: sc_client_db::BlocksPruning::KeepAll,
                    pruning_filters: vec![],
                    metrics_registry: None,
                },
                4096,
            )
            .unwrap(),
        );
        sc_finality_grandpa::warp_proof::NetworkProvider::new(
            backend,
            authority_set.into(),
            vec![AuthoritySetHardFork {
                set_id: 8,
                block: (boundary.hash(), CHECKPOINT_NUMBER),
                authorities: vec![(fresh.public().into(), 1)],
                last_finalized: Some(CHECKPOINT_NUMBER - 1),
            }],
        )
        .create_verifier()
    }

    fn signed_fragment(
        header: Header<u32, BlakeTwo256>,
        signer: &sp_core::ed25519::Pair,
        set: u64,
    ) -> sc_finality_grandpa::warp_proof::WarpSyncFragment<Block> {
        use sp_core::Pair;
        let precommit = finality_grandpa::Precommit {
            target_hash: header.hash(),
            target_number: *header.number(),
        };
        let message = finality_grandpa::Message::Precommit(precommit.clone());
        // This is sp-consensus-grandpa's localized payload, binding both round
        // and set. Fixed fixture seeds never leave the test process.
        let signature = signer.sign(&(message, 1u64, set).encode());
        let commit = sc_finality_grandpa::Commit::<<Block as BlockT>::Header> {
            target_hash: header.hash(),
            target_number: *header.number(),
            precommits: vec![finality_grandpa::SignedPrecommit {
                precommit,
                signature: signature.into(),
                id: signer.public().into(),
            }],
        };
        let encoded = (1u64, commit, Vec::<<Block as BlockT>::Header>::new()).encode();
        sc_finality_grandpa::warp_proof::WarpSyncFragment {
            header,
            justification: sc_finality_grandpa::GrandpaJustification::<Block>::decode_all(
                &mut encoded.as_slice(),
            )
            .unwrap(),
        }
    }

    #[test]
    fn sdk_warp_boundary_requires_fresh_keys_and_the_sequential_set() {
        use sp_core::Pair;
        let old = sp_core::ed25519::Pair::from_seed(&[21; 32]);
        let fresh = sp_core::ed25519::Pair::from_seed(&[22; 32]);
        let boundary: <Block as BlockT>::Header =
            serde_json::from_str(include_str!("testdata/recovery_checkpoint_header.json")).unwrap();
        let mut target = header(CHECKPOINT_NUMBER + 2);
        target
            .digest_mut()
            .push(DigestItem::Other(common::recovery::lineage_marker(
                fixture().policy_commitment.0,
            )));
        let wrap = |inner| RecoveryWarpVerifier {
            inner,
            recovery: Some(fixture()),
            last_number: None,
            boundary_seen: false,
            evidence: None,
        };
        let skipped = EncodedProof(
            WarpProofWire {
                proofs: vec![signed_fragment(target.clone(), &old, 7)],
                is_finished: true,
            }
            .encode(),
        );
        // The old set can sign a fabricated lineage digest. The mandatory
        // boundary guard rejects it before SDK old-set verification can pass.
        let mut skipped_verifier = wrap(sdk_fixture_verifier(&boundary, &old, &fresh));
        assert!(skipped_verifier.verify(&skipped).is_err());
        assert!(!skipped_verifier.boundary_seen);
        assert!(skipped_verifier.last_number.is_none());
        for (signer, set, accepted) in [(&fresh, 8, true), (&old, 8, false), (&fresh, 7, false)] {
            let proof = EncodedProof(
                WarpProofWire {
                    proofs: vec![
                        trusted_boundary_fragment(boundary.clone()),
                        signed_fragment(target.clone(), signer, set),
                    ],
                    is_finished: true,
                }
                .encode(),
            );
            let mut verifier = wrap(sdk_fixture_verifier(&boundary, &old, &fresh));
            let result = verifier.verify(&proof);
            assert_eq!(result.is_ok(), accepted);
            if accepted {
                assert!(
                    matches!(result.unwrap(), VerificationResult::Complete(head, _) if head == target)
                );
            }
        }
        // A boundary-only partial response can resume with a real fresh-key
        // proof. The wrapper separately forbids completing on that boundary.
        let mut verifier = wrap(sdk_fixture_verifier(&boundary, &old, &fresh));
        let first = EncodedProof(
            WarpProofWire {
                proofs: vec![trusted_boundary_fragment(boundary.clone())],
                is_finished: false,
            }
            .encode(),
        );
        assert!(matches!(
            verifier.verify(&first).unwrap(),
            VerificationResult::Partial(_)
        ));
        assert_eq!(verifier.next_proof_context(), boundary.hash());
        // A peer switch cannot reset the owned SDK verifier to old authorities
        // while retaining the wrapper's boundary marker. A fresh provider
        // verifier, tested above, starts with that marker absent as well.
        assert!(verifier.verify(&skipped).is_err());
        assert_eq!(verifier.next_proof_context(), boundary.hash());
        assert_eq!(verifier.last_number, Some(CHECKPOINT_NUMBER));
        assert!(verifier.boundary_seen);
        let second = EncodedProof(
            WarpProofWire {
                proofs: vec![signed_fragment(target, &fresh, 8)],
                is_finished: true,
            }
            .encode(),
        );
        assert!(matches!(
            verifier.verify(&second).unwrap(),
            VerificationResult::Complete(_, _)
        ));
    }

    #[test]
    fn retired_grandpa_keys_are_rejected_even_on_non_authorities() {
        let recovery = fixture();
        let old = recovery.previous_grandpa_authorities[0]
            .0
            .as_slice()
            .to_vec();
        let fresh = recovery.grandpa_authorities[0].0.as_slice().to_vec();
        assert!(validate_grandpa_keys(&recovery, &[old], false, true).is_err());
        assert!(validate_grandpa_keys(&recovery, &[], true, true).is_err());
        assert!(validate_grandpa_keys(&recovery, &[fresh], true, true).is_ok());
        assert!(validate_grandpa_keys(&recovery, &[vec![3; 32]], false, true).is_err());
        assert!(validate_grandpa_keys(&recovery, &[vec![4; 32]], true, true).is_ok());
        assert!(validate_grandpa_keys(&recovery, &[vec![5; 32]], false, true).is_err());
        assert!(validate_grandpa_keys(&recovery, &[], false, true).is_ok());
        assert!(validate_grandpa_keys(&recovery, &[vec![5; 32]], true, false).is_ok());
        assert!(validate_grandpa_keys(&recovery, &[vec![3; 32]], true, false).is_err());
    }

    #[test]
    fn importer_and_warp_descriptor_use_the_same_sequential_set_id() {
        let recovery = fixture();
        let spec = framenode_chain_spec::ChainSpec::builder(
            &[],
            framenode_chain_spec::recovery::Extensions {
                recovery: Some(recovery.clone()),
            },
        )
        .build();
        for descriptor in [hard_forks(&spec), hard_forks(&spec)] {
            assert_eq!(descriptor.len(), 1);
            assert_eq!(descriptor[0].set_id, recovery.previous_grandpa_set_id + 1);
            assert_eq!(
                descriptor[0].block,
                (CHECKPOINT_HASH.into(), CHECKPOINT_NUMBER)
            );
            assert_eq!(descriptor[0].last_finalized, Some(CHECKPOINT_NUMBER - 1));
            assert_eq!(descriptor[0].authorities, recovery.grandpa_authorities);
        }
    }

    #[test]
    fn sdk_silent_substitute_version_overwrite_is_rejected() {
        let original = framenode_chain_spec::main_net().unwrap();
        let mut json: serde_json::Value =
            serde_json::from_str(&sc_service::ChainSpec::as_json(&original, false).unwrap())
                .unwrap();
        let substitutes = json["codeSubstitutes"].as_object_mut().unwrap();
        let historical = substitutes["11271782"].clone();
        substitutes.insert("11271783".into(), historical);
        let invalid =
            framenode_chain_spec::ChainSpec::from_json_bytes(serde_json::to_vec(&json).unwrap())
                .unwrap();
        assert!(validate_spec(&invalid)
            .unwrap_err()
            .contains("duplicate substitute spec version"));
    }
}
