#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

use core::convert::TryInto;

use common::{
    balance, fixed, fixed_wrapper,
    fixnum::ops::Zero as _,
    prelude::{
        Balance, Error as CommonError, Fixed, FixedWrapper, QuoteAmount, SwapAmount, SwapOutcome,
    },
    prelude::{EnsureDEXManager, EnsureTradingPairExists},
    DEXId, LiquiditySource, LiquiditySourceFilter, LiquiditySourceType, ManagementMode, PSWAP, VAL,
};
use frame_support::traits::Get;
use frame_support::{decl_error, decl_event, decl_module, decl_storage, ensure, fail};
use frame_system::ensure_signed;
use liquidity_proxy::LiquidityProxyTrait;
use permissions::{Scope, BURN, MINT, SLASH, TRANSFER};
use pswap_distribution::{OnPswapBurned, PswapRemintInfo};
use sp_arithmetic::traits::Zero;
use sp_runtime::{DispatchError, DispatchResult};
use sp_std::collections::btree_set::BTreeSet;

pub trait Trait: common::Trait + assets::Trait + technical::Trait + trading_pair::Trait {
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
    type LiquidityProxy: LiquidityProxyTrait<Self::DEXId, Self::AccountId, Self::AssetId>;
    type EnsureDEXManager: EnsureDEXManager<Self::DEXId, Self::AccountId, DispatchError>;
    type EnsureTradingPairExists: EnsureTradingPairExists<Self::DEXId, Self::AssetId, DispatchError>;
}

type Assets<T> = assets::Module<T>;
type Technical<T> = technical::Module<T>;

pub const TECH_ACCOUNT_PREFIX: &[u8] = b"multicollateral-bonding-curve-pool";
pub const TECH_ACCOUNT_RESERVES: &[u8] = b"reserves";
pub const TECH_ACCOUNT_REWARDS: &[u8] = b"rewards";

// Reuse distribution account structs from single-collateral bonding curve pallet.
pub use bonding_curve_pool::DistributionAccountData;
pub use bonding_curve_pool::DistributionAccounts;

decl_storage! {
    trait Store for Module<T: Trait> as MulticollateralBondingCurve {
        /// Technical account used to store collateral tokens.
        pub ReservesAcc get(fn reserves_account_id) config(): T::TechAccountId;

        /// Buy price starting constant. This is the price users pay for new XOR.
        InitialPrice get(fn initial_price): Fixed = fixed!(200);

        /// Cofficients in buy price function.
        PriceChangeStep get(fn price_change_step): Fixed = fixed!(1337);
        PriceChangeRate get(fn price_change_rate): Fixed = fixed!(1);

        /// Sets the sell function as a fraction of the buy function, so there is margin between the two functions.
        SellPriceCoefficient get(fn sell_price_coefficient): Fixed = fixed!(0.8);

        /// Coefficient which determines the fraction of input collateral token to be exchanged to XOR and
        /// be distributed to predefined accounts. Relevant for the Buy function (when a user buys new XOR).
        AlwaysDistributeCoefficient get(fn always_distribute_coefficient): Fixed = fixed!(0.2);

        /// Base fee in XOR which is deducted on all trades, currently it's burned: 0.3%.
        BaseFee get(fn base_fee): Fixed = fixed!(0.003);

        /// Accounts that receive 20% buy/sell margin according predefined proportions.
        DistributionAccountsEntry get(fn distribution_accounts) config(): DistributionAccounts<DistributionAccountData<T::TechAccountId>>;

        /// Collateral Assets allowed to be sold on bonding curve.
        pub EnabledTargets get(fn enabled_targets): BTreeSet<T::AssetId>;

        /// Asset that is used to compare collateral assets by value, e.g., DAI.
        pub ReferenceAssetId get(fn reference_asset_id) config(): T::AssetId;

        /// Registry to store information about rewards owned by users in PSWAP. (claim_limit, available_rewards)
        pub Rewards get(fn rewards): map hasher(blake2_128_concat) T::AccountId => (Balance, Balance);

        /// Total amount of PSWAP owned by accounts.
        pub TotalRewards get(fn total_rewards): Balance;

        /// Number of reserve currencies selling which user will get rewards, namely all registered collaterals except PSWAP and VAL.
        pub IncentivisedCurrenciesNum get(fn incentivised_currencies_num): u32;

        /// Account which stores actual PSWAP intended for rewards.
        pub IncentivesAccountId get(fn incentives_account_id) config(): T::AccountId;

        /// Reward multipliers for special assets. Asset Id => Reward Multiplier
        pub AssetsWithOptionalRewardMultiplier: map hasher(twox_64_concat) T::AssetId => Option<Fixed>;

        /// Fraction of burned PSWAP that is used in reward vesting.
        /// TODO: this is wrong. It should be: min(0.55, 1−(−0.000357*(CURR_BLOCK_NUM÷14400)+0.9)−0.1).
        PswapBurnedDedicatedForRewards get(fn pswap_burned_dedicated_for_rewards): Fixed = fixed!(0.2);
    }
}

