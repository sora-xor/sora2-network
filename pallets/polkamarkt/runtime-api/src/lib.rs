// This file is part of the SORA network and Polkaswap app.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::string::String;
use codec::{Codec, Decode, Encode};
#[cfg(feature = "std")]
use common::utils::string_serialization;
use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::traits::{MaybeDisplay, MaybeFromStr};

#[derive(Clone, Eq, PartialEq, Encode, Decode, Default, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct BuyQuote<Balance> {
    pub market_id: u32,
    pub outcome: String,
    #[cfg_attr(
        feature = "std",
        serde(
            bound(
                serialize = "Balance: std::fmt::Display",
                deserialize = "Balance: std::str::FromStr"
            ),
            with = "string_serialization"
        )
    )]
    pub collateral_in: Balance,
    #[cfg_attr(
        feature = "std",
        serde(
            bound(
                serialize = "Balance: std::fmt::Display",
                deserialize = "Balance: std::str::FromStr"
            ),
            with = "string_serialization"
        )
    )]
    pub fee_amount: Balance,
    #[cfg_attr(
        feature = "std",
        serde(
            bound(
                serialize = "Balance: std::fmt::Display",
                deserialize = "Balance: std::str::FromStr"
            ),
            with = "string_serialization"
        )
    )]
    pub pricing_collateral: Balance,
    #[cfg_attr(
        feature = "std",
        serde(
            bound(
                serialize = "Balance: std::fmt::Display",
                deserialize = "Balance: std::str::FromStr"
            ),
            with = "string_serialization"
        )
    )]
    pub shares_out: Balance,
}

#[derive(Clone, Eq, PartialEq, Encode, Decode, Default, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct SellQuote<Balance> {
    pub market_id: u32,
    pub outcome: String,
    #[cfg_attr(
        feature = "std",
        serde(
            bound(
                serialize = "Balance: std::fmt::Display",
                deserialize = "Balance: std::str::FromStr"
            ),
            with = "string_serialization"
        )
    )]
    pub shares_in: Balance,
    #[cfg_attr(
        feature = "std",
        serde(
            bound(
                serialize = "Balance: std::fmt::Display",
                deserialize = "Balance: std::str::FromStr"
            ),
            with = "string_serialization"
        )
    )]
    pub gross_collateral_out: Balance,
    #[cfg_attr(
        feature = "std",
        serde(
            bound(
                serialize = "Balance: std::fmt::Display",
                deserialize = "Balance: std::str::FromStr"
            ),
            with = "string_serialization"
        )
    )]
    pub fee_amount: Balance,
    #[cfg_attr(
        feature = "std",
        serde(
            bound(
                serialize = "Balance: std::fmt::Display",
                deserialize = "Balance: std::str::FromStr"
            ),
            with = "string_serialization"
        )
    )]
    pub collateral_out: Balance,
}

#[derive(Clone, Eq, PartialEq, Encode, Decode, Default, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct ClaimableInfo<AccountId, Balance> {
    pub market_id: u32,
    #[cfg_attr(
        feature = "std",
        serde(
            bound(
                serialize = "AccountId: std::fmt::Display",
                deserialize = "AccountId: std::str::FromStr"
            ),
            with = "string_serialization"
        )
    )]
    pub account: AccountId,
    pub status: String,
    pub resolution_outcome: Option<String>,
    #[cfg_attr(
        feature = "std",
        serde(
            bound(
                serialize = "Balance: std::fmt::Display",
                deserialize = "Balance: std::str::FromStr"
            ),
            with = "string_serialization"
        )
    )]
    pub yes_shares: Balance,
    #[cfg_attr(
        feature = "std",
        serde(
            bound(
                serialize = "Balance: std::fmt::Display",
                deserialize = "Balance: std::str::FromStr"
            ),
            with = "string_serialization"
        )
    )]
    pub no_shares: Balance,
    #[cfg_attr(
        feature = "std",
        serde(
            bound(
                serialize = "Balance: std::fmt::Display",
                deserialize = "Balance: std::str::FromStr"
            ),
            with = "string_serialization"
        )
    )]
    pub net_collateral_paid: Balance,
    #[cfg_attr(
        feature = "std",
        serde(
            bound(
                serialize = "Balance: std::fmt::Display",
                deserialize = "Balance: std::str::FromStr"
            ),
            with = "string_serialization"
        )
    )]
    pub trader_payout: Balance,
    #[cfg_attr(
        feature = "std",
        serde(
            bound(
                serialize = "Balance: std::fmt::Display",
                deserialize = "Balance: std::str::FromStr"
            ),
            with = "string_serialization"
        )
    )]
    pub claimable_payout: Balance,
    pub creator_fees: Balance,
    pub is_creator: bool,
}

#[derive(Clone, Eq, PartialEq, Encode, Decode, Default, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct MarketState<Balance> {
    pub market_id: u32,
    pub mechanism: String,
    #[cfg_attr(
        feature = "std",
        serde(
            bound(
                serialize = "Balance: std::fmt::Display",
                deserialize = "Balance: std::str::FromStr"
            ),
            with = "string_serialization"
        )
    )]
    pub virtual_depth: Balance,
    #[cfg_attr(
        feature = "std",
        serde(
            bound(
                serialize = "Balance: std::fmt::Display",
                deserialize = "Balance: std::str::FromStr"
            ),
            with = "string_serialization"
        )
    )]
    pub real_yes_shares: Balance,
    #[cfg_attr(
        feature = "std",
        serde(
            bound(
                serialize = "Balance: std::fmt::Display",
                deserialize = "Balance: std::str::FromStr"
            ),
            with = "string_serialization"
        )
    )]
    pub real_no_shares: Balance,
    #[cfg_attr(
        feature = "std",
        serde(
            bound(
                serialize = "Balance: std::fmt::Display",
                deserialize = "Balance: std::str::FromStr"
            ),
            with = "string_serialization"
        )
    )]
    pub dpm_collateral: Balance,
    pub marginal_yes_price_bps: u32,
    pub marginal_no_price_bps: u32,
    pub implied_yes_probability_bps: u32,
    pub implied_no_probability_bps: u32,
}

sp_api::decl_runtime_apis! {
    pub trait PolkamarktAPI<AccountId, Balance> where
        AccountId: Codec + MaybeFromStr + MaybeDisplay,
        Balance: Codec + MaybeFromStr + MaybeDisplay,
    {
        fn quote_buy(market_id: u32, outcome: String, collateral_in: Balance) -> Option<BuyQuote<Balance>>;

        fn quote_sell(market_id: u32, outcome: String, shares_in: Balance) -> Option<SellQuote<Balance>>;

        fn market_state(market_id: u32) -> Option<MarketState<Balance>>;

        fn claimable(account_id: AccountId, market_id: u32) -> Option<ClaimableInfo<AccountId, Balance>>;
    }
}
