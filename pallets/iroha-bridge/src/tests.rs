use crate::{mock::*, KEY_TYPE, KEY_TYPE_2};
use frame_support::{assert_ok, traits::OnInitialize};
use sp_core::{crypto::AccountId32, sr25519, Pair, Public};
use sp_runtime::{
    traits::{Dispatchable, IdentifyAccount, Verify},
    MultiSignature as Signature,
};

use async_std::task;

use iroha::{bridge, config::Configuration, isi, prelude};
use iroha_client::client::account::by_id;
use iroha_client::{client::Client, config::Configuration as ClientConfiguration};
use iroha_client_no_std::prelude as no_std_prelude;
use parity_scale_codec::alloc::sync::Arc;
use parity_scale_codec::Decode;
use parking_lot::RwLock;
use sp_core::{
    offchain::{OffchainExt, TransactionPoolExt},
    testing::KeyStore,
    traits::KeystoreExt,
};
use sp_io::TestExternalities;
use std::thread;
use tempfile::TempDir;
// use crate::mock::offchain_testing::{self, OffchainState, PoolState, PendingRequest};

use sp_core::offchain::Timestamp;

use treasury::AssetKind;

pub type SubstrateAccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

pub struct ExtBuilder;

impl ExtBuilder {
    pub fn build() -> (
        TestExternalities,
        Arc<RwLock<PoolState>>,
        Arc<RwLock<OffchainState>>,
    ) {
        use sp_runtime::BuildStorage;

        let (offchain, offchain_state) = TestOffchainExt::new();
        let (pool, pool_state) = TestTransactionPoolExt::new();
        let keystore = KeyStore::new();
        keystore
            .write()
            .ed25519_generate_new(KEY_TYPE_2, Some("//Alice"))
            .unwrap();

        keystore
            .write()
            .sr25519_generate_new(KEY_TYPE, Some("//Alice"))
            .unwrap();

        let _root_account = get_account_id_from_seed::<sr25519::Public>("Alice");
        let endowed_accounts = vec![
            get_account_id_from_seed::<sr25519::Public>("Alice"),
            get_account_id_from_seed::<sr25519::Public>("Bob"),
        ];

        let storage = GenesisConfig {
            system: Some(frame_system::GenesisConfig::default()),
            pallet_balances_Instance1: Some(XORConfig {
                balances: endowed_accounts
                    .iter()
                    .cloned()
                    .filter(|x| {
                        x != &AccountId32::from([
                            52u8, 45, 84, 67, 137, 84, 47, 252, 35, 59, 237, 44, 144, 70, 71, 206,
                            243, 67, 8, 115, 247, 189, 204, 26, 181, 226, 232, 81, 123, 12, 81,
                            120,
                        ])
                    })
                    .map(|k| (k, 0))
                    .collect(),
            }),
            pallet_balances_Instance2: Some(DOTConfig {
                balances: endowed_accounts
                    .iter()
                    .cloned()
                    .map(|k| (k, 1 << 8))
                    .collect(),
            }),
            pallet_balances_Instance3: Some(KSMConfig {
                balances: endowed_accounts
                    .iter()
                    .cloned()
                    .map(|k| (k, 1 << 8))
                    .collect(),
            }),
            pallet_balances: Some(BalancesConfig {
                balances: endowed_accounts
                    .iter()
                    .cloned()
                    .map(|k| (k, 1 << 60))
                    .collect(),
            }),
            // pallet_sudo: Some(SudoConfig { key: root_key }),
            iroha_bridge: Some(IrohaBridgeConfig {
                authorities: endowed_accounts.clone(),
            }),
        }
        .build_storage()
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
    let iroha = prelude::Iroha::new(configuration);
    task::block_on(iroha.start()).expect("Failed to start Iroha.");
    //Prevents temp_dir from clean up untill the end of the tests.
    #[allow(clippy::empty_loop)]
    loop {}
}

/// A utility function for our tests. It simulates what the system module does for us (almost
/// analogous to `finalize_block`).
///
/// This function increments the block number and simulates what we have written in
/// `decl_module` as `fn offchain_worker(_now: T::BlockNumber)`: run the offchain logic if the
/// current node is an authority.
///
/// Also, since the offchain code might submit some transactions, it queries the transaction
/// queue and dispatches any submitted transaction. This is also needed because it is a
/// non-runtime logic (transaction queue) which needs to mocked inside a runtime test.
fn seal_block(n: u64, state: Arc<RwLock<PoolState>>, _oc_state: Arc<RwLock<OffchainState>>) {
    assert_eq!(System::block_number(), n);
    System::set_block_number(n + 1);
    IrohaBridge::offchain();

    let transactions = &mut state.write().transactions;
    while let Some(t) = transactions.pop() {
        let e: TestExtrinsic = Decode::decode(&mut &*t).unwrap();
        let (who, _) = e.signature.unwrap();
        let call = e.call;
        // in reality you would do `e.apply`, but this is a test. we assume we don't care
        // about validation etc.
        let _ = call.dispatch(Some(who).into()).unwrap();
    }
    IrohaBridge::on_initialize(System::block_number());
}

fn offchain_worker_loop(oc_state: Arc<RwLock<OffchainState>>) {
    tokio::runtime::Builder::new()
        .basic_scheduler()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async move {
            loop {
                {
                    let mut fulfilled_requests = vec![];
                    let mut reqs = vec![];
                    // I <3 tokio
                    {
                        let mut guard = oc_state.write();
                        guard.timestamp = Timestamp::from_unix_millis(
                            std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_millis() as u64,
                        );
                    }
                    {
                        let guard = oc_state.read();
                        let pending_requests = &guard.requests;
                        for (id, pending_request) in pending_requests {
                            if pending_request.sent && pending_request.response.is_none() {
                                reqs.push((id.0, pending_request.clone()));
                            }
                        }
                    }
                    for (id, pending_request) in reqs {
                        let bytes = reqwest::Client::new()
                            .post(&pending_request.uri)
                            .body(pending_request.body.clone())
                            .send()
                            .await
                            .unwrap()
                            .bytes()
                            .await
                            .unwrap()
                            .to_vec();

                        fulfilled_requests.push((id, pending_request, bytes));
                    }
                    {
                        let mut guard = oc_state.write();
                        for (id, request, bytes) in fulfilled_requests {
                            guard.fulfill_pending_request(id, request, bytes, vec![]);
                        }
                    }
                }
                thread::sleep(std::time::Duration::from_millis(100));
            }
        });
}

