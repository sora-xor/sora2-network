// Historical-reference verification only; not part of the production dependency graph.
fn legacy_recover(m: &[u8; 32], sig: &[u8; 64], id: u8) -> Result<[u8; 65], String> {
    legacy::recover(
        &legacy::Message::parse(m),
        &legacy::Signature::parse_overflowing(sig),
        &legacy::RecoveryId::parse(id).unwrap(),
    )
    .map(|p| p.serialize())
    .map_err(|e| e.to_string())
}
fn modern_recover(m: &[u8; 32], sig: &[u8; 64], id: u8) -> Result<[u8; 65], String> {
    modern::recover(
        &modern::Message::parse(m),
        &modern::Signature::parse_overflowing(sig),
        &modern::RecoveryId::parse(id).unwrap(),
    )
    .map(|p| p.serialize())
    .map_err(|e| e.to_string())
}
fn random_bytes<const N: usize>(state: &mut u64) -> [u8; N] {
    core::array::from_fn(|_| {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        *state as u8
    })
}
fn main() {
    let order: [u8; 32] =
        hex::decode("fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141")
            .unwrap()
            .try_into()
            .unwrap();
    let mut order_before = order;
    order_before[31] -= 1;
    let mut order_after = order;
    order_after[31] += 1;
    let mut state = 0x6f84a51dc928be37;
    let mut checks = 0usize;
    let mut transcript = Vec::new();
    let mut vectors = String::new();
    for case in 0..128 {
        let seed = if case == 0 {
            [1u8; 32]
        } else {
            random_bytes::<32>(&mut state)
        };
        let old = legacy::SecretKey::parse(&seed).unwrap();
        let new = modern::SecretKey::parse(&seed).unwrap();
        let old_pub = legacy::PublicKey::from_secret_key(&old);
        let new_pub = modern::PublicKey::from_secret_key(&new);
        assert_eq!(old_pub.serialize(), new_pub.serialize());
        assert_eq!(
            old_pub.serialize_compressed(),
            new_pub.serialize_compressed()
        );
        for msg in [
            [0; 32],
            [42; 32],
            [255; 32],
            order_before,
            order,
            order_after,
            random_bytes(&mut state),
            random_bytes(&mut state),
        ] {
            let om = legacy::Message::parse(&msg);
            let nm = modern::Message::parse(&msg);
            assert_eq!(om.serialize(), nm.serialize());
            let (os, oid) = legacy::sign(&om, &old);
            let (ns, nid) = modern::sign(&nm, &new);
            assert_eq!(os.serialize(), ns.serialize(), "signature case {case}");
            assert_eq!(oid.serialize(), nid.serialize());
            assert_eq!(
                legacy::verify(&om, &os, &old_pub),
                modern::verify(&nm, &ns, &new_pub)
            );
            let sig = os.serialize();
            let ksig = k256::ecdsa::Signature::from_slice(&sig).unwrap();
            let mut high = sig;
            high[32..].copy_from_slice(&(-*ksig.s()).to_bytes());
            assert!(legacy::verify(
                &om,
                &legacy::Signature::parse_standard(&high).unwrap(),
                &old_pub
            ));
            assert!(modern::verify(
                &nm,
                &modern::Signature::parse_standard(&high).unwrap(),
                &new_pub
            ));
            for (data, id) in [(sig, oid.serialize()), (high, oid.serialize() ^ 1)] {
                assert_eq!(
                    legacy_recover(&msg, &data, id),
                    modern_recover(&msg, &data, id)
                );
                checks += 1;
                transcript.extend_from_slice(&data);
            }
            transcript.extend_from_slice(&new_pub.serialize());
            if case == 0 {
                vectors.push_str(&format!(
                    "{} {} {}{} {}\n",
                    hex::encode(seed),
                    hex::encode(msg),
                    hex::encode(sig),
                    hex::encode([oid.serialize()]),
                    hex::encode(old_pub.serialize_compressed())
                ));
            }
            checks += 4;
        }
        let full = old_pub.serialize();
        for tag in 0..=8 {
            let mut hybrid = full;
            hybrid[0] = tag;
            let a = legacy::PublicKey::parse(&hybrid)
                .map(|p| p.serialize())
                .map_err(|e| e.to_string());
            let b = modern::PublicKey::parse(&hybrid)
                .map(|p| p.serialize())
                .map_err(|e| e.to_string());
            assert_eq!(a, b, "public tag {tag}");
            checks += 1;
        }
    }
    for case in 0..1024 {
        let msg = random_bytes(&mut state);
        let mut sig = random_bytes::<64>(&mut state);
        if case < 64 {
            sig = [0; 64];
            sig[31] = (case / 8) as u8;
            sig[63] = (case % 8) as u8;
        }
        if (64..70).contains(&case) {
            sig[..32].copy_from_slice(&order);
            sig[31] = sig[31].wrapping_add((case - 64) as u8);
        }
        if (70..76).contains(&case) {
            sig[32..].copy_from_slice(&order);
            sig[63] = sig[63].wrapping_add((case - 70) as u8);
        }
        assert_eq!(
            legacy::Signature::parse_standard(&sig)
                .map(|s| s.serialize())
                .map_err(|e| e.to_string()),
            modern::Signature::parse_standard(&sig)
                .map(|s| s.serialize())
                .map_err(|e| e.to_string())
        );
        assert_eq!(
            legacy::Signature::parse_overflowing(&sig).serialize(),
            modern::Signature::parse_overflowing(&sig).serialize()
        );
        for id in 0..4 {
            let a = legacy_recover(&msg, &sig, id);
            let b = modern_recover(&msg, &sig, id);
            assert_eq!(a, b, "recovery case {case} id {id}");
            checks += 1;
            transcript.extend_from_slice(format!("{b:?}").as_bytes());
        }
        checks += 2;
    }
    for n in 0..=255 {
        assert_eq!(
            legacy::RecoveryId::parse(n)
                .map(|r| r.serialize())
                .map_err(|e| e.to_string()),
            modern::RecoveryId::parse(n)
                .map(|r| r.serialize())
                .map_err(|e| e.to_string())
        );
        assert_eq!(
            legacy::RecoveryId::parse_rpc(n)
                .map(|r| r.serialize())
                .map_err(|e| e.to_string()),
            modern::RecoveryId::parse_rpc(n)
                .map(|r| r.serialize())
                .map_err(|e| e.to_string())
        );
        checks += 2;
    }
    for len in 0..=70 {
        let bytes = vec![0; len];
        assert_eq!(
            legacy::Signature::parse_standard_slice(&bytes)
                .map(|s| s.serialize())
                .map_err(|e| e.to_string()),
            modern::Signature::parse_standard_slice(&bytes)
                .map(|s| s.serialize())
                .map_err(|e| e.to_string())
        );
        assert_eq!(
            legacy::SecretKey::parse_slice(&bytes)
                .map(|s| s.serialize())
                .map_err(|e| e.to_string()),
            modern::SecretKey::parse_slice(&bytes)
                .map(|s| s.serialize())
                .map_err(|e| e.to_string())
        );
        checks += 2;
    }
    std::fs::write("signatures.txt", vectors).unwrap();
    std::fs::write("transcript.bin", transcript).unwrap();
    for r in 1..32 {
        let mut small = [0; 64];
        small[31] = r;
        small[63] = 1;
        if legacy_recover(&[42; 32], &small, 2).is_ok() {
            for id in 0..4 {
                println!(
                    "small-scalar-r-{r}-recovery-{id}: {:?}",
                    legacy_recover(&[42; 32], &small, id).map(hex::encode)
                );
            }
            break;
        }
    }
    println!("PASS: {checks} deterministic compatibility checks; 1024 byte-identical signatures");
}
