use alloc::boxed::Box;
use ethabi::Function;
use ethabi_contract::use_contract;
use frame_support::sp_io::hashing::keccak_256;
use once_cell::race::OnceBox;
use sp_std::collections::btree_map::BTreeMap;

use_contract!(
    eth_bridge_contract,
    r#"[{"anonymous":false,"inputs":[{"indexed":false,"internalType":"address","name":"peerId","type":"address"},{"indexed":false,"internalType":"bool","name":"removal","type":"bool"}],"name":"ChangePeers","type":"event"},{"anonymous":false,"inputs":[{"indexed":false,"internalType":"bytes32","name":"destination","type":"bytes32"},{"indexed":false,"internalType":"uint256","name":"amount","type":"uint256"},{"indexed":false,"internalType":"address","name":"token","type":"address"},{"indexed":false,"internalType":"bytes32","name":"sidechainAsset","type":"bytes32"}],"name":"Deposit","type":"event"},{"anonymous":false,"inputs":[{"indexed":false,"internalType":"address","name":"to","type":"address"}],"name":"Migrated","type":"event"},{"anonymous":false,"inputs":[],"name":"PreparedForMigration","type":"event"},{"anonymous":false,"inputs":[{"indexed":false,"internalType":"bytes32","name":"txHash","type":"bytes32"}],"name":"Withdrawal","type":"event"},{"inputs":[{"internalType":"address","name":"newToken","type":"address"},{"internalType":"string","name":"symbol","type":"string"},{"internalType":"string","name":"name","type":"string"},{"internalType":"uint8","name":"decimals","type":"uint8"},{"internalType":"bytes32","name":"txHash","type":"bytes32"},{"internalType":"uint8[]","name":"v","type":"uint8[]"},{"internalType":"bytes32[]","name":"r","type":"bytes32[]"},{"internalType":"bytes32[]","name":"s","type":"bytes32[]"}],"name":"addEthNativeToken","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"string","name":"name","type":"string"},{"internalType":"string","name":"symbol","type":"string"},{"internalType":"uint8","name":"decimals","type":"uint8"},{"internalType":"bytes32","name":"sidechainAssetId","type":"bytes32"},{"internalType":"bytes32","name":"txHash","type":"bytes32"},{"internalType":"uint8[]","name":"v","type":"uint8[]"},{"internalType":"bytes32[]","name":"r","type":"bytes32[]"},{"internalType":"bytes32[]","name":"s","type":"bytes32[]"}],"name":"addNewSidechainToken","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"newPeerAddress","type":"address"},{"internalType":"bytes32","name":"txHash","type":"bytes32"},{"internalType":"uint8[]","name":"v","type":"uint8[]"},{"internalType":"bytes32[]","name":"r","type":"bytes32[]"},{"internalType":"bytes32[]","name":"s","type":"bytes32[]"}],"name":"addPeerByPeer","outputs":[{"internalType":"bool","name":"","type":"bool"}],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"thisContractAddress","type":"address"},{"internalType":"bytes32","name":"salt","type":"bytes32"},{"internalType":"uint8[]","name":"v","type":"uint8[]"},{"internalType":"bytes32[]","name":"r","type":"bytes32[]"},{"internalType":"bytes32[]","name":"s","type":"bytes32[]"}],"name":"prepareForMigration","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"tokenAddress","type":"address"},{"internalType":"uint256","name":"amount","type":"uint256"},{"internalType":"address payable","name":"to","type":"address"},{"internalType":"address","name":"from","type":"address"},{"internalType":"bytes32","name":"txHash","type":"bytes32"},{"internalType":"uint8[]","name":"v","type":"uint8[]"},{"internalType":"bytes32[]","name":"r","type":"bytes32[]"},{"internalType":"bytes32[]","name":"s","type":"bytes32[]"}],"name":"receiveByEthereumAssetAddress","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"bytes32","name":"sidechainAssetId","type":"bytes32"},{"internalType":"uint256","name":"amount","type":"uint256"},{"internalType":"address","name":"to","type":"address"},{"internalType":"address","name":"from","type":"address"},{"internalType":"bytes32","name":"txHash","type":"bytes32"},{"internalType":"uint8[]","name":"v","type":"uint8[]"},{"internalType":"bytes32[]","name":"r","type":"bytes32[]"},{"internalType":"bytes32[]","name":"s","type":"bytes32[]"}],"name":"receiveBySidechainAssetId","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"peerAddress","type":"address"},{"internalType":"bytes32","name":"txHash","type":"bytes32"},{"internalType":"uint8[]","name":"v","type":"uint8[]"},{"internalType":"bytes32[]","name":"r","type":"bytes32[]"},{"internalType":"bytes32[]","name":"s","type":"bytes32[]"}],"name":"removePeerByPeer","outputs":[{"internalType":"bool","name":"","type":"bool"}],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"bytes32","name":"to","type":"bytes32"},{"internalType":"uint256","name":"amount","type":"uint256"},{"internalType":"address","name":"tokenAddress","type":"address"}],"name":"sendERC20ToSidechain","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"bytes32","name":"to","type":"bytes32"}],"name":"sendEthToSidechain","outputs":[],"stateMutability":"payable","type":"function"},{"inputs":[{"internalType":"address","name":"thisContractAddress","type":"address"},{"internalType":"bytes32","name":"salt","type":"bytes32"},{"internalType":"address","name":"newContractAddress","type":"address"},{"internalType":"address[]","name":"erc20nativeTokens","type":"address[]"},{"internalType":"uint8[]","name":"v","type":"uint8[]"},{"internalType":"bytes32[]","name":"r","type":"bytes32[]"},{"internalType":"bytes32[]","name":"s","type":"bytes32[]"}],"name":"shutDownAndMigrate","outputs":[],"stateMutability":"nonpayable","type":"function"},{"stateMutability":"payable","type":"receive"},{"stateMutability":"nonpayable","type":"fallback"},{"inputs":[{"internalType":"address[]","name":"initialPeers","type":"address[]"},{"internalType":"address","name":"addressVAL","type":"address"},{"internalType":"address","name":"addressXOR","type":"address"},{"internalType":"bytes32","name":"networkId","type":"bytes32"}],"stateMutability":"nonpayable","type":"constructor"},{"inputs":[],"name":"_addressVAL","outputs":[{"internalType":"address","name":"","type":"address"}],"stateMutability":"view","type":"function"},{"inputs":[],"name":"_addressXOR","outputs":[{"internalType":"address","name":"","type":"address"}],"stateMutability":"view","type":"function"},{"inputs":[],"name":"_networkId","outputs":[{"internalType":"bytes32","name":"","type":"bytes32"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"uint256","name":"","type":"uint256"}],"name":"_sidechainTokenAddressArray","outputs":[{"internalType":"address","name":"","type":"address"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"bytes32","name":"","type":"bytes32"}],"name":"_sidechainTokens","outputs":[{"internalType":"address","name":"","type":"address"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"address","name":"","type":"address"}],"name":"_sidechainTokensByAddress","outputs":[{"internalType":"bytes32","name":"","type":"bytes32"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"address","name":"","type":"address"}],"name":"_uniqueAddresses","outputs":[{"internalType":"bool","name":"","type":"bool"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"address","name":"","type":"address"}],"name":"acceptedEthTokens","outputs":[{"internalType":"bool","name":"","type":"bool"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"address","name":"","type":"address"}],"name":"isPeer","outputs":[{"internalType":"bool","name":"","type":"bool"}],"stateMutability":"view","type":"function"},{"inputs":[],"name":"peersCount","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"bytes32","name":"","type":"bytes32"}],"name":"used","outputs":[{"internalType":"bool","name":"","type":"bool"}],"stateMutability":"view","type":"function"}]"#
);

