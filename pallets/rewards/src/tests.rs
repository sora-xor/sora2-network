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

use common::{
    assert_approx_eq_abs, assert_noop_msg, balance, AssetId32, AssetInfoProvider, Balance,
    PredefinedAssetId, PSWAP, VAL,
};
use frame_support::assert_noop;
use frame_support::assert_ok;
use frame_support::traits::Hooks;
use hex_literal::hex;

use crate::mock::*;
use crate::{EthAddress, RewardInfo};
use orml_traits::MultiCurrency;

type Pallet = crate::Pallet<Runtime>;
type Error = crate::Error<Runtime>;
type Assets = assets::Pallet<Runtime>;

type ValOwners = crate::ValOwners<Runtime>;
type PswapFarmOwners = crate::PswapFarmOwners<Runtime>;
type PswapWaifuOwners = crate::PswapWaifuOwners<Runtime>;
type UmiNftReceivers = crate::UmiNftReceivers<Runtime>;
type UmiNftClaimed = crate::UmiNftClaimed<Runtime>;
type UmiNfts = crate::UmiNfts<Runtime>;
type EthAddresses = crate::EthAddresses<Runtime>;
type TotalValRewards = crate::TotalValRewards<Runtime>;
type ValBurnedSinceLastVesting = crate::ValBurnedSinceLastVesting<Runtime>;
type CurrentClaimableVal = crate::CurrentClaimableVal<Runtime>;
type TotalClaimableVal = crate::TotalClaimableVal<Runtime>;

fn account() -> AccountId {
    hex!("f08879dab4530529153a1bdb63e27cd3be45f1574a122b7e88579b6e5e60bd43").into()
}

fn assert_reward_info_approx(actual: RewardInfo, claimable: Balance, total: Balance) {
    assert_approx_eq_abs!(actual.claimable, claimable, balance!(0.000000001));
    assert_approx_eq_abs!(actual.total, total, balance!(0.000000001));
}

fn origin() -> RuntimeOrigin {
    RuntimeOrigin::signed(account())
}

#[test]
fn claim_fails_signature_invalid() {
    ExtBuilder::with_rewards(true).build().execute_with(|| {
        assert_noop!(
            Pallet::claim(
                origin(),
                hex!("bb7009c977888910a96d499f802e4524a939702aa6fc8ed473829bffce9289d850b97a720aa05d4a7e70e15733eeebc4fe862dcb").into(),
            ),
            Error::SignatureInvalid
        );
    });
}

#[test]
fn claim_succeeds_zero_v() {
    let account_id: AccountId =
        hex!("7c0f877cd5720eee40d1183556f1fbd34931a6ee08c5299b4de2b2b43176831a").into();
    ExtBuilder::with_rewards(true).build().execute_with(|| {
        let signature = hex!("22bea4c62999dc1be10cb603956b5731dfd296c9e0b0040e5fe8056db1e8df5648c519b704acdcdcf0d04ab01f81f2ed899edef437a4be8f36980d7f1119d7ce00").into();
        assert_ok!(Pallet::claim(RuntimeOrigin::signed(account_id.clone()), signature));
        assert_eq!(
            Assets::free_balance(&PSWAP, &account_id).unwrap(),
            balance!(100)
        );
    });
}

#[test]
fn claim_succeeds() {
    ExtBuilder::with_rewards(true).build().execute_with(|| {
        let signature = hex!("eb7009c977888910a96d499f802e4524a939702aa6fc8ed473829bffce9289d850b97a720aa05d4a7e70e15733eeebc4fe862dcb60e018c0bf560b2de013078f1c").into();
        assert_ok!(Pallet::claim(origin(), signature));
        assert_eq!(
            Assets::free_balance(&VAL, &account()).unwrap(),
            balance!(111)
        );
        assert_eq!(
            Assets::free_balance(&PSWAP, &account()).unwrap(),
            balance!(555)
        );
    });
}

