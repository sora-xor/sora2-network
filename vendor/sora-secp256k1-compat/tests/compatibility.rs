use sora_secp256k1_compat::{
    recover, sign, verify, Error, Message, PublicKey, PublicKeyFormat, RecoveryId, SecretKey,
    Signature,
};

fn decode<const N: usize>(hex: &str) -> [u8; N] {
    assert_eq!(hex.len(), N * 2);
    core::array::from_fn(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap())
}

const ORDER: [u8; 32] = [
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfe,
    0xba, 0xae, 0xdc, 0xe6, 0xaf, 0x48, 0xa0, 0x3b, 0xbf, 0xd2, 0x5e, 0x8c, 0xd0, 0x36, 0x41, 0x41,
];

#[test]
fn historical_deterministic_signatures_and_recovery_ids_are_unchanged() {
    // Captured from unmodified libsecp256k1 0.7.2, including digest n-1/n/n+1.
    for line in include_str!("fixtures/legacy-signatures.txt").lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        let secret = SecretKey::parse(&decode(fields[0])).unwrap();
        let message = Message::parse(&decode(fields[1]));
        let expected: [u8; 65] = decode(fields[2]);
        let public = PublicKey::parse_compressed(&decode(fields[3])).unwrap();
        let (signature, id) = sign(&message, &secret);
        assert_eq!(signature.serialize(), expected[..64]);
        assert_eq!(id.serialize(), expected[64]);
        assert_eq!(PublicKey::from_secret_key(&secret), public);
        assert!(verify(&message, &signature, &public));
        assert_eq!(recover(&message, &signature, &id).unwrap(), public);
        assert!(!verify(&Message::parse(&[17; 32]), &signature, &public));
    }
}

#[test]
fn high_s_signatures_retain_verification_and_recovery() {
    let secret = SecretKey::parse(&[1; 32]).unwrap();
    let message = Message::parse(&[42; 32]);
    let public = PublicKey::from_secret_key(&secret);
    let (low, id) = sign(&message, &secret);
    let parsed = k256::ecdsa::Signature::from_slice(&low.serialize()).unwrap();
    let mut bytes = low.serialize();
    bytes[32..].copy_from_slice(&(-*parsed.s()).to_bytes());
    let high = Signature::parse_standard(&bytes).unwrap();
    assert_ne!(high, low);
    assert!(verify(&message, &high, &public));
    assert_eq!(
        recover(
            &message,
            &high,
            &RecoveryId::parse(id.serialize() ^ 1).unwrap()
        )
        .unwrap(),
        public
    );
}

#[test]
fn overflowing_signature_parsing_preserves_v1_host_function_semantics() {
    let mut bytes = [0; 64];
    bytes[..32].copy_from_slice(&ORDER);
    bytes[32..].copy_from_slice(&ORDER);
    bytes[31] += 2;
    bytes[63] += 1;
    assert_eq!(
        Signature::parse_standard(&bytes),
        Err(Error::InvalidSignature)
    );
    let reduced = Signature::parse_overflowing(&bytes);
    let mut expected = [0; 64];
    expected[31] = 2;
    expected[63] = 1;
    assert_eq!(reduced.serialize(), expected);
    assert_eq!(Message::parse(&ORDER).serialize(), [0; 32]);

    // All four keys, including the rare recovery IDs 2/3, are historical fixtures.
    let keys = [
        "04a31146ae7cf28ba70a1376d7d9daa815268614d9d60bde7bfcf2c5a4c9ccb1406cef6932fc2ad6338953b592a0015cb8888ab63a2d549fcbceeae77b2bec9113",
        "04543bd18f68c903b573a7ac73a4e824ed1f7fee453128249694b497032981172dfc0b6a6a1dd5f3e9c1eb7d57fc3cf19b5dd140b3b44cd209076e74a8758d5050",
        "0455af6054167b290ad668b164794c097ae937cb3bac4a754b23c34cadff884a92c82993f0d9b17852426c7d74eed5806dc593f1ae7a55eb804f8175bd738cf793",
        "04a38bd5d4fa139ad33f08ee2cbb5715d4374b2bf034e1c2316446128d24441a7f3ae2596a9467839541ab53c0ad7617037e6f2230cf5cae9a21f97c91b7afffb3",
    ];
    for (id, expected) in keys.iter().enumerate() {
        let public = recover(
            &Message::parse(&[42; 32]),
            &reduced,
            &RecoveryId::parse(id as u8).unwrap(),
        )
        .unwrap();
        assert_eq!(public.serialize(), decode::<65>(expected));
        assert!(verify(&Message::parse(&[42; 32]), &reduced, &public));
    }
}

#[test]
fn zero_scalars_parse_but_never_verify_or_recover() {
    let zero = Signature::parse_standard(&[0; 64]).unwrap();
    let message = Message::parse(&[0; 32]);
    let public = PublicKey::from_secret_key(&SecretKey::parse(&[1; 32]).unwrap());
    assert!(!verify(&message, &zero, &public));
    for id in 0..4 {
        assert_eq!(
            recover(&message, &zero, &RecoveryId::parse(id).unwrap()),
            Err(Error::InvalidSignature)
        );
    }
    assert!(SecretKey::parse(&[0; 32]).is_err());
    assert!(SecretKey::parse(&ORDER).is_err());
    assert!(SecretKey::parse(&[255; 32]).is_err());
}

#[test]
fn historical_public_key_encodings_validate_curve_and_hybrid_parity() {
    let public = PublicKey::from_secret_key(&SecretKey::parse(&[1; 32]).unwrap());
    let full = public.serialize();
    assert_eq!(
        PublicKey::parse_slice(&full[1..], Some(PublicKeyFormat::Raw)),
        Ok(public)
    );
    assert_eq!(
        PublicKey::parse_slice(&public.serialize_compressed(), None),
        Ok(public)
    );
    for tag in 0..=255 {
        let mut encoded = full;
        encoded[0] = tag;
        assert_eq!(
            PublicKey::parse(&encoded).is_ok(),
            tag == 4 || tag == 6 + (full[64] & 1)
        );
    }
    let mut off_curve = full;
    off_curve[64] ^= 1;
    assert!(PublicKey::parse(&off_curve).is_err());
    assert!(PublicKey::parse_slice(&full, Some(PublicKeyFormat::Compressed)).is_err());
    assert!(PublicKey::parse_compressed(&[0; 33]).is_err());
}

#[test]
fn malformed_lengths_and_recovery_ids_are_rejected() {
    for len in 0..=70 {
        let bytes = vec![1; len];
        assert_eq!(Message::parse_slice(&bytes).is_ok(), len == 32);
        assert_eq!(SecretKey::parse_slice(&bytes).is_ok(), len == 32);
        assert_eq!(Signature::parse_standard_slice(&bytes).is_ok(), len == 64);
        assert_eq!(
            Signature::parse_overflowing_slice(&bytes).is_ok(),
            len == 64
        );
    }
    for id in 0..=255 {
        assert_eq!(RecoveryId::parse(id).is_ok(), id < 4);
        assert_eq!(RecoveryId::parse_rpc(id).is_ok(), (27..=30).contains(&id));
    }
}
