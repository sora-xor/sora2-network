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

use super::*;
use bridge_types::substrate::BridgeMessage;
use codec::{Decode, Encode, MaxEncodedLen};

use frame_support::traits::{Everything, UnfilteredDispatchable};
use frame_support::{assert_err, assert_noop, assert_ok, parameter_types, Deserialize, Serialize};
use scale_info::TypeInfo;
use sp_core::{ConstU64, RuntimeDebug, H256};
use sp_keyring::AccountKeyring as Keyring;
use sp_runtime::traits::{BlakeTwo256, IdentifyAccount, IdentityLookup, ValidateUnsigned, Verify};
use sp_runtime::transaction_validity::{
    InvalidTransaction, TransactionSource, TransactionValidityError,
};
use sp_runtime::{BuildStorage, MultiSignature};
use sp_std::convert::From;

use bridge_types::traits::MessageDispatch;
use bridge_types::{GenericNetworkId, GenericTimepoint};

use crate::inbound::Error;

use crate::inbound as bridge_inbound_channel;

type Block = frame_system::mocking::MockBlock<Test>;

const BASE_NETWORK_ID: SubNetworkId = SubNetworkId::Mainnet;

frame_support::construct_runtime!(
    pub enum Test
    {
        System: frame_system,
        Timestamp: pallet_timestamp,
        Balances: pallet_balances,
        BridgeInboundChannel: bridge_inbound_channel,
    }
);

pub type Signature = MultiSignature;
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

#[derive(
    Encode,
    Decode,
    PartialEq,
    Eq,
    RuntimeDebug,
    Clone,
    Copy,
    MaxEncodedLen,
    TypeInfo,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
)]
pub enum AssetId {
    Xor,
    Eth,
    Dai,
}

pub type Balance = u128;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
}

impl frame_system::Config for Test {
    type BaseCallFilter = Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = BlockHashCount;
    type DbWeight = ();
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ();
    type OnSetCode = ();
    type MaxConsumers = frame_support::traits::ConstU32<65536>;
    type Nonce = u64;
    type Block = Block;
}

parameter_types! {
    pub const ExistentialDeposit: u128 = 1;
    pub const MaxLocks: u32 = 50;
    pub const MaxReserves: u32 = 50;
}

impl pallet_balances::Config for Test {
    /// The ubiquitous event type.
    type RuntimeEvent = RuntimeEvent;
    type MaxLocks = MaxLocks;
    /// The type for recording an account's balance.
    type Balance = Balance;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
    type MaxReserves = MaxReserves;
    type ReserveIdentifier = ();
    type RuntimeHoldReason = ();
    type FreezeIdentifier = ();
    type MaxHolds = ();
    type MaxFreezes = ();
}

// Mock verifier
pub struct MockVerifier;

impl Verifier for MockVerifier {
    type Proof = Vec<u8>;

    fn verify(network_id: GenericNetworkId, _hash: H256, _proof: &Vec<u8>) -> DispatchResult {
        let network_id = match network_id {
            bridge_types::GenericNetworkId::Sub(ni) => ni,
            _ => return Err(Error::<Test>::InvalidNetwork.into()),
        };
        if network_id == BASE_NETWORK_ID {
            Ok(())
        } else {
            Err(Error::<Test>::InvalidNetwork.into())
        }
    }

    fn verify_weight(_proof: &Self::Proof) -> frame_support::weights::Weight {
        Default::default()
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn valid_proof() -> Option<Self::Proof> {
        Some(Default::default())
    }
}

// Mock Dispatch
pub struct MockMessageDispatch;

impl MessageDispatch<Test, SubNetworkId, MessageId, ()> for MockMessageDispatch {
    fn dispatch(_: SubNetworkId, _: MessageId, _: GenericTimepoint, _: &[u8], _: ()) {}

