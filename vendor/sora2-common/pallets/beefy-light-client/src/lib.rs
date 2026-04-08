// This file is part of the SORA network and Polkaswap app.

// Copyright (c) 2020, 2021, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

// Redistribution and use in source and binary forms, with or without modification,
// are permitted provided that the following conditions are met:

// Redistributions of source code must retain the above copyright notice, this list
// of conditions and the following disclaimer.
// Redistributions in binary form must reproduce the above copyright notice, this
// list of conditions and the following disclaimer in the documentation and/or other
// materials provided with the distribution.
//
// All advertising materials mentioning features or use of this software must display
// the following acknowledgement: This product includes software developed by Polka Biome
// Ltd., SORA, and Polkaswap.
//
// Neither the name of the Polka Biome Ltd. nor the names of its contributors may be used
// to endorse or promote products derived from this software without specific prior written permission.

// THIS SOFTWARE IS PROVIDED BY Polka Biome Ltd. AS IS AND ANY EXPRESS OR IMPLIED WARRANTIES,
// INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL Polka Biome Ltd. BE LIABLE FOR ANY
// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
// BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS;
// OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
// USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

#![cfg_attr(not(feature = "std"), no_std)]

use bridge_common::simplified_proof::*;
use bridge_common::{beefy_types::*, bitfield, simplified_proof::Proof};
use bridge_types::types::AuxiliaryDigest;
use bridge_types::types::AuxiliaryDigestItem;
use bridge_types::{GenericNetworkId, SubNetworkId};
use codec::Decode;
use codec::Encode;
use frame_support::ensure;
use frame_support::fail;
use frame_support::pallet_prelude::*;
use frame_support::traits::Randomness;
use frame_system::pallet_prelude::*;
pub use pallet::*;
use scale_info::prelude::vec::Vec;
use sp_core::H256;
use sp_core::{Get, RuntimeDebug};
use sp_io::hashing::keccak_256;
use sp_runtime::traits::Hash;
use sp_runtime::traits::Keccak256;
use sp_std::collections::vec_deque::VecDeque;

pub const MMR_ROOT_HISTORY_SIZE: usize = 30;
pub const THRESHOLD_NUMERATOR: u32 = 22;
pub const THRESHOLD_DENOMINATOR: u32 = 59;
pub const RANDOMNESS_SUBJECT: &[u8] = b"beefy-light-client";

pub use bitfield::BitField;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod fixtures;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[derive(Clone, RuntimeDebug, Encode, Decode, PartialEq, Eq, scale_info::TypeInfo)]
pub struct SubstrateBridgeMessageProof {
    pub proof: Proof<H256>,
    pub leaf: BeefyMMRLeaf,
    pub digest: AuxiliaryDigest,
}

impl codec::DecodeWithMemTracking for SubstrateBridgeMessageProof {}

fn recover_signature(sig: &[u8; 65], msg_hash: &H256) -> Option<EthAddress> {
    use sp_io::crypto::secp256k1_ecdsa_recover;

    secp256k1_ecdsa_recover(sig, &msg_hash.0)
        .map(|pubkey| EthAddress::from(H256::from_slice(&keccak_256(&pubkey))))
        .ok()
}

pub struct SidechainRandomness<T, N>(sp_std::marker::PhantomData<(T, N)>);

impl<T: Config, N: Get<SubNetworkId>> Randomness<sp_core::H256, BlockNumberFor<T>>
    for SidechainRandomness<T, N>
{
    fn random(subject: &[u8]) -> (sp_core::H256, BlockNumberFor<T>) {
        let (seed, block) = Self::random_seed();
        (
            sp_runtime::traits::Keccak256::hash_of(&(subject, seed)),
            block,
        )
    }

    fn random_seed() -> (sp_core::H256, BlockNumberFor<T>) {
        let network_id = N::get();
        LatestRandomSeed::<T>::get(network_id)
    }
}

