use frame_support::{
    sp_runtime::traits::BadOrigin,
    traits::Get,
    Parameter
};
use frame_system::RawOrigin;

/// Check on origin that it is a DEX owner.
pub trait EnsureDexOwner<DexId> {
    fn ensure_dex_owner<OuterOrigin, AccountId>(
        dex_id: &DexId,
        origin: OuterOrigin,
    ) -> Result<Option<AccountId>, BadOrigin>
    where
        OuterOrigin: Into<Result<RawOrigin<AccountId>, OuterOrigin>>;
}

impl<DexId> EnsureDexOwner<DexId> for () {
    fn ensure_dex_owner<OuterOrigin, AccountId>(
        _dex_id: &DexId,
        origin: OuterOrigin,
    ) -> Result<Option<AccountId>, BadOrigin>
    where
        OuterOrigin: Into<Result<RawOrigin<AccountId>, OuterOrigin>>,
    {
        match origin.into() {
            Ok(RawOrigin::Signed(t)) => Ok(Some(t)),
            Ok(RawOrigin::Root) => Ok(None),
            _ => Err(BadOrigin),
        }
    }
}

pub type AssetIdOf<T> = <T as Trait>::AssetId;

/// Common DEX trait. Used for DEX-related pallets.
pub trait Trait: frame_system::Trait + currencies::Trait {
    /// DEX identifier.
    type DexId: Parameter;
    /// DEX assets (currency) identifier.
    type AssetId: Parameter + Ord;
    /// The base asset as the core asset in all trading pairs
    type GetBaseAssetId: Get<AssetIdOf<Self>>;
    /// Performs checks for origin is a DEX owner.
    type EnsureDexOwner: EnsureDexOwner<Self::DexId>;

    fn ensure_dex_owner<OuterOrigin>(
        dex_id: &Self::DexId,
        origin: OuterOrigin,
    ) -> Result<Option<Self::AccountId>, BadOrigin>
    where
        OuterOrigin: Into<Result<RawOrigin<Self::AccountId>, OuterOrigin>>,
    {
        Self::EnsureDexOwner::ensure_dex_owner(dex_id, origin)
    }
}
