// This file is part of the SORA network and Polkaswap app.

// Copyright (c) 2020, 2021, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

// Redistribution and use in source and binary forms, with or without modification,
// are permitted provided that the following conditions are met:

// Redistributions of source code must retain the above copyright notice, this list
// of conditions and the following disclaimer.
// Redistributions in binary form must reproduce the above copyright notice, this
// list of conditions and the following disclaimer in the documentation and/or other
// materials provided with the distribution.
//
// All advertising materials mentioning features or use of this software must display
// the following acknowledgement: This product includes software developed by Polka Biome
// Ltd., SORA, and Polkaswap.
//
// Neither the name of the Polka Biome Ltd. nor the names of its contributors may be used
// to endorse or promote products derived from this software without specific prior written permission.

// THIS SOFTWARE IS PROVIDED BY Polka Biome Ltd. AS IS AND ANY EXPRESS OR IMPLIED WARRANTIES,
// INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL Polka Biome Ltd. BE LIABLE FOR ANY
// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
// BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS;
// OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
// USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

//! This pallet enables users to claim their rewards.
//!
//! There are following kinds of rewards:
//! * VAL for XOR owners
//! * PSWAP farming
//! * PSWAP NFT waifus

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::codec::{Decode, Encode};
use frame_support::dispatch::DispatchErrorWithPostInfo;
use frame_support::storage::StorageMap as StorageMapTrait;
use frame_support::RuntimeDebug;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::traits::{UniqueSaturatedInto, Zero};
use sp_runtime::{Perbill, Percent};
use sp_std::prelude::*;

use assets::AssetIdOf;
#[cfg(feature = "std")]
use common::balance;
use common::eth::EthAddress;
use common::prelude::FixedWrapper;
#[cfg(feature = "include-real-files")]
use common::vec_push;
use common::{eth, AccountIdOf, Balance, OnValBurned, VAL};

#[cfg(feature = "include-real-files")]
use hex_literal::hex;

pub use self::pallet::*;

pub mod weights;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

type WeightInfoOf<T> = <T as Config>::WeightInfo;

#[derive(Encode, Decode, Clone, RuntimeDebug, Default, PartialEq, Eq, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct RewardInfo {
    claimable: Balance,
    pub total: Balance,
}

impl RewardInfo {
    pub fn new(claimable: Balance, total: Balance) -> Self {
        Self { claimable, total }
    }
}

impl From<(Balance, Balance)> for RewardInfo {
    fn from(value: (Balance, Balance)) -> Self {
        RewardInfo::new(value.0, value.1)
    }
}

impl From<Balance> for RewardInfo {
    fn from(value: Balance) -> Self {
        RewardInfo::new(value, 0)
    }
}

pub const TECH_ACCOUNT_PREFIX: &[u8] = b"rewards";
pub const TECH_ACCOUNT_MAIN: &[u8] = b"main";

pub use weights::WeightInfo;

impl<T: Config> Pallet<T> {
    /// Get available rewards for a specified `eth_address`:
    /// - VAL
    /// - PSWAP Farming
    /// - PSWAP Waifu
    /// The rest are UMI NFTS.
    /// Returns the vector of available reward amounts.
    /// Interacts with `ValOwners`, `PswapFarmOwners`, `PswapWaifuOwners`, and `UmiNftReceivers`
    /// StorageMaps.
    ///
    /// Used in `claimables` RPC endpoint.
    ///
    /// - `eth_address`: address of an ETH account associated with the rewards
    pub fn claimables(eth_address: &EthAddress) -> Vec<Balance> {
        let mut res = vec![
            ValOwners::<T>::get(eth_address).claimable,
            PswapFarmOwners::<T>::get(eth_address),
            PswapWaifuOwners::<T>::get(eth_address),
        ];
        res.append(&mut UmiNftReceivers::<T>::get(eth_address));
        res
    }

    /// Calculate current vesting ratio for a given `elapsed` time.
    /// Returns the vesting ratio.
    /// Does not interact with the storage.
    ///
    /// Used in `on_initialize` hook.
    ///
    /// - `elapsed`: elapsed time in blocks
    fn current_vesting_ratio(elapsed: T::BlockNumber) -> Perbill {
        let max_percentage = T::MAX_VESTING_RATIO.deconstruct() as u32;
        if elapsed >= T::TIME_TO_SATURATION {
            Perbill::from_percent(max_percentage)
        } else {
            let elapsed_u32: u32 = elapsed.unique_saturated_into();
            let time_to_saturation: u32 = T::TIME_TO_SATURATION.unique_saturated_into();
            Perbill::from_rational(max_percentage * elapsed_u32, 100_u32 * time_to_saturation)
        }
    }

