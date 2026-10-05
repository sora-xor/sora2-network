// This file is part of the SORA network and Polkaswap app.
// SPDX-License-Identifier: BSD-4-Clause

use super::*;
use crate::{Executive, UncheckedExtrinsic, XorFee};
use codec::Decode;
use common::keeper::{submit, SubmitStatus};
use sp_runtime::{generic::Preamble, FixedPointNumber, FixedU128};

fn nonce(encoded: &[u8]) -> u32 {
    let tx = UncheckedExtrinsic::decode(&mut &encoded[..]).unwrap();
    match tx.preamble {
        Preamble::Signed(_, _, extra) => extra.4 .0,
        _ => panic!("keeper transaction must be signed"),
    }
}

#[test]
fn both_keeper_pallets_share_durable_nonces_and_pay_through_executive() {
    let mut externalities = ext();
    let (offchain, _) = sp_runtime::offchain::testing::TestOffchainExt::new();
    let (pool, pool_state) = sp_runtime::offchain::testing::TestTransactionPoolExt::new();
    externalities.register_extension(sp_core::offchain::OffchainDbExt::new(offchain.clone()));
    externalities.register_extension(sp_core::offchain::OffchainWorkerExt::new(offchain));
    externalities.register_extension(sp_core::offchain::TransactionPoolExt::new(pool));
    externalities.register_extension(sp_keystore::KeystoreExt::new(
        sp_keystore::testing::MemoryKeystore::new(),
    ));
    externalities.execute_with(|| {
        System::set_block_number(1);
        // Real blocks retain hashes used by CheckMortality when signing.
        frame_system::BlockHash::<Runtime>::insert(0, sp_core::H256::repeat_byte(1));
        frame_system::BlockHash::<Runtime>::insert(1, sp_core::H256::repeat_byte(2));
        let public =
            sp_io::crypto::sr25519_generate(kensetsu::crypto::KEY_TYPE, Some(b"//Alice".to_vec()));
        let who = AccountId::from(public);
        drop(Balances::deposit_creating(&who, 1_000_000 * UNIT));
        assert_ok!(XorFee::update_multiplier(
            RuntimeOrigin::root(),
            FixedU128::saturating_from_integer(1)
        ));
        let accrue = kensetsu::Call::<Runtime>::accrue {
            cdp_id: u32::MAX.into(),
        };
        let liquidate = apollo_platform::Call::<Runtime>::liquidate {
            user: AccountId::from([93; 32]),
            asset_id: common::XOR.into(),
        };
        assert_eq!(
            submit::<Runtime, _, kensetsu::crypto::AuthorityId>(accrue.clone()),
            Ok(SubmitStatus::Submitted)
        );
        assert_eq!(
            submit::<Runtime, _, apollo_platform::crypto::AuthorityId>(liquidate.clone()),
            Ok(SubmitStatus::Submitted)
        );
        let first = pool_state.read().transactions.clone();
        assert_eq!(
            first.iter().map(|tx| nonce(tx)).collect::<Vec<_>>(),
            vec![0, 1]
        );

        // Pending transactions must survive another offchain invocation without
        // changing nonces or creating replacements for the same operation.
        System::set_block_number(2);
        assert_eq!(
            submit::<Runtime, _, kensetsu::crypto::AuthorityId>(accrue),
            Ok(SubmitStatus::AlreadyPending)
        );
        assert_eq!(
            submit::<Runtime, _, apollo_platform::crypto::AuthorityId>(liquidate),
            Ok(SubmitStatus::AlreadyPending)
        );
        let retried = pool_state.read().transactions.clone();
        assert_eq!(retried.len(), 4);
        assert_eq!(&retried[2..], first.as_slice());

        for encoded in first {
            frame_system::BlockSize::<Runtime>::kill();
            frame_system::BlockWeight::<Runtime>::kill();
            System::reset_events();
            let before = Balances::free_balance(&who);
            let tx = UncheckedExtrinsic::decode(&mut &encoded[..]).unwrap();
            // The missing CDP/loan deliberately makes dispatch fail, while
            // admission, the real signature and payment extension all execute.
            assert!(Executive::apply_extrinsic(tx).unwrap().is_err());
            let charged = before - Balances::free_balance(&who);
            assert!(charged > 0);
            let recorded = System::events()
                .iter()
                .find_map(|record| match &record.event {
                    RuntimeEvent::TransactionPayment(
                        pallet_transaction_payment::Event::TransactionFeePaid {
                            actual_fee, ..
                        },
                    ) => Some(*actual_fee),
                    _ => None,
                })
                .unwrap();
            assert_eq!(recorded, charged);
        }
        assert_eq!(System::account_nonce(&who), 2);
        assert_eq!(
            submit::<Runtime, _, kensetsu::crypto::AuthorityId>(kensetsu::Call::liquidate {
                cdp_id: u32::MAX.into()
            }),
            Ok(SubmitStatus::Submitted)
        );
        assert_eq!(nonce(pool_state.read().transactions.last().unwrap()), 2);
    });
}
