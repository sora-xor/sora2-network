#![allow(unexpected_cfgs)]
#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use sp_runtime_interface::{
    pass_by::{AllocateAndReturnByCodec, PassFatPointerAndDecode},
    runtime_interface,
};
use sp_std::vec::Vec;

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct NexusSccpProofVerifyRequest {
    pub proof: Vec<u8>,
    pub expected_chain_id: Vec<u8>,
    pub trusted_peer_public_keys: Vec<Vec<u8>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct VerifiedBurnProofV1 {
    pub message_id: [u8; 32],
    pub source_domain: u32,
    pub dest_domain: u32,
    pub nonce: u64,
    pub sora_asset_id: [u8; 32],
    pub amount: u128,
    pub recipient: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct VerifiedTokenAddProofV1 {
    pub target_domain: u32,
    pub nonce: u64,
    pub sora_asset_id: [u8; 32],
    pub decimals: u8,
    pub name: [u8; 32],
    pub symbol: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct VerifiedTokenControlProofV1 {
    pub target_domain: u32,
    pub nonce: u64,
    pub sora_asset_id: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum VerifiedGovernanceProofActionV1 {
    Add(VerifiedTokenAddProofV1),
    Pause(VerifiedTokenControlProofV1),
    Resume(VerifiedTokenControlProofV1),
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct VerifiedGovernanceProofV1 {
    pub message_id: [u8; 32],
    pub action: VerifiedGovernanceProofActionV1,
}

#[runtime_interface]
pub trait IrohaProofApi {
    fn verify_nexus_sccp_burn_proof(
        request: PassFatPointerAndDecode<NexusSccpProofVerifyRequest>,
    ) -> AllocateAndReturnByCodec<Option<VerifiedBurnProofV1>> {
        #[cfg(feature = "std")]
        {
            verifier::verify_nexus_sccp_burn_proof(&request).ok()
        }

        #[cfg(not(feature = "std"))]
        {
            let _ = request;
            None
        }
    }

    fn verify_nexus_sccp_governance_proof(
        request: PassFatPointerAndDecode<NexusSccpProofVerifyRequest>,
    ) -> AllocateAndReturnByCodec<Option<VerifiedGovernanceProofV1>> {
        #[cfg(feature = "std")]
        {
            verifier::verify_nexus_sccp_governance_proof(&request).ok()
        }

        #[cfg(not(feature = "std"))]
        {
            let _ = request;
            None
        }
    }
}

#[cfg(feature = "std")]
pub mod verifier {
    use std::{
        collections::{BTreeMap, BTreeSet},
        str::FromStr,
    };

    use iroha_crypto::{Algorithm, Hash, PublicKey, Signature};
    use iroha_data_model::block::BlockHeader;
    use iroha_sccp::{
        burn_message_id, canonical_governance_payload_bytes, decode_nexus_bridge_finality_proof,
        decode_nexus_parliament_certificate, decode_nexus_sccp_burn_proof,
        decode_nexus_sccp_governance_proof, governance_message_id, nexus_commit_vote_preimage,
        verify_burn_bundle_structure, verify_governance_bundle_structure,
        verify_nexus_bridge_finality_proof_structure,
        verify_nexus_parliament_certificate_structure, GovernancePayloadV1,
        NexusBridgeFinalityProofV1, NexusParliamentCertificateV1, NexusParliamentSignatureSchemeV1,
        NexusSccpBurnProofV1, NexusSccpGovernanceProofV1,
    };
    use serde::de::DeserializeOwned;

    use crate::{
        NexusSccpProofVerifyRequest, VerifiedBurnProofV1, VerifiedGovernanceProofActionV1,
        VerifiedGovernanceProofV1, VerifiedTokenAddProofV1, VerifiedTokenControlProofV1,
    };

    type VerifyResult<T> = Result<T, String>;

    pub fn verify_nexus_sccp_burn_proof(
        request: &NexusSccpProofVerifyRequest,
    ) -> VerifyResult<VerifiedBurnProofV1> {
        let bundle: NexusSccpBurnProofV1 = decode_bundle(
            &request.proof,
            "Nexus SCCP burn proof bundle",
            decode_nexus_sccp_burn_proof,
        )?;
        if !verify_burn_bundle_structure(&bundle) {
            return Err("Nexus SCCP burn proof bundle has an invalid structure".to_string());
        }

        let finality_proof = decode_nexus_bridge_finality_proof(&bundle.finality_proof)
            .ok_or_else(|| "failed to decode Nexus SCCP finality proof".to_string())?;
        verify_nexus_bridge_finality_proof(
            &finality_proof,
            &request.expected_chain_id,
            &request.trusted_peer_public_keys,
        )?;

        Ok(VerifiedBurnProofV1 {
            message_id: burn_message_id(&bundle.payload),
            source_domain: bundle.payload.source_domain,
            dest_domain: bundle.payload.dest_domain,
            nonce: bundle.payload.nonce,
            sora_asset_id: bundle.payload.sora_asset_id,
            amount: bundle.payload.amount,
            recipient: bundle.payload.recipient,
        })
    }

    pub fn verify_nexus_sccp_governance_proof(
        request: &NexusSccpProofVerifyRequest,
    ) -> VerifyResult<VerifiedGovernanceProofV1> {
        let bundle: NexusSccpGovernanceProofV1 = decode_bundle(
            &request.proof,
            "Nexus SCCP governance proof bundle",
            decode_nexus_sccp_governance_proof,
        )?;
        if !verify_governance_bundle_structure(&bundle) {
            return Err("Nexus SCCP governance proof bundle has an invalid structure".to_string());
        }

        let finality_proof = decode_nexus_bridge_finality_proof(&bundle.finality_proof)
            .ok_or_else(|| "failed to decode Nexus SCCP finality proof".to_string())?;
        verify_nexus_bridge_finality_proof(
            &finality_proof,
            &request.expected_chain_id,
            &request.trusted_peer_public_keys,
        )?;

        let certificate = decode_nexus_parliament_certificate(&bundle.parliament_certificate)
            .ok_or_else(|| "failed to decode Nexus parliament certificate".to_string())?;
        verify_parliament_enactment_certificate(
            &certificate,
            &bundle.payload,
            finality_proof.height,
        )?;

        let action = match bundle.payload {
            GovernancePayloadV1::Add(payload) => {
                VerifiedGovernanceProofActionV1::Add(VerifiedTokenAddProofV1 {
                    target_domain: payload.target_domain,
                    nonce: payload.nonce,
                    sora_asset_id: payload.sora_asset_id,
                    decimals: payload.decimals,
                    name: payload.name,
                    symbol: payload.symbol,
                })
            }
            GovernancePayloadV1::Pause(payload) => {
                VerifiedGovernanceProofActionV1::Pause(VerifiedTokenControlProofV1 {
                    target_domain: payload.target_domain,
                    nonce: payload.nonce,
                    sora_asset_id: payload.sora_asset_id,
                })
            }
            GovernancePayloadV1::Resume(payload) => {
                VerifiedGovernanceProofActionV1::Resume(VerifiedTokenControlProofV1 {
                    target_domain: payload.target_domain,
                    nonce: payload.nonce,
                    sora_asset_id: payload.sora_asset_id,
                })
            }
        };

        Ok(VerifiedGovernanceProofV1 {
            message_id: governance_message_id(&bundle.payload),
            action,
        })
    }

    fn decode_bundle<T>(
        proof_bytes: &[u8],
        label: &str,
        norito_decode: impl Fn(&[u8]) -> Option<T>,
    ) -> VerifyResult<T>
    where
        T: DeserializeOwned,
    {
        if let Some(bundle) = norito_decode(proof_bytes) {
            return Ok(bundle);
        }

        serde_json::from_slice(proof_bytes)
            .map_err(|err| format!("failed to decode {label} as Norito or JSON: {err}"))
    }

    pub fn verify_nexus_bridge_finality_proof(
        proof: &NexusBridgeFinalityProofV1,
        expected_chain_id: &[u8],
        trusted_peer_public_keys: &[Vec<u8>],
    ) -> VerifyResult<()> {
        if !verify_nexus_bridge_finality_proof_structure(proof) {
            return Err("Nexus SCCP finality proof has an invalid structure".to_string());
        }

        let expected_chain_id = std::str::from_utf8(expected_chain_id)
            .map_err(|err| format!("invalid expected chain id bytes: {err}"))?;
        if proof.chain_id != expected_chain_id {
            return Err(format!(
                "Nexus SCCP finality proof chain_id mismatch: expected {expected_chain_id}, got {}",
                proof.chain_id
            ));
        }
        let block_header: BlockHeader = norito::decode_from_bytes(&proof.block_header_bytes)
            .map_err(|err| format!("failed to decode finalized Nexus block header: {err}"))?;
        if block_header.height().get() != proof.height {
            return Err(format!(
                "Nexus SCCP finality proof height mismatch: header={}, proof={}",
                block_header.height().get(),
                proof.height
            ));
        }
        let Some(header_sccp_commitment_root) = block_header.sccp_commitment_root() else {
            return Err(
                "finalized Nexus block header does not anchor an SCCP commitment root".to_string(),
            );
        };
        if header_sccp_commitment_root != proof.commitment_root {
            return Err(
                "finalized Nexus block header SCCP root does not match the proof root".to_string(),
            );
        }
        let block_hash = block_header.hash();
        if *block_hash.as_ref() != proof.block_hash {
            return Err(
                "Nexus SCCP finality proof block hash does not match the finalized block header"
                    .to_string(),
            );
        }

        let trusted_public_keys = trusted_peer_public_keys
            .iter()
            .map(|public_key| parse_public_key(public_key))
            .collect::<Result<Vec<_>, _>>()?;
        if trusted_public_keys.is_empty() {
            return Err("trusted Nexus validator set must not be empty".to_string());
        }

        let trusted_set = trusted_public_keys
            .iter()
            .map(ToString::to_string)
            .collect::<BTreeSet<_>>();
        if trusted_set.len() != trusted_public_keys.len() {
            return Err("trusted Nexus validator set contains duplicate public keys".to_string());
        }

        let validator_public_keys = proof
            .commit_qc
            .validator_public_keys
            .iter()
            .map(|public_key| {
                PublicKey::from_str(public_key)
                    .map_err(|err| format!("invalid validator public key in proof: {err}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let proof_set = validator_public_keys
            .iter()
            .map(ToString::to_string)
            .collect::<BTreeSet<_>>();
        if proof_set.len() != validator_public_keys.len() {
            return Err(
                "Nexus SCCP finality proof contains duplicate validator public keys".to_string(),
            );
        }
        if proof_set != trusted_set {
            return Err("Nexus SCCP finality proof validator set does not match the trusted Nexus validator set".to_string());
        }

        let roster_len = validator_public_keys.len();
        let signer_indices =
            signer_indices_from_bitmap(&proof.commit_qc.signers_bitmap, roster_len)?;
        let required = min_votes_for_len(roster_len);
        if signer_indices.len() < required {
            return Err(format!(
                "Nexus SCCP finality proof collected {} signatures but requires {}",
                signer_indices.len(),
                required
            ));
        }

        let mut signer_public_keys = Vec::with_capacity(signer_indices.len());
        let mut signer_pops = Vec::with_capacity(signer_indices.len());
        for idx in signer_indices {
            let public_key = validator_public_keys
                .get(idx)
                .ok_or_else(|| format!("validator signer index {idx} is out of bounds"))?;
            if public_key.algorithm() != Algorithm::BlsNormal {
                return Err(format!(
                    "validator signer index {idx} uses unsupported algorithm {:?}",
                    public_key.algorithm()
                ));
            }
            let pop = proof
                .commit_qc
                .validator_set_pops
                .get(idx)
                .ok_or_else(|| format!("validator signer index {idx} is missing a PoP"))?;
            signer_public_keys.push(public_key);
            signer_pops.push(pop.as_slice());
        }

        let preimage = nexus_commit_vote_preimage(&proof.chain_id, &proof.commit_qc);
        iroha_crypto::bls_normal_verify_preaggregated_same_message(
            &preimage,
            &proof.commit_qc.bls_aggregate_signature,
            &signer_public_keys,
            &signer_pops,
        )
        .map_err(|err| format!("Nexus finality aggregate signature verification failed: {err}"))
    }

    pub fn verify_parliament_enactment_certificate(
        certificate: &NexusParliamentCertificateV1,
        governance_payload: &GovernancePayloadV1,
        finality_height: u64,
    ) -> VerifyResult<()> {
        let governance_payload_bytes = canonical_governance_payload_bytes(governance_payload);
        if !verify_nexus_parliament_certificate_structure(
            certificate,
            &governance_payload_bytes,
            finality_height,
        ) {
            return Err("Nexus parliament certificate has an invalid structure".to_string());
        }
        if certificate.signature_scheme != NexusParliamentSignatureSchemeV1::SimpleThreshold {
            return Err("unsupported Nexus parliament signature scheme".to_string());
        }

        let payload_hash = Hash::new(&certificate.payload_bytes);
        let roster_by_signer = certificate
            .roster_members
            .iter()
            .map(|member| {
                (
                    member.signer.clone(),
                    member.public_keys.iter().cloned().collect::<BTreeSet<_>>(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut seen_signers = BTreeSet::new();
        for signature in &certificate.signatures {
            let Some(allowed_public_keys) = roster_by_signer.get(&signature.signer) else {
                return Err(
                    "parliament certificate signer is not a member of the anchored roster"
                        .to_string(),
                );
            };
            if !allowed_public_keys.contains(&signature.public_key) {
                return Err(
                    "parliament certificate signer key is not authorized by the anchored roster"
                        .to_string(),
                );
            }
            let public_key = PublicKey::from_str(&signature.public_key)
                .map_err(|err| format!("invalid parliament signer public key: {err}"))?;
            if !seen_signers.insert(signature.signer.clone()) {
                return Err("parliament certificate contains duplicate signer accounts".to_string());
            }
            Signature::from_bytes(&signature.signature)
                .verify(&public_key, payload_hash.as_ref())
                .map_err(|err| {
                    format!("parliament certificate contains an invalid signature: {err}")
                })?;
        }
        if seen_signers.len() < usize::from(certificate.required_signatures) {
            return Err(format!(
                "parliament certificate collected {} signatures but requires {}",
                seen_signers.len(),
                certificate.required_signatures
            ));
        }

        Ok(())
    }

    fn parse_public_key(public_key_bytes: &[u8]) -> VerifyResult<PublicKey> {
        let public_key = std::str::from_utf8(public_key_bytes)
            .map_err(|err| format!("invalid trusted peer public key bytes: {err}"))?;
        PublicKey::from_str(public_key)
            .map_err(|err| format!("invalid trusted peer public key: {err}"))
    }

    fn signer_indices_from_bitmap(bitmap: &[u8], roster_len: usize) -> VerifyResult<Vec<usize>> {
        let expected_len = roster_len.div_ceil(8);
        if bitmap.len() != expected_len {
            return Err(format!(
                "signer bitmap length mismatch: expected {expected_len}, got {}",
                bitmap.len()
            ));
        }

        let mut signers = Vec::new();
        for (byte_idx, byte) in bitmap.iter().enumerate() {
            if *byte == 0 {
                continue;
            }
            for bit in 0..8 {
                if (byte >> bit) & 1 == 0 {
                    continue;
                }
                let idx = byte_idx * 8 + bit;
                if idx >= roster_len {
                    return Err(format!(
                        "signer bitmap references out-of-bounds validator index {idx}"
                    ));
                }
                signers.push(idx);
            }
        }
        Ok(signers)
    }

    const fn min_votes_for_len(len: usize) -> usize {
        if len > 3 {
            ((len.saturating_sub(1)) / 3) * 2 + 1
        } else {
            len
        }
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use std::num::NonZeroU64;

    use iroha_crypto::{bls_normal_pop_prove, Algorithm, Hash, KeyPair, Signature};
    use iroha_data_model::block::BlockHeader;
    use iroha_sccp::{
        payload_hash, GovernancePayloadV1, NexusBridgeFinalityProofV1, NexusCommitQcV1,
        NexusConsensusPhaseV1, NexusParliamentCertificateV1, NexusParliamentRosterMemberV1,
        NexusParliamentSignatureSchemeV1, NexusParliamentSignatureV1,
    };

    use super::{verifier, NexusSccpProofVerifyRequest};

    #[test]
    fn verify_nexus_bridge_finality_proof_rejects_header_root_mismatch() {
        let (trusted_peer_public_keys, proof) =
            sample_finality_proof([0x11; 32], Some([0x22; 32]), None);

        let err = verifier::verify_nexus_bridge_finality_proof(
            &proof,
            proof.chain_id.as_bytes(),
            &trusted_peer_public_keys,
        )
        .expect_err("mismatched SCCP roots must fail");
        assert!(
            err.contains("SCCP root does not match"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn verify_nexus_bridge_finality_proof_rejects_header_hash_mismatch() {
        let (trusted_peer_public_keys, proof) =
            sample_finality_proof([0x33; 32], None, Some([0x44; 32]));

        let err = verifier::verify_nexus_bridge_finality_proof(
            &proof,
            proof.chain_id.as_bytes(),
            &trusted_peer_public_keys,
        )
        .expect_err("mismatched finalized block hash must fail");
        assert!(
            err.contains("block hash does not match"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn verify_parliament_enactment_certificate_rejects_invalid_public_key_format() {
        let governance_payload = GovernancePayloadV1::Pause(iroha_sccp::TokenControlPayloadV1 {
            version: 1,
            target_domain: iroha_sccp::SCCP_DOMAIN_SORA,
            nonce: 7,
            sora_asset_id: [0x55; 32],
        });
        let (_, finality_proof) = sample_finality_proof([0x55; 32], None, None);
        let certificate = NexusParliamentCertificateV1 {
            version: 1,
            preimage_hash: payload_hash(&iroha_sccp::canonical_governance_payload_bytes(
                &governance_payload,
            )),
            enactment_window_start: finality_proof.height,
            enactment_window_end: finality_proof.height,
            payload_bytes: b"nexus-certificate-payload".to_vec(),
            signature_scheme: NexusParliamentSignatureSchemeV1::SimpleThreshold,
            roster_epoch: 7,
            roster_members: vec![NexusParliamentRosterMemberV1 {
                signer: "alice@parliament".to_string(),
                public_keys: vec!["not-a-public-key".to_string()],
            }],
            required_signatures: 1,
            signatures: vec![NexusParliamentSignatureV1 {
                signer: "alice@parliament".to_string(),
                public_key: "not-a-public-key".to_string(),
                signature: vec![0xAA],
            }],
        };

        let err = verifier::verify_parliament_enactment_certificate(
            &certificate,
            &governance_payload,
            finality_proof.height,
        )
        .expect_err("invalid roster public key encoding must fail");
        assert!(
            err.contains("invalid parliament signer public key"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn verify_parliament_enactment_certificate_rejects_bad_signature() {
        let governance_payload = GovernancePayloadV1::Pause(iroha_sccp::TokenControlPayloadV1 {
            version: 1,
            target_domain: iroha_sccp::SCCP_DOMAIN_SORA,
            nonce: 8,
            sora_asset_id: [0x66; 32],
        });
        let (_, finality_proof) = sample_finality_proof([0x66; 32], None, None);
        let signer_keys = seeded_key_pair(0x33, Algorithm::Ed25519);
        let certificate_payload = b"nexus-certificate-payload".to_vec();
        let wrong_signature = Signature::new(
            signer_keys.private_key(),
            Hash::new(b"wrong-certificate-payload").as_ref(),
        );
        let certificate = NexusParliamentCertificateV1 {
            version: 1,
            preimage_hash: payload_hash(&iroha_sccp::canonical_governance_payload_bytes(
                &governance_payload,
            )),
            enactment_window_start: finality_proof.height,
            enactment_window_end: finality_proof.height,
            payload_bytes: certificate_payload,
            signature_scheme: NexusParliamentSignatureSchemeV1::SimpleThreshold,
            roster_epoch: 8,
            roster_members: vec![NexusParliamentRosterMemberV1 {
                signer: "alice@parliament".to_string(),
                public_keys: vec![signer_keys.public_key().to_string()],
            }],
            required_signatures: 1,
            signatures: vec![NexusParliamentSignatureV1 {
                signer: "alice@parliament".to_string(),
                public_key: signer_keys.public_key().to_string(),
                signature: wrong_signature.payload().to_vec(),
            }],
        };

        let err = verifier::verify_parliament_enactment_certificate(
            &certificate,
            &governance_payload,
            finality_proof.height,
        )
        .expect_err("mismatched certificate signature must fail");
        assert!(err.contains("invalid signature"), "unexpected error: {err}");
    }

    #[test]
    fn verify_nexus_sccp_burn_proof_accepts_json_bundle() {
        let payload = iroha_sccp::BurnPayloadV1 {
            version: 1,
            source_domain: iroha_sccp::SCCP_DOMAIN_ETH,
            dest_domain: iroha_sccp::SCCP_DOMAIN_SORA,
            nonce: 1,
            sora_asset_id: [0x11; 32],
            amount: 42,
            recipient: [0x22; 32],
        };
        let commitment = iroha_sccp::SccpHubCommitmentV1 {
            version: 1,
            kind: iroha_sccp::SccpHubMessageKind::Burn,
            target_domain: payload.dest_domain,
            message_id: iroha_sccp::burn_message_id(&payload),
            payload_hash: payload_hash(&iroha_sccp::canonical_burn_payload_bytes(&payload)),
            parliament_certificate_hash: None,
        };
        let commitment_root = iroha_sccp::commitment_leaf_hash(&commitment);
        let bundle = iroha_sccp::NexusSccpBurnProofV1 {
            version: 1,
            commitment_root,
            commitment,
            merkle_proof: iroha_sccp::SccpMerkleProofV1 { steps: Vec::new() },
            payload,
            finality_proof: norito::to_bytes(&sample_finality_proof(commitment_root, None, None).1)
                .expect("encode proof"),
        };
        let request = NexusSccpProofVerifyRequest {
            proof: serde_json::to_vec(&bundle).expect("encode bundle json"),
            expected_chain_id: bundle
                .finality_proof
                .as_slice()
                .len()
                .to_string()
                .into_bytes(),
            trusted_peer_public_keys: Vec::new(),
        };

        let err = verifier::verify_nexus_sccp_burn_proof(&request)
            .expect_err("wrong chain id should still prove JSON decode path");
        assert!(
            err.contains("chain_id mismatch") || err.contains("trusted Nexus validator set"),
            "unexpected error: {err}"
        );
    }

    fn sample_finality_proof(
        header_commitment_root: [u8; 32],
        proof_commitment_root_override: Option<[u8; 32]>,
        proof_block_hash_override: Option<[u8; 32]>,
    ) -> (Vec<Vec<u8>>, NexusBridgeFinalityProofV1) {
        let validator_keys = seeded_key_pair(0x21, Algorithm::BlsNormal);
        let validator_public_key = validator_keys.public_key().to_string();
        let validator_pop =
            bls_normal_pop_prove(validator_keys.private_key()).expect("BLS PoP should sign");

        let mut header = BlockHeader::new(
            NonZeroU64::new(42).expect("non-zero height"),
            None,
            None,
            None,
            1_717_171_717,
            0,
        );
        header.set_sccp_commitment_root(Some(header_commitment_root));

        let header_bytes = norito::to_bytes(&header).expect("header should encode");
        let canonical_block_hash: [u8; 32] = (*header.hash().as_ref()).into();
        let proof_block_hash = proof_block_hash_override.unwrap_or(canonical_block_hash);
        let proof = NexusBridgeFinalityProofV1 {
            version: 1,
            chain_id: "sora-nexus-mainnet".to_string(),
            height: header.height().get(),
            block_hash: proof_block_hash,
            commitment_root: proof_commitment_root_override.unwrap_or(header_commitment_root),
            block_header_bytes: header_bytes,
            commit_qc: NexusCommitQcV1 {
                version: 1,
                phase: NexusConsensusPhaseV1::Commit,
                height: header.height().get(),
                view: 9,
                epoch: 3,
                mode_tag: "normal".to_string(),
                subject_block_hash: proof_block_hash,
                validator_set_hash_version: 1,
                validator_public_keys: vec![validator_public_key.clone()],
                validator_set_pops: vec![validator_pop],
                signers_bitmap: vec![0b0000_0001],
                bls_aggregate_signature: vec![0x01],
            },
        };

        (vec![validator_public_key.into_bytes()], proof)
    }

    fn seeded_key_pair(seed_byte: u8, algorithm: Algorithm) -> KeyPair {
        KeyPair::from_seed(vec![seed_byte; 32], algorithm)
    }
}
