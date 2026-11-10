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

use crate::mock::RuntimeCall;
use crate::mock::RuntimeEvent;
use crate::mock::{
    new_tester, AccountId, BridgeOutboundChannel, BridgeProxy, Currencies, Dispatch, FungibleApp,
    System, Test, BASE_EVM_NETWORK_ID,
};
use crate::{BridgeRequest, Transactions};
use bridge_types::traits::MessageDispatch;
use bridge_types::GenericTimepoint;
use bridge_types::H160;
use bridge_types::{GenericAccount, GenericNetworkId, SubNetworkId};
use codec::Encode;
use common::{balance, FixedInner, OnDenominate, DAI, TBCD, XOR};
use frame_support::assert_noop;
use frame_support::assert_ok;
use frame_support::traits::Hooks;
use frame_system::RawOrigin;
use sp_keyring::sr25519::Keyring;

use bridge_types::evm::AdditionalEVMInboundData;
use bridge_types::types::{MessageDirection, MessageId, MessageStatus};

#[path = "coverage.rs"]
mod coverage;

#[test]
fn recovery_halt_keeps_failed_outgoing_liability_quarantined() {
    use bridge_types::traits::MessageStatusNotifier;
    new_tester().execute_with(|| {
        let owner: AccountId = Keyring::Alice.into();
        let network = GenericNetworkId::EVM(BASE_EVM_NETWORK_ID);
        let id = bridge_types::H256::repeat_byte(42);
        let request = BridgeRequest {
            source: GenericAccount::Sora(owner.clone()),
            dest: GenericAccount::EVM(H160::default()),
            asset_id: XOR,
            amount: 1_000,
            status: MessageStatus::Committed,
            start_timepoint: GenericTimepoint::Sora(1),
            end_timepoint: GenericTimepoint::Pending,
            direction: MessageDirection::Outbound,
        };
        crate::Senders::<Test>::insert(network, id, owner.clone());
        Transactions::<Test>::insert((network, owner.clone()), id, request.clone());
        sp_io::storage::set(common::recovery::BRIDGE_HALT_STORAGE_KEY, &[1]);
        let root = sp_io::storage::root(sp_runtime::StateVersion::V1);
        BridgeProxy::update_status(
            network,
            id,
            MessageStatus::Failed,
            GenericTimepoint::Sora(2),
        );
        assert_eq!(
            Transactions::<Test>::get((network, owner.clone()), id),
            Some(request)
        );
        assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), root);
        assert_noop!(
            BridgeProxy::refund(network, id, GenericAccount::Sora(owner.clone()), XOR, 1_000),
            crate::Error::<Test>::RecoveryHalted
        );
        assert_noop!(
            BridgeProxy::burn(
                RawOrigin::Signed(owner).into(),
                network,
                XOR,
                GenericAccount::EVM(H160::default()),
                1_000
            ),
            crate::Error::<Test>::RecoveryHalted
        );
    });
}

fn assert_event(event: RuntimeEvent) {
    System::events()
        .iter()
        .find(|e| e.event == event)
        .expect("Event not found");
}

fn create_refundable_transfer(owner: AccountId, amount: u128) -> bridge_types::H256 {
    assert_ok!(Currencies::update_balance(
        RawOrigin::Root.into(),
        owner.clone(),
        XOR,
        amount as FixedInner,
    ));
    assert_ok!(BridgeProxy::burn(
        RawOrigin::Signed(owner.clone()).into(),
        BASE_EVM_NETWORK_ID.into(),
        XOR,
        GenericAccount::EVM(H160::repeat_byte(9)),
        amount,
    ));
    Transactions::<Test>::iter_prefix((GenericNetworkId::EVM(BASE_EVM_NETWORK_ID), owner))
        .next()
        .unwrap()
        .0
}

