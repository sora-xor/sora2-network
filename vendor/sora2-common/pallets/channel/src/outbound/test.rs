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
use bridge_types::evm::AdditionalEVMOutboundData;
use bridge_types::EVMChainId;
use bridge_types::{GenericBridgeMessage, GenericCommitment};
use bridge_types::{GenericNetworkId, GenericTimepoint};
use codec::{Decode, Encode, MaxEncodedLen};
use currencies::BasicCurrencyAdapter;
use frame_support::derive_impl;

use bridge_types::traits::{OutboundChannel, TimepointProvider};
use frame_support::traits::Everything;
use frame_support::{assert_noop, assert_ok, parameter_types, Deserialize, Serialize};
use frame_system::RawOrigin;
use scale_info::TypeInfo;
use sp_core::{H160, H256};
use sp_keyring::sr25519::Keyring;

use sp_runtime::traits::{BlakeTwo256, IdentifyAccount, IdentityLookup, Verify};
use sp_runtime::{AccountId32, BuildStorage, MultiSignature};
use sp_std::convert::From;
use traits::parameter_type_with_key;

use crate::outbound as bridge_outbound_channel;

type Block = frame_system::mocking::MockBlock<Test>;

const BASE_NETWORK_ID: GenericNetworkId = GenericNetworkId::Sub(SubNetworkId::Mainnet);

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system::{Pallet, Call, Storage, Event<T>},
        Timestamp: pallet_timestamp::{Pallet, Call, Storage},
        Tokens: tokens::{Pallet, Call, Config<T>, Storage, Event<T>},
        Currencies: currencies::{Pallet, Call, Storage},
        Balances: pallet_balances::{Pallet, Call, Storage, Event<T>},
        BridgeOutboundChannel: bridge_outbound_channel::{Pallet, Config<T>, Storage, Event<T>},
    }
);

pub type Signature = MultiSignature;
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

#[derive(
    Encode,
    Decode,
    codec::DecodeWithMemTracking,
    PartialEq,
    Eq,
    Debug,
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
pub type Amount = i128;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
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
}

parameter_type_with_key! {
    pub ExistentialDeposits: |_currency_id: AssetId| -> Balance {
        0
    };
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type Balance = Balance;
    type RuntimeEvent = RuntimeEvent;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
    type MaxLocks = ();
    type MaxReserves = ();
    type ReserveIdentifier = ();
    type RuntimeHoldReason = ();
    type FreezeIdentifier = ();
    type MaxFreezes = ();
}

impl tokens::Config for Test {
    type Balance = Balance;
    type Amount = Amount;
    type CurrencyId = AssetId;
    type WeightInfo = ();
    type ExistentialDeposits = ExistentialDeposits;
    type CurrencyHooks = ();
    type MaxLocks = ();
    type MaxReserves = ();
    type ReserveIdentifier = ();
    type DustRemovalWhitelist = Everything;
}

impl currencies::Config for Test {
    type MultiCurrency = Tokens;
    type NativeCurrency = BasicCurrencyAdapter<Test, Balances, Amount, u64>;
    type GetNativeCurrencyId = GetBaseAssetId;
    type WeightInfo = ();
}
parameter_types! {
    pub const GetBaseAssetId: AssetId = AssetId::Xor;
    pub GetTeamReservesAccountId: AccountId = AccountId32::from([0; 32]);
    pub GetFeeAccountId: AccountId = AccountId32::from([1; 32]);
    pub GetTreasuryAccountId: AccountId = AccountId32::from([2; 32]);
}

parameter_types! {
    pub const MaxMessagePayloadSize: u32 = 128;
    pub const MaxMessagesPerCommit: u32 = 5;
    pub const ThisNetworkId: GenericNetworkId = GenericNetworkId::Sub(SubNetworkId::Mainnet);
}

pub struct GenericTimepointProvider;

impl TimepointProvider for GenericTimepointProvider {
    fn get_timepoint() -> bridge_types::GenericTimepoint {
        bridge_types::GenericTimepoint::Sora(System::block_number() as u32)
    }
}

parameter_types! {
    pub const BridgeMaxTotalGasLimit: u64 = 5_000_000;
    pub const BridgeMaxGasPerMessage: u64 = 5_000_000;
}

