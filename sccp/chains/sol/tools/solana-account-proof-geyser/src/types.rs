use std::collections::HashMap;

use borsh::{BorshDeserialize, BorshSerialize};
use agave_geyser_plugin_interface::geyser_plugin_interface::{
    ReplicaBlockInfoV2, ReplicaBlockInfoV3, ReplicaBlockInfoV4, SlotStatus,
};
use solana_sdk::hash::Hash;
use solana_sdk::pubkey::Pubkey;

pub type AccountHashAccumulator = HashMap<u64, AccountHashMap>;
pub type AccountHashMap = HashMap<Pubkey, (u64, Hash, AccountInfo)>;
pub type TransactionSigAccumulator = HashMap<u64, u64>;

#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
pub struct Proof {
    pub path: Vec<usize>,
    pub siblings: Vec<Vec<Hash>>,
}

#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
pub struct Data {
    pub pubkey: Pubkey,
    pub hash: Hash,
    pub account: AccountInfo,
}

#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
pub struct AccountDeltaProof(pub Pubkey, pub (Data, Proof));

#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
pub struct BankHashProof {
    pub proofs: Vec<AccountDeltaProof>,
    pub num_sigs: u64,
    pub parent_slot: u64,
    pub account_delta_root: Hash,
    pub parent_bankhash: Hash,
    pub blockhash: Hash,
}

#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
pub struct Update {
    pub slot: u64,
    pub root: Hash,
    pub proof: BankHashProof,
}

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct AccountInfo {
    pub pubkey: Pubkey,
    pub lamports: u64,
    pub owner: Pubkey,
    pub executable: bool,
    pub rent_epoch: u64,
    pub data: Vec<u8>,
    pub write_version: u64,
    pub slot: u64,
}

impl Default for AccountInfo {
    fn default() -> Self {
        Self {
            pubkey: Pubkey::default(),
            lamports: 0,
            owner: Pubkey::default(),
            executable: false,
            rent_epoch: 0,
            data: Vec::new(),
            write_version: 0,
            slot: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TransactionInfo {
    pub slot: u64,
    pub num_sigs: u64,
}

#[derive(Debug, Clone)]
pub struct BlockInfo {
    pub slot: u64,
    pub parent_slot: u64,
    pub parent_bankhash: String,
    pub blockhash: String,
}

impl<'a> From<&'a ReplicaBlockInfoV2<'a>> for BlockInfo {
    fn from(block: &'a ReplicaBlockInfoV2<'a>) -> Self {
        Self {
            slot: block.slot,
            parent_slot: block.parent_slot,
            parent_bankhash: block.parent_blockhash.to_string(),
            blockhash: block.blockhash.to_string(),
        }
    }
}

impl<'a> From<&'a ReplicaBlockInfoV3<'a>> for BlockInfo {
    fn from(block: &'a ReplicaBlockInfoV3<'a>) -> Self {
        Self {
            slot: block.slot,
            parent_slot: block.parent_slot,
            parent_bankhash: block.parent_blockhash.to_string(),
            blockhash: block.blockhash.to_string(),
        }
    }
}

impl<'a> From<&'a ReplicaBlockInfoV4<'a>> for BlockInfo {
    fn from(block: &'a ReplicaBlockInfoV4<'a>) -> Self {
        Self {
            slot: block.slot,
            parent_slot: block.parent_slot,
            parent_bankhash: block.parent_blockhash.to_string(),
            blockhash: block.blockhash.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SlotInfo {
    pub slot: u64,
    pub status: SlotStatus,
}

#[derive(Debug, Clone)]
pub enum GeyserMessage {
    AccountMessage(AccountInfo),
    BlockMessage(BlockInfo),
    TransactionMessage(TransactionInfo),
    SlotMessage(SlotInfo),
}
