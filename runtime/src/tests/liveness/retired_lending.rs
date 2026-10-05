// This file is part of the SORA network and Polkaswap app.
// SPDX-License-Identifier: BSD-4-Clause

use super::*;
use crate::{
    ApolloPlatform, Assets, Executive, Kensetsu, RuntimeCall, SignedExtra, UncheckedExtrinsic,
    XorFee,
};
use common::{AssetInfoProvider, APOLLO_ASSET_ID, KUSD, PSWAP, VAL};
use frame_support::{
    dispatch::{GetDispatchInfo, Pays},
    traits::{Get, Hooks},
    PalletId,
};
use sp_core::sr25519;
use sp_runtime::{
    generic::Era, traits::AccountIdConversion, DispatchResult, FixedPointNumber, FixedU128,
};

fn setup() -> sr25519::Pair {
    assert!(<<Runtime as kensetsu::Config>::RepaymentOnly as Get<
        bool,
    >>::get());
    assert!(<<Runtime as apollo_platform::Config>::RepaymentOnly as Get<bool>>::get());
    System::set_block_number(1);
    assert_ok!(XorFee::update_multiplier(
        RuntimeOrigin::root(),
        FixedU128::saturating_from_integer(1),
    ));
    let signer = sr25519::Pair::from_seed(&[77; 32]);
    drop(Balances::deposit_creating(
        &AccountId::from(signer.public()),
        1_000_000 * UNIT,
    ));
    signer
}

fn signed(call: RuntimeCall, signer: &sr25519::Pair) -> UncheckedExtrinsic {
    let who = AccountId::from(signer.public());
    let extra: SignedExtra = (
        frame_system::CheckSpecVersion::new(),
        frame_system::CheckTxVersion::new(),
        frame_system::CheckGenesis::new(),
        frame_system::CheckEra::from(Era::Immortal),
        frame_system::CheckNonce::from(System::account_nonce(&who)),
        frame_system::CheckWeight::new(),
        crate::charge_tx_payment_extension(),
    );
    let payload = crate::SignedPayload::new(call.clone(), extra.clone()).unwrap();
    let signature = payload.using_encoded(|bytes| signer.sign(bytes));
    UncheckedExtrinsic::new_signed(call, who, crate::Signature::Sr25519(signature), extra)
}

fn apply_paid(call: RuntimeCall, signer: &sr25519::Pair) -> DispatchResult {
    frame_system::BlockSize::<Runtime>::kill();
    frame_system::BlockWeight::<Runtime>::kill();
    System::reset_events();
    assert_eq!(call.get_dispatch_info().pays_fee, Pays::Yes);
    let who = AccountId::from(signer.public());
    let before = Balances::free_balance(&who);
    let nonce = System::account_nonce(&who);
    let result = Executive::apply_extrinsic(signed(call, signer)).expect("funded call admitted");
    let charged = before - Balances::free_balance(&who);
    assert!(charged > 0);
    assert_eq!(System::account_nonce(&who), nonce + 1);
    let emitted = System::events()
        .into_iter()
        .find_map(|record| match record.event {
            RuntimeEvent::TransactionPayment(
                pallet_transaction_payment::Event::TransactionFeePaid {
                    who: payer,
                    actual_fee,
                    tip,
                },
            ) if payer == who => {
                assert_eq!(tip, 0);
                Some(actual_fee)
            }
            _ => None,
        })
        .expect("payment extension emits retained XOR fee");
    assert_eq!(emitted, charged);
    result
}

fn pallet_state(name: &[u8]) -> Vec<(Vec<u8>, Vec<u8>)> {
    let prefix = sp_io::hashing::twox_128(name).to_vec();
    let mut key = prefix.clone();
    let mut state = Vec::new();
    while let Some(next) = sp_io::storage::next_key(&key) {
        if !next.starts_with(&prefix) {
            break;
        }
        state.push((next.clone(), sp_io::storage::get(&next).unwrap().to_vec()));
        key = next;
    }
    state
}