    /// Claims the reward for an account with specified `eth_address` and transfers the reward to
    /// the specified `account_id`.
    /// Does not directly return errors.
    /// Interacts with the specified `M` StorageMap.
    ///
    /// Used in `claim` extrinsic.
    ///
    /// - `eth_address`: The ETH address associated with the specified account
    /// - `account_id`: The account ID associated with the reward
    /// - `asset_id`: The reward's asset ID
    /// - `reserves_acc`: Technical account holding unclaimed rewards
    /// - `claimed`: Flag indicating whether the reward has been claimed
    /// - `is_eligible`: Flag indicating whether the account is eligible for the reward
    fn claim_reward<M: StorageMapTrait<EthAddress, Balance>>(
        eth_address: &EthAddress,
        account_id: &AccountIdOf<T>,
        asset_id: &AssetIdOf<T>,
        reserves_acc: &T::TechAccountId,
        claimed: &mut bool,
        is_eligible: &mut bool,
    ) -> Result<(), DispatchErrorWithPostInfo> {
        if let Ok(balance) = M::try_get(eth_address) {
            *is_eligible = true;
            if balance > 0 {
                technical::Pallet::<T>::transfer_out(asset_id, reserves_acc, account_id, balance)?;
                M::insert(eth_address, 0);
                *claimed = true;
            }
        }
        Ok(())
    }

    /// Claims the VAL reward for an account with specified `eth_address` and transfers the reward to
    /// the specified `account_id` if the `eth_address` is present in `ValOwners`.
    /// Does not directly return errors.
    /// Interacts with the `ValOwners` StorageMap and the `TotalValRewards`, `TotalClaimableVal` StorageValues.
    ///
    /// Used in `claim` extrinsic.
    ///
    /// - `eth_address`: The ETH address associated with the specified account
    /// - `account_id`: The account ID associated with the reward
    /// - `reserves_acc`: Technical account holding unclaimed rewards
    /// - `claimed`: Flag indicating whether the reward has been claimed
    /// - `is_eligible`: Flag indicating whether the account is eligible for the reward
    fn claim_val_reward(
        eth_address: &EthAddress,
        account_id: &AccountIdOf<T>,
        reserves_acc: &T::TechAccountId,
        claimed: &mut bool,
        is_eligible: &mut bool,
    ) -> Result<(), DispatchErrorWithPostInfo> {
        if let Ok(RewardInfo {
            claimable: amount,
            total,
        }) = ValOwners::<T>::try_get(eth_address)
        {
            *is_eligible = true;
            if amount > 0 {
                technical::Pallet::<T>::transfer_out(
                    &VAL.into(),
                    reserves_acc,
                    account_id,
                    amount,
                )?;
                ValOwners::<T>::mutate(eth_address, |v| {
                    *v = RewardInfo::new(0, total.saturating_sub(amount))
                });
                TotalValRewards::<T>::mutate(|v| *v = v.saturating_sub(amount));
                TotalClaimableVal::<T>::mutate(|v| *v = v.saturating_sub(amount));
                *claimed = true;
            }
        }
        Ok(())
    }

    /// Claims the UMI NFTs for an account with specified `eth_address` and transfers the NFTs to
    /// the specified `account_id` if the `eth_address` is present in `UmiNftReceivers`.
    /// Does not directly return errors.
    /// Interacts with the `UmiNftReceivers` StorageMap and the `UmiNfts` StorageValue.
    ///
    /// Used in `claim` extrinsic.
    ///
    /// - `eth_address`: The ETH address associated with the specified account
    /// - `account_id`: The account ID associated with the reward
    /// - `reserves_acc`: Technical account holding unclaimed rewards
    /// - `claimed`: Flag indicating whether the reward has been claimed
    /// - `is_eligible`: Flag indicating whether the account is eligible for the reward
    fn claim_umi_nfts(
        eth_address: &EthAddress,
        account_id: &AccountIdOf<T>,
        reserves_acc: &T::TechAccountId,
        claimed: &mut bool,
        is_eligible: &mut bool,
    ) -> Result<(), DispatchErrorWithPostInfo> {
        if let Ok(rewards) = UmiNftReceivers::<T>::try_get(eth_address) {
            *is_eligible = true;
            let mut updated_balances = rewards.clone();
            let nfts = UmiNfts::<T>::get();

            for (n, balance) in rewards.iter().enumerate() {
                if *balance > 0 {
                    let asset_id = nfts[n];
                    technical::Pallet::<T>::transfer_out(
                        &asset_id,
                        reserves_acc,
                        account_id,
                        *balance,
                    )?;
                    updated_balances[n] = 0;
                    *claimed = true;
                } else {
                    *claimed = false;
                }
            }

            UmiNftReceivers::<T>::insert(eth_address, updated_balances);
            UmiNftClaimed::<T>::insert(eth_address, claimed);
        }
        Ok(())
    }

