use crate::error::{AppError, AppResult};
use blake2::{Blake2b512, Digest};
use serde_json::{json, Value};
use std::hash::Hasher;
use twox_hash::XxHash64;

pub fn twox_128(data: &[u8]) -> [u8; 16] {
    let mut out = [0u8; 16];

    let mut hasher0 = XxHash64::with_seed(0);
    hasher0.write(data);
    out[0..8].copy_from_slice(&hasher0.finish().to_le_bytes());

    let mut hasher1 = XxHash64::with_seed(1);
    hasher1.write(data);
    out[8..16].copy_from_slice(&hasher1.finish().to_le_bytes());

    out
}

pub fn blake2_128(data: &[u8]) -> [u8; 16] {
    let digest = Blake2b512::digest(data);
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[0..16]);
    out
}

pub fn storage_prefix(pallet: &str, storage_item: &str) -> Vec<u8> {
    let mut key = Vec::with_capacity(32);
    key.extend_from_slice(&twox_128(pallet.as_bytes()));
    key.extend_from_slice(&twox_128(storage_item.as_bytes()));
    key
}

pub fn blake2_128_concat(encoded_key: &[u8]) -> Vec<u8> {
    let mut key = Vec::with_capacity(16 + encoded_key.len());
    key.extend_from_slice(&blake2_128(encoded_key));
    key.extend_from_slice(encoded_key);
    key
}

pub fn map_key(pallet: &str, storage_item: &str, encoded_key: &[u8]) -> String {
    let mut key = storage_prefix(pallet, storage_item);
    key.extend_from_slice(&blake2_128_concat(encoded_key));
    format!("0x{}", hex::encode(key))
}

pub fn decode_storage_bool(hex_value: Option<&str>) -> AppResult<bool> {
    let Some(raw) = hex_value else {
        return Ok(false);
    };
    let bytes = decode_hex_bytes(raw)?;
    if bytes.is_empty() {
        return Err(AppError::Rpc(
            "failed to decode SCALE bool: empty bytes".to_owned(),
        ));
    }
    match bytes[0] {
        0 => Ok(false),
        1 => Ok(true),
        other => Err(AppError::Rpc(format!(
            "failed to decode SCALE bool: unexpected byte {other}"
        ))),
    }
}

pub fn decode_token_state(hex_value: Option<&str>) -> AppResult<Option<Value>> {
    let Some(raw) = hex_value else {
        return Ok(None);
    };
    let bytes = decode_hex_bytes(raw)?;
    Ok(Some(parse_token_state(&bytes)?))
}

pub fn decode_optional_bytes(hex_value: Option<&str>) -> AppResult<Option<String>> {
    let Some(raw) = hex_value else {
        return Ok(None);
    };
    let bytes = decode_hex_bytes(raw)?;
    let mut offset = 0usize;
    let len = decode_scale_compact_len(&bytes, &mut offset)?;
    if offset + len > bytes.len() {
        return Err(AppError::Rpc(
            "failed to decode SCALE Vec<u8>: length out of bounds".to_owned(),
        ));
    }
    let value = &bytes[offset..offset + len];
    Ok(Some(format!("0x{}", hex::encode(value))))
}

fn parse_token_state(bytes: &[u8]) -> AppResult<Value> {
    if bytes.len() != 1 {
        return Err(AppError::Rpc(format!(
            "failed to decode TokenState: expected 1 byte, got {}",
            bytes.len()
        )));
    }

    let status = match bytes[0] {
        0 => "active",
        1 => "paused",
        other => {
            return Err(AppError::Rpc(format!(
                "failed to decode TokenState: unknown status variant {other}"
            )))
        }
    };

    Ok(json!({ "status": status }))
}

