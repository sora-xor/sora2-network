use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::program_pack::Pack;
use solana_program_test::{processor, ProgramTest};
use solana_sdk::{
    account::Account,
    instruction::{AccountMeta, Instruction, InstructionError},
    pubkey::Pubkey,
    rent::Rent,
    signature::{Keypair, Signer},
    system_instruction, system_program, sysvar,
    transaction::{Transaction, TransactionError},
    transport::TransportError,
};
use spl_token::state::{Account as TokenAccount, Mint as TokenMint};

use std::sync::OnceLock;
use tokio::sync::Mutex;

use sccp_sol::{
    burn_message_id, token_add_message_id, token_pause_message_id, token_resume_message_id,
    BurnPayloadV1, TokenAddPayloadV1, TokenControlPayloadV1, SCCP_DOMAIN_BSC, SCCP_DOMAIN_ETH,
    SCCP_DOMAIN_SOL, SCCP_DOMAIN_SORA, SCCP_DOMAIN_TON, SCCP_DOMAIN_TRON,
};
use sccp_sol_program::{
    process_instruction, BurnRecord, Config, InboundMarker, InboundStatus, SccpError,
    SccpInstruction, TokenConfig, TokenState,
};
use sccp_sol_verifier_program::{
    process_instruction as verifier_process_instruction, Commitment as VerifierCommitment,
    Config as VerifierConfig, MmrLeaf as VerifierMmrLeaf, MmrProof as VerifierMmrProof,
    SoraBurnProofV1, ValidatorProof as VerifierValidatorProof,
    ValidatorSet as VerifierValidatorSet, VerifierError, VerifierInstruction,
};

use libsecp256k1::{sign, Message, PublicKey, SecretKey};
use tiny_keccak::{Hasher, Keccak};

const SEED_PREFIX: &[u8] = b"sccp";
const SEED_VERIFIER: &[u8] = b"verifier";
const SEED_CONFIG: &[u8] = b"config";
const SEED_TOKEN: &[u8] = b"token";
const SEED_MINT: &[u8] = b"mint";
const SEED_BURN: &[u8] = b"burn";
const SEED_INBOUND: &[u8] = b"inbound";

// `solana-program-test` can be flaky under parallel test execution (shared global resources).
// Run these integration tests serially to keep CI/dev runs stable.
static PROGRAM_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

async fn program_test_lock() -> tokio::sync::MutexGuard<'static, ()> {
    PROGRAM_TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .await
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

fn expect_custom(err: TransportError, code: u32) {
    match err {
        TransportError::TransactionError(TransactionError::InstructionError(
            _,
            InstructionError::Custom(got),
        )) => assert_eq!(got, code),
        other => panic!("unexpected error: {other:?}"),
    }
}

fn keccak256(data: &[u8]) -> [u8; 32] {
    let mut k = Keccak::v256();
    k.update(data);
    let mut out = [0u8; 32];
    k.finalize(&mut out);
    out
}

fn ascii_fixed_32(input: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[..input.len()].copy_from_slice(input);
    out
}

fn sccp_digest_scale_for_message_ids(message_ids: &[[u8; 32]]) -> Vec<u8> {
    let mut digest_scale: Vec<u8> = Vec::with_capacity(1 + message_ids.len() * (6 + 32));
    digest_scale.push((message_ids.len() as u8) << 2);
    for msg_id in message_ids {
        digest_scale.extend_from_slice(&[0x00, 0x02, 0x50, 0x43, 0x43, 0x53]);
        digest_scale.extend_from_slice(msg_id);
    }
    digest_scale
}

const SECP256K1N: [u8; 32] = [
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfe,
    0xba, 0xae, 0xdc, 0xe6, 0xaf, 0x48, 0xa0, 0x3b, 0xbf, 0xd2, 0x5e, 0x8c, 0xd0, 0x36, 0x41, 0x41,
];

fn sub_be_32(a: [u8; 32], b: [u8; 32]) -> [u8; 32] {
    // Returns a - b for 256-bit big-endian numbers; assumes a >= b.
    let mut out = [0u8; 32];
    let mut borrow: i16 = 0;
    for i in (0..32).rev() {
        let ai = a[i] as i16;
        let bi = b[i] as i16;
        let mut v = ai - bi - borrow;
        if v < 0 {
            v += 256;
            borrow = 1;
        } else {
            borrow = 0;
        }
        out[i] = v as u8;
    }
    out
}

fn eth_address_from_secret(sk: &SecretKey) -> ([u8; 20], [u8; 64]) {
    let pk = PublicKey::from_secret_key(sk);
    let uncompressed = pk.serialize(); // 65 bytes, 0x04 || x || y
    let mut pubkey64 = [0u8; 64];
    pubkey64.copy_from_slice(&uncompressed[1..65]);

    let h = keccak256(&pubkey64);
    let mut addr = [0u8; 20];
    addr.copy_from_slice(&h[12..32]);
    (addr, pubkey64)
}

fn merkle_layers(mut leaves: Vec<[u8; 32]>) -> Vec<Vec<[u8; 32]>> {
    let mut layers = Vec::new();
    layers.push(leaves.clone());
    while leaves.len() > 1 {
        let mut next = Vec::with_capacity((leaves.len() + 1) / 2);
        let mut i = 0usize;
        while i < leaves.len() {
            let a = leaves[i];
            let b = if i + 1 < leaves.len() {
                Some(leaves[i + 1])
            } else {
                None
            };
            if let Some(b) = b {
                let mut combined = [0u8; 64];
                // Substrate `binary_merkle_tree`: ordered hashing (no sorting).
                combined[0..32].copy_from_slice(&a);
                combined[32..64].copy_from_slice(&b);
                next.push(keccak256(&combined));
            } else {
                next.push(a); // promote odd leaf
            }
            i += 2;
        }
        layers.push(next.clone());
        leaves = next;
    }
    layers
}

fn merkle_proof(layers: &[Vec<[u8; 32]>], mut idx: usize) -> Vec<[u8; 32]> {
    let mut proof = Vec::new();
    for level in 0..layers.len().saturating_sub(1) {
        let layer = &layers[level];
        let sib = if (idx % 2) == 1 { idx - 1 } else { idx + 1 };
        if sib < layer.len() {
            proof.push(layer[sib]);
        }
        idx /= 2;
    }
    proof
}

