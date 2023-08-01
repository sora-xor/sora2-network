// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.15;

import "../InboundChannel.sol";

contract TestInboundChannel is InboundChannel {
    constructor()
        InboundChannel()
    {}

    function reset() external onlyOwner {
        batch_nonce = 0;
    }
}
