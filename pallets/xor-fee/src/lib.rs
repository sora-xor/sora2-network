// This file is part of the SORA network and Polkaswap app.

// Copyright (c) 2020, 2021, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

// Redistribution and use in source and binary forms, with or without modification,
// are permitted provided that the following conditions are met:

// Redistributions of source code must retain the above copyright notice, this list
// of conditions and the following disclaimer.
// Redistributions in binary form must reproduce the above copyright notice, this
// list of conditions and the following disclaimer in the documentation and/or other
// materials provided with the distribution.
//
// All advertising materials mentioning features or use of this software must display
// the following acknowledgement: This product includes software developed by Polka Biome
// Ltd., SORA, and Polkaswap.
//
// Neither the name of the Polka Biome Ltd. nor the names of its contributors may be used
// to endorse or promote products derived from this software without specific prior written permission.

// THIS SOFTWARE IS PROVIDED BY Polka Biome Ltd. AS IS AND ANY EXPRESS OR IMPLIED WARRANTIES,
// INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL Polka Biome Ltd. BE LIABLE FOR ANY
// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
// BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS;
// OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
// USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
#[cfg(feature = "wip")] // Xorless fee
use common::prelude::FixedWrapper;
use common::prelude::SwapAmount;
use common::{
    AssetIdOf, AssetManager, Balance, BuyBackHandler, LiquidityProxyTrait, LiquiditySource,
    LiquiditySourceFilter, LiquiditySourceType, OnValBurned, ReferrerAccountProvider,
};
#[cfg(feature = "wip")] // Xorless fee
use common::{PriceToolsProvider, PriceVariant};
#[cfg(feature = "wip")] // Xorless fee
use frame_support::dispatch::extract_actual_weight;
use frame_support::dispatch::{DispatchInfo, GetDispatchInfo, Pays, PostDispatchInfo};
use frame_support::log::{error, warn};
use frame_support::pallet_prelude::{DispatchResultWithPostInfo, InvalidTransaction};
use frame_support::traits::Randomness;
use frame_support::traits::{Currency, ExistenceRequirement, Get, Imbalance, WithdrawReasons};
use frame_support::unsigned::TransactionValidityError;
use frame_support::weights::Weight;
use frame_support::weights::{
    WeightToFeeCoefficient, WeightToFeeCoefficients, WeightToFeePolynomial,
};
use pallet_transaction_payment as ptp;
use pallet_transaction_payment::{
    FeeDetails, InclusionFee, OnChargeTransaction, RuntimeDispatchInfo,
};
use smallvec::smallvec;
use sp_arithmetic::FixedPointOperand;
use sp_runtime::traits::{
    DispatchInfoOf, Dispatchable, Extrinsic as ExtrinsicT, PostDispatchInfoOf, SaturatedConversion,
    Saturating, UniqueSaturatedInto, Zero,
};
use sp_runtime::{DispatchError, DispatchResult, FixedPointNumber, FixedU128, Perbill, Percent};
use sp_std::boxed::Box;
#[cfg(feature = "wip")] // Xorless fee
use traits::MultiCurrency;

pub mod extension;

mod benchmarking;
pub mod weights;

#[cfg(test)]
pub mod mock;

pub mod migrations;
#[cfg(test)]
mod tests;

pub const TECH_ACCOUNT_PREFIX: &[u8] = b"xor-fee";
pub const TECH_ACCOUNT_MAIN: &[u8] = b"main";

type NegativeImbalanceOf<T> = <<T as Config>::XorCurrency as Currency<
    <T as frame_system::Config>::AccountId,
>>::NegativeImbalance;

type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

type BalanceOf<T> =
    <<T as Config>::XorCurrency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

type CallOf<T> = <T as frame_system::Config>::RuntimeCall;

type PaidFeeOf<T> = (
    AccountIdOf<T>,
    Option<NegativeImbalanceOf<T>>,
    Option<AssetIdOf<T>>,
);

// #[cfg_attr(test, derive(PartialEq))]
pub enum LiquidityInfo<T: Config> {
    /// Fees operate as normal
    Paid(
        AccountIdOf<T>,
        Option<NegativeImbalanceOf<T>>,
        Option<AssetIdOf<T>>,
    ),
    /// The fee payment has been postponed to after the transaction
    Postponed(AccountIdOf<T>),
    /// The fee should not be paid
    NotPaid,
}

/// Store fee paid in white listed asset
#[derive(
    Encode, Decode, Eq, PartialEq, Clone, PartialOrd, Ord, Debug, Default, scale_info::TypeInfo,
)]
pub struct AssetFee {
    /// fee paid with referral
    pub fee: Balance,
    /// fee paid without referral
    pub fee_without_referral: Balance,
}

impl<T: Config> sp_std::fmt::Debug for LiquidityInfo<T> {
    fn fmt(&self, f: &mut sp_std::fmt::Formatter<'_>) -> sp_std::fmt::Result {
        match self {
            LiquidityInfo::Paid(a, b, c) => {
                write!(
                    f,
                    "Paid({:?}, {:?}, {:?})",
                    a,
                    b.as_ref().map(|b| b.peek()),
                    c
                )
            }
            LiquidityInfo::Postponed(account_id) => {
                write!(f, "Postponed({:?})", account_id)
            }
            LiquidityInfo::NotPaid => {
                write!(f, "NotPaid")
            }
        }
    }
}

impl<T: Config> PartialEq for LiquidityInfo<T> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (LiquidityInfo::Paid(a1, b1, c1), LiquidityInfo::Paid(a2, b2, c2)) => {
                (a1 == a2)
                    && b1.as_ref().map(|b| b.peek()) == b2.as_ref().map(|b| b.peek())
                    && c1.eq(c2)
            }
            (LiquidityInfo::Postponed(a1), LiquidityInfo::Postponed(a2)) => a1 == a2,
            _ => false,
        }
    }
}

#[allow(clippy::derivable_impls)] // To use Default derive impl AccountId needs to implement Default trait
impl<T: Config> Default for LiquidityInfo<T> {
    fn default() -> Self {
        LiquidityInfo::NotPaid
    }
}

impl<T: Config>
    From<(
        AccountIdOf<T>,
        Option<NegativeImbalanceOf<T>>,
        Option<AssetIdOf<T>>,
    )> for LiquidityInfo<T>
{
    fn from(
        (account_id, paid, asset_id): (
            AccountIdOf<T>,
            Option<NegativeImbalanceOf<T>>,
            Option<AssetIdOf<T>>,
        ),
    ) -> Self {
        LiquidityInfo::Paid(account_id, paid, asset_id)
    }
}

