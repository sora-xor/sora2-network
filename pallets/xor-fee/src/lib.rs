#![cfg_attr(not(feature = "std"), no_std)]

use common::{prelude::*, Fixed};
use frame_support::{
    decl_error, decl_event, decl_module, decl_storage,
    traits::{Currency, Get, Imbalance},
};
use pallet_transaction_payment::OnTransactionPayment;
use sp_arithmetic::traits::UniqueSaturatedInto;
use sp_runtime::DispatchError;

pub const TECH_ACCOUNT_PREFIX: &[u8] = b"xor-fee";
pub const TECH_ACCOUNT_MAIN: &[u8] = b"main";

type NegativeImbalanceOf<T> = <<T as Trait>::XorCurrency as Currency<
    <T as frame_system::Trait>::AccountId,
>>::NegativeImbalance;

type BalanceOf<T> =
    <<T as Trait>::XorCurrency as Currency<<T as frame_system::Trait>::AccountId>>::Balance;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub trait Trait:
    frame_system::Trait + referral_system::Trait + assets::Trait + common::Trait + technical::Trait
{
    type Event: From<Event> + Into<<Self as frame_system::Trait>::Event>;

    /// XOR - The native currency of this blockchain.
    type XorCurrency: Currency<Self::AccountId> + Send + Sync;

    type XorId: Get<Self::AssetId>;

    type ValId: Get<Self::AssetId>;

    type ReferrerWeight: Get<u32>;

    type XorBurnedWeight: Get<u32>;

    type XorIntoValBurnedWeight: Get<u32>;

    type DEXIdValue: Get<Self::DEXId>;

    type LiquiditySource: common::LiquiditySource<
        Self::DEXId,
        Self::AccountId,
        Self::AssetId,
        common::Fixed,
        DispatchError,
    >;
}

decl_storage! {
    trait Store for Module<T: Trait> as XorFee {}
}

decl_event!(
    pub enum Event {}
);

decl_error! {
    pub enum Error for Module<T: Trait> {}
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        type Error = Error<T>;

        fn deposit_event() = default;
    }
}

impl<T: Trait> Module<T> {}

impl<T: Trait> OnTransactionPayment<T::AccountId, NegativeImbalanceOf<T>, BalanceOf<T>>
    for Module<T>
{
    fn on_payment(
        from_account: T::AccountId,
        fee: NegativeImbalanceOf<T>,
        tip: NegativeImbalanceOf<T>,
    ) {
        let amount = fee.merge(tip);
        let (referrer_xor, amount) = amount.ration(
            T::ReferrerWeight::get(),
            T::ReferrerWeight::get() + T::XorBurnedWeight::get() + T::XorIntoValBurnedWeight::get(),
        );
        let referrer = referral_system::Module::<T>::referrer_account(from_account);
        if referrer != T::AccountId::default() {
            let _result = T::XorCurrency::resolve_into_existing(&referrer, referrer_xor);
        }
        // TODO: decide what should be done with XOR if there is no referrer.
        // Burn XOR for now
        let (_xor_burned, xor_to_val) = amount.ration(
            T::XorBurnedWeight::get(),
            T::XorBurnedWeight::get() + T::XorIntoValBurnedWeight::get(),
        );
        let xor_to_val: u128 = xor_to_val.peek().unique_saturated_into();
        let xor_to_val: Fixed = xor_to_val.into();
        let tech_account_id = T::TechAccountId::from_generic_pair(
            TECH_ACCOUNT_PREFIX.to_vec(),
            TECH_ACCOUNT_MAIN.to_vec(),
        );
        // Trying to mint the `xor_to_val` tokens amount to `tech_account_id` of this pallet. Tokens were initially withdrawn as part of the fee.
        if technical::Module::<T>::mint(&T::XorId::get(), &tech_account_id, xor_to_val.into())
            .is_ok()
        {
            let account_id =
                technical::Module::<T>::tech_account_id_to_account_id(&tech_account_id)
                    .expect("Failed to get ordinary account id for technical account id.");
            // Trying to exchange XOR to VAL.
            // If exchange happens VAL will be burned (for more in depth look read VAL tokenomics), otherwise remove XOR from the tech account.
            if let Ok(swap_outcome) = T::LiquiditySource::exchange(
                &account_id,
                &account_id,
                &T::DEXIdValue::get(),
                &T::XorId::get(),
                &T::ValId::get(),
                SwapAmount::WithDesiredInput {
                    desired_amount_in: xor_to_val,
                    min_amount_out: 0.into(),
                },
            ) {
                let _result = technical::Module::<T>::burn(
                    &T::ValId::get(),
                    &tech_account_id,
                    swap_outcome.amount.into(),
                );
            //TODO: Notify staking pallet about VAL burned
            } else {
                let _result = technical::Module::<T>::burn(
                    &T::XorId::get(),
                    &tech_account_id,
                    xor_to_val.into(),
                );
            }
        }
    }
}
