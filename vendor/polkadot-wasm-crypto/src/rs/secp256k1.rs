// Copyright 2019-2025 @polkadot/wasm-crypto authors & contributors
// SPDX-License-Identifier: Apache-2.0

use sora_secp256k1_compat::{Message, PublicKey, RecoveryId, SecretKey, Signature};
use wasm_bindgen::prelude::*;

fn public_key(bytes: &[u8]) -> PublicKey {
    // The published C-library API only accepts SEC1 33/65-byte encodings.
    assert!(matches!(bytes.len(), 33 | 65), "Invalid pubkey provided.");
    PublicKey::parse_slice(bytes, None).expect("Invalid pubkey provided.")
}

#[wasm_bindgen]
pub fn ext_secp_pub_compress(pubkey: &[u8]) -> Vec<u8> {
    public_key(pubkey).serialize_compressed().to_vec()
}

#[wasm_bindgen]
pub fn ext_secp_pub_expand(pubkey: &[u8]) -> Vec<u8> {
    public_key(pubkey).serialize().to_vec()
}

#[wasm_bindgen]
pub fn ext_secp_from_seed(seed: &[u8]) -> Vec<u8> {
    let secret = SecretKey::parse_slice(seed).expect("Invalid seed provided.");
    let mut result = secret.serialize().to_vec();
    result.extend_from_slice(&PublicKey::from_secret_key(&secret).serialize_compressed());
    result
}

#[wasm_bindgen]
pub fn ext_secp_recover(hash: &[u8], sig: &[u8], rec: i32) -> Vec<u8> {
    let rec = u8::try_from(rec)
        .ok()
        .and_then(|r| RecoveryId::parse(r).ok())
        .expect("Invalid recovery data provided.");
    let message = Message::parse_slice(hash).expect("Invalid message provided.");
    let signature = Signature::parse_standard_slice(sig).expect("Invalid signature provided.");
    let parsed = k256::ecdsa::Signature::from_slice(sig).expect("Invalid signature provided.");
    // This JS API has always rejected high-S recovery, unlike the historical
    // Substrate host functions implemented by the shared adapter.
    assert_eq!(
        parsed.normalize_s(),
        parsed,
        "Non-normalized signature provided."
    );
    sora_secp256k1_compat::recover(&message, &signature, &rec)
        .expect("Unable to recover.")
        .serialize_compressed()
        .to_vec()
}

#[wasm_bindgen]
pub fn ext_secp_sign(hash: &[u8], seckey: &[u8]) -> Vec<u8> {
    assert_eq!(hash.len(), 32, "Invalid message provided.");
    let secret = k256::ecdsa::SigningKey::from_slice(seckey).expect("Invalid secret provided.");
    let mut generator = RawDigestNonce::new(seckey, hash);
    loop {
        let nonce_bytes = generator.next();
        if let Some(nonce) = k256::NonZeroScalar::from_repr((*nonce_bytes).into()).into_option() {
            let nonce = k256::elliptic_curve::zeroize::Zeroizing::new(nonce);
            if let Ok((signature, rec)) = ecdsa::hazmat::sign_prehashed::<k256::Secp256k1>(
                secret.as_nonzero_scalar(),
                &nonce,
                hash,
            ) {
                let mut result = signature.to_bytes().to_vec();
                result.push(rec.to_byte());
                return result;
            }
        }
    }
}

/// RFC6979 section 3.2 HMAC-SHA256 schedule, with the published wrapper's raw
/// digest input preserved. Its old C backend omitted bits2octets: replacing it
/// with k256's standard RFC6979 helper changes signatures when digest >= n.
/// All HMAC and ECDSA operations use maintained RustCrypto implementations.
/// This type is private and cannot be used to inject an external nonce.
struct RawDigestNonce {
    key: k256::elliptic_curve::zeroize::Zeroizing<[u8; 32]>,
    value: k256::elliptic_curve::zeroize::Zeroizing<[u8; 32]>,
}

