//! BABE keys are retained across the SORA Nexus recovery, so headers that the
//! abandoned chain signed after the checkpoint must never become recovery
//! slash evidence. Every recovery block uses a later slot, so only abandoned
//! evidence is refused; equivocation within the recovery chain stays slashable.

use crate::incident_recovery_consensus::RecoveryOwnership;
use crate::{AccountId, ReportLongevity, Runtime};
use common::recovery::POLICY_STORAGE_KEY;
use frame_system::pallet_prelude::HeaderFor;
use sp_runtime::{
    transaction_validity::{InvalidTransaction, TransactionValidityError},
    DispatchError,
};
use sp_staking::offence::OffenceReportSystem;

/// BABE slot of the abandoned chain's head, block 28,193,255.
pub const LAST_ABANDONED_BABE_SLOT: u64 = 298_871_044;

type BabeEvidence = (
    sp_consensus_babe::EquivocationProof<HeaderFor<Runtime>>,
    <Runtime as pallet_babe::Config>::KeyOwnerProof,
);
type NativeBabeReports = pallet_babe::EquivocationReportSystem<
    Runtime,
    crate::liveness::EquivocationReports,
    RecoveryOwnership,
    ReportLongevity,
>;

fn babe_report_allowed(evidence: &BabeEvidence) -> bool {
    !sp_io::storage::exists(POLICY_STORAGE_KEY) || *evidence.0.slot > LAST_ABANDONED_BABE_SLOT
}

/// Guards every report entry point, signed and unsigned dispatch included.
pub struct RecoveryBabeReports;
impl OffenceReportSystem<Option<AccountId>, BabeEvidence> for RecoveryBabeReports {
    type Longevity = ReportLongevity;

    fn publish_evidence(evidence: BabeEvidence) -> Result<(), ()> {
        if !babe_report_allowed(&evidence) {
            return Err(());
        }
        NativeBabeReports::publish_evidence(evidence)
    }

    fn check_evidence(evidence: BabeEvidence) -> Result<(), TransactionValidityError> {
        if !babe_report_allowed(&evidence) {
            return Err(InvalidTransaction::BadProof.into());
        }
        NativeBabeReports::check_evidence(evidence)
    }

    fn process_evidence(
        reporter: Option<AccountId>,
        evidence: BabeEvidence,
    ) -> Result<(), DispatchError> {
        if !babe_report_allowed(&evidence) {
            return Err(pallet_babe::Error::<Runtime>::InvalidEquivocationProof.into());
        }
        NativeBabeReports::process_evidence(reporter, evidence)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sp_consensus_babe::{
        digests::{CompatibleDigestItem, PreDigest, SecondaryPlainPreDigest},
        AuthorityPair, EquivocationProof, Slot,
    };
    use sp_core::{crypto::Pair, H256};
    use sp_runtime::{traits::Header as _, DigestItem};

    // A cryptographically valid double-authoring at `slot`, so the native
    // checks reject it only at the (deliberately bogus) ownership proof.
    fn equivocation(slot: u64) -> BabeEvidence {
        let pair = AuthorityPair::from_seed(&[7; 32]);
        let header = |parent: u8| {
            let mut header = crate::Header::new(
                1,
                H256::zero(),
                H256::zero(),
                H256::repeat_byte(parent),
                Default::default(),
            );
            let pre_digest = PreDigest::SecondaryPlain(SecondaryPlainPreDigest {
                authority_index: 0,
                slot: Slot::from(slot),
            });
            header
                .digest_mut()
                .push(<DigestItem as CompatibleDigestItem>::babe_pre_digest(
                    pre_digest,
                ));
            let seal = pair.sign(header.hash().as_ref());
            header
                .digest_mut()
                .push(<DigestItem as CompatibleDigestItem>::babe_seal(seal));
            header
        };
        let proof = EquivocationProof {
            offender: pair.public(),
            slot: Slot::from(slot),
            first_header: header(1),
            second_header: header(2),
        };
        assert!(sp_consensus_babe::check_equivocation_proof(proof.clone()));
        let ownership = sp_session::MembershipProof {
            session: 0,
            trie_nodes: Vec::new(),
            validator_count: 1,
        };
        (proof, ownership)
    }

    fn report(slot: u64) -> DispatchError {
        let (proof, ownership) = equivocation(slot);
        crate::Babe::report_equivocation(
            crate::RuntimeOrigin::signed(AccountId::new([1; 32])),
            Box::new(proof),
            ownership,
        )
        .unwrap_err()
        .error
    }

    #[test]
    fn abandoned_babe_evidence_is_refused_only_after_recovery_installation() {
        sp_io::TestExternalities::default().execute_with(|| {
            let guarded: DispatchError =
                pallet_babe::Error::<Runtime>::InvalidEquivocationProof.into();
            let native: DispatchError =
                pallet_babe::Error::<Runtime>::InvalidKeyOwnershipProof.into();
            // Before the recovery policy exists the evidence reaches the
            // ownership check, as on the abandoned chain itself.
            assert_eq!(report(LAST_ABANDONED_BABE_SLOT), native);
            sp_io::storage::set(POLICY_STORAGE_KEY, &[1; 32]);
            assert_eq!(report(LAST_ABANDONED_BABE_SLOT), guarded);
            assert_eq!(report(1), guarded);
            assert_eq!(report(LAST_ABANDONED_BABE_SLOT + 1), native);
            assert_eq!(
                RecoveryBabeReports::check_evidence(equivocation(LAST_ABANDONED_BABE_SLOT)),
                Err(TransactionValidityError::from(InvalidTransaction::BadProof))
            );
            assert_eq!(
                RecoveryBabeReports::publish_evidence(equivocation(LAST_ABANDONED_BABE_SLOT)),
                Err(())
            );
        });
    }
}