decl_error! {
    pub enum Error for Module<T: Trait> {
        /// An error occurred while calculating the price.
        PriceCalculationFailed,
        /// The pool can't perform exchange on itself.
        CannotExchangeWithSelf,
        /// It's not enough reserves in the pool to perform the operation.
        NotEnoughReserves,
        /// Attempt to initialize pool for pair that already exists.
        PoolAlreadyInitializedForPair,
        /// Attempt to get info for uninitialized pool.
        PoolNotInitialized,
        /// Indicated limits for slippage has not been met during transaction execution.
        SlippageLimitExceeded,
        /// Either user has no pending rewards or current limit is exceeded at the moment.
        NothingToClaim,
        /// User has pending reward, but rewards supply is insufficient at the moment.
        RewardsSupplyShortage,
        /// Indicated collateral asset is not enabled for pool.
        UnsupportedCollateralAssetId,
    }
}

decl_event!(
    pub enum Event<T>
    where
        DEXId = <T as common::Trait>::DEXId,
        AssetId = <T as assets::Trait>::AssetId,
    {
        /// Pool is initialized for pair. [DEX Id, Collateral Asset Id]
        PoolInitialized(DEXId, AssetId),
        /// Reference Asset has been changed for pool. [New Reference Asset Id]
        ReferenceAssetChanged(AssetId),
        /// Multiplier for reward has been updated on particular asset. [Asset Id, New Multiplier]
        OptionalRewardMultiplierUpdated(AssetId, Option<Fixed>),
    }
);

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        type Error = Error<T>;

        fn deposit_event() = default;

        /// Enable exchange path on the pool for pair BaseAsset-CollateralAsset.
        #[weight = 0]
        fn initialize_pool(origin, collateral_asset_id: T::AssetId) -> DispatchResult {
            let _who = <T as Trait>::EnsureDEXManager::ensure_can_manage(&DEXId::Polkaswap.into(), origin, ManagementMode::Private)?;
            Self::initialize_pool_unchecked(collateral_asset_id)
        }

        /// Change reference asset which is used to determine collateral assets value. Inteded to be e.g. stablecoin DAI.
        #[weight = 0]
        fn set_reference_asset(origin, reference_asset_id: T::AssetId) -> DispatchResult {
            let _who = <T as Trait>::EnsureDEXManager::ensure_can_manage(&DEXId::Polkaswap.into(), origin, ManagementMode::Private)?;
            ReferenceAssetId::<T>::put(reference_asset_id.clone());
            Self::deposit_event(RawEvent::ReferenceAssetChanged(reference_asset_id));
            Ok(())
        }

        /// Set multiplier which is applied to rewarded amount when depositing particular collateral assets.
        /// `None` value indicates reward without change, same as Some(1.0).
        #[weight = 0]
        fn set_optional_reward_multiplier(origin, collateral_asset_id: T::AssetId, multiplier: Option<Fixed>) -> DispatchResult {
            let _who = <T as Trait>::EnsureDEXManager::ensure_can_manage(&DEXId::Polkaswap.into(), origin, ManagementMode::Private)?;
            ensure!(Self::enabled_targets().contains(&collateral_asset_id), Error::<T>::UnsupportedCollateralAssetId);
            // NOTE: not using insert() here because it unwraps Option, which is not intended
            AssetsWithOptionalRewardMultiplier::<T>::mutate(&collateral_asset_id, |opt| *opt = multiplier.clone());
            Self::deposit_event(RawEvent::OptionalRewardMultiplierUpdated(collateral_asset_id, multiplier));
            Ok(())
        }

        #[weight = 0]
        fn claim_incentives(origin) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::claim_incentives_inner(&who)
        }
    }
}

/// This function is used by `exchange` function to transfer calculated `input_amount` of
/// `in_asset_id` to reserves and mint `output_amount` of `out_asset_id`.
///
/// This function always distributes a portion of input tokens (see `AlwaysDistributeCoefficient`), these are
/// referred as free reserves. After collateral input portion is exchanged to XOR, it's sent out to accounts
/// specified in `DistributionAccounts` struct and buy-back and burn some amount of VAL asset.
///
struct BuyMainAsset<T: Trait> {
    collateral_asset_id: T::AssetId,
    main_asset_id: T::AssetId,
    amount: SwapAmount<Balance>,
    from_account_id: T::AccountId,
    to_account_id: T::AccountId,
    reserves_tech_account_id: T::TechAccountId,
    reserves_account_id: T::AccountId,
}

impl<T: Trait> BuyMainAsset<T> {
    pub fn new(
        collateral_asset_id: T::AssetId,
        main_asset_id: T::AssetId,
        amount: SwapAmount<Balance>,
        from_account_id: T::AccountId,
        to_account_id: T::AccountId,
    ) -> Result<Self, DispatchError> {
        let reserves_tech_account_id = ReservesAcc::<T>::get();
        let reserves_account_id =
            Technical::<T>::tech_account_id_to_account_id(&reserves_tech_account_id)?;
        Ok(BuyMainAsset {
            collateral_asset_id,
            main_asset_id,
            amount,
            from_account_id,
            to_account_id,
            reserves_tech_account_id,
            reserves_account_id,
        })
    }

