#![no_main]

use borsh::{BorshDeserialize, BorshSerialize};
use libfuzzer_sys::fuzz_target;
use sccp_sol_verifier_program::{
    Commitment, MmrLeaf, MmrProof, SoraBurnProofV1, ValidatorProof, ValidatorSet,
};

fn assert_borsh_roundtrip<T>(data: &[u8])
where
    T: BorshDeserialize + BorshSerialize + PartialEq + core::fmt::Debug,
{
    if let Ok(decoded) = T::try_from_slice(data) {
        let encoded = decoded.try_to_vec().expect("borsh encode should succeed");
        let roundtrip = T::try_from_slice(&encoded).expect("borsh decode of own encoding should succeed");
        assert_eq!(roundtrip, decoded);
    }
}

fuzz_target!(|data: &[u8]| {
    assert_borsh_roundtrip::<SoraBurnProofV1>(data);
    assert_borsh_roundtrip::<Commitment>(data);
    assert_borsh_roundtrip::<MmrProof>(data);
    assert_borsh_roundtrip::<MmrLeaf>(data);
    assert_borsh_roundtrip::<ValidatorProof>(data);
    assert_borsh_roundtrip::<ValidatorSet>(data);
});
