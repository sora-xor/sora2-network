// This file is part of the SORA network and Polkaswap app.
//
// Copyright (c) 2026, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

use crate as sccp;
use bridge_types::traits::AuxiliaryDigestHandler;
use bridge_types::types::AuxiliaryDigestItem;
use codec::Encode;
use common::prelude::Balance;
use common::{
    mock_assets_config, mock_common_config, mock_currencies_config, mock_frame_system_config,
    mock_pallet_balances_config, mock_permissions_config, mock_technical_config,
    mock_tokens_config, DEXId, XST,
};
use currencies::BasicCurrencyAdapter;
use frame_support::weights::Weight;
use frame_support::{construct_runtime, parameter_types};
use frame_system;
use orml_traits::parameter_type_with_key;
use sp_core::crypto::AccountId32;
use sp_core::H256;
use sp_runtime::traits::Convert;
use sp_runtime::BuildStorage;
use sp_runtime::Perbill;
use sp_std::cell::RefCell;
use sp_std::collections::btree_set::BTreeSet;
use sp_std::marker::PhantomData;

#[allow(unused_imports)]
pub use common::mock::*;

pub type BlockNumber = u64;
pub type AccountId = AccountId32;
pub type Amount = i128;

pub type AssetId = common::AssetId32<common::mock::ComicAssetId>;
pub type TechAssetId = common::TechAssetId<common::mock::ComicAssetId>;
pub type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;

type Block = frame_system::mocking::MockBlock<Runtime>;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = Weight::from_parts(1024, 0);
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    pub const GetBaseAssetId: AssetId = common::AssetId32 { code: [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], phantom: PhantomData };
    pub const SccpMaxRemoteTokenIdLen: u32 = 64;
    pub const SccpMaxDomains: u32 = 16;
    pub const SccpMaxBscValidators: u32 = 64;
    pub const SccpMaxAttesters: u32 = 64;
}

parameter_type_with_key! {
    pub ExistentialDeposits: |_currency_id: AssetId| -> Balance {
        0
    };
}

construct_runtime! {
    pub enum Runtime {
        System: frame_system,
        Permissions: permissions,
        Balances: pallet_balances,
        Tokens: tokens,
        Currencies: currencies,
        Assets: assets,
        Technical: technical,
        Sccp: sccp,
    }
}

mock_assets_config!(Runtime);
mock_common_config!(Runtime);
mock_currencies_config!(Runtime);
mock_frame_system_config!(Runtime);
mock_pallet_balances_config!(Runtime);
mock_permissions_config!(Runtime);
mock_technical_config!(Runtime);
mock_tokens_config!(Runtime);

parameter_types! {
    pub GetBuyBackAssetId: AssetId = XST.into();
}

pub struct AccountId32Converter;

impl Convert<[u8; 32], AccountId> for AccountId32Converter {
    fn convert(a: [u8; 32]) -> AccountId {
        AccountId32::from(a)
    }
}

impl sccp::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type ManagerOrigin = frame_system::EnsureRoot<AccountId>;
    type AccountIdConverter = AccountId32Converter;
    type AssetInfoProvider = Assets;
    type LegacyBridgeAssetChecker = MockLegacyBridgeChecker;
    type AuxiliaryDigestHandler = MockAuxiliaryDigestHandler;
    type EthFinalizedStateProvider = MockEthFinalizedStateProvider;
    type EthFinalizedBurnProofVerifier = MockEthFinalizedBurnProofVerifier;
    type EthZkFinalizedBurnProofVerifier = MockEthZkFinalizedBurnProofVerifier;
    type SolanaFinalizedBurnProofVerifier = MockSolanaFinalizedBurnProofVerifier;
    type SubstrateFinalizedBurnProofVerifier = MockSubstrateFinalizedBurnProofVerifier;
    type MaxRemoteTokenIdLen = SccpMaxRemoteTokenIdLen;
    type MaxDomains = SccpMaxDomains;
    type MaxBscValidators = SccpMaxBscValidators;
    type MaxAttesters = SccpMaxAttesters;
    type WeightInfo = ();
}