    /// Assets deposition algorithm:
    ///
    /// ```nocompile
    /// R_f := A_I * c
    /// R := R + A_I - R_f
    /// ```
    ///
    /// where:
    /// `R` - current reserves
    /// `R_f` - free reserves, that can be distributed
    /// `c` - free amount coefficient of extra reserves
    /// `A_I` - amount of the input asset
    ///
    /// Returns (free_reserves, (input_amount, output_amount, fee_amount))
    fn deposit_input(&self) -> Result<(Balance, (Balance, Balance, Balance)), DispatchError> {
        common::with_transaction(|| {
            let (input_amount, output_amount, fee_amount) = Module::<T>::decide_buy_amounts(
                &self.main_asset_id,
                &self.collateral_asset_id,
                self.amount,
            )?;
            Technical::<T>::transfer_in(
                &self.collateral_asset_id,
                &self.from_account_id,
                &self.reserves_tech_account_id,
                input_amount,
            )?;
            let free_amount = FixedWrapper::from(input_amount)
                * FixedWrapper::from(AlwaysDistributeCoefficient::get());
            let free_amount = free_amount
                .try_into_balance()
                .map_err(|_| Error::<T>::PriceCalculationFailed)?;
            Ok((free_amount, (input_amount, output_amount, fee_amount)))
        })
    }

    fn distribute_reserves(&self, free_amount: Balance) -> Result<(), DispatchError> {
        common::with_transaction(|| {
            if free_amount == Balance::zero() {
                return Ok(());
            }

            let reserves_tech_acc = &self.reserves_tech_account_id;
            let reserves_acc = &self.reserves_account_id;
            let swapped_xor_amount = T::LiquidityProxy::exchange(
                reserves_acc,
                reserves_acc,
                &self.collateral_asset_id,
                &self.main_asset_id,
                SwapAmount::with_desired_input(free_amount, Balance::zero()).into(),
                Module::<T>::self_excluding_filter(),
            )?
            .amount
            .into();
            Technical::<T>::burn(&self.main_asset_id, reserves_tech_acc, swapped_xor_amount)?;
            Technical::<T>::mint(&self.main_asset_id, reserves_tech_acc, swapped_xor_amount)?;

            let fw_swapped_xor_amount = FixedWrapper::from(swapped_xor_amount);

            let distribution_accounts: DistributionAccounts<
                DistributionAccountData<T::TechAccountId>,
            > = DistributionAccountsEntry::<T>::get();
            for (to_tech_account_id, coefficient) in distribution_accounts
                .xor_distribution_as_array()
                .iter()
                .map(|x| (&x.account_id, x.coefficient))
            {
                let amount = fw_swapped_xor_amount.clone() * coefficient;
                let amount = amount
                    .try_into_balance()
                    .map_err(|_| Error::<T>::PriceCalculationFailed)?;
                technical::Module::<T>::transfer(
                    &self.main_asset_id,
                    reserves_tech_acc,
                    to_tech_account_id,
                    amount,
                )?;
            }
            let amount =
                fw_swapped_xor_amount.clone() * distribution_accounts.val_holders.coefficient;
            let amount = amount
                .try_into_balance()
                .map_err(|_| Error::<T>::PriceCalculationFailed)?;
            let val_amount = T::LiquidityProxy::exchange(
                reserves_acc,
                reserves_acc,
                &self.main_asset_id,
                &VAL.into(),
                SwapAmount::with_desired_input(amount, Balance::zero()),
                Module::<T>::self_excluding_filter(),
            )?
            .amount;
            Technical::<T>::burn(&VAL.into(), reserves_tech_acc, val_amount)?;
            Ok(())
        })
    }

    fn mint_output(&self, output_amount: Balance) -> Result<(), DispatchError> {
        Assets::<T>::mint_to(
            &self.main_asset_id,
            &self.reserves_account_id,
            &self.to_account_id,
            output_amount,
        )?;
        Ok(())
    }

    fn update_reward(
        &self,
        collateral_asset_amount: Balance,
        main_asset_amount: Balance,
    ) -> Result<(), DispatchError> {
        let mut pswap_amount = Module::<T>::calculate_buy_reward(
            &self.reserves_account_id,
            &self.collateral_asset_id,
            collateral_asset_amount,
            main_asset_amount,
        )?;
        if let Some(multiplier) =
            AssetsWithOptionalRewardMultiplier::<T>::get(&self.collateral_asset_id)
        {
            pswap_amount = (FixedWrapper::from(pswap_amount) * multiplier)
                .try_into_balance()
                .map_err(|_| Error::<T>::PriceCalculationFailed)?;
        }
        if !pswap_amount.is_zero() {
            Rewards::<T>::mutate(&self.from_account_id, |(_, ref mut available)| {
                *available = available.saturating_add(pswap_amount)
            });
            TotalRewards::mutate(|balance| *balance = balance.saturating_add(pswap_amount));
        }
        Ok(())
    }

    fn swap(&self) -> Result<SwapOutcome<Balance>, DispatchError> {
        common::with_transaction(|| {
            let (input_amount_free, (input_amount, output_amount, fee)) = self.deposit_input()?;
            self.distribute_reserves(input_amount_free)?;
            self.mint_output(output_amount.clone())?;
            self.update_reward(input_amount.clone(), output_amount.clone())?;
            Ok(match self.amount {
                SwapAmount::WithDesiredInput { .. } => SwapOutcome::new(output_amount, fee),
                SwapAmount::WithDesiredOutput { .. } => SwapOutcome::new(input_amount, fee),
            })
        })
    }
}

