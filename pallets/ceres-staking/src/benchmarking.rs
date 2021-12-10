//! Assets module benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use codec::Decode;
use common::{balance, AssetId32, AssetName, AssetSymbol, DEFAULT_BALANCE_PRECISION};
use frame_benchmarking::{benchmarks, Zero};
use frame_system::{EventRecord, RawOrigin};
use hex_literal::hex;
use sp_std::prelude::*;

use crate::Pallet as CeresStaking;

pub type AssetId = AssetId32<common::PredefinedAssetId>;
pub const CERES_ASSET_ID: AssetId = common::AssetId32::from_bytes(hex!(
    "008bcfd2387d3fc453333557eecb0efe59fcba128769b2feefdd306e98e66440"
));

// Support Functions
fn alice<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).unwrap_or_default()
}

fn assert_last_event<T: Config>(generic_event: <T as Config>::Event) {
    let events = frame_system::Module::<T>::events();
    let system_event: <T as frame_system::Config>::Event = generic_event.into();
    // compare to the last event record
    let EventRecord { event, .. } = &events[events.len() - 1];
    assert_eq!(event, &system_event);
}

benchmarks! {
    deposit {
        let caller = alice::<T>();
        let amount = balance!(100);
        frame_system::Pallet::<T>::inc_providers(&caller);
        let _ = assets::Pallet::<T>::register_asset_id(
            caller.clone(),
            CERES_ASSET_ID.into(),
            AssetSymbol(b"CERES".to_vec()),
            AssetName(b"Ceres".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::zero(),
            true,
            None,
            None,
        );
        let _ = assets::Pallet::<T>::mint(
            RawOrigin::Signed(caller.clone()).into(),
            CERES_ASSET_ID.into(),
            caller.clone(),
            balance!(101),
        );
    }: _(RawOrigin::Signed(caller.clone()), amount)
    verify {
        assert_last_event::<T>(Event::Deposited(caller.clone(), amount).into());
    }

    withdraw {
        let caller = alice::<T>();
        let amount = balance!(100);
        frame_system::Pallet::<T>::inc_providers(&caller);
        let _ = assets::Pallet::<T>::register_asset_id(
            caller.clone(),
            CERES_ASSET_ID.into(),
            AssetSymbol(b"CERES".to_vec()),
            AssetName(b"Ceres".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::zero(),
            true,
            None,
            None,
        );
        let _ = assets::Pallet::<T>::mint(
            RawOrigin::Signed(caller.clone()).into(),
            CERES_ASSET_ID.into(),
            caller.clone(),
            balance!(101),
        );
        let _ = CeresStaking::<T>::deposit(
            RawOrigin::Signed(caller.clone()).into(),
            amount
        );
    }: _(RawOrigin::Signed(caller.clone()))
    verify {
        assert_last_event::<T>(Event::Withdrawn(caller, amount, balance!(0)).into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::{ExtBuilder, Runtime};
    use frame_support::assert_ok;

    #[test]
    fn test_benchmarks() {
        ExtBuilder::default().build().execute_with(|| {
            assert_ok!(test_benchmark_deposit::<Runtime>());
            assert_ok!(test_benchmark_withdraw::<Runtime>());
        });
    }
}
