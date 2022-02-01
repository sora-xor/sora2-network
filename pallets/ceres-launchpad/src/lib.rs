#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

mod benchmarking;

use codec::{Decode, Encode};

#[derive(Encode, Decode, Default, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct ILOInfo<Balance, AccountId, BlockNumber> {
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
    start_block: BlockNumber,
    end_block: BlockNumber,
    token_vesting: VestingInfo<Balance, BlockNumber>,
    sold_tokens: Balance,
    funds_raised: Balance,
    succeeded: bool,
    failed: bool,
    lp_tokens: Balance,
    claimed_lp_tokens: bool,
    finish_block: BlockNumber,
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
    claiming_finished: bool,
    number_of_claims: u32,
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use crate::{ContributionInfo, ILOInfo, VestingInfo};
    use common::fixnum::ops::RoundMode;
    use common::prelude::{Balance, FixedWrapper, XOR};
    use common::{balance, DEXId, PoolXykPallet, PSWAP};
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use frame_system::{ensure_signed, RawOrigin};
    use hex_literal::hex;
    use sp_runtime::traits::{
        AccountIdConversion, CheckedDiv, Saturating, UniqueSaturatedInto, Zero,
    };
    use sp_runtime::ModuleId;

    const PALLET_ID: ModuleId = ModuleId(*b"crslaunc");

    #[pallet::config]
    pub trait Config:
        frame_system::Config
        + assets::Config
        + trading_pair::Config
        + pool_xyk::Config
        + ceres_liquidity_locker::Config
        + pswap_distribution::Config
    {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
    }

    type Assets<T> = assets::Pallet<T>;
    type TradingPair<T> = trading_pair::Pallet<T>;
    type PoolXYK<T> = pool_xyk::Pallet<T>;
    type CeresLiquidityLocker<T> = ceres_liquidity_locker::Pallet<T>;
    type PSWAPDistribution<T> = pswap_distribution::Pallet<T>;

    type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
    type AssetIdOf<T> = <T as assets::Config>::AssetId;

    #[pallet::pallet]
    #[pallet::generate_store(pub (super) trait Store)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::type_value]
    pub fn DefaultForPenaltiesAccount<T: Config>() -> AccountIdOf<T> {
        let bytes = hex!("0a0455d92e1fda8dee17b2c58761c8efca490ef2a1a03322dbfea7379481d517");
        AccountIdOf::<T>::decode(&mut &bytes[..]).unwrap_or_default()
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
        balance!(0.5)
    }

    /// Amount of CERES for contribution in ILO
    #[pallet::storage]
    #[pallet::getter(fn ceres_for_contribution_in_ilo)]
    pub type CeresForContributionInILO<T: Config> =
        StorageValue<_, Balance, ValueQuery, DefaultCeresForContributionInILO<T>>;

    #[pallet::type_value]
    pub fn DefaultForAuthorityAccount<T: Config>() -> AccountIdOf<T> {
        let bytes = hex!("34a5b78f5fbcdc92a28767d63b579690a4b2f6a179931b3ecc87f09fc9366d47");
        AccountIdOf::<T>::decode(&mut &bytes[..]).unwrap_or_default()
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
        /// ILONotStarted
        ILONotStarted,
        /// ILO is finished,
        ILOIsFinished,
        /// CantContributeInILO
        CantContributeInILO,
        /// HardCapIsHit
        HardCapIsHit,
        /// NotEnoughTokensToBuy
        NotEnoughTokensToBuy,
        /// ContributionIsLowerThenMin
        ContributionIsLowerThenMin,
        /// ContributionIsBiggerThenMax
        ContributionIsBiggerThenMax,
        /// NotEnoughFunds
        NotEnoughFunds,
        /// ILO for token does not exist
        ILODoesNotExist,
        /// ILO is not finished
        ILOIsNotFinished,
        /// Pool does not exist
        PoolDoesNotExist,
        /// Unauthorized
        Unauthorized,
        /// CantClaimLPTokens
        CantClaimLPTokens,
        /// Funds already claimed
        FundsAlreadyClaimed,
        /// Nothing to claim
        NothingToClaim,
        /// ILO is failed
        ILOIsFailed,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Create ILO
        #[pallet::weight(10000)]
        pub fn create_ilo(
            origin: OriginFor<T>,
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
            start_block: T::BlockNumber,
            end_block: T::BlockNumber,
            first_release_percent: Balance,
            vesting_period: T::BlockNumber,
            vesting_percent: Balance,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin.clone())?;

            // Get ILO info of token
            let mut ilo_info = <ILOs<T>>::get(&asset_id);

            // Check if ILO for token already exists
            ensure!(
                ilo_info.ilo_price == balance!(0),
                Error::<T>::ILOAlreadyExists
            );

            // Get current block
            let current_block = frame_system::Pallet::<T>::block_number();

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
                start_block,
                end_block,
                current_block,
                first_release_percent,
                vesting_period,
                vesting_percent,
            )?;

            ensure!(
                CeresBurnFeeAmount::<T>::get()
                    <= Assets::<T>::free_balance(&T::CeresAssetId::get().into(), &user)
                        .unwrap_or(0),
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
                T::CeresAssetId::get().into(),
                CeresBurnFeeAmount::<T>::get(),
            )?;

            // Transfer tokens to pallet
            Assets::<T>::transfer_from(&asset_id.into(), &user, &Self::account_id(), total_tokens)?;

            ilo_info = ILOInfo {
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
                claimed_lp_tokens: false,
                finish_block: 0u32.into(),
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

            ensure!(
                CeresForContributionInILO::<T>::get()
                    <= Assets::<T>::free_balance(&T::CeresAssetId::get().into(), &user)
                        .unwrap_or(0),
                Error::<T>::NotEnoughCeres
            );

            // Get ILO info
            let mut ilo_info = <ILOs<T>>::get(&asset_id);

            // Check if ILO for token exists
            ensure!(ilo_info.ilo_price != 0, Error::<T>::ILODoesNotExist);

            // Get contribution info
            let mut contribution_info = <Contributions<T>>::get(&asset_id, &user);

            ensure!(
                ilo_info.start_block < current_block,
                Error::<T>::ILONotStarted
            );
            ensure!(
                ilo_info.end_block > current_block,
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

            // Transfer XOR to pallet
            Assets::<T>::transfer_from(
                &XOR.into(),
                &user,
                &Self::account_id(),
                funds_to_contribute,
            )?;

            // Update storage
            <ILOs<T>>::insert(&asset_id, &ilo_info);
            <Contributions<T>>::insert(&asset_id, &user, contribution_info);

            // Emit event
            Self::deposit_event(Event::<T>::Contributed(user, asset_id, funds_to_contribute));

            // Return a successful DispatchResult
            Ok(().into())
        }

        #[pallet::weight(10000)]
        pub fn emergency_withdraw(
            origin: OriginFor<T>,
            asset_id: AssetIdOf<T>,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;
            let current_block = frame_system::Pallet::<T>::block_number();

            // Get ILO info
            let mut ilo_info = <ILOs<T>>::get(&asset_id);

            // Check if ILO for token exists
            ensure!(ilo_info.ilo_price != 0, Error::<T>::ILODoesNotExist);

            // Get contribution info
            let contribution_info = <Contributions<T>>::get(&asset_id, &user);

            ensure!(
                ilo_info.start_block < current_block,
                Error::<T>::ILONotStarted
            );
            ensure!(
                current_block < ilo_info.end_block,
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
            Assets::<T>::transfer_from(&XOR.into(), &pallet_account, &user, funds_to_claim)?;

            let penalty = contribution_info.funds_contributed - funds_to_claim;

            Assets::<T>::transfer_from(
                &XOR.into(),
                &pallet_account,
                &PenaltiesAccount::<T>::get(),
                penalty,
            )?;

            ilo_info.funds_raised -= contribution_info.funds_contributed;
            ilo_info.sold_tokens -= contribution_info.tokens_bought;

            // Update map
            <ILOs<T>>::insert(&asset_id, &ilo_info);
            <Contributions<T>>::remove(&asset_id, &user);

            // Emit event
            Self::deposit_event(Event::<T>::EmergencyWithdrawn(
                user,
                asset_id,
                contribution_info.funds_contributed,
            ));

            Ok(().into())
        }

        /// Finish ILO
        #[pallet::weight(10000)]
        pub fn finish_ilo(
            origin: OriginFor<T>,
            asset_id: AssetIdOf<T>,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin.clone())?;

            // Get ILO info of asset_id token
            let mut ilo_info = <ILOs<T>>::get(&asset_id);

            // Check if ILO for token already exists
            ensure!(ilo_info.ilo_price != 0, Error::<T>::ILODoesNotExist);

            // Get current block
            let current_block = frame_system::Pallet::<T>::block_number();
            ensure!(
                current_block > ilo_info.end_block,
                Error::<T>::ILOIsNotFinished
            );
            ensure!(!ilo_info.failed, Error::<T>::ILOIsFailed);
            if user != ilo_info.ilo_organizer {
                return Err(Error::<T>::Unauthorized.into());
            }

            let pallet_account = Self::account_id();
            if ilo_info.funds_raised < ilo_info.soft_cap {
                // Failed ILO
                ilo_info.failed = true;
                let total_tokens = ilo_info.tokens_for_liquidity + ilo_info.tokens_for_ilo;
                if !ilo_info.refund_type {
                    Assets::<T>::burn(
                        RawOrigin::Signed(pallet_account).into(),
                        asset_id.into(),
                        total_tokens,
                    )?;
                } else {
                    Assets::<T>::transfer_from(
                        &asset_id.into(),
                        &pallet_account,
                        &ilo_info.ilo_organizer,
                        total_tokens,
                    )?;
                }

                <ILOs<T>>::insert(&asset_id, &ilo_info);

                return Ok(().into());
            }

            // Transfer raised funds to team
            let funds_for_liquidity = (FixedWrapper::from(ilo_info.funds_raised)
                * FixedWrapper::from(ilo_info.liquidity_percent))
            .try_into_balance()
            .unwrap_or(0);
            let funds_for_team = ilo_info.funds_raised - funds_for_liquidity;
            Assets::<T>::transfer_from(
                &XOR.into(),
                &pallet_account,
                &ilo_info.ilo_organizer,
                funds_for_team,
            )?;

            // Register trading pair
            TradingPair::<T>::register(
                RawOrigin::Signed(pallet_account.clone()).into(),
                DEXId::Polkaswap.into(),
                XOR.into(),
                asset_id.into(),
            )?;

            // Initialize pool
            PoolXYK::<T>::initialize_pool(
                RawOrigin::Signed(pallet_account.clone()).into(),
                DEXId::Polkaswap.into(),
                XOR.into(),
                asset_id.into(),
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
                DEXId::Polkaswap.into(),
                XOR.into(),
                asset_id.into(),
                funds_for_liquidity,
                tokens_for_liquidity,
                balance!(0),
                balance!(0),
            )?;

            // Burn unused tokens for liquidity
            Assets::<T>::burn(
                RawOrigin::Signed(pallet_account.clone()).into(),
                asset_id.into(),
                ilo_info.tokens_for_liquidity - tokens_for_liquidity,
            )?;

            // Burn unused tokens for ilo
            Assets::<T>::burn(
                RawOrigin::Signed(pallet_account.clone()).into(),
                asset_id.into(),
                ilo_info.tokens_for_ilo - ilo_info.sold_tokens,
            )?;

            // Lock liquidity
            let unlocking_block = current_block
                + (T::BLOCKS_PER_ONE_DAY.saturating_mul(ilo_info.lockup_days.into())).into();
            CeresLiquidityLocker::<T>::lock_liquidity(
                RawOrigin::Signed(pallet_account.clone()).into(),
                XOR.into(),
                asset_id.into(),
                unlocking_block,
                balance!(1),
                true,
            )?;

            // Calculate LP tokens
            let pool_account = PoolXYK::<T>::properties_of_pool(XOR.into(), asset_id)
                .ok_or(Error::<T>::PoolDoesNotExist)?
                .0;
            ilo_info.lp_tokens =
                PoolXYK::<T>::balance_of_pool_provider(pool_account, pallet_account).unwrap_or(0);

            ilo_info.succeeded = true;
            ilo_info.finish_block = current_block;
            <ILOs<T>>::insert(&asset_id, &ilo_info);

            // Emit an event
            Self::deposit_event(Event::ILOFinished(user.clone(), asset_id));

            // Return a successful DispatchResult
            Ok(().into())
        }

        #[pallet::weight(10000)]
        pub fn claim_lp_tokens(
            origin: OriginFor<T>,
            asset_id: AssetIdOf<T>,
        ) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;
            let current_block = frame_system::Pallet::<T>::block_number();

            // Get ILO info
            let mut ilo_info = <ILOs<T>>::get(&asset_id);

            // Check if ILO for token exists
            ensure!(ilo_info.ilo_price != 0, Error::<T>::ILODoesNotExist);

            ensure!(!ilo_info.claimed_lp_tokens, Error::<T>::CantClaimLPTokens);

            let unlocking_block = ilo_info
                .finish_block
                .saturating_add(T::BLOCKS_PER_ONE_DAY.saturating_mul(ilo_info.lockup_days.into()));
            ensure!(
                current_block >= unlocking_block,
                Error::<T>::CantClaimLPTokens
            );

            if user != ilo_info.ilo_organizer {
                return Err(Error::<T>::Unauthorized.into());
            }

            let pallet_account = Self::account_id();

            // Get pool account
            let pool_account = PoolXYK::<T>::properties_of_pool(XOR.into(), asset_id)
                .ok_or(Error::<T>::PoolDoesNotExist)?
                .0;

            // Transfer LP tokens
            PoolXYK::<T>::transfer_lp_tokens(
                pool_account.clone(),
                XOR.into(),
                asset_id,
                pallet_account,
                user.clone(),
                ilo_info.lp_tokens,
            )?;

            ilo_info.claimed_lp_tokens = true;

            // Update storage
            <ILOs<T>>::insert(&asset_id, &ilo_info);

            // Emit an event
            Self::deposit_event(Event::ClaimedLP(user.clone(), asset_id));

            // Return a successful DispatchResult
            Ok(().into())
        }

        #[pallet::weight(10000)]
        pub fn claim(origin: OriginFor<T>, asset_id: AssetIdOf<T>) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            // Get ILO info
            let ilo_info = <ILOs<T>>::get(&asset_id);

            // Check if ILO for token exists
            ensure!(ilo_info.ilo_price != 0, Error::<T>::ILODoesNotExist);

            if !ilo_info.failed && !ilo_info.succeeded {
                return Err(Error::<T>::ILOIsNotFinished.into());
            }

            // Get contribution info
            let mut contribution_info = <Contributions<T>>::get(&asset_id, &user);
            ensure!(
                contribution_info.claiming_finished == false,
                Error::<T>::FundsAlreadyClaimed
            );

            let pallet_account = Self::account_id();

            // ILO failed
            if ilo_info.failed {
                // Claim unused funds
                Assets::<T>::transfer_from(
                    &XOR.into(),
                    &pallet_account,
                    &user,
                    contribution_info.funds_contributed,
                )?;
                contribution_info.claiming_finished = true;
            } else {
                // First claim
                if contribution_info.tokens_claimed == balance!(0) {
                    let tokens_to_claim = (FixedWrapper::from(contribution_info.tokens_bought)
                        * FixedWrapper::from(ilo_info.token_vesting.first_release_percent))
                    .try_into_balance()
                    .unwrap_or(0);
                    // Claim first time
                    Assets::<T>::transfer_from(
                        &asset_id.into(),
                        &pallet_account,
                        &user,
                        tokens_to_claim,
                    )?;
                    contribution_info.tokens_claimed += tokens_to_claim;
                    if ilo_info.token_vesting.first_release_percent == balance!(1) {
                        contribution_info.claiming_finished = true;
                    }
                } else {
                    // Claim the rest parts
                    let current_block = frame_system::Pallet::<T>::block_number();
                    let blocks_passed = current_block.saturating_sub(ilo_info.finish_block);

                    let potential_claims: u32 = blocks_passed
                        .checked_div(&ilo_info.token_vesting.vesting_period)
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
                        * FixedWrapper::from(ilo_info.token_vesting.vesting_percent))
                    .try_into_balance()
                    .unwrap_or(0);
                    let claimable = (FixedWrapper::from(tokens_per_claim)
                        * FixedWrapper::from(balance!(allowed_claims)))
                    .try_into_balance()
                    .unwrap_or(0);

                    // Claim tokens
                    Assets::<T>::transfer_from(
                        &asset_id.into(),
                        &pallet_account,
                        &user,
                        claimable,
                    )?;
                    contribution_info.tokens_claimed += claimable;
                    contribution_info.number_of_claims += allowed_claims;

                    let claimed_percent =
                        (FixedWrapper::from(ilo_info.token_vesting.vesting_percent)
                            * FixedWrapper::from(balance!(contribution_info.number_of_claims)))
                        .try_into_balance()
                        .unwrap_or(0)
                            + ilo_info.token_vesting.first_release_percent;

                    if claimed_percent == balance!(1) {
                        contribution_info.claiming_finished = true;
                    }
                }
            }

            <Contributions<T>>::insert(&asset_id, &user, contribution_info);

            // Emit an event
            Self::deposit_event(Event::Claimed(user.clone(), asset_id));

            Ok(().into())
        }

        /// Change CERES burn fee
        #[pallet::weight(10000)]
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
        #[pallet::weight(10000)]
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
        #[pallet::weight(10000)]
        pub fn claim_pswap_rewards(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let user = ensure_signed(origin)?;

            if user != AuthorityAccount::<T>::get() {
                return Err(Error::<T>::Unauthorized.into());
            }

            let pallet_account = Self::account_id();
            PSWAPDistribution::<T>::claim_incentive(
                RawOrigin::Signed(pallet_account.clone()).into(),
            )?;

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
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(now: T::BlockNumber) -> Weight {
            let mut counter: u64 = 0;

            if (now % T::BLOCKS_PER_ONE_DAY).is_zero() {
                let days_to_finish_ilo = 14u32;
                let pallet_account = Self::account_id();

                for ilo in <ILOs<T>>::iter() {
                    if now > ilo.1.end_block && !ilo.1.failed && !ilo.1.succeeded {
                        let finish_block = ilo.1.end_block
                            + (T::BLOCKS_PER_ONE_DAY.saturating_mul(days_to_finish_ilo.into()))
                                .into();
                        if now >= finish_block {
                            let mut ilo_info = <ILOs<T>>::get(&ilo.0);
                            ilo_info.failed = true;

                            let total_tokens =
                                ilo_info.tokens_for_liquidity + ilo_info.tokens_for_ilo;
                            if !ilo_info.refund_type {
                                let _ = Assets::<T>::burn(
                                    RawOrigin::Signed(pallet_account.clone()).into(),
                                    ilo.0.into(),
                                    total_tokens,
                                );
                            } else {
                                let _ = Assets::<T>::transfer_from(
                                    &ilo.0.into(),
                                    &pallet_account,
                                    &ilo_info.ilo_organizer,
                                    total_tokens,
                                );
                            }

                            <ILOs<T>>::insert(&ilo.0, ilo_info);
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
            PALLET_ID.into_account()
        }

        /// Check parameters
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
            start_block: T::BlockNumber,
            end_block: T::BlockNumber,
            current_block: T::BlockNumber,
            first_release_percent: Balance,
            vesting_period: T::BlockNumber,
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

            let tfi = ((FixedWrapper::from(hard_cap) / FixedWrapper::from(ilo_price))
                .get()
                .unwrap())
            .integral(RoundMode::Ceil);
            if tokens_for_ilo != balance!(tfi) {
                return Err(Error::<T>::InvalidNumberOfTokensForILO.into());
            }

            let tfl = ((FixedWrapper::from(hard_cap) * FixedWrapper::from(liquidity_percent))
                / FixedWrapper::from(listing_price))
            .get()
            .unwrap()
            .integral(RoundMode::Ceil);
            if tokens_for_liquidity != balance!(tfl) {
                return Err(Error::<T>::InvalidNumberOfTokensForLiquidity.into());
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
            if vesting_amount % vesting_percent != 0 {
                return Err(Error::<T>::InvalidVestingPercent.into());
            }

            if first_release_percent != one && vesting_period == 0u32.into() {
                return Err(Error::<T>::InvalidVestingPeriod.into());
            }

            Ok(().into())
        }
    }
}
