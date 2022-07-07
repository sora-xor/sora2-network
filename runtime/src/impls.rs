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

use frame_support::traits::{Currency, OnUnbalanced};
use frame_support::weights::constants::BlockExecutionWeight;
use frame_support::weights::{DispatchClass, Weight};

pub use common::weights::{BlockLength, BlockWeights, TransactionByteFee};

pub type NegativeImbalanceOf<T> = <<T as pallet_staking::Config>::Currency as Currency<
    <T as frame_system::Config>::AccountId,
>>::NegativeImbalance;

pub struct CollectiveWeightInfo<T>(PhantomData<T>);

pub struct DemocracyWeightInfo;

pub struct OnUnbalancedDemocracySlash<T> {
    _marker: PhantomData<T>,
}

const MAX_PREIMAGE_BYTES: u32 = 5 * 1024 * 1024;

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
            return max_weight.saturating_add(1);
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
            return max_weight.saturating_add(1);
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
    fn second(s: u32) -> Weight {
        <() as pallet_democracy::WeightInfo>::second(s)
    }
    fn vote_new(r: u32) -> Weight {
        <() as pallet_democracy::WeightInfo>::vote_new(r)
    }
    fn vote_existing(r: u32) -> Weight {
        <() as pallet_democracy::WeightInfo>::vote_existing(r)
    }
    fn emergency_cancel() -> Weight {
        <() as pallet_democracy::WeightInfo>::emergency_cancel()
    }
    fn blacklist(p: u32) -> Weight {
        <() as pallet_democracy::WeightInfo>::blacklist(p)
    }
    fn external_propose(v: u32) -> Weight {
        <() as pallet_democracy::WeightInfo>::external_propose(v)
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
    fn veto_external(v: u32) -> Weight {
        <() as pallet_democracy::WeightInfo>::veto_external(v)
    }
    fn cancel_proposal(p: u32) -> Weight {
        <() as pallet_democracy::WeightInfo>::cancel_proposal(p)
    }
    fn cancel_referendum() -> Weight {
        <() as pallet_democracy::WeightInfo>::cancel_referendum()
    }
    fn cancel_queued(r: u32) -> Weight {
        <() as pallet_democracy::WeightInfo>::cancel_queued(r)
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
    fn note_preimage(bytes: u32) -> Weight {
        let max_weight: Weight = BlockWeights::get()
            .get(DispatchClass::Normal)
            .max_extrinsic
            .expect("Democracy pallet must have max extrinsic weight");
        if bytes > MAX_PREIMAGE_BYTES {
            return max_weight.saturating_add(1);
        }
        let weight = <() as pallet_democracy::WeightInfo>::note_preimage(bytes);
        let max_dispatch_weight: Weight = max_weight.saturating_sub(BlockExecutionWeight::get());
        // We want to keep it as high as possible, but can't risk having it reject,
        // so we always the base block execution weight as a max
        max_dispatch_weight.min(weight)
    }
    fn note_imminent_preimage(b: u32) -> Weight {
        <() as pallet_democracy::WeightInfo>::note_imminent_preimage(b)
    }
    fn reap_preimage(b: u32) -> Weight {
        <() as pallet_democracy::WeightInfo>::reap_preimage(b)
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

#[cfg(test)]
mod test {
    use super::*;

    use frame_support::weights::Weight;
    use pallet_democracy::WeightInfo;

    const MAX_WEIGHT: Weight = 1_459_875_000_000_u64 as _;
    const MEBIBYTE: u32 = 1024 * 1024;

    // TODO: uncomment
    #[test]
    #[ignore]
    fn democracy_weight_info_should_scale_weight_linearly_up_to_max_preimage_size() {
        fn t(bytes: u32, expected: Weight) {
            let actual = DemocracyWeightInfo::note_preimage(bytes);
            assert_eq!(actual, expected);
            assert!(actual <= MAX_WEIGHT);
        }

        t(u32::MIN, 185073000);
        t(1, 185077000);
        t(500_000, 2185073000);
        t(1_000_000, 4_185_073_000);
        t(5 * MEBIBYTE, 21_156_593_000);
    }

    #[test]
    fn democracy_weight_info_should_overweight_for_huge_preimages() {
        fn t(bytes: u32) {
            let actual = DemocracyWeightInfo::note_preimage(bytes);
            assert_eq!(actual, 1_459_913_702_001_u64);
            assert!(actual > MAX_WEIGHT);
        }

        t(5 * MEBIBYTE + 1);
        t(u32::MAX);
    }
}
