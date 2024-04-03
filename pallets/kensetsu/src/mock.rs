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

use common::mock::ExistentialDeposits;
use common::prelude::{QuoteAmount, SwapAmount, SwapOutcome};
use common::{
    balance, mock_assets_config, mock_common_config, mock_currencies_config,
    mock_frame_system_config, mock_pallet_balances_config, mock_pallet_timestamp_config,
    mock_permissions_config, mock_technical_config, mock_tokens_config, Amount, AssetId32,
    AssetInfoProvider, AssetName, AssetSymbol, DEXId, FromGenericPair, LiquidityProxyTrait,
    LiquiditySourceFilter, LiquiditySourceType, PredefinedAssetId, PriceToolsProvider,
    PriceVariant, DAI, DEFAULT_BALANCE_PRECISION, KUSD, XOR, XST,
};
use currencies::BasicCurrencyAdapter;
use frame_support::dispatch::DispatchResult;
use frame_support::traits::{ConstU16, ConstU64, Everything, GenesisBuild, Randomness};
use frame_support::{ensure, parameter_types};
use frame_system::offchain::SendTransactionTypes;
use hex_literal::hex;
use permissions::Scope;
use sp_core::crypto::AccountId32;
use sp_core::{ConstU32, H256};
use sp_runtime::{
    testing::{Header, TestXt},
    traits::{BlakeTwo256, IdentifyAccount, IdentityLookup, Verify},
};
use sp_runtime::{DispatchError, MultiSignature};

type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;
type AssetId = AssetId32<PredefinedAssetId>;
type Balance = u128;
type Block = frame_system::mocking::MockBlock<TestRuntime>;
type BlockNumber = u64;
type Hash = H256;
type Moment = u64;
type Signature = MultiSignature;
type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
type TechAssetId = common::TechAssetId<PredefinedAssetId>;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<TestRuntime>;

pub struct MockRandomness;

impl Randomness<Option<Hash>, BlockNumber> for MockRandomness {
    fn random(_subject: &[u8]) -> (Option<Hash>, BlockNumber) {
        unimplemented!()
    }
}

pub struct MockPriceTools;

impl PriceToolsProvider<AssetId> for MockPriceTools {
    /// Returns `asset_id` price is $1
    fn get_average_price(
        _input_asset_id: &AssetId,
        _output_asset_id: &AssetId,
        _price_variant: PriceVariant,
    ) -> Result<Balance, DispatchError> {
        Ok(balance!(1))
    }

    /// Method not used
    fn register_asset(_asset_id: &AssetId) -> DispatchResult {
        unimplemented!()
    }
}

pub struct MockLiquidityProxy;

impl MockLiquidityProxy {
    const EXCHANGE_TECH_ACCOUNT: AccountId = AccountId32::new([33u8; 32]);

    /// Sets output amount in KUSD, mints this amount to LiquidityProxy account
    /// This amount will be used in the next exchange
    pub fn set_output_amount_for_the_next_exchange(output_amount_kusd: Balance) {
        assets::Pallet::<TestRuntime>::update_balance(
            RuntimeOrigin::root(),
            Self::EXCHANGE_TECH_ACCOUNT,
            KUSD,
            output_amount_kusd.try_into().unwrap(),
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
        if *output_asset_id != KUSD {
            Err(DispatchError::Other(
                "Wrong asset id for MockLiquidityProxy, KUSD only supported",
            ))
        } else {
            let amount =
                assets::Pallet::<TestRuntime>::free_balance(&KUSD, &Self::EXCHANGE_TECH_ACCOUNT)
                    .expect("must succeed");
            Ok(SwapOutcome {
                amount,
                fee: Default::default(),
            })
        }
    }

    /// Mocks liquidity provider exchange
    /// output_asset_id - must be KUSD
    /// output_amount - is equal to KUSD LiquidityProxy tech account balance. This amount might be
    /// set with associated function:
    /// MockLiquidityProxy::set_output_amount_for_the_next_exchange(balance)
    fn exchange(
        _dex_id: DEXId,
        sender: &AccountId,
        receiver: &AccountId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        amount: SwapAmount<common::Balance>,
        _filter: LiquiditySourceFilter<DEXId, LiquiditySourceType>,
    ) -> Result<SwapOutcome<common::Balance, AssetId>, DispatchError> {
        if *output_asset_id != KUSD {
            Err(DispatchError::Other(
                "Wrong asset id for MockLiquidityProxy, KUSD only supported",
            ))
        } else {
            match amount {
                SwapAmount::WithDesiredInput { .. } => unimplemented!(),
                SwapAmount::WithDesiredOutput {
                    desired_amount_out,
                    max_amount_in,
                } => {
                    assets::Pallet::<TestRuntime>::transfer_from(
                        input_asset_id,
                        sender,
                        &Self::EXCHANGE_TECH_ACCOUNT,
                        max_amount_in,
                    )?;
                    let out_amount = assets::Pallet::<TestRuntime>::free_balance(
                        &KUSD,
                        &Self::EXCHANGE_TECH_ACCOUNT,
                    )?;
                    ensure!(
                        desired_amount_out <= out_amount,
                        DispatchError::Other("Not enough liquidity")
                    );
                    assets::Pallet::<TestRuntime>::transfer_from(
                        output_asset_id,
                        &Self::EXCHANGE_TECH_ACCOUNT,
                        receiver,
                        out_amount,
                    )?;
                    Ok(SwapOutcome::new(out_amount, Default::default()))
                }
            }
        }
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
    pub const KusdAssetId: AssetId = KUSD;

    // 1 day
    pub const AccrueInterestPeriod: Moment = 86_400_000;

}

mock_assets_config!(TestRuntime);
mock_pallet_balances_config!(TestRuntime);
mock_common_config!(TestRuntime);
mock_currencies_config!(TestRuntime);
mock_frame_system_config!(TestRuntime);
mock_permissions_config!(TestRuntime);
mock_technical_config!(TestRuntime);
mock_tokens_config!(TestRuntime);
mock_pallet_timestamp_config!(TestRuntime);

impl kensetsu::Config for TestRuntime {
    type RuntimeEvent = RuntimeEvent;
    type Randomness = MockRandomness;
    type AssetInfoProvider = Assets;
    type TreasuryTechAccount = KensetsuTreasuryTechAccountId;
    type KusdAssetId = KusdAssetId;
    type PriceTools = MockPriceTools;
    type LiquidityProxy = MockLiquidityProxy;
    type MaxCdpsPerOwner = ConstU32<100>;
    type MaxRiskManagementTeamSize = ConstU32<100>;
    type AccrueInterestPeriod = AccrueInterestPeriod;
    type UnsignedPriority = ConstU64<100>;
    type UnsignedLongevity = ConstU64<100>;
    type WeightInfo = ();
}

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
                KUSD,
                assets_and_permissions_account_id,
                AssetSymbol(b"KUSD".to_vec()),
                AssetName(b"Kensetsu Stable Dollar".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                0,
                true,
                None,
                None,
            ),
        ],
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
