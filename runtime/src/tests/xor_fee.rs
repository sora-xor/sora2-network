#![allow(deprecated)]

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

use crate::mock::{ensure_pool_initialized, fill_spot_price};
use crate::xor_fee_impls::{CustomFeeDetails, CustomFees};
use crate::{
    charge_tx_payment_extension, AccountId, AssetId, Assets, Balance, Balances, Currencies,
    FeeReferrerWeight, FeeValBurnedWeight, FeeXorBurnedWeight, ForcedMultiplierAt,
    ForcedMultiplierValue, GetXorFeeAccountId, PoolXYK, Referrals, RemintXorBurnPercent, Runtime,
    RuntimeCall, RuntimeEvent, RuntimeOrigin, Signature, SignedExtra, Staking, System, Tokens,
    UncheckedExtrinsic, Weight, XorFee,
};
use codec::Encode;
use common::mock::{alice, bob, charlie};
use common::prelude::constants::{BIG_FEE, SMALL_FEE};
use common::prelude::{AssetName, AssetSymbol, FixedWrapper, SwapAmount};
#[cfg(feature = "wip")] // Xorless fee
use common::XykPool;
use common::{
    assert_approx_eq_abs, balance, fixed_wrapper, AssetInfoProvider, DEXId, FilterMode,
    OrderBookId, PriceVariant, DOT, KUSD, TBCD, VAL, XOR,
};
use frame_support::dispatch::{DispatchInfo, PostDispatchInfo};
use frame_support::pallet_prelude::{InvalidTransaction, Pays};
use frame_support::traits::{OnFinalize, OnInitialize};
use frame_support::unsigned::TransactionValidityError;
use frame_support::weights::WeightToFee as WeightToFeeTrait;
use frame_support::{assert_err, assert_ok};
use frame_system::EventRecord;
use framenode_chain_spec::ext;
use pallet_balances::NegativeImbalance;
use pallet_transaction_payment::OnChargeTransaction;
use referrals::ReferrerBalances;
use sp_core::{sr25519, twox_128, Pair as _};
use sp_runtime::generic;
use sp_runtime::traits::{Dispatchable, SignedExtension};
use sp_runtime::{AccountId32, DispatchError, FixedPointNumber, FixedU128};
use traits::MultiCurrency;

use vested_rewards::vesting_currencies::{LinearVestingSchedule, VestingScheduleVariant};

use xor_fee::extension::ChargeTransactionPayment;
use xor_fee::{
    ApplyCustomFees, LiquidityInfo, Multiplier as XorFeeMultiplier, XorToBuyBack, XorToVal,
};

type BlockWeights = <Runtime as frame_system::Config>::BlockWeights;
type LengthToFee = <Runtime as pallet_transaction_payment::Config>::LengthToFee;
type WeightToFee = <Runtime as pallet_transaction_payment::Config>::WeightToFee;

const MOCK_WEIGHT: Weight = Weight::from_parts(600_000_000, 0);

const INITIAL_BALANCE: Balance = balance!(1000);
const INITIAL_RESERVES: Balance = balance!(10000);
const TRANSFER_AMOUNT: Balance = balance!(69);

fn sora_parliament_account() -> AccountId {
    AccountId32::from([7; 32])
}

/// create a transaction info struct from weight. Handy to avoid building the whole struct.
fn info_from_weight(w: Weight) -> DispatchInfo {
    // pays_fee: Pays::Yes -- class: DispatchClass::Normal
    DispatchInfo {
        call_weight: w,
        extension_weight: Weight::zero(),
        ..Default::default()
    }
}

fn default_post_info() -> PostDispatchInfo {
    PostDispatchInfo {
        actual_weight: None,
        pays_fee: Default::default(),
    }
}

fn post_info_from_weight(w: Weight) -> PostDispatchInfo {
    PostDispatchInfo {
        actual_weight: Some(w),
        pays_fee: Default::default(),
    }
}

fn post_info_pays_no() -> PostDispatchInfo {
    PostDispatchInfo {
        actual_weight: None,
        pays_fee: Pays::No,
    }
}

fn assert_val_staking_reward_recorded(
    active_era: Option<sp_staking::EraIndex>,
    val_burned: Balance,
) {
    let staking_reward =
        val_burned.saturating_sub(crate::constants::rewards::VAL_BURN_PERCENT * val_burned);
    match active_era {
        Some(era) => assert_approx_eq_abs!(
            xor_fee::ValStakingEraReward::<Runtime>::get(era),
            staking_reward,
            balance!(0.00005)
        ),
        None => assert_approx_eq_abs!(
            xor_fee::UnassignedValStakingReward::<Runtime>::get(),
            staking_reward,
            balance!(0.00005)
        ),
    }
}

fn length_fee(len: usize) -> Balance {
    LengthToFee::weight_to_fee(&Weight::from_parts(len as u64, 0))
}

fn signed_extra() -> SignedExtra {
    let charge_tx_payment = charge_tx_payment_extension();
    (
        frame_system::CheckSpecVersion::<Runtime>::new(),
        frame_system::CheckTxVersion::<Runtime>::new(),
        frame_system::CheckGenesis::<Runtime>::new(),
        frame_system::CheckEra::<Runtime>::from(generic::Era::Immortal),
        frame_system::CheckNonce::<Runtime>::from(0),
        frame_system::CheckWeight::<Runtime>::new(),
        charge_tx_payment,
    )
}

fn signed_unchecked_extrinsic(call: RuntimeCall) -> UncheckedExtrinsic {
    UncheckedExtrinsic::new_signed(
        call,
        alice(),
        Signature::Sr25519(sr25519::Signature::from_raw([0; 64])),
        signed_extra(),
    )
}

fn signed_unchecked_extrinsic_from_pair(
    call: RuntimeCall,
    pair: &sr25519::Pair,
) -> UncheckedExtrinsic {
    let extra = signed_extra();
    let raw_payload = crate::SignedPayload::new(call.clone(), extra.clone()).unwrap();
    let signature = raw_payload.using_encoded(|payload| pair.sign(payload));

    UncheckedExtrinsic::new_signed(
        call,
        AccountId32::from(pair.public()),
        Signature::Sr25519(signature),
        extra,
    )
}

fn give_xor_initial_balance(target: AccountId) {
    increase_balance(target, XOR.into(), INITIAL_BALANCE);
}

fn increase_balance(target: AccountId, asset: AssetId, balance: Balance) {
    assert_ok!(Currencies::update_balance(
        RuntimeOrigin::root(),
        target,
        asset,
        balance as i128
    ));
}

fn set_weight_to_fee_multiplier(mul: u64) {
    // Set WeightToFee multiplier to one to not affect the test
    assert_ok!(XorFee::update_multiplier(
        RuntimeOrigin::root(),
        FixedU128::saturating_from_integer(mul)
    ));
}

#[test]
fn forced_multiplier_triggers_at_configured_block() {
    ext().execute_with(|| {
        let forced_at = ForcedMultiplierAt::get();
        assert!(forced_at > 0);

        let oversized =
            FixedU128::from_inner(1_486_296_111_770_910_720_000_000_000_000_000_000u128);
        XorFeeMultiplier::<Runtime>::put(oversized);

        let before_block = forced_at - 1;
        System::set_block_number(before_block);
        XorFee::on_initialize(before_block);
        assert_eq!(XorFeeMultiplier::<Runtime>::get(), oversized);

        System::set_block_number(forced_at);
        XorFee::on_initialize(forced_at);
        assert_eq!(
            XorFeeMultiplier::<Runtime>::get(),
            ForcedMultiplierValue::get()
        );
    });
}

#[cfg(feature = "wip")] // Xorless fee
fn add_asset_to_white_list_for_xorless(asset: AssetId) {
    assert_ok!(XorFee::add_asset_to_white_list(
        RuntimeOrigin::root(),
        asset,
    ));
}

#[cfg(feature = "wip")] // Xorless fee
#[test]
fn referrer_gets_bonus_from_xorless_tx_fee() {
    ext().execute_with(|| {
        set_weight_to_fee_multiplier(1);
        System::on_finalize(System::block_number());
        System::set_block_number(System::block_number() + 1);
        System::on_initialize(System::block_number());

        ensure_pool_initialized(XOR.into(), VAL.into());
        add_asset_to_white_list_for_xorless(VAL.into());

        increase_balance(bob(), XOR.into(), INITIAL_RESERVES);
        increase_balance(bob(), VAL.into(), INITIAL_RESERVES);
        increase_balance(alice(), VAL.into(), INITIAL_BALANCE);

        give_xor_initial_balance(alice());
        give_xor_initial_balance(charlie());

        Referrals::set_referrer_to(&alice(), charlie()).unwrap();

        PoolXYK::deposit_liquidity(
            RuntimeOrigin::signed(bob()),
            0,
            XOR.into(),
            VAL.into(),
            INITIAL_RESERVES,
            balance!(0.0000001),
            INITIAL_RESERVES,
            balance!(0.0000001),
        )
        .unwrap();

        fill_spot_price();

        let call: &<Runtime as frame_system::Config>::RuntimeCall =
            &RuntimeCall::XorFee(xor_fee::Call::xorless_call {
                call: Box::new(RuntimeCall::Assets(assets::Call::transfer {
                    asset_id: VAL.into(),
                    to: bob(),
                    amount: TRANSFER_AMOUNT,
                })),
                asset_id: VAL.into(),
            });

        let len = 10;
        let dispatch_info = info_from_weight(MOCK_WEIGHT);
        let len_fee = length_fee(len);
        let val_price = FixedWrapper::from(9999000);
        let val_fee = (SMALL_FEE + len_fee) * val_price;
        let balance_after_reserving_fee = (INITIAL_BALANCE - val_fee.clone()).into_balance();

        let pre = ChargeTransactionPayment::<Runtime>::new()
            .pre_dispatch(&alice(), call, &dispatch_info, len)
            .unwrap();

        assert_eq!(
            Currencies::free_balance(VAL.into(), &alice()),
            balance_after_reserving_fee
        );

        assert!(ChargeTransactionPayment::<Runtime>::post_dispatch(
            Some(pre),
            &dispatch_info,
            &default_post_info(),
            len,
            &Ok(())
        )
        .is_ok());
        assert_eq!(
            Currencies::free_balance(VAL.into(), &alice()),
            balance_after_reserving_fee
        );

        let referrer_fee = XorFee::calculate_portion_fee_from_weight(
            FeeReferrerWeight::get(),
            val_fee.into_balance(),
        );
        assert_eq!(
            Currencies::free_balance(VAL.into(), &charlie()),
            referrer_fee
        );

        assert_eq!(
            frame_system::Pallet::<Runtime>::events()
                .into_iter()
                .find_map(|EventRecord { event, .. }| match event {
                    crate::RuntimeEvent::XorFee(event) => {
                        if let xor_fee::Event::ReferrerRewarded(_, _, _, _) = event {
                            Some(event)
                        } else {
                            None
                        }
                    }
                    _ => None,
                }),
            Some(xor_fee::Event::ReferrerRewarded(
                alice(),
                charlie(),
                VAL,
                referrer_fee,
            ))
        );
    });
}

#[cfg(feature = "wip")] // Xorless fee
#[test]
fn fail_on_withdraw() {
    ext().execute_with(|| {
        set_weight_to_fee_multiplier(1);

        increase_balance(alice(), VAL.into(), INITIAL_BALANCE);

        give_xor_initial_balance(alice());
        increase_balance(bob(), XOR.into(), INITIAL_RESERVES);
        increase_balance(bob(), VAL.into(), INITIAL_RESERVES);
        ensure_pool_initialized(XOR.into(), VAL.into());
        PoolXYK::deposit_liquidity(
            RuntimeOrigin::signed(bob()),
            0,
            XOR.into(),
            VAL.into(),
            balance!(0.0007),
            100,
            balance!(0.0007),
            100,
        )
        .unwrap();

        let call: &<Runtime as frame_system::Config>::RuntimeCall =
            &RuntimeCall::XorFee(xor_fee::Call::xorless_call {
                call: Box::new(RuntimeCall::Assets(assets::Call::transfer {
                    asset_id: VAL.into(),
                    to: bob(),
                    amount: TRANSFER_AMOUNT,
                })),
                asset_id: VAL.into(),
            });

        let len = 10;
        let dispatch_info = info_from_weight(MOCK_WEIGHT);

        assert_err!(
            ChargeTransactionPayment::<Runtime>::new().pre_dispatch(
                &alice(),
                call,
                &dispatch_info,
                len
            ),
            TransactionValidityError::Invalid(InvalidTransaction::Custom(2))
        );

        assert_eq!(
            Currencies::free_balance(VAL.into(), &alice()),
            INITIAL_BALANCE
        );

        add_asset_to_white_list_for_xorless(VAL.into());

        assert_err!(
            ChargeTransactionPayment::<Runtime>::new().pre_dispatch(
                &alice(),
                call,
                &dispatch_info,
                len
            ),
            TransactionValidityError::Invalid(InvalidTransaction::Payment)
        );
    });
}

