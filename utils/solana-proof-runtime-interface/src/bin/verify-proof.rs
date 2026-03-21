use std::{env, fs, process::ExitCode};

use solana_proof_runtime_interface::{verifier, SolanaVerifyRequest, SolanaVoteAuthorityConfigV1};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut proof_file = None;
    let mut message_id = None;
    let mut router_program_id = None;
    let mut authority = None;
    let mut stake = None;
    let mut threshold_stake = None;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--proof-file" => proof_file = Some(next_arg(&mut args, "--proof-file")?),
            "--message-id-hex" => message_id = Some(next_arg(&mut args, "--message-id-hex")?),
            "--router-program-id-hex" => {
                router_program_id = Some(next_arg(&mut args, "--router-program-id-hex")?)
            }
            "--authority-hex" => authority = Some(next_arg(&mut args, "--authority-hex")?),
            "--stake" => stake = Some(next_arg(&mut args, "--stake")?),
            "--threshold-stake" => {
                threshold_stake = Some(next_arg(&mut args, "--threshold-stake")?)
            }
            "--help" | "-h" => {
                print_usage();
                return Ok(());
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    let proof = fs::read(proof_file.ok_or_else(|| "missing required --proof-file".to_string())?)
        .map_err(|err| format!("failed to read proof file: {err}"))?;
    let expected_message_id = parse_hex32(
        &message_id.ok_or_else(|| "missing required --message-id-hex".to_string())?,
        "--message-id-hex",
    )?;
    let expected_router_program_id = parse_hex32(
        &router_program_id.ok_or_else(|| "missing required --router-program-id-hex".to_string())?,
        "--router-program-id-hex",
    )?;
    let authority_pubkey = parse_hex32(
        &authority.ok_or_else(|| "missing required --authority-hex".to_string())?,
        "--authority-hex",
    )?;
    let stake = parse_u64(
        &stake.ok_or_else(|| "missing required --stake".to_string())?,
        "--stake",
    )?;
    let threshold_stake = parse_u64(
        &threshold_stake.ok_or_else(|| "missing required --threshold-stake".to_string())?,
        "--threshold-stake",
    )?;

    let request = SolanaVerifyRequest {
        proof,
        expected_message_id,
        expected_router_program_id,
        authorities: vec![SolanaVoteAuthorityConfigV1 {
            authority_pubkey,
            stake,
        }],
        threshold_stake,
    };

    verifier::verify_solana_finalized_burn_proof(&request)?;
    println!("ok");
    Ok(())
}

fn next_arg(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("missing value for {flag}"))
}

fn parse_hex32(input: &str, label: &str) -> Result<[u8; 32], String> {
    let trimmed = input.strip_prefix("0x").unwrap_or(input);
    if trimmed.len() != 64 {
        return Err(format!("{label} must be exactly 32 bytes of hex"));
    }
    let mut out = [0u8; 32];
    for (index, chunk) in trimmed.as_bytes().chunks_exact(2).enumerate() {
        let hi = decode_nibble(chunk[0], label)?;
        let lo = decode_nibble(chunk[1], label)?;
        out[index] = (hi << 4) | lo;
    }
    Ok(out)
}

fn decode_nibble(byte: u8, label: &str) -> Result<u8, String> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(format!("{label} contains non-hex characters")),
    }
}

fn parse_u64(input: &str, label: &str) -> Result<u64, String> {
    input
        .parse::<u64>()
        .map_err(|err| format!("failed to parse {label}: {err}"))
}

fn print_usage() {
    eprintln!(
        "usage: verify-proof --proof-file <path> --message-id-hex <0x..> --router-program-id-hex <0x..> --authority-hex <0x..> --stake <u64> --threshold-stake <u64>"
    );
}
