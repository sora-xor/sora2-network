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

pub fn double_map_key(
    pallet: &str,
    storage_item: &str,
    encoded_key1: &[u8],
    encoded_key2: &[u8],
) -> String {
    let mut key = storage_prefix(pallet, storage_item);
    key.extend_from_slice(&blake2_128_concat(encoded_key1));
    key.extend_from_slice(&blake2_128_concat(encoded_key2));
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

    if let Ok(decoded) = parse_token_state(&bytes, 4) {
        return Ok(Some(decoded));
    }

    let decoded = parse_token_state(&bytes, 8)?;
    Ok(Some(decoded))
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

pub fn decode_optional_bsc_params(hex_value: Option<&str>) -> AppResult<Option<Value>> {
    let Some(raw) = hex_value else {
        return Ok(None);
    };
    let bytes = decode_hex_bytes(raw)?;
    let mut offset = 0usize;
    let epoch_length = read_u64_le(&bytes, &mut offset)?;
    let confirmation_depth = read_u64_le(&bytes, &mut offset)?;
    let chain_id = read_u64_le(&bytes, &mut offset)?;
    let turn_length = read_u8(&bytes, &mut offset)?;
    Ok(Some(json!({
        "epoch_length": epoch_length,
        "confirmation_depth": confirmation_depth,
        "chain_id": chain_id,
        "turn_length": turn_length,
    })))
}

pub fn decode_optional_bsc_header(hex_value: Option<&str>) -> AppResult<Option<Value>> {
    let Some(raw) = hex_value else {
        return Ok(None);
    };
    let bytes = decode_hex_bytes(raw)?;
    let mut offset = 0usize;
    let hash = read_fixed::<32>(&bytes, &mut offset)?;
    let number = read_u64_le(&bytes, &mut offset)?;
    let state_root = read_fixed::<32>(&bytes, &mut offset)?;
    let signer = read_fixed::<20>(&bytes, &mut offset)?;

    Ok(Some(json!({
        "hash": format!("0x{}", hex::encode(hash)),
        "number": number,
        "state_root": format!("0x{}", hex::encode(state_root)),
        "signer": format!("0x{}", hex::encode(signer)),
    })))
}

pub fn decode_optional_tron_params(hex_value: Option<&str>) -> AppResult<Option<Value>> {
    let Some(raw) = hex_value else {
        return Ok(None);
    };
    let bytes = decode_hex_bytes(raw)?;
    let mut offset = 0usize;
    let address_prefix = read_u8(&bytes, &mut offset)?;
    let witness_count = read_u8(&bytes, &mut offset)?;
    let solidification_threshold = read_u8(&bytes, &mut offset)?;
    Ok(Some(json!({
        "address_prefix": address_prefix,
        "witness_count": witness_count,
        "solidification_threshold": solidification_threshold,
    })))
}

pub fn decode_optional_tron_header(hex_value: Option<&str>) -> AppResult<Option<Value>> {
    let Some(raw) = hex_value else {
        return Ok(None);
    };
    let bytes = decode_hex_bytes(raw)?;
    let mut offset = 0usize;
    let hash = read_fixed::<32>(&bytes, &mut offset)?;
    let number = read_u64_le(&bytes, &mut offset)?;
    let state_root = read_fixed::<32>(&bytes, &mut offset)?;
    let signer = read_fixed::<20>(&bytes, &mut offset)?;

    Ok(Some(json!({
        "hash": format!("0x{}", hex::encode(hash)),
        "number": number,
        "state_root": format!("0x{}", hex::encode(state_root)),
        "signer": format!("0x{}", hex::encode(signer)),
    })))
}

fn parse_token_state(bytes: &[u8], block_number_len: usize) -> AppResult<Value> {
    let mut offset = 0usize;
    let status = match read_u8(bytes, &mut offset)? {
        0 => "pending",
        1 => "active",
        2 => "removing",
        other => {
            return Err(AppError::Rpc(format!(
                "failed to decode TokenState: unknown status variant {other}"
            )))
        }
    };

    let outbound_enabled = read_bool(bytes, &mut offset)?;
    let inbound_enabled = read_bool(bytes, &mut offset)?;

    let has_until = read_u8(bytes, &mut offset)?;
    let inbound_enabled_until = match has_until {
        0 => None,
        1 => match block_number_len {
            4 => Some(read_u32_le(bytes, &mut offset)? as u64),
            8 => Some(read_u64_le(bytes, &mut offset)?),
            other => {
                return Err(AppError::Rpc(format!(
                    "unsupported block_number_len {other}"
                )))
            }
        },
        other => {
            return Err(AppError::Rpc(format!(
                "failed to decode Option<BlockNumber>: unexpected discriminant {other}"
            )))
        }
    };

    if offset != bytes.len() {
        return Err(AppError::Rpc(format!(
            "failed to decode TokenState: trailing bytes (decoded {offset}, total {})",
            bytes.len()
        )));
    }

    Ok(json!({
        "status": status,
        "outbound_enabled": outbound_enabled,
        "inbound_enabled": inbound_enabled,
        "inbound_enabled_until": inbound_enabled_until,
    }))
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

fn read_bool(bytes: &[u8], offset: &mut usize) -> AppResult<bool> {
    match read_u8(bytes, offset)? {
        0 => Ok(false),
        1 => Ok(true),
        other => Err(AppError::Rpc(format!("invalid SCALE bool byte {other}"))),
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

fn read_u32_le(bytes: &[u8], offset: &mut usize) -> AppResult<u32> {
    if *offset + 4 > bytes.len() {
        return Err(AppError::Rpc(
            "unexpected EOF while decoding u32".to_owned(),
        ));
    }
    let value = u32::from_le_bytes([
        bytes[*offset],
        bytes[*offset + 1],
        bytes[*offset + 2],
        bytes[*offset + 3],
    ]);
    *offset += 4;
    Ok(value)
}

fn read_u64_le(bytes: &[u8], offset: &mut usize) -> AppResult<u64> {
    if *offset + 8 > bytes.len() {
        return Err(AppError::Rpc(
            "unexpected EOF while decoding u64".to_owned(),
        ));
    }
    let value = u64::from_le_bytes([
        bytes[*offset],
        bytes[*offset + 1],
        bytes[*offset + 2],
        bytes[*offset + 3],
        bytes[*offset + 4],
        bytes[*offset + 5],
        bytes[*offset + 6],
        bytes[*offset + 7],
    ]);
    *offset += 8;
    Ok(value)
}

fn read_fixed<const N: usize>(bytes: &[u8], offset: &mut usize) -> AppResult<[u8; N]> {
    if *offset + N > bytes.len() {
        return Err(AppError::Rpc(format!(
            "unexpected EOF while decoding fixed bytes of length {N}"
        )));
    }
    let mut out = [0u8; N];
    out.copy_from_slice(&bytes[*offset..*offset + N]);
    *offset += N;
    Ok(out)
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
    fn decode_token_state_u32_block_number() {
        let raw = "0x0101000107000000";
        let decoded = decode_token_state(Some(raw))
            .expect("decode should succeed")
            .expect("value should exist");
        assert_eq!(
            decoded.get("status").and_then(Value::as_str),
            Some("active"),
            "decoded={decoded}"
        );
        assert_eq!(
            decoded.get("outbound_enabled").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            decoded.get("inbound_enabled").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            decoded.get("inbound_enabled_until").and_then(Value::as_u64),
            Some(7)
        );
    }

    #[test]
    fn decode_token_state_u64_block_number() {
        let mut bytes = vec![2u8, 0, 1, 1];
        bytes.extend_from_slice(&9u64.to_le_bytes());
        let raw = format!("0x{}", hex::encode(bytes));
        let decoded = decode_token_state(Some(&raw))
            .expect("decode should succeed")
            .expect("value should exist");
        assert_eq!(
            decoded.get("status").and_then(Value::as_str),
            Some("removing"),
            "decoded={decoded}"
        );
        assert_eq!(
            decoded.get("inbound_enabled_until").and_then(Value::as_u64),
            Some(9)
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
    fn decode_optional_bsc_params_happy_path() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&3u64.to_le_bytes());
        bytes.extend_from_slice(&4u64.to_le_bytes());
        bytes.extend_from_slice(&56u64.to_le_bytes());
        bytes.push(2u8);
        let raw = format!("0x{}", hex::encode(bytes));

        let decoded = decode_optional_bsc_params(Some(&raw))
            .expect("decode should succeed")
            .expect("value should be present");
        assert_eq!(decoded.get("epoch_length").and_then(Value::as_u64), Some(3));
        assert_eq!(
            decoded.get("confirmation_depth").and_then(Value::as_u64),
            Some(4)
        );
        assert_eq!(decoded.get("chain_id").and_then(Value::as_u64), Some(56));
        assert_eq!(decoded.get("turn_length").and_then(Value::as_u64), Some(2));
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
