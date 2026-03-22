#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]

extern crate alloc;

use alloc::vec::Vec;
use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint,
    entrypoint::ProgramResult,
    instruction::{AccountMeta, Instruction},
    msg,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    program_option::COption,
    program_pack::Pack,
    pubkey::Pubkey,
    system_instruction,
    sysvar::{clock::Clock, rent::Rent, Sysvar},
};

use sccp_sol::{
    burn_message_id, decode_burn_payload_v1, decode_token_add_payload_v1,
    decode_token_control_payload_v1, token_add_message_id, token_pause_message_id,
    token_resume_message_id, BurnPayloadV1, H256, TokenAddPayloadV1, TokenControlPayloadV1,
    SCCP_DOMAIN_BSC, SCCP_DOMAIN_ETH, SCCP_DOMAIN_SOL, SCCP_DOMAIN_SORA, SCCP_DOMAIN_TON,
    SCCP_DOMAIN_TRON,
};

use spl_token::{
    instruction as token_ix,
    state::{Account as TokenAccount, Mint as TokenMint},
};

const SEED_PREFIX: &[u8] = b"sccp";
const SEED_CONFIG: &[u8] = b"config";
const SEED_TOKEN: &[u8] = b"token";
const SEED_MINT: &[u8] = b"mint";
const SEED_BURN: &[u8] = b"burn";
const SEED_INBOUND: &[u8] = b"inbound";

const ACCOUNT_VERSION_V1: u8 = 1;

#[repr(u32)]
pub enum SccpError {
    InvalidInstructionData = 1,
    ConfigNotInitialized = 2,
    AlreadyInitialized = 3,
    NotGovernor = 4,
    InvalidPda = 5,
    InvalidOwner = 6,
    InvalidAccountSize = 7,
    DomainUnsupported = 8,
    DomainEqualsLocal = 9,
    AmountIsZero = 10,
    RecipientIsZero = 11,
    NonceOverflow = 12,
    TokenNotRegistered = 13,
    TokenAlreadyRegistered = 14,
    MintMismatch = 15,
    BurnRecordAlreadyExists = 16,
    InboundDomainPaused = 17,
    InboundAlreadyProcessed = 18,
    ProofInvalidated = 19,
    PayloadInvalidLength = 20,
    VerifierNotSet = 21,
    ProofVerificationFailed = 22,
    AmountTooLarge = 23,
    RecipientNotCanonical = 24,
    OutboundDomainPaused = 25,
    MintAuthorityMismatch = 26,
    NonZeroMintSupply = 27,
    FreezeAuthorityMismatch = 28,
    AdminPathDisabled = 29,
    VerifierProgramAlreadySet = 30,
    TokenNotActive = 31,
    TokenNotPaused = 32,
    InvalidGovernancePayload = 33,
    TokenMetadataInvalid = 34,
}

impl From<SccpError> for ProgramError {
    fn from(e: SccpError) -> Self {
        ProgramError::Custom(e as u32)
    }
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq, Eq)]
pub enum SccpInstruction {
    Initialize {
        governor: Pubkey,
    },
    SetGovernor {
        governor: Pubkey,
    },
    /// One-time verifier bootstrap. Once configured, the verifier program id is immutable.
    SetVerifierProgram {
        verifier_program: Pubkey,
    },
    /// Deterministically create an SPL mint PDA for `sora_asset_id` and register it.
    DeployToken {
        sora_asset_id: [u8; 32],
        decimals: u8,
    },
    RegisterToken {
        sora_asset_id: [u8; 32],
        mint: Pubkey,
    },
    /// Deprecated compatibility surface; always rejected with `AdminPathDisabled`.
    SetInboundDomainPaused {
        source_domain: u32,
        paused: bool,
    },
    /// Deprecated compatibility surface; always rejected with `AdminPathDisabled`.
    SetOutboundDomainPaused {
        dest_domain: u32,
        paused: bool,
    },
    /// Deprecated compatibility surface; always rejected with `AdminPathDisabled`.
    InvalidateInboundMessage {
        source_domain: u32,
        message_id: [u8; 32],
    },
    /// Deprecated compatibility surface; always rejected with `AdminPathDisabled`.
    ClearInvalidatedInboundMessage {
        source_domain: u32,
        message_id: [u8; 32],
    },

    /// Burn SPL tokens on Solana and create an on-chain burn record PDA.
    Burn {
        sora_asset_id: [u8; 32],
        amount: u64,
        dest_domain: u32,
        recipient: [u8; 32],
    },

    /// Mint SPL tokens on Solana based on a verified burn on `source_domain`.
    ///
    /// Fail-closed until a verifier program is configured.
    MintFromProof {
        source_domain: u32,
        payload: Vec<u8>,
        proof: Vec<u8>,
    },
    /// Proof-driven canonical token lifecycle activation on Solana.
    AddTokenFromProof {
        payload: Vec<u8>,
        proof: Vec<u8>,
    },
    PauseTokenFromProof {
        payload: Vec<u8>,
        proof: Vec<u8>,
    },
    ResumeTokenFromProof {
        payload: Vec<u8>,
        proof: Vec<u8>,
    },
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub version: u8,
    pub bump: u8,
    pub governor: Pubkey,
    pub verifier_program: Pubkey,
    pub outbound_nonce: u64,
    pub inbound_paused_mask: u64,
    pub outbound_paused_mask: u64,
}