#[test]
fn terminal_outgoing_status_cannot_refund_again_or_regress() {
    use bridge_types::traits::MessageStatusNotifier;
    use common::AssetInfoProvider;
    for terminal in [MessageStatus::Done, MessageStatus::Refunded] {
        new_tester().execute_with(|| {
            let owner: AccountId = Keyring::Alice.into();
            let other: AccountId = Keyring::Charlie.into();
            let network = GenericNetworkId::EVM(BASE_EVM_NETWORK_ID);
            let id = create_refundable_transfer(owner.clone(), 1_000);
            // A second user's custody makes repeated refunds financially exploitable.
            create_refundable_transfer(other, 1_000);
            let first_status = if terminal == MessageStatus::Refunded {
                MessageStatus::Failed
            } else {
                MessageStatus::Done
            };
            BridgeProxy::update_status(network, id, first_status, GenericTimepoint::EVM(10));
            assert_eq!(
                Transactions::<Test>::get((network, owner.clone()), id)
                    .unwrap()
                    .status,
                terminal
            );
            let balance = assets::Pallet::<Test>::free_balance(&XOR, &owner).unwrap();
            let before = sp_io::storage::root(sp_runtime::StateVersion::V1);
            for status in [
                MessageStatus::Failed,
                MessageStatus::Committed,
                MessageStatus::InQueue,
                MessageStatus::Approved,
                MessageStatus::Done,
                MessageStatus::Refunded,
            ] {
                BridgeProxy::update_status(network, id, status, GenericTimepoint::EVM(11));
                assert_eq!(
                    assets::Pallet::<Test>::free_balance(&XOR, &owner).unwrap(),
                    balance
                );
                assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), before);
            }
        });
    }
}

#[test]
fn failed_refund_preserves_locked_assets_and_can_be_retried() {
    use bridge_types::traits::MessageStatusNotifier;
    use common::AssetInfoProvider;
    new_tester().execute_with(|| {
        let owner: AccountId = Keyring::Alice.into();
        let network = GenericNetworkId::EVM(BASE_EVM_NETWORK_ID);
        let id = create_refundable_transfer(owner.clone(), 1_000);
        let bridge_account = BridgeProxy::bridge_account(network).unwrap();
        // Simulate unavailable custody after the outbound request was admitted.
        assert_ok!(Currencies::update_balance(
            RawOrigin::Root.into(),
            bridge_account.clone(),
            XOR,
            -1_000,
        ));
        BridgeProxy::update_status(
            network,
            id,
            MessageStatus::Failed,
            GenericTimepoint::EVM(10),
        );
        assert_eq!(crate::LockedAssets::<Test>::get(network, XOR), 1_000);
        assert_eq!(
            assets::Pallet::<Test>::free_balance(&XOR, &owner).unwrap(),
            0
        );
        assert_eq!(
            Transactions::<Test>::get((network, owner.clone()), id)
                .unwrap()
                .status,
            MessageStatus::Failed
        );
        assert_event(crate::Event::RefundFailed(id).into());

        assert_ok!(Currencies::update_balance(
            RawOrigin::Root.into(),
            bridge_account,
            XOR,
            1_000,
        ));
        BridgeProxy::update_status(
            network,
            id,
            MessageStatus::Failed,
            GenericTimepoint::EVM(11),
        );
        assert_eq!(crate::LockedAssets::<Test>::get(network, XOR), 0);
        assert_eq!(
            assets::Pallet::<Test>::free_balance(&XOR, &owner).unwrap(),
            1_000
        );
        assert_eq!(
            Transactions::<Test>::get((network, owner), id)
                .unwrap()
                .status,
            MessageStatus::Refunded
        );
    });
}

