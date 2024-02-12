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
// TODO #167: fix clippy warnings
#![allow(clippy::all)]

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
use sp_runtime::traits::{IdentifyAccount, Verify};
use sp_runtime::RuntimeDebug;
use sp_std::prelude::*;

pub use bridge_types::difficulty::ForkConfig as EthereumDifficultyConfig;
use bridge_types::ethashproof::{
    DoubleNodeWithMerkleProof as EthashProofData, EthashProver, MixNonce,
};
use bridge_types::evm::Proof;
use bridge_types::traits::{EthereumGasPriceOracle, Verifier};
pub use bridge_types::Header as EthereumHeader;
use bridge_types::{EVMChainId, HeaderId as EthereumHeaderId, Receipt, H256, U256};

pub use weights::WeightInfo;

/// Max number of finalized headers to keep.
const FINALIZED_HEADERS_TO_KEEP: u64 = 50_000;
/// Max number of headers we're pruning in single import call.
const HEADERS_TO_PRUNE_IN_SINGLE_IMPORT: u64 = 8;
/// Length of difficulties vector to store
const CHECK_DIFFICULTY_DIFFERENCE_NUMBER: u64 = 10;
/// Calculate the maximum difference between current header difficulty and maximum among stored in vector
pub(crate) const DIFFICULTY_DIFFERENCE: f64 =
    1.0 + 0.125 * (CHECK_DIFFICULTY_DIFFERENCE_NUMBER as f64);
const DIVISION_COEFFICIENT: u64 = 1000;
const DIFFICULTY_DIFFERENCE_MULT: U256 = U256([
    ((DIFFICULTY_DIFFERENCE * (DIVISION_COEFFICIENT as f64)) as u64) / DIVISION_COEFFICIENT,
    0,
    0,
    0,
]);

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

