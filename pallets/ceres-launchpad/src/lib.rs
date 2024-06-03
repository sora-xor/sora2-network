#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::large_enum_variant)]
#![allow(clippy::type_complexity)]

pub mod weights;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

mod benchmarking;

use codec::{Decode, Encode};
use common::TradingPairSourceManager;
pub use weights::WeightInfo;

#[derive(Encode, Decode, Default, PartialEq, Eq, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct ILOInfo<Balance, AccountId, Moment, AssetId> {
    ilo_organizer: AccountId,
    tokens_for_ilo: Balance,
    tokens_for_liquidity: Balance,
    ilo_price: Balance,
    soft_cap: Balance,
    hard_cap: Balance,
    min_contribution: Balance,
    max_contribution: Balance,
    refund_type: bool,
    liquidity_percent: Balance,
    listing_price: Balance,
    lockup_days: u32,
    start_timestamp: Moment,
    end_timestamp: Moment,
    contributors_vesting: ContributorsVesting<Balance, Moment>,
    team_vesting: TeamVesting<Balance, Moment>,
    sold_tokens: Balance,
    funds_raised: Balance,
    succeeded: bool,
    failed: bool,
    lp_tokens: Balance,
    claimed_lp_tokens: bool,
    finish_timestamp: Moment,
    base_asset: AssetId,
}

#[derive(Encode, Decode, Default, PartialEq, Eq, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct TeamVesting<Balance, Moment> {
    team_vesting_total_tokens: Balance,
    team_vesting_first_release_percent: Balance,
    team_vesting_period: Moment,
    team_vesting_percent: Balance,
}

#[derive(Encode, Decode, Default, PartialEq, Eq, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct ContributorsVesting<Balance, Moment> {
    first_release_percent: Balance,
    vesting_period: Moment,
    vesting_percent: Balance,
}

#[derive(Encode, Decode, Default, PartialEq, Eq, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct ContributionInfo<Balance> {
    funds_contributed: Balance,
    tokens_bought: Balance,
    tokens_claimed: Balance,
    claiming_finished: bool,
    number_of_claims: u32,
}

pub use pallet::*;

#[frame_support::pallet]
#[allow(clippy::too_many_arguments)]
pub mod pallet {
    use super::*;
    use crate::{ContributionInfo, ContributorsVesting, ILOInfo};
    use common::fixnum::ops::RoundMode;
    use common::prelude::{Balance, FixedWrapper, XOR};
    use common::Fixed;
    use common::{balance, AssetInfoProvider, DEXId, XykPool, PSWAP, XSTUSD};
    use frame_support::pallet_prelude::*;
    use frame_support::transactional;
    use frame_support::PalletId;
    use frame_system::pallet_prelude::*;
    use frame_system::{ensure_signed, RawOrigin};
    use hex_literal::hex;
    use pallet_timestamp as timestamp;
    use sp_runtime::traits::{
        AccountIdConversion, CheckedDiv, Saturating, UniqueSaturatedInto, Zero,
    };
    use sp_std::prelude::*;

    const PALLET_ID: PalletId = PalletId(*b"crslaunc");

    // TODO: #395 use AssetInfoProvider instead of assets pallet
    #[pallet::config]
    pub trait Config:
        frame_system::Config
        + assets::Config
        + pool_xyk::Config
        + ceres_liquidity_locker::Config
        + pswap_distribution::Config
        + vested_rewards::Config
        + ceres_token_locker::Config
        + timestamp::Config
    {
        /// One day represented in milliseconds
        const MILLISECONDS_PER_DAY: Self::Moment;

        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        type TradingPairSourceManager: TradingPairSourceManager<Self::DEXId, Self::AssetId>;

        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    type Assets<T> = assets::Pallet<T>;
    pub type Timestamp<T> = timestamp::Pallet<T>;
    type PoolXYK<T> = pool_xyk::Pallet<T>;
    type CeresLiquidityLocker<T> = ceres_liquidity_locker::Pallet<T>;
    type TokenLocker<T> = ceres_token_locker::Pallet<T>;
    type PSWAPDistribution<T> = pswap_distribution::Pallet<T>;
    type VestedRewards<T> = vested_rewards::Pallet<T>;

    type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
    type AssetIdOf<T> = <T as assets::Config>::AssetId;
    type CeresAssetIdOf<T> = <T as ceres_token_locker::Config>::CeresAssetId;

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::type_value]
    pub fn DefaultForPenaltiesAccount<T: Config>() -> AccountIdOf<T> {
        let bytes = hex!("96ea3c9c0be7bbc7b0656a1983db5eed75210256891a9609012362e36815b132");
        AccountIdOf::<T>::decode(&mut &bytes[..]).unwrap()
    }

    /// Account for collecting penalties
    #[pallet::storage]
    #[pallet::getter(fn penalties_account)]
    pub type PenaltiesAccount<T: Config> =
        StorageValue<_, AccountIdOf<T>, ValueQuery, DefaultForPenaltiesAccount<T>>;

    #[pallet::type_value]
    pub fn DefaultCeresBurnFeeAmount<T: Config>() -> Balance {
        balance!(10)
    }

    /// Amount of CERES for burn fee
    #[pallet::storage]
    #[pallet::getter(fn ceres_burn_fee_amount)]
    pub type CeresBurnFeeAmount<T: Config> =
        StorageValue<_, Balance, ValueQuery, DefaultCeresBurnFeeAmount<T>>;

    #[pallet::type_value]
    pub fn DefaultCeresForContributionInILO<T: Config>() -> Balance {
        balance!(1)
    }

