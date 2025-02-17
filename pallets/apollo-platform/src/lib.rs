#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]
use codec::{Decode, Encode};
use common::Balance;
pub use weights::WeightInfo;

mod benchmarking;
pub mod weights;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

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
    pub is_removed: bool,
}

pub use pallet::*;
pub mod migrations;

#[frame_support::pallet]
pub mod pallet {
    use crate::{BorrowingPosition, LendingPosition, PoolInfo, WeightInfo};
    use common::prelude::{Balance, FixedWrapper, SwapAmount};
    use common::{
        balance, AssetIdOf, AssetManager, DEXId, LiquiditySourceFilter, PriceVariant,
        CERES_ASSET_ID, DAI, KUSD,
    };
    use common::{LiquidityProxyTrait, PriceToolsProvider, APOLLO_ASSET_ID};
    use frame_support::log::{debug, warn};
    use frame_support::pallet_prelude::{ValueQuery, *};
    use frame_support::sp_runtime::traits::AccountIdConversion;
    use frame_support::traits::StorageVersion;
    use frame_support::PalletId;
    use frame_system::offchain::{SendTransactionTypes, SubmitTransaction};
    use frame_system::pallet_prelude::*;
    use frame_system::RawOrigin;
    use hex_literal::hex;
    use sp_runtime::traits::{UniqueSaturatedInto, Zero};
    use sp_std::collections::btree_map::BTreeMap;

    const PALLET_ID: PalletId = PalletId(*b"apollolb");

    #[pallet::config]
    pub trait Config:
        frame_system::Config
        + liquidity_proxy::Config
        + trading_pair::Config
        + common::Config
        + SendTransactionTypes<Call<Self>>
    {
        const BLOCKS_PER_FIFTEEN_MINUTES: BlockNumberFor<Self>;
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type PriceTools: PriceToolsProvider<AssetIdOf<Self>>;
        type LiquidityProxyPallet: LiquidityProxyTrait<
            Self::DEXId,
            Self::AccountId,
            AssetIdOf<Self>,
        >;

        /// A configuration for base priority of unsigned transactions.
        #[pallet::constant]
        type UnsignedPriority: Get<TransactionPriority>;

        /// A configuration for longevity of unsigned transactions.
        #[pallet::constant]
        type UnsignedLongevity: Get<u64>;

        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

    /// The current storage version.
    pub const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::generate_store(pub (super) trait Store)]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(PhantomData<T>);

    /// Lent asset -> AccountId -> LendingPosition
    #[pallet::storage]
    #[pallet::getter(fn user_lending_info)]
    pub type UserLendingInfo<T: Config> = StorageDoubleMap<
        _,
        Identity,
        AssetIdOf<T>,
        Identity,
        AccountIdOf<T>,
        LendingPosition<BlockNumberFor<T>>,
        OptionQuery,
    >;

    /// Borrowed asset -> AccountId -> (Collateral asset, BorrowingPosition)
    #[pallet::storage]
    #[pallet::getter(fn user_borrowing_info)]
    pub type UserBorrowingInfo<T: Config> = StorageDoubleMap<
        _,
        Identity,
        AssetIdOf<T>,
        Identity,
        AccountIdOf<T>,
        BTreeMap<AssetIdOf<T>, BorrowingPosition<BlockNumberFor<T>>>,
        OptionQuery,
    >;

