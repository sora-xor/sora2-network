#![cfg(feature = "runtime-benchmarks")]

use super::*;

use codec::Decode;
use common::{
    balance, AssetName, AssetSymbol, DEXId, PriceToolsPallet, PriceVariant, APOLLO_ASSET_ID,
    CERES_ASSET_ID, DAI, DEFAULT_BALANCE_PRECISION, DOT, XOR,
};
use frame_benchmarking::benchmarks;
use frame_support::traits::Hooks;
use frame_system::{EventRecord, RawOrigin};
use hex_literal::hex;
use sp_runtime::traits::AccountIdConversion;
use sp_std::prelude::*;

use crate::Pallet as ApolloPlatform;
use assets::Pallet as Assets;
use frame_support::PalletId;
use pool_xyk::Pallet as XYKPool;
use price_tools::Pallet as PriceTools;
use trading_pair::Pallet as TradingPair;

pub const DEX: DEXId = DEXId::Polkaswap;

// Support functions
fn alice<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).unwrap()
}

fn bob<T: Config>() -> T::AccountId {
    let bytes = hex!("8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48");
    T::AccountId::decode(&mut &bytes[..]).unwrap()
}

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
    let events = frame_system::Pallet::<T>::events();
    let system_event: <T as frame_system::Config>::RuntimeEvent = generic_event.into();
    // compare to the last event record
    let EventRecord { event, .. } = &events[events.len() - 1];
    assert_eq!(event, &system_event);
}

fn run_to_block<T: Config>(n: u32) {
    while frame_system::Pallet::<T>::block_number() < n.into() {
        frame_system::Pallet::<T>::on_finalize(frame_system::Pallet::<T>::block_number().into());
        frame_system::Pallet::<T>::set_block_number(
            frame_system::Pallet::<T>::block_number() + 1u32.into(),
        );
        frame_system::Pallet::<T>::on_initialize(frame_system::Pallet::<T>::block_number().into());
        ApolloPlatform::<T>::on_initialize(frame_system::Pallet::<T>::block_number().into());
    }
}

