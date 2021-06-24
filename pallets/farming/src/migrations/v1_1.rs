use frame_support::dispatch::Weight;
use frame_support::print;
use frame_support::traits::schedule::{Anon, DispatchTime};
use frame_support::traits::Get;
use frame_system::pallet_prelude::BlockNumberFor;
use frame_system::RawOrigin;
use sp_runtime::traits::Zero;
use sp_std::collections::btree_set::BTreeSet;

use pool_xyk::PoolProviders;

use crate::{Call, Config, Module};

pub fn migrate<T: Config>() -> Weight {
    let current_block_number = frame_system::Module::<T>::block_number();
    if !(current_block_number % T::REFRESH_FREQUENCY).is_zero() {
        let when = current_block_number - current_block_number % T::REFRESH_FREQUENCY
            + T::REFRESH_FREQUENCY;
        if T::Scheduler::schedule(
            DispatchTime::At(when),
            None,
            1,
            RawOrigin::Root.into(),
            Call::migrate_to_1_1().into(),
        )
        .is_err()
        {
            print("farming migration to v1.1 failed to schedule");
        }
        return 0;
    }

    let mut read_count = 0;
    let mut pools = BTreeSet::new();
    for (pool, _, _) in PoolProviders::<T>::iter() {
        read_count += 1;
        pools.insert(pool);
    }

    let write_count = pools.len() as u64;
    for (i, pool) in pools.into_iter().enumerate() {
        let block_number: BlockNumberFor<T> = (i as u32).into();
        Module::<T>::add_pool(pool, block_number);
    }

    T::DbWeight::get().reads_writes(read_count, write_count)
}

#[cfg(test)]
mod tests {
    use common::balance;
    use pool_xyk::PoolProviders;

    use crate::mock::{
        self, AccountId, ExtBuilder, Runtime, ALICE, BOB, CHARLIE, REFRESH_FREQUENCY,
    };
    use crate::{utils, Pools};

    // Ensure the migration happens if the current block starts a new interval of farming
    #[test]
    fn migrate_1() {
        ExtBuilder::default().build().execute_with(|| {
            const POOLS_PER_BLOCK: u64 = 2;

            let mut pools: Vec<AccountId> = (0..REFRESH_FREQUENCY * POOLS_PER_BLOCK)
                .into_iter()
                .map(|i| utils::account::<Runtime>(i as u32))
                .collect();
            let accounts = [ALICE(), BOB(), CHARLIE()];

            for pool in &pools {
                for account in &accounts {
                    PoolProviders::<Runtime>::insert(pool, account, balance!(100));
                }
            }

            super::migrate::<Runtime>();

            // Due to the nature of non-determinism of the StorageMap::iter we cannot have easy checks
            for (_, storage_pools) in Pools::<Runtime>::iter() {
                assert_eq!(storage_pools.len(), POOLS_PER_BLOCK as usize);

                for pool in storage_pools {
                    let index = pools.iter().position(|p| p == &pool).unwrap();
                    pools.remove(index);
                }
            }

            assert_eq!(pools, Vec::new());
        });
    }

    // Ensure the migration happens at the beginning of a next interval of farming if the current block doesn't start a new interval of farming
    #[test]
    fn migrate_2() {
        ExtBuilder::default().build().execute_with(|| {
            const POOLS_PER_BLOCK: u64 = 2;

            let mut pools: Vec<AccountId> = (0..REFRESH_FREQUENCY * POOLS_PER_BLOCK)
                .into_iter()
                .map(|i| utils::account::<Runtime>(i as u32))
                .collect();
            let accounts = [ALICE(), BOB(), CHARLIE()];

            for pool in &pools {
                for account in &accounts {
                    PoolProviders::<Runtime>::insert(pool, account, balance!(100));
                }
            }

            // Shift from the beginning of the interval
            mock::run_to_block(1);

            // Try migration
            super::migrate::<Runtime>();

            // Assert it didn't migrate yet
            assert_eq!(Pools::<Runtime>::iter().collect::<Vec<_>>(), Vec::new());

            // Shift to the beginning of a new interval
            mock::run_to_block(REFRESH_FREQUENCY);

            // Due to the nature of non-determinism of the StorageMap::iter we cannot have easy checks
            for (_, storage_pools) in Pools::<Runtime>::iter() {
                assert_eq!(storage_pools.len(), POOLS_PER_BLOCK as usize);

                for pool in storage_pools {
                    let index = pools.iter().position(|p| p == &pool).unwrap();
                    pools.remove(index);
                }
            }

            assert!(pools.is_empty());
        });
    }
}