impl RawDigestNonce {
    fn hmac(key: &[u8], chunks: &[&[u8]]) -> k256::elliptic_curve::zeroize::Zeroizing<[u8; 32]> {
        use hmac::{Hmac, KeyInit, Mac};
        let mut hmac =
            Hmac::<sha2::Sha256>::new_from_slice(key).expect("HMAC accepts 32-byte keys");
        for chunk in chunks {
            hmac.update(chunk);
        }
        k256::elliptic_curve::zeroize::Zeroizing::new(hmac.finalize().into_bytes().into())
    }

    fn new(secret: &[u8], digest: &[u8]) -> Self {
        use k256::elliptic_curve::zeroize::Zeroizing;
        let mut state = Self {
            key: Zeroizing::new([0; 32]),
            value: Zeroizing::new([1; 32]),
        };
        for separator in [0, 1] {
            state.key = Self::hmac(
                &state.key[..],
                &[&state.value[..], &[separator], secret, digest],
            );
            state.value = Self::hmac(&state.key[..], &[&state.value[..]]);
        }
        state
    }

    fn next(&mut self) -> k256::elliptic_curve::zeroize::Zeroizing<[u8; 32]> {
        self.value = Self::hmac(&self.key[..], &[&self.value[..]]);
        let candidate = self.value.clone();
        // The rejection/zero-signature retry step from RFC6979 3.2.h. The next
        // call advances rather than returning the same invalid candidate.
        self.key = Self::hmac(&self.key[..], &[&self.value[..], &[0]]);
        self.value = Self::hmac(&self.key[..], &[&self.value[..]]);
        candidate
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use hex_literal::hex;

    #[test]
    fn can_create_pair() {
        let seckey = hex!("4380de832af797688026ce24f85204d508243f201650c1a134929e5458b7fbae");
        let expected = hex!("4380de832af797688026ce24f85204d508243f201650c1a134929e5458b7fbae03fd8c74f795ced92064b86191cb2772b1e3a0947740aa0a5a6e379592471fd85b");
        let res = ext_secp_from_seed(&seckey);

        assert_eq!(res[..], expected[..]);
    }

    #[test]
    fn can_pub_compress_full() {
        let pubkey = hex!("04b9dc646dd71118e5f7fda681ad9eca36eb3ee96f344f582fbe7b5bcdebb1307763fe926c273235fd979a134076d00fd1683cbd35868cb485d4a3a640e52184af");
        let expected = hex!("03b9dc646dd71118e5f7fda681ad9eca36eb3ee96f344f582fbe7b5bcdebb13077");
        let res = ext_secp_pub_compress(&pubkey);

        assert_eq!(res[..], expected[..]);
    }

    #[test]
    fn can_pub_expand_comp() {
        let pubkey = hex!("03b9dc646dd71118e5f7fda681ad9eca36eb3ee96f344f582fbe7b5bcdebb13077");
        let expected = hex!("04b9dc646dd71118e5f7fda681ad9eca36eb3ee96f344f582fbe7b5bcdebb1307763fe926c273235fd979a134076d00fd1683cbd35868cb485d4a3a640e52184af");
        let res = ext_secp_pub_expand(&pubkey);

        assert_eq!(res[..], expected[..]);
    }

    #[test]
    fn can_recover() {
        let expected = hex!("028d13da15a02f3a70677339d51b14177ee9b49657662b35e56a9d9dee17db1d30");
        let sig = hex!("7505f2880114da51b3f5d535f8687953c0ab9af4ab81e592eaebebf53b728d2b6dfd9b5bcd70fee412b1f31360e7c2774009305cb84fc50c1d0ff8034dfa5fff");
        let msg = hex!("a30b64ce1eedf409c8afb801d72c05234e64849ea538c15dd3c8cf4ffcf166c9");
        let res = ext_secp_recover(&msg, &sig, 0);

        assert_eq!(res[..], expected[..]);
    }

    #[test]
    fn can_sign() {
        // JS expectation - doesn't match?
        // let expected = hex!("df92f73d9f060cefacf187b5414491cb992998ace017fa48839b5cda3e264ba8c4efa521361678d9b8582744d77aa4b8d886d7380b7808a683174afad9c4700300");
        let expected = hex!("df92f73d9f060cefacf187b5414491cb992998ace017fa48839b5cda3e264ba83b105adec9e9872647a7d8bb28855b45e22805aea3d097953cbb1391f671d13e01");
        let seckey = hex!("4380de832af797688026ce24f85204d508243f201650c1a134929e5458b7fbae");
        let msg = hex!("68c731589a583d08b70861683b59ce3dd56284cb2f0da5b6cd83e6641dac3aab");
        let res = ext_secp_sign(&msg, &seckey);

        assert_eq!(res[..], expected[..]);
    }

    #[test]
    #[should_panic(expected = "Non-normalized signature provided.")]
    fn rejects_high_s_signature() {
        // valid ECDSA signature with a high S value (non-normalized)
        let sig = hex!(
            "fa1b7a71500322d70bcd5b982c678c96c95f68893063e1621b8b85c867b80b1caae8c7c4f8b5c6d181e4c8898a3f4c329da0e2c21f6f80552a660e6c1d9fa201");
        let msg = hex!("9a3c9be3a14a94df8cda1c474c0e9f8b1b35b1d85d2122cb87e6eb3c518e3be3");

        let _ = ext_secp_recover(&msg, &sig, 0);
    }
}

#[cfg(test)]
mod boundary_tests {
    use super::*;
    use hex_literal::hex;

