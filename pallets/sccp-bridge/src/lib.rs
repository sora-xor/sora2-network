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

use base58::FromBase58;
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame_support::{dispatch::DispatchResult, pallet_prelude::*, traits::StorageVersion};
use frame_system::{ensure_signed, pallet_prelude::*};
use scale_info::TypeInfo;
use sp_io::hashing::{keccak_256, sha2_256};
use sp_runtime::RuntimeDebug;
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
pub struct RouteCommitmentKey<Payload> {
    pub source_domain: DomainId,
    pub route_id: EncodedPayload<Payload>,
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

    let has_uppercase = payload.iter().any(u8::is_ascii_uppercase);
    if !has_uppercase {
        return payload
            .iter()
            .all(|byte| byte.is_ascii_digit() || byte.is_ascii_lowercase());
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

        if should_be_uppercase != byte.is_ascii_uppercase() {
            return false;
        }
    }

    true
}

fn validate_ton_raw_codec(bytes: &[u8]) -> bool {
    let Ok(value) = core::str::from_utf8(bytes) else {
        return false;
    };
    let Some((workchain, account)) = value.split_once(':') else {
        return false;
    };
    !workchain.is_empty()
        && workchain.parse::<i32>().is_ok()
        && account.len() == 64
        && account
            .as_bytes()
            .iter()
            .copied()
            .all(|byte| decode_hex_nibble(byte).is_some())
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

fn validate_payload_codec(codec_id: u8, bytes: &[u8]) -> bool {
    match codec_id {
        SCCP_CODEC_TEXT_UTF8 => validate_utf8_codec(bytes),
        SCCP_CODEC_EVM_HEX => validate_evm_hex_codec(bytes),
        SCCP_CODEC_SOLANA_BASE58 => validate_solana_base58_codec(bytes),
        SCCP_CODEC_TON_RAW => validate_ton_raw_codec(bytes),
        SCCP_CODEC_TRON_BASE58CHECK => validate_tron_base58_codec(bytes),
        _ => false,
    }
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_system::ensure_root;

    type PayloadOf<T> = BoundedVec<u8, <T as Config>::MaxPayloadLen>;
    type ProofBlobOf<T> = BoundedVec<u8, <T as Config>::MaxProofBlobLen>;
    type EncodedPayloadOf<T> = EncodedPayload<PayloadOf<T>>;
    type RegistryAssetRecordOf<T> = RegistryAssetRecord<PayloadOf<T>>;
    type RouteRecordOf<T> = RouteRecord<PayloadOf<T>>;
    type OutboundMessageRecordOf<T> = OutboundMessageRecord<PayloadOf<T>>;
    type MessageProofReceiptOf<T> = MessageProofReceipt<PayloadOf<T>>;
    type InboundMessageKeyOf<T> = InboundMessageKey<PayloadOf<T>>;
    type RouteCommitmentKeyOf<T> = RouteCommitmentKey<PayloadOf<T>>;

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

        type WeightInfo: WeightInfo;
        type MessageProofVerifier: MessageProofVerifier;
    }

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(2);

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
    }

    #[pallet::error]
    pub enum Error<T> {
        PayloadTooLarge,
        UnsupportedDomain,
        UnsupportedCodec,
        InvalidPayloadFormat,
        AssetNotRegistered,
        RouteNotFound,
        RouteDisabled,
        RouteDomainMismatch,
        MessageAlreadyConsumed,
        ZeroAmount,
        ManualInboundFinalizationDisabled,
        ProofVerifierUnavailable,
        InvalidMessageProof,
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
    #[pallet::getter(fn latest_commitment_roots)]
    pub type LatestCommitmentRoots<T: Config> =
        StorageMap<_, Blake2_128Concat, RouteCommitmentKeyOf<T>, CommitmentRoot, OptionQuery>;

    #[pallet::call]
    impl<T: Config> Pallet<T> {
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
            RegistryAssets::<T>::insert(
                asset_id.clone(),
                RegistryAssetRecord {
                    home_domain,
                    decimals,
                    asset_id: asset_id.clone(),
                },
            );
            Self::deposit_event(Event::RegistryAssetImported {
                asset_id,
                home_domain,
                decimals,
            });
            Ok(())
        }

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

        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::pause_route())]
        pub fn pause_route(
            origin: OriginFor<T>,
            route_id_codec: u8,
            route_id: Vec<u8>,
        ) -> DispatchResult {
            ensure_root(origin)?;
            let route_id = Self::bounded_payload(route_id_codec, route_id)?;
            Routes::<T>::try_mutate(&route_id, |maybe_route| -> DispatchResult {
                let route = maybe_route.as_mut().ok_or(Error::<T>::RouteNotFound)?;
                route.enabled = false;
                Ok(())
            })?;
            Self::deposit_event(Event::RoutePaused { route_id });
            Ok(())
        }

        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::resume_route())]
        pub fn resume_route(
            origin: OriginFor<T>,
            route_id_codec: u8,
            route_id: Vec<u8>,
        ) -> DispatchResult {
            ensure_root(origin)?;
            let route_id = Self::bounded_payload(route_id_codec, route_id)?;
            Routes::<T>::try_mutate(&route_id, |maybe_route| -> DispatchResult {
                let route = maybe_route.as_mut().ok_or(Error::<T>::RouteNotFound)?;
                route.enabled = true;
                Ok(())
            })?;
            Self::deposit_event(Event::RouteResumed { route_id });
            Ok(())
        }

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
            Self::ensure_supported_domain(source_domain)?;
            Self::ensure_supported_domain(dest_domain)?;
            Self::ensure_supported_domain(asset_home_domain)?;
            ensure!(
                source_domain != dest_domain,
                Error::<T>::RouteDomainMismatch
            );
            ensure!(amount != 0, Error::<T>::ZeroAmount);
            let route_id = Self::bounded_payload(route_id_codec, route_id)?;
            let sender = Self::bounded_payload(sender_codec, sender)?;
            let recipient = Self::bounded_payload(recipient_codec, recipient)?;
            let route = Routes::<T>::get(&route_id).ok_or(Error::<T>::RouteNotFound)?;
            ensure!(route.enabled, Error::<T>::RouteDisabled);
            ensure!(
                route.remote_domain == dest_domain,
                Error::<T>::RouteDomainMismatch
            );

            let nonce = NextOutboundNonce::<T>::get();
            NextOutboundNonce::<T>::put(nonce.saturating_add(1));
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
            let _relayer = ensure_signed(origin)?;
            let proof_family = Self::bounded(proof_family)?;
            let verifier_backend = Self::bounded(verifier_backend)?;
            let proof_bytes = Self::bounded_proof_blob(proof_bytes)?;
            let public_inputs = Self::bounded_proof_blob(public_inputs)?;
            let bundle_bytes = Self::bounded_proof_blob(bundle_bytes)?;
            let route_id = Self::bounded_payload(route_id_codec, route_id)?;

            T::MessageProofVerifier::verify_message_proof(
                proof_family.as_slice(),
                verifier_backend.as_slice(),
                proof_bytes.as_slice(),
                public_inputs.as_slice(),
                bundle_bytes.as_slice(),
                &message_id,
                route_id.codec,
                route_id.bytes.as_slice(),
                source_domain,
                &commitment_root,
            )
            .map_err(|error| match error {
                MessageProofVerificationError::Unavailable => Error::<T>::ProofVerifierUnavailable,
                MessageProofVerificationError::Invalid => Error::<T>::InvalidMessageProof,
            })?;
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
    }

    impl<T: Config> Pallet<T> {
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

        pub(crate) fn finalize_verified_inbound(
            message_id: MessageId,
            route_id: EncodedPayloadOf<T>,
            source_domain: DomainId,
            commitment_root: CommitmentRoot,
        ) -> DispatchResult {
            Self::ensure_inbound_can_finalize(message_id, &route_id, source_domain)?;
            let inbound_key = Self::inbound_message_key(message_id, &route_id, source_domain);
            let route_commitment_key = Self::route_commitment_key(&route_id, source_domain);
            ConsumedInboundMessages::<T>::insert(inbound_key, true);
            LatestCommitmentRoots::<T>::insert(route_commitment_key, commitment_root);
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

        fn route_commitment_key(
            route_id: &EncodedPayloadOf<T>,
            source_domain: DomainId,
        ) -> RouteCommitmentKeyOf<T> {
            RouteCommitmentKey {
                source_domain,
                route_id: route_id.clone(),
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
        pub const MaxProofBlobLen: u32 = 64;
        pub const AllowManualInboundFinalization: bool = false;
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
        type WeightInfo = ();
        type MessageProofVerifier = MockVerifier;
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

    fn route_commitment_key<T: pallet::Config>(
        route_id: &[u8],
        source_domain: DomainId,
    ) -> RouteCommitmentKey<TestPayloadOf<T>> {
        RouteCommitmentKey {
            source_domain,
            route_id: encoded_payload::<T>(SCCP_CODEC_TEXT_UTF8, route_id),
        }
    }

    fn seed_route<T: pallet::Config>(route_id: &[u8], remote_domain: DomainId) {
        let route_id = encoded_payload::<T>(SCCP_CODEC_TEXT_UTF8, route_id);
        let asset_id = encoded_payload::<T>(SCCP_CODEC_TEXT_UTF8, b"xor#universal");
        Routes::<T>::insert(
            route_id,
            RouteRecord {
                asset_id,
                remote_domain,
                enabled: true,
            },
        );
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
            let route_root_key =
                route_commitment_key::<TestRuntime>(b"nexus:eth:xor", SCCP_DOMAIN_ETH);
            let receipt = InboundProofReceipts::<TestRuntime>::get(inbound_key.clone())
                .expect("proof receipt");
            assert_eq!(receipt.proof_family.as_slice(), b"stark-fri-v1");
            assert_eq!(receipt.verifier_backend.as_slice(), b"substrate-runtime-v1");
            assert_eq!(receipt.route_id.codec, SCCP_CODEC_TEXT_UTF8);
            assert_eq!(receipt.route_id.bytes.as_slice(), b"nexus:eth:xor");
            assert!(ConsumedInboundMessages::<TestRuntime>::get(inbound_key));
            assert_eq!(
                LatestCommitmentRoots::<TestRuntime>::get(route_root_key),
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
    fn latest_roots_are_tracked_per_route() {
        new_test_ext().execute_with(|| {
            let message_id_a = [0x90; 32];
            let message_id_b = [0x91; 32];
            let root_a = [0xa0; 32];
            let root_b = [0xb0; 32];

            seed_route::<TestRuntime>(b"nexus:eth:xor", SCCP_DOMAIN_ETH);
            seed_route::<TestRuntime>(b"nexus:bsc:xor", SCCP_DOMAIN_BSC);

            assert_ok!(SccpBridge::finalize_verified_inbound(
                message_id_a,
                encoded_payload::<TestRuntime>(SCCP_CODEC_TEXT_UTF8, b"nexus:eth:xor"),
                SCCP_DOMAIN_ETH,
                root_a,
            ));
            assert_ok!(SccpBridge::finalize_verified_inbound(
                message_id_b,
                encoded_payload::<TestRuntime>(SCCP_CODEC_TEXT_UTF8, b"nexus:bsc:xor"),
                SCCP_DOMAIN_BSC,
                root_b,
            ));

            assert_eq!(
                LatestCommitmentRoots::<TestRuntime>::get(route_commitment_key::<TestRuntime>(
                    b"nexus:eth:xor",
                    SCCP_DOMAIN_ETH,
                )),
                Some(root_a)
            );
            assert_eq!(
                LatestCommitmentRoots::<TestRuntime>::get(route_commitment_key::<TestRuntime>(
                    b"nexus:bsc:xor",
                    SCCP_DOMAIN_BSC,
                )),
                Some(root_b)
            );
        });
    }
}
