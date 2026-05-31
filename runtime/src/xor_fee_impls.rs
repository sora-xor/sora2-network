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

use crate::*;
#[cfg(feature = "wip")] // EVM bridge
use bridge_types::{traits::EVMBridgeWithdrawFee, GenericNetworkId};
#[cfg(feature = "wip")] // Dynamic fee
use common::prelude::FixedWrapper;
use common::LiquidityProxyTrait;
#[cfg(feature = "wip")] // EVM bridge
use common::PriceToolsProvider;
#[cfg(feature = "wip")] // Xorless fee
use common::PriceVariant;
use frame_support::dispatch::DispatchResult;
use frame_support::traits::Currency;
use pallet_utility::Call as UtilityCall;
use sp_runtime::traits::Zero;
#[cfg(feature = "wip")] // Dynamic fee
use sp_runtime::FixedU128;
use sp_runtime::Perbill;
use sp_staking::{EraIndex, Page, StakingAccount};
use vested_rewards::vesting_currencies::VestingSchedule;

#[derive(Debug, PartialEq)]
pub struct CallDepth {
    pub swap_count: u32,
    pub depth: u32,
}

impl From<(u32, u32)> for CallDepth {
    fn from((swap_count, depth): (u32, u32)) -> Self {
        CallDepth { swap_count, depth }
    }
}

impl RuntimeCall {
    #[cfg(feature = "wip")] // EVM bridge
    pub fn withdraw_evm_fee(&self, who: &AccountId) -> DispatchResult {
        match self {
            Self::BridgeProxy(bridge_proxy::Call::burn {
                network_id: GenericNetworkId::EVM(chain_id),
                asset_id,
                ..
            }) => BridgeProxy::withdraw_transfer_fee(who, *chain_id, *asset_id)?,
            Self::EVMFungibleApp(evm_fungible_app::Call::burn {
                network_id,
                asset_id,
                ..
            }) => EVMFungibleApp::withdraw_transfer_fee(who, *network_id, *asset_id)?,
            _ => {}
        }
        Ok(())
    }

    #[cfg(not(feature = "wip"))] // EVM bridge
    pub fn additional_evm_fee(&self, _who: &AccountId) -> DispatchResult {
        Ok(())
    }

    #[cfg(feature = "wip")] // EVM bridge
    pub fn withdraw_evm_fee_nested(&self, who: &AccountId) -> DispatchResult {
        match self {
            Self::Multisig(pallet_multisig::Call::as_multi_threshold_1 { call, .. })
            | Self::Multisig(pallet_multisig::Call::as_multi { call, .. })
            | Self::Utility(UtilityCall::as_derivative { call, .. }) => {
                call.withdraw_evm_fee_nested(who)?
            }
            Self::Utility(UtilityCall::batch { calls })
            | Self::Utility(UtilityCall::batch_all { calls })
            | Self::Utility(UtilityCall::force_batch { calls }) => {
                for call in calls {
                    call.withdraw_evm_fee_nested(who)?;
                }
            }
            call => {
                call.withdraw_evm_fee(who)?;
            }
        }
        Ok(())
    }

    /// `vested_transfer` may be called only through `xorless_call` or manually
    /// so for other extrinsics depth is 2 or more
    pub fn swap_count_and_depth(&self, depth: u32) -> CallDepth {
        match self {
            Self::Multisig(pallet_multisig::Call::as_multi_threshold_1 { call, .. })
            | Self::Multisig(pallet_multisig::Call::as_multi { call, .. })
            | Self::Utility(UtilityCall::as_derivative { call, .. }) => {
                call.swap_count_and_depth(depth.saturating_add(2))
            }
            Self::Utility(UtilityCall::batch { calls })
            | Self::Utility(UtilityCall::batch_all { calls })
            | Self::Utility(UtilityCall::force_batch { calls }) => calls
                .iter()
                .map(|call| call.swap_count_and_depth(depth.saturating_add(2)))
                .fold(
                    CallDepth {
                        swap_count: 0,
                        depth: 0,
                    },
                    |acc, call_depth| CallDepth {
                        swap_count: acc.swap_count.saturating_add(call_depth.swap_count),
                        depth: acc.depth.max(call_depth.depth),
                    },
                ),
            Self::LiquidityProxy(liquidity_proxy::Call::swap { .. })
            | Self::LiquidityProxy(liquidity_proxy::Call::swap_transfer { .. })
            | Self::LiquidityProxy(liquidity_proxy::Call::swap_transfer_batch { .. }) => {
                CallDepth {
                    depth: 0,
                    swap_count: 1,
                }
            }
            Self::XorFee(xor_fee::Call::xorless_call { call, .. }) => {
                call.swap_count_and_depth(depth.saturating_add(1))
            }
            Self::VestedRewards(vested_rewards::Call::vested_transfer { .. }) => CallDepth {
                depth,
                swap_count: 0,
            },
            _ => CallDepth {
                depth: 0,
                swap_count: 0,
            },
        }
    }

    pub fn is_called_by_bridge_peer(&self, who: &AccountId) -> bool {
        match self {
            RuntimeCall::BridgeMultisig(call) => match call {
                bridge_multisig::Call::as_multi {
                    id: multisig_id, ..
                }
                | bridge_multisig::Call::as_multi_threshold_1 {
                    id: multisig_id, ..
                } => bridge_multisig::Accounts::<Runtime>::get(multisig_id)
                    .map(|acc| acc.is_signatory(&who)),
                _ => None,
            },
            RuntimeCall::EthBridge(call) => match call {
                eth_bridge::Call::approve_request { network_id, .. } => {
                    Some(eth_bridge::Pallet::<Runtime>::is_peer(who, *network_id))
                }
                eth_bridge::Call::register_incoming_request { incoming_request } => {
                    let net_id = incoming_request.network_id();
                    eth_bridge::BridgeAccount::<Runtime>::get(net_id).map(|acc| acc == *who)
                }
                eth_bridge::Call::import_incoming_request {
                    load_incoming_request,
                    ..
                } => {
                    let net_id = load_incoming_request.network_id();
                    eth_bridge::BridgeAccount::<Runtime>::get(net_id).map(|acc| acc == *who)
                }
                eth_bridge::Call::finalize_incoming_request { network_id, .. }
                | eth_bridge::Call::abort_request { network_id, .. } => {
                    eth_bridge::BridgeAccount::<Runtime>::get(network_id).map(|acc| acc == *who)
                }
                _ => None,
            },
            _ => None,
        }
        .unwrap_or(false)
    }
}

pub struct CustomFees;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StakingValPayoutPre {
    validator_stash: AccountId,
    era: EraIndex,
    page: Page,
}

pub struct StakingValPayout;

impl xor_fee::StakingValPayout<RuntimeCall, AccountId> for StakingValPayout {
    type Pre = StakingValPayoutPre;

    fn pre_dispatch(call: &RuntimeCall) -> Option<Self::Pre> {
        match call {
            RuntimeCall::Staking(pallet_staking::Call::payout_stakers {
                validator_stash,
                era,
            }) => {
                let page = next_claimable_staking_page(*era, validator_stash)?;
                Some(StakingValPayoutPre {
                    validator_stash: validator_stash.clone(),
                    era: *era,
                    page,
                })
            }
            RuntimeCall::Staking(pallet_staking::Call::payout_stakers_by_page {
                validator_stash,
                era,
                page,
            }) => Some(StakingValPayoutPre {
                validator_stash: validator_stash.clone(),
                era: *era,
                page: *page,
            }),
            _ => None,
        }
    }

    fn post_dispatch(pre: Option<Self::Pre>, result: &DispatchResult) {
        if result.is_err() {
            return;
        }

        if let Some(pre) = pre {
            if let Err(e) = pay_val_staking_reward(pre) {
                frame_support::__private::log::error!(
                    "failed to pay VAL staking reward after staking payout: {e:?}"
                );
            }
        }
    }
}

fn next_claimable_staking_page(era: EraIndex, validator: &AccountId) -> Option<Page> {
    let controller = pallet_staking::Pallet::<Runtime>::bonded(validator)?;
    let ledger =
        pallet_staking::Pallet::<Runtime>::ledger(StakingAccount::Controller(controller)).ok()?;

    if pallet_staking::ErasStakersClipped::<Runtime>::contains_key(era, validator) {
        return ledger
            .legacy_claimed_rewards
            .binary_search(&era)
            .is_err()
            .then_some(0);
    }

    let page_count = pallet_staking::ErasStakersOverview::<Runtime>::get(era, validator)
        .map(|overview| {
            if overview.page_count == 0 && !overview.own.is_zero() {
                1
            } else {
                overview.page_count
            }
        })
        .unwrap_or(1);
    let claimed_pages = pallet_staking::ClaimedRewards::<Runtime>::get(era, validator);
    (0..page_count).find(|page| !claimed_pages.contains(page))
}

fn pay_val_staking_reward(pre: StakingValPayoutPre) -> DispatchResult {
    let era_payout = xor_fee::ValStakingEraReward::<Runtime>::get(pre.era);
    if era_payout.is_zero() {
        return Ok(());
    }

    let era_reward_points = pallet_staking::ErasRewardPoints::<Runtime>::get(pre.era);
    if era_reward_points.total.is_zero() {
        return Ok(());
    }

    let validator_reward_points = era_reward_points
        .individual
        .get(&pre.validator_stash)
        .copied()
        .unwrap_or_else(Zero::zero);
    if validator_reward_points.is_zero() {
        return Ok(());
    }

    let exposure = match pallet_staking::EraInfo::<Runtime>::get_paged_exposure(
        pre.era,
        &pre.validator_stash,
        pre.page,
    ) {
        Some(exposure) if !exposure.total().is_zero() => exposure,
        _ => return Ok(()),
    };

    let validator_total_reward_part =
        Perbill::from_rational(validator_reward_points, era_reward_points.total);
    let validator_total_payout = validator_total_reward_part * era_payout;
    let validator_commission =
        pallet_staking::ErasValidatorPrefs::<Runtime>::get(pre.era, &pre.validator_stash)
            .commission;
    let validator_total_commission_payout = validator_commission * validator_total_payout;
    let validator_leftover_payout =
        validator_total_payout.saturating_sub(validator_total_commission_payout);

    let validator_exposure_part = Perbill::from_rational(exposure.own(), exposure.total());
    let validator_staking_payout = validator_exposure_part * validator_leftover_payout;
    let page_stake_part = Perbill::from_rational(exposure.page_total(), exposure.total());
    let validator_commission_payout = page_stake_part * validator_total_commission_payout;
    pay_val_to_reward_destination(
        &pre.validator_stash,
        pre.era,
        pre.page,
        validator_staking_payout + validator_commission_payout,
    )?;

    for nominator in exposure.others().iter() {
        let nominator_exposure_part = Perbill::from_rational(nominator.value, exposure.total());
        let nominator_reward = nominator_exposure_part * validator_leftover_payout;
        pay_val_to_reward_destination(&nominator.who, pre.era, pre.page, nominator_reward)?;
    }

    Ok(())
}

