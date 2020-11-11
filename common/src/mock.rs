use crate::AssetId;
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

impl From<AssetId> for crate::primitives::AssetId32<ComicAssetId> {
    fn from(asset: AssetId) -> Self {
        let comic = ComicAssetId::from(asset);
        crate::primitives::AssetId32::<ComicAssetId>::from(comic)
    }
}

impl From<AssetId> for ComicAssetId {
    fn from(asset_id: AssetId) -> Self {
        use crate::mock::ComicAssetId::*;
        match asset_id {
            AssetId::XOR => GoldenTicket,
            AssetId::DOT => AppleTree,
            AssetId::KSM => Apple,
            AssetId::USD => Teapot,
            AssetId::VAL => Flower,
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
impl<DEXId> From<crate::primitives::TechAssetId<AssetId, DEXId>> for AssetId {
    fn from(_tech: crate::primitives::TechAssetId<AssetId, DEXId>) -> Self {
        unimplemented!()
    }
}

// This is never used, and just makes some tests compatible.
#[deprecated]
impl<DEXId> TryFrom<AssetId>
    for crate::primitives::TechAssetId<crate::primitives::TechAssetId<AssetId, DEXId>, DEXId>
where
    crate::primitives::TechAssetId<AssetId, DEXId>: Decode,
{
    type Error = DispatchError;
    fn try_from(_asset: AssetId) -> Result<Self, Self::Error> {
        unimplemented!()
    }
}

use crate::primitives::*;
use sp_std::marker::PhantomData;
use sp_std::vec::Vec;

impl<AssetId> From<AssetId> for AssetId32<AssetId>
where
    AssetId32<AssetId>: From<TechAssetId<AssetId, DEXId>>,
    AssetId: crate::traits::IsRepresentation,
{
    fn from(asset_id: AssetId) -> Self {
        // Must be not representation, only direct asset must be here.
        // Assert must exist here because it must never heppend in runtime and must be covered by tests.
        assert!(!asset_id.is_representation());
        AssetId32::<AssetId>::from(TechAssetId::Wrapped(asset_id))
    }
}

impl<AssetId, DEXId> From<TechAssetId<AssetId, DEXId>> for AssetId32<AssetId>
where
    TechAssetId<AssetId, DEXId>: Encode,
    AssetId: crate::traits::IsRepresentation,
{
    fn from(tech_asset: TechAssetId<AssetId, DEXId>) -> Self {
        let mut slice = [0_u8; 32];
        let asset_encoded: Vec<u8> = tech_asset.encode();
        let asset_length = asset_encoded.len();
        // Encode size of TechAssetId must be always less or equal to 31.
        // Recursion of MakeTechAssetId is limited for this to specific number of iterations.
        // Assert must exist here because it must never heppend in runtime and must be covered by tests.
        assert!(asset_length <= 31);
        // Must be not representation, only direct asset must be here.
        // Assert must exist here because it must never heppend in runtime and must be covered by tests.
        assert!({
            match tech_asset {
                TechAssetId::Wrapped(a) => !a.is_representation(),
                _ => true,
            }
        });
        slice[0] = asset_length as u8;
        for i in 0..asset_length {
            slice[i + 1] = asset_encoded[i];
        }
        AssetId32::new(slice, PhantomData)
    }
}