#[test]
fn unsupported_refund_path_does_not_mark_request_refunded() {
    use bridge_types::traits::MessageStatusNotifier;
    new_tester().execute_with(|| {
        let owner: AccountId = Keyring::Alice.into();
        let network = GenericNetworkId::EVM(BASE_EVM_NETWORK_ID);
        let id = create_refundable_transfer(owner.clone(), 1_000);
        // Retained requests can outlive the app/asset route that handled them.
        Transactions::<Test>::mutate((network, owner.clone()), id, |request| {
            request.as_mut().unwrap().asset_id = TBCD;
        });
        BridgeProxy::update_status(
            network,
            id,
            MessageStatus::Failed,
            GenericTimepoint::EVM(10),
        );
        assert_eq!(
            Transactions::<Test>::get((network, owner), id)
                .unwrap()
                .status,
            MessageStatus::Failed
        );
        assert_eq!(crate::LockedAssets::<Test>::get(network, XOR), 1_000);
        assert_event(crate::Event::RefundFailed(id).into());
    });
}

#[test]
fn burn_successfull() {
    new_tester().execute_with(|| {
        let caller: AccountId = Keyring::Alice.into();
        assert_ok!(BridgeProxy::add_limited_asset(RawOrigin::Root.into(), XOR));
        Currencies::update_balance(
            RawOrigin::Root.into(),
            caller.clone(),
            XOR,
            balance!(1) as FixedInner,
        )
        .unwrap();
        BridgeProxy::burn(
            RawOrigin::Signed(caller.clone()).into(),
            BASE_EVM_NETWORK_ID.into(),
            XOR,
            GenericAccount::EVM(H160::default()),
            1000,
        )
        .unwrap();
        assert_eq!(
            crate::LockedAssets::<Test>::get(GenericNetworkId::EVM(BASE_EVM_NETWORK_ID), XOR),
            1000
        );
        assert_eq!(crate::ConsumedTransferLimit::<Test>::get(), 2500);
        assert_eq!(crate::TransferLimitUnlockSchedule::<Test>::get(601), 2500);

        let message_id = MessageId::batched(
            bridge_types::SubNetworkId::Mainnet.into(),
            BASE_EVM_NETWORK_ID.into(),
            1,
            0,
        )
        .hash();
        assert_eq!(
            Transactions::<Test>::get(
                (GenericNetworkId::EVM(BASE_EVM_NETWORK_ID), &caller),
                message_id,
            ),
            Some(BridgeRequest {
                source: GenericAccount::Sora(caller.clone()),
                dest: GenericAccount::EVM(H160::default()),
                asset_id: XOR,
                amount: 1000,
                status: MessageStatus::InQueue,
                start_timepoint: GenericTimepoint::Sora(1),
                end_timepoint: GenericTimepoint::Pending,
                direction: MessageDirection::Outbound,
            })
        );
        assert_event(crate::Event::RequestStatusUpdate(message_id, MessageStatus::InQueue).into());
        BridgeOutboundChannel::on_initialize(BridgeOutboundChannel::interval());
        assert_event(
            crate::Event::RequestStatusUpdate(message_id, MessageStatus::Committed).into(),
        );
        assert_eq!(
            Transactions::<Test>::get(
                (GenericNetworkId::EVM(BASE_EVM_NETWORK_ID), &caller),
                &message_id,
            ),
            Some(BridgeRequest {
                source: GenericAccount::Sora(caller.clone()),
                dest: GenericAccount::EVM(H160::default()),
                asset_id: XOR,
                amount: 1000,
                status: MessageStatus::Committed,
                start_timepoint: GenericTimepoint::Sora(1),
                end_timepoint: GenericTimepoint::Pending,
                direction: MessageDirection::Outbound,
            })
        );
    })
}

#[test]
fn burn_failed() {
    new_tester().execute_with(|| {
        let caller: AccountId = Keyring::Alice.into();
        assert_noop!(
            BridgeProxy::burn(
                RawOrigin::Signed(caller.clone()).into(),
                BASE_EVM_NETWORK_ID.into(),
                XOR,
                GenericAccount::EVM(H160::default()),
                1000,
            ),
            sp_runtime::TokenError::FundsUnavailable
        );
        assert_eq!(
            crate::LockedAssets::<Test>::get(GenericNetworkId::EVM(BASE_EVM_NETWORK_ID), XOR),
            0
        );
        assert_eq!(Transactions::<Test>::iter().count(), 0);
        assert_eq!(System::events().len(), 0);
    })
}

