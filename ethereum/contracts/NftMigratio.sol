pragma solidity ^0.7.4;
// "SPDX-License-Identifier: Apache License 2.0"

contract NftMigration {
    
    address public owner;
    address public nftCreator = 0x3482549fCa7511267C9Ef7089507c0F16eA1dcC1;
    mapping (address => bool) public acceptableAddresses;
    
    constructor(address[] memory addresses) {
        owner = msg.sender;
            for (uint i=0; i<addresses.length; i++) {
                acceptableAddresses[addresses[i]] = true;
            }
    }
    
    event Submit(
        bytes32 substrateAddress);
    
    function submit(
        bytes32 substrateAddress)
        public {
            require(msg.sender != owner, "Sender should not be contract owner");
            require(msg.sender != nftCreator, "Sender should not be NFT contract creator");
            require(acceptableAddresses[msg.sender], "Sender should be whitelisted");

            emit Submit(
                substrateAddress
            );
        }
}