#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::type_complexity)]
use codec::{Decode, Encode};
use common::Balance;

#[derive(Encode, Decode, Default, PartialEq, Eq, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct LendingPosition<BlockNumberFor> {
    pub lending_amount: Balance,
    pub lending_interest: Balance,
    pub last_lending_block: BlockNumberFor,
}

#[derive(Encode, Decode, Default, PartialEq, Eq, Clone, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct BorrowingPosition<BlockNumberFor> {
    pub collateral_amount: Balance,
    pub borrowing_amount: Balance,
    pub borrowing_interest: Balance,
    pub last_borrowing_block: BlockNumberFor,
    pub borrowing_rewards: Balance,
}

#[derive(Encode, Decode, Default, PartialEq, Eq, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct PoolInfo {
    pub total_liquidity: Balance,
    pub total_borrowed: Balance,
    pub total_collateral: Balance,
    pub basic_lending_rate: Balance,
    pub profit_lending_rate: Balance,
    pub borrowing_rate: Balance,
    pub borrowing_rewards_rate: Balance,
    pub loan_to_value: Balance,
    pub liquidation_threshold: Balance,
    pub optimal_utilization_rate: Balance,
    pub base_rate: Balance,
    pub slope_rate_1: Balance,
    pub slope_rate_2: Balance,
    pub reserve_factor: Balance,
    pub rewards: Balance,
}

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod test;

#[frame_support::pallet]
pub mod pallet {
    use crate::{BorrowingPosition, LendingPosition, PoolInfo};
    use common::prelude::{Balance, FixedWrapper, SwapAmount};
    use common::{balance, DEXId, LiquiditySourceFilter, PriceVariant, CERES_ASSET_ID, DAI, XOR};
    use common::{LiquidityProxyTrait, PriceToolsPallet, APOLLO_ASSET_ID};
    use frame_support::pallet_prelude::{ValueQuery, *};
    use frame_support::sp_runtime::traits::AccountIdConversion;
    use frame_support::PalletId;
    use frame_system::pallet_prelude::*;
    use frame_system::RawOrigin;
    use hex_literal::hex;
    use sp_runtime::traits::{UniqueSaturatedInto, Zero};
    use sp_std::collections::btree_map::BTreeMap;

    const PALLET_ID: PalletId = PalletId(*b"apollolb");

    #[pallet::config]
    pub trait Config:
        frame_system::Config + assets::Config + price_tools::Config + liquidity_proxy::Config
    {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type PriceTools: PriceToolsPallet<Self::AssetId>;
        type LiquidityProxyPallet: LiquidityProxyTrait<Self::DEXId, Self::AccountId, Self::AssetId>;
    }

    type Assets<T> = assets::Pallet<T>;
    // type PriceTools<T> = price_tools::Pallet<T>;
    // type LiquidityProxy<T> = liquidity_proxy::Pallet<T>;
    pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
    pub type AssetIdOf<T> = <T as assets::Config>::AssetId;

    #[pallet::pallet]
    #[pallet::generate_store(pub (super) trait Store)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(PhantomData<T>);

    /// AccountId -> Lended asset -> LendingPosition
    #[pallet::storage]
    #[pallet::getter(fn user_lending_info)]
    pub type UserLendingInfo<T: Config> = StorageDoubleMap<
        _,
        Identity,
        AccountIdOf<T>,
        Identity,
        AssetIdOf<T>,
        LendingPosition<BlockNumberFor<T>>,
        OptionQuery,
    >;

