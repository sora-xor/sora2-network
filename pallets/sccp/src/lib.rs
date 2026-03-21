// This file is part of the SORA network and Polkaswap app.
//
// Copyright (c) 2026, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

#![cfg_attr(not(feature = "std"), no_std)]
#![recursion_limit = "512"]
// This pallet is security-critical; keep logic explicit and avoid "clever" abstractions.

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod evm_proof;

#[cfg(any(test, feature = "fuzzing"))]
pub mod tron_proof;
#[cfg(not(any(test, feature = "fuzzing")))]
mod tron_proof;

use bridge_types::{
    traits::AuxiliaryDigestHandler, types::AuxiliaryDigestItem, GenericNetworkId, SubNetworkId,
};
use codec::{Decode, DecodeWithMemTracking, Encode};
use common::{hash, prelude::Balance, AssetInfoProvider, AssetName, AssetSymbol};
use frame_support::dispatch::DispatchResult;
use frame_support::pallet_prelude::*;
use frame_support::traits::EnsureOrigin;
use frame_support::{ensure, transactional};
use frame_system::pallet_prelude::*;
use permissions::{Scope, BURN, MINT};
use sp_core::{H160, H256};
use sp_io::hashing::keccak_256;
use sp_runtime::traits::Zero;
use sp_runtime::DispatchError;
use sp_std::prelude::*;

pub mod weights;
pub use pallet::*;
pub use weights::WeightInfo;

/// A lightweight interface for other pallets to check whether an asset is SCCP-managed.
///
/// Implementations should return `true` for any SCCP token state (Pending/Active/Removing).
pub trait SccpAssetChecker<AssetId> {
    fn is_sccp_asset(asset_id: &AssetId) -> bool;
}

impl<AssetId> SccpAssetChecker<AssetId> for () {
    fn is_sccp_asset(_asset_id: &AssetId) -> bool {
        false
    }
}

/// A lightweight interface for SCCP to check whether an asset is already managed by the legacy SORA bridge.
///
/// SCCP assets must be exclusive: an asset should not be supported by SCCP and the legacy bridge at the same time.
pub trait LegacyBridgeAssetChecker<AssetId> {
    fn is_legacy_bridge_asset(asset_id: &AssetId) -> bool;
}

impl<AssetId> LegacyBridgeAssetChecker<AssetId> for () {
    fn is_legacy_bridge_asset(_asset_id: &AssetId) -> bool {
        false
    }
}

/// A lightweight interface for sourcing finalized Ethereum execution state for SCCP inbound verification.
///
/// Implementations should return `(finalized_block_hash, finalized_state_root)` from an on-chain ETH
/// light-client source. Returning `None` keeps SCCP fail-closed for ETH beacon mode.
pub trait EthFinalizedStateProvider {
    fn latest_finalized_state() -> Option<(H256, H256)>;
}

impl EthFinalizedStateProvider for () {
    fn latest_finalized_state() -> Option<(H256, H256)> {
        None
    }
}

/// Pluggable on-chain verifier hook for finalized Ethereum burn proofs.
///
/// Implementations should return:
/// - `Some(true)` when the proof is valid and finalized under Ethereum consensus,
/// - `Some(false)` when the proof is invalid,
/// - `None` when finalized ETH verification is currently unavailable (fail-closed).
pub trait EthFinalizedBurnProofVerifier {
    fn is_available() -> bool;
    fn verify_finalized_burn(
        message_id: H256,
        payload: &BurnPayloadV1,
        proof: &[u8],
    ) -> Option<bool>;
}

impl EthFinalizedBurnProofVerifier for () {
    fn is_available() -> bool {
        false
    }

    fn verify_finalized_burn(
        _message_id: H256,
        _payload: &BurnPayloadV1,
        _proof: &[u8],
    ) -> Option<bool> {
        None
    }
}

/// Pluggable on-chain verifier hook for zk-proven Ethereum finalized burns.
///
/// Implementations should return:
/// - `Some(true)` when the proof is valid and finalized under Ethereum consensus,
/// - `Some(false)` when the proof is invalid,
/// - `None` when finalized ETH zk-proof verification is currently unavailable (fail-closed).
pub trait EthZkFinalizedBurnProofVerifier {
    fn is_available() -> bool;
    fn verify_finalized_burn(message_id: H256, proof: &[u8]) -> Option<bool>;
}

impl EthZkFinalizedBurnProofVerifier for () {
    fn is_available() -> bool {
        false
    }

    fn verify_finalized_burn(_message_id: H256, _proof: &[u8]) -> Option<bool> {
        None
    }
}

/// Pluggable on-chain verifier hook for Solana -> SORA burn proofs.
///
/// Implementations should return:
/// - `Some(true)` when the proof is valid and finalized under Solana consensus,
/// - `Some(false)` when the proof is invalid,
/// - `None` when finalized Solana verification is currently unavailable (fail-closed).
pub trait SolanaFinalizedBurnProofVerifier {
    fn is_available() -> bool;
    fn verify_finalized_burn(message_id: H256, proof: &[u8]) -> Option<bool>;
}

impl SolanaFinalizedBurnProofVerifier for () {
    fn is_available() -> bool {
        false
    }

    fn verify_finalized_burn(_message_id: H256, _proof: &[u8]) -> Option<bool> {
        None
    }
}

/// Pluggable on-chain verifier hook for trustless Substrate burn proofs into SORA.
///
/// This is used for SCCP domains backed by SORA parachains (Kusama/Polkadot).
///
/// Implementations should return:
/// - `Some(true)` when the proof is valid and finalized under Substrate consensus,
/// - `Some(false)` when the proof is invalid,
/// - `None` when finalized verification is currently unavailable (fail-closed).
pub trait SubstrateFinalizedBurnProofVerifier {
    fn is_available(source_domain: u32) -> bool;
    fn verify_finalized_burn(source_domain: u32, message_id: H256, proof: &[u8]) -> Option<bool>;
}

impl SubstrateFinalizedBurnProofVerifier for () {
    fn is_available(_source_domain: u32) -> bool {
        false
    }

    fn verify_finalized_burn(
        _source_domain: u32,
        _message_id: H256,
        _proof: &[u8],
    ) -> Option<bool> {
        None
    }
}

pub const SCCP_DOMAIN_SORA: u32 = 0;
pub const SCCP_DOMAIN_ETH: u32 = 1;
pub const SCCP_DOMAIN_BSC: u32 = 2;
pub const SCCP_DOMAIN_SOL: u32 = 3;
pub const SCCP_DOMAIN_TON: u32 = 4;
pub const SCCP_DOMAIN_TRON: u32 = 5;
pub const SCCP_DOMAIN_SORA_KUSAMA: u32 = 6;
pub const SCCP_DOMAIN_SORA_POLKADOT: u32 = 7;
/// Core SCCP remote domains that must always be configured per token before activation.
pub const SCCP_CORE_REMOTE_DOMAINS: [u32; 7] = [
    SCCP_DOMAIN_ETH,
    SCCP_DOMAIN_BSC,
    SCCP_DOMAIN_SOL,
    SCCP_DOMAIN_TON,
    SCCP_DOMAIN_TRON,
    SCCP_DOMAIN_SORA_KUSAMA,
    SCCP_DOMAIN_SORA_POLKADOT,
];

pub const SCCP_MSG_PREFIX_BURN_V1: &[u8] = b"sccp:burn:v1";
/// Domain-separated prefix used by legacy SCCP attestation hashes retained for fixture stability.
pub const SCCP_MSG_PREFIX_ATTEST_V1: &[u8] = b"sccp:attest:v1";
pub const ETH_FINALIZED_RECEIPT_BURN_PROOF_VERSION_V1: u8 = 1;
pub const ETH_ZK_FINALIZED_BURN_PROOF_VERSION_V1: u8 = 1;
pub const SOLANA_FINALIZED_BURN_PROOF_VERSION_V1: u8 = 1;
pub const SCCP_MAX_SOLANA_MERKLE_DEPTH: usize = 32;
pub const SCCP_MAX_SOLANA_ACCOUNT_DATA_BYTES: usize = 64 * 1024;
pub const SCCP_MAX_SOLANA_MESSAGE_BYTES: usize = 4 * 1024;

#[cfg(any(test, feature = "fuzzing"))]
pub fn decode_attester_quorum_proof_for_fuzz(
    proof: &[u8],
    max_attesters: usize,
) -> Option<Vec<[u8; 65]>> {
    let mut input = proof;
    let version = u8::decode(&mut input).ok()?;
    if version != 1 {
        return None;
    }
    let signatures = Vec::<[u8; 65]>::decode(&mut input).ok()?;
    if !input.is_empty() {
        return None;
    }
    if signatures.len() > max_attesters {
        return None;
    }
    Some(signatures)
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    fn any_payload() -> BurnPayloadV1 {
        BurnPayloadV1 {
            version: kani::any(),
            source_domain: kani::any(),
            dest_domain: kani::any(),
            nonce: kani::any(),
            sora_asset_id: kani::any(),
            amount: kani::any(),
            recipient: kani::any(),
        }
    }

    #[kani::proof]
    pub fn kani_burn_payload_roundtrip_bounded() {
        let payload = any_payload();
        let encoded = payload.encode();
        let decoded = BurnPayloadV1::decode(&mut encoded.as_slice())
            .expect("burn payload should roundtrip through SCALE encoding");
        assert_eq!(decoded, payload);
    }

    #[kani::proof]
    pub fn kani_burn_message_id_nonce_sensitivity_bounded() {
        let payload = any_payload();
        let mut nonce_changed = payload.clone();
        nonce_changed.nonce = nonce_changed.nonce.wrapping_add(1);
        kani::assume(payload.nonce != nonce_changed.nonce);

        let mut preimage_a = SCCP_MSG_PREFIX_BURN_V1.to_vec();
        preimage_a.extend(payload.encode());

        let mut preimage_b = SCCP_MSG_PREFIX_BURN_V1.to_vec();
        preimage_b.extend(nonce_changed.encode());

        assert_ne!(preimage_a, preimage_b);
    }

    #[kani::proof]
    pub fn kani_domain_separator_prefixes_bounded() {
        assert_ne!(SCCP_MSG_PREFIX_BURN_V1, SCCP_MSG_PREFIX_ATTEST_V1);
        assert!(SCCP_MSG_PREFIX_BURN_V1.starts_with(b"sccp:"));
        assert!(SCCP_MSG_PREFIX_ATTEST_V1.starts_with(b"sccp:"));
    }
}

pub const SCCP_TECH_ACC_PREFIX: &[u8] = b"sccp";
pub const SCCP_TECH_ACC_MAIN: &[u8] = b"main";

/// Generic network id used inside `AuxiliaryDigestItem::Commitment` for SCCP burn commitments.
///
/// We reuse `GenericNetworkId::EVMLegacy(u32)` to avoid changing the shared `bridge-types` enum,
/// reserving `0x5343_4350` ('SCCP') as sentinel.
pub const SCCP_DIGEST_NETWORK_ID: GenericNetworkId = GenericNetworkId::EVMLegacy(0x5343_4350);

fn digest_network_id_for_domain(domain_id: u32) -> GenericNetworkId {
    match domain_id {
        SCCP_DOMAIN_SORA_KUSAMA => GenericNetworkId::Sub(SubNetworkId::Kusama),
        SCCP_DOMAIN_SORA_POLKADOT => GenericNetworkId::Sub(SubNetworkId::Polkadot),
        _ => SCCP_DIGEST_NETWORK_ID,
    }
}

/// Solidity storage slot index of `mapping(bytes32 => BurnRecord) public burns;` in `SccpRouter`.
///
/// This is part of the SCCP protocol and must stay in sync with the router contract layout.
pub const SCCP_EVM_BURNS_MAPPING_SLOT: u64 = 4;

/// Hard bounds for inbound EVM proofs to avoid DoS via oversized MPT proofs.
pub const SCCP_MAX_EVM_PROOF_NODES: usize = 64;
pub const SCCP_MAX_EVM_PROOF_NODE_BYTES: usize = 2048;
pub const SCCP_MAX_EVM_PROOF_TOTAL_BYTES: usize = 64 * 1024;

/// Max size of a submitted BSC header RLP (DoS bound).
pub const SCCP_MAX_BSC_HEADER_RLP_BYTES: usize = 8 * 1024;

/// Number of BSC headers retained by the on-chain light client state.
///
/// Retention must be greater than `confirmation_depth` and large enough to cover the clique/parlia
/// "recent signers" window.
pub const SCCP_BSC_HEADER_RETENTION: u64 = 4096;

/// Max size of a submitted TRON header raw_data (protobuf bytes) (DoS bound).
pub const SCCP_MAX_TRON_RAW_DATA_BYTES: usize = 1024;

/// Number of TRON headers retained by the on-chain light client state.
pub const SCCP_TRON_HEADER_RETENTION: u64 = 4096;

/// Hard bounds for the repo-defined TON -> SORA proof bundle.
pub const SCCP_MAX_TON_PROOF_SECTION_BYTES: usize = 64 * 1024;
pub const SCCP_MAX_TON_PROOF_TOTAL_BYTES: usize = 256 * 1024;
/// Hard bound for the repo-defined finalized ETH burn proof payload submitted to SCCP.
pub const SCCP_MAX_ETH_FINALIZED_BURN_PROOF_BYTES: usize = 256 * 1024;
/// Hard bound for the repo-defined ETH zk proof payload submitted to SCCP.
pub const SCCP_MAX_ETH_ZK_PROOF_BYTES: usize = 256 * 1024;
/// Canonical ETH zk public input count for `EthZkFinalizedBurnProofV1`.
pub const ETH_ZK_PUBLIC_INPUT_COUNT_V1: usize = 10;
/// keccak256("SccpBurned(bytes32,bytes32,address,uint128,uint32,bytes32,uint64,bytes)")
pub const SCCP_ETH_BURN_EVENT_TOPIC0: H256 = H256([
    0xd8, 0x50, 0xac, 0x8d, 0x39, 0xa7, 0x95, 0x16, 0x0f, 0x3b, 0xaa, 0x24, 0xb8, 0x25, 0xb4, 0xb7,
    0x7b, 0xb2, 0x5f, 0x5d, 0x05, 0x77, 0x6d, 0x29, 0x35, 0x01, 0x63, 0x03, 0x55, 0x2b, 0x4d, 0x41,
]);

/// secp256k1 curve order / 2 (EIP-2), for rejecting malleable ECDSA signatures (high-`s`).
pub const SECP256K1N_HALF_ORDER: [u8; 32] = [
    0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0x5d, 0x57, 0x6e, 0x73, 0x57, 0xa4, 0x50, 0x1d, 0xdf, 0xe9, 0x2f, 0x46, 0x68, 0x1b, 0x20, 0xa0,
];

fn default_required_domains_for_bound<S: Get<u32>>() -> BoundedVec<u32, S> {
    // ETH, BSC, Solana, TON, TRON, SORA Kusama parachain, SORA Polkadot parachain
    let mut domains = BoundedVec::<u32, S>::default();
    for domain in SCCP_CORE_REMOTE_DOMAINS {
        if domains.try_push(domain).is_err() {
            break;
        }
    }
    domains
}

#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    Copy,
    Clone,
    PartialEq,
    Eq,
    RuntimeDebug,
    scale_info::TypeInfo,
    MaxEncodedLen,
)]
pub enum TokenStatus {
    Pending,
    Active,
    Removing,
}

#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    Clone,
    PartialEq,
    Eq,
    RuntimeDebug,
    scale_info::TypeInfo,
    MaxEncodedLen,
)]
pub struct TokenState<BlockNumber> {
    pub status: TokenStatus,
    pub outbound_enabled: bool,
    pub inbound_enabled: bool,
    pub inbound_enabled_until: Option<BlockNumber>,
}

impl<BlockNumber> TokenState<BlockNumber> {
    pub fn pending() -> Self {
        Self {
            status: TokenStatus::Pending,
            outbound_enabled: false,
            inbound_enabled: false,
            inbound_enabled_until: None,
        }
    }
}

#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    Clone,
    PartialEq,
    Eq,
    RuntimeDebug,
    scale_info::TypeInfo,
    MaxEncodedLen,
)]
pub struct BurnPayloadV1 {
    pub version: u8,
    pub source_domain: u32,
    pub dest_domain: u32,
    pub nonce: u64,
    pub sora_asset_id: [u8; 32],
    pub amount: Balance,
    pub recipient: [u8; 32],
}

#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    Clone,
    PartialEq,
    Eq,
    RuntimeDebug,
    scale_info::TypeInfo,
    MaxEncodedLen,
)]
pub struct BurnRecord<AccountId, AssetId, BlockNumber> {
    pub sender: AccountId,
    pub asset_id: AssetId,
    pub amount: Balance,
    pub dest_domain: u32,
    pub recipient: [u8; 32],
    pub nonce: u64,
    pub block_number: BlockNumber,
}