#[cfg(feature = "wip")] // Xorless fee
#[test]
fn remint_for_xorless_works() {
    ext().execute_with(|| {
        System::set_block_number(1);
        set_weight_to_fee_multiplier(1);

        Staking::on_finalize(0);

        add_asset_to_white_list_for_xorless(VAL.into());

        increase_balance(bob(), XOR.into(), 3 * INITIAL_RESERVES);
        increase_balance(alice(), VAL.into(), INITIAL_BALANCE);

        give_xor_initial_balance(alice());

        crate::TradingPair::register_pair(DEXId::Polkaswap.into(), XOR.into(), KUSD.into())
            .unwrap();

        for target in [KUSD, TBCD, VAL] {
            increase_balance(bob(), target.into(), 2 * INITIAL_RESERVES);
            ensure_pool_initialized(XOR.into(), target.into());
            PoolXYK::deposit_liquidity(
                RuntimeOrigin::signed(bob()),
                0,
                XOR.into(),
                target.into(),
                INITIAL_RESERVES,
                INITIAL_RESERVES,
                INITIAL_RESERVES,
                INITIAL_RESERVES,
            )
            .unwrap();
        }

        fill_spot_price();

        assert_eq!(rewards::ValBurnedSinceLastVesting::<Runtime>::get(), 0u128);

        let call: &<Runtime as frame_system::Config>::RuntimeCall =
            &RuntimeCall::XorFee(xor_fee::Call::xorless_call {
                call: Box::new(RuntimeCall::Assets(assets::Call::transfer {
                    asset_id: VAL.into(),
                    to: bob(),
                    amount: TRANSFER_AMOUNT,
                })),
                asset_id: VAL.into(),
            });

        let len = 10;
        let dispatch_info = info_from_weight(MOCK_WEIGHT);
        let val_price = 999900009999000099;
        let val_fee = FixedWrapper::from(SMALL_FEE + length_fee(len)) * val_price;
        let balance_after_reserving_fee =
            (INITIAL_BALANCE - val_fee.clone() - val_fee.clone()).into_balance();
        let val_fee = val_fee.into_balance();

        // call without referral
        let pre = ChargeTransactionPayment::<Runtime>::new()
            .pre_dispatch(&alice(), call, &dispatch_info, len)
            .unwrap();

        assert!(ChargeTransactionPayment::<Runtime>::post_dispatch(
            Some(pre),
            &dispatch_info,
            &default_post_info(),
            len,
            &Ok(())
        )
        .is_ok());

        Referrals::set_referrer_to(&alice(), charlie()).unwrap();

        // call with referral
        let pre = ChargeTransactionPayment::<Runtime>::new()
            .pre_dispatch(&alice(), call, &dispatch_info, len)
            .unwrap();

        assert!(ChargeTransactionPayment::<Runtime>::post_dispatch(
            Some(pre),
            &dispatch_info,
            &default_post_info(),
            len,
            &Ok(())
        )
        .is_ok());

        assert_eq!(
            Currencies::free_balance(VAL.into(), &alice()),
            balance_after_reserving_fee
        );

        assert_eq!(
            XorFee::burnt_for_fee(VAL),
            xor_fee::AssetFee {
                fee: val_fee,
                fee_without_referral: val_fee
            }
        );

        let asset_fee_in_xor = calc_xyk_swap_result(INITIAL_RESERVES, INITIAL_RESERVES, val_fee);
        let asset_fee_without_ref_in_xor = calc_xyk_swap_result(
            INITIAL_RESERVES - asset_fee_in_xor,
            INITIAL_RESERVES + val_fee,
            val_fee,
        );
        let total_asset_fee_in_xor = asset_fee_in_xor + asset_fee_without_ref_in_xor;

        let total_xor_to_val = XorFee::calculate_portion_fee_from_weight(
            FeeValBurnedWeight::get(),
            total_asset_fee_in_xor,
        );

        let active_era = pallet_staking::ActiveEra::<Runtime>::get().map(|era| era.index);
        xor_fee::Pallet::<Runtime>::on_initialize(1);
        assert!(xor_fee::BurntForFee::<Runtime>::iter().next().is_none());

        let xor_to_val_after_xor_burn =
            total_xor_to_val.saturating_sub(RemintXorBurnPercent::get() * total_xor_to_val);
        let val_burned = calc_xyk_swap_result(
            INITIAL_RESERVES + val_fee + val_fee,
            INITIAL_RESERVES - total_asset_fee_in_xor,
            xor_to_val_after_xor_burn,
        );

        assert_eq!(rewards::ValBurnedSinceLastVesting::<Runtime>::get(), 0);
        assert_val_staking_reward_recorded(active_era, val_burned);

        assert_approx_eq_abs!(
            Assets::total_issuance(&KUSD.into()).unwrap(),
            2 * INITIAL_RESERVES,
            balance!(0.00001)
        );

        assert_approx_eq_abs!(
            Assets::total_issuance(&TBCD.into()).unwrap(),
            2 * INITIAL_RESERVES,
            balance!(0.00001)
        );
    });
}

#[cfg(feature = "wip")] // Xorless fee
#[test]
fn remint_for_xorless_fails_right() {
    ext().execute_with(|| {
        System::set_block_number(1);
        set_weight_to_fee_multiplier(1);

        Staking::on_finalize(0);

        add_asset_to_white_list_for_xorless(VAL.into());

        increase_balance(bob(), XOR.into(), 5 * INITIAL_RESERVES);
        increase_balance(alice(), VAL.into(), INITIAL_BALANCE);

        give_xor_initial_balance(alice());

        crate::TradingPair::register_pair(DEXId::Polkaswap.into(), XOR.into(), KUSD.into())
            .unwrap();

        increase_balance(bob(), VAL.into(), 2 * INITIAL_RESERVES);
        ensure_pool_initialized(XOR.into(), VAL.into());
        PoolXYK::deposit_liquidity(
            RuntimeOrigin::signed(bob()),
            DEXId::Polkaswap.into(),
            XOR.into(),
            VAL.into(),
            INITIAL_RESERVES,
            INITIAL_RESERVES,
            INITIAL_RESERVES,
            INITIAL_RESERVES,
        )
        .unwrap();

        fill_spot_price();

        let call: &<Runtime as frame_system::Config>::RuntimeCall =
            &RuntimeCall::XorFee(xor_fee::Call::xorless_call {
                call: Box::new(RuntimeCall::Assets(assets::Call::transfer {
                    asset_id: VAL.into(),
                    to: bob(),
                    amount: TRANSFER_AMOUNT,
                })),
                asset_id: VAL.into(),
            });
        let len = 10;
        let dispatch_info = info_from_weight(MOCK_WEIGHT);

        // call without referral
        let pre = ChargeTransactionPayment::<Runtime>::new()
            .pre_dispatch(&alice(), call, &dispatch_info, len)
            .unwrap();
        assert!(ChargeTransactionPayment::<Runtime>::post_dispatch(
            Some(pre),
            &dispatch_info,
            &default_post_info(),
            len,
            &Ok(())
        )
        .is_ok());

        let pool_account = PoolXYK::properties_of_pool(XOR.into(), VAL.into())
            .unwrap()
            .0;
        assert_ok!(PoolXYK::withdraw_liquidity(
            RuntimeOrigin::signed(bob()),
            DEXId::Polkaswap.into(),
            XOR.into(),
            VAL.into(),
            pool_xyk::PoolProviders::<Runtime>::get(pool_account, bob()).unwrap(),
            INITIAL_RESERVES,
            INITIAL_RESERVES,
        ));
        xor_fee::Pallet::<Runtime>::on_initialize(1);
        assert!(xor_fee::BurntForFee::<Runtime>::iter().next().is_some());
    });
}

#[test]
fn referrer_gets_bonus_from_tx_fee() {
    ext().execute_with(|| {
        set_weight_to_fee_multiplier(1);
        System::on_finalize(System::block_number());
        System::set_block_number(System::block_number() + 1);
        System::on_initialize(System::block_number());
        give_xor_initial_balance(alice());
        give_xor_initial_balance(charlie());
        Referrals::set_referrer_to(&alice(), charlie()).unwrap();
        let call: &<Runtime as frame_system::Config>::RuntimeCall =
            &RuntimeCall::Assets(assets::Call::transfer {
                asset_id: VAL.into(),
                to: bob(),
                amount: TRANSFER_AMOUNT,
            });

        let len = 10;
        let dispatch_info = info_from_weight(MOCK_WEIGHT);
        let fee = SMALL_FEE + length_fee(len);
        let pre = ChargeTransactionPayment::<Runtime>::new()
            .pre_dispatch(&alice(), call, &dispatch_info, len)
            .unwrap();
        let balance_after_reserving_fee = FixedWrapper::from(INITIAL_BALANCE) - fee;
        let balance_after_reserving_fee = balance_after_reserving_fee.into_balance();
        assert_eq!(Balances::free_balance(alice()), balance_after_reserving_fee);
        assert!(ChargeTransactionPayment::<Runtime>::post_dispatch(
            Some(pre),
            &dispatch_info,
            &default_post_info(),
            len,
            &Ok(())
        )
        .is_ok());
        assert_eq!(Balances::free_balance(alice()), balance_after_reserving_fee);
        let weights_sum: FixedWrapper = FixedWrapper::from(balance!(FeeReferrerWeight::get()))
            + FixedWrapper::from(balance!(FeeXorBurnedWeight::get()))
            + FixedWrapper::from(balance!(FeeValBurnedWeight::get()));
        let referrer_weight = FixedWrapper::from(balance!(FeeReferrerWeight::get()));
        let initial_balance = FixedWrapper::from(INITIAL_BALANCE);
        let referrer_fee = fee * referrer_weight / weights_sum;
        let expected_referrer_balance = referrer_fee.clone() + initial_balance;
        #[cfg(feature = "wip")] // Xorless fee
        assert_eq!(
            frame_system::Pallet::<Runtime>::events()
                .into_iter()
                .find_map(|EventRecord { event, .. }| match event {
                    crate::RuntimeEvent::XorFee(event) => {
                        if let xor_fee::Event::ReferrerRewarded(_, _, _, _) = event {
                            Some(event)
                        } else {
                            None
                        }
                    }
                    _ => None,
                }),
            Some(xor_fee::Event::ReferrerRewarded(
                alice(),
                charlie(),
                XOR,
                referrer_fee.into_balance(),
            ))
        );
        #[cfg(not(feature = "wip"))] // Xorless fee
        assert_eq!(
            frame_system::Pallet::<Runtime>::events()
                .into_iter()
                .find_map(|EventRecord { event, .. }| match event {
                    crate::RuntimeEvent::XorFee(event) => {
                        if let xor_fee::Event::ReferrerRewarded(_, _, _) = event {
                            Some(event)
                        } else {
                            None
                        }
                    }
                    _ => None,
                }),
            Some(xor_fee::Event::ReferrerRewarded(
                alice(),
                charlie(),
                referrer_fee.into_balance(),
            ))
        );
        assert!(
            Balances::free_balance(charlie())
                >= (expected_referrer_balance.clone() - fixed_wrapper!(1)).into_balance()
                && Balances::free_balance(charlie())
                    <= (expected_referrer_balance + fixed_wrapper!(1)).into_balance()
        );
    });
}

