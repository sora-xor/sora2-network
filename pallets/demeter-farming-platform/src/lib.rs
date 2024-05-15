#![cfg_attr(not(feature = "std"), no_std)]
// TODO #167: fix clippy warnings
#![allow(clippy::all)]

pub mod migrations;
pub mod weights;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

use codec::{Decode, Encode};
use common::{Balance, DemeterFarming};
pub use weights::WeightInfo;

/// Storage version.
#[derive(Encode, Decode, Eq, PartialEq, scale_info::TypeInfo)]
pub enum StorageVersion {
    /// Initial version
    V1,
    /// After adding base_asset field
    V2,
}

#[derive(Encode, Decode, Default, PartialEq, Eq, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct PoolData<AssetId> {
    pub multiplier: u32,
    pub deposit_fee: Balance,
    pub is_core: bool,
    pub is_farm: bool,
    pub total_tokens_in_pool: Balance,
    pub rewards: Balance,
    pub rewards_to_be_distributed: Balance,
    pub is_removed: bool,
    pub base_asset: AssetId,
}

#[derive(Encode, Decode, Default, PartialEq, Eq, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct TokenInfo<AccountId> {
    pub farms_total_multiplier: u32,
    pub staking_total_multiplier: u32,
    pub token_per_block: Balance,
    pub farms_allocation: Balance,
    pub staking_allocation: Balance,
    pub team_allocation: Balance,
    pub team_account: AccountId,
}

#[derive(Encode, Decode, Default, PartialEq, Eq, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct UserInfo<AssetId> {
    pub base_asset: AssetId,
    pub pool_asset: AssetId,
    pub reward_asset: AssetId,
    pub is_farm: bool,
    pub pooled_tokens: Balance,
    pub rewards: Balance,
}

pub use pallet::*;
use sp_runtime::DispatchError;

#[frame_support::pallet]
pub mod pallet {
    use crate::{migrations, PoolData, StorageVersion, TokenInfo, UserInfo, WeightInfo};
    use common::prelude::{AssetInfoProvider, Balance, FixedWrapper};
    use common::{balance, XykPool};
    use frame_support::pallet_prelude::*;
    use frame_support::transactional;
    use frame_support::PalletId;
    use frame_system::pallet_prelude::*;
    use frame_system::RawOrigin;
    use hex_literal::hex;
    use sp_runtime::traits::{AccountIdConversion, Zero};
    use sp_std::prelude::*;

    const PALLET_ID: PalletId = PalletId(*b"deofarms");

