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

use frame_support::dispatch::{DispatchError, DispatchResult};
use frame_support::traits::Get;
use frame_support::weights::Weight;
use frame_support::{ensure, Parameter};
use frame_system::ensure_signed;
use sp_std::collections::btree_set::BTreeSet;
use sp_std::vec::Vec;

use common::prelude::{Balance, EnsureDEXManager, Fixed, SwapAmount, SwapOutcome};
use common::{
    balance, hash, AssetName, AssetSymbol, EnsureTradingPairExists, FromGenericPair,
    GetPoolReserves, LiquiditySource, LiquiditySourceType, ManagementMode, RewardReason,
    ToFeeAccount,
};
use orml_traits::currency::MultiCurrency;
use permissions::{Scope, BURN, MINT};

mod aliases;
use aliases::{
    AccountIdOf, AssetIdOf, DEXIdOf, DepositLiquidityActionOf, PairSwapActionOf,
    PolySwapActionStructOf, TechAccountIdOf, TechAssetIdOf, WithdrawLiquidityActionOf,
};

pub mod weights;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[macro_use]
mod macros;

mod math;
mod utils;

mod bounds;
use bounds::*;

mod action_deposit_liquidity;
mod action_pair_swap;
mod action_poly_swap;
mod action_withdraw_liquidity;

mod operations;
pub use operations::*;

const MIN_LIQUIDITY: u128 = 1000;

pub trait WeightInfo {
    fn swap_pair() -> Weight;
    fn deposit_liquidity() -> Weight;
    fn withdraw_liquidity() -> Weight;
    fn initialize_pool() -> Weight;
}

impl<T: Config> Module<T> {
    fn initialize_pool_properties(
        dex_id: &T::DEXId,
        asset_a: &T::AssetId,
        asset_b: &T::AssetId,
        reserves_account_id: &T::AccountId,
        fees_account_id: &T::AccountId,
        marker_asset_id: &T::AssetId,
    ) -> DispatchResult {
        let base_asset_id: T::AssetId = T::GetBaseAssetId::get();
        let (sorted_asset_a, sorted_asset_b) = if &base_asset_id == asset_a {
            (asset_a, asset_b)
        } else if &base_asset_id == asset_b {
            (asset_b, asset_a)
        } else {
            let hash_key = common::comm_merkle_op(asset_a, asset_b);
            let (asset_a_pair, asset_b_pair) =
                common::sort_with_hash_key(hash_key, (asset_a, &()), (asset_b, &()));
            (asset_a_pair.0, asset_b_pair.0)
        };
        trading_pair::Module::<T>::enable_source_for_trading_pair(
            dex_id,
            sorted_asset_a,
            sorted_asset_b,
            LiquiditySourceType::XYKPool,
        )?;
        Properties::<T>::insert(
            sorted_asset_a,
            sorted_asset_b,
            (
                reserves_account_id.clone(),
                fees_account_id.clone(),
                marker_asset_id.clone(),
            ),
        );
        Ok(())
    }

    fn update_reserves(
        asset_a: &T::AssetId,
        asset_b: &T::AssetId,
        balance_pair: (&Balance, &Balance),
    ) {
        let base_asset_id: T::AssetId = T::GetBaseAssetId::get();
        if base_asset_id == asset_a.clone() {
            Reserves::<T>::insert(asset_a, asset_b, (balance_pair.0, balance_pair.1));
        } else if base_asset_id == asset_b.clone() {
            Reserves::<T>::insert(asset_b, asset_a, (balance_pair.1, balance_pair.0));
        } else {
            let hash_key = common::comm_merkle_op(asset_a, asset_b);
            let (pair_u, pair_v) = common::sort_with_hash_key(
                hash_key,
                (asset_a, balance_pair.0),
                (asset_b, balance_pair.1),
            );
            Reserves::<T>::insert(pair_u.0, pair_v.0, (pair_u.1, pair_v.1));
        }
    }

