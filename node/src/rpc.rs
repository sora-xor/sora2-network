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

#![warn(missing_docs)]

use common::{ContentSource, Description, TradingPair};
use framenode_runtime::opaque::Block;
use framenode_runtime::{
    eth_bridge, AccountId, AssetId, AssetName, AssetSymbol, Balance, BalancePrecision, DEXId,
    FilterMode, Index, LiquiditySourceType, Runtime, SwapVariant,
};
pub use sc_rpc::{DenyUnsafe, SubscriptionTaskExecutor};
use sp_api::ProvideRuntimeApi;
use sp_block_builder::BlockBuilder;
use sp_blockchain::{Error as BlockChainError, HeaderBackend, HeaderMetadata};
use sp_transaction_pool::TransactionPool;
use std::sync::Arc;

/// JsonRpcHandler
pub type JsonRpcHandler = jsonrpc_core::IoHandler<sc_rpc::Metadata>;

/// Full client dependencies.
pub struct FullDeps<C, P> {
    /// The client instance to use.
    pub client: Arc<C>,
    /// Transaction pool instance.
    pub pool: Arc<P>,
    /// Whether to deny unsafe calls
    pub deny_unsafe: DenyUnsafe,
}

/// Instantiate full RPC extensions.
pub fn create_full<C, P>(deps: FullDeps<C, P>) -> JsonRpcHandler
where
    C: ProvideRuntimeApi<Block>,
    C: HeaderBackend<Block> + HeaderMetadata<Block, Error = BlockChainError>,
    C: Send + Sync + 'static,
    C::Api: substrate_frame_rpc_system::AccountNonceApi<Block, AccountId, Index>,
    C::Api: pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance>,
    C::Api: dex_api_rpc::DEXRuntimeAPI<
        Block,
        AssetId,
        DEXId,
        Balance,
        LiquiditySourceType,
        SwapVariant,
    >,
    C::Api: dex_manager_rpc::DEXManagerRuntimeAPI<Block, DEXId>,
    C::Api: trading_pair_rpc::TradingPairRuntimeAPI<
        Block,
        DEXId,
        TradingPair<AssetId>,
        AssetId,
        LiquiditySourceType,
    >,
    C::Api: assets_rpc::AssetsRuntimeAPI<
        Block,
        AccountId,
        AssetId,
        Balance,
        AssetSymbol,
        AssetName,
        BalancePrecision,
        ContentSource,
        Description,
    >,
    C::Api: liquidity_proxy_rpc::LiquidityProxyRuntimeAPI<
        Block,
        DEXId,
        AssetId,
        Balance,
        SwapVariant,
        LiquiditySourceType,
        FilterMode,
    >,
    C::Api: eth_bridge_rpc::EthBridgeRuntimeApi<
        Block,
        sp_core::H256,
        eth_bridge::offchain::SignatureParams,
        AccountId,
        eth_bridge::requests::AssetKind,
        AssetId,
        sp_core::H160,
        eth_bridge::requests::OffchainRequest<Runtime>,
        eth_bridge::requests::RequestStatus,
        eth_bridge::requests::OutgoingRequestEncoded,
        framenode_runtime::NetworkId,
        framenode_runtime::BalancePrecision,
    >,
    C::Api: iroha_migration_rpc::IrohaMigrationRuntimeAPI<Block>,
    C::Api: pswap_distribution_rpc::PswapDistributionRuntimeAPI<Block, AccountId, Balance>,
    C::Api: rewards_rpc::RewardsRuntimeAPI<Block, sp_core::H160, Balance>,
    C::Api: vested_rewards_rpc::VestedRewardsRuntimeApi<Block, AccountId, AssetId, Balance>,
    C::Api: BlockBuilder<Block>,
    P: TransactionPool + Send + Sync + 'static,
{
    use assets_rpc::{AssetsAPI, AssetsClient};
    use dex_api_rpc::{DEX, DEXAPI};
    use dex_manager_rpc::{DEXManager, DEXManagerAPI};
    use eth_bridge_rpc::{EthBridgeApi, EthBridgeRpc};
    // use farming_rpc::*;
    use iroha_migration_rpc::{IrohaMigrationAPI, IrohaMigrationClient};
    use liquidity_proxy_rpc::{LiquidityProxyAPI, LiquidityProxyClient};
    use pallet_transaction_payment_rpc::{TransactionPayment, TransactionPaymentApi};
    use pswap_distribution_rpc::{PswapDistributionAPI, PswapDistributionClient};
    use rewards_rpc::{RewardsAPI, RewardsClient};
    use substrate_frame_rpc_system::{FullSystem, SystemApi};
    use trading_pair_rpc::{TradingPairAPI, TradingPairClient};
    use vested_rewards_rpc::{VestedRewardsApi, VestedRewardsClient};

    let mut io = jsonrpc_core::IoHandler::default();
    let FullDeps {
        client,
        pool,
        deny_unsafe,
    } = deps;
    io.extend_with(SystemApi::to_delegate(FullSystem::new(
        client.clone(),
        pool,
        deny_unsafe,
    )));
    io.extend_with(TransactionPaymentApi::to_delegate(TransactionPayment::new(
        client.clone(),
    )));
    io.extend_with(DEXAPI::to_delegate(DEX::new(client.clone())));
    io.extend_with(DEXManagerAPI::to_delegate(DEXManager::new(client.clone())));
    io.extend_with(TradingPairAPI::to_delegate(TradingPairClient::new(
        client.clone(),
    )));
    io.extend_with(AssetsAPI::to_delegate(AssetsClient::new(client.clone())));
    io.extend_with(LiquidityProxyAPI::to_delegate(LiquidityProxyClient::new(
        client.clone(),
    )));
    // io.extend_with(FarmingApi::to_delegate(FarmingRpc::new(client.clone())));
    io.extend_with(EthBridgeApi::to_delegate(EthBridgeRpc::new(client.clone())));
    io.extend_with(IrohaMigrationAPI::to_delegate(IrohaMigrationClient::new(
        client.clone(),
    )));
    io.extend_with(PswapDistributionAPI::to_delegate(
        PswapDistributionClient::new(client.clone()),
    ));
    io.extend_with(RewardsAPI::to_delegate(RewardsClient::new(client.clone())));
    io.extend_with(VestedRewardsApi::to_delegate(VestedRewardsClient::new(
        client,
    )));
    io
}
