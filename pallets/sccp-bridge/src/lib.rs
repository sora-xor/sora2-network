// This file is part of the SORA network and Polkaswap app.

// Copyright (c) 2020, 2021, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

// Redistribution and use in source and binary forms, with or without modification,
// are permitted provided that the following conditions are met:

// Redistributions of source code must retain the above copyright notice, this list
// of conditions and the following disclaimer.
// Redistributions in binary form must reproduce the above copyright notice, this
// list of conditions and the following disclaimer in the documentation and/or other
// materials provided with the distribution.
//
// All advertising materials mentioning features or use of this software must display
// the following acknowledgement: This product includes software developed by Polka Biome
// Ltd., SORA, and Polkaswap.
//
// Neither the name of the Polka Biome Ltd. nor the names of its contributors may be used
// to endorse or promote products derived from this software without specific prior written permission.

// THIS SOFTWARE IS PROVIDED BY Polka Biome Ltd. AS IS AND ANY EXPRESS OR IMPLIED WARRANTIES,
// INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL Polka Biome Ltd. BE LIABLE FOR ANY
// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
// BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS;
// OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
// USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::all)]

//! SCCP bridge registry and proof-ingestion pallet.
//!
//! SCCP relay on SORA2 is intentionally human-operated for the runtime proof
//! path. A relay operator uses a bridge web interface to inspect Nexus/Iroha
//! bundle metadata, fetch the `runtime-scale-v1` envelope, and submit the
//! matching signed SORA2 extrinsic through a wallet. The signed origin is only
//! the courier and fee payer. It is not trusted for authorization.
//!
//! Authorization comes from the verified payload itself: Nexus finality anchors,
//! Sora Parliament roster anchors, message ids, payload hashes, commitment
//! roots, and the runtime envelope checks performed by this pallet. There is no
//! off-chain worker or daemon requirement in this pallet.

use base58::FromBase58;
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame_support::{dispatch::DispatchResult, pallet_prelude::*, traits::StorageVersion};
use frame_system::{ensure_signed, pallet_prelude::*};
use scale_info::TypeInfo;
use sp_io::hashing::{blake2_256, keccak_256, sha2_256};
use sp_runtime::{DispatchError, RuntimeDebug};
use sp_std::vec::Vec;

pub use pallet::*;
pub use weights::WeightInfo;

pub mod weights;

pub type DomainId = u32;
pub type MessageId = [u8; 32];
pub type CommitmentRoot = [u8; 32];
pub type Nonce = u64;

pub const SCCP_DOMAIN_SORA: DomainId = 0;
pub const SCCP_DOMAIN_ETH: DomainId = 1;
pub const SCCP_DOMAIN_BSC: DomainId = 2;
pub const SCCP_DOMAIN_SOL: DomainId = 3;
pub const SCCP_DOMAIN_TON: DomainId = 4;
pub const SCCP_DOMAIN_TRON: DomainId = 5;
pub const SCCP_DOMAIN_SORA_KUSAMA: DomainId = 6;
pub const SCCP_DOMAIN_SORA_POLKADOT: DomainId = 7;
pub const SCCP_DOMAIN_SORA2: DomainId = 8;

pub const SCCP_CODEC_TEXT_UTF8: u8 = 1;
pub const SCCP_CODEC_EVM_HEX: u8 = 2;
pub const SCCP_CODEC_SOLANA_BASE58: u8 = 3;
pub const SCCP_CODEC_TON_RAW: u8 = 4;
pub const SCCP_CODEC_TRON_BASE58CHECK: u8 = 5;
pub const SCCP_CODEC_SORA_ASSET_ID: u8 = 6;

pub const SCCP_RUNTIME_PROOF_FAMILY_V1: &[u8] = b"runtime-scale-v1";
pub const SCCP_RUNTIME_VERIFIER_BACKEND_V1: &[u8] = b"sora-nexus-runtime-v1";

const SCCP_MSG_PREFIX_TOKEN_ADD_V1: &[u8] = b"sccp:token:add:v1";
const SCCP_MSG_PREFIX_TOKEN_PAUSE_V1: &[u8] = b"sccp:token:pause:v1";
const SCCP_MSG_PREFIX_TOKEN_RESUME_V1: &[u8] = b"sccp:token:resume:v1";
const SCCP_MSG_PREFIX_ASSET_REGISTER_V1: &[u8] = b"sccp:asset:register:v1";
const SCCP_MSG_PREFIX_ROUTE_ACTIVATE_V1: &[u8] = b"sccp:route:activate:v1";
const SCCP_MSG_PREFIX_TRANSFER_V1: &[u8] = b"sccp:transfer:v1";
const SCCP_HUB_LEAF_PREFIX_V1: &[u8] = b"sccp:hub:leaf:v1";
const SCCP_HUB_NODE_PREFIX_V1: &[u8] = b"sccp:hub:node:v1";
const SCCP_PAYLOAD_HASH_PREFIX_V1: &[u8] = b"sccp:payload:v1";

#[derive(
    Clone,
    Encode,
    Decode,
    DecodeWithMemTracking,
    Eq,
    PartialEq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
)]
pub struct EncodedPayload<Payload> {
    pub codec: u8,
    pub bytes: Payload,
}

#[derive(
    Clone,
    Encode,
    Decode,
    DecodeWithMemTracking,
    Eq,
    PartialEq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
)]
pub struct RegistryAssetRecord<Payload> {
    pub home_domain: DomainId,
    pub decimals: u8,
    pub asset_id: EncodedPayload<Payload>,
    pub enabled: bool,
    pub sora_asset_id: Option<MessageId>,
    pub name: Option<MessageId>,
    pub symbol: Option<MessageId>,
}

#[derive(
    Clone,
    Encode,
    Decode,
    DecodeWithMemTracking,
    Eq,
    PartialEq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
)]
pub struct RouteRecord<Payload> {
    pub asset_id: EncodedPayload<Payload>,
    pub remote_domain: DomainId,
    pub enabled: bool,
}

#[derive(
    Clone,
    Encode,
    Decode,
    DecodeWithMemTracking,
    Eq,
    PartialEq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
)]
pub struct OutboundMessageRecord<Payload> {
    pub route_id: EncodedPayload<Payload>,
    pub source_domain: DomainId,
    pub dest_domain: DomainId,
    pub asset_home_domain: DomainId,
    pub amount: u128,
    pub sender: EncodedPayload<Payload>,
    pub recipient: EncodedPayload<Payload>,
}

#[derive(
    Clone,
    Encode,
    Decode,
    DecodeWithMemTracking,
    Eq,
    PartialEq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
)]
pub struct MessageProofReceipt<Payload> {
    pub proof_family: Payload,
    pub verifier_backend: Payload,
    pub route_id: EncodedPayload<Payload>,
    pub source_domain: DomainId,
    pub commitment_root: CommitmentRoot,
}

#[derive(
    Clone,
    Copy,
    Encode,
    Decode,
    DecodeWithMemTracking,
    Eq,
    PartialEq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
)]
pub enum ControlMessageKind {
    AssetRegister,
    RouteActivate,
    TokenAdd,
    TokenPause,
    TokenResume,
}

#[derive(
    Clone,
    Encode,
    Decode,
    DecodeWithMemTracking,
    Eq,
    PartialEq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
)]
pub struct ControlProofReceipt<Payload> {
    pub proof_family: Payload,
    pub verifier_backend: Payload,
    pub message_kind: ControlMessageKind,
    pub target_domain: DomainId,
    pub commitment_root: CommitmentRoot,
}

#[derive(
    Clone,
    Encode,
    Decode,
    DecodeWithMemTracking,
    Eq,
    PartialEq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
)]
pub struct InboundMessageKey<Payload> {
    pub source_domain: DomainId,
    pub route_id: EncodedPayload<Payload>,
    pub message_id: MessageId,
}

#[derive(Clone, Copy, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub enum RuntimeProofKind {
    Burn,
    TokenAdd,
    TokenPause,
    TokenResume,
    AssetRegister,
    RouteActivate,
    Transfer,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct RuntimeHubCommitment {
    pub version: u8,
    pub kind: RuntimeProofKind,
    pub target_domain: DomainId,
    pub message_id: MessageId,
    pub payload_hash: MessageId,
    pub parliament_certificate_hash: Option<MessageId>,
}

#[derive(Clone, Copy, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct RuntimeMerkleStep {
    pub sibling_hash: MessageId,
    pub sibling_is_left: bool,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct RuntimeMerkleProof {
    pub steps: Vec<RuntimeMerkleStep>,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub enum RuntimeProofPayload<Payload> {
    AssetRegister {
        target_domain: DomainId,
        home_domain: DomainId,
        nonce: Nonce,
        asset_id: EncodedPayload<Payload>,
        decimals: u8,
    },
    RouteActivate {
        source_domain: DomainId,
        target_domain: DomainId,
        nonce: Nonce,
        asset_id: EncodedPayload<Payload>,
        route_id: EncodedPayload<Payload>,
    },
    Transfer {
        source_domain: DomainId,
        dest_domain: DomainId,
        nonce: Nonce,
        asset_home_domain: DomainId,
        asset_id: EncodedPayload<Payload>,
        amount: u128,
        sender: EncodedPayload<Payload>,
        recipient: EncodedPayload<Payload>,
        route_id: EncodedPayload<Payload>,
    },
    TokenAdd {
        target_domain: DomainId,
        nonce: Nonce,
        sora_asset_id: MessageId,
        decimals: u8,
        name: MessageId,
        symbol: MessageId,
    },
    TokenPause {
        target_domain: DomainId,
        nonce: Nonce,
        sora_asset_id: MessageId,
    },
    TokenResume {
        target_domain: DomainId,
        nonce: Nonce,
        sora_asset_id: MessageId,
    },
}

#[derive(Clone, Copy, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct RuntimeFinalityProof {
    pub version: u8,
    pub epoch: u64,
    pub height: u64,
    pub block_hash: MessageId,
    pub commitment_root: CommitmentRoot,
    pub validator_set_hash: MessageId,
    pub signature_count: u16,
}

#[derive(Clone, Copy, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct RuntimeParliamentCertificate {
    pub version: u8,
    pub preimage_hash: MessageId,
    pub enactment_window_start: u64,
    pub enactment_window_end: u64,
    pub roster_epoch: u64,
    pub roster_hash: MessageId,
    pub required_signatures: u16,
    pub signature_count: u16,
    pub certificate_hash: MessageId,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct SccpRuntimeProofEnvelope<Payload> {
    pub version: u8,
    pub commitment_root: CommitmentRoot,
    pub commitment: RuntimeHubCommitment,
    pub merkle_proof: RuntimeMerkleProof,
    pub payload: RuntimeProofPayload<Payload>,
    pub finality_proof: RuntimeFinalityProof,
    pub parliament_certificate: Option<RuntimeParliamentCertificate>,
}

#[derive(
    Clone,
    Copy,
    Encode,
    Decode,
    DecodeWithMemTracking,
    Eq,
    PartialEq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
)]
pub struct NexusFinalityAnchor {
    pub validator_set_hash: MessageId,
    pub min_signatures: u16,
}

#[derive(
    Clone,
    Copy,
    Encode,
    Decode,
    DecodeWithMemTracking,
    Eq,
    PartialEq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
)]
pub struct ParliamentRosterAnchor {
    pub roster_hash: MessageId,
    pub required_signatures: u16,
}

#[derive(Clone, Copy, Eq, PartialEq, RuntimeDebug)]
pub enum MessageProofVerificationError {
    Unavailable,
    Invalid,
}

pub trait MessageProofVerifier {
    fn verify_message_proof(
        proof_family: &[u8],
        verifier_backend: &[u8],
        proof_bytes: &[u8],
        public_inputs: &[u8],
        bundle_bytes: &[u8],
        message_id: &MessageId,
        route_id_codec: u8,
        route_id: &[u8],
        source_domain: DomainId,
        commitment_root: &CommitmentRoot,
    ) -> Result<(), MessageProofVerificationError>;
}

impl MessageProofVerifier for () {
    fn verify_message_proof(
        _proof_family: &[u8],
        _verifier_backend: &[u8],
        _proof_bytes: &[u8],
        _public_inputs: &[u8],
        _bundle_bytes: &[u8],
        _message_id: &MessageId,
        _route_id_codec: u8,
        _route_id: &[u8],
        _source_domain: DomainId,
        _commitment_root: &CommitmentRoot,
    ) -> Result<(), MessageProofVerificationError> {
        Err(MessageProofVerificationError::Unavailable)
    }
}

pub trait ControlMessageProofVerifier {
    fn verify_control_message_proof(
        proof_family: &[u8],
        verifier_backend: &[u8],
        proof_bytes: &[u8],
        public_inputs: &[u8],
        bundle_bytes: &[u8],
        message_id: &MessageId,
        message_kind: ControlMessageKind,
        target_domain: DomainId,
        commitment_root: &CommitmentRoot,
    ) -> Result<(), MessageProofVerificationError>;
}

impl ControlMessageProofVerifier for () {
    fn verify_control_message_proof(
        _proof_family: &[u8],
        _verifier_backend: &[u8],
        _proof_bytes: &[u8],
        _public_inputs: &[u8],
        _bundle_bytes: &[u8],
        _message_id: &MessageId,
        _message_kind: ControlMessageKind,
        _target_domain: DomainId,
        _commitment_root: &CommitmentRoot,
    ) -> Result<(), MessageProofVerificationError> {
        Err(MessageProofVerificationError::Unavailable)
    }
}

pub fn is_supported_domain(domain_id: DomainId) -> bool {
    matches!(
        domain_id,
        SCCP_DOMAIN_SORA
            | SCCP_DOMAIN_ETH
            | SCCP_DOMAIN_BSC
            | SCCP_DOMAIN_SOL
            | SCCP_DOMAIN_TON
            | SCCP_DOMAIN_TRON
            | SCCP_DOMAIN_SORA_KUSAMA
            | SCCP_DOMAIN_SORA_POLKADOT
            | SCCP_DOMAIN_SORA2
    )
}

pub fn is_supported_codec(codec_id: u8) -> bool {
    matches!(
        codec_id,
        SCCP_CODEC_TEXT_UTF8
            | SCCP_CODEC_EVM_HEX
            | SCCP_CODEC_SOLANA_BASE58
            | SCCP_CODEC_TON_RAW
            | SCCP_CODEC_TRON_BASE58CHECK
            | SCCP_CODEC_SORA_ASSET_ID
    )
}

fn decode_hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn decode_hex_into(input: &[u8], output: &mut [u8]) -> bool {
    if input.len() != output.len().saturating_mul(2) {
        return false;
    }

    for (index, chunk) in input.chunks_exact(2).enumerate() {
        let Some(high) = decode_hex_nibble(chunk[0]) else {
            return false;
        };
        let Some(low) = decode_hex_nibble(chunk[1]) else {
            return false;
        };
        output[index] = (high << 4) | low;
    }

    true
}

fn validate_utf8_codec(bytes: &[u8]) -> bool {
    !bytes.is_empty() && core::str::from_utf8(bytes).is_ok()
}

fn validate_evm_hex_codec(bytes: &[u8]) -> bool {
    if bytes.len() != 42 || bytes[..2] != *b"0x" {
        return false;
    }

    let payload = &bytes[2..];
    let mut decoded = [0u8; 20];
    if !decode_hex_into(payload, &mut decoded) {
        return false;
    }

    let lowercase_payload = payload
        .iter()
        .map(|byte| byte.to_ascii_lowercase())
        .collect::<Vec<_>>();
    let checksum = keccak_256(&lowercase_payload);

    for (index, byte) in payload.iter().copied().enumerate() {
        if byte.is_ascii_digit() {
            continue;
        }

        let checksum_nibble = if index % 2 == 0 {
            checksum[index / 2] >> 4
        } else {
            checksum[index / 2] & 0x0f
        };
        let should_be_uppercase = checksum_nibble >= 8;

        if should_be_uppercase {
            if !byte.is_ascii_uppercase() {
                return false;
            }
        } else if !byte.is_ascii_lowercase() {
            return false;
        }
    }

    true
}

fn validate_canonical_i32_decimal(value: &str) -> bool {
    if value.is_empty() || value.starts_with('+') {
        return false;
    }

    let digits = if let Some(digits) = value.strip_prefix('-') {
        if digits.is_empty() || digits == "0" {
            return false;
        }
        digits
    } else {
        value
    };

    digits
        .as_bytes()
        .iter()
        .copied()
        .all(|byte| byte.is_ascii_digit())
        && (digits.len() == 1 || digits.as_bytes()[0] != b'0')
        && value.parse::<i32>().is_ok()
}

