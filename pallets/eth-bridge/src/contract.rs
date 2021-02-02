use ethabi::Function;
use ethabi_contract::use_contract;
use frame_support::sp_io::hashing::keccak_256;
use once_cell::unsync::Lazy;
use sp_std::collections::btree_map::BTreeMap;
use_contract!(
    eth_bridge_contract,
    r#"[{"inputs":[{"internalType":"address[]","name":"initialPeers","type":"address[]"},{"internalType":"address","name":"addressVAL","type":"address"},{"internalType":"address","name":"addressXOR","type":"address"}],"stateMutability":"nonpayable","type":"constructor"},{"anonymous":false,"inputs":[{"indexed":false,"internalType":"address","name":"peerId","type":"address"},{"indexed":false,"internalType":"bool","name":"removal","type":"bool"}],"name":"ChangePeers","type":"event"},{"anonymous":false,"inputs":[{"indexed":false,"internalType":"bytes32","name":"destination","type":"bytes32"},{"indexed":false,"internalType":"uint256","name":"amount","type":"uint256"},{"indexed":false,"internalType":"address","name":"token","type":"address"},{"indexed":false,"internalType":"bytes32","name":"sidechainAsset","type":"bytes32"}],"name":"Deposit","type":"event"},{"anonymous":false,"inputs":[{"indexed":false,"internalType":"bytes32","name":"txHash","type":"bytes32"}],"name":"Withdrawal","type":"event"},{"stateMutability":"nonpayable","type":"fallback"},{"inputs":[],"name":"_addressVAL","outputs":[{"internalType":"address","name":"","type":"address"}],"stateMutability":"view","type":"function"},{"inputs":[],"name":"_addressXOR","outputs":[{"internalType":"address","name":"","type":"address"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"uint256","name":"","type":"uint256"}],"name":"_sidechainTokenAddressArray","outputs":[{"internalType":"address","name":"","type":"address"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"bytes32","name":"","type":"bytes32"}],"name":"_sidechainTokens","outputs":[{"internalType":"address","name":"","type":"address"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"address","name":"","type":"address"}],"name":"_sidechainTokensByAddress","outputs":[{"internalType":"bytes32","name":"","type":"bytes32"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"address","name":"","type":"address"}],"name":"_uniqueAddresses","outputs":[{"internalType":"bool","name":"","type":"bool"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"address","name":"","type":"address"}],"name":"acceptedEthTokens","outputs":[{"internalType":"bool","name":"","type":"bool"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"address","name":"newToken","type":"address"},{"internalType":"string","name":"ticker","type":"string"},{"internalType":"string","name":"name","type":"string"},{"internalType":"uint8","name":"decimals","type":"uint8"},{"internalType":"bytes32","name":"txHash","type":"bytes32"},{"internalType":"uint8[]","name":"v","type":"uint8[]"},{"internalType":"bytes32[]","name":"r","type":"bytes32[]"},{"internalType":"bytes32[]","name":"s","type":"bytes32[]"}],"name":"addEthNativeToken","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"string","name":"name","type":"string"},{"internalType":"string","name":"symbol","type":"string"},{"internalType":"uint8","name":"decimals","type":"uint8"},{"internalType":"uint256","name":"supply","type":"uint256"},{"internalType":"bytes32","name":"sidechainAssetId","type":"bytes32"},{"internalType":"bytes32","name":"txHash","type":"bytes32"},{"internalType":"uint8[]","name":"v","type":"uint8[]"},{"internalType":"bytes32[]","name":"r","type":"bytes32[]"},{"internalType":"bytes32[]","name":"s","type":"bytes32[]"}],"name":"addNewSidechainToken","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"newPeerAddress","type":"address"},{"internalType":"bytes32","name":"txHash","type":"bytes32"},{"internalType":"uint8[]","name":"v","type":"uint8[]"},{"internalType":"bytes32[]","name":"r","type":"bytes32[]"},{"internalType":"bytes32[]","name":"s","type":"bytes32[]"}],"name":"addPeerByPeer","outputs":[{"internalType":"bool","name":"","type":"bool"}],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"","type":"address"}],"name":"isPeer","outputs":[{"internalType":"bool","name":"","type":"bool"}],"stateMutability":"view","type":"function"},{"inputs":[],"name":"peersCount","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"address","name":"tokenAddress","type":"address"},{"internalType":"uint256","name":"amount","type":"uint256"},{"internalType":"address payable","name":"to","type":"address"},{"internalType":"address","name":"from","type":"address"},{"internalType":"bytes32","name":"txHash","type":"bytes32"},{"internalType":"uint8[]","name":"v","type":"uint8[]"},{"internalType":"bytes32[]","name":"r","type":"bytes32[]"},{"internalType":"bytes32[]","name":"s","type":"bytes32[]"}],"name":"receiveByEthereumAssetAddress","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"bytes32","name":"sidechainAssetId","type":"bytes32"},{"internalType":"uint256","name":"amount","type":"uint256"},{"internalType":"address","name":"to","type":"address"},{"internalType":"address","name":"from","type":"address"},{"internalType":"bytes32","name":"txHash","type":"bytes32"},{"internalType":"uint8[]","name":"v","type":"uint8[]"},{"internalType":"bytes32[]","name":"r","type":"bytes32[]"},{"internalType":"bytes32[]","name":"s","type":"bytes32[]"}],"name":"receiveBySidechainAssetId","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"peerAddress","type":"address"},{"internalType":"bytes32","name":"txHash","type":"bytes32"},{"internalType":"uint8[]","name":"v","type":"uint8[]"},{"internalType":"bytes32[]","name":"r","type":"bytes32[]"},{"internalType":"bytes32[]","name":"s","type":"bytes32[]"}],"name":"removePeerByPeer","outputs":[{"internalType":"bool","name":"","type":"bool"}],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"bytes32","name":"to","type":"bytes32"},{"internalType":"uint256","name":"amount","type":"uint256"},{"internalType":"address","name":"tokenAddress","type":"address"}],"name":"sendERC20ToSidechain","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"bytes32","name":"to","type":"bytes32"}],"name":"sendEthToSidechain","outputs":[],"stateMutability":"payable","type":"function"},{"inputs":[{"internalType":"address","name":"thisContractAddress","type":"address"},{"internalType":"string","name":"salt","type":"string"},{"internalType":"address","name":"newContractAddress","type":"address"},{"internalType":"address[]","name":"erc20nativeTokens","type":"address[]"},{"internalType":"uint8[]","name":"v","type":"uint8[]"},{"internalType":"bytes32[]","name":"r","type":"bytes32[]"},{"internalType":"bytes32[]","name":"s","type":"bytes32[]"}],"name":"shutDownAndMigrate","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"bytes32","name":"","type":"bytes32"}],"name":"used","outputs":[{"internalType":"bool","name":"","type":"bool"}],"stateMutability":"view","type":"function"},{"stateMutability":"payable","type":"receive"}]"#
);

