#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};

#[derive(Encode, Decode, Default, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct ILOInfo<Balance, AccountId, BlockNumber> {
    ilo_organizer: AccountId,
    number_of_tokens: Balance,
    ilo_price: Balance,
    soft_cap: Balance,
    hard_cap: Balance,
    min_contribution: Balance,
    max_contribution: Balance,
    refund_type: bool,
    liquidity_percent: Balance,
    listing_price: Balance,
    lockup_days: u32,
    start_block: BlockNumber,
    end_block: BlockNumber,
    token_vesting: VestingInfo<Balance, BlockNumber>,
    sold_tokens: Balance,
    funds_raised: Balance,
    succeeded: bool,
    failed: bool,
    lp_tokens: Balance,
}

#[derive(Encode, Decode, Default, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct VestingInfo<Balance, BlockNumber> {
    first_release_percent: Balance,
    vesting_period: BlockNumber,
    vesting_percent: Balance,
}

#[derive(Encode, Decode, Default, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct ContributionInfo<Balance> {
    funds_contributed: Balance,
    tokens_bought: Balance,
    tokens_claimed: Balance,
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use crate::{ContributionInfo, ILOInfo, VestingInfo};
    use common::balance;
    use common::prelude::{Balance, FixedWrapper};
    use frame_support::pallet_prelude::*;
    use frame_system::ensure_signed;
    use frame_system::pallet_prelude::*;
    use sp_runtime::traits::AccountIdConversion;
    use sp_runtime::ModuleId;

    const PALLET_ID: ModuleId = ModuleId(*b"crslaunc");

    #[pallet::config]
    pub trait Config: frame_system::Config + assets::Config {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        /// Ceres asset id
        type CeresAssetId: Get<AssetId>;
    }

    type Assets<T> = assets::Pallet<T>;
    pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
    type AssetIdOf<T> = <T as assets::Config>::AssetId;
    type AssetId = common::AssetId32<common::PredefinedAssetId>;

    #[pallet::pallet]
    #[pallet::generate_store(pub (super) trait Store)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::storage]
    #[pallet::getter(fn ilos)]
    pub type ILOs<T: Config> = StorageMap<
        _,
        Identity,
        AssetIdOf<T>,
        ILOInfo<Balance, AccountIdOf<T>, T::BlockNumber>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn contributions)]
    pub type Contributions<T: Config> = StorageDoubleMap<
        _,
        Identity,
        AssetIdOf<T>,
        Identity,
        AccountIdOf<T>,
        ContributionInfo<Balance>,
        ValueQuery,
    >;

    #[pallet::event]
    #[pallet::metadata(AccountIdOf<T> = "AccountId", AssetIdOf<T> = "AssetId", BalanceOf<T> = "Balance", T::BlockNumber = "BlockNumber")]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// ILO created [who, what]
        ILOCreated(AccountIdOf<T>, AssetIdOf<T>),
        /// Contribute [who, what, balance]
        Contribute(AccountIdOf<T>, AssetIdOf<T>, Balance),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// ILO for token already exists
        ILOAlreadyExists,
        /// Parameter can't be zero
        ParameterCantBeZero,
        /// Soft cap should be minimum 50% of hard cap
        InvalidSoftCap,
        /// Minimum contribution must be equal or greater than 0.01 XOR
        InvalidMinimumContribution,
        /// Maximum contribution must be greater than minimum contribution
        InvalidMaximumContribution,
        /// Minimum 51% of raised funds must go to liquidity
        InvalidLiquidityPercent,
        /// Lockup days must be minimum 30
        InvalidLockupDays,
        /// Start block must be in future
        InvalidStartBlock,
        /// End block must be greater than start block
        InvalidEndBlock,
        /// Listing price must be greater than ILO price
        InvalidPrice,
        /// First release percent can't be zero
        InvalidFirstReleasePercent,
        /// Invalid vesting percent
        InvalidVestingPercent,
        /// Vesting period can't be zero
        InvalidVestingPeriod,
        /// Not enough CERES
        NotEnoughCeres,
        /// Not enough ILO tokens
        NotEnoughTokens,
        ///ILONotStarted
        ILONotStarted,
        ///CantContributeInILO
        CantContributeInILO,
        ///HardCapIsHit
        HardCapIsHit,
        ///NotEnoughTokensToBuy
        NotEnoughTokensToBuy,
        ///ContributionIsLowerThenMin
        ContributionIsLowerThenMin,
        ///ContributionIsBiggerThenMax
        ContributionIsBiggerThenMax,
        ///ILODoesNotExist
        ILODoesNotExist,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Create ILO
        #[pallet::weight(10000)]
        pub fn create_ilo(
            origin: OriginFor<T>,
            asset_id: AssetIdOf<T>,
            number_of_tokens: Balance,
            ilo_price: Balance,
            soft_cap: Balance,
            hard_cap: Balance,
            min_contribution: Balance,
            max_contribution: Balance,
            refund_type: bool,
            liquidity_percent: Balance,
            listing_price: Balance,
            lockup_days: u32,
            start_block: T::BlockNumber,
            end_block: T::BlockNumber,
            first_release_percent: Balance,
            vesting_period: T::BlockNumber,
            vesting_percent: Balance,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin.clone())?;

            // Get ILO info of asset_id token
            let mut ilo_info = <ILOs<T>>::get(&asset_id);

            // Check if ILO for token already exists
            ensure!(ilo_info.ilo_price == 0, Error::<T>::ILOAlreadyExists);

            // Get current block
            let current_block = frame_system::Pallet::<T>::block_number();

            // Check parameters
            let result = Self::check_parameters(
                ilo_price,
                soft_cap,
                hard_cap,
                min_contribution,
                max_contribution,
                liquidity_percent,
                listing_price,
                lockup_days,
                start_block,
                end_block,
                current_block,
                first_release_percent,
                vesting_period,
                vesting_percent,
            );

            if result.is_err() {
                return Err(result.err().unwrap().into());
            }

            ensure!(
                balance!(10)
                    <= Assets::<T>::free_balance(&T::CeresAssetId::get().into(), &user)
                        .unwrap_or(0),
                Error::<T>::NotEnoughCeres
            );

            ensure!(
                number_of_tokens <= Assets::<T>::free_balance(&asset_id, &user).unwrap_or(0),
                Error::<T>::NotEnoughTokens
            );

            // Burn 10 CERES as fee
            Assets::<T>::burn(origin, T::CeresAssetId::get().into(), balance!(10))?;

            // Transfer ILO tokens to pallet
            Assets::<T>::transfer_from(
                &asset_id.into(),
                &user,
                &Self::account_id(),
                number_of_tokens,
            )?;

            ilo_info = ILOInfo {
                ilo_organizer: user.clone(),
                number_of_tokens,
                ilo_price,
                soft_cap,
                hard_cap,
                min_contribution,
                max_contribution,
                refund_type,
                liquidity_percent,
                listing_price,
                lockup_days,
                start_block,
                end_block,
                token_vesting: VestingInfo {
                    first_release_percent,
                    vesting_period,
                    vesting_percent,
                },
                sold_tokens: balance!(0),
                funds_raised: balance!(0),
                succeeded: false,
                failed: false,
                lp_tokens: balance!(0),
            };

            <ILOs<T>>::insert(&asset_id, &ilo_info);

            // Emit an event
            Self::deposit_event(Event::ILOCreated(user, asset_id));

            // Return a successful DispatchResult
            Ok(().into())
        }

        /// Contribute
        #[pallet::weight(10000)]
        pub fn contribute(
            origin: OriginFor<T>,
            asset_id: AssetIdOf<T>,
            funds_to_contribute: Balance,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;
            let current_block = frame_system::Pallet::<T>::block_number();

            // get ILO info
            let mut ilo_info = <ILOs<T>>::get(&asset_id);

            // Check if ILO for token exists
            ensure!(ilo_info.ilo_price != 0, Error::<T>::ILODoesNotExist);

            // Get contribution info
            let contribute_info = <Contributions<T>>::get(&asset_id, &user);

            ensure!(
                ilo_info.start_block >= current_block,
                Error::<T>::ILONotStarted
            );
            ensure!(
                ilo_info.end_block > ilo_info.start_block,
                Error::<T>::CantContributeInILO
            );
            ensure!(
                funds_to_contribute >= ilo_info.min_contribution,
                Error::<T>::ContributionIsLowerThenMin
            );
            ensure!(
                funds_to_contribute <= ilo_info.max_contribution,
                Error::<T>::ContributionIsBiggerThenMax
            );
            ensure!(
                ilo_info.funds_raised + funds_to_contribute <= ilo_info.hard_cap,
                Error::<T>::HardCapIsHit
            );
            ensure!(
                ilo_info.sold_tokens + funds_to_contribute <= ilo_info.number_of_tokens,
                Error::<T>::NotEnoughTokensToBuy
            );

            // Calculate amount of bought tokens
            let mut tokens_bought = (FixedWrapper::from(funds_to_contribute)
                / FixedWrapper::from(ilo_info.ilo_price))
            .try_into_balance()
            .unwrap_or(0);

            tokens_bought += funds_to_contribute;
            ilo_info.funds_raised += tokens_bought;
            ilo_info.sold_tokens += tokens_bought;

            // Transfer ILO tokens to pallet
            Assets::<T>::transfer_from(
                &asset_id.into(),
                &user,
                &Self::account_id(),
                tokens_bought,
            )?;

            // Update storage
            <ILOs<T>>::insert(&asset_id, &ilo_info);
            <Contributions<T>>::insert(&asset_id, &user, contribute_info);

            // Emit event
            Self::deposit_event(Event::<T>::Contribute(user, asset_id, funds_to_contribute));

            // Return a successful DispatchResult
            Ok(().into())
        }
    }

    impl<T: Config> Pallet<T> {
        /// The account ID of pallet
        fn account_id() -> T::AccountId {
            PALLET_ID.into_account()
        }

        /// Check parameters
        fn check_parameters(
            ilo_price: Balance,
            soft_cap: Balance,
            hard_cap: Balance,
            min_contribution: Balance,
            max_contribution: Balance,
            liquidity_percent: Balance,
            listing_price: Balance,
            lockup_days: u32,
            start_block: T::BlockNumber,
            end_block: T::BlockNumber,
            current_block: T::BlockNumber,
            first_release_percent: Balance,
            vesting_period: T::BlockNumber,
            vesting_percent: Balance,
        ) -> Result<(), DispatchError> {
            if ilo_price == balance!(0) {
                return Err(Error::<T>::ParameterCantBeZero.into());
            }

            if hard_cap == balance!(0) {
                return Err(Error::<T>::ParameterCantBeZero.into());
            }

            let min_soft_cap = (FixedWrapper::from(hard_cap) * FixedWrapper::from(0.5))
                .try_into_balance()
                .unwrap_or(0);
            if soft_cap < min_soft_cap {
                return Err(Error::<T>::InvalidSoftCap.into());
            }

            if min_contribution < balance!(0.01) {
                return Err(Error::<T>::InvalidMinimumContribution.into());
            }

            if min_contribution < max_contribution {
                return Err(Error::<T>::InvalidMaximumContribution.into());
            }

            if liquidity_percent < balance!(0.51) {
                return Err(Error::<T>::InvalidLiquidityPercent.into());
            }

            if lockup_days < 30 {
                return Err(Error::<T>::InvalidLockupDays.into());
            }

            if start_block <= current_block {
                return Err(Error::<T>::InvalidStartBlock.into());
            }

            if start_block >= end_block {
                return Err(Error::<T>::InvalidEndBlock.into());
            }

            if ilo_price >= listing_price {
                return Err(Error::<T>::InvalidPrice.into());
            }

            if first_release_percent == balance!(0) {
                return Err(Error::<T>::InvalidFirstReleasePercent.into());
            }

            if first_release_percent != balance!(1) && vesting_percent == balance!(0) {
                return Err(Error::<T>::InvalidVestingPercent.into());
            }

            if first_release_percent + vesting_percent > balance!(1) {
                return Err(Error::<T>::InvalidVestingPercent.into());
            }

            if first_release_percent != balance!(1) && vesting_period == 0u32.into() {
                return Err(Error::<T>::InvalidVestingPeriod.into());
            }

            Ok(().into())
        }
    }
}
