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
use bridge_types::types::GenericAdditionalInboundData;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::derive_impl;
use frame_support::traits::{Everything, UnfilteredDispatchable};
use frame_support::{assert_err, assert_noop, assert_ok, parameter_types, Deserialize, Serialize};
use scale_info::TypeInfo;
use sp_core::{ConstU128, ConstU64, H256};
use sp_keyring::sr25519::Keyring;

use sp_runtime::traits::{BlakeTwo256, IdentifyAccount, IdentityLookup, ValidateUnsigned, Verify};
use sp_runtime::transaction_validity::{
    InvalidTransaction, TransactionSource, TransactionValidityError,
};
use sp_runtime::{BuildStorage, DispatchError, MultiSignature};
use sp_std::convert::From;

use bridge_types::traits::MessageDispatch;
use bridge_types::{GenericNetworkId, GenericTimepoint};

use crate::inbound::Error;

use crate::inbound as bridge_inbound_channel;

type Block = frame_system::mocking::MockBlock<Test>;

const BASE_NETWORK_ID: GenericNetworkId = GenericNetworkId::Sub(SubNetworkId::Mainnet);

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system::{Pallet, Call, Storage, Event<T>},
        Timestamp: pallet_timestamp::{Pallet, Call, Storage},
        Balances: pallet_balances::{Pallet, Call, Storage, Event<T>},
        BridgeInboundChannel: bridge_inbound_channel::{Pallet, Call, Storage, Event<T>},
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
    pub const MaxLocks: u32 = 50;
    pub const MaxReserves: u32 = 50;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
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
    type MaxFreezes = ();
}

// Mock verifier
pub struct MockVerifier;

impl Verifier for MockVerifier {
    type Proof = Vec<u8>;

