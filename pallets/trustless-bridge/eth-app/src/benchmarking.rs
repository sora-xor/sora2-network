//! ETHApp pallet benchmarking
use super::*;

use bridge_types::types::{AdditionalEVMInboundData, CallOriginOutput};
use bridge_types::H160;
use bridge_types::H256;
use common::{balance, AssetId32, AssetInfoProvider, PredefinedAssetId, XOR};
use common::{AssetName, AssetSymbol, DEFAULT_BALANCE_PRECISION};
use frame_benchmarking::{account, benchmarks, whitelisted_caller};
use frame_support::traits::UnfilteredDispatchable;
use frame_system::RawOrigin;
use traits::MultiCurrency;

pub const BASE_NETWORK_ID: EVMChainId = EVMChainId::zero();

#[allow(unused_imports)]
use crate::Pallet as ETHApp;

benchmarks! {
    where_clause {where T::AssetId: From<AssetId32<PredefinedAssetId>>, <T as frame_system::Config>::RuntimeOrigin: From<dispatch::RawOrigin<EVMChainId, AdditionalEVMInboundData, CallOriginOutput<EVMChainId, H256, AdditionalEVMInboundData>>>}
    // Benchmark `burn` extrinsic under worst case conditions:
    // * `burn` successfully substracts amount from caller account
    // * The channel executes incentivization logic
    burn {
        let caller: T::AccountId = whitelisted_caller();
        let recipient = H160::repeat_byte(2);
        let amount = balance!(20);
        let asset_id: T::AssetId = XOR.into();

        <T as assets::Config>::Currency::deposit(asset_id.clone(), &caller, amount)?;

    }: _(RawOrigin::Signed(caller.clone()), BASE_NETWORK_ID, recipient, amount)
    verify {
        assert_eq!(assets::Pallet::<T>::total_balance(&asset_id, &caller).unwrap(), balance!(0));
    }

    // Benchmark `mint` extrinsic under worst case conditions:
    // * `mint` successfully adds amount to recipient account
    mint {
        let (contract, asset_id) = Addresses::<T>::get(BASE_NETWORK_ID).unwrap();
        let origin = dispatch::RawOrigin::new(CallOriginOutput{network_id: BASE_NETWORK_ID, additional: AdditionalEVMInboundData{ source: contract }, ..Default::default()});

        let recipient: T::AccountId = account("recipient", 0, 0);
        let recipient_lookup: <T::Lookup as StaticLookup>::Source = T::Lookup::unlookup(recipient.clone());
        let sender = H160::zero();
        let amount = balance!(500);

        let call = Call::<T>::mint{sender, recipient: recipient_lookup, amount: amount.into()};

    }: { call.dispatch_bypass_filter(origin.into())? }
    verify {
        assert_eq!(assets::Pallet::<T>::total_balance(&asset_id, &recipient).unwrap(), amount);
    }

    register_network {
        let contract = H160::repeat_byte(6);
        let asset_name = AssetName(b"ETH".to_vec());
        let asset_symbol = AssetSymbol(b"ETH".to_vec());
    }: _(RawOrigin::Root, BASE_NETWORK_ID + 1, asset_name, asset_symbol, DEFAULT_BALANCE_PRECISION, contract)
    verify {
        assert_eq!(Addresses::<T>::get(BASE_NETWORK_ID + 1).unwrap().0, contract);
    }

    register_network_with_existing_asset {
        let asset_id: T::AssetId = XOR.into();
        let contract = H160::repeat_byte(6);
    }: _(RawOrigin::Root, BASE_NETWORK_ID + 1, asset_id, contract)
    verify {
        assert_eq!(Addresses::<T>::get(BASE_NETWORK_ID + 1), Some((contract, asset_id)));
    }

    impl_benchmark_test_suite!(ETHApp, crate::mock::new_tester(), crate::mock::Test,);
}
