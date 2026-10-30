use super::*;
use crate::outbound::test::{new_tester, MaxMessagesPerCommit, Test};
use frame_support::{assert_noop, assert_ok};

#[test]
fn bounded_append_keeps_a_full_queue_intact() {
    new_tester().execute_with(|| {
        let network = GenericNetworkId::Sub(SubNetworkId::Mainnet);
        let message = GenericBridgeMessage::Sub(bridge_types::substrate::BridgeMessage {
            payload: vec![81].try_into().unwrap(),
            timepoint: GenericTimepoint::Pending,
        });
        for _ in 0..MaxMessagesPerCommit::get() {
            assert_ok!(Pallet::<Test>::append_message(network, message.clone()));
        }
        let full = MessageQueues::<Test>::get(network);
        assert_noop!(
            Pallet::<Test>::append_message(network, message),
            Error::<Test>::QueueSizeLimitReached
        );
        assert_eq!(MessageQueues::<Test>::get(network), full);
    });
}
