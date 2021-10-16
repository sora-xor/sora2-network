#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

use codec::{Decode, Encode};
use common::prelude::Balance;

#[derive(Encode, Decode, Default, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct StakingInfo {
    /// Amount of deposited CERES
    deposited: Balance,
    /// Current rewards in CERES
    rewards: Balance,
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use common::prelude::{Balance, FixedWrapper};
    use frame_support::pallet_prelude::*;
    use frame_system::ensure_signed;
    use frame_system::pallet_prelude::*;
    use sp_runtime::traits::AccountIdConversion;
    use sp_runtime::ModuleId;

    const PALLET_ID: ModuleId = ModuleId(*b"cerstake");

    type Assets<T> = assets::Pallet<T>;
    type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
    type AssetId = common::AssetId32<common::PredefinedAssetId>;

    #[pallet::config]
    pub trait Config: frame_system::Config + assets::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        /// Number of Ceres distributed per block
        type CeresPerBlock: Get<Balance>;

        /// Ceres asset id
        type CeresAssetId: Get<AssetId>;

        /// Maximum Ceres in staking pool
        type MaximumCeresInStakingPool: Get<Balance>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub (super) trait Store)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::event]
    #[pallet::metadata(AccountIdOf < T > = "AccountId")]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Ceres deposited. [who, amount]
        Deposited(AccountIdOf<T>, Balance),
        /// Staked Ceres and rewards withdrawn. [who, staked, rewards]
        Withdrawn(AccountIdOf<T>, Balance, Balance),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Staking pool is full
        StakingPoolIsFull,
    }

    /// AccountId -> StakingInfo
    #[pallet::storage]
    #[pallet::getter(fn stakers)]
    pub(super) type Stakers<T: Config> =
        StorageMap<_, Identity, AccountIdOf<T>, StakingInfo, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn total_deposited)]
    pub(super) type TotalDeposited<T: Config> = StorageValue<_, Balance, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn rewards_remaining)]
    pub(super) type RewardsRemaining<T: Config> = StorageValue<_, Balance, ValueQuery>;

    #[pallet::genesis_config]
    pub struct GenesisConfig {
        pub rewards_remaining: Balance,
    }

    #[cfg(feature = "std")]
    impl Default for GenesisConfig {
        fn default() -> Self {
            Self {
                rewards_remaining: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig {
        fn build(&self) {
            RewardsRemaining::<T>::put(self.rewards_remaining);
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(10000)]
        pub fn deposit(origin: OriginFor<T>, amount: Balance) -> DispatchResultWithPostInfo {
            // Check that the extrinsic was signed and get the signer.
            // This function will return an error if the extrinsic is not signed.
            let source = ensure_signed(origin)?;

            // Maximum CERES to be in staking pool equals MaximumCeresInStakingPool
            let total_deposited = (FixedWrapper::from(TotalDeposited::<T>::get())
                + FixedWrapper::from(amount))
            .try_into_balance()
            .unwrap_or(TotalDeposited::<T>::get());
            ensure!(
                total_deposited <= T::MaximumCeresInStakingPool::get(),
                Error::<T>::StakingPoolIsFull
            );

            // Transfer CERES to staking
            Assets::<T>::transfer_from(
                &T::CeresAssetId::get().into(),
                &source,
                &Self::account_id(),
                amount,
            )?;

            // Update total deposited CERES amount
            TotalDeposited::<T>::put(total_deposited);

            // Get staking info of extrinsic caller
            let mut staking_info = <Stakers<T>>::get(&source);

            // Set staking info
            let deposited_amount =
                FixedWrapper::from(staking_info.deposited) + FixedWrapper::from(amount);
            staking_info.deposited = deposited_amount
                .try_into_balance()
                .unwrap_or(staking_info.deposited);

            // Put updated staking info into storage
            <Stakers<T>>::insert(&source, staking_info);

            // Emit an event
            Self::deposit_event(Event::<T>::Deposited(source, amount));

            // Return a successful DispatchResult
            Ok(().into())
        }

        #[pallet::weight(10000)]
        pub fn withdraw(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            // Check that the extrinsic was signed and get the signer.
            // This function will return an error if the extrinsic is not signed.
            let source = ensure_signed(origin)?;

            // Get staking info of extrinsic caller
            let staking_info = <Stakers<T>>::get(&source);
            let deposited = staking_info.deposited;
            let rewards = staking_info.rewards;
            let withdrawing_amount = deposited + rewards;

            // Withdraw CERES
            Assets::<T>::transfer_from(
                &T::CeresAssetId::get().into(),
                &Self::account_id(),
                &source,
                withdrawing_amount,
            )?;

            // Update total deposited CERES amount
            let total_deposited = (FixedWrapper::from(TotalDeposited::<T>::get())
                - FixedWrapper::from(deposited))
            .try_into_balance()
            .unwrap_or(TotalDeposited::<T>::get());
            TotalDeposited::<T>::put(total_deposited);

            // Update storage
            <Stakers<T>>::remove(&source);

            // Emit an event
            Self::deposit_event(Event::<T>::Withdrawn(source, deposited, rewards));

            // Return a successful DispatchResult
            Ok(().into())
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(_n: T::BlockNumber) -> Weight {
            let mut counter: u64 = 0;

            if RewardsRemaining::<T>::get() >= T::CeresPerBlock::get() {
                for staker in <Stakers<T>>::iter() {
                    let share_in_pool = FixedWrapper::from(staker.1.deposited)
                        / FixedWrapper::from(TotalDeposited::<T>::get());
                    let reward = share_in_pool * FixedWrapper::from(T::CeresPerBlock::get());

                    let mut staking_info = <Stakers<T>>::get(&staker.0);
                    staking_info.rewards = (FixedWrapper::from(staking_info.rewards) + reward)
                        .try_into_balance()
                        .unwrap_or(staking_info.rewards);

                    <Stakers<T>>::insert(&staker.0, staking_info);
                    counter += 1;
                }

                let rewards_remaining = FixedWrapper::from(RewardsRemaining::<T>::get())
                    - FixedWrapper::from(T::CeresPerBlock::get());
                RewardsRemaining::<T>::put(
                    rewards_remaining
                        .try_into_balance()
                        .unwrap_or(RewardsRemaining::<T>::get()),
                );
            }

            counter
        }
    }

    impl<T: Config> Pallet<T> {
        /// The account ID of pallet
        fn account_id() -> T::AccountId {
            PALLET_ID.into_account()
        }
    }
}
