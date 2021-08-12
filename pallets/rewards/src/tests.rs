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
    assert_approx_eq, assert_noop_msg, balance, generate_storage_instance, Balance, PSWAP, VAL,
};
use frame_support::pallet_prelude::*;
use frame_support::traits::PalletVersion;
use frame_support::{assert_noop, assert_ok};
use frame_system::RawOrigin;
use hex_literal::hex;
use sp_io::TestExternalities;

use crate::mock::*;
use crate::{EthereumAddress, PswapFarmOwners, ReservesAcc, RewardInfo};

type Pallet = crate::Pallet<Runtime>;
type Error = crate::Error<Runtime>;
type Assets = assets::Pallet<Runtime>;

type ValOwners = crate::ValOwners<Runtime>;
type EthAddresses = crate::EthAddresses<Runtime>;
type TotalValRewards = crate::TotalValRewards<Runtime>;
type ValBurnedSinceLastVesting = crate::ValBurnedSinceLastVesting<Runtime>;
type CurrentClaimableVal = crate::CurrentClaimableVal<Runtime>;
type TotalClaimableVal = crate::TotalClaimableVal<Runtime>;
type MigrationPending = crate::MigrationPending<Runtime>;

type PalletInfoOf<T> = <T as frame_system::Config>::PalletInfo;

generate_storage_instance!(Rewards, ValOwners);

type DeprecatedValOwners =
    StorageMap<ValOwnersOldInstance, Identity, EthereumAddress, Balance, ValueQuery>;

fn account() -> AccountId {
    hex!("f08879dab4530529153a1bdb63e27cd3be45f1574a122b7e88579b6e5e60bd43").into()
}

