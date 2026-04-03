use std::{env, fs, path::PathBuf};

#[cfg(not(test))]
use std::process;

use serde_json::{json, Value};
use tiny_keccak::{Hasher, Keccak};

const BURN_PREFIX: &[u8] = b"sccp:burn:v1";
const TOKEN_ADD_PREFIX: &[u8] = b"sccp:token:add:v1";
const TOKEN_PAUSE_PREFIX: &[u8] = b"sccp:token:pause:v1";
const TOKEN_RESUME_PREFIX: &[u8] = b"sccp:token:resume:v1";

const IX_MINT_FROM_PROOF: u8 = 10;
const IX_ADD_TOKEN_FROM_PROOF: u8 = 11;
const IX_PAUSE_TOKEN_FROM_PROOF: u8 = 12;
const IX_RESUME_TOKEN_FROM_PROOF: u8 = 13;

#[derive(Default)]
struct Args {
    bundle_json_file: Option<PathBuf>,
    bundle_norito_file: Option<PathBuf>,
    bundle_norito_hex: Option<String>,
    bundle_scale_file: Option<PathBuf>,
    bundle_scale_hex: Option<String>,
    local_domain: Option<u32>,
}

fn usage(message: &str) -> ! {
    #[cfg(test)]
    {
        panic!("{message}");
    }

    #[cfg(not(test))]
    {
        let mut lines = vec![
            "Usage:".to_owned(),
            "  cargo run --manifest-path tools/nexus-bundle-instruction-builder/Cargo.toml -- [--bundle-json-file <path>] [--bundle-norito-file <path> | --bundle-norito-hex 0x<...>] [--local-domain <u32>]".to_owned(),
            "".to_owned(),
            "When omitted, bundle paths and local domain are read from SCCP_SCENARIO_CONTEXT_FILE / SCCP_HUB_BUNDLE_* env vars.".to_owned(),
        ];
        if !message.is_empty() {
            lines.push("".to_owned());
            lines.push(message.to_owned());
        }
        eprintln!("{}", lines.join("\n"));
        process::exit(1);
    }
}

fn parse_args() -> Args {
    let mut out = Args::default();
    let argv: Vec<String> = env::args().collect();
    let mut i = 1usize;
    while i < argv.len() {
        match argv[i].as_str() {
            "--bundle-json-file" => {
                i += 1;
                let value = argv
                    .get(i)
                    .unwrap_or_else(|| usage("missing value for --bundle-json-file"));
                out.bundle_json_file = Some(PathBuf::from(value));
            }
            "--bundle-norito-file" => {
                i += 1;
                let value = argv
                    .get(i)
                    .unwrap_or_else(|| usage("missing value for --bundle-norito-file"));
                out.bundle_norito_file = Some(PathBuf::from(value));
            }
            "--bundle-norito-hex" => {
                i += 1;
                let value = argv
                    .get(i)
                    .unwrap_or_else(|| usage("missing value for --bundle-norito-hex"));
                out.bundle_norito_hex = Some(value.clone());
            }
            "--bundle-scale-file" => {
                i += 1;
                let value = argv
                    .get(i)
                    .unwrap_or_else(|| usage("missing value for --bundle-scale-file"));
                out.bundle_scale_file = Some(PathBuf::from(value));
            }
            "--bundle-scale-hex" => {
                i += 1;
                let value = argv
                    .get(i)
                    .unwrap_or_else(|| usage("missing value for --bundle-scale-hex"));
                out.bundle_scale_hex = Some(value.clone());
            }
            "--local-domain" => {
                i += 1;
                let value = argv
                    .get(i)
                    .unwrap_or_else(|| usage("missing value for --local-domain"));
                out.local_domain = Some(parse_u32_string(value, "local-domain"));
            }
            "--help" | "-h" => usage(""),
            other => usage(&format!("unknown or incomplete argument: {other}")),
        }
        i += 1;
    }
    out
}

