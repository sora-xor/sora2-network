use ark_ff::PrimeField;
use fflonk::pcs::Commitment;

use common::domain::EvaluatedDomain;
use common::gadgets::booleanity::BooleanityValues;
use common::gadgets::fixed_cells::FixedCellsValues;
use common::gadgets::inner_prod::InnerProdValues;
use common::gadgets::sw_cond_add::CondAddValues;
use common::gadgets::VerifierGadget;
use common::piop::VerifierPiop;

use crate::piop::{FixedColumnsCommitted, RingCommitments};
use crate::RingEvaluations;

pub struct PiopVerifier<F: PrimeField, C: Commitment<F>> {
    domain_evals: EvaluatedDomain<F>,
    fixed_columns_committed: FixedColumnsCommitted<F, C>,
    witness_columns_committed: RingCommitments<F, C>,
    // Gadget verifiers:
    booleanity: BooleanityValues<F>,
    inner_prod: InnerProdValues<F>,
    inner_prod_acc: FixedCellsValues<F>,
    cond_add: CondAddValues<F>,
    cond_add_acc_x: FixedCellsValues<F>,
    cond_add_acc_y: FixedCellsValues<F>,
}

impl<F: PrimeField, C: Commitment<F>> PiopVerifier<F, C> {
    pub fn init(
        domain_evals: EvaluatedDomain<F>,
        fixed_columns_committed: FixedColumnsCommitted<F, C>,
        witness_columns_committed: RingCommitments<F, C>,
        all_columns_evaluated: RingEvaluations<F>,
        init: (F, F),
        result: (F, F),
    ) -> Self {
        let cond_add = CondAddValues {
            bitmask: all_columns_evaluated.bits,
            points: (all_columns_evaluated.points[0], all_columns_evaluated.points[1]),
            not_last: domain_evals.not_last_row,
            acc: (all_columns_evaluated.cond_add_acc[0], all_columns_evaluated.cond_add_acc[1]),
        };

        let inner_prod = InnerProdValues {
            a: all_columns_evaluated.ring_selector,
            b: all_columns_evaluated.bits,
            not_last: domain_evals.not_last_row,
            acc: all_columns_evaluated.inn_prod_acc,
        };

        let booleanity = BooleanityValues {
            bits: all_columns_evaluated.bits,
        };

        let cond_add_acc_x = FixedCellsValues {
            col: all_columns_evaluated.cond_add_acc[0],
            col_first: init.0,
            col_last: result.0,
            l_first: domain_evals.l_first,
            l_last: domain_evals.l_last,
        };

        let cond_add_acc_y = FixedCellsValues {
            col: all_columns_evaluated.cond_add_acc[1],
            col_first: init.1,
            col_last: result.1,
            l_first: domain_evals.l_first,
            l_last: domain_evals.l_last,
        };

        let inner_prod_acc = FixedCellsValues {
            col: all_columns_evaluated.inn_prod_acc,
            col_first: F::zero(),
            col_last: F::one(),
            l_first: domain_evals.l_first,
            l_last: domain_evals.l_last,
        };

        Self {
            domain_evals,
            fixed_columns_committed,
            witness_columns_committed,
            inner_prod,
            cond_add,
            booleanity,
            cond_add_acc_x,
            cond_add_acc_y,
            inner_prod_acc,
        }
    }
}

impl<F: PrimeField, C: Commitment<F>> VerifierPiop<F, C> for PiopVerifier<F, C> {
    const N_CONSTRAINTS: usize = 7;
    const N_COLUMNS: usize = 7;

    fn precommitted_columns(&self) -> Vec<C> {
        self.fixed_columns_committed.as_vec()
    }

    fn evaluate_constraints_main(&self) -> Vec<F> {
        vec![
            self.inner_prod.evaluate_constraints_main(),
            self.cond_add.evaluate_constraints_main(),
            self.booleanity.evaluate_constraints_main(),
            self.cond_add_acc_x.evaluate_constraints_main(),
            self.cond_add_acc_y.evaluate_constraints_main(),
            self.inner_prod_acc.evaluate_constraints_main(),
        ].concat()
    }

    fn constraint_polynomials_linearized_commitments(&self) -> Vec<C> {
        let inner_prod_acc = self.witness_columns_committed.inn_prod_acc.mul(self.inner_prod.not_last);
        let acc_x = &self.witness_columns_committed.cond_add_acc[0];
        let acc_y = &self.witness_columns_committed.cond_add_acc[1];

        let (c_acc_x, c_acc_y) = self.cond_add.acc_coeffs_1();
        let c1_lin = acc_x.mul(c_acc_x) + acc_y.mul(c_acc_y);

        let (c_acc_x, c_acc_y) = self.cond_add.acc_coeffs_2();
        let c2_lin = acc_x.mul(c_acc_x) + acc_y.mul(c_acc_y);

        vec![
            inner_prod_acc,
            c1_lin,
            c2_lin,
        ]
    }

    fn domain_evaluated(&self) -> &EvaluatedDomain<F> {
        &self.domain_evals
    }
}
