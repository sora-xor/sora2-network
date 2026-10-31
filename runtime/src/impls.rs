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

use core::marker::PhantomData;

use bridge_types::ton::TonBalance;
use bridge_types::{GenericAccount, GenericAssetId, GenericBalance};
use codec::{Decode, DecodeWithMemTracking, Encode};
use common::weights::BlockWeights;
use common::Denominator;
use frame_support::dispatch::DispatchClass;
use frame_support::traits::OnUnbalanced;
use frame_support::weights::constants::BlockExecutionWeight;
use frame_support::weights::Weight;
use frame_support::{
    dispatch::{DispatchInfo, GetDispatchInfo, PostDispatchInfo},
    traits::Contains,
};

use scale_info::TypeInfo;
use sp_core::U256;
use sp_runtime::traits::{Convert, Dispatchable};
use sp_runtime::{DispatchError, DispatchErrorWithPostInfo};
use sp_staking::StakingAccount;

pub type NegativeImbalanceOf<T> = pallet_balances::NegativeImbalance<T>;

pub struct CollectiveWeightInfo<T>(PhantomData<T>);

pub struct DemocracyWeightInfo;

pub struct PreimageWeightInfo;

pub struct StakingRewardPoints;

impl pallet_authorship::EventHandler<crate::AccountId, crate::BlockNumber> for StakingRewardPoints {
    fn note_author(author: crate::AccountId) {
        let reward_stash = pallet_staking::Pallet::<crate::Runtime>::ledger(
            StakingAccount::Controller(author.clone()),
        )
        .or_else(|_| {
            pallet_staking::Pallet::<crate::Runtime>::ledger(StakingAccount::Stash(author.clone()))
        })
        .map(|ledger| ledger.stash)
        .unwrap_or(author);

        pallet_staking::Pallet::<crate::Runtime>::reward_by_ids([(reward_stash, 20)]);
    }
}

pub struct OnUnbalancedDemocracySlash<T> {
    _marker: PhantomData<T>,
}

const MAX_PREIMAGE_BYTES: u32 = 5 * 1024 * 1024;

impl pallet_preimage::WeightInfo for PreimageWeightInfo {
    fn note_preimage(bytes: u32) -> Weight {
        let max_weight: Weight = BlockWeights::get()
            .get(DispatchClass::Normal)
            .max_extrinsic
            .expect("Democracy pallet must have max extrinsic weight");
        if bytes > MAX_PREIMAGE_BYTES {
            return max_weight.saturating_add(Weight::from_parts(1, 0));
        }
        let weight = <() as pallet_preimage::WeightInfo>::note_preimage(bytes);
        let max_dispatch_weight: Weight = max_weight.saturating_sub(BlockExecutionWeight::get());
        // We want to keep it as high as possible, but can't risk having it reject,
        // so we always the base block execution weight as a max
        max_dispatch_weight.min(weight)
    }

    fn note_requested_preimage(s: u32) -> Weight {
        <() as pallet_preimage::WeightInfo>::note_requested_preimage(s)
    }

    fn note_no_deposit_preimage(s: u32) -> Weight {
        <() as pallet_preimage::WeightInfo>::note_no_deposit_preimage(s)
    }

    fn unnote_preimage() -> Weight {
        <() as pallet_preimage::WeightInfo>::unnote_preimage()
    }

    fn unnote_no_deposit_preimage() -> Weight {
        <() as pallet_preimage::WeightInfo>::unnote_no_deposit_preimage()
    }

    fn request_preimage() -> Weight {
        <() as pallet_preimage::WeightInfo>::request_preimage()
    }

    fn request_no_deposit_preimage() -> Weight {
        <() as pallet_preimage::WeightInfo>::request_no_deposit_preimage()
    }

    fn request_unnoted_preimage() -> Weight {
        <() as pallet_preimage::WeightInfo>::request_unnoted_preimage()
    }

    fn request_requested_preimage() -> Weight {
        <() as pallet_preimage::WeightInfo>::request_requested_preimage()
    }

    fn unrequest_preimage() -> Weight {
        <() as pallet_preimage::WeightInfo>::unrequest_preimage()
    }

    fn unrequest_unnoted_preimage() -> Weight {
        <() as pallet_preimage::WeightInfo>::unrequest_unnoted_preimage()
    }

    fn unrequest_multi_referenced_preimage() -> Weight {
        <() as pallet_preimage::WeightInfo>::unrequest_multi_referenced_preimage()
    }

    fn ensure_updated(n: u32) -> Weight {
        <() as pallet_preimage::WeightInfo>::ensure_updated(n)
    }
}