    /// Amount of CERES for contribution in ILO
    #[pallet::storage]
    #[pallet::getter(fn ceres_for_contribution_in_ilo)]
    pub type CeresForContributionInILO<T: Config> =
        StorageValue<_, Balance, ValueQuery, DefaultCeresForContributionInILO<T>>;

    #[pallet::type_value]
    pub fn DefaultFeePercentOnRaisedFunds<T: Config>() -> Balance {
        balance!(0.01)
    }

    /// Fee percent on raised funds in successful ILO
    #[pallet::storage]
    #[pallet::getter(fn fee_percent_on_raised_funds)]
    pub type FeePercentOnRaisedFunds<T: Config> =
        StorageValue<_, Balance, ValueQuery, DefaultFeePercentOnRaisedFunds<T>>;

    #[pallet::type_value]
    pub fn DefaultForAuthorityAccount<T: Config>() -> AccountIdOf<T> {
        let bytes = hex!("96ea3c9c0be7bbc7b0656a1983db5eed75210256891a9609012362e36815b132");
        AccountIdOf::<T>::decode(&mut &bytes[..]).unwrap()
    }

    /// Account which has permissions for changing CERES burn amount fee
    #[pallet::storage]
    #[pallet::getter(fn authority_account)]
    pub type AuthorityAccount<T: Config> =
        StorageValue<_, AccountIdOf<T>, ValueQuery, DefaultForAuthorityAccount<T>>;

    #[pallet::storage]
    #[pallet::getter(fn ilos)]
    pub type ILOs<T: Config> = StorageMap<
        _,
        Identity,
        AssetIdOf<T>,
        ILOInfo<Balance, AccountIdOf<T>, T::Moment, AssetIdOf<T>>,
        OptionQuery,
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