thread_local! {
    static LEGACY_BRIDGE_ASSETS: RefCell<BTreeSet<AssetId>> = RefCell::new(BTreeSet::new());
    static AUX_DIGEST_ITEMS: RefCell<Vec<AuxiliaryDigestItem>> = RefCell::new(Vec::new());
    static ETH_FINALIZED_STATE: RefCell<Option<(H256, H256)>> = RefCell::new(None);
    static ETH_FINALIZED_VERIFY_RESULT: RefCell<Option<bool>> = RefCell::new(None);
    static ETH_ZK_FINALIZED_VERIFY_RESULT: RefCell<Option<bool>> = RefCell::new(None);
    static SOLANA_FINALIZED_VERIFY_RESULT: RefCell<Option<bool>> = RefCell::new(None);
    static SUBSTRATE_FINALIZED_VERIFY_RESULT: RefCell<Option<bool>> = RefCell::new(None);
}

pub struct MockAuxiliaryDigestHandler;

impl AuxiliaryDigestHandler for MockAuxiliaryDigestHandler {
    fn add_item(item: AuxiliaryDigestItem) {
        AUX_DIGEST_ITEMS.with(|v| v.borrow_mut().push(item));
    }
}

pub fn take_aux_digest_items() -> Vec<AuxiliaryDigestItem> {
    AUX_DIGEST_ITEMS.with(|v| core::mem::take(&mut *v.borrow_mut()))
}

pub struct MockEthFinalizedStateProvider;

impl sccp::EthFinalizedStateProvider for MockEthFinalizedStateProvider {
    fn latest_finalized_state() -> Option<(H256, H256)> {
        ETH_FINALIZED_STATE.with(|s| *s.borrow())
    }
}

pub struct MockEthFinalizedBurnProofVerifier;

impl sccp::EthFinalizedBurnProofVerifier for MockEthFinalizedBurnProofVerifier {
    fn is_available() -> bool {
        ETH_FINALIZED_VERIFY_RESULT.with(|v| v.borrow().is_some())
    }

    fn verify_finalized_burn(
        message_id: H256,
        payload: &sccp::BurnPayloadV1,
        proof: &[u8],
    ) -> Option<bool> {
        let Some(decoded) = sccp::decode_eth_finalized_burn_proof_v1(proof) else {
            return Some(false);
        };
        if decoded.version != sccp::ETH_FINALIZED_RECEIPT_BURN_PROOF_VERSION_V1 {
            return Some(false);
        }
        let mut preimage = sccp::SCCP_MSG_PREFIX_BURN_V1.to_vec();
        preimage.extend(payload.encode());
        if message_id != H256::from_slice(&sp_io::hashing::keccak_256(&preimage)) {
            return Some(false);
        }
        ETH_FINALIZED_VERIFY_RESULT.with(|v| *v.borrow())
    }
}

pub fn set_eth_finalized_verify_result(result: Option<bool>) {
    ETH_FINALIZED_VERIFY_RESULT.with(|v| *v.borrow_mut() = result);
}

pub struct MockEthZkFinalizedBurnProofVerifier;

impl sccp::EthZkFinalizedBurnProofVerifier for MockEthZkFinalizedBurnProofVerifier {
    fn is_available() -> bool {
        ETH_ZK_FINALIZED_VERIFY_RESULT.with(|v| v.borrow().is_some())
    }

    fn verify_finalized_burn(message_id: H256, proof: &[u8]) -> Option<bool> {
        let Some(decoded) = sccp::decode_eth_zk_finalized_burn_proof_v1(proof) else {
            return Some(false);
        };
        if decoded.public_inputs.message_id != message_id {
            return Some(false);
        }
        let Some(router_address) = sccp::Pallet::<Runtime>::domain_endpoint(sccp::SCCP_DOMAIN_ETH)
        else {
            return Some(false);
        };
        if decoded.public_inputs.router_address.as_slice() != router_address.as_slice() {
            return Some(false);
        }
        if decoded.public_inputs.burn_storage_key
            != sccp::evm_burn_storage_key_for_message_id(message_id)
        {
            return Some(false);
        }
        ETH_ZK_FINALIZED_VERIFY_RESULT.with(|v| *v.borrow())
    }
}

pub fn set_eth_zk_finalized_verify_result(result: Option<bool>) {
    ETH_ZK_FINALIZED_VERIFY_RESULT.with(|v| *v.borrow_mut() = result);
}

pub struct MockSolanaFinalizedBurnProofVerifier;

impl sccp::SolanaFinalizedBurnProofVerifier for MockSolanaFinalizedBurnProofVerifier {
    fn is_available() -> bool {
        SOLANA_FINALIZED_VERIFY_RESULT.with(|v| v.borrow().is_some())
    }