fn decode_scale_compact_len(bytes: &[u8], offset: &mut usize) -> AppResult<usize> {
    let first = read_u8(bytes, offset)?;
    let mode = first & 0b11;

    match mode {
        0b00 => Ok((first >> 2) as usize),
        0b01 => {
            let second = read_u8(bytes, offset)?;
            let raw = u16::from_le_bytes([first, second]);
            Ok((raw >> 2) as usize)
        }
        0b10 => {
            let second = read_u8(bytes, offset)?;
            let third = read_u8(bytes, offset)?;
            let fourth = read_u8(bytes, offset)?;
            let raw = u32::from_le_bytes([first, second, third, fourth]);
            Ok((raw >> 2) as usize)
        }
        0b11 => {
            let byte_len = ((first >> 2) + 4) as usize;
            if *offset + byte_len > bytes.len() {
                return Err(AppError::Rpc(
                    "compact length with big-integer mode is out of bounds".to_owned(),
                ));
            }
            if byte_len > 8 {
                return Err(AppError::Rpc(
                    "compact length too large to fit usize on this decoder".to_owned(),
                ));
            }
            let mut raw: u64 = 0;
            for i in 0..byte_len {
                raw |= (bytes[*offset + i] as u64) << (8 * i);
            }
            *offset += byte_len;
            usize::try_from(raw)
                .map_err(|_| AppError::Rpc("compact length does not fit usize".to_owned()))
        }
        _ => unreachable!(),
    }
}

fn read_u8(bytes: &[u8], offset: &mut usize) -> AppResult<u8> {
    if *offset >= bytes.len() {
        return Err(AppError::Rpc(
            "unexpected EOF while decoding SCALE bytes".to_owned(),
        ));
    }
    let value = bytes[*offset];
    *offset += 1;
    Ok(value)
}

fn decode_hex_bytes(raw: &str) -> AppResult<Vec<u8>> {
    let normalized = raw.strip_prefix("0x").unwrap_or(raw);
    hex::decode(normalized)
        .map_err(|err| AppError::Rpc(format!("invalid hex storage bytes '{raw}': {err}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_token_state_active() {
        let raw = "0x00";
        let decoded = decode_token_state(Some(raw))
            .expect("decode should succeed")
            .expect("value should exist");
        assert_eq!(
            decoded.get("status").and_then(Value::as_str),
            Some("active"),
            "decoded={decoded}"
        );
    }

    #[test]
    fn decode_token_state_paused() {
        let raw = "0x01";
        let decoded = decode_token_state(Some(&raw))
            .expect("decode should succeed")
            .expect("value should exist");
        assert_eq!(
            decoded.get("status").and_then(Value::as_str),
            Some("paused"),
            "decoded={decoded}"
        );
    }

    #[test]
    fn decode_optional_bytes_happy_path() {
        let decoded =
            decode_optional_bytes(Some("0x08aabb")).expect("decode optional bytes should work");
        assert_eq!(decoded.as_deref(), Some("0xaabb"));
    }

    #[test]
    fn decode_optional_bytes_fails_on_out_of_bounds() {
        let error = decode_optional_bytes(Some("0x08aa")).expect_err("must fail");
        assert!(
            error.to_string().contains("length out of bounds"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn decode_storage_bool_variants() {
        assert_eq!(
            decode_storage_bool(None).expect("none should map to false"),
            false
        );
        assert_eq!(
            decode_storage_bool(Some("0x00")).expect("false should decode"),
            false
        );
        assert_eq!(
            decode_storage_bool(Some("0x01")).expect("true should decode"),
            true
        );
        let error = decode_storage_bool(Some("0x02")).expect_err("invalid bool must fail");
        assert!(
            error.to_string().contains("unexpected byte"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn compact_len_two_byte_mode_decodes() {
        let bytes = [0x19u8, 0x01u8]; // ((70 << 2) | 1) little-endian
        let mut offset = 0usize;
        let len = decode_scale_compact_len(&bytes, &mut offset).expect("must decode");
        assert_eq!(len, 70);
        assert_eq!(offset, 2);
    }
}