impl<T: Config> OnChargeTransaction<T> for Pallet<T>
where
    BalanceOf<T>: Into<u128>,
    DispatchInfoOf<CallOf<T>>: Into<DispatchInfo> + Clone,
{
    type Balance = BalanceOf<T>;
    type LiquidityInfo = LiquidityInfo<T>;

    fn withdraw_fee(
        who: &T::AccountId,
        call: &CallOf<T>,
        _dispatch_info: &DispatchInfoOf<CallOf<T>>,
        fee: BalanceOf<T>,
        _tip: BalanceOf<T>,
    ) -> Result<Self::LiquidityInfo, TransactionValidityError> {
        // Not pay fee at all. It's not possible to withdraw fee if it's disabled here.
        if fee.is_zero() || !T::CustomFees::should_be_paid(who, call) {
            return Ok((who.clone(), None, None).into());
        }

        // Use custom fee source for transaction
        let fee_source = T::CustomFees::get_fee_source(who, call, fee.into());

        // Postpone fee payment to post dispatch phase if we can't or don't want to pay it now
        if T::CustomFees::should_be_postponed(who, &fee_source, call, fee.into()) {
            return Ok(LiquidityInfo::Postponed(fee_source));
        }

        // Withdraw fee
        match T::WithdrawFee::withdraw_fee(who, &fee_source, call, fee.into()) {
            Ok(result) => return Ok(result.into()),
            Err(err) if err == Error::<T>::AssetNotFound.into() => {
                return Err(InvalidTransaction::Custom(2u8).into()); // Error index in xor fee pallet
            }
            Err(err) if err == Error::<T>::FeeCalculationFailed.into() => {
                return Err(TransactionValidityError::Invalid(
                    InvalidTransaction::Payment,
                ));
            }
            _ => {}
        }

        Err(InvalidTransaction::Payment.into())
    }

    fn correct_and_deposit_fee(
        who: &T::AccountId,
        _dispatch_info: &DispatchInfoOf<CallOf<T>>,
        _post_info: &PostDispatchInfoOf<CallOf<T>>,
        corrected_fee: BalanceOf<T>,
        tip: BalanceOf<T>,
        already_withdrawn: Self::LiquidityInfo,
    ) -> Result<(), TransactionValidityError> {
        let (fee_source, withdrawn, asset_id) = match already_withdrawn {
            LiquidityInfo::Paid(a, b, c) => (a, b, c),
            LiquidityInfo::Postponed(fee_source) => {
                let withdraw_reason = if tip.is_zero() {
                    WithdrawReasons::TRANSACTION_PAYMENT
                } else {
                    WithdrawReasons::TRANSACTION_PAYMENT | WithdrawReasons::TIP
                };
                let result = T::XorCurrency::withdraw(
                    &fee_source,
                    corrected_fee,
                    withdraw_reason,
                    ExistenceRequirement::KeepAlive,
                )
                .map_err(|_| InvalidTransaction::Payment)?;
                (fee_source, Some(result), None)
            }
            LiquidityInfo::NotPaid => (who.clone(), None, None),
        };

        if let Some(paid) = withdrawn {
            #[allow(unused_variables)]
            if let Some(asset_id) = asset_id {
                #[cfg(feature = "wip")] // Xorless fee
                {
                    let corrected_fee_as_asset = (corrected_fee.into()
                        * FixedWrapper::from(
                            T::PriceTools::get_average_price(
                                &T::XorId::get(),
                                &asset_id,
                                PriceVariant::Sell,
                            )
                            .map_err(|_| {
                                TransactionValidityError::Invalid(InvalidTransaction::Payment)
                            })?,
                        ))
                    .into_balance();
                    // Calculate the amount to refund to the caller
                    // Refund behavior is fully defined by CustomFee type or
                    // by default transaction payment pallet implementation if
                    // call is not subject for custom fee
                    let refund_amount = paid.peek().into().saturating_sub(corrected_fee_as_asset);
                    if paid.peek().into() < refund_amount {
                        return Err(TransactionValidityError::Invalid(
                            InvalidTransaction::Payment,
                        ));
                    }
                    let _ = T::MultiCurrency::deposit(asset_id, &fee_source, refund_amount).is_ok();

                    Self::deposit_event(Event::FeeWithdrawn(
                        fee_source,
                        asset_id,
                        corrected_fee_as_asset,
                    ));

                    if let Some(referrer) = T::ReferrerAccountProvider::get_referrer_account(who) {
                        let referrer_amount = Self::calculate_portion_fee_from_weight(
                            T::FeeReferrerWeight::get(),
                            corrected_fee_as_asset,
                        );

                        if T::MultiCurrency::deposit(asset_id, &referrer, referrer_amount).is_ok() {
                            BurntForFee::<T>::mutate(asset_id, |balance| {
                                balance.fee = balance.fee.saturating_add(corrected_fee_as_asset)
                            });

                            Self::deposit_event(Event::ReferrerRewarded(
                                who.clone(),
                                referrer,
                                asset_id,
                                referrer_amount,
                            ));
                            return Ok(());
                        }
                    }
                    BurntForFee::<T>::mutate(asset_id, |balance| {
                        balance.fee_without_referral = balance
                            .fee_without_referral
                            .saturating_add(corrected_fee_as_asset)
                    });
                }
                #[cfg(not(feature = "wip"))] // Xorless fee
                {
                    // Calculate the amount to refund to the caller
                    // Refund behavior is fully defined by CustomFee type or
                    // by default transaction payment pallet implementation if
                    // call is not subject for custom fee
                    let refund_amount = paid.peek().saturating_sub(corrected_fee);

                    // Refund to the the account that paid the fees. If this fails, the
                    // account might have dropped below the existential balance. In
                    // that case we don't refund anything.
                    let refund_imbalance =
                        T::XorCurrency::deposit_into_existing(&fee_source, refund_amount)
                            .unwrap_or_else(|_| {
                                <T::XorCurrency as Currency<T::AccountId>>::PositiveImbalance::zero(
                                )
                            });

                    let adjusted_paid = paid.offset(refund_imbalance).same().map_err(|_| {
                        TransactionValidityError::Invalid(InvalidTransaction::Payment)
                    })?;

                    Self::deposit_event(Event::FeeWithdrawn(
                        fee_source,
                        adjusted_paid.peek().into(),
                    ));

                    if adjusted_paid.peek().is_zero() {
                        return Ok(());
                    }

                    // Applying VAL buy-back-and-burn logic
                    let (referrer_xor, adjusted_paid) = adjusted_paid.ration(
                        T::FeeReferrerWeight::get(),
                        T::FeeXorBurnedWeight::get()
                            + T::FeeValBurnedWeight::get()
                            + T::FeeKusdBurnedWeight::get(),
                    );
                    let (xor_to_val, adjusted_paid) = adjusted_paid.ration(
                        T::FeeValBurnedWeight::get(),
                        T::FeeXorBurnedWeight::get() + T::FeeKusdBurnedWeight::get(),
                    );
                    let (xor_to_buy_back, _xor_burned) = adjusted_paid
                        .ration(T::FeeKusdBurnedWeight::get(), T::FeeXorBurnedWeight::get());
                    let mut xor_to_buy_back = xor_to_buy_back.peek();

                    if let Some(referrer) = T::ReferrerAccountProvider::get_referrer_account(who) {
                        let referrer_portion = referrer_xor.peek();
                        T::XorCurrency::resolve_creating(&referrer, referrer_xor);
                        Self::deposit_event(Event::ReferrerRewarded(
                            who.clone(),
                            referrer,
                            referrer_portion.into(),
                        ));
                    } else {
                        // Use XOR to BBB if there's no referrer
                        xor_to_buy_back = xor_to_buy_back.saturating_add(referrer_xor.peek());
                    }

                    let xor_to_val: Balance = xor_to_val.peek().unique_saturated_into();
                    let xor_to_buy_back: Balance = xor_to_buy_back.unique_saturated_into();
                    XorToVal::<T>::mutate(|balance| {
                        *balance = balance.saturating_add(xor_to_val);
                    });
                    XorToBuyBack::<T>::mutate(|balance| {
                        *balance = balance.saturating_add(xor_to_buy_back);
                    });
                }
            } else {
                // Calculate the amount to refund to the caller
                // Refund behavior is fully defined by CustomFee type or
                // by default transaction payment pallet implementation if
                // call is not subject for custom fee
                let refund_amount = paid.peek().saturating_sub(corrected_fee);

                // Refund to the the account that paid the fees. If this fails, the
                // account might have dropped below the existential balance. In
                // that case we don't refund anything.
                let refund_imbalance =
                    T::XorCurrency::deposit_into_existing(&fee_source, refund_amount)
                        .unwrap_or_else(|_| {
                            <T::XorCurrency as Currency<T::AccountId>>::PositiveImbalance::zero()
                        });

                let adjusted_paid = paid
                    .offset(refund_imbalance)
                    .same()
                    .map_err(|_| TransactionValidityError::Invalid(InvalidTransaction::Payment))?;

                #[cfg(not(feature = "wip"))]
                Self::deposit_event(Event::FeeWithdrawn(fee_source, adjusted_paid.peek().into()));
                #[cfg(feature = "wip")]
                Self::deposit_event(Event::FeeWithdrawn(
                    fee_source,
                    T::XorId::get(),
                    adjusted_paid.peek().into(),
                ));

                if adjusted_paid.peek().is_zero() {
                    return Ok(());
                }

                // Applying VAL buy-back-and-burn logic
                let (referrer_xor, adjusted_paid) = adjusted_paid.ration(
                    T::FeeReferrerWeight::get(),
                    T::FeeXorBurnedWeight::get()
                        + T::FeeValBurnedWeight::get()
                        + T::FeeKusdBurnedWeight::get(),
                );
                let (xor_to_val, adjusted_paid) = adjusted_paid.ration(
                    T::FeeValBurnedWeight::get(),
                    T::FeeXorBurnedWeight::get() + T::FeeKusdBurnedWeight::get(),
                );
                let (xor_to_buy_back, _xor_burned) = adjusted_paid
                    .ration(T::FeeKusdBurnedWeight::get(), T::FeeXorBurnedWeight::get());
                let mut xor_to_buy_back = xor_to_buy_back.peek();

                if let Some(referrer) = T::ReferrerAccountProvider::get_referrer_account(who) {
                    let referrer_portion = referrer_xor.peek();
                    T::XorCurrency::resolve_creating(&referrer, referrer_xor);
                    #[cfg(feature = "wip")]
                    Self::deposit_event(Event::ReferrerRewarded(
                        who.clone(),
                        referrer,
                        T::XorId::get(),
                        referrer_portion.into(),
                    ));
                    #[cfg(not(feature = "wip"))]
                    Self::deposit_event(Event::ReferrerRewarded(
                        who.clone(),
                        referrer,
                        referrer_portion.into(),
                    ));
                } else {
                    // Use XOR to BBB if there's no referrer
                    xor_to_buy_back = xor_to_buy_back.saturating_add(referrer_xor.peek());
                }

                let xor_to_val: Balance = xor_to_val.peek().unique_saturated_into();
                let xor_to_buy_back: Balance = xor_to_buy_back.unique_saturated_into();
                XorToVal::<T>::mutate(|balance| {
                    *balance = balance.saturating_add(xor_to_val);
                });
                XorToBuyBack::<T>::mutate(|balance| {
                    *balance = balance.saturating_add(xor_to_buy_back);
                });
            }
        }
        Ok(())
    }
}

