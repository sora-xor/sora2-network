// SPDX-License-Identifier: Apache-2.0
pragma solidity =0.8.13;

import "@openzeppelin/contracts/access/AccessControl.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/extensions/ERC20Burnable.sol";
import "./MasterToken.sol";
import "./ScaleCodec.sol";
import "./OutboundChannel.sol";

enum ChannelId {
    Basic,
    Incentivized
}

contract SidechainApp is AccessControl {
    using ScaleCodec for uint256;

    mapping(address => bool) public tokens;

    mapping(ChannelId => Channel) public channels;

    bytes2 constant MINT_CALL = 0x6500;
    bytes2 constant REGISTER_ASSET_CALL = 0x6501;

    event Burned(
        address token,
        address sender,
        bytes32 recipient,
        uint256 amount
    );

    event Minted(
        address token,
        bytes32 sender,
        address recipient,
        uint256 amount
    );

    struct Channel {
        address inbound;
        address outbound;
    }

    bytes32 public constant INBOUND_CHANNEL_ROLE =
        keccak256("INBOUND_CHANNEL_ROLE");

    constructor(Channel memory _basic, Channel memory _incentivized) {
        Channel storage c1 = channels[ChannelId.Basic];
        c1.inbound = _basic.inbound;
        c1.outbound = _basic.outbound;

        Channel storage c2 = channels[ChannelId.Incentivized];
        c2.inbound = _incentivized.inbound;
        c2.outbound = _incentivized.outbound;

        _setupRole(INBOUND_CHANNEL_ROLE, _basic.inbound);
        _setupRole(INBOUND_CHANNEL_ROLE, _incentivized.inbound);
    }

    function lock(
        address _token,
        bytes32 _recipient,
        uint256 _amount,
        ChannelId _channelId
    ) public {
        require(tokens[_token], "Token is not registered");
        require(
            _channelId == ChannelId.Basic ||
                _channelId == ChannelId.Incentivized,
            "Invalid channel ID"
        );

        ERC20Burnable mtoken = ERC20Burnable(_token);
        mtoken.burnFrom(msg.sender, _amount);
        emit Burned(_token, msg.sender, _recipient, _amount);

        bytes memory call = mintCall(_token, msg.sender, _recipient, _amount);

        OutboundChannel channel = OutboundChannel(
            channels[_channelId].outbound
        );
        channel.submit(msg.sender, call);
    }

    function unlock(
        address _token,
        bytes32 _sender,
        address _recipient,
        uint256 _amount
    ) public onlyRole(INBOUND_CHANNEL_ROLE) {
        require(tokens[_token], "Token is not registered");

        MasterToken tokenInstance = MasterToken(_token);
        tokenInstance.mintTokens(_recipient, _amount);
        emit Minted(_token, _sender, _recipient, _amount);
    }

    // SCALE-encode payload
    function mintCall(
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

    // SCALE-encode payload
    function registerAssetCall(address _token, bytes32 _asset_id)
        private
        pure
        returns (bytes memory)
    {
        return abi.encodePacked(REGISTER_ASSET_CALL, _asset_id, _token);
    }

    /**
     * Add new token from sidechain to the bridge white list.
     *
     * @param name token title
     * @param symbol token symbol
     * @param sidechainAssetId token id on the sidechain
     */
    function registerAsset(
        string memory name,
        string memory symbol,
        bytes32 sidechainAssetId
    ) public onlyRole(INBOUND_CHANNEL_ROLE) {
        // Create new instance of the token
        MasterToken tokenInstance = new MasterToken(
            name,
            symbol,
            address(this),
            0,
            sidechainAssetId
        );
        address tokenAddress = address(tokenInstance);
        tokens[tokenAddress] = true;

        bytes memory call = registerAssetCall(tokenAddress, sidechainAssetId);

        OutboundChannel channel = OutboundChannel(
            channels[ChannelId.Basic].outbound
        );
        channel.submit(msg.sender, call);
    }
}