impl Config {
    pub const LEN: usize = 1 + 1 + 32 + 32 + 8 + 8 + 8;
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenState {
    Active = 1,
    Paused = 2,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct TokenConfig {
    pub version: u8,
    pub bump: u8,
    pub sora_asset_id: [u8; 32],
    pub mint: Pubkey,
    pub state: TokenState,
}

impl TokenConfig {
    pub const LEN: usize = 1 + 1 + 32 + 32 + 1;
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct BurnRecord {
    pub version: u8,
    pub bump: u8,
    pub message_id: [u8; 32],
    pub payload: [u8; BurnPayloadV1::ENCODED_LEN],
    pub sender: Pubkey,
    pub mint: Pubkey,
    pub slot: u64,
}

impl BurnRecord {
    pub const LEN: usize = 1 + 1 + 32 + BurnPayloadV1::ENCODED_LEN + 32 + 32 + 8;
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum InboundStatus {
    None = 0,
    Processed = 1,
    Invalidated = 2,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct InboundMarker {
    pub version: u8,
    pub bump: u8,
    pub status: InboundStatus,
}

impl InboundMarker {
    pub const LEN: usize = 1 + 1 + 1;
}

entrypoint!(process_instruction);

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let ix = SccpInstruction::try_from_slice(instruction_data)
        .map_err(|_| ProgramError::from(SccpError::InvalidInstructionData))?;

    match ix {
        SccpInstruction::Initialize { governor } => initialize(program_id, accounts, governor),
        SccpInstruction::SetGovernor { governor } => set_governor(program_id, accounts, governor),
        SccpInstruction::SetVerifierProgram { verifier_program } => {
            set_verifier(program_id, accounts, verifier_program)
        }
        SccpInstruction::DeployToken {
            sora_asset_id,
            decimals,
        } => deploy_token(program_id, accounts, sora_asset_id, decimals),
        SccpInstruction::RegisterToken {
            sora_asset_id,
            mint,
        } => register_token(program_id, accounts, sora_asset_id, mint),
        SccpInstruction::SetInboundDomainPaused {
            source_domain,
            paused,
        } => set_inbound_domain_paused(program_id, accounts, source_domain, paused),
        SccpInstruction::SetOutboundDomainPaused {
            dest_domain,
            paused,
        } => set_outbound_domain_paused(program_id, accounts, dest_domain, paused),
        SccpInstruction::InvalidateInboundMessage {
            source_domain,
            message_id,
        } => invalidate_inbound_message(program_id, accounts, source_domain, message_id),
        SccpInstruction::ClearInvalidatedInboundMessage {
            source_domain,
            message_id,
        } => clear_invalidated_inbound_message(program_id, accounts, source_domain, message_id),
        SccpInstruction::Burn {
            sora_asset_id,
            amount,
            dest_domain,
            recipient,
        } => burn(
            program_id,
            accounts,
            sora_asset_id,
            amount,
            dest_domain,
            recipient,
        ),
        SccpInstruction::MintFromProof {
            source_domain,
            payload,
            proof,
        } => mint_from_proof(program_id, accounts, source_domain, &payload, &proof),
        SccpInstruction::AddTokenFromProof { payload, proof } => {
            add_token_from_proof(program_id, accounts, &payload, &proof)
        }
        SccpInstruction::PauseTokenFromProof { payload, proof } => {
            pause_token_from_proof(program_id, accounts, &payload, &proof)
        }
        SccpInstruction::ResumeTokenFromProof { payload, proof } => {
            resume_token_from_proof(program_id, accounts, &payload, &proof)
        }
    }
}

fn initialize(program_id: &Pubkey, accounts: &[AccountInfo], governor: Pubkey) -> ProgramResult {
    let mut it = accounts.iter();
    let payer = next_account_info(&mut it)?;
    let config_acc = next_account_info(&mut it)?;
    let system_program = next_account_info(&mut it)?;
    let governor_authority = it.next();

    if !payer.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if *system_program.key != solana_program::system_program::id() {
        return Err(ProgramError::IncorrectProgramId);
    }
    require_governor_authority(governor_authority.unwrap_or(payer), &governor)?;

    let (expected, bump) = config_pda(program_id);
    if *config_acc.key != expected {
        return Err(SccpError::InvalidPda.into());
    }

    // Refuse re-init if already owned by this program.
    if config_acc.owner == program_id {
        return Err(SccpError::AlreadyInitialized.into());
    }

    create_pda_account(
        payer,
        config_acc,
        Config::LEN,
        program_id,
        &[SEED_PREFIX, SEED_CONFIG, &[bump]],
    )?;

    let cfg = Config {
        version: ACCOUNT_VERSION_V1,
        bump,
        governor,
        verifier_program: Pubkey::default(),
        outbound_nonce: 0,
        inbound_paused_mask: 0,
        outbound_paused_mask: 0,
    };
    write_borsh::<Config>(config_acc, &cfg)?;
    Ok(())
}

fn set_governor(program_id: &Pubkey, accounts: &[AccountInfo], governor: Pubkey) -> ProgramResult {
    let _ = (program_id, accounts, governor);
    Err(SccpError::AdminPathDisabled.into())
}

fn set_verifier(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    verifier_program: Pubkey,
) -> ProgramResult {
    let mut it = accounts.iter();
    let signer = next_account_info(&mut it)?;
    let config_acc = next_account_info(&mut it)?;

    let mut cfg = load_config(program_id, config_acc)?;
    require_governor_authority(signer, &cfg.governor)?;
    if cfg.verifier_program != Pubkey::default() {
        return Err(SccpError::VerifierProgramAlreadySet.into());
    }
    cfg.verifier_program = verifier_program;
    write_borsh::<Config>(config_acc, &cfg)?;
    Ok(())
}

fn deploy_token(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sora_asset_id: [u8; 32],
    decimals: u8,
) -> ProgramResult {
    let mut it = accounts.iter();
    let payer = next_account_info(&mut it)?;
    let config_acc = next_account_info(&mut it)?;
    let token_cfg_acc = next_account_info(&mut it)?;
    let mint_acc = next_account_info(&mut it)?;
    let system_program = next_account_info(&mut it)?;
    let token_program = next_account_info(&mut it)?;
    let rent_sysvar = next_account_info(&mut it)?;
    let governor_authority = it.next();

    if !payer.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    let cfg = load_config(program_id, config_acc)?;
    require_governor_authority(governor_authority.unwrap_or(payer), &cfg.governor)?;
    if cfg.verifier_program != Pubkey::default() {
        return Err(SccpError::AdminPathDisabled.into());
    }

    if *system_program.key != solana_program::system_program::id() {
        return Err(ProgramError::IncorrectProgramId);
    }
    if *token_program.key != spl_token::id() {
        return Err(ProgramError::IncorrectProgramId);
    }
    if *rent_sysvar.key != solana_program::sysvar::rent::id() {
        return Err(ProgramError::InvalidArgument);
    }

    let (expected_mint, mint_bump) = mint_pda(program_id, &sora_asset_id);
    if *mint_acc.key != expected_mint {
        return Err(SccpError::InvalidPda.into());
    }
    if mint_acc.owner == &spl_token::id() {
        return Err(SccpError::TokenAlreadyRegistered.into());
    }

    let (expected_token_cfg, token_bump) = token_config_pda(program_id, &sora_asset_id);
    if *token_cfg_acc.key != expected_token_cfg {
        return Err(SccpError::InvalidPda.into());
    }
    if token_cfg_acc.owner == program_id {
        return Err(SccpError::TokenAlreadyRegistered.into());
    }

    // Create SPL mint PDA (owner = token program).
    create_pda_account(
        payer,
        mint_acc,
        spl_token::state::Mint::LEN,
        token_program.key,
        &[SEED_PREFIX, SEED_MINT, &sora_asset_id, &[mint_bump]],
    )?;
    let init_ix = token_ix::initialize_mint(
        token_program.key,
        mint_acc.key,
        config_acc.key,
        Some(config_acc.key),
        decimals,
    )?;
    invoke(&init_ix, &[mint_acc.clone(), rent_sysvar.clone()])?;

    // Create token config PDA (owner = this program).
    create_pda_account(
        payer,
        token_cfg_acc,
        TokenConfig::LEN,
        program_id,
        &[SEED_PREFIX, SEED_TOKEN, &sora_asset_id, &[token_bump]],
    )?;
    let t = TokenConfig {
        version: ACCOUNT_VERSION_V1,
        bump: token_bump,
        sora_asset_id,
        mint: *mint_acc.key,
        state: TokenState::Active,
    };
    write_borsh::<TokenConfig>(token_cfg_acc, &t)?;

    Ok(())
}

fn register_token(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sora_asset_id: [u8; 32],
    mint: Pubkey,
) -> ProgramResult {
    let mut it = accounts.iter();
    let payer = next_account_info(&mut it)?;
    let config_acc = next_account_info(&mut it)?;
    let token_cfg_acc = next_account_info(&mut it)?;
    let mint_acc = next_account_info(&mut it)?;
    let system_program = next_account_info(&mut it)?;
    let governor_authority = it.next();

    if !payer.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    let cfg = load_config(program_id, config_acc)?;
    require_governor_authority(governor_authority.unwrap_or(payer), &cfg.governor)?;
    if cfg.verifier_program != Pubkey::default() {
        return Err(SccpError::AdminPathDisabled.into());
    }
    if *system_program.key != solana_program::system_program::id() {
        return Err(ProgramError::IncorrectProgramId);
    }

    if *mint_acc.key != mint {
        return Err(SccpError::MintMismatch.into());
    }
    if mint_acc.owner != &spl_token::id() {
        return Err(SccpError::InvalidOwner.into());
    }
    let mint_state =
        TokenMint::unpack(&mint_acc.data.borrow()).map_err(|_| ProgramError::InvalidAccountData)?;
    ensure_bridge_controlled_mint(&mint_state, config_acc.key)?;
    if mint_state.supply != 0 {
        return Err(SccpError::NonZeroMintSupply.into());
    }

    let (expected, bump) = token_config_pda(program_id, &sora_asset_id);
    if *token_cfg_acc.key != expected {
        return Err(SccpError::InvalidPda.into());
    }

    if token_cfg_acc.owner == program_id {
        return Err(SccpError::TokenAlreadyRegistered.into());
    }

    create_pda_account(
        payer,
        token_cfg_acc,
        TokenConfig::LEN,
        program_id,
        &[SEED_PREFIX, SEED_TOKEN, &sora_asset_id, &[bump]],
    )?;

    let t = TokenConfig {
        version: ACCOUNT_VERSION_V1,
        bump,
        sora_asset_id,
        mint,
        state: TokenState::Active,
    };
    write_borsh::<TokenConfig>(token_cfg_acc, &t)?;
    Ok(())
}

fn set_inbound_domain_paused(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    source_domain: u32,
    paused: bool,
) -> ProgramResult {
    let _ = (program_id, accounts, source_domain, paused);
    Err(SccpError::AdminPathDisabled.into())
}

fn set_outbound_domain_paused(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    dest_domain: u32,
    paused: bool,
) -> ProgramResult {
    let _ = (program_id, accounts, dest_domain, paused);
    Err(SccpError::AdminPathDisabled.into())
}

fn invalidate_inbound_message(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    source_domain: u32,
    message_id: [u8; 32],
) -> ProgramResult {
    let _ = (program_id, accounts, source_domain, message_id);
    Err(SccpError::AdminPathDisabled.into())
}

fn clear_invalidated_inbound_message(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    source_domain: u32,
    message_id: [u8; 32],
) -> ProgramResult {
    let _ = (program_id, accounts, source_domain, message_id);
    Err(SccpError::AdminPathDisabled.into())
}

fn add_token_from_proof(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    payload: &[u8],
    proof: &[u8],
) -> ProgramResult {
    if payload.len() != TokenAddPayloadV1::ENCODED_LEN {
        return Err(SccpError::PayloadInvalidLength.into());
    }
    if proof.len() > 16 * 1024 {
        return Err(ProgramError::InvalidInstructionData);
    }

    let mut it = accounts.iter();
    let payer = next_account_info(&mut it)?;
    let config_acc = next_account_info(&mut it)?;
    let token_cfg_acc = next_account_info(&mut it)?;
    let mint_acc = next_account_info(&mut it)?;
    let marker_acc = next_account_info(&mut it)?;
    let system_program = next_account_info(&mut it)?;
    let token_program = next_account_info(&mut it)?;
    let rent_sysvar = next_account_info(&mut it)?;

    if !payer.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if *system_program.key != solana_program::system_program::id() {
        return Err(ProgramError::IncorrectProgramId);
    }
    if *token_program.key != spl_token::id() {
        return Err(ProgramError::IncorrectProgramId);
    }
    if *rent_sysvar.key != solana_program::sysvar::rent::id() {
        return Err(ProgramError::InvalidArgument);
    }

    let cfg = load_config(program_id, config_acc)?;
    if cfg.verifier_program == Pubkey::default() {
        return Err(SccpError::VerifierNotSet.into());
    }

    let p = decode_token_add_payload_v1(payload)
        .map_err(|_| ProgramError::from(SccpError::PayloadInvalidLength))?;
    if p.version != 1 {
        return Err(SccpError::InvalidGovernancePayload.into());
    }
    if p.target_domain != SCCP_DOMAIN_SOL {
        return Err(SccpError::DomainUnsupported.into());
    }
    validate_governance_label(&p.name)?;
    validate_governance_label(&p.symbol)?;

    let message_id = token_add_message_id(payload);
    let (expected_marker, marker_bump) = inbound_marker_pda(program_id, SCCP_DOMAIN_SORA, &message_id);
    if *marker_acc.key != expected_marker {
        return Err(SccpError::InvalidPda.into());
    }
    ensure_message_unprocessed(program_id, marker_acc)?;

    let verifier_program_acc = next_account_info(&mut it)?;
    let verifier_accounts: Vec<AccountInfo> = it.cloned().collect();
    invoke_generic_verifier(
        &cfg,
        verifier_program_acc,
        &verifier_accounts,
        build_governance_verifier_data(&message_id, proof),
    )?;

    let (expected_token_cfg, token_bump) = token_config_pda(program_id, &p.sora_asset_id);
    if *token_cfg_acc.key != expected_token_cfg {
        return Err(SccpError::InvalidPda.into());
    }
    let (expected_mint, mint_bump) = mint_pda(program_id, &p.sora_asset_id);
    if *mint_acc.key != expected_mint {
        return Err(SccpError::InvalidPda.into());
    }

    if mint_acc.owner == &spl_token::id() {
        let mint_state =
            TokenMint::unpack(&mint_acc.data.borrow()).map_err(|_| ProgramError::InvalidAccountData)?;
        ensure_bridge_controlled_mint(&mint_state, config_acc.key)?;
        if mint_state.decimals != p.decimals {
            return Err(SccpError::MintMismatch.into());
        }
    } else {
        create_pda_account(
            payer,
            mint_acc,
            spl_token::state::Mint::LEN,
            token_program.key,
            &[SEED_PREFIX, SEED_MINT, &p.sora_asset_id, &[mint_bump]],
        )?;
        let init_ix = token_ix::initialize_mint(
            token_program.key,
            mint_acc.key,
            config_acc.key,
            Some(config_acc.key),
            p.decimals,
        )?;
        invoke(&init_ix, &[mint_acc.clone(), rent_sysvar.clone()])?;
    }

    if token_cfg_acc.owner == program_id {
        let mut token_cfg = load_token_config(token_cfg_acc)?;
        if token_cfg.sora_asset_id != p.sora_asset_id || token_cfg.mint != *mint_acc.key {
            return Err(SccpError::MintMismatch.into());
        }
        token_cfg.state = TokenState::Active;
        write_borsh::<TokenConfig>(token_cfg_acc, &token_cfg)?;
    } else {
        create_pda_account(
            payer,
            token_cfg_acc,
            TokenConfig::LEN,
            program_id,
            &[SEED_PREFIX, SEED_TOKEN, &p.sora_asset_id, &[token_bump]],
        )?;
        let token_cfg = TokenConfig {
            version: ACCOUNT_VERSION_V1,
            bump: token_bump,
            sora_asset_id: p.sora_asset_id,
            mint: *mint_acc.key,
            state: TokenState::Active,
        };
        write_borsh::<TokenConfig>(token_cfg_acc, &token_cfg)?;
    }

    ensure_marker_account(
        program_id,
        payer,
        marker_acc,
        SCCP_DOMAIN_SORA,
        &message_id,
        marker_bump,
    )?;
    mark_message_processed(marker_acc)?;
    Ok(())
}

fn pause_token_from_proof(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    payload: &[u8],
    proof: &[u8],
) -> ProgramResult {
    process_token_control_from_proof(
        program_id,
        accounts,
        payload,
        proof,
        TokenState::Active,
        TokenState::Paused,
        token_pause_message_id,
    )
}

fn resume_token_from_proof(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    payload: &[u8],
    proof: &[u8],
) -> ProgramResult {
    process_token_control_from_proof(
        program_id,
        accounts,
        payload,
        proof,
        TokenState::Paused,
        TokenState::Active,
        token_resume_message_id,
    )
}

fn process_token_control_from_proof(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    payload: &[u8],
    proof: &[u8],
    expected_state: TokenState,
    next_state: TokenState,
    message_id_fn: fn(&[u8]) -> H256,
) -> ProgramResult {
    if payload.len() != TokenControlPayloadV1::ENCODED_LEN {
        return Err(SccpError::PayloadInvalidLength.into());
    }
    if proof.len() > 16 * 1024 {
        return Err(ProgramError::InvalidInstructionData);
    }

    let mut it = accounts.iter();
    let payer = next_account_info(&mut it)?;
    let config_acc = next_account_info(&mut it)?;
    let token_cfg_acc = next_account_info(&mut it)?;
    let marker_acc = next_account_info(&mut it)?;

    if !payer.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let cfg = load_config(program_id, config_acc)?;
    if cfg.verifier_program == Pubkey::default() {
        return Err(SccpError::VerifierNotSet.into());
    }

    let p = decode_token_control_payload_v1(payload)
        .map_err(|_| ProgramError::from(SccpError::PayloadInvalidLength))?;
    if p.version != 1 {
        return Err(SccpError::InvalidGovernancePayload.into());
    }
    if p.target_domain != SCCP_DOMAIN_SOL {
        return Err(SccpError::DomainUnsupported.into());
    }

    let message_id = message_id_fn(payload);
    let (expected_marker, marker_bump) = inbound_marker_pda(program_id, SCCP_DOMAIN_SORA, &message_id);
    if *marker_acc.key != expected_marker {
        return Err(SccpError::InvalidPda.into());
    }
    ensure_message_unprocessed(program_id, marker_acc)?;

    let (expected_token_cfg, _token_bump) = token_config_pda(program_id, &p.sora_asset_id);
    if *token_cfg_acc.key != expected_token_cfg {
        return Err(SccpError::InvalidPda.into());
    }
    let mut token_cfg = load_token_config(token_cfg_acc)?;
    if token_cfg.sora_asset_id != p.sora_asset_id {
        return Err(SccpError::InvalidGovernancePayload.into());
    }
    match (expected_state, token_cfg.state) {
        (TokenState::Active, TokenState::Active) => {}
        (TokenState::Paused, TokenState::Paused) => {}
        (TokenState::Active, _) => return Err(SccpError::TokenNotActive.into()),
        (TokenState::Paused, _) => return Err(SccpError::TokenNotPaused.into()),
    }

    let verifier_program_acc = next_account_info(&mut it)?;
    let verifier_accounts: Vec<AccountInfo> = it.cloned().collect();
    invoke_generic_verifier(
        &cfg,
        verifier_program_acc,
        &verifier_accounts,
        build_governance_verifier_data(&message_id, proof),
    )?;

    token_cfg.state = next_state;
    write_borsh::<TokenConfig>(token_cfg_acc, &token_cfg)?;
    ensure_marker_account(
        program_id,
        payer,
        marker_acc,
        SCCP_DOMAIN_SORA,
        &message_id,
        marker_bump,
    )?;
    mark_message_processed(marker_acc)?;
    Ok(())
}

fn burn(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sora_asset_id: [u8; 32],
    amount: u64,
    dest_domain: u32,
    recipient: [u8; 32],
) -> ProgramResult {
    ensure_supported_domain(dest_domain)?;
    if dest_domain == SCCP_DOMAIN_SOL {
        return Err(SccpError::DomainEqualsLocal.into());
    }
    if amount == 0 {
        return Err(SccpError::AmountIsZero.into());
    }
    if recipient == [0u8; 32] {
        return Err(SccpError::RecipientIsZero.into());
    }
    // EVM recipient encoding: 20-byte address right-aligned in a 32-byte field.
    if matches!(
        dest_domain,
        SCCP_DOMAIN_ETH | SCCP_DOMAIN_BSC | SCCP_DOMAIN_TRON
    ) {
        if recipient[..12] != [0u8; 12] {
            return Err(SccpError::RecipientNotCanonical.into());
        }
    }

    let mut it = accounts.iter();
    let user = next_account_info(&mut it)?;
    let config_acc = next_account_info(&mut it)?;
    let token_cfg_acc = next_account_info(&mut it)?;
    let user_token_acc = next_account_info(&mut it)?;
    let mint_acc = next_account_info(&mut it)?;
    let burn_record_acc = next_account_info(&mut it)?;
    let system_program = next_account_info(&mut it)?;
    let token_program = next_account_info(&mut it)?;

    if !user.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if *system_program.key != solana_program::system_program::id() {
        return Err(ProgramError::IncorrectProgramId);
    }
    if *token_program.key != spl_token::id() {
        return Err(ProgramError::IncorrectProgramId);
    }

    let mut cfg = load_config(program_id, config_acc)?;

    let bit = 1u64
        .checked_shl(dest_domain)
        .ok_or(SccpError::DomainUnsupported)?;
    if (cfg.outbound_paused_mask & bit) != 0 {
        msg!(
            "sccp:burn outbound paused dest_domain={} mask={:#x}",
            dest_domain,
            cfg.outbound_paused_mask
        );
        return Err(SccpError::OutboundDomainPaused.into());
    }

    // Load token config for this SORA asset id.
    let (expected_token_cfg, _bump) = token_config_pda(program_id, &sora_asset_id);
    if *token_cfg_acc.key != expected_token_cfg {
        return Err(SccpError::InvalidPda.into());
    }
    let token_cfg = load_token_config(token_cfg_acc)?;
    if token_cfg.mint != *mint_acc.key {
        return Err(SccpError::MintMismatch.into());
    }
    if token_cfg.state != TokenState::Active {
        return Err(SccpError::TokenNotActive.into());
    }

    // Ensure the user token account belongs to the user and is for the expected mint.
    if user_token_acc.owner != &spl_token::id() {
        return Err(SccpError::InvalidOwner.into());
    }
    if mint_acc.owner != &spl_token::id() {
        return Err(SccpError::InvalidOwner.into());
    }
    let ta = TokenAccount::unpack(&user_token_acc.data.borrow())?;
    if ta.owner != *user.key {
        return Err(SccpError::InvalidOwner.into());
    }
    if ta.mint != *mint_acc.key {
        return Err(SccpError::MintMismatch.into());
    }
    let mint_state =
        TokenMint::unpack(&mint_acc.data.borrow()).map_err(|_| ProgramError::InvalidAccountData)?;
    ensure_bridge_controlled_mint(&mint_state, config_acc.key)?;

    // Global monotonically-increasing nonce.
    if cfg.outbound_nonce == u64::MAX {
        return Err(SccpError::NonceOverflow.into());
    }
    cfg.outbound_nonce = cfg.outbound_nonce.saturating_add(1);
    write_borsh::<Config>(config_acc, &cfg)?;

    let payload = BurnPayloadV1 {
        version: 1,
        source_domain: SCCP_DOMAIN_SOL,
        dest_domain,
        nonce: cfg.outbound_nonce,
        sora_asset_id,
        amount: amount as u128,
        recipient,
    };
    let payload_bytes = payload.encode_scale();
    let message_id = burn_message_id(&payload_bytes);

    let (expected_burn, bump) = burn_record_pda(program_id, &message_id);
    if *burn_record_acc.key != expected_burn {
        return Err(SccpError::InvalidPda.into());
    }
    if burn_record_acc.owner == program_id {
        return Err(SccpError::BurnRecordAlreadyExists.into());
    }

    // Burn tokens from the user's token account.
    let ix = token_ix::burn(
        token_program.key,
        user_token_acc.key,
        mint_acc.key,
        user.key,
        &[],
        amount,
    )?;
    invoke(
        &ix,
        &[user_token_acc.clone(), mint_acc.clone(), user.clone()],
    )?;

    // Create burn record PDA.
    create_pda_account(
        user,
        burn_record_acc,
        BurnRecord::LEN,
        program_id,
        &[SEED_PREFIX, SEED_BURN, &message_id, &[bump]],
    )?;

    let slot = Clock::get()?.slot;
    let rec = BurnRecord {
        version: ACCOUNT_VERSION_V1,
        bump,
        message_id,
        payload: payload_bytes,
        sender: *user.key,
        mint: *mint_acc.key,
        slot,
    };
    write_borsh::<BurnRecord>(burn_record_acc, &rec)?;

    msg!("sccp:burn:v1 message_id={:02x?}", message_id);
    Ok(())
}

fn mint_from_proof(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    source_domain: u32,
    payload: &[u8],
    proof: &[u8],
) -> ProgramResult {
    ensure_supported_domain(source_domain)?;
    if source_domain == SCCP_DOMAIN_SOL {
        return Err(SccpError::DomainEqualsLocal.into());
    }
    if payload.len() != BurnPayloadV1::ENCODED_LEN {
        return Err(SccpError::PayloadInvalidLength.into());
    }
    if proof.len() > 16 * 1024 {
        return Err(ProgramError::InvalidInstructionData);
    }

    let mut it = accounts.iter();
    let payer = next_account_info(&mut it)?;
    let config_acc = next_account_info(&mut it)?;
    let token_cfg_acc = next_account_info(&mut it)?;
    let mint_acc = next_account_info(&mut it)?;
    let recipient_token_acc = next_account_info(&mut it)?;
    let marker_acc = next_account_info(&mut it)?;
    let system_program = next_account_info(&mut it)?;
    let token_program = next_account_info(&mut it)?;

    if !payer.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if *system_program.key != solana_program::system_program::id() {
        return Err(ProgramError::IncorrectProgramId);
    }
    if *token_program.key != spl_token::id() {
        return Err(ProgramError::IncorrectProgramId);
    }

    let cfg = load_config(program_id, config_acc)?;

    let bit = 1u64
        .checked_shl(source_domain)
        .ok_or(SccpError::DomainUnsupported)?;
    if (cfg.inbound_paused_mask & bit) != 0 {
        return Err(SccpError::InboundDomainPaused.into());
    }

    let message_id = burn_message_id(payload);
    let (expected_marker, bump) = inbound_marker_pda(program_id, source_domain, &message_id);
    if *marker_acc.key != expected_marker {
        return Err(SccpError::InvalidPda.into());
    }

    // Check marker status if it already exists.
    if marker_acc.owner == program_id {
        let m = load_inbound_marker(marker_acc)?;
        match m.status {
            InboundStatus::Processed => return Err(SccpError::InboundAlreadyProcessed.into()),
            InboundStatus::Invalidated => return Err(SccpError::ProofInvalidated.into()),
            InboundStatus::None => {}
        }
    }

    let p = decode_burn_payload_v1(payload)
        .map_err(|_| ProgramError::from(SccpError::PayloadInvalidLength))?;
    if p.version != 1 {
        return Err(SccpError::DomainUnsupported.into());
    }
    if p.source_domain != source_domain {
        return Err(SccpError::DomainUnsupported.into());
    }
    if p.dest_domain != SCCP_DOMAIN_SOL {
        return Err(SccpError::DomainUnsupported.into());
    }
    if p.amount == 0 {
        return Err(SccpError::AmountIsZero.into());
    }
    if p.recipient == [0u8; 32] {
        return Err(SccpError::RecipientIsZero.into());
    }
    if p.amount > u64::MAX as u128 {
        return Err(SccpError::AmountTooLarge.into());
    }

    // Token config lookup.
    let (expected_token_cfg, _bump) = token_config_pda(program_id, &p.sora_asset_id);
    if *token_cfg_acc.key != expected_token_cfg {
        return Err(SccpError::InvalidPda.into());
    }
    let token_cfg = load_token_config(token_cfg_acc)?;
    if token_cfg.mint != *mint_acc.key {
        return Err(SccpError::MintMismatch.into());
    }
    if token_cfg.state != TokenState::Active {
        return Err(SccpError::TokenNotActive.into());
    }

    // Ensure the recipient token account is owned by the recipient encoded in the payload.
    // Recipient encoding for Solana is a 32-byte ed25519 public key (wallet), not a token account address.
    // This prevents a relayer from redirecting a valid proof to an arbitrary token account.
    if recipient_token_acc.owner != &spl_token::id() {
        return Err(SccpError::InvalidOwner.into());
    }
    if mint_acc.owner != &spl_token::id() {
        return Err(SccpError::InvalidOwner.into());
    }
    let rta = TokenAccount::unpack(&recipient_token_acc.data.borrow())?;
    if rta.mint != *mint_acc.key {
        return Err(SccpError::MintMismatch.into());
    }
    if rta.owner.to_bytes() != p.recipient {
        return Err(SccpError::InvalidOwner.into());
    }
    let mint_state =
        TokenMint::unpack(&mint_acc.data.borrow()).map_err(|_| ProgramError::InvalidAccountData)?;
    ensure_bridge_controlled_mint(&mint_state, config_acc.key)?;

    // Verifier hook: fail-closed until configured.
    if cfg.verifier_program == Pubkey::default() {
        return Err(SccpError::VerifierNotSet.into());
    }

    // CPI requires the verifier program account to be provided as an account to this instruction.
    // This allows the runtime to resolve and execute the verifier program.
    let verifier_program_acc = next_account_info(&mut it)?;
    if *verifier_program_acc.key != cfg.verifier_program || !verifier_program_acc.executable {
        return Err(ProgramError::IncorrectProgramId);
    }

    // Pass through remaining accounts to verifier CPI (light client state, etc).
    let verifier_accounts: Vec<AccountInfo> = it.cloned().collect();
    let verifier_metas: Vec<AccountMeta> = verifier_accounts
        .iter()
        .map(|a| AccountMeta {
            pubkey: *a.key,
            is_signer: a.is_signer,
            is_writable: a.is_writable,
        })
        .collect();

    let mut verifier_data = Vec::with_capacity(1 + 4 + 32 + payload.len() + proof.len());
    verifier_data.push(1u8); // verifyBurnProof:v1
    verifier_data.extend_from_slice(&source_domain.to_le_bytes());
    verifier_data.extend_from_slice(&message_id);
    verifier_data.extend_from_slice(payload);
    verifier_data.extend_from_slice(proof);

    let verifier_ix = Instruction {
        program_id: cfg.verifier_program,
        accounts: verifier_metas,
        data: verifier_data,
    };
    let mut verifier_cpi_accounts = verifier_accounts.clone();
    verifier_cpi_accounts.push(verifier_program_acc.clone());
    invoke(&verifier_ix, &verifier_cpi_accounts)
        .map_err(|_| ProgramError::from(SccpError::ProofVerificationFailed))?;

    // Mint SPL tokens to recipient's token account. The SPL mint authority must be a PDA.
    // Convention: mint authority = config PDA.
    let (config_key, config_bump) = config_pda(program_id);
    if *config_acc.key != config_key {
        return Err(SccpError::InvalidPda.into());
    }
    let mint_to_ix = token_ix::mint_to(
        token_program.key,
        mint_acc.key,
        recipient_token_acc.key,
        config_acc.key,
        &[],
        p.amount as u64,
    )?;
    invoke_signed(
        &mint_to_ix,
        &[
            mint_acc.clone(),
            recipient_token_acc.clone(),
            config_acc.clone(),
        ],
        &[&[SEED_PREFIX, SEED_CONFIG, &[config_bump]]],
    )?;

    // Mark inbound processed.
    ensure_marker_account(
        program_id,
        payer,
        marker_acc,
        source_domain,
        &message_id,
        bump,
    )?;
    let mut m = load_inbound_marker(marker_acc)?;
    m.status = InboundStatus::Processed;
    write_borsh::<InboundMarker>(marker_acc, &m)?;

    msg!("sccp:mint:v1 message_id={:02x?}", message_id);
    Ok(())
}

fn config_pda(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[SEED_PREFIX, SEED_CONFIG], program_id)
}

fn token_config_pda(program_id: &Pubkey, sora_asset_id: &[u8; 32]) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[SEED_PREFIX, SEED_TOKEN, sora_asset_id], program_id)
}

fn mint_pda(program_id: &Pubkey, sora_asset_id: &[u8; 32]) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[SEED_PREFIX, SEED_MINT, sora_asset_id], program_id)
}

