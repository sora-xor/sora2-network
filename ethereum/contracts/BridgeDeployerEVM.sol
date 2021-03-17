pragma solidity ^0.7.4;
// "SPDX-License-Identifier: Apache License 2.0"

import "./BridgeEVM.sol";

contract BridgeDeployer {
    
    bytes32 public _networkId;
    address[] public _initialPeers;
    Bridge public _bridge;
    
    event NewBridgeDeployed(address bridgeAddress);

    /**
     * Constructor.
     * @param initialPeers - list of initial bridge validators on substrate side.
     * @param networkId id of current EVM network used for bridge purpose.
     */
    constructor(
        address[] memory initialPeers,
        bytes32 networkId)  {
        _initialPeers = initialPeers;
        _networkId = networkId;
    }
    
    function deployBridgeContract() public {
        _bridge = new Bridge(_initialPeers, _networkId);
        
        emit NewBridgeDeployed(address(_bridge));
    } 
}