impl<T: frame_system::Config> pallet_collective::WeightInfo for CollectiveWeightInfo<T> {
    fn set_members(m: u32, n: u32, p: u32) -> Weight {
        <() as pallet_collective::WeightInfo>::set_members(m, n, p)
    }
    fn execute(b: u32, m: u32) -> Weight {
        <() as pallet_collective::WeightInfo>::execute(b, m)
    }
    fn propose_execute(b: u32, m: u32) -> Weight {
        <() as pallet_collective::WeightInfo>::propose_execute(b, m)
    }
    fn propose_proposed(b: u32, m: u32, p: u32) -> Weight {
        <() as pallet_collective::WeightInfo>::propose_proposed(b, m, p)
    }
    fn vote(m: u32) -> Weight {
        <() as pallet_collective::WeightInfo>::vote(m)
    }
    fn close_early_disapproved(m: u32, p: u32) -> Weight {
        <() as pallet_collective::WeightInfo>::close_early_disapproved(m, p)
    }
    fn close_early_approved(bytes: u32, m: u32, p: u32) -> Weight {
        let max_weight: Weight = BlockWeights::get()
            .get(DispatchClass::Normal)
            .max_extrinsic
            .expect("Collective pallet must have max extrinsic weight");
        if bytes > MAX_PREIMAGE_BYTES {
            return max_weight.saturating_add(Weight::from_parts(1, 0));
        }
        let weight = <() as pallet_collective::WeightInfo>::close_early_approved(bytes, m, p);
        let max_dispatch_weight: Weight = max_weight.saturating_sub(BlockExecutionWeight::get());
        // We want to keep it as high as possible, but can't risk having it reject,
        // so we always the base block execution weight as a max
        max_dispatch_weight.min(weight)
    }
    fn close_disapproved(m: u32, p: u32) -> Weight {
        <() as pallet_collective::WeightInfo>::close_disapproved(m, p)
    }
    fn close_approved(bytes: u32, m: u32, p: u32) -> Weight {
        let max_weight: Weight = BlockWeights::get()
            .get(DispatchClass::Normal)
            .max_extrinsic
            .expect("Collective pallet must have max extrinsic weight");
        if bytes > MAX_PREIMAGE_BYTES {
            return max_weight.saturating_add(Weight::from_parts(1, 0));
        }
        let weight = <() as pallet_collective::WeightInfo>::close_approved(bytes, m, p);
        let max_dispatch_weight: Weight = max_weight.saturating_sub(BlockExecutionWeight::get());
        // We want to keep it as high as possible, but can't risk having it reject,
        // so we always the base block execution weight as a max
        max_dispatch_weight.min(weight)
    }
    fn disapprove_proposal(p: u32) -> Weight {
        <() as pallet_collective::WeightInfo>::disapprove_proposal(p)
    }
    fn kill(d: u32, p: u32) -> Weight {
        <() as pallet_collective::WeightInfo>::kill(d, p)
    }
    fn release_proposal_cost() -> Weight {
        <() as pallet_collective::WeightInfo>::release_proposal_cost()
    }
}

impl pallet_democracy::WeightInfo for DemocracyWeightInfo {
    fn on_initialize_base_with_launch_period(r: u32) -> Weight {
        <() as pallet_democracy::WeightInfo>::on_initialize_base_with_launch_period(r)
    }
    fn propose() -> Weight {
        <() as pallet_democracy::WeightInfo>::propose()
    }
    fn second() -> Weight {
        <() as pallet_democracy::WeightInfo>::second()
    }
    fn vote_new() -> Weight {
        <() as pallet_democracy::WeightInfo>::vote_new()
    }
    fn vote_existing() -> Weight {
        <() as pallet_democracy::WeightInfo>::vote_existing()
    }
    fn emergency_cancel() -> Weight {
        <() as pallet_democracy::WeightInfo>::emergency_cancel()
    }
    fn blacklist() -> Weight {
        <() as pallet_democracy::WeightInfo>::blacklist()
    }
    fn external_propose() -> Weight {
        <() as pallet_democracy::WeightInfo>::external_propose()
    }
    fn external_propose_majority() -> Weight {
        <() as pallet_democracy::WeightInfo>::external_propose_majority()
    }
    fn external_propose_default() -> Weight {
        <() as pallet_democracy::WeightInfo>::external_propose_default()
    }
    fn fast_track() -> Weight {
        <() as pallet_democracy::WeightInfo>::fast_track()
    }
    fn veto_external() -> Weight {
        <() as pallet_democracy::WeightInfo>::veto_external()
    }
    fn cancel_proposal() -> Weight {
        <() as pallet_democracy::WeightInfo>::cancel_proposal()
    }
    fn cancel_referendum() -> Weight {
        <() as pallet_democracy::WeightInfo>::cancel_referendum()
    }
    fn on_initialize_base(r: u32) -> Weight {
        <() as pallet_democracy::WeightInfo>::on_initialize_base(r)
    }
    fn delegate(r: u32) -> Weight {
        <() as pallet_democracy::WeightInfo>::delegate(r)
    }
    fn undelegate(r: u32) -> Weight {
        <() as pallet_democracy::WeightInfo>::undelegate(r)
    }
    fn clear_public_proposals() -> Weight {
        <() as pallet_democracy::WeightInfo>::clear_public_proposals()
    }
    fn unlock_remove(r: u32) -> Weight {
        <() as pallet_democracy::WeightInfo>::unlock_remove(r)
    }
    fn unlock_set(r: u32) -> Weight {
        <() as pallet_democracy::WeightInfo>::unlock_set(r)
    }
    fn remove_vote(r: u32) -> Weight {
        <() as pallet_democracy::WeightInfo>::remove_vote(r)
    }
    fn remove_other_vote(r: u32) -> Weight {
        <() as pallet_democracy::WeightInfo>::remove_other_vote(r)
    }

