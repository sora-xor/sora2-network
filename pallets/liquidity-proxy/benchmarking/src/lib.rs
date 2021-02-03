//! Liquidity Proxy benchmarking module.

#![cfg_attr(not(feature = "std"), no_std)]

#[macro_use]
extern crate alloc;

use liquidity_proxy::*;

use codec::Decode;
use common::{
    fixed,
    prelude::{Balance, SwapAmount},
    AssetSymbol, DEXId, FilterMode, DOT, XOR,
};
use frame_benchmarking::benchmarks;
use frame_support::traits::Get;
use frame_system::RawOrigin;
use hex_literal::hex;
use permissions::{BURN, MINT};
use sp_std::prelude::*;

use assets::Module as Assets;
use mock_liquidity_source::Module as MockLiquiditySource;
use permissions::Module as Permissions;
use pool_xyk::Module as XYKPool;
use technical::Module as Technical;
use trading_pair::Module as TradingPair;

pub const DEX: DEXId = DEXId::Polkaswap;

#[cfg(test)]
mod mock;

pub struct Module<T: Trait>(liquidity_proxy::Module<T>);
pub trait Trait:
    liquidity_proxy::Trait
    + pool_xyk::Trait
    + mock_liquidity_source::Trait<mock_liquidity_source::Instance1>
    + mock_liquidity_source::Trait<mock_liquidity_source::Instance2>
    + mock_liquidity_source::Trait<mock_liquidity_source::Instance3>
    + mock_liquidity_source::Trait<mock_liquidity_source::Instance4>
{
}

// Support Functions
fn alice<T: Trait>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).unwrap_or_default()
}

// Prepare Runtime for running benchmarks
fn setup_benchmark<T: Trait>() -> Result<(), &'static str> {
    let owner = alice::<T>();
    let owner_origin: <T as frame_system::Trait>::Origin = RawOrigin::Signed(owner.clone()).into();

    // Grant permissions to self in case they haven't been explicitly given in genesis config
    Permissions::<T>::grant_permission(owner.clone(), owner.clone(), MINT)?;
    Permissions::<T>::grant_permission(owner.clone(), owner.clone(), BURN)?;

    let _ = Assets::<T>::register_asset_id(
        owner.clone(),
        XOR.into(),
        AssetSymbol(b"XOR".to_vec()),
        18,
        Balance::from(0u32),
        true,
    );
    let _ = Assets::<T>::register_asset_id(
        owner.clone(),
        DOT.into(),
        AssetSymbol(b"DOT".to_vec()),
        18,
        Balance::from(0u32),
        true,
    );

    TradingPair::<T>::register(owner_origin.clone(), DEX.into(), XOR.into(), DOT.into())?;

    let (_, tech_acc_id, _fee_acc_id, mark_asset) =
        XYKPool::<T>::initialize_pool_unchecked(owner.clone(), DEX.into(), XOR.into(), DOT.into())?;

    let _ = Assets::<T>::register_asset_id(
        owner.clone(),
        mark_asset.clone().into(),
        AssetSymbol(b"PSWAP".to_vec()),
        18,
        Balance::from(0u32),
        true,
    );

    let repr: T::AccountId = Technical::<T>::tech_account_id_to_account_id(&tech_acc_id).unwrap();

    Permissions::<T>::grant_permission(owner.clone(), repr.clone(), MINT)?;
    Permissions::<T>::grant_permission(owner.clone(), repr.clone(), BURN)?;

    Assets::<T>::mint(
        owner_origin.clone(),
        XOR.into(),
        owner.clone(),
        fixed!(10000),
    )?;
    Assets::<T>::mint(
        owner_origin.clone(),
        DOT.into(),
        owner.clone(),
        fixed!(20000),
    )?;
    Assets::<T>::mint(
        owner_origin.clone(),
        XOR.into(),
        repr.clone(),
        fixed!(1000000),
    )?;
    Assets::<T>::mint(
        owner_origin.clone(),
        DOT.into(),
        repr.clone(),
        fixed!(1500000),
    )?;
    Assets::<T>::mint(
        owner_origin.clone(),
        mark_asset.into(),
        owner.clone(),
        fixed!(1500000000000),
    )?;

    // Adding reserves to mock sources
    // We don't want mock sources to contribute into an actual swap but still need to
    // include them in calculation of the optimal exchange path
    // Hence large imbalance in mock sources reserves (to ensure 100% of a swap likely go to XYKPool)
    MockLiquiditySource::<T, mock_liquidity_source::Instance1>::set_reserve(
        owner_origin.clone(),
        DEX.into(),
        DOT.into(),
        fixed!(10000000000000),
        fixed!(11000),
    )?;
    MockLiquiditySource::<T, mock_liquidity_source::Instance2>::set_reserve(
        owner_origin.clone(),
        DEX.into(),
        DOT.into(),
        fixed!(11000000000000),
        fixed!(14000),
    )?;
    MockLiquiditySource::<T, mock_liquidity_source::Instance3>::set_reserve(
        owner_origin.clone(),
        DEX.into(),
        DOT.into(),
        fixed!(8000000000000),
        fixed!(8000),
    )?;
    MockLiquiditySource::<T, mock_liquidity_source::Instance4>::set_reserve(
        owner_origin.clone(),
        DEX.into(),
        DOT.into(),
        fixed!(26000000000000),
        fixed!(36000),
    )?;

    Ok(())
}

benchmarks! {
    // These are the common parameters along with their instancing.
    _ {}

    swap_exact_input {
        let u in 0 .. 1000 => setup_benchmark::<T>()?;
        let caller = alice::<T>();
        let base_asset: T::AssetId = <T as assets::Trait>::GetBaseAssetId::get();
        let target_asset: T::AssetId = DOT.into();
        let initial_base_balance = Assets::<T>::free_balance(&base_asset, &caller).unwrap();
    }: swap(
        RawOrigin::Signed(caller.clone()),
        DEX.into(),
        base_asset.clone(),
        target_asset.clone(),
        SwapAmount::with_desired_input(fixed!(1000), fixed!(0)),
        Vec::new(),
        FilterMode::Disabled
    )
    verify {
        assert_eq!(
            Into::<u128>::into(Assets::<T>::free_balance(&base_asset, &caller).unwrap()),
            Into::<u128>::into(initial_base_balance) - 1_000_u128
        );
    }

    swap_exact_output {
        let u in 0 .. 1000 => setup_benchmark::<T>()?;
        let caller = alice::<T>();
        let base_asset: T::AssetId = <T as assets::Trait>::GetBaseAssetId::get();
        let target_asset: T::AssetId = DOT.into();
        let initial_target_balance = Assets::<T>::free_balance(&target_asset, &caller).unwrap();
    }: swap(
        RawOrigin::Signed(caller.clone()),
        DEX.into(),
        base_asset.clone(),
        target_asset.clone(),
        SwapAmount::with_desired_output(fixed!(1000), fixed!(10000)),
        Vec::new(),
        FilterMode::Disabled
    )
    verify {
        assert_eq!(
            Into::<u128>::into(Assets::<T>::free_balance(&target_asset, &caller).unwrap()),
            Into::<u128>::into(initial_target_balance) + 1_000_u128
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
            assert_ok!(test_benchmark_swap_exact_input::<Runtime>());
            assert_ok!(test_benchmark_swap_exact_output::<Runtime>());
        });
    }
}
