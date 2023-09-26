#![cfg_attr(not(feature = "std"), no_std)]
// TODO #167: fix clippy warnings
#![allow(clippy::all)]

pub mod migrations;
pub mod weights;

mod benchmarking;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

use codec::{Decode, Encode};
use common::Balance;
pub use weights::WeightInfo;

#[derive(Encode, Decode, Default, PartialEq, Eq, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct VotingInfo {
    /// Voting option
    voting_option: u32,
    /// Number of votes
    number_of_votes: Balance,
    /// Ceres withdrawn
    ceres_withdrawn: bool,
}

#[derive(Encode, Decode, Default, PartialEq, Eq, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct PollInfo<Moment> {
    /// Number of options
    pub number_of_options: u32,
    /// Poll start timestamp
    pub poll_start_timestamp: Moment,
    /// Poll end timestamp
    pub poll_end_timestamp: Moment,
}

/// Storage version.
#[derive(Encode, Decode, Eq, PartialEq, scale_info::TypeInfo)]
pub enum StorageVersion {
    /// Initial version
    V1,
    /// After migrating to timestamp calculation
    V2,
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use crate::{migrations, PollInfo, StorageVersion, VotingInfo, WeightInfo};
    use common::prelude::Balance;
    use frame_support::pallet_prelude::*;
    use frame_support::sp_runtime::traits::AccountIdConversion;
    use frame_support::transactional;
    use frame_support::PalletId;
    use frame_system::ensure_signed;
    use frame_system::pallet_prelude::*;
    use pallet_timestamp as timestamp;
    use sp_std::prelude::*;

    const PALLET_ID: PalletId = PalletId(*b"ceresgov");

