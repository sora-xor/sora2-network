// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.15;

import "../InboundChannel.sol";
import "@openzeppelin/contracts/access/Ownable.sol";

contract TestInboundChannel is InboundChannel, Ownable {
    constructor(address _beefyLightClient)
        InboundChannel(_beefyLightClient)
    {}

    function reset() external onlyOwner {
        batch_nonce = 0;
    }
}
