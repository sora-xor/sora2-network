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

#![cfg_attr(not(feature = "std"), no_std)]
// TODO #167: fix clippy warnings
#![allow(clippy::all)]

use common::{
    balance, AssetInfoProvider, Balance, APOLLO_ASSET_ID, HERMES_ASSET_ID, PSWAP, VAL, XOR,
};
use frame_support::ensure;
use hex_literal::hex;
use sp_arithmetic::traits::Saturating;

mod benchmarking;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub mod weights;
pub use weights::WeightInfo;

type Assets<T> = assets::Pallet<T>;
type System<T> = frame_system::Pallet<T>;
type Technical<T> = technical::Pallet<T>;
type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;
type WeightInfoOf<T> = <T as Config>::WeightInfo;

pub const TECH_ACCOUNT_PREFIX: &[u8] = b"faucet";
pub const TECH_ACCOUNT_MAIN: &[u8] = b"main";
// Value to at least have enough funds for updating the limit
pub const DEFAULT_LIMIT: Balance = balance!(5);

pub fn transfer_limit_block_count<T: frame_system::Config>() -> BlockNumberOf<T> {
    14400u32.into()
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::pallet_prelude::*;
    use frame_support::traits::StorageVersion;
    use frame_system::pallet_prelude::*;
    use hex_literal::hex;
    use sp_core::H160;

    use common::AccountIdOf;
    use rewards::{PswapFarmOwners, PswapWaifuOwners, RewardInfo, ValOwners};

    use super::*;

    #[pallet::config]
    pub trait Config:
        frame_system::Config + assets::Config + rewards::Config + technical::Config
    {
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
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Transfers the specified amount of asset to the specified account.
        /// The supported assets are: XOR, VAL, PSWAP.
        ///
        /// # Errors
        ///
        /// AssetNotSupported is returned if `asset_id` is something the function doesn't support.
        /// AmountAboveLimit is returned if `target` has already received their daily limit of `asset_id`.
        /// NotEnoughReserves is returned if `amount` is greater than the reserves
        #[pallet::call_index(0)]
        #[pallet::weight((WeightInfoOf::<T>::transfer(), Pays::No))]
        pub fn transfer(
            _origin: OriginFor<T>,
            asset_id: T::AssetId,
            target: AccountIdOf<T>,
            amount: Balance,
        ) -> DispatchResultWithPostInfo {
            Self::ensure_asset_supported(asset_id)?;
            let block_number = System::<T>::block_number();
            let (block_number, taken_amount) =
                Self::prepare_transfer(&target, asset_id, amount, block_number)?;
            let reserves_tech_account_id = Self::reserves_account_id();
            let reserves_account_id =
                Technical::<T>::tech_account_id_to_account_id(&reserves_tech_account_id)?;
            let reserves_amount = Assets::<T>::total_balance(&asset_id, &reserves_account_id)?;
            ensure!(amount <= reserves_amount, Error::<T>::NotEnoughReserves);
            technical::Pallet::<T>::transfer_out(
                &asset_id,
                &reserves_tech_account_id,
                &target,
                amount,
            )?;
            Transfers::<T>::insert(target.clone(), asset_id, (block_number, taken_amount));
            Self::deposit_event(Event::Transferred(target, amount));
            Ok(().into())
        }

        #[pallet::call_index(1)]
        #[pallet::weight((WeightInfoOf::<T>::reset_rewards(), Pays::No))]
        pub fn reset_rewards(_origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            common::storage_remove_all!(ValOwners::<T>);
            ValOwners::<T>::insert(
                H160::from(hex!("21Bc9f4a3d9Dc86f142F802668dB7D908cF0A636")),
                RewardInfo::from(balance!(111)),
            );
            ValOwners::<T>::insert(
                H160::from(hex!("D67fea281B2C5dC3271509c1b628E0867a9815D7")),
                RewardInfo::from(balance!(444)),
            );

            common::storage_remove_all!(PswapFarmOwners::<T>);
            PswapFarmOwners::<T>::insert(
                H160::from(hex!("4fE143cDD48791cB364823A41e018AEC5cBb9AbB")),
                balance!(222),
            );
            PswapFarmOwners::<T>::insert(
                H160::from(hex!("D67fea281B2C5dC3271509c1b628E0867a9815D7")),
                balance!(555),
            );

            common::storage_remove_all!(PswapWaifuOwners::<T>);
            PswapWaifuOwners::<T>::insert(
                H160::from(hex!("886021F300dC809269CFC758A2364a2baF63af0c")),
                balance!(333),
            );

            Ok(().into())
        }

        #[pallet::call_index(2)]
        #[pallet::weight((WeightInfoOf::<T>::update_limit(), Pays::No))]
        pub fn update_limit(
            origin: OriginFor<T>,
            new_limit: Balance,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            TransferLimit::<T>::set(new_limit);
            Self::deposit_event(Event::LimitUpdated(new_limit));
            Ok(().into())
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        // The amount is transferred to the account. [account, amount]
        Transferred(AccountIdOf<T>, Balance),
        // Limit on transfer updated. [new_limit]
        LimitUpdated(Balance),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Asset is not supported.
        AssetNotSupported,
        /// Amount is above limit.
        AmountAboveLimit,
        /// Not enough reserves.
        NotEnoughReserves,
    }

    #[pallet::storage]
    #[pallet::getter(fn reserves_account_id)]
    pub(super) type ReservesAcc<T: Config> = StorageValue<_, T::TechAccountId, ValueQuery>;

    #[pallet::storage]
    pub(super) type Transfers<T: Config> = StorageDoubleMap<
        _,
        Identity,
        T::AccountId,
        Blake2_256,
        T::AssetId,
        (BlockNumberOf<T>, Balance),
    >;

    #[pallet::type_value]
    pub fn DefaultForTransferLimit<T: Config>() -> Balance {
        DEFAULT_LIMIT
    }

    #[pallet::storage]
    #[pallet::getter(fn transfer_limit)]
    pub(super) type TransferLimit<T: Config> =
        StorageValue<_, Balance, ValueQuery, DefaultForTransferLimit<T>>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub reserves_account_id: T::TechAccountId,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                reserves_account_id: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            ReservesAcc::<T>::put(&self.reserves_account_id);
        }
    }
}