fn check_response_assets(response: &prelude::QueryResult, expected_xor_amount: u32) {
    if let prelude::QueryResult::GetAccount(get_account_result) = response {
        let account = &get_account_result.account;
        let assets = &account.assets;
        let xor_amount = assets
            .iter()
            .find(|(_, asset)| asset.id.definition_id.name == "XOR")
            .map(|(_, asset)| asset.quantity)
            .unwrap_or(0);
        assert_eq!(xor_amount, expected_xor_amount);
        println!(
            "{} account balance on Iroha is: {} XOR",
            account.id, expected_xor_amount
        );
    } else {
        panic!("insufficient XOR amount");
    }
}

#[async_std::test]
async fn should_transfer_asset_between_iroha_and_substrate() {
    thread::spawn(create_and_start_iroha);
    thread::sleep(std::time::Duration::from_secs(30));

    let configuration =
        ClientConfiguration::from_path("config.json").expect("Failed to load configuration.");
    let mut iroha_client = Client::new(&configuration);

    let bridge_account_id = prelude::AccountId::new("bridge", "polkadot");
    let get_bridge_account = by_id(bridge_account_id.clone());
    let response = iroha_client
        .request(&get_bridge_account)
        .await
        .expect("Failed to send request.");
    check_response_assets(&response, 0);

    let global_domain_name = "global";
    let user_account_id = prelude::AccountId::new("root".into(), global_domain_name);
    let get_user_account = by_id(user_account_id.clone());
    let response = iroha_client
        .request(&get_user_account)
        .await
        .expect("Failed to send request.");
    check_response_assets(&response, 100);
    let xor_asset_def = prelude::AssetDefinition::new(prelude::AssetDefinitionId {
        name: "XOR".into(),
        domain_name: global_domain_name.into(),
    });
    let iroha_transfer_xor = prelude::Transfer::new(
        user_account_id.clone(),
        prelude::Asset::with_quantity(
            prelude::AssetId::new(xor_asset_def.id.clone(), user_account_id.clone()),
            100,
        ),
        bridge_account_id.clone(),
    )
    .into();
    iroha_client
        .submit(iroha_transfer_xor)
        .await
        .expect("Failed to send request");
    thread::sleep(std::time::Duration::from_secs(3));

    let (mut ext, state, oc_state) = ExtBuilder::build();

    let oc_state_clone = oc_state.clone();

    let no_std_user_account_id = no_std_prelude::AccountId {
        name: user_account_id.name.clone(),
        domain_name: user_account_id.domain_name.clone(),
    };
    thread::spawn(|| offchain_worker_loop(oc_state_clone));
    ext.execute_with(|| {
        seal_block(1, state.clone(), oc_state.clone());
        seal_block(2, state.clone(), oc_state.clone());
        // TODO: check balance on substrate account?
    });

    let get_bridge_account = by_id(bridge_account_id.clone());
    let response = iroha_client
        .request(&get_bridge_account)
        .await
        .expect("Failed to send request.");
    check_response_assets(&response, 0);

    let get_user_account = by_id(user_account_id.clone());
    let response = iroha_client
        .request(&get_user_account)
        .await
        .expect("Failed to send request.");
    check_response_assets(&response, 0);

    ext.execute_with(|| {
        let signer = get_account_id_from_seed::<sr25519::Public>("Alice");
        let amount = 100u128;
        let nonce = 0u8;
        assert_ok!(IrohaBridge::request_transfer(
            Some(signer).into(),
            no_std_user_account_id.clone(),
            AssetKind::XOR,
            amount,
            nonce
        ));

        seal_block(3, state.clone(), oc_state.clone());
        seal_block(4, state.clone(), oc_state.clone());
    });
    thread::sleep(std::time::Duration::from_secs(10));

    let get_user_account = by_id(user_account_id.clone());
    let response = iroha_client
        .request(&get_user_account)
        .await
        .expect("Failed to send request.");
    check_response_assets(&response, 100);
}
