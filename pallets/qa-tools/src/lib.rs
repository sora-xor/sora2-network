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

//! # Qa tools pallet
//!
//! As mentioned in the name, it's a pallet containing extrinsics or other tools that can help
//! QAs in their work. Additionally, it is intended to be used for simplifying unit testing.
//!
//! Because of its nature, the pallet should never be released in production. Therefore, it is
//! expected to be guarded by `private-net` feature.
//! It is not as thoroughly designed and tested as other pallets, so issues with it can be expected.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]
#![feature(is_some_and)]

pub use pallet::*;
pub use weights::WeightInfo;

// private-net to make circular dependencies work
#[cfg(all(test, feature = "private-net"))]
mod tests;

pub mod pallet_tools;
pub mod weights;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    use common::{
        AccountIdOf, AssetIdOf, AssetInfoProvider, AssetName, AssetSymbol, BalancePrecision,
        ContentSource, DEXInfo, Description, DexIdOf, DexInfoProvider, ExtendedAssetsManager,
        OrderBookId, SyntheticInfoProvider, TradingPairSourceManager,
    };
    use frame_support::dispatch::{DispatchErrorWithPostInfo, PostDispatchInfo};
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use order_book::MomentOf;
    use pallet_tools::liquidity_proxy::liquidity_sources;
    use pallet_tools::pool_xyk::AssetPairInput;
    use pallet_tools::xst::{BaseInput, SyntheticInput, SyntheticOutput};
    use sp_std::prelude::*;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config:
        frame_system::Config
        + assets::Config
        + common::Config
        + dex_manager::Config
        + extended_assets::Config
        + order_book::Config
        + pool_xyk::Config
        + presto::Config
        + xst::Config
        + price_tools::Config
        + band::Config
        + oracle_proxy::Config
        + multicollateral_bonding_curve_pool::Config
    {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type AssetInfoProvider: AssetInfoProvider<
            AssetIdOf<Self>,
            Self::AccountId,
            AssetSymbol,
            AssetName,
            BalancePrecision,
            ContentSource,
            Description,
        >;
        type DexInfoProvider: DexInfoProvider<Self::DEXId, DEXInfo<AssetIdOf<Self>>>;
        type SyntheticInfoProvider: SyntheticInfoProvider<AssetIdOf<Self>>;
        type TradingPairSourceManager: TradingPairSourceManager<Self::DEXId, AssetIdOf<Self>>;
        type ExtendedAssetsManager: ExtendedAssetsManager<
            AssetIdOf<Self>,
            Self::Moment,
            ContentSource,
        >;
        type Symbol: From<<Self as band::Config>::Symbol>
            + From<<Self as xst::Config>::Symbol>
            + Into<<Self as xst::Config>::Symbol>
            + Into<<Self as band::Config>::Symbol>
            + From<common::SymbolName>
            + Parameter
            + Ord;
        type WeightInfo: WeightInfo;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Requested order books have been created.
        OrderBooksCreated,
        /// Requested order book have been filled.
        OrderBooksFilled,
        /// Xyk liquidity source has been initialized successfully.
        XykInitialized {
            /// Exact prices for token pairs achievable after the initialization.
            /// Should correspond 1-to-1 to the initialization input and be quite close to the given values.
            prices_achieved: Vec<AssetPairInput<DexIdOf<T>, AssetIdOf<T>>>,
        },
        /// XST liquidity source has been initialized successfully.
        XstInitialized {
            /// Exact `quote`/`exchange` calls achievable after the initialization.
            /// Should correspond 1-to-1 to the initialization input and be quite close to the given values.
            quotes_achieved: Vec<SyntheticOutput<AssetIdOf<T>>>,
        },
        /// Multicollateral bonding curve liquidity source has been initialized successfully.
        McbcInitialized {
            /// Exact reference prices achieved for the collateral assets.
            collateral_ref_prices: Vec<(AssetIdOf<T>, pallet_tools::price_tools::AssetPrices)>,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        // common errors
        /// Error in calculations.
        ArithmeticError,
        /// Buy price cannot be lower than sell price of an asset
        BuyLessThanSell,

        // order book errors
        /// Did not find an order book with given id to fill. Likely an error with order book creation.
        CannotFillUnknownOrderBook,
        /// Order Book already exists
        OrderBookAlreadyExists,
        /// Price step, best price, and worst price must be a multiple of order book's tick size.
        /// Price step must also be non-zero.
        IncorrectPrice,
        /// Provided range is incorrect, check that lower bound is less or equal than the upper one.
        EmptyRandomRange,
        /// The range for generating order amounts must be within order book's accepted values.
        OutOfBoundsRandomRange,
        /// The count of created orders is too large.
        TooManyOrders,
        /// The count of prices to fill is too large.
        TooManyPrices,

        // xyk pool errors
        /// Cannot initialize pool with for non-divisible assets.
        AssetsMustBeDivisible,

        // xst errors
        /// Cannot register new asset because it already exists.
        AssetAlreadyExists,
        /// Could not find already existing synthetic.
        UnknownSynthetic,

        // mcbc errors
        /// Cannot initialize MCBC for unknown asset.
        UnknownMcbcAsset,
        /// TBCD must be initialized using different field/function (see `tbcd_collateral` and `TbcdCollateralInput`).
        IncorrectCollateralAsset,

        // price-tools errors
        /// Cannot deduce price of synthetic base asset because there is no existing price for reference asset.
        /// You can use `price_tools_set_asset_price` extrinsic to set its price.
        ReferenceAssetPriceNotFound,

        // presto errors
        /// Cannot initialize SBT metadata with the list of regulated assets
        FailToInitializeRegulatedAssets,
    }

    #[derive(Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, Debug)]
    pub enum InputAssetId<AssetId> {
        McbcReference,
        XstReference,
        Other(AssetId),
    }

    impl<AssetId> InputAssetId<AssetId> {
        pub fn resolve<T>(self) -> AssetIdOf<T>
        where
            T: Config,
            AssetIdOf<T>: From<AssetId>,
        {
            match self {
                InputAssetId::McbcReference => {
                    multicollateral_bonding_curve_pool::ReferenceAssetId::<T>::get()
                }
                InputAssetId::XstReference => xst::ReferenceAssetId::<T>::get(),
                InputAssetId::Other(id) => id.into(),
            }
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Create multiple many order books with parameters and fill them according to given parameters.
        ///
        /// Balance for placing the orders is minted automatically, trading pairs are
        /// created if needed.
        ///
        /// In order to create empty order books, one can leave settings empty.
        ///
        /// Parameters:
        /// - `origin`: root
        /// - `bids_owner`: Creator of the buy orders placed on the order books,
        /// - `asks_owner`: Creator of the sell orders placed on the order books,
        /// - `settings`: Parameters for creation of the order book and placing the orders in each order book.
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::order_book_create_and_fill_batch())]
        pub fn order_book_create_and_fill_batch(
            origin: OriginFor<T>,
            bids_owner: T::AccountId,
            asks_owner: T::AccountId,
            settings: Vec<(
                OrderBookId<AssetIdOf<T>, T::DEXId>,
                pallet_tools::order_book::OrderBookAttributes,
                pallet_tools::order_book::FillInput<MomentOf<T>, BlockNumberFor<T>>,
            )>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            // Replace with more convenient `with_pays_fee` when/if available
            // https://github.com/paritytech/substrate/pull/14470
            liquidity_sources::create_and_fill_order_book_batch::<T>(
                bids_owner, asks_owner, settings,
            )
            .map_err(|e| DispatchErrorWithPostInfo {
                post_info: PostDispatchInfo {
                    actual_weight: None,
                    pays_fee: Pays::No,
                },
                error: e,
            })?;

            // Even though these facts can be deduced from the extrinsic execution success,
            // it would be strange not to emit anything, while other initialization extrinsics do.
            Self::deposit_event(Event::<T>::OrderBooksCreated);
            Self::deposit_event(Event::<T>::OrderBooksFilled);

            // Extrinsic is only for testing, so we return all fees
            // for simplicity.
            Ok(PostDispatchInfo {
                actual_weight: None,
                pays_fee: Pays::No,
            })
        }

        /// Fill the order books according to given parameters.
        ///
        /// Balance for placing the orders is minted automatically.
        ///
        /// Parameters:
        /// - `origin`: root
        /// - `bids_owner`: Creator of the buy orders placed on the order books,
        /// - `asks_owner`: Creator of the sell orders placed on the order books,
        /// - `settings`: Parameters for placing the orders in each order book.
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::order_book_fill_batch())]
        pub fn order_book_fill_batch(
            origin: OriginFor<T>,
            bids_owner: T::AccountId,
            asks_owner: T::AccountId,
            settings: Vec<(
                OrderBookId<AssetIdOf<T>, T::DEXId>,
                pallet_tools::order_book::FillInput<MomentOf<T>, BlockNumberFor<T>>,
            )>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            // Replace with more convenient `with_pays_fee` when/if available
            // https://github.com/paritytech/substrate/pull/14470
            liquidity_sources::fill_order_book::<T>(bids_owner, asks_owner, settings).map_err(
                |e| DispatchErrorWithPostInfo {
                    post_info: PostDispatchInfo {
                        actual_weight: None,
                        pays_fee: Pays::No,
                    },
                    error: e,
                },
            )?;

            Self::deposit_event(Event::<T>::OrderBooksFilled);

            // Extrinsic is only for testing, so we return all fees
            // for simplicity.
            Ok(PostDispatchInfo {
                actual_weight: None,
                pays_fee: Pays::No,
            })
        }

        /// Initialize xyk pool liquidity source.
        ///
        /// Parameters:
        /// - `origin`: Root
        /// - `account`: Some account to use during the initialization
        /// - `pairs`: Asset pairs to initialize.
        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::xyk_initialize())]
        pub fn xyk_initialize(
            origin: OriginFor<T>,
            account: AccountIdOf<T>,
            pairs: Vec<AssetPairInput<DexIdOf<T>, AssetIdOf<T>>>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            let prices_achieved =
                liquidity_sources::initialize_xyk::<T>(account, pairs).map_err(|e| {
                    DispatchErrorWithPostInfo {
                        post_info: PostDispatchInfo {
                            actual_weight: None,
                            pays_fee: Pays::No,
                        },
                        error: e,
                    }
                })?;

            Self::deposit_event(Event::<T>::XykInitialized { prices_achieved });

            // Extrinsic is only for testing, so we return all fees
            // for simplicity.
            Ok(PostDispatchInfo {
                actual_weight: None,
                pays_fee: Pays::No,
            })
        }

        /// Initialize xst liquidity source. In xst's `quote`, one of the assets is the synthetic base
        /// (XST) and the other one is a synthetic asset.
        ///
        /// Parameters:
        /// - `origin`: Root
        /// - `base_prices`: Synthetic base asset price update. Usually buy price > sell.
        /// - `synthetics_prices`: Synthetic initialization;
        /// registration of an asset + setting up prices for target quotes.
        /// - `relayer`: Account which will be the author of prices fed to `band` pallet;
        ///
        /// Emits events with actual quotes achieved after initialization;
        /// more details in [`liquidity_sources::initialize_xst`]
        #[pallet::call_index(3)]
        #[pallet::weight(<T as Config>::WeightInfo::xst_initialize())]
        pub fn xst_initialize(
            origin: OriginFor<T>,
            base_prices: Option<BaseInput>,
            synthetics_prices: Vec<SyntheticInput<AssetIdOf<T>, <T as Config>::Symbol>>,
            relayer: T::AccountId,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            let quotes_achieved =
                liquidity_sources::initialize_xst::<T>(base_prices, synthetics_prices, relayer)
                    .map_err(|e| DispatchErrorWithPostInfo {
                        post_info: PostDispatchInfo {
                            actual_weight: None,
                            pays_fee: Pays::No,
                        },
                        error: e,
                    })?;

            Self::deposit_event(Event::<T>::XstInitialized { quotes_achieved });

            // Extrinsic is only for testing, so we return all fees
            // for simplicity.
            Ok(PostDispatchInfo {
                actual_weight: None,
                pays_fee: Pays::No,
            })
        }

        /// Initialize mcbc liquidity source.
        ///
        /// Parameters:
        /// - `origin`: Root
        /// - `base_supply`: Control supply of XOR,
        /// - `other_collaterals`: Variables related to arbitrary collateral-specific pricing,
        /// - `tbcd_collateral`: TBCD-specific pricing variables.
        #[pallet::call_index(4)]
        #[pallet::weight(<T as Config>::WeightInfo::price_tools_set_reference_asset_price())]
        pub fn mcbc_initialize(
            origin: OriginFor<T>,
            base_supply: Option<pallet_tools::mcbc::BaseSupply<T::AccountId>>,
            other_collaterals: Vec<pallet_tools::mcbc::OtherCollateralInput<AssetIdOf<T>>>,
            tbcd_collateral: Option<pallet_tools::mcbc::TbcdCollateralInput>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            let ref_prices = liquidity_sources::initialize_mcbc::<T>(
                base_supply,
                other_collaterals,
                tbcd_collateral,
            )
            .map_err(|e| DispatchErrorWithPostInfo {
                post_info: PostDispatchInfo {
                    actual_weight: None,
                    pays_fee: Pays::No,
                },
                error: e,
            })?;

            Self::deposit_event(Event::<T>::McbcInitialized {
                collateral_ref_prices: ref_prices.into_iter().collect(),
            });

            // Extrinsic is only for testing, so we return all fees
            // for simplicity.
            Ok(PostDispatchInfo {
                actual_weight: None,
                pays_fee: Pays::No,
            })
        }

        /// Set prices of an asset in `price_tools` pallet.
        /// Ignores pallet restrictions on price speed change.
        ///
        /// Parameters:
        /// - `origin`: Root
        /// - `asset_per_xor`: Prices (1 XOR in terms of the corresponding asset).
        /// - `asset_id`: Asset identifier; can be some common constant for easier input.
        #[pallet::call_index(5)]
        #[pallet::weight(<T as Config>::WeightInfo::price_tools_set_reference_asset_price())]
        pub fn price_tools_set_asset_price(
            origin: OriginFor<T>,
            asset_per_xor: pallet_tools::price_tools::AssetPrices,
            asset_id: InputAssetId<AssetIdOf<T>>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            let asset_id = asset_id.resolve::<T>();
            pallet_tools::price_tools::set_xor_prices::<T>(&asset_id, asset_per_xor)?;

            // Extrinsic is only for testing, so we return all fees
            // for simplicity.
            Ok(PostDispatchInfo {
                actual_weight: None,
                pays_fee: Pays::No,
            })
        }

        /// Allows to initialize necessary Presto data (assets, DEX etc.) in testnet without migration.
        ///
        /// Parameters:
        /// - `origin`: Root
        #[pallet::call_index(6)]
        #[pallet::weight(<T as Config>::WeightInfo::presto_initialize())]
        pub fn presto_initialize(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            pallet_tools::presto::fill_presto::<T>()?;

            // Extrinsic is only for testing, so we return all fees
            // for simplicity.
            Ok(PostDispatchInfo {
                actual_weight: None,
                pays_fee: Pays::No,
            })
        }

        /// Allows to clear all Presto data (assets, DEX etc.) in testnet without migration.
        ///
        /// Parameters:
        /// - `origin`: Root
        #[pallet::call_index(7)]
        #[pallet::weight(<T as Config>::WeightInfo::presto_clear())]
        pub fn presto_clear(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            pallet_tools::presto::clear_presto::<T>()?;

            // Extrinsic is only for testing, so we return all fees
            // for simplicity.
            Ok(PostDispatchInfo {
                actual_weight: None,
                pays_fee: Pays::No,
            })
        }
    }
}
