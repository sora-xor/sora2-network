// SPDX-License-Identifier: Apache-2.0
pragma solidity =0.8.13;

import "@openzeppelin/contracts/security/ReentrancyGuard.sol";
import "./RewardSource.sol";
import "./ScaleCodec.sol";
import "./EthTokenReceiver.sol";
import "./GenericApp.sol";

contract ETHApp is GenericApp, RewardSource, EthTokenReceiver, ReentrancyGuard {
    using ScaleCodec for uint256;

    event Locked(address sender, bytes32 recipient, uint256 amount);

    event Unlocked(bytes32 sender, address recipient, uint256 amount);

    bytes2 constant MINT_CALL = 0x6401;

    bytes32 public constant REWARD_ROLE = keccak256("REWARD_ROLE");

    constructor(
        address rewarder,
        address _inbound,
        OutboundChannel _outbound
    ) GenericApp(_inbound, _outbound) {
        _setupRole(REWARD_ROLE, rewarder);
    }

    function lock(bytes32 _recipient) public payable {
        require(msg.value > 0, "Value of transaction must be positive");

        emit Locked(msg.sender, _recipient, msg.value);

        bytes memory call = encodeCall(msg.sender, _recipient, msg.value);

        outbound.submit(msg.sender, call);
    }

    function unlock(
        bytes32 _sender,
        address payable _recipient,
        uint256 _amount
    ) public onlyRole(INBOUND_CHANNEL_ROLE) nonReentrant {
        require(_amount > 0, "Must unlock a positive amount");
        (bool success, ) = _recipient.call{value: _amount}("");
        require(success, "Transfer failed.");
        emit Unlocked(_sender, _recipient, _amount);
    }

    // SCALE-encode payload
    function encodeCall(
        address _sender,
        bytes32 _recipient,
        uint256 _amount
    ) private pure returns (bytes memory) {
        return
            abi.encodePacked(
                MINT_CALL,
                _sender,
                //bytes1(0x00), // Encode recipient as MultiAddress::Id
                _recipient,
                _amount.encode256()
            );
    }

    function reward(address payable _recipient, uint256 _amount)
        external
        override
        onlyRole(REWARD_ROLE)
        nonReentrant
    {
        (bool success, ) = _recipient.call{value: _amount}("");
        require(success, "Transfer failed.");
    }

    function receivePayment() external payable override {}
}
