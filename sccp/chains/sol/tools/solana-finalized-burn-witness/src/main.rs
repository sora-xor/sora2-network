use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs,
    io::BufReader,
    net::TcpStream,
    path::PathBuf,
    str::FromStr,
    time::Duration,
};

use anyhow::{bail, ensure, Context, Result};
use base64::Engine as _;
use borsh::BorshDeserialize;
use clap::{Args, Parser, ValueEnum};
use parity_scale_codec::Encode;
use serde::{Deserialize, Serialize};
use solana_rpc_client::rpc_client::RpcClient;
use solana_rpc_client_api::config::RpcBlockConfig;
use solana_sdk::{
    commitment_config::CommitmentConfig, hash::Hash, message::VersionedMessage, pubkey::Pubkey,
    sysvar::slot_hashes::SlotHashes,
    vote::{
        instruction::VoteInstruction,
        state::{Lockout, Vote, VoteStateUpdate},
    },
};
use solana_transaction_status::{TransactionDetails, UiConfirmedBlock, UiTransactionEncoding};

const SLOT_HASHES_SYSVAR_ID: &str = "SysvarS1otHashes111111111111111111111111111";
const SCCP_SEED_PREFIX: &[u8] = b"sccp";
const SCCP_SEED_BURN: &[u8] = b"burn";

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Build a Solana -> SORA finalized burn proof from a Geyser account-proof stream and RPC vote blocks"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand, Debug)]
enum Commands {
    BuildProof(BuildProofArgs),
}

#[derive(Clone, Debug, ValueEnum)]
enum StdoutFormat {
    Hex,
    Base64,
    Both,
    None,
}

