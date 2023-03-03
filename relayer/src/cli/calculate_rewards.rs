use std::collections::BTreeMap;

use super::*;
use crate::{
    prelude::*,
    substrate::{AccountId, AssetId},
};
use bridge_types::H256;
use clap::*;
use codec::Decode;
use common::{prelude::FixedWrapper, Balance};
use indicatif::HumanFloatCount;
use indicatif::{FormattedDuration, ProgressState};
use indicatif::{ProgressIterator, ProgressStyle};
use substrate_gen::runtime::runtime_types::farming::PoolFarmer;

const MINUTE: u32 = 10;
const HOUR: u32 = 60 * MINUTE;
const DAY: u32 = 24 * HOUR;
// const PSWAP_PER_DAY: u128 = common::balance!(2500000);
const REFRESH_FREQUENCY: u32 = 2 * HOUR;
const VESTING_FREQUENCY: u32 = 6 * HOUR;
const VESTING_COEFF: u32 = 3;

fn my_style() -> AnyResult<ProgressStyle> {
    let style = ProgressStyle::with_template(
		"[{elapsed_precise}/{my_duration}] [{bar:80.red/240}] ({human_pos}/{human_len}, {my_per_sec}, {per_sec}, ETA {my_eta})",
	)?
	.progress_chars("━╸━")
	.with_key("my_eta", |s: &ProgressState, w: &mut dyn std::fmt::Write| {
		let _ = match (s.pos(), s.len()) {
			(pos, Some(len)) if pos > 0 => {
				let frac = (len - pos) as f64 / pos as f64;
				let eta = s.elapsed().as_secs_f64() * frac;
				write!(w, "{:#}", FormattedDuration(std::time::Duration::from_secs_f64(eta)))
			}
			_ => write!(w, "-"),
		};
	})
	.with_key("my_per_sec", |s: &ProgressState, w: &mut dyn std::fmt::Write| {
		let _ = match s.pos() {
			pos if pos > 0 => {
				let per_sec = pos as f64 / s.elapsed().as_secs_f64();
				write!(w, "{:#} it/s", HumanFloatCount(per_sec))
			}
			_ => write!(w, "-"),
		};
	})
	.with_key("my_duration", |s: &ProgressState, w: &mut dyn std::fmt::Write| {
		let _ = match (s.pos(), s.len()) {
			(pos, Some(len)) if pos > 0 => {
				let frac = len as f64 / pos as f64;
				let duration = s.elapsed().as_secs_f64() * frac;
				write!(w, "{:#}", FormattedDuration(std::time::Duration::from_secs_f64(duration)))
			}
			_ => write!(w, "-"),
		};
	});
    Ok(style)
}