    // TODO: #395 use AssetInfoProvider instead of assets pallet
    #[pallet::config]
    pub trait Config:
        frame_system::Config + assets::Config + technical::Config + ceres_liquidity_locker::Config
    {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Demeter asset id
        type DemeterAssetId: Get<Self::AssetId>;

        /// One hour represented in block number
        const BLOCKS_PER_HOUR_AND_A_HALF: BlockNumberFor<Self>;

        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    type Assets<T> = assets::Pallet<T>;
    pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
    pub type AssetIdOf<T> = <T as assets::Config>::AssetId;

    #[pallet::pallet]
    #[pallet::generate_store(pub (super) trait Store)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::storage]
    #[pallet::getter(fn token_info)]
    pub type TokenInfos<T: Config> =
        StorageMap<_, Identity, AssetIdOf<T>, TokenInfo<AccountIdOf<T>>, OptionQuery>;

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
        Vec<PoolData<AssetIdOf<T>>>,
        ValueQuery,
    >;

    #[pallet::type_value]
    pub fn DefaultForPalletStorageVersion<T: Config>() -> StorageVersion {
        StorageVersion::V1
    }

    /// Pallet storage version
    #[pallet::storage]
    #[pallet::getter(fn pallet_storage_version)]
    pub type PalletStorageVersion<T: Config> =
        StorageValue<_, StorageVersion, ValueQuery, DefaultForPalletStorageVersion<T>>;

    #[pallet::type_value]
    pub fn DefaultForAuthorityAccount<T: Config>() -> AccountIdOf<T> {
        let bytes = hex!("fc096e24663f4dd1e2d48092c73213354c067c0c715ec68e7fcad185da626801");
        AccountIdOf::<T>::decode(&mut &bytes[..]).unwrap()
    }

    #[pallet::storage]
    #[pallet::getter(fn authority_account)]
    pub type AuthorityAccount<T: Config> =
        StorageValue<_, AccountIdOf<T>, ValueQuery, DefaultForAuthorityAccount<T>>;

    #[pallet::type_value]
    pub fn DefaultFeeAccount<T: Config>() -> AccountIdOf<T> {
        let bytes = hex!("fc096e24663f4dd1e2d48092c73213354c067c0c715ec68e7fcad185da626801");
        AccountIdOf::<T>::decode(&mut &bytes[..]).unwrap()
    }

    /// Account for fees
    #[pallet::storage]
    #[pallet::getter(fn fee_account)]
    pub type FeeAccount<T: Config> =
        StorageValue<_, AccountIdOf<T>, ValueQuery, DefaultFeeAccount<T>>;

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Token registered [who, what]
        TokenRegistered(AccountIdOf<T>, AssetIdOf<T>),
        /// Pool added [who, base_asset, pool_asset, reward_asset, is_farm]
        PoolAdded(
            AccountIdOf<T>,
            AssetIdOf<T>,
            AssetIdOf<T>,
            AssetIdOf<T>,
            bool,
        ),
        /// Reward Withdrawn [who, amount, base_asset, pool_asset, reward_asset, is_farm]
        RewardWithdrawn(
            AccountIdOf<T>,
            Balance,
            AssetIdOf<T>,
            AssetIdOf<T>,
            AssetIdOf<T>,
            bool,
        ),
        /// Withdrawn [who, amount, base_asset, pool_asset, reward_asset, is_farm]
        Withdrawn(
            AccountIdOf<T>,
            Balance,
            AssetIdOf<T>,
            AssetIdOf<T>,
            AssetIdOf<T>,
            bool,
        ),
        /// Pool removed [who, base_asset, pool_asset, reward_asset, is_farm]
        PoolRemoved(
            AccountIdOf<T>,
            AssetIdOf<T>,
            AssetIdOf<T>,
            AssetIdOf<T>,
            bool,
        ),
        /// Deposited [who, base_asset, pool_asset, reward_asset, is_farm, amount]
        Deposited(
            AccountIdOf<T>,
            AssetIdOf<T>,
            AssetIdOf<T>,
            AssetIdOf<T>,
            bool,
            Balance,
        ),
        /// Multiplier Changed [who, base_asset, pool_asset, reward_asset, is_farm, amount]
        MultiplierChanged(
            AccountIdOf<T>,
            AssetIdOf<T>,
            AssetIdOf<T>,
            AssetIdOf<T>,
            bool,
            u32,
        ),
        /// DepositFeeChanged [who, base_asset, pool_asset, reward_asset, is_farm, amount]
        DepositFeeChanged(
            AccountIdOf<T>,
            AssetIdOf<T>,
            AssetIdOf<T>,
            AssetIdOf<T>,
            bool,
            Balance,
        ),
        /// Token info changed [who, what]
        TokenInfoChanged(AccountIdOf<T>, AssetIdOf<T>),
        /// Total tokens changed [who, base_asset, pool_asset, reward_asset, is_farm, amount]
        TotalTokensChanged(
            AccountIdOf<T>,
            AssetIdOf<T>,
            AssetIdOf<T>,
            AssetIdOf<T>,
            bool,
            Balance,
        ),
        /// Info changed [who, base_asset, pool_asset, reward_asset, is_farm, amount]
        InfoChanged(
            AccountIdOf<T>,
            AssetIdOf<T>,
            AssetIdOf<T>,
            AssetIdOf<T>,
            bool,
            Balance,
        ),
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
        /// Invalid deposit fee
        InvalidDepositFee,
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
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::register_token())]
        pub fn register_token(
            origin: OriginFor<T>,
            pool_asset: AssetIdOf<T>,
            token_per_block: Balance,
            farms_allocation: Balance,
            staking_allocation: Balance,
            team_allocation: Balance,
            team_account: AccountIdOf<T>,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            if user != AuthorityAccount::<T>::get() {
                return Err(Error::<T>::Unauthorized.into());
            }

            // Check if token is already registered
            ensure!(
                !<TokenInfos<T>>::contains_key(&pool_asset),
                Error::<T>::TokenAlreadyRegistered
            );

            // Check if token_per_block is zero
            ensure!(token_per_block != 0, Error::<T>::TokenPerBlockCantBeZero);

            if (farms_allocation == 0 && staking_allocation == 0)
                || (farms_allocation + staking_allocation + team_allocation != balance!(1))
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
                team_account,
            };

            <TokenInfos<T>>::insert(&pool_asset, &token_info);

            // Emit an event
            Self::deposit_event(Event::TokenRegistered(user, pool_asset));

            // Return a successful DispatchResult
            Ok(().into())
        }

        /// Add pool
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::add_pool())]
        pub fn add_pool(
            origin: OriginFor<T>,
            base_asset: AssetIdOf<T>,
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

            // Check if deposit fee is valid
            ensure!(deposit_fee <= balance!(1), Error::<T>::InvalidDepositFee);

            // Get token info
            let mut token_info = <TokenInfos<T>>::get(&reward_asset)
                .ok_or(Error::<T>::RewardTokenIsNotRegistered)?;

            // Check if pool already exists
            let pool_infos = <Pools<T>>::get(&pool_asset, &reward_asset);
            for pool_info in pool_infos {
                if !pool_info.is_removed
                    && pool_info.is_farm == is_farm
                    && pool_info.base_asset == base_asset
                {
                    return Err(Error::<T>::PoolAlreadyExists.into());
                }
            }

            let pool_info = PoolData {
                multiplier,
                deposit_fee,
                is_core,
                is_farm,
                total_tokens_in_pool: 0,
                rewards: 0,
                rewards_to_be_distributed: 0,
                is_removed: false,
                base_asset,
            };

            if is_farm {
                token_info.farms_total_multiplier += multiplier;
            } else {
                token_info.staking_total_multiplier += multiplier;
            }

            <TokenInfos<T>>::insert(&reward_asset, token_info);
            <Pools<T>>::append(&pool_asset, &reward_asset, pool_info);

            // Emit an event
            Self::deposit_event(Event::PoolAdded(
                user,
                base_asset,
                pool_asset,
                reward_asset,
                is_farm,
            ));

            // Return a successful DispatchResult
            Ok(().into())
        }

        /// Deposit to pool

        #[transactional]
        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::deposit())]
        pub fn deposit(
            origin: OriginFor<T>,
            base_asset: AssetIdOf<T>,
            pool_asset: AssetIdOf<T>,
            reward_asset: AssetIdOf<T>,
            is_farm: bool,
            mut pooled_tokens: Balance,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            // Get pool info and check if pool exists
            let mut pool_infos = <Pools<T>>::get(&pool_asset, &reward_asset);
            let mut exist = false;
            let mut pool_info = &mut Default::default();
            for p_info in pool_infos.iter_mut() {
                if !p_info.is_removed
                    && p_info.is_farm == is_farm
                    && p_info.base_asset == base_asset
                {
                    exist = true;
                    pool_info = p_info;
                }
            }
            ensure!(exist, Error::<T>::PoolDoesNotExist);

            // Get user info if exists or create new if does not exist
            let mut user_info = UserInfo {
                base_asset,
                pool_asset,
                reward_asset,
                is_farm,
                pooled_tokens: 0,
                rewards: 0,
            };
            exist = false;
            let mut user_infos = <UserInfos<T>>::get(&user);
            for u_info in &user_infos {
                if u_info.pool_asset == pool_asset
                    && u_info.reward_asset == reward_asset
                    && u_info.is_farm == is_farm
                    && u_info.base_asset == base_asset
                {
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

                if pool_info.deposit_fee != balance!(0) {
                    let fee = (FixedWrapper::from(pooled_tokens)
                        * FixedWrapper::from(pool_info.deposit_fee))
                    .try_into_balance()
                    .unwrap_or(0);
                    pooled_tokens -= fee;

                    Assets::<T>::transfer_from(&pool_asset, &user, &FeeAccount::<T>::get(), fee)?;
                }
                Assets::<T>::transfer_from(&pool_asset, &user, &Self::account_id(), pooled_tokens)?;
            } else {
                let pool_account = T::XYKPool::properties_of_pool(base_asset, pool_asset.clone())
                    .ok_or(Error::<T>::PoolDoesNotExist)?
                    .0;

                let mut lp_tokens =
                    T::XYKPool::balance_of_pool_provider(pool_account.clone(), user.clone())
                        .unwrap_or(0);
                if lp_tokens < user_info.pooled_tokens {
                    lp_tokens = user_info.pooled_tokens;
                }
                ensure!(
                    pooled_tokens <= lp_tokens - user_info.pooled_tokens,
                    Error::<T>::InsufficientLPTokens
                );

                if pool_info.deposit_fee != balance!(0) {
                    let fee = (FixedWrapper::from(pooled_tokens)
                        * FixedWrapper::from(pool_info.deposit_fee))
                    .try_into_balance()
                    .unwrap_or(0);
                    pooled_tokens -= fee;

                    T::XYKPool::transfer_lp_tokens(
                        pool_account.clone(),
                        base_asset,
                        pool_asset,
                        user.clone(),
                        FeeAccount::<T>::get(),
                        fee,
                    )?;

                    lp_tokens =
                        T::XYKPool::balance_of_pool_provider(pool_account.clone(), user.clone())
                            .unwrap_or(0);
                }

                // Handle total LP changed in other XOR or XSTUSD/pool_asset farming pools
                for u_info in user_infos.iter_mut() {
                    if u_info.pool_asset == pool_asset
                        && u_info.reward_asset != reward_asset
                        && u_info.is_farm
                        && u_info.base_asset == base_asset
                    {
                        if u_info.pooled_tokens > lp_tokens {
                            let pool_tokens_diff = u_info.pooled_tokens - lp_tokens;
                            u_info.pooled_tokens = lp_tokens;
                            let mut pool_data = <Pools<T>>::get(&pool_asset, &u_info.reward_asset);
                            for p_info in pool_data.iter_mut() {
                                if !p_info.is_removed
                                    && p_info.is_farm == is_farm
                                    && p_info.base_asset == base_asset
                                {
                                    p_info.total_tokens_in_pool -= pool_tokens_diff;
                                }
                            }
                            <Pools<T>>::insert(&pool_asset, &u_info.reward_asset, pool_data);
                        }
                    }
                }
            }

            // Update user info
            if exist {
                for u_info in user_infos.iter_mut() {
                    if u_info.pool_asset == pool_asset
                        && u_info.reward_asset == reward_asset
                        && u_info.is_farm == is_farm
                        && u_info.base_asset == base_asset
                    {
                        u_info.pooled_tokens += pooled_tokens;
                    }
                }
            } else {
                user_info.pooled_tokens += pooled_tokens;
                user_infos.push(user_info);
            }
            <UserInfos<T>>::insert(&user, user_infos);

            // Update pool info
            for p_info in pool_infos.iter_mut() {
                if !p_info.is_removed
                    && p_info.is_farm == is_farm
                    && p_info.base_asset == base_asset
                {
                    p_info.total_tokens_in_pool += pooled_tokens;
                }
            }

            <Pools<T>>::insert(&pool_asset, &reward_asset, pool_infos);

            // Emit an event
            Self::deposit_event(Event::Deposited(
                user,
                base_asset,
                pool_asset,
                reward_asset,
                is_farm,
                pooled_tokens,
            ));

            // Return a successful DispatchResult
            Ok(().into())
        }

        /// Get rewards

        #[transactional]
        #[pallet::call_index(3)]
        #[pallet::weight(<T as Config>::WeightInfo::get_rewards())]
        pub fn get_rewards(
            origin: OriginFor<T>,
            base_asset: AssetIdOf<T>,
            pool_asset: AssetIdOf<T>,
            reward_asset: AssetIdOf<T>,
            is_farm: bool,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            // Get pool info and check if pool has rewards
            let mut pool_infos = <Pools<T>>::get(&pool_asset, &reward_asset);
            let mut exist = false;
            let mut pool_info_rewards = balance!(0);

            for p_info in pool_infos.iter_mut() {
                if p_info.is_farm == is_farm && p_info.base_asset == base_asset {
                    exist = true;
                    pool_info_rewards = p_info.rewards;
                }
            }
            ensure!(exist, Error::<T>::PoolDoesNotExist);

            // Get user info
            let mut user_infos = <UserInfos<T>>::get(&user);
            let mut rewards = 0;

            for user_info in user_infos.iter_mut() {
                if user_info.pool_asset == pool_asset
                    && user_info.reward_asset == reward_asset
                    && user_info.is_farm == is_farm
                    && user_info.base_asset == base_asset
                {
                    ensure!(user_info.rewards != 0, Error::<T>::ZeroRewards);
                    ensure!(
                        pool_info_rewards >= user_info.rewards,
                        Error::<T>::PoolDoesNotHaveRewards
                    );

                    Assets::<T>::transfer_from(
                        &user_info.reward_asset,
                        &Self::account_id(),
                        &user,
                        user_info.rewards,
                    )?;

                    rewards = user_info.rewards;
                    pool_info_rewards -= user_info.rewards;
                    user_info.rewards = 0;
                }
            }

            for p_info in pool_infos.iter_mut() {
                if p_info.is_farm == is_farm && p_info.base_asset == base_asset {
                    p_info.rewards = pool_info_rewards;
                }
            }

            // Update storage
            <UserInfos<T>>::insert(&user, user_infos);
            <Pools<T>>::insert(&pool_asset, &reward_asset, pool_infos);

            // Emit an event
            Self::deposit_event(Event::<T>::RewardWithdrawn(
                user,
                rewards,
                base_asset,
                pool_asset,
                reward_asset,
                is_farm,
            ));

            // Return a successful DispatchResult
            Ok(().into())
        }

        /// Withdraw

        #[transactional]
        #[pallet::call_index(4)]
        #[pallet::weight(<T as Config>::WeightInfo::withdraw())]
        pub fn withdraw(
            origin: OriginFor<T>,
            base_asset: AssetIdOf<T>,
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
                    && user_info.base_asset == base_asset
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
                if pool_info.is_farm == is_farm && pool_info.base_asset == base_asset {
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
                base_asset,
                pool_asset,
                reward_asset,
                is_farm,
            ));

            // Return a successful DispatchResult
            Ok(().into())
        }

        /// Remove pool
        #[pallet::call_index(5)]
        #[pallet::weight(<T as Config>::WeightInfo::remove_pool())]
        pub fn remove_pool(
            origin: OriginFor<T>,
            base_asset: AssetIdOf<T>,
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
                if pool_info.is_farm == is_farm && pool_info.base_asset == base_asset {
                    pool_info.is_removed = true;
                }
            }

            <Pools<T>>::insert(&pool_asset, &reward_asset, &pool_infos);

            // Emit an event
            Self::deposit_event(Event::<T>::PoolRemoved(
                user,
                base_asset,
                pool_asset,
                reward_asset,
                is_farm,
            ));

            // Return a successful DispatchResult
            Ok(().into())
        }

        /// Change pool multiplier
        #[pallet::call_index(6)]
        #[pallet::weight(<T as Config>::WeightInfo::change_pool_multiplier())]
        pub fn change_pool_multiplier(
            origin: OriginFor<T>,
            base_asset: AssetIdOf<T>,
            pool_asset: AssetIdOf<T>,
            reward_asset: AssetIdOf<T>,
            is_farm: bool,
            new_multiplier: u32,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            if user != AuthorityAccount::<T>::get() {
                return Err(Error::<T>::Unauthorized.into());
            }

            // Get pool info and check if pool exists
            let mut pool_infos = <Pools<T>>::get(&pool_asset, &reward_asset);
            let mut old_multiplier = 0;
            let mut exist = false;

            for p_info in pool_infos.iter_mut() {
                if p_info.is_farm == is_farm && p_info.base_asset == base_asset {
                    exist = true;
                    old_multiplier = p_info.multiplier;
                    p_info.multiplier = new_multiplier;
                }
            }
            ensure!(exist, Error::<T>::PoolDoesNotExist);

            let mut token_info = <TokenInfos<T>>::get(&reward_asset)
                .ok_or(Error::<T>::RewardTokenIsNotRegistered)?;

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
                base_asset,
                pool_asset,
                reward_asset,
                is_farm,
                new_multiplier,
            ));

            // Return a successful DispatchResult
            Ok(().into())
        }

        /// Change total tokens
        #[pallet::call_index(7)]
        #[pallet::weight(<T as Config>::WeightInfo::change_total_tokens())]
        pub fn change_total_tokens(
            origin: OriginFor<T>,
            base_asset: AssetIdOf<T>,
            pool_asset: AssetIdOf<T>,
            reward_asset: AssetIdOf<T>,
            is_farm: bool,
            total_tokens: Balance,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            if user != AuthorityAccount::<T>::get() {
                return Err(Error::<T>::Unauthorized.into());
            }

            // Get pool info
            let mut pool_infos = <Pools<T>>::get(&pool_asset, &reward_asset);
            let mut exist = false;

            for p_info in pool_infos.iter_mut() {
                if !p_info.is_removed
                    && p_info.is_farm == is_farm
                    && p_info.base_asset == base_asset
                {
                    exist = true;
                    p_info.total_tokens_in_pool = total_tokens;
                }
            }
            ensure!(exist, Error::<T>::PoolDoesNotExist);

            <Pools<T>>::insert(&pool_asset, &reward_asset, pool_infos);

            // Emit an event
            Self::deposit_event(Event::<T>::TotalTokensChanged(
                user,
                base_asset,
                pool_asset,
                reward_asset,
                is_farm,
                total_tokens,
            ));

            // Return a successful DispatchResult
            Ok(().into())
        }

        /// Change info
        #[pallet::call_index(8)]
        #[pallet::weight(<T as Config>::WeightInfo::change_info())]
        pub fn change_info(
            origin: OriginFor<T>,
            changed_user: AccountIdOf<T>,
            base_asset: AssetIdOf<T>,
            pool_asset: AssetIdOf<T>,
            reward_asset: AssetIdOf<T>,
            is_farm: bool,
            pool_tokens: Balance,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            if user != AuthorityAccount::<T>::get() {
                return Err(Error::<T>::Unauthorized.into());
            }

            // Get pool info
            let mut user_infos = <UserInfos<T>>::get(&changed_user);
            for u_info in user_infos.iter_mut() {
                if u_info.pool_asset == pool_asset
                    && u_info.reward_asset == reward_asset
                    && u_info.is_farm == is_farm
                    && u_info.base_asset == base_asset
                {
                    u_info.pooled_tokens = pool_tokens;
                }
            }

            <UserInfos<T>>::insert(&changed_user, &user_infos);

            // Emit an event
            Self::deposit_event(Event::<T>::InfoChanged(
                changed_user,
                base_asset,
                pool_asset,
                reward_asset,
                is_farm,
                pool_tokens,
            ));

            // Return a successful DispatchResult
            Ok(().into())
        }

        /// Change pool deposit fee
        #[pallet::call_index(9)]
        #[pallet::weight(<T as Config>::WeightInfo::change_pool_deposit_fee())]
        pub fn change_pool_deposit_fee(
            origin: OriginFor<T>,
            base_asset: AssetIdOf<T>,
            pool_asset: AssetIdOf<T>,
            reward_asset: AssetIdOf<T>,
            is_farm: bool,
            deposit_fee: Balance,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            if user != AuthorityAccount::<T>::get() {
                return Err(Error::<T>::Unauthorized.into());
            }

            // Check if deposit fee is valid
            ensure!(deposit_fee <= balance!(1), Error::<T>::InvalidDepositFee);

            let mut pool_infos = <Pools<T>>::get(&pool_asset, &reward_asset);
            let mut exist = false;

            for p_info in pool_infos.iter_mut() {
                if !p_info.is_removed
                    && p_info.is_farm == is_farm
                    && p_info.base_asset == base_asset
                {
                    exist = true;
                    p_info.deposit_fee = deposit_fee;
                }
            }
            ensure!(exist, Error::<T>::PoolDoesNotExist);

            <Pools<T>>::insert(&pool_asset, &reward_asset, pool_infos);

            // Emit an event
            Self::deposit_event(Event::<T>::DepositFeeChanged(
                user,
                base_asset,
                pool_asset,
                reward_asset,
                is_farm,
                deposit_fee,
            ));

            // Return a successful DispatchResult
            Ok(().into())
        }

        /// Change token info
        #[pallet::call_index(10)]
        #[pallet::weight(<T as Config>::WeightInfo::change_token_info())]
        pub fn change_token_info(
            origin: OriginFor<T>,
            pool_asset: AssetIdOf<T>,
            token_per_block: Balance,
            farms_allocation: Balance,
            staking_allocation: Balance,
            team_allocation: Balance,
            team_account: AccountIdOf<T>,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            if user != AuthorityAccount::<T>::get() {
                return Err(Error::<T>::Unauthorized.into());
            }

            // Get token info
            let mut token_info =
                <TokenInfos<T>>::get(&pool_asset).ok_or(Error::<T>::RewardTokenIsNotRegistered)?;

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
            token_info.team_account = team_account;

            <TokenInfos<T>>::insert(&pool_asset, &token_info);

            // Emit an event
            Self::deposit_event(Event::TokenInfoChanged(user, pool_asset));

            // Return a successful DispatchResult
            Ok(().into())
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(now: BlockNumberFor<T>) -> Weight {
            let mut counter = Weight::zero();

            if (now % T::BLOCKS_PER_HOUR_AND_A_HALF).is_zero() {
                counter = Self::distribute_rewards_to_users();
            }
            if (now % T::BLOCKS_PER_ONE_DAY).is_zero() {
                counter = Self::distribute_rewards_to_pools();
            }

            counter
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
        /// The account ID of pallet
        fn account_id() -> T::AccountId {
            PALLET_ID.into_account_truncating()
        }

        fn mint_deo() {
            let blocks = 14400_u32;
            let deo_info = if let Some(info) = <TokenInfos<T>>::get(&T::DemeterAssetId::get()) {
                info
            } else {
                return;
            };

            let amount = (FixedWrapper::from(balance!(blocks))
                * FixedWrapper::from(deo_info.token_per_block))
            .try_into_balance()
            .unwrap_or(0);
            let amount_for_team = (FixedWrapper::from(amount)
                * FixedWrapper::from(deo_info.team_allocation))
            .try_into_balance()
            .unwrap_or(0);
            let amount_for_farming_and_staking = amount - amount_for_team;

            let _ = Assets::<T>::mint(
                RawOrigin::Signed(AuthorityAccount::<T>::get()).into(),
                T::DemeterAssetId::get().into(),
                Self::account_id(),
                amount_for_farming_and_staking,
            );
        }

        /// Distribute rewards to pools
        fn distribute_rewards_to_pools() -> Weight {
            let mut counter: u64 = 0;
            let blocks = 14400_u32;

            Self::mint_deo();

            // Distribute rewards to pools
            let zero = balance!(0);
            for (token_asset_id, token_info) in TokenInfos::<T>::iter() {
                let amount_per_day = (FixedWrapper::from(balance!(blocks))
                    * FixedWrapper::from(token_info.token_per_block))
                .try_into_balance()
                .unwrap_or(zero);
                let amount_for_farming = (FixedWrapper::from(amount_per_day)
                    * FixedWrapper::from(token_info.farms_allocation))
                .try_into_balance()
                .unwrap_or(zero);
                let amount_for_staking = (FixedWrapper::from(amount_per_day)
                    * FixedWrapper::from(token_info.staking_allocation))
                .try_into_balance()
                .unwrap_or(zero);
                let amount_for_team = (FixedWrapper::from(amount_per_day)
                    * FixedWrapper::from(token_info.team_allocation))
                .try_into_balance()
                .unwrap_or(zero);

                let _ = Assets::<T>::transfer_from(
                    &token_asset_id,
                    &Self::account_id(),
                    &token_info.team_account,
                    amount_for_team,
                );

                for (pool_asset, reward_asset, mut pool_infos) in Pools::<T>::iter() {
                    if reward_asset == token_asset_id {
                        for pool_info in pool_infos.iter_mut() {
                            if !pool_info.is_removed && pool_info.total_tokens_in_pool != zero {
                                let total_multiplier;
                                let amount;

                                if !pool_info.is_farm {
                                    total_multiplier = token_info.staking_total_multiplier;
                                    amount = amount_for_staking;
                                } else {
                                    total_multiplier = token_info.farms_total_multiplier;
                                    amount = amount_for_farming;
                                }

                                let reward = (FixedWrapper::from(amount)
                                    * (FixedWrapper::from(balance!(pool_info.multiplier))
                                        / FixedWrapper::from(balance!(total_multiplier))))
                                .try_into_balance()
                                .unwrap_or(zero);

                                pool_info.rewards_to_be_distributed = reward;
                            }
                        }

                        <Pools<T>>::insert(pool_asset, reward_asset, pool_infos);
                        counter += 1;
                    }
                }
            }

            T::DbWeight::get()
                .reads(counter + 1)
                .saturating_add(T::DbWeight::get().writes(counter))
        }

        fn distribute_rewards_to_users() -> Weight {
            let mut counter: u64 = 0;
            let per_hour_and_half = balance!(0.0625);
            let zero = balance!(0);

            for (pool_asset, reward_asset, mut pool_infos) in Pools::<T>::iter() {
                for pool_info in pool_infos.iter_mut() {
                    if pool_info.rewards_to_be_distributed != zero && !pool_info.is_removed {
                        let amount_per_hour =
                            (FixedWrapper::from(pool_info.rewards_to_be_distributed)
                                * FixedWrapper::from(per_hour_and_half))
                            .try_into_balance()
                            .unwrap_or(zero);

                        for (user, mut user_infos) in UserInfos::<T>::iter() {
                            let mut changed = false;
                            for user_info in user_infos.iter_mut() {
                                if user_info.pool_asset == pool_asset
                                    && user_info.reward_asset == reward_asset
                                    && user_info.is_farm == pool_info.is_farm
                                    && user_info.base_asset == pool_info.base_asset
                                {
                                    let amount_per_user = (FixedWrapper::from(amount_per_hour)
                                        * (FixedWrapper::from(user_info.pooled_tokens)
                                            / FixedWrapper::from(pool_info.total_tokens_in_pool)))
                                    .try_into_balance()
                                    .unwrap_or(zero);
                                    user_info.rewards += amount_per_user;
                                    changed = true;
                                }
                            }
                            if changed {
                                <UserInfos<T>>::insert(user, user_infos);
                                counter += 1;
                            }
                        }

                        pool_info.rewards += amount_per_hour;
                    }
                }
                <Pools<T>>::insert(pool_asset, reward_asset, pool_infos);
                counter += 1;
            }

            T::DbWeight::get()
                .reads(counter)
                .saturating_add(T::DbWeight::get().writes(counter))
        }

        /// Check if user has enough free liquidity for withdrawing
        pub fn check_if_has_enough_liquidity_out_of_farming(
            user: &AccountIdOf<T>,
            asset_a: AssetIdOf<T>,
            asset_b: AssetIdOf<T>,
            withdrawing_amount: Balance,
        ) -> bool {
            // Get pool account
            let pool_account: AccountIdOf<T> =
                if let Some(account) = T::XYKPool::properties_of_pool(asset_a, asset_b) {
                    account.0
                } else {
                    return false;
                };

            // Calculate number of pool tokens
            let pool_tokens =
                T::XYKPool::balance_of_pool_provider(pool_account.clone(), user.clone())
                    .unwrap_or(0);
            if pool_tokens == 0 {
                return false;
            }

            let mut pooled_tokens = balance!(0);
            let user_infos = <UserInfos<T>>::get(&user);
            for user_info in user_infos {
                if user_info.pool_asset == asset_b
                    && user_info.is_farm
                    && user_info.base_asset == asset_a
                {
                    if pooled_tokens < user_info.pooled_tokens {
                        pooled_tokens = user_info.pooled_tokens;
                    }
                }
            }

            let free_pool_tokens = pool_tokens.checked_sub(pooled_tokens).unwrap_or(0);

            pooled_tokens == balance!(0) || free_pool_tokens >= withdrawing_amount
        }
    }
}

impl<T: Config> DemeterFarming<T::AccountId, T::AssetId> for Pallet<T> {
    fn update_pool_tokens(
        user: T::AccountId,
        pool_tokens: Balance,
        base_asset: T::AssetId,
        pool_asset: T::AssetId,
    ) -> Result<(), DispatchError> {
        let mut user_infos = <UserInfos<T>>::get(&user);
        for u_info in user_infos.iter_mut() {
            if u_info.pool_asset == pool_asset && u_info.is_farm && u_info.base_asset == base_asset
            {
                if u_info.pooled_tokens > pool_tokens {
                    let pool_tokens_diff = u_info.pooled_tokens - pool_tokens;
                    u_info.pooled_tokens = pool_tokens;
                    let mut pool_data = <Pools<T>>::get(&pool_asset, &u_info.reward_asset);
                    for p_info in pool_data.iter_mut() {
                        if !p_info.is_removed && p_info.is_farm && p_info.base_asset == base_asset {
                            p_info.total_tokens_in_pool -= pool_tokens_diff;
                        }
                    }
                    <Pools<T>>::insert(&pool_asset, &u_info.reward_asset, pool_data);
                }
            }
        }
        <UserInfos<T>>::insert(user, user_infos);

        Ok(())
    }
}
