use super::*;
use crate::mock::{new_test_ext, BridgeMaxPeers, Test};

fn peers(count: u32) -> BTreeSet<ecdsa::Public> {
    (0..count)
        .map(|index| {
            let mut bytes = [0u8; 33];
            bytes[0] = 2;
            bytes[1..5].copy_from_slice(&index.to_be_bytes());
            ecdsa::Public::from_raw(bytes)
        })
        .collect()
}

#[test]
fn canonicalization_bounds_distinct_peers_after_deduplication() {
    let limit = BridgeMaxPeers::get();
    let maximum = peers(limit);
    let duplicates = maximum.iter().copied().chain(maximum.iter().copied());
    let canonical = Pallet::<Test>::canonicalize_peers(duplicates, true).unwrap();
    assert_eq!(canonical.iter().copied().collect::<BTreeSet<_>>(), maximum);
    assert_eq!(
        Pallet::<Test>::canonicalize_peers(peers(limit + 1), true),
        Err(Error::<Test>::TooMuchPeers.into())
    );
    assert_eq!(
        Pallet::<Test>::canonicalize_peers(Vec::new(), false),
        Err(Error::<Test>::NotEnoughPeers.into())
    );
}

#[test]
fn peer_payload_conversion_preserves_order_and_rejects_oversized_sets() {
    let maximum = peers(BridgeMaxPeers::get());
    assert_eq!(
        Pallet::<Test>::peers_to_vec(&maximum).unwrap().into_inner(),
        maximum.iter().copied().collect::<Vec<_>>()
    );
    assert_eq!(
        Pallet::<Test>::peers_to_vec(&peers(BridgeMaxPeers::get() + 1)),
        Err(Error::<Test>::TooMuchPeers.into())
    );
}

#[test]
fn empty_overlap_check_is_a_read_only_noop_even_with_registered_peers() {
    new_test_ext().execute_with(|| {
        Peers::<Test>::insert(
            GenericNetworkId::Sub(SubNetworkId::Mainnet),
            Pallet::<Test>::canonicalize_peers(peers(4), true).unwrap(),
        );
        let root = sp_io::storage::root(sp_runtime::StateVersion::V1);
        assert_eq!(
            Pallet::<Test>::ensure_no_substrate_peer_overlap(
                GenericNetworkId::Sub(SubNetworkId::Liberland),
                std::iter::empty()
            ),
            Ok(())
        );
        assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), root);
    });
}
