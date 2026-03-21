#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]

extern crate alloc;

use alloc::{boxed::Box, vec::Vec};
use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint,
    entrypoint::ProgramResult,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    system_instruction,
    sysvar::{rent::Rent, Sysvar},
};

use solana_program::keccak::hashv as keccak_hashv;
use solana_program::secp256k1_recover::secp256k1_recover;

use sccp_sol::{
    burn_message_id, decode_burn_payload_v1, BurnPayloadV1, H256, SCCP_DOMAIN_BSC,
    SCCP_DOMAIN_ETH, SCCP_DOMAIN_SOL, SCCP_DOMAIN_SORA, SCCP_DOMAIN_TON, SCCP_DOMAIN_TRON,
};

const SEED_PREFIX: &[u8] = b"sccp";
const SEED_VERIFIER: &[u8] = b"verifier";
const SEED_CONFIG: &[u8] = b"config";

const ACCOUNT_VERSION_V1: u8 = 1;

const MMR_ROOT_HISTORY_SIZE: usize = 30;
const SECP256K1N_HALF_ORDER: [u8; 32] = [
    0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0x5d, 0x57, 0x6e, 0x73, 0x57, 0xa4, 0x50, 0x1d, 0xdf, 0xe9, 0x2f, 0x46, 0x68, 0x1b, 0x20, 0xa0,
];

// Leaf provider digest commitment network id sentinel (matches SORA pallet `SCCP_DIGEST_NETWORK_ID`).
const SCCP_DIGEST_NETWORK_ID: u32 = 0x5343_4350; // 'SCCP'

// SCALE enum discriminants (bridge-types v1.0.27):
const AUX_DIGEST_ITEM_COMMITMENT: u8 = 0;
const GENERIC_NETWORK_ID_EVM: u8 = 0;
const GENERIC_NETWORK_ID_SUB: u8 = 1;
const GENERIC_NETWORK_ID_EVM_LEGACY: u8 = 2;
const GENERIC_NETWORK_ID_TON: u8 = 3;

const IX_INITIALIZE: u8 = 0;
const IX_VERIFY_BURN_PROOF: u8 = 1;
const IX_SUBMIT_SIGNATURE_COMMITMENT: u8 = 2;
const IX_SET_GOVERNOR: u8 = 3;

#[repr(u32)]
pub enum VerifierError {
    InvalidInstructionData = 1,
    AlreadyInitialized = 2,
    InvalidPda = 3,
    InvalidOwner = 4,
    InvalidAccountSize = 5,
    NotGovernor = 6,
    ConfigNotInitialized = 7,
    PayloadInvalidLength = 8,
    ProofTooLarge = 9,
    SourceDomainUnsupported = 10,
    UnknownMmrRoot = 11,
    InvalidDigestHash = 12,
    CommitmentNotFoundInDigest = 13,
    InvalidValidatorSetId = 14,
    PayloadBlocknumberTooOld = 15,
    NotEnoughValidatorSignatures = 16,
    InvalidValidatorProof = 17,
    InvalidSignature = 18,
    InvalidMerkleProof = 19,
    InvalidMmrProof = 20,
    AdminPathDisabled = 21,
}

