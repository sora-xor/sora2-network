#![cfg_attr(not(feature = "std"), no_std)]
use common::{AssetId32, Balance, PredefinedAssetId};
use ink::primitives::AccountId;
use scale::Encode;

// TODO: Add comments with description
#[derive(Encode)]
pub enum RuntimeCall {
    #[codec(index = 21)]
    Assets(AssetsCall),
}

#[derive(Encode)]
pub enum AssetsCall {
    #[codec(index = 1)]
    Transfer {
        asset_id: AssetId32<PredefinedAssetId>,
        to: AccountId,
        amount: Balance,
    },
}

#[ink::contract]
mod asset_contract {
    use crate::transfer_contract::{AssetsCall, RuntimeCall};
    use common::AssetId32;
    use scale::{Decode, Encode};

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
            to: AccountId,
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
