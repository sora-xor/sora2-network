use ark_ec::short_weierstrass::{Affine, SWCurveConfig};
use ark_ff::PrimeField;
use fflonk::pcs::PCS;

use common::prover::PlonkProver;

use crate::piop::{FixedColumns, PiopProver, ProverKey};
use crate::piop::params::PiopParams;
use crate::RingProof;

pub struct RingProver<F: PrimeField, CS: PCS<F>, Curve: SWCurveConfig<BaseField=F>> {
    piop_params: PiopParams<F, Curve>,
    fixed_columns: FixedColumns<F, Affine<Curve>>,
    k: usize,

    plonk_prover: PlonkProver<F, CS, merlin::Transcript>,
}


impl<F: PrimeField, CS: PCS<F>, Curve: SWCurveConfig<BaseField=F>> RingProver<F, CS, Curve> {
    pub fn init(prover_key: ProverKey<F, CS, Affine<Curve>>,
                piop_params: PiopParams<F, Curve>,
                k: usize,
                empty_transcript: merlin::Transcript,
    ) -> Self {
        let ProverKey { pcs_ck, fixed_columns, verifier_key } = prover_key;

        let plonk_prover = PlonkProver::init(pcs_ck, verifier_key, empty_transcript);

        Self {
            piop_params,
            fixed_columns,
            k,
            plonk_prover,
        }
    }

    pub fn prove(&self, t: Curve::ScalarField) -> RingProof<F, CS> {
        let piop = PiopProver::build(&self.piop_params, self.fixed_columns.clone(), self.k, t);
        self.plonk_prover.prove(piop)
    }
}