    fn dispatch_weight(_: &[u8]) -> frame_support::weights::Weight {
        Default::default()
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn successful_dispatch_event(
        _: MessageId,
    ) -> Option<<Test as frame_system::Config>::RuntimeEvent> {
        None
    }
}

parameter_types! {
    pub SourceAccount: AccountId = Keyring::Eve.into();
}

impl pallet_timestamp::Config for Test {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = ();
    type WeightInfo = ();
}

parameter_types! {
    pub const MaxMessagePayloadSize: u32 = 128;
    pub const MaxMessagesPerCommit: u32 = 5;
    pub const ThisNetworkId: GenericNetworkId = GenericNetworkId::Sub(SubNetworkId::Mainnet);
}

impl bridge_inbound_channel::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Verifier = MockVerifier;
    type MessageDispatch = MockMessageDispatch;
    type UnsignedLongevity = ConstU64<100>;
    type UnsignedPriority = ConstU64<100>;
    type MaxMessagePayloadSize = MaxMessagePayloadSize;
    type MaxMessagesPerCommit = MaxMessagesPerCommit;
    type ThisNetworkId = ThisNetworkId;
    type WeightInfo = ();
}

pub fn new_tester() -> sp_io::TestExternalities {
    let mut storage = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    let bob: AccountId = Keyring::Bob.into();
    pallet_balances::GenesisConfig::<Test> {
        balances: vec![(bob, 1_000_000_000_000_000_000)],
    }
    .assimilate_storage(&mut storage)
    .unwrap();

    let mut ext: sp_io::TestExternalities = storage.into();
    ext.execute_with(|| System::set_block_number(1));
    ext
}

#[test]
fn test_submit() {
    new_tester().execute_with(|| {
        let origin = RuntimeOrigin::none();

        // Submit message 1
        let message_1 = BridgeMessage {
            timepoint: Default::default(),
            payload: Default::default(),
        };
        let commitment =
            bridge_types::GenericCommitment::Sub(bridge_types::substrate::Commitment {
                nonce: 1,
                messages: vec![message_1].try_into().unwrap(),
            });

        let call = Call::<Test>::submit {
            network_id: BASE_NETWORK_ID,
            commitment,
            proof: vec![],
        };

        assert_ok!(Pallet::<Test>::validate_unsigned(
            TransactionSource::External,
            &call
        ));
        assert_ok!(call.dispatch_bypass_filter(origin.clone()));

        let nonce: u64 = <ChannelNonces<Test>>::get(BASE_NETWORK_ID);
        assert_eq!(nonce, 1);

        // Submit message 2
        let message_2 = BridgeMessage {
            timepoint: Default::default(),
            payload: Default::default(),
        };
        let commitment =
            bridge_types::GenericCommitment::Sub(bridge_types::substrate::Commitment {
                nonce: 2,
                messages: vec![message_2].try_into().unwrap(),
            });

        let call = Call::<Test>::submit {
            network_id: BASE_NETWORK_ID,
            commitment,
            proof: vec![],
        };

        assert_ok!(Pallet::<Test>::validate_unsigned(
            TransactionSource::External,
            &call
        ));
        assert_ok!(call.dispatch_bypass_filter(origin));

        let nonce: u64 = <ChannelNonces<Test>>::get(BASE_NETWORK_ID);
        assert_eq!(nonce, 2);
    });
}

#[test]
fn test_submit_with_invalid_nonce() {
    new_tester().execute_with(|| {
        let origin = RuntimeOrigin::none();

        // Submit message
        let message = BridgeMessage {
            timepoint: Default::default(),
            payload: Default::default(),
        };
        let commitment =
            bridge_types::GenericCommitment::Sub(bridge_types::substrate::Commitment {
                nonce: 1,
                messages: vec![message].try_into().unwrap(),
            });

        let call = Call::<Test>::submit {
            network_id: BASE_NETWORK_ID,
            commitment,
            proof: vec![],
        };

        assert_ok!(Pallet::<Test>::validate_unsigned(
            TransactionSource::External,
            &call
        ));
        assert_ok!(call.clone().dispatch_bypass_filter(origin.clone()));

        let nonce: u64 = <ChannelNonces<Test>>::get(BASE_NETWORK_ID);
        assert_eq!(nonce, 1);

        // Submit the same again
        assert_err!(
            Pallet::<Test>::validate_unsigned(TransactionSource::External, &call),
            TransactionValidityError::Invalid(InvalidTransaction::BadProof)
        );
        assert_noop!(
            call.dispatch_bypass_filter(origin),
            Error::<Test>::InvalidNonce
        );
    });
}

#[test]
fn test_submit_with_invalid_network_id() {
    new_tester().execute_with(|| {
        let origin = RuntimeOrigin::none();

        // Submit message
        let message = BridgeMessage {
            timepoint: Default::default(),
            payload: Default::default(),
        };
        let commitment =
            bridge_types::GenericCommitment::Sub(bridge_types::substrate::Commitment {
                nonce: 1,
                messages: vec![message].try_into().unwrap(),
            });

        let call = Call::<Test>::submit {
            network_id: SubNetworkId::Kusama,
            commitment,
            proof: vec![],
        };

        assert_err!(
            Pallet::<Test>::validate_unsigned(TransactionSource::External, &call),
            TransactionValidityError::Invalid(InvalidTransaction::BadProof)
        );
        assert_noop!(
            call.dispatch_bypass_filter(origin),
            Error::<Test>::InvalidNetwork
        );
    });
}