pub type CustomFeeDetailsOf<T> =
    <<T as Config>::CustomFees as ApplyCustomFees<CallOf<T>, AccountIdOf<T>>>::FeeDetails;

/// Trait whose implementation allows to redefine extrinsics fees based
/// on the extrinsic's `Call` variant and dispatch result
pub trait ApplyCustomFees<Call: Dispatchable, AccountId> {
    /// Additinal information to be passed between `Self::compute_fee` and `Self::compute_actual_fee`
    type FeeDetails;

    /// Check if the fee payment should be postponed
    ///
    /// Parameters:
    /// `who` is the caller of the extrinsic
    /// `fee_source` is the account which will pay fees
    /// `call` is the Call extracted from the extrinsic
    /// `fee` is the pre dispatch fee
    ///
    /// Returns:
    /// `true` then fee payment should be postponed to the post dispatch phase
    /// `false` then fee should be paid at pre dispatch phase and corrected at post dispatch phase
    ///
    /// This call should check if `fee_source` will have enough funds to pay the fee after call dispatch
    /// and if not then it should return `false`
    fn should_be_postponed(
        who: &AccountId,
        fee_source: &AccountId,
        call: &Call,
        fee: Balance,
    ) -> bool;

    /// Check if the fee should be paid for this extrinsic
    ///
    /// Parameters:
    /// `who` is the caller of the extrinsic
    /// `call` is the Call extracted from the extrinsic
    ///
    /// Returns:
    /// `true` then fee should be paid
    /// `false` then fee should not be paid
    fn should_be_paid(who: &AccountId, call: &Call) -> bool;

    /// Get the account which will pay fees
    ///
    /// Parameters:
    /// `who` is the caller of the extrinsic
    /// `call` is the Call extracted from the extrinsic
    /// `fee` is the pre dispatch fee
    ///
    /// Returns account which will pay fees
    fn get_fee_source(who: &AccountId, call: &Call, fee: Balance) -> AccountId;

    /// Compute custom fees for this call
    ///
    /// Parameters:
    /// `call` is the Call extracted from the extrinsic
    ///
    /// Returns:
    /// `Some(..)` if custom fees should be applied. Then `Balance` value is used as fee
    /// and `Self::FeeDetails` is passed to `Self::compute_actual_fee` at post dispatch phase
    /// `None` if default transaction payment pallet fees should be used
    fn compute_fee(call: &Call) -> Option<(Balance, Self::FeeDetails)>;

