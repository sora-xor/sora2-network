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
    AccountId, AssetId, Assets, Balance, Balances, Currencies, FeeKusdBurnedWeight,
    FeeReferrerWeight, FeeValBurnedWeight, FeeXorBurnedWeight, GetXorFeeAccountId, PoolXYK,
    Referrals, RemintKusdBuyBackPercent, RemintTbcdBuyBackPercent, Runtime, RuntimeCall,
    RuntimeOrigin, Staking, System, Tokens, Weight, XorFee,
};
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
use sp_core::twox_128;
use sp_runtime::traits::{Dispatchable, SignedExtension};
use sp_runtime::{AccountId32, FixedPointNumber, FixedU128};
use traits::MultiCurrency;

use vested_rewards::vesting_currencies::{LinearVestingSchedule, VestingScheduleVariant};

use xor_fee::extension::ChargeTransactionPayment;
use xor_fee::{ApplyCustomFees, LiquidityInfo, XorToBuyBack, XorToVal};

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
        weight: w,
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

        let val_price = FixedWrapper::from(9999000);
        let val_fee = SMALL_FEE * val_price;
        let balance_after_reserving_fee = (INITIAL_BALANCE - val_fee.clone()).into_balance();

        let len = 10;
        let dispatch_info = info_from_weight(MOCK_WEIGHT);

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

        assert_eq!(
            pallet_staking::Pallet::<Runtime>::era_val_burned(),
            0_u128.into()
        );

        let call: &<Runtime as frame_system::Config>::RuntimeCall =
            &RuntimeCall::XorFee(xor_fee::Call::xorless_call {
                call: Box::new(RuntimeCall::Assets(assets::Call::transfer {
                    asset_id: VAL.into(),
                    to: bob(),
                    amount: TRANSFER_AMOUNT,
                })),
                asset_id: VAL.into(),
            });

        let val_price = 999900009999000099;
        let val_fee = FixedWrapper::from(SMALL_FEE) * val_price;
        let balance_after_reserving_fee =
            (INITIAL_BALANCE - val_fee.clone() - val_fee.clone()).into_balance();
        let val_fee = val_fee.into_balance();

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

        let total_xor_to_buy_back = XorFee::calculate_portion_fee_from_weight(
            FeeKusdBurnedWeight::get() + FeeReferrerWeight::get(),
            asset_fee_without_ref_in_xor,
        ) + XorFee::calculate_portion_fee_from_weight(
            FeeKusdBurnedWeight::get(),
            asset_fee_in_xor,
        );

        xor_fee::Pallet::<Runtime>::on_initialize(1);
        assert!(xor_fee::BurntForFee::<Runtime>::iter().next().is_none());

        let val_burned = calc_xyk_swap_result(
            INITIAL_RESERVES + val_fee + val_fee,
            INITIAL_RESERVES - total_asset_fee_in_xor,
            total_xor_to_val,
        );
        let remint_buy_back_percent =
            RemintKusdBuyBackPercent::get() + RemintTbcdBuyBackPercent::get();
        let xor_to_remint_buy_back = calc_xyk_swap_result(
            INITIAL_RESERVES - val_burned + val_fee + val_fee,
            INITIAL_RESERVES + total_xor_to_val - total_asset_fee_in_xor,
            remint_buy_back_percent * val_burned,
        );

        assert_approx_eq_abs!(
            pallet_staking::Pallet::<Runtime>::era_val_burned(),
            val_burned - remint_buy_back_percent * val_burned,
            balance!(0.000000001)
        );

        let kusd_burned =
            calc_xyk_swap_result(INITIAL_RESERVES, INITIAL_RESERVES, total_xor_to_buy_back);
        let kusd_burned_remint = calc_xyk_swap_result(
            INITIAL_RESERVES + total_xor_to_buy_back,
            INITIAL_RESERVES - kusd_burned,
            (RemintKusdBuyBackPercent::get() / remint_buy_back_percent) * xor_to_remint_buy_back,
        );
        let tbcd_burned_remint = calc_xyk_swap_result(
            INITIAL_RESERVES,
            INITIAL_RESERVES,
            (RemintTbcdBuyBackPercent::get() / remint_buy_back_percent) * xor_to_remint_buy_back,
        );

        assert_approx_eq_abs!(
            Assets::total_issuance(&KUSD.into()).unwrap(),
            2 * INITIAL_RESERVES - kusd_burned - kusd_burned_remint,
            balance!(0.00001)
        );

        assert_approx_eq_abs!(
            Assets::total_issuance(&TBCD.into()).unwrap(),
            2 * INITIAL_RESERVES - tbcd_burned_remint,
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
        let pre = ChargeTransactionPayment::<Runtime>::new()
            .pre_dispatch(&alice(), call, &dispatch_info, len)
            .unwrap();
        let balance_after_reserving_fee = FixedWrapper::from(INITIAL_BALANCE) - SMALL_FEE;
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
            + FixedWrapper::from(balance!(FeeKusdBurnedWeight::get()))
            + FixedWrapper::from(balance!(FeeValBurnedWeight::get()));
        let referrer_weight = FixedWrapper::from(balance!(FeeReferrerWeight::get()));
        let initial_balance = FixedWrapper::from(INITIAL_BALANCE);
        let referrer_fee = SMALL_FEE * referrer_weight / weights_sum;
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

        assert_eq!(
            pallet_staking::Pallet::<Runtime>::era_val_burned(),
            0_u128.into()
        );

        let mut total_xor_to_val = 0;
        let mut total_xor_to_buy_back = 0;
        for _ in 0..3 {
            let call: &<Runtime as frame_system::Config>::RuntimeCall =
                &RuntimeCall::Assets(assets::Call::transfer {
                    asset_id: VAL.into(),
                    to: bob(),
                    amount: TRANSFER_AMOUNT,
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
            let weights_sum = FeeReferrerWeight::get() as u128
                + FeeXorBurnedWeight::get() as u128
                + FeeValBurnedWeight::get() as u128
                + FeeKusdBurnedWeight::get() as u128;
            total_xor_to_val += SMALL_FEE * FeeValBurnedWeight::get() as u128 / weights_sum;
            total_xor_to_buy_back += SMALL_FEE
                * (FeeKusdBurnedWeight::get() + FeeReferrerWeight::get()) as u128
                / weights_sum;
        }

        // The correct answer is 3E-13 away
        assert_eq!(XorToVal::<Runtime>::get(), total_xor_to_val);
        assert_eq!(XorToBuyBack::<Runtime>::get(), total_xor_to_buy_back);
        assert_eq!(
            pallet_staking::Pallet::<Runtime>::era_val_burned(),
            0_u128.into()
        );

        xor_fee::Pallet::<Runtime>::on_initialize(1);

        let val_burned = calc_xyk_swap_result(INITIAL_RESERVES, INITIAL_RESERVES, total_xor_to_val);
        let remint_buy_back_percent =
            RemintKusdBuyBackPercent::get() + RemintTbcdBuyBackPercent::get();
        let xor_to_remint_buy_back = calc_xyk_swap_result(
            INITIAL_RESERVES - val_burned,
            INITIAL_RESERVES + total_xor_to_val,
            remint_buy_back_percent * val_burned,
        );

        assert_approx_eq_abs!(
            pallet_staking::Pallet::<Runtime>::era_val_burned(),
            val_burned - remint_buy_back_percent * val_burned,
            balance!(0.000000001)
        );

        let kusd_burned =
            calc_xyk_swap_result(INITIAL_RESERVES, INITIAL_RESERVES, total_xor_to_buy_back);
        let kusd_burned_remint = calc_xyk_swap_result(
            INITIAL_RESERVES + total_xor_to_buy_back,
            INITIAL_RESERVES - kusd_burned,
            (RemintKusdBuyBackPercent::get() / remint_buy_back_percent) * xor_to_remint_buy_back,
        );
        let tbcd_burned_remint = calc_xyk_swap_result(
            INITIAL_RESERVES,
            INITIAL_RESERVES,
            (RemintTbcdBuyBackPercent::get() / remint_buy_back_percent) * xor_to_remint_buy_back,
        );

        assert_approx_eq_abs!(
            crate::Assets::total_issuance(&KUSD.into()).unwrap(),
            balance!(20000) - kusd_burned - kusd_burned_remint,
            balance!(0.00001)
        );

        assert_approx_eq_abs!(
            crate::Assets::total_issuance(&TBCD.into()).unwrap(),
            balance!(20000) - tbcd_burned_remint,
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
        let len_fee = LengthToFee::weight_to_fee(&Weight::from_parts(len as u64, 0));
        let weight_fee = WeightToFee::weight_to_fee(&MOCK_WEIGHT);

        // A ten-fold extrinsic; fee is 0.007 XOR
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
            balance_after_fee_withdrawal = balance_after_fee_withdrawal - BIG_FEE;
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

        // A normal extrinsic; fee is 0.0007 XOR
        let call: &<Runtime as frame_system::Config>::RuntimeCall =
            &RuntimeCall::Assets(assets::Call::mint {
                asset_id: XOR,
                to: bob(),
                amount: balance!(1),
            });

        let pre = ChargeTransactionPayment::<Runtime>::new()
            .pre_dispatch(&alice(), call, &dispatch_info, len)
            .unwrap();
        let balance_after_fee_withdrawal = balance_after_fee_withdrawal - SMALL_FEE;
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
fn custom_fees_multiplied() {
    ext().execute_with(|| {
        let multiplier = 3;
        set_weight_to_fee_multiplier(multiplier);
        let multiplier: u128 = multiplier.into();

        give_xor_initial_balance(alice());
        give_xor_initial_balance(bob());

        let len = 10;
        let dispatch_info = info_from_weight(MOCK_WEIGHT);

        // A ten-fold extrinsic; fee is (0.007 * multiplier) XOR
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
            balance_after_fee_withdrawal = balance_after_fee_withdrawal - multiplier * BIG_FEE;
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

        // A normal extrinsic; fee is (0.0007 * multiplier) XOR
        let call: &<Runtime as frame_system::Config>::RuntimeCall =
            &RuntimeCall::Assets(assets::Call::mint {
                asset_id: XOR,
                to: bob(),
                amount: balance!(1),
            });

        let pre = ChargeTransactionPayment::<Runtime>::new()
            .pre_dispatch(&alice(), call, &dispatch_info, len)
            .unwrap();
        let balance_after_fee_withdrawal = balance_after_fee_withdrawal - multiplier * SMALL_FEE;
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
        let final_fee = (base_fee + len_fee + weight_fee) * 3;

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
        assert_eq!(Balances::free_balance(&tech_account_id), 0_u128.into());

        let len = 10;
        let dispatch_info = info_from_weight(MOCK_WEIGHT);

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
            FixedWrapper::from(INITIAL_BALANCE) - fixed_wrapper!(0.007);
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
        assert_eq!(Balances::free_balance(tech_account_id), 0_u128.into());
    });
}

#[test]
fn actual_weight_is_ignored_works() {
    ext().execute_with(|| {
        set_weight_to_fee_multiplier(1);
        give_xor_initial_balance(alice());

        let len = 10;
        let dispatch_info = info_from_weight(MOCK_WEIGHT);

        let call: &<Runtime as frame_system::Config>::RuntimeCall =
            &RuntimeCall::Assets(assets::Call::transfer {
                asset_id: XOR.into(),
                to: bob(),
                amount: TRANSFER_AMOUNT,
            });

        let pre = ChargeTransactionPayment::<Runtime>::new()
            .pre_dispatch(&alice(), call, &dispatch_info, len)
            .unwrap();
        let balance_after_fee_withdrawal = FixedWrapper::from(INITIAL_BALANCE) - SMALL_FEE;
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
        assert_eq!(
            Balances::free_balance(sora_parliament_account()),
            0_u128.into()
        );
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
        let fee = balance!(0.007);
        let xor_into_val_burned_weight = FeeValBurnedWeight::get() as u128;
        let weights_sum = FeeReferrerWeight::get() as u128
            + FeeXorBurnedWeight::get() as u128
            + xor_into_val_burned_weight;
        let x = FixedWrapper::from(fee / (weights_sum / xor_into_val_burned_weight));
        let y = INITIAL_RESERVES;
        let val_burned = (x.clone() * y / (x + y)).into_balance();

        let remint_buy_back_percent =
            RemintKusdBuyBackPercent::get() + RemintTbcdBuyBackPercent::get();
        let expected_balance = FixedWrapper::from(remint_buy_back_percent * val_burned);

        xor_fee::Pallet::<Runtime>::on_initialize(1);

        // Mock uses MockLiquiditySource that doesn't exchange.
        assert!(
            Tokens::free_balance(VAL.into(), &sora_parliament_account())
                >= (expected_balance.clone() - FixedWrapper::from(1i32)).into_balance()
                && Balances::free_balance(sora_parliament_account())
                    <= (expected_balance + FixedWrapper::from(1i32)).into_balance()
        );
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
        let (fee, custom_fee_details) = XorFee::compute_fee(len, &call, &info, 0);
        assert_eq!(fee, SMALL_FEE);
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
                Some(CustomFeeDetails::Regular(SMALL_FEE))
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

        let fee = SMALL_FEE / 2;

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

        let fee = balance!(0.000215);

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

        let fee = SMALL_FEE;

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

        let fee = SMALL_FEE;

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
        let transaction_fee = multiplier.saturating_mul_int(3 * SMALL_FEE);

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
            initial_balance - multiplier.saturating_mul_int(SMALL_FEE)
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

        assert_eq!(
            pallet_staking::Pallet::<Runtime>::era_val_burned(),
            0_u128.into()
        );

        let mut total_xor_to_val = 0;
        let mut total_xor_to_buy_back = 0;
        for _ in 0..3 {
            let call: &<Runtime as frame_system::Config>::RuntimeCall =
                &RuntimeCall::Assets(assets::Call::transfer {
                    asset_id: VAL.into(),
                    to: bob(),
                    amount: TRANSFER_AMOUNT,
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
            let weights_sum = FeeReferrerWeight::get() as u128
                + FeeXorBurnedWeight::get() as u128
                + FeeValBurnedWeight::get() as u128
                + FeeKusdBurnedWeight::get() as u128;
            total_xor_to_val += SMALL_FEE * FeeValBurnedWeight::get() as u128 / weights_sum;
            total_xor_to_buy_back += SMALL_FEE
                * (FeeKusdBurnedWeight::get() + FeeReferrerWeight::get()) as u128
                / weights_sum;
        }

        // The correct answer is 3E-13 away
        assert_eq!(XorToVal::<Runtime>::get(), total_xor_to_val);
        assert_eq!(XorToBuyBack::<Runtime>::get(), total_xor_to_buy_back);
        assert_eq!(
            pallet_staking::Pallet::<Runtime>::era_val_burned(),
            0_u128.into()
        );

        pallet_randomness_collective_flip::Pallet::<Runtime>::on_initialize(1);
        xor_fee::Pallet::<Runtime>::on_initialize(1);

        assert_eq!(pallet_staking::Pallet::<Runtime>::era_val_burned(), 0);
        assert_eq!(
            crate::Assets::total_issuance(&KUSD.into()).unwrap(),
            balance!(20000)
        );
        assert_eq!(
            crate::Assets::total_issuance(&TBCD.into()).unwrap(),
            balance!(20000)
        );

        frame_system::Pallet::<Runtime>::kill_prefix(
            frame_system::RawOrigin::Root.into(),
            twox_128(b"RandomnessCollectiveFlip").to_vec(),
            100,
        )
        .unwrap();

        xor_fee::Pallet::<Runtime>::on_initialize(1);

        let val_burned = calc_xyk_swap_result(INITIAL_RESERVES, INITIAL_RESERVES, total_xor_to_val);
        let remint_buy_back_percent =
            RemintKusdBuyBackPercent::get() + RemintTbcdBuyBackPercent::get();
        let xor_to_remint_buy_back = calc_xyk_swap_result(
            INITIAL_RESERVES - val_burned,
            INITIAL_RESERVES + total_xor_to_val,
            remint_buy_back_percent * val_burned,
        );

        assert_approx_eq_abs!(
            pallet_staking::Pallet::<Runtime>::era_val_burned(),
            val_burned - remint_buy_back_percent * val_burned,
            balance!(0.000000001)
        );

        let kusd_burned =
            calc_xyk_swap_result(INITIAL_RESERVES, INITIAL_RESERVES, total_xor_to_buy_back);
        let kusd_burned_remint = calc_xyk_swap_result(
            INITIAL_RESERVES + total_xor_to_buy_back,
            INITIAL_RESERVES - kusd_burned,
            (RemintKusdBuyBackPercent::get() / remint_buy_back_percent) * xor_to_remint_buy_back,
        );
        let tbcd_burned_remint = calc_xyk_swap_result(
            INITIAL_RESERVES,
            INITIAL_RESERVES,
            (RemintTbcdBuyBackPercent::get() / remint_buy_back_percent) * xor_to_remint_buy_back,
        );

        assert_approx_eq_abs!(
            crate::Assets::total_issuance(&KUSD.into()).unwrap(),
            balance!(20000) - kusd_burned - kusd_burned_remint,
            balance!(0.00001)
        );

        assert_approx_eq_abs!(
            crate::Assets::total_issuance(&TBCD.into()).unwrap(),
            balance!(20000) - tbcd_burned_remint,
            balance!(0.00001)
        );
    });
}
