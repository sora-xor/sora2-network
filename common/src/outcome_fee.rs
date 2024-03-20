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

use crate::fixed_wrapper::FixedWrapper;
use crate::{Balance, Fixed};

use codec::{Decode, Encode};
use fixnum::ops::Zero as _;
use frame_support::RuntimeDebug;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::traits::{Saturating, Zero};
use sp_std::collections::btree_map::BTreeMap;

#[derive(
    Encode, Decode, Eq, PartialEq, Clone, Ord, PartialOrd, RuntimeDebug, scale_info::TypeInfo,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct OutcomeFee<AssetId: Ord, AmountType>(pub BTreeMap<AssetId, AmountType>);

impl<AssetId: Ord, AmountType> OutcomeFee<AssetId, AmountType> {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }
}

impl<AssetId: Ord, AmountType> Default for OutcomeFee<AssetId, AmountType> {
    fn default() -> Self {
        Self::new()
    }
}

impl<AssetId: Ord, AmountType: Zero> OutcomeFee<AssetId, AmountType> {
    pub fn from_asset(asset: AssetId, amount: AmountType) -> Self {
        if amount.is_zero() {
            Self::new()
        } else {
            Self(BTreeMap::from([(asset, amount)]))
        }
    }

    pub fn is_zero_fee(&self) -> bool {
        if self.0.is_empty() {
            return true;
        }
        for value in self.0.values() {
            if !value.is_zero() {
                return false;
            }
        }
        true
    }
}

impl<AssetId, AmountType> OutcomeFee<AssetId, AmountType>
where
    AssetId: Ord,
    AmountType: Copy + Zero,
{
    pub fn get_by_asset(&self, asset: &AssetId) -> AmountType {
        self.0.get(asset).copied().unwrap_or(Zero::zero())
    }
}

impl<AssetId, AmountType> OutcomeFee<AssetId, AmountType>
where
    AssetId: Ord,
    AmountType: Copy + Saturating,
{
    pub fn add_by_asset(&mut self, asset: AssetId, amount: AmountType) {
        self.0
            .entry(asset)
            .and_modify(|value| *value = value.saturating_add(amount))
            .or_insert(amount);
    }

    pub fn merge(mut self, other: Self) -> Self {
        for (asset, other_amount) in other.0 {
            self.0
                .entry(asset)
                .and_modify(|amount| *amount = amount.saturating_add(other_amount))
                .or_insert(other_amount);
        }
        self
    }

    pub fn reduce(mut self, other: Self) -> Self
    where
        AmountType: Zero,
    {
        for (asset, other_amount) in other.0 {
            self.0
                .entry(asset)
                .and_modify(|amount| *amount = amount.saturating_sub(other_amount))
                .or_insert(other_amount);
        }
        self.0.retain(|_, amount| !amount.is_zero());
        self
    }
}

impl<AssetId: Ord> OutcomeFee<AssetId, Balance> {
    pub fn rescale_by_ratio(mut self, ratio: FixedWrapper) -> Option<Self> {
        for value in self.0.values_mut() {
            *value = (FixedWrapper::from(*value) * ratio.clone())
                .try_into_balance()
                .ok()?;
        }
        Some(self)
    }

    // Multiply all values by `n`
    pub fn saturating_mul_usize(self, n: usize) -> Self
    where
        AssetId: Copy,
    {
        Self(
            self.0
                .iter()
                .map(|(&asset, amount)| (asset, amount.saturating_mul(n as Balance)))
                .collect(),
        )
    }
}

// Most used fee assets
impl<AssetId, AmountType> OutcomeFee<AssetId, AmountType>
where
    AssetId: Ord + From<crate::AssetId32<crate::PredefinedAssetId>>,
    AmountType: Copy + Zero,
{
    pub fn xor(amount: AmountType) -> Self {
        Self::from_asset(crate::XOR.into(), amount)
    }

    pub fn xst(amount: AmountType) -> Self {
        Self::from_asset(crate::XST.into(), amount)
    }

    pub fn xstusd(amount: AmountType) -> Self {
        Self::from_asset(crate::XSTUSD.into(), amount)
    }

    pub fn get_xor(&self) -> AmountType {
        self.get_by_asset(&crate::XOR.into())
    }

    pub fn get_xst(&self) -> AmountType {
        self.get_by_asset(&crate::XST.into())
    }

    pub fn get_xstusd(&self) -> AmountType {
        self.get_by_asset(&crate::XSTUSD.into())
    }
}

impl<AssetId, AmountType> OutcomeFee<AssetId, AmountType>
where
    AssetId: Ord + From<crate::AssetId32<crate::PredefinedAssetId>>,
    AmountType: Copy + Saturating,
{
    pub fn add_xor(&mut self, amount: AmountType) {
        self.add_by_asset(crate::XOR.into(), amount);
    }

    pub fn add_xst(&mut self, amount: AmountType) {
        self.add_by_asset(crate::XST.into(), amount);
    }

    pub fn add_xstusd(&mut self, amount: AmountType) {
        self.add_by_asset(crate::XSTUSD.into(), amount);
    }
}

// It is needed to have the special impl for Fixed,
// because Fixed doesn't implement some general traits.
impl<AssetId> OutcomeFee<AssetId, Fixed>
where
    AssetId: Ord,
{
    pub fn from_asset_fixed(asset: AssetId, amount: Fixed) -> Self {
        if amount == Fixed::ZERO {
            Self::new()
        } else {
            Self(BTreeMap::from([(asset, amount)]))
        }
    }

    pub fn get_by_asset_fixed(&self, asset: &AssetId) -> Fixed {
        self.0.get(asset).copied().unwrap_or(Fixed::ZERO)
    }
}

impl<AssetId> OutcomeFee<AssetId, Fixed>
where
    AssetId: Ord + From<crate::AssetId32<crate::PredefinedAssetId>>,
{
    pub fn xor_fixed(amount: Fixed) -> Self {
        Self::from_asset_fixed(crate::XOR.into(), amount)
    }

    pub fn xst_fixed(amount: Fixed) -> Self {
        Self::from_asset_fixed(crate::XST.into(), amount)
    }

    pub fn xstusd_fixed(amount: Fixed) -> Self {
        Self::from_asset_fixed(crate::XSTUSD.into(), amount)
    }

    pub fn get_xor_fixed(&self) -> Fixed {
        self.get_by_asset_fixed(&crate::XOR.into())
    }

    pub fn get_xst_fixed(&self) -> Fixed {
        self.get_by_asset_fixed(&crate::XST.into())
    }

    pub fn get_xstusd_fixed(&self) -> Fixed {
        self.get_by_asset_fixed(&crate::XSTUSD.into())
    }
}