fn burn_record_pda(program_id: &Pubkey, message_id: &H256) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[SEED_PREFIX, SEED_BURN, message_id], program_id)
}

fn inbound_marker_pda(program_id: &Pubkey, source_domain: u32, message_id: &H256) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            SEED_PREFIX,
            SEED_INBOUND,
            &source_domain.to_le_bytes(),
            message_id,
        ],
        program_id,
    )
}

fn ensure_supported_domain(domain: u32) -> Result<(), ProgramError> {
    match domain {
        SCCP_DOMAIN_SORA | SCCP_DOMAIN_ETH | SCCP_DOMAIN_BSC | SCCP_DOMAIN_SOL
        | SCCP_DOMAIN_TON | SCCP_DOMAIN_TRON => Ok(()),
        _ => Err(SccpError::DomainUnsupported.into()),
    }
}

fn load_config(program_id: &Pubkey, config_acc: &AccountInfo) -> Result<Config, ProgramError> {
    let (expected, _bump) = config_pda(program_id);
    if *config_acc.key != expected {
        return Err(SccpError::InvalidPda.into());
    }
    if config_acc.owner != program_id {
        return Err(SccpError::ConfigNotInitialized.into());
    }
    let cfg = read_borsh::<Config>(config_acc)?;
    if cfg.version != ACCOUNT_VERSION_V1 {
        return Err(SccpError::InvalidAccountSize.into());
    }
    Ok(cfg)
}

