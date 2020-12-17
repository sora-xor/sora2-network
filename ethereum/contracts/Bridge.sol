pragma solidity ^0.7.4;
// "SPDX-License-Identifier: Apache License 2.0"

import "./IERC20.sol";
import "./MasterToken.sol";

/**
 * Provides functionality of bridge contract
 */
contract Bridge {
    bool internal initialized_;
    mapping(address => bool) public isPeer;
    uint public peersCount;
    /** Substrate proofs used */
    mapping(bytes32 => bool) public used;
    mapping(address => bool) public _uniqueAddresses;

    mapping(bytes32 => address) public _sidechainTokens;
    mapping(address => bytes32) public _sidechainTokensByAddress;
    address[] public _sidechainTokenAddressArray;

    event Withdrawal(bytes32 txHash);
    event Deposit(bytes32 destination, uint amount, address token, bytes32 sidechainAsset);
    event ChangePeers(address peerId, bool removal);
    
    address public addressVAL = 0xe88f8313e61A97cEc1871EE37fBbe2a8bf3ed1E4;
    address public addressXOR = 0x40FD72257597aA14C7231A7B1aaa29Fce868F677;

    /**
     * Constructor.
     * @param initialPeers - list of initial bridge validators on substrate side.
     */
    constructor(
        address[] memory initialPeers)  {
        for (uint8 i = 0; i < initialPeers.length; i++) {
            addPeer(initialPeers[i]);
        }
        initialized_ = true;
    }
    
    modifier shouldBeInitialized {
        require(initialized_ == true, "Contract should be initialized to use this function");
        _;
    }
    
    function shutDownAndMigrate(
        address thisContractAddress, 
        string memory salt,
        address newContractAddress,
        address[] calldata erc20nativeTokens,  //List of ERC20 tokens with non zero balances for this contract. Can be taken from substrate bridge peers.
        uint8[] memory v,
        bytes32[] memory r,
        bytes32[] memory s
        )
    public
    shouldBeInitialized {
        require(address(this) == thisContractAddress);
        require(checkSignatures(keccak256(abi.encode(thisContractAddress, salt, erc20nativeTokens)),
            v,
            r,
            s), "Peer signatures are invalid"
        );
        for(uint i=0; i<_sidechainTokenAddressArray.length; i++) {
            MasterToken token = MasterToken(_sidechainTokenAddressArray[i]);
            token.transferOwnership(newContractAddress);
        }
        for(uint i=0; i<erc20nativeTokens.length; i++) {
            IERC20 token = IERC20(erc20nativeTokens[i]);
            token.transfer(newContractAddress,  token.balanceOf(address(this)));
        }
        initialized_ = false;
    }
    
    function addNewSidechainToken(
        string memory name, 
        string memory symbol,
        uint8 decimals,
        uint256 supply,
        bytes32 sidechainAssetId,
        uint8[] memory v,
        bytes32[] memory r,
        bytes32[] memory s) 
        public {
        
        require(checkSignatures(keccak256(abi.encode(
            name, 
            symbol, 
            decimals, 
            supply, 
            sidechainAssetId)),
            v,
            r,
            s), "Peer signatures are invalid"
        );
        // Create new instance of the token
        MasterToken tokenInstance = new MasterToken(name, symbol, decimals, address(this), supply, sidechainAssetId);
        _sidechainTokens[sidechainAssetId] = address(tokenInstance);
        _sidechainTokensByAddress[address(tokenInstance)] = sidechainAssetId;
        _sidechainTokenAddressArray.push(address(tokenInstance));
    }
    
    function sendEthToSidechain(
        bytes32 to
        ) 
    public 
    payable
    shouldBeInitialized {
        require(msg.value > 0, "ETH VALUE SHOULD BE MORE THAN 0");
        bytes32 empty;
        emit Deposit(to, msg.value, address(0x0), empty);
    }

    /**
     * A special function-like stub to allow ether accepting
     */
    function sendERC20ToSidechain(
        bytes32 to, 
        uint amount, 
        address tokenAddress) 
        external 
        shouldBeInitialized {

        IERC20 token = IERC20(tokenAddress);
        
        require (token.allowance(msg.sender, address(this)) >= amount, "NOT ENOUGH DELEGATED TOKENS ON SENDER BALANCE");

        bytes32 sidechainAssetId = _sidechainTokensByAddress[tokenAddress];
        if(sidechainAssetId.length != 0 || addressVAL == tokenAddress || addressXOR == tokenAddress) {
            MasterToken mtoken = MasterToken(tokenAddress);
            mtoken.burnFrom(msg.sender, amount);
        } else {
            token.transferFrom(msg.sender, address(this), amount);
        }
        emit Deposit(to, amount, tokenAddress, sidechainAssetId);
    }

    function addPeerByPeer(
        address newPeerAddress,
        bytes32 txHash,
        uint8[] memory v,
        bytes32[] memory r,
        bytes32[] memory s
    )
    public 
    shouldBeInitialized
    returns (bool)
    {
        require(used[txHash] == false);
        require(checkSignatures(keccak256(abi.encode(newPeerAddress, txHash)),
            v,
            r,
            s), "Peer signatures are invalid"
        );

        addPeer(newPeerAddress);
        used[txHash] = true;
        emit ChangePeers(newPeerAddress, false);
        return true;
    }

    function removePeerByPeer(
        address peerAddress,
        bytes32 txHash,
        uint8[] memory v,
        bytes32[] memory r,
        bytes32[] memory s
    )
    public 
    shouldBeInitialized
    returns (bool)
    {
        require(used[txHash] == false);
        require(checkSignatures(
                keccak256(abi.encode(peerAddress, txHash)),
                v,
                r,
                s), "Peer signatures are invalid"
        );

        removePeer(peerAddress);
        used[txHash] = true;
        emit ChangePeers(peerAddress, true);
        return true;
    }

    /**
     * Withdraws specified amount of ether or one of ERC-20 tokens to provided address
     * @param tokenAddress address of token to withdraw (0 for ether)
     * @param amount amount of tokens or ether to withdraw
     * @param to target account address
     * @param txHash hash of transaction from Iroha
     * @param v array of signatures of tx_hash (v-component)
     * @param r array of signatures of tx_hash (r-component)
     * @param s array of signatures of tx_hash (s-component)
     * @param from relay contract address
     */
    function receiveByEthereumAssetAddress(
        address tokenAddress,
        uint256 amount,
        address payable to,
        bytes32 txHash,
        uint8[] memory v,
        bytes32[] memory r,
        bytes32[] memory s,
        address from
    )
    public
    {
        require(used[txHash] == false);
        require(checkSignatures(
                keccak256(abi.encode(tokenAddress, amount, to, txHash, from)),
                v,
                r,
                s), "Peer signatures are invalid"
        );

        if (tokenAddress == address(0)) {
            used[txHash] = true;
            // untrusted transfer, relies on provided cryptographic proof
            to.transfer(amount);
        } else {
            IERC20 coin = IERC20(tokenAddress);
            used[txHash] = true;
            // untrusted call, relies on provided cryptographic proof
            coin.transfer(to, amount);
        }
        emit Withdrawal(txHash);
    }
    
/**
     * Mint new Token
     * @param sidechainAssetId id of sidechainToken to mint
     * @param amount how much to mint
     * @param to destination address
     * @param txHash hash of transaction from Iroha
     * @param v array of signatures of tx_hash (v-component)
     * @param r array of signatures of tx_hash (r-component)
     * @param s array of signatures of tx_hash (s-component)
     */
    function receiveBySidechainAssetId(
        bytes32 sidechainAssetId,
        uint256 amount,
        address to,
        bytes32 txHash,
        uint8[] memory v,
        bytes32[] memory r,
        bytes32[] memory s,
        address from
    )
    public
    {   
        require(_sidechainTokens[sidechainAssetId] != address(0x0), "Sidechain asset is not registered");
        require(used[txHash] == false);
        require(checkSignatures(
                keccak256(abi.encode(sidechainAssetId, amount, to, txHash, from)),
                v,
                r,
                s), "Peer signatures are invalid"
        );

        MasterToken tokenInstance = MasterToken(_sidechainTokens[sidechainAssetId]);       
        tokenInstance.mintTokens(to, amount);
        used[txHash] = true;
        emit Withdrawal(txHash);
    }

    /**
     * Checks given addresses for duplicates and if they are peers signatures
     * @param hash unsigned data
     * @param v v-component of signature from hash
     * @param r r-component of signature from hash
     * @param s s-component of signature from hash
     * @return true if all given addresses are correct or false otherwise
     */
    function checkSignatures(bytes32 hash,
        uint8[] memory v,
        bytes32[] memory r,
        bytes32[] memory s
    ) 
    private 
    returns (bool) {
        require(peersCount >= 1);
        require(v.length == r.length);
        require(r.length == s.length);
        uint needSigs = peersCount - (peersCount - 1) / 3;
        require(s.length >= needSigs);

        uint count = 0;
        address[] memory recoveredAddresses = new address[](s.length);
        for (uint i = 0; i < s.length; ++i) {
            address recoveredAddress = recoverAddress(
                hash,
                v[i],
                r[i],
                s[i]
            );

            // not a peer address or not unique
            if (isPeer[recoveredAddress] != true || _uniqueAddresses[recoveredAddress] == true) {
                continue;
            }
            recoveredAddresses[count] = recoveredAddress;
            count = count + 1;
            _uniqueAddresses[recoveredAddress] = true;
        }

        // restore state for future usages
        for (uint i = 0; i < count; ++i) {
            _uniqueAddresses[recoveredAddresses[i]] = false;
        }

        return count >= needSigs;
    }

    /**
     * Recovers address from a given single signature
     * @param hash unsigned data
     * @param v v-component of signature from hash
     * @param r r-component of signature from hash
     * @param s s-component of signature from hash
     * @return address recovered from signature
     */
    function recoverAddress(
        bytes32 hash, 
        uint8 v, 
        bytes32 r, 
        bytes32 s) 
    private 
    pure 
    returns (address) {
        bytes32 simple_hash = keccak256(abi.encodePacked("\x19Ethereum Signed Message:\n32", hash));
        address res = ecrecover(simple_hash, v, r, s);
        return res;
    }

    /**
     * Adds new peer to list of signature verifiers. 
     * Internal function
     * @param newAddress address of new peer
     */
    function addPeer(address newAddress) 
    internal 
    returns (uint) {
        require(isPeer[newAddress] == false);
        isPeer[newAddress] = true;
        ++peersCount;
        return peersCount;
    }

    function removePeer(address peerAddress) 
    internal {
        require(isPeer[peerAddress] == true);
        isPeer[peerAddress] = false;
        --peersCount;
    }
}