    fn verify(network_id: GenericNetworkId, _hash: H256, proof: &Vec<u8>) -> DispatchResult {
        if proof == &vec![255] {
            return Err(sp_runtime::DispatchError::Other("Invalid bridge proof"));
        }
        let network_id = match network_id {
            bridge_types::GenericNetworkId::EVM(chain) if chain == [1u8; 32].into() => {
                return Ok(())
            }
            bridge_types::GenericNetworkId::TON(bridge_types::ton::TonNetworkId::Mainnet) => {
                return Ok(())
            }
            bridge_types::GenericNetworkId::EVM(_)
            | bridge_types::GenericNetworkId::TON(_)
            | bridge_types::GenericNetworkId::EVMLegacy(_) => {
                return Err(Error::<Test>::InvalidNetwork.into())
            }
            bridge_types::GenericNetworkId::Sub(ni) => ni,
        };
        if GenericNetworkId::from(network_id) == BASE_NETWORK_ID {
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

impl MessageDispatch<Test, GenericNetworkId, MessageId, GenericAdditionalInboundData>
    for MockMessageDispatch
{
    fn dispatch(
        _: GenericNetworkId,
        _: MessageId,
        _: GenericTimepoint,
        payload: &[u8],
        _: GenericAdditionalInboundData,
    ) -> bridge_types::traits::MessageDispatchOutcome {
        if payload == [255] {
            bridge_types::traits::MessageDispatchOutcome::Failed
        } else {
            let count = sp_io::storage::get(b"test/applied-messages")
                .and_then(|value| value.first().copied())
                .unwrap_or(0);
            sp_io::storage::set(b"test/applied-messages", &[count.saturating_add(1)]);
            bridge_types::traits::MessageDispatchOutcome::Applied
        }
    }

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

pub struct OutboundChannelImpl;

impl OutboundChannel<EVMChainId, AccountId, AdditionalEVMOutboundData> for OutboundChannelImpl {
    fn submit(
        network_id: EVMChainId,
        _who: &frame_system::RawOrigin<AccountId>,
        payload: &[u8],
        additional: AdditionalEVMOutboundData,
    ) -> Result<H256, DispatchError> {
        if sp_io::storage::exists(b"test/outbound-fails") {
            return Err(DispatchError::Unavailable);
        }
        sp_io::storage::set(
            b"test/outbound",
            &(network_id, payload, additional).encode(),
        );
        Ok(H256::repeat_byte(1))
    }

    fn submit_weight() -> frame_support::weights::Weight {
        frame_support::weights::Weight::from_all(1)
    }
}

pub struct TestFeeHandler;
impl bridge_types::traits::EVMFeeHandler<H256> for TestFeeHandler {
    fn get_latest_base_fee(_: EVMChainId) -> Result<bridge_types::U256, DispatchError> {
        Err(DispatchError::Unavailable)
    }
    fn get_network_fee_asset(_: EVMChainId) -> Result<H256, DispatchError> {
        Err(DispatchError::Unavailable)
    }
    fn on_fee_paid(network: EVMChainId, relayer: H160, amount: bridge_types::U256) {
        sp_io::storage::set(b"test/fee-paid", &(network, relayer, amount).encode());
    }
    fn update_base_fee(network: EVMChainId, fee: bridge_types::U256, block: u64) {
        sp_io::storage::set(b"test/base-fee", &(network, fee, block).encode());
    }
    fn can_update_base_fee(_: EVMChainId, _: bridge_types::U256, _: u64) -> bool {
        sp_io::storage::exists(b"test/allow-base-fee")
    }
}

impl bridge_inbound_channel::Config for Test {
    type MessageStatusNotifier = ();
    type AssetId = H256;
    type Balance = Balance;
    type EVMFeeHandler = TestFeeHandler;
    type OutboundChannel = OutboundChannelImpl;
    type Verifier = MockVerifier;
    type MessageDispatch = MockMessageDispatch;
    type UnsignedLongevity = ConstU64<100>;
    type UnsignedPriority = ConstU64<100>;
    type MaxMessagePayloadSize = MaxMessagePayloadSize;
    type MaxMessagesPerCommit = MaxMessagesPerCommit;
    type ThisNetworkId = ThisNetworkId;
    type EVMPriorityFee = ConstU128<5_000_000_000>;
    type WeightInfo = ();
}

pub fn new_tester() -> sp_io::TestExternalities {
    let mut storage = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    let bob: AccountId = Keyring::Bob.into();
    pallet_balances::GenesisConfig::<Test> {
        balances: vec![(bob, 1_000_000_000_000_000_000)],
        ..Default::default()
    }
    .assimilate_storage(&mut storage)
    .unwrap();

    let mut ext: sp_io::TestExternalities = storage.into();
    ext.execute_with(|| System::set_block_number(1));
    ext
}

#[test]
#[expect(
    deprecated,
    reason = "Exercise the deployed bridge unsigned admission contract during the SDK migration"
)]
fn test_submit() {
    new_tester().execute_with(|| {
        let origin = RuntimeOrigin::signed(Keyring::Bob.into());

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
#[expect(
    deprecated,
    reason = "Exercise the deployed bridge unsigned admission contract during the SDK migration"
)]
fn test_submit_with_invalid_nonce() {
    new_tester().execute_with(|| {
        let origin = RuntimeOrigin::signed(Keyring::Bob.into());

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
#[expect(
    deprecated,
    reason = "Exercise the deployed bridge unsigned admission contract during the SDK migration"
)]
fn test_submit_with_invalid_network_id() {
    new_tester().execute_with(|| {
        let origin = RuntimeOrigin::signed(Keyring::Bob.into());

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
            network_id: SubNetworkId::Kusama.into(),
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

#[test]
#[expect(
    deprecated,
    reason = "Exercise the deployed bridge unsigned admission contract during the SDK migration"
)]
fn authenticated_deliveries_stay_free_and_consume_failed_batch_nonces() {
    new_tester().execute_with(|| {
        use frame_support::dispatch::{GetDispatchInfo, Pays};
        for (nonce, payloads, expected) in [
            (1, vec![vec![1]], Pays::No),
            (2, vec![vec![1], vec![255]], Pays::No),
            (3, vec![], Pays::No),
        ] {
            let messages = payloads
                .into_iter()
                .map(|payload| BridgeMessage {
                    timepoint: Default::default(),
                    payload: payload.try_into().unwrap(),
                })
                .collect::<Vec<_>>()
                .try_into()
                .unwrap();
            let commitment =
                bridge_types::GenericCommitment::Sub(bridge_types::substrate::Commitment {
                    nonce,
                    messages,
                });
            let call = Call::<Test>::submit {
                network_id: BASE_NETWORK_ID,
                commitment,
                proof: vec![],
            };
            assert_eq!(call.get_dispatch_info().pays_fee, Pays::Yes);
            for source in [
                TransactionSource::External,
                TransactionSource::Local,
                TransactionSource::InBlock,
            ] {
                let before = sp_io::storage::root(sp_runtime::StateVersion::V1);
                assert_ok!(Pallet::<Test>::validate_unsigned(source, &call));
                assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), before);
            }
            assert_ok!(Pallet::<Test>::pre_dispatch(&call));
            let origin = if nonce == 2 {
                RuntimeOrigin::none()
            } else {
                RuntimeOrigin::signed(Keyring::Bob.into())
            };
            let result = call.clone().dispatch_bypass_filter(origin).unwrap();
            assert_eq!(result.pays_fee, expected);
            assert_eq!(
                sp_io::storage::get(b"test/applied-messages").unwrap()[0],
                if nonce == 1 { 1 } else { 2 }
            );
            assert_eq!(ChannelNonces::<Test>::get(BASE_NETWORK_ID), nonce);
            for source in [
                TransactionSource::External,
                TransactionSource::Local,
                TransactionSource::InBlock,
            ] {
                assert_err!(
                    Pallet::<Test>::validate_unsigned(source, &call),
                    TransactionValidityError::Invalid(InvalidTransaction::BadProof)
                );
            }
            assert!(Pallet::<Test>::pre_dispatch(&call).is_err());
            assert_noop!(
                call.dispatch_bypass_filter(RuntimeOrigin::signed(Keyring::Bob.into())),
                Error::<Test>::InvalidNonce
            );
        }
    });
}

#[test]
fn signed_status_report_refunds_authenticated_remote_failure_reporting() {
    new_tester().execute_with(|| {
        use frame_support::dispatch::Pays;
        let chain: EVMChainId = [1u8; 32].into();
        let channel = H160::repeat_byte(4);
        EVMChannelAddresses::<Test>::insert(chain, channel);
        let report = bridge_types::evm::StatusReport {
            channel,
            block_number: 10,
            relayer: H160::repeat_byte(3),
            nonce: 1,
            results: vec![false].try_into().unwrap(),
            gas_spent: 100.into(),
            base_fee: 10.into(),
        };
        let call = Call::<Test>::submit {
            network_id: GenericNetworkId::EVM(chain),
            commitment: bridge_types::GenericCommitment::EVM(
                bridge_types::evm::Commitment::StatusReport(report),
            ),
            proof: vec![],
        };
        let result = call
            .clone()
            .dispatch_bypass_filter(RuntimeOrigin::signed(Keyring::Bob.into()))
            .unwrap();
        assert_eq!(result.pays_fee, Pays::No);
        assert_eq!(
            ReportedChannelNonces::<Test>::get(GenericNetworkId::EVM(chain)),
            1
        );
        assert_noop!(
            call.dispatch_bypass_filter(RuntimeOrigin::signed(Keyring::Bob.into())),
            Error::<Test>::InvalidNonce
        );
    });
}

#[test]
#[expect(
    deprecated,
    reason = "Exercise the deployed bridge unsigned admission contract during the SDK migration"
)]
fn unauthenticated_or_wrong_type_submission_is_rejected_before_execution() {
    new_tester().execute_with(|| {
        let commitment =
            bridge_types::GenericCommitment::Sub(bridge_types::substrate::Commitment {
                nonce: 1,
                messages: vec![BridgeMessage {
                    timepoint: Default::default(),
                    payload: vec![1].try_into().unwrap(),
                }]
                .try_into()
                .unwrap(),
            });
        let call = Call::<Test>::submit {
            network_id: BASE_NETWORK_ID,
            commitment,
            proof: vec![255],
        };
        let before = sp_io::storage::root(sp_runtime::StateVersion::V1);
        for source in [
            TransactionSource::External,
            TransactionSource::Local,
            TransactionSource::InBlock,
        ] {
            assert_err!(
                Pallet::<Test>::validate_unsigned(source, &call),
                TransactionValidityError::Invalid(InvalidTransaction::BadProof)
            );
        }
        assert!(Pallet::<Test>::pre_dispatch(&call).is_err());
        assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), before);
        assert_eq!(ChannelNonces::<Test>::get(BASE_NETWORK_ID), 0);
        let wrong_type = Call::<Test>::submit {
            network_id: BASE_NETWORK_ID,
            commitment: bridge_types::GenericCommitment::EVM(
                bridge_types::evm::Commitment::BaseFeeUpdate(bridge_types::evm::BaseFeeUpdate {
                    new_base_fee: 1.into(),
                    evm_block_number: 1,
                }),
            ),
            proof: vec![],
        };
        assert_err!(
            Pallet::<Test>::validate_unsigned(TransactionSource::External, &wrong_type),
            TransactionValidityError::Invalid(InvalidTransaction::BadProof)
        );
        assert!(Pallet::<Test>::pre_dispatch(&wrong_type).is_err());
        assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), before);
    });
}

