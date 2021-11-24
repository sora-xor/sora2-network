#![cfg_attr(not(feature = "std"), no_std)]
#![feature(destructuring_assignment)]

use codec::{Decode, Encode};

#[derive(Encode, Decode, Default, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct LockInfo<Balance, BlockNumber, AssetId> {
    /// Amount of locked pool tokens
    pool_tokens: Balance,
    /// The time (block height) at which the tokens will be unlock
    unlocking_block: BlockNumber,
    /// Base asset of locked liquidity
    asset_a: AssetId,
    /// Target asset of locked liquidity
    asset_b: AssetId,
}

pub use pallet::*;
#[frame_support::pallet]
pub mod pallet {
    use crate::LockInfo;
    use common::prelude::{Balance, FixedWrapper};
    use common::{balance, LiquiditySource};
    use frame_support::pallet_prelude::*;
    use frame_system::ensure_signed;
    use frame_system::pallet_prelude::*;
    use hex_literal::hex;
    use sp_std::vec::Vec;

    #[pallet::config]
    pub trait Config: frame_system::Config + assets::Config {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        /// Reference to pool_xyk pallet
        type XYKPool: LiquiditySource<
            Self::DEXId,
            Self::AccountId,
            Self::AssetId,
            Balance,
            DispatchError,
        >;

        /// Ceres asset id
        type CeresAssetId: Get<Self::AssetId>;
    }

    type Assets<T> = assets::Pallet<T>;
    type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
    type AssetIdOf<T> = <T as assets::Config>::AssetId;

    #[pallet::pallet]
    #[pallet::generate_store(pub (super) trait Store)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::type_value]
    pub fn DefaultForFeesOptionOneAccount<T: Config>() -> AccountIdOf<T> {
        let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
        AccountIdOf::<T>::decode(&mut &bytes[..]).unwrap_or_default()
    }

    #[pallet::type_value]
    pub fn DefaultForFeesOptionTwoAccount<T: Config>() -> AccountIdOf<T> {
        let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
        AccountIdOf::<T>::decode(&mut &bytes[..]).unwrap_or_default()
    }

    /// Account for collecting fees from Option 1
    #[pallet::storage]
    #[pallet::getter(fn fees_option_one_account)]
    pub type FeesOptionOneAccount<T: Config> =
        StorageValue<_, AccountIdOf<T>, ValueQuery, DefaultForFeesOptionOneAccount<T>>;

    /// Account for collecting fees from Option 2
    #[pallet::storage]
    #[pallet::getter(fn fees_option_two_account)]
    pub type FeesOptionTwoAccount<T: Config> =
        StorageValue<_, AccountIdOf<T>, ValueQuery, DefaultForFeesOptionTwoAccount<T>>;

    #[pallet::storage]
    #[pallet::getter(fn locker_data)]
    pub(super) type LockerData<T: Config> = StorageMap<
        _,
        Identity,
        AccountIdOf<T>,
        Vec<LockInfo<Balance, T::BlockNumber, AssetIdOf<T>>>,
        ValueQuery,
    >;

