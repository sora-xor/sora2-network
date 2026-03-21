#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use sp_runtime_interface::{pass_by::PassFatPointerAndDecode, runtime_interface};
use sp_std::vec::Vec;

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct TonVerifyRequest {
    pub trusted_checkpoint_seqno: u32,
    pub trusted_checkpoint_hash: [u8; 32],
    pub proof: Vec<u8>,
    pub expected_master_account_id: [u8; 32],
    pub expected_code_hash: [u8; 32],
    pub expected_message_id: [u8; 32],
    pub expected_dest_domain: u32,
    pub expected_recipient32: [u8; 32],
    pub expected_amount: u128,
    pub expected_nonce: u64,
}

#[runtime_interface]
pub trait TonProofApi {
    fn verify_ton_burn_proof(request: PassFatPointerAndDecode<TonVerifyRequest>) -> bool {
        #[cfg(feature = "std")]
        {
            verifier::verify_ton_burn_proof(&request).is_ok()
        }

        #[cfg(not(feature = "std"))]
        {
            let _ = request;
            false
        }
    }
}

#[cfg(feature = "std")]
pub use verifier::{build_test_fixture, TonTestFixture, TonTestFixtureInput};

#[cfg(feature = "std")]
mod verifier {
    use codec::{Decode, Encode};
    use ed25519_dalek::{Signer, SigningKey};
    use everscale_crypto::tl;
    use everscale_types::boc::Boc;
    use everscale_types::cell::{
        Cell, CellBuilder, CellFamily, HashBytes, UsageTree, UsageTreeMode,
    };
    use everscale_types::dict::{AugDict, Dict};
    use everscale_types::merkle::MerkleProof;
    use everscale_types::models::config::{
        CatchainConfig, ConfigParam34, ValidatorDescription, ValidatorSet,
    };
    use everscale_types::models::currency::CurrencyCollection;
    use everscale_types::models::message::{IntAddr, StdAddr};
    use everscale_types::models::{
        account::{Account, AccountState, OptionalAccount, ShardAccount, StateInit, StorageInfo},
        block::{
            Block, BlockExtra, BlockId, BlockInfo, BlockProof, BlockRef, BlockSignature,
            BlockSignatureExt, McBlockExtra, PrevBlockRef, ShardDescription, ShardIdent, ValueFlow,
        },
        shard::{
            DepthBalanceInfo, McStateExtra, ShardAccounts, ShardStateUnsplit, ValidatorBaseInfo,
            ValidatorInfo,
        },
        Lazy,
    };
    use everscale_types::num::Tokens;
    use std::num::NonZeroU16;

    use crate::TonVerifyRequest;

    const PROOF_VERSION: u8 = 1;
    const MASTERCHAIN_SECTION_VERSION_V1: u8 = 1;
    const MASTERCHAIN_SECTION_VERSION_V2: u8 = 2;
    const SHARD_SECTION_VERSION: u8 = 1;
    const SORA_DOMAIN: u32 = 0;

    type VerifyResult<T> = Result<T, String>;

    #[derive(Debug, Encode, Decode, Clone, PartialEq, Eq)]
    struct TonBurnRecordV1 {
        dest_domain: u32,
        recipient32: [u8; 32],
        jetton_amount: u128,
        nonce: u64,
    }

    #[derive(Debug, Encode, Decode, Clone, PartialEq, Eq)]
    struct TonBurnProofV1 {
        version: u8,
        trusted_checkpoint_seqno: u32,
        trusted_checkpoint_hash: [u8; 32],
        target_mc_seqno: u32,
        target_mc_block_hash: [u8; 32],
        jetton_master_account_id: [u8; 32],
        jetton_master_code_hash: [u8; 32],
        burn_message_id: [u8; 32],
        burn_record: TonBurnRecordV1,
        masterchain_proof: Vec<u8>,
        shard_proof: Vec<u8>,
        account_proof: Vec<u8>,
        burns_dict_proof: Vec<u8>,
    }

    #[derive(Debug, Encode, Decode, Clone, PartialEq, Eq)]
    struct TonMasterchainProofSectionV1 {
        version: u8,
        checkpoint_block_boc: Vec<u8>,
        checkpoint_state_extra_proof_boc: Vec<u8>,
        target_block_proof_boc: Vec<u8>,
        target_state_extra_proof_boc: Vec<u8>,
    }

    #[derive(Debug, Encode, Decode, Clone, PartialEq, Eq)]
    struct TonLiteBlockSignatureV2 {
        node_id_short: [u8; 32],
        signature: [u8; 64],
    }

    #[derive(Debug, Encode, Decode, Clone, PartialEq, Eq)]
    struct TonLiteSignatureSetV2 {
        validator_list_hash_short: u32,
        catchain_seqno: u32,
        signatures: Vec<TonLiteBlockSignatureV2>,
    }

    #[derive(Debug, Encode, Decode, Clone, PartialEq, Eq)]
    struct TonMasterchainProofSectionV2 {
        version: u8,
        checkpoint_block_boc: Vec<u8>,
        checkpoint_state_extra_proof_boc: Vec<u8>,
        target_block_boc: Vec<u8>,
        target_signatures: TonLiteSignatureSetV2,
        target_state_extra_proof_boc: Vec<u8>,
    }

    #[derive(Debug, Encode, Decode, Clone, PartialEq, Eq)]
    struct TonShardProofSectionV1 {
        version: u8,
        shard_block_boc: Vec<u8>,
        shard_state_accounts_proof_boc: Vec<u8>,
    }

    #[derive(Clone, everscale_types::cell::Store, everscale_types::cell::Load)]
    struct ContractBurnRecord {
        burn_initiator: IntAddr,
        dest_domain: u32,
        recipient32: HashBytes,
        jetton_amount: Tokens,
        nonce: u64,
    }

    #[derive(Clone, everscale_types::cell::Store, everscale_types::cell::Load)]
    struct ContractSccpStorageExtra {
        sora_asset_id: HashBytes,
        nonce: u64,
        inbound_paused_mask: u64,
        outbound_paused_mask: u64,
        invalidated_inbound: Dict<HashBytes, bool>,
        processed_inbound: Dict<HashBytes, bool>,
        burns: Dict<HashBytes, Cell>,
    }

    #[derive(Clone, everscale_types::cell::Store, everscale_types::cell::Load)]
    struct ContractMinterStorage {
        total_supply: Tokens,
        governor_address: IntAddr,
        verifier_address: Option<IntAddr>,
        jetton_wallet_code: Cell,
        metadata_uri: Cell,
        sccp: Cell,
    }