#[test]
fn recovery_halt_requires_exact_current_context_and_still_checks_authentication() {
    new_tester().execute_with(|| {
        let commitment =
            bridge_types::GenericCommitment::Sub(bridge_types::substrate::Commitment {
                nonce: 1,
                messages: vec![BridgeMessage {
                    timepoint: Default::default(),
                    payload: vec![1].try_into().unwrap(),
                }]
                .try_into()
                .unwrap(),
            });
        sp_io::storage::set(crate::recovery::HALT_KEY, &[1]);
        let validate =
            || Pallet::<Test>::validate_submission(BASE_NETWORK_ID, &commitment, &vec![255]);
        assert_noop!(validate(), Error::<Test>::RecoveryHalted);
        let number: u32 = System::block_number().try_into().unwrap();
        sp_io::storage::set(crate::recovery::INBOUND_CONTEXT_KEY, &number.to_le_bytes());
        assert_ok!(Pallet::<Test>::validate_submission(
            BASE_NETWORK_ID,
            &commitment,
            &vec![]
        ));
        // A recovery context never supplies or replaces a valid counterpart proof.
        assert!(validate().is_err());
        assert_eq!(ChannelNonces::<Test>::get(BASE_NETWORK_ID), 0);
        let malformed = [number.to_le_bytes().as_slice(), &[0]].concat();
        sp_io::storage::set(crate::recovery::INBOUND_CONTEXT_KEY, &malformed);
        assert_noop!(validate(), Error::<Test>::RecoveryHalted);
    });
}

