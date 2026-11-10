use super::*;
use crate::outbound::mock::{new_tester, MaxMessagePayloadSize, MaxMessagesPerCommit, Test};
use frame_support::{assert_noop, assert_ok};

#[test]
fn bounded_write_preserves_payload_and_queue_limits_independently_of_admission() {
    new_tester().execute_with(|| {
        let network = SubNetworkId::Mainnet;
        let payload = vec![82; MaxMessagePayloadSize::get() as usize];
        assert_ok!(Pallet::<Test>::append_message(network, &payload));
        assert_eq!(
            MessageQueues::<Test>::get(network)[0].payload.as_slice(),
            payload.as_slice()
        );
        assert_noop!(
            Pallet::<Test>::append_message(network, &vec![82; payload.len() + 1]),
            Error::<Test>::PayloadTooLarge
        );
        for _ in 1..MaxMessagesPerCommit::get() {
            assert_ok!(Pallet::<Test>::append_message(network, &[83]));
        }
        let full = MessageQueues::<Test>::get(network);
        assert_noop!(
            Pallet::<Test>::append_message(network, &[84]),
            Error::<Test>::QueueSizeLimitReached
        );
        assert_eq!(MessageQueues::<Test>::get(network), full);
    });
}

#[test]
fn capacity_queries_are_read_only_and_respect_each_networks_queue_boundary() {
    new_tester().execute_with(|| {
        let network = SubNetworkId::Mainnet;
        let limit = MaxMessagesPerCommit::get();
        let root = sp_io::storage::root(sp_runtime::StateVersion::V1);
        assert!(Pallet::<Test>::can_queue_messages(network, 0));
        assert!(Pallet::<Test>::can_queue_messages(network, limit));
        assert!(!Pallet::<Test>::can_queue_messages(network, limit + 1));
        assert!(!Pallet::<Test>::can_queue_messages(network, u32::MAX));
        assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), root);
        assert_ok!(Pallet::<Test>::append_message(network, &[1]));
        assert!(Pallet::<Test>::can_queue_messages(network, limit - 1));
        assert!(!Pallet::<Test>::can_queue_messages(network, limit));
        for _ in 1..limit {
            assert_ok!(Pallet::<Test>::append_message(network, &[2]));
        }
        let root = sp_io::storage::root(sp_runtime::StateVersion::V1);
        assert!(Pallet::<Test>::can_queue_messages(network, 0));
        assert!(!Pallet::<Test>::can_queue_messages(network, 1));
        assert!(Pallet::<Test>::can_queue_messages(
            SubNetworkId::Liberland,
            limit
        ));
        assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), root);
    });
}

#[test]
fn capacity_queries_reject_nonce_exhaustion_and_retained_batch_quarantine() {
    new_tester().execute_with(|| {
        let network = SubNetworkId::Mainnet;
        ChannelNonces::<Test>::insert(network, u64::MAX);
        let root = sp_io::storage::root(sp_runtime::StateVersion::V1);
        assert!(Pallet::<Test>::can_queue_messages(network, 0));
        assert!(!Pallet::<Test>::can_queue_messages(network, 1));
        assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), root);
        ChannelNonces::<Test>::insert(network, 7);
        sp_io::storage::set(
            &bridge_types::recovery::batch_key(1, network.into(), 8),
            &[1],
        );
        let root = sp_io::storage::root(sp_runtime::StateVersion::V1);
        assert!(Pallet::<Test>::can_queue_messages(network, 0));
        assert!(!Pallet::<Test>::can_queue_messages(network, 1));
        assert!(Pallet::<Test>::can_queue_messages(
            SubNetworkId::Liberland,
            1
        ));
        assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), root);
    });
}