#[derive(Args, Debug)]
struct BuildProofArgs {
    #[arg(long)]
    router_program_id: String,
    #[arg(long)]
    message_id: String,
    #[arg(long, default_value = "http://127.0.0.1:8899")]
    rpc_url: String,
    #[arg(long)]
    geyser_addr: Option<String>,
    #[arg(long)]
    update_file: Vec<PathBuf>,
    #[arg(long, default_value_t = 120)]
    timeout_secs: u64,
    #[arg(long)]
    authority_set_json: Option<PathBuf>,
    #[arg(long)]
    threshold_stake: Option<u64>,
    #[arg(long)]
    json_output: Option<PathBuf>,
    #[arg(long)]
    proof_output: Option<PathBuf>,
    #[arg(long, value_enum, default_value = "both")]
    stdout_format: StdoutFormat,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthorityStakeEntry {
    authority_pubkey: String,
    stake: u64,
}

#[derive(Clone, Debug, BorshDeserialize)]
struct GeyserProof {
    path: Vec<usize>,
    siblings: Vec<Vec<Hash>>,
}

#[derive(Clone, Debug, BorshDeserialize)]
struct GeyserData {
    pubkey: Pubkey,
    hash: Hash,
    account: GeyserAccountInfo,
}

#[derive(Clone, Debug, BorshDeserialize)]
struct GeyserAccountDeltaProof(pub Pubkey, pub (GeyserData, GeyserProof));

#[derive(Clone, Debug, BorshDeserialize)]
struct GeyserBankHashProof {
    proofs: Vec<GeyserAccountDeltaProof>,
    num_sigs: u64,
    parent_slot: u64,
    account_delta_root: Hash,
    parent_bankhash: Hash,
    blockhash: Hash,
}

#[derive(Clone, Debug, BorshDeserialize)]
struct GeyserUpdate {
    slot: u64,
    root: Hash,
    proof: GeyserBankHashProof,
}

#[derive(Clone, Debug, BorshDeserialize)]
struct GeyserAccountInfo {
    pubkey: Pubkey,
    lamports: u64,
    owner: Pubkey,
    executable: bool,
    rent_epoch: u64,
    data: Vec<u8>,
    write_version: u64,
    slot: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum DecodedVoteInstruction {
    Vote(Vote),
    VoteSwitch(Vote, Hash),
    UpdateVoteState(VoteStateUpdate),
    UpdateVoteStateSwitch(VoteStateUpdate, Hash),
    CompactUpdateVoteState(VoteStateUpdate),
    CompactUpdateVoteStateSwitch(VoteStateUpdate, Hash),
    TowerSync(TowerSyncVote),
    TowerSyncSwitch(TowerSyncVote, Hash),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct VoteSwitchFields(Vote, Hash);

#[derive(Clone, Debug, Serialize, Deserialize)]
struct VoteStateUpdateSwitchFields(VoteStateUpdate, Hash);

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CompactLockoutOffset {
    offset: u64,
    confirmation_count: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CompactVoteStateUpdateFields {
    root: u64,
    lockout_offsets: Vec<CompactLockoutOffset>,
    hash: Hash,
    timestamp: Option<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CompactVoteStateUpdateSwitchFields(CompactVoteStateUpdateFields, Hash);

#[derive(Clone, Debug, Serialize, Deserialize)]
struct TowerSyncVote {
    lockouts: VecDeque<Lockout>,
    root: Option<u64>,
    hash: Hash,
    timestamp: Option<i64>,
    block_id: Hash,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CompactTowerSyncFields {
    root: u64,
    lockout_offsets: Vec<CompactLockoutOffset>,
    hash: Hash,
    timestamp: Option<i64>,
    block_id: Hash,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CompactTowerSyncSwitchFields(CompactTowerSyncFields, Hash);

#[derive(Clone, Debug, Encode)]
struct ScaleH256([u8; 32]);

#[derive(Clone, Debug, Encode)]
struct ScaleSolanaFinalizedBurnPublicInputsV1 {
    message_id: ScaleH256,
    finalized_slot: u64,
    finalized_bank_hash: ScaleH256,
    finalized_slot_hash: ScaleH256,
    router_program_id: [u8; 32],
    burn_record_pda: [u8; 32],
    burn_record_owner: [u8; 32],
    burn_record_data_hash: ScaleH256,
}

#[derive(Clone, Debug, Encode)]
struct ScaleSolanaMerkleProofV1 {
    path: Vec<u8>,
    siblings: Vec<Vec<ScaleH256>>,
}

#[derive(Clone, Debug, Encode)]
struct ScaleSolanaAccountInfoV1 {
    pubkey: [u8; 32],
    lamports: u64,
    owner: [u8; 32],
    executable: bool,
    rent_epoch: u64,
    data: Vec<u8>,
    write_version: u64,
    slot: u64,
}

#[derive(Clone, Debug, Encode)]
struct ScaleSolanaAccountDeltaProofV1 {
    account: ScaleSolanaAccountInfoV1,
    merkle_proof: ScaleSolanaMerkleProofV1,
}

#[derive(Clone, Debug, Encode)]
struct ScaleSolanaBankHashProofV1 {
    slot: u64,
    bank_hash: ScaleH256,
    account_delta_root: ScaleH256,
    parent_bank_hash: ScaleH256,
    blockhash: ScaleH256,
    num_sigs: u64,
    account_proof: ScaleSolanaAccountDeltaProofV1,
}

#[derive(Clone, Debug, Encode)]
struct ScaleSolanaVoteProofV1 {
    authority_pubkey: [u8; 32],
    signature: [u8; 64],
    signed_message: Vec<u8>,
    vote_slot: u64,
    vote_bank_hash: ScaleH256,
    rooted_slot: Option<u64>,
    slot_hashes_proof: ScaleSolanaBankHashProofV1,
}

#[derive(Clone, Debug, Encode)]
struct ScaleSolanaFinalizedBurnProofV1 {
    version: u8,
    public_inputs: ScaleSolanaFinalizedBurnPublicInputsV1,
    burn_proof: ScaleSolanaBankHashProofV1,
    vote_proofs: Vec<ScaleSolanaVoteProofV1>,
}

#[derive(Clone, Debug)]
struct BurnCapture {
    finalized_slot: u64,
    finalized_bank_hash: [u8; 32],
    finalized_slot_hash: Option<[u8; 32]>,
    burn_proof: ScaleSolanaBankHashProofV1,
    public_inputs: ScaleSolanaFinalizedBurnPublicInputsV1,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProofSummary {
    message_id: String,
    router_program_id: String,
    burn_record_pda: String,
    finalized_slot: u64,
    finalized_bank_hash: String,
    finalized_slot_hash: String,
    vote_proof_count: usize,
    authority_count: usize,
    collected_stake: Option<u64>,
    threshold_stake: Option<u64>,
}

#[derive(Default)]
struct ProofBuildState {
    burn: Option<BurnCapture>,
    vote_proofs: Vec<ScaleSolanaVoteProofV1>,
    seen_vote_signatures: BTreeSet<[u8; 64]>,
    seen_authorities: BTreeSet<[u8; 32]>,
    slot_hashes_proofs: BTreeMap<u64, ScaleSolanaBankHashProofV1>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::BuildProof(args) => run_build_proof(args),
    }
}

fn run_build_proof(args: BuildProofArgs) -> Result<()> {
    ensure!(
        args.geyser_addr.is_some() || !args.update_file.is_empty(),
        "provide either --geyser-addr or at least one --update-file",
    );

    let router_program_id =
        Pubkey::from_str(&args.router_program_id).context("invalid --router-program-id")?;
    let message_id = parse_hex_32(&args.message_id).context("invalid --message-id")?;
    let (burn_record_pda, _) = Pubkey::find_program_address(
        &[SCCP_SEED_PREFIX, SCCP_SEED_BURN, &message_id],
        &router_program_id,
    );

    let authority_set = if let Some(path) = &args.authority_set_json {
        Some(load_authority_set(path)?)
    } else {
        None
    };
    let threshold_stake = match (&authority_set, args.threshold_stake) {
        (_, Some(explicit)) => Some(explicit),
        (Some(entries), None) => {
            let total = entries
                .values()
                .try_fold(0u64, |acc, stake| acc.checked_add(*stake))
                .context("authority stake total overflow")?;
            Some(
                total
                    .checked_mul(2)
                    .and_then(|v| v.checked_div(3))
                    .and_then(|v| v.checked_add(1))
                    .context("authority threshold overflow")?,
            )
        }
        (None, None) => None,
    };

    let rpc = RpcClient::new_with_commitment(args.rpc_url.clone(), CommitmentConfig::confirmed());
    let slot_hashes_sysvar = Pubkey::from_str(SLOT_HASHES_SYSVAR_ID)
        .expect("constant SlotHashes sysvar pubkey is valid");
    let mut state = ProofBuildState::default();

    process_updates(&args, |update| {
        let current_slot_hashes_proof =
            maybe_capture_slot_hashes_proof(&update, &slot_hashes_sysvar)?;
        if let Some(ref slot_hashes_proof) = current_slot_hashes_proof {
            eprintln!(
                "cached SlotHashes proof for slot {} (bank_hash={})",
                slot_hashes_proof.slot,
                prefixed_hex(&slot_hashes_proof.bank_hash.0)
            );
            state
                .slot_hashes_proofs
                .entry(slot_hashes_proof.slot)
                .or_insert_with(|| slot_hashes_proof.clone());
        }

        if state.burn.is_none() {
            if let Some(capture) =
                maybe_capture_burn(&update, &router_program_id, &burn_record_pda, &message_id)?
            {
                eprintln!(
                    "captured burn witness at slot {} for burn record {}",
                    capture.finalized_slot, burn_record_pda
                );
                state.burn = Some(capture);
            }
        }

        if let (Some(burn), Some(slot_hashes_proof)) =
            (state.burn.as_mut(), current_slot_hashes_proof.as_ref())
        {
            if maybe_capture_finalized_slot_hash(burn, slot_hashes_proof)? {
                eprintln!(
                    "captured finalized slot hash for slot {} as {} from SlotHashes proof at slot {}",
                    burn.finalized_slot,
                    prefixed_hex(
                        burn.finalized_slot_hash
                            .as_ref()
                            .expect("captured slot hash is populated"),
                    ),
                    slot_hashes_proof.slot,
                );
            }
        }

        if let (Some(burn), Some(slot_hashes_proof)) = (&state.burn, current_slot_hashes_proof.as_ref()) {
            let mut vote_proofs =
                maybe_capture_vote_proofs(&rpc, update.slot, burn, slot_hashes_proof)?;
            for vote_proof in vote_proofs.drain(..) {
                if state.seen_vote_signatures.insert(vote_proof.signature) {
                    state.seen_authorities.insert(vote_proof.authority_pubkey);
                    state.vote_proofs.push(vote_proof);
                    eprintln!(
                        "captured vote proof at slot {} (total={})",
                        update.slot,
                        state.vote_proofs.len()
                    );
                }
            }

            if proof_complete(&state, authority_set.as_ref(), threshold_stake) {
                eprintln!(
                    "proof complete with burn slot {} and {} vote proofs",
                    burn.finalized_slot,
                    state.vote_proofs.len()
                );
                return Ok(true);
            }
        }
        Ok(false)
    })?;

    let burn = state
        .burn
        .clone()
        .context("did not capture a matching burn-record inclusion proof")?;
    ensure!(
        burn.finalized_slot_hash.is_some(),
        "did not capture a canonical finalized slot hash for the burn slot",
    );
    ensure!(
        !state.vote_proofs.is_empty(),
        "did not capture any qualifying vote proofs after observing the burn slot",
    );
    if let (Some(authority_set), Some(threshold_stake)) = (&authority_set, threshold_stake) {
        let collected_stake = collected_stake(&state, authority_set)?;
        ensure!(
            collected_stake >= threshold_stake,
            "collected vote proofs only cover {} stake but threshold is {}",
            collected_stake,
            threshold_stake,
        );
    }

    let proof = ScaleSolanaFinalizedBurnProofV1 {
        version: sccp_sol::SOLANA_FINALIZED_BURN_PROOF_VERSION_V1,
        public_inputs: burn.public_inputs.clone(),
        burn_proof: burn.burn_proof.clone(),
        vote_proofs: state.vote_proofs.clone(),
    };
    let proof_bytes = proof.encode();

    if let Some(path) = &args.proof_output {
        fs::write(path, &proof_bytes)
            .with_context(|| format!("failed to write proof bytes to {}", path.display()))?;
    }

    if let Some(path) = &args.json_output {
        let summary = build_summary(
            &burn,
            &state,
            authority_set.as_ref(),
            threshold_stake,
            burn_record_pda.to_bytes(),
            message_id,
            router_program_id.to_bytes(),
        )?;
        fs::write(path, serde_json::to_vec_pretty(&summary)?)
            .with_context(|| format!("failed to write summary JSON to {}", path.display()))?;
    }

    match args.stdout_format {
        StdoutFormat::Hex => {
            println!("hex=0x{}", hex::encode(&proof_bytes));
        }
        StdoutFormat::Base64 => {
            println!(
                "base64={}",
                base64::engine::general_purpose::STANDARD.encode(&proof_bytes)
            );
        }
        StdoutFormat::Both => {
            println!("hex=0x{}", hex::encode(&proof_bytes));
            println!(
                "base64={}",
                base64::engine::general_purpose::STANDARD.encode(&proof_bytes)
            );
        }
        StdoutFormat::None => {}
    }

    Ok(())
}

fn build_summary(
    burn: &BurnCapture,
    state: &ProofBuildState,
    authority_set: Option<&BTreeMap<[u8; 32], u64>>,
    threshold_stake: Option<u64>,
    burn_record_pda: [u8; 32],
    message_id: [u8; 32],
    router_program_id: [u8; 32],
) -> Result<ProofSummary> {
    Ok(ProofSummary {
        message_id: prefixed_hex(&message_id),
        router_program_id: Pubkey::new_from_array(router_program_id).to_string(),
        burn_record_pda: Pubkey::new_from_array(burn_record_pda).to_string(),
        finalized_slot: burn.finalized_slot,
        finalized_bank_hash: prefixed_hex(&burn.finalized_bank_hash),
        finalized_slot_hash: prefixed_hex(
            burn.finalized_slot_hash
                .as_ref()
                .expect("finalized slot hash is populated before summary"),
        ),
        vote_proof_count: state.vote_proofs.len(),
        authority_count: state.seen_authorities.len(),
        collected_stake: authority_set
            .map(|authority_set| collected_stake(state, authority_set))
            .transpose()?,
        threshold_stake,
    })
}

fn proof_complete(
    state: &ProofBuildState,
    authority_set: Option<&BTreeMap<[u8; 32], u64>>,
    threshold_stake: Option<u64>,
) -> bool {
    if state
        .burn
        .as_ref()
        .is_none_or(|burn| burn.finalized_slot_hash.is_none())
        || state.vote_proofs.is_empty()
    {
        return false;
    }
    match (authority_set, threshold_stake) {
        (Some(authority_set), Some(threshold_stake)) => collected_stake(state, authority_set)
            .map(|stake| stake >= threshold_stake)
            .unwrap_or(false),
        _ => true,
    }
}

fn collected_stake(
    state: &ProofBuildState,
    authority_set: &BTreeMap<[u8; 32], u64>,
) -> Result<u64> {
    state
        .seen_authorities
        .iter()
        .try_fold(0u64, |acc, authority| {
            let Some(stake) = authority_set.get(authority) else {
                return Ok(acc);
            };
            acc.checked_add(*stake).context("collected stake overflow")
        })
}

fn maybe_capture_finalized_slot_hash(
    burn: &mut BurnCapture,
    slot_hashes_proof: &ScaleSolanaBankHashProofV1,
) -> Result<bool> {
    if burn.finalized_slot_hash.is_some() {
        return Ok(false);
    }

    let slot_hashes: SlotHashes = bincode::deserialize(&slot_hashes_proof.account_proof.account.data)
        .context("failed to decode SlotHashes sysvar account from proof")?;
    let Some(finalized_slot_hash) = slot_hashes.get(&burn.finalized_slot).map(|hash| hash.to_bytes())
    else {
        return Ok(false);
    };
    burn.finalized_slot_hash = Some(finalized_slot_hash);
    burn.public_inputs.finalized_slot_hash = ScaleH256(finalized_slot_hash);
    Ok(true)
}

fn load_authority_set(path: &PathBuf) -> Result<BTreeMap<[u8; 32], u64>> {
    let raw = fs::read(path)
        .with_context(|| format!("failed to read authority set JSON from {}", path.display()))?;
    let entries: Vec<AuthorityStakeEntry> =
        serde_json::from_slice(&raw).context("invalid authority set JSON")?;
    let mut out = BTreeMap::new();
    for entry in entries {
        let pubkey = Pubkey::from_str(&entry.authority_pubkey)
            .with_context(|| format!("invalid authority pubkey {}", entry.authority_pubkey))?;
        ensure!(
            out.insert(pubkey.to_bytes(), entry.stake).is_none(),
            "duplicate authority {} in authority set",
            entry.authority_pubkey,
        );
    }
    Ok(out)
}

fn process_updates<F>(args: &BuildProofArgs, mut on_update: F) -> Result<()>
where
    F: FnMut(GeyserUpdate) -> Result<bool>,
{
    if !args.update_file.is_empty() {
        for path in &args.update_file {
            let bytes = fs::read(path)
                .with_context(|| format!("failed to read update file {}", path.display()))?;
            let update = GeyserUpdate::try_from_slice(&bytes).with_context(|| {
                format!("failed to decode Borsh update from {}", path.display())
            })?;
            if on_update(update)? {
                return Ok(());
            }
        }
        return Ok(());
    }

    let geyser_addr = args
        .geyser_addr
        .as_ref()
        .context("missing --geyser-addr for streaming mode")?;
    let stream = TcpStream::connect(geyser_addr)
        .with_context(|| format!("failed to connect to {geyser_addr}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(args.timeout_secs)))
        .context("failed to set geyser stream read timeout")?;
    let mut reader = BufReader::new(stream);
    loop {
        match GeyserUpdate::deserialize_reader(&mut reader) {
            Ok(update) => {
                if on_update(update)? {
                    return Ok(());
                }
            }
            Err(err) if is_timeout_error(&err) => {
                bail!("timed out waiting for enough witness updates from the geyser stream");
            }
            Err(err) => {
                bail!("failed to decode Borsh update from geyser stream: {err}");
            }
        }
    }
}

fn maybe_capture_burn(
    update: &GeyserUpdate,
    router_program_id: &Pubkey,
    burn_record_pda: &Pubkey,
    message_id: &[u8; 32],
) -> Result<Option<BurnCapture>> {
    let Some(account_proof) = find_account_delta_proof(update, burn_record_pda) else {
        return Ok(None);
    };
    let data = &account_proof.1 .0;
    ensure!(
        data.account.owner == *router_program_id,
        "burn record account owner {} did not match router program {}",
        data.account.owner,
        router_program_id,
    );
    let burn_record =
        sccp_sol::decode_solana_burn_record_account_v1(&data.account.data).map_err(|err| {
            anyhow::anyhow!("failed to decode canonical SCCP burn-record account: {err:?}")
        })?;
    ensure!(
        &burn_record.message_id == message_id,
        "burn record messageId {} did not match requested messageId {}",
        prefixed_hex(&burn_record.message_id),
        prefixed_hex(message_id),
    );
    ensure!(
        burn_record.slot == update.slot,
        "burn record slot {} did not match confirmed slot {}",
        burn_record.slot,
        update.slot,
    );

    let burn_proof = convert_bank_hash_proof(update, account_proof)?;
    Ok(Some(BurnCapture {
        finalized_slot: update.slot,
        finalized_bank_hash: update.root.to_bytes(),
        finalized_slot_hash: None,
        public_inputs: ScaleSolanaFinalizedBurnPublicInputsV1 {
            message_id: ScaleH256(*message_id),
            finalized_slot: update.slot,
            finalized_bank_hash: ScaleH256(update.root.to_bytes()),
            finalized_slot_hash: ScaleH256([0u8; 32]),
            router_program_id: router_program_id.to_bytes(),
            burn_record_pda: burn_record_pda.to_bytes(),
            burn_record_owner: router_program_id.to_bytes(),
            burn_record_data_hash: ScaleH256(sccp_sol::solana_burn_record_data_hash(
                &data.account.data,
            )),
        },
        burn_proof,
    }))
}

fn maybe_capture_vote_proofs(
    rpc: &RpcClient,
    block_slot: u64,
    burn: &BurnCapture,
    slot_hashes_proof: &ScaleSolanaBankHashProofV1,
) -> Result<Vec<ScaleSolanaVoteProofV1>> {
    if block_slot < burn.finalized_slot {
        return Ok(Vec::new());
    }
    let Some(finalized_slot_hash) = burn.finalized_slot_hash.as_ref() else {
        return Ok(Vec::new());
    };
    let block = rpc
        .get_block_with_config(
            block_slot,
            RpcBlockConfig {
                encoding: Some(UiTransactionEncoding::Base64),
                transaction_details: Some(TransactionDetails::Full),
                rewards: Some(false),
                commitment: Some(CommitmentConfig::confirmed()),
                max_supported_transaction_version: Some(0),
            },
        )
        .with_context(|| format!("failed to fetch block {} from RPC", block_slot))?;
    build_vote_proofs_from_block(
        block,
        block_slot,
        burn.finalized_slot,
        finalized_slot_hash,
        slot_hashes_proof,
    )
}

fn maybe_capture_slot_hashes_proof(
    update: &GeyserUpdate,
    slot_hashes_sysvar: &Pubkey,
) -> Result<Option<ScaleSolanaBankHashProofV1>> {
    let Some(slot_hashes_account_proof) = find_account_delta_proof(update, slot_hashes_sysvar) else {
        return Ok(None);
    };

    let proof = convert_bank_hash_proof(update, slot_hashes_account_proof)?;
    let _: SlotHashes = bincode::deserialize(&proof.account_proof.account.data)
        .context("failed to decode SlotHashes sysvar account from geyser proof")?;

    Ok(Some(proof))
}

fn build_vote_proofs_from_block(
    block: UiConfirmedBlock,
    block_slot: u64,
    target_burn_slot: u64,
    target_burn_slot_hash: &[u8; 32],
    slot_hashes_proof: &ScaleSolanaBankHashProofV1,
) -> Result<Vec<ScaleSolanaVoteProofV1>> {
    let vote_program = solana_sdk::vote::program::id();
    let slot_hashes: SlotHashes = bincode::deserialize(&slot_hashes_proof.account_proof.account.data)
        .context("failed to decode SlotHashes sysvar account from proof")?;
    let mut out = Vec::new();
    let mut total_vote_txs = 0usize;
    let mut before_target = 0usize;
    let mut root_before_target = 0usize;
    let mut slot_hash_missing = 0usize;
    let mut hash_mismatch = 0usize;
    let mut decode_fail = 0usize;
    let mut unsupported_variant = 0usize;
    let mut authority_lookup_fail = 0usize;
    let mut signature_lookup_fail = 0usize;
    for encoded_tx in block.transactions.unwrap_or_default() {
        let Some(transaction) = encoded_tx.transaction.decode() else {
            continue;
        };
        let VersionedMessage::Legacy(message) = transaction.message else {
            continue;
        };
        let Some(instruction) = message.instructions.first() else {
            continue;
        };
        let Some(program_id) = message
            .account_keys
            .get(instruction.program_id_index as usize)
        else {
            continue;
        };
        if *program_id != vote_program {
            continue;
        }
        total_vote_txs += 1;

        let Ok(vote_instruction) = decode_vote_instruction(&instruction.data) else {
            decode_fail += 1;
            continue;
        };
        let Some((
            vote_slot,
            vote_hash,
            rooted_slot,
            authority_account_index,
        )) = extract_vote_details(&vote_instruction)
        else {
            unsupported_variant += 1;
            continue;
        };
        if vote_slot < target_burn_slot {
            before_target += 1;
            continue;
        }
        if rooted_slot.is_some_and(|root| root < target_burn_slot) {
            root_before_target += 1;
            continue;
        }
        let Some(voted_slot_hash) = slot_hashes.get(&vote_slot) else {
            slot_hash_missing += 1;
            continue;
        };
        if voted_slot_hash.to_bytes() != vote_hash.to_bytes() {
            hash_mismatch += 1;
            continue;
        }
        let Some(target_slot_hash) = slot_hashes.get(&target_burn_slot) else {
            slot_hash_missing += 1;
            continue;
        };
        if target_slot_hash.to_bytes() != *target_burn_slot_hash {
            hash_mismatch += 1;
            continue;
        }

        let Some(authority_pubkey) = instruction
            .accounts
            .get(authority_account_index)
            .and_then(|index| message.account_keys.get(usize::from(*index)))
        else {
            authority_lookup_fail += 1;
            continue;
        };
        let signer_limit = usize::from(message.header.num_required_signatures);
        let Some(signature_index) = message.account_keys[..signer_limit]
            .iter()
            .position(|candidate| candidate == authority_pubkey)
        else {
            signature_lookup_fail += 1;
            continue;
        };
        let Some(signature) = transaction.signatures.get(signature_index) else {
            continue;
        };

        let signature_bytes: [u8; 64] = signature.as_ref().try_into().map_err(|_| {
            anyhow::anyhow!(
                "unexpected Solana signature length {}",
                signature.as_ref().len()
            )
        })?;

        out.push(ScaleSolanaVoteProofV1 {
            authority_pubkey: authority_pubkey.to_bytes(),
            signature: signature_bytes,
            signed_message: message.serialize(),
            vote_slot,
            vote_bank_hash: ScaleH256(vote_hash.to_bytes()),
            rooted_slot,
            slot_hashes_proof: slot_hashes_proof.clone(),
        });
    }
    if total_vote_txs > 0 {
        eprintln!(
            "scanned vote block {}: total={} matched={} before_target={} root_before_target={} missing_slot_hash_proof={} hash_mismatch={} decode_fail={} unsupported_variant={} authority_lookup_fail={} signature_lookup_fail={}",
            block_slot,
            total_vote_txs,
            out.len(),
            before_target,
            root_before_target,
            slot_hash_missing,
            hash_mismatch,
            decode_fail,
            unsupported_variant,
            authority_lookup_fail,
            signature_lookup_fail,
        );
    }
    Ok(out)
}

fn decode_vote_instruction(data: &[u8]) -> Result<DecodedVoteInstruction> {
    if data.len() < 4 {
        bail!("vote instruction data is truncated");
    }

    let mut discriminant = [0u8; 4];
    discriminant.copy_from_slice(&data[..4]);
    let payload = &data[4..];
    match u32::from_le_bytes(discriminant) {
        2 => bincode::deserialize::<Vote>(payload)
            .map(DecodedVoteInstruction::Vote)
            .context("failed to decode Vote instruction"),
        6 => bincode::deserialize::<VoteSwitchFields>(payload)
            .map(|fields| DecodedVoteInstruction::VoteSwitch(fields.0, fields.1))
            .context("failed to decode VoteSwitch instruction"),
        8 => bincode::deserialize::<VoteStateUpdate>(payload)
            .map(DecodedVoteInstruction::UpdateVoteState)
            .context("failed to decode UpdateVoteState instruction"),
        9 => bincode::deserialize::<VoteStateUpdateSwitchFields>(payload)
            .map(|fields| DecodedVoteInstruction::UpdateVoteStateSwitch(fields.0, fields.1))
            .context("failed to decode UpdateVoteStateSwitch instruction"),
        12 => parse_compact_vote_state_update(payload).map(DecodedVoteInstruction::CompactUpdateVoteState),
        13 => {
            let (update, hash) = parse_compact_vote_state_update_switch(payload)?;
            Ok(DecodedVoteInstruction::CompactUpdateVoteStateSwitch(update, hash))
        }
        14 => parse_compact_tower_sync(payload).map(DecodedVoteInstruction::TowerSync),
        15 => {
            let (tower_sync, hash) = parse_compact_tower_sync_switch(payload)?;
            Ok(DecodedVoteInstruction::TowerSyncSwitch(tower_sync, hash))
        }
        _ => bail!("unsupported vote instruction discriminant"),
    }
}

fn parse_compact_vote_state_update(payload: &[u8]) -> Result<VoteStateUpdate> {
    let mut offset = 0usize;
    let root_raw = take_u64_le(payload, &mut offset, "compact_vote_root")?;
    let lockouts_len = usize::from(decode_short_vec_len(payload, &mut offset)?);
    let mut previous = if root_raw == u64::MAX { 0 } else { root_raw };
    let mut lockouts = VecDeque::new();
    for _ in 0..lockouts_len {
        let delta = decode_varint_u64(payload, &mut offset, "compact_vote_lockout_offset")?;
        let confirmation_count = take_u8(payload, &mut offset, "compact_vote_confirmation_count")?;
        previous = previous
            .checked_add(delta)
            .context("invalid compact vote-state lockout offset")?;
        lockouts.push_back(Lockout::new_with_confirmation_count(
            previous,
            u32::from(confirmation_count),
        ));
    }
    let hash = Hash::new_from_array(take_array::<32>(payload, &mut offset, "compact_vote_hash")?);
    let timestamp = take_option_i64(payload, &mut offset, "compact_vote_timestamp")?;
    ensure!(
        offset == payload.len(),
        "compact vote-state payload has trailing bytes",
    );
    Ok(VoteStateUpdate {
        lockouts,
        root: (root_raw != u64::MAX).then_some(root_raw),
        hash,
        timestamp,
    })
}

fn parse_compact_vote_state_update_switch(payload: &[u8]) -> Result<(VoteStateUpdate, Hash)> {
    let update = parse_compact_vote_state_update_prefix(payload)?;
    let mut offset = update.1;
    let switch_hash =
        Hash::new_from_array(take_array::<32>(payload, &mut offset, "compact_vote_switch_hash")?);
    ensure!(
        offset == payload.len(),
        "compact vote-state switch payload has trailing bytes",
    );
    Ok((update.0, switch_hash))
}

fn parse_compact_vote_state_update_prefix(payload: &[u8]) -> Result<(VoteStateUpdate, usize)> {
    let mut offset = 0usize;
    let root_raw = take_u64_le(payload, &mut offset, "compact_vote_root")?;
    let lockouts_len = usize::from(decode_short_vec_len(payload, &mut offset)?);
    let mut previous = if root_raw == u64::MAX { 0 } else { root_raw };
    let mut lockouts = VecDeque::new();
    for _ in 0..lockouts_len {
        let delta = decode_varint_u64(payload, &mut offset, "compact_vote_lockout_offset")?;
        let confirmation_count = take_u8(payload, &mut offset, "compact_vote_confirmation_count")?;
        previous = previous
            .checked_add(delta)
            .context("invalid compact vote-state lockout offset")?;
        lockouts.push_back(Lockout::new_with_confirmation_count(
            previous,
            u32::from(confirmation_count),
        ));
    }
    let hash = Hash::new_from_array(take_array::<32>(payload, &mut offset, "compact_vote_hash")?);
    let timestamp = take_option_i64(payload, &mut offset, "compact_vote_timestamp")?;
    Ok((
        VoteStateUpdate {
            lockouts,
            root: (root_raw != u64::MAX).then_some(root_raw),
            hash,
            timestamp,
        },
        offset,
    ))
}

fn parse_compact_tower_sync(payload: &[u8]) -> Result<TowerSyncVote> {
    let (tower_sync, offset) = parse_compact_tower_sync_prefix(payload)?;
    ensure!(
        offset == payload.len(),
        "tower-sync payload has trailing bytes",
    );
    Ok(tower_sync)
}

fn parse_compact_tower_sync_switch(payload: &[u8]) -> Result<(TowerSyncVote, Hash)> {
    let (tower_sync, mut offset) = parse_compact_tower_sync_prefix(payload)?;
    let switch_hash =
        Hash::new_from_array(take_array::<32>(payload, &mut offset, "tower_sync_switch_hash")?);
    ensure!(
        offset == payload.len(),
        "tower-sync switch payload has trailing bytes",
    );
    Ok((tower_sync, switch_hash))
}

fn parse_compact_tower_sync_prefix(payload: &[u8]) -> Result<(TowerSyncVote, usize)> {
    let mut offset = 0usize;
    let root_raw = take_u64_le(payload, &mut offset, "tower_sync_root")?;
    let lockouts_len = usize::from(decode_short_vec_len(payload, &mut offset)?);
    let mut previous = if root_raw == u64::MAX { 0 } else { root_raw };
    let mut lockouts = VecDeque::new();
    for _ in 0..lockouts_len {
        let delta = decode_varint_u64(payload, &mut offset, "tower_sync_lockout_offset")?;
        let confirmation_count = take_u8(payload, &mut offset, "tower_sync_confirmation_count")?;
        previous = previous
            .checked_add(delta)
            .context("invalid compact tower-sync lockout offset")?;
        lockouts.push_back(Lockout::new_with_confirmation_count(
            previous,
            u32::from(confirmation_count),
        ));
    }
    let hash = Hash::new_from_array(take_array::<32>(payload, &mut offset, "tower_sync_hash")?);
    let timestamp = take_option_i64(payload, &mut offset, "tower_sync_timestamp")?;
    let block_id =
        Hash::new_from_array(take_array::<32>(payload, &mut offset, "tower_sync_block_id")?);
    Ok((
        TowerSyncVote {
            lockouts,
            root: (root_raw != u64::MAX).then_some(root_raw),
            hash,
            timestamp,
            block_id,
        },
        offset,
    ))
}

fn take_u8(input: &[u8], offset: &mut usize, label: &str) -> Result<u8> {
    let value = input
        .get(*offset)
        .copied()
        .with_context(|| format!("vote instruction truncated while reading {label}"))?;
    *offset += 1;
    Ok(value)
}

fn take_array<const N: usize>(input: &[u8], offset: &mut usize, label: &str) -> Result<[u8; N]> {
    let end = offset
        .checked_add(N)
        .with_context(|| format!("vote instruction offset overflow while reading {label}"))?;
    let slice = input
        .get(*offset..end)
        .with_context(|| format!("vote instruction truncated while reading {label}"))?;
    let mut out = [0u8; N];
    out.copy_from_slice(slice);
    *offset = end;
    Ok(out)
}

fn take_u64_le(input: &[u8], offset: &mut usize, label: &str) -> Result<u64> {
    Ok(u64::from_le_bytes(take_array::<8>(input, offset, label)?))
}

fn take_i64_le(input: &[u8], offset: &mut usize, label: &str) -> Result<i64> {
    Ok(i64::from_le_bytes(take_array::<8>(input, offset, label)?))
}

fn take_option_i64(input: &[u8], offset: &mut usize, label: &str) -> Result<Option<i64>> {
    match take_u8(input, offset, label)? {
        0 => Ok(None),
        1 => take_i64_le(input, offset, label).map(Some),
        tag => bail!("invalid option tag {} while reading {}", tag, label),
    }
}

fn decode_short_vec_len(input: &[u8], offset: &mut usize) -> Result<u16> {
    let mut value = 0u16;
    for byte_index in 0..3 {
        let byte = take_u8(input, offset, "short_vec_len")?;
        let low_bits = u16::from(byte & 0x7f);
        value = value
            .checked_add(
                low_bits
                    .checked_shl(u32::try_from(byte_index * 7).unwrap_or(u32::MAX))
                    .context("short_vec length shift overflow")?,
            )
            .context("short_vec length overflow")?;
        if byte & 0x80 == 0 {
            ensure!(
                !(byte_index > 0 && low_bits == 0),
                "short_vec uses a non-canonical alias encoding",
            );
            return Ok(value);
        }
    }
    bail!("short_vec length exceeds three bytes")
}

fn decode_varint_u64(input: &[u8], offset: &mut usize, label: &str) -> Result<u64> {
    let mut value = 0u64;
    for byte_index in 0..10 {
        let byte = take_u8(input, offset, label)?;
        let low_bits = u64::from(byte & 0x7f);
        value = value
            .checked_add(
                low_bits
                    .checked_shl(u32::try_from(byte_index * 7).unwrap_or(u32::MAX))
                    .context("varint shift overflow")?,
            )
            .context("varint overflow")?;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    bail!("varint exceeds ten bytes")
}

fn extract_vote_details(
    vote_instruction: &DecodedVoteInstruction,
) -> Option<(u64, Hash, Option<u64>, usize)> {
    match vote_instruction {
        DecodedVoteInstruction::Vote(vote) | DecodedVoteInstruction::VoteSwitch(vote, _) => {
            Some((vote.last_voted_slot()?, vote.hash, None, 3))
        }
        DecodedVoteInstruction::UpdateVoteState(update)
        | DecodedVoteInstruction::UpdateVoteStateSwitch(update, _)
        | DecodedVoteInstruction::CompactUpdateVoteState(update)
        | DecodedVoteInstruction::CompactUpdateVoteStateSwitch(update, _) => {
            Some((update.last_voted_slot()?, update.hash, update.root, 1))
        }
        DecodedVoteInstruction::TowerSync(tower_sync)
        | DecodedVoteInstruction::TowerSyncSwitch(tower_sync, _) => Some((
            tower_sync.lockouts.back()?.slot(),
            tower_sync.hash,
            tower_sync.root,
            1,
        )),
    }
}

fn find_account_delta_proof<'a>(
    update: &'a GeyserUpdate,
    target: &Pubkey,
) -> Option<&'a GeyserAccountDeltaProof> {
    update.proof.proofs.iter().find(|proof| proof.0 == *target)
}

fn convert_bank_hash_proof(
    update: &GeyserUpdate,
    account_proof: &GeyserAccountDeltaProof,
) -> Result<ScaleSolanaBankHashProofV1> {
    let data = &account_proof.1 .0;
    let proof = &account_proof.1 .1;
    ensure!(
        data.pubkey == data.account.pubkey,
        "geyser proof pubkey {} did not match account pubkey {}",
        data.pubkey,
        data.account.pubkey,
    );
    ensure!(
        data.hash == hash_solana_account(&data.account),
        "geyser proof hash for account {} did not match locally recomputed account hash",
        data.pubkey,
    );

    Ok(ScaleSolanaBankHashProofV1 {
        slot: update.slot,
        bank_hash: ScaleH256(update.root.to_bytes()),
        account_delta_root: ScaleH256(update.proof.account_delta_root.to_bytes()),
        parent_bank_hash: ScaleH256(update.proof.parent_bankhash.to_bytes()),
        blockhash: ScaleH256(update.proof.blockhash.to_bytes()),
        num_sigs: update.proof.num_sigs,
        account_proof: ScaleSolanaAccountDeltaProofV1 {
            account: ScaleSolanaAccountInfoV1 {
                pubkey: data.account.pubkey.to_bytes(),
                lamports: data.account.lamports,
                owner: data.account.owner.to_bytes(),
                executable: data.account.executable,
                rent_epoch: data.account.rent_epoch,
                data: data.account.data.clone(),
                write_version: data.account.write_version,
                slot: data.account.slot,
            },
            merkle_proof: convert_merkle_proof(proof)?,
        },
    })
}

fn convert_merkle_proof(proof: &GeyserProof) -> Result<ScaleSolanaMerkleProofV1> {
    ensure!(
        proof.path.len() == proof.siblings.len(),
        "geyser proof path length did not match sibling depth",
    );
    let path = proof
        .path
        .iter()
        .map(|index| {
            let index = u8::try_from(*index).context("merkle proof index does not fit into u8")?;
            ensure!(
                index < 16,
                "merkle proof index {} exceeded Solana fanout",
                index
            );
            Ok(index)
        })
        .collect::<Result<Vec<_>>>()?;
    let siblings = proof
        .siblings
        .iter()
        .map(|level| {
            ensure!(
                level.len() <= 15,
                "merkle proof sibling level exceeded Solana fanout"
            );
            Ok(level
                .iter()
                .map(|hash| ScaleH256(hash.to_bytes()))
                .collect::<Vec<_>>())
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(ScaleSolanaMerkleProofV1 { path, siblings })
}

fn hash_solana_account(account: &GeyserAccountInfo) -> Hash {
    if account.lamports == 0 {
        return Hash::default();
    }
    let mut hasher = blake3::Hasher::new();
    hasher.update(&account.lamports.to_le_bytes());
    hasher.update(&account.rent_epoch.to_le_bytes());
    hasher.update(&account.data);
    hasher.update(&[u8::from(account.executable)]);
    hasher.update(account.owner.as_ref());
    hasher.update(account.pubkey.as_ref());
    Hash::new_from_array(hasher.finalize().into())
}

fn parse_hex_32(value: &str) -> Result<[u8; 32]> {
    let normalized = value.strip_prefix("0x").unwrap_or(value);
    let bytes = hex::decode(normalized)?;
    let len = bytes.len();
    let out: [u8; 32] = bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("expected 32 bytes, got {}", len))?;
    Ok(out)
}

fn prefixed_hex(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}

fn is_timeout_error(err: &std::io::Error) -> bool {
    matches!(
        err.kind(),
        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
    )
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use solana_sdk::{
        hash::hashv,
        signature::Keypair,
        signer::Signer,
        transaction::Transaction,
        vote::{instruction as vote_instruction, state::{Lockout, VoteStateUpdate}},
    };
    use solana_transaction_status::{Encodable, EncodedTransactionWithStatusMeta};

    use super::*;

    fn sample_payload() -> sccp_sol::BurnPayloadV1 {
        sccp_sol::BurnPayloadV1 {
            version: 1,
            source_domain: sccp_sol::SCCP_DOMAIN_SOL,
            dest_domain: sccp_sol::SCCP_DOMAIN_SORA,
            nonce: 7,
            sora_asset_id: [0x55; 32],
            amount: 42,
            recipient: [0x77; 32],
        }
    }

    fn make_geyser_update_for_account(
        pubkey: Pubkey,
        owner: Pubkey,
        data: Vec<u8>,
        slot: u64,
        num_sigs: u64,
    ) -> GeyserUpdate {
        let account = GeyserAccountInfo {
            pubkey,
            lamports: 1,
            owner,
            executable: false,
            rent_epoch: 0,
            data,
            write_version: 1,
            slot,
        };
        let account_hash = hash_solana_account(&account);
        let parent_bankhash = Hash::new_unique();
        let blockhash = Hash::new_unique();
        let root = hashv(&[
            parent_bankhash.as_ref(),
            account_hash.as_ref(),
            &num_sigs.to_le_bytes(),
            blockhash.as_ref(),
        ]);

        GeyserUpdate {
            slot,
            root,
            proof: GeyserBankHashProof {
                proofs: vec![GeyserAccountDeltaProof(
                    pubkey,
                    (
                        GeyserData {
                            pubkey,
                            hash: account_hash,
                            account,
                        },
                        GeyserProof {
                            path: vec![],
                            siblings: vec![],
                        },
                    ),
                )],
                num_sigs,
                parent_slot: slot.saturating_sub(1),
                account_delta_root: account_hash,
                parent_bankhash,
                blockhash,
            },
        }
    }

    fn sample_slot_hashes_proof(
        proof_slot: u64,
        entries: &[(u64, Hash)],
    ) -> ScaleSolanaBankHashProofV1 {
        let slot_hashes = SlotHashes::new(entries);
        ScaleSolanaBankHashProofV1 {
            slot: proof_slot,
            bank_hash: ScaleH256(Hash::new_unique().to_bytes()),
            account_delta_root: ScaleH256(Hash::new_unique().to_bytes()),
            parent_bank_hash: ScaleH256(Hash::new_unique().to_bytes()),
            blockhash: ScaleH256(Hash::new_unique().to_bytes()),
            num_sigs: 1,
            account_proof: ScaleSolanaAccountDeltaProofV1 {
                account: ScaleSolanaAccountInfoV1 {
                    pubkey: [0x11; 32],
                    lamports: 1,
                    owner: [0x22; 32],
                    executable: false,
                    rent_epoch: 0,
                    data: bincode::serialize(&slot_hashes).expect("slot hashes serialize"),
                    write_version: 1,
                    slot: proof_slot,
                },
                merkle_proof: ScaleSolanaMerkleProofV1 {
                    path: vec![],
                    siblings: vec![],
                },
            },
        }
    }

    #[test]
    fn maybe_capture_burn_returns_canonical_public_inputs() {
        let router_program_id = Pubkey::new_unique();
        let payload = sample_payload();
        let payload_bytes = payload.encode_scale();
        let message_id = sccp_sol::burn_message_id(&payload_bytes);
        let (burn_record_pda, bump) = Pubkey::find_program_address(
            &[SCCP_SEED_PREFIX, SCCP_SEED_BURN, &message_id],
            &router_program_id,
        );
        let record = sccp_sol::SolanaBurnRecordAccountV1 {
            version: 1,
            bump,
            message_id,
            payload: payload_bytes,
            sender: [0x33; 32],
            mint: [0x44; 32],
            slot: 42,
        };
        let account_data = record.encode_account_data().to_vec();
        let update = make_geyser_update_for_account(
            burn_record_pda,
            router_program_id,
            account_data.clone(),
            record.slot,
            3,
        );

        let capture =
            maybe_capture_burn(&update, &router_program_id, &burn_record_pda, &message_id)
                .unwrap()
                .expect("capture");

        assert_eq!(capture.finalized_slot, record.slot);
        assert_eq!(capture.finalized_bank_hash, update.root.to_bytes());
        assert_eq!(capture.finalized_slot_hash, None);
        assert_eq!(capture.public_inputs.message_id.0, message_id);
        assert_eq!(capture.public_inputs.finalized_slot_hash.0, [0u8; 32]);
        assert_eq!(
            capture.public_inputs.burn_record_data_hash.0,
            sccp_sol::solana_burn_record_data_hash(&account_data)
        );
        assert_eq!(
            capture.burn_proof.account_proof.account.data,
            account_data
        );
        assert_eq!(
            capture.burn_proof.account_proof.account.owner,
            router_program_id.to_bytes()
        );
    }

    #[test]
    fn maybe_capture_burn_rejects_message_id_mismatch() {
        let router_program_id = Pubkey::new_unique();
        let payload = sample_payload();
        let payload_bytes = payload.encode_scale();
        let message_id = sccp_sol::burn_message_id(&payload_bytes);
        let wrong_message_id = [0x99; 32];
        let (burn_record_pda, bump) = Pubkey::find_program_address(
            &[SCCP_SEED_PREFIX, SCCP_SEED_BURN, &message_id],
            &router_program_id,
        );
        let record = sccp_sol::SolanaBurnRecordAccountV1 {
            version: 1,
            bump,
            message_id,
            payload: payload_bytes,
            sender: [0x33; 32],
            mint: [0x44; 32],
            slot: 42,
        };
        let update = make_geyser_update_for_account(
            burn_record_pda,
            router_program_id,
            record.encode_account_data().to_vec(),
            record.slot,
            1,
        );

        let err = maybe_capture_burn(
            &update,
            &router_program_id,
            &burn_record_pda,
            &wrong_message_id,
        )
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("did not match requested messageId"));
    }

    #[test]
    fn maybe_capture_finalized_slot_hash_reads_slot_hashes_sysvar_entry() {
        let router_program_id = Pubkey::new_unique();
        let payload = sample_payload();
        let payload_bytes = payload.encode_scale();
        let message_id = sccp_sol::burn_message_id(&payload_bytes);
        let (burn_record_pda, bump) = Pubkey::find_program_address(
            &[SCCP_SEED_PREFIX, SCCP_SEED_BURN, &message_id],
            &router_program_id,
        );
        let record = sccp_sol::SolanaBurnRecordAccountV1 {
            version: 1,
            bump,
            message_id,
            payload: payload_bytes,
            sender: [0x33; 32],
            mint: [0x44; 32],
            slot: 42,
        };
        let burn_update = make_geyser_update_for_account(
            burn_record_pda,
            router_program_id,
            record.encode_account_data().to_vec(),
            record.slot,
            3,
        );
        let mut burn = maybe_capture_burn(
            &burn_update,
            &router_program_id,
            &burn_record_pda,
            &message_id,
        )
        .unwrap()
        .expect("capture");
        let finalized_slot_hash = Hash::new_unique();
        let slot_hashes_proof =
            sample_slot_hashes_proof(record.slot + 1, &[(record.slot, finalized_slot_hash)]);

        assert!(maybe_capture_finalized_slot_hash(&mut burn, &slot_hashes_proof).unwrap());
        assert_eq!(
            burn.finalized_slot_hash,
            Some(finalized_slot_hash.to_bytes())
        );
        assert_eq!(
            burn.public_inputs.finalized_slot_hash.0,
            finalized_slot_hash.to_bytes()
        );
    }

    #[test]
    fn build_vote_proofs_from_block_extracts_signed_compact_update_vote() {
        let authority = Keypair::new();
        let vote_account = Pubkey::new_unique();
        let vote_slot = 43u64;
        let target_burn_slot = 42u64;
        let vote_hash = Hash::new_unique();
        let target_burn_slot_hash = Hash::new_unique();
        let recent_blockhash = Hash::new_unique();
        let vote_state_update = VoteStateUpdate::new(
            VecDeque::from(vec![Lockout::new(vote_slot)]),
            Some(target_burn_slot),
            vote_hash,
        );
        let instruction = vote_instruction::compact_update_vote_state(
            &vote_account,
            &authority.pubkey(),
            vote_state_update,
        );
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&authority.pubkey()),
            &[&authority],
            recent_blockhash,
        );
        let encoded_tx = EncodedTransactionWithStatusMeta {
            transaction: transaction.encode(UiTransactionEncoding::Base64),
            meta: None,
            version: None,
        };
        let slot_hashes_proof = sample_slot_hashes_proof(
            vote_slot,
            &[(vote_slot, vote_hash), (target_burn_slot, target_burn_slot_hash)],
        );
        let block = UiConfirmedBlock {
            previous_blockhash: Hash::new_unique().to_string(),
            blockhash: recent_blockhash.to_string(),
            parent_slot: vote_slot - 1,
            transactions: Some(vec![encoded_tx]),
            signatures: None,
            rewards: None,
            block_time: None,
            block_height: None,
        };

        let proofs = build_vote_proofs_from_block(
            block,
            vote_slot,
            target_burn_slot,
            &target_burn_slot_hash.to_bytes(),
            &slot_hashes_proof,
        )
        .unwrap();

        assert_eq!(proofs.len(), 1);
        assert_eq!(proofs[0].authority_pubkey, authority.pubkey().to_bytes());
        assert_eq!(proofs[0].signature, transaction.signatures[0].as_ref());
        assert_eq!(proofs[0].signed_message, transaction.message.serialize());
        assert_eq!(proofs[0].vote_slot, vote_slot);
        assert_eq!(proofs[0].vote_bank_hash.0, vote_hash.to_bytes());
        assert_eq!(proofs[0].rooted_slot, Some(target_burn_slot));
    }

    #[test]
    fn build_vote_proofs_from_block_rejects_root_before_target_burn_slot() {
        let authority = Keypair::new();
        let vote_account = Pubkey::new_unique();
        let vote_slot = 43u64;
        let target_burn_slot = 42u64;
        let vote_hash = Hash::new_unique();
        let target_burn_slot_hash = Hash::new_unique();
        let recent_blockhash = Hash::new_unique();
        let vote_state_update = VoteStateUpdate::new(
            VecDeque::from(vec![Lockout::new(vote_slot)]),
            Some(target_burn_slot - 1),
            vote_hash,
        );
        let instruction = vote_instruction::compact_update_vote_state(
            &vote_account,
            &authority.pubkey(),
            vote_state_update,
        );
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&authority.pubkey()),
            &[&authority],
            recent_blockhash,
        );
        let encoded_tx = EncodedTransactionWithStatusMeta {
            transaction: transaction.encode(UiTransactionEncoding::Base64),
            meta: None,
            version: None,
        };
        let block = UiConfirmedBlock {
            previous_blockhash: Hash::new_unique().to_string(),
            blockhash: recent_blockhash.to_string(),
            parent_slot: vote_slot - 1,
            transactions: Some(vec![encoded_tx]),
            signatures: None,
            rewards: None,
            block_time: None,
            block_height: None,
        };

        let proofs = build_vote_proofs_from_block(
            block,
            vote_slot,
            target_burn_slot,
            &target_burn_slot_hash.to_bytes(),
            &sample_slot_hashes_proof(
                vote_slot,
                &[(vote_slot, vote_hash), (target_burn_slot, target_burn_slot_hash)],
            ),
        )
        .unwrap();

        assert!(proofs.is_empty());
    }
}
