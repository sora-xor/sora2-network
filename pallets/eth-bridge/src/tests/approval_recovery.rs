use super::mock::EthBridge;
use crate::offchain::SignatureParams;
use secp256k1::{Message, PublicKey, RecoveryId, SecretKey, Signature};
use sp_core::ecdsa;
use sp_runtime::{traits::IdentifyAccount, MultiSigner};

const ORDER: [u8; 32] =
    hex_literal::hex!("fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141");

fn decode<const N: usize>(hex: &str) -> [u8; N] {
    assert_eq!(hex.len(), N * 2);
    core::array::from_fn(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap())
}

fn scalar(value: u8) -> [u8; 32] {
    let mut result = [0; 32];
    result[31] = value;
    result
}

fn scalar_edges() -> [[u8; 32]; 6] {
    let mut below = ORDER;
    below[31] -= 1;
    let mut above = ORDER;
    above[31] += 1;
    [[0; 32], scalar(1), below, ORDER, above, [255; 32]]
}

fn signature(bytes: [u8; 65]) -> SignatureParams {
    SignatureParams {
        r: bytes[..32].try_into().unwrap(),
        s: bytes[32..64].try_into().unwrap(),
        v: bytes[64],
    }
}

// Independent bridge-domain guard: the host itself also accepts 2/3 and 29/30.
fn canonical_bytes(signature: &SignatureParams) -> Option<[u8; 65]> {
    let mut bytes = [0; 65];
    bytes[..32].copy_from_slice(&signature.r);
    bytes[32..64].copy_from_slice(&signature.s);
    bytes[64] = match signature.v {
        0 | 1 => signature.v,
        27 | 28 => signature.v - 27,
        _ => return None,
    };
    Some(bytes)
}

fn local_recovery(message: &Message, signature: &SignatureParams) -> Option<[u8; 33]> {
    let bytes = canonical_bytes(signature)?;
    let signature = Signature::parse_standard_slice(&bytes[..64]).ok()?;
    let recovery_id = RecoveryId::parse(bytes[64]).ok()?;
    secp256k1::recover(message, &signature, &recovery_id)
        .ok()
        .map(|key| key.serialize_compressed())
}

fn guarded_host_recovery(message: &Message, signature: &SignatureParams) -> Option<[u8; 33]> {
    let bytes = canonical_bytes(signature)?;
    // Preserve strict scalar parsing even if host implementation/version changes.
    Signature::parse_standard_slice(&bytes[..64]).ok()?;
    // The default host binding is v2. Message::parse already reduces the digest.
    sp_io::crypto::secp256k1_ecdsa_recover_compressed(&bytes, &message.serialize()).ok()
}

fn assert_recovery_agrees(message: &Message, signature: &SignatureParams) -> Option<[u8; 33]> {
    let expected = local_recovery(message, signature);
    assert_eq!(
        guarded_host_recovery(message, signature),
        expected,
        "host/local mismatch for {signature:?}, digest {:?}",
        message.serialize(),
    );
    assert_eq!(
        EthBridge::recover_prepared_message_signer(message, signature).map(|key| key.0),
        expected,
        "production/local mismatch for {signature:?}, digest {:?}",
        message.serialize(),
    );
    expected
}

fn high_s(signature: &SignatureParams) -> SignatureParams {
    let mut result = signature.clone();
    let mut borrow = false;
    for i in (0..32).rev() {
        let (difference, first_borrow) = ORDER[i].overflowing_sub(signature.s[i]);
        let (difference, second_borrow) = difference.overflowing_sub(u8::from(borrow));
        result.s[i] = difference;
        borrow = first_borrow || second_borrow;
    }
    assert!(!borrow);
    result.v ^= 1;
    result
}

fn assert_wire_forms(message: &Message, low: &SignatureParams, expected: [u8; 33]) {
    let high = high_s(low);
    assert!(low.s < high.s, "the signing fixture must produce low S");
    for mut proof in [low.clone(), high] {
        for offset in [0, 27] {
            let raw_v = proof.v;
            proof.v += offset;
            assert_eq!(assert_recovery_agrees(message, &proof), Some(expected));
            proof.v = raw_v;
        }
    }
}