#[test]
fn recovery_context_cannot_admit_abandoned_outbound_status_reports() {
    new_tester().execute_with(|| {
        let chain: EVMChainId = [1u8; 32].into();
        let channel = H160::repeat_byte(4);
        EVMChannelAddresses::<Test>::insert(chain, channel);
        sp_io::storage::set(crate::recovery::HALT_KEY, &[1]);
        let number: u32 = System::block_number().try_into().unwrap();
        sp_io::storage::set(crate::recovery::INBOUND_CONTEXT_KEY, &number.to_le_bytes());
        let report = bridge_types::evm::StatusReport {
            channel,
            block_number: 10,
            relayer: H160::repeat_byte(3),
            nonce: 1,
            results: vec![false].try_into().unwrap(),
            gas_spent: 100.into(),
            base_fee: 10.into(),
        };
        assert_noop!(
            Pallet::<Test>::submit(
                RuntimeOrigin::signed(Keyring::Bob.into()),
                GenericNetworkId::EVM(chain),
                bridge_types::GenericCommitment::EVM(bridge_types::evm::Commitment::StatusReport(
                    report
                )),
                vec![]
            ),
            Error::<Test>::RecoveryHalted
        );
        assert_eq!(
            ReportedChannelNonces::<Test>::get(GenericNetworkId::EVM(chain)),
            0
        );
    });
}

