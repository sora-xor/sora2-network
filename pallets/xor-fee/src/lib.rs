#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
    decl_error, decl_event, decl_module, decl_storage,
    traits::{Currency, Get, Imbalance},
};
use pallet_staking::ValBurnedNotifier;
use pallet_transaction_payment::OnTransactionPayment;
use traits::{MultiCurrency, MultiCurrencyExtended, MultiLockableCurrency};

type NegativeImbalanceOf<T> = <<T as Trait>::XorCurrency as Currency<
    <T as frame_system::Trait>::AccountId,
>>::NegativeImbalance;

type BalanceOf<T> =
    <<T as Trait>::XorCurrency as Currency<<T as frame_system::Trait>::AccountId>>::Balance;

/// The balance type for MultiCurrency.
pub type MultiCurrencyBalanceOf<T> =
    <<T as Trait>::MultiCurrency as MultiCurrency<<T as frame_system::Trait>::AccountId>>::Balance;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub trait Trait: frame_system::Trait + referral_system::Trait {
    type Event: From<Event> + Into<<Self as frame_system::Trait>::Event>;

    /// XOR - The native currency of this blockchain.
    type XorCurrency: Currency<Self::AccountId> + Send + Sync;

    type MultiCurrency: MultiCurrencyExtended<Self::AccountId>
        + MultiLockableCurrency<Self::AccountId, Moment = Self::BlockNumber>;

    //type ValToXorId: Get<u32>;

    type ReferrerWeight: Get<u32>;

    type XorBurnedWeight: Get<u32>;

    type XorIntoValBurnedWeight: Get<u32>;

    type ValBurnedNotifier: ValBurnedNotifier<MultiCurrencyBalanceOf<Self>>;
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
        let (_xor_burned, _xor_to_val) = amount.ration(
            T::XorBurnedWeight::get(),
            T::XorBurnedWeight::get() + T::XorIntoValBurnedWeight::get(),
        );
        // TODO: buy back `VAL` through `DexManager`, when the interface is ready
        // let _val_burned = DexManager::buy(xor_to_val.peek(), ValToXorId::get());
        // For now placeholder is used.
        let val_burned = 1;
        T::ValBurnedNotifier::notify_val_burned(val_burned.into());
    }
}