#[test]
fn guarded_host_recovery_preserves_historical_signature_corpus() {
    // Public deterministic fixture keys, captured from libsecp256k1 0.7.2.
    // Reuse fixed signatures and expected keys, rather than regenerating an oracle.
    let corpus = include_str!(
        "../../../../vendor/sora-secp256k1-compat/tests/fixtures/legacy-signatures.txt"
    );
    let mut digests = std::collections::BTreeSet::new();
    for line in corpus.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        assert_eq!(fields.len(), 4);
        let digest = decode(fields[1]);
        digests.insert(digest);
        let message = Message::parse(&digest);
        let proof = signature(decode(fields[2]));
        assert_wire_forms(&message, &proof, decode(fields[3]));
    }
    for digest in [
        scalar_edges()[0],
        scalar_edges()[2],
        ORDER,
        scalar_edges()[4],
        [255; 32],
    ] {
        assert!(digests.contains(&digest), "missing digest boundary fixture");
    }
    assert_eq!(Message::parse(&ORDER).serialize(), [0; 32]);
    assert_eq!(Message::parse(&scalar_edges()[4]).serialize(), scalar(1));
}

#[test]
fn guarded_host_recovery_matches_distinct_keys_digest_boundaries_and_high_s() {
    for index in 1..=8 {
        let secret = SecretKey::parse(&scalar(index)).unwrap();
        let expected = PublicKey::from_secret_key(&secret).serialize_compressed();
        for digest in scalar_edges() {
            let message = Message::parse(&digest);
            let (signed, id) = secp256k1::sign(&message, &secret);
            let mut bytes = [0; 65];
            bytes[..64].copy_from_slice(&signed.serialize());
            bytes[64] = id.serialize();
            assert_wire_forms(&message, &signature(bytes), expected);
        }
    }
}

#[test]
fn guarded_host_recovery_preserves_strict_scalar_boundaries() {
    for r in scalar_edges() {
        for s in scalar_edges() {
            let proof = SignatureParams { r, s, v: 0 };
            let bytes = canonical_bytes(&proof).unwrap();
            assert_eq!(
                Signature::parse_standard_slice(&bytes[..64]).is_ok(),
                r < ORDER && s < ORDER,
                "strict parsing must not reduce out-of-range scalars",
            );
            for digest in scalar_edges() {
                let message = Message::parse(&digest);
                for v in [0, 1, 27, 28] {
                    let proof = SignatureParams { r, s, v };
                    let recovered = assert_recovery_agrees(&message, &proof);
                    if r == [0; 32] || s == [0; 32] || r >= ORDER || s >= ORDER {
                        assert_eq!(recovered, None);
                    }
                }
            }
        }
    }
}

#[test]
fn guarded_host_recovery_matches_random_noncurve_and_infinity_cases() {
    let mut accepted = 0;
    let mut rejected = 0;
    for index in 0u32..96 {
        let derive = |domain: u8| {
            let mut input = [0; 5];
            input[0] = domain;
            input[1..].copy_from_slice(&index.to_le_bytes());
            sp_io::hashing::keccak_256(&input)
        };
        let message = Message::parse(&derive(0));
        for v in [0, 1] {
            let proof = SignatureParams {
                r: derive(1),
                s: derive(2),
                v,
            };
            if assert_recovery_agrees(&message, &proof).is_some() {
                accepted += 1;
            } else {
                rejected += 1;
            }
        }
    }
    assert!(
        accepted > 0 && rejected > 0,
        "corpus must exercise both results"
    );

    let message = Message::parse(&scalar(1));
    for x in [5, 7, 9, 10, 11, 15, 17, 18] {
        for v in [0, 1, 27, 28] {
            let mut point = [0; 33];
            point[0] = 2 + (v % 27);
            point[1..].copy_from_slice(&scalar(x));
            assert!(PublicKey::parse_compressed(&point).is_err());
            let proof = SignatureParams {
                r: scalar(x),
                s: scalar(1),
                v,
            };
            assert_eq!(assert_recovery_agrees(&message, &proof), None);
        }
    }
    // R=G, s=z=1 => r^-1(sR-zG) is the point at infinity, not a public key.
    let infinity = SignatureParams {
        r: hex_literal::hex!("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"),
        s: scalar(1),
        v: 0,
    };
    assert_eq!(assert_recovery_agrees(&message, &infinity), None);
}

