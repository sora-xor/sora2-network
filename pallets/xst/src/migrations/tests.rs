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

use crate::{
    migrations::{BaseFee, EnabledSynthetics as OldEnabledSynthetics, PermissionedTechAccount},
    mock::*,
    pallet::Pallet,
    EnabledSynthetics,
};
use common::{fixed, Fixed, XSTUSD};
use frame_support::traits::{GetStorageVersion as _, OnRuntimeUpgrade, StorageVersion};

#[test]
fn test() {
    ExtBuilder::default().build().execute_with(|| {
        StorageVersion::new(1).put::<Pallet<Runtime>>();
        BaseFee::<Runtime>::put::<Fixed>(fixed!(0.00666));

        super::CustomSyntheticsUpgrade::<Runtime>::on_runtime_upgrade();

        let info = EnabledSynthetics::<Runtime>::get(XSTUSD)
            .expect("XSTUSD synthetic should be enabled after migration");

        assert_eq!(info.reference_symbol, common::SymbolName::usd().into());
        assert_eq!(info.fee_ratio, fixed!(0.00666));
        assert!(!BaseFee::<Runtime>::exists());
        assert!(!PermissionedTechAccount::<Runtime>::exists());
        assert!(!OldEnabledSynthetics::<Runtime>::exists());

        assert_eq!(Pallet::<Runtime>::on_chain_storage_version(), 2);
    });
}
