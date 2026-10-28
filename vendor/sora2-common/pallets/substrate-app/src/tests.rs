// This file is part of the SORA network and Polkaswap app.

// Copyright (c) 2020, 2021, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

// Redistribution and use in source and binary forms, with or without modification,
// are permitted provided that the following conditions are met:

// Redistributions of source code must retain the above copyright notice, this list
// of conditions and the following disclaimer.
// Redistributions in binary form must reproduce the above copyright notice, this
// list of conditions and the following disclaimer in the documentation and/or other
// materials provided with the distribution.
//
// All advertising materials mentioning features or use of this software must display
// the following acknowledgement: This product includes software developed by Polka Biome
// Ltd., SORA, and Polkaswap.
//
// Neither the name of the Polka Biome Ltd. nor the names of its contributors may be used
// to endorse or promote products derived from this software without specific prior written permission.

// THIS SOFTWARE IS PROVIDED BY Polka Biome Ltd. AS IS AND ANY EXPRESS OR IMPLIED WARRANTIES,
// INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL Polka Biome Ltd. BE LIABLE FOR ANY
// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
// BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS;
// OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
// USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

use crate::mock::{new_tester, new_tester_no_registered_assets, Currencies, RuntimeOrigin};
use crate::mock::{AssetId, SubstrateApp, System, Test};
use crate::Error;
use bridge_types::test_utils::BridgeAssetLockerImpl;
use bridge_types::traits::BridgeOriginOutput;
use bridge_types::types::AssetKind;
use bridge_types::SubNetworkId;
use bridge_types::{GenericAccount, GenericAssetId, GenericBalance};
use frame_support::{assert_noop, assert_ok};
use frame_system::Origin;
use sp_core::H256;
use sp_keyring::sr25519::Keyring;
use traits::MultiCurrency;

#[test]
fn it_works_deposit_event_mint_not_registered() {
    new_tester().execute_with(|| {
        let origin_liberland: RuntimeOrigin = dispatch::RawOrigin::new(BridgeOriginOutput::new(
            SubNetworkId::Liberland,
            H256([0; 32]),
            bridge_types::GenericTimepoint::Unknown,
            (),
        ))
        .into();
        let asset_id = AssetId::Custom(1);
        let sender = bridge_types::GenericAccount::Sora(Keyring::Alice.into());
        let recipient: <Test as frame_system::Config>::AccountId = Keyring::Alice.into();
        let amount = 1_000_000_000_000_000_000;

        assert_ok!(SubstrateApp::mint(
            origin_liberland,
            asset_id,
            sender,
            recipient,
            GenericBalance::Substrate(amount),
        ));
        assert!(System::events().iter().any(|r| r.event
            == crate::mock::RuntimeEvent::SubstrateApp(crate::Event::FailedToMint(
                H256([0; 32]),
                Error::<Test>::TokenIsNotRegistered.into()
            ))));
    });
}

#[test]
fn it_fails_mint_no_precision() {
    new_tester().execute_with(|| {
        let origin_liberland: RuntimeOrigin = dispatch::RawOrigin::new(BridgeOriginOutput::new(
            SubNetworkId::Liberland,
            H256([0; 32]),
            bridge_types::GenericTimepoint::Unknown,
            (),
        ))
        .into();
        let asset_id = AssetId::Custom(1);
        let sender = bridge_types::GenericAccount::Sora(Keyring::Alice.into());
        let recipient: <Test as frame_system::Config>::AccountId = Keyring::Alice.into();
        let amount = 1_000_000_000_000_000_000;

        crate::AssetKinds::<Test>::insert(SubNetworkId::Liberland, asset_id, AssetKind::Thischain);

        assert_ok!(SubstrateApp::mint(
            origin_liberland,
            asset_id,
            sender,
            recipient,
            GenericBalance::Substrate(amount),
        ));
        assert!(System::events().iter().any(|r| r.event
            == crate::mock::RuntimeEvent::SubstrateApp(crate::Event::FailedToMint(
                H256([0; 32]),
                Error::<Test>::UnknownPrecision.into()
            ))));
    });
}

