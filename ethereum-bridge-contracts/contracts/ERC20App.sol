// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.15;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";
import "./libraries/ScaleCodec.sol";
import "./interfaces/IAssetRegister.sol";
import "./GenericApp.sol";

contract ERC20App is GenericApp, IAssetRegister, ReentrancyGuard {
    using ScaleCodec for uint256;
    using SafeERC20 for IERC20;

    mapping(address => bool) public tokens;

    bytes2 constant MINT_CALL = 0x6500;

    event Locked(
        address token,
        address sender,
        bytes32 recipient,
        uint256 amount
    );

    event Unlocked(
        address token,
        bytes32 sender,
        address recipient,
        uint256 amount
    );

    constructor(
        address _inbound,
        address _outbound, // an address of an IOutboundChannel contract
        address migrationApp
    ) GenericApp(_inbound, _outbound) {
        _setupRole(INBOUND_CHANNEL_ROLE, migrationApp);
    }

    function lock(
        address token,
        bytes32 recipient,
        uint256 amount
    ) external {
        require(tokens[token], "Token is not registered");
        require(amount > 0, "Must lock a positive amount");

        IERC20 _token = IERC20(token);
        uint256 beforeBalance = _token.balanceOf(address(this));
        _token.safeTransferFrom(msg.sender, address(this), amount);
        uint256 transferredAmount = _token.balanceOf(address(this)) -
            beforeBalance;

        emit Locked(token, msg.sender, recipient, transferredAmount);

        bytes memory call = encodeCall(
            token,
            msg.sender,
            recipient,
            transferredAmount
        );

        outbound.submit(msg.sender, call);
    }

    function unlock(
        address token,
        bytes32 sender,
        address recipient,
        uint256 amount
    ) external onlyRole(INBOUND_CHANNEL_ROLE) nonReentrant {
        require(tokens[token], "Token is not registered");
        require(
            recipient != address(0x0),
            "Recipient must not be a zero address"
        );
        require(amount > 0, "Must unlock a positive amount");
        IERC20(token).safeTransfer(recipient, amount);
        emit Unlocked(token, sender, recipient, amount);  
    }

    // SCALE-encode payload
    function encodeCall(
        address token,
        address sender,
        bytes32 recipient,
        uint256 amount
    ) private pure returns (bytes memory) {
        return
            abi.encodePacked(
                MINT_CALL,
                token,
                sender,
                recipient,
                amount.encode256()
            );
    }

    /**
     * @dev Adds a new token from sidechain to the bridge whitelist.
     * @param token token address
     */
    function addTokenToWhitelist(address token)
        external
        onlyRole(INBOUND_CHANNEL_ROLE)
    {
        require(!tokens[token], "Token is already registered");
        tokens[token] = true;
    }
}