impl bridge_outbound_channel::Config for Test {
    type MaxMessagePayloadSize = MaxMessagePayloadSize;
    type MaxMessagesPerCommit = MaxMessagesPerCommit;
    type MessageStatusNotifier = ();
    type AuxiliaryDigestHandler = ();
    type AssetId = ();
    type Balance = u128;
    type WeightInfo = ();
    type TimepointProvider = GenericTimepointProvider;
    type ThisNetworkId = ThisNetworkId;
    type MaxGasPerCommit = BridgeMaxTotalGasLimit;
    type MaxGasPerMessage = BridgeMaxGasPerMessage;
}

impl pallet_timestamp::Config for Test {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = ();
    type WeightInfo = ();
}

pub fn new_tester() -> sp_io::TestExternalities {
    init_test_logging();
    let mut storage = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    let config: bridge_outbound_channel::GenesisConfig<Test> =
        bridge_outbound_channel::GenesisConfig {
            interval: 10u32.into(),
        };
    config.assimilate_storage(&mut storage).unwrap();

    let bob: AccountId = Keyring::Bob.into();

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![(bob, 1u32.into())],
        ..Default::default()
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
        let who: AccountId = Keyring::Bob.into();

        assert_ok!(BridgeOutboundChannel::submit(
            BASE_NETWORK_ID.sub().unwrap(),
            &RawOrigin::Signed(who.clone()),
            &[0, 1, 2],
            ()
        ));
        BridgeOutboundChannel::commit(BASE_NETWORK_ID);
        assert_eq!(<ChannelNonces<Test>>::get(BASE_NETWORK_ID), 1);

        assert_ok!(BridgeOutboundChannel::submit(
            BASE_NETWORK_ID.sub().unwrap(),
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
                BASE_NETWORK_ID.sub().unwrap(),
                &RawOrigin::Signed(who.clone()),
                &[0, 1, 2],
                (),
            )
            .unwrap();
        });

        assert_noop!(
            BridgeOutboundChannel::submit(
                BASE_NETWORK_ID.sub().unwrap(),
                &RawOrigin::Signed(who),
                &[0, 1, 2],
                ()
            ),
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
                BASE_NETWORK_ID.sub().unwrap(),
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
            BridgeOutboundChannel::submit(
                BASE_NETWORK_ID.sub().unwrap(),
                &RawOrigin::Signed(who),
                &[0, 1, 2],
                ()
            ),
            Error::<Test>::Overflow,
        );
    });
}

#[test]
fn evm_queue_total_gas_is_cleared_after_commit() {
    new_tester().execute_with(|| {
        let who: AccountId = Keyring::Bob.into();
        let network_id = EVMChainId::default();
        let additional_data = AdditionalEVMOutboundData {
            max_gas: 2_500_000u32.into(),
            target: H160::zero(),
        };

        assert_ok!(<BridgeOutboundChannel as OutboundChannel<
            EVMChainId,
            AccountId,
            AdditionalEVMOutboundData,
        >>::submit(
            network_id,
            &RawOrigin::Signed(who.clone()),
            &[0, 1, 2],
            additional_data,
        ));
        assert_eq!(
            QueueTotalGas::<Test>::get(GenericNetworkId::EVM(network_id)),
            2_500_000u32.into(),
        );

        BridgeOutboundChannel::commit(GenericNetworkId::EVM(network_id));
        assert!(MessageQueues::<Test>::get(GenericNetworkId::EVM(network_id)).is_empty());
        assert_eq!(
            QueueTotalGas::<Test>::get(GenericNetworkId::EVM(network_id)),
            0u32.into(),
        );

        assert_ok!(<BridgeOutboundChannel as OutboundChannel<
            EVMChainId,
            AccountId,
            AdditionalEVMOutboundData,
        >>::submit(
            network_id,
            &RawOrigin::Signed(who),
            &[0, 1, 2],
            additional_data,
        ));
    });
}

