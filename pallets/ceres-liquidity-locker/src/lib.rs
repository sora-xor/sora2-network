#![cfg_attr(not(feature = "std"), no_std)]
// TODO #167: fix clippy warnings
#![allow(clippy::all)]

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub mod migrations;
pub mod weights;

use codec::{Decode, Encode};
use frame_support::weights::Weight;
pub use weights::WeightInfo;

#[derive(Encode, Decode, Default, PartialEq, Eq, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct LockInfo<Balance, Moment, AssetId> {
    /// Amount of locked pool tokens
    pub pool_tokens: Balance,
    /// The timestamp at which the tokens will be unlocked
    pub unlocking_timestamp: Moment,
    /// Base asset of locked liquidity
    pub asset_a: AssetId,
    /// Target asset of locked liquidity
    pub asset_b: AssetId,
}

/// Storage version.
#[derive(Encode, Decode, Eq, PartialEq, scale_info::TypeInfo)]
pub enum StorageVersion {
    /// Initial version
    V1,
    /// After migrating to timestamp calculation
    V2,
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use crate::{migrations, LockInfo, StorageVersion, WeightInfo};
    use common::prelude::{Balance, FixedWrapper};
    use common::{balance, DemeterFarming, XykPool};
    use frame_support::pallet_prelude::*;
    use frame_support::sp_runtime::traits::Zero;
    use frame_system::ensure_signed;
    use frame_system::pallet_prelude::*;
    use hex_literal::hex;
    use pallet_timestamp as timestamp;
    use sp_std::vec::Vec;

    #[pallet::config]
    pub trait Config: frame_system::Config + assets::Config + timestamp::Config {
        /// One day represented in block number
        const BLOCKS_PER_ONE_DAY: BlockNumberFor<Self>;

        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Reference to pool_xyk pallet
        type XYKPool: XykPool<Self::AccountId, Self::AssetId>;

        /// Reference to demeter_farming_platform pallet
        type DemeterFarmingPlatform: DemeterFarming<Self::AccountId, Self::AssetId>;

        /// Ceres asset id
        type CeresAssetId: Get<Self::AssetId>;

        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    type Assets<T> = assets::Pallet<T>;
    pub type Timestamp<T> = timestamp::Pallet<T>;
    pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
    pub type AssetIdOf<T> = <T as common::Config>::AssetId;

    #[pallet::pallet]
    #[pallet::generate_store(pub (super) trait Store)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::type_value]
    pub fn DefaultForFeesOptionOneAccount<T: Config>() -> AccountIdOf<T> {
        let bytes = hex!("96ea3c9c0be7bbc7b0656a1983db5eed75210256891a9609012362e36815b132");
        AccountIdOf::<T>::decode(&mut &bytes[..]).expect("Failed to decode account ID")
    }

    /// Account for collecting fees from Option 1
    #[pallet::storage]
    #[pallet::getter(fn fees_option_one_account)]
    pub type FeesOptionOneAccount<T: Config> =
        StorageValue<_, AccountIdOf<T>, ValueQuery, DefaultForFeesOptionOneAccount<T>>;

    #[pallet::type_value]
    pub fn DefaultForFeesOptionTwoAccount<T: Config>() -> AccountIdOf<T> {
        let bytes = hex!("0a0455d92e1fda8dee17b2c58761c8efca490ef2a1a03322dbfea7379481d517");
        AccountIdOf::<T>::decode(&mut &bytes[..]).expect("Failed to decode account ID")
    }

    /// Account for collecting fees from Option 2
    #[pallet::storage]
    #[pallet::getter(fn fees_option_two_account)]
    pub type FeesOptionTwoAccount<T: Config> =
        StorageValue<_, AccountIdOf<T>, ValueQuery, DefaultForFeesOptionTwoAccount<T>>;

    #[pallet::type_value]
    pub fn DefaultForOptionTwoCeresAmount<T: Config>() -> Balance {
        balance!(20)
    }

    /// Amount of CERES for locker fees option two
    #[pallet::storage]
    #[pallet::getter(fn fees_option_two_ceres_amount)]
    pub type FeesOptionTwoCeresAmount<T: Config> =
        StorageValue<_, Balance, ValueQuery, DefaultForOptionTwoCeresAmount<T>>;

    #[pallet::type_value]
    pub fn DefaultForAuthorityAccount<T: Config>() -> AccountIdOf<T> {
        let bytes = hex!("34a5b78f5fbcdc92a28767d63b579690a4b2f6a179931b3ecc87f09fc9366d47");
        AccountIdOf::<T>::decode(&mut &bytes[..]).expect("Failed to decode account ID")
    }