fn fund_asset(who: &AccountId, asset: crate::AssetId, amount: Balance) {
    assert_ok!(Assets::update_balance(
        RuntimeOrigin::root(),
        who.clone(),
        asset,
        amount as i128
    ));
}

fn asset_balance(who: &AccountId, asset: crate::AssetId) -> Balance {
    Assets::free_balance(&asset, who).unwrap()
}

#[test]
fn retired_kensetsu_operations_are_paid_failures_even_in_batches() {
    ext().execute_with(|| {
        let signer = setup();
        for call in [
            kensetsu::Call::create_cdp {
                collateral_asset_id: PSWAP,
                collateral_amount: UNIT,
                stablecoin_asset_id: KUSD,
                borrow_amount_min: 0,
                borrow_amount_max: UNIT,
                cdp_type: kensetsu::CdpType::Type2,
            },
            kensetsu::Call::deposit_collateral {
                cdp_id: 1,
                collateral_amount: UNIT,
            },
            kensetsu::Call::borrow {
                cdp_id: 1,
                borrow_amount_min: 0,
                borrow_amount_max: UNIT,
            },
            kensetsu::Call::accrue { cdp_id: 1 },
            kensetsu::Call::liquidate { cdp_id: 1 },
            kensetsu::Call::donate {
                stablecoin_asset_id: KUSD,
                amount: UNIT,
            },
        ] {
            let direct = RuntimeCall::Kensetsu(call);
            let wrapped = RuntimeCall::Utility(pallet_utility::Call::batch_all {
                calls: vec![direct.clone()],
            });
            for transaction in [direct, wrapped] {
                let before = pallet_state(b"Kensetsu");
                assert_eq!(
                    apply_paid(transaction, &signer).unwrap_err(),
                    kensetsu::Error::<Runtime>::RepaymentOnly.into()
                );
                assert_eq!(pallet_state(b"Kensetsu"), before);
            }
        }
    });
}

#[test]
fn retired_apollo_operations_are_paid_failures_even_in_batches() {
    ext().execute_with(|| {
        let signer = setup();
        for call in [
            apollo_platform::Call::lend {
                lending_asset: VAL,
                lending_amount: UNIT,
            },
            apollo_platform::Call::borrow {
                collateral_asset: VAL,
                borrowing_asset: PSWAP,
                borrowing_amount: UNIT,
                loan_to_value: UNIT,
            },
            apollo_platform::Call::add_collateral {
                collateral_asset: VAL,
                collateral_amount: UNIT,
                borrowing_asset: PSWAP,
            },
            apollo_platform::Call::liquidate {
                user: AccountId::from(signer.public()),
                asset_id: PSWAP,
            },
        ] {
            let direct = RuntimeCall::ApolloPlatform(call);
            let wrapped = RuntimeCall::Utility(pallet_utility::Call::batch_all {
                calls: vec![direct.clone()],
            });
            for transaction in [direct, wrapped] {
                let before = pallet_state(b"ApolloPlatform");
                assert_eq!(
                    apply_paid(transaction, &signer).unwrap_err(),
                    apollo_platform::Error::<Runtime>::RepaymentOnly.into()
                );
                assert_eq!(pallet_state(b"ApolloPlatform"), before);
            }
        }
    });
}

fn seed_cdp(who: &AccountId) {
    // State from before retirement: 200 PSWAP collateral backs 100 KUSD debt.
    // A zero-rate fixture isolates the exit from the separate interest policy.
    kensetsu::CDPDepository::<Runtime>::insert(
        1,
        kensetsu::CollateralizedDebtPosition {
            owner: who.clone(),
            collateral_asset_id: PSWAP,
            collateral_amount: 200 * UNIT,
            stablecoin_asset_id: KUSD,
            debt: 100 * UNIT,
            interest_coefficient: FixedU128::saturating_from_integer(1),
        },
    );
    kensetsu::CdpOwnerIndex::<Runtime>::insert(
        who,
        frame_support::BoundedVec::<_, <Runtime as kensetsu::Config>::MaxCdpsPerOwner>::try_from(
            vec![1u128],
        )
        .unwrap(),
    );
    kensetsu::CollateralInfos::<Runtime>::insert(
        kensetsu::StablecoinCollateralIdentifier {
            collateral_asset_id: PSWAP,
            stablecoin_asset_id: KUSD,
        },
        kensetsu::CollateralInfo {
            risk_parameters: kensetsu::CollateralRiskParameters::default(),
            total_collateral: 200 * UNIT,
            stablecoin_supply: 100 * UNIT,
            last_fee_update_time: 0,
            interest_coefficient: FixedU128::saturating_from_integer(1),
        },
    );
    fund_asset(
        &crate::KensetsuDepositoryAccountId::get(),
        PSWAP,
        200 * UNIT,
    );
    fund_asset(who, KUSD, 100 * UNIT);
}

