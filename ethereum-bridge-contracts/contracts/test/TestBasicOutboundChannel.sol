// SPDX-License-Identifier: Apache-2.0
pragma solidity =0.8.13;

import "../BasicOutboundChannel.sol";

contract TestBasicOutboundChannel is BasicOutboundChannel {
    address public deployer;

    constructor() BasicOutboundChannel() {
        deployer = msg.sender;
    }

    function reset() public {
        require(msg.sender == deployer, "Only deployer can reset contract");
        nonce = 0;
    }
}
