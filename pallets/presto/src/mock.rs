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

#![cfg(feature = "wip")] // presto

use crate as presto;

use common::mock::ExistentialDeposits;
use common::{
    mock_assets_config, mock_common_config, mock_currencies_config, mock_frame_system_config,
    mock_pallet_balances_config, mock_pallet_timestamp_config, mock_permissions_config,
    mock_technical_config, mock_tokens_config, Amount, AssetId32, AssetName, AssetSymbol,
    BoundedString, DEXId, FromGenericPair, PredefinedAssetId, DEFAULT_BALANCE_PRECISION, KUSD,
    PRUSD, XOR,
};
use currencies::BasicCurrencyAdapter;
use frame_support::traits::{ConstU32, GenesisBuild};
use frame_support::{construct_runtime, parameter_types};
use permissions::Scope;
use sp_runtime::AccountId32;

pub type AccountId = AccountId32;
pub type AssetId = AssetId32<PredefinedAssetId>;
type Balance = u128;
type BlockNumber = u64;
pub type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
type TechAssetId = common::TechAssetId<PredefinedAssetId>;
type Block = frame_system::mocking::MockBlock<Runtime>;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;

construct_runtime! {
    pub enum Runtime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        Tokens: tokens::{Pallet, Call, Config<T>, Storage, Event<T>},
        Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
        Currencies: currencies::{Pallet, Call, Storage},
        Assets: assets::{Pallet, Call, Config<T>, Storage, Event<T>},
        Balances: pallet_balances::{Pallet, Call, Storage, Event<T>},
        Technical: technical::{Pallet, Call, Config<T>, Storage, Event<T>},
        Permissions: permissions::{Pallet, Call, Config<T>, Storage, Event<T>},
        Presto: presto::{Pallet, Call, Storage, Event<T>},
    }
}

parameter_types! {
    pub const GetBaseAssetId: AssetId = XOR;
    pub const GetBuyBackAssetId: AssetId = KUSD;
}

mock_common_config!(Runtime);
mock_assets_config!(Runtime);
mock_currencies_config!(Runtime);
mock_tokens_config!(Runtime);
mock_pallet_balances_config!(Runtime);
mock_frame_system_config!(Runtime);
mock_technical_config!(Runtime);
mock_pallet_timestamp_config!(Runtime);
mock_permissions_config!(Runtime);

parameter_types! {
    pub const PrestoUsdAssetId: AssetId = PRUSD;
    pub PrestoTechAccountId: TechAccountId = {
        TechAccountId::from_generic_pair(
            presto::TECH_ACCOUNT_PREFIX.to_vec(),
            presto::TECH_ACCOUNT_MAIN.to_vec(),
        )
    };
    pub PrestoAccountId: AccountId = {
        let tech_account_id = PrestoTechAccountId::get();
        technical::Pallet::<Runtime>::tech_account_id_to_account_id(&tech_account_id).unwrap()
    };
    pub PrestoBufferTechAccountId: TechAccountId = {
        TechAccountId::from_generic_pair(
            presto::TECH_ACCOUNT_PREFIX.to_vec(),
            presto::TECH_ACCOUNT_BUFFER.to_vec(),
        )
    };
    pub PrestoBufferAccountId: AccountId = {
        let tech_account_id = PrestoBufferTechAccountId::get();
        technical::Pallet::<Runtime>::tech_account_id_to_account_id(&tech_account_id).unwrap()
    };
}

impl presto::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type PrestoUsdAssetId = PrestoUsdAssetId;
    type PrestoTechAccount = PrestoTechAccountId;
    type PrestoBufferTechAccount = PrestoBufferTechAccountId;
    type RequestId = u64;
    type CropReceiptId = u64;
    type MaxPrestoManagersCount = ConstU32<100>;
    type MaxPrestoAuditorsCount = ConstU32<100>;
    type MaxUserRequestCount = ConstU32<65536>;
    type MaxUserCropReceiptCount = ConstU32<65536>;
    type MaxRequestPaymentReferenceSize = ConstU32<100>;
    type MaxRequestDetailsSize = ConstU32<200>;
    type MaxPlaceOfIssueSize = ConstU32<100>;
    type MaxDebtorSize = ConstU32<80>;
    type MaxCreditorSize = ConstU32<80>;
    type MaxCropReceiptContentSize = ConstU32<30720>;
    type Time = Timestamp;
    type WeightInfo = ();
}

