//! Liquidity Proxy benchmarking module.

#![cfg_attr(not(feature = "std"), no_std)]

#[macro_use]
extern crate alloc;

use liquidity_proxy::*;

use bonding_curve_pool::{DistributionAccountData, DistributionAccounts};
use codec::Decode;
use common::prelude::{Balance, SwapAmount};
use common::{
    balance, fixed, AssetName, AssetSymbol, DEXId, FilterMode, LiquiditySource,
    LiquiditySourceFilter, LiquiditySourceType, TechPurpose, DOT, PSWAP, USDT, VAL, XOR,
};
use frame_benchmarking::benchmarks;
use frame_support::traits::Get;
use frame_system::RawOrigin;
use hex_literal::hex;
use permissions::{BURN, MINT};
use sp_std::prelude::*;

use assets::Module as Assets;
use multicollateral_bonding_curve_pool::Module as MBCPool;
use permissions::Module as Permissions;
use pool_xyk::Module as XYKPool;
use technical::Module as Technical;
use trading_pair::Module as TradingPair;

pub const DEX: DEXId = DEXId::Polkaswap;

#[cfg(test)]
mod mock;

pub struct Module<T: Config>(liquidity_proxy::Module<T>);
pub trait Config:
    liquidity_proxy::Config + pool_xyk::Config + multicollateral_bonding_curve_pool::Config
{
}

// Support Functions
fn alice<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).unwrap_or_default()
}

// Prepare Runtime for running benchmarks
fn setup_benchmark<T: Config>() -> Result<(), &'static str> {
    let owner = alice::<T>();
    let owner_origin: <T as frame_system::Config>::Origin = RawOrigin::Signed(owner.clone()).into();

    // Grant permissions to self in case they haven't been explicitly given in genesis config
    Permissions::<T>::grant_permission(owner.clone(), owner.clone(), MINT)?;
    Permissions::<T>::grant_permission(owner.clone(), owner.clone(), BURN)?;

    TradingPair::<T>::register(owner_origin.clone(), DEX.into(), XOR.into(), DOT.into())?;
    TradingPair::<T>::register(owner_origin.clone(), DEX.into(), XOR.into(), USDT.into())?;
    TradingPair::<T>::register(owner_origin.clone(), DEX.into(), XOR.into(), VAL.into())?;
    TradingPair::<T>::register(owner_origin.clone(), DEX.into(), XOR.into(), PSWAP.into())?;

    XYKPool::<T>::initialize_pool(owner_origin.clone(), DEX.into(), XOR.into(), DOT.into())?;
    XYKPool::<T>::initialize_pool(owner_origin.clone(), DEX.into(), XOR.into(), VAL.into())?;
    XYKPool::<T>::initialize_pool(owner_origin.clone(), DEX.into(), XOR.into(), PSWAP.into())?;

    XYKPool::<T>::deposit_liquidity(
        owner_origin.clone(),
        DEX.into(),
        XOR.into(),
        DOT.into(),
        balance!(1000),
        balance!(2000),
        balance!(0),
        balance!(0),
    )?;

    XYKPool::<T>::deposit_liquidity(
        owner_origin.clone(),
        DEX.into(),
        XOR.into(),
        VAL.into(),
        balance!(1000),
        balance!(2000),
        balance!(0),
        balance!(0),
    )?;

    XYKPool::<T>::deposit_liquidity(
        owner_origin.clone(),
        DEX.into(),
        XOR.into(),
        PSWAP.into(),
        balance!(1000),
        balance!(2000),
        balance!(0),
        balance!(0),
    )?;

    MBCPool::<T>::initialize_pool(owner_origin.clone(), USDT.into())?;
    MBCPool::<T>::initialize_pool(owner_origin.clone(), VAL.into())?;

    assert!(MBCPool::<T>::can_exchange(
        &DEXId::Polkaswap.into(),
        &USDT.into(),
        &XOR.into()
    ));

    Ok(())
}

benchmarks! {
    swap_exact_input {
        let u in 0 .. 1000 => setup_benchmark::<T>()?;
        let caller = alice::<T>();
        let base_asset: T::AssetId = <T as assets::Config>::GetBaseAssetId::get();
        let target_asset: T::AssetId = DOT.into();
        let initial_base_balance = Assets::<T>::free_balance(&base_asset, &caller).unwrap();
    }: swap(
        RawOrigin::Signed(caller.clone()),
        DEX.into(),
        base_asset.clone(),
        target_asset.clone(),
        SwapAmount::with_desired_input(balance!(100), 0),
        Vec::new(),
        FilterMode::Disabled
    )
    verify {
        assert_eq!(
            Into::<u128>::into(Assets::<T>::free_balance(&base_asset, &caller).unwrap()),
            Into::<u128>::into(initial_base_balance) - balance!(100)
        );
    }

    swap_exact_output {
        let u in 0 .. 1000 => setup_benchmark::<T>()?;
        let caller = alice::<T>();
        let base_asset: T::AssetId = <T as assets::Config>::GetBaseAssetId::get();
        let target_asset: T::AssetId = DOT.into();
        let initial_target_balance = Assets::<T>::free_balance(&target_asset, &caller).unwrap();
    }: swap(
        RawOrigin::Signed(caller.clone()),
        DEX.into(),
        base_asset.clone(),
        target_asset.clone(),
        SwapAmount::with_desired_output(balance!(100), balance!(100)),
        Vec::new(),
        FilterMode::Disabled
    )
    verify {
        assert_eq!(
            Into::<u128>::into(Assets::<T>::free_balance(&target_asset, &caller).unwrap()),
            Into::<u128>::into(initial_target_balance) + balance!(100)
        );
    }
}

// swap_exact_input_multiple {
//     let u in 0 .. 1000 => setup_benchmark::<T>()?;
//     let caller = alice::<T>();
//     let from_asset: T::AssetId = VAL.into();
//     let to_asset: T::AssetId = XOR.into();
//     let initial_from_balance = Assets::<T>::free_balance(&from_asset, &caller).unwrap();
// }: swap(
//     RawOrigin::Signed(caller.clone()),
//     DEX.into(),
//     from_asset.clone(),
//     to_asset.clone(),
//     SwapAmount::with_desired_input(balance!(10), 0),
//     vec![LiquiditySourceType::XYKPool],
//     FilterMode::ForbidSelected
// )
// verify {
//     assert_eq!(
//         Into::<u128>::into(Assets::<T>::free_balance(&from_asset, &caller).unwrap()),
//         Into::<u128>::into(initial_from_balance) - balance!(100)
//     );
// }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::{ExtBuilder, Runtime};
    use frame_support::assert_ok;

    #[test]
    fn test_benchmarks() {
        ExtBuilder::default().build().execute_with(|| {
            assert_ok!(test_benchmark_swap_exact_input::<Runtime>());
            assert_ok!(test_benchmark_swap_exact_output::<Runtime>());
            // assert_ok!(test_benchmark_swap_exact_input_multiple::<Runtime>());
        });
    }
}
