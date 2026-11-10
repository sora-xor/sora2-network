use crate::mock::{
    test_ext, Referrals as ReferralPallet, Runtime, RuntimeOrigin, System, ALICE, BOB,
    MINTING_ACCOUNT,
};
use crate::{Error, SuspendedReferral, SuspendedReferrers};
use common::ReferrerAccountProvider;
use frame_support::{assert_noop, assert_ok};

#[test]
fn retained_referral_custody_disables_fees_until_exact_owner_reconfirms() {
    test_ext().execute_with(|| {
        assert_ok!(ReferralPallet::set_referrer_to(&ALICE, BOB));
        let owner_refs = frame_system::Account::<Runtime>::get(ALICE);
        let referrer_refs = frame_system::Account::<Runtime>::get(BOB);
        let reverse = crate::Referrals::<Runtime>::get(BOB);
        SuspendedReferrers::<Runtime>::insert(
            ALICE,
            SuspendedReferral {
                referrer: BOB,
                evidence_commitment: [7; 32],
            },
        );
        assert_eq!(
            <ReferralPallet as ReferrerAccountProvider<_>>::get_referrer_account(&ALICE),
            None
        );
        assert_noop!(
            ReferralPallet::set_referrer_to(&ALICE, MINTING_ACCOUNT),
            Error::<Runtime>::InvalidSuspension
        );
        assert_noop!(
            ReferralPallet::confirm_suspended_referrer(RuntimeOrigin::signed(ALICE), BOB),
            Error::<Runtime>::SuspensionUnavailable
        );
        frame_support::storage::unhashed::put_raw(common::recovery::POLICY_STORAGE_KEY, &[1; 32]);
        System::set_block_number(common::recovery::CHECKPOINT_NUMBER as u64 + 1);
        assert_noop!(
            ReferralPallet::confirm_suspended_referrer(RuntimeOrigin::signed(BOB), BOB),
            Error::<Runtime>::SuspensionUnavailable
        );
        assert_noop!(
            ReferralPallet::confirm_suspended_referrer(
                RuntimeOrigin::signed(ALICE),
                MINTING_ACCOUNT
            ),
            Error::<Runtime>::InvalidSuspension
        );
        assert_ok!(ReferralPallet::confirm_suspended_referrer(
            RuntimeOrigin::signed(ALICE),
            BOB
        ));
        assert_eq!(
            <ReferralPallet as ReferrerAccountProvider<_>>::get_referrer_account(&ALICE),
            Some(BOB)
        );
        assert_eq!(frame_system::Account::<Runtime>::get(ALICE), owner_refs);
        assert_eq!(frame_system::Account::<Runtime>::get(BOB), referrer_refs);
        assert_eq!(crate::Referrals::<Runtime>::get(BOB), reverse);
        assert_noop!(
            ReferralPallet::confirm_suspended_referrer(RuntimeOrigin::signed(ALICE), BOB),
            Error::<Runtime>::SuspensionUnavailable
        );
    });
}

#[test]
fn corrupt_suspended_edge_stays_inactive_and_cannot_release_references() {
    test_ext().execute_with(|| {
        assert_ok!(ReferralPallet::set_referrer_to(&ALICE, BOB));
        SuspendedReferrers::<Runtime>::insert(
            ALICE,
            SuspendedReferral {
                referrer: BOB,
                evidence_commitment: [7; 32],
            },
        );
        crate::Referrals::<Runtime>::remove(BOB);
        frame_support::storage::unhashed::put_raw(common::recovery::POLICY_STORAGE_KEY, &[1; 32]);
        System::set_block_number(common::recovery::CHECKPOINT_NUMBER as u64 + 1);
        let before = sp_io::storage::root(sp_runtime::StateVersion::V1);
        assert_noop!(
            ReferralPallet::confirm_suspended_referrer(RuntimeOrigin::signed(ALICE), BOB),
            Error::<Runtime>::InvalidSuspension
        );
        assert_eq!(
            <ReferralPallet as ReferrerAccountProvider<_>>::get_referrer_account(&ALICE),
            None
        );
        assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), before);
    });
}
