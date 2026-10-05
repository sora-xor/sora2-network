// This file is part of the SORA network and Polkaswap app.
// SPDX-License-Identifier: BSD-4-Clause

use super::*;
use crate::{Executive, IrohaMigration, RuntimeCall, SignedExtra, UncheckedExtrinsic, XorFee};
use ed25519_dalek_iroha::{Digest, Keypair, PublicKey, SecretKey};
use frame_support::{assert_err, dispatch::GetDispatchInfo};
use sha3::Sha3_256;
use sp_core::sr25519;
use sp_runtime::{generic::Era, FixedPointNumber, FixedU128};

const ADDRESS: &str = "sponsored-migration@sora";

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn raw_map(name: &[u8], key: impl Encode, value: impl Encode) {
    let encoded = key.encode();
    let mut storage_key = frame_support::storage::storage_prefix(b"IrohaMigration", name).to_vec();
    storage_key.extend(sp_core::blake2_128(&encoded));
    storage_key.extend(encoded);
    sp_io::storage::set(&storage_key, &value.encode());
}

fn fixture(fail: bool) -> (sr25519::Pair, AccountId, RuntimeCall, String) {
    System::set_block_number(1);
    assert_ok!(XorFee::update_multiplier(
        RuntimeOrigin::root(),
        FixedU128::saturating_from_integer(1)
    ));
    let signer = sr25519::Pair::from_seed(&[79; 32]);
    let who = AccountId::from(signer.public());
    let sponsor = AccountId::from([78; 32]);
    let secret = SecretKey::from_bytes(&[71; 32]).unwrap();
    let key = Keypair {
        public: PublicKey::from(&secret),
        secret,
    };
    let public = hex(key.public.as_bytes());
    raw_map(
        b"PublicKeys",
        ADDRESS.to_string(),
        vec![(false, public.clone())],
    );
    let message = format!(
        "SORA2-IROHA-MIGRATION-V2\ngenesis_hash=0x{}\niroha_address={}\niroha_public_key={}\nsubstrate_account=0x{}",
        hex(&crate::IrohaMigrationGenesisHash::get().encode()), ADDRESS, public, hex(&who.encode()),
    );
    let mut digest = Sha3_256::default();
    digest.update(message.as_bytes());
    let signature = hex(&key.sign_prehashed(digest, None).unwrap().to_bytes());
    let call = RuntimeCall::IrohaMigration(iroha_migration::Call::migrate {
        iroha_address: ADDRESS.to_string(),
        iroha_public_key: public.clone(),
        iroha_signature: signature,
    });
    if fail {
        // Eligibility and its signature are valid; settlement fails only after
        // payment because the recipient already has a different referral.
        let referrer = AccountId::from([80; 32]);
        raw_map(
            b"Referrers",
            ADDRESS.to_string(),
            "existing-referrer@sora".to_string(),
        );
        raw_map(
            b"MigratedAccounts",
            "existing-referrer@sora".to_string(),
            referrer.clone(),
        );
        referrals::Referrers::<Runtime>::insert(&who, referrer);
    }
    (signer, sponsor, call, public)
}

fn grant(sponsor: &AccountId, who: &AccountId, public: &str, attempts: u8) {
    assert_ok!(IrohaMigration::sponsor_migration(
        RuntimeOrigin::signed(sponsor.clone()),
        who.clone(),
        ADDRESS.into(),
        public.into(),
        10_000 * UNIT,
        attempts,
        100,
    ));
}

fn wire(call: RuntimeCall, signer: &sr25519::Pair) -> UncheckedExtrinsic {
    let who = AccountId::from(signer.public());
    let extra: SignedExtra = (
        frame_system::CheckSpecVersion::new(),
        frame_system::CheckTxVersion::new(),
        frame_system::CheckGenesis::new(),
        frame_system::CheckEra::from(Era::Immortal),
        frame_system::CheckNonce::from(System::account_nonce(&who)),
        frame_system::CheckWeight::new(),
        crate::charge_tx_payment_extension(),
    );
    let payload = crate::SignedPayload::new(call.clone(), extra.clone()).unwrap();
    let signature = payload.using_encoded(|bytes| signer.sign(bytes));
    UncheckedExtrinsic::new_signed(call, who, crate::Signature::Sr25519(signature), extra)
}