    /// User AccountId -> Collateral Asset -> Total Collateral Amount
    #[pallet::storage]
    #[pallet::getter(fn user_total_collateral)]
    pub type UserTotalCollateral<T: Config> =
        StorageDoubleMap<_, Identity, AccountIdOf<T>, Identity, AssetIdOf<T>, Balance, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn pool_info)]
    pub type PoolData<T: Config> = StorageMap<_, Identity, AssetIdOf<T>, PoolInfo, OptionQuery>;

    /// BlockNumber -> AssetId (for updating pools interests by block)
    #[pallet::storage]
    #[pallet::getter(fn pools_by_block)]
    pub type PoolsByBlock<T: Config> =
        StorageMap<_, Identity, BlockNumberFor<T>, AssetIdOf<T>, OptionQuery>;

    #[pallet::type_value]
    pub fn DefaultForAuthorityAccount<T: Config>() -> AccountIdOf<T> {
        let bytes = hex!("04beb508e2b0da93e9ab77d65934562f55d11452f0582a31f61d2257fa4e3625");
        AccountIdOf::<T>::decode(&mut &bytes[..]).unwrap()
    }

    #[pallet::type_value]
    pub fn DefaultForTreasuryAccount<T: Config>() -> AccountIdOf<T> {
        let bytes = hex!("987579f1d0158f7d3507f0516ac156547f0d3066bbffca4bb6d186291bbd7c11");
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

    /// Default collateral factor
    #[pallet::type_value]
    pub fn DefaultCollateralFactor<T: Config>() -> Balance {
        balance!(0.001)
    }

    #[pallet::storage]
    #[pallet::getter(fn collateral_factor)]
    pub type CollateralFactor<T: Config> =
        StorageValue<_, Balance, ValueQuery, DefaultCollateralFactor<T>>;

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
        /// Lent [who, asset_id, amount]
        Lent(AccountIdOf<T>, AssetIdOf<T>, Balance),
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
        /// Changed Borrowing factor [who, amount]
        ChangedCollateralFactorAmount(AccountIdOf<T>, Balance),
        /// Liquidated [who, asset_id]
        Liquidated(AccountIdOf<T>, AssetIdOf<T>),
        /// Pool removed [who, asset_id]
        PoolRemoved(AccountIdOf<T>, AssetIdOf<T>),
        /// Pool info edited [who, asset_id]
        PoolInfoEdited(AccountIdOf<T>, AssetIdOf<T>),
        /// Collateral added [who, collateral_asset, collateral_amount, borrow_asset]
        CollateralAdded(AccountIdOf<T>, AssetIdOf<T>, Balance, AssetIdOf<T>),
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
        /// The amount that is being lent is invalid
        InvalidLendingAmount,
        /// Collateral token does not exists
        CollateralTokenDoesNotExist,
        /// No lending amount to borrow
        NoLendingAmountToBorrow,
        /// Same borrowing and collateral assets
        SameCollateralAndBorrowingAssets,
        /// No liquidity for borrowing asset
        NoLiquidityForBorrowingAsset,
        /// Nothing lent
        NothingLent,
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
        /// User should not be liquidated
        InvalidLiquidation,
        /// Pool is removed
        PoolIsRemoved,
        /// Invalid borrowing amount
        InvalidBorrowingAmount,
        /// Invalid loan to value
        InvalidLoanToValue,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Add pool
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::add_pool())]
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

            if let Some(pool_info) = <PoolData<T>>::get(asset_id) {
                ensure!(pool_info.is_removed, Error::<T>::AssetAlreadyListed);
            }

            // Check parameters
            if loan_to_value > balance!(1)
                || liquidation_threshold > balance!(1)
                || optimal_utilization_rate > balance!(1)
                || reserve_factor > balance!(1)
            {
                return Err(Error::<T>::InvalidPoolParameters.into());
            }

            // Recalculate basic lending rate and borrowing rewards rate
            let mut num_of_pools = <PoolData<T>>::iter()
                .filter(|(_, pool_info)| !pool_info.is_removed)
                .count() as u32;
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
                if pool_info.is_removed {
                    continue;
                }
                pool_info.basic_lending_rate = basic_lending_rate;
                pool_info.borrowing_rewards_rate = borrowing_rewards_rate;
                <PoolData<T>>::insert(asset_id, pool_info);
            }

            if let Some(mut pool_info) = <PoolData<T>>::get(asset_id) {
                pool_info.basic_lending_rate = basic_lending_rate;
                pool_info.borrowing_rewards_rate = borrowing_rewards_rate;
                pool_info.loan_to_value = loan_to_value;
                pool_info.liquidation_threshold = liquidation_threshold;
                pool_info.optimal_utilization_rate = optimal_utilization_rate;
                pool_info.base_rate = base_rate;
                pool_info.slope_rate_1 = slope_rate_1;
                pool_info.slope_rate_2 = slope_rate_2;
                pool_info.reserve_factor = reserve_factor;
                pool_info.is_removed = false;
                <PoolData<T>>::insert(asset_id, pool_info);
            } else {
                // Create a new pool
                let new_pool_info = PoolInfo {
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
                    is_removed: false,
                };

                <PoolData<T>>::insert(asset_id, new_pool_info);

                // Add pool to PoolsByBlock map
                let num_of_pools = <PoolsByBlock<T>>::iter().count() as u32;
                let block_number: BlockNumberFor<T> = num_of_pools.into();
                <PoolsByBlock<T>>::insert(block_number, asset_id);
            }

            // Register asset on PriceTools
            if !T::PriceTools::is_asset_registered(&asset_id) {
                T::PriceTools::register_asset(&asset_id)?;
            }

            Self::deposit_event(Event::PoolAdded(user, asset_id));
            Ok(().into())
        }

        /// Lend token
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::lend())]
        pub fn lend(
            origin: OriginFor<T>,
            lending_asset: AssetIdOf<T>,
            lending_amount: Balance,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            // Check if lending amount is minimum 10$
            let lending_asset_price = Self::get_price(lending_asset);
            let lending_amount_in_dollars: u128 = (FixedWrapper::from(lending_amount)
                * FixedWrapper::from(lending_asset_price))
            .try_into_balance()
            .unwrap_or(0);
            ensure!(
                lending_amount_in_dollars >= balance!(10),
                Error::<T>::InvalidLendingAmount
            );

            let mut pool_info =
                <PoolData<T>>::get(lending_asset).ok_or(Error::<T>::PoolDoesNotExist)?;
            ensure!(!pool_info.is_removed, Error::<T>::PoolIsRemoved);

            // Add lending amount and interest to user if exists, otherwise create new user
            if let Some(mut user_info) = <UserLendingInfo<T>>::get(lending_asset, user.clone()) {
                // Calculate interest in APOLLO token
                let block_number = <frame_system::Pallet<T>>::block_number();
                let interests =
                    Self::calculate_lending_earnings(&user_info, &pool_info, block_number);
                user_info.lending_interest += interests.0 + interests.1;
                user_info.lending_amount += lending_amount;
                user_info.last_lending_block = <frame_system::Pallet<T>>::block_number();
                <UserLendingInfo<T>>::insert(lending_asset, user.clone(), user_info);
            } else {
                let new_user_info = LendingPosition {
                    lending_amount,
                    lending_interest: 0,
                    last_lending_block: <frame_system::Pallet<T>>::block_number(),
                };
                <UserLendingInfo<T>>::insert(lending_asset, user.clone(), new_user_info);
            }

            // Transfer lending amount to pallet
            T::AssetManager::transfer_from(
                &lending_asset,
                &user,
                &Self::account_id(),
                lending_amount,
            )
            .map_err(|_| Error::<T>::CanNotTransferLendingAmount)?;
            pool_info.total_liquidity += lending_amount;
            <PoolData<T>>::insert(lending_asset, pool_info);

            Self::deposit_event(Event::Lent(user, lending_asset, lending_amount));
            Ok(().into())
        }

        /// Borrow token
        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::borrow())]
        pub fn borrow(
            origin: OriginFor<T>,
            collateral_asset: AssetIdOf<T>,
            borrowing_asset: AssetIdOf<T>,
            borrowing_amount: Balance,
            loan_to_value: Balance,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            ensure!(
                collateral_asset != borrowing_asset,
                Error::<T>::SameCollateralAndBorrowingAssets
            );

            // Check if borrowing amount is minimum 10$
            let borrow_asset_price = Self::get_price(borrowing_asset);
            let borrowing_amount_in_dollars: u128 = (FixedWrapper::from(borrowing_amount)
                * FixedWrapper::from(borrow_asset_price))
            .try_into_balance()
            .unwrap_or(0);
            ensure!(
                borrowing_amount_in_dollars >= balance!(10),
                Error::<T>::InvalidBorrowingAmount
            );

            let mut borrow_pool_info =
                <PoolData<T>>::get(borrowing_asset).ok_or(Error::<T>::PoolDoesNotExist)?;
            ensure!(!borrow_pool_info.is_removed, Error::<T>::PoolIsRemoved);
            ensure!(
                borrowing_amount <= borrow_pool_info.total_liquidity,
                Error::<T>::NoLiquidityForBorrowingAsset
            );
            ensure!(
                loan_to_value != balance!(0) && loan_to_value <= borrow_pool_info.loan_to_value,
                Error::<T>::InvalidLoanToValue
            );

            let mut collateral_pool_info =
                <PoolData<T>>::get(collateral_asset).ok_or(Error::<T>::PoolDoesNotExist)?;
            ensure!(!collateral_pool_info.is_removed, Error::<T>::PoolIsRemoved);
            let mut user_lending_info = <UserLendingInfo<T>>::get(collateral_asset, user.clone())
                .ok_or(Error::<T>::NothingLent)?;
            let collateral_asset_price = Self::get_price(collateral_asset);

            // Calculate required collateral asset in dollars
            let coll_amount_in_dollars = ((FixedWrapper::from(borrowing_amount)
                / FixedWrapper::from(loan_to_value))
                * FixedWrapper::from(borrow_asset_price))
            .try_into_balance()
            .unwrap_or(0);

            // Calculate collateral amount in tokens of chosen asset
            let collateral_amount = (FixedWrapper::from(coll_amount_in_dollars)
                / FixedWrapper::from(collateral_asset_price))
            .try_into_balance()
            .unwrap_or(0);

            let mut borrow_info =
                <UserBorrowingInfo<T>>::get(borrowing_asset, user.clone()).unwrap_or_default();

            if collateral_asset == KUSD.into() {
                let factor = <CollateralFactor<T>>::get();

                // To get total collateral for a user
                let total_existing_collateral =
                    <UserTotalCollateral<T>>::get(user.clone(), collateral_asset)
                        .unwrap_or(Zero::zero());

                // Calculate the maximum allowed collateral for KUSD
                let max_allowed_collateral = Self::calculate_max_allowed_collateral(
                    user_lending_info
                        .lending_amount
                        .saturating_add(total_existing_collateral),
                    factor,
                )?;

                let new_total_collateral =
                    total_existing_collateral.saturating_add(collateral_amount);

                ensure!(
                    new_total_collateral <= max_allowed_collateral,
                    Error::<T>::InvalidCollateralAmount
                );
            }

            ensure!(
                collateral_amount <= user_lending_info.lending_amount,
                Error::<T>::InvalidCollateralAmount
            );

            // Add borrowing amount, collateral amount and interest to user if exists, otherwise create new user
            if let Some(mut user_info) = borrow_info.get_mut(&collateral_asset) {
                let block_number = <frame_system::Pallet<T>>::block_number();
                let calculated_interest = Self::calculate_borrowing_interest_and_reward(
                    user_info,
                    &borrow_pool_info,
                    block_number,
                );
                user_info.borrowing_interest += calculated_interest.0;
                user_info.borrowing_rewards += calculated_interest.1;
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
            <UserBorrowingInfo<T>>::insert(borrowing_asset, user.clone(), borrow_info);

            // Update user's lending info according to given collateral
            let block_number = <frame_system::Pallet<T>>::block_number();
            let interests = Self::calculate_lending_earnings(
                &user_lending_info,
                &collateral_pool_info,
                block_number,
            );
            user_lending_info.lending_interest += interests.0 + interests.1;
            user_lending_info.lending_amount = user_lending_info
                .lending_amount
                .saturating_sub(collateral_amount);
            user_lending_info.last_lending_block = <frame_system::Pallet<T>>::block_number();
            <UserLendingInfo<T>>::insert(collateral_asset, user.clone(), user_lending_info);

            // Update collateral and borrowing assets pools
            borrow_pool_info.total_liquidity = borrow_pool_info
                .total_liquidity
                .saturating_sub(borrowing_amount);
            borrow_pool_info.total_borrowed += borrowing_amount;
            collateral_pool_info.total_liquidity = collateral_pool_info
                .total_liquidity
                .saturating_sub(collateral_amount);
            collateral_pool_info.total_collateral += collateral_amount;

            <PoolData<T>>::insert(collateral_asset, collateral_pool_info);
            <PoolData<T>>::insert(borrowing_asset, borrow_pool_info);

            // Update the total collateral
            Self::update_total_collateral(&user, &collateral_asset, collateral_amount)?;

            // Transfer borrowing amount to user
            T::AssetManager::transfer_from(
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
        #[pallet::weight(<T as Config>::WeightInfo::get_rewards())]
        pub fn get_rewards(
            origin: OriginFor<T>,
            asset_id: AssetIdOf<T>,
            is_lending: bool,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            let block_number = <frame_system::Pallet<T>>::block_number();
            let pool_info = PoolData::<T>::get(asset_id).ok_or(Error::<T>::PoolDoesNotExist)?;

            // Check if user has lent or borrowed rewards
            if is_lending {
                let mut lend_user_info = <UserLendingInfo<T>>::get(asset_id, user.clone())
                    .ok_or(Error::<T>::NothingLent)?;
                let interests =
                    Self::calculate_lending_earnings(&lend_user_info, &pool_info, block_number);
                lend_user_info.lending_interest += interests.0 + interests.1;
                lend_user_info.last_lending_block = <frame_system::Pallet<T>>::block_number();

                ensure!(
                    lend_user_info.lending_interest > 0,
                    Error::<T>::NoRewardsToClaim
                );

                T::AssetManager::transfer_from(
                    &APOLLO_ASSET_ID.into(),
                    &Self::account_id(),
                    &user,
                    lend_user_info.lending_interest,
                )
                .map_err(|_| Error::<T>::UnableToTransferRewards)?;

                let lending_rewards = lend_user_info.lending_interest;
                lend_user_info.lending_interest = 0;
                <UserLendingInfo<T>>::insert(asset_id, user.clone(), &lend_user_info);

                Self::deposit_event(Event::ClaimedLendingRewards(
                    user,
                    asset_id,
                    lending_rewards,
                ));
            } else {
                let mut user_infos = <UserBorrowingInfo<T>>::get(asset_id, user.clone())
                    .ok_or(Error::<T>::NothingBorrowed)?;
                let block_number = <frame_system::Pallet<T>>::block_number();

                let mut borrowing_rewards = 0;
                for (_, mut user_info) in user_infos.iter_mut() {
                    let interest_and_reward = Self::calculate_borrowing_interest_and_reward(
                        user_info,
                        &pool_info,
                        block_number,
                    );
                    user_info.borrowing_interest += interest_and_reward.0;
                    user_info.borrowing_rewards += interest_and_reward.1;
                    user_info.last_borrowing_block = block_number;
                    borrowing_rewards += user_info.borrowing_rewards;
                    user_info.borrowing_rewards = 0;
                }

                ensure!(borrowing_rewards > 0, Error::<T>::NoRewardsToClaim);

                T::AssetManager::transfer_from(
                    &APOLLO_ASSET_ID.into(),
                    &Self::account_id(),
                    &user,
                    borrowing_rewards,
                )
                .map_err(|_| Error::<T>::UnableToTransferRewards)?;

                <UserBorrowingInfo<T>>::insert(asset_id, user.clone(), &user_infos);

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
        #[pallet::weight(<T as Config>::WeightInfo::withdraw())]
        pub fn withdraw(
            origin: OriginFor<T>,
            withdrawn_asset: AssetIdOf<T>,
            withdrawn_amount: Balance,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            let mut pool_info =
                <PoolData<T>>::get(withdrawn_asset).ok_or(Error::<T>::PoolDoesNotExist)?;
            let mut user_info = <UserLendingInfo<T>>::get(withdrawn_asset, user.clone())
                .ok_or(Error::<T>::NothingLent)?;

            ensure!(
                withdrawn_amount <= user_info.lending_amount,
                Error::<T>::LendingAmountExceeded
            );
            ensure!(
                withdrawn_amount < pool_info.total_liquidity,
                Error::<T>::CanNotTransferLendingAmount
            );

            // Transfer lending amount
            T::AssetManager::transfer_from(
                &withdrawn_asset,
                &Self::account_id(),
                &user,
                withdrawn_amount,
            )
            .map_err(|_| Error::<T>::CanNotTransferLendingAmount)?;

            let previous_lending_amount = user_info.lending_amount;

            let block_number = <frame_system::Pallet<T>>::block_number();
            let interests: (u128, u128) =
                Self::calculate_lending_earnings(&user_info, &pool_info, block_number);
            user_info.lending_amount = user_info.lending_amount.saturating_sub(withdrawn_amount);
            user_info.lending_interest += interests.0 + interests.1;
            user_info.last_lending_block = block_number;

            // Check if lending amount is less than user's lending amount
            if withdrawn_amount < previous_lending_amount {
                <UserLendingInfo<T>>::insert(withdrawn_asset, user.clone(), user_info);
            } else {
                // Transfer lending interest when user withdraws whole lending amount
                T::AssetManager::transfer_from(
                    &APOLLO_ASSET_ID.into(),
                    &Self::account_id(),
                    &user,
                    user_info.lending_interest,
                )
                .map_err(|_| Error::<T>::CanNotTransferLendingInterest)?;
                <UserLendingInfo<T>>::remove(withdrawn_asset, user.clone());
            }

            pool_info.total_liquidity = pool_info.total_liquidity.saturating_sub(withdrawn_amount);
            <PoolData<T>>::insert(withdrawn_asset, pool_info);

            Self::deposit_event(Event::Withdrawn(user, withdrawn_asset, withdrawn_amount));
            Ok(().into())
        }

        /// Repay
        #[pallet::call_index(5)]
        #[pallet::weight(<T as Config>::WeightInfo::repay())]
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
            let mut borrow_user_info = <UserBorrowingInfo<T>>::get(borrowing_asset, user.clone())
                .ok_or(Error::<T>::NothingBorrowed)?;
            let mut user_info = borrow_user_info
                .get(&collateral_asset)
                .cloned()
                .ok_or(Error::<T>::NonexistentBorrowingPosition)?;

            let block_number = <frame_system::Pallet<T>>::block_number();
            let interest_and_reward = Self::calculate_borrowing_interest_and_reward(
                &user_info,
                &borrow_pool_info,
                block_number,
            );
            user_info.borrowing_interest += interest_and_reward.0;
            user_info.borrowing_rewards += interest_and_reward.1;
            user_info.last_borrowing_block = block_number;

            // Total repaid
            let mut total_repaid: Balance = amount_to_repay;

            if amount_to_repay <= user_info.borrowing_interest {
                // If user is repaying only part or whole interest
                user_info.borrowing_interest =
                    user_info.borrowing_interest.saturating_sub(amount_to_repay);
                borrow_user_info.insert(collateral_asset, user_info);

                T::AssetManager::transfer_from(
                    &borrowing_asset,
                    &user,
                    &Self::account_id(),
                    amount_to_repay,
                )
                .map_err(|_| Error::<T>::CanNotTransferAmountToRepay)?;

                <UserBorrowingInfo<T>>::insert(borrowing_asset, user.clone(), &borrow_user_info);

                Self::distribute_protocol_interest(
                    borrowing_asset,
                    amount_to_repay,
                    borrowing_asset,
                )?;
            } else if amount_to_repay > user_info.borrowing_interest
                && amount_to_repay < user_info.borrowing_interest + user_info.borrowing_amount
            {
                // If user is repaying whole interest plus part of the borrowed amount
                let repaid_amount = user_info.borrowing_interest;
                let remaining_amount = amount_to_repay.saturating_sub(user_info.borrowing_interest);
                user_info.borrowing_amount =
                    user_info.borrowing_amount.saturating_sub(remaining_amount);
                user_info.borrowing_interest = 0;
                borrow_pool_info.total_borrowed = borrow_pool_info
                    .total_borrowed
                    .saturating_sub(remaining_amount);
                borrow_pool_info.total_liquidity += remaining_amount;
                <PoolData<T>>::insert(borrowing_asset, borrow_pool_info);

                T::AssetManager::transfer_from(
                    &borrowing_asset,
                    &user,
                    &Self::account_id(),
                    amount_to_repay,
                )
                .map_err(|_| Error::<T>::CanNotTransferAmountToRepay)?;

                borrow_user_info.insert(collateral_asset, user_info);
                <UserBorrowingInfo<T>>::insert(borrowing_asset, user.clone(), &borrow_user_info);

                Self::distribute_protocol_interest(
                    borrowing_asset,
                    repaid_amount,
                    borrowing_asset,
                )?;
            } else if amount_to_repay >= user_info.borrowing_interest + user_info.borrowing_amount {
                // If user is repaying the whole position
                let total_borrowed_amount =
                    user_info.borrowing_interest + user_info.borrowing_amount;

                // Update pools
                borrow_pool_info.total_borrowed = borrow_pool_info
                    .total_borrowed
                    .saturating_sub(user_info.borrowing_amount);
                collateral_pool_info.total_collateral = collateral_pool_info
                    .total_collateral
                    .saturating_sub(user_info.collateral_amount);
                borrow_pool_info.total_liquidity += user_info.borrowing_amount;

                <PoolData<T>>::insert(collateral_asset, collateral_pool_info);
                <PoolData<T>>::insert(borrowing_asset, borrow_pool_info);

                // Update the total collateral
                Self::decrease_total_collateral(
                    &user,
                    &collateral_asset,
                    user_info.collateral_amount,
                )?;

                // Transfer borrowing amount and borrowing interest to pallet
                T::AssetManager::transfer_from(
                    &borrowing_asset,
                    &user,
                    &Self::account_id(),
                    total_borrowed_amount,
                )
                .map_err(|_| Error::<T>::CanNotTransferBorrowingAmount)?;

                // Transfer collateral to user
                T::AssetManager::transfer_from(
                    &collateral_asset,
                    &Self::account_id(),
                    &user,
                    user_info.collateral_amount,
                )
                .map_err(|_| Error::<T>::UnableToTransferCollateral)?;

                // Transfer borrowing rewards to user
                T::AssetManager::transfer_from(
                    &APOLLO_ASSET_ID.into(),
                    &Self::account_id(),
                    &user,
                    user_info.borrowing_rewards,
                )
                .map_err(|_| Error::<T>::CanNotTransferBorrowingRewards)?;

                borrow_user_info.remove(&collateral_asset);
                if borrow_user_info.is_empty() {
                    <UserBorrowingInfo<T>>::remove(borrowing_asset, user.clone());
                } else {
                    <UserBorrowingInfo<T>>::insert(borrowing_asset, user.clone(), borrow_user_info);
                }

                Self::distribute_protocol_interest(
                    borrowing_asset,
                    user_info.borrowing_interest,
                    borrowing_asset,
                )?;

                // Updating total repaid
                total_repaid = total_borrowed_amount;
            }

            Self::deposit_event(Event::Repaid(user, borrowing_asset, total_repaid));
            Ok(().into())
        }

        /// Change rewards amount
        #[pallet::call_index(6)]
        #[pallet::weight(<T as Config>::WeightInfo::change_rewards_amount())]
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

        /// Change rewards per block
        #[pallet::call_index(7)]
        #[pallet::weight(<T as Config>::WeightInfo::change_rewards_per_block())]
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

        /// Liquidate
        #[pallet::call_index(8)]
        #[pallet::weight(<T as Config>::WeightInfo::liquidate())]
        pub fn liquidate(
            _origin: OriginFor<T>,
            user: AccountIdOf<T>,
            asset_id: AssetIdOf<T>,
        ) -> DispatchResult {
            let user_infos =
                UserBorrowingInfo::<T>::get(asset_id, user.clone()).unwrap_or_default();
            ensure!(!user_infos.is_empty(), Error::<T>::InvalidLiquidation);

            if !Self::check_liquidation(&user_infos, asset_id) {
                return Err(Error::<T>::InvalidLiquidation.into());
            }

            // Calculate total borrow and total collateral in dollars
            let mut total_borrowed: Balance = 0;

            // Distributing and calculating total borrowed
            for (collateral_asset, user_info) in user_infos.iter() {
                // Calculate collateral in dollars
                let collateral_asset_price = Self::get_price(*collateral_asset);
                let collateral_amount_in_dollars =
                    (FixedWrapper::from(user_info.collateral_amount)
                        * FixedWrapper::from(collateral_asset_price))
                    .try_into_balance()
                    .unwrap_or(0);

                // Calculate user's borrowed amount
                let borrow_asset_price = Self::get_price(asset_id);
                let user_borrowed_in_dollars = (FixedWrapper::from(user_info.borrowing_amount)
                    * FixedWrapper::from(borrow_asset_price))
                .try_into_balance()
                .unwrap_or(0);

                // Calculating amount to distribute in dollars and converting to collateral token amount
                let amount_to_distribute_in_dollars =
                    collateral_amount_in_dollars.saturating_sub(user_borrowed_in_dollars);
                let amount_to_distribute = (FixedWrapper::from(amount_to_distribute_in_dollars)
                    / FixedWrapper::from(collateral_asset_price))
                .try_into_balance()
                .unwrap_or(0);

                // Distributing the amount calculated as sufficit of collateral asset in dollars over borrowed amount in dollars
                let _ = Self::distribute_protocol_interest(
                    *collateral_asset,
                    amount_to_distribute,
                    asset_id,
                );

                // Amount to exchange
                let amount_to_exchange = (FixedWrapper::from(user_borrowed_in_dollars)
                    / FixedWrapper::from(collateral_asset_price))
                .try_into_balance()
                .unwrap_or(0);

                // Exchange collateral asset into borrowed asset  on Pallet
                T::LiquidityProxyPallet::exchange(
                    DEXId::Polkaswap.into(),
                    &Self::account_id(),
                    &Self::account_id(),
                    collateral_asset,
                    &asset_id,
                    SwapAmount::with_desired_input(amount_to_exchange, Balance::zero()),
                    LiquiditySourceFilter::empty(DEXId::Polkaswap.into()),
                )?;

                // Updating collateral pool total_collateral amount and total_liquidity
                let mut collateral_pool_info =
                    PoolData::<T>::get(*collateral_asset).unwrap_or_default();
                collateral_pool_info.total_collateral = collateral_pool_info
                    .total_collateral
                    .saturating_sub(user_info.collateral_amount);

                // Update the total collateral
                Self::decrease_total_collateral(
                    &user,
                    collateral_asset,
                    user_info.collateral_amount,
                )?;

                <PoolData<T>>::insert(*collateral_asset, collateral_pool_info);
                // Add user's borrowed amount tied with this asset to total_borrowed in given asset
                total_borrowed += user_info.borrowing_amount;
            }

            // Updating total_borrowed and total_liquidity for given asset
            let mut borrow_pool_info = PoolData::<T>::get(asset_id).unwrap_or_default();
            borrow_pool_info.total_borrowed = borrow_pool_info
                .total_borrowed
                .saturating_sub(total_borrowed);
            borrow_pool_info.total_liquidity += total_borrowed;

            <PoolData<T>>::insert(asset_id, borrow_pool_info);
            <UserBorrowingInfo<T>>::remove(asset_id, user.clone());

            Self::deposit_event(Event::Liquidated(user, asset_id));

            Ok(())
        }

        /// Remove pool
        #[pallet::call_index(9)]
        #[pallet::weight(<T as Config>::WeightInfo::remove_pool())]
        pub fn remove_pool(
            origin: OriginFor<T>,
            asset_id_to_remove: AssetIdOf<T>,
        ) -> DispatchResult {
            let user = ensure_signed(origin)?;

            if user != AuthorityAccount::<T>::get() {
                return Err(Error::<T>::Unauthorized.into());
            }

            let mut pool_info =
                PoolData::<T>::get(asset_id_to_remove).ok_or(Error::<T>::PoolDoesNotExist)?;
            pool_info.basic_lending_rate = 0;
            pool_info.borrowing_rewards_rate = 0;
            pool_info.is_removed = true;
            <PoolData<T>>::insert(asset_id_to_remove, pool_info);

            // Recalculate basic lending rate and borrowing rewards rate
            let num_of_pools = <PoolData<T>>::iter()
                .filter(|(_, pool_info)| !pool_info.is_removed)
                .count() as u32;

            let basic_lending_rate = (FixedWrapper::from(LendingRewardsPerBlock::<T>::get())
                / FixedWrapper::from(balance!(num_of_pools)))
            .try_into_balance()
            .unwrap_or(0);
            let borrowing_rewards_rate = (FixedWrapper::from(BorrowingRewardsPerBlock::<T>::get())
                / FixedWrapper::from(balance!(num_of_pools)))
            .try_into_balance()
            .unwrap_or(0);

            for (asset_id, mut pool_info) in <PoolData<T>>::iter() {
                if pool_info.is_removed {
                    continue;
                }
                pool_info.basic_lending_rate = basic_lending_rate;
                pool_info.borrowing_rewards_rate = borrowing_rewards_rate;
                <PoolData<T>>::insert(asset_id, pool_info);
            }

            Self::deposit_event(Event::PoolRemoved(user, asset_id_to_remove));
            Ok(())
        }

        /// Edit pool info
        #[pallet::call_index(10)]
        #[pallet::weight(<T as Config>::WeightInfo::edit_pool_info())]
        pub fn edit_pool_info(
            origin: OriginFor<T>,
            asset_id: AssetIdOf<T>,
            new_loan_to_value: Balance,
            new_liquidation_threshold: Balance,
            new_optimal_utilization_rate: Balance,
            new_base_rate: Balance,
            new_slope_rate_1: Balance,
            new_slope_rate_2: Balance,
            new_reserve_factor: Balance,
            new_tl: Balance,
            new_tb: Balance,
            new_tc: Balance,
        ) -> DispatchResult {
            let user = ensure_signed(origin)?;

            if user != AuthorityAccount::<T>::get() {
                return Err(Error::<T>::Unauthorized.into());
            }

            // Check parameters
            if new_loan_to_value > balance!(1)
                || new_liquidation_threshold > balance!(1)
                || new_optimal_utilization_rate > balance!(1)
                || new_reserve_factor > balance!(1)
            {
                return Err(Error::<T>::InvalidPoolParameters.into());
            }

            let mut pool_info = PoolData::<T>::get(asset_id).ok_or(Error::<T>::PoolDoesNotExist)?;

            // Check if pool is removed
            ensure!(!pool_info.is_removed, Error::<T>::PoolIsRemoved);

            // Update pool info
            pool_info.loan_to_value = new_loan_to_value;
            pool_info.liquidation_threshold = new_liquidation_threshold;
            pool_info.optimal_utilization_rate = new_optimal_utilization_rate;
            pool_info.base_rate = new_base_rate;
            pool_info.slope_rate_1 = new_slope_rate_1;
            pool_info.slope_rate_2 = new_slope_rate_2;
            pool_info.reserve_factor = new_reserve_factor;
            pool_info.total_liquidity = new_tl;
            pool_info.total_borrowed = new_tb;
            pool_info.total_collateral = new_tc;

            // Saving new pool info
            <PoolData<T>>::insert(asset_id, pool_info);

            Self::deposit_event(Event::PoolInfoEdited(user, asset_id));
            Ok(())
        }

        /// Add more collateral to borrowing position
        #[pallet::call_index(11)]
        #[pallet::weight(<T as Config>::WeightInfo::add_collateral())]
        pub fn add_collateral(
            origin: OriginFor<T>,
            collateral_asset: AssetIdOf<T>,
            collateral_amount: Balance,
            borrowing_asset: AssetIdOf<T>,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            ensure!(
                collateral_asset != borrowing_asset,
                Error::<T>::SameCollateralAndBorrowingAssets
            );

            let borrow_pool_info =
                <PoolData<T>>::get(borrowing_asset).ok_or(Error::<T>::PoolDoesNotExist)?;
            ensure!(!borrow_pool_info.is_removed, Error::<T>::PoolIsRemoved);

            let mut collateral_pool_info =
                <PoolData<T>>::get(collateral_asset).ok_or(Error::<T>::PoolDoesNotExist)?;
            ensure!(!collateral_pool_info.is_removed, Error::<T>::PoolIsRemoved);

            let mut user_lending_info = <UserLendingInfo<T>>::get(collateral_asset, user.clone())
                .ok_or(Error::<T>::NothingLent)?;

            ensure!(
                collateral_amount <= user_lending_info.lending_amount,
                Error::<T>::InvalidCollateralAmount
            );

            let mut borrow_info =
                <UserBorrowingInfo<T>>::get(borrowing_asset, user.clone()).unwrap_or_default();

            if collateral_asset == KUSD.into() {
                let factor = <CollateralFactor<T>>::get();
                // To get total collateral for a user
                let total_existing_collateral =
                    <UserTotalCollateral<T>>::get(user.clone(), collateral_asset)
                        .unwrap_or(Zero::zero());

                // Calculate the maximum allowed collateral for KUSD
                let max_allowed_collateral = Self::calculate_max_allowed_collateral(
                    user_lending_info
                        .lending_amount
                        .saturating_add(total_existing_collateral),
                    factor,
                )?;

                let new_total_collateral =
                    total_existing_collateral.saturating_add(collateral_amount);

                ensure!(
                    new_total_collateral <= max_allowed_collateral,
                    Error::<T>::InvalidCollateralAmount
                );
            }

            // Add borrowing amount, collateral amount and interest to user if exists, otherwise return error
            if let Some(mut user_info) = borrow_info.get_mut(&collateral_asset) {
                let block_number = <frame_system::Pallet<T>>::block_number();
                let calculated_interest = Self::calculate_borrowing_interest_and_reward(
                    user_info,
                    &borrow_pool_info,
                    block_number,
                );
                user_info.borrowing_interest += calculated_interest.0;
                user_info.borrowing_rewards += calculated_interest.1;
                user_info.collateral_amount += collateral_amount;
                user_info.last_borrowing_block = block_number;
            } else {
                return Err(Error::<T>::NonexistentBorrowingPosition.into());
            }
            <UserBorrowingInfo<T>>::insert(borrowing_asset, user.clone(), borrow_info);

            // Update user's lending info according to given collateral
            let block_number = <frame_system::Pallet<T>>::block_number();
            let interests = Self::calculate_lending_earnings(
                &user_lending_info,
                &collateral_pool_info,
                block_number,
            );
            user_lending_info.lending_interest += interests.0 + interests.1;
            user_lending_info.lending_amount = user_lending_info
                .lending_amount
                .saturating_sub(collateral_amount);
            user_lending_info.last_lending_block = <frame_system::Pallet<T>>::block_number();
            <UserLendingInfo<T>>::insert(collateral_asset, user.clone(), user_lending_info);

            // Update collateral asset pool
            collateral_pool_info.total_liquidity = collateral_pool_info
                .total_liquidity
                .saturating_sub(collateral_amount);
            collateral_pool_info.total_collateral += collateral_amount;

            // Update the total collateral
            Self::update_total_collateral(&user, &collateral_asset, collateral_amount)?;

            <PoolData<T>>::insert(collateral_asset, collateral_pool_info);

            Self::deposit_event(Event::CollateralAdded(
                user,
                collateral_asset,
                collateral_amount,
                borrowing_asset,
            ));

            Ok(().into())
        }

        /// Change rewards amount
        #[pallet::call_index(12)]
        #[pallet::weight(<T as Config>::WeightInfo::change_collateral_factor())]
        pub fn change_collateral_factor(
            origin: OriginFor<T>,
            amount: Balance,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            if user != AuthorityAccount::<T>::get() {
                return Err(Error::<T>::Unauthorized.into());
            }

            <CollateralFactor<T>>::put(amount);

            Self::deposit_event(Event::ChangedCollateralFactorAmount(user, amount));
            Ok(().into())
        }
    }

    /// Validate unsigned call to this pallet.
    #[pallet::validate_unsigned]
    impl<T: Config> ValidateUnsigned for Pallet<T> {
        type Call = Call<T>;

        /// It is allowed to call only liquidate() and only if it fulfills conditions.
        fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
            match call {
                Call::liquidate { user, asset_id } => {
                    let user_infos =
                        UserBorrowingInfo::<T>::get(asset_id, user.clone()).unwrap_or_default();
                    if Self::check_liquidation(&user_infos, *asset_id) {
                        ValidTransaction::with_tag_prefix("Apollo::liquidate")
                            .priority(T::UnsignedPriority::get())
                            .longevity(T::UnsignedLongevity::get())
                            .and_provides((user, asset_id))
                            .propagate(true)
                            .build()
                    } else {
                        InvalidTransaction::Call.into()
                    }
                }
                _ => {
                    warn!("Unknown unsigned call {:?}", call);
                    InvalidTransaction::Call.into()
                }
            }
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(now: T::BlockNumber) -> Weight {
            let distribution_rewards = Self::update_interests(now);
            let rates = Self::update_rates(now);

            <LendingRewards<T>>::put(
                <LendingRewards<T>>::get() - <LendingRewardsPerBlock<T>>::get(),
            );
            <BorrowingRewards<T>>::put(
                <BorrowingRewards<T>>::get() - <BorrowingRewardsPerBlock<T>>::get(),
            );

            distribution_rewards.saturating_add(rates).saturating_add(
                T::DbWeight::get()
                    .reads(4)
                    .saturating_add(T::DbWeight::get().writes(2)),
            )
        }

        /// Off-chain worker procedure - calls liquidations
        fn offchain_worker(block_number: T::BlockNumber) {
            debug!(
                "Entering off-chain worker, block number is {:?}",
                block_number
            );

            for (asset_id, user, user_infos) in UserBorrowingInfo::<T>::iter() {
                // Check liquidation
                if Self::check_liquidation(&user_infos, asset_id) {
                    // Liquidate
                    debug!("Liquidation of user {:?}", user);
                    let call = Call::<T>::liquidate {
                        user: user.clone(),
                        asset_id,
                    };
                    if let Err(err) =
                        SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into())
                    {
                        warn!(
                            "Failed in offchain_worker send liquidate(user: {:?}): {:?}",
                            user, err
                        );
                    }
                }
            }
        }
    }

    impl<T: Config> Pallet<T> {
        /// The account ID of pallet
        fn account_id() -> T::AccountId {
            PALLET_ID.into_account_truncating()
        }

        pub fn get_price(asset_id: AssetIdOf<T>) -> Balance {
            // Get average price from PriceTools pallet
            let buy_price =
                T::PriceTools::get_average_price(&asset_id, &DAI.into(), PriceVariant::Buy)
                    .unwrap_or_default();

            let sell_price =
                T::PriceTools::get_average_price(&asset_id, &DAI.into(), PriceVariant::Sell)
                    .unwrap_or_default();

            // Average price in dollars
            (FixedWrapper::from(buy_price + sell_price) / FixedWrapper::from(balance!(2)))
                .try_into_balance()
                .unwrap_or(0)
        }

        pub fn check_liquidation(
            user_infos: &BTreeMap<AssetIdOf<T>, BorrowingPosition<BlockNumberFor<T>>>,
            borrowing_asset: AssetIdOf<T>,
        ) -> bool {
            let mut sum_of_thresholds: Balance = 0;
            let mut total_borrowed: Balance = 0;

            for (collateral_asset, user_info) in user_infos.iter() {
                let collateral_pool_info = PoolData::<T>::get(collateral_asset).unwrap_or_default();
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
            }

            let borrowing_asset_price = Self::get_price(borrowing_asset);
            let total_borrowed_in_dollars: u128 = (FixedWrapper::from(total_borrowed)
                * FixedWrapper::from(borrowing_asset_price))
            .try_into_balance()
            .unwrap_or(0);

            let health_factor = (FixedWrapper::from(sum_of_thresholds)
                / FixedWrapper::from(total_borrowed_in_dollars))
            .try_into_balance()
            .unwrap_or(0);

            health_factor < balance!(1)
        }

        pub fn calculate_lending_earnings(
            user_info: &LendingPosition<BlockNumberFor<T>>,
            pool_info: &PoolInfo,
            block_number: BlockNumberFor<T>,
        ) -> (Balance, Balance) {
            let total_lending_blocks: u128 =
                (block_number - user_info.last_lending_block).unique_saturated_into();

            let share_in_pool = FixedWrapper::from(user_info.lending_amount)
                / FixedWrapper::from(pool_info.total_liquidity);

            // Rewards from initial APOLLO distribution
            let basic_reward_per_block =
                FixedWrapper::from(pool_info.basic_lending_rate) * share_in_pool.clone();

            // Rewards from profit made through repayments and liquidations
            let profit_reward_per_block =
                FixedWrapper::from(pool_info.profit_lending_rate) * share_in_pool;

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

        pub fn calculate_borrowing_interest_and_reward(
            user_info: &BorrowingPosition<BlockNumberFor<T>>,
            pool_info: &PoolInfo,
            block_number: BlockNumberFor<T>,
        ) -> (Balance, Balance) {
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

        /// Increase total collateral amount for a user and asset
        fn update_total_collateral(
            user: &AccountIdOf<T>,
            collateral_asset: &AssetIdOf<T>,
            amount_to_add: Balance,
        ) -> DispatchResult {
            <UserTotalCollateral<T>>::mutate(user, collateral_asset, |current_collateral| {
                // If no existing collateral, start with the new amount
                // Otherwise, add the new amount
                *current_collateral = Some(
                    current_collateral
                        .unwrap_or(Zero::zero())
                        .saturating_add(amount_to_add),
                )
            });

            Ok(())
        }

        /// Decrease total collateral amount for a user and asset
        fn decrease_total_collateral(
            user: &AccountIdOf<T>,
            collateral_asset: &AssetIdOf<T>,
            amount_to_remove: Balance,
        ) -> DispatchResult {
            <UserTotalCollateral<T>>::mutate(user, collateral_asset, |current_collateral| {
                if let Some(current) = *current_collateral {
                    let new_amount = current.saturating_sub(amount_to_remove);

                    // Remove the entry if it reaches zero, otherwise update
                    if new_amount == Zero::zero() {
                        *current_collateral = None;
                    } else {
                        *current_collateral = Some(new_amount);
                    }
                }
            });

            Ok(())
        }

        pub fn distribute_protocol_interest(
            asset_id: AssetIdOf<T>,
            amount: Balance,
            borrowing_asset_id: AssetIdOf<T>,
        ) -> DispatchResultWithPostInfo {
            let mut pool_info =
                PoolData::<T>::get(borrowing_asset_id).ok_or(Error::<T>::PoolDoesNotExist)?;
            let caller = Self::account_id();

            // Calculate rewards and reserves amounts based on Reserve Factor
            let reserves_amount = (FixedWrapper::from(pool_info.reserve_factor)
                * FixedWrapper::from(amount))
            .try_into_balance()
            .unwrap_or(0);
            let rewards_amount = amount.saturating_sub(reserves_amount);

            let outcome = T::LiquidityProxyPallet::exchange(
                DEXId::Polkaswap.into(),
                &caller,
                &caller,
                &asset_id,
                &APOLLO_ASSET_ID.into(),
                SwapAmount::with_desired_input(rewards_amount, Balance::zero()),
                LiquiditySourceFilter::empty(DEXId::Polkaswap.into()),
            )?;

            let buyback_amount = outcome.amount;

            pool_info.rewards += buyback_amount;

            <PoolData<T>>::insert(borrowing_asset_id, pool_info);

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

            // Transfer amount to developer fund
            T::AssetManager::transfer_from(
                &asset_id,
                &Self::account_id(),
                &AuthorityAccount::<T>::get(),
                developer_amount,
            )
            .map_err(|_| Error::<T>::CanNotTransferAmountToDevelopers)?;

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
            )?;

            T::AssetManager::burn(
                RawOrigin::Signed(caller).into(),
                CERES_ASSET_ID.into(),
                outcome.amount,
            )?;

            Ok(().into())
        }

        fn calculate_max_allowed_collateral(
            lending_amount: Balance,
            factor: Balance,
        ) -> Result<Balance, DispatchError> {
            Ok(
                (FixedWrapper::from(lending_amount) * FixedWrapper::from(factor))
                    .try_into_balance()
                    .unwrap_or(0),
            )
        }

        fn update_interests(block_number: BlockNumberFor<T>) -> Weight {
            let mut counter: u64 = 0;
            let pool_index = block_number % T::BLOCKS_PER_FIFTEEN_MINUTES;
            let num_of_pools = <PoolsByBlock<T>>::iter().count() as u32;
            if pool_index >= num_of_pools.into() {
                return T::DbWeight::get().reads(counter);
            }
            let pool_asset = <PoolsByBlock<T>>::get(pool_index).unwrap_or_default();
            let mut pool_info = <PoolData<T>>::get(pool_asset).unwrap_or_default();

            // Update lending interests
            let mut rewards: Balance = 0;
            for (account_id, mut user_info) in UserLendingInfo::<T>::iter_prefix(pool_asset) {
                let user_interests =
                    Self::calculate_lending_earnings(&user_info, &pool_info, block_number);
                user_info.lending_interest += user_interests.0 + user_interests.1;
                user_info.last_lending_block = block_number;
                rewards += user_interests.1;

                <UserLendingInfo<T>>::insert(pool_asset, account_id.clone(), user_info);
                counter += 1;
            }

            // Update pool rewards
            pool_info.rewards = pool_info.rewards.saturating_sub(rewards);
            <PoolData<T>>::insert(pool_asset, &pool_info);
            counter += 1;

            // Update borrowing interests
            for (account_id, mut user_infos) in UserBorrowingInfo::<T>::iter_prefix(pool_asset) {
                for (_, mut user_info) in user_infos.iter_mut() {
                    let user_interests = Self::calculate_borrowing_interest_and_reward(
                        user_info,
                        &pool_info,
                        block_number,
                    );
                    user_info.borrowing_interest += user_interests.0;
                    user_info.borrowing_rewards += user_interests.1;
                    user_info.last_borrowing_block = block_number;
                }
                <UserBorrowingInfo<T>>::insert(pool_asset, account_id.clone(), user_infos.clone());
                counter += 1;
            }

            T::DbWeight::get()
                .reads(counter + 4)
                .saturating_add(T::DbWeight::get().writes(counter + 4))
        }

        fn update_rates(_current_block: T::BlockNumber) -> Weight {
            let mut counter: u64 = 0;

            for (asset_id, mut pool_info) in PoolData::<T>::iter() {
                let utilization_rate = (FixedWrapper::from(pool_info.total_borrowed)
                    / (FixedWrapper::from(pool_info.total_borrowed)
                        + FixedWrapper::from(pool_info.total_liquidity)))
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
                    pool_info.borrowing_rate = ((FixedWrapper::from(pool_info.base_rate)
                        + FixedWrapper::from(pool_info.slope_rate_1)
                        + ((FixedWrapper::from(utilization_rate)
                            - FixedWrapper::from(pool_info.optimal_utilization_rate))
                            / (FixedWrapper::from(balance!(1))
                                - FixedWrapper::from(pool_info.optimal_utilization_rate)))
                            * FixedWrapper::from(pool_info.slope_rate_2))
                        / FixedWrapper::from(balance!(5256000)))
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
    }
}
