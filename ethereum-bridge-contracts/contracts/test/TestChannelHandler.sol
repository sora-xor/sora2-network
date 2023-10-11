// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.15;

import "../ChannelHandler.sol";

contract TestInboundChannel is ChannelHandler {
    constructor()
        ChannelHandler()
    {}

    function reset() external onlyOwner {
        batchNonce = 0;
    }
}