fn hash_commitment(c: &VerifierCommitment) -> [u8; 32] {
    let mut out = [0u8; 48];
    out[0] = 0x04; // compact(vec len=1)
    out[1] = b'm';
    out[2] = b'h';
    out[3] = 0x80; // compact(vec<u8> len=32)
    out[4..36].copy_from_slice(&c.mmr_root);
    out[36..40].copy_from_slice(&c.block_number.to_le_bytes());
    out[40..48].copy_from_slice(&c.validator_set_id.to_le_bytes());
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

#[tokio::test]
async fn solana_program_initialize_succeeds_with_prefunded_config_pda() {
    let test_lock = program_test_lock().await;
    let program_id = Pubkey::new_unique();
    let pt = ProgramTest::new(
        "sccp_sol_program",
        program_id,
        processor!(process_instruction),
    );
    let (mut banks_client, payer, _recent_blockhash) = pt.start().await;

    let (config, _config_bump) = config_pda(&program_id);
    let prefund_lamports = banks_client.get_rent().await.unwrap().minimum_balance(0);

    {
        let tx = Transaction::new_signed_with_payer(
            &[system_instruction::transfer(
                &payer.pubkey(),
                &config,
                prefund_lamports,
            )],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        banks_client.process_transaction(tx).await.unwrap();
    }

    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: SccpInstruction::Initialize {
                governor: payer.pubkey(),
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        banks_client.process_transaction(tx).await.unwrap();
    }

    let cfg_acc = banks_client.get_account(config).await.unwrap().unwrap();
    assert_eq!(cfg_acc.owner, program_id);
    let cfg = Config::try_from_slice(&cfg_acc.data).unwrap();
    assert_eq!(cfg.governor, payer.pubkey());

    drop(test_lock);
}

#[tokio::test]
async fn solana_verifier_initialize_succeeds_with_prefunded_config_pda() {
    let test_lock = program_test_lock().await;
    let verifier_program_id = Pubkey::new_unique();
    let pt = ProgramTest::new(
        "sccp_sol_verifier_program",
        verifier_program_id,
        processor!(verifier_process_instruction),
    );
    let (mut banks_client, payer, _recent_blockhash) = pt.start().await;

    let (verifier_config, _verifier_config_bump) = verifier_config_pda(&verifier_program_id);
    let prefund_lamports = banks_client.get_rent().await.unwrap().minimum_balance(0);
    let empty_vset = VerifierValidatorSet {
        id: 1,
        len: 1,
        root: [0x11u8; 32],
    };
    let next_vset = VerifierValidatorSet {
        id: 2,
        len: 1,
        root: [0x22u8; 32],
    };

    {
        let tx = Transaction::new_signed_with_payer(
            &[system_instruction::transfer(
                &payer.pubkey(),
                &verifier_config,
                prefund_lamports,
            )],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        banks_client.process_transaction(tx).await.unwrap();
    }

    {
        let ix = Instruction {
            program_id: verifier_program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(verifier_config, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: VerifierInstruction::Initialize {
                governor: payer.pubkey(),
                latest_beefy_block: 0,
                current_validator_set: empty_vset,
                next_validator_set: next_vset,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        banks_client.process_transaction(tx).await.unwrap();
    }

    let cfg_acc = banks_client
        .get_account(verifier_config)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(cfg_acc.owner, verifier_program_id);

    drop(test_lock);
}

#[tokio::test]
async fn solana_program_register_token_rejects_mint_without_bridge_authority() {
    let test_lock = program_test_lock().await;
    let program_id = Pubkey::new_unique();
    let mut pt = ProgramTest::new(
        "sccp_sol_program",
        program_id,
        processor!(process_instruction),
    );
    pt.add_program(
        "spl_token",
        spl_token::id(),
        processor!(spl_token::processor::Processor::process),
    );
    let (mut banks_client, payer, _recent_blockhash) = pt.start().await;

    let (config, _config_bump) = config_pda(&program_id);
    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: SccpInstruction::Initialize {
                governor: payer.pubkey(),
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        banks_client.process_transaction(tx).await.unwrap();
    }

    let mint = Keypair::new();
    let other_authority = Keypair::new();
    {
        let rent = banks_client.get_rent().await.unwrap();
        let lamports = rent.minimum_balance(TokenMint::LEN);
        let create_ix = system_instruction::create_account(
            &payer.pubkey(),
            &mint.pubkey(),
            lamports,
            TokenMint::LEN as u64,
            &spl_token::id(),
        );
        let init_ix = spl_token::instruction::initialize_mint(
            &spl_token::id(),
            &mint.pubkey(),
            &other_authority.pubkey(),
            None,
            6,
        )
        .unwrap();
        let tx = Transaction::new_signed_with_payer(
            &[create_ix, init_ix],
            Some(&payer.pubkey()),
            &[&payer, &mint],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        banks_client.process_transaction(tx).await.unwrap();
    }

    let sora_asset_id = [0x42u8; 32];
    let (token_cfg, _token_cfg_bump) = token_config_pda(&program_id, &sora_asset_id);
    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new(token_cfg, false),
                AccountMeta::new_readonly(mint.pubkey(), false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: SccpInstruction::RegisterToken {
                sora_asset_id,
                mint: mint.pubkey(),
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::MintAuthorityMismatch as u32);
    }

    drop(test_lock);
}

#[tokio::test]
async fn solana_program_register_token_rejects_mint_with_existing_supply() {
    let test_lock = program_test_lock().await;
    let program_id = Pubkey::new_unique();
    let mut pt = ProgramTest::new(
        "sccp_sol_program",
        program_id,
        processor!(process_instruction),
    );
    pt.add_program(
        "spl_token",
        spl_token::id(),
        processor!(spl_token::processor::Processor::process),
    );
    let (mut banks_client, payer, _recent_blockhash) = pt.start().await;

    let (config, _config_bump) = config_pda(&program_id);
    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: SccpInstruction::Initialize {
                governor: payer.pubkey(),
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        banks_client.process_transaction(tx).await.unwrap();
    }

    let mint = Keypair::new();
    let holder = Keypair::new();
    let holder_token = Keypair::new();
    {
        let rent = banks_client.get_rent().await.unwrap();
        let mint_lamports = rent.minimum_balance(TokenMint::LEN);
        let token_lamports = rent.minimum_balance(TokenAccount::LEN);
        let create_mint_ix = system_instruction::create_account(
            &payer.pubkey(),
            &mint.pubkey(),
            mint_lamports,
            TokenMint::LEN as u64,
            &spl_token::id(),
        );
        let init_mint_ix = spl_token::instruction::initialize_mint(
            &spl_token::id(),
            &mint.pubkey(),
            &payer.pubkey(),
            None,
            6,
        )
        .unwrap();
        let create_token_ix = system_instruction::create_account(
            &payer.pubkey(),
            &holder_token.pubkey(),
            token_lamports,
            TokenAccount::LEN as u64,
            &spl_token::id(),
        );
        let init_token_ix = spl_token::instruction::initialize_account(
            &spl_token::id(),
            &holder_token.pubkey(),
            &mint.pubkey(),
            &holder.pubkey(),
        )
        .unwrap();
        let mint_to_ix = spl_token::instruction::mint_to(
            &spl_token::id(),
            &mint.pubkey(),
            &holder_token.pubkey(),
            &payer.pubkey(),
            &[],
            1,
        )
        .unwrap();
        let handoff_ix = spl_token::instruction::set_authority(
            &spl_token::id(),
            &mint.pubkey(),
            Some(&config),
            spl_token::instruction::AuthorityType::MintTokens,
            &payer.pubkey(),
            &[],
        )
        .unwrap();
        let tx = Transaction::new_signed_with_payer(
            &[
                create_mint_ix,
                init_mint_ix,
                create_token_ix,
                init_token_ix,
                mint_to_ix,
                handoff_ix,
            ],
            Some(&payer.pubkey()),
            &[&payer, &mint, &holder_token],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        banks_client.process_transaction(tx).await.unwrap();
    }

    let sora_asset_id = [0x43u8; 32];
    let (token_cfg, _token_cfg_bump) = token_config_pda(&program_id, &sora_asset_id);
    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new(token_cfg, false),
                AccountMeta::new_readonly(mint.pubkey(), false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: SccpInstruction::RegisterToken {
                sora_asset_id,
                mint: mint.pubkey(),
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::NonZeroMintSupply as u32);
    }

    drop(test_lock);
}

#[tokio::test]
async fn solana_program_register_token_rejects_third_party_freeze_authority() {
    let test_lock = program_test_lock().await;
    let program_id = Pubkey::new_unique();
    let mut pt = ProgramTest::new(
        "sccp_sol_program",
        program_id,
        processor!(process_instruction),
    );
    pt.add_program(
        "spl_token",
        spl_token::id(),
        processor!(spl_token::processor::Processor::process),
    );
    let (mut banks_client, payer, _recent_blockhash) = pt.start().await;

    let (config, _config_bump) = config_pda(&program_id);
    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: SccpInstruction::Initialize {
                governor: payer.pubkey(),
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        banks_client.process_transaction(tx).await.unwrap();
    }

    let mint = Keypair::new();
    let other_freeze_authority = Keypair::new();
    {
        let rent = banks_client.get_rent().await.unwrap();
        let lamports = rent.minimum_balance(TokenMint::LEN);
        let create_ix = system_instruction::create_account(
            &payer.pubkey(),
            &mint.pubkey(),
            lamports,
            TokenMint::LEN as u64,
            &spl_token::id(),
        );
        let init_ix = spl_token::instruction::initialize_mint(
            &spl_token::id(),
            &mint.pubkey(),
            &payer.pubkey(),
            Some(&other_freeze_authority.pubkey()),
            6,
        )
        .unwrap();
        let handoff_ix = spl_token::instruction::set_authority(
            &spl_token::id(),
            &mint.pubkey(),
            Some(&config),
            spl_token::instruction::AuthorityType::MintTokens,
            &payer.pubkey(),
            &[],
        )
        .unwrap();
        let tx = Transaction::new_signed_with_payer(
            &[create_ix, init_ix, handoff_ix],
            Some(&payer.pubkey()),
            &[&payer, &mint],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        banks_client.process_transaction(tx).await.unwrap();
    }

    let sora_asset_id = [0x44u8; 32];
    let (token_cfg, _token_cfg_bump) = token_config_pda(&program_id, &sora_asset_id);
    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new(token_cfg, false),
                AccountMeta::new_readonly(mint.pubkey(), false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: SccpInstruction::RegisterToken {
                sora_asset_id,
                mint: mint.pubkey(),
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::FreezeAuthorityMismatch as u32);
    }

    drop(test_lock);
}

#[tokio::test]
async fn solana_program_burn_rejects_legacy_registration_without_bridge_mint_control() {
    let test_lock = program_test_lock().await;
    let program_id = Pubkey::new_unique();
    let sora_asset_id = [0x45u8; 32];
    let mint = Keypair::new();
    let (token_cfg, token_cfg_bump) = token_config_pda(&program_id, &sora_asset_id);

    let mut pt = ProgramTest::new(
        "sccp_sol_program",
        program_id,
        processor!(process_instruction),
    );
    pt.add_program(
        "spl_token",
        spl_token::id(),
        processor!(spl_token::processor::Processor::process),
    );
    pt.add_account(
        token_cfg,
        Account {
            lamports: Rent::default().minimum_balance(TokenConfig::LEN),
            data: TokenConfig {
                version: 1,
                bump: token_cfg_bump,
                sora_asset_id,
                mint: mint.pubkey(),
                state: TokenState::Active,
            }
            .try_to_vec()
            .unwrap(),
            owner: program_id,
            executable: false,
            rent_epoch: 0,
        },
    );
    let (mut banks_client, payer, _recent_blockhash) = pt.start().await;

    let (config, _config_bump) = config_pda(&program_id);
    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: SccpInstruction::Initialize {
                governor: payer.pubkey(),
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        banks_client.process_transaction(tx).await.unwrap();
    }

    let other_authority = Keypair::new();
    let user = Keypair::new();
    let user_token = Keypair::new();
    {
        let rent = banks_client.get_rent().await.unwrap();
        let mint_lamports = rent.minimum_balance(TokenMint::LEN);
        let token_lamports = rent.minimum_balance(TokenAccount::LEN);
        let create_mint_ix = system_instruction::create_account(
            &payer.pubkey(),
            &mint.pubkey(),
            mint_lamports,
            TokenMint::LEN as u64,
            &spl_token::id(),
        );
        let init_mint_ix = spl_token::instruction::initialize_mint(
            &spl_token::id(),
            &mint.pubkey(),
            &other_authority.pubkey(),
            None,
            6,
        )
        .unwrap();
        let create_token_ix = system_instruction::create_account(
            &payer.pubkey(),
            &user_token.pubkey(),
            token_lamports,
            TokenAccount::LEN as u64,
            &spl_token::id(),
        );
        let init_token_ix = spl_token::instruction::initialize_account(
            &spl_token::id(),
            &user_token.pubkey(),
            &mint.pubkey(),
            &user.pubkey(),
        )
        .unwrap();
        let mint_to_ix = spl_token::instruction::mint_to(
            &spl_token::id(),
            &mint.pubkey(),
            &user_token.pubkey(),
            &other_authority.pubkey(),
            &[],
            5,
        )
        .unwrap();
        let tx = Transaction::new_signed_with_payer(
            &[
                create_mint_ix,
                init_mint_ix,
                create_token_ix,
                init_token_ix,
                mint_to_ix,
            ],
            Some(&payer.pubkey()),
            &[&payer, &mint, &user_token, &other_authority],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        banks_client.process_transaction(tx).await.unwrap();
    }

    let mut recipient = [0u8; 32];
    recipient[12..32].copy_from_slice(&[0x11u8; 20]);
    let burn_payload = BurnPayloadV1 {
        version: 1,
        source_domain: SCCP_DOMAIN_SOL,
        dest_domain: SCCP_DOMAIN_ETH,
        nonce: 1,
        sora_asset_id,
        amount: 1,
        recipient,
    };
    let burn_message = burn_message_id(&burn_payload.encode_scale());
    let (burn_record, _burn_bump) = burn_record_pda(&program_id, &burn_message);

    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(user.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new(token_cfg, false),
                AccountMeta::new(user_token.pubkey(), false),
                AccountMeta::new_readonly(mint.pubkey(), false),
                AccountMeta::new(burn_record, false),
                AccountMeta::new_readonly(system_program::id(), false),
                AccountMeta::new_readonly(spl_token::id(), false),
            ],
            data: SccpInstruction::Burn {
                sora_asset_id,
                amount: 1,
                dest_domain: SCCP_DOMAIN_ETH,
                recipient,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer, &user],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::MintAuthorityMismatch as u32);
    }

    drop(test_lock);
}

#[tokio::test]
async fn solana_program_burn_rejects_wrong_token_config_version() {
    let test_lock = program_test_lock().await;
    let program_id = Pubkey::new_unique();
    let sora_asset_id = [0x46u8; 32];
    let mint = Keypair::new();
    let (token_cfg, token_cfg_bump) = token_config_pda(&program_id, &sora_asset_id);

    let mut pt = ProgramTest::new(
        "sccp_sol_program",
        program_id,
        processor!(process_instruction),
    );
    pt.add_program(
        "spl_token",
        spl_token::id(),
        processor!(spl_token::processor::Processor::process),
    );
    pt.add_account(
        token_cfg,
        Account {
            lamports: Rent::default().minimum_balance(TokenConfig::LEN),
            data: TokenConfig {
                version: 2,
                bump: token_cfg_bump,
                sora_asset_id,
                mint: mint.pubkey(),
                state: TokenState::Active,
            }
            .try_to_vec()
            .unwrap(),
            owner: program_id,
            executable: false,
            rent_epoch: 0,
        },
    );
    let (mut banks_client, payer, _recent_blockhash) = pt.start().await;

    let (config, _config_bump) = config_pda(&program_id);
    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: SccpInstruction::Initialize {
                governor: payer.pubkey(),
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        banks_client.process_transaction(tx).await.unwrap();
    }

    let user = Keypair::new();
    let user_token = Keypair::new();
    {
        let rent = banks_client.get_rent().await.unwrap();
        let mint_lamports = rent.minimum_balance(TokenMint::LEN);
        let token_lamports = rent.minimum_balance(TokenAccount::LEN);
        let create_mint_ix = system_instruction::create_account(
            &payer.pubkey(),
            &mint.pubkey(),
            mint_lamports,
            TokenMint::LEN as u64,
            &spl_token::id(),
        );
        let init_mint_ix = spl_token::instruction::initialize_mint(
            &spl_token::id(),
            &mint.pubkey(),
            &payer.pubkey(),
            None,
            6,
        )
        .unwrap();
        let create_token_ix = system_instruction::create_account(
            &payer.pubkey(),
            &user_token.pubkey(),
            token_lamports,
            TokenAccount::LEN as u64,
            &spl_token::id(),
        );
        let init_token_ix = spl_token::instruction::initialize_account(
            &spl_token::id(),
            &user_token.pubkey(),
            &mint.pubkey(),
            &user.pubkey(),
        )
        .unwrap();
        let mint_to_ix = spl_token::instruction::mint_to(
            &spl_token::id(),
            &mint.pubkey(),
            &user_token.pubkey(),
            &payer.pubkey(),
            &[],
            5,
        )
        .unwrap();
        let handoff_ix = spl_token::instruction::set_authority(
            &spl_token::id(),
            &mint.pubkey(),
            Some(&config),
            spl_token::instruction::AuthorityType::MintTokens,
            &payer.pubkey(),
            &[],
        )
        .unwrap();
        let tx = Transaction::new_signed_with_payer(
            &[
                create_mint_ix,
                init_mint_ix,
                create_token_ix,
                init_token_ix,
                mint_to_ix,
                handoff_ix,
            ],
            Some(&payer.pubkey()),
            &[&payer, &mint, &user_token],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        banks_client.process_transaction(tx).await.unwrap();
    }

    let mut recipient = [0u8; 32];
    recipient[12..32].copy_from_slice(&[0x22u8; 20]);
    let burn_payload = BurnPayloadV1 {
        version: 1,
        source_domain: SCCP_DOMAIN_SOL,
        dest_domain: SCCP_DOMAIN_ETH,
        nonce: 1,
        sora_asset_id,
        amount: 1,
        recipient,
    };
    let burn_message = burn_message_id(&burn_payload.encode_scale());
    let (burn_record, _burn_bump) = burn_record_pda(&program_id, &burn_message);

    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(user.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new(token_cfg, false),
                AccountMeta::new(user_token.pubkey(), false),
                AccountMeta::new_readonly(mint.pubkey(), false),
                AccountMeta::new(burn_record, false),
                AccountMeta::new_readonly(system_program::id(), false),
                AccountMeta::new_readonly(spl_token::id(), false),
            ],
            data: SccpInstruction::Burn {
                sora_asset_id,
                amount: 1,
                dest_domain: SCCP_DOMAIN_ETH,
                recipient,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer, &user],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::InvalidAccountSize as u32);
    }

    drop(test_lock);
}

#[tokio::test]
async fn solana_program_clear_invalidated_rejects_wrong_marker_version() {
    let test_lock = program_test_lock().await;
    let program_id = Pubkey::new_unique();
    let source_domain = SCCP_DOMAIN_ETH;
    let message_id = [0x47u8; 32];
    let (marker, marker_bump) = inbound_marker_pda(&program_id, source_domain, &message_id);

    let mut pt = ProgramTest::new(
        "sccp_sol_program",
        program_id,
        processor!(process_instruction),
    );
    pt.add_account(
        marker,
        Account {
            lamports: Rent::default().minimum_balance(InboundMarker::LEN),
            data: InboundMarker {
                version: 2,
                bump: marker_bump,
                status: InboundStatus::Invalidated,
            }
            .try_to_vec()
            .unwrap(),
            owner: program_id,
            executable: false,
            rent_epoch: 0,
        },
    );
    let (mut banks_client, payer, _recent_blockhash) = pt.start().await;

    let (config, _config_bump) = config_pda(&program_id);
    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: SccpInstruction::Initialize {
                governor: payer.pubkey(),
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        banks_client.process_transaction(tx).await.unwrap();
    }

    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new(marker, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: SccpInstruction::ClearInvalidatedInboundMessage {
                source_domain,
                message_id,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::AdminPathDisabled as u32);
    }

    drop(test_lock);
}

#[tokio::test]
async fn solana_verifier_rejects_duplicate_validator_keys() {
    let test_lock = program_test_lock().await;
    let verifier_program_id = Pubkey::new_unique();
    let pt = ProgramTest::new(
        "sccp_sol_verifier_program",
        verifier_program_id,
        processor!(verifier_process_instruction),
    );
    let (mut banks_client, payer, _recent_blockhash) = pt.start().await;

    let (verifier_config, _verifier_config_bump) = verifier_config_pda(&verifier_program_id);

    // Initialize the verifier light client.
    let validator_sks: Vec<SecretKey> = vec![
        SecretKey::parse(&[1u8; 32]).unwrap(),
        SecretKey::parse(&[2u8; 32]).unwrap(),
        SecretKey::parse(&[3u8; 32]).unwrap(),
        SecretKey::parse(&[4u8; 32]).unwrap(),
    ];
    let mut validator_addrs: Vec<[u8; 20]> = Vec::new();
    let mut validator_leaf_hashes: Vec<[u8; 32]> = Vec::new();
    for sk in validator_sks.iter() {
        let (addr, _pk64) = eth_address_from_secret(sk);
        validator_addrs.push(addr);
        validator_leaf_hashes.push(keccak256(&addr)); // leaf hash = keccak(address20)
    }
    let validator_layers = merkle_layers(validator_leaf_hashes);
    let vset_root = validator_layers.last().unwrap()[0];
    let current_vset = VerifierValidatorSet {
        id: 1,
        len: 4,
        root: vset_root,
    };
    let next_vset = VerifierValidatorSet {
        id: 2,
        len: 4,
        root: vset_root,
    };
    {
        let ix = Instruction {
            program_id: verifier_program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(verifier_config, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: VerifierInstruction::Initialize {
                governor: payer.pubkey(),
                latest_beefy_block: 0,
                current_validator_set: current_vset,
                next_validator_set: next_vset,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        banks_client.process_transaction(tx).await.unwrap();
    }

    // Build a minimal commitment where MMR root == leaf hash (single-leaf proof).
    let leaf = VerifierMmrLeaf {
        version: 0,
        parent_number: 1,
        parent_hash: [0x55u8; 32],
        next_authority_set_id: next_vset.id,
        next_authority_set_len: next_vset.len,
        next_authority_set_root: vset_root,
        random_seed: [0x66u8; 32],
        digest_hash: [0x77u8; 32],
    };
    let leaf_hash = hash_leaf(&leaf);
    let commitment = VerifierCommitment {
        mmr_root: leaf_hash,
        block_number: 1,
        validator_set_id: current_vset.id,
    };
    let commitment_hash = hash_commitment(&commitment);
    let msg = Message::parse(&commitment_hash);

    // Duplicate validator key should be rejected even when positions are unique.
    let (sig, recid) = sign(&msg, &validator_sks[0]);
    let mut sig65 = Vec::with_capacity(65);
    sig65.extend_from_slice(&sig.serialize());
    sig65.push(recid.serialize());

    let validator_proof = VerifierValidatorProof {
        signatures: vec![sig65.clone(), sig65.clone(), sig65],
        positions: vec![0, 1, 2],
        public_keys: vec![validator_addrs[0], validator_addrs[0], validator_addrs[0]],
        public_key_merkle_proofs: vec![
            merkle_proof(&validator_layers, 0),
            merkle_proof(&validator_layers, 0),
            merkle_proof(&validator_layers, 0),
        ],
    };
    let mmr_proof = VerifierMmrProof {
        leaf_index: 0,
        leaf_count: 1,
        items: vec![],
    };
    {
        let ix = Instruction {
            program_id: verifier_program_id,
            accounts: vec![AccountMeta::new(verifier_config, false)],
            data: VerifierInstruction::SubmitSignatureCommitment {
                commitment,
                validator_proof,
                latest_mmr_leaf: leaf,
                proof: mmr_proof,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), VerifierError::InvalidValidatorProof as u32);
    }

    drop(test_lock);
}

#[tokio::test]
async fn solana_verifier_rejects_insufficient_signatures() {
    let test_lock = program_test_lock().await;
    let verifier_program_id = Pubkey::new_unique();
    let pt = ProgramTest::new(
        "sccp_sol_verifier_program",
        verifier_program_id,
        processor!(verifier_process_instruction),
    );
    let (mut banks_client, payer, _recent_blockhash) = pt.start().await;

    let (verifier_config, _verifier_config_bump) = verifier_config_pda(&verifier_program_id);

    // Initialize the verifier light client.
    let validator_sks: Vec<SecretKey> = vec![
        SecretKey::parse(&[1u8; 32]).unwrap(),
        SecretKey::parse(&[2u8; 32]).unwrap(),
        SecretKey::parse(&[3u8; 32]).unwrap(),
        SecretKey::parse(&[4u8; 32]).unwrap(),
    ];
    let mut validator_addrs: Vec<[u8; 20]> = Vec::new();
    let mut validator_leaf_hashes: Vec<[u8; 32]> = Vec::new();
    for sk in validator_sks.iter() {
        let (addr, _pk64) = eth_address_from_secret(sk);
        validator_addrs.push(addr);
        validator_leaf_hashes.push(keccak256(&addr)); // leaf hash = keccak(address20)
    }
    let validator_layers = merkle_layers(validator_leaf_hashes);
    let vset_root = validator_layers.last().unwrap()[0];
    let current_vset = VerifierValidatorSet {
        id: 1,
        len: 4,
        root: vset_root,
    };
    let next_vset = VerifierValidatorSet {
        id: 2,
        len: 4,
        root: vset_root,
    };
    {
        let ix = Instruction {
            program_id: verifier_program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(verifier_config, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: VerifierInstruction::Initialize {
                governor: payer.pubkey(),
                latest_beefy_block: 0,
                current_validator_set: current_vset,
                next_validator_set: next_vset,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        banks_client.process_transaction(tx).await.unwrap();
    }

    // Build a minimal commitment where MMR root == leaf hash (single-leaf proof).
    let leaf = VerifierMmrLeaf {
        version: 0,
        parent_number: 1,
        parent_hash: [0x55u8; 32],
        next_authority_set_id: next_vset.id,
        next_authority_set_len: next_vset.len,
        next_authority_set_root: vset_root,
        random_seed: [0x66u8; 32],
        digest_hash: [0x77u8; 32],
    };
    let leaf_hash = hash_leaf(&leaf);
    let commitment = VerifierCommitment {
        mmr_root: leaf_hash,
        block_number: 1,
        validator_set_id: current_vset.id,
    };
    let commitment_hash = hash_commitment(&commitment);
    let msg = Message::parse(&commitment_hash);

    // Threshold for 4 validators is 3; submit only 2 signatures.
    let mut sigs: Vec<Vec<u8>> = Vec::new();
    let mut positions: Vec<u64> = Vec::new();
    let mut pub_keys: Vec<[u8; 20]> = Vec::new();
    let mut merkle_proofs: Vec<Vec<[u8; 32]>> = Vec::new();
    for i in 0..2 {
        let (sig, recid) = sign(&msg, &validator_sks[i]);
        let mut sig65 = Vec::with_capacity(65);
        sig65.extend_from_slice(&sig.serialize());
        sig65.push(recid.serialize());
        sigs.push(sig65);
        positions.push(i as u64);
        pub_keys.push(validator_addrs[i]);
        merkle_proofs.push(merkle_proof(&validator_layers, i));
    }
    let validator_proof = VerifierValidatorProof {
        signatures: sigs,
        positions,
        public_keys: pub_keys,
        public_key_merkle_proofs: merkle_proofs,
    };
    let mmr_proof = VerifierMmrProof {
        leaf_index: 0,
        leaf_count: 1,
        items: vec![],
    };
    {
        let ix = Instruction {
            program_id: verifier_program_id,
            accounts: vec![AccountMeta::new(verifier_config, false)],
            data: VerifierInstruction::SubmitSignatureCommitment {
                commitment,
                validator_proof,
                latest_mmr_leaf: leaf,
                proof: mmr_proof,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(
            err.into(),
            VerifierError::NotEnoughValidatorSignatures as u32,
        );
    }

    drop(test_lock);
}

#[tokio::test]
async fn solana_verifier_initialize_rejects_zero_length_validator_sets() {
    let test_lock = program_test_lock().await;
    let verifier_program_id = Pubkey::new_unique();
    let pt = ProgramTest::new(
        "sccp_sol_verifier_program",
        verifier_program_id,
        processor!(verifier_process_instruction),
    );
    let (mut banks_client, payer, _recent_blockhash) = pt.start().await;

    let (verifier_config, _verifier_config_bump) = verifier_config_pda(&verifier_program_id);

    let zero_vset = VerifierValidatorSet {
        id: 1,
        len: 0,
        root: [0u8; 32],
    };
    let zero_next = VerifierValidatorSet {
        id: 2,
        len: 0,
        root: [0u8; 32],
    };
    {
        let ix = Instruction {
            program_id: verifier_program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(verifier_config, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: VerifierInstruction::Initialize {
                governor: payer.pubkey(),
                latest_beefy_block: 0,
                current_validator_set: zero_vset,
                next_validator_set: zero_next,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), VerifierError::InvalidValidatorProof as u32);
    }

    assert!(
        banks_client
            .get_account(verifier_config)
            .await
            .unwrap()
            .is_none(),
        "failed initialize must not create verifier config account"
    );

    drop(test_lock);
}

#[tokio::test]
async fn solana_verifier_rejects_zero_length_validator_set_rotation() {
    let test_lock = program_test_lock().await;
    let verifier_program_id = Pubkey::new_unique();
    let pt = ProgramTest::new(
        "sccp_sol_verifier_program",
        verifier_program_id,
        processor!(verifier_process_instruction),
    );
    let (mut banks_client, payer, _recent_blockhash) = pt.start().await;

    let (verifier_config, _verifier_config_bump) = verifier_config_pda(&verifier_program_id);

    let validator_sks: Vec<SecretKey> = vec![
        SecretKey::parse(&[1u8; 32]).unwrap(),
        SecretKey::parse(&[2u8; 32]).unwrap(),
        SecretKey::parse(&[3u8; 32]).unwrap(),
        SecretKey::parse(&[4u8; 32]).unwrap(),
    ];
    let mut validator_addrs: Vec<[u8; 20]> = Vec::new();
    let mut validator_leaf_hashes: Vec<[u8; 32]> = Vec::new();
    for sk in validator_sks.iter() {
        let (addr, _pk64) = eth_address_from_secret(sk);
        validator_addrs.push(addr);
        validator_leaf_hashes.push(keccak256(&addr));
    }
    let validator_layers = merkle_layers(validator_leaf_hashes);
    let vset_root = validator_layers.last().unwrap()[0];
    let current_vset = VerifierValidatorSet {
        id: 1,
        len: 4,
        root: vset_root,
    };
    let next_vset = VerifierValidatorSet {
        id: 2,
        len: 4,
        root: vset_root,
    };
    {
        let ix = Instruction {
            program_id: verifier_program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(verifier_config, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: VerifierInstruction::Initialize {
                governor: payer.pubkey(),
                latest_beefy_block: 0,
                current_validator_set: current_vset,
                next_validator_set: next_vset,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        banks_client.process_transaction(tx).await.unwrap();
    }

    let bad_leaf = VerifierMmrLeaf {
        version: 0,
        parent_number: 1,
        parent_hash: [0x55u8; 32],
        next_authority_set_id: next_vset.id + 1,
        next_authority_set_len: 0,
        next_authority_set_root: [0x88u8; 32],
        random_seed: [0x66u8; 32],
        digest_hash: [0x77u8; 32],
    };
    let bad_leaf_hash = hash_leaf(&bad_leaf);
    let commitment = VerifierCommitment {
        mmr_root: bad_leaf_hash,
        block_number: 1,
        validator_set_id: current_vset.id,
    };
    let commitment_hash = hash_commitment(&commitment);
    let msg = Message::parse(&commitment_hash);

    let mut sigs: Vec<Vec<u8>> = Vec::new();
    let mut positions: Vec<u64> = Vec::new();
    let mut pub_keys: Vec<[u8; 20]> = Vec::new();
    let mut merkle_proofs: Vec<Vec<[u8; 32]>> = Vec::new();
    for i in 0..3 {
        let (sig, recid) = sign(&msg, &validator_sks[i]);
        let mut sig65 = Vec::with_capacity(65);
        sig65.extend_from_slice(&sig.serialize());
        sig65.push(recid.serialize());
        sigs.push(sig65);
        positions.push(i as u64);
        pub_keys.push(validator_addrs[i]);
        merkle_proofs.push(merkle_proof(&validator_layers, i));
    }
    let validator_proof = VerifierValidatorProof {
        signatures: sigs,
        positions,
        public_keys: pub_keys,
        public_key_merkle_proofs: merkle_proofs,
    };
    let mmr_proof = VerifierMmrProof {
        leaf_index: 0,
        leaf_count: 1,
        items: vec![],
    };

    {
        let ix = Instruction {
            program_id: verifier_program_id,
            accounts: vec![AccountMeta::new(verifier_config, false)],
            data: VerifierInstruction::SubmitSignatureCommitment {
                commitment,
                validator_proof,
                latest_mmr_leaf: bad_leaf,
                proof: mmr_proof,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), VerifierError::InvalidValidatorProof as u32);
    }

    let cfg_acc = banks_client
        .get_account(verifier_config)
        .await
        .unwrap()
        .expect("verifier config should remain initialized");
    let cfg = VerifierConfig::try_from_slice(&cfg_acc.data).unwrap();
    assert_eq!(cfg.latest_beefy_block, 0);
    assert_eq!(cfg.current_validator_set, current_vset);
    assert_eq!(cfg.next_validator_set, next_vset);

    drop(test_lock);
}

#[tokio::test]
async fn solana_verifier_rejects_unsupported_source_domain_in_burn_proof_path() {
    let test_lock = program_test_lock().await;
    let verifier_program_id = Pubkey::new_unique();
    let pt = ProgramTest::new(
        "sccp_sol_verifier_program",
        verifier_program_id,
        processor!(verifier_process_instruction),
    );
    let (mut banks_client, payer, _recent_blockhash) = pt.start().await;

    let (verifier_config, _verifier_config_bump) = verifier_config_pda(&verifier_program_id);

    // Minimal initialize for verifier config.
    {
        let empty_set = VerifierValidatorSet {
            id: 1,
            len: 1,
            root: [0u8; 32],
        };
        let next_set = VerifierValidatorSet {
            id: 2,
            len: 1,
            root: [0u8; 32],
        };
        let ix = Instruction {
            program_id: verifier_program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(verifier_config, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: VerifierInstruction::Initialize {
                governor: payer.pubkey(),
                latest_beefy_block: 0,
                current_validator_set: empty_set,
                next_validator_set: next_set,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        banks_client.process_transaction(tx).await.unwrap();
    }

    // Build VerifyBurnProof raw payload:
    // [1-byte tag=1] [u32 source_domain] [message_id] [97-byte payload] [proof_bytes...]
    let payload = BurnPayloadV1 {
        version: 1,
        source_domain: 99, // unsupported domain
        dest_domain: SCCP_DOMAIN_SOL,
        nonce: 1,
        sora_asset_id: [0x11u8; 32],
        amount: 1u128,
        recipient: [0x22u8; 32],
    };
    let payload_bytes = payload.encode_scale();
    let message_id = burn_message_id(&payload_bytes);

    let mut data = Vec::with_capacity(1 + 4 + 32 + BurnPayloadV1::ENCODED_LEN);
    data.push(1u8);
    data.extend_from_slice(&99u32.to_le_bytes());
    data.extend_from_slice(&message_id);
    data.extend_from_slice(&payload_bytes);

    let ix = Instruction {
        program_id: verifier_program_id,
        accounts: vec![AccountMeta::new(verifier_config, false)],
        data,
    };
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer],
        banks_client.get_latest_blockhash().await.unwrap(),
    );
    let err = banks_client.process_transaction(tx).await.unwrap_err();
    expect_custom(err.into(), VerifierError::SourceDomainUnsupported as u32);

    drop(test_lock);
}

#[tokio::test]
async fn solana_verifier_rejects_malleable_high_s_signatures() {
    let test_lock = program_test_lock().await;
    let verifier_program_id = Pubkey::new_unique();
    let pt = ProgramTest::new(
        "sccp_sol_verifier_program",
        verifier_program_id,
        processor!(verifier_process_instruction),
    );
    let (mut banks_client, payer, _recent_blockhash) = pt.start().await;

    let (verifier_config, _verifier_config_bump) = verifier_config_pda(&verifier_program_id);

    // Initialize verifier.
    let validator_sks: Vec<SecretKey> = vec![
        SecretKey::parse(&[1u8; 32]).unwrap(),
        SecretKey::parse(&[2u8; 32]).unwrap(),
        SecretKey::parse(&[3u8; 32]).unwrap(),
        SecretKey::parse(&[4u8; 32]).unwrap(),
    ];
    let mut validator_addrs: Vec<[u8; 20]> = Vec::new();
    let mut validator_leaf_hashes: Vec<[u8; 32]> = Vec::new();
    for sk in validator_sks.iter() {
        let (addr, _pk64) = eth_address_from_secret(sk);
        validator_addrs.push(addr);
        validator_leaf_hashes.push(keccak256(&addr));
    }
    let validator_layers = merkle_layers(validator_leaf_hashes);
    let vset_root = validator_layers.last().unwrap()[0];
    let current_vset = VerifierValidatorSet {
        id: 1,
        len: 4,
        root: vset_root,
    };
    let next_vset = VerifierValidatorSet {
        id: 2,
        len: 4,
        root: vset_root,
    };
    {
        let ix = Instruction {
            program_id: verifier_program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(verifier_config, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: VerifierInstruction::Initialize {
                governor: payer.pubkey(),
                latest_beefy_block: 0,
                current_validator_set: current_vset,
                next_validator_set: next_vset,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        banks_client.process_transaction(tx).await.unwrap();
    }

    // Build a minimal commitment where MMR root == leaf hash (single-leaf proof).
    let leaf = VerifierMmrLeaf {
        version: 0,
        parent_number: 1,
        parent_hash: [0x55u8; 32],
        next_authority_set_id: next_vset.id,
        next_authority_set_len: next_vset.len,
        next_authority_set_root: vset_root,
        random_seed: [0x66u8; 32],
        digest_hash: [0x77u8; 32],
    };
    let leaf_hash = hash_leaf(&leaf);
    let commitment = VerifierCommitment {
        mmr_root: leaf_hash,
        block_number: 1,
        validator_set_id: current_vset.id,
    };
    let commitment_hash = hash_commitment(&commitment);
    let msg = Message::parse(&commitment_hash);

    // Build one high-s malleable signature and two regular ones.
    let (sig0, recid0) = sign(&msg, &validator_sks[0]);
    let sig0raw = sig0.serialize();
    let mut s0 = [0u8; 32];
    s0.copy_from_slice(&sig0raw[32..64]);
    let high_s0 = sub_be_32(SECP256K1N, s0);
    let mut sig0_high = vec![0u8; 65];
    sig0_high[0..32].copy_from_slice(&sig0raw[0..32]);
    sig0_high[32..64].copy_from_slice(&high_s0);
    sig0_high[64] = recid0.serialize() ^ 1; // malleated recovery id

    let mut sigs: Vec<Vec<u8>> = vec![sig0_high];
    let mut positions: Vec<u64> = vec![0];
    let mut pub_keys: Vec<[u8; 20]> = vec![validator_addrs[0]];
    let mut merkle_proofs: Vec<Vec<[u8; 32]>> = vec![merkle_proof(&validator_layers, 0)];
    for i in 1..=2 {
        let (sig, recid) = sign(&msg, &validator_sks[i]);
        let mut sig65 = Vec::with_capacity(65);
        sig65.extend_from_slice(&sig.serialize());
        sig65.push(recid.serialize());
        sigs.push(sig65);
        positions.push(i as u64);
        pub_keys.push(validator_addrs[i]);
        merkle_proofs.push(merkle_proof(&validator_layers, i));
    }
    let validator_proof = VerifierValidatorProof {
        signatures: sigs,
        positions,
        public_keys: pub_keys,
        public_key_merkle_proofs: merkle_proofs,
    };
    let mmr_proof = VerifierMmrProof {
        leaf_index: 0,
        leaf_count: 1,
        items: vec![],
    };
    {
        let ix = Instruction {
            program_id: verifier_program_id,
            accounts: vec![AccountMeta::new(verifier_config, false)],
            data: VerifierInstruction::SubmitSignatureCommitment {
                commitment,
                validator_proof,
                latest_mmr_leaf: leaf,
                proof: mmr_proof,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), VerifierError::InvalidSignature as u32);
    }

    // Zero-r signature component must fail closed.
    {
        let mut sig0_zero_r = vec![0u8; 65];
        sig0_zero_r[32..64].copy_from_slice(&sig0raw[32..64]);
        sig0_zero_r[64] = recid0.serialize();

        let mut sig1 = Vec::with_capacity(65);
        let (sig1_raw, recid1) = sign(&msg, &validator_sks[1]);
        sig1.extend_from_slice(&sig1_raw.serialize());
        sig1.push(recid1.serialize());

        let mut sig2 = Vec::with_capacity(65);
        let (sig2_raw, recid2) = sign(&msg, &validator_sks[2]);
        sig2.extend_from_slice(&sig2_raw.serialize());
        sig2.push(recid2.serialize());

        let validator_proof = VerifierValidatorProof {
            signatures: vec![sig0_zero_r, sig1, sig2],
            positions: vec![0, 1, 2],
            public_keys: vec![validator_addrs[0], validator_addrs[1], validator_addrs[2]],
            public_key_merkle_proofs: vec![
                merkle_proof(&validator_layers, 0),
                merkle_proof(&validator_layers, 1),
                merkle_proof(&validator_layers, 2),
            ],
        };
        let ix = Instruction {
            program_id: verifier_program_id,
            accounts: vec![AccountMeta::new(verifier_config, false)],
            data: VerifierInstruction::SubmitSignatureCommitment {
                commitment: VerifierCommitment {
                    mmr_root: leaf_hash,
                    block_number: 1,
                    validator_set_id: current_vset.id,
                },
                validator_proof,
                latest_mmr_leaf: VerifierMmrLeaf {
                    version: 0,
                    parent_number: 1,
                    parent_hash: [0x55u8; 32],
                    next_authority_set_id: next_vset.id,
                    next_authority_set_len: next_vset.len,
                    next_authority_set_root: vset_root,
                    random_seed: [0x66u8; 32],
                    digest_hash: [0x77u8; 32],
                },
                proof: VerifierMmrProof {
                    leaf_index: 0,
                    leaf_count: 1,
                    items: vec![],
                },
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), VerifierError::InvalidSignature as u32);
    }

    // Zero-s signature component must fail closed.
    {
        let mut sig0_zero_s = vec![0u8; 65];
        sig0_zero_s[0..32].copy_from_slice(&sig0raw[0..32]);
        sig0_zero_s[64] = recid0.serialize();

        let mut sig1 = Vec::with_capacity(65);
        let (sig1_raw, recid1) = sign(&msg, &validator_sks[1]);
        sig1.extend_from_slice(&sig1_raw.serialize());
        sig1.push(recid1.serialize());

        let mut sig2 = Vec::with_capacity(65);
        let (sig2_raw, recid2) = sign(&msg, &validator_sks[2]);
        sig2.extend_from_slice(&sig2_raw.serialize());
        sig2.push(recid2.serialize());

        let validator_proof = VerifierValidatorProof {
            signatures: vec![sig0_zero_s, sig1, sig2],
            positions: vec![0, 1, 2],
            public_keys: vec![validator_addrs[0], validator_addrs[1], validator_addrs[2]],
            public_key_merkle_proofs: vec![
                merkle_proof(&validator_layers, 0),
                merkle_proof(&validator_layers, 1),
                merkle_proof(&validator_layers, 2),
            ],
        };
        let ix = Instruction {
            program_id: verifier_program_id,
            accounts: vec![AccountMeta::new(verifier_config, false)],
            data: VerifierInstruction::SubmitSignatureCommitment {
                commitment: VerifierCommitment {
                    mmr_root: leaf_hash,
                    block_number: 1,
                    validator_set_id: current_vset.id,
                },
                validator_proof,
                latest_mmr_leaf: VerifierMmrLeaf {
                    version: 0,
                    parent_number: 1,
                    parent_hash: [0x55u8; 32],
                    next_authority_set_id: next_vset.id,
                    next_authority_set_len: next_vset.len,
                    next_authority_set_root: vset_root,
                    random_seed: [0x66u8; 32],
                    digest_hash: [0x77u8; 32],
                },
                proof: VerifierMmrProof {
                    leaf_index: 0,
                    leaf_count: 1,
                    items: vec![],
                },
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), VerifierError::InvalidSignature as u32);
    }

    // Out-of-range recovery id must fail closed.
    {
        let mut sig0_bad_v = vec![0u8; 65];
        sig0_bad_v[0..64].copy_from_slice(&sig0raw);
        sig0_bad_v[64] = 31; // 31 - 27 = 4 -> invalid recovery id

        let mut sig1 = Vec::with_capacity(65);
        let (sig1_raw, recid1) = sign(&msg, &validator_sks[1]);
        sig1.extend_from_slice(&sig1_raw.serialize());
        sig1.push(recid1.serialize());

        let mut sig2 = Vec::with_capacity(65);
        let (sig2_raw, recid2) = sign(&msg, &validator_sks[2]);
        sig2.extend_from_slice(&sig2_raw.serialize());
        sig2.push(recid2.serialize());

        let validator_proof = VerifierValidatorProof {
            signatures: vec![sig0_bad_v, sig1, sig2],
            positions: vec![0, 1, 2],
            public_keys: vec![validator_addrs[0], validator_addrs[1], validator_addrs[2]],
            public_key_merkle_proofs: vec![
                merkle_proof(&validator_layers, 0),
                merkle_proof(&validator_layers, 1),
                merkle_proof(&validator_layers, 2),
            ],
        };
        let ix = Instruction {
            program_id: verifier_program_id,
            accounts: vec![AccountMeta::new(verifier_config, false)],
            data: VerifierInstruction::SubmitSignatureCommitment {
                commitment: VerifierCommitment {
                    mmr_root: leaf_hash,
                    block_number: 1,
                    validator_set_id: current_vset.id,
                },
                validator_proof,
                latest_mmr_leaf: VerifierMmrLeaf {
                    version: 0,
                    parent_number: 1,
                    parent_hash: [0x55u8; 32],
                    next_authority_set_id: next_vset.id,
                    next_authority_set_len: next_vset.len,
                    next_authority_set_root: vset_root,
                    random_seed: [0x66u8; 32],
                    digest_hash: [0x77u8; 32],
                },
                proof: VerifierMmrProof {
                    leaf_index: 0,
                    leaf_count: 1,
                    items: vec![],
                },
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), VerifierError::InvalidSignature as u32);
    }

    // Out-of-range validator positions must fail closed.
    {
        let mut signatures: Vec<Vec<u8>> = Vec::new();
        let mut positions: Vec<u64> = Vec::new();
        let mut public_keys: Vec<[u8; 20]> = Vec::new();
        let mut public_key_merkle_proofs: Vec<Vec<[u8; 32]>> = Vec::new();

        for i in 0..=2 {
            let (sig_raw, recid) = sign(&msg, &validator_sks[i]);
            let mut sig65 = Vec::with_capacity(65);
            sig65.extend_from_slice(&sig_raw.serialize());
            sig65.push(recid.serialize());
            signatures.push(sig65);
            positions.push(if i == 2 {
                current_vset.len as u64
            } else {
                i as u64
            });
            public_keys.push(validator_addrs[i]);
            public_key_merkle_proofs.push(merkle_proof(&validator_layers, i));
        }

        let validator_proof = VerifierValidatorProof {
            signatures,
            positions,
            public_keys,
            public_key_merkle_proofs,
        };

        let ix = Instruction {
            program_id: verifier_program_id,
            accounts: vec![AccountMeta::new(verifier_config, false)],
            data: VerifierInstruction::SubmitSignatureCommitment {
                commitment: VerifierCommitment {
                    mmr_root: leaf_hash,
                    block_number: 1,
                    validator_set_id: current_vset.id,
                },
                validator_proof,
                latest_mmr_leaf: VerifierMmrLeaf {
                    version: 0,
                    parent_number: 1,
                    parent_hash: [0x55u8; 32],
                    next_authority_set_id: next_vset.id,
                    next_authority_set_len: next_vset.len,
                    next_authority_set_root: vset_root,
                    random_seed: [0x66u8; 32],
                    digest_hash: [0x77u8; 32],
                },
                proof: VerifierMmrProof {
                    leaf_index: 0,
                    leaf_count: 1,
                    items: vec![],
                },
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), VerifierError::InvalidValidatorProof as u32);
    }

    // Validator proof vector length mismatches must fail closed.
    {
        let mut signatures: Vec<Vec<u8>> = Vec::new();
        let mut positions: Vec<u64> = Vec::new();
        let mut public_keys: Vec<[u8; 20]> = Vec::new();
        let mut public_key_merkle_proofs: Vec<Vec<[u8; 32]>> = Vec::new();

        for i in 0..=2 {
            let (sig_raw, recid) = sign(&msg, &validator_sks[i]);
            let mut sig65 = Vec::with_capacity(65);
            sig65.extend_from_slice(&sig_raw.serialize());
            sig65.push(recid.serialize());
            signatures.push(sig65);
            positions.push(i as u64);
            public_keys.push(validator_addrs[i]);
            public_key_merkle_proofs.push(merkle_proof(&validator_layers, i));
        }
        public_key_merkle_proofs.pop();

        let validator_proof = VerifierValidatorProof {
            signatures,
            positions,
            public_keys,
            public_key_merkle_proofs,
        };

        let ix = Instruction {
            program_id: verifier_program_id,
            accounts: vec![AccountMeta::new(verifier_config, false)],
            data: VerifierInstruction::SubmitSignatureCommitment {
                commitment: VerifierCommitment {
                    mmr_root: leaf_hash,
                    block_number: 1,
                    validator_set_id: current_vset.id,
                },
                validator_proof,
                latest_mmr_leaf: VerifierMmrLeaf {
                    version: 0,
                    parent_number: 1,
                    parent_hash: [0x55u8; 32],
                    next_authority_set_id: next_vset.id,
                    next_authority_set_len: next_vset.len,
                    next_authority_set_root: vset_root,
                    random_seed: [0x66u8; 32],
                    digest_hash: [0x77u8; 32],
                },
                proof: VerifierMmrProof {
                    leaf_index: 0,
                    leaf_count: 1,
                    items: vec![],
                },
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), VerifierError::InvalidValidatorProof as u32);
    }

    // Non-65-byte signatures must fail closed.
    {
        let mut signatures: Vec<Vec<u8>> = Vec::new();
        let mut positions: Vec<u64> = Vec::new();
        let mut public_keys: Vec<[u8; 20]> = Vec::new();
        let mut public_key_merkle_proofs: Vec<Vec<[u8; 32]>> = Vec::new();

        for i in 0..=2 {
            let (sig_raw, recid) = sign(&msg, &validator_sks[i]);
            let mut sig65 = Vec::with_capacity(65);
            sig65.extend_from_slice(&sig_raw.serialize());
            sig65.push(recid.serialize());
            if i == 0 {
                sig65.pop();
            }
            signatures.push(sig65);
            positions.push(i as u64);
            public_keys.push(validator_addrs[i]);
            public_key_merkle_proofs.push(merkle_proof(&validator_layers, i));
        }

        let validator_proof = VerifierValidatorProof {
            signatures,
            positions,
            public_keys,
            public_key_merkle_proofs,
        };

        let ix = Instruction {
            program_id: verifier_program_id,
            accounts: vec![AccountMeta::new(verifier_config, false)],
            data: VerifierInstruction::SubmitSignatureCommitment {
                commitment: VerifierCommitment {
                    mmr_root: leaf_hash,
                    block_number: 1,
                    validator_set_id: current_vset.id,
                },
                validator_proof,
                latest_mmr_leaf: VerifierMmrLeaf {
                    version: 0,
                    parent_number: 1,
                    parent_hash: [0x55u8; 32],
                    next_authority_set_id: next_vset.id,
                    next_authority_set_len: next_vset.len,
                    next_authority_set_root: vset_root,
                    random_seed: [0x66u8; 32],
                    digest_hash: [0x77u8; 32],
                },
                proof: VerifierMmrProof {
                    leaf_index: 0,
                    leaf_count: 1,
                    items: vec![],
                },
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), VerifierError::InvalidSignature as u32);
    }

    // Low-v signatures (0/1) should be accepted.
    {
        let mut signatures: Vec<Vec<u8>> = Vec::new();
        let mut positions: Vec<u64> = Vec::new();
        let mut public_keys: Vec<[u8; 20]> = Vec::new();
        let mut public_key_merkle_proofs: Vec<Vec<[u8; 32]>> = Vec::new();

        for i in 0..=2 {
            let (sig_raw, recid) = sign(&msg, &validator_sks[i]);
            let mut sig65 = Vec::with_capacity(65);
            sig65.extend_from_slice(&sig_raw.serialize());
            // `recid.serialize()` is low-v parity in {0,1}.
            sig65.push(recid.serialize());
            signatures.push(sig65);
            positions.push(i as u64);
            public_keys.push(validator_addrs[i]);
            public_key_merkle_proofs.push(merkle_proof(&validator_layers, i));
        }

        let validator_proof = VerifierValidatorProof {
            signatures,
            positions,
            public_keys,
            public_key_merkle_proofs,
        };

        let ix = Instruction {
            program_id: verifier_program_id,
            accounts: vec![AccountMeta::new(verifier_config, false)],
            data: VerifierInstruction::SubmitSignatureCommitment {
                commitment: VerifierCommitment {
                    mmr_root: leaf_hash,
                    block_number: 1,
                    validator_set_id: current_vset.id,
                },
                validator_proof,
                latest_mmr_leaf: VerifierMmrLeaf {
                    version: 0,
                    parent_number: 1,
                    parent_hash: [0x55u8; 32],
                    next_authority_set_id: next_vset.id,
                    next_authority_set_len: next_vset.len,
                    next_authority_set_root: vset_root,
                    random_seed: [0x66u8; 32],
                    digest_hash: [0x77u8; 32],
                },
                proof: VerifierMmrProof {
                    leaf_index: 0,
                    leaf_count: 1,
                    items: vec![],
                },
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        banks_client.process_transaction(tx).await.unwrap();
    }

    // Replay of the same commitment must fail as stale after successful import.
    {
        let mut signatures: Vec<Vec<u8>> = Vec::new();
        let mut positions: Vec<u64> = Vec::new();
        let mut public_keys: Vec<[u8; 20]> = Vec::new();
        let mut public_key_merkle_proofs: Vec<Vec<[u8; 32]>> = Vec::new();

        for i in 0..=2 {
            let (sig_raw, recid) = sign(&msg, &validator_sks[i]);
            let mut sig65 = Vec::with_capacity(65);
            sig65.extend_from_slice(&sig_raw.serialize());
            sig65.push(recid.serialize());
            signatures.push(sig65);
            positions.push(i as u64);
            public_keys.push(validator_addrs[i]);
            public_key_merkle_proofs.push(merkle_proof(&validator_layers, i));
        }

        let validator_proof = VerifierValidatorProof {
            signatures,
            positions,
            public_keys,
            public_key_merkle_proofs,
        };

        let ix = Instruction {
            program_id: verifier_program_id,
            accounts: vec![AccountMeta::new(verifier_config, false)],
            data: VerifierInstruction::SubmitSignatureCommitment {
                commitment: VerifierCommitment {
                    mmr_root: leaf_hash,
                    block_number: 1,
                    validator_set_id: current_vset.id,
                },
                validator_proof,
                latest_mmr_leaf: VerifierMmrLeaf {
                    version: 0,
                    parent_number: 1,
                    parent_hash: [0x55u8; 32],
                    next_authority_set_id: next_vset.id,
                    next_authority_set_len: next_vset.len,
                    next_authority_set_root: vset_root,
                    random_seed: [0x66u8; 32],
                    digest_hash: [0x77u8; 32],
                },
                proof: VerifierMmrProof {
                    leaf_index: 0,
                    leaf_count: 1,
                    items: vec![],
                },
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), VerifierError::PayloadBlocknumberTooOld as u32);
    }

    drop(test_lock);
}

#[tokio::test]
async fn solana_program_flow_burn_and_mint_with_admin_paths_disabled() {
    let test_lock = program_test_lock().await;
    let program_id = Pubkey::new_unique();
    let verifier_program_id = Pubkey::new_unique();

    let mut pt = ProgramTest::new(
        "sccp_sol_program",
        program_id,
        processor!(process_instruction),
    );
    pt.add_program(
        "spl_token",
        spl_token::id(),
        processor!(spl_token::processor::Processor::process),
    );
    pt.add_program(
        "sccp_sol_verifier_program",
        verifier_program_id,
        processor!(verifier_process_instruction),
    );

    let (mut banks_client, payer, recent_blockhash) = pt.start().await;

    let (config, _config_bump) = config_pda(&program_id);
    let (verifier_config, _verifier_config_bump) = verifier_config_pda(&verifier_program_id);

    // Initialize.
    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: SccpInstruction::Initialize {
                governor: payer.pubkey(),
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        banks_client.process_transaction(tx).await.unwrap();
    }

    // Bind verifier program once during bootstrap.
    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
            ],
            data: SccpInstruction::SetVerifierProgram {
                verifier_program: verifier_program_id,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            recent_blockhash,
        );
        banks_client.process_transaction(tx).await.unwrap();
    }

    // Initialize the verifier light client (bootstrap with a synthetic validator set).
    // In production, this must be bootstrapped from SORA chain state (validator sets + latest beefy block).
    let validator_sks: Vec<SecretKey> = vec![
        SecretKey::parse(&[1u8; 32]).unwrap(),
        SecretKey::parse(&[2u8; 32]).unwrap(),
        SecretKey::parse(&[3u8; 32]).unwrap(),
        SecretKey::parse(&[4u8; 32]).unwrap(),
    ];
    let mut validator_addrs: Vec<[u8; 20]> = Vec::new();
    let mut validator_leaf_hashes: Vec<[u8; 32]> = Vec::new();
    for sk in validator_sks.iter() {
        let (addr, _pk64) = eth_address_from_secret(sk);
        validator_addrs.push(addr);
        validator_leaf_hashes.push(keccak256(&addr)); // leaf hash = keccak(address20)
    }
    let validator_layers = merkle_layers(validator_leaf_hashes);
    let vset_root = validator_layers.last().unwrap()[0];
    let current_vset = VerifierValidatorSet {
        id: 1,
        len: 4,
        root: vset_root,
    };
    let next_vset = VerifierValidatorSet {
        id: 2,
        len: 4,
        root: vset_root,
    };
    {
        let ix = Instruction {
            program_id: verifier_program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(verifier_config, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: VerifierInstruction::Initialize {
                governor: payer.pubkey(),
                latest_beefy_block: 0,
                current_validator_set: current_vset,
                next_validator_set: next_vset,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            recent_blockhash,
        );
        banks_client.process_transaction(tx).await.unwrap();
    }

    // Add token from a SORA-finalized governance proof.
    let sora_asset_id = [0x11u8; 32];
    let (token_cfg, _token_cfg_bump) = token_config_pda(&program_id, &sora_asset_id);
    let (mint, _mint_bump) = mint_pda(&program_id, &sora_asset_id);
    let add_payload = TokenAddPayloadV1 {
        version: 1,
        target_domain: SCCP_DOMAIN_SOL,
        nonce: 1,
        sora_asset_id,
        decimals: 6,
        name: ascii_fixed_32(b"SCCP Solana"),
        symbol: ascii_fixed_32(b"sSOL"),
    };
    let add_payload_bytes = add_payload.encode_scale();
    let add_message_id = token_add_message_id(&add_payload_bytes);
    let (add_marker, _add_marker_bump) =
        inbound_marker_pda(&program_id, SCCP_DOMAIN_SORA, &add_message_id);
    let add_digest_scale = sccp_digest_scale_for_message_ids(&[add_message_id]);
    let add_digest_hash = keccak256(&add_digest_scale);
    let add_leaf = VerifierMmrLeaf {
        version: 0,
        parent_number: 1,
        parent_hash: [0x54u8; 32],
        next_authority_set_id: next_vset.id,
        next_authority_set_len: next_vset.len,
        next_authority_set_root: vset_root,
        random_seed: [0x65u8; 32],
        digest_hash: add_digest_hash,
    };
    let add_commitment = VerifierCommitment {
        mmr_root: hash_leaf(&add_leaf),
        block_number: 1,
        validator_set_id: current_vset.id,
    };
    let add_commitment_hash = hash_commitment(&add_commitment);
    let add_msg = Message::parse(&add_commitment_hash);
    let mut add_sigs: Vec<Vec<u8>> = Vec::new();
    let mut add_positions: Vec<u64> = Vec::new();
    let mut add_pub_keys: Vec<[u8; 20]> = Vec::new();
    let mut add_merkle_proofs: Vec<Vec<[u8; 32]>> = Vec::new();
    for i in 0..3 {
        let (sig, recid) = sign(&add_msg, &validator_sks[i]);
        let mut sig65 = Vec::with_capacity(65);
        sig65.extend_from_slice(&sig.serialize());
        sig65.push(recid.serialize());
        add_sigs.push(sig65);
        add_positions.push(i as u64);
        add_pub_keys.push(validator_addrs[i]);
        add_merkle_proofs.push(merkle_proof(&validator_layers, i));
    }
    let add_validator_proof = VerifierValidatorProof {
        signatures: add_sigs,
        positions: add_positions,
        public_keys: add_pub_keys,
        public_key_merkle_proofs: add_merkle_proofs,
    };
    let add_mmr_proof = VerifierMmrProof {
        leaf_index: 0,
        leaf_count: 1,
        items: vec![],
    };
    {
        let import_ix = Instruction {
            program_id: verifier_program_id,
            accounts: vec![AccountMeta::new(verifier_config, false)],
            data: VerifierInstruction::SubmitSignatureCommitment {
                commitment: add_commitment,
                validator_proof: add_validator_proof,
                latest_mmr_leaf: add_leaf,
                proof: add_mmr_proof.clone(),
            }
            .try_to_vec()
            .unwrap(),
        };
        let import_tx = Transaction::new_signed_with_payer(
            &[import_ix],
            Some(&payer.pubkey()),
            &[&payer],
            recent_blockhash,
        );
        banks_client.process_transaction(import_tx).await.unwrap();

        let add_proof_bytes = SoraBurnProofV1 {
            mmr_proof: add_mmr_proof,
            leaf: add_leaf,
            digest_scale: add_digest_scale,
        }
        .try_to_vec()
        .unwrap();
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new(token_cfg, false),
                AccountMeta::new(mint, false),
                AccountMeta::new(add_marker, false),
                AccountMeta::new_readonly(system_program::id(), false),
                AccountMeta::new_readonly(spl_token::id(), false),
                AccountMeta::new_readonly(sysvar::rent::id(), false),
                AccountMeta::new_readonly(verifier_program_id, false),
                AccountMeta::new_readonly(verifier_config, false),
            ],
            data: SccpInstruction::AddTokenFromProof {
                payload: add_payload_bytes.to_vec(),
                proof: add_proof_bytes,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            recent_blockhash,
        );
        banks_client.process_transaction(tx).await.unwrap();
    }

    // Create recipient wallet + token account.
    let alice = Keypair::new();
    let alice_token = Keypair::new();
    {
        let rent = banks_client.get_rent().await.unwrap();
        let lamports = rent.minimum_balance(TokenAccount::LEN);
        let create_ix = system_instruction::create_account(
            &payer.pubkey(),
            &alice_token.pubkey(),
            lamports,
            TokenAccount::LEN as u64,
            &spl_token::id(),
        );
        let init_ix = spl_token::instruction::initialize_account(
            &spl_token::id(),
            &alice_token.pubkey(),
            &mint,
            &alice.pubkey(),
        )
        .unwrap();
        let tx = Transaction::new_signed_with_payer(
            &[create_ix, init_ix],
            Some(&payer.pubkey()),
            &[&payer, &alice_token],
            recent_blockhash,
        );
        banks_client.process_transaction(tx).await.unwrap();
    }

    // MintFromProof (verified by on-chain SORA BEEFY+MMR light client):
    // - SORA -> SOL
    // - ETH -> SOL (attested/finalized by SORA and committed in its digest)
    let mint_amount: u64 = 100;
    let mint_amount_eth: u64 = 7;

    let inbound_payload_sora = BurnPayloadV1 {
        version: 1,
        source_domain: SCCP_DOMAIN_SORA,
        dest_domain: SCCP_DOMAIN_SOL,
        nonce: 1,
        sora_asset_id,
        amount: mint_amount as u128,
        recipient: alice.pubkey().to_bytes(),
    };
    let inbound_payload_sora_bytes = inbound_payload_sora.encode_scale();
    let inbound_message_id_sora = burn_message_id(&inbound_payload_sora_bytes);
    let (marker_sora, _marker_sora_bump) =
        inbound_marker_pda(&program_id, SCCP_DOMAIN_SORA, &inbound_message_id_sora);

    let inbound_payload_eth = BurnPayloadV1 {
        version: 1,
        source_domain: SCCP_DOMAIN_ETH,
        dest_domain: SCCP_DOMAIN_SOL,
        nonce: 2,
        sora_asset_id,
        amount: mint_amount_eth as u128,
        recipient: alice.pubkey().to_bytes(),
    };
    let inbound_payload_eth_bytes = inbound_payload_eth.encode_scale();
    let inbound_message_id_eth = burn_message_id(&inbound_payload_eth_bytes);
    let (marker_eth, _marker_eth_bump) =
        inbound_marker_pda(&program_id, SCCP_DOMAIN_ETH, &inbound_message_id_eth);

    // Import a synthetic "finalized" MMR root into verifier state, then construct the proof bytes.
    let digest_scale = sccp_digest_scale_for_message_ids(&[inbound_message_id_sora, inbound_message_id_eth]);
    let digest_hash = keccak256(&digest_scale);

    let leaf = VerifierMmrLeaf {
        version: 0,
        parent_number: 2,
        parent_hash: [0x55u8; 32],
        next_authority_set_id: next_vset.id,
        next_authority_set_len: next_vset.len,
        next_authority_set_root: vset_root,
        random_seed: [0x66u8; 32],
        digest_hash,
    };
    let leaf_hash = hash_leaf(&leaf);
    let commitment = VerifierCommitment {
        mmr_root: leaf_hash,
        block_number: 2,
        validator_set_id: current_vset.id,
    };
    let commitment_hash = hash_commitment(&commitment);
    let msg = Message::parse(&commitment_hash);

    let mut sigs: Vec<Vec<u8>> = Vec::new();
    let mut positions: Vec<u64> = Vec::new();
    let mut pub_keys: Vec<[u8; 20]> = Vec::new();
    let mut merkle_proofs: Vec<Vec<[u8; 32]>> = Vec::new();
    for i in 0..3 {
        let (sig, recid) = sign(&msg, &validator_sks[i]);
        let mut sig65 = Vec::with_capacity(65);
        sig65.extend_from_slice(&sig.serialize());
        sig65.push(recid.serialize());
        sigs.push(sig65);
        positions.push(i as u64);
        pub_keys.push(validator_addrs[i]);
        merkle_proofs.push(merkle_proof(&validator_layers, i));
    }
    let validator_proof = VerifierValidatorProof {
        signatures: sigs,
        positions,
        public_keys: pub_keys,
        public_key_merkle_proofs: merkle_proofs,
    };
    let mmr_proof = VerifierMmrProof {
        leaf_index: 0,
        leaf_count: 1,
        items: vec![],
    };
    {
        let ix = Instruction {
            program_id: verifier_program_id,
            accounts: vec![AccountMeta::new(verifier_config, false)],
            data: VerifierInstruction::SubmitSignatureCommitment {
                commitment,
                validator_proof: validator_proof.clone(),
                latest_mmr_leaf: leaf,
                proof: mmr_proof.clone(),
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            recent_blockhash,
        );
        banks_client.process_transaction(tx).await.unwrap();
    }

    let burn_proof_bytes = SoraBurnProofV1 {
        mmr_proof,
        leaf,
        digest_scale,
    }
    .try_to_vec()
    .unwrap();

    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new(token_cfg, false),
                AccountMeta::new(mint, false),
                AccountMeta::new(alice_token.pubkey(), false),
                AccountMeta::new(marker_sora, false),
                AccountMeta::new_readonly(system_program::id(), false),
                AccountMeta::new_readonly(spl_token::id(), false),
                AccountMeta::new_readonly(verifier_program_id, false),
                AccountMeta::new_readonly(verifier_config, false),
            ],
            data: SccpInstruction::MintFromProof {
                source_domain: SCCP_DOMAIN_SORA,
                payload: inbound_payload_sora_bytes.to_vec(),
                proof: burn_proof_bytes.clone(),
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            recent_blockhash,
        );
        banks_client.process_transaction(tx).await.unwrap();
    }

    // ETH -> SOL (attested by SORA).
    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new(token_cfg, false),
                AccountMeta::new(mint, false),
                AccountMeta::new(alice_token.pubkey(), false),
                AccountMeta::new(marker_eth, false),
                AccountMeta::new_readonly(system_program::id(), false),
                AccountMeta::new_readonly(spl_token::id(), false),
                AccountMeta::new_readonly(verifier_program_id, false),
                AccountMeta::new_readonly(verifier_config, false),
            ],
            data: SccpInstruction::MintFromProof {
                source_domain: SCCP_DOMAIN_ETH,
                payload: inbound_payload_eth_bytes.to_vec(),
                proof: burn_proof_bytes.clone(),
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        banks_client.process_transaction(tx).await.unwrap();
    }
    {
        let acct = banks_client
            .get_account(alice_token.pubkey())
            .await
            .unwrap()
            .unwrap();
        let ta = TokenAccount::unpack(&acct.data).unwrap();
        assert_eq!(ta.amount, mint_amount + mint_amount_eth);
        assert_eq!(ta.owner, alice.pubkey());
        assert_eq!(ta.mint, mint);
    }

    // Pause via governance proof, which must block both burn and mint until resume.
    let pause_payload = TokenControlPayloadV1 {
        version: 1,
        target_domain: SCCP_DOMAIN_SOL,
        nonce: 2,
        sora_asset_id,
    };
    let pause_payload_bytes = pause_payload.encode_scale();
    let pause_message_id = token_pause_message_id(&pause_payload_bytes);
    let (pause_marker, _pause_marker_bump) =
        inbound_marker_pda(&program_id, SCCP_DOMAIN_SORA, &pause_message_id);
    let pause_digest_scale = sccp_digest_scale_for_message_ids(&[pause_message_id]);
    let pause_digest_hash = keccak256(&pause_digest_scale);
    let pause_leaf = VerifierMmrLeaf {
        version: 0,
        parent_number: 5,
        parent_hash: [0x58u8; 32],
        next_authority_set_id: next_vset.id,
        next_authority_set_len: next_vset.len,
        next_authority_set_root: vset_root,
        random_seed: [0x69u8; 32],
        digest_hash: pause_digest_hash,
    };
    let pause_commitment = VerifierCommitment {
        mmr_root: hash_leaf(&pause_leaf),
        block_number: 5,
        validator_set_id: current_vset.id,
    };
    let pause_commitment_hash = hash_commitment(&pause_commitment);
    let pause_msg = Message::parse(&pause_commitment_hash);
    let mut pause_sigs: Vec<Vec<u8>> = Vec::new();
    let mut pause_positions: Vec<u64> = Vec::new();
    let mut pause_pub_keys: Vec<[u8; 20]> = Vec::new();
    let mut pause_merkle_proofs: Vec<Vec<[u8; 32]>> = Vec::new();
    for i in 0..3 {
        let (sig, recid) = sign(&pause_msg, &validator_sks[i]);
        let mut sig65 = Vec::with_capacity(65);
        sig65.extend_from_slice(&sig.serialize());
        sig65.push(recid.serialize());
        pause_sigs.push(sig65);
        pause_positions.push(i as u64);
        pause_pub_keys.push(validator_addrs[i]);
        pause_merkle_proofs.push(merkle_proof(&validator_layers, i));
    }
    let pause_validator_proof = VerifierValidatorProof {
        signatures: pause_sigs,
        positions: pause_positions,
        public_keys: pause_pub_keys,
        public_key_merkle_proofs: pause_merkle_proofs,
    };
    let pause_mmr_proof = VerifierMmrProof {
        leaf_index: 0,
        leaf_count: 1,
        items: vec![],
    };
    {
        let import_ix = Instruction {
            program_id: verifier_program_id,
            accounts: vec![AccountMeta::new(verifier_config, false)],
            data: VerifierInstruction::SubmitSignatureCommitment {
                commitment: pause_commitment,
                validator_proof: pause_validator_proof,
                latest_mmr_leaf: pause_leaf,
                proof: pause_mmr_proof.clone(),
            }
            .try_to_vec()
            .unwrap(),
        };
        let import_tx = Transaction::new_signed_with_payer(
            &[import_ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        banks_client.process_transaction(import_tx).await.unwrap();

        let pause_proof_bytes = SoraBurnProofV1 {
            mmr_proof: pause_mmr_proof,
            leaf: pause_leaf,
            digest_scale: pause_digest_scale,
        }
        .try_to_vec()
        .unwrap();
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new(token_cfg, false),
                AccountMeta::new(pause_marker, false),
                AccountMeta::new_readonly(verifier_program_id, false),
                AccountMeta::new_readonly(verifier_config, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: SccpInstruction::PauseTokenFromProof {
                payload: pause_payload_bytes.to_vec(),
                proof: pause_proof_bytes,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        banks_client.process_transaction(tx).await.unwrap();
    }

    // Burn must fail while paused.
    {
        let mut evm_recipient = [0u8; 32];
        evm_recipient[12..].copy_from_slice(&[0x44u8; 20]);
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(alice.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new(token_cfg, false),
                AccountMeta::new(alice_token.pubkey(), false),
                AccountMeta::new(mint, false),
                AccountMeta::new(Pubkey::new_unique(), false),
                AccountMeta::new_readonly(system_program::id(), false),
                AccountMeta::new_readonly(spl_token::id(), false),
            ],
            data: SccpInstruction::Burn {
                sora_asset_id,
                amount: 1,
                dest_domain: SCCP_DOMAIN_ETH,
                recipient: evm_recipient,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer, &alice],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::TokenNotActive as u32);
    }

    // A valid inbound proof must also fail closed while paused, without poisoning replay state.
    let paused_inbound_payload = BurnPayloadV1 {
        version: 1,
        source_domain: SCCP_DOMAIN_SORA,
        dest_domain: SCCP_DOMAIN_SOL,
        nonce: 12,
        sora_asset_id,
        amount: 1,
        recipient: alice.pubkey().to_bytes(),
    };
    let paused_inbound_payload_bytes = paused_inbound_payload.encode_scale();
    let paused_inbound_message_id = burn_message_id(&paused_inbound_payload_bytes);
    let (paused_inbound_marker, _paused_inbound_marker_bump) =
        inbound_marker_pda(&program_id, SCCP_DOMAIN_SORA, &paused_inbound_message_id);
    let paused_inbound_digest_scale = sccp_digest_scale_for_message_ids(&[paused_inbound_message_id]);
    let paused_inbound_digest_hash = keccak256(&paused_inbound_digest_scale);
    let paused_inbound_leaf = VerifierMmrLeaf {
        version: 0,
        parent_number: 6,
        parent_hash: [0x59u8; 32],
        next_authority_set_id: next_vset.id,
        next_authority_set_len: next_vset.len,
        next_authority_set_root: vset_root,
        random_seed: [0x6au8; 32],
        digest_hash: paused_inbound_digest_hash,
    };
    let paused_inbound_commitment = VerifierCommitment {
        mmr_root: hash_leaf(&paused_inbound_leaf),
        block_number: 6,
        validator_set_id: current_vset.id,
    };
    let paused_inbound_commitment_hash = hash_commitment(&paused_inbound_commitment);
    let paused_inbound_msg = Message::parse(&paused_inbound_commitment_hash);
    let mut paused_inbound_sigs: Vec<Vec<u8>> = Vec::new();
    let mut paused_inbound_positions: Vec<u64> = Vec::new();
    let mut paused_inbound_pub_keys: Vec<[u8; 20]> = Vec::new();
    let mut paused_inbound_merkle_proofs: Vec<Vec<[u8; 32]>> = Vec::new();
    for i in 0..3 {
        let (sig, recid) = sign(&paused_inbound_msg, &validator_sks[i]);
        let mut sig65 = Vec::with_capacity(65);
        sig65.extend_from_slice(&sig.serialize());
        sig65.push(recid.serialize());
        paused_inbound_sigs.push(sig65);
        paused_inbound_positions.push(i as u64);
        paused_inbound_pub_keys.push(validator_addrs[i]);
        paused_inbound_merkle_proofs.push(merkle_proof(&validator_layers, i));
    }
    let paused_inbound_validator_proof = VerifierValidatorProof {
        signatures: paused_inbound_sigs,
        positions: paused_inbound_positions,
        public_keys: paused_inbound_pub_keys,
        public_key_merkle_proofs: paused_inbound_merkle_proofs,
    };
    let paused_inbound_mmr_proof = VerifierMmrProof {
        leaf_index: 0,
        leaf_count: 1,
        items: vec![],
    };
    let paused_inbound_proof_bytes = {
        let import_ix = Instruction {
            program_id: verifier_program_id,
            accounts: vec![AccountMeta::new(verifier_config, false)],
            data: VerifierInstruction::SubmitSignatureCommitment {
                commitment: paused_inbound_commitment,
                validator_proof: paused_inbound_validator_proof,
                latest_mmr_leaf: paused_inbound_leaf,
                proof: paused_inbound_mmr_proof.clone(),
            }
            .try_to_vec()
            .unwrap(),
        };
        let import_tx = Transaction::new_signed_with_payer(
            &[import_ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        banks_client.process_transaction(import_tx).await.unwrap();
        SoraBurnProofV1 {
            mmr_proof: paused_inbound_mmr_proof,
            leaf: paused_inbound_leaf,
            digest_scale: paused_inbound_digest_scale,
        }
        .try_to_vec()
        .unwrap()
    };
    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new(token_cfg, false),
                AccountMeta::new(mint, false),
                AccountMeta::new(alice_token.pubkey(), false),
                AccountMeta::new(paused_inbound_marker, false),
                AccountMeta::new_readonly(system_program::id(), false),
                AccountMeta::new_readonly(spl_token::id(), false),
                AccountMeta::new_readonly(verifier_program_id, false),
                AccountMeta::new_readonly(verifier_config, false),
            ],
            data: SccpInstruction::MintFromProof {
                source_domain: SCCP_DOMAIN_SORA,
                payload: paused_inbound_payload_bytes.to_vec(),
                proof: paused_inbound_proof_bytes.clone(),
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::TokenNotActive as u32);
    }

    // Resume via governance proof and reuse the exact same mint proof successfully.
    let resume_payload = TokenControlPayloadV1 {
        version: 1,
        target_domain: SCCP_DOMAIN_SOL,
        nonce: 3,
        sora_asset_id,
    };
    let resume_payload_bytes = resume_payload.encode_scale();
    let resume_message_id = token_resume_message_id(&resume_payload_bytes);
    let (resume_marker, _resume_marker_bump) =
        inbound_marker_pda(&program_id, SCCP_DOMAIN_SORA, &resume_message_id);
    let resume_digest_scale = sccp_digest_scale_for_message_ids(&[resume_message_id]);
    let resume_digest_hash = keccak256(&resume_digest_scale);
    let resume_leaf = VerifierMmrLeaf {
        version: 0,
        parent_number: 7,
        parent_hash: [0x5au8; 32],
        next_authority_set_id: next_vset.id,
        next_authority_set_len: next_vset.len,
        next_authority_set_root: vset_root,
        random_seed: [0x6bu8; 32],
        digest_hash: resume_digest_hash,
    };
    let resume_commitment = VerifierCommitment {
        mmr_root: hash_leaf(&resume_leaf),
        block_number: 7,
        validator_set_id: current_vset.id,
    };
    let resume_commitment_hash = hash_commitment(&resume_commitment);
    let resume_msg = Message::parse(&resume_commitment_hash);
    let mut resume_sigs: Vec<Vec<u8>> = Vec::new();
    let mut resume_positions: Vec<u64> = Vec::new();
    let mut resume_pub_keys: Vec<[u8; 20]> = Vec::new();
    let mut resume_merkle_proofs: Vec<Vec<[u8; 32]>> = Vec::new();
    for i in 0..3 {
        let (sig, recid) = sign(&resume_msg, &validator_sks[i]);
        let mut sig65 = Vec::with_capacity(65);
        sig65.extend_from_slice(&sig.serialize());
        sig65.push(recid.serialize());
        resume_sigs.push(sig65);
        resume_positions.push(i as u64);
        resume_pub_keys.push(validator_addrs[i]);
        resume_merkle_proofs.push(merkle_proof(&validator_layers, i));
    }
    let resume_validator_proof = VerifierValidatorProof {
        signatures: resume_sigs,
        positions: resume_positions,
        public_keys: resume_pub_keys,
        public_key_merkle_proofs: resume_merkle_proofs,
    };
    let resume_mmr_proof = VerifierMmrProof {
        leaf_index: 0,
        leaf_count: 1,
        items: vec![],
    };
    {
        let import_ix = Instruction {
            program_id: verifier_program_id,
            accounts: vec![AccountMeta::new(verifier_config, false)],
            data: VerifierInstruction::SubmitSignatureCommitment {
                commitment: resume_commitment,
                validator_proof: resume_validator_proof,
                latest_mmr_leaf: resume_leaf,
                proof: resume_mmr_proof.clone(),
            }
            .try_to_vec()
            .unwrap(),
        };
        let import_tx = Transaction::new_signed_with_payer(
            &[import_ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        banks_client.process_transaction(import_tx).await.unwrap();

        let resume_proof_bytes = SoraBurnProofV1 {
            mmr_proof: resume_mmr_proof,
            leaf: resume_leaf,
            digest_scale: resume_digest_scale,
        }
        .try_to_vec()
        .unwrap();
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new(token_cfg, false),
                AccountMeta::new(resume_marker, false),
                AccountMeta::new_readonly(verifier_program_id, false),
                AccountMeta::new_readonly(verifier_config, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: SccpInstruction::ResumeTokenFromProof {
                payload: resume_payload_bytes.to_vec(),
                proof: resume_proof_bytes,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        banks_client.process_transaction(tx).await.unwrap();
    }
    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new(token_cfg, false),
                AccountMeta::new(mint, false),
                AccountMeta::new(alice_token.pubkey(), false),
                AccountMeta::new(paused_inbound_marker, false),
                AccountMeta::new_readonly(system_program::id(), false),
                AccountMeta::new_readonly(spl_token::id(), false),
                AccountMeta::new_readonly(verifier_program_id, false),
                AccountMeta::new_readonly(verifier_config, false),
            ],
            data: SccpInstruction::MintFromProof {
                source_domain: SCCP_DOMAIN_SORA,
                payload: paused_inbound_payload_bytes.to_vec(),
                proof: paused_inbound_proof_bytes,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        banks_client.process_transaction(tx).await.unwrap();
    }
    {
        let acct = banks_client
            .get_account(alice_token.pubkey())
            .await
            .unwrap()
            .unwrap();
        let ta = TokenAccount::unpack(&acct.data).unwrap();
        assert_eq!(ta.amount, mint_amount + mint_amount_eth + 1);
    }

    // Digest SCALE trailing bytes must fail closed even when message id is otherwise present.
    {
        let trailing_payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_SOL,
            nonce: 99,
            sora_asset_id,
            amount: 3,
            recipient: alice.pubkey().to_bytes(),
        };
        let trailing_payload_bytes = trailing_payload.encode_scale();
        let trailing_message_id = burn_message_id(&trailing_payload_bytes);
        let (trailing_marker, _trailing_marker_bump) =
            inbound_marker_pda(&program_id, SCCP_DOMAIN_SORA, &trailing_message_id);

        let mut trailing_digest_scale: Vec<u8> = Vec::with_capacity(1 + 7 + 32 + 1);
        trailing_digest_scale.push(0x04); // compact(len=1)
        trailing_digest_scale.extend_from_slice(&[0x00, 0x02, 0x50, 0x43, 0x43, 0x53]);
        trailing_digest_scale.extend_from_slice(&trailing_message_id);
        trailing_digest_scale.push(0x00); // trailing garbage byte
        let trailing_digest_hash = keccak256(&trailing_digest_scale);

        let trailing_leaf = VerifierMmrLeaf {
            version: 0,
            parent_number: 8,
            parent_hash: [0x56u8; 32],
            next_authority_set_id: next_vset.id,
            next_authority_set_len: next_vset.len,
            next_authority_set_root: vset_root,
            random_seed: [0x67u8; 32],
            digest_hash: trailing_digest_hash,
        };
        let trailing_leaf_hash = hash_leaf(&trailing_leaf);
        let trailing_commitment = VerifierCommitment {
            mmr_root: trailing_leaf_hash,
            block_number: 8,
            validator_set_id: current_vset.id,
        };
        let trailing_commitment_hash = hash_commitment(&trailing_commitment);
        let trailing_msg = Message::parse(&trailing_commitment_hash);

        let mut trailing_sigs: Vec<Vec<u8>> = Vec::new();
        let mut trailing_positions: Vec<u64> = Vec::new();
        let mut trailing_pub_keys: Vec<[u8; 20]> = Vec::new();
        let mut trailing_merkle_proofs: Vec<Vec<[u8; 32]>> = Vec::new();
        for i in 0..3 {
            let (sig, recid) = sign(&trailing_msg, &validator_sks[i]);
            let mut sig65 = Vec::with_capacity(65);
            sig65.extend_from_slice(&sig.serialize());
            sig65.push(recid.serialize());
            trailing_sigs.push(sig65);
            trailing_positions.push(i as u64);
            trailing_pub_keys.push(validator_addrs[i]);
            trailing_merkle_proofs.push(merkle_proof(&validator_layers, i));
        }
        let trailing_validator_proof = VerifierValidatorProof {
            signatures: trailing_sigs,
            positions: trailing_positions,
            public_keys: trailing_pub_keys,
            public_key_merkle_proofs: trailing_merkle_proofs,
        };
        let trailing_mmr_proof = VerifierMmrProof {
            leaf_index: 0,
            leaf_count: 1,
            items: vec![],
        };
        let trailing_import_ix = Instruction {
            program_id: verifier_program_id,
            accounts: vec![AccountMeta::new(verifier_config, false)],
            data: VerifierInstruction::SubmitSignatureCommitment {
                commitment: trailing_commitment,
                validator_proof: trailing_validator_proof,
                latest_mmr_leaf: trailing_leaf,
                proof: trailing_mmr_proof.clone(),
            }
            .try_to_vec()
            .unwrap(),
        };
        let trailing_import_tx = Transaction::new_signed_with_payer(
            &[trailing_import_ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        banks_client
            .process_transaction(trailing_import_tx)
            .await
            .unwrap();

        let trailing_burn_proof_bytes = SoraBurnProofV1 {
            mmr_proof: trailing_mmr_proof,
            leaf: trailing_leaf,
            digest_scale: trailing_digest_scale,
        }
        .try_to_vec()
        .unwrap();

        let before_failed_mint_amount = {
            let acct = banks_client
                .get_account(alice_token.pubkey())
                .await
                .unwrap()
                .unwrap();
            let ta = TokenAccount::unpack(&acct.data).unwrap();
            ta.amount
        };
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new(token_cfg, false),
                AccountMeta::new(mint, false),
                AccountMeta::new(alice_token.pubkey(), false),
                AccountMeta::new(trailing_marker, false),
                AccountMeta::new_readonly(system_program::id(), false),
                AccountMeta::new_readonly(spl_token::id(), false),
                AccountMeta::new_readonly(verifier_program_id, false),
                AccountMeta::new_readonly(verifier_config, false),
            ],
            data: SccpInstruction::MintFromProof {
                source_domain: SCCP_DOMAIN_SORA,
                payload: trailing_payload_bytes.to_vec(),
                proof: trailing_burn_proof_bytes,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::ProofVerificationFailed as u32);
        assert!(
            banks_client
                .get_account(trailing_marker)
                .await
                .unwrap()
                .is_none(),
            "failed trailing-digest mint must not create inbound marker account"
        );
        let after_failed_mint_amount = {
            let acct = banks_client
                .get_account(alice_token.pubkey())
                .await
                .unwrap()
                .unwrap();
            let ta = TokenAccount::unpack(&acct.data).unwrap();
            ta.amount
        };
        assert_eq!(
            after_failed_mint_amount, before_failed_mint_amount,
            "failed trailing-digest mint must not change recipient token balance"
        );
    }

    // Digest SCALE with mode=3 vec length prefix must fail closed.
    {
        let mode3_payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_SOL,
            nonce: 100,
            sora_asset_id,
            amount: 4,
            recipient: alice.pubkey().to_bytes(),
        };
        let mode3_payload_bytes = mode3_payload.encode_scale();
        let mode3_message_id = burn_message_id(&mode3_payload_bytes);
        let (mode3_marker, _mode3_marker_bump) =
            inbound_marker_pda(&program_id, SCCP_DOMAIN_SORA, &mode3_message_id);

        // Compact-u32 mode=3 is intentionally unsupported by the verifier parser.
        let mode3_digest_scale: Vec<u8> = vec![0x03, 0x00, 0x00, 0x00, 0x00];
        let mode3_digest_hash = keccak256(&mode3_digest_scale);

        let mode3_leaf = VerifierMmrLeaf {
            version: 0,
            parent_number: 9,
            parent_hash: [0x57u8; 32],
            next_authority_set_id: next_vset.id,
            next_authority_set_len: next_vset.len,
            next_authority_set_root: vset_root,
            random_seed: [0x68u8; 32],
            digest_hash: mode3_digest_hash,
        };
        let mode3_leaf_hash = hash_leaf(&mode3_leaf);
        let mode3_commitment = VerifierCommitment {
            mmr_root: mode3_leaf_hash,
            block_number: 9,
            validator_set_id: current_vset.id,
        };
        let mode3_commitment_hash = hash_commitment(&mode3_commitment);
        let mode3_msg = Message::parse(&mode3_commitment_hash);

        let mut mode3_sigs: Vec<Vec<u8>> = Vec::new();
        let mut mode3_positions: Vec<u64> = Vec::new();
        let mut mode3_pub_keys: Vec<[u8; 20]> = Vec::new();
        let mut mode3_merkle_proofs: Vec<Vec<[u8; 32]>> = Vec::new();
        for i in 0..3 {
            let (sig, recid) = sign(&mode3_msg, &validator_sks[i]);
            let mut sig65 = Vec::with_capacity(65);
            sig65.extend_from_slice(&sig.serialize());
            sig65.push(recid.serialize());
            mode3_sigs.push(sig65);
            mode3_positions.push(i as u64);
            mode3_pub_keys.push(validator_addrs[i]);
            mode3_merkle_proofs.push(merkle_proof(&validator_layers, i));
        }
        let mode3_validator_proof = VerifierValidatorProof {
            signatures: mode3_sigs,
            positions: mode3_positions,
            public_keys: mode3_pub_keys,
            public_key_merkle_proofs: mode3_merkle_proofs,
        };
        let mode3_mmr_proof = VerifierMmrProof {
            leaf_index: 0,
            leaf_count: 1,
            items: vec![],
        };
        let mode3_import_ix = Instruction {
            program_id: verifier_program_id,
            accounts: vec![AccountMeta::new(verifier_config, false)],
            data: VerifierInstruction::SubmitSignatureCommitment {
                commitment: mode3_commitment,
                validator_proof: mode3_validator_proof,
                latest_mmr_leaf: mode3_leaf,
                proof: mode3_mmr_proof.clone(),
            }
            .try_to_vec()
            .unwrap(),
        };
        let mode3_import_tx = Transaction::new_signed_with_payer(
            &[mode3_import_ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        banks_client
            .process_transaction(mode3_import_tx)
            .await
            .unwrap();

        let mode3_burn_proof_bytes = SoraBurnProofV1 {
            mmr_proof: mode3_mmr_proof,
            leaf: mode3_leaf,
            digest_scale: mode3_digest_scale,
        }
        .try_to_vec()
        .unwrap();

        let before_failed_mode3_mint_amount = {
            let acct = banks_client
                .get_account(alice_token.pubkey())
                .await
                .unwrap()
                .unwrap();
            let ta = TokenAccount::unpack(&acct.data).unwrap();
            ta.amount
        };
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new(token_cfg, false),
                AccountMeta::new(mint, false),
                AccountMeta::new(alice_token.pubkey(), false),
                AccountMeta::new(mode3_marker, false),
                AccountMeta::new_readonly(system_program::id(), false),
                AccountMeta::new_readonly(spl_token::id(), false),
                AccountMeta::new_readonly(verifier_program_id, false),
                AccountMeta::new_readonly(verifier_config, false),
            ],
            data: SccpInstruction::MintFromProof {
                source_domain: SCCP_DOMAIN_SORA,
                payload: mode3_payload_bytes.to_vec(),
                proof: mode3_burn_proof_bytes,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::ProofVerificationFailed as u32);
        assert!(
            banks_client
                .get_account(mode3_marker)
                .await
                .unwrap()
                .is_none(),
            "failed mode=3 digest mint must not create inbound marker account"
        );
        let after_failed_mode3_mint_amount = {
            let acct = banks_client
                .get_account(alice_token.pubkey())
                .await
                .unwrap()
                .unwrap();
            let ta = TokenAccount::unpack(&acct.data).unwrap();
            ta.amount
        };
        assert_eq!(
            after_failed_mode3_mint_amount, before_failed_mode3_mint_amount,
            "failed mode=3 digest mint must not change recipient token balance"
        );
    }

    // Unsupported source domain must fail-closed in mint path.
    {
        let unsupported_source_domain: u32 = 99;
        let (unsupported_marker, _unsupported_marker_bump) = inbound_marker_pda(
            &program_id,
            unsupported_source_domain,
            &inbound_message_id_sora,
        );
        let before_failed_mint_amount = {
            let acct = banks_client
                .get_account(alice_token.pubkey())
                .await
                .unwrap()
                .unwrap();
            let ta = TokenAccount::unpack(&acct.data).unwrap();
            ta.amount
        };
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new(token_cfg, false),
                AccountMeta::new(mint, false),
                AccountMeta::new(alice_token.pubkey(), false),
                AccountMeta::new(unsupported_marker, false),
                AccountMeta::new_readonly(system_program::id(), false),
                AccountMeta::new_readonly(spl_token::id(), false),
                AccountMeta::new_readonly(verifier_program_id, false),
                AccountMeta::new_readonly(verifier_config, false),
            ],
            data: SccpInstruction::MintFromProof {
                source_domain: unsupported_source_domain,
                payload: inbound_payload_sora_bytes.to_vec(),
                proof: vec![],
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            recent_blockhash,
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::DomainUnsupported as u32);
        assert!(
            banks_client
                .get_account(unsupported_marker)
                .await
                .unwrap()
                .is_none(),
            "failed unsupported-domain mint must not create inbound marker account"
        );
        let after_failed_mint_amount = {
            let acct = banks_client
                .get_account(alice_token.pubkey())
                .await
                .unwrap()
                .unwrap();
            let ta = TokenAccount::unpack(&acct.data).unwrap();
            ta.amount
        };
        assert_eq!(
            after_failed_mint_amount, before_failed_mint_amount,
            "failed unsupported-domain mint must not change recipient token balance"
        );
    }

    // Zero-amount inbound payload must fail closed.
    {
        let zero_amount_payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_SOL,
            nonce: 3,
            sora_asset_id,
            amount: 0,
            recipient: alice.pubkey().to_bytes(),
        };
        let zero_amount_payload_bytes = zero_amount_payload.encode_scale();
        let zero_amount_message_id = burn_message_id(&zero_amount_payload_bytes);
        let (zero_amount_marker, _zero_amount_marker_bump) =
            inbound_marker_pda(&program_id, SCCP_DOMAIN_SORA, &zero_amount_message_id);

        let before_failed_mint_amount = {
            let acct = banks_client
                .get_account(alice_token.pubkey())
                .await
                .unwrap()
                .unwrap();
            let ta = TokenAccount::unpack(&acct.data).unwrap();
            ta.amount
        };
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new(token_cfg, false),
                AccountMeta::new(mint, false),
                AccountMeta::new(alice_token.pubkey(), false),
                AccountMeta::new(zero_amount_marker, false),
                AccountMeta::new_readonly(system_program::id(), false),
                AccountMeta::new_readonly(spl_token::id(), false),
                AccountMeta::new_readonly(verifier_program_id, false),
                AccountMeta::new_readonly(verifier_config, false),
            ],
            data: SccpInstruction::MintFromProof {
                source_domain: SCCP_DOMAIN_SORA,
                payload: zero_amount_payload_bytes.to_vec(),
                proof: burn_proof_bytes.clone(),
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            recent_blockhash,
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::AmountIsZero as u32);
        assert!(
            banks_client
                .get_account(zero_amount_marker)
                .await
                .unwrap()
                .is_none(),
            "failed zero-amount mint must not create inbound marker account"
        );
        let after_failed_mint_amount = {
            let acct = banks_client
                .get_account(alice_token.pubkey())
                .await
                .unwrap()
                .unwrap();
            let ta = TokenAccount::unpack(&acct.data).unwrap();
            ta.amount
        };
        assert_eq!(
            after_failed_mint_amount, before_failed_mint_amount,
            "failed zero-amount mint must not change recipient token balance"
        );
    }

    // Disabled local admin paths fail closed before domain validation.
    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
            ],
            data: SccpInstruction::SetInboundDomainPaused {
                source_domain: 99,
                paused: true,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            recent_blockhash,
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::AdminPathDisabled as u32);
    }
    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
            ],
            data: SccpInstruction::SetOutboundDomainPaused {
                dest_domain: 99,
                paused: true,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            recent_blockhash,
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::AdminPathDisabled as u32);
    }

    // Unsupported burn destination domain must fail-closed.
    {
        let unsupported_burn_rec = burn_record_pda(&program_id, &[0xBBu8; 32]).0;
        let before_failed_burn_nonce = {
            let cfg_acc = banks_client.get_account(config).await.unwrap().unwrap();
            let cfg_state = Config::try_from_slice(&cfg_acc.data).unwrap();
            cfg_state.outbound_nonce
        };
        let before_failed_burn_amount = {
            let acct = banks_client
                .get_account(alice_token.pubkey())
                .await
                .unwrap()
                .unwrap();
            let ta = TokenAccount::unpack(&acct.data).unwrap();
            ta.amount
        };
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(alice.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new(token_cfg, false),
                AccountMeta::new(alice_token.pubkey(), false),
                AccountMeta::new(mint, false),
                AccountMeta::new(unsupported_burn_rec, false),
                AccountMeta::new_readonly(system_program::id(), false),
                AccountMeta::new_readonly(spl_token::id(), false),
            ],
            data: SccpInstruction::Burn {
                sora_asset_id,
                amount: 1,
                dest_domain: 99,
                recipient: [0x22u8; 32],
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer, &alice],
            recent_blockhash,
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::DomainUnsupported as u32);
        assert!(
            banks_client
                .get_account(unsupported_burn_rec)
                .await
                .unwrap()
                .is_none(),
            "failed unsupported-domain burn must not create burn-record account"
        );
        let after_failed_burn_amount = {
            let acct = banks_client
                .get_account(alice_token.pubkey())
                .await
                .unwrap()
                .unwrap();
            let ta = TokenAccount::unpack(&acct.data).unwrap();
            ta.amount
        };
        assert_eq!(
            after_failed_burn_amount, before_failed_burn_amount,
            "failed unsupported-domain burn must not change sender token balance"
        );
        let after_failed_burn_nonce = {
            let cfg_acc = banks_client.get_account(config).await.unwrap().unwrap();
            let cfg_state = Config::try_from_slice(&cfg_acc.data).unwrap();
            cfg_state.outbound_nonce
        };
        assert_eq!(
            after_failed_burn_nonce, before_failed_burn_nonce,
            "failed unsupported-domain burn must not change outbound nonce"
        );
    }

    // Zero-amount burn must fail closed.
    {
        let zero_burn_rec = burn_record_pda(&program_id, &[0xBCu8; 32]).0;
        let before_zero_burn_nonce = {
            let cfg_acc = banks_client.get_account(config).await.unwrap().unwrap();
            let cfg_state = Config::try_from_slice(&cfg_acc.data).unwrap();
            cfg_state.outbound_nonce
        };
        let before_zero_burn_amount = {
            let acct = banks_client
                .get_account(alice_token.pubkey())
                .await
                .unwrap()
                .unwrap();
            let ta = TokenAccount::unpack(&acct.data).unwrap();
            ta.amount
        };
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(alice.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new(token_cfg, false),
                AccountMeta::new(alice_token.pubkey(), false),
                AccountMeta::new(mint, false),
                AccountMeta::new(zero_burn_rec, false),
                AccountMeta::new_readonly(system_program::id(), false),
                AccountMeta::new_readonly(spl_token::id(), false),
            ],
            data: SccpInstruction::Burn {
                sora_asset_id,
                amount: 0,
                dest_domain: SCCP_DOMAIN_ETH,
                recipient: [0x22u8; 32],
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer, &alice],
            recent_blockhash,
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::AmountIsZero as u32);
        assert!(
            banks_client
                .get_account(zero_burn_rec)
                .await
                .unwrap()
                .is_none(),
            "failed zero-amount burn must not create burn-record account"
        );
        let after_zero_burn_amount = {
            let acct = banks_client
                .get_account(alice_token.pubkey())
                .await
                .unwrap()
                .unwrap();
            let ta = TokenAccount::unpack(&acct.data).unwrap();
            ta.amount
        };
        assert_eq!(
            after_zero_burn_amount, before_zero_burn_amount,
            "failed zero-amount burn must not change sender token balance"
        );
        let after_zero_burn_nonce = {
            let cfg_acc = banks_client.get_account(config).await.unwrap().unwrap();
            let cfg_state = Config::try_from_slice(&cfg_acc.data).unwrap();
            cfg_state.outbound_nonce
        };
        assert_eq!(
            after_zero_burn_nonce, before_zero_burn_nonce,
            "failed zero-amount burn must not change outbound nonce"
        );
    }

    // Burn to an EVM domain must enforce canonical recipient encoding (high 12 bytes must be zero).
    {
        let burn_amount: u64 = 1;
        let mut bad_recipient = [0u8; 32];
        bad_recipient[0] = 1; // non-zero high bytes => non-canonical EVM encoding

        let dummy_burn_rec = burn_record_pda(&program_id, &[0xAAu8; 32]).0;
        let before_bad_recipient_nonce = {
            let cfg_acc = banks_client.get_account(config).await.unwrap().unwrap();
            let cfg_state = Config::try_from_slice(&cfg_acc.data).unwrap();
            cfg_state.outbound_nonce
        };
        let before_bad_recipient_burn_amount = {
            let acct = banks_client
                .get_account(alice_token.pubkey())
                .await
                .unwrap()
                .unwrap();
            let ta = TokenAccount::unpack(&acct.data).unwrap();
            ta.amount
        };
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(alice.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new(token_cfg, false),
                AccountMeta::new(alice_token.pubkey(), false),
                AccountMeta::new(mint, false),
                AccountMeta::new(dummy_burn_rec, false),
                AccountMeta::new_readonly(system_program::id(), false),
                AccountMeta::new_readonly(spl_token::id(), false),
            ],
            data: SccpInstruction::Burn {
                sora_asset_id,
                amount: burn_amount,
                dest_domain: SCCP_DOMAIN_ETH,
                recipient: bad_recipient,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer, &alice],
            recent_blockhash,
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::RecipientNotCanonical as u32);
        assert!(
            banks_client
                .get_account(dummy_burn_rec)
                .await
                .unwrap()
                .is_none(),
            "failed non-canonical burn must not create burn-record account"
        );
        let after_bad_recipient_burn_amount = {
            let acct = banks_client
                .get_account(alice_token.pubkey())
                .await
                .unwrap()
                .unwrap();
            let ta = TokenAccount::unpack(&acct.data).unwrap();
            ta.amount
        };
        assert_eq!(
            after_bad_recipient_burn_amount, before_bad_recipient_burn_amount,
            "failed non-canonical burn must not change sender token balance"
        );
        let after_bad_recipient_nonce = {
            let cfg_acc = banks_client.get_account(config).await.unwrap().unwrap();
            let cfg_state = Config::try_from_slice(&cfg_acc.data).unwrap();
            cfg_state.outbound_nonce
        };
        assert_eq!(
            after_bad_recipient_nonce, before_bad_recipient_nonce,
            "failed non-canonical burn must not change outbound nonce"
        );
    }

    // Burn with an all-zero recipient must fail closed and not mutate balance/state.
    {
        let burn_amount: u64 = 1;
        let zero_recipient = [0u8; 32];

        let zero_burn_rec = burn_record_pda(&program_id, &[0xBBu8; 32]).0;
        let before_zero_recipient_nonce = {
            let cfg_acc = banks_client.get_account(config).await.unwrap().unwrap();
            let cfg_state = Config::try_from_slice(&cfg_acc.data).unwrap();
            cfg_state.outbound_nonce
        };
        let before_zero_recipient_burn_amount = {
            let acct = banks_client
                .get_account(alice_token.pubkey())
                .await
                .unwrap()
                .unwrap();
            let ta = TokenAccount::unpack(&acct.data).unwrap();
            ta.amount
        };
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(alice.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new(token_cfg, false),
                AccountMeta::new(alice_token.pubkey(), false),
                AccountMeta::new(mint, false),
                AccountMeta::new(zero_burn_rec, false),
                AccountMeta::new_readonly(system_program::id(), false),
                AccountMeta::new_readonly(spl_token::id(), false),
            ],
            data: SccpInstruction::Burn {
                sora_asset_id,
                amount: burn_amount,
                dest_domain: SCCP_DOMAIN_ETH,
                recipient: zero_recipient,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer, &alice],
            recent_blockhash,
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::RecipientIsZero as u32);
        assert!(
            banks_client
                .get_account(zero_burn_rec)
                .await
                .unwrap()
                .is_none(),
            "failed zero-recipient burn must not create burn-record account"
        );
        let after_zero_recipient_burn_amount = {
            let acct = banks_client
                .get_account(alice_token.pubkey())
                .await
                .unwrap()
                .unwrap();
            let ta = TokenAccount::unpack(&acct.data).unwrap();
            ta.amount
        };
        assert_eq!(
            after_zero_recipient_burn_amount, before_zero_recipient_burn_amount,
            "failed zero-recipient burn must not change sender token balance"
        );
        let after_zero_recipient_nonce = {
            let cfg_acc = banks_client.get_account(config).await.unwrap().unwrap();
            let cfg_state = Config::try_from_slice(&cfg_acc.data).unwrap();
            cfg_state.outbound_nonce
        };
        assert_eq!(
            after_zero_recipient_nonce, before_zero_recipient_nonce,
            "failed zero-recipient burn must not change outbound nonce"
        );
    }

    // Replay must fail: InboundAlreadyProcessed.
    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new(token_cfg, false),
                AccountMeta::new(mint, false),
                AccountMeta::new(alice_token.pubkey(), false),
                AccountMeta::new(marker_sora, false),
                AccountMeta::new_readonly(system_program::id(), false),
                AccountMeta::new_readonly(spl_token::id(), false),
                AccountMeta::new_readonly(verifier_program_id, false),
                AccountMeta::new_readonly(verifier_config, false),
            ],
            data: SccpInstruction::MintFromProof {
                source_domain: SCCP_DOMAIN_SORA,
                payload: inbound_payload_sora_bytes.to_vec(),
                proof: vec![],
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::InboundAlreadyProcessed as u32);
    }

    // Clear-invalidated is disabled and must not mutate a processed marker.
    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new(marker_sora, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: SccpInstruction::ClearInvalidatedInboundMessage {
                source_domain: SCCP_DOMAIN_SORA,
                message_id: inbound_message_id_sora,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::AdminPathDisabled as u32);
    }
    {
        let marker_acc = banks_client
            .get_account(marker_sora)
            .await
            .unwrap()
            .unwrap();
        let marker_data = InboundMarker::try_from_slice(&marker_acc.data).unwrap();
        assert_eq!(marker_data.status, InboundStatus::Processed);
    }
    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new(token_cfg, false),
                AccountMeta::new(mint, false),
                AccountMeta::new(alice_token.pubkey(), false),
                AccountMeta::new(marker_sora, false),
                AccountMeta::new_readonly(system_program::id(), false),
                AccountMeta::new_readonly(spl_token::id(), false),
                AccountMeta::new_readonly(verifier_program_id, false),
                AccountMeta::new_readonly(verifier_config, false),
            ],
            data: SccpInstruction::MintFromProof {
                source_domain: SCCP_DOMAIN_SORA,
                payload: inbound_payload_sora_bytes.to_vec(),
                proof: vec![],
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::InboundAlreadyProcessed as u32);
    }

    // Disabled local admin attempts must fail closed without mutating inbound state.
    let inv_payload = BurnPayloadV1 {
        version: 1,
        source_domain: SCCP_DOMAIN_ETH,
        dest_domain: SCCP_DOMAIN_SOL,
        nonce: 2,
        sora_asset_id,
        amount: 1u128,
        recipient: alice.pubkey().to_bytes(),
    };
    let inv_payload_bytes = inv_payload.encode_scale();
    let inv_message_id = burn_message_id(&inv_payload_bytes);
    let (inv_marker, _inv_marker_bump) =
        inbound_marker_pda(&program_id, SCCP_DOMAIN_ETH, &inv_message_id);
    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new(inv_marker, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: SccpInstruction::InvalidateInboundMessage {
                source_domain: SCCP_DOMAIN_ETH,
                message_id: inv_message_id,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            recent_blockhash,
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::AdminPathDisabled as u32);
    }
    assert!(
        banks_client
            .get_account(inv_marker)
            .await
            .unwrap()
            .is_none(),
        "disabled invalidation must not create inbound marker account"
    );

    // Disabled inbound pause path must not toggle config bits.
    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
            ],
            data: SccpInstruction::SetInboundDomainPaused {
                source_domain: SCCP_DOMAIN_SORA,
                paused: true,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            recent_blockhash,
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::AdminPathDisabled as u32);
    }
    {
        let cfg_mid = banks_client.get_account(config).await.unwrap().unwrap();
        let cfg_mid = Config::try_from_slice(&cfg_mid.data).unwrap();
        assert_eq!(cfg_mid.inbound_paused_mask, 0);
    }

    // Burn SOL -> SORA: creates burn record PDA keyed by messageId.
    let burn_amount: u64 = 10;
    let recipient_on_sora = [0x22u8; 32];

    // Fund alice so she can pay rent for the burn record PDA creation (burn uses `user` as payer).
    {
        let ix = system_instruction::transfer(&payer.pubkey(), &alice.pubkey(), 1_000_000_000);
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            recent_blockhash,
        );
        banks_client.process_transaction(tx).await.unwrap();
    }

    // Read outbound nonce before burn (should start at 0 and become 1 after burn).
    let cfg_before = banks_client.get_account(config).await.unwrap().unwrap();
    let cfg_before = Config::try_from_slice(&cfg_before.data).unwrap();
    assert_eq!(cfg_before.outbound_nonce, 0);

    let burn_payload = BurnPayloadV1 {
        version: 1,
        source_domain: SCCP_DOMAIN_SOL,
        dest_domain: SCCP_DOMAIN_SORA,
        nonce: 1, // cfg.outbound_nonce increments from 0 -> 1
        sora_asset_id,
        amount: burn_amount as u128,
        recipient: recipient_on_sora,
    };
    let burn_payload_bytes = burn_payload.encode_scale();
    let burn_message_id = burn_message_id(&burn_payload_bytes);
    let (burn_rec, _burn_rec_bump) = burn_record_pda(&program_id, &burn_message_id);

    // Disabled outbound pause path must not toggle config bits.
    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
            ],
            data: SccpInstruction::SetOutboundDomainPaused {
                dest_domain: SCCP_DOMAIN_SORA,
                paused: true,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            recent_blockhash,
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::AdminPathDisabled as u32);

        let cfg_mid = banks_client.get_account(config).await.unwrap().unwrap();
        let cfg_mid = Config::try_from_slice(&cfg_mid.data).unwrap();
        assert_eq!(cfg_mid.outbound_nonce, 0);
        assert_eq!(cfg_mid.outbound_paused_mask, 0);
    }

    // Outbound config must remain unpaused.
    {
        let cfg = banks_client.get_account(config).await.unwrap().unwrap();
        let cfg = Config::try_from_slice(&cfg.data).unwrap();
        assert_eq!(cfg.outbound_paused_mask, 0);
    }

    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(alice.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new(token_cfg, false),
                AccountMeta::new(alice_token.pubkey(), false),
                AccountMeta::new(mint, false),
                AccountMeta::new(burn_rec, false),
                AccountMeta::new_readonly(system_program::id(), false),
                AccountMeta::new_readonly(spl_token::id(), false),
            ],
            data: SccpInstruction::Burn {
                sora_asset_id,
                amount: burn_amount,
                dest_domain: SCCP_DOMAIN_SORA,
                recipient: recipient_on_sora,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer, &alice],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        banks_client.process_transaction(tx).await.unwrap();
    }

    {
        let acct = banks_client
            .get_account(alice_token.pubkey())
            .await
            .unwrap()
            .unwrap();
        let ta = TokenAccount::unpack(&acct.data).unwrap();
        assert_eq!(ta.amount, mint_amount + mint_amount_eth + 1 - burn_amount);
    }

    // Burn record exists and matches expected message id.
    {
        let rec_acc = banks_client.get_account(burn_rec).await.unwrap().unwrap();
        let rec = BurnRecord::try_from_slice(&rec_acc.data).unwrap();
        assert_eq!(rec.message_id, burn_message_id);
        assert_eq!(rec.payload, burn_payload_bytes);
        assert_eq!(rec.sender, alice.pubkey());
        assert_eq!(rec.mint, mint);
        assert_eq!(rec.version, 1);
    }

    // Config outbound nonce updated.
    {
        let cfg_after = banks_client.get_account(config).await.unwrap().unwrap();
        let cfg_after = Config::try_from_slice(&cfg_after.data).unwrap();
        assert_eq!(cfg_after.outbound_nonce, 1);
    }

    drop(test_lock);
}

#[tokio::test]
async fn solana_program_governor_gates_bootstrap_and_disables_local_admin_calls() {
    let test_lock = program_test_lock().await;
    let program_id = Pubkey::new_unique();
    let mut pt = ProgramTest::new(
        "sccp_sol_program",
        program_id,
        processor!(process_instruction),
    );
    pt.add_program(
        "spl_token",
        spl_token::id(),
        processor!(spl_token::processor::Processor::process),
    );
    let (mut banks_client, payer, _recent_blockhash) = pt.start().await;

    let attacker = Keypair::new();
    let (config, _config_bump) = config_pda(&program_id);

    // Initialize config with payer as governor.
    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: SccpInstruction::Initialize {
                governor: payer.pubkey(),
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        banks_client.process_transaction(tx).await.unwrap();
    }

    // Fund attacker so the signer account exists in runtime account loading.
    {
        let ix = system_instruction::transfer(&payer.pubkey(), &attacker.pubkey(), 1_000_000);
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        banks_client.process_transaction(tx).await.unwrap();
    }

    let verifier_program = Pubkey::new_unique();

    // Non-governor signers cannot bind the verifier program.
    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(attacker.pubkey(), true),
                AccountMeta::new(config, false),
            ],
            data: SccpInstruction::SetVerifierProgram { verifier_program }
                .try_to_vec()
                .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer, &attacker],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::NotGovernor as u32);
    }

    {
        let cfg_acc = banks_client.get_account(config).await.unwrap().unwrap();
        let cfg = Config::try_from_slice(&cfg_acc.data).unwrap();
        assert_eq!(cfg.verifier_program, Pubkey::default());
    }

    // Only the configured governor can register the verifier program.
    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
            ],
            data: SccpInstruction::SetVerifierProgram { verifier_program }
                .try_to_vec()
                .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        banks_client.process_transaction(tx).await.unwrap();
    }

    {
        let cfg_acc = banks_client.get_account(config).await.unwrap().unwrap();
        let cfg = Config::try_from_slice(&cfg_acc.data).unwrap();
        assert_eq!(cfg.verifier_program, verifier_program);
    }

    let sora_asset_id = [0x55u8; 32];
    let (token_cfg, _token_bump) = token_config_pda(&program_id, &sora_asset_id);
    let (mint, _mint_bump) = mint_pda(&program_id, &sora_asset_id);

    // Token deployment is also governor-gated.
    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(attacker.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new(token_cfg, false),
                AccountMeta::new(mint, false),
                AccountMeta::new_readonly(system_program::id(), false),
                AccountMeta::new_readonly(spl_token::id(), false),
                AccountMeta::new_readonly(sysvar::rent::id(), false),
            ],
            data: SccpInstruction::DeployToken {
                sora_asset_id,
                decimals: 9,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer, &attacker],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::NotGovernor as u32);
    }

    // The verifier binding is immutable once configured.
    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
            ],
            data: SccpInstruction::SetVerifierProgram {
                verifier_program: Pubkey::new_unique(),
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::VerifierProgramAlreadySet as u32);
    }

    // Local admin calls are disabled entirely.
    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(attacker.pubkey(), true),
                AccountMeta::new(config, false),
            ],
            data: SccpInstruction::SetGovernor {
                governor: attacker.pubkey(),
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer, &attacker],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::AdminPathDisabled as u32);
    }

    let message_id = [0x42u8; 32];
    let (marker, _marker_bump) = inbound_marker_pda(&program_id, SCCP_DOMAIN_SORA, &message_id);

    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(attacker.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new(marker, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: SccpInstruction::InvalidateInboundMessage {
                source_domain: SCCP_DOMAIN_SORA,
                message_id,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer, &attacker],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::AdminPathDisabled as u32);
    }

    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(attacker.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new(marker, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: SccpInstruction::ClearInvalidatedInboundMessage {
                source_domain: SCCP_DOMAIN_SORA,
                message_id,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer, &attacker],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::AdminPathDisabled as u32);
    }

    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(attacker.pubkey(), true),
                AccountMeta::new(config, false),
            ],
            data: SccpInstruction::SetInboundDomainPaused {
                source_domain: SCCP_DOMAIN_SORA,
                paused: true,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer, &attacker],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::AdminPathDisabled as u32);
    }

    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(attacker.pubkey(), true),
                AccountMeta::new(config, false),
            ],
            data: SccpInstruction::SetOutboundDomainPaused {
                dest_domain: SCCP_DOMAIN_ETH,
                paused: true,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer, &attacker],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::AdminPathDisabled as u32);
    }

    drop(test_lock);
}

