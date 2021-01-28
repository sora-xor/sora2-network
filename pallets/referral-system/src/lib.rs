#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
    decl_error, decl_event, decl_module, decl_storage, ensure, sp_runtime::DispatchError,
};

pub trait Trait: frame_system::Trait {
    type Event: From<Event> + Into<<Self as frame_system::Trait>::Event>;
}

decl_storage! {
    trait Store for Module<T: Trait> as ReferralSystem {
        // Referrer's account by the account of the user who was referred.
        pub Referrers get(fn referrer_account) config(referrers): map hasher(blake2_128_concat) T::AccountId => Option<T::AccountId>;
    }
}

decl_event!(
    pub enum Event {}
);

decl_error! {
    pub enum Error for Module<T: Trait> {
        /// Account already has a referrer.
        AlreadyHasReferrer
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        type Error = Error<T>;

        fn deposit_event() = default;
    }
}

impl<T: Trait> Module<T> {
    pub fn set_referrer_to(
        referral: &T::AccountId,
        referrer: T::AccountId,
    ) -> Result<(), DispatchError> {
        Referrers::<T>::mutate(&referral, |r| {
            ensure!(r.is_none(), Error::<T>::AlreadyHasReferrer);
            *r = Some(referrer);
            Ok(())
        })
    }
}