impl From<VerifierError> for ProgramError {
    fn from(e: VerifierError) -> Self {
        ProgramError::Custom(e as u32)
    }
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValidatorSet {
    pub id: u64,
    pub len: u32,
    pub root: [u8; 32],
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Commitment {
    pub mmr_root: [u8; 32],
    pub block_number: u32,
    pub validator_set_id: u64,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct ValidatorProof {
    pub signatures: Vec<Vec<u8>>, // 65 bytes each
    pub positions: Vec<u64>, // not used for membership, but checked for bounds/uniqueness
    pub public_keys: Vec<[u8; 20]>, // Ethereum addresses
    pub public_key_merkle_proofs: Vec<Vec<[u8; 32]>>,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct MmrProof {
    pub leaf_index: u64,
    pub leaf_count: u64,
    pub items: Vec<[u8; 32]>,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MmrLeaf {
    pub version: u8,
    pub parent_number: u32,
    pub parent_hash: [u8; 32],
    pub next_authority_set_id: u64,
    pub next_authority_set_len: u32,
    pub next_authority_set_root: [u8; 32],
    pub random_seed: [u8; 32],
    pub digest_hash: [u8; 32],
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct SoraBurnProofV1 {
    pub mmr_proof: MmrProof,
    pub leaf: MmrLeaf,
    pub digest_scale: Vec<u8>,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub version: u8,
    pub bump: u8,
    pub governor: Pubkey,
    pub latest_beefy_block: u64,
    pub current_validator_set: ValidatorSet,
    pub next_validator_set: ValidatorSet,
    pub mmr_roots_pos: u32,
    pub mmr_roots: [[u8; 32]; MMR_ROOT_HISTORY_SIZE],
}

impl Config {
    // 1 + 1 + 32 + 8 + (8+4+32)*2 + 4 + 30*32
    pub const LEN: usize = 1094;
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq, Eq)]
pub enum VerifierInstruction {
    /// Create config PDA and set initial validator sets during one-time bootstrap.
    Initialize {
        governor: Pubkey,
        latest_beefy_block: u64,
        current_validator_set: ValidatorSet,
        next_validator_set: ValidatorSet,
    },
    /// Reserved discriminator; `VerifyBurnProof` is parsed from raw bytes produced by SCCP router CPI.
    VerifyBurnProof,
    /// Import a new finalized MMR root by verifying a BEEFY commitment + signatures (permissionless).
    SubmitSignatureCommitment {
        commitment: Commitment,
        validator_proof: ValidatorProof,
        latest_mmr_leaf: MmrLeaf,
        proof: MmrProof,
    },
    /// Deprecated compatibility surface; always rejected with `AdminPathDisabled`.
    SetGovernor { governor: Pubkey },
}

#[derive(BorshDeserialize)]
struct InitializeArgs {
    governor: Pubkey,
    latest_beefy_block: u64,
    current_validator_set: ValidatorSet,
    next_validator_set: ValidatorSet,
}

#[derive(BorshDeserialize)]
struct SubmitSignatureCommitmentArgs {
    commitment: Commitment,
    validator_proof: ValidatorProof,
    latest_mmr_leaf: MmrLeaf,
    proof: MmrProof,
}

#[derive(BorshDeserialize)]
struct SetGovernorArgs {
    governor: Pubkey,
}

entrypoint!(process_instruction);

#[inline(never)]
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    if instruction_data.is_empty() {
        return Err(VerifierError::InvalidInstructionData.into());
    }

    // `VerifyBurnProof` is invoked by the SCCP router CPI using a custom byte layout (not Borsh).
    match instruction_data[0] {
        IX_INITIALIZE => {
            let args = parse_instruction_args::<InitializeArgs>(&instruction_data[1..])?;
            initialize(
                program_id,
                accounts,
                args.governor,
                args.latest_beefy_block,
                args.current_validator_set,
                args.next_validator_set,
            )
        }
        IX_VERIFY_BURN_PROOF => verify_burn_proof(program_id, accounts, instruction_data),
        IX_SUBMIT_SIGNATURE_COMMITMENT => {
            let args = parse_instruction_args::<SubmitSignatureCommitmentArgs>(&instruction_data[1..])?;
            submit_signature_commitment(
                program_id,
                accounts,
                args.commitment,
                args.validator_proof,
                args.latest_mmr_leaf,
                args.proof,
            )
        }
        IX_SET_GOVERNOR => {
            let args = parse_instruction_args::<SetGovernorArgs>(&instruction_data[1..])?;
            set_governor(program_id, accounts, args.governor)
        }
        _ => Err(VerifierError::InvalidInstructionData.into()),
    }
}

#[inline(never)]
fn initialize(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    governor: Pubkey,
    latest_beefy_block: u64,
    current_validator_set: ValidatorSet,
    next_validator_set: ValidatorSet,
) -> ProgramResult {
    let mut it = accounts.iter();
    let payer = next_account_info(&mut it)?;
    let config_acc = next_account_info(&mut it)?;
    let system_program = next_account_info(&mut it)?;

    if !payer.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if *system_program.key != solana_program::system_program::id() {
        return Err(ProgramError::IncorrectProgramId);
    }

    let (expected, bump) = config_pda(program_id);
    if *config_acc.key != expected {
        return Err(VerifierError::InvalidPda.into());
    }

    // Refuse re-init if already owned by this program.
    if config_acc.owner == program_id {
        return Err(VerifierError::AlreadyInitialized.into());
    }

    create_pda_account(
        payer,
        config_acc,
        Config::LEN,
        program_id,
        &[SEED_PREFIX, SEED_VERIFIER, SEED_CONFIG, &[bump]],
    )?;

    let current_validator_set = validate_validator_set(current_validator_set)?;
    let next_validator_set = validate_validator_set(next_validator_set)?;
    let cfg = Box::new(Config {
        version: ACCOUNT_VERSION_V1,
        bump,
        governor,
        latest_beefy_block,
        current_validator_set,
        next_validator_set,
        mmr_roots_pos: 0,
        mmr_roots: [[0u8; 32]; MMR_ROOT_HISTORY_SIZE],
    });
    write_borsh::<Config>(config_acc, cfg.as_ref())?;
    Ok(())
}

#[inline(never)]
fn set_governor(program_id: &Pubkey, accounts: &[AccountInfo], governor: Pubkey) -> ProgramResult {
    let _ = (program_id, accounts, governor);
    Err(VerifierError::AdminPathDisabled.into())
}

#[inline(never)]
fn submit_signature_commitment(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    commitment: Commitment,
    validator_proof: ValidatorProof,
    latest_mmr_leaf: MmrLeaf,
    proof: MmrProof,
) -> ProgramResult {
    let mut it = accounts.iter();
    let config_acc = next_account_info(&mut it)?;

    let mut cfg = load_config(program_id, config_acc)?;

    // Basic freshness check (fail fast).
    if (commitment.block_number as u64) <= cfg.latest_beefy_block {
        return Err(VerifierError::PayloadBlocknumberTooOld.into());
    }

    let vset = if commitment.validator_set_id == cfg.current_validator_set.id {
        cfg.current_validator_set
    } else if commitment.validator_set_id == cfg.next_validator_set.id {
        cfg.next_validator_set
    } else {
        return Err(VerifierError::InvalidValidatorSetId.into());
    };

    verify_commitment_signatures(&commitment, &validator_proof, vset)?;

    // Verify the provided MMR leaf is included under the payload root.
    let leaf_hash = hash_leaf(&latest_mmr_leaf);
    let root = mmr_proof_root(leaf_hash, &proof).map_err(|_| ProgramError::from(VerifierError::InvalidMmrProof))?;
    if root != commitment.mmr_root {
        return Err(VerifierError::InvalidMmrProof.into());
    }
    ensure_commitment_matches_leaf(&commitment, &latest_mmr_leaf)?;

    add_known_mmr_root(&mut cfg, commitment.mmr_root);
    cfg.latest_beefy_block = commitment.block_number as u64;

    // Apply validator set changes (if any) from the leaf.
    let new_vset = ValidatorSet {
        id: latest_mmr_leaf.next_authority_set_id,
        len: latest_mmr_leaf.next_authority_set_len,
        root: latest_mmr_leaf.next_authority_set_root,
    };
    if new_vset.id > cfg.next_validator_set.id {
        cfg.current_validator_set = cfg.next_validator_set;
        cfg.next_validator_set = validate_validator_set(new_vset)?;
    }

    write_borsh::<Config>(config_acc, &cfg)?;
    Ok(())
}

#[inline(never)]
fn verify_burn_proof(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    // Layout (produced by SCCP router CPI):
    // 1 byte version (1)
    // u32 source_domain LE
    // [32] message_id
    // payload (97 bytes)
    // proof (rest)
    if data.len() < 1 + 4 + 32 + BurnPayloadV1::ENCODED_LEN {
        return Err(VerifierError::InvalidInstructionData.into());
    }

    let mut it = accounts.iter();
    let config_acc = next_account_info(&mut it)?;
    let cfg = load_config(program_id, config_acc)?;

    // The first u32 is the burn origin domain.
    // This verifier only supports SCCP-known domains and rejects local SOL-as-source loopbacks.
    let source_domain = u32::from_le_bytes([data[1], data[2], data[3], data[4]]);
    let supported = matches!(
        source_domain,
        SCCP_DOMAIN_SORA | SCCP_DOMAIN_ETH | SCCP_DOMAIN_BSC | SCCP_DOMAIN_TON | SCCP_DOMAIN_TRON
    );
    if !supported || source_domain == SCCP_DOMAIN_SOL {
        return Err(VerifierError::SourceDomainUnsupported.into());
    }

    let message_id: H256 = data[5..37]
        .try_into()
        .map_err(|_| VerifierError::InvalidInstructionData)?;
    let payload = &data[37..37 + BurnPayloadV1::ENCODED_LEN];

    // Ensure payload matches message_id (fail closed).
    let computed = burn_message_id(payload);
    if computed != message_id {
        return Err(VerifierError::InvalidInstructionData.into());
    }

    // Ensure payload has the expected version.
    let p = decode_burn_payload_v1(payload).map_err(|_| ProgramError::from(VerifierError::PayloadInvalidLength))?;
    if p.version != 1 {
        return Err(VerifierError::PayloadInvalidLength.into());
    }
    // The caller-provided source domain must match payload source domain.
    if p.source_domain != source_domain {
        return Err(VerifierError::InvalidInstructionData.into());
    }
    // This verifier is for minting on Solana only.
    if p.dest_domain != SCCP_DOMAIN_SOL {
        return Err(VerifierError::InvalidInstructionData.into());
    }

    let proof_bytes = &data[37 + BurnPayloadV1::ENCODED_LEN..];
    if proof_bytes.len() > 16 * 1024 {
        return Err(VerifierError::ProofTooLarge.into());
    }
    let proof = SoraBurnProofV1::try_from_slice(proof_bytes)
        .map_err(|_| ProgramError::from(VerifierError::InvalidInstructionData))?;
    if proof.mmr_proof.items.len() >= 64 {
        return Err(VerifierError::InvalidMmrProof.into());
    }

    let leaf_hash = hash_leaf(&proof.leaf);
    let root =
        mmr_proof_root(leaf_hash, &proof.mmr_proof).map_err(|_| ProgramError::from(VerifierError::InvalidMmrProof))?;
    if !is_known_root(&cfg, &root) {
        return Err(VerifierError::UnknownMmrRoot.into());
    }

    let digest_hash = keccak256(&proof.digest_scale);
    if digest_hash != proof.leaf.digest_hash {
        return Err(VerifierError::InvalidDigestHash.into());
    }

    if !digest_has_sccp_commitment(&proof.digest_scale, &message_id) {
        return Err(VerifierError::CommitmentNotFoundInDigest.into());
    }

    Ok(())
}

fn config_pda(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[SEED_PREFIX, SEED_VERIFIER, SEED_CONFIG], program_id)
}

#[inline(never)]
fn load_config(program_id: &Pubkey, acc: &AccountInfo) -> Result<Box<Config>, ProgramError> {
    let (expected, _bump) = config_pda(program_id);
    if *acc.key != expected {
        return Err(VerifierError::InvalidPda.into());
    }
    if acc.owner != program_id {
        return Err(VerifierError::ConfigNotInitialized.into());
    }
    let cfg = Box::new(read_borsh::<Config>(acc)?);
    validate_config(cfg)
}

fn add_known_mmr_root(cfg: &mut Config, root: [u8; 32]) {
    // A small fixed ring buffer; O(30) scan is cheap.
    if is_known_root(cfg, &root) {
        return;
    }
    let pos = (cfg.mmr_roots_pos as usize) % MMR_ROOT_HISTORY_SIZE;
    cfg.mmr_roots[pos] = root;
    cfg.mmr_roots_pos = ((pos + 1) % MMR_ROOT_HISTORY_SIZE) as u32;
}

fn is_known_root(cfg: &Config, root: &[u8; 32]) -> bool {
    cfg.mmr_roots.iter().any(|x| x == root)
}

fn verify_commitment_signatures(
    commitment: &Commitment,
    proof: &ValidatorProof,
    vset: ValidatorSet,
) -> Result<(), ProgramError> {
    let num = vset.len as usize;
    if num == 0 {
        return Err(VerifierError::InvalidValidatorProof.into());
    }
    let threshold = ((num * 2) + 2) / 3; // ceil(2n/3)

    let n = proof.signatures.len();
    if proof.positions.len() != n || proof.public_keys.len() != n || proof.public_key_merkle_proofs.len() != n {
        return Err(VerifierError::InvalidValidatorProof.into());
    }
    if n < threshold {
        return Err(VerifierError::NotEnoughValidatorSignatures.into());
    }

    // Ensure unique positions and unique public keys.
    let mut seen_pos = vec![false; num];
    for i in 0..n {
        let pos = proof.positions[i] as usize;
        if pos >= num {
            return Err(VerifierError::InvalidValidatorProof.into());
        }
        if seen_pos[pos] {
            return Err(VerifierError::InvalidValidatorProof.into());
        }
        seen_pos[pos] = true;

        for j in 0..i {
            if proof.public_keys[j] == proof.public_keys[i] {
                return Err(VerifierError::InvalidValidatorProof.into());
            }
        }
    }

    let commitment_hash = hash_commitment(commitment);

    for i in 0..n {
        let addr = proof.public_keys[i];

        // Membership proof against the validator set root.
        let pos = proof.positions[i];
        if !verify_beefy_merkle_proof(vset.root, vset.len, pos, &addr, &proof.public_key_merkle_proofs[i]) {
            return Err(VerifierError::InvalidMerkleProof.into());
        }

        // Signature check against commitment hash.
        let sig = &proof.signatures[i];
        if sig.len() != 65 {
            return Err(VerifierError::InvalidSignature.into());
        }
        let (sig64, rec_id) = parse_eth_signature(sig)?;
        let pk = secp256k1_recover(&commitment_hash, rec_id, &sig64)
            .map_err(|_| ProgramError::from(VerifierError::InvalidSignature))?;
        let recovered = eth_address_from_pubkey(&pk.to_bytes());
        if recovered != addr {
            return Err(VerifierError::InvalidSignature.into());
        }
    }

    Ok(())
}

#[inline(never)]
fn validate_config(cfg: Box<Config>) -> Result<Box<Config>, ProgramError> {
    if cfg.version != ACCOUNT_VERSION_V1 {
        return Err(VerifierError::InvalidAccountSize.into());
    }
    Ok(cfg)
}

fn parse_instruction_args<T: BorshDeserialize>(data: &[u8]) -> Result<T, ProgramError> {
    let mut cursor = data;
    let value = T::deserialize(&mut cursor)
        .map_err(|_| ProgramError::from(VerifierError::InvalidInstructionData))?;
    if !cursor.is_empty() {
        return Err(VerifierError::InvalidInstructionData.into());
    }
    Ok(value)
}

fn validate_validator_set(vset: ValidatorSet) -> Result<ValidatorSet, ProgramError> {
    if vset.len == 0 {
        return Err(VerifierError::InvalidValidatorProof.into());
    }
    Ok(vset)
}

fn ensure_commitment_matches_leaf(commitment: &Commitment, leaf: &MmrLeaf) -> ProgramResult {
    if leaf.parent_number != commitment.block_number {
        return Err(VerifierError::InvalidMmrProof.into());
    }
    Ok(())
}

fn parse_eth_signature(sig65: &[u8]) -> Result<([u8; 64], u8), ProgramError> {
    if sig65.len() != 65 {
        return Err(VerifierError::InvalidSignature.into());
    }

    let mut sig64 = [0u8; 64];
    sig64.copy_from_slice(&sig65[0..64]);

    let mut r = [0u8; 32];
    r.copy_from_slice(&sig64[0..32]);
    let mut s = [0u8; 32];
    s.copy_from_slice(&sig64[32..64]);

    // Reject malleable / invalid ECDSA signatures (EIP-2 style).
    if r == [0u8; 32] || s == [0u8; 32] || s > SECP256K1N_HALF_ORDER {
        return Err(VerifierError::InvalidSignature.into());
    }

    let mut v = sig65[64];
    if v >= 27 {
        v = v.wrapping_sub(27);
    }
    if v > 1 {
        return Err(VerifierError::InvalidSignature.into());
    }
    Ok((sig64, v))
}

fn eth_address_from_pubkey(pubkey64: &[u8; 64]) -> [u8; 20] {
    let h = keccak256(pubkey64);
    let mut out = [0u8; 20];
    out.copy_from_slice(&h[12..32]);
    out
}

fn verify_beefy_merkle_proof(
    root: [u8; 32],
    set_len: u32,
    pos: u64,
    addr20: &[u8; 20],
    proof: &Vec<[u8; 32]>,
) -> bool {
    // Substrate `binary_merkle_tree` (ordered, no sorting):
    // - leafHash = keccak256(leaf_bytes) where leaf_bytes = bytes20(address)
    // - internal: keccak256(left || right)
    // - if odd number of nodes: last node is promoted
    if pos >= set_len as u64 {
        return false;
    }

    let mut current = keccak256(addr20);
    let mut idx = pos;
    let mut n = set_len as u64;
    let mut used: usize = 0;

    while n > 1 {
        let is_right = (idx & 1) == 1;
        if is_right {
            if used >= proof.len() {
                return false;
            }
            let sibling = proof[used];
            used += 1;
            let mut combined = [0u8; 64];
            combined[0..32].copy_from_slice(&sibling);
            combined[32..64].copy_from_slice(&current);
            current = keccak256(&combined);
        } else {
            // If this is the last odd node, it is promoted without hashing.
            if idx != n.saturating_sub(1) {
                if used >= proof.len() {
                    return false;
                }
                let sibling = proof[used];
                used += 1;
                let mut combined = [0u8; 64];
                combined[0..32].copy_from_slice(&current);
                combined[32..64].copy_from_slice(&sibling);
                current = keccak256(&combined);
            }
        }
        idx >>= 1;
        n = (n + 1) / 2;
    }

    used == proof.len() && current == root
}

fn hash_commitment(c: &Commitment) -> [u8; 32] {
    // SCALE(sp_beefy::Commitment<u32>) with payload restricted to one entry:
    // "mh" -> Vec<u8> of length 32 (mmr_root bytes).
    //
    // Layout (48 bytes):
    // [0] compact(vec len=1)=0x04
    // [1..3] "mh"
    // [3] compact(vec<u8> len=32)=0x80
    // [4..36] mmr_root
    // [36..40] u32 block_number (LE)
    // [40..48] u64 validator_set_id (LE)
    let mut out = [0u8; 48];
    out[0] = 0x04;
    out[1] = b'm';
    out[2] = b'h';
    out[3] = 0x80;
    out[4..36].copy_from_slice(&c.mmr_root);
    out[36..40].copy_from_slice(&c.block_number.to_le_bytes());
    out[40..48].copy_from_slice(&c.validator_set_id.to_le_bytes());
    keccak256(&out)
}

fn hash_leaf(leaf: &MmrLeaf) -> [u8; 32] {
    let scale = encode_leaf_scale(leaf);
    keccak256(&scale)
}

fn encode_leaf_scale(leaf: &MmrLeaf) -> [u8; 145] {
    // SCALE(sp_beefy::mmr::MmrLeaf<u32, H256, H256, LeafExtraData<H256,H256>>)
    // (fixed-width encoding):
    // version:u8
    // parent_number:u32 (LE)
    // parent_hash:[32]
    // next_authority_set_id:u64 (LE)
    // next_authority_set_len:u32 (LE)
    // next_authority_set_root:[32]
    // random_seed:[32]
    // digest_hash:[32]
    let mut out = [0u8; 145];
    out[0] = leaf.version;
    out[1..5].copy_from_slice(&leaf.parent_number.to_le_bytes());
    out[5..37].copy_from_slice(&leaf.parent_hash);
    out[37..45].copy_from_slice(&leaf.next_authority_set_id.to_le_bytes());
    out[45..49].copy_from_slice(&leaf.next_authority_set_len.to_le_bytes());
    out[49..81].copy_from_slice(&leaf.next_authority_set_root);
    out[81..113].copy_from_slice(&leaf.random_seed);
    out[113..145].copy_from_slice(&leaf.digest_hash);
    out
}

fn mmr_proof_root(leaf_hash: [u8; 32], proof: &MmrProof) -> Result<[u8; 32], VerifierError> {
    if proof.leaf_count == 0 || proof.leaf_index >= proof.leaf_count {
        return Err(VerifierError::InvalidMmrProof);
    }

    let mmr_size = leaf_index_to_mmr_size(proof.leaf_count - 1);
    let leaf_pos = leaf_index_to_pos(proof.leaf_index);

    let peaks = get_peaks(mmr_size);
    let mut peaks_hashes: Vec<[u8; 32]> = Vec::with_capacity(peaks.len() + 1);

    let mut proof_idx: usize = 0;
    let mut leaf_used = false;
    for peak_pos in peaks.into_iter() {
        if !leaf_used && leaf_pos <= peak_pos {
            let peak_root = if leaf_pos == peak_pos {
                leaf_hash
            } else {
                let (r, next) =
                    calculate_peak_root_single(leaf_pos, leaf_hash, peak_pos, &proof.items, proof_idx)?;
                proof_idx = next;
                r
            };
            leaf_used = true;
            peaks_hashes.push(peak_root);
        } else {
            // No leaf for this peak: proof carries the peak hash, or a bagged RHS peaks hash.
            if proof_idx < proof.items.len() {
                peaks_hashes.push(proof.items[proof_idx]);
                proof_idx += 1;
            } else {
                break;
            }
        }
    }

    if !leaf_used {
        return Err(VerifierError::InvalidMmrProof);
    }

    // Optional bagged RHS peaks hash (see `ckb-merkle-mountain-range` proof generation).
    if proof_idx < proof.items.len() {
        peaks_hashes.push(proof.items[proof_idx]);
        proof_idx += 1;
    }
    if proof_idx != proof.items.len() || peaks_hashes.is_empty() {
        return Err(VerifierError::InvalidMmrProof);
    }

    // Bag peaks right-to-left via hash(right, left).
    while peaks_hashes.len() > 1 {
        let right = peaks_hashes.pop().ok_or(VerifierError::InvalidMmrProof)?;
        let left = peaks_hashes.pop().ok_or(VerifierError::InvalidMmrProof)?;
        peaks_hashes.push(keccak256_two(right, left));
    }
    peaks_hashes.pop().ok_or(VerifierError::InvalidMmrProof)
}

fn keccak256(data: &[u8]) -> [u8; 32] {
    keccak_hashv(&[data]).0
}

fn keccak256_two(a: [u8; 32], b: [u8; 32]) -> [u8; 32] {
    let mut combined = [0u8; 64];
    combined[0..32].copy_from_slice(&a);
    combined[32..64].copy_from_slice(&b);
    keccak256(&combined)
}

// --- MMR helpers (ported from `ckb-merkle-mountain-range`) ---

fn leaf_index_to_pos(index: u64) -> u64 {
    leaf_index_to_mmr_size(index) - ((index + 1).trailing_zeros() as u64) - 1
}

fn leaf_index_to_mmr_size(index: u64) -> u64 {
    let leaves_count = index + 1;
    let peak_count = leaves_count.count_ones() as u64;
    2 * leaves_count - peak_count
}

fn pos_height_in_tree(mut pos: u64) -> u32 {
    pos += 1;
    fn all_ones(num: u64) -> bool {
        num != 0 && num.count_zeros() == num.leading_zeros()
    }
    fn jump_left(pos: u64) -> u64 {
        let bit_length = 64 - pos.leading_zeros();
        let most_significant_bits = 1 << (bit_length - 1);
        pos - (most_significant_bits - 1)
    }

    while !all_ones(pos) {
        pos = jump_left(pos)
    }

    64 - pos.leading_zeros() - 1
}

fn parent_offset(height: u32) -> u64 {
    2u64 << height
}

fn sibling_offset(height: u32) -> u64 {
    (2u64 << height) - 1
}

fn get_peaks(mmr_size: u64) -> Vec<u64> {
    let mut pos_s = Vec::new();
    let (mut height, mut pos) = left_peak_height_pos(mmr_size);
    pos_s.push(pos);
    while height > 0 {
        let peak = match get_right_peak(height, pos, mmr_size) {
            Some(peak) => peak,
            None => break,
        };
        height = peak.0;
        pos = peak.1;
        pos_s.push(pos);
    }
    pos_s
}

fn get_right_peak(mut height: u32, mut pos: u64, mmr_size: u64) -> Option<(u32, u64)> {
    // move to right sibling pos
    pos += sibling_offset(height);
    // loop until we find a pos in mmr
    while pos > mmr_size - 1 {
        if height == 0 {
            return None;
        }
        // move to left child
        pos -= parent_offset(height - 1);
        height -= 1;
    }
    Some((height, pos))
}

fn get_peak_pos_by_height(height: u32) -> u64 {
    (1u64 << (height + 1)) - 2
}

fn left_peak_height_pos(mmr_size: u64) -> (u32, u64) {
    let mut height = 1;
    let mut prev_pos = 0;
    let mut pos = get_peak_pos_by_height(height);
    while pos < mmr_size {
        height += 1;
        prev_pos = pos;
        pos = get_peak_pos_by_height(height);
    }
    (height - 1, prev_pos)
}

fn calculate_peak_root_single(
    mut pos: u64,
    mut item: [u8; 32],
    peak_pos: u64,
    proof_items: &[[u8; 32]],
    mut proof_idx: usize,
) -> Result<([u8; 32], usize), VerifierError> {
    let mut height: u32 = 0;
    loop {
        if pos == peak_pos {
            return Ok((item, proof_idx));
        }

        let next_height = pos_height_in_tree(pos + 1);
        let pos_is_right = next_height > height;
        let parent_pos = if pos_is_right {
            pos + 1
        } else {
            pos + parent_offset(height)
        };

        let sibling = *proof_items.get(proof_idx).ok_or(VerifierError::InvalidMmrProof)?;
        proof_idx = proof_idx.saturating_add(1);

        let parent_item = if pos_is_right {
            keccak256_two(sibling, item)
        } else {
            keccak256_two(item, sibling)
        };

        if parent_pos > peak_pos {
            return Err(VerifierError::InvalidMmrProof);
        }

        pos = parent_pos;
        item = parent_item;
        height = height.saturating_add(1);
    }
}

fn digest_has_sccp_commitment(digest_scale: &[u8], message_id: &H256) -> bool {
    let (n, mut off) = match read_compact_u32(digest_scale, 0) {
        Some(x) => x,
        None => return false,
    };

    let mut found = 0u32;
    for _ in 0..n {
        if off >= digest_scale.len() {
            return false;
        }
        let item_kind = digest_scale[off];
        off += 1;
        if item_kind != AUX_DIGEST_ITEM_COMMITMENT {
            return false;
        }

        if off >= digest_scale.len() {
            return false;
        }
        let network_kind = digest_scale[off];
        off += 1;

        let mut network_id = u32::MAX;
        match network_kind {
            GENERIC_NETWORK_ID_EVM_LEGACY => {
                if off + 4 > digest_scale.len() {
                    return false;
                }
                network_id = u32::from_le_bytes([
                    digest_scale[off],
                    digest_scale[off + 1],
                    digest_scale[off + 2],
                    digest_scale[off + 3],
                ]);
                off += 4;
            }
            GENERIC_NETWORK_ID_EVM => {
                // EVMChainId = H256 (32 bytes)
                if off + 32 > digest_scale.len() {
                    return false;
                }
                off += 32;
            }
            GENERIC_NETWORK_ID_SUB => {
                // SubNetworkId enum (1 byte)
                if off + 1 > digest_scale.len() {
                    return false;
                }
                off += 1;
            }
            GENERIC_NETWORK_ID_TON => {
                // TonNetworkId enum (1 byte)
                if off + 1 > digest_scale.len() {
                    return false;
                }
                off += 1;
            }
            _ => return false,
        }

        if off + 32 > digest_scale.len() {
            return false;
        }
        let item_hash: H256 = match digest_scale[off..off + 32].try_into() {
            Ok(x) => x,
            Err(_) => return false,
        };
        off += 32;

        if network_id == SCCP_DIGEST_NETWORK_ID && &item_hash == message_id {
            found = found.saturating_add(1);
        }
    }

    // SCALE vectors must be consumed exactly; reject trailing bytes.
    found == 1 && off == digest_scale.len()
}

fn read_compact_u32(data: &[u8], off: usize) -> Option<(u32, usize)> {
    if off >= data.len() {
        return None;
    }
    let b0 = data[off];
    let mode = b0 & 0x03;
    if mode == 0 {
        return Some(((b0 >> 2) as u32, off + 1));
    }
    if mode == 1 {
        if off + 2 > data.len() {
            return None;
        }
        let b1 = data[off + 1] as u32;
        let v = ((b0 as u32) >> 2) | (b1 << 6);
        return Some((v, off + 2));
    }
    if mode == 2 {
        if off + 4 > data.len() {
            return None;
        }
        let v = ((b0 as u32) >> 2)
            | ((data[off + 1] as u32) << 6)
            | ((data[off + 2] as u32) << 14)
            | ((data[off + 3] as u32) << 22);
        return Some((v, off + 4));
    }
    None // mode == 3 (big int) not supported
}

#[inline(never)]
fn read_borsh<T: BorshDeserialize>(acc: &AccountInfo) -> Result<T, ProgramError> {
    T::try_from_slice(&acc.data.borrow()).map_err(|_| ProgramError::from(VerifierError::InvalidAccountSize))
}

fn write_borsh<T: BorshSerialize>(acc: &AccountInfo, v: &T) -> Result<(), ProgramError> {
    let mut data = acc.data.borrow_mut();
    v.serialize(&mut &mut data[..])
        .map_err(|_| ProgramError::from(VerifierError::InvalidAccountSize))
}

fn create_pda_account<'a>(
    payer: &AccountInfo<'a>,
    pda: &AccountInfo<'a>,
    space: usize,
    owner: &Pubkey,
    signer_seeds: &[&[u8]],
) -> ProgramResult {
    if pda.owner != &solana_program::system_program::id() {
        return Err(VerifierError::InvalidOwner.into());
    }
    if pda.data_len() != 0 {
        return Err(VerifierError::InvalidAccountSize.into());
    }

    let rent = Rent::get()?;
    let lamports = rent.minimum_balance(space);

    if pda.lamports() == 0 {
        let ix = system_instruction::create_account(payer.key, pda.key, lamports, space as u64, owner);
        invoke_signed(&ix, &[payer.clone(), pda.clone()], &[signer_seeds])?;
        return Ok(());
    }

    let top_up = lamports.saturating_sub(pda.lamports());
    if top_up > 0 {
        let ix = system_instruction::transfer(payer.key, pda.key, top_up);
        invoke(&ix, &[payer.clone(), pda.clone()])?;
    }

    let allocate_ix = system_instruction::allocate(pda.key, space as u64);
    invoke_signed(&allocate_ix, &[pda.clone()], &[signer_seeds])?;

    let assign_ix = system_instruction::assign(pda.key, owner);
    invoke_signed(&assign_ix, &[pda.clone()], &[signer_seeds])?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_message_id() -> H256 {
        [0xabu8; 32]
    }

    fn encode_compact_u32(v: u32) -> Vec<u8> {
        if v < (1 << 6) {
            return vec![(v as u8) << 2];
        }
        if v < (1 << 14) {
            return vec![((v as u16) << 2 | 0x01) as u8, (v >> 6) as u8];
        }
        // 4-byte mode.
        vec![
            ((v << 2) | 0x02) as u8,
            (v >> 6) as u8,
            (v >> 14) as u8,
            (v >> 22) as u8,
        ]
    }

    fn digest_with_single_legacy_commitment(message_id: &H256) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&encode_compact_u32(1)); // one item
        out.push(AUX_DIGEST_ITEM_COMMITMENT);
        out.push(GENERIC_NETWORK_ID_EVM_LEGACY);
        out.extend_from_slice(&SCCP_DIGEST_NETWORK_ID.to_le_bytes());
        out.extend_from_slice(message_id);
        out
    }

