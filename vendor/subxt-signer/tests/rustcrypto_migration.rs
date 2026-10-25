#![cfg(feature = "unstable-eth")]

use subxt_signer::eth::{self, dev, DerivationPath, Keypair};

#[test]
fn bip32_migration_preserves_wallet_keys_and_signatures() {
    // Public development fixtures from the upstream Moonbeam compatibility tests.
    let fixtures = [
        (
            dev::alith(),
            "5fb92d6e98884f76de468fa3f6278f8807c48bebc13595d45af5bdc4da702133",
        ),
        (
            dev::baltathar(),
            "8075991ce870b93a8870eca0c0f91913d12f47948ca0fd25b49c6fa7cdbeee8b",
        ),
    ];
    let message = b"SORA Nexus dependency migration compatibility vector";
    for (derived, expected_secret) in fixtures {
        let expected =
            Keypair::from_secret_key(hex::decode(expected_secret).unwrap().try_into().unwrap())
                .unwrap();
        assert_eq!(derived.public_key().0, expected.public_key().0);
        assert_eq!(derived.sign(message), expected.sign(message));
        assert!(eth::verify(
            &derived.sign(message),
            message,
            &expected.public_key()
        ));
        assert!(!eth::verify(
            &derived.sign(message),
            b"modified message",
            &expected.public_key()
        ));
    }
    assert!(Keypair::from_seed(&[0u8; 15]).is_err());
    assert!("m/2147483648".parse::<DerivationPath>().is_err());
}
