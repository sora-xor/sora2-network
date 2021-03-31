#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::dispatch::DispatchError;
use frame_support::ensure;

use common::balance;
use common::prelude::{Balance, FixedWrapper};

use crate::aliases::{AssetIdOf, TechAccountIdOf};
use crate::{to_balance, to_fixed_wrapper};

use crate::{Config, Error, Module};

impl<T: Config> Module<T> {
    #[inline]
    pub fn get_fee_for_source(
        _asset_id: &AssetIdOf<T>,
        _tech_acc: &TechAccountIdOf<T>,
        x_in: &Balance,
    ) -> Result<Balance, DispatchError> {
        let fxw_x_in = FixedWrapper::from(*x_in);
        //TODO: get this value from DEXInfo.
        let result =
            (fxw_x_in * FixedWrapper::from(balance!(3))) / FixedWrapper::from(balance!(1000));
        Ok(to_balance!(result))
    }

    #[inline]
    pub fn get_fee_for_destination(
        _asset_id: &AssetIdOf<T>,
        _tech_acc: &TechAccountIdOf<T>,
        y_out: &Balance,
    ) -> Result<Balance, DispatchError> {
        let fxw_y_out = FixedWrapper::from(*y_out);
        //TODO: get this value from DEXInfo.
        let result =
            (fxw_y_out * FixedWrapper::from(balance!(3))) / FixedWrapper::from(balance!(1000));
        Ok(to_balance!(result))
    }

    pub fn calculate_optimal_deposit(
        _total_supply: Balance,
        reserve_a: Balance,
        reserve_b: Balance,
        amount_a_desired: Balance,
        amount_b_desired: Balance,
        amount_a_min: Balance,
        amount_b_min: Balance,
    ) -> Result<(Balance, Balance), DispatchError> {
        let opt_am_a_des = to_balance!(
            to_fixed_wrapper!(amount_b_desired)
                / (to_fixed_wrapper!(reserve_b) / to_fixed_wrapper!(reserve_a))
        );
        let opt_am_b_des = to_balance!(
            to_fixed_wrapper!(amount_a_desired)
                / (to_fixed_wrapper!(reserve_a) / to_fixed_wrapper!(reserve_b))
        );

        if opt_am_b_des <= amount_b_desired {
            ensure!(
                opt_am_b_des >= amount_b_min,
                Error::<T>::ImpossibleToDecideValidPairValuesFromRangeForThisPool
            );
            Ok((amount_a_desired, opt_am_b_des))
        } else {
            ensure!(
                opt_am_a_des >= amount_a_min && opt_am_a_des <= amount_a_desired,
                Error::<T>::ImpossibleToDecideValidPairValuesFromRangeForThisPool
            );
            Ok((opt_am_a_des, amount_b_desired))
        }
    }

    pub fn calc_deposit_liquidity_1(
        total_supply: Balance,
        reserve_a: Balance,
        reserve_b: Balance,
        amount_a_desired: Balance,
        amount_b_desired: Balance,
        amount_a_min: Balance,
        amount_b_min: Balance,
    ) -> Result<(Balance, Balance, Balance), DispatchError> {
        let (am_a_des, am_b_des) = Module::<T>::calculate_optimal_deposit(
            total_supply,
            reserve_a,
            reserve_b,
            amount_a_desired,
            amount_b_desired,
            amount_a_min,
            amount_b_min,
        )?;
        let lhs = to_balance!(
            to_fixed_wrapper!(am_a_des)
                / (to_fixed_wrapper!(reserve_a) / to_fixed_wrapper!(total_supply))
        );
        let rhs = to_balance!(
            to_fixed_wrapper!(am_b_des)
                / (to_fixed_wrapper!(reserve_b) / to_fixed_wrapper!(total_supply))
        );
        let min_value = lhs.min(rhs);
        Ok((am_a_des, am_b_des, min_value))
    }