    #[test]
    fn nonce_retry_matches_independent_c_reference() {
        // Recorded from secp256k1-sys 0.4.2's C hash_impl.h generator, independently
        // compiled with cc. SHA256 of that header: 5638c72220be432175d8401fd0bcea0512d4ba0d5c95e8ad710b69917068b0e9.
        let secret = hex!("0000000000000000000000000000000000000000000000000000000000000001");
        let digest = hex!("fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141");
        let mut generator = RawDigestNonce::new(&secret, &digest);
        assert_eq!(
            *generator.next(),
            hex!("0d9b7698bb66c2fd7edf1e91fe90d1c6247b694bd34cf1322fcf94e074a2c518")
        );
        assert_eq!(
            *generator.next(),
            hex!("aae11b6fdf1da3ce91cba3553671815c23c48bf6f997daabf4bd4ff4474d23b3")
        );
        assert_eq!(
            *generator.next(),
            hex!("54a641076afbce1882fafe98ed01042068f349075c6b384f5e4591693f53fffa")
        );
    }

    #[test]
    fn preserves_published_raw_digest_nonce_boundary_signatures() {
        // Public synthetic key 1, captured from the published 7.5.4 Wasm.
        let secret = hex!("0000000000000000000000000000000000000000000000000000000000000001");
        assert_eq!(ext_secp_sign(&hex!("fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141"), &secret), hex!("584d928bc02fb43cb889788304f47adb538f03a963fbe649a4112398adca0b704dd3158c4f4d7e45a5ec3a41bd407d2822e0302a4342ec5279dde7cf90175eef01"));
        assert_eq!(ext_secp_sign(&hex!("fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364142"), &secret), hex!("a9504c51587ef19a650701945cd2afe99c5817afbbf554a9cb0203232dd17cac61cb99547455e5b68881cbb06e63b1096d6071826aca811a1a8680661f00d3ca01"));
        assert_eq!(ext_secp_sign(&hex!("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"), &secret), hex!("3f8fe493cf305a7f02b2d2c060ba66a8f7bd13a7a64d5200c0655ad069bd85b51cf94236c3857e33a1023a5216cbc81b1dc3adcc1c71f4212df1997ffdfb140a01"));
    }
}