    fn verify_finalized_burn(message_id: H256, proof: &[u8]) -> Option<bool> {
        let Some(decoded) = sccp::decode_solana_finalized_burn_proof_v1(proof) else {
            return Some(false);
        };
        if decoded.public_inputs.message_id != message_id {
            return Some(false);
        }
        SOLANA_FINALIZED_VERIFY_RESULT.with(|v| *v.borrow())
    }
}

pub fn set_solana_finalized_verify_result(result: Option<bool>) {
    SOLANA_FINALIZED_VERIFY_RESULT.with(|v| *v.borrow_mut() = result);
}

pub struct MockSubstrateFinalizedBurnProofVerifier;

impl sccp::SubstrateFinalizedBurnProofVerifier for MockSubstrateFinalizedBurnProofVerifier {
    fn is_available(_source_domain: u32) -> bool {
        SUBSTRATE_FINALIZED_VERIFY_RESULT.with(|v| v.borrow().is_some())
    }

    fn verify_finalized_burn(
        _source_domain: u32,
        _message_id: H256,
        _proof: &[u8],
    ) -> Option<bool> {
        SUBSTRATE_FINALIZED_VERIFY_RESULT.with(|v| *v.borrow())
    }
}

pub fn set_substrate_finalized_verify_result(result: Option<bool>) {
    SUBSTRATE_FINALIZED_VERIFY_RESULT.with(|v| *v.borrow_mut() = result);
}

pub struct MockLegacyBridgeChecker;

impl sccp::LegacyBridgeAssetChecker<AssetId> for MockLegacyBridgeChecker {
    fn is_legacy_bridge_asset(asset_id: &AssetId) -> bool {
        LEGACY_BRIDGE_ASSETS.with(|s| s.borrow().contains(asset_id))
    }
}

pub fn set_legacy_bridge_asset(asset_id: AssetId, present: bool) {
    LEGACY_BRIDGE_ASSETS.with(|s| {
        if present {
            s.borrow_mut().insert(asset_id);
        } else {
            s.borrow_mut().remove(&asset_id);
        }
    })
}

pub fn alice() -> AccountId {
    AccountId32::from([1; 32])
}

pub struct ExtBuilder {
    endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
    required_domains: Option<sp_runtime::BoundedVec<u32, SccpMaxDomains>>,
}

impl Default for ExtBuilder {
    fn default() -> Self {
        Self {
            endowed_accounts: vec![(alice(), GetBaseAssetId::get(), 1u32.into())],
            required_domains: None,
        }
    }
}

impl ExtBuilder {
    pub fn with_required_domains(mut self, domains: Vec<u32>) -> Self {
        self.required_domains = Some(domains.try_into().expect("required domains fit bound"));
        self
    }

    pub fn build(self) -> sp_io::TestExternalities {
        let mut t = frame_system::GenesisConfig::<Runtime>::default()
            .build_storage()
            .unwrap();

        // Reset thread-local "legacy bridge" state between tests.
        LEGACY_BRIDGE_ASSETS.with(|s| s.borrow_mut().clear());
        AUX_DIGEST_ITEMS.with(|v| v.borrow_mut().clear());
        ETH_FINALIZED_STATE.with(|s| *s.borrow_mut() = None);
        SOLANA_FINALIZED_VERIFY_RESULT.with(|v| *v.borrow_mut() = None);
        SUBSTRATE_FINALIZED_VERIFY_RESULT.with(|v| *v.borrow_mut() = None);

        pallet_balances::GenesisConfig::<Runtime> {
            balances: self
                .endowed_accounts
                .iter()
                .map(|(acc, _, balance)| (acc.clone(), *balance))
                .collect(),
            dev_accounts: None,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        permissions::GenesisConfig::<Runtime> {
            initial_permission_owners: vec![],
            initial_permissions: vec![],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        tokens::GenesisConfig::<Runtime> {
            balances: self.endowed_accounts,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        // Ensure SCCP defaults (grace period + required domains) are initialized.
        let mut sccp_genesis = sccp::GenesisConfig::<Runtime>::default();
        if let Some(required_domains) = self.required_domains {
            sccp_genesis.required_domains = required_domains;
        }
        sccp_genesis.assimilate_storage(&mut t).unwrap();

        t.into()
    }
}