fn load_token_config(token_cfg_acc: &AccountInfo) -> Result<TokenConfig, ProgramError> {
    let token_cfg = read_borsh::<TokenConfig>(token_cfg_acc)?;
    if token_cfg.version != ACCOUNT_VERSION_V1 {
        return Err(SccpError::InvalidAccountSize.into());
    }
    Ok(token_cfg)
}

fn validate_governance_label(label: &[u8; 32]) -> ProgramResult {
    let mut seen_zero = false;
    let mut non_zero_len = 0usize;
    for byte in label {
        if *byte == 0 {
            seen_zero = true;
            continue;
        }
        if seen_zero || !(0x20..=0x7e).contains(byte) {
            return Err(SccpError::TokenMetadataInvalid.into());
        }
        non_zero_len += 1;
    }
    if non_zero_len == 0 {
        return Err(SccpError::TokenMetadataInvalid.into());
    }
    Ok(())
}

fn load_inbound_marker(marker_acc: &AccountInfo) -> Result<InboundMarker, ProgramError> {
    let marker = read_borsh::<InboundMarker>(marker_acc)?;
    if marker.version != ACCOUNT_VERSION_V1 {
        return Err(SccpError::InvalidAccountSize.into());
    }
    Ok(marker)
}

fn ensure_bridge_controlled_mint(mint_state: &TokenMint, config_key: &Pubkey) -> ProgramResult {
    if mint_state.mint_authority != COption::Some(*config_key) {
        return Err(SccpError::MintAuthorityMismatch.into());
    }
    match mint_state.freeze_authority {
        COption::None => Ok(()),
        COption::Some(authority) if authority == *config_key => Ok(()),
        _ => Err(SccpError::FreezeAuthorityMismatch.into()),
    }
}