#[test]
fn retired_kensetsu_partial_repayment_and_close_unlock_existing_collateral() {
    ext().execute_with(|| {
        let signer = setup();
        let who = AccountId::from(signer.public());
        seed_cdp(&who);
        let before = asset_balance(&who, PSWAP);
        assert_ok!(apply_paid(
            RuntimeCall::Kensetsu(kensetsu::Call::repay_debt {
                cdp_id: 1,
                amount: 40 * UNIT
            }),
            &signer
        ));
        assert_eq!(Kensetsu::cdp(1).unwrap().debt, 60 * UNIT);
        assert_eq!(asset_balance(&who, KUSD), 60 * UNIT);
        assert_eq!(asset_balance(&who, PSWAP), before);
        assert_ok!(apply_paid(
            RuntimeCall::Kensetsu(kensetsu::Call::close_cdp { cdp_id: 1 }),
            &signer
        ));
        assert!(Kensetsu::cdp(1).is_none());
        assert!(Kensetsu::cdp_owner_index(&who).is_none());
        assert_eq!(asset_balance(&who, KUSD), 0);
        assert_eq!(asset_balance(&who, PSWAP), before + 200 * UNIT);
        let collateral = Kensetsu::collateral_infos(kensetsu::StablecoinCollateralIdentifier {
            collateral_asset_id: PSWAP,
            stablecoin_asset_id: KUSD,
        })
        .unwrap();
        assert_eq!(collateral.total_collateral, 0);
        assert_eq!(collateral.stablecoin_supply, 0);
    });
}

fn apollo_account() -> AccountId {
    PalletId(*b"apollolb").into_account_truncating()
}

fn seed_apollo_borrow(who: &AccountId) {
    apollo_platform::PoolData::<Runtime>::insert(
        PSWAP,
        apollo_platform::PoolInfo {
            total_borrowed: 100 * UNIT,
            ..Default::default()
        },
    );
    apollo_platform::PoolData::<Runtime>::insert(
        VAL,
        apollo_platform::PoolInfo {
            total_collateral: 200 * UNIT,
            ..Default::default()
        },
    );
    apollo_platform::UserBorrowingInfo::<Runtime>::insert(
        PSWAP,
        who,
        std::collections::BTreeMap::from([(
            VAL,
            apollo_platform::BorrowingPosition {
                collateral_amount: 200 * UNIT,
                borrowing_amount: 100 * UNIT,
                last_borrowing_block: 1,
                ..Default::default()
            },
        )]),
    );
    apollo_platform::UserTotalCollateral::<Runtime>::insert(who, VAL, 200 * UNIT);
    fund_asset(who, PSWAP, 100 * UNIT);
    fund_asset(&apollo_account(), VAL, 200 * UNIT);
}