#[test]
fn mint_successfull() {
    new_tester().execute_with(|| {
        let recipient: AccountId = Keyring::Alice.into();
        let source = FungibleApp::app_address(BASE_EVM_NETWORK_ID).unwrap();
        let token = FungibleApp::token_address(BASE_EVM_NETWORK_ID, DAI).unwrap();
        Dispatch::dispatch(
            BASE_EVM_NETWORK_ID.into(),
            MessageId::basic(
                BASE_EVM_NETWORK_ID.into(),
                bridge_types::SubNetworkId::Mainnet.into(),
                0,
            ),
            GenericTimepoint::Parachain(1),
            &RuntimeCall::FungibleApp(evm_fungible_app::Call::mint {
                token,
                sender: Default::default(),
                recipient: recipient.clone(),
                amount: 1000u64.into(),
            })
            .encode(),
            AdditionalEVMInboundData { source }.into(),
        );
        assert_eq!(
            crate::LockedAssets::<Test>::get(GenericNetworkId::EVM(BASE_EVM_NETWORK_ID), DAI),
            1000
        );
        let message_id = MessageId::basic(
            BASE_EVM_NETWORK_ID.into(),
            bridge_types::SubNetworkId::Mainnet.into(),
            0,
        )
        .hash();
        assert_eq!(
            Transactions::<Test>::get(
                (GenericNetworkId::EVM(BASE_EVM_NETWORK_ID), &recipient),
                message_id
            ),
            Some(BridgeRequest {
                source: GenericAccount::EVM(H160::default()),
                dest: GenericAccount::Sora(recipient.clone()),
                asset_id: DAI,
                amount: 1000,
                status: MessageStatus::Done,
                start_timepoint: GenericTimepoint::Parachain(1),
                end_timepoint: GenericTimepoint::Sora(1),
                direction: MessageDirection::Inbound,
            })
        );
        assert_event(crate::Event::RequestStatusUpdate(message_id, MessageStatus::Done).into());
    })
}

#[test]
fn mint_failed() {
    new_tester().execute_with(|| {
        let recipient: AccountId = Keyring::Alice.into();
        let source = FungibleApp::app_address(BASE_EVM_NETWORK_ID).unwrap();
        let token = FungibleApp::token_address(BASE_EVM_NETWORK_ID, XOR).unwrap();
        Dispatch::dispatch(
            BASE_EVM_NETWORK_ID.into(),
            MessageId::basic(
                BASE_EVM_NETWORK_ID.into(),
                bridge_types::SubNetworkId::Mainnet.into(),
                0,
            ),
            Default::default(),
            &RuntimeCall::FungibleApp(evm_fungible_app::Call::mint {
                token,
                sender: Default::default(),
                recipient: recipient.clone(),
                amount: 1000u64.into(),
            })
            .encode(),
            AdditionalEVMInboundData { source }.into(),
        );
        assert_eq!(
            crate::LockedAssets::<Test>::get(GenericNetworkId::EVM(BASE_EVM_NETWORK_ID), XOR),
            0
        );
        assert_eq!(Transactions::<Test>::iter().count(), 0);
        assert_eq!(System::events().len(), 1);
    })
}

#[test]
fn mint_not_enough_locked() {
    new_tester().execute_with(|| {
        let recipient: AccountId = Keyring::Alice.into();
        let source = FungibleApp::app_address(BASE_EVM_NETWORK_ID).unwrap();
        let token = FungibleApp::token_address(BASE_EVM_NETWORK_ID, XOR).unwrap();
        Dispatch::dispatch(
            BASE_EVM_NETWORK_ID.into(),
            MessageId::basic(
                BASE_EVM_NETWORK_ID.into(),
                bridge_types::SubNetworkId::Mainnet.into(),
                0,
            ),
            GenericTimepoint::Parachain(1),
            &RuntimeCall::FungibleApp(evm_fungible_app::Call::mint {
                token,
                sender: Default::default(),
                recipient: recipient.clone(),
                amount: 1000u64.into(),
            })
            .encode(),
            AdditionalEVMInboundData { source }.into(),
        );
        assert_eq!(
            crate::LockedAssets::<Test>::get(GenericNetworkId::EVM(BASE_EVM_NETWORK_ID), XOR),
            0
        );
        assert_event(
            dispatch::Event::<Test>::MessageDispatched(
                MessageId::basic(
                    BASE_EVM_NETWORK_ID.into(),
                    bridge_types::SubNetworkId::Mainnet.into(),
                    0,
                ),
                Err(crate::Error::<Test>::NotEnoughLockedLiquidity.into()),
            )
            .into(),
        );
    })
}

