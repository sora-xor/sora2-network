use crate::{Config, Error};
use assets::AssetIdOf;
use codec::{Decode, Encode};
use common::prelude::BalanceUnit;
use common::{
    balance, AssetInfoProvider, Balance, DEXInfo, DexIdOf, DexInfoProvider, TradingPair,
    TradingPairSourceManager, XOR,
};
use frame_support::dispatch::{DispatchError, RawOrigin};
use sp_arithmetic::traits::CheckedMul;
use sp_std::fmt::Debug;
use sp_std::vec::Vec;

#[derive(Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, Debug)]
pub struct AssetPairInput<DEXId, AssetId> {
    pub dex_id: DEXId,
    pub asset_a: AssetId,
    pub asset_b: AssetId,
    /// Price of `asset_a` in terms of `asset_b` (how much `asset_b` is needed to buy 1 `asset_a`)
    pub price: Balance,
}

impl<DEXId, AssetId> AssetPairInput<DEXId, AssetId> {
    // `price` - Price of `asset_a` in terms of `asset_b` (how much `asset_b` is needed to buy 1
    // `asset_a`)
    pub fn new(dex_id: DEXId, asset_a: AssetId, asset_b: AssetId, price: Balance) -> Self {
        Self {
            dex_id,
            asset_a,
            asset_b,
            price,
        }
    }
}

/// `None` if neither of the assets is base
fn trading_pair_from_asset_ids<T: Config>(
    dex_info: DEXInfo<AssetIdOf<T>>,
    asset_a: AssetIdOf<T>,
    asset_b: AssetIdOf<T>,
) -> Option<TradingPair<AssetIdOf<T>>> {
    if asset_a == dex_info.base_asset_id {
        Some(TradingPair {
            base_asset_id: asset_a,
            target_asset_id: asset_b,
        })
    } else if asset_b == dex_info.base_asset_id {
        Some(TradingPair {
            base_asset_id: asset_b,
            target_asset_id: asset_a,
        })
    } else {
        None
    }
}

/// Initialize xyk liquidity source for multiple asset pairs at once.
///
/// ## Return
///
/// Due to limited precision of fixed-point numbers, the requested price might not be precisely
/// obtainable. Therefore, actual resulting price is returned.
///
/// Note: with current implementation the prices should always be equal
pub fn initialize<T: Config + pool_xyk::Config>(
    caller: T::AccountId,
    pairs: Vec<AssetPairInput<DexIdOf<T>, AssetIdOf<T>>>,
) -> Result<Vec<AssetPairInput<DexIdOf<T>, AssetIdOf<T>>>, DispatchError> {
    let mut actual_prices = pairs.clone();
    for (
        AssetPairInput {
            dex_id,
            asset_a,
            asset_b,
            price: expected_price,
        },
        AssetPairInput {
            price: actual_price,
            ..
        },
    ) in pairs.into_iter().zip(actual_prices.iter_mut())
    {
        if <T as Config>::AssetInfoProvider::is_non_divisible(&asset_a)
            || <T as Config>::AssetInfoProvider::is_non_divisible(&asset_b)
        {
            return Err(Error::<T>::AssetsMustBeDivisible.into());
        }

        let dex_info = <T as Config>::DexInfoProvider::get_dex_info(&dex_id)?;
        let trading_pair = trading_pair_from_asset_ids::<T>(dex_info, asset_a, asset_b)
            .ok_or(pool_xyk::Error::<T>::BaseAssetIsNotMatchedWithAnyAssetArguments)?;

        if !<T as Config>::TradingPairSourceManager::is_trading_pair_enabled(
            &dex_id,
            &trading_pair.base_asset_id,
            &trading_pair.target_asset_id,
        )? {
            <T as Config>::TradingPairSourceManager::register_pair(
                dex_id,
                trading_pair.base_asset_id,
                trading_pair.target_asset_id,
            )?
        }

        pool_xyk::Pallet::<T>::initialize_pool(
            RawOrigin::Signed(caller.clone()).into(),
            dex_id,
            asset_a,
            asset_b,
        )
        .map_err(|e| e.error)?;

        // Some magic numbers taken from existing init code
        // https://github.com/soramitsu/sora2-api-tests/blob/f590995abbd3b191a57b988ba3c10607a89d6f89/tests/testAccount/mintTokensForPairs.test.ts#L136
        let value_a: BalanceUnit = if asset_a == XOR.into() {
            balance!(1000000).into()
        } else {
            balance!(10000).into()
        };
        let price = BalanceUnit::divisible(expected_price);
        let value_b = value_a
            .checked_mul(&price)
            .ok_or(Error::<T>::ArithmeticError)?;

        assets::Pallet::<T>::mint_unchecked(&asset_a, &caller, *value_a.balance())?;
        assets::Pallet::<T>::mint_unchecked(&asset_b, &caller, *value_b.balance())?;

        *actual_price = *(value_b / value_a).balance();
        pool_xyk::Pallet::<T>::deposit_liquidity(
            RawOrigin::Signed(caller.clone()).into(),
            dex_id,
            asset_a,
            asset_b,
            *value_a.balance(),
            *value_b.balance(),
            // no need for range when the pool is empty
            *value_a.balance(),
            *value_b.balance(),
        )
        .map_err(|e| e.error)?;
    }
    Ok(actual_prices)
}