fn reset() {
    frame_system::BlockSize::<Runtime>::kill();
    frame_system::BlockWeight::<Runtime>::kill();
    System::reset_events();
}

#[test]
fn zero_xor_migration_uses_sponsor_and_refunds_only_after_success() {
    ext().execute_with(|| {
        let (signer, sponsor, call, public) = fixture(false);
        let who = AccountId::from(signer.public());
        drop(Balances::deposit_creating(&sponsor, 100_000 * UNIT));
        grant(&sponsor, &who, &public, 1);
        assert_eq!(Balances::free_balance(&who), 0);
        let before = Balances::free_balance(&sponsor);
        reset();
        assert_ok!(Executive::apply_extrinsic(wire(call.clone(), &signer)).unwrap());
        assert_eq!(Balances::free_balance(&sponsor), before);
        assert_eq!(Balances::free_balance(&who), 0);
        let id = IrohaMigration::sponsorship_id(&who, &ADDRESS.into(), &public);
        assert_eq!(
            iroha_migration::FeeSponsorships::<Runtime>::get(id)
                .unwrap()
                .remaining_attempts,
            0
        );
        assert_eq!(System::account_nonce(&who), 1);
        reset();
        assert_err!(
            Executive::apply_extrinsic(wire(call, &signer)),
            InvalidTransaction::Payment
        );
    });
}

#[test]
fn failed_settlement_charges_sponsor_and_consumes_retry_budget_outside_rollback() {
    ext().execute_with(|| {
        let (signer, sponsor, call, public) = fixture(true);
        let who = AccountId::from(signer.public());
        drop(Balances::deposit_creating(&sponsor, 100_000 * UNIT));
        grant(&sponsor, &who, &public, 2);
        let id = IrohaMigration::sponsorship_id(&who, &ADDRESS.into(), &public);
        for remaining in [1, 0] {
            let before = Balances::free_balance(&sponsor);
            reset();
            let error = Executive::apply_extrinsic(wire(call.clone(), &signer))
                .unwrap()
                .unwrap_err();
            assert_eq!(
                error,
                iroha_migration::Error::<Runtime>::ReferralMigrationFailed.into()
            );
            let charged = before - Balances::free_balance(&sponsor);
            assert!(charged > 0);
            assert_eq!(Balances::free_balance(&who), 0);
            assert_eq!(
                iroha_migration::FeeSponsorships::<Runtime>::get(id)
                    .unwrap()
                    .remaining_attempts,
                remaining
            );
            let emitted_fee = System::events()
                .iter()
                .find_map(|record| match &record.event {
                    RuntimeEvent::TransactionPayment(
                        pallet_transaction_payment::Event::TransactionFeePaid {
                            actual_fee,
                            tip,
                            ..
                        },
                    ) => {
                        assert_eq!(*tip, 0);
                        Some(*actual_fee)
                    }
                    _ => None,
                })
                .unwrap();
            assert_eq!(emitted_fee, charged);
        }
        assert_eq!(System::account_nonce(&who), 2);
        reset();
        assert_err!(
            Executive::apply_extrinsic(wire(call, &signer)),
            InvalidTransaction::Payment
        );
    });
}

#[test]
fn unfunded_sponsor_does_not_consume_authorization_or_admit_migration() {
    ext().execute_with(|| {
        let (signer, sponsor, call, public) = fixture(false);
        let who = AccountId::from(signer.public());
        drop(Balances::deposit_creating(&sponsor, 100_000 * UNIT));
        grant(&sponsor, &who, &public, 1);
        drop(Balances::make_free_balance_be(&sponsor, 0));
        let id = IrohaMigration::sponsorship_id(&who, &ADDRESS.into(), &public);
        reset();
        assert_err!(
            Executive::apply_extrinsic(wire(call.clone(), &signer)),
            InvalidTransaction::Payment
        );
        assert_eq!(
            iroha_migration::FeeSponsorships::<Runtime>::get(id)
                .unwrap()
                .remaining_attempts,
            1
        );
        // Exercise the withdrawal path independently of validation as well.
        use pallet_transaction_payment::OnChargeTransaction;
        assert!(<XorFee as OnChargeTransaction<Runtime>>::withdraw_fee(
            &who,
            &call,
            &call.get_dispatch_info(),
            UNIT,
            0,
        )
        .is_err());
        assert_eq!(
            iroha_migration::FeeSponsorships::<Runtime>::get(id)
                .unwrap()
                .remaining_attempts,
            1
        );
    });
}

