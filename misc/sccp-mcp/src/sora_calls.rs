use crate::error::{AppError, AppResult};
use crate::payload::{
    encode_burn_payload_scale, parse_hex_fixed, parse_payload, parse_u128, BurnPayload,
};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct SoraCallSpec {
    pub name: &'static str,
    pub call_index: u8,
    pub args: &'static [&'static str],
}

#[derive(Debug, Clone)]
pub struct EncodedSoraCall {
    pub name: &'static str,
    pub pallet_index: u8,
    pub call_index: u8,
    pub arg_bytes: Vec<u8>,
    pub call_data: Vec<u8>,
}

const SPECS: &[SoraCallSpec] = &[
    SoraCallSpec {
        name: "add_token",
        call_index: 0,
        args: &["asset_id: 0x<32-byte>"],
    },
    SoraCallSpec {
        name: "set_remote_token",
        call_index: 1,
        args: &[
            "asset_id: 0x<32-byte>",
            "domain_id: u32",
            "remote_token_id: 0x<bytes>",
        ],
    },
    SoraCallSpec {
        name: "activate_token",
        call_index: 2,
        args: &["asset_id: 0x<32-byte>"],
    },
    SoraCallSpec {
        name: "remove_token",
        call_index: 3,
        args: &["asset_id: 0x<32-byte>"],
    },
    SoraCallSpec {
        name: "finalize_remove",
        call_index: 4,
        args: &["asset_id: 0x<32-byte>"],
    },
    SoraCallSpec {
        name: "set_inbound_grace_period",
        call_index: 5,
        args: &["blocks: u64 (encoded using profile block_number_bytes)"],
    },
    SoraCallSpec {
        name: "set_required_domains",
        call_index: 6,
        args: &["domains: [u32, ...]"],
    },
    SoraCallSpec {
        name: "burn",
        call_index: 7,
        args: &[
            "asset_id: 0x<32-byte>",
            "amount: decimal-or-hex u128",
            "dest_domain: u32",
            "recipient: 0x<32-byte>",
        ],
    },
    SoraCallSpec {
        name: "mint_from_proof",
        call_index: 8,
        args: &[
            "source_domain: u32",
            "payload: BurnPayloadV1 JSON",
            "proof: 0x<bytes>",
        ],
    },
    SoraCallSpec {
        name: "set_inbound_domain_paused",
        call_index: 9,
        args: &["domain_id: u32", "paused: bool"],
    },
    SoraCallSpec {
        name: "invalidate_inbound_message",
        call_index: 10,
        args: &["source_domain: u32", "message_id: 0x<32-byte>"],
    },
    SoraCallSpec {
        name: "clear_invalidated_inbound_message",
        call_index: 11,
        args: &["source_domain: u32", "message_id: 0x<32-byte>"],
    },
    SoraCallSpec {
        name: "set_domain_endpoint",
        call_index: 12,
        args: &["domain_id: u32", "endpoint_id: 0x<bytes>"],
    },
    SoraCallSpec {
        name: "clear_domain_endpoint",
        call_index: 13,
        args: &["domain_id: u32"],
    },
    SoraCallSpec {
        name: "pause_token",
        call_index: 14,
        args: &["asset_id: 0x<32-byte>"],
    },
    SoraCallSpec {
        name: "resume_token",
        call_index: 15,
        args: &["asset_id: 0x<32-byte>"],
    },
    SoraCallSpec {
        name: "init_bsc_light_client",
        call_index: 16,
        args: &[
            "checkpoint_header_rlp: 0x<bytes>",
            "validators: [0x<20-byte>, ...]",
            "epoch_length: u64",
            "confirmation_depth: u64",
            "chain_id: u64",
            "turn_length: u8",
        ],
    },
    SoraCallSpec {
        name: "submit_bsc_header",
        call_index: 17,
        args: &["header_rlp: 0x<bytes>"],
    },
    SoraCallSpec {
        name: "set_bsc_validators",
        call_index: 18,
        args: &["validators: [0x<20-byte>, ...]"],
    },
    SoraCallSpec {
        name: "set_outbound_domain_paused",
        call_index: 19,
        args: &["domain_id: u32", "paused: bool"],
    },
    SoraCallSpec {
        name: "attest_burn",
        call_index: 21,
        args: &[
            "source_domain: u32",
            "payload: BurnPayloadV1 JSON",
            "proof: 0x<bytes>",
        ],
    },
    SoraCallSpec {
        name: "set_inbound_finality_mode",
        call_index: 22,
        args: &["domain_id: u32", "mode: enum-string-or-index"],
    },
    SoraCallSpec {
        name: "init_tron_light_client",
        call_index: 23,
        args: &[
            "checkpoint_raw_data: 0x<bytes>",
            "checkpoint_witness_signature: 0x<65-byte>",
            "witnesses: [0x<20-byte>, ...]",
            "address_prefix: u8",
        ],
    },
    SoraCallSpec {
        name: "submit_tron_header",
        call_index: 24,
        args: &["raw_data: 0x<bytes>", "witness_signature: 0x<65-byte>"],
    },
    SoraCallSpec {
        name: "set_tron_witnesses",
        call_index: 25,
        args: &["witnesses: [0x<20-byte>, ...]"],
    },
];

