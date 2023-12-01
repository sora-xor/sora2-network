// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.15;

import "@openzeppelin/contracts/security/ReentrancyGuard.sol";
import "./libraries/ScaleCodec.sol";
import "./interfaces/IEthTokenReceiver.sol";
import "./GenericApp.sol";

/** 
* @dev The contract was analyzed using Slither static analysis framework. All recommendations have been taken 
* into account and some detectors have been disabled at developers' discretion using `slither-disable-next-line`. 
*/
contract ETHApp is
    GenericApp,
    IEthTokenReceiver,
    ReentrancyGuard
{
    using ScaleCodec for uint256;

    event Locked(address sender, bytes32 recipient, uint256 amount);

    event Unlocked(bytes32 sender, address recipient, uint256 amount);

    bytes2 constant MINT_CALL = 0x6401;

    bytes32 public constant REWARD_ROLE = keccak256("REWARD_ROLE");

    constructor(
        address rewarder,
        address inboundChannel,
        address outboundChannel // an address of an IOutboundChannel contract
    ) GenericApp(inboundChannel, outboundChannel) {
        _setupRole(REWARD_ROLE, rewarder);
    }

    function lock(bytes32 recipient) external payable {
        require(msg.value > 0, "Value of transaction must be positive");

        emit Locked(msg.sender, recipient, msg.value);

        bytes memory call = encodeCall(msg.sender, recipient, msg.value);

        outbound.submit(msg.sender, call);
    }

    function unlock(
        bytes32 sender,
        address payable recipient,
        uint256 amount
    ) external onlyRole(INBOUND_CHANNEL_ROLE) nonReentrant {
        require(
            recipient != address(0x0),
            "Recipient must not be a zero address"
        );
        require(amount > 0, "Must unlock a positive amount");
        // slither-disable-next-line arbitrary-send,low-level-calls
        (bool success, ) = recipient.call{value: amount}("");
        require(success, "Transfer failed.");
        emit Unlocked(sender, recipient, amount);
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
                amount.encode256()
            );
    }

    function receivePayment() external payable override {}
}
