// This file is part of the SORA network and Polkaswap app.

// Copyright (c) 2020, 2021, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

// Redistribution and use in source and binary forms, with or without modification,
// are permitted provided that the following conditions are met:

// Redistributions of source code must retain the above copyright notice, this list
// of conditions and the following disclaimer.
// Redistributions in binary form must reproduce the above copyright notice, this
// list of conditions and the following disclaimer in the documentation and/or other
// materials provided with the distribution.
//
// All advertising materials mentioning features or use of this software must display
// the following acknowledgement: This product includes software developed by Polka Biome
// Ltd., SORA, and Polkaswap.
//
// Neither the name of the Polka Biome Ltd. nor the names of its contributors may be used
// to endorse or promote products derived from this software without specific prior written permission.

// THIS SOFTWARE IS PROVIDED BY Polka Biome Ltd. AS IS AND ANY EXPRESS OR IMPLIED WARRANTIES,
// INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL Polka Biome Ltd. BE LIABLE FOR ANY
// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
// BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS;
// OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
// USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

#![cfg_attr(not(feature = "std"), no_std)]

use common::prelude::FixedWrapper;
use common::{balance, Balance, FromGenericPair, PSWAP, XOR};
pub use domain::*;
use frame_support::codec::{Decode, Encode};
use frame_support::dispatch::{DispatchError, DispatchResult};
use frame_support::weights::Weight;
use frame_support::{ensure, RuntimeDebug};
use frame_system::ensure_signed;
use orml_traits::currency::MultiCurrency;
use sp_std::collections::btree_set::BTreeSet;

pub trait WeightInfo {
    fn create() -> Weight;
    fn lock_to_farm() -> Weight;
    fn unlock_from_farm() -> Weight;
}

impl WeightInfo for () {
    fn create() -> Weight {
        100_000_000
    }
    fn lock_to_farm() -> Weight {
        100_000_000
    }
    fn unlock_from_farm() -> Weight {
        100_000_000
    }
}

use sp_std::convert::TryInto;

//use serde::{Deserialize, Serialize};

/*
 * This is for debug output to log, and checking graph in gnuplot and comparing,
 * maybe this will be needed.
use sp_runtime::print;
 */

mod demo_price;
use demo_price::*;

pub mod domain;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

// For smooth price testing change this value to 1.
// After testing change this values from 1 to 1000.
const UPDATE_PRICES_EVERY_N_BLOCK: u32 = 1000;

/// Period 100 = 1 week, if interval is 1000 block where one block each 6 seconds.
const SMOOTH_PERIOD: u128 = 100;

type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
type TechAccountIdOf<T> = <T as technical::Config>::TechAccountId;
type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;
type FarmerOf<T> = Farmer<AccountIdOf<T>, TechAccountIdOf<T>, BlockNumberOf<T>>;

//#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]

#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DiscoverClaim<AmountType> {
    pub units_per_blocks: AmountType,
    pub available_origin: AmountType,
    pub available_claim: AmountType,
}

type Pair<T> = (T, T);

/// Structure used in calculation of smooth price, two weighted exponential average curves used to
/// approximate one half of normal distribution, for smoother price calculations.
#[derive(Clone, RuntimeDebug, Eq, PartialEq, Encode, Decode)]
pub struct SmoothPriceState {
    smooth_price: Balance,
    weavg_normal: Pair<Balance>,
    weavg_short: Pair<Balance>,
}