#[allow(clippy::large_enum_variant)]
#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use bridge_types::SubNetworkId;
    use frame_support::dispatch::DispatchResultWithPostInfo;
    use frame_support::pallet_prelude::OptionQuery;
    use frame_support::traits::BuildGenesisConfig;
    use frame_support::{fail, Twox64Concat};

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        type Randomness: frame_support::traits::Randomness<Self::Hash, BlockNumberFor<Self>>;
    }

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    // The pallet's runtime storage items.
    #[pallet::storage]
    #[pallet::getter(fn latest_mmr_roots)]
    pub type LatestMMRRoots<T> =
        StorageMap<_, Twox64Concat, SubNetworkId, VecDeque<H256>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn latest_beefy_block)]
    pub type LatestBeefyBlock<T> = StorageMap<_, Twox64Concat, SubNetworkId, u64, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn latest_random_seed)]
    pub type LatestRandomSeed<T> =
        StorageMap<_, Twox64Concat, SubNetworkId, (H256, BlockNumberFor<T>), ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn current_validator_set)]
    pub type CurrentValidatorSet<T> =
        StorageMap<_, Twox64Concat, SubNetworkId, ValidatorSet, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn next_validator_set)]
    pub type NextValidatorSet<T> =
        StorageMap<_, Twox64Concat, SubNetworkId, ValidatorSet, OptionQuery>;

    #[pallet::type_value]
    pub fn DefaultForThisNetworkId() -> SubNetworkId {
        SubNetworkId::Mainnet
    }

    #[pallet::storage]
    #[pallet::getter(fn this_network_id)]
    pub type ThisNetworkId<T> = StorageValue<_, SubNetworkId, ValueQuery, DefaultForThisNetworkId>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        VerificationSuccessful(SubNetworkId, T::AccountId, u32),
        NewMMRRoot(SubNetworkId, H256, u64),
        ValidatorRegistryUpdated(SubNetworkId, H256, u32, u64),
    }

    #[pallet::error]
    pub enum Error<T> {
        InvalidValidatorSetId,
        InvalidMMRProof,
        PayloadBlocknumberTooOld,
        PayloadBlocknumberTooNew,
        CannotSwitchOldValidatorSet,
        NotEnoughValidatorSignatures,
        InvalidNumberOfSignatures,
        InvalidNumberOfPositions,
        InvalidNumberOfPublicKeys,
        ValidatorNotOnceInbitfield,
        ValidatorSetIncorrectPosition,
        InvalidSignature,
        MerklePositionTooHigh,
        MerkleProofTooShort,
        MerkleProofTooHigh,
        PalletNotInitialized,
        InvalidDigestHash,
        CommitmentNotFoundInDigest,
        MMRPayloadNotFound,
        InvalidNetworkId,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    // Dispatchable functions allows users to interact with the pallet and invoke state changes.
    // These functions materialize as "extrinsics", which are often compared to transactions.
    // Dispatchable functions must be annotated with a weight and must return a DispatchResult.
    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(0, 0))]
        pub fn initialize(
            origin: OriginFor<T>,
            network_id: SubNetworkId,
            latest_beefy_block: u64,
            validator_set: ValidatorSet,
            next_validator_set: ValidatorSet,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            LatestBeefyBlock::<T>::set(network_id, latest_beefy_block);
            CurrentValidatorSet::<T>::set(network_id, Some(validator_set));
            NextValidatorSet::<T>::set(network_id, Some(next_validator_set));
            Ok(().into())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(0, 0))]
        #[frame_support::transactional]
        pub fn submit_signature_commitment(
            origin: OriginFor<T>,
            network_id: SubNetworkId,
            commitment: Commitment,
            validator_proof: ValidatorProof,
            latest_mmr_leaf: BeefyMMRLeaf,
            proof: Proof<H256>,
        ) -> DispatchResultWithPostInfo {
            let signer = ensure_signed(origin)?;
            log::debug!(
                "BeefyLightClient: submit_signature_commitment: {:?}",
                commitment
            );
            log::debug!(
                "BeefyLightClient: submit_signature_commitment validator proof: {:?}",
                validator_proof
            );
            log::debug!(
                "BeefyLightClient: submit_signature_commitment latest_mmr_leaf: {:?}",
                latest_mmr_leaf
            );
            log::debug!(
                "BeefyLightClient: submit_signature_commitment proof: {:?}",
                proof
            );
            let current_validator_set = match Self::current_validator_set(network_id) {
                None => fail!(Error::<T>::PalletNotInitialized),
                Some(x) => x,
            };
            let next_validator_set = match Self::next_validator_set(network_id) {
                None => fail!(Error::<T>::PalletNotInitialized),
                Some(x) => x,
            };
            let vset = match (commitment.validator_set_id) == current_validator_set.id {
                true => current_validator_set,
                false => match (commitment.validator_set_id) == next_validator_set.id {
                    true => next_validator_set,
                    false => fail!(Error::<T>::InvalidValidatorSetId),
                },
            };
            Self::verify_commitment(network_id, &commitment, &validator_proof, vset)?;
            let payload = commitment
                .payload
                .get_decoded::<H256>(&sp_consensus_beefy::known_payloads::MMR_ROOT_ID)
                .ok_or(Error::<T>::MMRPayloadNotFound)?;
            Self::verify_newest_mmr_leaf(&latest_mmr_leaf, &payload, &proof)?;
            Self::process_payload(network_id, payload, commitment.block_number.into())?;

            let block_number = <frame_system::Pallet<T>>::block_number();
            LatestRandomSeed::<T>::set(
                network_id,
                (latest_mmr_leaf.leaf_extra.random_seed, block_number),
            );

            Self::deposit_event(Event::VerificationSuccessful(
                network_id,
                signer,
                commitment.block_number,
            ));
            Self::apply_validator_set_changes(
                network_id,
                latest_mmr_leaf.beefy_next_authority_set,
            )?;
            Ok(().into())
        }
    }

    #[pallet::genesis_config]
    pub struct GenesisConfig<T> {
        /// Network id for current network
        pub network_id: SubNetworkId,
        phantom: PhantomData<T>,
    }

    impl<T> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                network_id: SubNetworkId::Mainnet,
                phantom: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            ThisNetworkId::<T>::put(self.network_id);
        }
    }
}