    /// Adds the specified ETH address to the list of UMI NFT receivers.
    /// Does not directly return errors.
    /// Interacts with the `UmiNftReceivers`, `UmiNftClaimed` StorageMaps and the `UmiNfts` StorageValue.
    ///
    /// Used in `claim` extrinsic.
    ///
    /// - `receiver`: The ETH address added to the list of UMI NFT receivers
    fn add_umi_nft_receiver(receiver: &EthAddress) -> Result<(), DispatchErrorWithPostInfo> {
        if !UmiNftClaimed::<T>::get(receiver) {
            UmiNftReceivers::<T>::insert(receiver, vec![1; UmiNfts::<T>::get().len()]);
        }
        Ok(())
    }
}

impl<T: Config> OnValBurned for Pallet<T> {
    fn on_val_burned(amount: Balance) {
        ValBurnedSinceLastVesting::<T>::mutate(|v| {
            *v = v.saturating_add(amount.saturating_sub(T::VAL_BURN_PERCENT * amount))
        });
    }
}

#[frame_support::pallet]
pub mod pallet {
    use common::PSWAP;
    use frame_support::pallet_prelude::*;
    use frame_support::traits::StorageVersion;
    use frame_support::transactional;
    use frame_system::pallet_prelude::*;
    use secp256k1::util::SIGNATURE_SIZE;
    use secp256k1::{RecoveryId, Signature};
    use sp_std::vec::Vec;

    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + assets::Config + technical::Config {
        /// How often the rewards data are being updated
        const UPDATE_FREQUENCY: BlockNumberFor<Self>;
        /// Vested amount is updated every `BLOCKS_PER_DAY` blocks
        const BLOCKS_PER_DAY: BlockNumberFor<Self>;
        /// Max number of addresses to be processed in one take
        const MAX_CHUNK_SIZE: usize;
        /// Max percentage of daily burned VAL that can be vested as rewards
        const MAX_VESTING_RATIO: Percent;
        /// The amount of time until vesting ratio reaches saturation at `MAX_VESTING_RATIO`
        const TIME_TO_SATURATION: BlockNumberFor<Self>;
        /// Percentage of VAL burned without vesting
        const VAL_BURN_PERCENT: Percent;
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type WeightInfo: WeightInfo;
    }

