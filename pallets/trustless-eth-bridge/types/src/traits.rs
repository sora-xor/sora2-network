//! # Core
//!
//! Common traits and types

use crate::{
    types::{AppKind, MessageStatus},
    EthNetworkId, Log,
};
use common::Balance;
use ethereum_types::H256;
use frame_support::dispatch::{DispatchError, DispatchResult};
use frame_system::{Config, RawOrigin};
use sp_core::{H160, U256};
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
        max_gas: U256,
        payload: &[u8],
    ) -> Result<H256, DispatchError>;
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

pub trait EvmBridgeApp<AccountId, AssetId, Balance> {
    fn is_asset_supported(network_id: EthNetworkId, asset_id: AssetId) -> bool;

    fn transfer(
        network_id: EthNetworkId,
        asset_id: AssetId,
        sender: AccountId,
        recipient: H160,
        amount: Balance,
    ) -> Result<H256, DispatchError>;

    fn list_supported_assets(network_id: EthNetworkId) -> Vec<(AppKind, AssetId)>;

    fn list_apps(network_id: EthNetworkId) -> Vec<(AppKind, H160)>;
}

pub trait MessageStatusNotifier<AssetId, AccountId> {
    fn update_status(network_id: EthNetworkId, id: H256, status: MessageStatus);

    fn inbound_request(
        network_id: EthNetworkId,
        message_id: H256,
        source: H160,
        dest: AccountId,
        asset_id: AssetId,
        amount: Balance,
    );

    fn outbound_request(
        network_id: EthNetworkId,
        message_id: H256,
        source: AccountId,
        dest: H160,
        asset_id: AssetId,
        amount: Balance,
    );
}

impl<AssetId, AccountId> MessageStatusNotifier<AssetId, AccountId> for () {
    fn update_status(_network_id: EthNetworkId, _id: H256, _status: MessageStatus) {}

    fn inbound_request(
        _network_id: EthNetworkId,
        _message_id: H256,
        _source: H160,
        _dest: AccountId,
        _asset_id: AssetId,
        _amount: Balance,
    ) {
    }

    fn outbound_request(
        _network_id: EthNetworkId,
        _message_id: H256,
        _source: AccountId,
        _dest: H160,
        _asset_id: AssetId,
        _amount: Balance,
    ) {
    }
}
