// This file is part of the SORA network and Polkaswap app.

// Copyright (c) 2020, 2021, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

// Redistribution and use in source and binary forms, with or without modification,
// are permitted provided that the following conditions are met:

// Redistributions of source code must retain the above copyright notice, this list
// of conditions and the following disclaimer.
// Redistributions in binary form must reproduce the above copyright notice, this
// list of conditions and the following disclaimer in the documentation and/or other
// materials provided with the distribution.
//
// All advertising materials mentioning features or use of this software must display
// the following acknowledgement: This product includes software developed by Polka Biome
// Ltd., SORA, and Polkaswap.
//
// Neither the name of the Polka Biome Ltd. nor the names of its contributors may be used
// to endorse or promote products derived from this software without specific prior written permission.

// THIS SOFTWARE IS PROVIDED BY Polka Biome Ltd. AS IS AND ANY EXPRESS OR IMPLIED WARRANTIES,
// INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL Polka Biome Ltd. BE LIABLE FOR ANY
// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
// BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS;
// OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
// USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

//! # Core
//!
//! Common traits and types

use core::fmt::Debug;

use crate::types::AssetKind;
use crate::types::AuxiliaryDigestItem;
use crate::EVMChainId;
use crate::GenericTimepoint;
use crate::H256;
use crate::U256;
use crate::{
    types::{BridgeAppInfo, BridgeAssetInfo, MessageStatus, RawAssetInfo},
    GenericAccount, GenericNetworkId,
};
use codec::FullCodec;
use frame_support::weights::Weight;
use frame_support::{dispatch::DispatchResult, Parameter};
use frame_system::{Config, RawOrigin};
use scale_info::TypeInfo;
use sp_core::H160;
use sp_runtime::traits::AtLeast32BitUnsigned;
use sp_runtime::traits::MaybeSerializeDeserialize;
use sp_runtime::DispatchError;
use sp_std::prelude::*;

/// A trait for verifying messages.
///
/// This trait should be implemented by runtime modules that wish to provide message verification functionality.
pub trait Verifier {
    type Proof: FullCodec + TypeInfo + Clone + Debug + PartialEq;

    /// Verify hashed message with given proof
    fn verify(network_id: GenericNetworkId, message: H256, proof: &Self::Proof) -> DispatchResult;

    /// The weight of the message verification function
    fn verify_weight(proof: &Self::Proof) -> Weight;

    /// Valid proof for this Verifier, used for benchmarking
    #[cfg(feature = "runtime-benchmarks")]
    fn valid_proof() -> Option<Self::Proof>;
}

/// Outbound submission for applications
pub trait OutboundChannel<NetworkId, AccountId, Additional> {
    fn submit(
        network_id: NetworkId,
        who: &RawOrigin<AccountId>,
        payload: &[u8],
        additional: Additional,
    ) -> Result<H256, DispatchError>;

    fn submit_weight() -> Weight;
}

pub trait EVMOutboundChannel {
    fn submit_gas(chain_id: EVMChainId) -> Result<U256, DispatchError>;
}

/// Dispatch a message
pub trait MessageDispatch<T: Config, NetworkId, MessageId, Additional> {
    fn dispatch(
        network_id: NetworkId,
        id: MessageId,
        timepoint: GenericTimepoint,
        payload: &[u8],
        additional: Additional,
    );

    fn dispatch_weight(payload: &[u8]) -> Weight;

    #[cfg(feature = "runtime-benchmarks")]
    fn successful_dispatch_event(id: MessageId) -> Option<<T as Config>::RuntimeEvent>;
}

pub trait AppRegistry<NetworkId, Source> {
    fn register_app(network_id: NetworkId, app: Source) -> DispatchResult;
    fn deregister_app(network_id: NetworkId, app: Source) -> DispatchResult;
}

impl<NetworkId, Source> AppRegistry<NetworkId, Source> for () {
    fn register_app(_network_id: NetworkId, _app: Source) -> DispatchResult {
        Ok(())
    }