#[test]
fn evm_and_ton_inbound_messages_require_registered_channels_and_fresh_nonces() {
    use bridge_types::{evm, ton, GenericCommitment};
    use frame_support::dispatch::GetDispatchInfo;
    new_tester().execute_with(|| {
        let chain: EVMChainId = [1; 32].into();
        let channel = H160::repeat_byte(4);
        let evm = GenericCommitment::EVM(evm::Commitment::Inbound(evm::InboundCommitment {
            channel,
            source: H160::repeat_byte(8),
            nonce: 1,
            block_number: 12,
            payload: vec![1].try_into().unwrap(),
        }));
        assert_noop!(
            Pallet::<Test>::validate_submission(chain.into(), &evm, &vec![]),
            Error::<Test>::InvalidNetwork
        );
        assert_noop!(
            Pallet::<Test>::register_evm_channel(
                RuntimeOrigin::signed(Keyring::Bob.into()),
                chain,
                channel
            ),
            DispatchError::BadOrigin
        );
        assert_ok!(Pallet::<Test>::register_evm_channel(
            RuntimeOrigin::root(),
            chain,
            H160::zero()
        ));
        assert_noop!(
            Pallet::<Test>::validate_submission(chain.into(), &evm, &vec![]),
            Error::<Test>::InvalidSourceChannel
        );
        assert_ok!(Pallet::<Test>::register_evm_channel(
            RuntimeOrigin::root(),
            chain,
            channel
        ));
        let evm_call = Call::<Test>::submit {
            network_id: chain.into(),
            commitment: evm.clone(),
            proof: vec![],
        };
        assert!(evm_call.get_dispatch_info().call_weight.ref_time() > 0);
        assert_ok!(evm_call.dispatch_bypass_filter(RuntimeOrigin::none()));
        assert_eq!(ChannelNonces::<Test>::get(GenericNetworkId::EVM(chain)), 1);
        assert_noop!(
            Pallet::<Test>::validate_submission(chain.into(), &evm, &vec![]),
            Error::<Test>::InvalidNonce
        );

        let ton_network = ton::TonNetworkId::Mainnet;
        let ton_channel = ton::TonAddress::new(0, H256::repeat_byte(5));
        let ton = GenericCommitment::TON(ton::Commitment::Inbound(ton::InboundCommitment {
            channel: ton_channel,
            source: ton::TonAddress::new(0, H256::repeat_byte(6)),
            nonce: 1,
            transaction_id: ton::TonTransactionId {
                lt: 12,
                hash: H256::repeat_byte(7),
            },
            payload: vec![2].try_into().unwrap(),
        }));
        let ton_id = GenericNetworkId::TON(ton_network);
        assert_noop!(
            Pallet::<Test>::validate_submission(ton_id, &ton, &vec![]),
            Error::<Test>::InvalidNetwork
        );
        assert_noop!(
            Pallet::<Test>::register_ton_channel(
                RuntimeOrigin::signed(Keyring::Bob.into()),
                ton_network,
                ton_channel
            ),
            DispatchError::BadOrigin
        );
        assert_ok!(Pallet::<Test>::register_ton_channel(
            RuntimeOrigin::root(),
            ton_network,
            Default::default()
        ));
        assert_noop!(
            Pallet::<Test>::validate_submission(ton_id, &ton, &vec![]),
            Error::<Test>::InvalidSourceChannel
        );
        assert_ok!(Pallet::<Test>::register_ton_channel(
            RuntimeOrigin::root(),
            ton_network,
            ton_channel
        ));
        let call = Call::<Test>::submit {
            network_id: ton_id,
            commitment: ton.clone(),
            proof: vec![],
        };
        assert!(call.get_dispatch_info().call_weight.ref_time() > 0);
        assert_ok!(call.dispatch_bypass_filter(RuntimeOrigin::signed(Keyring::Bob.into())));
        assert_eq!(ChannelNonces::<Test>::get(ton_id), 1);
        assert_noop!(
            Pallet::<Test>::validate_submission(ton_id, &ton, &vec![]),
            Error::<Test>::InvalidNonce
        );
        assert_eq!(sp_io::storage::get(b"test/applied-messages").unwrap()[0], 2);
    });
}