fn parse_u32_string(value: &str, label: &str) -> u32 {
    value
        .parse::<u32>()
        .unwrap_or_else(|_| usage(&format!("{label} must be a u32 decimal string")))
}

fn parse_u32_value(value: &Value, label: &str) -> u32 {
    match value {
        Value::Number(number) => number
            .as_u64()
            .and_then(|v| u32::try_from(v).ok())
            .unwrap_or_else(|| usage(&format!("{label} must fit in u32"))),
        Value::String(text) => parse_u32_string(text, label),
        _ => usage(&format!("{label} must be a u32 integer or decimal string")),
    }
}

fn parse_u64_value(value: &Value, label: &str) -> u64 {
    match value {
        Value::Number(number) => number
            .as_u64()
            .unwrap_or_else(|| usage(&format!("{label} must be a non-negative integer"))),
        Value::String(text) => text
            .parse::<u64>()
            .unwrap_or_else(|_| usage(&format!("{label} must be a u64 decimal string"))),
        _ => usage(&format!("{label} must be a u64 integer or decimal string")),
    }
}

fn parse_u128_value(value: &Value, label: &str) -> u128 {
    match value {
        Value::Number(number) => number.as_u64().map(u128::from).unwrap_or_else(|| {
            usage(&format!(
                "{label} must be encoded as a decimal string once above u64::MAX"
            ))
        }),
        Value::String(text) => text
            .parse::<u128>()
            .unwrap_or_else(|_| usage(&format!("{label} must be a u128 decimal string"))),
        _ => usage(&format!("{label} must be a u128 integer or decimal string")),
    }
}

fn normalize_hex(input: &str, label: &str, expected_len: Option<usize>) -> String {
    let raw = input
        .strip_prefix("0x")
        .or_else(|| input.strip_prefix("0X"))
        .unwrap_or_else(|| usage(&format!("{label} must be a 0x-prefixed hex string")));
    if raw.len() % 2 != 0 {
        usage(&format!("{label} must have an even number of hex digits"));
    }
    if !raw.chars().all(|c| c.is_ascii_hexdigit()) {
        usage(&format!("{label} must contain only hex digits"));
    }
    if let Some(expected_bytes) = expected_len {
        if raw.len() != expected_bytes * 2 {
            usage(&format!("{label} must be {expected_bytes} bytes"));
        }
    }
    format!("0x{}", raw.to_ascii_lowercase())
}

fn hex_to_bytes(input: &str, label: &str, expected_len: Option<usize>) -> Vec<u8> {
    let normalized = normalize_hex(input, label, expected_len);
    hex::decode(&normalized[2..])
        .unwrap_or_else(|_| usage(&format!("failed to decode {label} as hex")))
}

fn value_as_object<'a>(value: &'a Value, label: &str) -> &'a serde_json::Map<String, Value> {
    value
        .as_object()
        .unwrap_or_else(|| usage(&format!("{label} must be an object")))
}

fn object_field<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
    label: &str,
) -> &'a Value {
    object
        .get(key)
        .unwrap_or_else(|| usage(&format!("missing {label}.{key}")))
}

fn field_hex32(object: &serde_json::Map<String, Value>, key: &str, label: &str) -> Vec<u8> {
    let value = object_field(object, key, label)
        .as_str()
        .unwrap_or_else(|| usage(&format!("{label}.{key} must be a hex string")));
    hex_to_bytes(value, &format!("{label}.{key}"), Some(32))
}

