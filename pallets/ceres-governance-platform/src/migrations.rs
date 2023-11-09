use crate::*;
use codec::{Decode, Encode};
use common::generate_storage_instance;
use common::CERES_ASSET_ID;
use frame_support::log;
use frame_support::pallet_prelude::*;
use frame_support::BoundedVec;
use hex_literal::hex;
use sp_core::H256;
use sp_io::hashing::blake2_256;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::prelude::*;

#[derive(Encode, Decode, Default, PartialEq, Eq, scale_info::TypeInfo, Debug)]
pub struct OldPollInfo<Moment> {
    pub number_of_options: u32,
    pub poll_start_timestamp: Moment,
    pub poll_end_timestamp: Moment,
}

generate_storage_instance!(CeresGovernancePlatform, PollData);
type OldPollData<Moment> =
    StorageMap<PollDataOldInstance, Identity, Vec<u8>, OldPollInfo<Moment>, ValueQuery>;

#[derive(Encode, Decode, Default, PartialEq, Eq, scale_info::TypeInfo)]
pub struct OldVotingInfo {
    pub voting_option: u32,
    pub number_of_votes: Balance,
    pub ceres_withdrawn: bool,
}

generate_storage_instance!(CeresGovernancePlatform, Voting);
type OldVoting<T> = StorageDoubleMap<
    VotingOldInstance,
    Identity,
    Vec<u8>,
    Identity,
    AccountIdOf<T>,
    OldVotingInfo,
    ValueQuery,
>;