impl<T: Config> Pallet<T> {
    pub fn create_unchecked(
        who: T::AccountId,
        origin_asset_id: T::AssetId,
        claim_asset_id: T::AssetId,
    ) -> Result<Option<FarmId>, DispatchError> {
        permissions::Pallet::<T>::check_permission(who.clone(), permissions::CREATE_FARM)?;
        let farm_id = NextFarmId::<T>::get();
        let current_block = <frame_system::Pallet<T>>::block_number();
        let farming_state = FarmingState::<Balance, T::BlockNumber> {
            units_per_blocks: 0,
            last_change: current_block,
            units_locked: 0,
        };
        let incentive_model = IncentiveModel::<T::AssetId, Balance, T::BlockNumber> {
            suitable_for_block: current_block,
            origin_asset_id,
            claim_asset_id,
            amount_of_origin: Some(balance!(99999)),
            origin_to_claim_ratio: Some(balance!(1)),
        };
        let farm = Farm::<T::AccountId, T::AssetId, T::BlockNumber> {
            id: farm_id,
            owner_id: who.clone(),
            creation_block_number: current_block,
            aggregated_state: farming_state,
            incentive_model_state: incentive_model,
        };

        let _amount_of_origin = farm
            .incentive_model_state
            .amount_of_origin
            .ok_or(Error::<T>::SomeValuesIsNotSet)?;

        Farms::<T>::insert(farm_id, farm);

        Self::deposit_event(Event::FarmCreated(farm_id, who));
        NextFarmId::<T>::set(farm_id + 1);
        Ok(Some(farm_id))
    }

    pub fn get_or_create_farmer(
        account_id: T::AccountId,
        farm_id: FarmId,
    ) -> Result<FarmerOf<T>, DispatchError> {
        let farmer_id = (farm_id, account_id.clone());
        match Farmers::<T>::get(farm_id, account_id.clone()) {
            Some(farmer) => Ok(farmer),
            None => {
                let tech_id = T::TechAccountId::from_generic_pair(
                    "FARMING_PALLET".into(),
                    farmer_id.encode(),
                );
                frame_system::Pallet::<T>::inc_consumers(&account_id)
                    .map_err(|_| Error::<T>::IncRefError)?;
                technical::Pallet::<T>::register_tech_account_id_if_not_exist(&tech_id)?;
                let current_block = <frame_system::Pallet<T>>::block_number();
                let farmer = FarmerOf::<T> {
                    id: farmer_id,
                    tech_account_id: tech_id,
                    state: FarmingState::<Balance, T::BlockNumber> {
                        units_per_blocks: 0,
                        last_change: current_block,
                        units_locked: 0,
                    },
                };
                Farmers::<T>::insert(farm_id, account_id.clone(), farmer.clone());
                Self::deposit_event(Event::FarmerCreated(farm_id, account_id));
                Ok(farmer)
            }
        }
    }

    fn get_xor_part_amount_from_marker(
        _dex_id: T::DEXId,
        asset_id: T::AssetId,
        amount: Balance,
    ) -> Result<Balance, DispatchError> {
        use assets::AssetRecord::*;
        use assets::AssetRecordArg::*;
        use common::AssetIdExtraAssetRecordArg::*;
        let tuple = assets::Module::<T>::tuple_from_asset_id(&asset_id)
            .ok_or(Error::<T>::UnableToGetPoolInformationFromTechAsset)?;
        match tuple {
            Arity3(GenericU128(tag), Extra(lst_extra), Extra(acc_extra)) => {
                ensure!(
                    tag == common::hash_to_u128_pair(b"Marking asset").0,
                    Error::<T>::UnableToGetPoolInformationFromTechAsset
                );
                ensure!(
                    lst_extra == LstId(common::LiquiditySourceType::XYKPool.into()).into(),
                    Error::<T>::ThisTypeOfLiquiditySourceIsNotImplementedOrSupported
                );
                match acc_extra.into() {
                    AccountId(extra_acc) => {
                        let acc: AccountIdOf<T> = extra_acc.into();
                        pool_xyk::Module::<T>::get_xor_part_from_pool_account(acc, amount)
                    }
                    _ => {
                        return Err(Error::<T>::UnableToGetPoolInformationFromTechAsset.into());
                    }
                }
            }
            _ => {
                return Err(Error::<T>::UnableToGetPoolInformationFromTechAsset.into());
            }
        }
    }

