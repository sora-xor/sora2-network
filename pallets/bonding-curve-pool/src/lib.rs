#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

use codec::{Decode, Encode};
use common::{
    fixed,
    prelude::{Balance, Error as CommonError, Fixed, FixedWrapper, SwapAmount, SwapOutcome},
    DEXId, LiquiditySource, USD, VAL,
};
use frame_support::traits::Get;
use frame_support::{decl_error, decl_module, decl_storage, ensure, fail};
use permissions::{Scope, BURN, MINT, SLASH, TRANSFER};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_arithmetic::traits::Zero;
use sp_runtime::DispatchError;
pub trait Trait: common::Trait + assets::Trait + technical::Trait {
    type DEXApi: LiquiditySource<
        Self::DEXId,
        Self::AccountId,
        Self::AssetId,
        Balance,
        DispatchError,
    >;
}

type Assets<T> = assets::Module<T>;
type Technical<T> = technical::Module<T>;

pub const TECH_ACCOUNT_PREFIX: &[u8] = b"bonding-curve-pool";
pub const TECH_ACCOUNT_RESERVES: &[u8] = b"reserves";

#[derive(Debug, Encode, Decode, Clone)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct DistributionAccountData<TechAccountId> {
    pub account_id: TechAccountId,
    coefficient: Fixed,
}

impl<TechAccountId: Default> Default for DistributionAccountData<TechAccountId> {
    fn default() -> Self {
        Self {
            account_id: Default::default(),
            coefficient: Default::default(),
        }
    }
}

impl<TechAccountId> DistributionAccountData<TechAccountId> {
    pub fn new(account_id: TechAccountId, coefficient: Fixed) -> Self {
        DistributionAccountData {
            account_id,
            coefficient,
        }
    }
}

#[derive(Debug, Encode, Decode, Clone)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct DistributionAccounts<DistributionAccountData> {
    pub xor_allocation: DistributionAccountData,
    pub sora_citizens: DistributionAccountData,
    pub stores_and_shops: DistributionAccountData,
    pub parliament_and_development: DistributionAccountData,
    pub projects: DistributionAccountData,
    pub val_holders: DistributionAccountData,
}

impl<TechAccountId> DistributionAccounts<DistributionAccountData<TechAccountId>> {
    pub fn xor_distribution_as_array(&self) -> [&DistributionAccountData<TechAccountId>; 5] {
        [
            &self.xor_allocation,
            &self.sora_citizens,
            &self.stores_and_shops,
            &self.parliament_and_development,
            &self.projects,
        ]
    }

    pub fn xor_distribution_accounts_as_array(&self) -> [&TechAccountId; 5] {
        [
            &self.xor_allocation.account_id,
            &self.sora_citizens.account_id,
            &self.stores_and_shops.account_id,
            &self.parliament_and_development.account_id,
            &self.projects.account_id,
        ]
    }
}

impl<DistributionAccountData: Default> Default for DistributionAccounts<DistributionAccountData> {
    fn default() -> Self {
        Self {
            xor_allocation: Default::default(),
            sora_citizens: Default::default(),
            stores_and_shops: Default::default(),
            parliament_and_development: Default::default(),
            projects: Default::default(),
            val_holders: Default::default(),
        }
    }
}

