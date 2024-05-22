#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::type_complexity)]

pub mod migrations;
pub mod weights;

mod benchmarking;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

use codec::{Decode, Encode};
use common::{Balance, BoundedString};
use frame_support::BoundedVec;
pub use weights::WeightInfo;

#[derive(Encode, Decode, Default, PartialEq, Eq, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct VotingInfo {
    /// Voting option
    voting_option: u32,
    /// Number of votes
    number_of_votes: Balance,
    /// Asset withdrawn
    asset_withdrawn: bool,
}

#[derive(Encode, Decode, Default, PartialEq, Eq, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
#[scale_info(skip_type_params(StringLimit, OptionsLimit, TitleLimit, DescriptionLimit))]
pub struct PollInfo<
    AssetId,
    Moment,
    StringLimit: sp_core::Get<u32>,
    OptionsLimit: sp_core::Get<u32>,
    TitleLimit: sp_core::Get<u32>,
    DescriptionLimit: sp_core::Get<u32>,
> {
    /// Asset id
    pub poll_asset: AssetId,
    /// Poll start timestamp
    pub poll_start_timestamp: Moment,
    /// Poll end timestamp
    pub poll_end_timestamp: Moment,
    /// Poll title
    pub title: BoundedString<TitleLimit>,
    /// Description
    pub description: BoundedString<DescriptionLimit>,
    /// Options
    pub options: BoundedVec<BoundedString<StringLimit>, OptionsLimit>,
}

/// Storage version.
#[derive(Encode, Decode, Eq, PartialEq, Debug, scale_info::TypeInfo)]
pub enum StorageVersion {
    /// Initial version
    V1,
    /// After migrating to timestamp calculation
    V2,
    /// After migrating to open governance
    V3,
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use crate::{migrations, PollInfo, StorageVersion, VotingInfo, WeightInfo};
    use common::prelude::Balance;
    use common::BoundedString;
    use common::{AssetIdOf, AssetManager};
    use frame_support::log;
    use frame_support::pallet_prelude::OptionQuery;
    use frame_support::pallet_prelude::ValueQuery;
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
    use sp_std::collections::btree_set::BTreeSet;

    const PALLET_ID: PalletId = PalletId(*b"ceresgov");

    #[pallet::config]
    pub trait Config: frame_system::Config + technical::Config + timestamp::Config {
        /// String limit
        type StringLimit: Get<u32>;

        /// Options limit
        type OptionsLimit: Get<u32>;

        /// Title limit
        type TitleLimit: Get<u32>;

