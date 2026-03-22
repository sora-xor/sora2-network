use anyhow::{anyhow, bail, Context, Result};
use borsh::{BorshDeserialize, BorshSerialize};
use clap::{Parser, Subcommand};
use libsecp256k1::{sign, Message, PublicKey, SecretKey};
use serde_json::json;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    instruction::{AccountMeta, Instruction},
    native_token::LAMPORTS_PER_SOL,
    program_pack::Pack,
    pubkey::Pubkey,
    signature::{read_keypair_file, Keypair, Signature, Signer},
    system_instruction, system_program, sysvar,
    transaction::Transaction,
};
use spl_token::state::Account as TokenAccount;

use sccp_sol::{burn_message_id, BurnPayloadV1, SCCP_DOMAIN_SOL, SCCP_DOMAIN_SORA};
use sccp_sol_program::{BurnRecord, Config as RouterConfig, SccpInstruction, TokenConfig};
use sccp_sol_verifier_program::{
    Commitment as VerifierCommitment, Config as VerifierConfig, MmrLeaf as VerifierMmrLeaf,
    MmrProof as VerifierMmrProof, SoraBurnProofV1, ValidatorProof as VerifierValidatorProof,
    ValidatorSet as VerifierValidatorSet, VerifierInstruction,
};

const SEED_PREFIX: &[u8] = b"sccp";
const SEED_VERIFIER: &[u8] = b"verifier";
const SEED_CONFIG: &[u8] = b"config";
const SEED_TOKEN: &[u8] = b"token";
const SEED_MINT: &[u8] = b"mint";
const SEED_BURN: &[u8] = b"burn";
const SEED_INBOUND: &[u8] = b"inbound";

const ALICE_RECIPIENT32_HEX: &str =
    "d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d";
const DEFAULT_SORA_ASSET_ID_HEX: &str =
    "0200000000000000000000000000000000000000000000000000000000000000";
const USER_MIN_SOL_BALANCE: u64 = LAMPORTS_PER_SOL;

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Plan(PlanArgs),
    Execute(ExecuteArgs),
    Bootstrap(BootstrapArgs),
}

#[derive(Parser, Debug)]
struct PlanArgs {
    #[arg(long)]
    router_program_id: String,
    #[arg(long, default_value = DEFAULT_SORA_ASSET_ID_HEX)]
    sora_asset_id_hex: String,
    #[arg(long, default_value_t = 5)]
    burn_amount: u64,
    #[arg(long, default_value = ALICE_RECIPIENT32_HEX)]
    recipient32_hex: String,
}

#[derive(Parser, Debug)]
struct ExecuteArgs {
    #[arg(long, default_value = "http://127.0.0.1:8899")]
    rpc_url: String,
    #[arg(long, default_value = "~/.config/solana/id.json")]
    payer_keypair: String,
    #[arg(long)]
    router_program_id: String,
    #[arg(long)]
    verifier_program_id: String,
    #[arg(long, default_value = DEFAULT_SORA_ASSET_ID_HEX)]
    sora_asset_id_hex: String,
    #[arg(long, default_value_t = 5)]
    burn_amount: u64,
    #[arg(long, default_value_t = 11)]
    mint_amount: u64,
    #[arg(long, default_value = ALICE_RECIPIENT32_HEX)]
    recipient32_hex: String,
    #[arg(long, default_value_t = 20)]
    airdrop_sol: u64,
}

#[derive(Parser, Debug)]
struct BootstrapArgs {
    #[arg(long, default_value = "http://127.0.0.1:8899")]
    rpc_url: String,
    #[arg(long, default_value = "~/.config/solana/id.json")]
    payer_keypair: String,
    #[arg(long)]
    governor_pubkey: String,
    #[arg(long)]
    router_program_id: String,
    #[arg(long)]
    verifier_program_id: String,
    #[arg(long)]
    governor_keypair: Option<String>,
    #[arg(long)]
    latest_beefy_block: Option<u64>,
    #[arg(long)]
    current_validator_set_id: Option<u64>,
    #[arg(long)]
    current_validator_set_len: Option<u32>,
    #[arg(long)]
    current_validator_set_root_hex: Option<String>,
    #[arg(long)]
    next_validator_set_id: Option<u64>,
    #[arg(long)]
    next_validator_set_len: Option<u32>,
    #[arg(long)]
    next_validator_set_root_hex: Option<String>,
}

#[derive(Clone)]
struct BurnPlan {
    payload: BurnPayloadV1,
    payload_bytes: [u8; BurnPayloadV1::ENCODED_LEN],
    message_id: [u8; 32],
    burn_record_pda: Pubkey,
}

struct VerifierBootstrapInput {
    latest_beefy_block: u64,
    current_validator_set: VerifierValidatorSet,
    next_validator_set: VerifierValidatorSet,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Plan(args) => cmd_plan(args),
        Command::Execute(args) => cmd_execute(args),
        Command::Bootstrap(args) => cmd_bootstrap(args),
    }
}

fn cmd_plan(args: PlanArgs) -> Result<()> {
    let router_program_id = parse_pubkey(&args.router_program_id, "router program id")?;
    let sora_asset_id = parse_hex32(&args.sora_asset_id_hex, "sora_asset_id_hex")?;
    let recipient = parse_hex32(&args.recipient32_hex, "recipient32_hex")?;
    let plan = build_burn_plan(
        router_program_id,
        sora_asset_id,
        args.burn_amount,
        recipient,
    );
    print_plan(&plan, router_program_id, sora_asset_id);
    Ok(())
}