    pub fn initialize_pool_unchecked(
        _source: AccountIdOf<T>,
        dex_id: DEXIdOf<T>,
        asset_a: AssetIdOf<T>,
        asset_b: AssetIdOf<T>,
    ) -> Result<
        (
            common::TradingPair<TechAssetIdOf<T>>,
            TechAccountIdOf<T>,
            TechAccountIdOf<T>,
            TechAssetIdOf<T>,
        ),
        DispatchError,
    > {
        let (trading_pair, tech_acc_id) =
            Module::<T>::tech_account_from_dex_and_asset_pair(dex_id, asset_a, asset_b)?;
        let fee_acc_id = tech_acc_id.to_fee_account().unwrap();
        let mark_asset = Module::<T>::get_marking_asset(&tech_acc_id)?;
        // Function initialize_pools is usually called once, just quick check if tech
        // account is not registered is enough to do the job.
        // If function is called second time, than this is not usual case and additional checks
        // can be done, check every condition for `PoolIsAlreadyInitialized`.
        if technical::Module::<T>::ensure_tech_account_registered(&tech_acc_id).is_ok() {
            if technical::Module::<T>::ensure_tech_account_registered(&fee_acc_id).is_ok()
                && assets::Module::<T>::ensure_asset_exists(&mark_asset.into()).is_ok()
                && trading_pair::Module::<T>::ensure_trading_pair_exists(
                    &dex_id,
                    &trading_pair.base_asset_id.into(),
                    &trading_pair.target_asset_id.into(),
                )
                .is_ok()
            {
                Err(Error::<T>::PoolIsAlreadyInitialized)?;
            } else {
                Err(Error::<T>::PoolInitializationIsInvalid)?;
            }
        }
        technical::Module::<T>::register_tech_account_id(tech_acc_id.clone())?;
        technical::Module::<T>::register_tech_account_id(fee_acc_id.clone())?;
        Ok((trading_pair, tech_acc_id, fee_acc_id, mark_asset))
    }

    fn deposit_liquidity_unchecked(
        source: AccountIdOf<T>,
        dex_id: DEXIdOf<T>,
        input_asset_a: AssetIdOf<T>,
        input_asset_b: AssetIdOf<T>,
        input_a_desired: Balance,
        input_b_desired: Balance,
        input_a_min: Balance,
        input_b_min: Balance,
    ) -> DispatchResult {
        let (_, tech_acc_id) = Module::<T>::tech_account_from_dex_and_asset_pair(
            dex_id,
            input_asset_a,
            input_asset_b,
        )?;
        ensure!(
            input_a_desired >= input_a_min && input_b_desired >= input_b_min,
            Error::<T>::InvalidMinimumBoundValueOfBalance
        );
        let mark_asset = Module::<T>::get_marking_asset(&tech_acc_id)?;
        let action = PolySwapActionStructOf::<T>::DepositLiquidity(DepositLiquidityActionOf::<T> {
            client_account: None,
            receiver_account: None,
            pool_account: tech_acc_id,
            source: ResourcePair(
                Resource {
                    asset: input_asset_a,
                    amount: Bounds::<Balance>::RangeFromDesiredToMin(input_a_desired, input_a_min),
                },
                Resource {
                    asset: input_asset_b,
                    amount: Bounds::<Balance>::RangeFromDesiredToMin(input_b_desired, input_b_min),
                },
            ),
            destination: Resource {
                asset: mark_asset,
                amount: Bounds::Decide,
            },
            min_liquidity: None,
        });
        let action = T::PolySwapAction::from(action);
        let mut action = action.into();
        technical::Module::<T>::perform_create_swap(source, &mut action)?;
        Ok(())
    }

    fn withdraw_liquidity_unchecked(
        source: AccountIdOf<T>,
        dex_id: DEXIdOf<T>,
        output_asset_a: AssetIdOf<T>,
        output_asset_b: AssetIdOf<T>,
        marker_asset_desired: Balance,
        output_a_min: Balance,
        output_b_min: Balance,
    ) -> DispatchResult {
        let (_, tech_acc_id) = Module::<T>::tech_account_from_dex_and_asset_pair(
            dex_id,
            output_asset_a,
            output_asset_b,
        )?;
        let mark_asset = Module::<T>::get_marking_asset(&tech_acc_id)?;
        let action =
            PolySwapActionStructOf::<T>::WithdrawLiquidity(WithdrawLiquidityActionOf::<T> {
                client_account: None,
                receiver_account_a: None,
                receiver_account_b: None,
                pool_account: tech_acc_id,
                source: Resource {
                    asset: mark_asset,
                    amount: Bounds::Desired(marker_asset_desired),
                },
                destination: ResourcePair(
                    Resource {
                        asset: output_asset_a,
                        amount: Bounds::Min(output_a_min),
                    },
                    Resource {
                        asset: output_asset_b,
                        amount: Bounds::Min(output_b_min),
                    },
                ),
            });
        let action = T::PolySwapAction::from(action);
        let mut action = action.into();
        technical::Module::<T>::perform_create_swap(source, &mut action)?;
        Ok(())
    }
}

