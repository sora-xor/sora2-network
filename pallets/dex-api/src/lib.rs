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
    type MockLiquiditySource2: LiquiditySource<
        Self::DEXId,
        Self::AccountId,
        Self::AssetId,
        Fixed,
        DispatchError,
    >;
    type MockLiquiditySource3: LiquiditySource<
        Self::DEXId,
        Self::AccountId,
        Self::AssetId,
        Fixed,
        DispatchError,
    >;
    type MockLiquiditySource4: LiquiditySource<
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
    type XYKPool: LiquiditySource<
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
        use LiquiditySourceType::*;
        macro_rules! can_exchange {
            ($source_type:ident) => {
                T::$source_type::can_exchange(
                    &liquidity_source_id.dex_id,
                    input_asset_id,
                    output_asset_id,
                )
            };
        }
        match liquidity_source_id.liquidity_source_index {
            XYKPool => can_exchange!(XYKPool),
            BondingCurvePool => can_exchange!(BondingCurvePool),
            MockPool => can_exchange!(MockLiquiditySource),
            MockPool2 => can_exchange!(MockLiquiditySource2),
            MockPool3 => can_exchange!(MockLiquiditySource3),
            MockPool4 => can_exchange!(MockLiquiditySource4),
        }
    }

    fn quote(
        liquidity_source_id: &LiquiditySourceId<T::DEXId, LiquiditySourceType>,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        swap_amount: SwapAmount<Fixed>,
    ) -> Result<SwapOutcome<Fixed>, DispatchError> {
        use LiquiditySourceType::*;
        macro_rules! quote {
            ($source_type:ident) => {
                T::$source_type::quote(
                    &liquidity_source_id.dex_id,
                    input_asset_id,
                    output_asset_id,
                    swap_amount.into(),
                )
                .map(Into::into)
            };
        }
        match liquidity_source_id.liquidity_source_index {
            LiquiditySourceType::XYKPool => quote!(XYKPool),
            BondingCurvePool => quote!(BondingCurvePool),
            MockPool => quote!(MockLiquiditySource),
            MockPool2 => quote!(MockLiquiditySource2),
            MockPool3 => quote!(MockLiquiditySource3),
            MockPool4 => quote!(MockLiquiditySource4),
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
        use LiquiditySourceType::*;
        macro_rules! exchange {
            ($source_type:ident) => {
                T::$source_type::exchange(
                    sender,
                    receiver,
                    &liquidity_source_id.dex_id,
                    input_asset_id,
                    output_asset_id,
                    swap_amount.into(),
                )
                .map(Into::into)
            };
        }
        match liquidity_source_id.liquidity_source_index {
            XYKPool => exchange!(XYKPool),
            BondingCurvePool => exchange!(BondingCurvePool),
            MockPool => exchange!(MockLiquiditySource),
            MockPool2 => exchange!(MockLiquiditySource2),
            MockPool3 => exchange!(MockLiquiditySource3),
            MockPool4 => exchange!(MockLiquiditySource4),
        }
    }
}

impl<T: Trait> Module<T> {
    pub fn get_supported_types() -> Vec<LiquiditySourceType> {
        [
            LiquiditySourceType::MockPool,
            LiquiditySourceType::MockPool2,
            LiquiditySourceType::MockPool3,
            LiquiditySourceType::MockPool4,
            LiquiditySourceType::XYKPool,
        ]
        .into()
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
        let supported_types = Self::get_supported_types();
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