        /// Description limit
        type DescriptionLimit: Get<u32>;

        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

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
        StorageDoubleMap<_, Identity, H256, Identity, AccountIdOf<T>, VotingInfo, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn poll_data)]
    pub type PollData<T: Config> = StorageMap<
        _,
        Identity,
        H256,
        PollInfo<
            AssetIdOf<T>,
            T::Moment,
            T::StringLimit,
            T::OptionsLimit,
            T::TitleLimit,
            T::DescriptionLimit,
        >,
        OptionQuery,
    >;

    #[pallet::type_value]
    pub fn DefaultForPalletStorageVersion<T: Config>() -> StorageVersion {
        StorageVersion::V1
    }

    /// Pallet storage version
    #[pallet::storage]
    #[pallet::getter(fn pallet_storage_version)]
    pub type PalletStorageVersion<T: Config> =
        StorageValue<_, StorageVersion, ValueQuery, DefaultForPalletStorageVersion<T>>;

    #[pallet::type_value]
    pub fn DefaultForAuthorityAccount<T: Config>() -> AccountIdOf<T> {
        let bytes = hex!("96ea3c9c0be7bbc7b0656a1983db5eed75210256891a9609012362e36815b132");
        AccountIdOf::<T>::decode(&mut &bytes[..]).unwrap()
    }

    /// Account which has permissions for creating a poll
    #[pallet::storage]
    #[pallet::getter(fn authority_account)]
    pub type AuthorityAccount<T: Config> =
        StorageValue<_, AccountIdOf<T>, ValueQuery, DefaultForAuthorityAccount<T>>;

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Voting [who, poll, option, asset, balance]
        Voted(AccountIdOf<T>, H256, u32, AssetIdOf<T>, Balance),
        /// Create poll [who, title, poll_asset, start_timestamp, end_timestamp]
        Created(
            AccountIdOf<T>,
            BoundedString<T::TitleLimit>,
            AssetIdOf<T>,
            T::Moment,
            T::Moment,
        ),
        /// Withdrawn [who, poll, asset, balance]
        Withdrawn(AccountIdOf<T>, H256, AssetIdOf<T>, Balance),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Poll is finished
        PollIsFinished,
        /// Poll is not started
        PollIsNotStarted,
        /// Not enough funds
        NotEnoughFunds,
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
        /// Invalid voting options
        InvalidVotingOptions,
        /// Too many voting options
        TooManyVotingOptions,
        /// Duplicate options
        DuplicateOptions,
        /// Poll does not exist
        PollDoesNotExist,
        /// Invalid option
        InvalidOption,
        /// Not voted
        NotVoted,
        /// Unauthorized
        Unauthorized,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Voting for option
        #[transactional]
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::vote())]
        pub fn vote(
            origin: OriginFor<T>,
            poll_id: H256,
            voting_option: u32,
            number_of_votes: Balance,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            ensure!(number_of_votes > 0, Error::<T>::InvalidNumberOfVotes);

            let poll_info = <PollData<T>>::get(poll_id).ok_or(Error::<T>::PollDoesNotExist)?;
            let current_timestamp = Timestamp::<T>::get();

            ensure!(
                current_timestamp >= poll_info.poll_start_timestamp,
                Error::<T>::PollIsNotStarted
            );

            ensure!(
                current_timestamp <= poll_info.poll_end_timestamp,
                Error::<T>::PollIsFinished
            );

            // Check if voting option is valid, if not return error
            let number_of_options = poll_info.options.len() as u32;
            ensure!(
                voting_option <= number_of_options && voting_option > 0u32,
                Error::<T>::InvalidOption
            );

            // If already voted for one option, then can't vote for another option. But he can increase the number of votes on first option
            if let Some(mut voting_info) = <Voting<T>>::get(poll_id, &user) {
                ensure!(
                    voting_info.voting_option == voting_option,
                    Error::<T>::VoteDenied
                );
                voting_info.number_of_votes += number_of_votes;
                <Voting<T>>::insert(poll_id, &user, voting_info);
            } else {
                let new_voting_info = VotingInfo {
                    voting_option,
                    number_of_votes,
                    asset_withdrawn: false,
                };
                <Voting<T>>::insert(poll_id, &user, new_voting_info);
            }

            // Transfer asset to pallet
            T::AssetManager::transfer_from(
                &poll_info.poll_asset,
                &user,
                &Self::account_id(),
                number_of_votes,
            )
            .map_err(|_assets_err| Error::<T>::NotEnoughFunds)?;

            //Emit event
            Self::deposit_event(Event::<T>::Voted(
                user,
                poll_id,
                voting_option,
                poll_info.poll_asset,
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
            poll_asset: AssetIdOf<T>,
            poll_start_timestamp: T::Moment,
            poll_end_timestamp: T::Moment,
            title: BoundedString<T::TitleLimit>,
            description: BoundedString<T::DescriptionLimit>,
            options: BoundedVec<BoundedString<T::StringLimit>, T::OptionsLimit>,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;
            let current_timestamp = Timestamp::<T>::get();

            if user != AuthorityAccount::<T>::get() {
                return Err(Error::<T>::Unauthorized.into());
            }

            ensure!(
                poll_start_timestamp >= current_timestamp,
                Error::<T>::InvalidStartTimestamp
            );

            ensure!(
                poll_end_timestamp > poll_start_timestamp,
                Error::<T>::InvalidEndTimestamp
            );

            let nonce = frame_system::Pallet::<T>::account_nonce(&user);
            let encoded: [u8; 32] = (&user, nonce).using_encoded(blake2_256);
            let poll_id = H256::from(encoded);

            let options_len = options.len();
            if options_len < 2 {
                return Err(Error::<T>::InvalidVotingOptions.into());
            }
            if options_len > 5 {
                return Err(Error::<T>::TooManyVotingOptions.into());
            }

            let options_set = BTreeSet::from_iter(&options);
            if options_set.len() != options_len {
                return Err(Error::<T>::DuplicateOptions.into());
            }

            let poll_info = PollInfo {
                poll_asset,
                poll_start_timestamp,
                poll_end_timestamp,
                title: title.clone(),
                description,
                options,
            };

            <PollData<T>>::insert(poll_id, poll_info);

            //Emit event
            Self::deposit_event(Event::<T>::Created(
                user,
                title.clone(),
                poll_asset,
                poll_start_timestamp,
                poll_end_timestamp,
            ));

            // Return a successful DispatchResult
            Ok(().into())
        }

        /// Withdraw voting funds
        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::withdraw())]
        pub fn withdraw(origin: OriginFor<T>, poll_id: H256) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            let poll_info = <PollData<T>>::get(poll_id).ok_or(Error::<T>::PollDoesNotExist)?;
            let current_timestamp = Timestamp::<T>::get();

            ensure!(
                current_timestamp > poll_info.poll_end_timestamp,
                Error::<T>::PollIsNotFinished
            );

            // Update storage
            let mut voting_info = <Voting<T>>::get(poll_id, &user).ok_or(Error::<T>::NotVoted)?;
            ensure!(
                !voting_info.asset_withdrawn,
                Error::<T>::FundsAlreadyWithdrawn
            );

            // Withdraw asset
            T::AssetManager::transfer_from(
                &poll_info.poll_asset,
                &Self::account_id(),
                &user,
                voting_info.number_of_votes,
            )?;

            voting_info.asset_withdrawn = true;
            <Voting<T>>::insert(poll_id, &user, &voting_info);

            //Emit event
            Self::deposit_event(Event::<T>::Withdrawn(
                user,
                poll_id,
                poll_info.poll_asset,
                voting_info.number_of_votes,
            ));

            // Return a successful DispatchResult
            Ok(().into())
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_runtime_upgrade() -> Weight {
            if Self::pallet_storage_version() == StorageVersion::V2 {
                sp_runtime::runtime_logger::RuntimeLogger::init();
                log::info!(
                    "Applying migration to version 2: Migrating to open governance - version 3"
                );

                if let Err(err) = common::with_transaction(migrations::migrate::<T>) {
                    log::error!("Failed to migrate: {}", err);
                } else {
                    PalletStorageVersion::<T>::put(StorageVersion::V3);
                }
                <T as frame_system::Config>::BlockWeights::get().max_block
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
