//! # Ethereum Light Client Verifier
//!
//! The verifier module verifies `Message` objects by verifying the existence
//! of their corresponding Ethereum log in a block in the Ethereum PoW network.
//! More specifically, the module checks a Merkle proof to confirm the existence
//! of a receipt, and the given log within the receipt, in a given block.
//!
//! This module relies on the relayer service which submits `import_header`
//! extrinsics, in order, as new blocks in the Ethereum network are authored.
//! It stores the most recent `FINALIZED_HEADERS_TO_KEEP` + `DescendantsUntilFinalized`
//! headers and prunes older headers. This means verification will only succeed
//! for messages from *finalized* blocks no older than `FINALIZED_HEADERS_TO_KEEP`.
//!
//! ## Usage
//!
//! This module implements the `Verifier` interface. Other modules should reference
//! this module using the `Verifier` type and perform verification using `Verifier::verify`.
//!
#![allow(unused_variables)]
#![cfg_attr(not(feature = "std"), no_std)]

mod weights;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

use codec::{Decode, Encode};
use frame_support::dispatch::{DispatchError, DispatchResult};
use frame_support::log;
use frame_support::traits::Get;
use frame_system::ensure_signed;
use sp_runtime::RuntimeDebug;
use sp_std::prelude::*;

use bridge_types::difficulty::calc_difficulty;
pub use bridge_types::difficulty::DifficultyConfig as EthereumDifficultyConfig;
use bridge_types::ethashproof::{DoubleNodeWithMerkleProof as EthashProofData, EthashProver};
use bridge_types::traits::Verifier;
use bridge_types::types::{Message, Proof};
pub use bridge_types::Header as EthereumHeader;
use bridge_types::{EthNetworkId, HeaderId as EthereumHeaderId, Log, Receipt, H256, U256};

pub use weights::WeightInfo;

/// Max number of finalized headers to keep.
const FINALIZED_HEADERS_TO_KEEP: u64 = 50_000;
/// Max number of headers we're pruning in single import call.
const HEADERS_TO_PRUNE_IN_SINGLE_IMPORT: u64 = 8;

/// Ethereum block header as it is stored in the runtime storage.
#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, scale_info::TypeInfo)]
pub struct StoredHeader<Submitter> {
    /// Submitter of this header. This will be None for the initial header
    /// or the account ID of the relay.
    pub submitter: Option<Submitter>,
    /// The block header itself.
    pub header: EthereumHeader,
    /// Total difficulty of the chain.
    pub total_difficulty: U256,
    /// Indicates if the header is part of the canonical chain, i.e. has
    /// at least DescendantsUntilFinalized descendants.
    pub finalized: bool,
}

