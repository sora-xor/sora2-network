#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::unnecessary_mut_passed)]

use codec::Codec;
use sp_std::prelude::*;

sp_api::decl_runtime_apis! {
    pub trait DEXManagerAPI<DEXId> where
        DEXId: Codec,
    {
        fn list_dex_ids() -> Vec<DEXId>;
    }
}
