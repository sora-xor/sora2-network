#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::unnecessary_mut_passed)]

use codec::Codec;
use sp_std::prelude::*;

sp_api::decl_runtime_apis! {
    pub trait FarmingRuntimeApi<AccountId, FarmName, FarmInfo, FarmerInfo> where
        AccountId: Codec,
        FarmName: Codec,
        FarmInfo: Codec,
        FarmerInfo: Codec,
    {
        fn get_farm_info(who: AccountId, name: FarmName) -> Option<FarmInfo>;

        fn get_farmer_info(who: AccountId, name: FarmName) -> Option<FarmerInfo>;
    }
}