#[test]
fn it_fails_mint_wrong_amount() {
    new_tester().execute_with(|| {
        let origin_liberland: RuntimeOrigin = dispatch::RawOrigin::new(BridgeOriginOutput::new(
            SubNetworkId::Liberland,
            H256([0; 32]),
            bridge_types::GenericTimepoint::Unknown,
            (),
        ))
        .into();
        let asset_id = AssetId::Xor;
        let sender = bridge_types::GenericAccount::Sora(Keyring::Alice.into());
        let recipient: <Test as frame_system::Config>::AccountId = Keyring::Alice.into();
        let amount = 0;

        crate::AssetKinds::<Test>::insert(SubNetworkId::Liberland, asset_id, AssetKind::Thischain);

        assert_ok!(SubstrateApp::mint(
            origin_liberland,
            asset_id,
            sender,
            recipient,
            GenericBalance::Substrate(amount),
        ));
        assert!(System::events().iter().any(|r| r.event
            == crate::mock::RuntimeEvent::SubstrateApp(crate::Event::FailedToMint(
                H256([0; 32]),
                Error::<Test>::WrongAmount.into()
            ))));
    });
}

#[test]
fn it_works_burn() {
    new_tester().execute_with(|| {
        let origin = Origin::<Test>::Signed(Keyring::Alice.into());
        let network_id = SubNetworkId::Liberland;
        let amount = 1_000_000;

        // send XOR
        assert_ok!(SubstrateApp::burn(
            origin.into(),
            network_id,
            AssetId::Xor,
            GenericAccount::Sora(Keyring::Alice.into()),
            amount
        ));

        let bridge_acc = BridgeAssetLockerImpl::<Currencies>::bridge_account(network_id.into());
        assert_eq!(Currencies::total_balance(AssetId::Xor, &bridge_acc), amount);
        assert_eq!(
            Currencies::total_balance(AssetId::Xor, &Keyring::Alice.into()),
            1_000_000_000_000_000_000 - amount
        );
    });
}

#[test]
fn it_fails_burn_token_not_registered() {
    new_tester().execute_with(|| {
        let origin = Origin::<Test>::Signed(Keyring::Alice.into());
        let network_id = SubNetworkId::Liberland;
        let amount = 1_000_000;

        // send XOR
        assert_noop!(
            SubstrateApp::burn(
                origin.into(),
                network_id,
                AssetId::Eth,
                GenericAccount::Sora(Keyring::Alice.into()),
                amount
            ),
            Error::<Test>::TokenIsNotRegistered
        );
    });
}

#[test]
fn it_fails_burn_unknown_presicion() {
    new_tester().execute_with(|| {
        let origin = Origin::<Test>::Signed(Keyring::Alice.into());
        let network_id = SubNetworkId::Liberland;
        let amount = 1_000_000;
        let asset_id = AssetId::Dai;

        crate::AssetKinds::<Test>::insert(SubNetworkId::Liberland, asset_id, AssetKind::Thischain);

        assert_noop!(
            SubstrateApp::burn(
                origin.into(),
                network_id,
                asset_id,
                GenericAccount::Sora(Keyring::Alice.into()),
                amount
            ),
            Error::<Test>::UnknownPrecision
        );
    });
}

#[test]
fn it_works_register_sidechain_asset() {
    new_tester_no_registered_assets().execute_with(|| {
        let origin = Origin::<Test>::Root;
        let network_id = SubNetworkId::Mainnet;
        let symbol = "LLD";
        let name = "LLD";
        let asset_id = GenericAssetId::Liberland(bridge_types::LiberlandAssetId::LLD);

        assert_ok!(SubstrateApp::register_sidechain_asset(
            origin.into(),
            network_id,
            asset_id,
            symbol.into(),
            name.into(),
        ));
    });
}

#[test]
fn it_works_incoming_thischain_asset_registration() {
    new_tester_no_registered_assets().execute_with(|| {
        let origin_liberland: RuntimeOrigin = dispatch::RawOrigin::new(BridgeOriginOutput::new(
            SubNetworkId::Liberland,
            H256([0; 32]),
            bridge_types::GenericTimepoint::Unknown,
            (),
        ))
        .into();
        let asset_id = AssetId::Xor;
        let generic_asset_id = GenericAssetId::Sora(H256([0; 32]));

        assert_ok!(SubstrateApp::incoming_thischain_asset_registration(
            origin_liberland,
            asset_id,
            generic_asset_id,
        ));
    });
}

