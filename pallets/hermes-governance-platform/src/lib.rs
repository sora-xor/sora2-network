#![cfg_attr(not(feature = "std"), no_std)]

mod benchmarking;
pub mod weights;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

extern crate alloc;

use alloc::string::String;
use codec::{Decode, Encode};
use common::Balance;
use frame_support::weights::Weight;
use frame_support::RuntimeDebug;

pub trait WeightInfo {
    fn vote() -> Weight;
    fn create_poll() -> Weight;
    fn withdraw_funds_voter() -> Weight;
    fn withdraw_funds_creator() -> Weight;
    fn change_min_hermes_for_voting() -> Weight;
    fn change_min_hermes_for_creating_poll() -> Weight;
}

#[derive(Encode, Decode, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo, Clone, Copy)]
pub enum VotingOption {
    Yes,
    No,
}

#[derive(Encode, Decode, PartialEq, Eq, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct HermesVotingInfo {
    /// Voting option
    voting_option: VotingOption,
    /// Number of hermes
    number_of_hermes: Balance,
    /// Hermes withdrawn
    hermes_withdrawn: bool,
}

#[derive(Encode, Decode, Default, PartialEq, Eq, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct HermesPollInfo<AccountId, Moment> {
    /// Creator of poll
    pub creator: AccountId,
    /// Hermes Locked
    pub hermes_locked: Balance,
    /// Poll start timestamp
    pub poll_start_timestamp: Moment,
    /// Poll end timestamp
    pub poll_end_timestamp: Moment,
    /// Poll title
    pub title: String,
    /// Description
    pub description: String,
    /// Creator Hermes withdrawn
    pub creator_hermes_withdrawn: bool,
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use crate::{HermesPollInfo, HermesVotingInfo, VotingOption, WeightInfo};
    use alloc::string::String;
    use common::prelude::Balance;
    use common::{balance, AssetInfoProvider};
    use frame_support::pallet_prelude::*;
    use frame_support::sp_runtime::traits::AccountIdConversion;
    use frame_support::transactional;
    use frame_support::PalletId;
    use frame_system::ensure_signed;
    use frame_system::pallet_prelude::*;
    use hex_literal::hex;
    use pallet_timestamp as timestamp;
    use sp_core::H256;
    use sp_io::hashing::blake2_256;

    const PALLET_ID: PalletId = PalletId(*b"hermsgov");

    // TODO: #395 use AssetInfoProvider instead of assets pallet
    #[pallet::config]
    pub trait Config:
        frame_system::Config + assets::Config + technical::Config + timestamp::Config
    {
        /// Minimum duration of poll represented in milliseconds
        const MIN_DURATION_OF_POLL: Self::Moment;

        /// Maximum duration of poll represented in milliseconds
        const MAX_DURATION_OF_POLL: Self::Moment;

        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Hermes asset id
        type HermesAssetId: Get<Self::AssetId>;

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
    #[pallet::getter(fn hermes_votings)]
    pub type HermesVotings<T: Config> = StorageDoubleMap<
        _,
        Identity,
        H256,
        Identity,
        AccountIdOf<T>,
        HermesVotingInfo,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn hermes_poll_data)]
    pub type HermesPollData<T: Config> =
        StorageMap<_, Identity, H256, HermesPollInfo<AccountIdOf<T>, T::Moment>, OptionQuery>;

    #[pallet::type_value]
    pub fn DefaultMinimumHermesVotingAmount<T: Config>() -> Balance {
        balance!(1000)
    }

    #[pallet::storage]
    #[pallet::getter(fn min_hermes_for_voting)]
    pub type MinimumHermesVotingAmount<T: Config> =
        StorageValue<_, Balance, ValueQuery, DefaultMinimumHermesVotingAmount<T>>;

    #[pallet::type_value]
    pub fn DefaultMinimumHermesAmountForCreatingPoll<T: Config>() -> Balance {
        balance!(100000)
    }

    #[pallet::storage]
    #[pallet::getter(fn min_hermes_for_creating_poll)]
    pub type MinimumHermesAmountForCreatingPoll<T: Config> =
        StorageValue<_, Balance, ValueQuery, DefaultMinimumHermesAmountForCreatingPoll<T>>;

    #[pallet::type_value]
    pub fn DefaultForAuthorityAccount<T: Config>() -> AccountIdOf<T> {
        let bytes = hex!("96ea3c9c0be7bbc7b0656a1983db5eed75210256891a9609012362e36815b132");
        AccountIdOf::<T>::decode(&mut &bytes[..]).unwrap()
    }

    /// Account which has permissions for changing Hermes minimum amount for voting and creating a poll
    #[pallet::storage]
    #[pallet::getter(fn authority_account)]
    pub type AuthorityAccount<T: Config> =
        StorageValue<_, AccountIdOf<T>, ValueQuery, DefaultForAuthorityAccount<T>>;

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Voting [who, poll, option]
        Voted(AccountIdOf<T>, H256, VotingOption),
        /// Create poll [who, title, start_timestamp, end_timestamp]
        Created(AccountIdOf<T>, String, T::Moment, T::Moment),
        /// Voter Funds Withdrawn [who, balance]
        VoterFundsWithdrawn(AccountIdOf<T>, Balance),
        /// Creator Funds Withdrawn [who, balance]
        CreatorFundsWithdrawn(AccountIdOf<T>, Balance),
        /// Change minimum Hermes for voting [balance]
        MinimumHermesForVotingChanged(Balance),
        /// Change minimum Hermes for creating poll [balance]
        MinimumHermesForCreatingPollChanged(Balance),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Poll Is Not Started
        PollIsNotStarted,
        ///Poll Is Finished
        PollIsFinished,
        /// Invalid Start Timestamp
        InvalidStartTimestamp,
        ///Invalid End Timestamp,
        InvalidEndTimestamp,
        /// Not Enough Hermes For Creating Poll
        NotEnoughHermesForCreatingPoll,
        /// Funds Already Withdrawn
        FundsAlreadyWithdrawn,
        /// Poll Is Not Finished
        PollIsNotFinished,
        /// You Are Not Creator
        YouAreNotCreator,
        /// Unauthorized
        Unauthorized,
        /// Poll Does Not Exist,
        PollDoesNotExist,
        /// Not Enough Hermes For Voting
        NotEnoughHermesForVoting,
        /// AlreadyVoted,
        AlreadyVoted,
        /// Invalid Minimum Duration Of Poll
        InvalidMinimumDurationOfPoll,
        /// Invalid Maximum Duration Of Poll
        InvalidMaximumDurationOfPoll,
        /// NotVoted
        NotVoted,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Vote for some option
        #[transactional]
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::vote())]
        pub fn vote(
            origin: OriginFor<T>,
            poll_id: H256,
            voting_option: VotingOption,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            let current_timestamp = Timestamp::<T>::get();
            let hermes_poll_info =
                <HermesPollData<T>>::get(&poll_id).ok_or(Error::<T>::PollDoesNotExist)?;

            ensure!(
                current_timestamp >= hermes_poll_info.poll_start_timestamp,
                Error::<T>::PollIsNotStarted
            );

            ensure!(
                current_timestamp <= hermes_poll_info.poll_end_timestamp,
                Error::<T>::PollIsFinished
            );

            ensure!(
                MinimumHermesVotingAmount::<T>::get()
                    <= Assets::<T>::free_balance(&T::HermesAssetId::get().into(), &user)
                        .unwrap_or(0),
                Error::<T>::NotEnoughHermesForVoting
            );

            ensure!(
                !<HermesVotings<T>>::contains_key(&poll_id, &user),
                Error::<T>::AlreadyVoted
            );

            let hermes_voting_info = HermesVotingInfo {
                voting_option,
                number_of_hermes: MinimumHermesVotingAmount::<T>::get(),
                hermes_withdrawn: false,
            };

            // Transfer Hermes to pallet
            Assets::<T>::transfer_from(
                &T::HermesAssetId::get().into(),
                &user,
                &Self::account_id(),
                hermes_voting_info.number_of_hermes,
            )
            .map_err(|_assets_err| Error::<T>::NotEnoughHermesForVoting)?;

            // Update storage
            <HermesVotings<T>>::insert(&poll_id, &user, hermes_voting_info);

            // Emit event
            Self::deposit_event(Event::<T>::Voted(user, poll_id, voting_option));

            // Return a successful DispatchResult
            Ok(().into())
        }

        /// Create poll
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::create_poll())]
        pub fn create_poll(
            origin: OriginFor<T>,
            poll_start_timestamp: T::Moment,
            poll_end_timestamp: T::Moment,
            title: String,
            description: String,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;
            let current_timestamp = Timestamp::<T>::get();

            ensure!(
                poll_start_timestamp >= current_timestamp,
                Error::<T>::InvalidStartTimestamp
            );

            ensure!(
                poll_end_timestamp > poll_start_timestamp,
                Error::<T>::InvalidEndTimestamp
            );

            ensure!(
                (poll_end_timestamp - poll_start_timestamp) >= T::MIN_DURATION_OF_POLL,
                Error::<T>::InvalidMinimumDurationOfPoll
            );

            ensure!(
                (poll_end_timestamp - poll_start_timestamp) <= T::MAX_DURATION_OF_POLL,
                Error::<T>::InvalidMaximumDurationOfPoll
            );

            ensure!(
                MinimumHermesAmountForCreatingPoll::<T>::get()
                    <= Assets::<T>::free_balance(&T::HermesAssetId::get().into(), &user)
                        .unwrap_or(0),
                Error::<T>::NotEnoughHermesForCreatingPoll
            );

            let nonce = frame_system::Pallet::<T>::account_nonce(&user);
            let encoded: [u8; 32] = (&user, nonce).using_encoded(blake2_256);
            let poll_id = H256::from(encoded);

            let hermes_poll_info = HermesPollInfo {
                creator: user.clone(),
                hermes_locked: MinimumHermesAmountForCreatingPoll::<T>::get(),
                poll_start_timestamp,
                poll_end_timestamp,
                title: title.clone(),
                description,
                creator_hermes_withdrawn: false,
            };

            // Transfer Hermes to pallet
            Assets::<T>::transfer_from(
                &T::HermesAssetId::get().into(),
                &user.clone(),
                &Self::account_id(),
                hermes_poll_info.hermes_locked,
            )
            .map_err(|_assets_err| Error::<T>::NotEnoughHermesForCreatingPoll)?;

            <HermesPollData<T>>::insert(&poll_id, hermes_poll_info);

            // Emit event
            Self::deposit_event(Event::<T>::Created(
                user.clone(),
                title,
                poll_start_timestamp,
                poll_end_timestamp,
            ));

            // Return a successful DispatchResult
            Ok(().into())
        }

        /// Withdraw funds voter
        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::withdraw_funds_voter())]
        pub fn withdraw_funds_voter(
            origin: OriginFor<T>,
            poll_id: H256,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;
            let current_timestamp = Timestamp::<T>::get();
            let hermes_poll_info =
                <HermesPollData<T>>::get(&poll_id).ok_or(Error::<T>::PollDoesNotExist)?;

            ensure!(
                current_timestamp > hermes_poll_info.poll_end_timestamp,
                Error::<T>::PollIsNotFinished
            );

            let mut hermes_voting_info =
                <HermesVotings<T>>::get(&poll_id, &user).ok_or(Error::<T>::NotVoted)?;

            ensure!(
                hermes_voting_info.hermes_withdrawn == false,
                Error::<T>::FundsAlreadyWithdrawn
            );

            // Withdraw Hermes
            Assets::<T>::transfer_from(
                &T::HermesAssetId::get().into(),
                &Self::account_id(),
                &user,
                hermes_voting_info.number_of_hermes,
            )?;

            hermes_voting_info.hermes_withdrawn = true;
            <HermesVotings<T>>::insert(&poll_id, &user, &hermes_voting_info);

            // Emit event
            Self::deposit_event(Event::<T>::VoterFundsWithdrawn(
                user,
                hermes_voting_info.number_of_hermes,
            ));

            // Return a successful DispatchResult
            Ok(().into())
        }

        /// Withdraw funds creator
        #[pallet::call_index(3)]
        #[pallet::weight(<T as Config>::WeightInfo::withdraw_funds_creator())]
        pub fn withdraw_funds_creator(
            origin: OriginFor<T>,
            poll_id: H256,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;
            let current_timestamp = Timestamp::<T>::get();
            let mut hermes_poll_info =
                <HermesPollData<T>>::get(&poll_id).ok_or(Error::<T>::PollDoesNotExist)?;

            ensure!(
                hermes_poll_info.creator == user,
                Error::<T>::YouAreNotCreator
            );

            ensure!(
                current_timestamp > hermes_poll_info.poll_end_timestamp,
                Error::<T>::PollIsNotFinished
            );

            ensure!(
                hermes_poll_info.creator_hermes_withdrawn == false,
                Error::<T>::FundsAlreadyWithdrawn
            );

            // Withdraw Creator Hermes
            Assets::<T>::transfer_from(
                &T::HermesAssetId::get().into(),
                &Self::account_id(),
                &user,
                hermes_poll_info.hermes_locked,
            )?;

            hermes_poll_info.creator_hermes_withdrawn = true;
            <HermesPollData<T>>::insert(&poll_id, &hermes_poll_info);

            // Emit event
            Self::deposit_event(Event::<T>::CreatorFundsWithdrawn(
                user,
                hermes_poll_info.hermes_locked,
            ));

            // Return a successful DispatchResult
            Ok(().into())
        }

        /// Change minimum Hermes for voting
        #[pallet::call_index(4)]
        #[pallet::weight(<T as Config>::WeightInfo::change_min_hermes_for_voting())]
        pub fn change_min_hermes_for_voting(
            origin: OriginFor<T>,
            hermes_amount: Balance,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            ensure!(
                user == AuthorityAccount::<T>::get(),
                Error::<T>::Unauthorized
            );

            MinimumHermesVotingAmount::<T>::put(hermes_amount);

            // Emit event
            Self::deposit_event(Event::MinimumHermesForVotingChanged(hermes_amount));

            // Return a successful DispatchResult
            Ok(().into())
        }

        /// Change minimum Hermes for creating a poll
        #[pallet::call_index(5)]
        #[pallet::weight(<T as Config>::WeightInfo::change_min_hermes_for_creating_poll())]
        pub fn change_min_hermes_for_creating_poll(
            origin: OriginFor<T>,
            hermes_amount: Balance,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            ensure!(
                user == AuthorityAccount::<T>::get(),
                Error::<T>::Unauthorized
            );

            MinimumHermesAmountForCreatingPoll::<T>::put(hermes_amount);

            // Emit event
            Self::deposit_event(Event::MinimumHermesForCreatingPollChanged(hermes_amount));

            // Return a successful DispatchResult
            Ok(().into())
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    impl<T: Config> Pallet<T> {
        /// The account ID of pallet
        fn account_id() -> T::AccountId {
            PALLET_ID.into_account_truncating()
        }
    }
}
