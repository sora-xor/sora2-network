#![cfg_attr(not(feature = "std"), no_std)]

use common::{
    prelude::{SwapAmount, SwapOutcome},
    Fixed, LiquidityRegistry, LiquiditySource, LiquiditySourceFilter, LiquiditySourceId,
    LiquiditySourceType,
};
use frame_support::{decl_error, decl_event, decl_module, sp_runtime::DispatchError};
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
        SomethingWrong,
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
            LiquiditySourceType::MockPool => T::MockLiquiditySource::can_exchange(
                &liquidity_source_id.dex_id,
                &input_asset_id,
                &output_asset_id,
            ),
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
        Ok(dex_manager::Module::<T>::list_dex_ids()
            .iter()
            .filter_map(|dex_id| {
                let source_id =
                    LiquiditySourceId::new(dex_id.clone(), LiquiditySourceType::MockPool);
                if filter.matches(&source_id)
                    && T::MockLiquiditySource::can_exchange(dex_id, input_asset_id, output_asset_id)
                {
                    Some(source_id)
                } else {
                    None
                }
            })
            .collect())
    }
}
