//! EthereumLightClient pallet benchmarking
use super::*;

use bridge_types::import_digest;
use bridge_types::network_config::NetworkConfig as EthNetworkConfig;
use frame_benchmarking::benchmarks;
use frame_support::assert_ok;
use frame_support::dispatch::UnfilteredDispatchable;
use frame_support::unsigned::ValidateUnsigned;
use frame_system::RawOrigin;
use sp_core::sr25519::{Public, Signature};
use sp_runtime::transaction_validity::TransactionSource;

use crate::Pallet as EthereumLightClient;

mod data;

/// The index up until which headers are reserved for pruning. The header at
/// `data::headers_11963025_to_11963069()[RESERVED_FOR_PRUNING]` is specially
/// chosen to be a sibling of the previous header. Indices 0 to RESERVED_FOR_PRUNING - 1
/// contain strictly increasing block numbers.
const RESERVED_FOR_PRUNING: usize = HEADERS_TO_PRUNE_IN_SINGLE_IMPORT as usize;

fn get_best_block<T: Config>() -> (EthereumHeaderId, U256) {
    <BestBlock<T>>::get(EthNetworkConfig::Mainnet.chain_id()).unwrap()
}

fn get_blocks_to_prune<T: Config>() -> PruningRange {
    <BlocksToPrune<T>>::get(EthNetworkConfig::Mainnet.chain_id()).unwrap()
}

fn set_blocks_to_prune<T: Config>(oldest_unpruned: u64, oldest_to_keep: u64) {
    <BlocksToPrune<T>>::insert(
        EthNetworkConfig::Mainnet.chain_id(),
        PruningRange {
            oldest_unpruned_block: oldest_unpruned,
            oldest_block_to_keep: oldest_to_keep,
        },
    );
}

fn assert_header_pruned<T: Config>(hash: H256, number: u64) {
    assert!(Headers::<T>::get(EthNetworkConfig::Mainnet.chain_id(), hash).is_none());

    let hashes_at_number = <HeadersByNumber<T>>::get(EthNetworkConfig::Mainnet.chain_id(), number);
    assert!(hashes_at_number.is_none() || !hashes_at_number.unwrap().contains(&hash),);
}

fn digest_signature<T: crate::Config>(
    signer: &Public,
    network_id: &EVMChainId,
    header: &EthereumHeader,
) -> Signature {
    sp_io::crypto::sr25519_sign(123.into(), signer, &import_digest(network_id, header)[..]).unwrap()
}

/// Calls validate_unsigned and dispatches the call. We need to count
/// both steps as the weight should represent actual computations
/// happened.
fn validate_dispatch<T: crate::Config>(call: Call<T>) -> Result<(), &'static str> {
    EthereumLightClient::<T>::validate_unsigned(TransactionSource::InBlock, &call)
        .map_err(|e| -> &'static str { e.into() })?;
    <Call<T> as Decode>::decode(&mut &*call.encode())
        .expect("Should be decoded fine, encoding is just above")
        .dispatch_bypass_filter(RawOrigin::None.into())?;
    Ok(())
}

