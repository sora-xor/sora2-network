use ark_ec::AffineRepr;
use ark_ec::pairing::Pairing;
use ark_poly::{EvaluationDomain, GeneralEvaluationDomain};
use ark_serialize::*;
use ark_std::{vec, vec::Vec};

use crate::pcs::{CommitterKey, PcsParams, RawVerifierKey, VerifierKey};
use crate::pcs::kzg::lagrange::LagrangianCK;
use crate::pcs::kzg::urs::URS;

impl<E: Pairing> PcsParams for URS<E> {
    type CK = KzgCommitterKey<E::G1Affine>;
    type VK = KzgVerifierKey<E>;
    type RVK = RawKzgVerifierKey<E>;

    fn ck(&self) -> Self::CK {
        let monomial = MonomialCK {
            powers_in_g1: self.powers_in_g1.clone()
        };
        KzgCommitterKey { monomial, lagrangian: None }
    }

    fn ck_with_lagrangian(&self, domain_size: usize) -> Self::CK {
        let domain = GeneralEvaluationDomain::new(domain_size).unwrap();
        assert_eq!(domain.size(), domain_size, "domains of size {} are not supported", domain_size);
        assert!(domain_size <= self.powers_in_g1.len());
        let monomial = MonomialCK {
            powers_in_g1: self.powers_in_g1[0..domain_size].to_vec()
        };
        let lagrangian = Some(monomial.to_lagrangian(domain));
        KzgCommitterKey { monomial, lagrangian }
    }

    fn vk(&self) -> Self::VK {
        self.raw_vk().prepare()
    }

    /// Non-prepared verifier key. Can be used for serialization.
    fn raw_vk(&self) -> Self::RVK {
        assert!(self.powers_in_g1.len() > 0, "no G1 generator");
        assert!(self.powers_in_g2.len() > 1, "{} powers in G2", self.powers_in_g2.len());

        RawKzgVerifierKey {
            g1: self.powers_in_g1[0],
            g2: self.powers_in_g2[0],
            tau_in_g2: self.powers_in_g2[1],
        }
    }
}

#[derive(Clone, Debug, CanonicalSerialize, CanonicalDeserialize)]
pub struct KzgCommitterKey<G: AffineRepr> {
    pub(crate) monomial: MonomialCK<G>,
    pub(crate) lagrangian: Option<LagrangianCK<G>>,
}


/// Used to commit to and to open univariate polynomials of degree up to self.max_degree().
#[derive(Clone, Debug, CanonicalSerialize, CanonicalDeserialize)]
pub struct MonomialCK<G: AffineRepr> {
    // G1, tau.G1, tau^2.G1, ..., tau^n1.G1
    pub(crate) powers_in_g1: Vec<G>,
}

impl<G: AffineRepr> CommitterKey for MonomialCK<G> {
    fn max_degree(&self) -> usize {
        self.powers_in_g1.len() - 1
    }
}

impl<G: AffineRepr> CommitterKey for KzgCommitterKey<G> {
    fn max_degree(&self) -> usize {
        self.monomial.max_degree()
    }
}


/// Verifier key with G2 elements not "prepared". Exists only to be serializable.
/// KzgVerifierKey is used for verification.
#[derive(Clone, Debug, CanonicalSerialize, CanonicalDeserialize)]
pub struct RawKzgVerifierKey<E: Pairing> {
    g1: E::G1Affine,
    // generator of G1
    g2: E::G2Affine,
    // generator of G2
    tau_in_g2: E::G2Affine, // tau.g2
}


impl<E: Pairing> RawVerifierKey for RawKzgVerifierKey<E> {
    type VK = KzgVerifierKey<E>;

    /// Returns the key that is used to verify openings in a single point. It has points in G2 "prepared".
    /// "Preparation" is a pre-computation that makes pairing computation with these points more efficient.
    /// At the same time usual arithmetic operations are not implemented for "prepared" points.
    fn prepare(&self) -> KzgVerifierKey<E> {
        KzgVerifierKey {
            g1: self.g1,
            g2: self.g2.into(),
            tau_in_g2: self.tau_in_g2.into(),
        }
    }
}


/// "Prepared" verifier key capable of verifying opening in a single point, given the commitment is in G1.
/// Use RawKzgVerifierKey for serialization.
#[derive(Clone, Debug)]
pub struct KzgVerifierKey<E: Pairing> {
    // generator of G1
    pub(crate) g1: E::G1Affine,
    // G1Prepared is just a wrapper around G1Affine // TODO: fixed-base precomputations
    // generator of G2, prepared
    pub(crate) g2: E::G2Prepared,
    // G2Prepared can be used as a pairing RHS only
    // tau.g2, prepared
    pub(crate) tau_in_g2: E::G2Prepared, // G2Prepared can be used as a pairing RHS only
}

impl<E: Pairing> VerifierKey for KzgVerifierKey<E> {}

impl<E: Pairing> From<KzgVerifierKey<E>> for KzgCommitterKey<E::G1Affine> {
    fn from(vk: KzgVerifierKey<E>) -> Self {
        let monomial = MonomialCK { powers_in_g1: vec![vk.g1] };
        Self { monomial, lagrangian: None }
    }
}

