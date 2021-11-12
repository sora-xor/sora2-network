#![cfg_attr(not(feature = "std"), no_std)]
#![feature(destructuring_assignment)]

use codec::{Decode, Encode};

#[derive(Encode, Decode, Default, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct LockInfo<Balance, BlockNumber, AssetId> {
    /// Balance of pooled tokens
    pool_tokens: Balance,
    /// The time (block height) at which the tokens will be unlock
    unlocking_block: BlockNumber,
    /// Balance of first pair of tokens
    asset_a: AssetId,
    /// Balance of second pair of tokens
    asset_b: AssetId,
}

pub use pallet::*;
#[frame_support::pallet]
pub mod pallet {
    use crate::LockInfo;
    use common::prelude::Balance;
    use common::LiquiditySource;
    use frame_support::pallet_prelude::*;
    use frame_system::ensure_signed;
    use frame_system::pallet_prelude::*;
    use sp_runtime::traits::AccountIdConversion;
    use sp_runtime::ModuleId;
    use sp_std::vec::Vec;

    const PALLET_ID: ModuleId = ModuleId(*b"crlocker");

    #[pallet::config]
    pub trait Config: frame_system::Config + assets::Config {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type XYKPool: LiquiditySource<
            Self::DEXId,
            Self::AccountId,
            Self::AssetId,
            Balance,
            DispatchError,
        >;
    }

    type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

    #[pallet::pallet]
    #[pallet::generate_store(pub (super) trait Store)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::storage]
    #[pallet::getter(fn locker_data)]
    pub(super) type LockerData<T: Config> = StorageMap<
        _,
        Identity,
        AccountIdOf<T>,
        Vec<LockInfo<Balance, T::BlockNumber, T::AssetId>>,
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
        ///No funds deposited
        NoFundsDeposited,
        ///Funds are deposited
        FundsAreDeposited,
        ///Asset missing
        AssetMissing,
        ///Liquidity Is Locked
        LiquidityIsLocked,
        ///Cant Unlock Liquidity
        CantUnlockLiquidity,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Lock liquidity
        #[pallet::weight(10000)]
        pub fn lock_liquidity(
            origin: OriginFor<T>,
            asset_a: T::AssetId,
            asset_b: T::AssetId,
            unlocking_block: T::BlockNumber,
            percentage_of_pool_tokens: Balance,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            let mut lock_info = LockInfo {
                pool_tokens: 0,
                asset_a,
                asset_b,
                unlocking_block,
            };

            // Get current block
            let current_block = frame_system::Pallet::<T>::block_number();

            // Calculate number of pool tokens to be locked
            let pool_account: AccountIdOf<T> = T::XYKPool::properties(asset_a, asset_b)
                .expect("Pool does not exist")
                .0;
            let pool_tokens = T::XYKPool::pool_providers(pool_account, user.clone())
                .expect("User is not pool provider");
            lock_info.pool_tokens = pool_tokens * percentage_of_pool_tokens;

            // Put updated address info into storage
            // Get lock info of extrinsic caller
            let mut lockups = <LockerData<T>>::get(&user);
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
            user: &T::AccountId,
            asset_a: T::AssetId,
            asset_b: T::AssetId,
            pool_tokens: Balance,
        ) -> Balance {
            // Get lock info of extrinsic caller
            let mut lockups = <LockerData<T>>::get(&user);
            let current_block = frame_system::Pallet::<T>::block_number();
            let mut allowed_withdrawal_amount: Balance = 0;
            let mut counter = 0;

            for (i, lock_info) in lockups.iter().enumerate() {
                if lock_info.asset_a == asset_a && lock_info.asset_b == asset_b {
                    if current_block < lock_info.unlocking_block {
                        if lock_info.pool_tokens < pool_tokens {
                            allowed_withdrawal_amount = pool_tokens - lock_info.pool_tokens;
                        }
                        break;
                    } else {
                        counter = i;
                    }
                }
            }

            lockups.remove(counter);
            <LockerData<T>>::insert(&user, lockups);

            return allowed_withdrawal_amount;
        }

        /// The account ID of pallet
        fn account_id() -> T::AccountId {
            PALLET_ID.into_account()
        }
    }
}