#[test]
fn sponsorship_is_claimant_bound_excludes_tips_and_wrappers_and_can_be_revoked() {
    ext().execute_with(|| {
        let (signer, sponsor, call, public) = fixture(false);
        let who = AccountId::from(signer.public());
        drop(Balances::deposit_creating(&sponsor, 100_000 * UNIT));
        grant(&sponsor, &who, &public, 1);
        assert_eq!(
            crate::migration_fees::sponsor(&who, &call, UNIT, 0),
            Some(sponsor.clone())
        );
        assert_eq!(crate::migration_fees::sponsor(&who, &call, UNIT, 1), None);
        assert_eq!(
            crate::migration_fees::sponsor(&AccountId::from([81; 32]), &call, UNIT, 0),
            None
        );
        assert_eq!(
            crate::migration_fees::sponsor(&who, &call, 10_001 * UNIT, 0),
            None
        );
        let wrapped = RuntimeCall::Utility(pallet_utility::Call::batch_all {
            calls: vec![call.clone()],
        });
        assert_eq!(
            crate::migration_fees::sponsor(&who, &wrapped, UNIT, 0),
            None
        );
        let id = IrohaMigration::sponsorship_id(&who, &ADDRESS.into(), &public);
        assert!(IrohaMigration::revoke_sponsorship(
            RuntimeOrigin::signed(AccountId::from([81; 32])),
            id
        )
        .is_err());
        assert_ok!(IrohaMigration::revoke_sponsorship(
            RuntimeOrigin::signed(sponsor),
            id
        ));
        assert_eq!(crate::migration_fees::sponsor(&who, &call, UNIT, 0), None);
        assert!(!iroha_migration::FeeSponsorships::<Runtime>::contains_key(
            id
        ));
    });
}

#[test]
fn unfunded_volunteer_cannot_block_self_payment_or_another_sponsor() {
    ext().execute_with(|| {
        let (signer, unfunded, call, public) = fixture(true);
        let who = AccountId::from(signer.public());
        drop(Balances::deposit_creating(&unfunded, 100_000 * UNIT));
        grant(&unfunded, &who, &public, 1);
        drop(Balances::make_free_balance_be(&unfunded, 0));
        drop(Balances::deposit_creating(&who, 100_000 * UNIT));
        assert_eq!(crate::migration_fees::sponsor(&who, &call, UNIT, 0), None);
        let before = Balances::free_balance(&who);
        reset();
        assert!(Executive::apply_extrinsic(wire(call.clone(), &signer))
            .unwrap()
            .is_err());
        assert!(Balances::free_balance(&who) < before);
        let id = IrohaMigration::sponsorship_id(&who, &ADDRESS.into(), &public);
        assert_eq!(
            iroha_migration::FeeSponsorships::<Runtime>::get(id)
                .unwrap()
                .remaining_attempts,
            1
        );

        let replacement = AccountId::from([83; 32]);
        drop(Balances::deposit_creating(&replacement, 100_000 * UNIT));
        let refs = System::account(&who).sufficients;
        grant(&replacement, &who, &public, 2);
        assert_eq!(System::account(&who).sufficients, refs);
        assert_eq!(
            crate::migration_fees::sponsor(&who, &call, UNIT, 0),
            Some(replacement)
        );
        drop(Balances::deposit_creating(&unfunded, 100_000 * UNIT));
        assert!(
            IrohaMigration::sponsor_migration(
                RuntimeOrigin::signed(unfunded),
                who.clone(),
                ADDRESS.into(),
                public.into(),
                10_000 * UNIT,
                1,
                100,
            )
            .is_err(),
            "an active funded grant cannot be overwritten by a third party"
        );
        assert_ok!(IrohaMigration::revoke_sponsorship(
            RuntimeOrigin::signed(who.clone()),
            id
        ));
        assert_eq!(System::account(&who).sufficients, refs - 1);
    });
}