#[test]
fn it_works_finalize_asset_registration() {
    new_tester_no_registered_assets().execute_with(|| {
        let origin_liberland: RuntimeOrigin = dispatch::RawOrigin::new(BridgeOriginOutput::new(
            SubNetworkId::Liberland,
            H256([0; 32]),
            bridge_types::GenericTimepoint::Unknown,
            (),
        ))
        .into();
        let asset_id = AssetId::Xor;
        let generic_asset_id = GenericAssetId::Sora(H256([0; 32]));

        assert_ok!(SubstrateApp::finalize_asset_registration(
            origin_liberland,
            asset_id,
            generic_asset_id,
            AssetKind::Thischain,
            12,
        ));
    });
}

#[test]
fn mint_failure_reports_transport_success_but_preserves_failed_application_outcome() {
    new_tester().execute_with(|| {
        let origin: RuntimeOrigin = dispatch::RawOrigin::new(BridgeOriginOutput::new(
            SubNetworkId::Liberland,
            H256([7; 32]),
            bridge_types::GenericTimepoint::Unknown,
            (),
        ))
        .into();
        let outcome = SubstrateApp::mint_with_outcome(
            origin,
            AssetId::Custom(1),
            GenericAccount::Sora(Keyring::Alice.into()),
            Keyring::Alice.into(),
            GenericBalance::Substrate(1_000_000_000_000_000_000),
        )
        .unwrap();
        assert_eq!(
            outcome,
            bridge_types::traits::MessageDispatchOutcome::Failed
        );
        assert!(System::events().iter().any(|event| event.event
            == crate::mock::RuntimeEvent::SubstrateApp(crate::Event::FailedToMint(
                H256([7; 32]),
                Error::<Test>::TokenIsNotRegistered.into()
            ))));
    });
}

#[test]
fn recovery_mint_journals_ack_without_outbound_nonce_or_duplicate_credit() {
    new_tester().execute_with(|| {
        use bridge_types::traits::MessageDispatchOutcome;
        use bridge_types::types::MessageStatus;
        let network = SubNetworkId::Liberland;
        let id = H256::repeat_byte(19);
        let recipient = Keyring::Bob.to_account_id();
        let origin = || -> RuntimeOrigin {
            dispatch::RawOrigin::new(BridgeOriginOutput::new(
                network,
                id,
                bridge_types::GenericTimepoint::Unknown,
                (),
            ))
            .into()
        };
        crate::AssetKinds::<Test>::insert(network, AssetId::Xor, AssetKind::Sidechain);
        crate::SidechainPrecision::<Test>::insert(network, AssetId::Xor, 18);
        sp_io::storage::set(crate::recovery::HALT_KEY, &[1]);
        let before = Currencies::total_balance(AssetId::Xor, &recipient);
        let mint = || {
            SubstrateApp::mint_with_outcome(
                origin(),
                AssetId::Xor,
                GenericAccount::Sora(Keyring::Alice.into()),
                recipient.clone(),
                GenericBalance::Substrate(1_000_000),
            )
        };
        assert_noop!(mint(), Error::<Test>::RecoveryHalted);
        for context in [
            vec![],
            vec![1],
            0u32.to_le_bytes().to_vec(),
            [1u32.to_le_bytes().as_slice(), &[0]].concat(),
        ] {
            sp_io::storage::set(crate::recovery::INBOUND_CONTEXT_KEY, &context);
            assert_noop!(mint(), Error::<Test>::RecoveryHalted);
        }
        sp_io::storage::set(crate::recovery::INBOUND_CONTEXT_KEY, &1u32.to_le_bytes());
        assert_eq!(mint().unwrap(), MessageDispatchOutcome::Applied);
        assert_eq!(
            Currencies::total_balance(AssetId::Xor, &recipient),
            before + 100_000 // Fixture conversion preserves the native 10:1 precision ratio.
        );
        assert_eq!(
            crate::DeferredTransferResults::<Test>::get(network, id),
            Some(MessageStatus::Done)
        );
        assert_eq!(
            substrate_bridge_channel::outbound::ChannelNonces::<Test>::get(network),
            0
        );
        assert_noop!(mint(), Error::<Test>::DeferredResultExists);
        assert_noop!(
            SubstrateApp::update_transaction_status(origin(), id, MessageStatus::Failed),
            Error::<Test>::RecoveryHalted
        );
        sp_io::storage::clear(crate::recovery::INBOUND_CONTEXT_KEY);
        assert!(!crate::recovery::inbound_allowed::<Test>());
    });
}

