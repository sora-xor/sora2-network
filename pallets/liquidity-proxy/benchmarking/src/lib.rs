//! Liquidity Proxy benchmarking module.

#![cfg_attr(not(feature = "std"), no_std)]

#[macro_use]
extern crate alloc;

use liquidity_proxy::*;

use codec::Decode;
use common::prelude::{Balance, SwapAmount};
use common::{
    balance, AssetName, AssetSymbol, DEXId, FilterMode, LiquiditySource, LiquiditySourceType, DOT,
    PSWAP, USDT, VAL, XOR,
};
use frame_benchmarking::{benchmarks, Zero};
use frame_support::traits::Get;
use frame_system::RawOrigin;
use hex_literal::hex;
use sp_std::prelude::*;

use assets::Pallet as Assets;
use multicollateral_bonding_curve_pool::Pallet as MBCPool;
use permissions::Pallet as Permissions;
use pool_xyk::Pallet as XYKPool;
use trading_pair::Pallet as TradingPair;

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
    let dex_id: T::DEXId = DEX.into();

    // Grant permissions to self in case they haven't been explicitly given in genesis config
    Permissions::<T>::assign_permission(
        owner.clone(),
        &owner,
        permissions::MANAGE_DEX,
        permissions::Scope::Limited(common::hash(&dex_id)),
    )
    .unwrap();
    let _ = Permissions::<T>::assign_permission(
        owner.clone(),
        &owner,
        permissions::MINT,
        permissions::Scope::Unlimited,
    );
    let _ = Permissions::<T>::assign_permission(
        owner.clone(),
        &owner,
        permissions::BURN,
        permissions::Scope::Unlimited,
    );
    let _ = Assets::<T>::register_asset_id(
        owner.clone(),
        USDT.into(),
        AssetSymbol(b"TESTUSD".to_vec()),
        AssetName(b"USD".to_vec()),
        18,
        Balance::zero(),
        true,
    );
    let _ = Assets::<T>::register_asset_id(
        owner.clone(),
        DOT.into(),
        AssetSymbol(b"TESTDOT".to_vec()),
        AssetName(b"DOT".to_vec()),
        18,
        Balance::zero(),
        true,
    );
    Assets::<T>::mint_to(&XOR.into(), &owner.clone(), &owner.clone(), balance!(50000)).unwrap();
    Assets::<T>::mint_to(
        &DOT.into(),
        &owner.clone(),
        &owner.clone(),
        balance!(50000000),
    )
    .unwrap();
    Assets::<T>::mint_to(
        &USDT.into(),
        &owner.clone(),
        &owner.clone(),
        balance!(50000000),
    )
    .unwrap();
    Assets::<T>::mint_to(
        &VAL.into(),
        &owner.clone(),
        &owner.clone(),
        balance!(50000000),
    )
    .unwrap();
    Assets::<T>::mint_to(
        &PSWAP.into(),
        &owner.clone(),
        &owner.clone(),
        balance!(50000000),
    )
    .unwrap();

    TradingPair::<T>::register(owner_origin.clone(), DEX.into(), XOR.into(), DOT.into()).unwrap();
    TradingPair::<T>::register(owner_origin.clone(), DEX.into(), XOR.into(), USDT.into()).unwrap();
    TradingPair::<T>::register(owner_origin.clone(), DEX.into(), XOR.into(), VAL.into()).unwrap();
    TradingPair::<T>::register(owner_origin.clone(), DEX.into(), XOR.into(), PSWAP.into()).unwrap();

    XYKPool::<T>::initialize_pool(owner_origin.clone(), DEX.into(), XOR.into(), DOT.into())?;
    XYKPool::<T>::initialize_pool(owner_origin.clone(), DEX.into(), XOR.into(), VAL.into())?;
    XYKPool::<T>::initialize_pool(owner_origin.clone(), DEX.into(), XOR.into(), PSWAP.into())?;
    XYKPool::<T>::initialize_pool(owner_origin.clone(), DEX.into(), XOR.into(), USDT.into())?;

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
    XYKPool::<T>::deposit_liquidity(
        owner_origin.clone(),
        DEX.into(),
        XOR.into(),
        USDT.into(),
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
    swap_exact_input_primary_only {
        setup_benchmark::<T>()?;
        let caller = alice::<T>();
        let from_asset: T::AssetId = VAL.into();
        let to_asset: T::AssetId = XOR.into();
        let initial_from_balance = Assets::<T>::free_balance(&from_asset, &caller).unwrap();
    }: swap(
        RawOrigin::Signed(caller.clone()),
        DEX.into(),
        from_asset.clone(),
        to_asset.clone(),
        SwapAmount::with_desired_input(balance!(100), 0),
        [LiquiditySourceType::MulticollateralBondingCurvePool].into(),
        FilterMode::AllowSelected
    )
    verify {
        assert_eq!(
            Into::<u128>::into(Assets::<T>::free_balance(&from_asset, &caller).unwrap()),
            Into::<u128>::into(initial_from_balance) - balance!(100)
        );
    }

    // TODO: resolve slippage exceeded issue
    // swap_exact_output_primary_only {
    //     setup_benchmark::<T>()?;
    //     let caller = alice::<T>();
    //     let from_asset: T::AssetId = VAL.into();
    //     let to_asset: T::AssetId = XOR.into();
    //     let initial_to_balance = Assets::<T>::free_balance(&to_asset, &caller).unwrap();
    // }: swap(
    //     RawOrigin::Signed(caller.clone()),
    //     DEX.into(),
    //     from_asset.clone(),
    //     to_asset.clone(),
    //     SwapAmount::with_desired_output(balance!(100), balance!(10000000)),
    //     [LiquiditySourceType::MulticollateralBondingCurvePool].into(),
    //     FilterMode::AllowSelected
    // )
    // verify {
    //     assert_eq!(
    //         Into::<u128>::into(Assets::<T>::free_balance(&to_asset, &caller).unwrap()),
    //         Into::<u128>::into(initial_to_balance) + balance!(1)
    //     );
    // }

    swap_exact_input_secondary_only {
        setup_benchmark::<T>()?;
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
        [LiquiditySourceType::XYKPool].into(),
        FilterMode::AllowSelected
    )
    verify {
        assert_eq!(
            Into::<u128>::into(Assets::<T>::free_balance(&base_asset, &caller).unwrap()),
            Into::<u128>::into(initial_base_balance) - balance!(100)
        );
    }

    swap_exact_output_secondary_only {
        setup_benchmark::<T>()?;
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
        [LiquiditySourceType::XYKPool].into(),
        FilterMode::AllowSelected
    )
    verify {
        assert_eq!(
            Into::<u128>::into(Assets::<T>::free_balance(&target_asset, &caller).unwrap()),
            Into::<u128>::into(initial_target_balance) + balance!(100)
        );
    }

    swap_exact_input_multiple {
        setup_benchmark::<T>()?;
        let caller = alice::<T>();
        let from_asset: T::AssetId = VAL.into();
        let to_asset: T::AssetId = DOT.into();
        let initial_from_balance = Assets::<T>::free_balance(&from_asset, &caller).unwrap();
    }: swap(
        RawOrigin::Signed(caller.clone()),
        DEX.into(),
        from_asset.clone(),
        to_asset.clone(),
        SwapAmount::with_desired_input(balance!(1), 0),
        Vec::new(),
        FilterMode::Disabled
    )
    verify {
        assert_eq!(
            Into::<u128>::into(Assets::<T>::free_balance(&from_asset, &caller).unwrap()),
            Into::<u128>::into(initial_from_balance) - balance!(1)
        );
    }

    swap_exact_output_multiple {
        setup_benchmark::<T>()?;
        let caller = alice::<T>();
        let from_asset: T::AssetId = VAL.into();
        let to_asset: T::AssetId = DOT.into();
        let initial_to_balance = Assets::<T>::free_balance(&to_asset, &caller).unwrap();
    }: swap(
        RawOrigin::Signed(caller.clone()),
        DEX.into(),
        from_asset.clone(),
        to_asset.clone(),
        SwapAmount::with_desired_output(balance!(1), balance!(10000000)),
        Vec::new(),
        FilterMode::Disabled
    )
    verify {
        assert_eq!(
            Into::<u128>::into(Assets::<T>::free_balance(&to_asset, &caller).unwrap()),
            Into::<u128>::into(initial_to_balance) + balance!(1)
        );
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
            assert_ok!(test_benchmark_swap_exact_input_primary_only::<Runtime>());
            // assert_ok!(test_benchmark_swap_exact_output_primary_only::<Runtime>());
            assert_ok!(test_benchmark_swap_exact_input_secondary_only::<Runtime>());
            assert_ok!(test_benchmark_swap_exact_output_secondary_only::<Runtime>());
            assert_ok!(test_benchmark_swap_exact_input_multiple::<Runtime>());
            assert_ok!(test_benchmark_swap_exact_output_multiple::<Runtime>());
        });
    }
}
