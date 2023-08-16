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
// TODO #167: fix clippy warnings
#![allow(clippy::all)]

use common::prelude::SwapAmount;
use common::{
    AssetInfoProvider, Balance, BuyBackHandler, FilterMode, LiquidityProxyTrait,
    LiquiditySourceFilter, LiquiditySourceType, OnValBurned,
};
use frame_support::dispatch::{DispatchInfo, GetDispatchInfo, Pays};
use frame_support::log::error;
use frame_support::pallet_prelude::InvalidTransaction;
use frame_support::traits::{Currency, ExistenceRequirement, Get, Imbalance, WithdrawReasons};
use frame_support::unsigned::TransactionValidityError;
use frame_support::weights::{
    WeightToFeeCoefficient, WeightToFeeCoefficients, WeightToFeePolynomial,
};
use pallet_transaction_payment::{
    FeeDetails, InclusionFee, OnChargeTransaction, RuntimeDispatchInfo,
};
use smallvec::smallvec;
use sp_runtime::generic::{CheckedExtrinsic, UncheckedExtrinsic};
use sp_runtime::traits::{
    DispatchInfoOf, Dispatchable, Extrinsic as ExtrinsicT, PostDispatchInfoOf, SaturatedConversion,
    SignedExtension, UniqueSaturatedInto, Zero,
};
use sp_runtime::{DispatchError, FixedPointNumber, FixedU128, Perbill, Percent};
use sp_staking::SessionIndex;
use sp_std::vec::Vec;

mod benchmarking;
pub mod weights;

#[cfg(test)]
pub mod mock;

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
type Assets<T> = assets::Pallet<T>;

// #[cfg_attr(test, derive(PartialEq))]
pub enum LiquidityInfo<T: Config> {
    /// Fees operate as normal
    Paid((T::AccountId, Option<NegativeImbalanceOf<T>>)),
    /// The fee payment has been postponed to after the transaction
    Postponed(T::AccountId, BalanceOf<T>),
    /// Default value
    NotPaid,
}

