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
    FilterMode, LiquiditySourceType, Nonce, ResolveTime, Runtime, SwapVariant, Symbol,
};
use jsonrpsee::RpcModule;
use sc_client_api::AuxStore;
use sc_consensus_babe::{BabeApi, BabeWorkerHandle};
pub use sc_rpc::{DenyUnsafe, SubscriptionTaskExecutor};
use sc_service::TransactionPool;
use sp_api::ProvideRuntimeApi;
use sp_block_builder::BlockBuilder;
use sp_blockchain::{Error as BlockChainError, HeaderBackend, HeaderMetadata};
use sp_consensus::SelectChain;
use sp_core::H256 as Hash;
use sp_keystore::KeystorePtr;
use std::sync::Arc;

/// A type representing all RPC extensions.
pub type RpcExtension = RpcModule<()>;

use sc_consensus_beefy::communication::notification::{
    BeefyBestBlockStream, BeefyVersionedFinalityProofStream,
};
/// Dependencies for BEEFY
pub struct BeefyDeps {
    /// Receives notifications about finality proof events from BEEFY.
    pub beefy_finality_proof_stream: BeefyVersionedFinalityProofStream<Block>,
    /// Receives notifications about best block events from BEEFY.
    pub beefy_best_block_stream: BeefyBestBlockStream<Block>,
    /// Executor to drive the subscription manager in the BEEFY RPC handler.
    pub subscription_executor: SubscriptionTaskExecutor,
}

/// Extra dependencies for BABE.
pub struct BabeDeps {
    /// A handle to the BABE worker for issuing requests.
    pub babe_worker_handle: BabeWorkerHandle<Block>,
    /// The keystore that manages the keys of the node.
    pub keystore: KeystorePtr,
}

/// Full client dependencies
pub struct FullDeps<C, P, SC, B> {
    /// The client instance to use.
    pub client: Arc<C>,
    /// Transaction pool instance.
    pub pool: Arc<P>,
    /// The SelectChain Strategy
    pub select_chain: SC,
    /// A copy of the chain spec.
    pub chain_spec: Box<dyn sc_chain_spec::ChainSpec>,
    /// Whether to deny unsafe calls
    pub deny_unsafe: DenyUnsafe,
    /// BABE specific dependencies.
    pub babe: BabeDeps,
    /// BEEFY specific dependencies.
    pub beefy: BeefyDeps,
    /// Backend used by the node.
    pub backend: Arc<B>,
}

#[cfg(feature = "wip")]
pub fn add_wip_rpc<C>(
    mut rpc: RpcExtension,
    client: Arc<C>,
) -> Result<RpcExtension, Box<dyn std::error::Error + Send + Sync>>
where
    C: ProvideRuntimeApi<Block>,
    C: HeaderBackend<Block> + HeaderMetadata<Block, Error = BlockChainError>,
    C: Send + Sync + 'static,
    C::Api: beefy_light_client_rpc::BeefyLightClientRuntimeAPI<Block, beefy_light_client::BitField>,
{
    use beefy_light_client_rpc::{BeefyLightClientAPIServer, BeefyLightClientClient};
    rpc.merge(BeefyLightClientClient::new(client.clone()).into_rpc())?;
    Ok(rpc)
}

#[cfg(feature = "stage")]
pub fn add_stage_rpc(
    rpc: RpcExtension,
) -> Result<RpcExtension, Box<dyn std::error::Error + Send + Sync>> {
    Ok(rpc)
}