#[allow(non_snake_case)]
impl<T: Trait> Module<T> {
    #[inline]
    fn self_excluding_filter() -> LiquiditySourceFilter<T::DEXId, LiquiditySourceType> {
        LiquiditySourceFilter::with_forbidden(
            DEXId::Polkaswap.into(),
            [LiquiditySourceType::MulticollateralBondingCurvePool].into(),
        )
    }

    fn initialize_pool_unchecked(collateral_asset_id: T::AssetId) -> DispatchResult {
        common::with_transaction(|| {
            ensure!(
                !EnabledTargets::<T>::get().contains(&collateral_asset_id),
                Error::<T>::PoolAlreadyInitializedForPair
            );
            T::EnsureTradingPairExists::ensure_trading_pair_exists(
                &DEXId::Polkaswap.into(),
                &T::GetBaseAssetId::get(),
                &collateral_asset_id,
            )?;
            trading_pair::Module::<T>::enable_source_for_trading_pair(
                &DEXId::Polkaswap.into(),
                &T::GetBaseAssetId::get(),
                &collateral_asset_id,
                LiquiditySourceType::MulticollateralBondingCurvePool,
            )?;
            if Self::collateral_is_incentivised(&collateral_asset_id) {
                IncentivisedCurrenciesNum::mutate(|num| *num += 1)
            }
            EnabledTargets::<T>::mutate(|set| set.insert(collateral_asset_id));
            Self::deposit_event(RawEvent::PoolInitialized(
                DEXId::Polkaswap.into(),
                collateral_asset_id,
            ));
            Ok(())
        })
    }

    /// Buy function with regards to asset total supply and its change delta. It represents the amount of
    /// input collateral required from User in order to receive requested XOR amount. I.e. the price User buys at.
    ///
    /// XOR is also referred as main asset.
    /// Value of `delta` is assumed to be either positive or negative.
    /// For every `PC_S` tokens the price goes up by `PC_R`.
    ///
    /// `P_BM1(Q) = (Q + q) / (PC_S * PC_R) + P_I`
    ///
    /// where
    /// `P_BM1(Q)`: buy price for one asset
    /// `P_I`: initial asset price
    /// `PC_R`: price change rate
    /// `PC_S`: price change step
    /// `Q`: asset issuance (quantity)
    /// `q`: asset issuance change
    pub fn buy_function(main_asset_id: &T::AssetId, delta: Fixed) -> Result<Fixed, DispatchError> {
        let total_issuance = Assets::<T>::total_issuance(main_asset_id)?;
        let Q: FixedWrapper = total_issuance.into();
        let P_I = Self::initial_price();
        let PC_S = Self::price_change_step();
        let PC_R: FixedWrapper = Self::price_change_rate().into();
        let price = (Q + delta) / (PC_S * PC_R) + P_I;
        price
            .get()
            .map_err(|_| Error::<T>::PriceCalculationFailed.into())
    }

    /// Calculates and returns the current buy price, assuming that input is the collateral asset and output is the main asset.
    ///
    /// To calculate price for a specific amount of assets (with desired main asset output),
    /// one needs to calculate the area of a right trapezoid.
    ///
    ///
    /// 1) Amount of collateral tokens needed in USD to get `q` XOR tokens
    /// ```nocompile
    /// ((AB + CD) / 2) * BH
    /// In our case:
    /// `q` is main asset supply delta.
    /// ```
    /// 2) Amount of XOR tokens received by depositing `S` collateral tokens in USD
    /// Solving right trapezoid area formula with respect to delta `q`,
    /// actual square `S` is known and represents collateral amount.
    /// We have a quadratic equation:
    /// ```nocompile
    /// buy_function(x) = M*x + P_I
    ///
    /// M * q² + 2 * AB * q - 2 * S = 0
    /// equation with two solutions, taking only positive one:
    /// q = (√((AB * 2 / M)² + 8 * S / M) - 2 * AB / M) / 2
    /// ```
    /// where
    /// `Q`: current asset issuance (quantity)
    /// `q`: asset issuance delta (deposited/received quantity)
    /// `M` : buy function slope coefficient
    /// `P_I`: initial asset price
    /// `AB` : buy_function(Q)
    /// `CD` : buy_function(Q + q)
    /// `BH` : = q
    pub fn buy_price(
        main_asset_id: &T::AssetId,
        collateral_asset_id: &T::AssetId,
        quantity: QuoteAmount<Balance>,
    ) -> Result<Fixed, DispatchError> {
        let PC_S = FixedWrapper::from(Self::price_change_step()); // price change step
        let PC_R = Self::price_change_rate(); // price change rate
        let PC = PC_S * PC_R; // price change

        let current_state: FixedWrapper = Self::buy_function(main_asset_id, Fixed::ZERO)?.into();
        let collateral_price_per_reference_unit: FixedWrapper =
            Self::reference_price(collateral_asset_id)?.into();

        match quantity {
            QuoteAmount::WithDesiredInput {
                desired_amount_in: collateral_quantity,
            } => {
                let collateral_reference_in =
                    collateral_price_per_reference_unit * collateral_quantity;

                let under_pow = current_state.clone() * PC.clone() * fixed_wrapper!(2.0);
                let under_sqrt = under_pow.clone() * under_pow
                    + fixed_wrapper!(8.0) * PC.clone() * collateral_reference_in;
                let main_out =
                    under_sqrt.sqrt_accurate() / fixed_wrapper!(2.0) - PC * current_state;
                main_out
                    .get()
                    .map_err(|_| Error::<T>::PriceCalculationFailed.into())
                    .map(|value| value.max(Fixed::ZERO))
            }
            QuoteAmount::WithDesiredOutput {
                desired_amount_out: main_quantity,
            } => {
                let new_state: FixedWrapper = Self::buy_function(
                    main_asset_id,
                    FixedWrapper::from(main_quantity)
                        .get()
                        .map_err(|_| Error::<T>::PriceCalculationFailed)?,
                )?
                .into();
                let collateral_reference_in =
                    ((current_state + new_state) / fixed_wrapper!(2.0)) * main_quantity;
                let collateral_quantity =
                    collateral_reference_in / collateral_price_per_reference_unit;
                collateral_quantity
                    .get()
                    .map_err(|_| Error::<T>::PriceCalculationFailed.into())
                    .map(|value| value.max(Fixed::ZERO))
            }
        }
    }

