//! # Core
//!
//! Common traits and types

use crate::{
    types::{BridgeAppInfo, BridgeAssetInfo, MessageStatus},
    EthNetworkId,
};
use common::Balance;
use ethereum_types::H256;
use frame_support::dispatch::{DispatchError, DispatchResult};
use frame_system::{Config, RawOrigin};
use sp_core::{H160, U256};
use sp_std::prelude::*;

use crate::types::Message;

/// A trait for verifying messages.
///
/// This trait should be implemented by runtime modules that wish to provide message verification functionality.
pub trait Verifier {
    type Result;
    fn verify(network_id: EthNetworkId, message: &Message) -> Result<Self::Result, DispatchError>;
    fn initialize_storage(
        network_id: EthNetworkId,
        headers: Vec<crate::Header>,
        difficulty: u128,
        descendants_until_final: u8,
    ) -> Result<(), &'static str>;
}

/// Outbound submission for applications
pub trait OutboundChannel<AccountId> {
    fn submit(
        network_id: EthNetworkId,
        who: &RawOrigin<AccountId>,
        target: H160,
        max_gas: U256,
        payload: &[u8],
    ) -> Result<H256, DispatchError>;
}

/// Dispatch a message
pub trait MessageDispatch<T: Config, NetworkId, Source, MessageId> {
    fn dispatch(
        network_id: NetworkId,
        source: Source,
        id: MessageId,
        timestamp: u64,
        payload: &[u8],
    );
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

    fn list_supported_assets(network_id: EthNetworkId) -> Vec<BridgeAssetInfo<AssetId>>;

    fn list_apps(network_id: EthNetworkId) -> Vec<BridgeAppInfo>;
}

pub trait MessageStatusNotifier<AssetId, AccountId> {
    fn update_status(
        network_id: EthNetworkId,
        id: H256,
        status: MessageStatus,
        end_timestamp: Option<u64>,
    );

    fn inbound_request(
        network_id: EthNetworkId,
        message_id: H256,
        source: H160,
        dest: AccountId,
        asset_id: AssetId,
        amount: Balance,
        start_timestamp: u64,
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
    fn update_status(
        _network_id: EthNetworkId,
        _id: H256,
        _status: MessageStatus,
        _end_timestamp: Option<u64>,
    ) {
    }

    fn inbound_request(
        _network_id: EthNetworkId,
        _message_id: H256,
        _source: H160,
        _dest: AccountId,
        _asset_id: AssetId,
        _amount: Balance,
        _start_timestamp: u64,
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

/// Trait that every origin (like Ethereum origin or Parachain origin) should implement
pub trait OriginOutput<NetworkId, Source> {
    /// Construct new origin
    fn new(network_id: NetworkId, source: Source, message_id: H256, timestamp: u64) -> Self;
}
