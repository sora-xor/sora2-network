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

use crate::crop_receipt::{crop_receipt_content_template, Country, Rating};
use crate::{Config, Coupons, Event, Pallet};
use codec::Decode;
use common::{balance, AssetIdOf, AssetInfoProvider, BoundedString};
use frame_benchmarking::benchmarks;
use frame_system::{EventRecord, RawOrigin};
use hex_literal::hex;
use sp_core::Get;
use sp_std::vec;
use sp_std::vec::Vec;

type Assets<T> = assets::Pallet<T>;

fn alice<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).unwrap()
}

fn bob<T: Config>() -> T::AccountId {
    let bytes = hex!("8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48");
    T::AccountId::decode(&mut &bytes[..]).unwrap()
}

fn tech_account_id_to_account_id<T: Config>(tech: &T::TechAccountId) -> T::AccountId {
    technical::Pallet::<T>::tech_account_id_to_account_id(tech).unwrap()
}

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
    let events = frame_system::Pallet::<T>::events();
    let system_event: <T as frame_system::Config>::RuntimeEvent = generic_event.into();
    // compare to the last event record
    let EventRecord { event, .. } = &events[events.len() - 1];
    assert_eq!(event, &system_event);
}

benchmarks! {
    where_clause {
        where T: assets::Config<AssetId = AssetIdOf<T>>,
    }

    add_presto_manager {
    }: {
        Pallet::<T>::add_presto_manager(RawOrigin::Root.into(), alice::<T>()).unwrap();
    }
    verify {
        assert_eq!(Pallet::<T>::managers(), vec![alice::<T>()]);
        assert_last_event::<T>(Event::<T>::ManagerAdded { manager: alice::<T>() }.into());
    }

    remove_presto_manager {
        Pallet::<T>::add_presto_manager(RawOrigin::Root.into(), alice::<T>()).unwrap();
    }: {
        Pallet::<T>::remove_presto_manager(RawOrigin::Root.into(), alice::<T>()).unwrap();
    }
    verify {
        assert_eq!(Pallet::<T>::managers(), vec![]);
        assert_last_event::<T>(Event::<T>::ManagerRemoved { manager: alice::<T>() }.into());
    }

    add_presto_auditor {
    }: {
        Pallet::<T>::add_presto_auditor(RawOrigin::Root.into(), alice::<T>()).unwrap();
    }
    verify {
        assert_eq!(Pallet::<T>::auditors(), vec![alice::<T>()]);
        assert_last_event::<T>(Event::<T>::AuditorAdded { auditor: alice::<T>() }.into());
    }

    remove_presto_auditor {
        Pallet::<T>::add_presto_auditor(RawOrigin::Root.into(), alice::<T>()).unwrap();
    }: {
        Pallet::<T>::remove_presto_auditor(RawOrigin::Root.into(), alice::<T>()).unwrap();
    }
    verify {
        assert_eq!(Pallet::<T>::auditors(), vec![]);
        assert_last_event::<T>(Event::<T>::AuditorRemoved { auditor: alice::<T>() }.into());
    }

    mint_presto_usd {
        Pallet::<T>::add_presto_manager(RawOrigin::Root.into(), alice::<T>()).unwrap();
        let amount = balance!(1000);
    }: {
        Pallet::<T>::mint_presto_usd(RawOrigin::Signed(alice::<T>()).into(), amount).unwrap();
    }
    verify {
        assert_eq!(Assets::<T>::free_balance(&T::PrestoUsdAssetId::get(), &tech_account_id_to_account_id::<T>(&T::PrestoTechAccount::get())).unwrap(), amount);
        assert_last_event::<T>(Event::<T>::PrestoUsdMinted { amount, by: alice::<T>() }.into());
    }

    burn_presto_usd {
        Pallet::<T>::add_presto_manager(RawOrigin::Root.into(), alice::<T>()).unwrap();
        let mint_amount = balance!(1000);
        Pallet::<T>::mint_presto_usd(RawOrigin::Signed(alice::<T>()).into(), mint_amount).unwrap();
        let burn_amount = balance!(200);
    }: {
        Pallet::<T>::burn_presto_usd(RawOrigin::Signed(alice::<T>()).into(), burn_amount).unwrap();
    }
    verify {
        assert_eq!(Assets::<T>::free_balance(&T::PrestoUsdAssetId::get(), &tech_account_id_to_account_id::<T>(&T::PrestoTechAccount::get())).unwrap(), mint_amount - burn_amount);
        assert_last_event::<T>(Event::<T>::PrestoUsdBurned { amount: burn_amount, by: alice::<T>() }.into());
    }

    send_presto_usd {
        Pallet::<T>::add_presto_manager(RawOrigin::Root.into(), alice::<T>()).unwrap();
        let mint_amount = balance!(1000);
        Pallet::<T>::mint_presto_usd(RawOrigin::Signed(alice::<T>()).into(), mint_amount).unwrap();
        let send_amount = balance!(200);
    }: {
        Pallet::<T>::send_presto_usd(RawOrigin::Signed(alice::<T>()).into(), send_amount, bob::<T>()).unwrap();
    }
    verify {
        assert_eq!(Assets::<T>::free_balance(&T::PrestoUsdAssetId::get(), &tech_account_id_to_account_id::<T>(&T::PrestoTechAccount::get())).unwrap(), mint_amount - send_amount);
        assert_eq!(Assets::<T>::free_balance(&T::PrestoUsdAssetId::get(), &bob::<T>()).unwrap(), send_amount);
    }

    create_deposit_request {
        let request_id = 1u32.into();
    }: {
        Pallet::<T>::create_deposit_request(RawOrigin::Signed(bob::<T>()).into(), balance!(10000), BoundedString::truncate_from("payment reference"), Some(BoundedString::truncate_from("details"))).unwrap();
    }
    verify {
        assert_last_event::<T>(Event::<T>::RequestCreated { id: request_id, by: bob::<T>() }.into());
    }

    create_withdraw_request {
        Pallet::<T>::add_presto_manager(RawOrigin::Root.into(), alice::<T>()).unwrap();
        Pallet::<T>::mint_presto_usd(RawOrigin::Signed(alice::<T>()).into(), balance!(10000)).unwrap();
        let send_amount = balance!(2000);
        Pallet::<T>::send_presto_usd(RawOrigin::Signed(alice::<T>()).into(), send_amount, bob::<T>()).unwrap();
        let withdraw_amount = balance!(1000);
        let request_id = 1u32.into();
    }: {
        Pallet::<T>::create_withdraw_request(RawOrigin::Signed(bob::<T>()).into(), withdraw_amount, Some(BoundedString::truncate_from("details"))).unwrap();
    }
    verify {
        assert_eq!(Assets::<T>::free_balance(&T::PrestoUsdAssetId::get(), &bob::<T>()).unwrap(), send_amount - withdraw_amount);
        assert_eq!(Assets::<T>::free_balance(&T::PrestoUsdAssetId::get(), &tech_account_id_to_account_id::<T>(&T::PrestoBufferTechAccount::get())).unwrap(), withdraw_amount);
        assert_last_event::<T>(Event::<T>::RequestCreated { id: request_id, by: bob::<T>() }.into());
    }

    cancel_request {
        Pallet::<T>::create_deposit_request(RawOrigin::Signed(bob::<T>()).into(), balance!(10000), BoundedString::truncate_from("payment reference"), Some(BoundedString::truncate_from("details"))).unwrap();
        let request_id = 1u32.into();
    }: {
        Pallet::<T>::cancel_request(RawOrigin::Signed(bob::<T>()).into(), request_id).unwrap();
    }
    verify {
        assert_last_event::<T>(Event::<T>::RequestCancelled { id: request_id }.into());
    }

    approve_deposit_request {
        Pallet::<T>::add_presto_manager(RawOrigin::Root.into(), alice::<T>()).unwrap();
        let mint_amount = balance!(100000);
        Pallet::<T>::mint_presto_usd(RawOrigin::Signed(alice::<T>()).into(), mint_amount).unwrap();
        let deposit_amount = balance!(10000);
        Pallet::<T>::create_deposit_request(RawOrigin::Signed(bob::<T>()).into(), deposit_amount, BoundedString::truncate_from("payment reference"), Some(BoundedString::truncate_from("details"))).unwrap();
        let request_id = 1u32.into();
    }: {
        Pallet::<T>::approve_deposit_request(RawOrigin::Signed(alice::<T>()).into(), request_id).unwrap();
    }
    verify {
        assert_eq!(Assets::<T>::free_balance(&T::PrestoUsdAssetId::get(), &tech_account_id_to_account_id::<T>(&T::PrestoTechAccount::get())).unwrap(), mint_amount - deposit_amount);
        assert_eq!(Assets::<T>::free_balance(&T::PrestoUsdAssetId::get(), &bob::<T>()).unwrap(), deposit_amount);
        assert_last_event::<T>(Event::<T>::RequestApproved { id: request_id, by: alice::<T>() }.into());
    }

    approve_withdraw_request {
        Pallet::<T>::add_presto_manager(RawOrigin::Root.into(), alice::<T>()).unwrap();
        let mint_amount = balance!(10000);
        Pallet::<T>::mint_presto_usd(RawOrigin::Signed(alice::<T>()).into(), mint_amount).unwrap();
        let send_amount = balance!(2000);
        Pallet::<T>::send_presto_usd(RawOrigin::Signed(alice::<T>()).into(), send_amount, bob::<T>()).unwrap();
        let withdraw_amount = balance!(1000);
        let treasury_amount = mint_amount - send_amount;
        Pallet::<T>::create_withdraw_request(RawOrigin::Signed(bob::<T>()).into(), withdraw_amount, Some(BoundedString::truncate_from("details"))).unwrap();
        let request_id = 1u32.into();
    }: {
        Pallet::<T>::approve_withdraw_request(RawOrigin::Signed(alice::<T>()).into(), request_id, BoundedString::truncate_from("payment reference")).unwrap();
    }
    verify {
        assert_eq!(Assets::<T>::free_balance(&T::PrestoUsdAssetId::get(), &tech_account_id_to_account_id::<T>(&T::PrestoTechAccount::get())).unwrap(), treasury_amount + withdraw_amount);
        assert_eq!(Assets::<T>::free_balance(&T::PrestoUsdAssetId::get(), &tech_account_id_to_account_id::<T>(&T::PrestoBufferTechAccount::get())).unwrap(), 0);
        assert_last_event::<T>(Event::<T>::RequestApproved { id: request_id, by: alice::<T>() }.into());
    }

    decline_request {
        Pallet::<T>::add_presto_manager(RawOrigin::Root.into(), alice::<T>()).unwrap();
        Pallet::<T>::create_deposit_request(RawOrigin::Signed(bob::<T>()).into(), balance!(10000), BoundedString::truncate_from("payment reference"), Some(BoundedString::truncate_from("details"))).unwrap();
        let request_id = 1u32.into();
    }: {
        Pallet::<T>::decline_request(RawOrigin::Signed(alice::<T>()).into(), request_id).unwrap();
    }
    verify {
        assert_last_event::<T>(Event::<T>::RequestDeclined { id: request_id, by: alice::<T>() }.into());
    }

    create_crop_receipt {
        let amount = balance!(10000);
        let close_initial_period = 123u32.into();
        let date_of_issue = 234u32.into();
        let place_of_issue = BoundedString::truncate_from("place of issue");
        let debtor = BoundedString::truncate_from("debtor");
        let creditor = BoundedString::truncate_from("creditor");
        let perfomance_time = 345u32.into();
        let data = crop_receipt_content_template::<T>();

        let crop_receipt_id = 1u32.into();
    }: {
        Pallet::<T>::create_crop_receipt(RawOrigin::Signed(bob::<T>()).into(), amount, Country::Brazil, close_initial_period, date_of_issue, place_of_issue, debtor, creditor, perfomance_time, data).unwrap();
    }
    verify {
        assert_last_event::<T>(Event::<T>::CropReceiptCreated { id: crop_receipt_id, by: bob::<T>() }.into());
    }

    rate_crop_receipt {
        Pallet::<T>::add_presto_auditor(RawOrigin::Root.into(), alice::<T>()).unwrap();

        let amount = balance!(10000);
        let close_initial_period = 123u32.into();
        let date_of_issue = 234u32.into();
        let place_of_issue = BoundedString::truncate_from("place of issue");
        let debtor = BoundedString::truncate_from("debtor");
        let creditor = BoundedString::truncate_from("creditor");
        let perfomance_time = 345u32.into();
        let data = crop_receipt_content_template::<T>();
        Pallet::<T>::create_crop_receipt(RawOrigin::Signed(bob::<T>()).into(), amount, Country::Brazil, close_initial_period, date_of_issue, place_of_issue, debtor, creditor, perfomance_time, data).unwrap();

        let crop_receipt_id = 1u32.into();
    }: {
        Pallet::<T>::rate_crop_receipt(RawOrigin::Signed(alice::<T>()).into(), crop_receipt_id, Rating::AA).unwrap();
    }
    verify {
        assert_last_event::<T>(Event::<T>::CropReceiptRated { id: crop_receipt_id, by: alice::<T>() }.into());
    }

    decline_crop_receipt {
        Pallet::<T>::add_presto_auditor(RawOrigin::Root.into(), alice::<T>()).unwrap();

        let amount = balance!(10000);
        let close_initial_period = 123u32.into();
        let date_of_issue = 234u32.into();
        let place_of_issue = BoundedString::truncate_from("place of issue");
        let debtor = BoundedString::truncate_from("debtor");
        let creditor = BoundedString::truncate_from("creditor");
        let perfomance_time = 345u32.into();
        let data = crop_receipt_content_template::<T>();
        Pallet::<T>::create_crop_receipt(RawOrigin::Signed(bob::<T>()).into(), amount, Country::Brazil, close_initial_period, date_of_issue, place_of_issue, debtor, creditor, perfomance_time, data).unwrap();

        let crop_receipt_id = 1u32.into();

        Pallet::<T>::rate_crop_receipt(RawOrigin::Signed(alice::<T>()).into(), crop_receipt_id, Rating::AA).unwrap();
    }: {
        Pallet::<T>::decline_crop_receipt(RawOrigin::Signed(bob::<T>()).into(), crop_receipt_id).unwrap();
    }
    verify {
        assert_last_event::<T>(Event::<T>::CropReceiptDeclined { id: crop_receipt_id }.into());
    }

    publish_crop_receipt {
        Pallet::<T>::add_presto_auditor(RawOrigin::Root.into(), alice::<T>()).unwrap();

        let amount = balance!(10000);
        let close_initial_period = 123u32.into();
        let date_of_issue = 234u32.into();
        let place_of_issue = BoundedString::truncate_from("place of issue");
        let debtor = BoundedString::truncate_from("debtor");
        let creditor = BoundedString::truncate_from("creditor");
        let perfomance_time = 345u32.into();
        let data = crop_receipt_content_template::<T>();
        Pallet::<T>::create_crop_receipt(RawOrigin::Signed(bob::<T>()).into(), amount, Country::Brazil, close_initial_period, date_of_issue, place_of_issue, debtor, creditor, perfomance_time, data).unwrap();

        let crop_receipt_id = 1u32.into();

        Pallet::<T>::rate_crop_receipt(RawOrigin::Signed(alice::<T>()).into(), crop_receipt_id, Rating::AA).unwrap();

        let supply = 1000;
    }: {
        Pallet::<T>::publish_crop_receipt(RawOrigin::Signed(bob::<T>()).into(), crop_receipt_id, supply).unwrap();
    }
    verify {
        let coupon_asset_id = Coupons::<T>::iter().collect::<Vec<_>>().first().unwrap().0;
        assert_last_event::<T>(Event::<T>::CropReceiptPublished { id: crop_receipt_id, coupon_asset_id }.into());
    }

    impl_benchmark_test_suite!(
        Pallet,
        crate::mock::ext(),
        crate::mock::Runtime
    );
}