#[test]
fn retired_apollo_repayment_unlocks_existing_collateral() {
    ext().execute_with(|| {
        let signer = setup();
        let who = AccountId::from(signer.public());
        seed_apollo_borrow(&who);
        let collateral_before = asset_balance(&who, VAL);
        assert_ok!(apply_paid(
            RuntimeCall::ApolloPlatform(apollo_platform::Call::repay {
                collateral_asset: VAL,
                borrowing_asset: PSWAP,
                amount_to_repay: 40 * UNIT
            }),
            &signer
        ));
        assert_eq!(
            ApolloPlatform::user_borrowing_info(PSWAP, &who).unwrap()[&VAL].borrowing_amount,
            60 * UNIT
        );
        assert_eq!(asset_balance(&who, VAL), collateral_before);
        assert_ok!(apply_paid(
            RuntimeCall::ApolloPlatform(apollo_platform::Call::repay {
                collateral_asset: VAL,
                borrowing_asset: PSWAP,
                amount_to_repay: 60 * UNIT
            }),
            &signer
        ));
        assert!(ApolloPlatform::user_borrowing_info(PSWAP, &who).is_none());
        assert_eq!(asset_balance(&who, PSWAP), 0);
        assert_eq!(asset_balance(&who, VAL), collateral_before + 200 * UNIT);
        assert!(!apollo_platform::UserTotalCollateral::<Runtime>::contains_key(&who, VAL));
        assert_eq!(ApolloPlatform::pool_info(PSWAP).unwrap().total_borrowed, 0);
        assert_eq!(
            ApolloPlatform::pool_info(PSWAP).unwrap().total_liquidity,
            100 * UNIT
        );
        assert_eq!(ApolloPlatform::pool_info(VAL).unwrap().total_collateral, 0);
    });
}

#[test]
fn retired_apollo_last_lender_withdraws_exact_remaining_liquidity() {
    ext().execute_with(|| {
        let signer = setup();
        let who = AccountId::from(signer.public());
        apollo_platform::PoolData::<Runtime>::insert(
            VAL,
            apollo_platform::PoolInfo {
                total_liquidity: 100 * UNIT,
                ..Default::default()
            },
        );
        apollo_platform::UserLendingInfo::<Runtime>::insert(
            VAL,
            &who,
            apollo_platform::LendingPosition {
                lending_amount: 100 * UNIT,
                last_lending_block: 1,
                ..Default::default()
            },
        );
        fund_asset(&apollo_account(), VAL, 100 * UNIT);
        let before = asset_balance(&who, VAL);
        assert_ok!(apply_paid(
            RuntimeCall::ApolloPlatform(apollo_platform::Call::withdraw {
                withdrawn_asset: VAL,
                withdrawn_amount: 100 * UNIT
            }),
            &signer
        ));
        assert_eq!(asset_balance(&who, VAL), before + 100 * UNIT);
        assert!(ApolloPlatform::user_lending_info(VAL, &who).is_none());
        assert_eq!(ApolloPlatform::pool_info(VAL).unwrap().total_liquidity, 0);
    });
}

