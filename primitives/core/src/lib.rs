//! # Core
//!
//! Common traits and types

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::dispatch::{DispatchError, DispatchResult};
use frame_system::Config;
use snowbridge_ethereum::{EthNetworkId, Log};
use sp_core::H160;

pub mod assets;
pub mod nft;
pub mod types;

pub use types::{ChannelId, Message, MessageId, MessageNonce, Proof};

pub use assets::{AssetId, MultiAsset, SingleAsset};

pub use nft::{ERC721TokenData, TokenInfo};

/// A trait for verifying messages.
///
/// This trait should be implemented by runtime modules that wish to provide message verification functionality.
pub trait Verifier {
    fn verify(network_id: EthNetworkId, message: &Message) -> Result<Log, DispatchError>;
}

/// Outbound submission for applications
pub trait OutboundRouter<AccountId> {
    fn submit(
        network_id: EthNetworkId,
        channel: H160,
        channel_id: ChannelId,
        who: &AccountId,
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