fn validate_ton_raw_codec(bytes: &[u8]) -> bool {
    let Ok(value) = core::str::from_utf8(bytes) else {
        return false;
    };
    let Some((workchain, account)) = value.split_once(':') else {
        return false;
    };
    validate_canonical_i32_decimal(workchain)
        && account.len() == 64
        && account
            .as_bytes()
            .iter()
            .copied()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn decode_base58(bytes: &[u8]) -> Option<Vec<u8>> {
    core::str::from_utf8(bytes).ok()?.from_base58().ok()
}

fn validate_solana_base58_codec(bytes: &[u8]) -> bool {
    decode_base58(bytes)
        .map(|decoded| decoded.len() == 32)
        .unwrap_or(false)
}

fn validate_tron_base58_codec(bytes: &[u8]) -> bool {
    let Some(decoded) = decode_base58(bytes) else {
        return false;
    };
    if decoded.len() != 25 || decoded[0] != 0x41 {
        return false;
    }

    let payload_len = decoded.len() - 4;
    let checksum = sha2_256(&sha2_256(&decoded[..payload_len]));
    decoded[payload_len..] == checksum[..4]
}

fn validate_sora_asset_id_codec(bytes: &[u8]) -> bool {
    bytes.len() == 32
}

fn validate_payload_codec(codec_id: u8, bytes: &[u8]) -> bool {
    match codec_id {
        SCCP_CODEC_TEXT_UTF8 => validate_utf8_codec(bytes),
        SCCP_CODEC_EVM_HEX => validate_evm_hex_codec(bytes),
        SCCP_CODEC_SOLANA_BASE58 => validate_solana_base58_codec(bytes),
        SCCP_CODEC_TON_RAW => validate_ton_raw_codec(bytes),
        SCCP_CODEC_TRON_BASE58CHECK => validate_tron_base58_codec(bytes),
        SCCP_CODEC_SORA_ASSET_ID => validate_sora_asset_id_codec(bytes),
        _ => false,
    }
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_system::ensure_root;

    pub type PayloadOf<T> = BoundedVec<u8, <T as Config>::MaxPayloadLen>;
    type ProofBlobOf<T> = BoundedVec<u8, <T as Config>::MaxProofBlobLen>;
    pub type EncodedPayloadOf<T> = EncodedPayload<PayloadOf<T>>;
    type RegistryAssetRecordOf<T> = RegistryAssetRecord<PayloadOf<T>>;
    type RouteRecordOf<T> = RouteRecord<PayloadOf<T>>;
    type OutboundMessageRecordOf<T> = OutboundMessageRecord<PayloadOf<T>>;
    type MessageProofReceiptOf<T> = MessageProofReceipt<PayloadOf<T>>;
    type ControlProofReceiptOf<T> = ControlProofReceipt<PayloadOf<T>>;
    type InboundMessageKeyOf<T> = InboundMessageKey<PayloadOf<T>>;
    type RuntimeProofPayloadOf<T> = RuntimeProofPayload<PayloadOf<T>>;
    type SccpRuntimeProofEnvelopeOf<T> = SccpRuntimeProofEnvelope<PayloadOf<T>>;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        #[allow(deprecated)]
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        #[pallet::constant]
        type MaxPayloadLen: Get<u32>;

        #[pallet::constant]
        type MaxProofBlobLen: Get<u32>;

        #[pallet::constant]
        type AllowManualInboundFinalization: Get<bool>;

        #[pallet::constant]
        type LocalDomain: Get<DomainId>;

        type WeightInfo: WeightInfo;
        type MessageProofVerifier: MessageProofVerifier;
        type ControlMessageProofVerifier: ControlMessageProofVerifier;
    }

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(5);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        RegistryAssetImported {
            asset_id: EncodedPayloadOf<T>,
            home_domain: DomainId,
            decimals: u8,
        },
        RouteActivated {
            route_id: EncodedPayloadOf<T>,
            asset_id: EncodedPayloadOf<T>,
            remote_domain: DomainId,
        },
        RoutePaused {
            route_id: EncodedPayloadOf<T>,
        },
        RouteResumed {
            route_id: EncodedPayloadOf<T>,
        },
        OutboundRecorded {
            nonce: Nonce,
            route_id: EncodedPayloadOf<T>,
            dest_domain: DomainId,
        },
        InboundFinalized {
            message_id: MessageId,
            route_id: EncodedPayloadOf<T>,
            source_domain: DomainId,
            commitment_root: CommitmentRoot,
        },
        MessageProofAccepted {
            message_id: MessageId,
            proof_family: PayloadOf<T>,
            verifier_backend: PayloadOf<T>,
            route_id: EncodedPayloadOf<T>,
            source_domain: DomainId,
            commitment_root: CommitmentRoot,
        },
        ControlMessageProofAccepted {
            message_id: MessageId,
            proof_family: PayloadOf<T>,
            verifier_backend: PayloadOf<T>,
            message_kind: ControlMessageKind,
            target_domain: DomainId,
            commitment_root: CommitmentRoot,
        },
        RegistryAssetPaused {
            asset_id: EncodedPayloadOf<T>,
        },
        RegistryAssetResumed {
            asset_id: EncodedPayloadOf<T>,
        },
        NexusFinalityAnchorUpdated {
            epoch: u64,
            validator_set_hash: MessageId,
            min_signatures: u16,
        },
        ParliamentRosterAnchorUpdated {
            epoch: u64,
            roster_hash: MessageId,
            required_signatures: u16,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        PayloadTooLarge,
        UnsupportedDomain,
        UnsupportedCodec,
        InvalidPayloadFormat,
        AssetAlreadyRegistered,
        AssetNotRegistered,
        RouteAlreadyExists,
        RouteNotFound,
        RouteDisabled,
        RouteDomainMismatch,
        AssetHomeDomainMismatch,
        MessageAlreadyConsumed,
        ControlMessageAlreadyConsumed,
        TargetDomainMismatch,
        MessageIdMismatch,
        ZeroAmount,
        ManualInboundFinalizationDisabled,
        ProofVerifierUnavailable,
        InvalidMessageProof,
        NonceOverflow,
        AssetDisabled,
        ProofBundleDecodeFailed,
        InvalidRuntimeProofEnvelope,
        MissingNexusFinalityAnchor,
        MissingParliamentRosterAnchor,
        InsufficientProofSignatures,
        ParliamentProofRequired,
    }

    #[pallet::storage]
    #[pallet::getter(fn registry_asset)]
    pub type RegistryAssets<T: Config> =
        StorageMap<_, Blake2_128Concat, EncodedPayloadOf<T>, RegistryAssetRecordOf<T>, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn route)]
    pub type Routes<T: Config> =
        StorageMap<_, Blake2_128Concat, EncodedPayloadOf<T>, RouteRecordOf<T>, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn next_outbound_nonce)]
    pub type NextOutboundNonce<T: Config> = StorageValue<_, Nonce, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn outbound_message)]
    pub type OutboundMessages<T: Config> =
        StorageMap<_, Blake2_128Concat, Nonce, OutboundMessageRecordOf<T>, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn inbound_consumed)]
    pub type ConsumedInboundMessages<T: Config> =
        StorageMap<_, Blake2_128Concat, InboundMessageKeyOf<T>, bool, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn inbound_proof_receipt)]
    pub type InboundProofReceipts<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        InboundMessageKeyOf<T>,
        MessageProofReceiptOf<T>,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn inbound_commitment_root)]
    pub type InboundCommitmentRoots<T: Config> =
        StorageMap<_, Blake2_128Concat, InboundMessageKeyOf<T>, CommitmentRoot, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn control_message_consumed)]
    pub type ConsumedControlMessages<T: Config> =
        StorageMap<_, Blake2_128Concat, MessageId, bool, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn control_proof_receipt)]
    pub type ControlProofReceipts<T: Config> =
        StorageMap<_, Blake2_128Concat, MessageId, ControlProofReceiptOf<T>, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn nexus_finality_anchor)]
    pub type TrustedNexusFinalityAnchors<T: Config> =
        StorageMap<_, Blake2_128Concat, u64, NexusFinalityAnchor, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn parliament_roster_anchor)]
    pub type TrustedParliamentRosters<T: Config> =
        StorageMap<_, Blake2_128Concat, u64, ParliamentRosterAnchor, OptionQuery>;

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Bootstrap/admin override for importing a registry asset.
        /// Production imports should arrive through `submit_asset_register_proof`.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::import_registry_asset())]
        pub fn import_registry_asset(
            origin: OriginFor<T>,
            asset_id_codec: u8,
            asset_id: Vec<u8>,
            home_domain: DomainId,
            decimals: u8,
        ) -> DispatchResult {
            ensure_root(origin)?;
            Self::ensure_supported_domain(home_domain)?;
            let asset_id = Self::bounded_payload(asset_id_codec, asset_id)?;
            Self::import_registry_asset_record(asset_id, home_domain, decimals)
        }

        /// Bootstrap/admin override for activating a route.
        /// Production route activation should arrive through `submit_route_activate_proof`.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::activate_route())]
        pub fn activate_route(
            origin: OriginFor<T>,
            route_id_codec: u8,
            route_id: Vec<u8>,
            asset_id_codec: u8,
            asset_id: Vec<u8>,
            remote_domain: DomainId,
        ) -> DispatchResult {
            ensure_root(origin)?;
            Self::ensure_supported_domain(remote_domain)?;
            let route_id = Self::bounded_payload(route_id_codec, route_id)?;
            let asset_id = Self::bounded_payload(asset_id_codec, asset_id)?;
            Self::activate_route_record(route_id, asset_id, remote_domain)
        }

        /// Emergency/admin route pause.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::pause_route())]
        pub fn pause_route(
            origin: OriginFor<T>,
            route_id_codec: u8,
            route_id: Vec<u8>,
        ) -> DispatchResult {
            ensure_root(origin)?;
            let route_id = Self::bounded_payload(route_id_codec, route_id)?;
            Self::set_route_enabled(route_id, false)
        }

        /// Emergency/admin route resume.
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::resume_route())]
        pub fn resume_route(
            origin: OriginFor<T>,
            route_id_codec: u8,
            route_id: Vec<u8>,
        ) -> DispatchResult {
            ensure_root(origin)?;
            let route_id = Self::bounded_payload(route_id_codec, route_id)?;
            Self::set_route_enabled(route_id, true)
        }

        /// Administrative entry point for manually recording an outbound message.
        /// User-facing bridge pallets should escrow/burn assets first and then call
        /// `record_outbound_message`.
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::record_outbound())]
        pub fn record_outbound(
            origin: OriginFor<T>,
            route_id_codec: u8,
            route_id: Vec<u8>,
            source_domain: DomainId,
            dest_domain: DomainId,
            asset_home_domain: DomainId,
            amount: u128,
            sender_codec: u8,
            sender: Vec<u8>,
            recipient_codec: u8,
            recipient: Vec<u8>,
        ) -> DispatchResult {
            ensure_root(origin)?;
            let route_id = Self::bounded_payload(route_id_codec, route_id)?;
            let sender = Self::bounded_payload(sender_codec, sender)?;
            let recipient = Self::bounded_payload(recipient_codec, recipient)?;
            Self::record_outbound_message(
                route_id,
                source_domain,
                dest_domain,
                asset_home_domain,
                amount,
                sender,
                recipient,
            )?;
            Ok(())
        }

        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::finalize_inbound())]
        pub fn finalize_inbound(
            origin: OriginFor<T>,
            message_id: MessageId,
            route_id_codec: u8,
            route_id: Vec<u8>,
            source_domain: DomainId,
            commitment_root: CommitmentRoot,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(
                T::AllowManualInboundFinalization::get(),
                Error::<T>::ManualInboundFinalizationDisabled
            );
            let route_id = Self::bounded_payload(route_id_codec, route_id)?;
            Self::finalize_verified_inbound(message_id, route_id, source_domain, commitment_root)
        }

        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::submit_message_proof(
            proof_bytes.len() as u32,
            public_inputs.len() as u32,
            bundle_bytes.len() as u32,
        ))]
        /// Submit an inbound transfer proof delivered by a relay operator.
        ///
        /// For `runtime-scale-v1`, the relay operator is expected to submit the
        /// SCALE envelope fetched through the bridge console as `bundle_bytes`.
        /// The signed origin is only the wallet paying for the relay
        /// transaction; validity is decided by the on-chain proof and anchor
        /// checks.
        pub fn submit_message_proof(
            origin: OriginFor<T>,
            proof_family: Vec<u8>,
            verifier_backend: Vec<u8>,
            proof_bytes: Vec<u8>,
            public_inputs: Vec<u8>,
            bundle_bytes: Vec<u8>,
            message_id: MessageId,
            route_id_codec: u8,
            route_id: Vec<u8>,
            source_domain: DomainId,
            commitment_root: CommitmentRoot,
        ) -> DispatchResult {
            let _relay_operator = ensure_signed(origin)?;
            let proof_family = Self::bounded(proof_family)?;
            let verifier_backend = Self::bounded(verifier_backend)?;
            let proof_bytes = Self::bounded_proof_blob(proof_bytes)?;
            let public_inputs = Self::bounded_proof_blob(public_inputs)?;
            let bundle_bytes = Self::bounded_proof_blob(bundle_bytes)?;
            let route_id = Self::bounded_payload(route_id_codec, route_id)?;

            Self::verify_inbound_message_proof(
                &proof_family,
                &verifier_backend,
                &proof_bytes,
                &public_inputs,
                &bundle_bytes,
                &message_id,
                &route_id,
                source_domain,
                &commitment_root,
            )?;
            Self::ensure_inbound_can_finalize(message_id, &route_id, source_domain)?;
            let inbound_key = Self::inbound_message_key(message_id, &route_id, source_domain);

            InboundProofReceipts::<T>::insert(
                inbound_key,
                MessageProofReceipt {
                    proof_family: proof_family.clone(),
                    verifier_backend: verifier_backend.clone(),
                    route_id: route_id.clone(),
                    source_domain,
                    commitment_root,
                },
            );
            Self::deposit_event(Event::MessageProofAccepted {
                message_id,
                proof_family,
                verifier_backend,
                route_id: route_id.clone(),
                source_domain,
                commitment_root,
            });
            Self::finalize_verified_inbound(message_id, route_id, source_domain, commitment_root)
        }

        #[pallet::call_index(7)]
        #[pallet::weight(T::WeightInfo::submit_control_message_proof(
            proof_bytes.len() as u32,
            public_inputs.len() as u32,
            bundle_bytes.len() as u32,
        ))]
        /// Submit a governed asset registration proof delivered by a relay operator.
        ///
        /// Runtime control proofs require the Nexus finality anchor and the Sora
        /// Parliament roster anchor for the certificate epoch. The relay
        /// operator does not authorize the asset; they only submit the wallet
        /// transaction carrying the proof envelope.
        pub fn submit_asset_register_proof(
            origin: OriginFor<T>,
            proof_family: Vec<u8>,
            verifier_backend: Vec<u8>,
            proof_bytes: Vec<u8>,
            public_inputs: Vec<u8>,
            bundle_bytes: Vec<u8>,
            message_id: MessageId,
            target_domain: DomainId,
            home_domain: DomainId,
            nonce: Nonce,
            asset_id_codec: u8,
            asset_id: Vec<u8>,
            decimals: u8,
            commitment_root: CommitmentRoot,
        ) -> DispatchResult {
            let _relay_operator = ensure_signed(origin)?;
            let proof_family = Self::bounded(proof_family)?;
            let verifier_backend = Self::bounded(verifier_backend)?;
            let proof_bytes = Self::bounded_proof_blob(proof_bytes)?;
            let public_inputs = Self::bounded_proof_blob(public_inputs)?;
            let bundle_bytes = Self::bounded_proof_blob(bundle_bytes)?;
            Self::ensure_local_target_domain(target_domain)?;
            Self::ensure_supported_domain(home_domain)?;
            let asset_id = Self::bounded_payload(asset_id_codec, asset_id)?;
            let expected_message_id = Self::asset_register_message_id(
                target_domain,
                home_domain,
                nonce,
                &asset_id,
                decimals,
            );
            ensure!(
                message_id == expected_message_id,
                Error::<T>::MessageIdMismatch
            );
            Self::ensure_control_message_can_apply(message_id)?;

            let runtime_payload = Self::verify_control_proof(
                &proof_family,
                &verifier_backend,
                &proof_bytes,
                &public_inputs,
                &bundle_bytes,
                &message_id,
                ControlMessageKind::AssetRegister,
                target_domain,
                &commitment_root,
            )?;
            if let Some(runtime_payload) = runtime_payload {
                Self::ensure_asset_register_runtime_payload_matches(
                    &runtime_payload,
                    target_domain,
                    home_domain,
                    nonce,
                    &asset_id,
                    decimals,
                )?;
            }
            Self::import_registry_asset_record(asset_id, home_domain, decimals)?;
            Self::record_control_proof_receipt(
                message_id,
                proof_family,
                verifier_backend,
                ControlMessageKind::AssetRegister,
                target_domain,
                commitment_root,
            );
            Ok(())
        }

        #[pallet::call_index(8)]
        #[pallet::weight(T::WeightInfo::submit_control_message_proof(
            proof_bytes.len() as u32,
            public_inputs.len() as u32,
            bundle_bytes.len() as u32,
        ))]
        pub fn submit_route_activate_proof(
            origin: OriginFor<T>,
            proof_family: Vec<u8>,
            verifier_backend: Vec<u8>,
            proof_bytes: Vec<u8>,
            public_inputs: Vec<u8>,
            bundle_bytes: Vec<u8>,
            message_id: MessageId,
            source_domain: DomainId,
            target_domain: DomainId,
            nonce: Nonce,
            asset_id_codec: u8,
            asset_id: Vec<u8>,
            route_id_codec: u8,
            route_id: Vec<u8>,
            commitment_root: CommitmentRoot,
        ) -> DispatchResult {
            let _relay_operator = ensure_signed(origin)?;
            let proof_family = Self::bounded(proof_family)?;
            let verifier_backend = Self::bounded(verifier_backend)?;
            let proof_bytes = Self::bounded_proof_blob(proof_bytes)?;
            let public_inputs = Self::bounded_proof_blob(public_inputs)?;
            let bundle_bytes = Self::bounded_proof_blob(bundle_bytes)?;
            Self::ensure_supported_domain(source_domain)?;
            Self::ensure_local_target_domain(target_domain)?;
            ensure!(
                source_domain != target_domain,
                Error::<T>::RouteDomainMismatch
            );
            let asset_id = Self::bounded_payload(asset_id_codec, asset_id)?;
            let route_id = Self::bounded_payload(route_id_codec, route_id)?;
            let expected_message_id = Self::route_activate_message_id(
                source_domain,
                target_domain,
                nonce,
                &asset_id,
                &route_id,
            );
            ensure!(
                message_id == expected_message_id,
                Error::<T>::MessageIdMismatch
            );
            Self::ensure_control_message_can_apply(message_id)?;

            let runtime_payload = Self::verify_control_proof(
                &proof_family,
                &verifier_backend,
                &proof_bytes,
                &public_inputs,
                &bundle_bytes,
                &message_id,
                ControlMessageKind::RouteActivate,
                target_domain,
                &commitment_root,
            )?;
            if let Some(runtime_payload) = runtime_payload {
                Self::ensure_route_activate_runtime_payload_matches(
                    &runtime_payload,
                    source_domain,
                    target_domain,
                    nonce,
                    &asset_id,
                    &route_id,
                )?;
            }
            Self::activate_route_record(route_id, asset_id, source_domain)?;
            Self::record_control_proof_receipt(
                message_id,
                proof_family,
                verifier_backend,
                ControlMessageKind::RouteActivate,
                target_domain,
                commitment_root,
            );
            Ok(())
        }

        #[pallet::call_index(9)]
        #[pallet::weight(T::WeightInfo::submit_control_message_proof(
			proof_bytes.len() as u32,
			public_inputs.len() as u32,
			bundle_bytes.len() as u32,
		))]
        pub fn submit_token_add_proof(
            origin: OriginFor<T>,
            proof_family: Vec<u8>,
            verifier_backend: Vec<u8>,
            proof_bytes: Vec<u8>,
            public_inputs: Vec<u8>,
            bundle_bytes: Vec<u8>,
            message_id: MessageId,
            target_domain: DomainId,
            nonce: Nonce,
            sora_asset_id: MessageId,
            decimals: u8,
            name: MessageId,
            symbol: MessageId,
            commitment_root: CommitmentRoot,
        ) -> DispatchResult {
            let _relay_operator = ensure_signed(origin)?;
            let proof_family = Self::bounded(proof_family)?;
            let verifier_backend = Self::bounded(verifier_backend)?;
            let proof_bytes = Self::bounded_proof_blob(proof_bytes)?;
            let public_inputs = Self::bounded_proof_blob(public_inputs)?;
            let bundle_bytes = Self::bounded_proof_blob(bundle_bytes)?;
            Self::ensure_local_target_domain(target_domain)?;
            let expected_message_id = Self::token_add_message_id(
                target_domain,
                nonce,
                sora_asset_id,
                decimals,
                name,
                symbol,
            );
            ensure!(
                message_id == expected_message_id,
                Error::<T>::MessageIdMismatch
            );
            Self::ensure_control_message_can_apply(message_id)?;

            let runtime_payload = Self::verify_control_proof(
                &proof_family,
                &verifier_backend,
                &proof_bytes,
                &public_inputs,
                &bundle_bytes,
                &message_id,
                ControlMessageKind::TokenAdd,
                target_domain,
                &commitment_root,
            )?;
            if let Some(runtime_payload) = runtime_payload {
                Self::ensure_token_add_runtime_payload_matches(
                    &runtime_payload,
                    target_domain,
                    nonce,
                    sora_asset_id,
                    decimals,
                    name,
                    symbol,
                )?;
            }
            let asset_id = Self::sora_asset_payload(sora_asset_id)?;
            Self::import_registry_asset_record_with_metadata(
                asset_id,
                target_domain,
                decimals,
                true,
                Some(sora_asset_id),
                Some(name),
                Some(symbol),
            )?;
            Self::record_control_proof_receipt(
                message_id,
                proof_family,
                verifier_backend,
                ControlMessageKind::TokenAdd,
                target_domain,
                commitment_root,
            );
            Ok(())
        }

        #[pallet::call_index(10)]
        #[pallet::weight(T::WeightInfo::submit_control_message_proof(
			proof_bytes.len() as u32,
			public_inputs.len() as u32,
			bundle_bytes.len() as u32,
		))]
        pub fn submit_token_pause_proof(
            origin: OriginFor<T>,
            proof_family: Vec<u8>,
            verifier_backend: Vec<u8>,
            proof_bytes: Vec<u8>,
            public_inputs: Vec<u8>,
            bundle_bytes: Vec<u8>,
            message_id: MessageId,
            target_domain: DomainId,
            nonce: Nonce,
            sora_asset_id: MessageId,
            commitment_root: CommitmentRoot,
        ) -> DispatchResult {
            let _relay_operator = ensure_signed(origin)?;
            let proof_family = Self::bounded(proof_family)?;
            let verifier_backend = Self::bounded(verifier_backend)?;
            let proof_bytes = Self::bounded_proof_blob(proof_bytes)?;
            let public_inputs = Self::bounded_proof_blob(public_inputs)?;
            let bundle_bytes = Self::bounded_proof_blob(bundle_bytes)?;
            Self::ensure_local_target_domain(target_domain)?;
            let expected_message_id =
                Self::token_pause_message_id(target_domain, nonce, sora_asset_id);
            ensure!(
                message_id == expected_message_id,
                Error::<T>::MessageIdMismatch
            );
            Self::ensure_control_message_can_apply(message_id)?;

            let runtime_payload = Self::verify_control_proof(
                &proof_family,
                &verifier_backend,
                &proof_bytes,
                &public_inputs,
                &bundle_bytes,
                &message_id,
                ControlMessageKind::TokenPause,
                target_domain,
                &commitment_root,
            )?;
            if let Some(runtime_payload) = runtime_payload {
                Self::ensure_token_control_runtime_payload_matches(
                    &runtime_payload,
                    ControlMessageKind::TokenPause,
                    target_domain,
                    nonce,
                    sora_asset_id,
                )?;
            }
            let asset_id = Self::sora_asset_payload(sora_asset_id)?;
            Self::set_registry_asset_enabled(asset_id, false)?;
            Self::record_control_proof_receipt(
                message_id,
                proof_family,
                verifier_backend,
                ControlMessageKind::TokenPause,
                target_domain,
                commitment_root,
            );
            Ok(())
        }

        #[pallet::call_index(11)]
        #[pallet::weight(T::WeightInfo::submit_control_message_proof(
			proof_bytes.len() as u32,
			public_inputs.len() as u32,
			bundle_bytes.len() as u32,
		))]
        pub fn submit_token_resume_proof(
            origin: OriginFor<T>,
            proof_family: Vec<u8>,
            verifier_backend: Vec<u8>,
            proof_bytes: Vec<u8>,
            public_inputs: Vec<u8>,
            bundle_bytes: Vec<u8>,
            message_id: MessageId,
            target_domain: DomainId,
            nonce: Nonce,
            sora_asset_id: MessageId,
            commitment_root: CommitmentRoot,
        ) -> DispatchResult {
            let _relay_operator = ensure_signed(origin)?;
            let proof_family = Self::bounded(proof_family)?;
            let verifier_backend = Self::bounded(verifier_backend)?;
            let proof_bytes = Self::bounded_proof_blob(proof_bytes)?;
            let public_inputs = Self::bounded_proof_blob(public_inputs)?;
            let bundle_bytes = Self::bounded_proof_blob(bundle_bytes)?;
            Self::ensure_local_target_domain(target_domain)?;
            let expected_message_id =
                Self::token_resume_message_id(target_domain, nonce, sora_asset_id);
            ensure!(
                message_id == expected_message_id,
                Error::<T>::MessageIdMismatch
            );
            Self::ensure_control_message_can_apply(message_id)?;

            let runtime_payload = Self::verify_control_proof(
                &proof_family,
                &verifier_backend,
                &proof_bytes,
                &public_inputs,
                &bundle_bytes,
                &message_id,
                ControlMessageKind::TokenResume,
                target_domain,
                &commitment_root,
            )?;
            if let Some(runtime_payload) = runtime_payload {
                Self::ensure_token_control_runtime_payload_matches(
                    &runtime_payload,
                    ControlMessageKind::TokenResume,
                    target_domain,
                    nonce,
                    sora_asset_id,
                )?;
            }
            let asset_id = Self::sora_asset_payload(sora_asset_id)?;
            Self::set_registry_asset_enabled(asset_id, true)?;
            Self::record_control_proof_receipt(
                message_id,
                proof_family,
                verifier_backend,
                ControlMessageKind::TokenResume,
                target_domain,
                commitment_root,
            );
            Ok(())
        }

        #[pallet::call_index(12)]
        #[pallet::weight(T::WeightInfo::set_nexus_finality_anchor())]
        pub fn set_nexus_finality_anchor(
            origin: OriginFor<T>,
            epoch: u64,
            validator_set_hash: MessageId,
            min_signatures: u16,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(min_signatures != 0, Error::<T>::InvalidRuntimeProofEnvelope);
            TrustedNexusFinalityAnchors::<T>::insert(
                epoch,
                NexusFinalityAnchor {
                    validator_set_hash,
                    min_signatures,
                },
            );
            Self::deposit_event(Event::NexusFinalityAnchorUpdated {
                epoch,
                validator_set_hash,
                min_signatures,
            });
            Ok(())
        }

        #[pallet::call_index(13)]
        #[pallet::weight(T::WeightInfo::set_parliament_roster_anchor())]
        pub fn set_parliament_roster_anchor(
            origin: OriginFor<T>,
            epoch: u64,
            roster_hash: MessageId,
            required_signatures: u16,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(
                required_signatures != 0,
                Error::<T>::InvalidRuntimeProofEnvelope
            );
            TrustedParliamentRosters::<T>::insert(
                epoch,
                ParliamentRosterAnchor {
                    roster_hash,
                    required_signatures,
                },
            );
            Self::deposit_event(Event::ParliamentRosterAnchorUpdated {
                epoch,
                roster_hash,
                required_signatures,
            });
            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        pub fn record_outbound_message(
            route_id: EncodedPayloadOf<T>,
            source_domain: DomainId,
            dest_domain: DomainId,
            asset_home_domain: DomainId,
            amount: u128,
            sender: EncodedPayloadOf<T>,
            recipient: EncodedPayloadOf<T>,
        ) -> Result<Nonce, DispatchError> {
            Self::ensure_valid_encoded_payload(&route_id)?;
            Self::ensure_valid_encoded_payload(&sender)?;
            Self::ensure_valid_encoded_payload(&recipient)?;
            Self::ensure_supported_domain(source_domain)?;
            Self::ensure_supported_domain(dest_domain)?;
            Self::ensure_supported_domain(asset_home_domain)?;
            ensure!(
                source_domain != dest_domain,
                Error::<T>::RouteDomainMismatch
            );
            ensure!(amount != 0, Error::<T>::ZeroAmount);

            let route = Routes::<T>::get(&route_id).ok_or(Error::<T>::RouteNotFound)?;
            ensure!(route.enabled, Error::<T>::RouteDisabled);
            ensure!(
                route.remote_domain == dest_domain,
                Error::<T>::RouteDomainMismatch
            );
            let asset_record =
                RegistryAssets::<T>::get(&route.asset_id).ok_or(Error::<T>::AssetNotRegistered)?;
            ensure!(asset_record.enabled, Error::<T>::AssetDisabled);
            ensure!(
                asset_record.home_domain == asset_home_domain,
                Error::<T>::AssetHomeDomainMismatch
            );

            let nonce = NextOutboundNonce::<T>::get();
            let next_nonce = nonce.checked_add(1).ok_or(Error::<T>::NonceOverflow)?;
            NextOutboundNonce::<T>::put(next_nonce);
            OutboundMessages::<T>::insert(
                nonce,
                OutboundMessageRecord {
                    route_id: route_id.clone(),
                    source_domain,
                    dest_domain,
                    asset_home_domain,
                    amount,
                    sender,
                    recipient,
                },
            );
            Self::deposit_event(Event::OutboundRecorded {
                nonce,
                route_id,
                dest_domain,
            });
            Ok(nonce)
        }

        fn import_registry_asset_record(
            asset_id: EncodedPayloadOf<T>,
            home_domain: DomainId,
            decimals: u8,
        ) -> DispatchResult {
            Self::import_registry_asset_record_with_metadata(
                asset_id,
                home_domain,
                decimals,
                true,
                None,
                None,
                None,
            )
        }

        fn import_registry_asset_record_with_metadata(
            asset_id: EncodedPayloadOf<T>,
            home_domain: DomainId,
            decimals: u8,
            enabled: bool,
            sora_asset_id: Option<MessageId>,
            name: Option<MessageId>,
            symbol: Option<MessageId>,
        ) -> DispatchResult {
            ensure!(
                !RegistryAssets::<T>::contains_key(&asset_id),
                Error::<T>::AssetAlreadyRegistered
            );
            RegistryAssets::<T>::insert(
                asset_id.clone(),
                RegistryAssetRecord {
                    home_domain,
                    decimals,
                    asset_id: asset_id.clone(),
                    enabled,
                    sora_asset_id,
                    name,
                    symbol,
                },
            );
            Self::deposit_event(Event::RegistryAssetImported {
                asset_id,
                home_domain,
                decimals,
            });
            Ok(())
        }

        fn set_registry_asset_enabled(
            asset_id: EncodedPayloadOf<T>,
            enabled: bool,
        ) -> DispatchResult {
            RegistryAssets::<T>::try_mutate(&asset_id, |maybe_asset| -> DispatchResult {
                let asset = maybe_asset.as_mut().ok_or(Error::<T>::AssetNotRegistered)?;
                asset.enabled = enabled;
                Ok(())
            })?;
            if enabled {
                Self::deposit_event(Event::RegistryAssetResumed { asset_id });
            } else {
                Self::deposit_event(Event::RegistryAssetPaused { asset_id });
            }
            Ok(())
        }

        fn activate_route_record(
            route_id: EncodedPayloadOf<T>,
            asset_id: EncodedPayloadOf<T>,
            remote_domain: DomainId,
        ) -> DispatchResult {
            ensure!(
                !Routes::<T>::contains_key(&route_id),
                Error::<T>::RouteAlreadyExists
            );
            ensure!(
                RegistryAssets::<T>::contains_key(&asset_id),
                Error::<T>::AssetNotRegistered
            );
            Routes::<T>::insert(
                route_id.clone(),
                RouteRecord {
                    asset_id: asset_id.clone(),
                    remote_domain,
                    enabled: true,
                },
            );
            Self::deposit_event(Event::RouteActivated {
                route_id,
                asset_id,
                remote_domain,
            });
            Ok(())
        }

        fn set_route_enabled(route_id: EncodedPayloadOf<T>, enabled: bool) -> DispatchResult {
            Routes::<T>::try_mutate(&route_id, |maybe_route| -> DispatchResult {
                let route = maybe_route.as_mut().ok_or(Error::<T>::RouteNotFound)?;
                route.enabled = enabled;
                Ok(())
            })?;
            if enabled {
                Self::deposit_event(Event::RouteResumed { route_id });
            } else {
                Self::deposit_event(Event::RoutePaused { route_id });
            }
            Ok(())
        }

        fn map_proof_error(error: MessageProofVerificationError) -> Error<T> {
            match error {
                MessageProofVerificationError::Unavailable => Error::<T>::ProofVerifierUnavailable,
                MessageProofVerificationError::Invalid => Error::<T>::InvalidMessageProof,
            }
        }

        fn ensure_local_target_domain(target_domain: DomainId) -> Result<(), Error<T>> {
            Self::ensure_supported_domain(target_domain)?;
            ensure!(
                target_domain == T::LocalDomain::get(),
                Error::<T>::TargetDomainMismatch
            );
            Ok(())
        }

        fn ensure_control_message_can_apply(message_id: MessageId) -> Result<(), Error<T>> {
            ensure!(
                !ConsumedControlMessages::<T>::get(message_id),
                Error::<T>::ControlMessageAlreadyConsumed
            );
            Ok(())
        }

        fn record_control_proof_receipt(
            message_id: MessageId,
            proof_family: PayloadOf<T>,
            verifier_backend: PayloadOf<T>,
            message_kind: ControlMessageKind,
            target_domain: DomainId,
            commitment_root: CommitmentRoot,
        ) {
            ConsumedControlMessages::<T>::insert(message_id, true);
            ControlProofReceipts::<T>::insert(
                message_id,
                ControlProofReceipt {
                    proof_family: proof_family.clone(),
                    verifier_backend: verifier_backend.clone(),
                    message_kind,
                    target_domain,
                    commitment_root,
                },
            );
            Self::deposit_event(Event::ControlMessageProofAccepted {
                message_id,
                proof_family,
                verifier_backend,
                message_kind,
                target_domain,
                commitment_root,
            });
        }

        fn ensure_supported_domain(domain_id: DomainId) -> Result<(), Error<T>> {
            ensure!(
                is_supported_domain(domain_id),
                Error::<T>::UnsupportedDomain
            );
            Ok(())
        }

        fn bounded(payload: Vec<u8>) -> Result<PayloadOf<T>, Error<T>> {
            payload.try_into().map_err(|_| Error::<T>::PayloadTooLarge)
        }

        fn bounded_proof_blob(payload: Vec<u8>) -> Result<ProofBlobOf<T>, Error<T>> {
            payload.try_into().map_err(|_| Error::<T>::PayloadTooLarge)
        }

        fn bounded_payload(codec: u8, payload: Vec<u8>) -> Result<EncodedPayloadOf<T>, Error<T>> {
            ensure!(is_supported_codec(codec), Error::<T>::UnsupportedCodec);
            ensure!(
                validate_payload_codec(codec, &payload),
                Error::<T>::InvalidPayloadFormat
            );
            Ok(EncodedPayload {
                codec,
                bytes: Self::bounded(payload)?,
            })
        }

        fn ensure_valid_encoded_payload(payload: &EncodedPayloadOf<T>) -> Result<(), Error<T>> {
            ensure!(
                is_supported_codec(payload.codec),
                Error::<T>::UnsupportedCodec
            );
            ensure!(
                validate_payload_codec(payload.codec, payload.bytes.as_slice()),
                Error::<T>::InvalidPayloadFormat
            );
            Ok(())
        }

        fn sora_asset_payload(sora_asset_id: MessageId) -> Result<EncodedPayloadOf<T>, Error<T>> {
            Ok(EncodedPayload {
                codec: SCCP_CODEC_SORA_ASSET_ID,
                bytes: Self::bounded(sora_asset_id.to_vec())?,
            })
        }

        fn is_runtime_proof_marker(
            proof_family: &PayloadOf<T>,
            verifier_backend: &PayloadOf<T>,
        ) -> bool {
            proof_family.as_slice() == SCCP_RUNTIME_PROOF_FAMILY_V1
                && verifier_backend.as_slice() == SCCP_RUNTIME_VERIFIER_BACKEND_V1
        }

        fn control_kind_to_runtime_kind(message_kind: ControlMessageKind) -> RuntimeProofKind {
            match message_kind {
                ControlMessageKind::AssetRegister => RuntimeProofKind::AssetRegister,
                ControlMessageKind::RouteActivate => RuntimeProofKind::RouteActivate,
                ControlMessageKind::TokenAdd => RuntimeProofKind::TokenAdd,
                ControlMessageKind::TokenPause => RuntimeProofKind::TokenPause,
                ControlMessageKind::TokenResume => RuntimeProofKind::TokenResume,
            }
        }

        fn verify_inbound_message_proof(
            proof_family: &PayloadOf<T>,
            verifier_backend: &PayloadOf<T>,
            proof_bytes: &ProofBlobOf<T>,
            public_inputs: &ProofBlobOf<T>,
            bundle_bytes: &ProofBlobOf<T>,
            message_id: &MessageId,
            route_id: &EncodedPayloadOf<T>,
            source_domain: DomainId,
            commitment_root: &CommitmentRoot,
        ) -> Result<(), Error<T>> {
            if Self::is_runtime_proof_marker(proof_family, verifier_backend) {
                let runtime_payload = Self::verify_runtime_envelope(
                    bundle_bytes.as_slice(),
                    RuntimeProofKind::Transfer,
                    message_id,
                    T::LocalDomain::get(),
                    commitment_root,
                    false,
                )?;
                Self::ensure_transfer_runtime_payload_matches(
                    &runtime_payload,
                    route_id,
                    source_domain,
                )?;
                return Ok(());
            }

            T::MessageProofVerifier::verify_message_proof(
                proof_family.as_slice(),
                verifier_backend.as_slice(),
                proof_bytes.as_slice(),
                public_inputs.as_slice(),
                bundle_bytes.as_slice(),
                message_id,
                route_id.codec,
                route_id.bytes.as_slice(),
                source_domain,
                commitment_root,
            )
            .map_err(Self::map_proof_error)
        }

        fn verify_control_proof(
            proof_family: &PayloadOf<T>,
            verifier_backend: &PayloadOf<T>,
            proof_bytes: &ProofBlobOf<T>,
            public_inputs: &ProofBlobOf<T>,
            bundle_bytes: &ProofBlobOf<T>,
            message_id: &MessageId,
            message_kind: ControlMessageKind,
            target_domain: DomainId,
            commitment_root: &CommitmentRoot,
        ) -> Result<Option<RuntimeProofPayloadOf<T>>, Error<T>> {
            if Self::is_runtime_proof_marker(proof_family, verifier_backend) {
                let runtime_payload = Self::verify_runtime_envelope(
                    bundle_bytes.as_slice(),
                    Self::control_kind_to_runtime_kind(message_kind),
                    message_id,
                    target_domain,
                    commitment_root,
                    true,
                )?;
                return Ok(Some(runtime_payload));
            }

            T::ControlMessageProofVerifier::verify_control_message_proof(
                proof_family.as_slice(),
                verifier_backend.as_slice(),
                proof_bytes.as_slice(),
                public_inputs.as_slice(),
                bundle_bytes.as_slice(),
                message_id,
                message_kind,
                target_domain,
                commitment_root,
            )
            .map_err(Self::map_proof_error)?;
            Ok(None)
        }

        fn verify_runtime_envelope(
            bundle_bytes: &[u8],
            expected_kind: RuntimeProofKind,
            expected_message_id: &MessageId,
            expected_target_domain: DomainId,
            expected_commitment_root: &CommitmentRoot,
            require_parliament: bool,
        ) -> Result<RuntimeProofPayloadOf<T>, Error<T>> {
            let mut input = bundle_bytes;
            let envelope = SccpRuntimeProofEnvelopeOf::<T>::decode(&mut input)
                .map_err(|_| Error::<T>::ProofBundleDecodeFailed)?;
            ensure!(input.is_empty(), Error::<T>::ProofBundleDecodeFailed);
            ensure!(
                envelope.version == 1,
                Error::<T>::InvalidRuntimeProofEnvelope
            );
            ensure!(
                envelope.commitment_root == *expected_commitment_root,
                Error::<T>::InvalidRuntimeProofEnvelope
            );
            Self::ensure_valid_runtime_payload(&envelope.payload)?;

            let payload_kind = Self::runtime_payload_kind(&envelope.payload);
            let target_domain = Self::runtime_payload_target_domain(&envelope.payload);
            let message_id = Self::runtime_payload_message_id(&envelope.payload);
            let payload_bytes = Self::canonical_runtime_payload_bytes(&envelope.payload);
            let payload_hash = Self::payload_hash(&payload_bytes);

            ensure!(
                payload_kind == expected_kind,
                Error::<T>::InvalidRuntimeProofEnvelope
            );
            ensure!(
                target_domain == expected_target_domain,
                Error::<T>::TargetDomainMismatch
            );
            ensure!(
                message_id == *expected_message_id,
                Error::<T>::MessageIdMismatch
            );
            ensure!(
                envelope.commitment.version == 1
                    && envelope.commitment.kind == expected_kind
                    && envelope.commitment.target_domain == expected_target_domain
                    && envelope.commitment.message_id == *expected_message_id
                    && envelope.commitment.payload_hash == payload_hash,
                Error::<T>::InvalidRuntimeProofEnvelope
            );
            ensure!(
                Self::merkle_root_from_commitment(&envelope.commitment, &envelope.merkle_proof)
                    == envelope.commitment_root,
                Error::<T>::InvalidRuntimeProofEnvelope
            );
            Self::verify_runtime_finality_proof(
                &envelope.finality_proof,
                &envelope.commitment_root,
            )?;

            if require_parliament {
                let Some(certificate) = envelope.parliament_certificate else {
                    return Err(Error::<T>::ParliamentProofRequired);
                };
                Self::verify_runtime_parliament_certificate(
                    &certificate,
                    envelope.finality_proof.height,
                    &payload_hash,
                    envelope.commitment.parliament_certificate_hash.as_ref(),
                )?;
            } else {
                ensure!(
                    envelope.parliament_certificate.is_none()
                        && envelope.commitment.parliament_certificate_hash.is_none(),
                    Error::<T>::InvalidRuntimeProofEnvelope
                );
            }

            Ok(envelope.payload)
        }

        fn verify_runtime_finality_proof(
            finality_proof: &RuntimeFinalityProof,
            commitment_root: &CommitmentRoot,
        ) -> Result<(), Error<T>> {
            ensure!(
                finality_proof.version == 1
                    && finality_proof.height != 0
                    && finality_proof.block_hash != [0u8; 32]
                    && finality_proof.commitment_root == *commitment_root
                    && finality_proof.signature_count != 0,
                Error::<T>::InvalidRuntimeProofEnvelope
            );
            let anchor = TrustedNexusFinalityAnchors::<T>::get(finality_proof.epoch)
                .ok_or(Error::<T>::MissingNexusFinalityAnchor)?;
            ensure!(
                anchor.validator_set_hash == finality_proof.validator_set_hash,
                Error::<T>::InvalidRuntimeProofEnvelope
            );
            ensure!(
                finality_proof.signature_count >= anchor.min_signatures,
                Error::<T>::InsufficientProofSignatures
            );
            Ok(())
        }

        fn verify_runtime_parliament_certificate(
            certificate: &RuntimeParliamentCertificate,
            finality_height: u64,
            expected_payload_hash: &MessageId,
            expected_certificate_hash: Option<&MessageId>,
        ) -> Result<(), Error<T>> {
            ensure!(
                certificate.version == 1
                    && certificate.preimage_hash == *expected_payload_hash
                    && certificate.enactment_window_start <= certificate.enactment_window_end
                    && finality_height >= certificate.enactment_window_start
                    && finality_height <= certificate.enactment_window_end
                    && certificate.required_signatures != 0
                    && certificate.signature_count != 0
                    && expected_certificate_hash == Some(&certificate.certificate_hash),
                Error::<T>::InvalidRuntimeProofEnvelope
            );
            let anchor = TrustedParliamentRosters::<T>::get(certificate.roster_epoch)
                .ok_or(Error::<T>::MissingParliamentRosterAnchor)?;
            ensure!(
                anchor.roster_hash == certificate.roster_hash
                    && anchor.required_signatures == certificate.required_signatures,
                Error::<T>::InvalidRuntimeProofEnvelope
            );
            ensure!(
                certificate.signature_count >= certificate.required_signatures,
                Error::<T>::InsufficientProofSignatures
            );
            Ok(())
        }

        fn ensure_valid_runtime_payload(
            payload: &RuntimeProofPayloadOf<T>,
        ) -> Result<(), Error<T>> {
            match payload {
                RuntimeProofPayload::AssetRegister {
                    target_domain,
                    home_domain,
                    asset_id,
                    ..
                } => {
                    Self::ensure_supported_domain(*target_domain)?;
                    Self::ensure_supported_domain(*home_domain)?;
                    Self::ensure_valid_encoded_payload(asset_id)?;
                }
                RuntimeProofPayload::RouteActivate {
                    source_domain,
                    target_domain,
                    asset_id,
                    route_id,
                    ..
                } => {
                    Self::ensure_supported_domain(*source_domain)?;
                    Self::ensure_supported_domain(*target_domain)?;
                    ensure!(
                        source_domain != target_domain,
                        Error::<T>::RouteDomainMismatch
                    );
                    Self::ensure_valid_encoded_payload(asset_id)?;
                    Self::ensure_valid_encoded_payload(route_id)?;
                }
                RuntimeProofPayload::Transfer {
                    source_domain,
                    dest_domain,
                    asset_home_domain,
                    asset_id,
                    amount,
                    sender,
                    recipient,
                    route_id,
                    ..
                } => {
                    Self::ensure_supported_domain(*source_domain)?;
                    Self::ensure_supported_domain(*dest_domain)?;
                    Self::ensure_supported_domain(*asset_home_domain)?;
                    ensure!(
                        source_domain != dest_domain,
                        Error::<T>::RouteDomainMismatch
                    );
                    ensure!(*amount != 0, Error::<T>::ZeroAmount);
                    Self::ensure_valid_encoded_payload(asset_id)?;
                    Self::ensure_valid_encoded_payload(sender)?;
                    Self::ensure_valid_encoded_payload(recipient)?;
                    Self::ensure_valid_encoded_payload(route_id)?;
                }
                RuntimeProofPayload::TokenAdd { target_domain, .. }
                | RuntimeProofPayload::TokenPause { target_domain, .. }
                | RuntimeProofPayload::TokenResume { target_domain, .. } => {
                    Self::ensure_supported_domain(*target_domain)?;
                }
            }
            Ok(())
        }

        fn ensure_asset_register_runtime_payload_matches(
            payload: &RuntimeProofPayloadOf<T>,
            expected_target_domain: DomainId,
            expected_home_domain: DomainId,
            expected_nonce: Nonce,
            expected_asset_id: &EncodedPayloadOf<T>,
            expected_decimals: u8,
        ) -> Result<(), Error<T>> {
            match payload {
                RuntimeProofPayload::AssetRegister {
                    target_domain,
                    home_domain,
                    nonce,
                    asset_id,
                    decimals,
                } if *target_domain == expected_target_domain
                    && *home_domain == expected_home_domain
                    && *nonce == expected_nonce
                    && asset_id == expected_asset_id
                    && *decimals == expected_decimals =>
                {
                    Ok(())
                }
                _ => Err(Error::<T>::InvalidRuntimeProofEnvelope),
            }
        }

        fn ensure_route_activate_runtime_payload_matches(
            payload: &RuntimeProofPayloadOf<T>,
            expected_source_domain: DomainId,
            expected_target_domain: DomainId,
            expected_nonce: Nonce,
            expected_asset_id: &EncodedPayloadOf<T>,
            expected_route_id: &EncodedPayloadOf<T>,
        ) -> Result<(), Error<T>> {
            match payload {
                RuntimeProofPayload::RouteActivate {
                    source_domain,
                    target_domain,
                    nonce,
                    asset_id,
                    route_id,
                } if *source_domain == expected_source_domain
                    && *target_domain == expected_target_domain
                    && *nonce == expected_nonce
                    && asset_id == expected_asset_id
                    && route_id == expected_route_id =>
                {
                    Ok(())
                }
                _ => Err(Error::<T>::InvalidRuntimeProofEnvelope),
            }
        }

        fn ensure_token_add_runtime_payload_matches(
            payload: &RuntimeProofPayloadOf<T>,
            expected_target_domain: DomainId,
            expected_nonce: Nonce,
            expected_sora_asset_id: MessageId,
            expected_decimals: u8,
            expected_name: MessageId,
            expected_symbol: MessageId,
        ) -> Result<(), Error<T>> {
            match payload {
                RuntimeProofPayload::TokenAdd {
                    target_domain,
                    nonce,
                    sora_asset_id,
                    decimals,
                    name,
                    symbol,
                } if *target_domain == expected_target_domain
                    && *nonce == expected_nonce
                    && *sora_asset_id == expected_sora_asset_id
                    && *decimals == expected_decimals
                    && *name == expected_name
                    && *symbol == expected_symbol =>
                {
                    Ok(())
                }
                _ => Err(Error::<T>::InvalidRuntimeProofEnvelope),
            }
        }

        fn ensure_token_control_runtime_payload_matches(
            payload: &RuntimeProofPayloadOf<T>,
            expected_kind: ControlMessageKind,
            expected_target_domain: DomainId,
            expected_nonce: Nonce,
            expected_sora_asset_id: MessageId,
        ) -> Result<(), Error<T>> {
            match (expected_kind, payload) {
                (
                    ControlMessageKind::TokenPause,
                    RuntimeProofPayload::TokenPause {
                        target_domain,
                        nonce,
                        sora_asset_id,
                    },
                )
                | (
                    ControlMessageKind::TokenResume,
                    RuntimeProofPayload::TokenResume {
                        target_domain,
                        nonce,
                        sora_asset_id,
                    },
                ) if *target_domain == expected_target_domain
                    && *nonce == expected_nonce
                    && *sora_asset_id == expected_sora_asset_id =>
                {
                    Ok(())
                }
                _ => Err(Error::<T>::InvalidRuntimeProofEnvelope),
            }
        }

        fn ensure_transfer_runtime_payload_matches(
            payload: &RuntimeProofPayloadOf<T>,
            expected_route_id: &EncodedPayloadOf<T>,
            expected_source_domain: DomainId,
        ) -> Result<(), Error<T>> {
            match payload {
                RuntimeProofPayload::Transfer {
                    source_domain,
                    dest_domain,
                    route_id,
                    ..
                } if *source_domain == expected_source_domain
                    && *dest_domain == T::LocalDomain::get()
                    && route_id == expected_route_id =>
                {
                    Ok(())
                }
                _ => Err(Error::<T>::InvalidRuntimeProofEnvelope),
            }
        }

        pub(crate) fn asset_register_message_id(
            target_domain: DomainId,
            home_domain: DomainId,
            nonce: Nonce,
            asset_id: &EncodedPayloadOf<T>,
            decimals: u8,
        ) -> MessageId {
            Self::prefixed_keccak(
                SCCP_MSG_PREFIX_ASSET_REGISTER_V1,
                &Self::asset_register_payload_bytes(
                    target_domain,
                    home_domain,
                    nonce,
                    asset_id,
                    decimals,
                ),
            )
        }

        pub(crate) fn route_activate_message_id(
            source_domain: DomainId,
            target_domain: DomainId,
            nonce: Nonce,
            asset_id: &EncodedPayloadOf<T>,
            route_id: &EncodedPayloadOf<T>,
        ) -> MessageId {
            Self::prefixed_keccak(
                SCCP_MSG_PREFIX_ROUTE_ACTIVATE_V1,
                &Self::route_activate_payload_bytes(
                    source_domain,
                    target_domain,
                    nonce,
                    asset_id,
                    route_id,
                ),
            )
        }

        pub(crate) fn token_add_message_id(
            target_domain: DomainId,
            nonce: Nonce,
            sora_asset_id: MessageId,
            decimals: u8,
            name: MessageId,
            symbol: MessageId,
        ) -> MessageId {
            Self::prefixed_keccak(
                SCCP_MSG_PREFIX_TOKEN_ADD_V1,
                &Self::token_add_payload_bytes(
                    target_domain,
                    nonce,
                    sora_asset_id,
                    decimals,
                    name,
                    symbol,
                ),
            )
        }

        pub(crate) fn token_pause_message_id(
            target_domain: DomainId,
            nonce: Nonce,
            sora_asset_id: MessageId,
        ) -> MessageId {
            Self::prefixed_keccak(
                SCCP_MSG_PREFIX_TOKEN_PAUSE_V1,
                &Self::token_control_payload_bytes(target_domain, nonce, sora_asset_id),
            )
        }

        pub(crate) fn token_resume_message_id(
            target_domain: DomainId,
            nonce: Nonce,
            sora_asset_id: MessageId,
        ) -> MessageId {
            Self::prefixed_keccak(
                SCCP_MSG_PREFIX_TOKEN_RESUME_V1,
                &Self::token_control_payload_bytes(target_domain, nonce, sora_asset_id),
            )
        }

        fn transfer_message_id(
            source_domain: DomainId,
            dest_domain: DomainId,
            nonce: Nonce,
            asset_home_domain: DomainId,
            asset_id: &EncodedPayloadOf<T>,
            amount: u128,
            sender: &EncodedPayloadOf<T>,
            recipient: &EncodedPayloadOf<T>,
            route_id: &EncodedPayloadOf<T>,
        ) -> MessageId {
            Self::prefixed_keccak(
                SCCP_MSG_PREFIX_TRANSFER_V1,
                &Self::transfer_payload_bytes(
                    source_domain,
                    dest_domain,
                    nonce,
                    asset_home_domain,
                    asset_id,
                    amount,
                    sender,
                    recipient,
                    route_id,
                ),
            )
        }

        pub(crate) fn runtime_payload_message_id(payload: &RuntimeProofPayloadOf<T>) -> MessageId {
            match payload {
                RuntimeProofPayload::AssetRegister {
                    target_domain,
                    home_domain,
                    nonce,
                    asset_id,
                    decimals,
                } => Self::asset_register_message_id(
                    *target_domain,
                    *home_domain,
                    *nonce,
                    asset_id,
                    *decimals,
                ),
                RuntimeProofPayload::RouteActivate {
                    source_domain,
                    target_domain,
                    nonce,
                    asset_id,
                    route_id,
                } => Self::route_activate_message_id(
                    *source_domain,
                    *target_domain,
                    *nonce,
                    asset_id,
                    route_id,
                ),
                RuntimeProofPayload::Transfer {
                    source_domain,
                    dest_domain,
                    nonce,
                    asset_home_domain,
                    asset_id,
                    amount,
                    sender,
                    recipient,
                    route_id,
                } => Self::transfer_message_id(
                    *source_domain,
                    *dest_domain,
                    *nonce,
                    *asset_home_domain,
                    asset_id,
                    *amount,
                    sender,
                    recipient,
                    route_id,
                ),
                RuntimeProofPayload::TokenAdd {
                    target_domain,
                    nonce,
                    sora_asset_id,
                    decimals,
                    name,
                    symbol,
                } => Self::token_add_message_id(
                    *target_domain,
                    *nonce,
                    *sora_asset_id,
                    *decimals,
                    *name,
                    *symbol,
                ),
                RuntimeProofPayload::TokenPause {
                    target_domain,
                    nonce,
                    sora_asset_id,
                } => Self::token_pause_message_id(*target_domain, *nonce, *sora_asset_id),
                RuntimeProofPayload::TokenResume {
                    target_domain,
                    nonce,
                    sora_asset_id,
                } => Self::token_resume_message_id(*target_domain, *nonce, *sora_asset_id),
            }
        }

        fn runtime_payload_kind(payload: &RuntimeProofPayloadOf<T>) -> RuntimeProofKind {
            match payload {
                RuntimeProofPayload::AssetRegister { .. } => RuntimeProofKind::AssetRegister,
                RuntimeProofPayload::RouteActivate { .. } => RuntimeProofKind::RouteActivate,
                RuntimeProofPayload::Transfer { .. } => RuntimeProofKind::Transfer,
                RuntimeProofPayload::TokenAdd { .. } => RuntimeProofKind::TokenAdd,
                RuntimeProofPayload::TokenPause { .. } => RuntimeProofKind::TokenPause,
                RuntimeProofPayload::TokenResume { .. } => RuntimeProofKind::TokenResume,
            }
        }

        fn runtime_payload_target_domain(payload: &RuntimeProofPayloadOf<T>) -> DomainId {
            match payload {
                RuntimeProofPayload::AssetRegister { target_domain, .. }
                | RuntimeProofPayload::RouteActivate { target_domain, .. }
                | RuntimeProofPayload::TokenAdd { target_domain, .. }
                | RuntimeProofPayload::TokenPause { target_domain, .. }
                | RuntimeProofPayload::TokenResume { target_domain, .. } => *target_domain,
                RuntimeProofPayload::Transfer { dest_domain, .. } => *dest_domain,
            }
        }

        pub(crate) fn canonical_runtime_payload_bytes(
            payload: &RuntimeProofPayloadOf<T>,
        ) -> Vec<u8> {
            let mut out = Vec::new();
            match payload {
                RuntimeProofPayload::AssetRegister {
                    target_domain,
                    home_domain,
                    nonce,
                    asset_id,
                    decimals,
                } => {
                    Self::push_u8(&mut out, 0);
                    out.extend_from_slice(&Self::asset_register_payload_bytes(
                        *target_domain,
                        *home_domain,
                        *nonce,
                        asset_id,
                        *decimals,
                    ));
                }
                RuntimeProofPayload::RouteActivate {
                    source_domain,
                    target_domain,
                    nonce,
                    asset_id,
                    route_id,
                } => {
                    Self::push_u8(&mut out, 1);
                    out.extend_from_slice(&Self::route_activate_payload_bytes(
                        *source_domain,
                        *target_domain,
                        *nonce,
                        asset_id,
                        route_id,
                    ));
                }
                RuntimeProofPayload::Transfer {
                    source_domain,
                    dest_domain,
                    nonce,
                    asset_home_domain,
                    asset_id,
                    amount,
                    sender,
                    recipient,
                    route_id,
                } => {
                    Self::push_u8(&mut out, 2);
                    out.extend_from_slice(&Self::transfer_payload_bytes(
                        *source_domain,
                        *dest_domain,
                        *nonce,
                        *asset_home_domain,
                        asset_id,
                        *amount,
                        sender,
                        recipient,
                        route_id,
                    ));
                }
                RuntimeProofPayload::TokenAdd {
                    target_domain,
                    nonce,
                    sora_asset_id,
                    decimals,
                    name,
                    symbol,
                } => {
                    Self::push_u8(&mut out, 0);
                    out.extend_from_slice(&Self::token_add_payload_bytes(
                        *target_domain,
                        *nonce,
                        *sora_asset_id,
                        *decimals,
                        *name,
                        *symbol,
                    ));
                }
                RuntimeProofPayload::TokenPause {
                    target_domain,
                    nonce,
                    sora_asset_id,
                } => {
                    Self::push_u8(&mut out, 1);
                    out.extend_from_slice(&Self::token_control_payload_bytes(
                        *target_domain,
                        *nonce,
                        *sora_asset_id,
                    ));
                }
                RuntimeProofPayload::TokenResume {
                    target_domain,
                    nonce,
                    sora_asset_id,
                } => {
                    Self::push_u8(&mut out, 2);
                    out.extend_from_slice(&Self::token_control_payload_bytes(
                        *target_domain,
                        *nonce,
                        *sora_asset_id,
                    ));
                }
            }
            out
        }

        fn asset_register_payload_bytes(
            target_domain: DomainId,
            home_domain: DomainId,
            nonce: Nonce,
            asset_id: &EncodedPayloadOf<T>,
            decimals: u8,
        ) -> Vec<u8> {
            let mut out = Vec::new();
            Self::push_u8(&mut out, 1);
            Self::push_u32(&mut out, target_domain);
            Self::push_u32(&mut out, home_domain);
            Self::push_u64(&mut out, nonce);
            Self::push_u8(&mut out, asset_id.codec);
            Self::push_vec(&mut out, asset_id.bytes.as_slice());
            Self::push_u8(&mut out, decimals);
            out
        }

        fn route_activate_payload_bytes(
            source_domain: DomainId,
            target_domain: DomainId,
            nonce: Nonce,
            asset_id: &EncodedPayloadOf<T>,
            route_id: &EncodedPayloadOf<T>,
        ) -> Vec<u8> {
            let mut out = Vec::new();
            Self::push_u8(&mut out, 1);
            Self::push_u32(&mut out, source_domain);
            Self::push_u32(&mut out, target_domain);
            Self::push_u64(&mut out, nonce);
            Self::push_u8(&mut out, asset_id.codec);
            Self::push_vec(&mut out, asset_id.bytes.as_slice());
            Self::push_u8(&mut out, route_id.codec);
            Self::push_vec(&mut out, route_id.bytes.as_slice());
            out
        }

        fn transfer_payload_bytes(
            source_domain: DomainId,
            dest_domain: DomainId,
            nonce: Nonce,
            asset_home_domain: DomainId,
            asset_id: &EncodedPayloadOf<T>,
            amount: u128,
            sender: &EncodedPayloadOf<T>,
            recipient: &EncodedPayloadOf<T>,
            route_id: &EncodedPayloadOf<T>,
        ) -> Vec<u8> {
            let mut out = Vec::new();
            Self::push_u8(&mut out, 1);
            Self::push_u32(&mut out, source_domain);
            Self::push_u32(&mut out, dest_domain);
            Self::push_u64(&mut out, nonce);
            Self::push_u32(&mut out, asset_home_domain);
            Self::push_u8(&mut out, asset_id.codec);
            Self::push_vec(&mut out, asset_id.bytes.as_slice());
            Self::push_u128(&mut out, amount);
            Self::push_u8(&mut out, sender.codec);
            Self::push_vec(&mut out, sender.bytes.as_slice());
            Self::push_u8(&mut out, recipient.codec);
            Self::push_vec(&mut out, recipient.bytes.as_slice());
            Self::push_u8(&mut out, route_id.codec);
            Self::push_vec(&mut out, route_id.bytes.as_slice());
            out
        }

        fn token_add_payload_bytes(
            target_domain: DomainId,
            nonce: Nonce,
            sora_asset_id: MessageId,
            decimals: u8,
            name: MessageId,
            symbol: MessageId,
        ) -> Vec<u8> {
            let mut out = Vec::new();
            Self::push_u8(&mut out, 1);
            Self::push_u32(&mut out, target_domain);
            Self::push_u64(&mut out, nonce);
            out.extend_from_slice(&sora_asset_id);
            Self::push_u8(&mut out, decimals);
            out.extend_from_slice(&name);
            out.extend_from_slice(&symbol);
            out
        }

        fn token_control_payload_bytes(
            target_domain: DomainId,
            nonce: Nonce,
            sora_asset_id: MessageId,
        ) -> Vec<u8> {
            let mut out = Vec::new();
            Self::push_u8(&mut out, 1);
            Self::push_u32(&mut out, target_domain);
            Self::push_u64(&mut out, nonce);
            out.extend_from_slice(&sora_asset_id);
            out
        }

        fn runtime_kind_code(kind: RuntimeProofKind) -> u8 {
            match kind {
                RuntimeProofKind::Burn => 0,
                RuntimeProofKind::TokenAdd => 1,
                RuntimeProofKind::TokenPause => 2,
                RuntimeProofKind::TokenResume => 3,
                RuntimeProofKind::AssetRegister => 4,
                RuntimeProofKind::RouteActivate => 5,
                RuntimeProofKind::Transfer => 6,
            }
        }

        fn canonical_runtime_commitment_bytes(commitment: &RuntimeHubCommitment) -> Vec<u8> {
            let mut out = Vec::with_capacity(1 + 1 + 4 + 32 + 32 + 1 + 32);
            Self::push_u8(&mut out, commitment.version);
            Self::push_u8(&mut out, Self::runtime_kind_code(commitment.kind));
            Self::push_u32(&mut out, commitment.target_domain);
            out.extend_from_slice(&commitment.message_id);
            out.extend_from_slice(&commitment.payload_hash);
            match commitment.parliament_certificate_hash {
                Some(hash) => {
                    Self::push_u8(&mut out, 1);
                    out.extend_from_slice(&hash);
                }
                None => Self::push_u8(&mut out, 0),
            }
            out
        }

        pub(crate) fn payload_hash(payload: &[u8]) -> MessageId {
            Self::prefixed_blake2(SCCP_PAYLOAD_HASH_PREFIX_V1, payload)
        }

        fn commitment_leaf_hash(commitment: &RuntimeHubCommitment) -> MessageId {
            Self::prefixed_blake2(
                SCCP_HUB_LEAF_PREFIX_V1,
                &Self::canonical_runtime_commitment_bytes(commitment),
            )
        }

        fn hash_merkle_node(left: &MessageId, right: &MessageId) -> MessageId {
            let mut out = Vec::with_capacity(left.len() + right.len());
            out.extend_from_slice(left);
            out.extend_from_slice(right);
            Self::prefixed_blake2(SCCP_HUB_NODE_PREFIX_V1, &out)
        }

        pub(crate) fn merkle_root_from_commitment(
            commitment: &RuntimeHubCommitment,
            proof: &RuntimeMerkleProof,
        ) -> MessageId {
            let mut current = Self::commitment_leaf_hash(commitment);
            for step in &proof.steps {
                current = if step.sibling_is_left {
                    Self::hash_merkle_node(&step.sibling_hash, &current)
                } else {
                    Self::hash_merkle_node(&current, &step.sibling_hash)
                };
            }
            current
        }

        fn prefixed_keccak(prefix: &[u8], payload: &[u8]) -> MessageId {
            let mut preimage = Vec::with_capacity(prefix.len().saturating_add(payload.len()));
            preimage.extend_from_slice(prefix);
            preimage.extend_from_slice(payload);
            keccak_256(&preimage)
        }

        fn prefixed_blake2(prefix: &[u8], payload: &[u8]) -> MessageId {
            let mut preimage = Vec::with_capacity(prefix.len().saturating_add(payload.len()));
            preimage.extend_from_slice(prefix);
            preimage.extend_from_slice(payload);
            blake2_256(&preimage)
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

        fn push_vec(out: &mut Vec<u8>, value: &[u8]) {
            Self::push_u32(out, value.len() as u32);
            out.extend_from_slice(value);
        }

        pub(crate) fn finalize_verified_inbound(
            message_id: MessageId,
            route_id: EncodedPayloadOf<T>,
            source_domain: DomainId,
            commitment_root: CommitmentRoot,
        ) -> DispatchResult {
            Self::ensure_inbound_can_finalize(message_id, &route_id, source_domain)?;
            let inbound_key = Self::inbound_message_key(message_id, &route_id, source_domain);
            ConsumedInboundMessages::<T>::insert(inbound_key.clone(), true);
            InboundCommitmentRoots::<T>::insert(inbound_key, commitment_root);
            Self::deposit_event(Event::InboundFinalized {
                message_id,
                route_id,
                source_domain,
                commitment_root,
            });
            Ok(())
        }

        fn ensure_inbound_can_finalize(
            message_id: MessageId,
            route_id: &EncodedPayloadOf<T>,
            source_domain: DomainId,
        ) -> Result<(), Error<T>> {
            Self::ensure_supported_domain(source_domain)?;
            let inbound_key = Self::inbound_message_key(message_id, route_id, source_domain);
            ensure!(
                !ConsumedInboundMessages::<T>::get(inbound_key),
                Error::<T>::MessageAlreadyConsumed
            );
            let route = Routes::<T>::get(route_id).ok_or(Error::<T>::RouteNotFound)?;
            ensure!(route.enabled, Error::<T>::RouteDisabled);
            ensure!(
                route.remote_domain == source_domain,
                Error::<T>::RouteDomainMismatch
            );
            let asset_record =
                RegistryAssets::<T>::get(&route.asset_id).ok_or(Error::<T>::AssetNotRegistered)?;
            ensure!(asset_record.enabled, Error::<T>::AssetDisabled);
            Ok(())
        }

        fn inbound_message_key(
            message_id: MessageId,
            route_id: &EncodedPayloadOf<T>,
            source_domain: DomainId,
        ) -> InboundMessageKeyOf<T> {
            InboundMessageKey {
                source_domain,
                route_id: route_id.clone(),
                message_id,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::RefCell;
    use frame_support::{assert_noop, assert_ok, construct_runtime, parameter_types, BoundedVec};
    use sp_core::H256;
    use sp_runtime::{
        traits::{BlakeTwo256, IdentityLookup},
        BuildStorage,
    };

    #[test]
    fn supported_domains_cover_requested_chain_matrix() {
        for domain in [
            SCCP_DOMAIN_SORA,
            SCCP_DOMAIN_ETH,
            SCCP_DOMAIN_BSC,
            SCCP_DOMAIN_SOL,
            SCCP_DOMAIN_TON,
            SCCP_DOMAIN_TRON,
            SCCP_DOMAIN_SORA2,
        ] {
            assert!(
                is_supported_domain(domain),
                "domain {domain} should be supported"
            );
        }
    }

    #[test]
    fn codec_validation_accepts_utf8_and_ton_raw_formats() {
        assert!(validate_payload_codec(
            SCCP_CODEC_TEXT_UTF8,
            b"nexus:ton:xor"
        ));
        assert!(validate_payload_codec(
            SCCP_CODEC_TON_RAW,
            b"0:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        ));
        assert!(validate_payload_codec(SCCP_CODEC_SORA_ASSET_ID, &[1; 32]));
    }

    #[test]
    fn codec_validation_enforces_canonical_evm_addresses() {
        assert!(validate_payload_codec(
            SCCP_CODEC_EVM_HEX,
            b"0xde709f2102306220921060314715629080e2fb77"
        ));
        assert!(validate_payload_codec(
            SCCP_CODEC_EVM_HEX,
            b"0x52908400098527886E0F7030069857D2E4169EE7"
        ));
        assert!(!validate_payload_codec(
            SCCP_CODEC_EVM_HEX,
            b"0x52908400098527886e0f7030069857d2e4169ee7"
        ));
        assert!(!validate_payload_codec(
            SCCP_CODEC_EVM_HEX,
            b"0x52908400098527886E0F7030069857D2E4169Ee7"
        ));
        assert!(!validate_payload_codec(
            SCCP_CODEC_EVM_HEX,
            b"0x52908400098527886E0F7030069857D2E4169EE"
        ));
    }

    #[test]
    fn codec_validation_enforces_solana_decoding_length() {
        assert!(validate_payload_codec(
            SCCP_CODEC_SOLANA_BASE58,
            b"11111111111111111111111111111111"
        ));
        assert!(!validate_payload_codec(
            SCCP_CODEC_SOLANA_BASE58,
            b"0OIl11111111111111111111111111111"
        ));
        assert!(!validate_payload_codec(
            SCCP_CODEC_SOLANA_BASE58,
            b"1111111111111111111111111111111"
        ));
    }

    #[test]
    fn codec_validation_enforces_tron_prefix_and_checksum() {
        assert!(validate_payload_codec(
            SCCP_CODEC_TRON_BASE58CHECK,
            b"T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb"
        ));
        assert!(!validate_payload_codec(
            SCCP_CODEC_TRON_BASE58CHECK,
            b"T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwc"
        ));
        assert!(!validate_payload_codec(
            SCCP_CODEC_TRON_BASE58CHECK,
            b"1BoatSLRHtKNngkdXEeobR76b53LETtpyT"
        ));
    }

    #[test]
    fn codec_validation_rejects_empty_utf8_and_non_raw_ton_formats() {
        assert!(!validate_payload_codec(SCCP_CODEC_TEXT_UTF8, &[]));
        assert!(!validate_payload_codec(
            SCCP_CODEC_TON_RAW,
            b"EQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAM9c"
        ));
        assert!(!validate_payload_codec(
            SCCP_CODEC_TON_RAW,
            b"+0:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        ));
        assert!(!validate_payload_codec(
            SCCP_CODEC_TON_RAW,
            b"00:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        ));
        assert!(!validate_payload_codec(
            SCCP_CODEC_TON_RAW,
            b"0:0123456789ABCDEF0123456789abcdef0123456789abcdef0123456789abcdef"
        ));
        assert!(!validate_payload_codec(SCCP_CODEC_SORA_ASSET_ID, &[1; 31]));
        assert!(!validate_payload_codec(SCCP_CODEC_SORA_ASSET_ID, &[1; 33]));
    }

    construct_runtime!(
        pub enum TestRuntime
        {
            System: frame_system,
            SccpBridge: pallet,
        }
    );

    parameter_types! {
        pub const BlockHashCount: u64 = 250;
        pub const MaxPayloadLen: u32 = 256;
        pub const MaxProofBlobLen: u32 = 4096;
        pub const AllowManualInboundFinalization: bool = false;
        pub const LocalDomain: DomainId = SCCP_DOMAIN_SORA2;
    }

    #[derive(Clone, Copy)]
    enum VerifierMode {
        Valid,
        Unavailable,
    }

    thread_local! {
        static VERIFIER_MODE: RefCell<VerifierMode> = const { RefCell::new(VerifierMode::Valid) };
    }

    type TestPayloadOf<T> = BoundedVec<u8, <T as pallet::Config>::MaxPayloadLen>;

    #[derive(Default)]
    pub struct MockVerifier;

    impl MessageProofVerifier for MockVerifier {
        fn verify_message_proof(
            proof_family: &[u8],
            verifier_backend: &[u8],
            proof_bytes: &[u8],
            public_inputs: &[u8],
            bundle_bytes: &[u8],
            message_id: &MessageId,
            route_id_codec: u8,
            route_id: &[u8],
            source_domain: DomainId,
            commitment_root: &CommitmentRoot,
        ) -> Result<(), MessageProofVerificationError> {
            if VERIFIER_MODE.with(|mode| matches!(*mode.borrow(), VerifierMode::Unavailable)) {
                return Err(MessageProofVerificationError::Unavailable);
            }
            if proof_family != b"stark-fri-v1"
                || verifier_backend != b"substrate-runtime-v1"
                || proof_bytes != b"proof"
                || public_inputs != b"public-inputs"
                || bundle_bytes != b"bundle"
                || message_id != &[0x11; 32]
                || route_id_codec != SCCP_CODEC_TEXT_UTF8
                || route_id != b"nexus:eth:xor"
                || source_domain != SCCP_DOMAIN_ETH
                || commitment_root != &[0x22; 32]
            {
                return Err(MessageProofVerificationError::Invalid);
            }
            Ok(())
        }
    }

    impl ControlMessageProofVerifier for MockVerifier {
        fn verify_control_message_proof(
            proof_family: &[u8],
            verifier_backend: &[u8],
            proof_bytes: &[u8],
            public_inputs: &[u8],
            bundle_bytes: &[u8],
            message_id: &MessageId,
            message_kind: ControlMessageKind,
            target_domain: DomainId,
            commitment_root: &CommitmentRoot,
        ) -> Result<(), MessageProofVerificationError> {
            if VERIFIER_MODE.with(|mode| matches!(*mode.borrow(), VerifierMode::Unavailable)) {
                return Err(MessageProofVerificationError::Unavailable);
            }
            if proof_family != b"stark-fri-v1"
                || verifier_backend != b"substrate-runtime-v1"
                || proof_bytes != b"proof"
                || public_inputs != b"public-inputs"
                || bundle_bytes != b"bundle"
                || target_domain != SCCP_DOMAIN_SORA2
                || commitment_root != &[0x33; 32]
            {
                return Err(MessageProofVerificationError::Invalid);
            }

            let expected_message_id = match message_kind {
                ControlMessageKind::AssetRegister => {
                    Pallet::<TestRuntime>::asset_register_message_id(
                        SCCP_DOMAIN_SORA2,
                        SCCP_DOMAIN_SORA,
                        7,
                        &encoded_payload::<TestRuntime>(SCCP_CODEC_TEXT_UTF8, b"xor#universal"),
                        18,
                    )
                }
                ControlMessageKind::RouteActivate => {
                    Pallet::<TestRuntime>::route_activate_message_id(
                        SCCP_DOMAIN_ETH,
                        SCCP_DOMAIN_SORA2,
                        8,
                        &encoded_payload::<TestRuntime>(SCCP_CODEC_TEXT_UTF8, b"xor#universal"),
                        &encoded_payload::<TestRuntime>(SCCP_CODEC_TEXT_UTF8, b"nexus:eth:xor"),
                    )
                }
                ControlMessageKind::TokenAdd
                | ControlMessageKind::TokenPause
                | ControlMessageKind::TokenResume => {
                    return Err(MessageProofVerificationError::Invalid)
                }
            };
            if *message_id != expected_message_id {
                return Err(MessageProofVerificationError::Invalid);
            }
            Ok(())
        }
    }

    impl frame_system::Config for TestRuntime {
        type BaseCallFilter = frame_support::traits::Everything;
        type BlockWeights = ();
        type BlockLength = ();
        type RuntimeOrigin = RuntimeOrigin;
        type RuntimeCall = RuntimeCall;
        type RuntimeEvent = RuntimeEvent;
        type RuntimeTask = ();
        type Nonce = u64;
        type Hash = H256;
        type Hashing = BlakeTwo256;
        type AccountId = u64;
        type Lookup = IdentityLookup<Self::AccountId>;
        type Block = frame_system::mocking::MockBlock<Self>;
        type BlockHashCount = BlockHashCount;
        type Version = ();
        type PalletInfo = PalletInfo;
        type AccountData = ();
        type OnNewAccount = ();
        type OnKilledAccount = ();
        type SystemWeightInfo = ();
        type ExtensionsWeightInfo = ();
        type DbWeight = ();
        type SS58Prefix = ();
        type OnSetCode = ();
        type MaxConsumers = frame_support::traits::ConstU32<16>;
        type SingleBlockMigrations = ();
        type MultiBlockMigrator = ();
        type PreInherents = ();
        type PostInherents = ();
        type PostTransactions = ();
    }

    impl pallet::Config for TestRuntime {
        type RuntimeEvent = RuntimeEvent;
        type MaxPayloadLen = MaxPayloadLen;
        type MaxProofBlobLen = MaxProofBlobLen;
        type AllowManualInboundFinalization = AllowManualInboundFinalization;
        type LocalDomain = LocalDomain;
        type WeightInfo = ();
        type MessageProofVerifier = MockVerifier;
        type ControlMessageProofVerifier = MockVerifier;
    }

    fn new_test_ext() -> sp_io::TestExternalities {
        frame_system::GenesisConfig::<TestRuntime>::default()
            .build_storage()
            .expect("build storage")
            .into()
    }

    fn encoded_payload<T: pallet::Config>(
        codec: u8,
        payload: &[u8],
    ) -> EncodedPayload<TestPayloadOf<T>> {
        EncodedPayload {
            codec,
            bytes: payload.to_vec().try_into().expect("bounded payload"),
        }
    }

    fn inbound_message_key<T: pallet::Config>(
        message_id: MessageId,
        route_id: &[u8],
        source_domain: DomainId,
    ) -> InboundMessageKey<TestPayloadOf<T>> {
        InboundMessageKey {
            source_domain,
            route_id: encoded_payload::<T>(SCCP_CODEC_TEXT_UTF8, route_id),
            message_id,
        }
    }

    fn seed_route<T: pallet::Config>(route_id: &[u8], remote_domain: DomainId) {
        let route_id = encoded_payload::<T>(SCCP_CODEC_TEXT_UTF8, route_id);
        let asset_id = encoded_payload::<T>(SCCP_CODEC_TEXT_UTF8, b"xor#universal");
        RegistryAssets::<T>::insert(
            asset_id.clone(),
            RegistryAssetRecord {
                home_domain: SCCP_DOMAIN_SORA,
                decimals: 18,
                asset_id: asset_id.clone(),
                enabled: true,
                sora_asset_id: None,
                name: None,
                symbol: None,
            },
        );
        Routes::<T>::insert(
            route_id,
            RouteRecord {
                asset_id,
                remote_domain,
                enabled: true,
            },
        );
    }

    const FINALITY_EPOCH: u64 = 7;
    const PARLIAMENT_EPOCH: u64 = 11;
    const VALIDATOR_SET_HASH: MessageId = [0xa1; 32];
    const ROSTER_HASH: MessageId = [0xb2; 32];
    const CERTIFICATE_HASH: MessageId = [0xc3; 32];

    fn seed_runtime_finality_anchor() {
        assert_ok!(SccpBridge::set_nexus_finality_anchor(
            RuntimeOrigin::root(),
            FINALITY_EPOCH,
            VALIDATOR_SET_HASH,
            3,
        ));
    }

    fn seed_runtime_parliament_anchor() {
        assert_ok!(SccpBridge::set_parliament_roster_anchor(
            RuntimeOrigin::root(),
            PARLIAMENT_EPOCH,
            ROSTER_HASH,
            2,
        ));
    }

    fn seed_runtime_anchors() {
        seed_runtime_finality_anchor();
        seed_runtime_parliament_anchor();
    }

    fn runtime_test_payload_kind(
        payload: &RuntimeProofPayload<TestPayloadOf<TestRuntime>>,
    ) -> RuntimeProofKind {
        match payload {
            RuntimeProofPayload::AssetRegister { .. } => RuntimeProofKind::AssetRegister,
            RuntimeProofPayload::RouteActivate { .. } => RuntimeProofKind::RouteActivate,
            RuntimeProofPayload::Transfer { .. } => RuntimeProofKind::Transfer,
            RuntimeProofPayload::TokenAdd { .. } => RuntimeProofKind::TokenAdd,
            RuntimeProofPayload::TokenPause { .. } => RuntimeProofKind::TokenPause,
            RuntimeProofPayload::TokenResume { .. } => RuntimeProofKind::TokenResume,
        }
    }

    fn runtime_test_payload_target_domain(
        payload: &RuntimeProofPayload<TestPayloadOf<TestRuntime>>,
    ) -> DomainId {
        match payload {
            RuntimeProofPayload::AssetRegister { target_domain, .. }
            | RuntimeProofPayload::RouteActivate { target_domain, .. }
            | RuntimeProofPayload::TokenAdd { target_domain, .. }
            | RuntimeProofPayload::TokenPause { target_domain, .. }
            | RuntimeProofPayload::TokenResume { target_domain, .. } => *target_domain,
            RuntimeProofPayload::Transfer { dest_domain, .. } => *dest_domain,
        }
    }

    fn runtime_bundle(
        payload: RuntimeProofPayload<TestPayloadOf<TestRuntime>>,
        require_parliament: bool,
    ) -> (MessageId, CommitmentRoot, Vec<u8>) {
        let message_id = Pallet::<TestRuntime>::runtime_payload_message_id(&payload);
        let payload_hash = Pallet::<TestRuntime>::payload_hash(
            &Pallet::<TestRuntime>::canonical_runtime_payload_bytes(&payload),
        );
        let commitment = RuntimeHubCommitment {
            version: 1,
            kind: runtime_test_payload_kind(&payload),
            target_domain: runtime_test_payload_target_domain(&payload),
            message_id,
            payload_hash,
            parliament_certificate_hash: require_parliament.then_some(CERTIFICATE_HASH),
        };
        let merkle_proof = RuntimeMerkleProof { steps: Vec::new() };
        let commitment_root =
            Pallet::<TestRuntime>::merkle_root_from_commitment(&commitment, &merkle_proof);
        let finality_proof = RuntimeFinalityProof {
            version: 1,
            epoch: FINALITY_EPOCH,
            height: 42,
            block_hash: [0xd4; 32],
            commitment_root,
            validator_set_hash: VALIDATOR_SET_HASH,
            signature_count: 3,
        };
        let parliament_certificate = require_parliament.then_some(RuntimeParliamentCertificate {
            version: 1,
            preimage_hash: payload_hash,
            enactment_window_start: 1,
            enactment_window_end: 100,
            roster_epoch: PARLIAMENT_EPOCH,
            roster_hash: ROSTER_HASH,
            required_signatures: 2,
            signature_count: 2,
            certificate_hash: CERTIFICATE_HASH,
        });
        let envelope = SccpRuntimeProofEnvelope {
            version: 1,
            commitment_root,
            commitment,
            merkle_proof,
            payload,
            finality_proof,
            parliament_certificate,
        };
        (message_id, commitment_root, envelope.encode())
    }

    fn runtime_proof_family() -> Vec<u8> {
        SCCP_RUNTIME_PROOF_FAMILY_V1.to_vec()
    }

    fn runtime_verifier_backend() -> Vec<u8> {
        SCCP_RUNTIME_VERIFIER_BACKEND_V1.to_vec()
    }

    #[test]
    fn import_registry_asset_rejects_duplicate_asset_id() {
        new_test_ext().execute_with(|| {
            assert_ok!(SccpBridge::import_registry_asset(
                RuntimeOrigin::root(),
                SCCP_CODEC_TEXT_UTF8,
                b"xor#universal".to_vec(),
                SCCP_DOMAIN_SORA,
                18,
            ));

            assert_noop!(
                SccpBridge::import_registry_asset(
                    RuntimeOrigin::root(),
                    SCCP_CODEC_TEXT_UTF8,
                    b"xor#universal".to_vec(),
                    SCCP_DOMAIN_ETH,
                    6,
                ),
                Error::<TestRuntime>::AssetAlreadyRegistered
            );

            let asset_id = encoded_payload::<TestRuntime>(SCCP_CODEC_TEXT_UTF8, b"xor#universal");
            let record = RegistryAssets::<TestRuntime>::get(asset_id).expect("asset record");
            assert_eq!(record.home_domain, SCCP_DOMAIN_SORA);
            assert_eq!(record.decimals, 18);
        });
    }

    #[test]
    fn activate_route_rejects_duplicate_route_id() {
        new_test_ext().execute_with(|| {
            assert_ok!(SccpBridge::import_registry_asset(
                RuntimeOrigin::root(),
                SCCP_CODEC_TEXT_UTF8,
                b"xor#universal".to_vec(),
                SCCP_DOMAIN_SORA,
                18,
            ));
            assert_ok!(SccpBridge::activate_route(
                RuntimeOrigin::root(),
                SCCP_CODEC_TEXT_UTF8,
                b"nexus:eth:xor".to_vec(),
                SCCP_CODEC_TEXT_UTF8,
                b"xor#universal".to_vec(),
                SCCP_DOMAIN_ETH,
            ));

            assert_noop!(
                SccpBridge::activate_route(
                    RuntimeOrigin::root(),
                    SCCP_CODEC_TEXT_UTF8,
                    b"nexus:eth:xor".to_vec(),
                    SCCP_CODEC_TEXT_UTF8,
                    b"xor#universal".to_vec(),
                    SCCP_DOMAIN_BSC,
                ),
                Error::<TestRuntime>::RouteAlreadyExists
            );

            let route_id = encoded_payload::<TestRuntime>(SCCP_CODEC_TEXT_UTF8, b"nexus:eth:xor");
            let route = Routes::<TestRuntime>::get(route_id).expect("route");
            assert_eq!(route.remote_domain, SCCP_DOMAIN_ETH);
        });
    }

    #[test]
    fn submit_asset_register_proof_imports_governance_authorized_asset() {
        new_test_ext().execute_with(|| {
            VERIFIER_MODE.with(|mode| *mode.borrow_mut() = VerifierMode::Valid);
            let asset_id = encoded_payload::<TestRuntime>(SCCP_CODEC_TEXT_UTF8, b"xor#universal");
            let message_id = Pallet::<TestRuntime>::asset_register_message_id(
                SCCP_DOMAIN_SORA2,
                SCCP_DOMAIN_SORA,
                7,
                &asset_id,
                18,
            );
            let commitment_root = [0x33; 32];

            assert_ok!(SccpBridge::submit_asset_register_proof(
                RuntimeOrigin::signed(1),
                b"stark-fri-v1".to_vec(),
                b"substrate-runtime-v1".to_vec(),
                b"proof".to_vec(),
                b"public-inputs".to_vec(),
                b"bundle".to_vec(),
                message_id,
                SCCP_DOMAIN_SORA2,
                SCCP_DOMAIN_SORA,
                7,
                SCCP_CODEC_TEXT_UTF8,
                b"xor#universal".to_vec(),
                18,
                commitment_root,
            ));

            let record = RegistryAssets::<TestRuntime>::get(asset_id).expect("asset record");
            assert_eq!(record.home_domain, SCCP_DOMAIN_SORA);
            assert_eq!(record.decimals, 18);
            assert!(ConsumedControlMessages::<TestRuntime>::get(message_id));
            let receipt =
                ControlProofReceipts::<TestRuntime>::get(message_id).expect("control receipt");
            assert_eq!(receipt.message_kind, ControlMessageKind::AssetRegister);
            assert_eq!(receipt.target_domain, SCCP_DOMAIN_SORA2);
            assert_eq!(receipt.commitment_root, commitment_root);
        });
    }

    #[test]
    fn submit_route_activate_proof_activates_governance_authorized_route() {
        new_test_ext().execute_with(|| {
            VERIFIER_MODE.with(|mode| *mode.borrow_mut() = VerifierMode::Valid);
            assert_ok!(SccpBridge::import_registry_asset(
                RuntimeOrigin::root(),
                SCCP_CODEC_TEXT_UTF8,
                b"xor#universal".to_vec(),
                SCCP_DOMAIN_SORA,
                18,
            ));
            let asset_id = encoded_payload::<TestRuntime>(SCCP_CODEC_TEXT_UTF8, b"xor#universal");
            let route_id = encoded_payload::<TestRuntime>(SCCP_CODEC_TEXT_UTF8, b"nexus:eth:xor");
            let message_id = Pallet::<TestRuntime>::route_activate_message_id(
                SCCP_DOMAIN_ETH,
                SCCP_DOMAIN_SORA2,
                8,
                &asset_id,
                &route_id,
            );
            let commitment_root = [0x33; 32];

            assert_ok!(SccpBridge::submit_route_activate_proof(
                RuntimeOrigin::signed(1),
                b"stark-fri-v1".to_vec(),
                b"substrate-runtime-v1".to_vec(),
                b"proof".to_vec(),
                b"public-inputs".to_vec(),
                b"bundle".to_vec(),
                message_id,
                SCCP_DOMAIN_ETH,
                SCCP_DOMAIN_SORA2,
                8,
                SCCP_CODEC_TEXT_UTF8,
                b"xor#universal".to_vec(),
                SCCP_CODEC_TEXT_UTF8,
                b"nexus:eth:xor".to_vec(),
                commitment_root,
            ));

            let route = Routes::<TestRuntime>::get(route_id).expect("route record");
            assert_eq!(route.remote_domain, SCCP_DOMAIN_ETH);
            assert!(route.enabled);
            assert!(ConsumedControlMessages::<TestRuntime>::get(message_id));
            let receipt =
                ControlProofReceipts::<TestRuntime>::get(message_id).expect("control receipt");
            assert_eq!(receipt.message_kind, ControlMessageKind::RouteActivate);
            assert_eq!(receipt.target_domain, SCCP_DOMAIN_SORA2);
            assert_eq!(receipt.commitment_root, commitment_root);
        });
    }

    #[test]
    fn submit_token_add_proof_imports_runtime_authorized_sora_asset() {
        new_test_ext().execute_with(|| {
            seed_runtime_anchors();
            let sora_asset_id = [0x10; 32];
            let name = [0x11; 32];
            let symbol = [0x12; 32];
            let (message_id, commitment_root, bundle_bytes) = runtime_bundle(
                RuntimeProofPayload::TokenAdd {
                    target_domain: SCCP_DOMAIN_SORA2,
                    nonce: 41,
                    sora_asset_id,
                    decimals: 18,
                    name,
                    symbol,
                },
                true,
            );

            assert_ok!(SccpBridge::submit_token_add_proof(
                RuntimeOrigin::signed(1),
                runtime_proof_family(),
                runtime_verifier_backend(),
                Vec::new(),
                Vec::new(),
                bundle_bytes,
                message_id,
                SCCP_DOMAIN_SORA2,
                41,
                sora_asset_id,
                18,
                name,
                symbol,
                commitment_root,
            ));

            let asset_id = encoded_payload::<TestRuntime>(SCCP_CODEC_SORA_ASSET_ID, &sora_asset_id);
            let record = RegistryAssets::<TestRuntime>::get(asset_id).expect("asset record");
            assert_eq!(record.home_domain, SCCP_DOMAIN_SORA2);
            assert_eq!(record.decimals, 18);
            assert!(record.enabled);
            assert_eq!(record.sora_asset_id, Some(sora_asset_id));
            assert_eq!(record.name, Some(name));
            assert_eq!(record.symbol, Some(symbol));
            assert!(ConsumedControlMessages::<TestRuntime>::get(message_id));
            let receipt =
                ControlProofReceipts::<TestRuntime>::get(message_id).expect("control receipt");
            assert_eq!(receipt.message_kind, ControlMessageKind::TokenAdd);
            assert_eq!(receipt.commitment_root, commitment_root);
        });
    }

    #[test]
    fn token_pause_and_resume_toggle_asset_level_bridge_access() {
        new_test_ext().execute_with(|| {
            seed_runtime_anchors();
            let sora_asset_id = [0x20; 32];
            let name = [0x21; 32];
            let symbol = [0x22; 32];
            let (add_message_id, add_root, add_bundle) = runtime_bundle(
                RuntimeProofPayload::TokenAdd {
                    target_domain: SCCP_DOMAIN_SORA2,
                    nonce: 50,
                    sora_asset_id,
                    decimals: 18,
                    name,
                    symbol,
                },
                true,
            );
            assert_ok!(SccpBridge::submit_token_add_proof(
                RuntimeOrigin::signed(1),
                runtime_proof_family(),
                runtime_verifier_backend(),
                Vec::new(),
                Vec::new(),
                add_bundle,
                add_message_id,
                SCCP_DOMAIN_SORA2,
                50,
                sora_asset_id,
                18,
                name,
                symbol,
                add_root,
            ));
            assert_ok!(SccpBridge::activate_route(
                RuntimeOrigin::root(),
                SCCP_CODEC_TEXT_UTF8,
                b"nexus:eth:xor".to_vec(),
                SCCP_CODEC_SORA_ASSET_ID,
                sora_asset_id.to_vec(),
                SCCP_DOMAIN_ETH,
            ));

            let (pause_message_id, pause_root, pause_bundle) = runtime_bundle(
                RuntimeProofPayload::TokenPause {
                    target_domain: SCCP_DOMAIN_SORA2,
                    nonce: 51,
                    sora_asset_id,
                },
                true,
            );
            assert_ok!(SccpBridge::submit_token_pause_proof(
                RuntimeOrigin::signed(1),
                runtime_proof_family(),
                runtime_verifier_backend(),
                Vec::new(),
                Vec::new(),
                pause_bundle,
                pause_message_id,
                SCCP_DOMAIN_SORA2,
                51,
                sora_asset_id,
                pause_root,
            ));
            let asset_id = encoded_payload::<TestRuntime>(SCCP_CODEC_SORA_ASSET_ID, &sora_asset_id);
            assert!(
                !RegistryAssets::<TestRuntime>::get(asset_id.clone())
                    .expect("asset")
                    .enabled
            );

            assert_eq!(
                SccpBridge::record_outbound_message(
                    encoded_payload::<TestRuntime>(SCCP_CODEC_TEXT_UTF8, b"nexus:eth:xor"),
                    SCCP_DOMAIN_SORA2,
                    SCCP_DOMAIN_ETH,
                    SCCP_DOMAIN_SORA2,
                    1,
                    encoded_payload::<TestRuntime>(SCCP_CODEC_TEXT_UTF8, b"alice"),
                    encoded_payload::<TestRuntime>(SCCP_CODEC_TEXT_UTF8, b"bob"),
                ),
                Err(Error::<TestRuntime>::AssetDisabled.into())
            );
            assert_noop!(
                SccpBridge::submit_message_proof(
                    RuntimeOrigin::signed(1),
                    b"stark-fri-v1".to_vec(),
                    b"substrate-runtime-v1".to_vec(),
                    b"proof".to_vec(),
                    b"public-inputs".to_vec(),
                    b"bundle".to_vec(),
                    [0x11; 32],
                    SCCP_CODEC_TEXT_UTF8,
                    b"nexus:eth:xor".to_vec(),
                    SCCP_DOMAIN_ETH,
                    [0x22; 32],
                ),
                Error::<TestRuntime>::AssetDisabled
            );

            let (resume_message_id, resume_root, resume_bundle) = runtime_bundle(
                RuntimeProofPayload::TokenResume {
                    target_domain: SCCP_DOMAIN_SORA2,
                    nonce: 52,
                    sora_asset_id,
                },
                true,
            );
            assert_ok!(SccpBridge::submit_token_resume_proof(
                RuntimeOrigin::signed(1),
                runtime_proof_family(),
                runtime_verifier_backend(),
                Vec::new(),
                Vec::new(),
                resume_bundle,
                resume_message_id,
                SCCP_DOMAIN_SORA2,
                52,
                sora_asset_id,
                resume_root,
            ));
            assert!(
                RegistryAssets::<TestRuntime>::get(asset_id)
                    .expect("asset")
                    .enabled
            );
            assert_ok!(SccpBridge::record_outbound_message(
                encoded_payload::<TestRuntime>(SCCP_CODEC_TEXT_UTF8, b"nexus:eth:xor"),
                SCCP_DOMAIN_SORA2,
                SCCP_DOMAIN_ETH,
                SCCP_DOMAIN_SORA2,
                1,
                encoded_payload::<TestRuntime>(SCCP_CODEC_TEXT_UTF8, b"alice"),
                encoded_payload::<TestRuntime>(SCCP_CODEC_TEXT_UTF8, b"bob"),
            ));
        });
    }

    #[test]
    fn runtime_control_proof_rejects_missing_anchors() {
        new_test_ext().execute_with(|| {
            let sora_asset_id = [0x30; 32];
            let name = [0x31; 32];
            let symbol = [0x32; 32];
            let (message_id, commitment_root, bundle_bytes) = runtime_bundle(
                RuntimeProofPayload::TokenAdd {
                    target_domain: SCCP_DOMAIN_SORA2,
                    nonce: 60,
                    sora_asset_id,
                    decimals: 18,
                    name,
                    symbol,
                },
                true,
            );

            assert_noop!(
                SccpBridge::submit_token_add_proof(
                    RuntimeOrigin::signed(1),
                    runtime_proof_family(),
                    runtime_verifier_backend(),
                    Vec::new(),
                    Vec::new(),
                    bundle_bytes.clone(),
                    message_id,
                    SCCP_DOMAIN_SORA2,
                    60,
                    sora_asset_id,
                    18,
                    name,
                    symbol,
                    commitment_root,
                ),
                Error::<TestRuntime>::MissingNexusFinalityAnchor
            );

            seed_runtime_finality_anchor();
            assert_noop!(
                SccpBridge::submit_token_add_proof(
                    RuntimeOrigin::signed(1),
                    runtime_proof_family(),
                    runtime_verifier_backend(),
                    Vec::new(),
                    Vec::new(),
                    bundle_bytes,
                    message_id,
                    SCCP_DOMAIN_SORA2,
                    60,
                    sora_asset_id,
                    18,
                    name,
                    symbol,
                    commitment_root,
                ),
                Error::<TestRuntime>::MissingParliamentRosterAnchor
            );
        });
    }

    #[test]
    fn runtime_transfer_proof_finalizes_inbound_without_parliament_certificate() {
        new_test_ext().execute_with(|| {
            seed_runtime_finality_anchor();
            seed_route::<TestRuntime>(b"nexus:eth:xor", SCCP_DOMAIN_ETH);
            let route_id = encoded_payload::<TestRuntime>(SCCP_CODEC_TEXT_UTF8, b"nexus:eth:xor");
            let (message_id, commitment_root, bundle_bytes) = runtime_bundle(
                RuntimeProofPayload::Transfer {
                    source_domain: SCCP_DOMAIN_ETH,
                    dest_domain: SCCP_DOMAIN_SORA2,
                    nonce: 70,
                    asset_home_domain: SCCP_DOMAIN_SORA,
                    asset_id: encoded_payload::<TestRuntime>(
                        SCCP_CODEC_TEXT_UTF8,
                        b"xor#universal",
                    ),
                    amount: 1,
                    sender: encoded_payload::<TestRuntime>(SCCP_CODEC_TEXT_UTF8, b"alice"),
                    recipient: encoded_payload::<TestRuntime>(SCCP_CODEC_TEXT_UTF8, b"bob"),
                    route_id: route_id.clone(),
                },
                false,
            );

            assert_ok!(SccpBridge::submit_message_proof(
                RuntimeOrigin::signed(1),
                runtime_proof_family(),
                runtime_verifier_backend(),
                Vec::new(),
                Vec::new(),
                bundle_bytes,
                message_id,
                SCCP_CODEC_TEXT_UTF8,
                b"nexus:eth:xor".to_vec(),
                SCCP_DOMAIN_ETH,
                commitment_root,
            ));
            assert!(ConsumedInboundMessages::<TestRuntime>::get(
                inbound_message_key::<TestRuntime>(message_id, b"nexus:eth:xor", SCCP_DOMAIN_ETH)
            ));
        });
    }

    #[test]
    fn submit_control_proof_rejects_wrong_target_domain() {
        new_test_ext().execute_with(|| {
            let asset_id = encoded_payload::<TestRuntime>(SCCP_CODEC_TEXT_UTF8, b"xor#universal");
            let message_id = Pallet::<TestRuntime>::asset_register_message_id(
                SCCP_DOMAIN_SORA2,
                SCCP_DOMAIN_SORA,
                7,
                &asset_id,
                18,
            );

            assert_noop!(
                SccpBridge::submit_asset_register_proof(
                    RuntimeOrigin::signed(1),
                    b"stark-fri-v1".to_vec(),
                    b"substrate-runtime-v1".to_vec(),
                    b"proof".to_vec(),
                    b"public-inputs".to_vec(),
                    b"bundle".to_vec(),
                    message_id,
                    SCCP_DOMAIN_SORA,
                    SCCP_DOMAIN_SORA,
                    7,
                    SCCP_CODEC_TEXT_UTF8,
                    b"xor#universal".to_vec(),
                    18,
                    [0x33; 32],
                ),
                Error::<TestRuntime>::TargetDomainMismatch
            );
        });
    }

    #[test]
    fn submit_control_proof_rejects_message_id_mismatch() {
        new_test_ext().execute_with(|| {
            assert_noop!(
                SccpBridge::submit_asset_register_proof(
                    RuntimeOrigin::signed(1),
                    b"stark-fri-v1".to_vec(),
                    b"substrate-runtime-v1".to_vec(),
                    b"proof".to_vec(),
                    b"public-inputs".to_vec(),
                    b"bundle".to_vec(),
                    [0x99; 32],
                    SCCP_DOMAIN_SORA2,
                    SCCP_DOMAIN_SORA,
                    7,
                    SCCP_CODEC_TEXT_UTF8,
                    b"xor#universal".to_vec(),
                    18,
                    [0x33; 32],
                ),
                Error::<TestRuntime>::MessageIdMismatch
            );
        });
    }

    #[test]
    fn submit_message_proof_records_receipt_and_finalizes_inbound() {
        new_test_ext().execute_with(|| {
            VERIFIER_MODE.with(|mode| *mode.borrow_mut() = VerifierMode::Valid);
            seed_route::<TestRuntime>(b"nexus:eth:xor", SCCP_DOMAIN_ETH);
            let message_id = [0x11; 32];
            let commitment_root = [0x22; 32];

            assert_ok!(SccpBridge::submit_message_proof(
                RuntimeOrigin::signed(1),
                b"stark-fri-v1".to_vec(),
                b"substrate-runtime-v1".to_vec(),
                b"proof".to_vec(),
                b"public-inputs".to_vec(),
                b"bundle".to_vec(),
                message_id,
                SCCP_CODEC_TEXT_UTF8,
                b"nexus:eth:xor".to_vec(),
                SCCP_DOMAIN_ETH,
                commitment_root,
            ));

            let inbound_key =
                inbound_message_key::<TestRuntime>(message_id, b"nexus:eth:xor", SCCP_DOMAIN_ETH);
            let receipt = InboundProofReceipts::<TestRuntime>::get(inbound_key.clone())
                .expect("proof receipt");
            assert_eq!(receipt.proof_family.as_slice(), b"stark-fri-v1");
            assert_eq!(receipt.verifier_backend.as_slice(), b"substrate-runtime-v1");
            assert_eq!(receipt.route_id.codec, SCCP_CODEC_TEXT_UTF8);
            assert_eq!(receipt.route_id.bytes.as_slice(), b"nexus:eth:xor");
            assert!(ConsumedInboundMessages::<TestRuntime>::get(
                inbound_key.clone()
            ));
            assert_eq!(
                InboundCommitmentRoots::<TestRuntime>::get(inbound_key),
                Some(commitment_root)
            );
        });
    }

    #[test]
    fn submit_message_proof_fails_closed_when_verifier_is_unavailable() {
        new_test_ext().execute_with(|| {
            VERIFIER_MODE.with(|mode| *mode.borrow_mut() = VerifierMode::Unavailable);
            seed_route::<TestRuntime>(b"nexus:eth:xor", SCCP_DOMAIN_ETH);

            assert_noop!(
                SccpBridge::submit_message_proof(
                    RuntimeOrigin::signed(1),
                    b"stark-fri-v1".to_vec(),
                    b"substrate-runtime-v1".to_vec(),
                    b"proof".to_vec(),
                    b"public-inputs".to_vec(),
                    b"bundle".to_vec(),
                    [0x11; 32],
                    SCCP_CODEC_TEXT_UTF8,
                    b"nexus:eth:xor".to_vec(),
                    SCCP_DOMAIN_ETH,
                    [0x22; 32],
                ),
                Error::<TestRuntime>::ProofVerifierUnavailable
            );
        });
    }

    #[test]
    fn submit_message_proof_rejects_tampered_inputs() {
        new_test_ext().execute_with(|| {
            VERIFIER_MODE.with(|mode| *mode.borrow_mut() = VerifierMode::Valid);
            seed_route::<TestRuntime>(b"nexus:eth:xor", SCCP_DOMAIN_ETH);

            assert_noop!(
                SccpBridge::submit_message_proof(
                    RuntimeOrigin::signed(1),
                    b"stark-fri-v1".to_vec(),
                    b"substrate-runtime-v1".to_vec(),
                    b"proof".to_vec(),
                    b"tampered".to_vec(),
                    b"bundle".to_vec(),
                    [0x11; 32],
                    SCCP_CODEC_TEXT_UTF8,
                    b"nexus:eth:xor".to_vec(),
                    SCCP_DOMAIN_ETH,
                    [0x22; 32],
                ),
                Error::<TestRuntime>::InvalidMessageProof
            );
        });
    }

    #[test]
    fn manual_inbound_finalization_is_disabled_by_default() {
        new_test_ext().execute_with(|| {
            seed_route::<TestRuntime>(b"nexus:eth:xor", SCCP_DOMAIN_ETH);

            assert_noop!(
                SccpBridge::finalize_inbound(
                    RuntimeOrigin::root(),
                    [0x11; 32],
                    SCCP_CODEC_TEXT_UTF8,
                    b"nexus:eth:xor".to_vec(),
                    SCCP_DOMAIN_ETH,
                    [0x22; 32],
                ),
                Error::<TestRuntime>::ManualInboundFinalizationDisabled
            );
        });
    }

    #[test]
    fn submit_message_proof_rejects_paused_routes() {
        new_test_ext().execute_with(|| {
            VERIFIER_MODE.with(|mode| *mode.borrow_mut() = VerifierMode::Valid);
            seed_route::<TestRuntime>(b"nexus:eth:xor", SCCP_DOMAIN_ETH);
            assert_ok!(SccpBridge::pause_route(
                RuntimeOrigin::root(),
                SCCP_CODEC_TEXT_UTF8,
                b"nexus:eth:xor".to_vec(),
            ));

            assert_noop!(
                SccpBridge::submit_message_proof(
                    RuntimeOrigin::signed(1),
                    b"stark-fri-v1".to_vec(),
                    b"substrate-runtime-v1".to_vec(),
                    b"proof".to_vec(),
                    b"public-inputs".to_vec(),
                    b"bundle".to_vec(),
                    [0x11; 32],
                    SCCP_CODEC_TEXT_UTF8,
                    b"nexus:eth:xor".to_vec(),
                    SCCP_DOMAIN_ETH,
                    [0x22; 32],
                ),
                Error::<TestRuntime>::RouteDisabled
            );
        });
    }

    #[test]
    fn submit_message_proof_rejects_oversized_proof_blobs() {
        new_test_ext().execute_with(|| {
            VERIFIER_MODE.with(|mode| *mode.borrow_mut() = VerifierMode::Valid);
            seed_route::<TestRuntime>(b"nexus:eth:xor", SCCP_DOMAIN_ETH);

            assert_noop!(
                SccpBridge::submit_message_proof(
                    RuntimeOrigin::signed(1),
                    b"stark-fri-v1".to_vec(),
                    b"substrate-runtime-v1".to_vec(),
                    vec![b'p'; (MaxProofBlobLen::get() + 1) as usize],
                    b"public-inputs".to_vec(),
                    b"bundle".to_vec(),
                    [0x11; 32],
                    SCCP_CODEC_TEXT_UTF8,
                    b"nexus:eth:xor".to_vec(),
                    SCCP_DOMAIN_ETH,
                    [0x22; 32],
                ),
                Error::<TestRuntime>::PayloadTooLarge
            );

            assert_noop!(
                SccpBridge::submit_message_proof(
                    RuntimeOrigin::signed(1),
                    b"stark-fri-v1".to_vec(),
                    b"substrate-runtime-v1".to_vec(),
                    b"proof".to_vec(),
                    vec![b'i'; (MaxProofBlobLen::get() + 1) as usize],
                    b"bundle".to_vec(),
                    [0x11; 32],
                    SCCP_CODEC_TEXT_UTF8,
                    b"nexus:eth:xor".to_vec(),
                    SCCP_DOMAIN_ETH,
                    [0x22; 32],
                ),
                Error::<TestRuntime>::PayloadTooLarge
            );

            assert_noop!(
                SccpBridge::submit_message_proof(
                    RuntimeOrigin::signed(1),
                    b"stark-fri-v1".to_vec(),
                    b"substrate-runtime-v1".to_vec(),
                    b"proof".to_vec(),
                    b"public-inputs".to_vec(),
                    vec![b'b'; (MaxProofBlobLen::get() + 1) as usize],
                    [0x11; 32],
                    SCCP_CODEC_TEXT_UTF8,
                    b"nexus:eth:xor".to_vec(),
                    SCCP_DOMAIN_ETH,
                    [0x22; 32],
                ),
                Error::<TestRuntime>::PayloadTooLarge
            );
        });
    }

    #[test]
    fn same_message_id_can_finalize_on_different_route_domain_pairs() {
        new_test_ext().execute_with(|| {
            let message_id = [0x33; 32];
            let eth_root = [0x44; 32];
            let bsc_root = [0x55; 32];
            let eth_route = encoded_payload::<TestRuntime>(SCCP_CODEC_TEXT_UTF8, b"nexus:eth:xor");
            let bsc_route = encoded_payload::<TestRuntime>(SCCP_CODEC_TEXT_UTF8, b"nexus:bsc:xor");

            seed_route::<TestRuntime>(b"nexus:eth:xor", SCCP_DOMAIN_ETH);
            seed_route::<TestRuntime>(b"nexus:bsc:xor", SCCP_DOMAIN_BSC);

            assert_ok!(SccpBridge::finalize_verified_inbound(
                message_id,
                eth_route,
                SCCP_DOMAIN_ETH,
                eth_root,
            ));
            assert_ok!(SccpBridge::finalize_verified_inbound(
                message_id,
                bsc_route,
                SCCP_DOMAIN_BSC,
                bsc_root,
            ));

            assert!(ConsumedInboundMessages::<TestRuntime>::get(
                inbound_message_key::<TestRuntime>(message_id, b"nexus:eth:xor", SCCP_DOMAIN_ETH)
            ));
            assert!(ConsumedInboundMessages::<TestRuntime>::get(
                inbound_message_key::<TestRuntime>(message_id, b"nexus:bsc:xor", SCCP_DOMAIN_BSC)
            ));
        });
    }

    #[test]
    fn replay_is_rejected_for_the_same_composite_inbound_key() {
        new_test_ext().execute_with(|| {
            let message_id = [0x66; 32];
            let route = encoded_payload::<TestRuntime>(SCCP_CODEC_TEXT_UTF8, b"nexus:eth:xor");

            seed_route::<TestRuntime>(b"nexus:eth:xor", SCCP_DOMAIN_ETH);

            assert_ok!(SccpBridge::finalize_verified_inbound(
                message_id,
                route.clone(),
                SCCP_DOMAIN_ETH,
                [0x77; 32],
            ));
            assert_noop!(
                SccpBridge::finalize_verified_inbound(
                    message_id,
                    route,
                    SCCP_DOMAIN_ETH,
                    [0x88; 32],
                ),
                Error::<TestRuntime>::MessageAlreadyConsumed
            );
        });
    }

    #[test]
    fn commitment_roots_are_tracked_per_message_on_same_route() {
        new_test_ext().execute_with(|| {
            let message_id_a = [0x90; 32];
            let message_id_b = [0x91; 32];
            let root_a = [0xa0; 32];
            let root_b = [0xb0; 32];

            seed_route::<TestRuntime>(b"nexus:eth:xor", SCCP_DOMAIN_ETH);

            assert_ok!(SccpBridge::finalize_verified_inbound(
                message_id_a,
                encoded_payload::<TestRuntime>(SCCP_CODEC_TEXT_UTF8, b"nexus:eth:xor"),
                SCCP_DOMAIN_ETH,
                root_a,
            ));
            assert_ok!(SccpBridge::finalize_verified_inbound(
                message_id_b,
                encoded_payload::<TestRuntime>(SCCP_CODEC_TEXT_UTF8, b"nexus:eth:xor"),
                SCCP_DOMAIN_ETH,
                root_b,
            ));

            assert_eq!(
                InboundCommitmentRoots::<TestRuntime>::get(inbound_message_key::<TestRuntime>(
                    message_id_a,
                    b"nexus:eth:xor",
                    SCCP_DOMAIN_ETH,
                )),
                Some(root_a)
            );
            assert_eq!(
                InboundCommitmentRoots::<TestRuntime>::get(inbound_message_key::<TestRuntime>(
                    message_id_b,
                    b"nexus:eth:xor",
                    SCCP_DOMAIN_ETH,
                )),
                Some(root_b)
            );
        });
    }

    #[test]
    fn record_outbound_message_is_available_for_internal_callers() {
        new_test_ext().execute_with(|| {
            seed_route::<TestRuntime>(b"nexus:eth:xor", SCCP_DOMAIN_ETH);

            let nonce = SccpBridge::record_outbound_message(
                encoded_payload::<TestRuntime>(SCCP_CODEC_TEXT_UTF8, b"nexus:eth:xor"),
                SCCP_DOMAIN_SORA,
                SCCP_DOMAIN_ETH,
                SCCP_DOMAIN_SORA,
                1,
                encoded_payload::<TestRuntime>(SCCP_CODEC_TEXT_UTF8, b"alice"),
                encoded_payload::<TestRuntime>(SCCP_CODEC_TEXT_UTF8, b"bob"),
            )
            .expect("record outbound message");

            assert_eq!(nonce, 0);
            assert_eq!(NextOutboundNonce::<TestRuntime>::get(), 1);
            assert!(OutboundMessages::<TestRuntime>::get(nonce).is_some());
        });
    }

    #[test]
    fn record_outbound_message_revalidates_internal_payloads() {
        new_test_ext().execute_with(|| {
            seed_route::<TestRuntime>(b"nexus:eth:xor", SCCP_DOMAIN_ETH);
            let non_canonical_sender = EncodedPayload {
                codec: SCCP_CODEC_EVM_HEX,
                bytes: b"0x52908400098527886e0f7030069857d2e4169ee7"
                    .to_vec()
                    .try_into()
                    .expect("bounded payload"),
            };

            assert_eq!(
                SccpBridge::record_outbound_message(
                    encoded_payload::<TestRuntime>(SCCP_CODEC_TEXT_UTF8, b"nexus:eth:xor"),
                    SCCP_DOMAIN_SORA,
                    SCCP_DOMAIN_ETH,
                    SCCP_DOMAIN_SORA,
                    1,
                    non_canonical_sender,
                    encoded_payload::<TestRuntime>(SCCP_CODEC_TEXT_UTF8, b"bob"),
                ),
                Err(Error::<TestRuntime>::InvalidPayloadFormat.into())
            );
            assert_eq!(NextOutboundNonce::<TestRuntime>::get(), 0);
            assert!(OutboundMessages::<TestRuntime>::get(0).is_none());
        });
    }

    #[test]
    fn record_outbound_message_rejects_route_asset_home_mismatch() {
        new_test_ext().execute_with(|| {
            seed_route::<TestRuntime>(b"nexus:eth:xor", SCCP_DOMAIN_ETH);

            assert_eq!(
                SccpBridge::record_outbound_message(
                    encoded_payload::<TestRuntime>(SCCP_CODEC_TEXT_UTF8, b"nexus:eth:xor"),
                    SCCP_DOMAIN_SORA,
                    SCCP_DOMAIN_ETH,
                    SCCP_DOMAIN_ETH,
                    1,
                    encoded_payload::<TestRuntime>(SCCP_CODEC_TEXT_UTF8, b"alice"),
                    encoded_payload::<TestRuntime>(SCCP_CODEC_TEXT_UTF8, b"bob"),
                ),
                Err(Error::<TestRuntime>::AssetHomeDomainMismatch.into())
            );
            assert_eq!(NextOutboundNonce::<TestRuntime>::get(), 0);
            assert!(OutboundMessages::<TestRuntime>::get(0).is_none());
        });
    }

    #[test]
    fn record_outbound_fails_closed_on_nonce_overflow() {
        new_test_ext().execute_with(|| {
            seed_route::<TestRuntime>(b"nexus:eth:xor", SCCP_DOMAIN_ETH);
            NextOutboundNonce::<TestRuntime>::put(Nonce::MAX);

            assert_noop!(
                SccpBridge::record_outbound(
                    RuntimeOrigin::root(),
                    SCCP_CODEC_TEXT_UTF8,
                    b"nexus:eth:xor".to_vec(),
                    SCCP_DOMAIN_SORA,
                    SCCP_DOMAIN_ETH,
                    SCCP_DOMAIN_SORA,
                    1,
                    SCCP_CODEC_TEXT_UTF8,
                    b"alice".to_vec(),
                    SCCP_CODEC_TEXT_UTF8,
                    b"bob".to_vec(),
                ),
                Error::<TestRuntime>::NonceOverflow
            );
            assert!(OutboundMessages::<TestRuntime>::get(Nonce::MAX).is_none());
            assert_eq!(NextOutboundNonce::<TestRuntime>::get(), Nonce::MAX);
        });
    }
}
