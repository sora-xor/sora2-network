//! ETHApp pallet benchmarking
use super::*;

use bridge_types::evm::AdditionalEVMInboundData;
use bridge_types::types::CallOriginOutput;
use bridge_types::H256;
use common::{AssetId32, PredefinedAssetId, DAI};
use frame_benchmarking::{benchmarks, impl_benchmark_test_suite};
use frame_system::RawOrigin;

pub const BASE_NETWORK_ID: EVMChainId = EVMChainId::zero();

#[allow(unused_imports)]
use crate::Pallet as MigrationApp;

benchmarks! {
    where_clause {where
        T::AssetId: From<AssetId32<PredefinedAssetId>>,
        <T as frame_system::Config>::RuntimeOrigin: From<dispatch::RawOrigin<CallOriginOutput<EVMChainId, H256, AdditionalEVMInboundData>>>,
        erc20_app::AssetIdOf<T>: From<AssetId32<PredefinedAssetId>>,
        eth_app::AssetIdOf<T>: From<AssetId32<PredefinedAssetId>>,
    }
    register_network {
        let contract = H160::repeat_byte(6);
    }: _(RawOrigin::Root, BASE_NETWORK_ID + 1, contract)
    verify {
        assert_eq!(Addresses::<T>::get(BASE_NETWORK_ID + 1), Some(contract));
    }
    migrate_eth {
    }: _(RawOrigin::Root, BASE_NETWORK_ID)
    migrate_erc20 {
    }: _(RawOrigin::Root, BASE_NETWORK_ID, vec![(DAI.into(), H160::repeat_byte(12), 18)])
    migrate_sidechain {
    }: _(RawOrigin::Root, BASE_NETWORK_ID, vec![(DAI.into(), H160::repeat_byte(12), 18)])
}

impl_benchmark_test_suite!(MigrationApp, crate::mock::new_tester(), crate::mock::Test,);
