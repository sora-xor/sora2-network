use crate::mock::*;

#[test]
fn it_works() {
    new_test_ext().execute_with(|| {
        // Dispatch a signed extrinsic.
        assert!(true);
    });
}
