use crate::mock::{
    child_of_genesis_ethereum_header, child_of_header, ethereum_header_from_file,
    ethereum_header_mix_nonce_from_file, ethereum_header_proof_from_file,
    genesis_ethereum_block_hash, genesis_ethereum_header, log_payload, message_with_receipt_proof,
    new_tester, new_tester_with_config, receipt_root_and_proof, ropsten_london_header,
    ropsten_london_message,
};
use bridge_types::network_config::NetworkConfig as EthNetworkConfig;
use bridge_types::traits::EthereumGasPriceOracle;
use bridge_types::traits::Verifier as VerifierConfig;
use bridge_types::{import_digest, EVMChainId, U256};
use frame_support::pallet_prelude::InvalidTransaction;
use frame_support::unsigned::TransactionValidityError;
use sp_core::sr25519::Pair as PairSr25519;
use sp_core::Pair;
use sp_runtime::traits::{Hash, IdentifyAccount, Keccak256};

use crate::mock::{mock_verifier, mock_verifier_with_pow};

use crate::mock::mock_verifier::{RuntimeOrigin, Test, Verifier};

use crate::{
    BestBlock, Call, Error, EthereumHeader, FinalizedBlock, GenesisConfig, Headers,
    HeadersByNumber, PruningRange,
};
use frame_support::assert_noop;
use frame_support::{assert_err, assert_ok};
use sp_keyring::AccountKeyring as Keyring;
use sp_runtime::traits::ValidateUnsigned;
use sp_runtime::{MultiSignature, MultiSigner};

fn digest_signature<T: crate::Config>(
    signer: &PairSr25519,
    network_id: &EVMChainId,
    header: &EthereumHeader,
) -> MultiSignature {
    sp_runtime::MultiSignature::Sr25519(signer.clone().sign(&import_digest(network_id, header)[..]))
}

#[test]
fn it_tracks_highest_difficulty_ethereum_chain() {
    new_tester::<Test>().execute_with(|| {
        let mut child1 = child_of_genesis_ethereum_header();
        child1.difficulty = 0xbc140caa61087i64.into();
        let child1_hash = child1.compute_hash();
        let mut child2 = child_of_genesis_ethereum_header();
        child2.difficulty = 0x20000.into();
        let network_id = EthNetworkConfig::Ropsten.chain_id();

        let ferdie = Keyring::Ferdie;
        assert_ok!(Verifier::import_header(
            RuntimeOrigin::none(),
            network_id,
            child1.clone(),
            Default::default(),
            Default::default(),
            MultiSigner::from(ferdie.clone()).into_account(),
            digest_signature::<mock_verifier::Test>(&ferdie.pair(), &network_id, &child1),
        ));
        assert_ok!(Verifier::import_header(
            RuntimeOrigin::none(),
            network_id,
            child2.clone(),
            Default::default(),
            Default::default(),
            MultiSigner::from(ferdie.clone()).into_account(),
            digest_signature::<mock_verifier::Test>(&ferdie.pair(), &network_id, &child2),
        ));

        let (header_id, highest_difficulty) =
            <BestBlock<Test>>::get(network_id).expect("Best block not found");
        assert_eq!(header_id.hash, child1_hash);
        assert_eq!(highest_difficulty, 0xbc140caa61087i64.into());
    });
}

#[test]
fn it_tracks_multiple_unfinalized_ethereum_forks() {
    new_tester::<Test>().execute_with(|| {
        let child1 = child_of_genesis_ethereum_header();
        let child1_hash = child1.compute_hash();
        let mut child2 = child1.clone();
        // make child2 have a different hash to child1
        child2.difficulty = 0x20000i64.into();
        let child2_hash = child2.compute_hash();
        let network_id = EthNetworkConfig::Ropsten.chain_id();

        let ferdie = Keyring::Ferdie;
        assert_ok!(Verifier::import_header(
            RuntimeOrigin::none(),
            network_id,
            child1.clone(),
            Default::default(),
            Default::default(),
            MultiSigner::from(ferdie.clone()).into_account(),
            digest_signature::<mock_verifier::Test>(&ferdie.pair(), &network_id, &child1),
        ));
        assert_ok!(Verifier::import_header(
            RuntimeOrigin::none(),
            network_id,
            child2.clone(),
            Default::default(),
            Default::default(),
            MultiSigner::from(ferdie.clone()).into_account(),
            digest_signature::<mock_verifier::Test>(&ferdie.pair(), &network_id, &child2),
        ));

        assert!(<Headers<Test>>::contains_key(network_id, child1_hash));
        assert!(<Headers<Test>>::contains_key(network_id, child2_hash));
        assert_eq!(
            <HeadersByNumber<Test>>::get(network_id, 1).unwrap(),
            vec![child1_hash, child2_hash]
        );
    });
}

