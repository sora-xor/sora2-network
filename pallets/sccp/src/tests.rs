use crate::mock::*;
use crate::{
    Error, TokenStatus, VerifiedBurnProof, VerifiedGovernanceProof, VerifiedGovernanceProofAction,
    VerifiedTokenAddProof, VerifiedTokenControlProof, SCCP_DIGEST_NETWORK_ID, SCCP_DOMAIN_BSC,
    SCCP_DOMAIN_ETH, SCCP_DOMAIN_SORA,
};
use bridge_types::types::AuxiliaryDigestItem;
use common::{AssetName, AssetSymbol, DEFAULT_BALANCE_PRECISION};
use frame_support::{assert_noop, assert_ok};
use iroha_sccp::{burn_message_id, BurnPayloadV1, TokenAddPayloadV1, TokenControlPayloadV1};
use sp_core::H256;

fn dummy_proof_bytes(tag: u8) -> Vec<u8> {
    vec![tag]
}

fn register_mintable_asset(asset_id: AssetId, symbol: &[u8], name: &[u8]) {
    assert_ok!(assets::Pallet::<Runtime>::register_asset_id(
        alice(),
        asset_id,
        AssetSymbol(symbol.to_vec()),
        AssetName(name.to_vec()),
        DEFAULT_BALANCE_PRECISION,
        0u32.into(),
        true,
        common::AssetType::Regular,
        None,
        None,
    ));
}

fn register_bridgeable_asset(asset_id: AssetId, symbol: &[u8], name: &[u8]) {
    register_mintable_asset(asset_id, symbol, name);
    set_nexus_governance_verify_result(Some(verified_add_token_proof(asset_id, symbol, name)));
    assert_ok!(Sccp::add_token_from_proof(
        RuntimeOrigin::signed(alice()),
        dummy_proof_bytes(0xA1),
    ));
}

fn verified_add_token_proof(
    asset_id: AssetId,
    symbol: &[u8],
    name: &[u8],
) -> VerifiedGovernanceProof {
    let payload = TokenAddPayloadV1 {
        version: 1,
        target_domain: SCCP_DOMAIN_SORA,
        nonce: 1,
        sora_asset_id: H256::from(asset_id).0,
        decimals: DEFAULT_BALANCE_PRECISION,
        name: fixed_32(name),
        symbol: fixed_32(symbol),
    };
    VerifiedGovernanceProof {
        message_id: Sccp::token_add_message_id(&payload),
        action: VerifiedGovernanceProofAction::Add(VerifiedTokenAddProof {
            target_domain: payload.target_domain,
            nonce: payload.nonce,
            sora_asset_id: H256(payload.sora_asset_id),
            decimals: payload.decimals,
            name: payload.name,
            symbol: payload.symbol,
        }),
    }
}

fn verified_pause_token_proof(asset_id: AssetId, nonce: u64) -> VerifiedGovernanceProof {
    let payload = TokenControlPayloadV1 {
        version: 1,
        target_domain: SCCP_DOMAIN_SORA,
        nonce,
        sora_asset_id: H256::from(asset_id).0,
    };
    VerifiedGovernanceProof {
        message_id: Sccp::token_pause_message_id(&payload),
        action: VerifiedGovernanceProofAction::Pause(VerifiedTokenControlProof {
            target_domain: payload.target_domain,
            nonce: payload.nonce,
            sora_asset_id: H256(payload.sora_asset_id),
        }),
    }
}

fn verified_resume_token_proof(asset_id: AssetId, nonce: u64) -> VerifiedGovernanceProof {
    let payload = TokenControlPayloadV1 {
        version: 1,
        target_domain: SCCP_DOMAIN_SORA,
        nonce,
        sora_asset_id: H256::from(asset_id).0,
    };
    VerifiedGovernanceProof {
        message_id: Sccp::token_resume_message_id(&payload),
        action: VerifiedGovernanceProofAction::Resume(VerifiedTokenControlProof {
            target_domain: payload.target_domain,
            nonce: payload.nonce,
            sora_asset_id: H256(payload.sora_asset_id),
        }),
    }
}

fn verified_burn_proof(
    asset_id: AssetId,
    source_domain: u32,
    nonce: u64,
    amount: u128,
    recipient: [u8; 32],
) -> VerifiedBurnProof {
    let payload = BurnPayloadV1 {
        version: 1,
        source_domain,
        dest_domain: SCCP_DOMAIN_SORA,
        nonce,
        sora_asset_id: H256::from(asset_id).0,
        amount,
        recipient,
    };
    VerifiedBurnProof {
        message_id: H256(burn_message_id(&payload)),
        source_domain,
        dest_domain: SCCP_DOMAIN_SORA,
        nonce,
        sora_asset_id: H256(payload.sora_asset_id),
        amount,
        recipient,
    }
}

fn fixed_32(input: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[..input.len()].copy_from_slice(input);
    out
}

fn canonical_evm_recipient() -> [u8; 32] {
    let mut recipient = [0u8; 32];
    recipient[12..].copy_from_slice(&[0x44u8; 20]);
    recipient
}

