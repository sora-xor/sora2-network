#![cfg_attr(not(feature = "std"), no_std)]
#![feature(destructuring_assignment)]

use codec::{Decode, Encode};

#[derive(Encode, Decode, Default, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct PoolInfo<Balance> {
    multiplier: u32,
    deposit_fee: Balance,
    is_core: bool,
    is_farm: bool,
    total_tokens_in_pool: Balance,
    rewards: Balance,
    is_removed: bool,
}

#[derive(Encode, Decode, Default, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct TokenInfo<Balance> {
    farms_total_multiplier: u32,
    staking_total_multiplier: u32,
    token_per_block: Balance,
    farms_allocation: Balance,
    staking_allocation: Balance,
    team_allocation: Balance,
}

#[derive(Encode, Decode, Default, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct UserInfo<Balance, AssetId> {
    pool_asset: AssetId,
    reward_asset: AssetId,
    is_farm: bool,
    pooled_tokens: Balance,
    rewards: Balance,
}

#[frame_support::pallet]
pub mod pallet {
    use crate::{PoolInfo, TokenInfo, UserInfo};
    use common::prelude::Balance;
    use frame_support::pallet_prelude::*;
    use frame_support::traits::Vec;
    use frame_system::pallet_prelude::*;
    use sp_runtime::ModuleId;

    const PALLET_ID: ModuleId = ModuleId(*b"dmtrfarm");

    #[pallet::config]
    pub trait Config: frame_system::Config + assets::Config + technical::Config {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
    }

    type Assets<T> = assets::Pallet<T>;
    pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
    type AssetIdOf<T> = <T as assets::Config>::AssetId;

    #[pallet::pallet]
    #[pallet::generate_store(pub (super) trait Store)]
    pub struct Pallet<T>(PhantomData<T>);

    /// A vote of a particular user for a particular poll
    #[pallet::storage]
    #[pallet::getter(fn token_info)]
    pub type TokenInfos<T: Config> =
        StorageMap<_, Identity, AssetIdOf<T>, TokenInfo<Balance>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn user_info)]
    pub type UserInfos<T: Config> =
        StorageMap<_, Identity, AccountIdOf<T>, Vec<UserInfo<Balance, AssetIdOf<T>>>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn pools)]
    pub type Pools<T: Config> = StorageDoubleMap<
        _,
        Identity,
        AssetIdOf<T>,
        Identity,
        AssetIdOf<T>,
        Vec<PoolInfo<Balance>>,
        ValueQuery,
    >;

    #[pallet::event]
    #[pallet::metadata(AccountIdOf<T> = "AccountId", BalanceOf<T> = "Balance", T::BlockNumber = "BlockNumber")]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {}

    #[pallet::error]
    pub enum Error<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {}

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    impl<T: Config> Pallet<T> {}
}