    fn set_external_metadata() -> Weight {
        <() as pallet_democracy::WeightInfo>::set_external_metadata()
    }

    fn clear_external_metadata() -> Weight {
        <() as pallet_democracy::WeightInfo>::clear_external_metadata()
    }

    fn set_proposal_metadata() -> Weight {
        <() as pallet_democracy::WeightInfo>::set_proposal_metadata()
    }

    fn clear_proposal_metadata() -> Weight {
        <() as pallet_democracy::WeightInfo>::clear_proposal_metadata()
    }

    fn set_referendum_metadata() -> Weight {
        <() as pallet_democracy::WeightInfo>::set_referendum_metadata()
    }

    fn clear_referendum_metadata() -> Weight {
        <() as pallet_democracy::WeightInfo>::clear_referendum_metadata()
    }
}

impl<T: frame_system::Config + pallet_staking::Config + pallet_balances::Config>
    OnUnbalanced<NegativeImbalanceOf<T>> for OnUnbalancedDemocracySlash<T>
{
    /// This implementation allows us to handle the funds that were burned in democracy pallet.
    /// Democracy pallet already did `slash_reserved` for them.
    fn on_nonzero_unbalanced(_amount: NegativeImbalanceOf<T>) {}
}

#[derive(Encode, Decode, DecodeWithMemTracking, Clone, PartialEq, Eq, Debug, TypeInfo)]
pub struct DispatchableSubstrateBridgeCall(bridge_types::substrate::BridgeCall);

impl DispatchableSubstrateBridgeCall {
    fn ensure_source_app(&self, origin: &crate::RuntimeOrigin) -> Result<(), DispatchError> {
        use bridge_types::{substrate::BridgeCall, SubNetworkId};
        let source: Result<
            dispatch::RawOrigin<
                bridge_types::types::CallOriginOutput<SubNetworkId, sp_core::H256, ()>,
            >,
            _,
        > = origin.clone().into();
        let Ok(source) = source else {
            return Ok(());
        };
        let allowed = match &self.0 {
            BridgeCall::SubstrateApp(_) => source.origin.network_id == SubNetworkId::Liberland,
            BridgeCall::ParachainApp(_) => matches!(
                source.origin.network_id,
                SubNetworkId::Kusama
                    | SubNetworkId::Polkadot
                    | SubNetworkId::Rococo
                    | SubNetworkId::Alphanet
            ),
            BridgeCall::DataSigner(_) | BridgeCall::MultisigVerifier(_) => true,
            // EVM/TON apps belong to their own authenticated generic channel.
            BridgeCall::XCMApp(_) | BridgeCall::FAApp(_) | BridgeCall::JettonApp(_) => false,
        };
        if allowed {
            Ok(())
        } else {
            Err(DispatchError::BadOrigin)
        }
    }
}

impl Dispatchable for DispatchableSubstrateBridgeCall {
    type RuntimeOrigin = crate::RuntimeOrigin;
    type Config = crate::Runtime;
    type Info = DispatchInfo;
    type PostInfo = PostDispatchInfo;

    fn dispatch(
        self,
        origin: Self::RuntimeOrigin,
    ) -> sp_runtime::DispatchResultWithInfo<Self::PostInfo> {
        self.ensure_source_app(&origin)?;
        frame_support::__private::log::debug!("Dispatching SubstrateBridgeCall: {:?}", self.0);
        match self.0 {
            bridge_types::substrate::BridgeCall::ParachainApp(msg) => {
                let call: parachain_bridge_app::Call<crate::Runtime> = msg.into();
                let call: crate::RuntimeCall = call.into();
                call.dispatch(origin)
            }
            bridge_types::substrate::BridgeCall::XCMApp(_msg) => Err(DispatchErrorWithPostInfo {
                post_info: Default::default(),
                error: DispatchError::Other("Unavailable"),
            }),
            bridge_types::substrate::BridgeCall::DataSigner(msg) => {
                let call: bridge_data_signer::Call<crate::Runtime> = msg.into();
                let call: crate::RuntimeCall = call.into();
                call.dispatch(origin)
            }
            bridge_types::substrate::BridgeCall::MultisigVerifier(msg) => {
                let call: multisig_verifier::Call<crate::Runtime> = msg.into();
                let call: crate::RuntimeCall = call.into();
                call.dispatch(origin)
            }
            bridge_types::substrate::BridgeCall::SubstrateApp(msg) => {
                let call: substrate_bridge_app::Call<crate::Runtime> = msg.try_into()?;
                let call: crate::RuntimeCall = call.into();
                call.dispatch(origin)
            }
            #[cfg(feature = "wip")] // EVM bridge
            bridge_types::substrate::BridgeCall::FAApp(msg) => {
                let call: evm_fungible_app::Call<crate::Runtime> = msg.into();
                let call: crate::RuntimeCall = call.into();
                call.dispatch(origin)
            }
            #[cfg(not(feature = "wip"))] // EVM bridge
            bridge_types::substrate::BridgeCall::FAApp(_) => Err(DispatchErrorWithPostInfo {
                post_info: Default::default(),
                error: DispatchError::Other("Unavailable"),
            }),
            bridge_types::substrate::BridgeCall::JettonApp(msg) => {
                let call: jetton_app::Call<crate::Runtime> = msg.into();
                let call: crate::RuntimeCall = call.into();
                call.dispatch(origin)
            }
        }
    }
}