pub type MethodId = [u8; 4];

pub fn calculate_method_id(function: &Function) -> MethodId {
    let signature = function.signature();
    let mut id = [0u8; 4];
    id.copy_from_slice(&keccak_256(signature.as_bytes())[..4]);
    id
}

pub const ADD_ETH_NATIVE_TOKEN_FN: Lazy<Function> =
    Lazy::new(|| eth_bridge_contract::functions::add_eth_native_token::function());
pub const ADD_ETH_NATIVE_TOKEN_ID: Lazy<MethodId> =
    Lazy::new(|| calculate_method_id(&*ADD_ETH_NATIVE_TOKEN_FN));
pub const ADD_ETH_NATIVE_TOKEN_TX_HASH_ARG_POS: usize = 4;

pub const ADD_NEW_SIDECHAIN_TOKEN_FN: Lazy<Function> =
    Lazy::new(|| eth_bridge_contract::functions::add_new_sidechain_token::function());
pub const ADD_NEW_SIDECHAIN_TOKEN_ID: Lazy<MethodId> =
    Lazy::new(|| calculate_method_id(&*ADD_NEW_SIDECHAIN_TOKEN_FN));
pub const ADD_NEW_SIDECHAIN_TOKEN_TX_HASH_ARG_POS: usize = 5;