pub fn ext() -> sp_io::TestExternalities {
    let assets_and_permissions_tech_account_id =
        TechAccountId::Generic(b"SYSTEM_ACCOUNT".to_vec(), b"ASSETS_PERMISSIONS".to_vec());
    let assets_and_permissions_account_id =
        technical::Pallet::<Runtime>::tech_account_id_to_account_id(
            &assets_and_permissions_tech_account_id,
        )
        .unwrap();

    let mut storage = frame_system::GenesisConfig::default()
        .build_storage::<Runtime>()
        .unwrap();

    TechnicalConfig {
        register_tech_accounts: vec![
            (PrestoAccountId::get(), PrestoTechAccountId::get()),
            (
                PrestoBufferAccountId::get(),
                PrestoBufferTechAccountId::get(),
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
        initial_permissions: vec![(
            assets_and_permissions_account_id.clone(),
            Scope::Unlimited,
            vec![permissions::MINT, permissions::BURN],
        )],
    }
    .assimilate_storage(&mut storage)
    .unwrap();

    AssetsConfig {
        endowed_assets: vec![
            (
                XOR,
                assets_and_permissions_account_id,
                AssetSymbol(b"XOR".to_vec()),
                AssetName(b"SORA".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                0,
                true,
                None,
                None,
            ),
            (
                PRUSD,
                PrestoAccountId::get(),
                AssetSymbol(b"PRUSD".to_vec()),
                AssetName(b"Presto USD".to_vec()),
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

pub fn crop_receipt_content_template(
) -> BoundedString<<Runtime as presto::Config>::MaxCropReceiptContentSize> {
    let content = r#"{
    "section_1": [
        {
            "title": "SECTION I. BASIC CONDITIONS"
        },
        {
            "title": "1. The Crop Receipt underlying agreement:",
            "value": "This Crop Receipt is issued by the Debtor [as a performance security of] obligations under the Agreement [specify name / type of contract] № [specify contract number] of [specify contract date] concluded between the Debtor and the Creditor."
        },
        {
            "title": "2. Unconditional Obligation:",
            "value": "This Crop Receipt establishes unconditional obligation of the Debtor, secured by the collateral [and surety], to pay the Creditor the amount calculated according to the formula given in Appendix №1."
        },
        {
            "title": "3. Time of performance:",
            "value": "[insert date]"
        },
        {
            "title": "4. Terms and Place of Performance:",
            "value": "The Debtor shall pay funds as performance of the unconditional obligation under the Crop Receipt (hereinafter - 'Payment') by transferring funds in non-cash form in ] to the Creditor’s bank account ], indicated by the Creditor in writing. Other payments under this Crop Receipt are made in the same way.\n\n[On the date of issuance of this Crop Receipt, the Creditor indicated the following bank account: [indicate the details of the Creditor's bank account, if no assignment of the Creditor’s rights under this crop receipt is envisaged]].\n\nAll service fees charged by the Debtor's bank [and intermediary bank (if any)] during such funds transfer shall be covered by the Debtor."
        },
        {
            "title": "5. Pledge",
            "value": "To secure its obligations under the Crop Receipt, the Debtor pledges (hereinafter - 'Pledge') the future harvest [name the farm produce used as a pledge] (hereinafter - 'Subject of Pledge') grown on land plots located in [specify district name] district of [] , [[owned by the Debtor by virtue of [document certifying ownership title to the land]] and / or [leased or otherwise used by the Debtor on the basis of lease agreements or other transactions]], (hereinafter - 'the Land Plots').\n\nOn the day of harvest, the subject of the Pledge becomes the corresponding volume of harvested farm produce in the total amount of not less [specify number] [specify units of measurement - tons / kilogram / liters / pieces / heads / other]], grown on the Land Plots.\n\n[The Pledge shall also apply to the future harvest of other farm produce grown or to be grown by the Debtor on the Land, as well as all harvested farm produce grown on the Land Plots.]\n\nOn the date of issuance of this Crop Receipt, the value of the Subject of Pledge is estimated at ….. [indicate the value of the future harvest excluding VAT], without VAT. "
        },
        {
            "title": "6. Secured Claims",
            "value": "The Pledge secures all Creditor's claims to the Debtor to fulfill each and all of its payment obligations under the Crop Receipt in such amount, in such currency, within such term and in such order as established in the Crop Receipt (hereinafter - 'Secured Claims'). The Creditor’s claims secured by the Pledge include (but are not limited to) the following:\n\n(i) Payment under the Crop Receipt as provided in paragraphs [2] (Unconditional obligation), [3] (Term of performance) and [4] (Terms and place of performance) specified above;\n\n(ii) [reimbursement of the Creditor's expenses to complete crop growing works and / or eliminate breaches in the farming technology for producing agricultural produce, as provided in paragraph [13] (Completion of Future Crops Production) below;\n\n(iii) reimbursement of the Creditor's expenses related to obtaining the notary enforcement writ, as specified in paragraph [26] (Expenses for the Notary Enforcement Writ) below;\n\n(iv)] payment of penalties, as provided by current legislation [and in paragraph [22] (Penalties) below].\n\nTerm of Fulfillment of the Obligation: Each Secured Claim has a term established for it by the relevant provisions of the Crop Receipt.\n\nSecured Claims Value: The Pledge secures the full value of the Secured Claims, regardless of what this value may be at any time during the term of this Crop Receipt. Without limiting the above, on the date of issuance of this Crop Receipt, the value of the Secured Claims is equal to [  ] (hereinafter - the 'Estimated Value'), which is the value of the right of claim under this Crop Receipt on the day of its issuance for accounting purposes."
        },
        {
            "title": "7. Endorsement",
            "value": "Creditor is [not] entitled to assign its rights under the Crop Receipt [without the prior written consent of the Debtor] / [in the absence / presence of the following conditions: ________________________] to any third party [(except [specify the name of the specific individual or legal entity to whom the rights can be assigned])] by making an Inscription of Assignment. Change of the Debtor under the Crop Receipt is not allowed."
        }
    ],
    "section_2": [
        {
            "title": "SECTION II. ADDITIONAL CONDITIONS"
        },
        {
            "title": "8. Debtor's Guarantees:",
            "value": "The Debtor represents, acknowledges, and guarantees that:\n(i) The future harvest of agricultural produce, which is the Subject of the Pledge under the Crop Receipt, is not alienated or encumbered in any way, including under other Crop receipts, and is not disputed (including in court); is not seized (arrested) or bailed, and no third parties have any interest in it.\n(ii) The Subject of the Pledge is not encumbered by any debts or obligations [except as a security against fulfillment by the Debtor's obligations under previously issued Crop receipts, namely: [provide details]].\n(iii) The Debtor has the right [of ownership and/or use] to the Land Plots and is entitled to issue this Crop Receipt.\n(iv) The Debtor is not aware of any disputes concerning their rights to use the Land Plots for growing agricultural produce.\n(v) The Creditor and/or their representatives will have unimpeded access to the places of growing and storage of the Subject of Pledge."
        },
        {
            "title": "9. Insurance:",
            "value": "The Debtor is [obliged/entitled] at its own expense to insure in favor of the Creditor the future harvest and/or the harvested crop, which is the Subject of Pledge, having previously agreed in writing with the Creditor the list of insurance cases and the insurance company."
        },
        {
            "title": "10. Production Technology:",
            "value": "The Debtor undertakes to comply with the agrotechnology specified in Annex №3 for growing produce on the Land Plots. In the event of a dispute regarding compliance with the growing technology, the Debtor and Creditor will seek [recommendation/dispute resolution] from [designated person or agreed authority]."
        },
        {
            "title": "11. Marketing Year:",
            "value": "The marketing year of the produce that is the Subject of Pledge begins on [specify date and month] and lasts for/until [one calendar year/other time period]."
        },
        {
            "title": "12. Monitoring:",
            "value": "Monitoring of the Subject of Pledge is conducted by the Creditor as per Article 8 of the Law during [[the entire term of this Crop Receipt]/[other specified term]]."
        },
        {
            "title": "13. Completion of Future Crops Production:",
            "value": "If the Creditor completes growing the future produce, as per Article 8 of the Law, the Debtor reimburses the Creditor all documented costs for such production (e.g., harvesting, transportation, storage) within [ten days] of receiving written notification from the Creditor."
        },
        {
            "title": "14. Harvesting and Storage:",
            "value": "The Debtor is [not] obliged to notify the Creditor in writing of their intention to start harvesting from the Land Plots and the place of storage for the harvested crops [no later than [three] calendar days before harvesting]. Unless agreed otherwise, the harvested produce shall be stored at the address in Annex №3."
        },
        {
            "title": "15. Interference/Obstruction of Business Activity:",
            "value": "The Debtor acknowledges that the following Creditor actions do not constitute interference:\n(i) Suspension of unauthorized harvesting by the Debtor without prior notice, as outlined in paragraph [14].\n(ii) Suspension of transportation/storage of harvested crops without prior written consent of the Creditor."
        },
        {
            "title": "16. Crop Loss:",
            "value": "In case of crop loss, defined as yield below [10%] of expected harvest (Annex №1) or [30%] fewer plants per hectare, consequences under Article 7 of the Law apply."
        },
        {
            "title": "17. Insufficient Crops:",
            "value": "If crops are insufficient for fulfilling the Debtor's obligations, consequences under [Article 7 of the Law]/[Debtor and Creditor agreement] apply."
        },
        {
            "title": "18. Partial Settlement:",
            "value": "Partial settlement of the Crop Receipt is [not] allowed. Minimum partial settlement amount: [not applicable]/[not established]/[__% of Estimated Value]."
        },
        {
            "title": "19. Alienation of the Subject of Pledge:",
            "value": "The Debtor may not alienate (sell, transfer, exchange, etc.) the Subject of Pledge without the written consent of the Creditor until obligations are fully settled."
        },
        {
            "title": "20. Subsequent Pledge:",
            "value": "Subsequent pledge of future crops or other crops from the Land Plots is [not] allowed without prior written consent of the Creditor until obligations are fully settled."
        },
        {
            "title": "21. Presumptions:",
            "value": "All agricultural produce of the Debtor is presumed to be harvested from the Land Plots unless proven otherwise."
        },
        {
            "title": "22. Penalties:",
            "value": "Various penalties for non-performance, unauthorized actions, and delays are detailed, including penalties for late payments, unauthorized harvesting, and transportation/storage violations."
        },
        {
            "title": "23. Additional Pledge:",
            "value": "Additional collateral provided by the Debtor (if any) is arranged via a separate agreement."
        },
        {
            "title": "24. Additional Obligations of the Debtor:",
            "value": "After harvesting, the Debtor shall:\n(i) Obtain a quality certificate for the Subject of Pledge.\n(ii) Ensure safekeeping of the Subject of Pledge at the designated storage.\n(iii) Cover all storage-related costs."
        },
        {
            "title": "25. Guarantee:",
            "value": "Obligations are [not] secured by a financial institution guarantee."
        },
        {
            "title": "26. Expenses for the Notary's Enforcement Writ:",
            "value": "The Debtor reimburses Creditor's documented expenses for obtaining an enforcement writ within [ten days] of notification."
        },
        {
            "title": "27. Taxes:",
            "value": "Payments to a non-resident Creditor are subject to withholding tax as per legislation and treaties."
        },
        {
            "title": "28. Other:",
            "value": "[Additional conditions, if any.]"
        }
    ],
    "section_3": [
        {
            "title": "SECTION III. FINAL PROVISIONS"
        },
        {
            "title": "29. Advisal of Rights",
            "value": "By signing this Crop Receipt [each] signatory acknowledges that he/she has read and understood all the provisions of this Crop Receipt, and legal consequences of this document, that is corresponds to his/her real intentions, he/she has no objections to any part of this Crop Receipt, the content of which corresponds to the current legislation and contains all information on the relationship between the Debtor [and Guarantor] and the Creditor; the notary explained the meaning of signing this Crop Receipt and all provisions of the current legislation governing the relations under crop receipts; and that for certification of this Crop Receipt all relevant documents, which are of their final and current version have been made available."
        },
        {
            "title": "30. Signature",
            "value": "By signing the Crop Receipt the Debtor [and Guarantor] acknowledges that the issuance of the Crop Receipt reflects his/her free will, the Debtor [and Guarantor] is not under the influence of difficult circumstances, is not mistaken with regard to any circumstances of significance (nature of the Crop Receipt, rights and obligations of the Debtor [and Guarantor] and the Creditor, other conditions of the Crop Receipt), and that provisions of the Crop Receipt are favorable for him/her. The Debtor [and Guarantor] accepts the risk of non-fulfilling the provisions of the Crop Receipt due to significant change in the circumstances that the Debtor [and Guarantor] relied upon while issuing this Crop Receipt. On behalf of the Debtor this Crop Receipt is signed by his/her [manager / representative / other authorized signatory] ___________________, which acts on the basis [statute / power of attorney / other document: ____________], whose identity has been established and the authority verified. [On behalf of the Guarantor, this Crop Receipt is signed by his/her [manager / representative/ other authorized signatory] _______________________, acting on the basis of [statute / power of attorney/ other document: ____________], whose identity has been established and authority verified.]"
        },
        {
            "title": "31. Amendments",
            "value": "Unless otherwise provided by the legislation of XXX in effect at that time, the relations arising under this Crop Receipt may be amended by closing a separate agreement between the Debtor [and Guarantor] and the Creditor and recording these changes in the Crop receipts register."
        },
        {
            "title": "32. Registrations in Registers",
            "value": "The Crop Receipt shall be considered issued from the date of its registration in the Crop Receipts Register and is valid until its full settlement. Information on the Pledge under the Crop Receipt shall be recorded in the State Register of Encumbrances on Movable Property."
        },
        {
            "title": "33. Registration Costs",
            "value": "The costs associated with issuing this Crop Receipt are paid by [[the Debtor] / [the Creditor] / [Debtor and Creditor in equal portions] / [Debtor and Creditor in the following ratio: ___% - Debtor and ___% - Creditor] – choose what applies]."
        },
        {
            "title": "34. Settlement of the Crop Receipt",
            "value": "By signing this Crop Receipt, the Debtor certifies that he/she has informed the Creditor of the latter's obligation, stipulated by Article 12 of the Law, to make the inscription 'Settled' on the Crop Receipt confirmed by the signature and seal of the Creditor, and to return this Crop Receipt to the Debtor within 3 (three) working days upon receipt from the Debtor [and / or Guarantor] of the full settlement of obligations under this Crop Receipt; the Creditor has been also notified on the consequences of non-fulfillment of this obligation according to Article 13 of the Law."
        },
        {
            "title": "35. Language and Copies",
            "value": "The Crop Receipt is issued in the Ukrainian language in two copies having equal legal force, one of which is intended to be kept within files of the case _____________ by [public / private] notary [name of the state notary office] __________ notarial district, and the other copy is given to the Creditor."
        }
    ]
}"#;

    BoundedString::truncate_from(content)
}