#[test]
fn it_tracks_only_one_finalized_ethereum_fork() {
    new_tester::<Test>().execute_with(|| {
        let block1 = child_of_genesis_ethereum_header();
        let block1_hash = block1.compute_hash();
        let block2 = child_of_header(&block1);
        let block2_hash = block2.compute_hash();
        let block3 = child_of_header(&block2);
        let block3_hash = block3.compute_hash();
        let mut block4 = child_of_genesis_ethereum_header();
        block4.difficulty = 2u32.into();
        let mut block5 = child_of_header(&block4);
        block5.difficulty = 3u32.into();
        let mut block6 = child_of_genesis_ethereum_header();
        block6.difficulty = 5u32.into();
        let network_id = EthNetworkConfig::Ropsten.chain_id();

        // Initial chain:
        //   B0
        //   |  \
        //   B1  B4
        //   |
        //   B2
        //   |
        //   B3
        let ferdie = Keyring::Ferdie;
        for header in vec![block1, block4, block2, block3].into_iter() {
            assert_ok!(Verifier::import_header(
                RuntimeOrigin::none(),
                network_id,
                header.clone(),
                Default::default(),
                Default::default(),
                MultiSigner::from(ferdie.clone()).into_account(),
                digest_signature::<mock_verifier::Test>(&ferdie.pair(), &network_id, &header),
            ));
        }
        // Relies on DescendantsUntilFinalized = 2
        assert_eq!(
            <FinalizedBlock<Test>>::get(network_id)
                .expect("Finalized block not found")
                .hash,
            block1_hash
        );
        assert!(
            <Headers<Test>>::get(network_id, block1_hash)
                .unwrap()
                .finalized
        );
        assert!(
            <Headers<Test>>::get(network_id, block2_hash)
                .unwrap()
                .finalized
                == false
        );
        assert_eq!(
            BestBlock::<Test>::get(network_id)
                .expect("Best block not found")
                .0
                .hash,
            block3_hash
        );

        // With invalid forks (invalid since B1 is final):
        //       B0
        //     / | \
        //   B6  B1  B4
        //       |    \
        //       B2    B5
        //       |
        //       B3
        assert_err!(
            Verifier::import_header(
                RuntimeOrigin::none(),
                network_id,
                block5.clone(),
                Default::default(),
                Default::default(),
                MultiSigner::from(ferdie.clone()).into_account(),
                digest_signature::<mock_verifier::Test>(&ferdie.pair(), &network_id, &block5),
            ),
            Error::<Test>::HeaderOnStaleFork,
        );
        assert_err!(
            Verifier::import_header(
                RuntimeOrigin::none(),
                network_id,
                block6.clone(),
                Default::default(),
                Default::default(),
                MultiSigner::from(ferdie.clone()).into_account(),
                digest_signature::<mock_verifier::Test>(&ferdie.pair(), &network_id, &block6),
            ),
            Error::<Test>::AncientHeader,
        );
    });
}

