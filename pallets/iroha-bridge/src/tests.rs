// Tests to be written here

#[cfg(test)]
mod tests {
    use crate::{mock::*, Error};
    use frame_support::{assert_noop, assert_ok};

    #[test]
    fn it_works_for_default_value() {
        new_test_ext().execute_with(|| {
            // Just a dummy test for the dummy function `do_something`
            // calling the `do_something` function with a value 42
            // assert_ok!(TemplateModule::do_something(Origin::signed(1), 42));
            // asserting that the stored value is equal to what we stored
            // assert_eq!(TemplateModule::something(), Some(42));
        });
    }

    #[test]
    fn correct_error_for_none_value() {
        // new_test_ext().execute_with(|| {
        //     // Ensure the correct error is thrown on None value
        //     assert_noop!(
        //     TemplateModule::cause_error(Origin::signed(1)),
        //     Error::<TestRuntime>::NoneValue
        // );
        // });
    }

    use async_std::task;
    use iroha::{bridge, config::Configuration, isi, prelude::*};
    use iroha_client::{
        client::{self, Client},
        config::Configuration as ClientConfiguration,
    };
    use std::thread;
    use tempfile::TempDir;
    fn create_and_start_iroha() {
        let temp_dir = TempDir::new().expect("Failed to create TempDir.");
        let mut configuration =
            Configuration::from_path("").expect("Failed to load configuration.");
        configuration
            .kura_configuration
            .kura_block_store_path(temp_dir.path());
        let iroha = Iroha::new(configuration);
        task::block_on(iroha.start()).expect("Failed to start Iroha.");
        //Prevents temp_dir from clean up untill the end of the tests.
        #[allow(clippy::empty_loop)]
        loop {}
    }
}
