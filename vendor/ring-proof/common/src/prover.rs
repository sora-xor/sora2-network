use ark_ff::PrimeField;
use ark_poly::{Evaluations, Polynomial};
use ark_serialize::CanonicalSerialize;
use fflonk::aggregation::single::aggregate_polys;
use fflonk::pcs::PCS;

use crate::piop::ProverPiop;
use crate::Proof;
use crate::transcript::Transcript;

pub struct PlonkProver<F: PrimeField, CS: PCS<F>, T: Transcript<F, CS>> {
    // Polynomial commitment scheme committer's key.
    pcs_ck: CS::CK,
    // Transcript,
    // initialized with the public parameters and the commitments to the precommitted columns.
    transcript_prelude: T,
}

impl<F: PrimeField, CS: PCS<F>, T: Transcript<F, CS>> PlonkProver<F, CS, T> {
    pub fn init(pcs_ck: CS::CK,
                verifier_key: impl CanonicalSerialize, //TODO: a type,
                empty_transcript: T) -> Self {
        let mut transcript_prelude = empty_transcript;
        transcript_prelude._add_serializable(b"vk", &verifier_key);

        Self {
            pcs_ck,
            transcript_prelude,
        }
    }

    pub fn prove<P>(&self, piop: P) -> Proof<F, CS, P::Commitments, P::Evaluations>
        where P: ProverPiop<F, CS::C>
    {
        let mut transcript = self.transcript_prelude.clone();
        transcript.add_instance(&piop.result());
        // ROUND 1
        // The prover commits to the columns.
        let column_commitments = piop.committed_columns(|p| CS::commit(&self.pcs_ck, p));
        transcript.add_committed_cols(&column_commitments);

        // ROUND 2
        let constraint_polys = piop.constraints();
        let alphas = transcript.get_constraints_aggregation_coeffs(constraint_polys.len());
        // Aggregate constraint polynomials in evaluation form...
        let agg_constraint_poly = Self::aggregate_evaluations(&constraint_polys, &alphas);
        // ...and then interpolate (to save some FFTs).
        let agg_constraint_poly = agg_constraint_poly.interpolate();
        let quotient_poly = piop.domain().divide_by_vanishing_poly(&agg_constraint_poly);
        // The prover commits to the quotient polynomial...
        let quotient_commitment = CS::commit(&self.pcs_ck, &quotient_poly);
        transcript.add_quotient_commitment(&quotient_commitment);

        // and receives the evaluation point in response

        // ROUND 3
        let zeta = transcript.get_evaluation_point();
        let columns_to_open = piop.columns();
        let columns_at_zeta = piop.columns_evaluated(&zeta);
        let constraint_polys_linearized = piop.constraints_lin(&zeta);
        let lin = aggregate_polys(&constraint_polys_linearized, &alphas);
        let omega = piop.domain().omega();
        let zeta_omega = zeta * omega;
        let lin_at_zeta_omega = lin.evaluate(&zeta_omega);
        transcript.add_evaluations(&columns_at_zeta, &lin_at_zeta_omega);


        let polys_at_zeta = [columns_to_open, vec![quotient_poly]].concat();
        let nus = transcript.get_kzg_aggregation_challenges(polys_at_zeta.len());
        let agg_at_zeta = aggregate_polys(&polys_at_zeta, &nus);
        let agg_at_zeta_proof = CS::open(&self.pcs_ck, &agg_at_zeta, zeta);
        let lin_at_zeta_omega_proof = CS::open(&self.pcs_ck, &lin, zeta_omega);
        Proof {
            column_commitments,
            quotient_commitment,
            columns_at_zeta,
            lin_at_zeta_omega,
            agg_at_zeta_proof,
            lin_at_zeta_omega_proof,
        }
    }

    fn aggregate_evaluations(polys: &[Evaluations<F>], coeffs: &[F]) -> Evaluations<F> {
        assert_eq!(coeffs.len(), polys.len());
        polys.iter().zip(coeffs.iter())
            .map(|(p, &c)| p * c)
            .reduce(|acc, p| &acc + &p).unwrap()
    }
}