    fn merkle_layers_from_leaves(mut leaves: Vec<[u8; 32]>) -> Vec<Vec<[u8; 32]>> {
        let mut layers = vec![leaves.clone()];
        while leaves.len() > 1 {
            let mut next = Vec::new();
            let mut i = 0usize;
            while i < leaves.len() {
                let left = leaves[i];
                if i + 1 < leaves.len() {
                    let right = leaves[i + 1];
                    let mut combined = [0u8; 64];
                    combined[0..32].copy_from_slice(&left);
                    combined[32..64].copy_from_slice(&right);
                    next.push(keccak256(&combined));
                } else {
                    // Substrate `binary_merkle_tree`: odd tail is promoted.
                    next.push(left);
                }
                i += 2;
            }
            layers.push(next.clone());
            leaves = next;
        }
        layers
    }

    fn merkle_proof_for_index(layers: &[Vec<[u8; 32]>], mut idx: usize) -> Vec<[u8; 32]> {
        let mut proof = Vec::new();
        for nodes in layers.iter().take(layers.len().saturating_sub(1)) {
            let sibling = if idx % 2 == 0 {
                idx.saturating_add(1)
            } else {
                idx.saturating_sub(1)
            };
            if sibling < nodes.len() {
                proof.push(nodes[sibling]);
            }
            idx >>= 1;
        }
        proof
    }