#[derive(Args, Clone, Debug)]
pub(super) struct Command {
    #[clap(long, short)]
    start: u32,
    #[clap(long, short)]
    end: Option<u32>,
    #[clap(long, short)]
    output: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RewardsResult {
    rewards_xor_only: BTreeMap<AccountId, u128>,
    rewards_xor_xstusd: BTreeMap<AccountId, u128>,
    rewards_xstusd: BTreeMap<AccountId, u128>,
    rewards_xstusd_only: BTreeMap<AccountId, u128>,
}

impl Command {
    pub(super) async fn run(&self, args: &BaseArgs) -> AnyResult<()> {
        let sub = args.get_unsigned_substrate().await?;
        let end = if let Some(end) = self.end {
            end
        } else {
            sub.block_number::<u32>(None).await?
        };
        let calc = RewardCalculator::new(sub, self.start, end).await?;
        let result = calc.run().await?;
        let result = serde_json::to_string_pretty(&result)?;
        if let Some(output) = &self.output {
            std::fs::write(output, result)?;
        }
        Ok(())
    }
}

pub struct RewardCalculator {
    sub: SubUnsignedClient,
    farmers: BTreeMap<AccountId, Vec<PoolFarmer>>,
    farming_pools: BTreeMap<u32, Vec<AccountId>>,
    pool_properties: BTreeMap<AccountId, (AssetId, AssetId)>,
    pool_creation: BTreeMap<u32, Vec<(u32, AccountId)>>,
    rewards: RewardsResult,
    start: u32,
    end: u32,
}

impl RewardCalculator {
    pub async fn new(sub: SubUnsignedClient, start: u32, end: u32) -> AnyResult<Self> {
        let start_block_hash = sub.block_hash(Some(start)).await?;
        let end_block_hash = sub.block_hash(Some(end)).await?;
        log::info!("Load farmers");
        let farmers = Self::load_farmers(&sub, start_block_hash).await?;
        log::info!("Load farming pools");
        let farming_pools = Self::load_farming_pools(&sub, start_block_hash).await?;
        log::info!("Load pool properties");
        let pool_properties = Self::load_pool_properties(&sub, end_block_hash).await?;
        log::info!("Load pool creation blocks");
        let pool_creation = Self::load_pool_creation_blocks(&sub, end_block_hash).await?;
        Ok(Self {
            sub,
            farming_pools,
            farmers,
            pool_properties,
            pool_creation,
            start,
            end,
            rewards: Default::default(),
        })
    }

    async fn run(mut self) -> AnyResult<RewardsResult> {
        for now in ((self.start + 1)..=self.end)
            .into_iter()
            .collect::<Vec<_>>()
            .into_iter()
            .progress_with_style(my_style()?)
        {
            self.process_block(now).await?;
        }
        let rewards = self.rewards.rewards_xor_only.values().sum::<u128>();
        println!("XOR only: {:?}", rewards as f64 / 1e18f64);
        let rewards = self.rewards.rewards_xor_xstusd.values().sum::<u128>();
        println!("XOR+XSTUSD only: {:?}", rewards as f64 / 1e18f64);
        let rewards = self.rewards.rewards_xstusd.values().sum::<u128>();
        println!("XSTUSD-XOR: {:?}", rewards as f64 / 1e18f64);
        let rewards = self.rewards.rewards_xstusd_only.values().sum::<u128>();
        println!("XSTUSD only: {:?}", rewards as f64 / 1e18f64);
        Ok(self.rewards)
    }

    async fn process_block(&mut self, now: u32) -> AnyResult<()> {
        let block_hash = self.sub.block_hash(Some(now)).await?;
        self.refresh_pools(now, block_hash).await?;
        if now % VESTING_FREQUENCY == 0 {
            self.vest(now).await?;
        }
        if let Some(new_pools) = self.pool_creation.remove(&now) {
            for (_dex_id, pool_acc) in new_pools {
                self.farming_pools
                    .entry(now % REFRESH_FREQUENCY)
                    .or_insert(vec![])
                    .push(pool_acc);
            }
        }
        Ok(())
    }

    async fn vest(&mut self, now: u32) -> AnyResult<()> {
        let accounts = self.prepare_accounts_for_vesting(now, |base| {
            if base == common::XOR {
                Some(true)
            } else {
                None
            }
        })?;
        Self::vest_account_rewards(accounts, &mut self.rewards.rewards_xor_only);
        let accounts = self.prepare_accounts_for_vesting(now, |base| {
            if base == common::XSTUSD {
                Some(true)
            } else {
                None
            }
        })?;
        Self::vest_account_rewards(accounts, &mut self.rewards.rewards_xstusd_only);
        let accounts = self.prepare_accounts_for_vesting(now, |_base| Some(true))?;
        Self::vest_account_rewards(accounts, &mut self.rewards.rewards_xor_xstusd);
        let accounts = self.prepare_accounts_for_vesting(now, |base| {
            if base == common::XSTUSD {
                Some(true)
            } else {
                Some(false)
            }
        })?;
        Self::vest_account_rewards(accounts, &mut self.rewards.rewards_xstusd);
        Ok(())
    }