impl<T: Config> bridge_types::traits::Verifier for Pallet<T> {
    type Proof = SubstrateBridgeMessageProof;
    fn verify(
        network_id: GenericNetworkId,
        commitment_hash: H256,
        proof: &SubstrateBridgeMessageProof,
    ) -> DispatchResult {
        let network_id = network_id.sub().ok_or(Error::<T>::InvalidNetworkId)?;
        let this_network_id = ThisNetworkId::<T>::get();
        Self::verify_mmr_leaf(network_id, &proof.leaf, &proof.proof)?;
        let digest_hash = proof.digest.using_encoded(keccak_256);
        ensure!(
            digest_hash == proof.leaf.leaf_extra.digest_hash.0,
            Error::<T>::InvalidMMRProof
        );
        let count = proof
            .digest
            .logs
            .iter()
            .filter(|x| {
                let AuxiliaryDigestItem::Commitment(log_network_id, log_commitment_hash) = x;
                if let GenericNetworkId::Sub(log_network_id) = log_network_id {
                    return *log_network_id == this_network_id
                        && commitment_hash == *log_commitment_hash;
                }
                false
            })
            .count();
        ensure!(count == 1, Error::<T>::CommitmentNotFoundInDigest);

        Ok(())
    }

    fn verify_weight(_proof: &Self::Proof) -> Weight {
        Default::default()
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn valid_proof() -> Option<Self::Proof> {
        None
    }
}

impl<T: Config> Pallet<T> {
    pub fn add_known_mmr_root(network_id: SubNetworkId, root: H256) {
        let mut mmr_roots = LatestMMRRoots::<T>::get(network_id);
        // Add new root to the front of the list to check it first
        mmr_roots.push_front(root);
        if mmr_roots.len() > MMR_ROOT_HISTORY_SIZE {
            mmr_roots.pop_back();
        }
        LatestMMRRoots::<T>::insert(network_id, mmr_roots);
    }

    pub fn is_known_root(network_id: SubNetworkId, root: H256) -> bool {
        let mmr_roots = LatestMMRRoots::<T>::get(network_id);
        mmr_roots.contains(&root)
    }

    #[inline]
    pub fn get_latest_mmr_root(network_id: SubNetworkId) -> Option<H256> {
        LatestMMRRoots::<T>::get(network_id).back().cloned()
    }

    #[inline]
    pub fn verify_beefy_merkle_leaf(
        network_id: SubNetworkId,
        beefy_mmr_leaf: H256,
        proof: &Proof<H256>,
    ) -> bool {
        let proof_root = proof.root(hasher, beefy_mmr_leaf);
        Self::is_known_root(network_id, proof_root)
    }