    /// Calculates and returns the current sell price, assuming that input is the main asset and output is the collateral asset.
    ///
    /// To calculate sell price for a specific amount of assets:
    /// 1. Current reserves of collateral token are taken
    /// 2. Same amount by value is assumed for main asset
    ///   2.1 Values are compared via getting prices for both main and collateral tokens with regard to another token
    ///       called reference token which is set for particular pair. This should be e.g. stablecoin DAI.
    ///   2.2 Reference price for base token is taken as 80% of current bonding curve buy price.
    ///   2.3 Reference price for collateral token is taken as current market price, i.e. price for 1 token on liquidity proxy.
    /// 3. Given known reserves for main and collateral, output collateral amount is calculated by applying x*y=k model resulting
    ///    in curve-like dependency.
    pub fn sell_price(
        main_asset_id: &T::AssetId,
        collateral_asset_id: &T::AssetId,
        quantity: QuoteAmount<Balance>,
    ) -> Result<Fixed, DispatchError> {
        let reserves_tech_account_id = ReservesAcc::<T>::get();
        let reserves_account_id =
            Technical::<T>::tech_account_id_to_account_id(&reserves_tech_account_id)?;
        let collateral_supply: FixedWrapper =
            Assets::<T>::free_balance(collateral_asset_id, &reserves_account_id)?.into();
        // Get reference prices for base and collateral to understand token value.
        let main_price_per_reference_unit: FixedWrapper =
            Self::sell_function(main_asset_id, Fixed::ZERO)?.into();
        let collateral_price_per_reference_unit: FixedWrapper =
            Self::reference_price(collateral_asset_id)?.into();
        // Assume main token reserve is equal by reference value to collateral token reserve.
        let main_supply = collateral_supply.clone() * collateral_price_per_reference_unit
            / main_price_per_reference_unit;
        let collateral_supply_unwrapped = collateral_supply
            .clone()
            .get()
            .map_err(|_| Error::<T>::PriceCalculationFailed)?;

        match quantity {
            QuoteAmount::WithDesiredInput {
                desired_amount_in: quantity_main,
            } => {
                let output_collateral =
                    (quantity_main * collateral_supply) / (main_supply + quantity_main);
                let output_collateral_unwrapped = output_collateral
                    .get()
                    .map_err(|_| Error::<T>::PriceCalculationFailed)?;
                ensure!(
                    output_collateral_unwrapped < collateral_supply_unwrapped,
                    Error::<T>::NotEnoughReserves
                );
                Ok(output_collateral_unwrapped)
            }
            QuoteAmount::WithDesiredOutput {
                desired_amount_out: quantity_collateral,
            } => {
                let collateral_supply_unwrapped = collateral_supply_unwrapped
                    .into_bits()
                    .try_into()
                    .map_err(|_| Error::<T>::PriceCalculationFailed)?;
                ensure!(
                    quantity_collateral < collateral_supply_unwrapped,
                    Error::<T>::NotEnoughReserves
                );
                let output_main =
                    (main_supply * quantity_collateral) / (collateral_supply - quantity_collateral);
                output_main
                    .get()
                    .map_err(|_| Error::<T>::PriceCalculationFailed.into())
            }
        }
    }

    /// Sell function with regards to asset total supply and its change delta. It represents the amount of
    /// output collateral tokens received by User by indicating exact sold XOR amount. I.e. the price User sells at.
    ///
    /// Value of `delta` is assumed to be either positive or negative.
    /// Sell function is `P_Sc`% of buy function (see `buy_function`).
    ///
    /// `P_S = P_Sc * P_B`
    /// where
    /// `P_Sc: sell price coefficient (%)`
    pub fn sell_function(main_asset_id: &T::AssetId, delta: Fixed) -> Result<Fixed, DispatchError> {
        let P_B = Self::buy_function(main_asset_id, delta)?;
        let P_Sc = FixedWrapper::from(Self::sell_price_coefficient());
        let price = P_Sc * P_B;
        price
            .get()
            .map_err(|_| Error::<T>::PriceCalculationFailed.into())
    }

