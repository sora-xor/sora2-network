// SPDX-License-Identifier: Apache-2.0
pragma solidity =0.8.13;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "./ScaleCodec.sol";
import "./IAssetRegister.sol";
import "./GenericApp.sol";

contract ERC20App is GenericApp, IAssetRegister {
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
        OutboundChannel _outbound,
        address migrationApp
    ) GenericApp(_inbound, _outbound) {
        _setupRole(INBOUND_CHANNEL_ROLE, migrationApp);
    }

    function lock(
        address _token,
        bytes32 _recipient,
        uint256 _amount
    ) public {
        require(tokens[_token], "Token is not registered");
        require(_amount > 0, "Must lock a positive amount");

        IERC20 token = IERC20(_token);
        uint256 beforeBalance = token.balanceOf(address(this));
        IERC20(_token).safeTransferFrom(msg.sender, address(this), _amount);
        uint256 transferredAmount = token.balanceOf(address(this)) -
            beforeBalance;

        emit Locked(_token, msg.sender, _recipient, transferredAmount);

        bytes memory call = encodeCall(
            _token,
            msg.sender,
            _recipient,
            transferredAmount
        );

        outbound.submit(msg.sender, call);
    }

    function unlock(
        address _token,
        bytes32 _sender,
        address _recipient,
        uint256 _amount
    ) public onlyRole(INBOUND_CHANNEL_ROLE) {
        require(tokens[_token], "Token is not registered");
        require(
            _recipient != address(0x0),
            "Recipient must not be a zero address"
        );
        require(_amount > 0, "Must unlock a positive amount");
        IERC20(_token).safeTransfer(_recipient, _amount);
        emit Unlocked(_token, _sender, _recipient, _amount);
    }

    // SCALE-encode payload
    function encodeCall(
        address _token,
        address _sender,
        bytes32 _recipient,
        uint256 _amount
    ) private pure returns (bytes memory) {
        return
            abi.encodePacked(
                MINT_CALL,
                _token,
                _sender,
                _recipient,
                _amount.encode256()
            );
    }

    /**
     * Add new token from sidechain to the bridge white list.
     *
     * @param token token address
     */
    function registerAsset(address token)
        public
        onlyRole(INBOUND_CHANNEL_ROLE)
    {
        tokens[token] = true;
    }

    function registerExistingAsset(address token)
        public
        override
        onlyRole(INBOUND_CHANNEL_ROLE)
    {
        tokens[token] = true;
    }
}