impl<T: Config> Pallet<T> {
    fn ensure_asset_supported(asset_id: T::AssetId) -> Result<(), Error<T>> {
        let xor = XOR.into();
        let val = VAL.into();
        let pswap = PSWAP.into();
        let ceres = common::AssetId32::from_bytes(hex!(
            "008bcfd2387d3fc453333557eecb0efe59fcba128769b2feefdd306e98e66440"
        ))
        .into();

        if asset_id == xor
            || asset_id == val
            || asset_id == pswap
            || asset_id == ceres
            || asset_id == HERMES_ASSET_ID.into()
            || asset_id == APOLLO_ASSET_ID.into()
        {
            Ok(())
        } else {
            Err(Error::AssetNotSupported)
        }
    }

    /// Checks if new transfer is allowed, considering previous transfers.
    ///
    /// If new transfer is allowed, returns content to put in `Transfers` if the transfer is succeeded
    fn prepare_transfer(
        target: &T::AccountId,
        asset_id: T::AssetId,
        amount: Balance,
        current_block_number: BlockNumberOf<T>,
    ) -> Result<(BlockNumberOf<T>, Balance), Error<T>> {
        let balance_limit = Self::transfer_limit();
        ensure!(amount <= balance_limit, Error::AmountAboveLimit);
        if let Some((initial_block_number, taken_amount)) = Transfers::<T>::get(target, asset_id) {
            let transfer_limit_block_count = transfer_limit_block_count::<T>();
            if transfer_limit_block_count
                <= current_block_number.saturating_sub(initial_block_number)
            {
                // The previous transfer has happened a long time ago
                Ok((current_block_number, amount))
            } else if amount <= balance_limit.saturating_sub(taken_amount) {
                // Use `initial_block_number` because the previous transfer has happened recently.
                Ok((initial_block_number, taken_amount + amount))
            } else {
                Err(Error::<T>::AmountAboveLimit)
            }
        } else {
            Ok((current_block_number, amount))
        }
    }
}
