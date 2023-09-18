// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.15;

import "./interfaces/IEthTokenReceiver.sol";
import "./GenericApp.sol";
import { InvalidAmount, InvalidRecipient, FailedCall } from "./Error.sol";

/**
 * @dev The contract was analyzed using Slither static analysis framework. All recommendations have been taken
 * into account and some detectors have been disabled at developers' discretion using `slither-disable-next-line`.
 */
contract ETHApp is GenericApp, IEthTokenReceiver {
    event Locked(address sender, bytes32 recipient, uint256 amount);
    event Unlocked(bytes32 sender, address recipient, uint256 amount);
    event MigratedEth(address contractAddress);

    bytes2 constant MINT_CALL = 0x0201;

    constructor(address channelHandler) GenericApp(channelHandler) {}

    fallback() external {
        revert();
    }

    receive() external payable {
        revert();
    }

    function lock(bytes32 recipient) external payable {
        if (msg.value == 0) revert InvalidAmount();
        if (recipient == bytes32(0)) revert InvalidRecipient();
        emit Locked(msg.sender, recipient, msg.value);
        bytes memory call = encodeCall(msg.sender, recipient, msg.value);
        handler.submitMessage(call);
    }

    function unlock(
        bytes32 sender,
        address payable recipient,
        uint256 amount
    ) external onlyRole(INBOUND_CHANNEL_ROLE) nonReentrant {
        if (recipient == address(0x0)) revert InvalidRecipient();
        if (amount == 0) revert InvalidAmount();
        // slither-disable-next-line arbitrary-send,low-level-calls
        (bool success, ) = recipient.call{value: amount}("");
        if (!success) revert FailedCall();
        emit Unlocked(sender, recipient, amount);
    }

    function migrateEth(
        address contractAddress
    ) external onlyRole(INBOUND_CHANNEL_ROLE) nonReentrant {
        IEthTokenReceiver receiver = IEthTokenReceiver(contractAddress);
        // slither-disable-next-line arbitrary-send
        receiver.receivePayment{value: address(this).balance}();
        emit MigratedEth(contractAddress);
    }

    // SCALE-encode payload
    function encodeCall(
        address sender,
        bytes32 recipient,
        uint256 amount
    ) private pure returns (bytes memory) {
        return
            abi.encodePacked(
                MINT_CALL,
                sender,
                //bytes1(0x00), // Encode recipient as MultiAddress::Id
                recipient,
                amount
            );
    }

    function receivePayment() external payable override {}
}