    pub(crate) fn verify_ton_burn_proof(request: &TonVerifyRequest) -> VerifyResult<()> {
        let decoded = decode_exact::<TonBurnProofV1>(&request.proof, "outer TON burn proof")?;
        if decoded.version != PROOF_VERSION {
            return Err("unsupported TON burn proof version".into());
        }
        if decoded.trusted_checkpoint_seqno != request.trusted_checkpoint_seqno
            || decoded.trusted_checkpoint_hash != request.trusted_checkpoint_hash
        {
            return Err("trusted checkpoint mismatch".into());
        }
        if decoded.jetton_master_account_id != request.expected_master_account_id {
            return Err("jetton master account id mismatch".into());
        }
        if decoded.jetton_master_code_hash != request.expected_code_hash {
            return Err("jetton master code hash mismatch".into());
        }
        if decoded.burn_message_id != request.expected_message_id {
            return Err("burn message id mismatch".into());
        }
        if decoded.burn_record.dest_domain != request.expected_dest_domain
            || decoded.burn_record.dest_domain != SORA_DOMAIN
            || decoded.burn_record.recipient32 != request.expected_recipient32
            || decoded.burn_record.jetton_amount != request.expected_amount
            || decoded.burn_record.nonce != request.expected_nonce
        {
            return Err("outer burn record does not match expected payload".into());
        }

        let masterchain_section_version = decoded
            .masterchain_proof
            .first()
            .copied()
            .ok_or_else(|| "masterchain proof section is empty".to_string())?;
        let (
            checkpoint_block_boc,
            checkpoint_state_extra_proof_boc,
            target_block_root,
            target_block,
            target_block_id,
            target_signatures,
            target_signature_info,
            target_state_extra_proof_boc,
        ) = match masterchain_section_version {
            MASTERCHAIN_SECTION_VERSION_V1 => {
                let masterchain_section = decode_exact::<TonMasterchainProofSectionV1>(
                    &decoded.masterchain_proof,
                    "masterchain proof section",
                )?;
                let target_block_proof_cell = decode_boc(
                    &masterchain_section.target_block_proof_boc,
                    "target masterchain block proof",
                )?;
                let target_block_proof = target_block_proof_cell
                    .as_ref()
                    .parse::<BlockProof>()
                    .map_err(|e| format!("target masterchain block proof parse failed: {e}"))?;
                if target_block_proof.proof_for.shard != ShardIdent::MASTERCHAIN {
                    return Err("target masterchain proof is not for the masterchain".into());
                }
                if target_block_proof.proof_for.seqno != decoded.target_mc_seqno
                    || target_block_proof.proof_for.root_hash
                        != hash_bytes(decoded.target_mc_block_hash)
                {
                    return Err("target masterchain block id mismatch".into());
                }
                let target_block_signatures = target_block_proof
                    .signatures
                    .clone()
                    .ok_or_else(|| "target masterchain proof is missing signatures".to_string())?;
                let (target_block_root, target_block) =
                    parse_block_from_proof(&target_block_proof)?;
                (
                    masterchain_section.checkpoint_block_boc,
                    masterchain_section.checkpoint_state_extra_proof_boc,
                    target_block_root,
                    target_block,
                    target_block_proof.proof_for,
                    target_block_signatures.signatures,
                    target_block_signatures.validator_info,
                    masterchain_section.target_state_extra_proof_boc,
                )
            }
            MASTERCHAIN_SECTION_VERSION_V2 => {
                let masterchain_section = decode_exact::<TonMasterchainProofSectionV2>(
                    &decoded.masterchain_proof,
                    "masterchain proof section",
                )?;
                let target_block_root = decode_boc(
                    &masterchain_section.target_block_boc,
                    "target masterchain block",
                )?;
                let target_block = target_block_root
                    .as_ref()
                    .parse::<Block>()
                    .map_err(|e| format!("target masterchain block parse failed: {e}"))?;
                let mut signatures = Dict::<u16, BlockSignature>::new();
                for (index, signature) in masterchain_section
                    .target_signatures
                    .signatures
                    .into_iter()
                    .enumerate()
                {
                    signatures
                        .set(
                            index as u16,
                            BlockSignature {
                                node_id_short: HashBytes(signature.node_id_short),
                                signature: everscale_types::models::block::Signature(
                                    signature.signature,
                                ),
                            },
                        )
                        .map_err(|e| {
                            format!("target masterchain signature set build failed: {e}")
                        })?;
                }
                let target_block_boc = Boc::encode(target_block_root.as_ref());
                (
                    masterchain_section.checkpoint_block_boc,
                    masterchain_section.checkpoint_state_extra_proof_boc,
                    target_block_root,
                    target_block,
                    BlockId {
                        shard: ShardIdent::MASTERCHAIN,
                        seqno: decoded.target_mc_seqno,
                        root_hash: hash_bytes(decoded.target_mc_block_hash),
                        file_hash: Boc::file_hash(&target_block_boc),
                    },
                    signatures,
                    ValidatorBaseInfo {
                        validator_list_hash_short: masterchain_section
                            .target_signatures
                            .validator_list_hash_short,
                        catchain_seqno: masterchain_section.target_signatures.catchain_seqno,
                    },
                    masterchain_section.target_state_extra_proof_boc,
                )
            }
            _ => return Err("unsupported masterchain proof section version".into()),
        };
        let shard_section =
            decode_exact::<TonShardProofSectionV1>(&decoded.shard_proof, "shard proof section")?;
        if shard_section.version != SHARD_SECTION_VERSION {
            return Err("unsupported shard proof section version".into());
        }

        let checkpoint_block_cell = decode_boc(&checkpoint_block_boc, "checkpoint block")?;
        let checkpoint_block = checkpoint_block_cell
            .as_ref()
            .parse::<Block>()
            .map_err(|e| format!("checkpoint block parse failed: {e}"))?;
        let checkpoint_info = checkpoint_block
            .load_info()
            .map_err(|e| format!("checkpoint block info parse failed: {e}"))?;
        ensure_masterchain_block(
            &checkpoint_block_cell,
            &checkpoint_info,
            request.trusted_checkpoint_seqno,
            &request.trusted_checkpoint_hash,
            None,
        )?;
        let checkpoint_state_hash = checkpoint_block
            .load_state_update()
            .map_err(|e| format!("checkpoint state update parse failed: {e}"))?
            .new_hash;
        let checkpoint_state = parse_state_proof(
            &checkpoint_state_extra_proof_boc,
            &checkpoint_state_hash,
            "checkpoint masterchain state proof",
        )?;
        if checkpoint_state.seqno != checkpoint_info.seqno
            || checkpoint_state.shard_ident != ShardIdent::MASTERCHAIN
        {
            return Err("checkpoint masterchain state does not match checkpoint block".into());
        }
        let checkpoint_extra = checkpoint_state
            .load_custom()
            .map_err(|e| format!("checkpoint custom state parse failed: {e}"))?
            .ok_or_else(|| "checkpoint masterchain state lacks custom data".to_string())?;

        let catchain_config = checkpoint_extra
            .config
            .get_catchain_config()
            .map_err(|e| format!("checkpoint catchain config load failed: {e}"))?;
        let validator_set = checkpoint_extra
            .config
            .get_current_validator_set()
            .map_err(|e| format!("checkpoint current validator set load failed: {e}"))?;

        if decoded.target_mc_seqno < request.trusted_checkpoint_seqno {
            return Err("target masterchain block precedes the trusted checkpoint".into());
        }
        let target_info = target_block
            .load_info()
            .map_err(|e| format!("target masterchain block info parse failed: {e}"))?;
        ensure_masterchain_block(
            &target_block_root,
            &target_info,
            decoded.target_mc_seqno,
            &decoded.target_mc_block_hash,
            Some(&target_block_id.file_hash.0),
        )?;
        if target_info.gen_catchain_seqno != checkpoint_extra.validator_info.catchain_seqno {
            return Err(
                "target masterchain block is outside the trusted checkpoint validator session"
                    .into(),
            );
        }
        let (subset, hash_short) = validator_set
            .compute_mc_subset(
                target_info.gen_catchain_seqno,
                catchain_config.shuffle_mc_validators,
            )
            .ok_or_else(|| "failed to compute masterchain validator subset".to_string())?;
        if hash_short != target_info.gen_validator_list_hash_short {
            return Err("target masterchain block validator subset hash mismatch".into());
        }
        if target_signature_info.catchain_seqno != target_info.gen_catchain_seqno
            || target_signature_info.validator_list_hash_short
                != target_info.gen_validator_list_hash_short
        {
            return Err("target masterchain signature metadata mismatch".into());
        }
        let expected_total_weight = subset.iter().map(|item| item.weight).sum::<u64>();
        if expected_total_weight == 0 {
            return Err("target masterchain validator subset is empty".into());
        }
        let signed_weight = target_signatures
            .check_signatures(&subset, &Block::build_data_for_sign(&target_block_id))
            .map_err(|e| format!("target masterchain signature verification failed: {e}"))?;
        if signed_weight.saturating_mul(3) <= expected_total_weight.saturating_mul(2) {
            return Err("target masterchain proof does not reach the finality threshold".into());
        }

        let target_state_hash = target_block
            .load_state_update()
            .map_err(|e| format!("target state update parse failed: {e}"))?
            .new_hash;
        let target_state = parse_state_proof(
            &target_state_extra_proof_boc,
            &target_state_hash,
            "target masterchain state proof",
        )?;
        if target_state.seqno != target_info.seqno
            || target_state.shard_ident != ShardIdent::MASTERCHAIN
        {
            return Err("target masterchain state does not match target block".into());
        }
        let target_extra = target_state
            .load_custom()
            .map_err(|e| format!("target custom state parse failed: {e}"))?
            .ok_or_else(|| "target masterchain state lacks custom data".to_string())?;

        let shard_block_cell = decode_boc(&shard_section.shard_block_boc, "target shard block")?;
        let shard_block = shard_block_cell
            .as_ref()
            .parse::<Block>()
            .map_err(|e| format!("target shard block parse failed: {e}"))?;
        let shard_info = shard_block
            .load_info()
            .map_err(|e| format!("target shard block info parse failed: {e}"))?;
        if shard_info.shard == ShardIdent::MASTERCHAIN {
            return Err("shard proof unexpectedly targets the masterchain".into());
        }
        if !shard_info
            .shard
            .contains_account(&hash_bytes(request.expected_master_account_id))
        {
            return Err(
                "target shard does not contain the configured jetton master account".into(),
            );
        }
        let shard_file_hash = Boc::file_hash(&shard_section.shard_block_boc);
        let workchain_shards = target_extra
            .shards
            .get_workchain_shards(shard_info.shard.workchain())
            .map_err(|e| format!("target shard tree load failed: {e}"))?
            .ok_or_else(|| {
                "target masterchain state does not contain the shard workchain".to_string()
            })?;
        let shard_description = workchain_shards
            .iter()
            .find_map(|entry| match entry {
                Ok((ident, descr)) if ident == shard_info.shard => Some(Ok(descr)),
                Ok(_) => None,
                Err(err) => Some(Err(err)),
            })
            .transpose()
            .map_err(|e| format!("target shard description parse failed: {e}"))?
            .ok_or_else(|| {
                "target masterchain state does not contain the shard block".to_string()
            })?;
        if shard_description.seqno != shard_info.seqno
            || shard_description.root_hash != *shard_block_cell.as_ref().repr_hash()
            || shard_description.file_hash != shard_file_hash
        {
            return Err(
                "target shard block does not match the masterchain shard description".into(),
            );
        }
        if shard_description.reg_mc_seqno > decoded.target_mc_seqno {
            return Err(
                "target shard block references a newer masterchain block than the proof target"
                    .into(),
            );
        }

        let shard_state_hash = shard_block
            .load_state_update()
            .map_err(|e| format!("target shard state update parse failed: {e}"))?
            .new_hash;
        let shard_state = parse_state_proof(
            &shard_section.shard_state_accounts_proof_boc,
            &shard_state_hash,
            "target shard state proof",
        )?;
        if shard_state.shard_ident != shard_info.shard || shard_state.seqno != shard_info.seqno {
            return Err("target shard state does not match the shard block".into());
        }
        let accounts_root_hash = *shard_state.accounts.inner().as_ref().repr_hash();
        let account_proof_cell = decode_boc(&decoded.account_proof, "account proof")?;
        let account_proof = account_proof_cell
            .as_ref()
            .parse::<MerkleProof>()
            .map_err(|e| format!("account proof parse failed: {e}"))?;
        if account_proof.hash != accounts_root_hash {
            return Err("account proof root hash does not match the shard accounts root".into());
        }
        let proven_accounts = account_proof
            .cell
            .as_ref()
            .virtualize()
            .parse::<ShardAccounts>()
            .map_err(|e| format!("proven shard accounts parse failed: {e}"))?;
        let (_, shard_account) = proven_accounts
            .get(hash_bytes(request.expected_master_account_id))
            .map_err(|e| format!("jetton master shard account lookup failed: {e}"))?
            .ok_or_else(|| "jetton master shard account is absent".to_string())?;
        let account = shard_account
            .load_account()
            .map_err(|e| format!("jetton master account parse failed: {e}"))?
            .ok_or_else(|| "jetton master account does not exist".to_string())?;
        let state_init = match &account.state {
            AccountState::Active(state_init) => state_init,
            _ => return Err("jetton master account is not active".into()),
        };
        let std_addr = account
            .address
            .as_std()
            .ok_or_else(|| "jetton master account is not a standard address".to_string())?;
        if std_addr.address != hash_bytes(request.expected_master_account_id) {
            return Err("jetton master account hash does not match the configured master".into());
        }
        let code = state_init
            .code
            .as_ref()
            .ok_or_else(|| "jetton master account is missing code".to_string())?;
        if code.as_ref().repr_hash() != &hash_bytes(request.expected_code_hash) {
            return Err("jetton master account code hash mismatch".into());
        }
        let data = state_init
            .data
            .as_ref()
            .ok_or_else(|| "jetton master account is missing data".to_string())?;

        let burns_proof_cell = decode_boc(&decoded.burns_dict_proof, "burns dictionary proof")?;
        let burns_proof = burns_proof_cell
            .as_ref()
            .parse::<MerkleProof>()
            .map_err(|e| format!("burns dictionary proof parse failed: {e}"))?;
        if burns_proof.hash != *data.as_ref().repr_hash() {
            return Err(
                "burns dictionary proof root hash does not match the account data root".into(),
            );
        }
        let proven_storage = burns_proof
            .cell
            .as_ref()
            .virtualize()
            .parse::<ContractMinterStorage>()
            .map_err(|e| format!("proven minter storage parse failed: {e}"))?;
        let proven_sccp = proven_storage
            .sccp
            .as_ref()
            .parse::<ContractSccpStorageExtra>()
            .map_err(|e| format!("proven SCCP storage parse failed: {e}"))?;
        let burn_cell = proven_sccp
            .burns
            .get(hash_bytes(request.expected_message_id))
            .map_err(|e| format!("burn record lookup failed: {e}"))?
            .ok_or_else(|| "burn record is absent".to_string())?;
        let burn_record = burn_cell
            .as_ref()
            .parse::<ContractBurnRecord>()
            .map_err(|e| format!("burn record parse failed: {e}"))?;
        if burn_record.dest_domain != request.expected_dest_domain
            || burn_record.dest_domain != decoded.burn_record.dest_domain
            || burn_record.recipient32 != hash_bytes(request.expected_recipient32)
            || burn_record.recipient32 != hash_bytes(decoded.burn_record.recipient32)
            || burn_record.jetton_amount.into_inner() != request.expected_amount
            || burn_record.jetton_amount.into_inner() != decoded.burn_record.jetton_amount
            || burn_record.nonce != request.expected_nonce
            || burn_record.nonce != decoded.burn_record.nonce
        {
            return Err("proved burn record does not match the submitted payload".into());
        }
        if decoded.target_mc_seqno == request.trusted_checkpoint_seqno
            && decoded.target_mc_block_hash != request.trusted_checkpoint_hash
        {
            return Err("target masterchain block hash does not match the trusted checkpoint at equal seqno".into());
        }

        Ok(())
    }

