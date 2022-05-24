use bridge_types::traits::{MessageDispatch, Verifier};
use bridge_types::types::{ChannelId, Message, MessageId};
use bridge_types::EthNetworkId;
use frame_support::dispatch::DispatchResult;
use frame_support::traits::Get;
use frame_system::ensure_signed;
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
    use bridge_types::traits::{AppRegistry, OutboundRouter};
    use frame_support::log::{debug, warn};
    use frame_support::pallet_prelude::*;
    use frame_support::traits::StorageVersion;
    use frame_system::pallet_prelude::*;
    use frame_system::RawOrigin;

    #[pallet::config]
    pub trait Config: frame_system::Config + assets::Config + technical::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        /// Verifier module for message verification.
        type Verifier: Verifier;

        /// Verifier module for message verification.
        type MessageDispatch: MessageDispatch<Self, MessageId>;

        type FeeConverter: Convert<U256, BalanceOf<Self>>;

        /// The base asset as the core asset in all trading pairs
        type FeeAssetId: Get<Self::AssetId>;

        type FeeTechAccountId: Get<Self::TechAccountId>;

        type TreasuryTechAccountId: Get<Self::TechAccountId>;

        type OutboundRouter: OutboundRouter<Self::AccountId>;

        /// Weight information for extrinsics in this pallet
        type WeightInfo: WeightInfo;
    }

    /// Source channel on the ethereum side
    #[pallet::storage]
    #[pallet::getter(fn source_channel)]
    pub type ChannelAddresses<T: Config> = StorageMap<_, Identity, EthNetworkId, H160, OptionQuery>;

    #[pallet::storage]
    pub type ChannelNonces<T: Config> = StorageMap<_, Identity, EthNetworkId, u64, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn reward_fraction)]
    pub(super) type RewardFraction<T: Config> =
        StorageValue<_, Perbill, ValueQuery, DefaultRewardFraction>;

    #[pallet::type_value]
    pub(super) fn DefaultRewardFraction() -> Perbill {
        Perbill::from_percent(80)
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

    #[pallet::event]
    //#[pallet::generate_deposit(pub(super) fn deposit_event)]
    /// This module has no events
    pub enum Event<T: Config> {}

    #[pallet::error]
    pub enum Error<T> {
        /// Message came from an invalid network.
        InvalidNetwork,
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
        /// Call encoding failed.
        CallEncodeFailed,
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
                <ChannelAddresses<T>>::get(network_id).ok_or(Error::<T>::InvalidNetwork)?
                    == envelope.channel,
                Error::<T>::InvalidSourceChannel
            );

            // Verify message nonce
            <ChannelNonces<T>>::try_mutate(network_id, |nonce| -> DispatchResult {
                if envelope.nonce != *nonce + 1 {
                    Err(Error::<T>::InvalidNonce.into())
                } else {
                    *nonce += 1;
                    Ok(())
                }
            })?;

            Self::handle_fee(envelope.fee, &relayer);

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
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            Self::register_channel_inner(network_id, channel)?;
            Ok(().into())
        }

        #[pallet::weight(<T as Config>::WeightInfo::set_reward_fraction())]
        pub fn set_reward_fraction(
            origin: OriginFor<T>,
            fraction: Perbill,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            RewardFraction::<T>::set(fraction);
            Ok(().into())
        }
    }

    impl<T: Config> AppRegistry for Pallet<T> {
        fn register_app(network_id: EthNetworkId, app: H160) -> DispatchResult {
            let target =
                ChannelAddresses::<T>::get(network_id).ok_or(Error::<T>::InvalidNetwork)?;

            let message = bridge_types::channel_abi::RegisterOperatorPayload { operator: app };

            T::OutboundRouter::submit(
                network_id,
                ChannelId::Basic,
                &RawOrigin::Root,
                target,
                message
                    .encode()
                    .map_err(|_| Error::<T>::CallEncodeFailed)?
                    .as_ref(),
            )?;
            Ok(())
        }

        fn deregister_app(network_id: EthNetworkId, app: H160) -> DispatchResult {
            let target =
                ChannelAddresses::<T>::get(network_id).ok_or(Error::<T>::InvalidNetwork)?;

            let message = bridge_types::channel_abi::DeregisterOperatorPayload { operator: app };

            T::OutboundRouter::submit(
                network_id,
                ChannelId::Basic,
                &RawOrigin::Root,
                target,
                message
                    .encode()
                    .map_err(|_| Error::<T>::CallEncodeFailed)?
                    .as_ref(),
            )?;
            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        pub fn register_channel_inner(network_id: EthNetworkId, channel: H160) -> DispatchResult {
            ensure!(
                <ChannelAddresses<T>>::contains_key(network_id) == false,
                Error::<T>::ContractExists
            );
            <ChannelAddresses<T>>::insert(network_id, channel);
            Ok(())
        }

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
        pub fn handle_fee(amount: BalanceOf<T>, relayer: &T::AccountId) {
            if amount.is_zero() {
                return;
            }
            let reward_fraction: Perbill = RewardFraction::<T>::get();
            let reward_amount = reward_fraction.mul_ceil(amount);

            if let Err(err) = technical::Pallet::<T>::transfer_out(
                &T::FeeAssetId::get(),
                &T::FeeTechAccountId::get(),
                relayer,
                reward_amount,
            ) {
                warn!("Unable to transfer reward to relayer: {:?}", err);
                return;
            }

            if let Some(treasure_amount) = amount.checked_sub(reward_amount) {
                if let Err(err) = technical::Pallet::<T>::transfer(
                    &T::FeeAssetId::get(),
                    &T::FeeTechAccountId::get(),
                    &T::TreasuryTechAccountId::get(),
                    treasure_amount,
                ) {
                    warn!("Unable to transfer reward to relayer: {:?}", err);
                }
            }
        }
    }

    #[pallet::genesis_config]
    pub struct GenesisConfig {
        pub networks: Vec<(EthNetworkId, H160)>,
        pub reward_fraction: Perbill,
    }

    #[cfg(feature = "std")]
    impl Default for GenesisConfig {
        fn default() -> Self {
            Self {
                networks: Default::default(),
                reward_fraction: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig {
        fn build(&self) {
            for (network_id, channel) in &self.networks {
                Pallet::<T>::register_channel_inner(*network_id, *channel).unwrap();
            }
            RewardFraction::<T>::set(self.reward_fraction);
        }
    }
}