decl_storage! {
    trait Store for Module<T: Trait> as BondingCurve {
        ReservesAcc get(fn reserves_account_id) config(): T::TechAccountId;
        Fee get(fn fee): Fixed = fixed!(0,1%);
        InitialPrice get(fn initial_price): Fixed = fixed!(99,3);
        PriceChangeStep get(fn price_change_step): Fixed = 5000.into();
        PriceChangeRate get(fn price_change_rate): Fixed = 100.into();
        SellPriceCoefficient get(fn sell_price_coefficient): Fixed = fixed!(80%);
        DistributionAccountsEntry get(fn distribution_accounts) config(): DistributionAccounts<DistributionAccountData<T::TechAccountId>>;
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
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        type Error = Error<T>;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwapKind {
    Buy,
    Sell,
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
    in_asset_id: T::AssetId,
    out_asset_id: T::AssetId,
    output_amount: Balance,
    from_account_id: T::AccountId,
    to_account_id: T::AccountId,
    reserves_tech_account_id: T::TechAccountId,
    reserves_account_id: T::AccountId,
}

impl<T: Trait> BuyMainAsset<T> {
    pub fn new(
        in_asset_id: T::AssetId,
        out_asset_id: T::AssetId,
        output_amount: Balance,
        from_account_id: T::AccountId,
        to_account_id: T::AccountId,
    ) -> Result<Self, DispatchError> {
        let reserves_tech_account_id = ReservesAcc::<T>::get();
        let reserves_account_id =
            Technical::<T>::tech_account_id_to_account_id(&reserves_tech_account_id)?;
        Ok(BuyMainAsset {
            in_asset_id,
            out_asset_id,
            output_amount,
            from_account_id,
            to_account_id,
            reserves_tech_account_id,
            reserves_account_id,
        })
    }

    /// Assets deposition algorithm:
    ///
    /// ```nocompile
    /// R_e := P_SM('all XOR')
    /// R := R + A_I
    /// R_f := max((R - R_e) * c, 0)
    /// ```
    ///
    /// where:
    /// `R` - current reserves
    /// `R_e` - expected reserves (sell price of all XOR in the reserves)
    /// `R_f` - free reserves, that can be distributed
    /// `c` - free amount coefficient of extra reserves
    /// `A_I` - amount of the input asset
    /// `P_SM` - sell price for main asset
    fn deposit_input(&self) -> Result<Balance, DispatchError> {
        common::with_transaction(|| {
            let out_asset = &self.out_asset_id;
            let in_asset = &self.in_asset_id;
            let input_amount = Balance(Module::<T>::price_for_main_asset(
                out_asset,
                self.output_amount,
                SwapKind::Buy,
            )?);
            let total_issuance = Assets::<T>::total_issuance(out_asset)?;
            let reserves_expected = Balance(Module::<T>::price_for_main_asset(
                out_asset,
                total_issuance,
                SwapKind::Sell,
            )?);
            Technical::<T>::transfer_in(
                in_asset,
                &self.from_account_id,
                &self.reserves_tech_account_id,
                input_amount,
            )?;
            let reserves = Assets::<T>::total_balance(in_asset, &self.reserves_account_id)?;
            let free_amount = if reserves > reserves_expected {
                let amount_free_coefficient: Balance = fixed!(20%).into();
                (reserves - reserves_expected) * amount_free_coefficient
            } else {
                Balance::zero()
            };
            Ok(free_amount)
        })
    }

    fn distribute_reserves(&self, free_amount: Balance) -> Result<(), DispatchError> {
        common::with_transaction(|| {
            if free_amount == Balance::zero() {
                return Ok(());
            }

            let reserves_tech_acc = &self.reserves_tech_account_id;
            let reserves_acc = &self.reserves_account_id;
            let in_asset = &self.in_asset_id;
            let out_asset = &self.out_asset_id;
            let swapped_xor_amount = T::DEXApi::exchange(
                reserves_acc,
                reserves_acc,
                &DEXId::Polkaswap.into(),
                in_asset,
                out_asset,
                SwapAmount::with_desired_input(free_amount, Balance::zero()), // TODO: do we need to set `min_amount_out`?
            )?
            .amount;
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

            let val_amount = T::DEXApi::exchange(
                reserves_acc,
                reserves_acc,
                &DEXId::Polkaswap.into(),
                out_asset,
                &VAL.into(),
                SwapAmount::with_desired_input(
                    swapped_xor_amount * Balance(distribution_accounts.val_holders.coefficient),
                    Balance::zero(),
                ),
            )?
            .amount;
            Technical::<T>::burn(&VAL.into(), reserves_tech_acc, val_amount)?;
            Ok(())
        })
    }

    fn mint_output(&self) -> Result<SwapOutcome<Balance>, DispatchError> {
        // TODO: deal with fee.
        let fee_amount = Balance(Fee::get()) * self.output_amount;
        let transfer_amount = self.output_amount - fee_amount;
        Assets::<T>::mint_to(
            &self.out_asset_id,
            &self.reserves_account_id,
            &self.to_account_id,
            transfer_amount,
        )?;
        Ok(SwapOutcome::new(transfer_amount, fee_amount))
    }

    fn swap(&self) -> Result<SwapOutcome<Balance>, DispatchError> {
        common::with_transaction(|| {
            let input_amount_free = self.deposit_input()?;
            self.distribute_reserves(input_amount_free)?;
            self.mint_output()
        })
    }
}

#[allow(non_snake_case)]
impl<T: Trait> Module<T> {
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
    pub fn buy_price_for_one_main_asset(out_asset_id: &T::AssetId) -> Result<Fixed, DispatchError> {
        let total_issuance = Assets::<T>::total_issuance(out_asset_id)?;
        let Q: FixedWrapper = total_issuance.into();
        let P_I = Self::initial_price();
        let PC_S = Self::price_change_step();
        let PC_R = Self::price_change_rate();
        let price = Q / (PC_S * PC_R) + P_I;
        price.get().ok_or(Error::<T>::CalculatePriceFailed.into())
    }

    /// Calculates and returns the current buy/sell price for main asset.
    ///
    /// To calculate price for a specific amount of assets,
    /// one needs to integrate the equation of buy price (`P_B(Q)`):
    ///
    /// ```nocompile
    /// P_M(Q, Q') = ∫ [P_B(x) dx, x = Q to Q']
    ///            = x² / (2 * PC_S * PC_R) + P_I * x, x = Q to Q'
    ///            = (Q' / (2 * PC_S * PC_R) + P_I) * Q' -
    ///              (Q  / (2 * PC_S * PC_R) + P_I) * Q;
    ///
    /// P_BM(Q, q) = P_M(Q, Q+q);
    /// P_SM(Q, q) = P_M(Q-q, Q) * P_Sc
    /// ```
    /// where
    /// `Q`: current asset issuance (quantity)
    /// `Q'`: new asset issuance (quantity)
    /// `P_I`: initial asset price
    /// `PC_R`: price change rate
    /// `PC_S`: price change step
    /// `P_Sc: sell price coefficient (%)`
    /// `P_M(Q, Q')`: helper function to calculate price for `q` assets, where `q = |Q' - Q|`
    /// `P_BM(Q, q)`: price for `q` assets to buy
    /// `P_SM(Q, q)`: price for `q` assets to sell
    ///
    /// [Formula calculation](https://www.wolframalpha.com/input/?i=p+%3D+q+%2F+(s+*+r)+%2B+i+integrate+for+q&assumption="i"+->+"Variable")
    #[rustfmt::skip]
    pub fn price_for_main_asset(main_asset_id: &T::AssetId, quantity: Balance, kind: SwapKind) -> Result<Fixed, DispatchError> {
        let total_issuance = Assets::<T>::total_issuance(&main_asset_id)?;
        let Q = FixedWrapper::from(total_issuance);
        let P_I = Self::initial_price();
        let PC_S = FixedWrapper::from(Self::price_change_step());
        let PC_R = Self::price_change_rate();

        let Q_prime = if kind == SwapKind::Buy { Q + quantity } else { Q - quantity };
        let two_times_PC_S_times_PC_R = 2 * PC_S * PC_R;
        let to = (Q_prime / two_times_PC_S_times_PC_R + P_I) * Q_prime;
        let from = (Q / two_times_PC_S_times_PC_R + P_I) * Q;
        let price: FixedWrapper = if kind == SwapKind::Buy {
            to - from
        } else {
            let P_Sc = FixedWrapper::from(Self::sell_price_coefficient());
            P_Sc * (from - to)
        };
        price.get().ok_or(Error::<T>::CalculatePriceFailed.into())
    }

    /// Calculates and returns the current sell price for one main asset.
    /// Sell price is `P_Sc`% of buy price (see `buy_price_for_one_main_asset`).
    ///
    /// `P_S = P_Sc * P_B`
    /// where
    /// `P_Sc: sell price coefficient (%)`
    pub fn sell_price_for_one_main_asset(in_asset_id: &T::AssetId) -> Result<Fixed, DispatchError> {
        let P_B = Self::buy_price_for_one_main_asset(in_asset_id)?;
        let P_Sc = FixedWrapper::from(Self::sell_price_coefficient());
        let price = P_Sc * P_B;
        price.get().ok_or(Error::<T>::CalculatePriceFailed.into())
    }

    /// This function is used by `exchange` function to burn `input_amount` of `in_asset_id`
    /// and transfer calculated amount of `out_asset_id` to the receiver from reserves.
    ///
    /// If there's not enough reserves in the pool, `NotEnoughReserves` error will be returned.
    ///
    /// Note: all fees will are burned in the current version.
    fn sell_main_asset(
        _dex_id: &T::DEXId,
        in_asset_id: &T::AssetId,
        out_asset_id: &T::AssetId,
        input_amount: Balance,
        from_account_id: &T::AccountId,
        to_account_id: &T::AccountId,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        common::with_transaction(|| {
            let reserves_tech_account_id = Self::reserves_account_id();
            let reserves_account_id =
                Technical::<T>::tech_account_id_to_account_id(&reserves_tech_account_id)?;
            let output_amount = Balance(Self::price_for_main_asset(
                in_asset_id,
                input_amount,
                SwapKind::Sell,
            )?);
            // TODO: deal with fee.
            let fee_amount = Balance(Self::fee()) * output_amount;
            let transfer_amount = output_amount - fee_amount;
            let reserves_amount = Assets::<T>::total_balance(out_asset_id, &reserves_account_id)?;
            ensure!(
                reserves_amount >= transfer_amount,
                Error::<T>::NotEnoughReserves
            );
            technical::Module::<T>::transfer_out(
                out_asset_id,
                &reserves_tech_account_id,
                &to_account_id,
                transfer_amount,
            )?;
            Assets::<T>::burn_from(
                in_asset_id,
                &reserves_account_id,
                from_account_id,
                input_amount,
            )?;
            Ok(SwapOutcome::new(transfer_amount, fee_amount))
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
        let base_asset_id = &T::GetBaseAssetId::get();
        // Can trade only with XOR (base asset) and USD on Polkaswap.
        *dex_id == DEXId::Polkaswap.into()
            && ((input_asset_id == &USD.into() && output_asset_id == base_asset_id)
                || (output_asset_id == &USD.into() && input_asset_id == base_asset_id))
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
                    let amount = Self::price_for_main_asset(
                        input_asset_id,
                        base_amount_in.into(),
                        SwapKind::Sell,
                    )?
                    .into();
                    Ok(SwapOutcome::new(amount, amount * Balance(Self::fee())))
                }
                SwapAmount::WithDesiredOutput {
                    desired_amount_out: _target_amount_out,
                    ..
                } => {
                    fail!(CommonError::<T>::UnsupportedSwapMethod);
                }
            }
        } else {
            match swap_amount {
                SwapAmount::WithDesiredInput {
                    desired_amount_in: _target_amount_in,
                    ..
                } => {
                    fail!(CommonError::<T>::UnsupportedSwapMethod);
                }
                SwapAmount::WithDesiredOutput {
                    desired_amount_out: base_amount_out,
                    ..
                } => {
                    let amount = Self::price_for_main_asset(
                        output_asset_id,
                        base_amount_out.into(),
                        SwapKind::Buy,
                    )?
                    .into();
                    Ok(SwapOutcome::new(amount, amount * Balance(Self::fee())))
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
        let reserves_account_id =
            &Technical::<T>::tech_account_id_to_account_id(&Self::reserves_account_id())?;
        // This is needed to prevent recursion calls.
        if sender == reserves_account_id && receiver == reserves_account_id {
            fail!(Error::<T>::CantExchangeOnItself);
        }
        if !Self::can_exchange(dex_id, input_asset_id, output_asset_id) {
            fail!(CommonError::<T>::CantExchange);
        }
        let base_asset_id = &T::GetBaseAssetId::get();
        if input_asset_id == base_asset_id {
            match desired_amount {
                SwapAmount::WithDesiredInput {
                    desired_amount_in: base_amount_in,
                    ..
                } => Self::sell_main_asset(
                    dex_id,
                    input_asset_id,
                    output_asset_id,
                    base_amount_in,
                    sender,
                    receiver,
                ),
                SwapAmount::WithDesiredOutput {
                    desired_amount_out: _target_amount_out,
                    ..
                } => {
                    fail!(CommonError::<T>::UnsupportedSwapMethod);
                }
            }
        } else {
            match desired_amount {
                SwapAmount::WithDesiredInput {
                    desired_amount_in: _target_amount_in,
                    ..
                } => {
                    fail!(CommonError::<T>::UnsupportedSwapMethod);
                }
                SwapAmount::WithDesiredOutput {
                    desired_amount_out: base_amount_out,
                    ..
                } => BuyMainAsset::<T>::new(
                    *input_asset_id,
                    *output_asset_id,
                    base_amount_out,
                    sender.clone(),
                    receiver.clone(),
                )?
                .swap(),
            }
        }
    }
}