    /// Compute actual fees for this call
    ///
    /// Parameters:
    /// `post_info` is the `PostDispatchInfo` returned from the call
    /// `info` is the `DispatchInfo` for the call
    /// `result` is the `DispatchResult` returned from the call
    /// `fee_details` is the `Self::FeeDetails` returned from the previous `Self::compute_fee` call
    ///
    /// Returns:
    /// `Some(..)` if custom post dispatch fees should be applied
    /// `None` if transaction payment pallet post dispatch fees should be used
    fn compute_actual_fee(
        post_info: &PostDispatchInfoOf<Call>,
        info: &DispatchInfoOf<Call>,
        result: &DispatchResult,
        fee_details: Option<Self::FeeDetails>,
    ) -> Option<Balance>;
}

impl<Call: Dispatchable, AccountId: Clone> ApplyCustomFees<Call, AccountId> for () {
    type FeeDetails = ();

    fn should_be_postponed(
        _who: &AccountId,
        _fee_source: &AccountId,
        _call: &Call,
        _fee: Balance,
    ) -> bool {
        false
    }

    fn should_be_paid(_who: &AccountId, _call: &Call) -> bool {
        true
    }

    fn compute_fee(_call: &Call) -> Option<(Balance, Self::FeeDetails)> {
        None
    }

    fn compute_actual_fee(
        _post_info: &PostDispatchInfoOf<Call>,
        _info: &DispatchInfoOf<Call>,
        _result: &DispatchResult,
        _fee_details: Option<Self::FeeDetails>,
    ) -> Option<Balance> {
        None
    }

    fn get_fee_source(who: &AccountId, _call: &Call, _fee: Balance) -> AccountId {
        who.clone()
    }
}

pub trait WithdrawFee<T: Config> {
    fn withdraw_fee(
        who: &T::AccountId,
        fee_source: &T::AccountId,
        call: &CallOf<T>,
        fee: Balance,
    ) -> Result<PaidFeeOf<T>, DispatchError>;
}

/// Trait for dynamic fee update via multiplier
pub trait CalculateMultiplier<AssetId, Error> {
    /// Parameters:
    /// `input_asset` is asset id which price should be fetched;
    /// `ref_asset` is asset id in which price will be fetched
    fn calculate_multiplier(input_asset: &AssetId, ref_asset: &AssetId)
        -> Result<FixedU128, Error>;
}

impl<AssetId> CalculateMultiplier<AssetId, DispatchError> for () {
    fn calculate_multiplier(
        _input_asset: &AssetId,
        _ref_asset: &AssetId,
    ) -> Result<FixedU128, DispatchError> {
        Err(DispatchError::CannotLookup)
    }
}

impl<T: Config> Pallet<T>
where
    CallOf<T>: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>,
    BalanceOf<T>: FixedPointOperand + Into<Balance>,
    T: ptp::Config<OnChargeTransaction = Pallet<T>>,
{
    pub fn multiplied_fee(mut fee: FeeDetails<BalanceOf<T>>) -> FeeDetails<BalanceOf<T>> {
        let multiplier = Multiplier::<T>::get();
        fee.inclusion_fee = fee.inclusion_fee.map(|fee| InclusionFee {
            base_fee: multiplier.saturating_mul_int(fee.base_fee),
            len_fee: multiplier.saturating_mul_int(fee.len_fee),
            adjusted_weight_fee: multiplier.saturating_mul_int(fee.adjusted_weight_fee),
        });
        fee.tip = multiplier.saturating_mul_int(fee.tip);

        fee
    }
    pub fn compute_fee_details(
        len: u32,
        call: &CallOf<T>,
        info: &DispatchInfoOf<CallOf<T>>,
        tip: BalanceOf<T>,
    ) -> (FeeDetails<BalanceOf<T>>, Option<CustomFeeDetailsOf<T>>) {
        if matches!(info.pays_fee, Pays::No) {
            return (
                FeeDetails {
                    tip,
                    inclusion_fee: None,
                },
                None,
            );
        }
        let maybe_custom_fee = T::CustomFees::compute_fee(call);
        let (fee, kind) = match maybe_custom_fee {
            Some((0, custom_details)) => (
                FeeDetails {
                    inclusion_fee: None,
                    tip,
                },
                Some(custom_details),
            ),
            Some((custom_fee, custom_details)) => (
                FeeDetails {
                    inclusion_fee: Some(InclusionFee {
                        base_fee: 0_u32.into(),
                        len_fee: 0_u32.into(),
                        adjusted_weight_fee: BalanceOf::<T>::saturated_from(custom_fee),
                    }),
                    tip,
                },
                Some(custom_details),
            ),
            None => (
                pallet_transaction_payment::Pallet::<T>::compute_fee_details(len, info, tip),
                None,
            ),
        };
        (Self::multiplied_fee(fee), kind)
    }

    pub fn compute_fee(
        len: u32,
        call: &CallOf<T>,
        info: &DispatchInfoOf<CallOf<T>>,
        tip: BalanceOf<T>,
    ) -> (BalanceOf<T>, Option<CustomFeeDetailsOf<T>>) {
        let (fee, details) = Self::compute_fee_details(len, call, info, tip);
        (fee.final_fee(), details)
    }

    pub fn compute_actual_fee(
        len: u32,
        info: &DispatchInfoOf<CallOf<T>>,
        post_info: &PostDispatchInfoOf<CallOf<T>>,
        result: &DispatchResult,
        tip: BalanceOf<T>,
        custom_fee_details: Option<CustomFeeDetailsOf<T>>,
    ) -> BalanceOf<T> {
        Self::compute_actual_fee_details(len, info, post_info, result, tip, custom_fee_details)
            .final_fee()
    }

    pub fn compute_actual_fee_details(
        len: u32,
        info: &DispatchInfoOf<CallOf<T>>,
        post_info: &PostDispatchInfoOf<CallOf<T>>,
        result: &DispatchResult,
        tip: BalanceOf<T>,
        custom_fee_details: Option<CustomFeeDetailsOf<T>>,
    ) -> FeeDetails<BalanceOf<T>> {
        let pays = post_info.pays_fee(info);
        if matches!(pays, Pays::No) {
            return FeeDetails {
                inclusion_fee: None,
                tip,
            };
        }
        let maybe_custom_fee =
            T::CustomFees::compute_actual_fee(post_info, info, result, custom_fee_details);
        let fee = match maybe_custom_fee {
            Some(0) => FeeDetails {
                inclusion_fee: None,
                tip,
            },
            Some(custom_fee) => FeeDetails {
                inclusion_fee: Some(InclusionFee {
                    base_fee: 0_u32.into(),
                    len_fee: 0_u32.into(),
                    adjusted_weight_fee: BalanceOf::<T>::saturated_from(custom_fee),
                }),
                tip,
            },
            None => pallet_transaction_payment::Pallet::<T>::compute_fee_details(len, info, tip),
        };
        Self::multiplied_fee(fee)
    }

    // Returns value if custom fee is applicable to an extrinsic and `None` otherwise
    pub fn query_info<Extrinsic: Clone + ExtrinsicT + GetDispatchInfo>(
        unchecked_extrinsic: &Extrinsic,
        call: &CallOf<T>,
        len: u32,
    ) -> RuntimeDispatchInfo<BalanceOf<T>> {
        let dispatch_info = <Extrinsic as GetDispatchInfo>::get_dispatch_info(unchecked_extrinsic);

        let partial_fee = if unchecked_extrinsic.is_signed().unwrap_or(false) {
            Self::compute_fee(len, call, &dispatch_info, 0u32.into()).0
        } else {
            0u32.into()
        };

        let DispatchInfo { weight, class, .. } = dispatch_info;

        RuntimeDispatchInfo {
            weight,
            class,
            partial_fee,
        }
    }

    // Returns value if custom fee is applicable to an extrinsic and `None` otherwise
    pub fn query_fee_details<Extrinsic: ExtrinsicT + GetDispatchInfo>(
        unchecked_extrinsic: &Extrinsic,
        call: &CallOf<T>,
        len: u32,
    ) -> FeeDetails<BalanceOf<T>> {
        let info = <Extrinsic as GetDispatchInfo>::get_dispatch_info(unchecked_extrinsic);
        Self::compute_fee_details(len, call, &info, 0u32.into()).0
    }
}

