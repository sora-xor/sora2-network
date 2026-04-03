#![cfg(feature = "runtime-benchmarks")]

use super::*;
use common::{
    AssetInfoProvider, AssetManager, AssetName, AssetSymbol, AssetType, DEXId, DEXInfo,
    PriceToolsProvider, XOR,
};
use frame_benchmarking::v2::*;
use frame_support::traits::Get;
use frame_system::RawOrigin;
use sp_runtime::traits::{One, Saturating, Zero};

type BenchBalanceOf<T> = <T as crate::Config>::Balance;
type BenchAssetIdOf<T> = <T as crate::Config>::AssetId;

fn default_condition_input() -> ConditionInput {
    ConditionInput {
        question: b"Will Hydra succeed across all markets?".to_vec(),
        oracle: b"Chainlink".to_vec(),
        resolution_source: b"https://oracle.example.com".to_vec(),
    }
}

fn bench_balance<T>(amount: u32) -> BenchBalanceOf<T>
where
    T: crate::Config,
{
    amount.into()
}

fn mint_canonical_balance<T>(who: &T::AccountId, amount: BenchBalanceOf<T>)
where
    T: crate::Config + frame_system::Config,
{
    if amount.is_zero() {
        return;
    }
    let asset = T::CanonicalStableAssetId::get();
    T::Assets::mint_for_bench(asset, who, amount).expect("benchmark canonical funding");
}

fn fund_canonical_fee<T>(who: &T::AccountId)
where
    T: crate::Config + frame_system::Config,
{
    let fee = T::MinCreationFee::get();
    let amount = fee.saturating_add(fee);
    mint_canonical_balance::<T>(who, amount);
}

fn benchmark_bond_amount<T>() -> BenchBalanceOf<T>
where
    T: crate::Config,
{
    let min = T::GovernanceBondMinimum::get();
    if min.is_zero() {
        BenchBalanceOf::<T>::one()
    } else {
        min
    }
}

fn setup_creator_market<T>(caller: &T::AccountId, seed: BenchBalanceOf<T>)
where
    T: crate::Config + frame_system::Config,
    T::AccountId: Clone,
{
    GovernanceBonds::<T>::insert(caller, T::GovernanceBondMinimum::get());
    fund_canonical_fee::<T>(caller);
    mint_canonical_balance::<T>(caller, seed);
    let metadata = default_condition_input();
    Pallet::<T>::create_condition(RawOrigin::Signed(caller.clone()).into(), metadata)
        .expect("condition setup");
    let close = <frame_system::Pallet<T>>::block_number()
        + T::MinMarketDuration::get()
        + BlockNumberFor::<T>::one();
    Pallet::<T>::create_market(RawOrigin::Signed(caller.clone()).into(), 0, close, seed)
        .expect("market setup");
}

fn market_close_block<T>() -> BlockNumberFor<T>
where
    T: crate::Config + frame_system::Config,
{
    Markets::<T>::get(0).expect("market").close_block
}

fn setup_buyback_pool<T>(caller: &T::AccountId)
where
    T: crate::Config<AssetId = common::AssetIdOf<T>, Balance = common::Balance>
        + frame_system::Config
        + dex_manager::Config
        + trading_pair::Config
        + price_tools::Config
        + pool_xyk::Config,
    common::AssetIdOf<T>: From<common::AssetId32<common::PredefinedAssetId>>,
    T::AccountId: Clone,
{
    let unit: common::Balance = 1_000_000_000_000_000_000;
    let dex_id: <T as common::Config>::DEXId = DEXId::Polkaswap.into();
    let xor: BenchAssetIdOf<T> = XOR.into();
    if !dex_manager::DEXInfos::<T>::contains_key(&dex_id) {
        dex_manager::DEXInfos::<T>::insert(
            &dex_id,
            DEXInfo {
                base_asset_id: xor,
                synthetic_base_asset_id: xor,
                is_public: true,
            },
        );
    }

    let stable: BenchAssetIdOf<T> = T::CanonicalStableAssetId::get();
    if !<T as pool_xyk::Config>::AssetInfoProvider::asset_exists(&stable) {
        <T as common::Config>::AssetManager::register_asset_id(
            caller.clone(),
            stable,
            AssetSymbol(b"KUSD".to_vec()),
            AssetName(b"Benchmark KUSD".to_vec()),
            18,
            0,
            true,
            AssetType::Regular,
            None,
            None,
        )
        .expect("benchmark stable asset registration");
    }
    T::Assets::mint_for_bench(xor, caller, 1_000u128.saturating_mul(unit))
        .expect("benchmark xor funding");
    T::Assets::mint_for_bench(stable, caller, 1_000u128.saturating_mul(unit))
        .expect("benchmark canonical funding");

    let _ = trading_pair::Pallet::<T>::register_pair(dex_id.clone(), xor, stable);
    let _ = pool_xyk::Pallet::<T>::initialize_pool(
        RawOrigin::Signed(caller.clone()).into(),
        dex_id.clone(),
        xor,
        stable,
    );
    pool_xyk::Pallet::<T>::deposit_liquidity(
        RawOrigin::Signed(caller.clone()).into(),
        dex_id,
        xor,
        stable,
        unit,
        unit,
        unit,
        unit,
    )
    .expect("benchmark buyback pool setup");
    let _ = price_tools::Pallet::<T>::register_asset(&stable);
    for _ in 0..price_tools::AVG_BLOCK_SPAN {
        price_tools::Pallet::<T>::average_prices_calculation_routine();
    }
}