#[test]
fn it_prunes_ethereum_headers_correctly() {
    new_tester::<Test>().execute_with(|| {
        let block1 = child_of_genesis_ethereum_header();
        let block1_hash = block1.compute_hash();
        let block2 = child_of_header(&block1);
        let block2_hash = block2.compute_hash();
        let block3 = child_of_header(&block2);
        let block3_hash = block3.compute_hash();
        let mut block4 = child_of_genesis_ethereum_header();
        block4.difficulty = 2i64.into();
        let block4_hash = block4.compute_hash();
        let network_id = EthNetworkConfig::Ropsten.chain_id();

        // Initial chain:
        //   B0
        //   |  \
        //   B1  B4
        //   |
        //   B2
        //   |
        //   B3
        let ferdie = Keyring::Ferdie;
        for header in vec![block1, block4, block2, block3].into_iter() {
            assert_ok!(Verifier::import_header(
                RuntimeOrigin::none(),
                network_id,
                header.clone(),
                Default::default(),
                Default::default(),
                MultiSigner::from(ferdie.clone()).into_account(),
                digest_signature::<mock_verifier::Test>(&ferdie.pair(), &network_id, &header),
            ));
        }

        // Prune genesis block
        let new_range = Verifier::prune_header_range(
            network_id,
            &PruningRange {
                oldest_unpruned_block: 0,
                oldest_block_to_keep: 1,
            },
            2,
            1,
        );
        assert_eq!(
            new_range,
            PruningRange {
                oldest_unpruned_block: 1,
                oldest_block_to_keep: 1
            },
        );
        assert!(!<Headers<Test>>::contains_key(
            network_id,
            genesis_ethereum_block_hash()
        ));
        assert!(!<HeadersByNumber<Test>>::contains_key(network_id, 0));

        // Prune next block (B1)
        let new_range = Verifier::prune_header_range(
            network_id,
            &PruningRange {
                oldest_unpruned_block: 1,
                oldest_block_to_keep: 1,
            },
            1,
            2,
        );
        assert_eq!(
            new_range,
            PruningRange {
                oldest_unpruned_block: 1,
                oldest_block_to_keep: 2
            },
        );
        assert!(!<Headers<Test>>::contains_key(network_id, block1_hash));
        assert!(<Headers<Test>>::contains_key(network_id, block4_hash));
        assert_eq!(
            <HeadersByNumber<Test>>::get(network_id, 1).unwrap(),
            vec![block4_hash]
        );

        // Prune next two blocks (B4, B2)
        let new_range = Verifier::prune_header_range(
            network_id,
            &PruningRange {
                oldest_unpruned_block: 1,
                oldest_block_to_keep: 2,
            },
            2,
            4,
        );
        assert_eq!(
            new_range,
            PruningRange {
                oldest_unpruned_block: 3,
                oldest_block_to_keep: 4
            },
        );
        assert!(!<Headers<Test>>::contains_key(network_id, block4_hash));
        assert!(!<HeadersByNumber<Test>>::contains_key(network_id, 1));
        assert!(!<Headers<Test>>::contains_key(network_id, block2_hash));
        assert!(!<HeadersByNumber<Test>>::contains_key(network_id, 2));

        // Finally, we're left with B3
        assert!(<Headers<Test>>::contains_key(network_id, block3_hash));
        assert_eq!(
            HeadersByNumber::<Test>::get(network_id, 3).unwrap(),
            vec![block3_hash]
        );
    });
}

#[test]
fn it_imports_ethereum_header_only_once() {
    new_tester::<Test>().execute_with(|| {
        let child = child_of_genesis_ethereum_header();
        let child_for_reimport = child.clone();
        let network_id = EthNetworkConfig::Ropsten.chain_id();

        let ferdie = Keyring::Ferdie;
        assert_ok!(Verifier::import_header(
            RuntimeOrigin::none(),
            network_id,
            child.clone(),
            Default::default(),
            Default::default(),
            MultiSigner::from(ferdie.clone()).into_account(),
            digest_signature::<mock_verifier::Test>(&ferdie.pair(), &network_id, &child),
        ));
        assert_err!(
            Verifier::import_header(
                RuntimeOrigin::none(),
                network_id,
                child_for_reimport.clone(),
                Default::default(),
                Default::default(),
                MultiSigner::from(ferdie.clone()).into_account(),
                digest_signature::<mock_verifier::Test>(
                    &ferdie.pair(),
                    &network_id,
                    &child_for_reimport
                ),
            ),
            Error::<Test>::DuplicateHeader,
        );
    });
}

