#![allow(unexpected_cfgs)]
#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use sp_runtime_interface::{pass_by::PassFatPointerAndDecode, runtime_interface};
use sp_std::vec::Vec;

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct SolanaVoteAuthorityConfigV1 {
    pub authority_pubkey: [u8; 32],
    pub stake: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct SolanaVerifyRequest {
    pub proof: Vec<u8>,
    pub expected_message_id: [u8; 32],
    pub expected_router_program_id: [u8; 32],
    pub authorities: Vec<SolanaVoteAuthorityConfigV1>,
    pub threshold_stake: u64,
}

#[runtime_interface]
pub trait SolanaProofApi {
    fn verify_solana_finalized_burn_proof(
        request: PassFatPointerAndDecode<SolanaVerifyRequest>,
    ) -> bool {
        #[cfg(feature = "std")]
        {
            verifier::verify_solana_finalized_burn_proof(&request).is_ok()
        }

        #[cfg(not(feature = "std"))]
        {
            let _ = request;
            false
        }
    }
}

#[cfg(feature = "std")]
pub mod verifier {
    use std::collections::{BTreeMap, BTreeSet, VecDeque};

    use blake3::Hasher as Blake3Hasher;
    use codec::{Decode, Encode};
    use curve25519_dalek::edwards::CompressedEdwardsY;
    use ed25519_dalek::{Signature, VerifyingKey};
    use serde::{Deserialize, Serialize};
    use sha2::{Digest as _, Sha256};
    use sha3::Keccak256;

    use crate::{SolanaVerifyRequest, SolanaVoteAuthorityConfigV1};

    const SOLANA_MERKLE_FANOUT: usize = 16;
    const SOLANA_MAX_SEEDS: usize = 16;
    const SOLANA_MAX_SEED_LEN: usize = 32;
    const PDA_MARKER: &[u8] = b"ProgramDerivedAddress";

    const SCCP_SEED_PREFIX: &[u8] = b"sccp";
    const SCCP_SEED_BURN: &[u8] = b"burn";

    const VOTE_PROGRAM_ID: [u8; 32] =
        hex32("0761481d357474bb7c4d7624ebd3bdb3d8355e73d11043fc0da3538000000000");
    const SYSVAR_PROGRAM_ID: [u8; 32] =
        hex32("06a7d5171875f729c73d93408f216120067ed88c76e08c287fc1946000000000");
    const SLOT_HASHES_SYSVAR_ID: [u8; 32] =
        hex32("06a7d517192f0aafc6f265e3fb77cc7ada82c529d0be3b136e2d005520000000");
    const SOLANA_BURN_PAYLOAD_ENCODED_LEN: usize = 97;
    const SOLANA_FINALIZED_BURN_PROOF_VERSION_V1: u8 = 1;
    const SCCP_DOMAIN_SORA: u32 = 0;
    const SCCP_DOMAIN_SOL: u32 = 3;
    const SCCP_MSG_PREFIX_BURN_V1: &[u8] = b"sccp:burn:v1";
    const SCCP_MAX_SOLANA_MERKLE_DEPTH: usize = 32;
    const SCCP_MAX_SOLANA_ACCOUNT_DATA_BYTES: usize = 64 * 1024;
    const SCCP_MAX_SOLANA_MESSAGE_BYTES: usize = 4 * 1024;

    type VerifyResult<T> = Result<T, String>;
    type SolHash = [u8; 32];

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
    struct H256(pub [u8; 32]);

    impl From<[u8; 32]> for H256 {
        fn from(value: [u8; 32]) -> Self {
            Self(value)
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
    struct BurnPayloadV1 {
        version: u8,
        source_domain: u32,
        dest_domain: u32,
        nonce: u64,
        sora_asset_id: [u8; 32],
        amount: u128,
        recipient: [u8; 32],
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct SolanaBurnRecordAccountV1 {
        version: u8,
        bump: u8,
        message_id: [u8; 32],
        payload: [u8; SOLANA_BURN_PAYLOAD_ENCODED_LEN],
        sender: [u8; 32],
        mint: [u8; 32],
        slot: u64,
    }

    impl SolanaBurnRecordAccountV1 {
        #[cfg(test)]
        fn encode_account_data(&self) -> [u8; 203] {
            let mut out = [0u8; 203];
            out[0] = self.version;
            out[1] = self.bump;
            out[2..34].copy_from_slice(&self.message_id);
            out[34..131].copy_from_slice(&self.payload);
            out[131..163].copy_from_slice(&self.sender);
            out[163..195].copy_from_slice(&self.mint);
            out[195..203].copy_from_slice(&self.slot.to_le_bytes());
            out
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
    struct SolanaFinalizedBurnPublicInputsV1 {
        message_id: H256,
        finalized_slot: u64,
        finalized_bank_hash: H256,
        finalized_slot_hash: H256,
        router_program_id: [u8; 32],
        burn_record_pda: [u8; 32],
        burn_record_owner: [u8; 32],
        burn_record_data_hash: H256,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
    struct SolanaMerkleProofV1 {
        path: Vec<u8>,
        siblings: Vec<Vec<H256>>,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
    struct SolanaAccountInfoV1 {
        pubkey: [u8; 32],
        lamports: u64,
        owner: [u8; 32],
        executable: bool,
        rent_epoch: u64,
        data: Vec<u8>,
        write_version: u64,
        slot: u64,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
    struct SolanaAccountDeltaProofV1 {
        account: SolanaAccountInfoV1,
        merkle_proof: SolanaMerkleProofV1,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
    struct SolanaBankHashProofV1 {
        slot: u64,
        bank_hash: H256,
        account_delta_root: H256,
        parent_bank_hash: H256,
        blockhash: H256,
        num_sigs: u64,
        account_proof: SolanaAccountDeltaProofV1,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
    struct SolanaVoteProofV1 {
        authority_pubkey: [u8; 32],
        signature: [u8; 64],
        signed_message: Vec<u8>,
        vote_slot: u64,
        vote_bank_hash: H256,
        rooted_slot: Option<u64>,
        slot_hashes_proof: SolanaBankHashProofV1,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
    struct SolanaFinalizedBurnProofV1 {
        version: u8,
        public_inputs: SolanaFinalizedBurnPublicInputsV1,
        burn_proof: SolanaBankHashProofV1,
        vote_proofs: Vec<SolanaVoteProofV1>,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct LocalMessageHeader {
        num_required_signatures: u8,
        num_readonly_signed_accounts: u8,
        num_readonly_unsigned_accounts: u8,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct LocalCompiledInstruction {
        program_id_index: u8,
        #[serde(with = "short_vec")]
        accounts: Vec<u8>,
        #[serde(with = "short_vec")]
        data: Vec<u8>,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct LocalMessage {
        header: LocalMessageHeader,
        #[serde(with = "short_vec")]
        account_keys: Vec<[u8; 32]>,
        recent_blockhash: SolHash,
        #[serde(with = "short_vec")]
        instructions: Vec<LocalCompiledInstruction>,
    }

    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
    struct Lockout {
        slot: u64,
        confirmation_count: u32,
    }

    impl Lockout {
        #[cfg(test)]
        fn new(slot: u64) -> Self {
            Self {
                slot,
                confirmation_count: 1,
            }
        }

        fn new_with_confirmation_count(slot: u64, confirmation_count: u32) -> Self {
            Self {
                slot,
                confirmation_count,
            }
        }

        fn slot(&self) -> u64 {
            self.slot
        }
    }

    #[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
    struct Vote {
        slots: Vec<u64>,
        hash: SolHash,
        timestamp: Option<i64>,
    }

    impl Vote {
        fn last_voted_slot(&self) -> Option<u64> {
            self.slots.last().copied()
        }
    }

    #[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
    struct VoteStateUpdate {
        lockouts: VecDeque<Lockout>,
        root: Option<u64>,
        hash: SolHash,
        timestamp: Option<i64>,
    }

    impl VoteStateUpdate {
        fn last_voted_slot(&self) -> Option<u64> {
            self.lockouts.back().map(|lockout| lockout.slot())
        }
    }

    #[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
    struct TowerSync {
        lockouts: VecDeque<Lockout>,
        root: Option<u64>,
        hash: SolHash,
        timestamp: Option<i64>,
        block_id: SolHash,
    }

    impl TowerSync {
        fn last_voted_slot(&self) -> Option<u64> {
            self.lockouts.back().map(|lockout| lockout.slot())
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    enum VoteAuthorize {
        Voter,
        Withdrawer,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct VoteInit {
        node_pubkey: [u8; 32],
        authorized_voter: [u8; 32],
        authorized_withdrawer: [u8; 32],
        commission: u8,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct VoteAuthorizeWithSeedArgs {
        authorization_type: VoteAuthorize,
        current_authority_derived_key_owner: [u8; 32],
        current_authority_derived_key_seed: String,
        new_authority: [u8; 32],
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct VoteAuthorizeCheckedWithSeedArgs {
        authorization_type: VoteAuthorize,
        current_authority_derived_key_owner: [u8; 32],
        current_authority_derived_key_seed: String,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    enum VoteInstruction {
        InitializeAccount(VoteInit),
        Authorize([u8; 32], VoteAuthorize),
        Vote(Vote),
        Withdraw(u64),
        UpdateValidatorIdentity,
        UpdateCommission(u8),
        VoteSwitch(Vote, SolHash),
        AuthorizeChecked(VoteAuthorize),
        UpdateVoteState(VoteStateUpdate),
        UpdateVoteStateSwitch(VoteStateUpdate, SolHash),
        AuthorizeWithSeed(VoteAuthorizeWithSeedArgs),
        AuthorizeCheckedWithSeed(VoteAuthorizeCheckedWithSeedArgs),
        CompactUpdateVoteState(#[serde(with = "serde_compact_vote_state_update")] VoteStateUpdate),
        CompactUpdateVoteStateSwitch(
            #[serde(with = "serde_compact_vote_state_update")] VoteStateUpdate,
            SolHash,
        ),
        TowerSync(TowerSync),
        TowerSyncSwitch(TowerSync, SolHash),
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct VoteSwitchFields(Vote, SolHash);

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct VoteStateUpdateSwitchFields(VoteStateUpdate, SolHash);

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct CompactLockoutOffset {
        offset: u64,
        confirmation_count: u8,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct CompactVoteStateUpdateFields {
        root: u64,
        lockout_offsets: Vec<CompactLockoutOffset>,
        hash: SolHash,
        timestamp: Option<i64>,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct CompactVoteStateUpdateSwitchFields(CompactVoteStateUpdateFields, SolHash);

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct SlotHashes(Vec<(u64, SolHash)>);

    impl SlotHashes {
        fn get(&self, slot: &u64) -> Option<&SolHash> {
            self.0
                .iter()
                .find(|(candidate, _)| candidate == slot)
                .map(|(_, hash)| hash)
        }
    }

    fn decode_legacy_message(input: &[u8]) -> VerifyResult<LocalMessage> {
        let mut offset = 0usize;
        let header = LocalMessageHeader {
            num_required_signatures: take_u8(input, &mut offset, "num_required_signatures")?,
            num_readonly_signed_accounts: take_u8(
                input,
                &mut offset,
                "num_readonly_signed_accounts",
            )?,
            num_readonly_unsigned_accounts: take_u8(
                input,
                &mut offset,
                "num_readonly_unsigned_accounts",
            )?,
        };

        let account_keys_len = usize::from(decode_short_vec_len(input, &mut offset)?);
        let mut account_keys = Vec::with_capacity(account_keys_len);
        for _ in 0..account_keys_len {
            account_keys.push(take_array::<32>(input, &mut offset, "account_key")?);
        }

        let recent_blockhash = take_array::<32>(input, &mut offset, "recent_blockhash")?;
        let instructions_len = usize::from(decode_short_vec_len(input, &mut offset)?);
        let mut instructions = Vec::with_capacity(instructions_len);
        for _ in 0..instructions_len {
            let program_id_index = take_u8(input, &mut offset, "instruction_program_id_index")?;
            let accounts_len = usize::from(decode_short_vec_len(input, &mut offset)?);
            let accounts = take_vec(input, &mut offset, accounts_len, "instruction_accounts")?;
            let data_len = usize::from(decode_short_vec_len(input, &mut offset)?);
            let data = take_vec(input, &mut offset, data_len, "instruction_data")?;
            instructions.push(LocalCompiledInstruction {
                program_id_index,
                accounts,
                data,
            });
        }

        if offset != input.len() {
            return Err("vote message contains trailing bytes".into());
        }

        Ok(LocalMessage {
            header,
            account_keys,
            recent_blockhash,
            instructions,
        })
    }

    #[cfg(test)]
    fn encode_legacy_message(message: &LocalMessage) -> Vec<u8> {
        let mut out = Vec::new();
        out.push(message.header.num_required_signatures);
        out.push(message.header.num_readonly_signed_accounts);
        out.push(message.header.num_readonly_unsigned_accounts);
        out.extend_from_slice(&encode_short_vec_len(message.account_keys.len()));
        for key in &message.account_keys {
            out.extend_from_slice(key);
        }
        out.extend_from_slice(&message.recent_blockhash);
        out.extend_from_slice(&encode_short_vec_len(message.instructions.len()));
        for instruction in &message.instructions {
            out.push(instruction.program_id_index);
            out.extend_from_slice(&encode_short_vec_len(instruction.accounts.len()));
            out.extend_from_slice(&instruction.accounts);
            out.extend_from_slice(&encode_short_vec_len(instruction.data.len()));
            out.extend_from_slice(&instruction.data);
        }
        out
    }

    fn take_u8(input: &[u8], offset: &mut usize, label: &str) -> VerifyResult<u8> {
        let value = input
            .get(*offset)
            .copied()
            .ok_or_else(|| format!("vote message is truncated while reading {label}"))?;
        *offset += 1;
        Ok(value)
    }

    fn take_array<const N: usize>(
        input: &[u8],
        offset: &mut usize,
        label: &str,
    ) -> VerifyResult<[u8; N]> {
        let end = offset
            .checked_add(N)
            .ok_or_else(|| format!("vote message offset overflow while reading {label}"))?;
        let slice = input
            .get(*offset..end)
            .ok_or_else(|| format!("vote message is truncated while reading {label}"))?;
        let mut out = [0u8; N];
        out.copy_from_slice(slice);
        *offset = end;
        Ok(out)
    }

    fn take_u64_le(input: &[u8], offset: &mut usize, label: &str) -> VerifyResult<u64> {
        Ok(u64::from_le_bytes(take_array::<8>(input, offset, label)?))
    }

    fn take_i64_le(input: &[u8], offset: &mut usize, label: &str) -> VerifyResult<i64> {
        Ok(i64::from_le_bytes(take_array::<8>(input, offset, label)?))
    }

    fn take_option_i64(input: &[u8], offset: &mut usize, label: &str) -> VerifyResult<Option<i64>> {
        match take_u8(input, offset, label)? {
            0 => Ok(None),
            1 => take_i64_le(input, offset, label).map(Some),
            tag => Err(format!(
                "invalid option tag {} while reading {}",
                tag, label
            )),
        }
    }

    fn take_vec(
        input: &[u8],
        offset: &mut usize,
        len: usize,
        label: &str,
    ) -> VerifyResult<Vec<u8>> {
        let end = offset
            .checked_add(len)
            .ok_or_else(|| format!("vote message offset overflow while reading {label}"))?;
        let slice = input
            .get(*offset..end)
            .ok_or_else(|| format!("vote message is truncated while reading {label}"))?;
        *offset = end;
        Ok(slice.to_vec())
    }

    fn decode_short_vec_len(input: &[u8], offset: &mut usize) -> VerifyResult<u16> {
        let mut value = 0u16;
        for byte_index in 0..3 {
            let byte = take_u8(input, offset, "short_vec_len")?;
            let low_bits = u16::from(byte & 0x7f);
            value = value
                .checked_add(
                    low_bits
                        .checked_shl(u32::try_from(byte_index * 7).unwrap_or(u32::MAX))
                        .ok_or_else(|| "short_vec length shift overflow".to_string())?,
                )
                .ok_or_else(|| "short_vec length overflow".to_string())?;
            if byte & 0x80 == 0 {
                if byte_index > 0 && low_bits == 0 {
                    return Err("short_vec uses a non-canonical alias encoding".into());
                }
                return Ok(value);
            }
            if byte_index == 2 {
                return Err("short_vec length exceeds three bytes".into());
            }
        }
        Err("short_vec length decode fell through".into())
    }

    fn decode_varint_u64(input: &[u8], offset: &mut usize, label: &str) -> VerifyResult<u64> {
        let mut value = 0u64;
        for byte_index in 0..10 {
            let byte = take_u8(input, offset, label)?;
            let low_bits = u64::from(byte & 0x7f);
            value = value
                .checked_add(
                    low_bits
                        .checked_shl(u32::try_from(byte_index * 7).unwrap_or(u32::MAX))
                        .ok_or_else(|| "varint shift overflow".to_string())?,
                )
                .ok_or_else(|| "varint overflow".to_string())?;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err("varint exceeds ten bytes".into())
    }

    #[cfg(test)]
    fn encode_short_vec_len(len: usize) -> Vec<u8> {
        let mut remaining = u16::try_from(len).expect("test message lengths fit into u16");
        let mut out = Vec::new();
        loop {
            let mut byte = (remaining & 0x7f) as u8;
            remaining >>= 7;
            if remaining == 0 {
                out.push(byte);
                break;
            }
            byte |= 0x80;
            out.push(byte);
        }
        out
    }

    fn decode_vote_instruction_data(data: &[u8]) -> VerifyResult<VoteInstruction> {
        if data.len() < 4 {
            return Err("vote instruction data is truncated".into());
        }

        let mut discriminant = [0u8; 4];
        discriminant.copy_from_slice(&data[..4]);
        let payload = &data[4..];
        match u32::from_le_bytes(discriminant) {
            2 => deserialize_vote_payload::<Vote>(payload).map(VoteInstruction::Vote),
            6 => deserialize_vote_payload::<VoteSwitchFields>(payload)
                .map(|fields| VoteInstruction::VoteSwitch(fields.0, fields.1)),
            8 => deserialize_vote_payload::<VoteStateUpdate>(payload)
                .map(VoteInstruction::UpdateVoteState),
            9 => deserialize_vote_payload::<VoteStateUpdateSwitchFields>(payload)
                .map(|fields| VoteInstruction::UpdateVoteStateSwitch(fields.0, fields.1)),
            12 => deserialize_vote_payload::<CompactVoteStateUpdateFields>(payload).and_then(
                |compact| {
                    expand_compact_vote_state_update(compact)
                        .map(VoteInstruction::CompactUpdateVoteState)
                },
            ),
            13 => deserialize_vote_payload::<CompactVoteStateUpdateSwitchFields>(payload).and_then(
                |fields| {
                    expand_compact_vote_state_update(fields.0).map(|update| {
                        VoteInstruction::CompactUpdateVoteStateSwitch(update, fields.1)
                    })
                },
            ),
            14 => parse_compact_tower_sync(payload).map(VoteInstruction::TowerSync),
            15 => parse_compact_tower_sync_switch(payload)
                .map(|(tower_sync, hash)| VoteInstruction::TowerSyncSwitch(tower_sync, hash)),
            _ => Err("unsupported vote instruction variant in proof".into()),
        }
    }

    #[cfg(test)]
    fn serialize_vote_instruction_data(vote_ix: &VoteInstruction) -> VerifyResult<Vec<u8>> {
        let mut out = Vec::new();
        match vote_ix {
            VoteInstruction::Vote(vote) => {
                out.extend_from_slice(&2u32.to_le_bytes());
                out.extend_from_slice(&bincode::serialize(vote).map_err(|e| e.to_string())?);
            }
            VoteInstruction::VoteSwitch(vote, hash) => {
                out.extend_from_slice(&6u32.to_le_bytes());
                out.extend_from_slice(
                    &bincode::serialize(&VoteSwitchFields(vote.clone(), *hash))
                        .map_err(|e| e.to_string())?,
                );
            }
            VoteInstruction::UpdateVoteState(update) => {
                out.extend_from_slice(&8u32.to_le_bytes());
                out.extend_from_slice(&bincode::serialize(update).map_err(|e| e.to_string())?);
            }
            VoteInstruction::UpdateVoteStateSwitch(update, hash) => {
                out.extend_from_slice(&9u32.to_le_bytes());
                out.extend_from_slice(
                    &bincode::serialize(&VoteStateUpdateSwitchFields(update.clone(), *hash))
                        .map_err(|e| e.to_string())?,
                );
            }
            VoteInstruction::CompactUpdateVoteState(update) => {
                out.extend_from_slice(&12u32.to_le_bytes());
                out.extend_from_slice(
                    &bincode::serialize(&compact_vote_state_update(update))
                        .map_err(|e| e.to_string())?,
                );
            }
            VoteInstruction::CompactUpdateVoteStateSwitch(update, hash) => {
                out.extend_from_slice(&13u32.to_le_bytes());
                out.extend_from_slice(
                    &bincode::serialize(&CompactVoteStateUpdateSwitchFields(
                        compact_vote_state_update(update),
                        *hash,
                    ))
                    .map_err(|e| e.to_string())?,
                );
            }
            VoteInstruction::TowerSync(_) | VoteInstruction::TowerSyncSwitch(_, _) => {
                return Err("unsupported TowerSync serializer in test helper".into())
            }
            _ => return Err("unsupported vote instruction variant in proof".into()),
        }
        Ok(out)
    }

    fn deserialize_vote_payload<T>(payload: &[u8]) -> VerifyResult<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        bincode::deserialize(payload).map_err(|e| format!("failed to decode vote instruction: {e}"))
    }

    #[cfg(test)]
    fn compact_vote_state_update(update: &VoteStateUpdate) -> CompactVoteStateUpdateFields {
        let mut previous = update.root.unwrap_or_default();
        let mut lockout_offsets = Vec::with_capacity(update.lockouts.len());
        for lockout in &update.lockouts {
            let offset = lockout
                .slot()
                .checked_sub(previous)
                .expect("test vote lockout offsets are monotonic");
            let confirmation_count =
                u8::try_from(lockout.confirmation_count).expect("confirmation count fits into u8");
            lockout_offsets.push(CompactLockoutOffset {
                offset,
                confirmation_count,
            });
            previous = lockout.slot();
        }
        CompactVoteStateUpdateFields {
            root: update.root.unwrap_or(u64::MAX),
            lockout_offsets,
            hash: update.hash,
            timestamp: update.timestamp,
        }
    }

    fn expand_compact_vote_state_update(
        compact: CompactVoteStateUpdateFields,
    ) -> VerifyResult<VoteStateUpdate> {
        let root = (compact.root != u64::MAX).then_some(compact.root);
        let mut previous = root.unwrap_or_default();
        let mut lockouts = VecDeque::new();
        for lockout_offset in compact.lockout_offsets {
            previous = previous
                .checked_add(lockout_offset.offset)
                .ok_or_else(|| "invalid compact vote-state lockout offset".to_string())?;
            lockouts.push_back(Lockout::new_with_confirmation_count(
                previous,
                u32::from(lockout_offset.confirmation_count),
            ));
        }
        Ok(VoteStateUpdate {
            lockouts,
            root,
            hash: compact.hash,
            timestamp: compact.timestamp,
        })
    }

    fn parse_compact_tower_sync(payload: &[u8]) -> VerifyResult<TowerSync> {
        let (tower_sync, offset) = parse_compact_tower_sync_prefix(payload)?;
        if offset != payload.len() {
            return Err("tower-sync payload has trailing bytes".into());
        }
        Ok(tower_sync)
    }

    fn parse_compact_tower_sync_switch(payload: &[u8]) -> VerifyResult<(TowerSync, SolHash)> {
        let (tower_sync, mut offset) = parse_compact_tower_sync_prefix(payload)?;
        let switch_hash = take_array::<32>(payload, &mut offset, "tower_sync_switch_hash")?;
        if offset != payload.len() {
            return Err("tower-sync switch payload has trailing bytes".into());
        }
        Ok((tower_sync, switch_hash))
    }

    fn parse_compact_tower_sync_prefix(payload: &[u8]) -> VerifyResult<(TowerSync, usize)> {
        let mut offset = 0usize;
        let root_raw = take_u64_le(payload, &mut offset, "tower_sync_root")?;
        let lockouts_len = usize::from(decode_short_vec_len(payload, &mut offset)?);
        let mut previous = if root_raw == u64::MAX { 0 } else { root_raw };
        let mut lockouts = VecDeque::new();
        for _ in 0..lockouts_len {
            let delta = decode_varint_u64(payload, &mut offset, "tower_sync_lockout_offset")?;
            let confirmation_count =
                take_u8(payload, &mut offset, "tower_sync_confirmation_count")?;
            previous = previous
                .checked_add(delta)
                .ok_or_else(|| "invalid compact tower-sync lockout offset".to_string())?;
            lockouts.push_back(Lockout::new_with_confirmation_count(
                previous,
                u32::from(confirmation_count),
            ));
        }
        let hash = take_array::<32>(payload, &mut offset, "tower_sync_hash")?;
        let timestamp = take_option_i64(payload, &mut offset, "tower_sync_timestamp")?;
        let block_id = take_array::<32>(payload, &mut offset, "tower_sync_block_id")?;
        Ok((
            TowerSync {
                lockouts,
                root: (root_raw != u64::MAX).then_some(root_raw),
                hash,
                timestamp,
                block_id,
            },
            offset,
        ))
    }

    fn decode_solana_finalized_burn_proof_v1(proof: &[u8]) -> Option<SolanaFinalizedBurnProofV1> {
        let mut input = proof;
        let decoded = SolanaFinalizedBurnProofV1::decode(&mut input).ok()?;
        if !input.is_empty()
            || decoded.version != SOLANA_FINALIZED_BURN_PROOF_VERSION_V1
            || decoded.vote_proofs.is_empty()
            || decoded.burn_proof.account_proof.account.data.len()
                > SCCP_MAX_SOLANA_ACCOUNT_DATA_BYTES
            || decoded.burn_proof.account_proof.merkle_proof.path.len()
                > SCCP_MAX_SOLANA_MERKLE_DEPTH
            || decoded.burn_proof.account_proof.merkle_proof.path.len()
                != decoded.burn_proof.account_proof.merkle_proof.siblings.len()
        {
            return None;
        }

        for vote in &decoded.vote_proofs {
            if vote.signed_message.len() > SCCP_MAX_SOLANA_MESSAGE_BYTES
                || vote.slot_hashes_proof.account_proof.account.data.len()
                    > SCCP_MAX_SOLANA_ACCOUNT_DATA_BYTES
                || vote.slot_hashes_proof.account_proof.merkle_proof.path.len()
                    > SCCP_MAX_SOLANA_MERKLE_DEPTH
                || vote.slot_hashes_proof.account_proof.merkle_proof.path.len()
                    != vote
                        .slot_hashes_proof
                        .account_proof
                        .merkle_proof
                        .siblings
                        .len()
                || vote
                    .slot_hashes_proof
                    .account_proof
                    .merkle_proof
                    .siblings
                    .iter()
                    .any(|level| level.len() > 15)
            {
                return None;
            }
        }

        if decoded
            .burn_proof
            .account_proof
            .merkle_proof
            .siblings
            .iter()
            .any(|level| level.len() > 15)
        {
            return None;
        }

        Some(decoded)
    }

    pub fn verify_solana_finalized_burn_proof(request: &SolanaVerifyRequest) -> VerifyResult<()> {
        let proof = decode_solana_finalized_burn_proof_v1(&request.proof)
            .ok_or_else(|| "invalid SCALE-encoded Solana finalized-burn proof".to_string())?;
        verify_expected_bindings(&proof, request)?;

        let burn_account = verify_bank_hash_proof(
            &proof.burn_proof,
            proof.public_inputs.finalized_bank_hash.0,
            proof.public_inputs.finalized_slot,
        )?;
        verify_burn_account(&burn_account, &proof, request)?;

        let authority_stakes = authority_stake_map(&request.authorities)?;
        let mut accumulated_stake = 0u64;
        let mut seen = BTreeSet::new();
        for vote_proof in &proof.vote_proofs {
            verify_vote_proof(vote_proof, &proof)?;
            if seen.insert(vote_proof.authority_pubkey) {
                let stake = *authority_stakes
                    .get(&vote_proof.authority_pubkey)
                    .ok_or_else(|| {
                        "vote proof is signed by an unconfigured authority".to_string()
                    })?;
                accumulated_stake = accumulated_stake
                    .checked_add(stake)
                    .ok_or_else(|| "stake accumulation overflow".to_string())?;
            }
        }

        if accumulated_stake < request.threshold_stake {
            return Err("Solana vote quorum stake is below the configured threshold".into());
        }
        Ok(())
    }

    fn verify_expected_bindings(
        proof: &SolanaFinalizedBurnProofV1,
        request: &SolanaVerifyRequest,
    ) -> VerifyResult<()> {
        if proof.public_inputs.message_id.0 != request.expected_message_id {
            return Err("public inputs messageId does not match the SCCP payload".into());
        }
        if proof.public_inputs.router_program_id != request.expected_router_program_id {
            return Err(
                "public inputs router program id does not match the configured endpoint".into(),
            );
        }
        if proof.public_inputs.finalized_slot != proof.burn_proof.slot {
            return Err("burn proof slot does not match public inputs finalized slot".into());
        }
        if proof.public_inputs.finalized_bank_hash != proof.burn_proof.bank_hash {
            return Err(
                "burn proof bank hash does not match public inputs finalized bank hash".into(),
            );
        }
        Ok(())
    }

    fn authority_stake_map(
        authorities: &[SolanaVoteAuthorityConfigV1],
    ) -> VerifyResult<BTreeMap<[u8; 32], u64>> {
        if authorities.is_empty() {
            return Err("no Solana vote authorities are configured".into());
        }

        let mut map = BTreeMap::new();
        for authority in authorities {
            if authority.stake == 0 {
                return Err("configured Solana authority stake cannot be zero".into());
            }
            if map
                .insert(authority.authority_pubkey, authority.stake)
                .is_some()
            {
                return Err("duplicate Solana authority in configured set".into());
            }
        }
        Ok(map)
    }

    fn verify_bank_hash_proof(
        proof: &SolanaBankHashProofV1,
        expected_bank_hash: [u8; 32],
        expected_slot: u64,
    ) -> VerifyResult<SolanaAccountInfoV1> {
        if proof.slot != expected_slot {
            return Err("bank-hash proof slot mismatch".into());
        }
        let account = &proof.account_proof.account;
        if account.slot != expected_slot {
            return Err("account update slot mismatch".into());
        }

        let leaf_hash = hash_account(account);
        verify_merkle_proof(
            leaf_hash,
            &proof.account_proof.merkle_proof,
            proof.account_delta_root.0,
        )?;

        let reconstructed_bank_hash = bank_hash(
            proof.parent_bank_hash.0,
            proof.account_delta_root.0,
            proof.num_sigs,
            proof.blockhash.0,
        );
        if reconstructed_bank_hash != expected_bank_hash {
            return Err(
                "reconstructed bank hash does not match the supplied finalized bank hash".into(),
            );
        }

        Ok(account.clone())
    }

    fn verify_burn_account(
        burn_account: &SolanaAccountInfoV1,
        proof: &SolanaFinalizedBurnProofV1,
        request: &SolanaVerifyRequest,
    ) -> VerifyResult<()> {
        if burn_account.pubkey != proof.public_inputs.burn_record_pda {
            return Err("burn account pubkey does not match public inputs burn record PDA".into());
        }
        if burn_account.owner != proof.public_inputs.burn_record_owner {
            return Err("burn account owner does not match public inputs burn record owner".into());
        }
        if burn_account.owner != request.expected_router_program_id {
            return Err(
                "burn account owner does not match the configured Solana router program".into(),
            );
        }

        let burn_record = decode_burn_record(&burn_account.data)?;
        if burn_record.message_id != request.expected_message_id {
            return Err("burn record messageId does not match the SCCP payload".into());
        }
        if burn_record.slot != proof.public_inputs.finalized_slot {
            return Err("burn record slot does not match public inputs finalized slot".into());
        }

        let payload = decode_payload(&burn_record.payload)?;
        if payload.source_domain != SCCP_DOMAIN_SOL || payload.dest_domain != SCCP_DOMAIN_SORA {
            return Err("burn record payload is not a Solana -> SORA SCCP burn".into());
        }
        let recomputed_message_id = burn_message_id(&burn_record.payload);
        if recomputed_message_id != request.expected_message_id {
            return Err("burn record payload does not hash to the expected SCCP messageId".into());
        }

        let expected_data_hash = keccak_hash(&burn_account.data);
        if expected_data_hash != proof.public_inputs.burn_record_data_hash.0 {
            return Err("burn record data hash does not match public inputs".into());
        }

        let (expected_pda, expected_bump) = find_program_address(
            &[
                SCCP_SEED_PREFIX,
                SCCP_SEED_BURN,
                &request.expected_message_id,
            ],
            &request.expected_router_program_id,
        )?;
        if burn_account.pubkey != expected_pda {
            return Err("burn record PDA is not the canonical SCCP PDA for this messageId".into());
        }
        if burn_record.bump != expected_bump {
            return Err("burn record PDA bump does not match the canonical PDA derivation".into());
        }
        Ok(())
    }

    fn verify_vote_proof(
        vote_proof: &SolanaVoteProofV1,
        finalized_proof: &SolanaFinalizedBurnProofV1,
    ) -> VerifyResult<()> {
        let slot_hashes_account = verify_bank_hash_proof(
            &vote_proof.slot_hashes_proof,
            vote_proof.slot_hashes_proof.bank_hash.0,
            vote_proof.slot_hashes_proof.slot,
        )?;
        if slot_hashes_account.pubkey != SLOT_HASHES_SYSVAR_ID {
            return Err(
                "vote SlotHashes account proof does not target the SlotHashes sysvar".into(),
            );
        }
        if slot_hashes_account.owner != SYSVAR_PROGRAM_ID {
            return Err("vote SlotHashes account owner is not the Solana sysvar program".into());
        }

        let slot_hashes: SlotHashes = bincode::deserialize(&slot_hashes_account.data)
            .map_err(|e| format!("failed to decode SlotHashes sysvar account: {e}"))?;
        match slot_hashes.get(&finalized_proof.public_inputs.finalized_slot) {
            Some(hash) if *hash == finalized_proof.public_inputs.finalized_slot_hash.0 => {}
            Some(_) => {
                return Err(
                    "vote SlotHashes sysvar contains the target slot with a different slot hash"
                        .into(),
                )
            }
            None => {
                return Err(
                    "vote SlotHashes sysvar does not contain the target finalized slot".into(),
                )
            }
        }
        match slot_hashes.get(&vote_proof.vote_slot) {
            Some(hash) if *hash == vote_proof.vote_bank_hash.0 => {}
            Some(_) => {
                return Err(
                    "vote SlotHashes sysvar contains the voted slot with a different vote hash"
                        .into(),
                )
            }
            None => return Err("vote SlotHashes sysvar does not contain the voted slot".into()),
        }

        let message = decode_legacy_message(&vote_proof.signed_message)?;
        sanitize_message(&message)?;
        verify_vote_message_signature(&message, vote_proof)?;
        verify_vote_instruction(
            &message,
            vote_proof,
            finalized_proof.public_inputs.finalized_slot,
        )?;
        Ok(())
    }

    fn sanitize_message(message: &LocalMessage) -> VerifyResult<()> {
        let account_keys_len = message.account_keys.len();
        if usize::from(message.header.num_required_signatures)
            + usize::from(message.header.num_readonly_unsigned_accounts)
            > account_keys_len
        {
            return Err("vote message header exceeds account key length".into());
        }
        if message.header.num_readonly_signed_accounts >= message.header.num_required_signatures {
            return Err("vote message signer layout is invalid".into());
        }

        for instruction in &message.instructions {
            if usize::from(instruction.program_id_index) >= account_keys_len
                || instruction.program_id_index == 0
            {
                return Err("vote message program id index is invalid".into());
            }
            for account in &instruction.accounts {
                if usize::from(*account) >= account_keys_len {
                    return Err("vote instruction account index is out of bounds".into());
                }
            }
        }

        Ok(())
    }

    fn verify_vote_message_signature(
        message: &LocalMessage,
        vote_proof: &SolanaVoteProofV1,
    ) -> VerifyResult<()> {
        let signature = Signature::from_bytes(&vote_proof.signature);
        let public = VerifyingKey::from_bytes(&vote_proof.authority_pubkey)
            .map_err(|e| format!("vote authority public key is invalid: {e}"))?;
        if public
            .verify_strict(&vote_proof.signed_message, &signature)
            .is_err()
        {
            return Err("vote authority signature is invalid for the supplied message".into());
        }

        let signer_limit = usize::from(message.header.num_required_signatures);
        if !message.account_keys[..signer_limit].contains(&vote_proof.authority_pubkey) {
            return Err("vote authority is not a required signer of the vote message".into());
        }
        Ok(())
    }

    fn verify_vote_instruction(
        message: &LocalMessage,
        vote_proof: &SolanaVoteProofV1,
        target_slot: u64,
    ) -> VerifyResult<()> {
        let instruction = message
            .instructions
            .first()
            .ok_or_else(|| "vote message has no instructions".to_string())?;
        let program_id = message
            .account_keys
            .get(usize::from(instruction.program_id_index))
            .ok_or_else(|| "vote message program id index is out of bounds".to_string())?;
        if *program_id != VOTE_PROGRAM_ID {
            return Err("first vote message instruction is not the Solana vote program".into());
        }

        let vote_ix = decode_vote_instruction_data(&instruction.data)?;
        let authority_ix_index = authorized_voter_index(&vote_ix)?;
        let ix_authority = instruction
            .accounts
            .get(authority_ix_index)
            .and_then(|index| message.account_keys.get(usize::from(*index)))
            .ok_or_else(|| "vote authority account index is out of bounds".to_string())?;
        if *ix_authority != vote_proof.authority_pubkey {
            return Err("vote proof authority does not match the vote instruction signer".into());
        }

        let (vote_slot, vote_hash, rooted_slot) = vote_targets(&vote_ix)?;
        if vote_slot != vote_proof.vote_slot {
            return Err("vote proof vote slot does not match the signed vote instruction".into());
        }
        if vote_hash != vote_proof.vote_bank_hash.0 {
            return Err("vote proof bank hash does not match the signed vote instruction".into());
        }
        if let Some(root) = rooted_slot {
            if let Some(claimed_root) = vote_proof.rooted_slot {
                if claimed_root != root {
                    return Err(
                        "vote proof rooted slot does not match the signed vote instruction".into(),
                    );
                }
            }
            if root < target_slot {
                return Err("vote instruction root does not finalize the target burn slot".into());
            }
        }
        if vote_slot < target_slot {
            return Err("vote slot predates the target burn slot".into());
        }
        Ok(())
    }

    fn authorized_voter_index(vote_ix: &VoteInstruction) -> VerifyResult<usize> {
        match vote_ix {
            VoteInstruction::Vote(_) | VoteInstruction::VoteSwitch(_, _) => Ok(3),
            VoteInstruction::UpdateVoteState(_)
            | VoteInstruction::UpdateVoteStateSwitch(_, _)
            | VoteInstruction::CompactUpdateVoteState(_)
            | VoteInstruction::CompactUpdateVoteStateSwitch(_, _)
            | VoteInstruction::TowerSync(_)
            | VoteInstruction::TowerSyncSwitch(_, _) => Ok(1),
            _ => Err("unsupported vote instruction variant in proof".into()),
        }
    }

    fn vote_targets(vote_ix: &VoteInstruction) -> VerifyResult<(u64, SolHash, Option<u64>)> {
        match vote_ix {
            VoteInstruction::Vote(vote) | VoteInstruction::VoteSwitch(vote, _) => Ok((
                vote.last_voted_slot()
                    .ok_or_else(|| "vote instruction contains no slots".to_string())?,
                vote.hash,
                None,
            )),
            VoteInstruction::UpdateVoteState(update)
            | VoteInstruction::UpdateVoteStateSwitch(update, _)
            | VoteInstruction::CompactUpdateVoteState(update)
            | VoteInstruction::CompactUpdateVoteStateSwitch(update, _) => Ok((
                update
                    .last_voted_slot()
                    .ok_or_else(|| "vote-state update contains no lockouts".to_string())?,
                update.hash,
                update.root,
            )),
            VoteInstruction::TowerSync(tower_sync)
            | VoteInstruction::TowerSyncSwitch(tower_sync, _) => Ok((
                tower_sync
                    .last_voted_slot()
                    .ok_or_else(|| "tower-sync instruction contains no lockouts".to_string())?,
                tower_sync.hash,
                tower_sync.root,
            )),
            _ => Err("unsupported vote instruction variant in proof".into()),
        }
    }

    fn decode_burn_record(data: &[u8]) -> VerifyResult<SolanaBurnRecordAccountV1> {
        if data.len() != 203 {
            return Err("burn record account data has an unexpected length".into());
        }
        let mut payload = [0u8; SOLANA_BURN_PAYLOAD_ENCODED_LEN];
        payload.copy_from_slice(&data[34..131]);
        let mut message_id = [0u8; 32];
        message_id.copy_from_slice(&data[2..34]);
        let mut sender = [0u8; 32];
        sender.copy_from_slice(&data[131..163]);
        let mut mint = [0u8; 32];
        mint.copy_from_slice(&data[163..195]);
        let mut slot_bytes = [0u8; 8];
        slot_bytes.copy_from_slice(&data[195..203]);
        Ok(SolanaBurnRecordAccountV1 {
            version: data[0],
            bump: data[1],
            message_id,
            payload,
            sender,
            mint,
            slot: u64::from_le_bytes(slot_bytes),
        })
    }

    fn decode_payload(
        payload: &[u8; SOLANA_BURN_PAYLOAD_ENCODED_LEN],
    ) -> VerifyResult<BurnPayloadV1> {
        BurnPayloadV1::decode(&mut &payload[..])
            .map_err(|e| format!("failed to decode burn payload: {e:?}"))
    }

    fn burn_message_id(payload: &[u8]) -> [u8; 32] {
        keccak_hash([SCCP_MSG_PREFIX_BURN_V1, payload].concat())
    }

    fn verify_merkle_proof(
        leaf_hash: SolHash,
        proof: &SolanaMerkleProofV1,
        root: SolHash,
    ) -> VerifyResult<()> {
        if proof.path.len() != proof.siblings.len() {
            return Err("Merkle proof path length does not match sibling depth".into());
        }

        let mut current_hash = leaf_hash;
        for (index, siblings) in proof.path.iter().zip(&proof.siblings) {
            if usize::from(*index) >= SOLANA_MERKLE_FANOUT {
                return Err("Merkle proof path index exceeds Solana fanout".into());
            }
            if siblings.len() >= SOLANA_MERKLE_FANOUT {
                return Err("Merkle proof sibling set exceeds Solana fanout".into());
            }

            let mut level = Vec::with_capacity((siblings.len() + 1) * 32);
            let mut sibling_iter = siblings.iter();
            for position in 0..=siblings.len() {
                if position == usize::from(*index) {
                    level.extend_from_slice(&current_hash);
                } else if let Some(sibling) = sibling_iter.next() {
                    level.extend_from_slice(&sibling.0);
                }
            }
            if sibling_iter.next().is_some() {
                return Err("Merkle proof level contains too many siblings".into());
            }
            current_hash = sha2_hash(&level);
        }

        if current_hash != root {
            return Err("Merkle proof reconstructed a different root".into());
        }
        Ok(())
    }

    fn hash_account(account: &SolanaAccountInfoV1) -> SolHash {
        if account.lamports == 0 {
            return [0u8; 32];
        }

        let mut hasher = Blake3Hasher::new();
        hasher.update(&account.lamports.to_le_bytes());
        hasher.update(&account.rent_epoch.to_le_bytes());
        hasher.update(&account.data);
        hasher.update(&[u8::from(account.executable)]);
        hasher.update(&account.owner);
        hasher.update(&account.pubkey);
        hasher.finalize().into()
    }

    fn bank_hash(
        parent_bank_hash: SolHash,
        account_delta_root: SolHash,
        num_sigs: u64,
        blockhash: SolHash,
    ) -> SolHash {
        sha2_hashv(&[
            &parent_bank_hash,
            &account_delta_root,
            &num_sigs.to_le_bytes(),
            &blockhash,
        ])
    }

    fn keccak_hash(data: impl AsRef<[u8]>) -> SolHash {
        Keccak256::digest(data.as_ref()).into()
    }

    fn sha2_hash(data: impl AsRef<[u8]>) -> SolHash {
        Sha256::digest(data.as_ref()).into()
    }

    fn sha2_hashv(parts: &[&[u8]]) -> SolHash {
        let mut data = Vec::with_capacity(parts.iter().map(|part| part.len()).sum());
        for part in parts {
            data.extend_from_slice(part);
        }
        sha2_hash(data)
    }

    fn find_program_address(
        seeds: &[&[u8]],
        program_id: &[u8; 32],
    ) -> VerifyResult<([u8; 32], u8)> {
        for bump in (0u8..=u8::MAX).rev() {
            let mut bumped = seeds.to_vec();
            let bump_seed = [bump];
            bumped.push(&bump_seed);
            if let Ok(address) = create_program_address(&bumped, program_id) {
                return Ok((address, bump));
            }
        }
        Err("could not derive a valid Solana program address".into())
    }

    fn create_program_address(seeds: &[&[u8]], program_id: &[u8; 32]) -> VerifyResult<[u8; 32]> {
        if seeds.len() > SOLANA_MAX_SEEDS {
            return Err("too many Solana PDA seeds".into());
        }
        if seeds.iter().any(|seed| seed.len() > SOLANA_MAX_SEED_LEN) {
            return Err("Solana PDA seed length exceeds the protocol limit".into());
        }

        let mut data = Vec::new();
        for seed in seeds {
            data.extend_from_slice(seed);
        }
        data.extend_from_slice(program_id);
        data.extend_from_slice(PDA_MARKER);
        let candidate = sha2_hash(data);
        if bytes_are_curve_point(&candidate) {
            return Err("Solana PDA candidate is on the ed25519 curve".into());
        }
        Ok(candidate)
    }

    fn bytes_are_curve_point(bytes: &[u8; 32]) -> bool {
        CompressedEdwardsY(*bytes).decompress().is_some()
    }

    const fn hex32(hex: &str) -> [u8; 32] {
        let bytes = hex.as_bytes();
        let mut out = [0u8; 32];
        let mut i = 0;
        while i < 32 {
            out[i] = (hex_nibble(bytes[i * 2]) << 4) | hex_nibble(bytes[i * 2 + 1]);
            i += 1;
        }
        out
    }

    const fn hex_nibble(byte: u8) -> u8 {
        match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            b'A'..=b'F' => byte - b'A' + 10,
            _ => panic!("invalid hex digit"),
        }
    }

    mod short_vec {
        use serde::{
            de::{self, Deserializer, SeqAccess, Visitor},
            ser::{self, SerializeTuple, Serializer},
            Deserialize, Serialize,
        };
        use std::{convert::TryFrom, fmt, marker::PhantomData};

        #[derive(Default)]
        pub struct ShortU16(pub u16);

        impl Serialize for ShortU16 {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                let mut seq = serializer.serialize_tuple(1)?;
                let mut rem_val = self.0;
                loop {
                    let mut elem = (rem_val & 0x7f) as u8;
                    rem_val >>= 7;
                    if rem_val == 0 {
                        seq.serialize_element(&elem)?;
                        break;
                    } else {
                        elem |= 0x80;
                        seq.serialize_element(&elem)?;
                    }
                }
                seq.end()
            }
        }

        enum VisitStatus {
            Done(u16),
            More(u16),
        }

        #[derive(Debug)]
        enum VisitError {
            TooLong(usize),
            TooShort(usize),
            Overflow(u32),
            Alias,
            ByteThreeContinues,
        }

        impl VisitError {
            fn into_de_error<'de, A>(self) -> A::Error
            where
                A: SeqAccess<'de>,
            {
                match self {
                    VisitError::TooLong(len) => {
                        de::Error::invalid_length(len, &"three or fewer bytes")
                    }
                    VisitError::TooShort(len) => de::Error::invalid_length(len, &"more bytes"),
                    VisitError::Overflow(val) => de::Error::invalid_value(
                        de::Unexpected::Unsigned(val as u64),
                        &"a value in the range [0, 65535]",
                    ),
                    VisitError::Alias => de::Error::invalid_value(
                        de::Unexpected::Other("alias encoding"),
                        &"strict form encoding",
                    ),
                    VisitError::ByteThreeContinues => de::Error::invalid_value(
                        de::Unexpected::Other("continue signal on byte-three"),
                        &"a terminal signal on or before byte-three",
                    ),
                }
            }
        }

        const MAX_ENCODING_LENGTH: usize = 3;

        fn visit_byte(elem: u8, val: u16, nth_byte: usize) -> Result<VisitStatus, VisitError> {
            if elem == 0 && nth_byte != 0 {
                return Err(VisitError::Alias);
            }

            let val = u32::from(val);
            let elem = u32::from(elem);
            let elem_val = elem & 0x7f;
            let elem_done = (elem & 0x80) == 0;

            if nth_byte >= MAX_ENCODING_LENGTH {
                return Err(VisitError::TooLong(nth_byte.saturating_add(1)));
            } else if nth_byte == MAX_ENCODING_LENGTH.saturating_sub(1) && !elem_done {
                return Err(VisitError::ByteThreeContinues);
            }

            let shift = u32::try_from(nth_byte)
                .unwrap_or(u32::MAX)
                .saturating_mul(7);
            let elem_val = elem_val.checked_shl(shift).unwrap_or(u32::MAX);
            let new_val = val | elem_val;
            let val = u16::try_from(new_val).map_err(|_| VisitError::Overflow(new_val))?;

            if elem_done {
                Ok(VisitStatus::Done(val))
            } else {
                Ok(VisitStatus::More(val))
            }
        }

        struct ShortU16Visitor;

        impl<'de> Visitor<'de> for ShortU16Visitor {
            type Value = ShortU16;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a ShortU16")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<ShortU16, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut val: u16 = 0;
                for nth_byte in 0..MAX_ENCODING_LENGTH {
                    let elem: u8 = seq.next_element()?.ok_or_else(|| {
                        VisitError::TooShort(nth_byte.saturating_add(1)).into_de_error::<A>()
                    })?;
                    match visit_byte(elem, val, nth_byte).map_err(|e| e.into_de_error::<A>())? {
                        VisitStatus::Done(new_val) => return Ok(ShortU16(new_val)),
                        VisitStatus::More(new_val) => val = new_val,
                    }
                }
                Err(VisitError::ByteThreeContinues.into_de_error::<A>())
            }
        }

        impl<'de> Deserialize<'de> for ShortU16 {
            fn deserialize<D>(deserializer: D) -> Result<ShortU16, D::Error>
            where
                D: Deserializer<'de>,
            {
                deserializer.deserialize_tuple(3, ShortU16Visitor)
            }
        }

        pub fn serialize<S: Serializer, T: Serialize>(
            elements: &[T],
            serializer: S,
        ) -> Result<S::Ok, S::Error> {
            let mut seq = serializer.serialize_tuple(1)?;
            if elements.len() > u16::MAX as usize {
                return Err(ser::Error::custom("length larger than u16"));
            }
            seq.serialize_element(&ShortU16(elements.len() as u16))?;
            for element in elements {
                seq.serialize_element(element)?;
            }
            seq.end()
        }

        struct ShortVecVisitor<T> {
            marker: PhantomData<T>,
        }

        impl<'de, T> Visitor<'de> for ShortVecVisitor<T>
        where
            T: Deserialize<'de>,
        {
            type Value = Vec<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a Vec with a multi-byte length")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Vec<T>, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let short_len: ShortU16 = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let mut result = Vec::with_capacity(short_len.0 as usize);
                for i in 0..short_len.0 {
                    let elem = seq
                        .next_element()?
                        .ok_or_else(|| de::Error::invalid_length(i as usize, &self))?;
                    result.push(elem);
                }
                Ok(result)
            }
        }

        pub fn deserialize<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
        where
            D: Deserializer<'de>,
            T: Deserialize<'de>,
        {
            deserializer.deserialize_tuple(
                1,
                ShortVecVisitor {
                    marker: PhantomData,
                },
            )
        }
    }

    mod serde_varint {
        use serde::{
            de::{Error as _, SeqAccess, Visitor},
            ser::SerializeTuple,
            Deserializer, Serializer,
        };
        use std::{fmt, marker::PhantomData};

        pub trait VarInt: Sized {
            fn visit_seq<'de, A>(seq: A) -> Result<Self, A::Error>
            where
                A: SeqAccess<'de>;

            fn serialize<S>(self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer;
        }

        struct VarIntVisitor<T> {
            marker: PhantomData<T>,
        }

        impl<'de, T> Visitor<'de> for VarIntVisitor<T>
        where
            T: VarInt,
        {
            type Value = T;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a VarInt")
            }

            fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                T::visit_seq(seq)
            }
        }

        pub fn serialize<S, T>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
            T: Copy + VarInt,
        {
            (*value).serialize(serializer)
        }

        pub fn deserialize<'de, D, T>(deserializer: D) -> Result<T, D::Error>
        where
            D: Deserializer<'de>,
            T: VarInt,
        {
            deserializer.deserialize_tuple(
                (core::mem::size_of::<T>() * 8 + 6) / 7,
                VarIntVisitor {
                    marker: PhantomData,
                },
            )
        }

        macro_rules! impl_var_int {
            ($type:ty) => {
                impl VarInt for $type {
                    fn visit_seq<'de, A>(mut seq: A) -> Result<Self, A::Error>
                    where
                        A: SeqAccess<'de>,
                    {
                        let mut out = 0;
                        let mut shift = 0u32;
                        while shift < <$type>::BITS {
                            let Some(byte) = seq.next_element::<u8>()? else {
                                return Err(A::Error::custom("Invalid Sequence"));
                            };
                            out |= ((byte & 0x7f) as Self) << shift;
                            if byte & 0x80 == 0 {
                                if (out >> shift) as u8 != byte {
                                    return Err(A::Error::custom("Last Byte Truncated"));
                                }
                                if byte == 0u8 && (shift != 0 || out != 0) {
                                    return Err(A::Error::custom("Invalid Trailing Zeros"));
                                }
                                return Ok(out);
                            }
                            shift += 7;
                        }
                        Err(A::Error::custom("Left Shift Overflows"))
                    }

                    fn serialize<S>(mut self, serializer: S) -> Result<S::Ok, S::Error>
                    where
                        S: Serializer,
                    {
                        let bits = <$type>::BITS - self.leading_zeros();
                        let num_bytes = ((bits + 6) / 7).max(1) as usize;
                        let mut seq = serializer.serialize_tuple(num_bytes)?;
                        while self >= 0x80 {
                            let byte = ((self & 0x7f) | 0x80) as u8;
                            seq.serialize_element(&byte)?;
                            self >>= 7;
                        }
                        seq.serialize_element(&(self as u8))?;
                        seq.end()
                    }
                }
            };
        }

        impl_var_int!(u16);
        impl_var_int!(u32);
        impl_var_int!(u64);
    }

    mod serde_compact_vote_state_update {
        use serde::{Deserialize, Deserializer, Serialize, Serializer};

        use super::{serde_varint, short_vec, Lockout, SolHash, VoteStateUpdate};

        #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
        struct LockoutOffset {
            #[serde(with = "serde_varint")]
            offset: u64,
            confirmation_count: u8,
        }

        #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
        struct CompactVoteStateUpdate {
            root: u64,
            #[serde(with = "short_vec")]
            lockout_offsets: Vec<LockoutOffset>,
            hash: SolHash,
            timestamp: Option<i64>,
        }

        pub fn serialize<S>(
            vote_state_update: &VoteStateUpdate,
            serializer: S,
        ) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let mut previous = vote_state_update.root.unwrap_or_default();
            let mut offsets = Vec::with_capacity(vote_state_update.lockouts.len());
            for lockout in &vote_state_update.lockouts {
                let offset = lockout
                    .slot()
                    .checked_sub(previous)
                    .ok_or_else(|| serde::ser::Error::custom("Invalid vote lockout"))?;
                let confirmation_count = u8::try_from(lockout.confirmation_count)
                    .map_err(|_| serde::ser::Error::custom("Invalid confirmation count"))?;
                offsets.push(LockoutOffset {
                    offset,
                    confirmation_count,
                });
                previous = lockout.slot();
            }
            CompactVoteStateUpdate {
                root: vote_state_update.root.unwrap_or(u64::MAX),
                lockout_offsets: offsets,
                hash: vote_state_update.hash,
                timestamp: vote_state_update.timestamp,
            }
            .serialize(serializer)
        }

        pub fn deserialize<'de, D>(deserializer: D) -> Result<VoteStateUpdate, D::Error>
        where
            D: Deserializer<'de>,
        {
            let compact = CompactVoteStateUpdate::deserialize(deserializer)?;
            let root = (compact.root != u64::MAX).then_some(compact.root);
            let mut previous = root.unwrap_or_default();
            let mut lockouts = std::collections::VecDeque::new();
            for lockout_offset in compact.lockout_offsets {
                previous = previous
                    .checked_add(lockout_offset.offset)
                    .ok_or_else(|| serde::de::Error::custom("Invalid lockout offset"))?;
                lockouts.push_back(Lockout::new_with_confirmation_count(
                    previous,
                    u32::from(lockout_offset.confirmation_count),
                ));
            }
            Ok(VoteStateUpdate {
                lockouts,
                root,
                hash: compact.hash,
                timestamp: compact.timestamp,
            })
        }
    }

    #[cfg(test)]
    mod tests {
        use codec::Encode;
        use ed25519_dalek::{Signer, SigningKey};

        use super::*;

        fn make_bank_hash_proof(account: SolanaAccountInfoV1) -> SolanaBankHashProofV1 {
            let account_hash = hash_account(&account);
            let proof = SolanaMerkleProofV1 {
                path: Vec::new(),
                siblings: Vec::new(),
            };
            let parent_bank_hash = [0x31; 32];
            let blockhash = [0x32; 32];
            let num_sigs = 2u64;
            let bank_hash = bank_hash(parent_bank_hash, account_hash, num_sigs, blockhash);
            SolanaBankHashProofV1 {
                slot: account.slot,
                bank_hash: bank_hash.into(),
                account_delta_root: account_hash.into(),
                parent_bank_hash: parent_bank_hash.into(),
                blockhash: blockhash.into(),
                num_sigs,
                account_proof: SolanaAccountDeltaProofV1 {
                    account,
                    merkle_proof: proof,
                },
            }
        }

        fn burn_record_account(
            router_program_id: [u8; 32],
            payload: BurnPayloadV1,
            slot: u64,
        ) -> (SolanaAccountInfoV1, [u8; 32], [u8; 32], [u8; 32]) {
            let payload_bytes = payload.encode();
            let message_id =
                keccak_hash([SCCP_MSG_PREFIX_BURN_V1, payload_bytes.as_slice()].concat());
            let (burn_record_pda, bump) = find_program_address(
                &[SCCP_SEED_PREFIX, SCCP_SEED_BURN, &message_id],
                &router_program_id,
            )
            .unwrap();

            let mut payload_array = [0u8; SOLANA_BURN_PAYLOAD_ENCODED_LEN];
            payload_array.copy_from_slice(&payload_bytes);
            let record = SolanaBurnRecordAccountV1 {
                version: 1,
                bump,
                message_id,
                payload: payload_array,
                sender: [0x21; 32],
                mint: [0x22; 32],
                slot,
            };
            let account_data = record.encode_account_data().to_vec();
            (
                SolanaAccountInfoV1 {
                    pubkey: burn_record_pda,
                    lamports: 1,
                    owner: router_program_id,
                    executable: false,
                    rent_epoch: 0,
                    data: account_data,
                    write_version: 1,
                    slot,
                },
                burn_record_pda,
                keccak_hash(record.encode_account_data()),
                message_id,
            )
        }

        fn slot_hashes_account(
            entries: &[(u64, [u8; 32])],
            proof_slot: u64,
        ) -> SolanaAccountInfoV1 {
            let slot_hashes = SlotHashes(entries.to_vec());
            SolanaAccountInfoV1 {
                pubkey: SLOT_HASHES_SYSVAR_ID,
                lamports: 1,
                owner: SYSVAR_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
                data: bincode::serialize(&slot_hashes).unwrap(),
                write_version: 1,
                slot: proof_slot,
            }
        }

        fn signed_vote_message(
            authority: &SigningKey,
            vote_slot: u64,
            vote_hash: [u8; 32],
        ) -> (Vec<u8>, [u8; 64], Option<u64>) {
            let instruction = LocalCompiledInstruction {
                program_id_index: 2,
                accounts: vec![1, 0],
                data: serialize_vote_instruction_data(&VoteInstruction::CompactUpdateVoteState(
                    VoteStateUpdate {
                        lockouts: VecDeque::from(vec![Lockout::new(vote_slot)]),
                        root: Some(vote_slot),
                        hash: vote_hash,
                        timestamp: None,
                    },
                ))
                .unwrap(),
            };
            let message = LocalMessage {
                header: LocalMessageHeader {
                    num_required_signatures: 1,
                    num_readonly_signed_accounts: 0,
                    num_readonly_unsigned_accounts: 1,
                },
                account_keys: vec![
                    authority.verifying_key().to_bytes(),
                    [0x51; 32],
                    VOTE_PROGRAM_ID,
                ],
                recent_blockhash: [0x09; 32],
                instructions: vec![instruction],
            };
            let signed_message = encode_legacy_message(&message);
            let signature = authority.sign(&signed_message).to_bytes();
            (signed_message, signature, Some(vote_slot))
        }

        #[test]
        fn accepts_valid_vote_quorum_proof() {
            let authority = SigningKey::from_bytes(&[7u8; 32]);
            let router_program_id = [0x14; 32];
            let payload = BurnPayloadV1 {
                version: 1,
                source_domain: SCCP_DOMAIN_SOL,
                dest_domain: SCCP_DOMAIN_SORA,
                nonce: 7,
                sora_asset_id: [0x55; 32],
                amount: 42,
                recipient: [0x77; 32],
            };
            let slot = 42u64;
            let (burn_account, burn_record_pda, burn_record_data_hash, message_id) =
                burn_record_account(router_program_id, payload, slot);
            let burn_proof = make_bank_hash_proof(burn_account);

            let vote_slot = 43u64;
            let finalized_slot_hash = [0x41; 32];
            let vote_hash = [0x42; 32];
            let slot_hashes_proof = make_bank_hash_proof(slot_hashes_account(
                &[(vote_slot, vote_hash), (slot, finalized_slot_hash)],
                vote_slot,
            ));
            let (signed_message, signature, rooted_slot) =
                signed_vote_message(&authority, vote_slot, vote_hash);

            let proof = SolanaFinalizedBurnProofV1 {
                version: SOLANA_FINALIZED_BURN_PROOF_VERSION_V1,
                public_inputs: SolanaFinalizedBurnPublicInputsV1 {
                    message_id: message_id.into(),
                    finalized_slot: slot,
                    finalized_bank_hash: burn_proof.bank_hash,
                    finalized_slot_hash: finalized_slot_hash.into(),
                    router_program_id,
                    burn_record_pda,
                    burn_record_owner: router_program_id,
                    burn_record_data_hash: burn_record_data_hash.into(),
                },
                burn_proof,
                vote_proofs: vec![SolanaVoteProofV1 {
                    authority_pubkey: authority.verifying_key().to_bytes(),
                    signature,
                    signed_message,
                    vote_slot,
                    vote_bank_hash: vote_hash.into(),
                    rooted_slot,
                    slot_hashes_proof,
                }],
            };

            let request = SolanaVerifyRequest {
                proof: proof.encode(),
                expected_message_id: message_id,
                expected_router_program_id: router_program_id,
                authorities: vec![SolanaVoteAuthorityConfigV1 {
                    authority_pubkey: authority.verifying_key().to_bytes(),
                    stake: 100,
                }],
                threshold_stake: 67,
            };

            assert!(verify_solana_finalized_burn_proof(&request).is_ok());
        }

        #[test]
        fn rejects_vote_signature_mismatch() {
            let authority = SigningKey::from_bytes(&[7u8; 32]);
            let other = SigningKey::from_bytes(&[8u8; 32]);
            let router_program_id = [0x14; 32];
            let payload = BurnPayloadV1 {
                version: 1,
                source_domain: SCCP_DOMAIN_SOL,
                dest_domain: SCCP_DOMAIN_SORA,
                nonce: 7,
                sora_asset_id: [0x55; 32],
                amount: 42,
                recipient: [0x77; 32],
            };
            let slot = 42u64;
            let (burn_account, burn_record_pda, burn_record_data_hash, message_id) =
                burn_record_account(router_program_id, payload, slot);
            let burn_proof = make_bank_hash_proof(burn_account);
            let vote_slot = 43u64;
            let finalized_slot_hash = [0x41; 32];
            let vote_hash = [0x42; 32];
            let slot_hashes_proof = make_bank_hash_proof(slot_hashes_account(
                &[(vote_slot, vote_hash), (slot, finalized_slot_hash)],
                vote_slot,
            ));
            let (signed_message, _signature, rooted_slot) =
                signed_vote_message(&authority, vote_slot, vote_hash);
            let forged_signature = other.sign(&signed_message).to_bytes();

            let proof = SolanaFinalizedBurnProofV1 {
                version: SOLANA_FINALIZED_BURN_PROOF_VERSION_V1,
                public_inputs: SolanaFinalizedBurnPublicInputsV1 {
                    message_id: message_id.into(),
                    finalized_slot: slot,
                    finalized_bank_hash: burn_proof.bank_hash,
                    finalized_slot_hash: finalized_slot_hash.into(),
                    router_program_id,
                    burn_record_pda,
                    burn_record_owner: router_program_id,
                    burn_record_data_hash: burn_record_data_hash.into(),
                },
                burn_proof,
                vote_proofs: vec![SolanaVoteProofV1 {
                    authority_pubkey: authority.verifying_key().to_bytes(),
                    signature: forged_signature,
                    signed_message,
                    vote_slot,
                    vote_bank_hash: vote_hash.into(),
                    rooted_slot,
                    slot_hashes_proof,
                }],
            };

            let request = SolanaVerifyRequest {
                proof: proof.encode(),
                expected_message_id: message_id,
                expected_router_program_id: router_program_id,
                authorities: vec![SolanaVoteAuthorityConfigV1 {
                    authority_pubkey: authority.verifying_key().to_bytes(),
                    stake: 100,
                }],
                threshold_stake: 67,
            };

            assert!(verify_solana_finalized_burn_proof(&request).is_err());
        }
    }
}