    /// Decompose SwapAmount into particular buy quotation query.
    ///
    /// Returns ordered pair: (input_amount, output_amount, fee_amount).
    fn decide_buy_amounts(
        main_asset_id: &T::AssetId,
        collateral_asset_id: &T::AssetId,
        amount: SwapAmount<Balance>,
    ) -> Result<(Balance, Balance, Balance), DispatchError> {
        Ok(match amount {
            SwapAmount::WithDesiredInput {
                desired_amount_in,
                min_amount_out,
            } => {
                let mut output_amount: Balance = FixedWrapper::from(Self::buy_price(
                    main_asset_id,
                    collateral_asset_id,
                    QuoteAmount::with_desired_input(desired_amount_in),
                )?)
                .try_into_balance()
                .map_err(|_| Error::<T>::PriceCalculationFailed)?;
                let fee_amount = (FixedWrapper::from(BaseFee::get()) * output_amount)
                    .try_into_balance()
                    .map_err(|_| Error::<T>::PriceCalculationFailed)?;
                output_amount = output_amount.saturating_sub(fee_amount);
                ensure!(
                    output_amount >= min_amount_out,
                    Error::<T>::SlippageLimitExceeded
                );
                (desired_amount_in, output_amount, fee_amount)
            }
            SwapAmount::WithDesiredOutput {
                desired_amount_out,
                max_amount_in,
            } => {
                let desired_amount_out_with_fee = (FixedWrapper::from(desired_amount_out)
                    / (fixed_wrapper!(1) - BaseFee::get()))
                .try_into_balance()
                .map_err(|_| Error::<T>::PriceCalculationFailed)?;
                let input_amount = Self::buy_price(
                    main_asset_id,
                    collateral_asset_id,
                    QuoteAmount::with_desired_output(desired_amount_out_with_fee.clone()),
                )?;
                let input_amount = input_amount
                    .into_bits()
                    .try_into()
                    .map_err(|_| Error::<T>::PriceCalculationFailed)?;
                ensure!(
                    input_amount <= max_amount_in,
                    Error::<T>::SlippageLimitExceeded
                );
                (
                    input_amount,
                    desired_amount_out,
                    desired_amount_out_with_fee.saturating_sub(desired_amount_out),
                )
            }
        })
    }

    /// Decompose SwapAmount into particular sell quotation query.
    ///
    /// Returns ordered pair: (input_amount, output_amount, fee_amount).
    fn decide_sell_amounts(
        main_asset_id: &T::AssetId,
        collateral_asset_id: &T::AssetId,
        amount: SwapAmount<Balance>,
    ) -> Result<(Balance, Balance, Balance), DispatchError> {
        Ok(match amount {
            SwapAmount::WithDesiredInput {
                desired_amount_in,
                min_amount_out,
            } => {
                let fee_amount = (FixedWrapper::from(BaseFee::get())
                    * FixedWrapper::from(desired_amount_in))
                .try_into_balance()
                .map_err(|_| Error::<T>::PriceCalculationFailed)?;
                let output_amount = Self::sell_price(
                    main_asset_id,
                    collateral_asset_id,
                    QuoteAmount::with_desired_input(
                        desired_amount_in.saturating_sub(fee_amount.clone()),
                    ),
                )?;
                let output_amount = output_amount
                    .into_bits()
                    .try_into()
                    .map_err(|_| Error::<T>::PriceCalculationFailed)?;
                ensure!(
                    output_amount >= min_amount_out,
                    Error::<T>::SlippageLimitExceeded
                );
                (desired_amount_in, output_amount, fee_amount)
            }
            SwapAmount::WithDesiredOutput {
                desired_amount_out,
                max_amount_in,
            } => {
                let input_amount: Balance = FixedWrapper::from(Self::sell_price(
                    main_asset_id,
                    collateral_asset_id,
                    QuoteAmount::with_desired_output(desired_amount_out),
                )?)
                .try_into_balance()
                .map_err(|_| Error::<T>::PriceCalculationFailed)?;
                let input_amount_with_fee =
                    FixedWrapper::from(input_amount) / (fixed_wrapper!(1) - BaseFee::get());
                let input_amount_with_fee = input_amount_with_fee
                    .try_into_balance()
                    .map_err(|_| Error::<T>::PriceCalculationFailed)?;
                ensure!(
                    input_amount <= max_amount_in,
                    Error::<T>::SlippageLimitExceeded
                );
                (
                    input_amount_with_fee,
                    desired_amount_out,
                    input_amount_with_fee.saturating_sub(input_amount),
                )
            }
        })
    }

    /// This function is used by `exchange` function to burn `input_amount` derived from `amount` of `main_asset_id`
    /// and transfer calculated amount of `collateral_asset_id` to the receiver from reserves.
    ///
    /// If there's not enough reserves in the pool, `NotEnoughReserves` error will be returned.
    ///
    fn sell_main_asset(
        _dex_id: &T::DEXId,
        main_asset_id: &T::AssetId,
        collateral_asset_id: &T::AssetId,
        amount: SwapAmount<Balance>,
        from_account_id: &T::AccountId,
        to_account_id: &T::AccountId,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        common::with_transaction(|| {
            let reserves_tech_account_id = Self::reserves_account_id();
            let reserves_account_id =
                Technical::<T>::tech_account_id_to_account_id(&reserves_tech_account_id)?;
            let (input_amount, output_amount, fee_amount) =
                Self::decide_sell_amounts(main_asset_id, collateral_asset_id, amount)?;
            let reserves_amount =
                Assets::<T>::total_balance(collateral_asset_id, &reserves_account_id)?;
            ensure!(
                reserves_amount >= output_amount,
                Error::<T>::NotEnoughReserves
            );
            technical::Module::<T>::transfer_out(
                collateral_asset_id,
                &reserves_tech_account_id,
                &to_account_id,
                output_amount,
            )?;
            Assets::<T>::burn_from(
                main_asset_id,
                &reserves_account_id,
                from_account_id,
                input_amount,
            )?;
            Ok(SwapOutcome::new(output_amount, fee_amount))
        })
    }

