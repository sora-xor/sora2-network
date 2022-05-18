use super::*;
use eth_trie::Trie;
use ethers::prelude::*;
use futures::stream::FuturesOrdered;
use futures::TryStreamExt;
use rlp::{Encodable, RlpStream};
use std::sync::Arc;

#[derive(Debug)]
pub struct BlockWithReceipts {
    trie: eth_trie::EthTrie<eth_trie::MemoryDB>,
}

impl BlockWithReceipts {
    pub async fn load<M, B>(client: M, block_id: B) -> anyhow::Result<Self>
    where
        M: Middleware + Send + Sync,
        M::Error: 'static,
        B: Into<BlockId> + Send + Sync,
    {
        let block_id = block_id.into();
        let block = client
            .get_block(block_id)
            .await
            .with_context(|| format!("get_block({:?})", block_id))?
            .ok_or(anyhow::anyhow!(format!("Block {:?} not found", block_id)))?;
        let receipts = block
            .transactions
            .iter()
            .map(|tx| client.get_transaction_receipt(*tx))
            .collect::<FuturesOrdered<_>>()
            .map(|x| {
                let x = x?;
                x.ok_or_else(|| anyhow::anyhow!(""))
            })
            .try_collect::<Vec<_>>()
            .await?;
        let db = eth_trie::MemoryDB::new(false);
        let mut trie = eth_trie::EthTrie::new(Arc::new(db));
        for (i, receipt) in receipts.iter().enumerate() {
            let i = rlp::encode(&i);
            let receipt = super::receipt::TypedReceipt::from(receipt).encode();
            trie.insert(&i, &receipt)?;
        }
        if trie.root_hash()? != block.receipts_root.0.into() {
            return Err(anyhow::anyhow!(format!(
                "Incorrect receipt root hash: {} != {}",
                trie.root_hash()?,
                block.receipts_root
            )));
        }
        Ok(Self { trie })
    }

    pub fn prove(&mut self, tx_id: usize) -> AnyResult<Vec<Vec<u8>>> {
        let key = rlp::encode(&tx_id);
        let proof = self.trie.get_proof(&key)?;
        Ok(proof)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionOutcome {
    Unknown,
    StateRoot(H256),
    StatusCode(u8),
}

impl From<&TransactionReceipt> for TransactionOutcome {
    fn from(t: &TransactionReceipt) -> Self {
        if let Some(root) = t.root {
            Self::StateRoot(root)
        } else if let Some(status) = t.status {
            Self::StatusCode(status.as_u64() as u8)
        } else {
            Self::Unknown
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    address: H160,
    topics: Vec<H256>,
    data: Vec<u8>,
}

impl Encodable for LogEntry {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(3);
        s.append(&self.address);
        s.append_list(&self.topics);
        s.append(&self.data);
    }
}

impl From<&Log> for LogEntry {
    fn from(log: &Log) -> Self {
        Self {
            address: log.address,
            topics: log.topics.clone(),
            data: log.data.to_vec(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyReceipt {
    pub gas_used: U256,
    pub log_bloom: Bloom,
    pub logs: Vec<LogEntry>,
    pub outcome: TransactionOutcome,
}

impl From<&TransactionReceipt> for LegacyReceipt {
    fn from(t: &TransactionReceipt) -> Self {
        Self {
            gas_used: t.cumulative_gas_used,
            log_bloom: t.logs_bloom,
            logs: t.logs.iter().map(|x| x.into()).collect(),
            outcome: t.into(),
        }
    }
}

impl LegacyReceipt {
    pub fn rlp_append(&self, s: &mut RlpStream) {
        match self.outcome {
            TransactionOutcome::Unknown => {
                s.begin_list(3);
            }
            TransactionOutcome::StateRoot(ref root) => {
                s.begin_list(4);
                s.append(root);
            }
            TransactionOutcome::StatusCode(ref status_code) => {
                s.begin_list(4);
                s.append(status_code);
            }
        }
        s.append(&self.gas_used);
        s.append(&self.log_bloom);
        s.append_list(&self.logs);
    }
}

#[derive(Eq, Hash, Debug, Copy, Clone, PartialEq)]
#[repr(u8)]
pub enum TypedTxId {
    EIP1559Transaction = 0x02,
    AccessList = 0x01,
    #[allow(dead_code)]
    Legacy = 0x00,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypedReceipt {
    Legacy(LegacyReceipt),
    AccessList(LegacyReceipt),
    EIP1559Transaction(LegacyReceipt),
}

impl From<&TransactionReceipt> for TypedReceipt {
    fn from(t: &TransactionReceipt) -> Self {
        let legacy = t.into();
        match t.transaction_type {
            Some(x) if x.as_u64() == 1 => Self::AccessList(legacy),
            Some(x) if x.as_u64() == 2 => Self::EIP1559Transaction(legacy),
            _ => Self::Legacy(legacy),
        }
    }
}

impl TypedReceipt {
    pub fn encode(&self) -> Vec<u8> {
        match self {
            Self::Legacy(receipt) => {
                let mut s = RlpStream::new();
                receipt.rlp_append(&mut s);
                s.as_raw().to_vec()
            }
            Self::AccessList(receipt) => {
                let mut rlps = RlpStream::new();
                receipt.rlp_append(&mut rlps);
                [&[TypedTxId::AccessList as u8], rlps.as_raw()].concat()
            }
            Self::EIP1559Transaction(receipt) => {
                let mut rlps = RlpStream::new();
                receipt.rlp_append(&mut rlps);
                [&[TypedTxId::EIP1559Transaction as u8], rlps.as_raw()].concat()
            }
        }
    }
}
