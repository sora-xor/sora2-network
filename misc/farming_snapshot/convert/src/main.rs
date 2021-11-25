use parity_scale_codec::{Decode, Encode, Output};
use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::{BufRead, BufReader};

use sp_core::crypto::{AccountId32, Ss58AddressFormat, Ss58Codec};

fn parse_pools() -> HashMap<String, Vec<(String, u32)>> {
    let reader = BufReader::new(File::open("../pools.txt").unwrap());

    let mut pools = HashMap::new();
    let mut current_pool: Option<(String, Vec<(String, u32)>)> = None;

    for line in reader.lines() {
        let line = line.unwrap();
        if line.contains(":") {
            let mut parts = line.split(": ");
            let account = parts.next().unwrap().trim().to_string();
            let block: u32 = parts.next().unwrap().parse().unwrap();

            let (_, accounts) = current_pool.as_mut().unwrap();
            accounts.push((account, block));
        } else {
            if let Some((pool, accounts)) = current_pool.take() {
                pools.insert(pool, accounts);
            }

            current_pool = Some((line, Vec::new()));
        }
    }

    let (pool, accounts) = current_pool.unwrap();
    pools.insert(pool, accounts);

    pools
}

fn convert_pools(
    pools: HashMap<String, Vec<(String, u32)>>,
) -> BTreeMap<AccountId32, Vec<(AccountId32, u32)>> {
    sp_core::crypto::set_default_ss58_version(Ss58AddressFormat::Custom(69));

    pools
        .into_iter()
        .map(|(pool, accounts)| {
            let pool = AccountId32::from_string(&pool).unwrap();
            let accounts = accounts
                .into_iter()
                .map(|(account, block)| (AccountId32::from_string(&account).unwrap(), block))
                .collect();
            (pool, accounts)
        })
        .collect()
}

fn do_pools() {
    let pools = parse_pools();
    let pools = convert_pools(pools);
    let bytes = pools.encode();
    let decoded_pools =
        <BTreeMap<AccountId32, Vec<(AccountId32, u32)>>>::decode(&mut &bytes[..]).unwrap();
    assert_eq!(pools, decoded_pools);

    let mut file = File::create("../pools").unwrap();
    file.write(&bytes);
}

fn parse_rewards() -> HashMap<String, u128> {
    let reader = BufReader::new(File::open("../rewards.txt").unwrap());

    let mut rewards = HashMap::new();

    for line in reader.lines() {
        let line = line.unwrap();
        let mut parts = line.split(": ");
        let account = parts.next().unwrap().to_string();
        let reward: u128 = parts.next().unwrap().parse().unwrap();
        rewards.insert(account, reward);
    }

    rewards
}

fn convert_rewards(rewards: HashMap<String, u128>) -> Vec<(AccountId32, u128)> {
    sp_core::crypto::set_default_ss58_version(Ss58AddressFormat::Custom(69));

    rewards
        .into_iter()
        .map(|(account, reward)| {
            let account = AccountId32::from_string(&account).unwrap();
            (account, reward)
        })
        .collect()
}

fn do_rewards() {
    let rewards = parse_rewards();
    let rewards = convert_rewards(rewards);
    let bytes = rewards.encode();
    let decoded_rewards =
        <Vec<(AccountId32, u128)>>::decode(&mut &bytes[..]).unwrap();
    assert_eq!(rewards, decoded_rewards);

    let mut file = File::create("../rewards").unwrap();
    file.write(&bytes);
}

fn main() {
    do_pools();
    do_rewards();
}
