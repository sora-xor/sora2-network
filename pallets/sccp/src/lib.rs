// This file is part of the SORA network and Polkaswap app.
//
// Copyright (c) 2026, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

use bridge_types::{
    traits::AuxiliaryDigestHandler, types::AuxiliaryDigestItem, GenericNetworkId, SubNetworkId,
};
use codec::{Decode, Encode};
use common::{
    hash, prelude::Balance, AssetInfoProvider, AssetManager, AssetName, AssetSymbol,
    FromGenericPair,
};
use frame_support::dispatch::DispatchResult;
use frame_support::pallet_prelude::*;
use frame_support::{ensure, transactional};
use frame_system::pallet_prelude::*;
use iroha_sccp::{
    burn_message_id as shared_burn_message_id, is_supported_domain,
    token_add_message_id as shared_token_add_message_id,
    token_pause_message_id as shared_token_pause_message_id,
    token_resume_message_id as shared_token_resume_message_id,
};
use permissions::{Scope, BURN, MINT};
use sp_core::H256;
use sp_runtime::traits::{Convert, Zero};
use sp_runtime::DispatchError;
use sp_std::prelude::*;

pub mod weights;
pub use iroha_sccp::{
    BurnPayloadV1, TokenAddPayloadV1, TokenControlPayloadV1, SCCP_CORE_REMOTE_DOMAINS,
    SCCP_DOMAIN_BSC, SCCP_DOMAIN_ETH, SCCP_DOMAIN_SOL, SCCP_DOMAIN_SORA, SCCP_DOMAIN_SORA_KUSAMA,
    SCCP_DOMAIN_SORA_POLKADOT, SCCP_DOMAIN_TON, SCCP_DOMAIN_TRON, SCCP_MSG_PREFIX_BURN_V1,
    SCCP_MSG_PREFIX_TOKEN_ADD_V1, SCCP_MSG_PREFIX_TOKEN_PAUSE_V1, SCCP_MSG_PREFIX_TOKEN_RESUME_V1,
};
pub use pallet::*;
pub use weights::WeightInfo;

pub const SCCP_TECH_ACC_PREFIX: &[u8] = b"sccp";
pub const SCCP_TECH_ACC_MAIN: &[u8] = b"main";

/// Generic network id used inside `AuxiliaryDigestItem::Commitment` for SCCP burn commitments.
pub const SCCP_DIGEST_NETWORK_ID: GenericNetworkId = GenericNetworkId::EVMLegacy(0x5343_4350);

fn digest_network_id_for_domain(domain_id: u32) -> GenericNetworkId {
    match domain_id {
        SCCP_DOMAIN_SORA_KUSAMA => GenericNetworkId::Sub(SubNetworkId::Kusama),
        SCCP_DOMAIN_SORA_POLKADOT => GenericNetworkId::Sub(SubNetworkId::Polkadot),
        _ => SCCP_DIGEST_NETWORK_ID,
    }
}

/// Lightweight trait used by other pallets to filter SCCP-managed assets.
pub trait SccpAssetChecker<AssetId> {
    fn is_sccp_asset(asset_id: &AssetId) -> bool;
}

impl<AssetId> SccpAssetChecker<AssetId> for () {
    fn is_sccp_asset(_asset_id: &AssetId) -> bool {
        false
    }
}

/// Assets cannot be bridged by the legacy bridge and SCCP at the same time.
pub trait LegacyBridgeAssetChecker<AssetId> {
    fn is_legacy_bridge_asset(asset_id: &AssetId) -> bool;
}

impl<AssetId> LegacyBridgeAssetChecker<AssetId> for () {
    fn is_legacy_bridge_asset(_asset_id: &AssetId) -> bool {
        false
    }
}

/// Runtime hook that verifies a Nexus SCCP burn proof bundle against Nexus finality.
pub trait NexusSccpBurnProofVerifier {
    fn is_available() -> bool;
    fn verify_burn_proof(proof: &[u8]) -> Option<VerifiedBurnProof>;
}

impl NexusSccpBurnProofVerifier for () {
    fn is_available() -> bool {
        false
    }

    fn verify_burn_proof(_proof: &[u8]) -> Option<VerifiedBurnProof> {
        None
    }
}

/// Runtime hook that verifies a Nexus SCCP governance proof bundle, including parliament auth.
pub trait NexusSccpGovernanceProofVerifier {
    fn is_available() -> bool;
    fn verify_governance_proof(proof: &[u8]) -> Option<VerifiedGovernanceProof>;
}

impl NexusSccpGovernanceProofVerifier for () {
    fn is_available() -> bool {
        false
    }

