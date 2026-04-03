// This file is part of the SORA network and Polkaswap app.
//
// Copyright (c) 2026, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

use crate as sccp;
use bridge_types::traits::AuxiliaryDigestHandler;
use bridge_types::types::AuxiliaryDigestItem;
use common::prelude::Balance;
use common::{
    mock_assets_config, mock_common_config, mock_currencies_config, mock_frame_system_config,
    mock_pallet_balances_config, mock_permissions_config, mock_technical_config,
    mock_tokens_config, DEXId, XST,
};
use currencies::BasicCurrencyAdapter;
use frame_support::weights::Weight;
use frame_support::{construct_runtime, parameter_types};
use orml_traits::parameter_type_with_key;
use sp_core::crypto::AccountId32;
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
    type AccountIdConverter = AccountId32Converter;
    type AssetInfoProvider = Assets;
    type LegacyBridgeAssetChecker = MockLegacyBridgeChecker;
    type AuxiliaryDigestHandler = MockAuxiliaryDigestHandler;
    type NexusSccpBurnProofVerifier = MockNexusBurnProofVerifier;
    type NexusSccpGovernanceProofVerifier = MockNexusGovernanceProofVerifier;
    type WeightInfo = ();
}

thread_local! {
    static LEGACY_BRIDGE_ASSETS: RefCell<BTreeSet<AssetId>> = RefCell::new(BTreeSet::new());
    static AUX_DIGEST_ITEMS: RefCell<Vec<AuxiliaryDigestItem>> = RefCell::new(Vec::new());
    static NEXUS_BURN_VERIFY_RESULT: RefCell<Option<sccp::VerifiedBurnProof>> = RefCell::new(None);
    static NEXUS_GOVERNANCE_VERIFY_RESULT: RefCell<Option<sccp::VerifiedGovernanceProof>> = RefCell::new(None);
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

pub struct MockNexusBurnProofVerifier;

impl sccp::NexusSccpBurnProofVerifier for MockNexusBurnProofVerifier {
    fn is_available() -> bool {
        NEXUS_BURN_VERIFY_RESULT.with(|v| v.borrow().is_some())
    }

    fn verify_burn_proof(_proof: &[u8]) -> Option<sccp::VerifiedBurnProof> {
        NEXUS_BURN_VERIFY_RESULT.with(|v| v.borrow().clone())
    }
}

pub fn set_nexus_burn_verify_result(result: Option<sccp::VerifiedBurnProof>) {
    NEXUS_BURN_VERIFY_RESULT.with(|v| *v.borrow_mut() = result);
}

pub struct MockNexusGovernanceProofVerifier;

impl sccp::NexusSccpGovernanceProofVerifier for MockNexusGovernanceProofVerifier {
    fn is_available() -> bool {
        NEXUS_GOVERNANCE_VERIFY_RESULT.with(|v| v.borrow().is_some())
    }

    fn verify_governance_proof(_proof: &[u8]) -> Option<sccp::VerifiedGovernanceProof> {
        NEXUS_GOVERNANCE_VERIFY_RESULT.with(|v| v.borrow().clone())
    }
}

pub fn set_nexus_governance_verify_result(result: Option<sccp::VerifiedGovernanceProof>) {
    NEXUS_GOVERNANCE_VERIFY_RESULT.with(|v| *v.borrow_mut() = result);
}

pub struct MockLegacyBridgeChecker;

impl sccp::LegacyBridgeAssetChecker<AssetId> for MockLegacyBridgeChecker {
    fn is_legacy_bridge_asset(asset_id: &AssetId) -> bool {
        LEGACY_BRIDGE_ASSETS.with(|s| s.borrow().contains(asset_id))
    }
}

pub fn alice() -> AccountId {
    AccountId32::from([1; 32])
}

pub struct ExtBuilder {
    endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
}

impl Default for ExtBuilder {
    fn default() -> Self {
        Self {
            endowed_accounts: vec![(alice(), GetBaseAssetId::get(), 1u32.into())],
        }
    }
}

impl ExtBuilder {
    pub fn build(self) -> sp_io::TestExternalities {
        let mut t = frame_system::GenesisConfig::<Runtime>::default()
            .build_storage()
            .unwrap();

        LEGACY_BRIDGE_ASSETS.with(|s| s.borrow_mut().clear());
        AUX_DIGEST_ITEMS.with(|v| v.borrow_mut().clear());
        NEXUS_BURN_VERIFY_RESULT.with(|v| *v.borrow_mut() = None);
        NEXUS_GOVERNANCE_VERIFY_RESULT.with(|v| *v.borrow_mut() = None);

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

        t.into()
    }
}
