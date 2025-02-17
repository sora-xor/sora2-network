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
use pallet_utility::Call as UtilityCall;
use sp_runtime::traits::Zero;
#[cfg(feature = "wip")] // Dynamic fee
use sp_runtime::FixedU128;
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
            | RuntimeCall::PoolXYK(..)
            | RuntimeCall::Rewards(..)
            | RuntimeCall::Staking(pallet_staking::Call::payout_stakers { .. })
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
            RuntimeCall::Band(..) => Some(MINIMAL_FEE),
            RuntimeCall::Soratopia(soratopia::Call::check_in {}) => Some(MINIMAL_FEE),
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
    use pallet_utility::Call as UtilityCall;
    use sp_core::H256;
    use sp_runtime::AccountId32;
    use vested_rewards::vesting_currencies::{LinearVestingSchedule, VestingScheduleVariant};

    use crate::{
        xor_fee_impls::{CallDepth, CustomFeeDetails, CustomFees},
        *,
    };
    use common::OrderBookId;
    use common::{balance, PriceVariant, VAL, XOR};
    use xor_fee::ApplyCustomFees;

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
