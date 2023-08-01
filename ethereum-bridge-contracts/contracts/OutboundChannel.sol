// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.15;

import "@openzeppelin/contracts/access/AccessControl.sol";
import "./interfaces/IOutboundChannel.sol";
import "./ChannelAccess.sol";

// OutboundChannel is a channel that sends ordered messages with an increasing nonce. It will have
// incentivization too.
contract OutboundChannel is IOutboundChannel, ChannelAccess, AccessControl {
    // Governance contracts will administer using this role.
    bytes32 public constant CONFIG_UPDATE_ROLE =
        keccak256("CONFIG_UPDATE_ROLE");

    // Nonce for last submitted message
    uint64 public nonce;

    constructor() {
        _setupRole(DEFAULT_ADMIN_ROLE, msg.sender);
    }

    // Once-off post-construction call to set initial configuration.
    function initialize(
        address[] calldata configUpdaters,
        address[] calldata defaultOperatorsSet
    ) external onlyRole(DEFAULT_ADMIN_ROLE) {
        // Set initial configuration
        for (uint256 i = 0; i < configUpdaters.length; i++) {
            grantRole(CONFIG_UPDATE_ROLE, configUpdaters[i]);
        }
        for (uint256 i = 0; i < defaultOperatorsSet.length; i++) {
            _authorizeDefaultOperator(defaultOperatorsSet[i]);
        }
        // drop admin privileges
        renounceRole(DEFAULT_ADMIN_ROLE, msg.sender);
    }

    // Authorize an operator/app to submit messages for *all* users.
    function authorizeDefaultOperator(address operator)
        external
        onlyRole(CONFIG_UPDATE_ROLE)
    {
        _authorizeDefaultOperator(operator);
    }

    // Revoke authorization.
    function revokeDefaultOperator(address operator)
        external
        onlyRole(CONFIG_UPDATE_ROLE)
    {
        _revokeDefaultOperator(operator);
    }

    /**
     * @dev Sends a message across the channel
     */
    function submit(address feePayer, bytes calldata payload)
        external
        override
    {
        require(
            isOperatorFor(msg.sender, feePayer),
            "Caller is not an operator for fee payer"
        );
        nonce = nonce + 1;
        emit Message(msg.sender, nonce, payload);
    }
}