    #[test]
    fn parse_eth_signature_accepts_v_27_28_and_low_v() {
        let mut sig = [0u8; 65];
        sig[0] = 1; // r != 0
        sig[63] = 1; // s != 0 and low

        sig[64] = 27;
        let (_, v27) = parse_eth_signature(&sig).expect("v=27 should normalize to low-v");
        assert_eq!(v27, 0);

        sig[64] = 28;
        let (_, v28) = parse_eth_signature(&sig).expect("v=28 should normalize to low-v");
        assert_eq!(v28, 1);

        sig[64] = 1;
        let (_, v1) = parse_eth_signature(&sig).expect("v=1 should be accepted");
        assert_eq!(v1, 1);
    }

    #[test]
    fn parse_eth_signature_rejects_wrong_length_inputs() {
        let short = [0u8; 64];
        let err = parse_eth_signature(&short).expect_err("64-byte signature should be rejected");
        assert_eq!(err, ProgramError::Custom(VerifierError::InvalidSignature as u32));

        let overlong = [0u8; 66];
        let err = parse_eth_signature(&overlong).expect_err("66-byte signature should be rejected");
        assert_eq!(err, ProgramError::Custom(VerifierError::InvalidSignature as u32));
    }

    #[test]
    fn parse_eth_signature_rejects_invalid_v_and_zero_components() {
        let mut sig = [0u8; 65];
        sig[0] = 1;
        sig[63] = 1;

        sig[64] = 31;
        let bad_v = parse_eth_signature(&sig).expect_err("v=31 should be rejected");
        assert_eq!(bad_v, ProgramError::Custom(VerifierError::InvalidSignature as u32));

        sig[64] = 27;
        sig[0] = 0;
        let zero_r = parse_eth_signature(&sig).expect_err("r=0 should be rejected");
        assert_eq!(
            zero_r,
            ProgramError::Custom(VerifierError::InvalidSignature as u32)
        );

        sig[0] = 1;
        sig[32..64].fill(0);
        let zero_s = parse_eth_signature(&sig).expect_err("s=0 should be rejected");
        assert_eq!(
            zero_s,
            ProgramError::Custom(VerifierError::InvalidSignature as u32)
        );
    }