#[tokio::test]
async fn solana_verifier_initialize_requires_governor() {
    let test_lock = program_test_lock().await;
    let program_id = Pubkey::new_unique();
    let pt = ProgramTest::new(
        "sccp_sol_verifier_program",
        program_id,
        processor!(verifier_process_instruction),
    );
    let (mut banks_client, payer, _recent_blockhash) = pt.start().await;

    let attacker = Keypair::new();
    let (config, _config_bump) = verifier_config_pda(&program_id);

    {
        let ix = system_instruction::transfer(&payer.pubkey(), &attacker.pubkey(), 1_000_000);
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        banks_client.process_transaction(tx).await.unwrap();
    }

    let validator_set = VerifierValidatorSet {
        id: 1,
        len: 1,
        root: [0u8; 32],
    };
    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(attacker.pubkey(), true),
            AccountMeta::new(config, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: VerifierInstruction::Initialize {
            governor: payer.pubkey(),
            latest_beefy_block: 0,
            current_validator_set: validator_set,
            next_validator_set: validator_set,
        }
        .try_to_vec()
        .unwrap(),
    };
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &attacker],
        banks_client.get_latest_blockhash().await.unwrap(),
    );
    let err = banks_client.process_transaction(tx).await.unwrap_err();
    expect_custom(err.into(), VerifierError::NotGovernor as u32);

    drop(test_lock);
}

