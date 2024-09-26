#![cfg(feature = "runtime-benchmarks")]

use super::*;

use codec::Decode;
use common::{
    balance, AssetInfoProvider, AssetManager, AssetName, AssetSymbol, DEXId, PriceToolsProvider,
    APOLLO_ASSET_ID, CERES_ASSET_ID, DAI, DEFAULT_BALANCE_PRECISION, DOT, XOR,
};
use frame_benchmarking::benchmarks;
use frame_support::traits::Hooks;
use frame_system::{EventRecord, RawOrigin};
use hex_literal::hex;
use sp_runtime::traits::AccountIdConversion;
use sp_std::prelude::*;

use crate::Pallet as ApolloPlatform;
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
        frame_system::Pallet::<T>::on_finalize(frame_system::Pallet::<T>::block_number());
        frame_system::Pallet::<T>::set_block_number(
            frame_system::Pallet::<T>::block_number() + 1u32.into(),
        );
        frame_system::Pallet::<T>::on_initialize(frame_system::Pallet::<T>::block_number());
        ApolloPlatform::<T>::on_initialize(frame_system::Pallet::<T>::block_number());
    }
}

fn setup_benchmark<T: Config>() -> Result<(), &'static str> {
    let owner = alice::<T>();
    let pallet_account: AccountIdOf<T> = PalletId(*b"apollolb").into_account_truncating();
    let owner_origin: <T as frame_system::Config>::RuntimeOrigin =
        RawOrigin::Signed(owner.clone()).into();
    let xor_owner: T::AccountId =
        <T as liquidity_proxy::Config>::AssetInfoProvider::get_asset_owner(&XOR.into()).unwrap();
    let dai_owner: T::AccountId =
        <T as liquidity_proxy::Config>::AssetInfoProvider::get_asset_owner(&DAI.into()).unwrap();
    let ceres_owner: T::AccountId =
        <T as liquidity_proxy::Config>::AssetInfoProvider::get_asset_owner(&CERES_ASSET_ID.into())
            .unwrap();
    let apollo_owner: T::AccountId =
        <T as liquidity_proxy::Config>::AssetInfoProvider::get_asset_owner(&APOLLO_ASSET_ID.into())
            .unwrap();

    // Register assets
    T::AssetManager::register_asset_id(
        owner.clone(),
        DOT.into(),
        AssetSymbol(b"DOT".to_vec()),
        AssetName(b"Polkadot".to_vec()),
        DEFAULT_BALANCE_PRECISION,
        Balance::from(0u32),
        true,
        common::AssetType::Regular,
        None,
        None,
    )
    .unwrap();

    // Mint assets to Alice
    T::AssetManager::mint(
        RawOrigin::Signed(xor_owner).into(),
        XOR.into(),
        owner.clone(),
        balance!(1000),
    )
    .unwrap();

    T::AssetManager::mint(
        owner_origin.clone(),
        DOT.into(),
        owner.clone(),
        balance!(500),
    )
    .unwrap();

    T::AssetManager::mint(
        RawOrigin::Signed(dai_owner).into(),
        DAI.into(),
        owner.clone(),
        balance!(500),
    )
    .unwrap();

    T::AssetManager::mint(
        RawOrigin::Signed(apollo_owner.clone()).into(),
        APOLLO_ASSET_ID.into(),
        owner.clone(),
        balance!(500),
    )
    .unwrap();

    T::AssetManager::mint(
        RawOrigin::Signed(apollo_owner).into(),
        APOLLO_ASSET_ID.into(),
        pallet_account,
        balance!(100000),
    )
    .unwrap();

    T::AssetManager::mint(
        RawOrigin::Signed(ceres_owner).into(),
        CERES_ASSET_ID.into(),
        owner,
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
        owner_origin,
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
        PriceTools::<T>::average_prices_calculation_routine();
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

        setup_benchmark::<T>()?;

        ApolloPlatform::<T>::add_pool(
            RawOrigin::Signed(caller).into(),
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
        assert_last_event::<T>(Event::Lent(alice, asset_id.into(), lending_amount).into());
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

        let collateral_amount = balance!(99.009900990099009999);
        let borrow_amount = balance!(100);

        setup_benchmark::<T>()?;

        let xor_owner: T::AccountId = <T as liquidity_proxy::Config>::AssetInfoProvider::get_asset_owner(&XOR.into()).unwrap();

        T::AssetManager::mint(
            RawOrigin::Signed(alice.clone()).into(),
            DOT.into(),
            alice.clone(),
            balance!(200)
        ).unwrap();

        T::AssetManager::mint(
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
            RawOrigin::Signed(caller).into(),
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
            RawOrigin::Signed(bob).into(),
            XOR.into(),
            lending_amount_bob
        ).unwrap();

    }: {
        ApolloPlatform::<T>::borrow(
            RawOrigin::Signed(alice.clone()).into(),
            DOT.into(),
            XOR.into(),
            borrow_amount,
            loan_to_value
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

        let lending_amount = balance!(50);

        setup_benchmark::<T>()?;

        ApolloPlatform::<T>::add_pool(
            RawOrigin::Signed(caller).into(),
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
            asset_id.into(),
            lending_amount
        ).unwrap();

        run_to_block::<T>(151);

    }: {
        ApolloPlatform::<T>::get_rewards(RawOrigin::Signed(alice.clone()).into(), asset_id.into(), true).unwrap()
    } verify {
        assert_last_event::<T>(Event::ClaimedLendingRewards(alice, asset_id.into(), balance!(5.74581425)).into());
    }

    withdraw {
        let caller = pallet::AuthorityAccount::<T>::get();
        let alice = alice::<T>();
        let bob = bob::<T>();
        let asset_id_xor = XOR;
        let loan_to_value = balance!(1);
        let liquidation_threshold = balance!(0.1);
        let optimal_utilization_rate = balance!(1);
        let base_rate = balance!(1);
        let slope_rate_1 = balance!(1);
        let slope_rate_2 = balance!(1);
        let reserve_factor = balance!(1);

        let lending_amount_alice = balance!(50);
        let lending_amount_bob = balance!(200000);

        setup_benchmark::<T>()?;

        let xor_owner: T::AccountId = <T as liquidity_proxy::Config>::AssetInfoProvider::get_asset_owner(&XOR.into()).unwrap();

        T::AssetManager::mint(
            RawOrigin::Signed(xor_owner).into(),
            XOR.into(),
            bob.clone(),
            balance!(300000)
        ).unwrap();

        ApolloPlatform::<T>::add_pool(
            RawOrigin::Signed(caller).into(),
            asset_id_xor.into(),
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
            asset_id_xor.into(),
            lending_amount_alice
        ).unwrap();

        ApolloPlatform::<T>::lend(
            RawOrigin::Signed(bob).into(),
            asset_id_xor.into(),
            lending_amount_bob
        ).unwrap();

        run_to_block::<T>(101);
    }: {
        ApolloPlatform::<T>::withdraw(
            RawOrigin::Signed(alice.clone()).into(),
            asset_id_xor.into(),
            lending_amount_alice
        ).unwrap()
    } verify {
        assert_last_event::<T>(Event::Withdrawn(alice, XOR.into(), lending_amount_alice).into());
    }

    repay {
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
        let reserve_factor_0 = balance!(0.1);
        let reserve_factor = balance!(1);

        let lending_amount_alice = balance!(300);
        let lending_amount_bob = balance!(200000);
        let borrow_amount = balance!(100);
        let amount_to_repay = balance!(500);

        setup_benchmark::<T>()?;

        let xor_owner: T::AccountId = <T as liquidity_proxy::Config>::AssetInfoProvider::get_asset_owner(&XOR.into()).unwrap();

        T::AssetManager::mint(
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
            reserve_factor_0,
        ).unwrap();

        ApolloPlatform::<T>::add_pool(
            RawOrigin::Signed(caller).into(),
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
            RawOrigin::Signed(bob).into(),
            XOR.into(),
            lending_amount_bob
        ).unwrap();

        ApolloPlatform::<T>::borrow(
            RawOrigin::Signed(alice.clone()).into(),
            DOT.into(),
            XOR.into(),
            borrow_amount,
            loan_to_value
        ).unwrap();

        run_to_block::<T>(151);
    }: {
        ApolloPlatform::<T>::repay(
            RawOrigin::Signed(alice.clone()).into(),
            DOT.into(),
            XOR.into(),
            amount_to_repay
        ).unwrap()
    } verify {
        assert_last_event::<T>(Event::Repaid(alice, XOR.into(), 100002874343607297800).into());
    }

    change_rewards_amount {
        let caller = pallet::AuthorityAccount::<T>::get();
    }: {
        ApolloPlatform::<T>::change_rewards_amount(
            RawOrigin::Signed(caller.clone()).into(),
            true,
            balance!(1)
        ).unwrap()
    } verify {
        assert_last_event::<T>(Event::ChangedRewardsAmount(caller, true, balance!(1)).into());
    }

    change_rewards_per_block {
        let caller = pallet::AuthorityAccount::<T>::get();
        let asset_id_xor = XOR;
        let loan_to_value = balance!(1);
        let liquidation_threshold = balance!(0.1);
        let optimal_utilization_rate = balance!(1);
        let base_rate = balance!(1);
        let slope_rate_1 = balance!(1);
        let slope_rate_2 = balance!(1);
        let reserve_factor = balance!(1);

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
    }: {
        ApolloPlatform::<T>::change_rewards_per_block(
            RawOrigin::Signed(caller.clone()).into(),
            true,
            balance!(1)
        ).unwrap()
    } verify {
        assert_last_event::<T>(Event::ChangedRewardsAmountPerBlock(caller, true, balance!(1)).into());
    }

    liquidate {
        let caller = pallet::AuthorityAccount::<T>::get();
        let alice = alice::<T>();
        let bob = bob::<T>();
        let asset_id_xor = XOR;
        let asset_id_dot = DOT;
        let loan_to_value = balance!(1);
        let liquidation_threshold = balance!(0.1);
        let optimal_utilization_rate = balance!(1);
        let base_rate = balance!(1);
        let slope_rate_1 = balance!(1);
        let slope_rate_2 = balance!(1);
        let reserve_factor = balance!(0.1);

        let lending_amount_alice = balance!(300);
        let lending_amount_bob = balance!(200000);
        let borrow_amount = balance!(100);

        setup_benchmark::<T>()?;

        let xor_owner: T::AccountId = <T as liquidity_proxy::Config>::AssetInfoProvider::get_asset_owner(&XOR.into()).unwrap();

        T::AssetManager::mint(
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
            RawOrigin::Signed(bob).into(),
            XOR.into(),
            lending_amount_bob
        ).unwrap();

        ApolloPlatform::<T>::borrow(
            RawOrigin::Signed(alice.clone()).into(),
            DOT.into(),
            XOR.into(),
            borrow_amount,
            loan_to_value
        ).unwrap();
    }: {
        ApolloPlatform::<T>::liquidate(
            RawOrigin::Signed(caller.clone()).into(),
            alice.clone(),
            XOR.into()
        ).unwrap()
    } verify {
        assert_last_event::<T>(Event::Liquidated(alice, XOR.into()).into());
    }

    remove_pool {
        let caller = pallet::AuthorityAccount::<T>::get();
        let asset_id = XOR;
        let loan_to_value = balance!(1);
        let liquidation_threshold = balance!(1);
        let optimal_utilization_rate = balance!(1);
        let base_rate = balance!(1);
        let slope_rate_1 = balance!(1);
        let slope_rate_2 = balance!(1);
        let reserve_factor = balance!(1);

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
         ApolloPlatform::<T>::remove_pool(
            RawOrigin::Signed(caller.clone()).into(),
            asset_id.into()
        ).unwrap()
    }
    verify {
        assert_last_event::<T>(Event::PoolRemoved(caller, asset_id.into()).into());
    }

    edit_pool_info {
        let caller = pallet::AuthorityAccount::<T>::get();
        let asset_id = XOR;
        let initial_parameter_value = balance!(1);
        let edit_parameter_value = balance!(0.8);

        ApolloPlatform::<T>::add_pool(
            RawOrigin::Signed(caller.clone()).into(),
            asset_id.into(),
            initial_parameter_value,
            initial_parameter_value,
            initial_parameter_value,
            initial_parameter_value,
            initial_parameter_value,
            initial_parameter_value,
            initial_parameter_value,
        ).unwrap();
    }: {
         ApolloPlatform::<T>::edit_pool_info(
            RawOrigin::Signed(caller.clone()).into(),
            asset_id.into(),
            edit_parameter_value,
            edit_parameter_value,
            edit_parameter_value,
            edit_parameter_value,
            edit_parameter_value,
            edit_parameter_value,
            edit_parameter_value,
            edit_parameter_value,
            edit_parameter_value,
            edit_parameter_value,
        ).unwrap()
    }
    verify {
        assert_last_event::<T>(Event::PoolInfoEdited(caller, asset_id.into()).into());
    }

    add_collateral {
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

        let collateral_amount = balance!(99.009900990099009999);
        let borrow_amount = balance!(100);
        let additional_collateral_amount = balance!(2);

        setup_benchmark::<T>()?;

        let xor_owner: T::AccountId = <T as liquidity_proxy::Config>::AssetInfoProvider::get_asset_owner(&XOR.into()).unwrap();

        T::AssetManager::mint(
            RawOrigin::Signed(alice.clone()).into(),
            DOT.into(),
            alice.clone(),
            balance!(200)
        ).unwrap();

        T::AssetManager::mint(
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
            RawOrigin::Signed(caller).into(),
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
            RawOrigin::Signed(bob).into(),
            XOR.into(),
            lending_amount_bob
        ).unwrap();

        ApolloPlatform::<T>::borrow(
            RawOrigin::Signed(alice.clone()).into(),
            DOT.into(),
            XOR.into(),
            borrow_amount,
            loan_to_value
        ).unwrap();
    }: {
        ApolloPlatform::<T>::add_collateral(
          RawOrigin::Signed(alice.clone()).into(),
           DOT.into(),
          additional_collateral_amount,
           XOR.into(),
        ).unwrap()
    }
    verify {
        assert_last_event::<T>(Event::CollateralAdded(alice, DOT.into(), additional_collateral_amount, XOR.into()).into());
    }
}