fn cmd_execute(args: ExecuteArgs) -> Result<()> {
    let rpc = RpcClient::new_with_commitment(args.rpc_url.clone(), CommitmentConfig::confirmed());
    let payer_path = expand_home(&args.payer_keypair)?;
    let payer = read_keypair_file(&payer_path).map_err(|err| {
        anyhow!(
            "failed to read payer keypair {}: {err}",
            payer_path.display()
        )
    })?;
    let router_program_id = parse_pubkey(&args.router_program_id, "router program id")?;
    let verifier_program_id = parse_pubkey(&args.verifier_program_id, "verifier program id")?;
    let sora_asset_id = parse_hex32(&args.sora_asset_id_hex, "sora_asset_id_hex")?;
    let recipient = parse_hex32(&args.recipient32_hex, "recipient32_hex")?;
    let burn_plan = build_burn_plan(
        router_program_id,
        sora_asset_id,
        args.burn_amount,
        recipient,
    );

    ensure_airdrop(&rpc, &payer, args.airdrop_sol)?;

    let (router_config, _) = config_pda(&router_program_id);
    let (verifier_config, _) = verifier_config_pda(&verifier_program_id);
    let (token_cfg, _) = token_config_pda(&router_program_id, &sora_asset_id);
    let (mint, _) = mint_pda(&router_program_id, &sora_asset_id);
    let validator_root = synthetic_validator_set_root()?;
    let verifier_bootstrap = VerifierBootstrapInput {
        latest_beefy_block: 0,
        current_validator_set: VerifierValidatorSet {
            id: 1,
            len: 4,
            root: validator_root,
        },
        next_validator_set: VerifierValidatorSet {
            id: 2,
            len: 4,
            root: validator_root,
        },
    };

    initialize_router(
        &rpc,
        &payer,
        &payer,
        payer.pubkey(),
        router_program_id,
        router_config,
    )?;
    set_router_verifier(
        &rpc,
        &payer,
        &payer,
        router_program_id,
        router_config,
        verifier_program_id,
    )?;
    initialize_verifier(
        &rpc,
        &payer,
        &payer,
        payer.pubkey(),
        verifier_program_id,
        verifier_config,
        &verifier_bootstrap,
    )?;
    deploy_token(
        &rpc,
        &payer,
        &payer,
        router_program_id,
        router_config,
        token_cfg,
        mint,
        sora_asset_id,
    )?;

    let user = Keypair::new();
    let user_token = Keypair::new();
    ensure_balance(&rpc, &user.pubkey(), USER_MIN_SOL_BALANCE)?;
    create_user_token_account(&rpc, &payer, &user, &user_token, mint)?;

    let vote_authorities = collect_vote_authorities(&rpc)?;
    let mint_summary = mint_to_user_from_synthetic_sora_proof(
        &rpc,
        &payer,
        router_program_id,
        verifier_program_id,
        router_config,
        verifier_config,
        token_cfg,
        mint,
        &user,
        user_token.pubkey(),
        sora_asset_id,
        args.mint_amount,
    )?;

    execute_burn(
        &rpc,
        &payer,
        &user,
        router_program_id,
        router_config,
        token_cfg,
        user_token.pubkey(),
        mint,
        &burn_plan,
    )?;

    let burn_record_account = rpc
        .get_account(&burn_plan.burn_record_pda)
        .with_context(|| format!("failed to fetch burn record {}", burn_plan.burn_record_pda))?;
    let burn_record = BurnRecord::try_from_slice(&burn_record_account.data)
        .context("failed to decode on-chain burn record")?;
    let user_token_state = rpc
        .get_account(&user_token.pubkey())
        .context("failed to fetch user token account")?;
    let token_state =
        TokenAccount::unpack(&user_token_state.data).context("failed to unpack token account")?;
    let router_cfg = rpc
        .get_account(&router_config)
        .context("failed to fetch router config")?;
    let router_cfg =
        RouterConfig::try_from_slice(&router_cfg.data).context("failed to decode router config")?;
    let token_cfg_account = rpc
        .get_account(&token_cfg)
        .context("failed to fetch token config")?;
    let token_cfg_state = TokenConfig::try_from_slice(&token_cfg_account.data)
        .context("failed to decode token config")?;
    let verifier_cfg = rpc
        .get_account(&verifier_config)
        .context("failed to fetch verifier config")?;
    let verifier_cfg = VerifierConfig::try_from_slice(&verifier_cfg.data)
        .context("failed to decode verifier config")?;

    let summary = json!({
        "rpcUrl": args.rpc_url,
        "payer": payer.pubkey().to_string(),
        "routerProgramId": router_program_id.to_string(),
        "verifierProgramId": verifier_program_id.to_string(),
        "routerConfigPda": router_config.to_string(),
        "verifierConfigPda": verifier_config.to_string(),
        "tokenConfigPda": token_cfg.to_string(),
        "mint": mint.to_string(),
        "soraAssetId": format!("0x{}", hex::encode(sora_asset_id)),
        "user": user.pubkey().to_string(),
        "userTokenAccount": user_token.pubkey().to_string(),
        "soraRecipient32": format!("0x{}", hex::encode(recipient)),
        "mintAmount": args.mint_amount,
        "burnAmount": args.burn_amount,
        "mintSummary": mint_summary,
        "burnPayloadHex": format!("0x{}", hex::encode(burn_plan.payload_bytes)),
        "messageId": format!("0x{}", hex::encode(burn_plan.message_id)),
        "burnRecordPda": burn_plan.burn_record_pda.to_string(),
        "burnRecordSlot": burn_record.slot,
        "burnRecordSender": burn_record.sender.to_string(),
        "burnRecordMint": burn_record.mint.to_string(),
        "routerOutboundNonce": router_cfg.outbound_nonce,
        "routerVerifierProgram": router_cfg.verifier_program.to_string(),
        "tokenConfigMint": token_cfg_state.mint.to_string(),
        "verifierLatestBeefyBlock": verifier_cfg.latest_beefy_block,
        "userRemainingTokenAmount": token_state.amount,
        "voteAuthorities": vote_authorities,
    });
    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}