    fn decode_exact<T: Decode>(bytes: &[u8], label: &str) -> VerifyResult<T> {
        let mut input = bytes;
        let decoded = T::decode(&mut input).map_err(|_| format!("failed to decode {label}"))?;
        if !input.is_empty() {
            return Err(format!("{label} contains trailing bytes"));
        }
        Ok(decoded)
    }

    fn decode_boc(bytes: &[u8], label: &str) -> VerifyResult<Cell> {
        Boc::decode(bytes).map_err(|e| format!("failed to decode {label} BOC: {e}"))
    }

    fn parse_state_proof(
        bytes: &[u8],
        expected_root_hash: &HashBytes,
        label: &str,
    ) -> VerifyResult<ShardStateUnsplit> {
        let proof_cell = decode_boc(bytes, label)?;
        let proof = proof_cell
            .as_ref()
            .parse::<MerkleProof>()
            .map_err(|e| format!("{label} parse failed: {e}"))?;
        if &proof.hash != expected_root_hash {
            return Err(format!("{label} root hash mismatch"));
        }
        proof
            .cell
            .as_ref()
            .virtualize()
            .parse::<ShardStateUnsplit>()
            .map_err(|e| format!("{label} state parse failed: {e}"))
    }

    fn parse_block_from_proof(proof: &BlockProof) -> VerifyResult<(Cell, Block)> {
        let root_proof = proof
            .root
            .as_ref()
            .parse::<MerkleProof>()
            .map_err(|e| format!("block root proof parse failed: {e}"))?;
        if root_proof.hash != proof.proof_for.root_hash {
            return Err("block root proof hash mismatch".into());
        }
        let block_root = root_proof.cell;
        let block = block_root
            .as_ref()
            .virtualize()
            .parse::<Block>()
            .map_err(|e| format!("proved block parse failed: {e}"))?;
        Ok((block_root, block))
    }