#[tokio::test]
async fn solana_program_admin_paths_are_disabled_for_inbound_markers() {
    let test_lock = program_test_lock().await;
    let program_id = Pubkey::new_unique();
    let pt = ProgramTest::new(
        "sccp_sol_program",
        program_id,
        processor!(process_instruction),
    );
    let (mut banks_client, payer, _recent_blockhash) = pt.start().await;

    let (config, _config_bump) = config_pda(&program_id);

    // Initialize config with payer as governor.
    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: SccpInstruction::Initialize {
                governor: payer.pubkey(),
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        banks_client.process_transaction(tx).await.unwrap();
    }

    let message_id = [0xabu8; 32];
    let (marker, _marker_bump) = inbound_marker_pda(&program_id, SCCP_DOMAIN_SORA, &message_id);

    // Invalidate is disabled and must not create marker state.
    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new(marker, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: SccpInstruction::InvalidateInboundMessage {
                source_domain: SCCP_DOMAIN_SORA,
                message_id,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::AdminPathDisabled as u32);
    }
    assert!(banks_client.get_account(marker).await.unwrap().is_none());

    // Clear-invalidated is disabled too and must leave marker state absent.
    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new(marker, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: SccpInstruction::ClearInvalidatedInboundMessage {
                source_domain: SCCP_DOMAIN_SORA,
                message_id,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::AdminPathDisabled as u32);
    }
    assert!(banks_client.get_account(marker).await.unwrap().is_none());

    // Local-domain controls now fail on the disabled admin path before domain guards.
    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
            ],
            data: SccpInstruction::SetInboundDomainPaused {
                source_domain: SCCP_DOMAIN_SOL,
                paused: true,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::AdminPathDisabled as u32);
    }

    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
            ],
            data: SccpInstruction::SetOutboundDomainPaused {
                dest_domain: SCCP_DOMAIN_SOL,
                paused: true,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::AdminPathDisabled as u32);
    }

    let (local_marker, _local_marker_bump) =
        inbound_marker_pda(&program_id, SCCP_DOMAIN_SOL, &message_id);

    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new(local_marker, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: SccpInstruction::InvalidateInboundMessage {
                source_domain: SCCP_DOMAIN_SOL,
                message_id,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::AdminPathDisabled as u32);
    }

    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new(local_marker, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: SccpInstruction::ClearInvalidatedInboundMessage {
                source_domain: SCCP_DOMAIN_SOL,
                message_id,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::AdminPathDisabled as u32);
    }

    let (unsupported_marker, _unsupported_marker_bump) =
        inbound_marker_pda(&program_id, 99, &message_id);

    // Unsupported domains now also fail on the disabled admin path first.
    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new(unsupported_marker, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: SccpInstruction::InvalidateInboundMessage {
                source_domain: 99,
                message_id,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::AdminPathDisabled as u32);
    }

    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new(unsupported_marker, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: SccpInstruction::ClearInvalidatedInboundMessage {
                source_domain: 99,
                message_id,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::AdminPathDisabled as u32);
    }

    drop(test_lock);
}