#[test]
fn guarded_host_recovery_rejects_every_non_bridge_recovery_id() {
    let secret = SecretKey::parse(&scalar(1)).unwrap();
    let (mut proof, _) = EthBridge::sign_message(b"recovery-id boundary", &secret);
    let message = common::eth::prepare_message(b"recovery-id boundary");
    let mut accepted = 0;
    for v in 0..=255 {
        proof.v = v;
        let result = assert_recovery_agrees(&message, &proof);
        if matches!(v, 0 | 1 | 27 | 28) {
            assert!(result.is_some());
            accepted += 1;
        } else {
            assert_eq!(result, None, "bridge must reject v={v}");
        }
    }
    assert_eq!(accepted, 4);
}

#[test]
fn production_approval_verification_preserves_message_key_and_account_binding() {
    let payload = b"SORA Nexus bridge approval binding";
    let secret = SecretKey::parse(&scalar(1)).unwrap();
    let (low, key) = EthBridge::sign_message(payload, &secret);
    let key = ecdsa::Public::from_raw(key.serialize_compressed());
    let author = MultiSigner::Ecdsa(key.clone()).into_account();
    let wrong_key = ecdsa::Public::from_raw(
        PublicKey::from_secret_key(&SecretKey::parse(&scalar(2)).unwrap()).serialize_compressed(),
    );
    let wrong_author = MultiSigner::Ecdsa(wrong_key.clone()).into_account();
    for mut proof in [low.clone(), high_s(&low)] {
        for offset in [0, 27] {
            let raw_v = proof.v;
            proof.v += offset;
            assert!(EthBridge::verify_message(payload, &proof, &key, &author));
            assert!(!EthBridge::verify_message(
                payload,
                &proof,
                &key,
                &wrong_author
            ));
            assert!(!EthBridge::verify_message(
                payload, &proof, &wrong_key, &author
            ));
            assert!(!EthBridge::verify_message(
                payload,
                &proof,
                &wrong_key,
                &wrong_author
            ));
            assert!(!EthBridge::verify_message(
                b"other approval",
                &proof,
                &key,
                &author
            ));
            proof.v = raw_v;
        }
    }
    let mut wrong_parity = low.clone();
    wrong_parity.v ^= 1;
    let recovered = assert_recovery_agrees(&common::eth::prepare_message(payload), &wrong_parity);
    assert!(
        recovered.is_some(),
        "wrong parity can recover a valid different key"
    );
    assert_ne!(recovered, Some(key.0));
    assert!(!EthBridge::verify_message(
        payload,
        &wrong_parity,
        &key,
        &author
    ));

    let mut noncurve = [0; 33];
    noncurve[0] = 2;
    noncurve[32] = 5;
    let mut invalid_prefix = key.0;
    invalid_prefix[0] = 4;
    for bytes in [[0; 33], [255; 33], noncurve, invalid_prefix] {
        assert!(PublicKey::parse_compressed(&bytes).is_err());
        let malformed_key = ecdsa::Public::from_raw(bytes);
        let matching_author = MultiSigner::Ecdsa(malformed_key.clone()).into_account();
        assert!(!EthBridge::verify_message(
            payload,
            &low,
            &malformed_key,
            &matching_author
        ));
    }
    for v in 0..=255 {
        if !matches!(v, 0 | 1 | 27 | 28) {
            let mut proof = low.clone();
            proof.v = v;
            assert!(!EthBridge::verify_message(payload, &proof, &key, &author));
        }
    }
    for scalar in [[0; 32], ORDER, scalar_edges()[4], [255; 32]] {
        for proof in [
            SignatureParams {
                r: scalar,
                ..low.clone()
            },
            SignatureParams {
                s: scalar,
                ..low.clone()
            },
        ] {
            assert!(!EthBridge::verify_message(payload, &proof, &key, &author));
        }
    }
}