    fn verify_governance_proof(_proof: &[u8]) -> Option<VerifiedGovernanceProof> {
        None
    }
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
    Active,
    Paused,
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
pub struct TokenState {
    pub status: TokenStatus,
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
pub struct VerifiedBurnProof {
    pub message_id: H256,
    pub source_domain: u32,
    pub dest_domain: u32,
    pub nonce: u64,
    pub sora_asset_id: H256,
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
pub struct VerifiedTokenAddProof {
    pub target_domain: u32,
    pub nonce: u64,
    pub sora_asset_id: H256,
    pub decimals: u8,
    pub name: [u8; 32],
    pub symbol: [u8; 32],
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
pub struct VerifiedTokenControlProof {
    pub target_domain: u32,
    pub nonce: u64,
    pub sora_asset_id: H256,
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
pub enum VerifiedGovernanceProofAction {
    Add(VerifiedTokenAddProof),
    Pause(VerifiedTokenControlProof),
    Resume(VerifiedTokenControlProof),
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
pub struct VerifiedGovernanceProof {
    pub message_id: H256,
    pub action: VerifiedGovernanceProofAction,
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    pub type AssetIdOf<T> = common::AssetIdOf<T>;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config:
        frame_system::Config + technical::Config + permissions::Config + common::Config
    {
        #[allow(deprecated)]
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

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

        /// Handler used to append local SCCP burn commitments to the on-chain auxiliary digest.
        type AuxiliaryDigestHandler: AuxiliaryDigestHandler;

        /// Nexus burn proof verifier.
        type NexusSccpBurnProofVerifier: NexusSccpBurnProofVerifier;

        /// Nexus governance proof verifier.
        type NexusSccpGovernanceProofVerifier: NexusSccpGovernanceProofVerifier;

        type WeightInfo: WeightInfo;
    }

    #[pallet::storage]
    #[pallet::getter(fn token_state)]
    pub(super) type Tokens<T: Config> =
        StorageMap<_, Blake2_128Concat, AssetIdOf<T>, TokenState, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn burn_record)]
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

    #[pallet::storage]
    #[pallet::getter(fn applied_governance)]
    pub(super) type AppliedGovernance<T: Config> =
        StorageMap<_, Blake2_128Concat, H256, bool, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn nonce)]
    pub(super) type Nonce<T: Config> = StorageValue<_, u64, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        TokenAddedFromProof {
            asset_id: AssetIdOf<T>,
            message_id: H256,
        },
        TokenPausedFromProof {
            asset_id: AssetIdOf<T>,
            message_id: H256,
        },
        TokenResumedFromProof {
            asset_id: AssetIdOf<T>,
            message_id: H256,
        },
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
    }

    #[pallet::error]
    pub enum Error<T> {
        TokenAlreadyExists,
        TokenNotFound,
        TokenNotActive,
        TokenNotPaused,
        DomainUnsupported,
        RecipientIsZero,
        AmountIsZero,
        NonceOverflow,
        BurnRecordAlreadyExists,
        InboundAlreadyProcessed,
        GovernanceAlreadyApplied,
        ProofVerificationFailed,
        ProofVerificationUnavailable,
        AssetSupplyNotMintable,
        AssetMetadataInvalid,
        RecipientNotCanonical,
        AssetOnLegacyBridge,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
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
                dest_domain != SCCP_DOMAIN_SORA && is_supported_domain(dest_domain),
                Error::<T>::DomainUnsupported
            );
            if matches!(
                dest_domain,
                SCCP_DOMAIN_ETH | SCCP_DOMAIN_BSC | SCCP_DOMAIN_TRON
            ) {
                ensure!(
                    recipient[..12] == [0u8; 12],
                    Error::<T>::RecipientNotCanonical
                );
            }

            let state = Tokens::<T>::get(&asset_id).ok_or(Error::<T>::TokenNotFound)?;
            ensure!(
                matches!(state.status, TokenStatus::Active),
                Error::<T>::TokenNotActive
            );

