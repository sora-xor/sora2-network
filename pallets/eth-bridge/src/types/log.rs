use crate::types::{BlockNumber, Bytes, Index, H160, H256, U256, U64};
use alloc::string::String;
use serde::{Deserialize, Serialize, Serializer};
use sp_std::prelude::*;

/// A log produced by a transaction.
#[serde(crate = "serde", rename_all = "camelCase")]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Log {
    /// H160
    pub address: H160,
    /// Topics
    pub topics: Vec<H256>,
    /// Data
    pub data: Bytes,
    /// Block Hash
    pub block_hash: Option<H256>,
    /// Block Number
    pub block_number: Option<U64>,
    /// Transaction Hash
    pub transaction_hash: Option<H256>,
    /// Transaction Index
    pub transaction_index: Option<Index>,
    /// Log Index in Block
    pub log_index: Option<U256>,
    /// Log Index in Transaction
    pub transaction_log_index: Option<U256>,
    /// Log Type
    pub log_type: Option<String>,
    /// Removed
    pub removed: Option<bool>,
}

impl Log {
    /// Returns true if the log has been removed.
    pub fn is_removed(&self) -> bool {
        match self.removed {
            Some(val_removed) => return val_removed,
            None => (),
        }
        match self.log_type {
            Some(ref val_log_type) => {
                if val_log_type == "removed" {
                    return true;
                }
            }
            None => (),
        }
        false
    }
}

#[derive(Default, Debug, PartialEq, Clone)]
struct ValueOrArray<T>(Vec<T>);

impl<T> Serialize for ValueOrArray<T>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0.len() {
            0 => serializer.serialize_none(),
            1 => Serialize::serialize(&self.0[0], serializer),
            _ => Serialize::serialize(&self.0, serializer),
        }
    }
}

/// Filter
#[serde(crate = "serde", rename_all = "camelCase")]
#[derive(Default, Debug, PartialEq, Clone, Serialize)]
pub struct Filter {
    /// From Block
    #[serde(skip_serializing_if = "Option::is_none")]
    from_block: Option<BlockNumber>,
    /// To Block
    #[serde(skip_serializing_if = "Option::is_none")]
    to_block: Option<BlockNumber>,
    /// Address
    #[serde(skip_serializing_if = "Option::is_none")]
    address: Option<ValueOrArray<H160>>,
    /// Topics
    #[serde(skip_serializing_if = "Option::is_none")]
    topics: Option<Vec<Option<ValueOrArray<H256>>>>,
    /// Limit
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<usize>,
}

/// Filter Builder
#[derive(Default, Clone)]
pub struct FilterBuilder {
    filter: Filter,
}

impl FilterBuilder {
    /// Sets from block
    pub fn from_block(mut self, block: BlockNumber) -> Self {
        self.filter.from_block = Some(block);
        self
    }

    /// Sets to block
    pub fn to_block(mut self, block: BlockNumber) -> Self {
        self.filter.to_block = Some(block);
        self
    }

    /// Single address
    pub fn address(mut self, address: Vec<H160>) -> Self {
        self.filter.address = Some(ValueOrArray(address));
        self
    }

    /// Topics
    pub fn topics(
        mut self,
        topic1: Option<Vec<H256>>,
        topic2: Option<Vec<H256>>,
        topic3: Option<Vec<H256>>,
        topic4: Option<Vec<H256>>,
    ) -> Self {
        let mut topics = vec![topic1, topic2, topic3, topic4]
            .into_iter()
            .rev()
            .skip_while(Option::is_none)
            .map(|option| option.map(ValueOrArray))
            .collect::<Vec<_>>();
        topics.reverse();

        self.filter.topics = Some(topics);
        self
    }

    /// Limit the result
    pub fn limit(mut self, limit: usize) -> Self {
        self.filter.limit = Some(limit);
        self
    }

    /// Returns filter
    pub fn build(&self) -> Filter {
        self.filter.clone()
    }
}

#[cfg(test)]
mod tests {
    use crate::types::{
        log::{Bytes, Log},
        Address, H160, H256,
    };

    #[test]
    fn is_removed_removed_true() {
        let log = Log {
            address: Address::from_low_u64_be(1),
            topics: vec![],
            data: Bytes(vec![]),
            block_hash: Some(H256::from_low_u64_be(2)),
            block_number: Some(1.into()),
            transaction_hash: Some(H256::from_low_u64_be(3)),
            transaction_index: Some(0.into()),
            log_index: Some(0.into()),
            transaction_log_index: Some(0.into()),
            log_type: None,
            removed: Some(true),
        };
        assert!(log.is_removed());
    }

    #[test]
    fn is_removed_removed_false() {
        let log = Log {
            address: H160::from_low_u64_be(1),
            topics: vec![],
            data: Bytes(vec![]),
            block_hash: Some(H256::from_low_u64_be(2)),
            block_number: Some(1.into()),
            transaction_hash: Some(H256::from_low_u64_be(3)),
            transaction_index: Some(0.into()),
            log_index: Some(0.into()),
            transaction_log_index: Some(0.into()),
            log_type: None,
            removed: Some(false),
        };
        assert!(!log.is_removed());
    }

    #[test]
    fn is_removed_log_type_removed() {
        let log = Log {
            address: Address::from_low_u64_be(1),
            topics: vec![],
            data: Bytes(vec![]),
            block_hash: Some(H256::from_low_u64_be(2)),
            block_number: Some(1.into()),
            transaction_hash: Some(H256::from_low_u64_be(3)),
            transaction_index: Some(0.into()),
            log_index: Some(0.into()),
            transaction_log_index: Some(0.into()),
            log_type: Some("removed".into()),
            removed: None,
        };
        assert!(log.is_removed());
    }

    #[test]
    fn is_removed_log_type_mined() {
        let log = Log {
            address: Address::from_low_u64_be(1),
            topics: vec![],
            data: Bytes(vec![]),
            block_hash: Some(H256::from_low_u64_be(2)),
            block_number: Some(1.into()),
            transaction_hash: Some(H256::from_low_u64_be(3)),
            transaction_index: Some(0.into()),
            log_index: Some(0.into()),
            transaction_log_index: Some(0.into()),
            log_type: Some("mined".into()),
            removed: None,
        };
        assert!(!log.is_removed());
    }

    #[test]
    fn is_removed_log_type_and_removed_none() {
        let log = Log {
            address: Address::from_low_u64_be(1),
            topics: vec![],
            data: Bytes(vec![]),
            block_hash: Some(H256::from_low_u64_be(2)),
            block_number: Some(1.into()),
            transaction_hash: Some(H256::from_low_u64_be(3)),
            transaction_index: Some(0.into()),
            log_index: Some(0.into()),
            transaction_log_index: Some(0.into()),
            log_type: None,
            removed: None,
        };
        assert!(!log.is_removed());
    }
}