#[test]
fn burn_no_enough_locked() {
    new_tester().execute_with(|| {
        let caller: AccountId = Keyring::Alice.into();
        Currencies::update_balance(
            RawOrigin::Root.into(),
            caller.clone(),
            DAI,
            balance!(1) as FixedInner,
        )
        .unwrap();
        assert_noop!(
            BridgeProxy::burn(
                RawOrigin::Signed(caller.clone()).into(),
                BASE_EVM_NETWORK_ID.into(),
                DAI,
                GenericAccount::EVM(H160::default()),
                1000,
            ),
            crate::Error::<Test>::NotEnoughLockedLiquidity
        );
        assert_eq!(
            crate::LockedAssets::<Test>::get(GenericNetworkId::EVM(BASE_EVM_NETWORK_ID), DAI),
            0
        );
    })
}

#[test]
fn denominate_scales_only_legacy_bridge_xor_locked_assets() {
    new_tester().execute_with(|| {
        let legacy_evm = GenericNetworkId::EVMLegacy(0);
        let kusama = GenericNetworkId::Sub(SubNetworkId::Kusama);
        let polkadot = GenericNetworkId::Sub(SubNetworkId::Polkadot);
        let current_evm = GenericNetworkId::EVM(BASE_EVM_NETWORK_ID);
        let substrate_mainnet = GenericNetworkId::Sub(SubNetworkId::Mainnet);

        crate::LockedAssets::<Test>::insert(legacy_evm, XOR, balance!(100));
        crate::LockedAssets::<Test>::insert(kusama, XOR, balance!(200));
        crate::LockedAssets::<Test>::insert(polkadot, XOR, balance!(300));
        crate::LockedAssets::<Test>::insert(current_evm, XOR, balance!(400));
        crate::LockedAssets::<Test>::insert(substrate_mainnet, XOR, balance!(500));
        crate::LockedAssets::<Test>::insert(legacy_evm, TBCD, balance!(30));
        crate::LockedAssets::<Test>::insert(legacy_evm, DAI, balance!(50));

        assert_ok!(BridgeProxy::on_denominate(&10));

        assert_eq!(
            crate::LockedAssets::<Test>::get(legacy_evm, XOR),
            balance!(10)
        );
        assert_eq!(crate::LockedAssets::<Test>::get(kusama, XOR), balance!(20));
        assert_eq!(
            crate::LockedAssets::<Test>::get(polkadot, XOR),
            balance!(30)
        );
        assert_eq!(
            crate::LockedAssets::<Test>::get(current_evm, XOR),
            balance!(400)
        );
        assert_eq!(
            crate::LockedAssets::<Test>::get(substrate_mainnet, XOR),
            balance!(500)
        );
        assert_eq!(
            crate::LockedAssets::<Test>::get(legacy_evm, TBCD),
            balance!(30)
        );
        assert_eq!(
            crate::LockedAssets::<Test>::get(legacy_evm, DAI),
            balance!(50)
        );
    })
}

