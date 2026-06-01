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

fn repeated_bytes(byte: u8, len: u32) -> Vec<u8> {
    sp_std::vec![byte; len as usize]
}

fn default_condition_input<T: crate::Config>() -> ConditionInput {
    let metadata_len = T::MaxMetadataLength::get();
    ConditionInput {
        question: repeated_bytes(b'Q', metadata_len),
        oracle: repeated_bytes(b'O', metadata_len),
        resolution_source: repeated_bytes(b'S', metadata_len),
    }
}

fn default_condition_details<T: crate::Config>() -> ConditionDetailsInput {
    let metadata_len = T::MaxMetadataLength::get();
    ConditionDetailsInput {
        category: repeated_bytes(b'C', metadata_len),
        tags: repeated_bytes(b'T', metadata_len),
        metadata_uri: repeated_bytes(b'M', metadata_len),
        metadata_hash: Some([7; 32]),
        rules_uri: repeated_bytes(b'R', metadata_len),
    }
}

fn default_evidence<T: crate::Config>() -> EvidenceInput {
    EvidenceInput {
        uri: repeated_bytes(b'E', T::MaxMetadataLength::get()),
        hash: Some([9; 32]),
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

fn setup_creator_market<T>(caller: &T::AccountId, seed: BenchBalanceOf<T>)
where
    T: crate::Config + frame_system::Config,
    T::AccountId: Clone,
{
    fund_canonical_fee::<T>(caller);
    mint_canonical_balance::<T>(caller, seed);
    let metadata = default_condition_input::<T>();
    Pallet::<T>::create_condition(RawOrigin::Signed(caller.clone()).into(), metadata)
        .expect("condition setup");
    let close = <frame_system::Pallet<T>>::block_number()
        + T::MinMarketDuration::get()
        + BlockNumberFor::<T>::one();
    T::Assets::transfer(
        T::CanonicalStableAssetId::get(),
        caller,
        &Pallet::<T>::account_id(),
        seed,
    )
    .expect("legacy seed transfer");
    Markets::<T>::insert(
        0,
        Market {
            creator: caller.clone(),
            condition_id: 0,
            close_block: close,
            collateral_asset: T::CanonicalStableAssetId::get(),
            seed_liquidity: seed,
            mechanism: MarketMechanism::LegacyAmm,
            status: MarketStatus::Open,
        },
    );
    MarketPools::<T>::insert(
        0,
        MarketPool {
            collateral: seed,
            yes: seed,
            no: seed,
        },
    );
    LiquidityPositions::<T>::insert(
        0,
        caller,
        LiquidityPosition {
            shares: seed,
            collateral_contributed: seed,
        },
    );
    LiquidityPositionTotals::<T>::insert(
        0,
        LiquidityTotals {
            total_shares: seed,
            total_collateral_contributed: seed,
        },
    );
    ConditionMarket::<T>::insert(0, 0);
    NextMarketId::<T>::put(1);
}

fn setup_orderbook_market<T>(caller: &T::AccountId) -> BlockNumberFor<T>
where
    T: crate::Config + frame_system::Config,
    T::AccountId: Clone,
{
    fund_canonical_fee::<T>(caller);
    let metadata = default_condition_input::<T>();
    Pallet::<T>::create_condition(RawOrigin::Signed(caller.clone()).into(), metadata)
        .expect("condition setup");
    let close = <frame_system::Pallet<T>>::block_number()
        + T::MinMarketDuration::get()
        + BlockNumberFor::<T>::one();
    Pallet::<T>::create_market(RawOrigin::Signed(caller.clone()).into(), 0, close)
        .expect("order-book market setup");
    close
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
        fund_canonical_fee::<T>(&caller);
        let metadata = default_condition_input::<T>();

        #[extrinsic_call]
        create_condition(RawOrigin::Signed(caller), metadata);
    }

    #[benchmark]
    fn create_condition_with_details() {
        let caller: T::AccountId = whitelisted_caller();
        fund_canonical_fee::<T>(&caller);
        let metadata = default_condition_input::<T>();
        let details = default_condition_details::<T>();

        #[extrinsic_call]
        create_condition_with_details(RawOrigin::Signed(caller), metadata, details);
    }

    #[benchmark]
    fn create_market() {
        let caller: T::AccountId = whitelisted_caller();
        fund_canonical_fee::<T>(&caller);
        let metadata = default_condition_input::<T>();
        Pallet::<T>::create_condition(RawOrigin::Signed(caller.clone()).into(), metadata)
            .expect("condition setup");
        let close = <frame_system::Pallet<T>>::block_number()
            + T::MinMarketDuration::get()
            + BlockNumberFor::<T>::one();

        #[extrinsic_call]
        create_market(RawOrigin::Signed(caller), 0, close);
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
    fn flip_position() {
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
        flip_position(
            RawOrigin::Signed(trader),
            0,
            BinaryOutcome::Yes,
            bench_balance::<T>(5_000),
            BenchBalanceOf::<T>::zero(),
            BenchBalanceOf::<T>::zero(),
        );
    }

    #[benchmark]
    fn add_liquidity() {
        let caller: T::AccountId = whitelisted_caller();
        setup_creator_market::<T>(&caller, bench_balance::<T>(100_000));
        let provider: T::AccountId = account("provider", 0, 0);
        mint_canonical_balance::<T>(&provider, bench_balance::<T>(20_000));

        #[extrinsic_call]
        add_liquidity(
            RawOrigin::Signed(provider),
            0,
            bench_balance::<T>(10_000),
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
    fn resolve_market() {
        let caller: T::AccountId = whitelisted_caller();
        setup_creator_market::<T>(&caller, bench_balance::<T>(100_000));
        let close = market_close_block::<T>();
        <frame_system::Pallet<T>>::set_block_number(close);

        #[extrinsic_call]
        resolve_market(RawOrigin::Root, 0, BinaryOutcome::Yes);
    }

    #[benchmark]
    fn resolve_market_with_evidence() {
        let caller: T::AccountId = whitelisted_caller();
        setup_creator_market::<T>(&caller, bench_balance::<T>(100_000));
        let close = market_close_block::<T>();
        <frame_system::Pallet<T>>::set_block_number(close);

        #[extrinsic_call]
        resolve_market_with_evidence(
            RawOrigin::Root,
            0,
            BinaryOutcome::Yes,
            default_evidence::<T>(),
        );
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
    fn emergency_cancel_market() {
        let caller: T::AccountId = whitelisted_caller();
        setup_creator_market::<T>(&caller, bench_balance::<T>(100_000));

        #[extrinsic_call]
        emergency_cancel_market(RawOrigin::Root, 0, default_evidence::<T>());
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
    fn claim_markets(n: Linear<1, { T::MaxBatchClaims::get() }>) {
        let caller: T::AccountId = whitelisted_caller();
        let trader: T::AccountId = account("trader", 0, 0);
        let batch = n;
        let seed = bench_balance::<T>(10_000);
        let stake = bench_balance::<T>(1_000);
        let mut market_ids = Vec::new();
        for market_id in 0..batch {
            fund_canonical_fee::<T>(&caller);
            mint_canonical_balance::<T>(&caller, seed);
            mint_canonical_balance::<T>(&trader, stake);
            Pallet::<T>::create_condition(
                RawOrigin::Signed(caller.clone()).into(),
                default_condition_input::<T>(),
            )
            .expect("condition setup");
            let close = <frame_system::Pallet<T>>::block_number()
                + T::MinMarketDuration::get()
                + BlockNumberFor::<T>::one();
            Pallet::<T>::create_market(RawOrigin::Signed(caller.clone()).into(), market_id, close)
                .expect("market setup");
            T::Assets::transfer(
                T::CanonicalStableAssetId::get(),
                &caller,
                &Pallet::<T>::account_id(),
                seed,
            )
            .expect("legacy seed transfer");
            Markets::<T>::mutate(market_id, |market| {
                let market = market.as_mut().expect("created market");
                market.seed_liquidity = seed;
                market.mechanism = MarketMechanism::LegacyAmm;
            });
            MarketPools::<T>::insert(
                market_id,
                MarketPool {
                    collateral: seed,
                    yes: seed,
                    no: seed,
                },
            );
            LiquidityPositions::<T>::insert(
                market_id,
                &caller,
                LiquidityPosition {
                    shares: seed,
                    collateral_contributed: seed,
                },
            );
            LiquidityPositionTotals::<T>::insert(
                market_id,
                LiquidityTotals {
                    total_shares: seed,
                    total_collateral_contributed: seed,
                },
            );
            Pallet::<T>::buy(
                RawOrigin::Signed(trader.clone()).into(),
                market_id,
                BinaryOutcome::Yes,
                stake,
                BenchBalanceOf::<T>::zero(),
            )
            .expect("buy setup");
            market_ids.push(market_id);
        }
        let close = market_close_block::<T>();
        <frame_system::Pallet<T>>::set_block_number(close);
        for market_id in 0..batch {
            Pallet::<T>::resolve_market(RawOrigin::Root.into(), market_id, BinaryOutcome::Yes)
                .expect("resolve setup");
        }
        let market_ids = market_ids.try_into().expect("bounded batch");

        #[extrinsic_call]
        claim_markets(RawOrigin::Signed(trader), market_ids);
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
    fn claim_liquidity() {
        let caller: T::AccountId = whitelisted_caller();
        setup_creator_market::<T>(&caller, bench_balance::<T>(100_000));
        let provider: T::AccountId = account("provider", 0, 0);
        mint_canonical_balance::<T>(&provider, bench_balance::<T>(20_000));
        Pallet::<T>::add_liquidity(
            RawOrigin::Signed(provider.clone()).into(),
            0,
            bench_balance::<T>(10_000),
            BenchBalanceOf::<T>::zero(),
        )
        .expect("liquidity setup");
        let close = market_close_block::<T>();
        <frame_system::Pallet<T>>::set_block_number(close);
        Pallet::<T>::resolve_market(RawOrigin::Root.into(), 0, BinaryOutcome::Yes)
            .expect("resolve setup");

        #[extrinsic_call]
        claim_liquidity(RawOrigin::Signed(provider), 0, BenchBalanceOf::<T>::zero());
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

    #[benchmark]
    fn place_order() {
        let caller: T::AccountId = whitelisted_caller();
        setup_orderbook_market::<T>(&caller);
        let maker: T::AccountId = account("maker", 0, 0);
        let taker: T::AccountId = account("taker", 0, 0);
        let fills = T::MaxFillsPerOrder::get()
            .min(T::MaxOrdersPerPrice::get())
            .max(1);
        let shares_per_order = bench_balance::<T>(100);
        let total_shares = shares_per_order.saturating_mul(fills.into());
        mint_canonical_balance::<T>(&maker, total_shares);
        mint_canonical_balance::<T>(&taker, total_shares.saturating_mul(2u32.into()));
        Pallet::<T>::split_position(RawOrigin::Signed(maker.clone()).into(), 0, total_shares)
            .expect("maker split setup");
        for _ in 0..fills {
            Pallet::<T>::place_order(
                RawOrigin::Signed(maker.clone()).into(),
                0,
                BinaryOutcome::Yes,
                OrderSide::Sell,
                50,
                shares_per_order,
                TimeInForce::Gtc,
            )
            .expect("maker order setup");
        }

        #[extrinsic_call]
        place_order(
            RawOrigin::Signed(taker),
            0,
            BinaryOutcome::Yes,
            OrderSide::Buy,
            50,
            total_shares,
            TimeInForce::Ioc,
        );
    }

    #[benchmark]
    fn cancel_order() {
        let caller: T::AccountId = whitelisted_caller();
        setup_orderbook_market::<T>(&caller);
        mint_canonical_balance::<T>(&caller, bench_balance::<T>(1_000));
        Pallet::<T>::place_order(
            RawOrigin::Signed(caller.clone()).into(),
            0,
            BinaryOutcome::Yes,
            OrderSide::Buy,
            50,
            bench_balance::<T>(100),
            TimeInForce::Gtc,
        )
        .expect("order setup");

        #[extrinsic_call]
        cancel_order(RawOrigin::Signed(caller), 0);
    }

    #[benchmark]
    fn split_position() {
        let caller: T::AccountId = whitelisted_caller();
        setup_orderbook_market::<T>(&caller);
        let shares = bench_balance::<T>(100);
        mint_canonical_balance::<T>(&caller, shares);

        #[extrinsic_call]
        split_position(RawOrigin::Signed(caller), 0, shares);
    }

    #[benchmark]
    fn merge_positions() {
        let caller: T::AccountId = whitelisted_caller();
        setup_orderbook_market::<T>(&caller);
        let shares = bench_balance::<T>(100);
        mint_canonical_balance::<T>(&caller, shares);
        Pallet::<T>::split_position(RawOrigin::Signed(caller.clone()).into(), 0, shares)
            .expect("split setup");

        #[extrinsic_call]
        merge_positions(RawOrigin::Signed(caller), 0, shares);
    }
}
