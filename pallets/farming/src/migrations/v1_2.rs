use crate::{Config, Pallet, PoolFarmer, PoolFarmers};
use codec::Decode;
use common::{generate_storage_instance, RewardReason};
use frame_support::debug::{debug, error};
use frame_support::dispatch::Weight;
use frame_support::pallet_prelude::*;
use frame_support::storage::types::StorageMap;
use frame_support::traits::Get;
use sp_runtime::AccountId32;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::vec::Vec;

#[cfg(feature = "private-net")]
const POOLS_BYTES: &[u8] = include_bytes!("../../../../misc/farming_snapshot/dev/pools");
#[cfg(feature = "private-net")]
const REWARDS_BYTES: &[u8] = include_bytes!("../../../../misc/farming_snapshot/dev/rewards");

#[cfg(not(feature = "private-net"))]
const POOLS_BYTES: &[u8] = include_bytes!("../../../../misc/farming_snapshot/main/pools");
#[cfg(not(feature = "private-net"))]
const REWARDS_BYTES: &[u8] = include_bytes!("../../../../misc/farming_snapshot/main/rewards");

generate_storage_instance!(Farming, SavedValues);
type OldSavedValues<T> = StorageMap<
    SavedValuesOldInstance,
    Identity,
    <T as frame_system::Config>::BlockNumber,
    Vec<(<T as frame_system::Config>::AccountId, Vec<PoolFarmer<T>>)>,
    ValueQuery,
>;

pub fn migrate<T: Config>() -> Weight {
    let pools = BTreeMap::<AccountId32, Vec<(AccountId32, u32)>>::decode(&mut &POOLS_BYTES[..]);
    let pools = match pools {
        Ok(pools) => {
            debug!("pools: {}", pools.len());
            pools
        }
        Err(e) => {
            error!("failed to decode pools: {:?}", e);
            return 0;
        }
    };

    let rewards = Vec::<(AccountId32, u128)>::decode(&mut &REWARDS_BYTES[..]);
    let rewards = match rewards {
        Ok(rewards) => {
            debug!("rewards: {}", rewards.len());
            rewards
        }
        Err(e) => {
            error!("failed to decode rewards: {:?}", e);
            return 0;
        }
    };

    apply_pool_farmers::<T>(&pools);
    apply_saved_values::<T>(&pools);
    apply_rewards::<T>(rewards);

    T::BlockWeights::get()
        .get(DispatchClass::Normal)
        .max_extrinsic
        .unwrap_or(0)
}

fn apply_pool_farmers<T: Config>(pools: &BTreeMap<AccountId32, Vec<(AccountId32, u32)>>) {
    for (pool, mut farmers) in PoolFarmers::<T>::iter() {
        let accounts = pools.get(&pool);

        for farmer in &mut farmers {
            if let Some((_, block)) =
                accounts.and_then(|a| a.iter().find(|(account, _)| farmer.account == *account))
            {
                farmer.block = T::BlockNumber::from(*block);
            } else {
                farmer.block -= farmer.block % T::REFRESH_FREQUENCY;
            }
        }

        PoolFarmers::<T>::insert(pool, farmers);
    }
}

fn apply_saved_values<T: Config>(pools: &BTreeMap<AccountId32, Vec<(AccountId32, u32)>>) {
    let mut block_number = if let Some(b) = OldSavedValues::<T>::iter()
        .filter(|(_, accounts)| !accounts.is_empty())
        .fold(None, |a, b| {
            if a.map(|a| a > b.0).unwrap_or(true) {
                Some(b.0)
            } else {
                a
            }
        }) {
        b
    } else {
        return;
    };

    // Take blocks from snapshot only for farmers who are known to be farming since the first vesting
    let last_block = block_number;

    loop {
        let saved_pools = OldSavedValues::<T>::take(block_number);
        if saved_pools.is_empty() {
            break;
        }

        let mut accounts = BTreeMap::new();

        for (pool, mut saved_accounts) in saved_pools {
            let snapshot_accounts = pools.get(&pool);

            for saved_account in &mut saved_accounts {
                if saved_account.block < last_block {
                    if let Some(x) = snapshot_accounts
                        .and_then(|a| a.iter().find(|x| x.0 == saved_account.account))
                    {
                        saved_account.block = x.1.into();
                    }
                }

                saved_account.block -= saved_account.block % T::REFRESH_FREQUENCY;
            }

            Pallet::<T>::prepare_pool_accounts_for_vesting(
                saved_accounts,
                block_number,
                &mut accounts,
            );
        }

        debug!("vest for {}", accounts.len());

        Pallet::<T>::vest_account_rewards(accounts);

        block_number += T::VESTING_FREQUENCY;
    }

    // Empty values were skipped, remove them
    OldSavedValues::<T>::remove_all();
}