#[test]
fn recovery_halt_preserves_queued_messages_and_nonces_without_committing() {
    new_tester().execute_with(|| {
        let origin = RawOrigin::Signed(Keyring::Bob.to_account_id());
        assert_ok!(BridgeOutboundChannel::submit(
            BASE_NETWORK_ID.sub().unwrap(),
            &origin,
            &[0, 1, 2],
            ()
        ));
        let queue = MessageQueues::<Test>::get(BASE_NETWORK_ID);
        sp_io::storage::set(crate::recovery::HALT_KEY, &[1]);
        sp_io::storage::set(crate::recovery::INBOUND_CONTEXT_KEY, &1u32.to_le_bytes());
        assert_noop!(
            BridgeOutboundChannel::submit(BASE_NETWORK_ID.sub().unwrap(), &origin, &[3], ()),
            Error::<Test>::RecoveryHalted
        );
        BridgeOutboundChannel::commit(BASE_NETWORK_ID);
        let _ = <BridgeOutboundChannel as frame_support::traits::Hooks<_>>::on_initialize(10);
        assert_eq!(MessageQueues::<Test>::get(BASE_NETWORK_ID), queue);
        assert_eq!(ChannelNonces::<Test>::get(BASE_NETWORK_ID), 0);
        assert!(LatestCommitment::<Test>::get(BASE_NETWORK_ID).is_none());
    });
}

#[test]
fn permanent_batch_quarantine_survives_global_halt_removal() {
    new_tester().execute_with(|| {
        let origin = RawOrigin::Signed(Keyring::Bob.to_account_id());
        assert_ok!(BridgeOutboundChannel::submit(
            BASE_NETWORK_ID.sub().unwrap(),
            &origin,
            &[0, 1, 2],
            ()
        ));
        let queue = MessageQueues::<Test>::get(BASE_NETWORK_ID);
        sp_io::storage::set(
            &bridge_types::recovery::batch_key(0, BASE_NETWORK_ID, 1),
            &[1],
        );
        sp_io::storage::clear(crate::recovery::HALT_KEY);
        sp_io::storage::set(crate::recovery::INBOUND_CONTEXT_KEY, &1u32.to_le_bytes());
        assert_noop!(
            BridgeOutboundChannel::submit(BASE_NETWORK_ID.sub().unwrap(), &origin, &[3], ()),
            Error::<Test>::RecoveryHalted
        );
        BridgeOutboundChannel::commit(BASE_NETWORK_ID);
        let _ = <BridgeOutboundChannel as frame_support::traits::Hooks<_>>::on_initialize(10);
        assert_eq!(MessageQueues::<Test>::get(BASE_NETWORK_ID), queue);
        assert_eq!(ChannelNonces::<Test>::get(BASE_NETWORK_ID), 0);
        assert!(LatestCommitment::<Test>::get(BASE_NETWORK_ID).is_none());
    });
}

#[test]
fn automatic_commit_obeys_interval_and_empty_queue_keeps_nonce() {
    use frame_support::traits::Hooks;
    new_tester().execute_with(|| {
        BridgeOutboundChannel::commit(BASE_NETWORK_ID);
        assert_eq!(ChannelNonces::<Test>::get(BASE_NETWORK_ID), 0);
        assert_ok!(BridgeOutboundChannel::submit(BASE_NETWORK_ID.sub().unwrap(), &RawOrigin::Root, &[1], ()));
        assert_eq!(BridgeOutboundChannel::recovery_queue(BASE_NETWORK_ID).len(), 1);
        BridgeOutboundChannel::on_initialize(9);
        assert_eq!(ChannelNonces::<Test>::get(BASE_NETWORK_ID), 0);
        sp_io::storage::set(b":sora-nexus:recovery:policy:v1", &[1]);
        BridgeOutboundChannel::on_initialize(10);
        assert_eq!(ChannelNonces::<Test>::get(BASE_NETWORK_ID), 1);
        assert!(BridgeOutboundChannel::recovery_queue(BASE_NETWORK_ID).is_empty());
        assert!(matches!(LatestCommitment::<Test>::get(BASE_NETWORK_ID).unwrap().commitment, bridge_types::GenericCommitment::Sub(batch) if batch.nonce == 1 && batch.messages[0].payload.as_slice() == [1]));
        BridgeOutboundChannel::commit(BASE_NETWORK_ID);
        assert_eq!(ChannelNonces::<Test>::get(BASE_NETWORK_ID), 1);
    });
}

