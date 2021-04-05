//! PSWAP distribution module benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use codec::Decode;
use frame_benchmarking::benchmarks;
use frame_system::{EventRecord, RawOrigin};
use hex_literal::hex;
use sp_std::prelude::*;
use traits::MultiCurrencyExtended;

use common::{AssetSymbol, AssetName, Fixed, balance, XOR, PSWAP};
use common::fixnum::ops::One;

use crate::Pallet as MBCPool;
use assets::Pallet as Assets;
use trading_pair::Pallet as TradingPair;
use tokens::Pallet as Tokens;
use sp_std::convert::TryFrom;

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
    claim_incentive {
        let caller = alice::<T>();
        ShareholderAccounts::<T>::insert(caller.clone(), Fixed::ONE);
        ClaimableShares::<T>::put(Fixed::ONE);
        let pswap_rewards_account = T::GetTechnicalAccountId::get();
        let pswap_asset_id: T::AssetId = PSWAP.into();
        let pswap_currency = <T::AssetId as Into<<T as tokens::Config>::CurrencyId>>::into(pswap_asset_id);
        let pswap_amount = <T as tokens::Config>::Amount::try_from(balance!(500)).map_err(|_|()).unwrap();
        Tokens::<T>::update_balance(pswap_currency, &pswap_rewards_account, pswap_amount).unwrap();
    }: _(
        RawOrigin::Signed(caller.clone())
    )
    verify {
        assert_eq!(ClaimableShares::<T>::get(), fixed!(0));
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
            assert_ok!(test_benchmark_claim_incentive::<Runtime>());
        });
    }
}