    fn deregister_app(_network_id: NetworkId, _app: Source) -> DispatchResult {
        Ok(())
    }
}

pub trait BridgeApp<AccountId, Recipient, AssetId, Balance> {
    fn is_asset_supported(network_id: GenericNetworkId, asset_id: AssetId) -> bool;

    // Initiates transfer to Sidechain by burning the asset on substrate side
    fn transfer(
        network_id: GenericNetworkId,
        asset_id: AssetId,
        sender: AccountId,
        recipient: Recipient,
        amount: Balance,
    ) -> Result<H256, DispatchError>;

    fn refund(
        network_id: GenericNetworkId,
        message_id: H256,
        recipient: AccountId,
        asset_id: AssetId,
        amount: Balance,
    ) -> DispatchResult;

    fn list_supported_assets(network_id: GenericNetworkId) -> Vec<BridgeAssetInfo>;

    fn list_apps() -> Vec<BridgeAppInfo>;

    fn transfer_weight() -> Weight;

    fn refund_weight() -> Weight;

    fn is_asset_supported_weight() -> Weight;
}

pub trait EVMBridgeWithdrawFee<AccountId, AssetId> {
    fn withdraw_transfer_fee(
        who: &AccountId,
        chain_id: EVMChainId,
        asset_id: AssetId,
    ) -> DispatchResult;
}

impl<AccountId, AssetId> EVMBridgeWithdrawFee<AccountId, AssetId> for () {
    fn withdraw_transfer_fee(
        _who: &AccountId,
        _chain_id: EVMChainId,
        _asset_id: AssetId,
    ) -> DispatchResult {
        Err(DispatchError::Unavailable)
    }
}

impl<AccountId, Recipient, AssetId, Balance> BridgeApp<AccountId, Recipient, AssetId, Balance>
    for ()
{
    fn is_asset_supported(_network_id: GenericNetworkId, _asset_id: AssetId) -> bool {
        false
    }

    fn transfer(
        _network_id: GenericNetworkId,
        _asset_id: AssetId,
        _sender: AccountId,
        _recipient: Recipient,
        _amount: Balance,
    ) -> Result<H256, DispatchError> {
        Err(DispatchError::Unavailable)
    }

    fn refund(
        _network_id: GenericNetworkId,
        _message_id: H256,
        _recipient: AccountId,
        _asset_id: AssetId,
        _amount: Balance,
    ) -> DispatchResult {
        Err(DispatchError::Unavailable)
    }

    fn list_supported_assets(_network_id: GenericNetworkId) -> Vec<BridgeAssetInfo> {
        vec![]
    }

    fn list_apps() -> Vec<BridgeAppInfo> {
        vec![]
    }

    fn is_asset_supported_weight() -> Weight {
        Default::default()
    }

    fn transfer_weight() -> Weight {
        Default::default()
    }

    fn refund_weight() -> Weight {
        Default::default()
    }
}

#[allow(clippy::too_many_arguments)]
pub trait MessageStatusNotifier<AssetId, AccountId, Balance> {
    fn update_status(
        network_id: GenericNetworkId,
        message_id: H256,
        status: MessageStatus,
        end_timepoint: GenericTimepoint,
    );

    fn inbound_request(
        network_id: GenericNetworkId,
        message_id: H256,
        source: GenericAccount,
        dest: AccountId,
        asset_id: AssetId,
        amount: Balance,
        start_timestamp: GenericTimepoint,
        status: MessageStatus,
    );

    fn outbound_request(
        network_id: GenericNetworkId,
        message_id: H256,
        source: AccountId,
        dest: GenericAccount,
        asset_id: AssetId,
        amount: Balance,
        status: MessageStatus,
    );
}

