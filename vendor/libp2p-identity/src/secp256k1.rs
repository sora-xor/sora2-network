// Copyright 2019 Parity Technologies (UK) Ltd.
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the "Software"),
// to deal in the Software without restriction, including without limitation
// the rights to use, copy, modify, merge, publish, distribute, sublicense,
// and/or sell copies of the Software, and to permit persons to whom the
// Software is furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

//! Secp256k1 keys.

use core::{cmp, fmt, hash};

use asn1_der::typed::{DerDecodable, Sequence};
use k256::{
    ecdsa::Signature,
    sha2::{Digest as ShaDigestTrait, Sha256},
};
use zeroize::Zeroize;
#[cfg(feature = "rand")]
use k256::elliptic_curve::Generate;

use super::error::DecodingError;

/// A Secp256k1 keypair.
#[derive(Clone)]
pub struct Keypair {
    secret: SecretKey,
    public: PublicKey,
}

impl Keypair {
    /// Generate a new sec256k1 `Keypair`.
    #[cfg(feature = "rand")]
    pub fn generate() -> Keypair {
        Keypair::from(SecretKey::generate())
    }

    /// Get the public key of this keypair.
    pub fn public(&self) -> &PublicKey {
        &self.public
    }

    /// Get the secret key of this keypair.
    pub fn secret(&self) -> &SecretKey {
        &self.secret
    }
}

impl fmt::Debug for Keypair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Keypair")
            .field("public", &self.public)
            .finish()
    }
}

/// Promote a Secp256k1 secret key into a keypair.
impl From<SecretKey> for Keypair {
    fn from(secret: SecretKey) -> Keypair {
        let public = PublicKey(*secret.0.verifying_key());
        Keypair { secret, public }
    }
}

/// Demote a Secp256k1 keypair into a secret key.
impl From<Keypair> for SecretKey {
    fn from(kp: Keypair) -> SecretKey {
        kp.secret
    }
}

/// A Secp256k1 secret key.
#[derive(Clone)]
pub struct SecretKey(k256::ecdsa::SigningKey);

impl fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SecretKey")
    }
}

impl SecretKey {
    /// Generate a new random Secp256k1 secret key.
    #[cfg(feature = "rand")]
    pub fn generate() -> SecretKey {
        SecretKey(k256::ecdsa::SigningKey::generate_from_rng(&mut rand10::rng()))
    }

    /// Create a secret key from a byte slice, zeroing the slice on success.
    /// If the bytes do not constitute a valid Secp256k1 secret key, an
    /// error is returned.
    ///
    /// Note that the expected binary format is the same as `libsecp256k1`'s.
    pub fn try_from_bytes(mut sk: impl AsMut<[u8]>) -> Result<SecretKey, DecodingError> {
        let sk_bytes = sk.as_mut();
        let secret = k256::ecdsa::SigningKey::from_slice(sk_bytes)
            .map_err(|e| DecodingError::failed_to_parse("parse secp256k1 secret key", e))?;
        sk_bytes.zeroize();
        Ok(SecretKey(secret))
    }

    /// Decode a DER-encoded Secp256k1 secret key in an ECPrivateKey
    /// structure as defined in [RFC5915], zeroing the input slice on success.
    ///
    /// [RFC5915]: https://tools.ietf.org/html/rfc5915
    pub fn from_der(mut der: impl AsMut<[u8]>) -> Result<SecretKey, DecodingError> {
        // TODO: Stricter parsing.
        let der_obj = der.as_mut();

        let mut sk_bytes = Sequence::decode(der_obj)
            .and_then(|seq| seq.get(1))
            .and_then(Vec::load)
            .map_err(|e| DecodingError::failed_to_parse("secp256k1 SecretKey bytes", e))?;

        let sk = SecretKey::try_from_bytes(&mut sk_bytes)?;
        sk_bytes.zeroize();
        der_obj.zeroize();
        Ok(sk)
    }

    /// Sign a message with this secret key, producing a DER-encoded
    /// ECDSA signature, as defined in [RFC3278].
    ///
    /// [RFC3278]: https://tools.ietf.org/html/rfc3278#section-8.2
    pub fn sign(&self, msg: &[u8]) -> Vec<u8> {
        use k256::ecdsa::signature::Signer;

        Signer::<k256::ecdsa::Signature>::sign(&self.0, msg)
            .to_der()
            .to_bytes()
            .into_vec()
    }

    /// Returns the raw bytes of the secret key.
    pub fn to_bytes(&self) -> [u8; 32] {
        self.0.to_bytes().into()
    }
}

/// A Secp256k1 public key.
#[derive(Eq, Clone)]
pub struct PublicKey(k256::ecdsa::VerifyingKey);