            let nonce = Nonce::<T>::try_mutate(|nonce| -> Result<u64, DispatchError> {
                ensure!(*nonce != u64::MAX, Error::<T>::NonceOverflow);
                *nonce = nonce.saturating_add(1);
                Ok(*nonce)
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
                    sender,
                    asset_id,
                    amount,
                    dest_domain,
                    recipient,
                    nonce,
                    block_number: frame_system::Pallet::<T>::block_number(),
                },
            );

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

        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::mint_from_proof())]
        #[transactional]
        pub fn mint_from_proof(origin: OriginFor<T>, proof: Vec<u8>) -> DispatchResult {
            let _ = ensure_signed(origin)?;
            let verified = T::NexusSccpBurnProofVerifier::verify_burn_proof(&proof)
                .ok_or(Error::<T>::ProofVerificationUnavailable)?;
            ensure!(
                verified.dest_domain == SCCP_DOMAIN_SORA,
                Error::<T>::DomainUnsupported
            );
            ensure!(verified.amount > Zero::zero(), Error::<T>::AmountIsZero);
            ensure!(verified.recipient != [0u8; 32], Error::<T>::RecipientIsZero);
            ensure!(
                !ProcessedInbound::<T>::get(verified.message_id),
                Error::<T>::InboundAlreadyProcessed
            );

            let asset_id: AssetIdOf<T> = AssetIdOf::<T>::from(verified.sora_asset_id);
            let state = Tokens::<T>::get(&asset_id).ok_or(Error::<T>::TokenNotFound)?;
            ensure!(
                matches!(state.status, TokenStatus::Active),
                Error::<T>::TokenNotActive
            );
            let recipient: T::AccountId = T::AccountIdConverter::convert(verified.recipient);
            let sccp_account = Self::sccp_account()?;
            <T as common::Config>::AssetManager::mint_to(
                &asset_id,
                &sccp_account,
                &recipient,
                verified.amount,
            )?;

            ProcessedInbound::<T>::insert(verified.message_id, true);
            Self::deposit_event(Event::SccpMinted {
                message_id: verified.message_id,
                asset_id,
                amount: verified.amount,
                recipient,
            });
            Ok(())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::add_token_from_proof())]
        #[transactional]
        pub fn add_token_from_proof(origin: OriginFor<T>, proof: Vec<u8>) -> DispatchResult {
            let _ = ensure_signed(origin)?;
            let decoded = Self::verify_governance_proof(&proof, SccpGovernanceAction::Add)?;
            let VerifiedGovernanceProofAction::Add(payload) = decoded.action else {
                return Err(Error::<T>::ProofVerificationFailed.into());
            };

            let asset_id = AssetIdOf::<T>::from(payload.sora_asset_id);
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
            Self::ensure_token_metadata_matches(&asset_id, &payload)?;
            Self::ensure_sccp_permissions(&asset_id)?;

            Tokens::<T>::insert(
                &asset_id,
                TokenState {
                    status: TokenStatus::Active,
                },
            );
            AppliedGovernance::<T>::insert(decoded.message_id, true);
            Self::deposit_event(Event::TokenAddedFromProof {
                asset_id,
                message_id: decoded.message_id,
            });
            Ok(())
        }

        #[pallet::call_index(3)]
        #[pallet::weight(<T as Config>::WeightInfo::pause_token_from_proof())]
        #[transactional]
        pub fn pause_token_from_proof(origin: OriginFor<T>, proof: Vec<u8>) -> DispatchResult {
            let _ = ensure_signed(origin)?;
            let decoded = Self::verify_governance_proof(&proof, SccpGovernanceAction::Pause)?;
            let VerifiedGovernanceProofAction::Pause(payload) = decoded.action else {
                return Err(Error::<T>::ProofVerificationFailed.into());
            };

            let asset_id = AssetIdOf::<T>::from(payload.sora_asset_id);
            Tokens::<T>::try_mutate(&asset_id, |state| -> DispatchResult {
                let Some(state) = state.as_mut() else {
                    return Err(Error::<T>::TokenNotFound.into());
                };
                ensure!(
                    matches!(state.status, TokenStatus::Active),
                    Error::<T>::TokenNotActive
                );
                state.status = TokenStatus::Paused;
                Ok(())
            })?;

            AppliedGovernance::<T>::insert(decoded.message_id, true);
            Self::deposit_event(Event::TokenPausedFromProof {
                asset_id,
                message_id: decoded.message_id,
            });
            Ok(())
        }

        #[pallet::call_index(4)]
        #[pallet::weight(<T as Config>::WeightInfo::resume_token_from_proof())]
        #[transactional]
        pub fn resume_token_from_proof(origin: OriginFor<T>, proof: Vec<u8>) -> DispatchResult {
            let _ = ensure_signed(origin)?;
            let decoded = Self::verify_governance_proof(&proof, SccpGovernanceAction::Resume)?;
            let VerifiedGovernanceProofAction::Resume(payload) = decoded.action else {
                return Err(Error::<T>::ProofVerificationFailed.into());
            };

            let asset_id = AssetIdOf::<T>::from(payload.sora_asset_id);
            Tokens::<T>::try_mutate(&asset_id, |state| -> DispatchResult {
                let Some(state) = state.as_mut() else {
                    return Err(Error::<T>::TokenNotFound.into());
                };
                ensure!(
                    matches!(state.status, TokenStatus::Paused),
                    Error::<T>::TokenNotPaused
                );
                state.status = TokenStatus::Active;
                Ok(())
            })?;

            AppliedGovernance::<T>::insert(decoded.message_id, true);
            Self::deposit_event(Event::TokenResumedFromProof {
                asset_id,
                message_id: decoded.message_id,
            });
            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        #[cfg(feature = "std")]
        pub fn register_token_for_tests(asset_id: AssetIdOf<T>) -> DispatchResult {
            <T as Config>::AssetInfoProvider::ensure_asset_exists(&asset_id)?;
            ensure!(
                !T::LegacyBridgeAssetChecker::is_legacy_bridge_asset(&asset_id),
                Error::<T>::AssetOnLegacyBridge
            );
            Self::ensure_asset_is_mintable(&asset_id)?;
            Self::ensure_sccp_permissions(&asset_id)?;
            Tokens::<T>::insert(
                asset_id,
                TokenState {
                    status: TokenStatus::Active,
                },
            );
            Ok(())
        }

        pub fn burn_message_id(payload: &BurnPayloadV1) -> H256 {
            H256(shared_burn_message_id(payload))
        }

        pub fn token_add_message_id(payload: &TokenAddPayloadV1) -> H256 {
            H256(shared_token_add_message_id(payload))
        }

        pub fn token_pause_message_id(payload: &TokenControlPayloadV1) -> H256 {
            H256(shared_token_pause_message_id(payload))
        }

        pub fn token_resume_message_id(payload: &TokenControlPayloadV1) -> H256 {
            H256(shared_token_resume_message_id(payload))
        }

        fn verify_governance_proof(
            proof: &[u8],
            expected_action: SccpGovernanceAction,
        ) -> Result<VerifiedGovernanceProof, DispatchError> {
            let decoded = T::NexusSccpGovernanceProofVerifier::verify_governance_proof(proof)
                .ok_or(Error::<T>::ProofVerificationUnavailable)?;

            let actual_action = match decoded.action {
                VerifiedGovernanceProofAction::Add(ref payload) => {
                    ensure!(
                        payload.target_domain == SCCP_DOMAIN_SORA,
                        Error::<T>::DomainUnsupported
                    );
                    SccpGovernanceAction::Add
                }
                VerifiedGovernanceProofAction::Pause(ref payload) => {
                    ensure!(
                        payload.target_domain == SCCP_DOMAIN_SORA,
                        Error::<T>::DomainUnsupported
                    );
                    SccpGovernanceAction::Pause
                }
                VerifiedGovernanceProofAction::Resume(ref payload) => {
                    ensure!(
                        payload.target_domain == SCCP_DOMAIN_SORA,
                        Error::<T>::DomainUnsupported
                    );
                    SccpGovernanceAction::Resume
                }
            };
            ensure!(
                actual_action == expected_action,
                Error::<T>::ProofVerificationFailed
            );

            ensure!(
                !AppliedGovernance::<T>::get(decoded.message_id),
                Error::<T>::GovernanceAlreadyApplied
            );
            Ok(decoded)
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

        fn ensure_asset_is_mintable(asset_id: &AssetIdOf<T>) -> DispatchResult {
            let (_symbol, _name, _precision, is_mintable, ..) =
                <T as Config>::AssetInfoProvider::get_asset_info(asset_id);
            ensure!(is_mintable, Error::<T>::AssetSupplyNotMintable);
            Ok(())
        }

        fn ensure_token_metadata_matches(
            asset_id: &AssetIdOf<T>,
            payload: &VerifiedTokenAddProof,
        ) -> DispatchResult {
            let (symbol, name, precision, ..) =
                <T as Config>::AssetInfoProvider::get_asset_info(asset_id);
            ensure!(
                precision == payload.decimals,
                Error::<T>::AssetMetadataInvalid
            );
            ensure!(
                Self::governance_ascii_fixed_32(name.0.as_slice())? == payload.name,
                Error::<T>::AssetMetadataInvalid
            );
            ensure!(
                Self::governance_ascii_fixed_32(symbol.0.as_slice())? == payload.symbol,
                Error::<T>::AssetMetadataInvalid
            );
            Ok(())
        }

        fn governance_ascii_fixed_32(input: &[u8]) -> Result<[u8; 32], DispatchError> {
            ensure!(
                !input.is_empty()
                    && input.len() <= 32
                    && input
                        .iter()
                        .all(|byte| byte.is_ascii() && !byte.is_ascii_control()),
                Error::<T>::AssetMetadataInvalid
            );
            let mut out = [0u8; 32];
            out[..input.len()].copy_from_slice(input);
            Ok(out)
        }
    }

    impl<T: Config> SccpAssetChecker<AssetIdOf<T>> for Pallet<T> {
        fn is_sccp_asset(asset_id: &AssetIdOf<T>) -> bool {
            Tokens::<T>::contains_key(asset_id)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SccpGovernanceAction {
    Add,
    Pause,
    Resume,
}
