use crate::{AssetId, AssetId32, TechAssetId};
use codec::{Decode, Encode};
use frame_support::dispatch::DispatchError;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_std::convert::TryFrom;

#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, PartialOrd, Ord, Debug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Hash))]
#[repr(u8)]
pub enum ComicAssetId {
    GoldenTicket,
    AppleTree,
    Apple,
    Teapot,
    Flower,
    RedPepper,
    BlackPepper,
    AcmeSpyKit,
    BatteryForMusicPlayer,
    MusicPlayer,
    Headphones,
    GreenPromise,
    BluePromise,
}

impl crate::traits::IsRepresentation for ComicAssetId {
    fn is_representation(&self) -> bool {
        false
    }
}

impl From<AssetId> for AssetId32<ComicAssetId> {
    fn from(asset: AssetId) -> Self {
        let comic = ComicAssetId::from(asset);
        AssetId32::<ComicAssetId>::from(comic)
    }
}

impl From<AssetId> for ComicAssetId {
    fn from(asset_id: AssetId) -> Self {
        use ComicAssetId::*;
        match asset_id {
            AssetId::XOR => GoldenTicket,
            AssetId::DOT => AppleTree,
            AssetId::KSM => Apple,
            AssetId::USD => Teapot,
            AssetId::VAL => Flower,
            AssetId::PSWAP => RedPepper,
        }
    }
}

impl Default for ComicAssetId {
    fn default() -> Self {
        Self::GoldenTicket
    }
}

// This is never used, and just makes some tests compatible.
#[deprecated]
impl From<AssetId32<AssetId>> for AssetId32<ComicAssetId> {
    fn from(_asset: AssetId32<AssetId>) -> Self {
        unreachable!()
    }
}

// This is never used, and just makes some tests compatible.
#[deprecated]
impl<DEXId> From<TechAssetId<AssetId, DEXId>> for AssetId {
    fn from(_tech: TechAssetId<AssetId, DEXId>) -> Self {
        unimplemented!()
    }
}

// This is never used, and just makes some tests compatible.
#[deprecated]
impl<DEXId> TryFrom<AssetId> for TechAssetId<TechAssetId<AssetId, DEXId>, DEXId>
where
    TechAssetId<AssetId, DEXId>: Decode,
{
    type Error = DispatchError;
    fn try_from(_asset: AssetId) -> Result<Self, Self::Error> {
        unimplemented!()
    }
}

impl<DEXId> From<AssetId> for TechAssetId<ComicAssetId, DEXId> {
    fn from(asset_id: AssetId) -> Self {
        TechAssetId::Wrapped(ComicAssetId::from(asset_id))
    }
}