impl<T: Config> Pallet<T> {
    pub fn random_remint() -> Weight {
        let mut weight = Weight::default();
        weight.saturating_accrue(T::DbWeight::get().reads(3));
        let period = RemintPeriod::<T>::get();
        let (randomness, _) = T::Randomness::random(&b"xor-fee"[..]);
        match u32::decode(&mut randomness.as_ref()) {
            Ok(random_number) => {
                if random_number % period == 0 {
                    weight.saturating_accrue(T::DbWeight::get().reads(2));
                    let mut xor_to_val = Balance::zero();
                    let mut xor_to_buy_back = Balance::zero();

                    #[cfg(feature = "wip")] // Xorless fee
                    Self::remint_fee_asset(&mut weight, &mut xor_to_val, &mut xor_to_buy_back);

                    xor_to_val = xor_to_val.saturating_add(XorToVal::<T>::take());
                    if xor_to_val != 0 {
                        if let Err(e) = Self::remint_val(&mut weight, xor_to_val) {
                            error!("xor fee remint failed: {:?}", e);
                        }
                    }

                    xor_to_buy_back = xor_to_buy_back.saturating_add(XorToBuyBack::<T>::take());

                    if xor_to_buy_back != 0 {
                        if let Err(e) = Self::remint_buy_back(&mut weight, xor_to_buy_back) {
                            error!("XOR remint buy back failed: {:?}", e);
                        }
                    }
                }
            }
            Err(error) => {
                warn!("Failed to get randomness for xor-fee: {}", error);
            }
        }
        weight
    }

    pub fn remint_val(weight: &mut Weight, xor_to_val: Balance) -> Result<(), DispatchError> {
        let tech_account_id = <T as Config>::GetTechnicalAccountId::get();
        let xor = T::XorId::get();
        let val = T::ValId::get();
        let kusd = T::KusdId::get();
        let tbcd = T::TbcdId::get();

        // Re-minting the `xor_to_val` tokens amount to `tech_account_id` of this pallet.
        // The tokens being re-minted had initially been withdrawn as a part of the fee.
        weight.saturating_accrue(T::DbWeight::get().reads_writes(2, 1));
        T::AssetManager::mint_to(&xor, &tech_account_id, &tech_account_id, xor_to_val)?;
        // Attempting to swap XOR with VAL on secondary market
        // If successful, VAL will be burned, otherwise burn newly minted XOR from the tech account
        weight.saturating_accrue(T::PoolXyk::exchange_weight());
        match T::LiquidityProxy::exchange(
            T::DEXIdValue::get(),
            &tech_account_id,
            &tech_account_id,
            &xor,
            &val,
            SwapAmount::WithDesiredInput {
                desired_amount_in: xor_to_val,
                min_amount_out: 0,
            },
            LiquiditySourceFilter::with_forbidden(
                T::DEXIdValue::get(),
                [LiquiditySourceType::MulticollateralBondingCurvePool].into(),
            ),
        ) {
            Ok(swap_outcome) => {
                let mut val_to_burn = swap_outcome.amount;
                let tbcd_buy_back = T::RemintTbcdBuyBackPercent::get() * val_to_burn;
                let kusd_buy_back = T::RemintKusdBuyBackPercent::get() * val_to_burn;

                if let Err(e) = common::with_transaction(|| {
                    weight.saturating_accrue(
                        T::DbWeight::get()
                            .reads_writes(2, 1)
                            .saturating_add(T::PoolXyk::exchange_weight()),
                    );
                    T::BuyBackHandler::buy_back_and_burn(
                        &tech_account_id,
                        &val,
                        &tbcd,
                        tbcd_buy_back,
                    )
                }) {
                    frame_support::log::error!("Failed to buy back TBCD: {e:?}");
                } else {
                    val_to_burn = val_to_burn.saturating_sub(tbcd_buy_back);
                }

                if let Err(e) = common::with_transaction(|| {
                    weight.saturating_accrue(
                        T::DbWeight::get()
                            .reads_writes(2, 1)
                            .saturating_add(T::PoolXyk::exchange_weight()),
                    );
                    T::BuyBackHandler::buy_back_and_burn(
                        &tech_account_id,
                        &val,
                        &kusd,
                        kusd_buy_back,
                    )
                }) {
                    frame_support::log::error!("Failed to buy back KUSD: {e:?}");
                } else {
                    val_to_burn = val_to_burn.saturating_sub(kusd_buy_back);
                }

                weight.saturating_accrue(T::DbWeight::get().reads_writes(1, 1));
                T::OnValBurned::on_val_burned(val_to_burn);
                weight.saturating_accrue(T::DbWeight::get().reads_writes(2, 1));
                T::AssetManager::burn_from(&val, &tech_account_id, &tech_account_id, val_to_burn)?;
            }
            Err(e) => {
                error!(
                    "failed to exchange xor to val, burning {} XOR, e: {:?}",
                    xor_to_val, e
                );
                weight.saturating_accrue(T::DbWeight::get().reads_writes(2, 1));
                T::AssetManager::burn_from(&xor, &tech_account_id, &tech_account_id, xor_to_val)?;
            }
        }

        Ok(())
    }

