#![cfg_attr(not(feature = "std"), no_std)]

pub mod weights;

mod benchmarking;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

use codec::{Decode, Encode};
use common::prelude::Balance;
use frame_support::weights::Weight;
pub use weights::WeightInfo;

#[derive(Encode, Decode, Default, PartialEq, Eq, scale_info::TypeInfo)]
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
    use common::balance;
    use common::prelude::{Balance, FixedWrapper};
    use frame_support::pallet_prelude::*;
    use frame_support::PalletId;
    use frame_system::ensure_signed;
    use frame_system::pallet_prelude::*;
    use hex_literal::hex;
    use sp_runtime::traits::{AccountIdConversion, Zero};

    const PALLET_ID: PalletId = PalletId(*b"cerstake");

    type Assets<T> = assets::Pallet<T>;
    type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
    type AssetId = common::AssetId32<common::PredefinedAssetId>;

    #[pallet::config]
    pub trait Config: frame_system::Config + assets::Config + technical::Config {
        /// One day represented in block number
        const BLOCKS_PER_ONE_DAY: BlockNumberFor<Self>;

        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Number of Ceres distributed per day
        type CeresPerDay: Get<Balance>;

        /// Ceres asset id
        type CeresAssetId: Get<AssetId>;

        /// Maximum Ceres in staking pool
        type MaximumCeresInStakingPool: Get<Balance>;

        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub (super) trait Store)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Ceres deposited. [who, amount]
        Deposited(AccountIdOf<T>, Balance),
        /// Staked Ceres and rewards withdrawn. [who, staked, rewards]
        Withdrawn(AccountIdOf<T>, Balance, Balance),
        /// Rewards changed [balance]
        RewardsChanged(Balance),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Staking pool is full
        StakingPoolIsFull,
        /// Unauthorized
        Unauthorized,
    }

    #[pallet::type_value]
    pub fn DefaultForAuthorityAccount<T: Config>() -> AccountIdOf<T> {
        let bytes = hex!("96ea3c9c0be7bbc7b0656a1983db5eed75210256891a9609012362e36815b132");
        AccountIdOf::<T>::decode(&mut &bytes[..]).unwrap()
    }

    /// Account which has permissions for changing remaining rewards
    #[pallet::storage]
    #[pallet::getter(fn authority_account)]
    pub type AuthorityAccount<T: Config> =
        StorageValue<_, AccountIdOf<T>, ValueQuery, DefaultForAuthorityAccount<T>>;

    /// AccountId -> StakingInfo
    #[pallet::storage]
    #[pallet::getter(fn stakers)]
    pub(super) type Stakers<T: Config> =
        StorageMap<_, Identity, AccountIdOf<T>, StakingInfo, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn total_deposited)]
    pub(super) type TotalDeposited<T: Config> = StorageValue<_, Balance, ValueQuery>;

    #[pallet::type_value]
    pub fn RewardsRemainingDefault() -> Balance {
        balance!(600)
    }

    #[pallet::storage]
    #[pallet::getter(fn rewards_remaining)]
    pub(super) type RewardsRemaining<T: Config> =
        StorageValue<_, Balance, ValueQuery, RewardsRemainingDefault>;

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::deposit())]
        pub fn deposit(origin: OriginFor<T>, amount: Balance) -> DispatchResultWithPostInfo {
            // Check that the extrinsic was signed and get the signer.
            // This function will return an error if the extrinsic is not signed.
            let source = ensure_signed(origin)?;

            // Maximum CERES to be in staking pool equals MaximumCeresInStakingPool
            let total_deposited = TotalDeposited::<T>::get() + amount;
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
            staking_info.deposited += amount;

            // Put updated staking info into storage
            <Stakers<T>>::insert(&source, staking_info);

            // Emit an event
            Self::deposit_event(Event::<T>::Deposited(source, amount));

            // Return a successful DispatchResult
            Ok(().into())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::deposit())]
        pub fn withdraw(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            // Check that the extrinsic was signed and get the signer.
            // This function will return an error if the extrinsic is not signed.
            let source = ensure_signed(origin)?;

            // Get staking info of extrinsic caller
            let staking_info = <Stakers<T>>::get(&source);
            let withdrawing_amount = staking_info.deposited + staking_info.rewards;

            // Withdraw CERES
            Assets::<T>::transfer_from(
                &T::CeresAssetId::get().into(),
                &Self::account_id(),
                &source,
                withdrawing_amount,
            )?;

            // Update total deposited CERES amount
            let total_deposited = TotalDeposited::<T>::get() - staking_info.deposited;
            TotalDeposited::<T>::put(total_deposited);

            // Update storage
            <Stakers<T>>::remove(&source);

            // Emit an event
            Self::deposit_event(Event::<T>::Withdrawn(
                source,
                staking_info.deposited,
                staking_info.rewards,
            ));

            // Return a successful DispatchResult
            Ok(().into())
        }

        /// Change RewardsRemaining
        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::change_rewards_remaining())]
        pub fn change_rewards_remaining(
            origin: OriginFor<T>,
            rewards_remaining: Balance,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            if user != AuthorityAccount::<T>::get() {
                return Err(Error::<T>::Unauthorized.into());
            }

            RewardsRemaining::<T>::put(rewards_remaining);

            // Emit an event
            Self::deposit_event(Event::RewardsChanged(rewards_remaining));

            Ok(().into())
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(now: T::BlockNumber) -> Weight {
            let mut counter: u64 = 0;

            if (now % T::BLOCKS_PER_ONE_DAY).is_zero() {
                let rewards_remaining = RewardsRemaining::<T>::get();
                let ceres_per_day = T::CeresPerDay::get();

                if rewards_remaining >= ceres_per_day {
                    let total_deposited = FixedWrapper::from(TotalDeposited::<T>::get());
                    let ceres_per_day_fixed = FixedWrapper::from(ceres_per_day);

                    for staker in <Stakers<T>>::iter() {
                        let share_in_pool =
                            FixedWrapper::from(staker.1.deposited) / total_deposited.clone();
                        let reward = share_in_pool * ceres_per_day_fixed.clone();

                        let mut staking_info = <Stakers<T>>::get(&staker.0);
                        staking_info.rewards = (FixedWrapper::from(staking_info.rewards) + reward)
                            .try_into_balance()
                            .unwrap_or(staking_info.rewards);

                        <Stakers<T>>::insert(&staker.0, staking_info);
                        counter += 1;
                    }

                    RewardsRemaining::<T>::put(rewards_remaining - ceres_per_day);
                }
            }

            T::DbWeight::get()
                .reads(4)
                .saturating_add(T::DbWeight::get().writes(counter))
        }
    }

    impl<T: Config> Pallet<T> {
        /// The account ID of pallet
        fn account_id() -> T::AccountId {
            PALLET_ID.into_account_truncating()
        }
    }
}
