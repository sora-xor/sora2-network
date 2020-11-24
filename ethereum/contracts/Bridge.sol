pragma solidity ^0.6.12;
// "SPDX-License-Identifier: Apache License 2.0"

import "./IERC20.sol";
import "./MasterToken.sol";

/**
 * Provides functionality of master contract
 */
contract Bridge {
    bool internal initialized_;
    bool internal enabled_;
    bytes32 public proof;
    uint256 public proofReward;
    address public owner_;
    mapping(address => bool) public isPeer;
    uint public peersCount;
    /** Iroha tx hashes used */
    mapping(bytes32 => bool) public used;
    mapping(address => bool) public uniqueAddresses;

    MasterToken public tokenInstance;

    mapping(address => bool) public isToken;

    event Withdrawal(bytes32 txHash);
    event Deposit(string destination, uint amount, address token);

    event EnableContract(address provider, bytes32 proof);

    /**
     * Constructor. Sets contract owner to contract creator.
     */
    constructor(
        address[] memory initialPeers, 
        string memory name, 
        string memory symbol, 
        uint8 decimals, 
        address beneficiary, 
        uint256 supply, 
        uint256 reward) public {
        initialize(msg.sender, initialPeers, name, symbol, decimals, beneficiary, supply, reward);
    }

    /**
     * Initialization of smart contract.
     */
    function initialize(address owner, address[] memory initialPeers, string memory name, string memory symbol, uint8 decimals, address beneficiary, uint256 supply, uint256 reward) public {
        require(!initialized_);

        owner_ = owner;
        for (uint8 i = 0; i < initialPeers.length; i++) {
            addPeer(initialPeers[i]);
        }

        // 0 means ether which is definitely in whitelist
        isToken[address(0)] = true;

        // Create new instance of the token
        tokenInstance = new MasterToken(name, symbol, decimals, beneficiary, supply);
        isToken[address(tokenInstance)] = true;

        proofReward = reward;

        initialized_ = true;
    }

    /**
     * @dev Throws if called by any account other than the owner.
     */
    modifier onlyOwner() {
        require(isOwner());
        _;
    }

    /**
     * @dev Throws if called when the contract is disabled.
     */
    modifier enabled() {
        require(enabled_);
        _;
    }

    /**
     * @return true if `msg.sender` is the owner of the contract.
     */
    function isOwner() public view returns (bool) {
        return msg.sender == owner_;
    }
    
    function depositEth(string memory destination) 
    public 
    payable {
        require(msg.value > 0);

        emit Deposit(destination, msg.value, address(0x0));
    }

    /**
     * A special function-like stub to allow ether accepting
     */
    function depositERC20(
        string memory destination, 
        uint amount, 
        address tokenAddress) 
        external 
        payable {
            
        IERC20 token = IERC20(tokenAddress);
        
        require (token.allowance(msg.sender, address(this)) >= amount, "NOT ENOUGH DELEGATED TOKENS ON SENDER BALANCE");

        token.transferFrom(msg.sender, address(this), amount);
        
        emit Deposit(destination, amount, tokenAddress);
    }

    function submitProof(
        bytes32 proofArg,
        uint8[] memory v,
        bytes32[] memory r,
        bytes32[] memory s
    )
    public
    {
        require(!enabled_, "Proof has been submitted already");
        require(checkSignatures(
                keccak256(abi.encodePacked(proofArg)),
                v,
                r,
                s)
        );

        tokenInstance.mintTokens(msg.sender, proofReward);

        proof = proofArg;
        enabled_ = true;
        emit EnableContract(msg.sender, proofArg);
    }

    /**
     * Adds new peer to list of signature verifiers. Can be called only by contract owner.
     * @param newAddress address of new peer
     */
    function addPeer(address newAddress) private returns (uint) {
        require(isPeer[newAddress] == false);
        isPeer[newAddress] = true;
        ++peersCount;
        return peersCount;
    }

    function removePeer(address peerAddress) private {
        require(isPeer[peerAddress] == true);
        isPeer[peerAddress] = false;
        --peersCount;
    }

    function addPeerByPeer(
        address newPeerAddress,
        bytes32 txHash,
        uint8[] memory v,
        bytes32[] memory r,
        bytes32[] memory s
    )
    public returns (bool)
    {
        require(used[txHash] == false);
        require(checkSignatures(keccak256(abi.encodePacked(newPeerAddress, txHash)),
            v,
            r,
            s)
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
    public returns (bool)
    {
        require(used[txHash] == false);
        require(checkSignatures(
                keccak256(abi.encodePacked(peerAddress, txHash)),
                v,
                r,
                s)
        );

        removePeer(peerAddress);
        used[txHash] = true;
        return true;
    }

    /**
     * Adds new token to whitelist. Token should not been already added.
     * @param newToken token to add
     */
    function addToken(address newToken) public onlyOwner {
        require(isToken[newToken] == false);
        isToken[newToken] = true;
    }

    /**
     * Checks is given token inside a whitelist or not
     * @param tokenAddress address of token to check
     * @return true if token inside whitelist or false otherwise
     */
    function checkTokenAddress(address tokenAddress) public view returns (bool) {
        return isToken[tokenAddress];
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
    function withdraw(
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
    enabled
    {
        require(checkTokenAddress(tokenAddress));
        require(used[txHash] == false);
        require(checkSignatures(
                keccak256(abi.encodePacked(tokenAddress, amount, to, txHash, from)),
                v,
                r,
                s)
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
    ) private returns (bool) {
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
            if (isPeer[recoveredAddress] != true || uniqueAddresses[recoveredAddress] == true) {
                continue;
            }
            recoveredAddresses[count] = recoveredAddress;
            count = count + 1;
            uniqueAddresses[recoveredAddress] = true;
        }

        // restore state for future usages
        for (uint i = 0; i < count; ++i) {
            uniqueAddresses[recoveredAddresses[i]] = false;
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
    function recoverAddress(bytes32 hash, uint8 v, bytes32 r, bytes32 s) private pure returns (address) {
        bytes32 simple_hash = keccak256(abi.encodePacked("\x19Ethereum Signed Message:\n32", hash));
        address res = ecrecover(simple_hash, v, r, s);
        return res;
    }

    /**
     * Mint new Token
     * @param tokenAddress address to mint
     * @param amount how much to mint
     * @param beneficiary destination address
     * @param txHash hash of transaction from Iroha
     * @param v array of signatures of tx_hash (v-component)
     * @param r array of signatures of tx_hash (r-component)
     * @param s array of signatures of tx_hash (s-component)
     */
    function mintTokensByPeers(
        address tokenAddress,
        uint256 amount,
        address beneficiary,
        bytes32 txHash,
        uint8[] memory v,
        bytes32[] memory r,
        bytes32[] memory s,
        address from
    )
    public
    enabled
    {
        require(address(tokenInstance) == tokenAddress);
        require(used[txHash] == false);
        require(checkSignatures(
                keccak256(abi.encodePacked(tokenAddress, amount, beneficiary, txHash, from)),
                v,
                r,
                s)
        );

        tokenInstance.mintTokens(beneficiary, amount);
        used[txHash] = true;
        emit Withdrawal(txHash);
    }
}