#[test]
fn claim_fails_nothing_to_claim() {
    ExtBuilder::with_rewards(true).build().execute_with(|| {
        let signature: Vec<u8> = hex!("eb7009c977888910a96d499f802e4524a939702aa6fc8ed473829bffce9289d850b97a720aa05d4a7e70e15733eeebc4fe862dcb60e018c0bf560b2de013078f1c").into();
        assert_ok!(Pallet::claim(origin(), signature.clone()));
        assert_noop!(Pallet::claim(origin(), signature), Error::NothingToClaim);
    });
}

#[test]
fn claim_fails_no_rewards() {
    ExtBuilder::with_rewards(true).build().execute_with(|| {
        let signature = hex!("6619441577e5173239a52ee52cc7d2eaf57b294defeb0a564e11c4e3c197a95574d81bd4bc747976c1e163be5adecf6bc6ceff69ef3ee2948ff90fdcaa02d5411c").into();
        assert_noop!(Pallet::claim(origin(), signature), Error::AddressNotEligible);
    });
}

#[test]
fn claim_over_limit() {
    ExtBuilder::with_rewards(true).build().execute_with(|| {
        let signature = hex!("20994c1a98b6818832555f5ab840ef6c7d468f46e192bed4921724629475975f440582a9f1416ffd7720538d30af601cbe18ffded8e0eea38c18d24714b57e381b").into();
        assert_noop_msg!(Pallet::claim(origin(), signature), "BalanceTooLow");
    });
}

#[test]
fn claim_rolls_back_earlier_rewards_when_later_reward_transfer_fails() {
    ExtBuilder::with_rewards(true).build().execute_with(|| {
        let eth_address = EthAddress::from(hex!("21Bc9f4a3d9Dc86f142F802668dB7D908cF0A636"));
        let signature: Vec<u8> = hex!("eb7009c977888910a96d499f802e4524a939702aa6fc8ed473829bffce9289d850b97a720aa05d4a7e70e15733eeebc4fe862dcb60e018c0bf560b2de013078f1c").into();
        let val_before = ValOwners::get(eth_address);
        let farm_before = PswapFarmOwners::get(eth_address);
        let waifu_before = balance!(2000);
        let total_val_before = TotalValRewards::get();
        let total_claimable_before = TotalClaimableVal::get();
        PswapWaifuOwners::insert(eth_address, waifu_before);

        assert_noop_msg!(Pallet::claim(origin(), signature), "BalanceTooLow");

        assert_eq!(Assets::free_balance(&VAL, &account()).unwrap(), 0);
        assert_eq!(Assets::free_balance(&PSWAP, &account()).unwrap(), 0);
        assert_eq!(ValOwners::get(eth_address), val_before);
        assert_eq!(PswapFarmOwners::get(eth_address), farm_before);
        assert_eq!(PswapWaifuOwners::get(eth_address), waifu_before);
        assert_eq!(TotalValRewards::get(), total_val_before);
        assert_eq!(TotalClaimableVal::get(), total_claimable_before);
    });
}

#[test]
fn can_add_umi_nft_receiver() {
    ExtBuilder::with_rewards(true).build().execute_with(|| {
        let addresses = vec![EthAddress::from(hex!(
            "baf5777f2250ec5e294b6f3dee28fcefad607975"
        ))];
        assert!(UmiNftReceivers::get(addresses[0]).is_empty());

        assert_ok!(Pallet::add_umi_nft_receivers(
            RuntimeOrigin::root(),
            addresses.clone()
        ));

        assert!(!UmiNftReceivers::get(addresses[0]).is_empty());
    });
}