#[test]
fn denominate_zero_factor_keeps_locked_assets_unchanged() {
    new_tester().execute_with(|| {
        let legacy_evm = GenericNetworkId::EVMLegacy(0);
        let current_evm = GenericNetworkId::EVM(BASE_EVM_NETWORK_ID);
        crate::LockedAssets::<Test>::insert(legacy_evm, XOR, balance!(100));
        crate::LockedAssets::<Test>::insert(current_evm, XOR, balance!(200));
        crate::LockedAssets::<Test>::insert(legacy_evm, TBCD, balance!(30));
        crate::LockedAssets::<Test>::insert(legacy_evm, DAI, balance!(50));

        assert_noop!(
            BridgeProxy::on_denominate(&0),
            sp_runtime::DispatchError::Arithmetic(sp_runtime::ArithmeticError::DivisionByZero)
        );

        assert_eq!(
            crate::LockedAssets::<Test>::get(legacy_evm, XOR),
            balance!(100)
        );
        assert_eq!(
            crate::LockedAssets::<Test>::get(current_evm, XOR),
            balance!(200)
        );
        assert_eq!(
            crate::LockedAssets::<Test>::get(legacy_evm, TBCD),
            balance!(30)
        );
        assert_eq!(
            crate::LockedAssets::<Test>::get(legacy_evm, DAI),
            balance!(50)
        );
    })
}

#[test]
fn add_remove_limited_asset_works() {
    new_tester().execute_with(|| {
        assert!(!crate::LimitedAssets::<Test>::get(DAI));

        assert_ok!(BridgeProxy::add_limited_asset(RawOrigin::Root.into(), DAI));
        assert!(crate::LimitedAssets::<Test>::get(DAI));

        assert_noop!(
            BridgeProxy::add_limited_asset(RawOrigin::Root.into(), DAI),
            crate::Error::<Test>::AssetAlreadyLimited
        );
        assert!(crate::LimitedAssets::<Test>::get(DAI));

        assert_ok!(BridgeProxy::remove_limited_asset(
            RawOrigin::Root.into(),
            DAI
        ));
        assert!(!crate::LimitedAssets::<Test>::get(DAI));

        assert_noop!(
            BridgeProxy::remove_limited_asset(RawOrigin::Root.into(), DAI),
            crate::Error::<Test>::AssetNotLimited
        );
        assert!(!crate::LimitedAssets::<Test>::get(DAI));
    })
}

#[test]
fn update_transfer_limit_works() {
    new_tester().execute_with(|| {
        let settings = crate::TransferLimitSettings {
            max_amount: 1000,
            period_blocks: 100u32.into(),
        };
        assert_ok!(BridgeProxy::update_transfer_limit(
            RawOrigin::Root.into(),
            settings.clone()
        ));
        assert_eq!(crate::TransferLimit::<Test>::get(), settings);

        let wrong_settings = crate::TransferLimitSettings {
            max_amount: 1000,
            period_blocks: 0u32.into(),
        };
        assert_noop!(
            BridgeProxy::update_transfer_limit(RawOrigin::Root.into(), wrong_settings.clone()),
            crate::Error::<Test>::WrongLimitSettings
        );
        assert_eq!(crate::TransferLimit::<Test>::get(), settings);
    })
}