pub fn evm_burn_storage_key_for_message_id(message_id: H256) -> H256 {
    let mut slot_bytes = [0u8; 32];
    slot_bytes[24..].copy_from_slice(&SCCP_EVM_BURNS_MAPPING_SLOT.to_be_bytes());
    let mut preimage = [0u8; 64];
    preimage[..32].copy_from_slice(&message_id.0);
    preimage[32..].copy_from_slice(&slot_bytes);
    let slot_base = keccak_256(&preimage);
    H256::from_slice(&keccak_256(&slot_base))
}

#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    Clone,
    PartialEq,
    Eq,
    RuntimeDebug,
    scale_info::TypeInfo,
    MaxEncodedLen,
)]
pub struct EthZkFinalizedBurnPublicInputsV1 {
    pub message_id: H256,
    pub finalized_block_hash: H256,
    pub execution_state_root: H256,
    pub router_address: [u8; 20],
    pub burn_storage_key: H256,
}

#[derive(
    Encode, Decode, DecodeWithMemTracking, Clone, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo,
)]
pub struct EthZkFinalizedBurnProofV1 {
    pub version: u8,
    pub public_inputs: EthZkFinalizedBurnPublicInputsV1,
    pub evm_burn_proof: Vec<u8>,
    pub zk_proof: Vec<u8>,
}

/// ETH zk mode execution proof bundle (v1).
///
/// This is separate from `EvmBurnProofV1`: zk mode binds an execution header to the public inputs
/// and then proves account/storage inclusion against the header's `state_root`.
#[derive(
    Encode, Decode, DecodeWithMemTracking, Clone, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo,
)]
pub struct EthZkEvmBurnProofV1 {
    /// RLP-encoded Ethereum execution header. The runtime derives `block_hash` and `state_root`
    /// from this header and matches them against the zk proof public inputs.
    pub execution_header_rlp: Vec<u8>,
    /// EVM state trie proof for the SCCP router account (RLP-encoded MPT nodes).
    pub account_proof: Vec<Vec<u8>>,
    /// EVM storage trie proof for `burns[messageId].sender` (RLP-encoded MPT nodes).
    pub storage_proof: Vec<Vec<u8>>,
}

fn encode_eth_zk_bytes_as_u128_pair(bytes: &[u8]) -> [u128; 2] {
    debug_assert!(bytes.len() <= 32);
    let mut padded = [0u8; 32];
    padded[32 - bytes.len()..].copy_from_slice(bytes);
    [
        u128::from_be_bytes(
            padded[..16]
                .try_into()
                .expect("left-padded public input must fit into 16 bytes"),
        ),
        u128::from_be_bytes(
            padded[16..]
                .try_into()
                .expect("left-padded public input must fit into 16 bytes"),
        ),
    ]
}

/// Canonical ETH zk public-input packing for `EthZkFinalizedBurnProofV1`.
///
/// Concrete proof-system backends map these 10 `u128` limbs into their proving field in-order.
pub fn eth_zk_public_inputs_v1(
    public_inputs: &EthZkFinalizedBurnPublicInputsV1,
) -> [u128; ETH_ZK_PUBLIC_INPUT_COUNT_V1] {
    let message_id = encode_eth_zk_bytes_as_u128_pair(&public_inputs.message_id.0);
    let finalized_block_hash =
        encode_eth_zk_bytes_as_u128_pair(&public_inputs.finalized_block_hash.0);
    let execution_state_root =
        encode_eth_zk_bytes_as_u128_pair(&public_inputs.execution_state_root.0);
    let router_address = encode_eth_zk_bytes_as_u128_pair(&public_inputs.router_address);
    let burn_storage_key = encode_eth_zk_bytes_as_u128_pair(&public_inputs.burn_storage_key.0);

    [
        message_id[0],
        message_id[1],
        finalized_block_hash[0],
        finalized_block_hash[1],
        execution_state_root[0],
        execution_state_root[1],
        router_address[0],
        router_address[1],
        burn_storage_key[0],
        burn_storage_key[1],
    ]
}

pub fn decode_eth_zk_finalized_burn_proof_v1(proof: &[u8]) -> Option<EthZkFinalizedBurnProofV1> {
    let mut input = proof;
    let decoded = EthZkFinalizedBurnProofV1::decode(&mut input).ok()?;
    if !input.is_empty()
        || decoded.version != ETH_ZK_FINALIZED_BURN_PROOF_VERSION_V1
        || decoded.evm_burn_proof.len() > SCCP_MAX_ETH_ZK_PROOF_BYTES
        || decoded.zk_proof.len() > SCCP_MAX_ETH_ZK_PROOF_BYTES
    {
        return None;
    }
    Some(decoded)
}

#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    Clone,
    PartialEq,
    Eq,
    RuntimeDebug,
    scale_info::TypeInfo,
    MaxEncodedLen,
)]
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

#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    Clone,
    PartialEq,
    Eq,
    RuntimeDebug,
    scale_info::TypeInfo,
    MaxEncodedLen,
)]
pub struct SolanaVoteAuthorityV1 {
    pub authority_pubkey: [u8; 32],
    pub stake: u64,
}

#[derive(
    Encode, Decode, DecodeWithMemTracking, Clone, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo,
)]
pub struct SolanaMerkleProofV1 {
    pub path: Vec<u8>,
    pub siblings: Vec<Vec<H256>>,
}

#[derive(
    Encode, Decode, DecodeWithMemTracking, Clone, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo,
)]
pub struct SolanaAccountInfoV1 {
    pub pubkey: [u8; 32],
    pub lamports: u64,
    pub owner: [u8; 32],
    pub executable: bool,
    pub rent_epoch: u64,
    pub data: Vec<u8>,
    pub write_version: u64,
    pub slot: u64,
}

#[derive(
    Encode, Decode, DecodeWithMemTracking, Clone, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo,
)]
pub struct SolanaAccountDeltaProofV1 {
    pub account: SolanaAccountInfoV1,
    pub merkle_proof: SolanaMerkleProofV1,
}

#[derive(
    Encode, Decode, DecodeWithMemTracking, Clone, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo,
)]
pub struct SolanaBankHashProofV1 {
    pub slot: u64,
    pub bank_hash: H256,
    pub account_delta_root: H256,
    pub parent_bank_hash: H256,
    pub blockhash: H256,
    pub num_sigs: u64,
    pub account_proof: SolanaAccountDeltaProofV1,
}

#[derive(
    Encode, Decode, DecodeWithMemTracking, Clone, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo,
)]
pub struct SolanaVoteProofV1 {
    pub authority_pubkey: [u8; 32],
    pub signature: [u8; 64],
    pub signed_message: Vec<u8>,
    pub vote_slot: u64,
    pub vote_bank_hash: H256,
    pub rooted_slot: Option<u64>,
    pub slot_hashes_proof: SolanaBankHashProofV1,
}

#[derive(
    Encode, Decode, DecodeWithMemTracking, Clone, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo,
)]
pub struct SolanaFinalizedBurnProofV1 {
    pub version: u8,
    pub public_inputs: SolanaFinalizedBurnPublicInputsV1,
    pub burn_proof: SolanaBankHashProofV1,
    pub vote_proofs: Vec<SolanaVoteProofV1>,
}

pub fn decode_solana_finalized_burn_proof_v1(proof: &[u8]) -> Option<SolanaFinalizedBurnProofV1> {
    let mut input = proof;
    let decoded = SolanaFinalizedBurnProofV1::decode(&mut input).ok()?;
    if !input.is_empty()
        || decoded.version != SOLANA_FINALIZED_BURN_PROOF_VERSION_V1
        || decoded.vote_proofs.is_empty()
        || decoded.burn_proof.account_proof.account.data.len() > SCCP_MAX_SOLANA_ACCOUNT_DATA_BYTES
        || decoded.burn_proof.account_proof.merkle_proof.path.len() > SCCP_MAX_SOLANA_MERKLE_DEPTH
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
        {
            return None;
        }
        if vote
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

/// Governance-defined finality mode for inbound proofs to SORA per source domain.
#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    Copy,
    Clone,
    PartialEq,
    Eq,
    RuntimeDebug,
    scale_info::TypeInfo,
    MaxEncodedLen,
)]
pub enum InboundFinalityMode {
    /// Inbound from this domain is disabled (fail-closed).
    #[codec(index = 0)]
    Disabled,
    /// Reserved legacy slot for the removed governance-pinned EVM anchor mode.
    #[codec(index = 1)]
    LegacyEvmAnchor,
    /// BSC on-chain header verifier finalized state root only.
    #[codec(index = 2)]
    BscLightClient,
    /// Reserved legacy slot for the removed BSC light-client-or-anchor fallback mode.
    #[codec(index = 3)]
    LegacyBscLightClientOrAnchor,
    /// Ethereum beacon light client.
    #[codec(index = 4)]
    EthBeaconLightClient,
    /// Solana finalized-slot light client.
    #[codec(index = 5)]
    SolanaLightClient,
    /// TON masterchain light client.
    #[codec(index = 6)]
    TonLightClient,
    /// TRON witness light client: on-chain header verifier + "solidified block" finality (>70% witnesses).
    #[codec(index = 7)]
    TronLightClient,
    /// Substrate light client for SORA parachain domains (Kusama/Polkadot relay contexts).
    #[codec(index = 8)]
    SubstrateLightClient,
    /// Reserved legacy slot for the removed attester quorum mode.
    #[codec(index = 9)]
    LegacyAttesterQuorum,
    /// Ethereum finalized-burn zk proof verified on-chain by the SORA runtime.
    #[codec(index = 10)]
    EthZkProof,
}

#[allow(non_upper_case_globals)]
impl InboundFinalityMode {
    pub const EvmAnchor: Self = Self::LegacyEvmAnchor;
    pub const BscLightClientOrAnchor: Self = Self::LegacyBscLightClientOrAnchor;
    pub const AttesterQuorum: Self = Self::LegacyAttesterQuorum;
}

#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    Clone,
    PartialEq,
    Eq,
    RuntimeDebug,
    scale_info::TypeInfo,
    MaxEncodedLen,
)]
pub struct EvmInboundAnchor {
    pub block_number: u64,
    pub block_hash: H256,
    pub state_root: H256,
}

/// Governance-pinned TON finalized checkpoint used as the trust root for inbound TON proofs.
#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    Clone,
    PartialEq,
    Eq,
    RuntimeDebug,
    scale_info::TypeInfo,
    MaxEncodedLen,
)]
pub struct TonTrustedCheckpoint {
    pub mc_seqno: u32,
    pub mc_block_hash: H256,
}

/// Canonical TON burn-record fields stored in the SCCP jetton master under `burns[messageId]`.
#[derive(
    Encode, Decode, DecodeWithMemTracking, Clone, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo,
)]
pub struct TonBurnRecordV1 {
    pub dest_domain: u32,
    pub recipient32: [u8; 32],
    pub jetton_amount: Balance,
    pub nonce: u64,
}

/// Versioned TON -> SORA proof bundle emitted by `sccp-ton`.
///
/// The proof is intentionally self-contained and binds the submitted burn to:
/// - a governance-pinned TON finalized checkpoint,
/// - the configured jetton master account id (`remote_token_id`), and
/// - the configured jetton master code hash (`domain_endpoint`).
#[derive(
    Encode, Decode, DecodeWithMemTracking, Clone, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo,
)]
pub struct TonBurnProofV1 {
    pub version: u8,
    pub trusted_checkpoint_seqno: u32,
    pub trusted_checkpoint_hash: H256,
    pub target_mc_seqno: u32,
    pub target_mc_block_hash: H256,
    pub jetton_master_account_id: [u8; 32],
    pub jetton_master_code_hash: H256,
    pub burn_message_id: H256,
    pub burn_record: TonBurnRecordV1,
    pub masterchain_proof: Vec<u8>,
    pub shard_proof: Vec<u8>,
    pub account_proof: Vec<u8>,
    pub burns_dict_proof: Vec<u8>,
}

#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    Clone,
    PartialEq,
    Eq,
    RuntimeDebug,
    scale_info::TypeInfo,
    MaxEncodedLen,
)]
pub struct BscLightClientParams {
    pub epoch_length: u64,
    pub confirmation_depth: u64,
    /// Chain id used for BSC `types.SealHash(header, chainId)` signature verification.
    ///
    /// For BSC mainnet this is `56`.
    pub chain_id: u64,
    /// Parlia turn length (a.k.a. sprint length): number of consecutive blocks the in-turn
    /// validator is expected to produce.
    ///
    /// This affects both the difficulty rule and the "recent signer" rule.
    pub turn_length: u8,
}

#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    Clone,
    PartialEq,
    Eq,
    RuntimeDebug,
    scale_info::TypeInfo,
    MaxEncodedLen,
)]
pub struct BscHeaderMeta {
    pub hash: H256,
    pub number: u64,
    pub state_root: H256,
    pub signer: H160,
}

#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    Clone,
    PartialEq,
    Eq,
    RuntimeDebug,
    scale_info::TypeInfo,
    MaxEncodedLen,
)]
pub struct TronLightClientParams {
    /// TRON address prefix for witness addresses (typically `0x41` on mainnet).
    pub address_prefix: u8,
    /// Number of witnesses in the active schedule (TRON mainnet: 27).
    pub witness_count: u8,
    /// Finality threshold: `ceil(0.7 * witness_count)` (TRON mainnet: 19).
    pub solidification_threshold: u8,
}

#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    Clone,
    PartialEq,
    Eq,
    RuntimeDebug,
    scale_info::TypeInfo,
    MaxEncodedLen,
)]
pub struct TronHeaderMeta {
    pub hash: H256,
    pub number: u64,
    pub state_root: H256,
    pub signer: H160,
}

#[derive(Clone, PartialEq, Eq, RuntimeDebug)]
struct BscParsedHeader {
    hash: H256,
    parent_hash: H256,
    number: u64,
    state_root: H256,
    difficulty: u64,
    signer: H160,
    is_epoch: bool,
    epoch_validators: Vec<H160>,
    epoch_turn_length: Option<u8>,
}

#[derive(Clone, PartialEq, Eq, RuntimeDebug)]
struct TronParsedHeader {
    hash: H256,
    parent_hash: H256,
    number: u64,
    state_root: H256,
    signer: H160,
}

/// EVM inbound burn proof (v1).
///
/// The proof is verified against a governance-provided anchor `state_root` for the given domain.
#[derive(
    Encode, Decode, DecodeWithMemTracking, Clone, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo,
)]
pub struct EvmBurnProofV1 {
    pub anchor_block_hash: H256,
    /// EVM state trie proof for the SCCP router account (RLP-encoded MPT nodes).
    pub account_proof: Vec<Vec<u8>>,
    /// EVM storage trie proof for `burns[messageId].sender` (RLP-encoded MPT nodes).
    pub storage_proof: Vec<Vec<u8>>,
}

/// Ethereum finalized receipt burn proof (v1).
///
/// The proof is self-contained for the burn event itself:
/// - `execution_proof` proves an Ethereum execution payload is finalized under the on-chain ETH
///   beacon verifier,
/// - `receipt_proof` proves receipt inclusion under that execution payload's `receipts_root`,
/// - SCCP then matches the canonical `SccpBurned` log against the submitted burn payload.
#[derive(
    Encode, Decode, DecodeWithMemTracking, Clone, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo,
)]
pub struct EthFinalizedBurnProofV1 {
    pub version: u8,
    /// SCALE-encoded `snowbridge_beacon_primitives::ExecutionProof`.
    pub execution_proof: Vec<u8>,
    /// Ethereum receipt trie proof (RLP-encoded MPT nodes).
    pub receipt_proof: Vec<Vec<u8>>,
}