#[test]
fn notify_val_burned_works() {
    ext().execute_with(|| {
        System::set_block_number(1);
        set_weight_to_fee_multiplier(1);
        increase_balance(alice(), XOR.into(), INITIAL_RESERVES);

        Staking::on_finalize(0);

        increase_balance(bob(), XOR.into(), 3 * INITIAL_RESERVES);

        crate::TradingPair::register_pair(DEXId::Polkaswap.into(), XOR.into(), KUSD.into())
            .unwrap();

        for target in [VAL, KUSD, TBCD] {
            increase_balance(bob(), target.into(), 2 * INITIAL_RESERVES);
            ensure_pool_initialized(XOR.into(), target.into());
            PoolXYK::deposit_liquidity(
                RuntimeOrigin::signed(bob()),
                0,
                XOR.into(),
                target.into(),
                INITIAL_RESERVES,
                INITIAL_RESERVES,
                INITIAL_RESERVES,
                INITIAL_RESERVES,
            )
            .unwrap();
        }

        fill_spot_price();

        assert_eq!(rewards::ValBurnedSinceLastVesting::<Runtime>::get(), 0u128);

        let mut total_xor_to_val = 0;
        for _ in 0..3 {
            let call: &<Runtime as frame_system::Config>::RuntimeCall =
                &RuntimeCall::Assets(assets::Call::transfer {
                    asset_id: VAL.into(),
                    to: bob(),
                    amount: TRANSFER_AMOUNT,
                });

            let len = 10;
            let dispatch_info = info_from_weight(MOCK_WEIGHT);
            let fee = SMALL_FEE + length_fee(len);
            let pre = ChargeTransactionPayment::<Runtime>::new()
                .pre_dispatch(&alice(), call, &dispatch_info, len)
                .unwrap();
            assert!(ChargeTransactionPayment::<Runtime>::post_dispatch(
                Some(pre),
                &dispatch_info,
                &default_post_info(),
                len,
                &Ok(())
            )
            .is_ok());
            let weights_sum = FeeReferrerWeight::get() as u128
                + FeeXorBurnedWeight::get() as u128
                + FeeValBurnedWeight::get() as u128;
            total_xor_to_val += fee * FeeValBurnedWeight::get() as u128 / weights_sum;
        }

        // Bucket values may differ by a few base units due to integer ration rounding.
        let actual_xor_to_val = XorToVal::<Runtime>::get();
        assert_approx_eq_abs!(actual_xor_to_val, total_xor_to_val, 10);
        assert_eq!(XorToBuyBack::<Runtime>::get(), 0);
        assert_eq!(rewards::ValBurnedSinceLastVesting::<Runtime>::get(), 0u128);

        let active_era = pallet_staking::ActiveEra::<Runtime>::get().map(|era| era.index);
        xor_fee::Pallet::<Runtime>::on_initialize(1);

        let xor_to_val_after_xor_burn =
            actual_xor_to_val.saturating_sub(RemintXorBurnPercent::get() * actual_xor_to_val);
        let val_burned = calc_xyk_swap_result(
            INITIAL_RESERVES,
            INITIAL_RESERVES,
            xor_to_val_after_xor_burn,
        );

        assert_eq!(rewards::ValBurnedSinceLastVesting::<Runtime>::get(), 0);
        assert_val_staking_reward_recorded(active_era, val_burned);

        assert_approx_eq_abs!(
            crate::Assets::total_issuance(&KUSD.into()).unwrap(),
            balance!(20000),
            balance!(0.00001)
        );

        assert_approx_eq_abs!(
            crate::Assets::total_issuance(&TBCD.into()).unwrap(),
            balance!(20000),
            balance!(0.00001)
        );
    });
}

fn calc_xyk_swap_result(reserve_a: Balance, reserve_b: Balance, input: Balance) -> Balance {
    let x = FixedWrapper::from(reserve_a);
    let y = FixedWrapper::from(reserve_b);
    let x_in = FixedWrapper::from(input);
    let res = (x_in.clone() * y) / (x + x_in) * fixed_wrapper!(0.994);
    res.try_into_balance().unwrap()
}

#[test]
fn length_fee_charges_encoded_bytes() {
    ext().execute_with(|| {
        assert_eq!(length_fee(0), 0);
        assert_eq!(length_fee(1024), balance!(0.0001024));
    });
}

#[test]
fn paid_fees_increase_with_encoded_len() {
    ext().execute_with(|| {
        set_weight_to_fee_multiplier(1);

        let short_len = 100;
        let long_len = 10_000;
        let dispatch_info = info_from_weight(MOCK_WEIGHT);

        let standard_call = RuntimeCall::OracleProxy(oracle_proxy::Call::enable_oracle {
            oracle: common::Oracle::BandChainFeed,
        });
        let standard_short = XorFee::compute_fee(short_len, &standard_call, &dispatch_info, 0).0;
        let standard_long = XorFee::compute_fee(long_len, &standard_call, &dispatch_info, 0).0;
        assert!(standard_long > standard_short);

        let custom_call = RuntimeCall::Assets(assets::Call::mint {
            asset_id: XOR,
            to: bob(),
            amount: balance!(1),
        });
        let custom_short = XorFee::compute_fee(short_len, &custom_call, &dispatch_info, 0).0;
        let custom_long = XorFee::compute_fee(long_len, &custom_call, &dispatch_info, 0).0;
        assert_eq!(custom_short, SMALL_FEE + length_fee(short_len as usize));
        assert!(custom_long > custom_short);
    });
}

#[test]
fn standard_fee_details_expose_length_fee() {
    ext().execute_with(|| {
        set_weight_to_fee_multiplier(1);

        let len = 4096;
        let dispatch_info = info_from_weight(MOCK_WEIGHT);
        let call = RuntimeCall::OracleProxy(oracle_proxy::Call::enable_oracle {
            oracle: common::Oracle::BandChainFeed,
        });

        let (fee_details, custom_fee_details) =
            XorFee::compute_fee_details(len as u32, &call, &dispatch_info, 0);
        let inclusion_fee = fee_details.inclusion_fee.clone().unwrap();

        assert_eq!(custom_fee_details, None);
        assert_eq!(
            inclusion_fee.base_fee,
            WeightToFee::weight_to_fee(
                &BlockWeights::get().get(dispatch_info.class).base_extrinsic
            )
        );
        assert_eq!(inclusion_fee.len_fee, length_fee(len));
        assert_eq!(
            inclusion_fee.adjusted_weight_fee,
            WeightToFee::weight_to_fee(&MOCK_WEIGHT)
        );
        assert_eq!(
            fee_details.final_fee(),
            inclusion_fee.base_fee + inclusion_fee.len_fee + inclusion_fee.adjusted_weight_fee
        );
    });
}

#[test]
fn custom_fee_details_add_length_without_changing_custom_metadata() {
    ext().execute_with(|| {
        set_weight_to_fee_multiplier(1);

        let len = 2048;
        let dispatch_info = info_from_weight(MOCK_WEIGHT);
        let call = RuntimeCall::Assets(assets::Call::mint {
            asset_id: XOR,
            to: bob(),
            amount: balance!(1),
        });

        let (fee_details, custom_fee_details) =
            XorFee::compute_fee_details(len as u32, &call, &dispatch_info, 0);
        let inclusion_fee = fee_details.inclusion_fee.clone().unwrap();

        assert_eq!(
            custom_fee_details,
            Some(CustomFeeDetails::Regular(SMALL_FEE))
        );
        assert_eq!(inclusion_fee.base_fee, 0);
        assert_eq!(inclusion_fee.len_fee, length_fee(len));
        assert_eq!(inclusion_fee.adjusted_weight_fee, SMALL_FEE);
        assert_eq!(fee_details.final_fee(), SMALL_FEE + length_fee(len));
    });
}

#[test]
fn custom_actual_fee_details_add_length_fee() {
    ext().execute_with(|| {
        set_weight_to_fee_multiplier(1);

        let len = 512;
        let dispatch_info = info_from_weight(MOCK_WEIGHT);
        let fee_details = XorFee::compute_actual_fee_details(
            len as u32,
            &dispatch_info,
            &default_post_info(),
            &Ok(()),
            0,
            Some(CustomFeeDetails::Regular(BIG_FEE)),
        );
        let inclusion_fee = fee_details.inclusion_fee.clone().unwrap();

        assert_eq!(inclusion_fee.base_fee, 0);
        assert_eq!(inclusion_fee.len_fee, length_fee(len));
        assert_eq!(inclusion_fee.adjusted_weight_fee, BIG_FEE);
        assert_eq!(fee_details.final_fee(), BIG_FEE + length_fee(len));
    });
}

#[test]
fn standard_actual_fee_details_preserve_weight_and_length() {
    ext().execute_with(|| {
        set_weight_to_fee_multiplier(1);

        let len = 777;
        let actual_weight = MOCK_WEIGHT / 2;
        let dispatch_info = info_from_weight(MOCK_WEIGHT);
        let fee_details = XorFee::compute_actual_fee_details(
            len as u32,
            &dispatch_info,
            &post_info_from_weight(actual_weight),
            &Ok(()),
            0,
            None,
        );
        let inclusion_fee = fee_details.inclusion_fee.clone().unwrap();

        assert_eq!(
            inclusion_fee.base_fee,
            WeightToFee::weight_to_fee(
                &BlockWeights::get().get(dispatch_info.class).base_extrinsic
            )
        );
        assert_eq!(inclusion_fee.len_fee, length_fee(len));
        assert_eq!(
            inclusion_fee.adjusted_weight_fee,
            WeightToFee::weight_to_fee(&MOCK_WEIGHT)
        );
    });
}

#[test]
fn vested_transfer_actual_fee_variants_keep_length_fee() {
    ext().execute_with(|| {
        set_weight_to_fee_multiplier(1);

        let len = 333;
        let dispatch_info = info_from_weight(MOCK_WEIGHT);
        let fee_details = Some(CustomFeeDetails::VestedTransferClaims((
            3 * SMALL_FEE,
            SMALL_FEE,
        )));

        let ok_fee = XorFee::compute_actual_fee_details(
            len as u32,
            &dispatch_info,
            &post_info_from_weight(MOCK_WEIGHT),
            &Ok(()),
            0,
            fee_details,
        );
        let ok_inclusion_fee = ok_fee.inclusion_fee.clone().unwrap();
        assert_eq!(ok_inclusion_fee.len_fee, length_fee(len));
        assert_eq!(ok_inclusion_fee.adjusted_weight_fee, 3 * SMALL_FEE);
        assert_eq!(ok_fee.final_fee(), 3 * SMALL_FEE + length_fee(len));

        let err_fee = XorFee::compute_actual_fee_details(
            len as u32,
            &dispatch_info,
            &post_info_from_weight(MOCK_WEIGHT),
            &Err(DispatchError::Other("vested transfer failed")),
            0,
            fee_details,
        );
        let err_inclusion_fee = err_fee.inclusion_fee.clone().unwrap();
        assert_eq!(err_inclusion_fee.len_fee, length_fee(len));
        assert_eq!(err_inclusion_fee.adjusted_weight_fee, SMALL_FEE);
        assert_eq!(err_fee.final_fee(), SMALL_FEE + length_fee(len));
    });
}

#[test]
fn zero_actual_custom_fee_stays_free_even_with_length() {
    ext().execute_with(|| {
        set_weight_to_fee_multiplier(1);

        let len = 10_000;
        let fee_details = XorFee::compute_actual_fee_details(
            len,
            &info_from_weight(MOCK_WEIGHT),
            &default_post_info(),
            &Ok(()),
            0,
            Some(CustomFeeDetails::Regular(0)),
        );

        assert!(fee_details.inclusion_fee.is_none());
        assert_eq!(fee_details.final_fee(), 0);
    });
}