    pub fn lock_to_farm_unchecked(
        who: T::AccountId,
        dex_id: T::DEXId,
        farm_id: FarmId,
        asset_id: T::AssetId,
        amount: Balance,
    ) -> DispatchResult {
        permissions::Pallet::<T>::check_permission(who.clone(), permissions::LOCK_TO_FARM)?;
        let xor_part = Pallet::<T>::get_xor_part_amount_from_marker(dex_id, asset_id, amount)?;
        let mut farm = Farms::<T>::get(&farm_id).ok_or(Error::<T>::FarmNotFound)?;
        let current_block = <frame_system::Pallet<T>>::block_number();
        let mut farmer = Self::get_or_create_farmer(who.clone(), farm_id)?;
        farmer
            .state
            .put_to_locked(Some(&mut farm.aggregated_state), current_block, xor_part)
            .map_err(|()| Error::<T>::CalculationOrOperationWithFarmingStateIsFailed)?;
        // Technical account for farmer is unique, so this is lock.
        technical::Pallet::<T>::transfer_in(&asset_id, &who, &farmer.tech_account_id, amount)?;
        // If previous operation is fail than transfer is not done, and next code is not performed,
        // and this code is about writeing to storage map.
        Farms::<T>::insert(farm.id, farm);
        Farmers::<T>::insert(farmer.id.0.clone(), farmer.id.1.clone(), farmer);
        MarkerTokensIndex::<T>::mutate((farm_id, who), |mti| mti.insert(asset_id));
        Ok(())
    }

    pub fn unlock_from_farm_unchecked(
        who: T::AccountId,
        dex_id: T::DEXId,
        farm_id: FarmId,
        opt_asset_id: Option<T::AssetId>,
        amount_opt: Option<Balance>,
    ) -> DispatchResult {
        permissions::Pallet::<T>::check_permission(who.clone(), permissions::UNLOCK_FROM_FARM)?;
        let mut farm = Farms::<T>::get(&farm_id).ok_or(Error::<T>::FarmNotFound)?;
        let current_block = <frame_system::Pallet<T>>::block_number();
        let mut farmer = Self::get_or_create_farmer(who.clone(), farm_id)?;
        let ta_repr =
            technical::Pallet::<T>::tech_account_id_to_account_id(&farmer.tech_account_id)?;
        let amount_opt = match (amount_opt, opt_asset_id) {
            (_, Some(asset_id)) => {
                let amount = match amount_opt {
                    Some(amount) => amount,
                    None => {
                        MarkerTokensIndex::<T>::mutate((farm_id.clone(), who.clone()), |mti| {
                            mti.remove(&asset_id)
                        });
                        <assets::Pallet<T>>::free_balance(&asset_id, &ta_repr)?
                    }
                };
                let xor_part =
                    Pallet::<T>::get_xor_part_amount_from_marker(dex_id, asset_id, amount)?;
                farmer
                    .state
                    .remove_from_locked(Some(&mut farm.aggregated_state), current_block, xor_part)
                    .map_err(|()| Error::<T>::CalculationOrOperationWithFarmingStateIsFailed)?;
                Some(amount)
            }
            (None, None) => {
                farmer
                    .state
                    .remove_all_from_locked(Some(&mut farm.aggregated_state), current_block)
                    .map_err(|()| Error::<T>::CalculationOrOperationWithFarmingStateIsFailed)?;
                let mti = MarkerTokensIndex::<T>::get((farm_id, who.clone()));
                for asset_id in mti {
                    let amount = <assets::Pallet<T>>::free_balance(&asset_id, &ta_repr)?;
                    // Asset is None so unlock all assets, this is like exiting from farm.
                    technical::Pallet::<T>::transfer_out(
                        &asset_id,
                        &farmer.tech_account_id,
                        &who,
                        amount,
                    )?;
                }
                let empty: BTreeSet<T::AssetId> = BTreeSet::new();
                MarkerTokensIndex::<T>::insert((farm_id, who.clone()), empty);
                None
            }
            _ => {
                return Err(Error::<T>::CaseIsNotSupported.into());
            }
        };
        if let Some(amount) = amount_opt {
            // Technical account for farmer is unique, so this is unlock.
            technical::Pallet::<T>::transfer_out(
                &opt_asset_id.unwrap(),
                &farmer.tech_account_id,
                &who,
                amount,
            )?;
        }
        // If previous operation is fail than transfer is not done, and next code is not performed,
        // and this code is about writeing to storage map.
        Farms::<T>::insert(farm.id, farm);
        Farmers::<T>::insert(farmer.id.0.clone(), farmer.id.1.clone(), farmer);
        Ok(())
    }