    /// Calulate (y_output,fee) pair where fee can be fee_of_y1 or fee_of_x_in, and output is
    /// without fee.
    pub fn calc_output_for_exact_input(
        asset_a: &AssetIdOf<T>,
        asset_b: &AssetIdOf<T>,
        tech_acc: &TechAccountIdOf<T>,
        get_fee_from_destination: bool,
        x: &Balance,
        y: &Balance,
        x_in: &Balance,
    ) -> Result<(Balance, Balance), DispatchError> {
        let fxw_x = FixedWrapper::from(x.clone());
        let fxw_y = FixedWrapper::from(y.clone());
        let fxw_x_in = FixedWrapper::from(x_in.clone());
        if get_fee_from_destination {
            Module::<T>::guard_fee_from_destination(asset_a, asset_b)?;
            //let fxw_y1 = (fxw_x_in.clone() * fxw_y) / (fxw_x + fxw_x_in);
            let fxw_y1 = fxw_x_in.clone() / ((fxw_x + fxw_x_in) / fxw_y);
            let y1 = to_balance!(fxw_y1);
            let fee_of_y1 = Module::<T>::get_fee_for_destination(asset_a, tech_acc, &y1)?;
            Ok((y1, fee_of_y1))
        } else {
            Module::<T>::guard_fee_from_source(asset_a, asset_b)?;
            let fee_of_x_in = Module::<T>::get_fee_for_source(asset_a, tech_acc, x_in)?;
            let fxw_fee_of_x_in = FixedWrapper::from(fee_of_x_in);
            let fxw_x_in_subfee = fxw_x_in - fxw_fee_of_x_in;
            //TODO: this comments exist now for comparation of multiplication version, please remove it
            //than precision problems will finally set to best solution.
            //let fxw_y_out = (fxw_x_in_subfee.clone() * fxw_y) / (fxw_x + fxw_x_in_subfee);
            let fxw_y_out = fxw_x_in_subfee.clone() / ((fxw_x + fxw_x_in_subfee) / fxw_y);
            let y_out = to_balance!(fxw_y_out);
            Ok((y_out, fee_of_x_in))
        }
    }

    /// Calulate (x_input,fee) pair where fee can be fee_of_y1 or fee_of_x_in, and input is
    /// without fee.
    pub fn calc_input_for_exact_output(
        asset_a: &AssetIdOf<T>,
        asset_b: &AssetIdOf<T>,
        tech_acc: &TechAccountIdOf<T>,
        get_fee_from_destination: bool,
        x: &Balance,
        y: &Balance,
        y_out: &Balance,
    ) -> Result<(Balance, Balance), DispatchError> {
        let fxw_x = FixedWrapper::from(x.clone());
        let fxw_y = FixedWrapper::from(y.clone());
        let fxw_y_out = FixedWrapper::from(y_out.clone());
        if get_fee_from_destination {
            Module::<T>::guard_fee_from_destination(asset_a, asset_b)?;
            let unit = balance!(1);
            let fract_a: Balance = Module::<T>::get_fee_for_destination(asset_a, tech_acc, &unit)?;
            let fract_b: Balance = unit - fract_a;
            let fxw_fract_b = FixedWrapper::from(fract_b);
            let fxw_y1 = fxw_y_out.clone() / fxw_fract_b;
            //let fxw_x_in = (fxw_x * fxw_y1.clone()) / (fxw_y - fxw_y1.clone());
            let fxw_x_in = fxw_x / ((fxw_y - fxw_y1.clone()) / fxw_y1.clone());
            let fxw_fee = fxw_y1 - fxw_y_out;
            let x_in = to_balance!(fxw_x_in);
            let fee = to_balance!(fxw_fee);
            Ok((x_in, fee))
        } else {
            Module::<T>::guard_fee_from_source(asset_a, asset_b)?;
            let y_minus_y_out = *y - *y_out;
            let ymyo_fee = Module::<T>::get_fee_for_source(asset_a, tech_acc, &y_minus_y_out)?;
            let ymyo_subfee = y_minus_y_out - ymyo_fee;
            let fxw_ymyo_subfee = FixedWrapper::from(ymyo_subfee);
            //TODO: this comments exist now for comparation of multiplication version, please remove it
            //than precision problems will finally set to best solution.
            //let fxw_x_in = (fxw_x * fxw_y_out) / fxw_ymyo_subfee;
            let fxw_x_in = fxw_x / (fxw_ymyo_subfee / fxw_y_out);
            let x_in = to_balance!(fxw_x_in);
            let fee = Module::<T>::get_fee_for_source(asset_a, tech_acc, &x_in)?;
            Ok((x_in, fee))
        }
    }

    pub fn get_xor_part_from_pool_account(
        pool_acc: T::AccountId,
        liq_amount: Balance,
    ) -> Result<Balance, DispatchError> {
        let tech_acc = technical::Module::<T>::lookup_tech_account_id(&pool_acc)?;
        let trading_pair = match tech_acc.into() {
            common::TechAccountId::Pure(_, common::TechPurpose::LiquidityKeeper(trading_pair)) => {
                trading_pair
            }
            _ => {
                return Err(Error::<T>::UnableToGetXORPartFromMarkerAsset.into());
            }
        };
        let b_in_pool =
            assets::Module::<T>::free_balance(&trading_pair.base_asset_id.into(), &pool_acc)?;
        let t_in_pool =
            assets::Module::<T>::free_balance(&trading_pair.target_asset_id.into(), &pool_acc)?;
        let fxw_liq_in_pool =
            to_fixed_wrapper!(b_in_pool).multiply_and_sqrt(&to_fixed_wrapper!(t_in_pool));
        let fxw_piece = fxw_liq_in_pool / to_fixed_wrapper!(liq_amount);
        let fxw_value = to_fixed_wrapper!(b_in_pool) / fxw_piece;
        let value = to_balance!(fxw_value);
        Ok(value)
    }
}
