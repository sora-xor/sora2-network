// SPDX-License-Identifier: Apache-2.0
pragma solidity =0.8.13;

import "../IncentivizedOutboundChannel.sol";
import "@openzeppelin/contracts/access/Ownable.sol";

contract TestIncentivizedOutboundChannel is
    IncentivizedOutboundChannel,
    Ownable
{
    constructor() IncentivizedOutboundChannel() {}

    function reset() public onlyOwner {
        nonce = 0;
    }
}