    #[test]
    fn parse_eth_signature_rejects_non_evm_recovery_ids() {
        let mut sig = [0u8; 65];
        sig[0] = 1;
        sig[63] = 1;

        for bad_v in [2u8, 3u8, 29u8, 30u8, 31u8, 32u8, 255u8] {
            sig[64] = bad_v;
            let err = parse_eth_signature(&sig)
                .expect_err("non-EVM recovery id should be rejected");
            assert_eq!(
                err,
                ProgramError::Custom(VerifierError::InvalidSignature as u32)
            );
        }
    }

    #[test]
    fn parse_eth_signature_enforces_half_order_s_boundary() {
        let mut sig = [0u8; 65];
        sig[0] = 1; // non-zero r
        sig[64] = 27;

        // s == half-order must be accepted.
        sig[32..64].copy_from_slice(&SECP256K1N_HALF_ORDER);
        parse_eth_signature(&sig).expect("s equal to half-order should be accepted");

        // s > half-order must be rejected.
        let mut too_high = SECP256K1N_HALF_ORDER;
        too_high[31] = too_high[31].saturating_add(1);
        sig[32..64].copy_from_slice(&too_high);
        let err = parse_eth_signature(&sig).expect_err("s above half-order should fail");
        assert_eq!(err, ProgramError::Custom(VerifierError::InvalidSignature as u32));
    }