pub fn supported_sora_calls() -> &'static [SoraCallSpec] {
    SPECS
}

pub fn encode_sora_call(
    call_name: &str,
    args: &Value,
    pallet_index: u8,
    block_number_bytes: u8,
    max_call_bytes: usize,
    max_proof_bytes: usize,
) -> AppResult<EncodedSoraCall> {
    let spec = SPECS
        .iter()
        .find(|item| item.name == call_name)
        .ok_or_else(|| AppError::InvalidArgument(format!("unsupported SCCP call '{call_name}'")))?;

    let arg_bytes = match call_name {
        "add_token" => {
            let mut out = Vec::new();
            out.extend_from_slice(&required_h256(args, "asset_id")?);
            out
        }
        "set_remote_token" => {
            let mut out = Vec::new();
            out.extend_from_slice(&required_h256(args, "asset_id")?);
            push_u32(&mut out, required_u32(args, "domain_id")?);
            push_vec_bytes(&mut out, &required_hex_bytes(args, "remote_token_id")?)?;
            out
        }
        "activate_token" | "remove_token" | "finalize_remove" | "pause_token"
        | "resume_token" => {
            let mut out = Vec::new();
            out.extend_from_slice(&required_h256(args, "asset_id")?);
            out
        }
        "set_inbound_grace_period" => {
            let mut out = Vec::new();
            let blocks = required_u64(args, "blocks")?;
            match block_number_bytes {
                4 => {
                    let blocks_u32 = u32::try_from(blocks).map_err(|_| {
                        AppError::InvalidArgument(
                            "blocks does not fit into 4-byte block number".to_owned(),
                        )
                    })?;
                    push_u32(&mut out, blocks_u32);
                }
                8 => push_u64(&mut out, blocks),
                _ => {
                    return Err(AppError::InvalidArgument(format!(
                        "unsupported block_number_bytes {}, expected 4 or 8",
                        block_number_bytes
                    )))
                }
            }
            out
        }
        "set_required_domains" => {
            let mut out = Vec::new();
            let domains = required_u32_array(args, "domains")?;
            push_compact_len(&mut out, domains.len())?;
            for domain in domains {
                push_u32(&mut out, domain);
            }
            out
        }
        "burn" => {
            let mut out = Vec::new();
            out.extend_from_slice(&required_h256(args, "asset_id")?);
            let amount = required_amount(args, "amount")?;
            push_u128(&mut out, amount);
            push_u32(&mut out, required_u32(args, "dest_domain")?);
            out.extend_from_slice(&required_h256(args, "recipient")?);
            out
        }
        "mint_from_proof" | "attest_burn" => {
            let mut out = Vec::new();
            push_u32(&mut out, required_u32(args, "source_domain")?);

            let payload_value = required_value(args, "payload")?;
            let payload: BurnPayload = parse_payload(payload_value)?;
            out.extend_from_slice(&encode_burn_payload_scale(&payload)?);

            let proof = required_hex_bytes(args, "proof")?;
            if proof.len() > max_proof_bytes {
                return Err(AppError::InvalidArgument(format!(
                    "proof bytes exceed max_proof_bytes: {} > {}",
                    proof.len(),
                    max_proof_bytes
                )));
            }
            push_vec_bytes(&mut out, &proof)?;
            out
        }
        "set_inbound_domain_paused" | "set_outbound_domain_paused" => {
            let mut out = Vec::new();
            push_u32(&mut out, required_u32(args, "domain_id")?);
            push_bool(&mut out, required_bool(args, "paused")?);
            out
        }
        "invalidate_inbound_message" | "clear_invalidated_inbound_message" => {
            let mut out = Vec::new();
            push_u32(&mut out, required_u32(args, "source_domain")?);
            out.extend_from_slice(&required_h256(args, "message_id")?);
            out
        }
        "set_domain_endpoint" => {
            let mut out = Vec::new();
            push_u32(&mut out, required_u32(args, "domain_id")?);
            push_vec_bytes(&mut out, &required_hex_bytes(args, "endpoint_id")?)?;
            out
        }
        "clear_domain_endpoint" => {
            let mut out = Vec::new();
            push_u32(&mut out, required_u32(args, "domain_id")?);
            out
        }
        "init_bsc_light_client" => {
            let mut out = Vec::new();
            push_vec_bytes(
                &mut out,
                &required_hex_bytes(args, "checkpoint_header_rlp")?,
            )?;
            push_vec_h160(&mut out, &required_h160_array(args, "validators")?)?;
            push_u64(&mut out, required_u64(args, "epoch_length")?);
            push_u64(&mut out, required_u64(args, "confirmation_depth")?);
            push_u64(&mut out, required_u64(args, "chain_id")?);
            push_u8(&mut out, required_u8(args, "turn_length")?);
            out
        }
        "submit_bsc_header" => {
            let mut out = Vec::new();
            push_vec_bytes(&mut out, &required_hex_bytes(args, "header_rlp")?)?;
            out
        }
        "set_bsc_validators" => {
            let mut out = Vec::new();
            push_vec_h160(&mut out, &required_h160_array(args, "validators")?)?;
            out
        }
        "set_inbound_finality_mode" => {
            let mut out = Vec::new();
            push_u32(&mut out, required_u32(args, "domain_id")?);
            push_u8(&mut out, required_finality_mode(args, "mode")?);
            out
        }
        "init_tron_light_client" => {
            let mut out = Vec::new();
            push_vec_bytes(&mut out, &required_hex_bytes(args, "checkpoint_raw_data")?)?;
            push_vec_bytes(
                &mut out,
                &required_hex_fixed_bytes(args, "checkpoint_witness_signature", 65)?,
            )?;
            push_vec_h160(&mut out, &required_h160_array(args, "witnesses")?)?;
            push_u8(&mut out, required_u8(args, "address_prefix")?);
            out
        }
        "submit_tron_header" => {
            let mut out = Vec::new();
            push_vec_bytes(&mut out, &required_hex_bytes(args, "raw_data")?)?;
            push_vec_bytes(
                &mut out,
                &required_hex_fixed_bytes(args, "witness_signature", 65)?,
            )?;
            out
        }
        "set_tron_witnesses" => {
            let mut out = Vec::new();
            push_vec_h160(&mut out, &required_h160_array(args, "witnesses")?)?;
            out
        }
        other => {
            return Err(AppError::InvalidArgument(format!(
                "call '{other}' is not implemented in encoder"
            )))
        }
    };

    let mut call_data = Vec::with_capacity(2 + arg_bytes.len());
    call_data.push(pallet_index);
    call_data.push(spec.call_index);
    call_data.extend_from_slice(&arg_bytes);

    if call_data.len() > max_call_bytes {
        return Err(AppError::InvalidArgument(format!(
            "call bytes exceed max_call_bytes: {} > {}",
            call_data.len(),
            max_call_bytes
        )));
    }

    Ok(EncodedSoraCall {
        name: spec.name,
        pallet_index,
        call_index: spec.call_index,
        arg_bytes,
        call_data,
    })
}