impl<T: Config> LiquiditySource<T::DEXId, T::AccountId, T::AssetId, Balance, DispatchError>
    for Module<T>
{
    fn can_exchange(
        dex_id: &T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
    ) -> bool {
        // Function clause is used here, because in this case it is other scope and it not
        // conflicted with bool type.
        let res = || {
            let tech_acc_id = T::TechAccountId::from_generic_pair(
                "PoolXYK".into(),
                "CanExchangeOperation".into(),
            );
            //TODO: Account registration is not needed to do operation, is this ok?
            //Technical::register_tech_account_id(tech_acc_id)?;
            let repr = technical::Module::<T>::tech_account_id_to_account_id(&tech_acc_id)?;
            //FIXME: Use special max variable that is good for this operation.
            T::Currency::deposit(input_asset_id.clone(), &repr, balance!(999999999))?;
            let swap_amount = common::prelude::SwapAmount::WithDesiredInput {
                //FIXME: Use special max variable that is good for this operation.
                desired_amount_in: balance!(0.000000001),
                min_amount_out: 0,
            };
            Module::<T>::exchange(
                &repr,
                &repr,
                dex_id,
                input_asset_id,
                output_asset_id,
                swap_amount,
            )?;
            Ok(())
        };
        frame_support::storage::with_transaction(|| {
            let v: DispatchResult = res();
            sp_runtime::TransactionOutcome::Rollback(v.is_ok())
        })
    }

    fn quote(
        dex_id: &T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        swap_amount: SwapAmount<Balance>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        let res = || {
            let tech_acc_id =
                T::TechAccountId::from_generic_pair("PoolXYK".into(), "QuoteOperation".into());
            //TODO: Account registration is not needed to do operation, is this ok?
            //Technical::register_tech_account_id(tech_acc_id)?;
            let repr = technical::Module::<T>::tech_account_id_to_account_id(&tech_acc_id)?;
            //FIXME: Use special max variable that is good for this operation.
            T::Currency::deposit(input_asset_id.clone(), &repr, balance!(999999999))?;
            Module::<T>::exchange(
                &repr,
                &repr,
                dex_id,
                input_asset_id,
                output_asset_id,
                swap_amount,
            )
        };
        frame_support::storage::with_transaction(|| {
            let v: Result<SwapOutcome<Balance>, DispatchError> = res();
            sp_runtime::TransactionOutcome::Rollback(v)
        })
    }

    fn exchange(
        sender: &T::AccountId,
        receiver: &T::AccountId,
        dex_id: &T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        swap_amount: SwapAmount<Balance>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        let (_, tech_acc_id) = Module::<T>::tech_account_from_dex_and_asset_pair(
            *dex_id,
            *input_asset_id,
            *output_asset_id,
        )?;
        let (source_amount, destination_amount) =
            Module::<T>::get_bounds_from_swap_amount(swap_amount.clone())?;
        let mut action = PolySwapActionStructOf::<T>::PairSwap(PairSwapActionOf::<T> {
            client_account: None,
            receiver_account: Some(receiver.clone()),
            pool_account: tech_acc_id,
            source: Resource {
                asset: *input_asset_id,
                amount: source_amount,
            },
            destination: Resource {
                asset: *output_asset_id,
                amount: destination_amount,
            },
            fee: None,
            fee_account: None,
            get_fee_from_destination: None,
        });
        common::SwapRulesValidation::<AccountIdOf<T>, TechAccountIdOf<T>, T>::prepare_and_validate(
            &mut action,
            Some(sender),
        )?;

        // It is guarantee that unwrap is always ok.
        // Clone is used here because action is used for perform_create_swap_unchecked.
        let retval = match action.clone() {
            PolySwapAction::PairSwap(a) => {
                let (fee, amount) = match swap_amount {
                    SwapAmount::WithDesiredInput { .. } => {
                        (a.fee.unwrap(), a.destination.amount.unwrap())
                    }
                    SwapAmount::WithDesiredOutput { .. } => {
                        (a.fee.unwrap(), a.source.amount.unwrap())
                    }
                };
                Ok(common::prelude::SwapOutcome::new(amount, fee))
            }
            _ => unreachable!("we know that always PairSwap is used"),
        };

        let action = T::PolySwapAction::from(action);
        let mut action = action.into();
        technical::Module::<T>::perform_create_swap_unchecked(sender.clone(), &mut action)?;

        retval
    }

    fn check_rewards(
        _target_id: &T::DEXId,
        _input_asset_id: &T::AssetId,
        _output_asset_id: &T::AssetId,
        _input_amount: Balance,
        _output_amount: Balance,
    ) -> Result<Vec<(Balance, T::AssetId, RewardReason)>, DispatchError> {
        // XYK Pool has no rewards currently
        Ok(Vec::new())
    }
}

