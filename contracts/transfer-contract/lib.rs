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

#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
mod transfer_contract {
    use contract_extrinsics::{RuntimeCall, assets::AssetsCall, primitives::AssetId32};
    use ink::prelude::string::String;
    use sp_runtime::AccountId32;
    use ink::prelude::format;

    #[ink(storage)]
    #[derive(Default)]
    pub struct AssetContract;

    #[ink::scale_derive(Encode, Decode, TypeInfo)]
    #[derive(Debug, PartialEq, Eq)]
    pub enum RuntimeError {
        CallRuntimeFailed,
    }

    impl AssetContract {
        #[ink(constructor)]
        pub fn new() -> Self {
            Default::default()
        }

        #[ink(message)]
        pub fn transfer(
            &self,
            asset_id: AssetId32,
            to: AccountId32,
            amount: Balance,
        ) -> Result<(), RuntimeError> {
            self.env()
                .call_runtime(&RuntimeCall::Assets(AssetsCall::Transfer {
                    asset_id,
                    to,
                    amount,
                }))
                .map_err(|_| RuntimeError::CallRuntimeFailed)
        }
    }

    #[cfg(all(test, feature = "e2e-tests"))]
    mod e2e_tests {
        use super::*;
        use ink_e2e::{
            ChainBackend,
            ContractsBackend,
        };

        use ink::{
            env::{
                test::default_accounts,
                DefaultEnvironment,
            },
        };
        use sp_runtime::AccountId32;
        use contract_extrinsics::primitives::AssetId32;

        type E2EResult<T> = Result<T, Box<dyn std::error::Error>>;

        /// The base number of indivisible units for balances on the
        /// `substrate-contracts-node`.
        const UNIT: Balance = 1_000_000_000_000;

        /// The contract will be given 1000 tokens during instantiation.
        const CONTRACT_BALANCE: Balance = 1_000 * UNIT;

        /// The receiver will get enough funds to have the required existential deposit.
        ///
        /// If your chain has this threshold higher, increase the transfer value.
        const TRANSFER_VALUE: Balance = 1 / 10 * UNIT;

        /// An amount that is below the existential deposit, so that a transfer to an
        /// empty account fails.
        ///
        /// Must not be zero, because such an operation would be a successful no-op.
        const INSUFFICIENT_TRANSFER_VALUE: Balance = 1;

        pub fn alice() -> AccountId32 {
            AccountId32::from([1; 32])
        }

        pub fn bob() -> AccountId32 {
            AccountId32::from([2; 32])
        }

        /// Positive case scenario:
        ///  - the call is valid
        ///  - the call execution succeeds
        #[ink_e2e::test]
        async fn transfer_with_call_runtime_works<Client: E2EBackend>(
            mut client: Client,
        ) -> E2EResult<()> {
            // given
            let mut constructor = AssetContractRef::new();
            let contract = client
                .instantiate("transfer-contract", &ink_e2e::alice(), &mut constructor)
                .value(CONTRACT_BALANCE)
                .submit()
                .await
                .expect("instantiate failed");
            let mut call_builder = contract.call_builder::<AssetContract>();

            let receiver = bob();
            let mut asset_id = AssetId32::new([0u8; 32]);

            let contract_balance_before = client
                .free_balance(contract.account_id)
                .await
                .expect("Failed to get account balance");
            let receiver_balance_before = client
                .free_balance(default_accounts::<DefaultEnvironment>().bob)
                .await
                .expect("Failed to get account balance");

            // when
            let transfer_message =
                call_builder.transfer(asset_id, receiver.clone(), TRANSFER_VALUE);

            let call_res = client
                .call(&ink_e2e::alice(), &transfer_message)
                .submit()
                .await
                .expect("call failed");

            assert!(call_res.return_value().is_ok());

            // then
            let contract_balance_after = client
                .free_balance(contract.account_id)
                .await
                .expect("Failed to get account balance");
            let receiver_balance_after = client
                .free_balance(default_accounts::<DefaultEnvironment>().bob)
                .await
                .expect("Failed to get account balance");

            assert_eq!(
                contract_balance_before,
                contract_balance_after + TRANSFER_VALUE
            );
            assert_eq!(
                receiver_balance_before,
                receiver_balance_after - TRANSFER_VALUE
            );

            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {}
