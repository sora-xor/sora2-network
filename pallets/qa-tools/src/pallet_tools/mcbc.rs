use crate::pallet_tools::price_tools::CalculatedXorPrices;
use crate::Config;
use crate::{pallet_tools, Error};
use codec::{Decode, Encode};
use common::prelude::FixedWrapper;
use common::{AssetInfoProvider, Balance, PriceVariant};
use frame_support::dispatch::DispatchResult;
use frame_support::ensure;
use frame_support::traits::Get;

#[derive(Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, Debug)]
pub struct ReferencePriceInput {
    pub buy: Balance,
    pub sell: Balance,
}

/// Input for initializing collateral assets except TBCD.
#[derive(Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, Debug)]
pub struct OtherCollateralInput<AssetId> {
    /// Collateral asset id
    pub asset: AssetId,
    /// Price of collateral in terms of reference asset. Linearly affects the exchange amounts.
    /// (if collateral costs 10x more sell output should be 10x smaller)
    pub ref_prices: Option<ReferencePriceInput>,
    /// Desired amount of collateral asset in the MCBC reserve account. Affects actual sell
    /// price according to formulae.
    pub reserves: Option<Balance>,
}

/// Input for initializing TBCD collateral.
#[derive(Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, Debug)]
pub struct TbcdCollateralInput<AssetId> {
    pub regular_collateral_input: OtherCollateralInput<AssetId>,
    pub xor_ref_prices: ReferencePriceInput,
}

pub struct BaseSupply<AccountId> {
    pub base_supply_collector: AccountId,
    pub new_base_supply: Balance,
}

pub(crate) fn initialize_single_collateral<T: Config>(
    input: OtherCollateralInput<T::AssetId>,
) -> DispatchResult {
    let reference_asset = multicollateral_bonding_curve_pool::ReferenceAssetId::<T>::get();
    if let Some(ref_prices) = input.ref_prices {
        let CalculatedXorPrices {
            asset_a: collateral_xor_prices,
            asset_b: _,
        } = pallet_tools::price_tools::calculate_xor_prices::<T>(
            &input.asset,
            &reference_asset,
            ref_prices.buy,
            ref_prices.sell,
        )?;

        ensure!(
            collateral_xor_prices.buy >= collateral_xor_prices.sell,
            Error::<T>::BuyLessThanSell
        );
        pallet_tools::price_tools::set_price::<T>(
            &input.asset,
            collateral_xor_prices.buy,
            PriceVariant::Buy,
        )?;
        pallet_tools::price_tools::set_price::<T>(
            &input.asset,
            collateral_xor_prices.sell,
            PriceVariant::Sell,
        )?;
    }

    // initialize reserves

    // let base_asset = T::GetBaseAssetId::get();
    // let reference_asset = multicollateral_bonding_curve_pool::Pallet::<T>::reference_asset_id();
    // let total_issuance = assets::Pallet::<T>::total_issuance(&base_asset)?;
    // todo: register TP if not exist
    // TradingPair::register(
    //     RuntimeOrigin::signed(alice()),
    //     DEXId::Polkaswap.into(),
    //     XOR,
    //     VAL,
    // )
    // .expect("Failed to register trading pair.");
    // TradingPair::register(
    //     RuntimeOrigin::signed(alice()),
    //     DEXId::Polkaswap.into(),
    //     XOR,
    //     XSTUSD,
    // )
    // .expect("Failed to register trading pair.");

    // todo: initialize pool if not already
    // MBCPool::initialize_pool_unchecked(VAL, false).expect("Failed to initialize pool.");

    // todo: register account if not present???
    // let bonding_curve_tech_account_id = TechAccountId::Pure(
    //     DEXId::Polkaswap,
    //     TechPurpose::Identifier(b"bonding_curve_tech_account_id".to_vec()),
    // );
    // Technical::register_tech_account_id(bonding_curve_tech_account_id.clone())?;
    // MBCPool::set_reserves_account_id(bonding_curve_tech_account_id.clone())?;

    // todo: use traits where possible (not only here, in whole pallet)
    // let reserve_amount_expected = FixedWrapper::from(total_issuance)
    //     * multicollateral_bonding_curve_pool::Pallet::<T>::sell_function(
    //         &base_asset,
    //         &input.asset,
    //         Fixed::ZERO,
    //     )?;

    // let pool_reference_amount = reserve_amount_expected * ratio;
    // let pool_reference_amount = pool_reference_amount
    //     .try_into_balance()
    //     .map_err(|_| Error::<T>::ArithmeticError)?;
    // let pool_val_amount = <T as Config>::LiquidityProxy::quote(
    //     DEXId::Polkaswap.into(),
    //     &reference_asset,
    //     &input.asset,
    //     QuoteAmount::with_desired_input(pool_reference_amount),
    //     LiquiditySourceFilter::empty(DEXId::Polkaswap.into()),
    //     true,
    // )?;

    // let reserves_account =
    //     multicollateral_bonding_curve_pool::Pallet::<T>::reserves_account_id();
    // technical::Pallet::<T>::mint(&input.asset, &reserves_account, pool_val_amount.amount)?;

    Ok(())
}

pub(crate) fn initialize_tbcd_collateral<T: Config>(
    input: TbcdCollateralInput<T::AssetId>,
) -> DispatchResult {
    // handle xor ref price
    // input.xor_ref_prices

    initialize_single_collateral::<T>(input.regular_collateral_input)
}

pub(crate) fn initialize_base_supply<T: Config>(input: BaseSupply<T::AccountId>) -> DispatchResult {
    let base_asset_id = &T::GetBaseAssetId::get();
    let current_base_supply: FixedWrapper =
        assets::Pallet::<T>::total_issuance(base_asset_id)?.into();
    let supply_delta = input.new_base_supply - current_base_supply;
    let supply_delta = supply_delta
        .get()
        .map_err(|_| Error::<T>::ArithmeticError)?
        .into_bits();

    // realistically the error should never be triggered
    let owner =
        assets::Pallet::<T>::asset_owner(&base_asset_id).ok_or(Error::<T>::UnknownMCBCAsset)?;
    if supply_delta > 0 {
        let mint_amount = supply_delta
            .try_into()
            .map_err(|_| Error::<T>::ArithmeticError)?;
        assets::Pallet::<T>::mint_to(
            base_asset_id,
            &owner,
            &input.base_supply_collector,
            mint_amount,
        )?;
    } else if supply_delta < 0 {
        let burn_amount = supply_delta
            .abs()
            .try_into()
            .map_err(|_| Error::<T>::ArithmeticError)?;
        assets::Pallet::<T>::burn_from(
            base_asset_id,
            &owner,
            &input.base_supply_collector,
            burn_amount,
        )?;
    }
    Ok(())
}
