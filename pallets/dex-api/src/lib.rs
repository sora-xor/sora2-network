#![cfg_attr(not(feature = "std"), no_std)]

use common::{
    balance::Balance,
    prelude::{SwapAmount, SwapOutcome, SwapVariant},
    LiquidityRegistry, LiquiditySource, LiquiditySourceFilter, LiquiditySourceId,
    LiquiditySourceType,
};
use frame_support::{
    decl_event, decl_module, decl_storage, dispatch::DispatchResult, sp_runtime::DispatchError,
    weights::Weight,
};
use frame_system::ensure_signed;
use sp_std::vec::Vec;

mod weights;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub trait WeightInfo {
    fn swap() -> Weight;
}

type DEXManager<T> = dex_manager::Module<T>;

pub trait Trait: common::Trait + dex_manager::Trait + trading_pair::Trait {
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
    type MockLiquiditySource: LiquiditySource<
        Self::DEXId,
        Self::AccountId,
        Self::AssetId,
        Balance,
        DispatchError,
    >;
    type MockLiquiditySource2: LiquiditySource<
        Self::DEXId,
        Self::AccountId,
        Self::AssetId,
        Balance,
        DispatchError,
    >;
    type MockLiquiditySource3: LiquiditySource<
        Self::DEXId,
        Self::AccountId,
        Self::AssetId,
        Balance,
        DispatchError,
    >;
    type MockLiquiditySource4: LiquiditySource<
        Self::DEXId,
        Self::AccountId,
        Self::AssetId,
        Balance,
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

    /// Weight information for extrinsics in this pallet.
    type WeightInfo: WeightInfo;
}

decl_storage! {
    trait Store for Module<T: Trait> as DexApiModule {
        pub EnabledSourceTypes config(source_types): Vec<LiquiditySourceType>;
    }
}

decl_event!(
    pub enum Event<T>
    where
        AccountId = <T as frame_system::Trait>::AccountId,
        AssetId = <T as assets::Trait>::AssetId,
        DEXId = <T as common::Trait>::DEXId,
    {
        /// Exchange of tokens has been performed
        /// [Sender Account, Receiver Account, DEX Id, LiquiditySourceType, Input Asset Id, Output Asset Id, Input Amount, Output Amount, Fee Amount]
        DirectExchange(
            AccountId,
            AccountId,
            DEXId,
            LiquiditySourceType,
            AssetId,
            AssetId,
            Balance,
            Balance,
            Balance,
        ),
    }
);

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {

        fn deposit_event() = default;

        /// Perform swap with specified parameters. Gateway for invoking liquidity source exchanges.
        ///
        /// - `dex_id`: ID of the exchange.
        /// - `liquidity_source_type`: Type of liquidity source to perform swap on.
        /// - `input_asset_id`: ID of Asset to be deposited from sender account into pool reserves.
        /// - `output_asset_id`: ID of Asset t0 be withdrawn from pool reserves into receiver account.
        /// - `amount`: Either amount of desired input or output tokens, determined by `swap_variant` parameter.
        /// - `limit`: Either maximum input amount or minimum output amount tolerated for successful swap,
        ///            determined by `swap_variant` parameter.
        /// - `swap_variant`: Either 'WithDesiredInput' or 'WithDesiredOutput', indicates amounts purpose.
        /// - `receiver`: Optional value, indicates AccountId for swap receiver. If not set, default is `sender`.
        #[weight = <T as Trait>::WeightInfo::swap()]
        pub fn swap(
            origin,
            dex_id: T::DEXId,
            liquidity_source_type: LiquiditySourceType,
            input_asset_id: T::AssetId,
            output_asset_id: T::AssetId,
            amount: Balance,
            limit: Balance,
            swap_variant: SwapVariant,
            receiver: Option<T::AccountId>
        ) -> DispatchResult {
            let sender = ensure_signed(origin)?;
            let receiver = receiver.unwrap_or(sender.clone());
            let outcome = Self::exchange(
                &sender,
                &receiver,
                &LiquiditySourceId::<T::DEXId, LiquiditySourceType>::new(dex_id.clone(), liquidity_source_type.clone()),
                &input_asset_id,
                &output_asset_id,
                SwapAmount::with_variant(swap_variant, amount.clone(), limit.clone())
            )?;
            let (input_amount, output_amount) = match swap_variant {
                SwapVariant::WithDesiredInput => (amount, outcome.amount.clone()),
                SwapVariant::WithDesiredOutput => (outcome.amount.clone(), amount),
            };
            Self::deposit_event(
                RawEvent::DirectExchange(
                    sender,
                    receiver,
                    dex_id,
                    liquidity_source_type,
                    input_asset_id,
                    output_asset_id,
                    input_amount,
                    output_amount,
                    outcome.fee.clone()
                )
            );
            Ok(())
        }
    }
}

impl<T: Trait>
    LiquiditySource<
        LiquiditySourceId<T::DEXId, LiquiditySourceType>,
        T::AccountId,
        T::AssetId,
        Balance,
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
        swap_amount: SwapAmount<Balance>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
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
        swap_amount: SwapAmount<Balance>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
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
    /// List liquidity source types which are enabled on chain, this applies to all DEX'es.
    /// Used in aggregation pallets, such as liquidity-proxy.
    pub fn get_supported_types() -> Vec<LiquiditySourceType> {
        EnabledSourceTypes::get()
    }
}

impl<T: Trait>
    LiquidityRegistry<
        T::DEXId,
        T::AccountId,
        T::AssetId,
        LiquiditySourceType,
        Balance,
        DispatchError,
    > for Module<T>
{
    fn list_liquidity_sources(
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        filter: LiquiditySourceFilter<T::DEXId, LiquiditySourceType>,
    ) -> Result<Vec<LiquiditySourceId<T::DEXId, LiquiditySourceType>>, DispatchError> {
        let supported_types = Self::get_supported_types();
        DEXManager::<T>::ensure_dex_exists(&filter.dex_id)?;
        Ok(supported_types
            .iter()
            .filter_map(|source_type| {
                if filter.matches_index(*source_type)
                    && Self::can_exchange(
                        &LiquiditySourceId::new(filter.dex_id, *source_type),
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