impl bridge_types::traits::DispatchWithOutcome for DispatchableSubstrateBridgeCall {
    fn dispatch_with_outcome(
        self,
        origin: Self::RuntimeOrigin,
    ) -> Result<bridge_types::traits::MessageDispatchOutcome, DispatchError> {
        use bridge_types::traits::MessageDispatchOutcome;
        self.ensure_source_app(&origin)?;
        match self.0 {
            bridge_types::substrate::BridgeCall::SubstrateApp(message) => {
                let call: substrate_bridge_app::Call<crate::Runtime> = message.try_into()?;
                match call {
                    substrate_bridge_app::Call::mint {
                        asset_id,
                        sender,
                        recipient,
                        amount,
                    } => crate::SubstrateBridgeApp::mint_with_outcome(
                        origin, asset_id, sender, recipient, amount,
                    ),
                    other => crate::RuntimeCall::from(other)
                        .dispatch(origin)
                        .map(|_| MessageDispatchOutcome::Applied)
                        .map_err(|e| e.error),
                }
            }
            other => Self(other)
                .dispatch(origin)
                .map(|_| MessageDispatchOutcome::Applied)
                .map_err(|e| e.error),
        }
    }
}

impl GetDispatchInfo for DispatchableSubstrateBridgeCall {
    fn get_dispatch_info(&self) -> DispatchInfo {
        match &self.0 {
            bridge_types::substrate::BridgeCall::ParachainApp(msg) => {
                let call: parachain_bridge_app::Call<crate::Runtime> = msg.clone().into();
                call.get_dispatch_info()
            }
            bridge_types::substrate::BridgeCall::XCMApp(_msg) => Default::default(),
            bridge_types::substrate::BridgeCall::DataSigner(msg) => {
                let call: bridge_data_signer::Call<crate::Runtime> = msg.clone().into();
                call.get_dispatch_info()
            }
            bridge_types::substrate::BridgeCall::MultisigVerifier(msg) => {
                let call: multisig_verifier::Call<crate::Runtime> = msg.clone().into();
                call.get_dispatch_info()
            }
            bridge_types::substrate::BridgeCall::SubstrateApp(msg) => {
                let call: substrate_bridge_app::Call<crate::Runtime> =
                    match substrate_bridge_app::Call::try_from(msg.clone()) {
                        Ok(c) => c,
                        Err(_) => return Default::default(),
                    };
                call.get_dispatch_info()
            }
            #[cfg(feature = "wip")] // EVM bridge
            bridge_types::substrate::BridgeCall::FAApp(msg) => {
                let call: evm_fungible_app::Call<crate::Runtime> = msg.clone().into();
                call.get_dispatch_info()
            }
            #[cfg(not(feature = "wip"))] // EVM bridge
            bridge_types::substrate::BridgeCall::FAApp(_) => Default::default(),
            bridge_types::substrate::BridgeCall::JettonApp(msg) => {
                let call: jetton_app::Call<crate::Runtime> = msg.clone().into();
                call.get_dispatch_info()
            }
        }
    }
}

pub struct LiberlandAccountIdConverter;
impl Convert<crate::AccountId, GenericAccount> for LiberlandAccountIdConverter {
    fn convert(a: crate::AccountId) -> GenericAccount {
        GenericAccount::Sora(a)
    }
}

pub struct LiberlandAssetIdConverter;
impl Convert<crate::AssetId, GenericAssetId> for LiberlandAssetIdConverter {
    fn convert(a: crate::AssetId) -> GenericAssetId {
        GenericAssetId::Sora(a.into())
    }
}

pub struct BalancePrecisionConverter;

impl BalancePrecisionConverter {
    /// Smallest pair of whole local/remote units representing the same value.
    /// Reducing the denomination/decimal ratio before multiplication prevents
    /// both premature truncation and a factor-scaled local custody amount.
    fn conversion_units(
        asset_id: &crate::AssetId,
        sidechain_precision: u8,
    ) -> Option<(crate::Balance, U256)> {
        let factor = crate::Denomination::current_factor(asset_id);
        if factor == 0 {
            return None;
        }
        let local_precision = crate::Assets::asset_infos_v2(asset_id).precision;
        if local_precision >= sidechain_precision {
            let scale = 10_u128.checked_pow((local_precision - sidechain_precision).into())?;
            let (mut a, mut b) = (factor, scale);
            while b != 0 {
                (a, b) = (b, a % b);
            }
            Some((scale / a, (factor / a).into()))
        } else {
            let scale = 10_u128.checked_pow((sidechain_precision - local_precision).into())?;
            Some((1, U256::from(factor).checked_mul(scale.into())?))
        }
    }

    fn from_remote(
        asset_id: &crate::AssetId,
        sidechain_precision: u8,
        amount: U256,
    ) -> Option<(crate::Balance, U256)> {
        let (local_unit, remote_unit) = Self::conversion_units(asset_id, sidechain_precision)?;
        let units = amount.checked_div(remote_unit)?;
        let local = units.checked_mul(local_unit.into())?.try_into().ok()?;
        Some((local, units.checked_mul(remote_unit)?))
    }

