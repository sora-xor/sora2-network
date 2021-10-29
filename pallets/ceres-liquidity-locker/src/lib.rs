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

#[derive(Encode, Decode, Default, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct CheckLockInfo<Balance> {
    locked: bool,
    allowed_withdrawal_amount: Balance,
}

pub use pallet::*;
#[frame_support::pallet]
pub mod pallet {
    use crate::{CheckLockInfo, LockInfo};
    use common::prelude::Balance;
    use frame_support::pallet_prelude::*;
    use frame_system::ensure_signed;
    use frame_system::pallet_prelude::*;
    use sp_runtime::traits::AccountIdConversion;
    use sp_runtime::ModuleId;
    use sp_std::vec::Vec;

    const PALLET_ID: ModuleId = ModuleId(*b"crlocker");

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
    }

    type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
    type AssetId = common::AssetId32<common::PredefinedAssetId>;

    #[pallet::pallet]
    #[pallet::generate_store(pub (super) trait Store)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::storage]
    #[pallet::getter(fn locker_data)]
    pub(super) type LockerData<T: Config> = StorageMap<
        _,
        Identity,
        AccountIdOf<T>,
        Vec<LockInfo<Balance, T::BlockNumber, AssetId>>,
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
            asset_a: AssetId,
            asset_b: AssetId,
            unlocking_block: T::BlockNumber,
            percentage_of_pool_tokens: Balance,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            let lock_info = LockInfo {
                pool_tokens: 0,
                asset_a,
                asset_b,
                unlocking_block,
            };

            // Get current block
            let current_block = frame_system::Pallet::<T>::block_number();

            // CALCULATE NUMBER OF POOL TOKENS TO BE LOCKED

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
        /// Check if liquidity locked
        pub fn check_if_liquidity_locked(
            user: T::AccountId,
            asset_a: AssetId,
            asset_b: AssetId,
            pool_tokens: Balance,
        ) -> CheckLockInfo<Balance> {
            // Get lock info of extrinsic caller
            let mut lockups = <LockerData<T>>::get(&user);
            let current_block = frame_system::Pallet::<T>::block_number();
            let mut temp = CheckLockInfo {
                locked: false,
                allowed_withdrawal_amount: 0,
            };
            let mut counter = 0;

            for (i, lock_info) in lockups.iter().enumerate() {
                if lock_info.asset_a == asset_a && lock_info.asset_b == asset_b {
                    if current_block < lock_info.unlocking_block {
                        temp.locked = true;
                        if lock_info.pool_tokens < pool_tokens {
                            temp.allowed_withdrawal_amount = pool_tokens - lock_info.pool_tokens;
                        }
                        break;
                    } else {
                        counter = i;
                    }
                }
            }

            lockups.remove(counter);
            <LockerData<T>>::insert(&user, lockups);

            return temp;
        }

        /// The account ID of pallet
        fn account_id() -> T::AccountId {
            PALLET_ID.into_account()
        }
    }
}
