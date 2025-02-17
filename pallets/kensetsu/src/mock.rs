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

use crate as kensetsu;
use std::collections::BTreeSet;

use common::mock::ExistentialDeposits;
use common::prelude::{QuoteAmount, SwapAmount, SwapOutcome};
use common::{
    balance, mock_assets_config, mock_common_config, mock_currencies_config,
    mock_frame_system_config, mock_kensetsu_config, mock_pallet_balances_config,
    mock_pallet_timestamp_config, mock_permissions_config, mock_technical_config,
    mock_tokens_config, Amount, AssetId32, AssetInfoProvider, AssetName, AssetSymbol, DEXId,
    DataFeed, FromGenericPair, LiquidityProxyTrait, LiquiditySourceFilter, LiquiditySourceType,
    PredefinedAssetId, PriceToolsProvider, PriceVariant, Rate, SymbolName,
    TradingPairSourceManager, DAI, DEFAULT_BALANCE_PRECISION, KARMA, KEN, KGOLD, KUSD, KXOR, TBCD,
    XOR, XST,
};
use currencies::BasicCurrencyAdapter;
use frame_support::dispatch::DispatchResult;
use frame_support::parameter_types;
use frame_support::traits::{GenesisBuild, Randomness};
use frame_system::offchain::SendTransactionTypes;
use permissions::Scope;
use sp_arithmetic::Percent;
use sp_core::crypto::AccountId32;
use sp_core::H256;
use sp_runtime::{
    testing::TestXt,
    traits::{IdentifyAccount, Verify},
};
use sp_runtime::{DispatchError, MultiSignature};

type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;
type AssetId = AssetId32<PredefinedAssetId>;
type Balance = u128;
type Block = frame_system::mocking::MockBlock<TestRuntime>;
type BlockNumber = u64;
type Hash = H256;
type Signature = MultiSignature;
type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
type TechAssetId = common::TechAssetId<PredefinedAssetId>;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<TestRuntime>;

pub struct MockRandomness;

impl Randomness<Hash, BlockNumber> for MockRandomness {
    fn random(_subject: &[u8]) -> (Hash, BlockNumber) {
        unimplemented!()
    }
}

pub struct MockPriceTools;

impl PriceToolsProvider<AssetId> for MockPriceTools {
    fn is_asset_registered(_asset_id: &AssetId) -> bool {
        true
    }

    /// Returns `asset_id` price is $1
    fn get_average_price(
        _input_asset_id: &AssetId,
        _output_asset_id: &AssetId,
        _price_variant: PriceVariant,
    ) -> Result<Balance, DispatchError> {
        Ok(balance!(1))
    }

    /// Mocks asset id registration in PriceToolsProvider.
    fn register_asset(_asset_id: &AssetId) -> DispatchResult {
        Ok(())
    }
}

pub struct MockLiquidityProxy;

impl MockLiquidityProxy {
    const EXCHANGE_TECH_ACCOUNT: AccountId = AccountId32::new([33u8; 32]);

    /// Sets output amount and input amount in any token, mints output amount to
    /// LiquidityProxy account. These amounts will be used in the next exchange.
    pub fn set_amounts_for_the_next_exchange(
        asset_id: AssetId32<PredefinedAssetId>,
        amount: Balance,
    ) {
        assets::Pallet::<TestRuntime>::update_balance(
            RuntimeOrigin::root(),
            Self::EXCHANGE_TECH_ACCOUNT,
            asset_id,
            amount.try_into().unwrap(),
        )
        .expect("must succeed");
    }
}