#[test]
fn zero_actual_custom_fee_keeps_multiplied_tip_without_length_fee() {
    ext().execute_with(|| {
        let multiplier = 6;
        set_weight_to_fee_multiplier(multiplier);

        let len = 10_000;
        let tip = balance!(0.000002);
        let fee_details = XorFee::compute_actual_fee_details(
            len,
            &info_from_weight(MOCK_WEIGHT),
            &default_post_info(),
            &Ok(()),
            tip,
            Some(CustomFeeDetails::Regular(0)),
        );
        let multiplier = Balance::from(multiplier);

        assert!(fee_details.inclusion_fee.is_none());
        assert_eq!(fee_details.tip, multiplier * tip);
        assert_eq!(fee_details.final_fee(), multiplier * tip);
    });
}

#[test]
fn pays_no_fee_details_ignore_length_fee() {
    ext().execute_with(|| {
        let len = 10_000;
        let dispatch_info = DispatchInfo {
            pays_fee: Pays::No,
            ..info_from_weight(MOCK_WEIGHT)
        };
        let call = RuntimeCall::Assets(assets::Call::mint {
            asset_id: XOR,
            to: bob(),
            amount: balance!(1),
        });

        let (fee_details, custom_fee_details) =
            XorFee::compute_fee_details(len, &call, &dispatch_info, 0);
        assert_eq!(custom_fee_details, None);
        assert!(fee_details.inclusion_fee.is_none());
        assert_eq!(fee_details.final_fee(), 0);

        let actual_fee_details = XorFee::compute_actual_fee_details(
            len,
            &info_from_weight(MOCK_WEIGHT),
            &post_info_pays_no(),
            &Ok(()),
            0,
            Some(CustomFeeDetails::Regular(BIG_FEE)),
        );
        assert!(actual_fee_details.inclusion_fee.is_none());
        assert_eq!(actual_fee_details.final_fee(), 0);
    });
}

#[test]
fn pays_no_fee_details_keep_tip_but_ignore_length_and_custom_fee() {
    ext().execute_with(|| {
        let multiplier = 5;
        set_weight_to_fee_multiplier(multiplier);

        let len = 10_000;
        let tip = balance!(0.000009);
        let call = RuntimeCall::Assets(assets::Call::mint {
            asset_id: XOR,
            to: bob(),
            amount: balance!(1),
        });
        let dispatch_info = DispatchInfo {
            pays_fee: Pays::No,
            ..info_from_weight(MOCK_WEIGHT)
        };

        let (fee_details, custom_fee_details) =
            XorFee::compute_fee_details(len, &call, &dispatch_info, tip);
        assert_eq!(custom_fee_details, None);
        assert!(fee_details.inclusion_fee.is_none());
        assert_eq!(fee_details.tip, tip);
        assert_eq!(fee_details.final_fee(), tip);

        let actual_fee_details = XorFee::compute_actual_fee_details(
            len,
            &info_from_weight(MOCK_WEIGHT),
            &post_info_pays_no(),
            &Ok(()),
            tip,
            Some(CustomFeeDetails::Regular(BIG_FEE)),
        );
        assert!(actual_fee_details.inclusion_fee.is_none());
        assert_eq!(actual_fee_details.tip, tip);
        assert_eq!(actual_fee_details.final_fee(), tip);
    });
}

#[test]
fn fee_multiplier_does_not_apply_to_custom_length_fee() {
    ext().execute_with(|| {
        let multiplier = 3;
        set_weight_to_fee_multiplier(multiplier);

        let len = 100;
        let dispatch_info = info_from_weight(MOCK_WEIGHT);
        let call = RuntimeCall::Assets(assets::Call::mint {
            asset_id: XOR,
            to: bob(),
            amount: balance!(1),
        });

        let (fee_details, custom_fee_details) =
            XorFee::compute_fee_details(len as u32, &call, &dispatch_info, 0);
        let inclusion_fee = fee_details.inclusion_fee.clone().unwrap();
        let multiplier = Balance::from(multiplier);

        assert_eq!(
            custom_fee_details,
            Some(CustomFeeDetails::Regular(SMALL_FEE))
        );
        assert_eq!(inclusion_fee.base_fee, 0);
        assert_eq!(inclusion_fee.len_fee, length_fee(len));
        assert_eq!(inclusion_fee.adjusted_weight_fee, multiplier * SMALL_FEE);
        assert_eq!(
            fee_details.final_fee(),
            multiplier * SMALL_FEE + length_fee(len)
        );
    });
}

#[test]
fn fee_multiplier_applies_to_tip_but_not_custom_length_fee() {
    ext().execute_with(|| {
        let multiplier = 4;
        set_weight_to_fee_multiplier(multiplier);

        let len = 123;
        let tip = balance!(0.000003);
        let dispatch_info = info_from_weight(MOCK_WEIGHT);
        let call = RuntimeCall::Assets(assets::Call::mint {
            asset_id: XOR,
            to: bob(),
            amount: balance!(1),
        });

        let (fee_details, custom_fee_details) =
            XorFee::compute_fee_details(len as u32, &call, &dispatch_info, tip);
        let inclusion_fee = fee_details.inclusion_fee.clone().unwrap();
        let multiplier = Balance::from(multiplier);

        assert_eq!(
            custom_fee_details,
            Some(CustomFeeDetails::Regular(SMALL_FEE))
        );
        assert_eq!(inclusion_fee.base_fee, 0);
        assert_eq!(inclusion_fee.len_fee, length_fee(len));
        assert_eq!(inclusion_fee.adjusted_weight_fee, multiplier * SMALL_FEE);
        assert_eq!(fee_details.tip, multiplier * tip);
        assert_eq!(
            fee_details.final_fee(),
            multiplier * (SMALL_FEE + tip) + length_fee(len)
        );
    });
}

#[test]
fn compute_fee_matches_fee_details_for_standard_and_custom_calls() {
    ext().execute_with(|| {
        set_weight_to_fee_multiplier(1);

        let len = 321;
        let tip = balance!(0.000007);
        let dispatch_info = info_from_weight(MOCK_WEIGHT);
        let standard_call = RuntimeCall::OracleProxy(oracle_proxy::Call::enable_oracle {
            oracle: common::Oracle::BandChainFeed,
        });
        let custom_call = RuntimeCall::Assets(assets::Call::mint {
            asset_id: XOR,
            to: bob(),
            amount: balance!(1),
        });

        let (standard_fee_details, standard_custom_details) =
            XorFee::compute_fee_details(len as u32, &standard_call, &dispatch_info, tip);
        let (standard_fee, standard_compute_details) =
            XorFee::compute_fee(len as u32, &standard_call, &dispatch_info, tip);
        assert_eq!(standard_custom_details, None);
        assert_eq!(standard_compute_details, None);
        assert_eq!(standard_fee, standard_fee_details.final_fee());

        let (custom_fee_details, custom_details) =
            XorFee::compute_fee_details(len as u32, &custom_call, &dispatch_info, tip);
        let (custom_fee, custom_compute_details) =
            XorFee::compute_fee(len as u32, &custom_call, &dispatch_info, tip);
        assert_eq!(custom_details, Some(CustomFeeDetails::Regular(SMALL_FEE)));
        assert_eq!(
            custom_compute_details,
            Some(CustomFeeDetails::Regular(SMALL_FEE))
        );
        assert_eq!(custom_fee, custom_fee_details.final_fee());
        assert_eq!(custom_fee, SMALL_FEE + length_fee(len) + tip);
    });
}

#[test]
fn compute_actual_fee_matches_details_for_standard_and_custom_calls() {
    ext().execute_with(|| {
        set_weight_to_fee_multiplier(1);

        let len = 654;
        let tip = balance!(0.000004);
        let dispatch_info = info_from_weight(MOCK_WEIGHT);
        let post_info = post_info_from_weight(MOCK_WEIGHT / 2);

        let standard_fee_details =
            XorFee::compute_actual_fee_details(len, &dispatch_info, &post_info, &Ok(()), tip, None);
        let standard_fee =
            XorFee::compute_actual_fee(len, &dispatch_info, &post_info, &Ok(()), tip, None);
        assert_eq!(standard_fee, standard_fee_details.final_fee());

        let custom_fee_details = XorFee::compute_actual_fee_details(
            len,
            &dispatch_info,
            &post_info,
            &Ok(()),
            tip,
            Some(CustomFeeDetails::Regular(SMALL_FEE)),
        );
        let custom_fee = XorFee::compute_actual_fee(
            len,
            &dispatch_info,
            &post_info,
            &Ok(()),
            tip,
            Some(CustomFeeDetails::Regular(SMALL_FEE)),
        );
        assert_eq!(custom_fee, custom_fee_details.final_fee());
        assert_eq!(custom_fee, SMALL_FEE + length_fee(len as usize) + tip);
    });
}

#[test]
fn bare_query_info_and_fee_details_stay_free_with_length() {
    ext().execute_with(|| {
        set_weight_to_fee_multiplier(1);

        let len = 4096;
        let call = RuntimeCall::Assets(assets::Call::mint {
            asset_id: XOR,
            to: bob(),
            amount: balance!(1),
        });
        let unchecked_extrinsic = UncheckedExtrinsic::new_bare(call.clone());

        let info = XorFee::query_info(&unchecked_extrinsic, &call, len);
        assert_eq!(info.partial_fee, 0);

        let fee_details = XorFee::query_fee_details(&unchecked_extrinsic, &call, len);
        assert!(fee_details.inclusion_fee.is_none());
        assert_eq!(fee_details.final_fee(), 0);
    });
}

#[test]
fn signed_query_info_and_fee_details_charge_custom_length_fee() {
    ext().execute_with(|| {
        set_weight_to_fee_multiplier(1);

        let len = 2048;
        let call = RuntimeCall::Assets(assets::Call::mint {
            asset_id: XOR,
            to: bob(),
            amount: balance!(1),
        });
        let unchecked_extrinsic = signed_unchecked_extrinsic(call.clone());

        let info = XorFee::query_info(&unchecked_extrinsic, &call, len);
        assert_eq!(info.partial_fee, SMALL_FEE + length_fee(len as usize));

        let fee_details = XorFee::query_fee_details(&unchecked_extrinsic, &call, len);
        let inclusion_fee = fee_details.inclusion_fee.clone().unwrap();
        assert_eq!(inclusion_fee.base_fee, 0);
        assert_eq!(inclusion_fee.len_fee, length_fee(len as usize));
        assert_eq!(inclusion_fee.adjusted_weight_fee, SMALL_FEE);
        assert_eq!(fee_details.final_fee(), info.partial_fee);
    });
}

#[test]
fn applied_signed_extrinsic_decreases_xor_total_supply() {
    ext().execute_with(|| {
        System::set_block_number(1);
        set_weight_to_fee_multiplier(1);

        let pair = sr25519::Pair::from_seed(&[7; 32]);
        let who = AccountId32::from(pair.public());
        increase_balance(who.clone(), XOR.into(), INITIAL_BALANCE);
        increase_balance(who.clone(), VAL.into(), TRANSFER_AMOUNT);

        let xor_issuance_before = Balances::total_issuance();
        assert_eq!(
            Assets::total_issuance(&XOR.into()).unwrap(),
            xor_issuance_before
        );

        let call = RuntimeCall::Assets(assets::Call::transfer {
            asset_id: VAL.into(),
            to: bob(),
            amount: TRANSFER_AMOUNT,
        });
        let extrinsic = signed_unchecked_extrinsic_from_pair(call, &pair);
        let expected_fee = XorFee::query_info(
            &extrinsic,
            &extrinsic.function,
            extrinsic.encoded_size() as u32,
        )
        .partial_fee;

        let dispatch_result = crate::Executive::apply_extrinsic(extrinsic).unwrap();
        assert_ok!(dispatch_result);

        let xor_issuance_after = Balances::total_issuance();
        assert_eq!(xor_issuance_after, xor_issuance_before - expected_fee);
        assert_eq!(
            Assets::total_issuance(&XOR.into()).unwrap(),
            xor_issuance_after
        );
        let rescinded: Balance = System::events()
            .iter()
            .filter_map(|EventRecord { event, .. }| match event {
                RuntimeEvent::Balances(pallet_balances::Event::Rescinded { amount }) => {
                    Some(*amount)
                }
                _ => None,
            })
            .sum();
        assert_eq!(rescinded, expected_fee);
    });
}

