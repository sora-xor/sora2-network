#![cfg_attr(not(feature = "std"), no_std)]

#[ink::contract]
mod asset_contract {
    use common::AssetId32;
    use contract_extrinsics::assets::AssetsCall;
    use contract_extrinsics::RuntimeCall;
    use scale::{Decode, Encode};
    use sp_runtime::AccountId32;

    #[ink(storage)]
    #[derive(Default)]
    pub struct AssetContract;

    #[derive(Debug, PartialEq, Eq, Encode, Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
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
            asset_id: [u8; 32],
            to: AccountId32,
            amount: Balance,
        ) -> Result<(), RuntimeError> {
            self.env()
                .call_runtime(&RuntimeCall::Assets(AssetsCall::Transfer {
                    asset_id: AssetId32::from_bytes(asset_id),
                    to,
                    amount,
                }))
                .map_err(|_| RuntimeError::CallRuntimeFailed)
        }
    }
}

#[cfg(test)]
mod tests {
    use ink::env::DefaultEnvironment;

    fn default_accounts() -> ink::env::test::DefaultAccounts<DefaultEnvironment> {
        ink::env::test::default_accounts::<DefaultEnvironment>()
    }
}
