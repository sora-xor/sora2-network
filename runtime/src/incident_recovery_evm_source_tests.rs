//! Public deterministic fixtures only. Never use this seed for production keys.
use super::*;
use sp_core::{ecdsa, Pair};

const PUBLIC_SEED: [u8; 32] = [0x46; 32];

fn unhex(text: &str) -> Vec<u8> {
    assert_eq!(text.len() % 2, 0);
    text.as_bytes()
        .chunks(2)
        .map(|pair| {
            let digit = |x| match x {
                b'0'..=b'9' => x - b'0',
                b'a'..=b'f' => x - b'a' + 10,
                _ => panic!("fixture hex"),
            };
            digit(pair[0]) * 16 + digit(pair[1])
        })
        .collect()
}

fn expected_actor() -> H160 {
    H160::from_slice(&unhex("9d8a62f656a8d1615c1294fd71e9cfb3e4855a4f"))
}

fn unsigned_fields(kind: u8) -> Vec<Vec<u8>> {
    let n = |value: u64| rlp::encode(&value).to_vec();
    let address = rlp::encode(&&[0x35u8; 20][..]).to_vec();
    let empty = vec![0x80];
    let access = vec![0xc0];
    match kind {
        0 => vec![
            n(9),
            n(20_000_000_000),
            n(21_000),
            address,
            n(1_000_000_000_000_000_000),
            empty,
        ],
        1 => vec![
            n(1),
            n(9),
            n(20_000_000_000),
            n(100_000),
            address,
            n(1),
            empty,
            access,
        ],
        2 => vec![
            n(1),
            n(9),
            n(1),
            n(20_000_000_000),
            n(100_000),
            address,
            n(1),
            empty,
            access,
        ],
        3 => {
            let mut hashes = RlpStream::new_list(1);
            let mut versioned = [0x42u8; 32];
            versioned[0] = 1;
            hashes.append(&&versioned[..]);
            vec![
                n(1),
                n(9),
                n(1),
                n(20_000_000_000),
                n(100_000),
                address,
                n(1),
                empty,
                access,
                n(7),
                hashes.out().to_vec(),
            ]
        }
        4 => {
            // The outer actor is not this authorization address. A structurally
            // well-formed invalid authorization may be skipped during execution.
            let mut authorizations = RlpStream::new_list(1);
            authorizations.begin_list(6);
            authorizations.append(&0u64);
            authorizations.append(&&[0x99u8; 20][..]);
            authorizations.append(&0u64);
            authorizations.append(&7u8); // invalid inner parity is skip-able
            authorizations.append(&0u8);
            authorizations.append(&0u8);
            vec![
                n(1),
                n(9),
                n(1),
                n(20_000_000_000),
                n(100_000),
                address,
                n(1),
                empty,
                access,
                authorizations.out().to_vec(),
            ]
        }
        _ => unreachable!(),
    }
}

fn body(fields: &[Vec<u8>]) -> Vec<u8> {
    let mut stream = RlpStream::new_list(fields.len());
    for field in fields {
        stream.append_raw(field, 1);
    }
    stream.out().to_vec()
}

fn signed(kind: u8, fields: &[Vec<u8>], protected: bool) -> Vec<u8> {
    // Independent EIP signing preimage construction; deliberately does not call
    // the production parser or its signing_hash helper.
    let mut signing = fields.to_vec();
    if kind == 0 && protected {
        signing.extend([vec![1], vec![0x80], vec![0x80]]);
    }
    let mut preimage = body(&signing);
    if kind != 0 {
        preimage.insert(0, kind);
    }
    let signature =
        ecdsa::Pair::from_seed(&PUBLIC_SEED).sign_prehashed(&sp_io::hashing::keccak_256(&preimage));
    let mut complete = fields.to_vec();
    let parity = signature.0[64];
    assert!(parity <= 1);
    complete.push(
        rlp::encode(
            &(if kind == 0 {
                parity + if protected { 37 } else { 27 }
            } else {
                parity
            }),
        )
        .to_vec(),
    );
    for scalar in [&signature.0[..32], &signature.0[32..64]] {
        let start = scalar
            .iter()
            .position(|byte| *byte != 0)
            .unwrap_or(scalar.len());
        complete.push(rlp::encode(&&scalar[start..]).to_vec());
    }
    let mut result = body(&complete);
    if kind != 0 {
        result.insert(0, kind);
    }
    result
}