fn cmd_bootstrap(args: BootstrapArgs) -> Result<()> {
    let rpc = RpcClient::new_with_commitment(args.rpc_url.clone(), CommitmentConfig::confirmed());
    let payer_path = expand_home(&args.payer_keypair)?;
    let payer = read_keypair_file(&payer_path).map_err(|err| {
        anyhow!(
            "failed to read payer keypair {}: {err}",
            payer_path.display()
        )
    })?;
    let governor_pubkey = parse_pubkey(&args.governor_pubkey, "governor pubkey")?;
    let router_program_id = parse_pubkey(&args.router_program_id, "router program id")?;
    let verifier_program_id = parse_pubkey(&args.verifier_program_id, "verifier program id")?;
    let bootstrap_input = parse_verifier_bootstrap_input(&args)?;

    let governor_keypair = match &args.governor_keypair {
        Some(path) => {
            let path = expand_home(path)?;
            let keypair = read_keypair_file(&path).map_err(|err| {
                anyhow!("failed to read governor keypair {}: {err}", path.display())
            })?;
            if keypair.pubkey() != governor_pubkey {
                bail!(
                    "governor keypair pubkey {} did not match expected governor pubkey {}",
                    keypair.pubkey(),
                    governor_pubkey
                );
            }
            Some(keypair)
        }
        None => None,
    };
    let governor_signer = match governor_keypair.as_ref() {
        Some(keypair) => keypair,
        None if payer.pubkey() == governor_pubkey => &payer,
        None => bail!(
            "governor-keypair is required when governor pubkey {} differs from payer {}",
            governor_pubkey,
            payer.pubkey()
        ),
    };

    let (router_config, _) = config_pda(&router_program_id);
    let (verifier_config, _) = verifier_config_pda(&verifier_program_id);

    let mut router_initialize = json!({
        "configPda": router_config.to_string(),
    });
    let mut set_verifier_summary = json!({
        "configPda": router_config.to_string(),
        "verifierProgramId": verifier_program_id.to_string(),
    });
    let mut verifier_initialize = json!({
        "configPda": verifier_config.to_string(),
    });

    let mut router_cfg = if let Some(account) = maybe_get_account(&rpc, &router_config)? {
        if account.owner != router_program_id {
            bail!(
                "router config {} exists with unexpected owner {}",
                router_config,
                account.owner
            );
        }
        let cfg = RouterConfig::try_from_slice(&account.data)
            .context("failed to decode existing router config account")?;
        if cfg.governor != governor_pubkey {
            bail!(
                "router config governor {} did not match expected {}",
                cfg.governor,
                governor_pubkey
            );
        }
        router_initialize["status"] = json!("already-initialized");
        router_initialize["governor"] = json!(cfg.governor.to_string());
        cfg
    } else {
        let sig = initialize_router(
            &rpc,
            &payer,
            governor_signer,
            governor_pubkey,
            router_program_id,
            router_config,
        )?;
        let account = maybe_get_account(&rpc, &router_config)?
            .ok_or_else(|| anyhow!("router config account missing after initialize"))?;
        let cfg = RouterConfig::try_from_slice(&account.data)
            .context("failed to decode router config after initialize")?;
        router_initialize["status"] = json!("sent");
        router_initialize["signature"] = json!(sig.to_string());
        router_initialize["governor"] = json!(cfg.governor.to_string());
        cfg
    };

    if router_cfg.verifier_program == Pubkey::default() {
        let sig = set_router_verifier(
            &rpc,
            &payer,
            governor_signer,
            router_program_id,
            router_config,
            verifier_program_id,
        )?;
        router_cfg.verifier_program = verifier_program_id;
        set_verifier_summary["status"] = json!("sent");
        set_verifier_summary["signature"] = json!(sig.to_string());
    } else if router_cfg.verifier_program == verifier_program_id {
        set_verifier_summary["status"] = json!("already-bound");
    } else {
        bail!(
            "router config already bound to verifier {} instead of expected {}",
            router_cfg.verifier_program,
            verifier_program_id
        );
    }

    if let Some(account) = maybe_get_account(&rpc, &verifier_config)? {
        if account.owner != verifier_program_id {
            bail!(
                "verifier config {} exists with unexpected owner {}",
                verifier_config,
                account.owner
            );
        }
        let cfg = VerifierConfig::try_from_slice(&account.data)
            .context("failed to decode existing verifier config account")?;
        if cfg.governor != governor_pubkey {
            bail!(
                "verifier config governor {} did not match expected {}",
                cfg.governor,
                governor_pubkey
            );
        }
        verifier_initialize["status"] = json!("already-initialized");
        verifier_initialize["latestBeefyBlock"] = json!(cfg.latest_beefy_block);
    } else {
        let bootstrap_input = bootstrap_input.ok_or_else(|| {
            anyhow!(
                "verifier bootstrap inputs are required when the verifier config is not initialized"
            )
        })?;
        let sig = initialize_verifier(
            &rpc,
            &payer,
            governor_signer,
            governor_pubkey,
            verifier_program_id,
            verifier_config,
            &bootstrap_input,
        )?;
        verifier_initialize["status"] = json!("sent");
        verifier_initialize["signature"] = json!(sig.to_string());
        verifier_initialize["latestBeefyBlock"] = json!(bootstrap_input.latest_beefy_block);
    }

    let summary = json!({
        "rpcUrl": args.rpc_url,
        "payer": payer.pubkey().to_string(),
        "governor": governor_pubkey.to_string(),
        "governorSigner": governor_signer.pubkey().to_string(),
        "routerProgramId": router_program_id.to_string(),
        "verifierProgramId": verifier_program_id.to_string(),
        "router": {
            "initialize": router_initialize,
            "setVerifierProgram": set_verifier_summary,
        },
        "verifier": {
            "initialize": verifier_initialize,
        },
    });
    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}

