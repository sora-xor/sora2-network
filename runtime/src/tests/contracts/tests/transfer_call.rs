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

use crate::tests::contracts::mock::{instantiate_contract, ExtBuilder, GAS_LIMIT};
use crate::tests::contracts::tests::compile_module;
use crate::{assets::WeightInfo, Contracts, Runtime, RuntimeCall};
use codec::{Decode, Encode};
use common::mock::{alice, bob};
use common::{balance, XOR};
use frame_support::{assert_ok, weights::Weight};
use pallet_contracts::{CollectEvents, DebugInfo, Determinism};
use pallet_contracts_primitives::ContractResult;
use sp_core::crypto::AccountId32;

#[test]
fn call_transfer_right() {
    let (code, _hash) = compile_module::<Runtime>("call_runtime_contract").unwrap();
    ExtBuilder::default().build().execute_with(|| {
        let contract_addr: AccountId32 = instantiate_contract(code);

        let call = RuntimeCall::Assets(assets::Call::transfer {
            asset_id: XOR,
            to: bob(),
            amount: balance!(1),
        });

        let result = Contracts::bare_call(
            alice(),
            contract_addr.clone(),
            0,
            GAS_LIMIT,
            None,
            call.encode(),
            DebugInfo::Skip,
            CollectEvents::Skip,
            Determinism::Enforced,
        );

        let ContractResult {
            gas_consumed,
            gas_required,
            storage_deposit: _storage_deposit,
            debug_message: _debug_message,
            result,
            ..
        } = result;

        assert_eq!(u32::decode(&mut result.unwrap().data.as_ref()).unwrap(), 0);

        let weight: Weight = <() as WeightInfo>::transfer();

        assert!(weight.ref_time() < gas_consumed.ref_time());
        assert!(weight.proof_size() < gas_consumed.proof_size());
        assert_ok!(
            Contracts::bare_call(
                alice(),
                contract_addr.clone(),
                0,
                gas_required,
                None,
                call.encode(),
                DebugInfo::Skip,
                CollectEvents::Skip,
                Determinism::Enforced,
            )
            .result
        );
    });
}