#[test]
fn add_token_from_proof_registers_active_token() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::Tomato.into();

    ext.execute_with(|| {
        register_mintable_asset(asset_id, b"TST", b"Test");
        let verified = verified_add_token_proof(asset_id, b"TST", b"Test");
        let message_id = verified.message_id;
        set_nexus_governance_verify_result(Some(verified));

        assert_ok!(Sccp::add_token_from_proof(
            RuntimeOrigin::signed(alice()),
            dummy_proof_bytes(0x01),
        ));

        let state = Sccp::token_state(asset_id).expect("token state");
        assert_eq!(state.status, TokenStatus::Active);
        assert!(Sccp::applied_governance(message_id));
    });
}

#[test]
fn burn_commits_local_message_id_for_hub_consumption() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::Pan.into();

    ext.execute_with(|| {
        register_bridgeable_asset(asset_id, b"PAN", b"Pan");

        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            alice(),
            asset_id,
            1_000i128,
        ));

        let recipient = canonical_evm_recipient();
        assert_ok!(Sccp::burn(
            RuntimeOrigin::signed(alice()),
            asset_id,
            25,
            SCCP_DOMAIN_ETH,
            recipient,
        ));

        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_ETH,
            nonce: 1,
            sora_asset_id: H256::from(asset_id).0,
            amount: 25,
            recipient,
        };
        let message_id = H256(burn_message_id(&payload));
        assert!(Sccp::burn_record(message_id).is_some());
        assert_eq!(
            take_aux_digest_items(),
            vec![AuxiliaryDigestItem::Commitment(
                SCCP_DIGEST_NETWORK_ID,
                bridge_types::H256::from_slice(message_id.as_bytes()),
            )]
        );
    });
}

#[test]
fn mint_from_proof_mints_once() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::Potato.into();

    ext.execute_with(|| {
        register_bridgeable_asset(asset_id, b"POT", b"Potato");
        let recipient = [9u8; 32];
        let verified = verified_burn_proof(asset_id, SCCP_DOMAIN_BSC, 7, 50, recipient);
        let message_id = verified.message_id;
        set_nexus_burn_verify_result(Some(verified));

        assert_ok!(Sccp::mint_from_proof(
            RuntimeOrigin::signed(alice()),
            dummy_proof_bytes(0x02),
        ));
        assert!(Sccp::processed_inbound(message_id));

        assert_noop!(
            Sccp::mint_from_proof(RuntimeOrigin::signed(alice()), dummy_proof_bytes(0x02)),
            Error::<Runtime>::InboundAlreadyProcessed
        );
    });
}

#[test]
fn pause_and_resume_proofs_drive_token_state() {
    let mut ext = ExtBuilder::default().build();
    let asset_id: AssetId = common::mock::ComicAssetId::BluePromise.into();

    ext.execute_with(|| {
        register_bridgeable_asset(asset_id, b"BLU", b"Blue");

        let pause = verified_pause_token_proof(asset_id, 2);
        let pause_message_id = pause.message_id;
        set_nexus_governance_verify_result(Some(pause));
        assert_ok!(Sccp::pause_token_from_proof(
            RuntimeOrigin::signed(alice()),
            dummy_proof_bytes(0x03),
        ));
        assert_eq!(
            Sccp::token_state(asset_id).expect("token state").status,
            TokenStatus::Paused
        );
        assert!(Sccp::applied_governance(pause_message_id));

        assert_noop!(
            Sccp::burn(
                RuntimeOrigin::signed(alice()),
                asset_id,
                1,
                SCCP_DOMAIN_ETH,
                canonical_evm_recipient(),
            ),
            Error::<Runtime>::TokenNotActive
        );

        let resume = verified_resume_token_proof(asset_id, 3);
        let resume_message_id = resume.message_id;
        set_nexus_governance_verify_result(Some(resume));
        assert_ok!(Sccp::resume_token_from_proof(
            RuntimeOrigin::signed(alice()),
            dummy_proof_bytes(0x04),
        ));
        assert_eq!(
            Sccp::token_state(asset_id).expect("token state").status,
            TokenStatus::Active
        );
        assert!(Sccp::applied_governance(resume_message_id));
    });
}

#[test]
fn governance_and_burn_fail_closed_when_verifier_unavailable() {
    let mut ext = ExtBuilder::default().build();
    let governance_asset_id: AssetId = common::mock::ComicAssetId::Mango.into();
    let burn_asset_id: AssetId = common::mock::ComicAssetId::Future.into();

    ext.execute_with(|| {
        register_mintable_asset(governance_asset_id, b"CRT", b"Carrot");

        assert_noop!(
            Sccp::add_token_from_proof(RuntimeOrigin::signed(alice()), dummy_proof_bytes(0x05)),
            Error::<Runtime>::ProofVerificationUnavailable
        );

        register_bridgeable_asset(burn_asset_id, b"BRT", b"Beet");
        set_nexus_burn_verify_result(None);
        assert_noop!(
            Sccp::mint_from_proof(RuntimeOrigin::signed(alice()), dummy_proof_bytes(0x06)),
            Error::<Runtime>::ProofVerificationUnavailable
        );
    });
}