    fn prepare_account_rewards(
        accounts: BTreeMap<AccountId, (FixedWrapper, FixedWrapper)>,
    ) -> BTreeMap<AccountId, u128> {
        let total_weight = accounts
            .values()
            .fold(FixedWrapper::from(0), |a, b| a + b.1.clone());

        let reward = {
            let reward_per_day = FixedWrapper::from(common::balance!(2500000));
            let freq: u128 = VESTING_FREQUENCY.into();
            let blocks: u128 = DAY.into();
            let reward_vesting_part = FixedWrapper::from(common::balance!(freq))
                / FixedWrapper::from(common::balance!(blocks));
            reward_per_day * reward_vesting_part
        };

        accounts
            .into_iter()
            .map(|(account, weight)| {
                let account_reward = reward.clone() * weight.0 / total_weight.clone();
                let account_reward = account_reward.try_into_balance().unwrap_or(0);
                (account, account_reward)
            })
            .collect()
    }

    fn vest_account_rewards(
        accounts: BTreeMap<AccountId, (FixedWrapper, FixedWrapper)>,
        rewards: &mut BTreeMap<AccountId, u128>,
    ) {
        let new_rewards = Self::prepare_account_rewards(accounts);

        for (account, reward) in new_rewards {
            let current = rewards.entry(account).or_default();
            *current += reward;
        }
    }

    fn prepare_accounts_for_vesting<F>(
        &self,
        now: u32,
        should_include: F,
    ) -> AnyResult<BTreeMap<AccountId, (FixedWrapper, FixedWrapper)>>
    where
        F: Fn(AssetId) -> Option<bool>,
    {
        let mut accounts = BTreeMap::new();
        for (pool, farmers) in self.farmers.iter() {
            let (base, _) = self.pool_properties.get(pool).cloned().unwrap();
            if let Some(include) = should_include(base) {
                Self::prepare_pool_accounts_for_vesting(farmers, now, &mut accounts, include);
            }
        }
        Ok(accounts)
    }

    fn prepare_pool_accounts_for_vesting(
        farmers: &[PoolFarmer],
        now: u32,
        accounts: &mut BTreeMap<AccountId, (FixedWrapper, FixedWrapper)>,
        include: bool,
    ) {
        if farmers.is_empty() {
            return;
        }

        for farmer in farmers {
            let weight =
                Self::get_farmer_weight_amplified_by_time(farmer.weight, farmer.block, now);
            let mut entry = accounts
                .entry(farmer.account.clone())
                .or_insert((FixedWrapper::from(0u128), FixedWrapper::from(0u128)));
            entry.1 = entry.1.clone() + weight.clone();
            if include {
                entry.0 = entry.0.clone() + weight;
            }
        }
    }

    fn get_farmer_weight_amplified_by_time(
        farmer_weight: u128,
        farmer_block: u32,
        now: u32,
    ) -> FixedWrapper {
        // Ti
        let farmer_farming_time: u32 = now - farmer_block;
        let farmer_farming_time = FixedWrapper::from(common::balance!(farmer_farming_time));

        // Vi(t)
        let coeff = (FixedWrapper::from(common::balance!(1))
            + farmer_farming_time.clone() / FixedWrapper::from(common::balance!(now)))
        .pow(VESTING_COEFF);

        coeff * farmer_weight
    }

    async fn refresh_pools(&mut self, now: u32, block: H256) -> AnyResult<()> {
        if let Some(pools) = self.farming_pools.get(&(now % REFRESH_FREQUENCY)).cloned() {
            for pool in pools {
                self.refresh_pool(pool, now, block).await?;
            }
        }
        Ok(())
    }