fn push_le_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_le_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_le_u128(out: &mut Vec<u8>, value: u128) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn encode_burn_payload(payload: &serde_json::Map<String, Value>) -> Vec<u8> {
    let mut out = Vec::with_capacity(97);
    out.push(parse_u32_value(
        object_field(payload, "version", "payload"),
        "payload.version",
    ) as u8);
    push_le_u32(
        &mut out,
        parse_u32_value(
            object_field(payload, "source_domain", "payload"),
            "payload.source_domain",
        ),
    );
    push_le_u32(
        &mut out,
        parse_u32_value(
            object_field(payload, "dest_domain", "payload"),
            "payload.dest_domain",
        ),
    );
    push_le_u64(
        &mut out,
        parse_u64_value(object_field(payload, "nonce", "payload"), "payload.nonce"),
    );
    out.extend_from_slice(&field_hex32(payload, "sora_asset_id", "payload"));
    push_le_u128(
        &mut out,
        parse_u128_value(object_field(payload, "amount", "payload"), "payload.amount"),
    );
    out.extend_from_slice(&field_hex32(payload, "recipient", "payload"));
    out
}

fn encode_token_add_payload(payload: &serde_json::Map<String, Value>) -> Vec<u8> {
    let mut out = Vec::with_capacity(110);
    out.push(parse_u32_value(
        object_field(payload, "version", "payload"),
        "payload.version",
    ) as u8);
    push_le_u32(
        &mut out,
        parse_u32_value(
            object_field(payload, "target_domain", "payload"),
            "payload.target_domain",
        ),
    );
    push_le_u64(
        &mut out,
        parse_u64_value(object_field(payload, "nonce", "payload"), "payload.nonce"),
    );
    out.extend_from_slice(&field_hex32(payload, "sora_asset_id", "payload"));
    out.push(parse_u32_value(
        object_field(payload, "decimals", "payload"),
        "payload.decimals",
    ) as u8);
    out.extend_from_slice(&field_hex32(payload, "name", "payload"));
    out.extend_from_slice(&field_hex32(payload, "symbol", "payload"));
    out
}

fn encode_token_control_payload(payload: &serde_json::Map<String, Value>) -> Vec<u8> {
    let mut out = Vec::with_capacity(45);
    out.push(parse_u32_value(
        object_field(payload, "version", "payload"),
        "payload.version",
    ) as u8);
    push_le_u32(
        &mut out,
        parse_u32_value(
            object_field(payload, "target_domain", "payload"),
            "payload.target_domain",
        ),
    );
    push_le_u64(
        &mut out,
        parse_u64_value(object_field(payload, "nonce", "payload"), "payload.nonce"),
    );
    out.extend_from_slice(&field_hex32(payload, "sora_asset_id", "payload"));
    out
}

fn keccak256(parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Keccak::v256();
    for part in parts {
        hasher.update(part);
    }
    let mut out = [0u8; 32];
    hasher.finalize(&mut out);
    out
}

fn hex_string(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}

fn encode_vec(bytes: &[u8]) -> Vec<u8> {
    let len = u32::try_from(bytes.len())
        .unwrap_or_else(|_| usage("vector is too large for Borsh encoding"));
    let mut out = Vec::with_capacity(4 + bytes.len());
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(bytes);
    out
}

fn build_instruction(tag: u8, parts: &[&[u8]]) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + parts.iter().map(|part| part.len()).sum::<usize>());
    out.push(tag);
    for part in parts {
        out.extend_from_slice(part);
    }
    out
}