fn setup_benchmark<T: Config>() -> Result<(), &'static str> {
    let owner = alice::<T>();
    let pallet_account: AccountIdOf<T> = PalletId(*b"apollolb").into_account_truncating();
    let owner_origin: <T as frame_system::Config>::RuntimeOrigin =
        RawOrigin::Signed(owner.clone()).into();
    let xor_owner: T::AccountId = assets::AssetOwners::<T>::get::<T::AssetId>(XOR.into()).unwrap();
    let dai_owner: T::AccountId = assets::AssetOwners::<T>::get::<T::AssetId>(DAI.into()).unwrap();

    // Register assets
    Assets::<T>::register_asset_id(
        owner.clone(),
        DOT.into(),
        AssetSymbol(b"DOT".to_vec()),
        AssetName(b"Polkadot".to_vec()),
        DEFAULT_BALANCE_PRECISION,
        Balance::from(0u32),
        true,
        None,
        None,
    )
    .unwrap();

    Assets::<T>::register_asset_id(
        owner.clone(),
        APOLLO_ASSET_ID.into(),
        AssetSymbol(b"APOLLO".to_vec()),
        AssetName(b"Apollo".to_vec()),
        DEFAULT_BALANCE_PRECISION,
        Balance::from(0u32),
        true,
        None,
        None,
    )
    .unwrap();

    Assets::<T>::register_asset_id(
        owner.clone(),
        CERES_ASSET_ID.into(),
        AssetSymbol(b"CERES".to_vec()),
        AssetName(b"Ceres".to_vec()),
        DEFAULT_BALANCE_PRECISION,
        Balance::from(0u32),
        true,
        None,
        None,
    )
    .unwrap();

    // Mint assets to Alice
    Assets::<T>::mint(
        RawOrigin::Signed(xor_owner.clone()).into(),
        XOR.into(),
        owner.clone(),
        balance!(1000),
    )
    .unwrap();

    Assets::<T>::mint(
        owner_origin.clone(),
        DOT.into(),
        owner.clone(),
        balance!(500),
    )
    .unwrap();

    Assets::<T>::mint(
        RawOrigin::Signed(dai_owner.clone()).into(),
        DAI.into(),
        owner.clone(),
        balance!(500),
    )
    .unwrap();

    Assets::<T>::mint(
        owner_origin.clone(),
        APOLLO_ASSET_ID.into(),
        owner.clone(),
        balance!(500),
    )
    .unwrap();

    Assets::<T>::mint(
        owner_origin.clone(),
        APOLLO_ASSET_ID.into(),
        pallet_account.clone(),
        balance!(100000),
    )
    .unwrap();

    Assets::<T>::mint(
        owner_origin.clone(),
        CERES_ASSET_ID.into(),
        owner.clone(),
        balance!(500),
    )
    .unwrap();

    // Register trading pairs
    TradingPair::<T>::register(owner_origin.clone(), DEX.into(), XOR.into(), DOT.into()).unwrap();
    TradingPair::<T>::register(
        owner_origin.clone(),
        DEX.into(),
        XOR.into(),
        APOLLO_ASSET_ID.into(),
    )
    .unwrap();
    TradingPair::<T>::register(
        owner_origin.clone(),
        DEX.into(),
        XOR.into(),
        CERES_ASSET_ID.into(),
    )
    .unwrap();

    // Initialize pools and deposit liquidity
    XYKPool::<T>::initialize_pool(owner_origin.clone(), DEX.into(), XOR.into(), DOT.into())?;
    XYKPool::<T>::initialize_pool(owner_origin.clone(), DEX.into(), XOR.into(), DAI.into())?;
    XYKPool::<T>::initialize_pool(
        owner_origin.clone(),
        DEX.into(),
        XOR.into(),
        APOLLO_ASSET_ID.into(),
    )?;
    XYKPool::<T>::initialize_pool(
        owner_origin.clone(),
        DEX.into(),
        XOR.into(),
        CERES_ASSET_ID.into(),
    )?;

    XYKPool::<T>::deposit_liquidity(
        owner_origin.clone(),
        DEX.into(),
        XOR.into(),
        DOT.into(),
        balance!(100),
        balance!(100),
        balance!(100),
        balance!(100),
    )?;

    XYKPool::<T>::deposit_liquidity(
        owner_origin.clone(),
        DEX.into(),
        XOR.into(),
        DAI.into(),
        balance!(100),
        balance!(100),
        balance!(100),
        balance!(100),
    )?;

    XYKPool::<T>::deposit_liquidity(
        owner_origin.clone(),
        DEX.into(),
        XOR.into(),
        APOLLO_ASSET_ID.into(),
        balance!(100),
        balance!(100),
        balance!(100),
        balance!(100),
    )?;

    XYKPool::<T>::deposit_liquidity(
        owner_origin.clone(),
        DEX.into(),
        XOR.into(),
        CERES_ASSET_ID.into(),
        balance!(100),
        balance!(100),
        balance!(100),
        balance!(100),
    )?;

    // Register assets to PriceTools and fill PricesInfos
    PriceTools::<T>::register_asset(&DOT.into()).unwrap();
    for _ in 0..30 {
        PriceTools::<T>::average_prices_calculation_routine(PriceVariant::Buy);
        PriceTools::<T>::average_prices_calculation_routine(PriceVariant::Sell);
    }

    Ok(())
}