#[test]
fn evm_fee_updates_and_reports_apply_authenticated_values() {
    use bridge_types::{evm, GenericCommitment, U256};
    use frame_support::dispatch::GetDispatchInfo;
    new_tester().execute_with(|| {
        let chain: EVMChainId = [1; 32].into();
        let update = GenericCommitment::EVM(evm::Commitment::BaseFeeUpdate(evm::BaseFeeUpdate {
            new_base_fee: 123u32.into(),
            evm_block_number: 100,
        }));
        assert_noop!(
            Pallet::<Test>::validate_submission(chain.into(), &update, &vec![]),
            Error::<Test>::InvalidBaseFeeUpdate
        );
        sp_io::storage::set(b"test/allow-base-fee", &[1]);
        let call = Call::<Test>::submit {
            network_id: chain.into(),
            commitment: update,
            proof: vec![],
        };
        assert!(call.get_dispatch_info().call_weight.ref_time() > 0);
        assert_ok!(call.dispatch_bypass_filter(RuntimeOrigin::none()));
        assert_eq!(
            sp_io::storage::get(b"test/base-fee").unwrap().to_vec(),
            (chain, U256::from(123u32), 100u64).encode()
        );
        let channel = H160::repeat_byte(4);
        let relayer = H160::repeat_byte(5);
        assert_ok!(Pallet::<Test>::register_evm_channel(
            RuntimeOrigin::root(),
            chain,
            channel
        ));
        let report = GenericCommitment::EVM(evm::Commitment::StatusReport(evm::StatusReport {
            channel,
            block_number: 101,
            relayer,
            nonce: 1,
            results: vec![true, false].try_into().unwrap(),
            gas_spent: 100u32.into(),
            base_fee: 10u32.into(),
        }));
        let call = Call::<Test>::submit {
            network_id: chain.into(),
            commitment: report,
            proof: vec![],
        };
        assert!(call.get_dispatch_info().call_weight.ref_time() > 0);
        assert_ok!(call.dispatch_bypass_filter(RuntimeOrigin::none()));
        let expected =
            U256::from(100u64 + EVM_GAS_OVERHEAD).saturating_mul(U256::from(5_000_000_010u64));
        assert_eq!(
            sp_io::storage::get(b"test/fee-paid").unwrap().to_vec(),
            (chain, relayer, expected).encode()
        );
        assert_eq!(
            ReportedChannelNonces::<Test>::get(GenericNetworkId::EVM(chain)),
            1
        );
    });
}

#[test]
#[allow(deprecated)]
fn app_registry_encodes_requests_and_unsigned_validation_rejects_configuration_calls() {
    use bridge_types::traits::AppRegistry;
    new_tester().execute_with(|| {
        let chain: EVMChainId = [1; 32].into();
        let channel = H160::repeat_byte(4);
        let app = H160::repeat_byte(5);
        assert_noop!(
            Pallet::<Test>::register_app(chain, app),
            Error::<Test>::InvalidNetwork
        );
        assert_noop!(
            Pallet::<Test>::deregister_app(chain, app),
            Error::<Test>::InvalidNetwork
        );
        assert_ok!(Pallet::<Test>::register_evm_channel(
            RuntimeOrigin::root(),
            chain,
            channel
        ));
        for register in [true, false] {
            let payload = if register {
                assert_ok!(Pallet::<Test>::register_app(chain, app));
                bridge_types::channel_abi::RegisterAppPayload { app }
                    .encode()
                    .unwrap()
            } else {
                assert_ok!(Pallet::<Test>::deregister_app(chain, app));
                bridge_types::channel_abi::RemoveAppPayload { app }
                    .encode()
                    .unwrap()
            };
            assert_eq!(
                sp_io::storage::get(b"test/outbound").unwrap().to_vec(),
                (
                    chain,
                    payload,
                    AdditionalEVMOutboundData {
                        target: channel,
                        max_gas: 100_000u64.into()
                    }
                )
                    .encode()
            );
        }
        let call = Call::<Test>::register_evm_channel {
            network_id: chain,
            channel_address: channel,
        };
        assert_eq!(
            Pallet::<Test>::validate_unsigned(TransactionSource::External, &call),
            InvalidTransaction::Call.into()
        );
    });
}

