use crate::*;

pub struct GetPoolsWithBlock;

impl Get<Vec<(AccountId, BlockNumber)>> for GetPoolsWithBlock {
    fn get() -> Vec<(AccountId, BlockNumber)> {
        let mut res = vec![];
        for (_fee_account, (dex_id, pool_account, _freq, block)) in
            pswap_distribution::SubscribedAccounts::<Runtime>::iter()
        {
            if dex_id == u32::from(common::DEXId::PolkaswapXSTUSD) {
                res.push((pool_account, block));
            }
        }
        res
    }
}

pub type Migrations = (farming::migrations::v2::Migrate<Runtime, GetPoolsWithBlock>,);