#[test]
fn umi_nft_claim_ignores_zero_trailing_entries() {
    ExtBuilder::with_rewards(true).build().execute_with(|| {
        let address = EthAddress::from(hex!("3c52e573fd320153013f40b817dda4f9d648613c"));
        let signature = hex!("5615253e3998c99cd9008baf9c471d7a8f5690bb35a40f872b7cbbf19bad616d4490fc78a84d4568673cf397243ac79eb1684a7e54440f862aecebb54c10474f1c");
        let umi_nfts: Vec<AssetId32<PredefinedAssetId>> = vec![PSWAP, VAL.into()];
        UmiNfts::put(umi_nfts);
        UmiNftReceivers::insert(address, vec![1, 0]);

        assert_ok!(Pallet::claim(origin(), signature.into()));

        assert_eq!(currencies::Pallet::<Runtime>::free_balance(PSWAP.into(), &account()), 1);
        assert_eq!(currencies::Pallet::<Runtime>::free_balance(VAL.into(), &account()), 0);
        assert_eq!(UmiNftReceivers::get(address), vec![0, 0]);
        assert!(UmiNftClaimed::get(address));
    });
}

#[test]
fn umi_nft_claim_all_zero_entries_does_not_mark_claimed() {
    ExtBuilder::with_rewards(true).build().execute_with(|| {
        let address = EthAddress::from(hex!("3c52e573fd320153013f40b817dda4f9d648613c"));
        let signature = hex!("5615253e3998c99cd9008baf9c471d7a8f5690bb35a40f872b7cbbf19bad616d4490fc78a84d4568673cf397243ac79eb1684a7e54440f862aecebb54c10474f1c");
        let umi_nfts: Vec<AssetId32<PredefinedAssetId>> = vec![PSWAP, VAL.into()];
        UmiNfts::put(umi_nfts);
        UmiNftReceivers::insert(address, vec![0, 0]);

        assert_noop!(
            Pallet::claim(origin(), signature.into()),
            Error::NothingToClaim
        );

        assert_eq!(currencies::Pallet::<Runtime>::free_balance(PSWAP.into(), &account()), 0);
        assert_eq!(currencies::Pallet::<Runtime>::free_balance(VAL.into(), &account()), 0);
        assert_eq!(UmiNftReceivers::get(address), vec![0, 0]);
        assert!(!UmiNftClaimed::get(address));
    });
}

#[test]
fn umi_nft_claim_rejects_receiver_entry_without_configured_nft() {
    ExtBuilder::with_rewards(true).build().execute_with(|| {
        let address = EthAddress::from(hex!("3c52e573fd320153013f40b817dda4f9d648613c"));
        let signature = hex!("5615253e3998c99cd9008baf9c471d7a8f5690bb35a40f872b7cbbf19bad616d4490fc78a84d4568673cf397243ac79eb1684a7e54440f862aecebb54c10474f1c");
        let umi_nfts: Vec<AssetId32<PredefinedAssetId>> = vec![PSWAP];
        UmiNfts::put(umi_nfts);
        UmiNftReceivers::insert(address, vec![0, 1]);

        assert_noop!(
            Pallet::claim(origin(), signature.into()),
            Error::InvalidUmiNftStorage
        );

        assert_eq!(currencies::Pallet::<Runtime>::free_balance(PSWAP.into(), &account()), 0);
        assert_eq!(currencies::Pallet::<Runtime>::free_balance(VAL.into(), &account()), 0);
        assert_eq!(UmiNftReceivers::get(address), vec![0, 1]);
        assert!(!UmiNftClaimed::get(address));
    });
}

