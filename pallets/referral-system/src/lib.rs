#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::ensure;
use frame_support::sp_runtime::DispatchError;

impl<T: Config> Module<T> {
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

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {}

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {}

    #[pallet::error]
    pub enum Error<T> {
        /// Account already has a referrer.
        AlreadyHasReferrer,
    }

    #[pallet::storage]
    #[pallet::getter(fn referrer_account)]
    pub type Referrers<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, T::AccountId>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub referrers: Vec<(T::AccountId, T::AccountId)>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                referrers: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            self.referrers.iter().for_each(|(k, v)| {
                Referrers::<T>::insert(k, v);
            });
        }
    }
}