    #[pallet::storage]
    #[pallet::getter(fn whitelisted_contributors)]
    pub type WhitelistedContributors<T: Config> = StorageValue<_, Vec<AccountIdOf<T>>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn whitelisted_ilo_organizers)]
    pub type WhitelistedIloOrganizers<T: Config> = StorageValue<_, Vec<AccountIdOf<T>>, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// ILO created [who, what]
        ILOCreated(AccountIdOf<T>, AssetIdOf<T>),
        /// Contribute [who, what, balance]
        Contributed(AccountIdOf<T>, AssetIdOf<T>, Balance),
        /// Emergency withdraw [who, what, balance]
        EmergencyWithdrawn(AccountIdOf<T>, AssetIdOf<T>, Balance),
        /// ILO finished [who, what]
        ILOFinished(AccountIdOf<T>, AssetIdOf<T>),
        /// Claim LP Tokens [who, what]
        ClaimedLP(AccountIdOf<T>, AssetIdOf<T>),
        /// Claim tokens [who, what]
        Claimed(AccountIdOf<T>, AssetIdOf<T>),
        /// Fee changed [balance]
        FeeChanged(Balance),
        /// PSWAP claimed
        ClaimedPSWAP(),
        /// Contributor whitelisted [who]
        WhitelistedContributor(AccountIdOf<T>),
        /// ILO organizer whitelisted [who]
        WhitelistedIloOrganizer(AccountIdOf<T>),
        /// Contributor removed [who]
        RemovedWhitelistedContributor(AccountIdOf<T>),
        /// ILO organizer removed [who]
        RemovedWhitelistedIloOrganizer(AccountIdOf<T>),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// ILO for token already exists
        ILOAlreadyExists,
        /// Parameter can't be zero
        ParameterCantBeZero,
        /// Soft cap should be minimum 50% of hard cap
        InvalidSoftCap,
        /// Minimum contribution must be equal or greater than 0.01 base asset tokens
        InvalidMinimumContribution,
        /// Maximum contribution must be greater than minimum contribution
        InvalidMaximumContribution,
        /// Minimum 51% of raised funds must go to liquidity
        InvalidLiquidityPercent,
        /// Lockup days must be minimum 30
        InvalidLockupDays,
        /// Start timestamp be in future
        InvalidStartTimestamp,
        /// End timestamp must be greater than start timestamp
        InvalidEndTimestamp,
        /// Listing price must be greater than ILO price
        InvalidPrice,
        /// Invalid number of tokens for liquidity
        InvalidNumberOfTokensForLiquidity,
        /// Invalid number of tokens for ILO
        InvalidNumberOfTokensForILO,
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
        /// ILO not started
        ILONotStarted,
        /// ILO is finished,
        ILOIsFinished,
        /// Can't contribute in ILO
        CantContributeInILO,
        /// Hard cap is hit
        HardCapIsHit,
        /// Not enough tokens to buy
        NotEnoughTokensToBuy,
        /// Contribution is lower than min
        ContributionIsLowerThenMin,
        /// Contribution is bigger than max
        ContributionIsBiggerThenMax,
        /// Not enough funds
        NotEnoughFunds,
        /// ILO for token does not exist
        ILODoesNotExist,
        /// ILO is not finished
        ILOIsNotFinished,
        /// Pool does not exist
        PoolDoesNotExist,
        /// Unauthorized
        Unauthorized,
        /// Can't claim LP tokens
        CantClaimLPTokens,
        /// Funds already claimed
        FundsAlreadyClaimed,
        /// Nothing to claim
        NothingToClaim,
        /// ILO is failed
        ILOIsFailed,
        /// ILO is succeeded
        ILOIsSucceeded,
        /// Can't create ILO for listed token
        CantCreateILOForListedToken,
        /// Account is not whitelisted
        AccountIsNotWhitelisted,
        /// Team first release percent can't be zero
        InvalidTeamFirstReleasePercent,
        /// Team invalid vesting percent
        InvalidTeamVestingPercent,
        /// Team vesting period can't be zero
        InvalidTeamVestingPeriod,
        /// Not enough team tokens to lock
        NotEnoughTeamTokensToLock,
        /// Invalid fee percent on raised funds
        InvalidFeePercent,
        /// Asset in which funds are being raised is not supported
        BaseAssetNotSupported,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Create ILO
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::create_ilo())]
        pub fn create_ilo(
            origin: OriginFor<T>,
            base_asset: AssetIdOf<T>,
            asset_id: AssetIdOf<T>,
            tokens_for_ilo: Balance,
            tokens_for_liquidity: Balance,
            ilo_price: Balance,
            soft_cap: Balance,
            hard_cap: Balance,
            min_contribution: Balance,
            max_contribution: Balance,
            refund_type: bool,
            liquidity_percent: Balance,
            listing_price: Balance,
            lockup_days: u32,
            start_timestamp: T::Moment,
            end_timestamp: T::Moment,
            team_vesting_total_tokens: Balance,
            team_vesting_first_release_percent: Balance,
            team_vesting_period: T::Moment,
            team_vesting_percent: Balance,
            first_release_percent: Balance,
            vesting_period: T::Moment,
            vesting_percent: Balance,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin.clone())?;

            if !WhitelistedIloOrganizers::<T>::get().contains(&user) {
                return Err(Error::<T>::AccountIsNotWhitelisted.into());
            }

            if base_asset != XOR.into() && base_asset != XSTUSD.into() {
                return Err(Error::<T>::BaseAssetNotSupported.into());
            }

            ensure!(
                !<ILOs<T>>::contains_key(asset_id),
                Error::<T>::ILOAlreadyExists
            );

            // Check if ILO for token already exists
            ensure!(
                !<ILOs<T>>::contains_key(asset_id),
                Error::<T>::ILOAlreadyExists
            );

            let dex_id = if base_asset == XOR.into() {
                DEXId::Polkaswap.into()
            } else {
                DEXId::PolkaswapXSTUSD.into()
            };

            ensure!(
                !<T as Config>::TradingPairSourceManager::is_trading_pair_enabled(
                    &dex_id,
                    &base_asset,
                    &asset_id
                )
                .unwrap_or(true),
                Error::<T>::CantCreateILOForListedToken
            );

            // Get current timestamp
            let current_timestamp = Timestamp::<T>::get();

            // Check parameters
            Self::check_parameters(
                tokens_for_ilo,
                tokens_for_liquidity,
                ilo_price,
                soft_cap,
                hard_cap,
                min_contribution,
                max_contribution,
                liquidity_percent,
                listing_price,
                lockup_days,
                start_timestamp,
                end_timestamp,
                current_timestamp,
                team_vesting_total_tokens,
                team_vesting_first_release_percent,
                team_vesting_period,
                team_vesting_percent,
                first_release_percent,
                vesting_period,
                vesting_percent,
            )?;

            ensure!(
                CeresBurnFeeAmount::<T>::get()
                    <= Assets::<T>::free_balance(&CeresAssetIdOf::<T>::get(), &user).unwrap_or(0),
                Error::<T>::NotEnoughCeres
            );

            let total_tokens = tokens_for_liquidity + tokens_for_ilo;
            ensure!(
                total_tokens <= Assets::<T>::free_balance(&asset_id, &user).unwrap_or(0),
                Error::<T>::NotEnoughTokens
            );

            // Burn CERES as fee
            Assets::<T>::burn(
                origin,
                CeresAssetIdOf::<T>::get(),
                CeresBurnFeeAmount::<T>::get(),
            )?;

            // Transfer tokens to pallet
            Assets::<T>::transfer_from(&asset_id, &user, &Self::account_id(), total_tokens)?;

            let ilo_info = ILOInfo {
                ilo_organizer: user.clone(),
                tokens_for_ilo,
                tokens_for_liquidity,
                ilo_price,
                soft_cap,
                hard_cap,
                min_contribution,
                max_contribution,
                refund_type,
                liquidity_percent,
                listing_price,
                lockup_days,
                start_timestamp,
                end_timestamp,
                contributors_vesting: ContributorsVesting {
                    first_release_percent,
                    vesting_period,
                    vesting_percent,
                },
                team_vesting: TeamVesting {
                    team_vesting_total_tokens,
                    team_vesting_first_release_percent,
                    team_vesting_period,
                    team_vesting_percent,
                },
                sold_tokens: balance!(0),
                funds_raised: balance!(0),
                succeeded: false,
                failed: false,
                lp_tokens: balance!(0),
                claimed_lp_tokens: false,
                finish_timestamp: 0u32.into(),
                base_asset,
            };

            <ILOs<T>>::insert(asset_id, &ilo_info);

            // Emit an event
            Self::deposit_event(Event::ILOCreated(user, asset_id));

            // Return a successful DispatchResult
            Ok(().into())
        }

        /// Contribute
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::contribute())]
        pub fn contribute(
            origin: OriginFor<T>,
            asset_id: AssetIdOf<T>,
            funds_to_contribute: Balance,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            if !WhitelistedContributors::<T>::get().contains(&user) {
                return Err(Error::<T>::AccountIsNotWhitelisted.into());
            }

            let current_timestamp = Timestamp::<T>::get();

            ensure!(
                CeresForContributionInILO::<T>::get()
                    <= Assets::<T>::free_balance(&CeresAssetIdOf::<T>::get(), &user).unwrap_or(0),
                Error::<T>::NotEnoughCeres
            );

            // Get ILO info
            let mut ilo_info = <ILOs<T>>::get(asset_id).ok_or(Error::<T>::ILODoesNotExist)?;

            // Get contribution info
            let mut contribution_info = <Contributions<T>>::get(asset_id, &user);

            ensure!(
                ilo_info.start_timestamp < current_timestamp,
                Error::<T>::ILONotStarted
            );
            ensure!(
                ilo_info.end_timestamp > current_timestamp,
                Error::<T>::ILOIsFinished
            );
            ensure!(
                funds_to_contribute >= ilo_info.min_contribution,
                Error::<T>::ContributionIsLowerThenMin
            );
            ensure!(
                contribution_info.funds_contributed + funds_to_contribute
                    <= ilo_info.max_contribution,
                Error::<T>::ContributionIsBiggerThenMax
            );
            ensure!(
                ilo_info.funds_raised + funds_to_contribute <= ilo_info.hard_cap,
                Error::<T>::HardCapIsHit
            );

            // Calculate amount of bought tokens
            let tokens_bought = (FixedWrapper::from(funds_to_contribute)
                / FixedWrapper::from(ilo_info.ilo_price))
            .try_into_balance()
            .unwrap_or(0);

            ilo_info.funds_raised += funds_to_contribute;
            ilo_info.sold_tokens += tokens_bought;
            contribution_info.funds_contributed += funds_to_contribute;
            contribution_info.tokens_bought += tokens_bought;

            // Transfer base_asset to pallet
            Assets::<T>::transfer_from(
                &ilo_info.base_asset,
                &user,
                &Self::account_id(),
                funds_to_contribute,
            )?;

            // Update storage
            <ILOs<T>>::insert(asset_id, &ilo_info);
            <Contributions<T>>::insert(asset_id, &user, contribution_info);

            // Emit event
            Self::deposit_event(Event::<T>::Contributed(user, asset_id, funds_to_contribute));

            // Return a successful DispatchResult
            Ok(().into())
        }

        /// Emergency withdraw

        #[transactional]
        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::emergency_withdraw())]
        pub fn emergency_withdraw(
            origin: OriginFor<T>,
            asset_id: AssetIdOf<T>,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;
            let current_timestamp = Timestamp::<T>::get();

            // Get ILO info
            let mut ilo_info = <ILOs<T>>::get(asset_id).ok_or(Error::<T>::ILODoesNotExist)?;

            // Get contribution info
            let contribution_info = <Contributions<T>>::get(asset_id, &user);

            ensure!(
                ilo_info.start_timestamp < current_timestamp,
                Error::<T>::ILONotStarted
            );
            ensure!(
                current_timestamp < ilo_info.end_timestamp,
                Error::<T>::ILOIsFinished
            );
            ensure!(
                contribution_info.funds_contributed > 0,
                Error::<T>::NotEnoughFunds
            );

            let funds_to_claim = (FixedWrapper::from(contribution_info.funds_contributed)
                * FixedWrapper::from(0.8))
            .try_into_balance()
            .unwrap_or(0);

            let pallet_account = Self::account_id();
            // Emergency withdraw funds
            Assets::<T>::transfer_from(
                &ilo_info.base_asset,
                &pallet_account,
                &user,
                funds_to_claim,
            )?;

            let penalty = contribution_info.funds_contributed - funds_to_claim;

            Assets::<T>::transfer_from(
                &ilo_info.base_asset,
                &pallet_account,
                &PenaltiesAccount::<T>::get(),
                penalty,
            )?;

            ilo_info.funds_raised -= contribution_info.funds_contributed;
            ilo_info.sold_tokens -= contribution_info.tokens_bought;

            // Update map
            <ILOs<T>>::insert(asset_id, &ilo_info);
            <Contributions<T>>::remove(asset_id, &user);

            // Emit event
            Self::deposit_event(Event::<T>::EmergencyWithdrawn(
                user,
                asset_id,
                contribution_info.funds_contributed,
            ));

            Ok(().into())
        }

        /// Finish ILO
        #[transactional]
        #[pallet::call_index(3)]
        #[pallet::weight(<T as Config>::WeightInfo::finish_ilo())]
        pub fn finish_ilo(
            origin: OriginFor<T>,
            asset_id: AssetIdOf<T>,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin.clone())?;

            // Get ILO info of asset_id token
            let mut ilo_info = <ILOs<T>>::get(asset_id).ok_or(Error::<T>::ILODoesNotExist)?;

            if user != ilo_info.ilo_organizer {
                return Err(Error::<T>::Unauthorized.into());
            }

            // Get current timestamp
            let current_timestamp = Timestamp::<T>::get();
            ensure!(
                current_timestamp > ilo_info.end_timestamp
                    || ilo_info.funds_raised == ilo_info.hard_cap,
                Error::<T>::ILOIsNotFinished
            );
            ensure!(!ilo_info.failed, Error::<T>::ILOIsFailed);
            ensure!(!ilo_info.succeeded, Error::<T>::ILOIsSucceeded);

            let pallet_account = Self::account_id();
            if ilo_info.funds_raised < ilo_info.soft_cap {
                // Failed ILO
                ilo_info.failed = true;
                let total_tokens = ilo_info.tokens_for_liquidity + ilo_info.tokens_for_ilo;
                if !ilo_info.refund_type {
                    Assets::<T>::burn(
                        RawOrigin::Signed(pallet_account).into(),
                        asset_id,
                        total_tokens,
                    )?;
                } else {
                    Assets::<T>::transfer_from(
                        &asset_id,
                        &pallet_account,
                        &ilo_info.ilo_organizer,
                        total_tokens,
                    )?;
                }

                <ILOs<T>>::insert(asset_id, &ilo_info);

                return Ok(().into());
            }

            // Transfer fee to authority account
            let funds_raised_fee = (FixedWrapper::from(ilo_info.funds_raised)
                * FixedWrapper::from(FeePercentOnRaisedFunds::<T>::get()))
            .try_into_balance()
            .unwrap_or(0);
            Assets::<T>::transfer_from(
                &ilo_info.base_asset,
                &pallet_account,
                &AuthorityAccount::<T>::get(),
                funds_raised_fee,
            )?;

            // Transfer raised funds to team
            let raised_funds_without_fee = ilo_info.funds_raised - funds_raised_fee;
            let funds_for_liquidity = (FixedWrapper::from(raised_funds_without_fee)
                * FixedWrapper::from(ilo_info.liquidity_percent))
            .try_into_balance()
            .unwrap_or(0);
            let funds_for_team = raised_funds_without_fee - funds_for_liquidity;
            Assets::<T>::transfer_from(
                &ilo_info.base_asset,
                &pallet_account,
                &ilo_info.ilo_organizer,
                funds_for_team,
            )?;

            let dex_id = if ilo_info.base_asset == XOR.into() {
                DEXId::Polkaswap.into()
            } else {
                DEXId::PolkaswapXSTUSD.into()
            };
            // Register trading pair
            <T as Config>::TradingPairSourceManager::register_pair(
                dex_id,
                ilo_info.base_asset,
                asset_id,
            )?;

            // Initialize pool
            PoolXYK::<T>::initialize_pool(
                RawOrigin::Signed(pallet_account.clone()).into(),
                dex_id,
                ilo_info.base_asset,
                asset_id,
            )?;

            // Deposit liquidity
            let tokens_for_liquidity = (FixedWrapper::from(funds_for_liquidity)
                / FixedWrapper::from(ilo_info.listing_price))
            .try_into_balance()
            .unwrap_or(0);
            ensure!(
                tokens_for_liquidity <= ilo_info.tokens_for_liquidity,
                Error::<T>::NotEnoughTokens
            );
            PoolXYK::<T>::deposit_liquidity(
                RawOrigin::Signed(pallet_account.clone()).into(),
                dex_id,
                ilo_info.base_asset,
                asset_id,
                funds_for_liquidity,
                tokens_for_liquidity,
                funds_for_liquidity,
                tokens_for_liquidity,
            )?;

            // Burn unused tokens for liquidity
            Assets::<T>::burn(
                RawOrigin::Signed(pallet_account.clone()).into(),
                asset_id,
                ilo_info.tokens_for_liquidity - tokens_for_liquidity,
            )?;

            // Burn unused tokens for ilo
            Assets::<T>::burn(
                RawOrigin::Signed(pallet_account.clone()).into(),
                asset_id,
                ilo_info.tokens_for_ilo - ilo_info.sold_tokens,
            )?;

            // Lock liquidity
            let unlocking_liq_timestamp = current_timestamp
                + (T::MILLISECONDS_PER_DAY.saturating_mul(ilo_info.lockup_days.into()));
            CeresLiquidityLocker::<T>::lock_liquidity(
                RawOrigin::Signed(pallet_account.clone()).into(),
                ilo_info.base_asset,
                asset_id,
                unlocking_liq_timestamp,
                balance!(1),
                true,
            )?;

            // Calculate LP tokens
            let pool_account = PoolXYK::<T>::properties_of_pool(ilo_info.base_asset, asset_id)
                .ok_or(Error::<T>::PoolDoesNotExist)?
                .0;
            ilo_info.lp_tokens =
                PoolXYK::<T>::balance_of_pool_provider(pool_account, pallet_account).unwrap_or(0);

            ilo_info.succeeded = true;
            ilo_info.finish_timestamp = current_timestamp;
            <ILOs<T>>::insert(asset_id, &ilo_info);

            // Lock team tokens
            if ilo_info.team_vesting.team_vesting_total_tokens != balance!(0) {
                let mut vesting_amount =
                    balance!(1) - ilo_info.team_vesting.team_vesting_first_release_percent;
                let tokens_to_lock =
                    (FixedWrapper::from(ilo_info.team_vesting.team_vesting_total_tokens)
                        * FixedWrapper::from(vesting_amount))
                    .try_into_balance()
                    .unwrap_or(0);

                ensure!(
                    tokens_to_lock <= Assets::<T>::free_balance(&asset_id, &user).unwrap_or(0),
                    Error::<T>::NotEnoughTeamTokensToLock
                );

                let mut unlocking_timestamp =
                    current_timestamp + ilo_info.team_vesting.team_vesting_period;
                let tokens_to_lock_per_period =
                    (FixedWrapper::from(ilo_info.team_vesting.team_vesting_total_tokens)
                        * FixedWrapper::from(ilo_info.team_vesting.team_vesting_percent))
                    .try_into_balance()
                    .unwrap_or(0);

                while vesting_amount > balance!(0) {
                    TokenLocker::<T>::lock_tokens(
                        origin.clone(),
                        asset_id,
                        unlocking_timestamp,
                        tokens_to_lock_per_period,
                    )?;

                    unlocking_timestamp += ilo_info.team_vesting.team_vesting_period;
                    vesting_amount = vesting_amount
                        .checked_sub(ilo_info.team_vesting.team_vesting_percent)
                        .unwrap_or(balance!(0));
                }
            }

            // Emit an event
            Self::deposit_event(Event::ILOFinished(user, asset_id));

            // Return a successful DispatchResult
            Ok(().into())
        }

        /// Claim LP tokens
        #[pallet::call_index(4)]
        #[pallet::weight(<T as Config>::WeightInfo::claim_lp_tokens())]
        pub fn claim_lp_tokens(
            origin: OriginFor<T>,
            asset_id: AssetIdOf<T>,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;
            let current_timestamp = Timestamp::<T>::get();

            // Get ILO info
            let mut ilo_info = <ILOs<T>>::get(asset_id).ok_or(Error::<T>::ILODoesNotExist)?;

            if user != ilo_info.ilo_organizer {
                return Err(Error::<T>::Unauthorized.into());
            }

            ensure!(!ilo_info.claimed_lp_tokens, Error::<T>::CantClaimLPTokens);

            let unlocking_timestamp = ilo_info.finish_timestamp.saturating_add(
                T::MILLISECONDS_PER_DAY.saturating_mul(ilo_info.lockup_days.into()),
            );
            ensure!(
                current_timestamp >= unlocking_timestamp,
                Error::<T>::CantClaimLPTokens
            );

            let pallet_account = Self::account_id();

            // Get pool account
            let pool_account = PoolXYK::<T>::properties_of_pool(ilo_info.base_asset, asset_id)
                .ok_or(Error::<T>::PoolDoesNotExist)?
                .0;

            // Transfer LP tokens
            PoolXYK::<T>::transfer_lp_tokens(
                pool_account,
                ilo_info.base_asset,
                asset_id,
                pallet_account,
                user.clone(),
                ilo_info.lp_tokens,
            )?;

            ilo_info.claimed_lp_tokens = true;

            // Update storage
            <ILOs<T>>::insert(asset_id, &ilo_info);

            // Emit an event
            Self::deposit_event(Event::ClaimedLP(user, asset_id));

            // Return a successful DispatchResult
            Ok(().into())
        }

        /// Claim tokens
        #[pallet::call_index(5)]
        #[pallet::weight(<T as Config>::WeightInfo::claim())]
        pub fn claim(origin: OriginFor<T>, asset_id: AssetIdOf<T>) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            // Get ILO info
            let ilo_info = <ILOs<T>>::get(asset_id).ok_or(Error::<T>::ILODoesNotExist)?;

            if !ilo_info.failed && !ilo_info.succeeded {
                return Err(Error::<T>::ILOIsNotFinished.into());
            }

            // Get contribution info
            let mut contribution_info = <Contributions<T>>::get(asset_id, &user);
            ensure!(
                !contribution_info.claiming_finished,
                Error::<T>::FundsAlreadyClaimed
            );

            let pallet_account = Self::account_id();

            // ILO failed
            if ilo_info.failed {
                // Claim unused funds
                Assets::<T>::transfer_from(
                    &ilo_info.base_asset,
                    &pallet_account,
                    &user,
                    contribution_info.funds_contributed,
                )?;
                contribution_info.claiming_finished = true;
            } else {
                // First claim
                if contribution_info.tokens_claimed == balance!(0) {
                    let tokens_to_claim = (FixedWrapper::from(contribution_info.tokens_bought)
                        * FixedWrapper::from(ilo_info.contributors_vesting.first_release_percent))
                    .try_into_balance()
                    .unwrap_or(0);
                    // Claim first time
                    Assets::<T>::transfer_from(&asset_id, &pallet_account, &user, tokens_to_claim)?;
                    contribution_info.tokens_claimed += tokens_to_claim;
                    if ilo_info.contributors_vesting.first_release_percent == balance!(1) {
                        contribution_info.claiming_finished = true;
                    }
                } else {
                    // Claim the rest parts
                    let current_timestamp = Timestamp::<T>::get();
                    let time_passed = current_timestamp.saturating_sub(ilo_info.finish_timestamp);

                    let potential_claims: u32 = time_passed
                        .checked_div(&ilo_info.contributors_vesting.vesting_period)
                        .unwrap_or(0u32.into())
                        .unique_saturated_into();
                    if potential_claims == 0 {
                        return Err(Error::<T>::NothingToClaim.into());
                    }
                    let allowed_claims = potential_claims - contribution_info.number_of_claims;
                    if allowed_claims == 0 {
                        return Err(Error::<T>::NothingToClaim.into());
                    }

                    let tokens_per_claim = (FixedWrapper::from(contribution_info.tokens_bought)
                        * FixedWrapper::from(ilo_info.contributors_vesting.vesting_percent))
                    .try_into_balance()
                    .unwrap_or(0);
                    let mut claimable = (FixedWrapper::from(tokens_per_claim)
                        * FixedWrapper::from(balance!(allowed_claims)))
                    .try_into_balance()
                    .unwrap_or(0);
                    let left_to_claim =
                        contribution_info.tokens_bought - contribution_info.tokens_claimed;

                    if left_to_claim < claimable {
                        claimable = left_to_claim;
                    }

                    // Claim tokens
                    Assets::<T>::transfer_from(&asset_id, &pallet_account, &user, claimable)?;
                    contribution_info.tokens_claimed += claimable;
                    contribution_info.number_of_claims += (claimable / tokens_per_claim) as u32;

                    let claimed_percent =
                        (FixedWrapper::from(ilo_info.contributors_vesting.vesting_percent)
                            * FixedWrapper::from(balance!(contribution_info.number_of_claims)))
                        .try_into_balance()
                        .unwrap_or(0)
                            + ilo_info.contributors_vesting.first_release_percent;

                    if claimed_percent >= balance!(1) {
                        contribution_info.claiming_finished = true;
                    }
                }
            }

            <Contributions<T>>::insert(asset_id, &user, contribution_info);

            // Emit an event
            Self::deposit_event(Event::Claimed(user, asset_id));

            Ok(().into())
        }

        /// Change fee percent on raised funds in successful ILO
        #[pallet::call_index(6)]
        #[pallet::weight(<T as Config>::WeightInfo::change_ceres_burn_fee())]
        pub fn change_fee_percent_for_raised_funds(
            origin: OriginFor<T>,
            fee_percent: Balance,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            if user != AuthorityAccount::<T>::get() {
                return Err(Error::<T>::Unauthorized.into());
            }

            if fee_percent > balance!(1) {
                return Err(Error::<T>::InvalidFeePercent.into());
            }

            FeePercentOnRaisedFunds::<T>::put(fee_percent);

            // Emit an event
            Self::deposit_event(Event::FeeChanged(fee_percent));

            Ok(().into())
        }

        /// Change CERES burn fee
        #[pallet::call_index(7)]
        #[pallet::weight(<T as Config>::WeightInfo::change_ceres_burn_fee())]
        pub fn change_ceres_burn_fee(
            origin: OriginFor<T>,
            ceres_fee: Balance,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            if user != AuthorityAccount::<T>::get() {
                return Err(Error::<T>::Unauthorized.into());
            }

            CeresBurnFeeAmount::<T>::put(ceres_fee);

            // Emit an event
            Self::deposit_event(Event::FeeChanged(ceres_fee));

            Ok(().into())
        }

        /// Change CERES contribution fee
        #[pallet::call_index(8)]
        #[pallet::weight(<T as Config>::WeightInfo::change_ceres_contribution_fee())]
        pub fn change_ceres_contribution_fee(
            origin: OriginFor<T>,
            ceres_fee: Balance,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            if user != AuthorityAccount::<T>::get() {
                return Err(Error::<T>::Unauthorized.into());
            }

            CeresForContributionInILO::<T>::put(ceres_fee);

            // Emit an event
            Self::deposit_event(Event::FeeChanged(ceres_fee));

            Ok(().into())
        }

        /// Claim PSWAP rewards
        #[transactional]
        #[pallet::call_index(9)]
        #[pallet::weight(<T as Config>::WeightInfo::claim_pswap_rewards())]
        pub fn claim_pswap_rewards(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            if user != AuthorityAccount::<T>::get() {
                return Err(Error::<T>::Unauthorized.into());
            }

            let pallet_account = Self::account_id();
            PSWAPDistribution::<T>::claim_incentive(
                RawOrigin::Signed(pallet_account.clone()).into(),
            )?;

            let _ =
                VestedRewards::<T>::claim_rewards(RawOrigin::Signed(pallet_account.clone()).into());

            let pswap_rewards =
                Assets::<T>::free_balance(&PSWAP.into(), &pallet_account).unwrap_or(0);

            // Claim PSWAP rewards
            Assets::<T>::transfer_from(
                &PSWAP.into(),
                &pallet_account,
                &AuthorityAccount::<T>::get(),
                pswap_rewards,
            )?;

            // Emit an event
            Self::deposit_event(Event::ClaimedPSWAP());

            Ok(().into())
        }

        /// Add whitelisted contributor
        #[pallet::call_index(10)]
        #[pallet::weight(<T as Config>::WeightInfo::add_whitelisted_contributor())]
        pub fn add_whitelisted_contributor(
            origin: OriginFor<T>,
            contributor: AccountIdOf<T>,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            if user != AuthorityAccount::<T>::get() {
                return Err(Error::<T>::Unauthorized.into());
            }

            WhitelistedContributors::<T>::append(&contributor);

            // Emit an event
            Self::deposit_event(Event::WhitelistedContributor(contributor));

            Ok(().into())
        }

        /// Remove whitelisted contributor
        #[pallet::call_index(11)]
        #[pallet::weight(<T as Config>::WeightInfo::remove_whitelisted_contributor())]
        pub fn remove_whitelisted_contributor(
            origin: OriginFor<T>,
            contributor: AccountIdOf<T>,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            if user != AuthorityAccount::<T>::get() {
                return Err(Error::<T>::Unauthorized.into());
            }

            let mut temp = WhitelistedContributors::<T>::get();
            temp.retain(|x| *x != contributor);
            WhitelistedContributors::<T>::set(temp);

            // Emit an event
            Self::deposit_event(Event::RemovedWhitelistedContributor(contributor));

            Ok(().into())
        }

        /// Add whitelisted ILO organizer
        #[pallet::call_index(12)]
        #[pallet::weight(<T as Config>::WeightInfo::add_whitelisted_ilo_organizer())]
        pub fn add_whitelisted_ilo_organizer(
            origin: OriginFor<T>,
            ilo_organizer: AccountIdOf<T>,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            if user != AuthorityAccount::<T>::get() {
                return Err(Error::<T>::Unauthorized.into());
            }

            WhitelistedIloOrganizers::<T>::append(&ilo_organizer);

            // Emit an event
            Self::deposit_event(Event::WhitelistedIloOrganizer(ilo_organizer));

            Ok(().into())
        }

        /// Remove whitelisted ILO organizer
        #[pallet::call_index(13)]
        #[pallet::weight(<T as Config>::WeightInfo::remove_whitelisted_ilo_organizer())]
        pub fn remove_whitelisted_ilo_organizer(
            origin: OriginFor<T>,
            ilo_organizer: AccountIdOf<T>,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            if user != AuthorityAccount::<T>::get() {
                return Err(Error::<T>::Unauthorized.into());
            }

            let mut temp = WhitelistedIloOrganizers::<T>::get();
            temp.retain(|x| *x != ilo_organizer);
            WhitelistedIloOrganizers::<T>::set(temp);

            // Emit an event
            Self::deposit_event(Event::RemovedWhitelistedIloOrganizer(ilo_organizer));

            Ok(().into())
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(now: BlockNumberFor<T>) -> Weight {
            let mut counter: u64 = 0;

            if (now % T::BLOCKS_PER_ONE_DAY).is_zero() {
                let current_timestamp = Timestamp::<T>::get();
                let days_to_finish_ilo = 14u32;
                let pallet_account = Self::account_id();

                let ilos = ILOs::<T>::iter().collect::<Vec<_>>();
                for (ilo_asset, mut ilo_info) in ilos {
                    if current_timestamp > ilo_info.end_timestamp
                        && !ilo_info.failed
                        && !ilo_info.succeeded
                    {
                        let finish_timestamp = ilo_info.end_timestamp
                            + (T::MILLISECONDS_PER_DAY.saturating_mul(days_to_finish_ilo.into()));
                        if current_timestamp >= finish_timestamp {
                            ilo_info.failed = true;

                            let total_tokens =
                                ilo_info.tokens_for_liquidity + ilo_info.tokens_for_ilo;
                            if !ilo_info.refund_type {
                                let _ = Assets::<T>::burn(
                                    RawOrigin::Signed(pallet_account.clone()).into(),
                                    ilo_asset,
                                    total_tokens,
                                );
                            } else {
                                let _ = Assets::<T>::transfer_from(
                                    &ilo_asset,
                                    &pallet_account,
                                    &ilo_info.ilo_organizer,
                                    total_tokens,
                                );
                            }

                            <ILOs<T>>::insert(ilo_asset, ilo_info);
                            counter += 1;
                        }
                    }
                }
            }

            T::DbWeight::get()
                .reads(counter)
                .saturating_add(T::DbWeight::get().writes(counter))
        }
    }

    impl<T: Config> Pallet<T> {
        /// The account ID of pallet
        fn account_id() -> T::AccountId {
            PALLET_ID.into_account_truncating()
        }

        /// Check parameters
        #[allow(clippy::too_many_arguments)]
        fn check_parameters(
            tokens_for_ilo: Balance,
            tokens_for_liquidity: Balance,
            ilo_price: Balance,
            soft_cap: Balance,
            hard_cap: Balance,
            min_contribution: Balance,
            max_contribution: Balance,
            liquidity_percent: Balance,
            listing_price: Balance,
            lockup_days: u32,
            start_timestamp: T::Moment,
            end_timestamp: T::Moment,
            current_timestamp: T::Moment,
            team_vesting_total_tokens: Balance,
            team_vesting_first_release_percent: Balance,
            team_vesting_period: T::Moment,
            team_vesting_percent: Balance,
            first_release_percent: Balance,
            vesting_period: T::Moment,
            vesting_percent: Balance,
        ) -> Result<(), DispatchError> {
            let zero = balance!(0);
            if ilo_price == zero {
                return Err(Error::<T>::ParameterCantBeZero.into());
            }

            if hard_cap == zero {
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

            if min_contribution >= max_contribution {
                return Err(Error::<T>::InvalidMaximumContribution.into());
            }

            if liquidity_percent > balance!(1) || liquidity_percent < balance!(0.51) {
                return Err(Error::<T>::InvalidLiquidityPercent.into());
            }

            if lockup_days < 30 {
                return Err(Error::<T>::InvalidLockupDays.into());
            }

            if start_timestamp <= current_timestamp {
                return Err(Error::<T>::InvalidStartTimestamp.into());
            }

            if start_timestamp >= end_timestamp {
                return Err(Error::<T>::InvalidEndTimestamp.into());
            }

            if ilo_price >= listing_price {
                return Err(Error::<T>::InvalidPrice.into());
            }

            let tfi = ((FixedWrapper::from(hard_cap) / FixedWrapper::from(ilo_price))
                .get()
                .map_err(|_| Error::<T>::InvalidNumberOfTokensForILO)?)
            .integral(RoundMode::Ceil);
            let tfi_balance = Fixed::try_from(tfi).unwrap_or(Default::default());

            if tokens_for_ilo != balance!(tfi_balance) {
                return Err(Error::<T>::InvalidNumberOfTokensForILO.into());
            }

            let tfl = ((FixedWrapper::from(hard_cap) * FixedWrapper::from(liquidity_percent))
                / FixedWrapper::from(listing_price))
            .get()
            .map_err(|_| Error::<T>::InvalidNumberOfTokensForLiquidity)?
            .integral(RoundMode::Ceil);
            let tfl_balance = Fixed::try_from(tfl).unwrap_or(Default::default());

            if tokens_for_liquidity != balance!(tfl_balance) {
                return Err(Error::<T>::InvalidNumberOfTokensForLiquidity.into());
            }

            // If team vesting is selected
            if team_vesting_total_tokens != zero {
                if team_vesting_first_release_percent == zero {
                    return Err(Error::<T>::InvalidTeamFirstReleasePercent.into());
                }

                let one = balance!(1);
                if team_vesting_first_release_percent != one && team_vesting_percent == zero {
                    return Err(Error::<T>::InvalidTeamVestingPercent.into());
                }

                if team_vesting_first_release_percent + team_vesting_percent > one {
                    return Err(Error::<T>::InvalidTeamVestingPercent.into());
                }

                let team_vesting_amount = one - team_vesting_first_release_percent;
                if team_vesting_first_release_percent != one
                    && team_vesting_amount % team_vesting_percent != 0
                {
                    return Err(Error::<T>::InvalidTeamVestingPercent.into());
                }

                if team_vesting_first_release_percent != one && team_vesting_period == 0u32.into() {
                    return Err(Error::<T>::InvalidTeamVestingPeriod.into());
                }
            }

            if first_release_percent == zero {
                return Err(Error::<T>::InvalidFirstReleasePercent.into());
            }

            let one = balance!(1);
            if first_release_percent != one && vesting_percent == zero {
                return Err(Error::<T>::InvalidVestingPercent.into());
            }

            if first_release_percent + vesting_percent > one {
                return Err(Error::<T>::InvalidVestingPercent.into());
            }

            let vesting_amount = one - first_release_percent;
            if first_release_percent != one && vesting_amount % vesting_percent != 0 {
                return Err(Error::<T>::InvalidVestingPercent.into());
            }

            if first_release_percent != one && vesting_period == 0u32.into() {
                return Err(Error::<T>::InvalidVestingPeriod.into());
            }

            Ok(())
        }
    }
}