impl LiquidityProxyTrait<DEXId, AccountId, AssetId> for MockLiquidityProxy {
    /// Mocks liquidity provider quote
    /// output_asset_id must be KUSD
    fn quote(
        _dex_id: DEXId,
        _input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        _amount: QuoteAmount<common::Balance>,
        _filter: LiquiditySourceFilter<DEXId, LiquiditySourceType>,
        _deduce_fee: bool,
    ) -> Result<SwapOutcome<common::Balance, AssetId>, DispatchError> {
        if *output_asset_id != KUSD
            && *output_asset_id != KEN
            && *output_asset_id != KARMA
            && *output_asset_id != TBCD
        {
            Err(DispatchError::Other(
                "Wrong asset id for MockLiquidityProxy::quote, KUSD, KEN, KARMA, TBCD only supported",
            ))
        } else {
            let amount = assets::Pallet::<TestRuntime>::free_balance(
                output_asset_id,
                &Self::EXCHANGE_TECH_ACCOUNT,
            )
            .expect("must succeed");
            Ok(SwapOutcome::new(amount, Default::default()))
        }
    }

    /// Mocks liquidity provider exchange
    /// output_asset_id - must be KUSD
    /// Use MockLiquidityProxy::set_amounts_for_the_next_exchange() prior to this method.
    fn exchange(
        _dex_id: DEXId,
        sender: &AccountId,
        receiver: &AccountId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        _amount: SwapAmount<common::Balance>,
        _filter: LiquiditySourceFilter<DEXId, LiquiditySourceType>,
    ) -> Result<SwapOutcome<common::Balance, AssetId>, DispatchError> {
        if *output_asset_id != KUSD
            && *output_asset_id != KEN
            && *output_asset_id != KARMA
            && *output_asset_id != TBCD
        {
            Err(DispatchError::Other(
                "Wrong asset id for MockLiquidityProxy::exchange, KUSD, KEN, KARMA, TBCD only supported",
            ))
        } else {
            let swap_amount = assets::Pallet::<TestRuntime>::free_balance(
                output_asset_id,
                &Self::EXCHANGE_TECH_ACCOUNT,
            )?;
            assets::Pallet::<TestRuntime>::transfer_from(
                input_asset_id,
                sender,
                &Self::EXCHANGE_TECH_ACCOUNT,
                swap_amount,
            )?;
            assets::Pallet::<TestRuntime>::transfer_from(
                output_asset_id,
                &Self::EXCHANGE_TECH_ACCOUNT,
                receiver,
                swap_amount,
            )?;
            Ok(SwapOutcome::new(swap_amount, Default::default()))
        }
    }
}

pub struct MockOracle;

impl DataFeed<SymbolName, Rate, u64> for MockOracle {
    fn quote(symbol: &SymbolName) -> Result<Option<Rate>, DispatchError> {
        if *symbol == SymbolName::xau() {
            Ok(Some(Rate {
                value: balance!(2500),
                last_updated: 0,
                dynamic_fee: Default::default(),
            }))
        } else {
            Ok(None)
        }
    }

    fn list_enabled_symbols() -> Result<Vec<(SymbolName, u64)>, DispatchError> {
        Ok(vec![(SymbolName::xau(), 0)])
    }

    fn quote_unchecked(_symbol: &SymbolName) -> Option<Rate> {
        unimplemented!();
    }
}

pub struct MockTradingPairSourceManager;

impl TradingPairSourceManager<DEXId, AssetId> for MockTradingPairSourceManager {
    fn list_enabled_sources_for_trading_pair(
        _dex_id: &DEXId,
        _base_asset_id: &AssetId,
        _target_asset_id: &AssetId,
    ) -> Result<BTreeSet<LiquiditySourceType>, DispatchError> {
        unimplemented!();
    }

    fn is_source_enabled_for_trading_pair(
        _dex_id: &DEXId,
        _base_asset_id: &AssetId,
        _target_asset_id: &AssetId,
        _source_type: LiquiditySourceType,
    ) -> Result<bool, DispatchError> {
        Ok(false)
    }

    fn enable_source_for_trading_pair(
        _dex_id: &DEXId,
        _base_asset_id: &AssetId,
        _target_asset_id: &AssetId,
        _source_type: LiquiditySourceType,
    ) -> DispatchResult {
        Ok(())
    }