pub fn decode_eth_finalized_burn_proof_v1(proof: &[u8]) -> Option<EthFinalizedBurnProofV1> {
    let mut input = proof;
    let decoded = EthFinalizedBurnProofV1::decode(&mut input).ok()?;
    if !input.is_empty()
        || decoded.version != ETH_FINALIZED_RECEIPT_BURN_PROOF_VERSION_V1
        || decoded.execution_proof.len() > SCCP_MAX_ETH_FINALIZED_BURN_PROOF_BYTES
        || decoded.receipt_proof.len() > SCCP_MAX_EVM_PROOF_NODES
    {
        return None;
    }

    let mut total = 0usize;
    for node in decoded.receipt_proof.iter() {
        if node.len() > SCCP_MAX_EVM_PROOF_NODE_BYTES {
            return None;
        }
        total = total.saturating_add(node.len());
        if total > SCCP_MAX_EVM_PROOF_TOTAL_BYTES {
            return None;
        }
    }

    Some(decoded)
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use common::AssetManager;
    use common::FromGenericPair;
    use frame_support::traits::Get;
    use sp_runtime::traits::Convert;
    use sp_runtime::BoundedVec;
    use sp_runtime::Saturating;

    #[pallet::config]
    pub trait Config:
        frame_system::Config + technical::Config + permissions::Config + common::Config
    {
        #[allow(deprecated)]
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Governance-protected origin for managing SCCP configuration.
        type ManagerOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        /// Converts 32-byte recipient encoding into a local account ID.
        type AccountIdConverter: sp_runtime::traits::Convert<[u8; 32], Self::AccountId>;

        /// Asset info provider (typically `assets::Pallet`).
        type AssetInfoProvider: AssetInfoProvider<
            AssetIdOf<Self>,
            Self::AccountId,
            AssetSymbol,
            AssetName,
            common::BalancePrecision,
            common::ContentSource,
            common::Description,
        >;

        /// Checks whether an asset is already registered on the legacy bridge.
        type LegacyBridgeAssetChecker: LegacyBridgeAssetChecker<AssetIdOf<Self>>;

        /// Handler used to append SCCP burn commitments to the on-chain auxiliary digest.
        ///
        /// In production this should be configured to `LeafProvider`, so burns can be proven to
        /// external chains via BEEFY+MMR.
        type AuxiliaryDigestHandler: AuxiliaryDigestHandler;

        /// Provider for finalized ETH execution state roots used by `EthBeaconLightClient` mode.
        ///
        /// This should be wired to an on-chain Ethereum light client integration. Returning `None`
        /// keeps ETH inbound verification fail-closed.
        type EthFinalizedStateProvider: EthFinalizedStateProvider;

        /// Provider for finalized Ethereum burn-log verification used by
        /// `EthBeaconLightClient` mode.
        ///
        /// Returning unavailable keeps ETH inbound verification fail-closed.
        type EthFinalizedBurnProofVerifier: EthFinalizedBurnProofVerifier;

        /// Provider for zk-proven Ethereum finalized burn verification used by `EthZkProof` mode.
        ///
        /// Returning unavailable keeps ETH inbound verification fail-closed for zk-proof mode.
        type EthZkFinalizedBurnProofVerifier: EthZkFinalizedBurnProofVerifier;

        /// Provider for trustless Solana finalized-slot burn verification used by
        /// `SolanaLightClient` mode.
        ///
        /// Returning unavailable keeps SOL inbound verification fail-closed.
        type SolanaFinalizedBurnProofVerifier: SolanaFinalizedBurnProofVerifier;

        /// Provider for trustless Substrate finalized burn verification used by
        /// `SubstrateLightClient` mode.
        ///
        /// Returning unavailable keeps Substrate-domain inbound verification fail-closed.
        type SubstrateFinalizedBurnProofVerifier: SubstrateFinalizedBurnProofVerifier;

        /// Max length (in bytes) of a remote token identifier stored on SORA (address/pubkey/etc.).
        #[pallet::constant]
        type MaxRemoteTokenIdLen: Get<u32>;

        /// Max number of required domains.
        #[pallet::constant]
        type MaxDomains: Get<u32>;

        /// Max number of validators for the BSC on-chain light client.
        #[pallet::constant]
        type MaxBscValidators: Get<u32>;

        /// Max number of external authorities tracked for SCCP proof systems.
        #[pallet::constant]
        type MaxAttesters: Get<u32>;

        type WeightInfo: WeightInfo;
    }

    pub type AssetIdOf<T> = common::AssetIdOf<T>;
    pub type RemoteTokenIdOf<T> = BoundedVec<u8, <T as Config>::MaxRemoteTokenIdLen>;
    pub type RequiredDomainsOf<T> = BoundedVec<u32, <T as Config>::MaxDomains>;
    pub type BscValidatorsOf<T> = BoundedVec<H160, <T as Config>::MaxBscValidators>;
    pub type SolanaVoteAuthoritiesOf<T> =
        BoundedVec<SolanaVoteAuthorityV1, <T as Config>::MaxAttesters>;

    #[pallet::storage]
    #[pallet::getter(fn token_state)]
    pub(super) type Tokens<T: Config> =
        StorageMap<_, Blake2_128Concat, AssetIdOf<T>, TokenState<BlockNumberFor<T>>, OptionQuery>;

    /// SCCP router/program identifier per remote domain, configured by governance.
    ///
    /// This is required for **inbound** proof verification (e.g., verifying an EVM storage proof
    /// requires knowing the router contract address).
    #[pallet::storage]
    #[pallet::getter(fn domain_endpoint)]
    pub(super) type DomainEndpoint<T: Config> =
        StorageMap<_, Blake2_128Concat, u32, RemoteTokenIdOf<T>, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn inbound_grace_period)]
    pub(super) type InboundGracePeriod<T: Config> = StorageValue<_, BlockNumberFor<T>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn required_domains)]
    pub(super) type RequiredDomains<T: Config> = StorageValue<_, RequiredDomainsOf<T>, ValueQuery>;

    /// Governance pause switch for inbound SCCP operations from a given source domain.
    #[pallet::storage]
    #[pallet::getter(fn inbound_domain_paused)]
    pub(super) type InboundDomainPaused<T: Config> =
        StorageMap<_, Blake2_128Concat, u32, bool, ValueQuery>;

    /// Governance pause switch for outbound SCCP operations to a given destination domain.
    #[pallet::storage]
    #[pallet::getter(fn outbound_domain_paused)]
    pub(super) type OutboundDomainPaused<T: Config> =
        StorageMap<_, Blake2_128Concat, u32, bool, ValueQuery>;

    /// Governance-selected finality mode for inbound proofs per source domain.
    ///
    /// If unset for a domain, pallet defaults are used (see `default_inbound_finality_mode`).
    #[pallet::storage]
    #[pallet::getter(fn inbound_finality_mode_override)]
    pub(super) type InboundFinalityModes<T: Config> =
        StorageMap<_, Blake2_128Concat, u32, InboundFinalityMode, OptionQuery>;

    /// Governance-pinned Solana vote-authority set used for trustless SOL -> SORA proofs.
    ///
    /// Each entry binds an authorized vote signer to its stake weight for the relevant Solana
    /// epoch. The SORA verifier requires a strict >2/3 supermajority over this set.
    #[pallet::storage]
    #[pallet::getter(fn solana_vote_authorities)]
    pub(super) type SolanaVoteAuthorities<T: Config> =
        StorageValue<_, SolanaVoteAuthoritiesOf<T>, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn remote_token)]
    pub(super) type RemoteToken<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        AssetIdOf<T>,
        Blake2_128Concat,
        u32,
        RemoteTokenIdOf<T>,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn nonce)]
    pub(super) type Nonce<T: Config> = StorageValue<_, u64, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn burns)]
    pub(super) type Burns<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        H256,
        BurnRecord<T::AccountId, AssetIdOf<T>, BlockNumberFor<T>>,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn processed_inbound)]
    pub(super) type ProcessedInbound<T: Config> =
        StorageMap<_, Blake2_128Concat, H256, bool, ValueQuery>;

    /// Messages that were verified on SORA for a non-SORA destination and committed into the digest.
    ///
    /// This is used for direct (source -> dest) transfers where SORA serves as the trustless
    /// verification hub: SORA verifies the source-chain burn, then commits the `messageId` into
    /// its auxiliary digest so the destination chain can mint by verifying a SORA BEEFY+MMR proof.
    #[pallet::storage]
    #[pallet::getter(fn attested_outbound)]
    pub(super) type AttestedOutbound<T: Config> =
        StorageMap<_, Blake2_128Concat, H256, bool, ValueQuery>;

    /// Governance denylist for specific inbound messages (burn proofs) from a given source domain.
    #[pallet::storage]
    #[pallet::getter(fn invalidated_inbound)]
    pub(super) type InvalidatedInbound<T: Config> =
        StorageDoubleMap<_, Blake2_128Concat, u32, Blake2_128Concat, H256, bool, ValueQuery>;

    // === BSC light client (inbound-to-SORA finality) ===

    #[pallet::storage]
    #[pallet::getter(fn bsc_params)]
    pub(super) type BscParams<T: Config> = StorageValue<_, BscLightClientParams, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn bsc_validators)]
    pub(super) type BscValidators<T: Config> = StorageValue<_, BscValidatorsOf<T>, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn bsc_head)]
    pub(super) type BscHead<T: Config> = StorageValue<_, BscHeaderMeta, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn bsc_finalized)]
    pub(super) type BscFinalized<T: Config> = StorageValue<_, BscHeaderMeta, OptionQuery>;

    /// Recently imported BSC headers (by number).
    ///
    /// This is used for:
    /// - clique/parlia "recent signer" rule checks
    /// - finalized header lookup at `head.number - confirmation_depth`
    #[pallet::storage]
    #[pallet::getter(fn bsc_header_by_number)]
    pub(super) type BscHeadersByNumber<T: Config> =
        StorageMap<_, Blake2_128Concat, u64, BscHeaderMeta, OptionQuery>;

    /// Validator sets observed on BSC epoch blocks (by epoch block number).
    ///
    /// Used to apply validator-set changes trustlessly once the consensus-defined activation
    /// point is reached.
    #[pallet::storage]
    #[pallet::getter(fn bsc_epoch_validators)]
    pub(super) type BscEpochValidators<T: Config> =
        StorageMap<_, Blake2_128Concat, u64, BscValidatorsOf<T>, OptionQuery>;

    /// Turn length (sprint length) observed on a BSC epoch block (by epoch block number).
    #[pallet::storage]
    #[pallet::getter(fn bsc_epoch_turn_length)]
    pub(super) type BscEpochTurnLength<T: Config> =
        StorageMap<_, Blake2_128Concat, u64, u8, OptionQuery>;

    /// Lower bound (exclusive) for "recent signer" checks on the BSC header verifier.
    ///
    /// This is used to emulate Parlia's behavior of clearing miner history during validator-set
    /// switches, without deleting historical header metadata needed for finalized state-root lookup.
    #[pallet::storage]
    #[pallet::getter(fn bsc_recents_lower_bound)]
    pub(super) type BscRecentsLowerBound<T: Config> = StorageValue<_, u64, ValueQuery>;

    // === TRON light client (inbound-to-SORA finality) ===

    #[pallet::storage]
    #[pallet::getter(fn tron_params)]
    pub(super) type TronParams<T: Config> = StorageValue<_, TronLightClientParams, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn tron_witnesses)]
    pub(super) type TronWitnesses<T: Config> = StorageValue<_, BscValidatorsOf<T>, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn tron_head)]
    pub(super) type TronHead<T: Config> = StorageValue<_, TronHeaderMeta, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn tron_finalized)]
    pub(super) type TronFinalized<T: Config> = StorageValue<_, TronHeaderMeta, OptionQuery>;

    /// Recently imported TRON headers (by number).
    #[pallet::storage]
    #[pallet::getter(fn tron_header_by_number)]
    pub(super) type TronHeadersByNumber<T: Config> =
        StorageMap<_, Blake2_128Concat, u64, TronHeaderMeta, OptionQuery>;

    // === TON inbound trust root ===

    #[pallet::storage]
    #[pallet::getter(fn ton_trusted_checkpoint)]
    pub(super) type TonTrustedCheckpointState<T: Config> =
        StorageValue<_, TonTrustedCheckpoint, OptionQuery>;

    #[pallet::type_value]
    pub fn DefaultInboundGracePeriod<T: Config>() -> BlockNumberFor<T> {
        // Default: ~7 days, assuming 6s blocks (~100800 blocks/week). Governance can adjust.
        100_800u32.into()
    }

    #[pallet::type_value]
    pub fn DefaultRequiredDomains<T: Config>() -> RequiredDomainsOf<T> {
        default_required_domains_for_bound::<T::MaxDomains>()
    }

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub inbound_grace_period: BlockNumberFor<T>,
        pub required_domains: RequiredDomainsOf<T>,
    }

    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                inbound_grace_period: DefaultInboundGracePeriod::<T>::get(),
                required_domains: DefaultRequiredDomains::<T>::get(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            InboundGracePeriod::<T>::set(self.inbound_grace_period);
            let normalized =
                Pallet::<T>::normalize_required_domains(self.required_domains.clone().into_inner())
                    .expect("invalid SCCP genesis required_domains");
            RequiredDomains::<T>::set(normalized);
        }
    }

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_runtime_upgrade() -> Weight {
            Weight::zero()
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        TokenAdded {
            asset_id: AssetIdOf<T>,
        },
        RemoteTokenSet {
            asset_id: AssetIdOf<T>,
            domain_id: u32,
            id_hash: H256,
        },
        DomainEndpointSet {
            domain_id: u32,
            id_hash: H256,
        },
        DomainEndpointCleared {
            domain_id: u32,
        },
        TokenActivated {
            asset_id: AssetIdOf<T>,
        },
        TokenRemoved {
            asset_id: AssetIdOf<T>,
            inbound_enabled_until: BlockNumberFor<T>,
        },
        TokenRemovalFinalized {
            asset_id: AssetIdOf<T>,
        },
        InboundGracePeriodSet {
            blocks: BlockNumberFor<T>,
        },
        RequiredDomainsSet {
            domains_hash: H256,
        },
        InboundDomainPausedSet {
            domain_id: u32,
            paused: bool,
        },
        OutboundDomainPausedSet {
            domain_id: u32,
            paused: bool,
        },
        InboundFinalityModeSet {
            domain_id: u32,
            mode: InboundFinalityMode,
        },
        SolanaVoteAuthoritiesSet {
            authorities_hash: H256,
            total_stake: u64,
            threshold_stake: u64,
        },
        SolanaVoteAuthoritiesCleared,
        InboundMessageInvalidated {
            source_domain: u32,
            message_id: H256,
        },
        InboundMessageRevalidated {
            source_domain: u32,
            message_id: H256,
        },
        BscLightClientInitialized {
            head_hash: H256,
            head_number: u64,
        },
        BscHeaderImported {
            hash: H256,
            number: u64,
            signer: H160,
            state_root: H256,
        },
        BscFinalizedUpdated {
            hash: H256,
            number: u64,
            state_root: H256,
        },
        BscValidatorsUpdated {
            number: u64,
            validators_hash: H256,
        },

        TronLightClientInitialized {
            head_hash: H256,
            head_number: u64,
        },
        TronHeaderImported {
            hash: H256,
            number: u64,
            signer: H160,
            state_root: H256,
        },
        TronFinalizedUpdated {
            hash: H256,
            number: u64,
            state_root: H256,
        },
        TronWitnessesUpdated {
            number: u64,
            witnesses_hash: H256,
        },
        TonTrustedCheckpointSet {
            mc_seqno: u32,
            mc_block_hash: H256,
        },
        TonTrustedCheckpointCleared,
        SccpBurned {
            message_id: H256,
            asset_id: AssetIdOf<T>,
            amount: Balance,
            dest_domain: u32,
            recipient: [u8; 32],
            nonce: u64,
        },
        SccpMinted {
            message_id: H256,
            asset_id: AssetIdOf<T>,
            amount: Balance,
            recipient: T::AccountId,
        },
        /// A non-SORA burn was verified on SORA and committed into the digest for minting on
        /// `dest_domain` via SORA BEEFY+MMR light clients.
        SccpBurnAttested {
            message_id: H256,
            asset_id: AssetIdOf<T>,
            amount: Balance,
            source_domain: u32,
            dest_domain: u32,
            recipient: [u8; 32],
            nonce: u64,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        TokenAlreadyExists,
        TokenNotFound,
        TokenNotPending,
        TokenNotActive,
        TokenNotRemoving,
        OutboundDisabled,
        InboundDisabled,
        GracePeriodNotExpired,
        DomainUnsupported,
        RemoteTokenMissing,
        RemoteTokenInvalidLength,
        DomainEndpointMissing,
        DomainEndpointInvalidLength,
        RecipientIsZero,
        AmountIsZero,
        NonceOverflow,
        BurnRecordAlreadyExists,
        BurnRecordNotFound,
        InboundAlreadyProcessed,
        BurnAlreadyAttested,
        InboundDomainPaused,
        OutboundDomainPaused,
        ProofInvalidated,
        ProofVerificationFailed,
        AssetSupplyNotMintable,
        RecipientNotCanonical,
        AssetOnLegacyBridge,
        RequiredDomainsInvalid,
        InboundFinalityModeUnsupported,
        InboundFinalityUnavailable,
        BscLightClientNotInitialized,
        BscHeaderTooLarge,
        BscHeaderInvalid,
        BscValidatorsInvalid,
        TronLightClientNotInitialized,
        TronHeaderTooLarge,
        TronHeaderInvalid,
        TronWitnessesInvalid,
        TonProofTooLarge,
        SolanaVoteAuthoritiesInvalid,
    }

    #[allow(non_upper_case_globals)]
    impl<T> Error<T> {
        pub const InboundFinalityModeDeprecated: Self = Self::InboundFinalityModeUnsupported;
        pub const EvmInboundAnchorMissing: Self = Self::InboundFinalityUnavailable;
        pub const InboundAttestersInvalid: Self = Self::SolanaVoteAuthoritiesInvalid;
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Add a token to SCCP (governance).
        ///
        /// Creates a `Pending` token entry and reserves SCCP's scoped mint/burn permissions.
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::add_token())]
        #[transactional]
        pub fn add_token(origin: OriginFor<T>, asset_id: AssetIdOf<T>) -> DispatchResult {
            T::ManagerOrigin::ensure_origin(origin)?;
            ensure!(
                !Tokens::<T>::contains_key(&asset_id),
                Error::<T>::TokenAlreadyExists
            );
            <T as Config>::AssetInfoProvider::ensure_asset_exists(&asset_id)?;
            ensure!(
                !T::LegacyBridgeAssetChecker::is_legacy_bridge_asset(&asset_id),
                Error::<T>::AssetOnLegacyBridge
            );
            Self::ensure_asset_is_mintable(&asset_id)?;
            Tokens::<T>::insert(&asset_id, TokenState::pending());
            Self::ensure_sccp_permissions(&asset_id)?;
            Self::deposit_event(Event::TokenAdded { asset_id });
            Ok(())
        }

        /// Set remote wrapped token identifier for a given domain (governance).
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::set_remote_token())]
        #[transactional]
        pub fn set_remote_token(
            origin: OriginFor<T>,
            asset_id: AssetIdOf<T>,
            domain_id: u32,
            remote_token_id: Vec<u8>,
        ) -> DispatchResult {
            T::ManagerOrigin::ensure_origin(origin)?;
            ensure!(
                Tokens::<T>::contains_key(&asset_id),
                Error::<T>::TokenNotFound
            );
            ensure!(domain_id != SCCP_DOMAIN_SORA, Error::<T>::DomainUnsupported);
            Self::ensure_supported_domain(domain_id)?;
            Self::ensure_remote_token_len(domain_id, remote_token_id.len())?;
            let bounded: RemoteTokenIdOf<T> = remote_token_id
                .try_into()
                .map_err(|_| Error::<T>::RemoteTokenInvalidLength)?;
            let id_hash = H256::from_slice(&keccak_256(bounded.as_slice()));
            RemoteToken::<T>::insert(&asset_id, domain_id, bounded);
            Self::deposit_event(Event::RemoteTokenSet {
                asset_id,
                domain_id,
                id_hash,
            });
            Ok(())
        }

        /// Set the SCCP router/program identifier for a given remote domain (governance).
        ///
        /// This is a global per-domain config, not per-token.
        #[pallet::call_index(12)]
        #[pallet::weight(<T as Config>::WeightInfo::set_domain_endpoint())]
        #[transactional]
        pub fn set_domain_endpoint(
            origin: OriginFor<T>,
            domain_id: u32,
            endpoint_id: Vec<u8>,
        ) -> DispatchResult {
            T::ManagerOrigin::ensure_origin(origin)?;
            ensure!(domain_id != SCCP_DOMAIN_SORA, Error::<T>::DomainUnsupported);
            Self::ensure_supported_domain(domain_id)?;
            Self::ensure_domain_endpoint_len(domain_id, endpoint_id.len())?;
            let bounded: RemoteTokenIdOf<T> = endpoint_id
                .try_into()
                .map_err(|_| Error::<T>::DomainEndpointInvalidLength)?;
            let id_hash = H256::from_slice(&keccak_256(bounded.as_slice()));
            DomainEndpoint::<T>::insert(domain_id, bounded);
            Self::deposit_event(Event::DomainEndpointSet { domain_id, id_hash });
            Ok(())
        }

        /// Initialize the on-chain BSC header-chain verifier (governance).
        ///
        /// This is the bootstrap step for trustless BSC inbound-to-SORA proofs:
        /// after initialization, anyone can submit subsequent headers, and SCCP can use the
        /// finalized BSC state root to verify MPT storage proofs.
        #[pallet::call_index(16)]
        #[pallet::weight(<T as Config>::WeightInfo::init_bsc_light_client(validators.len() as u32))]
        #[transactional]
        pub fn init_bsc_light_client(
            origin: OriginFor<T>,
            checkpoint_header_rlp: Vec<u8>,
            validators: Vec<H160>,
            epoch_length: u64,
            confirmation_depth: u64,
            chain_id: u64,
            turn_length: u8,
        ) -> DispatchResult {
            T::ManagerOrigin::ensure_origin(origin)?;
            ensure!(
                checkpoint_header_rlp.len() <= SCCP_MAX_BSC_HEADER_RLP_BYTES,
                Error::<T>::BscHeaderTooLarge
            );
            ensure!(epoch_length > 0, Error::<T>::BscValidatorsInvalid);
            ensure!(chain_id > 0, Error::<T>::BscValidatorsInvalid);
            ensure!(turn_length > 0, Error::<T>::BscValidatorsInvalid);
            ensure!(
                confirmation_depth < SCCP_BSC_HEADER_RETENTION,
                Error::<T>::BscValidatorsInvalid
            );
            let mut sorted = validators.clone();
            sorted.sort();
            ensure!(
                sorted.windows(2).all(|w| w[0] != w[1]),
                Error::<T>::BscValidatorsInvalid
            );
            let bounded: BscValidatorsOf<T> = sorted
                .clone()
                .try_into()
                .map_err(|_| Error::<T>::BscValidatorsInvalid)?;
            ensure!(!bounded.is_empty(), Error::<T>::BscValidatorsInvalid);

            let parsed = Self::bsc_parse_and_verify_header(
                &checkpoint_header_rlp,
                &bounded,
                epoch_length,
                chain_id,
            )?;

            // Re-initialization must clear old header history to avoid mixing chains or
            // breaking the "recent signer" rule checks.
            let _ = BscHeadersByNumber::<T>::clear(u32::MAX, None);
            let _ = BscEpochValidators::<T>::clear(u32::MAX, None);
            let _ = BscEpochTurnLength::<T>::clear(u32::MAX, None);
            BscHead::<T>::kill();
            BscFinalized::<T>::kill();
            BscRecentsLowerBound::<T>::set(0);

            BscParams::<T>::set(Some(BscLightClientParams {
                epoch_length,
                confirmation_depth,
                chain_id,
                turn_length,
            }));
            BscValidators::<T>::set(Some(bounded));
            BscHead::<T>::set(Some(BscHeaderMeta {
                hash: parsed.hash,
                number: parsed.number,
                state_root: parsed.state_root,
                signer: parsed.signer,
            }));
            BscHeadersByNumber::<T>::insert(
                parsed.number,
                BscHeaderMeta {
                    hash: parsed.hash,
                    number: parsed.number,
                    state_root: parsed.state_root,
                    signer: parsed.signer,
                },
            );
            if parsed.is_epoch {
                if !parsed.epoch_validators.is_empty() {
                    let bounded_epoch: BscValidatorsOf<T> = parsed
                        .epoch_validators
                        .clone()
                        .try_into()
                        .map_err(|_| Error::<T>::BscValidatorsInvalid)?;
                    BscEpochValidators::<T>::insert(parsed.number, bounded_epoch);
                }
                if let Some(tl) = parsed.epoch_turn_length {
                    BscEpochTurnLength::<T>::insert(parsed.number, tl);
                }
            }

            // Initialize finalized state if confirmation depth is 0.
            if confirmation_depth == 0 {
                BscFinalized::<T>::set(Some(BscHeaderMeta {
                    hash: parsed.hash,
                    number: parsed.number,
                    state_root: parsed.state_root,
                    signer: parsed.signer,
                }));
                Self::deposit_event(Event::BscFinalizedUpdated {
                    hash: parsed.hash,
                    number: parsed.number,
                    state_root: parsed.state_root,
                });
            } else {
                BscFinalized::<T>::kill();
            }

            // If the checkpoint is an epoch block with a validator list, require it matches the
            // configured validator set.
            if parsed.is_epoch && !parsed.epoch_validators.is_empty() {
                ensure!(
                    parsed.epoch_validators == sorted,
                    Error::<T>::BscValidatorsInvalid
                );
                let validators_hash = H256::from_slice(&keccak_256(&sorted.encode()));
                Self::deposit_event(Event::BscValidatorsUpdated {
                    number: parsed.number,
                    validators_hash,
                });
            }

            Self::deposit_event(Event::BscLightClientInitialized {
                head_hash: parsed.hash,
                head_number: parsed.number,
            });
            Ok(())
        }

        /// Submit a new BSC header to advance the on-chain verifier (permissionless).
        #[pallet::call_index(17)]
        #[pallet::weight(<T as Config>::WeightInfo::submit_bsc_header(header_rlp.len() as u32))]
        #[transactional]
        pub fn submit_bsc_header(origin: OriginFor<T>, header_rlp: Vec<u8>) -> DispatchResult {
            let _ = ensure_signed(origin)?;
            ensure!(
                header_rlp.len() <= SCCP_MAX_BSC_HEADER_RLP_BYTES,
                Error::<T>::BscHeaderTooLarge
            );
            let params = BscParams::<T>::get().ok_or(Error::<T>::BscLightClientNotInitialized)?;
            let validators =
                BscValidators::<T>::get().ok_or(Error::<T>::BscLightClientNotInitialized)?;
            let head = BscHead::<T>::get().ok_or(Error::<T>::BscLightClientNotInitialized)?;

            let parsed = Self::bsc_parse_and_verify_header(
                &header_rlp,
                &validators,
                params.epoch_length,
                params.chain_id,
            )?;

            // Only accept linear extension for now (fail-closed on forks).
            ensure!(
                parsed.number == head.number.saturating_add(1),
                Error::<T>::BscHeaderInvalid
            );
            ensure!(
                parsed.parent_hash == head.hash,
                Error::<T>::BscHeaderInvalid
            );

            // Enforce parlia "recent signer" rule.
            let vlen = validators.len() as u64;
            let turn_len = params.turn_length as u64;
            ensure!(turn_len > 0, Error::<T>::BscHeaderInvalid);
            let check_len = (vlen / 2)
                .saturating_add(1)
                .saturating_mul(turn_len)
                .saturating_sub(1);
            if check_len > 0 {
                let lb = BscRecentsLowerBound::<T>::get();
                let mut seen: u64 = 0;
                for i in 1..=check_len {
                    let n = match parsed.number.checked_sub(i) {
                        Some(n) => n,
                        None => break,
                    };
                    if n <= lb {
                        break;
                    }
                    if let Some(prev) = BscHeadersByNumber::<T>::get(n) {
                        if prev.signer == parsed.signer {
                            seen = seen.saturating_add(1);
                            // A validator may only sign `turn_length` blocks in the recent window.
                            ensure!(seen < turn_len, Error::<T>::BscHeaderInvalid);
                        }
                    }
                }
            }

            // Enforce in-turn difficulty rule (Clique/Parlia style).
            let expected_idx = ((parsed.number / turn_len) % vlen) as usize;
            let expected = validators
                .get(expected_idx)
                .copied()
                .ok_or(Error::<T>::BscHeaderInvalid)?;
            let in_turn = parsed.signer == expected;
            let expected_diff = if in_turn { 2u64 } else { 1u64 };
            ensure!(
                parsed.difficulty == expected_diff,
                Error::<T>::BscHeaderInvalid
            );

            // Epoch blocks may carry a validator list (and, post-Bohr, turn length).
            // These are recorded and applied at the consensus-defined activation point.

            let meta = BscHeaderMeta {
                hash: parsed.hash,
                number: parsed.number,
                state_root: parsed.state_root,
                signer: parsed.signer,
            };
            BscHead::<T>::set(Some(meta.clone()));
            BscHeadersByNumber::<T>::insert(parsed.number, meta.clone());
            Self::deposit_event(Event::BscHeaderImported {
                hash: meta.hash,
                number: meta.number,
                signer: meta.signer,
                state_root: meta.state_root,
            });

            // Record epoch metadata (validator list / turn length), if present.
            if parsed.is_epoch {
                if !parsed.epoch_validators.is_empty() {
                    let bounded_epoch: BscValidatorsOf<T> = parsed
                        .epoch_validators
                        .clone()
                        .try_into()
                        .map_err(|_| Error::<T>::BscValidatorsInvalid)?;
                    BscEpochValidators::<T>::insert(parsed.number, bounded_epoch);
                }
                if let Some(tl) = parsed.epoch_turn_length {
                    BscEpochTurnLength::<T>::insert(parsed.number, tl);
                }
            }

            // Update finalized header, if possible.
            if params.confirmation_depth == 0 {
                BscFinalized::<T>::set(Some(meta.clone()));
                Self::deposit_event(Event::BscFinalizedUpdated {
                    hash: meta.hash,
                    number: meta.number,
                    state_root: meta.state_root,
                });
            } else if meta.number >= params.confirmation_depth {
                let finalized_number = meta.number - params.confirmation_depth;
                if let Some(f) = BscHeadersByNumber::<T>::get(finalized_number) {
                    BscFinalized::<T>::set(Some(f.clone()));
                    Self::deposit_event(Event::BscFinalizedUpdated {
                        hash: f.hash,
                        number: f.number,
                        state_root: f.state_root,
                    });
                }
            }

            // Trustless validator-set (and turn length) updates.
            //
            // In Parlia, epoch blocks (number % epoch_length == 0) carry the next validator set,
            // but the set is applied after `minerHistoryCheckLen()` blocks. We replicate that
            // activation rule in a linear-chain setting.
            if meta.number > 0
                && check_len > 0
                && (meta.number % params.epoch_length) == check_len
                && meta.number >= check_len
            {
                let epoch_block = meta.number - check_len;
                if let Some(new_vals) = BscEpochValidators::<T>::get(epoch_block) {
                    let mut new_turn = params.turn_length;
                    if let Some(tl) = BscEpochTurnLength::<T>::get(epoch_block) {
                        if tl > 0 {
                            new_turn = tl;
                        }
                    }

                    let validators_changed = new_vals.as_slice() != validators.as_slice();
                    let turn_length_changed = new_turn != params.turn_length;
                    if validators_changed || turn_length_changed {
                        BscValidators::<T>::set(Some(new_vals.clone()));
                        BscRecentsLowerBound::<T>::set(meta.number);
                        BscParams::<T>::set(Some(BscLightClientParams {
                            epoch_length: params.epoch_length,
                            confirmation_depth: params.confirmation_depth,
                            chain_id: params.chain_id,
                            turn_length: new_turn,
                        }));

                        if validators_changed {
                            let validators_hash =
                                H256::from_slice(&keccak_256(&new_vals.into_inner().encode()));
                            Self::deposit_event(Event::BscValidatorsUpdated {
                                number: meta.number,
                                validators_hash,
                            });
                        }
                    }
                }
            }

            // Prune old headers.
            if meta.number > SCCP_BSC_HEADER_RETENTION {
                let prune = meta.number - SCCP_BSC_HEADER_RETENTION;
                BscHeadersByNumber::<T>::remove(prune);
                BscEpochValidators::<T>::remove(prune);
                BscEpochTurnLength::<T>::remove(prune);
            }

            Ok(())
        }

        /// Update the configured BSC validator set (governance).
        ///
        /// The BSC light client attempts to apply validator-set updates automatically from
        /// epoch headers. This extrinsic exists as an emergency override (e.g., if the verifier
        /// gets stuck fail-closed due to an unexpected consensus upgrade).
        #[pallet::call_index(18)]
        #[pallet::weight(<T as Config>::WeightInfo::set_bsc_validators(validators.len() as u32))]
        #[transactional]
        pub fn set_bsc_validators(origin: OriginFor<T>, validators: Vec<H160>) -> DispatchResult {
            T::ManagerOrigin::ensure_origin(origin)?;
            let mut sorted = validators.clone();
            sorted.sort();
            ensure!(
                sorted.windows(2).all(|w| w[0] != w[1]),
                Error::<T>::BscValidatorsInvalid
            );
            let bounded: BscValidatorsOf<T> = sorted
                .clone()
                .try_into()
                .map_err(|_| Error::<T>::BscValidatorsInvalid)?;
            ensure!(!bounded.is_empty(), Error::<T>::BscValidatorsInvalid);
            BscValidators::<T>::set(Some(bounded));

            let validators_hash = H256::from_slice(&keccak_256(&sorted.encode()));
            let number = BscHead::<T>::get().map(|h| h.number).unwrap_or(0);
            Self::deposit_event(Event::BscValidatorsUpdated {
                number,
                validators_hash,
            });
            Ok(())
        }

        /// Initialize the on-chain TRON header verifier (governance).
        ///
        /// This enables trustless TRON inbound-to-SORA proofs once a finalized/soldified TRON
        /// header is bootstrapped. Subsequent headers can be imported permissionlessly and
        /// solidification will advance on-chain.
        #[pallet::call_index(23)]
        #[pallet::weight(<T as Config>::WeightInfo::init_tron_light_client(witnesses.len() as u32))]
        #[transactional]
        pub fn init_tron_light_client(
            origin: OriginFor<T>,
            checkpoint_raw_data: Vec<u8>,
            checkpoint_witness_signature: Vec<u8>,
            witnesses: Vec<H160>,
            address_prefix: u8,
        ) -> DispatchResult {
            T::ManagerOrigin::ensure_origin(origin)?;
            ensure!(
                checkpoint_raw_data.len() <= SCCP_MAX_TRON_RAW_DATA_BYTES,
                Error::<T>::TronHeaderTooLarge
            );
            ensure!(
                checkpoint_witness_signature.len() == 65,
                Error::<T>::TronHeaderInvalid
            );

            let mut sorted = witnesses.clone();
            sorted.sort();
            ensure!(
                sorted.windows(2).all(|w| w[0] != w[1]),
                Error::<T>::TronWitnessesInvalid
            );
            let bounded: BscValidatorsOf<T> = sorted
                .clone()
                .try_into()
                .map_err(|_| Error::<T>::TronWitnessesInvalid)?;
            ensure!(!bounded.is_empty(), Error::<T>::TronWitnessesInvalid);

            let witness_count: u8 = bounded
                .len()
                .try_into()
                .map_err(|_| Error::<T>::TronWitnessesInvalid)?;
            // TRON solidification rule: >70% approvals.
            let threshold_u64 = ((bounded.len() as u64) * 70 + 99) / 100;
            let solidification_threshold: u8 = threshold_u64
                .try_into()
                .map_err(|_| Error::<T>::TronWitnessesInvalid)?;
            ensure!(
                solidification_threshold > 0,
                Error::<T>::TronWitnessesInvalid
            );

            let params = TronLightClientParams {
                address_prefix,
                witness_count,
                solidification_threshold,
            };

            let mut sig = [0u8; 65];
            sig.copy_from_slice(checkpoint_witness_signature.as_slice());
            let parsed = Self::tron_parse_and_verify_header(
                checkpoint_raw_data.as_slice(),
                &sig,
                &bounded,
                &params,
            )?;

            // Re-initialization must clear old header history to avoid mixing chains.
            let _ = TronHeadersByNumber::<T>::clear(u32::MAX, None);
            TronHead::<T>::kill();
            TronFinalized::<T>::kill();

            TronParams::<T>::set(Some(params));
            TronWitnesses::<T>::set(Some(bounded));

            let meta = TronHeaderMeta {
                hash: parsed.hash,
                number: parsed.number,
                state_root: parsed.state_root,
                signer: parsed.signer,
            };
            TronHead::<T>::set(Some(meta.clone()));
            TronHeadersByNumber::<T>::insert(parsed.number, meta.clone());

            // Bootstrap finalized state as the provided checkpoint (governance must pick a
            // known solidified header).
            TronFinalized::<T>::set(Some(meta.clone()));
            Self::deposit_event(Event::TronFinalizedUpdated {
                hash: meta.hash,
                number: meta.number,
                state_root: meta.state_root,
            });

            let witnesses_hash = H256::from_slice(&keccak_256(&sorted.encode()));
            Self::deposit_event(Event::TronWitnessesUpdated {
                number: parsed.number,
                witnesses_hash,
            });

            Self::deposit_event(Event::TronLightClientInitialized {
                head_hash: parsed.hash,
                head_number: parsed.number,
            });
            Ok(())
        }

        /// Submit a new TRON header to advance the on-chain verifier (permissionless).
        #[pallet::call_index(24)]
        #[pallet::weight(<T as Config>::WeightInfo::submit_tron_header(raw_data.len() as u32))]
        #[transactional]
        pub fn submit_tron_header(
            origin: OriginFor<T>,
            raw_data: Vec<u8>,
            witness_signature: Vec<u8>,
        ) -> DispatchResult {
            let _ = ensure_signed(origin)?;
            ensure!(
                raw_data.len() <= SCCP_MAX_TRON_RAW_DATA_BYTES,
                Error::<T>::TronHeaderTooLarge
            );
            ensure!(witness_signature.len() == 65, Error::<T>::TronHeaderInvalid);

            let params = TronParams::<T>::get().ok_or(Error::<T>::TronLightClientNotInitialized)?;
            let witnesses =
                TronWitnesses::<T>::get().ok_or(Error::<T>::TronLightClientNotInitialized)?;
            let head = TronHead::<T>::get().ok_or(Error::<T>::TronLightClientNotInitialized)?;

            let mut sig = [0u8; 65];
            sig.copy_from_slice(witness_signature.as_slice());
            let parsed =
                Self::tron_parse_and_verify_header(raw_data.as_slice(), &sig, &witnesses, &params)?;

            // Only accept linear extension for now (fail-closed on forks).
            ensure!(
                parsed.number == head.number.saturating_add(1),
                Error::<T>::TronHeaderInvalid
            );
            ensure!(
                parsed.parent_hash == head.hash,
                Error::<T>::TronHeaderInvalid
            );

            let meta = TronHeaderMeta {
                hash: parsed.hash,
                number: parsed.number,
                state_root: parsed.state_root,
                signer: parsed.signer,
            };
            TronHead::<T>::set(Some(meta.clone()));
            TronHeadersByNumber::<T>::insert(parsed.number, meta.clone());

            Self::deposit_event(Event::TronHeaderImported {
                hash: meta.hash,
                number: meta.number,
                signer: meta.signer,
                state_root: meta.state_root,
            });

            // Advance solidified header under the TRON "irreversible block" rule:
            // A block is solidified once it is followed by enough blocks produced by distinct
            // witnesses, exceeding 70% of the witness set.
            if let Some(finalized) = TronFinalized::<T>::get() {
                let threshold = params.solidification_threshold as u64;
                if threshold > 0 {
                    let candidate = finalized.number.saturating_add(1);
                    let needed_head = candidate.saturating_add(threshold.saturating_sub(1));
                    if meta.number >= needed_head {
                        let end = needed_head;
                        let mut seen: Vec<H160> = Vec::with_capacity(threshold as usize);
                        let mut ok = true;
                        let mut n = candidate;
                        while n <= end {
                            let Some(h) = TronHeadersByNumber::<T>::get(n) else {
                                ok = false;
                                break;
                            };
                            if seen.iter().any(|x| *x == h.signer) {
                                ok = false;
                                break;
                            }
                            seen.push(h.signer);
                            n = n.saturating_add(1);
                        }
                        if ok {
                            if let Some(new_finalized) = TronHeadersByNumber::<T>::get(candidate) {
                                TronFinalized::<T>::set(Some(new_finalized.clone()));
                                Self::deposit_event(Event::TronFinalizedUpdated {
                                    hash: new_finalized.hash,
                                    number: new_finalized.number,
                                    state_root: new_finalized.state_root,
                                });
                            }
                        }
                    }
                }
            }

            // Prune old headers.
            if meta.number > SCCP_TRON_HEADER_RETENTION {
                let prune = meta.number - SCCP_TRON_HEADER_RETENTION;
                TronHeadersByNumber::<T>::remove(prune);
            }

            Ok(())
        }

        /// Update the configured TRON witness set (governance).
        ///
        /// TRON witness rotation is not carried in block headers; governance must update the
        /// witness set when needed (or re-initialize the light client at epoch boundaries).
        #[pallet::call_index(25)]
        #[pallet::weight(<T as Config>::WeightInfo::set_tron_witnesses(witnesses.len() as u32))]
        #[transactional]
        pub fn set_tron_witnesses(origin: OriginFor<T>, witnesses: Vec<H160>) -> DispatchResult {
            T::ManagerOrigin::ensure_origin(origin)?;
            let mut sorted = witnesses.clone();
            sorted.sort();
            ensure!(
                sorted.windows(2).all(|w| w[0] != w[1]),
                Error::<T>::TronWitnessesInvalid
            );
            let bounded: BscValidatorsOf<T> = sorted
                .clone()
                .try_into()
                .map_err(|_| Error::<T>::TronWitnessesInvalid)?;
            ensure!(!bounded.is_empty(), Error::<T>::TronWitnessesInvalid);
            TronWitnesses::<T>::set(Some(bounded));

            // Update params-derived threshold.
            if let Some(mut params) = TronParams::<T>::get() {
                let witness_count: u8 = sorted
                    .len()
                    .try_into()
                    .map_err(|_| Error::<T>::TronWitnessesInvalid)?;
                let threshold_u64 = ((sorted.len() as u64) * 70 + 99) / 100;
                let solidification_threshold: u8 = threshold_u64
                    .try_into()
                    .map_err(|_| Error::<T>::TronWitnessesInvalid)?;
                params.witness_count = witness_count;
                params.solidification_threshold = solidification_threshold;
                TronParams::<T>::set(Some(params));
            }

            let witnesses_hash = H256::from_slice(&keccak_256(&sorted.encode()));
            let number = TronHead::<T>::get().map(|h| h.number).unwrap_or(0);
            Self::deposit_event(Event::TronWitnessesUpdated {
                number,
                witnesses_hash,
            });
            Ok(())
        }

        /// Clear the SCCP router/program identifier for a given remote domain (governance).
        #[pallet::call_index(13)]
        #[pallet::weight(<T as Config>::WeightInfo::clear_domain_endpoint())]
        pub fn clear_domain_endpoint(origin: OriginFor<T>, domain_id: u32) -> DispatchResult {
            T::ManagerOrigin::ensure_origin(origin)?;
            ensure!(domain_id != SCCP_DOMAIN_SORA, Error::<T>::DomainUnsupported);
            Self::ensure_supported_domain(domain_id)?;
            DomainEndpoint::<T>::remove(domain_id);
            Self::deposit_event(Event::DomainEndpointCleared { domain_id });
            Ok(())
        }

        /// Set the trusted TON finalized checkpoint used by `TonLightClient` mode (governance).
        #[pallet::call_index(28)]
        #[pallet::weight(<T as Config>::WeightInfo::set_domain_endpoint())]
        pub fn set_ton_trusted_checkpoint(
            origin: OriginFor<T>,
            mc_seqno: u32,
            mc_block_hash: H256,
        ) -> DispatchResult {
            T::ManagerOrigin::ensure_origin(origin)?;
            TonTrustedCheckpointState::<T>::put(TonTrustedCheckpoint {
                mc_seqno,
                mc_block_hash,
            });
            Self::deposit_event(Event::TonTrustedCheckpointSet {
                mc_seqno,
                mc_block_hash,
            });
            Ok(())
        }

        /// Clear the trusted TON finalized checkpoint (governance).
        #[pallet::call_index(29)]
        #[pallet::weight(<T as Config>::WeightInfo::clear_domain_endpoint())]
        pub fn clear_ton_trusted_checkpoint(origin: OriginFor<T>) -> DispatchResult {
            T::ManagerOrigin::ensure_origin(origin)?;
            TonTrustedCheckpointState::<T>::kill();
            Self::deposit_event(Event::TonTrustedCheckpointCleared);
            Ok(())
        }

        /// Activate a previously-added token (governance).
        ///
        /// Requires remote token identifiers and domain endpoints for:
        /// - all `RequiredDomains` configured by governance, and
        /// - all SCCP core remote domains (ETH/BSC/SOL/TON/TRON).
        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::activate_token())]
        #[transactional]
        pub fn activate_token(origin: OriginFor<T>, asset_id: AssetIdOf<T>) -> DispatchResult {
            T::ManagerOrigin::ensure_origin(origin)?;
            Tokens::<T>::try_mutate(&asset_id, |state| -> DispatchResult {
                let Some(state) = state.as_mut() else {
                    return Err(Error::<T>::TokenNotFound.into());
                };
                ensure!(
                    matches!(state.status, TokenStatus::Pending),
                    Error::<T>::TokenNotPending
                );
                // Ensure remote token IDs are configured for governance-required domains.
                for domain_id in RequiredDomains::<T>::get().into_inner().into_iter() {
                    Self::ensure_token_domain_activation_configured(&asset_id, domain_id)?;
                }

                // Security invariant: SCCP tokens must have deployed representations and
                // configured endpoints on every core target chain.
                for domain_id in SCCP_CORE_REMOTE_DOMAINS.into_iter() {
                    Self::ensure_token_domain_activation_configured(&asset_id, domain_id)?;
                }
                state.status = TokenStatus::Active;
                state.outbound_enabled = true;
                state.inbound_enabled = true;
                state.inbound_enabled_until = None;
                Ok(())
            })?;
            Self::deposit_event(Event::TokenActivated { asset_id });
            Ok(())
        }

        /// Remove a token from SCCP (governance).
        ///
        /// Outbound burns are disabled immediately. Inbound mints are allowed only until
        /// `now + InboundGracePeriod`.
        #[pallet::call_index(3)]
        #[pallet::weight(<T as Config>::WeightInfo::remove_token())]
        #[transactional]
        pub fn remove_token(origin: OriginFor<T>, asset_id: AssetIdOf<T>) -> DispatchResult {
            T::ManagerOrigin::ensure_origin(origin)?;
            let now = frame_system::Pallet::<T>::block_number();
            let until = now.saturating_add(InboundGracePeriod::<T>::get());
            Tokens::<T>::try_mutate(&asset_id, |state| -> DispatchResult {
                let Some(state) = state.as_mut() else {
                    return Err(Error::<T>::TokenNotFound.into());
                };
                state.status = TokenStatus::Removing;
                state.outbound_enabled = false;
                state.inbound_enabled = false;
                state.inbound_enabled_until = Some(until);
                Ok(())
            })?;
            Self::deposit_event(Event::TokenRemoved {
                asset_id,
                inbound_enabled_until: until,
            });
            Ok(())
        }

        /// Finalize token removal after grace period expires (governance).
        #[pallet::call_index(4)]
        #[pallet::weight(<T as Config>::WeightInfo::finalize_remove())]
        #[transactional]
        pub fn finalize_remove(origin: OriginFor<T>, asset_id: AssetIdOf<T>) -> DispatchResult {
            T::ManagerOrigin::ensure_origin(origin)?;
            let now = frame_system::Pallet::<T>::block_number();
            let state = Tokens::<T>::get(&asset_id).ok_or(Error::<T>::TokenNotFound)?;
            ensure!(
                matches!(state.status, TokenStatus::Removing),
                Error::<T>::TokenNotRemoving
            );
            let Some(until) = state.inbound_enabled_until else {
                return Err(Error::<T>::TokenNotRemoving.into());
            };
            ensure!(now > until, Error::<T>::GracePeriodNotExpired);

            Tokens::<T>::remove(&asset_id);
            // Remove all configured remote token ids for this asset.
            let _ = RemoteToken::<T>::clear_prefix(&asset_id, u32::MAX, None);
            Self::deposit_event(Event::TokenRemovalFinalized { asset_id });
            Ok(())
        }

        /// Update inbound grace period (governance).
        #[pallet::call_index(5)]
        #[pallet::weight(<T as Config>::WeightInfo::set_inbound_grace_period())]
        pub fn set_inbound_grace_period(
            origin: OriginFor<T>,
            blocks: BlockNumberFor<T>,
        ) -> DispatchResult {
            T::ManagerOrigin::ensure_origin(origin)?;
            InboundGracePeriod::<T>::set(blocks);
            Self::deposit_event(Event::InboundGracePeriodSet { blocks });
            Ok(())
        }

        /// Update required domains list (governance).
        ///
        /// For first release, this list must be exactly SCCP core remote domains
        /// (ETH/BSC/SOL/TON/TRON), persisted in canonical sorted order.
        #[pallet::call_index(6)]
        #[pallet::weight(<T as Config>::WeightInfo::set_required_domains(domains.len() as u32))]
        pub fn set_required_domains(origin: OriginFor<T>, domains: Vec<u32>) -> DispatchResult {
            T::ManagerOrigin::ensure_origin(origin)?;
            let bounded = Self::normalize_required_domains(domains)?;
            let sorted = bounded.clone().into_inner();
            // Persist canonical ordering to avoid equivalent sets producing different state hashes.
            RequiredDomains::<T>::set(bounded);
            let domains_hash = H256::from_slice(&keccak_256(&sorted.encode()));
            Self::deposit_event(Event::RequiredDomainsSet { domains_hash });
            Ok(())
        }

        /// Set inbound finality mode for a source domain (governance).
        ///
        /// This defines how proofs from `domain_id` are considered finalized for minting/attesting on SORA.
        #[pallet::call_index(22)]
        #[pallet::weight(<T as Config>::WeightInfo::set_inbound_finality_mode())]
        pub fn set_inbound_finality_mode(
            origin: OriginFor<T>,
            domain_id: u32,
            mode: InboundFinalityMode,
        ) -> DispatchResult {
            T::ManagerOrigin::ensure_origin(origin)?;
            ensure!(domain_id != SCCP_DOMAIN_SORA, Error::<T>::DomainUnsupported);
            Self::ensure_supported_domain(domain_id)?;
            Self::ensure_inbound_finality_mode_supported(domain_id, mode)?;

            InboundFinalityModes::<T>::insert(domain_id, mode);
            Self::deposit_event(Event::InboundFinalityModeSet { domain_id, mode });
            Ok(())
        }

        /// Pause or resume inbound SCCP operations coming from a specific source domain (governance).
        #[pallet::call_index(9)]
        #[pallet::weight(<T as Config>::WeightInfo::set_inbound_domain_paused())]
        pub fn set_inbound_domain_paused(
            origin: OriginFor<T>,
            domain_id: u32,
            paused: bool,
        ) -> DispatchResult {
            T::ManagerOrigin::ensure_origin(origin)?;
            ensure!(domain_id != SCCP_DOMAIN_SORA, Error::<T>::DomainUnsupported);
            Self::ensure_supported_domain(domain_id)?;
            InboundDomainPaused::<T>::insert(domain_id, paused);
            Self::deposit_event(Event::InboundDomainPausedSet { domain_id, paused });
            Ok(())
        }

        /// Invalidate a specific inbound SCCP burn message so it can never be minted on SORA (governance).
        #[pallet::call_index(10)]
        #[pallet::weight(<T as Config>::WeightInfo::invalidate_inbound_message())]
        pub fn invalidate_inbound_message(
            origin: OriginFor<T>,
            source_domain: u32,
            message_id: H256,
        ) -> DispatchResult {
            T::ManagerOrigin::ensure_origin(origin)?;
            ensure!(
                source_domain != SCCP_DOMAIN_SORA,
                Error::<T>::DomainUnsupported
            );
            Self::ensure_supported_domain(source_domain)?;
            InvalidatedInbound::<T>::insert(source_domain, message_id, true);
            Self::deposit_event(Event::InboundMessageInvalidated {
                source_domain,
                message_id,
            });
            Ok(())
        }

        /// Remove an invalidation for an inbound SCCP burn message (governance).
        #[pallet::call_index(11)]
        #[pallet::weight(<T as Config>::WeightInfo::clear_invalidated_inbound_message())]
        pub fn clear_invalidated_inbound_message(
            origin: OriginFor<T>,
            source_domain: u32,
            message_id: H256,
        ) -> DispatchResult {
            T::ManagerOrigin::ensure_origin(origin)?;
            ensure!(
                source_domain != SCCP_DOMAIN_SORA,
                Error::<T>::DomainUnsupported
            );
            Self::ensure_supported_domain(source_domain)?;
            InvalidatedInbound::<T>::remove(source_domain, message_id);
            Self::deposit_event(Event::InboundMessageRevalidated {
                source_domain,
                message_id,
            });
            Ok(())
        }

        /// Burn tokens on SORA and create an on-chain burn record that can be proven to a target chain.
        #[pallet::call_index(7)]
        #[pallet::weight(<T as Config>::WeightInfo::burn())]
        #[transactional]
        pub fn burn(
            origin: OriginFor<T>,
            asset_id: AssetIdOf<T>,
            amount: Balance,
            dest_domain: u32,
            recipient: [u8; 32],
        ) -> DispatchResult {
            let sender = ensure_signed(origin)?;
            ensure!(amount > Zero::zero(), Error::<T>::AmountIsZero);
            ensure!(recipient != [0u8; 32], Error::<T>::RecipientIsZero);
            ensure!(
                dest_domain != SCCP_DOMAIN_SORA,
                Error::<T>::DomainUnsupported
            );
            Self::ensure_supported_domain(dest_domain)?;
            ensure!(
                !OutboundDomainPaused::<T>::get(dest_domain),
                Error::<T>::OutboundDomainPaused
            );
            // EVM recipient encoding: 20-byte address right-aligned in a 32-byte field.
            // Enforce canonical encoding when the destination is an EVM domain to avoid ambiguous
            // representations and guaranteed-mint failures on the destination router.
            if matches!(
                dest_domain,
                SCCP_DOMAIN_ETH | SCCP_DOMAIN_BSC | SCCP_DOMAIN_TRON
            ) {
                ensure!(
                    recipient[..12] == [0u8; 12],
                    Error::<T>::RecipientNotCanonical
                );
            }
            ensure!(
                DomainEndpoint::<T>::contains_key(dest_domain),
                Error::<T>::DomainEndpointMissing
            );

            let state = Tokens::<T>::get(&asset_id).ok_or(Error::<T>::TokenNotFound)?;
            ensure!(
                matches!(state.status, TokenStatus::Active),
                Error::<T>::TokenNotActive
            );
            ensure!(state.outbound_enabled, Error::<T>::OutboundDisabled);

            ensure!(
                RemoteToken::<T>::contains_key(&asset_id, dest_domain),
                Error::<T>::RemoteTokenMissing
            );

            // Global monotonically-increasing nonce to guarantee unique message ids.
            let nonce = Nonce::<T>::try_mutate(|n| -> Result<u64, DispatchError> {
                ensure!(*n != u64::MAX, Error::<T>::NonceOverflow);
                *n += 1;
                Ok(*n)
            })?;

            let asset_h256: H256 = asset_id.into();
            let payload = BurnPayloadV1 {
                version: 1,
                source_domain: SCCP_DOMAIN_SORA,
                dest_domain,
                nonce,
                sora_asset_id: asset_h256.0,
                amount,
                recipient,
            };
            let message_id = Self::burn_message_id(&payload);
            ensure!(
                !Burns::<T>::contains_key(message_id),
                Error::<T>::BurnRecordAlreadyExists
            );

            // Burn from sender using SCCP technical account as issuer (scoped permission).
            let sccp_account = Self::sccp_account()?;
            <T as common::Config>::AssetManager::burn_from(
                &asset_id,
                &sccp_account,
                &sender,
                amount,
            )?;

            Burns::<T>::insert(
                message_id,
                BurnRecord {
                    sender: sender.clone(),
                    asset_id,
                    amount,
                    dest_domain,
                    recipient,
                    nonce,
                    block_number: frame_system::Pallet::<T>::block_number(),
                },
            );

            // Commit the burn message id into the auxiliary digest so it can be proven to other
            // chains via BEEFY+MMR light clients.
            T::AuxiliaryDigestHandler::add_item(AuxiliaryDigestItem::Commitment(
                digest_network_id_for_domain(dest_domain),
                bridge_types::H256::from_slice(message_id.as_bytes()),
            ));

            Self::deposit_event(Event::SccpBurned {
                message_id,
                asset_id,
                amount,
                dest_domain,
                recipient,
                nonce,
            });
            Ok(())
        }

        /// Pause or resume outbound SCCP operations targeting a specific destination domain (governance).
        #[pallet::call_index(19)]
        #[pallet::weight(<T as Config>::WeightInfo::set_outbound_domain_paused())]
        pub fn set_outbound_domain_paused(
            origin: OriginFor<T>,
            domain_id: u32,
            paused: bool,
        ) -> DispatchResult {
            T::ManagerOrigin::ensure_origin(origin)?;
            ensure!(domain_id != SCCP_DOMAIN_SORA, Error::<T>::DomainUnsupported);
            Self::ensure_supported_domain(domain_id)?;
            OutboundDomainPaused::<T>::insert(domain_id, paused);
            Self::deposit_event(Event::OutboundDomainPausedSet { domain_id, paused });
            Ok(())
        }

        /// Mint tokens on SORA based on an on-chain verifiable proof of burn on a source chain.
        ///
        /// Proof verification is source-chain-specific and controlled by
        /// `set_inbound_finality_mode` (governance).
        #[pallet::call_index(8)]
        #[pallet::weight(<T as Config>::WeightInfo::mint_from_proof())]
        #[transactional]
        pub fn mint_from_proof(
            origin: OriginFor<T>,
            source_domain: u32,
            payload: BurnPayloadV1,
            proof: Vec<u8>,
        ) -> DispatchResult {
            let _ = ensure_signed(origin)?;

            // Dest must be SORA.
            ensure!(
                payload.dest_domain == SCCP_DOMAIN_SORA,
                Error::<T>::DomainUnsupported
            );
            ensure!(
                payload.source_domain == source_domain,
                Error::<T>::DomainUnsupported
            );
            ensure!(payload.version == 1, Error::<T>::DomainUnsupported);
            ensure!(
                source_domain != SCCP_DOMAIN_SORA,
                Error::<T>::DomainUnsupported
            );
            Self::ensure_supported_domain(source_domain)?;
            ensure!(
                DomainEndpoint::<T>::contains_key(source_domain),
                Error::<T>::DomainEndpointMissing
            );
            Self::ensure_inbound_finality_available(source_domain)?;
            ensure!(
                !InboundDomainPaused::<T>::get(source_domain),
                Error::<T>::InboundDomainPaused
            );
            ensure!(payload.amount > Zero::zero(), Error::<T>::AmountIsZero);
            ensure!(payload.recipient != [0u8; 32], Error::<T>::RecipientIsZero);
            let asset_id: AssetIdOf<T> = AssetIdOf::<T>::from(H256(payload.sora_asset_id));

            // Require that the token has a deployed representation on the source domain.
            ensure!(
                RemoteToken::<T>::contains_key(&asset_id, source_domain),
                Error::<T>::RemoteTokenMissing
            );

            // Token must be active or in grace period.
            let state = Tokens::<T>::get(&asset_id).ok_or(Error::<T>::TokenNotFound)?;
            let now = frame_system::Pallet::<T>::block_number();
            let inbound_allowed = match state.status {
                TokenStatus::Active => state.inbound_enabled,
                TokenStatus::Removing => state
                    .inbound_enabled_until
                    .map(|until| now <= until)
                    .unwrap_or(false),
                TokenStatus::Pending => false,
            };
            ensure!(inbound_allowed, Error::<T>::InboundDisabled);

            let message_id = Self::burn_message_id(&payload);
            ensure!(
                !InvalidatedInbound::<T>::get(source_domain, message_id),
                Error::<T>::ProofInvalidated
            );
            ensure!(
                !ProcessedInbound::<T>::get(message_id),
                Error::<T>::InboundAlreadyProcessed
            );

            // Verify burn proof using the configured inbound finality mode. This is fail-closed:
            // if the required light client (or anchor) is unavailable, minting must not happen.
            let verified =
                Self::verify_burn_proof(source_domain, &asset_id, &payload, message_id, &proof)?;
            ensure!(verified, Error::<T>::ProofVerificationFailed);

            let recipient: T::AccountId = T::AccountIdConverter::convert(payload.recipient);
            let sccp_account = Self::sccp_account()?;
            <T as common::Config>::AssetManager::mint_to(
                &asset_id,
                &sccp_account,
                &recipient,
                payload.amount,
            )?;

            ProcessedInbound::<T>::insert(message_id, true);
            Self::deposit_event(Event::SccpMinted {
                message_id,
                asset_id,
                amount: payload.amount,
                recipient,
            });
            Ok(())
        }

        /// Verify a burn on a remote chain and commit it into the SORA auxiliary digest so it can
        /// be minted on the destination chain via SORA BEEFY+MMR light clients.
        ///
        /// This enables direct transfers between non-SORA domains, with SORA acting as a
        /// trustless verification hub:
        /// 1. user burns on `source_domain` with `dest_domain != SORA`
        /// 2. user submits the burn proof to SORA via this extrinsic
        /// 3. user submits a SORA BEEFY+MMR proof of the digest commitment to `dest_domain` to mint
        #[pallet::call_index(21)]
        #[pallet::weight(<T as Config>::WeightInfo::attest_burn())]
        #[transactional]
        pub fn attest_burn(
            origin: OriginFor<T>,
            source_domain: u32,
            payload: BurnPayloadV1,
            proof: Vec<u8>,
        ) -> DispatchResult {
            let _ = ensure_signed(origin)?;

            // Basic payload sanity.
            ensure!(payload.version == 1, Error::<T>::DomainUnsupported);
            ensure!(
                payload.source_domain == source_domain,
                Error::<T>::DomainUnsupported
            );
            ensure!(
                source_domain != SCCP_DOMAIN_SORA,
                Error::<T>::DomainUnsupported
            );
            ensure!(
                payload.dest_domain != SCCP_DOMAIN_SORA,
                Error::<T>::DomainUnsupported
            );
            ensure!(
                payload.dest_domain != source_domain,
                Error::<T>::DomainUnsupported
            );
            Self::ensure_supported_domain(source_domain)?;
            Self::ensure_supported_domain(payload.dest_domain)?;

            // Incident controls.
            ensure!(
                !InboundDomainPaused::<T>::get(source_domain),
                Error::<T>::InboundDomainPaused
            );
            ensure!(
                !OutboundDomainPaused::<T>::get(payload.dest_domain),
                Error::<T>::OutboundDomainPaused
            );

            // Enforce canonical encoding when the destination is an EVM domain, to avoid
            // committing unmintable messages.
            if matches!(
                payload.dest_domain,
                SCCP_DOMAIN_ETH | SCCP_DOMAIN_BSC | SCCP_DOMAIN_TRON
            ) {
                ensure!(
                    payload.recipient[..12] == [0u8; 12],
                    Error::<T>::RecipientNotCanonical
                );
            }

            ensure!(payload.amount > Zero::zero(), Error::<T>::AmountIsZero);
            ensure!(payload.recipient != [0u8; 32], Error::<T>::RecipientIsZero);

            ensure!(
                DomainEndpoint::<T>::contains_key(source_domain),
                Error::<T>::DomainEndpointMissing
            );

            // Ensure source-chain finality verifier availability for the configured mode.
            Self::ensure_inbound_finality_available(source_domain)?;

            let asset_id: AssetIdOf<T> = AssetIdOf::<T>::from(H256(payload.sora_asset_id));

            // Token must exist and accept inbound operations.
            let state = Tokens::<T>::get(&asset_id).ok_or(Error::<T>::TokenNotFound)?;
            let now = frame_system::Pallet::<T>::block_number();
            let inbound_allowed = match state.status {
                TokenStatus::Active => state.inbound_enabled,
                TokenStatus::Removing => state
                    .inbound_enabled_until
                    .map(|until| now <= until)
                    .unwrap_or(false),
                TokenStatus::Pending => false,
            };
            ensure!(inbound_allowed, Error::<T>::InboundDisabled);

            // Require that the token has deployed representations on both source and destination.
            ensure!(
                RemoteToken::<T>::contains_key(&asset_id, source_domain),
                Error::<T>::RemoteTokenMissing
            );
            ensure!(
                RemoteToken::<T>::contains_key(&asset_id, payload.dest_domain),
                Error::<T>::RemoteTokenMissing
            );

            let message_id = Self::burn_message_id(&payload);
            ensure!(
                !InvalidatedInbound::<T>::get(source_domain, message_id),
                Error::<T>::ProofInvalidated
            );
            ensure!(
                !AttestedOutbound::<T>::get(message_id),
                Error::<T>::BurnAlreadyAttested
            );

            let verified =
                Self::verify_burn_proof(source_domain, &asset_id, &payload, message_id, &proof)?;
            ensure!(verified, Error::<T>::ProofVerificationFailed);

            AttestedOutbound::<T>::insert(message_id, true);
            T::AuxiliaryDigestHandler::add_item(AuxiliaryDigestItem::Commitment(
                digest_network_id_for_domain(payload.dest_domain),
                bridge_types::H256::from_slice(message_id.as_bytes()),
            ));

            Self::deposit_event(Event::SccpBurnAttested {
                message_id,
                asset_id,
                amount: payload.amount,
                source_domain,
                dest_domain: payload.dest_domain,
                recipient: payload.recipient,
                nonce: payload.nonce,
            });
            Ok(())
        }

        /// Configure the Solana vote-authority set used by `SolanaLightClient` mode (governance).
        ///
        /// The configured set is expected to match the trusted Solana epoch vote authorities and
        /// their stake weights. Verification requires a strict >2/3 supermajority by stake.
        #[pallet::call_index(30)]
        #[pallet::weight(<T as Config>::WeightInfo::set_solana_vote_authorities(authorities.len() as u32))]
        pub fn set_solana_vote_authorities(
            origin: OriginFor<T>,
            authorities: Vec<SolanaVoteAuthorityV1>,
        ) -> DispatchResult {
            T::ManagerOrigin::ensure_origin(origin)?;
            ensure!(
                !authorities.is_empty(),
                Error::<T>::SolanaVoteAuthoritiesInvalid
            );

            let mut sorted = authorities;
            sorted.sort_by(|a, b| a.authority_pubkey.cmp(&b.authority_pubkey));
            ensure!(
                sorted
                    .windows(2)
                    .all(|w| w[0].authority_pubkey != w[1].authority_pubkey),
                Error::<T>::SolanaVoteAuthoritiesInvalid
            );
            ensure!(
                sorted.iter().all(|a| a.stake > 0),
                Error::<T>::SolanaVoteAuthoritiesInvalid
            );

            let total_stake = sorted
                .iter()
                .try_fold(0u64, |acc, authority| acc.checked_add(authority.stake))
                .ok_or(Error::<T>::SolanaVoteAuthoritiesInvalid)?;
            ensure!(total_stake > 0, Error::<T>::SolanaVoteAuthoritiesInvalid);

            let bounded: SolanaVoteAuthoritiesOf<T> = sorted
                .clone()
                .try_into()
                .map_err(|_| Error::<T>::SolanaVoteAuthoritiesInvalid)?;
            SolanaVoteAuthorities::<T>::put(bounded);

            let authorities_hash = H256::from_slice(&keccak_256(&sorted.encode()));
            let threshold_stake = total_stake
                .checked_mul(2)
                .and_then(|v| v.checked_div(3))
                .and_then(|v| v.checked_add(1))
                .ok_or(Error::<T>::SolanaVoteAuthoritiesInvalid)?;
            Self::deposit_event(Event::SolanaVoteAuthoritiesSet {
                authorities_hash,
                total_stake,
                threshold_stake,
            });
            Ok(())
        }

        /// Clear the Solana vote-authority set used by `SolanaLightClient` mode (governance).
        #[pallet::call_index(31)]
        #[pallet::weight(<T as Config>::WeightInfo::clear_solana_vote_authorities())]
        pub fn clear_solana_vote_authorities(origin: OriginFor<T>) -> DispatchResult {
            T::ManagerOrigin::ensure_origin(origin)?;
            SolanaVoteAuthorities::<T>::kill();
            Self::deposit_event(Event::SolanaVoteAuthoritiesCleared);
            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        pub fn set_evm_inbound_anchor(
            origin: OriginFor<T>,
            domain_id: u32,
            block_number: u64,
            block_hash: H256,
            state_root: H256,
        ) -> DispatchResult {
            T::ManagerOrigin::ensure_origin(origin)?;
            let _ = (domain_id, block_number, block_hash, state_root);
            Err(Error::<T>::InboundFinalityModeUnsupported.into())
        }

        pub fn clear_evm_inbound_anchor(origin: OriginFor<T>, domain_id: u32) -> DispatchResult {
            T::ManagerOrigin::ensure_origin(origin)?;
            let _ = domain_id;
            Err(Error::<T>::InboundFinalityModeUnsupported.into())
        }

        pub fn set_evm_anchor_mode_enabled(
            origin: OriginFor<T>,
            domain_id: u32,
            enabled: bool,
        ) -> DispatchResult {
            T::ManagerOrigin::ensure_origin(origin)?;
            let _ = (domain_id, enabled);
            Err(Error::<T>::InboundFinalityModeUnsupported.into())
        }

        pub fn set_inbound_attesters(
            origin: OriginFor<T>,
            domain_id: u32,
            attesters: Vec<H160>,
            threshold: u32,
        ) -> DispatchResult {
            T::ManagerOrigin::ensure_origin(origin)?;
            let _ = (domain_id, attesters, threshold);
            Err(Error::<T>::InboundFinalityModeUnsupported.into())
        }

        pub fn clear_inbound_attesters(origin: OriginFor<T>, domain_id: u32) -> DispatchResult {
            T::ManagerOrigin::ensure_origin(origin)?;
            let _ = domain_id;
            Err(Error::<T>::InboundFinalityModeUnsupported.into())
        }

        pub fn evm_inbound_anchor(_domain_id: u32) -> Option<EvmInboundAnchor> {
            None
        }

        pub fn evm_anchor_mode_enabled(_domain_id: u32) -> bool {
            false
        }

        pub fn inbound_attesters(
            _domain_id: u32,
        ) -> Option<BoundedVec<H160, <T as Config>::MaxAttesters>> {
            None
        }

        pub fn inbound_attester_threshold(_domain_id: u32) -> Option<u32> {
            None
        }

        /// Returns true if `asset_id` is currently managed by SCCP (any status).
        pub fn is_sccp_asset(asset_id: &AssetIdOf<T>) -> bool {
            Tokens::<T>::contains_key(asset_id)
        }

        fn burn_message_id(payload: &BurnPayloadV1) -> H256 {
            let mut preimage = SCCP_MSG_PREFIX_BURN_V1.to_vec();
            preimage.extend(payload.encode());
            H256::from_slice(&keccak_256(&preimage))
        }

        fn sccp_tech_account() -> <T as technical::Config>::TechAccountId {
            FromGenericPair::from_generic_pair(
                SCCP_TECH_ACC_PREFIX.to_vec(),
                SCCP_TECH_ACC_MAIN.to_vec(),
            )
        }

        fn sccp_account() -> Result<T::AccountId, DispatchError> {
            technical::Pallet::<T>::register_tech_account_id_if_not_exist(
                &Self::sccp_tech_account(),
            )?;
            technical::Pallet::<T>::tech_account_id_to_account_id(&Self::sccp_tech_account())
        }

        fn ensure_sccp_permissions(asset_id: &AssetIdOf<T>) -> DispatchResult {
            let sccp_account = Self::sccp_account()?;
            let scope = Scope::Limited(hash(asset_id));
            for permission_id in [BURN, MINT] {
                if permissions::Pallet::<T>::check_permission_with_scope(
                    sccp_account.clone(),
                    permission_id,
                    &scope,
                )
                .is_err()
                {
                    permissions::Pallet::<T>::assign_permission(
                        sccp_account.clone(),
                        &sccp_account,
                        permission_id,
                        scope,
                    )?;
                }
            }
            Ok(())
        }

        fn ensure_supported_domain(domain_id: u32) -> Result<(), DispatchError> {
            match domain_id {
                SCCP_DOMAIN_SORA
                | SCCP_DOMAIN_ETH
                | SCCP_DOMAIN_BSC
                | SCCP_DOMAIN_SOL
                | SCCP_DOMAIN_TON
                | SCCP_DOMAIN_TRON
                | SCCP_DOMAIN_SORA_KUSAMA
                | SCCP_DOMAIN_SORA_POLKADOT => Ok(()),
                _ => Err(Error::<T>::DomainUnsupported.into()),
            }
        }

        fn normalize_required_domains(
            domains: Vec<u32>,
        ) -> Result<RequiredDomainsOf<T>, DispatchError> {
            for &domain_id in domains.iter() {
                ensure!(domain_id != SCCP_DOMAIN_SORA, Error::<T>::DomainUnsupported);
                Self::ensure_supported_domain(domain_id)?;
            }
            let mut sorted = domains;
            sorted.sort();
            ensure!(
                sorted.windows(2).all(|w| w[0] != w[1]),
                Error::<T>::RequiredDomainsInvalid
            );
            let mut required_core = SCCP_CORE_REMOTE_DOMAINS.to_vec();
            required_core.sort();
            ensure!(sorted == required_core, Error::<T>::RequiredDomainsInvalid);
            sorted
                .try_into()
                .map_err(|_| Error::<T>::RequiredDomainsInvalid.into())
        }

        fn ensure_remote_token_len(domain_id: u32, len: usize) -> Result<(), DispatchError> {
            let expected = match domain_id {
                SCCP_DOMAIN_ETH | SCCP_DOMAIN_BSC | SCCP_DOMAIN_TRON => 20usize,
                SCCP_DOMAIN_SOL
                | SCCP_DOMAIN_TON
                | SCCP_DOMAIN_SORA_KUSAMA
                | SCCP_DOMAIN_SORA_POLKADOT => 32usize,
                SCCP_DOMAIN_SORA => 0usize, // not used
                _ => return Err(Error::<T>::DomainUnsupported.into()),
            };
            if expected == 0 {
                return Err(Error::<T>::DomainUnsupported.into());
            }
            ensure!(len == expected, Error::<T>::RemoteTokenInvalidLength);
            Ok(())
        }

        fn ensure_domain_endpoint_len(domain_id: u32, len: usize) -> Result<(), DispatchError> {
            let expected = match domain_id {
                SCCP_DOMAIN_ETH | SCCP_DOMAIN_BSC | SCCP_DOMAIN_TRON => 20usize,
                SCCP_DOMAIN_SOL
                | SCCP_DOMAIN_TON
                | SCCP_DOMAIN_SORA_KUSAMA
                | SCCP_DOMAIN_SORA_POLKADOT => 32usize,
                SCCP_DOMAIN_SORA => 0usize, // not used
                _ => return Err(Error::<T>::DomainUnsupported.into()),
            };
            if expected == 0 {
                return Err(Error::<T>::DomainUnsupported.into());
            }
            ensure!(len == expected, Error::<T>::DomainEndpointInvalidLength);
            Ok(())
        }

        fn ensure_token_domain_activation_configured(
            asset_id: &AssetIdOf<T>,
            domain_id: u32,
        ) -> DispatchResult {
            let remote =
                RemoteToken::<T>::get(asset_id, domain_id).ok_or(Error::<T>::RemoteTokenMissing)?;
            Self::ensure_remote_token_len(domain_id, remote.len())?;

            let endpoint =
                DomainEndpoint::<T>::get(domain_id).ok_or(Error::<T>::DomainEndpointMissing)?;
            Self::ensure_domain_endpoint_len(domain_id, endpoint.len())?;
            Ok(())
        }

        fn be_u64_from_bytes(bytes: &[u8]) -> Option<u64> {
            if bytes.is_empty() {
                return Some(0);
            }
            if bytes.len() > 8 {
                return None;
            }
            let mut out = 0u64;
            for &b in bytes {
                out = out.checked_shl(8)?;
                out |= b as u64;
            }
            Some(out)
        }

        fn bsc_parse_and_verify_header(
            header_rlp: &[u8],
            validators: &BscValidatorsOf<T>,
            epoch_length: u64,
            chain_id: u64,
        ) -> Result<BscParsedHeader, DispatchError> {
            use crate::evm_proof::RlpItem;

            let hash = H256::from_slice(&keccak_256(header_rlp));
            let item =
                crate::evm_proof::rlp_decode(header_rlp).ok_or(Error::<T>::BscHeaderInvalid)?;
            let RlpItem::List(items) = item else {
                return Err(Error::<T>::BscHeaderInvalid.into());
            };
            ensure!(items.len() >= 15, Error::<T>::BscHeaderInvalid);
            // BSC tracks Ethereum header RLP with optional fields appended over time (EIP-1559/4895/4844/4788/7685).
            // For SCCP we only require: parentHash, stateRoot, number, difficulty, extraData (signature).
            // All other fields are used only as part of the Parlia seal hash.

            let mut fields: Vec<&[u8]> = Vec::with_capacity(items.len());
            for it in items.iter() {
                let RlpItem::Bytes(b) = it else {
                    return Err(Error::<T>::BscHeaderInvalid.into());
                };
                fields.push(b);
            }

            let parent_bytes = fields[0];
            ensure!(parent_bytes.len() == 32, Error::<T>::BscHeaderInvalid);
            let parent_hash = H256::from_slice(parent_bytes);

            let beneficiary_bytes = fields[2];
            ensure!(beneficiary_bytes.len() == 20, Error::<T>::BscHeaderInvalid);
            let beneficiary = H160::from_slice(beneficiary_bytes);

            let state_root_bytes = fields[3];
            ensure!(state_root_bytes.len() == 32, Error::<T>::BscHeaderInvalid);
            let state_root = H256::from_slice(state_root_bytes);

            let difficulty =
                Self::be_u64_from_bytes(fields[7]).ok_or(Error::<T>::BscHeaderInvalid)?;
            let number = Self::be_u64_from_bytes(fields[8]).ok_or(Error::<T>::BscHeaderInvalid)?;

            let extra_data = fields[12];
            ensure!(extra_data.len() >= 32 + 65, Error::<T>::BscHeaderInvalid);
            let sig_start = extra_data.len() - 65;
            let extra_no_sig = &extra_data[..sig_start];
            let sig_bytes = &extra_data[sig_start..];

            let is_epoch = number % epoch_length == 0;
            let mut epoch_validators: Vec<H160> = Vec::new();
            let mut epoch_turn_length: Option<u8> = None;

            // Vanity is always present, but after Luban forks non-epoch blocks can also carry
            // vote attestations (fast finality) before the seal signature. Do not enforce
            // any strict length beyond the vanity.
            ensure!(extra_no_sig.len() >= 32, Error::<T>::BscHeaderInvalid);
            if is_epoch {
                // Try Luban-era format first:
                // vanity(32) || num(1) || num*(addr(20)+bls_pubkey(48)) || ... || seal(65)
                if extra_no_sig.len() >= 33 {
                    const VALIDATOR_BYTES_LEN: usize = 20 + 48;
                    let num = extra_no_sig[32] as usize;
                    let start = 32usize + 1usize;
                    let end_luban = start.saturating_add(num.saturating_mul(VALIDATOR_BYTES_LEN));
                    let end_pre_luban = start.saturating_add(num.saturating_mul(20));

                    if num > 0 && end_luban <= extra_no_sig.len() {
                        // Luban+: validator entry includes vote-address BLS public key.
                        for i in 0..num {
                            let off = start + i * VALIDATOR_BYTES_LEN;
                            epoch_validators.push(H160::from_slice(&extra_no_sig[off..off + 20]));
                        }
                        // On Bohr-era BSC, epoch blocks also carry `turnLength` right after the validator bytes.
                        if let Some(t) = extra_no_sig.get(end_luban).copied() {
                            if t > 0 && t <= 64 {
                                epoch_turn_length = Some(t);
                            }
                        }
                    } else if num > 0 && end_pre_luban <= extra_no_sig.len() {
                        // Pre-Luban: vanity(32) || num(1) || num*addr(20) || ...
                        let val_bytes = &extra_no_sig[start..end_pre_luban];
                        for chunk in val_bytes.chunks(20) {
                            epoch_validators.push(H160::from_slice(chunk));
                        }
                    }
                }
            }
            epoch_validators.sort();
            epoch_validators.dedup();

            let mut sig = [0u8; 65];
            sig.copy_from_slice(sig_bytes);

            // Reject malleable / invalid ECDSA signatures.
            //
            // BSC consensus uses the header hash (which includes signature bytes) as the block id,
            // so accepting both low-`s` and high-`s` would allow multiple hashes for the same
            // signed content. Fail closed and match canonical EIP-2 style rules.
            let r_bytes = &sig[0..32];
            let s_bytes = &sig[32..64];
            ensure!(
                r_bytes.iter().any(|&b| b != 0),
                Error::<T>::BscHeaderInvalid
            );
            ensure!(
                s_bytes.iter().any(|&b| b != 0),
                Error::<T>::BscHeaderInvalid
            );
            // Big-endian compare: s <= n/2
            let mut s_ok = true;
            for i in 0..32 {
                if s_bytes[i] < SECP256K1N_HALF_ORDER[i] {
                    break;
                }
                if s_bytes[i] > SECP256K1N_HALF_ORDER[i] {
                    s_ok = false;
                    break;
                }
            }
            ensure!(s_ok, Error::<T>::BscHeaderInvalid);

            // Parlia seal hash: keccak256(rlp([chainId, header_fields..., extraData_without_sig, ...])).
            //
            // This mirrors `bnb-chain/bsc/core/types.EncodeSigHeader`.
            let chain_id_bytes = chain_id.to_be_bytes();
            let first = chain_id_bytes
                .iter()
                .position(|&b| b != 0)
                .unwrap_or(chain_id_bytes.len().saturating_sub(1));
            let chain_id_min = &chain_id_bytes[first..];

            let mut enc_items: Vec<Vec<u8>> = Vec::with_capacity(1 + fields.len());
            enc_items.push(crate::evm_proof::rlp_encode_bytes(chain_id_min));
            // Base Ethereum header fields (15 items).
            for i in 0..15 {
                let raw = if i == 12 { extra_no_sig } else { fields[i] };
                enc_items.push(crate::evm_proof::rlp_encode_bytes(raw));
            }
            // Optional header fields (EIP-1559/4895/4844/4788/7685 and future extensions).
            //
            // BSC's Parlia seal hash includes all header fields present, in order. Fail-closed
            // by hashing exactly what is in the signed header (except for stripping the seal
            // signature from `extraData`).
            for i in 15..fields.len() {
                enc_items.push(crate::evm_proof::rlp_encode_bytes(fields[i]));
            }
            let seal_rlp = crate::evm_proof::rlp_encode_list(&enc_items);
            let seal_hash = H256::from_slice(&keccak_256(&seal_rlp));

            let pk = sp_io::crypto::secp256k1_ecdsa_recover(&sig, &seal_hash.0)
                .map_err(|_| Error::<T>::BscHeaderInvalid)?;
            let addr = H160::from_slice(&keccak_256(&pk)[12..]);

            // Parlia requires the header beneficiary/coinbase equals the signer.
            ensure!(addr == beneficiary, Error::<T>::BscHeaderInvalid);
            ensure!(
                validators.iter().any(|v| *v == addr),
                Error::<T>::BscHeaderInvalid
            );

            Ok(BscParsedHeader {
                hash,
                parent_hash,
                number,
                state_root,
                difficulty,
                signer: addr,
                is_epoch,
                epoch_validators,
                epoch_turn_length,
            })
        }

        fn tron_parse_and_verify_header(
            raw_data: &[u8],
            witness_sig: &[u8; 65],
            witnesses: &BscValidatorsOf<T>,
            params: &TronLightClientParams,
        ) -> Result<TronParsedHeader, DispatchError> {
            let raw = crate::tron_proof::parse_tron_header_raw(raw_data)
                .ok_or(Error::<T>::TronHeaderInvalid)?;
            // SCCP requires an account state root for EVM MPT proofs.
            let state_root = raw.account_state_root;

            let raw_hash = crate::tron_proof::raw_data_hash(raw_data);
            let hash = crate::tron_proof::block_id_from_raw_hash(raw.number, &raw_hash);

            let signer = crate::tron_proof::recover_eth_address_from_sig(
                &raw_hash,
                witness_sig,
                &SECP256K1N_HALF_ORDER,
            )
            .ok_or(Error::<T>::TronHeaderInvalid)?;

            // Witness address binding (TRON uses `prefix || eth_address20` style).
            ensure!(
                raw.witness_address[0] == params.address_prefix,
                Error::<T>::TronHeaderInvalid
            );
            ensure!(
                &raw.witness_address[1..] == signer.as_bytes(),
                Error::<T>::TronHeaderInvalid
            );
            // Witness must be in the configured witness set.
            ensure!(
                witnesses.iter().any(|w| *w == signer),
                Error::<T>::TronHeaderInvalid
            );

            Ok(TronParsedHeader {
                hash,
                parent_hash: raw.parent_hash,
                number: raw.number,
                state_root,
                signer,
            })
        }

        fn verify_burn_proof(
            source_domain: u32,
            asset_id: &AssetIdOf<T>,
            payload: &BurnPayloadV1,
            message_id: H256,
            proof: &[u8],
        ) -> Result<bool, DispatchError> {
            // Inbound-to-SORA verification is mode- and source-chain-specific.
            match Self::inbound_finality_mode_for_domain(source_domain) {
                InboundFinalityMode::Disabled => Err(Error::<T>::InboundFinalityUnavailable.into()),
                InboundFinalityMode::LegacyEvmAnchor
                | InboundFinalityMode::LegacyBscLightClientOrAnchor
                | InboundFinalityMode::LegacyAttesterQuorum => {
                    Err(Error::<T>::InboundFinalityModeUnsupported.into())
                }
                InboundFinalityMode::EthBeaconLightClient => {
                    ensure!(
                        source_domain == SCCP_DOMAIN_ETH,
                        Error::<T>::InboundFinalityModeUnsupported
                    );
                    T::EthFinalizedBurnProofVerifier::verify_finalized_burn(
                        message_id, payload, proof,
                    )
                    .ok_or(Error::<T>::InboundFinalityUnavailable.into())
                }
                InboundFinalityMode::EthZkProof => {
                    ensure!(
                        source_domain == SCCP_DOMAIN_ETH,
                        Error::<T>::InboundFinalityModeUnsupported
                    );
                    T::EthZkFinalizedBurnProofVerifier::verify_finalized_burn(message_id, proof)
                        .ok_or(Error::<T>::InboundFinalityUnavailable.into())
                }
                InboundFinalityMode::BscLightClient => {
                    Self::verify_evm_burn_proof(source_domain, message_id, proof)
                }
                InboundFinalityMode::TronLightClient => {
                    ensure!(
                        source_domain == SCCP_DOMAIN_TRON,
                        Error::<T>::InboundFinalityModeUnsupported
                    );
                    let f =
                        TronFinalized::<T>::get().ok_or(Error::<T>::InboundFinalityUnavailable)?;
                    Self::verify_evm_burn_proof_at_root(
                        source_domain,
                        message_id,
                        proof,
                        f.hash,
                        f.state_root,
                    )
                }
                InboundFinalityMode::SolanaLightClient => {
                    ensure!(
                        source_domain == SCCP_DOMAIN_SOL,
                        Error::<T>::InboundFinalityModeUnsupported
                    );
                    T::SolanaFinalizedBurnProofVerifier::verify_finalized_burn(message_id, proof)
                        .ok_or(Error::<T>::InboundFinalityUnavailable.into())
                }
                InboundFinalityMode::TonLightClient => {
                    ensure!(
                        source_domain == SCCP_DOMAIN_TON,
                        Error::<T>::InboundFinalityModeUnsupported
                    );
                    Self::verify_ton_burn_proof(asset_id, payload, message_id, proof)
                }
                InboundFinalityMode::SubstrateLightClient => {
                    ensure!(
                        matches!(
                            source_domain,
                            SCCP_DOMAIN_SORA_KUSAMA | SCCP_DOMAIN_SORA_POLKADOT
                        ),
                        Error::<T>::InboundFinalityModeUnsupported
                    );
                    T::SubstrateFinalizedBurnProofVerifier::verify_finalized_burn(
                        source_domain,
                        message_id,
                        proof,
                    )
                    .ok_or(Error::<T>::InboundFinalityUnavailable.into())
                }
            }
        }

        fn verify_ton_burn_proof(
            asset_id: &AssetIdOf<T>,
            payload: &BurnPayloadV1,
            message_id: H256,
            proof: &[u8],
        ) -> Result<bool, DispatchError> {
            let checkpoint = TonTrustedCheckpointState::<T>::get()
                .ok_or(Error::<T>::InboundFinalityUnavailable)?;

            let mut input = proof;
            let decoded = match TonBurnProofV1::decode(&mut input) {
                Ok(decoded) => decoded,
                Err(_) => return Ok(false),
            };
            if !input.is_empty() {
                return Ok(false);
            }

            if decoded.version != 1 {
                return Ok(false);
            }
            if decoded.trusted_checkpoint_seqno != checkpoint.mc_seqno
                || decoded.trusted_checkpoint_hash != checkpoint.mc_block_hash
            {
                return Ok(false);
            }
            if decoded.target_mc_seqno < decoded.trusted_checkpoint_seqno {
                return Ok(false);
            }
            if decoded.target_mc_seqno == decoded.trusted_checkpoint_seqno
                && decoded.target_mc_block_hash != decoded.trusted_checkpoint_hash
            {
                return Ok(false);
            }
            if decoded.burn_message_id != message_id {
                return Ok(false);
            }
            if decoded.burn_record.dest_domain != SCCP_DOMAIN_SORA
                || decoded.burn_record.dest_domain != payload.dest_domain
            {
                return Ok(false);
            }
            if decoded.burn_record.recipient32 != payload.recipient
                || decoded.burn_record.jetton_amount != payload.amount
                || decoded.burn_record.nonce != payload.nonce
            {
                return Ok(false);
            }

            let expected_master = match RemoteToken::<T>::get(asset_id, SCCP_DOMAIN_TON) {
                Some(remote) if remote.len() == 32 => {
                    let mut out = [0u8; 32];
                    out.copy_from_slice(remote.as_slice());
                    out
                }
                _ => return Ok(false),
            };
            let expected_code_hash = match DomainEndpoint::<T>::get(SCCP_DOMAIN_TON) {
                Some(endpoint) if endpoint.len() == 32 => H256::from_slice(endpoint.as_slice()),
                _ => return Ok(false),
            };

            if decoded.jetton_master_account_id != expected_master
                || decoded.jetton_master_code_hash != expected_code_hash
            {
                return Ok(false);
            }

            let section_lens = [
                decoded.masterchain_proof.len(),
                decoded.shard_proof.len(),
                decoded.account_proof.len(),
                decoded.burns_dict_proof.len(),
            ];
            if section_lens.iter().any(|len| *len == 0) {
                return Ok(false);
            }
            if section_lens
                .iter()
                .any(|len| *len > SCCP_MAX_TON_PROOF_SECTION_BYTES)
            {
                return Err(Error::<T>::TonProofTooLarge.into());
            }
            let total_len = section_lens.iter().copied().sum::<usize>();
            if total_len > SCCP_MAX_TON_PROOF_TOTAL_BYTES {
                return Err(Error::<T>::TonProofTooLarge.into());
            }

            Ok(
                ton_proof_runtime_interface::ton_proof_api::verify_ton_burn_proof(
                    ton_proof_runtime_interface::TonVerifyRequest {
                        trusted_checkpoint_seqno: checkpoint.mc_seqno,
                        trusted_checkpoint_hash: checkpoint.mc_block_hash.0,
                        proof: proof.to_vec(),
                        expected_master_account_id: expected_master,
                        expected_code_hash: expected_code_hash.0,
                        expected_message_id: message_id.0,
                        expected_dest_domain: payload.dest_domain,
                        expected_recipient32: payload.recipient,
                        expected_amount: payload.amount,
                        expected_nonce: payload.nonce,
                    },
                ),
            )
        }

        fn verify_evm_burn_proof(
            source_domain: u32,
            message_id: H256,
            proof: &[u8],
        ) -> Result<bool, DispatchError> {
            let (expected_block_hash, state_root) = match source_domain {
                SCCP_DOMAIN_BSC => {
                    let finalized =
                        BscFinalized::<T>::get().ok_or(Error::<T>::InboundFinalityUnavailable)?;
                    (finalized.hash, finalized.state_root)
                }
                _ => return Ok(false),
            };

            Self::verify_evm_burn_proof_at_root(
                source_domain,
                message_id,
                proof,
                expected_block_hash,
                state_root,
            )
        }

        fn verify_evm_burn_proof_at_root(
            source_domain: u32,
            message_id: H256,
            proof: &[u8],
            expected_block_hash: H256,
            state_root: H256,
        ) -> Result<bool, DispatchError> {
            let mut input = proof;
            let p = EvmBurnProofV1::decode(&mut input)
                .map_err(|_| Error::<T>::ProofVerificationFailed)?;
            // Fail closed: forbid trailing garbage.
            if !input.is_empty() {
                return Ok(false);
            }
            if p.anchor_block_hash != expected_block_hash {
                return Ok(false);
            }

            // Basic DoS bounds.
            if p.account_proof.len() > SCCP_MAX_EVM_PROOF_NODES
                || p.storage_proof.len() > SCCP_MAX_EVM_PROOF_NODES
            {
                return Ok(false);
            }
            let mut total: usize = 0;
            for n in p.account_proof.iter().chain(p.storage_proof.iter()) {
                if n.len() > SCCP_MAX_EVM_PROOF_NODE_BYTES {
                    return Ok(false);
                }
                total = total.saturating_add(n.len());
                if total > SCCP_MAX_EVM_PROOF_TOTAL_BYTES {
                    return Ok(false);
                }
            }

            // Router (contract) address is configured per source domain.
            let endpoint =
                DomainEndpoint::<T>::get(source_domain).ok_or(Error::<T>::DomainEndpointMissing)?;
            Self::ensure_domain_endpoint_len(source_domain, endpoint.len())?;
            let mut router_addr = [0u8; 20];
            router_addr.copy_from_slice(endpoint.as_slice());

            // Account trie key: keccak256(address_bytes20).
            let account_key = keccak_256(&router_addr);

            // Storage trie key for burns[messageId].sender:
            // slot_base = keccak256(messageId || u256(mapping_slot))
            // storage_key = keccak256(slot_base)
            let storage_key = evm_burn_storage_key_for_message_id(message_id);

            let account_val_rlp =
                crate::evm_proof::mpt_get(state_root, &account_key, &p.account_proof)
                    .ok_or(Error::<T>::ProofVerificationFailed)?;
            let storage_root = crate::evm_proof::evm_account_storage_root(&account_val_rlp)
                .ok_or(Error::<T>::ProofVerificationFailed)?;
            let storage_val_rlp =
                crate::evm_proof::mpt_get(storage_root, &storage_key.0, &p.storage_proof)
                    .ok_or(Error::<T>::ProofVerificationFailed)?;

            let payload =
                crate::evm_proof::rlp_decode_bytes_payload(&storage_val_rlp).unwrap_or(&[]);
            // Non-zero means the burn record exists.
            Ok(payload.iter().any(|&b| b != 0))
        }

        fn ensure_asset_is_mintable(asset_id: &AssetIdOf<T>) -> DispatchResult {
            let (_symbol, _name, _precision, is_mintable, ..) =
                <T as Config>::AssetInfoProvider::get_asset_info(asset_id);
            ensure!(is_mintable, Error::<T>::AssetSupplyNotMintable);
            Ok(())
        }

        fn default_inbound_finality_mode(domain_id: u32) -> InboundFinalityMode {
            match domain_id {
                // Security-first defaults: trustless chain-specific finality modes.
                // If the corresponding on-chain verifier is not initialized, SCCP fails closed.
                SCCP_DOMAIN_ETH => InboundFinalityMode::EthBeaconLightClient,
                SCCP_DOMAIN_BSC => InboundFinalityMode::BscLightClient,
                SCCP_DOMAIN_SOL => InboundFinalityMode::SolanaLightClient,
                SCCP_DOMAIN_TON => InboundFinalityMode::TonLightClient,
                SCCP_DOMAIN_TRON => InboundFinalityMode::TronLightClient,
                SCCP_DOMAIN_SORA_KUSAMA | SCCP_DOMAIN_SORA_POLKADOT => {
                    InboundFinalityMode::SubstrateLightClient
                }
                _ => InboundFinalityMode::Disabled,
            }
        }

        fn inbound_finality_mode_for_domain(domain_id: u32) -> InboundFinalityMode {
            InboundFinalityModes::<T>::get(domain_id)
                .unwrap_or_else(|| Self::default_inbound_finality_mode(domain_id))
        }

        fn ensure_inbound_finality_mode_supported(
            domain_id: u32,
            mode: InboundFinalityMode,
        ) -> DispatchResult {
            let supported = match domain_id {
                SCCP_DOMAIN_ETH => matches!(
                    mode,
                    InboundFinalityMode::Disabled
                        | InboundFinalityMode::EthBeaconLightClient
                        | InboundFinalityMode::EthZkProof
                ),
                SCCP_DOMAIN_BSC => matches!(
                    mode,
                    InboundFinalityMode::Disabled | InboundFinalityMode::BscLightClient
                ),
                SCCP_DOMAIN_SOL => {
                    matches!(
                        mode,
                        InboundFinalityMode::Disabled | InboundFinalityMode::SolanaLightClient
                    )
                }
                SCCP_DOMAIN_TON => {
                    matches!(
                        mode,
                        InboundFinalityMode::Disabled | InboundFinalityMode::TonLightClient
                    )
                }
                SCCP_DOMAIN_TRON => matches!(
                    mode,
                    InboundFinalityMode::Disabled | InboundFinalityMode::TronLightClient
                ),
                SCCP_DOMAIN_SORA_KUSAMA | SCCP_DOMAIN_SORA_POLKADOT => matches!(
                    mode,
                    InboundFinalityMode::Disabled | InboundFinalityMode::SubstrateLightClient
                ),
                _ => false,
            };
            ensure!(supported, Error::<T>::InboundFinalityModeUnsupported);
            Ok(())
        }

        fn ensure_inbound_finality_available(source_domain: u32) -> DispatchResult {
            match Self::inbound_finality_mode_for_domain(source_domain) {
                InboundFinalityMode::Disabled => Err(Error::<T>::InboundFinalityUnavailable.into()),
                InboundFinalityMode::LegacyEvmAnchor
                | InboundFinalityMode::LegacyBscLightClientOrAnchor
                | InboundFinalityMode::LegacyAttesterQuorum => {
                    Err(Error::<T>::InboundFinalityModeUnsupported.into())
                }
                InboundFinalityMode::BscLightClient => {
                    ensure!(
                        source_domain == SCCP_DOMAIN_BSC,
                        Error::<T>::InboundFinalityModeUnsupported
                    );
                    ensure!(
                        BscFinalized::<T>::get().is_some(),
                        Error::<T>::InboundFinalityUnavailable
                    );
                    Ok(())
                }
                InboundFinalityMode::TronLightClient => {
                    ensure!(
                        source_domain == SCCP_DOMAIN_TRON,
                        Error::<T>::InboundFinalityModeUnsupported
                    );
                    ensure!(
                        TronFinalized::<T>::get().is_some(),
                        Error::<T>::InboundFinalityUnavailable
                    );
                    Ok(())
                }
                InboundFinalityMode::EthBeaconLightClient => {
                    ensure!(
                        source_domain == SCCP_DOMAIN_ETH,
                        Error::<T>::InboundFinalityModeUnsupported
                    );
                    ensure!(
                        T::EthFinalizedBurnProofVerifier::is_available(),
                        Error::<T>::InboundFinalityUnavailable
                    );
                    Ok(())
                }
                InboundFinalityMode::EthZkProof => {
                    ensure!(
                        source_domain == SCCP_DOMAIN_ETH,
                        Error::<T>::InboundFinalityModeUnsupported
                    );
                    ensure!(
                        T::EthZkFinalizedBurnProofVerifier::is_available(),
                        Error::<T>::InboundFinalityUnavailable
                    );
                    Ok(())
                }
                InboundFinalityMode::SolanaLightClient => {
                    ensure!(
                        source_domain == SCCP_DOMAIN_SOL,
                        Error::<T>::InboundFinalityModeUnsupported
                    );
                    ensure!(
                        T::SolanaFinalizedBurnProofVerifier::is_available(),
                        Error::<T>::InboundFinalityUnavailable
                    );
                    Ok(())
                }
                InboundFinalityMode::TonLightClient => {
                    ensure!(
                        source_domain == SCCP_DOMAIN_TON,
                        Error::<T>::InboundFinalityModeUnsupported
                    );
                    ensure!(
                        TonTrustedCheckpointState::<T>::get().is_some(),
                        Error::<T>::InboundFinalityUnavailable
                    );
                    Ok(())
                }
                InboundFinalityMode::SubstrateLightClient => {
                    ensure!(
                        matches!(
                            source_domain,
                            SCCP_DOMAIN_SORA_KUSAMA | SCCP_DOMAIN_SORA_POLKADOT
                        ),
                        Error::<T>::InboundFinalityModeUnsupported
                    );
                    ensure!(
                        T::SubstrateFinalizedBurnProofVerifier::is_available(source_domain),
                        Error::<T>::InboundFinalityUnavailable
                    );
                    Ok(())
                }
            }
        }
    }
}

impl<T: pallet::Config> SccpAssetChecker<common::AssetIdOf<T>> for pallet::Pallet<T> {
    fn is_sccp_asset(asset_id: &common::AssetIdOf<T>) -> bool {
        pallet::Pallet::<T>::is_sccp_asset(asset_id)
    }
}