    fn to_remote(
        asset_id: &crate::AssetId,
        sidechain_precision: u8,
        amount: crate::Balance,
    ) -> Option<(crate::Balance, U256)> {
        let (local_unit, remote_unit) = Self::conversion_units(asset_id, sidechain_precision)?;
        let units = amount.checked_div(local_unit)?;
        Some((
            units.checked_mul(local_unit)?,
            U256::from(units).checked_mul(remote_unit)?,
        ))
    }

    #[cfg(test)]
    fn convert_precision(
        precision_from: u8,
        precision_to: u8,
        amount: crate::Balance,
    ) -> Option<(crate::Balance, crate::Balance)> {
        if precision_from == precision_to {
            return Some((amount, amount));
        }
        if precision_from < precision_to {
            let exp = (precision_to - precision_from) as u32;
            let coeff = 10_u128.checked_pow(exp)?;
            let coerced_amount = amount.checked_mul(coeff)?;
            // No rounding in this case
            Some((amount, coerced_amount))
        } else {
            let exp = (precision_from - precision_to) as u32;
            let coeff = 10_u128.checked_pow(exp)?;
            let coerced_amount = amount.checked_div(coeff)?;
            Some((coerced_amount * coeff, coerced_amount))
        }
    }
}

impl bridge_types::traits::BalancePrecisionConverter<crate::AssetId, crate::Balance, crate::Balance>
    for BalancePrecisionConverter
{
    fn from_sidechain(
        asset_id: &crate::AssetId,
        sidechain_precision: u8,
        amount: crate::Balance,
    ) -> Option<(crate::Balance, crate::Balance)> {
        let (local, remote) = Self::from_remote(asset_id, sidechain_precision, amount.into())?;
        Some((local, remote.try_into().ok()?))
    }

    fn to_sidechain(
        asset_id: &crate::AssetId,
        sidechain_precision: u8,
        amount: crate::Balance,
    ) -> Option<(crate::Balance, crate::Balance)> {
        let (local, remote) = Self::to_remote(asset_id, sidechain_precision, amount)?;
        Some((local, remote.try_into().ok()?))
    }
}

impl bridge_types::traits::BalancePrecisionConverter<crate::AssetId, crate::Balance, U256>
    for BalancePrecisionConverter
{
    fn from_sidechain(
        asset_id: &crate::AssetId,
        sidechain_precision: u8,
        amount: U256,
    ) -> Option<(crate::Balance, U256)> {
        Self::from_remote(asset_id, sidechain_precision, amount)
    }

    fn to_sidechain(
        asset_id: &crate::AssetId,
        sidechain_precision: u8,
        amount: crate::Balance,
    ) -> Option<(crate::Balance, U256)> {
        Self::to_remote(asset_id, sidechain_precision, amount)
    }
}

impl bridge_types::traits::BalancePrecisionConverter<crate::AssetId, crate::Balance, TonBalance>
    for BalancePrecisionConverter
{
    fn from_sidechain(
        asset_id: &crate::AssetId,
        sidechain_precision: u8,
        amount: TonBalance,
    ) -> Option<(crate::Balance, TonBalance)> {
        let (local, remote) =
            Self::from_remote(asset_id, sidechain_precision, amount.balance().into())?;
        Some((local, TonBalance::new(remote.try_into().ok()?)))
    }

    fn to_sidechain(
        asset_id: &crate::AssetId,
        sidechain_precision: u8,
        amount: crate::Balance,
    ) -> Option<(crate::Balance, TonBalance)> {
        let (local, remote) = Self::to_remote(asset_id, sidechain_precision, amount)?;
        Some((local, TonBalance::new(remote.try_into().ok()?)))
    }
}

pub struct GenericBalancePrecisionConverter;
impl bridge_types::traits::BalancePrecisionConverter<crate::AssetId, crate::Balance, GenericBalance>
    for GenericBalancePrecisionConverter
{
    fn from_sidechain(
        asset_id: &crate::AssetId,
        sidechain_precision: u8,
        amount: GenericBalance,
    ) -> Option<(crate::Balance, GenericBalance)> {
        use bridge_types::traits::BalancePrecisionConverter as Converter;
        match amount {
            GenericBalance::Substrate(amount) => {
                <BalancePrecisionConverter as Converter<_, _, crate::Balance>>::from_sidechain(
                    asset_id,
                    sidechain_precision,
                    amount,
                )
                .map(|(local, remote)| (local, GenericBalance::Substrate(remote)))
            }
            GenericBalance::TON(amount) => {
                <BalancePrecisionConverter as Converter<_, _, TonBalance>>::from_sidechain(
                    asset_id,
                    sidechain_precision,
                    amount,
                )
                .map(|(local, remote)| (local, GenericBalance::TON(remote)))
            }
            GenericBalance::EVM(_) => None,
        }
    }

    fn to_sidechain(
        asset_id: &crate::AssetId,
        sidechain_precision: u8,
        amount: crate::Balance,
    ) -> Option<(crate::Balance, GenericBalance)> {
        use bridge_types::traits::BalancePrecisionConverter as Converter;
        <BalancePrecisionConverter as Converter<_, _, crate::Balance>>::to_sidechain(
            asset_id,
            sidechain_precision,
            amount,
        )
        .map(|(local, remote)| (local, GenericBalance::Substrate(remote)))
    }
}