fn print_plan(plan: &BurnPlan, router_program_id: Pubkey, sora_asset_id: [u8; 32]) {
    let out = json!({
        "routerProgramId": router_program_id.to_string(),
        "soraAssetId": format!("0x{}", hex::encode(sora_asset_id)),
        "burnPayloadHex": format!("0x{}", hex::encode(plan.payload_bytes)),
        "messageId": format!("0x{}", hex::encode(plan.message_id)),
        "burnRecordPda": plan.burn_record_pda.to_string(),
        "burnAmount": plan.payload.amount,
        "soraRecipient32": format!("0x{}", hex::encode(plan.payload.recipient)),
    });
    println!("{}", serde_json::to_string_pretty(&out).unwrap());
}

fn build_burn_plan(
    router_program_id: Pubkey,
    sora_asset_id: [u8; 32],
    burn_amount: u64,
    recipient: [u8; 32],
) -> BurnPlan {
    let payload = BurnPayloadV1 {
        version: 1,
        source_domain: SCCP_DOMAIN_SOL,
        dest_domain: SCCP_DOMAIN_SORA,
        nonce: 1,
        sora_asset_id,
        amount: burn_amount as u128,
        recipient,
    };
    let payload_bytes = payload.encode_scale();
    let message_id = burn_message_id(&payload_bytes);
    let (burn_record_pda, _) = burn_record_pda(&router_program_id, &message_id);
    BurnPlan {
        payload,
        payload_bytes,
        message_id,
        burn_record_pda,
    }
}

fn initialize_router(
    rpc: &RpcClient,
    payer: &Keypair,
    governor_signer: &Keypair,
    governor_pubkey: Pubkey,
    router_program_id: Pubkey,
    router_config: Pubkey,
) -> Result<Signature> {
    let mut accounts = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(router_config, false),
        AccountMeta::new_readonly(system_program::id(), false),
    ];
    if governor_signer.pubkey() != payer.pubkey() {
        accounts.push(AccountMeta::new_readonly(governor_signer.pubkey(), true));
    }
    let ix = Instruction {
        program_id: router_program_id,
        accounts,
        data: SccpInstruction::Initialize {
            governor: governor_pubkey,
        }
        .try_to_vec()
        .context("failed to encode router initialize")?,
    };
    send_with_optional_governor(rpc, payer, governor_signer, &[ix])
        .context("router initialize failed")
}

fn set_router_verifier(
    rpc: &RpcClient,
    payer: &Keypair,
    governor_signer: &Keypair,
    router_program_id: Pubkey,
    router_config: Pubkey,
    verifier_program_id: Pubkey,
) -> Result<Signature> {
    let ix = Instruction {
        program_id: router_program_id,
        accounts: vec![
            AccountMeta::new(governor_signer.pubkey(), true),
            AccountMeta::new(router_config, false),
        ],
        data: SccpInstruction::SetVerifierProgram {
            verifier_program: verifier_program_id,
        }
        .try_to_vec()
        .context("failed to encode set verifier")?,
    };
    send_with_optional_governor(rpc, payer, governor_signer, &[ix]).context("set verifier failed")
}

fn initialize_verifier(
    rpc: &RpcClient,
    payer: &Keypair,
    governor_signer: &Keypair,
    governor_pubkey: Pubkey,
    verifier_program_id: Pubkey,
    verifier_config: Pubkey,
    bootstrap_input: &VerifierBootstrapInput,
) -> Result<Signature> {
    let mut accounts = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(verifier_config, false),
        AccountMeta::new_readonly(system_program::id(), false),
    ];
    if governor_signer.pubkey() != payer.pubkey() {
        accounts.push(AccountMeta::new_readonly(governor_signer.pubkey(), true));
    }
    let ix = Instruction {
        program_id: verifier_program_id,
        accounts,
        data: VerifierInstruction::Initialize {
            governor: governor_pubkey,
            latest_beefy_block: bootstrap_input.latest_beefy_block,
            current_validator_set: bootstrap_input.current_validator_set,
            next_validator_set: bootstrap_input.next_validator_set,
        }
        .try_to_vec()
        .context("failed to encode verifier initialize")?,
    };
    send_with_optional_governor(rpc, payer, governor_signer, &[ix])
        .context("verifier initialize failed")
}