#[benchmarks(where
    T: crate::Config<AssetId = common::AssetIdOf<T>, Balance = common::Balance>
        + dex_manager::Config
        + trading_pair::Config
        + price_tools::Config
        + pool_xyk::Config,
    common::AssetIdOf<T>: From<common::AssetId32<common::PredefinedAssetId>>,
    T::AccountId: From<<T as frame_system::Config>::AccountId> + Clone,
)]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn create_condition() {
        let caller: T::AccountId = whitelisted_caller();
        GovernanceBonds::<T>::insert(&caller, T::GovernanceBondMinimum::get());
        let metadata = default_condition_input();

        #[extrinsic_call]
        create_condition(RawOrigin::Signed(caller), metadata);
    }

    #[benchmark]
    fn create_opengov_condition() {
        let caller: T::AccountId = whitelisted_caller();
        GovernanceBonds::<T>::insert(&caller, T::GovernanceBondMinimum::get());
        let metadata = default_condition_input();
        let proposal = OpengovProposalInput {
            network: RelayNetwork::Polkadot,
            parachain_id: 1,
            track_id: 1,
            referendum_index: 1,
            plaza_tag: b"benchmark".to_vec(),
        };

        #[extrinsic_call]
        create_opengov_condition(RawOrigin::Signed(caller), metadata, proposal);
    }

    #[benchmark]
    fn create_market() {
        let caller: T::AccountId = whitelisted_caller();
        GovernanceBonds::<T>::insert(&caller, T::GovernanceBondMinimum::get());
        fund_canonical_fee::<T>(&caller);
        let seed = bench_balance::<T>(10_000);
        mint_canonical_balance::<T>(&caller, seed);
        let metadata = default_condition_input();
        Pallet::<T>::create_condition(RawOrigin::Signed(caller.clone()).into(), metadata)
            .expect("condition setup");
        let close = <frame_system::Pallet<T>>::block_number()
            + T::MinMarketDuration::get()
            + BlockNumberFor::<T>::one();

        #[extrinsic_call]
        create_market(RawOrigin::Signed(caller), 0, close, seed);
    }

    #[benchmark]
    fn buy() {
        let caller: T::AccountId = whitelisted_caller();
        setup_creator_market::<T>(&caller, bench_balance::<T>(100_000));
        let trader: T::AccountId = account("trader", 0, 0);
        mint_canonical_balance::<T>(&trader, bench_balance::<T>(20_000));

        #[extrinsic_call]
        buy(
            RawOrigin::Signed(trader),
            0,
            BinaryOutcome::Yes,
            bench_balance::<T>(10_000),
            BenchBalanceOf::<T>::zero(),
        );
    }

    #[benchmark]
    fn sell() {
        let caller: T::AccountId = whitelisted_caller();
        setup_creator_market::<T>(&caller, bench_balance::<T>(100_000));
        let trader: T::AccountId = account("trader", 0, 0);
        mint_canonical_balance::<T>(&trader, bench_balance::<T>(20_000));
        Pallet::<T>::buy(
            RawOrigin::Signed(trader.clone()).into(),
            0,
            BinaryOutcome::Yes,
            bench_balance::<T>(10_000),
            BenchBalanceOf::<T>::zero(),
        )
        .expect("buy setup");

        #[extrinsic_call]
        sell(
            RawOrigin::Signed(trader),
            0,
            BinaryOutcome::Yes,
            bench_balance::<T>(5_000),
            BenchBalanceOf::<T>::zero(),
        );
    }

    #[benchmark]
    fn sync_market_status() {
        let caller: T::AccountId = whitelisted_caller();
        setup_creator_market::<T>(&caller, bench_balance::<T>(100_000));
        let close = market_close_block::<T>();
        <frame_system::Pallet<T>>::set_block_number(close);

        #[extrinsic_call]
        sync_market_status(RawOrigin::Signed(caller), 0);
    }

    #[benchmark]
    fn bond_governance() {
        let caller: T::AccountId = whitelisted_caller();
        let amount = benchmark_bond_amount::<T>();
        T::Assets::mint_for_bench(T::CanonicalStableAssetId::get(), &caller, amount)
            .expect("bond funding");

        #[extrinsic_call]
        bond_governance(RawOrigin::Signed(caller), amount);
    }

    #[benchmark]
    fn unbond_governance() {
        let caller: T::AccountId = whitelisted_caller();
        let amount = benchmark_bond_amount::<T>();
        T::Assets::mint_for_bench(T::CanonicalStableAssetId::get(), &caller, amount)
            .expect("bond funding");
        Pallet::<T>::bond_governance(RawOrigin::Signed(caller.clone()).into(), amount)
            .expect("bond setup");

        #[extrinsic_call]
        unbond_governance(RawOrigin::Signed(caller), amount);
    }

    #[benchmark]
    fn resolve_market() {
        let caller: T::AccountId = whitelisted_caller();
        setup_creator_market::<T>(&caller, bench_balance::<T>(100_000));
        let close = market_close_block::<T>();
        <frame_system::Pallet<T>>::set_block_number(close);

        #[extrinsic_call]
        resolve_market(RawOrigin::Root, 0, BinaryOutcome::Yes);
    }

    #[benchmark]
    fn cancel_market() {
        let caller: T::AccountId = whitelisted_caller();
        setup_creator_market::<T>(&caller, bench_balance::<T>(100_000));
        let close = market_close_block::<T>();
        <frame_system::Pallet<T>>::set_block_number(close);

        #[extrinsic_call]
        cancel_market(RawOrigin::Root, 0);
    }

    #[benchmark]
    fn claim_market() {
        let caller: T::AccountId = whitelisted_caller();
        setup_creator_market::<T>(&caller, bench_balance::<T>(100_000));
        let trader: T::AccountId = account("trader", 0, 0);
        mint_canonical_balance::<T>(&trader, bench_balance::<T>(20_000));
        Pallet::<T>::buy(
            RawOrigin::Signed(trader.clone()).into(),
            0,
            BinaryOutcome::Yes,
            bench_balance::<T>(10_000),
            BenchBalanceOf::<T>::zero(),
        )
        .expect("buy setup");
        let close = market_close_block::<T>();
        <frame_system::Pallet<T>>::set_block_number(close);
        Pallet::<T>::resolve_market(RawOrigin::Root.into(), 0, BinaryOutcome::Yes)
            .expect("resolve setup");

        #[extrinsic_call]
        claim_market(RawOrigin::Signed(trader), 0);
    }

    #[benchmark]
    fn claim_creator_fees() {
        let caller: T::AccountId = whitelisted_caller();
        setup_creator_market::<T>(&caller, bench_balance::<T>(100_000));
        let trader: T::AccountId = account("trader", 0, 0);
        mint_canonical_balance::<T>(&trader, bench_balance::<T>(20_000));
        Pallet::<T>::buy(
            RawOrigin::Signed(trader).into(),
            0,
            BinaryOutcome::Yes,
            bench_balance::<T>(10_000),
            BenchBalanceOf::<T>::zero(),
        )
        .expect("buy setup");

        #[extrinsic_call]
        claim_creator_fees(RawOrigin::Signed(caller), 0);
    }

    #[benchmark]
    fn claim_creator_liquidity() {
        let caller: T::AccountId = whitelisted_caller();
        setup_creator_market::<T>(&caller, bench_balance::<T>(100_000));
        let close = market_close_block::<T>();
        <frame_system::Pallet<T>>::set_block_number(close);
        Pallet::<T>::resolve_market(RawOrigin::Root.into(), 0, BinaryOutcome::Yes)
            .expect("resolve setup");

        #[extrinsic_call]
        claim_creator_liquidity(RawOrigin::Signed(caller), 0);
    }

    #[benchmark]
    fn sweep_xor_buyback_and_burn() {
        let unit: common::Balance = 1_000_000_000_000_000_000;
        let caller: T::AccountId = whitelisted_caller();
        setup_creator_market::<T>(&caller, 100u128.saturating_mul(unit));
        setup_buyback_pool::<T>(&caller);
        let trader: T::AccountId = account("trader", 0, 0);
        mint_canonical_balance::<T>(&trader, 20u128.saturating_mul(unit));
        Pallet::<T>::buy(
            RawOrigin::Signed(trader).into(),
            0,
            BinaryOutcome::Yes,
            10u128.saturating_mul(unit),
            BenchBalanceOf::<T>::zero(),
        )
        .expect("buy setup");

        #[extrinsic_call]
        sweep_xor_buyback_and_burn(RawOrigin::Signed(caller));
    }
}
