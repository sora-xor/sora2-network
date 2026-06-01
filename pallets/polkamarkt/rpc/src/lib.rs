// This file is part of the SORA network and Polkaswap app.

use codec::Codec;
use jsonrpsee::{core::RpcResult as Result, proc_macros::rpc, types::ErrorObjectOwned};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::traits::{Block as BlockT, MaybeDisplay, MaybeFromStr};
use std::sync::Arc;

fn runtime_error_into_rpc_error(error: impl core::fmt::Debug) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(1, "Runtime error", Some(format!("{error:?}")))
}

pub use polkamarkt_runtime_api::PolkamarktAPI as PolkamarktRuntimeAPI;
use polkamarkt_runtime_api::{
    BuyQuote, ClaimableInfo, FlipQuote, LiquidityQuote, OrderBook, OrderQuote, SellQuote,
};

#[rpc(server)]
pub trait PolkamarktAPI<
    BlockHash,
    AccountId,
    Balance,
    OptionBuyQuote,
    OptionSellQuote,
    OptionLiquidityQuote,
    OptionFlipQuote,
    OptionOrderQuote,
    OptionOrderBook,
    OptionClaimableInfo,
>
{
    #[method(name = "polkamarkt_quoteBuy")]
    fn quote_buy(
        &self,
        market_id: u32,
        outcome: String,
        collateral_in: Balance,
        at: Option<BlockHash>,
    ) -> Result<OptionBuyQuote>;

    #[method(name = "polkamarkt_quoteSell")]
    fn quote_sell(
        &self,
        market_id: u32,
        outcome: String,
        shares_in: Balance,
        at: Option<BlockHash>,
    ) -> Result<OptionSellQuote>;

    #[method(name = "polkamarkt_quoteAddLiquidity")]
    fn quote_add_liquidity(
        &self,
        market_id: u32,
        collateral_in: Balance,
        at: Option<BlockHash>,
    ) -> Result<OptionLiquidityQuote>;

    #[method(name = "polkamarkt_quoteFlipPosition")]
    fn quote_flip_position(
        &self,
        market_id: u32,
        from_outcome: String,
        shares_in: Balance,
        at: Option<BlockHash>,
    ) -> Result<OptionFlipQuote>;

    #[method(name = "polkamarkt_quoteOrder")]
    fn quote_order(
        &self,
        market_id: u32,
        outcome: String,
        side: String,
        price_cents: u8,
        shares: Balance,
        at: Option<BlockHash>,
    ) -> Result<OptionOrderQuote>;

    #[method(name = "polkamarkt_orderBook")]
    fn order_book(
        &self,
        market_id: u32,
        outcome: String,
        depth: u32,
        at: Option<BlockHash>,
    ) -> Result<OptionOrderBook>;

    #[method(name = "polkamarkt_claimable")]
    fn claimable(
        &self,
        account_id: AccountId,
        market_id: u32,
        at: Option<BlockHash>,
    ) -> Result<OptionClaimableInfo>;
}

pub struct PolkamarktClient<C, B> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<B>,
}

impl<C, B> PolkamarktClient<C, B> {
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            _marker: Default::default(),
        }
    }
}

impl<C, Block, AccountId, Balance>
    PolkamarktAPIServer<
        <Block as BlockT>::Hash,
        AccountId,
        Balance,
        Option<BuyQuote<Balance>>,
        Option<SellQuote<Balance>>,
        Option<LiquidityQuote<Balance>>,
        Option<FlipQuote<Balance>>,
        Option<OrderQuote<Balance>>,
        Option<OrderBook<Balance>>,
        Option<ClaimableInfo<AccountId, Balance>>,
    > for PolkamarktClient<C, Block>
where
    Block: BlockT,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    C::Api: PolkamarktRuntimeAPI<Block, AccountId, Balance>,
    AccountId: Codec + MaybeFromStr + MaybeDisplay,
    Balance: Codec + MaybeFromStr + MaybeDisplay,
{
    fn quote_buy(
        &self,
        market_id: u32,
        outcome: String,
        collateral_in: Balance,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<Option<BuyQuote<Balance>>> {
        let api = self.client.runtime_api();
        let at = at.unwrap_or(self.client.info().best_hash);
        api.quote_buy(at, market_id, outcome, collateral_in)
            .map_err(runtime_error_into_rpc_error)
    }

    fn quote_sell(
        &self,
        market_id: u32,
        outcome: String,
        shares_in: Balance,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<Option<SellQuote<Balance>>> {
        let api = self.client.runtime_api();
        let at = at.unwrap_or(self.client.info().best_hash);
        api.quote_sell(at, market_id, outcome, shares_in)
            .map_err(runtime_error_into_rpc_error)
    }

    fn quote_add_liquidity(
        &self,
        market_id: u32,
        collateral_in: Balance,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<Option<LiquidityQuote<Balance>>> {
        let api = self.client.runtime_api();
        let at = at.unwrap_or(self.client.info().best_hash);
        api.quote_add_liquidity(at, market_id, collateral_in)
            .map_err(runtime_error_into_rpc_error)
    }

    fn quote_flip_position(
        &self,
        market_id: u32,
        from_outcome: String,
        shares_in: Balance,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<Option<FlipQuote<Balance>>> {
        let api = self.client.runtime_api();
        let at = at.unwrap_or(self.client.info().best_hash);
        api.quote_flip_position(at, market_id, from_outcome, shares_in)
            .map_err(runtime_error_into_rpc_error)
    }

    fn quote_order(
        &self,
        market_id: u32,
        outcome: String,
        side: String,
        price_cents: u8,
        shares: Balance,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<Option<OrderQuote<Balance>>> {
        let api = self.client.runtime_api();
        let at = at.unwrap_or(self.client.info().best_hash);
        api.quote_order(at, market_id, outcome, side, price_cents, shares)
            .map_err(runtime_error_into_rpc_error)
    }

    fn order_book(
        &self,
        market_id: u32,
        outcome: String,
        depth: u32,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<Option<OrderBook<Balance>>> {
        let api = self.client.runtime_api();
        let at = at.unwrap_or(self.client.info().best_hash);
        api.order_book(at, market_id, outcome, depth)
            .map_err(runtime_error_into_rpc_error)
    }

    fn claimable(
        &self,
        account_id: AccountId,
        market_id: u32,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<Option<ClaimableInfo<AccountId, Balance>>> {
        let api = self.client.runtime_api();
        let at = at.unwrap_or(self.client.info().best_hash);
        api.claimable(at, account_id, market_id)
            .map_err(runtime_error_into_rpc_error)
    }
}