pub fn encode_attester_quorum_proof(signatures: &[Vec<u8>], version: u8) -> AppResult<Vec<u8>> {
    if version != 1 {
        return Err(AppError::InvalidArgument(
            "attester quorum proof version must currently be 1".to_owned(),
        ));
    }

    let mut out = Vec::new();
    out.push(version);
    push_compact_len(&mut out, signatures.len())?;
    for (idx, sig) in signatures.iter().enumerate() {
        if sig.len() != 65 {
            return Err(AppError::InvalidArgument(format!(
                "signature at index {idx} must be 65 bytes, got {}",
                sig.len()
            )));
        }
        out.extend_from_slice(sig);
    }
    Ok(out)
}

fn required_value<'a>(value: &'a Value, field: &str) -> AppResult<&'a Value> {
    value
        .get(field)
        .ok_or_else(|| AppError::InvalidArgument(format!("missing field '{field}'")))
}

fn required_u32(value: &Value, field: &str) -> AppResult<u32> {
    let raw = value
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| AppError::InvalidArgument(format!("missing integer field '{field}'")))?;
    u32::try_from(raw)
        .map_err(|_| AppError::InvalidArgument(format!("field '{field}' does not fit u32")))
}

fn required_u64(value: &Value, field: &str) -> AppResult<u64> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| AppError::InvalidArgument(format!("missing integer field '{field}'")))
}