    fn ensure_masterchain_block(
        block_root: &Cell,
        info: &BlockInfo,
        expected_seqno: u32,
        expected_root_hash: &[u8; 32],
        expected_file_hash: Option<&[u8; 32]>,
    ) -> VerifyResult<()> {
        if info.shard != ShardIdent::MASTERCHAIN {
            return Err("block is not a masterchain block".into());
        }
        if info.seqno != expected_seqno {
            return Err("block seqno mismatch".into());
        }
        if block_root.as_ref().repr_hash().0 != *expected_root_hash {
            return Err("block root hash mismatch".into());
        }
        if let Some(expected_file_hash) = expected_file_hash {
            let file_hash = Boc::file_hash(Boc::encode(block_root.as_ref()));
            if file_hash.0 != *expected_file_hash {
                return Err("block file hash mismatch".into());
            }
        }
        Ok(())
    }

    fn hash_bytes(bytes: [u8; 32]) -> HashBytes {
        HashBytes(bytes)
    }

    #[derive(Debug, Clone)]
    pub struct TonTestFixture {
        pub proof: Vec<u8>,
        pub trusted_checkpoint_seqno: u32,
        pub trusted_checkpoint_hash: [u8; 32],
        pub target_mc_seqno: u32,
        pub target_mc_block_hash: [u8; 32],
        pub jetton_master_account_id: [u8; 32],
        pub jetton_master_code_hash: [u8; 32],
    }

    #[derive(Debug, Clone, Copy)]
    pub struct TonTestFixtureInput {
        pub message_id: [u8; 32],
        pub recipient32: [u8; 32],
        pub jetton_amount: u128,
        pub nonce: u64,
    }

    pub fn build_test_fixture(input: TonTestFixtureInput) -> TonTestFixture {
        let fixture = build_test_fixture_impl(input).expect("TON test fixture should build");
        verify_ton_burn_proof(&TonVerifyRequest {
            trusted_checkpoint_seqno: fixture.trusted_checkpoint_seqno,
            trusted_checkpoint_hash: fixture.trusted_checkpoint_hash,
            proof: fixture.proof.clone(),
            expected_master_account_id: fixture.jetton_master_account_id,
            expected_code_hash: fixture.jetton_master_code_hash,
            expected_message_id: input.message_id,
            expected_dest_domain: SORA_DOMAIN,
            expected_recipient32: input.recipient32,
            expected_amount: input.jetton_amount,
            expected_nonce: input.nonce,
        })
        .expect("generated TON test fixture should verify");
        fixture
    }