#[test]
fn it_rejects_wrong_signature() {
    new_tester::<Test>().execute_with(|| {
        let child = child_of_genesis_ethereum_header();
        let ferdie = Keyring::Ferdie;
        let signature_author = Keyring::Eve;
        let child_of_child: EthereumHeader = Default::default();
        let network_id = EthNetworkConfig::Ropsten.chain_id();

        // We call pre_dispatch here because signature verification
        // is performed only there; we don't do it second time in
        // extrinsic itself
        frame_support::assert_noop!(
            Verifier::pre_dispatch(&Call::import_header {
                network_id,
                header: child.clone(),
                proof: Default::default(),
                mix_nonce: Default::default(),
                // Signer/submitter does not match with signature
                submitter: MultiSigner::from(ferdie.clone()).into_account(),
                signature: digest_signature::<mock_verifier::Test>(
                    &signature_author.pair(),
                    &network_id,
                    &child_of_child
                ),
            }),
            TransactionValidityError::Invalid(InvalidTransaction::Custom(
                Error::<Test>::InvalidSignature.into()
            ))
        );
    });
}

#[test]
fn it_rejects_ethereum_header_before_parent() {
    new_tester::<Test>().execute_with(|| {
        let child = child_of_genesis_ethereum_header();
        let mut child_of_child: EthereumHeader = Default::default();
        child_of_child.parent_hash = child.compute_hash();
        let network_id = EthNetworkConfig::Ropsten.chain_id();

        let ferdie = Keyring::Ferdie;
        assert_err!(
            Verifier::import_header(
                RuntimeOrigin::none(),
                network_id,
                child_of_child.clone(),
                Default::default(),
                Default::default(),
                MultiSigner::from(ferdie.clone()).into_account(),
                digest_signature::<mock_verifier::Test>(
                    &ferdie.pair(),
                    &network_id,
                    &child_of_child
                ),
            ),
            Error::<Test>::MissingParentHeader,
        );
    });
}

#[test]
fn it_validates_proof_of_work() {
    new_tester_with_config::<mock_verifier_with_pow::Test>(GenesisConfig {
        initial_networks: vec![(
            EthNetworkConfig::Mainnet,
            ethereum_header_from_file(11090290, ""),
            0u32.into(),
        )],
    })
    .execute_with(|| {
        let header1 = ethereum_header_from_file(11090291, "");
        let header1_proof = ethereum_header_proof_from_file(11090291, "");
        let header1_mix_nonce = ethereum_header_mix_nonce_from_file(11090291, "");
        let header2 = ethereum_header_from_file(11090292, "");
        let network_id = EthNetworkConfig::Mainnet.chain_id();

        let ferdie = Keyring::Ferdie;

        // Incorrect nonce
        assert_err!(
            mock_verifier_with_pow::Verifier::import_header(
                mock_verifier_with_pow::RuntimeOrigin::none(),
                network_id,
                header1.clone(),
                header1_proof.clone(),
                Default::default(),
                MultiSigner::from(ferdie.clone()).into_account(),
                digest_signature::<mock_verifier_with_pow::Test>(
                    &ferdie.pair(),
                    &network_id,
                    &header1
                ),
            ),
            Error::<mock_verifier_with_pow::Test>::InvalidHeader,
        );

        // Incorrect proof
        assert_err!(
            mock_verifier_with_pow::Verifier::import_header(
                mock_verifier_with_pow::RuntimeOrigin::none(),
                network_id,
                header1.clone(),
                Default::default(),
                header1_mix_nonce.clone(),
                MultiSigner::from(ferdie.clone()).into_account(),
                digest_signature::<mock_verifier_with_pow::Test>(
                    &ferdie.pair(),
                    &network_id,
                    &header1
                ),
            ),
            Error::<mock_verifier_with_pow::Test>::InvalidHeader,
        );

        assert_ok!(mock_verifier_with_pow::Verifier::import_header(
            mock_verifier_with_pow::RuntimeOrigin::none(),
            network_id,
            header1.clone(),
            header1_proof,
            header1_mix_nonce,
            MultiSigner::from(ferdie.clone()).into_account(),
            digest_signature::<mock_verifier_with_pow::Test>(&ferdie.pair(), &network_id, &header1),
        ));

        // Both proof & nonce are incorrect
        assert_err!(
            mock_verifier_with_pow::Verifier::import_header(
                mock_verifier_with_pow::RuntimeOrigin::none(),
                network_id,
                header2.clone(),
                Default::default(),
                Default::default(),
                MultiSigner::from(ferdie.clone()).into_account(),
                digest_signature::<mock_verifier_with_pow::Test>(
                    &ferdie.pair(),
                    &network_id,
                    &header2
                ),
            ),
            Error::<mock_verifier_with_pow::Test>::InvalidHeader,
        );
    });
}