fn require_governor_authority(authority: &AccountInfo, governor: &Pubkey) -> ProgramResult {
    if !authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if authority.key != governor {
        return Err(SccpError::NotGovernor.into());
    }
    Ok(())
}

fn build_governance_verifier_data(message_id: &H256, proof: &[u8]) -> Vec<u8> {
    let mut verifier_data = Vec::with_capacity(1 + 32 + proof.len());
    verifier_data.push(4u8); // verifyGovernanceProof:v1
    verifier_data.extend_from_slice(message_id);
    verifier_data.extend_from_slice(proof);
    verifier_data
}

fn invoke_generic_verifier<'a>(
    cfg: &Config,
    verifier_program_acc: &AccountInfo<'a>,
    verifier_accounts: &[AccountInfo<'a>],
    data: Vec<u8>,
) -> Result<(), ProgramError> {
    if *verifier_program_acc.key != cfg.verifier_program || !verifier_program_acc.executable {
        return Err(ProgramError::IncorrectProgramId);
    }
    let verifier_metas: Vec<AccountMeta> = verifier_accounts
        .iter()
        .map(|a| AccountMeta {
            pubkey: *a.key,
            is_signer: a.is_signer,
            is_writable: a.is_writable,
        })
        .collect();

    let verifier_ix = Instruction {
        program_id: cfg.verifier_program,
        accounts: verifier_metas,
        data,
    };
    let mut verifier_cpi_accounts = verifier_accounts.to_vec();
    verifier_cpi_accounts.push(verifier_program_acc.clone());
    invoke(&verifier_ix, &verifier_cpi_accounts)
        .map_err(|_| ProgramError::from(SccpError::ProofVerificationFailed))
}