    pub fn prepare_and_optional_claim(
        who: T::AccountId,
        farm_id: FarmId,
        amount_opt: Option<Balance>,
        perform_write_to_database: bool,
    ) -> Result<DiscoverClaim<Balance>, DispatchError> {
        permissions::Pallet::<T>::check_permission(who.clone(), permissions::CLAIM_FROM_FARM)?;
        let mut farm = Farms::<T>::get(&farm_id).ok_or(Error::<T>::FarmNotFound)?;
        let mut farmer = Self::get_or_create_farmer(who.clone(), farm_id)?;
        let current_block = <frame_system::Pallet<T>>::block_number();
        farm.aggregated_state
            .recalculate(current_block)
            .map_err(|()| Error::<T>::CalculationOrOperationWithFarmingStateIsFailed)?;
        farmer
            .state
            .recalculate(current_block)
            .map_err(|()| Error::<T>::CalculationOrOperationWithFarmingStateIsFailed)?;
        let total_upb = FixedWrapper::from(farm.aggregated_state.units_per_blocks);
        let mut upb = FixedWrapper::from(farmer.state.units_per_blocks);
        ensure!(upb > FixedWrapper::from(0), Error::<T>::NothingToClaim);
        let piece = total_upb / upb.clone();
        let amount_of_origin = FixedWrapper::from(
            farm.incentive_model_state
                .amount_of_origin
                .ok_or(Error::<T>::SomeValuesIsNotSet)?,
        );

        if farm.incentive_model_state.suitable_for_block < current_block {
            //TODO: Now it is limited for xor pswap, that about other assets ?
            farm.incentive_model_state.origin_to_claim_ratio =
                Pallet::<T>::get_smooth_price_for_xor_pswap();
        }
        let origin_to_claim_ratio = FixedWrapper::from(
            farm.incentive_model_state
                .origin_to_claim_ratio
                .ok_or(Error::<T>::SomeValuesIsNotSet)?,
        );

        let mut piece_of_origin = amount_of_origin.clone() / piece;
        let mut piece_of_claim = piece_of_origin.clone() * origin_to_claim_ratio.clone();

        match amount_opt {
            None => (),
            Some(amount) => {
                let amount = FixedWrapper::from(amount);
                ensure!(
                    amount <= piece_of_claim,
                    Error::<T>::AmountIsOutOfAvailableValue
                );
                let down = piece_of_claim.clone() / amount;
                upb = upb / down.clone();
                piece_of_origin = piece_of_origin / down.clone();
                piece_of_claim = piece_of_claim / down.clone();
            }
        }

        let upb = upb.into_balance();
        let amount_of_origin = amount_of_origin.into_balance();
        let piece_of_origin = piece_of_origin.into_balance();
        let piece_of_claim = piece_of_claim.into_balance();

        if perform_write_to_database {
            farmer
                .state
                .remove_from_upb(Some(&mut farm.aggregated_state), current_block, upb)
                .map_err(|()| Error::<T>::CalculationOrOperationWithFarmingStateIsFailed)?;
            farm.incentive_model_state.amount_of_origin = Some(amount_of_origin - piece_of_origin);
            T::Currency::deposit(
                farm.incentive_model_state.claim_asset_id,
                &who,
                piece_of_claim,
            )?;
            Farms::<T>::insert(farm.id, farm.clone());
            Farmers::<T>::insert(farmer.id.0.clone(), farmer.id.1.clone(), farmer);
            farm.incentive_model_state.suitable_for_block = current_block;
            Self::deposit_event(Event::IncentiveClaimed(farm_id, who));
        }

        Ok(DiscoverClaim::<Balance> {
            units_per_blocks: upb,
            available_origin: piece_of_origin,
            available_claim: piece_of_claim,
        })
    }