fn verify(raw: &[u8]) -> Result<AuthenticatedEvmSource, RecoveryError> {
    authenticated_transaction_source(raw, H256::from(sp_io::hashing::keccak_256(raw)))
}

fn replace_field(raw: &[u8], index: usize, replacement: Vec<u8>) -> Vec<u8> {
    let kind = if raw[0] >= 0xc0 { 0 } else { raw[0] };
    let payload = if kind == 0 { raw } else { &raw[1..] };
    let list = rlp::Rlp::new(payload);
    let mut fields = (0..list.item_count().unwrap())
        .map(|i| list.at(i).unwrap().as_raw().to_vec())
        .collect::<Vec<_>>();
    fields[index] = replacement;
    let mut result = body(&fields);
    if kind != 0 {
        result.insert(0, kind);
    }
    result
}

#[test]
fn evm_source_eip155_published_vector_matches_known_signing_hash_and_actor() {
    // EIP-155 canonical published example, not a vector generated by the parser.
    let raw = unhex("f86c098504a817c800825208943535353535353535353535353535353535353535880de0b6b3a76400008025a028ef61340bd939bc2195fe537567866003e1a15d3c71ff63e1590620aa636276a067cbe9d8997f761aecb703304b3800ccf555c9f3dc64214b297fb1966a3b6d83");
    assert_eq!(
        signing_hash(&signing_fields(&raw).unwrap()),
        H256::from_slice(&unhex(
            "daf5a779ae972f972197303d7b574746c7ef83eadac0f2791ad23db92e4c8e53"
        ))
    );
    assert_eq!(
        verify(&raw).unwrap(),
        AuthenticatedEvmSource {
            signer: expected_actor(),
            destination: Some(H160::repeat_byte(0x35)),
            chain_id: Some(U256::one()),
            transaction_type: 0
        }
    );
}

#[test]
fn evm_source_all_five_envelopes_recover_the_outer_public_actor() {
    for kind in 0..=4 {
        let raw = signed(kind, &unsigned_fields(kind), true);
        assert_eq!(
            verify(&raw).unwrap(),
            AuthenticatedEvmSource {
                signer: expected_actor(),
                destination: Some(H160::repeat_byte(0x35)),
                chain_id: Some(U256::one()),
                transaction_type: kind
            }
        );
    }
    let raw = signed(0, &unsigned_fields(0), false);
    assert_eq!(
        verify(&raw).unwrap(),
        AuthenticatedEvmSource {
            signer: expected_actor(),
            destination: Some(H160::repeat_byte(0x35)),
            chain_id: None,
            transaction_type: 0
        }
    );
}

#[test]
fn evm_source_wrong_raw_transaction_hash_and_changed_signing_domain_reject_or_change_actor() {
    let raw = signed(2, &unsigned_fields(2), true);
    assert_eq!(
        authenticated_transaction_source(&raw, H256::repeat_byte(0x42)),
        Err(RecoveryError::WrongPreimage)
    );
    let changed = replace_field(&raw, 0, vec![2]);
    assert!(authenticated_transaction_source(
        &changed,
        H256::from(sp_io::hashing::keccak_256(&raw))
    )
    .is_err());
    let recovered = verify(&changed).unwrap();
    assert_eq!(recovered.chain_id, Some(U256::from(2u8)));
    assert_ne!(recovered.signer, expected_actor());
}

#[test]
fn evm_source_rejects_zero_overflow_high_s_and_invalid_outer_parity() {
    let raw = signed(2, &unsigned_fields(2), true);
    for (index, value) in [
        (10, vec![0x80]),
        (11, vec![0x80]),
        (9, vec![2]),
        (10, rlp::encode(&&CURVE_ORDER[..]).to_vec()),
        (11, rlp::encode(&&CURVE_ORDER[..]).to_vec()),
    ] {
        assert!(verify(&replace_field(&raw, index, value)).is_err());
    }
    let half_plus_one = U256::from_big_endian(&CURVE_ORDER) / U256::from(2u8) + U256::one();
    let mut s = [0u8; 32];
    half_plus_one.write_as_big_endian(&mut s);
    assert!(verify(&replace_field(&raw, 11, rlp::encode(&&s[..]).to_vec())).is_err());
    for v in [0u8, 1, 26, 29, 34] {
        assert!(verify(&replace_field(
            &signed(0, &unsigned_fields(0), true),
            6,
            rlp::encode(&v).to_vec()
        ))
        .is_err());
    }
}

