//! Iroha's historical Ed25519/SHA3 transcript on maintained Dalek primitives.
//!
//! Iroha signs SHA3-256(message), using SHA3-512 for key expansion, nonce,
//! and challenge. It does not use the RFC 8032 Ed25519ph domain separator.
//! `raw_verify` preserves that transcript and canonical scalar checking.
//! Replacing this with `verify_prehashed` would reject historical signatures.

use ed25519_dalek::{hazmat::raw_verify, Signature, SignatureError, VerifyingKey};
use sha3::{Digest, Sha3_256, Sha3_512};

pub(crate) fn verify(
    public_key: &VerifyingKey,
    message: &[u8],
    signature: &Signature,
) -> Result<(), SignatureError> {
    raw_verify::<Sha3_512>(public_key, &Sha3_256::digest(message), signature)
}

// Signing is only needed to construct test and benchmark inputs. The expanded
// secret and public key are always derived together from the same seed.
#[cfg(any(test, feature = "runtime-benchmarks"))]
pub(crate) struct TestKeypair {
    secret: ed25519_dalek::hazmat::ExpandedSecretKey,
    pub(crate) public: VerifyingKey,
}

#[cfg(any(test, feature = "runtime-benchmarks"))]
impl TestKeypair {
    pub(crate) fn from_seed(seed: [u8; 32]) -> Self {
        let secret =
            ed25519_dalek::hazmat::ExpandedSecretKey::from_bytes(&Sha3_512::digest(seed).into());
        let public = VerifyingKey::from(&secret);
        Self { secret, public }
    }

    pub(crate) fn sign(&self, message: &[u8]) -> Signature {
        ed25519_dalek::hazmat::raw_sign::<Sha3_512>(
            &self.secret,
            &Sha3_256::digest(message),
            &self.public,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Produced by sora2-ed25519-dalek-iroha commit 11f9bb4, before migration.
    // These fixed bytes ensure the expected scheme cannot drift with our signer.
    const PUBLIC: &str = "d68fc2641f357afcf9e1659a1b0479714061b9dbfcfc9e19f6d8dbf7d21544e9";
    const SIGNATURE: &str = "2bb97062c4c3eda97e0c5443831810cf4e17bca7d7e0c3768b46771f16466bbee87f39e527bd19c90f27aae89eddb6e841d00cf005a0d3dc5defd2c390cdf806";
    const MESSAGE: &[u8] = b"SORA Nexus Iroha migration";

    #[test]
    fn verifies_historical_sha3_signature() {
        let public = VerifyingKey::try_from(hex::decode(PUBLIC).unwrap().as_slice()).unwrap();
        let signature = Signature::from_slice(&hex::decode(SIGNATURE).unwrap()).unwrap();
        assert!(verify(&public, MESSAGE, &signature).is_ok());
        assert!(verify(&public, b"changed message", &signature).is_err());
        let keypair = TestKeypair::from_seed([71; 32]);
        assert_eq!(hex::encode(keypair.public.as_bytes()), PUBLIC);
        assert_eq!(hex::encode(keypair.sign(MESSAGE).to_bytes()), SIGNATURE);
    }

    #[test]
    fn rejects_changed_and_noncanonical_signatures_without_panicking() {
        let public = VerifyingKey::try_from(hex::decode(PUBLIC).unwrap().as_slice()).unwrap();
        let bytes: [u8; 64] = hex::decode(SIGNATURE).unwrap().try_into().unwrap();
        for index in 0..64 {
            let mut changed = bytes;
            changed[index] ^= 0x80;
            assert!(verify(&public, MESSAGE, &Signature::from_bytes(&changed)).is_err());
        }
        let mut noncanonical = bytes;
        noncanonical[32..].fill(0xff);
        assert!(verify(&public, MESSAGE, &Signature::from_bytes(&noncanonical)).is_err());
    }
}