benchmarks! {
    add_pool {
        let caller = pallet::AuthorityAccount::<T>::get();
        let asset_id = XOR;
        let loan_to_value = balance!(1);
        let liquidation_threshold = balance!(1);
        let optimal_utilization_rate = balance!(1);
        let base_rate = balance!(1);
        let slope_rate_1 = balance!(1);
        let slope_rate_2 = balance!(1);
        let reserve_factor = balance!(1);
    }: {
         ApolloPlatform::<T>::add_pool(
            RawOrigin::Signed(caller.clone()).into(),
            asset_id.into(),
            loan_to_value,
            liquidation_threshold,
            optimal_utilization_rate,
            base_rate,
            slope_rate_1,
            slope_rate_2,
            reserve_factor,
        ).unwrap()
    }
    verify {
        assert_last_event::<T>(Event::PoolAdded(caller, asset_id.into()).into());
    }

    lend {
        let caller = pallet::AuthorityAccount::<T>::get();
        let alice = alice::<T>();
        let asset_id = XOR;
        let loan_to_value = balance!(1);
        let liquidation_threshold = balance!(1);
        let optimal_utilization_rate = balance!(1);
        let base_rate = balance!(1);
        let slope_rate_1 = balance!(1);
        let slope_rate_2 = balance!(1);
        let reserve_factor = balance!(1);

        let lending_amount = balance!(100);

        let mint = assets::Pallet::<T>::mint_to(
            &XOR.into(),
            &alice,
            &alice,
            balance!(300000)
        );

        ApolloPlatform::<T>::add_pool(
            RawOrigin::Signed(caller.clone()).into(),
            asset_id.into(),
            loan_to_value,
            liquidation_threshold,
            optimal_utilization_rate,
            base_rate,
            slope_rate_1,
            slope_rate_2,
            reserve_factor,
        ).unwrap();
    }: {
        ApolloPlatform::<T>::lend(
            RawOrigin::Signed(alice.clone()).into(),
            XOR.into(),
            lending_amount
        ).unwrap()
    } verify {
        assert_last_event::<T>(Event::Lended(alice, asset_id.into(), lending_amount).into());
    }

    borrow {
        let caller = pallet::AuthorityAccount::<T>::get();
        let alice = alice::<T>();
        let bob = bob::<T>();
        let asset_id_xor = XOR;
        let asset_id_dot = DOT;
        let loan_to_value = balance!(1);
        let liquidation_threshold = balance!(1);
        let optimal_utilization_rate = balance!(1);
        let base_rate = balance!(1);
        let slope_rate_1 = balance!(1);
        let slope_rate_2 = balance!(1);
        let reserve_factor = balance!(1);

        let lending_amount_alice = balance!(300);
        let lending_amount_bob = balance!(200000);

        let collateral_amount = balance!(101.00000000000000001);
        let borrow_amount = balance!(100);

        setup_benchmark::<T>()?;

        let xor_owner: T::AccountId = assets::AssetOwners::<T>::get::<T::AssetId>(XOR.into()).unwrap();

        Assets::<T>::mint(
            RawOrigin::Signed(alice.clone()).into(),
            DOT.into(),
            alice.clone(),
            balance!(200)
        ).unwrap();

        Assets::<T>::mint(
            RawOrigin::Signed(xor_owner).into(),
            XOR.into(),
            bob.clone(),
            balance!(300000)
        ).unwrap();

        ApolloPlatform::<T>::add_pool(
            RawOrigin::Signed(caller.clone()).into(),
            asset_id_xor.into(),
            loan_to_value,
            liquidation_threshold,
            optimal_utilization_rate,
            base_rate,
            slope_rate_1,
            slope_rate_2,
            reserve_factor,
        ).unwrap();

        ApolloPlatform::<T>::add_pool(
            RawOrigin::Signed(caller.clone()).into(),
            asset_id_dot.into(),
            loan_to_value,
            liquidation_threshold,
            optimal_utilization_rate,
            base_rate,
            slope_rate_1,
            slope_rate_2,
            reserve_factor,
        ).unwrap();

        ApolloPlatform::<T>::lend(
            RawOrigin::Signed(alice.clone()).into(),
            DOT.into(),
            lending_amount_alice
        ).unwrap();

        ApolloPlatform::<T>::lend(
            RawOrigin::Signed(bob.clone()).into(),
            XOR.into(),
            lending_amount_bob
        ).unwrap();

    }: {
        ApolloPlatform::<T>::borrow(
            RawOrigin::Signed(alice.clone()).into(),
            DOT.into(),
            XOR.into(),
            borrow_amount
        ).unwrap()
    } verify {
        assert_last_event::<T>(Event::Borrowed(alice, DOT.into(), collateral_amount, XOR.into(), borrow_amount).into());
    }

    get_rewards {
        let caller = pallet::AuthorityAccount::<T>::get();
        let alice = alice::<T>();
        let asset_id = XOR;
        let loan_to_value = balance!(1);
        let liquidation_threshold = balance!(1);
        let optimal_utilization_rate = balance!(1);
        let base_rate = balance!(1);
        let slope_rate_1 = balance!(1);
        let slope_rate_2 = balance!(1);
        let reserve_factor = balance!(1);

        let lending_amount = balance!(300);

        Assets::<T>::mint(
            RawOrigin::Signed(alice.clone()).into(),
            XOR.into(),
            alice.clone(),
            balance!(500)
        ).unwrap();

        ApolloPlatform::<T>::add_pool(
            RawOrigin::Signed(caller.clone()).into(),
            asset_id.into(),
            loan_to_value,
            liquidation_threshold,
            optimal_utilization_rate,
            base_rate,
            slope_rate_1,
            slope_rate_2,
            reserve_factor,
        ).unwrap();

        ApolloPlatform::<T>::lend(
            RawOrigin::Signed(alice.clone()).into(),
            XOR.into(),
            lending_amount
        ).unwrap();

        run_to_block::<T>(150);

    }: {
        ApolloPlatform::<T>::get_rewards(RawOrigin::Signed(alice.clone()).into(), APOLLO_ASSET_ID.into(), true)
    } verify {
        assert_last_event::<T>(Event::ClaimedLendingRewards(alice, APOLLO_ASSET_ID.into(), balance!(20)).into());
    }

    impl_benchmark_test_suite!(
        Pallet,
        crate::mock::ExtBuilder::default().build(),
        crate::mock::Runtime
    );
}