    /// AccountId -> Borrowed asset -> (Collateral asset, BorrowingPosition)
    #[pallet::storage]
    #[pallet::getter(fn user_borrowing_info)]
    pub type UserBorrowingInfo<T: Config> = StorageDoubleMap<
        _,
        Identity,
        AccountIdOf<T>,
        Identity,
        AssetIdOf<T>,
        BTreeMap<AssetIdOf<T>, BorrowingPosition<BlockNumberFor<T>>>,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn pool_info)]
    pub type PoolData<T: Config> = StorageMap<_, Identity, AssetIdOf<T>, PoolInfo, OptionQuery>;

    #[pallet::type_value]
    pub fn DefaultForAuthorityAccount<T: Config>() -> AccountIdOf<T> {
        let bytes = hex!("96ea3c9c0be7bbc7b0656a1983db5eed75210256891a9609012362e36815b132");
        AccountIdOf::<T>::decode(&mut &bytes[..]).unwrap()
    }

    #[pallet::type_value]
    pub fn DefaultForTreasuryAccount<T: Config>() -> AccountIdOf<T> {
        let bytes = hex!("04621d8671f5ccc9c4dd86898e346f8ca163bc290ceee6bd2a8376a6d6dd8547");
        AccountIdOf::<T>::decode(&mut &bytes[..]).unwrap()
    }

    #[pallet::storage]
    #[pallet::getter(fn authority_account)]
    pub type AuthorityAccount<T: Config> =
        StorageValue<_, AccountIdOf<T>, ValueQuery, DefaultForAuthorityAccount<T>>;

    #[pallet::storage]
    #[pallet::getter(fn treasury_account)]
    pub type TreasuryAccount<T: Config> =
        StorageValue<_, AccountIdOf<T>, ValueQuery, DefaultForTreasuryAccount<T>>;

    #[pallet::type_value]
    pub fn FixedLendingRewards<T: Config>() -> Balance {
        balance!(200000)
    }

    /// Default lending rewards
    #[pallet::storage]
    #[pallet::getter(fn lending_rewards)]
    pub type LendingRewards<T: Config> =
        StorageValue<_, Balance, ValueQuery, FixedLendingRewards<T>>;

    #[pallet::type_value]
    pub fn FixedBorrowingRewards<T: Config>() -> Balance {
        balance!(100000)
    }

    /// Default borrowing rewards
    #[pallet::storage]
    #[pallet::getter(fn borrowing_rewards)]
    pub type BorrowingRewards<T: Config> =
        StorageValue<_, Balance, ValueQuery, FixedBorrowingRewards<T>>;

    #[pallet::type_value]
    pub fn FixedLendingRewardsPerBlock<T: Config>() -> Balance {
        balance!(0.03805175)
    }

    /// Default lending rewards per block
    #[pallet::storage]
    #[pallet::getter(fn lending_rewards_per_block)]
    pub type LendingRewardsPerBlock<T: Config> =
        StorageValue<_, Balance, ValueQuery, FixedLendingRewardsPerBlock<T>>;

    #[pallet::type_value]
    pub fn FixedBorrowingRewardsPerBlock<T: Config>() -> Balance {
        balance!(0.01902587)
    }

    /// Default borrowing rewards
    #[pallet::storage]
    #[pallet::getter(fn borrowing_rewards_per_block)]
    pub type BorrowingRewardsPerBlock<T: Config> =
        StorageValue<_, Balance, ValueQuery, FixedBorrowingRewardsPerBlock<T>>;

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Pool added [who, asset_id]
        PoolAdded(AccountIdOf<T>, AssetIdOf<T>),
        /// Lended [who, asset_id, amount]
        Lended(AccountIdOf<T>, AssetIdOf<T>, Balance),
        /// Borrowed [who, collateral_asset, collateral_amount, borrow_asset, borrow_amount]
        Borrowed(AccountIdOf<T>, AssetIdOf<T>, Balance, AssetIdOf<T>, Balance),
        /// ClaimedLendingRewards [who, asset_id, amount]
        ClaimedLendingRewards(AccountIdOf<T>, AssetIdOf<T>, Balance),
        /// ClaimedBorrowingRewards [who, asset_id, amount]
        ClaimedBorrowingRewards(AccountIdOf<T>, AssetIdOf<T>, Balance),
        /// Withdrawn [who, asset_id, amount]
        Withdrawn(AccountIdOf<T>, AssetIdOf<T>, Balance),
        /// Repaid [who, asset_id, amount]
        Repaid(AccountIdOf<T>, AssetIdOf<T>, Balance),
        //// ChangedRewardsAmount [who, is_lending, amount]
        ChangedRewardsAmount(AccountIdOf<T>, bool, Balance),
        //// ChangedRewardsAmountPerBlock [who, is_lending, amount]
        ChangedRewardsAmountPerBlock(AccountIdOf<T>, bool, Balance),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Unauthorized
        Unauthorized,
        /// Asset already listed
        AssetAlreadyListed,
        /// Invalid pool parameters
        InvalidPoolParameters,
        /// Pool does not exist
        PoolDoesNotExist,
        /// The amount that is being lended is invalid
        InvalidLendingAmount,
        /// Collateral token does not exists
        CollateralTokenDoesNotExist,
        /// No lending amount to borrow
        NoLendingAmountToBorrow,
        /// Same borrowing and collateral assets
        SameCollateralAndBorrowingAssets,
        /// No liquidity for borrowing asset
        NoLiquidityForBorrowingAsset,
        /// Nothing lended
        NothingLended,
        /// Invalid collateral amount
        InvalidCollateralAmount,
        /// Can not transfer borrowing amount
        CanNotTransferBorrowingAmount,
        /// Can not transfer collateral amount
        CanNotTransferCollateralAmount,
        /// No rewards to claim
        NoRewardsToClaim,
        /// Unable to transfer rewards
        UnableToTransferRewards,
        /// Insufficient lending amount
        InsufficientLendingAmount,
        /// Lending amount exceeded
        LendingAmountExceeded,
        /// Can not transfer lending amount
        CanNotTransferLendingAmount,
        /// Nothing borrowed
        NothingBorrowed,
        /// Nonexistent borrowing position
        NonexistentBorrowingPosition,
        /// Nothing to repay
        NothingToRepay,
        /// Can not transfer lending interest
        CanNotTransferLendingInterest,
        /// Unable to transfer collateral
        UnableToTransferCollateral,
        /// Unable to transfer amount to repay
        UnableToTransferAmountToRepay,
        /// Can not withdraw lending amount
        CanNotWithdrawLendingAmount,
        /// Can not transfer borrowing rewards
        CanNotTransferBorrowingRewards,
        /// Can not transfer amount to repay
        CanNotTransferAmountToRepay,
        /// Can not transfer amount to developers
        CanNotTransferAmountToDevelopers,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Add pool
        #[pallet::call_index(0)]
        #[pallet::weight(10_000)]
        pub fn add_pool(
            origin: OriginFor<T>,
            asset_id: AssetIdOf<T>,
            loan_to_value: Balance,
            liquidation_threshold: Balance,
            optimal_utilization_rate: Balance,
            base_rate: Balance,
            slope_rate_1: Balance,
            slope_rate_2: Balance,
            reserve_factor: Balance,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            if user != AuthorityAccount::<T>::get() {
                return Err(Error::<T>::Unauthorized.into());
            }

            ensure!(
                !<PoolData<T>>::contains_key(asset_id),
                Error::<T>::AssetAlreadyListed
            );

            // Check parameters
            if loan_to_value > balance!(1)
                || liquidation_threshold > balance!(1)
                || optimal_utilization_rate > balance!(1)
                || reserve_factor > balance!(1)
            {
                return Err(Error::<T>::InvalidPoolParameters.into());
            }

            // Recalculate basic lending rate and borrowing rewards rate
            let mut num_of_pools = <PoolData<T>>::iter().count() as u32;
            num_of_pools += 1;

            let basic_lending_rate = (FixedWrapper::from(LendingRewardsPerBlock::<T>::get())
                / FixedWrapper::from(balance!(num_of_pools)))
            .try_into_balance()
            .unwrap_or(0);
            let borrowing_rewards_rate = (FixedWrapper::from(BorrowingRewardsPerBlock::<T>::get())
                / FixedWrapper::from(balance!(num_of_pools)))
            .try_into_balance()
            .unwrap_or(0);

            for (asset_id, mut pool_info) in <PoolData<T>>::iter() {
                pool_info.basic_lending_rate = basic_lending_rate;
                pool_info.borrowing_rewards_rate = borrowing_rewards_rate;
                <PoolData<T>>::insert(asset_id, pool_info);
            }

            // Create a new pool
            let pool_info = PoolInfo {
                total_liquidity: 0,
                total_borrowed: 0,
                total_collateral: 0,
                basic_lending_rate,
                profit_lending_rate: 0,
                borrowing_rate: 0,
                borrowing_rewards_rate,
                loan_to_value,
                liquidation_threshold,
                optimal_utilization_rate,
                base_rate,
                slope_rate_1,
                slope_rate_2,
                reserve_factor,
                rewards: 0,
            };

            <PoolData<T>>::insert(asset_id, pool_info);

            Self::deposit_event(Event::PoolAdded(user, asset_id));
            Ok(().into())
        }

        /// Lend token
        #[pallet::call_index(1)]
        #[pallet::weight(10_000)]
        pub fn lend(
            origin: OriginFor<T>,
            lending_asset: AssetIdOf<T>,
            lending_amount: Balance,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            ensure!(lending_amount > 0, Error::<T>::InvalidLendingAmount);

            let mut pool_info =
                <PoolData<T>>::get(lending_asset).ok_or(Error::<T>::PoolDoesNotExist)?;

            // Add lending amount and interest to user if exists, otherwise create new user
            if let Some(mut user_info) = <UserLendingInfo<T>>::get(user.clone(), lending_asset) {
                // Calculate interest in APOLLO token
                let block_number = <frame_system::Pallet<T>>::block_number();
                let interests =
                    Self::calculate_lending_earnings(&user, lending_asset, block_number);
                user_info.lending_interest += interests.0 + interests.1;
                user_info.lending_amount += lending_amount;
                user_info.last_lending_block = <frame_system::Pallet<T>>::block_number();
                <UserLendingInfo<T>>::insert(user.clone(), lending_asset, user_info);
            } else {
                let new_user_info = LendingPosition {
                    lending_amount,
                    lending_interest: 0,
                    last_lending_block: <frame_system::Pallet<T>>::block_number(),
                };
                <UserLendingInfo<T>>::insert(user.clone(), lending_asset, new_user_info);
            }

            // Transfer lending amount to pallet
            Assets::<T>::transfer_from(&lending_asset, &user, &Self::account_id(), lending_amount)
                .map_err(|_| Error::<T>::CanNotTransferLendingAmount)?;
            pool_info.total_liquidity += lending_amount;
            <PoolData<T>>::insert(lending_asset, pool_info);

            Self::deposit_event(Event::Lended(user, lending_asset, lending_amount));
            Ok(().into())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(10_000)]
        pub fn borrow(
            origin: OriginFor<T>,
            collateral_asset: AssetIdOf<T>,
            borrowing_asset: AssetIdOf<T>,
            borrowing_amount: Balance,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            ensure!(
                collateral_asset != borrowing_asset,
                Error::<T>::SameCollateralAndBorrowingAssets
            );

            let mut borrow_pool_info =
                <PoolData<T>>::get(borrowing_asset).ok_or(Error::<T>::PoolDoesNotExist)?;
            ensure!(
                borrowing_amount <= borrow_pool_info.total_liquidity,
                Error::<T>::NoLiquidityForBorrowingAsset
            );

            let mut collateral_pool_info =
                <PoolData<T>>::get(collateral_asset).ok_or(Error::<T>::PoolDoesNotExist)?;
            let mut user_lending_info = <UserLendingInfo<T>>::get(user.clone(), collateral_asset)
                .ok_or(Error::<T>::NothingLended)?;
            let collateral_asset_price = Self::get_price(collateral_asset);
            let borrow_asset_price = Self::get_price(borrowing_asset);

            // Calculate required collateral asset in dollars
            let coll_amount_in_dollars = ((FixedWrapper::from(borrowing_amount)
                / FixedWrapper::from(borrow_pool_info.loan_to_value))
                * FixedWrapper::from(borrow_asset_price))
            .try_into_balance()
            .unwrap_or(0);

            // Calculate collateral amount in tokens of chosen asset
            let collateral_amount = (FixedWrapper::from(coll_amount_in_dollars)
                / FixedWrapper::from(collateral_asset_price))
            .try_into_balance()
            .unwrap_or(0);

            ensure!(
                collateral_amount <= user_lending_info.lending_amount,
                Error::<T>::InvalidCollateralAmount
            );

            let mut borrow_info =
                <UserBorrowingInfo<T>>::get(user.clone(), borrowing_asset).unwrap_or_default();

            // Add borrowing amount, collateral amount and interest to user if exists, otherwise create new user
            if let Some(mut user_info) = borrow_info.get_mut(&collateral_asset) {
                let block_number = <frame_system::Pallet<T>>::block_number();
                let calculated_interest = Self::calculate_borrowing_interest_and_reward(
                    &user,
                    borrowing_asset,
                    collateral_asset,
                    block_number,
                );
                user_info.borrowing_interest += calculated_interest.0;
                user_info.collateral_amount += collateral_amount;
                user_info.borrowing_amount += borrowing_amount;
                user_info.last_borrowing_block = block_number;
            } else {
                let new_user_info = BorrowingPosition {
                    collateral_amount,
                    borrowing_amount,
                    borrowing_interest: 0,
                    last_borrowing_block: <frame_system::Pallet<T>>::block_number(),
                    borrowing_rewards: 0,
                };
                borrow_info.insert(collateral_asset, new_user_info);
            }
            <UserBorrowingInfo<T>>::insert(user.clone(), borrowing_asset, borrow_info);

            // Update user's lending info according to given collateral
            let block_number = <frame_system::Pallet<T>>::block_number();
            let interests = Self::calculate_lending_earnings(&user, collateral_asset, block_number);
            user_lending_info.lending_interest += interests.0 + interests.1;
            user_lending_info.lending_amount -= collateral_amount;
            user_lending_info.last_lending_block = <frame_system::Pallet<T>>::block_number();
            <UserLendingInfo<T>>::insert(user.clone(), collateral_asset, user_lending_info);

            // Update collateral and borrowing assets pools
            borrow_pool_info.total_liquidity -= borrowing_amount;
            borrow_pool_info.total_borrowed += borrowing_amount;
            collateral_pool_info.total_liquidity -= collateral_amount;
            collateral_pool_info.total_collateral += collateral_amount;

            <PoolData<T>>::insert(collateral_asset, collateral_pool_info);
            <PoolData<T>>::insert(borrowing_asset, borrow_pool_info);

            // Transfer borrowing amount to user
            Assets::<T>::transfer_from(
                &borrowing_asset,
                &Self::account_id(),
                &user,
                borrowing_amount,
            )
            .map_err(|_| Error::<T>::CanNotTransferBorrowingAmount)?;

            Self::deposit_event(Event::Borrowed(
                user,
                collateral_asset,
                collateral_amount,
                borrowing_asset,
                borrowing_amount,
            ));
            Ok(().into())
        }

        /// Get rewards
        #[pallet::call_index(3)]
        #[pallet::weight(10_000)]
        pub fn get_rewards(
            origin: OriginFor<T>,
            asset_id: AssetIdOf<T>,
            is_lending: bool,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            let block_number = <frame_system::Pallet<T>>::block_number();
            // Check if user has lended or borrowed rewards
            if is_lending {
                let mut lend_user_info = <UserLendingInfo<T>>::get(user.clone(), asset_id)
                    .ok_or(Error::<T>::NothingLended)?;
                let interests = Self::calculate_lending_earnings(&user, asset_id, block_number);
                lend_user_info.lending_interest += interests.0 + interests.1;
                lend_user_info.last_lending_block = <frame_system::Pallet<T>>::block_number();

                ensure!(
                    lend_user_info.lending_interest > 0,
                    Error::<T>::NoRewardsToClaim
                );

                Assets::<T>::transfer_from(
                    &APOLLO_ASSET_ID.into(),
                    &Self::account_id(),
                    &user,
                    lend_user_info.lending_interest,
                )
                .map_err(|_| Error::<T>::UnableToTransferRewards)?;

                let lending_rewards = lend_user_info.lending_interest;
                lend_user_info.lending_interest = 0;
                <UserLendingInfo<T>>::insert(user.clone(), asset_id, &lend_user_info);

                Self::deposit_event(Event::ClaimedLendingRewards(
                    user,
                    asset_id,
                    lending_rewards,
                ));
            } else {
                let mut user_infos = <UserBorrowingInfo<T>>::get(user.clone(), asset_id)
                    .ok_or(Error::<T>::NothingBorrowed)?;
                let block_number = <frame_system::Pallet<T>>::block_number();

                let mut borrowing_rewards = 0;
                for (collateral_asset, mut user_info) in user_infos.iter_mut() {
                    let interest_and_reward = Self::calculate_borrowing_interest_and_reward(
                        &user,
                        asset_id,
                        *collateral_asset,
                        block_number,
                    );
                    user_info.borrowing_interest += interest_and_reward.0;
                    user_info.borrowing_rewards += interest_and_reward.1;
                    user_info.last_borrowing_block = block_number;
                    borrowing_rewards += user_info.borrowing_rewards;
                    user_info.borrowing_rewards = 0;
                }

                ensure!(borrowing_rewards > 0, Error::<T>::NoRewardsToClaim);

                Assets::<T>::transfer_from(
                    &APOLLO_ASSET_ID.into(),
                    &Self::account_id(),
                    &user,
                    borrowing_rewards,
                )
                .map_err(|_| Error::<T>::UnableToTransferRewards)?;

                <UserBorrowingInfo<T>>::insert(user.clone(), asset_id, &user_infos);

                Self::deposit_event(Event::ClaimedBorrowingRewards(
                    user,
                    asset_id,
                    borrowing_rewards,
                ));
            }
            Ok(().into())
        }

        /// Withdraw
        #[pallet::call_index(4)]
        #[pallet::weight(10_000)]
        pub fn withdraw(
            origin: OriginFor<T>,
            withdrawn_asset: AssetIdOf<T>,
            withdrawn_amount: Balance,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            let mut pool_info =
                <PoolData<T>>::get(withdrawn_asset).ok_or(Error::<T>::PoolDoesNotExist)?;
            let mut user_info = <UserLendingInfo<T>>::get(user.clone(), withdrawn_asset)
                .ok_or(Error::<T>::NothingLended)?;

            ensure!(
                withdrawn_amount <= user_info.lending_amount,
                Error::<T>::LendingAmountExceeded
            );
            ensure!(
                withdrawn_amount < pool_info.total_liquidity,
                Error::<T>::CanNotTransferLendingAmount
            );

            // Transfer lending amount
            Assets::<T>::transfer_from(
                &withdrawn_asset,
                &Self::account_id(),
                &user,
                withdrawn_amount,
            )
            .map_err(|_| Error::<T>::CanNotTransferLendingAmount)?;

            let previous_lending_amount = user_info.lending_amount;

            let block_number = <frame_system::Pallet<T>>::block_number();
            let interests: (u128, u128) =
                Self::calculate_lending_earnings(&user, withdrawn_asset, block_number);
            user_info.lending_amount -= withdrawn_amount;
            user_info.lending_interest += interests.0 + interests.1;
            user_info.last_lending_block = block_number;

            // Check if lending amount is less than user's lending amount
            if withdrawn_amount < previous_lending_amount {
                <UserLendingInfo<T>>::insert(user.clone(), withdrawn_asset, user_info);
            } else {
                // Transfer lending interest when user withdraws whole lending amount
                Assets::<T>::transfer_from(
                    &APOLLO_ASSET_ID.into(),
                    &Self::account_id(),
                    &user,
                    user_info.lending_interest,
                )
                .map_err(|_| Error::<T>::CanNotTransferLendingInterest)?;
                <UserLendingInfo<T>>::remove(user.clone(), withdrawn_asset);
            }

            pool_info.total_liquidity -= withdrawn_amount;
            <PoolData<T>>::insert(withdrawn_asset, pool_info);

            Self::deposit_event(Event::Withdrawn(user, withdrawn_asset, withdrawn_amount));
            Ok(().into())
        }

        #[pallet::call_index(5)]
        #[pallet::weight(10_000)]
        pub fn repay(
            origin: OriginFor<T>,
            collateral_asset: AssetIdOf<T>,
            borrowing_asset: AssetIdOf<T>,
            amount_to_repay: Balance,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            let mut borrow_pool_info =
                <PoolData<T>>::get(borrowing_asset).ok_or(Error::<T>::PoolDoesNotExist)?;
            let mut collateral_pool_info =
                <PoolData<T>>::get(collateral_asset).ok_or(Error::<T>::PoolDoesNotExist)?;
            let mut borrow_user_info = <UserBorrowingInfo<T>>::get(user.clone(), borrowing_asset)
                .ok_or(Error::<T>::NothingBorrowed)?;
            let mut user_info = borrow_user_info
                .get(&collateral_asset)
                .cloned()
                .ok_or(Error::<T>::NonexistentBorrowingPosition)?;

            let block_number = <frame_system::Pallet<T>>::block_number();
            let interest_and_reward = Self::calculate_borrowing_interest_and_reward(
                &user,
                borrowing_asset,
                collateral_asset,
                block_number,
            );
            user_info.borrowing_interest += interest_and_reward.0;
            user_info.last_borrowing_block = block_number;

            if amount_to_repay <= user_info.borrowing_interest {
                // If user is repaying only part or whole interest
                user_info.borrowing_interest -= amount_to_repay;
                borrow_user_info.insert(collateral_asset, user_info);

                Assets::<T>::transfer_from(
                    &borrowing_asset,
                    &user,
                    &Self::account_id(),
                    amount_to_repay,
                )
                .map_err(|_| Error::<T>::CanNotTransferAmountToRepay)?;

                <UserBorrowingInfo<T>>::insert(user.clone(), borrowing_asset, &borrow_user_info);

                Self::distribute_protocol_interest(borrowing_asset, amount_to_repay)?;
            } else if amount_to_repay > user_info.borrowing_interest
                && amount_to_repay < user_info.borrowing_interest + user_info.borrowing_amount
            {
                // If user is repaying whole interest plus part of the borrowed amount
                let repaid_amount = user_info.borrowing_interest;
                let remaining_amount = amount_to_repay - user_info.borrowing_interest;
                user_info.borrowing_amount -= remaining_amount;
                user_info.borrowing_interest = 0;
                borrow_pool_info.total_borrowed -= remaining_amount;
                borrow_pool_info.total_liquidity += remaining_amount;
                <PoolData<T>>::insert(borrowing_asset, borrow_pool_info);

                Assets::<T>::transfer_from(
                    &borrowing_asset,
                    &user,
                    &Self::account_id(),
                    amount_to_repay,
                )
                .map_err(|_| Error::<T>::CanNotTransferAmountToRepay)?;

                borrow_user_info.insert(collateral_asset, user_info);
                <UserBorrowingInfo<T>>::insert(user.clone(), borrowing_asset, &borrow_user_info);

                Self::distribute_protocol_interest(borrowing_asset, repaid_amount)?;
            } else if amount_to_repay >= user_info.borrowing_interest + user_info.borrowing_amount {
                // If user is repaying the whole position
                let total_borrowed_amount =
                    user_info.borrowing_interest + user_info.borrowing_amount;

                // Update pools
                borrow_pool_info.total_borrowed -= user_info.borrowing_amount;
                borrow_pool_info.total_liquidity += user_info.borrowing_amount;
                collateral_pool_info.total_collateral -= user_info.collateral_amount;

                <PoolData<T>>::insert(collateral_asset, collateral_pool_info);
                <PoolData<T>>::insert(borrowing_asset, borrow_pool_info);

                // Transfer borrowing amount and borrowing interest to pallet
                Assets::<T>::transfer_from(
                    &borrowing_asset,
                    &user,
                    &Self::account_id(),
                    total_borrowed_amount,
                )
                .map_err(|_| Error::<T>::CanNotTransferBorrowingAmount)?;

                // Transfer collateral to user
                Assets::<T>::transfer_from(
                    &collateral_asset,
                    &Self::account_id(),
                    &user,
                    user_info.collateral_amount,
                )
                .map_err(|_| Error::<T>::UnableToTransferCollateral)?;

                // Transfer borrowing rewards to user
                Assets::<T>::transfer_from(
                    &APOLLO_ASSET_ID.into(),
                    &Self::account_id(),
                    &user,
                    user_info.borrowing_rewards,
                )
                .map_err(|_| Error::<T>::CanNotTransferBorrowingRewards)?;

                <UserBorrowingInfo<T>>::remove(user.clone(), borrowing_asset);
                Self::distribute_protocol_interest(borrowing_asset, user_info.borrowing_interest)?;
            }

            Self::deposit_event(Event::Repaid(user, borrowing_asset, amount_to_repay));
            Ok(().into())
        }

        #[pallet::call_index(6)]
        #[pallet::weight(10_000)]
        pub fn change_rewards_amount(
            origin: OriginFor<T>,
            is_lending: bool,
            amount: Balance,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            if user != AuthorityAccount::<T>::get() {
                return Err(Error::<T>::Unauthorized.into());
            }

            if is_lending {
                <LendingRewards<T>>::put(amount);
            } else {
                <BorrowingRewards<T>>::put(amount);
            }

            Self::deposit_event(Event::ChangedRewardsAmount(user, is_lending, amount));
            Ok(().into())
        }

        #[pallet::call_index(7)]
        #[pallet::weight(10_000)]
        pub fn change_rewards_per_block(
            origin: OriginFor<T>,
            is_lending: bool,
            amount: Balance,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            if user != AuthorityAccount::<T>::get() {
                return Err(Error::<T>::Unauthorized.into());
            }

            let num_of_pools = <PoolData<T>>::iter().count() as u32;

            if is_lending {
                // Recalculate basic lending rate
                let basic_lending_rate = (FixedWrapper::from(amount)
                    / FixedWrapper::from(balance!(num_of_pools)))
                .try_into_balance()
                .unwrap_or(0);
                for (asset_id, mut pool_info) in <PoolData<T>>::iter() {
                    pool_info.basic_lending_rate = basic_lending_rate;
                    <PoolData<T>>::insert(asset_id, pool_info);
                }
                <LendingRewardsPerBlock<T>>::put(amount);
            } else {
                // Recalculate borrowing rewards rate
                let borrowing_rewards_rate = (FixedWrapper::from(amount)
                    / FixedWrapper::from(balance!(num_of_pools)))
                .try_into_balance()
                .unwrap_or(0);
                for (asset_id, mut pool_info) in <PoolData<T>>::iter() {
                    pool_info.borrowing_rewards_rate = borrowing_rewards_rate;
                    <PoolData<T>>::insert(asset_id, pool_info);
                }

                <BorrowingRewardsPerBlock<T>>::put(amount);
            }

            Self::deposit_event(Event::ChangedRewardsAmountPerBlock(
                user, is_lending, amount,
            ));
            Ok(().into())
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(now: T::BlockNumber) -> Weight {
            let distribution_rewards = Self::update_interests(now);
            let rates = Self::update_rates(now);
            let liquidation = Self::check_liquidations(now);

            distribution_rewards
                .saturating_add(rates)
                .saturating_add(liquidation)
        }
    }

    impl<T: Config> Pallet<T> {
        /// The account ID of pallet
        fn account_id() -> T::AccountId {
            PALLET_ID.into_account_truncating()
        }

        pub fn get_price(asset_id: AssetIdOf<T>) -> Balance {
            // Get XOR price from the spot price function in PriceTools pallet
            let xor_price = T::PriceTools::spot_price(&DAI.into()).unwrap();

            if asset_id == XOR.into() {
                return xor_price;
            }

            // Get average price from PriceTools pallet
            let buy_price =
                T::PriceTools::get_average_price(&XOR.into(), &asset_id, PriceVariant::Buy)
                    .unwrap();

            let sell_price =
                T::PriceTools::get_average_price(&XOR.into(), &asset_id, PriceVariant::Sell)
                    .unwrap();

            // Average price in dollars
            (FixedWrapper::from(xor_price * FixedWrapper::from(buy_price + sell_price))
                / FixedWrapper::from(balance!(2)))
            .try_into_balance()
            .unwrap_or(0)
        }

        pub fn calculate_lending_earnings(
            user: &AccountIdOf<T>,
            asset_id: AssetIdOf<T>,
            block_number: BlockNumberFor<T>,
        ) -> (Balance, Balance) {
            let user_info = UserLendingInfo::<T>::get(user, asset_id).unwrap();
            let pool_info = PoolData::<T>::get(asset_id).unwrap();

            let total_lending_blocks: u128 =
                (block_number - user_info.last_lending_block).unique_saturated_into();

            let share_in_pool = FixedWrapper::from(user_info.lending_amount)
                / FixedWrapper::from(pool_info.total_liquidity);

            // Rewards from initial APOLLO distribution
            let basic_reward_per_block =
                FixedWrapper::from(pool_info.basic_lending_rate) * share_in_pool.clone();

            // Rewards from profit made through repayments and liquidations
            let profit_reward_per_block =
                FixedWrapper::from(pool_info.profit_lending_rate) * share_in_pool.clone();

            // Return (basic_lending_interest, profit_lending_interest)
            (
                (basic_reward_per_block * FixedWrapper::from(balance!(total_lending_blocks)))
                    .try_into_balance()
                    .unwrap_or(0),
                (profit_reward_per_block * FixedWrapper::from(balance!(total_lending_blocks)))
                    .try_into_balance()
                    .unwrap_or(0),
            )
        }

        fn calculate_borrowing_interest_and_reward(
            user: &AccountIdOf<T>,
            asset_id: AssetIdOf<T>,
            collateral_asset: AssetIdOf<T>,
            block_number: BlockNumberFor<T>,
        ) -> (Balance, Balance) {
            let borrow_user_info = UserBorrowingInfo::<T>::get(user, asset_id).unwrap();
            let user_info = borrow_user_info.get(&collateral_asset).unwrap();
            let pool_info = PoolData::<T>::get(asset_id).unwrap();
            let total_borrowing_blocks: u128 =
                (block_number - user_info.last_borrowing_block).unique_saturated_into();

            // Calculate borrowing interest
            let borrowing_interest_per_block = FixedWrapper::from(user_info.borrowing_amount)
                * FixedWrapper::from(pool_info.borrowing_rate);

            // Calculate borrowing reward
            let share_in_pool = FixedWrapper::from(user_info.borrowing_amount)
                / FixedWrapper::from(pool_info.total_borrowed);

            let borrowing_reward_per_block =
                FixedWrapper::from(pool_info.borrowing_rewards_rate) * share_in_pool;

            // Return (borrowing_interest, borrowing_reward)
            (
                (borrowing_interest_per_block
                    * FixedWrapper::from(balance!(total_borrowing_blocks)))
                .try_into_balance()
                .unwrap_or(0),
                (borrowing_reward_per_block * FixedWrapper::from(balance!(total_borrowing_blocks)))
                    .try_into_balance()
                    .unwrap_or(0),
            )
        }

        fn distribute_protocol_interest(
            asset_id: AssetIdOf<T>,
            amount: Balance,
        ) -> DispatchResultWithPostInfo {
            let mut pool_info = PoolData::<T>::get(asset_id).ok_or(Error::<T>::PoolDoesNotExist)?;
            let caller = Self::account_id();

            // Calculate rewards and reserves amounts based on Reserve Factor
            let reserves_amount = (FixedWrapper::from(pool_info.reserve_factor)
                * FixedWrapper::from(amount))
            .try_into_balance()
            .unwrap_or(0);
            let rewards_amount = amount - reserves_amount;

            let outcome = T::LiquidityProxyPallet::exchange(
                DEXId::Polkaswap.into(),
                &caller,
                &caller,
                &asset_id,
                &APOLLO_ASSET_ID.into(),
                SwapAmount::with_desired_input(rewards_amount, Balance::zero()),
                LiquiditySourceFilter::empty(DEXId::Polkaswap.into()),
            )
            .unwrap();

            let buyback_amount = outcome.amount;

            pool_info.rewards += buyback_amount;

            <PoolData<T>>::insert(asset_id, pool_info);

            // Calculate 60% of reserves to transfer APOLLO to treasury
            let apollo_amount = (FixedWrapper::from(reserves_amount)
                * FixedWrapper::from(balance!(0.6)))
            .try_into_balance()
            .unwrap_or(0);

            // Calculate 20% of reserves to buyback CERES
            let ceres_amount = (FixedWrapper::from(reserves_amount)
                * FixedWrapper::from(balance!(0.2)))
            .try_into_balance()
            .unwrap_or(0);

            // Calculate 20% of reserves to go to developer fund
            let developer_amount = (FixedWrapper::from(reserves_amount)
                * FixedWrapper::from(balance!(0.2)))
            .try_into_balance()
            .unwrap_or(0);

            // Transfer APOLLO to treasury
            T::LiquidityProxyPallet::exchange(
                DEXId::Polkaswap.into(),
                &caller,
                &TreasuryAccount::<T>::get(), // APOLLO Treasury
                &asset_id,
                &APOLLO_ASSET_ID.into(),
                SwapAmount::with_desired_input(apollo_amount, Balance::zero()),
                LiquiditySourceFilter::empty(DEXId::Polkaswap.into()),
            )?;

            // Buyback and burn CERES
            let outcome = T::LiquidityProxyPallet::exchange(
                DEXId::Polkaswap.into(),
                &caller,
                &caller,
                &asset_id,
                &CERES_ASSET_ID.into(),
                SwapAmount::with_desired_input(ceres_amount, Balance::zero()),
                LiquiditySourceFilter::empty(DEXId::Polkaswap.into()),
            )
            .unwrap();

            Assets::<T>::burn(
                RawOrigin::Signed(caller.clone()).into(),
                CERES_ASSET_ID.into(),
                outcome.amount,
            )?;

            // Transfer amount to developer fund
            Assets::<T>::transfer_from(
                &asset_id,
                &Self::account_id(),
                &AuthorityAccount::<T>::get(),
                developer_amount,
            )
            .map_err(|_| Error::<T>::CanNotTransferAmountToDevelopers)?;

            Ok(().into())
        }

        fn update_interests(block_number: BlockNumberFor<T>) -> Weight {
            let mut counter: u64 = 0;

            // Update lending interests
            let mut rewards_by_pools: BTreeMap<AssetIdOf<T>, Balance> = BTreeMap::new();
            for (user, asset_id, mut user_info) in UserLendingInfo::<T>::iter() {
                let user_interests =
                    Self::calculate_lending_earnings(&user, asset_id, block_number);
                user_info.lending_interest += user_interests.0 + user_interests.1;
                user_info.last_lending_block = block_number;

                if rewards_by_pools.contains_key(&asset_id) {
                    let mut rewards = rewards_by_pools[&asset_id];
                    rewards += user_interests.1;
                    rewards_by_pools.insert(asset_id, rewards);
                } else {
                    rewards_by_pools.insert(asset_id, user_interests.1);
                }

                <UserLendingInfo<T>>::insert(user.clone(), asset_id, user_info);
                counter += 1;
            }

            for (pool_asset_id, mut pool_info) in PoolData::<T>::iter() {
                pool_info.rewards = pool_info
                    .rewards
                    .checked_sub(rewards_by_pools[&pool_asset_id])
                    .unwrap_or(0);
                <PoolData<T>>::insert(pool_asset_id, pool_info);
                counter += 1;
            }

            // Update borrowing interests
            for (user, asset_id, mut user_infos) in UserBorrowingInfo::<T>::iter() {
                for (collateral_asset, mut user_info) in user_infos.iter_mut() {
                    let user_interests = Self::calculate_borrowing_interest_and_reward(
                        &user,
                        asset_id,
                        *collateral_asset,
                        block_number,
                    );
                    user_info.borrowing_interest += user_interests.0;
                    user_info.borrowing_rewards += user_interests.1;
                    user_info.last_borrowing_block = block_number;
                }
                <UserBorrowingInfo<T>>::insert(user.clone(), asset_id, user_infos.clone());
                counter += 1;
            }

            <LendingRewards<T>>::put(
                <LendingRewards<T>>::get() - <LendingRewardsPerBlock<T>>::get(),
            );
            <BorrowingRewards<T>>::put(
                <BorrowingRewards<T>>::get() - <BorrowingRewardsPerBlock<T>>::get(),
            );

            T::DbWeight::get()
                .reads(counter + 4)
                .saturating_add(T::DbWeight::get().writes(counter + 4))
        }

        fn update_rates(_current_block: T::BlockNumber) -> Weight {
            let mut counter: u64 = 0;

            for (asset_id, mut pool_info) in PoolData::<T>::iter() {
                let utilization_rate = (FixedWrapper::from(pool_info.total_borrowed)
                    / FixedWrapper::from(pool_info.total_liquidity))
                .try_into_balance()
                .unwrap_or(0);

                if utilization_rate < pool_info.optimal_utilization_rate {
                    // Update lending rate
                    pool_info.profit_lending_rate = (FixedWrapper::from(pool_info.rewards)
                        / FixedWrapper::from(balance!(5256000)))
                    .try_into_balance()
                    .unwrap_or(0);

                    // Update borrowing_rate -> Rt = (R0 + (U / Uopt) * Rslope1) / one_year
                    pool_info.borrowing_rate = ((FixedWrapper::from(pool_info.base_rate)
                        + (FixedWrapper::from(utilization_rate)
                            / FixedWrapper::from(pool_info.optimal_utilization_rate))
                            * FixedWrapper::from(pool_info.slope_rate_1))
                        / FixedWrapper::from(balance!(5256000)))
                    .try_into_balance()
                    .unwrap_or(0);
                } else {
                    // Update lending rate
                    pool_info.profit_lending_rate = ((FixedWrapper::from(pool_info.rewards)
                        / FixedWrapper::from(balance!(5256000)))
                        * (FixedWrapper::from(balance!(1)) + FixedWrapper::from(utilization_rate)))
                    .try_into_balance()
                    .unwrap_or(0);

                    // Update borrowing_rate -> Rt = (R0 + Rslope1 + ((Ut - Uopt) / (1 - Uopt)) * Rslope2) / one_year
                    pool_info.borrowing_rate = (FixedWrapper::from(pool_info.base_rate)
                        + FixedWrapper::from(pool_info.slope_rate_1)
                        + ((FixedWrapper::from(utilization_rate)
                            - FixedWrapper::from(pool_info.optimal_utilization_rate))
                            / (FixedWrapper::from(balance!(1))
                                - FixedWrapper::from(pool_info.optimal_utilization_rate)))
                            * FixedWrapper::from(pool_info.slope_rate_2))
                    .try_into_balance()
                    .unwrap_or(0);
                }

                <PoolData<T>>::insert(asset_id, pool_info);
                counter += 1;
            }

            T::DbWeight::get()
                .reads(counter)
                .saturating_add(T::DbWeight::get().writes(counter))
        }

        fn check_liquidations(_current_block: T::BlockNumber) -> Weight {
            let mut counter: u64 = 0;

            for (user, asset_id, user_infos) in UserBorrowingInfo::<T>::iter() {
                // Calculate health factor -> HF = SUM(collateral_in_dollars * liquidation_threshold) / total_borrowed_in_dollars
                let mut borrow_pool_info = PoolData::<T>::get(asset_id).unwrap();
                let mut sum_of_thresholds: Balance = 0;
                let mut total_borrowed: Balance = 0;
                let mut collaterals: BTreeMap<AssetIdOf<T>, Balance> = BTreeMap::new();

                for (collateral_asset, user_info) in user_infos.iter() {
                    let collateral_pool_info = PoolData::<T>::get(collateral_asset).unwrap();
                    let collateral_asset_price = Self::get_price(*collateral_asset);

                    // Multiply collateral value and liquidation threshold and then add it to the sum
                    let collateral_in_dollars = FixedWrapper::from(user_info.collateral_amount)
                        * FixedWrapper::from(collateral_asset_price);

                    sum_of_thresholds += (collateral_in_dollars
                        * FixedWrapper::from(collateral_pool_info.liquidation_threshold))
                    .try_into_balance()
                    .unwrap_or(0);

                    // Add borrowing amount to total borrowed
                    total_borrowed += user_info.borrowing_amount;

                    collaterals.insert(*collateral_asset, user_info.collateral_amount);
                }

                let borrowing_asset_price = Self::get_price(asset_id);
                let total_borrowed_in_dollars: u128 = (FixedWrapper::from(total_borrowed)
                    * FixedWrapper::from(borrowing_asset_price))
                .try_into_balance()
                .unwrap_or(0);

                let health_factor = (FixedWrapper::from(sum_of_thresholds)
                    / FixedWrapper::from(total_borrowed_in_dollars))
                .try_into_balance()
                .unwrap_or(0);

                // Check liquidation
                if health_factor < balance!(1) {
                    // Distribute liquidated collaterals to users and reserves
                    for (collateral_asset, collateral_amount) in collaterals.iter() {
                        let _ = Self::distribute_protocol_interest(
                            *collateral_asset,
                            *collateral_amount,
                        );
                        let mut collateral_pool_info =
                            PoolData::<T>::get(*collateral_asset).unwrap();
                        collateral_pool_info.total_collateral -= *collateral_amount;
                        <PoolData<T>>::insert(*collateral_asset, collateral_pool_info);
                        counter += 1;
                    }
                    borrow_pool_info.total_borrowed -= total_borrowed;
                    <PoolData<T>>::insert(asset_id, borrow_pool_info);
                    <UserBorrowingInfo<T>>::remove(user.clone(), asset_id);
                    counter += 2;
                }
            }

            T::DbWeight::get()
                .reads(counter + 2)
                .saturating_add(T::DbWeight::get().writes(counter))
        }
    }
}
