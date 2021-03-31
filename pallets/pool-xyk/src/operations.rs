#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use sp_runtime::RuntimeDebug;

use crate::bounds::*;

#[derive(Clone, RuntimeDebug, Eq, PartialEq, Encode, Decode)]
pub struct Resource<AssetId, Balance> {
    // This is `AssetId` of `Resource`.
    pub asset: AssetId,
    // This is amount of `Resurce`.
    pub amount: Bounds<Balance>,
}

#[derive(Clone, RuntimeDebug, Eq, PartialEq, Encode, Decode)]
pub struct ResourcePair<AssetId, Balance>(
    pub Resource<AssetId, Balance>,
    pub Resource<AssetId, Balance>,
);

#[derive(Clone, RuntimeDebug, Eq, PartialEq, Encode, Decode)]
pub struct PairSwapAction<AssetId, Balance, AccountId, TechAccountId> {
    pub client_account: Option<AccountId>,
    pub receiver_account: Option<AccountId>,
    pub pool_account: TechAccountId,
    pub source: Resource<AssetId, Balance>,
    pub destination: Resource<AssetId, Balance>,
    pub fee: Option<Balance>,
    pub fee_account: Option<TechAccountId>,
    pub get_fee_from_destination: Option<bool>,
}

#[derive(Clone, RuntimeDebug, Eq, PartialEq, Encode, Decode)]
pub struct DepositLiquidityAction<AssetId, TechAssetId, Balance, AccountId, TechAccountId> {
    pub client_account: Option<AccountId>,
    pub receiver_account: Option<AccountId>,
    pub pool_account: TechAccountId,
    pub source: ResourcePair<AssetId, Balance>,
    pub destination: Resource<TechAssetId, Balance>,
    pub min_liquidity: Option<Balance>,
}

#[derive(Clone, RuntimeDebug, Eq, PartialEq, Encode, Decode)]
pub struct WithdrawLiquidityAction<AssetId, TechAssetId, Balance, AccountId, TechAccountId> {
    pub client_account: Option<AccountId>,
    pub receiver_account_a: Option<AccountId>,
    pub receiver_account_b: Option<AccountId>,
    pub pool_account: TechAccountId,
    pub source: Resource<TechAssetId, Balance>,
    pub destination: ResourcePair<AssetId, Balance>,
}

#[derive(Clone, RuntimeDebug, Eq, PartialEq, Encode, Decode)]
pub enum PolySwapAction<AssetId, TechAssetId, Balance, AccountId, TechAccountId> {
    PairSwap(PairSwapAction<AssetId, Balance, AccountId, TechAccountId>),
    DepositLiquidity(
        DepositLiquidityAction<AssetId, TechAssetId, Balance, AccountId, TechAccountId>,
    ),
    WithdrawLiquidity(
        WithdrawLiquidityAction<AssetId, TechAssetId, Balance, AccountId, TechAccountId>,
    ),
}