impl<T: Config> sp_std::fmt::Debug for LiquidityInfo<T> {
    fn fmt(&self, f: &mut sp_std::fmt::Formatter<'_>) -> sp_std::fmt::Result {
        match self {
            LiquidityInfo::Paid((a, b)) => {
                write!(f, "Paid({:?}, {:?})", a, b.as_ref().map(|b| b.peek()))
            }
            LiquidityInfo::Postponed(account_id, b) => {
                write!(f, "Postponed({:?}, {:?})", account_id, b)
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
            (LiquidityInfo::Paid((a1, b1)), LiquidityInfo::Paid((a2, b2))) => {
                (a1 == a2) && b1.as_ref().map(|b| b.peek()) == b2.as_ref().map(|b| b.peek())
            }
            (LiquidityInfo::Postponed(a1, b1), LiquidityInfo::Postponed(a2, b2)) => {
                a1 == a2 && b1 == b2
            }
            _ => false,
        }
    }
}

impl<T: Config> Default for LiquidityInfo<T> {
    fn default() -> Self {
        LiquidityInfo::NotPaid
    }
}

impl<T: Config> From<(T::AccountId, Option<NegativeImbalanceOf<T>>)> for LiquidityInfo<T> {
    fn from((account_id, paid): (T::AccountId, Option<NegativeImbalanceOf<T>>)) -> Self {
        LiquidityInfo::Paid((account_id, paid))
    }
}

impl<T: Config> OnChargeTransaction<T> for Pallet<T>
where
    CallOf<T>: ExtractProxySwap<
            AccountId = T::AccountId,
            DexId = T::DEXId,
            AssetId = T::AssetId,
            Amount = SwapAmount<u128>,
        > + IsCalledByBridgePeer<T::AccountId>,
    BalanceOf<T>: Into<u128>,
    DispatchInfoOf<CallOf<T>>: Into<DispatchInfo> + Clone,
{
    type Balance = BalanceOf<T>;
    type LiquidityInfo = LiquidityInfo<T>;

    fn withdraw_fee(
        who: &T::AccountId,
        call: &CallOf<T>,
        _dispatch_info: &DispatchInfoOf<CallOf<T>>,
        fee: Self::Balance,
        _tip: Self::Balance,
    ) -> Result<Self::LiquidityInfo, TransactionValidityError> {
        if fee.is_zero() {
            return Ok((who.clone(), None).into());
        }

        let maybe_custom_fee = T::CustomFees::compute_fee(call);
        let final_fee: BalanceOf<T> = match maybe_custom_fee {
            Some(value) => BalanceOf::<T>::saturated_from(value),
            _ => fee,
        };

        if let Ok(result) = T::WithdrawFee::withdraw_fee(who, call, final_fee.into()) {
            return Ok(result.into());
        }

        // In case we are producing XOR, we perform exchange before fees are withdraw to allow 0-XOR accounts to trade
        let SwapInfo {
            fee_source,
            dex_id,
            input_asset_id,
            output_asset_id,
            amount,
            filter_mode,
            selected_source_types,
        } = call
            .extract()
            .ok_or(TransactionValidityError::from(InvalidTransaction::Payment))?;

        if output_asset_id != T::XorId::get() {
            return Err(InvalidTransaction::Payment.into());
        }

        // Check how much user has input asset
        let user_input_balance = assets::Pallet::<T>::free_balance(&input_asset_id, who)
            .map_err(|_| TransactionValidityError::from(InvalidTransaction::Payment))?;

        // How much does the user want to spend of their input asset
        let swap_input_amount = match amount {
            SwapAmount::WithDesiredInput {
                desired_amount_in, ..
            } => desired_amount_in,
            SwapAmount::WithDesiredOutput { max_amount_in, .. } => max_amount_in,
        };

        // The amount of input asset needed for this swap is more than the user has, so error
        if swap_input_amount > user_input_balance {
            return Err(InvalidTransaction::Payment.into());
        }

        let filter = LiquiditySourceFilter::with_mode(dex_id, filter_mode, selected_source_types);

        // Quote to see if there will be enough funds for the fee
        let swap =
            <T::LiquidityProxy as LiquidityProxyTrait<T::DEXId, T::AccountId, T::AssetId>>::quote(
                dex_id,
                &input_asset_id,
                &output_asset_id,
                amount.into(),
                filter.clone(),
                true,
            )
            .map_err(|_| InvalidTransaction::Payment)?;

        // Quote does not check if max_in or min_out are respected
        let (limits_ok, output_amount) = match amount {
            SwapAmount::WithDesiredInput { min_amount_out, .. } => {
                (swap.amount >= min_amount_out, swap.amount)
            }
            SwapAmount::WithDesiredOutput {
                desired_amount_out,
                max_amount_in,
                ..
            } => (swap.amount <= max_amount_in, desired_amount_out),
        };

        let fee_source = fee_source.unwrap_or(who.clone());
        // Check the swap result + existing balance is enough for fee
        if limits_ok
            && T::XorCurrency::free_balance(&fee_source).into() + output_amount
                - T::XorCurrency::minimum_balance().into()
                >= final_fee.into()
        {
            // The fee is applied afterwards, in correct_and_deposit_fee
            return Ok(LiquidityInfo::Postponed(fee_source, final_fee));
        }

        Err(InvalidTransaction::Payment.into())
    }

    fn correct_and_deposit_fee(
        who: &T::AccountId,
        _dispatch_info: &DispatchInfoOf<CallOf<T>>,
        _post_info: &PostDispatchInfoOf<CallOf<T>>,
        corrected_fee: Self::Balance,
        tip: Self::Balance,
        already_withdrawn: Self::LiquidityInfo,
    ) -> Result<(), TransactionValidityError> {
        let (fee_source, withdrawn) = match already_withdrawn {
            LiquidityInfo::Paid(opt) => opt,
            LiquidityInfo::Postponed(fee_source, fee) => {
                let withdraw_reason = if tip.is_zero() {
                    WithdrawReasons::TRANSACTION_PAYMENT
                } else {
                    WithdrawReasons::TRANSACTION_PAYMENT | WithdrawReasons::TIP
                };
                let result = T::XorCurrency::withdraw(
                    &fee_source,
                    fee,
                    withdraw_reason,
                    ExistenceRequirement::KeepAlive,
                )
                .ok();
                (fee_source, result)
            }
            LiquidityInfo::NotPaid => (who.clone(), None),
        };

        if let Some(paid) = withdrawn {
            // Calculate the amount to refund to the caller
            // A refund is possible in two cases:
            //  - the `Dispatchable:PostInfo` structure has the `pays_fee` field changed
            //    from `Payes::Yes` to `Pays::No` during exection. In this case the `corrected_fee`
            //    will be 0 so that the entire withdrawn amount should be refunded to the caller;
            //  - the extrinsic is not subject to the manual fees applied by means of the
            //    `ApplyCustomFees` trait implementation so that the withdrawn amount is
            //    completely defined by the extrinsic's weight and can change based on the
            //    `actual_weight` from the `Dispatchable::PostInfo` structure.
            // TODO: only the former case is currently supported; for the latter case we need a
            // reliable way to determine whether the extrinsic is or is not subject to manual fees.
            let refund_amount: Self::Balance = if corrected_fee == 0_u32.into() {
                paid.peek()
            } else {
                Self::Balance::zero()
            };

            // Refund to the the account that paid the fees. If this fails, the
            // account might have dropped below the existential balance. In
            // that case we don't refund anything.
            let refund_imbalance =
                T::XorCurrency::deposit_into_existing(&fee_source, refund_amount).unwrap_or_else(
                    |_| <T::XorCurrency as Currency<T::AccountId>>::PositiveImbalance::zero(),
                );

            // Offset the imbalance caused by paying the fees against the refunded amount.
            let adjusted_paid = paid
                .offset(refund_imbalance)
                .same()
                .map_err(|_| TransactionValidityError::Invalid(InvalidTransaction::Payment))?;

            Self::deposit_event(Event::FeeWithdrawn(
                fee_source.clone(),
                adjusted_paid.peek(),
            ));

            // Applying VAL buy-back-and-burn logic
            let xor_burned_weight = T::XorBurnedWeight::get();
            let xor_into_val_burned_weight = T::XorIntoValBurnedWeight::get();
            let (referrer_xor, adjusted_paid) = adjusted_paid.ration(
                T::ReferrerWeight::get(),
                xor_burned_weight + xor_into_val_burned_weight,
            );
            if let Some(referrer) = referrals::Pallet::<T>::referrer_account(who) {
                let referrer_portion = referrer_xor.peek();
                if T::XorCurrency::resolve_into_existing(&referrer, referrer_xor).is_ok() {
                    Self::deposit_event(Event::ReferrerRewarded(
                        who.clone(),
                        referrer,
                        referrer_portion.into(),
                    ));
                }
            }

            // TODO: decide what should be done with XOR if there is no referrer.
            // Burn XOR for now
            let (_xor_burned, xor_to_val) =
                adjusted_paid.ration(xor_burned_weight, xor_into_val_burned_weight);
            let xor_to_val: Balance = xor_to_val.peek().unique_saturated_into();
            XorToVal::<T>::mutate(|balance| {
                *balance += xor_to_val;
            });
        }
        Ok(())
    }
}

impl<T: Config> pallet_session::SessionManager<T::AccountId> for Pallet<T> {
    fn new_session(new_index: SessionIndex) -> Option<Vec<T::AccountId>> {
        <<T as Config>::SessionManager as pallet_session::SessionManager<_>>::new_session(new_index)
    }

