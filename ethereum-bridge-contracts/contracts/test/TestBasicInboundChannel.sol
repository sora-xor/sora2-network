// SPDX-License-Identifier: Apache-2.0
pragma solidity =0.8.13;

import "../BasicInboundChannel.sol";

contract TestBasicInboundChannel is BasicInboundChannel {
    address public deployer;

    constructor(BeefyLightClient _beefyLightClient)
        BasicInboundChannel(_beefyLightClient)
    {
        deployer = msg.sender;
    }

    function reset() public {
        require(msg.sender == deployer, "Only deployer can reset contract");
        nonce = 0;
    }
}