pub type MethodId = [u8; 4];

pub fn calculate_method_id(function: &Function) -> MethodId {
    let signature = function.signature(false);
    let mut id = [0u8; 4];
    id.copy_from_slice(&keccak_256(signature.as_bytes())[..4]);
    id
}

pub static ADD_ETH_NATIVE_TOKEN_FN: OnceBox<Function> = OnceBox::new();
pub static ADD_ETH_NATIVE_TOKEN_ID: OnceBox<MethodId> = OnceBox::new();
pub static ADD_ETH_NATIVE_TOKEN_TX_HASH_ARG_POS: usize = 4;

pub static ADD_NEW_SIDECHAIN_TOKEN_FN: OnceBox<Function> = OnceBox::new();
pub static ADD_NEW_SIDECHAIN_TOKEN_ID: OnceBox<MethodId> = OnceBox::new();
pub static ADD_NEW_SIDECHAIN_TOKEN_TX_HASH_ARG_POS: usize = 5;

pub static ADD_PEER_BY_PEER_FN: OnceBox<Function> = OnceBox::new();
pub static ADD_PEER_BY_PEER_ID: OnceBox<MethodId> = OnceBox::new();
pub static ADD_PEER_BY_PEER_TX_HASH_ARG_POS: usize = 1;

pub static REMOVE_PEER_BY_PEER_FN: OnceBox<Function> = OnceBox::new();
pub static REMOVE_PEER_BY_PEER_ID: OnceBox<MethodId> = OnceBox::new();
pub static REMOVE_PEER_BY_PEER_TX_HASH_ARG_POS: usize = 1;

pub static RECEIVE_BY_ETHEREUM_ASSET_ADDRESS_FN: OnceBox<Function> = OnceBox::new();
pub static RECEIVE_BY_ETHEREUM_ASSET_ADDRESS_ID: OnceBox<MethodId> = OnceBox::new();
pub static RECEIVE_BY_ETHEREUM_ASSET_ADDRESS_TX_HASH_ARG_POS: usize = 4;

