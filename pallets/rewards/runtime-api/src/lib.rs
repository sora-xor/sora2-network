#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Codec, Decode, Encode};
#[cfg(feature = "std")]
use common::utils::string_serialization;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::traits::{MaybeDisplay, MaybeFromStr};
use sp_std::prelude::*;

#[derive(Eq, PartialEq, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct BalanceInfo<Balance> {
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
    pub balance: Balance,
}

sp_api::decl_runtime_apis! {
    pub trait RewardsAPI<EthereumAddress, Balance> where
        EthereumAddress: Codec,
        Balance: Codec + MaybeFromStr + MaybeDisplay
    {
        fn claimables(eth_address: EthereumAddress) -> Vec<BalanceInfo<Balance>>;
    }
}