    #[inline]
    pub fn create_random_bit_field(
        network_id: SubNetworkId,
        validator_claims_bitfield: BitField,
        number_of_validators: u32,
    ) -> Result<BitField, Error<T>> {
        Self::random_n_bits_with_prior_check(
            network_id,
            &validator_claims_bitfield,
            Self::get_required_number_of_signatures(number_of_validators),
            number_of_validators,
        )
    }

    #[inline]
    pub fn create_initial_bitfield(bits_to_set: &[u32], length: usize) -> BitField {
        BitField::create_bitfield(bits_to_set, length)
    }

    #[inline]
    pub fn required_number_of_signatures(vset: &ValidatorSet) -> u32 {
        Self::get_required_number_of_signatures(vset.len)
    }

    /* Private Functions */

    fn verify_newest_mmr_leaf(
        leaf: &BeefyMMRLeaf,
        root: &H256,
        proof: &Proof<H256>,
    ) -> DispatchResultWithPostInfo {
        let hash_leaf = Keccak256::hash_of(&leaf);
        ensure!(
            verify_inclusion_proof(*root, hash_leaf, proof),
            Error::<T>::InvalidMMRProof
        );
        Ok(().into())
    }

    fn verify_mmr_leaf(
        network_id: SubNetworkId,
        leaf: &BeefyMMRLeaf,
        proof: &Proof<H256>,
    ) -> DispatchResult {
        let hash_leaf = Keccak256::hash_of(&leaf);
        let root = proof.root(hasher, hash_leaf);
        ensure!(
            Self::is_known_root(network_id, root),
            Error::<T>::InvalidMMRProof
        );
        Ok(())
    }

    fn process_payload(
        network_id: SubNetworkId,
        payload: H256,
        block_number: u64,
    ) -> DispatchResultWithPostInfo {
        ensure!(
            block_number > Self::latest_beefy_block(network_id,),
            Error::<T>::PayloadBlocknumberTooOld
        );
        Self::add_known_mmr_root(network_id, payload);
        LatestBeefyBlock::<T>::set(network_id, block_number);
        Self::deposit_event(Event::NewMMRRoot(network_id, payload, block_number));
        Ok(().into())
    }

    fn apply_validator_set_changes(
        network_id: SubNetworkId,
        new_vset: ValidatorSet,
    ) -> DispatchResultWithPostInfo {
        let next_validator_set = match Self::next_validator_set(network_id) {
            None => fail!(Error::<T>::PalletNotInitialized),
            Some(x) => x,
        };
        if new_vset.id > next_validator_set.id {
            CurrentValidatorSet::<T>::set(network_id, Some(next_validator_set));
            NextValidatorSet::<T>::set(network_id, Some(new_vset));
        }
        Ok(().into())
    }

    fn get_required_number_of_signatures(num_validators: u32) -> u32 {
        (num_validators * THRESHOLD_NUMERATOR + THRESHOLD_DENOMINATOR - 1) / THRESHOLD_DENOMINATOR
    }

    /*
     * @dev https://github.com/sora-xor/substrate/blob/7d914ce3ed34a27d7bb213caed374d64cde8cfa8/client/beefy/src/round.rs#L62
     */
    fn check_commitment_signatures_threshold(
        num_of_validators: u32,
        validator_claims_bitfield: &BitField,
    ) -> DispatchResultWithPostInfo {
        let threshold = num_of_validators - (num_of_validators - 1) / 3;
        let count = validator_claims_bitfield.count_set_bits() as u32;
        ensure!(count >= threshold, Error::<T>::NotEnoughValidatorSignatures);
        Ok(().into())
    }

    fn verify_commitment(
        network_id: SubNetworkId,
        commitment: &Commitment,
        proof: &ValidatorProof,
        vset: ValidatorSet,
    ) -> DispatchResultWithPostInfo {
        let number_of_validators = vset.len;
        let required_num_of_signatures =
            Self::get_required_number_of_signatures(number_of_validators);
        Self::check_commitment_signatures_threshold(
            number_of_validators,
            &proof.validator_claims_bitfield,
        )?;
        let random_bitfield = Self::random_n_bits_with_prior_check(
            network_id,
            &proof.validator_claims_bitfield,
            required_num_of_signatures,
            number_of_validators,
        )?;
        log::debug!("BeefyLightClient verify_commitment proof: {:?}", proof);
        log::debug!(
            "BeefyLightClient verify_commitment validator_claims_bitfield: {:?}",
            proof.validator_claims_bitfield
        );
        log::debug!(
            "BeefyLightClient verify_commitment random_bitfield: {:?}",
            random_bitfield
        );
        Self::verify_validator_proof_lengths(required_num_of_signatures, proof)?;
        let commitment_hash = Keccak256::hash_of(&commitment);
        Self::verify_validator_proof_signatures(
            &vset,
            random_bitfield,
            proof,
            required_num_of_signatures,
            commitment_hash,
        )?;
        Ok(().into())
    }

