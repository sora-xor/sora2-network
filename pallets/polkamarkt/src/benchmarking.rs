#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_support::traits::Get;
use frame_system::RawOrigin;
use sp_runtime::traits::{One, Saturating, Zero};

fn default_condition_input<BlockNumber>() -> ConditionInput<BlockNumber>
where
    BlockNumber: Default,
{
    ConditionInput {
        question: b"Will Hydra succeed across all markets?".to_vec(),
        oracle: b"Chainlink".to_vec(),
        resolution_source: b"https://oracle.example.com".to_vec(),
        submission_deadline: BlockNumber::default(),
    }
}

fn mint_canonical_balance<T>(who: &T::AccountId, amount: T::Balance)
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

#[benchmarks(where
    T::AccountId: From<<T as frame_system::Config>::AccountId>,
)]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn create_condition() {
        let caller: T::AccountId = whitelisted_caller();
        GovernanceBonds::<T>::insert(&caller, T::GovernanceBondMinimum::get());
        let metadata = default_condition_input::<BlockNumberFor<T>>();

        #[extrinsic_call]
        create_condition(RawOrigin::Signed(caller), metadata);
    }

    #[benchmark]
    fn create_market() {
        let caller: T::AccountId = whitelisted_caller();
        GovernanceBonds::<T>::insert(&caller, T::GovernanceBondMinimum::get());
        fund_canonical_fee::<T>(&caller);
        let seed = T::Balance::one();
        mint_canonical_balance::<T>(&caller, seed);
        let metadata = default_condition_input::<BlockNumberFor<T>>();
        Pallet::<T>::create_condition(RawOrigin::Signed(caller.clone()).into(), metadata)
            .expect("condition setup");

        let close = <frame_system::Pallet<T>>::block_number()
            + T::MinMarketDuration::get()
            + BlockNumberFor::<T>::one();

        #[extrinsic_call]
        create_market(RawOrigin::Signed(caller), 0, close, seed, None);
    }

    #[benchmark]
    fn commit_order() {
        let caller: T::AccountId = whitelisted_caller();
        GovernanceBonds::<T>::insert(&caller, T::GovernanceBondMinimum::get());
        fund_canonical_fee::<T>(&caller);
        let metadata = default_condition_input::<BlockNumberFor<T>>();
        Pallet::<T>::create_condition(RawOrigin::Signed(caller.clone()).into(), metadata)
            .expect("condition setup");
        let close = <frame_system::Pallet<T>>::block_number()
            + T::MinMarketDuration::get()
            + BlockNumberFor::<T>::one();
        Pallet::<T>::create_market(
            RawOrigin::Signed(caller.clone()).into(),
            0,
            close,
            T::Balance::zero(),
            None,
        )
        .expect("market setup");

        let commitment = [1u8; 32];

        #[extrinsic_call]
        commit_order(RawOrigin::Signed(caller), 0, commitment);
    }

    #[benchmark]
    fn reveal_order() {
        let caller: T::AccountId = whitelisted_caller();
        GovernanceBonds::<T>::insert(&caller, T::GovernanceBondMinimum::get());
        fund_canonical_fee::<T>(&caller);
        let metadata = default_condition_input::<BlockNumberFor<T>>();
        Pallet::<T>::create_condition(RawOrigin::Signed(caller.clone()).into(), metadata)
            .expect("condition setup");
        let close = <frame_system::Pallet<T>>::block_number()
            + T::MinMarketDuration::get()
            + BlockNumberFor::<T>::one();
        Pallet::<T>::create_market(
            RawOrigin::Signed(caller.clone()).into(),
            0,
            close,
            T::Balance::zero(),
            None,
        )
        .expect("market setup");

        let payload = b"BUY:10@50".to_vec();
        let salt = b"benchmark".to_vec();
        let order_value = T::Balance::one();
        let commitment =
            Pallet::<T>::compute_commitment_hash(&caller, 0, &payload, &salt, &order_value);

        Pallet::<T>::commit_order(RawOrigin::Signed(caller.clone()).into(), 0, commitment)
            .expect("commit order");

        let now = <frame_system::Pallet<T>>::block_number()
            + T::CommitmentRevealDelay::get()
            + BlockNumberFor::<T>::one();
        <frame_system::Pallet<T>>::set_block_number(now);

        #[extrinsic_call]
        reveal_order(RawOrigin::Signed(caller), 0, payload, salt, order_value);
    }

    #[benchmark]
    fn bridge_deposit() {
        let caller: T::AccountId = whitelisted_caller();
        let asset = T::UsdcAssetId::get();
        let amount = T::Balance::one();
        T::Assets::mint_for_bench(asset, &caller, amount).expect("bridge asset funding");

        #[extrinsic_call]
        bridge_deposit(RawOrigin::Signed(caller), asset, amount);
    }

    #[benchmark]
    fn bridge_withdraw() {
        let caller: T::AccountId = whitelisted_caller();
        let wallet = caller.clone();
        let amount = T::Balance::one();
        BridgeWallet::<T>::insert(&caller, wallet);
        BridgeEntitlements::<T>::insert(&caller, amount.saturating_add(amount));

        #[extrinsic_call]
        bridge_withdraw(RawOrigin::Signed(caller), amount);
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
    }
}
