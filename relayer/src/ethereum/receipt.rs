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
use bridge_types::log::Log;
use eth_trie::Trie;
use ethers::prelude::*;
use futures::stream::FuturesOrdered;
use futures::TryStreamExt;
use rlp::RlpStream;
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyReceipt {
    pub gas_used: U256,
    pub log_bloom: Bloom,
    pub logs: Vec<Log>,
    pub outcome: TransactionOutcome,
}

impl From<&TransactionReceipt> for LegacyReceipt {
    fn from(t: &TransactionReceipt) -> Self {
        Self {
            gas_used: t.cumulative_gas_used,
            log_bloom: t.logs_bloom,
            logs: t
                .logs
                .iter()
                .map(|l| Log {
                    address: l.address,
                    data: l.data.as_ref().to_vec(),
                    topics: l.topics.clone(),
                })
                .collect(),
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
