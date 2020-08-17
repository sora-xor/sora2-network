use crate::{mock::*, Error, KEY_TYPE, KEY_TYPE_2};
use frame_support::{assert_noop, assert_ok};
use sp_core::{crypto::AccountId32, ed25519, sr25519, Pair, Public};
use sp_runtime::{
    traits::{IdentifyAccount, Verify},
    MultiSignature as Signature,
};

use async_std::task;
use iroha::{bridge, config::Configuration, isi, prelude::*};
use iroha_client::{
    client::{self, Client},
    config::Configuration as ClientConfiguration,
};
use parity_scale_codec::alloc::sync::Arc;
use parking_lot::RwLock;
use sp_core::{
    offchain::{
        testing::{self, OffchainState, PoolState},
        OffchainExt, TransactionPoolExt,
    },
    testing::KeyStore,
    traits::KeystoreExt,
};
use sp_io::TestExternalities;
use std::thread;
use tempfile::TempDir;

pub type SubstrateAccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

pub struct ExtBuilder;

impl ExtBuilder {
    pub fn build() -> (
        TestExternalities,
        Arc<RwLock<PoolState>>,
        Arc<RwLock<OffchainState>>,
    ) {
        const PHRASE: &str =
            "expire stage crawl shell boss any story swamp skull yellow bamboo copy";

        let (offchain, offchain_state) = testing::TestOffchainExt::new();
        let (pool, pool_state) = testing::TestTransactionPoolExt::new();
        let keystore = KeyStore::new();
        keystore
            .write()
            .ed25519_generate_new(KEY_TYPE_2, Some("//OCW_ED"))
            .unwrap();

        keystore
            .write()
            .sr25519_generate_new(KEY_TYPE, Some("//OCW"))
            .unwrap();

        let root_account = get_account_id_from_seed::<sr25519::Public>("Alice");
        let endowed_accounts = vec![
            get_account_id_from_seed::<sr25519::Public>("Alice"),
            get_account_id_from_seed::<sr25519::Public>("Bob"),
        ];
        // let mut ext = new_test_ext(, );
        // system::GenesisConfig {
        //     // frame_system: Some(SystemConfig {
        //     //     code: WASM_BINARY.to_vec(),
        //     //     changes_trie_config: Default::default(),
        //     // }),
        //     pallet_balances: Some(BalancesConfig {
        //         balances: endowed_accounts
        //             .iter()
        //             .cloned()
        //             .map(|k| (k, 1 << 60))
        //             .collect(),
        //     }),
        //     // pallet_sudo: Some(SudoConfig { key: root_key }),
        // }.build_storage::<Test>().unwrap().into()

        let storage = frame_system::GenesisConfig::default()
            .build_storage::<Test>()
            .unwrap();

        let mut t = TestExternalities::from(storage);
        t.register_extension(OffchainExt::new(offchain));
        t.register_extension(TransactionPoolExt::new(pool));
        t.register_extension(KeystoreExt(keystore));
        t.execute_with(|| System::set_block_number(1));
        (t, pool_state, offchain_state)
    }
}

pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
    TPublic::Pair::from_string(&format!("//{}", seed), None)
        .expect("static values are valid; qed")
        .public()
}

type AccountPublic = <Signature as Verify>::Signer;

/// Helper function to generate an account ID from seed
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> SubstrateAccountId
where
    AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
    AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

fn create_and_start_iroha() {
    let temp_dir = TempDir::new().expect("Failed to create TempDir.");
    let mut configuration =
        Configuration::from_path("config.json").expect("Failed to load configuration.");
    configuration
        .kura_configuration
        .kura_block_store_path(temp_dir.path());
    let iroha = Iroha::new(configuration);
    task::block_on(iroha.start()).expect("Failed to start Iroha.");
    //Prevents temp_dir from clean up untill the end of the tests.
    #[allow(clippy::empty_loop)]
    loop {}
}

#[test]
fn it_works_for_default_value() {
    thread::spawn(create_and_start_iroha);
    thread::sleep(std::time::Duration::from_millis(3000));
    let (mut ext, _, _) = ExtBuilder::build();

    // let mut ext = new_test_ext(get_account_id_from_seed::<sr25519::Public>("Alice"), vec![get_account_id_from_seed::<sr25519::Public>("Alice"), get_account_id_from_seed::<sr25519::Public>("Bob"), ]);
    // let (offchain, state) = TestOffchainExt::new();
    // ext.set_offchain_externalities(offchain);
    // ext.register_extension(offchain);
    // offchain.
    ext.execute_with(|| {
        let signer = get_account_id_from_seed::<sr25519::Public>("Alice");
        assert_ok!(IrohaBridge::fetch_blocks_signed(Origin::signed(
            signer.clone()
        )));
        thread::sleep(std::time::Duration::from_millis(1000));
        assert_ok!(IrohaBridge::fetch_blocks_signed(Origin::signed(signer)));
        // assert_eq!(TemplateModule::something(), Some(42));
    });
}

// #[test]
// fn correct_error_for_none_value() {
//     new_test_ext().execute_with(|| {
//         // Ensure the correct error is thrown on None value
//         assert_noop!(
// 			TemplateModule::cause_error(Origin::signed(1)),
// 			Error::<Test>::NoneValue
// 		);
//     });
// }u
