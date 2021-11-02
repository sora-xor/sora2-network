use frame_support::dispatch::DispatchResult;
use frame_support::traits::{EnsureOrigin, Get};
use frame_system::ensure_signed;
use snowbridge_core::{ChannelId, Message, MessageDispatch, MessageId, Verifier};
use sp_core::{H160, U256};
use sp_std::convert::TryFrom;

use envelope::Envelope;

use sp_runtime::traits::{Convert, Zero};
use sp_runtime::Perbill;
use traits::MultiCurrency;

mod benchmarking;

pub mod weights;
pub use weights::WeightInfo;

#[cfg(test)]
mod test;

mod envelope;

type BalanceOf<T> = <<T as assets::Config>::Currency as MultiCurrency<
    <T as frame_system::Config>::AccountId,
>>::Balance;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::log::{debug, warn};
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use snowbridge_ethereum::EthNetworkId;

    #[pallet::config]
    pub trait Config: frame_system::Config + assets::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        /// Verifier module for message verification.
        type Verifier: Verifier;

        /// Verifier module for message verification.
        type MessageDispatch: MessageDispatch<Self, MessageId>;

        type FeeConverter: Convert<U256, BalanceOf<Self>>;

        /// The base asset as the core asset in all trading pairs
        type FeeAssetId: Get<Self::AssetId>;

        /// The origin which may update reward related params
        type UpdateOrigin: EnsureOrigin<Self::Origin>;

        /// Weight information for extrinsics in this pallet
        type WeightInfo: WeightInfo;
    }

    /// Source channel on the ethereum side
    #[pallet::storage]
    #[pallet::getter(fn source_channel)]
    pub type ChannelOwners<T: Config> =
        StorageDoubleMap<_, Identity, EthNetworkId, Identity, H160, T::AccountId, OptionQuery>;

    #[pallet::storage]
    pub type ChannelNonces<T: Config> =
        StorageDoubleMap<_, Identity, EthNetworkId, Identity, H160, u64, ValueQuery>;

    /// Source of funds to pay relayers
    #[pallet::storage]
    #[pallet::getter(fn source_account)]
    pub type SourceAccounts<T: Config> =
        StorageDoubleMap<_, Identity, EthNetworkId, Identity, H160, T::AccountId, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn reward_fraction)]
    pub(super) type RewardFraction<T: Config> = StorageValue<_, Perbill, ValueQuery>;

    /// Treasury Account
    #[pallet::storage]
    #[pallet::getter(fn treasury_account)]
    pub(super) type TreasuryAccount<T: Config> = StorageValue<_, T::AccountId, ValueQuery>;

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::event]
    #[pallet::metadata(AccountIdOf<T> = "AccountId")]
    //#[pallet::generate_deposit(pub(super) fn deposit_event)]
    /// This module has no events
    pub enum Event<T: Config> {}

    #[pallet::error]
    pub enum Error<T> {
        /// Message came from an invalid outbound channel on the Ethereum side.
        InvalidSourceChannel,
        /// Message has an invalid envelope.
        InvalidEnvelope,
        /// Message has an unexpected nonce.
        InvalidNonce,
        /// Incorrect reward fraction
        InvalidRewardFraction,
        /// This contract already exists
        ContractExists,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(<T as Config>::WeightInfo::submit())]
        pub fn submit(
            origin: OriginFor<T>,
            network_id: EthNetworkId,
            message: Message,
        ) -> DispatchResultWithPostInfo {
            let relayer = ensure_signed(origin)?;
            debug!("Recieved message from {:?}", relayer);
            // submit message to verifier for verification
            let log = T::Verifier::verify(network_id, &message)?;

            // Decode log into an Envelope
            let envelope: Envelope<T> =
                Envelope::try_from(log).map_err(|_| Error::<T>::InvalidEnvelope)?;

            ensure!(
                <ChannelOwners<T>>::contains_key(network_id, envelope.channel),
                Error::<T>::InvalidSourceChannel
            );

            // Verify message nonce
            <ChannelNonces<T>>::try_mutate(
                network_id,
                envelope.channel,
                |nonce| -> DispatchResult {
                    if envelope.nonce != *nonce + 1 {
                        Err(Error::<T>::InvalidNonce.into())
                    } else {
                        *nonce += 1;
                        Ok(())
                    }
                },
            )?;

            let source_account = <SourceAccounts<T>>::get(network_id, envelope.channel);
            Self::handle_fee(envelope.fee, &relayer, &source_account);

            let message_id = MessageId::new(ChannelId::Basic, envelope.nonce);
            T::MessageDispatch::dispatch(
                network_id,
                envelope.source,
                message_id,
                &envelope.payload,
            );

            Ok(().into())
        }

        #[pallet::weight(<T as Config>::WeightInfo::register_channel())]
        pub fn register_channel(
            origin: OriginFor<T>,
            network_id: EthNetworkId,
            channel: H160,
            fee_source: T::AccountId,
        ) -> DispatchResult {
            let owner = ensure_signed(origin)?;
            ensure!(
                <ChannelOwners<T>>::contains_key(network_id, channel) == false,
                Error::<T>::ContractExists
            );

            <ChannelOwners<T>>::insert(network_id, channel, owner);
            <SourceAccounts<T>>::insert(network_id, channel, fee_source);
            Ok(())
        }

        #[pallet::weight(<T as Config>::WeightInfo::set_reward_fraction())]
        pub fn set_reward_fraction(
            origin: OriginFor<T>,
            fraction: Perbill,
        ) -> DispatchResultWithPostInfo {
            T::UpdateOrigin::ensure_origin(origin)?;
            RewardFraction::<T>::set(fraction);
            Ok(().into())
        }
    }

    impl<T: Config> Pallet<T> {
        /*
        	* Pay the message submission fee into the relayer and treasury account.
        	*
        	* - If the fee is zero, do nothing
        	* - Otherwise, withdraw the fee amount from the DotApp module account, returning a negative imbalance
        	* - Figure out the fraction of the fee amount that should be paid to the relayer
        	* - Pay the relayer if their account exists, returning a positive imbalance.
        	* - Adjust the negative imbalance by offsetting the amount paid to the relayer
        	* - Resolve the negative imbalance by depositing it into the treasury account
        	*/
        pub fn handle_fee(
            amount: BalanceOf<T>,
            relayer: &T::AccountId,
            source_account: &T::AccountId,
        ) {
            if amount.is_zero() {
                return;
            }
            let reward_fraction: Perbill = RewardFraction::<T>::get();
            let reward_amount = reward_fraction.mul_ceil(amount);

            if let Err(err) =
                T::Currency::transfer(T::FeeAssetId::get(), source_account, relayer, reward_amount)
            {
                warn!("Unable to transfer reward to relayer: {:?}", err);
                return;
            }

            if let Some(treasure_amount) = amount.checked_sub(reward_amount) {
                if let Err(err) = T::Currency::transfer(
                    T::FeeAssetId::get(),
                    source_account,
                    &TreasuryAccount::<T>::get(),
                    treasure_amount,
                ) {
                    warn!("Unable to transfer reward to relayer: {:?}", err);
                }
            }
        }
    }

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub networks: Vec<(EthNetworkId, Vec<(H160, T::AccountId, T::AccountId)>)>,
        pub reward_fraction: Perbill,
        pub treasury_account: T::AccountId,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                networks: Default::default(),
                reward_fraction: Default::default(),
                treasury_account: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            for (network_id, channels) in &self.networks {
                for (channel, owner, source) in channels {
                    <ChannelOwners<T>>::insert(network_id, channel, owner);
                    <SourceAccounts<T>>::insert(network_id, channel, source);
                }
            }
            RewardFraction::<T>::set(self.reward_fraction);
            TreasuryAccount::<T>::set(self.treasury_account.clone());
        }
    }
}
