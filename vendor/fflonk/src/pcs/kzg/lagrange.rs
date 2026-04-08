use ark_ec::{AffineRepr, CurveGroup};
use ark_ff::Zero;
use ark_poly::{EvaluationDomain, GeneralEvaluationDomain};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use ark_std::rand::RngCore;
use ark_std::UniformRand;
use ark_std::vec::Vec;

use crate::pcs::CommitterKey;
use crate::pcs::kzg::params::MonomialCK;
use crate::utils::ec::single_base_msm;

/// Used to commit to univariate polynomials represented in the evaluation form.
#[derive(Clone, Debug, CanonicalSerialize, CanonicalDeserialize)]
pub struct LagrangianCK<G: AffineRepr, D: EvaluationDomain<G::ScalarField> = GeneralEvaluationDomain<<G as AffineRepr>::ScalarField>> {
    // L_0(tau).G, L_1(tau).G, ..., L_{n-1}(tau).G
    pub(crate) lis_in_g: Vec<G>,
    pub(crate) domain: D,
}


impl<G: AffineRepr> CommitterKey for LagrangianCK<G> {
    fn max_degree(&self) -> usize {
        self.lis_in_g.len() - 1
    }
}

impl<G: AffineRepr, D: EvaluationDomain<G::ScalarField>> LagrangianCK<G, D> {
    pub fn generate<R: RngCore>(domain: D, rng: &mut R) -> Self {
        let tau = G::ScalarField::rand(rng);
        let g = G::Group::rand(rng);
        Self::from_trapdoor(domain, tau, g)
    }

    pub fn from_trapdoor(domain: D, tau: G::ScalarField, g: G::Group) -> Self {
        assert!(!domain.evaluate_vanishing_polynomial(tau).is_zero()); // doesn't give a basis
        let lis_at_tau = domain.evaluate_all_lagrange_coefficients(tau); // L_i(tau)
        let lis_in_g = single_base_msm(&lis_at_tau, g); // L_i(tau).G
        Self { lis_in_g, domain }
    }
}

impl<G: AffineRepr> MonomialCK<G> {
    pub fn to_lagrangian<D: EvaluationDomain<G::ScalarField>>(&self, domain: D) -> LagrangianCK<G, D> {
        assert!(self.max_evals() >= domain.size());
        let mut monomial_bases = self.powers_in_g1.iter()
            .take(domain.size())
            .map(|p| p.into_group())
            .collect();

        let lagrangian_bases = {
            domain.ifft_in_place(&mut monomial_bases);
            monomial_bases
        };

        let lis_in_g = G::Group::normalize_batch(&lagrangian_bases);
        LagrangianCK {
            lis_in_g,
            domain,
        }
    }
}


#[cfg(test)]
mod tests {
    use ark_ec::pairing::Pairing;
    use ark_poly::{EvaluationDomain, GeneralEvaluationDomain};
    use ark_std::test_rng;

    use crate::pcs::kzg::urs::URS;
    use crate::pcs::PcsParams;
    use crate::tests::TestCurve;

    use super::*;

    #[test]
    fn test_derivation_from_monomial_urs() {
        let rng = &mut test_rng();
        let domain_size = 16;
        let domain = GeneralEvaluationDomain::new(domain_size).unwrap();

        let (tau, g1, g2) = URS::<TestCurve>::random_params(rng);
        let urs = URS::<TestCurve>::from_trapdoor(tau, domain_size, 0, g1, g2);
        let monomial_ck = urs.ck().monomial;
        let lagrangian_ck_from_monomial_urs = monomial_ck.to_lagrangian(domain);

        let lagrangian_ck_from_trapdoor = LagrangianCK::<<TestCurve as Pairing>::G1Affine>::from_trapdoor(domain, tau, g1);
        assert_eq!(lagrangian_ck_from_monomial_urs.lis_in_g, lagrangian_ck_from_trapdoor.lis_in_g);
    }
}