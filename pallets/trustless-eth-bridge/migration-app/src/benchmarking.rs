//! ETHApp pallet benchmarking
use super::*;

use common::{AssetId32, PredefinedAssetId};
use frame_benchmarking::{benchmarks, impl_benchmark_test_suite};
use frame_system::RawOrigin;
use sp_core::H160;

const BASE_NETWORK_ID: EthNetworkId = 12123;

#[allow(unused_imports)]
use crate::Pallet as MigrationApp;

benchmarks! {
    where_clause {where T::AssetId: From<AssetId32<PredefinedAssetId>>, <T as frame_system::Config>::Origin: From<dispatch::RawOrigin>}
    register_network {
        let contract = H160::repeat_byte(6);
    }: _(RawOrigin::Root, BASE_NETWORK_ID + 1, contract)
    verify {
        assert_eq!(Addresses::<T>::get(BASE_NETWORK_ID + 1), Some(contract));
    }
}

impl_benchmark_test_suite!(MigrationApp, crate::mock::new_tester(), crate::mock::Test,);
