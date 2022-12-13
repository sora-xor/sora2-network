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

use crate::mock::{
    new_tester, AccountId, BridgeOutboundChannel, Call, Currencies, Dispatch, ERC20App, Event,
    EvmBridgeProxy, System, Test, BASE_NETWORK_ID,
};
use crate::{BridgeRequest, Transactions};
use bridge_types::traits::MessageDispatch;
use codec::Encode;
use common::{balance, DAI, XOR};
use frame_support::assert_noop;
use frame_support::traits::Hooks;
use frame_system::RawOrigin;
use sp_core::H160;
use sp_keyring::AccountKeyring as Keyring;
use sp_runtime::traits::Hash;

use bridge_types::types::{AssetKind, MessageId, MessageStatus};

fn assert_event(event: Event) {
    System::events()
        .iter()
        .find(|e| e.event == event)
        .expect("Event not found");
}

#[test]
fn burn_successfull() {
    new_tester().execute_with(|| {
        let caller: AccountId = Keyring::Alice.into();
        Currencies::update_balance(
            RawOrigin::Root.into(),
            caller.clone(),
            XOR,
            balance!(1) as i128,
        )
        .unwrap();
        EvmBridgeProxy::burn(
            RawOrigin::Signed(caller.clone()).into(),
            BASE_NETWORK_ID,
            XOR,
            H160::default(),
            1000,
        )
        .unwrap();
        let message_id = BridgeOutboundChannel::make_message_id(1);
        assert_eq!(
            Transactions::<Test>::get(&caller, (BASE_NETWORK_ID, message_id)),
            Some(BridgeRequest::OutgoingTransfer {
                source: caller.clone(),
                dest: H160::default(),
                asset_id: XOR,
                amount: 1000,
                status: MessageStatus::InQueue,
                start_timestamp: 0,
                end_timestamp: None,
            })
        );
        assert_event(crate::Event::RequestStatusUpdate(message_id, MessageStatus::InQueue).into());
        BridgeOutboundChannel::on_initialize(BridgeOutboundChannel::interval());
        assert_event(
            crate::Event::RequestStatusUpdate(message_id, MessageStatus::Committed).into(),
        );
        assert_eq!(
            Transactions::<Test>::get(&caller, (BASE_NETWORK_ID, message_id)),
            Some(BridgeRequest::OutgoingTransfer {
                source: caller.clone(),
                dest: H160::default(),
                asset_id: XOR,
                amount: 1000,
                status: MessageStatus::Committed,
                start_timestamp: 0,
                end_timestamp: None,
            })
        );
    })
}

#[test]
fn burn_failed() {
    new_tester().execute_with(|| {
        let caller: AccountId = Keyring::Alice.into();
        assert_noop!(
            EvmBridgeProxy::burn(
                RawOrigin::Signed(caller.clone()).into(),
                BASE_NETWORK_ID,
                XOR,
                H160::default(),
                1000,
            ),
            pallet_balances::Error::<Test>::InsufficientBalance
        );
        assert_eq!(Transactions::<Test>::iter().count(), 0);
        assert_eq!(System::events().len(), 0);
    })
}

#[test]
fn mint_successfull() {
    new_tester().execute_with(|| {
        let recipient: AccountId = Keyring::Alice.into();
        let source = ERC20App::app_address(BASE_NETWORK_ID, AssetKind::Sidechain).unwrap();
        let token = ERC20App::token_address(BASE_NETWORK_ID, DAI).unwrap();
        Dispatch::dispatch(
            BASE_NETWORK_ID,
            source,
            MessageId::inbound(0),
            0,
            &Call::ERC20App(erc20_app::Call::mint {
                token,
                sender: Default::default(),
                recipient: recipient.clone(),
                amount: 1000u64.into(),
            })
            .encode(),
        );
        let message_id =
            MessageId::inbound(0).using_encoded(<Test as dispatch::Config>::Hashing::hash);
        assert_eq!(
            Transactions::<Test>::get(&recipient, (BASE_NETWORK_ID, message_id)),
            Some(BridgeRequest::IncomingTransfer {
                source: H160::default(),
                dest: recipient.clone(),
                asset_id: DAI,
                amount: 1000,
                status: MessageStatus::Done,
                start_timestamp: 0,
                end_timestamp: 0,
            })
        );
        assert_event(crate::Event::RequestStatusUpdate(message_id, MessageStatus::Done).into());
    })
}

#[test]
fn mint_failed() {
    new_tester().execute_with(|| {
        let recipient: AccountId = Keyring::Alice.into();
        let source = ERC20App::app_address(BASE_NETWORK_ID, AssetKind::Thischain).unwrap();
        let token = ERC20App::token_address(BASE_NETWORK_ID, DAI).unwrap();
        Dispatch::dispatch(
            BASE_NETWORK_ID,
            source,
            MessageId::inbound(0),
            0,
            &Call::ERC20App(erc20_app::Call::mint {
                token,
                sender: Default::default(),
                recipient: recipient.clone(),
                amount: 1000u64.into(),
            })
            .encode(),
        );
        assert_eq!(Transactions::<Test>::iter().count(), 0);
        assert_eq!(System::events().len(), 1);
    })
}