    pub fn get_farm_info(
        who: T::AccountId,
        farm_id: FarmId,
    ) -> Result<Option<FarmInfo<T::AccountId, T::AssetId, T::BlockNumber>>, Error<T>> {
        permissions::Pallet::<T>::check_permission(who.clone(), permissions::GET_FARM_INFO)
            .map_err(|_| Error::<T>::NotEnoughPermissions)?;
        let farm = Farms::<T>::get(farm_id).ok_or_else(|| Error::<T>::FarmNotFound)?;
        let current_block = <frame_system::Pallet<T>>::block_number();
        let mut farm_now = farm.clone();
        farm_now
            .aggregated_state
            .recalculate(current_block)
            .map_err(|()| Error::<T>::CalculationOrOperationWithFarmingStateIsFailed)?;
        Ok(Some(FarmInfo {
            farm: farm.clone(),
            total_upbu_now: farm_now.aggregated_state.units_per_blocks,
        }))
    }

    pub fn get_farmer_info(
        who: T::AccountId,
        farm_id: FarmId,
    ) -> Result<Option<FarmerInfo<T::AccountId, TechAccountIdOf<T>, T::BlockNumber>>, Error<T>>
    {
        permissions::Pallet::<T>::check_permission(who.clone(), permissions::GET_FARMER_INFO)
            .map_err(|_| Error::<T>::NotEnoughPermissions)?;
        let farmer = Farmers::<T>::get(farm_id, who).ok_or_else(|| Error::<T>::FarmNotFound)?;
        let current_block = <frame_system::Pallet<T>>::block_number();
        let mut farmer_now = farmer.clone();
        farmer_now
            .state
            .recalculate(current_block)
            .map_err(|()| Error::<T>::CalculationOrOperationWithFarmingStateIsFailed)?;
        Ok(Some(FarmerInfo {
            farmer: farmer.clone(),
            upbu_now: farmer_now.state.units_per_blocks,
        }))
    }

    pub fn get_smooth_price_for_xor_pswap() -> Option<Balance> {
        let opt_value = PricesStates::<T>::get(XOR, PSWAP).map(|v| v.smooth_price);
        let current_block = <frame_system::Pallet<T>>::block_number();
        if opt_value.is_none() {
            Pallet::<T>::update_xor_pswap_smooth_price(current_block);
            PricesStates::<T>::get(XOR, PSWAP).map(|v| v.smooth_price)
        } else {
            opt_value
        }
    }

