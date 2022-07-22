// SPDX-License-Identifier: Apache-2.0
pragma solidity =0.8.13;

import "../BasicInboundChannel.sol";
import "@openzeppelin/contracts/access/Ownable.sol";

contract TestBasicInboundChannel is BasicInboundChannel, Ownable {
    constructor(BeefyLightClient _beefyLightClient)
        BasicInboundChannel(_beefyLightClient)
    {}

    function reset() public onlyOwner {
        nonce = 0;
    }
}