    /// Account which has permissions for changing CERES amount fee
    #[pallet::storage]
    #[pallet::getter(fn authority_account)]
    pub type AuthorityAccount<T: Config> =
        StorageValue<_, AccountIdOf<T>, ValueQuery, DefaultForAuthorityAccount<T>>;

    #[pallet::type_value]
    pub fn DefaultForPalletStorageVersion<T: Config>() -> StorageVersion {
        StorageVersion::V1
    }

    /// Pallet storage version
    #[pallet::storage]
    #[pallet::getter(fn pallet_storage_version)]
    pub type PalletStorageVersion<T: Config> =
        StorageValue<_, StorageVersion, ValueQuery, DefaultForPalletStorageVersion<T>>;

    /// Contains data about lockups for each account
    #[pallet::storage]
    #[pallet::getter(fn locker_data)]
    pub type LockerData<T: Config> = StorageMap<
        _,
        Identity,
        AccountIdOf<T>,
        Vec<LockInfo<Balance, T::Moment, AssetIdOf<T>>>,
        ValueQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Funds Locked [who, amount, timestamp]
        Locked(AccountIdOf<T>, Balance, T::Moment),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Pool does not exist
        PoolDoesNotExist,
        /// Insufficient liquidity to lock
        InsufficientLiquidityToLock,
        /// Percentage greater than 100%
        InvalidPercentage,
        /// Unauthorized access
        Unauthorized,
        /// Unlocking date cannot be in past
        InvalidUnlockingTimestamp,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Lock liquidity
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::lock_liquidity())]
        pub fn lock_liquidity(
            origin: OriginFor<T>,
            asset_a: AssetIdOf<T>,
            asset_b: AssetIdOf<T>,
            unlocking_timestamp: T::Moment,
            percentage_of_pool_tokens: Balance,
            option: bool,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;
            ensure!(
                percentage_of_pool_tokens <= balance!(1),
                Error::<T>::InvalidPercentage
            );

            // Get current timestamp
            let current_timestamp = Timestamp::<T>::get();
            ensure!(
                unlocking_timestamp > current_timestamp,
                Error::<T>::InvalidUnlockingTimestamp
            );

            let mut lock_info = LockInfo {
                pool_tokens: 0,
                asset_a,
                asset_b,
                unlocking_timestamp,
            };

            // Get pool account
            let pool_account: AccountIdOf<T> = T::XYKPool::properties_of_pool(asset_a, asset_b)
                .ok_or(Error::<T>::PoolDoesNotExist)?
                .0;

            // Calculate number of pool tokens to be locked
            let mut pool_tokens =
                T::XYKPool::balance_of_pool_provider(pool_account.clone(), user.clone())
                    .unwrap_or(0);
            if pool_tokens == 0 {
                return Err(Error::<T>::InsufficientLiquidityToLock.into());
            }

            lock_info.pool_tokens = (FixedWrapper::from(pool_tokens)
                * FixedWrapper::from(percentage_of_pool_tokens))
            .try_into_balance()
            .unwrap_or(lock_info.pool_tokens);

            // Check if user has enough liquidity to lock
            let lockups = <LockerData<T>>::get(&user);
            let mut locked_pool_tokens = 0;

            for locks in lockups.iter() {
                if locks.asset_a == asset_a && locks.asset_b == asset_b {
                    if current_timestamp < locks.unlocking_timestamp {
                        locked_pool_tokens = locked_pool_tokens + locks.pool_tokens;
                    }
                }
            }

            let unlocked_pool_tokens = pool_tokens - locked_pool_tokens;
            ensure!(
                lock_info.pool_tokens <= unlocked_pool_tokens,
                Error::<T>::InsufficientLiquidityToLock
            );

            // Pay Locker fees
            if option {
                // Transfer 1% of LP tokens
                Self::pay_fee_in_lp_tokens(
                    pool_account.clone(),
                    asset_a,
                    asset_b,
                    user.clone(),
                    lock_info.pool_tokens,
                    FixedWrapper::from(0.01),
                    option,
                )?;
            } else {
                // Transfer CERES fee amount
                Assets::<T>::transfer_from(
                    &T::CeresAssetId::get().into(),
                    &user,
                    &FeesOptionTwoAccount::<T>::get(),
                    FeesOptionTwoCeresAmount::<T>::get(),
                )?;
                // Transfer 0.5% of LP tokens
                Self::pay_fee_in_lp_tokens(
                    pool_account.clone(),
                    asset_a,
                    asset_b,
                    user.clone(),
                    lock_info.pool_tokens,
                    FixedWrapper::from(0.005),
                    option,
                )?;
            }

            pool_tokens = T::XYKPool::balance_of_pool_provider(pool_account.clone(), user.clone())
                .unwrap_or(0);
            T::DemeterFarmingPlatform::update_pool_tokens(
                user.clone(),
                pool_tokens,
                asset_a,
                asset_b,
            )?;

            // Put updated address info into storage
            // Get lock info of extrinsic caller
            <LockerData<T>>::append(&user, lock_info);

            // Emit an event
            Self::deposit_event(Event::Locked(
                user,
                percentage_of_pool_tokens,
                current_timestamp,
            ));

            // Return a successful DispatchResult
            Ok(().into())
        }

        /// Change CERES fee
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::change_ceres_fee())]
        pub fn change_ceres_fee(
            origin: OriginFor<T>,
            ceres_fee: Balance,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            if user != AuthorityAccount::<T>::get() {
                return Err(Error::<T>::Unauthorized.into());
            }

            FeesOptionTwoCeresAmount::<T>::put(ceres_fee);
            Ok(().into())
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(now: T::BlockNumber) -> Weight {
            let mut counter: u64 = 0;

            if (now % T::BLOCKS_PER_ONE_DAY).is_zero() {
                let current_timestamp = Timestamp::<T>::get();
                for (account_id, mut lockups) in <LockerData<T>>::iter() {
                    let mut expired_locks = Vec::new();

                    // Save expired lock
                    for (index, lock) in lockups.iter().enumerate() {
                        if lock.unlocking_timestamp <= current_timestamp {
                            expired_locks.push(index);
                        }
                    }

                    for index in expired_locks.iter().rev() {
                        lockups.remove(*index);
                        counter += 1;
                    }

                    <LockerData<T>>::insert(account_id, lockups);
                }
            }

            T::DbWeight::get()
                .reads(1)
                .saturating_add(T::DbWeight::get().writes(counter))
        }

        fn on_runtime_upgrade() -> Weight {
            if Self::pallet_storage_version() == StorageVersion::V1 {
                let weight = migrations::migrate::<T>();
                PalletStorageVersion::<T>::put(StorageVersion::V2);
                weight
            } else {
                Weight::zero()
            }
        }
    }

    impl<T: Config> Pallet<T> {
        /// Check if user has enough unlocked liquidity for withdrawing
        pub fn check_if_has_enough_unlocked_liquidity(
            user: &AccountIdOf<T>,
            asset_a: AssetIdOf<T>,
            asset_b: AssetIdOf<T>,
            withdrawing_amount: Balance,
        ) -> bool {
            // Get lock info of extrinsic caller
            let lockups = <LockerData<T>>::get(&user);
            let current_timestamp = Timestamp::<T>::get();

            // Get pool account
            let pool_account: AccountIdOf<T> =
                if let Some(account) = T::XYKPool::properties_of_pool(asset_a, asset_b) {
                    account.0
                } else {
                    return false;
                };

            // Calculate number of pool tokens to be locked
            let pool_tokens =
                T::XYKPool::balance_of_pool_provider(pool_account.clone(), user.clone())
                    .unwrap_or(0);
            if pool_tokens == 0 {
                return false;
            }

            let mut locked_pool_tokens = 0;
            for locks in lockups.iter() {
                if locks.asset_a == asset_a && locks.asset_b == asset_b {
                    if current_timestamp < locks.unlocking_timestamp {
                        locked_pool_tokens = locked_pool_tokens + locks.pool_tokens;
                    }
                }
            }
            let unlocked_pool_tokens = pool_tokens.checked_sub(locked_pool_tokens).unwrap_or(0);

            if withdrawing_amount > pool_tokens || unlocked_pool_tokens >= withdrawing_amount {
                true
            } else {
                false
            }
        }

        /// Pay Locker fees in LP tokens
        fn pay_fee_in_lp_tokens(
            pool_account: AccountIdOf<T>,
            asset_a: AssetIdOf<T>,
            asset_b: AssetIdOf<T>,
            user: AccountIdOf<T>,
            pool_tokens: Balance,
            fee_percentage: FixedWrapper,
            option: bool,
        ) -> Result<(), DispatchError> {
            let pool_tokens = (FixedWrapper::from(pool_tokens) * fee_percentage)
                .try_into_balance()
                .unwrap_or(0);

            let fee_account = if option {
                FeesOptionOneAccount::<T>::get()
            } else {
                FeesOptionTwoAccount::<T>::get()
            };

            let result = T::XYKPool::transfer_lp_tokens(
                pool_account,
                asset_a,
                asset_b,
                user,
                fee_account,
                pool_tokens,
            );
            return result;
        }
    }
}