#[test]
fn retired_apollo_exit_preserves_unfunded_reward_claims() {
    ext().execute_with(|| {
        let signer = setup();
        let who = AccountId::from(signer.public());
        seed_apollo_borrow(&who);
        apollo_platform::UserBorrowingInfo::<Runtime>::mutate(PSWAP, &who, |positions| {
            positions
                .as_mut()
                .unwrap()
                .get_mut(&VAL)
                .unwrap()
                .borrowing_rewards = 7 * UNIT;
        });
        apollo_platform::PoolData::<Runtime>::mutate(VAL, |pool| {
            pool.as_mut().unwrap().total_liquidity = 100 * UNIT;
        });
        apollo_platform::UserLendingInfo::<Runtime>::insert(
            VAL,
            &who,
            apollo_platform::LendingPosition {
                lending_amount: 100 * UNIT,
                lending_interest: 5 * UNIT,
                last_lending_block: 1,
            },
        );
        fund_asset(&apollo_account(), VAL, 100 * UNIT);
        assert_eq!(asset_balance(&apollo_account(), APOLLO_ASSET_ID), 0);
        let before = asset_balance(&who, VAL);
        assert_ok!(apply_paid(
            RuntimeCall::ApolloPlatform(apollo_platform::Call::repay {
                collateral_asset: VAL,
                borrowing_asset: PSWAP,
                amount_to_repay: 100 * UNIT,
            }),
            &signer
        ));
        assert_ok!(apply_paid(
            RuntimeCall::ApolloPlatform(apollo_platform::Call::withdraw {
                withdrawn_asset: VAL,
                withdrawn_amount: 100 * UNIT,
            }),
            &signer
        ));
        assert_eq!(asset_balance(&who, VAL), before + 300 * UNIT);
        let positions = ApolloPlatform::user_borrowing_info(PSWAP, &who).unwrap();
        assert_eq!(positions[&VAL].borrowing_amount, 0);
        assert_eq!(positions[&VAL].collateral_amount, 0);
        assert_eq!(positions[&VAL].borrowing_rewards, 7 * UNIT);
        let lending = ApolloPlatform::user_lending_info(VAL, &who).unwrap();
        assert_eq!(lending.lending_amount, 0);
        assert_eq!(lending.lending_interest, 5 * UNIT);
        // A missing reward balance must leave both earned claims available;
        // principal and collateral exit has already completed independently.
        let claims = pallet_state(b"ApolloPlatform");
        assert!(apply_paid(
            RuntimeCall::ApolloPlatform(apollo_platform::Call::get_rewards {
                asset_id: PSWAP,
                is_lending: false,
            }),
            &signer
        )
        .is_err());
        assert_eq!(pallet_state(b"ApolloPlatform"), claims);
        fund_asset(&apollo_account(), APOLLO_ASSET_ID, 12 * UNIT);
        for (asset_id, is_lending) in [(PSWAP, false), (VAL, true)] {
            assert_ok!(apply_paid(
                RuntimeCall::ApolloPlatform(apollo_platform::Call::get_rewards {
                    asset_id,
                    is_lending,
                }),
                &signer
            ));
        }
        assert_eq!(asset_balance(&who, APOLLO_ASSET_ID), 12 * UNIT);
        assert_eq!(asset_balance(&apollo_account(), APOLLO_ASSET_ID), 0);
    });
}

#[test]
fn retired_lending_workers_submit_nothing_with_funded_keeper_keys() {
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
        let _ = setup();
        let public =
            sp_io::crypto::sr25519_generate(kensetsu::crypto::KEY_TYPE, Some(b"//Alice".to_vec()));
        let keeper = AccountId::from(public);
        drop(Balances::deposit_creating(&keeper, 1_000_000 * UNIT));
        assert!(sp_io::crypto::sr25519_public_keys(kensetsu::crypto::KEY_TYPE).contains(&public));
        seed_cdp(&keeper);
        seed_apollo_borrow(&keeper);
        // This debt would accrue well above its threshold in an active worker;
        // silence must not depend on an empty CDP map or missing signer funds.
        kensetsu::CollateralInfos::<Runtime>::mutate(
            kensetsu::StablecoinCollateralIdentifier {
                collateral_asset_id: PSWAP,
                stablecoin_asset_id: KUSD,
            },
            |info| {
                info.as_mut().unwrap().risk_parameters.stability_fee_rate =
                    FixedU128::saturating_from_rational(1, 100)
            },
        );
        kensetsu::StablecoinInfos::<Runtime>::mutate(KUSD, |info| {
            info.as_mut()
                .unwrap()
                .stablecoin_parameters
                .minimal_stability_fee_accrue = UNIT;
        });
        pallet_timestamp::Now::<Runtime>::put(100_000);
        let kensetsu_before = pallet_state(b"Kensetsu");
        let apollo_before = pallet_state(b"ApolloPlatform");
        for block in [1, 2, 1000] {
            System::set_block_number(block);
            <Kensetsu as Hooks<crate::BlockNumber>>::offchain_worker(block);
            <ApolloPlatform as Hooks<crate::BlockNumber>>::offchain_worker(block);
        }
        assert!(pool_state.read().transactions.is_empty());
        assert_eq!(pallet_state(b"Kensetsu"), kensetsu_before);
        assert_eq!(pallet_state(b"ApolloPlatform"), apollo_before);
        assert_eq!(System::account_nonce(&keeper), 0);
    });
}