fn origin() -> Origin {
    Origin::signed(account())
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
        assert_ok!(Pallet::claim(Origin::signed(account_id.clone()), signature));
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
fn storage_migration_to_v1_2_0_works() {
    ExtBuilder::with_rewards(true).build().execute_with(|| {
        PalletVersion {
            major: 1,
            minor: 1,
            patch: 0,
        }
        .put_into_storage::<PalletInfoOf<Runtime>, Pallet>();
        let expected_pswap = balance!(74339.224845900297630556);
        let expected_eth_address =
            EthereumAddress::from_slice(&hex!("e687c6c6b28745864871566134b5589aa05b953d"));

        let reserves_account_id = technical::Pallet::<Runtime>::tech_account_id_to_account_id(
            &ReservesAcc::<Runtime>::get(),
        )
        .unwrap();
        let balance_a =
            assets::Pallet::<Runtime>::free_balance(&PSWAP.into(), &reserves_account_id).unwrap();

        Pallet::on_runtime_upgrade();
        let balance_b =
            assets::Pallet::<Runtime>::free_balance(&PSWAP.into(), &reserves_account_id).unwrap();

        assert_eq!(balance_b - balance_a, expected_pswap);
        assert_eq!(
            PswapFarmOwners::<Runtime>::get(expected_eth_address),
            expected_pswap
        );
    });
}

#[test]
fn storage_migration_to_v1_2_0_works_2() {
    TestExternalities::new_empty().execute_with(|| {
        let old_val_owners: Vec<(EthereumAddress, Balance)> = vec![
            (
                hex!("21Bc9f4a3d9Dc86f142F802668dB7D908cF0A636").into(),
                balance!(100),
            ),
            (
                hex!("d170a274320333243b9f860e8891c6792de1ec19").into(),
                balance!(200),
            ),
            (
                hex!("886021f300dc809269cfc758a2364a2baf63af0c").into(),
                balance!(300),
            ),
            (
                hex!("8b98125055f70613bcee1a391e3096393bddb1ca").into(),
                balance!(400),
            ),
            (
                hex!("d0d6f3cafe2b0b2d1c04d5bcf44461dd6e4f0344").into(),
                balance!(500),
            ),
        ];
        for (k, v) in old_val_owners {
            DeprecatedValOwners::insert(k, v);
        }
        PalletVersion {
            major: 1,
            minor: 1,
            patch: 0,
        }
        .put_into_storage::<PalletInfoOf<Runtime>, Pallet>();

        assert_eq!(MigrationPending::get(), false);

        // Import data for storage migration
        let w = Pallet::on_runtime_upgrade();
        assert_eq!(w, 1002200);

        let mut val_owners = ValOwners::iter().collect::<Vec<_>>();
        val_owners.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(
            val_owners,
            vec![
                (
                    hex!("21Bc9f4a3d9Dc86f142F802668dB7D908cF0A636").into(),
                    (balance!(100), balance!(100)).into()
                ),
                (
                    hex!("886021f300dc809269cfc758a2364a2baf63af0c").into(),
                    (balance!(300), balance!(300)).into()
                ),
                (
                    hex!("8b98125055f70613bcee1a391e3096393bddb1ca").into(),
                    (balance!(400), balance!(400)).into()
                ),
                (
                    hex!("d0d6f3cafe2b0b2d1c04d5bcf44461dd6e4f0344").into(),
                    (balance!(500), balance!(500)).into()
                ),
                (
                    hex!("d170a274320333243b9f860e8891c6792de1ec19").into(),
                    (balance!(200), balance!(200)).into()
                ),
            ]
        );

        let mut chunks = EthAddresses::iter().collect::<Vec<_>>();
        chunks.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(chunks.len(), 5);
        assert_eq!(chunks[0].1.len(), 1);
        assert_eq!(chunks[4].1.len(), 1);

        assert_eq!(TotalValRewards::get(), balance!(1500));
        assert_eq!(TotalClaimableVal::get(), balance!(1500));
        assert_eq!(CurrentClaimableVal::get(), 0);
        assert_eq!(ValBurnedSinceLastVesting::get(), 0);
        assert_eq!(MigrationPending::get(), true);

        // Applying extrinsic to set unclaimed VAL rewards
        let unclaimed_val = unclaimed_val_data();
        assert_ok!(Pallet::finalize_storage_migration(
            RawOrigin::Root.into(),
            unclaimed_val
        ));
        assert_eq!(MigrationPending::get(), false);
        assert_eq!(TotalValRewards::get(), balance!(9000));

        val_owners = ValOwners::iter().collect::<Vec<_>>();
        val_owners.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(
            val_owners,
            vec![
                (
                    hex!("21Bc9f4a3d9Dc86f142F802668dB7D908cF0A636").into(),
                    (balance!(100), balance!(600)).into()
                ),
                (
                    hex!("886021f300dc809269cfc758a2364a2baf63af0c").into(),
                    (balance!(300), balance!(1800)).into()
                ),
                (
                    hex!("8b98125055f70613bcee1a391e3096393bddb1ca").into(),
                    (balance!(400), balance!(2400)).into()
                ),
                (
                    hex!("d0d6f3cafe2b0b2d1c04d5bcf44461dd6e4f0344").into(),
                    (balance!(500), balance!(3000)).into()
                ),
                (
                    hex!("d170a274320333243b9f860e8891c6792de1ec19").into(),
                    (balance!(200), balance!(1200)).into()
                ),
            ]
        );

        // All subsequent attempts to call this extrinsic result into an error
        assert_noop!(
            Pallet::finalize_storage_migration(RawOrigin::Root.into(), unclaimed_val_data()),
            Error::IllegalCall
        );
    });
}

#[test]
fn val_strategic_bonus_vesting_works() {
    ExtBuilder::with_rewards(true).build().execute_with(|| {
        let account_1: AccountId = account();
        let account_2: AccountId = hex!("7c0f877cd5720eee40d1183556f1fbd34931a6ee08c5299b4de2b2b43176831a").into();

        assert_eq!(TotalValRewards::get(), balance!(21000.1));
        assert_eq!(TotalClaimableVal::get(), balance!(3000));
        assert_eq!(EthAddresses::get(0), vec![EthereumAddress::from(hex!("21Bc9f4a3d9Dc86f142F802668dB7D908cF0A636"))]);
        assert_eq!(EthAddresses::get(1), vec![EthereumAddress::from(hex!("d170a274320333243b9f860e8891c6792de1ec19"))]);
        assert_eq!(EthAddresses::get(2), vec![EthereumAddress::from(hex!("886021f300dc809269cfc758a2364a2baf63af0c"))]);

        let blocks_per_day = <Runtime as crate::Config>::BLOCKS_PER_DAY;

        run_to_block(blocks_per_day - 1);
        assert_eq!(ValBurnedSinceLastVesting::get(), balance!(188.1));

        run_to_block(blocks_per_day);
        assert_eq!(CurrentClaimableVal::get(), balance!(20.691));
        assert_eq!(ValBurnedSinceLastVesting::get(), balance!(9.9));

        run_to_block(2 * blocks_per_day - 1);
        // By now vesting of total 20.9 VAL on a pro rata basis should have been taken place
        // There can be some loss of precision though due to pro rata distribution
        assert_approx_eq!(TotalClaimableVal::get(), balance!(3020.6909999999999), balance!(0.000000001));
        assert_eq!(
            ValOwners::get(EthereumAddress::from(hex!("21Bc9f4a3d9Dc86f142F802668dB7D908cF0A636"))),
            RewardInfo::new(balance!(111.985281022471321000), balance!(1000))
        );
        assert_eq!(
            ValOwners::get(EthereumAddress::from(hex!("d170a274320333243b9f860e8891c6792de1ec19"))),
            RewardInfo::new(balance!(2908.695620449426420000), balance!(20000))
        );
        assert_eq!(
            ValOwners::get(EthereumAddress::from(hex!("886021f300dc809269cfc758a2364a2baf63af0c"))),
            RewardInfo::new(balance!(0.010098528102247132), balance!(0.1))
        );

        // Claiming some rewards
        assert_ok!(Pallet::claim(
            Origin::signed(account_1.clone()),
            hex!("eb7009c977888910a96d499f802e4524a939702aa6fc8ed473829bffce9289d850b97a720aa05d4a7e70e15733eeebc4fe862dcb60e018c0bf560b2de013078f1c").into()
        ));
        assert_eq!(
            Assets::free_balance(&VAL, &account_1).unwrap(),
            balance!(111.985281022471321000)
        );
        assert_ok!(Pallet::claim(
            Origin::signed(account_2.clone()),
            hex!("22bea4c62999dc1be10cb603956b5731dfd296c9e0b0040e5fe8056db1e8df5648c519b704acdcdcf0d04ab01f81f2ed899edef437a4be8f36980d7f1119d7ce00").into()));
        assert_eq!(
            Assets::free_balance(&VAL, &account_2).unwrap(),
            balance!(2908.695620449426420000)
        );
        assert_eq!(TotalValRewards::get(), balance!(17979.419098528102259000));
        assert_eq!(TotalClaimableVal::get(), balance!(0.010098528102247132));

        run_to_block(2 * blocks_per_day);
        // More VAL is claimable, total amount of rewards remains
        assert_eq!(CurrentClaimableVal::get(), balance!(43.56));
        assert_eq!(TotalValRewards::get(), balance!(17979.419098528102259000));

        run_to_block(167 * blocks_per_day);
        // In this block all the rewards should have been vested
        assert_eq!(
            ValOwners::get(EthereumAddress::from(hex!("21Bc9f4a3d9Dc86f142F802668dB7D908cF0A636"))),
            RewardInfo::new(balance!(886.399690114186276904), balance!(888.014718977528679000))
        );
        assert_eq!(
            ValOwners::get(EthereumAddress::from(hex!("d170a274320333243b9f860e8891c6792de1ec19"))),
            RewardInfo::new(balance!(16956.699736344280235178), balance!(17091.304379550573580000))
        );
        assert_eq!(
            ValOwners::get(EthereumAddress::from(hex!("886021f300dc809269cfc758a2364a2baf63af0c"))),
            RewardInfo::new(balance!(0.1), balance!(0.1))
        );
    });
}
