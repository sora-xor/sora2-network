#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

use common::{
    fixed,
    fixnum::ops::Numeric,
    prelude::{
        Balance, Error as CommonError, Fixed, FixedWrapper, QuoteAmount, SwapAmount, SwapOutcome,
    },
    prelude::{EnsureDEXManager, EnsureTradingPairExists},
    DEXId, LiquiditySource, LiquiditySourceFilter, LiquiditySourceType, ManagementMode, VAL,
};
use frame_support::traits::Get;
use frame_support::{decl_error, decl_event, decl_module, decl_storage, ensure, fail};
use liquidity_proxy::LiquidityProxyTrait;
use permissions::{Scope, BURN, MINT, SLASH, TRANSFER};
use sp_arithmetic::traits::{One, Zero};
use sp_runtime::{DispatchError, DispatchResult};
use sp_std::collections::btree_set::BTreeSet;

pub trait Trait: common::Trait + assets::Trait + technical::Trait {
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
    type LiquidityProxy: LiquidityProxyTrait<Self::DEXId, Self::AccountId, Self::AssetId>;
    type EnsureDEXManager: EnsureDEXManager<Self::DEXId, Self::AccountId, DispatchError>;
    type EnsureTradingPairExists: EnsureTradingPairExists<Self::DEXId, Self::AssetId, DispatchError>;
}

type TradingPair<T> = common::prelude::TradingPair<<T as assets::Trait>::AssetId>;
type Assets<T> = assets::Module<T>;
type Technical<T> = technical::Module<T>;

pub const TECH_ACCOUNT_PREFIX: &[u8] = b"multicollateral-bonding-curve-pool";
pub const TECH_ACCOUNT_RESERVES: &[u8] = b"reserves";

pub use bonding_curve_pool::DistributionAccountData;
pub use bonding_curve_pool::DistributionAccounts;

decl_storage! {
    trait Store for Module<T: Trait> as MulticollateralBondingCurve {
        pub ReservesAcc get(fn reserves_account_id) config(): T::TechAccountId;
        Fee get(fn fee): Fixed = fixed!(0.001);
        InitialPrice get(fn initial_price): Fixed = fixed!(200);
        PriceChangeStep get(fn price_change_step): Fixed = fixed!(1337);
        PriceChangeRate get(fn price_change_rate): Fixed = fixed!(1);
        SellPriceCoefficient get(fn sell_price_coefficient): Fixed = fixed!(0.8);
        DistributionAccountsEntry get(fn distribution_accounts) config(): DistributionAccounts<DistributionAccountData<T::TechAccountId>>;
        pub EnabledPairs get(fn enabled_pairs): BTreeSet<TradingPair<T>>;
        pub ReferenceAssetId get(fn reference_asset_id) config(): T::AssetId;
    }
}

decl_error! {
    pub enum Error for Module<T: Trait> {
        /// An error occurred while calculating the price.
        CalculatePriceFailed,
        /// The pool can't perform exchange on itself.
        CantExchangeOnItself,
        /// It's not enough reserves in the pool to perform the operation.
        NotEnoughReserves,
        /// Attempt to initialize pool for pair that already exists.
        PoolAlreadyInitializedForPair,
        /// Attempt to get info for uninitialized pool.
        PoolNotInitialized,
        /// Indicated limits for slippage has not been met during transaction execution.
        SlippageFailed,
    }
}

decl_event!(
    pub enum Event<T>
    where
        DEXId = <T as common::Trait>::DEXId,
        TradingPair = TradingPair<T>,
    {
        /// Pool is initialized for pair. [DEX Id, Trading Pair]
        PoolInitialized(DEXId, TradingPair),
    }
);

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        type Error = Error<T>;

        fn deposit_event() = default;

        #[weight = 0]
        fn initialize_pool(origin, base_asset_id: T::AssetId, target_asset_id: T::AssetId) -> DispatchResult {
            let _who = T::EnsureDEXManager::ensure_can_manage(&DEXId::Polkaswap.into(), origin, ManagementMode::Private)?;
            Self::initialize_pool_unchecked(base_asset_id, target_asset_id)
        }

        #[weight = 0]
        fn set_reference_asset(origin, reference_asset_id: T::AssetId) -> DispatchResult {
            let _who = T::EnsureDEXManager::ensure_can_manage(&DEXId::Polkaswap.into(), origin, ManagementMode::Private)?;
            ReferenceAssetId::<T>::put(reference_asset_id);
            Ok(())
        }
    }
}