#[test]
fn it_rejects_ethereum_header_with_low_difficulty() {
    new_tester_with_config::<mock_verifier_with_pow::Test>(GenesisConfig {
        initial_networks: vec![(
            EthNetworkConfig::Ropsten,
            ethereum_header_from_file(11090291, ""),
            0u32.into(),
        )],
    })
    .execute_with(|| {
        let header = ethereum_header_from_file(11090292, "_low_difficulty");
        let header_proof = ethereum_header_proof_from_file(11090292, "_low_difficulty");
        let header_mix_nonce = ethereum_header_mix_nonce_from_file(11090292, "_low_difficulty");
        let network_id = EthNetworkConfig::Ropsten.chain_id();

        let ferdie = Keyring::Ferdie;
        assert_err!(
            mock_verifier_with_pow::Verifier::import_header(
                mock_verifier_with_pow::RuntimeOrigin::none(),
                network_id,
                header.clone(),
                header_proof,
                header_mix_nonce,
                MultiSigner::from(ferdie.clone()).into_account(),
                digest_signature::<mock_verifier_with_pow::Test>(
                    &ferdie.pair(),
                    &network_id,
                    &header
                ),
            ),
            Error::<mock_verifier_with_pow::Test>::InvalidHeader,
        );
    });
}

#[test]
fn it_confirms_receipt_inclusion_in_finalized_header() {
    let (receipts_root, receipt_proof) = receipt_root_and_proof();
    let mut finalized_header: EthereumHeader = Default::default();
    finalized_header.receipts_root = receipts_root;
    let finalized_header_hash = finalized_header.compute_hash();
    let network_id = EthNetworkConfig::Ropsten.chain_id();

    new_tester_with_config::<Test>(GenesisConfig {
        initial_networks: vec![(EthNetworkConfig::Ropsten, finalized_header, 0u32.into())],
    })
    .execute_with(|| {
        let (message, proof) =
            message_with_receipt_proof(log_payload(), finalized_header_hash, receipt_proof);
        let message_hash = Keccak256::hash_of(&message);
        assert_ok!(Verifier::verify(network_id.into(), message_hash, &proof));
    });
}

#[test]
fn it_confirms_receipt_inclusion_in_ropsten_london_header() {
    let finalized_header: EthereumHeader = ropsten_london_header();

    new_tester_with_config::<Test>(GenesisConfig {
        initial_networks: vec![(EthNetworkConfig::Ropsten, finalized_header, 0u32.into())],
    })
    .execute_with(|| {
        let (message, proof) = ropsten_london_message();
        let message_hash = Keccak256::hash_of(&message);
        assert_ok!(Verifier::verify(
            EthNetworkConfig::Ropsten.chain_id().into(),
            message_hash,
            &proof
        ));
    });
}

#[test]
fn it_denies_receipt_inclusion_for_invalid_proof() {
    new_tester::<Test>().execute_with(|| {
        let (_, receipt_proof) = receipt_root_and_proof();
        let network_id = EthNetworkConfig::Ropsten.chain_id();
        let (message, proof) =
            message_with_receipt_proof(log_payload(), genesis_ethereum_block_hash(), receipt_proof);
        let message_hash = Keccak256::hash_of(&message);
        assert_err!(
            Verifier::verify(network_id.into(), message_hash, &proof),
            Error::<Test>::InvalidProof,
        );
    });
}

