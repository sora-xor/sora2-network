// SPDX-License-Identifier: Apache-2.0
pragma solidity =0.8.13;

import "@openzeppelin/contracts/access/AccessControl.sol";
import "./OutboundChannel.sol";

contract GenericApp is AccessControl {
    OutboundChannel public outbound;

    address public inbound;

    bytes32 public constant INBOUND_CHANNEL_ROLE =
        keccak256("INBOUND_CHANNEL_ROLE");

    constructor(address _inbound, OutboundChannel _outbound) {
        _setupRole(INBOUND_CHANNEL_ROLE, _inbound);
        outbound = _outbound;
        inbound = _inbound;
    }
}