fn required_u8(value: &Value, field: &str) -> AppResult<u8> {
    let raw = value
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| AppError::InvalidArgument(format!("missing integer field '{field}'")))?;
    u8::try_from(raw)
        .map_err(|_| AppError::InvalidArgument(format!("field '{field}' does not fit u8")))
}

fn required_bool(value: &Value, field: &str) -> AppResult<bool> {
    value
        .get(field)
        .and_then(Value::as_bool)
        .ok_or_else(|| AppError::InvalidArgument(format!("missing bool field '{field}'")))
}

fn required_string<'a>(value: &'a Value, field: &str) -> AppResult<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::InvalidArgument(format!("missing string field '{field}'")))
}

fn required_h256(value: &Value, field: &str) -> AppResult<[u8; 32]> {
    let bytes = parse_hex_fixed(required_string(value, field)?, 32, field)?;
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn required_hex_fixed_bytes(value: &Value, field: &str, len: usize) -> AppResult<Vec<u8>> {
    parse_hex_fixed(required_string(value, field)?, len, field)
}

fn required_hex_bytes(value: &Value, field: &str) -> AppResult<Vec<u8>> {
    let text = required_string(value, field)?;
    let normalized = text.strip_prefix("0x").unwrap_or(text);
    hex::decode(normalized)
        .map_err(|err| AppError::InvalidArgument(format!("field '{field}' must be hex: {err}")))
}

fn required_h160_array(value: &Value, field: &str) -> AppResult<Vec<[u8; 20]>> {
    let list = value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| AppError::InvalidArgument(format!("field '{field}' must be array")))?;
    let mut out = Vec::with_capacity(list.len());
    for (idx, item) in list.iter().enumerate() {
        let text = item.as_str().ok_or_else(|| {
            AppError::InvalidArgument(format!("field '{field}[{idx}]' must be string"))
        })?;
        let bytes = parse_hex_fixed(text, 20, &format!("{field}[{idx}]"))?;
        let mut fixed = [0u8; 20];
        fixed.copy_from_slice(&bytes);
        out.push(fixed);
    }
    Ok(out)
}

fn required_u32_array(value: &Value, field: &str) -> AppResult<Vec<u32>> {
    let list = value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| AppError::InvalidArgument(format!("field '{field}' must be array")))?;
    let mut out = Vec::with_capacity(list.len());
    for (idx, item) in list.iter().enumerate() {
        let raw = item.as_u64().ok_or_else(|| {
            AppError::InvalidArgument(format!("field '{field}[{idx}]' must be integer"))
        })?;
        let parsed = u32::try_from(raw).map_err(|_| {
            AppError::InvalidArgument(format!("field '{field}[{idx}]' does not fit u32"))
        })?;
        out.push(parsed);
    }
    Ok(out)
}

