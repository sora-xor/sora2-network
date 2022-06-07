// SPDX-License-Identifier: Apache-2.0
pragma solidity =0.8.13;

import "../IncentivizedInboundChannel.sol";
import "@openzeppelin/contracts/access/Ownable.sol";

contract TestIncentivizedInboundChannel is IncentivizedInboundChannel, Ownable {
    constructor(BeefyLightClient _beefyLightClient)
        IncentivizedInboundChannel(_beefyLightClient)
    {}

    function reset() public onlyOwner {
        nonce = 0;
    }
}