#[test]
fn funded_underpriced_grant_can_be_replaced_without_claimant_xor() {
    ext().execute_with(|| {
        let (signer, sponsor, call, public) = fixture(false);
        let who = AccountId::from(signer.public());
        let attacker = AccountId::from([84; 32]);
        drop(Balances::deposit_creating(&attacker, 100_000 * UNIT));
        drop(Balances::deposit_creating(&sponsor, 100_000 * UNIT));
        assert_ok!(IrohaMigration::sponsor_migration(
            RuntimeOrigin::signed(attacker.clone()),
            who.clone(),
            ADDRESS.into(),
            public.clone(),
            1,
            3,
            u32::MAX,
        ));
        let id = IrohaMigration::sponsorship_id(&who, &ADDRESS.into(), &public);
        let references = System::account(&who).sufficients;
        let fee = XorFee::compute_fee(
            wire(call.clone(), &signer).encoded_size() as u32,
            &call,
            &call.get_dispatch_info(),
            0,
        )
        .0;
        assert!(fee > 1);
        assert_eq!(crate::migration_fees::sponsor(&who, &call, fee, 0), None);
        assert_eq!(Balances::free_balance(&who), 0);

        // The attacker retains its funding and its maximum-length expiry;
        // replacement needs neither its cooperation nor claimant funds.
        assert_ok!(IrohaMigration::sponsor_migration(
            RuntimeOrigin::signed(sponsor.clone()),
            who.clone(),
            ADDRESS.into(),
            public,
            10_000 * UNIT,
            3,
            u32::MAX,
        ));
        assert_eq!(System::account(&who).sufficients, references);
        assert_eq!(
            crate::migration_fees::sponsor(&who, &call, fee, 0),
            Some(sponsor.clone())
        );
        let sponsor_before = Balances::free_balance(&sponsor);
        let attacker_before = Balances::free_balance(&attacker);
        reset();
        assert_ok!(Executive::apply_extrinsic(wire(call, &signer)).unwrap());
        assert_eq!(Balances::free_balance(&who), 0);
        assert_eq!(Balances::free_balance(&sponsor), sponsor_before);
        assert_eq!(Balances::free_balance(&attacker), attacker_before);
        let remaining = iroha_migration::FeeSponsorships::<Runtime>::get(id).unwrap();
        assert_eq!(remaining.sponsor, sponsor);
        assert_eq!(remaining.remaining_attempts, 2);
        assert_eq!(System::account_nonce(&who), 1);
        assert!(System::events().iter().any(|record| matches!(
            &record.event,
            RuntimeEvent::IrohaMigration(iroha_migration::Event::Migrated(address, account))
                if address == ADDRESS && account == &who
        )));
    });
}