    async fn refresh_pool(&mut self, pool_acc: AccountId, now: u32, block: H256) -> AnyResult<()> {
        let old_farmers = self.farmers.get(&pool_acc).cloned().unwrap_or_default();
        let (base_asset, target_asset) = self.pool_properties.get(&pool_acc).cloned().unwrap();
        let base = Self::get_balance(&self.sub, base_asset, pool_acc.clone(), block).await?;
        let target =
            Self::get_balance(&self.sub, target_asset.clone(), pool_acc.clone(), block).await?;
        let mut new_farmers = vec![];
        for (account, pool_tokens) in Self::load_pool_providers(&self.sub, &pool_acc, block).await?
        {
            let weight = Self::get_account_weight(target_asset.clone(), base, target, pool_tokens)
                .unwrap_or(0);
            let block = if let Some(farmer) = old_farmers.iter().find(|f| f.account == account) {
                farmer.block
            } else {
                now - (now % REFRESH_FREQUENCY)
            };
            new_farmers.push(PoolFarmer {
                account,
                block,
                weight,
            });
        }
        if !new_farmers.is_empty() || !old_farmers.is_empty() {
            self.farmers.insert(pool_acc, new_farmers);
        }
        Ok(())
    }

    #[allow(dead_code)]
    async fn load_rewards(
        sub: &SubUnsignedClient,
        block: H256,
    ) -> AnyResult<BTreeMap<AccountId, u128>> {
        let mut pools_iter = sub
            .api()
            .storage()
            .vested_rewards()
            .rewards_iter(false, Some(block))
            .await?;
        let mut res = BTreeMap::new();
        while let Some((key, v)) = pools_iter.next().await? {
            let (_, account) = <([u8; 48], AccountId)>::decode(&mut &key.0[..])?;
            let amount = v
                .rewards
                .into_iter()
                .find_map(|(reason, amount)| {
                    if matches!(reason, common::RewardReason::LiquidityProvisionFarming) {
                        Some(amount)
                    } else {
                        None
                    }
                })
                .unwrap_or(0);
            res.insert(account, amount);
        }
        Ok(res)
    }

    async fn load_farming_pools(
        sub: &SubUnsignedClient,
        block: H256,
    ) -> AnyResult<BTreeMap<u32, Vec<AccountId>>> {
        let mut pools_iter = sub
            .api()
            .storage()
            .farming()
            .pools_iter(false, Some(block))
            .await?;
        let mut res = BTreeMap::new();
        while let Some((key, v)) = pools_iter.next().await? {
            let (_, block) = <([u8; 32], u32)>::decode(&mut &key.0[..])?;
            res.insert(block, v);
        }
        Ok(res)
    }

    async fn load_farmers(
        sub: &SubUnsignedClient,
        block: H256,
    ) -> AnyResult<BTreeMap<AccountId, Vec<PoolFarmer>>> {
        let mut pools_iter = sub
            .api()
            .storage()
            .farming()
            .pool_farmers_iter(false, Some(block))
            .await?;
        let mut res = BTreeMap::new();
        while let Some((key, v)) = pools_iter.next().await? {
            let (_, pool_acc) = <([u8; 32], AccountId)>::decode(&mut &key.0[..])?;
            res.insert(pool_acc, v);
        }
        Ok(res)
    }

    async fn load_pool_providers(
        sub: &SubUnsignedClient,
        pool_acc: &AccountId,
        block: H256,
    ) -> AnyResult<Vec<(AccountId, u128)>> {
        let res = Self::load_keys_with_prefix::<
            sub_runtime::pool_xyk::storage::PoolProviders,
            ([u8; 32], AccountId, AccountId),
        >(
            sub,
            vec![subxt::StorageMapKey::new(
                pool_acc,
                subxt::StorageHasher::Identity,
            )],
            block,
        )
        .await?
        .into_iter()
        .map(|(k, v)| (k.2, v))
        .collect();
        Ok(res)
    }

