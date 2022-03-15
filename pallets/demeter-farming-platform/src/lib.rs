#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

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
    rewards_to_be_distributed: Balance,
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

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use crate::{PoolInfo, TokenInfo, UserInfo};
    use common::prelude::Balance;
    use common::{balance, PoolXykPallet, XOR};
    use frame_support::pallet_prelude::*;
    use frame_support::traits::Vec;
    use frame_system::pallet_prelude::*;
    use hex_literal::hex;
    use sp_runtime::traits::AccountIdConversion;
    use sp_runtime::ModuleId;

    const PALLET_ID: ModuleId = ModuleId(*b"deofarms");

    #[pallet::config]
    pub trait Config:
        frame_system::Config + assets::Config + pool_xyk::Config + technical::Config
    {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
    }

    type Assets<T> = assets::Pallet<T>;
    type PoolXYK<T> = pool_xyk::Pallet<T>;
    pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
    type AssetIdOf<T> = <T as assets::Config>::AssetId;

    #[pallet::pallet]
    #[pallet::generate_store(pub (super) trait Store)]
    pub struct Pallet<T>(PhantomData<T>);

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

    #[pallet::type_value]
    pub fn DefaultForAuthorityAccount<T: Config>() -> AccountIdOf<T> {
        let bytes = hex!("96ea3c9c0be7bbc7b0656a1983db5eed75210256891a9609012362e36815b132");
        AccountIdOf::<T>::decode(&mut &bytes[..]).unwrap_or_default()
    }

    #[pallet::storage]
    #[pallet::getter(fn authority_account)]
    pub type AuthorityAccount<T: Config> =
        StorageValue<_, AccountIdOf<T>, ValueQuery, DefaultForAuthorityAccount<T>>;

    #[pallet::event]
    #[pallet::metadata(AccountIdOf<T> = "AccountId", BalanceOf<T> = "Balance", AssetIdOf<T> = "AssetId")]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Token registered [who, what]
        TokenRegistered(AccountIdOf<T>, AssetIdOf<T>),
        /// Pool added [who, pool_asset, reward_asset, is_farm]
        PoolAdded(AccountIdOf<T>, AssetIdOf<T>, AssetIdOf<T>, bool),
        /// Reward Withdrawn [who, amount, pool_asset, reward_asset, is_farm]
        RewardWithdrawn(AccountIdOf<T>, Balance, AssetIdOf<T>, AssetIdOf<T>, bool),
        /// Withdrawn [who, amount, pool_asset, reward_asset, is_farm]
        Withdrawn(AccountIdOf<T>, Balance, AssetIdOf<T>, AssetIdOf<T>, bool),
        /// Pool removed [who, pool_asset, reward_asset, is_farm]
        PoolRemoved(AccountIdOf<T>, AssetIdOf<T>, AssetIdOf<T>, bool),
        /// Deposited [who, pool_asset, reward_asset, is_farm, amount]
        Deposited(AccountIdOf<T>, AssetIdOf<T>, AssetIdOf<T>, bool, Balance),
        /// Multiplier Changed [who, pool_asset, reward_asset, is_farm, amount]
        MultiplierChanged(AccountIdOf<T>, AssetIdOf<T>, AssetIdOf<T>, bool, u32),
        /// DepositFeeChanged [who, pool_asset, reward_asset, is_farm, amount]
        DepositFeeChanged(AccountIdOf<T>, AssetIdOf<T>, AssetIdOf<T>, bool, Balance),
        /// Token info changed [who, what]
        TokenInfoChanged(AccountIdOf<T>, AssetIdOf<T>),
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
        /// Insufficient Funds
        InsufficientFunds,
        /// Zero Rewards
        ZeroRewards,
        /// Pool does not exist
        PoolDoesNotExist,
        /// Insufficient LP tokens
        InsufficientLPTokens,
        /// Pool does not have rewards,
        PoolDoesNotHaveRewards,
        /// Unauthorized
        Unauthorized,
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

            if user != AuthorityAccount::<T>::get() {
                return Err(Error::<T>::Unauthorized.into());
            }

            // Get token info
            let mut token_info = <TokenInfos<T>>::get(&pool_asset);

            // Check if token is already registered
            ensure!(
                token_info.token_per_block == 0,
                Error::<T>::TokenAlreadyRegistered
            );

            // Check if token_per_block is zero
            ensure!(token_per_block != 0, Error::<T>::TokenPerBlockCantBeZero);

            if (farms_allocation == 0 && staking_allocation == 0)
                || (farms_allocation + staking_allocation + team_allocation != balance!(1))
            {
                return Err(Error::<T>::InvalidAllocationParameters.into());
            }

            token_info = TokenInfo {
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

            if user != AuthorityAccount::<T>::get() {
                return Err(Error::<T>::Unauthorized.into());
            }

            // Check if multiplier is valid
            ensure!(multiplier >= 1, Error::<T>::InvalidMultiplier);

            // Get token info
            let mut token_info = <TokenInfos<T>>::get(&reward_asset);

            // Check if token is registered
            ensure!(
                token_info.token_per_block != 0,
                Error::<T>::RewardTokenIsNotRegistered
            );

            // Check if pool already exists
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
                rewards_to_be_distributed: 0,
                is_removed: false,
            };

            if is_farm {
                token_info.farms_total_multiplier += multiplier;
            } else {
                token_info.staking_total_multiplier += multiplier;
            }

            <TokenInfos<T>>::insert(&reward_asset, token_info);
            <Pools<T>>::append(&pool_asset, &reward_asset, pool_info);

            // Emit an event
            Self::deposit_event(Event::PoolAdded(user, pool_asset, reward_asset, is_farm));

            // Return a successful DispatchResult
            Ok(().into())
        }

        /// Deposit to pool
        #[pallet::weight(10000)]
        pub fn deposit(
            origin: OriginFor<T>,
            pool_asset: AssetIdOf<T>,
            reward_asset: AssetIdOf<T>,
            is_farm: bool,
            pooled_tokens: Balance,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            // Get pool info and check if pool exists
            let mut pool_infos = <Pools<T>>::get(&pool_asset, &reward_asset);
            let mut exist = false;
            for p_info in &pool_infos {
                if !p_info.is_removed && p_info.is_farm == is_farm {
                    exist = true;
                }
            }
            ensure!(exist, Error::<T>::PoolDoesNotExist);

            // Get user info if exists or create new if does not exist
            let mut user_info = UserInfo {
                pool_asset,
                reward_asset,
                is_farm,
                pooled_tokens: 0,
                rewards: 0,
            };
            exist = false;
            let mut user_infos = <UserInfos<T>>::get(&user);
            for u_info in &user_infos {
                if u_info.is_farm == is_farm {
                    user_info.pooled_tokens = u_info.pooled_tokens;
                    user_info.rewards = u_info.rewards;
                    exist = true;
                }
            }

            // Transfer pooled_tokens
            if !is_farm {
                ensure!(
                    pooled_tokens <= Assets::<T>::free_balance(&pool_asset, &user).unwrap_or(0),
                    Error::<T>::InsufficientFunds
                );
                Assets::<T>::transfer_from(&pool_asset, &user, &Self::account_id(), pooled_tokens)?;
            } else {
                let pool_account = PoolXYK::<T>::properties_of_pool(XOR.into(), pool_asset.clone())
                    .ok_or(Error::<T>::PoolDoesNotExist)?
                    .0;
                let lp_tokens =
                    PoolXYK::<T>::balance_of_pool_provider(pool_account, user.clone()).unwrap_or(0);
                ensure!(
                    pooled_tokens <= lp_tokens - user_info.pooled_tokens,
                    Error::<T>::InsufficientLPTokens
                )
            }

            // Update user info
            if exist {
                for u_info in user_infos.iter_mut() {
                    if u_info.is_farm == is_farm {
                        u_info.pooled_tokens += pooled_tokens;
                    }
                }
                <UserInfos<T>>::insert(&user, user_infos);
            } else {
                user_info.pooled_tokens += pooled_tokens;
                <UserInfos<T>>::append(&user, user_info);
            }

            // Update pool info
            for p_info in pool_infos.iter_mut() {
                if !p_info.is_removed && p_info.is_farm == is_farm {
                    p_info.total_tokens_in_pool += pooled_tokens;
                }
            }

            <Pools<T>>::insert(&pool_asset, &reward_asset, pool_infos);

            // Emit an event
            Self::deposit_event(Event::Deposited(
                user,
                pool_asset,
                reward_asset,
                is_farm,
                pooled_tokens,
            ));

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

            // Get pool info and check if pool has rewards
            let pool_infos: &mut Vec<PoolInfo> = &mut <Pools<T>>::get(&pool_asset, &reward_asset);
            let mut pool_info = &mut Default::default();
            for p_info in pool_infos.iter_mut() {
                if p_info.is_farm == is_farm {
                    pool_info = p_info;
                }
            }
            ensure!(pool_info.multiplier != 0, Error::<T>::PoolDoesNotExist);

            // Get user info
            let mut user_infos = <UserInfos<T>>::get(&user);

            let mut rewards = 0;

            for user_info in user_infos.iter_mut() {
                if user_info.pool_asset == pool_asset
                    && user_info.reward_asset == reward_asset
                    && user_info.is_farm == is_farm
                {
                    ensure!(user_info.rewards != 0, Error::<T>::ZeroRewards);
                    ensure!(
                        pool_info.rewards >= user_info.rewards,
                        Error::<T>::PoolDoesNotHaveRewards
                    );

                    Assets::<T>::transfer_from(
                        &user_info.reward_asset,
                        &Self::account_id(),
                        &user,
                        user_info.rewards,
                    )?;

                    rewards = user_info.rewards;
                    user_info.rewards = 0;
                    pool_info.rewards -= user_info.rewards;
                }
            }

            // Update storage
            <UserInfos<T>>::insert(&user, user_infos);

            // Emit an event
            Self::deposit_event(Event::<T>::RewardWithdrawn(
                user,
                rewards,
                pool_asset,
                reward_asset,
                is_farm,
            ));

            // Return a successful DispatchResult
            Ok(().into())
        }

        /// Withdraw
        #[pallet::weight(10000)]
        pub fn withdraw(
            origin: OriginFor<T>,
            pool_asset: AssetIdOf<T>,
            reward_asset: AssetIdOf<T>,
            pooled_tokens: Balance,
            is_farm: bool,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            // Get user info
            let mut user_infos = <UserInfos<T>>::get(&user);

            for user_info in user_infos.iter_mut() {
                if user_info.pool_asset == pool_asset
                    && user_info.reward_asset == reward_asset
                    && user_info.is_farm == is_farm
                {
                    ensure!(
                        pooled_tokens <= user_info.pooled_tokens,
                        Error::<T>::InsufficientFunds
                    );

                    if is_farm == false {
                        Assets::<T>::transfer_from(
                            &pool_asset,
                            &Self::account_id(),
                            &user,
                            pooled_tokens,
                        )?;
                    }
                    user_info.pooled_tokens -= pooled_tokens;
                }
            }

            // Get pool info
            let mut pool_infos = <Pools<T>>::get(&pool_asset, &reward_asset);
            for pool_info in pool_infos.iter_mut() {
                if pool_info.is_farm == is_farm {
                    pool_info.total_tokens_in_pool -= pooled_tokens;
                }
            }

            // Update storage
            <UserInfos<T>>::insert(&user, user_infos);
            <Pools<T>>::insert(&pool_asset, &reward_asset, &pool_infos);

            // Emit an event
            Self::deposit_event(Event::<T>::Withdrawn(
                user,
                pooled_tokens,
                pool_asset,
                reward_asset,
                is_farm,
            ));

            // Return a successful DispatchResult
            Ok(().into())
        }

        /// Remove pool
        #[pallet::weight(10000)]
        pub fn remove_pool(
            origin: OriginFor<T>,
            pool_asset: AssetIdOf<T>,
            reward_asset: AssetIdOf<T>,
            is_farm: bool,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            if user != AuthorityAccount::<T>::get() {
                return Err(Error::<T>::Unauthorized.into());
            }

            // Get pool info
            let mut pool_infos = <Pools<T>>::get(&pool_asset, &reward_asset);
            for pool_info in pool_infos.iter_mut() {
                if pool_info.is_farm == is_farm {
                    pool_info.is_removed = true;
                }
            }

            <Pools<T>>::insert(&pool_asset, &reward_asset, &pool_infos);

            // Emit an event
            Self::deposit_event(Event::<T>::PoolRemoved(
                user,
                pool_asset,
                reward_asset,
                is_farm,
            ));

            // Return a successful DispatchResult
            Ok(().into())
        }

        /// Change pool multiplier
        #[pallet::weight(10000)]
        pub fn change_pool_multiplier(
            origin: OriginFor<T>,
            pool_asset: AssetIdOf<T>,
            reward_asset: AssetIdOf<T>,
            is_farm: bool,
            new_multiplier: u32,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            if user != AuthorityAccount::<T>::get() {
                return Err(Error::<T>::Unauthorized.into());
            }

            // Check if multiplier is valid
            ensure!(new_multiplier >= 1, Error::<T>::InvalidMultiplier);

            // Get pool info and check if pool exists
            let mut pool_infos = <Pools<T>>::get(&pool_asset, &reward_asset);
            let mut old_multiplier = 0;
            let mut exist = false;

            for p_info in pool_infos.iter_mut() {
                if !p_info.is_removed && p_info.is_farm == is_farm {
                    exist = true;
                    old_multiplier = p_info.multiplier;
                    p_info.multiplier = new_multiplier;
                }
            }
            ensure!(exist, Error::<T>::PoolDoesNotExist);

            let mut token_info = <TokenInfos<T>>::get(&reward_asset);

            if is_farm {
                token_info.farms_total_multiplier =
                    token_info.farms_total_multiplier - old_multiplier + new_multiplier;
            } else {
                token_info.staking_total_multiplier =
                    token_info.staking_total_multiplier - old_multiplier + new_multiplier;
            }

            <TokenInfos<T>>::insert(&reward_asset, &token_info);
            <Pools<T>>::insert(&pool_asset, &reward_asset, pool_infos);

            // Emit an event
            Self::deposit_event(Event::<T>::MultiplierChanged(
                user,
                pool_asset,
                reward_asset,
                is_farm,
                new_multiplier,
            ));

            // Return a successful DispatchResult
            Ok(().into())
        }

        /// Change pool deposit fee
        #[pallet::weight(10000)]
        pub fn change_pool_deposit_fee(
            origin: OriginFor<T>,
            pool_asset: AssetIdOf<T>,
            reward_asset: AssetIdOf<T>,
            is_farm: bool,
            deposit_fee: Balance,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            if user != AuthorityAccount::<T>::get() {
                return Err(Error::<T>::Unauthorized.into());
            }

            // Get pool info and check if pool exists
            let pool_infos: &mut Vec<PoolInfo> = &mut <Pools<T>>::get(&pool_asset, &reward_asset);
            let mut pool_info = &mut Default::default();
            for p_info in pool_infos.iter_mut() {
                if !p_info.is_removed && p_info.is_farm == is_farm {
                    pool_info = p_info;
                }
            }
            ensure!(pool_info.multiplier != 0, Error::<T>::PoolDoesNotExist);

            pool_info.deposit_fee = deposit_fee;

            // Emit an event
            Self::deposit_event(Event::<T>::DepositFeeChanged(
                user,
                pool_asset,
                reward_asset,
                is_farm,
                deposit_fee,
            ));

            // Return a successful DispatchResult
            Ok(().into())
        }

        /// Change token info
        #[pallet::weight(10000)]
        pub fn change_token_info(
            origin: OriginFor<T>,
            pool_asset: AssetIdOf<T>,
            token_per_block: Balance,
            farms_allocation: Balance,
            staking_allocation: Balance,
            team_allocation: Balance,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            if user != AuthorityAccount::<T>::get() {
                return Err(Error::<T>::Unauthorized.into());
            }

            // Get token info
            let mut token_info = <TokenInfos<T>>::get(&pool_asset);

            // Check if token is already registered
            ensure!(
                token_info.token_per_block != 0,
                Error::<T>::RewardTokenIsNotRegistered
            );

            // Check if token_per_block is zero
            ensure!(token_per_block != 0, Error::<T>::TokenPerBlockCantBeZero);

            if (farms_allocation == 0 && staking_allocation == 0)
                || (farms_allocation + staking_allocation + team_allocation != balance!(1))
            {
                return Err(Error::<T>::InvalidAllocationParameters.into());
            }

            token_info.token_per_block = token_per_block;
            token_info.farms_allocation = farms_allocation;
            token_info.staking_allocation = staking_allocation;
            token_info.team_allocation = team_allocation;

            <TokenInfos<T>>::insert(&pool_asset, &token_info);

            // Emit an event
            Self::deposit_event(Event::TokenInfoChanged(user, pool_asset));

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