    #[pallet::config]
    pub trait Config:
        frame_system::Config + assets::Config + technical::Config + timestamp::Config
    {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Ceres asset id
        type CeresAssetId: Get<Self::AssetId>;

        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    type Assets<T> = assets::Pallet<T>;
    pub type Timestamp<T> = timestamp::Pallet<T>;
    pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

    #[pallet::pallet]
    #[pallet::generate_store(pub (super) trait Store)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(PhantomData<T>);

    /// A vote of a particular user for a particular poll
    #[pallet::storage]
    #[pallet::getter(fn votings)]
    pub type Voting<T: Config> =
        StorageDoubleMap<_, Identity, Vec<u8>, Identity, AccountIdOf<T>, VotingInfo, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn poll_data)]
    pub type PollData<T: Config> =
        StorageMap<_, Identity, Vec<u8>, PollInfo<T::Moment>, ValueQuery>;

    #[pallet::type_value]
    pub fn DefaultForPalletStorageVersion<T: Config>() -> StorageVersion {
        StorageVersion::V1
    }

    /// Pallet storage version
    #[pallet::storage]
    #[pallet::getter(fn pallet_storage_version)]
    pub type PalletStorageVersion<T: Config> =
        StorageValue<_, StorageVersion, ValueQuery, DefaultForPalletStorageVersion<T>>;

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Voting [who, poll, option, balance]
        Voted(AccountIdOf<T>, Vec<u8>, u32, Balance),
        /// Create poll [who, option, start_timestamp, end_timestamp]
        Created(AccountIdOf<T>, u32, T::Moment, T::Moment),
        /// Withdrawn [who, balance]
        Withdrawn(AccountIdOf<T>, Balance),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Invalid votes
        InvalidVotes,
        /// Poll is finished
        PollIsFinished,
        /// Poll is not started
        PollIsNotStarted,
        /// Not enough funds
        NotEnoughFunds,
        /// Invalid number of option
        InvalidNumberOfOption,
        /// Vote denied
        VoteDenied,
        /// Invalid start timestamp
        InvalidStartTimestamp,
        /// Invalid end timestamp
        InvalidEndTimestamp,
        /// Poll is not finished
        PollIsNotFinished,
        /// Invalid number of votes
        InvalidNumberOfVotes,
        /// Funds already withdrawn,
        FundsAlreadyWithdrawn,
        /// Poll id already exists
        PollIdAlreadyExists,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Voting for option
        #[transactional]
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::vote())]
        pub fn vote(
            origin: OriginFor<T>,
            poll_id: Vec<u8>,
            voting_option: u32,
            number_of_votes: Balance,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            ensure!(number_of_votes > 0, Error::<T>::InvalidNumberOfVotes);

            let poll_info = PollData::<T>::get(&poll_id);
            let current_timestamp = Timestamp::<T>::get();

            ensure!(
                current_timestamp >= poll_info.poll_start_timestamp,
                Error::<T>::PollIsNotStarted
            );

            ensure!(
                current_timestamp <= poll_info.poll_end_timestamp,
                Error::<T>::PollIsFinished
            );

            ensure!(
                voting_option <= poll_info.number_of_options && voting_option != 0,
                Error::<T>::InvalidNumberOfOption
            );

            let mut voting_info = <Voting<T>>::get(&poll_id, &user);

            if voting_info.voting_option == 0 {
                voting_info.voting_option = voting_option;
            } else {
                ensure!(
                    voting_info.voting_option == voting_option,
                    Error::<T>::VoteDenied
                )
            }

            voting_info.number_of_votes += number_of_votes;

            // Transfer Ceres to pallet
            Assets::<T>::transfer_from(
                &T::CeresAssetId::get().into(),
                &user,
                &Self::account_id(),
                number_of_votes,
            )
            .map_err(|_assets_err| Error::<T>::NotEnoughFunds)?;

            // Update storage
            <Voting<T>>::insert(&poll_id, &user, voting_info);

            //Emit event
            Self::deposit_event(Event::<T>::Voted(
                user,
                poll_id,
                voting_option,
                number_of_votes,
            ));

            // Return a successful DispatchResult
            Ok(().into())
        }

        /// Create poll
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::create_poll())]
        pub fn create_poll(
            origin: OriginFor<T>,
            poll_id: Vec<u8>,
            number_of_options: u32,
            poll_start_timestamp: T::Moment,
            poll_end_timestamp: T::Moment,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            let current_timestamp = Timestamp::<T>::get();

            let poll_info = <PollData<T>>::get(&poll_id);
            ensure!(
                poll_info.number_of_options == 0,
                Error::<T>::PollIdAlreadyExists
            );

            ensure!(number_of_options >= 2, Error::<T>::InvalidNumberOfOption);

            ensure!(
                poll_start_timestamp >= current_timestamp,
                Error::<T>::InvalidStartTimestamp
            );

            ensure!(
                poll_end_timestamp > poll_start_timestamp,
                Error::<T>::InvalidEndTimestamp
            );

            let poll_info = PollInfo {
                number_of_options,
                poll_end_timestamp,
                poll_start_timestamp,
            };

            <PollData<T>>::insert(&poll_id, poll_info);

            //Emit event
            Self::deposit_event(Event::<T>::Created(
                user,
                number_of_options,
                poll_start_timestamp,
                poll_end_timestamp,
            ));

            // Return a successful DispatchResult
            Ok(().into())
        }

        /// Withdraw voting funds
        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::withdraw())]
        pub fn withdraw(origin: OriginFor<T>, poll_id: Vec<u8>) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            let poll_info = PollData::<T>::get(&poll_id);
            let current_timestamp = Timestamp::<T>::get();

            ensure!(
                current_timestamp > poll_info.poll_end_timestamp,
                Error::<T>::PollIsNotFinished
            );

            // Update storage
            let mut voting_info = <Voting<T>>::get(&poll_id, &user);
            ensure!(
                voting_info.ceres_withdrawn == false,
                Error::<T>::FundsAlreadyWithdrawn
            );

            // Withdraw CERES
            Assets::<T>::transfer_from(
                &T::CeresAssetId::get().into(),
                &Self::account_id(),
                &user,
                voting_info.number_of_votes,
            )?;

            voting_info.ceres_withdrawn = true;
            <Voting<T>>::insert(&poll_id, &user, &voting_info);

            //Emit event
            Self::deposit_event(Event::<T>::Withdrawn(user, voting_info.number_of_votes));

            // Return a successful DispatchResult
            Ok(().into())
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
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
    }
}
