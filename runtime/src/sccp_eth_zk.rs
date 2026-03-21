use alloc::{vec, vec::Vec};

use codec::Decode;
use sp_core::keccak_256;
use winter_air::{BatchingMethod, FieldExtension};
use winter_verifier::{
    crypto::{hashers::Blake3_256, DefaultRandomCoin, MerkleTree},
    math::{fields::f128::BaseElement, FieldElement, ToElements},
    verify, AcceptableOptions, Air, AirContext, Assertion, EvaluationFrame, Proof, ProofOptions,
    TraceInfo, TransitionConstraintDegree,
};

const ETH_ZK_TRACE_WIDTH: usize = 11;
const ETH_ZK_TRACE_LENGTH: usize = 16;

const fn eth_zk_proof_options_v1() -> ProofOptions {
    ProofOptions::new(
        32,
        8,
        0,
        FieldExtension::None,
        8,
        31,
        BatchingMethod::Linear,
        BatchingMethod::Linear,
    )
}

#[derive(Clone)]
struct EthBurnStarkPublicInputs {
    elements: [BaseElement; sccp::ETH_ZK_PUBLIC_INPUT_COUNT_V1],
}

impl EthBurnStarkPublicInputs {
    fn from_sccp(public_inputs: &sccp::EthZkFinalizedBurnPublicInputsV1) -> Self {
        let packed = sccp::eth_zk_public_inputs_v1(public_inputs);
        Self {
            elements: packed.map(BaseElement::new),
        }
    }

    #[cfg(test)]
    fn into_sccp(self) -> sccp::EthZkFinalizedBurnPublicInputsV1 {
        use sp_core::H256;

        fn limbs_to_bytes(left: BaseElement, right: BaseElement) -> [u8; 32] {
            let mut bytes = [0u8; 32];
            bytes[..16]
                .copy_from_slice(&winter_verifier::math::StarkField::as_int(&left).to_be_bytes());
            bytes[16..]
                .copy_from_slice(&winter_verifier::math::StarkField::as_int(&right).to_be_bytes());
            bytes
        }

        let message_id = limbs_to_bytes(self.elements[0], self.elements[1]);
        let finalized_block_hash = limbs_to_bytes(self.elements[2], self.elements[3]);
        let execution_state_root = limbs_to_bytes(self.elements[4], self.elements[5]);
        let router_address_bytes = limbs_to_bytes(self.elements[6], self.elements[7]);
        let burn_storage_key = limbs_to_bytes(self.elements[8], self.elements[9]);

        let mut router_address = [0u8; 20];
        router_address.copy_from_slice(&router_address_bytes[12..]);

        sccp::EthZkFinalizedBurnPublicInputsV1 {
            message_id: H256(message_id),
            finalized_block_hash: H256(finalized_block_hash),
            execution_state_root: H256(execution_state_root),
            router_address,
            burn_storage_key: H256(burn_storage_key),
        }
    }
}

impl ToElements<BaseElement> for EthBurnStarkPublicInputs {
    fn to_elements(&self) -> Vec<BaseElement> {
        self.elements.to_vec()
    }
}

struct EthBurnStatementAir {
    context: AirContext<BaseElement>,
    input_elements: [BaseElement; sccp::ETH_ZK_PUBLIC_INPUT_COUNT_V1],
    expected_accumulator: BaseElement,
}

impl EthBurnStatementAir {
    fn accumulator_target(
        input_elements: &[BaseElement; sccp::ETH_ZK_PUBLIC_INPUT_COUNT_V1],
    ) -> BaseElement {
        input_elements
            .iter()
            .copied()
            .fold(BaseElement::ZERO, |acc, value| acc + value)
    }
}

impl Air for EthBurnStatementAir {
    type BaseField = BaseElement;
    type PublicInputs = EthBurnStarkPublicInputs;

    fn new(trace_info: TraceInfo, pub_inputs: Self::PublicInputs, options: ProofOptions) -> Self {
        assert_eq!(trace_info.width(), ETH_ZK_TRACE_WIDTH);
        assert_eq!(trace_info.length(), ETH_ZK_TRACE_LENGTH);

        let degrees = vec![TransitionConstraintDegree::new(1); ETH_ZK_TRACE_WIDTH];
        let num_assertions = 1 + sccp::ETH_ZK_PUBLIC_INPUT_COUNT_V1 + 1;
        let expected_accumulator = Self::accumulator_target(&pub_inputs.elements);

        Self {
            context: AirContext::new(trace_info, degrees, num_assertions, options),
            input_elements: pub_inputs.elements,
            expected_accumulator,
        }
    }

