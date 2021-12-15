#![cfg_attr(not(feature = "std"), no_std)]
#![feature(destructuring_assignment)]

use codec::{Decode, Encode};

#[derive(Encode, Decode, Default, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct VotingInfo<Balance> {
    /// Voting option
    voting_option: u32,
    /// Number of votes
    number_of_votes: Balance,
    /// Ceres withdrawn
    ceres_withdrawn: bool,
}

#[derive(Encode, Decode, Default, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct PollInfo<BlockNumber> {
    /// Number of options
    number_of_options: u32,
    /// Poll end block
    poll_end_block: BlockNumber,
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use crate::{PollInfo, VotingInfo};
    use common::prelude::Balance;
    use frame_support::pallet_prelude::*;
    use frame_system::ensure_signed;
    use frame_system::pallet_prelude::*;
    use sp_runtime::traits::AccountIdConversion;
    use sp_runtime::ModuleId;

    const PALLET_ID: ModuleId = ModuleId(*b"governac");

    #[pallet::config]
    pub trait Config: frame_system::Config + assets::Config {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        /// Ceres asset id
        type CeresAssetId: Get<Self::AssetId>;
    }

    type Assets<T> = assets::Pallet<T>;
    pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

    #[pallet::pallet]
    #[pallet::generate_store(pub (super) trait Store)]
    pub struct Pallet<T>(PhantomData<T>);

    /// Poll_id -> Account_id -> Vec<VotingInfo>
    #[pallet::storage]
    #[pallet::getter(fn votings)]
    pub type Voting<T: Config> = StorageDoubleMap<
        _,
        Identity,
        Vec<u8>,
        Identity,
        AccountIdOf<T>,
        Vec<VotingInfo<Balance>>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn poll_data)]
    pub type PollData<T: Config> =
        StorageMap<_, Identity, Vec<u8>, PollInfo<T::BlockNumber>, ValueQuery>;

    #[pallet::event]
    #[pallet::metadata(AccountIdOf<T> = "AccountId")]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Voting [who, option, balance]
        Voted(AccountIdOf<T>, u32, Balance),
        /// Create poll [who, option, end_block]
        Cretaed(AccountIdOf<T>, u32, T::BlockNumber),
        /// Withdrawn [who, balance]
        Withdrawn(AccountIdOf<T>, Balance),
    }

    #[pallet::error]
    pub enum Error<T> {
        ///Invalid Votes
        InvalidVotes,
        ///Invalid End Block
        InvalidEndBlock,
        ///Not Enough Funds
        NotEnoughFunds,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Voting for option
        #[pallet::weight(10000)]
        pub fn vote(
            origin: OriginFor<T>,
            poll_id: Vec<u8>,
            voting_option: u32,
            number_of_votes: Balance,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            ensure!(number_of_votes > 0, Error::<T>::InvalidVotes);

            let poll_info = PollData::<T>::get(&poll_id);
            let current_block = frame_system::Pallet::<T>::block_number();

            ensure!(
                current_block <= poll_info.poll_end_block,
                Error::<T>::InvalidEndBlock
            );

            let mut vote_info = VotingInfo {
                voting_option,
                number_of_votes: 0,
                ceres_withdrawn: false,
            };

            ensure!(
                number_of_votes
                    <= Assets::<T>::free_balance(&T::CeresAssetId::get(), &Self::account_id())
                        .unwrap_or(0),
                Error::<T>::NotEnoughFunds
            );

            vote_info.number_of_votes = number_of_votes;

            // Transfer Ceres to pallet
            Assets::<T>::transfer_from(
                &T::CeresAssetId::get().into(),
                &user,
                &Self::account_id(),
                number_of_votes,
            )?;

            //Update storage
            <Voting<T>>::append(&poll_id, &user, vote_info);

            //Emit event
            Self::deposit_event(Event::<T>::Voted(user, voting_option, number_of_votes));

            // Return a successful DispatchResult
            Ok(().into())
        }

        /// Create Poll
        #[pallet::weight(10000)]
        pub fn create_poll(
            origin: OriginFor<T>,
            poll_id: Vec<u8>,
            number_of_option: u32,
            pool_end_block: T::BlockNumber,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            let current_block = frame_system::Pallet::<T>::block_number();
            ensure!(pool_end_block > current_block, Error::<T>::InvalidEndBlock);

            let poll_info = PollInfo {
                number_of_options: number_of_option,
                poll_end_block: pool_end_block,
            };

            <PollData<T>>::insert(&poll_id, poll_info);

            //Emit event
            Self::deposit_event(Event::<T>::Cretaed(user, number_of_option, pool_end_block));

            // Return a successful DispatchResult
            Ok(().into())
        }

        /// Withdraw
        #[pallet::weight(10000)]
        pub fn withdraw(origin: OriginFor<T>, poll_id: Vec<u8>) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            let poll_info = PollData::<T>::get(&poll_id);
            let current_block = frame_system::Pallet::<T>::block_number();

            ensure!(
                current_block >= poll_info.poll_end_block,
                Error::<T>::InvalidEndBlock
            );

            //Update storage
            let mut votes = <Voting<T>>::get(&poll_id, &user);
            let mut total_votes = 0;
            for vote in votes.iter_mut() {
                total_votes += vote.number_of_votes;
                vote.ceres_withdrawn = true;
            }

            // Withdraw CERES
            Assets::<T>::transfer_from(
                &T::CeresAssetId::get().into(),
                &Self::account_id(),
                &user,
                total_votes,
            )?;

            //Emit event
            Self::deposit_event(Event::<T>::Withdrawn(user, total_votes));

            // Return a successful DispatchResult
            Ok(().into())
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    impl<T: Config> Pallet<T> {
        /// The account ID of pallet
        fn account_id() -> T::AccountId {
            PALLET_ID.into_account()
        }
    }
}