fn build_instruction_from_bundle(
    bundle: &Value,
    bundle_proof_hex: &str,
    local_domain: u32,
) -> Value {
    let bundle_object = value_as_object(bundle, "bundle");
    let commitment = value_as_object(
        object_field(bundle_object, "commitment", "bundle"),
        "bundle.commitment",
    );
    let payload_value = object_field(bundle_object, "payload", "bundle");
    let proof_bytes = hex_to_bytes(bundle_proof_hex, "bundle_proof_hex", None);

    if payload_value.get("source_domain").is_some() && payload_value.get("dest_domain").is_some() {
        let payload = value_as_object(payload_value, "bundle.payload");
        let payload_bytes = encode_burn_payload(payload);
        let message_id = keccak256(&[BURN_PREFIX, &payload_bytes]);
        let commitment_message_id = field_hex32(commitment, "message_id", "bundle.commitment");
        if commitment_message_id != message_id {
            usage(
                "burn bundle commitment.message_id does not match the canonical payload message_id",
            );
        }
        let dest_domain = parse_u32_value(
            object_field(payload, "dest_domain", "payload"),
            "payload.dest_domain",
        );
        if dest_domain != local_domain {
            usage("burn bundle payload.dest_domain does not match localDomain");
        }
        let source_domain = parse_u32_value(
            object_field(payload, "source_domain", "payload"),
            "payload.source_domain",
        );
        let mut source_domain_bytes = Vec::with_capacity(4);
        push_le_u32(&mut source_domain_bytes, source_domain);
        let instruction_bytes = build_instruction(
            IX_MINT_FROM_PROOF,
            &[
                &source_domain_bytes,
                &encode_vec(&payload_bytes),
                &encode_vec(&proof_bytes),
            ],
        );
        return json!({
            "ok": true,
            "instruction": "MintFromProof",
            "instruction_discriminant": IX_MINT_FROM_PROOF,
            "instruction_data_hex": hex_string(&instruction_bytes),
            "payload_hex": hex_string(&payload_bytes),
            "proof_hex": hex_string(&proof_bytes),
            "message_id": hex_string(&message_id),
            "source_domain": source_domain,
        });
    }

    let payload = value_as_object(payload_value, "bundle.payload");
    if payload.len() != 1 {
        usage("governance bundle payload must be an externally-tagged object with one variant");
    }
    let (variant, variant_payload_value) = payload.iter().next().expect("checked len");
    let variant_payload = value_as_object(variant_payload_value, "bundle.payload.variant");
    let target_domain = parse_u32_value(
        object_field(variant_payload, "target_domain", "payload"),
        "payload.target_domain",
    );
    if target_domain != local_domain {
        usage("governance bundle payload.target_domain does not match localDomain");
    }

    match variant.as_str() {
        "Add" => {
            let payload_bytes = encode_token_add_payload(variant_payload);
            let message_id = keccak256(&[TOKEN_ADD_PREFIX, &payload_bytes]);
            if field_hex32(commitment, "message_id", "bundle.commitment") != message_id {
                usage("governance add bundle commitment.message_id does not match the canonical payload message_id");
            }
            let instruction_bytes = build_instruction(
                IX_ADD_TOKEN_FROM_PROOF,
                &[&encode_vec(&payload_bytes), &encode_vec(&proof_bytes)],
            );
            json!({
                "ok": true,
                "instruction": "AddTokenFromProof",
                "instruction_discriminant": IX_ADD_TOKEN_FROM_PROOF,
                "instruction_data_hex": hex_string(&instruction_bytes),
                "payload_hex": hex_string(&payload_bytes),
                "proof_hex": hex_string(&proof_bytes),
                "message_id": hex_string(&message_id),
            })
        }
        "Pause" => {
            let payload_bytes = encode_token_control_payload(variant_payload);
            let message_id = keccak256(&[TOKEN_PAUSE_PREFIX, &payload_bytes]);
            if field_hex32(commitment, "message_id", "bundle.commitment") != message_id {
                usage("governance pause bundle commitment.message_id does not match the canonical payload message_id");
            }
            let instruction_bytes = build_instruction(
                IX_PAUSE_TOKEN_FROM_PROOF,
                &[&encode_vec(&payload_bytes), &encode_vec(&proof_bytes)],
            );
            json!({
                "ok": true,
                "instruction": "PauseTokenFromProof",
                "instruction_discriminant": IX_PAUSE_TOKEN_FROM_PROOF,
                "instruction_data_hex": hex_string(&instruction_bytes),
                "payload_hex": hex_string(&payload_bytes),
                "proof_hex": hex_string(&proof_bytes),
                "message_id": hex_string(&message_id),
            })
        }
        "Resume" => {
            let payload_bytes = encode_token_control_payload(variant_payload);
            let message_id = keccak256(&[TOKEN_RESUME_PREFIX, &payload_bytes]);
            if field_hex32(commitment, "message_id", "bundle.commitment") != message_id {
                usage("governance resume bundle commitment.message_id does not match the canonical payload message_id");
            }
            let instruction_bytes = build_instruction(
                IX_RESUME_TOKEN_FROM_PROOF,
                &[&encode_vec(&payload_bytes), &encode_vec(&proof_bytes)],
            );
            json!({
                "ok": true,
                "instruction": "ResumeTokenFromProof",
                "instruction_discriminant": IX_RESUME_TOKEN_FROM_PROOF,
                "instruction_data_hex": hex_string(&instruction_bytes),
                "payload_hex": hex_string(&payload_bytes),
                "proof_hex": hex_string(&proof_bytes),
                "message_id": hex_string(&message_id),
            })
        }
        _ => usage("unsupported governance bundle variant; expected Add, Pause, or Resume"),
    }
}

