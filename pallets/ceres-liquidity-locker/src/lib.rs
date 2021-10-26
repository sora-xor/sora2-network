#![cfg_attr(not(feature = "std"), no_std)]
#![feature(destructuring_assignment)]

pub use pallet::*;
use codec::{Decode, Encode};

pub use weights::WeightInfo;

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
pub struct Lockups<LockInfo> {
    lockups: Vec<LockInfo>,
}

#[derive(Encode, Decode, Default, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct CheckLockInfo<Balance> {
    locked: bool,
    allowed_withdrawal_amount: Balance,
}

// Jos jedna struktura boolean true/false da li je likvidnost zakljucana i allowed_withdrawal_amount

#[frame_support::pallet]
pub mod pallet {
    use frame_system::pallet_prelude::*;
    use frame_support::{dispatch::DispatchResult, pallet_prelude::*, PalletId};
    use frame_support::traits::{Currency, ReservableCurrency, ExistenceRequirement, LockableCurrency, WithdrawReasons};
    use frame_support::sp_runtime::traits::{Saturating, Zero, One};
    use frame_support::sp_runtime::sp_std::convert::TryInto;
    use frame_support::sp_runtime::{FixedU128, FixedPointNumber, SaturatedConversion};
    use sp_runtime::offchain::storage_lock::Lockable;
    use sp_runtime::traits::AccountIdConversion;
    use crate::{CheckLockInfo, LockInfo, Lockups, WeightInfo};
    use crate::aliases::{AssetIdOf};
    use common::prelude::{Balance, FixedWrapper};

    const PALLET_ID: PalletId = PalletId(*b"crlocker");

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        /// The currency in which deposit work
        type Currency: LockableCurrency<Self::AccountId, Moment = Self::BlockNumber>;

        /// Weight information for extrinsics in this pallet
        type WeightInfo: WeightInfo;

        /// Get pooled assets
        type BalanceOf: Get<Balance>;

        /// Get asset ID
        type AssetId: Get<AsssetIdOf>;
    }

    type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
    type AssetIdOf<T> = <T as Config>::AssetId;
    pub(crate) type BalanceOf<T> = <<T as Config>::Currency as Currency<AccountIdOf<T>>>::Balance;

    #[pallet::pallet]
    #[pallet::generate_store(pub (super) trait Store)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::storage]
    #[pallet::getter(fn locker_data)]
    pub(super) type LockerData<T: Config> = StorageMap<_, Blake2_128Concat, AccountIdOf<T>, Lockups<LockInfo<Balance, BlockNumber, AssetIdOf<T>>>, ValueQuery>;

    #[pallet::event]
    #[pallet::metadata(AccountIdOf<T> = "AccountId", BalanceOf<T> = "Balance", T::BlockNumber = "BlockNumber")]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Funds Locked [who, amount, block]
        Locked(AccountIdOf<T>, BalanceOf<T>, T::BlockNumber),
        /// Funds Unlocked
        Unlock(<T as frame_system::Config>::AccountId),
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
        CantUnlockLiquidity
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Lock liquidity
        #[pallet::weight(T::WeightInfo::lock_liquidity())]
        pub fn lock_liquidity(origin: OriginFor<T>,
                              asset_a: AssetIdOf<T>,
                              asset_b: AssetIdOf<T>,
                              unlocking_block: BlockNumber,
                              percentage_of_pool_tokens: Balance) -> DispatchResult {

            let user = ensure_signed(origin)?;

            let mut lock_info = LockInfo {
                pool_tokens: 0,
                asset_a,
                asset_b,
                unlocking_block
            };
            // lock_info.pool_tokens = ...

            /*let asset_pair = AssetIdOf::<T> {
                asset_a,
                asset_b
            };
            ensure!(Self::check_if_liquidity_locked(user, asset_pair, lock_info.pool_tokens)?, Error::<T>::LiquidityIsLocked);//ne treba
            */

            // Get current block
            let current_block = frame_system::Pallet::<T>::block_number();

            //Set lock info
            let pool_tokens_fixed = FixedU128::from_inner(lock_info.pool_tokens.saturated_into::<u128>());
            let percentage_of_pool_tokens_fixed = FixedU128::from_inner(percentage_of_pool_tokens.saturated_into::<u128>());
            lock_info.pool_tokens = pool_tokens_fixed.saturating_add(percentage_of_pool_tokens_fixed).into_inner().satur;


            // Put updated address info into storage
            // Get lock info of extrinsic caller
            let mut lockups = <LockerData<T>>::get(&user);
            lockups.add(lock_info);
            <LockerData<T>>::insert(&user, lockups);

            // Emit an event
            Self::deposit_event(Event::Locked(&user, percentage_of_pool_tokens, current_block));

            // Return a successful DispatchResult
            Ok(())
        }

        // /// Unlock liquidity
        /*#[pallet::weight(T::WeightInfo::unlock_liquidity())]
        pub fn unlock_liquidity(origin: OriginFor<T>, asset_a: AssetIdOf<T>, asset_b: AssetIdOf<T>) -> DispatchResult {

            let user = ensure_signed(origin)?;
            // Get address info of extrinsic caller and check if it has deposited funds
            let mut lockups = <LockerData<T>>::get(&user);
            ensure!(lock_info.pool_tokens != <BalanceOf<T>>::zero(), Error::<T>::NoFundsDeposited); // PREBACITI GORE PROVERU
            // DA LI SU POOL_TOKENS 0

            // Get current block
            let current_block = frame_system::Pallet::<T>::block_number();
            let asset_pair = AssetIdOf::<T> {
                asset_a,
                asset_b
            };
            ensure!(Self::check_if_liquidity_locked(user, asset_pair, lock_info.pool_tokens)?, Error::<T>::LiquidityIsLocked);
            ensure!(lock_info.unlocking_block == current_block, Error::<T>::CantUnlockLiquidity);

            T::Currency::remove_lock(PALLET_ID, &user);

            Self::deposit_event(Event::Unlock(&user));
            Ok(().into())
        }*/
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {}

    impl<T: Config> Pallet<T> {
        /// Check if liquidity locked

        pub fn check_if_liquidity_locked(user: T::AccountId, asset_a: AssetIdOf<T>, asset_b: AssetIdOf<T>, pool_tokens: Balance) -> CheckLockInfo<Balance> {
            // Get lock info of extrinsic caller
            let lockups = <LockerData<T>>::get(&user);
            let current_block = frame_system::Pallet::<T>::block_number();
            let mut temp = CheckLockInfo{ locked: false, allowed_withdrawal_amount: () };

            for lock_info in lockups.iter() {
                if lock_info.asset_a == asset_a && lock_info.asset_b == asset_b {
                    if current_block <= lock_info.unlocking_block {
                        temp.locked = true;
                        if lock_info.pool_tokens < pool_tokens {
                            temp.allowed_withdrawal_amount = pool_tokens - lock_info.pool_tokens;
                        }
                        break;
                    }
                }
            }

            return temp;
        }

        /// The account ID of pallet
        fn account_id() -> T::AccountId {
            PALLET_ID.into_account()
        }
    }
}