fn ensure_message_unprocessed(program_id: &Pubkey, marker_acc: &AccountInfo) -> ProgramResult {
    if marker_acc.owner == program_id {
        let marker = load_inbound_marker(marker_acc)?;
        match marker.status {
            InboundStatus::Processed => return Err(SccpError::InboundAlreadyProcessed.into()),
            InboundStatus::Invalidated => return Err(SccpError::ProofInvalidated.into()),
            InboundStatus::None => {}
        }
    }
    Ok(())
}

fn mark_message_processed(marker_acc: &AccountInfo) -> ProgramResult {
    let mut marker = load_inbound_marker(marker_acc)?;
    marker.status = InboundStatus::Processed;
    write_borsh::<InboundMarker>(marker_acc, &marker)
}

fn create_pda_account<'a>(
    payer: &AccountInfo<'a>,
    new_acc: &AccountInfo<'a>,
    space: usize,
    owner: &Pubkey,
    signer_seeds: &[&[u8]],
) -> ProgramResult {
    if new_acc.owner != &solana_program::system_program::id() {
        return Err(SccpError::InvalidOwner.into());
    }
    if new_acc.data_len() != 0 {
        return Err(SccpError::InvalidAccountSize.into());
    }

    let rent = Rent::get()?;
    let lamports = rent.minimum_balance(space);

    if new_acc.lamports() == 0 {
        let ix = system_instruction::create_account(
            payer.key,
            new_acc.key,
            lamports,
            space as u64,
            owner,
        );
        invoke_signed(&ix, &[payer.clone(), new_acc.clone()], &[signer_seeds])?;
        return Ok(());
    }

    let top_up = lamports.saturating_sub(new_acc.lamports());
    if top_up > 0 {
        let ix = system_instruction::transfer(payer.key, new_acc.key, top_up);
        invoke(&ix, &[payer.clone(), new_acc.clone()])?;
    }

    let allocate_ix = system_instruction::allocate(new_acc.key, space as u64);
    invoke_signed(&allocate_ix, &[new_acc.clone()], &[signer_seeds])?;

    let assign_ix = system_instruction::assign(new_acc.key, owner);
    invoke_signed(&assign_ix, &[new_acc.clone()], &[signer_seeds])?;

    Ok(())
}

fn ensure_marker_account<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    marker_acc: &AccountInfo<'a>,
    source_domain: u32,
    message_id: &H256,
    bump: u8,
) -> ProgramResult {
    if marker_acc.owner != program_id {
        // Create.
        create_pda_account(
            payer,
            marker_acc,
            InboundMarker::LEN,
            program_id,
            &[
                SEED_PREFIX,
                SEED_INBOUND,
                &source_domain.to_le_bytes(),
                message_id,
                &[bump],
            ],
        )?;
        let m = InboundMarker {
            version: ACCOUNT_VERSION_V1,
            bump,
            status: InboundStatus::None,
        };
        write_borsh::<InboundMarker>(marker_acc, &m)?;
    }
    Ok(())
}

fn read_borsh<T: BorshDeserialize>(acc: &AccountInfo) -> Result<T, ProgramError> {
    T::try_from_slice(&acc.data.borrow()).map_err(|_| ProgramError::InvalidAccountData)
}

fn write_borsh<T: BorshSerialize>(acc: &AccountInfo, v: &T) -> Result<(), ProgramError> {
    let mut data = acc.data.borrow_mut();
    v.serialize(&mut &mut data[..])
        .map_err(|_| ProgramError::AccountDataTooSmall)
}