#[test]
fn bridge_peer_with_nonzero_length_fee_still_skips_withdrawal() {
    ext().execute_with(|| {
        set_weight_to_fee_multiplier(1);

        let who = crate::EthBridge::bridge_account(0).unwrap();
        let call = RuntimeCall::EthBridge(eth_bridge::Call::finalize_incoming_request {
            hash: Default::default(),
            network_id: 0,
        });
        let len = 100;
        let dispatch_info = info_from_weight(Weight::from_parts(100, 100));
        let quoted_fee = XorFee::compute_fee(len, &call, &dispatch_info, 0).0;

        assert_eq!(quoted_fee, SMALL_FEE + length_fee(len as usize));
        assert_ok!(XorFee::can_withdraw_fee(
            &who,
            &call,
            &dispatch_info,
            quoted_fee,
            0
        ));
        assert_eq!(
            XorFee::withdraw_fee(&who, &call, &dispatch_info, quoted_fee, 0),
            Ok(LiquidityInfo::Paid(who, None, None))
        );
    });
}

#[test]
fn custom_fees_work() {
    ext().execute_with(|| {
        set_weight_to_fee_multiplier(1);
        give_xor_initial_balance(alice());
        give_xor_initial_balance(bob());

        let len: usize = 10;
        let dispatch_info = info_from_weight(MOCK_WEIGHT);
        let base_fee = WeightToFee::weight_to_fee(
            &BlockWeights::get().get(dispatch_info.class).base_extrinsic,
        );
        let len_fee = length_fee(len);
        let weight_fee = WeightToFee::weight_to_fee(&MOCK_WEIGHT);

        // A ten-fold extrinsic; fee is 0.007 XOR plus encoded length
        let calls: Vec<<Runtime as frame_system::Config>::RuntimeCall> = vec![
            RuntimeCall::Assets(assets::Call::register {
                symbol: AssetSymbol(b"ALIC".to_vec()),
                name: AssetName(b"ALICE".to_vec()),
                initial_supply: balance!(0),
                is_mintable: true,
                is_indivisible: false,
                opt_content_src: None,
                opt_desc: None,
            }),
            RuntimeCall::VestedRewards(vested_rewards::Call::claim_rewards {}),
        ];

        let mut balance_after_fee_withdrawal = FixedWrapper::from(INITIAL_BALANCE);
        for call in calls {
            let pre = ChargeTransactionPayment::<Runtime>::new()
                .pre_dispatch(&alice(), &call, &dispatch_info, len)
                .unwrap();
            balance_after_fee_withdrawal = balance_after_fee_withdrawal - (BIG_FEE + len_fee);
            let result = balance_after_fee_withdrawal.clone().into_balance();
            assert_eq!(Balances::free_balance(alice()), result);
            assert!(ChargeTransactionPayment::<Runtime>::post_dispatch(
                Some(pre),
                &dispatch_info,
                &default_post_info(),
                len,
                &Ok(())
            )
            .is_ok());
            assert_eq!(Balances::free_balance(alice()), result);
        }

        // A normal extrinsic; fee is 0.0007 XOR plus encoded length
        let call: &<Runtime as frame_system::Config>::RuntimeCall =
            &RuntimeCall::Assets(assets::Call::mint {
                asset_id: XOR,
                to: bob(),
                amount: balance!(1),
            });

        let pre = ChargeTransactionPayment::<Runtime>::new()
            .pre_dispatch(&alice(), call, &dispatch_info, len)
            .unwrap();
        let balance_after_fee_withdrawal = balance_after_fee_withdrawal - (SMALL_FEE + len_fee);
        let balance_after_fee_withdrawal = balance_after_fee_withdrawal.into_balance();
        assert_eq!(
            Balances::free_balance(alice()),
            balance_after_fee_withdrawal
        );
        assert!(ChargeTransactionPayment::<Runtime>::post_dispatch(
            Some(pre),
            &dispatch_info,
            &default_post_info(),
            len,
            &Ok(())
        )
        .is_ok());
        assert_eq!(
            Balances::free_balance(alice()),
            balance_after_fee_withdrawal
        );

        // An extrinsic without manual fee adjustment
        let call: &<Runtime as frame_system::Config>::RuntimeCall =
            &RuntimeCall::OracleProxy(oracle_proxy::Call::enable_oracle {
                oracle: common::Oracle::BandChainFeed,
            });

        let pre = ChargeTransactionPayment::<Runtime>::new()
            .pre_dispatch(&alice(), call, &dispatch_info, len)
            .unwrap();
        let balance_after_fee_withdrawal =
            FixedWrapper::from(balance_after_fee_withdrawal) - base_fee - len_fee - weight_fee;
        let balance_after_fee_withdrawal = balance_after_fee_withdrawal.into_balance();
        assert_eq!(
            Balances::free_balance(alice()),
            balance_after_fee_withdrawal
        );
        assert!(ChargeTransactionPayment::<Runtime>::post_dispatch(
            Some(pre),
            &dispatch_info,
            &default_post_info(),
            len,
            &Ok(())
        )
        .is_ok());
        assert_eq!(
            Balances::free_balance(alice()),
            balance_after_fee_withdrawal
        );
    });
}

#[test]
fn polkamarkt_growth_calls_charge_small_fee() {
    ext().execute_with(|| {
        set_weight_to_fee_multiplier(1);
        give_xor_initial_balance(alice());

        let len: usize = 10;
        let len_fee = length_fee(len);
        let dispatch_info = info_from_weight(MOCK_WEIGHT);
        let calls: Vec<<Runtime as frame_system::Config>::RuntimeCall> = vec![
            RuntimeCall::Polkamarkt(pallet_polkamarkt::Call::create_condition {
                metadata: pallet_polkamarkt::ConditionInput {
                    question: b"Will SORA win this benchmark market?".to_vec(),
                    oracle: b"Chainlink".to_vec(),
                    resolution_source: b"council-minutes".to_vec(),
                },
            }),
            RuntimeCall::Polkamarkt(pallet_polkamarkt::Call::create_market {
                condition_id: 0,
                close_block: 42,
                seed_liquidity: balance!(100),
            }),
        ];

        let mut balance_after_fee_withdrawal = FixedWrapper::from(INITIAL_BALANCE);
        for call in calls {
            let pre = ChargeTransactionPayment::<Runtime>::new()
                .pre_dispatch(&alice(), &call, &dispatch_info, len)
                .unwrap();
            balance_after_fee_withdrawal = balance_after_fee_withdrawal - (SMALL_FEE + len_fee);
            let result = balance_after_fee_withdrawal.clone().into_balance();
            assert_eq!(Balances::free_balance(alice()), result);
            assert!(ChargeTransactionPayment::<Runtime>::post_dispatch(
                Some(pre),
                &dispatch_info,
                &default_post_info(),
                len,
                &Ok(())
            )
            .is_ok());
            assert_eq!(Balances::free_balance(alice()), result);
        }
    });
}

#[test]
fn polkamarkt_buy_uses_standard_weight_fee() {
    ext().execute_with(|| {
        set_weight_to_fee_multiplier(1);
        give_xor_initial_balance(alice());

        let len: usize = 10;
        let dispatch_info = info_from_weight(MOCK_WEIGHT);
        let base_fee = WeightToFee::weight_to_fee(
            &BlockWeights::get().get(dispatch_info.class).base_extrinsic,
        );
        let len_fee = LengthToFee::weight_to_fee(&Weight::from_parts(len as u64, 0));
        let weight_fee = WeightToFee::weight_to_fee(&MOCK_WEIGHT);
        let call = RuntimeCall::Polkamarkt(pallet_polkamarkt::Call::buy {
            market_id: 0,
            outcome: pallet_polkamarkt::BinaryOutcome::Yes,
            collateral_in: balance!(10),
            min_shares_out: 0,
        });

        assert_eq!(CustomFees::compute_fee(&call), None);
        assert_eq!(
            XorFee::compute_fee(len as u32, &call, &dispatch_info, 0).0,
            base_fee + len_fee + weight_fee
        );

        let pre = ChargeTransactionPayment::<Runtime>::new()
            .pre_dispatch(&alice(), &call, &dispatch_info, len)
            .unwrap();
        let balance_after_fee_withdrawal =
            FixedWrapper::from(INITIAL_BALANCE) - base_fee - len_fee - weight_fee;
        let balance_after_fee_withdrawal = balance_after_fee_withdrawal.into_balance();
        assert_eq!(
            Balances::free_balance(alice()),
            balance_after_fee_withdrawal
        );
        assert!(ChargeTransactionPayment::<Runtime>::post_dispatch(
            Some(pre),
            &dispatch_info,
            &default_post_info(),
            len,
            &Ok(())
        )
        .is_ok());
        assert_eq!(
            Balances::free_balance(alice()),
            balance_after_fee_withdrawal
        );
    });
}

#[test]
fn custom_fees_multiplied() {
    ext().execute_with(|| {
        let multiplier = 3;
        set_weight_to_fee_multiplier(multiplier);
        let multiplier: u128 = multiplier.into();

        give_xor_initial_balance(alice());
        give_xor_initial_balance(bob());

        let len = 10;
        let dispatch_info = info_from_weight(MOCK_WEIGHT);
        let len_fee = length_fee(len);

        // A ten-fold extrinsic; fee is (0.007 plus encoded length) * multiplier XOR
        let calls: Vec<<Runtime as frame_system::Config>::RuntimeCall> = vec![
            RuntimeCall::Assets(assets::Call::register {
                symbol: AssetSymbol(b"ALIC".to_vec()),
                name: AssetName(b"ALICE".to_vec()),
                initial_supply: balance!(0),
                is_mintable: true,
                is_indivisible: false,
                opt_content_src: None,
                opt_desc: None,
            }),
            RuntimeCall::VestedRewards(vested_rewards::Call::claim_rewards {}),
        ];

        let mut balance_after_fee_withdrawal = FixedWrapper::from(INITIAL_BALANCE);
        for call in calls {
            let pre = ChargeTransactionPayment::<Runtime>::new()
                .pre_dispatch(&alice(), &call, &dispatch_info, len)
                .unwrap();
            balance_after_fee_withdrawal =
                balance_after_fee_withdrawal - multiplier * BIG_FEE - len_fee;
            let result = balance_after_fee_withdrawal.clone().into_balance();
            assert_eq!(Balances::free_balance(alice()), result);
            assert!(ChargeTransactionPayment::<Runtime>::post_dispatch(
                Some(pre),
                &dispatch_info,
                &default_post_info(),
                len,
                &Ok(())
            )
            .is_ok());
            assert_eq!(Balances::free_balance(alice()), result);
        }

        // A normal extrinsic; fee is multiplied, encoded length is not.
        let call: &<Runtime as frame_system::Config>::RuntimeCall =
            &RuntimeCall::Assets(assets::Call::mint {
                asset_id: XOR,
                to: bob(),
                amount: balance!(1),
            });

        let pre = ChargeTransactionPayment::<Runtime>::new()
            .pre_dispatch(&alice(), call, &dispatch_info, len)
            .unwrap();
        let balance_after_fee_withdrawal =
            balance_after_fee_withdrawal - multiplier * SMALL_FEE - len_fee;
        let balance_after_fee_withdrawal = balance_after_fee_withdrawal.into_balance();
        assert_eq!(
            Balances::free_balance(alice()),
            balance_after_fee_withdrawal
        );
        assert!(ChargeTransactionPayment::<Runtime>::post_dispatch(
            Some(pre),
            &dispatch_info,
            &default_post_info(),
            len,
            &Ok(())
        )
        .is_ok());
        assert_eq!(
            Balances::free_balance(alice()),
            balance_after_fee_withdrawal
        );
    });
}