impl<T: Config> GetPoolReserves<T::AssetId> for Module<T> {
    fn reserves(base_asset: &T::AssetId, other_asset: &T::AssetId) -> (Balance, Balance) {
        Reserves::<T>::get(base_asset, other_asset)
    }
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::config]
    pub trait Config:
        frame_system::Config
        + technical::Config
        + dex_manager::Config
        + trading_pair::Config
        + pswap_distribution::Config
    {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        //TODO: implement and use + Into<SwapActionOf<T> for this types.
        type PairSwapAction: common::SwapAction<AccountIdOf<Self>, TechAccountIdOf<Self>, Self>
            + Parameter;
        type DepositLiquidityAction: common::SwapAction<AccountIdOf<Self>, TechAccountIdOf<Self>, Self>
            + Parameter;
        type WithdrawLiquidityAction: common::SwapAction<AccountIdOf<Self>, TechAccountIdOf<Self>, Self>
            + Parameter;
        type PolySwapAction: common::SwapAction<AccountIdOf<Self>, TechAccountIdOf<Self>, Self>
            + Parameter
            + Into<<Self as technical::Config>::SwapAction>
            + From<PolySwapActionStructOf<Self>>;
        type EnsureDEXManager: EnsureDEXManager<Self::DEXId, Self::AccountId, DispatchError>;
        type GetFee: Get<Fixed>;

        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(<T as Config>::WeightInfo::swap_pair())]
        pub fn swap_pair(
            origin: OriginFor<T>,
            receiver: AccountIdOf<T>,
            dex_id: DEXIdOf<T>,
            input_asset_id: AssetIdOf<T>,
            output_asset_id: AssetIdOf<T>,
            swap_amount: SwapAmount<Balance>,
        ) -> DispatchResultWithPostInfo {
            let source = ensure_signed(origin)?;
            Module::<T>::exchange(
                &source,
                &receiver,
                &dex_id,
                &input_asset_id,
                &output_asset_id,
                swap_amount,
            )?;
            Ok(().into())
        }

        #[pallet::weight(<T as Config>::WeightInfo::deposit_liquidity())]
        pub fn deposit_liquidity(
            origin: OriginFor<T>,
            dex_id: DEXIdOf<T>,
            input_asset_a: AssetIdOf<T>,
            input_asset_b: AssetIdOf<T>,
            input_a_desired: Balance,
            input_b_desired: Balance,
            input_a_min: Balance,
            input_b_min: Balance,
        ) -> DispatchResultWithPostInfo {
            let source = ensure_signed(origin)?;
            Module::<T>::deposit_liquidity_unchecked(
                source,
                dex_id,
                input_asset_a,
                input_asset_b,
                input_a_desired,
                input_b_desired,
                input_a_min,
                input_b_min,
            )?;
            Ok(().into())
        }

        #[pallet::weight(<T as Config>::WeightInfo::withdraw_liquidity())]
        pub fn withdraw_liquidity(
            origin: OriginFor<T>,
            dex_id: DEXIdOf<T>,
            output_asset_a: AssetIdOf<T>,
            output_asset_b: AssetIdOf<T>,
            marker_asset_desired: Balance,
            output_a_min: Balance,
            output_b_min: Balance,
        ) -> DispatchResultWithPostInfo {
            let source = ensure_signed(origin)?;
            Module::<T>::withdraw_liquidity_unchecked(
                source,
                dex_id,
                output_asset_a,
                output_asset_b,
                marker_asset_desired,
                output_a_min,
                output_b_min,
            )?;
            Ok(().into())
        }

        #[pallet::weight(<T as Config>::WeightInfo::initialize_pool())]
        pub fn initialize_pool(
            origin: OriginFor<T>,
            dex_id: DEXIdOf<T>,
            asset_a: AssetIdOf<T>,
            asset_b: AssetIdOf<T>,
        ) -> DispatchResultWithPostInfo {
            common::with_transaction(|| {
                let source = ensure_signed(origin.clone())?;
                <T as Config>::EnsureDEXManager::ensure_can_manage(
                    &dex_id,
                    origin.clone(),
                    ManagementMode::Public,
                )?;
                let (_, tech_account_id, fees_account_id, mark_asset) =
                    Module::<T>::initialize_pool_unchecked(
                        source.clone(),
                        dex_id,
                        asset_a,
                        asset_b,
                    )?;
                let mark_asset_repr: T::AssetId = mark_asset.into();
                assets::Module::<T>::register_asset_id(
                    source.clone(),
                    mark_asset_repr,
                    AssetSymbol(b"XYKPOOL".to_vec()),
                    AssetName(b"XYK LP Tokens".to_vec()),
                    18,
                    0,
                    true,
                )?;
                let ta_repr =
                    technical::Module::<T>::tech_account_id_to_account_id(&tech_account_id)?;
                let fees_ta_repr =
                    technical::Module::<T>::tech_account_id_to_account_id(&fees_account_id)?;
                // Minting permission is needed for technical account to mint markered tokens of
                // liquidity into account who deposit liquidity.
                permissions::Module::<T>::grant_permission_with_scope(
                    source.clone(),
                    ta_repr.clone(),
                    MINT,
                    Scope::Limited(hash(&Into::<AssetIdOf<T>>::into(mark_asset.clone()))),
                )?;
                permissions::Module::<T>::grant_permission_with_scope(
                    source,
                    ta_repr.clone(),
                    BURN,
                    Scope::Limited(hash(&Into::<AssetIdOf<T>>::into(mark_asset.clone()))),
                )?;
                Module::<T>::initialize_pool_properties(
                    &dex_id,
                    &asset_a,
                    &asset_b,
                    &ta_repr,
                    &fees_ta_repr,
                    &mark_asset_repr,
                )?;
                pswap_distribution::Module::<T>::subscribe(
                    fees_ta_repr,
                    dex_id,
                    mark_asset_repr,
                    None,
                )?;
                MarkerTokensIndex::<T>::mutate(|mti| mti.insert(mark_asset_repr));
                Self::deposit_event(Event::PoolIsInitialized(ta_repr));
                Ok(().into())
            })
        }
    }

    #[pallet::event]
    #[pallet::metadata(AccountIdOf<T> = "AccountId")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        // New pool for particular pair was initialized. [Reserves Account Id]
        PoolIsInitialized(AccountIdOf<T>),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// It is impossible to calculate fee for some pair swap operation, or other operation.
        UnableToCalculateFee,
        /// Is is impossible to get balance.
        UnableToGetBalance,
        /// Impossible to decide asset pair amounts.
        ImpossibleToDecideAssetPairAmounts,
        /// Pool pair ratio and pair swap ratio are different.
        PoolPairRatioAndPairSwapRatioIsDifferent,
        /// Pair swap action fee is smaller than recommended.
        PairSwapActionFeeIsSmallerThanRecommended,
        /// Source balance is not large enough.
        SourceBalanceIsNotLargeEnough,
        /// Target balance is not large enough.
        TargetBalanceIsNotLargeEnough,
        /// It is not possible to derive fee account.
        UnableToDeriveFeeAccount,
        /// The fee account is invalid.
        FeeAccountIsInvalid,
        /// Source and client accounts do not match as equal.
        SourceAndClientAccountDoNotMatchAsEqual,
        /// In this case assets must not be same.
        AssetsMustNotBeSame,
        /// Impossible to decide deposit liquidity amounts.
        ImpossibleToDecideDepositLiquidityAmounts,
        /// Invalid deposit liquidity base asset amount.
        InvalidDepositLiquidityBasicAssetAmount,
        /// Invalid deposit liquidity target asset amount.
        InvalidDepositLiquidityTargetAssetAmount,
        /// Pair swap action minimum liquidity is smaller than recommended.
        PairSwapActionMinimumLiquidityIsSmallerThanRecommended,
        /// Destination amount of liquidity is not large enough.
        DestinationAmountOfLiquidityIsNotLargeEnough,
        /// Source base amount is not large enough.
        SourceBaseAmountIsNotLargeEnough,
        /// Target base amount is not large enough.
        TargetBaseAmountIsNotLargeEnough,
        /// The balance structure of pool is invalid.
        PoolIsInvalid,
        /// The pool has empty liquidity.
        PoolIsEmpty,
        /// Amount parameter has zero value, it is invalid.
        ZeroValueInAmountParameter,
        /// The account balance is invalid.
        AccountBalanceIsInvalid,
        /// Invalid deposit liquidity destination amount.
        InvalidDepositLiquidityDestinationAmount,
        /// Initial liquidity deposit ratio must be defined.
        InitialLiqudityDepositRatioMustBeDefined,
        /// Technical asset is not representable.
        TechAssetIsNotRepresentable,
        /// Unable or impossible to decide marker asset.
        UnableToDecideMarkerAsset,
        /// Unable or impossible to get asset representation.
        UnableToGetAssetRepr,
        /// Impossible to decide withdraw liquidity amounts.
        ImpossibleToDecideWithdrawLiquidityAmounts,
        /// Invalid withdraw liquidity base asset amount.
        InvalidWithdrawLiquidityBasicAssetAmount,
        /// Invalid withdraw liquidity target asset amount.
        InvalidWithdrawLiquidityTargetAssetAmount,
        /// Source base amount is too large.
        SourceBaseAmountIsTooLarge,
        /// Source balance of liquidity is not large enough.
        SourceBalanceOfLiquidityTokensIsNotLargeEnough,
        /// Destination base balance is not large enough.
        DestinationBaseBalanceIsNotLargeEnough,
        /// Destination base balance is not large enough.
        DestinationTargetBalanceIsNotLargeEnough,
        /// Asset for liquidity marking is invalid.
        InvalidAssetForLiquidityMarking,
        /// Error in asset decoding.
        AssetDecodingError,
        /// Calculated value is out of desired bounds.
        CalculatedValueIsOutOfDesiredBounds,
        /// The base asset is not matched with any asset arguments.
        BaseAssetIsNotMatchedWithAnyAssetArguments,
        /// Some values need to be same, the destination amount must be same.
        DestinationAmountMustBeSame,
        /// Some values need to be same, the source amount must be same.
        SourceAmountMustBeSame,
        /// The pool initialization is invalid and has failed.
        PoolInitializationIsInvalid,
        /// The pool is already initialized.
        PoolIsAlreadyInitialized,
        /// The minimum bound values of balance are invalid.
        InvalidMinimumBoundValueOfBalance,
        /// It is impossible to decide valid pair values from range for this pool.
        ImpossibleToDecideValidPairValuesFromRangeForThisPool,
        /// This range values is not validy by rules of correct range.
        RangeValuesIsInvalid,
        /// The values that is calculated is out out of required bounds.
        CalculatedValueIsNotMeetsRequiredBoundaries,
        /// In this case getting fee from destination is impossible.
        GettingFeeFromDestinationIsImpossible,
        /// Math calculation with fixed number has failed to complete.
        FixedWrapperCalculationFailed,
        /// This case if not supported by logic of pool of validation code.
        ThisCaseIsNotSupported,
        /// Pool becomes invalid after operation.
        PoolBecameInvalidAfterOperation,
        /// Unable to convert asset to tech asset id.
        UnableToConvertAssetToTechAssetId,
        /// Unable to get XOR part from marker asset.
        UnableToGetXORPartFromMarkerAsset,
        /// Pool token supply has reached limit of data type.
        PoolTokenSupplyOverflow,
    }

    /// Updated after last liquidity change operation.
    /// [Base Asset Id (XOR) -> Target Asset Id => (Base Balance, Target Balance)].
    /// This storage records is not used as source of information, but used as quick cache for
    /// information that comes from balances for assets from technical accounts.
    /// For example, communication with technical accounts and their storage is not needed, and this
    /// pair to balance cache can be used quickly.
    #[pallet::storage]
    #[pallet::getter(fn reserves)]
    pub type Reserves<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AssetId,
        Blake2_128Concat,
        T::AssetId,
        (Balance, Balance),
        ValueQuery,
    >;

    /// Collection of all registered marker tokens.
    #[pallet::storage]
    #[pallet::getter(fn marker_tokens_index)]
    pub type MarkerTokensIndex<T: Config> = StorageValue<_, BTreeSet<T::AssetId>, ValueQuery>;

    /// Properties of particular pool. [Reserves Account Id, Fees Account Id, Marker Asset Id]
    #[pallet::storage]
    #[pallet::getter(fn properties)]
    pub type Properties<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AssetId,
        Blake2_128Concat,
        T::AssetId,
        (T::AccountId, T::AccountId, T::AssetId),
    >;
}
