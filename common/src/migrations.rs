#[macro_export]
macro_rules! generate_storage_instance {
    ($pallet:ident, $storage:ident) => {
        $crate::paste::paste! {
            struct [<$storage OldInstance>];
            impl frame_support::traits::StorageInstance for [<$storage OldInstance>] {
                fn pallet_prefix() -> &'static str {
                    stringify!($pallet)
                }
                const STORAGE_PREFIX: &'static str = stringify!($storage);
            }
        }
    };
}