    fn verify_validator_proof_lengths(
        required_num_of_signatures: u32,
        proof: &ValidatorProof,
    ) -> DispatchResultWithPostInfo {
        ensure!(
            proof.signatures.len() as u32 == required_num_of_signatures,
            Error::<T>::InvalidNumberOfSignatures
        );
        ensure!(
            proof.positions.len() as u32 == required_num_of_signatures,
            Error::<T>::InvalidNumberOfPositions
        );
        ensure!(
            proof.public_keys.len() as u32 == required_num_of_signatures,
            Error::<T>::InvalidNumberOfPublicKeys
        );
        ensure!(
            proof.public_key_merkle_proofs.len() as u32 == required_num_of_signatures,
            Error::<T>::InvalidNumberOfPublicKeys
        );
        Ok(().into())
    }

    fn verify_validator_proof_signatures(
        vset: &ValidatorSet,
        mut random_bitfield: BitField,
        proof: &ValidatorProof,
        required_num_of_signatures: u32,
        commitment_hash: H256,
    ) -> DispatchResultWithPostInfo {
        let required_num_of_signatures = required_num_of_signatures as usize;
        for i in 0..required_num_of_signatures {
            Self::verify_validator_signature(
                vset,
                &mut random_bitfield,
                proof.signatures[i].clone(),
                proof.positions[i],
                proof.public_keys[i],
                &proof.public_key_merkle_proofs[i],
                commitment_hash,
            )?;
        }
        Ok(().into())
    }

    fn verify_validator_signature(
        vset: &ValidatorSet,
        random_bitfield: &mut BitField,
        signature: Vec<u8>,
        position: u128,
        public_key: EthAddress,
        public_key_merkle_proof: &[H256],
        commitment_hash: H256,
    ) -> DispatchResultWithPostInfo {
        ensure!(
            random_bitfield.is_set(position as usize),
            Error::<T>::ValidatorNotOnceInbitfield
        );
        random_bitfield.clear(position as usize);
        Self::check_validator_in_set(vset, public_key, position, public_key_merkle_proof)?;
        ensure!(signature.len() == 65, Error::<T>::InvalidSignature);
        let signature: [u8; 65] = match signature.try_into() {
            Ok(v) => v,
            Err(_) => fail!(Error::<T>::InvalidSignature),
        };
        let addr = match recover_signature(&signature, &commitment_hash) {
            Some(v) => v,
            None => fail!(Error::<T>::InvalidSignature),
        };
        ensure!(addr == public_key, Error::<T>::InvalidSignature);
        Ok(().into())
    }

    fn check_validator_in_set(
        vset: &ValidatorSet,
        addr: EthAddress,
        pos: u128,
        proof: &[H256],
    ) -> DispatchResultWithPostInfo {
        let current_validator_set_len = vset.len;
        let pos: u32 = pos
            .try_into()
            .map_err(|_| Error::<T>::MerklePositionTooHigh)?;
        ensure!(
            binary_merkle_tree::verify_proof::<sp_runtime::traits::Keccak256, _, _>(
                &vset.keyset_commitment,
                proof.iter().cloned(),
                current_validator_set_len,
                pos,
                &addr
            ),
            Error::<T>::ValidatorSetIncorrectPosition
        );
        Ok(().into())
    }

    pub fn random_n_bits_with_prior_check(
        network_id: SubNetworkId,
        prior: &BitField,
        n: u32,
        length: u32,
    ) -> Result<BitField, Error<T>> {
        let (raw_seed, _block) = T::Randomness::random(RANDOMNESS_SUBJECT);
        let latest_beefy_block = Self::latest_beefy_block(network_id);
        let seed = codec::Encode::using_encoded(
            &(raw_seed, latest_beefy_block),
            sp_io::hashing::blake2_128,
        );
        Ok(BitField::create_random_bitfield(
            prior,
            n,
            length,
            u128::from_be_bytes(seed),
        ))
    }
}