#[test]
fn evm_source_rejects_noncanonical_rlp_and_malformed_nested_tail() {
    for bytes in [
        vec![0xc2, 0x80, 0xb8],
        vec![0xf8, 1, 0x80],
        vec![0xc2, 0x81, 0],
        vec![0xf9, 0, 56],
        vec![0xc0, 0x80],
    ] {
        assert!(verify(&bytes).is_err());
    }
    let raw = signed(2, &unsigned_fields(2), true);
    assert!(verify(&replace_field(&raw, 1, vec![0x00])).is_err());
    assert!(verify(&replace_field(&raw, 8, vec![0xc2, 0x80, 0xb8])).is_err());
    let mut trailing = raw.clone();
    trailing.push(0x80);
    assert!(verify(&trailing).is_err());
}

#[test]
fn evm_source_rejects_unknown_type_blob_network_wrapper_and_wrong_field_shapes() {
    let blob = signed(3, &unsigned_fields(3), true);
    let wrapper = body(&[
        body(&[blob[1..].to_vec()]),
        vec![0xc0],
        vec![0xc0],
        vec![0xc0],
    ]);
    let mut network_form = vec![3];
    network_form.extend(wrapper);
    assert!(verify(&network_form).is_err());
    let mut unknown = signed(2, &unsigned_fields(2), true);
    unknown[0] = 5;
    assert!(verify(&unknown).is_err());
    assert!(verify(&replace_field(&blob, 10, vec![0xc0])).is_err());
    assert!(verify(&replace_field(&blob, 5, vec![0x80])).is_err());
    let typed = signed(1, &unsigned_fields(1), true);
    assert!(verify(&replace_field(&typed, 7, rlp::encode(&&[1u8][..]).to_vec())).is_err());
}

#[test]
fn evm_source_preserves_duplicate_access_entries_and_skipped_authorizations() {
    let mut fields = unsigned_fields(2);
    let mut entry = RlpStream::new_list(2);
    entry.append(&&[0x11u8; 20][..]);
    entry.begin_list(2);
    entry.append(&&[0x22u8; 32][..]);
    entry.append(&&[0x22u8; 32][..]);
    let entry = entry.out();
    let mut access = RlpStream::new_list(2);
    access.append_raw(&entry, 1);
    access.append_raw(&entry, 1);
    fields[8] = access.out().to_vec();
    assert_eq!(
        verify(&signed(2, &fields, true)).unwrap().signer,
        expected_actor()
    );
    assert_eq!(
        verify(&signed(4, &unsigned_fields(4), true))
            .unwrap()
            .signer,
        expected_actor()
    );
    let mut empty = unsigned_fields(4);
    empty[9] = vec![0xc0];
    assert!(verify(&signed(4, &empty, true)).is_err());
}

#[test]
fn evm_source_enforces_bytes_depth_and_item_bounds_before_recovery() {
    assert_eq!(
        verify(&vec![0; MAX_RAW_TRANSACTION_BYTES + 1]),
        Err(RecoveryError::ChunkBounds)
    );
    let mut nested = vec![0xc0];
    for _ in 0..MAX_RLP_DEPTH + 2 {
        nested = body(&[nested]);
    }
    assert_eq!(verify(&nested), Err(RecoveryError::ChunkBounds));
    let too_many = body(&vec![vec![0x80]; MAX_RLP_NODES + 1]);
    assert_eq!(verify(&too_many), Err(RecoveryError::ChunkBounds));
}

