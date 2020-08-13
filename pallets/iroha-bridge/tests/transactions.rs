#[cfg(test)]
mod tests {
    use async_std::task;
    use iroha::{bridge, config::Configuration, isi, prelude::*};
    use iroha_client::{
        client::{self, Client},
        config::Configuration as ClientConfiguration,
    };
    use std::thread;
    use tempfile::TempDir;

    const CONFIGURATION_PATH: &str = "../../../../config.json";

    #[async_std::test]
    //TODO: use cucumber to write `gherkin` instead of code.
    async fn client_can_transfer_asset_to_another_account() {
        // Given
        thread::spawn(create_and_start_iroha);

        let bpk = PublicKey {
            inner: [
                52, 80, 113, 218, 85, 229, 220, 206, 250, 170, 68, 3, 57, 65, 94, 249, 242, 102,
                51, 56, 163, 143, 125, 160, 223, 33, 190, 90, 180, 224, 85, 239,
            ],
        };
        let bsk = PrivateKey {
            inner: vec![
                250, 199, 149, 157, 191, 231, 47, 5, 46, 90, 12, 60, 141, 101, 48, 242, 2, 176, 47,
                216, 249, 245, 202, 53, 128, 236, 141, 235, 119, 151, 71, 158, 52, 80, 113, 218,
                85, 229, 220, 206, 250, 170, 68, 3, 57, 65, 94, 249, 242, 102, 51, 56, 163, 143,
                125, 160, 223, 33, 190, 90, 180, 224, 85, 239,
            ],
        };

        thread::sleep(std::time::Duration::from_millis(100));
        let configuration =
            Configuration::from_path(CONFIGURATION_PATH).expect("Failed to load configuration.");
        let mut iroha_client = Client::new(&ClientConfiguration::from_iroha_configuration(
            &configuration,
        ));
        let domain_name = "domain";
        let create_domain = isi::Add {
            object: Domain::new(domain_name.to_string()),
            destination_id: PeerId::new(
                &configuration.torii_configuration.torii_url,
                &configuration.public_key,
            ),
        };
        let account1_name = "bridge_admin";
        let account2_name = "account2";
        let bridge_acc_id = AccountId::new(account1_name, domain_name);
        let account_id = AccountId::new(account2_name, domain_name);
        let bridge_admin_account_id = AccountId::new(account1_name, &domain_name);
        let (public_key, _) = configuration.key_pair();
        let create_account1 = isi::Register {
            object: Account::with_signatory(account1_name, domain_name, public_key),
            destination_id: String::from(domain_name),
        };
        let create_account2 = isi::Register {
            object: Account::with_signatory(account2_name, domain_name, public_key),
            destination_id: String::from(domain_name),
        };
        let asset_definition_id = AssetDefinitionId::new("xor", domain_name);
        let quantity: u32 = 200;
        let create_asset = isi::Register {
            object: AssetDefinition::new(asset_definition_id.clone()),
            destination_id: domain_name.to_string(),
        };
        let mint_asset = isi::Mint {
            object: quantity,
            destination_id: AssetId {
                definition_id: asset_definition_id.clone(),
                account_id: bridge_acc_id.clone(),
            },
        };

        let bridge_domain_name = "polkadot".to_string();
        let bridge_def_id = BridgeDefinitionId {
            name: bridge_domain_name.clone(),
        };
        let bridge_def = BridgeDefinition {
            id: bridge_def_id.clone(),
            kind: BridgeKind::IClaim,
            owner_account_id: bridge_admin_account_id.clone(),
        };
        let ext_asset = bridge::asset::ExternalAsset {
            bridge_id: BridgeId::new(&bridge_def_id.name),
            name: "DOT".to_string(),
            id: "DOT".to_string(),
            decimals: 10,
        };
        let peer_id = PeerId::new("", &Default::default());
        let register_bridge = bridge::isi::register_bridge(peer_id, &bridge_def);
        let register_client = bridge::isi::add_client(&bridge_def_id, bpk.clone());
        let dot_asset_def = AssetDefinition::new(AssetDefinitionId {
            name: "DOT".to_string(),
            domain_name: bridge_domain_name.clone(),
        });
        let register_dot_asset = Register::new(dot_asset_def, bridge_domain_name.clone()).into();
        let xor_asset_def = AssetDefinition::new(AssetDefinitionId {
            name: "XOR".to_string(),
            domain_name: "global".into(),
        });
        let register_xor_asset =
            Register::new(xor_asset_def.clone(), domain_name.to_owned()).into();
        let register_ext_asset = bridge::isi::register_external_asset(&ext_asset);
        let mint_xor = Mint::new(
            100u32,
            AssetId::new(xor_asset_def.id.clone(), account_id.clone()),
        )
        .into();
        let bridge_account_id = AccountId::new("bridge", "polkadot");
        let transfer_xor = Transfer::new(
            account_id.clone(),
            Asset::with_quantity(
                AssetId::new(xor_asset_def.id.clone(), account_id.clone()),
                100,
            ),
            bridge_account_id.clone(),
        )
        .into();

        iroha_client
            .submit_all(vec![
                create_domain.into(),
                create_account1.into(),
                create_account2.into(),
                create_asset.into(),
                mint_asset.into(),
                register_xor_asset,
                register_bridge,
                register_client,
                register_dot_asset,
                register_ext_asset,
                mint_xor,
                transfer_xor,
            ])
            .await
            .expect("Failed to prepare state.");
        std::thread::sleep(std::time::Duration::from_millis(
            &configuration.sumeragi_configuration.pipeline_time_ms() * 2,
        ));
        //When
        let quantity = 20;
        let transfer_asset = isi::Transfer {
            source_id: bridge_acc_id.clone(),
            destination_id: account_id.clone(),
            object: Asset::with_quantity(
                AssetId {
                    definition_id: asset_definition_id.clone(),
                    account_id: bridge_acc_id.clone(),
                },
                quantity,
            ),
        };
        iroha_client
            .submit(transfer_asset.into())
            .await
            .expect("Failed to submit instruction.");
        std::thread::sleep(std::time::Duration::from_millis(
            &configuration.sumeragi_configuration.pipeline_time_ms() * 2,
        ));
        //Then
        let request = client::assets::by_account_id(account_id.clone());
        let query_result = iroha_client
            .request(&request)
            .await
            .expect("Failed to execute request.");
        if let QueryResult::GetAccountAssets(result) = query_result {
            let asset = result.assets.first().expect("Asset should exist.");
            assert_eq!(quantity, asset.quantity,);
            assert_eq!(account_id, asset.id.account_id,);
        } else {
            panic!("Wrong Query Result Type.");
        }
    }

    fn create_and_start_iroha() {
        let temp_dir = TempDir::new().expect("Failed to create TempDir.");
        let mut configuration =
            Configuration::from_path(CONFIGURATION_PATH).expect("Failed to load configuration.");
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