fn pay_val_to_reward_destination(
    stash: &AccountId,
    era: EraIndex,
    page: Page,
    amount: Balance,
) -> DispatchResult {
    if amount.is_zero() {
        return Ok(());
    }

    let Some(payee) = pallet_staking::Payee::<Runtime>::get(stash) else {
        return Ok(());
    };
    let dest = match payee {
        pallet_staking::RewardDestination::Staked | pallet_staking::RewardDestination::Stash => {
            stash.clone()
        }
        pallet_staking::RewardDestination::Account(dest) => dest,
        #[allow(deprecated)]
        pallet_staking::RewardDestination::Controller => {
            match pallet_staking::Pallet::<Runtime>::bonded(stash) {
                Some(controller) => controller,
                None => return Ok(()),
            }
        }
        pallet_staking::RewardDestination::None => return Ok(()),
    };

    let val = GetValAssetId::get();
    Assets::mint_unchecked(&val, &dest, amount)?;
    XorFee::deposit_val_staking_reward_paid(stash.clone(), dest, era, page, amount);
    Ok(())
}

impl CustomFees {
    fn match_call(call: &RuntimeCall) -> Option<Balance> {
        match call {
            RuntimeCall::LiquidityProxy(liquidity_proxy::Call::swap_transfer_batch {
                swap_batches,
                ..
            }) => Some(
                swap_batches
                    .iter()
                    .map(|x| x.receivers.len() as Balance)
                    .fold(Balance::zero(), |acc, x| acc.saturating_add(x))
                    .saturating_mul(SMALL_FEE)
                    .max(SMALL_FEE),
            ),
            RuntimeCall::Assets(assets::Call::register { .. })
            | RuntimeCall::EthBridge(eth_bridge::Call::transfer_to_sidechain { .. })
            | RuntimeCall::BridgeProxy(bridge_proxy::Call::burn { .. })
            | RuntimeCall::PoolXYK(pool_xyk::Call::withdraw_liquidity { .. })
            | RuntimeCall::Rewards(rewards::Call::claim { .. })
            | RuntimeCall::VestedRewards(vested_rewards::Call::claim_crowdloan_rewards {
                ..
            })
            | RuntimeCall::VestedRewards(vested_rewards::Call::claim_rewards { .. })
            | RuntimeCall::OrderBook(order_book::Call::update_orderbook { .. }) => Some(BIG_FEE),
            RuntimeCall::Assets(..)
            | RuntimeCall::EthBridge(..)
            | RuntimeCall::LiquidityProxy(..)
            | RuntimeCall::MulticollateralBondingCurvePool(..)
            | RuntimeCall::Polkamarkt(pallet_polkamarkt::Call::create_condition { .. })
            | RuntimeCall::Polkamarkt(pallet_polkamarkt::Call::create_market { .. })
            | RuntimeCall::Polkamarkt(pallet_polkamarkt::Call::buy { .. })
            | RuntimeCall::Polkamarkt(pallet_polkamarkt::Call::sell { .. })
            | RuntimeCall::Polkamarkt(pallet_polkamarkt::Call::flip_position { .. })
            | RuntimeCall::PoolXYK(..)
            | RuntimeCall::Rewards(..)
            | RuntimeCall::TradingPair(..)
            | RuntimeCall::Referrals(..)
            | RuntimeCall::OrderBook(..)
            | RuntimeCall::TechnicalCommittee(
                pallet_collective::Call::close { .. } | pallet_collective::Call::propose { .. },
            )
            | RuntimeCall::Council(
                pallet_collective::Call::close { .. } | pallet_collective::Call::propose { .. },
            )
            | RuntimeCall::VestedRewards(vested_rewards::Call::vested_transfer { .. })
            | RuntimeCall::VestedRewards(vested_rewards::Call::claim_unlocked { .. }) => {
                Some(SMALL_FEE)
            }
            // NOTE: reducing fees to 1/10 for payout_stakers (from SMALL_FEE)
            // https://github.com/sora-xor/sora2-network/issues/1335#issuecomment-3004262480
            RuntimeCall::Staking(pallet_staking::Call::payout_stakers { .. })
            | RuntimeCall::Staking(pallet_staking::Call::payout_stakers_by_page { .. })
            | RuntimeCall::Band(..)
            | RuntimeCall::Soratopia(soratopia::Call::check_in {}) => Some(MINIMAL_FEE),
            _ => None,
        }
    }
    fn base_fee(call: &RuntimeCall) -> Option<Balance> {
        match call {
            RuntimeCall::XorFee(xor_fee::Call::xorless_call { call, .. }) => Self::match_call(call),
            call => Self::match_call(call),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CustomFeeDetails {
    /// Regular call with custom fee without any additional logic
    Regular(Balance),

    /// OrderBook::place_limit_order custom fee depends on limit order lifetime
    LimitOrderLifetime(Option<Moment>),

    /// VestedReward::vested_transfer custom fee depends on count of auto claims
    VestedTransferClaims((Balance, Balance)),
}

// Flat fees implementation for the selected extrinsics.
// Returns a value if the extrinsic is subject to manual fee
// adjustment and `None` otherwise
impl xor_fee::ApplyCustomFees<RuntimeCall, AccountId> for CustomFees {
    type FeeDetails = CustomFeeDetails;

    fn compute_fee(call: &RuntimeCall) -> Option<(Balance, CustomFeeDetails)> {
        let mut fee = Self::base_fee(call)?;

        let mut compute_details = |call: &RuntimeCall| -> CustomFeeDetails {
            match call {
                RuntimeCall::OrderBook(order_book::Call::place_limit_order {
                    lifespan, ..
                }) => CustomFeeDetails::LimitOrderLifetime(*lifespan),
                RuntimeCall::VestedRewards(vested_rewards::Call::vested_transfer {
                    schedule,
                    ..
                }) => {
                    // claim fee = SMALL_FEE
                    let whole_claims_fee =
                        SMALL_FEE.saturating_mul(schedule.claims_count() as Balance);
                    let fee_without_claims = fee;
                    fee = fee.saturating_add(whole_claims_fee);
                    CustomFeeDetails::VestedTransferClaims((fee, fee_without_claims))
                }
                _ => CustomFeeDetails::Regular(fee),
            }
        };

        let details = match call {
            RuntimeCall::XorFee(xor_fee::Call::xorless_call { call, .. }) => compute_details(call),
            call => compute_details(call),
        };

        Some((fee, details))
    }

    fn should_be_postponed(
        who: &AccountId,
        _fee_source: &AccountId,
        call: &RuntimeCall,
        fee: Balance,
    ) -> bool {
        let balance = Balances::usable_balance_for_fees(who);
        match call {
            // In case we are producing XOR, we perform exchange before fees are withdraw to allow 0-XOR accounts to trade
            RuntimeCall::LiquidityProxy(liquidity_proxy::Call::swap {
                dex_id,
                input_asset_id,
                output_asset_id,
                swap_amount,
                selected_source_types,
                filter_mode,
            }) => {
                if *output_asset_id != XOR {
                    return false;
                }
                // Check how much user has input asset
                let user_input_balance = Currencies::free_balance(*input_asset_id, who);

                // How much does the user want to spend of their input asset
                let swap_input_amount = match swap_amount {
                    SwapAmount::WithDesiredInput {
                        desired_amount_in, ..
                    } => desired_amount_in,
                    SwapAmount::WithDesiredOutput { max_amount_in, .. } => max_amount_in,
                };

                // The amount of input asset needed for this swap is more than the user has, so error
                if *swap_input_amount > user_input_balance {
                    return false;
                }

                let filter = LiquiditySourceFilter::with_mode(
                    *dex_id,
                    filter_mode.clone(),
                    selected_source_types.clone(),
                );
                let Ok(swap_result) = LiquidityProxy::quote(
                    *dex_id,
                    input_asset_id,
                    output_asset_id,
                    (*swap_amount).into(),
                    filter,
                    true,
                ) else {
                    return false;
                };

                let (limits_ok, output_amount) = match swap_amount {
                    SwapAmount::WithDesiredInput { min_amount_out, .. } => {
                        (swap_result.amount >= *min_amount_out, swap_result.amount)
                    }
                    SwapAmount::WithDesiredOutput {
                        desired_amount_out,
                        max_amount_in,
                        ..
                    } => (swap_result.amount <= *max_amount_in, *desired_amount_out),
                };

                if limits_ok
                    && balance
                        .saturating_add(output_amount)
                        .saturating_sub(Balances::minimum_balance())
                        >= fee
                {
                    return true;
                } else {
                    return false;
                }
            }
            RuntimeCall::LiquidityProxy(liquidity_proxy::Call::xorless_transfer {
                dex_id,
                asset_id,
                amount,
                selected_source_types,
                filter_mode,
                desired_xor_amount,
                max_amount_in,
                ..
            }) => {
                // Pay fee as usual
                if balance > fee {
                    return false;
                }

                // Check how much user has input asset
                let user_input_balance = Currencies::free_balance(*asset_id, who);

                // The amount of input asset needed for this swap is more than the user has, so error
                if amount.saturating_add(*max_amount_in) > user_input_balance {
                    return false;
                }

                let filter = LiquiditySourceFilter::with_mode(
                    *dex_id,
                    filter_mode.clone(),
                    selected_source_types.clone(),
                );
                let Ok(swap_result) = LiquidityProxy::quote(
                    *dex_id,
                    asset_id,
                    &XOR,
                    QuoteAmount::with_desired_output(*desired_xor_amount),
                    filter,
                    true,
                ) else {
                    return false;
                };
                if swap_result.amount <= *max_amount_in
                    && balance
                        .saturating_add(*desired_xor_amount)
                        .saturating_sub(Balances::minimum_balance())
                        >= fee
                {
                    return true;
                } else {
                    return false;
                }
            }
            _ => return false,
        }
    }

    fn should_be_paid(who: &AccountId, call: &RuntimeCall) -> bool {
        if call.is_called_by_bridge_peer(who) {
            return false;
        }
        true
    }

    fn compute_actual_fee(
        _post_info: &sp_runtime::traits::PostDispatchInfoOf<RuntimeCall>,
        _info: &sp_runtime::traits::DispatchInfoOf<RuntimeCall>,
        _result: &sp_runtime::DispatchResult,
        fee_details: Option<CustomFeeDetails>,
    ) -> Option<Balance> {
        let fee_details = fee_details?;
        match fee_details {
            CustomFeeDetails::Regular(fee) => Some(fee),
            CustomFeeDetails::LimitOrderLifetime(lifetime) => {
                order_book::fee_calculator::FeeCalculator::<Runtime>::place_limit_order_fee(
                    lifetime,
                    _post_info.actual_weight.is_some(),
                    _result.is_err(),
                )
            }
            CustomFeeDetails::VestedTransferClaims((fee, fee_without_claims)) => {
                if _result.is_err() {
                    Some(fee_without_claims)
                } else {
                    Some(fee)
                }
            }
        }
    }

    fn get_fee_source(who: &AccountId, call: &RuntimeCall, _fee: Balance) -> AccountId {
        let fee_source = |call: &RuntimeCall| -> AccountId {
            match call {
                RuntimeCall::Referrals(referrals::Call::set_referrer { .. })
                    if Referrals::can_set_referrer(who) =>
                {
                    ReferralsReservesAcc::get()
                }
                _ => who.clone(),
            }
        };
        match call {
            RuntimeCall::XorFee(xor_fee::Call::xorless_call { call, .. }) => fee_source(call),
            call => fee_source(call),
        }
    }
}

pub struct WithdrawFee;

impl xor_fee::WithdrawFee<Runtime> for WithdrawFee {
    fn can_withdraw_fee(
        who: &AccountId,
        fee_source: &AccountId,
        call: &RuntimeCall,
        fee: Balance,
    ) -> Result<(), DispatchError> {
        match call {
            RuntimeCall::Referrals(referrals::Call::set_referrer { referrer })
                if Referrals::can_set_referrer(who) =>
            {
                let referrer_balance = Referrals::referrer_balance(referrer).unwrap_or_default();
                if referrer_balance < fee {
                    return Err(referrals::Error::<Runtime>::ReferrerInsufficientBalance.into());
                }
            }
            #[allow(unused_variables)] // Xorless fee
            RuntimeCall::XorFee(xor_fee::Call::xorless_call { call, asset_id }) => {
                #[cfg(feature = "wip")] // Xorless fee
                match call.as_ref() {
                    RuntimeCall::Referrals(referrals::Call::set_referrer { referrer })
                        if Referrals::can_set_referrer(who) =>
                    {
                        let referrer_balance =
                            Referrals::referrer_balance(referrer).unwrap_or_default();
                        if referrer_balance < fee {
                            return Err(
                                referrals::Error::<Runtime>::ReferrerInsufficientBalance.into()
                            );
                        }
                    }
                    _ => match *asset_id {
                        None => {}
                        Some(asset_id) if XorFee::whitelist_tokens().contains(&asset_id) => {
                            let asset_fee = FixedWrapper::from(PriceTools::get_average_price(
                                &GetXorAssetId::get(),
                                &asset_id,
                                PriceVariant::Buy,
                            )?) * fee;
                            let asset_fee = asset_fee.into_balance();
                            if asset_fee.lt(&MinimalFeeInAsset::get()) {
                                return Err(xor_fee::Error::<Runtime>::FeeCalculationFailed.into());
                            };
                            return Tokens::ensure_can_withdraw(asset_id, fee_source, asset_fee);
                        }
                        _ => return Err(xor_fee::Error::<Runtime>::AssetNotFound.into()),
                    },
                }
            }
            _ => {}
        }

        let current_balance = Balances::free_balance(fee_source);
        let resulting_balance = current_balance
            .checked_sub(fee)
            .ok_or(xor_fee::Error::<Runtime>::FeeCalculationFailed)?;

        Balances::ensure_can_withdraw(
            fee_source,
            fee,
            WithdrawReasons::TRANSACTION_PAYMENT,
            resulting_balance,
        )?;
        Ok(())
    }

    fn withdraw_fee(
        who: &AccountId,
        fee_source: &AccountId,
        call: &RuntimeCall,
        fee: Balance,
    ) -> Result<
        (
            AccountId,
            Option<NegativeImbalanceOf<Runtime>>,
            Option<AssetId>,
        ),
        DispatchError,
    > {
        match call {
            RuntimeCall::Referrals(referrals::Call::set_referrer { referrer })
                // Fee source should be set to referrer by `get_fee_source` method, if not 
                // it means that user can't set referrer
                if Referrals::can_set_referrer(who) =>
            {
                Referrals::withdraw_fee(referrer, fee)?;
            }
            #[allow(unused_variables)] // Xorless fee
            RuntimeCall::XorFee(xor_fee::Call::xorless_call {call, asset_id}) => {
                #[cfg(feature = "wip")] // Xorless fee
                match call.as_ref() {
                    RuntimeCall::Referrals(referrals::Call::set_referrer { referrer })
                    // Fee source should be set to referrer by `get_fee_source` method, if not
                    // it means that user can't set referrer
                    if Referrals::can_set_referrer(who) =>
                        {
                            Referrals::withdraw_fee(referrer, fee)?;
                        }
                    _ => {
                        match *asset_id {
                            None => {},
                            Some(asset_id) if XorFee::whitelist_tokens().contains(&asset_id) => {
                                let asset_fee = FixedWrapper::from(
                                    PriceTools::get_average_price(
                                        &GetXorAssetId::get(),
                                        &asset_id,
                                        PriceVariant::Buy)?
                                ) * fee;
                                let asset_fee = asset_fee.into_balance();
                                if asset_fee.lt(&MinimalFeeInAsset::get()) {
                                    return Err(xor_fee::Error::<Runtime>::FeeCalculationFailed.into())
                                };
                                return Ok((
                                    fee_source.clone(),
                                    Some(Tokens::withdraw(
                                        asset_id,
                                        fee_source,
                                        asset_fee,
                                        ExistenceRequirement::KeepAlive,
                                    ).map(|_| {
                                        NegativeImbalanceOf::<Runtime>::new(asset_fee)
                                    })?),
                                    Some(asset_id),
                                ))
                            }
                            _ => { return Err(xor_fee::Error::<Runtime>::AssetNotFound.into()) }
                        }
                    }
                }
            }
            _ => {}
        }
        #[cfg(feature = "wip")] // EVM bridge
        call.withdraw_evm_fee_nested(who)?;
        Ok((
            fee_source.clone(),
            Some(Balances::withdraw(
                fee_source,
                fee,
                WithdrawReasons::TRANSACTION_PAYMENT,
                ExistenceRequirement::KeepAlive,
            )?),
            None,
        ))
    }
}

#[cfg(feature = "wip")] // Dynamic fee
pub struct DynamicMultiplier;

#[cfg(feature = "wip")] // Dynamic fee
impl xor_fee::CalculateMultiplier<common::AssetIdOf<Runtime>, DispatchError> for DynamicMultiplier {
    fn calculate_multiplier(
        input_asset: &AssetId,
        ref_asset: &AssetId,
    ) -> Result<FixedU128, DispatchError> {
        let price: FixedWrapper = FixedWrapper::from(PriceTools::get_average_price(
            input_asset,
            ref_asset,
            common::PriceVariant::Sell,
        )?);
        let new_multiplier: Balance = (XorFee::small_reference_amount() / (SMALL_FEE * price))
            .try_into_balance()
            .map_err(|_| xor_fee::pallet::Error::<Runtime>::MultiplierCalculationFailed)?;
        Ok(FixedU128::from_inner(new_multiplier))
    }
}

#[cfg(test)]
mod tests {
    use frame_support::dispatch::{DispatchInfo, DispatchResult, PostDispatchInfo};
    use frame_support::traits::{Currency, OnRuntimeUpgrade};
    use frame_support::weights::Weight;
    use pallet_authorship::EventHandler;
    use pallet_utility::Call as UtilityCall;
    use sp_core::H256;
    #[allow(deprecated)]
    use sp_runtime::traits::SignedExtension;
    use vested_rewards::vesting_currencies::{LinearVestingSchedule, VestingScheduleVariant};

    use crate::{
        xor_fee_impls::{CallDepth, CustomFeeDetails, CustomFees},
        *,
    };
    use common::OrderBookId;
    use common::{balance, PriceVariant, VAL, XOR};
    use pallet_staking::{
        ActiveEraInfo, EraRewardPoints, Exposure, IndividualExposure, RewardDestination,
        StakingLedger, ValidatorPrefs,
    };
    use sp_runtime::{AccountId32, DispatchError, Perbill};
    use sp_staking::{ExposurePage, PagedExposureMetadata, StakingAccount};
    use xor_fee::{extension::ChargeTransactionPayment, ApplyCustomFees};

    #[test]
    fn check_calls_from_bridge_peers_pays_yes() {
        let call: &<Runtime as frame_system::Config>::RuntimeCall =
            &RuntimeCall::EthBridge(eth_bridge::Call::transfer_to_sidechain {
                asset_id: XOR.into(),
                to: Default::default(),
                amount: Default::default(),
                network_id: 0,
            });

        let who = AccountId32::from([0; 32]);

        assert!(CustomFees::should_be_paid(&who, call));
    }

    #[test]
    #[ignore] // TODO: fix check_calls_from_bridge_peers_pays_no test
    fn check_calls_from_bridge_peers_pays_no() {
        framenode_chain_spec::ext().execute_with(|| {
            let call: &<Runtime as frame_system::Config>::RuntimeCall =
                &RuntimeCall::EthBridge(eth_bridge::Call::finalize_incoming_request {
                    hash: H256::zero(),
                    network_id: 0,
                });

            let who = eth_bridge::BridgeAccount::<Runtime>::get(0).unwrap();

            assert!(!CustomFees::should_be_paid(&who, call));
        });
    }

    struct StakingPayoutFixture {
        validator: AccountId,
        controller: AccountId,
        nominator: AccountId,
        era: sp_staking::EraIndex,
    }

    fn setup_staking_payout_fixture(total_reward: Option<Balance>) -> StakingPayoutFixture {
        frame_system::Pallet::<Runtime>::set_block_number(1);

        let validator = AccountId32::from([41; 32]);
        let controller = AccountId32::from([42; 32]);
        let nominator = AccountId32::from([43; 32]);
        let era = 7;

        pallet_staking::Bonded::<Runtime>::insert(&validator, &controller);
        pallet_staking::Ledger::<Runtime>::insert(
            &controller,
            StakingLedger::<Runtime> {
                stash: validator.clone(),
                total: 100,
                active: 100,
                unlocking: Default::default(),
                legacy_claimed_rewards: Default::default(),
                controller: Some(controller.clone()),
            },
        );
        pallet_staking::Payee::<Runtime>::insert(&validator, RewardDestination::Stash);
        pallet_staking::Payee::<Runtime>::insert(&nominator, RewardDestination::Stash);
        pallet_staking::ErasRewardPoints::<Runtime>::insert(
            era,
            EraRewardPoints {
                total: 10,
                individual: vec![(validator.clone(), 10)].into_iter().collect(),
            },
        );
        pallet_staking::ErasStakersClipped::<Runtime>::insert(
            era,
            &validator,
            Exposure {
                total: 100,
                own: 40,
                others: vec![IndividualExposure {
                    who: nominator.clone(),
                    value: 60,
                }],
            },
        );
        pallet_staking::ErasValidatorPrefs::<Runtime>::insert(
            era,
            &validator,
            ValidatorPrefs::default(),
        );
        if let Some(total_reward) = total_reward {
            xor_fee::ValStakingEraReward::<Runtime>::insert(era, total_reward);
        }

        StakingPayoutFixture {
            validator,
            controller,
            nominator,
            era,
        }
    }

    fn payout_stakers_call(fixture: &StakingPayoutFixture) -> RuntimeCall {
        RuntimeCall::Staking(pallet_staking::Call::payout_stakers {
            validator_stash: fixture.validator.clone(),
            era: fixture.era,
        })
    }

    fn staking_payout_pre(call: &RuntimeCall) -> Option<super::StakingValPayoutPre> {
        <super::StakingValPayout as xor_fee::StakingValPayout<RuntimeCall, AccountId>>::pre_dispatch(
            call,
        )
    }

    fn staking_payout_post(pre: Option<super::StakingValPayoutPre>, result: &DispatchResult) {
        <super::StakingValPayout as xor_fee::StakingValPayout<RuntimeCall, AccountId>>::post_dispatch(
            pre, result,
        );
    }

    fn assert_no_val_rewards(fixture: &StakingPayoutFixture) {
        assert_eq!(Currencies::free_balance(VAL.into(), &fixture.validator), 0);
        assert_eq!(Currencies::free_balance(VAL.into(), &fixture.nominator), 0);
    }

    fn fund_fee_payer(who: &AccountId) {
        let _ = Balances::deposit_creating(who, balance!(1000) + Balances::minimum_balance());
    }

    fn info_from_weight(weight: Weight) -> DispatchInfo {
        DispatchInfo {
            call_weight: weight,
            extension_weight: Weight::from_parts(0, 0),
            ..Default::default()
        }
    }

    fn default_post_info() -> PostDispatchInfo {
        PostDispatchInfo {
            actual_weight: None,
            pays_fee: Default::default(),
        }
    }

    fn val_staking_reward_paid_events() -> Vec<(
        AccountId,
        AccountId,
        sp_staking::EraIndex,
        sp_staking::Page,
        Balance,
    )> {
        frame_system::Pallet::<Runtime>::events()
            .into_iter()
            .filter_map(|record| match record.event {
                RuntimeEvent::XorFee(xor_fee::Event::ValStakingRewardPaid(
                    stash,
                    dest,
                    era,
                    page,
                    amount,
                )) => Some((stash, dest, era, page, amount)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn staking_payout_hook_mints_val_rewards() {
        framenode_chain_spec::ext().execute_with(|| {
            let fixture = setup_staking_payout_fixture(Some(balance!(1000)));
            let call = payout_stakers_call(&fixture);
            let pre = staking_payout_pre(&call).expect("staking payout call should be recognized");

            staking_payout_post(Some(pre), &Ok(()));

            assert_eq!(
                Currencies::free_balance(VAL.into(), &fixture.validator),
                balance!(400)
            );
            assert_eq!(
                Currencies::free_balance(VAL.into(), &fixture.nominator),
                balance!(600)
            );
            assert_eq!(
                val_staking_reward_paid_events(),
                vec![
                    (
                        fixture.validator.clone(),
                        fixture.validator.clone(),
                        fixture.era,
                        0,
                        balance!(400)
                    ),
                    (
                        fixture.nominator.clone(),
                        fixture.nominator.clone(),
                        fixture.era,
                        0,
                        balance!(600)
                    ),
                ]
            );
        });
    }

    #[test]
    fn staking_payout_hook_uses_stash_reward_points_for_session_author() {
        framenode_chain_spec::ext().execute_with(|| {
            let fixture = setup_staking_payout_fixture(Some(balance!(1000)));
            pallet_staking::ActiveEra::<Runtime>::put(ActiveEraInfo {
                index: fixture.era,
                start: None,
            });
            pallet_staking::ErasRewardPoints::<Runtime>::remove(fixture.era);

            <Runtime as pallet_authorship::Config>::EventHandler::note_author(
                fixture.controller.clone(),
            );

            let reward_points = pallet_staking::ErasRewardPoints::<Runtime>::get(fixture.era);
            assert_eq!(reward_points.total, 20);
            assert_eq!(
                reward_points.individual.get(&fixture.validator).copied(),
                Some(20)
            );
            assert!(!reward_points.individual.contains_key(&fixture.controller));

            let call = payout_stakers_call(&fixture);
            let pre = staking_payout_pre(&call).expect("staking payout call should be recognized");
            staking_payout_post(Some(pre), &Ok(()));

            assert_eq!(
                Currencies::free_balance(VAL.into(), &fixture.validator),
                balance!(400)
            );
            assert_eq!(
                Currencies::free_balance(VAL.into(), &fixture.nominator),
                balance!(600)
            );
        });
    }

    #[test]
    fn staking_reward_points_use_stash_when_author_is_already_stash() {
        framenode_chain_spec::ext().execute_with(|| {
            let fixture = setup_staking_payout_fixture(None);
            pallet_staking::ActiveEra::<Runtime>::put(ActiveEraInfo {
                index: fixture.era,
                start: None,
            });
            pallet_staking::ErasRewardPoints::<Runtime>::remove(fixture.era);

            <Runtime as pallet_authorship::Config>::EventHandler::note_author(
                fixture.validator.clone(),
            );

            let reward_points = pallet_staking::ErasRewardPoints::<Runtime>::get(fixture.era);
            assert_eq!(reward_points.total, 20);
            assert_eq!(
                reward_points.individual.get(&fixture.validator).copied(),
                Some(20)
            );
            assert!(!reward_points.individual.contains_key(&fixture.controller));
        });
    }

    #[test]
    fn staking_reward_points_preserve_unbonded_author_without_panic() {
        framenode_chain_spec::ext().execute_with(|| {
            let fixture = setup_staking_payout_fixture(None);
            let unknown_author = AccountId32::from([99; 32]);
            pallet_staking::ActiveEra::<Runtime>::put(ActiveEraInfo {
                index: fixture.era,
                start: None,
            });
            pallet_staking::ErasRewardPoints::<Runtime>::remove(fixture.era);

            <Runtime as pallet_authorship::Config>::EventHandler::note_author(
                unknown_author.clone(),
            );

            let reward_points = pallet_staking::ErasRewardPoints::<Runtime>::get(fixture.era);
            assert_eq!(reward_points.total, 20);
            assert_eq!(
                reward_points.individual.get(&unknown_author).copied(),
                Some(20)
            );
            assert!(!reward_points.individual.contains_key(&fixture.validator));
        });
    }

    #[test]
    fn staking_reward_points_follow_current_session_validator_set() {
        framenode_chain_spec::ext().execute_with(|| {
            let active_era = pallet_staking::ActiveEra::<Runtime>::get()
                .expect("staking genesis should have an active era")
                .index;
            let author = pallet_session::Validators::<Runtime>::get()
                .into_iter()
                .next()
                .expect("staking genesis should have session validators");
            let reward_stash = pallet_staking::Pallet::<Runtime>::ledger(
                StakingAccount::Controller(author.clone()),
            )
            .or_else(|_| {
                pallet_staking::Pallet::<Runtime>::ledger(StakingAccount::Stash(author.clone()))
            })
            .map(|ledger| ledger.stash)
            .expect("session validator should resolve to a staking ledger");

            assert!(pallet_staking::Validators::<Runtime>::contains_key(
                &reward_stash
            ));

            pallet_staking::ErasRewardPoints::<Runtime>::remove(active_era);
            <Runtime as pallet_authorship::Config>::EventHandler::note_author(author.clone());

            let reward_points = pallet_staking::ErasRewardPoints::<Runtime>::get(active_era);
            assert_eq!(reward_points.total, 20);
            assert_eq!(
                reward_points.individual.get(&reward_stash).copied(),
                Some(20)
            );
            if author != reward_stash {
                assert!(!reward_points.individual.contains_key(&author));
            }
        });
    }

    #[test]
    fn staking_payout_hook_ignores_controller_keyed_reward_points() {
        framenode_chain_spec::ext().execute_with(|| {
            let fixture = setup_staking_payout_fixture(Some(balance!(1000)));
            pallet_staking::ErasRewardPoints::<Runtime>::insert(
                fixture.era,
                EraRewardPoints {
                    total: 20,
                    individual: vec![(fixture.controller.clone(), 20)].into_iter().collect(),
                },
            );

            let call = payout_stakers_call(&fixture);
            let pre = staking_payout_pre(&call).expect("staking payout call should be recognized");
            staking_payout_post(Some(pre), &Ok(()));

            assert_no_val_rewards(&fixture);
            assert!(val_staking_reward_paid_events().is_empty());
        });
    }

    #[test]
    fn staking_payout_reward_point_migration_remaps_session_author_points_to_stash() {
        framenode_chain_spec::ext().execute_with(|| {
            let fixture = setup_staking_payout_fixture(Some(balance!(1000)));
            pallet_staking::ErasRewardPoints::<Runtime>::insert(
                fixture.era,
                EraRewardPoints {
                    total: 20,
                    individual: vec![(fixture.controller.clone(), 20)].into_iter().collect(),
                },
            );

            crate::migrations::RemapStakingRewardPointsToStash::on_runtime_upgrade();

            let reward_points = pallet_staking::ErasRewardPoints::<Runtime>::get(fixture.era);
            assert_eq!(reward_points.total, 20);
            assert_eq!(
                reward_points.individual.get(&fixture.validator).copied(),
                Some(20)
            );
            assert!(!reward_points.individual.contains_key(&fixture.controller));
            assert!(crate::migrations::staking_reward_points_stash_remapped());
        });
    }

    #[test]
    fn staking_payout_reward_point_migration_merges_controller_and_stash_points() {
        framenode_chain_spec::ext().execute_with(|| {
            let fixture = setup_staking_payout_fixture(Some(balance!(1000)));
            pallet_staking::ErasRewardPoints::<Runtime>::insert(
                fixture.era,
                EraRewardPoints {
                    total: 30,
                    individual: vec![
                        (fixture.validator.clone(), 10),
                        (fixture.controller.clone(), 20),
                    ]
                    .into_iter()
                    .collect(),
                },
            );

            crate::migrations::RemapStakingRewardPointsToStash::on_runtime_upgrade();

            let reward_points = pallet_staking::ErasRewardPoints::<Runtime>::get(fixture.era);
            assert_eq!(reward_points.total, 30);
            assert_eq!(
                reward_points.individual.get(&fixture.validator).copied(),
                Some(30)
            );
            assert!(!reward_points.individual.contains_key(&fixture.controller));
        });
    }

    #[test]
    fn staking_payout_reward_point_migration_saturates_merged_points() {
        framenode_chain_spec::ext().execute_with(|| {
            let fixture = setup_staking_payout_fixture(Some(balance!(1000)));
            pallet_staking::ErasRewardPoints::<Runtime>::insert(
                fixture.era,
                EraRewardPoints {
                    total: u32::MAX,
                    individual: vec![
                        (fixture.validator.clone(), u32::MAX),
                        (fixture.controller.clone(), 1),
                    ]
                    .into_iter()
                    .collect(),
                },
            );

            crate::migrations::RemapStakingRewardPointsToStash::on_runtime_upgrade();

            let reward_points = pallet_staking::ErasRewardPoints::<Runtime>::get(fixture.era);
            assert_eq!(reward_points.total, u32::MAX);
            assert_eq!(
                reward_points.individual.get(&fixture.validator).copied(),
                Some(u32::MAX)
            );
            assert!(!reward_points.individual.contains_key(&fixture.controller));
        });
    }

    #[test]
    fn staking_payout_reward_point_migration_preserves_unbonded_accounts() {
        framenode_chain_spec::ext().execute_with(|| {
            let fixture = setup_staking_payout_fixture(Some(balance!(1000)));
            let unknown_author = AccountId32::from([99; 32]);
            pallet_staking::ErasRewardPoints::<Runtime>::insert(
                fixture.era,
                EraRewardPoints {
                    total: 20,
                    individual: vec![(unknown_author.clone(), 20)].into_iter().collect(),
                },
            );

            crate::migrations::RemapStakingRewardPointsToStash::on_runtime_upgrade();

            let reward_points = pallet_staking::ErasRewardPoints::<Runtime>::get(fixture.era);
            assert_eq!(reward_points.total, 20);
            assert_eq!(
                reward_points.individual.get(&unknown_author).copied(),
                Some(20)
            );
            assert!(!reward_points.individual.contains_key(&fixture.validator));
            assert!(crate::migrations::staking_reward_points_stash_remapped());
        });
    }

    #[test]
    fn staking_payout_reward_point_migration_is_one_shot() {
        framenode_chain_spec::ext().execute_with(|| {
            let fixture = setup_staking_payout_fixture(Some(balance!(1000)));

            crate::migrations::RemapStakingRewardPointsToStash::on_runtime_upgrade();
            assert!(crate::migrations::staking_reward_points_stash_remapped());

            pallet_staking::ErasRewardPoints::<Runtime>::insert(
                fixture.era,
                EraRewardPoints {
                    total: 20,
                    individual: vec![(fixture.controller.clone(), 20)].into_iter().collect(),
                },
            );
            crate::migrations::RemapStakingRewardPointsToStash::on_runtime_upgrade();

            let reward_points = pallet_staking::ErasRewardPoints::<Runtime>::get(fixture.era);
            assert_eq!(
                reward_points.individual.get(&fixture.controller).copied(),
                Some(20)
            );
            assert!(!reward_points.individual.contains_key(&fixture.validator));
        });
    }

    #[test]
    fn staking_payout_hook_does_not_mint_on_failed_dispatch() {
        framenode_chain_spec::ext().execute_with(|| {
            let fixture = setup_staking_payout_fixture(Some(balance!(1000)));
            let call = payout_stakers_call(&fixture);
            let pre = staking_payout_pre(&call).expect("staking payout call should be recognized");

            staking_payout_post(Some(pre), &Err(DispatchError::Other("staking failed")));

            assert_no_val_rewards(&fixture);
            assert!(val_staking_reward_paid_events().is_empty());
        });
    }

    #[test]
    #[allow(deprecated)]
    fn signed_extension_post_dispatch_triggers_staking_val_payout() {
        framenode_chain_spec::ext().execute_with(|| {
            let fixture = setup_staking_payout_fixture(Some(balance!(1000)));
            let caller = AccountId32::from([49; 32]);
            fund_fee_payer(&caller);
            let call = payout_stakers_call(&fixture);
            let info = info_from_weight(Weight::from_parts(100, 0));
            let len = 10;

            let pre = ChargeTransactionPayment::<Runtime>::new()
                .pre_dispatch(&caller, &call, &info, len)
                .expect("caller can pay staking payout fee");
            ChargeTransactionPayment::<Runtime>::post_dispatch(
                Some(pre),
                &info,
                &default_post_info(),
                len,
                &Ok(()),
            )
            .expect("post dispatch should settle the fee");

            assert_eq!(
                Currencies::free_balance(VAL.into(), &fixture.validator),
                balance!(400)
            );
            assert_eq!(
                Currencies::free_balance(VAL.into(), &fixture.nominator),
                balance!(600)
            );
            assert_eq!(
                val_staking_reward_paid_events(),
                vec![
                    (
                        fixture.validator.clone(),
                        fixture.validator.clone(),
                        fixture.era,
                        0,
                        balance!(400)
                    ),
                    (
                        fixture.nominator.clone(),
                        fixture.nominator.clone(),
                        fixture.era,
                        0,
                        balance!(600)
                    ),
                ]
            );
        });
    }

    #[test]
    #[allow(deprecated)]
    fn signed_extension_failed_dispatch_does_not_trigger_staking_val_payout() {
        framenode_chain_spec::ext().execute_with(|| {
            let fixture = setup_staking_payout_fixture(Some(balance!(1000)));
            let caller = AccountId32::from([50; 32]);
            fund_fee_payer(&caller);
            let call = payout_stakers_call(&fixture);
            let info = info_from_weight(Weight::from_parts(100, 0));
            let len = 10;

            let pre = ChargeTransactionPayment::<Runtime>::new()
                .pre_dispatch(&caller, &call, &info, len)
                .expect("caller can pay staking payout fee");
            ChargeTransactionPayment::<Runtime>::post_dispatch(
                Some(pre),
                &info,
                &default_post_info(),
                len,
                &Err(DispatchError::Other("staking failed")),
            )
            .expect("post dispatch should settle the fee");

            assert_no_val_rewards(&fixture);
            assert!(val_staking_reward_paid_events().is_empty());
        });
    }

    #[test]
    #[allow(deprecated)]
    fn signed_extension_validate_does_not_trigger_staking_val_payout() {
        framenode_chain_spec::ext().execute_with(|| {
            let fixture = setup_staking_payout_fixture(Some(balance!(1000)));
            let caller = AccountId32::from([51; 32]);
            fund_fee_payer(&caller);
            let call = payout_stakers_call(&fixture);
            let info = info_from_weight(Weight::from_parts(100, 0));
            let len = 10;
            let fee_payer_balance = Balances::usable_balance_for_fees(&caller);

            ChargeTransactionPayment::<Runtime>::new()
                .validate(&caller, &call, &info, len)
                .expect("funded caller validates staking payout fee");

            assert_eq!(
                Balances::usable_balance_for_fees(&caller),
                fee_payer_balance
            );
            assert_no_val_rewards(&fixture);
            assert!(val_staking_reward_paid_events().is_empty());
        });
    }

    #[test]
    #[allow(deprecated)]
    fn signed_extension_unfunded_pre_dispatch_does_not_trigger_staking_val_payout() {
        framenode_chain_spec::ext().execute_with(|| {
            let fixture = setup_staking_payout_fixture(Some(balance!(1000)));
            let caller = AccountId32::from([52; 32]);
            let call = payout_stakers_call(&fixture);
            let info = info_from_weight(Weight::from_parts(100, 0));

            assert!(ChargeTransactionPayment::<Runtime>::new()
                .pre_dispatch(&caller, &call, &info, 10)
                .is_err());
            assert_no_val_rewards(&fixture);
            assert!(val_staking_reward_paid_events().is_empty());
        });
    }

    #[test]
    #[allow(deprecated)]
    fn signed_extension_post_dispatch_without_pre_does_not_trigger_staking_val_payout() {
        framenode_chain_spec::ext().execute_with(|| {
            let fixture = setup_staking_payout_fixture(Some(balance!(1000)));
            let info = info_from_weight(Weight::from_parts(100, 0));

            ChargeTransactionPayment::<Runtime>::post_dispatch(
                None,
                &info,
                &default_post_info(),
                10,
                &Ok(()),
            )
            .expect("post dispatch without pre should be a noop");

            assert_no_val_rewards(&fixture);
            assert!(val_staking_reward_paid_events().is_empty());
        });
    }

    #[test]
    fn staking_payout_hook_does_not_mint_without_recorded_val_reward() {
        framenode_chain_spec::ext().execute_with(|| {
            let fixture = setup_staking_payout_fixture(None);
            let call = payout_stakers_call(&fixture);
            let pre = staking_payout_pre(&call).expect("staking payout call should be recognized");

            staking_payout_post(Some(pre), &Ok(()));

            assert_no_val_rewards(&fixture);
        });
    }

    #[test]
    fn staking_payout_hook_respects_reward_destination_none() {
        framenode_chain_spec::ext().execute_with(|| {
            let fixture = setup_staking_payout_fixture(Some(balance!(1000)));
            pallet_staking::Payee::<Runtime>::insert(&fixture.validator, RewardDestination::None);
            pallet_staking::Payee::<Runtime>::insert(&fixture.nominator, RewardDestination::None);
            let call = payout_stakers_call(&fixture);
            let pre = staking_payout_pre(&call).expect("staking payout call should be recognized");

            staking_payout_post(Some(pre), &Ok(()));

            assert_no_val_rewards(&fixture);
            assert!(val_staking_reward_paid_events().is_empty());
        });
    }

    #[test]
    fn staking_payout_hook_pays_account_reward_destinations() {
        framenode_chain_spec::ext().execute_with(|| {
            let fixture = setup_staking_payout_fixture(Some(balance!(1000)));
            let validator_payee = AccountId32::from([44; 32]);
            let nominator_payee = AccountId32::from([45; 32]);
            pallet_staking::Payee::<Runtime>::insert(
                &fixture.validator,
                RewardDestination::Account(validator_payee.clone()),
            );
            pallet_staking::Payee::<Runtime>::insert(
                &fixture.nominator,
                RewardDestination::Account(nominator_payee.clone()),
            );
            let call = payout_stakers_call(&fixture);
            let pre = staking_payout_pre(&call).expect("staking payout call should be recognized");

            staking_payout_post(Some(pre), &Ok(()));

            assert_no_val_rewards(&fixture);
            assert_eq!(
                Currencies::free_balance(VAL.into(), &validator_payee),
                balance!(400)
            );
            assert_eq!(
                Currencies::free_balance(VAL.into(), &nominator_payee),
                balance!(600)
            );
            assert_eq!(
                val_staking_reward_paid_events(),
                vec![
                    (
                        fixture.validator.clone(),
                        validator_payee,
                        fixture.era,
                        0,
                        balance!(400)
                    ),
                    (
                        fixture.nominator.clone(),
                        nominator_payee,
                        fixture.era,
                        0,
                        balance!(600)
                    ),
                ]
            );
        });
    }

    #[test]
    #[allow(deprecated)]
    fn staking_payout_hook_skips_controller_destination_without_bond() {
        framenode_chain_spec::ext().execute_with(|| {
            let fixture = setup_staking_payout_fixture(Some(balance!(1000)));
            pallet_staking::Payee::<Runtime>::insert(
                &fixture.nominator,
                RewardDestination::Controller,
            );
            let call = payout_stakers_call(&fixture);
            let pre = staking_payout_pre(&call).expect("staking payout call should be recognized");

            staking_payout_post(Some(pre), &Ok(()));

            assert_eq!(
                Currencies::free_balance(VAL.into(), &fixture.validator),
                balance!(400)
            );
            assert_eq!(Currencies::free_balance(VAL.into(), &fixture.nominator), 0);
            assert_eq!(
                val_staking_reward_paid_events(),
                vec![(
                    fixture.validator.clone(),
                    fixture.validator.clone(),
                    fixture.era,
                    0,
                    balance!(400)
                )]
            );
        });
    }

    #[test]
    fn staking_payout_hook_does_not_mint_with_zero_total_reward_points() {
        framenode_chain_spec::ext().execute_with(|| {
            let fixture = setup_staking_payout_fixture(Some(balance!(1000)));
            pallet_staking::ErasRewardPoints::<Runtime>::insert(
                fixture.era,
                EraRewardPoints {
                    total: 0,
                    individual: vec![(fixture.validator.clone(), 10)].into_iter().collect(),
                },
            );
            let call = payout_stakers_call(&fixture);
            let pre = staking_payout_pre(&call).expect("staking payout call should be recognized");

            staking_payout_post(Some(pre), &Ok(()));

            assert_no_val_rewards(&fixture);
        });
    }

    #[test]
    fn staking_payout_hook_does_not_mint_when_validator_has_no_reward_points() {
        framenode_chain_spec::ext().execute_with(|| {
            let fixture = setup_staking_payout_fixture(Some(balance!(1000)));
            pallet_staking::ErasRewardPoints::<Runtime>::insert(
                fixture.era,
                EraRewardPoints {
                    total: 10,
                    individual: Default::default(),
                },
            );
            let call = payout_stakers_call(&fixture);
            let pre = staking_payout_pre(&call).expect("staking payout call should be recognized");

            staking_payout_post(Some(pre), &Ok(()));

            assert_no_val_rewards(&fixture);
        });
    }

    #[test]
    fn staking_payout_hook_uses_validator_reward_point_share() {
        framenode_chain_spec::ext().execute_with(|| {
            let fixture = setup_staking_payout_fixture(Some(balance!(1000)));
            let other_validator = AccountId32::from([47; 32]);
            pallet_staking::ErasRewardPoints::<Runtime>::insert(
                fixture.era,
                EraRewardPoints {
                    total: 20,
                    individual: vec![(fixture.validator.clone(), 5), (other_validator, 15)]
                        .into_iter()
                        .collect(),
                },
            );
            let call = payout_stakers_call(&fixture);
            let pre = staking_payout_pre(&call).expect("staking payout call should be recognized");

            staking_payout_post(Some(pre), &Ok(()));

            assert_eq!(
                Currencies::free_balance(VAL.into(), &fixture.validator),
                balance!(100)
            );
            assert_eq!(
                Currencies::free_balance(VAL.into(), &fixture.nominator),
                balance!(150)
            );
        });
    }

    #[test]
    fn staking_payout_hook_full_commission_pays_only_validator() {
        framenode_chain_spec::ext().execute_with(|| {
            let fixture = setup_staking_payout_fixture(Some(balance!(1000)));
            pallet_staking::ErasValidatorPrefs::<Runtime>::insert(
                fixture.era,
                &fixture.validator,
                ValidatorPrefs {
                    commission: Perbill::one(),
                    ..Default::default()
                },
            );
            let call = payout_stakers_call(&fixture);
            let pre = staking_payout_pre(&call).expect("staking payout call should be recognized");

            staking_payout_post(Some(pre), &Ok(()));

            assert_eq!(
                Currencies::free_balance(VAL.into(), &fixture.validator),
                balance!(1000)
            );
            assert_eq!(Currencies::free_balance(VAL.into(), &fixture.nominator), 0);
        });
    }

    #[test]
    fn staking_payout_hook_missing_payee_skips_only_that_staker() {
        framenode_chain_spec::ext().execute_with(|| {
            let fixture = setup_staking_payout_fixture(Some(balance!(1000)));
            pallet_staking::Payee::<Runtime>::remove(&fixture.nominator);
            let call = payout_stakers_call(&fixture);
            let pre = staking_payout_pre(&call).expect("staking payout call should be recognized");

            staking_payout_post(Some(pre), &Ok(()));

            assert_eq!(
                Currencies::free_balance(VAL.into(), &fixture.validator),
                balance!(400)
            );
            assert_eq!(Currencies::free_balance(VAL.into(), &fixture.nominator), 0);
            assert_eq!(
                val_staking_reward_paid_events(),
                vec![(
                    fixture.validator.clone(),
                    fixture.validator.clone(),
                    fixture.era,
                    0,
                    balance!(400)
                )]
            );
        });
    }

    #[test]
    fn staking_payout_pre_dispatch_rejects_legacy_already_claimed_era() {
        framenode_chain_spec::ext().execute_with(|| {
            let fixture = setup_staking_payout_fixture(Some(balance!(1000)));
            pallet_staking::Ledger::<Runtime>::mutate(&fixture.controller, |ledger| {
                ledger
                    .as_mut()
                    .expect("test ledger exists")
                    .legacy_claimed_rewards
                    .try_push(fixture.era)
                    .expect("history depth accepts one era");
            });

            assert!(staking_payout_pre(&payout_stakers_call(&fixture)).is_none());
            assert_no_val_rewards(&fixture);
        });
    }

    #[test]
    fn staking_payout_pre_dispatch_returns_none_for_bonded_validator_without_ledger() {
        framenode_chain_spec::ext().execute_with(|| {
            let fixture = setup_staking_payout_fixture(Some(balance!(1000)));
            pallet_staking::Ledger::<Runtime>::remove(&fixture.controller);

            assert!(staking_payout_pre(&payout_stakers_call(&fixture)).is_none());
            assert_no_val_rewards(&fixture);
        });
    }

    #[test]
    fn staking_payout_pre_dispatch_returns_none_for_corrupt_bonded_ledger() {
        framenode_chain_spec::ext().execute_with(|| {
            let fixture = setup_staking_payout_fixture(Some(balance!(1000)));
            pallet_staking::Ledger::<Runtime>::mutate(&fixture.controller, |ledger| {
                ledger.as_mut().expect("test ledger exists").stash = AccountId32::from([48; 32]);
            });

            assert!(staking_payout_pre(&payout_stakers_call(&fixture)).is_none());
            assert_no_val_rewards(&fixture);
        });
    }

    #[test]
    fn staking_payout_pre_dispatch_ignores_non_staking_and_nested_utility_calls() {
        framenode_chain_spec::ext().execute_with(|| {
            let fixture = setup_staking_payout_fixture(Some(balance!(1000)));
            let payout_call = payout_stakers_call(&fixture);
            let non_staking_call = RuntimeCall::System(frame_system::Call::remark {
                remark: b"not staking".to_vec(),
            });
            let batch_call = RuntimeCall::Utility(UtilityCall::batch_all {
                calls: vec![payout_call],
            });

            assert!(staking_payout_pre(&non_staking_call).is_none());
            assert!(staking_payout_pre(&batch_call).is_none());
            staking_payout_post(None, &Ok(()));
            assert_no_val_rewards(&fixture);
        });
    }

    #[test]
    fn staking_payout_pre_dispatch_returns_none_for_unbonded_validator() {
        framenode_chain_spec::ext().execute_with(|| {
            let validator = AccountId32::from([46; 32]);
            let call = RuntimeCall::Staking(pallet_staking::Call::payout_stakers {
                validator_stash: validator,
                era: 0,
            });

            assert!(staking_payout_pre(&call).is_none());
        });
    }

    #[test]
    fn staking_payout_by_page_uses_only_requested_paged_exposure() {
        framenode_chain_spec::ext().execute_with(|| {
            let fixture = setup_staking_payout_fixture(Some(balance!(1000)));
            pallet_staking::ErasStakersOverview::<Runtime>::insert(
                fixture.era,
                &fixture.validator,
                PagedExposureMetadata {
                    total: 100,
                    own: 40,
                    nominator_count: 2,
                    page_count: 2,
                },
            );
            pallet_staking::ErasStakersPaged::<Runtime>::insert(
                (fixture.era, &fixture.validator, 1),
                ExposurePage {
                    page_total: 30,
                    others: vec![IndividualExposure {
                        who: fixture.nominator.clone(),
                        value: 30,
                    }],
                },
            );
            let call = RuntimeCall::Staking(pallet_staking::Call::payout_stakers_by_page {
                validator_stash: fixture.validator.clone(),
                era: fixture.era,
                page: 1,
            });
            let pre = staking_payout_pre(&call).expect("by-page call should be recognized");

            staking_payout_post(Some(pre), &Ok(()));

            assert_eq!(Currencies::free_balance(VAL.into(), &fixture.validator), 0);
            assert_eq!(
                Currencies::free_balance(VAL.into(), &fixture.nominator),
                balance!(300)
            );
        });
    }

    #[test]
    fn staking_payout_by_page_prorates_commission_to_requested_page() {
        framenode_chain_spec::ext().execute_with(|| {
            let fixture = setup_staking_payout_fixture(Some(balance!(1000)));
            pallet_staking::ErasStakersOverview::<Runtime>::insert(
                fixture.era,
                &fixture.validator,
                PagedExposureMetadata {
                    total: 100,
                    own: 40,
                    nominator_count: 2,
                    page_count: 2,
                },
            );
            pallet_staking::ErasStakersPaged::<Runtime>::insert(
                (fixture.era, &fixture.validator, 1),
                ExposurePage {
                    page_total: 30,
                    others: vec![IndividualExposure {
                        who: fixture.nominator.clone(),
                        value: 30,
                    }],
                },
            );
            pallet_staking::ErasValidatorPrefs::<Runtime>::insert(
                fixture.era,
                &fixture.validator,
                ValidatorPrefs {
                    commission: Perbill::from_percent(10),
                    ..Default::default()
                },
            );
            let call = RuntimeCall::Staking(pallet_staking::Call::payout_stakers_by_page {
                validator_stash: fixture.validator.clone(),
                era: fixture.era,
                page: 1,
            });
            let pre = staking_payout_pre(&call).expect("by-page call should be recognized");

            staking_payout_post(Some(pre), &Ok(()));

            assert_eq!(
                Currencies::free_balance(VAL.into(), &fixture.validator),
                balance!(30)
            );
            assert_eq!(
                Currencies::free_balance(VAL.into(), &fixture.nominator),
                balance!(270)
            );
        });
    }

    #[test]
    fn staking_payout_by_page_zero_prorates_own_stake_and_commission() {
        framenode_chain_spec::ext().execute_with(|| {
            let fixture = setup_staking_payout_fixture(Some(balance!(1000)));
            pallet_staking::ErasStakersOverview::<Runtime>::insert(
                fixture.era,
                &fixture.validator,
                PagedExposureMetadata {
                    total: 100,
                    own: 40,
                    nominator_count: 2,
                    page_count: 2,
                },
            );
            pallet_staking::ErasStakersPaged::<Runtime>::insert(
                (fixture.era, &fixture.validator, 0),
                ExposurePage {
                    page_total: 30,
                    others: vec![IndividualExposure {
                        who: fixture.nominator.clone(),
                        value: 30,
                    }],
                },
            );
            pallet_staking::ErasValidatorPrefs::<Runtime>::insert(
                fixture.era,
                &fixture.validator,
                ValidatorPrefs {
                    commission: Perbill::from_percent(10),
                    ..Default::default()
                },
            );
            let call = RuntimeCall::Staking(pallet_staking::Call::payout_stakers_by_page {
                validator_stash: fixture.validator.clone(),
                era: fixture.era,
                page: 0,
            });
            let pre = staking_payout_pre(&call).expect("by-page call should be recognized");

            staking_payout_post(Some(pre), &Ok(()));

            assert_eq!(
                Currencies::free_balance(VAL.into(), &fixture.validator),
                balance!(430)
            );
            assert_eq!(
                Currencies::free_balance(VAL.into(), &fixture.nominator),
                balance!(270)
            );
        });
    }

    #[test]
    fn staking_payout_handles_self_only_zero_page_count_overview() {
        framenode_chain_spec::ext().execute_with(|| {
            let fixture = setup_staking_payout_fixture(Some(balance!(1000)));
            pallet_staking::ErasStakersClipped::<Runtime>::remove(fixture.era, &fixture.validator);
            pallet_staking::ErasStakersOverview::<Runtime>::insert(
                fixture.era,
                &fixture.validator,
                PagedExposureMetadata {
                    total: 100,
                    own: 100,
                    nominator_count: 0,
                    page_count: 0,
                },
            );
            let call = payout_stakers_call(&fixture);
            let pre = staking_payout_pre(&call)
                .expect("self-only zero-page overview should remain payable");

            assert_eq!(pre.page, 0);
            staking_payout_post(Some(pre), &Ok(()));

            assert_eq!(
                Currencies::free_balance(VAL.into(), &fixture.validator),
                balance!(1000)
            );
            assert_eq!(Currencies::free_balance(VAL.into(), &fixture.nominator), 0);
        });
    }

    #[test]
    fn staking_payout_pre_dispatch_skips_claimed_paged_pages() {
        framenode_chain_spec::ext().execute_with(|| {
            let fixture = setup_staking_payout_fixture(Some(balance!(1000)));
            pallet_staking::ErasStakersClipped::<Runtime>::remove(fixture.era, &fixture.validator);
            pallet_staking::ErasStakersOverview::<Runtime>::insert(
                fixture.era,
                &fixture.validator,
                PagedExposureMetadata {
                    total: 100,
                    own: 40,
                    nominator_count: 2,
                    page_count: 2,
                },
            );
            pallet_staking::ClaimedRewards::<Runtime>::insert(
                fixture.era,
                &fixture.validator,
                vec![0],
            );

            let pre = staking_payout_pre(&payout_stakers_call(&fixture))
                .expect("second page should remain claimable");

            assert_eq!(pre.page, 1);
        });
    }

    #[test]
    fn staking_payout_pre_dispatch_rejects_fully_claimed_paged_exposure() {
        framenode_chain_spec::ext().execute_with(|| {
            let fixture = setup_staking_payout_fixture(Some(balance!(1000)));
            pallet_staking::ErasStakersClipped::<Runtime>::remove(fixture.era, &fixture.validator);
            pallet_staking::ErasStakersOverview::<Runtime>::insert(
                fixture.era,
                &fixture.validator,
                PagedExposureMetadata {
                    total: 100,
                    own: 40,
                    nominator_count: 2,
                    page_count: 2,
                },
            );
            pallet_staking::ClaimedRewards::<Runtime>::insert(
                fixture.era,
                &fixture.validator,
                vec![0, 1],
            );

            assert!(staking_payout_pre(&payout_stakers_call(&fixture)).is_none());
            assert_no_val_rewards(&fixture);
        });
    }

    #[test]
    fn staking_payout_pre_dispatch_rejects_empty_paged_overview() {
        framenode_chain_spec::ext().execute_with(|| {
            let fixture = setup_staking_payout_fixture(Some(balance!(1000)));
            pallet_staking::ErasStakersClipped::<Runtime>::remove(fixture.era, &fixture.validator);
            pallet_staking::ErasStakersOverview::<Runtime>::insert(
                fixture.era,
                &fixture.validator,
                PagedExposureMetadata {
                    total: 0,
                    own: 0,
                    nominator_count: 0,
                    page_count: 0,
                },
            );

            assert!(staking_payout_pre(&payout_stakers_call(&fixture)).is_none());
            assert_no_val_rewards(&fixture);
        });
    }

    #[test]
    fn staking_payout_hook_zero_total_exposure_mints_nothing() {
        framenode_chain_spec::ext().execute_with(|| {
            let fixture = setup_staking_payout_fixture(Some(balance!(1000)));
            pallet_staking::ErasStakersClipped::<Runtime>::insert(
                fixture.era,
                &fixture.validator,
                Exposure {
                    total: 0,
                    own: 0,
                    others: vec![IndividualExposure {
                        who: fixture.nominator.clone(),
                        value: 100,
                    }],
                },
            );
            let call = payout_stakers_call(&fixture);
            let pre = staking_payout_pre(&call).expect("staking payout call should be recognized");

            staking_payout_post(Some(pre), &Ok(()));

            assert_no_val_rewards(&fixture);
        });
    }

    #[test]
    fn staking_payout_tiny_validator_share_mints_no_zero_value_events() {
        framenode_chain_spec::ext().execute_with(|| {
            let fixture = setup_staking_payout_fixture(Some(1));
            pallet_staking::ErasRewardPoints::<Runtime>::insert(
                fixture.era,
                EraRewardPoints {
                    total: 10,
                    individual: vec![(fixture.validator.clone(), 1)].into_iter().collect(),
                },
            );
            let call = payout_stakers_call(&fixture);
            let pre = staking_payout_pre(&call).expect("staking payout call should be recognized");

            staking_payout_post(Some(pre), &Ok(()));

            assert_no_val_rewards(&fixture);
            assert!(val_staking_reward_paid_events().is_empty());
        });
    }

    #[test]
    fn staking_payout_by_page_with_missing_exposure_mints_nothing() {
        framenode_chain_spec::ext().execute_with(|| {
            let fixture = setup_staking_payout_fixture(Some(balance!(1000)));
            let call = RuntimeCall::Staking(pallet_staking::Call::payout_stakers_by_page {
                validator_stash: fixture.validator.clone(),
                era: fixture.era,
                page: 9,
            });
            let pre = staking_payout_pre(&call).expect("by-page call should be recognized");

            staking_payout_post(Some(pre), &Ok(()));

            assert_no_val_rewards(&fixture);
            assert!(val_staking_reward_paid_events().is_empty());
        });
    }

    #[test]
    fn staking_payout_by_page_claimed_page_failed_dispatch_mints_nothing() {
        framenode_chain_spec::ext().execute_with(|| {
            let fixture = setup_staking_payout_fixture(Some(balance!(1000)));
            pallet_staking::ClaimedRewards::<Runtime>::insert(
                fixture.era,
                &fixture.validator,
                vec![0],
            );
            let call = RuntimeCall::Staking(pallet_staking::Call::payout_stakers_by_page {
                validator_stash: fixture.validator.clone(),
                era: fixture.era,
                page: 0,
            });
            let pre = staking_payout_pre(&call).expect("by-page call should be recognized");

            staking_payout_post(Some(pre), &Err(DispatchError::Other("already claimed")));

            assert_no_val_rewards(&fixture);
            assert!(val_staking_reward_paid_events().is_empty());
        });
    }

    #[test]
    fn staking_payout_by_page_failed_dispatch_mints_nothing() {
        framenode_chain_spec::ext().execute_with(|| {
            let fixture = setup_staking_payout_fixture(Some(balance!(1000)));
            let call = RuntimeCall::Staking(pallet_staking::Call::payout_stakers_by_page {
                validator_stash: fixture.validator.clone(),
                era: fixture.era,
                page: 0,
            });
            let pre = staking_payout_pre(&call).expect("by-page call should be recognized");

            staking_payout_post(Some(pre), &Err(DispatchError::Other("staking failed")));

            assert_no_val_rewards(&fixture);
            assert!(val_staking_reward_paid_events().is_empty());
        });
    }

    #[test]
    fn simple_call_should_pass() {
        let call = RuntimeCall::Assets(assets::Call::transfer {
            asset_id: GetBaseAssetId::get(),
            to: From::from([1; 32]),
            amount: balance!(100),
        });

        assert_eq!(
            call.swap_count_and_depth(0),
            CallDepth {
                swap_count: 0,
                depth: 0,
            }
        );

        let schedule = VestingScheduleVariant::LinearVestingSchedule(LinearVestingSchedule {
            asset_id: DOT,
            start: 0u32,
            period: 10u32,
            period_count: 2u32,
            per_period: 10,
            remainder_amount: 0,
        });
        let call = RuntimeCall::VestedRewards(vested_rewards::Call::vested_transfer {
            dest: From::from([1; 32]),
            schedule: schedule.clone(),
        });

        assert_eq!(
            call.swap_count_and_depth(0),
            CallDepth {
                swap_count: 0,
                depth: 0,
            }
        );
    }

    #[test]
    fn xorless_call_vesting_should_pass() {
        let schedule = VestingScheduleVariant::LinearVestingSchedule(LinearVestingSchedule {
            asset_id: DOT,
            start: 0u32,
            period: 10u32,
            period_count: 2u32,
            per_period: 10,
            remainder_amount: 0,
        });
        let call = RuntimeCall::XorFee(xor_fee::Call::xorless_call {
            call: Box::new(RuntimeCall::VestedRewards(
                vested_rewards::Call::vested_transfer {
                    dest: From::from([1; 32]),
                    schedule: schedule.clone(),
                },
            )),
            asset_id: None,
        });

        assert_eq!(
            call.swap_count_and_depth(0),
            CallDepth {
                swap_count: 0,
                depth: 1,
            }
        );
    }

    #[test]
    fn regular_batch_should_pass() {
        let batch_calls = vec![
            assets::Call::transfer {
                asset_id: GetBaseAssetId::get(),
                to: From::from([1; 32]),
                amount: balance!(100),
            }
            .into(),
            assets::Call::transfer {
                asset_id: GetBaseAssetId::get(),
                to: From::from([1; 32]),
                amount: balance!(100),
            }
            .into(),
        ];

        let call_batch = RuntimeCall::Utility(UtilityCall::batch {
            calls: batch_calls.clone(),
        });
        let call_batch_all = RuntimeCall::Utility(UtilityCall::batch_all { calls: batch_calls });

        assert_eq!(
            call_batch.swap_count_and_depth(0),
            CallDepth {
                swap_count: 0,
                depth: 0,
            }
        );
        assert_eq!(
            call_batch_all.swap_count_and_depth(0),
            CallDepth {
                swap_count: 0,
                depth: 0,
            }
        );
    }

    #[test]
    fn regular_batch_should_not_pass_for_vesting() {
        let schedule = VestingScheduleVariant::LinearVestingSchedule(LinearVestingSchedule {
            asset_id: DOT,
            start: 0u32,
            period: 10u32,
            period_count: 2u32,
            per_period: 10,
            remainder_amount: 0,
        });
        let call = RuntimeCall::VestedRewards(vested_rewards::Call::vested_transfer {
            dest: From::from([1; 32]),
            schedule: schedule.clone(),
        });
        let batch_calls = vec![
            call,
            assets::Call::transfer {
                asset_id: GetBaseAssetId::get(),
                to: From::from([1; 32]),
                amount: balance!(100),
            }
            .into(),
        ];

        let call_batch = RuntimeCall::Utility(UtilityCall::batch {
            calls: batch_calls.clone(),
        });
        let call_batch_all = RuntimeCall::Utility(UtilityCall::batch_all { calls: batch_calls });

        assert_eq!(
            call_batch.swap_count_and_depth(0),
            CallDepth {
                swap_count: 0,
                depth: 2,
            }
        );
        assert_eq!(
            call_batch_all.swap_count_and_depth(0),
            CallDepth {
                swap_count: 0,
                depth: 2,
            }
        );
    }

    #[test]
    fn no_direct_call_not_work_for_vesting() {
        let schedule = VestingScheduleVariant::LinearVestingSchedule(LinearVestingSchedule {
            asset_id: DOT,
            start: 0u32,
            period: 10u32,
            period_count: 2u32,
            per_period: 10,
            remainder_amount: 0,
        });
        let call = Box::new(RuntimeCall::VestedRewards(
            vested_rewards::Call::vested_transfer {
                dest: From::from([1; 32]),
                schedule: schedule.clone(),
            },
        ));

        let utility_call = RuntimeCall::Utility(UtilityCall::as_derivative { index: 0, call });

        assert_eq!(
            utility_call.swap_count_and_depth(0),
            CallDepth {
                depth: 2,
                swap_count: 0
            }
        );
    }

    fn test_swap_in_batch(call: RuntimeCall) {
        let batch_calls = vec![
            assets::Call::transfer {
                asset_id: GetBaseAssetId::get(),
                to: From::from([1; 32]),
                amount: balance!(100),
            }
            .into(),
            call,
        ];

        let call_batch = RuntimeCall::Utility(UtilityCall::batch {
            calls: batch_calls.clone(),
        });
        let call_batch_all = RuntimeCall::Utility(UtilityCall::batch_all { calls: batch_calls });

        assert_eq!(
            call_batch.swap_count_and_depth(0),
            CallDepth {
                swap_count: 1,
                depth: 0,
            }
        );
        assert_eq!(
            call_batch_all.swap_count_and_depth(0),
            CallDepth {
                swap_count: 1,
                depth: 0,
            }
        );

        assert!(crate::BaseCallFilter::contains(&call_batch));
        assert!(crate::BaseCallFilter::contains(&call_batch_all));
    }

    #[test]
    fn swap_in_batch_should_fail() {
        test_swap_in_batch(
            liquidity_proxy::Call::swap {
                dex_id: 0,
                input_asset_id: VAL,
                output_asset_id: XOR,
                swap_amount: common::prelude::SwapAmount::WithDesiredInput {
                    desired_amount_in: crate::balance!(100),
                    min_amount_out: crate::balance!(100),
                },
                selected_source_types: vec![],
                filter_mode: common::FilterMode::Disabled,
            }
            .into(),
        );
    }

    #[test]
    fn swap_transfer_in_batch_should_fail() {
        test_swap_in_batch(
            liquidity_proxy::Call::swap_transfer {
                receiver: From::from([1; 32]),
                dex_id: 0,
                input_asset_id: VAL,
                output_asset_id: XOR,
                swap_amount: common::prelude::SwapAmount::WithDesiredInput {
                    desired_amount_in: crate::balance!(100),
                    min_amount_out: crate::balance!(100),
                },
                selected_source_types: vec![],
                filter_mode: common::FilterMode::Disabled,
            }
            .into(),
        );
    }

    #[test]
    fn compute_fee_works_fine() {
        // compute fee works fine for vested transfer

        let schedule = VestingScheduleVariant::LinearVestingSchedule(LinearVestingSchedule {
            asset_id: DOT,
            start: 0u32,
            period: 10u32,
            period_count: 2u32,
            per_period: 10,
            remainder_amount: 0,
        });

        let fee = 3 * SMALL_FEE;
        let fee_without_claims = SMALL_FEE;

        let vesting_call = RuntimeCall::VestedRewards(vested_rewards::Call::vested_transfer {
            dest: From::from([1; 32]),
            schedule,
        });
        let xorless_call_vesting = RuntimeCall::XorFee(xor_fee::Call::xorless_call {
            call: Box::new(vesting_call.clone()),
            asset_id: None,
        });
        assert_eq!(
            CustomFees::compute_fee(&xorless_call_vesting),
            Some((
                fee,
                CustomFeeDetails::VestedTransferClaims((fee, fee_without_claims))
            ))
        );
        assert_eq!(
            CustomFees::compute_fee(&vesting_call),
            Some((
                fee,
                CustomFeeDetails::VestedTransferClaims((fee, fee_without_claims))
            ))
        );

        // compute fee works fine for order book

        let order_book_id = OrderBookId {
            dex_id: common::DEXId::Polkaswap.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };
        let order_call = RuntimeCall::OrderBook(order_book::Call::place_limit_order {
            order_book_id,
            price: balance!(11),
            amount: balance!(100),
            side: PriceVariant::Sell,
            lifespan: None,
        });
        let xorless_call = RuntimeCall::XorFee(xor_fee::Call::xorless_call {
            call: Box::new(order_call.clone()),
            asset_id: None,
        });
        assert_eq!(
            CustomFees::compute_fee(&xorless_call),
            Some((SMALL_FEE, CustomFeeDetails::LimitOrderLifetime(None)))
        );
        assert_eq!(
            CustomFees::compute_fee(&order_call),
            Some((SMALL_FEE, CustomFeeDetails::LimitOrderLifetime(None)))
        );

        // compute fee works fine for Some predefined fee

        let transfer_call = RuntimeCall::Assets(assets::Call::transfer {
            asset_id: GetBaseAssetId::get(),
            to: From::from([1; 32]),
            amount: balance!(100),
        });
        let xorless_call = RuntimeCall::XorFee(xor_fee::Call::xorless_call {
            call: Box::new(transfer_call.clone()),
            asset_id: None,
        });
        assert_eq!(
            CustomFees::compute_fee(&transfer_call),
            Some((SMALL_FEE, CustomFeeDetails::Regular(SMALL_FEE)))
        );
        assert_eq!(
            CustomFees::compute_fee(&xorless_call),
            Some((SMALL_FEE, CustomFeeDetails::Regular(SMALL_FEE)))
        );

        let polkamarkt_call = RuntimeCall::Polkamarkt(pallet_polkamarkt::Call::create_market {
            condition_id: 1,
            close_block: 42,
            seed_liquidity: balance!(100),
        });
        let xorless_call = RuntimeCall::XorFee(xor_fee::Call::xorless_call {
            call: Box::new(polkamarkt_call.clone()),
            asset_id: None,
        });
        assert_eq!(
            CustomFees::compute_fee(&polkamarkt_call),
            Some((SMALL_FEE, CustomFeeDetails::Regular(SMALL_FEE)))
        );
        assert_eq!(
            CustomFees::compute_fee(&xorless_call),
            Some((SMALL_FEE, CustomFeeDetails::Regular(SMALL_FEE)))
        );

        for polkamarkt_call in [
            RuntimeCall::Polkamarkt(pallet_polkamarkt::Call::buy {
                market_id: 1,
                outcome: pallet_polkamarkt::BinaryOutcome::Yes,
                collateral_in: balance!(10),
                min_shares_out: 0,
            }),
            RuntimeCall::Polkamarkt(pallet_polkamarkt::Call::sell {
                market_id: 1,
                outcome: pallet_polkamarkt::BinaryOutcome::No,
                shares_in: balance!(5),
                min_collateral_out: 0,
            }),
            RuntimeCall::Polkamarkt(pallet_polkamarkt::Call::flip_position {
                market_id: 1,
                from_outcome: pallet_polkamarkt::BinaryOutcome::Yes,
                shares_in: balance!(5),
                min_collateral_out: 0,
                min_shares_out: 0,
            }),
        ] {
            let xorless_call = RuntimeCall::XorFee(xor_fee::Call::xorless_call {
                call: Box::new(polkamarkt_call.clone()),
                asset_id: None,
            });
            assert_eq!(
                CustomFees::compute_fee(&polkamarkt_call),
                Some((SMALL_FEE, CustomFeeDetails::Regular(SMALL_FEE)))
            );
            assert_eq!(
                CustomFees::compute_fee(&xorless_call),
                Some((SMALL_FEE, CustomFeeDetails::Regular(SMALL_FEE)))
            );
        }

        // compute fee works fine for others

        let set_call = RuntimeCall::Timestamp(pallet_timestamp::Call::set { now: 1_u64 });
        let xorless_call = RuntimeCall::XorFee(xor_fee::Call::xorless_call {
            call: Box::new(set_call.clone()),
            asset_id: None,
        });
        assert_eq!(CustomFees::compute_fee(&set_call), None);
        assert_eq!(CustomFees::compute_fee(&xorless_call), None);
    }
}
