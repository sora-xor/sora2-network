use frame_support::codec::{Decode, Encode};
use frame_support::dispatch::Weight;
use frame_support::pallet_prelude::OptionQuery;
use frame_support::storage::types::StorageMap;
use frame_support::traits::Get;
use frame_support::{Blake2_256, RuntimeDebug};

use common::generate_storage_instance;

use crate::{Config, PermissionId};

#[derive(PartialEq, Eq, Clone, Copy, RuntimeDebug, Encode, Decode, scale_info::TypeInfo)]
#[repr(u8)]
enum Mode {
    // The action associated with the permission is permitted if the account has the permission, otherwise it's forbidden
    Permit,
    // The action associated with the permission is forbidden if the account has the permission, otherwise it's permitted
    Forbid,
}

generate_storage_instance!(Permissions, Modes);
type OldModes = StorageMap<ModesOldInstance, Blake2_256, PermissionId, Mode, OptionQuery>;

pub fn migrate<T: Config>() -> Weight {
    OldModes::remove_all(None);

    // There were 12 default permissions
    T::DbWeight::get().writes(12)
}

#[cfg(test)]
mod tests {
    use crate::mock::{ExtBuilder, Runtime};
    use crate::MINT;

    use super::{Mode, OldModes};

    #[test]
    fn migrate() {
        ExtBuilder::default().build().execute_with(|| {
            OldModes::insert(MINT, Mode::Permit);

            let _ = super::migrate::<Runtime>();

            assert_eq!(OldModes::get(&MINT), None);
        });
    }
}
