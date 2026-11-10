// Copyright 2019-2025 @polkadot/wasm-crypto authors & contributors
// SPDX-License-Identifier: Apache-2.0

use ed25519_dalek::{Signature, Signer as _, SigningKey, Verifier as _, VerifyingKey};
use std::convert::TryFrom;
use wasm_bindgen::prelude::*;

/// Keypair helper function
fn new_from_seed(seed: &[u8]) -> SigningKey {
    SigningKey::from_bytes(seed.try_into().expect("Invalid seed provided."))
}

/// Generate a key pair.
///
/// * seed: UIntArray with 32 element
///
/// returned vector is the concatenation of first the seed (32 bytes)
/// followed by the public key (32) bytes, as the full secret keys.
#[wasm_bindgen]
pub fn ext_ed_from_seed(seed: &[u8]) -> Vec<u8> {
    new_from_seed(seed).to_keypair_bytes().to_vec()
}

/// Sign a message
///
/// The combination of both public and private key must be provided.
/// This is effectively equivalent to a keypair.
///
/// * _: UIntArray with 32 element (was pubkey, now ignored)
/// * private: UIntArray with 32 element
/// * message: Arbitrary length UIntArray
///
/// * returned vector is the signature consisting of 64 bytes.
#[wasm_bindgen]
pub fn ext_ed_sign(_: &[u8], seckey: &[u8], message: &[u8]) -> Vec<u8> {
    // https://github.com/MystenLabs/ed25519-unsafe-libs
    // we never use the provided pubkey
    new_from_seed(seckey).sign(message).to_bytes().to_vec()
}

/// Verify a message and its corresponding against a public key;
///
/// * signature: UIntArray with 64 element
/// * message: Arbitrary length UIntArray
/// * pubkey: UIntArray with 32 element
#[wasm_bindgen]
pub fn ext_ed_verify(signature: &[u8], message: &[u8], pubkey: &[u8]) -> bool {
    match (
        Signature::try_from(signature),
        pubkey
            .try_into()
            .ok()
            .and_then(|bytes| VerifyingKey::from_bytes(bytes).ok()),
    ) {
        (Ok(s), Some(k)) => k.verify(message, &s).is_ok(),
        _ => false,
    }
}

#[cfg(test)]
pub mod tests {
    extern crate rand;

    use super::*;
    use ed25519_dalek::{KEYPAIR_LENGTH, SECRET_KEY_LENGTH, SIGNATURE_LENGTH};
    use hex_literal::hex;

    fn generate_random_seed() -> Vec<u8> {
        (0..32).map(|_| rand::random::<u8>()).collect()
    }

    #[test]
    fn can_new_keypair() {
        let seed = generate_random_seed();
        let keypair = ext_ed_from_seed(seed.as_slice());

        assert!(keypair.len() == KEYPAIR_LENGTH);
    }

    #[test]
    fn creates_pair_from_known() {
        let seed = b"12345678901234567890123456789012";
        let expected = hex!("2f8c6129d816cf51c374bc7f08c3e63ed156cf78aefb4a6550d97b87997977ee");
        let keypair = ext_ed_from_seed(seed);
        let public = &keypair[SECRET_KEY_LENGTH..KEYPAIR_LENGTH];

        assert_eq!(public, expected);
    }

    #[test]
    fn can_sign_message() {
        let seed = generate_random_seed();
        let keypair = ext_ed_from_seed(seed.as_slice());
        let private = &keypair[0..SECRET_KEY_LENGTH];
        let public = &keypair[SECRET_KEY_LENGTH..KEYPAIR_LENGTH];
        let message = b"this is a message";
        let signature = ext_ed_sign(public, private, message);

        assert!(signature.len() == SIGNATURE_LENGTH);
    }

    #[test]
    fn can_verify_message() {
        let seed = generate_random_seed();
        let keypair = ext_ed_from_seed(seed.as_slice());
        let private = &keypair[0..SECRET_KEY_LENGTH];
        let public = &keypair[SECRET_KEY_LENGTH..KEYPAIR_LENGTH];
        let message = b"this is a message";
        let signature = ext_ed_sign(public, private, message);
        let is_valid = ext_ed_verify(&signature[..], message, public);

        assert!(is_valid);
    }

    #[test]
    fn can_verify_known() {
        let public = hex!("2f8c6129d816cf51c374bc7f08c3e63ed156cf78aefb4a6550d97b87997977ee");
        let message = b"this is a message";
        let signature = hex!("90588f3f512496f2dd40571d162e8182860081c74e2085316e7c4396918f07da412ee029978e4dd714057fe973bd9e7d645148bf7b66680d67c93227cde95202");
        let is_valid = ext_ed_verify(&signature, message, &public);

        assert!(is_valid);
    }

    #[test]
    fn can_verify_known_wrong() {
        let public = hex!("2f8c6129d816cf51c374bc7f08c3e63ed156cf78aefb4a6550d97b87997977ee");
        let message = b"this is a message";
        let signature = &[0u8; 64];
        let is_valid = ext_ed_verify(signature, message, &public);

        assert_eq!(is_valid, false);
    }
}

#[cfg(test)]
mod security_tests {
    use super::*;

    #[test]
    fn signing_ignores_caller_supplied_public_key() {
        let secret = [7u8; 32];
        let pair = ext_ed_from_seed(&secret);
        let signature = ext_ed_sign(&pair[32..], &secret, b"bound key");
        assert_eq!(signature, ext_ed_sign(&[3u8; 32], &secret, b"bound key"));
        assert!(ext_ed_verify(&signature, b"bound key", &pair[32..]));
    }

    #[test]
    fn verification_rejects_noncanonical_scalar_and_truncation() {
        let pair = ext_ed_from_seed(&[7u8; 32]);
        let mut signature = ext_ed_sign(&pair[32..], &pair[..32], b"canonical");
        signature[32..].fill(255);
        assert!(!ext_ed_verify(&signature, b"canonical", &pair[32..]));
        assert!(!ext_ed_verify(&signature[..63], b"canonical", &pair[32..]));
        assert!(!ext_ed_verify(&signature, b"canonical", &pair[32..63]));
    }
}