#[test]
fn it_denies_receipt_inclusion_for_invalid_log() {
    let (receipts_root, receipt_proof) = receipt_root_and_proof();
    let mut finalized_header: EthereumHeader = Default::default();
    finalized_header.receipts_root = receipts_root;
    let finalized_header_hash = finalized_header.compute_hash();
    let network_id = EthNetworkConfig::Ropsten.chain_id();

    new_tester_with_config::<Test>(GenesisConfig {
        initial_networks: vec![(EthNetworkConfig::Ropsten, finalized_header, 0u32.into())],
    })
    .execute_with(|| {
        // Valid log payload but doesn't exist in receipt
        let mut log = log_payload();
        log[3] = 204;
        let (message, proof) =
            message_with_receipt_proof(log, finalized_header_hash, receipt_proof);
        let message_hash = Keccak256::hash_of(&message);
        assert_err!(
            Verifier::verify(network_id.into(), message_hash, &proof),
            Error::<Test>::InvalidProof,
        );
    })
}

#[test]
fn it_denies_receipt_inclusion_for_invalid_header() {
    new_tester::<Test>().execute_with(|| {
        let log = log_payload();
        let (receipts_root, receipt_proof) = receipt_root_and_proof();
        let mut block1 = child_of_genesis_ethereum_header();
        block1.receipts_root = receipts_root;
        let block1_hash = block1.compute_hash();
        let mut block1_alt = child_of_genesis_ethereum_header();
        block1_alt.receipts_root = receipts_root;
        block1_alt.difficulty = 2i64.into();
        let block1_alt_hash = block1_alt.compute_hash();
        let block2_alt = child_of_header(&block1_alt);
        let block3_alt = child_of_header(&block2_alt);
        let block4_alt = child_of_header(&block3_alt);
        let network_id = EthNetworkConfig::Ropsten.chain_id();

        // Header hasn't been imported yet
        let (message, proof) =
            message_with_receipt_proof(log.clone(), block1_hash, receipt_proof.clone());
        let message_hash = Keccak256::hash_of(&message);
        assert_err!(
            Verifier::verify(network_id.into(), message_hash, &proof),
            Error::<Test>::MissingHeader,
        );

        let ferdie = Keyring::Ferdie;
        assert_ok!(Verifier::import_header(
            RuntimeOrigin::none(),
            network_id,
            block1.clone(),
            Default::default(),
            Default::default(),
            MultiSigner::from(ferdie.clone()).into_account(),
            digest_signature::<mock_verifier::Test>(&ferdie.pair(), &network_id, &block1),
        ));

        // Header has been imported but not finalized
        let (message, proof) =
            message_with_receipt_proof(log.clone(), block1_hash, receipt_proof.clone());
        let message_hash = Keccak256::hash_of(&message);
        assert_err!(
            Verifier::verify(network_id.into(), message_hash, &proof),
            Error::<Test>::HeaderNotFinalized,
        );

        // With alternate fork:
        //   B0
        //   |  \
        //   B1  B1_ALT
        //        \
        //         B2_ALT
        //          \
        //           B3_ALT
        for header in vec![block1_alt, block2_alt, block3_alt].into_iter() {
            assert_ok!(Verifier::import_header(
                RuntimeOrigin::none(),
                network_id,
                header.clone(),
                Default::default(),
                Default::default(),
                MultiSigner::from(ferdie.clone()).into_account(),
                digest_signature::<mock_verifier::Test>(&ferdie.pair(), &network_id, &header),
            ));
        }
        assert_eq!(
            <FinalizedBlock<Test>>::get(network_id,)
                .expect("Finalized block not found")
                .hash,
            block1_alt_hash
        );

        // A finalized header at this height exists, but it's not block1
        let (message, proof) =
            message_with_receipt_proof(log.clone(), block1_hash, receipt_proof.clone());
        let message_hash = Keccak256::hash_of(&message);
        assert_err!(
            Verifier::verify(network_id.into(), message_hash, &proof),
            Error::<Test>::HeaderNotFinalized,
        );

        assert_ok!(Verifier::import_header(
            RuntimeOrigin::none(),
            network_id,
            block4_alt.clone(),
            Default::default(),
            Default::default(),
            MultiSigner::from(ferdie.clone()).into_account(),
            digest_signature::<mock_verifier::Test>(&ferdie.pair(), &network_id, &block4_alt),
        ));

        // A finalized header at a newer height exists, but block1 isn't its ancestor
        let (message, proof) =
            message_with_receipt_proof(log.clone(), block1_hash, receipt_proof.clone());
        let message_hash = Keccak256::hash_of(&message);
        assert_err!(
            Verifier::verify(network_id.into(), message_hash, &proof),
            Error::<Test>::HeaderNotFinalized,
        );
        // Verification works for an ancestor of the finalized header
        let (message, proof) =
            message_with_receipt_proof(log.clone(), block1_alt_hash, receipt_proof.clone());
        let message_hash = Keccak256::hash_of(&message);
        assert_ok!(Verifier::verify(network_id.into(), message_hash, &proof),);
    });
}

