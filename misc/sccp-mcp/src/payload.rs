use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha3::{Digest, Keccak256};

pub const SCCP_DOMAIN_SORA: u32 = 0;
pub const SCCP_DOMAIN_ETH: u32 = 1;
pub const SCCP_DOMAIN_BSC: u32 = 2;
pub const SCCP_DOMAIN_SOL: u32 = 3;
pub const SCCP_DOMAIN_TON: u32 = 4;
pub const SCCP_DOMAIN_TRON: u32 = 5;

pub const SCCP_MSG_PREFIX_BURN_V1: &[u8] = b"sccp:burn:v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BurnPayload {
    pub version: u8,
    pub source_domain: u32,
    pub dest_domain: u32,
    pub nonce: u64,
    pub sora_asset_id: String,
    pub amount: String,
    pub recipient: String,
}

pub fn supported_domains() -> [u32; 6] {
    [
        SCCP_DOMAIN_SORA,
        SCCP_DOMAIN_ETH,
        SCCP_DOMAIN_BSC,
        SCCP_DOMAIN_SOL,
        SCCP_DOMAIN_TON,
        SCCP_DOMAIN_TRON,
    ]
}

pub fn parse_payload(value: &Value) -> AppResult<BurnPayload> {
    serde_json::from_value(value.clone())
        .map_err(|err| AppError::InvalidArgument(format!("invalid burn payload: {err}")))
}

pub fn validate_payload(payload: &BurnPayload) -> AppResult<Vec<String>> {
    let mut notes = Vec::new();

    if payload.version != 1 {
        return Err(AppError::InvalidArgument(format!(
            "payload.version must be 1, got {}",
            payload.version
        )));
    }

    if !supported_domains().contains(&payload.source_domain) {
        return Err(AppError::InvalidArgument(format!(
            "unsupported source_domain {}",
            payload.source_domain
        )));
    }

    if !supported_domains().contains(&payload.dest_domain) {
        return Err(AppError::InvalidArgument(format!(
            "unsupported dest_domain {}",
            payload.dest_domain
        )));
    }

    if payload.source_domain == payload.dest_domain {
        return Err(AppError::InvalidArgument(
            "source_domain and dest_domain must differ".to_owned(),
        ));
    }

    let recipient = parse_hex_fixed(&payload.recipient, 32, "recipient")?;
    if recipient.iter().all(|byte| *byte == 0) {
        return Err(AppError::InvalidArgument(
            "recipient cannot be all zero bytes".to_owned(),
        ));
    }

    let amount = parse_u128(&payload.amount)?;
    if amount == 0 {
        return Err(AppError::InvalidArgument(
            "amount must be greater than zero".to_owned(),
        ));
    }

    let _asset_id = parse_hex_fixed(&payload.sora_asset_id, 32, "sora_asset_id")?;

    if matches!(
        payload.dest_domain,
        SCCP_DOMAIN_ETH | SCCP_DOMAIN_BSC | SCCP_DOMAIN_TRON
    ) {
        if recipient[..12] != [0u8; 12] {
            return Err(AppError::InvalidArgument(
                "EVM recipient must be right-aligned in 32-byte field (first 12 bytes must be zero)"
                    .to_owned(),
            ));
        }
        notes.push("recipient is canonical for EVM destination".to_owned());
    }

    notes.push("payload validation passed".to_owned());
    Ok(notes)
}

pub fn message_id(payload: &BurnPayload) -> AppResult<String> {
    validate_payload(payload)?;
    let encoded = encode_burn_payload_scale(payload)?;
    let mut preimage = Vec::with_capacity(SCCP_MSG_PREFIX_BURN_V1.len() + encoded.len());
    preimage.extend_from_slice(SCCP_MSG_PREFIX_BURN_V1);
    preimage.extend_from_slice(&encoded);

    let digest = Keccak256::digest(&preimage);
    Ok(format!("0x{}", hex::encode(digest)))
}

