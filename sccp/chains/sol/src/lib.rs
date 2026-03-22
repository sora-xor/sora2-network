#![no_std]

extern crate alloc;

use tiny_keccak::{Hasher, Keccak};

pub const SCCP_DOMAIN_SORA: u32 = 0;
pub const SCCP_DOMAIN_ETH: u32 = 1;
pub const SCCP_DOMAIN_BSC: u32 = 2;
pub const SCCP_DOMAIN_SOL: u32 = 3;
pub const SCCP_DOMAIN_TON: u32 = 4;
pub const SCCP_DOMAIN_TRON: u32 = 5;

pub const SCCP_MSG_PREFIX_BURN_V1: &[u8] = b"sccp:burn:v1";
pub const SCCP_MSG_PREFIX_ATTEST_V1: &[u8] = b"sccp:attest:v1";
pub const SCCP_MSG_PREFIX_TOKEN_ADD_V1: &[u8] = b"sccp:token:add:v1";
pub const SCCP_MSG_PREFIX_TOKEN_PAUSE_V1: &[u8] = b"sccp:token:pause:v1";
pub const SCCP_MSG_PREFIX_TOKEN_RESUME_V1: &[u8] = b"sccp:token:resume:v1";

pub type H256 = [u8; 32];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BurnPayloadV1 {
    pub version: u8,
    pub source_domain: u32,
    pub dest_domain: u32,
    pub nonce: u64,
    pub sora_asset_id: [u8; 32],
    pub amount: u128,
    pub recipient: [u8; 32],
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TokenAddPayloadV1 {
    pub version: u8,
    pub target_domain: u32,
    pub nonce: u64,
    pub sora_asset_id: [u8; 32],
    pub decimals: u8,
    pub name: [u8; 32],
    pub symbol: [u8; 32],
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TokenControlPayloadV1 {
    pub version: u8,
    pub target_domain: u32,
    pub nonce: u64,
    pub sora_asset_id: [u8; 32],
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CodecError {
    InvalidLength,
}

impl BurnPayloadV1 {
    pub const ENCODED_LEN: usize = 97;

    /// SCALE encoding for fixed-width primitives (matches Substrate `parity-scale-codec`).
    pub fn encode_scale(&self) -> [u8; Self::ENCODED_LEN] {
        let mut out = [0u8; Self::ENCODED_LEN];
        out[0] = self.version;
        out[1..5].copy_from_slice(&self.source_domain.to_le_bytes());
        out[5..9].copy_from_slice(&self.dest_domain.to_le_bytes());
        out[9..17].copy_from_slice(&self.nonce.to_le_bytes());
        out[17..49].copy_from_slice(&self.sora_asset_id);
        out[49..65].copy_from_slice(&self.amount.to_le_bytes());
        out[65..97].copy_from_slice(&self.recipient);
        out
    }
}

impl TokenAddPayloadV1 {
    pub const ENCODED_LEN: usize = 110;

    pub fn encode_scale(&self) -> [u8; Self::ENCODED_LEN] {
        let mut out = [0u8; Self::ENCODED_LEN];
        out[0] = self.version;
        out[1..5].copy_from_slice(&self.target_domain.to_le_bytes());
        out[5..13].copy_from_slice(&self.nonce.to_le_bytes());
        out[13..45].copy_from_slice(&self.sora_asset_id);
        out[45] = self.decimals;
        out[46..78].copy_from_slice(&self.name);
        out[78..110].copy_from_slice(&self.symbol);
        out
    }
}

impl TokenControlPayloadV1 {
    pub const ENCODED_LEN: usize = 45;

    pub fn encode_scale(&self) -> [u8; Self::ENCODED_LEN] {
        let mut out = [0u8; Self::ENCODED_LEN];
        out[0] = self.version;
        out[1..5].copy_from_slice(&self.target_domain.to_le_bytes());
        out[5..13].copy_from_slice(&self.nonce.to_le_bytes());
        out[13..45].copy_from_slice(&self.sora_asset_id);
        out
    }
}

pub fn decode_burn_payload_v1(payload_scale: &[u8]) -> Result<BurnPayloadV1, CodecError> {
    if payload_scale.len() != BurnPayloadV1::ENCODED_LEN {
        return Err(CodecError::InvalidLength);
    }
    let mut b4 = [0u8; 4];
    let mut b8 = [0u8; 8];
    let mut b16 = [0u8; 16];

    b4.copy_from_slice(&payload_scale[1..5]);
    let source_domain = u32::from_le_bytes(b4);

    b4.copy_from_slice(&payload_scale[5..9]);
    let dest_domain = u32::from_le_bytes(b4);

    b8.copy_from_slice(&payload_scale[9..17]);
    let nonce = u64::from_le_bytes(b8);

    let mut sora_asset_id = [0u8; 32];
    sora_asset_id.copy_from_slice(&payload_scale[17..49]);

    b16.copy_from_slice(&payload_scale[49..65]);
    let amount = u128::from_le_bytes(b16);

    let mut recipient = [0u8; 32];
    recipient.copy_from_slice(&payload_scale[65..97]);

    Ok(BurnPayloadV1 {
        version: payload_scale[0],
        source_domain,
        dest_domain,
        nonce,
        sora_asset_id,
        amount,
        recipient,
    })
}

pub fn decode_token_add_payload_v1(payload_scale: &[u8]) -> Result<TokenAddPayloadV1, CodecError> {
    if payload_scale.len() != TokenAddPayloadV1::ENCODED_LEN {
        return Err(CodecError::InvalidLength);
    }

    let mut b4 = [0u8; 4];
    let mut b8 = [0u8; 8];
    b4.copy_from_slice(&payload_scale[1..5]);
    let target_domain = u32::from_le_bytes(b4);

    b8.copy_from_slice(&payload_scale[5..13]);
    let nonce = u64::from_le_bytes(b8);

    let mut sora_asset_id = [0u8; 32];
    sora_asset_id.copy_from_slice(&payload_scale[13..45]);

    let decimals = payload_scale[45];

    let mut name = [0u8; 32];
    name.copy_from_slice(&payload_scale[46..78]);

    let mut symbol = [0u8; 32];
    symbol.copy_from_slice(&payload_scale[78..110]);

    Ok(TokenAddPayloadV1 {
        version: payload_scale[0],
        target_domain,
        nonce,
        sora_asset_id,
        decimals,
        name,
        symbol,
    })
}

pub fn decode_token_control_payload_v1(
    payload_scale: &[u8],
) -> Result<TokenControlPayloadV1, CodecError> {
    if payload_scale.len() != TokenControlPayloadV1::ENCODED_LEN {
        return Err(CodecError::InvalidLength);
    }

    let mut b4 = [0u8; 4];
    let mut b8 = [0u8; 8];
    b4.copy_from_slice(&payload_scale[1..5]);
    let target_domain = u32::from_le_bytes(b4);

    b8.copy_from_slice(&payload_scale[5..13]);
    let nonce = u64::from_le_bytes(b8);

    let mut sora_asset_id = [0u8; 32];
    sora_asset_id.copy_from_slice(&payload_scale[13..45]);

    Ok(TokenControlPayloadV1 {
        version: payload_scale[0],
        target_domain,
        nonce,
        sora_asset_id,
    })
}

pub fn burn_message_id(payload_scale: &[u8]) -> H256 {
    prefixed_keccak(SCCP_MSG_PREFIX_BURN_V1, payload_scale)
}

pub fn token_add_message_id(payload_scale: &[u8]) -> H256 {
    prefixed_keccak(SCCP_MSG_PREFIX_TOKEN_ADD_V1, payload_scale)
}

pub fn token_pause_message_id(payload_scale: &[u8]) -> H256 {
    prefixed_keccak(SCCP_MSG_PREFIX_TOKEN_PAUSE_V1, payload_scale)
}

pub fn token_resume_message_id(payload_scale: &[u8]) -> H256 {
    prefixed_keccak(SCCP_MSG_PREFIX_TOKEN_RESUME_V1, payload_scale)
}

fn prefixed_keccak(prefix: &[u8], payload_scale: &[u8]) -> H256 {
    let mut k = Keccak::v256();
    k.update(prefix);
    k.update(payload_scale);
    let mut out = [0u8; 32];
    k.finalize(&mut out);
    out
}

pub fn attest_hash(message_id: &H256) -> H256 {
    let mut k = Keccak::v256();
    k.update(SCCP_MSG_PREFIX_ATTEST_V1);
    k.update(message_id);
    let mut out = [0u8; 32];
    k.finalize(&mut out);
    out
}

pub const SCCP_SOL_BURN_PROOF_INPUTS_SCHEMA_V1: &str = "sccp-sol-burn-proof-inputs/v1";
pub const SOLANA_FINALIZED_BURN_PROOF_VERSION_V1: u8 = 1;
pub const SOLANA_BURN_RECORD_ACCOUNT_DATA_LEN: usize =
    1 + 1 + 32 + BurnPayloadV1::ENCODED_LEN + 32 + 32 + 8;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SolanaBurnRecordAccountV1 {
    pub version: u8,
    pub bump: u8,
    pub message_id: H256,
    pub payload: [u8; BurnPayloadV1::ENCODED_LEN],
    pub sender: [u8; 32],
    pub mint: [u8; 32],
    pub slot: u64,
}

impl SolanaBurnRecordAccountV1 {
    pub fn encode_account_data(&self) -> [u8; SOLANA_BURN_RECORD_ACCOUNT_DATA_LEN] {
        let mut out = [0u8; SOLANA_BURN_RECORD_ACCOUNT_DATA_LEN];
        out[0] = self.version;
        out[1] = self.bump;
        out[2..34].copy_from_slice(&self.message_id);
        out[34..34 + BurnPayloadV1::ENCODED_LEN].copy_from_slice(&self.payload);
        out[131..163].copy_from_slice(&self.sender);
        out[163..195].copy_from_slice(&self.mint);
        out[195..203].copy_from_slice(&self.slot.to_le_bytes());
        out
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SolanaFinalizedBurnPublicInputsV1 {
    pub message_id: H256,
    pub finalized_slot: u64,
    pub finalized_bank_hash: H256,
    pub finalized_slot_hash: H256,
    pub router_program_id: [u8; 32],
    pub burn_record_pda: [u8; 32],
    pub burn_record_owner: [u8; 32],
    pub burn_record_data_hash: H256,
}

pub fn decode_solana_burn_record_account_v1(
    account_data: &[u8],
) -> Result<SolanaBurnRecordAccountV1, CodecError> {
    if account_data.len() != SOLANA_BURN_RECORD_ACCOUNT_DATA_LEN {
        return Err(CodecError::InvalidLength);
    }

    let mut message_id = [0u8; 32];
    message_id.copy_from_slice(&account_data[2..34]);

    let mut payload = [0u8; BurnPayloadV1::ENCODED_LEN];
    payload.copy_from_slice(&account_data[34..34 + BurnPayloadV1::ENCODED_LEN]);

    let mut sender = [0u8; 32];
    sender.copy_from_slice(&account_data[131..163]);

    let mut mint = [0u8; 32];
    mint.copy_from_slice(&account_data[163..195]);

    let mut slot_bytes = [0u8; 8];
    slot_bytes.copy_from_slice(&account_data[195..203]);

    Ok(SolanaBurnRecordAccountV1 {
        version: account_data[0],
        bump: account_data[1],
        message_id,
        payload,
        sender,
        mint,
        slot: u64::from_le_bytes(slot_bytes),
    })
}

pub fn solana_burn_record_data_hash(account_data: &[u8]) -> H256 {
    let mut k = Keccak::v256();
    k.update(account_data);
    let mut out = [0u8; 32];
    k.finalize(&mut out);
    out
}

pub fn solana_burn_record_account_hash(record: &SolanaBurnRecordAccountV1) -> H256 {
    solana_burn_record_data_hash(&record.encode_account_data())
}

pub fn solana_finalized_burn_public_inputs(
    finalized_bank_hash: H256,
    finalized_slot_hash: H256,
    router_program_id: [u8; 32],
    burn_record_pda: [u8; 32],
    burn_record_owner: [u8; 32],
    burn_record: &SolanaBurnRecordAccountV1,
) -> SolanaFinalizedBurnPublicInputsV1 {
    SolanaFinalizedBurnPublicInputsV1 {
        message_id: burn_record.message_id,
        finalized_slot: burn_record.slot,
        finalized_bank_hash,
        finalized_slot_hash,
        router_program_id,
        burn_record_pda,
        burn_record_owner,
        burn_record_data_hash: solana_burn_record_account_hash(burn_record),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;
    use hex::FromHex;
    use parity_scale_codec::Encode;
    use tiny_keccak::{Hasher, Keccak};

    #[derive(Encode)]
    struct RefPayload {
        version: u8,
        source_domain: u32,
        dest_domain: u32,
        nonce: u64,
        sora_asset_id: [u8; 32],
        amount: u128,
        recipient: [u8; 32],
    }

    #[derive(Encode)]
    struct RefTokenAddPayload {
        version: u8,
        target_domain: u32,
        nonce: u64,
        sora_asset_id: [u8; 32],
        decimals: u8,
        name: [u8; 32],
        symbol: [u8; 32],
    }

    #[derive(Encode)]
    struct RefTokenControlPayload {
        version: u8,
        target_domain: u32,
        nonce: u64,
        sora_asset_id: [u8; 32],
    }

    #[test]
    fn manual_scale_encoding_matches_parity_scale_codec() {
        let p = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 777,
            sora_asset_id: [0x11u8; 32],
            amount: 10,
            recipient: [0x22u8; 32],
        };

        let manual = p.encode_scale();
        let ref_bytes = RefPayload {
            version: p.version,
            source_domain: p.source_domain,
            dest_domain: p.dest_domain,
            nonce: p.nonce,
            sora_asset_id: p.sora_asset_id,
            amount: p.amount,
            recipient: p.recipient,
        }
        .encode();

        assert_eq!(ref_bytes.len(), BurnPayloadV1::ENCODED_LEN);
        assert_eq!(manual.as_slice(), ref_bytes.as_slice());
    }

    #[test]
    fn governance_payloads_match_parity_scale_codec() {
        let add = TokenAddPayloadV1 {
            version: 1,
            target_domain: SCCP_DOMAIN_SOL,
            nonce: 9,
            sora_asset_id: [0x11u8; 32],
            decimals: 18,
            name: [0x22u8; 32],
            symbol: [0x33u8; 32],
        };
        let add_manual = add.encode_scale();
        let add_ref = RefTokenAddPayload {
            version: add.version,
            target_domain: add.target_domain,
            nonce: add.nonce,
            sora_asset_id: add.sora_asset_id,
            decimals: add.decimals,
            name: add.name,
            symbol: add.symbol,
        }
        .encode();
        assert_eq!(add_ref.len(), TokenAddPayloadV1::ENCODED_LEN);
        assert_eq!(add_manual.as_slice(), add_ref.as_slice());
        assert_eq!(decode_token_add_payload_v1(&add_manual).unwrap(), add);

        let control = TokenControlPayloadV1 {
            version: 1,
            target_domain: SCCP_DOMAIN_SOL,
            nonce: 10,
            sora_asset_id: [0x44u8; 32],
        };
        let control_manual = control.encode_scale();
        let control_ref = RefTokenControlPayload {
            version: control.version,
            target_domain: control.target_domain,
            nonce: control.nonce,
            sora_asset_id: control.sora_asset_id,
        }
        .encode();
        assert_eq!(control_ref.len(), TokenControlPayloadV1::ENCODED_LEN);
        assert_eq!(control_manual.as_slice(), control_ref.as_slice());
        assert_eq!(
            decode_token_control_payload_v1(&control_manual).unwrap(),
            control
        );
    }

    #[test]
    fn fixtures_match_reference_vectors() {
        // Generated with a parity-scale-codec + tiny-keccak reference (see SPEC.md).
        let expected_payload = Vec::from_hex(
            "010100000000000000090300000000000011111111111111111111111111111111111111111111111111111111111111110a0000000000000000000000000000002222222222222222222222222222222222222222222222222222222222222222",
        )
        .unwrap();
        let expected_message_id =
            Vec::from_hex("f3cac8c5acfb0670a24e9ffeab7e409a9d54d1dc5e6dbaf0ee986462fe1ffb3a")
                .unwrap();

        let p = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 777,
            sora_asset_id: [0x11u8; 32],
            amount: 10,
            recipient: [0x22u8; 32],
        };

        let payload = p.encode_scale();
        assert_eq!(payload.as_slice(), expected_payload.as_slice());

        let decoded = decode_burn_payload_v1(&payload).unwrap();
        assert_eq!(decoded, p);

        let msg_id = burn_message_id(&payload);
        assert_eq!(msg_id.as_slice(), expected_message_id.as_slice());
    }

    #[test]
    fn decode_rejects_incorrect_payload_length() {
        let short = [0u8; BurnPayloadV1::ENCODED_LEN - 1];
        let long = [0u8; BurnPayloadV1::ENCODED_LEN + 1];
        let empty: [u8; 0] = [];
        assert_eq!(
            decode_burn_payload_v1(&short),
            Err(CodecError::InvalidLength)
        );
        assert_eq!(
            decode_burn_payload_v1(&long),
            Err(CodecError::InvalidLength)
        );
        assert_eq!(
            decode_burn_payload_v1(&empty),
            Err(CodecError::InvalidLength)
        );
    }

    #[test]
    fn solana_burn_record_account_round_trips_and_hash_changes_with_state() {
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SOL,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 99,
            sora_asset_id: [0x11; 32],
            amount: 42,
            recipient: [0x22; 32],
        };
        let payload_bytes = payload.encode_scale();
        let message_id = burn_message_id(&payload_bytes);
        let record = SolanaBurnRecordAccountV1 {
            version: 1,
            bump: 7,
            message_id,
            payload: payload_bytes,
            sender: [0x33; 32],
            mint: [0x44; 32],
            slot: 12345,
        };

        let encoded = record.encode_account_data();
        assert_eq!(encoded.len(), SOLANA_BURN_RECORD_ACCOUNT_DATA_LEN);

        let decoded = decode_solana_burn_record_account_v1(&encoded).unwrap();
        assert_eq!(decoded, record);

        let hash_a = solana_burn_record_account_hash(&record);
        let mut changed = record;
        changed.slot = changed.slot.saturating_add(1);
        let hash_b = solana_burn_record_account_hash(&changed);
        assert_ne!(hash_a, hash_b);
    }

    #[test]
    fn solana_finalized_burn_public_inputs_bind_record_message_and_hash() {
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SOL,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 7,
            sora_asset_id: [0xaa; 32],
            amount: 5,
            recipient: [0xbb; 32],
        };
        let payload_bytes = payload.encode_scale();
        let record = SolanaBurnRecordAccountV1 {
            version: 1,
            bump: 1,
            message_id: burn_message_id(&payload_bytes),
            payload: payload_bytes,
            sender: [0xcc; 32],
            mint: [0xdd; 32],
            slot: 77,
        };

        let public_inputs = solana_finalized_burn_public_inputs(
            [0x11; 32],
            [0x12; 32],
            [0x10; 32],
            [0x20; 32],
            [0x30; 32],
            &record,
        );

        assert_eq!(public_inputs.message_id, record.message_id);
        assert_eq!(public_inputs.finalized_slot, record.slot);
        assert_eq!(public_inputs.finalized_bank_hash, [0x11; 32]);
        assert_eq!(public_inputs.finalized_slot_hash, [0x12; 32]);
        assert_eq!(
            public_inputs.burn_record_data_hash,
            solana_burn_record_account_hash(&record)
        );
    }

    #[test]
    fn encode_decode_round_trip_with_extreme_values() {
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_TRON,
            dest_domain: SCCP_DOMAIN_TON,
            nonce: u64::MAX,
            sora_asset_id: [0xffu8; 32],
            amount: u128::MAX,
            recipient: [0xaau8; 32],
        };
        let encoded = payload.encode_scale();
        let decoded = decode_burn_payload_v1(&encoded).expect("payload must decode");
        assert_eq!(decoded, payload);
    }

    #[test]
    fn message_id_changes_when_payload_changes() {
        let mut payload_a = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 1,
            sora_asset_id: [0x11u8; 32],
            amount: 10,
            recipient: [0x22u8; 32],
        };
        let message_a = burn_message_id(&payload_a.encode_scale());

        payload_a.nonce = 2;
        let message_b = burn_message_id(&payload_a.encode_scale());

        assert_ne!(message_a, message_b);
    }

    #[test]
    fn burn_message_id_changes_when_version_changes() {
        let payload_v1 = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 1,
            sora_asset_id: [0x11u8; 32],
            amount: 10,
            recipient: [0x22u8; 32],
        };
        let mut payload_v2 = payload_v1;
        payload_v2.version = 2;

        let msg_v1 = burn_message_id(&payload_v1.encode_scale());
        let msg_v2 = burn_message_id(&payload_v2.encode_scale());
        assert_ne!(msg_v1, msg_v2);
    }

    #[test]
    fn attest_hash_is_domain_separated_from_burn_prefix() {
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 777,
            sora_asset_id: [0x11u8; 32],
            amount: 10,
            recipient: [0x22u8; 32],
        };
        let message_id = burn_message_id(&payload.encode_scale());
        let burn_of_message_id = burn_message_id(&message_id);
        let attested = attest_hash(&message_id);

        assert_ne!(attested, burn_of_message_id);
    }

    #[test]
    fn decode_interprets_fixed_width_fields_as_little_endian() {
        let mut payload = [0u8; BurnPayloadV1::ENCODED_LEN];
        payload[0] = 1;
        payload[1..5].copy_from_slice(&0x1122_3344u32.to_le_bytes());
        payload[5..9].copy_from_slice(&0x5566_7788u32.to_le_bytes());
        payload[9..17].copy_from_slice(&0x0102_0304_0506_0708u64.to_le_bytes());
        payload[17..49].copy_from_slice(&[0xabu8; 32]);
        payload[49..65]
            .copy_from_slice(&0x0102_0304_0506_0708_090a_0b0c_0d0e_0f10u128.to_le_bytes());
        payload[65..97].copy_from_slice(&[0xcdu8; 32]);

        let decoded = decode_burn_payload_v1(&payload).expect("payload must decode");
        assert_eq!(decoded.version, 1);
        assert_eq!(decoded.source_domain, 0x1122_3344);
        assert_eq!(decoded.dest_domain, 0x5566_7788);
        assert_eq!(decoded.nonce, 0x0102_0304_0506_0708);
        assert_eq!(decoded.sora_asset_id, [0xabu8; 32]);
        assert_eq!(
            decoded.amount,
            0x0102_0304_0506_0708_090a_0b0c_0d0e_0f10u128
        );
        assert_eq!(decoded.recipient, [0xcdu8; 32]);
    }

    #[test]
    fn encode_writes_fields_at_expected_offsets() {
        let payload = BurnPayloadV1 {
            version: 0x7f,
            source_domain: 0x1122_3344,
            dest_domain: 0x5566_7788,
            nonce: 0x0102_0304_0506_0708,
            sora_asset_id: [0xabu8; 32],
            amount: 0x0102_0304_0506_0708_090a_0b0c_0d0e_0f10u128,
            recipient: [0xcdu8; 32],
        };

        let encoded = payload.encode_scale();
        assert_eq!(encoded.len(), BurnPayloadV1::ENCODED_LEN);
        assert_eq!(encoded[0], payload.version);
        assert_eq!(&encoded[1..5], &payload.source_domain.to_le_bytes());
        assert_eq!(&encoded[5..9], &payload.dest_domain.to_le_bytes());
        assert_eq!(&encoded[9..17], &payload.nonce.to_le_bytes());
        assert_eq!(&encoded[17..49], &payload.sora_asset_id);
        assert_eq!(&encoded[49..65], &payload.amount.to_le_bytes());
        assert_eq!(&encoded[65..97], &payload.recipient);
    }

    #[test]
    fn decode_accepts_maximum_fixed_width_values() {
        let mut payload = [0u8; BurnPayloadV1::ENCODED_LEN];
        payload[0] = 0xff;
        payload[1..5].copy_from_slice(&u32::MAX.to_le_bytes());
        payload[5..9].copy_from_slice(&u32::MAX.to_le_bytes());
        payload[9..17].copy_from_slice(&u64::MAX.to_le_bytes());
        payload[17..49].fill(0xff);
        payload[49..65].copy_from_slice(&u128::MAX.to_le_bytes());
        payload[65..97].fill(0xff);

        let decoded = decode_burn_payload_v1(&payload).expect("payload must decode");
        assert_eq!(decoded.version, 0xff);
        assert_eq!(decoded.source_domain, u32::MAX);
        assert_eq!(decoded.dest_domain, u32::MAX);
        assert_eq!(decoded.nonce, u64::MAX);
        assert_eq!(decoded.sora_asset_id, [0xffu8; 32]);
        assert_eq!(decoded.amount, u128::MAX);
        assert_eq!(decoded.recipient, [0xffu8; 32]);
    }

    #[test]
    fn burn_message_id_is_domain_separated_from_plain_payload_hash() {
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 777,
            sora_asset_id: [0x11u8; 32],
            amount: 10,
            recipient: [0x22u8; 32],
        };
        let payload_bytes = payload.encode_scale();
        let with_prefix = burn_message_id(&payload_bytes);

        let mut k = Keccak::v256();
        k.update(&payload_bytes);
        let mut plain = [0u8; 32];
        k.finalize(&mut plain);

        assert_ne!(with_prefix, plain);
    }

    #[test]
    fn burn_message_id_matches_manual_keccak_with_burn_prefix() {
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 777,
            sora_asset_id: [0x11u8; 32],
            amount: 10,
            recipient: [0x22u8; 32],
        };
        let payload_bytes = payload.encode_scale();
        let from_helper = burn_message_id(&payload_bytes);

        let mut k = Keccak::v256();
        k.update(SCCP_MSG_PREFIX_BURN_V1);
        k.update(&payload_bytes);
        let mut manual = [0u8; 32];
        k.finalize(&mut manual);

        assert_eq!(from_helper, manual);
    }

    #[test]
    fn attest_hash_changes_when_message_id_changes() {
        let payload_a = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 100,
            sora_asset_id: [0x11u8; 32],
            amount: 7,
            recipient: [0x22u8; 32],
        };
        let mut payload_b = payload_a;
        payload_b.nonce = 101;

        let msg_a = burn_message_id(&payload_a.encode_scale());
        let msg_b = burn_message_id(&payload_b.encode_scale());

        let attest_a = attest_hash(&msg_a);
        let attest_b = attest_hash(&msg_b);
        assert_ne!(attest_a, attest_b);
    }

    #[test]
    fn attest_hash_is_stable_for_same_message_id() {
        let message_id = [0x99u8; 32];
        let a = attest_hash(&message_id);
        let b = attest_hash(&message_id);
        assert_eq!(a, b);
    }

    #[test]
    fn attest_hash_matches_manual_keccak_with_attest_prefix() {
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 777,
            sora_asset_id: [0x11u8; 32],
            amount: 10,
            recipient: [0x22u8; 32],
        };
        let message_id = burn_message_id(&payload.encode_scale());

        let from_helper = attest_hash(&message_id);

        let mut k = Keccak::v256();
        k.update(SCCP_MSG_PREFIX_ATTEST_V1);
        k.update(&message_id);
        let mut manual = [0u8; 32];
        k.finalize(&mut manual);

        assert_eq!(from_helper, manual);
    }

    #[test]
    fn attest_hash_is_domain_separated_from_plain_message_id_hash() {
        let message_id = [0x42u8; 32];
        let from_helper = attest_hash(&message_id);

        let mut k = Keccak::v256();
        k.update(&message_id);
        let mut plain = [0u8; 32];
        k.finalize(&mut plain);

        assert_ne!(from_helper, plain);
    }

    #[test]
    fn encode_decode_preserves_non_v1_version_byte() {
        let payload = BurnPayloadV1 {
            version: 7,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SOL,
            nonce: 42,
            sora_asset_id: [0x55u8; 32],
            amount: 123,
            recipient: [0x66u8; 32],
        };
        let encoded = payload.encode_scale();
        let decoded = decode_burn_payload_v1(&encoded).expect("payload should decode");
        assert_eq!(decoded.version, 7);
        assert_eq!(decoded, payload);
    }

    #[test]
    fn burn_message_id_is_stable_for_same_input() {
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_BSC,
            dest_domain: SCCP_DOMAIN_TON,
            nonce: 42,
            sora_asset_id: [0x33u8; 32],
            amount: 99,
            recipient: [0x44u8; 32],
        };
        let encoded = payload.encode_scale();
        let a = burn_message_id(&encoded);
        let b = burn_message_id(&encoded);
        assert_eq!(a, b);
    }

    #[test]
    fn domain_constants_match_spec_values() {
        assert_eq!(SCCP_DOMAIN_SORA, 0);
        assert_eq!(SCCP_DOMAIN_ETH, 1);
        assert_eq!(SCCP_DOMAIN_BSC, 2);
        assert_eq!(SCCP_DOMAIN_SOL, 3);
        assert_eq!(SCCP_DOMAIN_TON, 4);
        assert_eq!(SCCP_DOMAIN_TRON, 5);
    }

    #[test]
    fn domain_constants_are_unique() {
        let domains = [
            SCCP_DOMAIN_SORA,
            SCCP_DOMAIN_ETH,
            SCCP_DOMAIN_BSC,
            SCCP_DOMAIN_SOL,
            SCCP_DOMAIN_TON,
            SCCP_DOMAIN_TRON,
        ];
        for i in 0..domains.len() {
            for j in (i + 1)..domains.len() {
                assert_ne!(domains[i], domains[j]);
            }
        }
    }

    #[test]
    fn hash_prefix_constants_are_distinct_and_non_empty() {
        assert!(!SCCP_MSG_PREFIX_BURN_V1.is_empty());
        assert!(!SCCP_MSG_PREFIX_ATTEST_V1.is_empty());
        assert_ne!(SCCP_MSG_PREFIX_BURN_V1, SCCP_MSG_PREFIX_ATTEST_V1);
    }

    #[test]
    fn formal_assisted_burn_payload_roundtrip_bounded() {
        let source_domains = [SCCP_DOMAIN_SORA, SCCP_DOMAIN_ETH, SCCP_DOMAIN_TRON];
        let dest_domains = [SCCP_DOMAIN_SOL, SCCP_DOMAIN_TON, SCCP_DOMAIN_BSC];
        let nonces = [0u64, 1, 42, u64::MAX];
        let amounts = [0u128, 1, 10, (1u128 << 127), u128::MAX];

        for source_domain in source_domains {
            for dest_domain in dest_domains {
                for nonce in nonces {
                    for amount in amounts {
                        let payload = BurnPayloadV1 {
                            version: 1,
                            source_domain,
                            dest_domain,
                            nonce,
                            sora_asset_id: [0x11u8; 32],
                            amount,
                            recipient: [0x22u8; 32],
                        };
                        let encoded = payload.encode_scale();
                        assert_eq!(encoded.len(), BurnPayloadV1::ENCODED_LEN);
                        let decoded =
                            decode_burn_payload_v1(&encoded).expect("encoded payload must decode");
                        assert_eq!(decoded, payload);
                    }
                }
            }
        }
    }

    #[test]
    fn formal_assisted_decode_rejects_non_exact_lengths() {
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SOL,
            nonce: 9,
            sora_asset_id: [0x55u8; 32],
            amount: 777,
            recipient: [0x66u8; 32],
        };
        let encoded = payload.encode_scale();
        assert_eq!(encoded.len(), BurnPayloadV1::ENCODED_LEN);

        for len in 0..BurnPayloadV1::ENCODED_LEN {
            assert_eq!(
                decode_burn_payload_v1(&encoded[..len]),
                Err(CodecError::InvalidLength),
                "decode should fail for truncated length {}",
                len
            );
        }

        let mut extended = Vec::from(encoded);
        extended.push(0u8);
        assert_eq!(
            decode_burn_payload_v1(&extended),
            Err(CodecError::InvalidLength)
        );

        let decoded = decode_burn_payload_v1(&encoded).expect("exact-length payload must decode");
        assert_eq!(decoded, payload);
    }

    #[test]
    fn formal_assisted_message_id_and_attest_hash_sensitivity_bounded() {
        let mut payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 7,
            sora_asset_id: [0x11u8; 32],
            amount: 123,
            recipient: [0x22u8; 32],
        };

        let base_msg = burn_message_id(&payload.encode_scale());
        let base_attest = attest_hash(&base_msg);

        payload.nonce = payload.nonce.wrapping_add(1);
        let changed_nonce_msg = burn_message_id(&payload.encode_scale());
        let changed_nonce_attest = attest_hash(&changed_nonce_msg);
        assert_ne!(base_msg, changed_nonce_msg);
        assert_ne!(base_attest, changed_nonce_attest);

        payload.nonce = 7;
        payload.recipient[31] ^= 0x01;
        let changed_recipient_msg = burn_message_id(&payload.encode_scale());
        let changed_recipient_attest = attest_hash(&changed_recipient_msg);
        assert_ne!(base_msg, changed_recipient_msg);
        assert_ne!(base_attest, changed_recipient_attest);
    }

    #[test]
    fn formal_assisted_message_id_changes_with_amount_and_domain_variants() {
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SOL,
            nonce: 101,
            sora_asset_id: [0x11u8; 32],
            amount: 1_000,
            recipient: [0x22u8; 32],
        };

        let base_a = burn_message_id(&payload.encode_scale());
        let base_b = burn_message_id(&payload.encode_scale());
        assert_eq!(base_a, base_b);

        let mut amount_changed = payload;
        amount_changed.amount = amount_changed.amount.wrapping_add(1);
        let amount_id = burn_message_id(&amount_changed.encode_scale());
        assert_ne!(base_a, amount_id);

        let mut source_changed = payload;
        source_changed.source_domain = SCCP_DOMAIN_BSC;
        let source_id = burn_message_id(&source_changed.encode_scale());
        assert_ne!(base_a, source_id);

        let mut dest_changed = payload;
        dest_changed.dest_domain = SCCP_DOMAIN_TON;
        let dest_id = burn_message_id(&dest_changed.encode_scale());
        assert_ne!(base_a, dest_id);

        assert_ne!(amount_id, source_id);
        assert_ne!(amount_id, dest_id);
        assert_ne!(source_id, dest_id);
    }

    #[test]
    fn formal_assisted_message_id_is_unique_for_bounded_nonce_window() {
        let mut seen = Vec::<H256>::new();
        for nonce in 0u64..128u64 {
            let payload = BurnPayloadV1 {
                version: 1,
                source_domain: SCCP_DOMAIN_ETH,
                dest_domain: SCCP_DOMAIN_SOL,
                nonce,
                sora_asset_id: [0x33u8; 32],
                amount: 500,
                recipient: [0x44u8; 32],
            };
            let id = burn_message_id(&payload.encode_scale());
            assert!(
                !seen.contains(&id),
                "message id collision within bounded nonce window at nonce {}",
                nonce
            );
            seen.push(id);
        }
        assert_eq!(seen.len(), 128);
    }

    #[test]
    fn formal_assisted_attest_hash_is_stable_and_unique_for_bounded_nonce_window() {
        let mut seen = Vec::<H256>::new();
        for nonce in 0u64..128u64 {
            let payload = BurnPayloadV1 {
                version: 1,
                source_domain: SCCP_DOMAIN_BSC,
                dest_domain: SCCP_DOMAIN_TON,
                nonce,
                sora_asset_id: [0x51u8; 32],
                amount: 900,
                recipient: [0x61u8; 32],
            };

            let message_id = burn_message_id(&payload.encode_scale());
            let attest_a = attest_hash(&message_id);
            let attest_b = attest_hash(&message_id);
            assert_eq!(attest_a, attest_b);
            assert!(
                !seen.contains(&attest_a),
                "attest hash collision within bounded nonce window at nonce {}",
                nonce
            );
            seen.push(attest_a);
        }
        assert_eq!(seen.len(), 128);
    }

    #[test]
    fn formal_assisted_burn_message_id_is_unique_for_bounded_amount_window() {
        let mut seen = Vec::<H256>::new();
        for amount in 0u128..128u128 {
            let payload = BurnPayloadV1 {
                version: 1,
                source_domain: SCCP_DOMAIN_ETH,
                dest_domain: SCCP_DOMAIN_TON,
                nonce: 808,
                sora_asset_id: [0x74u8; 32],
                amount,
                recipient: [0x75u8; 32],
            };
            let id = burn_message_id(&payload.encode_scale());
            assert!(
                !seen.contains(&id),
                "message id collision within bounded amount window at amount {}",
                amount
            );
            seen.push(id);
        }
        assert_eq!(seen.len(), 128);
    }

    #[test]
    fn formal_assisted_burn_message_id_is_unique_for_bounded_source_window() {
        let mut seen = Vec::<H256>::new();
        for source_domain in 0u32..128u32 {
            let payload = BurnPayloadV1 {
                version: 1,
                source_domain,
                dest_domain: SCCP_DOMAIN_SOL,
                nonce: 909,
                sora_asset_id: [0x76u8; 32],
                amount: 2222,
                recipient: [0x77u8; 32],
            };
            let id = burn_message_id(&payload.encode_scale());
            assert!(
                !seen.contains(&id),
                "message id collision within bounded source-domain window at source {}",
                source_domain
            );
            seen.push(id);
        }
        assert_eq!(seen.len(), 128);
    }

    #[test]
    fn formal_assisted_burn_message_id_is_unique_for_bounded_destination_window() {
        let mut seen = Vec::<H256>::new();
        for dest_domain in 0u32..128u32 {
            let payload = BurnPayloadV1 {
                version: 1,
                source_domain: SCCP_DOMAIN_ETH,
                dest_domain,
                nonce: 1_212,
                sora_asset_id: [0x7au8; 32],
                amount: 2_500,
                recipient: [0x7bu8; 32],
            };
            let id = burn_message_id(&payload.encode_scale());
            assert!(
                !seen.contains(&id),
                "message id collision within bounded destination-domain window at destination {}",
                dest_domain
            );
            seen.push(id);
        }
        assert_eq!(seen.len(), 128);
    }

    #[test]
    fn formal_assisted_burn_message_id_is_unique_for_bounded_version_window() {
        let mut seen = Vec::<H256>::new();
        for version in 0u8..128u8 {
            let payload = BurnPayloadV1 {
                version,
                source_domain: SCCP_DOMAIN_ETH,
                dest_domain: SCCP_DOMAIN_SOL,
                nonce: 1_214,
                sora_asset_id: [0x7eu8; 32],
                amount: 3_500,
                recipient: [0x7fu8; 32],
            };
            let id = burn_message_id(&payload.encode_scale());
            assert!(
                !seen.contains(&id),
                "message id collision within bounded version window at version {}",
                version
            );
            seen.push(id);
        }
        assert_eq!(seen.len(), 128);
    }

    #[test]
    fn formal_assisted_burn_message_id_is_unique_for_bounded_asset_id_window() {
        let mut seen = Vec::<H256>::new();
        for i in 0u8..128u8 {
            let mut sora_asset_id = [0u8; 32];
            sora_asset_id[31] = i;
            let payload = BurnPayloadV1 {
                version: 1,
                source_domain: SCCP_DOMAIN_ETH,
                dest_domain: SCCP_DOMAIN_SOL,
                nonce: 1_316,
                sora_asset_id,
                amount: 5_000,
                recipient: [0x85u8; 32],
            };
            let id = burn_message_id(&payload.encode_scale());
            assert!(
                !seen.contains(&id),
                "message id collision within bounded asset-id window at index {}",
                i
            );
            seen.push(id);
        }
        assert_eq!(seen.len(), 128);
    }

    #[test]
    fn formal_assisted_burn_message_id_is_unique_for_bounded_source_destination_matrix() {
        let mut seen = Vec::<H256>::new();
        for source_domain in 0u32..8u32 {
            for dest_domain in 0u32..16u32 {
                let payload = BurnPayloadV1 {
                    version: 1,
                    source_domain,
                    dest_domain,
                    nonce: 1_317,
                    sora_asset_id: [0x86u8; 32],
                    amount: 5_500,
                    recipient: [0x87u8; 32],
                };
                let id = burn_message_id(&payload.encode_scale());
                assert!(
                    !seen.contains(&id),
                    "message id collision in source/destination matrix at {}->{}",
                    source_domain,
                    dest_domain
                );
                seen.push(id);
            }
        }
        assert_eq!(seen.len(), 128);
    }

    #[test]
    fn formal_assisted_burn_message_id_is_unique_for_bounded_recipient_window() {
        let mut seen = Vec::<H256>::new();
        for i in 0u8..128u8 {
            let mut recipient = [0u8; 32];
            recipient[31] = i;
            let payload = BurnPayloadV1 {
                version: 1,
                source_domain: SCCP_DOMAIN_TRON,
                dest_domain: SCCP_DOMAIN_SOL,
                nonce: 1_215,
                sora_asset_id: [0x80u8; 32],
                amount: 3_600,
                recipient,
            };
            let id = burn_message_id(&payload.encode_scale());
            assert!(
                !seen.contains(&id),
                "message id collision within bounded recipient window at index {}",
                i
            );
            seen.push(id);
        }
        assert_eq!(seen.len(), 128);
    }

    #[test]
    fn formal_assisted_attest_hash_is_unique_for_bounded_amount_window() {
        let mut seen = Vec::<H256>::new();
        for amount in 0u128..128u128 {
            let payload = BurnPayloadV1 {
                version: 1,
                source_domain: SCCP_DOMAIN_BSC,
                dest_domain: SCCP_DOMAIN_SOL,
                nonce: 1_111,
                sora_asset_id: [0x78u8; 32],
                amount,
                recipient: [0x79u8; 32],
            };
            let message_id = burn_message_id(&payload.encode_scale());
            let attested = attest_hash(&message_id);
            assert!(
                !seen.contains(&attested),
                "attest hash collision within bounded amount window at amount {}",
                amount
            );
            seen.push(attested);
        }
        assert_eq!(seen.len(), 128);
    }

    #[test]
    fn formal_assisted_attest_hash_is_unique_for_bounded_source_window() {
        let mut seen = Vec::<H256>::new();
        for source_domain in 0u32..128u32 {
            let payload = BurnPayloadV1 {
                version: 1,
                source_domain,
                dest_domain: SCCP_DOMAIN_TON,
                nonce: 1_314,
                sora_asset_id: [0x81u8; 32],
                amount: 4_000,
                recipient: [0x82u8; 32],
            };
            let message_id = burn_message_id(&payload.encode_scale());
            let attested = attest_hash(&message_id);
            assert!(
                !seen.contains(&attested),
                "attest hash collision within bounded source window at source {}",
                source_domain
            );
            seen.push(attested);
        }
        assert_eq!(seen.len(), 128);
    }

    #[test]
    fn formal_assisted_attest_hash_is_unique_for_bounded_version_window() {
        let mut seen = Vec::<H256>::new();
        for version in 0u8..128u8 {
            let payload = BurnPayloadV1 {
                version,
                source_domain: SCCP_DOMAIN_BSC,
                dest_domain: SCCP_DOMAIN_TON,
                nonce: 1_318,
                sora_asset_id: [0x88u8; 32],
                amount: 6_000,
                recipient: [0x89u8; 32],
            };
            let message_id = burn_message_id(&payload.encode_scale());
            let attested = attest_hash(&message_id);
            assert!(
                !seen.contains(&attested),
                "attest hash collision within bounded version window at version {}",
                version
            );
            seen.push(attested);
        }
        assert_eq!(seen.len(), 128);
    }

    #[test]
    fn formal_assisted_attest_hash_is_unique_for_bounded_asset_id_window() {
        let mut seen = Vec::<H256>::new();
        for i in 0u8..128u8 {
            let mut sora_asset_id = [0u8; 32];
            sora_asset_id[31] = i;
            let payload = BurnPayloadV1 {
                version: 1,
                source_domain: SCCP_DOMAIN_BSC,
                dest_domain: SCCP_DOMAIN_TON,
                nonce: 1_319,
                sora_asset_id,
                amount: 6_500,
                recipient: [0x8au8; 32],
            };
            let message_id = burn_message_id(&payload.encode_scale());
            let attested = attest_hash(&message_id);
            assert!(
                !seen.contains(&attested),
                "attest hash collision within bounded asset-id window at index {}",
                i
            );
            seen.push(attested);
        }
        assert_eq!(seen.len(), 128);
    }

    #[test]
    fn formal_assisted_attest_hash_is_unique_for_bounded_source_destination_matrix() {
        let mut seen = Vec::<H256>::new();
        for source_domain in 0u32..8u32 {
            for dest_domain in 0u32..16u32 {
                let payload = BurnPayloadV1 {
                    version: 1,
                    source_domain,
                    dest_domain,
                    nonce: 1_320,
                    sora_asset_id: [0x8bu8; 32],
                    amount: 7_000,
                    recipient: [0x8cu8; 32],
                };
                let message_id = burn_message_id(&payload.encode_scale());
                let attested = attest_hash(&message_id);
                assert!(
                    !seen.contains(&attested),
                    "attest hash collision in source/destination matrix at {}->{}",
                    source_domain,
                    dest_domain
                );
                seen.push(attested);
            }
        }
        assert_eq!(seen.len(), 128);
    }

    #[test]
    fn formal_assisted_burn_message_id_is_unique_for_bounded_nonce_amount_matrix() {
        let mut seen = Vec::<H256>::new();
        for nonce in 0u64..8u64 {
            for amount in 0u128..16u128 {
                let payload = BurnPayloadV1 {
                    version: 1,
                    source_domain: SCCP_DOMAIN_ETH,
                    dest_domain: SCCP_DOMAIN_SOL,
                    nonce,
                    sora_asset_id: [0x8du8; 32],
                    amount,
                    recipient: [0x8eu8; 32],
                };
                let id = burn_message_id(&payload.encode_scale());
                assert!(
                    !seen.contains(&id),
                    "message id collision in nonce/amount matrix at nonce={} amount={}",
                    nonce,
                    amount
                );
                seen.push(id);
            }
        }
        assert_eq!(seen.len(), 128);
    }

    #[test]
    fn formal_assisted_attest_hash_is_unique_for_bounded_nonce_amount_matrix() {
        let mut seen = Vec::<H256>::new();
        for nonce in 0u64..8u64 {
            for amount in 0u128..16u128 {
                let payload = BurnPayloadV1 {
                    version: 1,
                    source_domain: SCCP_DOMAIN_BSC,
                    dest_domain: SCCP_DOMAIN_TON,
                    nonce,
                    sora_asset_id: [0x8fu8; 32],
                    amount,
                    recipient: [0x90u8; 32],
                };
                let message_id = burn_message_id(&payload.encode_scale());
                let attested = attest_hash(&message_id);
                assert!(
                    !seen.contains(&attested),
                    "attest hash collision in nonce/amount matrix at nonce={} amount={}",
                    nonce,
                    amount
                );
                seen.push(attested);
            }
        }
        assert_eq!(seen.len(), 128);
    }

    #[test]
    fn formal_assisted_burn_message_id_is_unique_for_bounded_amount_recipient_matrix() {
        let mut seen = Vec::<H256>::new();
        for amount in 0u128..8u128 {
            for i in 0u8..16u8 {
                let mut recipient = [0u8; 32];
                recipient[31] = i;
                let payload = BurnPayloadV1 {
                    version: 1,
                    source_domain: SCCP_DOMAIN_ETH,
                    dest_domain: SCCP_DOMAIN_SOL,
                    nonce: 1_322,
                    sora_asset_id: [0x93u8; 32],
                    amount,
                    recipient,
                };
                let id = burn_message_id(&payload.encode_scale());
                assert!(
                    !seen.contains(&id),
                    "message id collision in amount/recipient matrix at amount={} recipient_index={}",
                    amount,
                    i
                );
                seen.push(id);
            }
        }
        assert_eq!(seen.len(), 128);
    }

    #[test]
    fn formal_assisted_attest_hash_is_unique_for_bounded_amount_recipient_matrix() {
        let mut seen = Vec::<H256>::new();
        for amount in 0u128..8u128 {
            for i in 0u8..16u8 {
                let mut recipient = [0u8; 32];
                recipient[31] = i;
                let payload = BurnPayloadV1 {
                    version: 1,
                    source_domain: SCCP_DOMAIN_BSC,
                    dest_domain: SCCP_DOMAIN_TON,
                    nonce: 1_323,
                    sora_asset_id: [0x94u8; 32],
                    amount,
                    recipient,
                };
                let message_id = burn_message_id(&payload.encode_scale());
                let attested = attest_hash(&message_id);
                assert!(
                    !seen.contains(&attested),
                    "attest hash collision in amount/recipient matrix at amount={} recipient_index={}",
                    amount,
                    i
                );
                seen.push(attested);
            }
        }
        assert_eq!(seen.len(), 128);
    }

    #[test]
    fn formal_assisted_burn_message_id_is_unique_for_bounded_nonce_asset_id_matrix() {
        let mut seen = Vec::<H256>::new();
        for nonce in 0u64..8u64 {
            for i in 0u8..16u8 {
                let mut sora_asset_id = [0u8; 32];
                sora_asset_id[31] = i;
                let payload = BurnPayloadV1 {
                    version: 1,
                    source_domain: SCCP_DOMAIN_ETH,
                    dest_domain: SCCP_DOMAIN_SOL,
                    nonce,
                    sora_asset_id,
                    amount: 8_000,
                    recipient: [0x95u8; 32],
                };
                let id = burn_message_id(&payload.encode_scale());
                assert!(
                    !seen.contains(&id),
                    "message id collision in nonce/asset-id matrix at nonce={} asset_index={}",
                    nonce,
                    i
                );
                seen.push(id);
            }
        }
        assert_eq!(seen.len(), 128);
    }

    #[test]
    fn formal_assisted_attest_hash_is_unique_for_bounded_nonce_asset_id_matrix() {
        let mut seen = Vec::<H256>::new();
        for nonce in 0u64..8u64 {
            for i in 0u8..16u8 {
                let mut sora_asset_id = [0u8; 32];
                sora_asset_id[31] = i;
                let payload = BurnPayloadV1 {
                    version: 1,
                    source_domain: SCCP_DOMAIN_BSC,
                    dest_domain: SCCP_DOMAIN_TON,
                    nonce,
                    sora_asset_id,
                    amount: 8_500,
                    recipient: [0x96u8; 32],
                };
                let message_id = burn_message_id(&payload.encode_scale());
                let attested = attest_hash(&message_id);
                assert!(
                    !seen.contains(&attested),
                    "attest hash collision in nonce/asset-id matrix at nonce={} asset_index={}",
                    nonce,
                    i
                );
                seen.push(attested);
            }
        }
        assert_eq!(seen.len(), 128);
    }

    #[test]
    fn formal_assisted_attest_hash_is_unique_for_bounded_destination_window() {
        let mut seen = Vec::<H256>::new();
        for dest_domain in 0u32..128u32 {
            let payload = BurnPayloadV1 {
                version: 1,
                source_domain: SCCP_DOMAIN_TRON,
                dest_domain,
                nonce: 1_313,
                sora_asset_id: [0x7cu8; 32],
                amount: 3_000,
                recipient: [0x7du8; 32],
            };
            let message_id = burn_message_id(&payload.encode_scale());
            let attested = attest_hash(&message_id);
            assert!(
                !seen.contains(&attested),
                "attest hash collision within bounded destination-domain window at destination {}",
                dest_domain
            );
            seen.push(attested);
        }
        assert_eq!(seen.len(), 128);
    }

    #[test]
    fn formal_assisted_burn_message_id_is_direction_sensitive_for_swapped_domains() {
        let forward = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SOL,
            nonce: 1_315,
            sora_asset_id: [0x83u8; 32],
            amount: 4_500,
            recipient: [0x84u8; 32],
        };
        let reverse = BurnPayloadV1 {
            source_domain: forward.dest_domain,
            dest_domain: forward.source_domain,
            ..forward
        };

        let forward_id = burn_message_id(&forward.encode_scale());
        let reverse_id = burn_message_id(&reverse.encode_scale());
        assert_ne!(forward_id, reverse_id);
    }

    #[test]
    fn formal_assisted_attest_hash_is_direction_sensitive_for_swapped_domains() {
        let forward = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SOL,
            nonce: 1_321,
            sora_asset_id: [0x91u8; 32],
            amount: 7_500,
            recipient: [0x92u8; 32],
        };
        let reverse = BurnPayloadV1 {
            source_domain: forward.dest_domain,
            dest_domain: forward.source_domain,
            ..forward
        };

        let forward_message_id = burn_message_id(&forward.encode_scale());
        let reverse_message_id = burn_message_id(&reverse.encode_scale());
        let forward_attest = attest_hash(&forward_message_id);
        let reverse_attest = attest_hash(&reverse_message_id);
        assert_ne!(forward_attest, reverse_attest);
    }

    #[test]
    fn formal_assisted_attest_hash_is_unique_for_bounded_recipient_window() {
        let mut seen = Vec::<H256>::new();
        for i in 0u8..128u8 {
            let mut recipient = [0u8; 32];
            recipient[31] = i;
            let payload = BurnPayloadV1 {
                version: 1,
                source_domain: SCCP_DOMAIN_TRON,
                dest_domain: SCCP_DOMAIN_SOL,
                nonce: 42,
                sora_asset_id: [0x71u8; 32],
                amount: 1_234,
                recipient,
            };
            let message_id = burn_message_id(&payload.encode_scale());
            let attested = attest_hash(&message_id);
            assert!(
                !seen.contains(&attested),
                "attest hash collision within bounded recipient window at index {}",
                i
            );
            seen.push(attested);
        }
        assert_eq!(seen.len(), 128);
    }

    #[test]
    fn formal_assisted_attest_hash_is_domain_separated_from_plain_message_hash() {
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_BSC,
            dest_domain: SCCP_DOMAIN_ETH,
            nonce: 91,
            sora_asset_id: [0x72u8; 32],
            amount: 1_001,
            recipient: [0x73u8; 32],
        };
        let message_id = burn_message_id(&payload.encode_scale());
        let attested = attest_hash(&message_id);

        let mut plain = [0u8; 32];
        let mut k = Keccak::v256();
        k.update(&message_id);
        k.finalize(&mut plain);

        assert_ne!(attested, plain);
    }

    #[test]
    fn formal_assisted_prefix_literals_remain_stable() {
        assert_eq!(SCCP_MSG_PREFIX_BURN_V1, b"sccp:burn:v1");
        assert_eq!(SCCP_MSG_PREFIX_ATTEST_V1, b"sccp:attest:v1");
    }

    #[test]
    fn formal_assisted_domain_separation_prefixes_are_effective() {
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 777,
            sora_asset_id: [0x11u8; 32],
            amount: 10,
            recipient: [0x22u8; 32],
        };

        let payload_bytes = payload.encode_scale();
        let msg_id = burn_message_id(&payload_bytes);
        let attested = attest_hash(&msg_id);

        let mut burn_prefixed = [0u8; 32];
        let mut k1 = Keccak::v256();
        k1.update(SCCP_MSG_PREFIX_BURN_V1);
        k1.update(&msg_id);
        k1.finalize(&mut burn_prefixed);

        let mut plain = [0u8; 32];
        let mut k2 = Keccak::v256();
        k2.update(&msg_id);
        k2.finalize(&mut plain);

        assert_ne!(attested, burn_prefixed);
        assert_ne!(attested, plain);
        assert_ne!(SCCP_MSG_PREFIX_BURN_V1, SCCP_MSG_PREFIX_ATTEST_V1);
    }
}