    fn update_xor_pswap_smooth_price(now: T::BlockNumber) {
        let result = now / UPDATE_PRICES_EVERY_N_BLOCK.into();
        let result = <T::BlockNumber as TryInto<u32>>::try_into(result);
        let index: u32 = match result {
            Ok(v) => v.try_into().unwrap(),
            _ => unreachable!(),
        };
        let pv_cur = get_demo_price(index);
        let pv_state = match PricesStates::<T>::get(XOR, PSWAP) {
            Some(v) => v,
            None => SmoothPriceState {
                smooth_price: pv_cur.0.clone(),
                weavg_normal: (pv_cur.0.clone(), pv_cur.1.clone()),
                weavg_short: (pv_cur.0.clone(), pv_cur.1.clone()),
            },
        };

        // Prepearing constants.
        let one: FixedWrapper = FixedWrapper::from(balance!(1));
        let two: FixedWrapper = FixedWrapper::from(balance!(2));
        let smooth: FixedWrapper = FixedWrapper::from(SMOOTH_PERIOD * balance!(1));
        let smooth_short = smooth.clone() / two.clone();

        // Getting quick variables for calculations.
        let p1 = FixedWrapper::from(pv_state.weavg_normal.0);
        let v1 = FixedWrapper::from(pv_state.weavg_normal.1);
        let p2 = FixedWrapper::from(pv_state.weavg_short.0);
        let v2 = FixedWrapper::from(pv_state.weavg_short.1);
        let pc = FixedWrapper::from(pv_cur.0);
        let vc = FixedWrapper::from(pv_cur.1);

        // Calculations for first weavg curve.
        let voldiv1 = one.clone() + one.clone() / smooth.clone();
        let ps1 = pc.clone() * vc.clone() / smooth.clone();
        let vs1 = v1.clone() + vc.clone() / smooth.clone();
        let p_res1 = (p1 * v1.clone() + ps1) / vs1.clone();
        let v_res1 = vs1 / voldiv1;

        // Calculations for second weavg curve (shorter period).
        let voldiv2 = one.clone() + one.clone() / smooth_short.clone();
        let ps2 = pc * vc.clone() / smooth_short.clone();
        let vs2 = v2.clone() + vc / smooth_short;
        let p_res2 = (p2 * v2 + ps2) / vs2.clone();
        let v_res2 = vs2 / voldiv2;

        // Compute smooth price as first half of normal distribution,
        // approximated by two weavg curves.
        let smooth_price = (p_res1.clone() - p_res2.clone() / two.clone()) * two.clone();
        let smooth_price = smooth_price.into_balance();

        // Updating smooth price state for this asset pair.
        let pv_state_update = SmoothPriceState {
            smooth_price: smooth_price,
            weavg_normal: (p_res1.into_balance(), v_res1.into_balance()),
            weavg_short: (p_res2.into_balance(), v_res2.into_balance()),
        };
        PricesStates::<T>::insert(XOR, PSWAP, pv_state_update);
        Self::deposit_event(Event::<T>::SmoothPriceUpdated(XOR, PSWAP, smooth_price));

        /*
         * This is for debug output to log, and checking graph in gnuplot and comparing,
         * maybe this will be needed.
        let ww: Balance = 100000u32.into();
        let msg: u32 = (smooth_price * ww).into();
        let msg2: u32 = (pc * ww).into();
        print("====START====");
        print(msg);
        print(msg2);
        print("====END====");
        */
    }

    pub fn perform_per_block_update(now: T::BlockNumber) -> Weight {
        if now % UPDATE_PRICES_EVERY_N_BLOCK.into() == 0u32.into() {
            Pallet::<T>::update_xor_pswap_smooth_price(now);
        }
        0u32.into()
    }

