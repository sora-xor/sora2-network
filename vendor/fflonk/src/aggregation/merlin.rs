use ark_ff::PrimeField;
use ark_serialize::{CanonicalSerialize, Compress};
use ark_std::vec;

use crate::aggregation::multiple::Transcript;
use crate::pcs::PCS;

impl<F: PrimeField, CS: PCS<F>> Transcript<F, CS> for merlin::Transcript {
    fn get_gamma(&mut self) -> F {
        let mut buf = [0u8; 16];
        self.challenge_bytes(b"gamma", &mut buf);
        F::from_random_bytes(&buf).unwrap()
    }

    fn commit_to_q(&mut self, q: &CS::C) {
        let mut buf = vec![0; q.serialized_size(Compress::No)];
        q.serialize_uncompressed(&mut buf).unwrap();
        self.append_message(b"q", &buf);
    }

    fn get_zeta(&mut self) -> F {
        let mut buf = [0u8; 16];
        self.challenge_bytes(b"zeta", &mut buf);
        F::from_random_bytes(&buf).unwrap()
    }
}