pub type Submitter<T> =
    <<<T as Config>::ImportSignature as Verify>::Signer as IdentifyAccount>::AccountId;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {

    use super::*;

    use bridge_types::network_config::{Consensus, NetworkConfig as EthNetworkConfig};
    use bridge_types::GenericNetworkId;
    use frame_support::pallet_prelude::*;
    use frame_support::traits::StorageVersion;
    use frame_system::pallet_prelude::*;
    use sp_runtime::traits::{IdentifyAccount, Verify};

    /// The current storage version.
    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// The number of descendants, in the highest difficulty chain, a block
        /// needs to have in order to be considered final.
        #[pallet::constant]
        type DescendantsUntilFinalized: Get<u8>;
        /// Determines whether Ethash PoW is verified for headers
        /// NOTE: Should only be false for dev
        #[pallet::constant]
        type VerifyPoW: Get<bool>;
        /// Weight information for extrinsics in this pallet
        type WeightInfo: WeightInfo;

        /// A configuration for base priority of unsigned transactions.
        #[pallet::constant]
        type UnsignedPriority: Get<TransactionPriority>;

        /// A configuration for longevity of unsigned transactions.
        #[pallet::constant]
        type UnsignedLongevity: Get<u64>;

        type ImportSignature: Verify<Signer = Self::Submitter> + Decode + Member + Encode + TypeInfo;
        type Submitter: IdentifyAccount<AccountId = Self::AccountId>
            + Decode
            + Member
            + Encode
            + TypeInfo
            + Clone;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T> {
        Finalized(EVMChainId, EthereumHeaderId),
    }

    #[derive(PartialEq, Clone)]
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
        /// Difficulty is too low comparing to last blocks difficulty
        DifficultyTooLow,
        /// Network state is not suitable to proceed transacton
        NetworkStateInvalid,
        /// This should never be returned - indicates a bug
        Unknown,
        /// Unsupported consensus engine
        ConsensusNotSupported,
        /// Signature provided inside unsigned extrinsic is not correct
        InvalidSignature,
        /// Header not found for block number
        HeaderNotFound,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    /// Best known block.
    #[pallet::storage]
    pub(super) type BestBlock<T: Config> =
        StorageMap<_, Identity, EVMChainId, (EthereumHeaderId, U256), OptionQuery>;

    /// Range of blocks that we want to prune.
    #[pallet::storage]
    pub(super) type BlocksToPrune<T: Config> =
        StorageMap<_, Identity, EVMChainId, PruningRange, OptionQuery>;

    /// Best finalized block.
    #[pallet::storage]
    pub(super) type FinalizedBlock<T: Config> =
        StorageMap<_, Identity, EVMChainId, EthereumHeaderId, OptionQuery>;

    /// Network config
    #[pallet::storage]
    pub(super) type NetworkConfig<T: Config> =
        StorageMap<_, Identity, EVMChainId, EthNetworkConfig, OptionQuery>;

    /// Map of imported headers by hash.
    #[pallet::storage]
    pub(super) type Headers<T: Config> = StorageDoubleMap<
        _,
        Identity,
        EVMChainId,
        Identity,
        H256,
        StoredHeader<T::AccountId>,
        OptionQuery,
    >;

    /// Map of imported header hashes by number.
    #[pallet::storage]
    pub(super) type HeadersByNumber<T: Config> =
        StorageDoubleMap<_, Identity, EVMChainId, Twox64Concat, u64, Vec<H256>, OptionQuery>;

    #[pallet::genesis_config]
    pub struct GenesisConfig {
        pub initial_networks: Vec<(EthNetworkConfig, EthereumHeader, U256)>,
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
            for (network_config, header, difficulty) in &self.initial_networks {
                NetworkConfig::<T>::insert(network_config.chain_id(), network_config);
                Pallet::<T>::initialize_storage_inner(
                    network_config.chain_id(),
                    vec![header.clone()],
                    difficulty.clone(),
                    0, // descendants_until_final = 0 forces the initial header to be finalized
                )
                .unwrap();

                <BlocksToPrune<T>>::insert(
                    network_config.chain_id(),
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
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::register_network())]
        pub fn register_network(
            origin: OriginFor<T>,
            network_config: EthNetworkConfig,
            header: EthereumHeader,
            initial_difficulty: U256,
        ) -> DispatchResult {
            ensure_root(origin)?;
            let network_id = network_config.chain_id();
            ensure!(
                <NetworkConfig<T>>::contains_key(network_id) == false,
                Error::<T>::NetworkAlreadyExists
            );
            ensure!(
                matches!(
                    network_config.consensus(),
                    Consensus::Ethash { .. } | Consensus::Etchash { .. }
                ),
                Error::<T>::ConsensusNotSupported
            );
            NetworkConfig::<T>::insert(network_id, network_config);
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

        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::update_difficulty_config())]
        pub fn update_difficulty_config(
            origin: OriginFor<T>,
            network_config: EthNetworkConfig,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(
                NetworkConfig::<T>::contains_key(network_config.chain_id()),
                Error::<T>::NetworkNotFound
            );
            NetworkConfig::<T>::insert(network_config.chain_id(), network_config);
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
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::import_header())]
        pub fn import_header(
            origin: OriginFor<T>,
            network_id: EVMChainId,
            header: EthereumHeader,
            proof: Vec<EthashProofData>,
            mix_nonce: MixNonce,
            submitter: <T as frame_system::Config>::AccountId,
            // Signature was already verified in `validate_unsigned()`
            _signature: <T as Config>::ImportSignature,
        ) -> DispatchResult {
            ensure_none(origin)?;

            log::trace!(
                target: "ethereum-light-client",
                "Received header {}. Starting import validation",
                header.number,
            );

            if let Err(err) = Self::validate_header(network_id, &header, &proof, &mix_nonce, true) {
                log::trace!(
                    target: "ethereum-light-client",
                    "Validation for header {} returned error. Skipping import",
                    header.number,
                );
                return Err(err.into());
            }

            log::trace!(
                target: "ethereum-light-client",
                "Validation succeeded. Starting import of header {}",
                header.number,
            );

            if let Err(err) = Self::import_validated_header(network_id, &submitter, &header) {
                log::trace!(
                    target: "ethereum-light-client",
                    "Import of header {} failed",
                    header.number,
                );
                return Err(err);
            }

            log::debug!(
                target: "ethereum-light-client",
                "Imported header {}!",
                header.number,
            );

            Ok(())
        }
    }

    impl<T: Config> Into<u8> for Error<T> {
        fn into(self) -> u8 {
            match self {
                Error::<T>::AncientHeader => 1,
                Error::<T>::MissingHeader => 2,
                Error::<T>::MissingParentHeader => 3,
                Error::<T>::DuplicateHeader => 4,
                Error::<T>::HeaderNotFinalized => 5,
                Error::<T>::HeaderOnStaleFork => 6,
                Error::<T>::InvalidHeader => 7,
                Error::<T>::InvalidProof => 8,
                Error::<T>::DecodeFailed => 9,
                Error::<T>::NetworkNotFound => 10,
                Error::<T>::NetworkAlreadyExists => 11,
                Error::<T>::Unknown => 12,
                Error::<T>::ConsensusNotSupported => 13,
                Error::<T>::InvalidSignature => 14,
                Error::<T>::DifficultyTooLow => 15,
                Error::<T>::NetworkStateInvalid => 16,
                Error::<T>::HeaderNotFound => 17,

                // Everything points to unreachable-ness (e.g. substrate macro definitions)
                // https://github.com/paritytech/substrate/blob/158cdfd1a43a122f8cfbf70473fcd54a3b418f3d/frame/support/procedural/src/pallet/expand/call.rs#L235
                Error::<T>::__Ignore(_, _) => unreachable!(),
            }
        }
    }

    #[pallet::validate_unsigned]
    impl<T: Config> ValidateUnsigned for Pallet<T> {
        type Call = Call<T>;
        // mb add prefetch with validate_ancestors=true to not include unnecessary stuff
        fn validate_unsigned(source: TransactionSource, call: &Self::Call) -> TransactionValidity {
            if let Call::import_header {
                network_id,
                header,
                proof,
                mix_nonce,
                submitter,
                signature,
            } = call
            {
                // We try to do as much verification from import_header as possible
                log::trace!(
                    target: "ethereum-light-client",
                    "Received header {}. Starting unsigned validation",
                    header.number,
                );
                if !signature.verify(
                    &bridge_types::import_digest(network_id, header)[..],
                    &submitter,
                ) {
                    return InvalidTransaction::Custom(Error::<T>::InvalidSignature.into()).into();
                }

                // Duplicate requests are filtered by substrate itself, but changing submitter +
                // signature pair bypasses this basic filtering. This leads to recalculation of one
                // PoW proof multiple times (until header with such number gets accepted).
                //
                // Since it doesn't affect the whole network and the solution is unclear (offchain
                // storage is inaccessible here) we decided to leave it.

                // We can't check parent, since it is most likely not imported (in storage) yet
                if let Err(err) =
                    Self::validate_header(*network_id, header, proof, mix_nonce, false)
                {
                    log::warn!(
                        target: "ethereum-light-client",
                        "Validation for header {} returned error {:?}. Dropping extrinsic",
                        header.number, err,
                    );
                    return InvalidTransaction::Custom(err.into()).into();
                }

                log::trace!(
                    target: "ethereum-light-client",
                    "Validation of header {} succeeded",
                    header.number,
                );

                let mut validity = ValidTransaction::with_tag_prefix("ImportHeaderETH")
                    .priority(T::UnsignedPriority::get())
                    .longevity(T::UnsignedLongevity::get())
                    .and_provides((network_id, header.compute_hash()))
                    .propagate(true);
                if !Headers::<T>::contains_key(network_id, header.parent_hash) {
                    validity = validity.and_requires((network_id, header.parent_hash));
                }
                validity.build()
            } else {
                log::warn!(
                    target: "ethereum-light-client",
                    "Unknown unsigned call, can't validate",
                );
                InvalidTransaction::Call.into()
            }
        }
    }

    impl<T: Config> Pallet<T> {
        // Validate an Ethereum header for import
        //
        // Must be called at least once with `validate_ancestors` flag,
        // for example in extrinsic dispatch
        fn validate_header(
            network_id: EVMChainId,
            header: &EthereumHeader,
            proof: &[EthashProofData],
            mix_nonce: &MixNonce,
            validate_ancestors: bool,
        ) -> Result<(), Error<T>> {
            let hash = header.compute_hash();
            ensure!(
                !<Headers<T>>::contains_key(network_id, hash),
                Error::<T>::DuplicateHeader,
            );

            let finalized_header_id =
                <FinalizedBlock<T>>::get(network_id).ok_or(Error::<T>::NetworkNotFound)?;

            let parent = <Headers<T>>::get(network_id, header.parent_hash).map(|x| x.header);
            // We require parent to be present if we validate ancestors
            if validate_ancestors {
                if let None = parent {
                    return Err(Error::<T>::MissingParentHeader);
                }
            }

            ensure!(
                header.number > finalized_header_id.number,
                Error::<T>::AncientHeader,
            );

            if validate_ancestors {
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
            }

            if !T::VerifyPoW::get() {
                return Ok(());
            }

            if let Some(parent) = &parent {
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
            } else {
                // Maximum that we can verify without having parent
                ensure!(
                    header.gas_used <= header.gas_limit
                        && header.gas_limit >= 5000u64.into()
                        && header.extra_data.len() <= 32,
                    Error::<T>::InvalidHeader,
                );
            }

            log::trace!(
                target: "ethereum-light-client",
                "Header {} passed basic verification",
                header.number
            );

            let consensus = NetworkConfig::<T>::get(network_id)
                .ok_or(Error::<T>::NetworkNotFound)?
                .consensus();
            if let Some(parent) = &parent {
                let header_difficulty = match consensus {
                    Consensus::Ethash { fork_config } => {
                        fork_config.calc_difficulty(header.timestamp, &parent)
                    }
                    Consensus::Etchash { fork_config } => {
                        fork_config.calc_difficulty(header.timestamp, &parent)
                    }
                    _ => return Err(Error::<T>::ConsensusNotSupported.into()),
                }
                .map_err(|err| {
                    log::debug!(
                        target: "ethereum-light-client",
                        "Header {} failed difficulty calculation: {}",
                        header.number, err
                    );
                    Error::<T>::InvalidHeader
                })?;
                ensure!(
                    header.difficulty == header_difficulty,
                    Error::<T>::InvalidHeader,
                );
            }

            Self::validate_header_difficulty(network_id, &header)?;

            log::trace!(
                target: "ethereum-light-client",
                "Header {} passed difficulty verification",
                header.number
            );

            let header_mix_hash = header.mix_hash().ok_or(Error::<T>::InvalidHeader)?;
            let header_nonce = header.nonce().ok_or(Error::<T>::InvalidHeader)?;

            log::trace!(target: "ethereum-light-client", "Prevalidating PoW with mix nonce");
            ensure!(
                H256::from(mix_nonce.as_bytes()) == header_mix_hash,
                Error::<T>::InvalidHeader,
            );
            let prover = EthashProver::new(consensus.calc_epoch_length(header.number));
            let result = prover.hashimoto_pre_validate(
                header.compute_partial_hash(),
                header_nonce,
                mix_nonce,
            );
            ensure!(
                U256::from(result.0) < ethash::cross_boundary(header.difficulty),
                Error::<T>::InvalidHeader,
            );

            log::trace!(target: "ethereum-light-client", "Calculating hashimoto_merkle");
            let (mix, result) = EthashProver::new(consensus.calc_epoch_length(header.number))
                .hashimoto_merkle(
                    header.compute_partial_hash(),
                    header_nonce,
                    header.number,
                    proof,
                )
                .map_err(|err| {
                    log::debug!(
                        target: "ethereum-light-client",
                        "Header {} failed PoW calculation: {:?}",
                        header.number, err
                    );
                    Error::<T>::InvalidHeader
                })?;

            log::trace!(
                target: "ethereum-light-client",
                "Header {} passed PoW verification",
                header.number
            );
            ensure!(
                H256::from(mix.as_bytes()) == header_mix_hash
                    && U256::from(result.0) < ethash::cross_boundary(header.difficulty),
                Error::<T>::InvalidHeader,
            );

            Ok(())
        }

        fn validate_header_difficulty(
            network_id: EVMChainId,
            new_header: &EthereumHeader,
        ) -> Result<(), Error<T>> {
            let check_block_number = match new_header
                .number
                .checked_sub(CHECK_DIFFICULTY_DIFFERENCE_NUMBER)
            {
                // If less than CHECK_DIFFICULTY_DIFFERENCE_NUMBER - ignore check
                None => return Ok(()),
                Some(num) => num,
            };

            let hashes = match HeadersByNumber::<T>::get(network_id, check_block_number) {
                // We trust our blockchain, so block should exist
                None => return Ok(()),
                Some(h) => h,
            };

            let headers_difficulty_max = match hashes
                .iter()
                .map(|hash| Headers::<T>::get(network_id, hash))
                .flat_map(|x| x)
                .map(|x| x.header.difficulty)
                .max()
            {
                None => frame_support::fail!(Error::<T>::NetworkStateInvalid),
                Some(max) => max,
            };

            // check total difficulty difference change and compare with new header difficulty
            ensure!(
                headers_difficulty_max
                    // .checked_sub(headers_prev_difficulty_min)
                    // .unwrap_or(0.into())
                    <= new_header
                        .difficulty
                        .checked_mul(DIFFICULTY_DIFFERENCE_MULT)
                        .unwrap_or(U256::MAX),
                Error::<T>::DifficultyTooLow
            );
            Ok(())
        }

        #[cfg(test)]
        pub fn validate_header_difficulty_test(
            network_id: EVMChainId,
            new_header: &EthereumHeader,
        ) -> DispatchResult {
            Self::validate_header_difficulty(network_id, new_header).map_err(|e| e.into())
        }

        // Import a new, validated Ethereum header
        fn import_validated_header(
            network_id: EVMChainId,
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
            network_id: EVMChainId,
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
            network_id: EVMChainId,
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
            network_id: EVMChainId,
            proof: &Proof,
        ) -> Result<(Receipt, u64), DispatchError> {
            let stored_header =
                <Headers<T>>::get(network_id, proof.block_hash).ok_or(Error::<T>::MissingHeader)?;

            ensure!(stored_header.finalized, Error::<T>::HeaderNotFinalized);

            let result = stored_header
                .header
                .check_receipt_proof(&proof.data)
                .ok_or(Error::<T>::InvalidProof)?;

            match result {
                Ok(receipt) => Ok((receipt, stored_header.header.timestamp)),
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
            network_id: EVMChainId,
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
                    parent.total_difficulty.saturating_add(header.difficulty)
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
        network_id: EVMChainId,
        mut hash: H256,
    ) -> impl Iterator<Item = (H256, EthereumHeader)> {
        sp_std::iter::from_fn(move || {
            let header = <Headers<T>>::get(network_id, &hash)?.header;
            let current_hash = hash;
            hash = header.parent_hash;
            Some((current_hash, header))
        })
    }

    impl<T: Config> EthereumGasPriceOracle for Pallet<T> {
        fn get_base_fee(
            network_id: EVMChainId,
            header_hash: H256,
        ) -> Result<Option<U256>, DispatchError> {
            let header =
                <Headers<T>>::get(network_id, &header_hash).ok_or(Error::<T>::HeaderNotFound)?;
            Ok(header.header.base_fee)
        }

        fn get_best_block_base_fee(network_id: EVMChainId) -> Result<Option<U256>, DispatchError> {
            let (header_id, _) =
                <BestBlock<T>>::get(network_id).ok_or(Error::<T>::NetworkNotFound)?;
            Self::get_base_fee(network_id, header_id.hash)
        }
    }

    impl<T: Config> Verifier for Pallet<T> {
        type Proof = Proof;
        /// Verify a message by verifying the existence of the corresponding
        /// Ethereum log in a block. Returns the log if successful.
        fn verify(
            network_id: GenericNetworkId,
            message_hash: H256,
            proof: &Self::Proof,
        ) -> DispatchResult {
            let network_id = network_id.evm().ok_or(Error::<T>::NetworkNotFound)?;
            let (receipt, timestamp) = Self::verify_receipt_inclusion(network_id, proof)?;

            log::trace!(
                target: "ethereum-light-client",
                "Verified receipt inclusion for transaction at index {} in block {}",
                proof.tx_index, proof.block_hash,
            );

            // Check transaction status according https://eips.ethereum.org/EIPS/eip-658
            if receipt.post_state_or_status != vec![1] {
                log::trace!(
                    target: "ethereum-light-client",
                    "Receipt has failed status for transaction at index {} in block {}",
                    proof.tx_index, proof.block_hash,
                );
                return Err(Error::<T>::InvalidProof.into());
            }

            if !receipt.contains_hashed_log(message_hash) {
                log::trace!(
                    target: "ethereum-light-client",
                    "Event log not found in receipt for transaction at index {} in block {}",
                    proof.tx_index, proof.block_hash,
                );
                return Err(Error::<T>::InvalidProof.into());
            }

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
}