#[test]
fn outbound_channel_is_trait_only_and_restores_the_default_interval() {
    use frame_support::traits::{GetCallIndex, GetCallName};
    assert!(Call::<Test>::get_call_names().is_empty());
    assert!(Call::<Test>::get_call_indices().is_empty());
    assert_eq!(
        BridgeOutboundChannel::call_functions().ty,
        scale_info::meta_type::<Call<Test>>()
    );
    new_tester().execute_with(|| {
        Interval::<Test>::kill();
        assert_eq!(BridgeOutboundChannel::interval(), 10);
    });
}

#[test]
fn evm_gas_limits_and_policy_rollback_protect_the_queue_budget() {
    new_tester().execute_with(|| {
        let network = EVMChainId::default();
        let origin = RawOrigin::Root;
        let data = |gas: u64| AdditionalEVMOutboundData {
            target: H160::repeat_byte(1),
            max_gas: gas.into(),
        };
        sp_io::storage::set(b":sora-nexus:recovery:policy:v1", &[1]);
        assert_noop!(
            BridgeOutboundChannel::submit(network, &origin, &[1], data(5_000_000)),
            Error::<Test>::MessageGasLimitExceeded
        );
        assert_ok!(BridgeOutboundChannel::submit(
            network,
            &origin,
            &[1],
            data(3_000_000)
        ));
        assert_noop!(
            BridgeOutboundChannel::submit(network, &origin, &[2], data(2_000_000)),
            Error::<Test>::CommitmentGasLimitExceeded
        );
        assert_eq!(
            QueueTotalGas::<Test>::get(GenericNetworkId::EVM(network)),
            3_000_000u32.into()
        );
        assert_noop!(
            BridgeOutboundChannel::submit(network, &origin, &[0; 129], data(1)),
            Error::<Test>::PayloadTooLarge
        );
        assert_eq!(
            QueueTotalGas::<Test>::get(GenericNetworkId::EVM(network)),
            3_000_000u32.into()
        );
        let queue = BridgeOutboundChannel::recovery_queue(network.into());
        sp_io::storage::set(
            &bridge_types::recovery::batch_key(0, network.into(), 1),
            &[1],
        );
        assert_noop!(
            BridgeOutboundChannel::submit(network, &origin, &[2], data(1)),
            Error::<Test>::RecoveryHalted
        );
        assert_eq!(
            QueueTotalGas::<Test>::get(GenericNetworkId::EVM(network)),
            3_000_000u32.into()
        );
        assert_eq!(BridgeOutboundChannel::recovery_queue(network.into()), queue);
        sp_io::storage::set(crate::recovery::HALT_KEY, &[1]);
        assert_noop!(
            BridgeOutboundChannel::submit(network, &origin, &[2], data(1)),
            Error::<Test>::RecoveryHalted
        );
    });
}

#[test]
fn outbound_weights_and_submit_gas_include_configured_policy_costs() {
    use bridge_types::traits::EVMOutboundChannel;
    new_tester().execute_with(|| {
        type SubChannel = BridgeOutboundChannel;
        let base = <SubChannel as OutboundChannel<SubNetworkId, AccountId, ()>>::submit_weight();
        assert_eq!(base, <SubChannel as OutboundChannel<EVMChainId, AccountId, AdditionalEVMOutboundData>>::submit_weight());
        assert_eq!(BridgeOutboundChannel::submit_gas(EVMChainId::default()).unwrap(), DefaultEVMSubmitGas::get());
        EVMSubmitGas::<Test>::insert(EVMChainId::default(), U256::from(123u32));
        assert_eq!(BridgeOutboundChannel::submit_gas(EVMChainId::default()).unwrap(), 123u32.into());
        sp_io::storage::set(b":sora-nexus:recovery:policy:v1", &[1]);
        let policy = <SubChannel as OutboundChannel<SubNetworkId, AccountId, ()>>::submit_weight();
        assert!(policy.ref_time() > base.ref_time());
        assert!(policy.proof_size() > base.proof_size());
        assert_eq!(policy, <SubChannel as OutboundChannel<EVMChainId, AccountId, AdditionalEVMOutboundData>>::submit_weight());
    });
}

