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

use bridge_types::{GenericAccount, GenericAssetId, GenericBalance};
use codec::{Decode, Encode};
use frame_support::dispatch::DispatchClass;
use frame_support::traits::{Currency, OnUnbalanced};
use frame_support::weights::constants::BlockExecutionWeight;
use frame_support::weights::Weight;
use frame_support::{
    dispatch::{DispatchInfo, Dispatchable, GetDispatchInfo, PostDispatchInfo},
    traits::Contains,
    RuntimeDebug,
};

pub use common::weights::{BlockLength, BlockWeights, TransactionByteFee};
use scale_info::TypeInfo;
use sp_core::U256;
use sp_runtime::traits::Convert;
use sp_runtime::{DispatchError, DispatchErrorWithPostInfo};

pub type NegativeImbalanceOf<T> = <<T as pallet_staking::Config>::Currency as Currency<
    <T as frame_system::Config>::AccountId,
>>::NegativeImbalance;

pub struct CollectiveWeightInfo<T>(PhantomData<T>);

pub struct DemocracyWeightInfo;

pub struct PreimageWeightInfo;

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
}

impl<T: frame_system::Config + pallet_staking::Config> OnUnbalanced<NegativeImbalanceOf<T>>
    for OnUnbalancedDemocracySlash<T>
{
    /// This implementation allows us to handle the funds that were burned in democracy pallet.
    /// Democracy pallet already did `slash_reserved` for them.
    fn on_nonzero_unbalanced(_amount: NegativeImbalanceOf<T>) {}
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo)]
pub struct DispatchableSubstrateBridgeCall(bridge_types::substrate::BridgeCall);

impl Dispatchable for DispatchableSubstrateBridgeCall {
    type RuntimeOrigin = crate::RuntimeOrigin;
    type Config = crate::Runtime;
    type Info = DispatchInfo;
    type PostInfo = PostDispatchInfo;

    fn dispatch(
        self,
        origin: Self::RuntimeOrigin,
    ) -> sp_runtime::DispatchResultWithInfo<Self::PostInfo> {
        frame_support::log::debug!("Dispatching SubstrateBridgeCall: {:?}", self.0);
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
        let thischain_precision = crate::Assets::asset_infos_v2(asset_id).precision;
        Self::convert_precision(sidechain_precision, thischain_precision, amount)
            .map(|(a, b)| (b, a))
    }

    fn to_sidechain(
        asset_id: &crate::AssetId,
        sidechain_precision: u8,
        amount: crate::Balance,
    ) -> Option<(crate::Balance, crate::Balance)> {
        let thischain_precision = crate::Assets::asset_infos_v2(asset_id).precision;
        Self::convert_precision(thischain_precision, sidechain_precision, amount)
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
        let thischain_precision = crate::Assets::asset_infos_v2(asset_id).precision;
        Self::convert_precision(
            sidechain_precision,
            thischain_precision,
            amount.try_into().ok()?,
        )
        .map(|(a, b)| (b, a.into()))
    }

    fn to_sidechain(
        asset_id: &crate::AssetId,
        sidechain_precision: u8,
        amount: crate::Balance,
    ) -> Option<(crate::Balance, U256)> {
        let thischain_precision = crate::Assets::asset_infos_v2(asset_id).precision;
        Self::convert_precision(thischain_precision, sidechain_precision, amount)
            .map(|(a, b)| (a, b.into()))
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
        let thischain_precision = crate::Assets::asset_infos_v2(asset_id).precision;
        match amount {
            GenericBalance::Substrate(val) => BalancePrecisionConverter::convert_precision(
                sidechain_precision,
                thischain_precision,
                val,
            )
            .map(|(a, b)| (b, GenericBalance::Substrate(a))),
            GenericBalance::EVM(_) => None,
        }
    }

    fn to_sidechain(
        asset_id: &crate::AssetId,
        sidechain_precision: u8,
        amount: crate::Balance,
    ) -> Option<(crate::Balance, GenericBalance)> {
        let thischain_precision = crate::Assets::asset_infos_v2(asset_id).precision;
        BalancePrecisionConverter::convert_precision(
            thischain_precision,
            sidechain_precision,
            amount,
        )
        .map(|(a, b)| (a, GenericBalance::Substrate(b)))
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
        }
    }
}

#[cfg(feature = "wip")] // EVM bridge
pub struct EVMBridgeCallFilter;

#[cfg(all(feature = "wip", not(feature = "runtime-benchmarks")))] // EVM bridge
impl Contains<crate::RuntimeCall> for EVMBridgeCallFilter {
    fn contains(call: &crate::RuntimeCall) -> bool {
        match call {
            crate::RuntimeCall::EVMFungibleApp(_) => true,
            _ => false,
        }
    }
}

#[cfg(all(feature = "wip", feature = "runtime-benchmarks"))] // EVM bridge
impl Contains<crate::RuntimeCall> for EVMBridgeCallFilter {
    fn contains(_call: &crate::RuntimeCall) -> bool {
        true
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
    fn democracy_weight_info_should_scale_weight_linearly_up_to_max_preimage_size() {
        fn t(bytes: u32, expected: Weight, name: &str) {
            let actual = PreimageWeightInfo::note_preimage(bytes);
            assert_eq!(actual.ref_time(), expected.ref_time(), "{}", name);
            assert!(actual.ref_time() <= MAX_WEIGHT.ref_time(), "{}", name);
        }

        t(u32::MIN, Weight::from_parts(248_828_000, 0), "u32::MIN");
        t(1, Weight::from_parts(248_829_705, 0), "1");
        t(500_000, Weight::from_parts(1_101_328_000, 0), "500_000");
        t(1_000_000, Weight::from_parts(1_953_828_000, 0), "1_000_000");
        t(
            5 * MEBIBYTE,
            Weight::from_parts(9_187_938_400, 0),
            "5 * MEBIBYTE",
        );
    }

    #[test]
    fn democracy_weight_info_should_overweight_for_huge_preimages() {
        fn t(bytes: u32) {
            let actual = PreimageWeightInfo::note_preimage(bytes);
            assert_eq!(actual.ref_time(), 1_459_900_160_001_u64);
            assert!(actual.ref_time() > MAX_WEIGHT.ref_time());
        }

        t(5 * MEBIBYTE + 1);
        t(u32::MAX);
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
}
