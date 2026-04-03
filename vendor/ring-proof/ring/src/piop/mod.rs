use std::marker::PhantomData;

use ark_ec::AffineRepr;
use ark_ec::short_weierstrass::{Affine, SWCurveConfig};
use ark_ff::PrimeField;
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use fflonk::pcs::{Commitment, PCS, PcsParams};

use common::{Column, ColumnsCommited, ColumnsEvaluated, FieldColumn};
use common::gadgets::sw_cond_add::AffineColumn;
pub(crate) use prover::PiopProver;
pub(crate) use verifier::PiopVerifier;

use crate::PiopParams;

mod prover;
mod verifier;
pub mod params;

#[derive(Clone, CanonicalSerialize, CanonicalDeserialize)]
pub struct RingCommitments<F: PrimeField, C: Commitment<F>> {
    pub(crate) bits: C,
    pub(crate) inn_prod_acc: C,
    pub(crate) cond_add_acc: [C; 2],
    pub(crate) phantom: PhantomData<F>,
}

impl<F: PrimeField, C: Commitment<F>> ColumnsCommited<F, C> for RingCommitments<F, C> {
    fn to_vec(self) -> Vec<C> {
        vec![
            self.bits,
            self.inn_prod_acc,
            self.cond_add_acc[0].clone(),
            self.cond_add_acc[1].clone(),
        ]
    }
}

#[derive(Clone, CanonicalSerialize, CanonicalDeserialize)]
pub struct RingEvaluations<F: PrimeField> {
    pub(crate) points: [F; 2],
    pub(crate) ring_selector: F,
    pub(crate) bits: F,
    pub(crate) inn_prod_acc: F,
    pub(crate) cond_add_acc: [F; 2],
}

impl<F: PrimeField> ColumnsEvaluated<F> for RingEvaluations<F> {
    fn to_vec(self) -> Vec<F> {
        vec![
            self.points[0],
            self.points[1],
            self.ring_selector,
            self.bits,
            self.inn_prod_acc,
            self.cond_add_acc[0],
            self.cond_add_acc[1],
        ]
    }
}

// Columns commitment to which the verifier knows (or trusts).
#[derive(Clone)]
pub struct FixedColumns<F: PrimeField, G: AffineRepr<BaseField=F>> {
    // Public keys of the ring participants in order,
    // followed by the powers-of-2 multiples of the second Pedersen base.
    // pk_1, ..., pk_n, H, 2H, 4H, ..., 2^sH
    // 1          n                     n+s+1
    points: AffineColumn<F, G>,
    // Binary column that highlights which rows of the table correspond to the ring.
    // 1, 1, ..., 1, 0, 0, ..., 0
    // 1          n
    ring_selector: FieldColumn<F>,
}

// Commitments to the fixed columns (see above).
#[derive(Clone, CanonicalSerialize, CanonicalDeserialize)]
pub struct FixedColumnsCommitted<F: PrimeField, C: Commitment<F>> {
    points: [C; 2],
    ring_selector: C,
    phantom: PhantomData<F>,
}

impl<F: PrimeField, C: Commitment<F>> FixedColumnsCommitted<F, C> {
    fn as_vec(&self) -> Vec<C> {
        vec![
            self.points[0].clone(),
            self.points[1].clone(),
            self.ring_selector.clone(),
        ]
    }
}

impl<F: PrimeField, G: AffineRepr<BaseField=F>> FixedColumns<F, G> {
    fn commit<CS: PCS<F>>(&self, ck: &CS::CK) -> FixedColumnsCommitted<F, CS::C> {
        let points = [
            CS::commit(ck, self.points.xs.as_poly()),
            CS::commit(ck, self.points.ys.as_poly()),
        ];
        let ring_selector = CS::commit(ck, self.ring_selector.as_poly());
        FixedColumnsCommitted { points, ring_selector, phantom: Default::default() }
    }
}

pub struct ProverKey<F: PrimeField, CS: PCS<F>, G: AffineRepr<BaseField=F>> {
    pub(crate) pcs_ck: CS::CK,
    pub(crate) fixed_columns: FixedColumns<F, G>,
    pub(crate) verifier_key: VerifierKey<F, CS>, // used in the Fiat-Shamir transform
}


#[derive(Clone, CanonicalSerialize, CanonicalDeserialize)]
pub struct VerifierKey<F: PrimeField, CS: PCS<F>> {
    pub(crate) pcs_raw_vk: <CS::Params as PcsParams>::RVK,
    pub(crate) fixed_columns_committed: FixedColumnsCommitted<F, CS::C>,
    //TODO: domain
}

pub fn index<F: PrimeField, CS: PCS<F>, Curve: SWCurveConfig<BaseField=F>>(
    pcs_params: CS::Params,
    piop_params: &PiopParams<F, Curve>,
    keys: Vec<Affine<Curve>>,
) -> (ProverKey<F, CS, Affine<Curve>>, VerifierKey<F, CS>) {
    let pcs_ck = pcs_params.ck();
    let pcs_raw_vk = pcs_params.raw_vk();
    let fixed_columns = piop_params.fixed_columns(&keys);
    let fixed_columns_committed = fixed_columns.commit::<CS>(&pcs_ck);
    let verifier_key = VerifierKey {
        pcs_raw_vk: pcs_raw_vk.clone(),
        fixed_columns_committed: fixed_columns_committed.clone(),
    };
    let prover_key = ProverKey { pcs_ck, fixed_columns, verifier_key };
    let verifier_key = VerifierKey { pcs_raw_vk, fixed_columns_committed };
    (prover_key, verifier_key)
}