    fn build_test_fixture_impl(input: TonTestFixtureInput) -> VerifyResult<TonTestFixture> {
        let checkpoint_seqno = 100;
        let target_mc_seqno = 101;
        let catchain_seqno = 77;

        let jetton_master_account_id = [0x44; 32];
        let jetton_master_address =
            IntAddr::Std(StdAddr::new(0, hash_bytes(jetton_master_account_id)));
        let burn_initiator = IntAddr::Std(StdAddr::new(0, hash_bytes([0x11; 32])));
        let governor_address = IntAddr::Std(StdAddr::new(0, hash_bytes([0x22; 32])));
        let verifier_address = IntAddr::Std(StdAddr::new(0, hash_bytes([0x33; 32])));

        let jetton_master_code = {
            let mut builder = CellBuilder::new();
            builder
                .store_u32(0x53434350)
                .map_err(|e| format!("jetton master code build failed: {e}"))?;
            builder
                .store_u64(0x544f4e50524f4f46)
                .map_err(|e| format!("jetton master code build failed: {e}"))?;
            builder
                .build()
                .map_err(|e| format!("jetton master code build failed: {e}"))?
        };
        let jetton_master_code_hash = jetton_master_code.as_ref().repr_hash().0;
        let empty_cell = Cell::empty_cell();

        let burn_record = ContractBurnRecord {
            burn_initiator,
            dest_domain: 0,
            recipient32: hash_bytes(input.recipient32),
            jetton_amount: Tokens::new(input.jetton_amount),
            nonce: input.nonce,
        };
        let burn_record_cell = CellBuilder::build_from(&burn_record)
            .map_err(|e| format!("burn record cell build failed: {e}"))?;

        let mut burns = Dict::<HashBytes, Cell>::new();
        burns
            .set(hash_bytes(input.message_id), burn_record_cell.clone())
            .map_err(|e| format!("burns dict build failed: {e}"))?;
        let sccp_storage = ContractSccpStorageExtra {
            sora_asset_id: hash_bytes([0x55; 32]),
            nonce: input.nonce,
            inbound_paused_mask: 0,
            outbound_paused_mask: 0,
            invalidated_inbound: Dict::new(),
            processed_inbound: Dict::new(),
            burns,
        };
        let sccp_storage_cell = CellBuilder::build_from(&sccp_storage)
            .map_err(|e| format!("SCCP storage build failed: {e}"))?;
        let minter_storage = ContractMinterStorage {
            total_supply: Tokens::new(input.jetton_amount * 10),
            governor_address,
            verifier_address: Some(verifier_address),
            jetton_wallet_code: Cell::empty_cell(),
            metadata_uri: Cell::empty_cell(),
            sccp: sccp_storage_cell.clone(),
        };
        let minter_storage_cell = CellBuilder::build_from(&minter_storage)
            .map_err(|e| format!("minter storage build failed: {e}"))?;

        let account = Account {
            address: jetton_master_address.clone(),
            storage_stat: StorageInfo::default(),
            last_trans_lt: 7,
            balance: CurrencyCollection::ZERO,
            state: AccountState::Active(StateInit {
                split_depth: None,
                special: None,
                code: Some(jetton_master_code.clone()),
                data: Some(minter_storage_cell.clone()),
                libraries: Dict::new(),
            }),
            init_code_hash: None,
        };
        let shard_account = ShardAccount {
            account: Lazy::new(&OptionalAccount(Some(account.clone())))
                .map_err(|e| format!("shard account build failed: {e}"))?,
            last_trans_hash: HashBytes::ZERO,
            last_trans_lt: 7,
        };
        let mut shard_accounts = AugDict::<HashBytes, DepthBalanceInfo, ShardAccount>::new();
        shard_accounts
            .set(
                hash_bytes(jetton_master_account_id),
                DepthBalanceInfo::default(),
                shard_account,
            )
            .map_err(|e| format!("shard accounts build failed: {e}"))?;

        let shard_ident = ShardIdent::new_full(0);
        let mut shard_state = ShardStateUnsplit::default();
        shard_state.shard_ident = shard_ident;
        shard_state.seqno = 200;
        shard_state.vert_seqno = 1;
        shard_state.gen_utime = 1_710_000_010;
        shard_state.gen_lt = 1_000;
        shard_state.min_ref_mc_seqno = target_mc_seqno;
        shard_state.accounts = Lazy::new(&shard_accounts)
            .map_err(|e| format!("shard state accounts build failed: {e}"))?;
        let shard_accounts_cell = shard_state.accounts.inner().clone();
        let shard_state_cell = CellBuilder::build_from(&shard_state)
            .map_err(|e| format!("shard state build failed: {e}"))?;

        let mut shard_block_info = BlockInfo::default();
        shard_block_info.seqno = shard_state.seqno;
        shard_block_info.vert_seqno = 1;
        shard_block_info.shard = shard_ident;
        shard_block_info.gen_utime = shard_state.gen_utime;
        shard_block_info.start_lt = 1_000;
        shard_block_info.end_lt = 1_100;
        shard_block_info.gen_validator_list_hash_short = 1;
        shard_block_info.gen_catchain_seqno = catchain_seqno;
        shard_block_info.min_ref_mc_seqno = target_mc_seqno;
        shard_block_info.prev_key_block_seqno = checkpoint_seqno;
        shard_block_info.set_prev_ref(&PrevBlockRef::Single(BlockRef {
            end_lt: 999,
            seqno: shard_state.seqno.saturating_sub(1),
            root_hash: HashBytes::ZERO,
            file_hash: HashBytes::ZERO,
        }));

        let shard_block = Block {
            global_id: 0,
            info: Lazy::new(&shard_block_info)
                .map_err(|e| format!("shard block info build failed: {e}"))?,
            value_flow: Lazy::new(&ValueFlow::default())
                .map_err(|e| format!("shard block value flow build failed: {e}"))?,
            state_update: Lazy::new(&everscale_types::merkle::MerkleUpdate {
                old_hash: *empty_cell.as_ref().repr_hash(),
                new_hash: *shard_state_cell.as_ref().repr_hash(),
                old_depth: empty_cell.as_ref().repr_depth(),
                new_depth: shard_state_cell.as_ref().repr_depth(),
                old: empty_cell.clone(),
                new: shard_state_cell.clone(),
            })
            .map_err(|e| format!("shard block state update build failed: {e}"))?,
            out_msg_queue_updates: Some(Dict::new()),
            extra: Lazy::new(&BlockExtra::default())
                .map_err(|e| format!("shard block extra build failed: {e}"))?,
        };
        let shard_block_cell = CellBuilder::build_from(&shard_block)
            .map_err(|e| format!("shard block build failed: {e}"))?;
        let shard_block_boc = Boc::encode(shard_block_cell.as_ref());
        let shard_description = ShardDescription {
            seqno: shard_block_info.seqno,
            reg_mc_seqno: target_mc_seqno,
            start_lt: shard_block_info.start_lt,
            end_lt: shard_block_info.end_lt,
            root_hash: *shard_block_cell.as_ref().repr_hash(),
            file_hash: Boc::file_hash(&shard_block_boc),
            before_split: false,
            before_merge: false,
            want_split: false,
            want_merge: false,
            nx_cc_updated: false,
            next_catchain_seqno: catchain_seqno,
            next_validator_shard: shard_ident.prefix(),
            min_ref_mc_seqno: target_mc_seqno,
            gen_utime: shard_block_info.gen_utime,
            split_merge_at: None,
            fees_collected: CurrencyCollection::ZERO,
            funds_created: CurrencyCollection::ZERO,
            copyleft_rewards: Dict::new(),
            proof_chain: None,
        };
        let shard_hashes = everscale_types::models::block::ShardHashes::from_shards([(
            &shard_ident,
            &shard_description,
        )])
        .map_err(|e| format!("shard hashes build failed: {e}"))?;

        let signing_keys = [
            SigningKey::from_bytes(&[1u8; 32]),
            SigningKey::from_bytes(&[2u8; 32]),
            SigningKey::from_bytes(&[3u8; 32]),
        ];
        let validator_list = signing_keys
            .iter()
            .enumerate()
            .map(|(index, key)| ValidatorDescription {
                public_key: hash_bytes(key.verifying_key().to_bytes()),
                weight: 1,
                adnl_addr: None,
                mc_seqno_since: 0,
                prev_total_weight: index as u64,
            })
            .collect::<Vec<_>>();
        let validator_set = ValidatorSet {
            utime_since: 1_710_000_000,
            utime_until: 1_710_000_999,
            main: NonZeroU16::new(validator_list.len() as u16).expect("non-zero"),
            total_weight: validator_list.len() as u64,
            list: validator_list.clone(),
        };
        let catchain_config = CatchainConfig {
            isolate_mc_validators: false,
            shuffle_mc_validators: false,
            mc_catchain_lifetime: 60,
            shard_catchain_lifetime: 60,
            shard_validators_lifetime: 60,
            shard_validators_num: validator_list.len() as u32,
        };
        let subset_hash_short = validator_set
            .compute_mc_subset(catchain_seqno, false)
            .expect("mc subset")
            .1;
        let mut blockchain_config =
            everscale_types::models::config::BlockchainConfig::new_empty(HashBytes::ZERO);
        blockchain_config
            .set_catchain_config(&catchain_config)
            .map_err(|e| format!("blockchain catchain config set failed: {e}"))?;
        blockchain_config
            .set::<ConfigParam34>(&validator_set)
            .map_err(|e| format!("blockchain validator set set failed: {e}"))?;

        let checkpoint_extra = McStateExtra {
            shards: everscale_types::models::block::ShardHashes::default(),
            config: blockchain_config.clone(),
            validator_info: ValidatorInfo {
                validator_list_hash_short: subset_hash_short,
                catchain_seqno,
                nx_cc_updated: false,
            },
            prev_blocks: AugDict::new(),
            after_key_block: false,
            last_key_block: None,
            block_create_stats: None,
            global_balance: CurrencyCollection::ZERO,
            copyleft_rewards: Dict::new(),
        };
        let target_extra = McStateExtra {
            shards: shard_hashes,
            config: blockchain_config,
            validator_info: ValidatorInfo {
                validator_list_hash_short: subset_hash_short,
                catchain_seqno,
                nx_cc_updated: false,
            },
            prev_blocks: AugDict::new(),
            after_key_block: false,
            last_key_block: None,
            block_create_stats: None,
            global_balance: CurrencyCollection::ZERO,
            copyleft_rewards: Dict::new(),
        };

        let checkpoint_state = {
            let mut state = ShardStateUnsplit::default();
            state.shard_ident = ShardIdent::MASTERCHAIN;
            state.seqno = checkpoint_seqno;
            state.vert_seqno = 1;
            state.gen_utime = 1_710_000_000;
            state.gen_lt = 10_000;
            state.min_ref_mc_seqno = checkpoint_seqno;
            state
                .set_custom(Some(&checkpoint_extra))
                .map_err(|e| format!("checkpoint custom state set failed: {e}"))?;
            state
        };
        let checkpoint_extra_cell = checkpoint_state
            .custom
            .as_ref()
            .ok_or_else(|| "checkpoint masterchain state lacks custom data".to_string())?
            .inner()
            .clone();
        let checkpoint_state_cell = CellBuilder::build_from(&checkpoint_state)
            .map_err(|e| format!("checkpoint state build failed: {e}"))?;
        let mut checkpoint_block_info = BlockInfo::default();
        checkpoint_block_info.seqno = checkpoint_seqno;
        checkpoint_block_info.vert_seqno = 1;
        checkpoint_block_info.shard = ShardIdent::MASTERCHAIN;
        checkpoint_block_info.gen_utime = checkpoint_state.gen_utime;
        checkpoint_block_info.start_lt = 10_000;
        checkpoint_block_info.end_lt = 10_100;
        checkpoint_block_info.gen_validator_list_hash_short = subset_hash_short;
        checkpoint_block_info.gen_catchain_seqno = catchain_seqno;
        checkpoint_block_info.min_ref_mc_seqno = checkpoint_seqno;
        checkpoint_block_info.prev_key_block_seqno = checkpoint_seqno.saturating_sub(1);
        checkpoint_block_info.set_prev_ref(&PrevBlockRef::Single(BlockRef {
            end_lt: 9_999,
            seqno: checkpoint_seqno.saturating_sub(1),
            root_hash: HashBytes::ZERO,
            file_hash: HashBytes::ZERO,
        }));
        let checkpoint_block = Block {
            global_id: 0,
            info: Lazy::new(&checkpoint_block_info)
                .map_err(|e| format!("checkpoint block info build failed: {e}"))?,
            value_flow: Lazy::new(&ValueFlow::default())
                .map_err(|e| format!("checkpoint block value flow build failed: {e}"))?,
            state_update: Lazy::new(&everscale_types::merkle::MerkleUpdate {
                old_hash: *empty_cell.as_ref().repr_hash(),
                new_hash: *checkpoint_state_cell.as_ref().repr_hash(),
                old_depth: empty_cell.as_ref().repr_depth(),
                new_depth: checkpoint_state_cell.as_ref().repr_depth(),
                old: empty_cell.clone(),
                new: checkpoint_state_cell.clone(),
            })
            .map_err(|e| format!("checkpoint block state update build failed: {e}"))?,
            out_msg_queue_updates: Some(Dict::new()),
            extra: Lazy::new(&BlockExtra::default())
                .map_err(|e| format!("checkpoint block extra build failed: {e}"))?,
        };
        let checkpoint_block_cell = CellBuilder::build_from(&checkpoint_block)
            .map_err(|e| format!("checkpoint block build failed: {e}"))?;
        let checkpoint_hash = checkpoint_block_cell.as_ref().repr_hash().0;

        let target_state = {
            let mut state = ShardStateUnsplit::default();
            state.shard_ident = ShardIdent::MASTERCHAIN;
            state.seqno = target_mc_seqno;
            state.vert_seqno = 1;
            state.gen_utime = 1_710_000_020;
            state.gen_lt = 10_200;
            state.min_ref_mc_seqno = target_mc_seqno;
            state
                .set_custom(Some(&target_extra))
                .map_err(|e| format!("target custom state set failed: {e}"))?;
            state
        };
        let target_extra_cell = target_state
            .custom
            .as_ref()
            .ok_or_else(|| "target masterchain state lacks custom data".to_string())?
            .inner()
            .clone();
        let target_state_cell = CellBuilder::build_from(&target_state)
            .map_err(|e| format!("target state build failed: {e}"))?;
        let checkpoint_block_ref = BlockRef {
            end_lt: checkpoint_block_info.end_lt,
            seqno: checkpoint_seqno,
            root_hash: *checkpoint_block_cell.as_ref().repr_hash(),
            file_hash: Boc::file_hash(Boc::encode(checkpoint_block_cell.as_ref())),
        };
        let mut target_block_info = BlockInfo::default();
        target_block_info.seqno = target_mc_seqno;
        target_block_info.vert_seqno = 1;
        target_block_info.shard = ShardIdent::MASTERCHAIN;
        target_block_info.gen_utime = target_state.gen_utime;
        target_block_info.start_lt = 10_200;
        target_block_info.end_lt = 10_300;
        target_block_info.gen_validator_list_hash_short = subset_hash_short;
        target_block_info.gen_catchain_seqno = catchain_seqno;
        target_block_info.min_ref_mc_seqno = checkpoint_seqno;
        target_block_info.prev_key_block_seqno = checkpoint_seqno;
        target_block_info.set_prev_ref(&PrevBlockRef::Single(checkpoint_block_ref.clone()));
        let target_block = Block {
            global_id: 0,
            info: Lazy::new(&target_block_info)
                .map_err(|e| format!("target block info build failed: {e}"))?,
            value_flow: Lazy::new(&ValueFlow::default())
                .map_err(|e| format!("target block value flow build failed: {e}"))?,
            state_update: Lazy::new(&everscale_types::merkle::MerkleUpdate {
                old_hash: *checkpoint_state_cell.as_ref().repr_hash(),
                new_hash: *target_state_cell.as_ref().repr_hash(),
                old_depth: checkpoint_state_cell.as_ref().repr_depth(),
                new_depth: target_state_cell.as_ref().repr_depth(),
                old: checkpoint_state_cell.clone(),
                new: target_state_cell.clone(),
            })
            .map_err(|e| format!("target block state update build failed: {e}"))?,
            out_msg_queue_updates: Some(Dict::new()),
            extra: Lazy::new(&BlockExtra {
                custom: Some(
                    Lazy::new(&McBlockExtra {
                        shards: everscale_types::models::block::ShardHashes::default(),
                        fees: everscale_types::models::block::ShardFees::new(),
                        prev_block_signatures: Dict::new(),
                        recover_create_msg: None,
                        mint_msg: None,
                        copyleft_msgs: Dict::new(),
                        config: None,
                    })
                    .map_err(|e| format!("target masterchain extra build failed: {e}"))?,
                ),
                ..BlockExtra::default()
            })
            .map_err(|e| format!("target block extra build failed: {e}"))?,
        };
        let target_block_cell = CellBuilder::build_from(&target_block)
            .map_err(|e| format!("target block build failed: {e}"))?;
        let target_block_boc = Boc::encode(target_block_cell.as_ref());
        let target_block_id = BlockId {
            shard: ShardIdent::MASTERCHAIN,
            seqno: target_mc_seqno,
            root_hash: *target_block_cell.as_ref().repr_hash(),
            file_hash: Boc::file_hash(&target_block_boc),
        };
        let block_sign_data = Block::build_data_for_sign(&target_block_id);
        let mut lite_signatures = Vec::with_capacity(signing_keys.len());
        for signing_key in &signing_keys {
            let verifying_key = signing_key.verifying_key();
            let signature = signing_key.sign(&block_sign_data);
            let node_id_short = tl_proto::hash(tl::PublicKey::Ed25519 {
                key: &verifying_key.to_bytes(),
            });
            lite_signatures.push(TonLiteBlockSignatureV2 {
                node_id_short,
                signature: signature.to_bytes(),
            });
        }

        let checkpoint_state_extra_proof = build_state_extra_proof(
            &checkpoint_state_cell,
            checkpoint_extra_cell.as_ref().repr_hash(),
        )?;
        let target_state_extra_proof =
            build_state_extra_proof(&target_state_cell, target_extra_cell.as_ref().repr_hash())?;
        let shard_state_accounts_proof = build_accounts_state_proof(
            &shard_state_cell,
            shard_accounts_cell.as_ref().repr_hash(),
        )?;
        let account_proof =
            build_account_proof(&shard_accounts_cell, hash_bytes(jetton_master_account_id))?;
        let burns_dict_proof =
            build_burns_proof(&minter_storage_cell, hash_bytes(input.message_id))?;

        let masterchain_section = TonMasterchainProofSectionV2 {
            version: MASTERCHAIN_SECTION_VERSION_V2,
            checkpoint_block_boc: Boc::encode(checkpoint_block_cell.as_ref()),
            checkpoint_state_extra_proof_boc: checkpoint_state_extra_proof,
            target_block_boc,
            target_signatures: TonLiteSignatureSetV2 {
                validator_list_hash_short: subset_hash_short,
                catchain_seqno,
                signatures: lite_signatures,
            },
            target_state_extra_proof_boc: target_state_extra_proof,
        };
        let shard_section = TonShardProofSectionV1 {
            version: SHARD_SECTION_VERSION,
            shard_block_boc,
            shard_state_accounts_proof_boc: shard_state_accounts_proof,
        };
        let proof = TonBurnProofV1 {
            version: PROOF_VERSION,
            trusted_checkpoint_seqno: checkpoint_seqno,
            trusted_checkpoint_hash: checkpoint_hash,
            target_mc_seqno,
            target_mc_block_hash: target_block_cell.as_ref().repr_hash().0,
            jetton_master_account_id,
            jetton_master_code_hash,
            burn_message_id: input.message_id,
            burn_record: TonBurnRecordV1 {
                dest_domain: 0,
                recipient32: input.recipient32,
                jetton_amount: input.jetton_amount,
                nonce: input.nonce,
            },
            masterchain_proof: masterchain_section.encode(),
            shard_proof: shard_section.encode(),
            account_proof,
            burns_dict_proof,
        }
        .encode();

        Ok(TonTestFixture {
            proof,
            trusted_checkpoint_seqno: checkpoint_seqno,
            trusted_checkpoint_hash: checkpoint_hash,
            target_mc_seqno,
            target_mc_block_hash: target_block_cell.as_ref().repr_hash().0,
            jetton_master_account_id,
            jetton_master_code_hash,
        })
    }

