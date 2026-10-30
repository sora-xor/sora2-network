use bridge_types::{
    traits::AuxiliaryDigestHandler,
    types::{AuxiliaryDigest, AuxiliaryDigestItem},
    SubNetworkId,
};
use frame_support::{
    derive_impl,
    traits::{Hooks, Randomness},
};
use sp_consensus_beefy::mmr::BeefyDataProvider;
use sp_core::H256;
use sp_runtime::{
    traits::{Hash, Keccak256},
    BuildStorage,
};

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        LeafProvider: crate,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = frame_system::mocking::MockBlock<Test>;
}

pub struct TestRandomness;
impl Randomness<H256, u64> for TestRandomness {
    fn random(subject: &[u8]) -> (H256, u64) {
        (Keccak256::hash(subject), System::block_number())
    }
}

impl crate::Config for Test {
    type Hash = H256;
    type Hashing = Keccak256;
    type Randomness = TestRandomness;
}

#[test]
fn leaf_provider_exposes_no_dispatchable_calls() {
    use frame_support::traits::{GetCallIndex, GetCallName};
    assert!(crate::Call::<Test>::get_call_names().is_empty());
    assert!(crate::Call::<Test>::get_call_indices().is_empty());
    assert_eq!(
        LeafProvider::call_functions().ty,
        scale_info::meta_type::<crate::Call<Test>>()
    );
}

#[test]
fn leaf_extra_authenticates_digest_order_and_uses_the_randomness_subject() {
    let mut ext: sp_io::TestExternalities = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap()
        .into();
    ext.execute_with(|| {
        assert_eq!(LeafProvider::latest_digest(), None);
        let empty = LeafProvider::extra_data();
        assert_eq!(
            empty.digest_hash,
            Keccak256::hash_of(&AuxiliaryDigest::default())
        );
        assert_eq!(
            empty.random_seed,
            Keccak256::hash(crate::RANDOMNESS_SUBJECT)
        );

        let first =
            AuxiliaryDigestItem::Commitment(SubNetworkId::Mainnet.into(), H256::repeat_byte(1));
        let second =
            AuxiliaryDigestItem::Commitment(SubNetworkId::Kusama.into(), H256::repeat_byte(2));
        LeafProvider::add_item(first);
        LeafProvider::add_item(second);
        assert_eq!(LeafProvider::latest_digest(), Some(vec![first, second]));
        assert_eq!(
            LeafProvider::extra_data().digest_hash,
            Keccak256::hash_of(&AuxiliaryDigest {
                logs: vec![first, second]
            })
        );
        assert_ne!(
            LeafProvider::extra_data().digest_hash,
            Keccak256::hash_of(&AuxiliaryDigest {
                logs: vec![second, first]
            })
        );
        LeafProvider::on_initialize(2);
        assert_eq!(LeafProvider::latest_digest(), None);
        assert_eq!(LeafProvider::extra_data(), empty);
        LeafProvider::add_item(second);
        assert_eq!(LeafProvider::latest_digest(), Some(vec![second]));
    });
}