    fn end_session(end_index: SessionIndex) {
        <<T as Config>::SessionManager as pallet_session::SessionManager<_>>::end_session(end_index)
    }

    fn start_session(start_index: SessionIndex) {
        <<T as Config>::SessionManager as pallet_session::SessionManager<_>>::start_session(
            start_index,
        )
    }

    fn new_session_genesis(new_index: SessionIndex) -> Option<Vec<T::AccountId>> {
        <<T as Config>::SessionManager as pallet_session::SessionManager<_>>::new_session_genesis(
            new_index,
        )
    }
}

impl<T: Config> pallet_session::historical::SessionManager<T::AccountId, T::FullIdentification>
    for Pallet<T>
{
    fn new_session(new_index: SessionIndex) -> Option<Vec<(T::AccountId, T::FullIdentification)>> {
        <<T as Config>::SessionManager as pallet_session::historical::SessionManager<_, _>>::new_session(new_index)
    }

    fn end_session(end_index: SessionIndex) {
        let xor_to_val = XorToVal::<T>::take();
        if xor_to_val != 0 {
            if let Err(e) = Self::remint(xor_to_val) {
                error!("xor fee remint failed: {:?}", e);
            }
        }

        <<T as Config>::SessionManager as pallet_session::historical::SessionManager<_, _>>::end_session(end_index)
    }

    fn start_session(start_index: SessionIndex) {
        <<T as Config>::SessionManager as pallet_session::historical::SessionManager<_, _>>::start_session(start_index)
    }

    fn new_session_genesis(
        new_index: SessionIndex,
    ) -> Option<Vec<(T::AccountId, T::FullIdentification)>> {
        <<T as Config>::SessionManager as pallet_session::historical::SessionManager<_, _>>::new_session_genesis(new_index)
    }
}

/// Trait whose implementation allows to redefine extrinsics fees based
/// exclusively on the extrinsic's `Call` variant
pub trait ApplyCustomFees<Call> {
    /// If a value is returned, it overrides the fee amount calculated by the
    /// TransactionPayment pallet based on `DispatchInfo` and `WeightToFee` conversion
    //  `None` as the output indicated the extrinsic is not subject to a manual fee
    /// adjustment so the original value from TransactionPayment pallet will be charged
    fn compute_fee(call: &Call) -> Option<Balance>;
}

impl<Call> ApplyCustomFees<Call> for () {
    fn compute_fee(_call: &Call) -> Option<Balance> {
        None
    }
}

pub struct SwapInfo<AccountId, DexId, AssetId, Amount> {
    pub fee_source: Option<AccountId>,
    pub dex_id: DexId,
    pub input_asset_id: AssetId,
    pub output_asset_id: AssetId,
    pub amount: Amount,
    pub selected_source_types: Vec<LiquiditySourceType>,
    pub filter_mode: FilterMode,
}

/// A trait for extracting call information out of liquidity_proxy.swap calls
pub trait ExtractProxySwap {
    type AccountId;
    type DexId;
    type AssetId;
    type Amount;
    fn extract(
        &self,
    ) -> Option<SwapInfo<Self::AccountId, Self::DexId, Self::AssetId, Self::Amount>>;
}

pub trait IsCalledByBridgePeer<AccountId> {
    fn is_called_by_bridge_peer(&self, who: &AccountId) -> bool;
}

/// A trait whose purpose is to extract the `Call` variant of an extrinsic
pub trait GetCall<Call> {
    fn get_call(&self) -> Call;
}

pub trait WithdrawFee<T: Config> {
    fn withdraw_fee(
        who: &T::AccountId,
        call: &CallOf<T>,
        fee: Balance,
    ) -> Result<(T::AccountId, Option<NegativeImbalanceOf<T>>), DispatchError>;
}

/// Implementation for unchecked extrinsic.
impl<Address, Call, Signature, Extra> GetCall<Call>
    for UncheckedExtrinsic<Address, Call, Signature, Extra>
where
    Call: Dispatchable + Clone,
    Extra: SignedExtension,
{
    fn get_call(&self) -> Call {
        self.function.clone()
    }
}

/// Implementation for checked extrinsic.
impl<Address, Call, Extra> GetCall<Call> for CheckedExtrinsic<Address, Call, Extra>
where
    Call: Dispatchable + Clone,
{
    fn get_call(&self) -> Call {
        self.function.clone()
    }
}

impl<T: Config> Pallet<T> {
    // Returns value if custom fee is applicable to an extrinsic and `None` otherwise
    pub fn query_info<Extrinsic: Clone + ExtrinsicT + GetDispatchInfo + GetCall<CallOf<T>>>(
        unchecked_extrinsic: &Extrinsic,
        _len: u32,
    ) -> Option<RuntimeDispatchInfo<BalanceOf<T>>>
    where
        <T as frame_system::Config>::RuntimeCall: Dispatchable<Info = DispatchInfo>,
    {
        let dispatch_info = <Extrinsic as GetDispatchInfo>::get_dispatch_info(unchecked_extrinsic);
        let DispatchInfo {
            weight,
            class,
            pays_fee,
        } = dispatch_info;

        if pays_fee == Pays::No {
            return None;
        }

        let call = <Extrinsic as GetCall<CallOf<T>>>::get_call(&unchecked_extrinsic);

        let maybe_custom_fee = T::CustomFees::compute_fee(&call);
        let res = match maybe_custom_fee {
            Some(value) => Some(RuntimeDispatchInfo {
                weight,
                class,
                partial_fee: BalanceOf::<T>::saturated_from(value),
            }),
            _ => None,
        };

        res
    }