    fn context(&self) -> &AirContext<Self::BaseField> {
        &self.context
    }

    fn evaluate_transition<E: FieldElement + From<Self::BaseField>>(
        &self,
        frame: &EvaluationFrame<E>,
        _periodic_values: &[E],
        result: &mut [E],
    ) {
        let current = frame.current();
        let next = frame.next();

        // Column 0 accumulates the left-shifted public inputs from columns 1..=10.
        result[0] = next[0] - current[0] - current[1];

        // Shift the public inputs left one column per row.
        for offset in 0..(sccp::ETH_ZK_PUBLIC_INPUT_COUNT_V1 - 1) {
            result[offset + 1] = next[offset + 1] - current[offset + 2];
        }

        // The tail column becomes zero after the shift.
        result[sccp::ETH_ZK_PUBLIC_INPUT_COUNT_V1] = next[ETH_ZK_TRACE_WIDTH - 1];
    }

    fn get_assertions(&self) -> Vec<Assertion<Self::BaseField>> {
        let mut assertions = Vec::with_capacity(1 + sccp::ETH_ZK_PUBLIC_INPUT_COUNT_V1 + 1);
        assertions.push(Assertion::single(0, 0, BaseElement::ZERO));
        for (index, value) in self.input_elements.iter().copied().enumerate() {
            assertions.push(Assertion::single(index + 1, 0, value));
        }
        assertions.push(Assertion::single(
            0,
            ETH_ZK_TRACE_LENGTH - 1,
            self.expected_accumulator,
        ));
        assertions
    }
}

pub(crate) fn verify_stark_v1(proof: &sccp::EthZkFinalizedBurnProofV1) -> bool {
    let Ok(stark_proof) = Proof::from_bytes(&proof.zk_proof) else {
        return false;
    };
    let public_inputs = EthBurnStarkPublicInputs::from_sccp(&proof.public_inputs);
    let acceptable_options = AcceptableOptions::OptionSet(vec![eth_zk_proof_options_v1()]);

    verify::<
        EthBurnStatementAir,
        Blake3_256<BaseElement>,
        DefaultRandomCoin<Blake3_256<BaseElement>>,
        MerkleTree<Blake3_256<BaseElement>>,
    >(stark_proof, public_inputs, &acceptable_options)
    .is_ok()
}

pub(crate) fn verify_evm_burn_proof_binding_v1(proof: &sccp::EthZkFinalizedBurnProofV1) -> bool {
    let mut input = proof.evm_burn_proof.as_slice();
    let Ok(decoded) = sccp::EthZkEvmBurnProofV1::decode(&mut input) else {
        return false;
    };
    if !input.is_empty() || decoded.execution_header_rlp.len() > sccp::SCCP_MAX_BSC_HEADER_RLP_BYTES
    {
        return false;
    }

    if decoded.account_proof.len() > sccp::SCCP_MAX_EVM_PROOF_NODES
        || decoded.storage_proof.len() > sccp::SCCP_MAX_EVM_PROOF_NODES
    {
        return false;
    }
    let mut total = 0usize;
    for node in decoded
        .account_proof
        .iter()
        .chain(decoded.storage_proof.iter())
    {
        if node.len() > sccp::SCCP_MAX_EVM_PROOF_NODE_BYTES {
            return false;
        }
        total = total.saturating_add(node.len());
        if total > sccp::SCCP_MAX_EVM_PROOF_TOTAL_BYTES {
            return false;
        }
    }

    let header_hash = keccak_256(&decoded.execution_header_rlp);
    if header_hash != proof.public_inputs.finalized_block_hash.0 {
        return false;
    }
    let Some(header) =
        sccp::evm_proof::parse_execution_header_minimal(&decoded.execution_header_rlp)
    else {
        return false;
    };
    let header_state_root = sp_core::H256::from_slice(header.state_root);
    if header_state_root != proof.public_inputs.execution_state_root {
        return false;
    }

    let account_key = keccak_256(&proof.public_inputs.router_address);
    let Some(account_val_rlp) =
        sccp::evm_proof::mpt_get(header_state_root, &account_key, &decoded.account_proof)
    else {
        return false;
    };
    let Some(storage_root) = sccp::evm_proof::evm_account_storage_root(&account_val_rlp) else {
        return false;
    };
    let Some(storage_val_rlp) = sccp::evm_proof::mpt_get(
        storage_root,
        &proof.public_inputs.burn_storage_key.0,
        &decoded.storage_proof,
    ) else {
        return false;
    };
    let payload = sccp::evm_proof::rlp_decode_bytes_payload(&storage_val_rlp).unwrap_or(&[]);
    payload.iter().any(|&byte| byte != 0)
}