fn deploy_token(
    rpc: &RpcClient,
    payer: &Keypair,
    governor_signer: &Keypair,
    router_program_id: Pubkey,
    router_config: Pubkey,
    token_cfg: Pubkey,
    mint: Pubkey,
    sora_asset_id: [u8; 32],
) -> Result<Signature> {
    let mut accounts = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(router_config, false),
        AccountMeta::new(token_cfg, false),
        AccountMeta::new(mint, false),
        AccountMeta::new_readonly(system_program::id(), false),
        AccountMeta::new_readonly(spl_token::id(), false),
        AccountMeta::new_readonly(sysvar::rent::id(), false),
    ];
    if governor_signer.pubkey() != payer.pubkey() {
        accounts.push(AccountMeta::new_readonly(governor_signer.pubkey(), true));
    }
    let ix = Instruction {
        program_id: router_program_id,
        accounts,
        data: SccpInstruction::DeployToken {
            sora_asset_id,
            decimals: 18,
        }
        .try_to_vec()
        .context("failed to encode deploy token")?,
    };
    send_with_optional_governor(rpc, payer, governor_signer, &[ix]).context("deploy token failed")
}

fn create_user_token_account(
    rpc: &RpcClient,
    payer: &Keypair,
    user: &Keypair,
    user_token: &Keypair,
    mint: Pubkey,
) -> Result<Signature> {
    let token_rent = rpc
        .get_minimum_balance_for_rent_exemption(TokenAccount::LEN)
        .context("failed to fetch token-account rent")?;
    let create_ix = system_instruction::create_account(
        &payer.pubkey(),
        &user_token.pubkey(),
        token_rent,
        TokenAccount::LEN as u64,
        &spl_token::id(),
    );
    let init_ix = spl_token::instruction::initialize_account(
        &spl_token::id(),
        &user_token.pubkey(),
        &mint,
        &user.pubkey(),
    )
    .context("failed to build initialize_account")?;
    send_tx(rpc, payer, &[payer, user_token], &[create_ix, init_ix])
        .context("create user token account failed")
}

#[allow(clippy::too_many_arguments)]
fn mint_to_user_from_synthetic_sora_proof(
    rpc: &RpcClient,
    payer: &Keypair,
    router_program_id: Pubkey,
    verifier_program_id: Pubkey,
    router_config: Pubkey,
    verifier_config: Pubkey,
    token_cfg: Pubkey,
    mint: Pubkey,
    user: &Keypair,
    user_token: Pubkey,
    sora_asset_id: [u8; 32],
    amount: u64,
) -> Result<serde_json::Value> {
    let inbound_payload = BurnPayloadV1 {
        version: 1,
        source_domain: SCCP_DOMAIN_SORA,
        dest_domain: SCCP_DOMAIN_SOL,
        nonce: 1,
        sora_asset_id,
        amount: amount as u128,
        recipient: user.pubkey().to_bytes(),
    };
    let inbound_payload_bytes = inbound_payload.encode_scale();
    let inbound_message_id = burn_message_id(&inbound_payload_bytes);
    let (marker_pda, _) =
        inbound_marker_pda(&router_program_id, SCCP_DOMAIN_SORA, &inbound_message_id);

    let import = build_synthetic_commitment_and_proof(inbound_message_id)?;
    let submit_ix = Instruction {
        program_id: verifier_program_id,
        accounts: vec![AccountMeta::new(verifier_config, false)],
        data: VerifierInstruction::SubmitSignatureCommitment {
            commitment: import.commitment,
            validator_proof: import.validator_proof.clone(),
            latest_mmr_leaf: import.leaf,
            proof: import.mmr_proof.clone(),
        }
        .try_to_vec()
        .context("failed to encode submit signature commitment")?,
    };
    send_tx(rpc, payer, &[payer], &[submit_ix]).context("submit signature commitment failed")?;

    let burn_proof = SoraBurnProofV1 {
        mmr_proof: import.mmr_proof,
        leaf: import.leaf,
        digest_scale: import.digest_scale.clone(),
    }
    .try_to_vec()
    .context("failed to encode router burn proof")?;

    let mint_ix = Instruction {
        program_id: router_program_id,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(router_config, false),
            AccountMeta::new(token_cfg, false),
            AccountMeta::new(mint, false),
            AccountMeta::new(user_token, false),
            AccountMeta::new(marker_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(spl_token::id(), false),
            AccountMeta::new_readonly(verifier_program_id, false),
            AccountMeta::new_readonly(verifier_config, false),
        ],
        data: SccpInstruction::MintFromProof {
            source_domain: SCCP_DOMAIN_SORA,
            payload: inbound_payload_bytes.to_vec(),
            proof: burn_proof,
        }
        .try_to_vec()
        .context("failed to encode router mint_from_proof")?,
    };
    send_tx(rpc, payer, &[payer], &[mint_ix]).context("router mint_from_proof failed")?;

    Ok(json!({
        "sourceDomain": SCCP_DOMAIN_SORA,
        "messageId": format!("0x{}", hex::encode(inbound_message_id)),
        "markerPda": marker_pda.to_string(),
        "payloadHex": format!("0x{}", hex::encode(inbound_payload_bytes)),
        "digestScaleHex": format!("0x{}", hex::encode(import.digest_scale)),
    }))
}

