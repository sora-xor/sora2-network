//! PSWAP distribution module benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use codec::Decode;
use frame_benchmarking::benchmarks;
use frame_system::RawOrigin;
use frame_support::traits::OnInitialize;
use hex_literal::hex;
use sp_std::prelude::*;
use traits::MultiCurrencyExtended;
use sp_io::hashing::blake2_256;
use sp_core::H256;
use sp_core::H512;

use common::{FromGenericPair, TechAccountId, fixnum::ops::One};
use common::{balance, Fixed, PSWAP, AssetName, AssetSymbol, XOR};

use sp_std::convert::TryFrom;
use tokens::Pallet as Tokens;
use technical::Pallet as Technical;
use assets::Pallet as Assets;
use permissions::Pallet as Permissions;

// Support Functions
fn alice<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).unwrap_or_default()
}

fn create_account<T: Config>(prefix: Vec<u8>, index: u128) -> T::AccountId {
    let tech_account: T::TechAccountId = T::TechAccountId::from_generic_pair(prefix, index.encode());
    Technical::<T>::tech_account_id_to_account_id(&tech_account).unwrap()
}

fn create_asset<T: Config>(prefix: Vec<u8>, index: u128) -> T::AssetId {
    let entropy: [u8; 32] = (prefix, index).using_encoded(blake2_256);
    T::AssetId::from(H256(entropy))
}

fn prepare_for_distribution<T: Config>(distribution_freq: u32) {
    let authority = alice::<T>();
    let _ = Permissions::<T>::assign_permission(
        authority.clone(),
        &authority,
        permissions::MINT,
        permissions::Scope::Unlimited,
    );
    for i in 1u128..10 {
        let pool_account = create_account::<T>(b"pool".to_vec(), i);
        let pool_asset = create_asset::<T>(b"pool".to_vec(), i);
        Assets::<T>::register_asset_id(authority.clone(), pool_asset.clone(), AssetSymbol(b"POOL".to_vec()), AssetName(b"POOL".to_vec()), 18, balance!(0), true).unwrap();
        Assets::<T>::mint_to(&PSWAP.into(), &authority, &pool_account, balance!(1000));
        Pallet::<T>::subscribe(pool_account.clone(), common::DEXId::Polkaswap.into(), pool_asset.clone(), Some(distribution_freq.into())).unwrap();
        for j in 1u128..1000 {
            let liquidity_provider = create_account::<T>(b"liquidity_provider".to_vec(), j);
            Assets::<T>::mint_to(&pool_asset, &authority, &liquidity_provider, balance!(100));
        }
    }
}

fn validate_distribution<T: Config>() {
    for i in 1u128..10 {
        let pool_asset = create_asset::<T>(b"pool".to_vec(), i);
        for j in 1u128..1000 {
            let liquidity_provider = create_account::<T>(b"liquidity_provider".to_vec(), j);
            Pallet::<T>::claim_incentive(RawOrigin::Signed(liquidity_provider.clone()).into());
            assert_eq!(Assets::<T>::free_balance(&pool_asset, &liquidity_provider).unwrap(), balance!(100));
            assert!(Assets::<T>::free_balance(&PSWAP.into(), &liquidity_provider).unwrap() > balance!(0));
        }
    }
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

    on_initialize_intensive {
        let distribution_freq = 15u32;
        prepare_for_distribution::<T>(distribution_freq);
    }: {
        Pallet::<T>::on_initialize(distribution_freq.into());
    }
    verify {
        validate_distribution::<T>();
    }

    on_initialize_regular {
        let distribution_freq = 15u32;
        prepare_for_distribution::<T>(distribution_freq - 1u32);
    }: {
        Pallet::<T>::on_initialize(distribution_freq.into());
    }
    verify {
        // nothing but checks is performed
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
            assert_ok!(test_benchmark_on_initialize_regular::<Runtime>());
            assert_ok!(test_benchmark_on_initialize_intensive::<Runtime>());
        });
    }
}