    fn disable_source_for_trading_pair(
        _dex_id: &DEXId,
        _base_asset_id: &AssetId,
        _target_asset_id: &AssetId,
        _source_type: LiquiditySourceType,
    ) -> DispatchResult {
        unimplemented!();
    }

    fn is_trading_pair_enabled(
        _dex_id: &DEXId,
        _base_asset_id: &AssetId,
        _target_asset_id: &AssetId,
    ) -> Result<bool, DispatchError> {
        Ok(false)
    }

    fn register_pair(
        _dex_id: DEXId,
        _base_asset_id: AssetId,
        _target_asset_id: AssetId,
    ) -> Result<(), DispatchError> {
        Ok(())
    }
}

frame_support::construct_runtime!(
    pub enum TestRuntime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Storage, Event<T>},
        Assets: assets::{Pallet, Call, Storage, Config<T>, Event<T>},
        Balances: pallet_balances::{Pallet, Call, Storage, Event<T>},
        Technical: technical::{Pallet, Call, Config<T>, Event<T>},
        Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
        Tokens: tokens::{Pallet, Call, Config<T>, Storage, Event<T>},
        Permissions: permissions::{Pallet, Call, Config<T>, Storage, Event<T>},
        Kensetsu: kensetsu::{Pallet, Call, Storage, Event<T>, ValidateUnsigned},
    }
);

pub type MockExtrinsic = TestXt<RuntimeCall, ()>;

impl<LocalCall> SendTransactionTypes<LocalCall> for TestRuntime
where
    RuntimeCall: From<LocalCall>,
{
    type Extrinsic = MockExtrinsic;
    type OverarchingCall = RuntimeCall;
}

parameter_types! {
    pub KensetsuDepositoryTechAccountId: TechAccountId = {
        TechAccountId::from_generic_pair(
            kensetsu::TECH_ACCOUNT_PREFIX.to_vec(),
            kensetsu::TECH_ACCOUNT_DEPOSITORY_MAIN.to_vec(),
        )
    };
    pub KensetsuDepositoryAccountId: AccountId = {
        let tech_account_id = KensetsuDepositoryTechAccountId::get();
        technical::Pallet::<TestRuntime>::tech_account_id_to_account_id(&tech_account_id)
                .expect("Failed to get ordinary account id for technical account id.")
    };
    pub KensetsuTreasuryTechAccountId: TechAccountId = {
        TechAccountId::from_generic_pair(
            kensetsu::TECH_ACCOUNT_PREFIX.to_vec(),
            kensetsu::TECH_ACCOUNT_TREASURY_MAIN.to_vec(),
        )
    };
    pub KensetsuTreasuryAccountId: AccountId = {
        let tech_account_id = KensetsuTreasuryTechAccountId::get();
        technical::Pallet::<TestRuntime>::tech_account_id_to_account_id(&tech_account_id)
                .expect("Failed to get ordinary account id for technical account id.")
    };
    pub const KenAssetId: AssetId = KEN;
    pub const KarmaAssetId: AssetId = KARMA;
    pub const TbcdAssetId: AssetId = TBCD;
    pub const GetKenIncentiveRemintPercent: Percent = Percent::from_percent(80);
    pub const GetKarmaIncentiveRemintPercent: Percent = Percent::from_percent(80);
    pub const MinimalStabilityFeeAccrue: Balance = balance!(1);
}

parameter_types! {
    pub const GetBaseAssetId: AssetId = XOR;
    pub const GetBuyBackAssetId: AssetId = XST;
}

mock_assets_config!(TestRuntime);
mock_common_config!(TestRuntime);
mock_currencies_config!(TestRuntime);
mock_frame_system_config!(TestRuntime);
mock_kensetsu_config!(TestRuntime);
mock_pallet_balances_config!(TestRuntime);
mock_pallet_timestamp_config!(TestRuntime);
mock_permissions_config!(TestRuntime);
mock_technical_config!(TestRuntime);
mock_tokens_config!(TestRuntime);