    #[test]
    fn verify_commitment_signatures_rejects_zero_length_validator_set() {
        let commitment = Commitment {
            mmr_root: [0u8; 32],
            block_number: 1,
            validator_set_id: 1,
        };
        let proof = ValidatorProof {
            signatures: vec![],
            positions: vec![],
            public_keys: vec![],
            public_key_merkle_proofs: vec![],
        };
        let vset = ValidatorSet {
            id: 1,
            len: 0,
            root: [0u8; 32],
        };

        let err = verify_commitment_signatures(&commitment, &proof, vset)
            .expect_err("zero-length validator set must fail closed");
        assert_eq!(
            err,
            ProgramError::Custom(VerifierError::InvalidValidatorProof as u32)
        );
    }

    #[test]
    fn verify_commitment_signatures_requires_one_of_one_for_len1_set() {
        let commitment = Commitment {
            mmr_root: [0u8; 32],
            block_number: 1,
            validator_set_id: 1,
        };
        let proof = ValidatorProof {
            signatures: vec![],
            positions: vec![],
            public_keys: vec![],
            public_key_merkle_proofs: vec![],
        };
        let vset = ValidatorSet {
            id: 1,
            len: 1,
            root: [0u8; 32],
        };

        let err = verify_commitment_signatures(&commitment, &proof, vset)
            .expect_err("0-of-1 signatures must fail threshold check");
        assert_eq!(
            err,
            ProgramError::Custom(VerifierError::NotEnoughValidatorSignatures as u32)
        );
    }