    // This function is used only in tests, that's why the compiler considers it to be unused
    #[cfg(test)]
    fn discover_claim(origin: T::Origin, farm_id: FarmId) -> Result<Balance, DispatchError> {
        let who = ensure_signed(origin)?;
        let discover = Pallet::<T>::prepare_and_optional_claim(who, farm_id, None, false)?;
        Ok(discover.available_claim)
    }
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use common::AssetId32;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::config]
    pub trait Config:
        frame_system::Config + permissions::Config + technical::Config + pool_xyk::Config
    {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(now: T::BlockNumber) -> Weight {
            Pallet::<T>::perform_per_block_update(now)
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(0)]
        pub fn create(
            origin: OriginFor<T>,
            origin_asset_id: T::AssetId,
            claim_asset_id: T::AssetId,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            Pallet::<T>::create_unchecked(who, origin_asset_id, claim_asset_id)?;
            Ok(().into())
        }

        #[pallet::weight(0)]
        pub fn lock_to_farm(
            origin: OriginFor<T>,
            dex_id: T::DEXId,
            farm_id: FarmId,
            asset_id: T::AssetId,
            amount: Balance,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            Pallet::<T>::lock_to_farm_unchecked(who, dex_id, farm_id, asset_id, amount)?;
            Ok(().into())
        }

        #[pallet::weight(0)]
        pub fn unlock_from_farm(
            origin: OriginFor<T>,
            dex_id: T::DEXId,
            farm_id: FarmId,
            opt_asset_id: Option<T::AssetId>,
            amount_opt: Option<Balance>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            common::with_transaction(|| {
                Pallet::<T>::unlock_from_farm_unchecked(
                    who,
                    dex_id,
                    farm_id,
                    opt_asset_id,
                    amount_opt,
                )?;
                Ok(().into())
            })
        }

        #[pallet::weight(0)]
        pub fn claim(
            origin: OriginFor<T>,
            farm_id: FarmId,
            amount_opt: Option<Balance>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            Pallet::<T>::prepare_and_optional_claim(who, farm_id, amount_opt, true)?;
            Ok(().into())
        }
    }

    #[pallet::event]
    #[pallet::metadata(AccountIdOf<T> = "AccountId", AssetId32<common::PredefinedAssetId> = "AssetId")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        FarmCreated(FarmId, AccountIdOf<T>),
        FarmerCreated(FarmId, AccountIdOf<T>),
        IncentiveClaimed(FarmId, AccountIdOf<T>),
        FarmerExit(FarmId, AccountIdOf<T>),
        SmoothPriceUpdated(
            AssetId32<common::PredefinedAssetId>,
            AssetId32<common::PredefinedAssetId>,
            Balance,
        ),
    }

    #[pallet::error]
    pub enum Error<T> {
        NotEnoughPermissions,
        FarmNotFound,
        FarmerNotFound,
        ShareNotFound,
        TechAccountIsMissing,
        FarmAlreadyClosed,
        FarmLocked,
        CalculationFailed,
        CalculationOrOperationWithFarmingStateIsFailed,
        SomeValuesIsNotSet,
        AmountIsOutOfAvailableValue,
        UnableToConvertAssetIdToTechAssetId,
        UnableToGetPoolInformationFromTechAsset,
        ThisTypeOfLiquiditySourceIsNotImplementedOrSupported,
        NothingToClaim,
        CaseIsNotSupported,
        /// Increment account reference error.
        IncRefError,
    }

    #[pallet::storage]
    #[pallet::getter(fn next_farm_id)]
    pub type NextFarmId<T: Config> = StorageValue<_, FarmId, ValueQuery>;

    #[pallet::storage]
    pub type Farms<T: Config> =
        StorageMap<_, Identity, FarmId, Farm<T::AccountId, T::AssetId, T::BlockNumber>>;

    #[pallet::storage]
    pub type Farmers<T: Config> = StorageDoubleMap<
        _,
        Identity,
        FarmId,
        Blake2_128Concat,
        T::AccountId,
        Farmer<T::AccountId, TechAccountIdOf<T>, T::BlockNumber>,
    >;

    #[pallet::storage]
    pub type PricesStates<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        AssetId32<common::PredefinedAssetId>,
        Blake2_128Concat,
        AssetId32<common::PredefinedAssetId>,
        SmoothPriceState,
    >;

    /// Collection of all registered marker tokens for farmer.
    #[pallet::storage]
    #[pallet::getter(fn marker_token_index)]
    pub type MarkerTokensIndex<T: Config> =
        StorageMap<_, Blake2_128Concat, (FarmId, T::AccountId), BTreeSet<T::AssetId>, ValueQuery>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub initial_farm: (T::AccountId, T::AssetId, T::AssetId),
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                initial_farm: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            let tup = self.initial_farm.clone();
            Pallet::<T>::create_unchecked(tup.0, tup.1, tup.2).expect("Failed to register farm.");
        }
    }
}