#[test]
fn transfer_limit_works() {
    new_tester().execute_with(|| {
        let caller: AccountId = Keyring::Alice.into();
        assert_ok!(BridgeProxy::add_limited_asset(RawOrigin::Root.into(), XOR));
        assert_ok!(Currencies::update_balance(
            RawOrigin::Root.into(),
            caller.clone(),
            XOR,
            balance!(50000) as i128,
        ));
        assert_ok!(BridgeProxy::burn(
            RawOrigin::Signed(caller.clone()).into(),
            BASE_EVM_NETWORK_ID.into(),
            XOR,
            GenericAccount::EVM(H160::default()),
            balance!(5000),
        ));
        assert_eq!(crate::ConsumedTransferLimit::<Test>::get(), balance!(12500));
        assert_eq!(
            crate::TransferLimitUnlockSchedule::<Test>::get(601),
            balance!(12500)
        );

        frame_system::Pallet::<Test>::set_block_number(100);
        assert_ok!(BridgeProxy::burn(
            RawOrigin::Signed(caller.clone()).into(),
            BASE_EVM_NETWORK_ID.into(),
            XOR,
            GenericAccount::EVM(H160::default()),
            balance!(10000),
        ));
        assert_eq!(crate::ConsumedTransferLimit::<Test>::get(), balance!(37500));
        assert_eq!(
            crate::TransferLimitUnlockSchedule::<Test>::get(601),
            balance!(12500)
        );
        assert_eq!(
            crate::TransferLimitUnlockSchedule::<Test>::get(700),
            balance!(25000)
        );

        assert_noop!(
            BridgeProxy::burn(
                RawOrigin::Signed(caller.clone()).into(),
                BASE_EVM_NETWORK_ID.into(),
                XOR,
                GenericAccount::EVM(H160::default()),
                balance!(7000),
            ),
            crate::Error::<Test>::TransferLimitReached
        );

        assert_eq!(crate::ConsumedTransferLimit::<Test>::get(), balance!(37500));
        assert_eq!(
            crate::TransferLimitUnlockSchedule::<Test>::get(601),
            balance!(12500)
        );
        assert_eq!(
            crate::TransferLimitUnlockSchedule::<Test>::get(700),
            balance!(25000)
        );

        BridgeProxy::on_initialize(601);

        assert_eq!(crate::ConsumedTransferLimit::<Test>::get(), balance!(25000));
        assert_eq!(crate::TransferLimitUnlockSchedule::<Test>::get(601), 0);
        assert_eq!(
            crate::TransferLimitUnlockSchedule::<Test>::get(700),
            balance!(25000)
        );

        frame_system::Pallet::<Test>::set_block_number(650);

        assert_ok!(BridgeProxy::burn(
            RawOrigin::Signed(caller.clone()).into(),
            BASE_EVM_NETWORK_ID.into(),
            XOR,
            GenericAccount::EVM(H160::default()),
            balance!(7000),
        ));

        assert_eq!(crate::ConsumedTransferLimit::<Test>::get(), balance!(42500));
        assert_eq!(
            crate::TransferLimitUnlockSchedule::<Test>::get(700),
            balance!(25000)
        );
        assert_eq!(
            crate::TransferLimitUnlockSchedule::<Test>::get(1250),
            balance!(17500)
        );
    })
}

#[test]
fn permanent_message_quarantine_preserves_status_and_refund_evidence_after_halt_removal() {
    use bridge_types::traits::MessageStatusNotifier;
    new_tester().execute_with(|| {
        let owner: AccountId = Keyring::Alice.into();
        let network = GenericNetworkId::EVM(BASE_EVM_NETWORK_ID);
        let id = bridge_types::H256::repeat_byte(42);
        let request = BridgeRequest {
            source: GenericAccount::Sora(owner.clone()),
            dest: GenericAccount::EVM(H160::default()),
            asset_id: XOR,
            amount: 1_000,
            status: MessageStatus::Committed,
            start_timepoint: GenericTimepoint::Sora(1),
            end_timepoint: GenericTimepoint::Pending,
            direction: MessageDirection::Outbound,
        };
        crate::Senders::<Test>::insert(network, id, owner.clone());
        Transactions::<Test>::insert((network, owner.clone()), id, request.clone());
        sp_io::storage::set(&bridge_types::recovery::message_key(network, id), &[1]);
        sp_io::storage::clear(common::recovery::BRIDGE_HALT_STORAGE_KEY);
        let root = sp_io::storage::root(sp_runtime::StateVersion::V1);
        BridgeProxy::update_status(
            network,
            id,
            MessageStatus::Failed,
            GenericTimepoint::Sora(2),
        );
        assert_eq!(
            Transactions::<Test>::get((network, owner.clone()), id),
            Some(request)
        );
        assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), root);
        assert_noop!(
            BridgeProxy::refund(network, id, GenericAccount::Sora(owner.clone()), XOR, 1_000),
            crate::Error::<Test>::RecoveryHalted
        );
    });
}
