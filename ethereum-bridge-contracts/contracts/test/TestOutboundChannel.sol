// SPDX-License-Identifier: Apache-2.0
pragma solidity =0.8.13;

import "../OutboundChannel.sol";
import "@openzeppelin/contracts/access/Ownable.sol";

contract TestOutboundChannel is OutboundChannel, Ownable {
    constructor() OutboundChannel() {}

    function reset() public onlyOwner {
        nonce = 0;
    }
}
