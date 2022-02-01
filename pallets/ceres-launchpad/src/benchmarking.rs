//! Ceres launchpad module benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use codec::Decode;
use common::{balance, AssetId32, AssetName, AssetSymbol, DEFAULT_BALANCE_PRECISION};
use frame_benchmarking::{benchmarks, Zero};
use frame_system::{EventRecord, RawOrigin};
use hex_literal::hex;
use sp_std::prelude::*;

use crate::Pallet as CeresLaunchpad;

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
    create_ilo {
        let caller = alice::<T>();
        frame_system::Pallet::<T>::inc_providers(&caller);
        let current_block = frame_system::Pallet::<T>::block_number();

        let assets_and_permissions_tech_account_id =
            T::TechAccountId::from_generic_pair(b"SYSTEM_ACCOUNT".to_vec(), b"ASSETS_PERMISSIONS".to_vec());
        let assets_and_permissions_account_id =
            Technical::<T>::tech_account_id_to_account_id(
                &assets_and_permissions_tech_account_id,
            ).unwrap();

        let _ = Assets::<T>::mint(
            RawOrigin::Signed(assets_and_permissions_account_id.clone()).into(),
            CERES_ASSET_ID.into(),
            caller.clone(),
            balance!(20000)
        );
    }: _(RawOrigin::Signed(caller.clone()),
        CERES_ASSET_ID.into(),
        balance!(7693),
        balance!(3000),
        balance!(0.13),
        balance!(600),
        balance!(1000),
        balance!(0.2),
        balance!(0.25),
        true,
        balance!(0.75),
        balance!(0.25),
        31,
        current_block + 5u32.into(),
        current_block + 10u32.into(),
        balance!(0.2),
        current_block + 3u32.into(),
        balance!(0.2)
    )
    verify {
        assert_last_event::<T>(Event::ILOCreated(caller.clone(), CERES_ASSET_ID.into()).into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::{ExtBuilder, Runtime};
    use frame_support::assert_ok;

    #[test]
    fn test_benchmarks() {
        ExtBuilder::empty().build().execute_with(|| {
            assert_ok!(test_benchmark_create_ilo::<Runtime>());
        });
    }
}
