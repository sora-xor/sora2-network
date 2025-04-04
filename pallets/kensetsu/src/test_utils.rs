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

use super::*;
use crate::mock::{RuntimeOrigin, TestRuntime};

use common::{AccountIdOf, AssetInfoProvider, Balance, DAI, KUSD, KXOR, XOR};
use frame_support::assert_ok;
use frame_system::pallet_prelude::OriginFor;
use hex_literal::hex;
use sp_arithmetic::{Perbill, Percent};
use sp_runtime::traits::Zero;
use sp_runtime::AccountId32;

type AccountId = AccountId32;
type KensetsuPallet = Pallet<TestRuntime>;
type AssetId = <TestRuntime as assets::Config>::AssetId;

/// Predefined AccountId `Alice`
pub fn alice_account_id() -> AccountId {
    AccountId32::from(hex!(
        "d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"
    ))
}

/// Regular client account Alice
pub fn alice() -> OriginFor<TestRuntime> {
    RuntimeOrigin::signed(alice_account_id())
}

/// Predefined AccountId `Bob`
pub fn bob_account_id() -> AccountId {
    AccountId32::from(hex!(
        "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
    ))
}

/// Regular client account Alice
pub fn bob() -> OriginFor<TestRuntime> {
    RuntimeOrigin::signed(bob_account_id())
}

/// Returns Kensetsu technical treasury account id.
pub fn depository_tech_account_id() -> AccountId {
    let tech_account = <TestRuntime as Config>::DepositoryTechAccount::get();
    technical::Pallet::<TestRuntime>::tech_account_id_to_account_id(&tech_account)
        .expect("Failed to get ordinary account id for technical account id.")
}

/// Returns Kensetsu technical treasury account id.
pub fn treasury_tech_account_id() -> AccountId {
    let tech_account = <TestRuntime as Config>::TreasuryTechAccount::get();
    technical::Pallet::<TestRuntime>::tech_account_id_to_account_id(&tech_account)
        .expect("Failed to get ordinary account id for technical account id.")
}

/// Sets protocol bad debt in KUSD.
pub fn set_bad_debt(bad_debt: Balance) {
    StablecoinInfos::<TestRuntime>::mutate(KUSD, |stablecoin_info| {
        stablecoin_info.as_mut().unwrap().bad_debt = bad_debt;
    });
}

/// Asserts that protocol bad debt is expected amount.
pub fn assert_bad_debt(expected_amount: Balance) {
    assert_eq!(
        StablecoinInfos::<TestRuntime>::get(KUSD).unwrap().bad_debt,
        expected_amount
    );
}

/// Sets protocol borrow tax.
pub fn set_borrow_tax(borrow_tax: Percent) {
    BorrowTax::<TestRuntime>::set(borrow_tax);
}

/// Configures Kensetsu Dollar stablecoin pegged to DAI.
pub fn set_kensetsu_dollar_stablecoin() {
    StablecoinInfos::<TestRuntime>::set::<AssetIdOf<TestRuntime>>(
        KUSD,
        Some(StablecoinInfo {
            bad_debt: 0,
            stablecoin_parameters: StablecoinParameters {
                peg_asset: PegAsset::SoraAssetId(DAI),
                minimal_stability_fee_accrue: balance!(1),
            },
        }),
    );
}

/// Configures Kensetsu Gold stablecoin pegged to XAU.
pub fn set_kensetsu_gold_stablecoin() {
    KensetsuPallet::register_stablecoin(
        RuntimeOrigin::root(),
        StablecoinParameters {
            peg_asset: PegAsset::OracleSymbol(SymbolName::xau()),
            minimal_stability_fee_accrue: balance!(0.01),
        },
    )
    .expect("Must add Kensetsu Gold pegged to XAU")
}

/// Configures KXOR stablecoin pegged to XOR.
pub fn set_kensetsu_xor_stablecoin() {
    StablecoinInfos::<TestRuntime>::set::<AssetIdOf<TestRuntime>>(
        KXOR,
        Some(StablecoinInfo {
            bad_debt: 0,
            stablecoin_parameters: StablecoinParameters {
                peg_asset: PegAsset::SoraAssetId(XOR),
                minimal_stability_fee_accrue: balance!(1),
            },
        }),
    );
}