#[test]
fn normal_fees_multiplied() {
    ext().execute_with(|| {
        set_weight_to_fee_multiplier(3);
        give_xor_initial_balance(alice());
        give_xor_initial_balance(bob());

        let len: usize = 10;
        let dispatch_info = info_from_weight(MOCK_WEIGHT);
        let base_fee = WeightToFee::weight_to_fee(
            &BlockWeights::get().get(dispatch_info.class).base_extrinsic,
        );
        let len_fee = LengthToFee::weight_to_fee(&Weight::from_parts(len as u64, 0));
        let weight_fee = WeightToFee::weight_to_fee(&MOCK_WEIGHT);
        let final_fee = (base_fee + weight_fee) * 3 + len_fee;

        let balance_after_fee_withdrawal = FixedWrapper::from(INITIAL_BALANCE);
        // An extrinsic without custom fee adjustment
        let call: &<Runtime as frame_system::Config>::RuntimeCall =
            &RuntimeCall::OracleProxy(oracle_proxy::Call::enable_oracle {
                oracle: common::Oracle::BandChainFeed,
            });

        let pre = ChargeTransactionPayment::<Runtime>::new()
            .pre_dispatch(&alice(), call, &dispatch_info, len)
            .unwrap();
        let balance_after_fee_withdrawal =
            FixedWrapper::from(balance_after_fee_withdrawal) - final_fee;
        let balance_after_fee_withdrawal = balance_after_fee_withdrawal.into_balance();
        assert_eq!(
            Balances::free_balance(alice()),
            balance_after_fee_withdrawal
        );
        assert!(ChargeTransactionPayment::<Runtime>::post_dispatch(
            Some(pre),
            &dispatch_info,
            &default_post_info(),
            len,
            &Ok(())
        )
        .is_ok());
        assert_eq!(
            Balances::free_balance(alice()),
            balance_after_fee_withdrawal
        );
    });
}

#[test]
fn refund_if_pays_no_works() {
    ext().execute_with(|| {
        set_weight_to_fee_multiplier(1);
        give_xor_initial_balance(alice());

        let tech_account_id = GetXorFeeAccountId::get();
        assert_eq!(Balances::free_balance(&tech_account_id), 0u128);

        let len = 10;
        let dispatch_info = info_from_weight(MOCK_WEIGHT);
        let len_fee = length_fee(len);

        let call: &<Runtime as frame_system::Config>::RuntimeCall =
            &RuntimeCall::Assets(assets::Call::register {
                symbol: AssetSymbol(b"ALIC".to_vec()),
                name: AssetName(b"ALICE".to_vec()),
                initial_supply: balance!(0),
                is_mintable: true,
                is_indivisible: false,
                opt_content_src: None,
                opt_desc: None,
            });

        let pre = ChargeTransactionPayment::<Runtime>::new()
            .pre_dispatch(&alice(), call, &dispatch_info, len)
            .unwrap();
        let balance_after_fee_withdrawal =
            FixedWrapper::from(INITIAL_BALANCE) - (BIG_FEE + len_fee);
        let balance_after_fee_withdrawal = balance_after_fee_withdrawal.into_balance();
        assert_eq!(
            Balances::free_balance(alice()),
            balance_after_fee_withdrawal
        );
        assert!(ChargeTransactionPayment::<Runtime>::post_dispatch(
            Some(pre),
            &dispatch_info,
            &post_info_pays_no(),
            len,
            &Ok(())
        )
        .is_ok());
        assert_eq!(Balances::free_balance(alice()), INITIAL_BALANCE,);
        assert_eq!(Balances::free_balance(tech_account_id), 0u128);
    });
}

#[test]
fn actual_weight_is_ignored_works() {
    ext().execute_with(|| {
        set_weight_to_fee_multiplier(1);
        give_xor_initial_balance(alice());

        let len = 10;
        let dispatch_info = info_from_weight(MOCK_WEIGHT);
        let len_fee = length_fee(len);

        let call: &<Runtime as frame_system::Config>::RuntimeCall =
            &RuntimeCall::Assets(assets::Call::transfer {
                asset_id: XOR.into(),
                to: bob(),
                amount: TRANSFER_AMOUNT,
            });

        let pre = ChargeTransactionPayment::<Runtime>::new()
            .pre_dispatch(&alice(), call, &dispatch_info, len)
            .unwrap();
        let balance_after_fee_withdrawal =
            FixedWrapper::from(INITIAL_BALANCE) - (SMALL_FEE + len_fee);
        let balance_after_fee_withdrawal = balance_after_fee_withdrawal.into_balance();
        assert_eq!(
            Balances::free_balance(alice()),
            balance_after_fee_withdrawal
        );
        assert!(ChargeTransactionPayment::<Runtime>::post_dispatch(
            Some(pre),
            &dispatch_info,
            &post_info_from_weight(MOCK_WEIGHT / 2),
            len,
            &Ok(())
        )
        .is_ok());
        assert_eq!(
            Balances::free_balance(alice()),
            balance_after_fee_withdrawal,
        );
    });
}

#[ignore]
#[test]
fn reminting_for_sora_parliament_works() {
    ext().execute_with(|| {
        set_weight_to_fee_multiplier(1);
        give_xor_initial_balance(alice());
        assert_eq!(Balances::free_balance(sora_parliament_account()), 0u128);
        let call: &<Runtime as frame_system::Config>::RuntimeCall =
            &RuntimeCall::Assets(assets::Call::register {
                symbol: AssetSymbol(b"ALIC".to_vec()),
                name: AssetName(b"ALICE".to_vec()),
                initial_supply: balance!(0),
                is_mintable: true,
                is_indivisible: false,
                opt_content_src: None,
                opt_desc: None,
            });

        let len = 10;
        let dispatch_info = info_from_weight(MOCK_WEIGHT);
        let pre = ChargeTransactionPayment::<Runtime>::new()
            .pre_dispatch(&alice(), call, &dispatch_info, len)
            .unwrap();
        assert!(ChargeTransactionPayment::<Runtime>::post_dispatch(
            Some(pre),
            &dispatch_info,
            &default_post_info(),
            len,
            &Ok(())
        )
        .is_ok());

        System::set_block_number(1);
        pallet_randomness_collective_flip::Pallet::<Runtime>::on_initialize(1);
        xor_fee::Pallet::<Runtime>::on_initialize(1);

        // Mock uses MockLiquiditySource that doesn't exchange, so no remint should happen.
        let sora_val = Tokens::free_balance(VAL.into(), &sora_parliament_account());
        let sora_xor = Balances::free_balance(sora_parliament_account());
        assert_eq!(sora_val, 0);
        assert_eq!(sora_xor, 0);
    });
}

/// No special fee handling should be performed
#[test]
fn fee_payment_regular_swap() {
    ext().execute_with(|| {
        give_xor_initial_balance(alice());

        let dispatch_info = info_from_weight(Weight::from_parts(100_000_000, 0));

        let call = RuntimeCall::LiquidityProxy(liquidity_proxy::Call::swap {
            dex_id: 0,
            input_asset_id: VAL,
            output_asset_id: XOR,
            swap_amount: SwapAmount::WithDesiredInput {
                desired_amount_in: balance!(100),
                min_amount_out: balance!(100),
            },
            selected_source_types: vec![],
            filter_mode: FilterMode::Disabled,
        });

        let regular_fee =
            xor_fee::Pallet::<Runtime>::withdraw_fee(&alice(), &call, &dispatch_info, 1337, 0);

        assert!(matches!(regular_fee, Ok(LiquidityInfo::Paid(..))));
    });
}

/// Fee should be postponed until after the transaction
#[test]
fn fee_payment_postponed_swap() {
    ext().execute_with(|| {
        set_weight_to_fee_multiplier(1);
        increase_balance(alice(), VAL.into(), balance!(1000));

        increase_balance(bob(), XOR.into(), balance!(1000));
        increase_balance(bob(), VAL.into(), balance!(1000));

        ensure_pool_initialized(XOR.into(), VAL.into());
        PoolXYK::deposit_liquidity(
            RuntimeOrigin::signed(bob()),
            0,
            XOR.into(),
            VAL.into(),
            balance!(500),
            balance!(500),
            balance!(450),
            balance!(450),
        )
        .unwrap();

        fill_spot_price();

        let dispatch_info = info_from_weight(Weight::from_parts(100_000_000, 0));

        let call = RuntimeCall::LiquidityProxy(liquidity_proxy::Call::swap {
            dex_id: 0,
            input_asset_id: VAL,
            output_asset_id: XOR,
            swap_amount: SwapAmount::WithDesiredInput {
                desired_amount_in: balance!(100),
                min_amount_out: balance!(50),
            },
            selected_source_types: vec![],
            filter_mode: FilterMode::Disabled,
        });

        let quoted_fee =
            xor_fee::Pallet::<Runtime>::withdraw_fee(&alice(), &call, &dispatch_info, SMALL_FEE, 0)
                .unwrap();

        assert_eq!(quoted_fee, LiquidityInfo::Postponed(alice()));
    });
}

/// Fee should be postponed until after the transaction
#[test]
fn fee_payment_postponed_swap_transfer() {
    ext().execute_with(|| {
        set_weight_to_fee_multiplier(1);
        increase_balance(alice(), VAL.into(), balance!(1000));

        increase_balance(bob(), XOR.into(), balance!(1000));
        increase_balance(bob(), VAL.into(), balance!(1000));

        ensure_pool_initialized(XOR.into(), VAL.into());
        PoolXYK::deposit_liquidity(
            RuntimeOrigin::signed(bob()),
            0,
            XOR.into(),
            VAL.into(),
            balance!(500),
            balance!(500),
            balance!(450),
            balance!(450),
        )
        .unwrap();

        fill_spot_price();

        let dispatch_info = info_from_weight(Weight::from_parts(100_000_000, 0));

        let call = RuntimeCall::LiquidityProxy(liquidity_proxy::Call::swap_transfer {
            receiver: bob(),
            dex_id: 0,
            input_asset_id: VAL,
            output_asset_id: XOR,
            swap_amount: SwapAmount::WithDesiredInput {
                desired_amount_in: balance!(100),
                min_amount_out: balance!(50),
            },
            selected_source_types: vec![],
            filter_mode: FilterMode::Disabled,
        });

        let quoted_fee =
            xor_fee::Pallet::<Runtime>::withdraw_fee(&alice(), &call, &dispatch_info, SMALL_FEE, 0);

        assert!(matches!(quoted_fee, Err(_)));
    });
}

/// Payment should not be postponed if we are not producing XOR
#[test]
fn fee_payment_should_not_postpone() {
    ext().execute_with(|| {
        let dispatch_info = info_from_weight(Weight::from_parts(100_000_000, 0));

        let call = RuntimeCall::LiquidityProxy(liquidity_proxy::Call::swap {
            dex_id: 0,
            input_asset_id: XOR,
            output_asset_id: VAL,
            swap_amount: SwapAmount::WithDesiredInput {
                desired_amount_in: balance!(100),
                min_amount_out: balance!(100),
            },
            selected_source_types: vec![],
            filter_mode: FilterMode::Disabled,
        });

        let quoted_fee =
            xor_fee::Pallet::<Runtime>::withdraw_fee(&alice(), &call, &dispatch_info, 1337, 0);

        assert!(matches!(quoted_fee, Err(_)));
    });
}

#[test]
fn withdraw_fee_set_referrer() {
    ext().execute_with(|| {
        set_weight_to_fee_multiplier(1);
        increase_balance(bob(), XOR.into(), balance!(1000));

        Referrals::reserve(RuntimeOrigin::signed(bob()), SMALL_FEE).unwrap();

        let dispatch_info = info_from_weight(Weight::from_parts(100_000_000, 0));
        let call = RuntimeCall::Referrals(referrals::Call::set_referrer { referrer: bob() });
        let initial_balance = Assets::free_balance(&XOR.into(), &alice()).unwrap();

        let result = XorFee::withdraw_fee(&alice(), &call, &dispatch_info, SMALL_FEE, 0);
        assert_eq!(
            result,
            Ok(LiquidityInfo::Paid(
                crate::ReferralsReservesAcc::get(),
                Some(NegativeImbalance::new(SMALL_FEE)),
                None
            ))
        );
        assert_eq!(
            Assets::free_balance(&XOR.into(), &alice()),
            Ok(initial_balance)
        );
    });
}