fn apply_rewards<T: Config>(rewards: Vec<(AccountId32, u128)>) {
    for (account, reward) in rewards {
        if let Err(e) = vested_rewards::Module::<T>::add_pending_reward(
            &account.into(),
            RewardReason::LiquidityProvisionFarming,
            reward,
        ) {
            error!("add_pending_reward failed: {:?}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::OldSavedValues;
    use crate::mock::{AccountId, ExtBuilder, Runtime};
    use crate::{Pallet, PoolFarmer, PoolFarmers};
    use common::{balance, RewardReason};
    use log::LevelFilter;
    use sp_core::crypto::{Ss58AddressFormat, Ss58Codec};
    use std::collections::btree_map::Entry;
    use std::collections::BTreeMap;

    fn first_pool() -> AccountId {
        AccountId::from_string("cnWjZubAFeX4AKJvfbjbxynVuyQ94vKezG35Lg2cGq7JSvEwd").unwrap()
    }

    fn second_pool() -> AccountId {
        AccountId::from_string("cnVqnjAiFxCEoe1XwEhyeYtkzMonNxDsKuLJb9QpwALBZLeHh").unwrap()
    }

    fn first_account() -> AccountId {
        AccountId::from_string("cnWtZSJUAVd4ZtqnkhbeCqoxnTEZQgapFb8vm2ixRZ5eYSFoc").unwrap()
    }

    fn second_account() -> AccountId {
        AccountId::from_string("cnUaRPDETnPGTuush6eGwFXyRPzE7wVSMSnFYNiJFSfDoKguZ").unwrap()
    }

    fn third_account() -> AccountId {
        AccountId::from_string("cnTyed7XCfdhrcqu45o3NaMWp4EPAR4x7saEsc28RHSdi9yEz").unwrap()
    }

    fn fourth_account() -> AccountId {
        AccountId::from_string("cnTLsumybNRsJ2iTmAvpM7peE6NXFJU4jc8GhkpJt999YFj6z").unwrap()
    }

    #[test]
    fn apply_pool_farmers() {
        fn prepare_pool_farmers() {
            PoolFarmers::<Runtime>::insert(
                first_pool(),
                vec![
                    PoolFarmer {
                        account: first_account(),
                        block: 1244402,
                        weight: 1000000000000000000,
                    },
                    PoolFarmer {
                        account: first_pool(),
                        block: 1244403,
                        weight: 1000000000000000000,
                    },
                ],
            );

            PoolFarmers::<Runtime>::insert(
                second_pool(),
                vec![PoolFarmer {
                    account: second_pool(),
                    block: 1244404,
                    weight: 1000000000000000000,
                }],
            );
        }

        fn assert_pool_farmers() {
            let farmers: Vec<_> = PoolFarmers::<Runtime>::iter().collect();
            assert_eq!(farmers.len(), 2);

            {
                let actual_farmers = farmers
                    .iter()
                    .find_map(|(pool, accounts)| {
                        if *pool == first_pool() {
                            Some(accounts)
                        } else {
                            None
                        }
                    })
                    .unwrap();
                let expected_farmers: Vec<PoolFarmer<Runtime>> = vec![
                    PoolFarmer {
                        account: first_account(),
                        block: 3600,
                        weight: 1000000000000000000,
                    },
                    PoolFarmer {
                        account: first_pool(),
                        block: 1244400,
                        weight: 1000000000000000000,
                    },
                ];
                assert_eq!(actual_farmers, &expected_farmers);
            }
            {
                let actual_farmers = farmers
                    .iter()
                    .find_map(|(pool, accounts)| {
                        if *pool == second_pool() {
                            Some(accounts)
                        } else {
                            None
                        }
                    })
                    .unwrap();
                let expected_farmers: Vec<PoolFarmer<Runtime>> = vec![PoolFarmer {
                    account: second_pool(),
                    block: 1244400,
                    weight: 1000000000000000000,
                }];
                assert_eq!(actual_farmers, &expected_farmers);
            }
        }

        let _ = env_logger::Builder::new()
            .filter_level(LevelFilter::Debug)
            .try_init();

        ExtBuilder::default().build().execute_with(|| {
            sp_core::crypto::set_default_ss58_version(Ss58AddressFormat::Custom(69));

            prepare_pool_farmers();

            let mut pools = BTreeMap::new();
            pools.insert(first_pool(), vec![(first_account(), 3600)]);
            super::apply_pool_farmers::<Runtime>(&pools);

            assert_pool_farmers();
        });
    }

    #[test]
    fn apply_rewards() {
        ExtBuilder::default().build().execute_with(|| {
            sp_core::crypto::set_default_ss58_version(Ss58AddressFormat::Custom(69));

            for account in &[first_account(), second_account()] {
                frame_system::Module::<Runtime>::inc_providers(&account);
            }

            let rewards = vec![
                (first_account(), balance!(1)),
                (second_account(), balance!(2)),
            ];
            super::apply_rewards::<Runtime>(rewards.clone());

            for (account, reward) in rewards {
                let reward_info = vested_rewards::Rewards::<Runtime>::get(&account);
                let actual_reward = *reward_info
                    .rewards
                    .get(&RewardReason::LiquidityProvisionFarming)
                    .unwrap();
                assert_eq!(actual_reward, reward);
            }
        });
    }

    #[test]
    fn apply_saved_values() {
        fn prepare_saved_values() {
            OldSavedValues::<Runtime>::insert(
                1245600,
                vec![
                    (
                        first_pool(),
                        vec![
                            PoolFarmer {
                                account: first_account(),
                                block: 1244402,
                                weight: 1000000000000000000,
                            },
                            PoolFarmer {
                                account: second_account(),
                                block: 1244403,
                                weight: 1000000000000000000,
                            },
                        ],
                    ),
                    (
                        second_pool(),
                        vec![
                            PoolFarmer {
                                account: first_account(),
                                block: 1244404,
                                weight: 1000000000000000000,
                            },
                            PoolFarmer {
                                account: second_account(),
                                block: 1244405,
                                weight: 1000000000000000000,
                            },
                        ],
                    ),
                ],
            );

            OldSavedValues::<Runtime>::insert(
                1249200,
                vec![(
                    first_pool(),
                    vec![
                        PoolFarmer {
                            account: first_account(),
                            block: 1244402,
                            weight: 1000000000000000000,
                        },
                        PoolFarmer {
                            account: second_account(),
                            block: 1245603,
                            weight: 1000000000000000000,
                        },
                        PoolFarmer {
                            account: fourth_account(),
                            block: 1245603,
                            weight: 1000000000000000000,
                        },
                    ],
                )],
            );
        }

        fn assert_rewards() {
            fn prepare_rewards_1245600() -> BTreeMap<AccountId, u128> {
                let mut accounts = BTreeMap::new();

                Pallet::<Runtime>::prepare_pool_accounts_for_vesting(
                    vec![
                        PoolFarmer {
                            account: first_account(),
                            block: 3600,
                            weight: 1000000000000000000,
                        },
                        PoolFarmer {
                            account: second_account(),
                            block: 3600,
                            weight: 1000000000000000000,
                        },
                    ],
                    1245600,
                    &mut accounts,
                );

                Pallet::<Runtime>::prepare_pool_accounts_for_vesting(
                    vec![
                        PoolFarmer {
                            account: first_account(),
                            block: 1244400,
                            weight: 1000000000000000000,
                        },
                        PoolFarmer {
                            account: second_account(),
                            block: 1244400,
                            weight: 1000000000000000000,
                        },
                    ],
                    1245600,
                    &mut accounts,
                );

                Pallet::<Runtime>::prepare_account_rewards(accounts)
            }

            fn prepare_rewards_1249200() -> BTreeMap<AccountId, u128> {
                let mut accounts = BTreeMap::new();

                Pallet::<Runtime>::prepare_pool_accounts_for_vesting(
                    vec![
                        PoolFarmer {
                            account: first_account(),
                            block: 3600,
                            weight: 1000000000000000000,
                        },
                        PoolFarmer {
                            account: second_account(),
                            block: 1245600,
                            weight: 1000000000000000000,
                        },
                        PoolFarmer {
                            account: fourth_account(),
                            block: 1245600,
                            weight: 1000000000000000000,
                        },
                    ],
                    1249200,
                    &mut accounts,
                );

                Pallet::<Runtime>::prepare_account_rewards(accounts)
            }

            fn reward(account: &AccountId) -> Option<u128> {
                let reward_info = vested_rewards::Rewards::<Runtime>::get(account);
                reward_info
                    .rewards
                    .get(&RewardReason::LiquidityProvisionFarming)
                    .cloned()
            }

            let rewards_125600 = prepare_rewards_1245600();
            let rewards_129200 = prepare_rewards_1249200();
            let mut rewards = rewards_125600;
            for (account, reward) in rewards_129200 {
                match rewards.entry(account) {
                    Entry::Vacant(entry) => {
                        entry.insert(reward);
                    }
                    Entry::Occupied(mut entry) => {
                        *entry.get_mut() += reward;
                    }
                }
            }

            for (account, actual_reward) in rewards {
                assert_eq!(Some(actual_reward), reward(&account));
            }

            assert_eq!(reward(&third_account()), None);
        }

        let _ = env_logger::Builder::new()
            .filter_level(LevelFilter::Debug)
            .try_init();

        ExtBuilder::default().build().execute_with(|| {
            sp_core::crypto::set_default_ss58_version(Ss58AddressFormat::Custom(69));

            prepare_saved_values();

            for account in &[
                first_account(),
                second_account(),
                third_account(),
                fourth_account(),
            ] {
                frame_system::Module::<Runtime>::inc_providers(&account);
            }

            let mut pools = BTreeMap::new();
            pools.insert(
                first_pool(),
                vec![
                    (first_account(), 3600),
                    (second_account(), 3600),
                    (third_account(), 3600),
                    (fourth_account(), 3600),
                ],
            );
            super::apply_saved_values::<Runtime>(&pools);

            // First account receives at 1245600 and 1249200 as it was from 3600
            // Second account receives at 1245600 as it was from 3600, but at 1249200 as it was from 1245603
            // Third account doesn't receive any rewards
            // Fourth account receives at 1249200 as it was from 1245601
            assert_rewards();
        });
    }
}
