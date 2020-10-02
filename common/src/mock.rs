use crate::AssetId;
use codec::{Decode, Encode};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

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