fn execute_burn(
    rpc: &RpcClient,
    payer: &Keypair,
    user: &Keypair,
    router_program_id: Pubkey,
    router_config: Pubkey,
    token_cfg: Pubkey,
    user_token: Pubkey,
    mint: Pubkey,
    burn_plan: &BurnPlan,
) -> Result<Signature> {
    let ix = Instruction {
        program_id: router_program_id,
        accounts: vec![
            AccountMeta::new(user.pubkey(), true),
            AccountMeta::new(router_config, false),
            AccountMeta::new(token_cfg, false),
            AccountMeta::new(user_token, false),
            AccountMeta::new(mint, false),
            AccountMeta::new(burn_plan.burn_record_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(spl_token::id(), false),
        ],
        data: SccpInstruction::Burn {
            sora_asset_id: burn_plan.payload.sora_asset_id,
            amount: burn_plan.payload.amount as u64,
            dest_domain: SCCP_DOMAIN_SORA,
            recipient: burn_plan.payload.recipient,
        }
        .try_to_vec()
        .context("failed to encode burn")?,
    };
    send_tx(rpc, payer, &[payer, user], &[ix]).context("burn transaction failed")
}

fn send_tx(
    rpc: &RpcClient,
    payer: &Keypair,
    signers: &[&Keypair],
    instructions: &[Instruction],
) -> Result<Signature> {
    let blockhash = rpc
        .get_latest_blockhash()
        .context("failed to fetch latest blockhash")?;
    let tx =
        Transaction::new_signed_with_payer(instructions, Some(&payer.pubkey()), signers, blockhash);
    match rpc.send_and_confirm_transaction(&tx) {
        Ok(sig) => Ok(sig),
        Err(err) => {
            let simulation = rpc
                .simulate_transaction(&tx)
                .ok()
                .and_then(|response| response.value.logs)
                .unwrap_or_default();
            let logs = if simulation.is_empty() {
                String::new()
            } else {
                format!("\nprogram logs:\n{}", simulation.join("\n"))
            };
            Err(anyhow!(
                "failed to send and confirm transaction: {err}{logs}"
            ))
        }
    }
}

fn send_with_optional_governor(
    rpc: &RpcClient,
    payer: &Keypair,
    governor_signer: &Keypair,
    instructions: &[Instruction],
) -> Result<Signature> {
    if governor_signer.pubkey() == payer.pubkey() {
        send_tx(rpc, payer, &[payer], instructions)
    } else {
        send_tx(rpc, payer, &[payer, governor_signer], instructions)
    }
}

fn maybe_get_account(
    rpc: &RpcClient,
    account: &Pubkey,
) -> Result<Option<solana_sdk::account::Account>> {
    let response = rpc
        .get_account_with_commitment(account, CommitmentConfig::confirmed())
        .with_context(|| format!("failed to fetch account {account}"))?;
    Ok(response.value)
}

fn ensure_airdrop(rpc: &RpcClient, payer: &Keypair, sol: u64) -> Result<()> {
    let lamports = sol.saturating_mul(LAMPORTS_PER_SOL);
    ensure_balance(rpc, &payer.pubkey(), lamports)
}

fn ensure_balance(rpc: &RpcClient, account: &Pubkey, lamports: u64) -> Result<()> {
    let starting_balance = rpc
        .get_balance(account)
        .with_context(|| format!("failed to fetch balance for {account}"))?;
    if starting_balance >= lamports {
        return Ok(());
    }
    let sig = rpc
        .request_airdrop(account, lamports.saturating_sub(starting_balance))
        .context("airdrop request failed")?;
    rpc.confirm_transaction(&sig)
        .context("airdrop confirmation failed")?;
    for _ in 0..20 {
        let balance = rpc
            .get_balance(account)
            .with_context(|| format!("failed to refetch balance for {account} after airdrop"))?;
        if balance >= lamports {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    bail!(
        "airdrop for {} confirmed but balance never reached {} lamports",
        account,
        lamports
    );
}

fn collect_vote_authorities(rpc: &RpcClient) -> Result<Vec<serde_json::Value>> {
    let vote_accounts = rpc
        .get_vote_accounts()
        .context("failed to query vote accounts")?;
    let mut authorities = Vec::new();
    for vote in vote_accounts
        .current
        .iter()
        .chain(vote_accounts.delinquent.iter())
    {
        authorities.push(json!({
            "authorityPubkey": vote.vote_pubkey,
            "stake": vote.activated_stake,
        }));
    }
    if authorities.is_empty() {
        bail!("validator returned no vote authorities");
    }
    Ok(authorities)
}

struct SyntheticCommitment {
    commitment: VerifierCommitment,
    validator_proof: VerifierValidatorProof,
    leaf: VerifierMmrLeaf,
    mmr_proof: VerifierMmrProof,
    digest_scale: Vec<u8>,
}

fn build_synthetic_commitment_and_proof(message_id: [u8; 32]) -> Result<SyntheticCommitment> {
    let validator_sks = vec![
        SecretKey::parse(&[1u8; 32]).context("invalid synthetic sk 1")?,
        SecretKey::parse(&[2u8; 32]).context("invalid synthetic sk 2")?,
        SecretKey::parse(&[3u8; 32]).context("invalid synthetic sk 3")?,
        SecretKey::parse(&[4u8; 32]).context("invalid synthetic sk 4")?,
    ];
    let mut validator_addrs = Vec::new();
    let mut validator_leaf_hashes = Vec::new();
    for sk in &validator_sks {
        let (addr, _) = eth_address_from_secret(sk);
        validator_addrs.push(addr);
        validator_leaf_hashes.push(keccak256(&addr));
    }
    let validator_layers = merkle_layers(validator_leaf_hashes);
    let validator_root = *validator_layers
        .last()
        .and_then(|layer| layer.first())
        .ok_or_else(|| anyhow!("empty synthetic validator merkle tree"))?;

    let mut digest_scale = Vec::with_capacity(1 + 38);
    digest_scale.push(0x04);
    digest_scale.extend_from_slice(&[0x00, 0x02, 0x50, 0x43, 0x43, 0x53]);
    digest_scale.extend_from_slice(&message_id);
    let digest_hash = keccak256(&digest_scale);

    let leaf = VerifierMmrLeaf {
        version: 0,
        parent_number: 1,
        parent_hash: [0x55u8; 32],
        next_authority_set_id: 2,
        next_authority_set_len: 4,
        next_authority_set_root: validator_root,
        random_seed: [0x66u8; 32],
        digest_hash,
    };
    let leaf_hash = hash_leaf(&leaf);
    let commitment = VerifierCommitment {
        mmr_root: leaf_hash,
        block_number: 1,
        validator_set_id: 1,
    };
    let commitment_hash = hash_commitment(&commitment);
    let msg = Message::parse(&commitment_hash);

    let mut signatures = Vec::new();
    let mut positions = Vec::new();
    let mut public_keys = Vec::new();
    let mut public_key_merkle_proofs = Vec::new();
    for (idx, sk) in validator_sks.iter().take(3).enumerate() {
        let (sig, recid) = sign(&msg, sk);
        let mut sig65 = Vec::with_capacity(65);
        sig65.extend_from_slice(&sig.serialize());
        sig65.push(recid.serialize());
        signatures.push(sig65);
        positions.push(idx as u64);
        public_keys.push(validator_addrs[idx]);
        public_key_merkle_proofs.push(merkle_proof(&validator_layers, idx));
    }

    Ok(SyntheticCommitment {
        commitment,
        validator_proof: VerifierValidatorProof {
            signatures,
            positions,
            public_keys,
            public_key_merkle_proofs,
        },
        leaf,
        mmr_proof: VerifierMmrProof {
            leaf_index: 0,
            leaf_count: 1,
            items: vec![],
        },
        digest_scale,
    })
}

fn synthetic_validator_set_root() -> Result<[u8; 32]> {
    let validator_sks = [
        SecretKey::parse(&[1u8; 32]).context("invalid synthetic sk 1")?,
        SecretKey::parse(&[2u8; 32]).context("invalid synthetic sk 2")?,
        SecretKey::parse(&[3u8; 32]).context("invalid synthetic sk 3")?,
        SecretKey::parse(&[4u8; 32]).context("invalid synthetic sk 4")?,
    ];
    let leaves = validator_sks
        .iter()
        .map(|sk| {
            let (addr, _) = eth_address_from_secret(sk);
            keccak256(&addr)
        })
        .collect::<Vec<_>>();
    merkle_layers(leaves)
        .last()
        .and_then(|layer| layer.first())
        .copied()
        .ok_or_else(|| anyhow!("empty synthetic validator merkle tree"))
}

fn parse_verifier_bootstrap_input(args: &BootstrapArgs) -> Result<Option<VerifierBootstrapInput>> {
    let provided = [
        args.latest_beefy_block.is_some(),
        args.current_validator_set_id.is_some(),
        args.current_validator_set_len.is_some(),
        args.current_validator_set_root_hex.is_some(),
        args.next_validator_set_id.is_some(),
        args.next_validator_set_len.is_some(),
        args.next_validator_set_root_hex.is_some(),
    ];
    if provided.iter().all(|flag| !flag) {
        return Ok(None);
    }
    if provided.iter().any(|flag| !flag) {
        bail!(
            "verifier bootstrap inputs are partial; provide latest beefy block plus full current/next validator set values"
        );
    }

    Ok(Some(VerifierBootstrapInput {
        latest_beefy_block: args.latest_beefy_block.expect("checked is_some"),
        current_validator_set: VerifierValidatorSet {
            id: args.current_validator_set_id.expect("checked is_some"),
            len: args.current_validator_set_len.expect("checked is_some"),
            root: parse_hex32(
                args.current_validator_set_root_hex
                    .as_deref()
                    .expect("checked is_some"),
                "current validator set root",
            )?,
        },
        next_validator_set: VerifierValidatorSet {
            id: args.next_validator_set_id.expect("checked is_some"),
            len: args.next_validator_set_len.expect("checked is_some"),
            root: parse_hex32(
                args.next_validator_set_root_hex
                    .as_deref()
                    .expect("checked is_some"),
                "next validator set root",
            )?,
        },
    }))
}

fn parse_pubkey(raw: &str, field: &str) -> Result<Pubkey> {
    raw.parse()
        .with_context(|| format!("failed to parse {field}: {raw}"))
}

fn parse_hex32(raw: &str, field: &str) -> Result<[u8; 32]> {
    let stripped = raw.strip_prefix("0x").unwrap_or(raw);
    let bytes =
        hex::decode(stripped).with_context(|| format!("failed to decode {field} as hex: {raw}"))?;
    if bytes.len() != 32 {
        bail!("{field} must decode to 32 bytes, got {}", bytes.len());
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn expand_home(path: &str) -> Result<std::path::PathBuf> {
    if let Some(rest) = path.strip_prefix("~/") {
        let home = std::env::var_os("HOME").ok_or_else(|| anyhow!("HOME is not set"))?;
        return Ok(std::path::PathBuf::from(home).join(rest));
    }
    Ok(std::path::PathBuf::from(path))
}

fn config_pda(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[SEED_PREFIX, SEED_CONFIG], program_id)
}

fn verifier_config_pda(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[SEED_PREFIX, SEED_VERIFIER, SEED_CONFIG], program_id)
}

fn token_config_pda(program_id: &Pubkey, sora_asset_id: &[u8; 32]) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[SEED_PREFIX, SEED_TOKEN, sora_asset_id], program_id)
}

fn mint_pda(program_id: &Pubkey, sora_asset_id: &[u8; 32]) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[SEED_PREFIX, SEED_MINT, sora_asset_id], program_id)
}