#[test]
fn evm_source_independent_noble_vectors_match_hashes_signer_and_chain_domain() {
    // Static outputs of the separate public Node/Noble implementation.
    let vectors: &[(&str, &str, &str, u8, bool)] = &[
        ("f86c098504a817c800825208943535353535353535353535353535353535353535880de0b6b3a76400008025a028ef61340bd939bc2195fe537567866003e1a15d3c71ff63e1590620aa636276a067cbe9d8997f761aecb703304b3800ccf555c9f3dc64214b297fb1966a3b6d83", "daf5a779ae972f972197303d7b574746c7ef83eadac0f2791ad23db92e4c8e53", "33469b22e9f636356c4160a87eb19df52b7412e8eac32a4a55ffe88ea8350788", 0, true),
        ("f86c098504a817c800825208943535353535353535353535353535353535353535880de0b6b3a7640000801ba08383adc8b8ae116f918fb44ca7ff9dfd8012596a5c130c6246a2cc717ba41cdaa053ddfacf5bd4aa7e46d1575acf52636ea659b91f29e2fb91c75567a279738f38", "f9e36c28c8cb35adba138005c02ab7aa7fbcd891f3139cb2eeed052a51cd2713", "9eb247ec381302e0ac0c3c8d8d14969bb49d31ae3d266274d3112e1a86585d94", 0, false),
        ("01f86701098504a817c800830186a09435353535353535353535353535353535353535350180c001a070b8cc8ea20af0207168bad1cab365d467dd4ac8fadabd77178a69e7ac227ae3a041b2fd6278b148eec2c5b2e660e4a371dc1d4103bc30a48c68ca0bc7a7a4dfab", "1bdc5f24e5f05b4803481dbc80b33bc0a06bfcaf29ea7a5f1d05f6d1dc6c2257", "a4c433b4a953dc4163a9f2755040415269490d57a22915a6c9a387e39bc56bcd", 1, true),
        ("02f8680109018504a817c800830186a09435353535353535353535353535353535353535350180c080a0f06c82c2392bf3e5d0eb0d20952cc8f6bc438d3b899f756b744a71ecfa055667a02995e29db8629c0a7629af4e8a3e2ebd9d14c0fac30f49ddd7079898392df008", "d1b49be6f270d112d2492b518a4b14b77a7438ffbaaf2abb55fb759bb925c9e7", "c71fa8f83d5dbdbf2fa9599c6e7db97166673090198afbe921155a7f9733aaad", 2, true),
        ("03f88b0109018504a817c800830186a09435353535353535353535353535353535353535350180c007e1a0014242424242424242424242424242424242424242424242424242424242424201a0fde5d9c9946a711e1c52e15eaec82ec6052a6b03d5edfdb5bfb7f65c676a3316a04a41831e4de2a8043ace808079bbda19c9d9c006dd96f8b6e75a9f94cdef0d4c", "3cb48174eeee4e78e49b11ae2c160684a7d4ce1284cfcb6591506ee4c8f3c5ec", "08c6f8b9c4622ff1c935d702fbf12cb62c25870f318051c018162e7fba9ec19f", 3, true),
        ("04f8840109018504a817c800830186a09435353535353535353535353535353535353535350180c0dbda809499999999999999999999999999999999999999998007808080a06aa49b764a9fcc03a01a5ca06c16380e2ef4d4531e124d602c01032737b26fefa0181e6a8bcbd5e00eb3edcce3a1d27d942482a2f0268aa5bb342cb505a996e093", "32dd51d6213f16ff51b0e72f7096720f08e7f9aec7e031cddca1fa22b2314ed2", "4b53461660fc3ed5fc7db101e565a6e6d5ff934da9502dc2bdc68be4fdf9921a", 4, true),
    ];
    for &(raw, signing, transaction, kind, protected) in vectors {
        let bytes = unhex(raw);
        let parsed = signing_fields(&bytes).unwrap();
        assert_eq!(signing_hash(&parsed), H256::from_slice(&unhex(signing)));
        assert_eq!(
            sp_io::hashing::keccak_256(&bytes).as_slice(),
            unhex(transaction)
        );
        let source =
            authenticated_transaction_source(&bytes, H256::from_slice(&unhex(transaction)))
                .unwrap();
        assert_eq!(source.signer, expected_actor());
        assert_eq!(source.destination, Some(H160::repeat_byte(0x35)));
        assert_eq!(source.transaction_type, kind);
        assert_eq!(
            source.chain_id,
            if protected { Some(U256::one()) } else { None }
        );
    }
}