fn read_context() -> Option<Value> {
    let path = env::var("SCCP_SCENARIO_CONTEXT_FILE").ok()?;
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn main() {
    let args = parse_args();
    let context = read_context();

    let bundle_json_file = args
        .bundle_json_file
        .or_else(|| {
            context
                .as_ref()
                .and_then(|value| value.get("hub_bundle_json_path"))
                .and_then(Value::as_str)
                .map(PathBuf::from)
        })
        .or_else(|| {
            env::var("SCCP_HUB_BUNDLE_JSON_PATH")
                .ok()
                .map(PathBuf::from)
        })
        .unwrap_or_else(|| {
            usage("missing --bundle-json-file and no hub bundle JSON path in scenario context")
        });

    let bundle_proof_hex = args
        .bundle_norito_hex
        .or_else(|| {
            context
                .as_ref()
                .and_then(|value| value.get("hub_bundle_norito_hex"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
        .or_else(|| env::var("SCCP_HUB_BUNDLE_NORITO_HEX").ok())
        .or_else(|| {
            args.bundle_norito_file.or_else(|| {
                context
                    .as_ref()
                    .and_then(|value| value.get("hub_bundle_norito_path"))
                    .and_then(Value::as_str)
                    .map(PathBuf::from)
            })
            .or_else(|| env::var("SCCP_HUB_BUNDLE_NORITO_PATH").ok().map(PathBuf::from))
            .map(|path| format!("0x{}", hex::encode(fs::read(path).unwrap_or_else(|_| usage("failed to read bundle norito file")))))
        })
        .or_else(|| {
            args.bundle_scale_hex
        .or_else(|| context.as_ref().and_then(|value| value.get("hub_bundle_scale_hex")).and_then(Value::as_str).map(ToOwned::to_owned))
        .or_else(|| env::var("SCCP_HUB_BUNDLE_SCALE_HEX").ok())
        .or_else(|| {
            args.bundle_scale_file.or_else(|| {
                context
                    .as_ref()
                    .and_then(|value| value.get("hub_bundle_scale_path"))
                    .and_then(Value::as_str)
                    .map(PathBuf::from)
            })
            .or_else(|| env::var("SCCP_HUB_BUNDLE_SCALE_PATH").ok().map(PathBuf::from))
            .map(|path| format!("0x{}", hex::encode(fs::read(path).unwrap_or_else(|_| usage("failed to read legacy bundle scale file")))))
        })
        })
        .unwrap_or_else(|| usage("missing --bundle-norito-file / --bundle-norito-hex and no hub bundle proof bytes in scenario context"));

    let local_domain = args
        .local_domain
        .or_else(|| {
            context
                .as_ref()
                .and_then(|value| value.get("dest_domain"))
                .map(|value| parse_u32_value(value, "dest_domain"))
        })
        .or_else(|| {
            env::var("SCCP_DEST_DOMAIN")
                .ok()
                .map(|value| parse_u32_string(&value, "SCCP_DEST_DOMAIN"))
        })
        .unwrap_or_else(|| usage("missing --local-domain and SCCP_DEST_DOMAIN"));

    let bundle_raw = fs::read_to_string(bundle_json_file)
        .unwrap_or_else(|_| usage("failed to read bundle json file"));
    let bundle_json: Value = serde_json::from_str(&bundle_raw)
        .unwrap_or_else(|_| usage("bundle json file did not contain valid JSON"));

    let output = build_instruction_from_bundle(&bundle_json, &bundle_proof_hex, local_domain);
    println!(
        "{}",
        serde_json::to_string(&output).expect("serializing output should succeed")
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn burn_bundle(local_domain: u32) -> Value {
        let payload = json!({
            "version": 1,
            "source_domain": 0,
            "dest_domain": local_domain,
            "nonce": "7",
            "sora_asset_id": format!("0x{}", "11".repeat(32)),
            "amount": "42",
            "recipient": format!("0x{}", "22".repeat(32)),
        });
        let payload_bytes = encode_burn_payload(value_as_object(&payload, "payload"));
        let message_id = hex_string(&keccak256(&[BURN_PREFIX, &payload_bytes]));
        json!({
            "version": 1,
            "commitment_root": format!("0x{}", "44".repeat(32)),
            "commitment": {
                "version": 1,
                "kind": "Burn",
                "target_domain": local_domain,
                "message_id": message_id,
                "payload_hash": format!("0x{}", "55".repeat(32)),
                "parliament_certificate_hash": null,
            },
            "merkle_proof": { "steps": [] },
            "payload": payload,
            "finality_proof": "0x1234",
        })
    }

    fn pause_bundle(local_domain: u32) -> Value {
        let variant_payload = json!({
            "version": 1,
            "target_domain": local_domain,
            "nonce": "9",
            "sora_asset_id": format!("0x{}", "33".repeat(32)),
        });
        let payload_bytes =
            encode_token_control_payload(value_as_object(&variant_payload, "payload"));
        let message_id = hex_string(&keccak256(&[TOKEN_PAUSE_PREFIX, &payload_bytes]));
        json!({
            "version": 1,
            "commitment_root": format!("0x{}", "66".repeat(32)),
            "commitment": {
                "version": 1,
                "kind": "TokenPause",
                "target_domain": local_domain,
                "message_id": message_id,
                "payload_hash": format!("0x{}", "77".repeat(32)),
                "parliament_certificate_hash": format!("0x{}", "88".repeat(32)),
            },
            "merkle_proof": { "steps": [] },
            "payload": {
                "Pause": variant_payload
            },
            "parliament_certificate": "0xbeef",
            "finality_proof": "0xfeed",
        })
    }

    #[test]
    fn builds_mint_instruction_from_burn_bundle() {
        let output = build_instruction_from_bundle(&burn_bundle(3), "0xc0de", 3);
        assert_eq!(output["instruction"], "MintFromProof");
        assert_eq!(output["instruction_discriminant"], IX_MINT_FROM_PROOF);
        assert_eq!(output["proof_hex"], "0xc0de");
    }

    #[test]
    fn builds_pause_instruction_from_governance_bundle() {
        let output = build_instruction_from_bundle(&pause_bundle(3), "0x1234", 3);
        assert_eq!(output["instruction"], "PauseTokenFromProof");
        assert_eq!(
            output["instruction_discriminant"],
            IX_PAUSE_TOKEN_FROM_PROOF
        );
    }

    #[test]
    fn rejects_wrong_local_domain() {
        let bundle = burn_bundle(1);
        let result =
            std::panic::catch_unwind(|| build_instruction_from_bundle(&bundle, "0x1234", 3));
        assert!(result.is_err());
    }

    #[test]
    fn rejects_governance_message_id_mismatch() {
        let mut bundle = pause_bundle(3);
        bundle["commitment"]["message_id"] = json!(format!("0x{}", "aa".repeat(32)));
        let result =
            std::panic::catch_unwind(|| build_instruction_from_bundle(&bundle, "0x1234", 3));
        assert!(result.is_err());
    }
}
