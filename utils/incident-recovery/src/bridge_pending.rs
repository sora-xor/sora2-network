//! No-refund disposition of exact, paired checkpoint outgoing liabilities.
//! This is a child of bridge_history so its full prefix projection and atomic
//! owner/index/marker certificate remain the single authority for all records.
use super::*;

#[derive(Debug, Encode, Decode, DecodeWithMemTracking)]
pub(super) struct PendingLiabilityTombstone {
    pub tag: [u8; 8],
    pub checkpoint: [u8; 32],
    pub source_record_hash: [u8; 32],
    pub sender_record_hash: [u8; 32],
    pub original_owner: [u8; 32],
    pub effective_owner: [u8; 32],
    pub evidence: [u8; 32],
    pub attacker_entitlement_extinguished: bool,
    pub refund_authorized: bool,
}

pub(super) fn marker(
    request: &Request,
    owner: &AccountId32,
    effective: &AccountId32,
    sender_value: &[u8],
    request_value: &[u8],
    evidence: [u8; 32],
) -> Result<PendingLiabilityTombstone> {
    if request.direction != MessageDirection::Outbound
        || matches!(
            request.status,
            MessageStatus::Done | MessageStatus::Refunded
        )
        || request.source != GenericAccount::Sora(owner.clone())
        || request.encode() != request_value
        || owner.encode() != sender_value
        || evidence == [0; 32]
        || (AsRef::<[u8; 32]>::as_ref(owner) == &recovery::ATTACKER_ACCOUNT && effective != owner)
    {
        return Err("pending outgoing quarantine requires an exact unchanged native liability and paired local owner".into());
    }
    Ok(PendingLiabilityTombstone {
        tag: *b"snxpend1",
        checkpoint: recovery::CHECKPOINT_HASH,
        source_record_hash: hash(request_value),
        sender_record_hash: hash(sender_value),
        original_owner: *AsRef::<[u8; 32]>::as_ref(owner),
        effective_owner: *AsRef::<[u8; 32]>::as_ref(effective),
        evidence,
        // This prevents restoration as a refundable claim. The native record is
        // immutable evidence, not a new spendable or executable liability.
        attacker_entitlement_extinguished: AsRef::<[u8; 32]>::as_ref(owner)
            == &recovery::ATTACKER_ACCOUNT,
        refund_authorized: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge_history::tests::{account, fixture, review};

    fn pending_fixture(
        status: MessageStatus,
    ) -> (MetadataSchema, Storage, Review, Vec<AccountReplacement>) {
        let (schema, mut storage, _, mapping) = fixture();
        let network = GenericNetworkId::Sub(bridge_types::SubNetworkId::Liberland);
        for number in [1, 3] {
            let owner = if number == 1 { account(1) } else { account(3) };
            let key = transaction_key(network, &owner, H256::repeat_byte(number));
            let mut request: Request = decode(&storage.top[&key]).unwrap();
            request.status = status;
            storage.top.insert(key, request.encode());
        }
        let mut review = review(&storage);
        review.quarantine_all_nonterminal_outgoing = true;
        (schema, storage, review, mapping)
    }

    #[test]
    fn pending_owner_pair_and_no_refund_marker_are_atomic_with_exact_liability_preservation() {
        for status in [
            MessageStatus::InQueue,
            MessageStatus::Committed,
            MessageStatus::Approved,
            MessageStatus::Failed,
        ] {
            let (schema, storage, review, mapping) = pending_fixture(status);
            let compilation = compile(&schema, &storage, &mapping, &review).unwrap();
            assert_eq!(compilation.report.nonterminal_outgoing_requests, 2);
            assert_eq!(compilation.report.quarantined_nonterminal_outgoing, 2);
            assert_eq!(compilation.report.remapped_nonterminal_outgoing, 1);
            assert_eq!(compilation.report.refunds_created, 0);
            assert!(!compilation.report.batch_authorizations_reconciled);
            let network = GenericNetworkId::Sub(bridge_types::SubNetworkId::Liberland);
            let id = H256::repeat_byte(1);
            let before: Request =
                decode(&storage.top[&transaction_key(network, &account(1), id)]).unwrap();
            let mut after: Request =
                decode(&compilation.expected[&transaction_key(network, &account(2), id)]).unwrap();
            assert_eq!(after.source, GenericAccount::Sora(account(2)));
            after.source = before.source.clone();
            assert_eq!(after, before);
            let marker: PendingLiabilityTombstone = decode(
                &compilation.expected[&bridge_types::recovery::historical_message_key(network, id)],
            )
            .unwrap();
            assert_eq!(marker.tag, *b"snxpend1");
            assert_eq!(marker.effective_owner, [2; 32]);
            assert!(!marker.refund_authorized);
            assert!(!marker.attacker_entitlement_extinguished);
            compilation
                .verify_groups(&compilation.groups, &mapping)
                .unwrap();
            let split: Vec<_> = compilation
                .groups
                .iter()
                .flatten()
                .map(|e| vec![e.clone()])
                .collect();
            assert!(compilation.verify_groups(&split, &mapping).is_err());
            let mut no_marker = compilation.groups.clone();
            for group in &mut no_marker {
                group.retain(|e| !e.key.starts_with(MARKERS));
            }
            assert!(compilation.verify_groups(&no_marker, &mapping).is_err());
        }
    }

    #[test]
    fn pending_quarantine_requires_explicit_policy_and_does_not_claim_incoming_reconciliation() {
        let (schema, mut storage, mut review, mapping) = pending_fixture(MessageStatus::Approved);
        review.quarantine_all_nonterminal_outgoing = false;
        assert!(compile(&schema, &storage, &mapping, &review).is_err());
        review.quarantine_all_nonterminal_outgoing = true;
        let network = GenericNetworkId::Sub(bridge_types::SubNetworkId::Liberland);
        let key = transaction_key(network, &account(1), H256::repeat_byte(2));
        let mut request: Request = decode(&storage.top[&key]).unwrap();
        request.status = MessageStatus::Committed;
        storage.top.insert(key, request.encode());
        let mut review = super::super::tests::review(&storage);
        review.quarantine_all_nonterminal_outgoing = true;
        assert!(compile(&schema, &storage, &mapping, &review).is_err());
    }

    #[test]
    fn attacker_pending_claim_is_extinguished_without_changing_amount_or_issuing_refund() {
        let (schema, mut storage, _, _) = pending_fixture(MessageStatus::Approved);
        let network = GenericNetworkId::Sub(bridge_types::SubNetworkId::Liberland);
        let id = H256::repeat_byte(3);
        let old_key = transaction_key(network, &account(3), id);
        let mut request: Request = decode(&storage.top.remove(&old_key).unwrap()).unwrap();
        let attacker = AccountId32::new(recovery::ATTACKER_ACCOUNT);
        request.source = GenericAccount::Sora(attacker.clone());
        storage
            .top
            .insert(transaction_key(network, &attacker, id), request.encode());
        storage
            .top
            .insert(sender_key(network, id), attacker.encode());
        let mut review = review(&storage);
        review.quarantine_all_nonterminal_outgoing = true;
        let c = compile(&schema, &storage, &[], &review).unwrap();
        assert_eq!(c.report.extinguished_attacker_pending_entitlements, 1);
        assert_eq!(
            c.expected[&transaction_key(network, &attacker, id)],
            request.encode()
        );
        let marker: PendingLiabilityTombstone =
            decode(&c.expected[&bridge_types::recovery::historical_message_key(network, id)])
                .unwrap();
        assert!(marker.attacker_entitlement_extinguished);
        assert!(!marker.refund_authorized);
    }
}
