#![cfg_attr(not(feature = "std"), no_std)]

use common::prelude::*;
pub use domain::*;
use frame_support::{
    codec::{Decode, Encode},
    decl_error, decl_event, decl_module, decl_storage,
    dispatch::{DispatchError, DispatchResult},
    ensure,
    weights::Weight,
    RuntimeDebug,
};
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

use common::GetLstIdAndTradingPairFromTechAsset;
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
const SMOOTH_PERIOD: u32 = 100;

type AccountIdOf<T> = <T as frame_system::Trait>::AccountId;
type TechAccountIdOf<T> = <T as technical::Trait>::TechAccountId;
//type AssetIdOf<T> = <T as assets::Trait>::AssetId;
type TechAssetIdOf<T> = <T as technical::Trait>::TechAssetId;
type BlockNumberOf<T> = <T as frame_system::Trait>::BlockNumber;
type FarmerOf<T> = Farmer<AccountIdOf<T>, TechAccountIdOf<T>, BlockNumberOf<T>>;

//#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]

#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DiscoverClaim<AmountType> {
    pub units_per_blocks: AmountType,
    pub available_origin: AmountType,
    pub available_claim: AmountType,
}

pub trait Trait:
    frame_system::Trait
    // + timestamp::Trait
    + permissions::Trait
    + technical::Trait
    // + sp_std::fmt::Debug
    + pool_xyk::Trait
{
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;

    /// Weight information for extrinsics in this pallet.
    type WeightInfo: WeightInfo;
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

decl_storage! {
    trait Store for Module<T: Trait> as FarmsStoreModule
    {
        pub NextFarmId get(fn next_farm_id): FarmId;

        pub Farms:
              map
                hasher(identity) FarmId
                  => Option<Farm<T::AccountId, T::AssetId, T::BlockNumber>>;

        pub Farmers:
              double_map
                hasher(identity) FarmId,
                hasher(blake2_128_concat) T::AccountId
                  => Option<Farmer<T::AccountId, TechAccountIdOf<T>, T::BlockNumber>>;

        pub PricesStates:
              double_map
                hasher(blake2_128_concat) AssetId32<common::AssetId>,
                hasher(blake2_128_concat) AssetId32<common::AssetId>
                  => Option<SmoothPriceState>;

        /// Collection of all registered marker tokens for farmer.
        pub MarkerTokensIndex get(fn marker_token_index): map hasher(blake2_128_concat) (FarmId, T::AccountId) => BTreeSet<T::AssetId>;

    }
    add_extra_genesis {
        config(initial_farm): (T::AccountId, T::AssetId, T::AssetId);
        build(|config: &GenesisConfig<T>| {
            let tup = config.initial_farm.clone();
            Module::<T>::create_unchecked(tup.0, tup.1, tup.2).expect("Failed to register farm.");
        })
    }
}

decl_event!(
    pub enum Event<T>
    where
        AccountId = <T as frame_system::Trait>::AccountId,
        AssetId = AssetId32<common::AssetId>,
    {
        FarmCreated(FarmId, AccountId),
        FarmerCreated(FarmId, AccountId),
        IncentiveClaimed(FarmId, AccountId),
        FarmerExit(FarmId, AccountId),
        SmoothPriceUpdated(AssetId, AssetId, Balance),
    }
);

decl_error! {
    pub enum Error for Module<T: Trait> {
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
    }
}

impl<T: Trait> Module<T> {
    pub fn create_unchecked(
        who: T::AccountId,
        origin_asset_id: T::AssetId,
        claim_asset_id: T::AssetId,
    ) -> Result<Option<FarmId>, DispatchError> {
        permissions::Module::<T>::check_permission(who.clone(), permissions::CREATE_FARM)?;
        let farm_id = NextFarmId::get();
        let current_block = <frame_system::Module<T>>::block_number();
        let farming_state = FarmingState::<Balance, T::BlockNumber> {
            units_per_blocks: 0u32.into(),
            last_change: current_block,
            units_locked: 0u32.into(),
        };
        let incentive_model = IncentiveModel::<T::AssetId, Balance, T::BlockNumber> {
            suitable_for_block: current_block,
            origin_asset_id,
            claim_asset_id,
            amount_of_origin: Some(99999u32.into()),
            origin_to_claim_ratio: Some(1u32.into()),
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

        Self::deposit_event(RawEvent::FarmCreated(farm_id, who));
        NextFarmId::set(farm_id + 1);
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
                technical::Module::<T>::register_tech_account_id_if_not_exist(&tech_id)?;
                let current_block = <frame_system::Module<T>>::block_number();
                let farmer = FarmerOf::<T> {
                    id: farmer_id,
                    tech_account_id: tech_id,
                    state: FarmingState::<Balance, T::BlockNumber> {
                        units_per_blocks: 0u32.into(),
                        last_change: current_block,
                        units_locked: 0u32.into(),
                    },
                };
                Farmers::<T>::insert(farm_id, account_id.clone(), farmer.clone());
                Self::deposit_event(RawEvent::FarmerCreated(farm_id, account_id));
                Ok(farmer)
            }
        }
    }

    fn get_xor_part_amount_from_marker(
        dex_id: T::DEXId,
        asset_id: T::AssetId,
        amount: Balance,
    ) -> Result<Balance, DispatchError> {
        let tech_asset: TechAssetIdOf<T> = asset_id
            .try_into()
            .map_err(|_| Error::<T>::UnableToConvertAssetIdToTechAssetId)?;
        let (lst, tp) = tech_asset
            .get_lst_id_and_trading_pair_from_tech_asset()
            .ok_or(Error::<T>::UnableToGetPoolInformationFromTechAsset)?;
        let xor_part = match lst {
            common::LiquiditySourceType::XYKPool => {
                pool_xyk::Module::<T>::get_xor_part_from_trading_pair(dex_id, tp, amount)?
            }
            _ => Err(Error::<T>::ThisTypeOfLiquiditySourceIsNotImplementedOrSupported)?,
        };
        Ok(xor_part)
    }

    pub fn lock_to_farm_unchecked(
        who: T::AccountId,
        dex_id: T::DEXId,
        farm_id: FarmId,
        asset_id: T::AssetId,
        amount: Balance,
    ) -> DispatchResult {
        permissions::Module::<T>::check_permission(who.clone(), permissions::LOCK_TO_FARM)?;
        let xor_part = Module::<T>::get_xor_part_amount_from_marker(dex_id, asset_id, amount)?;
        let mut farm = Farms::<T>::get(&farm_id).ok_or(Error::<T>::FarmNotFound)?;
        let current_block = <frame_system::Module<T>>::block_number();
        let mut farmer = Self::get_or_create_farmer(who.clone(), farm_id)?;
        farmer
            .state
            .put_to_locked(Some(&mut farm.aggregated_state), current_block, xor_part)
            .map_err(|()| Error::<T>::CalculationOrOperationWithFarmingStateIsFailed)?;
        // Technical account for farmer is unique, so this is lock.
        technical::Module::<T>::transfer_in(&asset_id, &who, &farmer.tech_account_id, amount)?;
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
        permissions::Module::<T>::check_permission(who.clone(), permissions::UNLOCK_FROM_FARM)?;
        let mut farm = Farms::<T>::get(&farm_id).ok_or(Error::<T>::FarmNotFound)?;
        let current_block = <frame_system::Module<T>>::block_number();
        let mut farmer = Self::get_or_create_farmer(who.clone(), farm_id)?;
        let ta_repr =
            technical::Module::<T>::tech_account_id_to_account_id(&farmer.tech_account_id)?;
        let amount_opt = match (amount_opt, opt_asset_id) {
            (_, Some(asset_id)) => {
                let amount = match amount_opt {
                    Some(amount) => amount,
                    None => {
                        MarkerTokensIndex::<T>::mutate((farm_id.clone(), who.clone()), |mti| {
                            mti.remove(&asset_id)
                        });
                        <assets::Module<T>>::free_balance(&asset_id, &ta_repr)?
                    }
                };
                let xor_part =
                    Module::<T>::get_xor_part_amount_from_marker(dex_id, asset_id, amount)?;
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
                    let amount = <assets::Module<T>>::free_balance(&asset_id, &ta_repr)?;
                    // Asset is None so unlock all assets, this is like exiting from farm.
                    technical::Module::<T>::transfer_out(
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
            technical::Module::<T>::transfer_out(
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
        permissions::Module::<T>::check_permission(who.clone(), permissions::CLAIM_FROM_FARM)?;
        let mut farm = Farms::<T>::get(&farm_id).ok_or(Error::<T>::FarmNotFound)?;
        let mut farmer = Self::get_or_create_farmer(who.clone(), farm_id)?;
        let current_block = <frame_system::Module<T>>::block_number();
        farm.aggregated_state
            .recalculate(current_block)
            .map_err(|()| Error::<T>::CalculationOrOperationWithFarmingStateIsFailed)?;
        farmer
            .state
            .recalculate(current_block)
            .map_err(|()| Error::<T>::CalculationOrOperationWithFarmingStateIsFailed)?;
        let total_upb = farm.aggregated_state.units_per_blocks;
        let mut upb = farmer.state.units_per_blocks;
        ensure!(upb > 0u32.into(), Error::<T>::NothingToClaim);
        let peace = total_upb / upb;
        let amount_of_origin = farm
            .incentive_model_state
            .amount_of_origin
            .ok_or(Error::<T>::SomeValuesIsNotSet)?;

        if farm.incentive_model_state.suitable_for_block < current_block {
            //TODO: Now it is limited for xor pswap, that about other assets ?
            farm.incentive_model_state.origin_to_claim_ratio =
                Module::<T>::get_smooth_price_for_xor_pswap();
        }
        let origin_to_claim_ratio = farm
            .incentive_model_state
            .origin_to_claim_ratio
            .ok_or(Error::<T>::SomeValuesIsNotSet)?;

        let mut peace_of_origin = amount_of_origin / peace;
        let mut peace_of_claim = peace_of_origin * origin_to_claim_ratio;

        match amount_opt {
            None => (),
            Some(amount) => {
                ensure!(
                    amount <= peace_of_claim,
                    Error::<T>::AmountIsOutOfAvailableValue
                );
                let down = peace_of_claim / amount;
                upb /= down;
                peace_of_origin /= down;
                peace_of_claim /= down;
            }
        }

        if perform_write_to_database {
            farmer
                .state
                .remove_from_upb(Some(&mut farm.aggregated_state), current_block, upb)
                .map_err(|()| Error::<T>::CalculationOrOperationWithFarmingStateIsFailed)?;
            farm.incentive_model_state.amount_of_origin = Some(amount_of_origin - peace_of_origin);
            T::Currency::deposit(
                farm.incentive_model_state.claim_asset_id,
                &who,
                peace_of_claim,
            )?;
            Farms::<T>::insert(farm.id, farm.clone());
            Farmers::<T>::insert(farmer.id.0.clone(), farmer.id.1.clone(), farmer);
            farm.incentive_model_state.suitable_for_block = current_block;
            Self::deposit_event(RawEvent::IncentiveClaimed(farm_id, who));
        }

        Ok(DiscoverClaim::<Balance> {
            units_per_blocks: upb,
            available_origin: peace_of_origin,
            available_claim: peace_of_claim,
        })
    }

    pub fn get_farm_info(
        who: T::AccountId,
        farm_id: FarmId,
    ) -> Result<Option<FarmInfo<T::AccountId, T::AssetId, T::BlockNumber>>, Error<T>> {
        permissions::Module::<T>::check_permission(who.clone(), permissions::GET_FARM_INFO)
            .map_err(|_| Error::<T>::NotEnoughPermissions)?;
        let farm = Farms::<T>::get(farm_id).ok_or_else(|| Error::<T>::FarmNotFound)?;
        let current_block = <frame_system::Module<T>>::block_number();
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
        permissions::Module::<T>::check_permission(who.clone(), permissions::GET_FARMER_INFO)
            .map_err(|_| Error::<T>::NotEnoughPermissions)?;
        let farmer = Farmers::<T>::get(farm_id, who).ok_or_else(|| Error::<T>::FarmNotFound)?;
        let current_block = <frame_system::Module<T>>::block_number();
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
        let opt_value = PricesStates::get(XOR, PSWAP).map(|v| v.smooth_price);
        let current_block = <frame_system::Module<T>>::block_number();
        if opt_value.is_none() {
            Module::<T>::update_xor_pswap_smooth_price(current_block);
            PricesStates::get(XOR, PSWAP).map(|v| v.smooth_price)
        } else {
            opt_value
        }
    }

    fn update_xor_pswap_smooth_price(now: T::BlockNumber) {
        let result: Result<u32, _> = (now / UPDATE_PRICES_EVERY_N_BLOCK.into()).try_into();
        let index: u32 = match result {
            Ok(v) => v.try_into().unwrap(),
            _ => unreachable!(),
        };
        let pv_cur = get_demo_price(index);
        let pv_state = match PricesStates::get(XOR, PSWAP) {
            Some(v) => v,
            None => SmoothPriceState {
                smooth_price: pv_cur.0.clone(),
                weavg_normal: (pv_cur.0.clone(), pv_cur.1.clone()),
                weavg_short: (pv_cur.0.clone(), pv_cur.1.clone()),
            },
        };

        // Prepearing constants.
        let one: Balance = 1u32.into();
        let two: Balance = 2u32.into();
        let smooth: Balance = SMOOTH_PERIOD.into();
        let smooth_short: Balance = smooth / two.clone();

        // Getting quick variables for calculations.
        let p1 = pv_state.weavg_normal.0;
        let v1 = pv_state.weavg_normal.1;
        let p2 = pv_state.weavg_short.0;
        let v2 = pv_state.weavg_short.1;
        let pc = pv_cur.0;
        let vc = pv_cur.1;

        // Calculations for first weavg curve.
        let voldiv1: Balance = one.clone() + one.clone() / smooth;
        let ps1 = pc * vc / smooth;
        let vs1 = v1 + vc / smooth;
        let p_res1 = (p1 * v1 + ps1) / vs1;
        let v_res1 = vs1 / voldiv1;

        // Calculations for second weavg curve (shorter period).
        let voldiv2: Balance = one.clone() + one.clone() / smooth_short;
        let ps2 = pc * vc / smooth_short;
        let vs2 = v2 + vc / smooth_short;
        let p_res2 = (p2 * v2 + ps2) / vs2;
        let v_res2 = vs2 / voldiv2;

        // Compute smooth price as first half of normal distribution,
        // approximated by two weavg curves.
        let smooth_price = (p_res1 - p_res2 / two.clone()) * two.clone();

        // Updating smooth price state for this asset pair.
        let pv_state_update = SmoothPriceState {
            smooth_price: smooth_price,
            weavg_normal: (p_res1, v_res1),
            weavg_short: (p_res2, v_res2),
        };
        PricesStates::insert(XOR, PSWAP, pv_state_update);
        Self::deposit_event(RawEvent::SmoothPriceUpdated(XOR, PSWAP, smooth_price));

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
            Module::<T>::update_xor_pswap_smooth_price(now);
        }
        0u32.into()
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin
    {
        type Error = Error<T>;
        fn deposit_event() = default;

        fn on_initialize(now: T::BlockNumber) -> Weight {
            Module::<T>::perform_per_block_update(now)
        }

        #[weight = 0]
        pub fn create(origin, origin_asset_id: T::AssetId, claim_asset_id: T::AssetId) -> Result<Option<FarmId>, DispatchError> {
            let who = ensure_signed(origin)?;
            Module::<T>::create_unchecked(who, origin_asset_id, claim_asset_id)
        }

        #[weight = 0]
        pub fn lock_to_farm(origin, dex_id: T::DEXId, farm_id: FarmId, asset_id: T::AssetId, amount: Balance) -> DispatchResult
        {
            let who = ensure_signed(origin)?;
            Module::<T>::lock_to_farm_unchecked(who, dex_id, farm_id, asset_id, amount)
        }

        #[weight = 0]
        pub fn unlock_from_farm(origin, dex_id: T::DEXId, farm_id: FarmId,
                                opt_asset_id: Option<T::AssetId>, amount_opt: Option<Balance>) -> DispatchResult
        {
            let who = ensure_signed(origin)?;
            common::with_transaction(|| {
                Module::<T>::unlock_from_farm_unchecked(who, dex_id, farm_id, opt_asset_id, amount_opt)
            })
        }

        #[weight = 0]
        pub fn claim(origin, farm_id: FarmId, amount_opt: Option<Balance>) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Module::<T>::prepare_and_optional_claim(who, farm_id, amount_opt, true)?;
            Ok(())
        }

        #[weight = 0]
        fn discover_claim(origin, farm_id: FarmId) -> Result<Option<u64>, DispatchError> {
            let who = ensure_signed(origin)?;
            let discover = Module::<T>::prepare_and_optional_claim(who, farm_id, None, false)?;
            Ok(Some(discover.available_claim.into()))
        }
    }
}
