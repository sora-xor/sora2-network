//! ERC20App pallet benchmarking

use crate::*;
use bridge_types::types::ChannelId;
use bridge_types::EthNetworkId;
use common::{balance, AssetId32, PredefinedAssetId, DAI, XOR};
use frame_benchmarking::{account, benchmarks, whitelisted_caller};
use frame_support::traits::{Get, UnfilteredDispatchable};
use frame_system::RawOrigin;
use sp_core::H160;
use sp_runtime::traits::StaticLookup;
use sp_std::prelude::*;
use traits::MultiCurrency;

const BASE_NETWORK_ID: EthNetworkId = 12123;

benchmarks! {
    where_clause {where T: basic_channel::outbound::Config + incentivized_channel::outbound::Config, <T as frame_system::Config>::Origin: From<dispatch::RawOrigin>, T::AssetId: From<AssetId32<PredefinedAssetId>>}

    burn_basic_channel {
        let caller: T::AccountId = whitelisted_caller();
        let asset_id: T::AssetId = XOR.into();
        let recipient = H160::repeat_byte(2);
        let amount = balance!(500);

        basic_channel::outbound::Pallet::<T>::register_operator(RawOrigin::Root.into(), BASE_NETWORK_ID, caller.clone()).unwrap();

        <T as assets::Config>::Currency::deposit(asset_id.clone(), &caller, amount)?;

    }: burn(RawOrigin::Signed(caller.clone()), BASE_NETWORK_ID, ChannelId::Basic, asset_id.clone(), recipient, amount)
    verify {
        assert_eq!(assets::Pallet::<T>::free_balance(&asset_id, &caller).unwrap(), 0);
    }

    burn_incentivized_channel {
        let caller: T::AccountId = whitelisted_caller();
        let asset_id: T::AssetId = XOR.into();
        let recipient = H160::repeat_byte(2);
        let amount = balance!(500);

        let fee_asset = <T as incentivized_channel::outbound::Config>::FeeCurrency::get();

        // deposit enough money to cover fees
        <T as assets::Config>::Currency::deposit(fee_asset.clone(), &caller, incentivized_channel::outbound::Fee::<T>::get())?;
        <T as assets::Config>::Currency::deposit(asset_id.clone(), &caller, amount)?;
    }: burn(RawOrigin::Signed(caller.clone()), BASE_NETWORK_ID, ChannelId::Incentivized, asset_id.clone(), recipient, amount)
    verify {
        assert_eq!(assets::Pallet::<T>::free_balance(&asset_id, &caller).unwrap(), 0);
    }

    // Benchmark `mint` extrinsic under worst case conditions:
    // * `mint` successfully adds amount to recipient account
    mint {
        let asset_id: T::AssetId = DAI.into();
        let token = TokenAddresses::<T>::get(BASE_NETWORK_ID, &asset_id).unwrap();
        let asset_kind = AssetKinds::<T>::get(BASE_NETWORK_ID, &asset_id).unwrap();
        let caller = AppAddresses::<T>::get(BASE_NETWORK_ID, asset_kind).unwrap();
        let origin = dispatch::RawOrigin::from((BASE_NETWORK_ID, caller));

        let recipient: T::AccountId = account("recipient", 0, 0);
        let recipient_lookup: <T::Lookup as StaticLookup>::Source = T::Lookup::unlookup(recipient.clone());
        let sender = H160::zero();
        let amount = balance!(500);

        let call = Call::<T>::mint { token, sender, recipient: recipient_lookup, amount: amount.into()};

    }: { call.dispatch_bypass_filter(origin.into())? }
    verify {
        assert_eq!(assets::Pallet::<T>::free_balance(&asset_id, &recipient).unwrap(), amount);
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_tester(), crate::mock::Test,);
}
