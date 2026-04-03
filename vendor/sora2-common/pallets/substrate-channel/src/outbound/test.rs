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

use crate::outbound::mock::*;
use crate::outbound::*;
use bridge_types::{
    substrate::Commitment, traits::OutboundChannel, types::GenericCommitmentWithBlock, SubNetworkId,
};
use frame_support::{assert_noop, assert_ok, traits::Hooks};
use frame_system::RawOrigin;
use sp_keyring::AccountKeyring as Keyring;
use sp_runtime::{AccountId32, BoundedVec, DispatchError};

#[test]
fn test_submit() {
    new_tester().execute_with(|| {
        let who: AccountId = Keyring::Bob.into();

        assert_ok!(BridgeOutboundChannel::submit(
            BASE_NETWORK_ID,
            &RawOrigin::Signed(who.clone()),
            &[0, 1, 2],
            ()
        ));
        BridgeOutboundChannel::commit(BASE_NETWORK_ID);
        assert_eq!(<ChannelNonces<Test>>::get(BASE_NETWORK_ID), 1);

        assert_ok!(BridgeOutboundChannel::submit(
            BASE_NETWORK_ID,
            &RawOrigin::Signed(who),
            &[0, 1, 2],
            ()
        ));
        BridgeOutboundChannel::commit(BASE_NETWORK_ID);
        assert_eq!(<ChannelNonces<Test>>::get(BASE_NETWORK_ID), 2);
    });
}

#[test]
fn test_submit_exceeds_queue_limit() {
    new_tester().execute_with(|| {
        let who: AccountId = Keyring::Bob.into();

        let max_messages = MaxMessagesPerCommit::get();
        (0..max_messages).for_each(|_| {
            BridgeOutboundChannel::submit(
                BASE_NETWORK_ID,
                &RawOrigin::Signed(who.clone()),
                &[0, 1, 2],
                (),
            )
            .unwrap();
        });

        assert_noop!(
            BridgeOutboundChannel::submit(BASE_NETWORK_ID, &RawOrigin::Signed(who), &[0, 1, 2], ()),
            Error::<Test>::QueueSizeLimitReached,
        );
    })
}

#[test]
fn test_submit_exceeds_payload_limit() {
    new_tester().execute_with(|| {
        let who: AccountId = Keyring::Bob.into();

        let max_payload_bytes = MaxMessagePayloadSize::get();
        let payload: Vec<u8> = (0..).take(max_payload_bytes as usize + 1).collect();

        assert_noop!(
            BridgeOutboundChannel::submit(
                BASE_NETWORK_ID,
                &RawOrigin::Signed(who),
                payload.as_slice(),
                ()
            ),
            Error::<Test>::PayloadTooLarge,
        );
    })
}

#[test]
fn test_submit_fails_on_nonce_overflow() {
    new_tester().execute_with(|| {
        let who: AccountId = Keyring::Bob.into();

        <ChannelNonces<Test>>::insert(BASE_NETWORK_ID, u64::MAX);
        assert_noop!(
            BridgeOutboundChannel::submit(BASE_NETWORK_ID, &RawOrigin::Signed(who), &[0, 1, 2], ()),
            Error::<Test>::Overflow,
        );
    });
}

#[test]
fn test_update_interval_bad_origin() {
    new_tester().execute_with(|| {
        let who: AccountId = Keyring::Bob.into();

        assert_noop!(
            BridgeOutboundChannel::update_interval(RawOrigin::Signed(who).into(), 1u64),
            DispatchError::BadOrigin,
        );
    });
}

#[test]
fn test_update_interval_works() {
    new_tester().execute_with(|| {
        assert_ok!(BridgeOutboundChannel::update_interval(
            RawOrigin::Root.into(),
            1u64
        ),);
    });
}

#[test]
fn test_update_interval_zero_interval() {
    new_tester().execute_with(|| {
        assert_noop!(
            BridgeOutboundChannel::update_interval(RawOrigin::Root.into(), 0u64),
            Error::<Test>::ZeroInterval,
        );
    });
}

#[test]
fn test_on_finalize() {
    new_tester().execute_with(|| {
        assert_ok!(BridgeOutboundChannel::submit(
            SubNetworkId::Alphanet,
            &RawOrigin::Root,
            &[1, 2],
            ()
        ));
        assert_ok!(BridgeOutboundChannel::submit(
            SubNetworkId::Kusama,
            &RawOrigin::Signed(AccountId32::new([1; 32])),
            &[3, 4],
            ()
        ));
        assert_ok!(BridgeOutboundChannel::submit(
            SubNetworkId::Kusama,
            &RawOrigin::Root,
            &[5, 6],
            ()
        ));
        BridgeOutboundChannel::on_finalize(10);
        assert_eq!(
            LatestCommitment::<Test>::get(SubNetworkId::Alphanet),
            Some(GenericCommitmentWithBlock {
                block_number: 1,
                commitment: bridge_types::GenericCommitment::Sub(Commitment {
                    messages: BoundedVec::truncate_from(vec![BridgeMessage {
                        payload: BoundedVec::truncate_from(vec![1, 2]),
                        timepoint: bridge_types::GenericTimepoint::Sora(1)
                    }]),
                    nonce: 1
                })
            })
        );
        assert_eq!(
            LatestCommitment::<Test>::get(SubNetworkId::Kusama),
            Some(GenericCommitmentWithBlock {
                block_number: 1,
                commitment: bridge_types::GenericCommitment::Sub(Commitment {
                    messages: BoundedVec::truncate_from(vec![
                        BridgeMessage {
                            payload: BoundedVec::truncate_from(vec![3, 4]),
                            timepoint: bridge_types::GenericTimepoint::Sora(1)
                        },
                        BridgeMessage {
                            payload: BoundedVec::truncate_from(vec![5, 6]),
                            timepoint: bridge_types::GenericTimepoint::Sora(1)
                        },
                    ]),
                    nonce: 1
                })
            })
        );
    });
}
