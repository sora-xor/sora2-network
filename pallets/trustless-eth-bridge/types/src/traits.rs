//! # Core
//!
//! Common traits and types

use crate::{EthNetworkId, Log};
use frame_support::dispatch::{DispatchError, DispatchResult};
use frame_system::{Config, RawOrigin};
use sp_core::H160;
use sp_std::prelude::*;

use crate::types::{ChannelId, Message};

/// A trait for verifying messages.
///
/// This trait should be implemented by runtime modules that wish to provide message verification functionality.
pub trait Verifier {
    fn verify(network_id: EthNetworkId, message: &Message) -> Result<Log, DispatchError>;
    fn initialize_storage(
        network_id: EthNetworkId,
        headers: Vec<crate::Header>,
        difficulty: u128,
        descendants_until_final: u8,
    ) -> Result<(), &'static str>;
}

/// Outbound submission for applications
pub trait OutboundRouter<AccountId> {
    fn submit(
        network_id: EthNetworkId,
        channel_id: ChannelId,
        who: &RawOrigin<AccountId>,
        target: H160,
        payload: &[u8],
    ) -> DispatchResult;
}

/// Add a message to a commitment
pub trait MessageCommitment {
    fn add(channel_id: ChannelId, target: H160, nonce: u64, payload: &[u8]) -> DispatchResult;
}

/// Dispatch a message
pub trait MessageDispatch<T: Config, MessageId> {
    fn dispatch(network_id: EthNetworkId, source: H160, id: MessageId, payload: &[u8]);
    #[cfg(feature = "runtime-benchmarks")]
    fn successful_dispatch_event(id: MessageId) -> Option<<T as Config>::Event>;
}

pub trait AppRegistry {
    fn register_app(network_id: EthNetworkId, app: H160) -> DispatchResult;
    fn deregister_app(network_id: EthNetworkId, app: H160) -> DispatchResult;
}

impl AppRegistry for () {
    fn register_app(_network_id: EthNetworkId, _app: H160) -> DispatchResult {
        Ok(())
    }

    fn deregister_app(_network_id: EthNetworkId, _app: H160) -> DispatchResult {
        Ok(())
    }
}
