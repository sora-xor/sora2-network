#![cfg_attr(not(feature = "std"), no_std)]

use common::prelude::SwapAmount;
use common::{Balance, FromGenericPair, LiquiditySourceFilter, LiquiditySourceType};
use frame_support::pallet_prelude::InvalidTransaction;
use frame_support::traits::{Currency, ExistenceRequirement, Get, Imbalance, WithdrawReasons};
use frame_support::unsigned::TransactionValidityError;
use liquidity_proxy::LiquidityProxyTrait;
use pallet_staking::ValBurnedNotifier;
use pallet_transaction_payment::OnChargeTransaction;
use sp_runtime::traits::{
    DispatchInfoOf, PostDispatchInfoOf, Saturating, UniqueSaturatedInto, Zero,
};

pub const TECH_ACCOUNT_PREFIX: &[u8] = b"xor-fee";
pub const TECH_ACCOUNT_MAIN: &[u8] = b"main";

type NegativeImbalanceOf<T> = <<T as Config>::XorCurrency as Currency<
    <T as frame_system::Config>::AccountId,
>>::NegativeImbalance;

type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

type BalanceOf<T> =
    <<T as Config>::XorCurrency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
type Technical<T> = technical::Pallet<T>;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

impl<T: Config> OnChargeTransaction<T> for Pallet<T> {
    type Balance = BalanceOf<T>;
    type LiquidityInfo = Option<NegativeImbalanceOf<T>>;

    fn withdraw_fee(
        who: &T::AccountId,
        _call: &T::Call,
        _dispatch_info: &DispatchInfoOf<T::Call>,
        fee: Self::Balance,
        tip: Self::Balance,
    ) -> Result<Self::LiquidityInfo, TransactionValidityError> {
        if fee.is_zero() {
            return Ok(None);
        }

        let withdraw_reason = if tip.is_zero() {
            WithdrawReasons::TRANSACTION_PAYMENT
        } else {
            WithdrawReasons::TRANSACTION_PAYMENT | WithdrawReasons::TIP
        };

        match T::XorCurrency::withdraw(who, fee, withdraw_reason, ExistenceRequirement::KeepAlive) {
            Ok(imbalance) => Ok(Some(imbalance)),
            Err(_) => Err(InvalidTransaction::Payment.into()),
        }
    }

    fn correct_and_deposit_fee(
        who: &T::AccountId,
        _dispatch_info: &DispatchInfoOf<T::Call>,
        _post_info: &PostDispatchInfoOf<T::Call>,
        corrected_fee: Self::Balance,
        _tip: Self::Balance,
        already_withdrawn: Self::LiquidityInfo,
    ) -> Result<(), TransactionValidityError> {
        if let Some(paid) = already_withdrawn {
            // Calculate how much refund we should return
            let refund_amount = paid.peek().saturating_sub(corrected_fee);
            // refund to the the account that paid the fees. If this fails, the
            // account might have dropped below the existential balance. In
            // that case we don't refund anything.
            let refund_imbalance = T::XorCurrency::deposit_into_existing(&who, refund_amount)
                .unwrap_or_else(|_| {
                    <T::XorCurrency as Currency<T::AccountId>>::PositiveImbalance::zero()
                });
            // merge the imbalance caused by paying the fees and refunding parts of it again.
            let adjusted_paid = paid
                .offset(refund_imbalance)
                .map_err(|_| TransactionValidityError::Invalid(InvalidTransaction::Payment))?;
            Self::deposit_event(Event::FeeWithdrawn(who.clone(), corrected_fee));
            let xor_burned_weight = T::XorBurnedWeight::get();
            let xor_into_val_burned_weight = T::XorIntoValBurnedWeight::get();
            let (referrer_xor, adjusted_paid) = adjusted_paid.ration(
                T::ReferrerWeight::get(),
                xor_burned_weight + xor_into_val_burned_weight,
            );
            if let Some(referrer) = referral_system::Pallet::<T>::referrer_account(who) {
                let _result = T::XorCurrency::resolve_into_existing(&referrer, referrer_xor);
            }
            // TODO: decide what should be done with XOR if there is no referrer.
            // Burn XOR for now
            let (_xor_burned, xor_to_val) =
                adjusted_paid.ration(xor_burned_weight, xor_into_val_burned_weight);
            // Convert `NegativeImbalanceOf<T>` value to a `Balance` type.
            // This method is saturation-free as long as `xor_to_val` doesn't exceed 13.106511852580896768
            // which is the case for fees (that are usually of the order of ~10^-4)
            let xor_to_val = xor_to_val.peek().unique_saturated_into();
            let tech_account_id = T::TechAccountId::from_generic_pair(
                TECH_ACCOUNT_PREFIX.to_vec(),
                TECH_ACCOUNT_MAIN.to_vec(),
            );
            // Trying to mint the `xor_to_val` tokens amount to `tech_account_id` of this pallet. Tokens were initially withdrawn as part of the fee.
            if Technical::<T>::mint(&T::XorId::get(), &tech_account_id, xor_to_val).is_ok() {
                let account_id = Technical::<T>::tech_account_id_to_account_id(&tech_account_id)
                    .expect("Failed to get ordinary account id for technical account id.");
                // Trying to swap XOR with VAL.
                // If swap goes through, VAL will be burned (for more in-depth look read VAL tokenomics), otherwise remove XOR from the tech account.
                match T::LiquidityProxy::exchange(
                    &account_id,
                    &account_id,
                    &T::XorId::get(),
                    &T::ValId::get(),
                    SwapAmount::WithDesiredInput {
                        desired_amount_in: xor_to_val,
                        min_amount_out: 0,
                    },
                    LiquiditySourceFilter::with_allowed(
                        T::DEXIdValue::get(),
                        [LiquiditySourceType::XYKPool].into(),
                    ),
                ) {
                    Ok(swap_outcome) => {
                        let val_to_burn = Balance::from(swap_outcome.amount);
                        if Technical::<T>::burn(
                            &T::ValId::get(),
                            &tech_account_id,
                            val_to_burn.clone(),
                        )
                        .is_ok()
                        {
                            T::ValBurnedNotifier::notify_val_burned(val_to_burn);
                        };
                    }
                    Err(_) => {
                        let _ =
                            Technical::<T>::burn(&T::XorId::get(), &tech_account_id, xor_to_val);
                    }
                }
            }
        }
        Ok(())
    }
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::config]
    pub trait Config:
        frame_system::Config
        + referral_system::Config
        + assets::Config
        + common::Config
        + technical::Config
        + pallet_transaction_payment::Config
    {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        /// XOR - The native currency of this blockchain.
        type XorCurrency: Currency<Self::AccountId> + Send + Sync;
        type XorId: Get<Self::AssetId>;
        type ValId: Get<Self::AssetId>;
        type ReferrerWeight: Get<u32>;
        type XorBurnedWeight: Get<u32>;
        type XorIntoValBurnedWeight: Get<u32>;
        type DEXIdValue: Get<Self::DEXId>;
        type LiquidityProxy: LiquidityProxyTrait<Self::DEXId, Self::AccountId, Self::AssetId>;
        type ValBurnedNotifier: ValBurnedNotifier<Balance>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {}

    #[pallet::event]
    #[pallet::metadata(AccountIdOf<T> = "AccountId", BalanceOf<T> = "Balance")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Fee has been withdrawn from user. [Account Id to withdraw from, Fee Amount]
        FeeWithdrawn(AccountIdOf<T>, BalanceOf<T>),
    }
}