    // Returns value if custom fee is applicable to an extrinsic and `None` otherwise
    pub fn query_fee_details<Extrinsic: ExtrinsicT + GetDispatchInfo + GetCall<CallOf<T>>>(
        unchecked_extrinsic: &Extrinsic,
        _len: u32,
    ) -> Option<FeeDetails<BalanceOf<T>>>
    where
        <T as frame_system::Config>::RuntimeCall: Dispatchable<Info = DispatchInfo>,
    {
        let call = <Extrinsic as GetCall<CallOf<T>>>::get_call(unchecked_extrinsic);
        let maybe_custom_fee = T::CustomFees::compute_fee(&call);
        let res = match maybe_custom_fee {
            Some(fee) => Some(FeeDetails {
                inclusion_fee: Some(InclusionFee {
                    base_fee: 0_u32.into(),
                    len_fee: 0_u32.into(),
                    adjusted_weight_fee: BalanceOf::<T>::saturated_from(fee),
                }),
                tip: 0_u32.into(),
            }),
            _ => None,
        };

        res
    }

    pub fn remint(xor_to_val: Balance) -> Result<(), DispatchError> {
        let tech_account_id = <T as Config>::GetTechnicalAccountId::get();
        let xor = T::XorId::get();
        let val = T::ValId::get();
        let xst = T::XstId::get();

        // Re-minting the `xor_to_val` tokens amount to `tech_account_id` of this pallet.
        // The tokens being re-minted had initially been withdrawn as a part of the fee.
        Assets::<T>::mint_to(&xor, &tech_account_id, &tech_account_id, xor_to_val)?;
        // Attempting to swap XOR with VAL on secondary market
        // If successful, VAL will be burned, otherwise burn newly minted XOR from the tech account
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
                let mut val_to_burn = Balance::from(swap_outcome.amount);
                T::OnValBurned::on_val_burned(val_to_burn.clone());

                let val_to_buy_back = T::BuyBackXSTPercent::get() * val_to_burn;
                let result = common::with_transaction(|| {
                    T::BuyBackHandler::buy_back_and_burn(
                        &tech_account_id,
                        &val,
                        &xst,
                        val_to_buy_back,
                    )
                });
                match result {
                    Ok(_) => {
                        val_to_burn -= val_to_buy_back;
                    }
                    Err(err) => {
                        error!("failed to exchange val to xst, burning VAL instead of buy back: {err:?}");
                    }
                }
                assets::Pallet::<T>::burn_from(
                    &val,
                    &tech_account_id,
                    &tech_account_id,
                    val_to_burn,
                )?;
            }
            Err(e) => {
                error!(
                    "failed to exchange xor to val, burning {} XOR, e: {:?}",
                    xor_to_val, e
                );
                Assets::<T>::burn_from(&xor, &tech_account_id, &tech_account_id, xor_to_val)?;
            }
        }