pub fn encode_burn_payload_scale(payload: &BurnPayload) -> AppResult<Vec<u8>> {
    let asset_id = parse_hex_fixed(&payload.sora_asset_id, 32, "sora_asset_id")?;
    let recipient = parse_hex_fixed(&payload.recipient, 32, "recipient")?;
    let amount = parse_u128(&payload.amount)?;

    let mut out = Vec::with_capacity(1 + 4 + 4 + 8 + 32 + 16 + 32);
    out.push(payload.version);
    out.extend_from_slice(&payload.source_domain.to_le_bytes());
    out.extend_from_slice(&payload.dest_domain.to_le_bytes());
    out.extend_from_slice(&payload.nonce.to_le_bytes());
    out.extend_from_slice(&asset_id);
    out.extend_from_slice(&amount.to_le_bytes());
    out.extend_from_slice(&recipient);
    Ok(out)
}

pub fn parse_hex_fixed(input: &str, expected_len: usize, field: &str) -> AppResult<Vec<u8>> {
    let normalized = input.strip_prefix("0x").unwrap_or(input);
    let bytes = hex::decode(normalized).map_err(|err| {
        AppError::InvalidArgument(format!("{field} must be hex; decode failed: {err}"))
    })?;
    if bytes.len() != expected_len {
        return Err(AppError::InvalidArgument(format!(
            "{field} must be {expected_len} bytes, got {}",
            bytes.len()
        )));
    }
    Ok(bytes)
}

pub fn parse_u128(input: &str) -> AppResult<u128> {
    if let Some(hex_part) = input.strip_prefix("0x") {
        u128::from_str_radix(hex_part, 16)
            .map_err(|err| AppError::InvalidArgument(format!("invalid amount hex: {err}")))
    } else {
        input
            .parse::<u128>()
            .map_err(|err| AppError::InvalidArgument(format!("invalid amount decimal: {err}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_payload() -> BurnPayload {
        BurnPayload {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_ETH,
            nonce: 42,
            sora_asset_id: "0x1111111111111111111111111111111111111111111111111111111111111111"
                .to_owned(),
            amount: "1000000000000000000".to_owned(),
            recipient: "0x0000000000000000000000002222222222222222222222222222222222222222"
                .to_owned(),
        }
    }

    #[test]
    fn message_id_is_stable_for_known_payload() {
        let payload = sample_payload();
        let digest = message_id(&payload).expect("message id must be computed");
        assert_eq!(
            digest,
            "0x96f68e7cb4c8d01b237295459b956d4982e521232173d3dd1dc7e25cec46d208"
        );
    }

    #[test]
    fn evm_recipient_must_be_canonical() {
        let mut payload = sample_payload();
        payload.recipient =
            "0x0100000000000000000000002222222222222222222222222222222222222222".to_owned();
        let error = validate_payload(&payload).expect_err("payload must fail");
        assert!(
            error
                .to_string()
                .contains("EVM recipient must be right-aligned"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_u128_accepts_decimal_and_hex() {
        let decimal = parse_u128("255").expect("decimal must parse");
        let hex = parse_u128("0xff").expect("hex must parse");
        assert_eq!(decimal, 255);
        assert_eq!(hex, 255);
    }

    #[test]
    fn validate_rejects_same_source_and_destination() {
        let mut payload = sample_payload();
        payload.dest_domain = payload.source_domain;
        let error = validate_payload(&payload).expect_err("must fail for loopback");
        assert!(
            error.to_string().contains("must differ"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn validate_rejects_zero_amount() {
        let mut payload = sample_payload();
        payload.amount = "0".to_owned();
        let error = validate_payload(&payload).expect_err("must fail for zero amount");
        assert!(
            error.to_string().contains("greater than zero"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn non_evm_destination_does_not_require_right_aligned_recipient() {
        let mut payload = sample_payload();
        payload.dest_domain = SCCP_DOMAIN_SOL;
        payload.recipient =
            "0x0100000000000000000000002222222222222222222222222222222222222222".to_owned();
        let notes = validate_payload(&payload).expect("SOL payload should be valid");
        assert!(
            notes.iter().any(|note| note.contains("validation passed")),
            "expected validation note, got: {notes:?}"
        );
    }
}