pub const ADD_PEER_BY_PEER_FN: Lazy<Function> =
    Lazy::new(|| eth_bridge_contract::functions::add_peer_by_peer::function());
pub const ADD_PEER_BY_PEER_ID: Lazy<MethodId> =
    Lazy::new(|| calculate_method_id(&*ADD_PEER_BY_PEER_FN));
pub const ADD_PEER_BY_PEER_TX_HASH_ARG_POS: usize = 1;

pub const REMOVE_PEER_BY_PEER_FN: Lazy<Function> =
    Lazy::new(|| eth_bridge_contract::functions::remove_peer_by_peer::function());
pub const REMOVE_PEER_BY_PEER_ID: Lazy<MethodId> =
    Lazy::new(|| calculate_method_id(&*REMOVE_PEER_BY_PEER_FN));
pub const REMOVE_PEER_BY_PEER_TX_HASH_ARG_POS: usize = 1;

pub const RECEIVE_BY_ETHEREUM_ASSET_ADDRESS_FN: Lazy<Function> =
    Lazy::new(|| eth_bridge_contract::functions::receive_by_ethereum_asset_address::function());
pub const RECEIVE_BY_ETHEREUM_ASSET_ADDRESS_ID: Lazy<MethodId> =
    Lazy::new(|| calculate_method_id(&*RECEIVE_BY_ETHEREUM_ASSET_ADDRESS_FN));
pub const RECEIVE_BY_ETHEREUM_ASSET_ADDRESS_TX_HASH_ARG_POS: usize = 4;

pub const RECEIVE_BY_SIDECHAIN_ASSET_ID_FN: Lazy<Function> =
    Lazy::new(|| eth_bridge_contract::functions::receive_by_sidechain_asset_id::function());
pub const RECEIVE_BY_SIDECHAIN_ASSET_ID_ID: Lazy<MethodId> =
    Lazy::new(|| calculate_method_id(&*RECEIVE_BY_SIDECHAIN_ASSET_ID_FN));
pub const RECEIVE_BY_SIDECHAIN_ASSET_ID_TX_HASH_ARG_POS: usize = 4;

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

pub const FUNCTIONS: Lazy<BTreeMap<MethodId, FunctionMeta>> = Lazy::new(|| {
    vec![
        (
            *ADD_ETH_NATIVE_TOKEN_ID,
            FunctionMeta::new(
                eth_bridge_contract::functions::add_eth_native_token::function(),
                ADD_ETH_NATIVE_TOKEN_TX_HASH_ARG_POS,
            ),
        ),
        (
            *ADD_NEW_SIDECHAIN_TOKEN_ID,
            FunctionMeta::new(
                eth_bridge_contract::functions::add_new_sidechain_token::function(),
                ADD_NEW_SIDECHAIN_TOKEN_TX_HASH_ARG_POS,
            ),
        ),
        (
            *ADD_PEER_BY_PEER_ID,
            FunctionMeta::new(
                eth_bridge_contract::functions::add_peer_by_peer::function(),
                ADD_PEER_BY_PEER_TX_HASH_ARG_POS,
            ),
        ),
        (
            *REMOVE_PEER_BY_PEER_ID,
            FunctionMeta::new(
                eth_bridge_contract::functions::remove_peer_by_peer::function(),
                REMOVE_PEER_BY_PEER_TX_HASH_ARG_POS,
            ),
        ),
        (
            *RECEIVE_BY_ETHEREUM_ASSET_ADDRESS_ID,
            FunctionMeta::new(
                eth_bridge_contract::functions::receive_by_ethereum_asset_address::function(),
                RECEIVE_BY_ETHEREUM_ASSET_ADDRESS_TX_HASH_ARG_POS,
            ),
        ),
        (
            *RECEIVE_BY_SIDECHAIN_ASSET_ID_ID,
            FunctionMeta::new(
                eth_bridge_contract::functions::receive_by_sidechain_asset_id::function(),
                RECEIVE_BY_SIDECHAIN_ASSET_ID_TX_HASH_ARG_POS,
            ),
        ),
    ]
    .into_iter()
    .collect()
});
