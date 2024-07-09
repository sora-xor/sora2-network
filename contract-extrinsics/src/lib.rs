#![cfg_attr(not(feature = "std"), no_std)]

use crate::assets::AssetsCall;
use crate::order_book::OrderBookCall;
use common::{AssetId32, PredefinedAssetId};
use frame_support::pallet_prelude::{MaybeSerializeDeserialize, Member};
use frame_support::sp_runtime::testing::H256;
use frame_support::sp_runtime::traits::{AtLeast32BitUnsigned, MaybeDisplay};
use frame_support::Parameter;
use scale::{Encode, MaxEncodedLen};
use sp_std::fmt::Debug;
pub mod assets;

pub mod order_book;

/// It is a part of the runtime dispatchables API.
/// `Ink!` doesn't expose the real enum, so we need a partial definition matching our targets.
/// You should get or count index of the pallet, using `construct_runtime!`, it is zero based
#[derive(Encode)]
pub enum RuntimeCall<AssetId: AssetIdBounds, AccountId: AccountIdBounds, OrderId: OrderIdBounds> {
    #[codec(index = 21)]
    Assets(AssetsCall<AssetId, AccountId>),
    #[codec(index = 57)]
    OrderBook(OrderBookCall<AssetId, OrderId>),
}

pub trait AssetIdBounds:
    Parameter
    + Member
    + Copy
    + MaybeSerializeDeserialize
    + Ord
    + Default
    + Into<AssetId32<PredefinedAssetId>>
    + From<AssetId32<PredefinedAssetId>>
    + From<H256>
    + Into<H256>
    + MaxEncodedLen
{
}

impl<T> AssetIdBounds for T where
    T: Parameter
        + Member
        + Copy
        + MaybeSerializeDeserialize
        + Ord
        + Default
        + Into<AssetId32<PredefinedAssetId>>
        + From<AssetId32<PredefinedAssetId>>
        + From<H256>
        + Into<H256>
        + MaxEncodedLen
{
}

pub trait AccountIdBounds:
    Parameter + Member + MaybeSerializeDeserialize + Debug + MaybeDisplay + Ord + MaxEncodedLen
{
}

impl<T> AccountIdBounds for T where
    T: Parameter + Member + MaybeSerializeDeserialize + Debug + MaybeDisplay + Ord + MaxEncodedLen
{
}

pub trait OrderIdBounds:
    Parameter
    + Member
    + MaybeSerializeDeserialize
    + Debug
    + MaybeDisplay
    + AtLeast32BitUnsigned
    + Copy
    + Ord
    + PartialEq
    + Eq
    + MaxEncodedLen
    + scale_info::TypeInfo
{
}

impl<T> OrderIdBounds for T where
    T: Parameter
        + Member
        + MaybeSerializeDeserialize
        + Debug
        + MaybeDisplay
        + AtLeast32BitUnsigned
        + Copy
        + Ord
        + PartialEq
        + Eq
        + MaxEncodedLen
        + scale_info::TypeInfo
{
}