fn required_amount(value: &Value, field: &str) -> AppResult<u128> {
    let raw = required_string(value, field)?;
    parse_u128(raw)
}

fn required_finality_mode(value: &Value, field: &str) -> AppResult<u8> {
    let raw = required_value(value, field)?;
    if let Some(index) = raw.as_u64() {
        let parsed = u8::try_from(index)
            .map_err(|_| AppError::InvalidArgument("mode does not fit u8".to_owned()))?;
        if matches!(parsed, 0 | 2 | 4 | 5 | 6 | 7 | 8) {
            return Ok(parsed);
        }
        return Err(AppError::InvalidArgument(
            "supported mode indexes are 0, 2, 4, 5, 6, 7, 8".to_owned(),
        ));
    }

    let text = raw.as_str().ok_or_else(|| {
        AppError::InvalidArgument("mode must be string or integer enum index".to_owned())
    })?;

    match text {
        "disabled" | "Disabled" => Ok(0),
        "bsc_light_client" | "BscLightClient" => Ok(2),
        "eth_beacon_light_client" | "EthBeaconLightClient" => Ok(4),
        "solana_light_client" | "SolanaLightClient" => Ok(5),
        "ton_light_client" | "TonLightClient" => Ok(6),
        "tron_light_client" | "TronLightClient" => Ok(7),
        "substrate_light_client" | "SubstrateLightClient" => Ok(8),
        _ => Err(AppError::InvalidArgument(format!(
            "unknown finality mode '{text}'"
        ))),
    }
}

fn push_bool(out: &mut Vec<u8>, value: bool) {
    out.push(if value { 1 } else { 0 });
}

fn push_u8(out: &mut Vec<u8>, value: u8) {
    out.push(value);
}

fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_u128(out: &mut Vec<u8>, value: u128) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_vec_bytes(out: &mut Vec<u8>, bytes: &[u8]) -> AppResult<()> {
    push_compact_len(out, bytes.len())?;
    out.extend_from_slice(bytes);
    Ok(())
}

fn push_vec_h160(out: &mut Vec<u8>, values: &[[u8; 20]]) -> AppResult<()> {
    push_compact_len(out, values.len())?;
    for value in values {
        out.extend_from_slice(value);
    }
    Ok(())
}