    #[test]
    fn verify_commitment_signatures_requires_two_of_two_for_len2_set() {
        let commitment = Commitment {
            mmr_root: [0u8; 32],
            block_number: 1,
            validator_set_id: 1,
        };
        let proof = ValidatorProof {
            signatures: vec![vec![]],
            positions: vec![0],
            public_keys: vec![[0u8; 20]],
            public_key_merkle_proofs: vec![vec![]],
        };
        let vset = ValidatorSet {
            id: 1,
            len: 2,
            root: [0u8; 32],
        };

        let err = verify_commitment_signatures(&commitment, &proof, vset)
            .expect_err("1-of-2 signatures must fail threshold check");
        assert_eq!(
            err,
            ProgramError::Custom(VerifierError::NotEnoughValidatorSignatures as u32)
        );
    }

    #[test]
    fn verify_commitment_signatures_accepts_two_of_three_threshold() {
        let commitment = Commitment {
            mmr_root: [0u8; 32],
            block_number: 1,
            validator_set_id: 1,
        };
        let proof = ValidatorProof {
            signatures: vec![vec![], vec![]],
            positions: vec![0, 1],
            public_keys: vec![[0u8; 20], [1u8; 20]],
            public_key_merkle_proofs: vec![vec![], vec![]],
        };
        let vset = ValidatorSet {
            id: 1,
            len: 3,
            root: [0u8; 32],
        };

        let err = verify_commitment_signatures(&commitment, &proof, vset)
            .expect_err("2-of-3 should clear quorum and fail later on proof validation");
        assert_eq!(err, ProgramError::Custom(VerifierError::InvalidMerkleProof as u32));
    }

    #[test]
    fn verify_commitment_signatures_accepts_four_of_six_threshold() {
        let commitment = Commitment {
            mmr_root: [0u8; 32],
            block_number: 1,
            validator_set_id: 1,
        };
        let proof = ValidatorProof {
            signatures: vec![vec![], vec![], vec![], vec![]],
            positions: vec![0, 1, 2, 3],
            public_keys: vec![[0u8; 20], [1u8; 20], [2u8; 20], [3u8; 20]],
            public_key_merkle_proofs: vec![vec![], vec![], vec![], vec![]],
        };
        let vset = ValidatorSet {
            id: 1,
            len: 6,
            root: [0u8; 32],
        };

        let err = verify_commitment_signatures(&commitment, &proof, vset)
            .expect_err("4-of-6 should clear quorum and fail later on proof validation");
        assert_eq!(err, ProgramError::Custom(VerifierError::InvalidMerkleProof as u32));
    }

    #[test]
    fn ensure_commitment_matches_leaf_rejects_block_number_mismatch() {
        let commitment = Commitment {
            mmr_root: [0u8; 32],
            block_number: 7,
            validator_set_id: 1,
        };
        let leaf = MmrLeaf {
            version: 0,
            parent_number: 8,
            parent_hash: [0u8; 32],
            next_authority_set_id: 2,
            next_authority_set_len: 4,
            next_authority_set_root: [0u8; 32],
            random_seed: [0u8; 32],
            digest_hash: [0u8; 32],
        };

        let err = ensure_commitment_matches_leaf(&commitment, &leaf)
            .expect_err("leaf block number must match justified commitment block");
        assert_eq!(err, ProgramError::Custom(VerifierError::InvalidMmrProof as u32));
    }

    #[test]
    fn validate_config_rejects_wrong_account_version() {
        let cfg = Config {
            version: 2,
            bump: 1,
            governor: Pubkey::new_from_array([0x11; 32]),
            latest_beefy_block: 7,
            current_validator_set: ValidatorSet {
                id: 1,
                len: 4,
                root: [0x22; 32],
            },
            next_validator_set: ValidatorSet {
                id: 2,
                len: 4,
                root: [0x33; 32],
            },
            mmr_roots_pos: 0,
            mmr_roots: [[0u8; 32]; MMR_ROOT_HISTORY_SIZE],
        };

        let err = validate_config(Box::new(cfg)).expect_err("unknown config version must fail closed");
        assert_eq!(err, ProgramError::Custom(VerifierError::InvalidAccountSize as u32));
    }

    #[test]
    fn verify_commitment_signatures_rejects_signature_count_above_validator_set_len() {
        let commitment = Commitment {
            mmr_root: [0u8; 32],
            block_number: 1,
            validator_set_id: 1,
        };
        let proof = ValidatorProof {
            signatures: vec![vec![], vec![], vec![]],
            positions: vec![0, 1, 2],
            public_keys: vec![[0x11u8; 20], [0x22u8; 20], [0x33u8; 20]],
            public_key_merkle_proofs: vec![vec![], vec![], vec![]],
        };
        let vset = ValidatorSet {
            id: 1,
            len: 2,
            root: [0u8; 32],
        };

        let err = verify_commitment_signatures(&commitment, &proof, vset)
            .expect_err("signature list longer than validator set must fail closed");
        assert_eq!(
            err,
            ProgramError::Custom(VerifierError::InvalidValidatorProof as u32)
        );
    }

    #[test]
    fn verify_beefy_merkle_proof_rejects_extra_trailing_sibling() {
        let addrs = [[0x11u8; 20], [0x22u8; 20], [0x33u8; 20], [0x44u8; 20]];
        let leaves = addrs.iter().map(|addr| keccak256(addr)).collect::<Vec<_>>();
        let layers = merkle_layers_from_leaves(leaves);
        let root = layers
            .last()
            .and_then(|level| level.first())
            .copied()
            .expect("root must exist");

        let proof = merkle_proof_for_index(&layers, 0);
        assert!(
            verify_beefy_merkle_proof(root, addrs.len() as u32, 0, &addrs[0], &proof),
            "sanity: valid proof should verify"
        );

        let mut with_extra = proof.clone();
        with_extra.push([0u8; 32]);
        assert!(
            !verify_beefy_merkle_proof(root, addrs.len() as u32, 0, &addrs[0], &with_extra),
            "extra trailing sibling must fail closed"
        );
    }

    #[test]
    fn read_compact_u32_decodes_supported_modes() {
        assert_eq!(read_compact_u32(&encode_compact_u32(63), 0), Some((63, 1)));
        assert_eq!(read_compact_u32(&encode_compact_u32(16383), 0), Some((16383, 2)));
        assert_eq!(read_compact_u32(&encode_compact_u32(0x3fff_ffff), 0), Some((0x3fff_ffff, 4)));
    }

    #[test]
    fn read_compact_u32_rejects_mode3_and_truncated_inputs() {
        // mode=3 (big-int) is intentionally unsupported in this parser.
        assert_eq!(read_compact_u32(&[0x03], 0), None);
        assert_eq!(read_compact_u32(&[0x01], 0), None); // mode=1 truncated
        assert_eq!(read_compact_u32(&[0x02, 0x00, 0x00], 0), None); // mode=2 truncated
    }

    #[test]
    fn read_compact_u32_rejects_out_of_bounds_offsets() {
        assert_eq!(read_compact_u32(&[], 0), None);
        assert_eq!(read_compact_u32(&[0x00], 1), None);
        assert_eq!(read_compact_u32(&[0x04, 0x00], 2), None);
    }

    #[test]
    fn digest_parser_accepts_single_matching_sccp_commitment() {
        let message_id = sample_message_id();
        let digest = digest_with_single_legacy_commitment(&message_id);
        assert!(digest_has_sccp_commitment(&digest, &message_id));
    }

    #[test]
    fn digest_parser_rejects_declared_item_without_body_bytes() {
        let message_id = sample_message_id();
        // Vec len=1, but no item bytes follow.
        let digest = encode_compact_u32(1);
        assert!(!digest_has_sccp_commitment(&digest, &message_id));
    }