impl<AssetId, AccountId, Balance> MessageStatusNotifier<AssetId, AccountId, Balance> for () {
    fn update_status(
        _network_id: GenericNetworkId,
        _message_id: H256,
        _status: MessageStatus,
        _end_timestamp: GenericTimepoint,
    ) {
    }

    fn inbound_request(
        _network_id: GenericNetworkId,
        _message_id: H256,
        _source: GenericAccount,
        _dest: AccountId,
        _asset_id: AssetId,
        _amount: Balance,
        _start_timestamp: GenericTimepoint,
        _status: MessageStatus,
    ) {
    }

    fn outbound_request(
        _network_id: GenericNetworkId,
        _message_id: H256,
        _source: AccountId,
        _dest: GenericAccount,
        _asset_id: AssetId,
        _amount: Balance,
        _status: MessageStatus,
    ) {
    }
}

/// Trait for gas price oracle on Ethereum-based networks.
pub trait EVMFeeHandler<AssetId> {
    /// Returns base fee for the best block.
    fn get_latest_base_fee(network_id: EVMChainId) -> Result<U256, DispatchError>;
    /// Get asset id of the sidechain native asset
    fn get_network_fee_asset(network_id: EVMChainId) -> Result<AssetId, DispatchError>;
    /// Fee was paid for transaction in sidechain
    fn on_fee_paid(network_id: EVMChainId, relayer: H160, amount: U256);
    /// Update base fee
    fn update_base_fee(network_id: EVMChainId, new_base_fee: U256, evm_block_number: u64);
    /// Verify base fee update parameters
    fn can_update_base_fee(
        network_id: EVMChainId,
        new_base_fee: U256,
        evm_block_number: u64,
    ) -> bool;
}

impl<AssetId> EVMFeeHandler<AssetId> for () {
    fn get_latest_base_fee(_network_id: EVMChainId) -> Result<U256, DispatchError> {
        Err(DispatchError::Unavailable)
    }
    fn get_network_fee_asset(_network_id: EVMChainId) -> Result<AssetId, DispatchError> {
        Err(DispatchError::Unavailable)
    }
    fn on_fee_paid(_network_id: EVMChainId, _relayer: H160, _amount: U256) {}
    fn update_base_fee(_network_id: EVMChainId, _new_base_fee: U256, _evm_block_number: u64) {}
    fn can_update_base_fee(
        _network_id: EVMChainId,
        _new_base_fee: U256,
        _evm_block_number: u64,
    ) -> bool {
        false
    }
}

/// Trait that every origin (like Ethereum origin or Parachain origin) should implement
pub trait BridgeOriginOutput: Sized {
    /// The Id of the network (i.e. Ethereum network id).
    type NetworkId: Default;

    /// The additional data for origin.
    type Additional: Default;

    /// Construct new origin
    fn new(
        network_id: Self::NetworkId,
        message_id: H256,
        timepoint: GenericTimepoint,
        additional: Self::Additional,
    ) -> Self;

    #[allow(clippy::result_unit_err)]
    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin() -> Result<Self, ()>;
}

pub trait BridgeAssetRegistry<AccountId, AssetId> {
    type AssetName: Parameter;
    type AssetSymbol: Parameter;

    fn register_asset(
        network_id: GenericNetworkId,
        name: Self::AssetName,
        symbol: Self::AssetSymbol,
    ) -> Result<AssetId, DispatchError>;

    fn ensure_asset_exists(asset_id: AssetId) -> bool;

    fn manage_asset(network_id: GenericNetworkId, asset_id: AssetId) -> DispatchResult;

    fn get_raw_info(asset_id: AssetId) -> RawAssetInfo;
}

pub trait AuxiliaryDigestHandler {
    fn add_item(item: AuxiliaryDigestItem);
}

impl AuxiliaryDigestHandler for () {
    fn add_item(_item: AuxiliaryDigestItem) {}
}

