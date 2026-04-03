use crate::error::{AppError, AppResult};
use crate::payload::{parse_hex_fixed, parse_u128};
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
        name: "burn",
        call_index: 0,
        args: &[
            "asset_id: 0x<32-byte>",
            "amount: decimal-or-hex u128",
            "dest_domain: u32",
            "recipient: 0x<32-byte>",
        ],
    },
    SoraCallSpec {
        name: "mint_from_proof",
        call_index: 1,
        args: &["proof: 0x<bytes>"],
    },
    SoraCallSpec {
        name: "add_token_from_proof",
        call_index: 2,
        args: &["proof: 0x<bytes>"],
    },
    SoraCallSpec {
        name: "pause_token_from_proof",
        call_index: 3,
        args: &["proof: 0x<bytes>"],
    },
    SoraCallSpec {
        name: "resume_token_from_proof",
        call_index: 4,
        args: &["proof: 0x<bytes>"],
    },
];

pub fn supported_sora_calls() -> &'static [SoraCallSpec] {
    SPECS
}

pub fn encode_sora_call(
    call_name: &str,
    args: &Value,
    pallet_index: u8,
    _block_number_bytes: u8,
    max_call_bytes: usize,
    max_proof_bytes: usize,
) -> AppResult<EncodedSoraCall> {
    let spec = SPECS
        .iter()
        .find(|item| item.name == call_name)
        .ok_or_else(|| AppError::InvalidArgument(format!("unsupported SCCP call '{call_name}'")))?;

    let arg_bytes = match call_name {
        "burn" => {
            let mut out = Vec::new();
            out.extend_from_slice(&required_h256(args, "asset_id")?);
            push_u128(&mut out, required_amount(args, "amount")?);
            push_u32(&mut out, required_u32(args, "dest_domain")?);
            out.extend_from_slice(&required_h256(args, "recipient")?);
            out
        }
        "mint_from_proof"
        | "add_token_from_proof"
        | "pause_token_from_proof"
        | "resume_token_from_proof" => {
            let mut out = Vec::new();
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

fn required_u32(value: &Value, field: &str) -> AppResult<u32> {
    let raw = value
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| AppError::InvalidArgument(format!("missing integer field '{field}'")))?;
    u32::try_from(raw)
        .map_err(|_| AppError::InvalidArgument(format!("field '{field}' does not fit u32")))
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

fn required_hex_bytes(value: &Value, field: &str) -> AppResult<Vec<u8>> {
    let text = required_string(value, field)?;
    let normalized = text.strip_prefix("0x").unwrap_or(text);
    hex::decode(normalized)
        .map_err(|err| AppError::InvalidArgument(format!("field '{field}' must be hex: {err}")))
}

fn required_amount(value: &Value, field: &str) -> AppResult<u128> {
    let raw = required_string(value, field)?;
    parse_u128(raw)
}

fn push_u32(out: &mut Vec<u8>, value: u32) {
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

    #[test]
    fn encode_burn_call_data() {
        let args = json!({
            "asset_id": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "amount": "12345",
            "dest_domain": 1,
            "recipient": "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        });
        let encoded = encode_sora_call("burn", &args, 42, 4, 1024, 1024).expect("must encode");
        assert_eq!(encoded.call_index, 0);
        assert_eq!(encoded.call_data.len(), 86);
        assert_eq!(encoded.call_data[0], 42);
        assert_eq!(encoded.call_data[1], 0);
    }

    #[test]
    fn encode_proof_only_call_data() {
        let args = json!({
            "proof": "0x0102"
        });
        let encoded =
            encode_sora_call("add_token_from_proof", &args, 77, 4, 1024, 1024).expect("encode");
        assert_eq!(encoded.call_index, 2);
        assert_eq!(encoded.arg_bytes, vec![8, 1, 2]);
        assert_eq!(encoded.call_data, vec![77, 2, 8, 1, 2]);
    }

    #[test]
    fn proof_only_call_rejects_oversized_proof() {
        let args = json!({
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
    fn call_size_limit_is_enforced() {
        let args = json!({
            "proof": "0x0102030405"
        });
        let error = encode_sora_call("resume_token_from_proof", &args, 1, 4, 4, 1024)
            .expect_err("must fail for size");
        assert!(
            error.to_string().contains("max_call_bytes"),
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
}
