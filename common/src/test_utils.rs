/// Can be useful to check that an extrinsic is failed due to an error in another pallet
#[macro_export]
macro_rules! assert_noop_msg {
    ( $x:expr, $msg:expr ) => {
        let h = frame_support::storage_root();
        if let Err(e) = $x {
            if let frame_support::dispatch::DispatchError::Module { message, .. } = e.error {
                assert_eq!(message, Some($msg));
            } else {
                panic!("expected DispatchError::Module, got {:?}", e.error);
            }
        } else {
            panic!("expected Err(_), got Ok(_)");
        }
        assert_eq!(h, frame_support::storage_root());
    };
}