        Ok(())
    }
}

pub use pallet::*;

pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_support::traits::StorageVersion;
    use frame_system::pallet_prelude::*;

    // TODO: #395 use AssetInfoProvider instead of assets pallet
    #[pallet::config]
    pub trait Config:
        frame_system::Config
        + referrals::Config
        + assets::Config
        + eth_bridge::Config
        + common::Config
        + pallet_transaction_payment::Config
        + pallet_session::historical::Config
    {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// XOR - The native currency of this blockchain.
        type XorCurrency: Currency<Self::AccountId> + Send + Sync;
        type XorId: Get<Self::AssetId>;
        type ValId: Get<Self::AssetId>;
        type XstId: Get<Self::AssetId>;
        type ReferrerWeight: Get<u32>;
        type XorBurnedWeight: Get<u32>;
        type XorIntoValBurnedWeight: Get<u32>;
        type BuyBackXSTPercent: Get<Percent>;
        type DEXIdValue: Get<Self::DEXId>;
        type LiquidityProxy: LiquidityProxyTrait<Self::DEXId, Self::AccountId, Self::AssetId>;
        type OnValBurned: OnValBurned;
        type CustomFees: ApplyCustomFees<CallOf<Self>>;
        type GetTechnicalAccountId: Get<Self::AccountId>;
        type SessionManager: pallet_session::historical::SessionManager<
            Self::AccountId,
            <Self as pallet_session::historical::Config>::FullIdentification,
        >;
        type BuyBackHandler: BuyBackHandler<Self::AccountId, Self::AssetId>;
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
        type WithdrawFee: WithdrawFee<Self>;
    }

    /// The current storage version.
    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

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
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Fee has been withdrawn from user. [Account Id to withdraw from, Fee Amount]
        FeeWithdrawn(AccountIdOf<T>, BalanceOf<T>),
        /// The portion of fee is sent to the referrer. [Referral, Referrer, Amount]
        ReferrerRewarded(AccountIdOf<T>, AccountIdOf<T>, Balance),
        /// New multiplier for weight to fee conversion is set
        /// (*1_000_000_000_000_000_000). [New value]
        WeightToFeeMultiplierUpdated(FixedU128),
    }

    /// The amount of XOR to be reminted and exchanged for VAL at the end of the session
    #[pallet::storage]
    #[pallet::getter(fn asset_infos)]
    pub type XorToVal<T: Config> = StorageValue<_, Balance, ValueQuery>;

    #[pallet::type_value]
    pub fn DefaultForFeeMultiplier<T: Config>() -> FixedU128 {
        // We set 100 as it's the given required value
        FixedU128::from(100)
    }

    // Multiplier used in WeightToFee conversion
    #[pallet::storage]
    #[pallet::getter(fn multiplier)]
    pub type Multiplier<T> = StorageValue<_, FixedU128, ValueQuery, DefaultForFeeMultiplier<T>>;

    // This affects `base_fee` and `weight_fee`. `length_fee` is too small
    // in comparison to them, so we should be fine multiplying only this parts.
    impl<T: Config> WeightToFeePolynomial for Pallet<T> {
        type Balance = Balance;

        fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
            smallvec!(WeightToFeeCoefficient {
                // 7_000_000 was the original coefficient taken as reference
                coeff_integer: <Multiplier<T>>::get().saturating_mul_int(7_000_000),
                coeff_frac: Perbill::zero(),
                negative: false,
                degree: 1,
            })
        }
    }
}
