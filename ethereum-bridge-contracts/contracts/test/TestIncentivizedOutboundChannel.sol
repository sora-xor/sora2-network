// SPDX-License-Identifier: Apache-2.0
pragma solidity =0.8.13;

import "../IncentivizedOutboundChannel.sol";

contract TestIncentivizedOutboundChannel is IncentivizedOutboundChannel {
    address public deployer;

    constructor() IncentivizedOutboundChannel() {
        deployer = msg.sender;
    }

    function reset() public {
        require(msg.sender == deployer, "Only deployer can reset contract");
        nonce = 0;
    }
}