impl fmt::Debug for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PublicKey(compressed): ")?;
        for byte in &self.to_bytes() {
            write!(f, "{byte:x}")?;
        }
        Ok(())
    }
}

impl cmp::PartialEq for PublicKey {
    fn eq(&self, other: &Self) -> bool {
        self.to_bytes().eq(&other.to_bytes())
    }
}

impl hash::Hash for PublicKey {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.to_bytes().hash(state);
    }
}

impl cmp::PartialOrd for PublicKey {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl cmp::Ord for PublicKey {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.to_bytes().cmp(&other.to_bytes())
    }
}

impl PublicKey {
    /// Verify the Secp256k1 signature on a message using the public key.
    pub fn verify(&self, msg: &[u8], sig: &[u8]) -> bool {
        let digest = Sha256::new_with_prefix(msg);
        self.verify_hash(digest.finalize().as_slice(), sig)
    }

    /// Verify the Secp256k1 DER-encoded signature on a raw 256-bit message using the public key.  
    /// Will return false if the hash is not 32 bytes long, or the signature cannot be parsed.
    pub fn verify_hash(&self, msg: &[u8], sig: &[u8]) -> bool {
        use k256::ecdsa::signature::hazmat::PrehashVerifier;

        if msg.len() != 32 {
            return false;
        }
        Signature::from_der(sig).is_ok_and(|s| {
            // libp2p has historically accepted both low-S and high-S signatures.
            // RustCrypto 0.14 enforces low-S, so normalize before verification.
            let s = s.normalize_s();
            self.0.verify_prehash(msg, &s).is_ok()
        })
    }

    /// Convert the public key to a byte buffer in compressed form, i.e. with one coordinate
    /// represented by a single bit.
    pub fn to_bytes(&self) -> [u8; 33] {
        let encoded_point = self.0.to_sec1_point(true);
        debug_assert!(encoded_point.as_bytes().len() == 33);
        let mut array: [u8; 33] = [0u8; 33];
        array.copy_from_slice(encoded_point.as_bytes());
        array
    }

    /// Convert the public key to a byte buffer in uncompressed form.
    pub fn to_bytes_uncompressed(&self) -> [u8; 65] {
        let encoded_point = self.0.to_sec1_point(false);
        debug_assert!(encoded_point.as_bytes().len() == 65);
        let mut array: [u8; 65] = [0u8; 65];
        array.copy_from_slice(encoded_point.as_bytes());
        array
    }

    /// Decode a public key from a byte slice in the format produced
    /// by `encode`.
    pub fn try_from_bytes(k: &[u8]) -> Result<PublicKey, DecodingError> {
        k256::ecdsa::VerifyingKey::from_sec1_bytes(k)
            .map_err(|e| DecodingError::failed_to_parse("secp256k1 public key", e))
            .map(PublicKey)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Captured from unmodified k256 0.13.4 before the RustCrypto migration.
    #[test]
    fn rustcrypto_migration_preserves_signatures_and_high_s_verification() {
        use hex_literal::hex;
        let message = b"SORA Nexus dependency migration compatibility vector";
        let secret = SecretKey::try_from_bytes([1u8; 32]).unwrap();
        let pair = Keypair::from(secret);
        let signature = hex!("3045022100e0a0f82c4bb65bc2ccd2accc61e8119a2eb9f2a0c3f2baf63f535178cec6da9702203466ed7f23ddd0467e16dc153e5ba833120781fdefa8e1b4e141a164644dbbe7");
        let high_s = hex!("3046022100e0a0f82c4bb65bc2ccd2accc61e8119a2eb9f2a0c3f2baf63f535178cec6da97022100cb991280dc222fb981e923eac1a457cba8a75ae8bf9fbe86de90bd286be8855a");
        assert_eq!(pair.public().to_bytes(), hex!("031b84c5567b126440995d3ed5aaba0565d71e1834604819ff9c17f5e9d5dd078f"));
        assert_eq!(pair.secret().sign(message), signature);
        assert!(pair.public().verify(message, &signature));
        assert!(pair.public().verify(message, &high_s));
        assert!(!pair.public().verify(b"modified message", &signature));
        assert!(!pair.public().verify_hash(&[0; 31], &signature));
        assert!(!pair.public().verify_hash(&[0; 33], &signature));
        assert!(!pair.public().verify(message, &[0; 64]));
    }

    #[test]
    #[cfg(feature = "rand")]
    fn secp256k1_secret_from_bytes() {
        let sk1 = SecretKey::generate();
        let mut sk_bytes = [0; 32];
        sk_bytes.copy_from_slice(&sk1.to_bytes()[..]);
        let sk2 = SecretKey::try_from_bytes(&mut sk_bytes).unwrap();
        assert_eq!(sk1.to_bytes(), sk2.to_bytes());
        assert_eq!(sk_bytes, [0; 32]);
    }
}