    pub fn remint_buy_back(
        weight: &mut Weight,
        xor_to_buy_back: Balance,
    ) -> Result<(), DispatchError> {
        let xor = T::XorId::get();
        let kusd = T::KusdId::get();
        common::with_transaction(|| {
            weight.saturating_accrue(
                T::DbWeight::get()
                    .reads_writes(4, 2)
                    .saturating_add(T::PoolXyk::exchange_weight()),
            );
            T::BuyBackHandler::mint_buy_back_and_burn(&xor, &kusd, xor_to_buy_back)
        })?;

        Ok(())
    }

    #[cfg(feature = "wip")] // Xorless fee
    pub fn remint_fee_asset(
        weight: &mut Weight,
        xor_to_val: &mut Balance,
        xor_to_buy_back: &mut Balance,
    ) {
        BurntForFee::<T>::iter().for_each(|(asset_id, asset_fee)| {
            weight.saturating_accrue(T::DbWeight::get().reads(1));
            let mut process_fee = |fee: Balance, additional_weight: u32| -> Result<(), ()> {
                if fee.is_zero() {
                    return Ok(());
                };
                match Self::remint_asset(weight, asset_id, fee) {
                    Ok(burnt_xor) => {
                        *xor_to_val =
                            xor_to_val.saturating_add(Self::calculate_portion_fee_from_weight(
                                T::FeeValBurnedWeight::get(),
                                burnt_xor,
                            ));
                        *xor_to_buy_back = xor_to_buy_back.saturating_add(
                            Self::calculate_portion_fee_from_weight(
                                T::FeeKusdBurnedWeight::get() + additional_weight,
                                burnt_xor,
                            ),
                        );
                        Ok(())
                    }
                    Err(e) => {
                        error!("white listed asset fee remint failed: {:?}", e);
                        Err(())
                    }
                }
            };

            // referral part going to the xor to buy back
            match (
                process_fee(asset_fee.fee, u32::zero()),
                process_fee(asset_fee.fee_without_referral, T::FeeReferrerWeight::get()),
            ) {
                (Ok(()), Ok(())) => {
                    BurntForFee::<T>::remove(asset_id);
                }
                (Ok(()), Err(())) if !asset_fee.fee.is_zero() => {
                    BurntForFee::<T>::mutate(asset_id, |fee| fee.fee = Balance::zero())
                }
                (Err(()), Ok(())) if !asset_fee.fee_without_referral.is_zero() => {
                    BurntForFee::<T>::mutate(asset_id, |fee| {
                        fee.fee_without_referral = Balance::zero()
                    })
                }
                _ => {}
            }
            weight.saturating_accrue(T::DbWeight::get().reads_writes(1, 1));
        });
    }

    #[cfg(feature = "wip")] // Xorless fee
    pub fn remint_asset(
        weight: &mut Weight,
        asset_id: AssetIdOf<T>,
        amount: Balance,
    ) -> Result<Balance, DispatchError> {
        let tech_account_id = <T as Config>::GetTechnicalAccountId::get();
        let xor = T::XorId::get();
        weight.saturating_accrue(T::DbWeight::get().reads_writes(2, 1));
        T::AssetManager::mint_to(&asset_id, &tech_account_id, &tech_account_id, amount)?;
        weight.saturating_accrue(T::PoolXyk::exchange_weight());
        match T::PoolXyk::exchange(
            &tech_account_id,
            &tech_account_id,
            &T::DEXIdValue::get(),
            &asset_id,
            &xor,
            SwapAmount::WithDesiredInput {
                desired_amount_in: amount,
                min_amount_out: T::MinimalFeeInAsset::get(),
            },
        ) {
            Ok(xor_to_burn) => {
                weight.saturating_accrue(T::DbWeight::get().reads_writes(2, 1));
                T::AssetManager::burn_from(
                    &xor,
                    &tech_account_id,
                    &tech_account_id,
                    xor_to_burn.0.amount,
                )?;
                Ok(xor_to_burn.0.amount)
            }
            Err(e) => {
                error!(
                    "failed to exchange asset {:?} to xor, burning {} asset, e: {:?}",
                    asset_id, amount, e
                );
                weight.saturating_accrue(T::DbWeight::get().reads_writes(2, 1));
                T::AssetManager::burn_from(&asset_id, &tech_account_id, &tech_account_id, amount)?;
                Err(e)
            }
        }
    }

    #[cfg(feature = "wip")] // Xorless fee
    pub fn calculate_portion_fee_from_weight(portion: u32, whole_amount: Balance) -> Balance {
        let portion = Perbill::from_rational(
            portion,
            T::FeeReferrerWeight::get()
                .saturating_add(T::FeeXorBurnedWeight::get())
                .saturating_add(T::FeeValBurnedWeight::get())
                .saturating_add(T::FeeKusdBurnedWeight::get()),
        );
        portion * whole_amount
    }

    #[cfg(feature = "wip")] // Xorless fee
    fn do_remove_asset_from_white_list(asset_id: AssetIdOf<T>) -> DispatchResultWithPostInfo {
        WhitelistTokensForFee::<T>::try_mutate(|whitelist| {
            if let Some(pos) = whitelist.iter().position(|id| *id == asset_id) {
                whitelist.remove(pos);
                Self::deposit_event(Event::AssetRemovedFromWhiteList(asset_id));
                Ok(().into())
            } else {
                Err(Error::<T>::AssetNotFound.into())
            }
        })
    }

    #[cfg(feature = "wip")] // Xorless fee
    fn do_add_asset_to_white_list(asset_id: AssetIdOf<T>) -> DispatchResultWithPostInfo {
        WhitelistTokensForFee::<T>::try_mutate(|whitelist| {
            if whitelist.contains(&asset_id) {
                return Err(Error::<T>::AssetAlreadyWhitelisted.into());
            }

            whitelist
                .try_push(asset_id)
                .map_err(|_| Error::<T>::WhitelistFull)?;

            Self::deposit_event(Event::AssetAddedToWhiteList(asset_id));
            Ok(().into())
        })
    }
}

pub use pallet::*;

pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use common::{AssetIdOf, PriceToolsProvider};
    use frame_support::pallet_prelude::*;
    use frame_support::traits::{Randomness, StorageVersion};
    use frame_system::pallet_prelude::*;

    #[pallet::config]
    pub trait Config:
        frame_system::Config + pallet_transaction_payment::Config + common::Config
    {
        type PermittedSetPeriod: EnsureOrigin<Self::RuntimeOrigin>;
        type DynamicMultiplier: CalculateMultiplier<AssetIdOf<Self>, DispatchError>;
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// XOR - The native currency of this blockchain.
        type XorCurrency: Currency<Self::AccountId> + Send + Sync;
        type XorId: Get<AssetIdOf<Self>>;
        type ValId: Get<AssetIdOf<Self>>;
        type KusdId: Get<AssetIdOf<Self>>;
        type TbcdId: Get<AssetIdOf<Self>>;
        type FeeReferrerWeight: Get<u32>;
        type FeeXorBurnedWeight: Get<u32>;
        type FeeValBurnedWeight: Get<u32>;
        type FeeKusdBurnedWeight: Get<u32>;
        type RemintTbcdBuyBackPercent: Get<Percent>;
        type RemintKusdBuyBackPercent: Get<Percent>;
        type DEXIdValue: Get<Self::DEXId>;
        type LiquidityProxy: LiquidityProxyTrait<Self::DEXId, Self::AccountId, AssetIdOf<Self>>;
        type OnValBurned: OnValBurned;
        type CustomFees: ApplyCustomFees<CallOf<Self>, Self::AccountId>;
        type GetTechnicalAccountId: Get<Self::AccountId>;
        type FullIdentification;
        type ReferrerAccountProvider: ReferrerAccountProvider<Self::AccountId>;
        type BuyBackHandler: BuyBackHandler<Self::AccountId, AssetIdOf<Self>>;
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
        type WithdrawFee: WithdrawFee<Self>;
        type MaxWhiteListTokens: Get<u32>;
        type RuntimeCall: Parameter
            + Dispatchable<RuntimeOrigin = Self::RuntimeOrigin, PostInfo = PostDispatchInfo>
            + GetDispatchInfo
            + From<frame_system::Call<Self>>;
        type PoolXyk: LiquiditySource<
            Self::DEXId,
            Self::AccountId,
            AssetIdOf<Self>,
            Balance,
            DispatchError,
        >;
        type WhiteListOrigin: EnsureOrigin<Self::RuntimeOrigin>;
        type PriceTools: PriceToolsProvider<AssetIdOf<Self>>;
        /// Main goal of the constant is to prevent zero fees
        type MinimalFeeInAsset: Get<Balance>;
        type Randomness: Randomness<Self::Hash, Self::BlockNumber>;
    }

    /// The current storage version.
    const STORAGE_VERSION: StorageVersion = StorageVersion::new(2);

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        #[cfg(feature = "wip")] // Dynamic fee
        fn on_initialize(current_block: BlockNumberFor<T>) -> Weight {
            let update_period = Self::update_period(); // 1 read
            let mut weight: Weight = T::DbWeight::get().reads(1);
            if !update_period.is_zero()
                && current_block % update_period == BlockNumberFor::<T>::zero()
            {
                match T::DynamicMultiplier::calculate_multiplier(
                    &common::XOR.into(),
                    &common::DAI.into(),
                ) {
                    Ok(new_multiplier) => {
                        <Multiplier<T>>::put(new_multiplier); // 1 write
                        Self::deposit_event(Event::WeightToFeeMultiplierUpdated(new_multiplier));
                        weight += T::DbWeight::get().writes(1);
                    }
                    Err(e) => {
                        frame_support::log::error!("Could not update Multiplier due to: {e:?}");
                    }
                }
            }
            weight.saturating_accrue(Self::random_remint());
            weight
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Update the multiplier for weight -> fee conversion.
        // TODO: benchmark on reference hardware
        // 0 is passed because argument is unused and no need to
        // do unnecessary conversions
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::update_multiplier())]
        pub fn update_multiplier(
            origin: OriginFor<T>,
            new_multiplier: FixedU128,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            <Multiplier<T>>::put(new_multiplier);
            Self::deposit_event(Event::WeightToFeeMultiplierUpdated(new_multiplier));
            Ok(().into())
        }

        /// Set new update period for `xor_fee::Multiplier` updating
        /// Set 0 to stop updating
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::set_fee_update_period())]
        pub fn set_fee_update_period(
            origin: OriginFor<T>,
            _new_period: <T as frame_system::Config>::BlockNumber,
        ) -> DispatchResultWithPostInfo {
            T::PermittedSetPeriod::ensure_origin(origin)?;
            #[cfg(feature = "wip")] // Dynamic fee
            {
                <UpdatePeriod<T>>::put(_new_period);
                Self::deposit_event(Event::PeriodUpdated(_new_period));
            }
            Ok(().into())
        }

        /// Set new small reference amount `xor_fee::SmallReferenceAmount`
        /// Small fee should tend to the amount value
        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::set_small_reference_amount())]
        pub fn set_small_reference_amount(
            origin: OriginFor<T>,
            new_reference_amount: Balance,
        ) -> DispatchResultWithPostInfo {
            ensure!(
                !new_reference_amount.is_zero(),
                Error::<T>::InvalidSmallReferenceAmount
            );
            ensure_root(origin)?;
            #[cfg(feature = "wip")] // Dynamic fee
            {
                <SmallReferenceAmount<T>>::put(new_reference_amount);
                Self::deposit_event(Event::SmallReferenceAmountUpdated(new_reference_amount));
            }
            Ok(().into())
        }

        /// Allow use assets from white list to pay for fee
        /// # Parameters:
        /// - `origin`: caller
        /// - `call`: dispatch call for which pay fee
        /// - `asset_id`: asset in which pay fee, where None - XOR
        #[allow(unused_variables)] // Used in extension
        #[pallet::call_index(3)]
        #[pallet::weight({
            let dispatch_info = call.get_dispatch_info();
            (
				<T as Config>::WeightInfo::xorless_call()
					.saturating_add(dispatch_info.weight),
				dispatch_info.class,
			)
        })]
        pub fn xorless_call(
            origin: OriginFor<T>,
            call: Box<<T as Config>::RuntimeCall>,
            asset_id: Option<AssetIdOf<T>>,
        ) -> DispatchResultWithPostInfo {
            ensure_signed(origin.clone())?;
            #[cfg(feature = "wip")] // Xorless fee
            {
                let call_info = call.get_dispatch_info();
                let call_result = call.dispatch(origin);
                let whole_weight = T::WeightInfo::xorless_call()
                    .saturating_add(extract_actual_weight(&call_result, &call_info));

                call_result
                    .map_err(|mut err| {
                        err.post_info = Some(whole_weight).into();
                        err
                    })
                    .map(|_| Some(whole_weight).into())
            }
            #[cfg(not(feature = "wip"))] // Xorless fee
            Err(DispatchError::Unavailable.into())
        }

        #[allow(unused_variables)] // Used in extension
        #[pallet::call_index(4)]
        #[pallet::weight(<T as Config>::WeightInfo::add_asset_to_white_list())]
        pub fn add_asset_to_white_list(
            origin: OriginFor<T>,
            asset_id: AssetIdOf<T>,
        ) -> DispatchResultWithPostInfo {
            <T as Config>::WhiteListOrigin::ensure_origin(origin)?;
            #[cfg(feature = "wip")] // Xorless fee
            return Self::do_add_asset_to_white_list(asset_id);
            #[cfg(not(feature = "wip"))] // Xorless fee
            Ok(().into())
        }

        #[allow(unused_variables)] // Used in extension
        #[pallet::call_index(5)]
        #[pallet::weight(<T as Config>::WeightInfo::remove_asset_from_white_list())]
        pub fn remove_asset_from_white_list(
            origin: OriginFor<T>,
            asset_id: AssetIdOf<T>,
        ) -> DispatchResultWithPostInfo {
            <T as Config>::WhiteListOrigin::ensure_origin(origin)?;
            #[cfg(feature = "wip")] // Xorless fee
            return Self::do_remove_asset_from_white_list(asset_id);
            #[cfg(not(feature = "wip"))] // Xorless fee
            Ok(().into())
        }

        #[pallet::call_index(6)]
        #[pallet::weight(<T as Config>::WeightInfo::remove_asset_from_white_list())]
        pub fn set_random_remint_period(
            origin: OriginFor<T>,
            period: u32,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            ensure!(
                !period.is_zero() && period <= 600,
                Error::<T>::WrongRemintPeriod
            );
            RemintPeriod::<T>::set(period);
            Self::deposit_event(Event::RemintPeriodUpdated(period));
            Ok(().into())
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        #[cfg(feature = "wip")] // Xorless fee
        /// Fee has been withdrawn from user. [Account Id to withdraw from, Asset Id to withdraw, Fee Amount]
        FeeWithdrawn(AccountIdOf<T>, AssetIdOf<T>, Balance),
        #[cfg(feature = "wip")] // Xorless fee
        /// The portion of fee is sent to the referrer. [Referral, Referrer, AssetId, Amount]
        ReferrerRewarded(AccountIdOf<T>, AccountIdOf<T>, AssetIdOf<T>, Balance),
        #[cfg(not(feature = "wip"))] // Xorless fee
        /// Fee has been withdrawn from user. [Account Id to withdraw from, Fee Amount]
        FeeWithdrawn(AccountIdOf<T>, Balance),
        #[cfg(not(feature = "wip"))] // Xorless fee
        /// The portion of fee is sent to the referrer. [Referral, Referrer, Amount]
        ReferrerRewarded(AccountIdOf<T>, AccountIdOf<T>, Balance),
        /// New multiplier for weight to fee conversion is set
        /// (*1_000_000_000_000_000_000). [New value]
        WeightToFeeMultiplierUpdated(FixedU128),
        #[cfg(feature = "wip")] // Dynamic fee
        /// New block number to update multiplier is set. [New value]
        PeriodUpdated(<T as frame_system::Config>::BlockNumber),
        #[cfg(feature = "wip")] // Dynamic fee
        /// New small reference amount set. [New value]
        SmallReferenceAmountUpdated(Balance),
        /// White list updated: [Asset removed]
        AssetRemovedFromWhiteList(AssetIdOf<T>),
        /// White list updated: [Asset added]
        AssetAddedToWhiteList(AssetIdOf<T>),
        /// Average remint period updated: [Period]
        RemintPeriodUpdated(u32),
    }
    #[pallet::error]
    pub enum Error<T> {
        /// Failed to calculate new multiplier.
        MultiplierCalculationFailed,
        /// `SmallReferenceAmount` is unsupported
        InvalidSmallReferenceAmount,
        /// Asset is not found in white list
        AssetNotFound, // Error index used for InvalidTransaction::Custom(index)
        /// Asset already in white list
        AssetAlreadyWhitelisted,
        /// White list is filled
        WhitelistFull,
        /// Failed to calculate fee in white listed asset
        FeeCalculationFailed,
        /// Remint period should not be 0 or to be greater than 600
        WrongRemintPeriod,
    }

    #[cfg(feature = "wip")] // Xorless fee
    /// AssetId -> Amount to pay for fee
    #[pallet::storage]
    #[pallet::getter(fn burnt_for_fee)]
    pub type BurntForFee<T: Config> =
        StorageMap<_, Blake2_128Concat, AssetIdOf<T>, AssetFee, ValueQuery>;

    #[cfg(feature = "wip")] // Xorless fee
    /// Tokens allowed for xorless execution
    #[pallet::storage]
    #[pallet::getter(fn whitelist_tokens)]
    pub type WhitelistTokensForFee<T: Config> =
        StorageValue<_, BoundedVec<AssetIdOf<T>, T::MaxWhiteListTokens>, ValueQuery>;

    #[cfg(feature = "wip")] // Dynamic fee
    /// Small fee value should be `SmallReferenceAmount` in reference asset id
    #[pallet::storage]
    #[pallet::getter(fn small_reference_amount)]
    pub type SmallReferenceAmount<T: Config> = StorageValue<_, Balance, ValueQuery>;

    #[cfg(feature = "wip")] // Dynamic fee
    /// Next block number to update multiplier
    /// If it is necessary to stop updating the multiplier,
    /// set 0 value
    #[pallet::storage]
    #[pallet::getter(fn update_period)]
    pub type UpdatePeriod<T> =
        StorageValue<_, <T as frame_system::Config>::BlockNumber, ValueQuery>;

    /// The amount of XOR to be reminted and exchanged for VAL at the end of the session
    #[pallet::storage]
    #[pallet::getter(fn xor_to_val)]
    pub type XorToVal<T: Config> = StorageValue<_, Balance, ValueQuery>;

    /// The amount of XOR to be reminted and exchanged for KUSD at the end of the session
    #[pallet::storage]
    #[pallet::getter(fn xor_to_kusd)]
    pub type XorToBuyBack<T: Config> = StorageValue<_, Balance, ValueQuery>;

    #[pallet::type_value]
    pub fn DefaultForFeeMultiplier<T: Config>() -> FixedU128 {
        FixedU128::from(600000)
    }

    // Multiplier used in WeightToFee conversion
    #[pallet::storage]
    #[pallet::getter(fn multiplier)]
    pub type Multiplier<T> = StorageValue<_, FixedU128, ValueQuery, DefaultForFeeMultiplier<T>>;

    #[pallet::type_value]
    pub fn DefaultForRemintPeriod<T: Config>() -> u32 {
        100
    }

    // Average period for random remint
    #[pallet::storage]
    #[pallet::getter(fn remint_period)]
    pub type RemintPeriod<T> = StorageValue<_, u32, ValueQuery, DefaultForRemintPeriod<T>>;

    // This affects `base_fee` and `weight_fee`. `length_fee` is too small
    // in comparison to them, so we should be fine multiplying only this parts.
    impl<T: Config> WeightToFeePolynomial for Pallet<T> {
        type Balance = Balance;

        fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
            smallvec!(WeightToFeeCoefficient {
                // 7_000_000 was the original coefficient taken as reference
                coeff_integer: 7_000_000,
                coeff_frac: Perbill::zero(),
                negative: false,
                degree: 1,
            })
        }
    }
}