    pub fn set_reserves_account_id(account: T::TechAccountId) -> Result<(), DispatchError> {
        common::with_transaction(|| {
            ReservesAcc::<T>::set(account.clone());
            let account_id = Technical::<T>::tech_account_id_to_account_id(&account)?;
            let permissions = [BURN, MINT, TRANSFER, SLASH];
            for permission in &permissions {
                permissions::Module::<T>::assign_permission(
                    account_id.clone(),
                    &account_id,
                    *permission,
                    Scope::Unlimited,
                )?;
            }
            Ok(())
        })
    }

    pub fn set_distribution_accounts(
        distribution_accounts: DistributionAccounts<DistributionAccountData<T::TechAccountId>>,
    ) {
        DistributionAccountsEntry::<T>::set(distribution_accounts);
    }

    /// This function is used to determine particular asset price in terms of a reference asset, which is set for
    /// bonding curve (there could be only single token chosen as reference for all comparisons). Basically, the
    /// reference token is expected to be a USD-bound stablecoin, e.g. DAI.
    ///
    /// Example use: understand actual value of two tokens in terms of USD.
    fn reference_price(asset_id: &T::AssetId) -> Result<Balance, DispatchError> {
        let reference_asset_id = ReferenceAssetId::<T>::get();
        let price = if asset_id == &reference_asset_id {
            balance!(1)
        } else {
            T::LiquidityProxy::quote(
                asset_id,
                &reference_asset_id,
                SwapAmount::with_desired_input(balance!(1), Balance::zero()),
                Self::self_excluding_filter(),
            )?
            .amount
        };
        Ok(price)
    }

    fn actual_reserves_reference_price(
        reserves_account_id: &T::AccountId,
        collateral_asset_id: &T::AssetId,
    ) -> Result<Balance, DispatchError> {
        let reserve = Assets::<T>::free_balance(&collateral_asset_id, &reserves_account_id)?;
        let price = Self::reference_price(&collateral_asset_id)?;
        (FixedWrapper::from(reserve) * price)
            .try_into_balance()
            .map_err(|_| Error::<T>::PriceCalculationFailed.into())
    }

    fn ideal_reserves_reference_price(delta: Fixed) -> Result<Balance, DispatchError> {
        let base_asset_id = T::GetBaseAssetId::get();
        let base_total_supply = Assets::<T>::total_issuance(&base_asset_id)?;
        let initial_state =
            FixedWrapper::from(Self::initial_price()) * Self::sell_price_coefficient();
        let current_state = Self::sell_function(&base_asset_id, delta)?;

        let price = (initial_state + current_state) / fixed_wrapper!(2.0)
            * FixedWrapper::from(base_total_supply);
        price
            .try_into_balance()
            .map_err(|_| Error::<T>::PriceCalculationFailed.into())
    }

    /// ideal = sell_function(0 to xor_total_supply)
    /// actual_before = input_currency_reserve * currency_usd_price
    /// actual_after = actual_before + input_currency_amount * input_currency_usd_price
    /// a = ideal_before - actual_before
    /// b = ideal_after - actual_after
    /// N = enabled reserve currencies except PSWAP and VAL
    /// reward_usd = (a - b) * (mean(a, b) / ideal_after) / (N * 3)
    pub fn calculate_buy_reward(
        reserves_account_id: &T::AccountId,
        collateral_asset_id: &T::AssetId,
        collateral_asset_amount: Balance,
        main_asset_amount: Balance,
    ) -> Result<Balance, DispatchError> {
        if !Self::collateral_is_incentivised(collateral_asset_id) {
            return Ok(Balance::zero());
        }

        // State values.
        let ideal_before: FixedWrapper = Self::ideal_reserves_reference_price(Fixed::ZERO)?.into();
        let ideal_after: FixedWrapper = Self::ideal_reserves_reference_price(
            FixedWrapper::from(main_asset_amount)
                .get()
                .map_err(|_| Error::<T>::PriceCalculationFailed)?,
        )?
        .into();
        let actual_before: FixedWrapper =
            Self::actual_reserves_reference_price(reserves_account_id, collateral_asset_id)?.into();
        let actual_after = actual_before.clone()
            + FixedWrapper::from(collateral_asset_amount)
                * Self::reference_price(collateral_asset_id)?;
        let N: u128 = IncentivisedCurrenciesNum::get().into();
        let N: FixedWrapper = FixedWrapper::from(N * balance!(1));

        // Calculate rewards in USD.
        let a = ideal_before.clone() - actual_before;
        let b = ideal_after.clone() - actual_after;
        let mean_ab = (a.clone() + b.clone()) / fixed_wrapper!(2);
        let first_term = a - b;
        let second_term = mean_ab / ideal_before;
        let reference_amount = (first_term * second_term) / (N * fixed_wrapper!(3));

        // Convert to PSWAP.
        let pswap_price = Self::reference_price(&PSWAP.into())?;
        let pswap_amount = reference_amount / FixedWrapper::from(pswap_price);
        pswap_amount
            .try_into_balance()
            .map_err(|_| Error::<T>::PriceCalculationFailed.into())
    }