    async fn load_keys_with_prefix<S: subxt::StorageEntry, K: Decode>(
        sub: &SubUnsignedClient,
        prefix: Vec<subxt::StorageMapKey>,
        block: H256,
    ) -> AnyResult<Vec<(K, S::Value)>> {
        let prefix = subxt::StorageEntryKey::Map(prefix)
            .final_key(subxt::storage::StorageKeyPrefix::new::<S>());
        let mut res = vec![];
        let mut start_key = None;
        loop {
            let keys = sub
                .api()
                .client
                .rpc()
                .storage_keys_paged(Some(prefix.clone()), 1000, start_key, Some(block))
                .await?;
            if keys.is_empty() {
                break;
            }
            start_key = keys.last().cloned();
            let values = sub
                .api()
                .client
                .rpc()
                .query_storage_at(&keys, Some(block))
                .await?;
            for changes in values {
                for (k, v) in changes.changes {
                    if let Some(v) = v {
                        res.push((
                            Decode::decode(&mut &k.0[..])?,
                            Decode::decode(&mut &v.0[..])?,
                        ));
                    }
                }
            }
        }
        Ok(res)
    }

    async fn load_pool_properties(
        sub: &SubUnsignedClient,
        block: H256,
    ) -> AnyResult<BTreeMap<AccountId, (AssetId, AssetId)>> {
        let mut pools_iter = sub
            .api()
            .storage()
            .pool_xyk()
            .properties_iter(false, Some(block))
            .await?;
        let mut res = BTreeMap::new();
        while let Some((key, v)) = pools_iter.next().await? {
            let (_, base, _, target) =
                <([u8; 48], AssetId, [u8; 16], AssetId)>::decode(&mut &key.0[..])?;
            res.insert(v.0, (base, target));
        }
        Ok(res)
    }

    async fn load_pool_creation_blocks(
        sub: &SubUnsignedClient,
        block: H256,
    ) -> AnyResult<BTreeMap<u32, Vec<(u32, AccountId)>>> {
        let mut pools_iter = sub
            .api()
            .storage()
            .pswap_distribution()
            .subscribed_accounts_iter(false, Some(block))
            .await?;
        let mut res = BTreeMap::new();
        while let Some((_, (dex_id, pool_acc, _, block))) = pools_iter.next().await? {
            res.entry(block).or_insert(vec![]).push((dex_id, pool_acc));
        }
        Ok(res)
    }

    fn reward_doubling_assets() -> &'static [AssetId] {
        &[
            common::PSWAP,
            common::VAL,
            common::DAI,
            common::ETH,
            common::XST,
        ]
    }

    async fn get_balance(
        sub: &SubUnsignedClient,
        asset: AssetId,
        acc: AccountId,
        block: H256,
    ) -> AnyResult<Balance> {
        let balance = if asset == common::XOR {
            sub.api()
                .storage()
                .system()
                .account(false, &acc, Some(block))
                .await?
                .data
                .free
        } else {
            sub.api()
                .storage()
                .tokens()
                .accounts(false, &acc, &asset, Some(block))
                .await?
                .free
        };
        Ok(balance)
    }

    fn get_base_asset_part(base: u128, target: u128, tokens: u128) -> AnyResult<u128> {
        let fxw_liq_in_pool =
            FixedWrapper::from(base).multiply_and_sqrt(&FixedWrapper::from(target));
        let fxw_piece = fxw_liq_in_pool / FixedWrapper::from(tokens);
        let fxw_value = FixedWrapper::from(base) / fxw_piece;
        let value = fxw_value.try_into_balance()?;
        Ok(value)
    }

    fn get_account_weight(
        asset: AssetId,
        base: u128,
        target: u128,
        tokens: u128,
    ) -> AnyResult<u128> {
        let base_amount = Self::get_base_asset_part(base, target, tokens)?;
        if base_amount < common::balance!(1) {
            return Ok(0);
        }
        let double_reward = Self::reward_doubling_assets().iter().any(|x| x == &asset);
        if double_reward {
            Ok(base_amount * 2)
        } else {
            Ok(base_amount)
        }
    }
}