#[test]
fn individual_quarantine_and_exhausted_nonce_prevent_committing_retained_messages() {
    new_tester().execute_with(|| {
        let id = BridgeOutboundChannel::submit(
            BASE_NETWORK_ID.sub().unwrap(),
            &RawOrigin::Root,
            &[1],
            (),
        )
        .unwrap();
        let queue = BridgeOutboundChannel::recovery_queue(BASE_NETWORK_ID);
        sp_io::storage::set(
            &bridge_types::recovery::message_key(BASE_NETWORK_ID, id),
            &[1],
        );
        assert_noop!(
            BridgeOutboundChannel::submit(
                BASE_NETWORK_ID.sub().unwrap(),
                &RawOrigin::Root,
                &[2],
                ()
            ),
            Error::<Test>::RecoveryHalted
        );
        BridgeOutboundChannel::commit(BASE_NETWORK_ID);
        assert_eq!(
            BridgeOutboundChannel::recovery_queue(BASE_NETWORK_ID),
            queue
        );
        assert_eq!(ChannelNonces::<Test>::get(BASE_NETWORK_ID), 0);
        ChannelNonces::<Test>::insert(BASE_NETWORK_ID, u64::MAX);
        BridgeOutboundChannel::commit(BASE_NETWORK_ID);
        assert_eq!(
            BridgeOutboundChannel::recovery_queue(BASE_NETWORK_ID),
            queue
        );
        assert_eq!(ChannelNonces::<Test>::get(BASE_NETWORK_ID), u64::MAX);
    });
}

// Exercise diagnostic formatting as it is enabled in an operator's debug build.
fn init_test_logging() {
    struct TestLogger;
    impl log::Log for TestLogger {
        fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
            metadata.level() <= log::Level::Debug
        }
        fn log(&self, record: &log::Record<'_>) {
            if self.enabled(record.metadata()) {
                eprintln!("{}", record.args());
            }
        }
        fn flush(&self) {}
    }
    static LOGGER: TestLogger = TestLogger;
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        let _ = log::set_logger(&LOGGER);
        log::set_max_level(log::LevelFilter::Debug);
    });
}

#[test]
fn legacy_mixed_domain_queues_do_not_include_foreign_payloads_in_commitments() {
    new_tester().execute_with(|| {
        let sub = GenericBridgeMessage::Sub(bridge_types::substrate::BridgeMessage {
            payload: vec![1].try_into().unwrap(),
            timepoint: GenericTimepoint::Pending,
        });
        let evm = GenericBridgeMessage::EVM(bridge_types::evm::Message {
            target: H160::repeat_byte(3),
            max_gas: 7u32.into(),
            payload: vec![2].try_into().unwrap(),
        });
        let evm_network = GenericNetworkId::EVM(H256::repeat_byte(73));
        for network in [BASE_NETWORK_ID, evm_network] {
            MessageQueues::<Test>::insert(
                network,
                BoundedVec::try_from(vec![sub.clone(), evm.clone()]).unwrap(),
            );
            BridgeOutboundChannel::commit(network);
            assert_eq!(ChannelNonces::<Test>::get(network), 1);
            assert!(MessageQueues::<Test>::get(network).is_empty());
            match LatestCommitment::<Test>::get(network).unwrap().commitment {
                GenericCommitment::Sub(batch) => {
                    assert_eq!(batch.messages.len(), 1);
                    assert_eq!(batch.messages[0].payload.as_slice(), [1]);
                }
                GenericCommitment::EVM(bridge_types::evm::Commitment::Outbound(batch)) => {
                    assert_eq!(batch.messages.len(), 1);
                    assert_eq!(batch.messages[0].payload.as_slice(), [2]);
                    assert_eq!(batch.total_max_gas, 7u32.into());
                }
                _ => panic!("unexpected commitment domain"),
            }
        }
        for network in [
            GenericNetworkId::EVMLegacy(74),
            GenericNetworkId::TON(bridge_types::ton::TonNetworkId::Testnet),
        ] {
            MessageQueues::<Test>::insert(
                network,
                BoundedVec::try_from(vec![sub.clone()]).unwrap(),
            );
            BridgeOutboundChannel::commit(network);
            assert!(LatestCommitment::<Test>::get(network).is_none());
            assert!(MessageQueues::<Test>::get(network).is_empty());
        }
    });
}
