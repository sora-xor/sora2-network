#![cfg_attr(not(feature = "std"), no_std)]

use common::balance::Balance;
use common::{
    prelude::{SwapAmount, SwapOutcome},
    Fixed, LiquidityRegistry, LiquiditySource, LiquiditySourceFilter, LiquiditySourceId,
    LiquiditySourceType,
};
use frame_support::{
    decl_error, decl_event, decl_module, ensure, sp_runtime::DispatchError, StorageMap,
};
use sp_std::vec::Vec;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub trait Trait: common::Trait + dex_manager::Trait + trading_pair::Trait {
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
    type MockLiquiditySource: LiquiditySource<
        Self::DEXId,
        Self::AccountId,
        Self::AssetId,
        Fixed,
        DispatchError,
    >;
    type BondingCurvePool: LiquiditySource<
        Self::DEXId,
        Self::AccountId,
        Self::AssetId,
        Balance,
        DispatchError,
    >;
}

decl_event!(
    pub enum Event<T>
    where
        AccountId = <T as frame_system::Trait>::AccountId,
    {
        SomethingHappened(AccountId),
    }
);

decl_error! {
    pub enum Error for Module<T: Trait> {
        DEXIdDoesNotExist,
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        type Error = Error<T>;

        fn deposit_event() = default;

        // TODO: implement extrinsics
    }
}

impl<T: Trait>
    LiquiditySource<
        LiquiditySourceId<T::DEXId, LiquiditySourceType>,
        T::AccountId,
        T::AssetId,
        Fixed,
        DispatchError,
    > for Module<T>
{
    fn can_exchange(
        liquidity_source_id: &LiquiditySourceId<T::DEXId, LiquiditySourceType>,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
    ) -> bool {
        match liquidity_source_id.liquidity_source_index {
            LiquiditySourceType::XYKPool => unimplemented!(),
            LiquiditySourceType::BondingCurvePool => T::BondingCurvePool::can_exchange(
                &liquidity_source_id.dex_id,
                &input_asset_id,
                &output_asset_id,
            ),
            LiquiditySourceType::MockPool => T::MockLiquiditySource::can_exchange(
                &liquidity_source_id.dex_id,
                &input_asset_id,
                &output_asset_id,
            )
            .into(),
        }
    }

    fn quote(
        liquidity_source_id: &LiquiditySourceId<T::DEXId, LiquiditySourceType>,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        swap_amount: SwapAmount<Fixed>,
    ) -> Result<SwapOutcome<Fixed>, DispatchError> {
        match liquidity_source_id.liquidity_source_index {
            LiquiditySourceType::XYKPool => unimplemented!(),
            LiquiditySourceType::BondingCurvePool => T::BondingCurvePool::quote(
                &liquidity_source_id.dex_id,
                input_asset_id,
                output_asset_id,
                swap_amount.into(),
            )
            .map(Into::into),
            LiquiditySourceType::MockPool => T::MockLiquiditySource::quote(
                &liquidity_source_id.dex_id,
                input_asset_id,
                output_asset_id,
                swap_amount,
            ),
        }
    }

    fn exchange(
        sender: &T::AccountId,
        receiver: &T::AccountId,
        liquidity_source_id: &LiquiditySourceId<T::DEXId, LiquiditySourceType>,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        swap_amount: SwapAmount<Fixed>,
    ) -> Result<SwapOutcome<Fixed>, DispatchError> {
        match liquidity_source_id.liquidity_source_index {
            LiquiditySourceType::XYKPool => unimplemented!(),
            LiquiditySourceType::BondingCurvePool => T::BondingCurvePool::exchange(
                sender,
                receiver,
                &liquidity_source_id.dex_id,
                input_asset_id,
                output_asset_id,
                swap_amount.into(),
            )
            .map(Into::into),
            LiquiditySourceType::MockPool => T::MockLiquiditySource::exchange(
                sender,
                receiver,
                &liquidity_source_id.dex_id,
                input_asset_id,
                output_asset_id,
                swap_amount,
            ),
        }
    }
}

impl<T: Trait>
    LiquidityRegistry<T::DEXId, T::AccountId, T::AssetId, LiquiditySourceType, Fixed, DispatchError>
    for Module<T>
{
    fn list_liquidity_sources(
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        filter: LiquiditySourceFilter<T::DEXId, LiquiditySourceType>,
    ) -> Result<Vec<LiquiditySourceId<T::DEXId, LiquiditySourceType>>, DispatchError> {
        let supported_types = &[LiquiditySourceType::MockPool];
        ensure!(
            dex_manager::DEXInfos::<T>::contains_key(filter.dex_id),
            Error::<T>::DEXIdDoesNotExist
        );

        Ok(supported_types
            .iter()
            .filter_map(|source_type| {
                if filter.matches_index(*source_type)
                    && T::MockLiquiditySource::can_exchange(
                        &filter.dex_id,
                        input_asset_id,
                        output_asset_id,
                    )
                {
                    Some(LiquiditySourceId::new(
                        filter.dex_id.clone(),
                        source_type.clone(),
                    ))
                } else {
                    None
                }
            })
            .collect())
    }
}