/// Instantiate full RPC extensions.
pub fn create_full<C, P, SC, B>(
    deps: FullDeps<C, P, SC, B>,
) -> Result<RpcExtension, Box<dyn std::error::Error + Send + Sync>>
where
    C: ProvideRuntimeApi<Block>
        + sc_client_api::BlockBackend<Block>
        + HeaderBackend<Block>
        + AuxStore
        + HeaderMetadata<Block, Error = BlockChainError>
        + Sync
        + Send
        + 'static,
    C: HeaderBackend<Block> + HeaderMetadata<Block, Error = BlockChainError>,
    C: Send + Sync + 'static,
    C::Api: mmr_rpc::MmrRuntimeApi<
        Block,
        <Block as sp_runtime::traits::Block>::Hash,
        <<Block as sp_runtime::traits::Block>::Header as sp_runtime::traits::Header>::Number,
    >,
    // C::Api: sp_beefy::BeefyApi<Block>,
    C::Api: substrate_frame_rpc_system::AccountNonceApi<Block, AccountId, Nonce>,
    C::Api: pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance>,
    C::Api: dex_api_rpc::DEXRuntimeAPI<
        Block,
        AssetId,
        DEXId,
        Balance,
        LiquiditySourceType,
        SwapVariant,
    >,
    C::Api: BabeApi<Block>,
    C::Api: oracle_proxy_rpc::OracleProxyRuntimeApi<Block, Symbol, ResolveTime>,
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
    C::Api: vested_rewards_rpc::VestedRewardsRuntimeApi<
        Block,
        AccountId,
        AssetId,
        Balance,
        common::CrowdloanTag,
    >,
    C::Api: farming_rpc::FarmingRuntimeApi<Block, AssetId>,
    C::Api: BlockBuilder<Block>,
    C::Api: farming_rpc::FarmingRuntimeApi<Block, AssetId>,
    C::Api: leaf_provider_rpc::LeafProviderRuntimeAPI<Block>,
    C::Api: bridge_proxy_rpc::BridgeProxyRuntimeAPI<Block, AssetId>,
    P: TransactionPool + Send + Sync + 'static,
    SC: SelectChain<Block> + 'static,
    B: sc_client_api::Backend<Block> + Send + Sync + 'static,
    B::State: sc_client_api::StateBackend<sp_runtime::traits::HashingFor<Block>>,
{
    use assets_rpc::{AssetsAPIServer, AssetsClient};
    use bridge_proxy_rpc::{BridgeProxyAPIServer, BridgeProxyClient};
    use dex_api_rpc::{DEXAPIServer, DEX};
    use dex_manager_rpc::{DEXManager, DEXManagerAPIServer};
    use eth_bridge_rpc::{EthBridgeApiServer, EthBridgeRpc};
    use farming_rpc::{FarmingApiServer, FarmingClient};
    use iroha_migration_rpc::{IrohaMigrationAPIServer, IrohaMigrationClient};
    use leaf_provider_rpc::{LeafProviderAPIServer, LeafProviderClient};
    use liquidity_proxy_rpc::{LiquidityProxyAPIServer, LiquidityProxyClient};
    use mmr_rpc::{Mmr, MmrApiServer};
    use oracle_proxy_rpc::{OracleProxyApiServer, OracleProxyClient};
    use pallet_transaction_payment_rpc::{TransactionPayment, TransactionPaymentApiServer};
    use pswap_distribution_rpc::{PswapDistributionAPIServer, PswapDistributionClient};
    use rewards_rpc::{RewardsAPIServer, RewardsClient};
    use sc_consensus_babe_rpc::{Babe, BabeApiServer};
    use sc_consensus_beefy_rpc::{Beefy, BeefyApiServer};
    use substrate_frame_rpc_system::{System, SystemApiServer};
    use trading_pair_rpc::{TradingPairAPIServer, TradingPairClient};
    use vested_rewards_rpc::{VestedRewardsApiServer, VestedRewardsClient};

    let mut io = RpcModule::new(());
    let FullDeps {
        client,
        pool,
        select_chain,
        chain_spec,
        deny_unsafe,
        babe,
        beefy,
        backend,
    } = deps;

    let BabeDeps {
        keystore,
        babe_worker_handle,
    } = babe;
    let properties = chain_spec.properties();

    io.merge(
        Mmr::new(
            client.clone(),
            backend
                .offchain_storage()
                .ok_or("Backend doesn't provide the required offchain storage")?,
        )
        .into_rpc(),
    )?;
    io.merge(
        Beefy::<Block>::new(
            beefy.beefy_finality_proof_stream,
            beefy.beefy_best_block_stream,
            beefy.subscription_executor,
        )?
        .into_rpc(),
    )?;

    io.merge(System::new(client.clone(), pool.clone(), deny_unsafe).into_rpc())?;
    io.merge(TransactionPayment::new(client.clone()).into_rpc())?;

    io.merge(
        Babe::new(
            client.clone(),
            babe_worker_handle.clone(),
            keystore,
            select_chain,
            deny_unsafe,
        )
        .into_rpc(),
    )?;

    io.merge(DEX::new(client.clone()).into_rpc())?;
    io.merge(DEXManager::new(client.clone()).into_rpc())?;
    io.merge(TradingPairClient::new(client.clone()).into_rpc())?;
    io.merge(AssetsClient::new(client.clone()).into_rpc())?;
    io.merge(LiquidityProxyClient::new(client.clone()).into_rpc())?;
    io.merge(OracleProxyClient::new(client.clone()).into_rpc())?;
    io.merge(EthBridgeRpc::new(client.clone()).into_rpc())?;
    io.merge(IrohaMigrationClient::new(client.clone()).into_rpc())?;
    io.merge(PswapDistributionClient::new(client.clone()).into_rpc())?;
    io.merge(RewardsClient::new(client.clone()).into_rpc())?;
    io.merge(VestedRewardsClient::new(client.clone()).into_rpc())?;
    io.merge(FarmingClient::new(client.clone()).into_rpc())?;
    io.merge(LeafProviderClient::new(client.clone()).into_rpc())?;
    io.merge(BridgeProxyClient::new(client.clone()).into_rpc())?;
    Ok(io)
}
