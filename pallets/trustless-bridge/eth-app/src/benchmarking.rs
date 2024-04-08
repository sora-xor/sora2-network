//! ETHApp pallet benchmarking
use super::*;

use crate::{AssetIdOf, AssetNameOf, AssetSymbolOf, BalanceOf};
use bridge_types::evm::AdditionalEVMInboundData;
use bridge_types::types::CallOriginOutput;
use bridge_types::H160;
use bridge_types::H256;
use common::{balance, AssetId32, PredefinedAssetId, XOR};
use common::{AssetName, AssetSymbol};
use frame_benchmarking::{account, benchmarks, whitelisted_caller};
use frame_support::traits::UnfilteredDispatchable;
use frame_system::RawOrigin;
use traits::MultiCurrency;

pub const BASE_NETWORK_ID: EVMChainId = EVMChainId::zero();

#[allow(unused_imports)]
use crate::Pallet as ETHApp;

benchmarks! {
    where_clause {where
        T: assets::Config,
        AssetIdOf<T>: From<AssetId32<PredefinedAssetId>>,
        AssetNameOf<T>: From<common::AssetName>,
        AssetSymbolOf<T>: From<common::AssetSymbol>,
        BalanceOf<T>: From<u128>,
        <T as common::Config>::AssetId: From<AssetIdOf<T>>,
        <T as frame_system::Config>::RuntimeOrigin: From<dispatch::RawOrigin<CallOriginOutput<EVMChainId, H256, AdditionalEVMInboundData>>>
    }
    // Benchmark `burn` extrinsic under worst case conditions:
    // * `burn` successfully substracts amount from caller account
    // * The channel executes incentivization logic
    burn {
        let caller: T::AccountId = whitelisted_caller();
        let recipient = H160::repeat_byte(2);
        let amount = balance!(20);
        let asset_id: AssetIdOf<T> = XOR.into();

        <T as common::Config>::Currency::deposit(asset_id.clone().into(), &caller, amount.into())?;

    }: _(RawOrigin::Signed(caller.clone()), BASE_NETWORK_ID, recipient, amount.into())
    verify {
        assert_eq!(<T as common::Config>::Currency::total_balance(asset_id.into(), &caller), balance!(0).into());
    }

    // Benchmark `mint` extrinsic under worst case conditions:
    // * `mint` successfully adds amount to recipient account
    mint {
        let (contract, asset_id, precision) = Addresses::<T>::get(BASE_NETWORK_ID).unwrap();
        let origin = dispatch::RawOrigin::new(CallOriginOutput{network_id: BASE_NETWORK_ID, additional: AdditionalEVMInboundData{ source: contract }, ..Default::default()});

        let recipient: T::AccountId = account("recipient", 0, 0);
        let recipient_lookup: <T::Lookup as StaticLookup>::Source = T::Lookup::unlookup(recipient.clone());
        let sender = H160::zero();
        let amount = balance!(500);

        let call = Call::<T>::mint{sender, recipient: recipient_lookup, amount: amount.into()};

    }: { call.dispatch_bypass_filter(origin.into())? }
    verify {
        assert_eq!(<T as assets::Config>::Currency::total_balance(asset_id.into(), &recipient), amount.into());
    }

    register_network {
        let contract = H160::repeat_byte(6);
        let asset_name = AssetName(b"ETH".to_vec());
        let asset_symbol = AssetSymbol(b"ETH".to_vec());
    }: _(RawOrigin::Root, BASE_NETWORK_ID + 1, asset_name.into(), asset_symbol.into(), 18, contract)
    verify {
        assert_eq!(Addresses::<T>::get(BASE_NETWORK_ID + 1).unwrap().0, contract);
    }

    register_network_with_existing_asset {
        let asset_id: AssetIdOf<T> = XOR.into();
        let contract = H160::repeat_byte(6);
    }: _(RawOrigin::Root, BASE_NETWORK_ID + 1, asset_id.clone(), contract, 18)
    verify {
        assert_eq!(Addresses::<T>::get(BASE_NETWORK_ID + 1), Some((contract, asset_id, 18)));
    }

    impl_benchmark_test_suite!(ETHApp, crate::mock::new_tester(), crate::mock::Test,);
}