#[test]
fn test_register_network() {
    new_tester::<Test>().execute_with(|| {
        assert_ok!(Verifier::register_network(
            RuntimeOrigin::root(),
            EthNetworkConfig::Sepolia,
            genesis_ethereum_header(),
            U256::zero(),
        ));

        let caller = Keyring::Ferdie;
        let header = child_of_genesis_ethereum_header();
        assert_ok!(Verifier::import_header(
            RuntimeOrigin::none(),
            EthNetworkConfig::Sepolia.chain_id(),
            header.clone(),
            Default::default(),
            Default::default(),
            MultiSigner::from(caller.clone()).into_account(),
            digest_signature::<mock_verifier::Test>(
                &caller.pair(),
                &EthNetworkConfig::Sepolia.chain_id(),
                &header
            ),
        ));
    });
}

#[test]
fn test_register_network_exists() {
    new_tester::<Test>().execute_with(|| {
        assert_noop!(
            Verifier::register_network(
                RuntimeOrigin::root(),
                EthNetworkConfig::Ropsten,
                genesis_ethereum_header(),
                U256::zero(),
            ),
            Error::<Test>::NetworkAlreadyExists
        );
    });
}

#[test]
fn it_validates_last_headers_difficulty() {
    new_tester_with_config::<mock_verifier_with_pow::Test>(GenesisConfig {
        initial_networks: vec![(
            EthNetworkConfig::Mainnet,
            ethereum_header_from_file(11090290, ""),
            0u32.into(),
        )],
    })
    .execute_with(|| {
        let network_id = EthNetworkConfig::Ropsten.chain_id();
        let mut header1 = ethereum_header_from_file(11090291, "");

        let ferdie = Keyring::Ferdie;
        let diff_mult: U256 = (crate::DIFFICULTY_DIFFERENCE as u64).into();
        let check_header_num = header1.number - crate::CHECK_DIFFICULTY_DIFFERENCE_NUMBER;

        add_header_for_diffiulty_check(
            network_id,
            check_header_num,
            header1.clone(),
            header1.difficulty * diff_mult * 1001 / 1000,
        );

        assert_err!(
            mock_verifier_with_pow::Verifier::validate_header_difficulty_test(network_id, &header1),
            Error::<Test>::DifficultyTooLow
        );

        // increase difficulty a little bit to fit the difference
        header1.difficulty = header1.difficulty * 1002 / 1000;
        assert_ok!(
            mock_verifier_with_pow::Verifier::validate_header_difficulty_test(network_id, &header1)
        );
    });
}