pub struct SubstrateBridgeCallFilter;

impl Contains<DispatchableSubstrateBridgeCall> for SubstrateBridgeCallFilter {
    fn contains(call: &DispatchableSubstrateBridgeCall) -> bool {
        match &call.0 {
            bridge_types::substrate::BridgeCall::ParachainApp(_) => true,
            bridge_types::substrate::BridgeCall::XCMApp(_) => false,
            bridge_types::substrate::BridgeCall::DataSigner(_) => true,
            bridge_types::substrate::BridgeCall::MultisigVerifier(_) => true,
            bridge_types::substrate::BridgeCall::SubstrateApp(_) => true,
            #[cfg(feature = "wip")] // EVM bridge
            bridge_types::substrate::BridgeCall::FAApp(_) => true,
            #[cfg(not(feature = "wip"))] // EVM bridge
            bridge_types::substrate::BridgeCall::FAApp(_) => false,
            bridge_types::substrate::BridgeCall::JettonApp(_) => true,
        }
    }
}

pub struct GenericBridgeCallFilter;

impl Contains<DispatchableSubstrateBridgeCall> for GenericBridgeCallFilter {
    fn contains(call: &DispatchableSubstrateBridgeCall) -> bool {
        match &call.0 {
            bridge_types::substrate::BridgeCall::ParachainApp(_) => false,
            bridge_types::substrate::BridgeCall::XCMApp(_) => false,
            bridge_types::substrate::BridgeCall::DataSigner(_) => false,
            bridge_types::substrate::BridgeCall::MultisigVerifier(_) => false,
            bridge_types::substrate::BridgeCall::SubstrateApp(_) => false,
            #[cfg(feature = "wip")] // EVM bridge
            bridge_types::substrate::BridgeCall::FAApp(_) => true,
            #[cfg(not(feature = "wip"))] // EVM bridge
            bridge_types::substrate::BridgeCall::FAApp(_) => false,
            bridge_types::substrate::BridgeCall::JettonApp(_) => true,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use frame_support::weights::Weight;
    use pallet_preimage::WeightInfo;

    const MAX_WEIGHT: Weight = Weight::from_parts(1_459_875_000_000_u64, 0);
    const MEBIBYTE: u32 = 1024 * 1024;

    #[test]
    fn authenticated_bridge_sources_cannot_dispatch_other_network_apps() {
        use bridge_types::{
            substrate::{BridgeCall, ParachainAppCall, SubstrateAppCall},
            traits::{BridgeOriginOutput, DispatchWithOutcome},
            types::{AssetKind, CallOriginOutput, MessageStatus},
            GenericTimepoint, SubNetworkId,
        };
        let origin = |network| -> crate::RuntimeOrigin {
            dispatch::RawOrigin::new(CallOriginOutput::new(
                network,
                sp_core::H256::zero(),
                GenericTimepoint::Unknown,
                (),
            ))
            .into()
        };
        let parachain = DispatchableSubstrateBridgeCall(BridgeCall::ParachainApp(
            ParachainAppCall::FinalizeAssetRegistration {
                asset_id: sp_core::H256::zero(),
                asset_kind: AssetKind::Sidechain,
            },
        ));
        let liberland = DispatchableSubstrateBridgeCall(BridgeCall::SubstrateApp(
            SubstrateAppCall::ReportTransferResult {
                message_id: sp_core::H256::zero(),
                message_status: MessageStatus::Done,
            },
        ));
        assert_eq!(
            parachain.ensure_source_app(&origin(SubNetworkId::Liberland)),
            Err(DispatchError::BadOrigin)
        );
        assert_eq!(
            liberland.ensure_source_app(&origin(SubNetworkId::Kusama)),
            Err(DispatchError::BadOrigin)
        );
        assert!(parachain
            .ensure_source_app(&origin(SubNetworkId::Kusama))
            .is_ok());
        assert!(liberland
            .ensure_source_app(&origin(SubNetworkId::Liberland))
            .is_ok());
        assert_eq!(
            liberland.dispatch_with_outcome(origin(SubNetworkId::Polkadot)),
            Err(DispatchError::BadOrigin)
        );
    }

    #[test]
    fn democracy_weight_info_should_scale_weight_linearly_up_to_max_preimage_size() {
        fn t(bytes: u32, expected_ref_time: u64, name: &str) {
            let actual = PreimageWeightInfo::note_preimage(bytes);
            assert_eq!(actual.ref_time(), expected_ref_time, "{}", name);
            assert!(actual.ref_time() <= MAX_WEIGHT.ref_time(), "{}", name);
        }

        let base = PreimageWeightInfo::note_preimage(0).ref_time();
        let per_byte = PreimageWeightInfo::note_preimage(1)
            .ref_time()
            .saturating_sub(base);

        t(u32::MIN, base, "u32::MIN");
        t(1, base.saturating_add(per_byte), "1");
        t(
            500_000,
            base.saturating_add(per_byte.saturating_mul(500_000)),
            "500_000",
        );
        t(
            1_000_000,
            base.saturating_add(per_byte.saturating_mul(1_000_000)),
            "1_000_000",
        );
        t(
            5 * MEBIBYTE,
            base.saturating_add(per_byte.saturating_mul((5 * MEBIBYTE).into())),
            "5 * MEBIBYTE",
        );
    }

    #[test]
    fn democracy_weight_info_should_overweight_for_huge_preimages() {
        let overweight = PreimageWeightInfo::note_preimage(5 * MEBIBYTE + 1).ref_time();
        assert!(overweight > MAX_WEIGHT.ref_time());
        assert_eq!(
            PreimageWeightInfo::note_preimage(u32::MAX).ref_time(),
            overweight
        );
    }

    #[test]
    fn test_balance_precision_converter() {
        assert_eq!(
            BalancePrecisionConverter::convert_precision(12, 18, 123_u128),
            Some((123_u128, 123_000_000_u128))
        );
        assert_eq!(
            BalancePrecisionConverter::convert_precision(6, 60, 123_u128),
            None
        );
        assert_eq!(
            BalancePrecisionConverter::convert_precision(6, 6, u128::MAX),
            Some((u128::MAX, u128::MAX))
        );
        assert_eq!(
            BalancePrecisionConverter::convert_precision(18, 12, 123_456_789_123_456_789_u128),
            Some((123_456_789_123_000_000_u128, 123_456_789_123_u128))
        );
        assert_eq!(
            BalancePrecisionConverter::convert_precision(18, 12, 123_456_789_123_000_000_u128),
            Some((123_456_789_123_000_000_u128, 123_456_789_123_u128))
        );
        assert_eq!(
            BalancePrecisionConverter::convert_precision(18, 9, 123_456_789_123_000_000_u128),
            Some((123_456_789_000_000_000_u128, 123_456_789_u128))
        );
    }

    #[test]
    fn bridge_precision_converter_returns_current_native_custody_units() {
        type Converter = BalancePrecisionConverter;
        type Runtime = crate::Runtime;
        use bridge_types::traits::BalancePrecisionConverter as Convert;
        sp_io::TestExternalities::new_empty().execute_with(|| {
            let factor = 10_u128.pow(38);
            frame_support::storage::unhashed::put(
                &frame_support::storage::storage_prefix(b"Denomination", b"Denominator"),
                &factor,
            );
            for asset_id in [crate::AssetId::from(common::XOR), common::TBCD.into()] {
                assets::AssetInfosV2::<Runtime>::mutate(asset_id, |info| info.precision = 18);
                let (locked, remote) =
                    <Converter as Convert<_, u128, u128>>::to_sidechain(&asset_id, 18, 2).unwrap();
                assert_eq!((locked, remote), (2, 2 * factor));
                let (returned, _) =
                    <Converter as Convert<_, u128, u128>>::from_sidechain(&asset_id, 18, remote)
                        .unwrap();
                assert_eq!(returned, locked);
                // Scaling must precede division on the return path: dividing
                // 10^32 remote units by 10^38 first incorrectly produced zero.
                let remote = 10_u128.pow(32);
                assert_eq!(
                    <Converter as Convert<_, u128, u128>>::to_sidechain(&asset_id, 12, 1,),
                    Some((1, remote))
                );
                assert_eq!(
                    <Converter as Convert<_, u128, u128>>::from_sidechain(&asset_id, 12, remote,),
                    Some((1, remote))
                );
                assert_eq!(
                    <Converter as Convert<_, u128, u128>>::to_sidechain(&asset_id, 18, 4,),
                    None,
                    "overflow remains rejected"
                );
            }
        });
    }

    #[test]
    fn bridge_precision_converter_preserves_ordinary_precision_dust() {
        use bridge_types::traits::BalancePrecisionConverter as Convert;
        sp_io::TestExternalities::new_empty().execute_with(|| {
            let asset_id = crate::AssetId::from(common::PSWAP);
            assets::AssetInfosV2::<crate::Runtime>::mutate(asset_id, |info| info.precision = 18);
            let (locked, remote) =
                <BalancePrecisionConverter as Convert<_, u128, u128>>::to_sidechain(
                    &asset_id,
                    12,
                    123_456_789_123_456_789,
                )
                .unwrap();
            assert_eq!((locked, remote), (123_456_789_123_000_000, 123_456_789_123));
            let (returned, _) =
                <BalancePrecisionConverter as Convert<_, u128, u128>>::from_sidechain(
                    &asset_id, 12, remote,
                )
                .unwrap();
            assert_eq!(returned, locked);
            assert_eq!(
                <BalancePrecisionConverter as Convert<_, u128, u128>>::to_sidechain(
                    &asset_id, 20, 123,
                ),
                Some((123, 12_300))
            );
        });
    }

    #[test]
    fn bridge_precision_converter_rounds_both_chains_to_whole_custody_units() {
        use bridge_types::traits::BalancePrecisionConverter as Convert;
        sp_io::TestExternalities::new_empty().execute_with(|| {
            let asset_id = crate::AssetId::from(common::XOR);
            assets::AssetInfosV2::<crate::Runtime>::mutate(asset_id, |info| info.precision = 18);
            frame_support::storage::unhashed::put(
                &frame_support::storage::storage_prefix(b"Denomination", b"Denominator"),
                &3u128,
            );
            // The smallest exact pair is 10 local units to 3 remote units.
            // Independently rounding 4 local units to 1 remote unit would
            // undercollateralize it if only floor(10 / 3) local units were locked.
            assert_eq!(
                <BalancePrecisionConverter as Convert<_, u128, u128>>::to_sidechain(
                    &asset_id, 17, 4,
                ),
                Some((0, 0))
            );
            assert_eq!(
                <BalancePrecisionConverter as Convert<_, u128, u128>>::to_sidechain(
                    &asset_id, 17, 10,
                ),
                Some((10, 3))
            );
            assert_eq!(
                <BalancePrecisionConverter as Convert<_, u128, u128>>::from_sidechain(
                    &asset_id, 17, 4,
                ),
                Some((10, 3))
            );
        });
    }

    #[test]
    fn bridge_precision_converter_matches_existing_unscaled_conversions() {
        use bridge_types::traits::BalancePrecisionConverter as Convert;
        sp_io::TestExternalities::new_empty().execute_with(|| {
            let asset_id = crate::AssetId::from(common::PSWAP);
            for local_precision in [0, 6, 12, 18, 38] {
                assets::AssetInfosV2::<crate::Runtime>::mutate(asset_id, |info| {
                    info.precision = local_precision
                });
                for remote_precision in [0, 6, 9, 12, 18, 20, 38, 57, 60] {
                    for amount in [0, 1, 123_456_789_123_456_789, u128::MAX] {
                        assert_eq!(
                            <BalancePrecisionConverter as Convert<_, u128, u128>>::to_sidechain(
                                &asset_id,
                                remote_precision,
                                amount
                            ),
                            BalancePrecisionConverter::convert_precision(
                                local_precision,
                                remote_precision,
                                amount
                            )
                        );
                        assert_eq!(
                            <BalancePrecisionConverter as Convert<_, u128, u128>>::from_sidechain(
                                &asset_id,
                                remote_precision,
                                amount
                            ),
                            BalancePrecisionConverter::convert_precision(
                                remote_precision,
                                local_precision,
                                amount
                            )
                            .map(|(a, b)| (b, a))
                        );
                    }
                }
            }
        });
    }

    #[test]
    fn bridge_precision_converter_wrappers_preserve_admitted_value_roundtrips() {
        use bridge_types::traits::BalancePrecisionConverter as Convert;
        sp_io::TestExternalities::new_empty().execute_with(|| {
            let asset_id = crate::AssetId::from(common::XOR);
            assets::AssetInfosV2::<crate::Runtime>::mutate(asset_id, |info| info.precision = 18);
            for factor in [1, 3, 10, 1000, 10_u128.pow(18), 10_u128.pow(38)] {
                frame_support::storage::unhashed::put(
                    &frame_support::storage::storage_prefix(b"Denomination", b"Denominator"),
                    &factor,
                );
                for precision in [6, 12, 18, 20, 38] {
                    for amount in [0, 1, 2, 9, 10, 11, 12_345, u128::MAX / 5, u128::MAX] {
                        if let Some((local, remote)) =
                            <BalancePrecisionConverter as Convert<_, u128, U256>>::to_sidechain(
                                &asset_id, precision, amount)
                        {
                            assert!(local <= amount);
                            assert_eq!(
                                <BalancePrecisionConverter as Convert<_, u128, U256>>::from_sidechain(
                                    &asset_id, precision, remote), Some((local, remote)));
                        }
                        let narrow = <BalancePrecisionConverter as Convert<_, u128, u128>>::to_sidechain(
                            &asset_id, precision, amount);
                        assert_eq!(
                            <BalancePrecisionConverter as Convert<_, u128, TonBalance>>::to_sidechain(
                                &asset_id, precision, amount),
                            narrow.map(|(local, remote)| (local, TonBalance::new(remote))));
                        assert_eq!(
                            <GenericBalancePrecisionConverter as Convert<_, u128, GenericBalance>>::to_sidechain(
                                &asset_id, precision, amount),
                            narrow.map(|(local, remote)| (local, GenericBalance::Substrate(remote))));
                        if let Some((local, remote)) = narrow {
                            assert_eq!(
                                <BalancePrecisionConverter as Convert<_, u128, u128>>::from_sidechain(
                                    &asset_id, precision, remote), Some((local, remote)));
                            assert_eq!(
                                <BalancePrecisionConverter as Convert<_, u128, TonBalance>>::from_sidechain(
                                    &asset_id, precision, TonBalance::new(remote)),
                                Some((local, TonBalance::new(remote))));
                            for generic in [GenericBalance::Substrate(remote), GenericBalance::TON(TonBalance::new(remote))] {
                                assert_eq!(
                                    <GenericBalancePrecisionConverter as Convert<_, u128, GenericBalance>>::from_sidechain(
                                        &asset_id, precision, generic.clone()), Some((local, generic)));
                            }
                        }
                    }
                }
            }
        });
    }
}
