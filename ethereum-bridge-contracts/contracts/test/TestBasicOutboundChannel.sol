// SPDX-License-Identifier: Apache-2.0
pragma solidity =0.8.13;

import "../BasicOutboundChannel.sol";
import "@openzeppelin/contracts/access/Ownable.sol";

contract TestBasicOutboundChannel is BasicOutboundChannel, Ownable {
    constructor() BasicOutboundChannel() {}

    function reset() public onlyOwner {
        nonce = 0;
    }
}
