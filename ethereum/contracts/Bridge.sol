pragma solidity ^0.6.12;
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
    /** Iroha tx hashes used */
    mapping(bytes32 => bool) public used;
    mapping(address => bool) public _uniqueAddresses;

    mapping(bytes32 => address) public _sidechainTokens;
    mapping(address => bytes32) public _sidechainTokensByAddress;

    event Withdrawal(bytes32 txHash);
    event Deposit(bytes32 destination, uint amount, address token, bytes32 sidechainAsset);

    /**
     * Constructor.
     * @param initialPeers - list of initial bridge validators on substrate side.
     */
    constructor(
        address[] memory initialPeers) 
        public {
        for (uint8 i = 0; i < initialPeers.length; i++) {
            addPeer(initialPeers[i]);
        }
        initialized_ = true;
    }
    
    modifier shouldBeInitialized {
        require(initialized_ == true, "Contract should be initialized to use this function");
        _;
    }
    
    function shutDown(
        address thisContractAddress, 
        string memory salt,
        uint8[] memory v,
        bytes32[] memory r,
        bytes32[] memory s
        )
    public
    shouldBeInitialized {
         require(checkSignatures(keccak256(abi.encodePacked(thisContractAddress, salt)),
            v,
            r,
            s), "Peer signatures are invalid"
        );
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
        
        require(checkSignatures(keccak256(abi.encodePacked(
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
    }
    
    function sendEthToSidechain(bytes32 destination) 
    public 
    payable
    shouldBeInitialized {
        require(msg.value > 0, "ETH VALUE SHOULD BE MORE THAN 0");
        bytes32 empty;
        emit Deposit(destination, msg.value, address(0x0), empty);
    }

    /**
     * A special function-like stub to allow ether accepting
     */
    function sendERC20ToSidechain(
        bytes32 destination, 
        uint amount, 
        address tokenAddress) 
        external 
        shouldBeInitialized {
            
        IERC20 token = IERC20(tokenAddress);
        
        require (token.allowance(msg.sender, address(this)) >= amount, "NOT ENOUGH DELEGATED TOKENS ON SENDER BALANCE");

        bytes32 sidechainAssetId = _sidechainTokensByAddress[tokenAddress];
        if(_sidechainTokens[sidechainAssetId] != address(0x0)) {
            MasterToken mtoken = MasterToken(tokenAddress);
            mtoken.burnFrom(msg.sender, amount);
        } else {
            token.transferFrom(msg.sender, address(this), amount);
        }
        emit Deposit(destination, amount, tokenAddress, sidechainAssetId);
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
        require(checkSignatures(keccak256(abi.encodePacked(newPeerAddress, txHash)),
            v,
            r,
            s), "Peer signatures are invalid"
        );

        addPeer(newPeerAddress);
        used[txHash] = true;
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
                keccak256(abi.encodePacked(peerAddress, txHash)),
                v,
                r,
                s), "Peer signatures are invalid"
        );

        removePeer(peerAddress);
        used[txHash] = true;
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
                keccak256(abi.encodePacked(tokenAddress, amount, to, txHash, from)),
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
     * @param beneficiary destination address
     * @param txHash hash of transaction from Iroha
     * @param v array of signatures of tx_hash (v-component)
     * @param r array of signatures of tx_hash (r-component)
     * @param s array of signatures of tx_hash (s-component)
     */
    function receiveBySidechainAssetId(
        bytes32 sidechainAssetId,
        uint256 amount,
        address beneficiary,
        bytes32 txHash,
        uint8[] memory v,
        bytes32[] memory r,
        bytes32[] memory s,
        address from
    )
    public
    {   
        require(_sidechainTokens[sidechainAssetId] != address(0x0), "Sidechain asset is not registered");
        MasterToken tokenInstance = MasterToken(_sidechainTokens[sidechainAssetId]);       
        require(used[txHash] == false);
        require(checkSignatures(
                keccak256(abi.encodePacked(sidechainAssetId, amount, beneficiary, txHash, from)),
                v,
                r,
                s), "Peer signatures are invalid"
        );

        tokenInstance.mintTokens(beneficiary, amount);
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