benchmarks! {
    where_clause {
        where
            <T as Config>::Submitter: From<Public>,
            <T as Config>::ImportSignature: From<Signature>,
    }
    // Benchmark `import_header` extrinsic under worst case conditions:
    // * Import will set a new best block.
    // * Import will set a new finalized header.s
    // * Import will iterate over the max value of DescendantsUntilFinalized headers
    //   in the chain.
    // * Import will prune HEADERS_TO_PRUNE_IN_SINGLE_IMPORT headers.
    // * Pruned headers will come from distinct block numbers so that we have the max
    //   number of HeaderByNumber::take calls.
    // * The last pruned header will have siblings that we don't prune and have to
    //   re-insert using <HeadersByNumber<T>>::insert.
    validate_unsigned_then_import_header {
        // We don't care about security but just about calculation time
        let caller_public = sp_io::crypto::sr25519_generate(123.into(), None);
        let caller = <T as Config>::Submitter::from(caller_public).into_account();

        let descendants_until_final = T::DescendantsUntilFinalized::get();

        let next_finalized_idx = RESERVED_FOR_PRUNING + 1;
        let next_tip_idx = next_finalized_idx + descendants_until_final as usize;
        let headers = data::headers_11963025_to_11963069();
        let header = headers[next_tip_idx].clone();
        let header_proof = data::header_proof(header.compute_hash()).unwrap();
        let header_mix_nonce = data::header_mix_nonce(header.compute_hash()).unwrap();

        NetworkConfig::<T>::insert(EthNetworkConfig::Mainnet.chain_id(), EthNetworkConfig::Mainnet);
        EthereumLightClient::<T>::initialize_storage_inner(
            EthNetworkConfig::Mainnet.chain_id(),
            headers[0..next_tip_idx].to_vec(),
            U256::zero(),
            descendants_until_final,
        )?;

        set_blocks_to_prune::<T>(
            headers[0].number,
            headers[next_finalized_idx].number,
        );
        let call = Call::<T>::import_header {
            network_id: EthNetworkConfig::Mainnet.chain_id(),
            header: header.clone(),
            proof: header_proof,
            mix_nonce: header_mix_nonce,
            submitter: caller,
            signature: <T as Config>::ImportSignature::from(digest_signature::<T>(&caller_public, &EthNetworkConfig::Mainnet.chain_id(), &header))
        };
    }: { validate_dispatch(call)? }
    verify {
        // Check that the best header has been updated
        let best = &headers[next_tip_idx];
        assert_eq!(
            get_best_block::<T>().0,
            EthereumHeaderId {
                number: best.number,
                hash: best.compute_hash(),
            },
        );

        // Check that `RESERVED_FOR_PRUNING` headers have been pruned
        // while leaving 1 sibling behind
        headers[0..RESERVED_FOR_PRUNING]
            .iter()
            .for_each(|h| assert_header_pruned::<T>(h.compute_hash(), h.number));
        let last_pruned_sibling = &headers[RESERVED_FOR_PRUNING];
        assert_eq!(
            get_blocks_to_prune::<T>().oldest_unpruned_block,
            last_pruned_sibling.number,
        );
    }

    // Benchmark `import_header` extrinsic under worst case conditions:
    // * Import will set a new best block.
    // * Import will *not* set a new finalized header because its sibling was imported first.
    // * Import will iterate over the max value of DescendantsUntilFinalized headers
    //   in the chain.
    // * Import will prune HEADERS_TO_PRUNE_IN_SINGLE_IMPORT headers.
    // * Pruned headers will come from distinct block numbers so that we have the max
    //   number of HeaderByNumber::take calls.
    // * The last pruned header will have siblings that we don't prune and have to
    //   re-insert using <HeadersByNumber<T>>::insert.
    import_header_not_new_finalized_with_max_prune {
        let caller_public = sp_io::crypto::sr25519_generate(123.into(), None);
        let caller = <T as Config>::Submitter::from(caller_public).into_account();

        let descendants_until_final = T::DescendantsUntilFinalized::get();

        let finalized_idx = RESERVED_FOR_PRUNING + 1;
        let next_tip_idx = finalized_idx + descendants_until_final as usize;
        let headers = data::headers_11963025_to_11963069();
        let header = headers[next_tip_idx].clone();
        let header_proof = data::header_proof(header.compute_hash()).unwrap();
        let header_mix_nonce = data::header_mix_nonce(header.compute_hash()).unwrap();

        let mut header_sibling = header.clone();
        header_sibling.difficulty -= 1u32.into();
        let mut init_headers = headers[0..next_tip_idx].to_vec();
        init_headers.append(&mut vec![header_sibling]);

        NetworkConfig::<T>::insert(EthNetworkConfig::Mainnet.chain_id(), EthNetworkConfig::Mainnet);
        EthereumLightClient::<T>::initialize_storage_inner(
            EthNetworkConfig::Mainnet.chain_id(),
            init_headers,
            U256::zero(),
            descendants_until_final,
        )?;

        set_blocks_to_prune::<T>(
            headers[0].number,
            headers[finalized_idx].number,
        );

        let call = Call::<T>::import_header {
            network_id: EthNetworkConfig::Mainnet.chain_id(),
            header: header.clone(),
            proof: header_proof,
            mix_nonce: header_mix_nonce,
            submitter: caller,
            signature: <T as Config>::ImportSignature::from(digest_signature::<T>(&caller_public, &EthNetworkConfig::Mainnet.chain_id(), &header))
        };
    }: { validate_dispatch(call)? }
    verify {
        // Check that the best header has been updated
        let best = &headers[next_tip_idx];
        assert_eq!(
            get_best_block::<T>().0,
            EthereumHeaderId {
                number: best.number,
                hash: best.compute_hash(),
            },
        );

        // Check that `RESERVED_FOR_PRUNING` headers have been pruned
        // while leaving 1 sibling behind
        headers[0..RESERVED_FOR_PRUNING]
            .iter()
            .for_each(|h| assert_header_pruned::<T>(h.compute_hash(), h.number));
        let last_pruned_sibling = &headers[RESERVED_FOR_PRUNING];
        assert_eq!(
            get_blocks_to_prune::<T>().oldest_unpruned_block,
            last_pruned_sibling.number,
        );
    }

    // Benchmark `import_header` extrinsic under average case conditions:
    // * Import will set a new best block.
    // * Import will set a new finalized header.
    // * Import will iterate over the max value of DescendantsUntilFinalized headers
    //   in the chain.
    // * Import will prune a single old header with no siblings.
    import_header_new_finalized_with_single_prune {
        let caller_public = sp_io::crypto::sr25519_generate(123.into(), None);
        let caller = <T as Config>::Submitter::from(caller_public).into_account();

        let descendants_until_final = T::DescendantsUntilFinalized::get();

        let finalized_idx = RESERVED_FOR_PRUNING + 1;
        let next_tip_idx = finalized_idx + descendants_until_final as usize;
        let headers = data::headers_11963025_to_11963069();
        let header = headers[next_tip_idx].clone();
        let header_proof = data::header_proof(header.compute_hash()).unwrap();
        let header_mix_nonce = data::header_mix_nonce(header.compute_hash()).unwrap();

        NetworkConfig::<T>::insert(EthNetworkConfig::Mainnet.chain_id(), EthNetworkConfig::Mainnet);
        EthereumLightClient::<T>::initialize_storage_inner(
            EthNetworkConfig::Mainnet.chain_id(),
            headers[0..next_tip_idx].to_vec(),
            U256::zero(),
            descendants_until_final,
        )?;

        set_blocks_to_prune::<T>(
            headers[0].number,
            headers[0].number + 1,
        );

        let call = Call::<T>::import_header {
            network_id: EthNetworkConfig::Mainnet.chain_id(),
            header: header.clone(),
            proof: header_proof,
            mix_nonce: header_mix_nonce,
            submitter: caller,
            signature: <T as Config>::ImportSignature::from(digest_signature::<T>(&caller_public, &EthNetworkConfig::Mainnet.chain_id(), &header))
        };
    }: { validate_dispatch(call)? }
    verify {
        // Check that the best header has been updated
        let best = &headers[next_tip_idx];
        assert_eq!(
            get_best_block::<T>().0,
            EthereumHeaderId {
                number: best.number,
                hash: best.compute_hash(),
            },
        );

        // Check that 1 header has been pruned
        let oldest_header = &headers[0];
        assert_header_pruned::<T>(oldest_header.compute_hash(), oldest_header.number);
        assert_eq!(
            get_blocks_to_prune::<T>().oldest_unpruned_block,
            oldest_header.number + 1,
        );
    }

    // Benchmark `import_header` extrinsic under average case conditions:
    // * Import will set a new best block.
    // * Import will *not* set a new finalized header because its sibling was imported first.
    // * Import will iterate over the max value of DescendantsUntilFinalized headers
    //   in the chain.
    // * Import will prune a single old header with no siblings.
    import_header_not_new_finalized_with_single_prune {
        let caller_public = sp_io::crypto::sr25519_generate(123.into(), None);
        let caller = <T as Config>::Submitter::from(caller_public).into_account();

        let descendants_until_final = T::DescendantsUntilFinalized::get();

        let finalized_idx = RESERVED_FOR_PRUNING + 1;
        let next_tip_idx = finalized_idx + descendants_until_final as usize;
        let headers = data::headers_11963025_to_11963069();
        let header = headers[next_tip_idx].clone();
        let header_proof = data::header_proof(header.compute_hash()).unwrap();
        let header_mix_nonce = data::header_mix_nonce(header.compute_hash()).unwrap();

        let mut header_sibling = header.clone();
        header_sibling.difficulty -= 1u32.into();
        let mut init_headers = headers[0..next_tip_idx].to_vec();
        init_headers.append(&mut vec![header_sibling]);

        NetworkConfig::<T>::insert(EthNetworkConfig::Mainnet.chain_id(), EthNetworkConfig::Mainnet);
        EthereumLightClient::<T>::initialize_storage_inner(
            EthNetworkConfig::Mainnet.chain_id(),
            init_headers,
            U256::zero(),
            descendants_until_final,
        )?;

        set_blocks_to_prune::<T>(
            headers[0].number,
            headers[0].number + 1,
        );

        let call = Call::<T>::import_header {
            network_id: EthNetworkConfig::Mainnet.chain_id(),
            header: header.clone(),
            proof: header_proof,
            mix_nonce: header_mix_nonce,
            submitter: caller,
            signature: <T as Config>::ImportSignature::from(digest_signature::<T>(&caller_public, &EthNetworkConfig::Mainnet.chain_id(), &header))
        };
    }: { validate_dispatch(call)? }
    verify {
        // Check that the best header has been updated
        let best = &headers[next_tip_idx];
        assert_eq!(
            get_best_block::<T>().0,
            EthereumHeaderId {
                number: best.number,
                hash: best.compute_hash(),
            },
        );

        // Check that 1 header has been pruned
        let oldest_header = &headers[0];
        assert_header_pruned::<T>(oldest_header.compute_hash(), oldest_header.number);
        assert_eq!(
            get_blocks_to_prune::<T>().oldest_unpruned_block,
            oldest_header.number + 1,
        );
    }

    register_network {
        let descendants_until_final = T::DescendantsUntilFinalized::get();

        let next_finalized_idx = RESERVED_FOR_PRUNING + 1;
        let next_tip_idx = next_finalized_idx + descendants_until_final as usize;
        let headers = data::headers_11963025_to_11963069();
    }: _(RawOrigin::Root, EthNetworkConfig::Mainnet, headers[next_tip_idx-1].clone(), U256::zero())
    verify {
        let header = headers[next_tip_idx].clone();
        let header_proof = data::header_proof(header.compute_hash()).unwrap();
        let header_mix_nonce = data::header_mix_nonce(header.compute_hash()).unwrap();

        let caller_public = sp_io::crypto::sr25519_generate(123.into(), None);
        let caller = <T as Config>::Submitter::from(caller_public).into_account();

        assert_ok!(EthereumLightClient::<T>::import_header(
            RawOrigin::None.into(),
            EthNetworkConfig::Mainnet.chain_id(),
            header.clone(),
            header_proof,
            header_mix_nonce,
            caller,
            <T as Config>::ImportSignature::from(digest_signature::<T>(&caller_public, &EthNetworkConfig::Mainnet.chain_id(), &header))
        ));
    }

    update_difficulty_config {
        let descendants_until_final = T::DescendantsUntilFinalized::get();

        let next_finalized_idx = RESERVED_FOR_PRUNING + 1;
        let next_tip_idx = next_finalized_idx + descendants_until_final as usize;
        let headers = data::headers_11963025_to_11963069();
        EthereumLightClient::<T>::register_network(RawOrigin::Root.into(), EthNetworkConfig::Mainnet, headers[next_tip_idx-1].clone(), U256::zero()).unwrap();
        let network_config = EthNetworkConfig::Custom {
            chain_id: EthNetworkConfig::Mainnet.chain_id(),
            consensus: EthNetworkConfig::Ropsten.consensus(),
        };
    }: _(RawOrigin::Root, network_config)
    verify {
        assert_eq!(crate::NetworkConfig::<T>::get(EthNetworkConfig::Mainnet.chain_id()).unwrap().consensus(), network_config.consensus());
    }

    impl_benchmark_test_suite!(
        EthereumLightClient,
        crate::mock::new_tester::<crate::mock::mock_verifier_with_pow::Test>(),
        crate::mock::mock_verifier_with_pow::Test,
    );
}