#[test]
fn replacement_cannot_reduce_a_funded_grants_limits() {
    ext().execute_with(|| {
        let (signer, sponsor, call, public) = fixture(true);
        let who = AccountId::from(signer.public());
        let replacement = AccountId::from([85; 32]);
        drop(Balances::deposit_creating(&sponsor, 100_000 * UNIT));
        drop(Balances::deposit_creating(&replacement, 100_000 * UNIT));
        grant(&sponsor, &who, &public, 3);
        reset();
        assert!(Executive::apply_extrinsic(wire(call.clone(), &signer))
            .unwrap()
            .is_err());
        let id = IrohaMigration::sponsorship_id(&who, &ADDRESS.into(), &public);
        let existing = iroha_migration::FeeSponsorships::<Runtime>::get(id).unwrap();
        let references = System::account(&who).sufficients;
        assert_eq!(existing.remaining_attempts, 2);
        assert!(existing.remaining_budget > 20_002 * UNIT);

        for (max_fee, attempts, valid_until) in [
            (10_000 * UNIT, 3, 101), // Equal fee cap cannot displace a live grant.
            (9_999 * UNIT, 3, 101),  // Lower fee cap despite increased attempts.
            (10_001 * UNIT, 2, 101), // Higher cap, but less remaining budget.
            (30_001 * UNIT, 1, 101), // More budget, but fewer remaining attempts.
            (10_001 * UNIT, 3, 99),  // Better funding, but an earlier expiry.
        ] {
            assert_err!(
                IrohaMigration::sponsor_migration(
                    RuntimeOrigin::signed(replacement.clone()),
                    who.clone(),
                    ADDRESS.into(),
                    public.clone(),
                    max_fee,
                    attempts,
                    valid_until,
                ),
                iroha_migration::Error::<Runtime>::NotFeeSponsor
            );
            assert_eq!(
                iroha_migration::FeeSponsorships::<Runtime>::get(id)
                    .unwrap()
                    .encode(),
                existing.encode()
            );
            assert_eq!(System::account(&who).sufficients, references);
        }

        assert_ok!(IrohaMigration::sponsor_migration(
            RuntimeOrigin::signed(replacement.clone()),
            who.clone(),
            ADDRESS.into(),
            public.clone(),
            10_001 * UNIT,
            3,
            100,
        ));
        // A stale competing replacement cannot undo the improved fee cap.
        assert_err!(
            IrohaMigration::sponsor_migration(
                RuntimeOrigin::signed(sponsor.clone()),
                who.clone(),
                ADDRESS.into(),
                public,
                10_000 * UNIT,
                3,
                100,
            ),
            iroha_migration::Error::<Runtime>::NotFeeSponsor
        );
        assert_eq!(System::account(&who).sufficients, references);
        let sponsor_before = Balances::free_balance(&sponsor);
        let replacement_before = Balances::free_balance(&replacement);
        reset();
        assert!(Executive::apply_extrinsic(wire(call, &signer))
            .unwrap()
            .is_err());
        assert_eq!(Balances::free_balance(&sponsor), sponsor_before);
        assert!(Balances::free_balance(&replacement) < replacement_before);
        let remaining = iroha_migration::FeeSponsorships::<Runtime>::get(id).unwrap();
        assert_eq!(remaining.sponsor, replacement);
        assert_eq!(remaining.remaining_attempts, 2);
        assert_eq!(Balances::free_balance(&who), 0);
    });
}

#[test]
fn invalid_proof_cannot_spend_sponsor_and_expired_grant_cannot_admit_zero_xor_claimant() {
    ext().execute_with(|| {
        let (signer, sponsor, mut call, public) = fixture(false);
        let who = AccountId::from(signer.public());
        drop(Balances::deposit_creating(&sponsor, 100_000 * UNIT));
        drop(Balances::deposit_creating(&who, 100_000 * UNIT));
        grant(&sponsor, &who, &public, 1);
        if let RuntimeCall::IrohaMigration(iroha_migration::Call::migrate {
            iroha_signature, ..
        }) = &mut call
        {
            *iroha_signature = "00".repeat(64);
        }
        let sponsor_before = Balances::free_balance(&sponsor);
        let payer_before = Balances::free_balance(&who);
        reset();
        assert!(Executive::apply_extrinsic(wire(call, &signer))
            .unwrap()
            .is_err());
        assert_eq!(Balances::free_balance(&sponsor), sponsor_before);
        assert!(Balances::free_balance(&who) < payer_before);
        let id = IrohaMigration::sponsorship_id(&who, &ADDRESS.into(), &public);
        assert_eq!(
            iroha_migration::FeeSponsorships::<Runtime>::get(id)
                .unwrap()
                .remaining_attempts,
            1
        );
    });
    ext().execute_with(|| {
        let (signer, sponsor, call, public) = fixture(false);
        let who = AccountId::from(signer.public());
        drop(Balances::deposit_creating(&sponsor, 100_000 * UNIT));
        grant(&sponsor, &who, &public, 1);
        System::set_block_number(101);
        assert_eq!(crate::migration_fees::sponsor(&who, &call, UNIT, 0), None);
        reset();
        assert_err!(
            Executive::apply_extrinsic(wire(call, &signer)),
            InvalidTransaction::Payment
        );
        let id = IrohaMigration::sponsorship_id(&who, &ADDRESS.into(), &public);
        assert_ok!(IrohaMigration::revoke_sponsorship(
            RuntimeOrigin::signed(AccountId::from([82; 32])),
            id
        ));
        assert!(!iroha_migration::FeeSponsorships::<Runtime>::contains_key(
            id
        ));
    });
}
