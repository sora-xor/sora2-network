//! ERC20App pallet benchmarking

use crate::*;
use bridge_types::evm::AdditionalEVMInboundData;
use bridge_types::types::AssetKind;
use bridge_types::types::CallOriginOutput;
use bridge_types::EVMChainId;
use bridge_types::H256;
use common::{
    balance, AssetId32, AssetName, AssetSymbol, PredefinedAssetId, DAI, DEFAULT_BALANCE_PRECISION,
    ETH, XOR,
};
use frame_benchmarking::{account, benchmarks, whitelisted_caller};
use frame_support::traits::{Get, UnfilteredDispatchable};
use frame_system::RawOrigin;
use sp_runtime::traits::StaticLookup;
use sp_std::prelude::*;
use traits::MultiCurrency;

pub const BASE_NETWORK_ID: EVMChainId = EVMChainId::zero();

benchmarks! {
    where_clause {where
        T: bridge_outbound_channel::Config + assets::Config,
        <T as frame_system::Config>::RuntimeOrigin: From<dispatch::RawOrigin<CallOriginOutput<EVMChainId, H256, AdditionalEVMInboundData>>>,
        AssetIdOf<T>: From<AssetId32<PredefinedAssetId>> + From<<T as assets::Config>::AssetId>,
        <T as assets::Config>::AssetId: From<AssetIdOf<T>>,
        AssetNameOf<T>: From<common::AssetName>,
        AssetSymbolOf<T>: From<common::AssetSymbol>,
        BalanceOf<T>: From<u128>,
    }

    burn {
        let caller: T::AccountId = whitelisted_caller();
        let asset_id: AssetIdOf<T> = XOR.into();
        let recipient = H160::repeat_byte(2);
        let amount = balance!(500);

        let fee_asset: AssetIdOf<T> = <T as bridge_outbound_channel::Config>::FeeCurrency::get().into();

        // deposit enough money to cover fees
        <T as assets::Config>::Currency::deposit(fee_asset.clone().into(), &caller, bridge_outbound_channel::Fee::<T>::get().into())?;
        <T as assets::Config>::Currency::deposit(asset_id.clone().into(), &caller, amount.into())?;
    }: burn(RawOrigin::Signed(caller.clone()), BASE_NETWORK_ID, asset_id.clone(), recipient, amount.into())
    verify {
        assert_eq!(<T as assets::Config>::Currency::free_balance(asset_id.into(), &caller), 0u128.into());
    }

    // Benchmark `mint` extrinsic under worst case conditions:
    // * `mint` successfully adds amount to recipient account
    mint {
        let asset_id: AssetIdOf<T> = DAI.into();
        let token = TokenAddresses::<T>::get(BASE_NETWORK_ID, &asset_id).unwrap();
        let asset_kind = AssetKinds::<T>::get(BASE_NETWORK_ID, &asset_id).unwrap();
        let caller = AppAddresses::<T>::get(BASE_NETWORK_ID, asset_kind).unwrap();
        let origin = dispatch::RawOrigin::new(CallOriginOutput {network_id: BASE_NETWORK_ID, additional: AdditionalEVMInboundData{source: caller}, ..Default::default()});

        let recipient: T::AccountId = account("recipient", 0, 0);
        let recipient_lookup: <T::Lookup as StaticLookup>::Source = T::Lookup::unlookup(recipient.clone());
        let sender = H160::zero();
        let amount = balance!(500);

        let call = Call::<T>::mint { token, sender, recipient: recipient_lookup, amount: amount.into()};

    }: { call.dispatch_bypass_filter(origin.into())? }
    verify {
        assert_eq!(<T as assets::Config>::Currency::free_balance(asset_id.into(), &recipient), amount.into());
    }

    register_erc20_app {
        let address = H160::repeat_byte(98);
        let network_id = BASE_NETWORK_ID + 1;
        assert!(!AppAddresses::<T>::contains_key(network_id, AssetKind::Sidechain));
    }: _(RawOrigin::Root, network_id, address)
    verify {
        assert!(AppAddresses::<T>::contains_key(network_id, AssetKind::Sidechain));
    }

    register_native_app {
        let address = H160::repeat_byte(98);
        let network_id = BASE_NETWORK_ID + 1;
        assert!(!AppAddresses::<T>::contains_key(network_id, AssetKind::Thischain));
    }: _(RawOrigin::Root, network_id, address)
    verify {
        assert!(AppAddresses::<T>::contains_key(network_id, AssetKind::Thischain));
    }

    register_erc20_asset {
        let asset_id: AssetIdOf<T> = ETH.into();
        let address = H160::repeat_byte(98);
        let network_id = BASE_NETWORK_ID;
        let symbol = AssetSymbol(b"ETH".to_vec());
        let name = AssetName(b"ETH".to_vec());
        assert!(!AssetsByAddresses::<T>::contains_key(network_id, address));
    }: _(RawOrigin::Root, network_id, address, symbol.into(), name.into(), DEFAULT_BALANCE_PRECISION)
    verify {
        assert!(AssetsByAddresses::<T>::contains_key(network_id, address));
    }

    register_native_asset {
        let asset_id: AssetIdOf<T> = ETH.into();
        let network_id = BASE_NETWORK_ID;
    }: _(RawOrigin::Root, network_id, asset_id)
    verify {
    }

    register_asset_internal {
        let asset_id: AssetIdOf<T> = ETH.into();
        let who = AppAddresses::<T>::get(BASE_NETWORK_ID, AssetKind::Thischain).unwrap();
        let origin = dispatch::RawOrigin::new(CallOriginOutput {network_id: BASE_NETWORK_ID, additional: AdditionalEVMInboundData{source: who}, ..Default::default()});
        let address = H160::repeat_byte(98);
        assert!(!TokenAddresses::<T>::contains_key(BASE_NETWORK_ID, &asset_id));
    }: _(origin, asset_id.clone(), address)
    verify {
        assert_eq!(AssetKinds::<T>::get(BASE_NETWORK_ID, &asset_id), Some(AssetKind::Thischain));
        assert!(TokenAddresses::<T>::contains_key(BASE_NETWORK_ID, &asset_id));
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_tester(), crate::mock::Test,);
}