fn burn_record_pda(program_id: &Pubkey, message_id: &[u8; 32]) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[SEED_PREFIX, SEED_BURN, message_id], program_id)
}

fn inbound_marker_pda(
    program_id: &Pubkey,
    source_domain: u32,
    message_id: &[u8; 32],
) -> (Pubkey, u8) {
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

fn keccak256(data: &[u8]) -> [u8; 32] {
    use tiny_keccak::{Hasher, Keccak};

    let mut k = Keccak::v256();
    k.update(data);
    let mut out = [0u8; 32];
    k.finalize(&mut out);
    out
}

fn eth_address_from_secret(sk: &SecretKey) -> ([u8; 20], [u8; 64]) {
    let pk = PublicKey::from_secret_key(sk);
    let uncompressed = pk.serialize();
    let mut pubkey64 = [0u8; 64];
    pubkey64.copy_from_slice(&uncompressed[1..65]);
    let hash = keccak256(&pubkey64);
    let mut addr = [0u8; 20];
    addr.copy_from_slice(&hash[12..32]);
    (addr, pubkey64)
}

fn merkle_layers(mut leaves: Vec<[u8; 32]>) -> Vec<Vec<[u8; 32]>> {
    let mut layers = vec![leaves.clone()];
    while leaves.len() > 1 {
        let mut next = Vec::with_capacity((leaves.len() + 1) / 2);
        let mut idx = 0usize;
        while idx < leaves.len() {
            let left = leaves[idx];
            if let Some(right) = leaves.get(idx + 1) {
                let mut combined = [0u8; 64];
                combined[..32].copy_from_slice(&left);
                combined[32..].copy_from_slice(right);
                next.push(keccak256(&combined));
            } else {
                next.push(left);
            }
            idx += 2;
        }
        layers.push(next.clone());
        leaves = next;
    }
    layers
}

fn merkle_proof(layers: &[Vec<[u8; 32]>], mut idx: usize) -> Vec<[u8; 32]> {
    let mut proof = Vec::new();
    for layer in layers.iter().take(layers.len().saturating_sub(1)) {
        let sibling = if idx % 2 == 1 { idx - 1 } else { idx + 1 };
        if sibling < layer.len() {
            proof.push(layer[sibling]);
        }
        idx /= 2;
    }
    proof
}

fn hash_commitment(commitment: &VerifierCommitment) -> [u8; 32] {
    let mut out = [0u8; 48];
    out[0] = 0x04;
    out[1] = b'm';
    out[2] = b'h';
    out[3] = 0x80;
    out[4..36].copy_from_slice(&commitment.mmr_root);
    out[36..40].copy_from_slice(&commitment.block_number.to_le_bytes());
    out[40..48].copy_from_slice(&commitment.validator_set_id.to_le_bytes());
    keccak256(&out)
}

fn hash_leaf(leaf: &VerifierMmrLeaf) -> [u8; 32] {
    let mut out = [0u8; 145];
    out[0] = leaf.version;
    out[1..5].copy_from_slice(&leaf.parent_number.to_le_bytes());
    out[5..37].copy_from_slice(&leaf.parent_hash);
    out[37..45].copy_from_slice(&leaf.next_authority_set_id.to_le_bytes());
    out[45..49].copy_from_slice(&leaf.next_authority_set_len.to_le_bytes());
    out[49..81].copy_from_slice(&leaf.next_authority_set_root);
    out[81..113].copy_from_slice(&leaf.random_seed);
    out[113..145].copy_from_slice(&leaf.digest_hash);
    keccak256(&out)
}