    #[pallet::event]
    #[pallet::metadata(AccountIdOf<T> = "AccountId", BalanceOf<T> = "Balance", T::BlockNumber = "BlockNumber")]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Funds Locked [who, amount, block]
        Locked(AccountIdOf<T>, Balance, T::BlockNumber),
    }

    #[pallet::error]
    pub enum Error<T> {
        ///Insufficient liquidity to lock
        InsufficientLiquidityToLock,
        ///Percentage greater than 100%
        InvalidPercentage,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Lock liquidity
        #[pallet::weight(10000)]
        pub fn lock_liquidity(
            origin: OriginFor<T>,
            asset_a: AssetIdOf<T>,
            asset_b: AssetIdOf<T>,
            unlocking_block: T::BlockNumber,
            percentage_of_pool_tokens: Balance,
            option: bool,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            ensure!(
                percentage_of_pool_tokens <= balance!(1),
                Error::<T>::InvalidPercentage
            );

            let mut lock_info = LockInfo {
                pool_tokens: 0,
                asset_a,
                asset_b,
                unlocking_block,
            };

            // Get current block
            let current_block = frame_system::Pallet::<T>::block_number();

            // Get pool account
            let pool_account: AccountIdOf<T> = T::XYKPool::properties(asset_a, asset_b)
                .expect("Pool does not exist")
                .0;

            // Calculate number of pool tokens to be locked
            let pool_tokens = T::XYKPool::pool_providers(pool_account.clone(), user.clone())
                .expect("User is not pool provider");
            lock_info.pool_tokens = (FixedWrapper::from(pool_tokens)
                * FixedWrapper::from(percentage_of_pool_tokens))
            .try_into_balance()
            .unwrap_or(lock_info.pool_tokens);

            // Check if user has enough liquidity to lock
            let mut lockups = <LockerData<T>>::get(&user);
            let mut locked_pool_tokens = 0;

            for (_, locks) in lockups.iter().enumerate() {
                if locks.asset_a == asset_a && locks.asset_b == asset_b {
                    if current_block < locks.unlocking_block {
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
                    pool_account,
                    asset_a,
                    asset_b,
                    user.clone(),
                    lock_info.pool_tokens,
                    FixedWrapper::from(0.01),
                    option,
                )?;
            } else {
                // Transfer 20 CERES
                Assets::<T>::transfer_from(
                    &T::CeresAssetId::get().into(),
                    &user,
                    &FeesOptionTwoAccount::<T>::get(),
                    balance!(20),
                )?;
                // Transfer 0.5% of LP tokens
                Self::pay_fee_in_lp_tokens(
                    pool_account,
                    asset_a,
                    asset_b,
                    user.clone(),
                    lock_info.pool_tokens,
                    FixedWrapper::from(0.005),
                    option,
                )?;
            }

            // Put updated address info into storage
            // Get lock info of extrinsic caller
            lockups.push(lock_info);
            <LockerData<T>>::insert(&user, lockups);

            // Emit an event
            Self::deposit_event(Event::Locked(
                user,
                percentage_of_pool_tokens,
                current_block,
            ));

            // Return a successful DispatchResult
            Ok(().into())
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {}

    impl<T: Config> Pallet<T> {
        /// Get allowed liquidity for withdrawing
        pub fn get_allowed_liquidity_for_withdrawing(
            user: &AccountIdOf<T>,
            asset_a: AssetIdOf<T>,
            asset_b: AssetIdOf<T>,
            withdrawing_amount: Balance,
        ) -> bool {
            // Get lock info of extrinsic caller
            let mut lockups = <LockerData<T>>::get(&user);
            let current_block = frame_system::Pallet::<T>::block_number();

            // Get pool account
            let pool_account: AccountIdOf<T> = T::XYKPool::properties(asset_a, asset_b)
                .expect("Pool does not exist")
                .0;

            // Calculate number of pool tokens to be locked
            let pool_tokens = T::XYKPool::pool_providers(pool_account.clone(), user.clone())
                .expect("User is not pool provider");

            let mut locked_pool_tokens = 0;
            let mut expired_locks = Vec::new();

            for (i, locks) in lockups.iter().enumerate() {
                if locks.asset_a == asset_a && locks.asset_b == asset_b {
                    if current_block < locks.unlocking_block {
                        locked_pool_tokens = locked_pool_tokens + locks.pool_tokens;
                    } else {
                        expired_locks.push(i);
                    }
                }
            }
            let unlocked_pool_tokens = pool_tokens - locked_pool_tokens;

            for (_, index) in expired_locks.iter().enumerate() {
                lockups.remove(*index);
            }
            <LockerData<T>>::insert(&user, lockups);

            return if withdrawing_amount > pool_tokens || unlocked_pool_tokens >= withdrawing_amount
            {
                true
            } else {
                false
            };
        }

        /// Pay Locker fees in LP tokens
        fn pay_fee_in_lp_tokens(
            pool_account: AccountIdOf<T>,
            asset_a: AssetIdOf<T>,
            asset_b: AssetIdOf<T>,
            user: AccountIdOf<T>,
            mut pool_tokens: Balance,
            fee_percentage: FixedWrapper,
            option: bool,
        ) -> Result<(), DispatchError> {
            pool_tokens = (FixedWrapper::from(pool_tokens) * fee_percentage)
                .try_into_balance()
                .unwrap_or(0);

            let mut fee_account = FeesOptionOneAccount::<T>::get();
            if !option {
                fee_account = FeesOptionTwoAccount::<T>::get();
            }

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
