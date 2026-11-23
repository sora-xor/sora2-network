// Copyright 2019-2025 @polkadot/wasm-crypto authors & contributors
// SPDX-License-Identifier: Apache-2.0

use bip39::{Language, Mnemonic};
use pbkdf2::pbkdf2_hmac;
use sha2::Sha512;
use wasm_bindgen::prelude::*;

/// Generate a bip39 phrase
///
/// words: number of words, either 12, 15, 18 21 or 24
///
/// Returns the bip 39 phrase
#[wasm_bindgen]
pub fn ext_bip39_generate(words: u32) -> String {
    assert!(
        matches!(words, 12 | 15 | 18 | 21 | 24),
        "Invalid count provided."
    );
    let mut entropy = vec![0; words as usize / 3 * 4];
    getrandom::fill(&mut entropy).expect("Cryptographic randomness unavailable.");
    Mnemonic::from_entropy_in(Language::English, &entropy)
        .expect("Invalid entropy length.")
        .to_string()
}

/// Create entropy from a bip39 phrase
///
/// * phrase: mnemonic phrase
///
/// Returns the entropy
#[wasm_bindgen]
pub fn ext_bip39_to_entropy(phrase: &str) -> Vec<u8> {
    match Mnemonic::parse_in(Language::English, phrase) {
        Ok(m) => m.to_entropy().to_vec(),
        _ => panic!("Invalid phrase provided."),
    }
}

/// Create a mini-secret from a bip39 phrase
///
/// * phrase: mnemonic phrase
///
/// Returns the 32-byte mini-secret via entropy
#[wasm_bindgen]
pub fn ext_bip39_to_mini_secret(phrase: &str, password: &str) -> Vec<u8> {
    match Mnemonic::parse_in(Language::English, phrase) {
        Ok(m) => {
            let mut res = [0u8; 64];
            let mut seed = vec![];

            seed.extend_from_slice(b"mnemonic");
            seed.extend_from_slice(password.as_bytes());

            pbkdf2_hmac::<Sha512>(&m.to_entropy(), &seed, 2048, &mut res);

            res[..32].to_vec()
        }
        _ => panic!("Invalid phrase provided."),
    }
}

/// Creates a BTC/ETH compatible seed from a bip-39 phrase
///
/// @phrase: mnemonic phrase
///
/// Returns a 32-byte seed
#[wasm_bindgen]
pub fn ext_bip39_to_seed(phrase: &str, password: &str) -> Vec<u8> {
    match Mnemonic::parse_in(Language::English, phrase) {
        Ok(m) => m.to_seed(password)[..32].to_vec(),
        _ => panic!("Invalid phrase provided."),
    }
}

/// Validates a bip39 phrase
///
/// * phrase: mnemonic phrase
///
/// Returns the true/false
#[wasm_bindgen]
pub fn ext_bip39_validate(phrase: &str) -> bool {
    // The published validation API requires exact English words and ASCII single
    // spaces, whereas its derivation APIs normalize whitespace and Unicode.
    Mnemonic::parse_in(Language::English, phrase)
        .is_ok_and(|mnemonic| mnemonic.to_string() == phrase)
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use hex_literal::hex;

    #[test]
    fn can_bip39_entropy() {
        let phrase = "legal winner thank year wave sausage worth useful legal winner thank yellow";
        let entropy = hex!("7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f");
        let res = ext_bip39_to_entropy(phrase);

        assert_eq!(res, entropy);
    }

    #[test]
    fn can_bip39_mini_secret() {
        let phrase = "legal winner thank year wave sausage worth useful legal winner thank yellow";
        let password = "Substrate";
        let mini = hex!("4313249608fe8ac10fd5886c92c4579007272cb77c21551ee5b8d60b78041685");
        let res = ext_bip39_to_mini_secret(phrase, password);

        assert_eq!(res[..], mini[..]);
    }

    #[test]
    fn can_bip39_seed() {
        let phrase = "seed sock milk update focus rotate barely fade car face mechanic mercy";
        let seed = hex!("3c121e20de068083b49c2315697fb59a2d9e8643c24e5ea7628132c58969a027");
        let res = ext_bip39_to_seed(phrase, "");

        assert_eq!(res[..], seed[..]);
    }

    #[test]
    fn can_bip39_generate() {
        let phrase = ext_bip39_generate(12);
        let is_valid = ext_bip39_validate(&phrase);

        assert!(is_valid);
    }

    #[test]
    fn can_bip39_validate() {
        let is_valid = ext_bip39_validate(
            "seed sock milk update focus rotate barely fade car face mechanic mercy",
        );
        let is_invalid = ext_bip39_validate(
            "wine photo extra cushion basket dwarf humor cloud truck job boat submit",
        );

        assert_eq!(is_valid, true);
        assert_eq!(is_invalid, false);
    }
}

#[cfg(test)]
mod compatibility_tests {
    use super::*;

    #[test]
    fn strict_validation_keeps_permissive_derivation_normalization() {
        let phrase = "legal winner thank year wave sausage worth useful legal winner thank yellow";
        let padded = format!(" {phrase} ");
        assert!(ext_bip39_validate(phrase));
        for alternative in [
            padded,
            phrase.replace(' ', "  "),
            phrase.replace(' ', "\t"),
            phrase.to_uppercase(),
        ] {
            assert!(!ext_bip39_validate(&alternative));
        }
        assert_eq!(
            ext_bip39_to_entropy(phrase),
            ext_bip39_to_entropy(&format!(" {phrase} "))
        );
    }
}