/// Blocks range that we want to prune.
#[derive(Clone, Encode, Decode, Default, PartialEq, RuntimeDebug, scale_info::TypeInfo)]
struct PruningRange {
    /// Number of the oldest unpruned block(s). This might be the block that we do not
    /// want to prune now (then it is equal to `oldest_block_to_keep`).
    pub oldest_unpruned_block: u64,
    /// Number of oldest block(s) that we want to keep. We want to prune blocks in range
    /// [`oldest_unpruned_block`; `oldest_block_to_keep`).
    pub oldest_block_to_keep: u64,
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {

    use super::*;

    use frame_support::pallet_prelude::*;
    use frame_support::traits::StorageVersion;
    use frame_system::pallet_prelude::*;

    /// The current storage version.
    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        /// The number of descendants, in the highest difficulty chain, a block
        /// needs to have in order to be considered final.
        #[pallet::constant]
        type DescendantsUntilFinalized: Get<u8>;
        /// Ethereum network parameters for header difficulty
        #[pallet::constant]
        type DifficultyConfig: Get<EthereumDifficultyConfig>;
        /// Determines whether Ethash PoW is verified for headers
        /// NOTE: Should only be false for dev
        #[pallet::constant]
        type VerifyPoW: Get<bool>;
        /// Weight information for extrinsics in this pallet
        type WeightInfo: WeightInfo;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T> {
        Finalized(EthNetworkId, EthereumHeaderId),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Header is same height or older than finalized block (we don't support forks).
        AncientHeader,
        /// Header referenced in inclusion proof doesn't exist, e.g. because it's
        /// pruned or older than genesis.
        MissingHeader,
        /// Header's parent has not been imported.
        MissingParentHeader,
        /// Header has already been imported.
        DuplicateHeader,
        /// Header referenced in inclusion proof is not final yet.
        HeaderNotFinalized,
        /// Header is on a stale fork, i.e. it's not a descendant of the latest finalized block
        HeaderOnStaleFork,
        /// One or more header fields are invalid.
        InvalidHeader,
        /// Proof could not be applied / verified.
        InvalidProof,
        /// Log could not be decoded
        DecodeFailed,
        /// Unknown network id passed
        NetworkNotFound,
        /// Network with given id already registered
        NetworkAlreadyExists,
        /// This should never be returned - indicates a bug
        Unknown,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    /// Best known block.
    #[pallet::storage]
    pub(super) type BestBlock<T: Config> =
        StorageMap<_, Identity, EthNetworkId, (EthereumHeaderId, U256), OptionQuery>;

    /// Range of blocks that we want to prune.
    #[pallet::storage]
    pub(super) type BlocksToPrune<T: Config> =
        StorageMap<_, Identity, EthNetworkId, PruningRange, OptionQuery>;

    /// Best finalized block.
    #[pallet::storage]
    pub(super) type FinalizedBlock<T: Config> =
        StorageMap<_, Identity, EthNetworkId, EthereumHeaderId, OptionQuery>;

    /// Map of imported headers by hash.
    #[pallet::storage]
    pub(super) type Headers<T: Config> = StorageDoubleMap<
        _,
        Identity,
        EthNetworkId,
        Identity,
        H256,
        StoredHeader<T::AccountId>,
        OptionQuery,
    >;

    /// Map of imported header hashes by number.
    #[pallet::storage]
    pub(super) type HeadersByNumber<T: Config> =
        StorageDoubleMap<_, Identity, EthNetworkId, Twox64Concat, u64, Vec<H256>, OptionQuery>;

    #[pallet::genesis_config]
    pub struct GenesisConfig {
        pub initial_networks: Vec<(EthNetworkId, EthereumHeader, U256)>,
    }

    #[cfg(feature = "std")]
    impl Default for GenesisConfig {
        fn default() -> Self {
            Self {
                initial_networks: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig {
        fn build(&self) {
            for (network_id, header, difficulty) in &self.initial_networks {
                Pallet::<T>::initialize_storage_inner(
                    *network_id,
                    vec![header.clone()],
                    difficulty.clone(),
                    0, // descendants_until_final = 0 forces the initial header to be finalized
                )
                .unwrap();

                <BlocksToPrune<T>>::insert(
                    network_id,
                    PruningRange {
                        oldest_unpruned_block: header.number,
                        oldest_block_to_keep: header.number,
                    },
                );
            }
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(T::WeightInfo::register_network())]
        pub fn register_network(
            origin: OriginFor<T>,
            network_id: EthNetworkId,
            header: EthereumHeader,
            initial_difficulty: U256,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(
                <BestBlock<T>>::contains_key(network_id) == false,
                Error::<T>::NetworkAlreadyExists
            );
            Pallet::<T>::initialize_storage_inner(
                network_id,
                vec![header.clone()],
                initial_difficulty,
                0,
            )
            // should never fail with single header
            .map_err(|_| Error::<T>::Unknown)?;

            <BlocksToPrune<T>>::insert(
                network_id,
                PruningRange {
                    oldest_unpruned_block: header.number,
                    oldest_block_to_keep: header.number,
                },
            );
            Ok(())
        }
        /// Import a single Ethereum PoW header.
        ///
        /// Note that this extrinsic has a very high weight. The weight is affected by the
        /// value of `DescendantsUntilFinalized`. Regenerate weights if it changes.
        ///
        /// The largest contributors to the worst case weight, in decreasing order, are:
        /// - Pruning: max 2 writes per pruned header + 2 writes to finalize pruning state.
        ///   Up to `HEADERS_TO_PRUNE_IN_SINGLE_IMPORT` can be pruned in one call.
        /// - Ethash validation: this cost is pure CPU. EthashProver checks a merkle proof
        ///   for each DAG node selected in the "hashimoto"-loop.
        /// - Iterating over ancestors: min `DescendantsUntilFinalized` reads to find the
        ///   newly finalized ancestor of a header.
        #[pallet::weight(T::WeightInfo::import_header())]
        pub fn import_header(
            origin: OriginFor<T>,
            network_id: EthNetworkId,
            header: EthereumHeader,
            proof: Vec<EthashProofData>,
        ) -> DispatchResult {
            let sender = ensure_signed(origin)?;

            log::trace!(
                target: "ethereum-light-client",
                "Received header {}. Starting validation",
                header.number,
            );

            if let Err(err) = Self::validate_header_to_import(network_id, &header, &proof) {
                log::trace!(
                    target: "ethereum-light-client",
                    "Validation for header {} returned error. Skipping import",
                    header.number,
                );
                return Err(err);
            }

            log::trace!(
                target: "ethereum-light-client",
                "Validation succeeded. Starting import of header {}",
                header.number,
            );

            if let Err(err) = Self::import_validated_header(network_id, &sender, &header) {
                log::trace!(
                    target: "ethereum-light-client",
                    "Import of header {} failed",
                    header.number,
                );
                return Err(err);
            }

            log::trace!(
                target: "ethereum-light-client",
                "Import of header {} succeeded!",
                header.number,
            );

            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        // Validate an Ethereum header for import
        fn validate_header_to_import(
            network_id: EthNetworkId,
            header: &EthereumHeader,
            proof: &[EthashProofData],
        ) -> DispatchResult {
            let hash = header.compute_hash();
            ensure!(
                !<Headers<T>>::contains_key(network_id, hash),
                Error::<T>::DuplicateHeader,
            );

            let finalized_header_id =
                <FinalizedBlock<T>>::get(network_id).ok_or(Error::<T>::NetworkNotFound)?;

            let parent = <Headers<T>>::get(network_id, header.parent_hash)
                .ok_or(Error::<T>::MissingParentHeader)?
                .header;

            ensure!(
                header.number > finalized_header_id.number,
                Error::<T>::AncientHeader,
            );

            // This iterates over DescendantsUntilFinalized headers in both the worst and
            // average case. Since we know that the parent header was imported successfully,
            // we know that the newest finalized header is at most, and on average,
            // DescendantsUntilFinalized headers before the parent.
            let ancestor_at_finalized_number =
                ancestry::<T>(network_id.clone(), header.parent_hash)
                    .find(|(_, ancestor)| ancestor.number == finalized_header_id.number);
            // We must find a matching ancestor above since AncientHeader check ensures
            // that iteration starts at or after the latest finalized block.
            ensure!(ancestor_at_finalized_number.is_some(), Error::<T>::Unknown,);
            ensure!(
                ancestor_at_finalized_number.unwrap().0 == finalized_header_id.hash,
                Error::<T>::HeaderOnStaleFork,
            );

            if !T::VerifyPoW::get() {
                return Ok(());
            }

            // See YellowPaper formula (50) in section 4.3.4
            ensure!(
                header.gas_used <= header.gas_limit
                    && header.gas_limit < parent.gas_limit * 1025 / 1024
                    && header.gas_limit > parent.gas_limit * 1023 / 1024
                    && header.gas_limit >= 5000u64.into()
                    && header.timestamp > parent.timestamp
                    && header.number == parent.number + 1
                    && header.extra_data.len() <= 32,
                Error::<T>::InvalidHeader,
            );

            log::trace!(
                target: "ethereum-light-client",
                "Header {} passed basic verification",
                header.number
            );

            let difficulty_config = T::DifficultyConfig::get();
            let header_difficulty = calc_difficulty(&difficulty_config, header.timestamp, &parent)
                .map_err(|_| Error::<T>::InvalidHeader)?;
            ensure!(
                header.difficulty == header_difficulty,
                Error::<T>::InvalidHeader,
            );

            log::trace!(
                target: "ethereum-light-client",
                "Header {} passed difficulty verification",
                header.number
            );

            let header_mix_hash = header.mix_hash().ok_or(Error::<T>::InvalidHeader)?;
            let header_nonce = header.nonce().ok_or(Error::<T>::InvalidHeader)?;
            let (mix_hash, result) = EthashProver::new()
                .hashimoto_merkle(
                    header.compute_partial_hash(),
                    header_nonce,
                    header.number,
                    proof,
                )
                .map_err(|_| Error::<T>::InvalidHeader)?;

            log::trace!(
                target: "ethereum-light-client",
                "Header {} passed PoW verification",
                header.number
            );
            ensure!(
                mix_hash == header_mix_hash
                    && U256::from(result.0) < ethash::cross_boundary(header.difficulty),
                Error::<T>::InvalidHeader,
            );

            Ok(())
        }

        // Import a new, validated Ethereum header
        fn import_validated_header(
            network_id: EthNetworkId,
            sender: &T::AccountId,
            header: &EthereumHeader,
        ) -> DispatchResult {
            let hash = header.compute_hash();
            let stored_parent_header = <Headers<T>>::get(network_id, header.parent_hash)
                .ok_or(Error::<T>::MissingParentHeader)?;
            let total_difficulty = stored_parent_header
                .total_difficulty
                .checked_add(header.difficulty)
                .ok_or("Total difficulty overflow")?;
            let header_to_store = StoredHeader {
                submitter: Some(sender.clone()),
                header: header.clone(),
                total_difficulty,
                finalized: false,
            };

            <Headers<T>>::insert(network_id, hash, header_to_store);

            if <HeadersByNumber<T>>::contains_key(network_id, header.number) {
                <HeadersByNumber<T>>::mutate(
                    network_id,
                    header.number,
                    |option| -> DispatchResult {
                        if let Some(hashes) = option {
                            hashes.push(hash);
                            return Ok(());
                        }
                        Err(Error::<T>::Unknown.into())
                    },
                )?;
            } else {
                <HeadersByNumber<T>>::insert(network_id, header.number, vec![hash]);
            }

            // Maybe track new highest difficulty chain
            let (_, highest_difficulty) =
                <BestBlock<T>>::get(network_id).ok_or(Error::<T>::NetworkNotFound)?;
            if total_difficulty > highest_difficulty
                || (!T::VerifyPoW::get() && total_difficulty == U256::zero())
            {
                let best_block_id = EthereumHeaderId {
                    number: header.number,
                    hash,
                };
                <BestBlock<T>>::insert(network_id, (best_block_id, total_difficulty));

                // Finalize blocks if possible
                let finalized_block_id =
                    <FinalizedBlock<T>>::get(network_id).ok_or(Error::<T>::NetworkNotFound)?;
                let new_finalized_block_id = Self::get_best_finalized_header(
                    network_id,
                    &best_block_id,
                    &finalized_block_id,
                )?;
                if new_finalized_block_id != finalized_block_id {
                    <FinalizedBlock<T>>::insert(network_id, new_finalized_block_id);
                    Self::deposit_event(Event::Finalized(network_id, new_finalized_block_id));
                    <Headers<T>>::mutate(
                        network_id,
                        new_finalized_block_id.hash,
                        |option| -> DispatchResult {
                            if let Some(header) = option {
                                header.finalized = true;
                                return Ok(());
                            }
                            Err(Error::<T>::Unknown.into())
                        },
                    )?;
                }

                // Clean up old headers
                let pruning_range =
                    <BlocksToPrune<T>>::get(network_id).ok_or(Error::<T>::NetworkNotFound)?;
                let new_pruning_range = Self::prune_header_range(
                    network_id,
                    &pruning_range,
                    HEADERS_TO_PRUNE_IN_SINGLE_IMPORT,
                    new_finalized_block_id
                        .number
                        .saturating_sub(FINALIZED_HEADERS_TO_KEEP),
                );
                if new_pruning_range != pruning_range {
                    <BlocksToPrune<T>>::insert(network_id, new_pruning_range);
                }
            }

            Ok(())
        }

        // Return the latest block that can be finalized based on the given
        // highest difficulty chain and previously finalized block.
        fn get_best_finalized_header(
            network_id: EthNetworkId,
            best_block_id: &EthereumHeaderId,
            finalized_block_id: &EthereumHeaderId,
        ) -> Result<EthereumHeaderId, DispatchError> {
            let required_descendants = T::DescendantsUntilFinalized::get() as usize;
            let maybe_newly_finalized_ancestor =
                ancestry::<T>(network_id.clone(), best_block_id.hash)
                    .enumerate()
                    .find_map(|(i, pair)| {
                        if i < required_descendants {
                            None
                        } else {
                            Some(pair)
                        }
                    });

            match maybe_newly_finalized_ancestor {
                Some((hash, header)) => {
                    // The header is newly finalized if it is younger than the current
                    // finalized block
                    if header.number > finalized_block_id.number {
                        return Ok(EthereumHeaderId {
                            hash: hash,
                            number: header.number,
                        });
                    }
                    if hash != finalized_block_id.hash {
                        return Err(Error::<T>::Unknown.into());
                    }
                    Ok(finalized_block_id.clone())
                }
                None => Ok(finalized_block_id.clone()),
            }
        }

        // Remove old headers, from oldest to newest, in the provided range
        // (adjusted to `prune_end` if newer). Only up to `max_headers_to_prune`
        // will be removed.
        pub(super) fn prune_header_range(
            network_id: EthNetworkId,
            pruning_range: &PruningRange,
            max_headers_to_prune: u64,
            prune_end: u64,
        ) -> PruningRange {
            let mut new_pruning_range = pruning_range.clone();

            // We can only increase this since pruning cannot be reverted...
            if prune_end > new_pruning_range.oldest_block_to_keep {
                new_pruning_range.oldest_block_to_keep = prune_end;
            }

            let start = new_pruning_range.oldest_unpruned_block;
            let end = new_pruning_range.oldest_block_to_keep;
            let mut blocks_pruned = 0;
            for number in start..end {
                if blocks_pruned == max_headers_to_prune {
                    break;
                }

                if let Some(hashes_at_number) = <HeadersByNumber<T>>::take(network_id, number) {
                    let mut remaining = hashes_at_number.len();
                    for hash in hashes_at_number.iter() {
                        <Headers<T>>::remove(network_id, hash);
                        blocks_pruned += 1;
                        remaining -= 1;
                        if blocks_pruned == max_headers_to_prune {
                            break;
                        }
                    }

                    if remaining > 0 {
                        let remainder = &hashes_at_number[hashes_at_number.len() - remaining..];
                        <HeadersByNumber<T>>::insert(network_id, number, remainder);
                    } else {
                        new_pruning_range.oldest_unpruned_block = number + 1;
                    }
                } else {
                    new_pruning_range.oldest_unpruned_block = number + 1;
                }
            }

            new_pruning_range
        }

        // Verifies that the receipt encoded in proof.data is included
        // in the block given by proof.block_hash. Inclusion is only
        // recognized if the block has been finalized.
        fn verify_receipt_inclusion(
            network_id: EthNetworkId,
            proof: &Proof,
        ) -> Result<Receipt, DispatchError> {
            let stored_header =
                <Headers<T>>::get(network_id, proof.block_hash).ok_or(Error::<T>::MissingHeader)?;

            ensure!(stored_header.finalized, Error::<T>::HeaderNotFinalized);

            let result = stored_header
                .header
                .check_receipt_proof(&proof.data)
                .ok_or(Error::<T>::InvalidProof)?;

            match result {
                Ok(receipt) => Ok(receipt),
                Err(err) => {
                    log::trace!(
                        target: "ethereum-light-client",
                        "Failed to decode transaction receipt: {}",
                        err
                    );
                    Err(Error::<T>::InvalidProof.into())
                }
            }
        }

        /// Import an ordered vec of Ethereum headers without performing
        /// validation.
        ///
        /// NOTE: This should only be used to initialize empty storage.
        pub(crate) fn initialize_storage_inner(
            network_id: EthNetworkId,
            headers: Vec<EthereumHeader>,
            initial_difficulty: U256,
            descendants_until_final: u8,
        ) -> Result<(), &'static str> {
            let insert_header_fn = |header: &EthereumHeader, total_difficulty: U256| {
                let hash = header.compute_hash();
                <Headers<T>>::insert(
                    network_id,
                    hash,
                    StoredHeader {
                        submitter: None,
                        header: header.clone(),
                        total_difficulty: total_difficulty,
                        finalized: false,
                    },
                );
                <HeadersByNumber<T>>::append(network_id, header.number, hash);

                EthereumHeaderId {
                    number: header.number,
                    hash: hash,
                }
            };

            let oldest_header = headers.get(0).ok_or("Need at least one header")?;
            let mut best_block_difficulty = initial_difficulty;
            let mut best_block_id = insert_header_fn(&oldest_header, best_block_difficulty);

            for (i, header) in headers.iter().enumerate().skip(1) {
                let prev_block_num = headers[i - 1].number;
                ensure!(
                    header.number == prev_block_num || header.number == prev_block_num + 1,
                    "Headers must be in order",
                );

                let total_difficulty = {
                    let parent = <Headers<T>>::get(network_id, header.parent_hash)
                        .ok_or("Missing parent header")?;
                    parent.total_difficulty + header.difficulty
                };

                let block_id = insert_header_fn(&header, total_difficulty);

                if total_difficulty > best_block_difficulty {
                    best_block_difficulty = total_difficulty;
                    best_block_id = block_id;
                }
            }

            <BestBlock<T>>::insert(network_id, (best_block_id, best_block_difficulty));

            let maybe_finalized_ancestor = ancestry::<T>(network_id.clone(), best_block_id.hash)
                .enumerate()
                .find_map(|(i, pair)| {
                    if i < descendants_until_final as usize {
                        None
                    } else {
                        Some(pair)
                    }
                });
            if let Some((hash, header)) = maybe_finalized_ancestor {
                <FinalizedBlock<T>>::insert(
                    network_id,
                    EthereumHeaderId {
                        hash: hash,
                        number: header.number,
                    },
                );
                let mut next_hash = Ok(hash);
                loop {
                    match next_hash {
                        Ok(hash) => {
                            next_hash = <Headers<T>>::mutate(network_id, hash, |option| {
                                if let Some(header) = option {
                                    header.finalized = true;
                                    return Ok(header.header.parent_hash);
                                }
                                Err("No header at hash")
                            })
                        }
                        _ => break,
                    }
                }
            } else {
                panic!("Network don't have finalized header");
            }

            Ok(())
        }
    }

    /// Return iterator over header ancestors, starting at given hash
    fn ancestry<T: Config>(
        network_id: EthNetworkId,
        mut hash: H256,
    ) -> impl Iterator<Item = (H256, EthereumHeader)> {
        sp_std::iter::from_fn(move || {
            let header = <Headers<T>>::get(network_id, &hash)?.header;
            let current_hash = hash;
            hash = header.parent_hash;
            Some((current_hash, header))
        })
    }

    impl<T: Config> Verifier for Pallet<T> {
        /// Verify a message by verifying the existence of the corresponding
        /// Ethereum log in a block. Returns the log if successful.
        fn verify(network_id: EthNetworkId, message: &Message) -> Result<Log, DispatchError> {
            let receipt = Self::verify_receipt_inclusion(network_id, &message.proof)?;

            log::trace!(
                target: "ethereum-light-client",
                "Verified receipt inclusion for transaction at index {} in block {}",
                message.proof.tx_index, message.proof.block_hash,
            );

            let log: Log = rlp::decode(&message.data).map_err(|_| Error::<T>::DecodeFailed)?;

            if !receipt.contains_log(&log) {
                log::trace!(
                    target: "ethereum-light-client",
                    "Event log not found in receipt for transaction at index {} in block {}",
                    message.proof.tx_index, message.proof.block_hash,
                );
                return Err(Error::<T>::InvalidProof.into());
            }

            Ok(log)
        }

        fn initialize_storage(
            network_id: EthNetworkId,
            headers: Vec<bridge_types::Header>,
            difficulty: u128,
            descendants_until_final: u8,
        ) -> Result<(), &'static str> {
            Self::initialize_storage_inner(
                network_id,
                headers,
                difficulty.into(),
                descendants_until_final,
            )
        }
    }
}
