// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.15;

import "@openzeppelin/contracts/access/AccessControl.sol";
import "./interfaces/IOutboundChannel.sol";

contract GenericApp is AccessControl {
    IOutboundChannel public outbound;
    address public inbound;

    bytes32 public constant INBOUND_CHANNEL_ROLE =
        keccak256("INBOUND_CHANNEL_ROLE");

    constructor(address _inbound, address _outbound) {
        _setupRole(INBOUND_CHANNEL_ROLE, _inbound);
        outbound = IOutboundChannel(_outbound);
        inbound = _inbound;
    }
}
