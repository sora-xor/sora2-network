#![cfg_attr(not(feature = "std"), no_std)]

use common::prelude::Balance;

use crate::operations::*;

pub type ExtraAccountIdOf<T> = <T as assets::Config>::ExtraAccountId;

pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

pub type AssetIdOf<T> = <T as assets::Config>::AssetId;

pub type TechAssetIdOf<T> = <T as technical::Config>::TechAssetId;

pub type TechAccountIdOf<T> = <T as technical::Config>::TechAccountId;

pub type DEXIdOf<T> = <T as common::Config>::DEXId;

pub type PolySwapActionStructOf<T> =
    PolySwapAction<AssetIdOf<T>, TechAssetIdOf<T>, Balance, AccountIdOf<T>, TechAccountIdOf<T>>;

pub type PairSwapActionOf<T> =
    PairSwapAction<AssetIdOf<T>, Balance, AccountIdOf<T>, TechAccountIdOf<T>>;

pub type WithdrawLiquidityActionOf<T> = WithdrawLiquidityAction<
    AssetIdOf<T>,
    TechAssetIdOf<T>,
    Balance,
    AccountIdOf<T>,
    TechAccountIdOf<T>,
>;

pub type DepositLiquidityActionOf<T> = DepositLiquidityAction<
    AssetIdOf<T>,
    TechAssetIdOf<T>,
    Balance,
    AccountIdOf<T>,
    TechAccountIdOf<T>,
>;

pub type DEXManager<T> = dex_manager::Module<T>;
