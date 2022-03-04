#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use common::Balance;

#[derive(Encode, Decode, Default, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct PoolInfo {
    multiplier: u32,
    deposit_fee: Balance,
    is_core: bool,
    is_farm: bool,
    total_tokens_in_pool: Balance,
    rewards: Balance,
    is_removed: bool,
}

#[derive(Encode, Decode, Default, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct TokenInfo {
    farms_total_multiplier: u32,
    staking_total_multiplier: u32,
    token_per_block: Balance,
    farms_allocation: Balance,
    staking_allocation: Balance,
    team_allocation: Balance,
}

#[derive(Encode, Decode, Default, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct UserInfo<AssetId> {
    pool_asset: AssetId,
    reward_asset: AssetId,
    is_farm: bool,
    pooled_tokens: Balance,
    rewards: Balance,
}

#[frame_support::pallet]
pub mod pallet {
    use crate::{PoolInfo, TokenInfo, UserInfo};
    use common::prelude::Balance;
    use frame_support::pallet_prelude::*;
    use frame_support::traits::Vec;
    use frame_system::pallet_prelude::*;
    use sp_runtime::traits::AccountIdConversion;
    use sp_runtime::ModuleId;

    const PALLET_ID: ModuleId = ModuleId(*b"deofarms");

    #[pallet::config]
    pub trait Config: frame_system::Config + assets::Config + technical::Config {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
    }

    type Assets<T> = assets::Pallet<T>;
    pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
    type AssetIdOf<T> = <T as assets::Config>::AssetId;

    #[pallet::pallet]
    #[pallet::generate_store(pub (super) trait Store)]
    pub struct Pallet<T>(PhantomData<T>);

    /// A vote of a particular user for a particular poll
    #[pallet::storage]
    #[pallet::getter(fn token_info)]
    pub type TokenInfos<T: Config> = StorageMap<_, Identity, AssetIdOf<T>, TokenInfo, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn user_info)]
    pub type UserInfos<T: Config> =
        StorageMap<_, Identity, AccountIdOf<T>, Vec<UserInfo<AssetIdOf<T>>>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn pools)]
    pub type Pools<T: Config> = StorageDoubleMap<
        _,
        Identity,
        AssetIdOf<T>,
        Identity,
        AssetIdOf<T>,
        Vec<PoolInfo>,
        ValueQuery,
    >;

    #[pallet::event]
    #[pallet::metadata(AccountIdOf<T> = "AccountId", BalanceOf<T> = "Balance", AssetIdOf<T> = "AssetId")]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Token registered [who, what]
        TokenRegistered(AccountIdOf<T>, AssetIdOf<T>),
        /// Pool added [who, pool_asset, reward_asset, is_farm]
        PoolAdded(AccountIdOf<T>, AssetIdOf<T>, AssetIdOf<T>, bool),
        /// Ceres deposited. [who, amount]
        RewardWithdrawn(AccountIdOf<T>, Balance),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Token is already registered
        TokenAlreadyRegistered,
        /// Token per block can't be zero
        TokenPerBlockCantBeZero,
        /// Invalid allocation parameters
        InvalidAllocationParameters,
        /// Multiplier must be greater or equal to 1
        InvalidMultiplier,
        /// Token is not registered
        RewardTokenIsNotRegistered,
        /// Pool already exists
        PoolAlreadyExists,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Register token for farming
        #[pallet::weight(10000)]
        pub fn register_token(
            origin: OriginFor<T>,
            pool_asset: AssetIdOf<T>,
            token_per_block: Balance,
            farms_allocation: Balance,
            staking_allocation: Balance,
            team_allocation: Balance,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            // Get token info
            let token_info = <TokenInfos<T>>::get(&pool_asset);

            // Check if token is already registered
            ensure!(
                token_info.token_per_block == 0,
                Error::<T>::TokenAlreadyRegistered
            );

            // Check if token_per_block is zero
            ensure!(token_per_block != 0, Error::<T>::TokenPerBlockCantBeZero);

            if (farms_allocation == 0 && staking_allocation == 0)
                || (farms_allocation + staking_allocation + team_allocation != 1)
            {
                return Err(Error::<T>::InvalidAllocationParameters.into());
            }

            let token_info = TokenInfo {
                farms_total_multiplier: 0,
                staking_total_multiplier: 0,
                token_per_block,
                farms_allocation,
                staking_allocation,
                team_allocation,
            };

            <TokenInfos<T>>::insert(&pool_asset, &token_info);

            // Emit an event
            Self::deposit_event(Event::TokenRegistered(user, pool_asset));

            // Return a successful DispatchResult
            Ok(().into())
        }

        /// Add pool
        #[pallet::weight(10000)]
        pub fn add_pool(
            origin: OriginFor<T>,
            pool_asset: AssetIdOf<T>,
            reward_asset: AssetIdOf<T>,
            is_farm: bool,
            multiplier: u32,
            deposit_fee: Balance,
            is_core: bool,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            // Check if multiplier is valid
            ensure!(multiplier >= 1, Error::<T>::InvalidMultiplier);

            // Get token info
            let token_info = <TokenInfos<T>>::get(&reward_asset);

            // Check if token is registered
            ensure!(
                token_info.token_per_block != 0,
                Error::<T>::RewardTokenIsNotRegistered
            );

            // Get token info
            let pool_infos = <Pools<T>>::get(&pool_asset, &reward_asset);
            for pool_info in pool_infos {
                if !pool_info.is_removed && pool_info.is_farm == is_farm {
                    return Err(Error::<T>::PoolAlreadyExists.into());
                }
            }

            let pool_info = PoolInfo {
                multiplier,
                deposit_fee,
                is_core,
                is_farm,
                total_tokens_in_pool: 0,
                rewards: 0,
                is_removed: false,
            };
            <Pools<T>>::append(&pool_asset, &reward_asset, pool_info);

            // Emit an event
            Self::deposit_event(Event::PoolAdded(user, pool_asset, reward_asset, is_farm));

            // Return a successful DispatchResult
            Ok(().into())
        }

        /// Get rewards
        #[pallet::weight(10000)]
        pub fn get_rewards(
            origin: OriginFor<T>,
            pool_asset: AssetIdOf<T>,
            reward_asset: AssetIdOf<T>,
            is_farm: bool,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            // Get user info
            let mut user_info = <UserInfos<T>>::get(&user);

            let mut rewards = 0;

            for users in user_info.iter_mut() {
                if users.pool_asset == pool_asset
                    && users.reward_asset == reward_asset
                    && users.is_farm == is_farm
                {
                    Assets::<T>::transfer_from(
                        &users.reward_asset,
                        &Self::account_id(),
                        &user,
                        users.rewards,
                    )?;
                }
                rewards = users.rewards;
                users.rewards = 0;
            }
            // Update storage
            <UserInfos<T>>::insert(&user, user_info);

            // Emit an event
            Self::deposit_event(Event::<T>::RewardWithdrawn(user, rewards));

            // Return a successful DispatchResult
            Ok(().into())
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    impl<T: Config> Pallet<T> {
        /// The account ID of pallet
        fn account_id() -> T::AccountId {
            PALLET_ID.into_account()
        }
    }
}
