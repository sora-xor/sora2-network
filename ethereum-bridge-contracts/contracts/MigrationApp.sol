// SPDX-License-Identifier: Apache-2.0
pragma solidity =0.8.13;

import "@openzeppelin/contracts/access/AccessControl.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/extensions/ERC20Burnable.sol";
import "./MasterToken.sol";
import "./ScaleCodec.sol";
import "./OutboundChannel.sol";
import "./IAssetRegister.sol";
import "./EthTokenReceiver.sol";

enum ChannelId {
    Basic,
    Incentivized
}

contract MigrationApp is AccessControl, EthTokenReceiver {
    using ScaleCodec for uint256;

    mapping(ChannelId => Channel) public channels;

    struct Channel {
        address inbound;
        address outbound;
    }

    bytes32 public constant INBOUND_CHANNEL_ROLE =
        keccak256("INBOUND_CHANNEL_ROLE");

    constructor(
        address _basic_inbound,
        address _basic_outbound,
        address _incentivized_inbound,
        address _incentivized_outbound
    ) {
        Channel storage c1 = channels[ChannelId.Basic];
        c1.inbound = _basic_inbound;
        c1.outbound = _basic_outbound;

        Channel storage c2 = channels[ChannelId.Incentivized];
        c2.inbound = _incentivized_inbound;
        c2.outbound = _incentivized_outbound;

        _setupRole(INBOUND_CHANNEL_ROLE, _basic_inbound);
        _setupRole(INBOUND_CHANNEL_ROLE, _incentivized_inbound);
    }

    event MigratedNativeErc20(address contractAddress);

    function migrateNativeErc20(
        address contractAddress,
        address[] calldata erc20nativeTokens
    ) public onlyRole(INBOUND_CHANNEL_ROLE) {
        IAssetRegister app = IAssetRegister(contractAddress);
        for (uint256 i = 0; i < erc20nativeTokens.length; i++) {
            IERC20 token = IERC20(erc20nativeTokens[i]);
            token.transfer(contractAddress, token.balanceOf(address(this)));
            app.registerExistingAsset(erc20nativeTokens[i]);
        }
        emit MigratedNativeErc20(contractAddress);
    }

    event MigratedEth(address contractAddress);

    function migrateEth(address contractAddress)
        public
        onlyRole(INBOUND_CHANNEL_ROLE)
    {
        EthTokenReceiver receiver = EthTokenReceiver(contractAddress);
        receiver.receivePayment{value: address(this).balance}();
        emit MigratedEth(contractAddress);
    }

    event MigratedSidechain(address contractAddress);

    function migrateSidechain(
        address contractAddress,
        address[] calldata sidechainTokens
    ) public onlyRole(INBOUND_CHANNEL_ROLE) {
        IAssetRegister app = IAssetRegister(contractAddress);
        for (uint256 i = 0; i < sidechainTokens.length; i++) {
            Ownable token = Ownable(sidechainTokens[i]);
            token.transferOwnership(contractAddress);
            app.registerExistingAsset(sidechainTokens[i]);
        }
        emit MigratedSidechain(contractAddress);
    }

    function receivePayment() external payable override {}
}