#[test]
fn it_validates_last_headers_difficulty_multi() {
    new_tester_with_config::<mock_verifier_with_pow::Test>(GenesisConfig {
        initial_networks: vec![(
            EthNetworkConfig::Mainnet,
            ethereum_header_from_file(11090290, ""),
            0u32.into(),
        )],
    })
    .execute_with(|| {
        let network_id = EthNetworkConfig::Ropsten.chain_id();
        let mut header1 = ethereum_header_from_file(11090291, "");
        let header2 = ethereum_header_from_file(11090292, "");

        let ferdie = Keyring::Ferdie;
        let diff_mult: U256 = (crate::DIFFICULTY_DIFFERENCE as u64).into();
        let check_header_num = header1.number - crate::CHECK_DIFFICULTY_DIFFERENCE_NUMBER;

        add_header_for_diffiulty_check(
            network_id,
            check_header_num,
            header1.clone(),
            header1.difficulty * diff_mult,
        );

        add_header_for_diffiulty_check(
            network_id,
            check_header_num,
            header2.clone(),
            header1.difficulty * diff_mult * 1001 / 1000,
        );

        assert_err!(
            mock_verifier_with_pow::Verifier::validate_header_difficulty_test(network_id, &header1),
            Error::<Test>::DifficultyTooLow
        );

        // increase difficulty a little bit to fit the difference
        header1.difficulty = header1.difficulty * 1002 / 1000;
        assert_ok!(
            mock_verifier_with_pow::Verifier::validate_header_difficulty_test(network_id, &header1)
        );
    });
}

#[test]
fn test_base_fee_oracle() {
    new_tester::<Test>().execute_with(|| {
        let mut header = child_of_genesis_ethereum_header();
        let base_fee = Some(U256::from(42));
        header.base_fee = base_fee;
        let header_hash = header.compute_hash();
        let ferdie = Keyring::Ferdie;

        let network_id = EthNetworkConfig::Ropsten.chain_id();
        assert_ok!(Verifier::import_header(
            RuntimeOrigin::none(),
            network_id,
            header.clone(),
            Default::default(),
            Default::default(),
            MultiSigner::from(ferdie.clone()).into_account(),
            digest_signature::<mock_verifier::Test>(&ferdie.pair(), &network_id, &header),
        ));

        assert_eq!(
            Verifier::get_base_fee(network_id, header_hash).unwrap(),
            base_fee
        );
        assert_eq!(
            Verifier::get_best_block_base_fee(network_id).unwrap(),
            base_fee
        );
    });
}

#[test]
fn test_base_fee_oracle_no_base_fee() {
    new_tester::<Test>().execute_with(|| {
        let mut header = child_of_genesis_ethereum_header();
        let base_fee = None;
        header.base_fee = base_fee;
        let header_hash = header.compute_hash();
        let ferdie = Keyring::Ferdie;

        let network_id = EthNetworkConfig::Ropsten.chain_id();
        assert_ok!(Verifier::import_header(
            RuntimeOrigin::none(),
            network_id,
            header.clone(),
            Default::default(),
            Default::default(),
            MultiSigner::from(ferdie.clone()).into_account(),
            digest_signature::<mock_verifier::Test>(&ferdie.pair(), &network_id, &header),
        ));

        assert_eq!(
            Verifier::get_base_fee(network_id, header_hash).unwrap(),
            base_fee
        );
        assert_eq!(
            Verifier::get_best_block_base_fee(network_id).unwrap(),
            base_fee
        );
    });
}

#[test]
fn test_base_fee_oracle_no_header() {
    new_tester::<Test>().execute_with(|| {
        let header = child_of_genesis_ethereum_header();
        let header_hash = header.compute_hash();
        let network_id = EthNetworkConfig::Ropsten.chain_id();

        assert_err!(
            Verifier::get_base_fee(network_id, header_hash),
            Error::<Test>::HeaderNotFound
        );
    });
}

fn add_header_for_diffiulty_check(
    network_id: EVMChainId,
    header_number: u64,
    mut header: EthereumHeader,
    difficulty: U256,
) {
    header.difficulty = difficulty;
    let hash = header.compute_hash();
    let header_to_store: crate::StoredHeader<crate::mock::AccountId> = crate::StoredHeader {
        submitter: None,
        header: header.clone(),
        total_difficulty: difficulty,
        finalized: false,
    };
    Headers::<Test>::insert(network_id, hash, header_to_store);
    HeadersByNumber::<Test>::try_mutate(
        network_id,
        header_number,
        |x| -> sp_runtime::DispatchResult {
            match x {
                None => *x = Some(vec![hash]),
                Some(v) => v.push(hash),
            }
            Ok(().into())
        },
    )
    .expect("add_header_for_diffiulty_check: add headers error");
}