    #[test]
    fn digest_parser_rejects_duplicate_matching_sccp_commitments() {
        let message_id = sample_message_id();
        let mut digest = Vec::new();
        digest.extend_from_slice(&encode_compact_u32(2)); // two items
        for _ in 0..2 {
            digest.push(AUX_DIGEST_ITEM_COMMITMENT);
            digest.push(GENERIC_NETWORK_ID_EVM_LEGACY);
            digest.extend_from_slice(&SCCP_DIGEST_NETWORK_ID.to_le_bytes());
            digest.extend_from_slice(&message_id);
        }
        assert!(!digest_has_sccp_commitment(&digest, &message_id));
    }

    #[test]
    fn digest_parser_rejects_non_commitment_item_kind() {
        let message_id = sample_message_id();
        let mut digest = Vec::new();
        digest.extend_from_slice(&encode_compact_u32(1));
        digest.push(1); // non-commitment kind
        digest.push(GENERIC_NETWORK_ID_EVM_LEGACY);
        digest.extend_from_slice(&SCCP_DIGEST_NETWORK_ID.to_le_bytes());
        digest.extend_from_slice(&message_id);
        assert!(!digest_has_sccp_commitment(&digest, &message_id));
    }

    #[test]
    fn digest_parser_rejects_empty_digest_vec() {
        let message_id = sample_message_id();
        let digest = encode_compact_u32(0); // Vec len=0, no commitment items
        assert!(!digest_has_sccp_commitment(&digest, &message_id));
    }

    #[test]
    fn digest_parser_rejects_mode3_length_prefix_for_digest_vec() {
        let message_id = sample_message_id();
        // mode=3 compact-u32 is intentionally unsupported in this parser.
        let digest = vec![0x03, 0x00, 0x00, 0x00, 0x00];
        assert!(!digest_has_sccp_commitment(&digest, &message_id));
    }

    #[test]
    fn digest_parser_rejects_truncated_network_or_hash_bytes() {
        let message_id = sample_message_id();

        // Truncated legacy network id (needs 4 bytes).
        let mut truncated_network = Vec::new();
        truncated_network.extend_from_slice(&encode_compact_u32(1));
        truncated_network.push(AUX_DIGEST_ITEM_COMMITMENT);
        truncated_network.push(GENERIC_NETWORK_ID_EVM_LEGACY);
        truncated_network.extend_from_slice(&SCCP_DIGEST_NETWORK_ID.to_le_bytes()[0..3]);
        assert!(!digest_has_sccp_commitment(&truncated_network, &message_id));

        // Truncated commitment hash (needs 32 bytes).
        let mut truncated_hash = Vec::new();
        truncated_hash.extend_from_slice(&encode_compact_u32(1));
        truncated_hash.push(AUX_DIGEST_ITEM_COMMITMENT);
        truncated_hash.push(GENERIC_NETWORK_ID_EVM_LEGACY);
        truncated_hash.extend_from_slice(&SCCP_DIGEST_NETWORK_ID.to_le_bytes());
        truncated_hash.extend_from_slice(&message_id[0..31]);
        assert!(!digest_has_sccp_commitment(&truncated_hash, &message_id));
    }

    #[test]
    fn digest_parser_rejects_unknown_network_kind_discriminant() {
        let message_id = sample_message_id();
        let mut digest = Vec::new();
        digest.extend_from_slice(&encode_compact_u32(1));
        digest.push(AUX_DIGEST_ITEM_COMMITMENT);
        digest.push(0xff); // unknown GenericNetworkId variant
        digest.extend_from_slice(&SCCP_DIGEST_NETWORK_ID.to_le_bytes());
        digest.extend_from_slice(&message_id);
        assert!(!digest_has_sccp_commitment(&digest, &message_id));
    }

    #[test]
    fn digest_parser_rejects_matching_hash_on_non_legacy_network_variants() {
        let message_id = sample_message_id();

        // EVM(H256 chain id) variant with matching hash.
        let mut evm = Vec::new();
        evm.extend_from_slice(&encode_compact_u32(1));
        evm.push(AUX_DIGEST_ITEM_COMMITMENT);
        evm.push(GENERIC_NETWORK_ID_EVM);
        evm.extend_from_slice(&[0xaa; 32]); // chain id
        evm.extend_from_slice(&message_id);
        assert!(!digest_has_sccp_commitment(&evm, &message_id));

        // Sub network variant with matching hash.
        let mut sub = Vec::new();
        sub.extend_from_slice(&encode_compact_u32(1));
        sub.push(AUX_DIGEST_ITEM_COMMITMENT);
        sub.push(GENERIC_NETWORK_ID_SUB);
        sub.push(0x00); // Mainnet
        sub.extend_from_slice(&message_id);
        assert!(!digest_has_sccp_commitment(&sub, &message_id));

        // TON network variant with matching hash.
        let mut ton = Vec::new();
        ton.extend_from_slice(&encode_compact_u32(1));
        ton.push(AUX_DIGEST_ITEM_COMMITMENT);
        ton.push(GENERIC_NETWORK_ID_TON);
        ton.push(0x00); // Mainnet
        ton.extend_from_slice(&message_id);
        assert!(!digest_has_sccp_commitment(&ton, &message_id));
    }

    #[test]
    fn digest_parser_rejects_malformed_second_item_even_if_first_matches() {
        let message_id = sample_message_id();
        let mut digest = Vec::new();
        digest.extend_from_slice(&encode_compact_u32(2)); // two items

        // Item 0: valid matching SCCP commitment.
        digest.push(AUX_DIGEST_ITEM_COMMITMENT);
        digest.push(GENERIC_NETWORK_ID_EVM_LEGACY);
        digest.extend_from_slice(&SCCP_DIGEST_NETWORK_ID.to_le_bytes());
        digest.extend_from_slice(&message_id);

        // Item 1: malformed/unknown network discriminant.
        digest.push(AUX_DIGEST_ITEM_COMMITMENT);
        digest.push(0xff);
        digest.extend_from_slice(&message_id);

        assert!(!digest_has_sccp_commitment(&digest, &message_id));
    }

    #[test]
    fn digest_parser_rejects_non_commitment_second_item_even_if_first_matches() {
        let message_id = sample_message_id();
        let mut digest = Vec::new();
        digest.extend_from_slice(&encode_compact_u32(2)); // two items

        // First item: valid matching SCCP commitment.
        digest.push(AUX_DIGEST_ITEM_COMMITMENT);
        digest.push(GENERIC_NETWORK_ID_EVM_LEGACY);
        digest.extend_from_slice(&SCCP_DIGEST_NETWORK_ID.to_le_bytes());
        digest.extend_from_slice(&message_id);

        // Second item: invalid kind; parser must fail-closed.
        digest.push(0x01);

        assert!(!digest_has_sccp_commitment(&digest, &message_id));
    }

    #[test]
    fn digest_parser_rejects_trailing_bytes_after_declared_items() {
        let message_id = sample_message_id();
        let mut digest = digest_with_single_legacy_commitment(&message_id);
        digest.push(0x00); // trailing garbage after declared vec items
        assert!(!digest_has_sccp_commitment(&digest, &message_id));
    }
}
