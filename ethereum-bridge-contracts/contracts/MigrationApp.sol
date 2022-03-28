// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.5;
pragma abicoder v1;

import "@openzeppelin/contracts/access/AccessControl.sol";
import "./sora2-eth/IERC20.sol";
import "./sora2-eth/MasterToken.sol";
import "./sora2-eth/ERC20Burnable.sol";
import "./ScaleCodec.sol";
import "./OutboundChannel.sol";
import "./IAssetRegister.sol";

enum ChannelId {
    Basic,
    Incentivized
}

contract MigrationApp is AccessControl {
    using ScaleCodec for uint256;

    mapping(ChannelId => Channel) public channels;

    bool public erc20_migrated;
    bool public eth_migrated;
    bool public sidechain_migrated;

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

        erc20_migrated = false;
        eth_migrated = false;
        sidechain_migrated = false;
    }

    /*
    Used to recieve Eth from old Bridge contract
    */
    function receivePayment() external payable {}

    event MigratedNativeErc20(address contractAddress);

    function migrateNativeErc20(
        address contractAddress,
        address[] calldata erc20nativeTokens
    ) public onlyRole(INBOUND_CHANNEL_ROLE) {
        require(erc20_migrated == false, "ERC20 assets already migrated");
        IAssetRegister app = IAssetRegister(contractAddress);
        for (uint256 i = 0; i < erc20nativeTokens.length; i++) {
            IERC20 token = IERC20(erc20nativeTokens[i]);
            token.transfer(contractAddress, token.balanceOf(address(this)));
            app.registerExistingAsset(erc20nativeTokens[i]);
        }
        erc20_migrated = true;
        emit MigratedNativeErc20(contractAddress);
    }

    event MigratedEth(address contractAddress);

    function migrateEth(address payable contractAddress)
        public
        onlyRole(INBOUND_CHANNEL_ROLE)
    {
        require(eth_migrated == false, "Eth asset already migrated");
        contractAddress.transfer(address(this).balance);
        eth_migrated = true;
        emit MigratedEth(contractAddress);
    }

    event MigratedSidechain(address contractAddress);

    function migrateSidechain(
        address contractAddress,
        address[] calldata sidechainTokens
    ) public onlyRole(INBOUND_CHANNEL_ROLE) {
        require(
            sidechain_migrated == false,
            "Sidechain assets already migrated"
        );
        IAssetRegister app = IAssetRegister(contractAddress);
        for (uint256 i = 0; i < sidechainTokens.length; i++) {
            Ownable token = Ownable(sidechainTokens[i]);
            token.transferOwnership(contractAddress);
            app.registerExistingAsset(sidechainTokens[i]);
        }
        sidechain_migrated = true;
        emit MigratedSidechain(contractAddress);
    }
}