/// Converter trait for Balance precision in different networks.
pub trait BalancePrecisionConverter<AssetId, Balance, SidechainBalance> {
    /// Convert thischain balance to sidechain balance.
    ///
    /// **Returns**
    /// * `Balance` - rounded thischain balance
    /// * `SidechainBalance` - converted sidechain balance
    ///
    /// Or
    /// * `None` - if thischain balance can't be converted to sidechain balance
    fn to_sidechain(
        asset_id: &AssetId,
        sidechain_precision: u8,
        amount: Balance,
    ) -> Option<(Balance, SidechainBalance)>;

    /// Convert sidechain balance to thischain balance.
    ///
    /// **Returns**
    /// * `Balance` - rounded thischain balance
    /// * `SidechainBalance` - converted sidechain balance
    ///
    /// Or
    /// * `None` - if sidechain balance can't be converted to thischain balance
    fn from_sidechain(
        asset_id: &AssetId,
        sidechain_precision: u8,
        amount: SidechainBalance,
    ) -> Option<(Balance, SidechainBalance)>;
}

impl<AssetId, Balance: Clone> BalancePrecisionConverter<AssetId, Balance, Balance> for () {
    fn to_sidechain(
        _asset_id: &AssetId,
        _sidechain_precision: u8,
        amount: Balance,
    ) -> Option<(Balance, Balance)> {
        Some((amount.clone(), amount))
    }

    fn from_sidechain(
        _asset_id: &AssetId,
        _sidechain_precision: u8,
        amount: Balance,
    ) -> Option<(Balance, Balance)> {
        Some((amount.clone(), amount))
    }
}

pub trait TimepointProvider {
    fn get_timepoint() -> GenericTimepoint;
}

pub trait BridgeAssetLocker<AccountId> {
    type AssetId: Parameter + MaybeSerializeDeserialize;
    type Balance: Parameter + AtLeast32BitUnsigned + MaybeSerializeDeserialize;

    fn lock_asset(
        network_id: GenericNetworkId,
        asset_kind: AssetKind,
        who: &AccountId,
        asset_id: &Self::AssetId,
        amount: &Self::Balance,
    ) -> DispatchResult;

    fn unlock_asset(
        network_id: GenericNetworkId,
        asset_kind: AssetKind,
        who: &AccountId,
        asset_id: &Self::AssetId,
        amount: &Self::Balance,
    ) -> DispatchResult;

    fn withdraw_fee(
        network_id: GenericNetworkId,
        who: &AccountId,
        asset_id: &Self::AssetId,
        amount: &Self::Balance,
    ) -> DispatchResult;

    fn refund_fee(
        network_id: GenericNetworkId,
        who: &AccountId,
        asset_id: &Self::AssetId,
        amount: &Self::Balance,
    ) -> DispatchResult;
}

/// Temporary trait for Hashi bridge to handle asset lock and unlock
pub trait BridgeAssetLockChecker<AssetId, Balance> {
    /// Perform additional checks and operations before asset lock.
    fn before_asset_lock(
        network_id: GenericNetworkId,
        asset_kind: AssetKind,
        asset_id: &AssetId,
        amount: &Balance,
    ) -> DispatchResult;

    /// Perform additional checks and operations before asset unlock.
    fn before_asset_unlock(
        network_id: GenericNetworkId,
        asset_kind: AssetKind,
        asset_id: &AssetId,
        amount: &Balance,
    ) -> DispatchResult;
}

impl<AssetId, Balance> BridgeAssetLockChecker<AssetId, Balance> for () {
    fn before_asset_lock(
        _network_id: GenericNetworkId,
        _asset_kind: AssetKind,
        _asset_id: &AssetId,
        _amount: &Balance,
    ) -> DispatchResult {
        Ok(())
    }

    fn before_asset_unlock(
        _network_id: GenericNetworkId,
        _asset_kind: AssetKind,
        _asset_id: &AssetId,
        _amount: &Balance,
    ) -> DispatchResult {
        Ok(())
    }
}