#[test]
fn recovery_failed_mint_rolls_back_partial_credit_and_journals_failed_result() {
    new_tester().execute_with(|| {
        let network = SubNetworkId::Liberland;
        let id = H256::repeat_byte(20);
        let recipient = Keyring::Bob.to_account_id();
        let origin: RuntimeOrigin = dispatch::RawOrigin::new(BridgeOriginOutput::new(
            network,
            id,
            bridge_types::GenericTimepoint::Unknown,
            (),
        ))
        .into();
        crate::AssetKinds::<Test>::insert(network, AssetId::Xor, AssetKind::Sidechain);
        crate::SidechainPrecision::<Test>::insert(network, AssetId::Xor, 18);
        sp_io::storage::set(crate::recovery::HALT_KEY, &[1]);
        sp_io::storage::set(crate::recovery::INBOUND_CONTEXT_KEY, &1u32.to_le_bytes());
        sp_io::storage::set(b"test:fail-after-recovery-credit", &[1]);
        let balance = Currencies::total_balance(AssetId::Xor, &recipient);
        let issuance = Currencies::total_issuance(AssetId::Xor);
        let outcome = SubstrateApp::mint_with_outcome(
            origin,
            AssetId::Xor,
            GenericAccount::Sora(Keyring::Alice.into()),
            recipient.clone(),
            GenericBalance::Substrate(1_000_000),
        )
        .unwrap();
        assert_eq!(
            outcome,
            bridge_types::traits::MessageDispatchOutcome::Failed
        );
        assert_eq!(Currencies::total_balance(AssetId::Xor, &recipient), balance);
        assert_eq!(Currencies::total_issuance(AssetId::Xor), issuance);
        assert!(!sp_io::storage::exists(b"test:partial-recovery-credit"));
        assert_eq!(
            crate::DeferredTransferResults::<Test>::get(network, id),
            Some(bridge_types::types::MessageStatus::Failed)
        );
        assert_eq!(
            substrate_bridge_channel::outbound::ChannelNonces::<Test>::get(network),
            0
        );
    });
}

#[test]
fn recovery_chunk_failure_rolls_back_successful_credit_and_deferred_ack_together() {
    new_tester().execute_with(|| {
        let network = SubNetworkId::Liberland;
        let id = H256::repeat_byte(21);
        crate::AssetKinds::<Test>::insert(network, AssetId::Xor, AssetKind::Sidechain);
        crate::SidechainPrecision::<Test>::insert(network, AssetId::Xor, 18);
        sp_io::storage::set(crate::recovery::HALT_KEY, &[1]);
        sp_io::storage::set(crate::recovery::INBOUND_CONTEXT_KEY, &1u32.to_le_bytes());
        let root = sp_io::storage::root(sp_runtime::StateVersion::V1);
        let result: Result<(), sp_runtime::DispatchError> =
            frame_support::storage::with_storage_layer(|| {
                let origin: RuntimeOrigin = dispatch::RawOrigin::new(BridgeOriginOutput::new(
                    network,
                    id,
                    bridge_types::GenericTimepoint::Unknown,
                    (),
                ))
                .into();
                SubstrateApp::mint_with_outcome(
                    origin,
                    AssetId::Xor,
                    GenericAccount::Sora(Keyring::Alice.into()),
                    Keyring::Bob.into(),
                    GenericBalance::Substrate(1_000_000),
                )?;
                assert!(crate::DeferredTransferResults::<Test>::contains_key(
                    network, id
                ));
                Err(sp_runtime::DispatchError::Other(
                    "later recovery invariant failed",
                ))
            });
        assert!(result.is_err());
        assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), root);
        assert!(!crate::DeferredTransferResults::<Test>::contains_key(
            network, id
        ));
    });
}