#[tokio::test]
async fn solana_program_mint_from_proof_rejects_local_domain_and_bad_lengths_early() {
    let test_lock = program_test_lock().await;
    let program_id = Pubkey::new_unique();
    let pt = ProgramTest::new(
        "sccp_sol_program",
        program_id,
        processor!(process_instruction),
    );
    let (mut banks_client, payer, _recent_blockhash) = pt.start().await;

    // Unsupported source domain must fail before account loading.
    {
        let ix = Instruction {
            program_id,
            accounts: vec![],
            data: SccpInstruction::MintFromProof {
                source_domain: 99,
                payload: vec![],
                proof: vec![],
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::DomainUnsupported as u32);
    }

    // Local source domain must fail before account loading.
    {
        let ix = Instruction {
            program_id,
            accounts: vec![],
            data: SccpInstruction::MintFromProof {
                source_domain: SCCP_DOMAIN_SOL,
                payload: vec![],
                proof: vec![],
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::DomainEqualsLocal as u32);
    }

    // Invalid payload length must fail before account loading.
    {
        let ix = Instruction {
            program_id,
            accounts: vec![],
            data: SccpInstruction::MintFromProof {
                source_domain: SCCP_DOMAIN_SORA,
                payload: vec![0u8; 7],
                proof: vec![],
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::PayloadInvalidLength as u32);
    }

    // Oversized proof must fail before account loading.
    {
        let ix = Instruction {
            program_id,
            accounts: vec![],
            data: SccpInstruction::MintFromProof {
                source_domain: SCCP_DOMAIN_SORA,
                payload: vec![0u8; BurnPayloadV1::ENCODED_LEN],
                proof: vec![0u8; 16 * 1024 + 1],
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        let err: TransportError = err.into();
        match err {
            TransportError::TransactionError(TransactionError::InstructionError(
                _,
                InstructionError::InvalidInstructionData,
            )) => {}
            other => panic!("expected InvalidInstructionData, got: {other:?}"),
        }
    }

    // Proof exactly at max size should pass size guard and proceed to account loading.
    {
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_SOL,
            nonce: 1,
            sora_asset_id: [0x11u8; 32],
            amount: 1,
            recipient: [0x22u8; 32],
        }
        .encode_scale()
        .to_vec();

        let ix = Instruction {
            program_id,
            accounts: vec![],
            data: SccpInstruction::MintFromProof {
                source_domain: SCCP_DOMAIN_SORA,
                payload,
                proof: vec![0u8; 16 * 1024],
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        let err: TransportError = err.into();
        match err {
            TransportError::TransactionError(TransactionError::InstructionError(
                _,
                InstructionError::NotEnoughAccountKeys,
            )) => {}
            other => panic!("expected NotEnoughAccountKeys, got: {other:?}"),
        }
    }

    drop(test_lock);
}

#[tokio::test]
async fn solana_program_burn_rejects_invalid_inputs_before_account_loading() {
    let test_lock = program_test_lock().await;
    let program_id = Pubkey::new_unique();
    let pt = ProgramTest::new(
        "sccp_sol_program",
        program_id,
        processor!(process_instruction),
    );
    let (mut banks_client, payer, _recent_blockhash) = pt.start().await;

    // Unsupported destination domain must fail before account loading.
    {
        let ix = Instruction {
            program_id,
            accounts: vec![],
            data: SccpInstruction::Burn {
                sora_asset_id: [0x11u8; 32],
                amount: 1,
                dest_domain: 99,
                recipient: [0x22u8; 32],
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::DomainUnsupported as u32);
    }

    // Local destination domain must fail before account loading.
    {
        let ix = Instruction {
            program_id,
            accounts: vec![],
            data: SccpInstruction::Burn {
                sora_asset_id: [0x11u8; 32],
                amount: 1,
                dest_domain: SCCP_DOMAIN_SOL,
                recipient: [0x22u8; 32],
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::DomainEqualsLocal as u32);
    }

    // Zero burn amount must fail before account loading.
    {
        let ix = Instruction {
            program_id,
            accounts: vec![],
            data: SccpInstruction::Burn {
                sora_asset_id: [0x11u8; 32],
                amount: 0,
                dest_domain: SCCP_DOMAIN_SORA,
                recipient: [0x22u8; 32],
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::AmountIsZero as u32);
    }

    // Zero recipient must fail before account loading.
    {
        let ix = Instruction {
            program_id,
            accounts: vec![],
            data: SccpInstruction::Burn {
                sora_asset_id: [0x11u8; 32],
                amount: 1,
                dest_domain: SCCP_DOMAIN_SORA,
                recipient: [0u8; 32],
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::RecipientIsZero as u32);
    }

    // Non-canonical EVM recipient must fail before account loading.
    let mut non_canonical = [0x22u8; 32];
    non_canonical[0] = 1;
    {
        let ix = Instruction {
            program_id,
            accounts: vec![],
            data: SccpInstruction::Burn {
                sora_asset_id: [0x11u8; 32],
                amount: 1,
                dest_domain: SCCP_DOMAIN_ETH,
                recipient: non_canonical,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::RecipientNotCanonical as u32);
    }

    // BSC/TRON share EVM recipient canonicalization rules and must fail identically.
    for evm_domain in [SCCP_DOMAIN_BSC, SCCP_DOMAIN_TRON] {
        let ix = Instruction {
            program_id,
            accounts: vec![],
            data: SccpInstruction::Burn {
                sora_asset_id: [0x11u8; 32],
                amount: 1,
                dest_domain: evm_domain,
                recipient: non_canonical,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::RecipientNotCanonical as u32);
    }

    // If high bytes are non-zero while the low 20 bytes are all zero, canonical check still wins.
    let mut non_canonical_low_zero = [0u8; 32];
    non_canonical_low_zero[0] = 1;
    {
        let ix = Instruction {
            program_id,
            accounts: vec![],
            data: SccpInstruction::Burn {
                sora_asset_id: [0x11u8; 32],
                amount: 1,
                dest_domain: SCCP_DOMAIN_ETH,
                recipient: non_canonical_low_zero,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::RecipientNotCanonical as u32);
    }

    for evm_domain in [SCCP_DOMAIN_BSC, SCCP_DOMAIN_TRON] {
        let ix = Instruction {
            program_id,
            accounts: vec![],
            data: SccpInstruction::Burn {
                sora_asset_id: [0x11u8; 32],
                amount: 1,
                dest_domain: evm_domain,
                recipient: non_canonical_low_zero,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::RecipientNotCanonical as u32);
    }

    // Canonical EVM recipient encoding (high 12 bytes zero, low 20 bytes non-zero)
    // should pass preflight and then fail later due to missing accounts.
    let mut canonical_evm = [0u8; 32];
    canonical_evm[12..].copy_from_slice(&[0x22u8; 20]);
    {
        let ix = Instruction {
            program_id,
            accounts: vec![],
            data: SccpInstruction::Burn {
                sora_asset_id: [0x11u8; 32],
                amount: 1,
                dest_domain: SCCP_DOMAIN_ETH,
                recipient: canonical_evm,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        let err: TransportError = err.into();
        match err {
            TransportError::TransactionError(TransactionError::InstructionError(
                _,
                InstructionError::NotEnoughAccountKeys,
            )) => {}
            other => panic!("expected NotEnoughAccountKeys, got: {other:?}"),
        }
    }

    for evm_domain in [SCCP_DOMAIN_BSC, SCCP_DOMAIN_TRON] {
        let ix = Instruction {
            program_id,
            accounts: vec![],
            data: SccpInstruction::Burn {
                sora_asset_id: [0x11u8; 32],
                amount: 1,
                dest_domain: evm_domain,
                recipient: canonical_evm,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        let err: TransportError = err.into();
        match err {
            TransportError::TransactionError(TransactionError::InstructionError(
                _,
                InstructionError::NotEnoughAccountKeys,
            )) => {}
            other => panic!("expected NotEnoughAccountKeys, got: {other:?}"),
        }
    }

    // The same recipient shape is valid for TON; this should pass preflight checks
    // and then fail later due to missing accounts.
    {
        let ix = Instruction {
            program_id,
            accounts: vec![],
            data: SccpInstruction::Burn {
                sora_asset_id: [0x11u8; 32],
                amount: 1,
                dest_domain: SCCP_DOMAIN_TON,
                recipient: non_canonical,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        let err: TransportError = err.into();
        match err {
            TransportError::TransactionError(TransactionError::InstructionError(
                _,
                InstructionError::NotEnoughAccountKeys,
            )) => {}
            other => panic!("expected NotEnoughAccountKeys, got: {other:?}"),
        }
    }

    drop(test_lock);
}

#[tokio::test]
async fn solana_program_clear_invalidated_is_disabled_for_fresh_marker() {
    let test_lock = program_test_lock().await;
    let program_id = Pubkey::new_unique();
    let pt = ProgramTest::new(
        "sccp_sol_program",
        program_id,
        processor!(process_instruction),
    );
    let (mut banks_client, payer, _recent_blockhash) = pt.start().await;

    let (config, _config_bump) = config_pda(&program_id);

    // Initialize config with payer as governor.
    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: SccpInstruction::Initialize {
                governor: payer.pubkey(),
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        banks_client.process_transaction(tx).await.unwrap();
    }

    let message_id = [0x55u8; 32];
    let (marker, _marker_bump) = inbound_marker_pda(&program_id, SCCP_DOMAIN_SORA, &message_id);

    // Clearing without prior invalidation is disabled and must not create marker state.
    {
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new(marker, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: SccpInstruction::ClearInvalidatedInboundMessage {
                source_domain: SCCP_DOMAIN_SORA,
                message_id,
            }
            .try_to_vec()
            .unwrap(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            banks_client.get_latest_blockhash().await.unwrap(),
        );
        let err = banks_client.process_transaction(tx).await.unwrap_err();
        expect_custom(err.into(), SccpError::AdminPathDisabled as u32);
    }
    assert!(banks_client.get_account(marker).await.unwrap().is_none());

    drop(test_lock);
}
