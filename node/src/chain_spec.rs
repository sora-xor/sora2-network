// Copyright 2020 Parity Technologies (UK) Ltd.

use iroha_client_no_std::crypto as iroha_crypto;
use std::convert::TryFrom;

use cumulus_primitives::ParaId;

use parachain_runtime::{
	AccountId, BalancesConfig, GenesisConfig, Signature, SudoConfig, SystemConfig,
	ParachainInfoConfig, WASM_BINARY,
	//IrohaBridgeConfig,
	KSMConfig, XORConfig, DOTConfig,
};

use sc_chain_spec::{ChainSpecExtension, ChainSpecGroup};
use sc_service::ChainType;
use serde::{Deserialize, Serialize};
use sp_core::{sr25519, Pair, Public};
use sp_core::crypto::AccountId32;
use sp_runtime::traits::{IdentifyAccount, Verify};

/// Specialized `ChainSpec`. This is a specialization of the general Substrate ChainSpec type.
pub type ChainSpec = sc_service::GenericChainSpec<GenesisConfig, Extensions>;

/// Helper function to generate a crypto pair from seed
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	TPublic::Pair::from_string(&format!("//{}", seed), None)
		.expect("static values are valid; qed")
		.public()
}

/// The extensions for the [`ChainSpec`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ChainSpecGroup, ChainSpecExtension)]
#[serde(deny_unknown_fields)]
pub struct Extensions {
	/// The relay chain of the Parachain.
	pub relay_chain: String,
	/// The id of the Parachain.
	pub para_id: u32,
}

impl Extensions {
	/// Try to get the extension from the given `ChainSpec`.
	pub fn try_get(chain_spec: &Box<dyn sc_service::ChainSpec>) -> Option<&Self> {
		sc_chain_spec::get_extension(chain_spec.extensions())
	}
}

type AccountPublic = <Signature as Verify>::Signer;

/// Helper function to generate an account ID from seed
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
	AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
	AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

pub fn get_chain_spec(id: ParaId) -> ChainSpec {
	ChainSpec::from_genesis(
		"Local Testnet",
		"local_testnet",
		ChainType::Local,
		move || {
			testnet_genesis(
				get_account_id_from_seed::<sr25519::Public>("Alice"),
				vec![
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_account_id_from_seed::<sr25519::Public>("Bob"),
					get_account_id_from_seed::<sr25519::Public>("Charlie"),
					get_account_id_from_seed::<sr25519::Public>("Dave"),
					get_account_id_from_seed::<sr25519::Public>("Eve"),
					get_account_id_from_seed::<sr25519::Public>("Ferdie"),
					get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
					get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
					get_account_id_from_seed::<sr25519::Public>("Charlie//stash"),
					get_account_id_from_seed::<sr25519::Public>("Dave//stash"),
					get_account_id_from_seed::<sr25519::Public>("Eve//stash"),
					get_account_id_from_seed::<sr25519::Public>("Ferdie//stash"),
					AccountId32::from([
						52u8, 45, 84, 67, 137, 84, 47, 252, 35, 59, 237, 44, 144, 70, 71, 206, 243,
						67, 8, 115, 247, 189, 204, 26, 181, 226, 232, 81, 123, 12, 81, 120,
					]),
				],
                /*
				vec![iroha_crypto::PublicKey::try_from(vec![
					52u8, 45, 84, 67, 137, 84, 47, 252, 35, 59, 237, 44, 144, 70, 71, 206, 243, 67,
					8, 115, 247, 189, 204, 26, 181, 226, 232, 81, 123, 12, 81, 120,
				]).unwrap()],
                */
				id,
			)
		},
		vec![],
		None,
		None,
		None,
		Extensions {
			relay_chain: "local_testnet".into(),
			para_id: id.into(),
		},
	)
}

pub fn staging_test_net(id: ParaId) -> ChainSpec {
	ChainSpec::from_genesis(
		"Staging Testnet",
		"staging_testnet",
		ChainType::Live,
		move || {
			testnet_genesis(
				get_account_id_from_seed::<sr25519::Public>("Alice"),
				vec![get_account_id_from_seed::<sr25519::Public>("Alice")],
                /*
				vec![iroha_crypto::PublicKey::try_from(vec![
					52u8, 45, 84, 67, 137, 84, 47, 252, 35, 59, 237, 44, 144, 70, 71, 206, 243, 67,
					8, 115, 247, 189, 204, 26, 181, 226, 232, 81, 123, 12, 81, 120,
				]).unwrap()],
                */
				id,
			)
		},
		Vec::new(),
		None,
		None,
		None,
		Extensions {
			relay_chain: "rococo_local_testnet".into(),
			para_id: id.into(),
		},
	)
}

fn testnet_genesis(
	root_key: AccountId,
	endowed_accounts: Vec<AccountId>,
	//iroha_peers: Vec<iroha_crypto::PublicKey>,
	id: ParaId,
) -> GenesisConfig {
	GenesisConfig {
		frame_system: Some(SystemConfig {
			code: WASM_BINARY.to_vec(),
			changes_trie_config: Default::default(),
		}),
		pallet_sudo: Some(SudoConfig { key: root_key }),
		parachain_info: Some(ParachainInfoConfig { parachain_id: id }),
		pallet_balances_Instance1: Some(XORConfig {
		balances: endowed_accounts
				.iter()
				.cloned()
				//.filter(|x| {
				//	x != &AccountId32::from([
				//		52u8, 45, 84, 67, 137, 84, 47, 252, 35, 59, 237, 44, 144, 70, 71, 206, 243,
				//		67, 8, 115, 247, 189, 204, 26, 181, 226, 232, 81, 123, 12, 81, 120,
				//	])
				//})
				// .map(|k| (k, 1 << 60))
				.map(|k| (k, 1 << 8))
				//.map(|k| (k, 0))
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
		//iroha_bridge: Some(IrohaBridgeConfig { authorities: endowed_accounts.clone(), iroha_peers }),
	}
}