    fn build_state_extra_proof(
        state_root: &Cell,
        target_hash: &HashBytes,
    ) -> VerifyResult<Vec<u8>> {
        let mut usage_tree = UsageTree::new(UsageTreeMode::OnDataAccess).with_subtrees();
        let tracked_root = usage_tree.track(state_root);
        let tracked_state = tracked_root
            .as_ref()
            .parse::<ShardStateUnsplit>()
            .map_err(|e| format!("tracked state parse failed: {e}"))?;
        let tracked_custom = tracked_state
            .custom
            .as_ref()
            .ok_or_else(|| "tracked state lacks custom data".to_string())?;
        if tracked_custom.inner().as_ref().repr_hash() != target_hash {
            return Err("tracked masterchain custom state hash mismatch".into());
        }
        usage_tree.add_subtree(tracked_custom.inner().as_ref());

        let proof = MerkleProof::create(tracked_root.as_ref(), usage_tree)
            .build()
            .map_err(|e| format!("state extra proof build failed: {e}"))?;
        let proof_cell = CellBuilder::build_from(&proof)
            .map_err(|e| format!("state extra proof cell build failed: {e}"))?;
        Ok(Boc::encode(proof_cell.as_ref()))
    }

    fn build_accounts_state_proof(
        shard_state_root: &Cell,
        target_hash: &HashBytes,
    ) -> VerifyResult<Vec<u8>> {
        let mut usage_tree = UsageTree::new(UsageTreeMode::OnDataAccess).with_subtrees();
        let tracked_root = usage_tree.track(shard_state_root);
        let tracked_state = tracked_root
            .as_ref()
            .parse::<ShardStateUnsplit>()
            .map_err(|e| format!("tracked shard state parse failed: {e}"))?;
        if tracked_state.accounts.inner().as_ref().repr_hash() != target_hash {
            return Err("tracked shard accounts hash mismatch".into());
        }
        usage_tree.add_subtree(tracked_state.accounts.inner().as_ref());

        let proof = MerkleProof::create(tracked_root.as_ref(), usage_tree)
            .build()
            .map_err(|e| format!("shard accounts proof build failed: {e}"))?;
        let proof_cell = CellBuilder::build_from(&proof)
            .map_err(|e| format!("shard accounts proof cell build failed: {e}"))?;
        Ok(Boc::encode(proof_cell.as_ref()))
    }