#[cfg(test)]
fn build_trace(public_inputs: &EthBurnStarkPublicInputs) -> winterfell::TraceTable<BaseElement> {
    let mut trace = winterfell::TraceTable::new(ETH_ZK_TRACE_WIDTH, ETH_ZK_TRACE_LENGTH);
    trace.fill(
        |state| {
            state[0] = BaseElement::ZERO;
            for (index, value) in public_inputs.elements.iter().copied().enumerate() {
                state[index + 1] = value;
            }
        },
        |_, state| {
            let shifted = state[1];
            state[0] += shifted;
            for column in 1..ETH_ZK_TRACE_WIDTH - 1 {
                state[column] = state[column + 1];
            }
            state[ETH_ZK_TRACE_WIDTH - 1] = BaseElement::ZERO;
        },
    );
    trace
}

#[cfg(test)]
struct EthBurnStatementProver {
    options: winterfell::ProofOptions,
}

#[cfg(test)]
impl EthBurnStatementProver {
    fn new() -> Self {
        Self {
            options: eth_zk_proof_options_v1(),
        }
    }
}

#[cfg(test)]
impl winterfell::Prover for EthBurnStatementProver {
    type BaseField = BaseElement;
    type Air = EthBurnStatementAir;
    type Trace = winterfell::TraceTable<Self::BaseField>;
    type HashFn = winterfell::crypto::hashers::Blake3_256<Self::BaseField>;
    type VC = winterfell::crypto::MerkleTree<Self::HashFn>;
    type RandomCoin = winterfell::crypto::DefaultRandomCoin<Self::HashFn>;
    type TraceLde<E>
        = winterfell::DefaultTraceLde<E, Self::HashFn, Self::VC>
    where
        E: winterfell::math::FieldElement<BaseField = Self::BaseField>;
    type ConstraintCommitment<E>
        = winterfell::DefaultConstraintCommitment<E, Self::HashFn, Self::VC>
    where
        E: winterfell::math::FieldElement<BaseField = Self::BaseField>;
    type ConstraintEvaluator<'a, E>
        = winterfell::DefaultConstraintEvaluator<'a, Self::Air, E>
    where
        E: winterfell::math::FieldElement<BaseField = Self::BaseField>;

    fn get_pub_inputs(&self, trace: &Self::Trace) -> EthBurnStarkPublicInputs {
        let mut elements = [BaseElement::ZERO; sccp::ETH_ZK_PUBLIC_INPUT_COUNT_V1];
        for (index, element) in elements.iter_mut().enumerate() {
            *element = trace.get(index + 1, 0);
        }
        EthBurnStarkPublicInputs { elements }
    }

    fn options(&self) -> &winterfell::ProofOptions {
        &self.options
    }

    fn new_trace_lde<E: winterfell::math::FieldElement<BaseField = Self::BaseField>>(
        &self,
        trace_info: &winterfell::TraceInfo,
        main_trace: &winterfell::matrix::ColMatrix<Self::BaseField>,
        domain: &winterfell::StarkDomain<Self::BaseField>,
        partition_option: winterfell::PartitionOptions,
    ) -> (Self::TraceLde<E>, winterfell::TracePolyTable<E>) {
        winterfell::DefaultTraceLde::new(trace_info, main_trace, domain, partition_option)
    }

    fn build_constraint_commitment<
        E: winterfell::math::FieldElement<BaseField = Self::BaseField>,
    >(
        &self,
        composition_poly_trace: winterfell::CompositionPolyTrace<E>,
        num_constraint_composition_columns: usize,
        domain: &winterfell::StarkDomain<Self::BaseField>,
        partition_options: winterfell::PartitionOptions,
    ) -> (
        Self::ConstraintCommitment<E>,
        winterfell::CompositionPoly<E>,
    ) {
        winterfell::DefaultConstraintCommitment::new(
            composition_poly_trace,
            num_constraint_composition_columns,
            domain,
            partition_options,
        )
    }