fn push_compact_len(out: &mut Vec<u8>, len: usize) -> AppResult<()> {
    if len <= 0b0011_1111 {
        out.push((len as u8) << 2);
        return Ok(());
    }

    if len <= 0b0011_1111_1111_1111 {
        let encoded = ((len as u16) << 2) | 0b01;
        out.extend_from_slice(&encoded.to_le_bytes());
        return Ok(());
    }

    if len <= 0x3fff_ffff {
        let encoded = ((len as u32) << 2) | 0b10;
        out.extend_from_slice(&encoded.to_le_bytes());
        return Ok(());
    }

    Err(AppError::InvalidArgument(
        "SCALE compact lengths > 0x3fff_ffff are not supported in this encoder".to_owned(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_inbound_payload_json() -> Value {
        json!({
            "version": 1,
            "source_domain": 1,
            "dest_domain": 0,
            "nonce": 7,
            "sora_asset_id": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "amount": "12345",
            "recipient": "0x0101010101010101010101010101010101010101010101010101010101010101"
        })
    }

    #[test]
    fn encode_add_token_call_data() {
        let args = json!({
            "asset_id": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        });
        let encoded = encode_sora_call("add_token", &args, 42, 4, 1024, 1024).expect("must encode");
        assert_eq!(encoded.call_index, 0);
        assert_eq!(encoded.call_data.len(), 34);
        assert_eq!(encoded.call_data[0], 42);
        assert_eq!(encoded.call_data[1], 0);
    }

    #[test]
    fn encode_attester_quorum_v1() {
        let signatures = vec![vec![0x11u8; 65], vec![0x22u8; 65]];
        let proof = encode_attester_quorum_proof(&signatures, 1).expect("must encode");
        assert_eq!(proof[0], 1);
        assert_eq!(proof[1], 8); // compact-encoded len=2
        assert_eq!(proof.len(), 1 + 1 + 65 + 65);
    }

    #[test]
    fn finality_mode_parses_string() {
        let args = json!({
            "domain_id": 1,
            "mode": "EthBeaconLightClient",
        });
        let encoded = encode_sora_call("set_inbound_finality_mode", &args, 1, 4, 1024, 1024)
            .expect("must encode");
        assert_eq!(encoded.call_index, 22);
        assert_eq!(encoded.arg_bytes[4], 4);
    }

    #[test]
    fn set_inbound_grace_period_encodes_with_u32_block_number() {
        let args = json!({
            "blocks": 17
        });
        let encoded = encode_sora_call("set_inbound_grace_period", &args, 9, 4, 1024, 1024)
            .expect("must encode");
        assert_eq!(encoded.call_index, 5);
        assert_eq!(encoded.arg_bytes, 17u32.to_le_bytes());
        assert_eq!(encoded.call_data[0], 9);
        assert_eq!(encoded.call_data[1], 5);
    }

    #[test]
    fn set_inbound_grace_period_encodes_with_u64_block_number() {
        let args = json!({
            "blocks": 17
        });
        let encoded = encode_sora_call("set_inbound_grace_period", &args, 9, 8, 1024, 1024)
            .expect("must encode");
        assert_eq!(encoded.arg_bytes, 17u64.to_le_bytes());
    }

    #[test]
    fn mint_from_proof_rejects_oversized_proof() {
        let args = json!({
            "source_domain": 1,
            "payload": sample_inbound_payload_json(),
            "proof": "0x0102"
        });
        let error = encode_sora_call("mint_from_proof", &args, 1, 4, 2048, 1)
            .expect_err("proof must exceed max");
        assert!(
            error.to_string().contains("max_proof_bytes"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn finality_mode_rejects_unknown_name() {
        let args = json!({
            "domain_id": 1,
            "mode": "not_a_mode"
        });
        let error = encode_sora_call("set_inbound_finality_mode", &args, 1, 4, 1024, 1024)
            .expect_err("unknown mode should fail");
        assert!(
            error.to_string().contains("unknown finality mode"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn call_size_limit_is_enforced() {
        let args = json!({
            "asset_id": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        });
        let error =
            encode_sora_call("add_token", &args, 1, 4, 10, 1024).expect_err("must fail for size");
        assert!(
            error.to_string().contains("max_call_bytes"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn attester_proof_rejects_bad_signature_length() {
        let signatures = vec![vec![0x11u8; 64]];
        let error =
            encode_attester_quorum_proof(&signatures, 1).expect_err("64-byte sig should fail");
        assert!(
            error.to_string().contains("must be 65 bytes"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn grace_period_rejects_unsupported_block_number_size() {
        let args = json!({
            "blocks": 1
        });
        let error = encode_sora_call("set_inbound_grace_period", &args, 1, 3, 1024, 1024)
            .expect_err("unsupported block number bytes should fail");
        assert!(
            error.to_string().contains("expected 4 or 8"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn unsupported_call_name_fails() {
        let args = json!({});
        let error = encode_sora_call("nonexistent", &args, 1, 4, 1024, 1024)
            .expect_err("unsupported calls must fail");
        assert!(
            error.to_string().contains("unsupported SCCP call"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn grace_period_u32_rejects_value_overflow() {
        let args = json!({
            "blocks": (u32::MAX as u64) + 1
        });
        let error = encode_sora_call("set_inbound_grace_period", &args, 1, 4, 1024, 1024)
            .expect_err("overflow should fail");
        assert!(
            error
                .to_string()
                .contains("does not fit into 4-byte block number"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn set_required_domains_uses_compact_length_encoding() {
        let args = json!({
            "domains": [1, 2, 3]
        });
        let encoded =
            encode_sora_call("set_required_domains", &args, 1, 4, 1024, 1024).expect("must work");
        assert_eq!(encoded.call_index, 6);
        assert_eq!(encoded.arg_bytes[0], 12); // compact-encoded vec length 3 => 0x0c
    }
}