#[test]
fn quarantined_inbound_messages_are_rejected_for_each_protocol() {
    use bridge_types::{evm, ton, GenericCommitment};
    new_tester().execute_with(|| {
        let chain: EVMChainId = [1; 32].into();
        let channel = H160::repeat_byte(4);
        assert_ok!(Pallet::<Test>::register_evm_channel(
            RuntimeOrigin::root(),
            chain,
            channel
        ));
        assert_ok!(Pallet::<Test>::register_ton_channel(
            RuntimeOrigin::root(),
            ton::TonNetworkId::Mainnet,
            Default::default()
        ));
        let cases = [
            (
                GenericNetworkId::EVM(chain),
                GenericCommitment::EVM(evm::Commitment::Inbound(evm::InboundCommitment {
                    channel,
                    source: channel,
                    nonce: 1,
                    block_number: 1,
                    payload: Default::default(),
                })),
                MessageId::basic(chain.into(), ThisNetworkId::get(), 1),
            ),
            (
                GenericNetworkId::TON(ton::TonNetworkId::Mainnet),
                GenericCommitment::TON(ton::Commitment::Inbound(ton::InboundCommitment {
                    channel: Default::default(),
                    source: Default::default(),
                    nonce: 1,
                    transaction_id: Default::default(),
                    payload: Default::default(),
                })),
                MessageId::basic(
                    GenericNetworkId::TON(ton::TonNetworkId::Mainnet),
                    ThisNetworkId::get(),
                    1,
                ),
            ),
            (
                BASE_NETWORK_ID,
                GenericCommitment::Sub(bridge_types::substrate::Commitment {
                    nonce: 1,
                    messages: vec![BridgeMessage {
                        timepoint: Default::default(),
                        payload: Default::default(),
                    }]
                    .try_into()
                    .unwrap(),
                }),
                MessageId::batched(BASE_NETWORK_ID, ThisNetworkId::get(), 1, 0),
            ),
        ];
        for (network, commitment, id) in cases {
            sp_io::storage::set(
                &bridge_types::recovery::message_key(network, id.hash()),
                &[1],
            );
            assert_noop!(
                Pallet::<Test>::validate_submission(network, &commitment, &vec![]),
                Error::<Test>::RecoveryHalted
            );
            assert_eq!(ChannelNonces::<Test>::get(network), 0);
        }
        System::set_block_number(u64::from(u32::MAX) + 1);
        sp_io::storage::set(crate::recovery::HALT_KEY, &[1]);
        sp_io::storage::set(crate::recovery::INBOUND_CONTEXT_KEY, &0u32.to_le_bytes());
        assert!(!crate::recovery::inbound_allowed::<Test>());
    });
}

#[test]
fn evm_outbound_commitments_and_root_submissions_are_rejected() {
    use bridge_types::{evm, GenericCommitment};
    use frame_support::dispatch::GetDispatchInfo;
    new_tester().execute_with(|| {
        let chain: EVMChainId = [1; 32].into();
        let commitment =
            GenericCommitment::EVM(evm::Commitment::Outbound(evm::OutboundCommitment {
                messages: Default::default(),
                total_max_gas: Default::default(),
                nonce: 1,
            }));
        let call = Call::<Test>::submit {
            network_id: chain.into(),
            commitment,
            proof: vec![],
        };
        assert!(call.get_dispatch_info().call_weight.ref_time() > 0);
        assert_noop!(
            call.clone().dispatch_bypass_filter(RuntimeOrigin::none()),
            Error::<Test>::InvalidCommitment
        );
        assert_noop!(
            call.dispatch_bypass_filter(RuntimeOrigin::root()),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn app_registration_propagates_channel_failure_without_mutation() {
    use bridge_types::traits::AppRegistry;
    new_tester().execute_with(|| {
        let chain = H256::repeat_byte(72);
        EVMChannelAddresses::<Test>::insert(chain, H160::repeat_byte(1));
        sp_io::storage::set(b"test/outbound-fails", &[1]);
        assert_noop!(
            Pallet::<Test>::register_app(chain, H160::repeat_byte(2)),
            DispatchError::Unavailable
        );
        assert_noop!(
            Pallet::<Test>::deregister_app(chain, H160::repeat_byte(2)),
            DispatchError::Unavailable
        );
    });
}
