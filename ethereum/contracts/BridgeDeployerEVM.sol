pragma solidity ^0.7.4;
// "SPDX-License-Identifier: Apache License 2.0"

import "./BridgeEVM.sol";

contract BridgeDeployerEVM {

    bytes32 public _networkId;
    address[] public _initialPeers;

    event NewBridgeDeployedEVM(address bridgeAddress);

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
        emit NewBridgeDeployedEVM(address(new BridgeEVM(_initialPeers, _networkId)));
    }
}