/// Configures Kensetsu with basic parameters.
///
/// Sets XOR asset id as collateral with default parameters and sets Kensetsu
/// dollar as stablecoin asset. As if it was called `set_stablecoin` and
/// `update_collateral_risk_parameters(XOR, some_info)`.
pub fn configure_kensetsu_dollar_for_xor(
    hard_cap: Balance,
    liquidation_ratio: Perbill,
    stability_fee_rate: FixedU128,
    minimal_collateral_deposit: Balance,
) {
    set_kensetsu_dollar_stablecoin();
    assert_ok!(KensetsuPallet::update_collateral_risk_parameters(
        RuntimeOrigin::root(),
        XOR,
        KUSD,
        CollateralRiskParameters {
            hard_cap,
            max_liquidation_lot: balance!(1000),
            liquidation_ratio,
            stability_fee_rate,
            minimal_collateral_deposit,
        }
    ));
}

/// Configures Kensetsu with basic parameters.
///
/// Sets XOR asset id as collateral with default parameters and sets KXOR as stablecoin asset. As if
/// it was called `set_stablecoin` and `update_collateral_risk_parameters(XOR, some_info)`.
pub fn configure_kxor_for_xor(
    hard_cap: Balance,
    liquidation_ratio: Perbill,
    stability_fee_rate: FixedU128,
    minimal_collateral_deposit: Balance,
) {
    set_kensetsu_xor_stablecoin();
    assert_ok!(KensetsuPallet::update_collateral_risk_parameters(
        RuntimeOrigin::root(),
        XOR,
        KXOR,
        CollateralRiskParameters {
            hard_cap,
            max_liquidation_lot: balance!(1000),
            liquidation_ratio,
            stability_fee_rate,
            minimal_collateral_deposit,
        }
    ));
}

/// Makes CDPs unsafe by changing liquidation ratio.
pub fn make_cdps_unsafe() {
    CollateralInfos::<TestRuntime>::mutate(
        StablecoinCollateralIdentifier {
            collateral_asset_id: XOR,
            stablecoin_asset_id: KUSD,
        },
        |info| {
            if let Some(info) = info.as_mut() {
                info.risk_parameters = CollateralRiskParameters {
                    hard_cap: Balance::MAX,
                    max_liquidation_lot: balance!(1000),
                    liquidation_ratio: Perbill::from_percent(1),
                    stability_fee_rate: FixedU128::zero(),
                    minimal_collateral_deposit: balance!(0),
                }
            }
        },
    );
}

/// Creates CDP with XOR as collateral asset id
pub fn create_cdp_for_xor(
    owner: OriginFor<TestRuntime>,
    collateral: Balance,
    debt: Balance,
) -> CdpId {
    add_balance(alice_account_id(), collateral, XOR);
    assert_ok!(KensetsuPallet::create_cdp(
        owner,
        XOR,
        collateral,
        KUSD,
        debt,
        debt,
        CdpType::Type2
    ));
    NextCDPId::<TestRuntime>::get()
}

/// Deposits to CDP
pub fn deposit_xor_to_cdp(
    owner: OriginFor<TestRuntime>,
    cdp_id: CdpId,
    collateral_amount: Balance,
) {
    add_balance(alice_account_id(), collateral_amount, XOR);
    assert_ok!(KensetsuPallet::deposit_collateral(
        owner,
        cdp_id,
        collateral_amount
    ));
}

/// Adds to account balance
pub fn add_balance(account: AccountId, balance: Balance, asset_id: AssetId) {
    assert_ok!(assets::Pallet::<TestRuntime>::update_balance(
        RuntimeOrigin::root(),
        account,
        asset_id,
        balance.try_into().unwrap()
    ));
}

/// Returns total supply for asset.
pub fn get_total_supply(asset_id: &AssetId) -> Balance {
    <TestRuntime as pallet::Config>::AssetInfoProvider::total_issuance(asset_id)
        .expect("Must succeed")
}

/// Asserts account balance is expected.
pub fn assert_balance(account: &AccountId, asset_id: &AssetId, expected: Balance) {
    assert_eq!(
        assets::Pallet::<TestRuntime>::free_balance(asset_id, account).unwrap(),
        expected
    );
}

/// Returns CDP ids where the account id is owner
pub fn get_account_cdp_ids(account_id: &AccountIdOf<TestRuntime>) -> Vec<CdpId> {
    CDPDepository::<TestRuntime>::iter()
        .filter(|(_, cdp)| cdp.owner == *account_id)
        .map(|(cdp_id, _)| cdp_id)
        .collect()
}