    /// The current storage version.
    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(now: T::BlockNumber) -> Weight {
            let mut consumed_weight: Weight = Weight::zero();

            if (now % T::BLOCKS_PER_DAY).is_zero() {
                if TotalValRewards::<T>::get() == TotalClaimableVal::<T>::get() {
                    // All VAL has been vested
                    CurrentClaimableVal::<T>::put(0);
                    return T::DbWeight::get().reads_writes(2, 1);
                }

                let val_burned = ValBurnedSinceLastVesting::<T>::get();
                let vesting_ratio = Self::current_vesting_ratio(now);
                let vested_amount = vesting_ratio * val_burned;
                CurrentClaimableVal::<T>::put(vested_amount);
                ValBurnedSinceLastVesting::<T>::put(0);

                consumed_weight += T::DbWeight::get().reads_writes(3, 3);
            }

            if (now % T::UPDATE_FREQUENCY).is_zero() {
                let total_rewards = TotalValRewards::<T>::get();
                if total_rewards == 0 {
                    return consumed_weight + T::DbWeight::get().reads(1);
                }

                let current_claimable = CurrentClaimableVal::<T>::get();
                if current_claimable == 0 {
                    return consumed_weight + T::DbWeight::get().reads(1);
                }
                consumed_weight += T::DbWeight::get().reads(2);

                let batch_index: u32 =
                    ((now % T::BLOCKS_PER_DAY) / T::UPDATE_FREQUENCY).unique_saturated_into();
                if let Ok(addresses) = EthAddresses::<T>::try_get(batch_index) {
                    let wrapped_current_claimable = FixedWrapper::from(current_claimable);
                    let wrapped_total_rewards = FixedWrapper::from(total_rewards);

                    let coeff = wrapped_current_claimable / wrapped_total_rewards;

                    addresses.iter().for_each(|addr| {
                        let RewardInfo { claimable, total } = ValOwners::<T>::get(addr);
                        let amount = (FixedWrapper::from(total) * coeff.clone())
                            .try_into_balance()
                            .unwrap_or(0);
                        let new_claimable = total.min(claimable.saturating_add(amount));
                        let amount = new_claimable - claimable;
                        ValOwners::<T>::mutate(addr, |v| {
                            *v = RewardInfo::new(new_claimable, total);
                        });
                        TotalClaimableVal::<T>::mutate(|v| *v = v.saturating_add(amount));
                        consumed_weight += T::DbWeight::get().reads_writes(2, 1);
                    });
                };
            }

            consumed_weight
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Claim the reward with signature.
        #[transactional]
        #[pallet::call_index(0)]
        #[pallet::weight(WeightInfoOf::<T>::claim())]
        pub fn claim(origin: OriginFor<T>, signature: Vec<u8>) -> DispatchResultWithPostInfo {
            let account_id = ensure_signed(origin)?;
            ensure!(
                signature.len() == SIGNATURE_SIZE + 1,
                Error::<T>::SignatureInvalid
            );
            let recovery_id = if signature[SIGNATURE_SIZE] >= 27 {
                signature[SIGNATURE_SIZE] - 27
            } else {
                signature[SIGNATURE_SIZE]
            };
            let recovery_id = RecoveryId::parse(recovery_id)
                .map_err(|_| Error::<T>::SignatureVerificationFailed)?;
            let signature = Signature::parse_standard_slice(&signature[..SIGNATURE_SIZE])
                .map_err(|_| Error::<T>::SignatureInvalid)?;
            let message = eth::prepare_message(&account_id.encode());
            let public_key = secp256k1::recover(&message, &signature, &recovery_id)
                .map_err(|_| Error::<T>::SignatureVerificationFailed)?;
            let eth_address = eth::public_key_to_eth_address(&public_key);
            let reserves_acc = ReservesAcc::<T>::get();
            let mut claimed = false;
            let mut is_eligible = false;
            Self::claim_val_reward(
                &eth_address,
                &account_id,
                &reserves_acc,
                &mut claimed,
                &mut is_eligible,
            )?;
            Self::claim_reward::<PswapFarmOwners<T>>(
                &eth_address,
                &account_id,
                &PSWAP.into(),
                &reserves_acc,
                &mut claimed,
                &mut is_eligible,
            )?;
            Self::claim_reward::<PswapWaifuOwners<T>>(
                &eth_address,
                &account_id,
                &PSWAP.into(),
                &reserves_acc,
                &mut claimed,
                &mut is_eligible,
            )?;
            Self::claim_umi_nfts(
                &eth_address,
                &account_id,
                &reserves_acc,
                &mut claimed,
                &mut is_eligible,
            )?;
            if claimed {
                Self::deposit_event(Event::<T>::Claimed(account_id));
                Ok(().into())
            } else if is_eligible {
                Err(Error::<T>::NothingToClaim.into())
            } else {
                Err(Error::<T>::AddressNotEligible.into())
            }
        }

        /// Finalize the update of unclaimed VAL data in storage
        /// Add addresses, who will receive UMI NFT rewards.
        #[transactional]
        #[pallet::call_index(1)]
        #[pallet::weight((WeightInfoOf::<T>::add_umi_nfts_receivers(receivers.len() as u32), Pays::No))]
        pub fn add_umi_nft_receivers(
            origin: OriginFor<T>,
            receivers: Vec<EthAddress>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            for address in receivers {
                Self::add_umi_nft_receiver(&address)?;
            }
            Ok(().into())
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// The account has claimed their rewards. [account]
        Claimed(AccountIdOf<T>),
        /// Storage migration to version 1.2.0 completed
        MigrationCompleted,
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The account has no claimable rewards at the time of claiming request
        NothingToClaim,
        /// Address is not eligible for any rewards
        AddressNotEligible,
        /// The signature is invalid
        SignatureInvalid,
        /// The signature verification failed
        SignatureVerificationFailed,
        /// Occurs if an attempt to repeat the unclaimed VAL data update is made
        IllegalCall,
    }

    #[pallet::storage]
    pub type ReservesAcc<T: Config> = StorageValue<_, T::TechAccountId, ValueQuery>;

    /// A map EthAddresses -> RewardInfo, that is claimable and remaining vested amounts per address
    #[pallet::storage]
    pub type ValOwners<T: Config> = StorageMap<_, Identity, EthAddress, RewardInfo, ValueQuery>;

    #[pallet::storage]
    pub type PswapFarmOwners<T: Config> = StorageMap<_, Identity, EthAddress, Balance, ValueQuery>;

    #[pallet::storage]
    pub type PswapWaifuOwners<T: Config> = StorageMap<_, Identity, EthAddress, Balance, ValueQuery>;

    /// UMI NFT receivers storage
    #[pallet::storage]
    pub type UmiNftReceivers<T: Config> =
        StorageMap<_, Identity, EthAddress, Vec<Balance>, ValueQuery>;

    /// Amount of VAL burned since last vesting
    #[pallet::storage]
    pub type ValBurnedSinceLastVesting<T: Config> = StorageValue<_, Balance, ValueQuery>;

    /// Amount of VAL currently being vested (aggregated over the previous period of 14,400 blocks)
    #[pallet::storage]
    pub type CurrentClaimableVal<T: Config> = StorageValue<_, Balance, ValueQuery>;

    /// All addresses are split in batches, `AddressBatches` maps batch number to a set of addresses
    #[pallet::storage]
    pub type EthAddresses<T: Config> = StorageMap<_, Identity, u32, Vec<EthAddress>, ValueQuery>;

    /// Total amount of VAL rewards either claimable now or some time in the future
    #[pallet::storage]
    pub type TotalValRewards<T: Config> = StorageValue<_, Balance, ValueQuery>;

    /// Total amount of VAL that can be claimed by users at current point in time
    #[pallet::storage]
    pub type TotalClaimableVal<T: Config> = StorageValue<_, Balance, ValueQuery>;

    /// A flag indicating whether VAL rewards data migration has been finalized
    #[pallet::storage]
    pub type MigrationPending<T: Config> = StorageValue<_, bool, ValueQuery>;

    /// The storage of available UMI NFTs.
    #[pallet::storage]
    pub type UmiNfts<T: Config> = StorageValue<_, Vec<T::AssetId>, ValueQuery>;

    /// Stores whether address already claimed UMI NFT rewards.
    #[pallet::storage]
    pub type UmiNftClaimed<T: Config> = StorageMap<_, Identity, EthAddress, bool, ValueQuery>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub reserves_account_id: T::TechAccountId,
        pub val_owners: Vec<(EthAddress, RewardInfo)>,
        pub pswap_farm_owners: Vec<(EthAddress, Balance)>,
        pub pswap_waifu_owners: Vec<(EthAddress, Balance)>,
        pub umi_nfts: Vec<T::AssetId>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                reserves_account_id: Default::default(),
                val_owners: Default::default(),
                pswap_farm_owners: Default::default(),
                pswap_waifu_owners: Default::default(),
                umi_nfts: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            ReservesAcc::<T>::put(&self.reserves_account_id);

            // Split the addresses in groups to avoid updating all rewards within a single block
            let mut iter = self.val_owners.chunks(T::MAX_CHUNK_SIZE);
            let mut batch_index: u32 = 0;
            while let Some(chunk) = iter.next() {
                EthAddresses::<T>::insert(
                    batch_index,
                    chunk
                        .iter()
                        .cloned()
                        .map(|(addr, _)| addr)
                        .collect::<Vec<_>>(),
                );
                batch_index += 1;
            }

            let mut total = balance!(0);
            let mut claimable = balance!(0);
            self.val_owners.iter().for_each(|(owner, value)| {
                ValOwners::<T>::insert(owner, value);
                claimable = claimable.saturating_add(value.claimable);
                total = total.saturating_add(value.total);
            });
            TotalValRewards::<T>::put(total);
            TotalClaimableVal::<T>::put(claimable);
            CurrentClaimableVal::<T>::put(balance!(0));
            ValBurnedSinceLastVesting::<T>::put(balance!(0));

            self.pswap_farm_owners.iter().for_each(|(owner, balance)| {
                PswapFarmOwners::<T>::insert(owner, balance);
            });
            self.pswap_waifu_owners.iter().for_each(|(owner, balance)| {
                PswapWaifuOwners::<T>::insert(owner, balance);
            });
            self.umi_nfts.iter().for_each(UmiNfts::<T>::append);
        }
    }
}