pub fn migrate<T: Config>() -> Result<(), &'static str> {
    let poll_asset: AssetIdOf<T> = CERES_ASSET_ID.into();
    let user = AuthorityAccount::<T>::get();
    let bytes = hex!("c4e7d5a63d8e887932bb6dc505dd204005c3ecfb6de5f1f0d3ac0a308b2b2915");
    let first_poll_creator = AccountIdOf::<T>::decode(&mut &bytes[..]).unwrap();
    let mut options = BoundedVec::default();
    options.try_push(BoundedString::truncate_from("Yes")).ok();
    options.try_push(BoundedString::truncate_from("No")).ok();

    //Drain old data
    let number_of_drained_polls = OldPollData::<T::Moment>::drain().count();
    log::info!("Number of polls: {}", number_of_drained_polls);

    let mut poll_start_timestamp_a: <T as pallet_timestamp::Config>::Moment = 1647612888u32.into();
    poll_start_timestamp_a = poll_start_timestamp_a * 1000u32.into();
    let mut poll_end_timestamp_a: <T as pallet_timestamp::Config>::Moment = 1647699288u32.into();
    poll_end_timestamp_a = poll_end_timestamp_a * 1000u32.into();
    let title_a = BoundedString::truncate_from(
        "Do you want Ceres staking v2 with rewards pool of 300 CERES to go live?",
    );
    let description_a =  BoundedString::truncate_from(
        "The Ceres v2 staking pool would have 300 CERES rewards taken from the Ceres Treasury wallet. Staking would have a 14,400 CERES pool limit and would last a month and a half with minimum APR 16.66%.");

    let nonce: <T as frame_system::Config>::Index = 305u32.into();
    let encoded = (&first_poll_creator, nonce).using_encoded(blake2_256);
    let poll_id_a = H256::from(encoded);

    <PollData<T>>::insert(
        poll_id_a,
        crate::PollInfo {
            poll_asset,
            poll_start_timestamp: poll_start_timestamp_a,
            poll_end_timestamp: poll_end_timestamp_a,
            title: title_a,
            description: description_a,
            options: options.clone(),
        },
    );

    let mut poll_start_timestamp_b: <T as pallet_timestamp::Config>::Moment = 1648804056u32.into();
    poll_start_timestamp_b = poll_start_timestamp_b * 1000u32.into();
    let mut poll_end_timestamp_b: <T as pallet_timestamp::Config>::Moment = 1648890456u32.into();
    poll_end_timestamp_b = poll_end_timestamp_b * 1000u32.into();
    let title_b =
        BoundedString::truncate_from("Can Launchpad costs be paid from the Treasury wallet?");
    let description_b = BoundedString::truncate_from(
        "Ceres Launchpad is coming soon with new SORA runtime release. Launchpad requires KYC services which should be paid (about $11,740).",
    );

    let nonce: <T as frame_system::Config>::Index = 15u32.into();
    let encoded = (&user, nonce).using_encoded(blake2_256);
    let poll_id_b = H256::from(encoded);

    <PollData<T>>::insert(
        poll_id_b,
        crate::PollInfo {
            poll_asset,
            poll_start_timestamp: poll_start_timestamp_b,
            poll_end_timestamp: poll_end_timestamp_b,
            title: title_b,
            description: description_b,
            options: options.clone(),
        },
    );

    let mut poll_start_timestamp_c: <T as pallet_timestamp::Config>::Moment = 1664388000u32.into();
    poll_start_timestamp_c = poll_start_timestamp_c * 1000u32.into();
    let mut poll_end_timestamp_c: <T as pallet_timestamp::Config>::Moment = 1664560800u32.into();
    poll_end_timestamp_c = poll_end_timestamp_c * 1000u32.into();
    let title_c = BoundedString::truncate_from(
        "Should DAI and CERES from Treasury be used for providing CERES liquidity on other parachain?",
    );
    let description_c =  BoundedString::truncate_from("The Ceres team plans to integrate its services and tools on other parachains in the DotSama ecosystem (Ceres/Demeter liquidity and Demeter farming still remain on SORA, it is the base of the Ceres project). The first parachain that the Ceres team wants to integrate their products on is Astar. For this purpose, liquidity should be provided for the CERES token on the Astar network and the proposal is to use 20,000 DAI and 582.46 CERES from the Treasury. If there is an opportunity in the future, the plan is to return the funds to the Treasury. Demeter liquidity (DEO Arena integration) on Astar will be provided from team's funds.");

    let nonce: <T as frame_system::Config>::Index = 69u32.into();
    let encoded = (&user, nonce).using_encoded(blake2_256);
    let poll_id_c = H256::from(encoded);

    <PollData<T>>::insert(
        poll_id_c,
        crate::PollInfo {
            poll_asset,
            poll_start_timestamp: poll_start_timestamp_c,
            poll_end_timestamp: poll_end_timestamp_c,
            title: title_c,
            description: description_c,
            options: options.clone(),
        },
    );

    let mut poll_start_timestamp_d: <T as pallet_timestamp::Config>::Moment = 1686934800u32.into();
    poll_start_timestamp_d = poll_start_timestamp_d * 1000u32.into();
    let mut poll_end_timestamp_d: <T as pallet_timestamp::Config>::Moment = 1687107600u32.into();
    poll_end_timestamp_d = poll_end_timestamp_d * 1000u32.into();
    let title_d =
        BoundedString::truncate_from("Should XOR/CERES farming pool with CERES rewards be closed?");
    let description_d = BoundedString::truncate_from(
        "Until now, the portion of Ceres fees was used for rewards for the XOR/CERES farming pool. If the pool were to close, CERES tokens would go for burning.",
    );

    let nonce: <T as frame_system::Config>::Index = 166u32.into();
    let encoded = (&user, nonce).using_encoded(blake2_256);
    let poll_id_d = H256::from(encoded);

    <PollData<T>>::insert(
        poll_id_d,
        crate::PollInfo {
            poll_asset,
            poll_start_timestamp: poll_start_timestamp_d,
            poll_end_timestamp: poll_end_timestamp_d,
            title: title_d,
            description: description_d,
            options: options.clone(),
        },
    );

    let old_poll_id_a = "16171D34600005D".as_bytes().to_vec();
    let old_poll_id_b = "16171D346000060".as_bytes().to_vec();
    let old_poll_id_c = "16171D346000063".as_bytes().to_vec();
    let old_poll_id_d = "16461FA8A000000".as_bytes().to_vec();

    // Map Vec<u8> -> H256
    let mut map = BTreeMap::<Vec<u8>, H256>::new();

    map.insert(old_poll_id_a, poll_id_a);
    map.insert(old_poll_id_b, poll_id_b);
    map.insert(old_poll_id_c, poll_id_c);
    map.insert(old_poll_id_d, poll_id_d);

    for old_poll_id in map.keys() {
        for (account, voting_info) in OldVoting::<T>::drain_prefix(&old_poll_id) {
            <Voting<T>>::insert(
                map.get(old_poll_id).unwrap(),
                account,
                crate::VotingInfo {
                    voting_option: if voting_info.voting_option == 1u32 {
                        BoundedString::truncate_from("Yes")
                    } else {
                        BoundedString::truncate_from("No")
                    },
                    number_of_votes: voting_info.number_of_votes,
                    asset_withdrawn: voting_info.ceres_withdrawn,
                },
            );
        }
    }

    Ok(())
}