// Builds testing externalities
pub fn new_test_ext() -> sp_io::TestExternalities {
    let assets_and_permissions_tech_account_id =
        TechAccountId::Generic(b"SYSTEM_ACCOUNT".to_vec(), b"ASSETS_PERMISSIONS".to_vec());
    let assets_and_permissions_account_id =
        technical::Pallet::<TestRuntime>::tech_account_id_to_account_id(
            &assets_and_permissions_tech_account_id,
        )
        .unwrap();

    let mut storage = frame_system::GenesisConfig::default()
        .build_storage::<TestRuntime>()
        .unwrap();
    TechnicalConfig {
        register_tech_accounts: vec![
            (
                KensetsuDepositoryAccountId::get(),
                KensetsuDepositoryTechAccountId::get(),
            ),
            (
                KensetsuTreasuryAccountId::get(),
                KensetsuTreasuryTechAccountId::get(),
            ),
            (
                assets_and_permissions_account_id.clone(),
                assets_and_permissions_tech_account_id,
            ),
        ],
    }
    .assimilate_storage(&mut storage)
    .unwrap();

    PermissionsConfig {
        initial_permission_owners: vec![
            (
                permissions::MINT,
                Scope::Unlimited,
                vec![assets_and_permissions_account_id.clone()],
            ),
            (
                permissions::BURN,
                Scope::Unlimited,
                vec![assets_and_permissions_account_id.clone()],
            ),
        ],
        initial_permissions: vec![
            (
                assets_and_permissions_account_id.clone(),
                Scope::Unlimited,
                vec![permissions::MINT, permissions::BURN],
            ),
            (
                KensetsuTreasuryAccountId::get(),
                Scope::Unlimited,
                vec![permissions::MINT, permissions::BURN],
            ),
        ],
    }
    .assimilate_storage(&mut storage)
    .unwrap();
    AssetsConfig {
        endowed_assets: vec![
            (
                XOR,
                assets_and_permissions_account_id.clone(),
                AssetSymbol(b"XOR".to_vec()),
                AssetName(b"SORA".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                0,
                true,
                None,
                None,
            ),
            (
                TBCD,
                assets_and_permissions_account_id.clone(),
                AssetSymbol(b"TBCD".to_vec()),
                AssetName(b"TBCD Token".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                0,
                true,
                None,
                None,
            ),
            (
                DAI,
                assets_and_permissions_account_id.clone(),
                AssetSymbol(b"DAI".to_vec()),
                AssetName(b"DAI".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                0,
                true,
                None,
                None,
            ),
            (
                KEN,
                assets_and_permissions_account_id.clone(),
                AssetSymbol(b"KEN".to_vec()),
                AssetName(b"KEN token".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                0,
                true,
                None,
                None,
            ),
            (
                KARMA,
                assets_and_permissions_account_id.clone(),
                AssetSymbol(b"KARMA".to_vec()),
                AssetName(b"KARMA token".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                0,
                true,
                None,
                None,
            ),
            (
                KUSD,
                assets_and_permissions_account_id.clone(),
                AssetSymbol(b"KUSD".to_vec()),
                AssetName(b"Kensetsu Stable Dollar".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                0,
                true,
                None,
                None,
            ),
            (
                KGOLD,
                assets_and_permissions_account_id.clone(),
                AssetSymbol(b"KGOLD".to_vec()),
                AssetName(b"Kensetsu ounce of gold".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                0,
                true,
                None,
                None,
            ),
            (
                KXOR,
                assets_and_permissions_account_id,
                AssetSymbol(b"KXOR".to_vec()),
                AssetName(b"Kensetsu Stable XOR".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                0,
                true,
                None,
                None,
            ),
        ],
        regulated_assets: Default::default(),
        sbt_assets: Default::default(),
    }
    .assimilate_storage(&mut storage)
    .unwrap();

    let mut ext: sp_io::TestExternalities = storage.into();
    ext.execute_with(|| {
        System::set_block_number(1);
        Timestamp::set_timestamp(0);
    });
    ext
}
