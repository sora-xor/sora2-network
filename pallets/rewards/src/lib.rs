//! This pallet enables users to claim their rewards.
//!
//! There are following kinds of rewards:
//! * VAL for XOR owners
//! * PSWAP farming
//! * PSWAP NFT waifus

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::dispatch::{DispatchErrorWithPostInfo, Weight};
use frame_support::storage::StorageMap as StorageMapTrait;
use sp_core::H160;
use sp_std::prelude::*;

use assets::AssetIdOf;
use common::{eth, AccountIdOf, Balance};

pub use self::pallet::*;

mod weights;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

type EthereumAddress = H160;
type WeightInfoOf<T> = <T as Config>::WeightInfo;

pub const TECH_ACCOUNT_PREFIX: &[u8] = b"rewards";
pub const TECH_ACCOUNT_MAIN: &[u8] = b"main";

pub trait WeightInfo {
    fn claim() -> Weight;
}

impl<T: Config> Pallet<T> {
    pub fn claimables(eth_address: &EthereumAddress) -> Vec<Balance> {
        vec![
            ValOwners::<T>::get(eth_address),
            PswapFarmOwners::<T>::get(eth_address),
            PswapWaifuOwners::<T>::get(eth_address),
        ]
    }

    fn claim_reward<M: StorageMapTrait<EthereumAddress, Balance>>(
        eth_address: &EthereumAddress,
        account_id: &AccountIdOf<T>,
        asset_id: &AssetIdOf<T>,
        reserves_acc: &T::TechAccountId,
        claimed: &mut bool,
        already_claimed: &mut bool,
    ) -> Result<(), DispatchErrorWithPostInfo> {
        if let Ok(balance) = M::try_get(eth_address) {
            if balance > 0 {
                technical::Module::<T>::transfer_out(asset_id, reserves_acc, account_id, balance)?;
                M::insert(eth_address, 0);
                *claimed = true;
            } else {
                *already_claimed = true;
            }
        }
        Ok(())
    }
}

#[frame_support::pallet]
pub mod pallet {
    use frame_support::pallet_prelude::*;
    use frame_support::transactional;
    use frame_system::pallet_prelude::*;
    use secp256k1::util::SIGNATURE_SIZE;
    use secp256k1::{RecoveryId, Signature};
    use sp_std::vec::Vec;

    use common::{PSWAP, VAL};

    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + assets::Config + technical::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type WeightInfo: WeightInfo;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(WeightInfoOf::<T>::claim())]
        #[transactional]
        pub fn claim(origin: OriginFor<T>, signature: Vec<u8>) -> DispatchResultWithPostInfo {
            let account_id = ensure_signed(origin)?;
            ensure!(
                signature.len() == SIGNATURE_SIZE + 1,
                Error::<T>::SignatureInvalid
            );
            let recovery_id = RecoveryId::parse(signature[SIGNATURE_SIZE] - 27)
                .map_err(|_| Error::<T>::SignatureVerificationFailed)?;
            let signature = Signature::parse_slice(&signature[..SIGNATURE_SIZE])
                .map_err(|_| Error::<T>::SignatureInvalid)?;
            let message = eth::prepare_message(&account_id.encode());
            let public_key = secp256k1::recover(&message, &signature, &recovery_id)
                .map_err(|_| Error::<T>::SignatureVerificationFailed)?;
            let eth_address = eth::public_key_to_eth_address(&public_key);
            let reserves_acc = ReservesAcc::<T>::get();
            let mut claimed = false;
            let mut already_claimed = false;
            Self::claim_reward::<ValOwners<T>>(
                &eth_address,
                &account_id,
                &VAL.into(),
                &reserves_acc,
                &mut claimed,
                &mut already_claimed,
            )?;
            Self::claim_reward::<PswapFarmOwners<T>>(
                &eth_address,
                &account_id,
                &PSWAP.into(),
                &reserves_acc,
                &mut claimed,
                &mut already_claimed,
            )?;
            Self::claim_reward::<PswapWaifuOwners<T>>(
                &eth_address,
                &account_id,
                &PSWAP.into(),
                &reserves_acc,
                &mut claimed,
                &mut already_claimed,
            )?;
            if claimed {
                Self::deposit_event(Event::<T>::Claimed(account_id));
                Ok(().into())
            } else if already_claimed {
                Err(Error::<T>::AlreadyClaimed.into())
            } else {
                Err(Error::<T>::NoRewards.into())
            }
        }
    }

    #[pallet::event]
    #[pallet::metadata(AccountIdOf<T> = "AccountId")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// The account has claimed their rewards. [account]
        Claimed(AccountIdOf<T>),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The account has no rewards
        NoRewards,
        /// The account has already claimed their rewards
        AlreadyClaimed,
        /// The signature is invalid
        SignatureInvalid,
        /// The signature verification failed
        SignatureVerificationFailed,
    }

    #[pallet::storage]
    pub(super) type ReservesAcc<T: Config> = StorageValue<_, T::TechAccountId, ValueQuery>;

    #[pallet::storage]
    pub(super) type ValOwners<T: Config> =
        StorageMap<_, Identity, EthereumAddress, Balance, ValueQuery>;

    #[pallet::storage]
    pub(super) type PswapFarmOwners<T: Config> =
        StorageMap<_, Identity, EthereumAddress, Balance, ValueQuery>;

    #[pallet::storage]
    pub(super) type PswapWaifuOwners<T: Config> =
        StorageMap<_, Identity, EthereumAddress, Balance, ValueQuery>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub reserves_account_id: T::TechAccountId,
        pub val_owners: Vec<(EthereumAddress, Balance)>,
        pub pswap_farm_owners: Vec<(EthereumAddress, Balance)>,
        pub pswap_waifu_owners: Vec<(EthereumAddress, Balance)>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                reserves_account_id: Default::default(),
                val_owners: Default::default(),
                pswap_farm_owners: Default::default(),
                pswap_waifu_owners: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            ReservesAcc::<T>::put(&self.reserves_account_id);
            self.val_owners.iter().for_each(|(owner, balance)| {
                ValOwners::<T>::insert(owner, balance);
            });
            self.pswap_farm_owners.iter().for_each(|(owner, balance)| {
                PswapFarmOwners::<T>::insert(owner, balance);
            });
            self.pswap_waifu_owners.iter().for_each(|(owner, balance)| {
                PswapWaifuOwners::<T>::insert(owner, balance);
            });
        }
    }
}
