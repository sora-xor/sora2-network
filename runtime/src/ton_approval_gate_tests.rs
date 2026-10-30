//! Local TON authorization is enforced before accepting disclosed peer signatures.
use crate::{
    BridgeDataSigner, MultiProof, MultiVerifier, MultisigVerifier, Runtime, RuntimeOrigin,
    ThisNetworkId,
};
use bridge_types::{
    ton::TonNetworkId,
    traits::Verifier,
    types::{AuxiliaryDigest, AuxiliaryDigestItem},
    GenericNetworkId, SubNetworkId,
};
use frame_support::{assert_noop, assert_ok, traits::Get};
use sp_core::{ecdsa, Pair, H256};
use sp_runtime::traits::{Hash, Keccak256};

#[test]
fn ton_requires_exact_pending_domain_and_current_valid_proof() {
    sp_io::TestExternalities::default().execute_with(|| {
        let network = GenericNetworkId::TON(TonNetworkId::Mainnet);
        let other = GenericNetworkId::TON(TonNetworkId::Testnet);
        let signer = ecdsa::Pair::from_seed(&[21; 32]); // Public test fixture only.
        let peers = vec![signer.public()];
        assert_ok!(MultisigVerifier::force_set_peers(
            RuntimeOrigin::root(),
            network,
            peers.clone().try_into().unwrap()
        ));
        assert_ok!(BridgeDataSigner::force_set_peers(
            RuntimeOrigin::root(),
            network,
            peers.clone().try_into().unwrap()
        ));
        assert_ok!(BridgeDataSigner::force_set_peers(
            RuntimeOrigin::root(),
            other,
            peers.try_into().unwrap()
        ));
        let commitment = H256::repeat_byte(11);
        let hash = Keccak256::hash_of(&(network, ThisNetworkId::get(), commitment));
        let proof = MultiProof::EVMMultisig(multisig_verifier::MultiEVMProof {
            proof: vec![signer.sign_prehashed(hash.as_fixed_bytes())],
        });
        assert_noop!(
            MultiVerifier::verify(network, commitment, &proof),
            bridge_data_signer::Error::<Runtime>::ApprovalNotPending
        );
        assert_ok!(BridgeDataSigner::register_pending_approval(
            RuntimeOrigin::root(),
            other,
            hash
        ));
        assert_noop!(
            MultiVerifier::verify(network, commitment, &proof),
            bridge_data_signer::Error::<Runtime>::ApprovalNotPending
        );
        assert_ok!(BridgeDataSigner::register_pending_approval(
            RuntimeOrigin::root(),
            network,
            commitment
        ));
        assert_noop!(
            MultiVerifier::verify(network, commitment, &proof),
            bridge_data_signer::Error::<Runtime>::ApprovalNotPending
        );
        assert_ok!(BridgeDataSigner::register_pending_approval(
            RuntimeOrigin::root(),
            network,
            hash
        ));
        assert_ok!(MultiVerifier::verify(network, commitment, &proof));
        let invalid = MultiProof::EVMMultisig(multisig_verifier::MultiEVMProof { proof: vec![] });
        assert!(MultiVerifier::verify(network, commitment, &invalid).is_err());
        let digest = AuxiliaryDigest {
            logs: vec![AuxiliaryDigestItem::Commitment(
                ThisNetworkId::get(),
                commitment,
            )],
        };
        let digest_proof = MultiProof::Multisig(multisig_verifier::Proof {
            proof: vec![signer.sign_prehashed(Keccak256::hash_of(&digest).as_fixed_bytes())],
            digest,
        });
        assert_noop!(
            MultiVerifier::verify(network, commitment, &digest_proof),
            multisig_verifier::Error::<Runtime>::InvalidNetworkId
        );
        assert_ok!(BridgeDataSigner::clear_pending_approval(
            RuntimeOrigin::root(),
            network,
            hash
        ));
        assert_noop!(
            MultiVerifier::verify(network, commitment, &proof),
            bridge_data_signer::Error::<Runtime>::ApprovalNotPending
        );
    });
}

#[test]
fn remote_substrate_digest_does_not_need_a_local_ton_authorization() {
    sp_io::TestExternalities::default().execute_with(|| {
        let network = GenericNetworkId::Sub(SubNetworkId::Polkadot);
        let signer = ecdsa::Pair::from_seed(&[22; 32]); // Public test fixture only.
        assert_ok!(MultisigVerifier::force_set_peers(
            RuntimeOrigin::root(),
            network,
            vec![signer.public()].try_into().unwrap()
        ));
        let commitment = H256::repeat_byte(12);
        let digest = AuxiliaryDigest {
            logs: vec![AuxiliaryDigestItem::Commitment(
                ThisNetworkId::get(),
                commitment,
            )],
        };
        let proof = MultiProof::Multisig(multisig_verifier::Proof {
            proof: vec![signer.sign_prehashed(Keccak256::hash_of(&digest).as_fixed_bytes())],
            digest,
        });
        assert_ok!(MultiVerifier::verify(network, commitment, &proof));
    });
}