#[test]
fn umi_nft_claim_rolls_back_first_transfer_if_later_nft_transfer_fails() {
    ExtBuilder::with_rewards(true).build().execute_with(|| {
        let address = EthAddress::from(hex!("3c52e573fd320153013f40b817dda4f9d648613c"));
        let signature = hex!("5615253e3998c99cd9008baf9c471d7a8f5690bb35a40f872b7cbbf19bad616d4490fc78a84d4568673cf397243ac79eb1684a7e54440f862aecebb54c10474f1c");
        let umi_nfts: Vec<AssetId32<PredefinedAssetId>> = vec![PSWAP, VAL.into()];
        UmiNfts::put(umi_nfts);
        UmiNftReceivers::insert(address, vec![1, Balance::MAX]);
        let reserve_pswap_before =
            currencies::Pallet::<Runtime>::free_balance(PSWAP.into(), &account_id());
        let reserve_val_before =
            currencies::Pallet::<Runtime>::free_balance(VAL.into(), &account_id());

        assert_noop_msg!(Pallet::claim(origin(), signature.into()), "BalanceTooLow");

        assert_eq!(currencies::Pallet::<Runtime>::free_balance(PSWAP.into(), &account()), 0);
        assert_eq!(currencies::Pallet::<Runtime>::free_balance(VAL.into(), &account()), 0);
        assert_eq!(
            currencies::Pallet::<Runtime>::free_balance(PSWAP.into(), &account_id()),
            reserve_pswap_before
        );
        assert_eq!(
            currencies::Pallet::<Runtime>::free_balance(VAL.into(), &account_id()),
            reserve_val_before
        );
        assert_eq!(UmiNftReceivers::get(address), vec![1, Balance::MAX]);
        assert!(!UmiNftClaimed::get(address));
    });
}

#[test]
fn can_claim_umi_nft_rewards() {
    ExtBuilder::with_rewards(true).build().execute_with(|| {
        let address = EthAddress::from(hex!("3c52e573fd320153013f40b817dda4f9d648613c"));

        assert_ok!(Pallet::add_umi_nft_receivers(RuntimeOrigin::root(), vec![address]));

        let signature = hex!("5615253e3998c99cd9008baf9c471d7a8f5690bb35a40f872b7cbbf19bad616d4490fc78a84d4568673cf397243ac79eb1684a7e54440f862aecebb54c10474f1c");

        assert_eq!(currencies::Pallet::<Runtime>::free_balance(PSWAP.into(), &account()), 0);

        assert_ok!(Pallet::claim(origin(), signature.into()));

        assert_eq!(currencies::Pallet::<Runtime>::free_balance(PSWAP.into(), &account()), 1);
    });
}