    fn build_account_proof(accounts_root: &Cell, key: HashBytes) -> VerifyResult<Vec<u8>> {
        let mut usage_tree = UsageTree::new(UsageTreeMode::OnDataAccess).with_subtrees();
        let tracked_root = usage_tree.track(accounts_root);
        let tracked_accounts = tracked_root
            .as_ref()
            .parse::<ShardAccounts>()
            .map_err(|e| format!("tracked accounts parse failed: {e}"))?;
        let (_, tracked_shard_account) = tracked_accounts
            .get(key)
            .map_err(|e| format!("tracked account lookup failed: {e}"))?
            .ok_or_else(|| "tracked account missing".to_string())?;
        usage_tree.add_subtree(tracked_shard_account.account.inner().as_ref());
        let proof = MerkleProof::create(tracked_root.as_ref(), usage_tree)
            .build()
            .map_err(|e| format!("account proof build failed: {e}"))?;
        let proof_cell = CellBuilder::build_from(&proof)
            .map_err(|e| format!("account proof cell build failed: {e}"))?;
        Ok(Boc::encode(proof_cell.as_ref()))
    }

    fn build_burns_proof(data_root: &Cell, message_id: HashBytes) -> VerifyResult<Vec<u8>> {
        let usage_tree = UsageTree::new(UsageTreeMode::OnDataAccess);
        let tracked_root = usage_tree.track(data_root);
        let tracked_storage = tracked_root
            .as_ref()
            .parse::<ContractMinterStorage>()
            .map_err(|e| format!("tracked minter storage parse failed: {e}"))?;
        let tracked_sccp = tracked_storage
            .sccp
            .as_ref()
            .parse::<ContractSccpStorageExtra>()
            .map_err(|e| format!("tracked SCCP storage parse failed: {e}"))?;
        tracked_sccp
            .burns
            .get(message_id)
            .map_err(|e| format!("tracked burn lookup failed: {e}"))?
            .ok_or_else(|| "tracked burn record missing".to_string())?;
        let proof = MerkleProof::create(tracked_root.as_ref(), usage_tree)
            .build()
            .map_err(|e| format!("burns proof build failed: {e}"))?;
        let proof_cell = CellBuilder::build_from(&proof)
            .map_err(|e| format!("burns proof cell build failed: {e}"))?;
        Ok(Boc::encode(proof_cell.as_ref()))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn generated_fixture_verifies() {
            let input = TonTestFixtureInput {
                message_id: [0x99; 32],
                recipient32: [0x77; 32],
                jetton_amount: 42,
                nonce: 7,
            };
            let fixture = build_test_fixture(input);
            assert!(verify_ton_burn_proof(&TonVerifyRequest {
                trusted_checkpoint_seqno: fixture.trusted_checkpoint_seqno,
                trusted_checkpoint_hash: fixture.trusted_checkpoint_hash,
                proof: fixture.proof.clone(),
                expected_master_account_id: fixture.jetton_master_account_id,
                expected_code_hash: fixture.jetton_master_code_hash,
                expected_message_id: input.message_id,
                expected_dest_domain: SORA_DOMAIN,
                expected_recipient32: input.recipient32,
                expected_amount: input.jetton_amount,
                expected_nonce: input.nonce,
            })
            .is_ok());
        }

        #[test]
        fn tampered_burn_record_fails() {
            let input = TonTestFixtureInput {
                message_id: [0x98; 32],
                recipient32: [0x55; 32],
                jetton_amount: 77,
                nonce: 11,
            };
            let fixture = build_test_fixture(input);
            let mut decoded =
                decode_exact::<TonBurnProofV1>(&fixture.proof, "fixture outer proof").unwrap();
            decoded.burn_record.jetton_amount += 1;
            let tampered = decoded.encode();
            assert!(verify_ton_burn_proof(&TonVerifyRequest {
                trusted_checkpoint_seqno: fixture.trusted_checkpoint_seqno,
                trusted_checkpoint_hash: fixture.trusted_checkpoint_hash,
                proof: tampered,
                expected_master_account_id: fixture.jetton_master_account_id,
                expected_code_hash: fixture.jetton_master_code_hash,
                expected_message_id: input.message_id,
                expected_dest_domain: SORA_DOMAIN,
                expected_recipient32: input.recipient32,
                expected_amount: input.jetton_amount,
                expected_nonce: input.nonce,
            })
            .is_err());
        }
    }
}
