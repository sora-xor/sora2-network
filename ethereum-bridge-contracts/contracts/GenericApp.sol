// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.15;

import "@openzeppelin/contracts/access/AccessControl.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";
import "./interfaces/IChannelHandler.sol";

contract GenericApp is AccessControl, ReentrancyGuard {
    IChannelHandler public immutable handler;

    bytes32 public constant INBOUND_CHANNEL_ROLE =
        keccak256("INBOUND_CHANNEL_ROLE");

    constructor(address inboundChannel) {
        require(inboundChannel != address(0), "Invalid inbound channel address");
        _setupRole(DEFAULT_ADMIN_ROLE, inboundChannel);
        _setupRole(INBOUND_CHANNEL_ROLE, inboundChannel);
        handler = IChannelHandler(inboundChannel);
    }
}