#[test]
fn val_strategic_bonus_vesting_works() {
    ExtBuilder::with_rewards(true).build().execute_with(|| {
        let account_1: AccountId = account();
        let account_2: AccountId = hex!("7c0f877cd5720eee40d1183556f1fbd34931a6ee08c5299b4de2b2b43176831a").into();

        assert_eq!(TotalValRewards::get(), balance!(21000.1));
        assert_eq!(TotalClaimableVal::get(), balance!(3000));
        assert_eq!(EthAddresses::get(0), vec![EthAddress::from(hex!("21Bc9f4a3d9Dc86f142F802668dB7D908cF0A636"))]);
        assert_eq!(EthAddresses::get(1), vec![EthAddress::from(hex!("d170a274320333243b9f860e8891c6792de1ec19"))]);
        assert_eq!(EthAddresses::get(2), vec![EthAddress::from(hex!("886021f300dc809269cfc758a2364a2baf63af0c"))]);

        let blocks_per_day = <Runtime as crate::Config>::BLOCKS_PER_DAY;

        run_to_block(blocks_per_day - 1);
        assert_eq!(ValBurnedSinceLastVesting::get(), balance!(184.3));

        run_to_block(blocks_per_day);
        assert_eq!(CurrentClaimableVal::get(), balance!(20.273));
        assert_eq!(ValBurnedSinceLastVesting::get(), balance!(9.7));

        run_to_block(2 * blocks_per_day - 1);
        // By now vesting of total 20.9 VAL on a pro rata basis should have been taken place
        // There can be some loss of precision though due to pro rata distribution
        assert_approx_eq_abs!(TotalClaimableVal::get(), balance!(3020.2729999999999), balance!(0.000000001));
        assert_reward_info_approx(
            ValOwners::get(EthAddress::from(hex!("21Bc9f4a3d9Dc86f142F802668dB7D908cF0A636"))),
            balance!(111.965376355350688000),
            balance!(1000),
        );
        assert_reward_info_approx(
            ValOwners::get(EthAddress::from(hex!("d170a274320333243b9f860e8891c6792de1ec19"))),
            balance!(2908.29752710701376000),
            balance!(20000),
        );
        assert_reward_info_approx(
            ValOwners::get(EthAddress::from(hex!("886021f300dc809269cfc758a2364a2baf63af0c"))),
            balance!(0.010096537635535068),
            balance!(0.1),
        );

        // Claiming some rewards
        assert_ok!(Pallet::claim(
            RuntimeOrigin::signed(account_1.clone()),
            hex!("eb7009c977888910a96d499f802e4524a939702aa6fc8ed473829bffce9289d850b97a720aa05d4a7e70e15733eeebc4fe862dcb60e018c0bf560b2de013078f1c").into()
        ));
        assert_approx_eq_abs!(
            Assets::free_balance(&VAL, &account_1).unwrap(),
            balance!(111.965376355350688000),
            balance!(0.000000001)
        );
        assert_ok!(Pallet::claim(
            RuntimeOrigin::signed(account_2.clone()),
            hex!("22bea4c62999dc1be10cb603956b5731dfd296c9e0b0040e5fe8056db1e8df5648c519b704acdcdcf0d04ab01f81f2ed899edef437a4be8f36980d7f1119d7ce00").into()));
        assert_approx_eq_abs!(
            Assets::free_balance(&VAL, &account_2).unwrap(),
            balance!(2908.297527107013760000),
            balance!(0.000000001)
        );
        assert_approx_eq_abs!(
            TotalValRewards::get(),
            balance!(17979.837096537635552000),
            balance!(0.000000001)
        );
        assert_approx_eq_abs!(
            TotalClaimableVal::get(),
            balance!(0.010096537635535068),
            balance!(0.000000001)
        );

        run_to_block(2 * blocks_per_day);
        // More VAL is claimable, total amount of rewards remains
        assert_eq!(CurrentClaimableVal::get(), balance!(42.68));
        assert_approx_eq_abs!(
            TotalValRewards::get(),
            balance!(17979.837096537635552000),
            balance!(0.000000001)
        );

        run_to_block(167 * blocks_per_day);
        // In this block all the rewards should have been vested
        assert_reward_info_approx(
            ValOwners::get(EthAddress::from(hex!("21Bc9f4a3d9Dc86f142F802668dB7D908cF0A636"))),
            balance!(868.491901448558237327),
            balance!(888.034623644649312000),
        );
        assert_reward_info_approx(
            ValOwners::get(EthAddress::from(hex!("d170a274320333243b9f860e8891c6792de1ec19"))),
            balance!(16614.140867795317401998),
            balance!(17091.702472892986240000),
        );
        assert_reward_info_approx(
            ValOwners::get(EthAddress::from(hex!("886021f300dc809269cfc758a2364a2baf63af0c"))),
            balance!(0.1),
            balance!(0.1),
        );
    });
}

#[test]
fn val_vesting_distribution_handles_large_totals() {
    ExtBuilder::with_rewards(false).build().execute_with(|| {
        let address = EthAddress::from(hex!("21Bc9f4a3d9Dc86f142F802668dB7D908cF0A636"));
        let total = i128::MAX as common::Balance + 1;
        let now = <Runtime as crate::Config>::UPDATE_FREQUENCY;
        let batch_index: u32 = ((now % <Runtime as crate::Config>::BLOCKS_PER_DAY)
            / <Runtime as crate::Config>::UPDATE_FREQUENCY) as u32;

        ValOwners::insert(address, RewardInfo::new(0, total));
        TotalValRewards::put(total);
        TotalClaimableVal::put(0);
        CurrentClaimableVal::put(balance!(1));
        EthAddresses::insert(batch_index, vec![address]);

        Pallet::on_initialize(now);

        assert_eq!(ValOwners::get(address), RewardInfo::new(balance!(1), total));
        assert_eq!(TotalClaimableVal::get(), balance!(1));
    });
}