    fn collateral_is_incentivised(collateral_asset_id: &T::AssetId) -> bool {
        collateral_asset_id != &PSWAP.into() && collateral_asset_id != &VAL.into()
    }

    fn claim_incentives_inner(account_id: &T::AccountId) -> DispatchResult {
        common::with_transaction(|| {
            let (rewards_limit, rewards_owned) = Rewards::<T>::get(account_id);
            let pswap_asset_id = PSWAP.into();
            let incentives_account_id = IncentivesAccountId::<T>::get();
            let available_rewards =
                Assets::<T>::free_balance(&pswap_asset_id, &incentives_account_id)?;
            let mut to_claim = rewards_limit.min(rewards_owned);
            ensure!(!to_claim.is_zero(), Error::<T>::NothingToClaim);
            to_claim = to_claim.min(available_rewards);
            ensure!(!to_claim.is_zero(), Error::<T>::RewardsSupplyShortage);
            Assets::<T>::transfer_from(
                &pswap_asset_id,
                &incentives_account_id,
                &account_id,
                to_claim,
            )?;
            Rewards::<T>::insert(
                account_id,
                (
                    rewards_limit.saturating_sub(to_claim),
                    rewards_owned.saturating_sub(to_claim),
                ),
            );
            TotalRewards::mutate(|balance| *balance = balance.saturating_sub(to_claim));
            Ok(())
        })
    }
}

impl<T: Trait> OnPswapBurned for Module<T> {
    fn on_pswap_burned(distribution: PswapRemintInfo) {
        let total_rewards = TotalRewards::get();
        let amount = FixedWrapper::from(distribution.vesting);

        // TODO: do we use full vesting amount here or portion as before?
        // let amount =
        //     FixedWrapper::from(amount) * FixedWrapper::from(PswapBurnedDedicatedForRewards::get());s

        if !total_rewards.is_zero() {
            Rewards::<T>::translate(|_key: T::AccountId, value: (Balance, Balance)| {
                let (limit, owned) = value;
                let limit_to_add =
                    FixedWrapper::from(owned) * amount.clone() / FixedWrapper::from(total_rewards);
                let new_limit = (limit_to_add + FixedWrapper::from(limit))
                    .try_into_balance()
                    .unwrap_or(limit);
                Some((new_limit, owned))
            })
        }
    }
}

impl<T: Trait> LiquiditySource<T::DEXId, T::AccountId, T::AssetId, Balance, DispatchError>
    for Module<T>
{
    fn can_exchange(
        dex_id: &T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
    ) -> bool {
        if *dex_id != DEXId::Polkaswap.into() {
            return false;
        }
        if input_asset_id == &T::GetBaseAssetId::get() {
            EnabledTargets::<T>::get().contains(&output_asset_id)
        } else {
            EnabledTargets::<T>::get().contains(&input_asset_id)
        }
    }

    fn quote(
        dex_id: &T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        swap_amount: SwapAmount<Balance>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        if !Self::can_exchange(dex_id, input_asset_id, output_asset_id) {
            fail!(CommonError::<T>::CantExchange);
        }
        let base_asset_id = &T::GetBaseAssetId::get();
        let (input_amount, output_amount, fee_amount) = if input_asset_id == base_asset_id {
            Self::decide_sell_amounts(&input_asset_id, &output_asset_id, swap_amount)?
        } else {
            Self::decide_buy_amounts(&output_asset_id, &input_asset_id, swap_amount)?
        };
        match swap_amount {
            SwapAmount::WithDesiredInput { .. } => Ok(SwapOutcome::new(output_amount, fee_amount)),
            SwapAmount::WithDesiredOutput { .. } => Ok(SwapOutcome::new(input_amount, fee_amount)),
        }
    }

    fn exchange(
        sender: &T::AccountId,
        receiver: &T::AccountId,
        dex_id: &T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        desired_amount: SwapAmount<Balance>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        if !Self::can_exchange(dex_id, input_asset_id, output_asset_id) {
            fail!(CommonError::<T>::CantExchange);
        }
        let reserves_account_id =
            &Technical::<T>::tech_account_id_to_account_id(&Self::reserves_account_id())?;
        // This is needed to prevent recursion calls.
        if sender == reserves_account_id && receiver == reserves_account_id {
            fail!(Error::<T>::CannotExchangeWithSelf);
        }
        let base_asset_id = &T::GetBaseAssetId::get();
        if input_asset_id == base_asset_id {
            Self::sell_main_asset(
                dex_id,
                input_asset_id,
                output_asset_id,
                desired_amount,
                sender,
                receiver,
            )
        } else {
            BuyMainAsset::<T>::new(
                *input_asset_id,
                *output_asset_id,
                desired_amount,
                sender.clone(),
                receiver.clone(),
            )?
            .swap()
        }
    }
}