    fn new_evaluator<'a, E: winterfell::math::FieldElement<BaseField = Self::BaseField>>(
        &self,
        air: &'a Self::Air,
        aux_rand_elements: Option<winterfell::AuxRandElements<E>>,
        composition_coefficients: winterfell::ConstraintCompositionCoefficients<E>,
    ) -> Self::ConstraintEvaluator<'a, E> {
        winterfell::DefaultConstraintEvaluator::new(
            air,
            aux_rand_elements,
            composition_coefficients,
        )
    }
}

#[cfg(test)]
pub(crate) fn build_test_fixture_v1(
    public_inputs: &sccp::EthZkFinalizedBurnPublicInputsV1,
) -> Vec<u8> {
    use winterfell::Prover as _;

    let inputs = EthBurnStarkPublicInputs::from_sccp(public_inputs);
    let trace = build_trace(&inputs);
    let prover = EthBurnStatementProver::new();
    prover.prove(trace).expect("test stark proof").to_bytes()
}

#[cfg(test)]
mod tests {
    use super::{
        build_test_fixture_v1, verify_evm_burn_proof_binding_v1, verify_stark_v1,
        EthBurnStarkPublicInputs,
    };
    use codec::Encode;
    use sp_core::keccak_256;
    use sp_core::H256;
    use winter_verifier::math::fields::f128::BaseElement;

    fn test_leaf_path_for_hashed_key(key32: &[u8; 32]) -> Vec<u8> {
        let mut out = Vec::with_capacity(33);
        out.push(0x20);
        out.extend_from_slice(key32);
        out
    }

    fn test_rlp_leaf_node(compact_path: &[u8], value: &[u8]) -> Vec<u8> {
        let mut stream = rlp::RlpStream::new_list(2);
        stream.append(&compact_path);
        stream.append(&value);
        stream.out().to_vec()
    }

    fn test_rlp_account_value(storage_root: H256) -> Vec<u8> {
        let mut stream = rlp::RlpStream::new_list(4);
        stream.append(&1u8);
        stream.append(&0u8);
        stream.append(&storage_root.as_bytes());
        stream.append(&[7u8; 32].as_slice());
        stream.out().to_vec()
    }

    fn test_execution_header_rlp(state_root: H256) -> Vec<u8> {
        let mut stream = rlp::RlpStream::new_list(4);
        stream.append(&[0x31u8; 32].as_slice());
        stream.append(&[0x32u8; 32].as_slice());
        stream.append(&[0x33u8; 20].as_slice());
        stream.append(&state_root.as_bytes());
        stream.out().to_vec()
    }

    fn build_public_inputs_fixture(
        message_id: H256,
        router_address: [u8; 20],
        burn_storage_key: H256,
    ) -> sccp::EthZkFinalizedBurnPublicInputsV1 {
        let storage_path = test_leaf_path_for_hashed_key(&burn_storage_key.0);
        let storage_leaf = test_rlp_leaf_node(&storage_path, &[0x01]);
        let storage_root = H256::from_slice(&keccak_256(&storage_leaf));

        let account_key = keccak_256(&router_address);
        let account_path = test_leaf_path_for_hashed_key(&account_key);
        let account_value = test_rlp_account_value(storage_root);
        let account_leaf = test_rlp_leaf_node(&account_path, &account_value);
        let execution_state_root = H256::from_slice(&keccak_256(&account_leaf));
        let execution_header_rlp = test_execution_header_rlp(execution_state_root);
        let finalized_block_hash = H256::from_slice(&keccak_256(&execution_header_rlp));

        sccp::EthZkFinalizedBurnPublicInputsV1 {
            message_id,
            finalized_block_hash,
            execution_state_root,
            router_address,
            burn_storage_key,
        }
    }