/// This function is used by `exchange` function to transfer calculated `input_amount` of
/// `in_asset_id` to reserves and mint `output_amount` of `out_asset_id`.
///
/// If there's enough reserves in the pool, this function will also distribute some free amount
/// to accounts specified in `DistributionAccounts` struct and buy-back and burn some amount
/// of VAL asset.
///
/// Note: all fees are going to reserves.
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
    fn deposit_input(&self) -> Result<(Balance, (Balance, Balance)), DispatchError> {
        common::with_transaction(|| {
            let main_asset_id = &self.main_asset_id;
            let collateral_asset_id = &self.collateral_asset_id;
            let (input_amount, output_amount) =
                Module::<T>::decide_buy_amounts(main_asset_id, collateral_asset_id, self.amount)?;
            Technical::<T>::transfer_in(
                collateral_asset_id,
                &self.from_account_id,
                &self.reserves_tech_account_id,
                input_amount,
            )?;
            let free_amount = input_amount * Balance(fixed!(0.2));
            Ok((free_amount, (input_amount, output_amount)))
        })
    }

    fn distribute_reserves(&self, free_amount: Balance) -> Result<(), DispatchError> {
        common::with_transaction(|| {
            if free_amount == Balance::zero() {
                return Ok(());
            }

            let reserves_tech_acc = &self.reserves_tech_account_id;
            let reserves_acc = &self.reserves_account_id;
            let in_asset = &self.collateral_asset_id;
            let out_asset = &self.main_asset_id;
            let swapped_xor_amount = T::LiquidityProxy::exchange(
                reserves_acc,
                reserves_acc,
                in_asset,
                out_asset,
                SwapAmount::with_desired_input(free_amount, Balance::zero()).into(),
                Module::<T>::self_excluding_filter(),
            )?
            .amount
            .into();
            Technical::<T>::burn(out_asset, reserves_tech_acc, swapped_xor_amount)?;
            Technical::<T>::mint(out_asset, reserves_tech_acc, swapped_xor_amount)?;

            let distribution_accounts: DistributionAccounts<
                DistributionAccountData<T::TechAccountId>,
            > = DistributionAccountsEntry::<T>::get();
            for (to_tech_account_id, coefficient) in distribution_accounts
                .xor_distribution_as_array()
                .iter()
                .map(|x| (&x.account_id, x.coefficient))
            {
                technical::Module::<T>::transfer(
                    out_asset,
                    reserves_tech_acc,
                    to_tech_account_id,
                    swapped_xor_amount * Balance(coefficient),
                )?;
            }
            let val_amount = if out_asset == &VAL.into() {
                Assets::<T>::free_balance(&VAL.into(), reserves_acc)?
            } else {
                T::LiquidityProxy::exchange(
                    reserves_acc,
                    reserves_acc,
                    out_asset,
                    &VAL.into(),
                    SwapAmount::with_desired_input(
                        swapped_xor_amount * Balance(distribution_accounts.val_holders.coefficient),
                        Balance::zero(),
                    ),
                    Module::<T>::self_excluding_filter(),
                )?
                .amount
            };
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

    fn swap(&self) -> Result<SwapOutcome<Balance>, DispatchError> {
        common::with_transaction(|| {
            let (input_amount_free, (input_amount, output_amount)) = self.deposit_input()?;
            self.distribute_reserves(input_amount_free)?;
            self.mint_output(output_amount.clone())?;
            Ok(match self.amount {
                SwapAmount::WithDesiredInput { .. } => {
                    SwapOutcome::new(output_amount, Balance::zero())
                }
                SwapAmount::WithDesiredOutput { .. } => {
                    SwapOutcome::new(input_amount, Balance::zero())
                }
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

    fn initialize_pool_unchecked(
        base_asset_id: T::AssetId,
        target_asset_id: T::AssetId,
    ) -> DispatchResult {
        common::with_transaction(|| {
            let pair = TradingPair::<T> {
                base_asset_id,
                target_asset_id,
            };
            ensure!(
                !EnabledPairs::<T>::get().contains(&pair),
                Error::<T>::PoolAlreadyInitializedForPair
            );
            T::EnsureTradingPairExists::ensure_trading_pair_exists(
                &DEXId::Polkaswap.into(),
                &base_asset_id,
                &target_asset_id,
            )?;
            EnabledPairs::<T>::mutate(|set| set.insert(pair));
            Self::deposit_event(RawEvent::PoolInitialized(DEXId::Polkaswap.into(), pair));
            Ok(())
        })
    }

    /// Calculates and returns the current buy price for one main asset.
    ///
    /// For every `PC_S` assets the price goes up by `PC_R`.
    ///
    /// `P_BM1(Q) = Q / (PC_S * PC_R) + P_I`
    ///
    /// where
    /// `P_BM1(Q)`: buy price for one asset
    /// `P_I`: initial asset price
    /// `PC_R`: price change rate
    /// `PC_S`: price change step
    /// `Q`: asset issuance (quantity)
    pub fn reference_buy_price_for_one_main_asset(
        main_asset_id: &T::AssetId,
    ) -> Result<Fixed, DispatchError> {
        let total_issuance = Assets::<T>::total_issuance(main_asset_id)?;
        let Q: FixedWrapper = total_issuance.into();
        let P_I = Self::initial_price();
        let PC_S = Self::price_change_step();
        let PC_R: FixedWrapper = Self::price_change_rate().into();
        let price = Q / (PC_S * PC_R) + P_I;
        price
            .get()
            .map_err(|_| Error::<T>::CalculatePriceFailed.into())
    }

    /// Calculates and returns the current buy price, assuming that input is the collateral asset and output is the main asset.
    ///
    /// To calculate price for a specific amount of assets (with desired main asset output),
    /// one needs to integrate the equation of buy price (`P_B(Q)`):
    ///
    /// ```nocompile
    /// P_M(Q, Q') = ∫ [P_B(x) dx, x = Q to Q']
    ///            = x² / (2 * PC_S * PC_R) + P_I * x, x = Q to Q'
    ///            = (Q' / (2 * PC_S * PC_R) + P_I) * Q' -
    ///              (Q  / (2 * PC_S * PC_R) + P_I) * Q;
    ///
    /// P_BM(Q, q) = P_M(Q, Q+q);
    /// ```
    /// Using derived formula for buy price, inverse price (with desired collateral asset input)
    /// is a solution for with respect to `q`.
    ///
    /// ```nocompile
    /// P_M(Q, Q')  = | (Q' / (2 * PC_S * PC_R) + P_I) * Q' -
    ///                 (Q  / (2 * PC_S * PC_R) + P_I) * Q |
    ///
    /// q_BM = √(Q² + 2 * Q * PC_S * PC_R * P_I + PC_S * PC_R *(PC_S * PC_R * P_I²
    ///         + 2 * P_TB(Q, Q'))) - Q - PC_S * PC_R * P_I
    ///```
    /// where
    /// `Q`: current asset issuance (quantity)
    /// `Q'`: new asset issuance (quantity)
    /// `P_I`: initial asset price
    /// `PC_R`: price change rate
    /// `PC_S`: price change step
    /// `P_Sc: sell price coefficient (%)`
    /// `P_M(Q, Q')`: helper function to calculate price for `q` assets, where `q = |Q' - Q|`
    /// `P_BM(Q, q)`: price for `q` assets to buy
    /// `q_BM`: price for `q` assets to be bought, when P_M(Q, Q') tokens are spend
    ///
    /// [buy with desired output](https://www.wolframalpha.com/input/?i=p+%3D+q+%2F+(s+*+r)+%2B+i+integrate+for+q&assumption="i"+->+"Variable")
    /// [buy with desired input](https://www.wolframalpha.com/input/?i=y+%3D+%28%28a%2Bx%29+%2F+%282+*+b+*+c%29+%2B+d%29+*+%28a%2Bx%29+-+%28+a+%2F+%282+*+b+*+c%29+%2B+d%29+*+a+solve+for+x)
    pub fn buy_price(
        main_asset_id: &T::AssetId,
        collateral_asset_id: &T::AssetId,
        quantity: QuoteAmount<Balance>,
    ) -> Result<Fixed, DispatchError> {
        // This call provides check for pool existance.
        let total_issuance = Assets::<T>::total_issuance(&main_asset_id)?;
        let Q = FixedWrapper::from(total_issuance);
        let P_I = Self::initial_price();
        let PC_S = FixedWrapper::from(Self::price_change_step());
        let PC_R = Self::price_change_rate();
        let collateral_price_per_reference_unit: FixedWrapper = T::LiquidityProxy::quote(
            collateral_asset_id,
            &ReferenceAssetId::<T>::get(),
            SwapAmount::with_desired_input(Balance::one(), Balance::zero()),
            Self::self_excluding_filter(),
        )?
        .amount
        .into();

        match quantity {
            QuoteAmount::WithDesiredInput {
                desired_amount_in: collateral_quantity,
            } => {
                // convert from collateral to reference price
                let IN = collateral_price_per_reference_unit * collateral_quantity;
                let PC_S_times_PC_R_times_P_I = PC_S.clone() * PC_R.clone() * P_I;
                let Q_squared = Q.clone() * Q.clone();
                let inner_term_a = 2 * Q.clone() * PC_S_times_PC_R_times_P_I.clone();
                let inner_term_b =
                    PC_S.clone() * PC_R * (PC_S_times_PC_R_times_P_I.clone() * P_I + 2 * IN);
                let under_sqrt = Q_squared + inner_term_a + inner_term_b;
                let output_main = under_sqrt.sqrt_accurate() - Q - PC_S_times_PC_R_times_P_I;
                Ok(output_main
                    .get()
                    .map_err(|_| Error::<T>::CalculatePriceFailed)?
                    .max(Fixed::ZERO)) // Limiting bound to zero because sqrt error subtraction can be negative.
            }
            QuoteAmount::WithDesiredOutput {
                desired_amount_out: main_quantity,
            } => {
                let Q_prime = Q.clone() + main_quantity;
                let two_times_PC_S_times_PC_R = 2 * PC_S * PC_R;
                let to = (Q_prime.clone() / two_times_PC_S_times_PC_R.clone() + P_I) * Q_prime;
                let from = (Q.clone() / two_times_PC_S_times_PC_R + P_I) * Q;
                let mut output_collateral = to - from;
                // convert from reference to collateral price
                output_collateral = output_collateral / collateral_price_per_reference_unit;
                Ok(output_collateral
                    .get()
                    .map_err(|_| Error::<T>::CalculatePriceFailed)?
                    .max(Fixed::ZERO)) // Limiting bound to zero because substracting value with error can result in negative value.
            }
        }
    }

    /// Calculates and returns the current buy price meaning
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
        // 20% sell price margin is already applied in sell function.
        let main_price_per_reference_unit: FixedWrapper =
            Self::reference_sell_price_for_one_main_asset(main_asset_id)?.into();
        let collateral_price_per_reference_unit: FixedWrapper = T::LiquidityProxy::quote(
            collateral_asset_id,
            &ReferenceAssetId::<T>::get(),
            SwapAmount::with_desired_input(Balance::one(), Balance::zero()),
            Self::self_excluding_filter(),
        )?
        .amount
        .into();
        // Assume main token reserve is equal by reference value to collateral token reserve.
        let main_supply = collateral_supply.clone() * collateral_price_per_reference_unit
            / main_price_per_reference_unit;
        let collateral_supply_unwrapped = collateral_supply
            .clone()
            .get()
            .map_err(|_| Error::<T>::CalculatePriceFailed)?;

        match quantity {
            QuoteAmount::WithDesiredInput {
                desired_amount_in: quantity_main,
            } => {
                let output_collateral =
                    (quantity_main * collateral_supply) / (main_supply + quantity_main);
                let output_collateral_unwrapped = output_collateral
                    .get()
                    .map_err(|_| Error::<T>::CalculatePriceFailed)?;
                ensure!(
                    output_collateral_unwrapped < collateral_supply_unwrapped.into(),
                    Error::<T>::NotEnoughReserves
                );
                Ok(output_collateral_unwrapped)
            }
            QuoteAmount::WithDesiredOutput {
                desired_amount_out: quantity_collateral,
            } => {
                ensure!(
                    quantity_collateral < collateral_supply_unwrapped.into(),
                    Error::<T>::NotEnoughReserves
                );
                let output_main =
                    (main_supply * quantity_collateral) / (collateral_supply - quantity_collateral);
                output_main
                    .get()
                    .map_err(|_| Error::<T>::CalculatePriceFailed.into())
            }
        }
    }

    /// Calculates and returns the current sell price for one main asset.
    /// Sell price is `P_Sc`% of buy price (see `buy_price_for_one_main_asset`).
    ///
    /// `P_S = P_Sc * P_B`
    /// where
    /// `P_Sc: sell price coefficient (%)`
    pub fn reference_sell_price_for_one_main_asset(
        main_asset_id: &T::AssetId,
    ) -> Result<Fixed, DispatchError> {
        let P_B = Self::reference_buy_price_for_one_main_asset(main_asset_id)?;
        let P_Sc = FixedWrapper::from(Self::sell_price_coefficient());
        let price = P_Sc * P_B;
        price
            .get()
            .map_err(|_| Error::<T>::CalculatePriceFailed.into())
    }

    /// Decompose SwapAmount into particular buy quotation query.
    ///
    /// Returns ordered pair: (input_amount, output_amount).
    fn decide_buy_amounts(
        main_asset_id: &T::AssetId,
        collateral_asset_id: &T::AssetId,
        amount: SwapAmount<Balance>,
    ) -> Result<(Balance, Balance), DispatchError> {
        Ok(match amount {
            SwapAmount::WithDesiredInput {
                desired_amount_in,
                min_amount_out,
            } => {
                let output_amount = Self::buy_price(
                    main_asset_id,
                    collateral_asset_id,
                    QuoteAmount::with_desired_input(desired_amount_in),
                )?
                .into();
                ensure!(output_amount >= min_amount_out, Error::<T>::SlippageFailed);
                (desired_amount_in, output_amount)
            }
            SwapAmount::WithDesiredOutput {
                desired_amount_out,
                max_amount_in,
            } => {
                let input_amount = Self::buy_price(
                    main_asset_id,
                    collateral_asset_id,
                    QuoteAmount::with_desired_output(desired_amount_out),
                )?
                .into();
                ensure!(input_amount <= max_amount_in, Error::<T>::SlippageFailed);
                (input_amount, desired_amount_out)
            }
        })
    }

    /// Decompose SwapAmount into particular sell quotation query.
    ///
    /// Returns ordered pair: (input_amount, output_amount).
    fn decide_sell_amounts(
        main_asset_id: &T::AssetId,
        collateral_asset_id: &T::AssetId,
        amount: SwapAmount<Balance>,
    ) -> Result<(Balance, Balance), DispatchError> {
        Ok(match amount {
            SwapAmount::WithDesiredInput {
                desired_amount_in,
                min_amount_out,
            } => {
                let output_amount = Self::sell_price(
                    main_asset_id,
                    collateral_asset_id,
                    QuoteAmount::with_desired_input(desired_amount_in),
                )?
                .into();
                ensure!(output_amount >= min_amount_out, Error::<T>::SlippageFailed);
                (desired_amount_in, output_amount)
            }
            SwapAmount::WithDesiredOutput {
                desired_amount_out,
                max_amount_in,
            } => {
                let input_amount = Self::sell_price(
                    main_asset_id,
                    collateral_asset_id,
                    QuoteAmount::with_desired_output(desired_amount_out),
                )?
                .into();
                ensure!(input_amount <= max_amount_in, Error::<T>::SlippageFailed);
                (input_amount, desired_amount_out)
            }
        })
    }

    /// This function is used by `exchange` function to burn `input_amount` derived from `amount` of `main_asset_id`
    /// and transfer calculated amount of `collateral_asset_id` to the receiver from reserves.
    ///
    /// If there's not enough reserves in the pool, `NotEnoughReserves` error will be returned.
    ///
    /// Note: all fees will are burned in the current version.
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
            let (input_amount, output_amount) =
                Self::decide_sell_amounts(main_asset_id, collateral_asset_id, amount)?;
            let transfer_amount = output_amount;
            let reserves_amount =
                Assets::<T>::total_balance(collateral_asset_id, &reserves_account_id)?;
            ensure!(
                reserves_amount >= transfer_amount,
                Error::<T>::NotEnoughReserves
            );
            technical::Module::<T>::transfer_out(
                collateral_asset_id,
                &reserves_tech_account_id,
                &to_account_id,
                transfer_amount,
            )?;
            Assets::<T>::burn_from(
                main_asset_id,
                &reserves_account_id,
                from_account_id,
                input_amount,
            )?;
            Ok(SwapOutcome::new(transfer_amount, Balance::zero()))
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
        let base_asset_id = &T::GetBaseAssetId::get();
        if input_asset_id == base_asset_id {
            let pair = TradingPair::<T> {
                base_asset_id: *input_asset_id,
                target_asset_id: *output_asset_id,
            };
            EnabledPairs::<T>::get().contains(&pair)
        } else {
            let pair = TradingPair::<T> {
                base_asset_id: *output_asset_id,
                target_asset_id: *input_asset_id,
            };
            EnabledPairs::<T>::get().contains(&pair)
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
        if input_asset_id == base_asset_id {
            match swap_amount {
                SwapAmount::WithDesiredInput {
                    desired_amount_in: base_amount_in,
                    ..
                } => {
                    let amount = Self::sell_price(
                        input_asset_id,
                        output_asset_id,
                        QuoteAmount::with_desired_input(base_amount_in),
                    )?
                    .into();
                    Ok(SwapOutcome::new(amount, Balance::zero()))
                }
                SwapAmount::WithDesiredOutput {
                    desired_amount_out: target_amount_out,
                    ..
                } => {
                    let amount = Self::sell_price(
                        input_asset_id,
                        output_asset_id,
                        QuoteAmount::with_desired_output(target_amount_out),
                    )?
                    .into();
                    Ok(SwapOutcome::new(amount, Balance::zero()))
                }
            }
        } else {
            match swap_amount {
                SwapAmount::WithDesiredInput {
                    desired_amount_in: target_amount_in,
                    ..
                } => {
                    let amount = Self::buy_price(
                        output_asset_id,
                        input_asset_id,
                        QuoteAmount::with_desired_input(target_amount_in),
                    )?
                    .into();
                    Ok(SwapOutcome::new(amount, Balance::zero()))
                }
                SwapAmount::WithDesiredOutput {
                    desired_amount_out: base_amount_out,
                    ..
                } => {
                    let amount = Self::buy_price(
                        output_asset_id,
                        input_asset_id,
                        QuoteAmount::with_desired_output(base_amount_out),
                    )?
                    .into();
                    Ok(SwapOutcome::new(amount, Balance::zero()))
                }
            }
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
            fail!(Error::<T>::CantExchangeOnItself);
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