#[test]
fn withdraw_fee_set_referrer_already() {
    ext().execute_with(|| {
        Referrals::set_referrer_to(&alice(), bob()).unwrap();

        increase_balance(bob(), XOR.into(), balance!(1000));

        Referrals::reserve(RuntimeOrigin::signed(bob()), SMALL_FEE).unwrap();

        let dispatch_info = info_from_weight(Weight::from_parts(100_000_000, 0));
        let call = RuntimeCall::Referrals(referrals::Call::set_referrer { referrer: bob() });
        let result = XorFee::withdraw_fee(&alice(), &call, &dispatch_info, 1337, 0);
        assert_eq!(
            result,
            Err(TransactionValidityError::Invalid(
                InvalidTransaction::Payment
            ))
        );
        assert_eq!(ReferrerBalances::<Runtime>::get(&bob()), Some(SMALL_FEE));
    });
}

#[test]
fn withdraw_fee_set_referrer_already2() {
    ext().execute_with(|| {
        set_weight_to_fee_multiplier(1);
        Referrals::set_referrer_to(&alice(), bob()).unwrap();

        increase_balance(alice(), XOR.into(), balance!(1));
        increase_balance(bob(), XOR.into(), balance!(1000));

        Referrals::reserve(RuntimeOrigin::signed(bob()), SMALL_FEE).unwrap();

        let dispatch_info = info_from_weight(Weight::from_parts(100_000_000, 0));
        let call = RuntimeCall::Referrals(referrals::Call::set_referrer { referrer: bob() });
        let result = XorFee::withdraw_fee(&alice(), &call, &dispatch_info, SMALL_FEE, 0);
        assert_eq!(
            result,
            Ok(LiquidityInfo::Paid(
                alice(),
                Some(NegativeImbalance::new(SMALL_FEE)),
                None
            ))
        );
        assert_eq!(
            Assets::free_balance(&XOR.into(), &alice()),
            Ok(balance!(1) - SMALL_FEE)
        );
        assert_eq!(ReferrerBalances::<Runtime>::get(&bob()), Some(SMALL_FEE));
    });
}

#[test]
fn it_works_eth_bridge_pays_no() {
    ext().execute_with(|| {
        set_weight_to_fee_multiplier(1);
        let who = crate::EthBridge::bridge_account(0).unwrap();
        let call = RuntimeCall::EthBridge(eth_bridge::Call::finalize_incoming_request {
            hash: Default::default(),
            network_id: 0,
        });
        let info = info_from_weight(Weight::from_parts(100, 100));
        let len = 100;
        let len_fee = length_fee(len as usize);
        let (fee, custom_fee_details) = XorFee::compute_fee(len, &call, &info, 0);
        assert_eq!(fee, SMALL_FEE + len_fee);
        assert_eq!(
            custom_fee_details,
            Some(CustomFeeDetails::Regular(SMALL_FEE))
        );
        assert_eq!(CustomFees::get_fee_source(&who, &call, fee), who);
        assert!(!CustomFees::should_be_paid(&who, &call));
        let res = xor_fee::extension::ChargeTransactionPayment::<Runtime>::new().pre_dispatch(
            &who,
            &call,
            &info,
            len as usize,
        );
        assert_eq!(
            res,
            Ok((
                0,
                who.clone(),
                LiquidityInfo::Paid(who, None, None),
                Some(CustomFeeDetails::Regular(SMALL_FEE)),
                None,
            ))
        );
    });
}

