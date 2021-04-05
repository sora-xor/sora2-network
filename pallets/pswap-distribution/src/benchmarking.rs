//! PSWAP distribution module benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use codec::Decode;
use frame_benchmarking::benchmarks;
use frame_system::RawOrigin;
use hex_literal::hex;
use sp_std::prelude::*;
use traits::MultiCurrencyExtended;

use common::fixnum::ops::One;
use common::{balance, Fixed, PSWAP};

use sp_std::convert::TryFrom;
use tokens::Pallet as Tokens;

// Support Functions
fn alice<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).unwrap_or_default()
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