pub static RECEIVE_BY_SIDECHAIN_ASSET_ID_FN: OnceBox<Function> = OnceBox::new();
pub static RECEIVE_BY_SIDECHAIN_ASSET_ID_ID: OnceBox<MethodId> = OnceBox::new();
pub static RECEIVE_BY_SIDECHAIN_ASSET_ID_TX_HASH_ARG_POS: usize = 4;

pub struct FunctionMeta {
    pub function: Function,
    pub tx_hash_arg_pos: usize,
}

impl FunctionMeta {
    pub fn new(function: Function, tx_hash_arg_pos: usize) -> Self {
        FunctionMeta {
            function,
            tx_hash_arg_pos,
        }
    }
}

pub static FUNCTIONS: OnceBox<BTreeMap<MethodId, FunctionMeta>> = OnceBox::new();

pub fn init_add_peer_by_peer_fn() -> Box<MethodId> {
    let add_peer_by_peer_fn = ADD_PEER_BY_PEER_FN
        .get_or_init(|| Box::new(eth_bridge_contract::functions::add_peer_by_peer::function()));
    Box::new(calculate_method_id(&add_peer_by_peer_fn))
}

pub fn init_remove_peer_by_peer_fn() -> Box<MethodId> {
    let remove_peer_by_peer_fn = REMOVE_PEER_BY_PEER_FN
        .get_or_init(|| Box::new(eth_bridge_contract::functions::remove_peer_by_peer::function()));
    Box::new(calculate_method_id(&remove_peer_by_peer_fn))
}

pub fn functions() -> Box<BTreeMap<MethodId, FunctionMeta>> {
    let add_eth_native_token_fn = ADD_ETH_NATIVE_TOKEN_FN
        .get_or_init(|| Box::new(eth_bridge_contract::functions::add_eth_native_token::function()));
    let add_new_sidechain_token_fn = ADD_NEW_SIDECHAIN_TOKEN_FN.get_or_init(|| {
        Box::new(eth_bridge_contract::functions::add_new_sidechain_token::function())
    });
    let add_peer_by_peer_fn = ADD_PEER_BY_PEER_FN
        .get_or_init(|| Box::new(eth_bridge_contract::functions::add_peer_by_peer::function()));
    let remove_peer_by_peer_fn = REMOVE_PEER_BY_PEER_FN
        .get_or_init(|| Box::new(eth_bridge_contract::functions::remove_peer_by_peer::function()));
    let receive_by_eth_asset_address_fn = RECEIVE_BY_ETHEREUM_ASSET_ADDRESS_FN.get_or_init(|| {
        Box::new(eth_bridge_contract::functions::receive_by_ethereum_asset_address::function())
    });
    let receive_by_sidechain_asset_id_fn = RECEIVE_BY_SIDECHAIN_ASSET_ID_FN.get_or_init(|| {
        Box::new(eth_bridge_contract::functions::receive_by_sidechain_asset_id::function())
    });
    let map = vec![
        (
            *ADD_ETH_NATIVE_TOKEN_ID
                .get_or_init(|| Box::new(calculate_method_id(&add_eth_native_token_fn))),
            FunctionMeta::new(
                add_eth_native_token_fn.clone(),
                ADD_ETH_NATIVE_TOKEN_TX_HASH_ARG_POS,
            ),
        ),
        (
            *ADD_NEW_SIDECHAIN_TOKEN_ID
                .get_or_init(|| Box::new(calculate_method_id(&add_new_sidechain_token_fn))),
            FunctionMeta::new(
                add_new_sidechain_token_fn.clone(),
                ADD_NEW_SIDECHAIN_TOKEN_TX_HASH_ARG_POS,
            ),
        ),
        (
            *ADD_PEER_BY_PEER_ID.get_or_init(init_add_peer_by_peer_fn),
            FunctionMeta::new(
                add_peer_by_peer_fn.clone(),
                ADD_PEER_BY_PEER_TX_HASH_ARG_POS,
            ),
        ),
        (
            *REMOVE_PEER_BY_PEER_ID.get_or_init(init_remove_peer_by_peer_fn),
            FunctionMeta::new(
                remove_peer_by_peer_fn.clone(),
                REMOVE_PEER_BY_PEER_TX_HASH_ARG_POS,
            ),
        ),
        (
            *RECEIVE_BY_ETHEREUM_ASSET_ADDRESS_ID
                .get_or_init(|| Box::new(calculate_method_id(&receive_by_eth_asset_address_fn))),
            FunctionMeta::new(
                receive_by_eth_asset_address_fn.clone(),
                RECEIVE_BY_ETHEREUM_ASSET_ADDRESS_TX_HASH_ARG_POS,
            ),
        ),
        (
            *RECEIVE_BY_SIDECHAIN_ASSET_ID_ID
                .get_or_init(|| Box::new(calculate_method_id(&receive_by_sidechain_asset_id_fn))),
            FunctionMeta::new(
                receive_by_sidechain_asset_id_fn.clone(),
                RECEIVE_BY_SIDECHAIN_ASSET_ID_TX_HASH_ARG_POS,
            ),
        ),
    ]
    .into_iter()
    .collect();
    Box::new(map)
}