#[test]
fn fee_not_postponed_place_limit_order() {
    ext().execute_with(|| {
        set_weight_to_fee_multiplier(1);
        give_xor_initial_balance(alice());

        let order_book_id = OrderBookId {
            dex_id: DEXId::Polkaswap.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        let dispatch_info = info_from_weight(Weight::from_parts(100_000_000, 0));

        let call = RuntimeCall::OrderBook(order_book::Call::place_limit_order {
            order_book_id,
            price: balance!(11),
            amount: balance!(100),
            side: PriceVariant::Sell,
            lifespan: None,
        });

        let quoted_fee =
            xor_fee::Pallet::<Runtime>::withdraw_fee(&alice(), &call, &dispatch_info, SMALL_FEE, 0)
                .unwrap();

        assert_eq!(
            quoted_fee,
            LiquidityInfo::Paid(alice(), Some(NegativeImbalance::new(SMALL_FEE)), None)
        );
    });
}

#[test]
fn withdraw_fee_place_limit_order_with_default_lifetime() {
    ext().execute_with(|| {
        set_weight_to_fee_multiplier(1);
        give_xor_initial_balance(alice());

        let order_book_id = OrderBookId {
            dex_id: DEXId::Polkaswap.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        let initial_balance = Assets::free_balance(&XOR.into(), &alice()).unwrap();

        let len: usize = 10;
        let len_fee = length_fee(len);
        let dispatch_info = info_from_weight(Weight::from_parts(100_000_000, 0));
        let call = RuntimeCall::OrderBook(order_book::Call::place_limit_order {
            order_book_id,
            price: balance!(11),
            amount: balance!(100),
            side: PriceVariant::Sell,
            lifespan: None,
        });

        let pre = ChargeTransactionPayment::<Runtime>::new()
            .pre_dispatch(&alice(), &call, &dispatch_info, len)
            .unwrap();
        assert!(ChargeTransactionPayment::<Runtime>::post_dispatch(
            Some(pre),
            &dispatch_info,
            &post_info_from_weight(MOCK_WEIGHT),
            len,
            &Ok(())
        )
        .is_ok());

        let fee = SMALL_FEE / 2 + len_fee;

        assert_eq!(
            Assets::free_balance(&XOR.into(), &alice()).unwrap(),
            initial_balance - fee
        );
    });
}

#[test]
fn withdraw_fee_place_limit_order_with_some_lifetime() {
    ext().execute_with(|| {
        set_weight_to_fee_multiplier(1);
        give_xor_initial_balance(alice());

        let order_book_id = OrderBookId {
            dex_id: DEXId::Polkaswap.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        let initial_balance = Assets::free_balance(&XOR.into(), &alice()).unwrap();

        let len: usize = 10;
        let len_fee = length_fee(len);
        let dispatch_info = info_from_weight(Weight::from_parts(100_000_000, 0));
        let call = RuntimeCall::OrderBook(order_book::Call::place_limit_order {
            order_book_id,
            price: balance!(11),
            amount: balance!(100),
            side: PriceVariant::Sell,
            lifespan: Some(259200000),
        });

        let pre = ChargeTransactionPayment::<Runtime>::new()
            .pre_dispatch(&alice(), &call, &dispatch_info, len)
            .unwrap();
        assert!(ChargeTransactionPayment::<Runtime>::post_dispatch(
            Some(pre),
            &dispatch_info,
            &post_info_from_weight(MOCK_WEIGHT),
            len,
            &Ok(())
        )
        .is_ok());

        let fee = balance!(0.000215) + len_fee;

        assert_eq!(
            Assets::free_balance(&XOR.into(), &alice()).unwrap(),
            initial_balance - fee
        );
    });
}

#[test]
fn withdraw_fee_place_limit_order_with_error() {
    ext().execute_with(|| {
        set_weight_to_fee_multiplier(1);
        give_xor_initial_balance(alice());

        let order_book_id = OrderBookId {
            dex_id: DEXId::Polkaswap.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        let initial_balance = Assets::free_balance(&XOR.into(), &alice()).unwrap();

        let len: usize = 10;
        let len_fee = length_fee(len);
        let dispatch_info = info_from_weight(Weight::from_parts(100_000_000, 0));
        let call = RuntimeCall::OrderBook(order_book::Call::place_limit_order {
            order_book_id,
            price: balance!(11),
            amount: balance!(100),
            side: PriceVariant::Sell,
            lifespan: None,
        });

        let pre = ChargeTransactionPayment::<Runtime>::new()
            .pre_dispatch(&alice(), &call, &dispatch_info, len)
            .unwrap();
        assert!(ChargeTransactionPayment::<Runtime>::post_dispatch(
            Some(pre),
            &dispatch_info,
            &post_info_from_weight(MOCK_WEIGHT),
            len,
            &Err(order_book::Error::<Runtime>::InvalidLimitOrderPrice.into())
        )
        .is_ok());

        let fee = SMALL_FEE + len_fee;

        assert_eq!(
            Assets::free_balance(&XOR.into(), &alice()).unwrap(),
            initial_balance - fee
        );
    });
}

#[test]
fn withdraw_fee_place_limit_order_with_crossing_spread() {
    ext().execute_with(|| {
        set_weight_to_fee_multiplier(1);
        give_xor_initial_balance(alice());

        let order_book_id = OrderBookId {
            dex_id: DEXId::Polkaswap.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        let initial_balance = Assets::free_balance(&XOR.into(), &alice()).unwrap();

        let len: usize = 10;
        let len_fee = length_fee(len);
        let dispatch_info = info_from_weight(Weight::from_parts(100_000_000, 0));
        let call = RuntimeCall::OrderBook(order_book::Call::place_limit_order {
            order_book_id,
            price: balance!(11),
            amount: balance!(100),
            side: PriceVariant::Sell,
            lifespan: None,
        });

        let pre = ChargeTransactionPayment::<Runtime>::new()
            .pre_dispatch(&alice(), &call, &dispatch_info, len)
            .unwrap();
        assert!(ChargeTransactionPayment::<Runtime>::post_dispatch(
            Some(pre),
            &dispatch_info,
            &default_post_info(), // none weight means that the limit was converted into market order
            len,
            &Ok(())
        )
        .is_ok());

        let fee = SMALL_FEE + len_fee;

        assert_eq!(
            Assets::free_balance(&XOR.into(), &alice()).unwrap(),
            initial_balance - fee
        );
    });
}

/// Fee should be postponed until after the transaction
#[test]
fn fee_payment_postponed_xorless_transfer() {
    ext().execute_with(|| {
        set_weight_to_fee_multiplier(1);
        increase_balance(alice(), VAL.into(), balance!(1000));

        increase_balance(bob(), XOR.into(), balance!(1000));
        increase_balance(bob(), VAL.into(), balance!(1000));

        ensure_pool_initialized(XOR.into(), VAL.into());
        PoolXYK::deposit_liquidity(
            RuntimeOrigin::signed(bob()),
            0,
            XOR.into(),
            VAL.into(),
            balance!(500),
            balance!(500),
            balance!(450),
            balance!(450),
        )
        .unwrap();

        fill_spot_price();

        let dispatch_info = info_from_weight(Weight::from_parts(100_000_000, 0));

        let call = RuntimeCall::LiquidityProxy(liquidity_proxy::Call::xorless_transfer {
            dex_id: 0,
            asset_id: VAL,
            desired_xor_amount: 0,
            max_amount_in: 0,
            amount: balance!(500),
            selected_source_types: vec![],
            filter_mode: FilterMode::Disabled,
            receiver: alice(),
            additional_data: Default::default(),
        });

        let quoted_fee =
            xor_fee::Pallet::<Runtime>::withdraw_fee(&bob(), &call, &dispatch_info, SMALL_FEE, 0)
                .unwrap();

        assert_eq!(
            quoted_fee,
            LiquidityInfo::Paid(bob(), Some(NegativeImbalance::new(SMALL_FEE)), None)
        );

        let call = RuntimeCall::LiquidityProxy(liquidity_proxy::Call::xorless_transfer {
            dex_id: 0,
            asset_id: VAL,
            desired_xor_amount: SMALL_FEE,
            max_amount_in: balance!(1),
            amount: balance!(10),
            selected_source_types: vec![],
            filter_mode: FilterMode::Disabled,
            receiver: bob(),
            additional_data: Default::default(),
        });

        let quoted_fee =
            xor_fee::Pallet::<Runtime>::withdraw_fee(&alice(), &call, &dispatch_info, SMALL_FEE, 0)
                .unwrap();

        assert_eq!(quoted_fee, LiquidityInfo::Postponed(alice()));

        assert_eq!(
            Assets::total_balance(&XOR.into(), &alice()).unwrap(),
            balance!(0)
        );
        assert_eq!(
            Assets::total_balance(&VAL.into(), &alice()).unwrap(),
            balance!(1000)
        );

        let post_info = call.dispatch(RuntimeOrigin::signed(alice())).unwrap();

        assert_eq!(
            Assets::total_balance(&XOR.into(), &alice()).unwrap(),
            SMALL_FEE
        );
        assert_eq!(
            Assets::total_balance(&VAL.into(), &alice()).unwrap(),
            balance!(989.999295773656019233)
        );
        assert_eq!(
            Assets::total_balance(&VAL.into(), &bob()).unwrap(),
            balance!(510)
        );

        assert_ok!(xor_fee::Pallet::<Runtime>::correct_and_deposit_fee(
            &alice(),
            &dispatch_info,
            &post_info,
            SMALL_FEE,
            0,
            quoted_fee
        ));

        assert_eq!(Assets::total_balance(&XOR.into(), &alice()).unwrap(), 0);
    });
}

/// Fee should be postponed until after the transaction
#[test]
fn fee_payment_postpone_failed_xorless_transfer() {
    ext().execute_with(|| {
        set_weight_to_fee_multiplier(1);
        increase_balance(alice(), VAL.into(), balance!(1000));

        increase_balance(bob(), XOR.into(), balance!(1000));
        increase_balance(bob(), VAL.into(), balance!(1000));

        ensure_pool_initialized(XOR.into(), VAL.into());
        PoolXYK::deposit_liquidity(
            RuntimeOrigin::signed(bob()),
            0,
            XOR.into(),
            VAL.into(),
            balance!(500),
            balance!(500),
            balance!(450),
            balance!(450),
        )
        .unwrap();

        fill_spot_price();

        let dispatch_info = info_from_weight(Weight::from_parts(100_000_000, 0));

        let call = RuntimeCall::LiquidityProxy(liquidity_proxy::Call::xorless_transfer {
            dex_id: 0,
            asset_id: VAL,
            desired_xor_amount: SMALL_FEE,
            max_amount_in: 1,
            amount: balance!(10),
            selected_source_types: vec![],
            filter_mode: FilterMode::Disabled,
            receiver: bob(),
            additional_data: Default::default(),
        });

        assert_err!(
            xor_fee::Pallet::<Runtime>::withdraw_fee(&alice(), &call, &dispatch_info, SMALL_FEE, 0),
            TransactionValidityError::Invalid(InvalidTransaction::Payment)
        );

        let call = RuntimeCall::LiquidityProxy(liquidity_proxy::Call::xorless_transfer {
            dex_id: 0,
            asset_id: VAL,
            desired_xor_amount: 0,
            max_amount_in: 0,
            amount: balance!(500),
            selected_source_types: vec![],
            filter_mode: FilterMode::Disabled,
            receiver: bob(),
            additional_data: Default::default(),
        });

        assert_err!(
            xor_fee::Pallet::<Runtime>::withdraw_fee(&alice(), &call, &dispatch_info, SMALL_FEE, 0),
            TransactionValidityError::Invalid(InvalidTransaction::Payment)
        );
    });
}

#[test]
fn right_custom_fee_for_vested_transfer_ok() {
    ext().execute_with(|| {
        set_weight_to_fee_multiplier(10);
        give_xor_initial_balance(alice());
        increase_balance(alice(), DOT.into(), balance!(10));

        let initial_balance = Assets::free_balance(&XOR.into(), &alice()).unwrap();

        let schedule = VestingScheduleVariant::LinearVestingSchedule(LinearVestingSchedule {
            asset_id: DOT,
            start: 0u32,
            period: 10u32,
            period_count: 2u32,
            per_period: 10,
            remainder_amount: 0,
        });

        let len: usize = 10;
        let len_fee = length_fee(len);
        let dispatch_info = info_from_weight(Weight::from_parts(100_000_000, 0));
        let call = RuntimeCall::VestedRewards(vested_rewards::Call::vested_transfer {
            dest: alice(),
            schedule: schedule.clone(),
        });

        let pre = ChargeTransactionPayment::<Runtime>::new()
            .pre_dispatch(&alice(), &call, &dispatch_info, len)
            .unwrap();
        assert!(ChargeTransactionPayment::<Runtime>::post_dispatch(
            Some(pre),
            &dispatch_info,
            &post_info_from_weight(MOCK_WEIGHT),
            len,
            &Ok(())
        )
        .is_ok());

        let multiplier = xor_fee::Pallet::<Runtime>::multiplier();
        let transaction_fee = multiplier.saturating_mul_int(3 * SMALL_FEE) + len_fee;

        assert_eq!(
            Assets::free_balance(&XOR.into(), &alice()).unwrap(),
            initial_balance - transaction_fee
        );
    });
}

#[test]
fn right_custom_fee_for_vested_transfer_err() {
    ext().execute_with(|| {
        set_weight_to_fee_multiplier(10);
        give_xor_initial_balance(alice());
        increase_balance(alice(), DOT.into(), balance!(10));

        let initial_balance = Assets::free_balance(&XOR.into(), &alice()).unwrap();

        let schedule = VestingScheduleVariant::LinearVestingSchedule(LinearVestingSchedule {
            asset_id: DOT,
            start: 0u32,
            period: 0u32,
            period_count: 2u32,
            per_period: 10,
            remainder_amount: 0,
        });

        let len: usize = 10;
        let len_fee = length_fee(len);
        let dispatch_info = info_from_weight(Weight::from_parts(100_000_000, 0));
        let call = RuntimeCall::VestedRewards(vested_rewards::Call::vested_transfer {
            dest: alice(),
            schedule: schedule.clone(),
        });

        let pre = ChargeTransactionPayment::<Runtime>::new()
            .pre_dispatch(&alice(), &call, &dispatch_info, len)
            .unwrap();
        assert!(ChargeTransactionPayment::<Runtime>::post_dispatch(
            Some(pre),
            &dispatch_info,
            &post_info_from_weight(MOCK_WEIGHT),
            len,
            &Err(vested_rewards::Error::<Runtime>::ZeroVestingPeriod.into())
        )
        .is_ok());

        let multiplier = xor_fee::Pallet::<Runtime>::multiplier();
        assert_eq!(
            Assets::free_balance(&XOR.into(), &alice()).unwrap(),
            initial_balance - multiplier.saturating_mul_int(SMALL_FEE) - len_fee
        );
    });
}

#[test]
fn random_remint_works() {
    ext().execute_with(|| {
        System::set_block_number(1);
        set_weight_to_fee_multiplier(1);
        increase_balance(alice(), XOR.into(), INITIAL_RESERVES);

        Staking::on_finalize(0);

        increase_balance(bob(), XOR.into(), 3 * INITIAL_RESERVES);

        crate::TradingPair::register_pair(DEXId::Polkaswap.into(), XOR.into(), KUSD.into())
            .unwrap();

        for target in [VAL, KUSD, TBCD] {
            increase_balance(bob(), target.into(), 2 * INITIAL_RESERVES);
            ensure_pool_initialized(XOR.into(), target.into());
            PoolXYK::deposit_liquidity(
                RuntimeOrigin::signed(bob()),
                0,
                XOR.into(),
                target.into(),
                INITIAL_RESERVES,
                INITIAL_RESERVES,
                INITIAL_RESERVES,
                INITIAL_RESERVES,
            )
            .unwrap();
        }

        fill_spot_price();

        assert_eq!(rewards::ValBurnedSinceLastVesting::<Runtime>::get(), 0u128);

        let xor_issuance_before_fees = Balances::total_issuance();
        assert_eq!(
            crate::Assets::total_issuance(&XOR.into()).unwrap(),
            xor_issuance_before_fees
        );
        let mut total_xor_to_val = 0;
        let mut total_fee = 0;
        for _ in 0..3 {
            let call: &<Runtime as frame_system::Config>::RuntimeCall =
                &RuntimeCall::Assets(assets::Call::transfer {
                    asset_id: VAL.into(),
                    to: bob(),
                    amount: TRANSFER_AMOUNT,
                });

            let len = 10;
            let dispatch_info = info_from_weight(MOCK_WEIGHT);
            let fee = SMALL_FEE + length_fee(len);
            let pre = ChargeTransactionPayment::<Runtime>::new()
                .pre_dispatch(&alice(), call, &dispatch_info, len)
                .unwrap();
            assert!(ChargeTransactionPayment::<Runtime>::post_dispatch(
                Some(pre),
                &dispatch_info,
                &default_post_info(),
                len,
                &Ok(())
            )
            .is_ok());
            let weights_sum = FeeReferrerWeight::get() as u128
                + FeeXorBurnedWeight::get() as u128
                + FeeValBurnedWeight::get() as u128;
            total_xor_to_val += fee * FeeValBurnedWeight::get() as u128 / weights_sum;
            total_fee += fee;
        }

        let xor_issuance_after_fees = xor_issuance_before_fees - total_fee;
        assert_eq!(Balances::total_issuance(), xor_issuance_after_fees);
        assert_eq!(
            crate::Assets::total_issuance(&XOR.into()).unwrap(),
            xor_issuance_after_fees
        );

        // Bucket values may differ by a few base units due to integer ration rounding.
        assert_approx_eq_abs!(XorToVal::<Runtime>::get(), total_xor_to_val, 10);
        assert_eq!(XorToBuyBack::<Runtime>::get(), 0);
        assert_eq!(rewards::ValBurnedSinceLastVesting::<Runtime>::get(), 0u128);

        pallet_randomness_collective_flip::Pallet::<Runtime>::on_initialize(1);
        xor_fee::Pallet::<Runtime>::on_initialize(1);

        assert_eq!(rewards::ValBurnedSinceLastVesting::<Runtime>::get(), 0);
        assert_eq!(
            crate::Assets::total_issuance(&KUSD.into()).unwrap(),
            balance!(20000)
        );
        assert_eq!(
            crate::Assets::total_issuance(&TBCD.into()).unwrap(),
            balance!(20000)
        );
        assert_eq!(Balances::total_issuance(), xor_issuance_after_fees);
        assert_eq!(
            crate::Assets::total_issuance(&XOR.into()).unwrap(),
            xor_issuance_after_fees
        );

        frame_system::Pallet::<Runtime>::kill_prefix(
            frame_system::RawOrigin::Root.into(),
            twox_128(b"RandomnessCollectiveFlip").to_vec(),
            100,
        )
        .unwrap();

        let actual_xor_to_val = XorToVal::<Runtime>::get();
        let actual_xor_to_buy_back = XorToBuyBack::<Runtime>::get();
        assert_eq!(actual_xor_to_buy_back, 0);
        let active_era = pallet_staking::ActiveEra::<Runtime>::get().map(|era| era.index);
        xor_fee::Pallet::<Runtime>::on_initialize(1);

        let xor_to_val_after_xor_burn =
            actual_xor_to_val.saturating_sub(RemintXorBurnPercent::get() * actual_xor_to_val);
        let val_burned = calc_xyk_swap_result(
            INITIAL_RESERVES,
            INITIAL_RESERVES,
            xor_to_val_after_xor_burn,
        );

        assert_eq!(rewards::ValBurnedSinceLastVesting::<Runtime>::get(), 0);
        assert_val_staking_reward_recorded(active_era, val_burned);

        assert_approx_eq_abs!(
            crate::Assets::total_issuance(&KUSD.into()).unwrap(),
            balance!(20000),
            balance!(0.00001)
        );

        assert_approx_eq_abs!(
            crate::Assets::total_issuance(&TBCD.into()).unwrap(),
            balance!(20000),
            balance!(0.00001)
        );

        let expected_xor_issuance = xor_issuance_before_fees
            .saturating_sub(total_fee)
            .saturating_add(actual_xor_to_val)
            .saturating_sub(RemintXorBurnPercent::get() * actual_xor_to_val);

        assert_eq!(Balances::total_issuance(), expected_xor_issuance);
        assert_eq!(
            crate::Assets::total_issuance(&XOR.into()).unwrap(),
            expected_xor_issuance
        );
        assert!(Balances::total_issuance() < xor_issuance_before_fees);
    });
}