    fn build_evm_burn_proof_fixture(
        public_inputs: &sccp::EthZkFinalizedBurnPublicInputsV1,
    ) -> sccp::EthZkFinalizedBurnProofV1 {
        let storage_path = test_leaf_path_for_hashed_key(&public_inputs.burn_storage_key.0);
        let storage_leaf = test_rlp_leaf_node(&storage_path, &[0x01]);
        let storage_root = H256::from_slice(&keccak_256(&storage_leaf));

        let account_key = keccak_256(&public_inputs.router_address);
        let account_path = test_leaf_path_for_hashed_key(&account_key);
        let account_value = test_rlp_account_value(storage_root);
        let account_leaf = test_rlp_leaf_node(&account_path, &account_value);
        let execution_state_root = H256::from_slice(&keccak_256(&account_leaf));
        assert_eq!(execution_state_root, public_inputs.execution_state_root);

        let execution_header_rlp = test_execution_header_rlp(execution_state_root);
        let finalized_block_hash = H256::from_slice(&keccak_256(&execution_header_rlp));
        assert_eq!(finalized_block_hash, public_inputs.finalized_block_hash);

        sccp::EthZkFinalizedBurnProofV1 {
            version: sccp::ETH_ZK_FINALIZED_BURN_PROOF_VERSION_V1,
            public_inputs: public_inputs.clone(),
            evm_burn_proof: sccp::EthZkEvmBurnProofV1 {
                execution_header_rlp,
                account_proof: vec![account_leaf],
                storage_proof: vec![storage_leaf],
            }
            .encode(),
            zk_proof: build_test_fixture_v1(public_inputs),
        }
    }

    #[test]
    fn stark_backend_accepts_valid_fixture_and_rejects_mutation() {
        let public_inputs = sccp::EthZkFinalizedBurnPublicInputsV1 {
            message_id: H256([0x11; 32]),
            finalized_block_hash: H256([0x22; 32]),
            execution_state_root: H256([0x33; 32]),
            router_address: [0x44; 20],
            burn_storage_key: H256([0x55; 32]),
        };
        let proof = sccp::EthZkFinalizedBurnProofV1 {
            version: sccp::ETH_ZK_FINALIZED_BURN_PROOF_VERSION_V1,
            public_inputs: public_inputs.clone(),
            evm_burn_proof: vec![],
            zk_proof: build_test_fixture_v1(&public_inputs),
        };
        assert!(verify_stark_v1(&proof));

        let mut mutated = proof.clone();
        mutated.public_inputs.message_id = H256([0xaa; 32]);
        assert!(!verify_stark_v1(&mutated));
    }

    #[test]
    fn stark_public_inputs_round_trip() {
        let original = EthBurnStarkPublicInputs {
            elements: [
                BaseElement::new(1u128),
                BaseElement::new(2u128),
                BaseElement::new(3u128),
                BaseElement::new(4u128),
                BaseElement::new(5u128),
                BaseElement::new(6u128),
                BaseElement::new(7u128),
                BaseElement::new(8u128),
                BaseElement::new(9u128),
                BaseElement::new(10u128),
            ],
        };
        assert_eq!(
            EthBurnStarkPublicInputs::from_sccp(&original.clone().into_sccp()).elements,
            original.elements
        );
    }

    #[test]
    fn evm_burn_binding_accepts_matching_execution_header() {
        let public_inputs =
            build_public_inputs_fixture(H256([0x11; 32]), [0x44; 20], H256([0x55; 32]));
        let proof = build_evm_burn_proof_fixture(&public_inputs);
        assert!(verify_evm_burn_proof_binding_v1(&proof));
    }

    #[test]
    fn evm_burn_binding_rejects_header_hash_mismatch() {
        let public_inputs =
            build_public_inputs_fixture(H256([0x11; 32]), [0x44; 20], H256([0x55; 32]));
        let mut proof = build_evm_burn_proof_fixture(&public_inputs);
        proof.public_inputs.finalized_block_hash = H256([0xaa; 32]);
        assert!(!verify_evm_burn_proof_binding_v1(&proof));
    }

    #[test]
    fn evm_burn_binding_rejects_state_root_mismatch() {
        let public_inputs =
            build_public_inputs_fixture(H256([0x11; 32]), [0x44; 20], H256([0x55; 32]));
        let mut proof = build_evm_burn_proof_fixture(&public_inputs);
        proof.public_inputs.execution_state_root = H256([0xbb; 32]);
        assert!(!verify_evm_burn_proof_binding_v1(&proof));
    }
}
