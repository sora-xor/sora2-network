use super::*;
use crate::inbound::test::{new_tester, Test};
use frame_support::{assert_noop, assert_ok};

#[test]
fn nonce_write_rejects_replay_gap_and_overflow_without_mutation() {
    new_tester().execute_with(|| {
        let network = SubNetworkId::Mainnet;
        ChannelNonces::<Test>::insert(network, 5);
        for invalid in [5, 7] {
            assert_noop!(
                Pallet::<Test>::update_channel_nonce(network, invalid),
                Error::<Test>::InvalidNonce
            );
        }
        assert_ok!(Pallet::<Test>::update_channel_nonce(network, 6));
        assert_eq!(ChannelNonces::<Test>::get(network), 6);
        ChannelNonces::<Test>::insert(network, u64::MAX);
        assert_noop!(
            Pallet::<Test>::update_channel_nonce(network, 0),
            Error::<Test>::InvalidNonce
        );
    });
}
